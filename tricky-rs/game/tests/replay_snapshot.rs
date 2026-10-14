//! The replay's world snapshot orchestration (board row G12i) against the game in the function runner (TRICKY_ELF;
//! skipped without it): the world's 14 node lists with fake nodes on them (their snapshot, load and object virtuals
//! are logging stand-ins), keyframes saved, and keyframes restored from made-up streams holding every node type (and
//! unknown ones) and dead objects; the node classes' load constructors are stand-ins that log and take their record.
//! Every call and all the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bxlist::{self as bx, Mem, NodeEnv};
use tricky_game::replaymgr::{self as rm, SnapshotEnv};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const FN_BASE: u32 = 0x01f2_0000;
const N_NODES: u32 = 30;
const NODE: u32 = 0x40;
const POOL_LEN: u32 = 0x40000;
const SBUF_LEN: u32 = 0x800;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What the stand-ins do, the same on both sides: the object virtual answers an instance near the node's +0x30;
/// a node class's load constructor takes its 0x10-byte record
#[derive(Default)]
struct World {
    log: Vec<Call>,
    pool: u32,
    allocs: u32,
}

fn is_ctor(f: u32) -> bool {
    rm::NODE_TYPES.iter().any(|t| t.3 == f)
}

struct Ours(World, u32 /* instances */);
impl NodeEnv for Ours {
    fn virtual_call(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, [vec![this], args.to_vec()].concat()));
        if (func - FN_BASE) == 0x5c {
            self.1.wrapping_add(m.r32(this + 0x30))
        } else {
            0
        }
    }
    fn alloc(&mut self, _m: &mut Mem, size: u32) -> u32 {
        self.0.log.push(Call(0x2ccf70, vec![size]));
        0
    }
    fn free(&mut self, _m: &mut Mem, p: u32) {
        self.0.log.push(Call(0x14a040, vec![p]));
    }
    fn delete(&mut self, _m: &mut Mem, p: u32) {
        self.0.log.push(Call(0x114f80, vec![p]));
    }
}
impl SnapshotEnv for Ours {
    fn pool_alloc(&mut self, _m: &mut Mem, size: u32, name: u32) -> u32 {
        self.0.log.push(Call(0x23d3b0, vec![size, name]));
        let p = self.0.pool + 0x800 * self.0.allocs;
        self.0.allocs += 1;
        p
    }
    fn construct(&mut self, m: &mut Mem, func: u32, args: &[u32]) {
        self.0.log.push(Call(func, args.to_vec()));
        if is_ctor(func) {
            let k = args[2];
            m.w32(k + 8, m.r32(k + 8).wrapping_add(0x10));
        }
    }
}

#[test]
fn replay_snapshot_orchestration_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x17d4e8, 0x17d548, 0x17d6e0, 0x17d860, 0x17d888, 0x17d8b0, 0x17d8d8, 0x17d900, 0x17d928, 0x17da50, 0x192c10, 0x192ca0, 0x192c00,
        0x192bf0, 0x23d5b0, 0x2401b0, 0x23d5c0, 0x2401c8, 0x192d20, 0x2cc808, 0x2ce114, 0x254a28, 0x193058, 0x193060, 0x18f5d0, 0x18f800,
        0x18f8a8, 0x190660, 0x190780, 0x18f6c0,
    ] {
        r.allow(a);
    }
    let world = Rc::new(RefCell::new(World::default()));
    let instances = r.mem.alloc(0x100 * 64);
    // the node virtuals
    for slot in [0xcu32, 0x24, 0x2c, 0x44, 0x4c, 0x5c] {
        let f = FN_BASE + slot;
        let w = world.clone();
        r.stub(f, move |c, m| {
            let this = c.gpr(4) as u32;
            let args = match slot {
                0xc | 0x24 | 0x44 | 0x4c => vec![this, c.gpr(5) as u32],
                _ => vec![this],
            };
            w.borrow_mut().log.push(Call(f, args));
            let v = if slot == 0x5c { instances.wrapping_add(m.read_u32(this + 0x30)) } else { 0 };
            c.set_gpr(2, v as u64);
        });
    }
    // the pool and the constructors
    let w = world.clone();
    r.stub(0x23d3b0, move |c, _| {
        let mut w = w.borrow_mut();
        w.log.push(Call(0x23d3b0, vec![c.gpr(5) as u32, c.gpr(7) as u32]));
        let p = w.pool + 0x800 * w.allocs;
        w.allocs += 1;
        c.set_gpr(2, p as u64);
    });
    for t in rm::NODE_TYPES.iter().map(|t| t.3).chain([0x13af38]) {
        let w = world.clone();
        r.stub(t, move |c, m| {
            let args = vec![c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32];
            w.borrow_mut().log.push(Call(t, args.clone()));
            if is_ctor(t) {
                let k = args[2];
                let v = m.read_u32(k + 8);
                m.write_u32(k + 8, v.wrapping_add(0x10));
            }
        });
    }
    let lists = r.mem.alloc(14 * 0x34 + 0x10);
    let nodes = r.mem.alloc(N_NODES * NODE);
    let vt = r.mem.alloc(0x60);
    let pool = r.mem.alloc(POOL_LEN);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x80);
    let mgr = r.mem.alloc(0x80);
    let kf = r.mem.alloc(0x30);
    let sbuf = r.mem.alloc(SBUF_LEN);
    let course = r.mem.alloc(0x60);
    world.borrow_mut().pool = pool;
    let regions = [
        (0u32, 0x100u32),
        (bx::GAPP, 0x18),
        (rm::RAND_GAME, 0x18),
        (0x346488, 8),
        (lists, 14 * 0x34 + 0x10),
        (nodes, N_NODES * NODE),
        (vt, 0x60),
        (pool, POOL_LEN),
        (app, 0x800),
        (game, 0x100),
        (ctl, 0x80),
        (mgr, 0x80),
        (kf, 0x30),
        (sbuf, SBUF_LEN),
        (course, 0x60),
    ];
    let snapshot = |r: &mut Runner| {
        let mut m = Mem::default();
        for (a, n) in regions {
            let mut b = vec![0u8; n as usize];
            r.mem.read(a, &mut b).unwrap();
            m.regions.push((a, b));
        }
        m
    };
    let node = |i: u32| nodes + NODE * i;
    let mut g = Rng(0x736e_6170_7368_0f12);
    let mut seen = [0u32; 8];
    for trial in 0..2000 {
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if ![bx::GAPP, 0].contains(a) {
                v.iter_mut().for_each(|b| *b = 0);
            }
        }
        m.w32(bx::GAPP, app);
        m.w32(app + 0x730, game);
        m.w32(game + 0x40, ctl);
        m.w32(ctl + 0xc, [SBUF_LEN, 0x40, 0x20][g.below(3) as usize]);
        m.w32(ctl + 0x58, (g.below(8) == 0) as u32);
        m.w32(0x346488 + 4, course);
        m.w32(course + 0x48, instances);
        m.w32(course + 0xc, 1 + g.below(64));
        for k in 0..6 {
            m.w32(rm::RAND_GAME + 4 * k, g.next());
        }
        for slot in [0xcu32, 0x24, 0x2c, 0x44, 0x4c, 0x5c] {
            m.w32(vt + slot, FN_BASE + slot);
        }
        // the lists: made empty, then nodes on the snapshot lists, 3, 4 and 13 (some lists sorted)
        m.w32(bx::WORLD, 14);
        m.w32(bx::WORLD + 4, lists + 0x10);
        m.w32(lists, 14);
        for l in 0..14 {
            let at = lists + 0x10 + 0x34 * l;
            m.w32(at, (g.below(3) == 0) as u32);
            m.w32(at + 4, at + 0x10);
            m.w32(at + 0x14, at + 4);
            m.w32(at + 0x1c, at + 0x28);
            m.w32(at + 0x2c, at + 0x1c);
        }
        for i in 0..N_NODES {
            let n = node(i);
            m.w32(n + 8, vt);
            m.w32(n + 0x30, g.below(64 * 0x100));
            let l = [2u32, 5, 7, 8, 3, 4, 0xd, 0xd][g.below(8) as usize];
            if g.below(4) != 0 {
                let at = lists + 0x10 + 0x34 * l + 4;
                m.w32(n, m.r32(at));
                m.w32(m.r32(at) + 4, n);
                m.w32(n + 4, at);
                m.w32(at, n);
            }
        }
        // the keyframe's stream; for a restore, a made-up one
        m.w32(kf, sbuf);
        m.w32(kf + 4, sbuf + 4 * g.below(8));
        m.w32(kf + 8, sbuf);
        m.w32(mgr + 0x58, g.below(2));
        let restoring = g.below(2) == 0;
        let op = if restoring { [2usize, 3, 4, 5][g.below(4) as usize] } else { [0usize, 1, 6, 7][g.below(4) as usize] };
        // an unknown type makes nothing and leaves its record unread (the rest is then read out of step, in the
        // game too): only for the single node's restore
        let unknown_ok = op == 4;
        if restoring {
            let mut at = sbuf + 24;
            for k in 0..6 {
                m.w32(sbuf + 4 * k, g.next());
            }
            for _list in 0..2 {
                let c = g.below(6);
                m.w32(at, c);
                at += 8;
                for _ in 0..c {
                    let ty = if unknown_ok && g.below(5) == 0 { [1i32, 0x19, 0x104, 0x3ef, -1][g.below(5) as usize] as u32 } else { rm::NODE_TYPES[g.below(31) as usize].0 as u32 };
                    m.w32(at, 0x10);
                    m.w32(at + 8, ty);
                    at += 0x10;
                }
            }
            let c = g.below(5);
            m.w32(at, c);
            at += 8;
            for _ in 0..c {
                m.w32(at, g.below(80));
                at += 4;
            }
        }
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        let flag = g.below(2);
        let frame = g.next();
        let l = [3u32, 4][g.below(2) as usize];
        let what = format!("trial {trial} op {op}");
        let mut ours = Ours(World { pool, ..Default::default() }, instances);
        let (f, args): (u32, Vec<u32>) = match op {
            0 => {
                rm::save_keyframe(&mut m, &mut ours, mgr, kf, frame, flag);
                (0x18f448, vec![mgr, kf, frame, flag])
            }
            1 => {
                rm::save_node_list(&mut m, &mut ours, kf, l);
                (0x18f5d0, vec![mgr, kf, l])
            }
            2 => {
                rm::restore_keyframe(&mut m, &mut ours, mgr, kf);
                (0x18f6c0, vec![mgr, kf])
            }
            3 => {
                m.w32(kf + 8, sbuf + 24);
                r.mem.write_u32(kf + 8, sbuf + 24);
                rm::restore_nodes(&mut m, &mut ours, mgr, kf, l);
                (0x18f800, vec![mgr, kf, l])
            }
            4 => {
                m.w32(kf + 8, sbuf + 32);
                r.mem.write_u32(kf + 8, sbuf + 32);
                rm::restore_node(&mut m, &mut ours, mgr, kf, l);
                (0x18f8a8, vec![mgr, kf, l])
            }
            5 => {
                // the dead nodes' part: after the two lists
                let mut at = sbuf + 24;
                for _ in 0..2 {
                    let c = m.r32(at);
                    at += 8 + 0x10 * c;
                }
                m.w32(kf + 8, at);
                r.mem.write_u32(kf + 8, at);
                rm::load_dead_nodes(&mut m, &mut ours, kf);
                (0x190780, vec![mgr, kf])
            }
            6 => {
                rm::save_obj_refs(&mut m, &mut ours, kf);
                (0x190660, vec![mgr, kf])
            }
            _ => {
                bx::merge_pending(&mut m, &mut ours, bx::WORLD, l);
                (0x17d548, vec![bx::WORLD, l])
            }
        };
        seen[op] += 1;
        *world.borrow_mut() = World { pool, ..Default::default() };
        for (k, a) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *a as i32 as i64 as u64);
        }
        r.call(f).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        assert_eq!(world.borrow().log, ours.0.log, "{what}: calls");
        let theirs = snapshot(&mut r);
        for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                let i = i & !3;
                panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
            }
        }
    }
    eprintln!("ops {seen:?}");
}

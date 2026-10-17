//! The world's node lists and the node base classes (board row E9k) against the game in the function runner
//! (TRICKY_ELF; skipped without it): the world's set made with random lists (some sorted), fake nodes whose vtables
//! point at stand-in functions (vf3 "goes before" compares a key word, vf4 "replaces" reads a flag word, the others
//! only log), then random calls: every result, every virtual call and all the memory compared after each. For
//! `BxList_CullToCount` vf6 answers a second key word (+0x38) and vf0 with 3 takes the node out of its list (so the cull ends).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bxlist::{self as bx, Mem, NodeEnv};

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

const FN_BASE: u32 = 0x01f1_0000;
const N_VT: u32 = 3;
const N_NODES: u32 = 24;
const NODE: u32 = 0x40;
const LISTS: u32 = 6;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, u32, Vec<u32>);

/// The stand-ins' answers, from memory: vf3 (slot +0x24): this's key (+0x30) <= the other's; vf4 (+0x2c): this's +0x34
fn answer(m: &mut dyn FnMut(u32) -> u32, func: u32, this: u32, args: &[u32]) -> u32 {
    match (func - FN_BASE) % 0x100 {
        0x24 => (m(this + 0x30) <= m(args[0] + 0x30)) as u32,
        0x2c => (m(this + 0x34) != 0) as u32,
        0x3c => m(this + 0x38),
        _ => 0,
    }
}

#[derive(Default)]
struct Ours {
    log: Vec<Call>,
    /// The nodes' base when vf0 with 3 unlinks (the cull's case)
    unlink: Option<u32>,
    alloc_at: u32,
}
impl NodeEnv for Ours {
    fn virtual_call(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.log.push(Call(func, this, args.to_vec()));
        if let Some(nodes) = self.unlink {
            if (func - FN_BASE) % 0x100 == 0xc && args == [3] {
                let n = nodes + (this - nodes) / NODE * NODE;
                let (next, prev) = (m.r32(n), m.r32(n + 4));
                m.w32(prev, next);
                m.w32(next + 4, prev);
                m.w32(n, 0);
                m.w32(n + 4, 0);
            }
        }
        answer(&mut |a| m.r32(a), func, this, args)
    }
    fn alloc(&mut self, _m: &mut Mem, size: u32) -> u32 {
        self.log.push(Call(0x2ccf70, size, vec![]));
        self.alloc_at
    }
    fn free(&mut self, _m: &mut Mem, p: u32) {
        self.log.push(Call(0x14a040, p, vec![]));
    }
    fn delete(&mut self, _m: &mut Mem, p: u32) {
        self.log.push(Call(0x114f80, p, vec![]));
    }
}

#[test]
fn node_lists_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x17d4e8, 0x17d548, 0x17d6e0, 0x17d860, 0x17d888, 0x17d8b0, 0x17d8d8, 0x17d900, 0x17d928, 0x17da50, 0x17db00, 0x17de58, 0x17cfe0, 0x17cf50, 0x17cf98, 0x17d048, 0x17d090, 0x17d1d8, 0x192c10, 0x192ca0, 0x2cc808, 0x2ce114] {
        r.allow(a);
    }
    let log = Rc::new(RefCell::new(Vec::<Call>::new()));
    let alloc_at = Rc::new(RefCell::new(0u32));
    let unlink = Rc::new(std::cell::Cell::new(None::<u32>));
    // the stand-in functions: each vtable's slots
    for vt in 0..N_VT {
        for slot in (0xc..0x60).step_by(8) {
            let func = FN_BASE + vt * 0x100 + slot;
            let l = log.clone();
            let ul = unlink.clone();
            r.stub(func, move |c, m| {
                let (this, a1) = (c.gpr(4) as u32, c.gpr(5) as u32);
                if let (Some(nodes), 0xc, 3) = (ul.get(), slot, a1) {
                    let n = nodes + (this - nodes) / NODE * NODE;
                    let (next, prev) = (m.read_u32(n), m.read_u32(n + 4));
                    m.write_u32(prev, next);
                    m.write_u32(next + 4, prev);
                    m.write_u32(n, 0);
                    m.write_u32(n + 4, 0);
                }
                let args = match slot {
                    0x24 => vec![a1],
                    0xc => vec![a1],
                    _ => vec![],
                };
                l.borrow_mut().push(Call(func, this, args.clone()));
                let v = answer(&mut |a| m.read_u32(a), func, this, &args);
                c.set_gpr(2, v as u64);
            });
        }
    }
    let l = log.clone();
    let aa = alloc_at.clone();
    r.stub(0x2ccf70, move |c, _| {
        l.borrow_mut().push(Call(0x2ccf70, c.gpr(5) as u32, vec![]));
        c.set_gpr(2, *aa.borrow() as u64)
    });
    let l = log.clone();
    r.stub(0x14a040, move |c, _| l.borrow_mut().push(Call(0x14a040, c.gpr(4) as u32, vec![])));
    let l = log.clone();
    r.stub(0x114f80, move |c, _| l.borrow_mut().push(Call(0x114f80, c.gpr(4) as u32, vec![])));
    let lists = r.mem.alloc(LISTS * 0x34 + 0x10);
    let nodes = r.mem.alloc(N_NODES * NODE);
    let vts = r.mem.alloc(N_VT * 0x60);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let race = r.mem.alloc(0x40);
    let ctl = r.mem.alloc(0x80);
    let stream = r.mem.alloc(0x10);
    let sbuf = r.mem.alloc(0x200);
    let regions = [
        (0u32, 0x100u32),
        (bx::GAPP, 0x18u32),
        (lists, LISTS * 0x34 + 0x10),
        (nodes, N_NODES * NODE),
        (vts, N_VT * 0x60),
        (app, 0x800),
        (game, 0x100),
        (race, 0x40),
        (ctl, 0x80),
        (stream, 0x10),
        (sbuf, 0x200),
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
    let mut g = Rng(0x6278_6c69_7374_0e9b);
    let mut seen = [0u32; 27];
    // merges that placed a node, and that replaced one (its vf0 called)
    let (mut placed, mut replaced) = (0u32, 0u32);
    let mut culled = 0u32;
    for trial in 0..1500 {
        // a fresh set: the lists made by BxList_Init on both sides
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if *a != bx::GAPP && *a != 0 {
                v.iter_mut().for_each(|b| *b = 0);
            }
        }
        m.w32(bx::GAPP, app);
        m.w32(bx::WORLD, 0);
        m.w32(bx::WORLD + 4, 0);
        m.w32(bx::COUNTER5, g.below(100));
        m.w32(bx::COUNTER, g.below(100));
        m.w32(app + 0x730, if g.below(5) == 0 { 0 } else { game });
        m.w32(game + 0x1c, race);
        m.w32(race + 0x18, g.next());
        m.w32(game + 0x40, ctl);
        m.w32(ctl + 0xc, 0x100);
        m.w32(stream, sbuf);
        m.w32(stream + 4, sbuf);
        m.w32(stream + 8, sbuf);
        for vt in 0..N_VT {
            for slot in (0xc..0x60).step_by(8) {
                m.w16(vts + vt * 0x60 + slot - 4, [0u16, 0, 0x10][vt as usize] * (slot != 0x24 && slot != 0x2c) as u16);
                m.w32(vts + vt * 0x60 + slot, FN_BASE + vt * 0x100 + slot);
            }
        }
        for i in 0..N_NODES {
            m.w32(node(i) + 8, vts + 0x60 * g.below(N_VT));
            m.w32(node(i) + 0x30, g.below(40));
            m.w32(node(i) + 0x34, (g.below(6) == 0) as u32);
            for k in [0xc, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x38, 0x3c] {
                m.w32(node(i) + k, g.next());
            }
            m.w32(node(i) + 0x30, g.below(40));
            // (vf6's key for the cull: few values, so the least is often shared)
            m.w32(node(i) + 0x38, g.below(3));
        }
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        let n_lists = 1 + g.below(LISTS);
        let mut ours = Ours { alloc_at: lists, ..Default::default() };
        bx::init(&mut m, &mut ours, bx::WORLD, n_lists);
        *alloc_at.borrow_mut() = lists;
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, bx::WORLD as u64);
        r.cpu.set_gpr(5, n_lists as u64);
        r.call(0x17d2f0).unwrap();
        assert_eq!(*log.borrow(), ours.log, "trial {trial}: init calls");
        for l in 0..n_lists {
            if g.below(2) == 0 {
                bx::set_sorted(&mut m, bx::WORLD, l);
                r.cpu.set_gpr(4, bx::WORLD as u64);
                r.cpu.set_gpr(5, l as u64);
                r.call(0x17dae0).unwrap();
            }
        }
        for step in 0..40 {
            let op = g.below(27) as usize;
            let what = format!("trial {trial} step {step} op {op}");
            let l = g.below(n_lists);
            let l2 = g.below(n_lists);
            let free_nodes: Vec<u32> = (0..N_NODES).map(node).filter(|&n| m.r32(n) == 0 && m.r32(n + 4) == 0).collect();
            let linked: Vec<u32> = (0..N_NODES).map(node).filter(|&n| m.r32(n) != 0 && m.r32(n + 4) != 0).collect();
            let pick = |g: &mut Rng, v: &Vec<u32>| if v.is_empty() { None } else { Some(v[g.below(v.len() as u32) as usize]) };
            let fresh = pick(&mut g, &free_nodes);
            let any_linked = pick(&mut g, &linked);
            let ty = [5u32, 3, 0][g.below(3) as usize];
            let flag = [2u32, 3][g.below(2) as usize];
            let mut ours = Ours::default();
            // the node a constructor or destructor touches gets a stand-in vtable back afterwards
            let mut restore: Option<(u32, bool)> = None;
            let (f, args, v): (u32, Vec<u32>, Option<u32>) = match op {
                0 | 1 | 2 => {
                    let Some(n) = fresh else { continue };
                    bx::add(&mut m, bx::WORLD, n, l);
                    (0x17d4e8, vec![bx::WORLD, n, l], None)
                }
                3 | 4 => {
                    bx::merge_pending(&mut m, &mut ours, bx::WORLD, l);
                    (0x17d548, vec![bx::WORLD, l], None)
                }
                5 => {
                    let Some(n) = any_linked else { continue };
                    bx::unlink(&mut m, n);
                    m.w32(n, 0);
                    m.w32(n + 4, 0);
                    (0x17d710, vec![bx::WORLD, n], None)
                }
                6 => {
                    let Some(n) = any_linked else { continue };
                    bx::move_to(&mut m, bx::WORLD, n, l);
                    (0x17d740, vec![bx::WORLD, n, l], None)
                }
                7 => {
                    if l == l2 {
                        continue;
                    }
                    bx::move_all(&mut m, &mut ours, bx::WORLD, l, l2);
                    (0x17d788, vec![bx::WORLD, l, l2], None)
                }
                8 => (0x17d860, vec![bx::WORLD, l], Some(bx::first(&m, bx::WORLD, l))),
                9 => (0x17d888, vec![bx::WORLD, l], Some(bx::sorted_first(&m, bx::WORLD, l))),
                10 => (0x17d900, vec![bx::WORLD, l], Some(bx::pending_first(&m, bx::WORLD, l))),
                11 => {
                    let Some(n) = any_linked else { continue };
                    (0x17d8b0, vec![bx::WORLD, n, l], Some(bx::next_or_null(&m, bx::WORLD, n, l)))
                }
                12 => {
                    let Some(n) = any_linked else { continue };
                    (0x17d8d8, vec![bx::WORLD, n, l], Some(bx::sorted_next(&m, bx::WORLD, n, l)))
                }
                13 => {
                    let Some(n) = any_linked else { continue };
                    (0x17d928, vec![bx::WORLD, n, l], Some(bx::pending_next(&m, bx::WORLD, n, l)))
                }
                14 => {
                    bx::update_all(&mut m, &mut ours, bx::WORLD, l);
                    (0x17d950, vec![bx::WORLD, l], None)
                }
                15 => {
                    bx::draw_all(&mut m, &mut ours, bx::WORLD, l);
                    (0x17d9d8, vec![bx::WORLD, l], None)
                }
                16 => {
                    bx::delete_all(&mut m, &mut ours, bx::WORLD, l);
                    (0x17da50, vec![bx::WORLD, l], None)
                }
                17 => {
                    let Some(n) = fresh else { continue };
                    bx::node_new(&mut m, n, l);
                    restore = Some((n, false));
                    (0x17cf50, vec![n, l], None)
                }
                18 => {
                    let Some(n) = fresh else { continue };
                    bx::type_node_new(&mut m, n, l, ty);
                    restore = Some((n, false));
                    (0x17d048, vec![n, l, ty], None)
                }
                19 => {
                    let Some(n) = fresh else { continue };
                    bx::sort_node_new(&mut m, n, l, ty);
                    restore = Some((n, false));
                    (0x17d0e0, vec![n, l, ty], None)
                }
                20 => {
                    let Some(n) = any_linked else { continue };
                    bx::node_destroy(&mut m, &mut ours, n, flag);
                    restore = Some((n, true));
                    (0x17cfe0, vec![n, flag], None)
                }
                21 => {
                    let n = node(g.below(N_NODES));
                    bx::sort_node_save(&mut m, n, stream);
                    (0x17d208, vec![n, stream], None)
                }
                22 => {
                    let Some(n) = fresh else { continue };
                    m.w32(stream + 8, sbuf + 4 * g.below(0x20));
                    r.mem.write_u32(stream + 8, m.r32(stream + 8));
                    bx::sort_node_load(&mut m, n, l, stream);
                    restore = Some((n, false));
                    (0x17d178, vec![n, l, stream], None)
                }
                23 => {
                    bx::save_counters(&mut m, stream);
                    (0x17d260, vec![stream], None)
                }
                24 => {
                    m.w32(stream + 8, sbuf + 4 * g.below(0x40));
                    r.mem.write_u32(stream + 8, m.r32(stream + 8));
                    bx::load_counters(&mut m, stream);
                    (0x17d2a0, vec![stream], None)
                }
                25 => {
                    let n = g.below(5) as i32;
                    ours.unlink = Some(nodes);
                    unlink.set(Some(nodes));
                    bx::cull_to_count(&mut m, &mut ours, bx::WORLD, l, n);
                    (0x17db00, vec![bx::WORLD, l, n as u32], None)
                }
                _ => {
                    // the stream filled up: a write past the control's size marks the overflow
                    m.w32(stream + 4, sbuf + 0xf8 + 4 * g.below(3));
                    r.mem.write_u32(stream + 4, m.r32(stream + 4));
                    let n = node(g.below(N_NODES));
                    bx::type_node_save(&mut m, n, stream);
                    (0x17d1d8, vec![n, stream], None)
                }
            };
            seen[op] += 1;
            log.borrow_mut().clear();
            if op == 25 {
                culled += ours.log.iter().filter(|c| (c.0 - FN_BASE) % 0x100 == 0xc).count() as u32;
            }
            for (k, a) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + k, *a as i32 as i64 as u64);
            }
            r.call(f).unwrap_or_else(|e| panic!("{what}: {e:?}"));
            unlink.set(None);
            if let Some(v) = v {
                assert_eq!(r.cpu.gpr(2) as u32, v, "{what}: result");
            }
            assert_eq!(*log.borrow(), ours.log, "{what}: calls");
            if matches!(op, 3 | 4) {
                placed += ours.log.iter().filter(|c| (c.0 - FN_BASE) % 0x100 == 0x24).count() as u32;
                replaced += ours.log.iter().filter(|c| (c.0 - FN_BASE) % 0x100 == 0xc).count() as u32;
            }
            let theirs = snapshot(&mut r);
            for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
                if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                    let i = i & !3;
                    panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
                }
            }
            if let Some((n, unlinked)) = restore {
                let vt = vts + 0x60 * g.below(N_VT);
                m.w32(n + 8, vt);
                r.mem.write_u32(n + 8, vt);
                if unlinked {
                    for k in [0, 4] {
                        m.w32(n + k, 0);
                        r.mem.write_u32(n + k, 0);
                    }
                }
            }
        }
        // the set freed
        let mut ours = Ours::default();
        bx::free(&mut m, &mut ours, bx::WORLD);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, bx::WORLD as u64);
        r.call(0x17d440).unwrap();
        assert_eq!(*log.borrow(), ours.log, "trial {trial}: free calls");
        let theirs = snapshot(&mut r);
        assert!(theirs == m, "trial {trial}: memory after the free");
    }
    eprintln!("ops {seen:?}, {placed} order questions in merges, {replaced} nodes replaced, {culled} culled");
    assert!(culled > 500, "the culls delete nodes");
    assert!(replaced > 20);
}

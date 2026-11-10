//! `tricky_game::animseq` (board row G5a1) against the original in the function runner (TRICKY_ELF; skipped without
//! it): every case a fresh random world (three layers with random sequences linked on them and some loose, the
//! sequences' fields, their source trees and the trees' clip headers, the clip key table), then one of the 27
//! functions on it with random arguments; delete, the tree maker and the pool's alloc are logging stand-ins, memset
//! runs as the original. The world's bytes, the key table, the calls and the answer compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animseq::{self as a, SeqEnv};
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const LAYERS: u32 = ARENA;
const SEQS: u32 = ARENA + 0x200;
const NSEQ: u32 = 14;
const POOL_AT: u32 = ARENA + 0x1000;
const TREES: u32 = ARENA + 0x1800;
const NTREE: u32 = 32;
const HDRS: u32 = ARENA + 0x2000;
const NHDR: u32 = 16;
const KEYS: u32 = a::KEY_TABLE;
const NKEY: u32 = 0x584;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
    pool_next: u32,
}

fn answer(w: &mut World, f: u32, args: &[u32]) -> u32 {
    match f {
        a::SKEL_ANIM_CREATE => TREES + 0x20 * (args[1] % NTREE),
        a::POOL_ALLOC => {
            let p = POOL_AT + 0x90 * w.pool_next;
            w.pool_next += 1;
            p
        }
        _ => 0,
    }
}

struct Ours(World);
impl SeqEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        answer(&mut self.0, func, args)
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// A fresh world in `m` (arena and key table); the vtables' and the none key's words already copied in
fn make_world(g: &mut Rng, m: &mut Mem, none: u32) -> Vec<u32> {
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    // the clip headers: child count, length, two marker lists
    for h in 0..NHDR {
        let at = HDRS + 0x20 * h;
        m.w8(at + 9, g.below(3) as u8);
        let len = if g.below(10) == 0 { g.below(2) } else { 1 + g.below(40) };
        m.w16(at + 0xa, len as u16);
        for k in 0..8 {
            let v = if g.below(3) == 0 { 0xffff } else { g.below(len + 4) };
            m.w16(at + 0xc + 2 * k, v as u16);
        }
    }
    // the trees: a header, children among the later trees
    for t in 0..NTREE {
        let at = TREES + 0x20 * t;
        m.w32(at + 8, if g.below(12) == 0 { 0 } else { HDRS + 0x20 * g.below(NHDR) });
        for c in 0..2 {
            let later = NTREE - 1 - t;
            let child = if later == 0 || g.below(3) == 0 { 0 } else { TREES + 0x20 * (t + 1 + g.below(later)) };
            m.w32(at + 0xc + 4 * c, child);
        }
    }
    // the key table: none or a small key
    for k in 0..NKEY {
        let v = if g.below(3) == 0 { none } else { g.below(40) | g.below(3) << 16 };
        m.w32(KEYS + 4 * k, v);
    }
    // the sequences
    for s in 0..NSEQ {
        let at = SEQS + 0x90 * s;
        let vt = if g.below(8) == 0 { a::SKEL_VTABLE } else { a::BDR_VTABLE };
        m.w32(at + 0x34, vt);
        // (sometimes a weight and rate that land on 1 or 0 exactly)
        let (w, rate) = match g.below(6) {
            0 => (0.5, 0.5),
            1 => (0.25, -0.25),
            _ => (g.f(-0.2, 1.2), g.f(-0.3, 0.3)),
        };
        wf(m, at + 8, w);
        wf(m, at + 0xc, rate);
        let speed = match g.below(10) {
            0 => 0.0,
            1..=4 => [1.0, -1.0, 2.0, -2.0, 0.5][g.below(5) as usize],
            _ => g.f(-2.0, 2.0),
        };
        wf(m, at + 0x10, speed);
        wf(m, at + 0x14, g.f(-4.0, 4.0));
        m.w32(at + 0x20, if g.below(4) == 0 { 1 } else { 0 });
        m.w32(at + 0x24, g.below(2));
        m.w32(at + 0x40, if g.below(4) == 0 { 0x300 } else { g.below(8) });
        m.w32(at + 0x44, g.below(4));
        for i in 0..3 {
            // (often whole or half frames, as the game's ticks make them)
            let t = match g.below(4) {
                3 => 0.0,
                0 => g.below(45) as f32,
                1 => g.below(90) as f32 * 0.5 - 1.0,
                _ => g.f(-3.0, 45.0),
            };
            wf(m, at + 0x48 + 4 * i, t);
            let tree = if g.below(30) == 0 { 0 } else { TREES + 0x20 * g.below(NTREE) };
            m.w32(at + 0x78 + 4 * i, tree);
            let key = if g.below(4) == 0 { none } else { g.below(40) | g.below(3) << 16 };
            m.w32(at + 0x84 + 4 * i, key);
        }
        m.w32(at + 0x60, if g.below(2) == 0 { u32::MAX } else { g.below(45) });
        m.w32(at + 0x6c, g.below(50).wrapping_sub(5));
        wf(m, at + 0x70, g.f(-0.5, 0.0));
        m.w32(at + 0x2c, 0);
        m.w32(at + 0x30, 0);
    }
    // the layers, with random sequences on them (each on one list at most); the rest loose
    let mut loose = Vec::new();
    for l in 0..3 {
        a::list_init(m, LAYERS + 0x70 * l);
    }
    for s in 0..NSEQ {
        let at = SEQS + 0x90 * s;
        match g.below(4) {
            3 => loose.push(at),
            l => a::list_insert(m, LAYERS + 0x70 * l, at),
        }
    }
    loose
}

/// Whether every source tree a sequence's lengths are read from has a header
fn sources_ok(m: &Mem, seq: u32) -> bool {
    (0..3).all(|i| {
        let t = m.r32(seq + 0x78 + 4 * i);
        t != 0 && m.r32(t + 8) != 0
    })
}

fn linked(m: &Mem) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for l in 0..3 {
        let layer = LAYERS + 0x70 * l;
        let mut n = m.r32(layer + 0x30);
        while n != layer + 0x38 {
            out.push((layer, n));
            n = m.r32(n + 0x30);
        }
    }
    out
}

#[test]
fn sequences_and_layers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x203a40u32, 0x203ae0, 0x203b00, 0x203b48, 0x203b60, 0x203b98, 0x203bd8, 0x203c20, 0x203cb0, 0x203cf8, 0x203d38,
        0x159a80, 0x159b08, 0x159b50, 0x159ba8, 0x159bb8, 0x159d88, 0x159e38, 0x159e80, 0x159ee8, 0x159ff8, 0x15a030,
        0x15a330, 0x15a3a8, 0x15a600, 0x15a658, 0x15a690, 0x15a6c8, 0x15a700, 0x15a760, 0x2cc808,
    ] {
        r.allow(f);
    }
    let world = Rc::new(RefCell::new(World::default()));
    for (f, n) in [(a::DELETE, 1usize), (a::SKEL_ANIM_CREATE, 2), (a::POOL_ALLOC, 6)] {
        let w = world.clone();
        r.stub(f, move |c, _| {
            let regs = [4usize, 5, 6, 7, 8, 9];
            let args: Vec<u32> = regs[..n].iter().map(|&k| c.gpr(k) as u32).collect();
            let mut w = w.borrow_mut();
            w.log.push(Call(f, args.clone()));
            let v = answer(&mut w, f, &args);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    // the game's constant words: the vtables (and the pool's name) and the none key
    let mut consts = Vec::new();
    for (at, len) in [(0x379018u32, 0x38u32), (0x39e468, 0x40)] {
        let mut b = vec![0u8; len as usize];
        r.mem.read(at, &mut b).unwrap();
        consts.push((at, b));
    }
    let none = u32::from_le_bytes(consts[1].1[0..4].try_into().unwrap());
    let mut g = Rng(0x15a0_0000_0000_0001);
    let mut counts = [0usize; 28];
    for case in 0..40_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (KEYS, vec![0; 4 * NKEY as usize])] };
        m.regions.extend(consts.iter().cloned());
        let loose = make_world(&mut g, &mut m, none);
        for (at, b) in &m.regions[..2] {
            r.mem.write(*at, b).unwrap();
        }
        let links = linked(&m);
        let seq = SEQS + 0x90 * g.below(NSEQ);
        let layer = LAYERS + 0x70 * g.below(3);
        let tree = TREES + 0x20 * g.below(NTREE);
        let kind = g.below(28) as usize;
        world.borrow_mut().log.clear();
        world.borrow_mut().pool_next = 0;
        let mut ours = Ours(World::default());
        let env = &mut ours;
        // (the function, its integer arguments, its float argument) and ours run
        let mut f12 = None;
        let (f, args, mine): (u32, Vec<u32>, Option<u32>) = match kind {
            0 => {
                a::list_init(&mut m, layer);
                (0x203b48, vec![layer], None)
            }
            1 => {
                let Some(&n) = loose.get(g.below(loose.len().max(1) as u32) as usize) else { continue };
                a::list_insert(&mut m, layer, n);
                (0x203ae0, vec![layer, n], None)
            }
            2 | 3 | 4 => {
                let Some(&(l, n)) = links.get(g.below(links.len().max(1) as u32) as usize) else { continue };
                match kind {
                    2 => {
                        a::list_unlink(&mut m, n);
                        (0x203cb0, vec![n], None)
                    }
                    3 => {
                        a::list_remove(&mut m, env, l, n);
                        (0x203bd8, vec![l, n], None)
                    }
                    _ => {
                        a::list_remove_reset(&mut m, env, l, n);
                        (0x203b00, vec![l, n], None)
                    }
                }
            }
            5 => {
                let k = g.below(6);
                (0x203b60, vec![layer, k], Some(a::list_nth(&m, layer, k)))
            }
            6 => (0x203b98, vec![layer], Some(a::list_count(&m, layer))),
            7 => {
                a::list_destroy_all(&mut m, env, layer);
                (0x203c20, vec![layer], None)
            }
            8 => {
                a::skel_init(&mut m, seq);
                (0x203cf8, vec![seq], None)
            }
            9 => {
                a::skel_update(&mut m, seq);
                (0x203a40, vec![seq], None)
            }
            10 => {
                a::destroy_tree(&mut m, env, tree);
                (0x159a80, vec![tree], None)
            }
            11 => (0x15a700, vec![], Some(a::seq_create(&mut m, env))),
            12 => {
                let p = POOL_AT + 0x90 * g.below(4);
                a::seq_construct(&mut m, env, p);
                (0x159b08, vec![p], None)
            }
            13 => {
                let i = g.below(3);
                a::seq_set_tree(&mut m, seq, i, tree);
                (0x159ba8, vec![seq, i, tree], None)
            }
            14 => {
                a::seq_reset(&mut m, env, seq);
                (0x159ee8, vec![seq], None)
            }
            15 => {
                let n = loose.first().copied().unwrap_or(seq);
                a::seq_delete(&mut m, env, n);
                (0x159ff8, vec![n], None)
            }
            16 => {
                let i = g.below(3);
                let clip = if g.below(10) == 0 { g.next() % 0x2000 } else { g.below(NKEY) };
                if clip >= NKEY {
                    continue;
                }
                a::set_source(&mut m, env, seq, i, clip);
                (0x159bb8, vec![seq, i, clip], None)
            }
            17..=22 => {
                if !sources_ok(&m, seq) {
                    continue;
                }
                match kind {
                    17 => {
                        let n = g.below(14) as i32 - 3;
                        a::set_blend_out(&mut m, seq, n);
                        (0x159d88, vec![seq, n as u32], None)
                    }
                    18 => {
                        let n = g.below(11) as i32;
                        a::set_blend_in(&mut m, seq, n);
                        (0x159e38, vec![seq, n as u32], None)
                    }
                    19 => (0x159e80, vec![seq], Some(a::phase(&m, seq).0.to_bits())),
                    20 => {
                        let dt = match g.below(8) {
                            0 => g.f(-2.0, 0.0),
                            1..=4 => [1.0, 2.0, 0.5][g.below(3) as usize],
                            _ => g.f(0.0, 4.0),
                        };
                        f12 = Some(dt);
                        a::advance(&mut m, seq, F(dt));
                        (0x15a030, vec![seq], None)
                    }
                    21 => {
                        let n = g.below(11) as i32;
                        a::set_end_blend_out(&mut m, seq, n);
                        (0x15a330, vec![seq, n as u32], None)
                    }
                    _ => {
                        a::seq_update(&mut m, seq);
                        (0x15a3a8, vec![seq], None)
                    }
                }
            }
            23 => {
                if !links.iter().filter(|(l, _)| *l == layer).all(|(_, n)| sources_ok(&m, *n)) {
                    continue;
                }
                let n = g.below(14) as i32 - 3;
                a::layer_blend_out_all(&mut m, layer, n);
                (0x15a600, vec![layer, n as u32], None)
            }
            24 => {
                let v = g.f(-3.0, 3.0);
                f12 = Some(v);
                a::layer_turn(&mut m, layer, F(v));
                (0x15a658, vec![layer], None)
            }
            25 => {
                let v = g.next();
                a::layer_set_stance(&mut m, layer, v);
                (0x15a690, vec![layer, v], None)
            }
            26 => {
                let clip = if g.below(4) == 0 { 0x300 } else { g.below(8) };
                (0x15a6c8, vec![layer, clip], Some(a::layer_find(&m, layer, clip)))
            }
            _ => {
                a::layer_clear_current(&mut m, layer);
                (0x15a760, vec![layer], None)
            }
        };
        counts[kind] += 1;
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        if let Some(v) = f12 {
            r.cpu.set_f(12, v);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        if let Some(v) = mine {
            let theirs = if f == 0x159e80 { r.cpu.f(0).to_bits() } else { r.cpu.gpr(2) as u32 };
            assert_eq!(theirs, v, "case {case}: {f:#x} answer");
        }
        assert_eq!(world.borrow().log, ours.0.log, "case {case}: {f:#x} calls");
        for (at, b) in &m.regions[..2] {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let k = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + k as u32, theirs[k], b[k]);
            }
        }
    }
    println!("{counts:?}");
}

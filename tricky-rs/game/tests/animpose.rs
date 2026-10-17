//! `tricky_game::animpose` (board row G5c3b) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random world: a skeleton (bones with parents, mirrors, flags of every kind), pose
//! objects (with and without a skeleton; up to ten sources with masks, stances, weights often 0, ½ or 1), source
//! trees (bound, or with their clip's offsets and data still to bind), random curves of every kind, the root matrix,
//! and for the whole blend a controller with sequences on its layers and a rider with pose objects of two parts.
//! Then one of the seven functions; the curve and angle functions run as the original. The world, the blend's
//! scratch, the guards and the answers compared.

use r5900::Runner;
use tricky_game::animpose as p;
use tricky_game::animseq as a;
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
    fn weight(&mut self) -> f32 {
        [0.0, 1.0, 0.5, self.f(0.0, 1.0)][self.below(4) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x10000;
const SKEL: u32 = ARENA;
const POSE: u32 = ARENA + 0x800;
const TREES: u32 = ARENA + 0x1000;
const NTREE: u32 = 12;
const TREE_SIZE: u32 = 0x2c0;
const HDRS: u32 = ARENA + 0x3400;
const DATA: u32 = ARENA + 0x4000;
const OFFS: u32 = ARENA + 0x6000;
const CTL: u32 = ARENA + 0x7000;
const SEQS: u32 = ARENA + 0x7400;
const RIDER: u32 = ARENA + 0x8000;
const SCRATCH: u32 = p::W0;
const SCRATCH_LEN: u32 = 0x958 - 0x170;

fn b24(v: f32) -> [u8; 3] {
    let b = v.to_bits();
    [(b >> 8) as u8, (b >> 16) as u8, (b >> 24) as u8]
}

fn piece(g: &mut Rng, kind: u32, len: u32) -> Vec<u8> {
    let mut v = ((len << 4 | kind) as u16).to_le_bytes().to_vec();
    let fl = |g: &mut Rng, v: &mut Vec<u8>| v.extend_from_slice(&b24(g.f(-2.0, 2.0)));
    match kind {
        0 => fl(g, &mut v),
        1 => (0..2).for_each(|_| fl(g, &mut v)),
        2 => (0..3).for_each(|_| fl(g, &mut v)),
        3 => (0..4).for_each(|_| fl(g, &mut v)),
        4 => (0..len).for_each(|_| v.extend_from_slice(&g.f(-2.0, 2.0).to_le_bytes())),
        6 => {
            (0..2).for_each(|_| fl(g, &mut v));
            (0..len).for_each(|_| v.push(g.next() as u8));
        }
        7 => {
            (0..2).for_each(|_| fl(g, &mut v));
            (0..len).for_each(|_| v.extend_from_slice(&(g.next() as u16).to_le_bytes()));
        }
        _ => {}
    }
    v
}

/// A random curve's bytes
fn curve(g: &mut Rng) -> Vec<u8> {
    let kind = [0u32, 1, 2, 3, 4, 5, 6, 7][g.below(8) as usize];
    if kind != 5 {
        let len = 1 + g.below(20);
        return piece(g, kind, len);
    }
    let n = 1 + g.below(3);
    let mut v = ((n << 4 | 5) as u16).to_le_bytes().to_vec();
    for _ in 0..n {
        let k = [0u32, 1, 2, 3, 4, 6, 7][g.below(7) as usize];
        let len = 1 + g.below(12);
        v.extend(piece(g, k, len));
    }
    v
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

struct World {
    nbones: u32,
}

/// The skeleton, the curves and the trees
fn make_world(g: &mut Rng, m: &mut Mem) -> World {
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    let nbones = 1 + g.below(12);
    for b in 0..nbones {
        let at = SKEL + 0x54 * b;
        let parent: i16 = if b == 0 || g.below(4) == 0 { -1 } else { g.below(b) as i16 };
        m.w16(at + 0x10, parent as u16);
        let tr = [0u16, 1, 4, 5][g.below(4) as usize];
        let rot = [0u16, 2, 8, 0xa][g.below(4) as usize];
        m.w16(at + 0x14, tr | rot);
        m.w16(at + 0x16, g.below(nbones) as u16);
        for o in (0x18..0x54).step_by(4) {
            wf(m, at + o, g.f(-2.0, 2.0));
        }
    }
    // the curves: 160 in a row, their offsets from DATA
    let mut at = 0u32;
    for k in 0..160 {
        let c = curve(g);
        m.w32(OFFS + 4 * k, at);
        for (i, b) in c.iter().enumerate() {
            m.w8(DATA + at + i as u32, *b);
        }
        at += c.len() as u32 + g.below(3);
    }
    // the clip headers: +4 the first channel's offset index, +8 the part, +9 children
    for h in 0..16 {
        let at = HDRS + 0x24 * h;
        m.w32(at + 4, g.below(80));
        m.w8(at + 8, [0u8, 1, 0xff][g.below(3) as usize]);
        m.w8(at + 9, g.below(3) as u8);
    }
    // the trees: bound (curves pointed at) or not yet; children among the trees
    for t in 0..NTREE {
        let tr = TREES + TREE_SIZE * t;
        m.w32(tr + 8, HDRS + 0x24 * g.below(16));
        for c in 0..2 {
            m.w32(tr + 0xc + 4 * c, TREES + TREE_SIZE * g.below(NTREE));
        }
        let mut ch = 0;
        for b in 0..22 {
            m.w32(tr + 0x2c + 4 * b, ch);
            ch = (ch + 1 + g.below(6)) % 120;
        }
        for c in 0..128 {
            m.w32(tr + 0xa4 + 4 * c, DATA + m.r32(OFFS + 4 * g.below(160)));
        }
        if g.below(3) == 0 {
            m.w32(tr + 0x2a4, OFFS);
            m.w32(tr + 0x2a8, if g.below(4) == 0 { 0 } else { DATA });
        } else {
            m.w32(tr + 0x2a4, 0);
            m.w32(tr + 0x2a8, if g.below(2) == 0 { 0 } else { DATA });
        }
    }
    World { nbones }
}

/// A pose object at `at`: with or without the skeleton, `sources` of the trees
fn make_pose(g: &mut Rng, m: &mut Mem, w: &World, at: u32, skel: bool) {
    m.w32(at + 0x388, if skel { SKEL } else { 0 });
    m.w32(at + 0x38c, if skel { w.nbones } else { 1 + g.below(12) });
    m.w32(at + 0x390, g.below(6));
    m.w32(at + 0x384, [0u32, 1, u32::MAX][g.below(3) as usize]);
    wf(m, at + 0x380, [1.0, 0.5, g.f(0.0, 2.0)][g.below(3) as usize]);
    let n = g.below(11);
    m.w32(at + 0x28c, n);
    for s in 0..10 {
        m.w32(at + 0x290 + 4 * s, TREES + TREE_SIZE * g.below(NTREE));
        wf(m, at + 0x2b8 + 4 * s, if g.below(2) == 0 { g.below(20) as f32 } else { g.f(0.0, 20.0) });
        wf(m, at + 0x2e0 + 4 * s, g.weight());
        wf(m, at + 0x308 + 4 * s, g.f(-3.2, 3.2));
        m.w32(at + 0x330 + 4 * s, if g.below(3) == 0 { 1 } else { 0 });
        m.w32(at + 0x358 + 4 * s, if g.below(2) == 0 { u32::MAX } else { g.next() });
    }
}

#[test]
fn the_pose_blend_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x157ed8u32, 0x158030, 0x202520, 0x102f80, 0x202758, 0x2027f0, 0x250c10, 0x202868, 0x202930, 0x202990, 0x2029d8,
        0x159910, 0x202b00, 0x15f7c8,
    ] {
        r.allow(f);
    }
    let mut g = Rng(0x2028_0000_0000_0001);
    let mut counts = [0usize; 7];
    for case in 0..12_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; SCRATCH_LEN as usize]),
                (p::GUARDS, vec![0; 0x18]),
                (p::MATRIX, vec![0; 0x40]),
            ],
        };
        let w = make_world(&mut g, &mut m);
        for k in 0..SCRATCH_LEN / 4 {
            m.w32(SCRATCH + 4 * k, g.next());
        }
        for k in 0..6 {
            m.w32(p::GUARDS + 4 * k, g.below(2));
        }
        for rr in 0..4 {
            for c in 0..4 {
                let v = if rr == c { 1.0 + g.f(-0.2, 0.2) } else { g.f(-0.3, 0.3) };
                wf(&mut m, p::MATRIX + 0x10 * rr + 4 * c, v);
            }
        }
        let kind = g.below(7) as usize;
        let skel = g.below(4) != 0;
        let (f, args, fl): (u32, Vec<u32>, Vec<f32>);
        match kind {
            0..=4 => {
                make_pose(&mut g, &mut m, &w, POSE, skel || kind == 4);
                for (at, b) in &m.regions {
                    r.mem.write(*at, b).unwrap();
                }
                match kind {
                    0 => {
                        p::begin(&mut m, POSE);
                        (f, args, fl) = (0x202868, vec![POSE], vec![]);
                    }
                    1 => {
                        let (tree, st, mask) = (g.next(), g.next(), g.next());
                        let v = [g.f(0.0, 9.0), g.weight(), g.f(-3.0, 3.0)];
                        p::add(&mut m, POSE, tree, st, mask, F(v[0]), F(v[1]), F(v[2]));
                        (f, args, fl) = (0x202930, vec![POSE, tree, st, mask], v.to_vec());
                    }
                    2 => {
                        let tree = TREES + TREE_SIZE * g.below(NTREE);
                        let sk = if skel { SKEL } else { 0 };
                        let n = if skel { w.nbones } else { 1 + g.below(12) };
                        p::bind(&mut m, tree, sk, n as i32);
                        (f, args, fl) = (0x159910, vec![tree, sk, n], vec![]);
                    }
                    3 => {
                        p::finish(&mut m, POSE);
                        (f, args, fl) = (0x202990, vec![POSE], vec![]);
                    }
                    _ => {
                        p::blend_skeleton(&mut m, POSE);
                        (f, args, fl) = (0x202b00, vec![POSE], vec![]);
                    }
                }
            }
            5 => {
                make_pose(&mut g, &mut m, &w, POSE, false);
                for (at, b) in &m.regions {
                    r.mem.write(*at, b).unwrap();
                }
                p::eval_curves(&mut m, POSE);
                (f, args, fl) = (0x2029d8, vec![POSE], vec![]);
            }
            _ => {
                // a controller: three layers, sequences with sources of the trees; a rider with one or two pose objects
                for k in 0..3 {
                    a::list_init(&mut m, CTL + 0x28 + 0x70 * k);
                }
                m.w32(CTL + 0x24, RIDER);
                for s in 0..8 {
                    let at = SEQS + 0x90 * s;
                    wf(&mut m, at + 8, g.weight());
                    wf(&mut m, at + 0x14, g.f(-3.0, 3.0));
                    m.w32(at + 0x18, if g.below(2) == 0 { u32::MAX } else { g.next() });
                    m.w32(at + 0x44, g.below(4));
                    for i in 0..3 {
                        wf(&mut m, at + 0x48 + 4 * i, g.f(0.0, 15.0));
                        wf(&mut m, at + 0x54 + 4 * i, g.weight());
                        m.w32(at + 0x78 + 4 * i, TREES + TREE_SIZE * g.below(NTREE));
                    }
                    m.w32(at + 0x64, g.below(2));
                    a::list_insert(&mut m, CTL + 0x28 + 0x70 * g.below(3), at);
                }
                let np = 1 + g.below(2);
                m.w32(RIDER + 0x7bc, np);
                for k in 0..np {
                    let sk = g.below(3) != 0;
                    make_pose(&mut g, &mut m, &w, RIDER + 0x7c0 + 0x394 * k, sk);
                }
                for (at, b) in &m.regions {
                    r.mem.write(*at, b).unwrap();
                }
                p::blend_pose(&mut m, CTL);
                (f, args, fl) = (0x15f7c8, vec![CTL], vec![]);
            }
        }
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        for (i, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + i, *v);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b && std::env::var("POSE_DBG").is_ok() {
                for k in (0..b.len()).step_by(4) {
                    if theirs[k..k + 4] != b[k..k + 4] {
                        let t = f32::from_le_bytes(theirs[k..k + 4].try_into().unwrap());
                        let o = f32::from_le_bytes(b[k..k + 4].try_into().unwrap());
                        eprintln!("{:#x}: theirs {t} ours {o}", at + k as u32);
                    }
                }
            }
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}");
}

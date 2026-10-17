//! `tricky_game::renderdraw` (board row G7l) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random renderer (its texture state's program and first texture, the packet
//! place) and vertices, with the counts at and around the chunks' bounds. The matrix update and the submit are
//! logging stand-ins. The memory and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::renderdraw as rd;
use tricky_game::riderdraw::RiderDrawEnv;

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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2_0000;
const R: u32 = ARENA;
const TS: u32 = ARENA + 0x2000;
const VERTS: u32 = ARENA + 0x4000;
const PKT: u32 = ARENA + 0x1_0000;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// A call's log; the submit's with the texture state as it is then (its word 0 and program, and the mark), which
/// the draw's end puts back
fn logged(func: u32, w: &[u32], read: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    match func {
        rd::SUBMIT => {
            let mut v = w[..4].to_vec();
            v.extend([read(TS), read(TS + 0x10) & 0xff, read(R + 0x474)]);
            v
        }
        _ => w[..1].to_vec(),
    }
}

struct Ours(Vec<Call>);
impl RiderDrawEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        self.0.push(Call(func, logged(func, &w, &mut |a| m.r32(a))));
        0
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, _this: u32, _args: &[u32]) -> u32 {
        panic!("no virtual expected: {func:#x}");
    }
}

#[test]
fn strips_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1de7c8);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in [rd::MATRIX_UPDATE, rd::SUBMIT] {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            l.borrow_mut().push(Call(func, logged(func, &w, &mut |a| mem.read_u32(a))));
        });
    }
    let mut g = Rng(0x5791_0000_0000_0001);
    let mut chunks = 0usize;
    for case in 0..2000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(R + 0x470, TS);
        m.w32(R + 0x474, 0);
        m.w8(TS + 0x10, [0u8, 5, 5, 3][g.below(4) as usize]);
        m.w32(TS, [0u32, 0, 9][g.below(3) as usize]);
        m.w16(TS + 8, [0u16, 3, 0xffff, 0x8000][g.below(4) as usize]);
        m.w32(R + 0x680, PKT + 0x10 * g.below(16));
        let n = [0u32, 1, 2, 3, 71, 72, 73, 74, 75, 144, 145, 146, 147, 216, 217, 218, 250][g.below(17) as usize];
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        rd::strips(&mut m, &mut ours, R, n, VERTS);
        r.cpu.set_gpr(4, R as u64);
        r.cpu.set_gpr(5, n as u64);
        r.cpu.set_gpr(6, VERTS as u64);
        r.call(0x1de7c8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case} (n {n}): the calls");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} (n {n}): memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        chunks += (n as usize).saturating_sub(2).div_ceil(71).max((n > 0) as usize);
    }
    println!("{chunks} chunks");
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}
impl Rng {
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[test]
fn sprite_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1dd5c8);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in [rd::MATRIX_UPDATE, rd::SUBMIT] {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            l.borrow_mut().push(Call(func, logged(func, &w, &mut |a| mem.read_u32(a))));
        });
    }
    const PLACE: u32 = ARENA + 0x3000;
    const SIZE2: u32 = ARENA + 0x3010;
    const ST0: u32 = ARENA + 0x3020;
    const ST1: u32 = ARENA + 0x3030;
    const COLOUR: u32 = ARENA + 0x3040;
    let mut g = Rng(0x5791_1000_0000_0001);
    let mut drawn = 0usize;
    for case in 0..6000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(R + 0x470, TS);
        m.w32(R + 0x680, PKT + 0x10 * g.below(16));
        // the screen matrix: a perspective onto a 640 × 448 screen about 2048 (the offset in the depth's row, so that
        // it survives the divide), w the depth
        for i in 0..4u32 {
            for j in 0..4u32 {
                let v = match (i, j) {
                    (0, 0) => g.f(200.0, 400.0),
                    (1, 1) => -g.f(200.0, 300.0),
                    (2, 2) => g.f(-500.0, 500.0),
                    (2, 3) => 1.0,
                    (2, 0) | (2, 1) => g.f(1800.0, 2300.0),
                    (3, 2) => g.f(0.0, 1.6e7),
                    _ => if g.below(3) == 0 { g.f(-0.2, 0.2) } else { 0.0 },
                };
                wf(&mut m, R + 0x1770 + 0x10 * i + 4 * j, v);
            }
        }
        wf(&mut m, R + 0x1730, g.f(200.0, 400.0));
        wf(&mut m, R + 0x1744, g.f(200.0, 300.0));
        for (k, v) in [1728.0f32, 1824.0, 1.6e7, 9.0e6, 2368.0, 2272.0, 0.0, 4.0e6].into_iter().enumerate() {
            wf(&mut m, R + 0x17b0 + 4 * k as u32, v);
        }
        // the place: on screen, off each side, behind the camera (w below 0), near and far
        let spread = [50.0f32, 400.0, 3000.0][g.below(3) as usize];
        for k in 0..2 {
            wf(&mut m, PLACE + 4 * k, g.f(-spread, spread));
        }
        wf(&mut m, PLACE + 8, [g.f(-5.0, 300.0), -10.0, g.f(-200.0, -0.5), 1.0, 50.0, g.f(0.5, 5.0)][g.below(6) as usize]);
        wf(&mut m, PLACE + 0xc, 1.0);
        wf(&mut m, SIZE2, [g.f(0.0, 5.0), g.f(0.0, 100.0)][g.below(2) as usize]);
        wf(&mut m, SIZE2 + 4, [g.f(0.0, 5.0), g.f(0.0, 100.0)][g.below(2) as usize]);
        for k in 0..2 {
            wf(&mut m, ST0 + 4 * k, [g.f(0.0, 1.0), -0.5, 1.0][g.below(3) as usize]);
            wf(&mut m, ST1 + 4 * k, [g.f(0.0, 1.0), -0.5, 1.0][g.below(3) as usize]);
        }
        for k in 0..4 {
            wf(&mut m, COLOUR + 4 * k, [g.f(0.0, 2.0), 0.0, 1.0, -0.5][g.below(4) as usize]);
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        rd::sprite(&mut m, &mut ours, R, PLACE, SIZE2, ST0, ST1, COLOUR);
        for (k, v) in [R, PLACE, SIZE2, ST0, ST1, COLOUR].into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as u64);
        }
        r.call(0x1dd5c8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case}: the calls");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        drawn += theirs.iter().any(|c| c.0 == rd::SUBMIT) as usize;
    }
    println!("{drawn} drawn");
    assert!(drawn > 500 && drawn < 5900);
}

#[test]
fn program9_data_and_skin_blend_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1dfe30);
    r.allow(0x1e7ac0);
    const MATS: u32 = ARENA + 0x8000;
    const TABLE: u32 = ARENA + 0x9800;
    const BONES: u32 = ARENA + 0x9900;
    const ENTRIES: u32 = ARENA + 0x9c00;
    const OUT: u32 = ARENA + 0xa000;
    const MAT: u32 = ARENA + 0x3000;
    let mut g = Rng(0x5791_2000_0000_0001);
    let mut blended = 0usize;
    for case in 0..4000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(R + 0x470, TS);
        m.w32(R + 0x680, PKT + 0x10 * g.below(16));
        m.w8(TS + 0x10, [0u8, 9, 9, 5][g.below(4) as usize]);
        if g.below(3) == 0 {
            let p = m.r32(R + 0x680);
            m.w32(TS, p);
        }
        m.w32(R + 0x3f8, OUT + 0x800);
        for k in 0..0x1800 / 4 {
            wf(&mut m, MATS + 4 * k, g.f(-50.0, 50.0));
        }
        for k in 0..8 {
            m.w32(TABLE + 4 * k, g.below(20));
        }
        let n = [0u32, 1, 2, 5, 12, 0xffff_ffff][g.below(6) as usize];
        for b in 0..12u32 {
            m.w32(BONES + 12 * b, [1u32, 1, 2, 3, 4, 0][g.below(6) as usize]);
            m.w32(BONES + 12 * b + 4, ENTRIES + 0x10 * b);
            for e in 0..4u32 {
                let at = ENTRIES + 0x10 * b + 4 * e;
                let w = g.below(101) as u16;
                m.w16(at, [100u16, 50, 25, 0, 33, 0xffce, w, w][g.below(8) as usize]);
                m.w8(at + 2, g.below(4) as u8);
                m.w8(at + 3, g.below(8) as u8);
            }
        }
        let out = if g.below(2) == 0 { 0 } else { OUT };
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        let prog9 = case % 2 == 0;
        if prog9 {
            rd::program9_data(&mut m, R, MAT, MAT + 0x40, MAT + 0x50);
            for (k, v) in [R, 0x19, 0, MAT, MAT + 0x40, MAT + 0x50].into_iter().enumerate() {
                r.cpu.set_gpr(4 + k, v as u64);
            }
            r.call(0x1dfe30).unwrap_or_else(|e| panic!("case {case}: {e}"));
        } else {
            rd::skin_blend(&mut m, R, MATS, TABLE, BONES, n, out);
            for (k, v) in [R, MATS, TABLE, BONES, n, out].into_iter().enumerate() {
                r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
            }
            r.call(0x1e7ac0).unwrap_or_else(|e| panic!("case {case}: {e}"));
            blended += (n as i32).max(0) as usize;
        }
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} ({}): memory at {:#x}: theirs {:#x} ours {:#x}", if prog9 { "program 9" } else { "blend" }, at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{blended} bones blended");
    assert!(blended > 5000);
}

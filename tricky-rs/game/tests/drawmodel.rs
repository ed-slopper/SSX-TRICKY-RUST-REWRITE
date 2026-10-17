//! `tricky_game::drawmodel` (board row G7b2) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random renderer (the frame's matrices made or not, the clip matrix, the fog's
//! scale, the texture cache, a packet buffer), model (bones with meshes of batches, bind poses, the world's matrix
//! as one, its scale), part with textures and their flags, matrix, light block, bones given or not, UV scroll or
//! not; then `DrawModel`. The renderer's virtuals, its matrix update, the submit and the frame's two calls are
//! logging stand-ins. The memory (bar the port's scratch: the game's stack), the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::drawmodel::{self as d, DrawEnv};
use tricky_game::pf::Mem;

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
const SIZE: u32 = 0x8000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x1800;
const CAM: u32 = ARENA + 0x1c00;
const TEXSTATE: u32 = ARENA + 0x1c80;
const MODEL: u32 = ARENA + 0x1d00;
const TABLE: u32 = ARENA + 0x1d80;
const MESHES: u32 = ARENA + 0x1f00;
const BATCHES: u32 = ARENA + 0x2000;
const PART: u32 = ARENA + 0x2200;
const TEXES: u32 = ARENA + 0x2300;
const MATRIX: u32 = ARENA + 0x2500;
const LIGHTS: u32 = ARENA + 0x2600;
const BONES: u32 = ARENA + 0x2700;
const POSES: u32 = ARENA + 0x2a00;
const SCROLL: u32 = ARENA + 0x2c00;
const SCROLLPKT: u32 = ARENA + 0x2d00;
const PKT: u32 = ARENA + 0x3000;
const SCRATCH: u32 = 0x0180_0000;
const SLOTS: [u32; 3] = [0x1e0, 0x1e8, 0x1f0];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>);
impl CullEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
}
impl DrawEnv for Ours {
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) {
        self.0.push(Call(func, std::iter::once(this).chain(args.iter().copied()).collect()));
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn draw_model_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1e1f70);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for (f, n) in [(0x1c4650u32, 1), (0x1c4c38, 2), (d::MATRIX_UPDATE, 1), (d::SUBMIT, 4)] {
        let l = log.clone();
        r.stub(f, move |c, _| l.borrow_mut().push(Call(f, (0..n).map(|k| c.gpr(4 + k) as u32).collect())));
    }
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, _| {
            let n = if off == 0x1e0 { 1 } else { 2 };
            l.borrow_mut().push(Call(vaddr(off), (0..=n).map(|k| c.gpr(4 + k) as u32).collect()));
        });
    }
    let mut g = Rng(0x0d3a_0000_0000_0001);
    let (mut packets, mut fogged) = (0usize, 0usize);
    for case in 0..20_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (d::WORLD_MATRIX, vec![0; 0x40]), (SCRATCH, vec![0; 0x720])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..0x720 / 4 {
            m.w32(SCRATCH + 4 * k, g.next());
        }
        for k in 0..16 {
            wf(&mut m, d::WORLD_MATRIX + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
        }
        // the renderer: the frame's matrices (made or not), the fog, the texture cache, the packet buffer
        m.w32(RENDER + 0x10, RVT);
        for off in SLOTS {
            m.w16(RVT + off, [0u16, 0x10][g.below(2) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(RENDER + 0x17d0, g.below(2));
        m.w32(RENDER + 0x3f4, CAM);
        for base in [0x1730u32, 0x4d0, 0x570, 0x5b0] {
            for k in 0..16 {
                wf(&mut m, RENDER + base + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else if k >= 12 { g.f(-500.0, 500.0) } else { g.f(-0.5, 0.5) });
            }
        }
        for k in 0..16 {
            wf(&mut m, CAM + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else { g.f(-0.5, 0.5) });
        }
        wf(&mut m, RENDER + 0x630, [g.f(0.0, 300.0), g.f(0.0, 300.0), 2.0e5][g.below(3) as usize]);
        wf(&mut m, RENDER + 0x63c, [g.f(0.0, 3.0), 0.5, -1.0, 400.0][g.below(4) as usize]);
        m.w32(RENDER + 0x640, g.below(2));
        m.w32(RENDER + 0x680, PKT);
        m.w32(RENDER + 0x470, TEXSTATE);
        m.w8(TEXSTATE + 0x10, [5u8, 5, 3][g.below(3) as usize]);
        m.w32(TEXSTATE, [0u32, 0, 7][g.below(3) as usize]);
        // the model: its bones (parents first; some posed at the world's matrix), meshes of batches, its scale
        m.w32(MODEL + 4, [0u32, 1, 1, 2, 4, 8, u32::MAX][g.below(7) as usize]);
        m.w32(MODEL + 8, TABLE);
        for k in 0..3 {
            wf(&mut m, MODEL + 0x18 + 4 * k, [1.0, g.f(0.2, 3.0)][g.below(2) as usize]);
        }
        for b in 0..8u32 {
            let rec = TABLE + 0x18 * b;
            m.w32(rec, if b == 0 || g.below(3) == 0 { u32::MAX } else { g.below(b) });
            m.w32(rec + 4, if g.below(5) == 0 { 0 } else { MESHES + 0x30 * g.below(4) });
            m.w32(rec + 0x14, if g.below(3) == 0 { d::WORLD_MATRIX } else { POSES + 0x40 * g.below(8) });
        }
        for k in 0..8 * 16 {
            wf(&mut m, POSES + 4 * k, g.f(-1.5, 1.5));
            wf(&mut m, BONES + 4 * k, g.f(-1.5, 1.5));
        }
        for k in 0..4 {
            let mesh = MESHES + 0x30 * k;
            m.w32(mesh + 0x20, [0u32, 1, 2, 3][g.below(4) as usize]);
            m.w32(mesh + 0x28, mesh + 0x10);
            for j in 0..3 {
                m.w32(mesh + 0x10 + 4 * j, BATCHES + 0x20 * g.below(6));
            }
        }
        for k in 0..6 {
            let b = BATCHES + 0x20 * k;
            m.w32(b + 4, g.below(4));
            m.w32(b + 0x10, SCROLLPKT + 0x10 * k);
        }
        for k in 0..4 {
            let t = TEXES + 0x50 * k;
            m.w32(PART + 4 + 4 * k, t);
            m.w32(t + 0x40, [0u32, 0x4_0000, 0x2_0000, 0x6_0000][g.below(4) as usize] | (g.next() & 0xffff));
        }
        for k in 0..16 {
            wf(&mut m, MATRIX + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else if k >= 12 { g.f(-500.0, 500.0) } else { g.f(-0.5, 0.5) });
        }
        wf(&mut m, SCROLL, g.f(-1.0, 1.0));
        wf(&mut m, SCROLL + 4, g.f(-1.0, 1.0));
        for (at, b) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, b).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let bones = if g.below(3) == 0 { BONES } else { 0 };
        let scroll = if g.below(3) == 0 { SCROLL } else { 0 };
        let t2 = scroll;
        d::draw_model(&mut m, &mut ours, RENDER, MODEL, PART, MATRIX, LIGHTS, bones, scroll, SCRATCH);
        for (i, v) in [RENDER, MODEL, PART, MATRIX, LIGHTS, bones, t2].into_iter().enumerate() {
            r.cpu.set_gpr(4 + i, v as u64);
        }
        r.call(0x1e1f70).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.0 {
            let i = theirs.iter().zip(&ours.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.0.len()));
            panic!("case {case}: call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.0.get(i));
        }
        packets += ours.0.iter().filter(|c| c.0 == d::SUBMIT).count();
        fogged += ours.0.iter().filter(|c| c.0 == vaddr(0x1e8) && c.1[1] == 2 && c.1[2] != 1).count();
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{packets} packets, {fogged} fogged blends");
    assert!(packets > 10_000 && fogged > 1000);
}

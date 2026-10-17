//! `tricky_game::cull` (board row G7c4) against the original in the function runner (TRICKY_ELF; skipped without it):
//! every case a renderer with random matrices (the camera's, +0x1730, +0x4d0, +0x570, and the clip matrix +0x5b0
//! when the frame's matrices are made already) and a random box near the view's edges; then `CullBox`, its VU0
//! micro program run by the runner's micro mode from the microcode the game uploads (the chain at 0x31cac0). The
//! two calls of the matrix update are logging stand-ins. The renderer's memory, the calls and the answer compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::{self as c, CullEnv};
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
const RENDER: u32 = ARENA;
const CAM: u32 = ARENA + 0x2000;
const BOX: u32 = ARENA + 0x2100;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>);
impl CullEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn cull_box_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    assert_eq!(r.load_vu0_chain(0x31cac0).unwrap(), 215, "the game's VU0 upload: 215 instructions");
    r.allow(0x1e1b10);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let l = log.clone();
    r.stub(c::VIEW_UPDATE, move |c, _| l.borrow_mut().push(Call(c::VIEW_UPDATE, vec![c.gpr(4) as u32])));
    let l = log.clone();
    r.stub(c::VIEW_PLANES, move |c, _| l.borrow_mut().push(Call(c::VIEW_PLANES, vec![c.gpr(4) as u32, c.gpr(5) as u32])));
    let mut g = Rng(0xc011_0000_0000_0001);
    let mut answers = [0usize; 3];
    let mut made = 0;
    for case in 0..30_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; 0x2200])] };
        for k in 0..0x2200 / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        let fresh = g.below(3) == 0;
        m.w32(RENDER + 0x17d0, if fresh { 0 } else { 1 + g.below(3) });
        m.w32(RENDER + 0x3f4, CAM);
        // the matrices: near the identity (the camera's with a position), so that boxes land about the view's edges
        let mats: &[u32] = if fresh { &[0x1730, 0x4d0, 0x570] } else { &[0x5b0] };
        for &base in mats.iter().chain([CAM - RENDER].iter()) {
            for row in 0..4 {
                for col in 0..4 {
                    let v = if row == col { g.f(0.7, 1.3) } else if row == 3 && base == CAM - RENDER { g.f(-1.0, 1.0) } else { g.f(-0.3, 0.3) };
                    wf(&mut m, RENDER + base + 0x10 * row + 4 * col, v);
                }
            }
        }
        // now and then the camera's matrix with a −0 (the copy keeps it) and, with the frame's matrices made, an
        // exact identity clip matrix with corners on the planes (x = ±w exactly)
        if g.below(4) == 0 {
            wf(&mut m, CAM + 4 * g.below(16), -0.0);
        }
        let exact = !fresh && g.below(4) == 0;
        if exact {
            for k in 0..16 {
                wf(&mut m, RENDER + 0x5b0 + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
            }
        }
        // the box: a centre about the view's edge, half sizes from nothing to wide; w (the corners' w) about 1
        for k in 0..3 {
            let (c0, e) = (g.f(-2.0, 2.0), [0.0, g.f(0.0, 0.3), g.f(0.0, 3.0)][g.below(3) as usize]);
            wf(&mut m, BOX + 4 * k, c0 - e);
            wf(&mut m, BOX + 0x10 + 4 * k, c0 + e);
        }
        wf(&mut m, BOX + 0xc, [1.0, g.f(0.5, 1.5), g.f(-1.5, -0.5)][g.below(3) as usize]);
        wf(&mut m, BOX + 0x1c, g.f(-2.0, 2.0));
        if exact {
            for k in 0..3 {
                let v = [[-1.0f32, 1.0], [-2.0, -1.0], [1.0, 2.0], [-0.5, 0.5], [-1.0, -1.0]][g.below(5) as usize];
                wf(&mut m, BOX + 4 * k, v[0]);
                wf(&mut m, BOX + 0x10 + 4 * k, v[1]);
            }
            wf(&mut m, BOX + 0xc, [1.0f32, -1.0][g.below(2) as usize]);
        }
        r.mem.write(ARENA, &m.regions[0].1).unwrap();
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let ans = c::cull_box(&mut m, &mut ours, RENDER, BOX, BOX + 0x10);
        for (i, v) in [RENDER, BOX, BOX + 0x10].iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as u64);
        }
        r.call(0x1e1b10).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32, ans, "case {case}: answer");
        assert_eq!(*log.borrow(), ours.0, "case {case}: calls");
        let mut theirs = vec![0u8; 0x2200];
        r.mem.read(ARENA, &mut theirs).unwrap();
        if theirs != m.regions[0].1 {
            let i = theirs.iter().zip(&m.regions[0].1).position(|(x, y)| x != y).unwrap();
            panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", ARENA + i as u32, theirs[i], m.regions[0].1[i]);
        }
        answers[ans as usize] += 1;
        made += fresh as usize;
    }
    println!("answers {answers:?}, matrices made {made}");
    assert!(answers.iter().all(|&n| n > 2000));
}

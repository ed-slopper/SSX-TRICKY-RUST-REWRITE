//! `tricky_game::marker` (board row G7g5) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random marker (its points, its height) under a random view (the marker
//! behind, near, between and past the far depth) and renderer (its vtable's this-shifts). The renderer's
//! virtuals are logging stand-ins (+0xf8 answers the view; +0x220 logs the corners it is handed). The memory and
//! the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::particledraw::{ParticleDrawEnv, APP};
use tricky_game::pf::Mem;
use tricky_game::marker as mk;

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
const SIZE: u32 = 0x2000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x800;
const TEX: u32 = ARENA + 0xa00;
const MARKER: u32 = ARENA + 0x1000;
const VIEW: u32 = ARENA + 0x1800;
const SLOTS: [u32; 7] = [0x1c0, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x220, 0xf8];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn logged(off: u32, w: &[u32], f12: u32, read: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    let mut v = vec![w[0]];
    match off {
        0x1c8 => v.extend([w[1], w[2], f12]),
        0x1c0 | 0x1e0 => v.push(w[1]),
        0xf8 => {}
        0x220 => {
            v.extend([w[1], w[3]]);
            for k in 0..w[1].min(20) {
                for j in (0x20..0x30).step_by(4) {
                    v.push(read(w[2] + 0x30 * k + j));
                }
            }
        }
        _ => v.extend([w[1], w[2]]),
    }
    v
}

struct Ours(Vec<Call>);
impl CullEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
}
impl ParticleDrawEnv for Ours {
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let off = func - vaddr(0);
        let mut w = vec![this];
        w.extend(args);
        let f12 = if off == 0x1c8 { args[2] } else { 0 };
        w.resize(6, 0);
        let mut read = |a: u32| m.r32(a);
        let l = logged(off, &w, f12, &mut read);
        self.0.push(Call(func, l));
        if off == 0xf8 { VIEW } else { 0 }
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn marker_draw_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x133c20);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, mem| {
            let w: Vec<u32> = (4..10).map(|k| c.gpr(k) as u32).collect();
            let mut read = |a: u32| mem.read_u32(a);
            let lg = logged(off, &w, c.fpr[12], &mut read);
            l.borrow_mut().push(Call(vaddr(off), lg));
            if off == 0xf8 {
                c.set_gpr(2, VIEW as u64);
            }
        });
    }
    let mut g = Rng(0x3a4c_0000_0000_0001);
    let mut sizes = [0usize; 3];
    for case in 0..5000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(RENDER + 0x10, RVT);
        for off in SLOTS {
            m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x30, TEX);
        // the view: a camera's rows, its depth's row picked so that the marker comes behind, near, between and far
        for i in 0..4 {
            for j in 0..4 {
                wf(&mut m, VIEW + 0x10 * i + 4 * j, if i == j { 1.0 } else { [0.0, -0.0, g.f(-0.5, 0.5)][g.below(3) as usize] });
            }
        }
        let depth = [-100.0f32, 0.0, 500.0, 20000.0, 40000.0, g.f(-1000.0, 35000.0)][g.below(6) as usize];
        // the points (some lanes ±0, as the corners' sums then show the zeros' signs)
        for k in 0..16 {
            wf(&mut m, MARKER + 4 * k, [g.f(-300.0, 300.0), g.f(-300.0, 300.0), 0.0, -0.0][g.below(4) as usize]);
        }
        wf(&mut m, MARKER + 0x28, depth);
        wf(&mut m, MARKER + 0x2c, 1.0);
        wf(&mut m, MARKER + 0x24c, [0.0f32, 30.0, g.f(-50.0, 50.0)][g.below(3) as usize]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        mk::draw(&mut m, &mut ours, MARKER);
        r.cpu.set_gpr(4, MARKER as u64);
        r.call(0x133c20).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.0 {
            let i = theirs.iter().zip(&ours.0).position(|(x, y)| x != y).unwrap_or(theirs.len().min(ours.0.len()));
            panic!("case {case}: call {i}: theirs {:x?} ours {:x?}", theirs.get(i), ours.0.get(i));
        }
        let z = f32::from_bits(m.r32(MARKER + 0x28));
        sizes[if z <= 0.0 { 0 } else if z >= 31128.54 { 2 } else { 1 }] += 1;
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            assert!(&th == b, "case {case}: memory at {at:#x} differs");
        }
    }
    println!("depths {sizes:?}");
    assert!(sizes.iter().all(|&n| n > 300));
}

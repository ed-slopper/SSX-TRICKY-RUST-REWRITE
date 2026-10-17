//! `tricky_game::quad2d` (board row G7m) against the original in the function runner (TRICKY_ELF; skipped
//! without it): each of the six 2D quad helpers on random colours (0, 1, and between, one or a corner each, some
//! the same), rectangles (some of no width or height, some off the screen) and texture rectangles. The
//! renderer's +0x220 is a logging stand-in reading the vertices it is handed (bar the word the game leaves
//! unwritten). The memory (bar the port's scratch: the game's stack) and the calls are compared. (The harness is
//! `tests/fxdraw.rs`'s.)

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::quad2d as q;
use tricky_game::particledraw::{ParticleDrawEnv, APP};
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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x1800;
const APPOBJ: u32 = ARENA + 0x1c00;
const TEXTABLE: u32 = ARENA + 0x1d00;
const COLOUR: u32 = ARENA + 0x1e00;
const SCRATCH: u32 = 0x0180_0000;
const SLOTS: [u32; 11] = [0x1c0, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0x220, 0x238, 0x240, 0x258, 0x280];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What a renderer virtual is logged with, from its registers' words and the memory they point at
fn logged(off: u32, w: &[u32], f12: u32, rd: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    let mut out = vec![w[0]];
    match off {
        0x1c8 => out.extend([w[1], w[2], f12]),
        0x1c0 | 0x1e0 | 0x258 => out.push(w[1]),
        0x220 | 0x238 => {
            out.extend([w[1], w[3]]);
            for k in 0..w[1].min(64) {
                for j in (0..0x30).step_by(4) {
                    if j != 0xc {
                        out.push(rd(w[2] + 0x30 * k + j));
                    }
                }
            }
        }
        0x240 => {
            for k in 0..4 {
                out.push(rd(w[1] + 4 * k));
            }
            for k in 0..4 {
                out.push(rd(w[2] + 4 * k));
            }
            out.extend([w[3], w[4], w[5]]);
        }
        0x280 => {
            for k in 0..8 {
                out.push(rd(w[1 + k as usize / 4] + 4 * (k % 4)));
            }
        }
        _ => out.extend([w[1], w[2]]),
    }
    out
}
/// The box test's answer: from the box's first word, so 0, 1 and 2 all come
fn answer(log: &[u32]) -> u32 {
    (log[1] >> 9) % 3
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
        w.extend(args.iter().copied());
        w.resize(6, 0);
        let f12 = if off == 0x1c8 { args[2] } else { 0 };
        if off == 0x1c8 {
            w[3] = 0;
        }
        let mut rd = |a: u32| m.r32(a);
        let l = logged(off, &w, f12, &mut rd);
        let a = if off == 0x280 { answer(&l) } else { 0 };
        self.0.push(Call(func, l));
        a
    }
}

fn runner(log: &Rc<RefCell<Vec<Call>>>) -> Option<Runner> {
    let mut r = Runner::from_env()?;
    for f in [0x132308u32, 0x12fb40, 0x12d890, 0x12d6c0, 0x19b8c8, 0x19bb48, 0x102f50, 0x250d60, 0x250e98] {
        r.allow(f);
    }
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, mem| {
            let w: Vec<u32> = [4, 5, 6, 7, 8, 9].map(|k| c.gpr(k) as u32).to_vec();
            let mut rd = |a: u32| mem.read_u32(a);
            let lg = logged(off, &w, c.fpr[12], &mut rd);
            let a = if off == 0x280 { answer(&lg) } else { 0 };
            l.borrow_mut().push(Call(vaddr(off), lg));
            c.set_gpr(2, a as u64);
        });
    }
    Some(r)
}

/// A fresh arena: random words, the renderer's vtable, the app's renderer and texture table
fn fresh(g: &mut Rng, scratch: u32) -> Mem {
    let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4]), (SCRATCH, vec![0; scratch as usize])] };
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    for k in 0..scratch / 4 {
        m.w32(SCRATCH + 4 * k, g.next());
    }
    m.w32(RENDER + 0x10, RVT);
    for off in SLOTS {
        m.w16(RVT + off, g.pick(&[0u16, 0x10, 0xfff0]));
        m.w32(RVT + off + 4, vaddr(off));
    }
    m.w32(APP, APPOBJ);
    m.w32(APPOBJ + 0x724, RENDER);
    m.w32(APPOBJ + 0x30, TEXTABLE);
    m
}
fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// One case: the port and the game, the calls and the memory (bar the scratch) compared
fn run(r: &mut Runner, log: &Rc<RefCell<Vec<Call>>>, m: &mut Mem, case: u32, port: &mut dyn FnMut(&mut Mem, &mut Ours), func: u32, regs: &[(usize, u64)], fregs: &[(usize, f32)]) -> Vec<Call> {
    for (at, b) in &m.regions {
        if *at != SCRATCH {
            r.mem.write(*at, b).unwrap();
        }
    }
    log.borrow_mut().clear();
    let mut ours = Ours(Vec::new());
    port(m, &mut ours);
    for &(k, v) in regs {
        r.cpu.set_gpr(k, v);
    }
    for &(k, v) in fregs {
        r.cpu.set_f(k, v);
    }
    r.call(func).unwrap_or_else(|e| panic!("case {case}: {e}"));
    let theirs = log.borrow().clone();
    if theirs != ours.0 {
        let i = theirs.iter().zip(&ours.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.0.len()));
        let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(24)])).unwrap_or_default();
        panic!("case {case}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), ours.0.len(), show(theirs.get(i)), show(ours.0.get(i)));
    }
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
    theirs
}

#[test]
fn quads_match_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    for f in [0x19b108u32, 0x19b2a0, 0x19b4d8, 0x19b730, 0x19bdf8, 0x19bf90] {
        r.allow(f);
    }
    let mut g = Rng(0x9ad2_0000_0000_0001);
    let mut seen = [0usize; 6];
    for case in 0..6000u32 {
        let mut m = fresh(&mut g, 0xc0);
        // five colours of 0x14 bytes (the first's +0x10 the depth)
        let cols: [u32; 5] = [0, 1, 2, 3, 4].map(|k| COLOUR + 0x20 * k);
        for &c in &cols {
            for k in 0..4 {
                let v = g.f(0.0, 2.0);
                wf(&mut m, c + 4 * k, g.pick(&[0.0, 1.0, 0.5, v, v]));
            }
            wf(&mut m, c + 0x10, g.f(-1.0, 1.0));
        }
        let mut corner = [cols[1], cols[2], cols[3], cols[4]];
        if g.below(4) == 0 {
            corner[g.below(4) as usize] = cols[0];
        }
        let size = |g: &mut Rng| { let v = g.f(0.0, 300.0); g.pick(&[0.0f32, 1.0, v, v]) };
        let (x, y) = (g.f(-100.0, 700.0), g.f(-100.0, 550.0));
        let (w, h) = (size(&mut g), size(&mut g));
        let t: [f32; 4] = [0, 1, 2, 3].map(|_| { let v = g.f(0.0, 1.0); g.pick(&[0.0f32, 1.0, v, v]) });
        let which = (case % 6) as usize;
        seen[which] += 1;
        let (func, a0) = [(0x19b108u32, cols[0]), (0x19b2a0, cols[0]), (0x19b4d8, cols[0]), (0x19b730, cols[0]), (0x19bdf8, cols[0]), (0x19bf90, cols[0])][which];
        let regs = [(4usize, a0 as u64), (5, corner[0] as u64), (6, corner[1] as u64), (7, corner[2] as u64), (8, corner[3] as u64)];
        let fregs = [(12usize, x), (13, y), (14, w), (15, h), (16, t[0]), (17, t[1]), (18, t[2]), (19, t[3])];
        let (fx, fy, fw, fh, ft) = (F(x), F(y), F(w), F(h), t.map(F));
        run(&mut r, &log, &mut m, case, &mut |m, e| match which {
            0 => q::filled_rect(m, e, a0, fx, fy, fw, fh, SCRATCH),
            1 => q::gradient_rect(m, e, a0, corner, fx, fy, fw, fh, SCRATCH),
            2 => q::gradient_quad(m, e, a0, corner, fx, fy, fw, fh, ft, SCRATCH),
            3 => q::textured_rect(m, e, a0, fx, fy, fw, fh, ft, SCRATCH),
            4 => q::submit_quad(m, e, a0, fx, fy, fw, fh, ft, SCRATCH),
            _ => q::gradient_quad_turned(m, e, a0, corner, fx, fy, fw, fh, ft, SCRATCH),
        }, func, &regs, &fregs);
    }
    assert!(seen.iter().all(|&n| n == 1000));
}

#[test]
fn lines_match_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    for f in [0x19c1e8u32, 0x19c310, 0x19c458, 0x19c620] {
        r.allow(f);
    }
    let mut g = Rng(0x9ad2_1000_0000_0001);
    let mut seen = [0usize; 4];
    for case in 0..4000u32 {
        let mut m = fresh(&mut g, 0xf0);
        let cols: [u32; 3] = [0, 1, 2].map(|k| COLOUR + 0x20 * k);
        for &c in &cols {
            for k in 0..4 {
                let v = g.f(0.0, 2.0);
                wf(&mut m, c + 4 * k, g.pick(&[0.0, 1.0, 0.5, v, v]));
            }
            wf(&mut m, c + 0x10, g.f(-1.0, 1.0));
        }
        let p: [f32; 6] = [0, 1, 2, 3, 4, 5].map(|_| { let v = g.f(-100.0, 700.0); g.pick(&[0.0f32, v, v]) });
        let fp = p.map(F);
        let which = (case % 4) as usize;
        seen[which] += 1;
        let fregs: Vec<(usize, f32)> = (0..6).map(|k| (12 + k, p[k])).collect();
        let (func, regs) = match which {
            0 => (0x19c1e8u32, vec![(4usize, cols[0] as u64)]),
            1 => (0x19c310, vec![(4, cols[0] as u64), (5, cols[1] as u64), (6, cols[2] as u64)]),
            2 => (0x19c458, vec![(4, cols[0] as u64)]),
            _ => (0x19c620, vec![(4, cols[0] as u64)]),
        };
        run(&mut r, &log, &mut m, case, &mut |m, e| match which {
            0 => q::line(m, e, cols[0], fp[0], fp[1], fp[2], fp[3], SCRATCH),
            1 => q::gradient_line(m, e, cols[0], cols[1], cols[2], fp[0], fp[1], fp[2], fp[3], SCRATCH),
            2 => q::rect_outline(m, e, cols[0], fp[0], fp[1], fp[2], fp[3], SCRATCH),
            _ => q::triangle(m, e, cols[0], [(fp[0], fp[1]), (fp[2], fp[3]), (fp[4], fp[5])], SCRATCH),
        }, func, &regs, &fregs);
    }
    assert!(seen.iter().all(|&n| n == 1000));
}

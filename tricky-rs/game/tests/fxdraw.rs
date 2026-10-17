//! `tricky_game::fxdraw` (board row G7e4) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the ground spray's, the speed dust's and the spray's draws, and the two turned quads. Every case
//! is a fresh random effect (rings with counts and oldest places past their ends and negative, entries live or
//! not, ages around their lives and fades, points from 0 to 25) under a random renderer (its vtable's
//! this-shifts) and app. The renderer's virtuals are logging stand-ins. They log what the draw hands them,
//! reading the vertices (bar the word the game leaves unwritten), places and boxes from the pointers; the box
//! test answers from the box. `Vec4_Scale`, `Math_Sin`, `Math_Cos` and `Fx_Spray_ComputeParticle` run as the
//! game's. The memory (bar the port's scratch: the game's stack) and the calls are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::fxdraw as fx;
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
const FX: u32 = ARENA + 0x3000;
const SCRATCH: u32 = 0x0180_0000;
const SLOTS: [u32; 10] = [0x1c0, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0x220, 0x240, 0x258, 0x280];
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
        0x220 => {
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
fn surface_spray_draw_matches_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    let mut g = Rng(0x0e74_0000_0000_0001);
    let mut draws = 0usize;
    for case in 0..3000u32 {
        let mut m = fresh(&mut g, 0x10);
        m.w32(FX, g.pick(&[-2i32, 0, 1, 3, 12, 40, 45]) as u32);
        m.w32(FX + 4, g.pick(&[0i32, 5, 39, 41, -3, 77]) as u32);
        for k in 0..40 {
            if g.below(3) == 0 {
                m.w32(FX + 0x40 + 0x150 * k, 0);
            }
        }
        let calls = run(&mut r, &log, &mut m, case, &mut |m, e| fx::surface_spray_draw(m, e, FX), 0x132308, &[(4, FX as u64)], &[]);
        draws += calls.iter().filter(|c| c.0 == vaddr(0x258)).count();
    }
    println!("{draws} sprays drawn");
    assert!(draws > 10_000);
}

#[test]
fn speed_dust_draw_matches_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    let mut g = Rng(0x0e74_1000_0000_0001);
    let (mut drawn, mut fading_in) = (0usize, 0usize);
    for case in 0..3000u32 {
        let mut m = fresh(&mut g, 0x20);
        m.w32(FX + 0xd50, g.pick(&[-1i32, 0, 1, 5, 30, 64, 70]) as u32);
        m.w32(FX + 0xd54, g.pick(&[0i32, 7, 63, 65, -5]) as u32);
        // (and the five before the ring: the oldest place −5 reaches them. The last two lie over the dust's frame,
        // so that is written after them)
        for k in -5i32..64 {
            let p = (FX + 0x50).wrapping_add((0x34 * k) as u32);
            let any = g.f(0.3, 3.0);
            let life = g.pick(&[0.5f32, 1.0, 2.0, any]);
            let age = g.pick(&[0.0, life * 0.1, life * 0.5, life, life * 1.5]);
            wf(&mut m, p, age);
            wf(&mut m, p + 0x28, life);
            wf(&mut m, p + 0x24, g.pick(&[0.0, life * 0.2, life * 0.6, age]));
            m.w32(p + 0x1c, g.below(8));
            for at in [0x14u32, 0x18, 0x20, 0x2c, 0x30] {
                wf(&mut m, p + at, g.f(-4.0, 4.0));
            }
        }
        for k in 0..16 {
            wf(&mut m, FX + 4 * k, g.f(-100.0, 100.0));
        }
        if (m.r32(FX + 0xd54) as i32) < 0 {
            // the texture numbers of the two over the frame (+0x38, +4) are read then
            m.w32(FX + 0x38, g.below(8));
            m.w32(FX + 4, g.below(8));
        }
        let calls = run(&mut r, &log, &mut m, case, &mut |m, e| fx::speed_dust_draw(m, e, FX, SCRATCH), 0x12fb40, &[(4, FX as u64)], &[]);
        drawn += calls.iter().filter(|c| c.0 == vaddr(0x240)).count();
        fading_in += (0..64u32).filter(|&k| {
            let p = FX + 0x50 + 0x34 * k;
            f32::from_bits(m.r32(p)) < f32::from_bits(m.r32(p + 0x24))
        }).count();
    }
    println!("{drawn} puffs drawn");
    assert!(drawn > 20_000 && fading_in > 10_000);
}

#[test]
fn spray_draw_particles_matches_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    let mut g = Rng(0x0e74_2000_0000_0001);
    let (mut strips, mut boxed) = (0usize, 0usize);
    for case in 0..1500u32 {
        let mut m = fresh(&mut g, 0xb30);
        let count = g.pick(&[0i32, 1, 2, 5, 7, -1]);
        let oldest = g.pick(&[0i32, 3, 4, 6, -2]);
        // (and the two before the ring: the oldest place −2 reaches them. The one just before has its age and
        // life on the count and the oldest place, so with −2 the count stays at 1 or less: only the one before
        // that is read; else the count is a float's bits and the draw runs a billion times)
        for k in -2i32..5 {
            let e = (FX + 0x20).wrapping_add((0x3c0 * k) as u32);
            let life = g.pick(&[0.5f32, 1.0, 3.0]);
            wf(&mut m, e + 0x3bc, life);
            wf(&mut m, e + 0x3b8, g.pick(&[0.0, life * 0.3, life, life * 1.2]));
            m.w32(e + 0x3b4, g.pick(&[0i32, 1, 2, 7, 25, -1]) as u32);
            for j in 0..25 {
                for l in 0..3 {
                    wf(&mut m, e + 16 * j + 4 * l, g.f(-5.0, 5.0));
                    wf(&mut m, e + 0x190 + 16 * j + 4 * l, g.f(-30.0, 30.0));
                }
                wf(&mut m, e + 0x340 + 4 * j, g.f(0.0, 1.0));
            }
            for l in 0..4 {
                wf(&mut m, e + 0x320 + 4 * l, g.f(-2000.0, 2000.0));
            }
            for l in 0..3 {
                let c = g.f(0.0, 1.5);
                wf(&mut m, e + 0x3a8 + 4 * l, g.pick(&[c, c, -1.0, 2.5]));
            }
        }
        m.w32(FX + 0x18, if oldest < 0 { count.min(1) } else { count } as u32);
        m.w32(FX + 0x1c, oldest as u32);
        let calls = run(&mut r, &log, &mut m, case, &mut |m, e| fx::spray_draw_particles(m, e, FX, SCRATCH), 0x12d890, &[(4, FX as u64)], &[]);
        strips += calls.iter().filter(|c| c.0 == vaddr(0x220) && c.1[1] > 0).count();
        boxed += calls.iter().filter(|c| c.0 == vaddr(0x280)).count();
    }
    println!("{strips} strips, {boxed} box tests");
    assert!(strips > 3000 && boxed > 2000);
}

#[test]
fn rotated_quads_match_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let Some(mut r) = runner(&log) else { return };
    let mut g = Rng(0x0e74_3000_0000_0001);
    for case in 0..4000u32 {
        let mut m = fresh(&mut g, 0xc0);
        for k in 0..4 {
            let c = g.f(0.0, 2.0);
            wf(&mut m, COLOUR + 4 * k, g.pick(&[0.0, 1.0, c]));
        }
        wf(&mut m, COLOUR + 0x10, g.f(0.0, 1.0));
        let a = g.f(-720.0, 720.0);
        let angle = g.pick(&[0.0f32, 90.0, -45.0, 370.0, a, a]);
        let (x, y, w, h) = (g.f(-50.0, 600.0), g.f(-50.0, 450.0), g.f(0.0, 200.0), g.f(0.0, 200.0));
        let tex = case % 2 == 1;
        let uv = [g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0)];
        let fregs = [(12, angle), (13, x), (14, y), (15, w), (16, h), (17, uv[1]), (18, uv[0]), (19, uv[2])];
        if tex {
            // (the ninth float on the caller's stack)
            let sp = r.cpu.gpr(29) as u32;
            r.mem.write_u32(sp, uv[3].to_bits());
        }
        let quad = if tex { Some(uv.map(F)) } else { None };
        run(&mut r, &log, &mut m, case, &mut |m, e| fx::sprite_draw_rotated_quad(m, e, COLOUR, F(angle), F(x), F(y), F(w), F(h), quad, SCRATCH), if tex { 0x19bb48 } else { 0x19b8c8 }, &[(4, COLOUR as u64)], &fregs);
    }
}

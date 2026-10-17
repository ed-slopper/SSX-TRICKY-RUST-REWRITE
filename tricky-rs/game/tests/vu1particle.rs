//! The particle draw run whole in the function runner (board rows G7e2, G7e3; TRICKY_ELF, skipped without it), as
//! the oracle for the VU1 particle programs' ports. All of it runs as original code:
//! - the renderer's camera: the frame's matrices (0x1e85b0) and its projection (vf20);
//! - each particle system drawn: program 7 by `ParticleSys_Draw` (its +0x258), program 6 by the renderer's +0x250
//!   called directly, both through `PS2Render_Submit` into a draw list;
//! - the flush (`PS2Draw_FlushBuckets`, with `PS2Draw_WriteTexState`, which uploads the program and the frame's
//!   words for it).
//!
//! The DMA chain the flush builds is then walked into `r5900::vu1::Vu1`, which runs the game's programs and logs
//! what they send to the GS. The systems' blocks are made here: a count of particles, a seed, a box (a corner and
//! three edges) in front of, around or behind the camera, a size, a colour and texture coordinates. The box test,
//! the texture and blend virtuals, the allocator, the fill and the screen's size are stand-ins.
//!
//! `particle_programs_match_the_game` then checks `tricky_game::particlevu` against every call of a ported
//! program: from the registers and data memory it was called with, the same data memory, registers and GS packets
//! after.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::vu1::{Run, Vu1};
use r5900::Runner;
use tricky_game::particlevu;
use tricky_game::vu::Vu;

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x0010_0000;
const APP: u32 = ARENA;
const R: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x2000;
const CAM: u32 = ARENA + 0x2400;
const TS: u32 = ARENA + 0x2500;
const TEXTABLE: u32 = ARENA + 0x2600;
const SLOT: u32 = ARENA + 0x3a00;
const HEAP: u32 = ARENA + 0x4000;
const SYSTEMS: u32 = ARENA + 0x2_0000;
const D: u32 = ARENA + 0x4_0000;
const PKT: u32 = ARENA + 0x6_0000;
const OUT: u32 = ARENA + 0x8_0000;
const PAGES: u32 = ARENA + 0xa_0000;
const BATCHES: u32 = ARENA + 0xa_1000;
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

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

fn w32(r: &mut Runner, at: u32, v: u32) {
    r.mem.write_u32(at, v);
}
fn wf(r: &mut Runner, at: u32, v: f32) {
    r.mem.write_u32(at, v.to_bits());
}

/// A camera: where it is and whether it looks down (else along +y); the far plane
struct Camera {
    at: [f32; 3],
    down: bool,
    far: f32,
}

/// A particle system's block for program 7 (9 quadwords) or 6 (20: the same nine, then random words), around
/// `centre`
fn block(g: &mut Rng, prog6: bool, centre: [f32; 3]) -> Vec<u32> {
    if prog6 {
        return block6(g, centre);
    }
    let mut b = vec![0u32; 36];
    let fb = |v: f32| v.to_bits();
    // the count (some past one packet's 99), the size's base and spread
    b[0] = [0u32, 1, 2, 7, 40, 98, 99, 100, 150, 198, 199, 250][g.below(12) as usize];
    b[1] = fb(g.f(-1.0, 3.0));
    b[2] = fb(g.f(0.5, 4.0));
    b[3] = g.next();
    // the seed
    for k in 4..8 {
        b[k] = g.next();
    }
    // the box: a corner (w 1) and three edges (w 0), the random point in [1, 2) along each
    let span = [10.0, 60.0, 300.0][g.below(3) as usize];
    for k in 0..3 {
        b[8 + k] = fb(centre[k] - 1.5 * span + g.f(-span, span));
    }
    b[11] = fb(1.0);
    for e in 0..3 {
        for k in 0..3 {
            b[12 + 4 * e + k] = fb(if k == e { span } else { g.f(-0.2, 0.2) * span });
        }
        b[15 + 4 * e] = 0;
    }
    // the colour, two texture coordinates
    for k in 0..4 {
        b[24 + k] = fb([0.0, 128.0, g.f(0.0, 255.0)][g.below(3) as usize]);
    }
    for k in 28..36 {
        b[k] = fb(g.f(0.0, 1.0));
    }
    b
}

/// A block for program 6 (20 quadwords): trails of points from a line near `centre`
fn block6(g: &mut Rng, centre: [f32; 3]) -> Vec<u32> {
    let mut b = vec![0u32; 80];
    let fb = |v: f32| v.to_bits();
    let mut q = |b: &mut Vec<u32>, k: usize, v: [f32; 4]| {
        for (j, x) in v.into_iter().enumerate() {
            b[4 * k + j] = fb(x);
        }
    };
    // +0: the trails, the points a trail (some past one packet's 97), the countdown and its step
    let trails = [0u32, 1, 2, 3, 5, 9][g.below(6) as usize];
    let points = [0u32, 1, 4, 12, 30, 60, 120, 200][g.below(8) as usize];
    q(&mut b, 0, [0.0, 0.0, g.f(-0.2, 3.0), [g.f(0.05, 0.6), 0.0, 1.5][g.below(3) as usize]]);
    b[0] = trails;
    b[1] = points;
    // +1: the size's spread and the fade's, then their bases (the fade the t a point is shown below)
    q(&mut b, 1, [g.f(0.0, 4.0), g.f(0.0, 1.0), g.f(0.5, 6.0), [g.f(-0.5, 2.5), 4.0][g.below(2) as usize]]);
    // +2: the alpha's step, t's step, two seeds; +3, +4 seeds
    q(&mut b, 2, [g.f(0.0, 0.1), [g.f(0.02, 0.4), 0.0, 1.0][g.below(3) as usize], 0.0, 0.0]);
    for k in [10usize, 11, 12, 13, 14, 15, 16, 17, 18, 19] {
        b[k] = g.next();
    }
    // +5: the trail's direction (by t); +6..+9 the bend's box (taken from +5); +10 the start's corner, +11, +12
    // its edges by R, +13 the step between trails
    let span = [5.0, 40.0, 200.0][g.below(3) as usize];
    let dir = |g: &mut Rng| [g.f(-span, span), g.f(-span, span), g.f(-span, span), 0.0];
    let d = dir(g);
    q(&mut b, 5, d);
    for k in 6..10 {
        let v = dir(g);
        q(&mut b, k, v);
    }
    // (sometimes the box's corner 0: ACC, which the program ends with, then exactly 0, its zero flags set)
    if g.below(4) == 0 {
        q(&mut b, 6, [0.0; 4]);
    }
    q(&mut b, 10, [centre[0] - span, centre[1] - span, centre[2] - span, 1.0]);
    for k in 11..14 {
        let v = dir(g);
        q(&mut b, k, v.map(|x| x * 0.5));
    }
    // +14..+17 the colour: a base, two terms by R, one by the countdown
    q(&mut b, 14, [g.f(0.0, 200.0), g.f(0.0, 200.0), g.f(0.0, 200.0), g.f(40.0, 300.0)]);
    for k in 15..18 {
        let v = [0; 4].map(|_| g.f(-60.0, 60.0));
        q(&mut b, k, v);
    }
    // +18, +19 the texture coordinates
    for k in 72..80 {
        b[k] = fb(g.f(0.0, 1.0));
    }
    b
}

/// One whole draw of `n` systems (program 6 when `prog6`); the VU1 that ran the chain (its calls recorded)
fn draw(seed: u64, n: usize, prog6: bool, cam: &Camera) -> Option<Vu1> {
    let mut r = Runner::from_env()?;
    let mut g = Rng(seed);
    r.mem.write(ARENA, &vec![0u8; SIZE as usize]).unwrap();
    for f in [0x1d8bc8u32, 0x1e2e60, 0x1e2a60, 0x1e8838, 0x1e88c8, 0x1c4d68, 0x1c4d30, 0x1c4f28, 0x1c4340, 0x1c4468, 0x1c6f98, 0x1c7068, 0x1c7088, 0x1c4fe0, 0x1c4220, 0x1c51f0, 0x1c5368, 0x1c42a0, 0x1c5c88, 0x1c0340, 0x1c03e8, 0x1c04f8, 0x1c0560, 0x1c05d8, 0x1c06a0, 0x1e85b0, 0x2cfc00, 0x1c4650, 0x1c4c38] {
        r.allow(f);
    }
    // the projection's calls: vf20, Math_SinCos, tanf and its parts
    for f in [0x1dc3e0u32, 0x250c10, 0x2f2880, 0x2f3108, 0x2f3e38] {
        r.allow(f);
    }
    let heap = Rc::new(RefCell::new(HEAP));
    let h = heap.clone();
    r.stub(0x2ccf70, move |c, _| {
        let mut p = h.borrow_mut();
        let at = *p;
        *p += (c.gpr(5) as u32 + 15) & !15;
        c.set_gpr(2, at as u64);
    });
    r.stub(0x2cc808, |c, m| {
        let (at, v, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        for k in 0..n / 4 {
            m.write_u32(at + 4 * k, v);
        }
    });
    r.stub(0x1c3818, |_, _| {});
    // the box test keeps every system; the state virtuals do nothing; the screen's size
    for off in [0x1c0u32, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0x200, 0x280] {
        r.stub(vaddr(off), |c, _| c.set_gpr(2, 0));
    }
    for (off, v) in [(0x60u32, 640u64), (0x58, 448)] {
        r.stub(vaddr(off), move |c, _| c.set_gpr(2, v));
    }
    w32(&mut r, 0x337c58, APP);
    w32(&mut r, APP + 0x724, R);
    w32(&mut r, APP + 0x30, TEXTABLE);
    w32(&mut r, R + 0x10, RVT);
    for off in [0x1c0u32, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0x200, 0x280, 0x60, 0x58] {
        w32(&mut r, RVT + off + 4, vaddr(off));
    }
    w32(&mut r, RVT + 0x250 + 4, 0x1e2a60);
    w32(&mut r, RVT + 0x258 + 4, 0x1e2e60);
    w32(&mut r, R + 0x470, TS);
    w32(&mut r, R + 0x480, D);
    w32(&mut r, R + 0x680, PKT);
    w32(&mut r, R + 0x3f4, CAM);
    w32(&mut r, D + 4, PAGES);
    w32(&mut r, D + 0xc, BATCHES);
    w32(&mut r, D + 0xfa64, 0x32);
    w32(&mut r, D + 0xfa68, 0x8c);
    r.call(0x1db640).expect("StaticInit_Render");
    // the camera (row vectors, translation last), the frame's other matrices plain
    let c = cam.at;
    for base in [0x1730u32, 0x4d0, 0x570, 0x5b0] {
        for k in 0..16 {
            wf(&mut r, R + base + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
        }
    }
    let view = if cam.down {
        [[1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [-c[0], c[1], c[2], 1.0]]
    } else {
        [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [-c[0], c[2], -c[1], 1.0]]
    };
    for (i, row) in view.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            wf(&mut r, CAM + 16 * i as u32 + 4 * j as u32, *v);
        }
    }
    w32(&mut r, R + 0x17d0, 0);
    r.cpu.set_gpr(4, R as u64);
    r.cpu.set_f(12, 1.0);
    r.cpu.set_f(13, 10.0);
    r.cpu.set_f(14, cam.far);
    r.call(0x1dc3e0).expect("the projection");
    for base in [0x4d0u32, 0x570] {
        wf(&mut r, R + base, 1.0);
        wf(&mut r, R + base + 0x14, 1.0);
    }
    // (and the screen's x and y scales, +0x5f0, which the particle programs use: 0 likewise; a screen's put in)
    wf(&mut r, R + 0x5f0, 320.0);
    wf(&mut r, R + 0x5f4, -224.0);
    // the systems: around the camera's view (ahead, beside, behind)
    let ahead = if cam.down { [0.0, 0.0, -1.0] } else { [0.0, 1.0, 0.0] };
    for k in 0..n {
        let ps = SYSTEMS + 0x200 * k as u32;
        let d = [-50.0, 5.0, 40.0, 200.0, 1500.0][g.below(5) as usize];
        let side = [g.f(-200.0, 200.0), g.f(-200.0, 200.0), g.f(-200.0, 200.0)];
        let centre = [0, 1, 2].map(|i| c[i] + ahead[i] * d + side[i] * 0.5);
        let b = block(&mut g, prog6, centre);
        for (i, w) in b.iter().enumerate() {
            w32(&mut r, ps + 0x10 + 4 * i as u32, *w);
        }
        w32(&mut r, ps + 4, g.below(4));
        w32(&mut r, ps + 8, g.below(11));
        if prog6 {
            r.cpu.set_gpr(4, R as u64);
            r.cpu.set_gpr(5, (ps + 0x10) as u64);
            r.call(0x1e2a60).expect("the program 6 packet");
        } else {
            r.cpu.set_gpr(4, ps as u64);
            r.call(0x1d8bc8).expect("ParticleSys_Draw");
        }
    }
    w32(&mut r, SLOT, OUT);
    r.cpu.set_gpr(4, D as u64);
    r.cpu.set_gpr(5, SLOT as u64);
    r.call(0x1c4fe0).expect("the flush");
    let mut v = Vu1 { record: true, trace: std::env::var("TRACE").is_ok(), ..Vu1::default() };
    // the registers as some earlier program left them (so that lanes a program does not load are not all 0)
    for k in 1..32 {
        v.regs.vf[k] = [0; 4].map(|_| g.f(-100.0, 100.0));
    }
    v.run_chain(&mut r.mem, OUT).expect("the chain on VU1");
    Some(v)
}

fn cameras() -> Vec<Camera> {
    vec![
        Camera { at: [0.0, 0.0, 300.0], down: true, far: 100000.0 },
        Camera { at: [100.0, -400.0, 50.0], down: false, far: 100000.0 },
        Camera { at: [0.0, 0.0, 20.0], down: false, far: 100000.0 },
        Camera { at: [-80.0, 30.0, 1000.0], down: true, far: 1200.0 },
    ]
}

#[test]
fn the_particle_draw_runs_on_vu1() {
    let Some(v) = draw(1, 12, false, &cameras()[0]) else { return };
    println!("{} program calls, {} kicks, {} GS quadwords", v.calls.len(), v.kicks.len(), v.gs.len());
    for run in v.runs.iter().take(2) {
        let q = |k: usize| [0, 1, 2, 3].map(|j| f32::from_bits(u32::from_le_bytes(run.data_in[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap())));
        println!("start {:#x} top {} consts {:?}", run.start, run.top, (0..7).map(q).collect::<Vec<_>>());
    }
    assert_eq!(v.calls.len(), 12);
}

/// The micro memory a chain loads, to its last instruction (a longer program's tail may follow it in the VU)
fn program(chain: u32) -> Option<Vec<u8>> {
    let mut r = Runner::from_env()?;
    let mut v = Vu1::default();
    v.run_chain(&mut r.mem, chain).ok()?;
    let end = v.micro.chunks(8).rposition(|p| p.iter().any(|b| *b != 0))? + 1;
    v.micro.truncate(8 * end);
    Some(v.micro)
}

/// A recorded call against the port: the same data memory, registers and GS packets
fn check(run: &Run, gs: &[[u32; 4]], port: fn(&mut Vu, u32), what: &str) -> usize {
    let mut p = Vu::new();
    p.vf = run.regs_in.vf;
    p.vi = run.regs_in.vi;
    p.acc = run.regs_in.acc;
    p.q = run.regs_in.q;
    p.clip = run.regs_in.clip;
    p.r = run.regs_in.r;
    p.i = run.regs_in.i;
    p.mac = run.regs_in.mac;
    p.top = run.top;
    for (k, q) in p.mem.iter_mut().enumerate() {
        *q = [0, 1, 2, 3].map(|j| u32::from_le_bytes(run.data_in[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap()));
    }
    port(&mut p, run.start);
    let theirs = &gs[run.gs_from..run.gs_to];
    if let Some(i) = theirs.iter().zip(&p.gs).position(|(a, b)| a != b) {
        panic!("{what} from {:#x}: GS quadword {i}: theirs {:x?} ours {:x?}", run.start, theirs[i], p.gs[i]);
    }
    assert_eq!(theirs.len(), p.gs.len(), "{what} from {:#x}: GS quadwords", run.start);
    for k in 0..0x400 {
        let t = [0, 1, 2, 3].map(|j| u32::from_le_bytes(run.data_out[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap()));
        assert_eq!(t, p.mem[k], "{what} from {:#x}: data memory at quadword {k}", run.start);
    }
    for k in 0..32 {
        assert_eq!(run.regs_out.vf[k].map(f32::to_bits), p.vf[k].map(f32::to_bits), "{what} from {:#x}: vf{k}", run.start);
    }
    assert_eq!(run.regs_out.vi, p.vi, "{what} from {:#x}: vi", run.start);
    assert_eq!(run.regs_out.acc.map(f32::to_bits), p.acc.map(f32::to_bits), "{what} from {:#x}: ACC", run.start);
    assert_eq!(run.regs_out.q.to_bits(), p.q.to_bits(), "{what} from {:#x}: Q", run.start);
    assert_eq!(run.regs_out.clip, p.clip, "{what} from {:#x}: the clip flags", run.start);
    assert_eq!(run.regs_out.r, p.r, "{what} from {:#x}: R", run.start);
    assert_eq!(run.regs_out.i.to_bits(), p.i.to_bits(), "{what} from {:#x}: I", run.start);
    assert_eq!(run.regs_out.mac, p.mac, "{what} from {:#x}: the MAC flags", run.start);
    theirs.len()
}

#[test]
fn particle_programs_match_the_game() {
    let ports: [(u32, bool, fn(&mut Vu, u32), &str); 2] = [(0x31c790, false, particlevu::run7, "program 7"), (0x31c260, true, particlevu::run6, "program 6")];
    for (chain, prog6, port, what) in ports {
        let Some(image) = program(chain) else { return };
        let (mut calls, mut quadwords, mut kicks, mut culled, mut full, mut none) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        for (k, cam) in cameras().iter().enumerate() {
            for seed in 0..6u64 {
                let Some(v) = draw(seed * 16 + k as u64, 40, prog6, cam) else { return };
                for run in &v.runs {
                    assert_eq!(run.micro[..image.len()], image[..], "{what}: the program in micro memory");
                    let sent = check(run, &v.gs, port, what);
                    quadwords += sent;
                    calls += 1;
                    let n = v.kicks.iter().filter(|&&(_, from)| from >= run.gs_from && from < run.gs_to).count();
                    kicks += n;
                    full += (n >= 2) as usize;
                    none += (n == 0) as usize;
                    // (program 7) sprites asked for (the count, one at least) against those sent
                    if !prog6 {
                        let count = (u32::from_le_bytes(run.data_in[16 * run.top as usize..16 * run.top as usize + 4].try_into().unwrap()) as u16 as i16).max(1) as usize;
                        culled += count - (sent - n) / 5;
                    }
                }
            }
        }
        println!("{what}: {calls} calls ({full} filling a packet, {none} sending nothing), {kicks} packets, {quadwords} GS quadwords, {culled} points culled");
        assert!(calls > 500 && full > 30 && none > 50 && (prog6 || culled > 1000), "{what}");
    }
}

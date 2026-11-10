//! `tricky_game::snowfall` (board row G7c1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random manager (its layers, origin, velocity, alpha, density, cell size, range),
//! a game with a random camera (near the origin's cells and far from them) and weather, and a renderer with a fake
//! vtable whose box test answers inside, outside or partly by the box; then one of 14 functions. The base
//! constructor, `Rand_Fx` and the renderer's calls (with their float arguments) are logging stand-ins. The memory,
//! the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::snowfall::{self as s, SnowEnv};

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
const SIZE: u32 = 0x4000;
const MAN: u32 = ARENA;
const APP: u32 = ARENA + 0x800;
const GAME: u32 = ARENA + 0x1000;
const RENDER: u32 = ARENA + 0x2000;
const VT: u32 = ARENA + 0x2100;
const OBJ30: u32 = ARENA + 0x2800;
const WEATHER: u32 = ARENA + 0x2900;
const POS: u32 = ARENA + 0x3000;
const STATICS: u32 = 0x3d0da0;
// the renderer's virtuals (stand-in addresses)
const VIRTUALS: [u32; 6] = [0x1c0, 0x1c8, 0x1e0, 0x1f0, 0x260, 0x280];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Base(u32, u32),
    Rand,
    R(u32, u32, Vec<u32>),
    Cull(u32, [u32; 4], [u32; 4]),
}

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

/// The box test's answer: mostly partly in (2) or inside (0), sometimes outside (1)
fn cull_answer(min: &[u32; 4], max: &[u32; 4]) -> u32 {
    let h = mix(min[0] ^ min[1].rotate_left(7) ^ max[2].rotate_left(13) ^ max[0]);
    match h % 8 {
        0 | 1 => 1,
        2 | 3 => 0,
        _ => 2,
    }
}

struct Ours {
    log: Vec<Call>,
    rand: u32,
}
impl SnowEnv for Ours {
    fn base_construct(&mut self, _m: &mut Mem, this: u32, a1: u32) {
        self.log.push(Call::Base(this, a1));
    }
    fn rand_fx(&mut self) -> u32 {
        self.log.push(Call::Rand);
        self.rand = mix(self.rand.wrapping_add(1));
        self.rand
    }
    fn renderer(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.log.push(Call::R(func, this, args.to_vec()));
        0
    }
    fn cull_box(&mut self, _m: &mut Mem, func: u32, _this: u32, min: [u32; 4], max: [u32; 4]) -> u32 {
        self.log.push(Call::Cull(func, min, max));
        cull_answer(&min, &max)
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn the_snowfall_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x1c8db8u32, 0x1c8ec8, 0x1c90c8, 0x1c90d0, 0x1c9130, 0x1c91c0, 0x1c9208, 0x1c9218, 0x1c92b8, 0x1c9310, 0x1c9388,
        0x1c9740, 0x1c9a18, 0x102f50,
    ] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let rand = Rc::new(RefCell::new(0u32));
    let l = log.clone();
    r.stub(0x17cf50, move |c, _| l.borrow_mut().push(Call::Base(c.gpr(4) as u32, c.gpr(5) as u32)));
    let (l, rr) = (log.clone(), rand.clone());
    r.stub(0x23d4a8, move |c, _| {
        l.borrow_mut().push(Call::Rand);
        let v = mix(rr.borrow().wrapping_add(1));
        *rr.borrow_mut() = v;
        c.set_gpr(2, v as i32 as i64 as u64);
    });
    for off in VIRTUALS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, m| {
            let this = c.gpr(4) as u32;
            match off {
                0x280 => {
                    let (a, b) = (c.gpr(5) as u32, c.gpr(6) as u32);
                    let min = [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k));
                    let max = [0, 1, 2, 3].map(|k| m.read_u32(b + 4 * k));
                    l.borrow_mut().push(Call::Cull(vaddr(off), min, max));
                    c.set_gpr(2, cull_answer(&min, &max) as u64);
                }
                0x1c8 => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32, c.gpr(6) as u32, c.f(12).to_bits()])),
                0x1f0 | 0x260 => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32, c.gpr(6) as u32])),
                _ => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32])),
            }
        });
    }
    let mut g = Rng(0x1c88_0000_0000_0001);
    let mut counts = [0usize; 14];
    for case in 0..6_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (STATICS, vec![0; 0x30]), (s::GAPP, vec![0; 4])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..12 {
            m.w32(STATICS + 4 * k, g.next());
        }
        m.w32(s::GAPP, APP);
        // the manager
        let size = [350.0, 100.0, g.f(50.0, 600.0)][g.below(3) as usize];
        wf(&mut m, MAN + 0x4b8, size);
        wf(&mut m, MAN + 0x4b4, [0.5, 0.05, g.f(0.0, 1.0)][g.below(3) as usize]);
        wf(&mut m, MAN + 0x4b0, g.f(0.0, 0.5));
        wf(&mut m, MAN + 0x4bc, g.f(1.0, 6.0));
        wf(&mut m, MAN + 0x4c0, g.f(2.0, 8.0));
        for k in 0..4 {
            wf(&mut m, MAN + 0x490 + 4 * k, if k == 3 { g.f(0.0, 2.0) } else { g.f(-3000.0, 3000.0) });
            wf(&mut m, MAN + 0x4a0 + 4 * k, g.f(-400.0, 400.0));
        }
        for k in 0..8 {
            m.w32(MAN + 0x10 + 0x90 * k, if g.below(4) == 0 { 0 } else { g.below(500) });
        }
        // the game, its camera and weather
        m.w32(APP + 0x724, RENDER);
        m.w32(APP + 0x730, GAME);
        m.w32(APP + 0x30, OBJ30);
        m.w32(RENDER + 0x10, VT);
        for off in VIRTUALS {
            // (a this-adjustment, as the renderer's virtual bases have)
            m.w32(VT + off, 0x20);
            m.w32(VT + off + 4, vaddr(off));
        }
        m.w32(GAME + 0x74, g.below(4).min(1));
        let cam = g.below(3);
        m.w32(GAME + 0x290, cam);
        m.w32(GAME + (cam << 7) + 0xa4, WEATHER);
        let dens = m.r32(MAN + 0x4b4);
        m.w32(WEATHER + 0x18, if g.below(2) == 0 { dens } else { g.f(0.0, 1.0).to_bits() });
        let near = g.below(2) == 0;
        for k in 0..4 {
            let o = f32::from_bits(m.r32(MAN + 0x490 + 4 * k));
            let v = if near { o + g.f(-2.0, 2.0) * size } else { g.f(-9000.0, 9000.0) };
            wf(&mut m, GAME + 0x90 + 0x80 * cam + 0x20 + 4 * k, v);
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let seed = g.next();
        *rand.borrow_mut() = seed;
        let mut ours = Ours { log: Vec::new(), rand: seed };
        let kind = g.below(14) as usize;
        let mut fl: Vec<f32> = Vec::new();
        let mut ans = None;
        let (f, args): (u32, Vec<u32>) = match kind {
            0 => {
                let a1 = g.next();
                ans = Some(s::construct(&mut m, &mut ours, MAN, a1));
                (0x1c8db8, vec![MAN, a1])
            }
            1 => {
                s::init_layers(&mut m, &mut ours, MAN);
                (0x1c8ec8, vec![MAN])
            }
            2 => {
                let v = g.f(0.0, 1.0);
                fl.push(v);
                s::set_alpha(&mut m, MAN, F(v));
                (0x1c90c8, vec![MAN])
            }
            3 => {
                let v = g.f(0.0, 1.0);
                fl.push(v);
                s::set_density(&mut m, MAN, F(v));
                (0x1c90d0, vec![MAN])
            }
            4 => {
                let v = [350.0, g.f(20.0, 800.0)][g.below(2) as usize];
                fl.push(v);
                s::set_scale(&mut m, MAN, F(v));
                (0x1c9130, vec![MAN])
            }
            5 => {
                let (a, b) = (g.f(0.0, 8.0), g.f(0.0, 8.0));
                fl.extend([a, b]);
                s::set_range(&mut m, MAN, F(a), F(b));
                (0x1c91c0, vec![MAN])
            }
            6 => {
                for k in 0..4 {
                    wf(&mut m, POS + 4 * k, g.f(-500.0, 500.0));
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                s::set_velocity(&mut m, MAN, POS);
                (0x1c9208, vec![MAN, POS])
            }
            7 => {
                s::snap_origin(&mut m, MAN);
                (0x1c9218, vec![MAN])
            }
            8 => {
                s::step(&mut m, MAN);
                (0x1c92b8, vec![MAN])
            }
            9 => {
                let at: Vec<f32> = (0..4).map(|_| g.f(-3000.0, 3000.0)).collect();
                for k in 0..4 {
                    wf(&mut m, POS + 4 * k, at[k as usize]);
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let l = MAN + 0x10 + 0x90 * g.below(8);
                s::draw_layer(&mut m, &mut ours, MAN, [F(at[0]), F(at[1]), F(at[2]), F(at[3])], l);
                (0x1c9310, vec![MAN, POS, l])
            }
            10 => {
                // a walk from a random level and cell
                let at: Vec<f32> = (0..4).map(|k| f32::from_bits(m.r32(MAN + 0x490 + 4 * k)) + g.f(-2.0, 2.0) * size).collect();
                for k in 0..4 {
                    wf(&mut m, POS + 4 * k, at[k as usize]);
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let (ix, iy, iz) = (g.below(9) as i32 - 4, g.below(9) as i32 - 4, g.below(9) as i32 - 4);
                let level = [4, 3, 2, 1][g.below(4) as usize];
                let inside = g.below(2);
                s::walk(&mut m, &mut ours, MAN, [F(at[0]), F(at[1]), F(at[2]), F(at[3])], ix, iy, iz, level, inside);
                (0x1c9388, vec![MAN, POS, ix as u32, iy as u32, iz as u32, level as u32, inside])
            }
            11 => {
                s::draw(&mut m, &mut ours, MAN);
                (0x1c9740, vec![MAN])
            }
            12 => {
                s::static_init(&mut m);
                (0x1c9a18, vec![])
            }
            _ => {
                // a whole draw from the start of the walk (level 0) with every test made
                m.w32(GAME + 0x74, 1);
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                s::draw(&mut m, &mut ours, MAN);
                (0x1c9740, vec![MAN])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        for (i, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + i, *v);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {f:#x} answer");
        }
        assert_eq!(*log.borrow(), ours.log, "case {case}: {f:#x} calls");
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}");
}

//! `tricky_game::lensfx` (board row G7d2a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random node (halves, views, kept flares with depths), world (cells with masks and
//! slots listing lights, some with flare flags), sun, renderer with a fake vtable and camera matrix; then one of 5
//! functions. The base object, the node's two parts and the renderer's virtuals are logging stand-ins with answers
//! made from what they are given. The memory, the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::lensfx::{self as lf, LensEnv};
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

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const APP: u32 = ARENA;
const OBJ30: u32 = ARENA + 0x100;
const GAME: u32 = ARENA + 0x200;
const SUN: u32 = ARENA + 0x600;
const RENDER: u32 = ARENA + 0x700;
const VT: u32 = ARENA + 0x800;
const CAMMAT: u32 = ARENA + 0xc00;
const GRID: u32 = ARENA + 0xd00;
const CELLS: u32 = ARENA + 0xe00;
const RECS: u32 = ARENA + 0x1000;
const LISTS: u32 = ARENA + 0x1800;
const LIGHTS: u32 = ARENA + 0x1c00;
const NODE0: u32 = ARENA + 0x3000;
const NCELL: u32 = 5;
const NLIGHT: u32 = 16;
const SLOTS: [u32; 9] = [0xc0, 0xd0, 0xf8, 0x190, 0x198, 0x270, 0x278, 0x318, 0x2a0];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    C(u32, Vec<u32>),
    V(u32, u32, Vec<u32>),
}

/// The stand-in projection: x and y over the depth (at least 1) × 300, about the screen's middle
fn project_answer(p: &[u32; 4]) -> [u32; 4] {
    let f = p.map(f32::from_bits);
    let z = f[2].abs().max(1.0);
    [f[0] / z * 300.0 + 320.0, f[1] / z * 300.0 + 224.0, f[2], 1.0].map(f32::to_bits)
}
/// The node's own virtuals (the PS2 node's) at these addresses
const NSLOTS: [u32; 7] = [0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80];
fn naddr(off: u32) -> u32 {
    0x01fe_0000 + off
}
/// +0x50's answer: nothing shows now and then, else a fraction
fn node_answer(func: u32, args: &[u32]) -> u32 {
    if func != naddr(0x50) {
        return 0;
    }
    let h = mix(args[0] ^ args[1].rotate_left(11));
    // (−0 now and then: the FPU's compare takes it for 0)
    match h % 6 {
        0 | 3 => 0,
        1 => 0x8000_0000,
        _ => ((h >> 8) as f32 / (1u32 << 24) as f32).to_bits(),
    }
}
fn hidden_answer(p: &[u32; 4]) -> u32 {
    (mix(p[0] ^ p[1].rotate_left(9)) % 4 == 0) as u32
}
fn vanswer(func: u32, this: u32, args: &[u32]) -> u32 {
    if func == vaddr(0xf8) {
        CAMMAT
    } else {
        mix(func ^ this ^ args.first().copied().unwrap_or(7))
    }
}

struct Ours(Vec<Call>);
impl LensEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call::C(func, args.to_vec()));
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.0.push(Call::V(func, this, args.to_vec()));
        vanswer(func, this, args)
    }
    fn view_quad(&mut self, _m: &mut Mem, func: u32, this: u32) -> [u32; 4] {
        self.0.push(Call::V(func, this, vec![]));
        [0, 1, 2, 3].map(|k| mix(this + k))
    }
    fn view_values(&mut self, _m: &mut Mem, func: u32, this: u32) -> [u32; 3] {
        self.0.push(Call::V(func, this, vec![]));
        [0, 1, 2].map(|k| mix(this ^ (k << 20)))
    }
    fn hidden(&mut self, _m: &mut Mem, func: u32, this: u32, p: [u32; 4]) -> u32 {
        self.0.push(Call::V(func, this, p.to_vec()));
        hidden_answer(&p)
    }
    fn project(&mut self, _m: &mut Mem, func: u32, this: u32, p: [u32; 4]) -> [u32; 4] {
        self.0.push(Call::V(func, this, p.to_vec()));
        project_answer(&p)
    }
    fn node_vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32], f12: u32) -> u32 {
        let mut a = args.to_vec();
        if func == naddr(0x50) {
            a.push(f12);
        }
        self.0.push(Call::V(func, this, a));
        node_answer(func, args)
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn lens_fx_node_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x1cbbc8u32, 0x1cbdb8, 0x1cbe20, 0x1cc088, 0x2006e8, 0x25ecb8, 0x1cd040, 0x250c10, 0x2510f8, 0x102f50, 0x1cc190, 0x250e98, 0x250d60, 0x137140, 0x137108] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for (f, n) in [(lf::BASE_CONSTRUCT, 2), (lf::BASE_DESTRUCT, 2)] {
        let l = log.clone();
        r.stub(f, move |c, _| l.borrow_mut().push(Call::C(f, (0..n).map(|k| c.gpr(4 + k) as u32).collect())));
    }
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, m| {
            let f = vaddr(off);
            match off {
                // the quad returned through a0, this in a1
                0xc0 => {
                    let (out, this) = (c.gpr(4) as u32, c.gpr(5) as u32);
                    for k in 0..4 {
                        m.write_u32(out + 4 * k, mix(this + k));
                    }
                    l.borrow_mut().push(Call::V(f, this, vec![]));
                }
                0xd0 => {
                    let this = c.gpr(4) as u32;
                    for k in 0..3u32 {
                        m.write_u32(c.gpr(5 + k as usize) as u32, mix(this ^ (k << 20)));
                    }
                    l.borrow_mut().push(Call::V(f, this, vec![]));
                }
                0x278 => {
                    let (out, this, a) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
                    let p = [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k));
                    for (k, w) in project_answer(&p).into_iter().enumerate() {
                        m.write_u32(out + 4 * k as u32, w);
                    }
                    l.borrow_mut().push(Call::V(f, this, p.to_vec()));
                }
                0x270 => {
                    let (this, a) = (c.gpr(4) as u32, c.gpr(5) as u32);
                    let p = [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k));
                    c.set_gpr(2, hidden_answer(&p) as u64);
                    l.borrow_mut().push(Call::V(f, this, p.to_vec()));
                }
                _ => {
                    let this = c.gpr(4) as u32;
                    let args: Vec<u32> = match off {
                        0xf8 => vec![],
                        0x318 => vec![c.gpr(5) as u32, c.gpr(6) as u32],
                        _ => vec![c.gpr(5) as u32],
                    };
                    c.set_gpr(2, vanswer(f, this, &args) as u64);
                    l.borrow_mut().push(Call::V(f, this, args));
                }
            }
        });
    }
    // the sun's ray table (and the word before it) as the exe has it
    let mut rays = vec![0u8; 0x130];
    r.mem.read(lf::SUN_RAYS - 8, &mut rays).unwrap();
    for off in NSLOTS {
        let l = log.clone();
        r.stub(naddr(off), move |c, _| {
            let f = naddr(off);
            let this = c.gpr(4) as u32;
            let mut args: Vec<u32> = match off {
                0x50 => vec![c.gpr(5) as u32, c.gpr(6) as u32],
                0x60 | 0x68 | 0x70 => vec![c.gpr(5) as u32],
                _ => vec![],
            };
            let ans = node_answer(f, &args);
            if off == 0x50 {
                args.push(c.f(12).to_bits());
                c.set_f(0, f32::from_bits(ans));
            }
            l.borrow_mut().push(Call::V(f, this, args));
        });
    }
    let mut g = Rng(0x1e45_0000_0000_0001);
    let mut counts = [0usize; 10];
    let mut drawn = 0usize;
    let mut suns = 0usize;
    let (mut offered, mut replaced) = (0usize, 0usize);
    for case in 0..20_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (lf::GAPP, vec![0; 4]),
                (lf::WORLD, vec![0; 0x3c + 4 * 0xa2 + 4]),
                (lf::SUN_RAYS - 8, rays.clone()),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(lf::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        m.w32(APP + 0x730, GAME);
        m.w32(APP + 0x30, OBJ30);
        m.w32(RENDER + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
            m.w32(VT + off + 4, vaddr(off));
        }
        m.w32(GAME + 0x2c, SUN);
        m.w32(GAME + 0x290, g.below(2));
        m.w32(GAME + 0x298, 1 + g.below(2));
        wf(&mut m, SUN + 0x5c, g.f(100.0, 3000.0));
        // the sun: on (mostly), its colours, size, angles and distance
        m.w32(SUN, (g.below(5) != 0) as u32);
        for o in [4u32, 8, 0xc, 0x18, 0x1c, 0x20] {
            m.w32(SUN + o, g.below(256));
        }
        wf(&mut m, SUN + 0x14, g.f(10.0, 200.0));
        wf(&mut m, SUN + 0x28, [g.f(-180.0, 180.0), 0.0, 90.0, 45.0][g.below(4) as usize]);
        wf(&mut m, SUN + 0x2c, g.f(-10.0, 80.0));
        wf(&mut m, SUN + 0x30, g.f(1000.0, 10000.0));
        for k in 0..16 {
            wf(&mut m, CAMMAT + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else if k >= 12 { g.f(-500.0, 500.0) } else { g.f(-0.5, 0.5) });
        }
        // the node (now and then off a 16-byte boundary): its half (−1 as made, now and then) and view, kept flares
        // (a view full now and then) with depths
        #[allow(non_snake_case)]
        let NODE = NODE0 + if g.below(6) == 0 { 4 } else { 0 };
        m.w32(NODE + 0xc, [0u32, 1, 0, 1, u32::MAX][g.below(5) as usize]);
        m.w32(NODE + 0x14, g.below(2));
        for k in 0..2 * 2 * 10 {
            let e = NODE + 0x1c0 + 0xd0 * k + (k / 10) * (0x820 - 0xd0 * 10);
            wf(&mut m, e + 0xc8, [g.f(0.0, 3000.0), -g.f(0.0, 10.0), 0.0][g.below(3) as usize]);
            m.w32(e + 0xcc, LIGHTS + 0x60 * g.below(NLIGHT));
        }
        wf(&mut m, NODE + 0x2fd8, g.f(0.0, 2.0));
        // the PS2 node's vtable (its drawing virtuals), the half ready, the sun and flare blocks' flags
        let nvt = NODE0 + 0x2fe0;
        m.w32(NODE + 8, nvt);
        for off in NSLOTS {
            m.w16(nvt + off, [0u16, 0x10][g.below(2) as usize]);
            m.w32(nvt + off + 4, naddr(off));
        }
        m.w32(NODE + 0x10, [u32::MAX, 0, 1][g.below(3) as usize]);
        m.w32(NODE + 0x20, g.below(3));
        m.w32(NODE + 0x24, g.below(3));
        for h in 0..2u32 {
            for v in 0..2u32 {
                m.w32(NODE + 0x2240 + 0x6c0 * h + 0x360 * v, (g.below(3) != 0) as u32);
                for k in 0..10u32 {
                    m.w32(NODE + 0x1c0 + 0x1040 * h + 0x820 * v + 0xd0 * k, g.below(3));
                }
            }
        }
        m.w32(NODE + 0x18, g.below(512));
        m.w32(NODE + 0x1c, g.below(512));
        // the view blocks: the screen's box, the camera matrix (a turn about z and a position), the near value
        for blk in 0..4u32 {
            let b = NODE + 0x60 * blk;
            m.w32(b + 0x30, g.below(64));
            m.w32(b + 0x34, g.below(64));
            m.w32(b + 0x38, 320 + g.below(321));
            m.w32(b + 0x3c, 224 + g.below(225));
            let a = g.f(-3.1, 3.1);
            let (sn, cs) = (a.sin(), a.cos());
            let rows = [[cs, -sn, 0.0, 0.0], [sn, cs, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-100.0, 100.0), 1.0]];
            for (i, row) in rows.iter().enumerate() {
                for (j, v) in row.iter().enumerate() {
                    wf(&mut m, b + 0x40 + 0x10 * i as u32 + 4 * j as u32, *v);
                }
            }
            wf(&mut m, b + 0x88, g.f(500.0, 12000.0));
        }
        // the flare counts (half −1 reads its counts from +0x1a8, where the last view block's +0x88 is)
        for k in 0..6 {
            m.w32(NODE + 0x1a8 + 4 * k, [0u32, 3, 9, 10, 10, 10][g.below(6) as usize]);
        }
        // the world: cells with masks, slots listing lights
        m.w32(lf::WORLD, GRID);
        m.w32(GRID + 0x48, 1 + g.below(3));
        let ncell = g.below(NCELL + 1);
        for c in 0..0xa2 {
            // (now and then a hole, with cells after it that the walk never reaches)
            let hole = ncell > 1 && c == ncell - 2 && case % 3 == 0;
            m.w32(lf::WORLD + 0x3c + 4 * c, if c < ncell && !hole { CELLS + 0x40 * c } else { 0 });
        }
        for c in 0..NCELL {
            let at = CELLS + 0x40 * c;
            m.w16(at + 0x36, g.below(8) as u16);
            m.w32(at + 0x38, RECS + 0x100 * c);
            for s in 0..3 {
                let rec = RECS + 0x100 * c + 0x4c * s;
                m.w16(rec + 0x2c, [0u16, 1, 3, 6, 0xffff][g.below(5) as usize]);
                let list = LISTS + 0x80 * c + 0x20 * s;
                m.w32(rec + 0x40, list);
                for k in 0..8 {
                    m.w32(list + 4 * k, LIGHTS + 0x60 * g.below(NLIGHT));
                }
            }
        }
        for k in 0..NLIGHT {
            let at = LIGHTS + 0x60 * k;
            m.w32(at + 4, g.next() & 0x1f0);
            wf(&mut m, at + 8, [g.f(0.0, 2.0), 0.0][(g.below(8) == 0) as usize]);
            for j in 0..3 {
                wf(&mut m, at + 0x10 + 4 * j, g.f(0.0, 1.0));
            }
            for j in 0..3 {
                wf(&mut m, at + 0x28 + 4 * j, g.f(-3000.0, 3000.0));
            }
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let kind = g.below(10) as usize;
        // (the flares' size registers: as the STANDIN takes them for a draw, else random)
        let sizes = if kind == 4 { [0.0f32, 0.0] } else { [g.f(10.0, 900.0), g.f(10.0, 900.0)] };
        r.cpu.set_f(26, sizes[0]);
        r.cpu.set_f(27, sizes[1]);
        // (the game draws view 0 first, which turns a half of −1 into 0)
        if kind == 4 && m.r32(NODE + 0xc) == u32::MAX {
            m.w32(GAME + 0x290, 0);
            r.mem.write_u32(GAME + 0x290, 0);
        }
        let mut ans = None;
        let mut fans = None;
        let mut fl = Vec::new();
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let a1 = g.next();
                ans = Some(lf::construct(&mut m, &mut ours, NODE, a1));
                (0x1cbbc8, vec![NODE, a1])
            }
            1 => {
                let a1 = g.below(4);
                lf::destruct(&mut m, &mut ours, NODE, a1);
                (0x1cbdb8, vec![NODE, a1])
            }
            2 => {
                let light = LIGHTS + 0x60 * g.below(NLIGHT);
                let d = [g.f(0.0, 3000.0), -1.0, 0.0][g.below(3) as usize];
                fl.push(d);
                lf::add_flare(&mut m, NODE, light, F(d));
                (0x1cc088, vec![NODE, light])
            }
            7 => {
                lf::callback(&mut m, &mut ours, NODE);
                drawn += ours.0.iter().filter(|c| matches!(c, Call::V(f, _, _) if *f == naddr(0x58))).count();
                (0x1cdc78, vec![NODE])
            }
            8 => {
                let k = [g.f(-1.0, 2.0), 0.0, 1.0, -0.0][g.below(4) as usize];
                fl.push(k);
                lf::set_scale(&mut m, NODE, F(k));
                (0x1cdfc0, vec![NODE])
            }
            9 => match g.below(4) {
                0 => {
                    fans = Some(lf::scale(&m, NODE).0.to_bits());
                    (0x1ce000, vec![NODE])
                }
                1 => {
                    let k = g.f(-3.0, 3.0);
                    fl.push(k);
                    ans = Some(lf::vec4_scale(&mut m, CAMMAT + 0x40, CAMMAT, F(k)));
                    (0x1ce058, vec![CAMMAT + 0x40, CAMMAT])
                }
                2 => {
                    let k = [g.f(-3.0, 3.0), 0.0][g.below(2) as usize];
                    fl.push(k);
                    ans = Some(lf::vec3_div(&mut m, CAMMAT + 0x44, CAMMAT + 4, F(k)));
                    (0x1ce088, vec![CAMMAT + 0x44, CAMMAT + 4])
                }
                _ => {
                    fans = Some(lf::vec3_dot(&m, CAMMAT, CAMMAT + 0x14).0.to_bits());
                    (0x1ce0c0, vec![CAMMAT, CAMMAT + 0x14])
                }
            },
            6 => {
                if m.r32(NODE + 0xc) == u32::MAX {
                    m.w32(NODE + 0xc, 0);
                    r.mem.write_u32(NODE + 0xc, 0);
                }
                lf::flares_part(&mut m, &mut ours, NODE, [F(sizes[0]), F(sizes[1])]);
                (0x1cc190, vec![NODE])
            }
            5 => {
                if m.r32(NODE + 0xc) == u32::MAX {
                    m.w32(NODE + 0xc, 1);
                    r.mem.write_u32(NODE + 0xc, 1);
                }
                lf::sun_part(&mut m, &mut ours, NODE);
                (0x1cd040, vec![NODE])
            }
            3 => {
                lf::lens_draw(&mut m, &mut ours, lf::WORLD, NODE);
                (0x2006e8, vec![lf::WORLD, NODE])
            }
            _ => {
                lf::draw(&mut m, &mut ours, NODE);
                (0x1cbe20, vec![NODE])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        for (i, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + i, *v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        if let Some(v) = fans {
            assert_eq!(r.cpu.f(0).to_bits(), v, "case {case}: {func:#x} float answer");
        }
        assert_eq!(*log.borrow(), ours.0, "case {case}: {func:#x} calls");
        if matches!(kind, 4 | 5) && m.r32(NODE + 0x2240 + 0x6c0 * m.r32(NODE + 0xc) + 0x360 * m.r32(NODE + 0x14)) == 1 {
            suns += 1;
        }
        if matches!(kind, 3 | 4) {
            offered += ours.0.iter().filter(|c| matches!(c, Call::V(f, _, _) if *f == vaddr(0x270))).count();
        }
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&theirs), w(b));
            }
        }
        if kind == 2 && m.r32((NODE + 0x1b0 + 4 * m.r32(NODE + 0x14)).wrapping_add(m.r32(NODE + 0xc).wrapping_mul(8))) as i32 >= 10 {
            replaced += 1;
        }
    }
    println!("{counts:?}, {offered} lights tested, {replaced} adds to a full view, {suns} suns drawn, {drawn} callbacks drawing");
    assert!(offered > 3000 && replaced > 300 && suns > 1000 && drawn > 500);
}

//! `tricky_game::surfacespray` (board row G7f6) against `Fx_SurfaceSpray` 0x1311b0 in the function runner
//! (TRICKY_ELF; skipped without it), the particle emitter functions running as original code: the whole pool and
//! the light's three floats compared word for word after each call.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tricky_game::emitter::V4;
use tricky_game::surfacespray::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
    fn small(&mut self) -> f32 {
        (self.next() % 2000) as f32 / 1000.0 - 1.0
    }
    fn v(&mut self) -> V4 {
        [self.f(), self.f(), self.f(), self.f()]
    }
    fn u(&mut self) -> V4 {
        [self.small(), self.small(), self.small(), 0.0]
    }
}

fn put(r: &mut Runner, a: u32, v: V4) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn surface_spray_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x102f50u32, 0x1d50b0, 0x1d52e8, 0x1d5300, 0x1d5380, 0x1d5418, 0x1d5548, 0x1d5790, 0x1d58e0, 0x1d59e0, 0x1d59f0] {
        r.allow(a);
    }
    // the factor randomiser draws through Rand_Fx too, so it runs as original code on the same stub
    r.allow(0x1d5180);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let pool = r.mem.alloc(BYTES as u32);
    let rider = r.mem.alloc(0x5c00);
    let light = r.mem.alloc(0x500);
    let app = r.mem.alloc(0x800);
    let mid = r.mem.alloc(0x40);
    let tab = r.mem.alloc(0x1000);
    let sprtab = r.mem.alloc(0x100);
    let sprites: Vec<u32> = (0..64).map(|i| 0x7000_0000 + 0x111 * i).collect();
    for (i, w) in sprites.iter().enumerate() {
        r.mem.write_u32(sprtab + 4 * i as u32, *w);
    }
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, mid);
    r.mem.write_u32(mid + 0x24, tab);
    r.mem.write_u32(app + 0x30, sprtab);
    r.mem.write_u32(rider + 0x5ae0, light);
    let mut g = Rng(0x1311_b0c4_e3a7_5d21);
    let mut s = Spray::default();
    let (mut spawned, mut sparked, mut ploughed, mut wrapped) = (0, 0, 0, 0);
    for case in 0..30_000 {
        if case % 2 == 0 {
            for w in s.w.iter_mut() {
                *w = (g.small() * 5.0).to_bits();
            }
            let cnt = [0u32, 1, 2, 5, 39, 40][(g.next() % 6) as usize];
            s.w[0] = cnt;
            s.w[1] = [0u32, 1, 20, 38, 39][(g.next() % 5) as usize];
            s.w[2] = (g.next() % 2) as u32;
            s.w[3] = ([70.0f32, 11.0, 10.0, 5.0][(g.next() % 4) as usize]).to_bits();
            s.w[8] = (g.next() % 2) as u32;
            for i in 0..40usize {
                let b = 0x10 + 0x54 * i;
                s.w[b + 0x3c / 4] = ([5.0f32, 12.0, 1.0][(g.next() % 3) as usize]).to_bits();
                s.w[b + 0x50] = 0;
                s.w[b + 0x51] = 0;
            }
        }
        let scalev = [1.0f32, 1.0, 12.0][(g.next() % 3) as usize];
        let vel = {
            let v = g.v();
            [v[0] * scalev, v[1] * scalev, v[2] * scalev, v[3]]
        };
        let surface = [0u32, 1, 2, 3, 9, 0xd, 0x12, 0x13, 5, 0x20][(g.next() % 10) as usize];
        let cfg: [u32; 9] = [
            ([0.0f32, 10.0, 60.0, 300.0, 2000.0][(g.next() % 5) as usize]).to_bits(),
            ([0.5f32, 1.0, 3.0][(g.next() % 3) as usize]).to_bits(),
            (g.small().abs() * 0.5).to_bits(),
            (g.small().abs() + 0.5).to_bits(),
            g.next() % 64,
            (g.small().abs()).to_bits(),
            (g.small() * 0.2).to_bits(),
            (g.small().abs()).to_bits(),
            g.next() % 8,
        ];
        let ri = SprayIn {
            vel,
            f214: [0.0f32, 0.5, -0.5, 1.0, g.small()][(g.next() % 5) as usize],
            flipped: (g.next() % 2) as u32,
            surface,
            hit: [0u32, 1, 2, 3][(g.next() % 4) as usize],
            ground: g.v(),
            motion: [2u32, 2, 2, 1, 3, 0, 5][(g.next() % 7) as usize],
            f13c: (g.next() % 2) as u32,
            w4a60: if g.next() % 3 == 0 { g.v() } else { g.u() },
            w4a70: if g.next() % 3 == 0 { g.v() } else { g.u() },
            w4a80: if g.next() % 3 == 0 { g.v() } else { g.u() },
            w4a90: g.v(),
            v180: g.u(),
            v190: g.u(),
            v1a0: g.u(),
            colour: [g.small().abs(), g.small().abs(), g.small().abs() * 1.2, g.small().abs()],
            cfg,
        };
        let mut lt = Light {
            f0c: [-0.1f32, 0.0, 0.5, 1.0, 1.2, 0.3, 0.9][(g.next() % 7) as usize],
            f10: [0.0f32, 0.5, 3.0, 30.0][(g.next() % 4) as usize],
            f14: [0.0f32, 1.0, 0.5][(g.next() % 3) as usize],
        };
        r.mem.write_u32(rider + 0x290, ri.surface);
        let c = tab + ri.surface * 100;
        for k in 0..9 {
            r.mem.write_u32(c + 0x34 + 4 * k, ri.cfg[k as usize]);
        }
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        r.mem.write_f32(rider + 0x214, ri.f214);
        r.mem.write_u32(rider + 0x1b4, ri.flipped);
        r.mem.write_u32(rider + 0x13c, ri.f13c);
        put(&mut r, rider + 0x150, ri.vel);
        put(&mut r, rider + 0x2d0, ri.ground);
        put(&mut r, rider + 0x4a60, ri.w4a60);
        put(&mut r, rider + 0x4a70, ri.w4a70);
        put(&mut r, rider + 0x4a80, ri.w4a80);
        put(&mut r, rider + 0x4a90, ri.w4a90);
        put(&mut r, rider + 0x180, ri.v180);
        put(&mut r, rider + 0x190, ri.v190);
        put(&mut r, rider + 0x1a0, ri.v1a0);
        put(&mut r, light + 0x470, ri.colour);
        r.mem.write_f32(light + 0xc, lt.f0c);
        r.mem.write_f32(light + 0x10, lt.f10);
        r.mem.write_f32(light + 0x14, lt.f14);
        for (i, w) in s.w.iter().enumerate() {
            r.mem.write_u32(pool + 4 * i as u32, *w);
        }
        let before = s.clone();
        let words: VecDeque<u32> = (0..24).map(|_| g.next() | (g.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        r.cpu.set_gpr(4, pool as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x1311b0).unwrap();
        let mut rd = words;
        update(&mut s, &ri, &mut lt, &sprites, &mut || rd.pop_front().expect("rand"));
        if s.w[0] != before.w[0] || s.w[1] != before.w[1] {
            if before.w[0] == 40 {
                wrapped += 1;
            }
            if ri.motion == 2 && matches!(ri.surface, 9 | 0xd | 0x12 | 0x13) || ri.motion == 3 {
                sparked += 1;
            } else {
                spawned += 1;
            }
        }
        if ri.motion == 1 {
            ploughed += 1;
        }
        for (i, w) in s.w.iter().enumerate() {
            assert_eq!(r.mem.read_u32(pool + 4 * i as u32), *w, "case {case} word {i:#x} (rider {ri:?})");
        }
        for (b, v) in [(0xc, lt.f0c), (0x10, lt.f10), (0x14, lt.f14)] {
            assert_eq!(r.mem.read_u32(light + b), v.to_bits(), "case {case} light {b:#x}");
        }
    }
    eprintln!("spawned {spawned} sparked {sparked} ploughed {ploughed} wrapped {wrapped}");
    assert!(spawned > 500 && sparked > 300 && wrapped > 50);
}

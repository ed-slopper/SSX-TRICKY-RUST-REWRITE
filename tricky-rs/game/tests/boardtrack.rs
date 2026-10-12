//! `tricky_game::boardtrack` (board row G7f) against `Fx_BoardTrack` 0x132648 in the function runner (TRICKY_ELF;
//! skipped without it): the strip object compared word for word after each call, over riders on every surface,
//! motion and light set.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boardtrack::*;

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
    fn v(&mut self) -> [f32; 4] {
        [self.f(), self.f(), self.f(), self.f()]
    }
    fn u(&mut self) -> [f32; 4] {
        [self.small(), self.small(), self.small(), 0.0]
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn board_track_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x1325b8);
    r.allow(0x102f50);
    let trail = r.mem.alloc(0x1a00);
    let rider = r.mem.alloc(0x5c00);
    let light = r.mem.alloc(0x500);
    let tab1 = r.mem.alloc(0x10);
    let tab2 = r.mem.alloc(0x400);
    r.mem.write_u32(rider + 0x4494, tab1);
    r.mem.write_u32(tab1 + 4, tab2);
    r.mem.write_u32(rider + 0x5ae0, light);
    let mut g = Rng(0x7ac4_b0a2_d132_648f);
    let mut t = Trail::default();
    let (mut off, mut fresh, mut kept) = (0, 0, 0);
    for case in 0..30_000 {
        if case % 2 == 0 {
            for w in t.w.iter_mut() {
                *w = g.f().to_bits();
            }
            t.w[0x1940 / 4] = g.next() % 100;
            t.w[0x1944 / 4] = [0, 1, 2, 5, 50, 89, 90, 91, 99, 100][(g.next() % 10) as usize];
            let fade = [0.0f32, 0.3, 0.6, 0.9, 1.0, 0.45][(g.next() % 6) as usize];
            t.w[0x1948 / 4] = fade.to_bits();
            t.w[0x194c / 4] = [-1.0f32, 1.0, 0.0, 0.5, -0.99][(g.next() % 5) as usize].to_bits();
        }
        let lights = Lights {
            ambient: [g.small().abs(), g.small().abs(), g.small().abs()],
            weights: [0, 1, 2].map(|_| [g.small().abs(), g.small(), g.small().abs()]),
            dirs: [0, 1, 2].map(|_| g.u()),
        };
        let pos = if g.next() % 5 == 0 { g.v() } else { [g.f() * 2.0, g.f(), g.f() * 3.0, 0.0] };
        let up = if g.next() % 3 == 0 { g.v() } else { [g.small() * 0.3, 1.0, g.small() * 0.3, 0.0] };
        let ground = g.v();
        let near = g.next() % 2 == 0;
        let last = if near { [ground[0] + g.small() * 40.0, ground[1] + g.small() * 10.0, ground[2] + g.small() * 40.0, ground[3]] } else { g.v() };
        t.w[0x1960 / 4..0x1960 / 4 + 4].copy_from_slice(&last.map(f32::to_bits));
        if g.next() % 2 == 0 {
            // the last direction: toward the new point
            let d = [ground[0] - last[0], ground[1] - last[1], ground[2] - last[2], ground[3] - last[3]];
            let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3]).sqrt();
            let ld = if n == 0.0 { d } else { d.map(|x| x / n) };
            t.w[0x1970 / 4..0x1970 / 4 + 4].copy_from_slice(&ld.map(f32::to_bits));
        }
        if g.next() % 2 == 0 {
            t.w[0x1980 / 4] = (g.small().abs() * 30.0).to_bits();
        }
        let ri = TrackIn {
            surface: [0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 19, 20, 21, 0x23][(g.next() % 17) as usize],
            motion: [2u32, 2, 5, 6, 0, 1, 3, 7][(g.next() % 8) as usize],
            hit: [0u32, 1, 2, 3, 4, 0xffff_ffff][(g.next() % 6) as usize],
            m: [g.v(), g.v(), g.v(), g.v()],
            table: [g.v(), g.v()],
            pos,
            up,
            ground,
            f1fc: [0.0f32, 1.0, 0.25][(g.next() % 3) as usize],
            f214: [0.0f32, 0.5, -0.5, 1.0, -1.0, 0.3, g.small()][(g.next() % 7) as usize],
            lights,
        };
        r.mem.write_u32(rider + 0x290, ri.surface);
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        for (i, m) in ri.m.iter().enumerate() {
            put(&mut r, rider + 0x5760 + 0x10 * i as u32, *m);
        }
        put(&mut r, tab2 + 0x330, ri.table[0]);
        put(&mut r, tab2 + 0x230, ri.table[1]);
        put(&mut r, rider + 0x150, ri.pos);
        put(&mut r, rider + 0x2a0, ri.up);
        put(&mut r, rider + 0x2d0, ri.ground);
        r.mem.write_f32(rider + 0x1fc, ri.f1fc);
        r.mem.write_f32(rider + 0x214, ri.f214);
        for k in 0..3 {
            r.mem.write_f32(light + 0x404 + 4 * k, ri.lights.ambient[k as usize]);
            for j in 0..3 {
                r.mem.write_f32(light + 0x414 + 0x10 * k + 4 * j, ri.lights.weights[k as usize][j as usize]);
            }
            put(&mut r, light + 0x440 + 0x10 * k, ri.lights.dirs[k as usize]);
        }
        for (i, w) in t.w.iter().enumerate() {
            r.mem.write_u32(trail + 4 * i as u32, *w);
        }
        let before = t.clone();
        r.cpu.set_gpr(4, trail as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x132648).unwrap();
        board_track(&mut t, &ri);
        if before.w[0x1940 / 4] != t.w[0x1940 / 4] {
            fresh += 1;
        } else if t.w[0x1950 / 4] == 0 {
            off += 1;
        } else {
            kept += 1;
        }
        for (i, w) in t.w.iter().enumerate() {
            let got = r.mem.read_u32(trail + 4 * i as u32);
            assert_eq!(got, *w, "case {case} word {i:#x} (byte {:#x}) {ri:?}", 4 * i);
        }
    }
    assert!(off > 1000 && fresh > 1000 && kept > 500, "{off} {fresh} {kept}");
}

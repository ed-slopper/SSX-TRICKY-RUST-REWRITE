//! `tricky_game::camera::eval_look_at` (board row E11b) against `CamCamera_EvalLookAt` 0x1722e8 in the function
//! runner (TRICKY_ELF; skipped without it). Math_Atan/Asin and Vec3_* run for real.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::camera::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 12 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            5 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn nz(&mut self) -> f32 {
        let v = self.f();
        if v == 0.0 { 0.5 } else { v }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.f(), self.f(), self.f()]
    }
    fn big(&mut self) -> f32 {
        match self.next() % 4 {
            0 => (self.next() % 6000) as f32,
            1 => -((self.next() % 6000) as f32),
            _ => self.f() * 300.0,
        }
    }
}

struct Env {
    t: Vec<[f32; 3]>,
}
impl LookEnv for Env {
    fn translation(&mut self, idx: i32) -> [f32; 3] {
        self.t[idx as usize]
    }
}

#[test]
fn eval_look_at_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x250c10u32, 0x2510f8, 0x1741d0, 0x251238, 0x250fd0, 0x137140, 0x137108, 0x102f80] {
        r.allow(a);
    }
    r.call(0x251238).unwrap();
    let cam = r.mem.alloc(0x100);
    let pb = r.mem.alloc(0x60);
    r.mem.write_u32(cam + 0x44, pb);
    let view = r.mem.alloc(0x60);
    let list = r.mem.alloc(8 * 0x20);
    let timer = r.mem.alloc(16);
    let mats = r.mem.alloc(8 * 0x40);
    let tbl = r.mem.alloc(16);
    r.mem.write_u32(tbl + 4, mats);
    let mut riders = Vec::new();
    for _ in 0..8 {
        let a = r.mem.alloc(0x5800);
        let b = r.mem.alloc(0x80);
        r.mem.write_u32(a + 0x464, b);
        r.mem.write_u32(a + 0x4494, tbl);
        r.mem.write_u32(a + 0x4498, tbl);
        riders.push((a, b));
    }
    let mut g = Rng(0x1234_5678_9abc_def1);
    for case in 0..60_000 {
        let mut p = LookP {
            w00: (g.next() % 6 != 0) as i32,
            w04: (g.next() % 8) as i32,
            off: g.v3(),
            f14: g.f(),
            f18: g.f(),
            f1c: g.f(),
            f20: g.f(),
            f24: g.f(),
            f28: g.f(),
            w2c: (g.next() % 2) as i32,
            w30: (g.next() % 2) as i32,
            w34: match g.next() % 6 {
                0 => -1,
                1 => -2,
                _ => (g.next() % 4) as i32,
            },
            w38: (g.next() % 2) as i32,
            f3c: g.f(),
            w40: (g.next() % 2) as i32,
        };
        if false {
            p.f20 = 0.5;
        }
        let tr: Vec<[f32; 3]> = (0..8).map(|_| g.v3()).collect();
        let mut entries = Vec::new();
        for k in 0..8usize {
            let (ra, rb) = riders[k];
            let has = g.next() % 8 != 0 || (p.w34 == -2 && p.w04 as usize == k);
            let rider = LookRider {
                motion: (g.next() % 9) as i32,
                f57b8: if g.next() % 2 == 0 { 0.0 } else { (g.next() % 100) as f32 / 1000.0 },
                v57d0: [g.big(), g.big(), g.big()],
                v57e0: [g.big(), g.big(), g.big(), g.f()],
                w290: (g.next() % 18) as i32,
                v2a0: g.v3(),
                v2d0: g.v3(),
                f464_60: g.f(),
                v4a90: g.v3(),
                t4498: tr[0],
            };
            let p14 = g.v3();
            entries.push(LookEntry { rider: if has { Some(rider.clone()) } else { None }, p14 });
            let e = list + 0x20 * k as u32;
            r.mem.write_u32(e, if has { ra } else { 0 });
            for i in 0..3 {
                r.mem.write_f32(e + 0x14 + 4 * i, p14[i as usize]);
            }
            if has {
                r.mem.write_u32(ra + 0x424, rider.motion as u32);
                r.mem.write_f32(ra + 0x57b8, rider.f57b8);
                r.mem.write_u32(ra + 0x290, rider.w290 as u32);
                for i in 0..3u32 {
                    r.mem.write_f32(ra + 0x57d0 + 4 * i, rider.v57d0[i as usize]);
                    r.mem.write_f32(ra + 0x2a0 + 4 * i, rider.v2a0[i as usize]);
                    r.mem.write_f32(ra + 0x2d0 + 4 * i, rider.v2d0[i as usize]);
                    r.mem.write_f32(ra + 0x4a90 + 4 * i, rider.v4a90[i as usize]);
                    r.mem.write_f32(ra + 0x1a0 + 4 * i, 0.5 + i as f32);
                }
                r.mem.write_f32(ra + 0x1a0 + 12, 0.25);
                r.mem.write_f32(ra + 0x4a90 + 12, 0.0);
                for i in 0..4u32 {
                    r.mem.write_f32(ra + 0x57e0 + 4 * i, rider.v57e0[i as usize]);
                }
                r.mem.write_f32(rb + 0x60, rider.f464_60);
            }
        }
        for (i, t) in tr.iter().enumerate() {
            for j in 0..3 {
                r.mem.write_f32(mats + 0x40 * i as u32 + 0x30 + 4 * j, t[j as usize]);
            }
            r.mem.write_f32(mats + 0x40 * i as u32 + 0x3c, 1.0);
        }
        let mut from = g.v3();
        if g.next() % 8 == 0 {
            from = [0.0; 3];
            p.off = [0.0; 3];
            p.w34 = 0;
            p.w30 = 0;
            entries[p.w04 as usize].p14 = [0.0; 3];
            for i in 0..3 {
                r.mem.write_f32(list + 0x20 * p.w04 as u32 + 0x14 + 4 * i, 0.0);
            }
        }
        let mut last = LastDir { x: g.f(), y: g.f() };
        let mut tmr = if g.next() % 2 == 0 { 0.0 } else { (g.next() % 200) as f32 / 1000.0 };
        let p7 = (g.next() % 3) as i64;
        let p8 = (g.next() % 3) as i64;
        let extra = g.f();
        // memory image
        let words: [(u32, u32); 15] = [
            (0, p.w00 as u32),
            (4, p.w04 as u32),
            (0x2c, p.w2c as u32),
            (0x30, p.w30 as u32),
            (0x34, p.w34 as u32),
            (0x38, p.w38 as u32),
            (0x40, p.w40 as u32),
            (0x8, p.off[0].to_bits()),
            (0xc, p.off[1].to_bits()),
            (0x10, p.off[2].to_bits()),
            (0x14, p.f14.to_bits()),
            (0x18, p.f18.to_bits()),
            (0x1c, p.f1c.to_bits()),
            (0x20, p.f20.to_bits()),
            (0x24, p.f24.to_bits()),
        ];
        for (o, w) in words {
            r.mem.write_u32(pb + o, w);
        }
        r.mem.write_f32(pb + 0x28, p.f28);
        r.mem.write_f32(pb + 0x3c, p.f3c);
        for i in 0..3u32 {
            r.mem.write_f32(view + 4 * i, from[i as usize]);
        }
        for o in [0xcu32, 0x10, 0x14, 0x18, 0x48] {
            r.mem.write_u32(view + o, 0xdead_beef);
        }
        r.mem.write_f32(0x335724, last.x);
        r.mem.write_f32(0x335728, last.y);
        r.mem.write_f32(timer, tmr);
        r.cpu.set_gpr(4, cam as u64);
        r.cpu.set_gpr(5, view as u64);
        r.cpu.set_gpr(6, list as u64);
        r.cpu.set_gpr(8, timer as u64);
        r.cpu.set_gpr(9, p7 as u64);
        r.cpu.set_gpr(10, p8 as u64);
        r.cpu.set_f(12, extra);
        r.call(0x1722e8).unwrap();
        let mut v = LookView { pos: from, yaw: 0.0, pitch: 0.0, roll: 0.0, fov: 0.0, f48: 0.0 };
        let mut env = Env { t: tr };
        eval_look_at(&p, &entries, &mut v, &mut last, &mut tmr, p7, p8, extra, &mut env);
        let got = [v.yaw, v.pitch, v.roll, v.fov, v.f48];
        let want: Vec<f32> = [0xcu32, 0x10, 0x14, 0x18, 0x48].iter().map(|o| r.mem.read_f32(view + o)).collect();
        for k in 0..5 {
            assert_eq!(got[k].to_bits(), want[k].to_bits(), "case {case} out {k}: {} vs {} ({p:?})", got[k], want[k]);
        }
        assert_eq!(tmr.to_bits(), r.mem.read_f32(timer).to_bits(), "case {case} timer");
        assert_eq!(last.x.to_bits(), r.mem.read_f32(0x335724).to_bits(), "case {case} last.x");
        assert_eq!(last.y.to_bits(), r.mem.read_f32(0x335728).to_bits(), "case {case} last.y");
    }
}

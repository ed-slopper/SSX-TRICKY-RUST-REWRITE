//! `tricky_game::camera::region_check_condition` (board row E11c) against `CamRegion_CheckCondition` 0x171500 in
//! the function runner (TRICKY_ELF; skipped without it).

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
        match self.next() % 8 {
            0 => 0.0,
            1 => 0.33,
            2 => f32::from_bits(0x3ea8_f5c3),
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
}

#[test]
fn check_condition_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x137140);
    let cam = r.mem.alloc(0x20);
    let rec = r.mem.alloc(0x200);
    let list = r.mem.alloc(8 * 0x20);
    let riders: Vec<u32> = (0..8).map(|_| r.mem.alloc(0x5800)).collect();
    r.mem.write_u32(cam + 4, rec);
    let mut g = Rng(0x1357_2468_9abc_def3);
    for case in 0..40_000 {
        let p = CondP { w184: (g.next() % 2) as i32, w18c: (g.next() % 2) as i32, f188: g.f() };
        let w154 = (g.next() % 8) as u32;
        let has = g.next() % 6 != 0;
        let rider = CondRider { motion: [4, 5, 6, 7][(g.next() % 4) as usize], f57b8: g.f(), v57c0: [g.f(), g.f(), g.f()] };
        let p14 = [g.f(), g.f(), g.f()];
        r.mem.write_u32(rec + 0x184, p.w184 as u32);
        r.mem.write_u32(rec + 0x18c, p.w18c as u32);
        r.mem.write_f32(rec + 0x188, p.f188);
        r.mem.write_u32(rec + 0x154, w154);
        let e = list + 0x20 * w154;
        r.mem.write_u32(e, if has { riders[w154 as usize] } else { 0 });
        for i in 0..3 {
            r.mem.write_f32(e + 0x14 + 4 * i, p14[i as usize]);
        }
        let ra = riders[w154 as usize];
        r.mem.write_u32(ra + 0x424, rider.motion as u32);
        r.mem.write_f32(ra + 0x57b8, rider.f57b8);
        for i in 0..3 {
            r.mem.write_f32(ra + 0x57c0 + 4 * i, rider.v57c0[i as usize]);
        }
        r.cpu.set_gpr(4, cam as u64);
        r.cpu.set_gpr(5, list as u64);
        r.call(0x171500).unwrap();
        let want = r.cpu.gpr(2);
        let got = region_check_condition(&p, if has { Some(&rider) } else { None }, p14);
        assert_eq!(got, want, "case {case} {p:?} {rider:?}");
    }
}

//! `tricky_game::camera::track_rider` (board row E11c) against `Camera_TrackRider` 0x173270 in the function
//! runner (TRICKY_ELF; skipped without it). Vec3_* and Vec4_Scale run for real.

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
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            4 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn v(&mut self) -> [f32; 4] {
        // velocity lengths around the 10 cut
        let s = [0.0f32, 0.5, 1.0, 3.0, 5.0, 100.0][(self.next() % 6) as usize];
        [self.f() * s * 0.2, self.f() * s * 0.2, self.f() * s * 0.2, if self.next() % 2 == 0 { 0.0 } else { self.f() }]
    }
}

#[test]
fn track_rider_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x137140u32, 0x137108, 0x102f80, 0x102f50] {
        r.allow(a);
    }
    let c = r.mem.alloc(0x40);
    let rd = r.mem.alloc(0x4b00);
    let mut g = Rng(0x600d_f00d_0000_0017);
    for case in 0..60_000 {
        let has = g.next() % 8 != 0;
        let rider = TrackRider {
            w424: [4, 5, 6, 7][(g.next() % 4) as usize],
            w428: if g.next() % 4 == 0 { 0x12 } else { (g.next() % 20) as i32 },
            v150: g.v(),
            v180: [g.f(), g.f(), g.f(), g.f()],
            f1b0: g.f(),
            v4ad0: [g.f(), g.f(), g.f()],
        };
        let mut dir = [g.f(), g.f(), g.f()];
        match g.next() % 8 {
            0 => dir[0] = 0.0,
            1 => {
                dir[0] = 0.0;
                dir[1] = 0.0;
            }
            2 => dir = [0.3 * (g.next() % 2) as f32, 0.2, 0.1],
            _ => {}
        }
        let mut t = Tracker { speed: g.f(), dir, marker: [g.f(), g.f(), g.f()] };
        let force = (g.next() % 2) as i64;
        r.mem.write_u32(c, if has { rd } else { 0 });
        r.mem.write_f32(c + 4, t.speed);
        for i in 0..3 {
            r.mem.write_f32(c + 8 + 4 * i, t.dir[i as usize]);
            r.mem.write_f32(c + 0x14 + 4 * i, t.marker[i as usize]);
        }
        r.mem.write_u32(rd + 0x424, rider.w424 as u32);
        r.mem.write_u32(rd + 0x428, rider.w428 as u32);
        r.mem.write_f32(rd + 0x1b0, rider.f1b0);
        for i in 0..4u32 {
            r.mem.write_f32(rd + 0x150 + 4 * i, rider.v150[i as usize]);
            r.mem.write_f32(rd + 0x180 + 4 * i, rider.v180[i as usize]);
            r.mem.write_f32(rd + 0x4ad0 + 4 * i, if i < 3 { rider.v4ad0[i as usize] } else { 1.0 });
        }
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, force as u64);
        r.call(0x173270).unwrap();
        track_rider(&mut t, if has { Some(&rider) } else { None }, force);
        let want: Vec<u32> = (1..5).map(|k| r.mem.read_u32(c + 4 * k)).collect();
        let got: Vec<u32> = [t.speed.to_bits()].into_iter().chain(t.dir.iter().map(|x| x.to_bits())).collect();
        assert_eq!(got, want, "case {case} speed+dir");
        let m: Vec<u32> = (0..3).map(|k| r.mem.read_u32(c + 0x14 + 4 * k)).collect();
        let gm: Vec<u32> = t.marker.iter().map(|x| x.to_bits()).collect();
        assert_eq!(gm, m, "case {case} marker");
    }
}

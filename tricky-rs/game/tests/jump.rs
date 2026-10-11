//! Our jump against the original's, bit for bit (board row G1a1): the game's own `Jump_ApplyImpulse` 0x1284e0 run
//! in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), with its two callees that
//! leave the physics (`Score_StartTrickChain` 0x156618, `Boarder_AddMeter` 0x11b018) recorded instead, and
//! `tricky_game::jump::jump_impulse` on the same rider.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::jump::{jump_impulse, JumpState};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    fn unit(&mut self, zmin: f32, zmax: f32) -> [f32; 4] {
        let (x, y, z) = (self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(zmin, zmax));
        let l = (x * x + y * y + z * z).sqrt().max(1e-3);
        [x / l, y / l, z / l, 0.0]
    }
}

fn put(r: &mut Runner, at: u32, v: [f32; 4]) {
    for (i, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * i as u32, *x);
    }
}

#[test]
fn jump_impulse_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    // every operation rounded to nearest, as in checked.rs (F4f, F4g; divide and square root assumed until F4c)
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x102f50, 0x250d60, 0x250e98] {
        r.allow(f); // Vec4_Scale, Math_Sin, Math_Cos: pure, run as the original
    }
    let steep = Rc::new(RefCell::new(None));
    let s2 = steep.clone();
    r.stub(0x156618, move |cpu, _| {
        *s2.borrow_mut() = Some(cpu.gpr(7) as u32); // the steep-lip flag, 4th argument
        cpu.set_f(0, 0.0);
    });
    r.stub(0x11b018, |_, _| {});
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    r.mem.write_u32(rd + 0x464, stats);
    let mut rng = Rng(0x00ab_cdef_1234_5678);
    let (mut n_steep, mut n_rail, mut n_plain) = (0, 0, 0);
    for case in 0..6000 {
        let s = JumpState {
            crouch: rng.f(0.0, 1.0),
            x208: if case % 4 == 0 { rng.f(0.0, 0.5) } else { 0.0 },
            vel: [rng.f(-2500.0, 2500.0), rng.f(-2500.0, 2500.0), rng.f(-600.0, 900.0), 0.0],
            jump_stat: rng.next() as u8,
            motion: if case % 7 == 0 { 3 } else { 2 },
            up: rng.unit(0.2, 1.0),
            // steep normals (z between cos 75° and cos 45°) half the time, to reach the lip branch
            normal: if case % 2 == 0 { rng.unit(0.25, 0.7) } else { rng.unit(0.6, 1.0) },
            forward: rng.unit(-0.4, 0.8),
        };
        let min = if case % 3 == 0 { f32::from_bits(0x441d_b83b) } else { rng.f(100.0, 900.0) };
        r.mem.write_f32(rd + 0x1c8, s.crouch);
        r.mem.write_f32(rd + 0x208, s.x208);
        put(&mut r, rd + 0x150, s.vel);
        r.mem.write_u8(stats + 0x17, s.jump_stat);
        r.mem.write_u32(rd + 0x424, s.motion);
        put(&mut r, rd + 0x1a0, s.up);
        put(&mut r, rd + 0x2a0, s.normal);
        put(&mut r, rd + 0x320, s.forward);
        r.mem.write_u32(rd + 0x1b4, 0);
        *steep.borrow_mut() = None;
        r.cpu.set_f(12, min);
        r.cpu.set_gpr(4, rd as u64);
        r.call(0x1284e0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let (v, lip) = jump_impulse(min, &s);
        let game: Vec<u32> = (0..4).map(|i| r.mem.read_u32(rd + 0x150 + 4 * i)).collect();
        let ours: Vec<u32> = v.iter().map(|x| x.to_bits()).collect();
        assert_eq!(game, ours, "case {case} {s:?} min {min}: game {:?} ours {v:?}", game.iter().map(|b| f32::from_bits(*b)).collect::<Vec<_>>());
        assert_eq!(*steep.borrow(), Some(lip as u32), "case {case}: steep-lip flag");
        match (s.motion, lip) {
            (3, _) => n_rail += 1,
            (_, true) => n_steep += 1,
            _ => n_plain += 1,
        }
    }
    assert!(n_steep > 300 && n_rail > 300 && n_plain > 300, "branches: steep {n_steep}, rail {n_rail}, plain {n_plain}");
}

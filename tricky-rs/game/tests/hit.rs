//! A rider hit against the original's (board row E8d1): `Boarder_RiderHit` 0x124920 run in the function runner on
//! the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with its calls answered and recorded, against
//! `tricky_game::hit::rider_hit` on the same rider: the rider's fields, the calls and the result, bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::hit::{rider_hit, HitCall, HitRider};

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
    fn unit(&mut self) -> [f32; 4] {
        let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        [v[0] / l, v[1] / l, v[2] / l, 0.0]
    }
}

const VCALL: u32 = 0x1000;
const MODE: u32 = 0x32de8c;

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}

/// A call the game made: one of ours, or one of the side calls ours stand for, by name.
#[derive(Debug, PartialEq)]
enum Rec {
    Call(HitCall),
    Other(&'static str),
}

#[test]
fn rider_hit_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50); // Vec4_Scale
    let log: Rc<RefCell<Vec<Rec>>> = Rc::default();
    let shove = Rc::new(RefCell::new(false));
    let l = log.clone();
    r.stub(0x1284e0, move |_, _| l.borrow_mut().push(Rec::Call(HitCall::Hop)));
    for (addr, name) in [(0x11f610u32, "motion"), (0x11f608, "state"), (0x124dd0, "stumble"), (0x11ea70, "bump"), (0x11eaf8, "rivals hit"), (0x234d00, "speech line"), (0x11ebf8, "knockdown"), (0x233858, "speech hit")] {
        let l = log.clone();
        r.stub(addr, move |_, _| l.borrow_mut().push(Rec::Other(name)));
    }
    let l = log.clone();
    r.stub(VCALL, move |c, _| l.borrow_mut().push(Rec::Call(HitCall::Push(c.f(12)))));
    let s = shove.clone();
    r.stub(0x15eec0, move |c, _| c.set_gpr(2, if *s.borrow() { 0x15 } else { 3 }));
    let l = log.clone();
    let s = shove.clone();
    r.stub(0x11d830, move |c, m| {
        let p = c.gpr(5) as u32;
        let at = get4(m, p);
        let vel_dir = get4(m, p + 0x10);
        let dir = get4(m, p + 0x20);
        let push = m.read_f32(p + 0x30);
        l.borrow_mut().push(Rec::Call(HitCall::Wipeout { at, vel_dir, dir, push, shove: *s.borrow() }));
    });
    let rider = r.mem.alloc(0x500);
    let other = r.mem.alloc(0x4800);
    let vt = r.mem.alloc(0x100);
    let rec = r.mem.alloc(0x20);
    let dirp = r.mem.alloc(0x10);
    r.mem.write_u32(rider, vt);
    r.mem.write_u16(vt + 0x68, 0);
    r.mem.write_u32(vt + 0x6c, VCALL);

    let mut g = Rng(0x4e1b_77c2_05a9_3d61);
    let mut seen = [0u32; 6];
    for case in 0..30_000 {
        let mut ours = HitRider {
            heading: g.f(-7.0, 7.0),
            pos: [g.f(-5000.0, 5000.0), g.f(-5000.0, 5000.0), g.f(-500.0, 500.0), 1.0],
            vel: [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-800.0, 800.0), 0.0],
            normal: if g.next() % 2 == 0 { [0.0, 0.0, 1.0, 0.0] } else { g.unit() },
            motion: [1, 2, 2, 3, 5, 6, 4, 0][g.next() as usize % 8],
            crouch: g.f(0.0, 1.0),
        };
        let speed = match g.next() % 4 {
            0 => g.f(-1500.0, 1500.0),
            1 => g.f(-200.0, 200.0),
            2 => [167.31738, 530.50287, 733.9661, 1000.0, -1000.0, 500.0, -500.0][g.next() as usize % 7],
            _ => g.f(-3000.0, 3000.0),
        };
        let dir = g.unit();
        let recv = [g.f(-200.0, 200.0), ours.heading + if g.next() % 2 == 0 { g.f(-0.9, 0.9) } else { g.f(-7.0, 7.0) }];
        let practice = g.next() % 3 == 0;
        *shove.borrow_mut() = g.next() % 2 == 0;
        let m = &mut r.mem;
        m.write_f32(rider + 0x1b0, ours.heading);
        put4(m, rider + 0x140, ours.pos);
        put4(m, rider + 0x150, ours.vel);
        put4(m, rider + 0x2a0, ours.normal);
        m.write_u32(rider + 0x424, ours.motion);
        m.write_f32(rider + 0x1c8, ours.crouch);
        m.write_f32(rec, recv[0]);
        m.write_f32(rec + 4, recv[1]);
        put4(m, dirp, dir);
        m.write_u32(MODE, if practice { 0 } else { 1 });
        log.borrow_mut().clear();
        r.cpu.set_f(12, speed);
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, other as u64);
        r.cpu.set_gpr(6, rec as u64);
        r.cpu.set_gpr(7, dirp as u64);
        r.call(0x124920).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let down = r.cpu.gpr(2) != 0;
        let mut calls = Vec::new();
        let ours_down = rider_hit(speed, dir, recv, practice, *shove.borrow(), &mut ours, &mut calls);
        let theirs = HitRider {
            heading: r.mem.read_f32(rider + 0x1b0),
            pos: get4(&mut r.mem, rider + 0x140),
            vel: get4(&mut r.mem, rider + 0x150),
            normal: get4(&mut r.mem, rider + 0x2a0),
            motion: r.mem.read_u32(rider + 0x424),
            crouch: r.mem.read_f32(rider + 0x1c8),
        };
        assert_eq!(format!("{theirs:?}"), format!("{ours:?}"), "case {case}: speed {speed} dir {dir:?} rec {recv:?}");
        assert_eq!(down, ours_down, "case {case}: knocked down");
        // our calls stand for the game's groups of calls
        let mut want = Vec::new();
        for c in &calls {
            match *c {
                HitCall::Hop => want.extend([Rec::Call(HitCall::Hop), Rec::Other("motion"), Rec::Other("state")]),
                HitCall::Push(_) => want.push(Rec::Call(*c)),
                HitCall::Stumble { shove } => want.extend([Rec::Other("stumble"), Rec::Other(if shove { "rivals hit" } else { "bump" }), Rec::Other("speech line")]),
                HitCall::Wipeout { .. } => want.extend([Rec::Call(*c), Rec::Other("knockdown"), Rec::Other("speech line"), Rec::Other("speech hit")]),
            }
        }
        assert_eq!(format!("{:?}", log.borrow()), format!("{want:?}"), "case {case}: calls (speed {speed}, motion {})", theirs.motion);
        let k = match calls.last() {
            Some(HitCall::Hop) => 0,
            Some(HitCall::Wipeout { .. }) => 1,
            Some(HitCall::Stumble { .. }) => 2,
            Some(HitCall::Push(_)) => 3,
            None => 4,
        };
        seen[k] += 1;
        seen[5] += (practice && down) as u32;
    }
    assert!(seen.iter().enumerate().all(|(k, n)| k == 4 || *n > 20), "outcomes (hop, down, stumble, push only, none, practice down): {seen:?}");
}

//! A rider hit against the original's (board row E8d1): `Boarder_RiderHit` 0x124920 run in the function runner on
//! the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with its calls answered and recorded, against
//! `tricky_game::hit::rider_hit` on the same rider: the rider's fields, the calls and the result, bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::hit::{pair_distances, rider_hit, HitCall, HitRider};

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

#[test]
fn pair_records_match_the_original() {
    // `Race_UpdatePairDistances` 0x115a80 on races of 2..8 riders at random places (some on the same x or y)
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2510f8); // Math_Atan
    let race = r.mem.alloc(0x200);
    let riders: Vec<u32> = (0..8).map(|_| r.mem.alloc(0x500)).collect();
    let mut g = Rng(0x0b5e_9d17_c2a4_6f30);
    for case in 0..3_000 {
        let n = 2 + g.next() as usize % 7;
        let mut pos = Vec::new();
        for k in 0..n {
            let mut p = [g.f(-50000.0, 50000.0), g.f(-50000.0, 50000.0), g.f(-5000.0, 5000.0), 1.0];
            if k > 0 && g.next() % 6 == 0 {
                let q: [f32; 4] = pos[g.next() as usize % k];
                if g.next() % 2 == 0 { p[0] = q[0] } else { p[1] = q[1] }
                if g.next() % 4 == 0 { p[0] = q[0]; p[1] = q[1]; }
            }
            pos.push(p);
        }
        let mut recs = vec![vec![[0.0f32; 2]; 8]; n];
        r.mem.write_u32(race + 0x88, n as u32);
        for (k, p) in pos.iter().enumerate() {
            r.mem.write_u32(race + 0xc4 + 4 * k as u32, riders[k]);
            put4(&mut r.mem, riders[k] + 0x480, *p);
            for j in 0..8u32 {
                let v = [g.f(-9.0, 9.0), g.f(-9.0, 9.0)];
                recs[k][j as usize] = v;
                r.mem.write_f32(riders[k] + 0x40 + 0x20 * j, v[0]);
                r.mem.write_f32(riders[k] + 0x44 + 0x20 * j, v[1]);
            }
        }
        r.cpu.set_gpr(5, race as u64);
        r.call(0x115a80).unwrap_or_else(|e| panic!("case {case}: {e}"));
        pair_distances(&pos, &mut recs);
        for k in 0..n {
            for j in 0..8u32 {
                let theirs = [r.mem.read_f32(riders[k] + 0x40 + 0x20 * j), r.mem.read_f32(riders[k] + 0x44 + 0x20 * j)];
                assert_eq!(theirs.map(f32::to_bits), recs[k][j as usize].map(f32::to_bits), "case {case}: rider {k} of {n}, record {j}: {theirs:?} vs {:?}", recs[k][j as usize]);
            }
        }
    }
}

#[test]
fn shoves_match_the_original() {
    // `Boarder_RiderCollisions` 0x123fc8 with no touching riders (an empty collision list), so only its shove loop
    // runs: random riders around a shover, its clip and the pair records' ticks answered; every `Boarder_RiderHit`
    // it hands out (speed, rider, direction) against `tricky_game::hit::shove`
    use tricky_game::hit::{shove, ShoveRider};
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2510f8); // Math_Atan
    r.allow(0x102f50); // Vec4_Scale
    const GAPP: u32 = 0x337c58;
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let world = r.mem.alloc(0x100);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, world);
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x5000)).collect();
    let stats: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x80)).collect();
    let st = Rc::new(RefCell::new(vec![0.0f32; 6]));
    let ids = riders.clone();
    let s = st.clone();
    r.stub(0x11d7b0, move |c, _| {
        let k = ids.iter().position(|p| *p as u64 == c.gpr(4)).unwrap();
        c.set_f(0, s.borrow()[k]);
    });
    let ids = riders.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, ids[c.gpr(5) as usize] as u64));
    let clip = Rc::new(RefCell::new((0u64, 0u64, 0u64)));
    let cl = clip.clone();
    r.stub(0x15eec0, move |c, _| c.set_gpr(2, cl.borrow().0));
    let cl = clip.clone();
    r.stub(0x15ffb8, move |c, _| c.set_gpr(2, if c.gpr(5) == 1 { cl.borrow().1 } else { cl.borrow().2 }));
    let hits: Rc<RefCell<Vec<(u32, u32, [u32; 4])>>> = Rc::default();
    let h = hits.clone();
    let ids = riders.clone();
    r.stub(0x124920, move |c, m| {
        let k = ids.iter().position(|p| *p as u64 == c.gpr(4)).unwrap() as u32;
        let d = c.gpr(7) as u32;
        h.borrow_mut().push((c.f(12).to_bits(), k, [m.read_u32(d), m.read_u32(d + 4), m.read_u32(d + 8), m.read_u32(d + 12)]));
        c.set_gpr(2, 0);
    });
    for a in [0x11e9a0u32, 0x11e988, 0x216960, 0x233cb8] {
        r.stub(a, |_, _| {});
    }
    let mut g = Rng(0x3a7c_e015_92bd_4f66);
    let mut reached = 0;
    for case in 0..30_000 {
        let n = 2 + g.next() as usize % 5;
        let me = g.next() as usize % n;
        let practice = g.next() % 3 == 0;
        r.mem.write_u32(MODE, if practice { 0 } else { 1 });
        r.mem.write_u32(world + 0x88, n as u32);
        let now = 1000 + g.next() % 1000;
        r.mem.write_u32(world + 0x18, now);
        *clip.borrow_mut() = (if g.next() % 6 == 0 { 3 } else { 0x15 }, (g.next() % 6 != 0) as u64, (g.next() % 6 == 0) as u64);
        let base = [g.f(-5000.0, 5000.0), g.f(-5000.0, 5000.0), g.f(-500.0, 500.0), 1.0];
        let mut sr = Vec::new();
        for k in 0..n {
            let o = [g.f(-250.0, 250.0), g.f(-250.0, 250.0), g.f(-200.0, 200.0), 0.0];
            let at = [base[0] + o[0], base[1] + o[1], base[2] + o[2], 1.0];
            let x = ShoveRider {
                pos: [at[0] + g.f(-20.0, 20.0), at[1] + g.f(-20.0, 20.0), at[2] + g.f(-50.0, 50.0), 1.0],
                vel: [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-500.0, 500.0), 0.0],
                fwd: g.unit(),
                side: g.unit(),
                up: if g.next() % 2 == 0 { [0.0, 0.0, 1.0, 0.0] } else { g.unit() },
                at,
                heading: g.f(-7.0, 7.0),
                speed_stat: g.f(50.0, 900.0),
                stability: g.next() as u8,
            };
            let p = riders[k];
            put4(&mut r.mem, p + 0x140, x.pos);
            put4(&mut r.mem, p + 0x150, x.vel);
            put4(&mut r.mem, p + 0x4aa0, x.fwd);
            put4(&mut r.mem, p + 0x4ab0, x.side);
            put4(&mut r.mem, p + 0x4ac0, x.up);
            put4(&mut r.mem, p + 0x4ad0, x.at);
            r.mem.write_f32(p + 0x1e0, x.heading);
            r.mem.write_u32(p + 0x460, k as u32);
            r.mem.write_u32(p + 0x464, stats[k]);
            r.mem.write_u8(stats[k] + 0x12, x.stability);
            r.mem.write_u32(p + 0x700, 0);
            st.borrow_mut()[k] = x.speed_stat;
            sr.push(x);
        }
        // the pair records' last-shove ticks: mostly long ago, some recent
        let mut ok = vec![true; n];
        for k in 0..n {
            let mine = if g.next() % 8 == 0 { now - 2 } else { now - 10 };
            let theirs = if g.next() % 8 == 0 { now - 3 } else { now - 10 };
            r.mem.write_u32(riders[me] + 0x40 + 0x20 * k as u32 + 0xc, mine);
            r.mem.write_u32(riders[k] + 0x40 + 0x20 * me as u32 + 0xc, theirs);
            ok[k] = (theirs as i32) < now as i32 - 3 && (mine as i32) <= now as i32 - 3;
        }
        hits.borrow_mut().clear();
        r.cpu.set_gpr(4, riders[me] as u64);
        r.call(0x123fc8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let (ct, f1, f2) = *clip.borrow();
        let mut want = Vec::new();
        for k in 0..n {
            if k == me || !ok[k] || ct != 0x15 || f1 == 0 || f2 != 0 {
                continue;
            }
            if let Some((speed, dir)) = shove(&sr[me], &sr[k], practice) {
                want.push((speed.to_bits(), k as u32, dir.map(f32::to_bits)));
            }
        }
        assert_eq!(*hits.borrow(), want, "case {case}: shover {me} of {n}");
        reached += want.len();
    }
    assert!(reached > 500, "shoves that reached: {reached}");
}

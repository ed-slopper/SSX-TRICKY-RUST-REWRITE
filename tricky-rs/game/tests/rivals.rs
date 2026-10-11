//! The grudge bookkeeping against the original's (board row G4a2): `Rivals_OnBump` 0x11ea70, `Rivals_OnHit`
//! 0x11eaf8, `Rivals_OnKnockdown` 0x11ebf8, `Rivals_StartGrudge` 0x11ecf8, `Rivals_EaseGrudge` 0x11ef00,
//! `Rivals_UpdateGrudgeTimeout` 0x11efd8 and `Rivals_UpdateAttitude` 0x11f050 run in the function runner on the
//! player's SLUS_203.26 (TRICKY_ELF; skipped without it) on two riders' records, against
//! `tricky_game::rivals::RivalRider`: every byte, counter and grudge field, and the speech call.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::rivals::{PairBytes, RivalCall, RivalRider};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn i8(&mut self) -> i8 {
        match self.next() % 4 {
            0 => [0, 60, 80, 81, 100, 127, -128, 45, 46, 89, 90, 91, 92, -90, -91][self.next() as usize % 15],
            _ => self.next() as i8,
        }
    }
}

const GAPP: u32 = 0x337c58;
const N: usize = 4;

fn put(m: &mut r5900::Mem, at: u32, stats: u32, r: &RivalRider) {
    m.write_u32(at + 0x460, r.index as u32);
    m.write_u32(at + 0x464, stats);
    m.write_u8(stats + 0x65, if r.human { 0 } else { 0xff });
    for (o, v) in [(0x118, r.on), (0x11c, r.since), (0x120, r.target), (0x124, r.level), (0x128, r.cooled)] {
        m.write_u32(at + o, v as u32);
    }
    for (k, p) in r.recs.iter().enumerate() {
        let b = at + 0x40 + 0x20 * k as u32;
        for (o, v) in [(0x10, p.attitude as u8), (0x11, p.cap as u8), (0x12, p.tolerance as u8), (0x13, p.anger as u8), (0x14, p.start as u8), (0x15, p.relation)] {
            m.write_u8(b + o, v);
        }
        m.write_u16(b + 0x16, p.bumps);
        m.write_u16(b + 0x18, p.shoves);
        m.write_u16(b + 0x1a, p.knocks);
        m.write_u32(b + 0x1c, p.rivalry as u32);
    }
}

fn byte(m: &mut r5900::Mem, a: u32) -> u8 {
    (m.read_u32(a & !3) >> (8 * (a & 3))) as u8
}
fn half(m: &mut r5900::Mem, a: u32) -> u16 {
    (m.read_u32(a & !3) >> (8 * (a & 2))) as u16
}

fn get(m: &mut r5900::Mem, at: u32, human: bool) -> RivalRider {
    let w = |m: &mut r5900::Mem, o: u32| m.read_u32(at + o) as i32;
    let mut r = RivalRider { index: w(m, 0x460), human, on: w(m, 0x118), since: w(m, 0x11c), target: w(m, 0x120), level: w(m, 0x124), cooled: w(m, 0x128), recs: Vec::new() };
    for k in 0..N as u32 {
        let b = at + 0x40 + 0x20 * k;
        r.recs.push(PairBytes {
            attitude: byte(m, b + 0x10) as i8,
            cap: byte(m, b + 0x11) as i8,
            tolerance: byte(m, b + 0x12) as i8,
            anger: byte(m, b + 0x13) as i8,
            start: byte(m, b + 0x14) as i8,
            relation: byte(m, b + 0x15),
            bumps: half(m, b + 0x16),
            shoves: half(m, b + 0x18),
            knocks: half(m, b + 0x1a),
            rivalry: m.read_u32(b + 0x1c) as i32,
        });
    }
    r
}

fn random_rider(g: &mut Rng, index: i32) -> RivalRider {
    let mut r = RivalRider {
        index,
        human: g.next() % 2 == 0,
        on: (g.next() % 3 != 0) as i32,
        since: (g.next() % 2000) as i32,
        target: (g.next() % N as u32) as i32,
        level: (g.next() % 3) as i32,
        cooled: (g.next() % 2) as i32,
        recs: Vec::new(),
    };
    for _ in 0..N {
        let tolerance = if g.next() % 2 == 0 { (g.next() % 100) as i8 } else { g.i8() };
        r.recs.push(PairBytes {
            attitude: g.i8(),
            cap: if g.next() % 2 == 0 { 100 } else { g.i8() },
            tolerance,
            anger: if g.next() % 3 == 0 { tolerance.wrapping_sub((g.next() % 40) as i8) } else { g.i8() },
            start: g.i8(),
            relation: (g.next() % 3) as u8,
            bumps: if g.next() % 3 == 0 { 0 } else { g.next() as u16 % 20 },
            shoves: if g.next() % 3 == 0 { 0 } else { g.next() as u16 % 20 },
            knocks: if g.next() % 3 == 0 { 0 } else { g.next() as u16 % 20 },
            rivalry: (g.next() % 20) as i32 - 5,
        });
    }
    r
}

#[test]
fn rivalry_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x11ecf8u32, 0x11ef00] {
        r.allow(f);
    }
    let speech = Rc::new(RefCell::new(0));
    let s = speech.clone();
    r.stub(0x2336b8, move |_, _| *s.borrow_mut() += 1);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let world = r.mem.alloc(0x100);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, world);
    let (pa, pb) = (r.mem.alloc(0x500), r.mem.alloc(0x500));
    let (sa, sb) = (r.mem.alloc(0x80), r.mem.alloc(0x80));
    let mut g = Rng(0x6d21_8f40_a3c7_1e95);
    let mut seen = [0u32; 8];
    for case in 0..40_000 {
        let ia = (g.next() % N as u32) as i32;
        let ib = (ia + 1 + (g.next() % (N as u32 - 1)) as i32) % N as i32;
        let mut a = random_rider(&mut g, ia);
        let mut b = random_rider(&mut g, ib);
        let now = (g.next() % 3000) as i32;
        r.mem.write_u32(world + 0x18, now as u32);
        put(&mut r.mem, pa, sa, &a);
        put(&mut r.mem, pb, sb, &b);
        *speech.borrow_mut() = 0;
        let what = g.next() % 7;
        let mut calls = Vec::new();
        let (addr, args): (u32, [u64; 2]) = match what {
            0 => { a.on_bump(&mut b, now, &mut calls); (0x11ea70, [pa as u64, pb as u64]) }
            1 => { a.on_hit(&mut b, now, &mut calls); (0x11eaf8, [pa as u64, pb as u64]) }
            2 => { a.on_knockdown(&mut b, now, &mut calls); (0x11ebf8, [pa as u64, pb as u64]) }
            3 => { a.start_grudge(&b, now, &mut calls); (0x11ecf8, [pa as u64, pb as u64]) }
            4 => { let k = (g.next() % 5) as i32 - 1; a.ease_grudge(k); (0x11ef00, [pa as u64, k as i64 as u64]) }
            5 => { a.update_grudge_timeout(now); (0x11efd8, [pa as u64, 0]) }
            _ => { a.update_attitude(b.index); (0x11f050, [pa as u64, pb as u64]) }
        };
        r.cpu.set_gpr(4, args[0]);
        r.cpu.set_gpr(5, args[1]);
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({what}): {e}"));
        let ta = get(&mut r.mem, pa, a.human);
        let tb = get(&mut r.mem, pb, b.human);
        assert_eq!(format!("{ta:?}"), format!("{a:?}"), "case {case} ({what}): the first rider");
        assert_eq!(format!("{tb:?}"), format!("{b:?}"), "case {case} ({what}): the second rider");
        assert_eq!(*speech.borrow(), calls.iter().filter(|c| **c == RivalCall::Grudge).count(), "case {case} ({what}): speech");
        seen[what as usize] += (a.on != 0 && a.level == 2) as u32;
        seen[7] += calls.len() as u32;
    }
    assert!(seen[7] > 100, "grudges with speech: {seen:?}");
}

#[test]
fn grudges_as_the_game_keeps_them() {
    use tricky_game::rivals::{index, Grudge};
    // Eddie (82 toward Moby, tolerance 16): one knock-down fills the tolerance: a level-2 grudge, rivalry +3
    let mut g = Grudge::new(index("Eddie"), index("Moby"));
    assert_eq!(g.level(), -1);
    g.hit(30.0, 1.0);
    assert_eq!((g.level(), g.score()), (2, 3), "Eddie against Moby");
    assert!(g.hostile() && g.shoves());
    // getting even (a knock-down on the player) adds 2 to the rivalry
    g.got_even(true);
    assert_eq!(g.score(), 5);
    // Kaori's cap toward Eddie is 59: knocked about all race, her grudge never passes level 0
    let mut k = Grudge::new(index("Kaori"), index("Eddie"));
    for s in 0..40 {
        k.hit(30.0, s as f32);
    }
    assert_eq!(k.level(), 0, "Kaori against Eddie");
    assert!(!k.hostile() && !k.shoves());
    // a level-1 grudge cools 360.44952 ticks (just over 6 s) after it began
    let mut m = Grudge::new(index("Moby"), index("Eddie"));
    for _ in 0..3 {
        m.hit(30.0, 10.0);
    }
    assert_eq!(m.level(), 1, "Moby (65) against Eddie");
    m.tick(16.0);
    assert!(m.hostile());
    m.tick(16.1);
    assert!(!m.hostile() && m.shoves(), "cooled but still shoving");
    // after the race with no contact: back to the start
    assert_eq!(Grudge::new(index("Mac"), index("Zoe")).settled(), 30.0);
}

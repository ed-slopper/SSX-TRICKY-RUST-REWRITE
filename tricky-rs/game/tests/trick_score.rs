//! Trick scoring against the original's, bit for bit (board row G2b): `Score_BuildTrickId`, `Score_LandTrick`,
//! `Score_StartTrickChain` and `Score_BankPoints` run in the function runner on the player's SLUS_203.26
//! (TRICKY_ELF; skipped without it), with their arithmetic run as the original (`Score_CalcPoints`,
//! `Score_CalcBasePoints`, `Score_GetMultiplier`, `Score_GetRankForPoints`, `Score_AirTimePoints`, the chain bonus,
//! `Scoring_ResetCombo`, `Score_CountRecentRepeats`) and the announcer, trick book and message calls answered and
//! recorded, against `tricky_game::score` on the same score objects in runs of several tricks (so repeats and
//! chains build up): every field, every call and the result.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::score::{Score, ScoreCall};

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
    fn i(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn b(&mut self, one_in: u32) -> u32 {
        (self.next() % one_in == 0) as u32
    }
}

const RIDER: u32 = 0x24c;

fn put(m: &mut r5900::Mem, at: u32, rider: u32, s: &Score) {
    for (o, v) in [(0x10, s.x10), (0x14, s.x14), (0x18, s.x18), (0x1c, s.x1c), (0x20, s.x20), (0xf0, s.xf0), (0xe8, s.best[0]), (0xec, s.best[1])] {
        m.write_u32(at + o, v);
    }
    for (o, v) in [(0x24, s.units), (0x28, s.mult), (0x2c, s.grab_rate), (0x50, s.x50), (0x54, s.x54), (0xc4, s.xc4), (0xc8, s.xc8), (0xcc, s.xcc), (0xd0, s.xd0), (0xd4, s.xd4), (0xd8, s.xd8)] {
        m.write_f32(at + o, v);
    }
    for (o, v) in [
        (0x30, s.spin),
        (0x34, s.flip),
        (0x38, s.grab_ticks),
        (0x3c, s.rail_ticks),
        (0x40, s.x40),
        (0x44, s.x44),
        (0x48, s.spin_scored),
        (0x4c, s.flip_scored),
        (0x58, s.x58),
        (0x5c, s.x5c),
        (0x74, s.x74),
        (0x78, s.x78),
        (0x7c, s.x7c),
        (0x80, s.x80),
        (0x84, s.x84),
        (0x88, s.x88),
        (0x8c, s.x8c),
        (0xe4, s.best_points),
        (0xf4, s.chain),
        (0x158, s.recent_at),
    ] {
        m.write_u32(at + o, v as u32);
    }
    for k in 0..4 {
        m.write_u32(at + 0x60 + 4 * k as u32, s.grabs[k]);
    }
    for k in 0..6 {
        m.write_u32(at + 0xf8 + 4 * k as u32, s.ranks[k] as u32);
    }
    for k in 0..5 {
        m.write_u32(at + 0x130 + 8 * k as u32, s.recent[k][0]);
        m.write_u32(at + 0x134 + 8 * k as u32, s.recent[k][1]);
    }
    for (k, e) in s.timed.iter().enumerate() {
        let b = at + 0x15c + 20 * k as u32;
        m.write_u32(b, e.0);
        m.write_f32(b + 8, e.1);
        m.write_f32(b + 12, e.2);
    }
    m.write_u32(at + RIDER, rider);
    m.write_u32(rider + 0x418, s.rider_418 as u32);
}

fn get(m: &mut r5900::Mem, at: u32, rider: u32) -> Score {
    let u = |m: &mut r5900::Mem, o: u32| m.read_u32(at + o);
    let i = |m: &mut r5900::Mem, o: u32| m.read_u32(at + o) as i32;
    let f = |m: &mut r5900::Mem, o: u32| m.read_f32(at + o);
    let mut s = Score {
        x10: u(m, 0x10),
        x14: u(m, 0x14),
        x18: u(m, 0x18),
        x1c: u(m, 0x1c),
        x20: u(m, 0x20),
        units: f(m, 0x24),
        mult: f(m, 0x28),
        grab_rate: f(m, 0x2c),
        spin: i(m, 0x30),
        flip: i(m, 0x34),
        grab_ticks: i(m, 0x38),
        rail_ticks: i(m, 0x3c),
        x40: i(m, 0x40),
        x44: i(m, 0x44),
        spin_scored: i(m, 0x48),
        flip_scored: i(m, 0x4c),
        x50: f(m, 0x50),
        x54: f(m, 0x54),
        x58: i(m, 0x58),
        x5c: i(m, 0x5c),
        x74: i(m, 0x74),
        x78: i(m, 0x78),
        x7c: i(m, 0x7c),
        x80: i(m, 0x80),
        x84: i(m, 0x84),
        x88: i(m, 0x88),
        x8c: i(m, 0x8c),
        xc4: f(m, 0xc4),
        xc8: f(m, 0xc8),
        xcc: f(m, 0xcc),
        xd0: f(m, 0xd0),
        xd4: f(m, 0xd4),
        xd8: f(m, 0xd8),
        best_points: i(m, 0xe4),
        best: [u(m, 0xe8), u(m, 0xec)],
        xf0: u(m, 0xf0),
        chain: i(m, 0xf4),
        recent_at: i(m, 0x158),
        rider_418: m.read_u32(rider + 0x418) as i32,
        ..Score::default()
    };
    for k in 0..4 {
        s.grabs[k] = u(m, 0x60 + 4 * k as u32);
    }
    for k in 0..6 {
        s.ranks[k] = i(m, 0xf8 + 4 * k as u32);
    }
    for k in 0..5 {
        s.recent[k] = [u(m, 0x130 + 8 * k as u32), u(m, 0x134 + 8 * k as u32)];
    }
    for k in 0..12 {
        let b = at + 0x15c + 20 * k as u32;
        s.timed[k] = (m.read_u32(b), m.read_f32(b + 8), m.read_f32(b + 12));
    }
    s
}

/// A new trick on top of whatever the score holds: spins and flips near whole half turns (some far past the
/// name table), grabs, extras.
fn new_trick(g: &mut Rng, s: &mut Score) {
    let half = |g: &mut Rng, n: i32| g.i(-n, n) * 180 + g.i(-100, 100);
    s.spin = match g.next() % 4 {
        0 => 0,
        1 => half(g, 4),
        2 => half(g, 12),
        _ => half(g, 25),
    };
    s.flip = match g.next() % 4 {
        0 => 0,
        1 => half(g, 4),
        _ => half(g, 9),
    };
    let ng = g.next() % 5;
    for k in 0..4 {
        s.grabs[k] = if (k as u32) < ng { g.i(1, 0x1ff) as u32 } else { 0 };
    }
    if g.b(6) == 1 {
        s.grabs[0] = 0;
    }
    s.units = g.f(0.0, 3.0);
    s.mult = [1.0, 1.3, 1.5, g.f(0.5, 3.0)][g.next() as usize % 4];
    s.x44 = [-1, g.i(0, 200), g.i(200, 900)][g.next() as usize % 3];
    s.rail_ticks = [-1, g.i(0, 50), g.i(50, 400)][g.next() as usize % 3];
    s.x50 = g.f(0.0, 5.0);
    s.x54 = g.f(0.0, 5.0);
    s.x58 = g.i(0, 20);
    s.x5c = g.i(0, 20);
    if g.b(3) == 1 {
        s.chain = g.i(-2, 7);
    }
    if g.b(3) == 1 {
        s.x20 = g.next() % 6;
    }
    s.x10 = g.b(3);
    s.x14 = g.b(3);
    s.x18 = g.b(5);
}

fn fresh(g: &mut Rng) -> Score {
    let mut s = Score { rider_418: if g.b(6) == 1 { g.i(0, 3) } else { -1 }, ..Score::default() };
    s.best_points = g.i(0, 20000);
    s.xd0 = g.f(0.0, 10.0);
    s.xd4 = g.f(0.0, 5.0);
    s.xd8 = g.f(0.0, 5.0);
    s.recent_at = g.i(0, 4);
    for k in 0..12 {
        s.timed[k] = ([0, 1, 0xc, 3][g.next() as usize % 4], g.f(0.0, 3.0), g.f(0.0, 3.0));
    }
    s
}

#[test]
fn trick_scoring_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x157198, 0x2cc808, 0x155440, 0x1554c0, 0x155410, 0x19e7a0, 0x156968, 0x1568b0, 0x155cc8, 0x157008, 0x157308, 0x157268, 0x157490] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<ScoreCall>>> = Rc::default();
    let l = log.clone();
    r.stub(0x157088, move |c, m| {
        let p = c.gpr(5) as u32;
        l.borrow_mut().push(ScoreCall::Announce([m.read_u32(p), m.read_u32(p + 4)], c.gpr(6) as u32));
    });
    let l = log.clone();
    r.stub(0x1579e0, move |c, m| {
        let p = c.gpr(5) as u32;
        l.borrow_mut().push(ScoreCall::TrickBook([m.read_u32(p), m.read_u32(p + 4)]));
    });
    let l = log.clone();
    r.stub(0x157330, move |c, _| {
        l.borrow_mut().push(ScoreCall::Message(c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as i32, c.gpr(7) as i32));
    });
    let focus = Rc::new(RefCell::new(false));
    let fo = focus.clone();
    r.stub(0x157c50, move |c, _| c.set_gpr(2, *fo.borrow() as u64));
    let audio = r.mem.alloc(0x7000);
    r.mem.write_u32(0x343050, audio);
    let at = r.mem.alloc(0x300);
    let rider = r.mem.alloc(0x420);

    let mut g = Rng(0x7e1c_5c0e_2b5a_9d31);
    let mut s = fresh(&mut g);
    let mut hit = [0u32; 16];
    let mut last = (0, 0, [0u32; 4]);
    for case in 0..60_000 {
        if g.b(8) == 1 {
            s = fresh(&mut g);
        }
        new_trick(&mut g, &mut s);
        if g.b(3) == 1 {
            // the same trick again: a repeat
            (s.spin, s.flip, s.grabs) = last;
            s.x18 = 0;
        }
        last = (s.spin, s.flip, s.grabs);
        put(&mut r.mem, at, rider, &s);
        r.mem.write_u32(audio + 0x6e00, 7);
        r.mem.write_u32(audio + 0x6e04, 7);
        log.borrow_mut().clear();
        let what = g.next() % 8;
        let mut ours = s;
        let (share, calls) = match what {
            0..=4 => {
                let (rail, a, b) = (g.b(4), g.b(3), g.b(3));
                r.cpu.set_gpr(4, at as u64);
                r.cpu.set_gpr(5, rail as u64);
                r.cpu.set_gpr(6, a as u64);
                r.cpu.set_gpr(7, b as u64);
                let out = ours.land_trick(rail, a, b);
                r.call(0x1569b0).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
                let id = ours.best;
                hit[0] += (s.rider_418 >= 0) as u32;
                hit[1] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce(_, 1))) as u32;
                hit[2] += out.1.iter().any(|c| matches!(c, ScoreCall::Message(_, 0xf, _, _))) as u32;
                hit[3] += out.1.iter().any(|c| matches!(c, ScoreCall::Message(_, 0x10, _, _))) as u32;
                hit[4] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce([0x2c, 0], _))) as u32;
                hit[5] += (rail != 0) as u32;
                hit[6] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce([w, _], _) if w & 0xff != 0 && *w != 0x2c)) as u32;
                hit[7] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce([w, _], _) if w & 0xff == 0 && w >> 28 != 0)) as u32;
                hit[8] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce([_, w], _) if w & 0xf00 == 0x300 || w & 0xf00 == 0x400)) as u32;
                hit[9] += out.1.iter().any(|c| matches!(c, ScoreCall::Announce([w, _], _) if (w >> 16) & 0xff == 0x65)) as u32;
                hit[10] += (id != s.best) as u32;
                out
            }
            5 | 6 => {
                let (switch, kind, a, b) = (g.b(2), g.next() % 6, g.b(3), g.b(3));
                let f = g.b(2) == 1;
                *focus.borrow_mut() = f;
                r.cpu.set_gpr(4, at as u64);
                r.cpu.set_gpr(5, switch as u64);
                r.cpu.set_gpr(6, kind as u64);
                r.cpu.set_gpr(7, a as u64);
                r.cpu.set_gpr(8, b as u64);
                let out = ours.start_trick_chain(switch, kind, a, b, f);
                r.call(0x156618).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
                if r.mem.read_u32(audio + 0x6e04) == 1 && r.mem.read_u32(audio + 0x6e00) == 0 {
                    log.borrow_mut().insert(0, ScoreCall::Focus);
                }
                hit[11] += (s.x20 != 0) as u32;
                hit[12] += (s.x20 != 0 && out.1.iter().any(|c| matches!(c, ScoreCall::Message(..)))) as u32;
                hit[13] += (ours.x1c != 0) as u32;
                out
            }
            _ => {
                r.cpu.set_gpr(4, at as u64);
                let out = ours.bank_points();
                r.call(0x155aa8).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
                hit[14] += (!out.1.is_empty()) as u32;
                out
            }
        };
        let theirs = get(&mut r.mem, at, rider);
        assert_eq!(*log.borrow(), calls, "case {case} ({what}) calls from {s:?}");
        assert_eq!(r.cpu.f(0).to_bits(), share.to_bits(), "case {case} ({what}) result from {s:?}");
        assert_eq!(format!("{theirs:?}"), format!("{ours:?}"), "case {case} ({what}) from {s:?}");
        s = ours;
    }
    println!("{hit:?}");
    for (k, n) in hit.iter().enumerate().take(15) {
        assert!(*n > 20, "branch {k} reached only {n} times: {hit:?}");
    }
}

/// `Score_FormatTrickName` 0x1551c0 (its newlib `sprintf` run as the original) against
/// `tricky_data::trickdef::TrickDef::name`: random ids and every id `build_trick_id` makes.
#[test]
fn trick_names_match_the_original() {
    use tricky_data::trickdef::TrickDef;
    let Some(mut r) = Runner::from_env() else { return };
    for f in ALLOW_SPRINTF {
        r.allow(f);
    }
    // strlen 0x2fd260 (its 128-bit MMI loop is not in the runner): the same length
    r.stub(0x2fd260, |c, m| {
        let s = c.gpr(4) as u32;
        let mut n = 0;
        while (m.read_u32((s + n) & !3) >> (8 * ((s + n) & 3))) as u8 != 0 {
            n += 1;
        }
        c.set_gpr(2, n as u64);
    });
    let buf = r.mem.alloc(0x200);
    let idp = r.mem.alloc(8);
    let mut g = Rng(0x51d3_77a0_c4e1_0b29);
    for case in 0..20_000 {
        let words = if case % 2 == 0 {
            let mut s = fresh(&mut g);
            new_trick(&mut g, &mut s);
            // the grabs' names stop at 0x65 (past it the table runs into other data)
            for k in &mut s.grabs {
                if *k != 0 {
                    *k = *k % 0x64 + 1;
                }
            }
            s.build_trick_id(g.b(4), g.b(3), g.b(3)).0
        } else {
            // every field in its table's range
            let w0 = g.next() % 0x2d | (g.next() % 0x66) << 8 | (g.next() % 0x66) << 16 | (g.next() % 13) << 24 | (g.next() % 11) << 28;
            let w1 = (g.next() % 5) | (g.next() % 5) << 4 | (g.next() % 5) << 8 | (g.next() % 6) << 12 | (g.next() % 5) << 16 | (g.next() % 4) << 20 | (g.next() % 4) << 24 | (g.next() % 3) << 27 | (g.next() % 2) << 30;
            [w0, w1]
        };
        r.mem.write_u32(idp, words[0]);
        r.mem.write_u32(idp + 4, words[1]);
        r.mem.write_u8(buf, 0xaa);
        r.cpu.set_gpr(4, buf as u64);
        r.cpu.set_gpr(5, idp as u64);
        r.call(0x1551c0).unwrap_or_else(|e| panic!("case {case} {words:08x?}: {e}"));
        let mut theirs = Vec::new();
        loop {
            let a = buf + theirs.len() as u32;
            let c = (r.mem.read_u32(a & !3) >> (8 * (a & 3))) as u8;
            if c == 0 {
                break;
            }
            theirs.push(c);
        }
        let def = TrickDef { words, flip: 0, spin: 0, rail: 0, plus: [0; 2], slot: [0; 2] };
        assert_eq!(String::from_utf8_lossy(&theirs), def.name(), "case {case} {words:08x?}");
    }
}

/// `sprintf` and what newlib's `_vfprintf_r` calls for `%s` (`vfprintf`, `__sinit` and the stdio set-up under it).
const ALLOW_SPRINTF: [u32; 12] = [0x1551c0, 0x2fca28, 0x2ff8e8, 0x2ff870, 0x2fa6b8, 0x2fa4f0, 0x304838, 0x3047f8, 0x304a10, 0x2ff770, 0x304398, 0x2fb364];

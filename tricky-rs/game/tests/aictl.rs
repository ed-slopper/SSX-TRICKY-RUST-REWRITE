//! The computer rider's control helpers (`tricky_game::aictl`) against the original in the function runner
//! (TRICKY_ELF; skipped without it): pad words, the object's fields, the calls made and the draws taken.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aictl::*;

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
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x250e98u32, 0x102f50] {
        r.allow(f);
    }
    Some(r)
}

#[test]
fn random_trick_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let table = r.mem.alloc(0x2d0 * 4);
    let mut g = Rng(0x1357_9bdf_2468_ace0);
    let bytes: Vec<u8> = (0..0x2d0 * 4).map(|_| g.next() as u8).collect();
    let _ = (table, bytes);
    // the real table lives in the ELF at 0x333098: read it through the runner
    let tbl: Vec<u8> = (0..0x2d0 * 3).map(|i| r.mem.read_u32(0x333098 + (i & !3)).to_le_bytes()[(i & 3) as usize]).collect();
    let rider = r.mem.alloc(0x500);
    let obj = r.mem.alloc(0x900);
    let stats = r.mem.alloc(0x100);
    r.mem.write_u32(obj + 0x464, stats);
    let _ = rider;
    let draw = Rc::new(RefCell::new(0u32));
    let d = draw.clone();
    r.stub(0x23d4d8, move |c, _| c.set_gpr(2, *d.borrow() as u64));
    for case in 0..3000 {
        let ch = g.next() % 3;
        let kind = g.next() % 3;
        *draw.borrow_mut() = g.next();
        r.mem.write_u32(stats + 0x44, ch);
        r.mem.write_u32(stats + 0x5c, kind);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x11d368).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let want = r.cpu.gpr(2) as u32;
        assert_eq!(random_trick(&tbl, ch, kind, *draw.borrow()), want, "case {case}");
    }
}

#[test]
fn gap_tests_match_the_original() {
    let Some(mut r) = runner() else { return };
    let rider = r.mem.alloc(0x500);
    let obj = r.mem.alloc(0x900);
    r.mem.write_u32(obj, rider);
    let mut g = Rng(0x7e57_0d1c_44aa_9021);
    for case in 0..20_000 {
        let idx = if g.next() % 5 == 0 { -1 } else { (g.next() % 6) as i32 };
        let rec = [g.f(-500.0, 500.0), g.f(-9.0, 9.0)];
        let (heading, min, max) = (g.f(-9.0, 9.0), g.f(-300.0, 300.0), g.f(-300.0, 300.0));
        r.mem.write_u32(rider + 0x100, idx as u32);
        if idx >= 0 {
            r.mem.write_f32(rider + 0x40 + 0x20 * idx as u32, rec[0]);
            r.mem.write_f32(rider + 0x44 + 0x20 * idx as u32, rec[1]);
        }
        r.mem.write_f32(rider + 0x370, heading);
        r.mem.write_f32(rider + 0x104, min);
        r.mem.write_f32(rider + 0x108, max);
        let ahead = (idx >= 0).then_some(rec);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x1399a8).unwrap_or_else(|e| panic!("below {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 & 1 != 0, below_gap_min(ahead, heading, min), "below {case}");
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x139a30).unwrap_or_else(|e| panic!("above {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 & 1 != 0, above_gap_max(ahead, heading, max), "above {case}");
    }
}

#[test]
fn update_skill_scale_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let rider = r.mem.alloc(0x500);
    let other = r.mem.alloc(0x500);
    let obj = r.mem.alloc(0x900);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    r.mem.write_u32(obj, rider);
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, other as u64));
    let mut g = Rng(0x0bad_cafe_f00d_1234);
    for case in 0..20_000 {
        let tick = if g.next() % 2 == 0 { 60 * (g.next() % 50) as i32 } else { g.next() as i32 % 5000 };
        let idx = if g.next() % 4 == 0 { -1 } else { (g.next() % 12) as i32 };
        let (own, ar) = (g.next() as i32 % 30, g.next() as i32 % 30);
        let (slow, fast, old) = (g.f(0.0, 3.0), g.f(0.0, 3.0), g.f(0.0, 3.0));
        r.mem.write_u32(race + 0x18, tick as u32);
        r.mem.write_u32(rider + 0x100, idx as u32);
        r.mem.write_u32(rider + 0x110, own as u32);
        r.mem.write_u32(other + 0x110, ar as u32);
        r.mem.write_f32(obj + 0x870, slow);
        r.mem.write_f32(obj + 0x874, fast);
        r.mem.write_f32(obj + 0x86c, old);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x13a220).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut s = old;
        update_skill_scale(tick, &mut s, slow, fast, (idx >= 0).then_some(ar), own);
        assert_eq!(r.mem.read_f32(obj + 0x86c).to_bits(), s.to_bits(), "case {case}");
    }
}

#[test]
fn countdown_and_idle_match_the_original() {
    let Some(mut r) = runner() else { return };
    let rider = r.mem.alloc(0x500);
    let obj = r.mem.alloc(0x900);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    let pad = r.mem.alloc(0x10);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    r.mem.write_u32(obj, rider);
    let stuck = Rc::new(RefCell::new(0u32));
    let st = stuck.clone();
    r.stub(0x139ab8, move |c, _| c.set_gpr(2, *st.borrow() as u64));
    let mut g = Rng(0x5eed_5eed_0001_0002);
    for case in 0..30_000 {
        let p = g.next();
        let skill = [1.0, g.f(0.2, 4.0)][g.next() as usize % 2];
        let time = [g.f(-90.0, 0.0), -30.0 * skill / 3.2585914, -60.0, -30.0][g.next() as usize % 4];
        let lean = [g.f(-1.0, 1.0), 0.0, 0.5][g.next() as usize % 3];
        let lean2 = [g.f(-1.0, 1.0), lean, lean][g.next() as usize % 3];
        let side = [0.0, 1.0, -1.0][g.next() as usize % 3];
        r.mem.write_u32(pad, p);
        r.mem.write_f32(obj + 0x86c, skill);
        r.mem.write_f32(race + 0x14, time);
        r.mem.write_f32(rider + 0x238, lean);
        r.mem.write_f32(rider + 0x240, lean2);
        r.mem.write_f32(obj + 0x868, side);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x138628).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let (mut op, mut os) = (p, side);
        controls_countdown(&mut op, skill, time, lean, lean2, &mut os);
        assert_eq!(r.mem.read_u32(pad), op, "pad {case}");
        assert_eq!(r.mem.read_f32(obj + 0x868).to_bits(), os.to_bits(), "side {case}");
        // idle
        let s = g.next() & 1;
        *stuck.borrow_mut() = s;
        r.mem.write_u32(pad, p);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x138b60).unwrap_or_else(|e| panic!("idle {case}: {e}"));
        let mut op = p;
        controls_idle(&mut op, s != 0);
        assert_eq!(r.mem.read_u32(pad), op, "idle {case}");
    }
}

#[test]
fn turn_around_and_air_match_the_original() {
    let Some(mut r) = runner() else { return };
    let rider = r.mem.alloc(0x5800 + 0x100);
    let obj = r.mem.alloc(0x900);
    let pad = r.mem.alloc(0x10);
    let stats = r.mem.alloc(0x100);
    r.mem.write_u32(obj, rider);
    r.mem.write_u32(rider + 0x464, stats);
    let steer = Rc::new(RefCell::new(0f32));
    let sv = steer.clone();
    r.stub(0x1372f8, move |c, _| c.set_f(0, *sv.borrow()));
    let marker: Rc<RefCell<(u32, [u32; 3])>> = Rc::default();
    let m = marker.clone();
    r.stub(0x1392a8, move |c, mem| {
        let (ret, o) = *m.borrow();
        let (a2, a3, t0) = (c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32);
        mem.write_u32(a2, o[0]);
        mem.write_u32(a3, o[1]);
        mem.write_u32(t0, o[2]);
        c.set_gpr(2, ret as u64);
    });
    let plans: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    let pl = plans.clone();
    r.stub(0x139d58, move |c, _| pl.borrow_mut().push((c.gpr(5) as u32, c.gpr(6) as u32)));
    let draws: Rc<RefCell<Vec<u32>>> = Rc::default();
    let dr = draws.clone();
    r.stub(0x23d4d8, move |c, _| c.set_gpr(2, dr.borrow_mut().remove(0) as u64));
    let mut g = Rng(0x4141_2020_9876_0abc);
    for case in 0..40_000 {
        let p = g.next();
        // turn around
        let slow = [g.f(0.0, 1.0), 0.5, 0.49][g.next() as usize % 3];
        let s = g.f(-1.5, 1.5);
        *steer.borrow_mut() = s;
        r.mem.write_u32(pad, p);
        r.mem.write_f32(rider + 0x57b8, slow);
        for o in [0x87c, 0x880, 0x878, 0x884] {
            r.mem.write_u32(obj + o, 0x77);
        }
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x138ac0).unwrap_or_else(|e| panic!("turn {case}: {e}"));
        let mut op = p;
        let t = controls_turn_around(&mut op, slow, || s);
        assert_eq!(r.mem.read_u32(pad), op, "turn pad {case}");
        assert_eq!(r.mem.read_u32(obj + 0x884), t.flag884, "884 {case}");
        assert_eq!(r.mem.read_u32(obj + 0x87c), t.word87c, "87c {case}");
        assert_eq!(r.mem.read_u32(obj + 0x880), t.word880, "880 {case}");
        assert_eq!(r.mem.read_u32(obj + 0x878), t.word878.unwrap_or(0x77), "878 {case}");
        // air
        let found = g.next() % 4 == 0;
        let kind = [0, 1, g.next()][g.next() as usize % 3];
        let (a, b) = (g.next(), g.next());
        *marker.borrow_mut() = (found as u32, [kind, a, b]);
        let tilt = [g.f(-1.0, 1.0), g.f(-0.05, 0.05), 0.05, -0.05][g.next() as usize % 4];
        let stat = [g.next() % 256, 100, 0, 16][g.next() as usize % 4];
        let kept = [0.0, g.f(-1.0, 1.0)][g.next() as usize % 2];
        let draw = g.next();
        *draws.borrow_mut() = vec![draw];
        plans.borrow_mut().clear();
        r.mem.write_u32(pad, p);
        r.mem.write_f32(rider + 0x198, tilt);
        r.mem.write_u32(stats + 0x1d, stat);
        r.mem.write_f32(obj + 0x888, kept);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x138bb8).unwrap_or_else(|e| panic!("air {case}: {e}"));
        let mut op = p;
        let mut k = kept;
        let mut calls = Vec::new();
        let mut d = vec![draw].into_iter();
        controls_air(&mut op, JumpMarker { found, kind, a, b }, tilt, stat, &mut k, &mut || d.next().expect("one draw"), &mut calls);
        assert_eq!(r.mem.read_u32(pad), op, "air pad {case}");
        assert_eq!(r.mem.read_f32(obj + 0x888).to_bits(), k.to_bits(), "kept {case}");
        let want: Vec<(u32, u32)> = calls.iter().map(|AirCall::Plan(a, b)| (*a, *b)).collect();
        assert_eq!(*plans.borrow(), want, "plans {case}");
        assert_eq!(draws.borrow().is_empty(), d.next().is_none() && true, "draws {case}");
    }
}

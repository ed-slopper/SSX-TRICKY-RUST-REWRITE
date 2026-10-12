//! `Ride_AITauntCheck` 0x103660 and `CruiseControl_RandomFlourish` 0x103b88 against `tricky_game::taunt`, run in the
//! function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with their calls answered and
//! recorded: the calls, the draws taken and the object's timer, bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::taunt::{ai_taunt, flourish, FlourishRider, GestureCall, TauntRider};

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

const GAPP: u32 = 0x337c58;

/// The game's calls as the test records them: the mask word is read when the next call happens.
fn setup(r: &mut Runner, log: &Rc<RefCell<Vec<String>>>, draws: &Rc<RefCell<Vec<u32>>>, riding: &Rc<RefCell<bool>>, rider: u32) {
    let rd = riding.clone();
    r.stub(0x15eec0, move |c, _| c.set_gpr(2, if *rd.borrow() { 8 } else { 3 }));
    let l = log.clone();
    r.stub(0x15fcf8, move |c, m| {
        let rider = c.gpr(4) as u32 - 0x46c0;
        l.borrow_mut().push(format!("mask {:#x}", m.read_u32(rider + 0x46dc)));
        l.borrow_mut().push(format!("clip {:#x}", c.gpr(5)));
    });
    let d = draws.clone();
    r.stub(0x23d4d8, move |c, _| c.set_gpr(2, d.borrow_mut().remove(0) as u64));
    r.stub(0x1814a8, |c, _| c.set_gpr(2, 0x9000 + c.gpr(5)));
    let l = log.clone();
    r.stub(0x234f68, move |c, m| {
        l.borrow_mut().push(format!("mask {:#x}", m.read_u32(rider + 0x46dc)));
        l.borrow_mut().push(format!("taunt {}", c.gpr(6) as u32 - 0x9000));
    });
}

fn layer_word(r: &mut Runner, rider: u32, layer: Option<i32>) {
    match layer {
        None => r.mem.write_u32(rider + 0x44d8, 0xffff_ffff),
        Some(l) => {
            r.mem.write_u32(rider + 0x44d8, 2);
            r.mem.write_u32(rider + 0xb50 + 2 * 0x394, l as u32);
        }
    }
}

/// our calls as the log lines the stubs write (the mask is shown with the call that follows it)
fn lines(calls: &[GestureCall]) -> Vec<String> {
    let mut v = Vec::new();
    let mut mask = 0u32;
    for c in calls {
        match *c {
            GestureCall::Mask(m) => mask = m,
            GestureCall::Taunt(i) => {
                v.push(format!("mask {mask:#x}"));
                v.push(format!("taunt {i}"));
            }
            GestureCall::Clip(k) => {
                v.push(format!("mask {mask:#x}"));
                v.push(format!("clip {k:#x}"));
            }
            GestureCall::MaskOff => mask = 0xffff_ffff,
        }
    }
    v
}

#[test]
fn ai_taunt_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let draws: Rc<RefCell<Vec<u32>>> = Rc::default();
    let riding = Rc::new(RefCell::new(true));
    let rider = r.mem.alloc(0x5000);
    let obj = r.mem.alloc(0x40);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    setup(&mut r, &log, &draws, &riding, rider);
    r.mem.write_u32(obj + 0x14, rider);
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    let mut g = Rng(0x2f6a_81c3_d450_97be);
    let mut seen = [0u32; 6];
    for case in 0..40_000 {
        let n = 2 + g.next() as usize % 6;
        let me = g.next() as usize % n;
        let mut ours = TauntRider {
            riding: g.next() % 8 != 0,
            me,
            passed: g.next() % 4 != 0,
            rival: g.next() as usize % n,
            stage: g.next() as i32 % 5 - 1,
            pairs: (0..n).map(|_| [g.f(0.0, 2000.0), g.f(-9.0, 9.0)]).collect(),
            heading: g.f(-7.0, 7.0),
            switch: g.next() % 2 == 0,
            layer: if g.next() % 5 == 0 { None } else { Some(g.next() as i32 % 40 - 5) },
            count: n,
        };
        if g.next() % 3 != 0 {
            ours.rival = if ours.rival == me { (me + 1) % n } else { ours.rival };
        }
        let mut timer = [0.0f32, 0.9, 1.0, 1.0253, 1.0254, 1.02][g.next() as usize % 6];
        *riding.borrow_mut() = ours.riding;
        let script = if g.next() % 5 == 0 { 95 } else { g.next() % 140 };
        *draws.borrow_mut() = vec![script];
        r.mem.write_f32(obj + 0xc, timer);
        r.mem.write_u32(race + 0x88, n as u32);
        r.mem.write_u32(rider + 0x460, me as u32);
        r.mem.write_u32(rider + 0x118, ours.passed as u32);
        r.mem.write_u32(rider + 0x120, ours.rival as u32);
        r.mem.write_u32(rider + 0x124, ours.stage as u32);
        for (k, p) in ours.pairs.iter().enumerate() {
            r.mem.write_f32(rider + 0x40 + 0x20 * k as u32, p[0]);
            r.mem.write_f32(rider + 0x44 + 0x20 * k as u32, p[1]);
        }
        r.mem.write_f32(rider + 0x1b0, ours.heading);
        r.mem.write_u32(rider + 0x1b4, ours.switch as u32);
        r.mem.write_u32(rider + 0x46dc, 0x1234);
        layer_word(&mut r, rider, ours.layer);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x103660).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut calls = Vec::new();
        let mut d = vec![script].into_iter();
        ai_taunt(&mut timer, &ours, &mut || d.next().expect("one draw"), &mut calls);
        let want = lines(&calls);
        assert_eq!(format!("{:?}", log.borrow()), format!("{want:?}"), "case {case}: {ours:?} timer {timer} script {script} draws left {:?}", draws.borrow());
        assert_eq!(r.mem.read_f32(obj + 0xc).to_bits(), timer.to_bits(), "case {case}: timer");
        let left = if calls.is_empty() { 0x1234 } else { 0xffff_ffff };
        assert_eq!(r.mem.read_u32(rider + 0x46dc), left, "case {case}: mask left");
        let k = match calls.iter().find_map(|c| if let GestureCall::Clip(k) = c { Some(*k) } else { None }) {
            Some(0x211) => 1,
            Some(0x212) => 2,
            Some(0x213) => 3,
            _ => 0,
        };
        seen[k] += 1;
        seen[4] += (!ours.riding) as u32;
        seen[5] += (d.next().is_none() && script != 0) as u32;
    }
    assert!(seen[1..4].iter().all(|&n| n > 100), "{seen:?}");
}

#[test]
fn flourish_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let draws: Rc<RefCell<Vec<u32>>> = Rc::default();
    let riding = Rc::new(RefCell::new(true));
    let rider = r.mem.alloc(0x5000);
    let obj = r.mem.alloc(0x40);
    setup(&mut r, &log, &draws, &riding, rider);
    r.mem.write_u32(obj + 0x14, rider);
    let mut g = Rng(0x91d3_2e7b_5a06_c4f8);
    let mut seen = [0u32; 5];
    for case in 0..40_000 {
        let ours = FlourishRider {
            riding: g.next() % 8 != 0,
            lean: [g.f(-1.2, 1.2), g.f(-0.7, 0.7), 0.5502836, -0.8000276, 0.9][g.next() as usize % 5],
            lean_rate: g.f(-1.0, 1.0),
            crouch: [g.f(0.0, 1.0), 0.2, 0.9, g.f(0.0, 0.3)][g.next() as usize % 4],
            layer: if g.next() % 5 == 0 { None } else { Some(g.next() as i32 % 40 - 5) },
        };
        *riding.borrow_mut() = ours.riding;
        let script = if g.next() % 3 == 0 { 0x60 * (g.next() % 100) } else { g.next() };
        *draws.borrow_mut() = vec![script];
        r.mem.write_f32(rider + 0x214, ours.lean);
        r.mem.write_f32(rider + 0x218, ours.lean_rate);
        r.mem.write_f32(rider + 0x208, ours.crouch);
        r.mem.write_u32(rider + 0x46dc, 0x1234);
        layer_word(&mut r, rider, ours.layer);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x103b88).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut calls = Vec::new();
        let mut d = vec![script].into_iter();
        flourish(&ours, &mut || d.next().expect("one draw"), &mut calls);
        let want = lines(&calls);
        assert_eq!(format!("{:?}", log.borrow()), format!("{want:?}"), "case {case}: {ours:?}");
        let left = if calls.is_empty() { 0x1234 } else { 0xffff_ffff };
        assert_eq!(r.mem.read_u32(rider + 0x46dc), left, "case {case}: mask left");
        let k = match calls.iter().find_map(|c| if let GestureCall::Clip(k) = c { Some(*k) } else { None }) {
            Some(0x209) => 1,
            Some(0x20a) => 2,
            Some(0x20b) => 3,
            _ => 0,
        };
        seen[k] += 1;
        seen[4] += (calls.iter().any(|c| matches!(c, GestureCall::MaskOff)) && k == 0) as u32;
    }
    assert!(seen[1..].iter().all(|&n| n > 50), "{seen:?}");
}

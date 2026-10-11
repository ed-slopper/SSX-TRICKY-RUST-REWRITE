//! Resets against the original's, bit for bit (board row G1c3): `Boarder_RequestReset` 0x118f10 (with
//! `Boarder_StartReset` 0x119be0), `ResetState_Update` 0x106a90 and `Gate2BaseControl_Update` 0x105dc0 run in the
//! function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), the calls they make recorded and
//! the queries answered, against `tricky_game::reset`.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::reset::{self, GateCall, RaceRider, Request, ResetState, Tick, TickCall};

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

type Log = Rc<RefCell<Vec<(&'static str, Vec<u32>)>>>;

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

fn runner() -> Option<(Runner, Log)> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Log = Rc::default();
    for (addr, name) in [(0x11f610u32, "motion"), (0x11f608, "state")] {
        let l = log.clone();
        r.stub(addr, move |c, _| l.borrow_mut().push((name, vec![c.gpr(4) as u32, c.gpr(5) as u32])));
    }
    Some((r, log))
}

#[test]
fn reset_requests_match_the_original() {
    let Some((mut r, log)) = runner() else { return };
    r.allow(0x119be0);
    let can = Rc::new(std::cell::Cell::new(true));
    let cn = can.clone();
    let l = log.clone();
    r.stub(0x196858, move |c, _| {
        l.borrow_mut().push(("can_reset", vec![c.f(12).to_bits(), c.gpr(4) as u32]));
        c.set_gpr(2, cn.get() as u64);
    });
    let l = log.clone();
    r.stub(0x218eb0, move |c, _| l.borrow_mut().push(("sound", vec![c.gpr(4) as u32, c.gpr(5) as u32])));
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x5b00)).collect();
    let rs = riders.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, rs[c.gpr(5) as usize] as u64));
    let (app, game, mgr, stats) = (r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(0x900), r.mem.alloc(0x100));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(0x343050, 0x5f5f);
    let mut g = Rng(0x41d7_9c2e_b053_86fa);
    let mut seen = [0u32; 4];
    for case in 0..40_000 {
        let n = 1 + g.next() as usize % 6;
        let me = g.next() as usize % n;
        let rider = riders[me];
        let mut all = Vec::new();
        for i in 0..n {
            let rr = RaceRider {
                to_finish: [g.f(-2000.0, 60000.0), g.f(9000.0, 11000.0), 10000.0][g.next() as usize % 3],
                place: (g.next() % (n as u32 + 1)) as i32,
                vel: [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-500.0, 500.0), 0.0],
            };
            all.push(rr);
            let m = &mut r.mem;
            m.write_f32(riders[i] + 0x374, rr.to_finish);
            m.write_u32(riders[i] + 0x110, rr.place as u32);
            put4(m, riders[i] + 0x150, rr.vel);
        }
        let height = [reset::GATE_RESET_HEIGHT, g.f(0.0, 300.0)][g.next() as usize % 2];
        let q = Request {
            finish: if g.next() % 6 == 0 { (g.next() % 9000) as i32 } else { -1 },
            can_reset: g.next() % 8 != 0,
            x41c: (g.next() % 3) as i32,
            to_finish: all[me].to_finish,
            place: all[me].place,
            no_speed: g.next() % 5 == 0,
            mode: [0, 1, 2, 3, 4, 7, 2, 4][g.next() as usize % 8],
            x32de88: if g.next() % 6 == 0 { 8 } else { (g.next() % 8) as i32 },
            riders: &all,
        };
        can.set(q.can_reset);
        let m = &mut r.mem;
        m.write_u32(0x32db6c, n as u32);
        m.write_u32(0x32de8c, q.mode as u32);
        m.write_u32(0x32de88, q.x32de88 as u32);
        m.write_u32(rider + 0x418, q.finish as u32);
        m.write_u32(rider + 0x41c, q.x41c as u32);
        m.write_u32(rider + 0x5ae0, mgr);
        m.write_u32(rider + 0x464, stats);
        m.write_u8(stats + 0x65, if q.no_speed { 0x80 } else { g.next() as u8 & 0x7f });
        m.write_f32(rider + 0x344, 1234.5);
        m.write_u32(rider + 0x57a4, 0x7777);
        for o in [0x81cu32, 0x820, 0x824] {
            m.write_u32(mgr + o, 0xdead_beef);
        }
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_f(12, height);
        r.call(0x118f10).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let got = reset::request_reset(height, &q);
        let ctx = format!("case {case}: {q:?}");
        let mut want: Vec<(&'static str, Vec<u32>)> = Vec::new();
        if q.finish < 0 {
            want.push(("can_reset", vec![1234.5f32.to_bits(), 0x7777]));
        }
        let fields = (r.mem.read_u32(mgr + 0x81c), r.mem.read_u32(mgr + 0x820), r.mem.read_u32(mgr + 0x824));
        match got {
            None => assert_eq!(fields, (0xdead_beef, 0xdead_beef, 0xdead_beef), "{ctx}: fields"),
            Some(s) => {
                if s.sound {
                    want.push(("sound", vec![0x5f5f, rider]));
                }
                if q.mode == 0 && n > 1 {
                    // `Boarder_StartReset` checks again
                    want.push(("can_reset", vec![1234.5f32.to_bits(), 0x7777]));
                }
                want.push(("motion", vec![rider, 4]));
                want.push(("state", vec![rider, 0x16]));
                assert_eq!(fields, (s.state.height.to_bits(), s.state.advance.to_bits(), s.state.x14 as u32), "{ctx}: fields");
                seen[if q.mode == 0 { 1 } else if s.state.advance != 0.0 { 2 } else { 3 }] += 1;
            }
        }
        if got.is_none() {
            seen[0] += 1;
        }
        assert_eq!(*log.borrow(), want, "{ctx}: calls");
    }
    println!("refused, mean, carried speed, none: {seen:?}");
    assert!(seen.iter().all(|&s| s > 1000), "{seen:?}");
}

#[test]
fn reset_ticks_match_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let l = log.clone();
    r.stub(0x1a7848, move |c, _| l.borrow_mut().push(("hud", vec![c.gpr(4) as u32, c.gpr(5) as u32])));
    let running = Rc::new(std::cell::Cell::new((false, false)));
    let rn = running.clone();
    r.stub(0x175e50, move |c, _| c.set_gpr(2, if c.gpr(4) == 0x3368b0 { rn.get().0 } else { rn.get().1 } as u64));
    let l = log.clone();
    r.stub(0x175eb8, move |c, _| l.borrow_mut().push(("stop", vec![c.gpr(4) as u32, c.gpr(5) as u32])));
    let l = log.clone();
    r.stub(0x118c08, move |c, _| l.borrow_mut().push(("advance", vec![c.f(12).to_bits(), c.gpr(4) as u32])));
    let l = log.clone();
    r.stub(0x119220, move |c, _| l.borrow_mut().push(("place", vec![c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32])));
    let (app, game, race, own, mgr, rider) = (r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(0x100), r.mem.alloc(0x100), r.mem.alloc(0x900), r.mem.alloc(0x5b00));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x38, 0x4848);
    r.mem.write_u32(game + 0xa0, own);
    r.mem.write_u32(mgr + 0x828, rider);
    let mut g = Rng(0x9a03_5ce7_12f4_bd68);
    let mut placed = 0;
    for case in 0..40_000 {
        let mut s = ResetState { timer: g.f(-0.05, 1.0), height: g.f(0.0, 200.0), advance: g.f(0.0, 3000.0), x14: -1 };
        let t = Tick {
            cam: (g.next() % 3) as i32 - 1,
            index: (g.next() % 3) as i32,
            race: if g.next() % 4 == 0 { None } else { Some([(g.next() % 12) as i32, 1, 9, 10][g.next() as usize % 4]).map(|k| (k, (g.next() % 4 == 0) as i32)) },
            script_player: (g.next() % 3) as i32,
            shared_script: g.next() % 2 == 0,
            own_script: g.next() % 2 == 0,
        };
        // the scripts' state as the game reads it: running, or the flag
        let (sr, sf) = if t.shared_script { [(true, false), (false, true), (true, true)][g.next() as usize % 3] } else { (false, false) };
        let (or, of) = if t.own_script { [(true, false), (false, true), (true, true)][g.next() as usize % 3] } else { (false, false) };
        running.set((sr, or));
        let fade = (g.f(-1.0, 2.0), g.f(-1.0, 2.0));
        let m = &mut r.mem;
        m.write_f32(mgr + 0x818, s.timer);
        m.write_f32(mgr + 0x81c, s.height);
        m.write_f32(mgr + 0x820, s.advance);
        m.write_u32(rider + 0x38, t.cam as u32);
        m.write_u32(rider + 0x460, t.index as u32);
        m.write_f32(rider + 0x448, fade.0);
        m.write_f32(rider + 0x44c, fade.1);
        m.write_u32(game + 0x40, if t.race.is_some() { race } else { 0 });
        if let Some((k, x54)) = t.race {
            m.write_u32(race + 0x20, k as u32);
            m.write_u32(race + 0x54, x54 as u32);
        }
        m.write_u32(0x337770, t.script_player as u32);
        m.write_u32(0x3368ec, sf as u32);
        m.write_u32(own + 0x3c, of as u32);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, (mgr + 0x810) as u64);
        r.call(0x106a90).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let ((a, b), calls) = reset::reset_update(&mut s, &t, fade);
        let want: Vec<(&'static str, Vec<u32>)> = calls.iter().flat_map(|c| match *c {
            TickCall::ShowHud => vec![("hud", vec![0x4848, 0xc])],
            TickCall::StopSharedScript => vec![("stop", vec![0x3368b0, 1])],
            TickCall::StopOwnScript => vec![("stop", vec![own, 1])],
            TickCall::Place { advance, height } => vec![("advance", vec![advance.to_bits(), rider]), ("place", vec![height.to_bits(), rider, 0])],
        }).collect();
        let ctx = format!("case {case}: {s:?} {t:?}");
        assert_eq!(*log.borrow(), want, "{ctx}: calls");
        assert_eq!((r.mem.read_u32(mgr + 0x818), r.mem.read_u32(rider + 0x448), r.mem.read_u32(rider + 0x44c)), (s.timer.to_bits(), a.to_bits(), b.to_bits()), "{ctx}: timer, fades");
        placed += calls.iter().any(|c| matches!(c, TickCall::Place { .. })) as u32;
    }
    assert!(placed > 5000);
}

#[test]
fn gate2_control_matches_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let l = log.clone();
    r.stub(0x11d038, move |c, _| l.borrow_mut().push(("boost", vec![c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32])));
    let l = log.clone();
    r.stub(0x126948, move |c, _| l.borrow_mut().push(("steer", vec![c.f(12).to_bits(), c.gpr(4) as u32])));
    let l = log.clone();
    r.stub(0x127918, move |c, _| l.borrow_mut().push(("lean", vec![c.f(12).to_bits(), c.f(13).to_bits(), c.gpr(4) as u32])));
    let clip = Rc::new(std::cell::Cell::new(0i32));
    let cl = clip.clone();
    let l = log.clone();
    r.stub(0x15fdd0, move |c, _| {
        l.borrow_mut().push(("clip", vec![c.gpr(4) as u32, c.gpr(5) as u32]));
        c.set_gpr(2, cl.get() as u64);
    });
    let l = log.clone();
    r.stub(0x118f10, move |c, _| l.borrow_mut().push(("reset", vec![c.gpr(4) as u32, c.f(12).to_bits()])));
    let (rider, holder, pad) = (r.mem.alloc(0x5b00), r.mem.alloc(16), r.mem.alloc(16));
    r.mem.write_u32(holder, rider);
    let mut g = Rng(0x3c88_f1a4_57e2_096b);
    for case in 0..40_000 {
        let p = g.next() ^ (g.next() << 16);
        let on_ground = g.next() % 2 == 0;
        let c = if g.next() % 3 == 0 { 0x1fd } else { (g.next() % 0x300) as i32 };
        clip.set(c);
        r.mem.write_u32(pad, p);
        r.mem.write_u32(rider + 0x290, on_ground as u32);
        r.mem.write_u32(rider + 0x304, 0x55);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, holder as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x105dc0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let calls = reset::gate2_base_control(p, on_ground, c);
        let mut x304 = 0x55;
        let mut want: Vec<(&'static str, Vec<u32>)> = Vec::new();
        for k in &calls {
            match *k {
                GateCall::Boost(a, b) => want.push(("boost", vec![rider, a as u32, b as u32])),
                GateCall::SteerTarget(s) => want.push(("steer", vec![s.to_bits(), rider])),
                GateCall::Lean(a, b) => {
                    want.push(("lean", vec![a.to_bits(), b.to_bits(), rider]));
                    if p & 1 == 0 && on_ground {
                        want.push(("clip", vec![rider + 0x46c0, 0]));
                    }
                }
                GateCall::Ride => {
                    want.push(("motion", vec![rider, 2]));
                    want.push(("state", vec![rider, 3]));
                }
                GateCall::Reset(v) => {
                    x304 = v;
                    want.push(("reset", vec![rider, reset::GATE_RESET_HEIGHT.to_bits()]));
                }
            }
        }
        assert_eq!(*log.borrow(), want, "case {case}: pad {p:#010x}, ground {on_ground}, clip {c:#x}");
        assert_eq!(r.mem.read_u32(rider + 0x304), x304, "case {case}: +0x304");
    }
}

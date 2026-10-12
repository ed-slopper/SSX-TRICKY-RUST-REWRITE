//! The boarder's small helpers (`tricky_game::boarder_misc`, board row G1f) against the original in the function
//! runner (TRICKY_ELF; skipped without it), every callee answered and logged.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boarder_misc::*;

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
    fn v(&mut self, k: f32) -> [f32; 4] {
        [self.f(-k, k), self.f(-k, k), self.f(-k, k), 0.0]
    }
}

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn world(r: &mut Runner) -> (u32, u32) {
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    (game, race)
}

#[test]
fn pick_idle_clip_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (_, race) = world(&mut r);
    let rider = r.mem.alloc(0x5000);
    let stats = r.mem.alloc(0x100);
    r.mem.write_u32(rider + 0x464, stats);
    let layer0 = Rc::new(RefCell::new(0i64));
    let layer2 = Rc::new(RefCell::new(0i64));
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let (l0, l2, lg) = (layer0.clone(), layer2.clone(), log.clone());
    r.stub(0x15fdd0, move |c, _| {
        let layer = c.gpr(5);
        lg.borrow_mut().push(format!("get {layer}"));
        c.set_gpr(2, (if layer == 0 { *l0.borrow() } else { *l2.borrow() }) as u64);
    });
    let lg = log.clone();
    r.stub(0x15fcf8, move |c, _| lg.borrow_mut().push(format!("play {:#x} {} {}", c.gpr(5), c.gpr(6), c.gpr(7))));
    let mut g = Rng(0x600d_cafe_0000_0001);
    let mut seen = [0u32; 10];
    for case in 0..60_000 {
        let scale = [60.0, 400.0, 1200.0, 2500.0][g.next() as usize % 4];
        let rd = IdleRider {
            state: [1, 0, 2][g.next() as usize % 3],
            vel: g.v(scale),
            fwd: g.v(1.0),
            side: g.v(1.0),
            up: g.v(1.0),
            kind: [2, 0xb, 3, 0][g.next() as usize % 4],
        };
        let tick = if g.next() % 3 == 0 { 0x12 * (g.next() % 50) as i32 } else { g.next() as i32 % 900 };
        let c0 = [0i64, 0x1fe, 0x1fd, 0x1fc, 5][g.next() as usize % 5];
        let c2 = [0x2f0i64, 0x2f1, 0x2f4, 0x2f5, 0x2f3, 0x2f8, 0x2f6, 0x2f7, 0x2f2, 9][g.next() as usize % 10];
        *layer0.borrow_mut() = c0;
        *layer2.borrow_mut() = c2;
        r.mem.write_u32(race + 0x18, tick as u32);
        r.mem.write_u32(rider + 0x428, rd.state);
        put4(&mut r.mem, rider + 0x150, rd.vel);
        put4(&mut r.mem, rider + 0x4aa0, rd.fwd);
        put4(&mut r.mem, rider + 0x4ab0, rd.side);
        put4(&mut r.mem, rider + 0x4ac0, rd.up);
        r.mem.write_u32(stats + 0x3c, rd.kind as u32);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x11d4e8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut ours_log = Vec::new();
        let played = pick_idle_clip(&rd, tick, &mut |l| {
            ours_log.push(format!("get {l}"));
            if l == 0 { c0 } else { c2 }
        });
        if let Some(w) = played {
            ours_log.push(format!("play {w:#x} 0 5"));
            seen[(w - 0x2f0) as usize] += 1;
        }
        assert_eq!(*log.borrow(), ours_log, "case {case}: {rd:?} tick {tick}");
    }
    assert!(seen[..9].iter().all(|&n| n > 50), "{seen:?}");
}

#[test]
fn hit_checkpoint_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (_, race) = world(&mut r);
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x5900)).collect();
    r.mem.write_u32(0x343050, 0x7000);
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let rs = riders.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, rs[c.gpr(5) as usize] as u64));
    let gap = Rc::new(RefCell::new(0i32));
    let (gp, lg) = (gap.clone(), log.clone());
    r.stub(0x11f118, move |c, _| {
        lg.borrow_mut().push("gap".into());
        c.set_gpr(2, *gp.borrow() as i64 as u64)
    });
    let lg = log.clone();
    r.stub(0x155c60, move |c, _| lg.borrow_mut().push(format!("score {:08x} {}", c.f(12).to_bits(), c.gpr(5))));
    let lg = log.clone();
    r.stub(0x233ee0, move |_, _| lg.borrow_mut().push("sound".into()));
    let lg = log.clone();
    r.stub(0x23d5d0, move |c, _| lg.borrow_mut().push(format!("printf {:#x}", c.gpr(4))));
    let mut g = Rng(0x1111_2222_3333_4444);
    for case in 0..30_000 {
        let n = 2 + g.next() as usize % 5;
        let me = g.next() as usize % n;
        let mode = g.next() % 7;
        let finished = g.next() % 4 == 0;
        let counts: Vec<i32> = (0..n).map(|_| g.next() as i32 % 4).collect();
        let points = g.next() as i32 % 500 - 100;
        let clock = g.f(-100.0, 100.0);
        *gap.borrow_mut() = g.next() as i32 % 3000;
        r.mem.write_u32(0x32de8c, mode);
        r.mem.write_u32(race + 0x88, n as u32);
        r.mem.write_f32(race + 0x14, clock);
        for k in 0..n {
            r.mem.write_u32(riders[k] + 0x20, counts[k] as u32);
            r.mem.write_u32(riders[k] + 0x418, if finished && k == me { 5 } else { u32::MAX });
        }
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, riders[me] as u64);
        r.cpu.set_gpr(5, points as i64 as u64);
        r.call(0x11e6f8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut mine = counts[me];
        let others: Vec<i32> = (0..n).filter(|&k| k != me).map(|k| counts[k]).collect();
        let mut calls = Vec::new();
        let g2 = *gap.borrow();
        let add = hit_checkpoint(mode, finished, &mut mine, &others, points, &mut || g2, &mut calls);
        let mut want = Vec::new();
        for c in &calls {
            match c {
                CheckpointCall::Printf => want.push("printf 0x3661d0".to_string()),
                CheckpointCall::Score(v, f) => {
                    if *f == 0 {
                        want.push("gap".to_string());
                    }
                    want.push(format!("score {:08x} {f}", v.to_bits()));
                }
                CheckpointCall::Sound => want.push("sound".to_string()),
            }
        }
        assert_eq!(*log.borrow(), want, "case {case}: mode {mode} counts {counts:?} me {me}");
        let count_now = r.mem.read_u32(riders[me] + 0x20) as i32;
        assert_eq!(count_now, mine, "case {case}: count");
        let clock_now = r.mem.read_f32(race + 0x14);
        assert_eq!(clock_now.to_bits(), (clock + add.unwrap_or(0.0)).to_bits() , "case {case}: clock");
    }
}

#[test]
fn small_helpers_match_the_original() {
    let Some(mut r) = runner() else { return };
    let (game, _) = world(&mut r);
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let rider = r.mem.alloc(0x5c00);
    let ctrl = r.mem.alloc(0x900);
    r.mem.write_u32(rider + 0x5ae0, ctrl);
    let can = Rc::new(RefCell::new(0u64));
    let lg = log.clone();
    let cn = can.clone();
    r.stub(0x196858, move |c, _| {
        lg.borrow_mut().push(format!("can {:08x} {:#x}", c.f(12).to_bits(), c.gpr(4)));
        c.set_gpr(2, *cn.borrow());
    });
    let lg = log.clone();
    r.stub(0x11f610, move |c, _| lg.borrow_mut().push(format!("motion {}", c.gpr(5))));
    let lg = log.clone();
    r.stub(0x11f608, move |c, _| lg.borrow_mut().push(format!("state {:#x}", c.gpr(5))));
    let lg = log.clone();
    r.stub(0x1a7848, move |c, _| lg.borrow_mut().push(format!("hud {:#x} {} {}", c.gpr(4), c.gpr(5), c.gpr(6) as i64)));
    r.mem.write_u32(game + 0x38, 0x4242);
    let mut g = Rng(0x0a0b_0c0d_0e0f_1011);
    for case in 0..4000 {
        let finish = [-1i32, 0, 7][g.next() as usize % 3];
        let canr = g.next() % 2 == 0;
        *can.borrow_mut() = canr as u64;
        let (a, b, c) = (g.f(-9.0, 9.0), g.f(-9.0, 9.0), g.next());
        r.mem.write_u32(rider + 0x418, finish as u32);
        r.mem.write_u32(rider + 0x344, 0x1234);
        r.mem.write_u32(rider + 0x57a4, 0x77);
        r.mem.write_f32(rider + 0x344, 12.5);
        for o in [0x81c, 0x820, 0x824] {
            r.mem.write_u32(ctrl + o, 0xdead);
        }
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, c as u64);
        r.cpu.set_f(12, a);
        r.cpu.set_f(13, b);
        // Boarder_StartResetAt(a=f12, rider=a2?, ...): the original takes (f12, f13) floats, a1 = c, a0 = rider
        r.call(0x119c90).unwrap_or_else(|e| panic!("reset {case}: {e}"));
        let got = log.borrow().clone();
        let want = start_reset_at(finish, || canr, a, b, c);
        let mut w = Vec::new();
        if finish < 0 {
            w.push(format!("can {:08x} {:#x}", 12.5f32.to_bits(), 0x77));
        }
        if want.is_some() {
            w.push("motion 4".to_string());
            w.push("state 0x16".to_string());
        }
        assert_eq!(got, w, "reset {case}");
        if let Some((x, y, z)) = want {
            assert_eq!(r.mem.read_f32(ctrl + 0x820).to_bits(), x.to_bits());
            assert_eq!(r.mem.read_f32(ctrl + 0x81c).to_bits(), y.to_bits());
            assert_eq!(r.mem.read_u32(ctrl + 0x824), z);
        }
        // SetStartState
        let flag = g.next() % 2 == 0;
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, flag as u64);
        r.call(0x11a7d0).unwrap();
        assert_eq!(*log.borrow(), vec!["hud 0x4242 0 -1".to_string(), format!("state {:#x}", start_state(flag))], "start state {case}");
    }
}

#[test]
fn motion_update_and_emitters_match_the_original() {
    let Some(mut r) = runner() else { return };
    let (game, _) = world(&mut r);
    let rider = r.mem.alloc(0x5c00);
    let boarder = r.mem.alloc(0x6000);
    r.mem.write_u32(boarder, rider);
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    for f in [0x108378u32, 0x10a0d8, 0x10b0d0, 0x10afe0, 0x10c2f0, 0x1102c8] {
        let lg = log.clone();
        r.stub(f, move |c, _| lg.borrow_mut().push(format!("{f:#x} {:#x}", c.gpr(4) as u32 - boarder)));
    }
    for m in 0u32..9 {
        r.mem.write_u32(rider + 0x424, m);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, boarder as u64);
        r.call(0x11c770).unwrap();
        let want: Vec<String> = motion_update(m).map(|(f, o)| format!("{f:#x} {o:#x}")).into_iter().collect();
        assert_eq!(*log.borrow(), want, "motion {m}");
    }
    // InitEaseRecords: EaseRecord_Init(0.0, boarder + offset) twelve times, the boarder returned
    let lg = log.clone();
    r.stub(0x1230e0, move |c, _| lg.borrow_mut().push(format!("ease {:#x} {:08x}", c.gpr(4) as u32 - boarder, c.f(12).to_bits())));
    log.borrow_mut().clear();
    r.cpu.set_gpr(4, boarder as u64);
    r.call(0x122f10).unwrap();
    let want: Vec<String> = EASE_RECORDS.iter().map(|o| format!("ease {o:#x} 00000000")).collect();
    assert_eq!(*log.borrow(), want);
    assert_eq!(r.cpu.gpr(2) as u32, boarder);
    // Boarder_GatherWorldEmitters
    r.mem.write_u32(0x346f58, 0x5555);
    let lg = log.clone();
    r.stub(0x22b370, move |c, m| {
        let p = c.gpr(5) as u32;
        lg.borrow_mut().push(format!("emit {:#x} {} {:08x} {:08x} {:08x}", c.gpr(4), c.gpr(6), m.read_u32(p), m.read_u32(p + 4), m.read_u32(p + 8)));
    });
    let mut g = Rng(0x5a5a_a5a5_1234_4321);
    for case in 0..2000 {
        let slot = g.next() % 3;
        let n = g.next() as usize % 6;
        r.mem.write_u32(rider + 0x460, slot);
        r.mem.write_u32(game + 0x298, n as u32);
        let ps: Vec<[f32; 3]> = (0..n)
            .map(|k| {
                let p = [g.f(-900.0, 900.0), g.f(-900.0, 900.0), g.f(-900.0, 900.0)];
                for (j, x) in p.iter().enumerate() {
                    r.mem.write_f32(game + 0x90 + 0x80 * k as u32 + 0x20 + 4 * j as u32, *x);
                }
                p
            })
            .collect();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x1166f0).unwrap_or_else(|e| panic!("emitters {case}: {e}"));
        let want: Vec<String> = gather_world_emitters(slot, &ps)
            .iter()
            .map(|(i, p)| format!("emit 0x5555 {i} {:08x} {:08x} {:08x}", p[0].to_bits(), p[1].to_bits(), p[2].to_bits()))
            .collect();
        assert_eq!(*log.borrow(), want, "emitters {case}: slot {slot}");
    }
}

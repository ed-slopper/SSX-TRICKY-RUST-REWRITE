//! The finish line against the original's (board row G1c1): `Boarder_SetBrakeTarget`, `Boarder_SetBrakeCrouchInput`
//! and `FinishState_Update` run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it),
//! with the checked helpers (`Boarder_SetCrouchTarget`, `Boarder_SetSteerTarget`) and the celebrate/disappointed
//! setters run as the original and the clip, pose, motion, medal and clip queries recorded or answered, and
//! `tricky_game::finish` on the same rider: every field and call bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::finish::{self, Call, Race, Rider, State};

const GAPP: u32 = 0x337c58;

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
    fn of(&mut self, v: &[f32]) -> f32 {
        v[(self.next() as usize) % v.len()]
    }
}

struct W {
    r: Runner,
    rd: u32,
    st: u32,
    game: u32,
    race_obj: u32,
    x34_obj: u32,
    log: Rc<RefCell<Vec<Call>>>,
    answers: Rc<RefCell<(i64, bool, i64)>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x126c00, 0x126948, 0x126c78, 0x126b38, 0x11f3b8, 0x11f3f8] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let answers = Rc::new(RefCell::new((0i64, false, 0i64)));
    let l = log.clone();
    r.stub(0x15fcf8, move |cpu, _| l.borrow_mut().push(Call::Clip(cpu.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x128150, move |_, _| l.borrow_mut().push(Call::Pose));
    r.stub(0x127e80, |_, _| {});
    let l = log.clone();
    r.stub(0x11f610, move |cpu, _| l.borrow_mut().push(Call::Motion(cpu.gpr(5) as u32)));
    let a = answers.clone();
    r.stub(0x16baa8, move |cpu, _| cpu.set_gpr(2, a.borrow().0 as u64));
    let a = answers.clone();
    r.stub(0x15ff40, move |cpu, _| cpu.set_gpr(2, a.borrow().1 as u64));
    let a = answers.clone();
    r.stub(0x15eec0, move |cpu, _| cpu.set_gpr(2, a.borrow().2 as u64));
    let rd = r.mem.alloc(0x6000);
    let st = r.mem.alloc(0x20);
    r.mem.write_u32(st + 0x10, rd);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let race_obj = r.mem.alloc(0x40);
    let x34_obj = r.mem.alloc(0x80);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race_obj);
    r.mem.write_u32(game + 0x34, x34_obj);
    Some(W { r, rd, st, game, race_obj, x34_obj, log, answers })
}

impl W {
    fn put(&mut self, p: &Rider) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        for (o, v) in [(0x1fc, p.brake), (0x200, p.brake_rate), (0x204, p.brake_target), (0x208, p.crouch), (0x20c, p.crouch_rate), (0x210, p.crouch_target), (0x214, p.steer), (0x218, p.steer_rate), (0x21c, p.steer_target), (0x130, p.boost_level), (0x46d8, p.x46d8)] {
            m.write_f32(rd + o, v);
        }
        for (o, v) in [(0x428, p.control), (0x420, p.class), (0x290, p.surface), (0x110, p.place as u32), (0x418, p.x418 as u32), (0x2c, p.celebrate as u32), (0x30, p.disappointed as u32), (0x34, p.x34 as u32)] {
            m.write_u32(rd + o, v);
        }
        for (k, x) in p.vel.iter().enumerate() {
            m.write_f32(rd + 0x150 + 4 * k as u32, *x);
        }
    }
    fn get(&mut self, like: &Rider) -> Rider {
        let (m, rd) = (&mut self.r.mem, self.rd);
        let g = |m: &mut r5900::Mem, o: u32| m.read_f32(rd + o);
        Rider {
            brake: g(m, 0x1fc),
            brake_rate: g(m, 0x200),
            brake_target: g(m, 0x204),
            crouch: g(m, 0x208),
            crouch_rate: g(m, 0x20c),
            crouch_target: g(m, 0x210),
            steer: g(m, 0x214),
            steer_rate: g(m, 0x218),
            steer_target: g(m, 0x21c),
            x46d8: g(m, 0x46d8),
            vel: [g(m, 0x150), g(m, 0x154), g(m, 0x158), g(m, 0x15c)],
            celebrate: m.read_u32(rd + 0x2c) as i32,
            disappointed: m.read_u32(rd + 0x30) as i32,
            x34: m.read_u32(rd + 0x34) as i32,
            ..*like
        }
    }
}

fn bits(p: &Rider) -> Vec<u32> {
    [p.brake, p.brake_rate, p.brake_target, p.crouch, p.crouch_rate, p.crouch_target, p.steer, p.steer_rate, p.steer_target, p.x46d8]
        .iter()
        .chain(p.vel.iter())
        .map(|x| x.to_bits())
        .chain([p.celebrate as u32, p.disappointed as u32, p.x34 as u32])
        .collect()
}

fn random_rider(g: &mut Rng) -> Rider {
    Rider {
        brake: g.of(&[0.0, 1.0, -1.0, 0.5, -0.3]),
        brake_rate: g.f(0.0, 5.0),
        brake_target: g.f(-1.0, 1.0),
        crouch: g.of(&[0.0, 0.0, 0.7, 1.0]),
        crouch_rate: g.f(0.0, 5.0),
        crouch_target: g.f(0.0, 1.0),
        steer: g.of(&[0.0, 0.0, 0.4, -0.6]),
        steer_rate: g.f(0.0, 8.0),
        steer_target: g.of(&[0.0, 0.5, -0.5, 0.9]),
        boost_level: g.of(&[0.0, 0.0, 1.0]),
        control: [1, 6, 3][(g.next() % 3) as usize],
        class: g.next() % 3,
        surface: g.next() % 6,
        vel: [g.f(-2000.0, 2000.0), g.f(-2000.0, 2000.0), g.f(-200.0, 200.0), 0.0].map(|x| if g.next() % 4 == 0 { x * 0.05 } else { x }),
        place: (g.next() % 6) as i32,
        x418: (g.next() % 100000) as i32,
        x46d8: 0.25,
        celebrate: 77,
        disappointed: 88,
        x34: 99,
    }
}

#[test]
fn brake_helpers_are_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x0b4a_7e00_0000_0001);
    for case in 0..20000 {
        let p0 = random_rider(&mut g);
        let (b, c) = (g.of(&[0.0, 1.0, -1.0, 2.0, -2.0, 0.3]), g.of(&[0.0, 1.0, 0.5]));
        w.put(&p0);
        w.r.cpu.set_f(12, b);
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x126c78).unwrap();
        let mut ours = p0;
        finish::set_brake_target(b, &mut ours);
        assert_eq!(bits(&w.get(&p0)), bits(&ours), "case {case}: Boarder_SetBrakeTarget({b}) on {p0:?}");

        w.put(&p0);
        w.r.cpu.set_f(12, b);
        w.r.cpu.set_f(13, c);
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x126b38).unwrap();
        let mut ours = p0;
        finish::set_brake_crouch_input(b, c, &mut ours);
        assert_eq!(bits(&w.get(&p0)), bits(&ours), "case {case}: Boarder_SetBrakeCrouchInput({b}, {c}) on {p0:?}");
    }
}

#[test]
fn finish_state_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x0f1a_15e0_0000_0001);
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..20000 {
        let p0 = random_rider(&mut g);
        let st0 = State { phase: g.next() % 6, ticks: [0, 150, 299, 300, 301, 500][(g.next() % 6) as usize] };
        let race = Race {
            mode: [0, 2, 3, 4, 5, 6, 7, 1][(g.next() % 8) as usize],
            riders: 1 + (g.next() % 6) as i32,
            target: (g.next() % 100000) as i32,
            medal: (g.next() % 4) as i64,
            x34_flag: g.next() % 2,
            tick: (g.next() % 9000) as i32,
            clip_at_end: g.next() % 2 == 0,
            clip_type1: [8, 0, 3][(g.next() % 3) as usize],
        };
        w.put(&p0);
        w.r.mem.write_u32(w.st + 8, st0.phase);
        w.r.mem.write_u32(w.st + 0xc, st0.ticks as u32);
        w.r.mem.write_u32(0x32de8c, race.mode);
        w.r.mem.write_u32(0x32db6c, race.riders as u32);
        w.r.mem.write_u32(0x32de98, race.target as u32);
        w.r.mem.write_u32(w.race_obj + 0x18, race.tick as u32);
        w.r.mem.write_u32(w.x34_obj + 100, race.x34_flag);
        let _ = w.game;
        *w.answers.borrow_mut() = (race.medal, race.clip_at_end, race.clip_type1);
        w.log.borrow_mut().clear();
        w.r.cpu.set_gpr(4, w.st as u64);
        w.r.call(0x106668).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = w.get(&p0);
        let tst = State { phase: w.r.mem.read_u32(w.st + 8), ticks: w.r.mem.read_u32(w.st + 0xc) as i32 };
        let (mut ours, mut st) = (p0, st0);
        let calls = finish::update(&mut st, &mut ours, &race);
        assert_eq!(bits(&theirs), bits(&ours), "case {case} {st0:?} {race:?}\n rider {p0:?}\n game {theirs:?}\n ours {ours:?}");
        assert_eq!(tst, st, "case {case}: state");
        assert_eq!(*w.log.borrow(), calls, "case {case}: calls");
        seen.insert((st0.phase, st.phase, calls.len()));
    }
    assert!(seen.len() >= 8, "paths seen: {seen:?}");
}

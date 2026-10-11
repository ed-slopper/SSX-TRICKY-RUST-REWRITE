//! The boarder meter and input helpers against the original's, bit for bit (board row G1d): `Boarder_AddMeter`,
//! `Boarder_UpdateBoost`, `Boarder_SetCrouchTarget` and `PrewindState_SetWindTarget` run in the function runner on
//! the player's SLUS_203.26 (TRICKY_ELF; skipped without it), their sound and HUD calls recorded, and
//! `tricky_game::boarder` on the same rider fields.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boarder::{self, Call, Fields};

const GAPP: u32 = 0x337c58;
const VCALL: u32 = 0x00f0_0000; // where the fake vtable's +0x84 points: a stub

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
    fn pick(&mut self, v: &[f32]) -> f32 {
        v[(self.next() as usize) % v.len()]
    }
}

struct World {
    r: Runner,
    rd: u32,
    stats: u32,
    race: u32,
    log: Rc<RefCell<Vec<(Call, u32)>>>,
    tricky: Rc<RefCell<bool>>,
}

fn world() -> Option<World> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    for (addr, c) in [(VCALL, Call::MeterGained), (0x2191c0, Call::MeterFull), (0x21dce0, Call::UberTimer), (0x21c610, Call::Tricky), (0x2192a8, Call::BoostSound), (0x2197d8, Call::BoostEmpty), (0x219760, Call::BoostStop)] {
        let l = log.clone();
        r.stub(addr, move |cpu, _| l.borrow_mut().push((c, cpu.f(12).to_bits())));
    }
    let tricky = Rc::new(RefCell::new(false));
    let t = tricky.clone();
    r.stub(0x11f430, move |cpu, _| cpu.set_gpr(2, *t.borrow() as u64));
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let vt = r.mem.alloc(0x100);
    r.mem.write_u32(rd, vt);
    r.mem.write_u16(vt + 0x80, 0);
    r.mem.write_u32(vt + 0x84, VCALL);
    r.mem.write_u32(rd + 0x464, stats);
    // gApp → +0x730 the game → +0x40 the race object → +0x20 its state
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let race = r.mem.alloc(0x40);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x40, race);
    Some(World { r, rd, stats, race, log, tricky })
}

impl World {
    fn put(&mut self, f: &Fields) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        m.write_f32(rd + 0x1c, f.meter);
        m.write_f32(rd + 0x24, f.uber_timer);
        m.write_u32(rd + 0x28, f.full_count as u32);
        m.write_f32(rd + 0x130, f.boost_level);
        for (o, v) in [(0x1fc, f.x1fc), (0x208, f.crouch), (0x20c, f.crouch_rate), (0x210, f.crouch_target), (0x250, f.x250), (0x254, f.x254), (0x258, f.x258), (0x25c, f.x25c), (0x260, f.x260), (0x264, f.x264)] {
            m.write_f32(rd + o, v);
        }
    }
    fn get(&mut self) -> Fields {
        let (m, rd) = (&mut self.r.mem, self.rd);
        let g = |m: &mut r5900::Mem, o: u32| m.read_f32(rd + o);
        Fields {
            meter: g(m, 0x1c),
            uber_timer: g(m, 0x24),
            full_count: m.read_u32(rd + 0x28) as i32,
            boost_level: g(m, 0x130),
            x1fc: g(m, 0x1fc),
            crouch: g(m, 0x208),
            crouch_rate: g(m, 0x20c),
            crouch_target: g(m, 0x210),
            x250: g(m, 0x250),
            x254: g(m, 0x254),
            x258: g(m, 0x258),
            x25c: g(m, 0x25c),
            x260: g(m, 0x260),
            x264: g(m, 0x264),
        }
    }
}

fn bits(f: &Fields) -> Vec<u32> {
    [f.meter, f.uber_timer, f.boost_level, f.x1fc, f.crouch, f.crouch_rate, f.crouch_target, f.x250, f.x254, f.x258, f.x25c, f.x260, f.x264].iter().map(|x| x.to_bits()).chain([f.full_count as u32]).collect()
}

fn random_fields(g: &mut Rng) -> Fields {
    Fields {
        meter: g.pick(&[0.0, 1.0, 0.84, 0.85, 0.3336, 0.666, 0.6660, 0.9999]).max(0.0) * if g.next() % 3 == 0 { 1.0 } else { 0.0 } + if g.next() % 3 == 0 { 0.0 } else { g.f(-0.05, 1.0) },
        uber_timer: g.pick(&[0.0, -0.1, 5.0, 20.0]),
        full_count: (g.next() % 9) as i32 - 4,
        boost_level: g.pick(&[0.0, 1.0, 0.25, 0.6]),
        x1fc: g.pick(&[0.0, 0.0, 1.0]),
        crouch: g.f(0.0, 1.0),
        crouch_rate: g.f(0.0, 5.0),
        crouch_target: g.f(0.0, 1.0),
        x250: g.f(0.0, 1.0),
        x254: g.f(0.0, 3.0),
        x258: g.f(0.0, 1.0),
        x25c: g.pick(&[1.0, 0.0, -0.5, 0.5]),
        x260: g.f(0.0, 20.0),
        x264: g.f(0.0, 1.0),
    }
}

#[test]
fn add_meter_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x00ad_d3e7_e500_0001);
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..8000 {
        let mut ours = random_fields(&mut g);
        let (a, b) = (g.f(-0.2, 0.2), g.f(0.0, 1.5));
        let units = g.pick(&[a, b, 0.0, -0.1, 0.04, 1.0]);
        let stat = g.next() as u8;
        w.put(&ours);
        w.r.mem.write_u8(w.stats + 0x1d, stat);
        w.log.borrow_mut().clear();
        w.r.cpu.set_f(12, units);
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x11b018).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let (gain, calls) = boarder::add_meter(units, stat, &mut ours);
        let theirs = w.get();
        assert_eq!(bits(&theirs), bits(&ours), "case {case} units {units}: game {theirs:?} ours {ours:?}");
        let got: Vec<Call> = w.log.borrow().iter().map(|c| c.0).collect();
        assert_eq!(got, calls, "case {case}: calls");
        assert_eq!(w.log.borrow()[0].1, gain.to_bits(), "case {case}: the gain passed on");
        seen.insert(calls);
    }
    assert!(seen.len() >= 4, "call patterns seen: {seen:?}");
}

#[test]
fn update_boost_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x00b0_057e_e500_0001);
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..8000 {
        let mut ours = random_fields(&mut g);
        let (held, start, tricky) = (g.next() % 4 != 0, g.next() % 2 == 0, g.next() % 5 == 0);
        // the race object: none, or states 0..11 (sounds only for 1..9)
        let state = g.next() % 13;
        let has_race = state != 12;
        let app = w.r.mem.read_u32(GAPP);
        let app_game = w.r.mem.read_u32(app + 0x730);
        w.r.mem.write_u32(app_game + 0x40, if has_race { w.race } else { 0 });
        w.r.mem.write_u32(w.race + 0x20, state);
        let sounds = !has_race || state == 0 || 9 < state;
        *w.tricky.borrow_mut() = tricky;
        w.put(&ours);
        w.log.borrow_mut().clear();
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.cpu.set_gpr(5, held as u64);
        w.r.cpu.set_gpr(6, start as u64);
        w.r.call(0x11d038).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let calls = boarder::update_boost(held, start, sounds, tricky, &mut ours);
        let theirs = w.get();
        assert_eq!(bits(&theirs), bits(&ours), "case {case} held {held} start {start} tricky {tricky} state {state}: game {theirs:?} ours {ours:?}");
        let got: Vec<Call> = w.log.borrow().iter().map(|c| c.0).collect();
        assert_eq!(got, calls, "case {case}: calls");
        seen.insert(calls);
    }
    assert!(seen.len() >= 3, "call patterns seen: {seen:?}");
}

#[test]
fn crouch_and_wind_targets_are_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x00c0_0c40_e500_0001);
    let state = w.r.mem.alloc(0x20);
    w.r.mem.write_u32(state + 0xc, w.rd);
    let wind = w.r.mem.alloc(12);
    for case in 0..8000 {
        let mut ours = random_fields(&mut g);
        let any = g.f(-1.0, 1.0);
        let target = g.pick(&[0.0, 1.0, -0.5, any]);
        w.put(&ours);
        w.r.cpu.set_f(12, target);
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x126c00).unwrap();
        boarder::set_crouch_target(target, &mut ours);
        assert_eq!(bits(&w.get()), bits(&ours), "case {case}: Boarder_SetCrouchTarget({target})");

        let mut wv = [g.f(-1.0, 1.0), g.f(0.0, 3.0), g.f(-1.0, 1.0)];
        let (v, any_rate) = (g.f(-1.0, 1.0), g.f(0.0, 5.0));
        let rate = g.pick(&[f32::from_bits(0x4000_0846), f32::from_bits(0x4080_0846), any_rate]);
        let (flip, stat) = (g.next() % 2 == 0, g.next() as u8);
        for (k, x) in wv.iter().enumerate() {
            w.r.mem.write_f32(wind + 4 * k as u32, *x);
        }
        w.r.mem.write_u8(w.stats + 0x18, stat);
        w.r.cpu.set_f(12, v);
        w.r.cpu.set_f(13, rate);
        w.r.cpu.set_gpr(4, state as u64);
        w.r.cpu.set_gpr(5, flip as u64);
        w.r.cpu.set_gpr(6, wind as u64);
        w.r.call(0x105a60).unwrap();
        boarder::set_wind_target(v, rate, flip, stat, &mut wv);
        let theirs: Vec<u32> = (0..3).map(|k| w.r.mem.read_u32(wind + 4 * k)).collect();
        assert_eq!(theirs, wv.map(f32::to_bits).to_vec(), "case {case}: PrewindState_SetWindTarget({v}, {rate}, {flip})");
    }
}

#[test]
fn steer_target_and_tricky_are_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x05ee_7a46_e500_0001);
    for case in 0..10000 {
        let s = boarder::SteerState {
            boost_level: if g.next() % 2 == 0 { 0.0 } else { g.f(0.2, 1.0) },
            class: g.next() % 3,
            vel: [g.f(-1500.0, 1500.0), g.f(-1500.0, 1500.0), g.f(-300.0, 300.0), 0.0],
            steer: g.f(-1.0, 1.0),
            surface: g.next() % 20,
        };
        let target = g.f(-1.3, 1.3);
        let rd = w.rd;
        w.r.mem.write_f32(rd + 0x130, s.boost_level);
        w.r.mem.write_u32(rd + 0x420, s.class);
        for (k, x) in s.vel.iter().enumerate() {
            w.r.mem.write_f32(rd + 0x150 + 4 * k as u32, *x);
        }
        w.r.mem.write_f32(rd + 0x214, s.steer);
        w.r.mem.write_u32(rd + 0x290, s.surface);
        w.r.cpu.set_f(12, target);
        w.r.cpu.set_gpr(4, rd as u64);
        w.r.call(0x126948).unwrap();
        let (t, rate) = boarder::set_steer_target(target, &s);
        assert_eq!(w.r.mem.read_u32(rd + 0x21c), t.to_bits(), "case {case} {s:?} target {target}: steer target");
        assert_eq!(w.r.mem.read_u32(rd + 0x218), rate.to_bits(), "case {case}: rate");
    }
    let mut r = Runner::from_env().unwrap();
    let rd = r.mem.alloc(0x6000);
    for n in [-1, 0, 5, 6, 7, 100] {
        r.mem.write_u32(rd + 0x58a8, n as u32);
        r.cpu.set_gpr(4, rd as u64);
        r.call(0x11f430).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, boarder::is_tricky(n), "Boarder_IsTricky({n})");
    }
}

#[test]
fn wind_clip_is_the_originals() {
    let Some(mut w) = world() else { return };
    w.r.allow(0x2510f8); // Math_Atan
    let clip = Rc::new(RefCell::new(None));
    let c = clip.clone();
    w.r.stub(0x15fcf8, move |cpu, _| *c.borrow_mut() = Some(cpu.gpr(5) as u32));
    let state = w.r.mem.alloc(0x20);
    w.r.mem.write_u32(state + 0xc, w.rd);
    let mut g = Rng(0x0217_d0c1_e500_0001);
    for case in 0..20000 {
        let pick = |g: &mut Rng| match g.next() % 4 { 0 => 0.0, 1 => g.f(-1.0, 1.0), 2 => [1.0, -1.0, 0.5, -0.5][(g.next() % 4) as usize], _ => g.f(-0.05, 0.05) };
        let (spin, flip, switch, kind) = (pick(&mut g), pick(&mut g), g.next() % 2 == 0, (g.next() % 5) as usize);
        let rd = w.rd;
        w.r.mem.write_f32(rd + 0x220, spin);
        w.r.mem.write_f32(rd + 0x22c, flip);
        w.r.mem.write_u32(rd + 0x1b4, switch as u32);
        w.r.mem.write_u32(rd + 0x1dc, kind as u32);
        *clip.borrow_mut() = None;
        w.r.cpu.set_gpr(4, state as u64);
        w.r.call(0x1055f0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(*clip.borrow(), Some(boarder::wind_clip(spin, flip, switch, kind)), "case {case}: spin {spin} flip {flip} switch {switch} kind {kind}");
    }
}

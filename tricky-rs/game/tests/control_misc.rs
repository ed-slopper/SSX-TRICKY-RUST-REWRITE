//! The boarder's control dispatch and small parts (board row G1g) against the game in the function runner
//! (TRICKY_ELF; skipped without it): `Boarder_UpdateControl` (every state's update, the fakie cruise inline),
//! `cBoarder::vf10` (the event bits), `Rumble_Add` / `Rumble_Update`, and the small helpers, with the callees
//! scripted the same on both sides: fields and every call compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::control::{self, ControlEnv, EventEnv, EventRider, Rider, Rumble, RumbleEnv};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.below(100_000) as f32 / 100_000.0)
    }
    fn pick(&mut self, v: &[f32]) -> f32 {
        v[self.below(v.len() as u32) as usize]
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Clip,
    Play(i32),
    State(u32),
    Rotate(u32),
    Reset(u32),
    Run(u32),
}

#[derive(Clone, Default)]
struct World {
    clip: i32,
    log: Vec<Ev>,
}

impl ControlEnv for World {
    fn clip(&mut self, _layer: u32) -> i32 {
        self.log.push(Ev::Clip);
        self.clip
    }
    fn clip_type(&mut self, _: u32) -> i32 {
        unreachable!()
    }
    fn clip_at_end(&mut self, _: u32) -> bool {
        unreachable!()
    }
    fn layer_phase(&mut self, _: u32) -> f32 {
        unreachable!()
    }
    fn play_clip(&mut self, _r: &mut Rider, clip: i32) {
        self.log.push(Ev::Play(clip));
    }
    fn set_state(&mut self, _r: &mut Rider, s: u32) {
        self.log.push(Ev::State(s));
    }
    fn set_motion(&mut self, _: &mut Rider, _: u32) {
        unreachable!()
    }
    fn set_steer_target(&mut self, _: &mut Rider, _: f32) {
        unreachable!()
    }
    fn set_crouch_target(&mut self, _: &mut Rider, _: f32) {
        unreachable!()
    }
    fn set_brake_crouch_input(&mut self, _: &mut Rider, _: f32, _: f32) {
        unreachable!()
    }
    fn set_brake_target(&mut self, _: &mut Rider, _: f32) {
        unreachable!()
    }
    fn update_boost(&mut self, _: &mut Rider, _: u32, _: u32) {
        unreachable!()
    }
    fn update_lean_anim(&mut self, _: &mut Rider, _: f32, _: f32) {
        unreachable!()
    }
    fn rotate_about_axis(&mut self, _r: &mut Rider, angle: f32) {
        self.log.push(Ev::Rotate(angle.to_bits()));
    }
    fn request_reset(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Reset(0x42caebe0));
    }
    fn play_celebrate(&mut self, _: &mut Rider) {}
    fn play_disappointed(&mut self, _: &mut Rider) {}
    fn play_wind_up(&mut self, _: &mut Rider) {}
    fn play_takeoff(&mut self, _: &mut Rider) {}
    fn ai_taunt_check(&mut self, _: &mut Rider) {}
    fn random_flourish(&mut self, _: &mut Rider) {}
    fn stop(&mut self, _: &mut Rider) {}
    fn race_riders(&mut self) -> i32 {
        0
    }
    fn race_state(&mut self) -> i32 {
        0
    }
    fn cam_start(&mut self, _: i32, _: &str) {}
    fn run_control(&mut self, _r: &mut Rider, control: u32) {
        self.log.push(Ev::Run(control));
    }
}

/// The updates `Boarder_UpdateControl` calls, by state, and where each one's object is in the handler (words)
const UPDATES: [(u32, u32, u32); 19] = [
    (2, 0x1041b0, 0x184),
    (3, 0x102fc8, 0x18c),
    (4, 0x104030, 0x194),
    (6, 0x106668, 0x19c),
    (7, 0x105dc0, 0x1a4),
    (8, 0x105f58, 0x1a8),
    (9, 0x1062b0, 0x1b0),
    (10, 0x1046b0, 0x1b4),
    (0xb, 0x104800, 0x1b8),
    (0xc, 0x104e68, 0x1bc),
    (0xd, 0x1000e0, 0x1c0),
    (0xe, 0x1050e8, 0x1c4),
    (0xf, 0x1073e0, 0x1c8),
    (0x10, 0x100908, 0x1d0),
    (0x11, 0x105bc0, 0x1f0),
    (0x12, 0x105c60, 500),
    (0x14, 0x108198, 0x1fc),
    (0x16, 0x106a90, 0x204),
    (0, 0, 0),
];

#[test]
fn dispatch_and_fakie_cruise_are_the_original() {
    let Some(mut r) = runner() else { return };
    let w = Rc::new(RefCell::new(World::default()));
    let handler = r.mem.alloc(0x900);
    for &(state, addr, word) in &UPDATES[..18] {
        let x = w.clone();
        r.stub(addr, move |c, _| {
            assert_eq!(c.gpr(4) as u32, handler + 4 * word, "state {state:#x}'s object");
            x.borrow_mut().log.push(Ev::Run(state));
        });
    }
    let x = w.clone();
    r.stub(0x15fdd0, move |c, _| {
        x.borrow_mut().log.push(Ev::Clip);
        let v = x.borrow().clip;
        c.set_gpr(2, v as i64 as u64)
    });
    let x = w.clone();
    r.stub(0x15fcf8, move |c, _| x.borrow_mut().log.push(Ev::Play(c.gpr(5) as i32)));
    let x = w.clone();
    r.stub(0x11f608, move |c, _| x.borrow_mut().log.push(Ev::State(c.gpr(5) as u32)));
    let x = w.clone();
    r.stub(0x128e90, move |c, _| x.borrow_mut().log.push(Ev::Rotate(c.f(12).to_bits())));
    let x = w.clone();
    r.stub(0x118f10, move |c, _| x.borrow_mut().log.push(Ev::Reset(c.f(12).to_bits())));
    let rider = r.mem.alloc(0x4800);
    let entry = r.mem.alloc(0x80);
    r.mem.write_u32(rider + 0x464, entry);
    r.mem.write_u32(handler, rider);
    r.mem.write_u32(handler + 4 * 0x198, rider);
    r.mem.write_u32(handler + 4 * 0x1f8, rider);
    let pad_at = r.mem.alloc(16);
    let mut g = Rng(0xd15_9a7c_0000_0001);
    let mut seen = std::collections::BTreeMap::new();
    for case in 0..40_000 {
        let mut rd = Rider {
            vel: [g.f(-2000.0, 2000.0), g.f(-2000.0, 2000.0), g.f(-300.0, 300.0), g.pick(&[0.0, 1.0])],
            board: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-0.3, 0.3), 0.0],
            control: if g.below(2) == 0 { 5 } else { g.below(0x18) },
            controller: if g.below(4) == 0 { g.below(3) as i32 } else { -1 },
            stance: g.below(2),
            x46d4: g.next(),
            facing: g.f(-4.0, 4.0),
            goofy: g.below(2) == 0,
            surface: g.below(2),
            reset_flag: g.below(2),
            ..Default::default()
        };
        let put = |r: &mut Runner, rd: &Rider| {
            for k in 0..4 {
                r.mem.write_f32(rider + 0x150 + 4 * k, rd.vel[k as usize]);
                r.mem.write_f32(rider + 0x180 + 4 * k, rd.board[k as usize]);
            }
            r.mem.write_u32(rider + 0x428, rd.control);
            r.mem.write_u32(rider + 0x418, rd.controller as u32);
            r.mem.write_u32(rider + 0x1b4, rd.stance);
            r.mem.write_u32(rider + 0x46d4, rd.x46d4);
            r.mem.write_f32(rider + 0x46e0, rd.facing);
            r.mem.write_u32(entry + 0x40, rd.goofy as u32);
            r.mem.write_u32(rider + 0x290, rd.surface);
            r.mem.write_u32(rider + 0x304, rd.reset_flag);
        };
        put(&mut r, &rd);
        let mut ours = World { clip: [0x1ff, 0x201, 0x202, 0x202, 0x203, 0x204, 0x221, 0][g.below(8) as usize], log: vec![] };
        *w.borrow_mut() = ours.clone();
        let mut pad = g.next() ^ (g.next() << 16);
        if g.below(3) == 0 {
            pad &= !(0x3f << 11);
        }
        r.mem.write_u32(pad_at, pad);
        r.cpu.set_gpr(4, handler as u64);
        r.cpu.set_gpr(5, pad_at as u64);
        r.call(0x11c9a0).unwrap();
        control::update_control(&mut ours, pad, &mut rd);
        let what = format!("case {case} control {:#x} clip {:#x} pad {pad:#x}", rd.control, ours.clip);
        assert_eq!(w.borrow().log, ours.log, "{what}: calls");
        let mut v = [0f32; 4];
        for k in 0..4 {
            v[k] = r.mem.read_f32(rider + 0x150 + 4 * k as u32);
        }
        assert_eq!(v.map(f32::to_bits), rd.vel.map(f32::to_bits), "{what}: velocity");
        assert_eq!(
            (r.mem.read_u32(rider + 0x1b4), r.mem.read_u32(rider + 0x46d4), r.mem.read_u32(rider + 0x46e0), r.mem.read_u32(rider + 0x304)),
            (rd.stance, rd.x46d4, rd.facing.to_bits(), rd.reset_flag),
            "{what}: stance, facing, reset flag"
        );
        for e in &ours.log {
            *seen.entry(format!("{e:?}")).or_insert(0u32) += 1;
        }
    }
    eprintln!("{seen:?}");
    for k in ["Play(513)", "Play(515)", "Play(516)", "State(3)", "Run(16)", "Run(22)"] {
        assert!(seen.contains_key(k), "never: {k}");
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ee {
    LessonEvent(u32),
    LessonEnd,
    Finish,
    Mult,
    OnCheckpoint,
    HitCheckpoint(u32),
    Placing(i32, i32),
    IsLead(i32),
    Music(u32),
    Cheer,
    Local(u32),
    Local2,
    Listener,
    Sound(u32, bool),
    Queue(u32),
    StopSound,
}

#[derive(Clone, Default)]
struct EvWorld {
    lesson: bool,
    lead: bool,
    log: Vec<Ee>,
}

impl EventEnv for EvWorld {
    fn lesson(&mut self) -> bool {
        self.lesson
    }
    fn lesson_event(&mut self, arg: u32) {
        self.log.push(Ee::LessonEvent(arg));
    }
    fn lesson_end(&mut self) {
        self.log.push(Ee::LessonEnd);
    }
    fn finish(&mut self, _: &mut EventRider) {
        self.log.push(Ee::Finish);
    }
    fn msg_multiplier(&mut self, _: &mut EventRider) {
        self.log.push(Ee::Mult);
    }
    fn on_checkpoint(&mut self, _: &mut EventRider) {
        self.log.push(Ee::OnCheckpoint);
    }
    fn hit_checkpoint(&mut self, _: &mut EventRider, arg: u32) {
        self.log.push(Ee::HitCheckpoint(arg));
    }
    fn placing(&mut self, _: &mut EventRider, before: i32, now: i32) {
        self.log.push(Ee::Placing(before, now));
    }
    fn is_lead(&mut self, slot: i32) -> bool {
        self.log.push(Ee::IsLead(slot));
        self.lead
    }
    fn music_event(&mut self, e: u32) {
        self.log.push(Ee::Music(e));
    }
    fn cheer(&mut self, _: &mut EventRider) {
        self.log.push(Ee::Cheer);
    }
    fn local_line(&mut self, _: &mut EventRider, k: u32) {
        self.log.push(Ee::Local(k));
    }
    fn local_line2(&mut self, _: &mut EventRider) {
        self.log.push(Ee::Local2);
    }
    fn listener_line(&mut self, _: &mut EventRider) {
        self.log.push(Ee::Listener);
    }
    fn event_sound(&mut self, _: &mut EventRider, event: [u32; 2], on: bool) {
        self.log.push(Ee::Sound(event[0], on));
    }
    fn queue_event_sound(&mut self, kind: u32) {
        self.log.push(Ee::Queue(kind));
    }
    fn stop_event_sound(&mut self) {
        self.log.push(Ee::StopSound);
    }
}

#[test]
fn event_bits_are_the_original() {
    let Some(mut r) = runner() else { return };
    let w = Rc::new(RefCell::new(EvWorld::default()));
    let rider = r.mem.alloc(0x5a00);
    let ev = r.mem.alloc(8);
    macro_rules! s {
        ($addr:expr, |$c:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, _| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    s!(0x14d928, |c, x| x.log.push(Ee::LessonEvent(c.gpr(5) as u32)));
    s!(0x14d930, |_c, x| x.log.push(Ee::LessonEnd));
    s!(0x11da40, |_c, x| x.log.push(Ee::Finish));
    s!(0x155ca0, |_c, x| x.log.push(Ee::Mult));
    s!(0x233ee0, |_c, x| x.log.push(Ee::OnCheckpoint));
    s!(0x11e6f8, |c, x| x.log.push(Ee::HitCheckpoint(c.gpr(5) as u32)));
    s!(0x233ba0, |c, x| x.log.push(Ee::Placing(c.gpr(6) as i32, c.gpr(7) as i32)));
    s!(0x17f1f0, |c, x| {
        x.log.push(Ee::IsLead(c.gpr(5) as i32));
        let v = x.lead;
        c.set_gpr(2, v as u64)
    });
    s!(0x225318, |c, x| x.log.push(Ee::Music(c.gpr(5) as u32)));
    s!(0x21e528, |_c, x| x.log.push(Ee::Cheer));
    s!(0x234558, |c, x| x.log.push(Ee::Local(c.gpr(6) as u32)));
    s!(0x234610, |_c, x| x.log.push(Ee::Local2));
    s!(0x233740, |_c, x| x.log.push(Ee::Listener));
    s!(0x21c0a0, |c, x| x.log.push(Ee::Sound(0, c.gpr(7) != 0)));
    s!(0x21d0b0, |c, x| x.log.push(Ee::Queue(c.gpr(5) as u32)));
    s!(0x21d5c8, |_c, x| x.log.push(Ee::StopSound));
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x100);
    r.mem.write_u32(app + 0x730, game);
    let mut g = Rng(0xe7e_e7e0_0000_0001);
    for case in 0..60_000 {
        let e = [1u32, 9, 10, 0xc, 0xd, 0xe, 0x10, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 3][g.below(15) as usize];
        let bit = 1u32 << e;
        let mut rd = EventRider {
            prev: (g.next() & !bit) | if g.below(2) == 0 { bit } else { 0 },
            cur: (g.next() & !bit) | if g.below(2) == 0 { bit } else { 0 },
            mult: g.below(3) as i32,
            controller: if g.below(3) == 0 { 0 } else { -1 },
            place_prev: g.below(6) as i32,
            place: g.below(6) as i32,
            slot: g.below(6) as i32,
        };
        let arg = g.next();
        let mut ours = EvWorld { lesson: g.below(6) == 0, lead: g.below(2) == 0, log: vec![] };
        *w.borrow_mut() = ours.clone();
        r.mem.write_u32(game + 0x34, if ours.lesson { game + 0x80 } else { 0 });
        for (o, v) in [(0x384u32, rd.prev), (0x388, rd.cur), (0x114, rd.mult as u32), (0x418, rd.controller as u32), (0x10c, rd.place_prev as u32), (0x110, rd.place as u32), (0x460, rd.slot as u32)] {
            r.mem.write_u32(rider + o, v);
        }
        r.mem.write_u32(ev, e);
        r.mem.write_u32(ev + 4, arg);
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, ev as u64);
        r.call(0x11a348).unwrap();
        control::event_bits(&mut ours, &mut rd, [e, arg]);
        // (the game's event sound call is logged without its event: the pointer passed is the event itself)
        let ours_log: Vec<Ee> = ours.log.iter().map(|x| if let Ee::Sound(_, on) = x { Ee::Sound(0, *on) } else { x.clone() }).collect();
        assert_eq!(w.borrow().log, ours_log, "case {case} event {e:#x}");
        assert_eq!(
            (r.mem.read_u32(rider + 0x114) as i32, r.mem.read_u32(rider + 0x10c) as i32),
            (rd.mult, rd.place_prev),
            "case {case} event {e:#x}: fields"
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Re {
    Pulse(i32),
    Motor(u32, i64),
}

#[derive(Clone, Default)]
struct RuWorld {
    held: bool,
    log: Vec<Re>,
}

impl RumbleEnv for RuWorld {
    fn held_off(&mut self) -> bool {
        self.held
    }
    fn pulse(&mut self, frames: i32) {
        self.log.push(Re::Pulse(frames));
    }
    fn motor(&mut self, strength: f32, mode: i64) {
        self.log.push(Re::Motor(strength.to_bits(), mode));
    }
}

#[test]
fn rumble_and_helpers_are_the_original() {
    let Some(mut r) = runner() else { return };
    let w = Rc::new(RefCell::new(RuWorld::default()));
    let dev = r.mem.alloc(0x100);
    let vt = r.mem.alloc(0x100);
    const PULSE: u32 = 0x01f0_0000;
    const MOTOR: u32 = 0x01f0_0010;
    r.mem.write_u32(dev + 0x38, vt);
    r.mem.write_u32(vt + 0x24, PULSE);
    r.mem.write_u32(vt + 0x2c, MOTOR);
    let x = w.clone();
    r.stub(PULSE, move |c, _| {
        assert_eq!(c.gpr(5), 1);
        x.borrow_mut().log.push(Re::Pulse(c.gpr(6) as i32))
    });
    let x = w.clone();
    r.stub(MOTOR, move |c, _| x.borrow_mut().log.push(Re::Motor(c.f(12).to_bits(), c.gpr(5) as i64)));
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x40);
    r.mem.write_u32(app + 0x730, game);
    let rum = r.mem.alloc(8);
    let blk = r.mem.alloc(0x6000);
    let (fo, bo, ent) = (r.mem.alloc(16), r.mem.alloc(0x700), r.mem.alloc(0x80));
    let mut g = Rng(0x7a3b_1e00_0000_0001);
    for case in 0..40_000 {
        let level = g.pick(&[0.0, -0.1, 0.02, 0.5, 1.0, 1.5, 2.0, 2.5]);
        let held = g.below(4);
        let has_ctl = g.below(2) == 0;
        r.mem.write_u32(game + 0x40, if has_ctl { ctl } else { 0 });
        let c20 = [0i32, 3, 9, 10, -1][held as usize % 5];
        r.mem.write_u32(ctl + 0x20, c20 as u32);
        let mut ours = RuWorld { held: has_ctl && c20 != 0 && c20 < 10, log: vec![] };
        *w.borrow_mut() = ours.clone();
        r.mem.write_u32(rum, dev);
        r.mem.write_f32(rum + 4, level);
        let mut ru = Rumble { level };
        let what;
        if g.below(2) == 0 {
            let amount = g.pick(&[0.0, 0.5, 0.51, 0.6, 1.0, 1.3, 2.0, 0.01]);
            r.cpu.set_f(12, amount);
            r.cpu.set_gpr(4, rum as u64);
            r.call(0x150be8).unwrap();
            ru.add(&mut ours, amount);
            what = format!("case {case} add {amount} level {level}");
        } else {
            r.cpu.set_gpr(4, rum as u64);
            r.call(0x150d18).unwrap();
            ru.update(&mut ours);
            what = format!("case {case} update level {level}");
        }
        assert_eq!(w.borrow().log, ours.log, "{what}: calls");
        assert_eq!(r.mem.read_u32(rum + 4), ru.level.to_bits(), "{what}: level");
        // the helpers
        let t = g.pick(&[0.0, 1.0, 0.5, 0.99, 1.01]);
        let sc = g.pick(&[0.0, 1.0, 0.5, 0.995, 1.005]);
        r.mem.write_f32(blk + 300, sc);
        r.cpu.set_f(12, t);
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x11cff0).unwrap();
        let mut s = sc;
        control::set_time_scale_rate_limited(t, &mut s);
        assert_eq!(r.mem.read_u32(blk + 300), s.to_bits(), "case {case}: time scale {t} from {sc}");
        let (sum, count, speed) = (g.f(0.0, 5000.0), g.below(500) as f32, g.f(0.0, 3000.0));
        r.mem.write_f32(blk + 0xdc, sum);
        r.mem.write_f32(blk + 0xe0, count);
        r.cpu.set_f(12, speed);
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x155dc8).unwrap();
        let (mut s1, mut c1) = (sum, count);
        control::add_speed_sample(speed, &mut s1, &mut c1);
        assert_eq!((r.mem.read_u32(blk + 0xdc), r.mem.read_u32(blk + 0xe0)), (s1.to_bits(), c1.to_bits()), "case {case}: speed sample");
        let n = g.below(4) as i32 - 1;
        r.mem.write_u32(blk + 0x5af0, n as u32);
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x11f468).unwrap();
        let mut m = n;
        assert_eq!(r.cpu.gpr(2) as i32, control::tick_sfx_fade_in(&mut m), "case {case}: fade in");
        assert_eq!(r.mem.read_u32(blk + 0x5af0) as i32, m);
        let (ph, ticks) = (g.below(6) as i32, g.below(300) as i32);
        r.mem.write_u32(fo + 8, ph as u32);
        r.mem.write_u32(fo + 0xc, ticks as u32);
        r.cpu.set_gpr(4, fo as u64);
        r.call(0x1065c8).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, control::finish_line_done(ph, ticks), "case {case}: finish line");
        // done finishing: the controller, the control state, the finish line at the board object's +0x670
        let (ctrl, cs) = (if g.below(2) == 0 { -1 } else { 0 }, [1u32, 6, 3][g.below(3) as usize]);
        r.mem.write_u32(bo + 0x678, ph as u32);
        r.mem.write_u32(blk + 0x5ae0, bo);
        r.mem.write_u32(blk + 0x418, ctrl as u32);
        r.mem.write_u32(blk + 0x428, cs);
        r.cpu.set_gpr(4, blk as u64);
        r.allow(0x1065f0);
        r.call(0x11d9e0).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, control::is_done_finishing(ctrl, cs, ph), "case {case}: done finishing");
        // FUN_0011f488 (the layer-0 phase scripted) and Boarder_LaunchFromGate (the state change logged)
        let (x1ec, x1e8, phase) = (g.below(3) as i32 - 1, [4u32, 3][g.below(2) as usize], g.pick(&[0.0, 0.49, 0.5, 0.9]));
        r.mem.write_u32(blk + 0x1ec, x1ec as u32);
        r.mem.write_u32(blk + 0x1e8, x1e8);
        r.stub(0x15fe70, move |c, _| c.set_f(0, phase));
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x11f488).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, control::x1ec_or_half(x1ec, x1e8 as i32, &mut || phase), "case {case}: 0x11f488");
        let cs2 = [8u32, 9, 3][g.below(3) as usize];
        r.mem.write_u32(blk + 0x428, cs2);
        let st = Rc::new(RefCell::new(None));
        let st2 = st.clone();
        r.stub(0x11f608, move |c, _| *st2.borrow_mut() = Some(c.gpr(5) as u32));
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x11a7b0).unwrap();
        assert_eq!(*st.borrow(), if cs2 == 8 { Some(9) } else { None }, "case {case}: launch from gate");
        let x60 = g.f(0.0, 3.0);
        r.mem.write_u32(blk + 0x464, ent);
        r.mem.write_f32(ent + 0x60, x60);
        r.cpu.set_gpr(4, blk as u64);
        r.call(0x11ea00).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), control::start_line_offset(x60).to_bits(), "case {case}: start line");
    }
}

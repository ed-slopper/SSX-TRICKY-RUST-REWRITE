//! The control states' updates (board row G1f) against the game in the function runner (TRICKY_ELF; skipped
//! without it): `CruiseControl_Update` (and `_LookBack`), `ChangeFakieControl_Update`, `BumpControl_Update`,
//! `LandHardControl_Update`, `LandNormalControl_Update`, `NaturalAirControl_Update`, `SitAndWaitControl_Update`,
//! `GetupFromSitControl_Update` on random riders and pads, the boarder's helpers and the animation scripted the same
//! on both sides: every field of the rider and the state object, and every call with its arguments, compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::control::{self, Bump, ControlEnv, Cruise, LandNormal, NaturalAir, Rider};

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

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Clip(u32),
    ClipType(u32),
    AtEnd(u32),
    Phase(u32),
    Play(i32, u32),
    State(u32),
    Motion(u32),
    Steer(u32),
    Crouch(u32),
    BrakeCrouch(u32, u32),
    Brake(u32),
    Boost(u32, u32),
    Lean(u32, u32),
    Rotate(u32),
    Reset(u32),
    Celebrate,
    Disappointed,
    WindUp,
    Takeoff,
    Taunt,
    Flourish,
    Stop,
    Cam(i32, String),
}

#[derive(Clone, Default)]
struct World {
    clips: [i32; 2],
    types: [i32; 2],
    at_end: bool,
    phase: f32,
    riders: i32,
    race_state: i32,
    log: Vec<Ev>,
}

impl ControlEnv for World {
    fn clip(&mut self, layer: u32) -> i32 {
        self.log.push(Ev::Clip(layer));
        self.clips[layer as usize]
    }
    fn clip_type(&mut self, layer: u32) -> i32 {
        self.log.push(Ev::ClipType(layer));
        self.types[layer as usize]
    }
    fn clip_at_end(&mut self, layer: u32) -> bool {
        self.log.push(Ev::AtEnd(layer));
        self.at_end
    }
    fn layer_phase(&mut self, layer: u32) -> f32 {
        self.log.push(Ev::Phase(layer));
        self.phase
    }
    fn play_clip(&mut self, r: &mut Rider, clip: i32) {
        self.log.push(Ev::Play(clip, r.filter));
    }
    fn set_state(&mut self, _r: &mut Rider, s: u32) {
        self.log.push(Ev::State(s));
    }
    fn set_motion(&mut self, _r: &mut Rider, m: u32) {
        self.log.push(Ev::Motion(m));
    }
    fn set_steer_target(&mut self, _r: &mut Rider, v: f32) {
        self.log.push(Ev::Steer(v.to_bits()));
    }
    fn set_crouch_target(&mut self, _r: &mut Rider, v: f32) {
        self.log.push(Ev::Crouch(v.to_bits()));
    }
    fn set_brake_crouch_input(&mut self, _r: &mut Rider, brake: f32, crouch: f32) {
        self.log.push(Ev::BrakeCrouch(brake.to_bits(), crouch.to_bits()));
    }
    fn set_brake_target(&mut self, _r: &mut Rider, v: f32) {
        self.log.push(Ev::Brake(v.to_bits()));
    }
    fn update_boost(&mut self, _r: &mut Rider, a: u32, b: u32) {
        self.log.push(Ev::Boost(a, b));
    }
    fn update_lean_anim(&mut self, _r: &mut Rider, a: f32, b: f32) {
        self.log.push(Ev::Lean(a.to_bits(), b.to_bits()));
    }
    fn rotate_about_axis(&mut self, _r: &mut Rider, angle: f32) {
        self.log.push(Ev::Rotate(angle.to_bits()));
    }
    fn request_reset(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Reset(0x42caebe0));
    }
    fn play_celebrate(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Celebrate);
    }
    fn play_disappointed(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Disappointed);
    }
    fn play_wind_up(&mut self, _r: &mut Rider) {
        self.log.push(Ev::WindUp);
    }
    fn play_takeoff(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Takeoff);
    }
    fn ai_taunt_check(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Taunt);
    }
    fn random_flourish(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Flourish);
    }
    fn stop(&mut self, _r: &mut Rider) {
        self.log.push(Ev::Stop);
    }
    fn race_riders(&mut self) -> i32 {
        self.riders
    }
    fn race_state(&mut self) -> i32 {
        self.race_state
    }
    fn cam_start(&mut self, index: i32, script: &str) {
        self.log.push(Ev::Cam(index, script.to_string()));
    }
}

/// The rider's fields at their offsets (words), the board object's at +0x5ae0's
fn fields(r: &Rider) -> Vec<(u32, u32)> {
    let mut v = vec![];
    for k in 0..4 {
        v.push((0x150 + 4 * k, r.vel[k as usize].to_bits()));
        v.push((0x180 + 4 * k, r.board[k as usize].to_bits()));
    }
    for (o, x) in [
        (0x1f0, r.x1f0),
        (0x1f4, r.x1f4),
        (0x1f8, r.x1f8),
        (0x1fc, r.brake),
        (0x200, r.brake_rate),
        (0x204, r.brake_target),
        (0x208, r.crouch),
        (0x20c, r.crouch_rate),
        (0x210, r.crouch_target),
        (0x214, r.steer),
        (0x218, r.steer_rate),
        (0x21c, r.steer_target),
        (0x220, r.x220),
        (0x224, r.x224),
        (0x228, r.x228),
        (0x22c, r.x22c),
        (0x230, r.x230),
        (0x234, r.x234),
        (0x268, r.lean),
        (0x26c, r.lean_rate),
        (0x270, r.lean_target),
        (0x57f4, r.air),
        (0x1b0, r.heading),
    ] {
        v.push((o, x.to_bits()));
    }
    for (o, x) in [(0x290, r.surface), (0x304, r.reset_flag), (0x418, r.controller as u32), (0x420, r.class), (0x38, r.index as u32), (0x460, r.slot as u32), (0x1b4, r.stance), (0x44d8, r.trick as u32), (0x46dc, r.filter)] {
        v.push((o, x));
    }
    for k in 0..8u32 {
        v.push((0x40 + 0x20 * k, r.dist[k as usize].to_bits()));
        v.push((0x44 + 0x20 * k, r.bearing[k as usize].to_bits()));
        v.push((0xb50 + 0x394 * k, r.trick_words[k as usize] as u32));
    }
    v
}

const BOARD_OBJ: [(u32, usize); 4] = [(0x718, 0), (0x76c, 1), (0x770, 2), (0x798, 3)];

fn board_words(r: &Rider) -> [u32; 4] {
    [r.b718, r.b76c, r.b770, r.b798]
}

fn random_rider(g: &mut Rng) -> Rider {
    let z = [0.0f32, 0.0, 0.3, -0.6, 1.0];
    let mut r = Rider {
        vel: [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-500.0, 500.0), g.pick(&[0.0, 1.0])],
        board: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-0.3, 0.3), 0.0],
        x1f0: g.f(0.0, 1.0),
        x1f4: g.f(0.0, 1.0),
        x1f8: g.f(0.0, 1.0),
        brake: g.pick(&z),
        brake_rate: g.f(0.0, 5.0),
        brake_target: g.pick(&z),
        crouch: g.pick(&[0.0, 0.2, 0.5, 0.7, 1.0, -0.1]),
        crouch_rate: g.f(0.0, 5.0),
        crouch_target: g.pick(&z),
        steer: g.pick(&[0.0, 0.1, -0.19, 0.2, -0.5, 1.0]),
        steer_rate: g.f(0.0, 5.0),
        steer_target: g.pick(&z),
        x220: g.pick(&z),
        x224: g.f(0.0, 5.0),
        x228: g.pick(&z),
        x22c: g.pick(&z),
        x230: g.f(0.0, 5.0),
        x234: g.pick(&z),
        lean: g.pick(&z),
        lean_rate: g.f(0.0, 5.0),
        lean_target: g.pick(&z),
        surface: g.below(3),
        reset_flag: g.below(2),
        controller: if g.below(4) == 0 { g.below(3) as i32 } else { -1 },
        class: g.below(4),
        air: g.pick(&[0.0, 0.35, 0.36, 2.0]),
        index: g.below(4) as i32 - 1,
        slot: g.below(6) as i32,
        heading: g.f(-3.2, 3.2),
        stance: g.below(2),
        dist: [0.0; 8],
        bearing: [0.0; 8],
        trick: g.below(9) as i32 - 1,
        trick_words: [0; 8],
        filter: g.next(),
        b718: g.below(2),
        b76c: g.next(),
        b770: g.next(),
        b798: g.next(),
    };
    for k in 0..8 {
        r.dist[k] = g.f(0.0, 3000.0);
        r.bearing[k] = g.f(-12.0, 12.0);
        r.trick_words[k] = g.below(20) as i32;
    }
    // slow riders now and then (the cruise's stop clip)
    if g.below(3) == 0 {
        r.vel = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-100.0, 100.0), 0.0];
    }
    r
}

/// A pad word with its fields filled at random (and the zero stick often)
fn random_pad(g: &mut Rng) -> u32 {
    let mut p = g.next() ^ (g.next() << 16);
    if g.below(3) == 0 {
        p &= !(0x3f << 11); // the 0x15 stick
    }
    if g.below(3) == 0 {
        p &= !(0x3f << 8); // the 0x12 / 0x18 sticks
    }
    if g.below(3) == 0 {
        p &= !(0x3f << 20);
    }
    p
}

#[test]
fn control_updates_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let w = Rc::new(RefCell::new(World::default()));
    let rider = r.mem.alloc(0x5c00);
    let board_obj = r.mem.alloc(0x800);
    let rider_vt = r.mem.alloc(0x100);
    const STOP: u32 = 0x01f0_0000;
    r.mem.write_u32(rider, rider_vt);
    r.mem.write_u32(rider_vt + 0x7c, STOP);
    r.mem.write_u32(rider + 0x5ae0, board_obj);
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x400);
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    macro_rules! s {
        ($addr:expr, |$c:ident, $m:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, $m| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    s!(0x15fdd0, |c, _m, x| {
        let l = c.gpr(5) as u32;
        x.log.push(Ev::Clip(l));
        c.set_gpr(2, x.clips[l as usize] as i64 as u64)
    });
    s!(0x15eec0, |c, _m, x| {
        let l = c.gpr(5) as u32;
        x.log.push(Ev::ClipType(l));
        c.set_gpr(2, x.types[l as usize] as i64 as u64)
    });
    s!(0x15ff40, |c, _m, x| {
        let l = c.gpr(5) as u32;
        x.log.push(Ev::AtEnd(l));
        c.set_gpr(2, x.at_end as u64)
    });
    s!(0x15fe70, |c, _m, x| {
        let l = c.gpr(5) as u32;
        x.log.push(Ev::Phase(l));
        c.set_f(0, x.phase)
    });
    s!(0x15fcf8, |c, m, x| x.log.push(Ev::Play(c.gpr(5) as i32, m.read_u32(rider + 0x46dc))));
    s!(0x11f608, |c, _m, x| x.log.push(Ev::State(c.gpr(5) as u32)));
    s!(0x11f610, |c, _m, x| x.log.push(Ev::Motion(c.gpr(5) as u32)));
    s!(0x126948, |c, _m, x| x.log.push(Ev::Steer(c.f(12).to_bits())));
    s!(0x126c00, |c, _m, x| x.log.push(Ev::Crouch(c.f(12).to_bits())));
    s!(0x126b38, |c, _m, x| x.log.push(Ev::BrakeCrouch(c.f(12).to_bits(), c.f(13).to_bits())));
    s!(0x126c78, |c, _m, x| x.log.push(Ev::Brake(c.f(12).to_bits())));
    s!(0x11d038, |c, _m, x| x.log.push(Ev::Boost(c.gpr(5) as u32, c.gpr(6) as u32)));
    s!(0x127918, |c, _m, x| x.log.push(Ev::Lean(c.f(12).to_bits(), c.f(13).to_bits())));
    s!(0x128e90, |c, _m, x| x.log.push(Ev::Rotate(c.f(12).to_bits())));
    s!(0x118f10, |c, _m, x| x.log.push(Ev::Reset(c.f(12).to_bits())));
    s!(0x128150, |_c, _m, x| x.log.push(Ev::Celebrate));
    s!(0x127e80, |_c, _m, x| x.log.push(Ev::Disappointed));
    s!(0x127d08, |_c, _m, x| x.log.push(Ev::WindUp));
    s!(0x127298, |_c, _m, x| x.log.push(Ev::Takeoff));
    s!(0x103660, |_c, _m, x| x.log.push(Ev::Taunt));
    s!(0x103b88, |_c, _m, x| x.log.push(Ev::Flourish));
    s!(STOP, |c, _m, x| {
        assert_eq!(c.gpr(5), 1);
        x.log.push(Ev::Stop)
    });
    s!(0x175de0, |c, m, x| {
        let p = c.gpr(5) as u32;
        let mut st = String::new();
        for k in 0.. {
            let b = (m.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
            if b == 0 {
                break;
            }
            st.push(b as char);
        }
        let cam = c.gpr(4) as u32;
        let idx = ((cam.wrapping_sub(game + 0xa0)) / 0x80) as i32;
        x.log.push(Ev::Cam(idx, st))
    });
    r.allow(0x1033d0);
    let obj = r.mem.alloc(0x40);
    let pad_at = r.mem.alloc(16);
    let mut g = Rng(0xc0_47_801_0000_0001);
    let mut seen = [0u32; 9];
    let mut reached = std::collections::BTreeMap::new();
    for case in 0..80_000 {
        let mut rd = random_rider(&mut g);
        for (o, v) in fields(&rd) {
            r.mem.write_u32(rider + o, v);
        }
        for (o, k) in BOARD_OBJ {
            r.mem.write_u32(board_obj + o, board_words(&rd)[k]);
        }
        let mut ours = World {
            clips: [[0x1ff, 0x202, 0x221, 0x224, 0x210, 0][g.below(6) as usize], 0],
            types: [[0, 1, 8][g.below(3) as usize], [0, 8][g.below(2) as usize]],
            at_end: g.below(2) == 0,
            phase: g.f(0.0, 1.0),
            riders: g.below(7) as i32,
            race_state: [4, 3][g.below(2) as usize],
            log: vec![],
        };
        // the cameras each race index's rider has
        for k in 0..4u32 {
            r.mem.write_u32(game + 0xa0 + 0x80 * k, game + 0xa0 + 0x80 * k);
        }
        r.mem.write_u32(race + 0x88, ours.riders as u32);
        r.mem.write_u32(race + 0x1c, ours.race_state as u32);
        *w.borrow_mut() = ours.clone();
        let pad = random_pad(&mut g);
        r.mem.write_u32(pad_at, pad);
        // the state objects: +8 / +0xc / +0x10 as each reads them, the rider pointer where each keeps it
        let (f8, fc, f10) = (g.pick(&[0.0, 0.5, 0.9, 1.0, 0.3]), g.f(-1.0, 1.0), g.below(2));
        for k in 0..16 {
            r.mem.write_u32(obj + 4 * k, 0);
        }
        r.mem.write_f32(obj + 8, f8);
        let op = g.below(9) as usize;
        seen[op] += 1;
        let mut out_obj: Vec<u32> = vec![];
        let mut ours_obj: Vec<u32> = vec![];
        match op {
            0 => {
                // cCruiseControl: +8 the look timer, +0x10 braking, +0x14 the rider
                let look = g.pick(&[0.0, 0.5, 0.99, 1.0, 2.0]);
                r.mem.write_f32(obj + 8, look);
                r.mem.write_u32(obj + 0x10, f10);
                r.mem.write_u32(obj + 0x14, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x102fc8).unwrap();
                let mut c = Cruise { look, braking: f10 };
                c.update(&mut ours, pad, &mut rd);
                out_obj = vec![r.mem.read_u32(obj + 8), r.mem.read_u32(obj + 0x10)];
                ours_obj = vec![c.look.to_bits(), c.braking];
            }
            1 => {
                r.mem.write_u32(obj, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x104030).unwrap();
                control::change_fakie(&mut ours, pad, &mut rd);
            }
            2 => {
                // cBumpControl: +8 the turn, +0xc done, +0x10 the rider
                r.mem.write_f32(obj + 0xc, fc);
                r.mem.write_u32(obj + 0x10, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x1041b0).unwrap();
                let mut b = Bump { turn: f8, done: fc };
                b.update(&mut ours, pad, &mut rd);
                out_obj = vec![r.mem.read_u32(obj + 0xc)];
                ours_obj = vec![b.done.to_bits()];
            }
            3 => {
                r.mem.write_u32(obj, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x104800).unwrap();
                control::land_hard(&mut ours, pad, &mut rd);
            }
            4 => {
                r.mem.write_u32(obj + 0xc, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x104e68).unwrap();
                LandNormal { crouch: f8 }.update(&mut ours, pad, &mut rd);
            }
            5 => {
                let s = g.pick(&[0.0, 0.3, -0.5, 1.0]);
                r.mem.write_f32(obj + 8, s);
                r.mem.write_u32(obj + 0xc, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x1000e0).unwrap();
                NaturalAir { steer: s }.update(&mut ours, pad, &mut rd);
            }
            6 => {
                r.mem.write_u32(obj, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.call(0x105bc0).unwrap();
                control::sit_and_wait(&mut ours, &mut rd);
            }
            7 => {
                r.mem.write_u32(obj, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.call(0x105c60).unwrap();
                control::getup_from_sit(&mut ours, &mut rd);
            }
            _ => {
                let look = g.pick(&[0.0, 0.99, 1.0, 2.0]);
                r.mem.write_f32(obj + 8, look);
                r.mem.write_u32(obj + 0x14, rider);
                r.cpu.set_gpr(4, obj as u64);
                r.call(0x1033d0).unwrap();
                let mut c = Cruise { look, braking: 0 };
                c.look_back(&mut ours, &mut rd);
                out_obj = vec![r.mem.read_u32(obj + 8)];
                ours_obj = vec![c.look.to_bits()];
            }
        }
        for e in &ours.log {
            let key = match e {
                Ev::Play(c, _) => format!("{op} play {c:#x}"),
                Ev::Rotate(_) => format!("{op} rotate"),
                Ev::Takeoff => format!("{op} takeoff"),
                Ev::Stop => format!("{op} stop"),
                Ev::Cam(..) => format!("{op} cam"),
                Ev::State(st) => format!("{op} state {st:#x}"),
                Ev::Reset(_) => format!("{op} reset"),
                _ => continue,
            };
            *reached.entry(key).or_insert(0u32) += 1;
        }
        let what = format!("case {case} op {op} pad {pad:#x}");
        assert_eq!(w.borrow().log, ours.log, "{what}: calls");
        assert_eq!(out_obj, ours_obj, "{what}: the state object");
        for (o, v) in fields(&rd) {
            assert_eq!(r.mem.read_u32(rider + o), v, "{what}: rider +{o:#x}");
        }
        for (o, k) in BOARD_OBJ {
            assert_eq!(r.mem.read_u32(board_obj + o), board_words(&rd)[k], "{what}: board +{o:#x}");
        }
    }
    eprintln!("ops {seen:?}
{reached:?}");
    for k in ["0 play 0x221", "0 stop", "8 play 0x210", "8 play 0x20f", "0 play 0x210", "2 rotate", "5 takeoff", "5 state 0x10", "6 cam", "7 play 0x221", "7 state 0x3", "0 state 0x6", "0 state 0xe", "1 state 0x5", "3 state 0x5", "4 state 0xe", "5 reset"] {
        assert!(reached.contains_key(k), "never reached: {k}");
    }
}

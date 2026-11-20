//! The ground's control states as the game runs them (board row G1i1): the control state (rider+0x428) on the snow is
//! cruise 3 (`CruiseControl_Update` 0x102fc8), prewind 0xe (`PrewindState_Update` 0x1050e8) or jump 10
//! (`JumpControl_Update` 0x1046b0), each on the pad word `cPlayer::GetControls` 0x151250 packs for it, after the
//! tick's motion (`cBoarder::vf7` 0x117198's order). They set targets; the now/rate/target records (brake +0x1fc,
//! crouch +0x208, steer +0x214, the wind-ups +0x220 and +0x22c) move toward them before the next tick's motion, by
//! their rate × 1/60 (`cBoarder::vf7`'s eases, the game's own step whatever the time scale).
//!
//! STANDIN (row G1i2): the air, rail, landing and wipe-out ticks still shape the port's own values (rider.rs
//! `step_tick`); the records take the port's values over when the rider is back on the snow, where the state is
//! cruise (or prewind with the jump button held, as the landing clip's end would choose: `after_clip`).

use super::{ground_tick, Input, Rider, RiderSpeech};
use crate::boarder::{self, Fields, Prewind, PrewindCall, SteerState};
use crate::control::{self, ControlEnv};
use crate::controls::{get_controls, ControlsIn, Mapper};
use crate::finish;

/// The game's 1/60 s (0x3c888889): what the records move by their rate each tick (`cBoarder::vf7`)
const DT: f32 = f32::from_bits(0x3c88_8889);

/// A now / rate / target record
pub type Rec = [f32; 3];

/// The records the ground's states set (the steer in the game's sign: the port's is the other way)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Records {
    /// +0x1fc brake, +0x208 crouch, +0x214 steer, +0x220 the spin wind-up, +0x22c the flip wind-up
    pub brake: Rec,
    pub crouch: Rec,
    pub steer: Rec,
    pub spin: Rec,
    pub flip: Rec,
}

/// The control states (rider+0x428) run here
pub const CRUISE: u32 = 3;
pub const JUMP: u32 = 10;
pub const PREWIND: u32 = 0xe;
pub const SPIN: u32 = 0x10;

/// The game's calls the states make, on the port's rider fields
struct Env<'a> {
    /// the meter (+0x1c) and the boost level (+0x130)
    meter: &'a mut f32,
    boost_level: &'a mut f32,
    tricky: bool,
    speech: &'a mut Vec<RiderSpeech>,
    /// `Boarder_SetState`'s new state, `Jump_ApplyImpulse` asked
    state: Option<u32>,
    jumped: bool,
}

/// The brake, crouch and steer of a state's rider as `finish`'s helpers take them
fn fin(r: &control::Rider, boost_level: f32) -> finish::Rider {
    finish::Rider {
        brake: r.brake,
        brake_rate: r.brake_rate,
        brake_target: r.brake_target,
        crouch: r.crouch,
        crouch_rate: r.crouch_rate,
        crouch_target: r.crouch_target,
        steer: r.steer,
        steer_rate: r.steer_rate,
        steer_target: r.steer_target,
        boost_level,
        control: r.control,
        class: r.class,
        surface: r.surface,
        vel: r.vel,
        ..Default::default()
    }
}

fn unfin(f: &finish::Rider, r: &mut control::Rider) {
    (r.brake, r.brake_rate, r.brake_target) = (f.brake, f.brake_rate, f.brake_target);
    (r.crouch, r.crouch_rate, r.crouch_target) = (f.crouch, f.crouch_rate, f.crouch_target);
    (r.steer, r.steer_rate, r.steer_target) = (f.steer, f.steer_rate, f.steer_target);
}

impl ControlEnv for Env<'_> {
    // STANDIN (row G5d): no clip plays yet, so none is asked about (layer 0's clip 0, of type 0, at its end)
    fn clip(&mut self, _layer: u32) -> i32 {
        0
    }
    fn clip_type(&mut self, _layer: u32) -> i32 {
        0
    }
    fn clip_at_end(&mut self, _layer: u32) -> bool {
        true
    }
    fn layer_phase(&mut self, _layer: u32) -> f32 {
        0.0
    }
    // STANDIN (row G5d): the clips the states play are not played
    fn play_clip(&mut self, _r: &mut control::Rider, _clip: i32) {}
    fn set_state(&mut self, _r: &mut control::Rider, s: u32) {
        self.state = Some(s);
    }
    fn set_motion(&mut self, _r: &mut control::Rider, _m: u32) {}
    /// `Boarder_SetSteerTarget` 0x126948
    fn set_steer_target(&mut self, r: &mut control::Rider, v: f32) {
        let s = SteerState { boost_level: *self.boost_level, class: r.class, vel: r.vel, steer: r.steer, surface: r.surface };
        (r.steer_target, r.steer_rate) = boarder::set_steer_target(v, &s);
    }
    /// `Boarder_SetCrouchTarget` 0x126c00
    fn set_crouch_target(&mut self, r: &mut control::Rider, v: f32) {
        let mut f = Fields { x1fc: r.brake, crouch: r.crouch, crouch_rate: r.crouch_rate, crouch_target: r.crouch_target, ..Default::default() };
        boarder::set_crouch_target(v, &mut f);
        (r.crouch_rate, r.crouch_target) = (f.crouch_rate, f.crouch_target);
    }
    /// `Boarder_SetBrakeCrouchInput` 0x126b38
    fn set_brake_crouch_input(&mut self, r: &mut control::Rider, brake: f32, crouch: f32) {
        let mut f = fin(r, *self.boost_level);
        finish::set_brake_crouch_input(brake, crouch, &mut f);
        unfin(&f, r);
    }
    /// `Boarder_SetBrakeTarget` 0x126c78
    fn set_brake_target(&mut self, r: &mut control::Rider, v: f32) {
        let mut f = fin(r, *self.boost_level);
        finish::set_brake_target(v, &mut f);
        unfin(&f, r);
    }
    /// `Boarder_UpdateBoost` 0x11d038 on the meter and the boost level (the flame's fields, +0x250.., not kept: the
    /// flame is not drawn from them yet)
    fn update_boost(&mut self, _r: &mut control::Rider, a: u32, b: u32) {
        let mut f = Fields { meter: *self.meter, boost_level: *self.boost_level, ..Default::default() };
        // the boost sounds play: the app has no replay (row E1b2g7d3b4e), as the game's test with no replay object
        for c in boarder::update_boost(a != 0, b != 0, true, self.tricky, &mut f) {
            if c == boarder::Call::BoostEmpty {
                self.speech.push(RiderSpeech::BoostEmpty);
            }
        }
        (*self.meter, *self.boost_level) = (f.meter, f.boost_level);
    }
    // STANDIN (row G5d): the lean animation, the turn of a bump clip, the celebrations, taunts and flourishes are the
    // animation's
    fn update_lean_anim(&mut self, _r: &mut control::Rider, _a: f32, _b: f32) {}
    fn rotate_about_axis(&mut self, _r: &mut control::Rider, _angle: f32) {}
    // STANDIN: the port's own respawn off the course (surface 0, rider.rs `step_tick`), no reset button
    fn request_reset(&mut self, _r: &mut control::Rider) {}
    fn jump_impulse(&mut self, _r: &mut control::Rider, _min: f32) {
        self.jumped = true;
    }
    /// `Takeoff_SetSpinRates` 0x126e30 enters the spin state (its rates are set at the take-off, `Rider::spin_enter_game`)
    fn takeoff_spin_rates(&mut self, _r: &mut control::Rider) {
        self.state = Some(SPIN);
    }
    fn play_celebrate(&mut self, _r: &mut control::Rider) {}
    fn play_disappointed(&mut self, _r: &mut control::Rider) {}
    fn play_wind_up(&mut self, _r: &mut control::Rider) {}
    fn play_takeoff(&mut self, _r: &mut control::Rider) {}
    fn ai_taunt_check(&mut self, _r: &mut control::Rider) {}
    fn random_flourish(&mut self, _r: &mut control::Rider) {}
    // STANDIN: the rider's virtual +0x7c(1) (not ported)
    fn stop(&mut self, _r: &mut control::Rider) {}
    // STANDIN: no other riders for the look back (a head turn, row G5d)
    fn race_riders(&mut self) -> i32 {
        0
    }
    fn race_state(&mut self) -> i32 {
        0
    }
    fn cam_start(&mut self, _index: i32, _script: &str) {}
    fn run_control(&mut self, _r: &mut control::Rider, _control: u32) {}
}

impl Rider {
    /// The records' "now" from the port's values (a tick of the port's own code may have changed them)
    fn records_from_port(&mut self) {
        self.rec.brake[0] = self.brake_v;
        self.rec.crouch[0] = self.crouch_v;
        self.rec.steer[0] = -self.steer;
        self.rec.spin[0] = self.wind;
        self.rec.flip[0] = self.wind_flip;
    }

    /// The port's values from the records
    fn port_from_records(&mut self) {
        self.brake_v = self.rec.brake[0];
        self.crouch_v = self.rec.crouch[0];
        self.steer = -self.rec.steer[0];
        self.wind = self.rec.spin[0];
        self.wind_flip = self.rec.flip[0];
    }

    /// `cBoarder::vf7` 0x117198 before the motion, on the snow: each record moves its rate × 1/60 toward its target
    /// (`boarder_tick::ease`)
    pub(super) fn ease_records(&mut self) {
        self.records_from_port();
        for r in [&mut self.rec.brake, &mut self.rec.crouch, &mut self.rec.steer, &mut self.rec.spin, &mut self.rec.flip] {
            r[0] = crate::boarder_tick::ease(r[0], r[2], r[1] * DT);
        }
        self.port_from_records();
    }

    /// `Boarder_SetState` 0x11f608 for the states run here, with their enters: `CruiseControl_Enter` 0x102fb0 (the
    /// object's +8, +0xc, +0x10 and rider+0x1dc cleared), `PrewindControl_Enter` 0x105070 (the steer, the lean +0x268
    /// and the brake to 0 at 6.6666665 a second, the state's flag cleared; its clip 0x217 not played, STANDIN row G5d)
    pub fn set_control(&mut self, s: u32) {
        if s == self.ctl {
            return;
        }
        self.ctl = s;
        match s {
            CRUISE => {
                self.cruise = control::Cruise::default();
                self.jump_c = 0.0;
            }
            PREWIND => {
                let r = f32::from_bits(0x40d5_5555);
                self.rec.steer[1..].copy_from_slice(&[r, 0.0]);
                self.rec.brake[1..].copy_from_slice(&[r, 0.0]);
                self.prewind_started = 0;
            }
            _ => {}
        }
    }

    /// The state's rider: the records, the velocity and the board's forward in the game's cm and axes
    fn control_rider(&self) -> control::Rider {
        let [b, c, s] = [self.rec.brake, self.rec.crouch, self.rec.steer];
        control::Rider {
            vel: ground_tick::to_game(self.vel, 100.0, 0.0),
            board: ground_tick::to_game(self.forward(), 1.0, 0.0),
            brake: b[0],
            brake_rate: b[1],
            brake_target: b[2],
            crouch: c[0],
            crouch_rate: c[1],
            crouch_target: c[2],
            steer: s[0],
            steer_rate: s[1],
            steer_target: s[2],
            surface: self.surface as u32,
            // STANDIN (row G1i2): the finish (+0x418) as not yet crossed; the app's own finish takes the rider over
            finish: -1,
            class: self.stats.kind as u32,
            control: self.ctl,
            trick: -1,
            filter: u32::MAX,
            goofy: self.data.goofy,
            ..Default::default()
        }
    }

    fn from_control_rider(&mut self, r: &control::Rider) {
        self.rec.brake = [r.brake, r.brake_rate, r.brake_target];
        self.rec.crouch = [r.crouch, r.crouch_rate, r.crouch_target];
        self.rec.steer = [r.steer, r.steer_rate, r.steer_target];
    }

    /// After the snow's motion, `cBoarder::vf7`'s control part: the pad word for the state (`cPlayer::GetControls`
    /// on the rider's button map, [`Rider::button_map`]) and the state's update. A rider
    /// back on the snow in another state is put in cruise (prewind with the jump button held). Returns true when the
    /// rider jumped (`JumpControl_Update`'s `Jump_ApplyImpulse`, motion 1, `Takeoff_SetSpinRates`).
    pub(super) fn ground_control(&mut self, input: &Input) -> bool {
        if !matches!(self.ctl, CRUISE | PREWIND | JUMP) {
            self.set_control(if input.jump { PREWIND } else { CRUISE });
        }
        self.read_pad(input);
        let map = self.button_map();
        let mut trick = 0x1a;
        let mut slope = false;
        let mut pad = get_controls(ControlsIn { state: self.ctl, paused: false }, &mut Mapper { map: &map, pad: &self.pad_dev }, &mut trick, &mut || 0, &mut slope);
        // STANDIN: the app's steer and wind-up as the word's six-bit sticks, past the pad's dead zone (0.38): the
        // computer riders and the autopilot steer by fractions (`cComputer::GetControls` 0x1384b8 writes its word
        // directly too); keys give the same as the pad (cruise: the steer at bit 5; prewind: the turn at 17, the
        // spin and flip wind-ups at 5 and 11)
        let six = |a: f32| ((a.clamp(-1.0, 1.0) * 31.0) as i32 as u32) & 0x3f;
        let put = |w: &mut u32, shift: u32, v: u32| *w = *w & !(0x3f << shift) | v << shift;
        match self.ctl {
            CRUISE => put(&mut pad, 5, six(-input.steer)),
            PREWIND => {
                put(&mut pad, 17, six(-input.steer));
                put(&mut pad, 5, six(input.wind.unwrap_or(input.steer)));
                put(&mut pad, 11, six(-input.flip));
            }
            _ => {}
        }
        let tricky = self.tricky();
        let (mut meter, mut level) = (self.boost, self.boost_lvl);
        let mut speech = std::mem::take(&mut self.speech);
        let mut env = Env { meter: &mut meter, boost_level: &mut level, tricky, speech: &mut speech, state: None, jumped: false };
        let mut cr = self.control_rider();
        match self.ctl {
            CRUISE => {
                let mut c = self.cruise;
                c.update(&mut env, pad, &mut cr);
                self.cruise = c;
            }
            JUMP => control::jump(&mut env, pad, &mut cr),
            _ => {}
        }
        let (state, jumped) = (env.state, env.jumped);
        self.speech = speech;
        if self.ctl == PREWIND {
            let mut p = Prewind {
                f: Fields { meter, boost_level: level, x1fc: cr.brake, crouch: cr.crouch, crouch_rate: cr.crouch_rate, crouch_target: cr.crouch_target, ..Default::default() },
                started: self.prewind_started,
                kind: 0,
                x418: -1,
                spin: self.rec.spin,
                flip: self.rec.flip,
                x1c8: self.jump_c,
                vel: cr.vel,
                board: cr.board,
                steer: cr.steer,
                steer_rate: cr.steer_rate,
                steer_target: cr.steer_target,
                surface: cr.surface,
                switch: self.switch as u32,
                class: cr.class,
                x304: 0,
                // (only "more than five" is read: `Boarder_IsTricky`)
                ubers: if tricky { 6 } else { 0 },
                windup_stat: self.stats.bytes[crate::stats::WINDUP],
            };
            let calls = boarder::prewind_update(pad, true, &mut p);
            (meter, level) = (p.f.meter, p.f.boost_level);
            (cr.crouch, cr.crouch_rate, cr.crouch_target) = (p.f.crouch, p.f.crouch_rate, p.f.crouch_target);
            (cr.steer, cr.steer_rate, cr.steer_target) = (p.steer, p.steer_rate, p.steer_target);
            (self.rec.spin, self.rec.flip, self.jump_c, self.prewind_started) = (p.spin, p.flip, p.x1c8, p.started);
            self.from_control_rider(&cr);
            for c in calls {
                match c {
                    PrewindCall::State(s) => self.set_control(s),
                    PrewindCall::Boost(boarder::Call::BoostEmpty) => self.speech.push(RiderSpeech::BoostEmpty),
                    // STANDIN: the clips (row G5d); the reset button and surface 0 are the port's own respawn
                    _ => {}
                }
            }
        } else {
            self.from_control_rider(&cr);
        }
        (self.boost, self.boost_lvl) = (meter, level);
        if let Some(s) = state {
            self.set_control(s);
        }
        self.port_from_records();
        // (the port's crouch kept for the jump: its prewind pose and the air's late release read it)
        self.charge = if self.ctl == PREWIND { self.crouch_v } else { 0.0 };
        if jumped {
            // `Takeoff_SetSpinRates` clears the wind-ups once their rates are taken (`Rider::take_off` keeps them)
            self.rec.spin = [0.0; 3];
            self.rec.flip = [0.0; 3];
        }
        jumped
    }
}

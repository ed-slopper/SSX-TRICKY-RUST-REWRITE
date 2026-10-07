//! Snowboarder physics. Plain functions over plain data so the same code runs in the game and
//! in the headless self-test. Bevy space: metres, seconds, Y up.
//!
//! The speeds, gravity, drag, turning, jump and rotation numbers are the original game's.

use crate::collide::CollisionWorld;
use crate::rails::Rails;
use crate::trickdata::{RiderData, COMBOS, RIDERS};
use std::collections::VecDeque;
use bevy::math::Vec3;

// The numbers below are the original game's, read out of the decompiled SLUS_203.26 (converted from
// its cm and 1/60 s frames to metres and seconds), for a rider with middling stats on ordinary snow.
/// gravity on the snow (surface table row 1), and in the air: gentle on the way up, hard on the way down
pub const GRAVITY: f32 = 13.0;
pub const GRAVITY_RISING: f32 = 8.5;
pub const GRAVITY_FALLING: f32 = 19.0;
/// the air only slows the horizontal part of the velocity (1/s)
pub const AIR_DRAG: f32 = 0.2;
/// drag on the snow as a rate (1/s): DRAG_LIN + DRAG_STAND when not crouched + DRAG_V * v + DRAG_V2 * v^2
pub const DRAG_LIN: f32 = 0.0019;
pub const DRAG_STAND: f32 = 0.1057;
pub const DRAG_V: f32 = 0.000196;
pub const DRAG_V2: f32 = 0.0000856;
/// braking adds this much drag (1/s)
pub const BRAKE: f32 = 1.88;
/// hard speed limits (m/s): 100.4 km/h on the snow, up to 120.5 km/h boosting or in the air
pub const SPEED_CAP: f32 = 27.89;
pub const BOOST_CAPS: [f32; 3] = [29.32, 30.72, 33.47];
pub const AIR_CAP: f32 = 33.47;
/// once the boost stops the limit comes back down this fast (m/s^2)
pub const CAP_DECAY: f32 = 2.08;
/// the rider pushes along by himself up to 51.85 km/h
pub const PUSH_TO: f32 = 14.4;
pub const PUSH: f32 = 1.75;
pub const PUSH_MAX: f32 = 5.0;
/// sideways acceleration of a full carve: g * tan(58.3 deg * 0.905); below TURN_FULL_SPEED it scales with speed
pub const TURN_ACCEL: f32 = 17.1;
pub const TURN_FULL_SPEED: f32 = 11.3;
/// rotation in the air (rad/s): what a jump starts with, with no wind-up and with a full one (a wound-up flip gets 2/3 of the spin's)
pub const AIR_SPIN: f32 = 4.63;
pub const AIR_SPIN_WOUND: f32 = 10.16;
pub const FLIP_RATE: f32 = AIR_SPIN;
pub const FLIP_WOUND: f32 = AIR_SPIN_WOUND * 2.0 / 3.0;
/// the rotation dies away through the jump (x0.70 each second) down to this floor
pub const SPIN_DECAY: f32 = 0.70;
pub const SPIN_FLOOR: f32 = 1.396;
/// seconds for the wind-up to follow the stick while crouched for a jump
pub const WIND_TIME: f32 = 0.5;
/// jump: 6.31 m/s at least, a little more with a full crouch (which takes 0.66 s)
pub const JUMP_MIN: f32 = 6.31;
pub const JUMP_FULL: f32 = 7.2;
pub const CHARGE_TIME: f32 = 0.66;
pub const BOOST_ACCEL: f32 = 3.75;
/// seconds of boost in a full meter
pub const BOOST_SECONDS: f32 = 22.0;
/// how fast sideways slip is killed (1/s); this is what makes the board carve
pub const EDGE_GRIP: f32 = 7.0;
pub const BODY_RADIUS: f32 = 0.45;
pub const WALL_BOUNCE: f32 = 0.25;
/// hitting something solid faster than this (70 km/h into it) is a wipe-out
pub const WALL_CRASH: f32 = 19.4;
/// landing further than this from upright is a wipe-out (about 100 degrees: the original is forgiving)
pub const CRASH_PITCH: f32 = 1.75;
/// rails do not carry you faster than this
pub const RAIL_CAP: f32 = 29.3;
/// how close (m) the board has to pass to a rail, while in the air, to lock on
pub const RAIL_REACH: f32 = 0.9;
pub const RAIL_FRICTION: f32 = 0.03;
/// how fast the board turns on a rail when steered (rad/s)
pub const RAIL_TWIST: f32 = 5.5;
// ---- scoring, from the original (points = value * 6786.5, rounded to 10)
pub const PTS_SPIN_180: f32 = 474.7;
pub const PTS_FLIP: f32 = 1696.6;
/// starting a grab: this times (grabs already done this jump + 1), up to four
pub const PTS_GRAB_START: f32 = 288.4;
/// holding a grab, per second, times the grab's rate (1 to 2.5; a tweak doubles it; ubers are 18 or 20)
pub const PTS_HOLD: f32 = 305.4;
pub const PTS_RAIL: f32 = 1018.2;
pub const PTS_ONTO_RAIL: f32 = 1221.7;
pub const PTS_OFF_RAIL: f32 = 1357.2;
pub const SWITCH_BONUS: f32 = 1.3;
/// seconds a grab has to be reached for before it counts
pub const GRAB_REACH: f32 = 0.22;
/// a full meter gives this long to do uber tricks
pub const UBER_SECONDS: f32 = 20.0;

/// The original's surface table, by the patch's surface type: gravity (cm/s^2), drag (linear,
/// v^2, v^3 with v in 10 m/s), side grip, most lean (degrees), the speed the rider pushes
/// himself to (km/h) and how hard.
pub const SURFACES: [[f32; 8]; 20] = [
    [989.826, 1.0, 1.0, 1.0, 1.0, 58.3, 0.0, 0.0],                       // 0 off the course
    [1300.85, 0.00204, 0.00196, 0.00751, 1.2017, 58.3, 51.8501, 2.00119], // 1 groomed snow
    [1200.45, 0.00606, 0.0061, 0.00884, 1.50908, 58.3, 45.6771, 1.81067], // 2 second-line snow
    [1151.31, 0.04996, 0.00505, 0.00508, 3.00133, 58.3, 40.5149, 3.08695], // 3 powder
    [999.135, 0.14175, 0.01857, 0.00795, 3.5026, 58.3, 36.1039, 2.01986],  // 4 deep powder
    [1350.93, 0.0, 0.0, 0.00254, 0.0025, 45.0004, 64.0374, 3.00319],      // 5 ice
    [980.0, 0.01, 0.0, 0.025, 1.0, 58.3, 52.5, 2.2],                      // 6 structures
    [980.0, 0.02913, 0.01153, 0.0, 0.19627, 58.3, 40.7847, 2.39158],      // 7
    [980.0, 0.00861, 0.00785, 0.00771, 1.57987, 58.3, 43.2219, 2.008],    // 8
    [980.0, 0.0545, 0.00501, 0.00522, 0.89228, 21.3399, 19.5848, 1.67792], // 9 rock, concrete
    [980.0, 0.01, 0.0, 0.025, 1.0, 58.3, 52.5, 2.2],                      // 10
    [980.0, 0.01096, 0.00249, 0.025, 0.0, 58.3, 26.3196, 1.46681],        // 11
    [980.0, 0.01, 0.00258, 0.0, 0.89228, 58.3, 54.3926, 2.212],           // 12
    [980.0, 0.03971, 0.14873, 0.04455, 0.1, 58.3, 60.9113, 1.42786],      // 13 metal
    [980.0, 0.00444, 0.00195, 0.00576, 0.03, 58.3, 65.3384, 2.2],         // 14
    [980.0, 0.00217, 0.00223, 0.00768, 1.0, 58.3, 52.5, 2.2],             // 15
    [980.0, 0.01, 0.0, 0.025, 1.0, 58.3, 52.5, 2.2],                      // 16
    [980.0, 0.01, 0.0, 0.025, 1.0, 58.3, 52.5, 2.2],                      // 17
    [1350.86, 0.0, 0.0, 0.0026, 1.20915, 58.3, 60.0436, 3.00478],         // 18 show-off ramps
    [980.0, 0.01, 0.0, 0.025, 1.0, 58.3, 52.5, 2.2],                      // 19
];

/// A rider's attributes as fractions (0 to 1), as the game's formulas use them.
#[derive(Clone, Copy)]
pub struct Stats { pub edging: f32, pub speed: f32, pub stability: f32, pub tricks: f32, pub jump: f32, pub windup: f32, pub weight: f32, pub alpine: bool }
impl Default for Stats {
    fn default() -> Self { Self { edging: 0.5, speed: 0.5, stability: 0.5, tricks: 0.5, jump: 0.75, windup: 0.75, weight: 75.0, alpine: false } }
}
impl Stats {
    /// A new rider or a fully trained one, on the chosen board: the game's own mix of 80% rider
    /// and 20% board.
    pub fn of(d: &RiderData, master: bool, board: &crate::trickdata::Board) -> Self {
        let f = |v: (u8, u8), g: usize| (0.8 * if master { v.1 } else { v.0 } as f32 + 0.2 * board.bonus[g] as f32) / 100.0;
        Self { edging: f(d.edging, 0), speed: f(d.speed, 1), stability: f(d.stability, 2), tricks: f(d.tricks, 3),
               jump: d.jump as f32 / 100.0, windup: d.windup as f32 / 100.0, weight: d.weight as f32, alpine: board.kind == 2 }
    }
    fn drag_linear(&self) -> f32 { if self.alpine { 0.7076 - 0.3020 * self.speed } else { 1.0650 - 0.3085 * self.speed } }
    fn drag_cubic(&self) -> f32 { 1.2848 - 0.2904 * self.speed }
    fn push(&self) -> f32 { if self.alpine { 1.2050 + 0.3050 * self.speed } else { 0.7381 + 0.2769 * self.speed } }
    fn brake(&self) -> f32 { 1.7513 * (0.615 + 0.918 * self.edging) }
    fn spin_scale(&self) -> f32 { 0.5449 + 0.6742 * self.tricks }
    fn spin_decay(&self) -> f32 { (0.99301 + 0.00201 * self.tricks).powi(60) }
    fn wind_time(&self) -> f32 { 1.0 / (2.0005 * (0.812 + 0.329 * self.windup)) }
    fn jump(&self, crouch: f32, speed: f32) -> f32 {
        let by_speed = if speed < 9.93 { 0.2468 + 0.881 * speed } else { 8.9987 };
        JUMP_MIN.max(crouch * crouch * (0.0985 + 0.787 * self.jump) * by_speed)
    }
    /// how far from upright (radians) a landing can be
    pub fn crash_pitch(&self) -> f32 { 2.684 / (2.0408 - 1.0374 * self.stability) }
    fn meter(&self) -> f32 { 0.9788 + 0.3597 * self.tricks }
    /// what a collision with another rider goes by
    pub fn mass(&self) -> f32 { self.weight * (self.stability * 2.1458 + 0.796) }
}
/// how long a wipe-out lasts: the bail and the get-up
pub const CRASH_SECONDS: f32 = 1.95;

#[derive(Clone, Copy, Default)]
pub struct Input {
    /// -1 = left, +1 = right
    pub steer: f32,
    pub tuck: bool,
    pub brake: bool,
    pub jump: bool,
    /// in the air: +1 = front flip, -1 = back flip
    pub flip: f32,
    /// in the air: the shoulder buttons held (L1 = 1, R1 = 2, L2 = 4, R2 = 8); each combination is a grab
    pub grab: u8,
    /// in the air while holding a grab: tweak it, or turn it into an uber trick when the meter allows
    pub tweak: bool,
    pub boost: bool,
}

#[derive(Clone)]
pub struct Rider {
    pub pos: Vec3,
    pub vel: Vec3,
    /// heading around world Y; forward is (-sin yaw, 0, -cos yaw)
    pub yaw: f32,
    pub grounded: bool,
    pub normal: Vec3,
    pub charge: f32,
    pub air_time: f32,
    pub spawn: (Vec3, f32),
    pub safe: (Vec3, f32),
    safe_timer: f32,
    pub respawns: u32,
    /// grinding: (rail index, distance along it, signed speed along it)
    pub rail: Option<(usize, f32, f32)>,
    rail_cooldown: f32,
    /// what the controls were on the last step (the pose follows it)
    pub input: Input,
    // ---- tricks
    /// signed turns of spin and flip made since leaving the snow (radians)
    pub spin: f32,
    pub flip: f32,
    flip_dir: f32,
    /// wind-up before a jump (-1..1) for spin and flip, and the rotation rates the jump started with
    pub wind: f32,
    pub wind_flip: f32,
    spin_rate: f32,
    flip_rate: f32,
    /// the speed limit right now (it lags behind when a boost ends)
    cap: f32,
    steer: f32,
    pub grab: u8,
    /// seconds the current grab has been held, which grabs have counted this jump, which were tweaked
    grab_hold: f32,
    grab_done: [bool; 16],
    grab_tweaked: [bool; 16],
    /// points earned so far in this jump from grabs and ubers, and what they were called
    air_pts: f32,
    air_names: Vec<String>,
    ubers_done: u8,
    switch_takeoff: bool,
    from_rail: bool,
    rail_pts: f32,
    /// the last five tricks landed: repeating one pays a half, a third...
    recent: VecDeque<String>,
    /// seconds left in which uber tricks can be done
    pub uber_timer: f32,
    pub stats: Stats,
    pub data: &'static RiderData,
    /// surface-table row of the snow under the board
    pub surface: u8,
    grind_time: f32,
    /// on a rail: the board's angle to the direction of travel, how far it has been turned in all,
    /// and how long it has been ridden sideways
    pub rail_twist: f32,
    rail_spun: f32,
    rail_side: f32,
    rail_entry: f32,
    rail_goal: f32,
    rail_held: bool,
    rail_side_signed: f32,
    /// seconds left of a shove, and which side it went to (+1 = right)
    pub shove: f32,
    pub shove_side: f32,
    /// how the last wipe-out went (0 forwards, 1 backwards, 2 / 3 to a side) and how the last
    /// landing came down (0 clean, 1 / 2 crooked to a side, 3 tilted)
    pub crash_kind: u8,
    pub land_kind: u8,
    /// riding tail-first (after landing a half spin); the body is drawn turned round
    pub switch: bool,
    pub score: u32,
    /// boost meter, 0..1
    pub boost: f32,
    pub boosting: bool,
    /// name and points of the last trick, and seconds left to show it
    pub last_trick: String,
    pub trick_timer: f32,
    /// seconds left recovering from a bad landing
    pub crashed: f32,
    // ---- uber tricks
    /// this rider's signature moves: (clip name, length in frames at 30 a second)
    pub ubers: Vec<(String, f32)>,
    /// frames into the current uber trick (0 = not doing one) and which one
    pub uber: f32,
    pub uber_id: usize,
    uber_done: bool,
    /// uber tricks landed this run; six spell TRICKY and make boost unlimited
    pub letters: u8,
    /// from a multiplier pickup: applied to the next trick landed
    pub multiplier: u32,
}

pub fn heading(yaw: f32) -> Vec3 { Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) }

impl Rider {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self { pos, vel: Vec3::ZERO, yaw, grounded: false, normal: Vec3::Y, charge: 0.0, air_time: 0.0,
               spawn: (pos, yaw), safe: (pos, yaw), safe_timer: 0.0, respawns: 0, rail: None, rail_cooldown: 0.0, input: Input::default(),
               spin: 0.0, flip: 0.0, flip_dir: 0.0, wind: 0.0, wind_flip: 0.0, spin_rate: AIR_SPIN, flip_rate: FLIP_RATE, cap: SPEED_CAP, steer: 0.0, grab: 0, grab_hold: 0.0, grab_done: [false; 16], grab_tweaked: [false; 16], air_pts: 0.0, air_names: Vec::new(), ubers_done: 0, switch_takeoff: false, from_rail: false, rail_pts: 0.0, recent: VecDeque::new(), uber_timer: 0.0, stats: Stats::default(), data: &RIDERS[0], surface: 1, grind_time: 0.0, rail_twist: 0.0, rail_spun: 0.0, rail_side: 0.0, rail_entry: 0.0, rail_goal: 0.0, rail_held: false, rail_side_signed: 0.0, shove: 0.0, shove_side: 1.0, crash_kind: 0, land_kind: 0, switch: false, score: 0, boost: 0.0, boosting: false,
               last_trick: String::new(), trick_timer: 0.0, crashed: 0.0,
               ubers: Vec::new(), uber: 0.0, uber_id: 0, uber_done: false, letters: 0, multiplier: 1 }
    }
    pub fn respawn(&mut self, at: (Vec3, f32)) {
        self.pos = at.0 + Vec3::Y * 0.5;
        self.yaw = at.1;
        self.vel = Vec3::ZERO;
        self.grounded = false;
        self.charge = 0.0;
        self.air_time = 0.0;
        self.rail = None;
        self.spin = 0.0;
        self.flip = 0.0;
        self.flip_dir = 0.0;
        self.switch = false;
        self.wind = 0.0;
        self.wind_flip = 0.0;
        self.cap = SPEED_CAP;
        self.clear_air();
        self.from_rail = false;
        self.rail_pts = 0.0;
        self.grind_time = 0.0;
        self.add_meter(-0.12);
        self.respawns += 1;
    }
    fn clear_air(&mut self) {
        self.spin = 0.0;
        self.flip = 0.0;
        self.flip_dir = 0.0;
        self.grab = 0;
        self.grab_hold = 0.0;
        self.grab_done = [false; 16];
        self.grab_tweaked = [false; 16];
        self.air_pts = 0.0;
        self.air_names.clear();
        self.ubers_done = 0;
        self.uber = 0.0;
        self.uber_done = false;
    }
    /// The boost meter. Filling it opens twenty seconds in which uber tricks can be done.
    pub fn add_meter(&mut self, delta: f32) {
        let was = self.boost;
        self.boost = (self.boost + delta * if delta > 0.0 { self.stats.meter() } else { 1.0 }).clamp(0.0, 1.0);
        if delta > 0.0 && self.boost >= 1.0 && was < 1.0 { self.uber_timer = UBER_SECONDS; }
    }
    fn wipe_out(&mut self) {
        self.last_trick = "CRASH".into();
        self.trick_timer = 2.0;
        self.vel *= 0.35;
        self.crashed = CRASH_SECONDS;
        self.add_meter(-0.1);
    }
    /// Bank a trick: the original halves (thirds, ...) the pay of one repeated among the last five.
    fn award(&mut self, names: Vec<String>, base: f32, mult: f32) {
        if names.is_empty() || base < 5.0 { return; }
        let key = names.join(" + ");
        let repeats = self.recent.iter().filter(|k| **k == key).count() as f32;
        self.recent.push_back(key.clone());
        if self.recent.len() > 5 { self.recent.pop_front(); }
        let pts = ((base * mult / (repeats + 1.0) / 10.0).round() * 10.0) as u32;
        self.score += pts;
        self.add_meter(base / 10000.0 / (repeats + 1.0));
        let note = if mult > 1.01 { format!("  x{mult:.1}").replace(".0", "") } else { String::new() };
        self.last_trick = format!("{key}{note}  +{pts}");
        self.trick_timer = 3.0;
    }
    /// Called when the board touches down (snow or rail): score what was done in the air.
    fn land(&mut self) {
        let half_turns = (self.spin.abs() / std::f32::consts::PI).round() as u32;
        let flips = (self.flip.abs() / std::f32::consts::TAU).round() as u32;
        let tau = std::f32::consts::TAU;
        let off = (self.flip.rem_euclid(tau)).min(tau - self.flip.rem_euclid(tau));
        let uber_cut_short = self.uber > 0.0 && !self.uber_done;
        if off > self.stats.crash_pitch() || (self.grab != 0 && self.grab_hold > GRAB_REACH) || uber_cut_short {
            // too far from upright, or still holding the board: wipe out
            self.crash_kind = if self.flip < -0.5 { 1 } else { 0 };
            self.wipe_out();
        } else {
            let mut names: Vec<String> = Vec::new();
            let mut pts = self.air_pts;
            if half_turns > 0 { names.push(format!("{}", half_turns * 180)); pts += half_turns as f32 * PTS_SPIN_180; }
            if flips > 0 {
                let n = ["", "Double ", "Triple ", "Quad "][(flips as usize - 1).min(3)];
                names.push(format!("{n}{} Flip", if self.flip > 0.0 { "Front" } else { "Back" }));
                pts += flips as f32 * PTS_FLIP;
            }
            names.extend(self.air_names.drain(..));
            if !names.is_empty() {
                if self.from_rail { pts += PTS_OFF_RAIL; }
                if self.air_time >= 4.0 { pts += ((self.air_time - 4.0) as u32 + 1) as f32 * 1000.0; }
                let mult = self.multiplier as f32 * if self.switch_takeoff { SWITCH_BONUS } else { 1.0 };
                if self.switch_takeoff { names.insert(0, "Switch".into()); }
                self.award(names, pts, mult);
            }
            self.letters = (self.letters + self.ubers_done).min(6);
            self.multiplier = 1;
        }
        self.from_rail = false;
        self.clear_air();
    }
    pub fn pick_multiplier(&mut self, m: u32) {
        self.multiplier = self.multiplier.max(m);
        self.last_trick = format!("x{m} on your next trick");
        self.trick_timer = 2.0;
    }
    pub fn pick_boost(&mut self, amount: f32, kick: bool) {
        self.add_meter(amount);
        if kick { let v = self.vel.normalize_or(heading(self.yaw)); self.vel += v * 6.0; }
        self.last_trick = if kick { "Speed boost".into() } else { "Boost meter filled".into() };
        self.trick_timer = 2.0;
    }
    /// Six landed uber tricks spell TRICKY: boost no longer runs out.
    pub fn tricky(&self) -> bool { self.letters >= 6 }
    /// Uber tricks can be started (the meter was filled in the last twenty seconds, or TRICKY).
    pub fn uber_ready(&self) -> bool { self.uber_timer > 0.0 || self.tricky() }
    fn end_grind(&mut self) {
        if self.grind_time > 0.25 {
            let turns = (self.rail_spun / std::f32::consts::PI).round() as u32;
            let slide = self.rail_side > self.grind_time * 0.5;
            let mut names = vec![format!("{:.1} s {}", self.grind_time, if !slide { if self.rail_twist.cos() < -0.5 { "Switch 50/50 Rail" } else { "50/50 Rail" } } else if self.rail_side_signed > 0.0 { "BS Rail" } else { "FS Rail" })];
            if turns > 0 { names.push(format!("{} on the rail", turns * 180)); }
            let pts = self.rail_pts + PTS_ONTO_RAIL + turns as f32 * PTS_SPIN_180;
            self.award(names, pts, 1.0);
            self.from_rail = true;
        }
        self.grind_time = 0.0;
        self.rail_pts = 0.0;
        self.rail_spun = 0.0;
        self.rail_side = 0.0;
    }

    /// The board meets the snow: it lines up with the direction of travel (tail-first if that is
    /// nearer), and a crooked or tilted landing costs speed, by the original's numbers.
    fn touch_down(&mut self) {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let wrap = |mut a: f32| { while a > PI { a -= TAU; } while a < -PI { a += TAU; } a };
        let hv = Vec3::new(self.vel.x, 0.0, self.vel.z);
        if hv.length() > 2.0 {
            let travel = f32::atan2(-hv.x, -hv.z);
            let mut d = wrap(self.yaw - travel);
            if d.abs() > FRAC_PI_2 {
                self.switch = !self.switch;
                d = wrap(d + PI);
            }
            self.vel *= 1.1136 - 0.2603 * d.abs().clamp(25f32.to_radians(), 80f32.to_radians());
            self.land_kind = if d.abs() < 25f32.to_radians() { 0 } else if d > 0.0 { 1 } else { 2 };
            self.yaw = travel + d * 0.3;
        }
        let off = self.flip.rem_euclid(TAU).min(TAU - self.flip.rem_euclid(TAU));
        if off > 25f32.to_radians() { self.land_kind = 3; }
        self.vel *= 1.0643 - 0.2454 * off.clamp(15f32.to_radians(), 50f32.to_radians());
    }
    /// Shoved over by another rider.
    pub fn knock_down(&mut self, push: Vec3) {
        if self.crashed > 0.0 { return; }
        self.crash_kind = if push.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 3 } else { 2 };
        self.vel = self.vel * 0.45 + push;
        self.crashed = CRASH_SECONDS;
        self.rail = None;
        self.charge = 0.0;
    }
    /// Which way the body faces: the board's heading, turned round when riding switch.
    pub fn facing(&self) -> f32 { if self.switch { self.yaw + std::f32::consts::PI } else { self.yaw } }
    fn jump_speed(&self) -> f32 { self.stats.jump(self.charge, self.vel.length()) }
    /// Leaving the snow: the wind-up decides how fast this jump can spin and flip.
    fn take_off(&mut self) {
        let k = self.stats.spin_scale();
        self.spin_rate = (5.2531 * k).max(11.517 * k * self.wind.abs());
        self.flip_rate = (5.2531 * k).max(11.517 * k * 2.0 / 3.0 * self.wind_flip.abs());
        self.switch_takeoff = self.switch;
        self.wind = 0.0;
        self.wind_flip = 0.0;
        self.charge = 0.0;
        self.grounded = false;
        self.air_time = 0.0;
    }
    /// Spin or flip rate this far into the jump.
    fn air_rate(&self, start: f32) -> f32 { (start * self.stats.spin_decay().powf(self.air_time)).max(SPIN_FLOOR) }
    fn crouch(&mut self, input: &Input, dt: f32) {
        if input.jump {
            self.charge = (self.charge + dt / CHARGE_TIME).min(1.0);
            let k = (dt / self.stats.wind_time()).min(1.0);
            self.wind += (input.steer - self.wind) * k;
            self.wind_flip += (input.flip - self.wind_flip) * k;
        }
    }

    /// The trick row the held shoulder buttons select, if any.
    pub fn grab_row(&self) -> Option<&'static crate::trickdata::Row> { (self.grab as usize).checked_sub(1).and_then(|i| self.data.rows.get(i)) }

    /// Board direction along the current surface.
    pub fn forward(&self) -> Vec3 {
        let h = heading(self.yaw);
        (h - self.normal * h.dot(self.normal)).normalize_or(h)
    }

    pub fn step(&mut self, world: &CollisionWorld, rails: &Rails, input: Input, dt: f32) {
        let surf = SURFACES[(self.surface as usize).min(19)];
        let g = Vec3::NEG_Y * surf[0] * 0.01;
        let mut input = input;
        // the meter leaks slowly; the uber window only runs down on the ground
        if self.tricky() { self.boost = 1.0; self.uber_timer = 1.0; }
        else {
            self.boost = (self.boost - 0.0043 * dt).max(0.0);
            if self.grounded || self.rail.is_some() { self.uber_timer = (self.uber_timer - dt).max(0.0); }
        }
        self.trick_timer = (self.trick_timer - dt).max(0.0);
        self.crashed = (self.crashed - dt).max(0.0);
        self.shove = (self.shove - dt).max(0.0);
        if self.crashed > 0.0 { input = Input { steer: input.steer * 0.3, ..Input::default() }; }
        self.input = input;
        self.boosting = false;
        self.rail_cooldown = (self.rail_cooldown - dt).max(0.0);

        // ---- grinding a rail: slide along it under gravity until it ends or we hop off
        if let Some((ri, mut d, mut s)) = self.rail {
            let rail = &rails.0[ri];
            let (_, tan) = rail.at(d);
            // the original: plain 9.8 gravity along the rail, no friction; boost pushes hard
            s += -9.8 * tan.y * dt;
            if input.boost && self.boost > 0.0 {
                let level = if self.boost > 0.666 || self.tricky() { 1.0 } else if self.boost > 0.334 { 0.6 } else { 0.25 };
                s += s.signum() * 24.5 * level * dt;
                if !self.tricky() { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); }
                self.boosting = true;
            }
            s = s.clamp(-AIR_CAP, AIR_CAP);
            let sc = s.abs() * 100.0;
            self.rail_pts += PTS_RAIL * dt * if sc >= 800.0 { 1.0 } else if sc < 50.0 { 0.0 } else { sc / (3200.0 - 3.0 * sc) };
            d += s * dt;
            let (p, tan) = rail.at(d);
            let travel = tan * s.signum();
            self.pos = p;
            self.vel = tan * s;
            // steering turns the board a quarter turn at a time, as the original's rail clips do
            // (regular, sideways, tail-first); holding the stick keeps it turning
            let q = std::f32::consts::FRAC_PI_2;
            let pressed = input.steer.abs() > 0.5;
            let arrived = (self.rail_goal - self.rail_twist).abs() < 0.03;
            if pressed && (!self.rail_held || arrived) { self.rail_goal = (self.rail_goal / q).round() * q + input.steer.signum() * q; }
            self.rail_held = pressed;
            let turn = (self.rail_goal - self.rail_twist).clamp(-RAIL_TWIST * dt, RAIL_TWIST * dt);
            self.rail_twist += turn;
            self.rail_spun += turn.abs();
            self.rail_side_signed += self.rail_twist.sin() * dt;
            if self.rail_twist.sin().abs() > 0.7 { self.rail_side += dt; }
            self.yaw = f32::atan2(-travel.x, -travel.z) - self.rail_twist;
            self.normal = (Vec3::Y - travel * travel.y).normalize_or(Vec3::Y);
            self.grounded = false;
            self.air_time = 0.0;
            self.grind_time += dt;
            let hop = !input.jump && self.charge > 0.0;
            self.crouch(&input, dt);
            if hop || d <= 0.0 || d >= rail.len() || s.abs() < 0.7 {
                if hop {
                    // hop off, sideways if steering
                    let side = Vec3::new(-travel.z, 0.0, travel.x);
                    self.vel += Vec3::Y * self.jump_speed() + side * input.steer * 2.5;
                }
                self.take_off();
                self.rail = None;
                self.rail_cooldown = 0.45;
                self.end_grind();
            } else {
                self.rail = Some((ri, d, s));
            }
            return;
        }

        let speed = self.vel.length();
        let mut jumped = false;

        // the stick does not act at once
        self.steer += (input.steer - self.steer) * (dt / 0.12).min(1.0);
        // a full carve leans the surface's limit: sideways acceleration g * tan(lean); powder steers slower
        let lean = (surf[5] * 0.905).to_radians().tan() * if self.surface == 3 || self.surface == 4 { 0.6 } else { 1.0 };
        let turn_rate = surf[0] * 0.01 * lean / speed.max(TURN_FULL_SPEED) * if input.brake { 1.3 } else { 1.0 };

        if self.grounded {
            let n = self.normal;
            self.yaw -= self.steer * turn_rate * dt;

            // gravity along the slope
            self.vel += (g - n * g.dot(n)) * dt;

            // split into along-the-board and sideways; the edge kills the sideways part and turns it
            // into travel along the board (a carve costs no speed in the original)
            let f = self.forward();
            let l = n.cross(f).normalize_or(Vec3::X);
            let (mut vf, mut vl) = (self.vel.dot(f), self.vel.dot(l));
            let before = (vf * vf + vl * vl).sqrt();
            // how well the edge holds depends on the snow (ice hardly at all) and the rider's edging
            let grip = EDGE_GRIP * (surf[4] / 1.2).clamp(0.03, 2.5) * (0.5 + self.stats.edging);
            vl *= (-grip * if input.brake { 0.5 } else { 1.0 } * dt).exp();
            if !input.brake {
                let after = (vf * vf + vl * vl).sqrt();
                if after > 0.5 { vf *= before / after; vl *= before / after; }
            }
            let crouched = input.tuck || input.jump;
            // the rider pushes himself along when slow
            let push_to = surf[6] / 3.6;
            if !input.brake && vf.abs() < push_to { vf += (self.stats.push() * surf[7] * (push_to - vf.abs())).min(PUSH_MAX) * dt; }
            let mut cap = SPEED_CAP;
            if input.boost && self.boost > 0.0 {
                let tier = if self.boost > 0.666 || self.tricky() { 2 } else if self.boost > 0.334 { 1 } else { 0 };
                let level = [0.25, 0.6, 1.0][tier];
                vf += BOOST_ACCEL * level * (1.0 - self.steer.abs()).max(0.0) * dt;
                cap = BOOST_CAPS[tier];
                if !self.tricky() { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); }
                self.boosting = true;
            }
            self.vel = f * vf + l * vl;

            let s = self.vel.length();
            let mut k = surf[1] * self.stats.drag_linear() + 0.1 * s * surf[2] + 0.01 * s * s * surf[3] * self.stats.drag_cubic();
            if !crouched { k += DRAG_STAND; }
            if input.brake { k += self.stats.brake(); }
            self.vel /= 1.0 + k * dt;

            // the speed limit: up at once with a boost, back down slowly afterwards
            self.cap = cap.max(self.cap - CAP_DECAY * dt);
            let s = self.vel.length();
            if s > self.cap { self.vel *= self.cap / s; }

            if input.jump {
                self.crouch(&input, dt);
            } else if self.charge > 0.0 {
                self.vel += (n + f * 0.2).normalize() * self.jump_speed();
                self.take_off();
                jumped = true;
            } else {
                self.wind = 0.0;
                self.wind_flip = 0.0;
            }
        } else {
            self.vel.y -= if self.vel.y > 0.0 { GRAVITY_RISING } else { GRAVITY_FALLING } * dt;
            let h = 1.0 / (1.0 + AIR_DRAG * dt);
            self.vel.x *= h;
            self.vel.z *= h;
            let s = self.vel.length();
            if s > AIR_CAP { self.vel *= AIR_CAP / s; }
            // the first instant off the snow (a bump, a facet edge) still steers like the ground;
            // only real air time spins
            let spin = if self.air_time < 0.15 { turn_rate } else { self.air_rate(self.spin_rate) };
            let steer = if self.air_time < 0.15 { self.steer } else { input.steer };
            self.yaw -= steer * spin * dt;
            self.air_time += dt;
            if self.air_time >= 0.15 {
                self.spin += input.steer * spin * dt;
                let rate = self.air_rate(self.flip_rate);
                if input.flip != 0.0 {
                    self.flip += input.flip * rate * dt;
                    self.flip_dir = input.flip.signum();
                } else if self.flip_dir != 0.0 {
                    // let go and the body levels itself to the nearest upright, as in the original:
                    // short of half way it comes back, past half way it carries on round
                    let tau = std::f32::consts::TAU;
                    let goal = (self.flip / tau).round() * tau;
                    let err = goal - self.flip;
                    let step = (2.2 * err.abs()).clamp(0.8, rate.max(0.8)) * dt;
                    if err.abs() <= step { self.flip = goal; self.flip_dir = 0.0; } else { self.flip += err.signum() * step; }
                }
                if self.uber > 0.0 && !self.uber_done {
                    // an uber trick plays right through; it pays for its whole length when it ends
                    self.uber += dt * 30.0;
                    self.grab = 0;
                    let frames = self.ubers.get(self.uber_id).map_or(60.0, |u| u.1);
                    if self.uber >= frames - 1.0 {
                        self.uber_done = true;
                        self.ubers_done += 1;
                        let row = self.data.rows.iter().find(|r| self.ubers.get(self.uber_id).is_some_and(|u| u.0 == r.uber_clip));
                        self.air_pts += frames / 30.0 * PTS_HOLD * row.map_or(18.0, |r| r.uber_rate);
                        self.air_names.push(row.map_or("Uber".into(), |r| format!("UBER {}", r.uber)));
                    }
                } else if let Some(i) = COMBOS.iter().position(|c| *c == input.grab & 15) {
                    if self.grab != i as u8 + 1 { self.grab = i as u8 + 1; self.grab_hold = 0.0; }
                    self.grab_hold += dt;
                    let row = &self.data.rows[i];
                    if self.grab_hold >= GRAB_REACH {
                        if !self.grab_done[i] {
                            // each different grab in a jump starts higher than the last
                            let before = self.grab_done.iter().filter(|d| **d).count().min(3) as f32;
                            self.grab_done[i] = true;
                            self.air_pts += PTS_GRAB_START * (before + 1.0);
                            self.air_names.push(row.grab.to_string());
                        }
                        let uber = self.ubers.iter().position(|u| !row.uber_clip.is_empty() && u.0 == row.uber_clip);
                        if input.tweak && self.uber_ready() && self.uber == 0.0 && uber.is_some() {
                            self.uber = 0.01;
                            self.uber_done = false;
                            self.uber_id = uber.unwrap();
                        } else {
                            if input.tweak && !self.grab_tweaked[i] {
                                self.grab_tweaked[i] = true;
                                if let Some(n) = self.air_names.iter_mut().rev().find(|n| **n == row.grab) { *n = row.tweak.to_string(); }
                            }
                            self.air_pts += PTS_HOLD * row.rate * if input.tweak { 2.0 } else { 1.0 } * dt;
                        }
                    }
                } else {
                    self.grab = 0;
                    self.grab_hold = 0.0;
                    if self.uber_done { self.uber = 0.0; self.uber_done = false; }
                }
            }
            if input.jump { self.charge = (self.charge + dt / CHARGE_TIME).min(1.0); }
        }

        let mut p = self.pos + self.vel * dt;

        // walls: a sphere around the body
        let lift = Vec3::Y * (BODY_RADIUS + 0.35);
        let (c, contacts) = world.push_out(p + lift, BODY_RADIUS);
        p = c - lift;
        for n in contacts {
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                self.vel -= n * vn * (1.0 + WALL_BOUNCE);
                if vn < -WALL_CRASH && self.crashed <= 0.0 { self.crash_kind = if n.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 2 } else { 3 }; self.wipe_out(); }
            }
        }

        let s = self.vel.length();
        if self.grounded {
            // stay on the snow unless it falls away from under us (a lip, a cliff)
            let snap = 0.25 + s * dt * 0.4;
            match world.ground(p, 1.2, snap) {
                Some(hit) if !(s > 6.0 && self.vel.dot(hit.normal) / s > 0.17) => {
                    p.y = hit.y;
                    self.surface = hit.surface;
                    let vn = self.vel.dot(hit.normal);
                    self.vel -= hit.normal * vn;
                    // riding through a dip bends the direction of travel; it does not cost speed
                    let after = self.vel.length();
                    if vn < 0.0 && after > 0.5 { self.vel *= s / after; }
                    let blend = 1.0 - (-18.0 * dt).exp();
                    self.normal = self.normal.lerp(hit.normal, blend).normalize_or(hit.normal);
                }
                _ => { let c = self.charge; self.take_off(); self.charge = c; }
            }
        } else if !jumped {
            // landing: is there snow at or above our feet, and are we moving into it?
            if let Some(hit) = world.ground(p, 1.2, 0.02) {
                let vn = self.vel.dot(hit.normal);
                if vn <= 0.5 {
                    p.y = hit.y;
                    self.vel -= hit.normal * vn.min(0.0);
                    // the original turns the whole fall into speed along the slope (up to the limit)
                    let after = self.vel.length();
                    if after > 0.5 { self.vel *= s.min(self.cap.max(SPEED_CAP)) / after; }
                    self.normal = hit.normal;
                    self.surface = hit.surface;
                    self.grounded = true;
                    if self.air_time > 0.15 { self.touch_down(); }
                    self.land();
                }
            }
        }
        self.pos = p;

        // dropping onto a rail from the air locks the board onto it
        if !self.grounded && !jumped && self.rail_cooldown <= 0.0 && self.air_time > 0.08 && self.vel.y < 2.5 {
            if let Some((ri, d, _)) = rails.nearest(self.pos, RAIL_REACH) {
                let (rp, tan) = rails.0[ri].at(d);
                let along = self.vel.dot(tan);
                if along.abs() > 3.0 && along.abs() > 0.45 * s {
                    self.pos = rp;
                    // the board keeps the angle it came in at
                    let travel = tan * along.signum();
                    let mut tw = f32::atan2(-travel.x, -travel.z) - self.facing();
                    while tw > std::f32::consts::PI { tw -= std::f32::consts::TAU; }
                    while tw < -std::f32::consts::PI { tw += std::f32::consts::TAU; }
                    // the original keeps your speed along the rail, and at least 20 km/h of it
                    let along = along.signum() * s.min(along.abs().max(5.56));
                    self.rail = Some((ri, d, along));
                    self.charge = 0.0;
                    self.land();
                    self.crashed = 0.0;
                    self.rail_twist = tw;
                    self.rail_goal = (tw / std::f32::consts::FRAC_PI_2).round() * std::f32::consts::FRAC_PI_2;
                    self.rail_held = true;
                    self.rail_side_signed = 0.0;
                    self.switch = false;
                    self.rail_entry = along.abs();
                    self.rail_spun = 0.0;
                    self.rail_side = 0.0;
                }
            }
        }

        // remember somewhere sane to come back to, and come back if we fall out of the world
        if self.grounded && s > 3.0 {
            self.safe_timer += dt;
            if self.safe_timer > 1.5 { self.safe = (self.pos, self.yaw); self.safe_timer = 0.0; }
        }
        // out of the world, or into one of the course's out-of-bounds boxes: back to the last good spot
        if self.pos.y < world.min_y - 30.0 || (!self.grounded && self.air_time > 9.0) || (self.rail.is_none() && world.in_reset(self.pos + Vec3::Y * 0.8, BODY_RADIUS)) {
            let at = self.safe;
            self.respawn(at);
        }
    }
}

/// Follows the course's racing line. Used by the self-test and by the opponents.
pub struct AiDriver {
    pub idx: usize,
    last_idx: usize,
    stuck_for: f32,
    /// how far to the side of the line this rider likes to be (m)
    pub offset: f32,
    /// 0..1: how much of the time it tucks
    pub skill: f32,
    pub finished: Option<f32>,
    pub stuck: u32,
    clock: f32,
    /// seconds spent alongside the player, and until this rider may shove again
    pub beside: f32,
    seen_respawns: u32,
    /// how far above the snow the racing line is at each point (it flies over the gaps)
    over: Vec<f32>,
    slow_for: f32,
    pub shove_wait: f32,
    /// the trick picked for the current jump: spin direction and grab
    trick: (f32, u8),
    jumps: u32,
}
impl AiDriver {
    pub fn new(offset: f32, skill: f32) -> Self { Self { idx: 0, last_idx: 0, stuck_for: 0.0, offset, skill, finished: None, stuck: 0, clock: 0.0, beside: 0.0, seen_respawns: 0, over: Vec::new(), slow_for: 0.0, shove_wait: 4.0, trick: (0.0, 0), jumps: 0 } }
    pub fn progress(&self, line: &[Vec3]) -> f32 { if line.len() < 2 { 0.0 } else { self.idx as f32 / (line.len() - 1) as f32 } }
    /// Decide the controls for this step. Puts the rider back on the line if it wedges itself.
    /// `clear` is how far the rider is above the snow.
    /// Look the course over once: where does the line leave the ground?
    pub fn learn(&mut self, world: &CollisionWorld, line: &[Vec3]) {
        if self.over.len() == line.len() { return; }
        self.over = line.iter().map(|p| world.ground(*p + Vec3::Y * 2.0, 2.0, 60.0).map_or(60.0, |h| p.y - h.y)).collect();
    }
    pub fn drive(&mut self, r: &mut Rider, line: &[Vec3], clear: f32, dt: f32) -> Input {
        if line.len() < 4 { return Input::default(); }
        self.clock += dt;
        if self.finished.is_some() { return Input { brake: true, ..Input::default() }; }
        let mut best = (self.idx, f32::MAX);
        for i in self.idx..(self.idx + 12).min(line.len()) {
            let d = (line[i] - r.pos).length_squared();
            if d < best.1 { best = (i, d); }
        }
        self.idx = best.0;
        if self.idx + 2 >= line.len() { self.finished = Some(self.clock); }
        if self.idx > self.last_idx { self.last_idx = self.idx; self.stuck_for = 0.0; } else { self.stuck_for += dt; }
        // wedged (hardly moving for a couple of seconds) or going nowhere: back onto the line
        self.slow_for = if r.vel.length() < 1.5 && r.crashed <= 0.0 { self.slow_for + dt } else { 0.0 };
        if (self.stuck_for > 7.0 || self.slow_for > 2.5) && self.clock > 12.0 {
            if std::env::var("TRICKY_RESETS").is_ok() { println!("stuck at t {:.0}: pos {:.1?} speed {:.1} grounded {} line point {} at {:.1?}, next {:.1?}", self.clock, r.pos, r.vel.length(), r.grounded, self.idx, line[self.idx], line[(self.idx + 1).min(line.len() - 1)]); }
            self.idx = (self.idx + 2).min(line.len() - 1);
            let nxt = line[(self.idx + 1).min(line.len() - 1)] - line[self.idx];
            r.respawn((line[self.idx] + Vec3::Y * 1.0, f32::atan2(-nxt.x, -nxt.z)));
            self.stuck += 1;
            self.stuck_for = 0.0;
            self.slow_for = 0.0;
            self.seen_respawns = r.respawns;
        }
        // sent back by a reset zone or a fall: rejoin on the racing line, not wherever the trouble started
        if r.respawns != self.seen_respawns {
            if self.clock > 1.0 {
                let nxt = line[(self.idx + 1).min(line.len() - 1)] - line[self.idx];
                r.respawn((line[self.idx] + Vec3::Y * 1.0, f32::atan2(-nxt.x, -nxt.z)));
            }
            self.seen_respawns = r.respawns;
        }
        // aim further down the line the faster we go
        let speed = r.vel.length();
        let last = line.len() - 1;
        let mut ahead = self.idx + 1;
        let mut run = 0.0;
        while ahead < last && run < (speed * 0.9).max(14.0) { run += (line[ahead + 1] - line[ahead]).length(); ahead += 1; }
        // how sharply the course bends over the next stretch decides how fast it can be taken
        let dir = |i: usize| { let d = line[(i + 1).min(last)] - line[i.min(last)]; Vec3::new(d.x, 0.0, d.z).normalize_or_zero() };
        let (mut bend, mut len, mut i) = (0.0f32, 0.0f32, self.idx);
        while i < last && len < (speed * 2.2).max(30.0) { bend += dir(i).angle_between(dir(i + 1)); len += (line[i + 1] - line[i]).length(); i += 1; }
        let safe_speed = if bend > 0.05 { (TURN_ACCEL * 0.75 * len / bend).sqrt() } else { 99.0 };
        let a = line[ahead.min(last)];
        let b = line[(ahead + 1).min(last)];
        let along = (b - a).normalize_or(Vec3::NEG_Z);
        let target = a + Vec3::new(-along.z, 0.0, along.x) * self.offset;
        let to = target - r.pos;
        let want = f32::atan2(-to.x, -to.z);
        let mut diff = want - r.yaw;
        while diff > std::f32::consts::PI { diff -= std::f32::consts::TAU; }
        while diff < -std::f32::consts::PI { diff += std::f32::consts::TAU; }
        if !r.grounded {
            // in the air: with enough room, spin and grab, and let go in time to land
            if r.rail.is_some() { return Input::default(); }
            if r.air_time < 0.05 {
                self.jumps += 1;
                let n = self.jumps + (self.offset.abs() * 10.0) as u32;
                self.trick = (if n % 2 == 0 { 1.0 } else { -1.0 }, COMBOS[(n % 9) as usize]);
            }
            let room = clear > 3.5 || (r.vel.y > 1.0 && clear > 1.5);
            let half = (r.spin.abs() / std::f32::consts::PI).fract();
            let spinning = room || (r.spin.abs() > 0.5 && half > 0.12 && half < 0.88 && clear > 1.0);
            return Input { steer: if spinning && self.skill > 0.6 { self.trick.0 } else { 0.0 }, grab: if room { self.trick.1 } else { 0 }, ..Input::default() };
        }
        // tuck in bursts, more often the better the rider
        let steer = (-diff * 2.5).clamp(-1.0, 1.0);
        // a gap coming up (the line is in the air): everything into speed
        let gap = (self.idx + 1..(self.idx + 7).min(self.over.len())).any(|i| self.over[i] > 5.0) && self.over.get(self.idx).is_some_and(|o| *o < 5.0);
        let brake = !gap && (speed > safe_speed * 1.08 || (diff.abs() > 0.8 && speed > 12.0));
        let easy = gap || (speed < safe_speed * 0.85 && diff.abs() < 0.4);
        let tuck = gap || (easy && ((self.clock * 0.35 + self.offset).sin() * 0.5 + 0.5) < self.skill);
        Input { steer, tuck, brake, boost: easy && r.boost > 0.25 && steer.abs() < 0.3, ..Input::default() }
    }
}

/// Timing a run from the gate to the end of the course's main race line.
#[derive(Clone, Copy, PartialEq)]
pub enum RaceState { Countdown, Running, Finished }
pub struct Race { pub line: Vec<Vec3>, pub idx: usize, pub time: f32, pub state: RaceState, pub best: Option<f32>, pub countdown: f32 }
impl Race {
    pub fn new(line: Vec<Vec3>) -> Self { Self { line, idx: 0, time: 0.0, state: RaceState::Countdown, best: None, countdown: 3.4 } }
    pub fn restart(&mut self) { self.idx = 0; self.time = 0.0; self.state = RaceState::Countdown; self.countdown = 3.4; }
    pub fn progress(&self) -> f32 { if self.line.len() < 2 { 0.0 } else { self.idx as f32 / (self.line.len() - 1) as f32 } }
    pub fn update(&mut self, r: &Rider, dt: f32) {
        if self.line.len() < 4 { return; }
        match self.state {
            RaceState::Countdown => { self.countdown -= dt; if self.countdown <= 0.0 { self.state = RaceState::Running; } }
            RaceState::Running => {
                self.time += dt;
                // follow our place along the line; look a little ahead only, so a shortcut that
                // passes near a later part of the course does not count as being there
                let mut best = (self.idx, f32::MAX);
                for i in self.idx..(self.idx + 14).min(self.line.len()) {
                    let d = (self.line[i] - r.pos).length_squared();
                    if d < best.1 { best = (i, d); }
                }
                self.idx = best.0;
                let end = *self.line.last().unwrap();
                if self.idx + 2 >= self.line.len() || (self.progress() > 0.8 && (end - r.pos).length() < 35.0) {
                    self.state = RaceState::Finished;
                    self.best = Some(self.best.map_or(self.time, |b| b.min(self.time)));
                }
            }
            RaceState::Finished => {}
        }
    }
}

/// Third-person camera that trails the rider. Plain data so the self-test can measure it.
pub struct ChaseCam { pub pos: Vec3, dir: Vec3, boom: f32 }
impl Default for ChaseCam { fn default() -> Self { Self { pos: Vec3::ZERO, dir: Vec3::ZERO, boom: 99.0 } } }
impl ChaseCam {
    /// Returns (camera position, point to look at).
    pub fn update(&mut self, world: &CollisionWorld, r: &Rider, dt: f32) -> (Vec3, Vec3) {
        // look where we are going; fall back to where the board points when nearly stopped
        let flat = Vec3::new(r.vel.x, 0.0, r.vel.z);
        let want = if flat.length() > 3.0 { r.vel.normalize() } else { heading(r.yaw) };
        let k = 1.0 - (-4.0 * dt).exp();
        self.dir = if self.dir == Vec3::ZERO { want } else { self.dir.lerp(want, k).normalize_or(want) };
        let back = Vec3::new(self.dir.x, self.dir.y * 0.5, self.dir.z).normalize_or(heading(r.yaw));
        let head = r.pos + Vec3::Y * 1.5;
        let look = r.pos + Vec3::Y * 1.1 + back * 3.0;
        let mut target = r.pos - back * 6.5 + Vec3::Y * 2.6;
        // On a steep slope the snow behind the rider is higher than the rider. Keep the camera's
        // resting place above it. Searching down from just over head height means a roof far
        // overhead is not mistaken for ground.
        if let Some(h) = world.ground(Vec3::new(target.x, head.y + 3.0, target.z), 0.0, 40.0) {
            target.y = target.y.max(h.y + 1.7);
        }
        // If something solid is still in the way (a wall, a low roof), shorten the boom. Pull in
        // quickly, let out slowly, so a post flashing past does not make the view pump.
        let full = (target - head).length().max(0.1);
        let clear = world.raycast(head, target).map(|d| (d - 0.4).max(1.0)).unwrap_or(full);
        let rate = if clear < self.boom { 25.0 } else { 2.5 };
        self.boom = self.boom.min(full) + (clear - self.boom.min(full)) * (1.0 - (-rate * dt).exp());
        let target = head + (target - head) / full * self.boom.min(full);
        let snap = self.pos.distance(target) > 40.0;
        let mut pos = if snap { target } else { self.pos.lerp(target, 1.0 - (-10.0 * dt).exp()) };
        // last resort: never below the snow directly underneath
        if let Some(hit) = world.ground(pos, 0.7, 2.0) { pos.y = pos.y.max(hit.y + 0.4); }
        self.pos = pos;
        (pos, look)
    }
}

/// Headless check: ride the course on autopilot along the race line and report what happens.
pub fn self_test(world: &CollisionWorld, rails: &Rails, line: &[Vec3], start_yaw: f32, seconds: f32) {
    if std::env::var("TRICKY_TRICKTEST").is_ok() {
        let Some(&start) = line.first() else { return };
        let tau = std::f32::consts::TAU;
        // (name, seconds of flip, seconds of spin, seconds of grab, hold the grab into the landing?)
        for (name, flip_s, spin_s, grab_s, hold) in [
            ("one backflip, wound up", 1.0, 0.0, 0.0, false),
            ("360 spin + nose grab", 0.0, tau / AIR_SPIN, 0.8, false),
            ("flip started too late (lands upside down)", -1.0, 0.0, 0.0, false),
            ("grab held into the landing", 0.0, 0.0, 9.0, true),
            ("nothing", 0.0, 0.0, 0.0, false),
        ] {
            let mut r = Rider::new(start + Vec3::Y * 22.0, start_yaw);
            r.vel = heading(start_yaw) * 4.0;
            if flip_s > 0.0 { r.flip_rate = FLIP_WOUND; }
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            while t < 8.0 && !(r.grounded && t > 0.5) {
                let air = (r.air_time - 0.15).max(0.0);
                let input = Input {
                    flip: if (air < flip_s && flip_s > 0.0) || (flip_s < 0.0 && air > 0.7) { -1.0 } else { 0.0 },
                    steer: if air < spin_s && spin_s > 0.0 { 1.0 } else { 0.0 },
                    grab: if air < grab_s || hold { 1 } else { 0 },
                    ..Input::default()
                };
                r.step(world, rails, if r.grounded { Input::default() } else { input }, dt);
                t += dt;
            }
            println!("{name}: landed after {t:.2} s -> \"{}\"  score {}  boost {:.0}%{}", r.last_trick, r.score, r.boost * 100.0, if r.crashed > 0.0 { "  (wiped out)" } else { "" });
        }
        // uber tricks: a two-second signature move started with a full meter
        for (name, height, full) in [("uber with enough air", 95.0, true), ("uber without enough air", 12.0, true), ("uber without a full meter", 95.0, false)] {
            let mut r = Rider::new(start + Vec3::Y * height, start_yaw);
            r.ubers = vec![("bxUT_MTINDY".into(), 60.0), ("bxUT_OTHER".into(), 60.0)];
            if full { r.add_meter(1.0); } else { r.add_meter(0.4); }
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            while t < 9.0 && !(r.grounded && t > 0.5) {
                r.step(world, rails, if r.grounded { Input::default() } else { Input { grab: if r.air_time < 1.2 { 1 } else { 0 }, tweak: r.air_time > 0.6, ..Input::default() } }, dt);
                t += dt;
            }
            println!("{name}: landed after {t:.2} s -> \"{}\"  score {}  letters {}{}", r.last_trick, r.score, r.letters, if r.crashed > 0.0 { "  (wiped out)" } else { "" });
        }
        return;
    }
    if std::env::var("TRICKY_RAILTEST").is_ok() {
        // drop onto the five longest rails a little way in, moving along them, and see what happens
        let mut order: Vec<usize> = (0..rails.0.len()).collect();
        order.sort_by(|a, b| rails.0[*b].len().total_cmp(&rails.0[*a].len()));
        for &ri in order.iter().take(5) {
            let rail = &rails.0[ri];
            let (p, tan) = rail.at(rail.len() * 0.1);
            let dir = if tan.y <= 0.0 { tan } else { -tan };
            let mut r = Rider::new(p + Vec3::Y * 1.2, f32::atan2(-dir.x, -dir.z));
            r.vel = dir * 12.0;
            let (mut t, mut on, mut first, mut top) = (0.0f32, 0.0f32, None, 0.0f32);
            while t < 60.0 {
                let steer = if std::env::var("TRICKY_RAILSTEER").is_ok() && r.rail.is_some() && on < 0.5 { 1.0 } else { 0.0 };
                r.step(world, rails, Input { steer, ..Input::default() }, 1.0 / 120.0);
                t += 1.0 / 120.0;
                if r.rail.is_some() { on += 1.0 / 120.0; first.get_or_insert(t); top = top.max(r.vel.length()); }
                else if first.is_some() { break; }
            }
            println!("rail {ri}: {:.0} m long, locked on after {:.2} s, rode it for {:.1} s, top {:.0} km/h, left it {}, twist {:.0} deg, \"{}\"",
                rail.len(), first.unwrap_or(-1.0), on, top * 3.6, if r.grounded { "onto snow" } else { "into the air" }, r.rail_twist.to_degrees(), r.last_trick);
        }
        return;
    }
    if let Ok(pr) = std::env::var("TRICKY_PROBE") {
        let v: Vec<f32> = pr.split(',').filter_map(|x| x.parse().ok()).collect();
        let c = Vec3::new(v[0], v[1], v[2]);
        if let Some(h) = world.ground(c, 15.0, 15.0) { world.explain(Vec3::new(c.x, h.y + 0.8, c.z), BODY_RADIUS); }
        println!("heights relative to {c:?} (rows = z from -20 to +20, cols = x from -20 to +20, step 2 m; '..' = no rideable ground within +-15 m; W = wall contact at body height)");
        for dz in -10..=10 {
            let mut row = String::new();
            for dx in -10..=10 {
                let p = c + Vec3::new(dx as f32 * 2.0, 0.0, dz as f32 * 2.0);
                match world.ground(p, 15.0, 15.0) {
                    Some(h) => { let w = !world.push_out(Vec3::new(p.x, h.y + 0.8, p.z), BODY_RADIUS).1.is_empty(); row += &format!("{:4.0}{}", h.y - c.y, if w { "W" } else { " " }); }
                    None => row += "  .. ",
                }
            }
            println!("{row}");
        }
        for (i, p) in line.iter().enumerate().filter(|(_, p)| (**p - c).length() < 40.0) { println!("line {i}: {:?}", *p - c); }
        return;
    }
    let Some(&start) = line.first() else { println!("no race line"); return };
    let mut r = Rider::new(start + Vec3::Y * 1.0, start_yaw);
    let dt = 1.0 / 120.0;
    let (mut t, mut next_log, mut top, mut air, mut dist) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut finished = None;
    let mut cam = ChaseCam::default();
    let (mut prev_off, mut prev_d, mut jumps, mut frames, mut sub) = (Vec3::ZERO, Vec3::ZERO, Vec::<f32>::new(), 0u32, 0u32);
    let (mut was_grounded, mut flips) = (false, 0u32);
    let (mut prev_len, mut pumps, mut prev_resp, mut prev_vel, mut rider_jerk) = (0.0f32, 0u32, 0u32, Vec3::ZERO, Vec::<(f32, f32)>::new());
    let (mut stuck_for, stuck, mut last_idx) = (0.0f32, 0u32, 0usize);
    let mut grind = 0.0f32;
    let lane: Vec<f32> = std::env::var("TRICKY_LANE").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let mut ai = AiDriver::new(lane.first().copied().unwrap_or(0.0), lane.get(1).copied().unwrap_or(1.0));
    while t < seconds {
        ai.learn(world, line);
        let input = ai.drive(&mut r, line, 0.0, dt);
        if std::env::var("TRICKY_RESETS").is_ok() && world.in_reset(r.pos + Vec3::Y * 0.8, BODY_RADIUS) { println!("reset at t {t:.1} pos {:.1?} idx {} grounded {} speed {:.0}", r.pos, ai.idx, r.grounded, r.vel.length()); }
        if std::env::var("TRICKY_TRACE").is_ok() && ((t / 0.5) as i32) != (((t - dt) / 0.5) as i32) { println!("t {t:.1} pos {:.1?} vel {:.1?} speed {:.1} grounded {} normal {:.2?} surface {} steer {:.2} yaw {:.2} idx {}", r.pos, r.vel, r.vel.length(), r.grounded, r.normal, r.surface, input.steer, r.yaw, ai.idx); }
        let idx = ai.idx;
        let best = (idx, (line[idx.min(line.len() - 1)] - r.pos).length_squared());
        let before = r.pos;
        let air_before = r.air_time;
        r.step(world, rails, input, dt);
        if std::env::var("TRICKY_RESETS").is_ok() && (r.pos - before).length() > 3.0 { println!("put back at t {t:.1}: was at {:.1?} (line point {} at {:.1?}), air {:.1} s, {}", before, ai.idx, line[ai.idx.min(line.len() - 1)], air_before, if air_before > 8.9 { "in the air too long" } else if before.y < world.min_y - 29.0 { "below the world" } else { "reset zone or autopilot" }); }
        if r.rail.is_some() { grind += dt; }
        dist += (r.pos - before).length();
        if r.grounded != was_grounded { flips += 1; was_grounded = r.grounded; }
        // a 60 fps camera frame every second physics step: how unevenly does the view move?
        sub += 1;
        if sub % 2 == 0 {
            let (cp, _) = cam.update(world, &r, dt * 2.0);
            let off = cp - r.pos;
            let len = off.length();
            if frames > 2 && (len - prev_len).abs() > 0.25 && r.respawns == prev_resp { pumps += 1; }
            if frames > 2 { rider_jerk.push(((r.vel - prev_vel).length() * dt * 2.0, t)); }
            prev_len = len; prev_resp = r.respawns; prev_vel = r.vel;
            let d = off - prev_off;
            if frames > 2 { jumps.push((d - prev_d).length()); }
            prev_off = off; prev_d = d; frames += 1;
        }
        if !r.grounded { air += dt; }
        top = top.max(r.vel.length());
        t += dt;
        if t >= next_log {
            println!("t={:5.1}s  line {:3}/{}  off-line {:5.1} m  speed {:5.1} km/h  {}  pos ({:.0}, {:.0}, {:.0})",
                t, idx, line.len(), best.1.sqrt(), r.vel.length() * 3.6, if r.grounded { "ground" } else { "air   " }, r.pos.x, r.pos.y, r.pos.z);
            next_log += 10.0;
        }
        if ai.finished.is_some() { finished = Some(t); break; }
        let _ = (&mut stuck_for, &mut last_idx);
    }
    match finished {
        Some(t) => println!("FINISHED in {t:.1} s"),
        None => println!("did not finish in {seconds:.0} s (reached line point {}/{})", ai.idx, line.len()),
    }
    jumps.sort_by(|a, b| a.total_cmp(b));
    if !jumps.is_empty() {
        let q = |f: f32| jumps[((jumps.len() - 1) as f32 * f) as usize];
        println!("camera jerk per 60 fps frame (m): median {:.3}, 99% {:.3}, worst {:.2}; frames with a visible jolt (>0.15 m): {}",
            q(0.5), q(0.99), q(1.0), jumps.iter().filter(|j| **j > 0.15).count());
    }
    println!("frames where the camera distance jumped by more than 25 cm: {pumps}");
    println!("frames where the rider's own speed changed abruptly (>0.15 m/frame): {}", rider_jerk.iter().filter(|j| j.0 > 0.15).count());
    println!("ground/air switches: {flips} ({:.1} per second)", flips as f32 / t.max(1.0));
    println!("distance {:.0} m, top speed {:.0} km/h, airborne {:.0} s, on rails {:.0} s, autopilot stuck {} times, fell out of the world {} times", dist, top * 3.6, air, grind, ai.stuck + stuck, r.respawns - ai.stuck - stuck);
}

//! Snowboarder physics. Plain functions over plain data so the same code runs in the game and
//! in the headless self-test. Bevy space: metres, seconds, Y up.
//!
//! The speeds, gravity, drag, turning, jump and rotation numbers are the original game's.

use crate::collide::CollisionWorld;
use crate::rails::Rails;
use crate::trickdata::{RiderData, COMBOS, RIDERS};
use std::collections::VecDeque;
use bevy::math::Vec3;
#[path = "wipeout.rs"]
pub mod wipeout;
pub use wipeout::{Wipe, course_points};

// The numbers below are the original game's, read out of the decompiled SLUS_203.26 (converted from
// its cm and 1/60 s frames to metres and seconds), for a rider with middling stats on ordinary snow.
/// gravity on the snow (surface table row 1), and in the air: gentle on the way up, hard on the way down
pub const GRAVITY: f32 = 13.0;
pub const GRAVITY_RISING: f32 = 8.5024;
pub const GRAVITY_FALLING: f32 = 19.0084;
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
/// The ground spring's band per surface (m): sunk deeper than d1 it pushes back fully, d2 is how far a
/// board can sink (powder 15-35 cm). From SurfaceTable_Init 0x256188 rows [7] and [8].
pub const SURF_BAND: [(f32, f32); 20] = [(1.0, 1.17844), (0.005, 0.0250417), (0.005, 0.0308877), (0.150913, 0.297817), (0.25204, 0.356268),
    (0.005, 0.0225027), (0.1, 0.2), (0.138205, 0.302712), (0.143509, 0.235225), (0.005, 0.0153559), (0.1, 0.2), (0.005, 0.01),
    (0.005, 0.0632051), (0.005, 0.015127), (0.005, 0.0513762), (0.1, 0.2), (0.1, 0.2), (0.1, 0.2), (0.00501527, 0.0505194), (0.1, 0.2)];
pub const SPEED_CAP: f32 = 27.89;
pub const BOOST_CAPS: [f32; 3] = [29.32, 30.72, 33.47];
pub const AIR_CAP: f32 = 33.472;
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
pub const BOOST_SECONDS: f32 = 22.218;
/// how fast sideways slip is killed (1/s); this is what makes the board carve
pub const EDGE_GRIP: f32 = 7.0;
pub const BODY_RADIUS: f32 = 0.45;
pub const WALL_BOUNCE: f32 = 0.25;
/// landing further than this from upright is a wipe-out (about 100 degrees: the original is forgiving)
pub const CRASH_PITCH: f32 = 1.75;
/// rails do not carry you faster than this
pub const RAIL_CAP: f32 = 29.3;
/// how close (m) the board has to pass to a rail, while in the air, to lock on
pub const RAIL_REACH: f32 = 0.9;
pub const RAIL_FRICTION: f32 = 0.03;
/// how fast the board turns on a rail when steered (rad/s)
/// a quarter turn on a rail is its 15-frame clip: 28 ticks (`RailSlideControl_CommitQuarterTurn`)
pub const RAIL_TWIST: f32 = std::f32::consts::FRAC_PI_2 / (28.0 / 60.0);
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

/// The rest of the surface table: how far above the snow (cm) the board may be before it is in
/// the air, and how hard the snow damps the board's bounce (1/s). Indexed like `SURFACES`.
pub const SURF_EXTRA: [[f32; 2]; 20] = [
    [20.0, 30.0], [2.742, 5.009], [2.849, 5.530], [15.04, 2.842], [30.03, 2.976], [2.775, 4.052], [20.0, 30.0],
    [21.37, 45.2], [20.0, 39.4], [2.02, 30.0], [20.0, 30.0], [0.70, 17.9], [3.95, 30.0], [2.64, 0.0], [1.56, 30.0],
    [20.0, 30.0], [20.0, 30.0], [20.0, 30.0], [2.58, 40.0], [20.0, 30.0],
];

/// One axis of rotation in the air (`SpinState_Update`): rate, the band it is held in, and the
/// largest error the auto-complete may still use.
#[derive(Clone, Copy, Default)]
pub struct Axis { pub w: f32, hi: f32, lo: f32, floor: f32, emax: f32 }
impl Axis {
    fn start(w0: f32) -> Self { Self { w: 0.0, hi: w0, lo: 0.3 * w0, floor: w0.abs().min(1.396), emax: std::f32::consts::TAU } }
    /// The stick is let go for the first time: the rotation may slow to nothing but never reverse.
    fn let_go(&mut self) {
        if self.hi < 0.0 { self.hi = 0.0 } else { self.lo = 0.0 }
        self.floor = 0.0;
        self.emax = std::f32::consts::TAU;
    }
    fn step(&mut self, dir: f32, complete: bool, gain: f32, shrink: f32, dt: f32, err: impl Fn(f32) -> f32) -> f32 {
        let side = (self.hi + self.lo).signum();
        if !complete {
            let t = dir * (self.hi + self.lo).abs();
            self.w += (t - self.w).clamp(-75.0 * dt, 75.0 * dt);
        } else {
            let e = err(side);
            self.emax = self.emax.min(e.abs());
            self.w = gain * e.clamp(-self.emax, self.emax);
        }
        self.hi *= shrink;
        self.lo *= shrink;
        if side > 0.0 { self.hi = self.hi.max(self.floor); self.lo = self.lo.max(self.floor); }
        else if side < 0.0 { self.hi = self.hi.min(-self.floor); self.lo = self.lo.min(-self.floor); }
        self.w = self.w.clamp(self.lo.min(self.hi), self.lo.max(self.hi));
        self.w * dt
    }
}

/// A rider's attributes as fractions (0 to 1), as the game's formulas use them.
#[derive(Clone, Copy)]
pub struct Stats { pub edging: f32, pub speed: f32, pub stability: f32, pub tricks: f32, pub jump: f32, pub windup: f32, pub weight: f32, pub alpine: bool,
    /// board class (0 BX, 1 freestyle, 2 alpine) and the meter-leak attribute (not in our tables yet: 0.5)
    pub kind: u8, pub meter_leak: f32,
    /// the fixed gate-start stat (stat byte 7, the same on every board): how fast the rider winds
    /// up in the gate and pushes out
    pub gate: f32 }
impl Default for Stats {
    fn default() -> Self { Self { edging: 0.5, speed: 0.5, stability: 0.5, tricks: 0.5, jump: 0.75, windup: 0.75, weight: 75.0, alpine: false, kind: 1, meter_leak: 0.5, gate: 0.7 } }
}
impl Stats {
    /// A new rider or a fully trained one, on the chosen board: the game's own mix of 80% rider
    /// and 20% board.
    /// `training` 0..1: from the character's starting attributes to its caps (career points / 240
    /// fill every bar in the original; computer riders gain a quarter of the player's points).
    pub fn of(d: &RiderData, training: f32, board: &crate::trickdata::Board) -> Self {
        let t = training.clamp(0.0, 1.0);
        let f = |v: (u8, u8), g: usize| (0.8 * (v.0 as f32 + (v.1 as f32 - v.0 as f32) * t) + 0.2 * board.bonus[g] as f32) / 100.0;
        Self { edging: f(d.edging, 0), speed: f(d.speed, 1), stability: f(d.stability, 2), tricks: f(d.tricks, 3),
               jump: d.jump as f32 / 100.0, windup: d.windup as f32 / 100.0, weight: d.weight as f32, alpine: board.kind == 2, kind: board.kind, meter_leak: 0.5,
               gate: match d.name { "Kaori" | "Marisol" => 0.80, "Luther" => 0.64, "Mac" => 0.85, "Moby" | "Seeiah" => 0.75, "Zoe" => 0.82, "JP" => 0.88, _ => 0.70 } }
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
    /// a computer rider's wind-up stick, apart from its steering (None: the stick is the steering)
    pub wind: Option<f32>,
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
    /// the safe spot is kept by the course (the AI path network), not by the rider
    pub safe_external: bool,
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
    pub grab_hold: f32,
    /// a grab let go of, still playing out its clip: (grab, frame)
    pub grab_out: Option<(u8, f32)>,
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
    /// a computer rider's taunt at the player (`bxRT_AITAUNT*`, `bxRT_TAUNTHS*/TS*`): seconds into it, and the clip
    pub taunt: f32,
    pub taunt_clip: String,
    /// over the line (`FinishState_Update` 0x106668): 0 braking, 1 coasting to a stop, 2 standing up
    /// and reacting; seconds since the line; whether it was a win (place < (N+1)/2)
    pub fin: Option<(u8, f32, bool)>,
    /// a show-off event: the course's pickups (multipliers, boost, time) only work in show-off
    pub showoff: bool,
    /// tricks landed since the trick book last looked (a human rider's only)
    pub landed: Vec<String>,
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
    /// seconds left recovering from a bad landing, and from a hard (but clean) one
    pub crashed: f32,
    pub hard_land: f32,
    /// analog crouch and brake (0..1), the crouch kept for a jump that is about to fire, and how
    /// long until it fires
    pub crouch_v: f32,
    pub brake_v: f32,
    jump_c: f32,
    jump_delay: f32,
    /// the board's height above the snow (m, along the normal) from the last probe
    height: f32,
    /// which way the course goes here (the rider pushes himself along only roughly that way)
    pub course_dir: Option<Vec3>,
    /// the race line near the rider and 8 m further along it (the original's 0x350 / 0x360): the
    /// tumble is pulled toward the first, and gets up facing the second
    pub course_pts: Option<(Vec3, Vec3)>,
    /// the wipe-out body (motion 5 / 6), and where and against what the crash happened (point,
    /// normal) for its first bounce
    pub wipe: Option<Wipe>,
    pub crash_at: Option<(Vec3, Vec3)>,
    /// back up without a get-up clip (CT_CRUNCH2BASE / STRETCH2BASE, A_FALLRECOVER): the clip id
    /// and seconds into it, for the body
    pub recover: Option<(u16, f32)>,
    /// a computer rider (put back on its feet rather than reset)
    pub ai: bool,
    /// a wipe-out (motion 5/6): seconds down, seconds of it in contact with the snow, seconds into
    /// getting up (negative while still tumbling), and whether it lasted so long the rider is put back
    pub down_t: f32,
    pub getup: f32,
    /// a stumble (hit a wall or a rival without going down): seconds left
    pub stumble: f32,
    /// the last wall knock this step: the body sphere's centre and the speed going in (m/s)
    pub wall_hit: Option<(Vec3, f32)>,
    /// speed-boost pickup (script op 0x11): seconds of full boost without the meter; spin-boost
    /// (op 0x12): seconds in which a takeoff spins and flips 1.6x as fast
    pub speed_timer: f32,
    pub spin_timer: f32,
    /// Megaplex tube: which way the rider came in (picks the way out), MAX when not in it
    pub tube_class: u8,
    /// the start gate (cGateAnticipateControl / cGateLaunchControl): progress through the gate clip,
    /// the highest and lowest it has been, how long it has sat still, where it is heading, the
    /// launch speed (m/s), and the state (8 waiting, 9 launching, 0 out)
    pub gate_p: f32, gate_hi: f32, gate_lo: f32, gate_still: f32, gate_target: f32, gate_speed: f32, pub gate_state: u8, gate_rock_dir: i32,
    /// the ground spring band in use (eases toward the surface's)
    band: (f32, f32),
    /// a "Late" trick: the rotation already banked in this jump, and that the next part is late
    spin_base: f32,
    flip_base: f32,
    late: bool,
    spin_used: bool,
    /// what put the rider down last (for the headless checks)
    pub cause: &'static str,
    /// rotation in the air: spin and flip, and whether the stick has been let go
    pub spin_ax: Axis,
    pub flip_ax: Axis,
    air_mode2: bool,
    /// on a rail: sideways offset from it (m) and seconds on it
    pub rail_off: f32,
    rail_time: f32,
    rail_halves: u32,
    boost_was: bool,
    /// tricks landed in a row (and rails over a second): two or more pay a chain bonus
    chain: u32,
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
               spawn: (pos, yaw), safe: (pos, yaw), safe_timer: 0.0, safe_external: false, respawns: 0, rail: None, rail_cooldown: 0.0, input: Input::default(),
               spin: 0.0, flip: 0.0, flip_dir: 0.0, wind: 0.0, wind_flip: 0.0, spin_rate: AIR_SPIN, flip_rate: FLIP_RATE, cap: SPEED_CAP, steer: 0.0, grab: 0, grab_hold: 0.0, grab_out: None, grab_done: [false; 16], grab_tweaked: [false; 16], air_pts: 0.0, air_names: Vec::new(), ubers_done: 0, switch_takeoff: false, from_rail: false, rail_pts: 0.0, recent: VecDeque::new(), uber_timer: 0.0, stats: Stats::default(), data: &RIDERS[0], surface: 1, grind_time: 0.0, rail_twist: 0.0, rail_spun: 0.0, rail_side: 0.0, rail_entry: 0.0, rail_goal: 0.0, rail_held: false, rail_side_signed: 0.0, shove: 0.0, shove_side: 1.0, taunt: -1.0, taunt_clip: String::new(), fin: None, showoff: false, landed: Vec::new(), crash_kind: 0, land_kind: 0, switch: false, score: 0, boost: 0.0, boosting: false,
               last_trick: String::new(), trick_timer: 0.0, crashed: 0.0, hard_land: 0.0, crouch_v: 0.0, brake_v: 0.0, jump_c: 0.0, jump_delay: 0.0, height: 0.0,
               course_dir: None, course_pts: None, wipe: None, crash_at: None, recover: None, ai: false, down_t: 0.0, getup: -1.0, stumble: 0.0, wall_hit: None, speed_timer: 0.0, spin_timer: 0.0, spin_used: false, tube_class: u8::MAX, gate_p: 0.0, gate_hi: 0.0, gate_lo: 0.0, gate_still: 0.0, gate_target: 0.0, gate_speed: 0.0, gate_state: 8, gate_rock_dir: 0, band: (0.005, 0.0250417), spin_base: 0.0, flip_base: 0.0, late: false, cause: "", spin_ax: Axis::default(), flip_ax: Axis::default(), air_mode2: false, rail_off: 0.0, rail_time: 0.0, rail_halves: 0, boost_was: false, chain: 0,
               ubers: Vec::new(), uber: 0.0, uber_id: 0, uber_done: false, letters: 0, multiplier: 1 }
    }
    /// `Boarder_Respawn`: dropped a metre above the snow, already moving down the course at 30 km/h.
    pub fn respawn(&mut self, at: (Vec3, f32)) {
        self.pos = at.0 + Vec3::Y * 1.0146;
        self.yaw = at.1;
        self.fin = None;
        self.vel = heading(at.1) * 8.3333;
        self.crashed = 0.0;
        self.getup = -1.0;
        self.wipe = None;
        self.recover = None;
        self.stumble = 0.0;
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
        self.spin_base = 0.0;
        self.flip_base = 0.0;
        self.late = false;
        self.flip_dir = 0.0;
        self.grab = 0;
        self.grab_hold = 0.0;
        self.grab_out = None;
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
        let _ = was;
        self.boost = (self.boost + delta * self.stats.meter()).clamp(0.0, 1.0);
        // any gain at a full meter (re)opens the twenty-second uber window
        if delta > 0.0 && self.boost >= 1.0 { self.uber_timer = UBER_SECONDS; }
    }
    /// `Boarder_Wipeout`: down into the tumble; costs a tenth of the meter (`Score_Crash`), and a
    /// running uber window closes with the meter cut to two thirds.
    fn wipe_out(&mut self) { self.go_down(true); }
    /// A wipe-out on demand (TRICKY_CRASHAT checks): as if the landing went wrong.
    pub fn force_wipe(&mut self) { self.cause = "forced"; self.crash_kind = 0; self.wipe_out(); }
    fn go_down(&mut self, lose_meter: bool) {
        if self.crashed > 0.0 { return; }
        self.last_trick = "CRASH".into();
        self.trick_timer = 2.0;
        // a crash in the middle of a trick in the uber window cuts the meter to two thirds and closes it
        let mid_trick = !self.grounded && (self.air_pts > 0.0 || self.spin.abs() > 1.0 || self.flip.abs() > 1.0 || !self.air_names.is_empty());
        if !self.tricky() && self.uber_timer > 0.0 && mid_trick { self.boost = self.boost.min(0.66599226); self.uber_timer = 0.0; }
        if lose_meter { self.add_meter(-0.1); }
        self.crashed = 1.0;
        self.down_t = 0.0;
        self.getup = -1.0;
        self.wipe = None;
        self.recover = None;
        self.rail = None;
        self.charge = 0.0;
        self.jump_c = 0.0;
        self.jump_delay = 0.0;
        self.uber = 0.0;
        self.grab = 0;
    }
    /// `Boarder_Stumble`: a knock that does not put you down (state 2): a moment off balance.
    pub fn stumble_by(&mut self, meter: bool) {
        if self.crashed > 0.0 || self.stumble > 0.0 { return; }
        if meter { self.add_meter(-0.02); }
        self.stumble = 0.5;
    }
    /// Bank a trick: the original halves (thirds, ...) the pay of one repeated among the last five.
    fn award(&mut self, names: Vec<String>, base: f32, mult: f32) { self.award_with(names, base, mult, true, 0.0) }
    /// `bonus` (chains, air time) is added after the repeat division and is not multiplied.
    fn award_with(&mut self, names: Vec<String>, base: f32, mult: f32, repeat_check: bool, bonus: f32) {
        // nothing is banked once over the line (Score_Land checks the finish time)
        if names.is_empty() || base + bonus < 5.0 || self.fin.is_some() { return; }
        let key = names.join(" + ");
        let repeats = if repeat_check { self.recent.iter().filter(|k| **k == key).count() as f32 } else { 0.0 };
        if repeat_check {
            self.recent.push_back(key.clone());
            if self.recent.len() > 5 { self.recent.pop_front(); }
        }
        let pts = (((base * mult / (repeats + 1.0) + bonus) / 10.0).round() * 10.0) as u32;
        self.score += pts;
        // the meter gets the unmultiplied points (never negative); computer riders get none from tricks
        if !self.ai { self.add_meter(base.max(0.0) / 10000.0 / (repeats + 1.0)); }
        let note = if mult > 1.01 { format!("  x{mult:.1}").replace(".0", "") } else { String::new() };
        self.last_trick = format!("{key}{note}  +{pts}");
        self.trick_timer = 3.0;
    }
    /// Score_Takeoff in mid-air: post and pay for the trick so far, start counting again.
    fn bank_late(&mut self) {
        use std::f32::consts::{PI, TAU};
        let (sp, fl) = (self.spin - self.spin_base, self.flip - self.flip_base);
        let grabs: Vec<String> = self.air_names.drain(..).collect();
        let name = trick_name(-sp, fl, self.switch_takeoff && !self.late, self.from_rail && !self.late, false, false, &grabs);
        if !name.is_empty() {
            let pts = self.air_pts + (sp.abs() / PI).round() * PTS_SPIN_180 + (fl.abs() / TAU).round() * PTS_FLIP;
            let name = if self.late { format!("Late {name}") } else { name };
            if !self.ai { self.landed.push(name.clone()); }
            self.award_with(vec![name], pts, self.multiplier as f32, !self.late, 0.0);
            self.chain += 1;
        }
        self.air_pts = 0.0;
        self.grab_done = [false; 16];
        self.grab_tweaked = [false; 16];
        self.spin_base = (self.spin / PI).round() * PI;
        self.flip_base = (self.flip / TAU).round() * TAU;
        self.late = true;
    }
    /// Called when the board touches down (snow or rail): score what was done in the air.
    fn land(&mut self) {
        // the spin-boost is spent on the jump it was used for
        if self.spin_used { self.spin_timer = 0.0; self.spin_used = false; }
        let half_turns = ((self.spin - self.spin_base).abs() / std::f32::consts::PI).round() as u32;
        let flips = ((self.flip - self.flip_base).abs() / std::f32::consts::TAU).round() as u32;
        let tau = std::f32::consts::TAU;
        let off = (self.flip.rem_euclid(tau)).min(tau - self.flip.rem_euclid(tau));
        let uber_cut_short = self.uber > 0.0 && !self.uber_done;
        if off > self.stats.crash_pitch() || !self.grab_safe() || uber_cut_short {
            // too far from upright, or still holding the board: wipe out
            self.crash_kind = if self.flip < -0.5 { 1 } else { 0 };
            if std::env::var("TRICKY_AIDBG").is_ok() { println!("    landing: flip {:.2} off {:.2} limit {:.2} spin {:.2} grab {} hold {:.2} uber {}", self.flip, off, self.stats.crash_pitch(), self.spin, self.grab, self.grab_hold, uber_cut_short); }
            self.cause = if off > self.stats.crash_pitch() { "landing angle" } else if uber_cut_short { "uber" } else { "grab" };
            self.wipe_out();
        } else {
            let mut pts = self.air_pts;
            if half_turns > 0 { pts += half_turns as f32 * PTS_SPIN_180; }
            if flips > 0 { pts += flips as f32 * PTS_FLIP; }
            let grabs: Vec<String> = self.air_names.drain(..).collect();
            let onto_rail = self.rail.is_some();
            // the game's spin sign is the other way round to ours (positive = backside)
            let name = trick_name(-(self.spin - self.spin_base), self.flip - self.flip_base, self.switch_takeoff && !self.late, self.from_rail && !self.late, onto_rail, self.switch != self.switch_takeoff, &grabs);
            // the part after a mid-air restart is "Late" and never counts as a repeat
            let name = if self.late && !name.is_empty() { format!("Late {name}") } else { name };
            if !name.is_empty() && !self.ai && self.fin.is_none() { self.landed.push(name.clone()); }
            let mut names: Vec<String> = if name.is_empty() { Vec::new() } else { vec![name.clone()] };
            // air time from 4 s pays 1000 a second; chains of tricks (and rails over a second) pay
            // on the landing that ends them; neither is multiplied
            let mut bonus = if self.air_time >= 4.0 { (self.air_time - 3.0).floor() * 1000.0 } else { 0.0 };
            let big_air = bonus > 0.0;
            if !names.is_empty() {
                if self.from_rail { pts += PTS_OFF_RAIL; }
                if onto_rail { pts += PTS_ONTO_RAIL; }
                self.chain += 1;
            }
            if !onto_rail {
                if self.chain >= 2 {
                    bonus += [4000.0, 8000.0, 12000.0, 16000.0][(self.chain.min(5) - 2) as usize];
                    // the original's labels: a chain of two is "1x COMBO" ... five and more "4+ COMBO"
                    names.push(["1x COMBO", "2x COMBO", "3x COMBO", "4+ COMBO"][(self.chain.min(5) - 2) as usize].to_string());
                }
                self.chain = 0;
            }
            if !names.is_empty() || bonus > 0.0 {
                // x1.3 only for a trick off a rail left switch; a pickup replaces it
                let mult = (self.multiplier as f32).max(if self.from_rail && self.switch_takeoff { SWITCH_BONUS } else { 1.0 });
                if big_air { names.push("BIG AIR BONUS".into()); }
                // "???" (beyond the tables) never counts as a repeat
                let check = name != "???" && !self.late;
                self.award_with(names, pts, mult, check, bonus);
            }
            self.letters = (self.letters + self.ubers_done).min(6);
            self.multiplier = 1;
        }
        self.from_rail = false;
        self.clear_air();
    }
    pub fn pick_multiplier(&mut self, m: u32) {
        // the original counts multipliers only in the air or on a rail
        if self.grounded || !self.showoff { return; }
        self.multiplier = self.multiplier.max(m);
        self.last_trick = format!("x{m} on your next trick");
        self.trick_timer = 2.0;
    }
    /// The gold speed-boost pickup: five seconds of full boost (`Boarder_SetSpeedBoostTimer`).
    pub fn pick_speed(&mut self, secs: f32) {
        self.speed_timer = self.speed_timer.max(secs);
        self.last_trick = "SPEED BOOST".into();
        self.trick_timer = 2.0;
    }
    /// The red/green trick-boost pickup: fifteen seconds of faster spins (`Boarder_SetSpinBoostZone`).
    pub fn pick_spin(&mut self, secs: f32) {
        self.spin_timer = self.spin_timer.max(secs);
        self.last_trick = "TRICK BOOST".into();
        self.trick_timer = 2.0;
    }
    /// Script op 0xf: straight onto the meter.
    pub fn pick_boost(&mut self, amount: f32) { self.add_meter(amount); }
    /// Six landed uber tricks spell TRICKY: boost no longer runs out.
    pub fn tricky(&self) -> bool { self.letters >= 6 }
    /// Uber tricks can be started (the meter was filled in the last twenty seconds, or TRICKY).
    pub fn uber_ready(&self) -> bool { self.uber_timer > 0.0 || self.tricky() }
    fn end_grind(&mut self) {
        if self.grind_time > 0.0 && self.rail_pts > 0.0 {
            let turns = self.rail_halves;
            let slide = self.rail_side > self.grind_time * 0.5;
            let mut names = vec![format!("{:.1} s {}", self.grind_time, if !slide { if self.rail_twist.cos() < -0.5 { "Switch 50/50 Rail" } else { "50/50 Rail" } } else if self.rail_side_signed > 0.0 { "BS Rail" } else { "FS Rail" })];
            if turns > 0 { names.push(format!("{} on the rail", turns * 180)); }
            // (the half turns are already in the rail points); rail tricks are never "repeats"
            let pts = self.rail_pts;
            self.award_with(names, pts, 1.0, false, 0.0);
            if self.grind_time > 1.0 { self.chain += 1; }
            self.from_rail = true;
        }
        self.rail_halves = 0;
        self.grind_time = 0.0;
        self.rail_pts = 0.0;
        self.rail_spun = 0.0;
        self.rail_side = 0.0;
    }

    /// The board meets the snow. The original only scales the speed for a crooked or tilted
    /// landing (edge grip then turns the board); a board coming down tail-first is turned round
    /// and the rider is switch.
    fn touch_down(&mut self) {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let wrap = |mut a: f32| { while a > PI { a -= TAU; } while a < -PI { a += TAU; } a };
        let hv = Vec3::new(self.vel.x, 0.0, self.vel.z);
        self.land_kind = 0;
        if hv.length() > 2.0 {
            let travel = f32::atan2(-hv.x, -hv.z);
            let mut d = wrap(self.yaw - travel);
            if d.abs() > FRAC_PI_2 {
                self.switch = !self.switch;
                self.yaw += PI;
                d = wrap(d + PI);
            }
            self.vel *= 1.1136 - 0.2603 * d.abs().clamp(25f32.to_radians(), 80f32.to_radians());
            if d.abs() >= 25f32.to_radians() { self.land_kind = if d > 0.0 { 1 } else { 2 }; }
        }
        let off = self.flip.rem_euclid(TAU).min(TAU - self.flip.rem_euclid(TAU));
        if off > 25f32.to_radians() { self.land_kind = 3; }
        self.vel *= 1.0643 - 0.2454 * off.clamp(15f32.to_radians(), 50f32.to_radians());
    }
    /// Shoved over by another rider.
    /// The original's rider hit (`Boarder_RiderHit`): `dv` is the velocity change from the hit.
    /// Over 7.34 m/s puts you down (a rail rider always comes off), over 1.67 m/s is a stumble.
    /// Returns true if it put the rider down.
    pub fn take_hit(&mut self, dv: Vec3, lose_meter: bool) -> bool {
        if self.crashed > 0.0 { return false; }
        let m = dv.length();
        let applied = if m > 5.0 { dv * (5.0 / m) } else { dv };
        self.vel += if self.grounded { applied - self.normal * applied.dot(self.normal) } else { applied };
        let airborne_up = !self.grounded && self.rail.is_none() && dv.y > 0.0;
        if (m > 7.3397 || self.rail.is_some()) && !airborne_up {
            self.crash_kind = if dv.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 3 } else { 2 };
            self.go_down(lose_meter);
            return true;
        }
        if m > 1.6732 { self.stumble_by(true); }
        false
    }
    /// Crossing the line puts the rider in the finish state, whoever is riding.
    pub fn start_finish(&mut self, win: bool) {
        if self.fin.is_none() { self.fin = Some((if self.vel.length() > 2.7778 { 0 } else { 1 }, 0.0, win)); }
    }
    /// The finish state's controls: hard on the brake, then nothing (the pad is ignored).
    pub fn finish_input(&self, input: Input) -> Input {
        match self.fin {
            Some((0, ..)) => Input { brake: true, ..Default::default() },
            Some(_) => Input::default(),
            None => input,
        }
    }
    /// After the physics step: brake until under 10 km/h, then lose 7% a tick to a stop and stand
    /// there (each stage gives up after 5 s).
    pub fn finish_after(&mut self, dt: f32) {
        let Some((sub, t, win)) = self.fin else { return };
        let t = t + dt;
        let timeout = t > 301.0 / 60.0;
        let sub = match sub {
            0 if self.vel.length() < 2.7778 || timeout || self.crashed > 0.0 => 1,
            1 => {
                if self.grounded { self.vel *= 0.93f32.powf(dt * 60.0); }
                if (self.grounded && self.vel.length() < 0.3) || timeout { 2 } else { 1 }
            }
            s => s,
        };
        if sub >= 2 && self.crashed <= 0.0 { self.vel = Vec3::ZERO; }
        self.fin = Some((sub, t, win));
    }
    /// `Boarder_IsFinishedAndStopped`: stopped, and 4 s since the line.
    pub fn finished_and_stopped(&self) -> bool { matches!(self.fin, Some((s, t, _)) if s >= 2 && t > 4.0) }
    pub fn knock_down(&mut self, push: Vec3) {
        if self.crashed > 0.0 { return; }
        self.crash_kind = if push.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 3 } else { 2 };
        self.vel += push;
        self.go_down(false);
    }
    /// Which way the body faces: the board's heading, turned round when riding switch.
    pub fn facing(&self) -> f32 { if self.switch { self.yaw + std::f32::consts::PI } else { self.yaw } }
    fn jump_speed(&self, c: f32) -> f32 { self.stats.jump(c, self.vel.length()) }
    /// Leaving the snow (`SpinState_Enter`): the wind-up decides the rotation band for this jump.
    fn take_off(&mut self) {
        // a spin-boost makes this takeoff's rotation 1.6x as fast
        self.spin_used = self.spin_timer > 0.0;
        let k = 11.517 * self.stats.spin_scale() * if self.spin_used { 1.6 } else { 1.0 };
        let fakie = if self.switch { 0.8 } else { 1.0 };
        let w_s = k * self.wind * fakie * if self.stats.alpine { 2.0 / 3.0 } else { 1.0 };
        let w_f = k * 2.0 / 3.0 * self.wind_flip * fakie;
        self.spin_ax = Axis::start(w_s);
        self.flip_ax = Axis::start(w_f);
        self.air_mode2 = false;
        self.switch_takeoff = self.switch;
        self.wind = 0.0;
        self.wind_flip = 0.0;
        self.charge = 0.0;
        self.grounded = false;
        self.air_time = 0.0;
    }
    /// Crouching for a jump (`PrewindState_Update`): the crouch and the wind-up follow their targets
    /// at the original's rates.
    fn prewind(&mut self, input: &Input, dt: f32) {
        // twice as quick on a rail (0x40800846 = 4.001 there)
        let rate = if self.rail.is_some() { 4.001 } else { 2.0005 } * (0.812 + 0.329 * self.stats.windup);
        let slew = |cur: &mut f32, target: f32| { let step = (target - *cur).abs() * rate * dt; *cur += (target - *cur).clamp(-step, step); };
        slew(&mut self.wind, input.wind.unwrap_or(input.steer));
        slew(&mut self.wind_flip, input.flip);
    }
    /// Letting go of the jump button: tiny wind-ups count for nothing, and a wind-up that is
    /// nearly all spin or all flip is made pure.
    fn release_windup(&mut self) {
        if self.wind.abs() < 0.2 { self.wind = 0.0; }
        if self.wind_flip.abs() < 0.2 { self.wind_flip = 0.0; }
        let a = self.wind_flip.abs().atan2(self.wind.abs());
        if a > 1.3962 { self.wind = 0.0; } else if a < 0.17457 { self.wind_flip = 0.0; }
    }

    /// The trick row the held shoulder buttons select, if any.
    pub fn grab_row(&self) -> Option<&'static crate::trickdata::Row> { (self.grab as usize).checked_sub(1).and_then(|i| self.data.rows.get(i)) }

    /// Board direction along the current surface.
    /// Which way the rider's feet-to-head points: the snow's normal when riding, tipped by the
    /// flip in the air.
    pub fn board_up(&self) -> Vec3 {
        if self.grounded || self.rail.is_some() { return self.normal; }
        let f = heading(self.yaw);
        Vec3::Y * self.flip.cos() - f * self.flip.sin()
    }
    pub fn forward(&self) -> Vec3 {
        let h = heading(self.yaw);
        (h - self.normal * h.dot(self.normal)).normalize_or(h)
    }
    /// Boost thrust level from the meter (1, 0.6, 0.25) when boosting, else 0.
    pub fn boost_level(&self) -> f32 { if self.boost > 0.666 || self.tricky() { 1.0 } else if self.boost > 0.3336 { 0.6013 } else { 0.25 } }

    pub fn step(&mut self, world: &CollisionWorld, rails: &Rails, input: Input, dt: f32) {
        let mut input = input;
        // the meter leaks slowly (human riders); the uber window runs down everywhere, but never
        // runs out in mid-air
        if self.tricky() { self.boost = 1.0; self.uber_timer = 1.0; }
        else {
            self.boost = (self.boost - (15.4835 - 15.2104 * self.stats.meter_leak) * 5.5e-4 * dt).max(0.0);
            let floor = if !self.grounded && self.rail.is_none() && self.uber_timer > 0.0 { 1.0 / 60.0 } else { 0.0 };
            self.uber_timer = (self.uber_timer - dt).max(floor);
        }
        self.trick_timer = (self.trick_timer - dt).max(0.0);
        self.hard_land = (self.hard_land - dt).max(0.0);
        self.stumble = (self.stumble - dt).max(0.0);
        self.speed_timer = (self.speed_timer - dt).max(0.0);
        self.spin_timer = (self.spin_timer - dt).max(0.0);
        if let Some((c, t)) = self.recover { self.recover = if t + dt < wipeout::clip_frames(c, self.stats.kind) / 30.0 { Some((c, t + dt)) } else { None }; }
        if self.crashed > 0.0 {
            self.wipe_step(world, dt);
            self.input = Input::default();
            return;
        }
        if self.stumble > 0.0 { input.jump = false; input.tuck = false; input.boost = false; }
        self.shove = (self.shove - dt).max(0.0);
        if self.taunt >= 0.0 { self.taunt += dt; if self.taunt > 2.0 || self.crashed > 0.0 || !self.grounded { self.taunt = -1.0; } }
        if self.crashed > 0.0 { input = Input { steer: input.steer * 0.3, ..Input::default() }; }
        if self.hard_land > 0.0 { input.jump = false; input.tuck = false; }
        self.input = input;
        self.boosting = false;
        self.rail_cooldown = (self.rail_cooldown - dt).max(0.0);
        let surf = SURFACES[(self.surface as usize).min(19)];
        let extra = SURF_EXTRA[(self.surface as usize).min(19)];
        let gmag = surf[0] * 0.01;

        // ---- input shaping (cBoarder::vf7): each value moves toward its target at a limited rate
        let speed0 = self.vel.length();
        let sped = self.speed_timer > 0.0;
        let boosting_input = (input.boost && self.boost > 0.0) || sped;
        let lim = if boosting_input { 1.0 } else { 0.905 };
        let steer_target = input.steer.clamp(-lim, lim) * (speed0 / 11.35).min(1.0);
        let err = steer_target - self.steer;
        let rate = (7.017 * err.abs()).clamp(0.1, 8.018) * if self.surface == 3 || self.surface == 4 { 0.6 } else { 1.0 };
        self.steer += err.clamp(-rate * dt, rate * dt);
        let crouch_target = if input.tuck || input.jump { 1.0 } else { 0.0 };
        let e = crouch_target - self.crouch_v;
        let r = if self.jump_delay > 0.0 { 13.333 } else { 5.0025 * e.abs().max(0.1) };
        self.crouch_v += e.clamp(-r * dt, r * dt);
        let brake_target = if input.brake && self.crouch_v <= 0.0 { 1.0 } else { 0.0 };
        let e = brake_target - self.brake_v;
        let r = if brake_target > 0.0 { 5.035 * e.abs().max(0.1) } else { 5.035 * (1.1 - e.abs()).max(0.1) };
        self.brake_v += e.clamp(-r * dt, r * dt);
        if input.jump && self.grounded { self.charge = self.crouch_v; }

        // ---- grinding a rail
        if self.rail.is_some() {
            self.rail_step(rails, input, dt);
            return;
        }

        let mut jumped = false;
        let mut p = self.pos;
        if self.grounded {
            // ---- riding (cBoarder_GroundMotion_Update), Bevy space: metres, Y up
            let n = self.normal;
            let f = self.forward();
            let left = n.cross(f).normalize_or(Vec3::X);
            let v = self.vel;
            let (vf, vl, vn) = (v.dot(f), v.dot(left), v.dot(n));
            // spring holding the board on the snow
            let h = self.height;
            let damp = extra[1];
            // the spring band follows the surface (SurfaceTable rows [7] [8]), easing at 1 m/s
            let band = SURF_BAND[(self.surface as usize).min(19)];
            self.band.0 += (band.0 - self.band.0).clamp(-dt, dt);
            self.band.1 += (band.1 - self.band.1).clamp(-dt, dt);
            let (d1, d2) = self.band;
            let f_n = if h > 0.0 { -(gmag / 0.30) * h - if vn > 0.0 { damp * vn } else { 0.0 } }
                else if h > -d1 { -gmag * h / d1 - damp * vn }
                else { gmag * (1.0 - 2.0 * (h.max(-d2) + d1) / (d2 - d1)) - damp * vn };
            // the lean: steering tips the support force sideways (the original's positive steer is left)
            let s_left = -self.steer;
            let phi = (surf[5] * s_left).to_radians();
            let m = f_n.min(2.0 * gmag);
            let lean = n * (f_n - m / phi.cos() + m) + left * (m * phi.tan());
            let lvl = if sped { 1.0 } else if boosting_input { self.boost_level() } else { 0.0 };
            // drag along the board, heavier under load
            let load = (f_n / gmag).max(1.0);
            let st = &self.stats;
            let afv = vf.abs();
            // riding switch on a BX or alpine board takes 15% / 30% off the stat terms
            let sw = if self.switch { match st.kind { 1 => 1.0, 0 => 0.84998363, _ => 0.69987833 } } else { 1.0 };
            // in powder the drag depends on how deep the board sits
            let mul = if self.surface == 3 || self.surface == 4 { 0.5 - h / d2 } else { 1.0 };
            let drag = -vf * mul * (load * surf[1] * st.drag_linear() * sw
                + (1.0 - self.crouch_v) * 0.10573
                + (1.0 + 1.2154 * lvl) * st.brake() * sw * self.brake_v * self.brake_v
                + afv * 0.1 * load * (surf[2] + afv * 0.1 * surf[3] * st.drag_cubic() * sw));
            // pushing along: toward the course direction only, harder the slower he goes
            let mut thrust = 0.0;
            if self.brake_v <= 0.0 {
                let deficit = (surf[6] / 3.6 - speed0).min(11.11);
                if deficit > 0.0 {
                    let ang = match self.course_dir {
                        Some(c) => {
                            let a = f32::atan2(-c.x, -c.z) - self.yaw;
                            let a = (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                            ((60f32.to_radians() - a.abs()) / 30f32.to_radians()).clamp(0.0, 1.0)
                        }
                        None => 1.0,
                    };
                    let mut push = surf[7] * st.push() * ang * deficit;
                    if self.crouch_v > 0.5 && self.steer.abs() < 0.2 && speed0 < 8.33 { push = push.max(0.2 * st.push() * deficit); }
                    thrust += push;
                }
            }
            // boost: only when going (nearly) straight
            if boosting_input {
                thrust += lvl * (0.07983 - self.steer.abs()).max(0.0) * (23.50 + 10.53 * f.y.max(0.0)) * surf[7];
                if !self.tricky() && !sped { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); }
                self.boosting = true;
            }
            // the edge resists sideways slip (weakly: the board turns by yawing after the velocity)
            let x = afv;
            let curve = if x < 5.556 { 0.20104 + 0.088997 * x } else if x < 13.889 { 0.69547 + 0.03613 * (x - 5.556) } else { (0.99655 - 0.010441 * (x - 13.889)).max(0.0) };
            let switch_k = if self.switch { match st.kind { 1 => 1.0, 0 => 0.84998, _ => 0.69988 } } else { 1.0 };
            let side = -vl * curve * surf[4] * (0.001 + 1.1412 * st.edging) / (1.0 + 3.5 * lvl) * switch_k;
            let mut a = lean + f * (drag + thrust) + left * side - Vec3::Y * gmag;
            if h < -d2 {
                p -= n * (h + d2).max(-0.10);
                self.vel -= n * vn;
                let an = a.dot(n);
                if an < 0.0 { a -= n * an; }
            }
            p += self.vel * dt;
            self.vel += a * dt;

            // the board yaws round after the direction of travel
            let v = self.vel;
            let sp = v.length();
            if sp > 0.05 {
                let (ac, b) = match st.kind { 2 => (0.20834, 0.26221), 0 => (0.30019, 0.34919), _ => (0.4, 0.52398) };
                let theta = (v.cross(f).dot(n) / sp).clamp(-0.99999, 0.99999).asin();
                let mut tgt = -theta + s_left * b * (1.0 + ac / 2.0 * (1.0 - s_left * s_left)) / (1.0 + 0.01 * self.crouch_v);
                if v.dot(f) < 0.0 { tgt = -tgt; }
                let fall = (Vec3::NEG_Y - n * (-n.y)).normalize_or_zero();
                let gg = ((sp / 5.556).min(1.0) * (0.5 - (v / sp).dot(fall).abs()) * 40.0).max(0.0).min(1.0) + 0.01;
                let k = (0.2003 * sp * sp * dt).min(1.0) * gg.max(if tgt > 0.0 { s_left } else { -s_left });
                let cap = 6f32.to_radians() * dt * 60.0;
                self.yaw += (tgt * k).clamp(-cap, cap);
            }
            // riding backwards slowly: the board swings round and he rides switch
            if self.vel.dot(self.forward()) < -1.11 && self.brake_v <= 0.0 && self.crashed <= 0.0 {
                self.yaw += std::f32::consts::PI;
                self.switch = !self.switch;
                self.steer = -self.steer;
            }

            // jump: letting go of the crouch springs off once the crouch has unwound
            if input.jump {
                self.prewind(&input, dt);
                self.jump_delay = 0.0;
            } else if self.charge > 0.0 && self.jump_delay <= 0.0 && self.jump_c <= 0.0 {
                self.jump_c = self.charge;
                self.jump_delay = (self.charge / 13.333).max(1e-4);
                self.release_windup();
            } else if self.jump_delay > 0.0 {
                self.jump_delay -= dt;
                if self.jump_delay <= 0.0 {
                    let c = self.jump_c;
                    self.vel += (n + f * 0.2).normalize() * self.jump_speed(c);
                    self.jump_c = 0.0;
                    self.take_off();
                    jumped = true;
                }
            } else if self.jump_c > 0.0 {
                // a jump released during a moment in the air (a bump) fires on touching down
                let c = self.jump_c;
                self.vel += (n + f * 0.2).normalize() * self.jump_speed(c);
                self.jump_c = 0.0;
                self.take_off();
                jumped = true;
            } else {
                self.wind = 0.0;
                self.wind_flip = 0.0;
                self.charge = 0.0;
            }
            // the speed limit (vf7): up at once with a boost, back down slowly afterwards
            // the boosting cap goes by the meter left (vf7), the speed-boost pickup by its own timer
            let m = self.boost;
            let target = if sped || (self.boosting && (m > 0.66599226 || self.tricky())) { BOOST_CAPS[2] } else if self.boosting && m > 0.33362994 { BOOST_CAPS[1] } else if self.boosting { BOOST_CAPS[0] } else { SPEED_CAP };
            self.cap = target.max(self.cap - CAP_DECAY * dt);
            let s = self.vel.length();
            if s > self.cap { self.vel *= self.cap / s; }
        } else {
            // ---- in the air (Air_IntegrateRK4 ballistics, SpinState_Update rotation)
            self.vel.y -= if self.vel.y > 0.0 { GRAVITY_RISING } else { GRAVITY_FALLING } * dt;
            let h = 1.0 / (1.0 + AIR_DRAG * dt);
            self.vel.x *= h;
            self.vel.z *= h;
            let s = self.vel.length();
            if s > AIR_CAP { self.vel *= AIR_CAP / s; }
            p += self.vel * dt;
            self.air_time += dt;
            if !input.jump && self.air_time < 0.15 && (self.charge > 0.0 || self.jump_c > 0.0) {
                let c = if self.jump_c > 0.0 { self.jump_c } else { self.charge };
                if self.jump_c <= 0.0 { self.release_windup(); }
                let n = self.normal;
                let f = self.forward();
                self.vel += (n + f * 0.2).normalize() * self.jump_speed(c);
                self.jump_c = 0.0;
                self.jump_delay = 0.0;
                self.take_off();
                jumped = true;
            }
            self.air_rotation(&input, dt);
            self.air_grabs(&input, dt);
            if input.jump { self.charge = self.crouch_v; }
        }

        // walls: a sphere around the body (Boarder_WallCollide: bounce off at least 0.56 m/s)
        let lift = Vec3::Y * (BODY_RADIUS + 0.35);
        let (c, contacts) = world.push_out(p + lift, BODY_RADIUS);
        p = c - lift;
        for n in contacts {
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                let vin = -vn;
                if self.wall_hit.is_none_or(|w| w.1 < self.vel.length()) { self.wall_hit = Some((c, self.vel.length())); }
                let side = if n.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 2 } else { 3 };
                let up = self.board_up();
                if self.grounded {
                    // Boarder_WallImpactCheck: s = max(vin/2, 0.56); over 8.33(up.n)+19.44 m/s is a crash,
                    // over 8.33(up.n) a stumble (only a real knock, not a brush)
                    let s = (0.5 * vin).max(0.5556);
                    let un = up.dot(n);
                    if s > 8.3333 * un + 19.4444 { self.crash_kind = side; self.cause = "wall"; self.crash_at = Some((c - n * BODY_RADIUS, n)); self.wipe_out(); }
                    else if s > 8.3333 * un && vin > 4.0 { self.stumble_by(true); }
                    self.vel += n * (vin + (0.5 * vin).max(0.556));
                } else if vin > 1.0 && n.y < 0.7 && self.air_time > 0.15 {
                    // in the air: glance off anything close to the way your feet point, else go down
                    let f = (0.6 + 0.0135 * vin).min(0.9);
                    // (inside the Megaplex tube the glass only steers the rider)
                    if up.dot(n) > f || self.tube_class != u8::MAX { self.vel += n * vin; }
                    else { if std::env::var("TRICKY_CRASHDBG").is_ok() { println!("air wall vin {vin:.2} n {n:.2?} up {up:.2?} flip {:.2} air {:.2}", self.flip, self.air_time); } self.crash_at = Some((c - n * BODY_RADIUS, n)); self.crash_kind = side; self.cause = "air wall"; self.wipe_out(); }
                } else {
                    self.vel += n * (vin + (0.5 * vin).max(0.556));
                }
            }
        }

        let s = self.vel.length();
        if self.grounded {
            // the probe (Boarder_GroundProbe): a ray from 2 m above to 1 m below along the normal
            match world.ground(p, 2.0, 1.0) {
                Some(hit) => {
                    let h = (p.y - hit.y) * hit.normal.y;
                    self.surface = hit.surface;
                    let air_h = SURF_EXTRA[(hit.surface as usize).min(19)][0] * 0.01;
                    if h > air_h && !jumped {
                        self.height = h;
                        let c = self.charge;
                        self.take_off();
                        self.charge = c;
                    } else {
                        self.height = h;
                        self.normal = hit.normal;
                        // speed-keeping bleed of the normal velocity (not in powder)
                        if hit.surface != 3 && hit.surface != 4 {
                            let before = self.vel.length();
                            self.vel -= hit.normal * (0.4 * self.vel.dot(hit.normal));
                            let after = self.vel.length();
                            if after > 1e-3 { self.vel *= before / after; }
                        }
                    }
                }
                None => { let c = self.charge; self.take_off(); self.charge = c; }
            }
        } else if !jumped {
            // landing: snow at or above our feet, and moving into it
            if let Some(hit) = world.ground(p, 1.2, 0.02) {
                let vn = self.vel.dot(hit.normal);
                if vn <= 0.5 {
                    p.y = hit.y;
                    let impact = vn;
                    self.vel -= hit.normal * vn.min(0.0);
                    // the fall becomes speed along the slope
                    let after = self.vel.length();
                    if after > 0.5 { self.vel *= s.min(self.cap.max(SPEED_CAP)) / after; }
                    self.normal = hit.normal;
                    self.surface = hit.surface;
                    self.height = 0.0;
                    self.grounded = true;
                    if self.air_time > 0.15 {
                        self.touch_down();
                        // a hard landing: too nose-down or too fast into the snow
                        let tau = std::f32::consts::TAU;
                        let r = self.flip - tau * (self.flip / tau).round();
                        if r < -std::f32::consts::FRAC_PI_4 || impact < -27.77 { self.hard_land = 0.5; }
                    }
                    self.land();
                    self.steer = 0.0;
                }
            }
        }
        self.pos = p;
        // the surface underneath: 0 is out of bounds (back onto the course, no charge), 6 (bounce /
        // unskiable) and 10 (wall) put the rider down (BoarderState3_Ride_Input, ground motion)
        if self.grounded && self.crashed <= 0.0 && self.rail.is_none() {
            match self.surface {
                0 => { let at = self.safe; self.respawn(at); return; }
                6 | 10 if self.tube_class == u8::MAX => { self.cause = "surface"; self.crash_kind = 0; self.wipe_out(); }
                _ => {}
            }
        }

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
                    self.rail_off = 0.0;
                    self.rail_time = 0.0;
                    self.rail_halves = 0;
                }
            }
        }

        self.boost_was = input.boost;
        // remember somewhere sane to come back to, and come back if we fall out of the world
        if self.grounded && s > 3.0 {
            self.safe_timer += dt;
            if self.safe_timer > 1.5 && !self.safe_external && self.surface != 0 { self.safe = (self.pos, self.yaw); self.safe_timer = 0.0; }
        }
        // out of the world, or into one of the course's out-of-bounds boxes: back to the last good spot
        if self.pos.y < world.min_y - 30.0 || (!self.grounded && self.air_time > 9.0) || (self.rail.is_none() && world.in_reset(self.pos + Vec3::Y * 0.8, BODY_RADIUS)) {
            let at = self.safe;
            self.respawn(at);
        }
    }

    /// Spin and flip in the air, as `SpinState_Update` does it. Holding the stick drives the
    /// rotation inside a band that narrows through the jump; letting go finishes it: forwards to
    /// the next half turn (spin) or full turn (flip), back only when within 40 degrees.
    fn air_rotation(&mut self, input: &Input, dt: f32) {
        use std::f32::consts::{PI, TAU};
        let dir_s = if input.steer > 0.5 { 1.0 } else if input.steer < -0.5 { -1.0 } else { 0.0 };
        let dir_f = if input.flip > 0.5 { 1.0 } else if input.flip < -0.5 { -1.0 } else { 0.0 };
        let k = self.stats.spin_scale();
        let shrink = (0.99301 + 0.00201 * self.stats.tricks).powf(dt * 60.0);
        if !self.air_mode2 && dir_s == 0.0 && dir_f == 0.0 && self.air_time > dt * 1.5 {
            // first moment with the stick let go: from now on the rotation completes itself
            self.air_mode2 = true;
            for ax in [&mut self.spin_ax, &mut self.flip_ax] { ax.let_go(); }
        }
        if self.air_mode2 && self.grab == 0 && self.uber == 0.0 && (dir_s != 0.0 || dir_f != 0.0) && self.spin_ax.w.abs() < 0.7 && self.flip_ax.w.abs() < 0.7 {
            // SpinState_StartFromStick: a fresh press once everything has settled starts a new
            // (unwound) rotation; what was done so far is banked and the rest is a "Late" trick
            let w0 = 5.2531 * k;
            self.spin_ax = Axis::start(dir_s * w0);
            self.flip_ax = Axis::start(dir_f * w0);
            self.air_mode2 = false;
            self.bank_late();
        }
        let gain = 2.509 * k;
        let mode2 = self.air_mode2;
        let spin_err = |a: f32, w_dir: f32| {
            let e = (a + PI).rem_euclid(TAU) - PI;
            if e.abs() < 40f32.to_radians() { -e }
            else if e.abs() > 140f32.to_radians() { PI * e.signum() - e }
            else { let next = if w_dir >= 0.0 { (a / PI).ceil() * PI } else { (a / PI).floor() * PI }; next - a }
        };
        let flip_err = |a: f32, w_dir: f32| {
            let e = (a + PI).rem_euclid(TAU) - PI;
            if e.abs() < 40f32.to_radians() { -e }
            else { let next = if w_dir >= 0.0 { (a / TAU).ceil() * TAU } else { (a / TAU).floor() * TAU }; next - a }
        };
        let spin = self.spin;
        let flip = self.flip;
        let ds = self.spin_ax.step(dir_s, mode2, gain, shrink, dt, |w| spin_err(spin, w));
        let df = self.flip_ax.step(dir_f, mode2, gain, shrink, dt, |w| flip_err(flip, w));
        self.spin += ds;
        self.yaw -= ds;
        self.flip += df;
        self.flip_dir = if self.flip_ax.w.abs() > 0.05 { self.flip_ax.w.signum() } else { 0.0 };
    }

    /// Grabs, tweaks and uber tricks while in the air.
    fn air_grabs(&mut self, input: &Input, dt: f32) {
        if self.air_time < 0.15 { return; }
        if self.uber > 0.0 && !self.uber_done {
            // an uber trick plays right through; it pays for its whole length when it ends
            self.uber += dt * 30.0;
            self.grab = 0;
            let frames = self.ubers.get(self.uber_id).map_or(60.0, |u| u.1);
            if self.uber >= frames - 1.0 {
                self.uber_done = true;
                self.ubers_done += 1;
                let clip = self.ubers.get(self.uber_id).map(|u| u.0.clone()).unwrap_or_default();
                let sig = crate::trickdata::signature_grab(self.data.name);
                let row = self.data.rows.iter().find(|r| (!r.uber_clip.is_empty() && !clip.contains("UT_SIG") && uber_grab(r.uber_clip) == uber_grab(&clip)) || (clip.contains("UT_SIG") && Some(r.grab) == sig));
                self.air_pts += frames / 30.0 * PTS_HOLD * row.map_or(18.0, |r| if r.uber_rate > 0.0 { r.uber_rate } else { 18.0 });
                // the uber takes the grab's place in the name (each rider's own uber names)
                let name = row.map_or("Uber".to_string(), |r| crate::trickdata::uber_name(self.data.name, r.grab).unwrap_or(r.uber).to_string());
                match row.and_then(|r| self.air_names.iter().rposition(|n| n == r.grab || n == r.tweak)) {
                    Some(i) => self.air_names[i] = name,
                    None => self.air_names.push(name),
                }
            }
        } else if let Some(i) = COMBOS.iter().position(|c| *c == input.grab & 15) {
            let rate = 30.0 * (0.78212035 + 0.8057834 * self.stats.tricks);
            if self.grab != i as u8 + 1 { self.grab = i as u8 + 1; self.grab_hold = 0.0; self.grab_out = None; }
            let row = &self.data.rows[i];
            let (m1, m2, _, _) = grab_markers(row.clip);
            // the grab clip reaches in and holds at its second marker (grab clips play at
            // 0.782 + 0.806 x tricks of normal speed, Grab_Update); the grab counts from marker 1
            self.grab_hold = (self.grab_hold + rate * dt).min(m2);
            if self.grab_hold >= m1 {
                if !self.grab_done[i] {
                    // each different grab in a jump starts higher than the last
                    let before = self.grab_done.iter().filter(|d| **d).count().min(3) as f32;
                    self.grab_done[i] = true;
                    self.air_pts += PTS_GRAB_START * (before + 1.0);
                    self.air_names.push(row.grab.to_string());
                }
                // the signature uber (bxUT_SIG...) starts from the grab the trick book gives it
                let sig = crate::trickdata::signature_grab(self.data.name) == Some(row.grab);
                // each board type has its own set (bxUT_, frUT_, exUT_ + style + grab): match on the grab
                let uber = self.ubers.iter().position(|u| (!row.uber_clip.is_empty() && uber_grab(&u.0) == uber_grab(row.uber_clip) && !u.0.contains("UT_SIG")) || (sig && u.0.contains("UT_SIG")));
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
            // let go: the clip plays out from where it was; landing before it is past its
            // fourth marker is a crash
            if self.grab != 0 { self.grab_out = Some((self.grab, self.grab_hold)); }
            self.grab = 0;
            self.grab_hold = 0.0;
            if let Some((g, f)) = self.grab_out {
                let rate = 30.0 * (0.78212035 + 0.8057834 * self.stats.tricks);
                let clip = self.data.rows[(g as usize - 1).min(14)].clip;
                let n = grab_markers(clip).3;
                self.grab_out = if f + rate * dt >= n - 1.0 { None } else { Some((g, f + rate * dt)) };
            }
            if self.uber_done { self.uber = 0.0; self.uber_done = false; }
        }
    }
    /// Is the board free to land (no grab clip short of its safe marker)?
    pub fn grab_safe(&self) -> bool {
        if self.grab != 0 { return false; }
        match self.grab_out {
            Some((g, f)) => f >= grab_markers(self.data.rows[(g as usize - 1).min(14)].clip).2,
            None => true,
        }
    }

    /// Riding a rail (`RailSlideMotion_Update` / `RailSlideControl_Update`). The stick balances:
    /// for the first 0.6 s the board pulls itself to the middle of the rail, after that it drifts
    /// off one side unless you lean against it, and too far off you fall off (not a crash). Boost
    /// held with left / right turns the board a quarter turn (regular, sideways, fakie). There is
    /// no friction and no speed limit: a slow rider on an uphill rail rolls back.
    fn rail_step(&mut self, rails: &Rails, input: Input, dt: f32) {
        use std::f32::consts::FRAC_PI_2 as Q;
        let Some((ri, mut d, mut s)) = self.rail else { return };
        self.rail_time += dt;
        let rail = &rails.0[ri];
        let (_, tan) = rail.at(d);
        s += -9.8 * tan.y * dt;
        // boost: a kick per press; held only while crouched
        let pressed = input.boost && !self.boost_was;
        if self.boost > 0.0 && (pressed || (input.boost && self.charge > 0.0)) {
            let level = self.boost_level();
            if pressed { s += s.signum() * 0.408 * level; if !self.tricky() { self.boost = (self.boost - 0.00075).max(0.0); } }
            else { s += s.signum() * 24.508 * level * dt; if !self.tricky() { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); } }
            self.boosting = true;
        }
        self.boost_was = input.boost;
        let sc = s.abs() * 100.0;
        self.rail_pts += PTS_RAIL * dt * if sc >= 800.0 { 1.0 } else if sc < 50.0 { 0.0 } else { sc / (3200.0 - 3.0 * sc) };
        d += s * dt;
        let (p, tan) = rail.at(d);
        let travel = tan * if s >= 0.0 { 1.0 } else { -1.0 };
        let left = Vec3::Y.cross(travel).normalize_or(Vec3::X);

        // balance: off > 0 is to the left of the rail; steering right pushes back to the left... of centre
        let v = s.abs();
        let g = if v < 5.5556 { 0.5 } else { 0.09 * v };
        let gc = g.clamp(0.5, 2.0);
        let hh = (1.0 / (g * g)).clamp(1.0, 4.0);
        let mut k = if self.rail_time < 0.6 || tan.y.abs() > 0.92 { -5.0 } else { 1.8153523 };
        if self.rail_off.abs() < 0.025 && k > -1.0 { k = -1.0; }
        let o = if k >= 0.0 { self.rail_off.clamp(-0.2, 0.2) } else { self.rail_off };
        let stick = if input.boost { 0.0 } else { self.steer };
        self.rail_off += (k * hh * o - stick * 1.4507674 * gc) * dt;

        // quarter turns: boost held plus left / right; holding keeps turning
        let want = input.boost && input.steer.abs() > 0.5 && self.charge <= 0.0;
        let arrived = (self.rail_goal - self.rail_twist).abs() < 0.03;
        if want && (!self.rail_held || arrived) { self.rail_goal = (self.rail_goal / Q).round() * Q + input.steer.signum() * Q; }
        self.rail_held = want;
        let turn = (self.rail_goal - self.rail_twist).clamp(-RAIL_TWIST * dt, RAIL_TWIST * dt);
        self.rail_twist += turn;
        self.rail_spun += turn.abs();
        // each new furthest half turn round pays a half spin
        let halves = (self.rail_twist / std::f32::consts::PI).round().abs() as u32;
        if halves > self.rail_halves { self.rail_halves = halves; self.rail_pts += PTS_SPIN_180; }
        self.rail_side_signed += self.rail_twist.sin() * dt;
        if self.rail_twist.sin().abs() > 0.7 { self.rail_side += dt; }

        self.pos = p + left * self.rail_off;
        self.vel = tan * s;
        self.yaw = f32::atan2(-travel.x, -travel.z) - self.rail_twist;
        self.normal = (Vec3::Y - travel * travel.y).normalize_or(Vec3::Y);
        self.grounded = false;
        self.air_time = 0.0;
        self.grind_time += dt;

        // falling off: the allowed offset narrows after the first 0.6 s, wider for slides
        let w = if self.rail_time < 0.6 { 1.0 } else { (1.0 - (self.rail_time - 0.6) * 1.6667).max(0.0) };
        let slide = self.rail_twist.sin().abs() > 0.7;
        let limit = if slide { 1.5 * w + 0.8 * (1.0 - w) } else { 0.5 * w + 0.3 * (1.0 - w) };
        let fell = self.rail_off.abs() > limit;
        let hop = !input.jump && self.charge > 0.0;
        if input.jump { self.prewind(&input, dt); self.charge = self.crouch_v; }
        if hop || fell || d <= 0.0 || d >= rail.len() {
            if hop {
                // Jump_ApplyImpulse on a rail: straight along the board's up, which the balance
                // offset tips about the rail (50-50: 0.01162 rad per cm, slides: 0.008725 rad per cm)
                let c = self.charge;
                let phi = (self.rail_off * 100.0 * if slide { 0.698 * 0.0125 } else { 0.349 * 0.0333 }).clamp(-0.8, 0.8);
                let up = Vec3::Y * phi.cos() + left * self.rail_off.signum() * phi.abs().sin();
                self.vel += up * self.jump_speed(c);
                self.release_windup();
            }
            if fell { self.vel += left * self.rail_off.signum() * 2.778; }
            // leaving fakie (or switch) gives the next air trick the switch bonus
            let fakie = self.rail_twist.cos() < -0.5;
            self.take_off();
            if fakie { self.switch_takeoff = true; self.switch = true; }
            self.rail = None;
            self.rail_cooldown = 0.45;
            self.end_grind();
        } else {
            self.rail = Some((ri, d, s));
        }
    }

}

/// What the course's AI paths tell the opponents, per point of the racing line: the speed to ride
/// at (m/s) and where to crouch for a jump (let go where the zone ends).
#[derive(Default, Clone)]
pub struct AiCourse { pub speed: Vec<f32>, pub crouch: Vec<bool>, /// the jump zone's value at each point (bit 0 flip, bit 1 spin), or -1
    pub jump: Vec<i32> }
impl AiCourse {
    /// Speed and jump zones along one AI path from its own events (distances along the path):
    /// the target speed is the one in force 6 m ahead, a jump zone counts from 3 m before it.
    pub fn from_path(l: &crate::course::Line, from: usize) -> Self {
        let mut speeds: Vec<&crate::course::Event> = l.events.iter().filter(|e| e.kind == 101).collect();
        speeds.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut c = AiCourse::default();
        for i in from..l.pts.len() {
            let a = l.at[i];
            let v = speeds.iter().rev().find(|e| e.start <= a + 6.0).map(|e| e.value).unwrap_or(0);
            c.speed.push(if v > 0 { v as f32 / 3.6 } else { 99.0 });
            let j = l.events.iter().find(|e| e.kind == 100 && e.start <= a + 3.0 && e.end >= a);
            c.crouch.push(j.is_some());
            c.jump.push(j.map_or(-1, |e| e.value));
        }
        c
    }
    fn extend(&mut self, o: AiCourse) { self.speed.extend(o.speed); self.crouch.extend(o.crouch); self.jump.extend(o.jump); }
    /// `events` are (start, end, type, value) in Bevy space, from `Level::ai_events`.
    pub fn build(line: &[Vec3], events: &[(Vec3, Vec3, i32, i32)]) -> Self {
        let n = line.len();
        let nearest = |p: Vec3| (0..n).min_by(|&a, &b| (line[a] - p).length_squared().total_cmp(&(line[b] - p).length_squared())).filter(|&i| (line[i] - p).length() < 25.0);
        let mut set: Vec<Option<f32>> = vec![None; n];
        let mut crouch = vec![false; n];
        let mut jump = vec![-1; n];
        for (a, b, kind, value) in events {
            let (Some(i), Some(j)) = (nearest(*a), nearest(*b)) else { continue };
            match kind {
                101 if *value > 0 => set[i] = Some(*value as f32 / 3.6),
                100 => {
                    for k in i.min(j)..=i.max(j) { crouch[k] = true; jump[k] = *value; }
                    if i == j && i > 0 { crouch[i - 1] = true; jump[i - 1] = *value; }
                }
                _ => {}
            }
        }
        // a target speed holds until the next one
        let mut cur = 99.0;
        let speed = set.iter().map(|s| { if let Some(v) = s { cur = *v; } cur }).collect();
        Self { speed, crouch, jump }
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
    /// the trick planned for the current jump (AIComputer_PlanJumpTrick): spin and flip direction
    /// (-1, 0, +1), the grab (shoulder mask, 0 none), letting go late, and whether it is planned
    plan: (f32, f32, u8, bool),
    planned: bool,
    rng: u32,
    jumps: u32,
    /// the original's catch-up: this rider's physics runs this much faster or slower (0.7..1.5)
    pub time_scale: f32,
    was_crouching: bool,
    /// the way this rider is going through the AI path network: the points so far, their speeds
    /// and jumps, the last path added, and whether the route has run out of paths
    pub route: Vec<Vec3>,
    route_course: AiCourse,
    route_last: Option<usize>,
    route_done: bool,
}
impl AiDriver {
    pub fn new(offset: f32, skill: f32) -> Self { Self { idx: 0, last_idx: 0, stuck_for: 0.0, offset, skill, finished: None, stuck: 0, clock: 0.0, beside: 0.0, seen_respawns: 0, over: Vec::new(), slow_for: 0.0, shove_wait: 4.0, plan: (0.0, 0.0, 0, false), planned: false, rng: 0x9E37_79B9 ^ ((offset * 1000.0) as i32 as u32).wrapping_mul(2654435761) ^ ((skill * 977.0) as u32), jumps: 0, time_scale: 1.0, was_crouching: false, route: Vec::new(), route_course: AiCourse::default(), route_last: None, route_done: false } }
    /// rand() % 100 - 16, as the original rolls it against the tricks stat (0..255)
    fn roll(&mut self) -> i32 {
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        (self.rng % 100) as i32 - 16
    }
    fn rand(&mut self) -> u32 { self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5; self.rng }
    /// AIComputer_PlanJumpTrick: whether to spin/flip (only where the jump allows), which way, a
    /// random grab of the rider's fifteen, and whether to hold it late.
    fn plan_trick(&mut self, r: &Rider, value: i32) {
        let b = (r.stats.tricks * 255.0) as i32;
        let (spin, flip) = (value & 2 != 0, value & 1 != 0);
        let trick = (spin || flip) && self.roll() < b;
        let k = if self.skill >= 1.0 { 0.0 } else { 1.0 - self.skill / 14.454015 };
        let late = (self.rand() % 100) as f32 * k > 20.0;
        let grab = if self.roll() < b { crate::trickdata::COMBOS[(self.rand() % 15) as usize] } else { 0 };
        let mut sd = if trick && spin { 1.0 } else { 0.0 };
        let mut fd = if trick && flip { 1.0 } else { 0.0 };
        if self.rand() & 1 != 0 { sd = -sd; }
        if self.rand() & 1 != 0 { fd = -fd; }
        self.plan = (sd, fd, grab, late);
        self.planned = true;
    }
    /// Our jumps do not always carry as far as the original's: drop a flip or spin that the
    /// predicted flight (jump straight off from here) has no time for, so the rider is not left
    /// upside down. The prediction steps the flight against the snow, as AirPredict_AtTakeoff does.
    fn trim_plan(&mut self, r: &Rider, world: &CollisionWorld) {
        let mut p = r.pos + Vec3::Y * 0.1;
        let mut v = r.vel + r.normal * JUMP_MIN;
        let (mut t, step) = (0.0f32, 1.0 / 30.0);
        while t < 4.0 {
            v.y -= if v.y > 0.0 { GRAVITY_RISING } else { GRAVITY_FALLING } * step;
            let q = p + v * step;
            t += step;
            if t > 0.1 && v.y < 0.0 && world.ground(q, 1.5, 0.0).is_some() { break; }
            p = q;
        }
        let k = 11.517 * r.stats.spin_scale();
        let need_flip = std::f32::consts::TAU / (k * 2.0 / 3.0) + 0.25;
        let need_spin = std::f32::consts::PI / k + 0.2;
        if t < need_flip { self.plan.1 = 0.0; }
        if t < need_spin { self.plan.0 = 0.0; }
    }
    pub fn progress(&self, line: &[Vec3]) -> f32 { if line.len() < 2 { 0.0 } else { self.idx as f32 / (line.len() - 1) as f32 } }
    /// Decide the controls for this step. Puts the rider back on the line if it wedges itself.
    /// `clear` is how far the rider is above the snow.
    /// Look the course over once: where does the line leave the ground?
    /// What this rider wants at a fork (the original's U3 preference): freestyle riders low on
    /// boost go for the trick lines (100), BX riders likewise; riders in the middle of the pack
    /// take the safe lines (0); alpine riders in the lead too.
    fn pref(r: &Rider, place: usize) -> f32 {
        match r.stats.kind {
            1 => if r.boost < 0.31187 { 100.0 } else if place == 3 || place == 4 { 0.0 } else { 50.0 },
            0 => if r.boost < 0.10338 { 100.0 } else if place == 2 || place == 3 { 0.0 } else { 50.0 },
            _ => if place == 0 { 0.0 } else { 50.0 },
        }
    }
    /// Drive along the AI path network (the original's opponents), choosing at each fork; with no
    /// paths, along the race line.
    pub fn drive_paths(&mut self, r: &mut Rider, world: &CollisionWorld, paths: &crate::course::AiPaths, line: &[Vec3], course: &AiCourse, place: usize, dt: f32) -> Input {
        if paths.is_empty() { self.learn(world, line); return self.drive(r, world, line, course, dt); }
        if self.route.is_empty() {
            match paths.select(r.pos, heading(r.yaw), Some(Self::pref(r, place)), None) {
                Some((pi, s)) => {
                    let l = &paths.paths[pi];
                    let from = l.at.iter().position(|a| *a >= s).unwrap_or(0);
                    self.route = l.pts[from..].to_vec();
                    self.route_course = AiCourse::from_path(l, from);
                    self.route_last = Some(pi);
                }
                None => { self.learn(world, line); return self.drive(r, world, line, course, dt); }
            }
        }
        // keep a stretch of route ahead: at the end of a path, pick the next one from where it ends
        while !self.route_done && self.idx + 12 >= self.route.len() {
            let n = self.route.len();
            let end = self.route[n - 1];
            let dir = end - self.route[n.saturating_sub(2)];
            match paths.select(end, Vec3::new(dir.x, 0.0, dir.z), Some(Self::pref(r, place)), self.route_last) {
                Some((pi, s)) if paths.paths[pi].point_at(s).distance(end) < 25.0 => {
                    let l = &paths.paths[pi];
                    let from = l.at.iter().position(|a| *a > s).unwrap_or(l.pts.len());
                    if from >= l.pts.len() { self.route_done = true; break; }
                    self.route.extend_from_slice(&l.pts[from..]);
                    self.route_course.extend(AiCourse::from_path(l, from));
                    self.route_last = Some(pi);
                }
                _ => {
                    // the network runs out (near the finish): carry on down the race line
                    let k = (0..line.len()).min_by(|a, b| (line[*a] - end).length_squared().total_cmp(&(line[*b] - end).length_squared())).unwrap_or(0);
                    let k = (k + 1).min(line.len());
                    self.route.extend_from_slice(&line[k..]);
                    let c = AiCourse { speed: course.speed.get(k..).unwrap_or(&[]).to_vec(), crouch: course.crouch.get(k..).unwrap_or(&[]).to_vec(), jump: course.jump.get(k..).unwrap_or(&[]).to_vec() };
                    self.route_course.extend(c);
                    self.route_done = true;
                }
            }
        }
        let route = std::mem::take(&mut self.route);
        let rc = std::mem::take(&mut self.route_course);
        self.learn(world, &route);
        let input = self.drive(r, world, &route, &rc, dt);
        self.route = route;
        self.route_course = rc;
        input
    }
    /// Never rejoin the line in, or just short of, something that puts the rider straight back (a
    /// shut iris door): on to the first line point with the next 12 m clear of it (the original
    /// puts a rider back on a respawnable path, which runs round the door).
    fn clear_of_resets(&mut self, world: &CollisionWorld, line: &[Vec3]) {
        let blocked = |i: usize| {
            let mut run = 0.0;
            let mut k = i;
            loop {
                if world.in_reset(line[k] + Vec3::Y * 1.8, BODY_RADIUS) { return true; }
                if k + 1 >= line.len() { return false; }
                let (a, b) = (line[k], line[k + 1]);
                // between line points too: they can be metres apart
                let n = ((b - a).length() / 1.0).ceil().max(1.0) as usize;
                for j in 1..n { if world.in_reset(a.lerp(b, j as f32 / n as f32) + Vec3::Y * 1.8, BODY_RADIUS) { return true; } }
                run += (b - a).length();
                k += 1;
                if run > 12.0 { return false; }
            }
        };
        let from = self.idx;
        while self.idx + 1 < line.len() && blocked(self.idx) && self.idx < from + 40 { self.idx += 1; }
        if blocked(self.idx) { self.idx = from; }
    }
    /// Moved far in one go (the Megaplex tube, a reset): find a new way through the paths.
    pub fn reroute(&mut self) { self.route.clear(); self.route_course = AiCourse::default(); self.route_last = None; self.route_done = false; self.idx = 0; self.last_idx = 0; self.over.clear(); }
    pub fn learn(&mut self, world: &CollisionWorld, line: &[Vec3]) {
        if self.over.len() == line.len() { return; }
        self.over = line.iter().map(|p| world.ground(*p + Vec3::Y * 2.0, 2.0, 60.0).map_or(60.0, |h| p.y - h.y)).collect();
    }
    pub fn drive(&mut self, r: &mut Rider, world: &CollisionWorld, line: &[Vec3], course: &AiCourse, dt: f32) -> Input {
        let clear = if r.grounded { 0.0 } else { world.ground(r.pos, 0.0, 60.0).map_or(60.0, |h| r.pos.y - h.y) };
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
            self.clear_of_resets(world, line);
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
                self.clear_of_resets(world, line);
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
            if r.rail.is_some() {
                // AIComputer_RailInput: balance; once steady, the trick roll starts quarter turns
                self.planned = false;
                let b = (-2.0 * r.rail_off).clamp(-1.0, 1.0);
                if b.abs() > 0.1 { return Input { steer: b, ..Input::default() }; }
                if self.roll() < (r.stats.tricks * 255.0) as i32 {
                    if self.plan.0 == 0.0 { self.plan.0 = if b >= 0.0 { 1.0 } else { -1.0 }; }
                    return Input { steer: -self.plan.0, boost: true, ..Input::default() };
                }
                return Input::default();
            }
            if r.air_time < 0.05 { self.jumps += 1; }
            if std::env::var("TRICKY_AIDBG").is_ok() && r.air_time > 0.0 && r.air_time <= 1.0 / 120.0 + 1e-4 { println!("  takeoff: plan {:?} planned {} clear {:.1} vy {:.1} wind {:.2}/{:.2} spinw {:.1} flipw {:.1}", self.plan, self.planned, clear, r.vel.y, r.spin_ax.w, r.flip_ax.w, r.spin_ax.hi, r.flip_ax.hi); }
            // time left in the air (AirPredict): rise and fall onto snow at the height below now
            let vy = r.vel.y;
            let left = if vy > 0.0 {
                let peak = clear + vy * vy / (2.0 * GRAVITY_RISING);
                vy / GRAVITY_RISING + (2.0 * peak.max(0.0) / GRAVITY_FALLING).sqrt()
            } else {
                (vy + (vy * vy + 2.0 * GRAVITY_FALLING * clear.max(0.0)).sqrt()) / GRAVITY_FALLING
            };
            let thr = 0.8 / (r.stats.tricks * 0.8057834 + 0.78212035);
            // a plain jump (no zone): a grab once it has been up a while with time to spare
            if !self.planned && r.air_time > thr && left > thr {
                let g = crate::trickdata::COMBOS[(self.rand() % 15) as usize];
                self.plan = (0.0, 0.0, g, false);
                self.planned = true;
            }
            let (mut spin_dir, mut flip_dir, grab, late) = self.plan;
            if !self.planned { spin_dir = 0.0; flip_dir = 0.0; }
            // AIComputer_AirTrickInput: keep turning; near a whole turn stop if another will not fit
            let flip_deg = r.flip.abs().to_degrees() % 360.0;
            if flip_dir != 0.0 && !(5.0..=340.0).contains(&flip_deg) && r.flip.abs() > 0.5 {
                let w = r.flip_ax.w.abs().max(0.5);
                if left < std::f32::consts::TAU / w { flip_dir = 0.0; self.plan.1 = 0.0; }
            }
            let p = if r.stats.alpine { 360.0 } else { 180.0 };
            let spin_deg = r.spin.abs().to_degrees() % p;
            if spin_dir != 0.0 && (spin_deg > p - 20.0 || spin_deg < 5.0) && r.spin.abs() > 0.5 {
                let w = r.spin_ax.w.abs().max(0.5);
                if left < p.to_radians() / w { spin_dir = 0.0; self.plan.0 = 0.0; }
            }
            // the grab: held until the landing is near (later for a late let-go)
            let hold = grab != 0 && left > if late { 0.3 } else { thr };
            if !hold && grab != 0 && left <= thr { self.plan.2 = 0; }
            if !self.planned || (spin_dir == 0.0 && flip_dir == 0.0 && !hold) {
                // nothing to do: steer for the line
                return Input { steer: if left < 0.5 { 0.0 } else { 0.0 }, ..Input::default() };
            }
            return Input { steer: spin_dir, flip: flip_dir, grab: if hold { grab } else { 0 }, ..Input::default() };
        }
        // steering (0x1372f8): proportional to the angle off, scaled by skill, a dead zone, a limit
        let out = diff.abs() * 6.2897 * self.skill / 1.0241;
        let steer = if out < 0.2 { 0.0 } else { -diff.signum() * out.min(0.9706) };
        // cruising (0x1375c8): the course's AI paths give a target speed; the line's bends cap it
        let target = course.speed.get(self.idx + 1).copied().unwrap_or(99.0).min(safe_speed);
        let gap = (self.idx + 1..(self.idx + 7).min(self.over.len())).any(|i| self.over[i] > 5.0) && self.over.get(self.idx).is_some_and(|o| *o < 5.0);
        let brake = !gap && speed > target + 1.389;
        let slow = gap || speed < (target - 1.389) * self.skill / 1.4103;
        let tuck = slow || speed < 8.33;
        let boost = (slow || r.boost >= 0.99) && steer.abs() < 0.08 && r.boost > 0.0;
        // jumps the AI paths mark: crouch through the zone, let go where it ends
        let crouch = course.crouch.get(self.idx).copied().unwrap_or(false) || course.crouch.get(self.idx + 1).copied().unwrap_or(false);
        let value = course.jump.get(self.idx).copied().filter(|v| *v >= 0).or(course.jump.get(self.idx + 1).copied().filter(|v| *v >= 0)).unwrap_or(0);
        // committing to the jump (AIComputer_GroundJumpApproach): close to the line and steady
        if crouch && !self.was_crouching { self.planned = false; }
        if crouch && !self.planned && steer.abs() < 0.536 { self.plan_trick(r, value); self.trim_plan(r, world); }
        if !crouch && r.grounded && self.planned && r.air_time <= 0.0 && !self.was_crouching { self.planned = false; }
        self.was_crouching = crouch;
        // winding up in the zone: the stick goes the way the trick will turn
        let (wind_s, wind_f) = if crouch && self.planned { (self.plan.0, self.plan.1) } else { (0.0, 0.0) };
        Input { steer, wind: Some(wind_s), flip: wind_f, tuck, brake, boost, jump: crouch && !brake, ..Input::default() }
    }
}

/// Timing a run from the gate to the end of the course's main race line.
#[derive(Clone, Copy, PartialEq)]
pub enum RaceState { Countdown, Running, Finished }
pub struct Race { pub line: Vec<Vec3>, pub idx: usize, pub time: f32, pub state: RaceState, pub best: Option<f32>, pub countdown: f32, pub course: AiCourse,
    /// the course's race lines, the player's progress along them, and seconds the show-off clock has gained
    pub lines: crate::course::Lines, pub prog: crate::course::Progress, pub bonus: f32, pub checkpoints: u32,
    /// the opponents' AI path network
    pub paths: crate::course::AiPaths,
    /// Tokyo Megaplex's lap tube, if the course has one
    pub tube: Option<crate::props::Tube>,
    /// the course's camera scripts, and whether the intro is playing (the countdown waits for it)
    pub cml: Option<std::sync::Arc<crate::intro::Cml>>, pub intro: bool,
    /// the riders' pre-race scene, and the course's start stage area (game units: where, how turned)
    pub scene: Option<crate::intro::SceneState>, pub stage: Option<(Vec3, bevy::prelude::Quat)>,
    /// the finish area (scene frame 2), and the post-race scenes: 0 not yet, 1 asked for, 2 playing,
    /// 3 over (results); the scripts and who plays what
    pub finish_area: Option<(Vec3, bevy::prelude::Quat)>, pub post: u8, pub post_scripts: Vec<String>, pub cast: Option<crate::intro::Cast> }
impl Race {
    /// The original's countdown: GO after 2.5 s (beeps at 0.5, 1.0, 1.5 and 2.0 s).
    pub fn new(line: Vec<Vec3>) -> Self { Self { line, idx: 0, time: 0.0, state: RaceState::Countdown, best: None, countdown: 2.5, course: AiCourse::default(), lines: Default::default(), prog: Default::default(), bonus: 0.0, checkpoints: 0, paths: Default::default(), tube: None, cml: None, intro: false, scene: None, stage: None, finish_area: None, post: 0, post_scripts: Vec::new(), cast: None } }
    pub fn restart(&mut self) { self.idx = 0; self.time = 0.0; self.state = RaceState::Countdown; self.countdown = 2.5; self.prog = Default::default(); self.bonus = 0.0; self.checkpoints = 0; self.post = 0; self.post_scripts.clear(); self.cast = None; }
    /// Metres of racing line still to go from a line point.
    pub fn remaining(&self, idx: usize) -> f32 { self.line.windows(2).skip(idx).map(|w| (w[1] - w[0]).length()).sum() }
    pub fn progress(&self) -> f32 { if self.line.len() < 2 { 0.0 } else { self.idx as f32 / (self.line.len() - 1) as f32 } }
    pub fn update(&mut self, r: &Rider, dt: f32) {
        if self.line.len() < 4 { return; }
        match self.state {
            RaceState::Countdown => { if !self.intro { self.countdown -= dt; } if self.countdown <= 0.0 { self.state = RaceState::Running; } }
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
                // the finish: the race lines' finish event when the course has one, else the line's end
                // (or, as a fallback, the end of the racing line the riders follow)
                let mut crossed = !self.lines.has_laps() && (self.idx + 2 >= self.line.len() || (self.progress() > 0.8 && (end - r.pos).length() < 35.0));
                for e in self.lines.update(&mut self.prog, r.pos, r.vel) {
                    if e.kind == crate::course::EV_CHECKPOINT { self.bonus += e.value as f32; self.checkpoints += 1; }
                    if e.kind == crate::course::EV_FINISH { crossed = true; }
                }
                if crossed {
                    self.state = RaceState::Finished;
                    self.best = Some(self.best.map_or(self.time, |b| b.min(self.time)));
                }
            }
            RaceState::Finished => {}
        }
    }
}

/// Third-person camera that trails the rider. Plain data so the self-test can measure it.
/// One of the original's named chase cameras (COMMONOB.CML), in metres and radians.
pub struct CamDef { pub name: &'static str, dist: f32, pitch: f32, div_down: f32, div_up: f32, lag: f32, look_up: f32, base_up: f32, pitch_off: f32, pub fov: f32, look_lag: f32, roll_div: f32, roll: bool }
/// The four the player cycles through, in the original's order: board, near, far, over.
pub const CAMS: [CamDef; 4] = [
    CamDef { name: "chase board", dist: 1.5, pitch: -0.5, div_down: 1.5, div_up: 1.0, lag: 6.0, look_up: 0.0, base_up: 0.0, pitch_off: 0.27, fov: 1.20, look_lag: 4.0, roll_div: 2.0, roll: true },
    CamDef { name: "chase near", dist: 1.8, pitch: -0.5, div_down: 2.0, div_up: 1.0, lag: 16.0, look_up: 1.0, base_up: 0.35, pitch_off: -0.16, fov: 1.65, look_lag: 5.0, roll_div: 4.0, roll: true },
    CamDef { name: "chase far", dist: 5.0, pitch: -0.7, div_down: 2.0, div_up: 1.5, lag: 15.0, look_up: 1.0, base_up: 0.0, pitch_off: 0.03, fov: 1.65, look_lag: 5.0, roll_div: 8.0, roll: true },
    CamDef { name: "chase over", dist: 10.0, pitch: -0.85, div_down: 2.0, div_up: 2.0, lag: 15.0, look_up: 1.0, base_up: 0.0, pitch_off: 0.15, fov: 1.65, look_lag: 25.0, roll_div: 4.0, roll: false },
];
fn wrap(a: f32) -> f32 { (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI }
/// A per-tick (60 Hz) fraction `1/div` as a fraction for a step of `dt` seconds.
fn tick_k(div: f32, dt: f32) -> f32 { 1.0 - (1.0 - 1.0 / div.max(1.0)).powf(dt * 60.0) }

/// The race camera (`cBxCamera_EvalNamedCam`): orbits behind the rider's filtered direction of travel.
pub struct ChaseCam { pub pos: Vec3, pub mode: usize, dir: Vec3, pitch: f32, yaw: f32, raw: Vec3, smooth: Vec3, head: f32, tilt: f32, pub roll: f32, pub fov: f32, blend: f32, cut: bool, dip: f32, dip_t: f32, was_air: bool, last_v: Vec3 }
impl Default for ChaseCam { fn default() -> Self { Self { pos: Vec3::ZERO, mode: 2, dir: Vec3::ZERO, pitch: 0.0, yaw: 0.0, raw: Vec3::ZERO, smooth: Vec3::ZERO, head: 0.0, tilt: 0.0, roll: 0.0, fov: 1.65, blend: 0.0, cut: true, dip: 0.0, dip_t: 1.0, was_air: false, last_v: Vec3::ZERO } } }
impl ChaseCam {
    pub fn cycle(&mut self, step: i32) { self.mode = (self.mode as i32 + step).rem_euclid(CAMS.len() as i32) as usize; self.cut = true; }
    pub fn cut(&mut self) { self.cut = true; }
    pub fn name(&self) -> &'static str { CAMS[self.mode].name }
    /// Returns (camera position, point to look at); `roll` and `fov` are left on the camera.
    pub fn update(&mut self, world: &CollisionWorld, r: &Rider, dt: f32) -> (Vec3, Vec3) {
        let c = &CAMS[self.mode];
        let down = r.crashed > 0.0;
        // the direction followed: velocity, filtered 5% a tick (1% while tumbling); the board when stopped
        let want = if r.vel.length() > 0.1 { r.vel.normalize() } else { heading(r.yaw) };
        let flat_len = |v: Vec3| Vec3::new(v.x, 0.0, v.z).length();
        if self.cut || self.dir == Vec3::ZERO || flat_len(self.dir) < 0.5 { self.dir = want; }
        else {
            let k = tick_k(if down { 100.0 } else { 20.0 }, dt);
            self.dir = (self.dir * (1.0 - k) + want * k).normalize_or(want);
        }
        let d = self.dir;
        let e = d.y.atan2(flat_len(d));
        let p_t = c.pitch + e / if d.y < 0.0 { c.div_down } else { c.div_up };
        let y_t = d.x.atan2(d.z);
        if self.cut { self.pitch = p_t; self.yaw = y_t; }
        else {
            let k = tick_k(c.lag, dt);
            self.pitch += (p_t - self.pitch) * k;
            self.yaw += wrap(y_t - self.yaw) * k;
        }
        let pivot = r.pos + Vec3::Y * (0.9 + c.base_up);
        let dist = c.dist;
        let back = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let mut cam = pivot - back * dist * self.pitch.cos() - Vec3::Y * dist * self.pitch.sin();
        // collision (Cam_CollidePivotToCamera): stop at whatever is in the way, pushed off it
        let off = if dist > 5.0 { 0.6 } else { 0.4 };
        if let Some(t) = world.raycast(pivot, cam) {
            let dir = (cam - pivot).normalize_or_zero();
            cam = pivot + dir * (t - off).max(0.3);
        }
        if let Some(h) = world.ground(cam + Vec3::Y * 0.5, 0.5, 3.0) { cam.y = cam.y.max(h.y + off); }
        // while the rider is down the camera all but stops where it is
        let kb = (dt * 60.0).min(4.0);
        self.blend = if down { (self.blend + 0.0291667 * kb).min(0.98) } else { (self.blend - 0.003 * kb).max(0.0) };
        if self.cut || self.blend <= 0.0 { self.raw = cam; } else { self.raw += (cam - self.raw) * (1.0 - self.blend.powf(dt * 60.0)); }
        // landing dip: a hard landing pulls the view down for five ticks
        if !r.grounded && r.rail.is_none() { self.was_air = true; self.last_v = r.vel; }
        else if self.was_air {
            self.was_air = false;
            let s = -self.last_v.y;
            if s > 10.0 { self.dip = 2.0 * ((s - 10.0) / 30.0).min(1.0); self.dip_t = 0.0; }
        }
        self.dip_t += dt;
        let mut look = pivot + Vec3::Y * c.look_up;
        if self.dip_t < 5.0 / 60.0 { look.y -= self.dip; }
        // the view: angles from the camera to the target, eased
        let to = look - self.raw;
        let head_t = to.x.atan2(to.z);
        let tilt_t = to.y.atan2(flat_len(to)) + c.pitch_off;
        let mut roll_t = 0.0;
        if c.roll && r.grounded && r.rail.is_none() && !down {
            let f = to.normalize_or(Vec3::Z);
            let u = (r.normal - f * r.normal.dot(f)).normalize_or(Vec3::Y);
            let z = (Vec3::Y - f * f.y).normalize_or(Vec3::Y);
            let a = z.dot(u).clamp(-1.0, 1.0).acos();
            roll_t = a.copysign(f.cross(z).dot(u)) / c.roll_div;
        }
        let look_lag = if self.blend > 0.0 && c.look_lag < 50.0 { 10.0 + 20.0 * self.blend } else { c.look_lag };
        if self.cut {
            self.smooth = self.raw; self.head = head_t; self.tilt = tilt_t; self.roll = roll_t; self.fov = c.fov;
        } else {
            let k = 1.0 - 0.8f32.powf(dt * 60.0);
            self.smooth += (self.raw - self.smooth) * k;
            let kl = tick_k(look_lag, dt);
            self.head += wrap(head_t - self.head) * kl;
            self.tilt += wrap(tilt_t - self.tilt) * kl;
            self.roll += wrap(roll_t - self.roll) * tick_k(25.0, dt);
            self.fov += (c.fov - self.fov) * kl;
        }
        self.cut = false;
        // a snap if the rider was moved far (a reset)
        if self.smooth.distance(pivot) > dist * 4.0 + 20.0 { self.cut = true; }
        self.pos = self.smooth;
        let view = Vec3::new(self.head.sin() * self.tilt.cos(), self.tilt.sin(), self.head.cos() * self.tilt.cos());
        (self.pos, self.pos + view * 5.0)
    }
}




impl Rider {
    /// The gate clip's markers (normalised): leaning forward goes to m0, the push-out fires at m1.
    pub fn gate_markers(&self) -> (f32, f32) { if self.stats.kind == 2 { (0.4, 0.8) } else { (16.0 / 30.0, 20.0 / 30.0) } }
    pub fn gate_reset(&mut self) { self.gate_state = 8; self.gate_p = 0.0; self.gate_hi = 0.0; self.gate_lo = 0.0; self.gate_still = 0.0; self.gate_target = 0.0; self.gate_speed = 0.0; }
    /// The computer's rocking in the gate: turn round whenever it gets where it was going.
    pub fn gate_rock(&mut self) -> i32 {
        if self.gate_p == self.gate_target || self.gate_rock_dir == 0 { self.gate_rock_dir = if self.gate_p > 0.0 { -1 } else { 1 }; }
        self.gate_rock_dir
    }
    fn gate_k(&self) -> f32 { (0.4875571 + 0.71808594 * self.stats.gate) * 4.437673 }
    fn gate_move(&mut self, rate: f32, dt: f32) {
        let step = rate * dt;
        let (t, p) = (self.gate_target, self.gate_p);
        let np = if t + step < p { p - step } else if p < t - step { p + step } else { t };
        if np == p { self.gate_still += dt; } else {
            self.gate_still = 0.0;
            if np > p { self.gate_hi = np } else { self.gate_lo = np }
        }
        self.gate_p = np;
    }
    /// In the gate during the countdown: up leans forward (to the first marker), nothing half way,
    /// back sits back. Rocking back just before GO is the slingshot start.
    pub fn gate_anticipate(&mut self, y: i32, dt: f32) {
        if self.gate_state != 8 { return; }
        let m0 = self.gate_markers().0;
        self.gate_target = if y > 0 { m0 } else if y == 0 { 0.5 * m0 } else { 0.0 };
        let rate = self.gate_k() * (self.gate_target - self.gate_p).abs().max(0.2146618);
        self.gate_move(rate, dt);
    }
    /// GO (GateAnticipate_Exit_LaunchSpeed): 20 km/h, or more for a well-timed rock back still moving.
    pub fn gate_go(&mut self) {
        if self.gate_state != 8 { return; }
        let mut s = 5.5555554;
        if self.gate_target < 0.5 && self.gate_still < 1.0 / 150.0 {
            let x = (self.gate_hi - self.gate_lo) * 20.248037 * (1.0 - self.gate_p) / (self.gate_still * 60.0 + 1.0);
            if x >= s { s = x; }
        }
        self.gate_speed = s * (0.4875571 + 0.71808594 * self.stats.gate);
        self.gate_state = 9;
        self.gate_target = 1.0;
    }
    /// Pushing out: the clip runs on; at its second marker the rider is moving; at its end he rides.
    /// Returns true while the rider is still held in the gate.
    pub fn gate_launch(&mut self, dt: f32) -> bool {
        if self.gate_state != 9 { return false; }
        let rate = self.gate_k() * 0.2146618;
        self.gate_move(rate, dt);
        let m1 = self.gate_markers().1;
        if self.gate_p >= m1 && self.vel.length() < 0.5 {
            self.vel = heading(self.yaw) * self.gate_speed;
            if self.gate_speed > 6.0 { self.last_trick = "SLINGSHOT START".into(); self.trick_timer = 1.5; }
        }
        if self.gate_p >= 1.0 { self.gate_state = 0; }
        self.gate_p < m1
    }
}

/// The grab an uber clip starts from: "bxUT_MTINDY" -> "INDY" (after the two-letter style).
fn uber_grab(clip: &str) -> &str { clip.split_once("UT_").map_or("", |(_, r)| r.get(2..).unwrap_or("")) }

/// Grab clip markers from the animation file (frames): the grab counts from marker 1, holds at
/// marker 2, and is safe to land only past marker 4 (`Landing_CheckAngles`); and the clip's length.
pub fn grab_markers(clip: &str) -> (f32, f32, f32, f32) {
    let (m1, m2, m4, n) = match clip {
        "bxT_CANADIAN" => (10, 17, 26, 36), "bxT_CHICKENSALAD" => (10, 14, 24, 30), "bxT_CRAIL" => (10, 17, 23, 33),
        "bxT_EXPERIMENT" => (10, 17, 30, 36), "bxT_FLYINGSQUIRREL" => (10, 14, 31, 35), "bxT_IGUANA" => (10, 14, 25, 30),
        "bxT_INDY" => (10, 17, 26, 35), "bxT_JAPAN" => (10, 17, 26, 33), "bxT_LEIN" => (10, 15, 21, 30),
        "bxT_MELANCHOLY" => (10, 14, 22, 33), "bxT_METHOD" => (10, 14, 27, 36), "bxT_MUTE" => (10, 17, 23, 32),
        "bxT_NOSEGRAB" => (10, 17, 24, 35), "bxT_NUCLEAR" => (10, 14, 23, 30), "bxT_ROASTBEEF" => (10, 16, 23, 30),
        "bxT_ROCKET" => (10, 17, 24, 32), "bxT_SEATBELTAIR" => (10, 16, 25, 35), "bxT_SLOBAIR" => (10, 14, 23, 30),
        "bxT_SPAGHETTI" => (10, 14, 23, 30), "bxT_STALEFISH" => (10, 20, 30, 36), "bxT_STALEMASKY" => (10, 17, 26, 35),
        "bxT_STIFFY" => (10, 14, 25, 32), "bxT_SWISSCHEESE" => (10, 17, 24, 35), "bxT_TAILGRAB" => (10, 14, 23, 30),
        _ => (10, 17, 25, 33),
    };
    (m1 as f32, m2 as f32, m4 as f32, n as f32)
}

/// The original's names for big spin-and-flip combinations (`Score_BuildTrickId`, table 0x320cf0):
/// (backside?, half turns of spin, flips: + front / - back) -> name.
fn special_trick(bs: bool, h: u32, f: i32) -> Option<&'static str> {
    Some(match (bs, h, f) {
        (true, 4, 3) => "Ambulance Trip", (true, 4, -3) => "Shell Cracker",
        (false, 5, 2) => "Full On", (false, 5, 3) => "Nose Picker", (false, 5, -2) => "Tripped Out", (false, 5, -3) => "Chunks",
        (true, 5, 2) => "Mindless", (true, 5, 3) => "Ligament", (true, 5, -2) => "Crippled Squirrel", (true, 5, -3) => "Deathwish",
        (false, 6, 3) => "Horrifying", (false, 6, -3) => "Heimlich", (true, 6, 3) => "Cartilage", (true, 6, -3) => "Iron Lung",
        (false, 7, 2) => "Breakdancer", (false, 7, 3) => "Gross", (false, 7, -2) => "Plain Brown Wrapper", (false, 7, -3) => "Gargle",
        (true, 7, 2) => "Banzai", (true, 7, 3) => "Hospitalized", (true, 7, -2) => "Montezuma", (true, 7, -3) => "Multiple Fracture",
        (false, 8, 2) => "Scratch Artist", (false, 8, 3) => "Backwash", (false, 8, -2) => "Brown Bag", (false, 8, -3) => "Roadkill",
        (true, 8, 2) => "Shiny", (true, 8, 3) => "Life Insurance", (true, 8, -2) => "Homesick", (true, 8, -3) => "Roadkill",
        (false, 9, 2) => "Famous", (false, 9, -2) => "Lunch Box", (true, 9, 2) => "Torpedo", (true, 9, -2) => "Double Jointed",
        (false, 10, 2) => "DJ", (false, 10, -2) => "Grab Bag", (true, 10, 2) => "Twister", (true, 10, -2) => "Spider",
        _ => return None,
    })
}
/// The trick's name as the original writes it (`Score_FormatTrickName`): prefix, side, spin,
/// flips (Rodeo / Misty when every flip carries 540 of spin), special name, the spin after the
/// flips, grabs ("To Late" a second one, "Combo Grab" for three), and "Air" / "To Fakie" / "To Rail".
/// `spin` is the game's sign (positive = backside); returns "" for nothing done.
pub fn trick_name(spin: f32, flip: f32, switch: bool, from_rail: bool, onto_rail: bool, to_fakie: bool, grabs: &[String]) -> String {
    use std::f32::consts::PI;
    let s = (spin / PI).round() as i32 * if switch { -1 } else { 1 };
    let fl = (flip / (2.0 * PI)).round() as i32;
    let (h, f) = (s.unsigned_abs(), fl.unsigned_abs());
    if h == 0 && f == 0 && grabs.is_empty() { return String::new(); }
    if h > 10 || f > 3 { return "???".into(); }
    let c = f.min(h / 3);
    let mut out = String::new();
    out += if from_rail { if switch { "Rail To Switch " } else { "Rail To " } } else if switch { "Switch " } else { "" };
    let special = special_trick(s > 0, h, fl);
    let mut after = false;
    match special {
        Some(n) => { out += n; out += " "; }
        None => {
            if s != 0 { out += if s > 0 { "BS " } else { "FS " }; }
            if f == 0 && h > 0 { out += &format!("{} ", h * 180); }
            if f > 0 {
                out += ["", "", "Double ", "Triple "][f as usize];
                out += if c == f { if fl < 0 { "Rodeo " } else { "Misty " } } else if fl < 0 { "Back Flip " } else { "Front Flip " };
            }
        }
    }
    if f > 0 && h > 0 && h != 3 * f && h <= 10 { out += &format!("{} ", h * 180); after = true; }
    let mut list: Vec<&String> = Vec::new();
    for g in grabs { if list.last() != Some(&g) { list.push(g); } }
    if let Some(g) = list.first() { out += g; out += " "; }
    if list.len() >= 2 { out += "To Late "; out += if list.len() >= 3 { "Combo Grab " } else { list[1] }; out += " "; }
    if onto_rail { out += "To Rail"; }
    else if special.is_none() && (c == 0 || after) {
        if to_fakie && !switch { out += "To Fakie"; }
        else if h == 0 && f == 0 && !list.is_empty() { out += "Air"; }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Headless check: ride the course on autopilot along the race line and report what happens.
pub fn self_test(world: &mut CollisionWorld, rails: &Rails, line: &[Vec3], course: &AiCourse, lines: &crate::course::Lines, paths: &crate::course::AiPaths, tube: Option<&crate::props::Tube>, start_yaw: f32, seconds: f32) {
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
            // as if it had left the snow with a full wind-up for the trick being tested
            let k = 11.517 * r.stats.spin_scale();
            if flip_s > 0.0 { r.flip_ax = Axis::start(-k * 2.0 / 3.0); }
            if spin_s > 0.0 { r.spin_ax = Axis::start(k); }
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
        // a Late trick: spin, let it settle, then press again in the same jump
        {
            let mut r = Rider::new(start + Vec3::Y * 60.0, start_yaw);
            let k = 11.517 * r.stats.spin_scale();
            r.spin_ax = Axis::start(k);
            let (mut t, dt, mut seen, mut second) = (0.0f32, 1.0 / 120.0, Vec::<String>::new(), None::<f32>);
            while t < 9.0 && !(r.grounded && t > 0.5) {
                let air = r.air_time;
                let settled = r.air_mode2 && r.spin_ax.w.abs() < 0.7 && air > 1.2;
                if second.is_none() && settled { second = Some(air); }
                let steer = if air < 0.35 || second.is_some_and(|s| air < s + 0.35) { 1.0 } else { 0.0 };
                r.step(world, rails, Input { steer, ..Input::default() }, dt);
                if !r.last_trick.is_empty() && seen.last() != Some(&r.last_trick) { seen.push(r.last_trick.clone()); }
                t += dt;
            }
            println!("late trick: {:?}", seen);
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
    if std::env::var("TRICKY_GATESIM").is_ok() {
        // the gate: hold forward, then rock back n ticks before GO; how fast and how soon out?
        for back in [0, 10, 20, 26, 30, 40, 60] {
            let mut r = Rider::new(Vec3::ZERO, 0.0);
            let dt = 1.0 / 60.0;
            for tick in 0..150 { r.gate_anticipate(if tick >= 150 - back { -1 } else { 1 }, dt); }
            r.gate_go();
            let mut t = 0;
            while r.gate_launch(dt) && t < 600 { t += 1; }
            println!("rock back {back:2} ticks before GO: launch {:.2} m/s after {} ticks", r.vel.length(), t);
        }
        return;
    }
    if std::env::var("TRICKY_JUMPTEST").is_ok() {
        // ride off the start, hold Space for a while, let go, and see how high it goes
        let Some(&start) = line.first() else { return };
        for hold in [0.1f32, 0.4, 0.8] {
            let mut r = Rider::new(start + Vec3::Y * 0.3, start_yaw);
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            let (mut top, mut base, mut left_at, mut air) = (0.0f32, 0.0, None, 0.0f32);
            while t < 6.0 {
                let jump = t > 2.0 && t < 2.0 + hold;
                if t > 1.99 && t < 2.0 { base = r.pos.y; }
                r.step(world, rails, Input { jump, ..Input::default() }, dt);
                if t > 2.0 && !r.grounded { if let Some(g) = world.ground(r.pos, 0.0, 50.0) { top = top.max(r.pos.y - g.y); } }
                if t > 2.0 && !r.grounded { air = air.max(r.air_time); if left_at.is_none() { left_at = Some(t - 2.0 - hold); } }
                t += dt;
            }
            println!("hold {hold:.1} s: left the snow {:?} s after letting go, peak {:.2} m above the snow, air {:.2} s", left_at, top, air);
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
    let (mut crashes, mut was_down, mut stumbles, mut was_stumble) = (0u32, false, 0u32, false);
    let mut last_seen = String::new();
    let mut prog = crate::course::Progress::default();
    let mut tube_t = 101.0f32;
    let lane: Vec<f32> = std::env::var("TRICKY_LANE").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let mut ai = AiDriver::new(lane.first().copied().unwrap_or(0.0), lane.get(1).copied().unwrap_or(1.0));
    let crash_at: Option<f32> = std::env::var("TRICKY_CRASHAT").ok().and_then(|v| v.parse().ok());
    while t < seconds {
        r.course_dir = { let l: &[Vec3] = if ai.route.len() > 2 { &ai.route } else { line }; let i = ai.idx.min(l.len().saturating_sub(2)); let j = (i + 4).min(l.len() - 1); Some((l[j] - l[i]).normalize_or_zero()) };
        r.course_pts = { let l: &[Vec3] = if ai.route.len() > 2 { &ai.route } else { line }; course_points(l, ai.idx, r.pos) };
        if crash_at.is_some_and(|c| t <= c && t + dt > c) && r.crashed <= 0.0 { r.force_wipe(); }
        // TRICKY_LINE: follow the race line instead of the AI path network
        let input = if std::env::var("TRICKY_LINE").is_ok() { ai.learn(world, line); ai.drive(&mut r, world, line, course, dt) }
            else { ai.drive_paths(&mut r, world, paths, line, course, 2, dt) };
        if r.crashed > 0.0 && !was_down { crashes += 1; println!("  wiped out at t {t:.1} ({}) speed {:.0} km/h", r.cause, r.vel.length() * 3.6); }
        if r.crashed > 0.0 && !was_down && std::env::var("TRICKY_AIDBG").is_ok() { println!("    flip {:.2} spin {:.2} air {:.2}", r.flip, r.spin, r.air_time); }
        if was_down && r.crashed <= 0.0 { println!("    back up after {:.1} s at {:.0} km/h", r.down_t, r.vel.length() * 3.6); }
        was_down = r.crashed > 0.0;
        if r.stumble > 0.0 && !was_stumble { stumbles += 1; }
        was_stumble = r.stumble > 0.0;
        if r.last_trick != last_seen { if !r.last_trick.is_empty() && std::env::var("TRICKY_TRICKLOG").is_ok() { println!("  t {t:.1}: {}", r.last_trick); } last_seen = r.last_trick.clone(); }
        if std::env::var("TRICKY_RESETS").is_ok() && world.in_reset(r.pos + Vec3::Y * 0.8, BODY_RADIUS) { println!("reset at t {t:.1} pos {:.1?} idx {} grounded {} speed {:.0}", r.pos, ai.idx, r.grounded, r.vel.length()); }
        if std::env::var("TRICKY_TRACE").is_ok() && ((t / 0.5) as i32) != (((t - dt) / 0.5) as i32) { println!("t {t:.1} pos {:.1?} vel {:.1?} speed {:.1} grounded {} normal {:.2?} surface {} steer {:.2} yaw {:.2} idx {}", r.pos, r.vel, r.vel.length(), r.grounded, r.normal, r.surface, input.steer, r.yaw, ai.idx); }
        let idx = ai.idx;
        let best = (idx, (line[idx.min(line.len() - 1)] - r.pos).length_squared());
        let before = r.pos;
        let air_before = r.air_time;
        if !paths.is_empty() && r.grounded && r.crashed <= 0.0 && r.rail.is_none() {
            if let Some((p, yaw)) = paths.respawn_point(r.pos) {
                let ground = world.ground(p + Vec3::Y * 3.0, 3.0, 20.0).map_or(p.y, |h| h.y);
                r.safe = (Vec3::new(p.x, ground, p.z), yaw);
                r.safe_external = true;
            }
        }
        // no scripts here: the iris doors' buttons open them directly
        let shut: Vec<bool> = world.gates.iter().map(|g| g.t.is_none()).collect();
        world.touch_gates(&[r.pos + Vec3::Y * 0.8]);
        if std::env::var("TRICKY_RESETS").is_ok() { for (g, s) in world.gates.iter().zip(shut) { if s && g.t.is_some() { println!("  t {t:.1}: door {} opens (open {:.1}..{:.1} s of {:.1})", g.inst, g.open.0, g.open.1, g.len); } } }
        world.tick_gates(dt);
        r.step(world, rails, input, dt);
        if let Some(tb) = tube { if tb.apply(&mut r, prog.laps, dt) { ai.reroute(); tube_t = t; println!("  t {t:.1}: up the tube, {} laps to go", prog.laps); } }
        if std::env::var("TRICKY_TUBE").is_ok() && ((t > tube_t - 2.0 && t < tube_t) || (t - tube_t < 3.0 && t >= tube_t)) && ((t * 10.0) as i32) != (((t - dt) * 10.0) as i32) { println!("    t {t:.1} pos {:.1?} vel {:.1?} grounded {} class {} down {:.1}", r.pos, r.vel, r.grounded, r.tube_class, r.crashed); }
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
        for e in lines.update(&mut prog, r.pos, r.vel) {
            println!("  t {t:.1}: course event {} (value {}) on line {}, {:.0} m to go", e.kind, e.value, prog.line, prog.dtf);
            if e.kind == crate::course::EV_FINISH && finished.is_none() { println!("  finish event at t {t:.1}"); }
        }
        if ai.finished.is_some() { finished = Some(t); break; }
        let _ = (&mut stuck_for, &mut last_idx);
    }
    match finished {
        Some(t) => {
            println!("FINISHED in {t:.1} s");
            // over the line: the finish state brakes the rider to a stop
            r.start_finish(true);
            let v0 = r.vel.length();
            let mut stop_at = None;
            for i in 0..(8 * 120) {
                let inp = r.finish_input(Input::default());
                r.step(world, rails, inp, 1.0 / 120.0);
                r.finish_after(1.0 / 120.0);
                if stop_at.is_none() && r.finished_and_stopped() { stop_at = Some(i as f32 / 120.0); }
            }
            println!("finish state: {:.1} m/s at the line, state {:?}, results after {:?} s", v0, r.fin.map(|f| f.0), stop_at);
        }
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
    println!("wipe-outs: {crashes}, stumbles: {stumbles}, score {}, last trick \"{}\"", r.score, r.last_trick);
    println!("ground/air switches: {flips} ({:.1} per second)", flips as f32 / t.max(1.0));
    println!("distance {:.0} m, top speed {:.0} km/h, airborne {:.0} s, on rails {:.0} s, autopilot stuck {} times, fell out of the world {} times", dist, top * 3.6, air, grind, ai.stuck + stuck, r.respawns - ai.stuck - stuck);
}

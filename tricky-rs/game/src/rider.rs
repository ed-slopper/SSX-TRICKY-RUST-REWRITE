//! Snowboarder physics. Plain functions over plain data so the same code runs in the game and
//! in the headless self-test. Bevy space: metres, seconds, Y up.
//!
//! The speeds, gravity, drag, turning, jump and rotation numbers are the original game's.

use crate::collide::CollisionWorld;
use crate::rails::Rails;
use crate::trickdata::{RiderData, COMBOS, RIDERS};
use crate::score::{Score, ScoreCall};
use glam::Vec3;
use crate::surface::SURFACE_TABLE;
use crate::orient::{self, Orient};
pub mod wipeout;
pub mod ground_tick;
pub use wipeout::{Wipe, course_points};

// The numbers below are the original game's (SLUS_203.26), converted from its cm and 60 Hz ticks to metres and
// seconds; each says where it is read (row E8i).
/// gravity in the air, m/s²: gentle on the way up, hard on the way down (`Air_IntegrateRK4` 0x12b340, crate::air)
pub const GRAVITY_RISING: f32 = -crate::air::GRAVITY_RISING * 0.01;
pub const GRAVITY_FALLING: f32 = -crate::air::GRAVITY_FALLING * 0.01;
/// The ground spring's band per surface (m): sunk deeper than d1 it pushes back fully, d2 is how far a
/// board can sink (powder 15-35 cm). `SurfaceTable_Init` 0x256188 words [7] and [8] (crate::surface), cm → m.
pub const SURF_BAND: [(f32, f32); 20] = {
    let mut t = [(0.0, 0.0); 20];
    let mut i = 0;
    while i < 20 {
        t[i] = (SURFACE_TABLE[i][7] * 0.01, SURFACE_TABLE[i][8] * 0.01);
        i += 1;
    }
    t
};
/// the speed limit on the snow and while boosting, m/s (`cBoarder::vf7` 0x117198: 2788.8394 cm/s; boosting with
/// the meter above 0, 0.33362994, 0.66599226 or with a speed-boost pickup 2932.43, 3072.1418, 3347.2222)
pub const SPEED_CAP: f32 = 2788.8394 * 0.01;
pub const BOOST_CAPS: [f32; 3] = [2932.43 * 0.01, 3072.1418 * 0.01, 3347.2222 * 0.01];
/// once the boost stops the limit comes back down 3.472222 cm/s a tick (`cBoarder::vf7` 0x117198), in m/s²
pub const CAP_DECAY: f32 = 3.472222 * 60.0 * 0.01;
/// STANDIN: our AI's corner-speed estimate (m/s², about g·tan(58.3°·0.905)), for the game's AI path following (G4)
pub const TURN_ACCEL: f32 = 17.1;
/// a spin's rate with no wind-up (rad/s), for the TRICKY_TRICKTEST self-test's timing only; the game's rates are
/// `Takeoff_SetSpinRates` 0x126e30
pub const AIR_SPIN: f32 = 4.63;
/// the smallest jump, m/s (`JumpControl_Update` 0x1046e4: 630.8786 cm/s)
pub const JUMP_MIN: f32 = 630.8786 * 0.01;
/// seconds of boost in a full meter: holding boost drains 0.000750150706 (0x3a44a5c3) a tick (`Boarder_UpdateBoost` 0x11d218)
pub const BOOST_SECONDS: f32 = 1.0 / (f32::from_bits(0x3a44_a5c3) * 60.0);
// STANDIN: one sphere round the body for walls, for the game's collision spheres and `Boarder_WallCollide` probes
pub const BODY_RADIUS: f32 = 0.45;
/// how close (m) the board has to pass to a rail, while in the air, to lock on
/// STANDIN: our reach, for the game's rail capture test (`Rail_TryCapture`)
pub const RAIL_REACH: f32 = 0.9;
/// how fast the board turns on a rail when steered (rad/s)
/// a quarter turn on a rail is its 15-frame clip: 28 ticks (`RailSlideControl_CommitQuarterTurn` 0x107730)
pub const RAIL_TWIST: f32 = std::f32::consts::FRAC_PI_2 / (28.0 / 60.0);
// ---- scoring: the score object's own functions (`score.rs`, checked bit for bit) on `Rider::sc`.
/// leaving a rail switch or fakie: the next air trick × 1.3 (`Score_StartTrickChain` 0x156618)
pub const SWITCH_BONUS: f32 = f32::from_bits(0x3fa6_6666);
/// a full meter gives this long to do uber tricks (`Score_StartGrab` 0x155ee8 and 0x156050 set it to 20.0)
pub const UBER_SECONDS: f32 = 20.0;

/// The original's surface table (`SurfaceTable_Init` 0x256188, crate::surface), by the patch's surface type:
/// gravity (cm/s^2), drag (linear, v^2, v^3 with v in 10 m/s), side grip, most lean (degrees), the speed the
/// rider pushes himself to (km/h) and how hard: words [0..=5], [11], [12]. Rows: 0 off the course, 1 groomed
/// snow, 2 second-line snow, 3 powder, 4 deep powder, 5 ice, 6 structures, 9 rock and concrete, 13 metal,
/// 18 show-off ramps.
pub const SURFACES: [[f32; 8]; 20] = {
    let mut t = [[0.0; 8]; 20];
    let mut i = 0;
    while i < 20 {
        let r = &SURFACE_TABLE[i];
        t[i] = [r[0], r[1], r[2], r[3], r[4], r[5], r[11], r[12]];
        i += 1;
    }
    t
};

/// The rest of the surface table: how far above the snow (cm) the board may be before it is in
/// the air, and how hard the snow damps the board's bounce (1/s): words [6] and [9]. Indexed like `SURFACES`.
pub const SURF_EXTRA: [[f32; 2]; 20] = {
    let mut t = [[0.0; 2]; 20];
    let mut i = 0;
    while i < 20 {
        t[i] = [SURFACE_TABLE[i][6], SURFACE_TABLE[i][9]];
        i += 1;
    }
    t
};

/// One axis of rotation in the air (`SpinState_Update`): rate, the band it is held in, and the
/// largest error the auto-complete may still use.
#[derive(Clone, Copy, Default)]
pub struct Axis { pub w: f32, pub hi: f32, pub lo: f32, pub floor: f32, pub emax: f32 }
impl Axis {
    pub fn start(w0: f32) -> Self { Self { w: 0.0, hi: w0, lo: 0.3 * w0, floor: w0.abs().min(1.396), emax: std::f32::consts::TAU } }
    /// The stick is let go for the first time: the rotation may slow to nothing but never reverse.
    pub fn let_go(&mut self) {
        if self.hi < 0.0 { self.hi = 0.0 } else { self.lo = 0.0 }
        self.floor = 0.0;
        self.emax = std::f32::consts::TAU;
    }
    pub fn step(&mut self, dir: f32, complete: bool, gain: f32, shrink: f32, dt: f32, err: impl Fn(f32) -> f32) -> f32 {
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
    pub gate: f32,
    /// the game's 16 stat bytes (stats+0x0e..0x1d, `crate::stats`), as `Career_SetupPlayerRider` 0x16a9d8 leaves them:
    /// the ground tick reads speed, edging, grip and the cubic term from them, the jump its stat byte
    pub bytes: [u8; 16],
    /// stats+0x60, the rider's body scale (`crate::stats::SCALE`): the ground probe starts `scale · lean` to the side
    pub scale: f32 }
impl Default for Stats {
    /// STANDIN: a neutral rider for tests: the bytes are our fractions as bytes, for the game's career (row G3), and
    /// a body scale of 1 (no rider's)
    fn default() -> Self {
        let mut bytes = [0u8; 16];
        for (i, f) in [(crate::stats::SPEED, 0.5), (crate::stats::EDGING, 0.5), (crate::stats::GRIP, 0.5), (crate::stats::CUBIC, 0.5), (crate::stats::JUMP, 0.75), (crate::stats::STABILITY, 0.5), (crate::stats::LEAK, 0.5), (crate::stats::METER, 0.5), (crate::stats::WINDUP, 0.75), (crate::stats::START, 0.7)] {
            bytes[i] = stat_byte(f);
        }
        Self { edging: 0.5, speed: 0.5, stability: 0.5, tricks: 0.5, jump: 0.75, windup: 0.75, weight: 75.0, alpine: false, kind: 1, meter_leak: 0.5, gate: 0.7, bytes, scale: 1.0 }
    }
}
impl Stats {
    /// A new rider or a fully trained one, on the chosen board: the game's own mix of 80% rider
    /// and 20% board.
    /// `training` 0..1: from the character's starting attributes to its caps (career points / 240
    /// fill every bar in the original; computer riders gain a quarter of the player's points).
    pub fn of(d: &RiderData, training: f32, board: &crate::trickdata::Board) -> Self {
        let t = training.clamp(0.0, 1.0);
        let f = |v: (u8, u8), g: usize| (0.8 * (v.0 as f32 + (v.1 as f32 - v.0 as f32) * t) + 0.2 * board.bonus[g] as f32) / 100.0;
        // the game's stat bytes (`Career_SetupPlayerRider` 0x16a9d8). STANDIN: each group's level is our training
        // fraction between its start and its cap, for the career's level bytes (row G3)
        let bytes = crate::trickdata::RIDERS.iter().position(|r| r.name == d.name).map_or([0; 16], |i| {
            let lvl = |v: (u8, u8)| (v.0 as f32 + (v.1 as f32 - v.0 as f32) * t).round() as u8;
            crate::stats::player_bytes(&crate::stats::BASE[i], &crate::stats::CAPS[i], board.bonus, [lvl(d.edging), lvl(d.speed), lvl(d.stability), lvl(d.tricks)], false)
        });
        let scale = crate::trickdata::RIDERS.iter().position(|r| r.name == d.name).map_or(1.0, |i| crate::stats::SCALE[i]);
        Self { edging: f(d.edging, 0), speed: f(d.speed, 1), stability: f(d.stability, 2), tricks: f(d.tricks, 3), bytes, scale,
               jump: d.jump as f32 / 100.0, windup: d.windup as f32 / 100.0, weight: d.weight as f32, alpine: board.kind == 2, kind: board.kind, meter_leak: 0.5,
               gate: match d.name { "Kaori" | "Marisol" => 0.80, "Luther" => 0.64, "Mac" => 0.85, "Moby" | "Seeiah" => 0.75, "Zoe" => 0.82, "JP" => 0.88, _ => 0.70 } }
    }
    /// The player's rider from the career: each group's fraction is the rider's level (the career record's +0xc..+0xf,
    /// as `Career_SetupPlayerRider` reads them for the stat bytes) mixed 80% with the board's 20%, the same mix the
    /// stat bytes are made with
    pub fn of_levels(d: &RiderData, levels: [u8; 4], board: &crate::trickdata::Board) -> Self {
        let mut s = Self::of(d, 0.0, board);
        let f = |g: usize| (0.8 * levels[g] as f32 + 0.2 * board.bonus[g] as f32) / 100.0;
        (s.edging, s.speed, s.stability, s.tricks) = (f(0), f(1), f(2), f(3));
        s
    }
    pub fn drag_linear(&self) -> f32 { if self.alpine { 0.7076 - 0.3020 * self.speed } else { 1.0650 - 0.3085 * self.speed } }
    pub fn drag_cubic(&self) -> f32 { 1.2848 - 0.2904 * self.speed }
    pub fn push(&self) -> f32 { if self.alpine { 1.2050 + 0.3050 * self.speed } else { 0.7381 + 0.2769 * self.speed } }
    pub fn brake(&self) -> f32 { 1.7513 * (0.615 + 0.918 * self.edging) }
    pub fn spin_scale(&self) -> f32 { 0.5449 + 0.6742 * self.tricks }
    pub fn spin_decay(&self) -> f32 { (0.99301 + 0.00201 * self.tricks).powi(60) }
    pub fn wind_time(&self) -> f32 { 1.0 / (2.0005 * crate::boarder::windup_factor(self.bytes[crate::stats::WINDUP])) }
    pub fn jump(&self, crouch: f32, speed: f32) -> f32 {
        // `Jump_ApplyImpulse` 0x1284e0, in m/s: S = 0.8809335·v + 24.678448 cm/s below 993.4801 cm/s, else 899.86835
        let by_speed = if speed < 9.934801 { 0.24678448 + 0.8809335 * speed } else { f32::from_bits(0x4460_f793) * 0.01 };
        JUMP_MIN.max(crouch * crouch * (0.09851468 + 0.7870027 * self.jump) * by_speed)
    }
    /// how far from upright (radians) a landing can be
    pub fn crash_pitch(&self) -> f32 { 2.684 / (2.0408 - 1.0374 * self.stability) }
    pub fn meter(&self) -> f32 { 0.9788 + 0.3597 * self.tricks }
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
    pub safe_timer: f32,
    /// the safe spot is kept by the course (the AI path network), not by the rider
    pub safe_external: bool,
    pub respawns: u32,
    /// grinding: (rail index, distance along it, signed speed along it)
    pub rail: Option<(usize, f32, f32)>,
    pub rail_cooldown: f32,
    /// what the controls were on the last step (the pose follows it)
    pub input: Input,
    // ---- tricks
    /// signed turns of spin and flip made since leaving the snow (radians)
    pub spin: f32,
    pub flip: f32,
    pub flip_dir: f32,
    /// wind-up before a jump (-1..1) for spin and flip, and the rotation rates the jump started with
    pub wind: f32,
    pub wind_flip: f32,
    /// the speed limit right now (it lags behind when a boost ends)
    pub cap: f32,
    pub steer: f32,
    pub grab: u8,
    /// seconds the current grab has been held, which grabs have counted this jump, which were tweaked
    pub grab_hold: f32,
    /// a grab let go of, still playing out its clip: (grab, frame)
    pub grab_out: Option<(u8, f32)>,
    pub grab_done: [bool; 16],
    pub grab_tweaked: [bool; 16],
    pub ubers_done: u8,
    pub switch_takeoff: bool,
    pub from_rail: bool,
    /// seconds left in which uber tricks can be done
    pub uber_timer: f32,
    pub stats: Stats,
    pub data: &'static RiderData,
    /// surface-table row of the snow under the board
    pub surface: u8,
    pub grind_time: f32,
    /// on a rail: the board's angle to the direction of travel, how far it has been turned in all,
    /// and how long it has been ridden sideways
    pub rail_twist: f32,
    pub rail_spun: f32,
    pub rail_side: f32,
    pub rail_entry: f32,
    pub rail_goal: f32,
    pub rail_held: bool,
    pub rail_side_signed: f32,
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
    pub jump_c: f32,
    pub jump_delay: f32,
    /// the board's height above the snow (m, along the normal) from the last probe
    pub height: f32,
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
    pub gate_p: f32, pub gate_hi: f32, pub gate_lo: f32, pub gate_still: f32, pub gate_target: f32, pub gate_speed: f32, pub gate_state: u8, pub gate_rock_dir: i32,
    /// the ground spring band in use (eases toward the surface's)
    pub band: (f32, f32),
    /// a "Late" trick: the rotation already banked in this jump, and that the next part is late
    pub spin_base: f32,
    pub flip_base: f32,
    pub late: bool,
    pub spin_used: bool,
    /// what put the rider down last (for the headless checks)
    pub cause: &'static str,
    /// rotation in the air: spin and flip, and whether the stick has been let go
    pub spin_ax: Axis,
    pub flip_ax: Axis,
    pub air_mode2: bool,
    /// on a rail: sideways offset from it (m) and seconds on it
    pub rail_off: f32,
    pub rail_time: f32,
    pub rail_halves: u32,
    pub boost_was: bool,
    /// tricks landed in a row (and rails over a second): two or more pay a chain bonus
    pub chain: u32,
    // ---- uber tricks
    /// this rider's signature moves: (clip name, length in frames at 30 a second)
    pub ubers: Vec<(String, f32)>,
    /// frames into the current uber trick (0 = not doing one) and which one
    pub uber: f32,
    pub uber_id: usize,
    pub uber_done: bool,
    /// uber tricks landed this run; six spell TRICKY and make boost unlimited
    pub letters: u8,
    /// from a multiplier pickup: applied to the next trick landed
    pub multiplier: u32,
    /// the score object (rider+0x5820): every trick, rail, grab, crash and bank goes through the game's own
    /// functions in `score.rs` (checked bit for bit); `score` and `letters` are read from it
    pub sc: Score,
    /// time not yet run through the 60 Hz score tick and meter drain (`Score_Tick`, `Boarder_DrainMeter`)
    pub tick_acc: f32,
    /// rider+0x28: full meters, mod 4 (`Boarder_AddMeter`)
    pub full_count: i32,
    /// rider+0x1dc on a rail: how the board sits on it (1 along, 2 backward, 3 and 4 across), from the capture
    pub rail_kind: u32,
    /// the rail motion object's +8: the lean angle (rad) that tips the normal on a steep rail, zero at the capture
    /// (`RailSlideMotion_Enter` 0x10b068), moved on by `ground_tick::rail_lean_angle` every tick on the rail
    pub rail_lean: f32,
    /// the rail quarter turns committed so far (`RailSlideControl_CommitQuarterTurn`), from the entry angle
    pub rail_goal0: f32,
    pub rail_committed: f32,
    /// the parts of the trick being shown, joined when its points come in
    pub show_parts: Vec<String>,
    /// this grab started scoring (`Score_StartGrab`) and was tweaked (its rate and its name the tweak's)
    pub grab_scored: bool,
    pub grab_tw: bool,
    /// the game-side rider of the exact ground tick (`ground_tick.rs`), kept between ticks
    pub gm: Option<ground_tick::GroundCache>,
    /// the crash on surface 6 or 10 sprays from here: (position, velocity, normal), for the effects (row G19)
    pub crash_spray: Option<(Vec3, Vec3, Vec3)>,
    /// the lean record (board row G20b): rider+0x244 the value (the probe's sideways offset, with `stats.scale`),
    /// +0x248 the rate and +0x24c the target, eased by `lean_ease` and aimed by `lean_aim` (`ground_tick.rs`)
    pub lean: [f32; 3],
    /// the rider's up, rider+0x2c0 (board row G20c), in the game's space (z up, w 0): the normal at `Boarder_PlaceAt`
    /// 0x11cae8, then unit(up + 0.5 · normal) at the end of every tick (`lean_up`, `Boarder_UpdateLeanTargets`
    /// 0x129500); the ground tick's springs and bob read it, `Boarder_UpdatePose` 0x129628 draws along it
    pub up: [f32; 4],
    /// rider+0x308: how high the body is drawn above the snow, cm (board row G16; `pose::ease_body_height`): the
    /// model is drawn at the position + this along the rider's up (`Boarder_UpdatePose` 0x129628)
    pub body_height: f32,
    /// the board's attitude the game keeps through the air (board rows G20c3b, G20c3c): the quaternion rider+0x170 and
    /// the axes +0x180..+0x1b0, and the port's yaw they were made for. Set at a placement (`Boarder_PlaceAt` 0x11cae8,
    /// `orient::placed_attitude`) and at a take-off off the snow (what the ground tick that took off left:
    /// `GroundCache::attitude`). The game's quaternion is not turned by the trick's spin and flip (those are the
    /// separate angles rider+0x1d0 and +0x1d4, applied at the pose, `pose::board_frame`), so it stays as it was until
    /// the landing; `seed_ground` starts the ground tick from it while the port left the yaw alone, spin and flip at
    /// zero. Dropped by the first ground tick (the game-side rider's own quaternion takes over) and by any spin or
    /// flip in the air (their landing alignment is `SpinState_Update`, rows G20c3c2 and G1a5b3b); only the wall bump
    /// of `AirMotion_Update` 0x108378 would turn it besides (row G20c3c3: the port's air walls do not).
    pub att: Option<(Orient, f32)>,
    /// the last patch the ground probe hit (rider+0x430), for `World_RayCast` on the course (row E7c3)
    pub last_hit: crate::terrain::LastHit,
    /// +0x2e4: the walls' impacts (`Boarder_AccumulateImpact` 0x125c48), decaying each tick ([`Rider::impact_tick`])
    pub impact: f32,
    /// +0x2f0: the last impact's normal, shrinking each tick
    pub impact_n: [f32; 4],
    /// +0x300: the air's wall knocks (`Boarder_WallImpactCheck` 0x1265c8 in motion 1), decaying each tick
    pub knocks: f32,
    /// +0x460: the rider's index in the race (0 the player, the computer riders after it)
    pub slot: u32,
    /// a placement's ray still to cast (`Boarder_PlaceAt`: the place, the game's heading): the respawn has no world,
    /// so the rider's next step casts it
    pub place_ray: Option<(Vec3, f32)>,
    /// +0x2e0: the kind of the ground probe's last hit (0 terrain, 1 an object with no surface, 2 a moving one), kept
    /// from the landing's probe into the ground tick and back
    pub hit_kind: u32,
    /// ticks before this rider can be hit by another again (`Boarder_RiderCollisions`: three ticks)
    pub hit_wait: u8,
}

pub fn heading(yaw: f32) -> Vec3 { Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) }

/// The first argument `Boarder_PlaceAt` 0x11cae8 gets for a port yaw (board row G20c3b): the angle of the board's
/// forward from +x in the game's space (x, y flat, z up), which is `atan2(dir.y, dir.x)` of the direction the callers
/// read from the path (`ResetState_PlaceRider` 0x119220, the start line in `cBoarder::vf8` 0x116ab8; both with the
/// inline atan2 of `Boarder_UpdateOrientation` 0x129008, so in (−π, π]). The port's forward `heading(yaw)` is
/// (−sin, 0, −cos) in x, y up, z toward the viewer; as the game's (x, −z, y) it points at atan2(cos yaw, −sin yaw)
/// = yaw + π/2, wrapped.
pub fn placed_heading(yaw: f32) -> f32 { wrap(yaw + std::f32::consts::FRAC_PI_2) }

impl Rider {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        let mut r = Self { pos, vel: Vec3::ZERO, yaw, grounded: false, normal: Vec3::Y, charge: 0.0, air_time: 0.0,
               spawn: (pos, yaw), safe: (pos, yaw), safe_timer: 0.0, safe_external: false, respawns: 0, rail: None, rail_cooldown: 0.0, input: Input::default(),
               spin: 0.0, flip: 0.0, flip_dir: 0.0, wind: 0.0, wind_flip: 0.0, cap: SPEED_CAP, steer: 0.0, grab: 0, grab_hold: 0.0, grab_out: None, grab_done: [false; 16], grab_tweaked: [false; 16], ubers_done: 0, switch_takeoff: false, from_rail: false, uber_timer: 0.0, stats: Stats::default(), data: &RIDERS[0], surface: 1, grind_time: 0.0, rail_twist: 0.0, rail_spun: 0.0, rail_side: 0.0, rail_entry: 0.0, rail_goal: 0.0, rail_held: false, rail_side_signed: 0.0, shove: 0.0, shove_side: 1.0, taunt: -1.0, taunt_clip: String::new(), fin: None, showoff: false, landed: Vec::new(), crash_kind: 0, land_kind: 0, switch: false, score: 0, boost: 0.0, boosting: false, body_height: 0.0,
               last_trick: String::new(), trick_timer: 0.0, crashed: 0.0, hard_land: 0.0, crouch_v: 0.0, brake_v: 0.0, jump_c: 0.0, jump_delay: 0.0, height: 0.0,
               course_dir: None, course_pts: None, wipe: None, crash_at: None, gm: None, crash_spray: None, lean: [0.0; 3], up: [0.0, 0.0, 1.0, 0.0], recover: None, ai: false, down_t: 0.0, getup: -1.0, stumble: 0.0, wall_hit: None, speed_timer: 0.0, spin_timer: 0.0, spin_used: false, tube_class: u8::MAX, gate_p: 0.0, gate_hi: 0.0, gate_lo: 0.0, gate_still: 0.0, gate_target: 0.0, gate_speed: 0.0, gate_state: 8, gate_rock_dir: 0, band: (0.005, 0.0250417), spin_base: 0.0, flip_base: 0.0, late: false, cause: "", spin_ax: Axis::default(), flip_ax: Axis::default(), air_mode2: false, rail_off: 0.0, rail_time: 0.0, rail_halves: 0, boost_was: false, chain: 0,
               ubers: Vec::new(), uber: 0.0, uber_id: 0, uber_done: false, letters: 0, multiplier: 1,
               sc: new_score(), tick_acc: 0.0, full_count: 0, rail_kind: 0, rail_lean: 0.0, rail_goal0: 0.0, rail_committed: 0.0, show_parts: Vec::new(), grab_scored: false, grab_tw: false, hit_wait: 0, att: None, last_hit: Default::default(), impact: 0.0, impact_n: [0.0; 4], knocks: 0.0, slot: 0, place_ray: None, hit_kind: 0 };
        // the start line's `Boarder_PlaceAt` 0x11cae8 (`cBoarder::vf8` 0x116ab8: heading from the path's direction)
        r.att = Some((orient::placed_attitude(placed_heading(yaw), None), yaw));
        r
    }
    /// `Boarder_Respawn`: dropped a metre above the snow, already moving down the course at 30 km/h.
    pub fn respawn(&mut self, at: (Vec3, f32)) {
        self.place(at.0, at.1, placed_heading(at.1));
    }
    /// A respawn at the game's own heading (`Boarder_PlaceAt`'s f12, as its callers make it: the inline atan2 of a
    /// direction in the game's space, `jump::atan2_inline`), kept exact on the rider; the port's yaw follows from it
    pub fn respawn_heading(&mut self, pos: Vec3, heading: f32) {
        self.place(pos, wrap(heading - std::f32::consts::FRAC_PI_2), heading);
    }
    fn place(&mut self, pos: Vec3, yaw: f32, game_heading: f32) {
        let at = (pos, yaw);
        self.place_ray = Some((pos, game_heading));
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
        self.grind_time = 0.0;
        // `Boarder_PlaceAt` 0x11cae8 zeroes the lean record (+0x244, +0x248, +0x24c)
        self.lean = [0.0; 3];
        // `Boarder_PlaceAt` 0x11cae8 sets the normal (+0x2a0) to the ray hit's under the spawn, (0,0,1,0) when it hits
        // nothing, and copies it to the rider's up (+0x2c0): flat here, and the hit's when the next step casts the ray
        // (`place_ray`; row G20c3b4)
        self.normal = ground_tick::port_dir(ground_tick::placed_normal(None));
        self.up = ground_tick::game_dir(self.normal);
        // and its attitude (G20c3b), which the game keeps through the drop: the quaternion is only turned by the
        // placement's ray (cast on the next step, `place_ray`), the heading and nothing else
        self.att = Some((orient::placed_attitude(game_heading, None), self.yaw));
        // `Boarder_PlaceAt` 0x11cae8: the chain banked, its meter change (−0.12) applied
        let (m, calls) = self.sc.bank_points();
        self.show(&calls);
        self.add_meter(m);
        self.respawns += 1;
    }
    pub fn clear_air(&mut self) {
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
        self.ubers_done = 0;
        self.uber = 0.0;
        self.uber_done = false;
    }
    /// The boost meter (`Boarder_AddMeter` 0x11b018, exact): the gain scaled by the meter stat, clamped to 0..1; a
    /// gain at a full meter (re)opens the twenty-second uber window.
    pub fn add_meter(&mut self, units: f32) {
        let mut f = crate::boarder::Fields { meter: self.boost, uber_timer: self.uber_timer, full_count: self.full_count, ..Default::default() };
        crate::boarder::add_meter(units, self.stats.bytes[crate::stats::METER], &mut f);
        self.boost = f.meter;
        self.uber_timer = f.uber_timer;
        self.full_count = f.full_count;
    }
    /// The game's per-tick score step and meter drain (`Score_Tick` 0x156e80, `Boarder_DrainMeter` 0x11b1f8), at
    /// 60 Hz whatever the physics step.
    fn score_ticks(&mut self, dt: f32) {
        // rider+0x418: −1 until the line, then the finish time (`Boarder_Finish` 0x11da40: race time × 100);
        // only its sign is read here (STANDIN: the time itself is not kept)
        self.sc.rider_418 = if self.fin.is_some() { 0 } else { -1 };
        self.tick_acc += dt;
        while self.tick_acc >= 1.0 / 60.0 {
            self.tick_acc -= 1.0 / 60.0;
            self.hit_wait = self.hit_wait.saturating_sub(1);
            let speed = match self.rail { Some((_, _, s)) => s.abs(), None => self.vel.length() } * 100.0;
            self.sc.tick(speed);
            let mut f = crate::boarder::Fields { meter: self.boost, uber_timer: self.uber_timer, full_count: self.full_count, ..Default::default() };
            // STANDIN: rider+0x424 = 1 taken as "in the air"; the drain stat (stats+0x1b) is our meter_leak
            let air = !self.grounded && self.rail.is_none();
            crate::boarder::drain_meter(air, self.fin.is_none(), self.stats.bytes[crate::stats::LEAK], self.tricky(), &mut f);
            self.boost = f.meter;
            self.uber_timer = f.uber_timer;
        }
        self.sync_score();
    }
    fn sync_score(&mut self) {
        self.score = self.sc.xf0;
        self.letters = self.sc.x88.clamp(0, 6) as u8;
    }
    /// What the score functions said, for the HUD: the trick's name (`Score_FormatTrickName` on the id the
    /// announcer gets), its bonuses, and its points when they come in.
    fn show(&mut self, calls: &[ScoreCall]) {
        for c in calls {
            // `Scoring_PushMessage` 0x157330: every message into the score's timed entries
            if let ScoreCall::Message(life, kind, value, extra) = *c {
                self.sc.push_message(life, kind, value, extra);
            }
            // `Score_AnnounceTrick` 0x157088: the id kept and its message
            if let ScoreCall::Announce(id, repeat) = *c {
                if let ScoreCall::Message(life, kind, value, extra) = self.sc.announce_trick(id, repeat) {
                    self.sc.push_message(life, kind, value, extra);
                }
            }
            match *c {
                ScoreCall::Announce(id, _) => {
                    let def = tricky_data::trickdef::TrickDef { words: id, flip: 0, spin: 0, rail: 0, plus: [0; 2], slot: [0; 2] };
                    let name = def.name().split_whitespace().collect::<Vec<_>>().join(" ");
                    if !name.is_empty() && !self.ai && self.fin.is_none() { self.landed.push(name.clone()); }
                    self.show_parts = vec![name];
                }
                // the chain bonus and big air; the labels are ours (STANDIN: the HUD's own text is not ported)
                ScoreCall::Message(_, 0xf, v, _) => self.show_parts.push(["1x COMBO", "2x COMBO", "3x COMBO", "4+ COMBO"][((v / 4000 - 1).clamp(0, 3)) as usize].to_string()),
                ScoreCall::Message(_, 0x10, _, _) => self.show_parts.push("BIG AIR BONUS".into()),
                ScoreCall::Message(_, 2 | 3, v, _) if v > 0 => {
                    let parts: Vec<String> = self.show_parts.drain(..).filter(|s| !s.is_empty()).collect();
                    self.last_trick = format!("{}  +{v}", parts.join(" + "));
                    self.trick_timer = 3.0;
                }
                _ => {}
            }
        }
        self.sync_score();
    }
    /// `Score_StartTrickChain` 0x156618 (a jump, or leaving a rail): the switch flag is the stance, `kind` rider+0x1dc
    /// (0 from snow, the rail's kind leaving one), `steep` `Jump_ApplyImpulse`'s steep lip; the share of a rail
    /// being left goes to the meter.
    fn begin_chain(&mut self, kind: u32, steep: bool) {
        let (share, calls) = self.sc.start_trick_chain(self.switch as u32, kind, steep as u32, 0, !self.ai);
        self.show(&calls);
        self.add_meter(share);
    }
    /// `Boarder_Wipeout` 0x11d830: down into the tumble; costs a tenth of the meter (`Score_Crash` 0x156de0), and a
    /// running uber window closes with the meter cut to two thirds.
    pub fn wipe_out(&mut self) { self.go_down(true); }
    /// A wipe-out on demand (TRICKY_CRASHAT checks): as if the landing went wrong.
    pub fn force_wipe(&mut self) { self.cause = "forced"; self.crash_kind = 0; self.wipe_out(); }
    pub fn go_down(&mut self, lose_meter: bool) {
        if self.crashed > 0.0 { return; }
        self.fall();
        // `Score_Crash` 0x156de0: the chain lost, the trick so far banked, −0.1 meter
        let (m, calls) = self.sc.crash(false);
        self.show(&calls);
        if lose_meter { self.add_meter(m); }
    }
    /// `Boarder_Wipeout` 0x11d830 itself: down, unscored (its callers score it).
    fn fall(&mut self) {
        if self.crashed > 0.0 { return; }
        self.last_trick = "CRASH".into();
        self.trick_timer = 2.0;
        // a crash in the middle of a trick in the uber window cuts the meter to two thirds and closes it
        let mid_trick = !self.grounded && (self.sc.units > 0.0 || self.spin.abs() > 1.0 || self.flip.abs() > 1.0 || self.sc.grabs[0] != 0);
        if !self.tricky() && self.uber_timer > 0.0 && mid_trick { self.boost = self.boost.min(0.66599226); self.uber_timer = 0.0; }
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
    /// `Boarder_Stumble` 0x124dd0: a knock that does not put you down (state 2): a moment off balance.
    pub fn stumble_by(&mut self, meter: bool) {
        if self.crashed > 0.0 || self.stumble > 0.0 { return; }
        if meter { self.add_meter(-0.02); }
        self.stumble = 0.5;
    }
    /// `SpinState_StartFromStick` 0x102158 in mid-air: the trick so far is named and counted toward repeats
    /// (`Score_CommitAndRestartChain`), a new chain starts and the rest is a "Late" trick.
    pub fn bank_late(&mut self) {
        let a = (self.switch != self.data.goofy) as u32;
        let (m, calls) = self.sc.commit_and_restart_chain(a, !self.ai);
        self.show(&calls);
        self.add_meter(m);
        self.grab_done = [false; 16];
        self.grab_tweaked = [false; 16];
        // STANDIN: the spin and flip count again from here (the original's rider+0x1d0/+0x1d4 at a restart is
        // not read yet)
        self.spin_base = self.spin;
        self.flip_base = self.flip;
        self.late = true;
    }
    /// Called when the board touches down: onto snow (`AirMotion_Update` 0x108378: `Score_LandTrick`(0, landing
    /// switch, normal z < 0.7)) or onto a rail (`Rail_TryCapture` 0x125cb8: `Score_LandTrick`(rail kind, stance, 0)).
    pub fn land(&mut self, rail: Option<u32>) {
        // the spin-boost is spent on the jump it was used for
        if self.spin_used { self.spin_timer = 0.0; self.spin_used = false; }
        let tau = std::f32::consts::TAU;
        let off = (self.flip.rem_euclid(tau)).min(tau - self.flip.rem_euclid(tau));
        let uber_cut_short = self.uber > 0.0 && !self.uber_done;
        if rail.is_none() && (off > self.stats.crash_pitch() || !self.grab_safe() || uber_cut_short) {
            // too far from upright, or still holding the board: wipe out
            self.crash_kind = if self.flip < -0.5 { 1 } else { 0 };
            if std::env::var("TRICKY_AIDBG").is_ok() { println!("    landing: flip {:.2} off {:.2} limit {:.2} spin {:.2} grab {} hold {:.2} uber {}", self.flip, off, self.stats.crash_pitch(), self.spin, self.grab, self.grab_hold, uber_cut_short); }
            self.cause = if off > self.stats.crash_pitch() { "landing angle" } else if uber_cut_short { "uber" } else { "grab" };
            self.wipe_out();
        } else {
            // a grab still held is let go as the board lands
            self.release_grab();
            let (kind, a, b) = match rail {
                Some(k) => (k, (self.switch != self.data.goofy) as u32, 0),
                None => (0, self.switch as u32, (self.normal.y < 0.7) as u32),
            };
            if std::env::var("TRICKY_SCOREDBG").is_ok() { println!("    land: spin {:.3} base {:.3} flip {:.3} base {:.3} goofy {} switch {} sc.spin {} sc.flip {} x10 {} x20 {} grabs {:?}", self.spin, self.spin_base, self.flip, self.flip_base, self.data.goofy, self.switch, self.sc.spin, self.sc.flip, self.sc.x10, self.sc.x20, self.sc.grabs); }
            let (share, calls) = self.sc.land_trick(kind, a, b);
            self.show(&calls);
            self.add_meter(share);
            self.multiplier = 1;
        }
        self.from_rail = false;
        self.clear_air();
    }
    pub fn pick_multiplier(&mut self, m: u32) {
        // `Pickup_Multiplier` 0x11e660 counts it in show-off for a rider whose +0x424 is 1 or 3. STANDIN: that
        // word is taken to be in the air or on a rail (not confirmed)
        if self.grounded || !self.showoff { return; }
        self.multiplier = self.multiplier.max(m);
        let _ = self.sc.set_pickup_multiplier(m as f32);
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
    /// The level scripts' boosts, teleports (op 0x18) and pickups; true on a teleport. A boost is `cBoostNode`'s
    /// Update (0x140688): a rider whose collision body its object's mesh hits (`cWorldBoarder`(rider+0x470, 1),
    /// `WorldNode_RayCast` 0x13aad0 > 0, on the course's collision, [`Rider::bind_body`]) gets its push (vf27
    /// 0x140838, `boosts.rs`): `d` = target − v·dir, when positive v += dir·d·rate per second (the game's d·rate/60
    /// a frame), never slowed, whatever the rider is doing. Nothing else lifts a rider off the ground: the ground
    /// tick's height test takes off (`GroundMotion_Update` 0x10a0d8, row E7c3g).
    /// STANDIN: the broadphase is the pad's box (round the object's meshes) against the body's bounding sphere's,
    /// for the boost's box in the game's broadphase (+0x70); without the course, the pad's box round the body
    /// point. Teleports and pickups are the pad's box round the body point, for their controllers (rows E9b2, E9c).
    pub fn apply_pads(&mut self, world: &CollisionWorld, dt: f32) -> bool {
        use crate::collide::PadKind;
        let point = self.pos + Vec3::Y * 0.8;
        let near = |p: &crate::collide::Pad| !(point.cmplt(p.lo - 0.3).any() || point.cmpgt(p.hi + 0.3).any());
        let mut body = None;
        for p in &world.pads {
            match p.kind {
                PadKind::Boost { dir, target, gain, inst } => {
                    let touching = match (&world.course, inst) {
                        (Some(course), Some(o)) => {
                            let b = body.get_or_insert_with(|| self.bind_body());
                            let (c, r) = (Vec3::new(b.center[0], b.center[2], -b.center[1]) * 0.01, b.radius * 0.01);
                            let overlap = !(c.cmplt(p.lo - r).any() || c.cmpgt(p.hi + r).any());
                            overlap && {
                                let mut q = crate::worldboarder::WorldBoarder::new(b, 1);
                                course.ray_cast_instance(&mut q, o as usize).0 > 0.0
                            }
                        }
                        _ => near(p),
                    };
                    let d = target - self.vel.dot(dir);
                    if touching && d > 0.0 { self.vel += dir * (d * gain * dt); }
                }
                // `Boarder_TeleportToInstance` 0x11a098 (`teleport::teleport_target`): the place by the rider's
                // slot, then `Boarder_PlaceAt` with no velocity
                PadKind::Teleport { places, heading } if near(p) => {
                    self.respawn_heading(places[self.slot.min(5) as usize], heading);
                    self.vel = Vec3::ZERO;
                    return true;
                }
                PadKind::Pickup { mult, speed, spin } if near(p) => {
                    if let Some(m) = mult { if self.multiplier < m as u32 { self.pick_multiplier(m as u32); } }
                    if let Some(s) = speed { if self.speed_timer < s - 0.5 { self.pick_speed(s); } }
                    if let Some(s) = spin { if self.spin_timer < s - 0.5 { self.pick_spin(s); } }
                }
                _ => {}
            }
        }
        false
    }
    /// Script op 0xf (`Pickup_Meter` 0x11e958): straight onto the meter, in show-off only.
    pub fn pick_boost(&mut self, amount: f32) { if self.showoff { self.add_meter(amount); } }
    /// Six landed uber tricks spell TRICKY: boost no longer runs out.
    pub fn tricky(&self) -> bool { crate::boarder::is_tricky(self.sc.x88) }
    /// Uber tricks can be started (the meter was filled in the last twenty seconds, or TRICKY).
    pub fn uber_ready(&self) -> bool { self.uber_timer > 0.0 || self.tricky() }
    /// Leaving a rail (`RailSlideMotion_Update` 0x10b0d0): the rail is scored as the chain's trick and a new chain
    /// begins (`Score_StartTrickChain`(stance, rail kind)).
    pub fn end_grind(&mut self) {
        if self.grind_time > 0.0 {
            let kind = self.rail_kind;
            self.begin_chain(kind, false);
            self.from_rail = true;
        }
        self.rail_halves = 0;
        self.grind_time = 0.0;
        self.rail_spun = 0.0;
        self.rail_side = 0.0;
    }

    /// A change of stance as the game's own routines make it (`ChangeFakieControl_Enter` 0x103eb8,
    /// `RailSlideControl_SwitchStance` 0x107fc8; `states.rs`): with the switch flag, the steer (+0x214) and the lean
    /// value (+0x244) change sign. The turn of the board itself stays the caller's (board row G20b2).
    pub(crate) fn set_switch(&mut self, switch: bool) {
        if switch != self.switch {
            self.switch = switch;
            self.steer = -self.steer;
            self.lean[0] = -self.lean[0];
        }
    }

    /// The board meets the snow (`Landing_ChooseState` 0x109308). The original only scales the speed for a crooked or tilted
    /// landing (edge grip then turns the board); a board coming down tail-first is turned round
    /// and the rider is switch.
    pub fn touch_down(&mut self) {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let wrap = |mut a: f32| { while a > PI { a -= TAU; } while a < -PI { a += TAU; } a };
        let hv = Vec3::new(self.vel.x, 0.0, self.vel.z);
        self.land_kind = 0;
        if hv.length() > 2.0 {
            let travel = f32::atan2(-hv.x, -hv.z);
            let mut d = wrap(self.yaw - travel);
            if d.abs() > FRAC_PI_2 {
                self.set_switch(!self.switch);
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
    /// Shoved over by another rider: `dv` the velocity change of the shove (m/s). A knock-down is scored as
    /// `Boarder_ScoreCrash` does (the trick banked; the meter only with `lose_meter`). Returns true if it put the
    /// rider down.
    pub fn take_hit(&mut self, dv: Vec3, lose_meter: bool) -> bool {
        let m = dv.length();
        if m <= 0.0 { return false; }
        let down = self.hit(m * 100.0, dv / m, true);
        if down {
            let (k, calls) = self.sc.crash(false);
            self.show(&calls);
            if lose_meter { self.add_meter(k); }
        }
        down
    }
    /// `Boarder_RiderHit` 0x124920, exact (`hit::rider_hit`, checked): a hit of `speed` (cm/s, signed) along the
    /// unit `dir` from a rider shoving (`shove`) or not. Over 733.97 cm/s it puts the rider down (unscored: the
    /// caller scores it), always on a rail, never in the air when the hit is upward; over 167.32 it is a stumble.
    /// STANDIN: the pair record the game keeps per pair of riders (where the wipe-out goes, the angle for the hop
    /// into the air) is not kept, so the hop never happens; the game mode is not knockdown practice; the push's
    /// virtual (+0x6c) is not called.
    pub fn hit(&mut self, speed: f32, dir: Vec3, shove: bool) -> bool {
        if self.crashed > 0.0 { return false; }
        let g = |v: Vec3, k: f32, w: f32| [v.x * k, v.z * k, v.y * k, w];
        let mut h = crate::hit::HitRider {
            heading: self.yaw,
            pos: g(self.pos, 100.0, 1.0),
            vel: g(self.vel, 100.0, 0.0),
            normal: g(self.normal, 1.0, 0.0),
            motion: if self.rail.is_some() { 3 } else if self.grounded { 2 } else { 1 },
            crouch: self.crouch_v,
        };
        let mut calls = Vec::new();
        let down = crate::hit::rider_hit(speed, g(dir, 1.0, 0.0), [0.0, self.yaw + std::f32::consts::PI], false, shove, &mut h, &mut calls);
        self.vel = Vec3::new(h.vel[0], h.vel[2], h.vel[1]) / 100.0;
        for c in calls {
            match c {
                crate::hit::HitCall::Stumble { .. } => self.stumble_by(true),
                crate::hit::HitCall::Wipeout { dir: d, .. } => {
                    let d = Vec3::new(d[0], d[2], d[1]);
                    self.crash_kind = if d.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 3 } else { 2 };
                    self.fall();
                }
                _ => {}
            }
        }
        down
    }
    /// `Boarder_ScoreKnockdown` 0x11e9a0: this rider put another down: a knock-down message and a full meter.
    pub fn knocked_someone(&mut self) {
        let call = self.sc.msg_knockdown();
        self.show(&[call]);
        self.add_meter(1.0);
    }
    /// The score functions' calls, for the HUD (as the rider shows its own).
    pub fn score_calls(&mut self, calls: &[ScoreCall]) {
        self.show(calls);
    }
    /// `Boarder_GetSpeedStat` 0x11d7b0: the rider's weight in a collision: weight × (0.79599 + 2.145779 × the
    /// stability byte / 255), × (1 + 8 × the boost level).
    pub fn speed_stat(&self) -> f32 {
        let m = stat_byte_w(self.stats.weight) as f32 * (self.stats.bytes[crate::stats::STABILITY] as f32 * 2.145_779 * 0.003_921_569 + 0.795_99);
        let lvl = if self.boosting { self.boost_level() } else { 0.0 };
        lvl * 7.999_995 * m + m
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
    pub fn jump_speed(&self, c: f32) -> f32 { self.stats.jump(c, self.vel.length()) }
    /// `Jump_ApplyImpulse` 0x1284e0, exact (`jump::jump_impulse`, checked): the jump's velocity from the crouch `c`,
    /// the jump stat, the speed, the ground normal, the board's forward and (off a rail) its up, at least 630.8786
    /// cm/s; a steep lip leans it along the lip. Returns the steep-lip flag `Score_StartTrickChain` gets.
    /// STANDIN: rider+0x208 (taken off the crouch) as 0, the stat byte from our 0..1 attribute.
    fn apply_jump(&mut self, c: f32, normal: Vec3, forward: Vec3, up: Vec3, rail: bool) -> bool {
        let g = |v: Vec3, k: f32| [v.x * k, v.z * k, v.y * k, 0.0];
        let s = crate::jump::JumpState {
            crouch: c,
            x208: 0.0,
            vel: g(self.vel, 100.0),
            jump_stat: self.stats.bytes[crate::stats::JUMP],
            motion: if rail { 3 } else { 0 },
            up: g(up, 1.0),
            normal: g(normal, 1.0),
            forward: g(forward, 1.0),
        };
        let (v, steep) = crate::jump::jump_impulse(f32::from_bits(0x441d_b83b), &s);
        if std::env::var("TRICKY_JUMPDBG").is_ok() { println!("    jump: c {c:.2} rail {rail} steep {steep} normal {normal:.2?} fwd {forward:.2?} vel {:.2?} -> {:.2?}", self.vel, Vec3::new(v[0], v[2], v[1]) / 100.0); }
        self.vel = Vec3::new(v[0], v[2], v[1]) / 100.0;
        steep
    }
    /// Leaving the snow (`SpinState_Enter`): the wind-up decides the rotation band for this jump.
    pub fn take_off(&mut self) {
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
        // the board's attitude is the game's own through the jump (row G20c3c): the quaternion rider+0x170 and the
        // axes +0x180..+0x1b0 are not touched in the air (the spin and flip are the separate angles +0x1d0 and
        // +0x1d4), so what the ground tick that took off left is what the landing's first tick starts from
        // (`seed_ground`). Taken from the game-side rider when the port has not moved the rider since; a take-off
        // with none (from the air, a rail) keeps what the rider already keeps.
        if let Some(c) = self.gm.take() {
            if c.is_at(self.pos, self.yaw) {
                self.att = Some((c.attitude(), self.yaw));
            }
        }
    }
    /// Crouching for a jump (`PrewindState_Update` 0x1050e8): the crouch and the wind-up follow their targets
    /// at the original's rates.
    pub fn prewind(&mut self, input: &Input, dt: f32) {
        // twice as quick on a rail (0x40800846 = 4.001 there)
        let rate = if self.rail.is_some() { 4.001 } else { 2.0005 } * crate::boarder::windup_factor(self.stats.bytes[crate::stats::WINDUP]);
        let slew = |cur: &mut f32, target: f32| { let step = (target - *cur).abs() * rate * dt; *cur += (target - *cur).clamp(-step, step); };
        slew(&mut self.wind, input.wind.unwrap_or(input.steer));
        slew(&mut self.wind_flip, input.flip);
    }
    /// Letting go of the jump button: tiny wind-ups count for nothing, and a wind-up that is
    /// nearly all spin or all flip is made pure.
    pub fn release_windup(&mut self) {
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

    /// One tick: on the snow `GroundMotion_Update` 0x10a0d8 with its forces (`Boarder_GroundSpringForce` 0x109878,
    /// `Boarder_GroundThrust` 0x109950, `Boarder_ForwardDrag` 0x109cb8, `Boarder_SideFriction` 0x109ef8), in the
    /// air `Air_IntegrateRK4` 0x12b340, on a rail `RailSlideMotion_Update` 0x10b0d0.
    pub fn step(&mut self, world: &CollisionWorld, rails: &Rails, input: Input, dt: f32) {
        // `Boarder_PlaceAt`'s ray under the last placement: on a hit the rider stands at the hit point, and the board
        // and its normal (+0x2a0, +0x2c0) are tipped to the hit's normal (else the port's drop from 1 m, flat)
        if let Some((at, heading)) = self.place_ray.take() {
            if let Some((p, n)) = ground_tick::placement_ray(world, at) {
                self.pos = ground_tick::port_dir(p) * 0.01;
                self.up = ground_tick::placed_normal(Some(n));
                self.normal = ground_tick::port_dir(self.up);
                self.att = Some((orient::placed_attitude(heading, Some(n)), self.yaw));
            }
        }
        // `cBoarder::vf7` 0x117198, before the tick's motion: the body's height eases toward the surface's (row G16)
        let motion = if self.crashed > 0.0 { 5 } else if self.rail.is_some() { 3 } else if self.grounded { 2 } else { 1 };
        self.body_height = crate::pose::ease_body_height(self.body_height, motion, crate::surface::SURFACE_TABLE[(self.surface as usize).min(19)][10]);
        // the lean record moves toward the target the last tick left (`cBoarder::vf7` 0x117198, before the motion),
        // and is aimed again from this tick's steer and speed at its end (`Boarder_UpdateLeanTargets` 0x129500)
        self.lean_ease(dt);
        self.step_tick(world, rails, input, dt);
        // the wipe-out runs these a game tick at a time itself
        if self.crashed <= 0.0 && impact_tick(&mut self.impact, &mut self.impact_n, &mut self.knocks, dt * 60.0) {
            let at = self.safe;
            self.respawn(at);
        }
        self.lean_aim();
    }
    fn step_tick(&mut self, world: &CollisionWorld, rails: &Rails, input: Input, dt: f32) {
        let mut input = input;
        self.score_ticks(dt);
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
        // the snow under the rider was handled by the ground tick (walls and probe included)
        let mut handled = false;
        let mut p = self.pos;
        if self.grounded {
            // ---- riding: the game's own ground tick, `GroundMotion_Update` 0x10a0d8 with its forces
            // (`Boarder_GroundSpringForce` 0x109878, `Boarder_GroundThrust` 0x109950, `Boarder_ForwardDrag` 0x109cb8,
            // `Boarder_SideFriction` 0x109ef8), the board's yaw, the walls and the ground probe: cm, z up, in
            // `ground_tick.rs` (row E8k)
            // the spring band follows the surface (SurfaceTable rows [7] [8]), easing at 1 m/s
            let band = SURF_BAND[(self.surface as usize).min(19)];
            self.band.0 += (band.0 - self.band.0).clamp(-dt, dt);
            self.band.1 += (band.1 - self.band.1).clamp(-dt, dt);
            let lvl = if sped { 1.0 } else if boosting_input { self.boost_level() } else { 0.0 };
            // the boost meter drains while boosting (the thrust itself is in ground_thrust)
            if boosting_input {
                if !self.tricky() && !sped { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); }
                self.boosting = true;
            }
            let left = self.ground_tick(world, dt, lvl);
            handled = true;
            p = self.pos;
            if left {
                // off the snow (`Boarder_TakeOff` ran inside the tick): the air from here
                let c = self.charge;
                self.take_off();
                self.charge = c;
            } else {
                let n = self.normal;
                let f = self.forward();
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
                        let steep = self.apply_jump(c, n, f, self.board_up(), false);
                        self.jump_c = 0.0;
                        self.take_off();
                        self.begin_chain(0, steep);
                        jumped = true;
                    }
                } else if self.jump_c > 0.0 {
                    // a jump released during a moment in the air (a bump) fires on touching down
                    let c = self.jump_c;
                    let steep = self.apply_jump(c, n, f, self.board_up(), false);
                    self.jump_c = 0.0;
                    self.take_off();
                    self.begin_chain(0, steep);
                    jumped = true;
                } else {
                    self.wind = 0.0;
                    self.wind_flip = 0.0;
                    self.charge = 0.0;
                }
                // the speed limit (vf7): up at once with a boost, back down slowly afterwards
                // the boosting cap goes by the meter left (vf7), the speed-boost pickup by its own timer
                let m = self.boost;
                let target = if sped || (self.boosting && (m > 0.66599226 || self.tricky())) { BOOST_CAPS[2] } else if self.boosting && m > 0.33362994 { BOOST_CAPS[1] } else if self.boosting && m > 0.0 { BOOST_CAPS[0] } else { SPEED_CAP };
                self.cap = target.max(self.cap - CAP_DECAY * dt);
                let s = self.vel.length();
                if s > self.cap { self.vel *= self.cap / s; }
            }
        } else {
            // ---- in the air (Air_IntegrateRK4 ballistics, SpinState_Update rotation)
            // the game's own step (checked bit for bit), in its cm and z up
            let (mut gp, mut gv) = ([p.x * 100.0, p.z * 100.0, p.y * 100.0, 1.0], [self.vel.x * 100.0, self.vel.z * 100.0, self.vel.y * 100.0, 0.0]);
            crate::air::integrate_rk4(dt, &mut gp, &mut gv, false);
            p = Vec3::new(gp[0], gp[2], gp[1]) / 100.0;
            self.vel = Vec3::new(gv[0], gv[2], gv[1]) / 100.0;
            self.air_time += dt;
            if !input.jump && self.air_time < 0.15 && (self.charge > 0.0 || self.jump_c > 0.0) {
                let c = if self.jump_c > 0.0 { self.jump_c } else { self.charge };
                if self.jump_c <= 0.0 { self.release_windup(); }
                let n = self.normal;
                let f = self.forward();
                let steep = self.apply_jump(c, n, f, self.board_up(), false);
                self.jump_c = 0.0;
                self.jump_delay = 0.0;
                self.take_off();
                self.begin_chain(0, steep);
                jumped = true;
            }
            self.air_rotation(&input, dt);
            // the landing alignment of a spin or flip is `SpinState_Update`'s (G20c3c2, G1a5b3b): until then only a jump
            // or a drop without either keeps the game's attitude (G20c3c, G20c3b)
            if self.spin != 0.0 || self.flip != 0.0 { self.att = None; }
            // `SpinState_Update` 0x100908: the spin (rider+0x1d0, mirrored for stats+0x40) and flip in degrees
            let m = if self.data.goofy { -1.0 } else { 1.0 };
            self.sc.update_spin((-(self.spin - self.spin_base) * m * 57.295776) as i32);
            self.sc.update_flip(((self.flip - self.flip_base) * 57.295776) as i32);
            self.air_grabs(&input, dt);
            if input.jump { self.charge = self.crouch_v; }
        }

        // The air tick's order is `AirMotion_Update`'s 0x108378 (row E8o, proven): the integration, then the down ray
        // (0x108554) that decides the landing, and only a rider not put down by it goes on to `Rail_TryCapture`
        // (0x1089f0), `Boarder_InstanceCollisions` (0x1089f8) and the wall ray (0x108a2c); a landed tick has no capture
        // and no air wall. So here: landing, then the rail, then the walls (below). The speed is the one going in.
        let s = self.vel.length();
        if handled {
            // the ground tick did the probe (and the walls)
        } else if !jumped {
            // landing: the game's down ray and probe on the course's collision (`Rider::course_landing`); without it
            // (a folder level) STANDIN: snow at or above our feet, and moving into it
            let found = if world.course.is_some() {
                self.course_landing(world, &mut p)
            } else {
                world.ground(p, 1.2, 0.02).filter(|h| self.vel.dot(h.normal) <= 0.5)
            };
            if let Some(hit) = found {
                let vn = self.vel.dot(hit.normal);
                {
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
                    self.land(None);
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

        // dropping onto a rail from the air locks the board onto it. Not a rider that was put down (`Rail_TryCapture`
        // 0x125cb8 is called by `AirMotion_Update` 0x1089f0 and `GroundMotion_Update` 0x10ae78 only, and
        // `Boarder_Wipeout` 0x11d830 has switched the motion to 5, `Boarder_SetMotion` 0x11f610 being immediate, whose
        // `WipeoutMotion_Update` 0x10c2f0 never captures, row E8m) and not one the landing probe put down: those never
        // get here. The air wall comes AFTER the capture (row E8o): a rider over a rail that hits a wall on the same
        // tick is captured (`Score_LandTrick` for the rail, `rail_kind` set) and then put down by the wall
        if !self.grounded && self.crashed <= 0.0 && !jumped && self.rail_cooldown <= 0.0 && self.air_time > 0.08 && self.vel.y < 2.5 && !self.capture_gated() {
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
                    // `Rail_TryCapture`: how the board sits on the rail (STANDIN: the angle's sign as ours)
                    let deg30 = 0.5235988;
                    self.rail_kind = if tw.abs() > 2.6179938 { 2 } else if tw < -deg30 { 4 } else if tw > deg30 { 3 } else { 1 };
                    // `RailSlideMotion_Enter` 0x10b068: the lean angle starts at zero
                    self.rail_lean = 0.0;
                    self.land(Some(self.rail_kind));
                    // `RailSlideControl_Enter`: the rail's spin starts from nothing
                    self.sc.update_spin(0);
                    self.rail_twist = tw;
                    self.rail_goal = (tw / std::f32::consts::FRAC_PI_2).round() * std::f32::consts::FRAC_PI_2;
                    self.rail_goal0 = self.rail_goal;
                    self.rail_committed = self.rail_goal;
                    self.rail_held = true;
                    self.rail_side_signed = 0.0;
                    self.set_switch(false);
                    self.rail_entry = along.abs();
                    self.rail_spun = 0.0;
                    self.rail_side = 0.0;
                    self.rail_off = 0.0;
                    self.rail_time = 0.0;
                    self.rail_halves = 0;
                }
            }
        }

        // the air's walls, last (`Boarder_InstanceCollisions` 0x1089f8, then the wall ray 0x108a2c with its glance or
        // `Boarder_Wipeout`): a sphere around the body, from where the capture left it. `AirMotion_Update` never calls
        // `Boarder_WallCollide` 0x126250 (its callers are the ground, rail, get-up and wipe-out motions): the bounce
        // branch of `walls` has no original in the air (row E8o, new gap). Not after a landing (grounded: the next tick's
        // ground tick has the walls) and not for a rider already down.
        if !handled && !self.grounded && self.crashed <= 0.0 {
            let mut q = self.pos;
            self.walls(world, &mut q);
            self.pos = q;
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

    /// Spin and flip in the air, as `SpinState_Update` 0x100908 does it. Holding the stick drives the
    /// rotation inside a band that narrows through the jump; letting go finishes it: forwards to
    /// the next half turn (spin) or full turn (flip), back only when within 40 degrees.
    pub fn air_rotation(&mut self, input: &Input, dt: f32) {
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

    /// Grabs, tweaks and uber tricks while in the air. The scoring is the game's (`SpinState_UpdateTrickClips`
    /// 0x1272e8): `Score_StartGrab` as a grab reaches its hold, the hold scored by `Score_Tick`, and
    /// `Scoring_OnTrickEvent` as it is let go (an uber's with its length in seconds).
    pub fn air_grabs(&mut self, input: &Input, dt: f32) {
        if self.air_time < 0.15 { return; }
        if self.uber > 0.0 && !self.uber_done {
            // an uber trick plays right through; it pays for its whole length when it ends
            self.uber += dt * 30.0;
            self.grab = 0;
            let frames = self.ubers.get(self.uber_id).map_or(60.0, |u| u.1);
            if self.uber >= frames - 1.0 {
                self.uber_done = true;
                self.ubers_done += 1;
                let clip = self.uber_clip();
                self.sc.end_grab(frames / 30.0, clip);
                self.grab_scored = false;
            }
        } else if let Some(i) = COMBOS.iter().position(|c| *c == input.grab & 15) {
            let rate = 30.0 * (0.78212035 + 0.8057834 * self.stats.tricks);
            if self.grab != i as u8 + 1 {
                self.release_grab();
                self.grab = i as u8 + 1;
                self.grab_hold = 0.0;
                self.grab_out = None;
            }
            let row = &self.data.rows[i];
            let (m1, m2, _, _) = grab_markers(row.clip);
            // the grab clip reaches in and holds at its second marker (grab clips play at
            // 0.782 + 0.806 x tricks of normal speed, Grab_Update); the grab counts from marker 1
            self.grab_hold = (self.grab_hold + rate * dt).min(m2);
            if self.grab_hold >= m1 {
                if !self.grab_scored {
                    self.grab_scored = true;
                    self.grab_tw = false;
                    self.grab_done[i] = true;
                    self.sc.start_grab(grab_clip(row.grab));
                }
                // the signature uber (bxUT_SIG...) starts from the grab the trick book gives it
                let sig = crate::trickdata::signature_grab(self.data.name) == Some(row.grab);
                // each board type has its own set (bxUT_, frUT_, exUT_ + style + grab): match on the grab
                let uber = self.ubers.iter().position(|u| (!row.uber_clip.is_empty() && uber_grab(&u.0) == uber_grab(row.uber_clip) && !u.0.contains("UT_SIG")) || (sig && u.0.contains("UT_SIG")));
                if input.tweak && self.uber_ready() && self.uber == 0.0 && uber.is_some() {
                    self.uber = 0.01;
                    self.uber_done = false;
                    self.uber_id = uber.unwrap();
                    // STANDIN: the uber clip replaces the grab, so its hold starts over at the uber's rate
                    let clip = self.uber_clip();
                    self.sc.start_grab(clip);
                } else if input.tweak && !self.grab_tw {
                    // STANDIN: a tweak keeps the grab going at the tweak clip's rate and takes its name
                    self.grab_tw = true;
                    self.grab_tweaked[i] = true;
                    if let Some(r) = crate::score::grab_rate(grab_clip(row.tweak)) { self.sc.grab_rate = r; }
                }
            }
        } else {
            // let go: the clip plays out from where it was; landing before it is past its
            // fourth marker is a crash
            if self.grab != 0 { self.grab_out = Some((self.grab, self.grab_hold)); }
            self.release_grab();
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
    /// The held grab let go (`Scoring_OnTrickEvent`(0, its clip)): into the jump's grab list.
    pub fn release_grab(&mut self) {
        if !self.grab_scored { return; }
        self.grab_scored = false;
        if self.grab == 0 { return; }
        let row = &self.data.rows[(self.grab as usize - 1).min(14)];
        let clip = grab_clip(if self.grab_tw { row.tweak } else { row.grab });
        self.sc.end_grab(0.0, clip);
    }
    /// The uber being done as the game's clip id (through its name in the trick-name table).
    fn uber_clip(&self) -> u32 {
        let clip = self.ubers.get(self.uber_id).map(|u| u.0.clone()).unwrap_or_default();
        let sig = crate::trickdata::signature_grab(self.data.name);
        let row = self.data.rows.iter().find(|r| (!r.uber_clip.is_empty() && !clip.contains("UT_SIG") && uber_grab(r.uber_clip) == uber_grab(&clip)) || (clip.contains("UT_SIG") && Some(r.grab) == sig));
        row.map_or(0, |r| grab_clip(crate::trickdata::uber_name(self.data.name, r.grab).unwrap_or(r.uber)))
    }
    /// The clip-type gate of `Rail_TryCapture` 0x125cb8 (row E8n): no capture while the layer-0 clip is of type 0xe, 0xf or
    /// 0x10 (descriptor table 0x325fa8: clip ids 0x278..=0x291, 0x292..=0x2ab and 0x2ac..=0x2db, the grab, tweak and uber
    /// clips) and clip flag 4 (`AnimCtl_TestClipFlag`, set by `AnimClip_Advance` 0x15a030 once the clip's time is past its
    /// fourth marker, row E8s) is not set. The port keeps no clip ids in the air, so this is its form of that test: the same one it makes for `Landing_CheckAngles` 0x12ba78 in [`Rider::land`]
    /// (a grab held or still playing out short of its fourth marker, an uber trick not yet done). The fall-recover clip
    /// 0x224 and the spin and flip clips are not in those ranges (0x224 is type 0): they never gate.
    pub fn capture_gated(&self) -> bool { !self.grab_safe() || (self.uber > 0.0 && !self.uber_done) }
    /// Is the board free to land (no grab clip short of its safe marker)?
    pub fn grab_safe(&self) -> bool {
        if self.grab != 0 { return false; }
        match self.grab_out {
            // flag 4 is set once the clip's time is past its fourth marker (`AnimClip_Advance` 0x15a030: marker < the larger of
            // the old and new time, row E8s): strictly past
            Some((g, f)) => f > grab_markers(self.data.rows[(g as usize - 1).min(14)].clip).2,
            None => true,
        }
    }

    /// Riding a rail (`RailSlideMotion_Update` 0x10b0d0 / `RailSlideControl_Update` 0x1073e0). The stick balances:
    /// for the first 0.6 s the board pulls itself to the middle of the rail, after that it drifts
    /// off one side unless you lean against it, and too far off you fall off (not a crash). Boost
    /// held with left / right turns the board a quarter turn (regular, sideways, fakie). There is
    /// no friction and no speed limit: a slow rider on an uphill rail rolls back.
    pub fn rail_step(&mut self, rails: &Rails, input: Input, dt: f32) {
        use std::f32::consts::FRAC_PI_2 as Q;
        let Some((ri, mut d, mut s)) = self.rail else { return };
        self.rail_time += dt;
        let rail = &rails.0[ri];
        let (_, tan) = rail.at(d);
        // `RailSlideMotion_Update` 0x10b0d0 reads the tangent where the board is now, and the balance offset before it moves
        let travel0 = tan * if s >= 0.0 { 1.0 } else { -1.0 };
        let board_up = ground_tick::game_dir(self.normal); // STANDIN for the board's up (+0x1a0) and, with `rail_lateral`, the closest point and the board's right (+0x190): rows G20c2c1, G20c2c3
        let lateral = ground_tick::rail_lateral(travel0, board_up, self.rail_off);
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
        // `RailSlideControl_CommitQuarterTurn` 0x107730: a quarter turn reached is the rail's spin (degrees)
        if (self.rail_goal - self.rail_twist).abs() < 1e-4 && self.rail_goal != self.rail_committed {
            self.rail_committed = self.rail_goal;
            self.sc.update_spin(((self.rail_goal - self.rail_goal0).to_degrees().round()) as i32);
        }
        self.rail_side_signed += self.rail_twist.sin() * dt;
        if self.rail_twist.sin().abs() > 0.7 { self.rail_side += dt; }

        self.pos = p + left * self.rail_off;
        self.vel = tan * s;
        self.yaw = f32::atan2(-travel.x, -travel.z) - self.rail_twist;
        // the normal and its lean angle as `RailSlideMotion_Update` 0x10b0d0 writes them (+0x2a0, object +8)
        self.rail_lean = ground_tick::rail_lean_angle(self.rail_lean, lateral, self.rail_kind, board_up[2]);
        self.normal = ground_tick::port_dir(ground_tick::rail_normal(ground_tick::game_dir(travel0), board_up, self.rail_lean));
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
                self.apply_jump(c, self.normal, self.forward(), up, true);
                self.release_windup();
            }
            if fell { self.vel += left * self.rail_off.signum() * 2.778; }
            // leaving fakie (or switch) gives the next air trick the switch bonus
            let fakie = self.rail_twist.cos() < -0.5;
            self.take_off();
            if fakie { self.switch_takeoff = true; self.set_switch(true); }
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
/// An angle wrapped to −π..π.
pub fn wrap(a: f32) -> f32 { (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI }
impl Rider {
    /// The gate clip's markers (normalised): leaning forward goes to m0, the push-out fires at m1.
    pub fn gate_markers(&self) -> (f32, f32) { if self.stats.kind == 2 { (0.4, 0.8) } else { (16.0 / 30.0, 20.0 / 30.0) } }
    pub fn gate_reset(&mut self) { self.gate_state = 8; self.gate_p = 0.0; self.gate_hi = 0.0; self.gate_lo = 0.0; self.gate_still = 0.0; self.gate_target = 0.0; self.gate_speed = 0.0; }
    /// The computer's rocking in the gate: turn round whenever it gets where it was going.
    pub fn gate_rock(&mut self) -> i32 {
        if self.gate_p == self.gate_target || self.gate_rock_dir == 0 { self.gate_rock_dir = if self.gate_p > 0.0 { -1 } else { 1 }; }
        self.gate_rock_dir
    }
    pub fn gate_k(&self) -> f32 { crate::gate::k(self.stats.bytes[crate::stats::START]) * 4.437673 }
    pub fn gate_move(&mut self, rate: f32, dt: f32) {
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
        self.gate_speed = s * crate::gate::k(self.stats.bytes[crate::stats::START]);
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
/// The trick's name as the original writes it (`Score_FormatTrickName` 0x1551c0): prefix, side, spin,
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


/// A new score object (`Score_Init` 0x1553a8: reset, then banked).
pub fn new_score() -> Score {
    let mut s = Score { rider_418: -1, ..Score::default() };
    s.reset();
    s.bank_points();
    s
}

/// The weight attribute (kg) as the game's weight byte (stats+0x1e). STANDIN: taken as the kilograms.
pub fn stat_byte_w(kg: f32) -> u8 {
    kg.round().clamp(0.0, 255.0) as u8
}

/// One of our 0..1 attributes as the game's stat byte. STANDIN: our stats are fractions of the game's bytes.
pub fn stat_byte(x: f32) -> u8 {
    (x * 255.0).round().clamp(0.0, 255.0) as u8
}

/// The game's clip id for a grab, tweak or uber by its name: its grab id in the trick-name table (0x320b58), then
/// the clip `Scoring_OnTrickEvent` maps to it. 0 if the name is not there.
pub fn grab_clip(name: &str) -> u32 {
    use tricky_data::trick_names as t;
    let want = format!("{name} ");
    let first = ((t::GRAB - t::BASE) / 4) as usize;
    let Some(id) = (0..0x66u32).find(|k| t::WORDS.get(first + *k as usize).copied().flatten() == Some(want.as_str())) else { return 0 };
    (0x278..0x2dc).find(|c| crate::score::grab_of_clip(*c).0 == id).unwrap_or(0)
}

/// `cBoarder::vf7` 0x117198 after the motion: +0x2f0 × 0.98; +0x2e4 × 0.95603746, over 4.492007 0 and
/// `Boarder_RequestReset` 0x118f10; +0x300 × 0.9782825, over 12.002146 the same. Returns whether a reset is asked.
/// `ticks` is the number of 60 Hz ticks (the factors raised to it: STANDIN for the port's variable step, exact at
/// one). Not ported: +0x304 = 1 with each reset, the reset's own checks (+0x418, `AIP_CanResetAt`), and the third
/// reset under the world (+0x148 under gWorld+0xc − 10000, the port's `min_y` check stands in).
pub fn impact_tick(impact: &mut f32, n: &mut [f32; 4], knocks: &mut f32, ticks: f32) -> bool {
    let one = ticks == 1.0;
    let f = |k: f32| if one { k } else { k.powf(ticks) };
    let shrink = f(f32::from_bits(0x3f7a_e148));
    for c in n.iter_mut() {
        *c *= shrink;
    }
    let mut reset = false;
    *impact *= f(0.956_037_46);
    if 4.492_007 < *impact {
        *impact = 0.0;
        reset = true;
    }
    *knocks *= f(0.978_282_5);
    if 12.002_146 < *knocks {
        *knocks = 0.0;
        reset = true;
    }
    reset
}

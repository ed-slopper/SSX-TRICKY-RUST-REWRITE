//! The boarder's per-tick update, whole and in the game's order (board row G1h): `cBoarder::vf7` 0x117198. It eases
//! the rider's now/rate/target records (the 12-byte records `Boarder_InitEaseRecords` 0x122f10 makes: brake, crouch,
//! steer, lean, ...), runs the motion (`Boarder_UpdateMotion` 0x11c770), caps the speed, asks the rider's controls
//! for the pad word and runs the control state (`Boarder_UpdateControl` 0x11c9a0), decays the impact records, scores
//! and picks the idle clip. Every call it makes goes through [`TickEnv`], each already ported on its own (the doc of
//! each method names where). Checked bit for bit in the function runner (`tests/boarder_tick.rs`).

use crate::jump::dot;

type V4 = [f32; 4];

/// The game's 1/60 s (0x3c888889).
const DT: f32 = f32::from_bits(0x3c88_8889);

/// The now/rate/target records eased each tick, in the game's order: the offset of each record's "now" (its rate at
/// +4, its target at +8). +0x1f0 the air's crouch-out, +0x1fc brake, +0x208 crouch, +0x214 steer, +0x244 lean,
/// +0x284 the fall-off blend; the others not named yet.
pub const EASES: [u32; 11] = [0x1f0, 0x1fc, 0x208, 0x214, 0x268, 0x220, 0x22c, 0x244, 0x250, 0x25c, 0x284];

/// What `cBoarder::vf7` reads and writes of the rider (offsets in the boarder).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickRider {
    /// +0x46b0: 0 skips the whole tick, the render's too
    pub active: u32,
    /// +0x18: the snow's weather at the rider, eased toward the game's density
    pub weather: f32,
    /// +0x1c: the boost meter (0..1)
    pub meter: f32,
    /// +0x12c: the time scale
    pub time_scale: f32,
    /// +0x130 the boost level, +0x134 the boost timer, +0x138 the spin boost time (both counted down here)
    pub boost_level: f32,
    pub boost_time: f32,
    pub spin_boost_time: f32,
    /// +0x148: the height of the rider's place (z up)
    pub pos_z: f32,
    /// +0x150: velocity
    pub vel: V4,
    /// +0x164: the distance travelled along the board's forward
    pub travelled: f32,
    /// +0x168: the board's pitch angle along its forward, eased 3° a tick
    pub pitch: f32,
    /// +0x180: the board's forward; +0x190: its right
    pub fwd: V4,
    pub right: V4,
    /// +0x1c4: the speed limit
    pub speed_limit: f32,
    /// the records of [`EASES`]: now, rate, target
    pub eases: [[f32; 3]; 11],
    /// +0x274, +0x278 targets; +0x27c, +0x280 eased toward them 5 per second (the quadrant blend)
    pub x274: f32,
    pub x278: f32,
    pub x27c: f32,
    pub x280: f32,
    /// +0x290: the surface under the rider (the row of the surface table)
    pub surface: u32,
    /// +0x294 / +0x298: eased toward the surface row's +0x1c / +0x20
    pub x294: f32,
    pub x298: f32,
    /// +0x2e4: the impact sum; +0x2f0: the crash normal; +0x300: the knocks; +0x304: the reset flag
    pub impact: f32,
    pub crash_normal: V4,
    pub knocks: f32,
    pub reset_flag: u32,
    /// +0x308: how high the body is drawn above the snow; +0x30c: eased to 1 on the snow, 0 off it
    pub height: f32,
    pub on_snow: f32,
    /// +0x384: last tick's event bits; +0x388: this tick's
    pub events_prev: u32,
    pub events: u32,
    /// +0x420: the board class; +0x424: the motion; +0x428: the control state
    pub class: u32,
    pub motion: u32,
    pub state: u32,
    /// +0x4a80: the board frame's up (+0x4a60's third row)
    pub board_up: V4,
}

/// What the tick reads of the game: the game object (`gApp`+0x730) and the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickGame {
    /// the game mode (game+0x1c → +0x1c): 4 the race line is followed, above 2 but not 5 the riders collide
    pub mode: i32,
    /// game+0x78: the snow's density
    pub density: f32,
    /// gWorld+0xc: the world's lowest height (10,000 below it resets the rider)
    pub world_floor: f32,
    /// the surface table's row for the rider's surface (game+0x24 + 100 · surface): +0x1c, +0x20, +0x28
    pub row_1c: f32,
    pub row_20: f32,
    pub row_28: f32,
}

/// The calls `cBoarder::vf7` makes, in its order. Each gets the rider so it can change what the tick reads after it.
pub trait TickEnv {
    /// `Boarder_GatherWorldEmitters` 0x1166f0 ([`crate::boarder_misc::gather_world_emitters`])
    fn gather_world_emitters(&mut self, r: &mut TickRider);
    /// `cGame::IsPaused` 0x17f078
    fn is_paused(&mut self) -> bool;
    /// `Math_Atan` 0x2510f8 ([`crate::math::atan`])
    fn atan(&mut self, x: f32) -> f32 {
        crate::math::atan(x)
    }
    /// `Boarder_UpdateMotion` 0x11c770 on the boarder object (+0x5ae0) ([`crate::boarder_misc::motion_update`])
    fn update_motion(&mut self, r: &mut TickRider);
    /// `Boarder_AddSpeedSample` 0x155dc8 on the score object (+0x5820) ([`crate::control::add_speed_sample`])
    fn add_speed_sample(&mut self, r: &mut TickRider, speed: f32);
    /// `Boarder_UpdateRaceLineProgress` 0x118298: the race line's progress, in the race-line mode (4)
    fn update_race_line(&mut self, r: &mut TickRider);
    /// `Boarder_UpdatePathAhead` 0x118090 ([`crate::aipath::update_path_ahead`])
    fn update_path_ahead(&mut self, r: &mut TickRider);
    /// the rider's virtual at vtable +0x54 (this adjusted by the short at +0x50): the pad word for this tick, built on
    /// the stack (16 bytes); a player's `cPlayer::GetControls`, a computer's `cComputer::GetControls` 0x1384b8
    fn get_controls(&mut self, r: &mut TickRider) -> [u32; 4];
    /// `Boarder_UpdateControl` 0x11c9a0 on the boarder object with the pad word ([`crate::control::update_control`])
    fn update_control(&mut self, r: &mut TickRider, pad: &[u32; 4]);
    /// `Boarder_RequestReset` 0x118f10 (f12 101.46) ([`crate::reset`])
    fn request_reset(&mut self, r: &mut TickRider, height: f32);
    /// `Score_Tick` 0x156e80 on the score object ([`crate::score`])
    fn score_tick(&mut self, r: &mut TickRider);
    /// `Boarder_PickIdleClip` 0x11d4e8 ([`crate::boarder_misc::pick_idle_clip`])
    fn pick_idle_clip(&mut self, r: &mut TickRider);
    /// `Boarder_RiderCollisions` 0x123fc8 ([`crate::ridercol`])
    fn rider_collisions(&mut self, r: &mut TickRider);
    /// `Rivals_UpdateGrudgeTimeout` 0x11efd8 ([`crate::rivals`])
    fn update_grudge_timeout(&mut self, r: &mut TickRider);
    /// `Boarder_UpdateLeanTargets` 0x129500 ([`crate::rider::ground_tick`])
    fn update_lean_targets(&mut self, r: &mut TickRider);
    /// `cBoarderRender::vf7` 0x1356f8 on the boarder object: the draw's per-tick update
    fn render_update(&mut self, r: &mut TickRider);
}

/// One eased step of `cBoarder::vf7`: `now` moves `step` toward `target`, or lands on it within a step.
pub fn ease(now: f32, target: f32, step: f32) -> f32 {
    if target + step < now {
        now - step
    } else if now < target - step {
        now + step
    } else {
        target
    }
}

/// The board's pitch along its forward as `cBoarder::vf7` aims +0x168: 0 for the board class 2 and the motion 4,
/// else the angle of (forward · up, right · up), `Math_Atan` of their ratio turned by π into the left half;
/// ±π/2 (by the sign) when the forward part is 0, and the right part itself when both are.
fn pitch_target(env: &mut dyn TickEnv, r: &TickRider) -> f32 {
    if r.class == 2 || r.motion == 4 {
        return 0.0;
    }
    let y = dot(r.board_up, r.right);
    let x = dot(r.board_up, r.fwd);
    if x == 0.0 {
        if y == 0.0 {
            y
        } else if 0.0 <= y {
            f32::from_bits(0x3fc9_0fdb)
        } else {
            f32::from_bits(0xbfc9_0fdb)
        }
    } else {
        let a = env.atan(y / x);
        let pi = f32::from_bits(0x4049_0fdb);
        if x < 0.0 {
            if 0.0 < y {
                a + pi
            } else {
                a - pi
            }
        } else {
            a
        }
    }
}

/// The speed limit `cBoarder::vf7` moves +0x1c4 to (cm/s): 3347.2222 while the boost timer runs; boosting, by the
/// meter: over 0.666 3347.2222, over 0.334 3072.1418, over 0 2932.43; else 2788.8394. A higher limit drops back
/// 3.472222 a tick.
pub fn speed_limit(r: &TickRider) -> f32 {
    let top = f32::from_bits(0x4551_338e);
    let target = if 0.0 < r.boost_time {
        top
    } else if 0.0 < r.boost_level {
        if f32::from_bits(0x3f2a_7e78) < r.meter {
            top
        } else if f32::from_bits(0x3eaa_d18b) < r.meter {
            f32::from_bits(0x4540_0245)
        } else if 0.0 < r.meter {
            f32::from_bits(0x4537_46e1)
        } else {
            f32::from_bits(0x452e_4d6e)
        }
    } else {
        f32::from_bits(0x452e_4d6e)
    };
    let down = r.speed_limit - f32::from_bits(0x405e_38e3);
    if target < down {
        down
    } else {
        target
    }
}

/// `cBoarder::vf7` 0x117198: the boarder's update, once a tick.
pub fn boarder_tick(env: &mut dyn TickEnv, g: &TickGame, r: &mut TickRider) {
    env.gather_world_emitters(r);
    if r.active == 0 {
        return;
    }
    if !env.is_paused() {
        before_motion(env, g, r);
        env.update_motion(r);
        after_motion(env, g, r);
    }
    env.render_update(r);
}

/// The part of `cBoarder::vf7` before the motion: the distance along the forward, the eased records, the body's
/// height, the pitch, the surface eases, the timers.
fn before_motion(env: &mut dyn TickEnv, g: &TickGame, r: &mut TickRider) {
    let along = dot(r.vel, r.fwd);
    r.travelled = r.travelled + along * (r.time_scale / 60.0);
    for e in r.eases.iter_mut() {
        e[0] = ease(e[0], e[2], e[1] * DT);
    }
    let c = f32::from_bits(0x3daa_aaab);
    r.x280 = ease(r.x280, r.x278, c);
    r.x27c = ease(r.x27c, r.x274, c);
    let body = f32::from_bits(0x3fd5_5555);
    r.height = ease(r.height, if r.motion == 2 { g.row_28 } else { 0.0 }, body);
    let pitch = pitch_target(env, r);
    r.pitch = ease(r.pitch, pitch, f32::from_bits(0x3d56_7750));
    r.x294 = ease(r.x294, g.row_1c, body);
    r.x298 = ease(r.x298, g.row_20, body);
    r.on_snow = ease(r.on_snow, if r.motion == 2 { 1.0 } else { 0.0 }, f32::from_bits(0x3ada_740e));
    for t in [&mut r.boost_time, &mut r.spin_boost_time] {
        *t -= DT;
        if *t <= 0.0 {
            *t = 0.0;
        }
    }
}

/// The part of `cBoarder::vf7` after the motion.
fn after_motion(env: &mut dyn TickEnv, g: &TickGame, r: &mut TickRider) {
    let mut speed = dot(r.vel, r.vel).sqrt();
    if r.motion != 1 {
        r.speed_limit = speed_limit(r);
        if r.speed_limit < speed {
            let k = r.speed_limit / speed;
            r.vel = r.vel.map(|v| v * k);
            speed = r.speed_limit;
        }
    }
    env.add_speed_sample(r, speed);
    if g.mode == 4 {
        r.events_prev = r.events;
        r.events = 0;
        env.update_race_line(r);
        env.update_path_ahead(r);
    }
    let density = if r.events & 0x4000 != 0 { 0.0 } else { g.density };
    r.weather = ease(r.weather, density, f32::from_bits(0x3c08_8889));
    let pad = env.get_controls(r);
    env.update_control(r, &pad);
    let k = f32::from_bits(0x3f7a_e148);
    r.crash_normal = r.crash_normal.map(|v| v * k);
    let reset = f32::from_bits(0x42ca_ebe0);
    r.impact *= f32::from_bits(0x3f74_bedf);
    if f32::from_bits(0x408f_be85) < r.impact {
        r.reset_flag = 1;
        env.request_reset(r, reset);
        r.impact = 0.0;
    }
    r.knocks *= f32::from_bits(0x3f7a_70b9);
    if f32::from_bits(0x4140_08ca) < r.knocks {
        r.reset_flag = 1;
        env.request_reset(r, reset);
        r.knocks = 0.0;
    }
    if r.pos_z < g.world_floor - 10000.0 {
        r.reset_flag = 1;
        env.request_reset(r, reset);
    }
    env.score_tick(r);
    if r.state != 0x16 {
        env.pick_idle_clip(r);
    }
    if 2 < g.mode && g.mode != 5 {
        env.rider_collisions(r);
    }
    env.update_grudge_timeout(r);
    env.update_lean_targets(r);
}

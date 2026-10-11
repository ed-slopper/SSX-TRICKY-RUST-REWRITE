//! One tick of riding on the snow exactly as the original runs it (board row E8h): `GroundMotion_Update` 0x10a0d8
//! with its callees. In cm and the game's z up, in the order of the machine code (the vector unit's sums, the
//! compare each branch makes, which value is read again from the rider and which is kept in a register).
//! `tests/ground_motion.rs` checks it bit for bit against the original in the function runner.
//!
//! What the function calls, and what this port does with each:
//! - run as ours, each checked bit for bit by its own test: `Boarder_GroundSpringForce` 0x109878,
//!   `Boarder_GroundThrust` 0x109950, `Boarder_ForwardDrag` 0x109cb8, `Boarder_SideFriction` 0x109ef8
//!   (`crate::ground`), `Boarder_ProbeGround` 0x128ae8 and `Boarder_GetFallLine` 0x128a18 (`crate::probe`),
//!   `Math_SinCos` 0x250c10, `Math_Asin` 0x250fd0 and `Math_Cos` 0x250e98 (`crate::math`), `Vec4_Scale` 0x102f50
//! - run as ours since E8j, checked in `tests/orient.rs`: `Boarder_RotateAboutAxis` 0x128e90 with the
//!   `Boarder_UpdateOrientation` 0x129008 it ends in (`crate::orient`, on the quaternion rider+0x170 and the
//!   board's forward, side, up and heading it derives)
//! - handed to [`Env`], because they are other rows' work: `Boarder_SetState` 0x11f608,
//!   `Boarder_WallCollide` 0x126250, the world's ray cast under the probe (`World_RayCast` 0x25aff8, E7),
//!   `Boarder_TakeOff` 0x128470 (ported in `crate::orient::take_off`; what it calls is still the port's to
//!   provide), the rider's virtual +0x64 (identified in E8j: it makes a burst of spray at the crash point and
//!   changes nothing else, see boarder.md; called with the position, velocity and normal when the surface is 6 or
//!   10), `Boarder_Wipeout` 0x11d830, `Score_Crash` 0x156de0, `Boarder_AddMeter` 0x11b018, rail capture (G1b, not ported yet)
//!   and `Boarder_InstanceCollisions` 0x125088. The test records each call, with its arguments, in the order they
//!   are made.

use crate::ground::{forward_drag, ground_thrust, side_friction, spring_force, DragState, FrictionState, ThrustState};
use crate::math;
use crate::orient::{rotate_about_axis, Orient};
use crate::probe::{self, add, cross, dot, scale, sub, Hit};

type V4 = [f32; 4];

/// A float the machine code loads with `lui`/`ori` (or from the constant pool), by its bits.
const fn k(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// 1/60, as the original writes it: `rider+0x12c` (the time scale) times this is the tick's dt in seconds.
const TICK: f32 = k(0x3c88_8889);
/// degrees to radians (0.017453292)
const DEG: f32 = k(0x3c8e_fa35);
/// the rider is "riding backwards" below this forward speed, cm/s (−111.11111)
const BACKWARDS: f32 = k(0xc2de_38e3);
/// How fast the board's bob (rider+0x1c0) dies away, 1/s (4.6840196)
const BOB_DECAY: f32 = k(0x4095_e37d);
/// The most the bob moves toward its target in a tick (1.6666666)
const BOB_STEP: f32 = k(0x3fd5_5555);
/// The sine's most, kept below 1 for `Math_Asin` (0.99999)
const NEAR_ONE: f32 = k(0x3f7f_ff58);
/// 6 degrees in radians: the most the board yaws toward the velocity in a tick (0.10471976)
const YAW_MAX: f32 = k(0x3dd6_7750);
/// The surface table's pointer-to-global load factor `DAT_0031d308`: `StaticInit_GroundMotion` 0x10af18 sets it
/// to `DAT_0031d304` / 30.0 = 30 / 30, so the spring force over gravity is `1 · force / g`.
const LOAD_SCALE: f32 = 30.0 / 30.0;
/// `DAT_0031d300`: what the game puts in rider+0x494 while `Boarder_WallCollide` runs (−1 afterwards).
pub const WALL_MASK: u32 = 3;

/// The stat bytes the ground forces read (each 0–255 as a float, as `crate::ground` takes them).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// stats+0x0e: speed
    pub speed: f32,
    /// stats+0x11: edging (the brake term)
    pub edging: f32,
    /// stats+0x13: edge grip
    pub grip: f32,
    /// stats+0x19: the cubic drag term
    pub cubic: f32,
    /// stats+0x40: the natural stance; rider+0x1b4 differing from it is riding switch
    pub stance: u32,
}

/// The rider as `GroundMotion_Update` reads and writes it (offsets from the rider object; vectors are four floats).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    /// what `Boarder_ProbeGround` works on: position +0x140, board forward +0x180, ground normal +0x2a0, ground
    /// point +0x2d0, forward in the ground's plane +0x320, side +0x330, fall line +0x2b0, the probe's offset
    /// (+0x244, stats+0x60), height over the ground +0x1bc, surface +0x290, ticks on it +0x29c, hit kind +0x2e0
    pub p: probe::Rider,
    /// velocity +0x150, cm/s
    pub vel: V4,
    /// the rider's up +0x2c0 (the bob is how far the position moved along it)
    pub up: V4,
    /// +0x1a0, the board's up (its z axis turned by the quaternion, proven in E8j: `Boarder_UpdateOrientation`); the
    /// board is turned from it toward the ground's normal
    pub board_up: V4,
    /// the board's attitude quaternion +0x170 (w, x, y, z) and its side +0x190 (the x axis turned by it): turned
    /// with `board_up`, the forward (`p.fwd`) and the heading `yaw` by `Boarder_RotateAboutAxis`
    pub quat: V4,
    pub board_side: V4,
    /// +0x12c time scale, +0x130 boost level (0 when not boosting), +0x134 speed-boost timer
    pub timescale: f32,
    pub boost_level: f32,
    pub speed_timer: f32,
    /// +0x160: the forward drag of this tick (written, not read here)
    pub drag: f32,
    /// the board's yaw +0x1b0 and the course's direction ahead +0x370, radians
    pub yaw: f32,
    pub course_yaw: f32,
    /// riding switch +0x1b4 (compared with `stats.stance`)
    pub switch: u32,
    /// +0x1c0: the vertical bob, eased toward the crouch's reach (cm)
    pub bob: f32,
    /// +0x1cc: the board's wobble (written)
    pub wobble: f32,
    /// +0x1d8: the lean angle, radians (written)
    pub lean: f32,
    /// brake +0x1fc, crouch +0x208, steer +0x214
    pub brake: f32,
    pub crouch: f32,
    pub steer: f32,
    /// the spring band +0x294, +0x298, cm
    pub d1: f32,
    pub d2: f32,
    /// board class +0x420 (0 BX, 1 freestyle, 2 alpine) and control state +0x428
    pub class: u32,
    pub state: u32,
    /// +0x494: [`WALL_MASK`] while the wall collision runs, −1 after
    pub mask: u32,
    pub stats: Stats,
    /// the playing clip is the skate push (clip 0x221, rider+0x46c0)
    pub skating: bool,
    /// the motion object's phase (cGroundMotion+8), radians, for the wobble
    pub phase: f32,
}

/// The record `Boarder_Wipeout` gets for a crash on surface 6 or 10 (the caller's stack, 0x50 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrashRecord {
    /// position, velocity direction (unit; twice in the record: +0x10 and +0x40), ground normal
    pub pos: V4,
    pub dir: V4,
    pub normal: V4,
    /// normal · velocity (only the first word of its slot is written)
    pub along: f32,
}

/// What `GroundMotion_Update` hands to other functions. Every method is one call of the original, at the point it
/// makes it; they may change the rider (the game's callees do).
pub trait Env {
    /// `Boarder_SetState` 0x11f608(rider, state)
    fn set_state(&mut self, r: &mut Rider, state: u32);
    /// `Boarder_WallCollide` 0x126250: true when it returns non-zero
    fn wall_collide(&mut self, r: &mut Rider) -> bool;
    /// the world's ray cast under `Boarder_ProbeGround` (`cWorldLine`(0.5, from, to, 2), `World_RayCast`): what it hit
    fn ray_cast(&mut self, r: &mut Rider, from: V4, to: V4) -> Option<Hit>;
    /// `Boarder_TakeOff` 0x128470 (`crate::orient::take_off`)
    fn take_off(&mut self, r: &mut Rider);
    /// the rider's virtual +0x64 (a spray burst, effects only), with the position, velocity and normal
    fn crash_virtual(&mut self, r: &mut Rider, pos: V4, vel: V4, normal: V4);
    /// `Boarder_Wipeout` 0x11d830(rider, record)
    fn wipeout(&mut self, r: &mut Rider, record: &CrashRecord);
    /// `Score_Crash` 0x156de0(rider+0x5820, 0): returns a float, which goes to `Boarder_AddMeter`
    fn score_crash(&mut self, r: &mut Rider) -> f32;
    /// `Boarder_AddMeter` 0x11b018(rider, amount)
    fn add_meter(&mut self, r: &mut Rider, amount: f32);
    /// rail capture (G1b, not ported yet)
    fn rail_try_capture(&mut self, r: &mut Rider);
    /// `Boarder_InstanceCollisions` 0x125088
    fn instance_collisions(&mut self, r: &mut Rider);
}

/// `Boarder_RotateAboutAxis` 0x128e90 on the rider's attitude fields (`crate::orient`).
fn rotate(r: &mut Rider, angle: f32, axis: V4) {
    let mut o = Orient { q: r.quat, fwd: r.p.fwd, side: r.board_side, up: r.board_up, yaw: r.yaw };
    rotate_about_axis(&mut o, angle, axis);
    r.quat = o.q;
    r.p.fwd = o.fwd;
    r.board_side = o.side;
    r.board_up = o.up;
    r.yaw = o.yaw;
}

/// `GroundMotion_Update` 0x10a0d8 on `r`, with the surface table `table` (`crate::surface::SURFACE_TABLE`: the
/// row of rider+0x290 as it was when the tick began is the one used throughout).
pub fn update(r: &mut Rider, table: &[[f32; 25]; 20], env: &mut dyn Env) {
    // speeds along the rider's normal, forward and side, before anything moves
    let vn = dot(r.vel, r.p.normal);
    let vf = dot(r.vel, r.p.plane_fwd);
    let vr = dot(r.vel, r.p.side);
    let row = &table[r.p.surface as usize];
    let dt = r.timescale * TICK;
    let steer = r.steer;

    // the four forces (`crate::ground`)
    let switch = r.switch != r.stats.stance;
    let mut spring = spring_force(r.p.height, vn, r.d1, r.d2, row[0], row[9]);
    let thrust = ground_thrust(
        &ThrustState {
            brake: r.brake,
            board_yaw: r.yaw,
            course_yaw: r.course_yaw,
            boost_level: r.boost_level,
            speed_timer: r.speed_timer,
            steer: r.steer,
            forward_up: r.p.plane_fwd[2],
            vel: r.vel,
            class: r.class,
            speed: r.stats.speed,
            skating: r.skating,
        },
        row[11],
        row[12],
    );
    let drag = forward_drag(
        vf,
        (LOAD_SCALE * spring) / row[0],
        [row[1], row[2], row[3]],
        &DragState {
            speed: r.stats.speed,
            edging: r.stats.edging,
            cubic: r.stats.cubic,
            class: r.class,
            switch,
            powder: (r.p.surface.wrapping_sub(3) < 2).then_some((r.p.height, r.d2)),
            crouch: r.crouch,
            boost_level: r.boost_level,
            brake: r.brake,
        },
    );
    let side = side_friction(
        vr,
        vf,
        steer,
        row[4],
        &FrictionState { grip: r.stats.grip, class: r.class, switch, boost_level: r.boost_level },
    );

    // no pull down the slope on a ceiling-facing normal
    if r.p.normal[2] < 0.0 && spring < 0.0 {
        spring = 0.0;
    }
    r.drag = drag;

    // the lean: steering tips the support force toward the side, the most it can carry is twice gravity
    r.lean = row[5] * DEG * r.steer;
    let (sin_l, cos_l) = math::sin_cos(r.lean);
    let twice_g = row[0] + row[0];
    let carried = if twice_g < spring { twice_g } else { spring };
    let m = carried / cos_l;
    let rest = spring - m;
    let w = add(scale(r.p.normal, cos_l), scale(r.p.side, sin_l));

    // riding backwards: the revert state
    if r.brake == 0.0 && vf < BACKWARDS && matches!(r.state, 3 | 5 | 0xc) {
        env.set_state(r, 4);
    }

    // the acceleration: the support force, the push along the board, the friction, gravity
    let a = add(scale(r.p.normal, rest), scale(w, m));
    let a = add(a, scale(r.p.plane_fwd, drag + thrust));
    let a = add(a, scale(r.p.side, side));
    let mut a = add(a, [0.0, 0.0, -row[0], 0.0]);

    // sunk deeper than the spring's band: pushed back out (at most 10 cm a tick), the speed into the snow removed
    let mut sunk = r.d2 + r.p.height;
    if sunk < -10.0 {
        sunk = -10.0;
    }
    if sunk < 0.0 {
        r.p.pos = sub(r.p.pos, scale(r.p.normal, sunk));
        r.p.height -= sunk;
        r.vel = sub(r.vel, scale(r.p.normal, vn));
        let an = dot(a, r.p.normal);
        if an < 0.0 {
            a = sub(a, scale(r.p.normal, an));
        }
    }

    // explicit Euler: position by the old velocity, then velocity by the acceleration
    let before = dot(r.p.pos, r.up);
    r.p.pos = add(r.p.pos, scale(r.vel, dt));
    r.vel = add(r.vel, scale(a, dt));
    let after = dot(r.p.pos, r.up);

    // the bob (rider+0x1c0): what the position lost along up, decaying, kept inside the crouch's reach and eased
    // to it (to 0 in the landing states 0xb and 0xc)
    r.bob = (r.bob + (before - after)) * (1.0 - dt * BOB_DECAY);
    let (low, high) = if r.class == 2 { (-10.0, 15.0) } else { (-20.0, 10.0) };
    let (hi, lo) = (r.crouch * high, (1.0 - r.crouch) * low);
    let mut target = if r.bob < lo {
        lo
    } else if hi < r.bob {
        hi
    } else {
        r.bob
    };
    if r.state.wrapping_sub(0xb) < 2 {
        target = 0.0;
    }
    let cur = r.bob;
    r.bob = if target + BOB_STEP < cur {
        cur - BOB_STEP
    } else if cur < target - BOB_STEP {
        cur + BOB_STEP
    } else {
        target
    };

    // the board yaws round after the velocity (not on the first tick of the prewind state 0xe with no steer)
    if r.state != 0xe || r.steer != 0.0 {
        let speed = dot(r.vel, r.vel).sqrt();
        let unit = scale(cross(r.vel, r.p.plane_fwd), 1.0 / speed);
        let mut s = dot(unit, r.p.normal);
        if s < -NEAR_ONE {
            s = -NEAR_ONE;
        } else if NEAR_ONE < s {
            s = NEAR_ONE;
        }
        let theta = math::asin(s);
        let (ac, b) = match r.class {
            2 => (k(0x3e55_56cf), k(0x3e86_406e)),
            0 => (k(0x3e99_b266), k(0x3eb2_c872)),
            _ => (k(0x3ecc_cccd), k(0x3f06_23b6)),
        };
        let forward = dot(r.vel, r.p.plane_fwd);
        let mut tgt = -theta + (((ac * 0.5) * (1.0 - steer * steer) + 1.0) * steer * b) / (r.crouch * k(0x3c23_d70a) + 1.0);
        if forward < 0.0 {
            tgt = -tgt;
        }
        let mut gain = speed * k(0x37a8_05c5) * speed * dt;
        if 1.0 < gain {
            gain = 1.0;
        }
        let along_fall = dot(r.vel, r.p.fall).abs() / speed;
        let fast = speed / k(0x440a_e38e);
        let fast = if 1.0 < fast { 1.0 } else { fast };
        let mut g = fast * (0.5 - along_fall) * 40.0;
        if g < 0.0 {
            g = 0.0;
        }
        g += k(0x3c24_8769);
        if 1.0 < g {
            g = 1.0;
        }
        let sel = if 0.0 < tgt { steer } else { -steer };
        let gain = if sel < g { gain * g } else { gain * sel };
        let mut turn = tgt * gain;
        if turn < -YAW_MAX {
            turn = -YAW_MAX;
        } else if YAW_MAX < turn {
            turn = YAW_MAX;
        }
        let n = r.p.normal;
        rotate(r, turn, n);
    }

    // the walls, then the ground under the new position
    r.mask = WALL_MASK;
    let wall = env.wall_collide(r);
    r.mask = 0xffff_ffff;
    let (from, to) = probe::line(&r.p);
    let hit = env.ray_cast(r, from, to);
    let on_ground = probe::probe(&mut r.p, hit);

    // off the powder (surfaces 3, 4) part of the speed into the ground is bled away, the speed kept
    if r.p.surface.wrapping_sub(3) >= 2 {
        let into = dot(r.vel, r.p.normal);
        let was = dot(r.vel, r.vel).sqrt();
        r.vel = sub(r.vel, scale(r.p.normal, into * k(0x3ecc_cccd)));
        let now = dot(r.vel, r.vel).sqrt();
        if now != 0.0 {
            r.vel = scale(r.vel, was / now);
        }
    }

    // airborne: no ground, or higher over it than the row's limit (the row of the tick's start)
    if !wall && (!on_ground || row[6] < r.p.height) {
        env.take_off(r);
        return;
    }

    if r.p.surface == 6 || r.p.surface == 10 {
        // the surface puts the rider down
        let pos = r.p.pos;
        let dir = scale(r.vel, 1.0 / dot(r.vel, r.vel).sqrt());
        let record = CrashRecord { pos, dir, normal: r.p.normal, along: dot(r.p.normal, r.vel) };
        let (vel, normal) = (r.vel, r.p.normal);
        env.crash_virtual(r, pos, vel, normal);
        env.wipeout(r, &record);
        let amount = env.score_crash(r);
        env.add_meter(r, amount);
        return;
    }

    // the board is turned a little toward the ground's normal (tilted up by half its slope's z)
    let tilted = add(r.p.normal, [0.0, 0.0, r.p.normal[2] * 0.5, 0.0]);
    let m = scale(tilted, 1.0 / dot(tilted, tilted).sqrt());
    let axis = cross(m, r.board_up);
    let len = dot(axis, axis).sqrt();
    if k(0x3727_c5ac) < len {
        let axis = scale(axis, 1.0 / len);
        let l = if NEAR_ONE < len { NEAR_ONE } else { len };
        let angle = -math::asin(l) * k(0x3e6e_f051);
        rotate(r, angle, axis);
    }

    // the wobble: the phase runs on with speed, steering and braking
    let speed = dot(r.vel, r.vel).sqrt();
    let amount = speed * k(0x39bc_be63);
    let busy = r.steer.abs() + r.brake.abs();
    let amount = if 1.0 < amount { 1.0 } else { amount };
    let amount = amount * (busy + k(0x3dcc_cccd));
    let phase = r.phase + amount * k(0x4029_999a);
    r.phase = phase;
    if k(0x40c9_0fdb) <= phase {
        r.phase = phase - k(0x40c9_0fdb);
    }
    r.wobble = amount * 7.5 * math::cos(r.phase);
    env.rail_try_capture(r);
    env.instance_collisions(r);
}

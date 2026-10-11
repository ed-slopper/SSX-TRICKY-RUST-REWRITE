//! Crossing the finish line exactly as the original does it (board row G1c1): the brake and crouch targets
//! (`Boarder_SetBrakeTarget` 0x126c78, `Boarder_SetBrakeCrouchInput` 0x126b38) and the finish state's four phases
//! (`FinishState_Update` 0x106668: slow down, stop, celebrate or be disappointed, wait for the clip). Checked bit for
//! bit against the original in the function runner (`tests/finish.rs`).

use crate::boarder::{self, Fields, SteerState};

/// The rider fields these read and write.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    /// brake now, its rate, its target (rider+0x1fc, +0x200, +0x204); crouch now, rate, target (+0x208..+0x210);
    /// steer now, rate, target (+0x214..+0x21c)
    pub brake: f32,
    pub brake_rate: f32,
    pub brake_target: f32,
    pub crouch: f32,
    pub crouch_rate: f32,
    pub crouch_target: f32,
    pub steer: f32,
    pub steer_rate: f32,
    pub steer_target: f32,
    /// boost level (+0x130), control state (+0x428), board class (+0x420), surface (+0x290)
    pub boost_level: f32,
    pub control: u32,
    pub class: u32,
    pub surface: u32,
    /// velocity (+0x150)
    pub vel: [f32; 4],
    /// place (+0x110), +0x418 (compared with the event's target in mode 7), +0x46d8 (the clip blend while the
    /// finish clip starts)
    pub place: i32,
    pub x418: i32,
    pub x46d8: f32,
    /// the celebrate / disappointed timers (+0x2c, +0x30: the race tick they started, −1 when off) and +0x34
    pub celebrate: i32,
    pub disappointed: i32,
    pub x34: i32,
}

/// `Boarder_SetBrakeTarget` 0x126c78: the brake target (+0x204) and rate (+0x200). Without boost (and not in control
/// state 6) the target is clamped to ±1; no brake while crouched; braking against the steer's direction flips it.
pub fn set_brake_target(b: f32, r: &mut Rider) {
    let b = if r.boost_level == 0.0 && r.control != 6 { b.clamp(-1.0, 1.0) } else { b };
    let t = r.steer_target;
    let mut target = b;
    if b != 0.0 && 0.0 < r.crouch {
        target = 0.0;
    }
    if (t < 0.0 && 0.0 < target) || (0.0 < t && target < 0.0) {
        target = -b;
    }
    let mut d = (target - r.brake).abs();
    if d < 0.1 {
        d = 0.1;
    }
    if target == 0.0 {
        d = 1.1 - d;
    }
    r.brake_target = target;
    r.brake_rate = d * 5.035_331_7;
}

fn crouch(r: &mut Rider, target: f32) {
    let mut f = Fields { x1fc: r.brake, crouch: r.crouch, crouch_rate: r.crouch_rate, crouch_target: r.crouch_target, ..Fields::default() };
    boarder::set_crouch_target(target, &mut f);
    r.crouch_rate = f.crouch_rate;
    r.crouch_target = f.crouch_target;
}

/// `Boarder_SetBrakeCrouchInput` 0x126b38(a, b): if `a` is positive while the brake is on (rider+0x1fc), only the
/// brake eases off (target 0, rate 5.0337); else if `b` is set while crouched (rider+0x208), only the crouch eases off
/// (target 0, rate 5.0025); otherwise the crouch target is set from `a` and the brake target from `b` (the machine
/// code passes them that way round: $f12 to `Boarder_SetCrouchTarget`, $f13 to `Boarder_SetBrakeTarget`).
pub fn set_brake_crouch_input(a: f32, b: f32, r: &mut Rider) {
    if 0.0 < a && r.brake != 0.0 {
        r.brake_target = 0.0;
        r.brake_rate = f32::from_bits(0x40a1_2170);
        return;
    }
    if b != 0.0 && 0.0 < r.crouch {
        r.crouch_target = 0.0;
        r.crouch_rate = f32::from_bits(0x40a0_1462);
        return;
    }
    crouch(r, a);
    set_brake_target(b, r);
}

fn steer_to_zero(r: &mut Rider) {
    let s = SteerState { boost_level: r.boost_level, class: r.class, vel: r.vel, steer: r.steer, surface: r.surface };
    let (t, rate) = boarder::set_steer_target(0.0, &s);
    r.steer_target = t;
    r.steer_rate = rate;
}

/// The finish state object (state+8 phase, state+0xc ticks in it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub phase: u32,
    pub ticks: i32,
}

/// What the race tells the finish state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Race {
    /// the game mode (0x32de8c), the number of riders (0x32db6c), the event's target for mode 7 (0x32de98)
    pub mode: u32,
    pub riders: i32,
    pub target: i32,
    /// in circuit modes (3, 5): `Circuit_GetMedal` of the rider's result (3 = none)
    pub medal: i64,
    /// mode 6: gApp+0x730 → +0x34 → +100
    pub x34_flag: u32,
    /// the race tick now (race+0x18), `AnimCtl_IsClipAtEnd` on layer 0, `AnimCtl_GetClipType` on layer 1
    pub tick: i32,
    pub clip_at_end: bool,
    pub clip_type1: i64,
}

/// The calls the finish state makes, besides the field writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Clip(u32),
    /// 0x128150 and 0x127e80 on the rider (the finish pose), then `Boarder_SetMotion`(4)
    Pose,
    Motion(u32),
}

/// `FinishState_Update` 0x106668, once a tick.
pub fn update(st: &mut State, r: &mut Rider, race: &Race) -> Vec<Call> {
    let mut calls = vec![];
    st.ticks = st.ticks.wrapping_add(1);
    let under = st.ticks < 301;
    match st.phase {
        0 => {
            set_brake_crouch_input(0.0, 1.0, r);
            if under && r.brake.abs() != 1.0 {
                let v = r.vel;
                let p = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
                let speed = (((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]).sqrt();
                if 277.777_77 <= speed {
                    return calls;
                }
            }
            steer_to_zero(r);
            st.phase = 1;
        }
        1 => {
            crouch(r, 0.0);
            steer_to_zero(r);
            set_brake_target(0.0, r);
            r.vel = r.vel.map(|x| x * f32::from_bits(0x3f6e_147b));
            if under && (r.brake != 0.0 || r.steer != 0.0) {
                return calls;
            }
            calls.push(Call::Clip(0x2ee));
            st.phase = 2;
            r.vel = [0.0; 4];
        }
        2 => {
            r.vel = [0.0; 4];
            if under && !race.clip_at_end {
                return calls;
            }
            let celebrate = match race.mode {
                3 | 5 => race.medal != 3,
                7 => r.place == 0 && (r.x418 as f32) < race.target as f32,
                6 => race.x34_flag != 0,
                _ => r.place < (race.riders + 1) / 2,
            };
            if celebrate {
                r.x34 = 0;
                r.celebrate = race.tick;
                r.disappointed = -1;
            } else {
                r.celebrate = -1;
                r.disappointed = race.tick;
            }
            calls.push(Call::Clip(0x2ef));
            r.x46d8 = 1.0;
            calls.push(Call::Pose);
            calls.push(Call::Motion(4));
            st.phase = 3;
        }
        3 if race.clip_type1 == 8 => st.phase = 4,
        _ => {}
    }
    calls
}

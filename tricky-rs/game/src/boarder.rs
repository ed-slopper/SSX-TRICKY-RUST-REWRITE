//! Boarder meter and input helpers exactly as the original computes them (board row G1d): the meter and uber
//! timer, the boost, the crouch and wind-up targets. Each function is one of the game's on the rider's own fields
//! (named by their offset), with its constants and order of operations; `tests/boarder.rs` checks each bit for bit
//! against the original in the function runner, including which of its sound and HUD calls it makes.

/// The rider fields these helpers read and write.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fields {
    /// +0x1c the meter (0..1), +0x24 the uber timer (seconds), +0x28 a counter of full meters, mod 4
    pub meter: f32,
    pub uber_timer: f32,
    pub full_count: i32,
    /// +0x130 the boost level while boosting (0 when not)
    pub boost_level: f32,
    /// +0x1fc, +0x208: crouch blockers and the crouch now; +0x20c, +0x210 the crouch rate and target
    pub x1fc: f32,
    pub crouch: f32,
    pub crouch_rate: f32,
    pub crouch_target: f32,
    /// +0x250..+0x264: the boost flame (scale +0x254, intensity +0x258, +0x25c..+0x264 its fade)
    pub x250: f32,
    pub x254: f32,
    pub x258: f32,
    pub x25c: f32,
    pub x260: f32,
    pub x264: f32,
}

/// The side calls a helper made, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Call {
    /// the rider's virtual +0x84 with the meter gained (`Boarder_AddMeter`)
    MeterGained,
    /// 0x2191c0: the meter has just filled
    MeterFull,
    /// 0x21dce0: the meter is full and more came in (the uber timer restarts)
    UberTimer,
    /// `Audio_TrickyTrigger` 0x21c610: "TRICKY"
    Tricky,
    /// `Sfx_UpdateBoost` 0x2192a8, 0x2197d8, 0x219760: boost sounds
    BoostSound,
    BoostEmpty,
    BoostStop,
}

/// `Boarder_AddMeter` 0x11b018: adds `units` (scaled by the meter stat, stats+0x1d) to the meter, clamped to
/// 0..1. A gain that reaches a full meter restarts the uber timer (20 s); one from an empty timer counts a full meter
/// and calls "TRICKY". Returns the calls made and the meter actually passed to the rider's virtual.
pub fn add_meter(units: f32, meter_stat: u8, f: &mut Fields) -> (f32, Vec<Call>) {
    let mut calls = vec![];
    let gain = units * (meter_stat as f32 * 0.359_662_23 * 0.003_921_569 + 0.978_775_8);
    calls.push(Call::MeterGained);
    let mut m = f.meter + gain;
    if f.meter < 0.85 && 1.0 <= m {
        calls.push(Call::MeterFull);
        m = f.meter + gain;
    }
    f.meter = m.clamp(0.0, 1.0);
    if f.meter == 1.0 && 0.0 < units {
        let old = f.uber_timer;
        f.uber_timer = 20.0;
        if old <= 0.0 {
            f.full_count = f.full_count.wrapping_add(1) % 4;
        }
        calls.push(Call::UberTimer);
        if old <= 0.0 {
            calls.push(Call::Tricky);
        }
    }
    (gain, calls)
}

/// `Boarder_SetCrouchTarget` 0x126c00: crouch toward `target` (no crouching while +0x1fc is set) at 5.002488 × the
/// distance per second, at least 0.5002488.
pub fn set_crouch_target(target: f32, f: &mut Fields) {
    let t = if 0.0 < target && f.x1fc != 0.0 { 0.0 } else { target };
    let d = (t - f.crouch).abs();
    let d = if d < 0.1 { 0.1 } else { d };
    f.crouch_target = t;
    f.crouch_rate = d * 5.002_488;
}

/// `PrewindState_SetWindTarget` 0x105a60 on a wind-up `[now, rate, target]` (rider+0x220 or +0x22c): the target
/// `v` (negated when `flip`), reached at `rate` × the distance, scaled by the wind-up stat (stats+0x18).
pub fn set_wind_target(v: f32, rate: f32, flip: bool, windup_stat: u8, wind: &mut [f32; 3]) {
    let v = if flip { -v } else { v };
    wind[2] = v;
    wind[1] = rate * (v - wind[0]).abs() * (windup_stat as f32 * 0.329_015_37 * 0.003_921_569 + 0.812_431_7);
}

/// `Boarder_UpdateBoost` 0x11d038, once a tick: `held` the boost button, `start` its second argument (the boost was
/// just pressed), `sounds` whether the race lets boost sounds play (no race object, or its state 1..9), `tricky`
/// `Boarder_IsTricky`. Boosting needs meter; it sets the boost level by the meter, drains 0.0007501507 a tick
/// (not in TRICKY) and drives the boost flame.
pub fn update_boost(held: bool, start: bool, sounds: bool, tricky: bool, f: &mut Fields) -> Vec<Call> {
    let mut calls = vec![];
    if !held || f.meter <= 0.0 {
        if sounds {
            if held && start {
                calls.push(Call::BoostEmpty);
            }
            if 0.0 < f.boost_level {
                calls.push(Call::BoostStop);
            }
        }
        f.boost_level = 0.0;
        f.x254 = f32::from_bits(0x3fc3_4fbc);
        f.x258 = 0.0;
        return calls;
    }
    let mut m = f.meter;
    if f.boost_level == 0.0 {
        if sounds {
            calls.push(Call::BoostSound);
            m = f.meter;
        }
        if start {
            f.x258 = f32::from_bits(if 0.665_992_26 < m {
                0x3f3f_fb12
            } else if 0.333_629_94 < m {
                0x3f0c_e13d
            } else {
                0x3f26_6b0c
            });
            f.x254 = f32::from_bits(0x4009_3b03);
            f.x264 = 1.0;
            f.x25c = 1.0;
            f.x260 = 0.0;
            m = f.meter;
        }
    }
    let flame = if 0.665_992_26 < m {
        f.boost_level = 1.0;
        0.749_924_8
    } else if 0.333_629_94 < m {
        f.boost_level = f32::from_bits(0x3f19_f24e);
        0.550_311_86
    } else {
        f.boost_level = f32::from_bits(0x3e80_03b9);
        0.650_070_9
    };
    if !tricky {
        f.meter -= 0.000_750_150_7;
    }
    if flame <= f.x250 {
        if f.x25c == 1.0 {
            f.x264 = 0.0;
            f.x260 = (0.0 - f.x25c).abs() / 0.064_303_32;
        } else if f.x25c <= 0.0 {
            f.x258 = 0.0;
            f.x254 = f32::from_bits(0x3fc3_4fbc);
        }
    }
    calls
}

/// What `Boarder_SetSteerTarget` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct SteerState {
    /// boost level (rider+0x130; boosting allows a full lock), board class (rider+0x420), velocity (rider+0x150),
    /// the steer now (rider+0x214) and the surface (rider+0x290; 3 and 4 are powder)
    pub boost_level: f32,
    pub class: u32,
    pub vel: [f32; 4],
    pub steer: f32,
    pub surface: u32,
}

/// `Boarder_SetSteerTarget` 0x126948: the steer target (to rider+0x21c) and the rate it is reached at (to rider+0x218).
/// Without boost the lock is ±0.9051856; below a class's full-lock speed (1138.27 cm/s alpine, 1134.97 BX, 1119.01
/// freestyle) it shrinks with speed; the rate is 7.0173593 × the distance, 0.1..8.018349, × 0.6 on powder.
pub fn set_steer_target(target: f32, s: &SteerState) -> (f32, f32) {
    let mut t = target;
    if s.boost_level == 0.0 {
        t = t.clamp(-0.905_185_6, 0.905_185_6);
    }
    let full = if s.class == 2 { 1_138.272_7 } else if s.class == 0 { 1_134.973_9 } else { 1_119.009_5 };
    let v = s.vel;
    let p = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let speed = (((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]).sqrt();
    let k = speed / full;
    if k < 1.0 {
        t *= k;
    }
    let mut rate = ((t - s.steer).abs() * 7.017_359_3).clamp(0.1, 8.018_349);
    if s.surface.wrapping_sub(3) < 2 {
        rate *= 0.599_972_6;
    }
    (t, rate)
}

/// `Boarder_IsTricky` 0x11f430: more than five ubers done this run (rider+0x58a8).
pub fn is_tricky(ubers: i32) -> bool {
    5 < ubers
}

/// The wind-up clips, by direction (centre, back, then the eight sectors) and by rider+0x1dc (0..4): the table the
/// game copies from 0x35fed0.
pub const WIND_CLIPS: [[u32; 5]; 9] = [
    [0x218, 0x218, 0x218, 0x261, 0x260],
    [0x220, 0x262, 0x262, 0x268, 0x26c],
    [0x21c, 0x21c, 0x21c, 0x268, 0x26c],
    [0x21a, 0x264, 0x264, 0x266, 0x26b],
    [0x21b, 0x21b, 0x21b, 0x269, 0x26d],
    [0x21e, 0x21e, 0x21e, 0x268, 0x26c],
    [0x219, 0x265, 0x265, 0x267, 0x26a],
    [0x21d, 0x21d, 0x21d, 0x269, 0x26d],
    [0x21f, 0x263, 0x263, 0x269, 0x26d],
];

/// `PrewindState_UpdateWindClip` 0x1055f0: the clip for the wind-up now (spin rider+0x220, flip rider+0x22c; both
/// negated riding switch), from `WIND_CLIPS` row by its direction and column `kind` (rider+0x1dc).
pub fn wind_clip(spin: f32, flip: f32, switch: bool, kind: usize) -> u32 {
    let (x, y) = if switch { (-spin, -flip) } else { (spin, flip) };
    let a = if x == 0.0 {
        if y == 0.0 { 0.0 } else if 0.0 <= y { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 }
    } else {
        let t = crate::math::atan(y / x);
        if x < 0.0 {
            if 0.0 < y { t + std::f32::consts::PI } else { t - std::f32::consts::PI }
        } else {
            t
        }
    };
    let row = if flip == 0.0 && spin == 0.0 {
        0
    } else if 2.748_893_5 < a.abs() {
        1
    } else if 1.963_495_4 < a {
        2
    } else if 1.178_097_2 < a {
        3
    } else if std::f32::consts::FRAC_PI_8 < a {
        4
    } else if a < -1.963_495_4 {
        5
    } else if a < -1.178_097_2 {
        6
    } else if a < -std::f32::consts::FRAC_PI_8 {
        7
    } else {
        8
    };
    WIND_CLIPS[row][kind]
}

/// The rider (and prewind state) fields `PrewindState_Update` reads and writes, around [`Fields`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Prewind {
    pub f: Fields,
    /// the prewind state's flag (state+8): the wind-up has started
    pub started: u32,
    /// rider+0x1dc (the kind of take-off: 0 snow; it picks the wind-up clip column and rate) and rider+0x418
    pub kind: u32,
    pub x418: i32,
    /// the spin and flip wind-ups, [now, rate, target] (rider+0x220.., rider+0x22c..)
    pub spin: [f32; 3],
    pub flip: [f32; 3],
    /// rider+0x1c8: the crouch kept for the jump
    pub x1c8: f32,
    /// velocity (rider+0x150), the board's forward in the air (rider+0x180)
    pub vel: [f32; 4],
    pub board: [f32; 4],
    /// the steer now, its rate and target (rider+0x214, +0x218, +0x21c)
    pub steer: f32,
    pub steer_rate: f32,
    pub steer_target: f32,
    pub surface: u32,
    pub switch: u32,
    pub class: u32,
    /// rider+0x304 (set with a reset request), rider+0x58a8 (ubers done)
    pub x304: u32,
    pub ubers: i32,
    /// the wind-up stat byte (stats+0x18)
    pub windup_stat: u8,
}

/// The side calls `PrewindState_Update` makes, in order (besides those of [`update_boost`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrewindCall {
    Boost(Call),
    /// `AnimCtl_PlayClip`(clip, 0, 5)
    Clip(u32),
    /// `Boarder_SetState`
    State(u32),
    /// `Boarder_RequestReset`(101.46)
    Reset,
}

fn sext6(word: u32, shift: u32) -> f32 {
    (((word << shift) as i32) >> 26) as f32 / 31.0
}

/// `PrewindState_Update` 0x1050e8, once a tick with the pad word `pad`: hold crouch to wind up (spin and flip from the
/// sticks, steer, boost), let go to jump (state 10, with the wind-up clip) or, from a standing start, ride on (state
/// 3); the select button or surface 0 asks for a reset. `sounds` as for [`update_boost`].
pub fn prewind_update(pad: u32, sounds: bool, p: &mut Prewind) -> Vec<PrewindCall> {
    let mut calls = vec![];
    let mut to_reset_check = false;
    let mut released = false;
    if p.kind == 0 {
        if pad & 4 == 0 && p.started == 0 && p.x418 < 0 {
            released = true;
        } else {
            p.spin[2] = 0.0;
            p.spin[1] = f32::from_bits(0x4000_0846);
            p.flip[2] = 0.0;
            p.flip[1] = f32::from_bits(0x4000_0846);
            p.started = 1;
            if p.spin[0] == p.flip[2] && p.flip[0] == p.flip[2] {
                calls.push(PrewindCall::Clip(0x1ff));
                calls.push(PrewindCall::State(3));
                return calls;
            }
            to_reset_check = true;
        }
    } else {
        released = true;
    }
    if released {
        if pad & 2 == 0 {
            // let go: jump with the wind-up there is, snapped to a pure spin or flip near an axis
            p.f.crouch_target = 0.0;
            p.f.crouch_rate = f32::from_bits(0x4155_5555);
            p.x1c8 = p.f.crouch;
            let mut s = p.spin[0].abs();
            let mut fl = p.flip[0].abs();
            if s < 0.2 {
                p.spin = [0.0, 0.0, 0.0];
                s = 0.0;
            }
            if fl < 0.2 {
                p.flip = [0.0, 0.0, 0.0];
                fl = 0.0;
            }
            if s != 0.0 || fl != 0.0 {
                let a = if s == 0.0 {
                    if 0.0 <= fl { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 }
                } else {
                    crate::math::atan(fl / s)
                };
                if 1.396_225_6 < a {
                    p.spin[2] = 0.0;
                    p.spin[0] = 0.0;
                    p.spin[1] = 0.0;
                } else if a < 0.174_570_72 {
                    p.flip[2] = 0.0;
                    p.flip[0] = 0.0;
                    p.flip[1] = 0.0;
                }
            }
            calls.push(PrewindCall::Clip(wind_clip(p.spin[0], p.flip[0], p.switch != 0, p.kind as usize)));
            calls.push(PrewindCall::State(10));
            return calls;
        }
        // crouched: boost, crouch, wind up from the sticks, steer
        let tricky = is_tricky(p.ubers);
        for c in update_boost((pad >> 3) & 1 != 0, false, sounds, tricky, &mut p.f) {
            calls.push(PrewindCall::Boost(c));
        }
        set_crouch_target(1.0, &mut p.f);
        let mut steer = sext6(pad, 9);
        if pad & 0x10 == 0 {
            let rate = f32::from_bits(if p.kind != 0 { 0x4080_0846 } else { 0x4000_0846 });
            set_wind_target(sext6(pad, 21), rate, false, p.windup_stat, &mut p.spin);
            set_wind_target(sext6(pad, 15), rate, p.switch ^ 1 != 0, p.windup_stat, &mut p.flip);
        } else {
            steer = -sext6(pad, 21);
        }
        let steer = steer.clamp(-0.5, 0.5);
        let (v, b) = (p.vel, p.board);
        let q = [v[0] * b[0], v[1] * b[1], v[2] * b[2], v[3] * b[3]];
        let along = ((q[0] + q[1]) + 1.0 * q[2]) + 1.0 * q[3];
        if 0.0 < along {
            let s = SteerState { boost_level: p.f.boost_level, class: p.class, vel: p.vel, steer: p.steer, surface: p.surface };
            let (t, rate) = set_steer_target(steer, &s);
            p.steer_target = t;
            p.steer_rate = rate;
        } else if steer != 0.0 {
            p.started = 1;
        }
        to_reset_check = true;
    }
    if to_reset_check && (pad & 1 != 0 || p.surface == 0) {
        p.x304 = (pad ^ 1) & 1;
        calls.push(PrewindCall::Reset);
    }
    calls
}

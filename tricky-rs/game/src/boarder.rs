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

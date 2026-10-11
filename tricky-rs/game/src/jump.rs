//! The jump exactly as the original computes it (`Jump_ApplyImpulse` 0x1284e0, board row G1a1): its constants,
//! its units (cm, cm/s, the game's z up) and the vector unit's order of operations, so it gives the same floats,
//! bit for bit, as the game's own code in the function runner (`tests/jump.rs`).

type V4 = [f32; 4];

/// `Math_Cos` and `Math_Sin` of the jump's fixed angles, as the game's own routines return them (read by running
/// `Math_Cos` 0x250e98 / `Math_Sin` 0x250d60 in the function runner): cos 50°, cos 70°, sin 20°.
const COS_50: f32 = f32::from_bits(0x3f24_8dbb);
const COS_70: f32 = f32::from_bits(0x3eaf_1d45);
const SIN_0: f32 = 0.0;
const SIN_20: f32 = f32::from_bits(0x3eaf_1d43);

/// A dot product as the vector unit forms it (vmul, then vaddabc / vmaddabc / vmaddbc): ((x + y) + z) + w.
pub(crate) fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

/// `Vec4_Scale` 0x102f50 and vmulbc: every lane times `k`.
pub(crate) fn scale(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

pub(crate) fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

pub(crate) fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// Normalised by vrsqrt (Q = 1 / sqrt(|a|²)) and vmulq.
pub(crate) fn normalize(a: V4) -> V4 {
    scale(a, 1.0 / dot(a, a).sqrt())
}

/// What `Jump_ApplyImpulse` reads from the rider.
#[derive(Clone, Copy, Debug, Default)]
pub struct JumpState {
    /// crouch (rider+0x1c8) and the amount taken off it (rider+0x208)
    pub crouch: f32,
    pub x208: f32,
    /// velocity, cm/s (rider+0x150)
    pub vel: V4,
    /// the jump stat byte (stats+0x17)
    pub jump_stat: u8,
    /// the motion (rider+0x424): 3 is the rail
    pub motion: u32,
    /// the board's up (rider+0x1a0), the ground normal (rider+0x2a0), the board's forward (rider+0x320)
    pub up: V4,
    pub normal: V4,
    pub forward: V4,
}

/// `Jump_ApplyImpulse` 0x1284e0: the rider's velocity after the jump, and whether it went off a steep lip (the
/// flag the game passes on to `Score_StartTrickChain`). `min` is the smallest jump (cm/s, the caller's $f12:
/// 630.8786 from `JumpControl_Update`).
pub fn jump_impulse(min: f32, s: &JumpState) -> (V4, bool) {
    let c = s.crouch - s.x208;
    let c = if c < 0.0 { 0.0 } else { c };
    let speed = dot(s.vel, s.vel).sqrt();
    let by_speed = if speed < 993.4801 { speed * 0.88093346 + 24.678448 } else { 899.86835 };
    let j = s.jump_stat as f32 * 0.78700274 * 0.003921569 + 0.09851468;
    // the machine code multiplies c·(c·J), then by the speed term (0x128584, 0x1285c4, 0x128600)
    let mut dv = c * (c * j) * by_speed;
    if dv < min {
        dv = min;
    }
    if s.motion == 3 {
        // off a rail: straight along the board's up
        return (add(s.vel, scale(s.up, dv)), false);
    }
    // how steep the lip is: 0 at 50° from flat, 1 at 70°
    let t = (s.normal[2] - COS_50) * (1.0 / (COS_70 - COS_50));
    if s.forward[2] < 0.0 || t < 0.0 || s.vel[2] < 0.0 {
        // the usual jump: along the normal tipped a fifth toward the board's forward
        let d = normalize(add(s.normal, scale(s.forward, 0.2)));
        return (add(s.vel, scale(d, dv)), false);
    }
    // a steep lip: the jump leans toward the board's forward along the lip, up to 0.9
    let h = normalize([s.normal[0], s.normal[1], 0.0, 0.0]);
    let along = normalize(sub(s.forward, scale(h, dot(s.forward, h))));
    let b = t * (along[2] - SIN_0) * (1.0 / (SIN_20 - SIN_0));
    let b = if 0.0 <= b { if 0.9 < b { 0.9 } else { b } } else { 0.0 };
    let a = 1.0 - b;
    let dir = normalize(add(scale(s.normal, a), scale(along, b)));
    let v = add(scale(s.vel, a), scale(along, b * speed));
    (add(scale(v, 0.5), scale(dir, dv)), true)
}

/// What `Takeoff_SetSpinRates` reads from the rider (after `Takeoff_Setup`, which it calls first).
#[derive(Clone, Copy, Debug, Default)]
pub struct TakeoffState {
    /// the clip playing on layer 0 (`AnimCtl_GetClip`): 0x218 and 0x260/0x261 take off with no spin
    pub clip: i64,
    /// rider+0x208
    pub x208: f32,
    /// the tricks stat byte (stats+0x1a)
    pub tricks_stat: u8,
    /// the spin and flip wind-ups (rider+0x220, rider+0x22c)
    pub wind_spin: f32,
    pub wind_flip: f32,
    /// board class (rider+0x420; 2 alpine) and switch/fakie (rider+0x1b4)
    pub class: u32,
    pub fakie: bool,
}

/// What `Takeoff_SetSpinRates` writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Takeoff {
    /// the air motion's spin and flip rates (motion+0x76c, motion+0x770)
    pub spin: f32,
    pub flip: f32,
    /// after clip 0x218 only: 1 − rider+0x208, written to rider+0x1f0 and rider+0x1f8 (rider+0x1f4 := 0)
    pub x1f0: Option<f32>,
    /// which clip routine runs: `Takeoff_PlayLaunchClip` 0x126fc0 (true) or `Boarder_PlayTakeoffClip` 0x127298
    pub launch_clip: bool,
}

/// `Takeoff_SetSpinRates` 0x126e30: the spin and flip the rider leaves the snow with, from the wind-ups and the
/// tricks stat (rad/s; a flip gets 2/3 of the spin's rate, an alpine board's spin 2/3, a flip switched riding fakie).
/// Then state 0x10 and the wind-ups are cleared (rider+0x220..0x234), in the game as here by the caller.
pub fn takeoff_spin_rates(s: &TakeoffState) -> Takeoff {
    if s.clip == 0x218 || s.clip == 0x260 || s.clip == 0x261 {
        let x1f0 = (s.clip == 0x218).then_some(1.0 - s.x208);
        return Takeoff { spin: 0.0, flip: 0.0, x1f0, launch_clip: false };
    }
    let k = (s.tricks_stat as f32 * 0.6741781 * 0.003921569 + 0.5449139) * 11.5171995;
    let mut spin = k * s.wind_spin;
    let mut flip = k * 0.6666667 * s.wind_flip;
    if s.class == 2 {
        spin *= 0.6666667;
    }
    if s.fakie {
        flip = -flip;
    }
    Takeoff { spin, flip, x1f0: None, launch_clip: true }
}

/// atan2 as the game's code writes it inline around `Math_Atan` (`crate::math::atan`): y over x, ±π/2 on the y
/// axis, 0 at the origin.
fn atan2_inline(y: f32, x: f32) -> f32 {
    if x == 0.0 {
        if y != 0.0 {
            if 0.0 <= y { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 }
        } else {
            y
        }
    } else {
        let t = crate::math::atan(y / x);
        if x < 0.0 {
            if 0.0 < y { t + std::f32::consts::PI } else { t - std::f32::consts::PI }
        } else {
            t
        }
    }
}

/// A two-bit direction field of the pad word: −1, 0 or 1 (the field's value 3 reads −1, 2 reads 0).
pub(crate) fn pad_dir(word: u32, shift: u32) -> f32 {
    match ((word << shift) as i32) >> 30 {
        -1 => -1.0,
        1 => 1.0,
        _ => 0.0,
    }
}

/// What `SpinState_StartFromStick` does when the stick (or d-pad) points somewhere.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpinStart {
    /// the spin and flip rates (spin object +0x2c, +0x30), rad/s
    pub spin: f32,
    pub flip: f32,
    /// the clip played for the direction (`AnimCtl_PlayClip`(clip, 0, 5)); `None` exactly on the −135° edge
    pub clip: Option<u32>,
}

/// `SpinState_StartFromStick` 0x102158: a spin started from the pad word `pad` (bit 0x400 picks which pair of
/// direction fields is read). `None` (the game returns 0) when no direction is held. Otherwise the game sets
/// spin+0x58 = 1, plays the clip, scores the start (`Boarder_AddMeter`) and goes to state 1 then 0x10.
/// `tricks` is the tricks stat byte (stats+0x1a), `switch` riding switch (rider+0x1b4).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the game's own comparisons, NaN behaviour included
pub fn spin_start_from_stick(pad: u32, tricks: u8, switch: bool) -> Option<SpinStart> {
    let (mut x, mut y) = if pad & 0x400 == 0 { (pad_dir(pad, 14), -pad_dir(pad, 12)) } else { (pad_dir(pad, 18), -pad_dir(pad, 16)) };
    // the game reads the second field as −1 → +1.0, 1 → −1.0, else −0.0
    if y == 0.0 {
        y = -0.0;
    }
    if x == 0.0 && y == 0.0 {
        return None;
    }
    // close to an axis it snaps onto it (within 10° of either)
    let a = atan2_inline(y.abs(), x.abs());
    if 1.396_225_6 < a {
        x = 0.0;
    } else if a < 0.174_570_72 {
        y = 0.0;
    }
    if switch {
        y = -y;
    }
    let k = tricks as f32 * 0.674_178_1 * 0.003_921_569 + 0.544_913_9;
    let mut flip = k * y * 5.253_080_4;
    if switch {
        flip = -flip;
    }
    let spin = k * x * 5.253_080_4;
    let dir = if switch { atan2_inline(-y, -x) } else { atan2_inline(y, x) };
    let a = dir;
    use std::f32::consts::{FRAC_PI_2 as P2, FRAC_PI_4 as P4, PI};
    let p34 = 2.356_194_5f32;
    let clip = if !(a <= p34) || a < -p34 {
        Some(0x22d)
    } else if !(a <= P2) && a < PI {
        Some(0x229)
    } else if !(a <= P4) && a < p34 {
        Some(0x227)
    } else if !(a <= 0.0) && a < P2 {
        Some(0x228)
    } else if !(a <= -P4) && a < P4 {
        Some(0x22c)
    } else if !(a <= -P2) && a < 0.0 {
        Some(0x22a)
    } else if !(a <= -p34) && a < -P4 {
        Some(0x226)
    } else if -PI < a && a < -P2 {
        Some(0x22b)
    } else {
        None
    };
    Some(SpinStart { spin, flip, clip })
}

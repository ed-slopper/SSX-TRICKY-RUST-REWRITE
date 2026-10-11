//! The landing check exactly as the original computes it (`Landing_CheckAngles` 0x12ba78, board row E8a1): in cm
//! and the game's z up, with the vector unit's order and the game's own arcsine and arctangent (`crate::math`),
//! checked bit for bit against the original in the function runner (`tests/landing.rs`).

use crate::math;

type V4 = [f32; 4];

fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn scale(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn normalize(a: V4) -> V4 {
    scale(a, 1.0 / dot(a, a).sqrt())
}

/// vopmula / vopmsub: a × b in x, y, z; w set to 0 (vsub.w of the result with itself).
fn cross(a: V4, b: V4) -> V4 {
    let acc = [a[1] * b[2], a[2] * b[0], a[0] * b[1]];
    [acc[0] - b[1] * a[2], acc[1] - b[2] * a[0], acc[2] - b[0] * a[1], 0.0]
}

/// What `Landing_CheckAngles` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct LandingState {
    /// the type of the clip on layer 0 (`AnimCtl_GetClipType`) and whether its flag 4 is set
    /// (`AnimCtl_TestClipFlag`, asked only for types 0xe, 0xf, 0x10: grab, tweak and uber clips)
    pub clip_type: i64,
    pub clip_flag4: bool,
    /// the surface under the board (rider+0x290)
    pub surface: u32,
    /// the board's forward axis as it flies (rider+0x180), velocity (rider+0x150), the board's up (rider+0x1a0)
    pub board: V4,
    pub vel: V4,
    pub up: V4,
    /// the stability stat byte (stats+0x16)
    pub stability: u8,
}

/// What it gives back, when it gets past its early outs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Landing {
    /// the new board forward and side on the landing surface (written to rider+0x320, rider+0x330)
    pub forward: V4,
    pub side: V4,
    /// the angles it reports (its third and fourth arguments): pitch, from the board to the travel direction
    /// (π/2 − asin), and roll about the normal
    pub pitch: f32,
    pub roll: f32,
    /// a clean landing: the roll, scaled by stability, within −2.967825..2.6840878
    pub ok: bool,
}

/// `Landing_CheckAngles` 0x12ba78 on the landing surface's normal `n`. `None` when it returns before touching
/// anything: a grab, tweak or uber clip not yet at its safe point, or surface 0, 6 or 10. Otherwise the rider goes
/// to state 1 (`Boarder_SetState`) and the board is laid on the surface.
pub fn check_angles(s: &LandingState, n: V4) -> Option<Landing> {
    if matches!(s.clip_type, 0xe..=0x10) && !s.clip_flag4 {
        return None;
    }
    if s.surface == 6 || s.surface == 10 || s.surface == 0 {
        return None;
    }
    let forward = normalize(sub(s.board, scale(n, dot(s.board, n))));
    let side = cross(n, forward);
    let vn = normalize(s.vel);
    let travel = normalize(sub(vn, scale(s.up, dot(vn, s.up))));
    let c = dot(s.board, travel);
    let c = c.clamp(-0.999_999_9, 0.999_999_9);
    let pitch = 1.570_796_4 - math::asin(c);
    let y = dot(s.up, n);
    let x = dot(s.board, n);
    let a = if y == 0.0 {
        if x != 0.0 {
            if 0.0 <= x { 1.570_796_4 } else { -1.570_796_4 }
        } else {
            x
        }
    } else {
        let t = math::atan(x / y);
        if y < 0.0 {
            if 0.0 < x { t + std::f32::consts::PI } else { t - std::f32::consts::PI }
        } else {
            t
        }
    };
    let roll = -a;
    let r = if 1.570_796_4 < pitch.abs() { a } else { -a };
    let r = r * (s.stability as f32 * -1.037_406_1 * 0.003_921_569 + 2.040_804);
    let ok = (-2.967_825..=2.684_087_8).contains(&r);
    Some(Landing { forward, side, pitch, roll, ok })
}

/// What `Landing_ChooseState` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChooseState {
    /// the clip type on layer 0 (`AnimCtl_GetClipType`): 0xe is a grab clip
    pub clip_type: i64,
    /// board class (rider+0x420; 2 alpine), riding switch (rider+0x1b4) and the rider's natural stance (stats+0x40)
    pub class: u32,
    pub switch: u32,
    pub stance: u32,
}

/// What it does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Choice {
    /// the landing clip played on layer 0 (`AnimCtl_PlayClip`(clip, 0, 5)) and the new state (0xc lands clean
    /// into riding, 0xb a hard landing)
    pub clip: u32,
    pub state: u32,
    /// out of a grab clip: the crouch and steer fields are reset (rider+0x1fc..0x21c, +0x268..0x270)
    pub grab_reset: bool,
    /// an alpine board landing backwards: the rider turns half round (π about the up axis, every clip layer −π,
    /// forward and side negated), riding switch flips, rider+0x46e0 gets the new heading offset
    pub turned: Option<(u32, f32)>,
}

/// `Landing_ChooseState` 0x109308 (after `Boarder_SetMotion`(2)): `yaw` and `pitch` from `Landing_CheckAngles`,
/// `vn` the speed into the snow (cm/s, negative going down).
pub fn choose_state(s: &ChooseState, yaw: f32, pitch: f32, vn: f32) -> Choice {
    let soft = -std::f32::consts::FRAC_PI_4 <= pitch;
    let firm = -2_777.105_2 <= vn;
    let pick = |clip, state| Choice { clip, state, grab_reset: false, turned: None };
    if s.clip_type == 0xe {
        return Choice { grab_reset: true, ..pick(0x1ff, 0xc) };
    }
    let forwards = yaw.abs() <= std::f32::consts::FRAC_PI_2;
    if s.class == 2 {
        if forwards {
            if s.switch == s.stance {
                if firm {
                    return if soft { pick(0x1ff, 0xc) } else { pick(0x242, 0xb) };
                }
                return pick(0x23e, 0xb);
            }
            return pick(0x240, 0xb);
        }
        let switch = s.switch ^ 1;
        let ang = if switch != 0 { std::f32::consts::PI } else { 0.0 };
        let t = ang * 0.159_154_94 + 0.5;
        let mut n = (t as i32) as f32;
        if t < n {
            n -= 1.0;
        }
        let turned = Some((switch, ang - n * 6.283_185_5));
        let clip = if switch == s.stance { 0x1ff } else { 0x240 };
        return Choice { turned, ..pick(clip, 0xb) };
    }
    if soft && firm {
        return pick(0x1ff, 0xc);
    }
    match (forwards, soft) {
        (true, true) => pick(0x23e, 0xb),
        (true, false) => pick(0x242, 0xb),
        (false, true) => pick(0x23f, 0xb),
        (false, false) => pick(0x243, 0xb),
    }
}

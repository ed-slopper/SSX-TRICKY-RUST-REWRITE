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

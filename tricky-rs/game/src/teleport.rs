//! `Boarder_TeleportToInstance` 0x11a098 (board row G20c3b3; script op 24): a rider put at an object, checked in the
//! function runner (`tests/teleport.rs`).
//!
//! The place is the object's matrix (rows: its axes scaled, then its position) applied to a side vector chosen by the
//! rider's +0x460 (0: (−1, 0), 1: (−1, 1), 2: (0, 1), 3: (1, 1), 4: (1, 0), any other none), times 300 cm, added to
//! the object's position; the heading is the angle of the object's second axis (its y row) in the x-y plane, the
//! game's inline atan2 around `Math_Atan`. Both as the vector unit forms them (multiply-adds row by row, x first).
//! Then `Boarder_PlaceAt` 0x11cae8 with no height, no velocity and flag 0, and for a player's rider (+0x38 ≥ 0)
//! 0x177db0 on that player's word (gApp+0x730's +0xa0 + 0x80·player).

use crate::jump::atan2_inline;

pub type V4 = [f32; 4];

/// The matrix applied to `v` as the vector unit does it: m0·x, + m1·y, + m2·z, then + m3·w
fn apply(m: &[V4; 4], v: V4) -> V4 {
    let mut r = [0.0f32; 4];
    for k in 0..4 {
        let acc = m[0][k] * v[0];
        let acc = acc + m[1][k] * v[1];
        let acc = acc + m[2][k] * v[2];
        r[k] = acc + m[3][k] * v[3];
    }
    r
}

/// The side vector by the rider's +0x460
pub fn side(slot: u32) -> V4 {
    match slot {
        0 => [-1.0, 0.0, 0.0, 0.0],
        1 => [-1.0, 1.0, 0.0, 0.0],
        2 => [0.0, 1.0, 0.0, 0.0],
        3 => [1.0, 1.0, 0.0, 0.0],
        4 => [1.0, 0.0, 0.0, 0.0],
        _ => [0.0; 4],
    }
}

/// The place (all four lanes, as `Boarder_PlaceAt` gets them) and the heading for the rider's +0x460 at the object's
/// matrix
pub fn teleport_target(m: &[V4; 4], slot: u32) -> (V4, f32) {
    let off = apply(m, side(slot));
    let off = [off[0] * 300.0, off[1] * 300.0, off[2] * 300.0, off[3] * 300.0];
    let place = [m[3][0] + off[0], m[3][1] + off[1], m[3][2] + off[2], m[3][3] + off[3]];
    let axis = apply(m, [0.0, 1.0, 0.0, 0.0]);
    (place, atan2_inline(axis[1], axis[0]))
}

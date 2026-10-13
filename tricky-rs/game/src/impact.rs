//! A rider's impact (board row G19): `cPlayer::vf11_1532c0` (a virtual of the player, args pos, vel, normal) hands
//! them on to `cBoarder::EmitBrakeSpray` 0x11a798 → `Boarder_EmitBrakeSpray` 0x136e70 → `BoarderRender_EmitBrakeSpray`
//! 0x135258, which are three register shuffles ending in `Fx_BrakeSpray_Emit` 0x12e008 (pool = the render object's
//! +0x99b0, the rider = word 0 of the object at boarder+0x5ae0, pos, vel, normal untouched; see
//! [`crate::brakespray::emit`]), and then to `Rumble_Impact` 0x150a80 (the pad's shake) unless a menu game mode
//! is up.

use crate::emitter::dot4;

/// What the game mode object (`*(gApp + 0x730) + 0x40`) says about the pad's shake: `None` when it has no such
/// object, else its +0x20 word.
pub type Mode = Option<i32>;

/// Whether the impact is muted: a mode object whose +0x20 is non-zero and under 10.
pub fn muted(mode: Mode) -> bool {
    matches!(mode, Some(x) if x != 0 && x < 10)
}

/// `Rumble_Impact` 0x150a80: the shake strength is `−vel·normal / 0.72` clamped to 0..2; over 0.3 and not muted
/// it starts the pad's shake of `clamp(1.6666666 · strength, 0.02, 0.1) · 60` ticks (answered); the rumble
/// object keeps the strongest (`peak`, its word 1).
pub fn rumble_impact(peak: &mut f32, vel: [f32; 4], normal: [f32; 4], mode: Mode) -> Option<i32> {
    let mut s = dot4(vel, normal) * f32::from_bits(0xbfb1_c71c);
    if s < 0.0 {
        s = 0.0;
    } else if 2.0 < s {
        s = 2.0;
    }
    let mut started = None;
    if f32::from_bits(0x3e99_999a) < s && !muted(mode) {
        let k = s * f32::from_bits(0x3fd5_5555);
        let mut t = f32::from_bits(0x3ca3_d70a);
        if !(k < t) {
            t = if f32::from_bits(0x3dcc_cccd) < k { f32::from_bits(0x3dcc_cccd) } else { k };
        }
        started = Some((t * 60.0) as i32);
    }
    if *peak < s {
        *peak = s;
    }
    started
}

/// `cPlayer::vf11_1532c0`'s own gate: the shake is skipped (after the spray) when muted.
pub fn player_rumbles(mode: Mode) -> bool {
    !muted(mode)
}

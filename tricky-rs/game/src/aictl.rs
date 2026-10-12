//! The computer rider's remaining control helpers (board row G4h): `AIComputer_ControlsCountdown` 0x138628,
//! `AIComputer_ControlsTurnAround` 0x138ac0, `AIComputer_ControlsIdle` 0x138b60, `AIComputer_ControlsAir` 0x138bb8,
//! `AIComputer_BelowGapMin` 0x1399a8, `AIComputer_AboveGapMax` 0x139a30, `AIComputer_UpdateSkillScale` 0x13a220 and
//! `AIComputer_RandomTrick` 0x11d368, in the game's units and its order of operations, checked in the function
//! runner (`tests/aictl.rs`). The pad word's fields: bits 0–5 the stick's x (a signed six-bit number, ±31), bits
//! 5–10 the stick's y in the air, bits 8–13 the stick's y on the ground; bit 1 the jump, bit 3 the trick button.

use crate::math::cos;

/// The stick's six-bit field, `(value × 31) as int & 0x3f`.
fn stick(v: f32) -> u32 {
    ((v * 31.0) as i32 as u32) & 0x3f
}

/// `AIComputer_RandomTrick` 0x11d368: the grab word `table[kind][character][draw % 15]` of the table at 0x333098
/// (`table` its bytes from 0x333098: rows of 0x10, three characters' worth of 0xf0 for each of the 0x2d0 of a kind).
/// `character` is stats+0x44 (the stats pointer is the object's +0x464), `kind` stats+0x5c, `draw` a `Rand_Game` word (unsigned remainder).
pub fn random_trick(table: &[u8], character: u32, kind: u32, draw: u32) -> u32 {
    let at = (kind as usize) * 0x2d0 + (character as usize) * 0xf0 + ((draw % 15) as usize) * 0x10;
    u32::from_le_bytes(table[at..at + 4].try_into().unwrap())
}

/// The distance along the heading to the pair record `rec` ([0] the distance, [1] the angle) seen from a rider with
/// heading `heading` (rider+0x370): `rec[0] × cos(rec[1] − heading + π)`, as both gap tests form it.
fn gap(rec: [f32; 2], heading: f32) -> f32 {
    rec[0] * cos((rec[1] - heading) + f32::from_bits(0x4049_0fdb))
}

/// `AIComputer_BelowGapMin` 0x1399a8: with a rider ahead (rider+0x100 not negative, `rec` its pair record), whether
/// the gap is under the rider's minimum (rider+0x104).
pub fn below_gap_min(ahead: Option<[f32; 2]>, heading: f32, min: f32) -> bool {
    ahead.is_some_and(|rec| gap(rec, heading) < min)
}

/// `AIComputer_AboveGapMax` 0x139a30: whether the gap is over the rider's maximum (rider+0x108).
pub fn above_gap_max(ahead: Option<[f32; 2]>, heading: f32, max: f32) -> bool {
    ahead.is_some_and(|rec| max < gap(rec, heading))
}

/// `AIComputer_UpdateSkillScale` 0x13a220: once a second (race tick `race+0x18` a multiple of 60, signed remainder)
/// the skill scale (object +0x86c) becomes the slow one (+0x870) with no rider ahead or the rider ahead ranked lower
/// (word +0x110, signed) than this one, else the fast one (+0x874).
pub fn update_skill_scale(tick: i32, scale: &mut f32, slow: f32, fast: f32, ahead_rank: Option<i32>, own_rank: i32) {
    if tick % 60 != 0 {
        return;
    }
    *scale = match ahead_rank {
        None => slow,
        Some(a) if a < own_rank => slow,
        Some(_) => fast,
    };
}

/// `AIComputer_ControlsIdle` 0x138b60: the pad word loses its stick and button bits (all but 0x1f81's complement:
/// `& 0xffffe07f & 0xffffff81`), then bit 0 is `AIComputer_CheckStuck`'s answer.
pub fn controls_idle(pad: &mut u32, stuck: bool) {
    *pad &= 0xffff_ff81 & 0xffff_e07f;
    *pad = (*pad & !1) | stuck as u32;
}

/// What `AIComputer_ControlsTurnAround` 0x138ac0 sets on the computer object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnAround {
    /// +0x884 = 1, +0x87c = 0, +0x880 = 0
    pub flag884: u32,
    pub word87c: u32,
    pub word880: u32,
    /// +0x878 = 0x1a once the rider has slowed under 0.5 (rider+0x57b8)
    pub word878: Option<u32>,
}

/// `AIComputer_ControlsTurnAround`: `steer` is what `AIComputer_SteerTowards`(the rider's line point +0x360) answers;
/// the stick's y is its negative.
pub fn controls_turn_around(pad: &mut u32, slow: f32, steer: impl FnOnce() -> f32) -> TurnAround {
    let mut t = TurnAround { flag884: 1, ..Default::default() };
    if slow < 0.5 {
        let s = steer();
        *pad = (*pad & 0xffff_c0ff) | (stick(-s) << 8);
        t.word878 = Some(0x1a);
    }
    t
}

/// `AIComputer_ControlsCountdown` 0x138628: the stick's x during the countdown. `skill` is +0x86c, `time` the
/// race's +0x14 (negative while counting down), `lean` and `lean2` rider+0x238 / +0x240, `side` the object's +0x868
/// (the side it leans, kept). Under 30 s (scaled by skill / 3.2585914) 31; under 60 s −31; else ±31 by the side.
pub fn controls_countdown(pad: &mut u32, skill: f32, time: f32, lean: f32, lean2: f32, side: &mut f32) {
    let t = -time;
    let scaled = |k: f32| if skill == 1.0 { k } else { (skill / f32::from_bits(0x4050_8cc3)) * k };
    if t < scaled(30.0) {
        *pad = (*pad & 0xffff_ffc0) | 0x1f;
        return;
    }
    if t < scaled(60.0) {
        *pad = (*pad & 0xffff_ffc0) | 0x21;
        return;
    }
    let s = if lean == lean2 || *side == 0.0 {
        let s = if 0.0 < lean { -1.0 } else { 1.0 };
        *side = s;
        s
    } else {
        *side
    };
    *pad = (*pad & 0xffff_ffc0) | stick(s);
}

/// What `AIComputer_FindJumpMarker` 0x1392a8 answers `AIComputer_ControlsAir`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JumpMarker {
    pub found: bool,
    pub kind: u32,
    pub a: u32,
    pub b: u32,
}

/// What `AIComputer_ControlsAir` 0x138bb8 asks for besides the pad word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirCall {
    /// `AIComputer_PlanJumpTrick`(object, a, b)
    Plan(u32, u32),
}

/// `AIComputer_ControlsAir`: a marker found ahead (and its first word not 0) sets the jump bit (2) and plans the
/// trick; else the stick's y (bits 5–10) is the rider's tilt (rider+0x198, negated and doubled, clamped to ±1)
/// when that is past 0.1; or, a draw `rand_game() % 100 − 16` below the stat (stats+0x1d) at a time, the trick button
/// (8) and the stick's y ±31 by the side kept at +0x888 (the first tilt seen).
pub fn controls_air(pad: &mut u32, marker: JumpMarker, tilt: f32, stat: u32, kept: &mut f32, rand_game: &mut dyn FnMut() -> u32, calls: &mut Vec<AirCall>) {
    if marker.found && marker.kind != 0 {
        *pad |= 2;
        calls.push(AirCall::Plan(marker.a, marker.b));
        return;
    }
    let mut s = -tilt;
    s += s;
    let s = if s < -1.0 { -1.0 } else if 1.0 < s { 1.0 } else { s };
    let y = if 0.1 < s.abs() {
        s
    } else {
        let draw = (rand_game() % 100) as i32 - 0x10;
        if !(draw < stat as i32) {
            return;
        }
        *pad |= 8;
        if *kept == 0.0 {
            *kept = s;
        }
        if *kept < 0.0 { 1.0 } else { -1.0 }
    };
    *pad = (*pad & 0xffff_f81f) | (stick(y) << 5);
}

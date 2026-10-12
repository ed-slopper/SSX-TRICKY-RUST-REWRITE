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

/// `AI_InitSkill` 0x13a2b0: the computer's two skill scales, `(slow, fast)` for object +0x870 / +0x874, from the
/// race mode (0x32de8c) and, for modes 4 and 2, the race word's low half (0x32de9c) or word 0x32dea4. Both end
/// clamped to 1.0 (the unreachable jump table in the original multiplies by 1.0).
pub fn init_skill(mode: u32, race_word: u32, word_a4: u32) -> (f32, f32) {
    let b = f32::from_bits;
    let (slow, fast) = if mode == 4 {
        match race_word & 0xffff {
            0 => (b(0x3f41_3e09), b(0x3ecf_43c5)),
            1 => (b(0x3f68_c259), b(0x3f35_59da)),
            _ => (1.0, b(0x3f73_88f2)),
        }
    } else if mode == 2 {
        match word_a4 {
            0 => (b(0x3f1b_5f37), b(0x3e8c_5b95)),
            1 => (b(0x3f5f_50b7), b(0x3f28_1a07)),
            _ => (1.0, b(0x3f72_f782)),
        }
    } else {
        (1.0, b(0x3f02_3d75))
    };
    (if 1.0 < slow { 1.0 } else { slow }, if 1.0 < fast { 1.0 } else { fast })
}

/// The object fields `AIComputer_Reset` 0x1383b0 sets (everything else it leaves).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResetFields {
    /// +0x87c, +0x880, +0x884, +0x888, +0x88c, +0x868, +0x894, +0x898 all 0
    pub zeroed: bool,
    /// +0x878 = 0x1a, +0x890 = -1, +0x89c (short) = 1, rider+0x41c = 0
    pub word878: u32,
    pub word890: i32,
    pub short89c: u16,
    /// +0x870 / +0x874 from `init_skill`, +0x86c the slow one
    pub slow: f32,
    pub fast: f32,
    pub skill: f32,
}

pub fn reset(mode: u32, race_word: u32, word_a4: u32) -> ResetFields {
    let (slow, fast) = init_skill(mode, race_word, word_a4);
    ResetFields { zeroed: true, word878: 0x1a, word890: -1, short89c: 1, slow, fast, skill: slow }
}

/// What `AIComputer_CheckStuck` 0x139ab8 reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StuckIn {
    /// rider +0x370 and +0x1b0, the heading and the path's
    pub heading: f32,
    pub path: f32,
    /// rider +0x424, +0x428 (the state words), +0x348 (speed), +0x57fc
    pub state: u32,
    pub state2: u32,
    pub speed: f32,
    pub f57fc: f32,
    /// the race mode word 0x32de8c and the object's skill +0x86c
    pub mode: u32,
    pub skill: f32,
    /// object +0x890 (signed), the counters +0x894 and +0x898 (kept)
    pub w890: i32,
    pub c894: u32,
    pub c898: u32,
}

/// `AIComputer_CheckStuck`: whether the computer rider is stuck (and should reverse/steer out). `below` and `above`
/// are `AIComputer_BelowGapMin` / `AboveGapMax`, called only where the original calls them.
pub fn check_stuck(s: &mut StuckIn, below: &mut dyn FnMut() -> bool, above: &mut dyn FnMut() -> bool) -> bool {
    let diff = s.heading - s.path;
    let f1 = diff * f32::from_bits(0x3e22_f983) + 0.5;
    let mut t = (f1 as i32) as f32;
    if f1 < t {
        t -= 1.0;
    }
    let wrapped = diff - t * f32::from_bits(0x40c9_0fdb);
    if s.w890 < 0 {
        s.c898 = if f32::from_bits(0x3f86_0a92) < wrapped.abs() { s.c898.wrapping_add(1) } else { 0 };
    } else {
        s.c898 = 0;
    }
    let mut long_stuck = s.c898 as i32 >= 0x3d;
    let bump = |c: &mut u32| *c = c.wrapping_add(1);
    'tally: {
        if s.state == 5 {
            long_stuck = false;
            bump(&mut s.c894);
            break 'tally;
        }
        if s.state == 3 && (s.c898 as i32) > 0 {
            return true;
        }
        let mut reset_a = false;
        if s.mode != 0 {
            if s.state == 1 || s.state == 6 || s.speed < 1000.0 {
                reset_a = true;
            } else {
                bump(&mut s.c894);
            }
        } else if s.speed < 1500.0 {
            s.c894 = 0;
            break 'tally;
        } else {
            bump(&mut s.c894);
        }
        if reset_a {
            long_stuck = false;
            s.c894 = 0;
            if s.state == 1 && 30.0 < s.f57fc {
                return true;
            }
        }
    }
    let mut late = long_stuck;
    if !late {
        let c = s.c894 as i32 as f32;
        late = if s.skill == 1.0 {
            222.0 < c
        } else {
            222.0 < c * (s.skill / f32::from_bits(0x41fb_4969))
        };
    }
    if !(0x13 <= s.state2 as i32 && (s.state2 as i32) < 0x15) {
        return late && s.w890 < 0;
    }
    if !late && !below() {
        return false;
    }
    !above()
}

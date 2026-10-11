//! The computer rider's target and steering helpers exactly (board row G4b): `AIComputer_SteerTowards` 0x1372f8,
//! `AIComputer_SetThrottle` 0x139f60, `AIComputer_SetTuck` 0x13a028, `AIComputer_IsRivalTarget` 0x139928 and
//! `AIComputer_PickTargetRider` 0x1396c8, in the game's units (cm, z up) and the vector unit's order of operations,
//! checked bit for bit in the function runner (`tests/aicomp.rs`). The computer object (cComputer) keeps its rider
//! (+0), its skill scale (+0x86c), its target rider (+0x890).

use crate::jump::{dot, normalize, scale, sub};

type V4 = [f32; 4];

/// The rider's frame as the AI reads it: position (+0x140), forward (+0x4aa0), side (+0x4ab0), up (+0x4ac0).
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub pos: V4,
    pub fwd: V4,
    pub side: V4,
    pub up: V4,
}

/// `AIComputer_SteerTowards` 0x1372f8: the steer (−0.9705659..0.9705659) toward `target`: its direction flattened
/// onto the rider's plane (normal = up), the angle −atan2(d·fwd, d·side) (the game's inline atan2 around
/// `Math_Atan`), × 6.2897024, × skill / 1.0241041 when the skill scale is not 1; under 0.2 nothing.
pub fn steer_towards(skill: f32, f: &Frame, target: V4) -> f32 {
    let d = normalize(sub(target, f.pos));
    let h = dot(d, f.up);
    let p = normalize(sub(d, scale(f.up, h)));
    let s = dot(p, f.side);
    let fw = dot(p, f.fwd);
    let angle = if s == 0.0 {
        if fw != 0.0 {
            if 0.0 <= fw { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 }
        } else {
            -fw
        }
    } else {
        let mut t = crate::math::atan(fw / s);
        if s < 0.0 {
            t = if 0.0 < fw { t + std::f32::consts::PI } else { t - std::f32::consts::PI };
        }
        -t
    };
    let mut v = angle.abs() * 6.289_702_4;
    if skill != 1.0 {
        v *= skill / 1.024_104_1;
    }
    if v < 0.2 {
        return 0.0;
    }
    if 0.970_565_9 < v {
        v = 0.970_565_9;
    }
    if angle < 0.0 {
        -v
    } else {
        v
    }
}

/// `AIComputer_SetThrottle` 0x139f60: the pad word's crouch stick (bits 11–16) full, or skill / 2.211699 when the
/// skill scale is not 1 and the rider is at 833.3333 cm/s or more.
pub fn set_throttle(skill: f32, vel: V4, word: u32) -> u32 {
    let mut k = 1.0;
    if skill != 1.0 {
        k = skill / 2.211_699;
    }
    if dot(vel, vel).sqrt() < 833.333_3 {
        k = 1.0;
    }
    word & 0xfffe_07ff | (((k * 31.0) as i32 as u32) & 0x3f) << 11
}

/// `AIComputer_SetTuck` 0x13a028: as [`set_throttle`], with the tuck bit (8) set.
pub fn set_tuck(skill: f32, vel: V4, word: u32) -> u32 {
    set_throttle(skill, vel, word) | 8
}

/// The grudge fields `AIComputer_IsRivalTarget` reads (rider+0x118 on, +0x120 against whom, +0x124 level, +0x128
/// cooled).
#[derive(Clone, Copy, Debug, Default)]
pub struct GrudgeOf {
    pub on: i32,
    pub target: i32,
    pub level: i32,
    pub cooled: i32,
}

/// `AIComputer_IsRivalTarget` 0x139928: rider `i` is this one's rival: in knockdown practice (game mode 0) always;
/// else a grudge against it at level 2, or at level 1 not yet cooled.
pub fn is_rival_target(practice: bool, g: &GrudgeOf, i: i32) -> bool {
    if practice {
        return true;
    }
    g.on != 0 && g.target == i && if g.level == 1 { g.cooled == 0 } else { g.level == 2 }
}

/// `AIComputer_PickTargetRider` 0x1396c8: whether the computer rider has a rider to ride at (`target`, +0x890, −1
/// for none) and if it is a rival (`rival`, written only when a rider is picked). A target is dropped past 1050 cm
/// (the pair record) or when rider+0x348 is over 600. With none, under 540 of rider+0x348 and on every 12th tick, the
/// riders within 700 cm are weighed: distance × (1 − cos bearing) (ahead is nearest), a rival's × (1 + |cos
/// bearing|); the lightest (ties to the later) is taken. `recs[i]` = the pair record [distance, bearing],
/// `heading` rider+0x1b0, `me` rider+0x460, `n` the riders in the race.
#[allow(clippy::too_many_arguments)]
pub fn pick_target_rider(target: &mut i32, rival: &mut bool, recs: &[[f32; 2]], heading: f32, x348: f32, me: i32, n: i32, tick: i32, is_rival: impl Fn(i32) -> bool) -> bool {
    if 0 <= *target && (1050.0 < recs[*target as usize][0] || 600.0 < x348) {
        *target = -1;
    }
    if 0 <= *target {
        return true;
    }
    if !(x348 < 540.0) || tick % 12 != 0 {
        return false;
    }
    let mut best = 1e10f32;
    for i in 0..n {
        if i == me {
            continue;
        }
        let dist = recs[i as usize][0];
        if !(dist <= 700.0) {
            continue;
        }
        let r = is_rival(i);
        let a = recs[i as usize][1] - heading;
        let k = a * 0.159_154_94 + 0.5;
        let mut fl = k as i32 as f32;
        if k < fl {
            fl -= 1.0;
        }
        let c = crate::math::cos(a - fl * 6.283_185_5);
        let w = if r { c.abs() + 1.0 } else { 1.0 - c };
        let v = dist * w;
        if v <= best {
            *rival = r;
            *target = i;
            best = v;
        }
    }
    0 <= *target
}

/// `AIComputer_FindNearbyRider` 0x1394e0 (called by `AIComputer_ControlsDispatch` 0x1389c8 every tick): the shove.
/// Not while the rider's layer-1 clip is a shove (type 0x15), only on every 6th tick: the first rider (in index
/// order) within 200 cm (the pair record) and alongside (the bearing from the rider's heading, wrapped, between 30°
/// and 150° either side) that the rider holds a grudge against at level 1 or 2 (cooled or not). `Some(true)` when
/// that rider is to the left of the heading (the bearing difference above 0), else `Some(false)`; the dispatcher
/// puts it in the pad word's shove field (bits 23–24: 3 for true, 1 for false).
pub fn find_nearby_rider(shoving: bool, tick: i32, recs: &[[f32; 2]], heading: f32, me: i32, n: i32, g: &GrudgeOf) -> Option<bool> {
    if shoving || tick % 6 != 0 {
        return None;
    }
    for i in 0..n {
        if i == me {
            continue;
        }
        let r = recs[i as usize];
        if !(r[0] < 200.0) {
            continue;
        }
        let a = heading - r[1];
        let k = a * 0.159_154_94 + 0.5;
        let mut fl = k as i32 as f32;
        if k < fl {
            fl -= 1.0;
        }
        let a = a - fl * 6.283_185_5;
        let m = a.abs();
        if !(0.523_598_8 < m) || !(m < 2.617_993_8) {
            continue;
        }
        if g.on != 0 && g.target == i && (g.level == 1 || g.level == 2) {
            return Some(0.0 < a);
        }
    }
    None
}

/// The shove field `AIComputer_ControlsDispatch` 0x1389c8 writes from [`find_nearby_rider`].
pub fn shove_bits(word: u32, side: Option<bool>) -> u32 {
    match side {
        Some(true) => word & 0xfe7f_ffff | 0x180_0000,
        Some(false) => word & 0xfe7f_ffff | 0x80_0000,
        None => word,
    }
}

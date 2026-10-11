//! A rider hit by another (board row E8d1): `Boarder_RiderHit` 0x124920 exactly, in the game's units (cm, cm/s, z
//! up) and the vector unit's order of operations, checked bit for bit in the function runner (`tests/hit.rs`).
//! `Boarder_RiderCollisions` 0x123fc8 calls it for both riders of a pair that touch (once every three ticks), with
//! the closing speed split by the two riders' speed stats ([`pair_speeds`]), and for a shove.

use crate::jump::{add, dot, normalize, scale, sub};

type V4 = [f32; 4];

/// What `Boarder_RiderHit` reads and writes on the rider hit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitRider {
    /// rider+0x1b0: the rider's heading (the record's angle is compared with it)
    pub heading: f32,
    /// rider+0x140 position, +0x150 velocity, +0x2a0 the ground normal (cm, cm/s)
    pub pos: V4,
    pub vel: V4,
    pub normal: V4,
    /// rider+0x424: the motion (1 air, 2 ground, 3 rail, 5 and 6 wiped out)
    pub motion: u32,
    /// rider+0x1c8: the crouch, set to 1 for the hop
    pub crouch: f32,
}

/// The calls `Boarder_RiderHit` makes, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitCall {
    /// knocked into the air: `Jump_ApplyImpulse`, `Boarder_SetMotion`(1), `Boarder_SetState`(0xd)
    Hop,
    /// the rider's virtual +0x6c with the push × 0.0012
    Push(f32),
    /// `Boarder_Stumble`, then (by the other rider's layer-1 clip: 0x15 is a shove) `Rivals_OnHit` or
    /// `Rivals_OnBump` and `Speech_RiderHitLine`(shove: 0, else 1; 0)
    Stumble { shove: bool },
    /// `Boarder_Wipeout` with: where (position + direction × the record's first word), the rider's velocity
    /// normalised, the direction, the push; then `Rivals_OnKnockdown`, `Speech_RiderHitLine`(not a shove, 1) and
    /// `Speech_OnRiderHit`
    Wipeout { at: V4, vel_dir: V4, dir: V4, push: f32, shove: bool },
}

/// `Boarder_RiderHit` 0x124920: the rider takes a hit of `speed` (cm/s, signed) along the unit `dir`, from a rider
/// whose layer-1 clip is a shove (`shove`, clip type 0x15) or not. `rec` is the pair's record (rider+0x40 + 0x20 ×
/// the other's index: [0] how far along the direction the wipe-out is placed, [1] an angle), `practice` the game
/// mode (0x32de8c) being 0, knockdown practice (a lower knock-down speed). Returns whether the rider was knocked down.
pub fn rider_hit(speed: f32, dir: V4, rec: [f32; 2], practice: bool, shove: bool, r: &mut HitRider, calls: &mut Vec<HitCall>) -> bool {
    let d = (r.heading - rec[1]) * 0.159_154_94 + 0.5;
    let mut turns = d as i32 as f32;
    if d < turns {
        turns -= 1.0;
    }
    let mut mag = speed.abs();
    let push = if speed < 0.0 {
        if -500.0 < speed { speed } else { -500.0 }
    } else if speed < 500.0 {
        speed
    } else {
        500.0
    };
    let along = dot(dir, r.normal) * push;
    if 1000.0 < mag && r.motion == 2 && ((r.heading - rec[1]) - turns * 6.283_185_5).abs() < 0.785_398_2 && 0.0 < along {
        r.crouch = 1.0;
        calls.push(HitCall::Hop);
        return false;
    }
    let mut d2 = dir;
    if r.motion != 1 {
        // on the ground or a rail: the push flattened onto the ground
        let n = r.normal;
        let k = dot(dir, n);
        d2 = normalize(sub(dir, scale(n, k)));
    }
    r.vel = add(r.vel, scale(d2, push));
    calls.push(HitCall::Push(push * 0.0012));
    if r.motion.wrapping_sub(5) < 2 {
        return false;
    }
    if r.motion == 1 && 0.0 < along {
        mag = 0.0;
    }
    let knock = if practice { 530.502_87 } else { 733.966_1 };
    if mag <= knock && r.motion != 3 {
        if 167.317_38 < mag {
            calls.push(HitCall::Stumble { shove });
        }
        return false;
    }
    let at = add(r.pos, scale(dir, rec[0]));
    calls.push(HitCall::Wipeout { at, vel_dir: normalize(r.vel), dir, push, shove });
    true
}

/// The closing speeds `Boarder_RiderCollisions` hands each rider of a touching pair: with `n` the unit from the
/// first to the second and `ma`, `mb` their speed stats (`Boarder_GetSpeedStat`; 1e30 for a rider finished and
/// stopped), k = (vb·n − va·n) / (ma + mb); the first takes mb·k, the second −ma·k (both along `n`).
pub fn pair_speeds(n: V4, va: V4, vb: V4, ma: f32, mb: f32) -> (f32, f32) {
    let k = (dot(vb, n) - dot(va, n)) / (ma + mb);
    (mb * k, -ma * k)
}

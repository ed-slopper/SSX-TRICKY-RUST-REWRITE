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

/// `Race_UpdatePairDistances` 0x115a80, every tick: for each pair of riders (in the race's order) the flat distance
/// between their positions (rider+0x480; x and y only) and the bearing each way (atan2 of the difference, the game's
/// inline atan2 around `Math_Atan`): the later rider keeps the angle from it to the earlier one as it comes, the
/// earlier one that angle + π wrapped to −π..π. `records[i][j]` is rider i's record of rider j (rider+0x40 + 0x20·j:
/// [0] the distance, [1] the bearing); the diagonal is left alone.
pub fn pair_distances(pos: &[V4], records: &mut [Vec<[f32; 2]>]) {
    for i in 0..pos.len() {
        for j in i + 1..pos.len() {
            let d = sub(pos[i], pos[j]);
            let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
            records[i][j][0] = dist;
            records[j][i][0] = dist;
            let a = crate::jump::atan2_inline(d[1], d[0]);
            let t = (a + std::f32::consts::PI) * 0.159_154_94 + 0.5;
            let mut fl = t as i32 as f32;
            if t < fl {
                fl -= 1.0;
            }
            records[i][j][1] = (a + std::f32::consts::PI) - fl * 6.283_185_5;
            records[j][i][1] = a;
        }
    }
}

/// What the shove reads of each rider: position (+0x140), velocity (+0x150), the frame the shove is measured in
/// (+0x4aa0 forward, +0x4ab0 side, +0x4ac0 up, +0x4ad0 where), the shove's heading (+0x1e0), the speed stat
/// (`Boarder_GetSpeedStat`) and the stability byte (stats+0x12).
#[derive(Clone, Copy, Debug, Default)]
pub struct ShoveRider {
    pub pos: V4,
    pub vel: V4,
    pub fwd: V4,
    pub side: V4,
    pub up: V4,
    pub at: V4,
    pub heading: f32,
    pub speed_stat: f32,
    pub stability: u8,
}

/// The shove in `Boarder_RiderCollisions` 0x123fc8 (the shover's layer-1 clip of type 0x15 between its flags 1
/// and 2, three ticks since either's last shove): the other rider is reached if within 150 cm along the shover's up,
/// 180 cm across it and 55° of its shove heading (the bearing −atan2(d·fwd, d·side)); the hit's speed is (108.58082,
/// 250.59537 in knockdown practice, + (6.6·n · (v_other − v_me))·m_me / (m_me + m_other)) × (0.54663557 + 0.739932 ×
/// stability / 255) along n, the unit from the shover to the other (positions +0x140), given to `Boarder_RiderHit` on
/// the other if the first sum is above 0. `None` if out of reach or not positive.
pub fn shove(me: &ShoveRider, other: &ShoveRider, practice: bool) -> Option<(f32, V4)> {
    let d = sub(other.at, me.at);
    let h = dot(d, me.up);
    if !(h.abs() <= 150.0) {
        return None;
    }
    let lat = sub(d, scale(me.up, h));
    if !(dot(lat, lat).sqrt() <= 180.0) {
        return None;
    }
    let f = dot(d, me.fwd);
    let s = dot(d, me.side);
    let bearing = if s == 0.0 {
        if f == 0.0 {
            -f
        } else if 0.0 <= f {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        }
    } else {
        let t = crate::math::atan(f / s);
        if s < 0.0 {
            -(if 0.0 < f { t + std::f32::consts::PI } else { t - std::f32::consts::PI })
        } else {
            -t
        }
    };
    let rel = bearing - me.heading;
    let k = rel * 0.159_154_94 + 0.5;
    let mut fl = k as i32 as f32;
    if k < fl {
        fl -= 1.0;
    }
    if !((rel - fl * 6.283_185_5).abs() < 0.959_931_1) {
        return None;
    }
    let base = if practice { 250.595_37 } else { 108.580_82 };
    let n = normalize(sub(other.pos, me.pos));
    let n66 = scale(n, f32::from_bits(0x40d3_40b7));
    let rv = sub(other.vel, me.vel);
    let speed = base + (dot(n66, rv) * me.speed_stat) / (me.speed_stat + other.speed_stat);
    if !(0.0 < speed) {
        return None;
    }
    Some((speed * (me.stability as f32 * 0.739_932 * 0.003_921_569 + 0.546_635_57), n))
}

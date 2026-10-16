//! The ground tick of [`Rider::step`] is the game's own (board row E8k): `GroundMotion_Update` 0x10a0d8 as
//! `crate::ground_motion::update` (cm, z up), run on a game-side copy of the rider that is kept between ticks and
//! rebuilt from the port's fields whenever the port moved the rider itself (landing, respawn, a get-up).
//!
//! The port keeps metres, y up and a yaw. The two spaces meet here: game (x, y, z) cm = port (x, −z, y) m, a
//! rotation (not a mirror), so the port's forward (−sin yaw, 0, −cos yaw) is the game's heading yaw + π/2 and the
//! identity attitude (the board's forward along +y) is the port's yaw 0 on level ground. The game's steer is
//! positive to the left, the port's to the right.
//!
//! What `GroundMotion_Update` hands to [`Env`] and what the port gives it:
//! - `Boarder_WallCollide` 0x126250: the port's own wall spheres (STANDIN, row E8b) in [`Rider::walls`]
//! - `World_RayCast` 0x25aff8 under `Boarder_ProbeGround`: our vertical query of the drawn mesh (STANDIN, row E7)
//! - `Boarder_TakeOff` 0x128470: exact (`crate::orient::take_off`) with `Jump_ApplyImpulse` 0x1284e0 exact
//!   (`crate::jump::jump_impulse`); `Boarder_SetMotion` and `Boarder_SetState` are bodies not ported (row G18) so
//!   they do nothing here, and the spin rates come from [`Rider::take_off`]
//! - STANDIN: the crash's virtual 0x64, `cBoarder::EmitBrakeSpray` 0x11a798, is only recorded in [`Rider::crash_spray`] (row G19)
//! - `Boarder_Wipeout` 0x11d830, `Score_Crash` 0x156de0, `Boarder_AddMeter` 0x11b018: all in [`Rider::go_down`]
//! - the probe's sideways offset (board row G20b): stats+0x60 (`Stats::scale`, `crate::stats::SCALE`) times rider+0x244,
//!   the lean record that [`lean_ease`] and [`lean_targets`] keep
//! - the rider's up (rider+0x2c0, board row G20c): the record [`Rider::up`], set by `Boarder_PlaceAt` 0x11cae8 and
//!   accumulated by [`lean_up`] at the end of every tick; the ground tick reads it, as `GroundMotion_Update` does
//! - the normal (rider+0x2a0, board row G20c2): on a rail `RailSlideMotion_Update` 0x10b0d0's ([`rail_normal`], with the
//!   lean angle [`Rider::rail_lean`] from [`rail_lean_angle`]) and at a respawn `Boarder_PlaceAt` 0x11cae8's
//!   ([`placed_normal`]); in the air it is still the port's own
//! - the rail normal's inputs (row G20c2c): `Rail_ClosestPoint` 0x259330 over the world grid ([`RailWorld::closest_point`]) and the
//!   whole normal from the game's own inputs ([`rail_slide_normal`]: the closest point, the fall-off limits, the lean
//!   angle and [`rail_normal`], from the board's frame, right, up, velocity and the motion's time, blend and angle) are
//!   exact and checked, but `Rider::rail_step` does not call them yet: the port has no world grid (STANDIN, row
//!   G20c2c1) and no attitude quaternion through a rail (row G20c2c3)
//! - rail capture (row G1b): not done while riding, as before; `Boarder_InstanceCollisions` 0x125088 only with the
//!   course's collision (row E8c3)

use super::{wrap, Rider};
use crate::collide::CollisionWorld;
use crate::ground_motion as gm;
use crate::jump::{self, JumpState};
use crate::orient::{self, Orient, TakeOffEnv, TakeOffRider};
use crate::probe::{self, add, cross, dot, scale, sub, Hit};
use crate::stats;
use crate::surface::SURFACE_TABLE;
use glam::Vec3;

type V4 = [f32; 4];

/// The lean record's rate (rider+0x248) as `Boarder_UpdateLeanTargets` 0x129500 sets it: `0x42855555`, 66.67 per second.
pub const LEAN_RATE: f32 = f32::from_bits(0x42855555);

/// The lean value (rider+0x244) moved one tick toward its target (+0x24c) at `rate` (+0x248) per second: the step of
/// `cBoarder::vf7` 0x117198 for this record, `rate · dt` (the game's `rate · 0.016666668`) either way, or the target
/// itself when it is within a step. `lean` = (value, rate, target).
pub fn lean_ease(lean: [f32; 3], dt: f32) -> f32 {
    let (cur, rate, target) = (lean[0], lean[1], lean[2]);
    let step = rate * dt;
    if target + step < cur {
        cur - step
    } else if cur < target - step {
        cur + step
    } else {
        target
    }
}

/// The lean record's new rate and target: `Boarder_UpdateLeanTargets` 0x129500 (its rider+0x248 / +0x24c half; the
/// rider+0x2c0 half is row G20c). The rate is always 66.67; the target is 0 in motion 3 (the rail), else `−60 · steer`
/// (+0x214, the game's sign) scaled by the speed (|+0x150| · 0.6944444) while that is under 1.
pub fn lean_targets(motion: u32, steer: f32, vel: V4) -> (f32, f32) {
    if motion == 3 {
        return (LEAN_RATE, 0.0);
    }
    let speed = dot(vel, vel).sqrt() * f32::from_bits(0x3f31c71c);
    let mut target = steer * -60.0;
    if speed < 1.0 {
        target *= speed;
    }
    (LEAN_RATE, target)
}

/// The rider's up (rider+0x2c0) one tick on: the other half of `Boarder_UpdateLeanTargets` 0x129500, up = unit(up + 0.5 ·
/// normal) over all four lanes, with the normal (+0x2a0) as the tick left it. In the vector unit's order: the half
/// normal is made first (`vmulbc`), added to the up (`vadd`), and the sum of squares and its `vrsqrt` make the unit
/// vector (`vmulq`), the same as [`unit`]. The up therefore trails the ground's normal (it is not the board's up,
/// +0x1a0, and `Boarder_UpdatePose` 0x129628 only reads it).
/// Writers of +0x2c0, all of `SLUS_203.26`: this function every tick (0x129618) and `Boarder_PlaceAt` 0x11cae8
/// (0x11cdf4), which starts it equal to the normal.
pub fn lean_up(up: V4, normal: V4) -> V4 {
    unit(add(up, scale(normal, 0.5)))
}

/// The rail tangent's |z| over which `RailSlideMotion_Update` 0x10b0d0 (0x10b76c) calls the rail steep (about 67° from
/// the level): `0.92`, 0x3f6b851f.
const STEEP_RAIL: f32 = f32::from_bits(0x3f6b851f);

/// The rail's lean angle, the rail motion object's +8 (zero from `RailSlideMotion_Enter` 0x10b068), one tick on:
/// `RailSlideMotion_Update` 0x10b0d0 (0x10b934–0x10b9e8). The angle aims at `lateral · k · m` (the rider's sideways
/// distance from the rail in cm, k·m = 0.6981317 · 0.0125 with the board across the rail, rider+0x1dc 3 or 4, else
/// 0.34906584 · 0.033333335), negated when the board's up points down (`up_z`, rider+0x1a8 < 0), and moves to it in
/// steps of at most 5 (radians). Its sine and cosine feed [`rail_normal`], which only reads them on a steep rail.
pub fn rail_lean_angle(prev: f32, lateral: f32, kind: u32, up_z: f32) -> f32 {
    let (per_cm, scale_) = if kind.wrapping_sub(3) < 2 {
        (f32::from_bits(0x3f32_b8c2), f32::from_bits(0x3c4c_cccd))
    } else {
        (f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3d08_8889))
    };
    let mut aim = lateral * per_cm * scale_;
    if up_z < 0.0 {
        aim = -aim;
    }
    let five = f32::from_bits(0x40a0_0000);
    if aim + five < prev {
        prev - five
    } else if prev < aim - five {
        prev + five
    } else {
        aim
    }
}

/// The ground normal (rider+0x2a0) `RailSlideMotion_Update` 0x10b0d0 leaves while the rider is on the rail (its store at
/// 0x10bbb8), from the rail's unit tangent `tangent` (the closest point's, turned to run with the velocity), the
/// board's up (rider+0x1a0, before the tick's own turn) and the lean angle [`rail_lean_angle`] just made. All four lanes,
/// in the vector unit's order.
/// - on a rail that is not steep (|tz| ≤ 0.92): the level's up with its part along the tangent taken out, made unit
///   length (`vmulx`, `vsub`, `vrsqrt`), turned round when it points against the board's up;
/// - on a steep one: the board's up tipped about the rail by the lean angle: `c1 = unit(up × t)`, `c2 = t × c1`,
///   normal = `c2 · cos − c1 · sin` (not made unit again).
pub fn rail_normal(tangent: V4, board_up: V4, angle: f32) -> V4 {
    let (sin, cos) = crate::math::sin_cos(angle);
    if STEEP_RAIL < tangent[2].abs() {
        let c1 = unit(cross(board_up, tangent));
        let c2 = cross(tangent, c1);
        sub(scale(c2, cos), scale(c1, sin))
    } else {
        let n = unit(sub([0.0, 0.0, 1.0, 0.0], scale(tangent, tangent[2])));
        if dot(n, board_up) < 0.0 {
            scale(n, -1.0)
        } else {
            n
        }
    }
}

/// The normal (rider+0x2a0) `Boarder_PlaceAt` 0x11cae8 leaves: (0, 0, 1, 0) (stored at 0x11cc5c), and when its ray cast
/// under the spawn (`World_RayCast` 0x25aff8, row E7) hits, the ray's normal `hit` (0x11cd08). It then copies the
/// normal to the rider's up (rider+0x2c0, 0x11cdf4: the four words). With the flag (a3 ≠ 0, the start-gate placements)
/// the ray is not cast and the default stays.
pub fn placed_normal(hit: Option<V4>) -> V4 {
    hit.unwrap_or([0.0, 0.0, 1.0, 0.0])
}

/// STANDIN: the rider's sideways distance from the rail in cm, the game's `rider+0x190 · (closest point − position)`
/// (0x10b438), for the game's own closest point (`Rail_ClosestPoint` 0x259330, here [`RailWorld::closest_point`], which
/// needs the world grid: row G20c2c1) and board attitude (the quaternion, rows G20c2c2 and G20c2c3): the port's offset
/// `off` (m, positive to the left of the travel) with the board's side (+0x190, its right) taken as the cross of the
/// tangent and the board's up `normal` (game space), the board lying along the rail.
pub(super) fn rail_lateral(travel: Vec3, normal: V4, off: f32) -> f32 {
    let side = unit(cross(game_dir(travel), normal));
    let left = Vec3::Y.cross(travel).normalize_or(Vec3::X);
    rail_offset(side, to_game(-left * off, 100.0, 0.0))
}

/// The sideways distance `RailSlideMotion_Update` 0x10b0d0 measures (0x10b438): the board's right (+0x190) dotted with
/// the vector from the board to the closest point on the rail (cm, all four lanes).
pub fn rail_offset(right: V4, to_rail: V4) -> f32 {
    dot(right, to_rail)
}

/// The rail's tangent turned to the way the board travels (0x10b1d0..0x10b228): reversed, all four lanes, when the unit
/// velocity (+0x150) is against it.
pub fn rail_travel(tangent: V4, velocity: V4) -> V4 {
    if dot(unit(velocity), tangent) < 0.0 { scale(tangent, -1.0) } else { tangent }
}

/// The reach of every box test in `Rail_ClosestPoint` 0x259330: 300 cm (`0x43960000`) round the query point.
const REACH: f32 = f32::from_bits(0x4396_0000);

/// The owner of a rail curve (the object at curve+0x58): `Rail_ClosestPoint` 0x259330 only looks at bit 0 of its
/// +0x18 (the rail is switched on) and hands its +0x24 word back as the hit's +0x34 (stored as rider+0x290 by
/// `RailSlideMotion_Update` 0x10b0d0, 0x10b158).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailOwner {
    /// +0x18: bit 0 set = the curve counts
    pub flags: u32,
    /// +0x24
    pub tag: u32,
}

/// One curve of the world grid (the record the pointer at patch+0x3c lists), cm, z up. Its four rows (curve+0x00,
/// +0x10, +0x20, +0x30) are the cubic's coefficients: the point at t is `m0·t³ + m1·t² + m2·t + m3`, all four lanes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailCurve {
    pub m: [V4; 4],
    /// curve+0x58. STANDIN (none): the original reads its +0x18 even for a null pointer (address 0x18); no table of
    /// the game has one, so a curve without an owner is simply skipped here.
    pub owner: Option<RailOwner>,
    /// curve+0x5c..0x73: the box (min x, y, z, then max x, y, z)
    pub bbox: [f32; 6],
}

impl RailCurve {
    /// The basis vector through the four rows: `vmulax`, `vmadday`, `vmaddaz`, `vmaddw` (each product rounded, then
    /// the sum), lane by lane.
    fn eval(&self, b: V4) -> V4 {
        let mut o = [0.0f32; 4];
        for k in 0..4 {
            o[k] = ((self.m[0][k] * b[0] + self.m[1][k] * b[1]) + self.m[2][k] * b[2]) + self.m[3][k] * b[3];
        }
        o
    }

    /// The point at t: the basis (t³, t², t, 1) with t³ = t · (t · t).
    fn at(&self, t: f32) -> V4 {
        let t2 = t * t;
        let t3 = t * t2;
        self.eval([t3, t2, t, 1.0])
    }
}

/// One patch record (0x4c bytes at `cell+0x38 + n·0x4c`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailPatch {
    /// patch+0x00..0x17: the box (min x, y, z, then max x, y, z)
    pub bbox: [f32; 6],
    /// patch+0x24 (a word), +0x28 (a halfword; the halfword after it, +0x2a, is the curve count), +0x2c (a halfword)
    /// and +0x48 (a word): the record is in use when any of them or the count is not zero (proven only as that
    /// test: what they hold is not known)
    pub x24: u32,
    pub x28: u16,
    pub x2c: i16,
    pub x48: u32,
    /// the curves the pointer at patch+0x3c lists; their count is the halfword at patch+0x2a
    pub curves: Vec<RailCurve>,
}

/// One cell of the world grid: its box and its patches (`gWorld+0x48` of them, contiguous).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailCell {
    pub bbox: [f32; 6],
    pub patches: Vec<RailPatch>,
}

/// The world grid `Rail_ClosestPoint` 0x259330 walks (the object at `gWorld` 0x346488, the same grid `World_RayCast`
/// 0x25aff8 reads): a rectangle of `nx · ny` square cells of side `cell` (cm) from the origin (`x0`, `y0`), each
/// pointing at a [`RailCell`] or at nothing. STANDIN (row G20c2c1): the port builds this from nothing yet; the game
/// builds it when it loads the course.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailWorld {
    /// world+0x04, +0x08
    pub x0: f32,
    pub y0: f32,
    /// world+0x28: the side of a cell
    pub cell: f32,
    /// world+0x2c, +0x30
    pub nx: i32,
    pub ny: i32,
    /// world+0x4c: `ny · nx` cells, row by row
    pub cells: Vec<Option<RailCell>>,
}

/// What `Rail_ClosestPoint` writes into its output record: the point on the curve (out+0x10), the unit tangent
/// there (out+0x20), and the owner's word (out+0x34).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailHit {
    pub point: V4,
    pub tangent: V4,
    pub word: u32,
}

/// The lower and upper ends of the parameter range `Rail_ClosestPoint` searches after its four chords, by the chord
/// that came closest (0 = the curve's start itself): the two five-word tables at 0x3a78e0 and 0x3a78f8.
const SEARCH_LO: [f32; 5] = [0.0, 0.0, 0.0, 0.25, 0.25];
const SEARCH_HI: [f32; 5] = [0.75, 0.75, 1.0, 1.0, 1.0];
/// The golden-section ratios (0x3ec3910c, 0x3f1e377a) and the width the search stops at (0x3a03126f).
const GOLD_LO: f32 = f32::from_bits(0x3ec3_910c);
const GOLD_HI: f32 = f32::from_bits(0x3f1e_377a);
const SEARCH_WIDTH: f32 = f32::from_bits(0x3a03_126f);

/// Distance between two points (all four lanes): `vsqrt` of the sum of squares, as the original measures it.
fn dist4(a: V4, b: V4) -> f32 {
    let v = sub(a, b);
    dot(v, v).sqrt()
}

/// The part of `Rail_ClosestPoint` 0x259330 that works on one curve (0x259790–0x259ee0): the start and four chords
/// at t = ¼, ½, ¾, 1 find the chord nearest `pos` (the foot of `pos` on each, clamped to it), a golden-section search
/// of the bracket [`SEARCH_LO`]/[`SEARCH_HI`] of that chord follows (at most 24 steps, until it is under 0.0005
/// wide), and the result is the *last* point it evaluated, with its parameter and distance (not the best one).
fn curve_search(c: &RailCurve, pos: V4) -> (V4, f32, f32) {
    // the chords
    let mut prev = c.at(0.0);
    let mut best = dist4(pos, prev);
    let mut best_k = 0usize;
    for k in 1..=4usize {
        let t = k as f32 * 0.25;
        let pk = c.at(t);
        let seg = sub(pk, prev);
        let rel = sub(pos, prev);
        let mut u = dot(rel, seg) / dot(seg, seg);
        if u < 0.0 {
            u = 0.0;
        }
        if 1.0 < u {
            u = 1.0;
        }
        let foot = add(prev, scale(seg, u));
        let d = dist4(pos, foot);
        if d < best {
            best = d;
            best_k = k;
        }
        prev = pk;
    }
    // the golden-section search
    let (mut a, mut b) = (SEARCH_LO[best_k], SEARCH_HI[best_k]);
    let w = b - a;
    let mut c1 = a + w * GOLD_LO;
    let mut c2 = a + w * GOLD_HI;
    let mut d1 = dist4(c.at(c1), pos);
    let mut d2 = dist4(c.at(c2), pos);
    // the point, parameter and distance of the last evaluation: at first c2 (the point left in the original's buffer
    // is the one at b, which no step can leave as the last: the first step always evaluates once more)
    let mut last = (c.at(b), c2, d2);
    let mut steps = 0;
    while steps < 24 && !((b - a).abs() <= SEARCH_WIDTH) {
        if d2 < d1 {
            a = c1;
            c1 = c2;
            d1 = d2;
            c2 = c2 * GOLD_HI + b * GOLD_LO;
            let p = c.at(c2);
            d2 = dist4(p, pos);
            last = (p, c2, d2);
        } else {
            b = c2;
            c2 = c1;
            d2 = d1;
            c1 = c1 * GOLD_HI + a * GOLD_LO;
            let p = c.at(c1);
            d1 = dist4(p, pos);
            last = (p, c1, d1);
        }
        steps += 1;
    }
    last
}

impl RailWorld {
    /// `Rail_ClosestPoint` 0x259330 (`world` = the grid at `gWorld`, `pos` = the query point, all four lanes; the third
    /// argument is not read). Walks the cells from two before the point's cell to its own (clamped into the grid) in
    /// x, y, skipping what is more than 300 cm from the point (cell, patch and curve boxes), and in each curve that is
    /// switched on (owner flags bit 0) searches the nearest point ([`curve_search`]); a curve's result is taken when
    /// it is the first or nearer than the best so far. `None` = the original returns 0 and writes nothing.
    pub fn closest_point(&self, pos: V4) -> Option<RailHit> {
        // the cell of the point: ((p − origin) + side) / side, cut toward zero (cvt.w.s)
        let cs = self.cell;
        let ix = (((pos[0] - self.x0) + cs) / cs) as i32;
        let iy = (((pos[1] - self.y0) + cs) / cs) as i32;
        let nx1 = self.nx.wrapping_sub(1) as i64;
        let ny1 = self.ny.wrapping_sub(1) as i64;
        let ix_c = if 0 < ix { ix as i64 } else { 0 };
        let iy_c = if 0 < iy { iy as i64 } else { 0 };
        let a1 = ix.wrapping_sub(2) as i64;
        let t1 = iy.wrapping_sub(2) as i64;
        let x_lo = if -1 < a1 { a1 } else { 0 };
        let x_hi = if nx1 < ix_c { nx1 } else { ix_c };
        let y_lo = if -1 < t1 { t1 } else { 0 };
        let y_hi = if ny1 < iy_c { ny1 } else { iy_c };
        let near = |b: &[f32; 6]| {
            b[0] <= pos[0] + REACH
                && pos[0] - REACH <= b[3]
                && b[1] <= pos[1] + REACH
                && pos[1] - REACH <= b[4]
                && b[2] <= pos[2] + REACH
                && pos[2] - REACH <= b[5]
        };
        let mut found: Option<(RailHit, f32)> = None;
        let mut x = x_lo;
        while x <= x_hi {
            let mut y = y_lo;
            while y <= y_hi {
                let cx = if x < 0 { 0 } else if nx1 < x { nx1 } else { x };
                let cy = if y < 0 { 0 } else if ny1 < y { ny1 } else { y };
                y += 1;
                let idx = cy * self.nx as i64 + cx;
                let Some(Some(cell)) = usize::try_from(idx).ok().and_then(|i| self.cells.get(i)) else { continue };
                if !near(&cell.bbox) {
                    continue;
                }
                for patch in &cell.patches {
                    if patch.x24 == 0 && patch.x28 == 0 && patch.curves.is_empty() && patch.x2c == 0 && patch.x48 == 0 {
                        continue;
                    }
                    if !near(&patch.bbox) {
                        continue;
                    }
                    for c in &patch.curves {
                        let Some(owner) = c.owner else { continue };
                        if owner.flags & 1 == 0 || !near(&c.bbox) {
                            continue;
                        }
                        let (point, t, d) = curve_search(c, pos);
                        if found.is_none_or(|(_, best)| d < best) {
                            // the tangent: the derivative's basis (3t², 2t, 1, 0) through the rows, made unit length
                            let dv = c.eval([(t * 3.0) * t, t + t, 1.0, 0.0]);
                            found = Some((RailHit { point, tangent: unit(dv), word: owner.tag }, d));
                        }
                    }
                }
            }
            x += 1;
        }
        found.map(|(h, _)| h)
    }
}

/// What `RailSlideMotion_Update` 0x10b0d0 reads to place the board on the rail, all in the game's space (cm, z up).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailSlideIn {
    /// rider+0x4a90: the board's position
    pub pos: V4,
    /// rider+0x4a60, +0x4a70, +0x4a80: the board's frame rows (x, y, z)
    pub frame: [V4; 3],
    /// rider+0x150: the velocity
    pub vel: V4,
    /// rider+0x190: the board's right (the attitude quaternion's x axis; rows G1, G5)
    pub side: V4,
    /// rider+0x1a0: the board's up (its z axis)
    pub up: V4,
    /// rider+0x1dc: the stance kind (3 and 4 are the slides)
    pub kind: u32,
    /// motion object +0xc: seconds on the rail
    pub time: f32,
    /// rider+0x284: the fall-off blend (1 for the first 0.6 s)
    pub blend: f32,
    /// motion object +8: the lean angle
    pub lean: f32,
}

/// The normal and lean angle `RailSlideMotion_Update` leaves while the board stays on the rail.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailNormal {
    /// rider+0x2a0
    pub normal: V4,
    /// motion object +8
    pub lean: f32,
}

/// What `RailSlideMotion_Update` does with the rail it finds: the hit it stores (rider+0x2d0 the point, +0x290 the
/// word), the blend it leaves at rider+0x284, and the normal and lean angle when the board is inside the fall-off
/// limits (`None`: it is outside, and the original takes the board off the rail).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailSlide {
    pub hit: RailHit,
    pub blend: f32,
    pub on: Option<RailNormal>,
}

/// The rail normal from the game's own inputs (board row G20c2c): `RailSlideMotion_Update` 0x10b0d0 from the call of
/// `Rail_ClosestPoint` (0x10b108) to the normal's store (0x10bbb8), without the velocity and the attitude it also
/// moves. `None`: no rail within reach (the original skips everything and takes the board off).
/// - the board's offset from the closest point is tested against the fall-off limits: with the blend b (1 while the
///   rail time is under 0.6 s), the point minus the position, moved 30 cm along the frame's y row, dotted with the
///   frame's rows must stay under 150b + 80(1−b), 50, 50b + 30(1−b) (x, y, z; at or over any of them = off);
/// - inside: the tangent is turned to the travel ([`rail_travel`]), the lean angle follows the sideways offset
///   `side · (point − position)` ([`rail_offset`], [`rail_lean_angle`]) and the normal is [`rail_normal`] of both.
pub fn rail_slide_normal(world: &RailWorld, i: &RailSlideIn) -> Option<RailSlide> {
    let hit = world.closest_point(i.pos)?;
    let d = sub(hit.point, i.pos);
    let mut blend = i.blend;
    if i.time < 0.6 {
        blend = 1.0;
    }
    let rest = 1.0 - blend;
    let off = add(d, scale(i.frame[1], -30.0));
    let outside = blend * 150.0 + rest * 80.0 <= dot(i.frame[0], off).abs()
        || blend * 50.0 + rest * 50.0 <= dot(i.frame[1], off).abs()
        || blend * 50.0 + rest * 30.0 <= dot(i.frame[2], off).abs();
    if outside {
        return Some(RailSlide { hit, blend, on: None });
    }
    let travel = rail_travel(hit.tangent, i.vel);
    let lean = rail_lean_angle(i.lean, rail_offset(i.side, d), i.kind, i.up[2]);
    Some(RailSlide { hit, blend, on: Some(RailNormal { normal: rail_normal(travel, i.up, lean), lean }) })
}

/// The board's attitude on the rail (board row G20c2c2): `RailSlideMotion_Update` 0x10b0d0 from the normal's store
/// (0x10bbb8) to `Boarder_UpdateOrientation` (0x10bdbc). From the normal `n` it just made, the travel `t`
/// ([`rail_travel`]) and the lean angle (its `Math_SinCos`): the board's up wanted, `n·cos + (n × t)·sin`
/// (`Vec4_Scale`, vadd); the target, the identity turned from z to that up (`Quat_RotateVecToVec`), then turned again
/// from its own forward (the y axis turned by it, as [`orient::turn_axis`]) to the travel; the quaternion
/// (rider+0x170) a twelfth of the way to it (`Quat_Slerp` at 1/12). The caller then runs
/// [`orient::update_orientation`] for +0x180..+0x1b0.
pub fn rail_attitude(q: V4, normal: V4, travel: V4, lean: f32) -> V4 {
    let (sin, cos) = crate::math::sin_cos(lean);
    let up = add(scale(normal, cos), scale(cross(normal, travel), sin));
    let target = orient::rotate_vec_to_vec([1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], up);
    let fwd = orient::turn_axis(target, [0.0, 1.0, 0.0]);
    let target = orient::rotate_vec_to_vec(target, fwd, travel);
    orient::slerp(f32::from_bits(0x3daa_aaab), q, target)
}

/// `Boarder_PlaceAt` 0x11cae8's placement ray (flag 0): `World_RayCast` 0x25aff8 on the course with a `cWorldLine`
/// (0, the place less (height + 200) cm in z, the place plus 200, kind 2), height 0 as the respawns pass it. On a hit
/// (t > 0) the rider's position (+0x140) becomes the hit's point and its normal the hit's: both, in the game's space.
/// None without the course's collision (a levels folder) or with no hit.
pub fn placement_ray(world: &CollisionWorld, place: Vec3) -> Option<(V4, V4)> {
    let course = world.course.as_ref()?;
    let p = to_game(place, 100.0, 1.0);
    let from = [p[0], p[1], p[2] - 200.0, p[3]];
    let to = [p[0], p[1], p[2] + 200.0, p[3]];
    let mut q = crate::worldline::WorldLine::new(0.0, from, to, 2);
    let (t, h) = course.ray_cast(&mut q, world.showoff, None, &|i| world.course_skips(i));
    h.filter(|_| 0.0 < t).map(|h| (h.pos, h.normal))
}

/// A game direction (z up) as a port direction (y up), the other way of [`game_dir`].
pub fn port_dir(v: V4) -> Vec3 {
    to_port(v, 1.0)
}

/// A port direction (unit, y up) as the game's (z up): [`to_game`] with k = 1, w = 0. The rider's normal and up are kept
/// in the game's space, so the lean record's tick and the ground tick take them as they are.
pub fn game_dir(v: Vec3) -> V4 {
    to_game(v, 1.0, 0.0)
}

const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

/// The port's heading (about y, forward (−sin, 0, −cos)) as the game's yaw (+0x1b0), the other way of the ground
/// tick's `wrap(g.yaw − π/2)`.
pub fn game_yaw(yaw: f32) -> f32 {
    super::wrap(yaw + HALF_PI)
}

/// A port vector (m, y up) as a game vector (cm, z up); `w` is 1 for a point, 0 for a direction.
pub(super) fn to_game(v: Vec3, k: f32, w: f32) -> V4 {
    [v.x * k, -v.z * k, v.y * k, w]
}

/// A game vector (cm, z up) as a port vector (m, y up) (`k` = 0.01), or a direction (`k` = 1).
pub(super) fn to_port(v: V4, k: f32) -> Vec3 {
    Vec3::new(v[0] * k, v[2] * k, -v[1] * k)
}

pub(super) fn unit(v: V4) -> V4 {
    scale(v, 1.0 / dot(v, v).sqrt())
}

/// The game-side rider kept between ticks, with what the port had written when it was last stored: if the port's
/// position, velocity or yaw differ from that, someone moved the rider and the game-side copy is rebuilt.
#[derive(Clone)]
pub struct GroundCache {
    g: gm::Rider,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
}

impl GroundCache {
    /// A game-side rider stored as the port had it when it was last written: at `pos`, `vel` and `yaw` (port units).
    /// The ground tick ends by storing one of these; tests hand in the original's own tick result with it.
    pub fn new(g: gm::Rider, pos: Vec3, vel: Vec3, yaw: f32) -> Self {
        Self { g, pos, vel, yaw }
    }

    /// The game-side rider.
    pub fn rider(&self) -> &gm::Rider {
        &self.g
    }

    /// The board's attitude as the game-side rider has it: the quaternion rider+0x170, the forward +0x180, side +0x190
    /// and up +0x1a0 it derives and the heading +0x1b0. The game keeps these untouched through a jump (board row
    /// G20c3c): this is what the landing's first ground tick starts from.
    pub fn attitude(&self) -> Orient {
        Orient { q: self.g.quat, fwd: self.g.p.fwd, side: self.g.board_side, up: self.g.board_up, yaw: self.g.yaw }
    }

    /// The port has not moved the rider since this was stored (same position and yaw).
    pub fn is_at(&self, pos: Vec3, yaw: f32) -> bool {
        self.pos == pos && self.yaw == yaw
    }
}

/// A rotation matrix with columns `x`, `y`, `z` as a unit quaternion (w, x, y, z).
pub(super) fn quat_of(x: V4, y: V4, z: V4) -> V4 {
    let (m00, m10, m20) = (x[0], x[1], x[2]);
    let (m01, m11, m21) = (y[0], y[1], y[2]);
    let (m02, m12, m22) = (z[0], z[1], z[2]);
    let tr = m00 + m11 + m22;
    if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [0.25 * s, (m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [(m21 - m12) / s, 0.25 * s, (m01 + m10) / s, (m02 + m20) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m02 - m20) / s, (m01 + m10) / s, 0.25 * s, (m12 + m21) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m10 - m01) / s, (m02 + m20) / s, (m12 + m21) / s, 0.25 * s]
    }
}

impl Rider {
    /// The landing of `AirMotion_Update` 0x108378 (0x108554..0x1089bc; rows E8r, E7c3d) on the course's collision:
    /// the down ray `cWorldLine`(0.5, P − 100·U, P + 200·U, 1) through the rider's last-hit record (+0x430), U the
    /// board's up (frame row +0x4ac0); if it hits and the velocity goes into its normal, `Boarder_ProbeGround`
    /// 0x128ae8 along that normal; landed when that hits and the height (+0x1bc) is under half the surface's word
    /// 6, put onto the hit point when the height is below 0. The hit for the port's landing, with `p` moved.
    /// STANDIN: P is the position, for the board frame times minus the pivot bone (rider+0x4aa0, the bone's
    /// +0x8c8; G5), and the probe runs from the position, for the pivot turned by half the spin and flip
    /// (0x1085a8..0x108960); the side is the normal × the board's forward in its plane, for rider+0x330. The down
    /// ray's normal is the rider's at every hit (0x108588, row G20c2b).
    pub(super) fn course_landing(&mut self, world: &CollisionWorld, p: &mut Vec3) -> Option<crate::collide::GroundHit> {
        let u = self.board_up();
        let (_, n, _) = world.course_line(0.5, *p - u * 1.0, *p + u * 2.0, 1, Some(&mut self.last_hit))?;
        // any hit's normal becomes the rider's (+0x2a0, stored at 0x108588 before the velocity is tested; row G20c2b)
        self.normal = n;
        if self.vel.dot(n) >= 0.0 {
            return None;
        }
        let normal = game_dir(n);
        let fwd = game_dir(super::heading(self.yaw));
        let plane = sub(fwd, scale(normal, dot(fwd, normal)));
        let plane = scale(plane, 1.0 / dot(plane, plane).sqrt());
        let mut pr = probe::Rider {
            pos: to_game(*p, 100.0, 1.0),
            fwd,
            normal,
            plane_fwd: plane,
            side: cross(normal, plane),
            x244: self.lean[0],
            stat60: self.stats.scale,
            surface: self.surface as u32,
            ..Default::default()
        };
        let (from, to) = probe::line(&pr);
        let hit = world
            .course_line(0.5, to_port(from, 0.01), to_port(to, 0.01), 2, Some(&mut self.last_hit))
            .map(|(at, n, surface)| Hit { kind: 0, point: to_game(at, 100.0, 1.0), normal: game_dir(n), surface });
        if !probe::probe(&mut pr, hit) {
            return None;
        }
        let surface = (pr.surface as usize).min(19);
        if !(pr.height < SURFACE_TABLE[surface][6] * 0.5) {
            return None;
        }
        if pr.height < 0.0 {
            *p = to_port(pr.ground, 0.01);
        }
        Some(crate::collide::GroundHit { y: p.y, normal: port_dir(pr.normal), surface: surface as u8 })
    }

    /// The game-side rider from the port's: the board lies on the ground (its up is the ground's normal) and points
    /// along the port's yaw. The attitude is the game's own, kept through the air (`Rider::att`), while the port left
    /// the yaw alone: after a placement `Boarder_PlaceAt` 0x11cae8's (row G20c3b), after a jump off the snow the one
    /// the take-off tick left (`Boarder_TakeOff` 0x128470 after `GroundMotion_Update` 0x10a0d8, row G20c3c). STANDIN:
    /// otherwise (a spin or flip in the air, a landing the port turned, a take-off from a rail) the game's attitude
    /// quaternion (rider+0x170) is rebuilt from the yaw and the normal (rows G20c3c2, G1a5b3b, G20c2c3 and G5).
    pub fn seed_ground(&self) -> gm::Rider {
        let n = unit(to_game(self.normal, 1.0, 0.0));
        let o = match self.att {
            Some((a, yaw)) if yaw == self.yaw => a,
            _ => {
                let psi = self.yaw + HALF_PI;
                let h = [psi.cos(), psi.sin(), 0.0, 0.0];
                let fwd = unit(sub(h, scale(n, dot(h, n))));
                let side = cross(fwd, n);
                let mut o = Orient { q: quat_of(side, fwd, n), ..Orient::default() };
                orient::update_orientation(&mut o);
                o
            }
        };
        let pos = to_game(self.pos, 100.0, 1.0);
        let plane_fwd = unit(sub(o.fwd, scale(n, dot(o.fwd, n))));
        let height = self.height * 100.0;
        gm::Rider {
            p: probe::Rider {
                pos,
                fwd: o.fwd,
                normal: n,
                ground: sub(pos, scale(n, height)),
                plane_fwd,
                side: cross(n, plane_fwd),
                fall: probe::fall_line(n, o.fwd),
                x244: self.lean[0],
                stat60: self.stats.scale,
                height,
                surface: self.surface as u32,
                surface_ticks: 0,
                kind: 0,
            },
            up: self.up,
            board_up: o.up,
            quat: o.q,
            board_side: o.side,
            yaw: o.yaw,
            ..gm::Rider::default()
        }
    }

    /// One tick on the snow: `GroundMotion_Update` 0x10a0d8 (`crate::ground_motion::update`). `boost_level` is
    /// rider+0x130. Returns true when the rider left the snow (`Boarder_TakeOff` ran). The port's position, velocity,
    /// yaw, normal, height, surface, switch and steer are written back.
    pub fn ground_tick(&mut self, world: &CollisionWorld, dt: f32, boost_level: f32) -> bool {
        let mut g = self.ground_input(dt, boost_level);
        let st = &self.stats;
        let jump_stat = st.bytes[stats::JUMP];
        let mut env = GroundEnv { r: self, world, left: false, jump_stat };
        gm::update(&mut g, &SURFACE_TABLE, &mut env);
        let left = env.left;

        // back to the port
        self.pos = to_port(g.p.pos, 0.01);
        self.vel = to_port(g.vel, 0.01);
        self.yaw = wrap(g.yaw - HALF_PI);
        self.normal = to_port(g.p.normal, 1.0);
        self.height = g.p.height * 0.01;
        self.surface = g.p.surface.min(19) as u8;
        self.switch = g.switch != 0;
        self.steer = -g.steer;
        self.lean[0] = g.p.x244;
        self.gm = Some(GroundCache { g, pos: self.pos, vel: self.vel, yaw: self.yaw });
        left
    }

    /// The game-side rider `GroundMotion_Update` 0x10a0d8 is handed this tick: the one kept from the last tick (or
    /// seeded from the port's, [`Rider::seed_ground`]) with what the port feeds every tick written over it (time scale,
    /// boost, steer, the stat bytes, the control state ...). Takes the kept rider out of `self.gm`; `ground_tick` puts
    /// the tick's result back. Public so a test can run the original on the same input (row G20c3c).
    pub fn ground_input(&mut self, dt: f32, boost_level: f32) -> gm::Rider {
        // the game-side rider: kept while the port left it alone
        let mut g = match self.gm.take() {
            Some(c) if c.pos == self.pos && c.yaw == self.yaw => {
                let mut g = c.g;
                if c.vel != self.vel {
                    g.vel = to_game(self.vel, 100.0, 0.0);
                }
                g
            }
            _ => {
                let mut g = self.seed_ground();
                g.vel = to_game(self.vel, 100.0, 0.0);
                // the game-side rider's own quaternion takes over from here
                self.att = None;
                g
            }
        };
        // exactly zero speed divides by zero in the original's yaw step; the original never rides at a standstill
        // (STANDIN: a millionth of a cm/s along the board for the start gate's states, row G1c)
        if dot(g.vel, g.vel) == 0.0 {
            g.vel = scale(g.p.plane_fwd, 1e-6);
        }
        let st = &self.stats;
        g.timescale = dt * 60.0;
        g.boost_level = boost_level;
        g.speed_timer = self.speed_timer;
        // rider+0x2c0: the record the last tick's `Boarder_UpdateLeanTargets` left (`Boarder_PlaceAt` for the first)
        g.up = self.up;
        // the course's direction ahead, as a game heading
        g.course_yaw = self.course_dir.map_or(g.yaw, |c| f32::atan2(-c.z, c.x));
        g.switch = self.switch as u32;
        g.brake = self.brake_v;
        g.crouch = self.crouch_v;
        g.steer = -self.steer;
        g.d1 = self.band.0 * 100.0;
        g.d2 = self.band.1 * 100.0;
        g.class = st.kind as u32;
        // STANDIN: the control state is "riding" (3), or "landing" (0xb) after a hard landing, for the game's state
        // machine (row G1)
        g.state = if self.hard_land > 0.0 { 0xb } else { 3 };
        g.mask = 0xffff_ffff;
        // the game's stat bytes (`crate::stats`): speed +0x0e, edging +0x11, grip +0x13 and the cubic term +0x19
        let b = |i: usize| st.bytes[i] as f32;
        // the probe's sideways offset: stats+0x60 · rider+0x244 (the cached copy is stale, the port eased the lean)
        g.p.x244 = self.lean[0];
        g.p.stat60 = st.scale;
        g.stats = gm::Stats { speed: b(stats::SPEED), edging: b(stats::EDGING), grip: b(stats::GRIP), cubic: b(stats::CUBIC), stance: 0 };
        // STANDIN: our crouch/steer/speed test, for the game's "skate push clip (0x221) playing" (G5)
        g.skating = self.crouch_v > 0.5 && self.steer.abs() < 0.2 && self.vel.length() < 8.33;
        g
    }

    /// STANDIN: the motion id (rider+0x424: 1 air, 2 ground, 3 rail, 5 wipeout) from the port's own flags, for the
    /// game's motion state machine (row G18)
    fn lean_motion(&self) -> u32 {
        if self.rail.is_some() {
            3
        } else if self.crashed > 0.0 {
            5
        } else if self.grounded {
            2
        } else {
            1
        }
    }

    /// The lean value one tick on (`cBoarder::vf7` 0x117198), before the tick's motion.
    pub(super) fn lean_ease(&mut self, dt: f32) {
        self.lean[0] = lean_ease(self.lean, dt);
    }

    /// The lean record aimed for the next tick and the rider's up moved toward the normal (`Boarder_UpdateLeanTargets`
    /// 0x129500), after the tick's motion: from the steer as it is now (the game's sign: the port's is the other way),
    /// the velocity in cm/s and the normal the tick ended with.
    /// In the air the normal (+0x2a0) is `AirMotion_Update` 0x108378's down ray's whenever that hits ([`Rider::course_landing`],
    /// row G20c2b; a levels folder without the course's collision keeps the port's own, STANDIN). On a rail it is
    /// `RailSlideMotion_Update`'s ([`rail_normal`]) and at a respawn `Boarder_PlaceAt`'s ([`placed_normal`] and its ray,
    /// [`placement_ray`]), row G20c2.
    pub fn lean_aim(&mut self) {
        let (rate, target) = lean_targets(self.lean_motion(), -self.steer, to_game(self.vel, 100.0, 0.0));
        self.lean[1] = rate;
        self.lean[2] = target;
        self.up = lean_up(self.up, game_dir(self.normal));
    }

    /// The walls: a sphere around the body (`Boarder_WallCollide` 0x126250: bounce off at least 0.56 m/s) and
    /// `Boarder_WallImpactCheck` 0x126540. STANDIN: our sphere against the drawn mesh's walls, for the game's
    /// collision spheres and probes (row E8b). Moves `p`; returns (pushed, hit a wall going in).
    pub(super) fn walls(&mut self, world: &CollisionWorld, p: &mut Vec3) -> (bool, bool) {
        use super::BODY_RADIUS;
        let lift = Vec3::Y * (BODY_RADIUS + 0.35);
        let (c, contacts) = world.push_out(*p + lift, BODY_RADIUS);
        *p = c - lift;
        let pushed = !contacts.is_empty();
        let mut hit = false;
        for n in contacts {
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                hit = true;
                let vin = -vn;
                if self.wall_hit.is_none_or(|w| w.1 < self.vel.length()) {
                    self.wall_hit = Some((c, self.vel.length()));
                }
                let side = if n.dot(Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())) > 0.0 { 2 } else { 3 };
                let up = self.board_up();
                if self.grounded {
                    // Boarder_WallImpactCheck: s = max(vin/2, 0.56); over 8.33(up.n)+19.44 m/s is a crash,
                    // over 8.33(up.n) a stumble (only a real knock, not a brush)
                    let s = (0.5 * vin).max(0.5556);
                    let un = up.dot(n);
                    if s > 8.3333 * un + 19.4444 {
                        self.crash_kind = side;
                        self.cause = "wall";
                        self.crash_at = Some((c - n * BODY_RADIUS, n));
                        self.wipe_out();
                    } else if s > 8.3333 * un && vin > 4.0 {
                        self.stumble_by(true);
                    }
                    self.vel += n * (vin + (0.5 * vin).max(0.556));
                } else if vin > 1.0 && n.y < 0.7 && self.air_time > 0.15 {
                    // in the air: glance off anything close to the way your feet point, else go down
                    let f = (0.6 + 0.0135 * vin).min(0.9);
                    // (inside the Megaplex tube the glass only steers the rider)
                    if up.dot(n) > f || self.tube_class != u8::MAX {
                        self.vel += n * vin;
                    } else {
                        if std::env::var("TRICKY_CRASHDBG").is_ok() {
                            println!("air wall vin {vin:.2} n {n:.2?} up {up:.2?} flip {:.2} air {:.2}", self.flip, self.air_time);
                        }
                        self.crash_at = Some((c - n * BODY_RADIUS, n));
                        self.crash_kind = side;
                        self.cause = "air wall";
                        self.wipe_out();
                    }
                } else {
                    self.vel += n * (vin + (0.5 * vin).max(0.556));
                }
            }
        }
        (pushed, hit)
    }
}

/// The rider's collision body (rider+0x470) as `Boarder_WallCollide` uses it: `CollBody_InitRider` 0x236050 with
/// the bounding radius stats+0x60 m (`cBoarder` vf8 0x116ab8), the mask [`gm::WALL_MASK`] (spheres 0 and 1: the
/// upper back and the head, 25 and 20 cm). STANDIN: the spheres on the skeleton's bind pose standing straight up
/// the board's up from the position (upper back 114.7 cm, head 144.5 cm, the bounding centre at the hips 91.7 cm,
/// read from the riders' `_body.mpf`, all riders alike, times stats+0x60), for `CollBody_UpdateFromBones` 0x2361b0
/// on the animated skeleton (row G5).
fn wall_body(g: &gm::Rider) -> crate::collbody::CollBody {
    let mut b = crate::collbody::CollBody::new();
    b.init_rider(g.p.stat60);
    let at = |h: f32| add(g.p.pos, scale(g.board_up, h * g.p.stat60));
    b.center = at(91.7);
    for s in b.spheres.iter_mut() {
        s.0 = b.center;
    }
    b.spheres[0].0 = at(114.7);
    b.spheres[1].0 = at(144.5);
    b.x24 = gm::WALL_MASK as i32;
    b
}

/// The bones `CollBody_UpdateFromBones` 0x2361b0 reads (`collbody::BONES`, bones 0 to 18), in the skeleton's bind
/// pose, cm from the root (hips): x along the board (foot to foot), y across it, z up, read from the riders'
/// `_body.mpf` (`Body3000`; brodi, elise, jp, kaori, zoe: luther, marisol, psymon, seeiah, moby differ by 2 cm at
/// the calves and feet).
const BIND: [(usize, [f32; 3]); 14] = [
    (2, [0.0, 4.7, 23.0]),
    (4, [0.0, -5.9, 52.8]),
    (6, [18.2, -2.5, 38.4]),
    (10, [-18.2, -2.5, 38.4]),
    (7, [44.0, 0.6, 22.3]),
    (11, [-44.0, 0.5, 22.3]),
    (8, [63.5, -6.0, 8.7]),
    (12, [-63.5, -6.0, 8.8]),
    (13, [11.8, -4.9, -13.0]),
    (16, [-11.8, -4.9, -13.0]),
    (14, [11.7, -15.7, -50.1]),
    (17, [-12.0, -15.7, -50.1]),
    (15, [13.1, 2.8, -91.8]),
    (18, [-13.7, 2.7, -91.8]),
];

impl Rider {
    /// The rider's collision body (rider+0x470, game units) with every sphere (mask −1, as `GroundMotion_Update`
    /// 0x10a0d8 leaves +0x494 after the walls), as the boosts' touch test takes it (`cWorldBoarder`(+0x470, 1)).
    /// STANDIN: `CollBody_UpdateFromBones` 0x2361b0 on the bind pose ([`BIND`]) standing on the board's up from
    /// the position (root 91.7 cm up, x along the board's forward), times stats+0x60, for the animated
    /// skeleton (row G5).
    pub fn bind_body(&self) -> crate::collbody::CollBody {
        let (up, fwd) = (to_game(self.board_up(), 1.0, 0.0), to_game(self.forward(), 1.0, 0.0));
        bind_body_at(to_game(self.pos, 100.0, 1.0), fwd, up, self.stats.scale)
    }
}

/// [`Rider::bind_body`] at `pos` (game units) with the board's forward and up and the body scale `k` (stats+0x60).
pub(super) fn bind_body_at(pos: V4, fwd: V4, up: V4, k: f32) -> crate::collbody::CollBody {
    {
        let side = cross(up, fwd);
        let root = add(pos, scale(up, 91.7 * k));
        let mut bones = [[0.0, 0.0, 0.0, 1.0]; 19];
        for (b, [x, y, z]) in BIND {
            bones[b] = add(root, add(scale(fwd, x * k), add(scale(side, y * k), scale(up, z * k))));
        }
        let mut body = crate::collbody::CollBody::new();
        body.init_rider(k);
        body.update_from_bones(&bones);
        body
    }
}

/// `Boarder_WallCollide`'s cast: `World_RayCast` 0x25aff8 with `cWorldBoarder`(the body, 1), its ground normal set
/// (+0x2e0, +0x2d0 = 1).
pub(super) fn wall_cast(world: &CollisionWorld, course: &crate::course_ground::CourseGround, body: &crate::collbody::CollBody, ground: V4) -> (f32, crate::walls::CastRecord) {
    let mut q = crate::worldboarder::WorldBoarder::new(body, 1);
    q.reject = Some(ground);
    let (t, h) = course.ray_cast(&mut q, world.showoff, None, &|i| world.course_skips(i));
    let rec = h.map_or(Default::default(), |h| crate::walls::CastRecord { pos: h.pos, normal: h.normal, object: h.object.map_or(0, |o| o as u32 + 1) });
    (t, rec)
}

/// The rider's handle for the scripts and sounds. STANDIN: 1 for the human, 2 for the others, for the rider's address.
pub(super) fn rider_id(r: &Rider) -> u32 {
    if r.ai { 2 } else { 1 }
}

/// `World_QueryInstances` 0x25bdc4 with `cWorldBoarder`(the body, 1): t, the record (object = index + 1), the object
/// (its index; its flags, its physics record's mass and bounce) and the sphere tree hit (the record's +0x30).
/// STANDIN: no object has a controller here, for the course's controllers (row E7b2); the rollers have left the
/// casts (their flags lose 0x20).
pub(super) fn touch_query(world: &CollisionWorld, course: &crate::course_ground::CourseGround, body: &crate::collbody::CollBody) -> (f32, crate::walls::CastRecord, crate::walls::Touched, Option<(usize, usize)>) {
    let mut q = crate::worldboarder::WorldBoarder::new(body, 1);
    // `Boarder_InstanceCollisions` sets the shape's +0x2f0 to 1 after making it: front faces only (row G4e6)
    q.front_only = true;
    let (t, h) = course.query_instances(&mut q, world.showoff, &|i| world.course_skips(i));
    let Some(h) = h else { return (t, Default::default(), Default::default(), None) };
    let o = h.object.unwrap_or(0);
    let ssf = &course.ssf;
    let phys = ssf.object_physics.get(o).and_then(|p| ssf.physics.get(*p as usize));
    let rec = crate::walls::CastRecord { pos: h.pos, normal: h.normal, object: o as u32 + 1 };
    let touched = crate::walls::Touched {
        id: o as u32,
        flags: course.object_flags(o),
        mass: phys.map_or(0.0, |p| p.mass()),
        bounce: phys.map_or(0.0, |p| p.bounce()),
        controller: None,
    };
    (t, rec, touched, h.tree)
}

/// A roller's sphere tree cast on the course (`World_RayCast` 0x25aff8, or `World_QueryInstances` 0x25bdc4 with
/// `query`): t, the point and the normal.
pub(super) fn roller_cast(world: &CollisionWorld, course: &crate::course_ground::CourseGround, q: &crate::sphtree::WorldSphTree, query: bool) -> (f32, V4, V4) {
    let (t, h) = if query {
        course.query_instances(&mut q.clone(), world.showoff, &|i| world.course_skips(i))
    } else {
        course.ray_cast(&mut q.clone(), world.showoff, None, &|i| world.course_skips(i))
    };
    h.map_or((t, [0.0; 4], [0.0; 4]), |h| (t, h.pos, h.normal))
}

/// The rollers among the rider's broadphase pairs kicked by its body (`roller::rider_kick`, in
/// `Boarder_InstanceCollisions` 0x125088 after the touch objects). STANDIN: the pairs are the rollers whose boxes
/// meet the body's, for the race's broadphase (rider+0x700). `body` reads the body as the kicks so far moved it.
pub(super) fn kick_rollers<E: crate::walls::WallEnv + crate::roller::RollerEnv>(course: &crate::course_ground::CourseGround, wr: &mut crate::walls::WallRider, id: u32, env: &mut E, body: fn(&E) -> crate::collbody::CollBody) {
    let start = body(env);
    let mut rollers = std::mem::take(&mut course.knock.lock().unwrap_or_else(|e| e.into_inner()).rollers);
    let (c, rad) = (start.center, start.radius);
    let (lo, hi) = ([c[0] - rad, c[1] - rad, c[2] - rad, 0.0], [c[0] + rad, c[1] + rad, c[2] + rad, 0.0]);
    for k in rollers.iter_mut().filter(|k| crate::knockables::overlap(k.body.lo, k.body.hi, lo, hi)) {
        let b = body(env);
        crate::roller::rider_kick(&mut k.body, k.object, wr, id, &b, env);
    }
    // the rollers the kicks started go first, as `knockables` keeps them (newest first)
    let mut kn = course.knock.lock().unwrap_or_else(|e| e.into_inner());
    let mut new = std::mem::take(&mut kn.rollers);
    new.append(&mut rollers);
    kn.rollers = new;
}

/// What a rider's rollers ask of the world while the rider kicks them (`roller::rider_kick`).
impl crate::roller::RollerEnv for WallGlue<'_> {
    fn ray_cast(&mut self, q: &crate::sphtree::WorldSphTree) -> (f32, V4, V4) {
        roller_cast(self.world, self.course, q, false)
    }
    fn query_instances(&mut self, q: &crate::sphtree::WorldSphTree) -> (f32, V4, V4) {
        roller_cast(self.world, self.course, q, true)
    }
    /// STANDIN: nothing, for the race's broadphase (`Broadphase_MoveBox` 0x14fe68).
    fn move_box(&mut self, _: V4, _: V4) {}
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.course.ssf.event_script(object as usize, event as usize)
    }
    fn start_script(&mut self, script: i32, object: u32) {
        self.starts.push((script, object));
    }
    /// `Sfx_InstanceCollision` 0x216c50 (the rider's hit on a roller).
    fn sfx(&mut self, _: u32, object: u32) {
        let v = to_game(self.r.vel, 100.0, 0.0);
        self.course.knock.lock().unwrap_or_else(|e| e.into_inner()).knocks.push((object, dot(v, v).sqrt()));
    }
}

/// What `Boarder_WallCollide` calls (`walls::WallEnv`), answered on the course and the port's rider.
struct WallGlue<'a> {
    r: &'a mut Rider,
    g: &'a mut gm::Rider,
    world: &'a CollisionWorld,
    course: &'a crate::course_ground::CourseGround,
    body: crate::collbody::CollBody,
    /// the sphere tree the last object query hit (the record's +0x30)
    tree: Option<(usize, usize)>,
    /// scripts the rollers the rider hit started (`cScriptNode`(3, script, object))
    starts: Vec<(i32, u32)>,
}

impl crate::walls::WallEnv for WallGlue<'_> {
    /// `World_RayCast` 0x25aff8 with `cWorldBoarder`(the body, 1), its ground normal set (+0x2e0, +0x2d0 = 1).
    fn cast_walls(&mut self, r: &crate::walls::WallRider) -> (f32, crate::walls::CastRecord) {
        wall_cast(self.world, self.course, &self.body, r.ground)
    }
    /// [`touch_query`]; the tree kept for the contact's record.
    fn query_objects(&mut self, _: &crate::walls::WallRider) -> (f32, crate::walls::CastRecord, crate::walls::Touched) {
        let (t, rec, touched, tree) = touch_query(self.world, self.course, &self.body);
        self.tree = tree;
        (t, rec, touched)
    }
    /// `Boarder_AccumulateImpact` 0x125c48: +0x2e4 += 1 − max(n · +0x2f0, 0), +0x2f0 = n.
    fn accumulate_impact(&mut self, _: &mut crate::walls::WallRider, n: V4) {
        accumulate_impact(&mut self.r.impact, &mut self.r.impact_n, n);
    }
    /// `Boarder_Translate` 0x1267a0: `CollBody_Translate` 0x236448 on the body (the broadphase box, +0x6e0/+0x6f0, the
    /// port does not keep).
    fn translate(&mut self, _: &mut crate::walls::WallRider, by: V4) {
        self.body.translate(by);
    }
    /// `Boarder_TurnFromWall` 0x1257a0 (`orient::turn_from_wall`) on the game-side rider's attitude.
    fn turn_from_wall(&mut self, _: &mut crate::walls::WallRider, n: V4) {
        let g = &mut *self.g;
        let mut o = Orient { q: g.quat, fwd: g.p.fwd, side: g.board_side, up: g.board_up, yaw: g.yaw };
        orient::turn_from_wall(&mut o, n);
        (g.quat, g.p.fwd, g.board_side, g.board_up, g.yaw) = (o.q, o.fwd, o.side, o.up, o.yaw);
    }
    /// `Boarder_Wipeout` 0x11d830 with the contact: the port's wipe-out (not inside the Megaplex tube, as the
    /// ground tick's; the side the wall was on is the crash's kind, 2 or 3).
    fn wipeout(&mut self, r: &mut crate::walls::WallRider, c: &crate::walls::Contact) {
        if self.r.tube_class != u8::MAX {
            return;
        }
        self.r.pos = to_port(r.pos, 0.01);
        self.r.vel = to_port(r.vel, 0.01);
        let n = to_port(c.normal, 1.0);
        self.r.crash_kind = if n.dot(Vec3::new(self.r.yaw.cos(), 0.0, -self.r.yaw.sin())) > 0.0 { 2 } else { 3 };
        self.r.cause = "wall";
        self.r.crash_at = Some((to_port(c.pos, 0.01), n));
        self.r.wipe_out();
    }
    /// `Rider::go_down` does `Score_Crash` and `Boarder_AddMeter` itself.
    fn score_crash(&mut self, _: &mut crate::walls::WallRider) -> f32 {
        0.0
    }
    fn add_meter(&mut self, _: &mut crate::walls::WallRider, _: f32) {}
    /// `Boarder_Stumble` 0x124dd0: the port's stumble.
    fn stumble(&mut self, _: &mut crate::walls::WallRider, _: V4, _: u32) {
        self.r.stumble_by(true);
    }
    /// `Boarder_GetSpeedStat` 0x11d7b0.
    fn speed_stat(&mut self, _: &crate::walls::WallRider) -> f32 {
        self.r.speed_stat()
    }
    fn knock(&mut self, _: &mut crate::walls::WallRider, _: f32) {}
    /// `Instance_OnPlayerContact` 0x13bd40 (`knockables::Knockables::contact`).
    fn contact(&mut self, object: u32, r: &mut crate::walls::WallRider, c: &crate::walls::Contact, rec: &crate::walls::CastRecord) {
        let rider = crate::knockables::Rider { id: rider_id(self.r), human: !self.r.ai, vel: r.vel };
        let record = crate::knockables::record_context(rec.pos, rec.normal, self.tree);
        let mut k = self.course.knock.lock().unwrap_or_else(|e| e.into_inner());
        k.contact(self.course, object, rider, c, record);
    }
    /// `Sfx_InstanceCollision` 0x216c50: the object and the rider's speed, for the app's sound.
    fn sfx_object(&mut self, r: &crate::walls::WallRider, object: u32) {
        let speed = dot(r.vel, r.vel).sqrt();
        self.course.knock.lock().unwrap_or_else(|e| e.into_inner()).knocks.push((object, speed));
    }
    /// `Sfx_OnWallHit` 0x2162f0: the port's wall sound takes the hardest knock of the tick.
    fn sfx_wall(&mut self, r: &crate::walls::WallRider) {
        let (at, speed) = (to_port(r.pos, 0.01), to_port(r.vel, 0.01).length());
        if self.r.wall_hit.is_none_or(|w| w.1 < speed) {
            self.r.wall_hit = Some((at, speed));
        }
    }
}

/// What the ground tick calls, answered by the port.
struct GroundEnv<'a> {
    r: &'a mut Rider,
    world: &'a CollisionWorld,
    /// `Boarder_TakeOff` ran
    left: bool,
    /// the jump stat byte (stats+0x17)
    jump_stat: u8,
}

impl GroundEnv<'_> {
    /// The port's rider takes over what the game-side one has now (before a call that works on the port's).
    fn give(&mut self, g: &gm::Rider) {
        self.r.pos = to_port(g.p.pos, 0.01);
        self.r.vel = to_port(g.vel, 0.01);
        self.r.yaw = wrap(g.yaw - HALF_PI);
        self.r.normal = to_port(g.p.normal, 1.0);
    }
}

impl gm::Env for GroundEnv<'_> {
    /// `Boarder_SetState` 0x11f608 through `states::set_state` (row G18a): the old control state's exit routine and
    /// the new one's enter routine (`states::run_routine`) on the game-side rider's fields (the revert, state 4, is
    /// `ChangeFakieControl_Enter`: the board turned half round about its own up, switch toggled, steer and lean
    /// negated). What the routines call (the boost, clips, the animation's layers) and the fields the game-side
    /// rider does not keep are left out (no animation controller here).
    fn set_state(&mut self, g: &mut gm::Rider, state: u32) {
        use crate::states::{self as st, Change};
        let mut cur = g.state;
        for change in st::set_state(&mut cur, state) {
            let (addr, a1) = match change {
                Change::Exit(f) => (f.addr, state),
                Change::Enter(f, old) => (f.addr, old),
                Change::ClearResetTimer => continue,
            };
            let mut sr = st::StateRider {
                vel: g.vel,
                orient: Orient { q: g.quat, fwd: g.p.fwd, side: g.board_side, up: g.board_up, yaw: g.yaw },
                stance: g.switch,
                x1fc: [g.brake, 0.0, 0.0, g.crouch, 0.0, 0.0],
                steer: g.steer,
                lean: g.p.x244,
                class: g.class,
                state: cur,
                stats40: g.stats.stance,
                ..Default::default()
            };
            let mut obj = st::StateObj::default();
            st::run_routine(addr, &mut sr, &mut obj, a1);
            g.quat = sr.orient.q;
            g.p.fwd = sr.orient.fwd;
            g.board_side = sr.orient.side;
            g.board_up = sr.orient.up;
            g.yaw = sr.orient.yaw;
            g.switch = sr.stance;
            g.brake = sr.x1fc[0];
            g.crouch = sr.x1fc[3];
            g.steer = sr.steer;
            g.p.x244 = sr.lean;
        }
        g.state = cur;
    }

    /// `Boarder_WallCollide` 0x126250 (`walls::wall_collide`) on the course's collision (row E7c3c), with the
    /// body [`wall_body`] gives; without the course's files (a folder level) the port's sphere against the drawn
    /// mesh ([`Rider::walls`], STANDIN). The air's walls (row E8o) are still the drawn mesh's.
    fn wall_collide(&mut self, g: &mut gm::Rider) -> bool {
        if let Some(course) = self.world.course.clone() {
            let mut wr = crate::walls::WallRider { pos: g.p.pos, vel: g.vel, ground: g.p.normal, knocks: self.r.knocks, mode: 2, state: g.state, up: g.board_up };
            let body = wall_body(g);
            let mut glue = WallGlue { r: &mut *self.r, g: &mut *g, world: self.world, course: &course, body, tree: None, starts: Vec::new() };
            let hit = crate::walls::wall_collide(&mut wr, &mut glue);
            self.r.knocks = wr.knocks;
            g.p.pos = wr.pos;
            g.vel = wr.vel;
            self.r.pos = to_port(g.p.pos, 0.01);
            self.r.vel = to_port(g.vel, 0.01);
            return hit;
        }
        self.give(g);
        let mut p = self.r.pos;
        let (pushed, hit) = self.r.walls(self.world, &mut p);
        if pushed {
            self.r.pos = p;
            g.p.pos = to_game(p, 100.0, 1.0);
            g.vel = to_game(self.r.vel, 100.0, 0.0);
        }
        hit
    }

    /// `cWorldLine::cWorldLine`(0.5, from, to, 2) and `World_RayCast` 0x25aff8 on the course (row E7c3), with the
    /// rider's last-patch record; nothing when the distance is negative.
    ///
    /// STANDIN (without the course's files, a folder level): a vertical query of the drawn mesh 2 m over to 1 m
    /// under the probe point (a third of the way up the line).
    fn ray_cast(&mut self, _g: &mut gm::Rider, from: V4, to: V4) -> Option<Hit> {
        if let Some(course) = &self.world.course {
            let mut q = crate::worldline::WorldLine::new(0.5, from, to, 2);
            let world = self.world;
            let (t, h) = course.ray_cast(&mut q, world.showoff, Some(&mut self.r.last_hit), &|i| world.course_skips(i));
            let h = h.filter(|_| t >= 0.0)?;
            return Some(Hit { kind: h.kind, point: h.pos, normal: h.normal, surface: h.surface as u32 });
        }
        let top = [from[0] + (to[0] - from[0]) / 3.0, from[1] + (to[1] - from[1]) / 3.0, from[2] + (to[2] - from[2]) / 3.0, 1.0];
        let at = to_port(top, 0.01);
        let hit = self.world.ground(at, 2.0, 1.0)?;
        Some(Hit {
            kind: 0,
            point: to_game(Vec3::new(at.x, hit.y, at.z), 100.0, 1.0),
            normal: to_game(hit.normal, 1.0, 0.0),
            surface: hit.surface as u32,
        })
    }

    /// `Boarder_TakeOff` 0x128470 (`crate::orient::take_off`).
    fn take_off(&mut self, g: &mut gm::Rider) {
        self.left = true;
        let mut tr = TakeOffRider { state: g.state, crouch: g.crouch, x208: g.crouch };
        let mut glue = TakeOffGlue { g, jump_stat: self.jump_stat };
        orient::take_off(&mut tr, &mut glue);
    }

    /// STANDIN: effects only, the spray burst is kept in `crash_spray`, for `cBoarder::EmitBrakeSpray` 0x11a798 (row G19).
    fn crash_virtual(&mut self, _g: &mut gm::Rider, pos: V4, vel: V4, normal: V4) {
        self.r.crash_spray = Some((to_port(pos, 0.01), to_port(vel, 0.01), to_port(normal, 1.0)));
    }

    /// `Boarder_Wipeout` 0x11d830 with its record. Inside the Megaplex tube the walls and glass only steer the rider
    /// (STANDIN: our tube, for the course's script, row E9). `Rider::go_down` also does `Score_Crash` 0x156de0 and
    /// `Boarder_AddMeter` 0x11b018, so those two calls below do nothing.
    fn wipeout(&mut self, g: &mut gm::Rider, rec: &gm::CrashRecord) {
        if self.r.tube_class != u8::MAX {
            return;
        }
        self.give(g);
        self.r.cause = "surface";
        self.r.crash_kind = 0;
        self.r.crash_at = Some((to_port(rec.pos, 0.01), to_port(rec.normal, 1.0)));
        self.r.wipe_out();
    }

    fn score_crash(&mut self, _g: &mut gm::Rider) -> f32 {
        0.0
    }

    fn add_meter(&mut self, _g: &mut gm::Rider, _amount: f32) {}

    /// STANDIN: nothing while riding, for `Rail_TryCapture` 0x125cb8 (row G1b); the port captures a rail from the air.
    fn rail_try_capture(&mut self, _g: &mut gm::Rider) {}

    /// `Boarder_InstanceCollisions` 0x125088 on the course (row E8c3): the touch objects (`walls::instance_collisions`),
    /// then the rider's rollers (`roller::rider_kick`).
    /// STANDIN: the rider's pairs are the rollers whose boxes meet the body's, for the race's broadphase (rider+0x700);
    /// the pairs' other nodes (types 0x100..0x103: the boost pads) are `Rider::apply_pads`'. The body is the bind
    /// pose's ([`Rider::bind_body`], mask −1, row G5). Without the course's files: nothing.
    fn instance_collisions(&mut self, g: &mut gm::Rider) {
        let Some(course) = self.world.course.clone() else { return };
        let mut wr = crate::walls::WallRider { pos: g.p.pos, vel: g.vel, ground: g.p.normal, knocks: self.r.knocks, mode: 2, state: g.state, up: g.board_up };
        let body = bind_body_at(g.p.pos, g.p.fwd, g.board_up, g.p.stat60);
        let id = rider_id(self.r);
        let mut glue = WallGlue { r: &mut *self.r, g: &mut *g, world: self.world, course: &course, body, tree: None, starts: Vec::new() };
        crate::walls::instance_collisions(&mut wr, &mut glue);
        kick_rollers(&course, &mut wr, id, &mut glue, |g| g.body);
        let starts = std::mem::take(&mut glue.starts);
        course.knock.lock().unwrap_or_else(|e| e.into_inner()).start_scripts(&course, &starts);
        self.r.knocks = wr.knocks;
        g.p.pos = wr.pos;
        g.vel = wr.vel;
        self.r.pos = to_port(g.p.pos, 0.01);
        self.r.vel = to_port(g.vel, 0.01);
    }
}

/// What `Boarder_TakeOff` calls, for a rider that leaves the snow without a jump.
struct TakeOffGlue<'a> {
    g: &'a mut gm::Rider,
    jump_stat: u8,
}

impl TakeOffEnv for TakeOffGlue<'_> {
    /// STANDIN: nothing, for `Boarder_SetMotion` 0x11f610's body (row G18); the port's `take_off` starts the air.
    fn set_motion(&mut self, _r: &mut TakeOffRider, _motion: u32) {}

    /// `Jump_ApplyImpulse` 0x1284e0 (`crate::jump::jump_impulse`) on the game-side rider.
    fn jump_impulse(&mut self, r: &mut TakeOffRider, min: f32) {
        let js = JumpState {
            crouch: r.crouch,
            x208: r.x208,
            vel: self.g.vel,
            jump_stat: self.jump_stat,
            motion: 0,
            up: self.g.board_up,
            normal: self.g.p.normal,
            forward: self.g.p.plane_fwd,
        };
        self.g.vel = jump::jump_impulse(min, &js).0;
    }

    /// Not reached from here (state 0xa, the jump control, is the port's own jump).
    fn takeoff_spin_rates(&mut self, _r: &mut TakeOffRider) {}

    /// STANDIN: nothing, for `Boarder_SetState` 0x11f608's body (row G18).
    fn set_state(&mut self, _r: &mut TakeOffRider, _state: u32) {}
}

/// Everything `RailSlideMotion_Update` 0x10b0d0 reads and writes (row G20c2c4), in the game's space (cm, z up).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailMotion {
    /// rider+0x140, +0x150
    pub pos: V4,
    pub vel: V4,
    /// rider+0x170..+0x1b0: the attitude and its axes (`side` +0x190, `up` +0x1a0)
    pub orient: Orient,
    /// rider+0x4a60..: the board's frame rows; +0x4a90 its position
    pub frame: [V4; 3],
    pub board_pos: V4,
    /// rider+0x1dc
    pub kind: u32,
    /// rider+0x284
    pub blend: f32,
    /// rider+0x2a0
    pub normal: V4,
    /// rider+0x2d0, +0x290: the last rail hit
    pub hit_point: V4,
    pub hit_word: u32,
    /// rider+0x12c: the tick's length in 60ths of a second
    pub ticks: f32,
    /// rider+0x130: the push along the rail (from the stick, × ±2450.8232)
    pub push: f32,
    /// rider+0x214 (the steer), +0x218, +0x21c
    pub steer: f32,
    pub x218: f32,
    pub x21c: f32,
    /// the motion object's +8 (lean angle) and +0xc (seconds on the rail)
    pub lean: f32,
    pub time: f32,
}

/// How a rail tick ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailEnd {
    /// still on: rider+0x494 = 3, `Boarder_InstanceCollisions`, `Boarder_WallCollide`(rider), +0x494 = −1
    OnRail,
    /// off (no rail in reach, or outside the fall-off limits after the push away): +0x214, +0x218, +0x21c = 0, then
    /// `Score_StartTrickChain`(rider+0x5820, rider+0x1b4 ≠ stats+0x40, +0x1dc, 0, 0) with its result to
    /// `Boarder_AddMeter`, `Takeoff_Setup`, `Boarder_PlayTakeoffClip`, `Boarder_SetMotion`(1), the state manager's
    /// +0x798, +0x76c, +0x770 = 0 and `Boarder_SetState`(0x10)
    TakeOff,
}

/// The world's pull along a rail (0x3bbf00, set once: (0, 0, −980, 0)).
const RAIL_GRAVITY: V4 = [0.0, 0.0, -980.0, 0.0];

/// `RailSlideMotion_Update` 0x10b0d0, all of it (rows G20c2c, G20c2c2, G20c2c4). With dt = rider+0x12c / 60, the
/// unit velocity and the rail's closest point to the board ([`RailWorld::closest_point`]; none: take off):
/// - the hit recorded; the travel tangent `t` ([`rail_travel`]); k = 0.5 under 555.55554 cm/s, else speed × 0.0009;
/// - outside the fall-off limits (see [`rail_slide_normal`]): pushed 277.77777 cm/s away along the unit offset to the
///   rail, or the tangent when they are within 60° of each other, first made square to the velocity when that runs
///   against it; then take off;
/// - inside: the time moves on dt; the position moves along the board's up by its offset to the rail that way
///   (clamped to ±8.333333); the velocity eased to its part along `t` by dt·30; gravity along `t` (and the push
///   +0x130 · ±2450.8232, the sign of the velocity along `t`) by dt into the velocity, which moves the position by
///   dt; the balance: steer (negated unless the board is upside down) · (k clamped 0.5..2 · 145.07674) − lateral
///   (clamped ±20 when the gain is not negative) · gain · clamp(1/k², 1..4), the gain −5 for the first 0.6 s or on
///   a steep rail, else 1.8153523, and at most −1 within 2.5000005 of the rail; upside down within 20 of the rail
///   at least 290.15347 back toward it; along the board's right by dt; then the lean angle, normal
///   ([`rail_normal`]) and attitude ([`rail_attitude`], [`orient::update_orientation`]).
pub fn rail_slide_motion(world: &RailWorld, m: &mut RailMotion) -> RailEnd {
    let vdir = unit(m.vel);
    let dt = m.ticks * f32::from_bits(0x3c88_8889);
    let Some(hit) = world.closest_point(m.board_pos) else { return take_off(m) };
    m.hit_word = hit.word;
    m.hit_point = hit.point;
    let t = if dot(vdir, hit.tangent) < 0.0 { scale(hit.tangent, -1.0) } else { hit.tangent };
    let speed = dot(m.vel, m.vel).sqrt();
    let k = if speed < f32::from_bits(0x440a_e38e) { 0.5 } else { speed * f32::from_bits(0x3a6b_edfb) };
    let k2 = k * k;
    let d = sub(hit.point, m.board_pos);
    let off = add(d, scale(m.frame[1], -30.0));
    let (ox, oy, oz) = (dot(m.frame[0], off), dot(m.frame[1], off), dot(m.frame[2], off));
    if m.time < 0.6 {
        m.blend = 1.0;
    }
    let b = m.blend;
    let rest = 1.0 - b;
    let inside = ox.abs() < b * 150.0 + rest * 80.0 && oy.abs() < b * 50.0 + rest * 50.0 && oz.abs() < b * 50.0 + rest * 30.0;
    if !inside {
        let dn = unit(d);
        let mut p = if 0.5 < dot(dn, t).abs() { t } else { dn };
        if dot(m.vel, p) < 0.0 {
            let vn = unit(m.vel);
            p = unit(sub(p, scale(vn, dot(p, vn))));
        }
        m.vel = sub(m.vel, scale(p, f32::from_bits(0x438a_e38e)));
        return take_off(m);
    }
    let lat = dot(m.orient.side, d);
    m.time += dt;
    let h = dot(d, m.orient.up);
    let lim = f32::from_bits(0x4105_5555);
    let h = if h < -lim { -lim } else if lim < h { lim } else { h };
    m.pos = add(m.pos, scale(m.orient.up, h));
    let vt = dot(m.vel, t);
    let along = scale(t, vt);
    let ease = dt * 30.0;
    m.vel = add(scale(along, ease), scale(m.vel, 1.0 - ease));
    let mut f = sub(scale(t, dot(RAIL_GRAVITY, t)), scale(along, 0.0));
    if 0.0 < m.push {
        let p = m.push * if 0.0 < vt { f32::from_bits(0x4519_2d2c) } else { f32::from_bits(0xc519_2d2c) };
        f = add(f, scale(t, p));
    }
    m.vel = add(m.vel, scale(f, dt));
    m.pos = add(m.pos, scale(m.vel, dt));
    let kk = if k < 0.5 { 0.5 } else if 2.0 < k { 2.0 } else { k };
    let st = if m.orient.up[2] < 0.0 { m.steer } else { -m.steer };
    let mut force = st * (kk * f32::from_bits(0x4311_13a5));
    let steep = STEEP_RAIL < t[2].abs();
    let mut gain = if m.time < 0.6 || steep { -5.0 } else { f32::from_bits(0x3fe8_5d77) };
    if lat.abs() < f32::from_bits(0x4020_0002) {
        gain = if -1.0 < gain { -1.0 } else { gain };
    }
    let inv = 1.0 / k2;
    let mm = if inv < 1.0 { 1.0 } else if 4.0 < inv { 4.0 } else { inv };
    let l = if 0.0 <= gain { if lat < -20.0 { -20.0 } else if 20.0 < lat { 20.0 } else { lat } } else { lat };
    force = force - l * (gain * mm);
    if m.orient.up[2] < 0.0 && lat.abs() < 20.0 {
        let cap = f32::from_bits(0x4391_13a5);
        if 0.0 < lat {
            if -cap < force {
                force = -cap;
            }
        } else if force < cap {
            force = cap;
        }
    }
    m.pos = add(m.pos, scale(m.orient.side, force * dt));
    m.lean = rail_lean_angle(m.lean, lat, m.kind, m.orient.up[2]);
    m.normal = rail_normal(t, m.orient.up, m.lean);
    m.orient.q = rail_attitude(m.orient.q, m.normal, t, m.lean);
    orient::update_orientation(&mut m.orient);
    RailEnd::OnRail
}

fn take_off(m: &mut RailMotion) -> RailEnd {
    m.x21c = 0.0;
    m.steer = 0.0;
    m.x218 = 0.0;
    RailEnd::TakeOff
}

/// What `Rail_TryCapture` 0x125cb8 reads, in the game's space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaptureIn {
    /// rider+0x150: the velocity
    pub vel: V4,
    /// rider+0x4a90 and the frame rows +0x4a60, +0x4a70, +0x4a80
    pub board_pos: V4,
    pub frame: [V4; 3],
    /// rider+0x284: the fall-off blend
    pub blend: f32,
}

/// A rail caught: the new velocity (rider+0x150), the rail kind (rider+0x1dc: 1 along, 2 backwards, 3 and 4
/// across) and the rail's word (rider+0x290, and the sound manager's +0x6e40). The game then calls
/// `Score_LandTrick` 0x1569b0(rider+0x5820, kind, stance +0x1b4, 0), `Boarder_AddMeter` with its result,
/// `Boarder_SetMotion`(3) and `Boarder_SetState`(0xf).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capture {
    pub vel: V4,
    pub kind: u32,
    pub word: u32,
}

/// `Rail_TryCapture` 0x125cb8: the rail's closest point to the board ([`RailWorld::closest_point`]); the board must
/// not have passed it (offset · travel direction ≥ −0.2, the offset taken 30 cm along the frame's y row as in
/// [`rail_slide_normal`]), nor be in a clip of type 0xe, 0xf or 0x10 without flag 4 (`clip_type`, asked up to three
/// times as the game asks `AnimCtl_GetClipType`, and `clip_flag4`, `AnimCtl_TestClipFlag`(4, 0)), and must be
/// within 0.9 of the fall-off limits. Then the velocity goes along the travel tangent at the speed, at most the
/// speed along it or 555.55554 if that is more, its z × 0.1; the kind from the angle of the tangent in the board's
/// frame (atan2(z row · t, x row · t)): beyond 150° 2, under −30° 4, over 30° 3, else 1.
pub fn rail_try_capture(world: &RailWorld, i: &CaptureIn, clip_type: &mut dyn FnMut() -> u32, clip_flag4: &mut dyn FnMut() -> bool) -> Option<Capture> {
    let vdir = unit(i.vel);
    let hit = world.closest_point(i.board_pos)?;
    let t = if dot(vdir, hit.tangent) < 0.0 { scale(hit.tangent, -1.0) } else { hit.tangent };
    let off = add(sub(hit.point, i.board_pos), scale(i.frame[1], -30.0));
    if dot(off, vdir) < -0.2 {
        return None;
    }
    let ct = clip_type();
    let in_trick = ct == 0xe || clip_type() == 0xf || clip_type() == 0x10;
    if in_trick && !clip_flag4() {
        return None;
    }
    let b = i.blend;
    let rest = 1.0 - b;
    let k = f32::from_bits(0x3f66_6666);
    let ox = dot(i.frame[0], off);
    let oy = dot(i.frame[1], off);
    let oz = dot(i.frame[2], off);
    if !(ox.abs() < (b * 150.0 + rest * 80.0) * k && oy.abs() < (b * 50.0 + rest * 50.0) * k && oz.abs() < (b * 50.0 + rest * 30.0) * k) {
        return None;
    }
    let along = dot(i.vel, t);
    let cap = if f32::from_bits(0x440a_e38e) < along { along } else { f32::from_bits(0x440a_e38e) };
    let speed = dot(i.vel, i.vel).sqrt();
    let s = if cap < speed { cap } else { speed };
    let mut vel = scale(t, s);
    vel[2] *= 0.1;
    let x = dot(i.frame[0], t);
    let y = dot(i.frame[2], t);
    let a = crate::jump::atan2_inline(y, x);
    let (wide, narrow) = (f32::from_bits(0x4027_8d36), f32::from_bits(0x3f06_0a92));
    let kind = if !(a.abs() <= wide) {
        2
    } else if !(-narrow <= a) {
        4
    } else if !(a <= narrow) {
        3
    } else {
        1
    };
    Some(Capture { vel, kind, word: hit.word })
}

/// `Boarder_AccumulateImpact` 0x125c48: +0x2e4 += 1 − max(n · +0x2f0, 0), +0x2f0 = n.
pub(super) fn accumulate_impact(impact: &mut f32, last: &mut V4, n: V4) {
    let d = dot(n, *last);
    *impact += if 0.0 < d { 1.0 - d } else { 1.0 };
    *last = n;
}

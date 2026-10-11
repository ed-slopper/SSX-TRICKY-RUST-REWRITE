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
//! - rail capture (row G1b) and `Boarder_InstanceCollisions` 0x125088 (row E8b): not done while riding, as before

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

/// A port vector (m, y up) as a game vector (cm, z up); `w` is 1 for a point, 0 for a direction.
fn to_game(v: Vec3, k: f32, w: f32) -> V4 {
    [v.x * k, -v.z * k, v.y * k, w]
}

/// A game vector (cm, z up) as a port vector (m, y up) (`k` = 0.01), or a direction (`k` = 1).
fn to_port(v: V4, k: f32) -> Vec3 {
    Vec3::new(v[0] * k, v[2] * k, -v[1] * k)
}

fn unit(v: V4) -> V4 {
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

/// A rotation matrix with columns `x`, `y`, `z` as a unit quaternion (w, x, y, z).
fn quat_of(x: V4, y: V4, z: V4) -> V4 {
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
    /// The game-side rider from the port's: the board lies on the ground (its up is the ground's normal) and points
    /// along the port's yaw. STANDIN: the game's attitude quaternion (rider+0x170) is only kept between ticks, not
    /// across a landing or a respawn, where the port has no such thing (rows G1 and G5).
    fn seed_ground(&self) -> gm::Rider {
        let n = unit(to_game(self.normal, 1.0, 0.0));
        let psi = self.yaw + HALF_PI;
        let h = [psi.cos(), psi.sin(), 0.0, 0.0];
        let fwd = unit(sub(h, scale(n, dot(h, n))));
        let side = cross(fwd, n);
        let mut o = Orient { q: quat_of(side, fwd, n), ..Orient::default() };
        orient::update_orientation(&mut o);
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
    pub(super) fn ground_tick(&mut self, world: &CollisionWorld, dt: f32, boost_level: f32) -> bool {
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
    /// STANDIN: in the air the normal (+0x2a0) is the port's own: the game's `AirMotion_Update` 0x108378 takes it from its
    /// down ray whenever that hits (0x108588, `World_RayCast` 0x25aff8, rows G20c2b and E7). On a rail it is
    /// `RailSlideMotion_Update`'s ([`rail_normal`]) and at a respawn `Boarder_PlaceAt`'s ([`placed_normal`], its ray
    /// hit apart), row G20c2.
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
    /// `Boarder_SetState` 0x11f608. Only the revert (state 4: ridden backwards, brake off) is done: the board swings
    /// half way round, switch toggles and the steer and the lean flip sign. STANDIN: our revert, for the state's body (row G18).
    fn set_state(&mut self, g: &mut gm::Rider, state: u32) {
        if state == 4 {
            let mut o = Orient { q: g.quat, fwd: g.p.fwd, side: g.board_side, up: g.board_up, yaw: g.yaw };
            orient::rotate_about_axis(&mut o, std::f32::consts::PI, g.p.normal);
            g.quat = o.q;
            g.p.fwd = o.fwd;
            g.board_side = o.side;
            g.board_up = o.up;
            g.yaw = o.yaw;
            g.switch ^= 1;
            g.steer = -g.steer;
            // `ChangeFakieControl_Enter` 0x103eb8 negates the lean value (+0x244) with the steer (+0x214)
            g.p.x244 = -g.p.x244;
        }
    }

    fn wall_collide(&mut self, g: &mut gm::Rider) -> bool {
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

    /// STANDIN: a vertical query of the drawn mesh 2 m over to 1 m under the probe point (a third of the way up the
    /// line), for `World_RayCast` along the ground's normal over the game's collision (row E7).
    fn ray_cast(&mut self, _g: &mut gm::Rider, from: V4, to: V4) -> Option<Hit> {
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

    /// STANDIN: nothing, for `Boarder_InstanceCollisions` 0x125088 (row E8b).
    fn instance_collisions(&mut self, _g: &mut gm::Rider) {}
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

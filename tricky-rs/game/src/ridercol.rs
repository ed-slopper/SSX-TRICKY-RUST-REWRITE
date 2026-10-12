//! `Boarder_RiderCollisions` 0x123fc8 (board row E8d4): what runs for each rider every tick to keep two riders from
//! overlapping and to hand out the hits. The pieces are checked elsewhere ([`crate::collbody`] for the spheres,
//! [`crate::hit`] for `Boarder_RiderHit`, the shove and the closing speeds); this is their driver, in the game's
//! order, checked in the function runner (`tests/ridercol.rs`) with every callee answered and logged.
//!
//! First every neighbour the rider's list holds (the objects in the broadphase around it, +0x700, riders only) whose
//! body overlaps this one's: the pair is pushed apart (0.55 of the overlap each, all of it for the one that is not
//! finished and stopped; none if both are), and, once every three ticks per pair, both get `Boarder_RiderHit` with
//! the closing speed split by their speed stats. Then every other rider of the race that this rider's layer-1 clip
//! (type 0x15, flag 1 set, flag 2 not) is shoving: [`crate::hit::shove`].

use crate::hit::{pair_speeds, shove, ShoveRider};
use crate::jump::{add, normalize, scale, sub};

type V4 = [f32; 4];

/// A rider's record of another (rider+0x40 + 0x20 × the other's index): +8 the tick of the last bump, +0xc of the
/// last shove, +0x1a how many times it has hit it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColRec {
    pub bump: i32,
    pub shove: i32,
    pub hits: u16,
}

/// What the driver reads and writes of a rider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColRider {
    /// +0x140 position, +0x150 velocity (cm, cm/s)
    pub pos: V4,
    pub vel: V4,
    /// +0x424: the motion (5 and 6 wiped out)
    pub motion: u32,
    /// +0x1e0 the shove heading; +0x4aa0 forward, +0x4ab0 side, +0x4ac0 up, +0x4ad0 where (the shove's frame)
    pub heading: f32,
    pub fwd: V4,
    pub side: V4,
    pub up: V4,
    pub at: V4,
    /// stats+0x12
    pub stability: u8,
    /// +0x40 + 0x20 × j
    pub rec: Vec<ColRec>,
}

/// Everything the driver asks of the rest of the game. Riders are named by their slot in the race.
pub trait ColEnv {
    /// `CollBody_OverlapSpheres`(a's body, b's body): the push (the first vector it writes) when they overlap
    fn overlap(&mut self, a: usize, b: usize) -> Option<V4>;
    /// `Boarder_IsFinishedAndStopped`
    fn finished_stopped(&mut self, r: usize) -> bool;
    /// `Boarder_Translate`(rider, vector)
    fn translate(&mut self, r: usize, v: V4);
    /// `Boarder_GetSpeedStat` (FUN_0011d7b0)
    fn speed_stat(&mut self, r: usize) -> f32;
    /// `Boarder_RiderHit`(speed, hit rider `to`, from rider `from`, the record `rec_owner`'s record of `rec_of`, dir)
    fn rider_hit(&mut self, speed: f32, to: usize, from: usize, rec_owner: usize, rec_of: usize, dir: V4) -> bool;
    /// `Boarder_ScoreKnockdown` 0x11e9a0, `Boarder_ScoreCrash` 0x11e988
    fn score_knockdown(&mut self, r: usize);
    fn score_crash(&mut self, r: usize);
    /// `Score_Crash`(rider+0x5820, 0) then `Boarder_AddMeter` of it on the rider
    fn crash_meter(&mut self, r: usize);
    /// `Rand_Game`, the result unused
    fn rand_game(&mut self);
    /// `Sfx_OnRiderCollision`(hitter, hit) and `Speech_OnRiderCollision`(hit)
    fn collision_sound(&mut self, hitter: usize, hit: usize);
    /// `AnimCtl_GetClipType`(rider+0x46c0, 1) and `AnimCtl_TestClipFlag`(…, layer, 1)
    fn clip_type1(&mut self, r: usize) -> u32;
    fn clip_flag(&mut self, r: usize, layer: u32) -> bool;
}

const FAR: u32 = 0x7149_f2ca;

/// `Boarder_RiderCollisions`(me): `list` the riders of me's neighbour list in order, `tick` the race's +0x18 and
/// `practice` the game mode (0x32de8c) being 0.
pub fn rider_collisions(me: usize, list: &[usize], riders: &mut [ColRider], tick: i32, practice: bool, env: &mut dyn ColEnv) {
    for &o in list {
        let Some(mut v) = env.overlap(me, o) else { continue };
        let fm = env.finished_stopped(me);
        let fo = env.finished_stopped(o);
        if !fm {
            if !fo {
                v = scale(v, 0.55);
            }
            let neg = scale(v, -1.0);
            env.translate(me, neg);
            riders[me].pos = sub(riders[me].pos, v);
            if !fo {
                env.translate(o, v);
                riders[o].pos = add(riders[o].pos, v);
            }
        } else if !fo {
            env.translate(o, v);
            riders[o].pos = add(riders[o].pos, v);
        }
        if tick - 3 < riders[me].rec[o].bump {
            continue;
        }
        riders[me].rec[o].bump = tick;
        riders[o].rec[me].bump = tick;
        let mut sm = env.speed_stat(me);
        let mut so = env.speed_stat(o);
        if env.finished_stopped(me) {
            sm = f32::from_bits(FAR);
        }
        if env.finished_stopped(o) {
            so = f32::from_bits(FAR);
        }
        let n = normalize(v);
        let (to_o, to_me) = pair_speeds(n, riders[me].vel, riders[o].vel, sm, so);
        let hit1 = env.rider_hit(to_o, me, o, me, o, n);
        let hit2 = env.rider_hit(to_me, o, me, o, me, n);
        if hit2 {
            if !hit1 {
                riders[me].rec[o].hits = riders[me].rec[o].hits.wrapping_add(1);
                if riders[me].motion.wrapping_sub(5) >= 2 {
                    env.score_knockdown(me);
                    env.score_crash(o);
                }
            } else {
                env.crash_meter(o);
                env.crash_meter(me);
                env.rand_game();
            }
        } else if hit1 {
            riders[o].rec[me].hits = riders[o].rec[me].hits.wrapping_add(1);
            if riders[o].motion.wrapping_sub(5) >= 2 {
                env.score_knockdown(o);
                env.score_crash(me);
            }
        }
    }
    for i in 0..riders.len() {
        if i == me {
            continue;
        }
        if !(riders[i].rec[me].shove < tick - 3) || env.clip_type1(me) != 0x15 || tick - 3 < riders[me].rec[i].shove {
            continue;
        }
        if !env.clip_flag(me, 1) || env.clip_flag(me, 2) {
            continue;
        }
        let s = |r: &ColRider, stat: f32| ShoveRider { pos: r.pos, vel: r.vel, fwd: r.fwd, side: r.side, up: r.up, at: r.at, heading: r.heading, speed_stat: stat, stability: r.stability };
        let (sm, so) = (env.speed_stat(me), env.speed_stat(i));
        let Some((speed, n)) = shove(&s(&riders[me], sm), &s(&riders[i], so), practice) else { continue };
        if env.rider_hit(speed, i, me, i, me, n) {
            env.score_knockdown(me);
            env.score_crash(i);
        }
        env.collision_sound(me, i);
        riders[me].rec[i].hits = riders[me].rec[i].hits.wrapping_add(1);
        riders[i].rec[me].shove = tick;
        riders[me].rec[i].shove = tick;
    }
}

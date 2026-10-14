//! The computer rider's riding modes on the port's rider (board row G4e1): each tick the rider's place on its AI path
//! moves on (`Boarder_UpdatePathAhead` 0x118090, [`aipath::update_path_ahead`]), an [`aicomp::Ai`] is filled from the
//! port's [`Rider`] (the game's units: cm, z up), the current mode runs (`AIComputer_RideLine` 0x1375c8,
//! `_RideToJump` 0x137858, `_RideNearRider` 0x137dc0, `_RideAtRival` 0x137a18) with the course queries on the path's
//! events (`AIComputer_FindJumpMarker` 0x1392a8, `AIComputer_FindSpeedHint` 0x1393e0), and the pad word it gives is
//! read the way the cruise control reads a pad (`CruiseControl_Update` 0x102fc8, `control.rs`) into the port's
//! [`Input`].

use crate::aicomp::{self, Ai, GrudgeOf, Mode, ModeCall, PathEvent};
use crate::aipath::{self, Chooser, Follow, Path};
use crate::rider::{ground_tick::game_dir, Input, Rider};
use glam::Vec3;

type V4 = [f32; 4];

/// A stick field of the pad word: six signed bits at `shl` from the top, over 31 (as `control.rs` reads it)
fn stick(pad: u32, shl: u32) -> f32 {
    ((pad << shl) as i32 >> 26) as f32 / 31.0
}

/// The pad word of `cComputer::GetControls` 0x1384b8 as the port's input: bit 1 the jump, bit 3 the boost
/// (`Boarder_UpdateBoost`'s first button), the steer (bits 5..10), crouch (11..16) and brake (17..22) sticks.
/// The game's steer is the port's negated (`GroundEnv`: `g.steer = -self.steer`). STANDIN: the port's input is
/// digital where the crouch and brake are sticks (any amount is the button), for the game's analog ones (row G1).
pub fn input_of(pad: u32) -> Input {
    Input {
        steer: -stick(pad, 0x15),
        tuck: stick(pad, 0xf) > 0.0,
        brake: stick(pad, 9) > 0.0,
        jump: pad & 2 != 0,
        boost: pad & 8 != 0,
        ..Default::default()
    }
}

/// A course's AI paths as the game keeps them, from the level's form of its `.aip` (`level::AiFile`, folder or disc):
/// each point's offset as a unit direction and length, each event's file code as its place in the event table
/// (`aip::EVENT_TYPES`). STANDIN: the directions renormalised from the folder form's offsets, for the `.aip`'s own
/// (row G4e3 may keep the `.aip`).
pub fn paths_of(ai: &tricky_data::level::AiFile) -> Vec<Path> {
    ai.ai_paths
        .iter()
        .map(|p| Path {
            start: p.path_pos,
            min: [0.0; 3],
            max: [0.0; 3],
            points: p.path_points.iter().map(|o| { let l = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt(); (if l == 0.0 { [0.0; 3] } else { [o[0] / l, o[1] / l, o[2] / l] }, l) }).collect(),
            u3: p.u3 as u32,
            respawnable: p.respawnable as u32,
            events: p.path_events.iter().map(|e| (tricky_data::aip::EVENT_TYPES.iter().position(|t| *t == e.event_type).unwrap_or(0) as u32, e.event_value as u32, e.event_start, e.event_end)).collect(),
        })
        .map(|mut p| { bounds(&mut p); p })
        .collect()
}

/// The path's box (`cPath` +0x1c, +0x28) round its points
fn bounds(p: &mut Path) {
    let mut at = p.start;
    let (mut lo, mut hi) = (at, at);
    for (d, l) in &p.points {
        for k in 0..3 {
            at[k] += d[k] * l;
            lo[k] = lo[k].min(at[k]);
            hi[k] = hi[k].max(at[k]);
        }
    }
    p.min = lo;
    p.max = hi;
}

/// A port position (m, y up) as the game's (cm, z up)
fn game_pos(v: Vec3) -> V4 {
    [v.x * 100.0, -v.z * 100.0, v.y * 100.0, 1.0]
}

/// A computer rider's AI: the computer's mode and target (cComputer +0x868 speed, +0x86c skill, +0x890 target,
/// +0x89c mode) and the rider's place on its path.
#[derive(Clone, Debug)]
pub struct AiRider {
    pub skill: f32,
    pub speed: f32,
    pub target: i32,
    pub mode: Mode,
    pub follow: Follow,
    /// the calls the last tick's mode made (`AIComputer_PlanJumpTrick`, not ported here: row G4e3)
    pub calls: Vec<ModeCall>,
}

/// What a tick reads of the race: the riders' positions (+0x140) and velocities (+0x150) in the game's units, the
/// pair records of this rider (distance, bearing; `hit::pair_distances`), the race's tick, knockdown practice.
pub struct RaceView<'a> {
    pub me: i32,
    pub others: &'a [(V4, V4)],
    pub recs: &'a [[f32; 2]],
    pub tick: i32,
    pub practice: bool,
}

impl AiRider {
    /// On the path nearest `pos` (`AIP_SortLinesByDistance`), projected from its start, riding the line.
    pub fn new(paths: &[Path], pos: Vec3, skill: f32) -> AiRider {
        let p = game_pos(pos);
        let path = aipath::sort_by_distance(paths, p, 1, false).first().copied().unwrap_or(0);
        let mut follow = Follow { path, last: 0.0, along: 0.0, off: 0.0, near: [0.0; 4], ahead: [0.0; 4], heading: 0.0, cache: aipath::Cache::default() };
        let (d, off) = paths[path].project_from(p, &mut follow.near, &mut follow.cache);
        follow.along = d;
        follow.last = d;
        follow.off = off;
        follow.ahead = paths[path].position_at(d + 800.0).0;
        AiRider { skill, speed: 0.0, target: -1, mode: Mode::Line, follow, calls: Vec::new() }
    }

    /// One 60 Hz tick: the place on the path, then the mode's pad word as the port's input.
    /// STANDIN: no grudge (`AIComputer_IsRivalTarget` sees none), the path preference kept always (rider virtual
    /// +0x74 = 1), the place in the race 0 (rows G4e2, G4e3).
    pub fn tick(&mut self, r: &Rider, paths: &[Path], race: &RaceView, rand: &mut dyn FnMut() -> i32) -> Input {
        let pos = game_pos(r.pos);
        let vel = game_dir(r.vel * 100.0);
        let c = Chooser { pos, vel, x41c: 0, class: r.stats.kind as u32, place: 0, meter: r.boost, keep: 1.0 };
        aipath::update_path_ahead(paths, &mut self.follow, true, race.tick, &c, rand);
        let up = game_dir(r.normal);
        let fwd0 = game_dir(crate::rider::heading(r.yaw));
        let h = aicomp_dot(fwd0, up);
        let fwd = norm([fwd0[0] - up[0] * h, fwd0[1] - up[1] * h, fwd0[2] - up[2] * h, 0.0]);
        // the board's x = y × z (forward × up), its right
        let right = [fwd[1] * up[2] - fwd[2] * up[1], fwd[2] * up[0] - fwd[0] * up[2], fwd[0] * up[1] - fwd[1] * up[0], 0.0];
        let mut ai = Ai {
            skill: self.skill,
            speed: self.speed,
            target: self.target,
            mode: self.mode,
            // the frame's rows as `Boarder_UpdatePose` 0x129628 builds them: the board's x (its right), y (its nose), z;
            // `aicomp::Frame` names the first two fwd and side, and `steer_towards` is 0 for a point along the second
            frame: aicomp::Frame { pos, fwd: right, side: fwd, up },
            vel,
            line: self.follow.ahead,
            travel: self.follow.heading,
            heading: crate::rider::ground_tick::game_yaw(r.yaw),
            x348: self.follow.off,
            meter: r.boost,
            me: race.me,
            recs: race.recs.to_vec(),
            grudge: GrudgeOf::default(),
        };
        let events: Vec<PathEvent> = paths[self.follow.path].events.iter().map(|&(kind, value, start, end)| PathEvent { kind, value, start, end }).collect();
        let (from, at) = (self.follow.last, self.follow.along);
        let mut jump = |s: &mut f32| aicomp::find_jump_marker(&events, from, at, s);
        let mut hint = |s: &mut f32| aicomp::find_speed_hint(&events, from, at, s);
        let others = race.others;
        let other = |i: i32| others.get(i as usize).copied().unwrap_or(([0.0; 4], [0.0; 4]));
        let mut w = aicomp::World { jump_marker: &mut jump, speed_hint: &mut hint, other: &other, tick: race.tick, n: others.len() as i32, practice: race.practice };
        self.calls.clear();
        let pad = ai.ride(&mut w, 0, &mut self.calls);
        self.speed = ai.speed;
        self.target = ai.target;
        self.mode = ai.mode;
        input_of(pad)
    }
}

fn aicomp_dot(a: V4, b: V4) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(v: V4) -> V4 {
    let l = aicomp_dot(v, v).sqrt();
    if l == 0.0 { v } else { [v[0] / l, v[1] / l, v[2] / l, 0.0] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_word_reads_as_the_cruise_control_does() {
        // steer full one way (31 in bits 5..10), crouch full (bits 11..16), the jump and boost bits
        let pad = 31 << 5 | 31 << 11 | 2 | 8;
        let i = input_of(pad);
        assert_eq!(i.steer, -1.0);
        assert!(i.tuck && i.jump && i.boost && !i.brake);
        let i = input_of(0x3e_0000 | ((-31i32 as u32) & 0x3f) << 5);
        assert_eq!(i.steer, 1.0);
        assert!(i.brake && !i.tuck);
    }
}

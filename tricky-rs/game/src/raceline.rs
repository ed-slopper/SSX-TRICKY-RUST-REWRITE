//! The rider's progress on the race line (board row E1b2g7a): `Boarder_UpdateRaceLineProgress` 0x118298 (run by
//! `cBoarder::vf7` 0x117198 in a race, game mode 4), `Boarder_SelectRaceLine` 0x118610 and `AIP_FindNearestLines`
//! 0x198110 under it, on the course's race lines as the game keeps them (`aipath::RaceLines`, cm, z up). Checked
//! bit for bit in the function runner on every course's own lines (`tests/raceline.rs`).
//!
//! What it gives: the rider's distance to the finish (+0x374) and the best of it (+0x378), the race-line events
//! the rider is in (each passed to `cBoarder::vf10` 0x11a348, `control::event_bits`, the event bits of rider+0x388),
//! and one more `vf10` call for each bit of rider+0x384 no event of this tick carried (bit, value 0).

use crate::aicomp::{events_between, PathEvent};
use crate::aipath::{Path, RaceLines, RacePlace};
use crate::jump::{add, dot, scale, sub};

type V4 = [f32; 4];

/// What `Boarder_UpdateRaceLineProgress` reads and writes of the rider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceRider {
    /// position (rider+0x140) and velocity (+0x150), cm and cm/s
    pub pos: V4,
    pub vel: V4,
    /// laps left (rider+0x114)
    pub laps_left: i32,
    /// the line (+0x57a0), distance to the finish (+0x374), its best (+0x378), the projection's memory (+0x3b0)
    pub place: RacePlace,
    /// the event bits of the last tick (+0x384) and of this one (+0x388)
    pub events_prev: u32,
    pub events: u32,
    /// the computer rider's slot (+0x418); below 0 for a player, the only riders who change line
    pub ai: i32,
}

/// One race-line event as `cBoarder::vf10` gets it: type (bit of rider+0x388) and value.
pub type Event = [u32; 2];

/// `cEventPath::vf0` 0x1969b0 on line `line`: `cPath::vf0` 0x197910 from the place `from` to the finish to the
/// place `to` (its distance to the finish minus each): the events overlapping that stretch (none when the rider
/// went back).
fn events(lines: &RaceLines, line: usize, from: f32, to: f32, mask: &mut u32, out: &mut Vec<Event>) {
    let ev: Vec<PathEvent> =
        lines.lines[line].events.iter().map(|&(kind, value, start, end)| PathEvent { kind, value, start, end }).collect();
    let t = lines.to_finish[line];
    for k in events_between(&ev, t - from, t - to, mask) {
        out.push([ev[k].kind, ev[k].value]);
    }
}

/// `Boarder_UpdateRaceLineProgress` 0x118298. `frame` is the race's tick count (gApp+0x730 → +0x1c → +0x18);
/// `vf10` is the rider's `cBoarder::vf10` 0x11a348 (vtable +0x5c), called with the rider as it stands.
///
/// The events are those of the stretch the rider beat its best by this tick (or of the point of its best when it
/// did not), and on a new line those from the best to the place on it.
pub fn update_progress(lines: &RaceLines, r: &mut RaceRider, frame: i32, vf10: &mut dyn FnMut(&mut RaceRider, Event)) {
    let mut near: V4 = [0.0, 0.0, 0.0, 1.0];
    let line = r.place.line;
    let (along, off) = lines.lines[line].project_from(r.pos, &mut near, &mut r.place.cache);
    r.place.to_finish = lines.to_finish[line] - along;
    let mut best = r.place.best;
    if 0 < r.laps_left {
        // `AIPathSet_GetMainPath` 0x197ef0: a new lap when the rider is more than half the main line back
        let half = lines.to_finish[lines.main] * 0.5;
        if half < r.place.to_finish - r.place.best {
            r.place.best = r.place.to_finish;
            best = r.place.to_finish;
        }
    }
    let mut list: Vec<Event> = Vec::new();
    if r.place.to_finish < best {
        events(lines, line, best, r.place.to_finish, &mut r.events, &mut list);
        r.place.best = r.place.to_finish;
    } else {
        events(lines, line, best, best, &mut r.events, &mut list);
    }
    if r.ai < 0 {
        let old = r.place.line;
        if 500.0 < off && frame % 60 == 0 {
            select_race_line(lines, r, false);
        } else if lines.lines[line].length() - 200.0 < along {
            select_race_line(lines, r, true);
        } else if fork_between(lines, line, r.place.best, r.place.to_finish) {
            select_race_line(lines, r, false);
        }
        if r.place.line != old {
            r.events = 0;
            r.place.cache.index = -1;
            let line = r.place.line;
            let (along, _) = lines.lines[line].project_from(r.pos, &mut near, &mut r.place.cache);
            r.place.to_finish = lines.to_finish[line] - along;
            if r.place.to_finish < r.place.best {
                events(lines, line, r.place.best, r.place.to_finish, &mut r.events, &mut list);
                r.place.best = r.place.to_finish;
            }
        }
    }
    let mut seen = 0u32;
    for ev in list {
        seen |= 1 << (ev[0] & 31);
        vf10(r, ev);
    }
    let gone = !seen;
    for bit in 0..32 {
        if (gone & r.events_prev) >> bit & 1 != 0 {
            vf10(r, [bit, 0]);
        }
    }
}

/// `cEventPath::vf1` 0x1969d8: `cPath::vf1` 0x197b90 (a fork marker) from the place `from` to the finish to the
/// place `to`, along the line (a place below 0 is its start, or its end for `to`).
fn fork_between(lines: &RaceLines, line: usize, from: f32, to: f32) -> bool {
    let t = lines.to_finish[line];
    let a = if from < 0.0 { 0.0 } else { t - from };
    let b = if to < 0.0 { lines.lines[line].length() } else { t - to };
    lines.lines[line].fork_between(a, b)
}

/// v / |v| on VU0 (the sum of four squares, `vrsqrt`, `vmulq`); 1/√0 is the largest float.
fn normalise(v: V4) -> V4 {
    let s = dot(v, v);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    scale(v, q)
}

/// `Boarder_SelectRaceLine` 0x118610: of the three lines nearest the point 800 ahead along the velocity, the one
/// whose point 800 on from there and whose nearest point to the rider are nearest (squared, summed); `forced`
/// leaves out the rider's own line (it is running out). The projection's memory is dropped (+0x3c4 = −1) when the
/// own line's place is more than 1600 off the one the rider has.
pub fn select_race_line(lines: &RaceLines, r: &mut RaceRider, forced: bool) {
    let u = normalise(r.vel);
    let p = add(r.pos, scale(u, 800.0));
    let current = r.place.line;
    let mut best = 1e37f32;
    let mut chosen = current;
    for l in nearest_lines(&lines.lines, p, 3) {
        if forced && l == current {
            continue;
        }
        let (along, near, _) = lines.lines[l].project(p);
        if l == current && 1600.0 < ((lines.to_finish[l] - r.place.to_finish) - along).abs() {
            r.place.cache.index = -1;
        }
        let (ahead, _) = lines.lines[l].position_at(along + 800.0);
        let a = sub(p, ahead);
        let b = sub(r.pos, near);
        let cost = dot(b, b) + dot(a, a);
        if cost < best {
            best = cost;
            chosen = l;
        }
    }
    r.place.line = chosen;
}

/// `AIP_FindNearestLines` 0x198110: up to `n` race lines by how far `p` is outside their boxes: first those it is
/// inside, in order, then the nearest one at a time (the first on a tie).
pub fn nearest_lines(lines: &[Path], p: V4, n: usize) -> Vec<usize> {
    let want = lines.len().min(n);
    let mut d = vec![0.0f32; lines.len()];
    let mut out = Vec::new();
    for k in 0..lines.len() {
        d[k] = lines[k].box_distance(p);
        if d[k] == 0.0 {
            d[k] = 1e37;
            out.push(k);
            if out.len() == want {
                return out;
            }
        }
    }
    while out.len() < want {
        let mut best = 0;
        let mut f = d[0];
        for k in 1..d.len() {
            if d[k] < f {
                f = d[k];
                best = k;
            }
        }
        out.push(best);
        d[best] = 1e37;
    }
    out
}

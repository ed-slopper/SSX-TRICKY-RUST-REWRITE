//! The course's race lines and their events, as the original tracks them
//! (`Boarder_UpdateRaceLineProgress`, `Boarder_SelectRaceLine`, `Path_QueryEventsAlong`).
//!
//! Each race line is a start point and segments; distances along a line are measured on the
//! level (2D) length of each segment, and every line knows its distance to the finish (DTF) at
//! its start. A rider's progress is the best (smallest) distance to the finish reached so far;
//! events between the old and the new best fire as the rider passes them.
use bevy::prelude::Vec3;

/// File event types on race lines (`Path_ReadEvent`: runtime = file + 1).
pub const EV_FINISH: i32 = 9;
pub const EV_CHECKPOINT: i32 = 11;
pub const EV_LAP: i32 = 12;
pub const EV_BRANCH: i32 = 300;
const LAPS_ON: bool = true;

#[derive(Clone, Copy, Debug)]
pub struct Event { pub kind: i32, pub value: i32, pub start: f32, pub end: f32 }

pub struct Line {
    /// points in Bevy space (metres, Y up)
    pub pts: Vec<Vec3>,
    /// distance along the line (level length) at each point
    pub at: Vec<f32>,
    pub dtf: f32,
    pub events: Vec<Event>,
    lo: Vec3,
    hi: Vec3,
}
impl Line {
    pub fn len(&self) -> f32 { *self.at.last().unwrap_or(&0.0) }
    /// (distance along, sideways distance on the level) of the nearest point to `p`.
    pub fn project(&self, p: Vec3) -> (f32, f32) {
        let mut best = (0.0, f32::MAX);
        for i in 0..self.pts.len().saturating_sub(1) {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let d = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
            let l = d.length();
            let t = if l > 1e-4 { (Vec3::new(p.x - a.x, 0.0, p.z - a.z).dot(d) / l).clamp(0.0, l) } else { 0.0 };
            let q = a + (b - a) * if l > 1e-4 { t / l } else { 0.0 };
            let off = Vec3::new(p.x - q.x, 0.0, p.z - q.z).length();
            if off < best.1 { best = (self.at[i] + t, off); }
        }
        best
    }
    /// The point at a distance along the line (clamped to its ends).
    pub fn point_at(&self, s: f32) -> Vec3 {
        let n = self.pts.len();
        if n == 0 { return Vec3::ZERO; }
        if s <= 0.0 { return self.pts[0]; }
        for i in 0..n - 1 {
            if s <= self.at[i + 1] {
                let l = (self.at[i + 1] - self.at[i]).max(1e-4);
                return self.pts[i].lerp(self.pts[i + 1], (s - self.at[i]) / l);
            }
        }
        self.pts[n - 1]
    }
    fn box_dist(&self, p: Vec3) -> f32 { (p - p.clamp(self.lo, self.hi)).length() }
}

#[derive(Default)]
pub struct Lines { pub lines: Vec<Line> }

/// One rider's progress along the race lines.
#[derive(Clone, Debug)]
pub struct Progress {
    pub line: usize,
    /// best (smallest) distance to the finish so far, and the current one
    pub best: f32,
    pub dtf: f32,
    pub off: f32,
    /// events the rider is inside: (line, event index)
    inside: Vec<(usize, usize)>,
    ticks: u32,
    pub started: bool,
    /// laps still to go (Tokyo Megaplex: 4 at the start, one off at each lap line)
    pub laps: u32,
}
impl Default for Progress { fn default() -> Self { Self { line: 0, best: f32::MAX, dtf: f32::MAX, off: 0.0, inside: Vec::new(), ticks: 0, started: false, laps: 0 } } }

impl Lines {
    /// `lines` are (start, offsets, distance to finish, events) in game units (cm, Z up).
    pub fn build(raw: &[(Vec3, Vec<Vec3>, f32, Vec<Event>)], g2b: impl Fn(Vec3) -> Vec3) -> Self {
        let lines = raw.iter().map(|(start, offs, dtf, evs)| {
            let mut p = *start;
            let mut pts = vec![g2b(p)];
            let mut at = vec![0.0f32];
            for o in offs {
                p += *o;
                at.push(at.last().unwrap() + Vec3::new(o.x, o.y, 0.0).length() * 0.01);
                pts.push(g2b(p));
            }
            let (lo, hi) = pts.iter().fold((Vec3::MAX, Vec3::MIN), |(l, h), q| (l.min(*q), h.max(*q)));
            let events = evs.iter().map(|e| Event { kind: e.kind, value: e.value, start: e.start * 0.01, end: e.end * 0.01 }).collect();
            Line { pts, at, dtf: dtf * 0.01, events, lo, hi }
        }).collect();
        Self { lines }
    }
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }
    /// Does any line carry a finish event?
    pub fn has_laps(&self) -> bool { LAPS_ON && self.lines.iter().any(|l| l.events.iter().any(|e| e.kind == EV_LAP)) }
    /// `Boarder_SelectRaceLine`: look 8 m ahead along the travel; of the three nearest lines, the one
    /// that both passes close to that point and carries on in that direction.
    fn select(&self, pos: Vec3, vel: Vec3, exclude: Option<usize>) -> Option<usize> {
        let p = pos + vel.normalize_or_zero() * 8.0;
        let mut near: Vec<usize> = (0..self.lines.len()).collect();
        near.sort_by(|a, b| self.lines[*a].box_dist(p).total_cmp(&self.lines[*b].box_dist(p)));
        near.into_iter().take(3).filter(|i| Some(*i) != exclude).map(|i| {
            let l = &self.lines[i];
            let (a, _) = l.project(p);
            let q = l.point_at(a);
            let cost = (p - l.point_at(a + 8.0)).length_squared() + (pos - q).length_squared();
            (i, cost)
        }).min_by(|a, b| a.1.total_cmp(&b.1)).map(|c| c.0)
    }
    /// Advance a rider's progress (one call per frame). Returns the events entered this time.
    pub fn update(&self, pr: &mut Progress, pos: Vec3, vel: Vec3) -> Vec<Event> {
        let mut out = Vec::new();
        if self.lines.is_empty() { return out; }
        pr.ticks += 1;
        if !pr.started {
            pr.started = true;
            // the course with a lap line (Tokyo Megaplex) is ridden four times round
            if LAPS_ON && self.lines.iter().any(|l| l.events.iter().any(|e| e.kind == EV_LAP)) { pr.laps = 4; }
            pr.line = self.select(pos, vel, None).unwrap_or(0);
        }
        let l = &self.lines[pr.line.min(self.lines.len() - 1)];
        let (along, off) = l.project(pos);
        pr.off = off;
        // change lines: well off this one (checked once a second), near its end, or at a fork
        let fork = l.events.iter().any(|e| e.kind == EV_BRANCH && (l.dtf - e.start - pr.best).abs() < 3.0);
        let pick = if off > 5.0 && pr.ticks.is_multiple_of(60) { Some(None) } else if along > l.len() - 2.0 { Some(Some(pr.line)) } else if fork { Some(None) } else { None };
        if let Some(ex) = pick {
            if let Some(n) = self.select(pos, vel, ex) {
                if n != pr.line { pr.line = n; pr.inside.clear(); }
            }
        }
        let li = pr.line;
        let l = &self.lines[li];
        let (along, _) = l.project(pos);
        pr.dtf = l.dtf - along;
        if pr.best == f32::MAX { pr.best = pr.dtf; }
        // round again: the distance to the finish jumps back up past half the course
        if pr.laps > 0 && pr.dtf - pr.best > 0.5 * self.lines[0].dtf { pr.best = pr.dtf; pr.inside.clear(); }
        // events between the best distance reached before and now (moving back fires nothing)
        let (lo, hi) = if pr.dtf < pr.best { (l.dtf - pr.best, l.dtf - pr.dtf) } else { let a = l.dtf - pr.best; (a, a) };
        if pr.dtf < pr.best { pr.best = pr.dtf; }
        let mut now = Vec::new();
        for (k, e) in l.events.iter().enumerate() {
            if e.start <= hi && e.end >= lo && !(e.start <= lo && e.end <= hi) {
                now.push((li, k));
                if !pr.inside.contains(&(li, k)) {
                    if e.kind == EV_LAP && pr.laps > 0 { pr.laps -= 1; }
                    // the finish only counts on the last lap
                    if !(e.kind == EV_FINISH && pr.laps > 0) { out.push(*e); }
                }
            }
        }
        pr.inside = now;
        out
    }
}

/// The opponents' AI paths (`cAIPath`): a network of paths that fork and join. Each has a
/// preference value (U3, 0..100) the original weighs against what a rider wants at a fork.
#[derive(Default)]
pub struct AiPaths { pub paths: Vec<Line>, pub u3: Vec<f32>, pub respawn: Vec<bool> }
impl AiPaths {
    /// `raw` is (start, offsets, U3, respawnable, events) in game units.
    pub fn build(raw: &[(Vec3, Vec<Vec3>, f32, bool, Vec<Event>)], g2b: impl Fn(Vec3) -> Vec3) -> Self {
        let lines = Lines::build(&raw.iter().map(|r| (r.0, r.1.clone(), 0.0, r.4.clone())).collect::<Vec<_>>(), g2b);
        Self { paths: lines.lines, u3: raw.iter().map(|r| r.2).collect(), respawn: raw.iter().map(|r| r.3).collect() }
    }
    pub fn is_empty(&self) -> bool { self.paths.is_empty() }
    /// `Boarder_SelectAIPath`: from a point 8 m along the travel, of the six nearest paths (not
    /// ones nearly run out there), the one that passes close and carries on that way, weighed by
    /// how well its U3 matches what the rider wants (`pref`, AI riders only). Returns (path, distance along).
    pub fn select(&self, pos: Vec3, dir: Vec3, pref: Option<f32>, exclude: Option<usize>) -> Option<(usize, f32)> {
        let p = pos + dir.normalize_or_zero() * 8.0;
        let mut near: Vec<usize> = (0..self.paths.len()).filter(|i| Some(*i) != exclude).collect();
        near.sort_by(|a, b| self.paths[*a].box_dist(p).total_cmp(&self.paths[*b].box_dist(p)));
        near.into_iter().take(6).filter_map(|i| {
            let l = &self.paths[i];
            let (s, _) = l.project(p);
            if s > l.len() - 2.0 { return None; }
            let q = l.point_at(s);
            let mut cost = (pos - q).length_squared() + (p - l.point_at(s + 8.0)).length_squared();
            if let Some(w) = pref { cost -= (100.0 - (self.u3[i] - w).abs()) * 2.3187357; }
            Some((i, s, cost))
        }).min_by(|a, b| a.2.total_cmp(&b.2)).map(|c| (c.0, c.1))
    }
}

impl AiPaths {
    /// Where a reset puts a rider (`Boarder_PlaceOnCourse`, `Path_AdjustRespawnDistance`): on the
    /// nearest respawnable AI path level with `pos`, moved back to the start of a "respawn before"
    /// zone (file event 103), on to the end of a "respawn after" zone (105) or past a jump (100).
    /// Returns (point on the path, heading yaw).
    pub fn respawn_point(&self, pos: Vec3) -> Option<(Vec3, f32)> {
        let (i, s) = (0..self.paths.len()).filter(|i| self.respawn[*i] && self.paths[*i].pts.len() > 1)
            .map(|i| { let (s, off) = self.paths[i].project(pos); let dy = (self.paths[i].point_at(s).y - pos.y).abs(); (i, s, off + dy * 0.5) })
            .min_by(|a, b| a.2.total_cmp(&b.2)).map(|c| (c.0, c.1))?;
        let l = &self.paths[i];
        let mut s = s;
        for e in &l.events {
            if s >= e.start && s <= e.end {
                match e.kind { 103 => s = e.start, 105 | 100 => s = e.end + 1.0, _ => {} }
            }
        }
        let s = s.clamp(0.0, (l.len() - 2.0).max(0.0));
        let p = l.point_at(s);
        let d = l.point_at(s + 2.0) - p;
        Some((p, f32::atan2(-d.x, -d.z)))
    }
}

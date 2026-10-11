//! The AI paths' geometry exactly (board row G4f): `AIP_GetPathLength` 0x197b50, `AIP_GetPositionAtDistance`
//! 0x1979f8, `AIP_GetDirectionAtDistance` 0x197738, `AIP_ProjectOntoPath` 0x197250, `AIP_ProjectOntoPathFrom`
//! 0x197478, `AIP_DistanceToLineBox` 0x197830 and `AIP_SortLinesByDistance` 0x197f28, on paths as the game keeps
//! them (cm, z up), checked bit for bit in the function runner on the disc's own paths (`tests/aipath.rs`).

use crate::jump::{add, scale, sub};

type V4 = [f32; 4];

/// A path as the game keeps it (`cPath`): its start (+0xc), its box (+0x1c min, +0x28 max), its points (+0x18:
/// unit direction and length, from the start), and `cAIPath`'s preference (+0x38) and respawnable flag (+0x3c).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub start: [f32; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub points: Vec<([f32; 3], f32)>,
    pub u3: u32,
    pub respawnable: u32,
    /// the events as the game keeps them: (type as its place in the table, value, start, end)
    pub events: Vec<(u32, u32, f32, f32)>,
}

impl Path {
    /// From a path of a course's `.aip`.
    pub fn from_aip(p: &tricky_data::aip::AipPath) -> Path {
        Path { start: p.start, min: p.v1, max: p.v2, points: p.points.clone(), u3: p.u3(), respawnable: p.respawnable(), events: p.events.iter().map(|e| (e.kind, e.value, e.start, e.end)).collect() }
    }

    fn dir(&self, k: usize) -> V4 {
        let d = self.points[k].0;
        [d[0], d[1], d[2], 0.0]
    }

    fn start4(&self) -> V4 {
        [self.start[0], self.start[1], self.start[2], 1.0]
    }

    /// `AIP_GetPathLength` 0x197b50: the lengths added in order.
    pub fn length(&self) -> f32 {
        let mut s = 0.0;
        for p in &self.points {
            s += p.1;
        }
        s
    }

    /// `AIP_GetPositionAtDistance` 0x1979f8: the point `d` along (w 1), and whether the path reaches it (else its
    /// end).
    pub fn position_at(&self, d: f32) -> (V4, bool) {
        let mut at = self.start4();
        let mut d = d;
        for k in 0..self.points.len() {
            let len = self.points[k].1;
            if d < len {
                return (add(at, scale(self.dir(k), d)), true);
            }
            d -= len;
            at = add(at, scale(self.dir(k), len));
        }
        (at, false)
    }

    /// `AIP_GetDirectionAtDistance` 0x197738: the direction of the segment `d` falls in (the last one past the end).
    pub fn direction_at(&self, d: f32) -> V4 {
        let mut d = d;
        let mut k = 0;
        while k + 1 != self.points.len() {
            let len = self.points[k].1;
            if d <= len {
                break;
            }
            d -= len;
            k += 1;
        }
        self.dir(k)
    }

    /// One segment's flat (x, y) projection: the clamped distance along, the raw one, the point and the squared
    /// flat distance to it.
    fn seg(&self, k: usize, at: V4, pos: V4) -> (f32, f32, V4, f32) {
        let d = self.dir(k);
        let len = self.points[k].1;
        let v = sub(pos, at);
        let raw = v[0] * d[0] + v[1] * d[1];
        let t = if raw < 0.0 { 0.0 } else if raw <= len { raw } else { len };
        let q = add(at, scale(d, t));
        let e = sub(pos, q);
        (t, raw, q, e[0] * e[0] + e[1] * e[1])
    }

    /// `AIP_ProjectOntoPath` 0x197250: over every segment, the nearest point in x and y: its distance along, the
    /// point, and the flat distance to it (0, 0 for no points).
    pub fn project(&self, pos: V4) -> (f32, V4, f32) {
        let mut at = self.start4();
        let (mut along, mut run, mut best) = (0.0, 0.0, 1e37f32);
        let mut lat2 = 0.0;
        let mut near = [0.0; 4];
        for k in 0..self.points.len() {
            let (t, _, q, d2) = self.seg(k, at, pos);
            if d2 < best {
                along = run + t;
                near = q;
                best = d2;
                lat2 = d2;
            }
            let len = self.points[k].1;
            at = add(at, scale(self.dir(k), len));
            run += len;
        }
        (along, near, lat2.sqrt())
    }

    /// `AIP_ProjectOntoPathFrom` 0x197478: as [`Path::project`], from the segment `cache` remembers (rider+0x390:
    /// its start, its distance along, its index; −1 to start over) and at most 3000 past it, stopping at the first
    /// segment the point falls within; the cache moves to the nearest segment. `near` keeps its value when no
    /// segment is nearer than 1e37.
    pub fn project_from(&self, pos: V4, near: &mut V4, cache: &mut Cache) -> (f32, f32) {
        let cached = cache.index;
        let (mut at, mut run, limit, mut k) = if cached < 0 { (self.start4(), 0.0, 1e37f32, 0) } else { (cache.start, cache.along, cache.along + 3000.0, cached as usize) };
        let (mut along, mut best, mut lat2) = (0.0f32, 1e37f32, 0.0f32);
        while k < self.points.len() {
            let (t, raw, q, d2) = self.seg(k, at, pos);
            if d2 < best {
                *near = q;
                along = run + t;
                *cache = Cache { start: at, along: run, index: k as i32 };
                best = d2;
                lat2 = d2;
                if 0 <= cached && t == raw {
                    return (along, lat2.sqrt());
                }
            }
            if limit < run {
                break;
            }
            let len = self.points[k].1;
            at = add(at, scale(self.dir(k), len));
            run += len;
            k += 1;
        }
        (along, lat2.sqrt())
    }

    /// `AIP_DistanceToLineBox` 0x197830: how far outside the path's box (the largest of the three axes).
    pub fn box_distance(&self, p: V4) -> f32 {
        let axis = |v: f32, lo: f32, hi: f32| if v < lo { lo - v } else if hi < v { v - hi } else { 0.0 };
        let (x, y, z) = (axis(p[0], self.min[0], self.max[0]), axis(p[1], self.min[1], self.max[1]), axis(p[2], self.min[2], self.max[2]));
        if y < x {
            if z <= x { x } else { z }
        } else if y < z {
            z
        } else {
            y
        }
    }
}

/// The projection's remembered segment (rider+0x390: start, +0x3a0 distance along, +0x3a4 index; −1 none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cache {
    pub start: V4,
    pub along: f32,
    pub index: i32,
}

impl Default for Cache {
    fn default() -> Self {
        Cache { start: [0.0; 4], along: 0.0, index: -1 }
    }
}

/// `AIP_SortLinesByDistance` 0x197f28: up to `n` paths by how far `p` is outside their boxes: first those it is
/// inside, in order, then the nearest one at a time (the first on a tie); only respawnable ones if `respawnable`.
pub fn sort_by_distance(paths: &[Path], p: V4, n: usize, respawnable: bool) -> Vec<usize> {
    let want = n.min(paths.len());
    let mut d: Vec<f32> = Vec::with_capacity(paths.len());
    let mut out = Vec::new();
    for (k, path) in paths.iter().enumerate() {
        let mut v = path.box_distance(p);
        if respawnable && path.respawnable == 0 {
            v = 1e37;
        }
        if v == 0.0 {
            v = 1e37;
            out.push(k);
            d.push(v);
            if out.len() == want {
                return out;
            }
        } else {
            d.push(v);
        }
    }
    while out.len() < want {
        let mut best = 0;
        for k in 1..d.len() {
            if !(d[best] <= d[k]) {
                best = k;
            }
        }
        d[best] = 1e37;
        out.push(best);
    }
    out
}

impl Path {
    /// `cPath::vf1` 0x197b90 (vtable +0x14): a fork marker (event type 0x1f, file code 300) overlapping `from..to`.
    pub fn fork_between(&self, from: f32, to: f32) -> bool {
        self.events.iter().any(|e| e.2 <= to && from <= e.3 && e.0 == 0x1f)
    }
}

/// The rider's place on its AI path (rider+0x57a4 the path, +0x340 the last distance along, +0x344 the distance
/// along, +0x348 how far off it (flat), +0x350 the nearest point, +0x360 the point 800 ahead, +0x370 the heading
/// from the one to the other, +0x390 the projection's memory).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub path: usize,
    pub last: f32,
    pub along: f32,
    pub off: f32,
    pub near: V4,
    pub ahead: V4,
    pub heading: f32,
    pub cache: Cache,
}

/// What `Boarder_ChoosePathAhead` weighs a path by: the rider's position (+0x140) and velocity (+0x150), +0x41c
/// (set: no preference), the board class (+0x420), its place (+0x110), the meter (+0x1c), how likely it keeps its
/// preference (the rider's virtual +0x74, 0..1).
#[derive(Clone, Copy, Debug, Default)]
pub struct Chooser {
    pub pos: V4,
    pub vel: V4,
    pub x41c: i32,
    pub class: u32,
    pub place: i32,
    pub meter: f32,
    pub keep: f32,
}

/// `Boarder_ChoosePathAhead` 0x118830: from the point 800 ahead along the velocity, of the six nearest paths (not
/// the one ridden when `forced`, at its end) that run on 200 past it, the one passing nearest the rider and the
/// point and carrying on that way (the squared distances from the rider to the path and from the point to the
/// path 800 further), less (100 − |its U3 − the preference|) × 23187.357. The preference: by board class, place and
/// meter (0 a leader on an alpine board; 100 short of meter; else 50), kept with the chance `keep` against
/// `Rand_Game() % 100` (`rand`, called once unless +0x41c). A new path moves the rider onto it (true).
pub fn choose_path_ahead(paths: &[Path], f: &mut Follow, forced: bool, c: &Chooser, rand: &mut dyn FnMut() -> i32) -> bool {
    let look = crate::jump::add(c.pos, scale(crate::jump::normalize(c.vel), 800.0));
    let mut pref = 50;
    if c.x41c == 0 {
        let mut p = 50;
        if c.class == 2 {
            if c.place == 0 {
                p = 0;
            } else if c.meter < 0.0 {
                p = 100;
            }
        } else if c.class == 0 {
            if 0.103_376_17 <= c.meter {
                if (c.place.wrapping_sub(2) as u32) < 2 {
                    p = 0;
                }
            } else {
                p = 100;
            }
        } else if 0.311_869_03 <= c.meter {
            if (c.place.wrapping_sub(3) as u32) < 2 {
                p = 0;
            }
        } else {
            p = 100;
        }
        pref = p;
        let r = rand();
        let k = c.keep * 100.0;
        // libgcc's `__fixunssfsi`: below 2^31 as a signed conversion (a negative chance wraps high)
        let kept = if k >= 2_147_483_648.0 { ((k - 2_147_483_648.0) as i32 as u32).wrapping_add(0x8000_0000) } else { k as i32 as u32 };
        if kept < (r % 100) as u32 {
            pref = 50;
        }
    }
    let mut best = 1e37f32;
    let mut chosen: Option<usize> = None;
    for k in sort_by_distance(paths, look, 6, false) {
        if k == f.path && forced {
            continue;
        }
        let p = &paths[k];
        let (d, near, _) = p.project(look);
        if !(d <= p.length() - 200.0) {
            continue;
        }
        let ahead = p.position_at(d + 800.0).0;
        let a = sub(look, ahead);
        let b = sub(c.pos, near);
        let mut cost = crate::jump::dot(b, b) + crate::jump::dot(a, a);
        if c.x41c == 0 {
            cost -= (100 - (p.u3 as i32 - pref).abs()) as f32 * 23_187.357;
        }
        if cost < best {
            best = cost;
            chosen = if k == f.path { None } else { Some(k) };
        }
    }
    let Some(k) = chosen else { return false };
    f.path = k;
    f.cache.index = -1;
    let (d, off) = paths[k].project_from(c.pos, &mut f.near, &mut f.cache);
    f.off = off;
    f.along = d;
    f.ahead = paths[k].position_at(d + 800.0).0;
    f.last = f.along;
    true
}

/// `Boarder_UpdatePathAhead` 0x118090, every tick: the rider's place on its path moves on (the last distance kept,
/// the projection from its memory, the point 800 ahead); while racing (rider+0x418 < 0, `racing`) a new path is
/// chosen when over 500 off it (on every 60th tick), within 200 of its end (not the same one), or past a fork
/// marker; then the heading from the nearest point to the point ahead (the game's inline atan2).
pub fn update_path_ahead(paths: &[Path], f: &mut Follow, racing: bool, tick: i32, c: &Chooser, rand: &mut dyn FnMut() -> i32) {
    f.last = f.along;
    let (d, off) = paths[f.path].project_from(c.pos, &mut f.near, &mut f.cache);
    f.off = off;
    f.along = d;
    f.ahead = paths[f.path].position_at(d + 800.0).0;
    if racing {
        if 500.0 < f.off && tick % 60 == 0 {
            choose_path_ahead(paths, f, false, c, rand);
        } else if paths[f.path].length() - 200.0 < f.along {
            choose_path_ahead(paths, f, true, c, rand);
        } else if paths[f.path].fork_between(f.last, f.along) {
            choose_path_ahead(paths, f, false, c, rand);
        }
    }
    let dx = f.ahead[0] - f.near[0];
    let dy = f.ahead[1] - f.near[1];
    f.heading = crate::jump::atan2_inline(dy, dx);
}

impl Path {
    /// `AIP_CanResetAt` 0x196858: no "no reset" event (type 0x1d, file code 104) covers `d`.
    pub fn can_reset_at(&self, d: f32) -> bool {
        !self.events.iter().any(|e| e.2 <= d && d <= e.3 && e.0 == 0x1d)
    }

    /// `ResetState_GetRespawnPos` 0x196758: where a rider reset at `d` comes back, and the point there. Of the
    /// events covering `d`: type 0x19 (code 100) moves it on to the furthest end, 0x1c (103) back to the earliest
    /// start and 0x1e (105) on to the furthest end; the 0x1c / 0x1e place wins unless it is still `d`.
    pub fn respawn_at(&self, d: f32) -> (f32, V4) {
        let (mut on, mut moved) = (d, d);
        for e in &self.events {
            if e.2 <= d && d <= e.3 {
                match e.0 {
                    0x19 => on = if e.3 < on { on } else { e.3 },
                    0x1c => moved = if moved < e.2 { moved } else { e.2 },
                    0x1e => moved = if e.3 < moved { moved } else { e.3 },
                    _ => {}
                }
            }
        }
        let at = if moved == d { on } else { moved };
        (at, self.position_at(at).0)
    }
}

/// The race lines (the `.aip`'s event paths, `cEventPath`, 0x3c bytes each): their geometry, each one's distance to
/// the finish at its start (+0x38, parameter 0) and the main one's index (the path set's +0x10).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceLines {
    pub lines: Vec<Path>,
    pub to_finish: Vec<f32>,
    pub main: usize,
}

impl RaceLines {
    pub fn from_aip(a: &tricky_data::aip::Aip) -> RaceLines {
        RaceLines {
            lines: a.event_paths.iter().map(Path::from_aip).collect(),
            to_finish: a.event_paths.iter().map(|p| f32::from_bits(p.param(0).unwrap_or(0))).collect(),
            main: a.main_event_path as usize,
        }
    }

    /// `AIPathSet_FindRaceLineAt` 0x1982d0: the first line reaching the place `d` from the finish (no further than
    /// the main line's start), and the point there.
    pub fn line_at(&self, d: f32) -> Option<(usize, V4)> {
        let main = self.to_finish[self.main];
        let d = if d <= main { d } else { main };
        for (k, l) in self.lines.iter().enumerate() {
            let v = self.to_finish[k] - d;
            if 0.0 <= v && v <= l.length() {
                return Some((k, l.position_at(v).0));
            }
        }
        None
    }
}

/// The rider's place on the race line (rider+0x57a0 the line, +0x374 its distance to the finish, +0x378 the best
/// of it this lap, +0x3b0 the projection's memory).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RacePlace {
    pub line: usize,
    pub to_finish: f32,
    pub best: f32,
    pub cache: Cache,
}

/// `Boarder_AdvanceOnPath` 0x118c08, for a reset: the point `by` further along the rider's path (on past its end
/// along its last direction), then, of the six nearest respawnable paths that run on 200 past it, the one passing
/// nearest it and carrying on that way (as [`choose_path_ahead`], with no preference); the rider's place moves to
/// that point.
pub fn advance_on_path(paths: &[Path], f: &mut Follow, by: f32) {
    let p = &paths[f.path];
    let len = p.length();
    let pt = if f.along + by <= len {
        p.position_at(f.along + by).0
    } else {
        let dir = p.direction_at(len);
        let end = p.position_at(len).0;
        add(end, scale(dir, by - (len - f.along)))
    };
    let mut best = 1e37f32;
    let mut chosen: Option<usize> = None;
    for k in sort_by_distance(paths, pt, 6, true) {
        let q = &paths[k];
        let (d, near, _) = q.project(pt);
        if !(d <= q.length() - 200.0) {
            continue;
        }
        let a = sub(pt, q.position_at(d + 800.0).0);
        let b = sub(pt, near);
        let cost = crate::jump::dot(b, b) + crate::jump::dot(a, a);
        if cost < best {
            best = cost;
            chosen = if k == f.path { None } else { Some(k) };
        }
    }
    if let Some(k) = chosen {
        f.path = k;
        f.cache.index = -1;
    }
    let (d, off) = paths[f.path].project_from(pt, &mut f.near, &mut f.cache);
    f.off = off;
    f.along = d;
    f.ahead = paths[f.path].position_at(d + 800.0).0;
    f.last = f.along;
}

/// `Boarder_FindNearestLine` 0x119d30 up to its call of `ResetState_PlaceRider`: the rider put on the race line
/// at the place `d` from the finish ([`RaceLines::line_at`]; false, nothing changed, where no line reaches it —
/// the game goes on with an unset point), on the respawnable path (of the six nearest) passing nearest that point,
/// with the point 800 ahead and the heading; then the place on the race line from the nearest point, and with laps
/// left (rider+0x114) the lap's best unless it went back or on more than half the main line.
pub fn find_nearest_line(paths: &[Path], lines: &RaceLines, f: &mut Follow, r: &mut RacePlace, d: f32, laps_left: i32) -> bool {
    let Some((line, pt)) = lines.line_at(d) else { return false };
    r.line = line;
    let mut best = 1e37f32;
    for k in sort_by_distance(paths, pt, 6, true) {
        let (along, near, lat) = paths[k].project(pt);
        if lat < best {
            f.last = along;
            f.path = k;
            f.near = near;
            f.cache.index = -1;
            f.off = lat;
            f.along = along;
            best = lat;
        }
    }
    f.ahead = paths[f.path].position_at(f.along + 800.0).0;
    f.heading = crate::jump::atan2_inline(f.ahead[1] - f.near[1], f.ahead[0] - f.near[0]);
    f.cache.index = -1;
    r.cache.index = -1;
    let mut scratch = [0.0; 4];
    let (along, _) = lines.lines[line].project_from(f.near, &mut scratch, &mut r.cache);
    r.to_finish = lines.to_finish[line] - along;
    if 0 < laps_left {
        let gain = r.to_finish - r.best;
        if gain < 0.0 || lines.to_finish[lines.main] * 0.5 < gain {
            r.best = r.to_finish;
        }
    }
    true
}

/// One `Boarder_PlaceAt` 0x11cae8 call of a reset: the heading, the height passed on (the caller's), the place and
/// the velocity (the path's direction × 833.3333, z 0). The game clears rider+0x24 just before it unless the rider
/// is Tricky-boosting (`Boarder_IsTricky`), and cuts the rider's camera (`Camera_CutForReset`) just after.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub heading: f32,
    pub height: f32,
    pub pos: V4,
    pub vel: V4,
}

/// One other rider as the reset sees it: position (+0x140) and velocity (+0x150).
pub type Other = (V4, V4);

struct Near {
    rider: usize,
    side: V4,
    plane: f32,
    used: bool,
    s: f32,
    sign: i32,
}

/// `ResetState_PlaceRider` 0x119220 (a0 the rider, a1 `spread`, f12 `height`): the rider's place moves to the
/// respawn point ([`Path::respawn_at`]: +0x344 and the point, +0x350), facing along the path there. With `spread`,
/// out of the way of the other riders standing within 1000 of it: each one's side line (across its direction of
/// travel, flat) that the point is within 199 of, nearest first, pushes it 200 clear, to the side that keeps it on
/// the same side of the lines already cleared (the first to the side it is on); a push that goes over 800 or into
/// the world (`ray` from the point raised by `height` to there as well, its hit fraction; ≥ 0 a hit) puts the rider
/// on the race line 500 further back instead ([`find_nearest_line`], whose own placement comes first) and keeps the
/// point that gives; else it moves there and, if the last rider cleared is heading within 30° of the path, takes
/// its direction. The placements made, in order (see [`Placement`]); false where the race line was not found (the
/// game goes on with an unset point).
#[allow(clippy::too_many_arguments)]
pub fn place_rider(paths: &[Path], lines: &RaceLines, f: &mut Follow, r: &mut RacePlace, laps_left: i32, spread: bool, height: f32, others: &[Other], ray: &mut dyn FnMut(V4, V4) -> f32, out: &mut Vec<Placement>) -> bool {
    let (along, at) = paths[f.path].respawn_at(f.along);
    f.along = along;
    f.near = at;
    let mut dir = paths[f.path].direction_at(f.along);
    let mut heading = crate::jump::atan2_inline(dir[1], dir[0]);
    if spread {
        let mut near: Vec<Near> = Vec::new();
        for (k, o) in others.iter().enumerate() {
            let d = sub(o.0, f.near);
            if crate::jump::dot(d, d).sqrt() < 1000.0 {
                near.push(Near { rider: k, side: [0.0; 4], plane: 0.0, used: false, s: 0.0, sign: 0 });
            }
        }
        if near.is_empty() {
            return finish(f, dir, heading, height, out);
        }
        for n in near.iter_mut() {
            let v = crate::jump::normalize(others[n.rider].1);
            // the side: v × up (vopmula / vopmsub), normalised
            let up = [0.0f32, 0.0, 1.0, 0.0];
            let c = [v[1] * up[2] - up[1] * v[2], v[2] * up[0] - up[2] * v[0], v[0] * up[1] - up[0] * v[1], 0.0];
            n.side = crate::jump::normalize(c);
            n.plane = -crate::jump::dot(others[n.rider].0, n.side);
        }
        let mut offset = [0.0f32; 4];
        let mut moved = false;
        let mut last = 0usize;
        loop {
            let mut best = 200.0f32;
            let mut pick: Option<usize> = None;
            for (i, n) in near.iter_mut().enumerate() {
                if n.used {
                    continue;
                }
                let s = crate::jump::dot(add(f.near, offset), n.side) + n.plane;
                let a = s.abs();
                if a < 199.0 && a < best {
                    n.s = s;
                    best = a;
                    pick = Some(i);
                }
            }
            let Some(p) = pick else { break };
            let s = near[p].s;
            let ps = near[p].side;
            let mut sign = 1;
            for j in near.iter().filter(|n| n.used) {
                let same = (0.0 <= s && 0 < sign) || (s < 0.0 && sign < 0);
                let t = if same {
                    if 0 < j.sign { crate::jump::dot(j.side, ps) } else { crate::jump::dot(scale(j.side, -1.0), ps) }
                } else if 0 < j.sign {
                    crate::jump::dot(j.side, scale(ps, -1.0))
                } else {
                    crate::jump::dot(scale(j.side, -1.0), scale(ps, -1.0))
                };
                if t <= 0.0 {
                    sign = -1;
                }
            }
            let push_on = if 0.0 <= s { 0 < sign } else { sign < 1 };
            let m = if push_on {
                near[p].sign = 1;
                200.0 - s
            } else {
                near[p].sign = -1;
                -(s + 200.0)
            };
            offset = add(offset, scale(ps, m));
            near[p].used = true;
            last = p;
            moved = true;
        }
        if moved {
            let far = 800.0 < crate::jump::dot(offset, offset).sqrt();
            let from = add(f.near, [0.0, 0.0, height, 0.0]);
            let hit = ray(from, add(from, offset));
            if far || 0.0 <= hit {
                if !find_nearest_line(paths, lines, f, r, r.to_finish + 500.0, laps_left) {
                    return false;
                }
                if !place_rider(paths, lines, f, r, laps_left, false, height, others, ray, out) {
                    return false;
                }
            } else {
                let v = crate::jump::normalize(others[near[last].rider].1);
                if 0.866 < crate::jump::dot(v, dir) {
                    dir = v;
                    heading = crate::jump::atan2_inline(dir[1], dir[0]);
                }
                f.near = add(f.near, offset);
            }
        }
    }
    finish(f, dir, heading, height, out)
}

fn finish(f: &Follow, dir: V4, heading: f32, height: f32, out: &mut Vec<Placement>) -> bool {
    out.push(Placement { heading, height, pos: f.near, vel: [dir[0] * 833.3333, dir[1] * 833.3333, 0.0, dir[3]] });
    true
}

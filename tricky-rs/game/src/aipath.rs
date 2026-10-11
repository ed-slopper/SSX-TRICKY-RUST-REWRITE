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
}

impl Path {
    /// From a path of a course's `.aip`.
    pub fn from_aip(p: &tricky_data::aip::AipPath) -> Path {
        Path { start: p.start, min: p.v1, max: p.v2, points: p.points.clone(), u3: p.u3(), respawnable: p.respawnable() }
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

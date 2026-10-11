//! The rider's shape for the world's casts (board row E7a3): `cWorldBoarder` (vtable 0x3a77d8), the rider's
//! collision body (rider+0x470, [`CollBody`]) tested against the world's triangles and boxes the way
//! `Boarder_WallCollide` and `Boarder_InstanceCollisions` use it, with `Collide_SphereTriangle` 0x236bb8, its
//! front-only 0x237480 and either-side 0x237d50 forms, `Collide_BoarderBox` 0x238640 / `Collide_SphereBox`
//! 0x23b6e8 and `CollBody_GetActiveBounds` 0x236830. A hit's "t" is the depth it reaches into the surface.
//! Checked in the function runner (`tests/worldboarder.rs`). Sphere trees (+0x38): row E7a4.

use crate::collbody::CollBody;
use crate::world::{CollMesh, Shape};
use crate::worldline::{tri_contains_point, Hit};

type V4 = [f32; 4];

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| x * k)
}

/// vmul, (x + y) + 1·z + 1·w.
fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn through(m: [V4; 4], v: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((m[0][i] * v[0] + m[1][i] * v[1]) + m[2][i] * v[2]) + m[3][i] * v[3])
}

/// `CollBody_GetActiveBounds` 0x236830: (min, max) of the spheres the mask (+0x24) names, or of the bounding
/// sphere when it is 0 (w then the centre's), from ±1e10 (w 1).
pub fn active_bounds(b: &CollBody) -> (V4, V4) {
    let mut lo = [1e10f32, 1e10, 1e10, 1.0];
    let mut hi = [-1e10f32, -1e10, -1e10, 1.0];
    if b.x24 == 0 {
        let r = b.radius;
        lo = sub(b.center, [r, r, r, 0.0]);
        hi = add(b.center, [r, r, r, 0.0]);
        return (lo, hi);
    }
    for (k, (c, r)) in b.spheres.iter().enumerate().take(b.count.max(0) as usize) {
        if (b.x24 >> k) & 1 == 0 {
            continue;
        }
        let a = sub(*c, [*r, *r, *r, 1.0]);
        for i in 0..3 {
            if a[i] < lo[i] {
                lo[i] = a[i];
            }
        }
        let z = add(*c, [*r, *r, *r, 1.0]);
        for i in 0..3 {
            if hi[i] < z[i] {
                hi[i] = z[i];
            }
        }
    }
    (lo, hi)
}

/// `CollBody_Transform` 0x236498: the bounding centre and each sphere's centre put through `m`.
fn transform(b: &CollBody, m: [V4; 4]) -> CollBody {
    let mut o = *b;
    o.center = through(m, b.center);
    for s in o.spheres.iter_mut().take(b.count.max(0) as usize) {
        s.0 = through(m, s.0);
    }
    o
}

/// Which plane test a triangle uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// `Collide_SphereTriangle`: |d| ≤ r
    Both,
    /// `Collide_SphereTriangleFront`: 0 ≤ d ≤ r
    Front,
    /// 0x237d50: |d| ≤ r, the normal turned to face the sphere
    Either,
}

/// A segment from `a` along `e` against a sphere (the triangle's edges): with w = c − a and L = |e|, f = (w · e)/L;
/// hit when f ≥ 0, |w|² − f² ≤ r² and not L < f − √(r² − (|w|² − f²)).
fn edge(ww: f32, w: V4, e: V4, r2: f32) -> bool {
    let len = dot(e, e).sqrt();
    let f = dot(w, e) / len;
    if !(0.0 <= f) {
        return false;
    }
    let q = ww - f * f;
    if !(q <= r2) {
        return false;
    }
    !(len < f - (r2 - q).sqrt())
}

/// Whether a sphere at `c` of radius `r` touches the triangle, its projection `p` on the plane given: inside the
/// triangle, or (with c at least r from a) along an edge ab, ac or bc.
fn touches(c: V4, r: f32, a: V4, b: V4, cc: V4, p: V4) -> bool {
    if tri_contains_point(a, b, cc, p) {
        return true;
    }
    let w = sub(c, a);
    let r2 = r * r;
    let ww = dot(w, w);
    if !(r2 <= ww) {
        return true;
    }
    if edge(ww, w, sub(b, a), r2) || edge(ww, w, sub(cc, a), r2) {
        return true;
    }
    let w2 = sub(c, b);
    edge(dot(w2, w2), w2, sub(cc, b), r2)
}

/// The sphere-triangle tests: with no mask the bounding sphere (penetration n·(r − d) at its projection), else
/// each named sphere with d below its radius and below the best so far (the shallowest wins).
pub fn sphere_triangle(body: &CollBody, a: V4, b: V4, c: V4, n: V4, side: Side) -> Option<(V4, V4)> {
    let mut n = n;
    let mut d = dot(n, sub(body.center, a));
    let r = body.radius;
    match side {
        Side::Both => {
            if !(d.abs() <= r) {
                return None;
            }
        }
        Side::Front => {
            if !(0.0 <= d) || r < d {
                return None;
            }
        }
        Side::Either => {
            if !(d.abs() <= r) {
                return None;
            }
            if d < 0.0 {
                d = -d;
                n = scale(n, -1.0);
            }
        }
    }
    let p = sub(body.center, scale(n, d));
    if !touches(body.center, r, a, b, c, p) {
        return None;
    }
    if body.x24 == 0 {
        return Some((scale(n, r - d), p));
    }
    let mut best = 1e10f32;
    let mut out = None;
    for (k, (sc, sr)) in body.spheres.iter().enumerate().take(body.count.max(0) as usize) {
        if (body.x24 >> k) & 1 == 0 {
            continue;
        }
        let dk = dot(n, sub(*sc, a));
        if !(dk < *sr && dk < best) {
            continue;
        }
        let pk = sub(*sc, scale(n, dk));
        if touches(*sc, *sr, a, b, c, pk) {
            out = Some((scale(n, *sr - dk), pk));
            best = dk;
        }
    }
    if best != 1e10 {
        out
    } else {
        None
    }
}

/// `Collide_SphereBox` 0x23b6e8(r, lo, hi, c): for each axis whose other two lanes of c lie in the box, the
/// depths (c − lo) + r and (hi − c) + r (both not negative) against the best so far (from 1e30): the point (c moved
/// to that face), the face's normal and the depth.
pub fn sphere_box(r: f32, lo: V4, hi: V4, c: V4) -> Option<(V4, V4, f32)> {
    let mut best = 1e30f32;
    let mut out = None;
    for k in 0..3 {
        let (i, j) = [(1, 2), (0, 2), (0, 1)][k];
        if !(lo[i] <= c[i] && c[i] <= hi[i] && lo[j] <= c[j] && c[j] <= hi[j]) {
            continue;
        }
        let a = (c[k] - lo[k]) + r;
        let b = (hi[k] - c[k]) + r;
        // x skips only on a negative depth, y and z need both not negative (a NaN parts them)
        if (k == 0 && (a < 0.0 || b < 0.0)) || (k != 0 && !(0.0 <= a && 0.0 <= b)) {
            continue;
        }
        if a < best {
            let mut p = c;
            p[k] = lo[k];
            let mut nn = [0.0; 4];
            nn[k] = -1.0;
            out = Some((p, nn, a));
            best = a;
        }
        if b < best {
            let mut p = c;
            p[k] = hi[k];
            let mut nn = [0.0; 4];
            nn[k] = 1.0;
            out = Some((p, nn, b));
            best = b;
        }
    }
    if best != 1e30 {
        out
    } else {
        None
    }
}

/// `Collide_BoarderBox` 0x238640: the bounding sphere against the box; with a mask the game tests the bounding
/// sphere again once for every named sphere (not the sphere itself), keeping it when deeper than −1 — so the same
/// answer, or none when the mask names no sphere.
pub fn boarder_box(body: &CollBody, lo: V4, hi: V4) -> Option<(V4, V4)> {
    let (p, n, depth) = sphere_box(body.radius, lo, hi, body.center)?;
    if body.x24 == 0 {
        return Some((scale(n, depth), p));
    }
    let mut best = -1.0f32;
    let mut out = None;
    for k in 0..body.count.max(0) as usize {
        if (body.x24 >> k) & 1 == 0 {
            continue;
        }
        if let Some((p, n, depth)) = sphere_box(body.radius, lo, hi, body.center) {
            if best < depth {
                out = Some((scale(n, depth), p));
                best = depth;
            }
        }
    }
    if best == -1.0 {
        None
    } else {
        out
    }
}

/// `cWorldBoarder`: +0 its kind (1 when made with a 0 flag), +4 in an object's frame, +0x10 / +0x20 the body's
/// active bounds (max, min), +0x30 their middle, +0x50 the body in use (+0x54 the rider's, +0x60 a copy put in a
/// frame), +0x2d0 / +0x2e0 a normal to reject hits near (dot > 0.8), +0x2f0 front faces only.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldBoarder {
    pub kind: u32,
    pub local: u32,
    pub hi: V4,
    pub lo: V4,
    pub mid: V4,
    pub body: CollBody,
    pub rider: CollBody,
    pub reject: Option<V4>,
    pub front_only: bool,
}

impl WorldBoarder {
    /// `cWorldBoarder::cWorldBoarder` 0x257810(body, flag).
    pub fn new(body: &CollBody, flag: u32) -> WorldBoarder {
        let (lo, hi) = active_bounds(body);
        let mid = scale(add(lo, hi), 0.5);
        WorldBoarder { kind: (flag == 0) as u32, local: 0, hi, lo, mid, body: *body, rider: *body, reject: None, front_only: false }
    }

    /// The +0x10 (sic, the result in t) of a hit: depth √(pen · pen) and normal pen·(1/depth).
    fn hit(pen: V4, contact: V4) -> Hit {
        let depth = dot(pen, pen).sqrt();
        let q = if depth == 0.0 { f32::MAX } else { 1.0 / depth };
        Hit { pos: contact, normal: scale(pen, q), t: depth }
    }

    /// +0x28 (0x257988): the body against a triangle (front faces only when +0x2f0 says)
    pub fn triangle(&self, a: V4, b: V4, c: V4, n: V4) -> Option<Hit> {
        let side = if self.front_only { Side::Front } else { Side::Both };
        sphere_triangle(&self.body, a, b, c, n, side).map(|(pen, p)| Self::hit(pen, p))
    }

    /// +0x30 (0x257a50): from either side
    pub fn triangle_either_side(&self, a: V4, b: V4, c: V4, n: V4) -> Option<Hit> {
        sphere_triangle(&self.body, a, b, c, n, Side::Either).map(|(pen, p)| Self::hit(pen, p))
    }
}

impl Shape for WorldBoarder {
    /// +0x18 (0x257c50): kind 1: twice the body's radius; kind 0: −1 (the first hit is taken)
    fn target(&self) -> f32 {
        if self.kind != 0 {
            self.body.radius + self.body.radius
        } else {
            -1.0
        }
    }
    fn hi(&self) -> V4 {
        self.hi
    }
    fn lo(&self) -> V4 {
        self.lo
    }
    fn mid(&self) -> V4 {
        self.mid
    }
    /// +0x08 (0x257918): the body in use copied and put through `m`
    fn push(&mut self, m: [V4; 4]) {
        self.body = transform(&self.body, m);
        self.local = 1;
    }
    /// +0x10 (0x257978)
    fn pop(&mut self) {
        self.local = 0;
        self.body = self.rider;
    }
    /// +0x20 (0x2578c0)
    fn reject(&self, n: V4) -> bool {
        self.reject.is_some_and(|g| 0.8 < dot(n, g))
    }
    /// +0x40 (0x257ba8)
    fn boxed(&self, lo: V4, hi: V4) -> Option<Hit> {
        boarder_box(&self.body, lo, hi).map(|(pen, p)| Self::hit(pen, p))
    }
    fn sphere_tree(&self, _: usize, _: usize) -> Option<Hit> {
        None
    }
    /// `World_RayCastMeshTris` with +0x28 / +0x30
    fn mesh(&self, m: &CollMesh, either_side: bool) -> Option<Hit> {
        let target = self.target();
        let mut last = 1e30f32;
        let mut best = None;
        for (i, tri) in m.tris.iter().enumerate() {
            let v = |k: usize| m.verts.get(tri[k] as usize).copied().unwrap_or_default();
            let n = m.planes.get(i).copied().unwrap_or_default();
            let h = if either_side { self.triangle_either_side(v(0), v(1), v(2), n) } else { self.triangle(v(0), v(1), v(2), n) };
            let Some(h) = h else { continue };
            let before = last - target;
            last = h.t;
            if (h.t - target).abs() < before.abs() {
                best = Some(h);
                if target < 0.0 {
                    return best;
                }
            }
        }
        if last != 1e30 {
            best
        } else {
            None
        }
    }
}

impl crate::terrain::TriQuery for WorldBoarder {
    fn kind(&self) -> u32 {
        self.kind
    }
    fn bounds(&self) -> (V4, V4) {
        (self.lo, self.hi)
    }
    fn tri(&self, a: V4, b: V4, c: V4, n: V4) -> Option<Hit> {
        self.triangle(a, b, c, n)
    }
    /// its +0x50 is never called: only kind 2 refines
    fn refine(&self, _: f32, _: f32, _: &[V4; 16], _: &mut Hit) {}
}

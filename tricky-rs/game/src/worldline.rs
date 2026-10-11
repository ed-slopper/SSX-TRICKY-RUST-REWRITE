//! The world's line queries (board row E7a): `cWorldLine` (vtable 0x3a7aa8), the shape the ground probe and most
//! other casts send through `World_RayCast`, with its triangle, box and collision-mesh tests, and the vector unit's
//! point-in-triangle program. Checked against the game in the function runner (`tests/worldline.rs`).
//!
//! A query's virtual functions (vtable offset: what): +0x08 work in an instance's frame (the line put through its
//! matrix), +0x10 back to the world, +0x18 the target (t the caller wants hits nearest to), +0x20 reject a hit (no
//! for every shape), +0x28 a triangle, +0x30 a triangle from either side, +0x38 a sphere tree (none for a line),
//! +0x40 a box, +0x48 a patch (the shared one, through the patch cache: row E7c), +0x50 a patch's surface point.

use crate::ps2f::F;

type V4 = [f32; 4];

/// A hit: where, the surface's normal there, and t along the line (0 at its start, 1 at its end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub pos: V4,
    pub normal: V4,
    pub t: f32,
}

/// `cWorldLine` (0xa0 bytes): +0 its kind (the caller's: the ground probe 2), +4 1 while in an instance's frame,
/// +0x10 / +0x20 its box's corners (max, min; w 1), +0x30 the box's middle, +0x50 / +0x60 the start and direction
/// in use, +0x70 / +0x80 the same in the world, +0x90 the target t.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLine {
    pub kind: u32,
    pub local: u32,
    pub hi: V4,
    pub lo: V4,
    pub mid: V4,
    pub start: V4,
    pub dir: V4,
    pub world_start: V4,
    pub world_dir: V4,
    pub target: f32,
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(a: V4, k: f32) -> V4 {
    a.map(|v| v * k)
}

/// vmul, then (x + y) + 1·z + 1·w as the macro code sums it.
fn dot4(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

/// vmula / vmadda by the rows, then vmadd: the matrix's rows weighted by v's lanes, summed in order.
fn through(m: [V4; 4], v: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((m[0][i] * v[0] + m[1][i] * v[1]) + m[2][i] * v[2]) + m[3][i] * v[3])
}

/// The box tests' face normals (0x3d2d10, filled on first use): −x, +x, −y, +y, −z, +z.
const FACES: [V4; 6] = [[-1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

impl WorldLine {
    /// `cWorldLine::cWorldLine` 0x25d7f0(target, from, to, kind): the box of the two ends and its middle
    /// ((min + max) · ½, vdiv 1/2), direction to − from (all four lanes).
    pub fn new(target: f32, from: V4, to: V4, kind: u32) -> WorldLine {
        let d = sub(to, from);
        let mx = |k: usize| if to[k] < from[k] { from[k] } else { to[k] };
        let mn = |k: usize| if from[k] < to[k] { from[k] } else { to[k] };
        let hi = [mx(0), mx(1), mx(2), 1.0];
        let lo = [mn(0), mn(1), mn(2), 1.0];
        let mid = scale(add(lo, hi), 0.5);
        WorldLine { kind, local: 0, hi, lo, mid, start: from, dir: d, world_start: from, world_dir: d, target }
    }

    /// +0x08 (0x25d948): the start and direction put through `m` (an instance's inverse frame).
    pub fn push(&mut self, m: [V4; 4]) {
        self.start = through(m, self.world_start);
        self.dir = through(m, self.world_dir);
        self.local = 1;
    }

    /// +0x10 (0x25d9c8): back to the world's start and direction.
    pub fn pop(&mut self) {
        self.start = self.world_start;
        self.dir = self.world_dir;
        self.local = 0;
    }

    /// +0x28 (0x25d9e0): the triangle a, b, c with normal n (its plane through a). Nothing when the line runs along
    /// the plane (|dir · n| < 1e-10); else t = ((a − start) · n) / (dir · n) is written (`t`, even when the line
    /// misses), and for 0 ≤ t ≤ 1 the point start + dir · t and n are written and the answer is whether the point is
    /// inside (`Tri_ContainsPointVu0`).
    pub fn triangle(&self, a: V4, b: V4, c: V4, n: V4) -> TriTest {
        let den = dot4(self.dir, n);
        if den.abs() < 1e-10 {
            return TriTest { t: None, at: None, hit: false };
        }
        let t = dot4(sub(a, self.start), n) / den;
        if !(0.0 <= t && t <= 1.0) {
            return TriTest { t: Some(t), at: None, hit: false };
        }
        let p = add(self.start, scale(self.dir, t));
        TriTest { t: Some(t), at: Some((p, n)), hit: tri_contains_point(a, b, c, p) }
    }

    /// +0x30 (0x25db18): [`Self::triangle`] from whichever side the line comes (n turned round, all four lanes,
    /// when dir · n < 0).
    pub fn triangle_either_side(&self, a: V4, b: V4, c: V4, n: V4) -> TriTest {
        let n = if dot4(self.dir, n) < 0.0 { scale(n, -1.0) } else { n };
        self.triangle(a, b, c, n)
    }

    /// +0x40 (0x25dc48): the box lo..hi (slabs on x, y, z where |dir| > 1e-10; the line's t from −1 to 2 to
    /// start). The way in when it is at or past 0 and (the way out is past 1 or the way in is nearer the target);
    /// else the way out when it is before 1. With the face's normal.
    pub fn boxed(&self, lo: V4, hi: V4) -> Option<Hit> {
        let (mut near, mut far) = (-1.0f32, 2.0f32);
        let (mut face_in, mut face_out) = (-1i32, -1i32);
        for k in 0..3 {
            let d = self.dir[k];
            if 1e-10 < d.abs() {
                let t1 = (lo[k] - self.start[k]) / d;
                let t2 = (hi[k] - self.start[k]) / d;
                let mut f = 2 * k as i32;
                let (mut a, mut b) = (t1, t2);
                if t2 < t1 {
                    f += 1;
                    a = t2;
                    b = t1;
                }
                let mut fi = f;
                if a <= near {
                    a = near;
                    fi = face_in;
                }
                if b < far {
                    face_out = f ^ 1;
                    far = b;
                }
                if far < a || far < 0.0 {
                    return None;
                }
                near = a;
                face_in = fi;
                if 1.0 < a {
                    return None;
                }
            }
        }
        let at = |t: f32, f: i32| Hit { pos: add(self.start, scale(self.dir, t)), normal: FACES.get(f as usize).copied().unwrap_or([0.0; 4]), t };
        if near < 0.0 || (far <= 1.0 && (near - self.target).abs() <= (far - self.target).abs()) {
            if far < 1.0 {
                Some(at(far, face_out))
            } else {
                None
            }
        } else {
            Some(at(near, face_in))
        }
    }

    /// `World_RayCastMeshTris` 0x25d3d8: an `.ssf` collision mesh's triangles (one-sided, or either side when its
    /// flag says), keeping the hit nearest the target; stops at the first hit when the target is below 0. Each hit
    /// is compared with the last hit's t, not the best so far (the game sets that on every hit).
    pub fn mesh(&self, tris: &[[u32; 3]], verts: &[V4], planes: &[V4], either_side: bool) -> Option<Hit> {
        let target = self.target;
        let mut last = 1e30f32;
        let mut best = None;
        for (i, tri) in tris.iter().enumerate() {
            let v = |k: usize| verts.get(tri[k] as usize).copied().unwrap_or_default();
            let n = planes.get(i).copied().unwrap_or_default();
            let r = if either_side { self.triangle_either_side(v(0), v(1), v(2), n) } else { self.triangle(v(0), v(1), v(2), n) };
            if !r.hit {
                continue;
            }
            let (pos, normal) = r.at.unwrap();
            let t = r.t.unwrap();
            let before = last - target;
            last = t;
            if (t - target).abs() < before.abs() {
                best = Some(Hit { pos, normal, t });
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

/// What a triangle test writes: t (once the line is not along the plane), the point and normal (once t is in
/// 0..1), and the answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriTest {
    pub t: Option<f32>,
    pub at: Option<(V4, V4)>,
    pub hit: bool,
}

/// x·x' + y·y' then + 1·z·z' (vmul.xyz, vadday.x, vmaddz.x with vf5.x = 1).
fn dot3(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2]];
    (p[0] + p[1]) + 1.0 * p[2]
}

/// vopmula / vopmsub: a × b.
fn cross(a: V4, b: V4) -> V4 {
    let acc = [a[1] * b[2], a[2] * b[0], a[0] * b[1]];
    [acc[0] - b[1] * a[2], acc[1] - b[2] * a[0], acc[2] - b[0] * a[1], 0.0]
}

/// `Tri_ContainsPointVu0` 0x23b6b0: the vector unit 0 program at micro address 0x570 (uploaded by the chain at
/// 0x31cac0), read from its code (the runner cannot run micro mode). With e1 = b − a, e2 = c − a, w = p − a and
/// n = e1 × e2: d = e1 · (n × e2) must be beyond ±1e-10 (vclipw against I); then u = (w · (n × e2)) / d and
/// v = (n · (w × e1)) / d, and the sign flags (read four cycles after each result) must say u, v not negative,
/// u − 1 and (u − 1) + v negative. The caller's tolerance argument (f12) is not used.
pub fn tri_contains_point(a: V4, b: V4, c: V4, p: V4) -> bool {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let w = sub(p, a);
    let n = cross(e1, e2);
    let we1 = cross(w, e1);
    let ne2 = cross(n, e2);
    let d = dot3(e1, ne2);
    if !(d > 1e-10 || d < -1e-10) {
        return false;
    }
    let q = 1.0 / d;
    let u = dot3(w, ne2) * q;
    let v = dot3(n, we1) * q;
    let neg = |x: f32| x.is_sign_negative();
    if neg(u) || neg(v) {
        return false;
    }
    let u1 = u - 1.0;
    neg(u1) && neg(u1 + v * 1.0)
}

type P4 = [F; 4];

fn pf(v: V4) -> P4 {
    v.map(F)
}

/// vmula.x / vmadda.y / vmadda.z / vmadd.w: g0·b.x + g1·b.y + g2·b.z + g3·b.w, lane by lane.
fn comb(g: &[P4], b: P4) -> P4 {
    [0, 1, 2, 3].map(|i| ((g[0][i] * b[0] + g[1][i] * b[1]) + g[2][i] * b[2]) + g[3][i] * b[3])
}

/// vmul then (x + y) + 1·z + 1·w.
fn dot_p(a: P4, b: P4) -> F {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + F(1.0) * p[2]) + F(1.0) * p[3]
}

/// The patch's four row vectors combined by `u` (G₀..G₃), then lane `k` of each dotted with `v`.
fn surface(c: &[P4; 16], u: P4, v: P4) -> P4 {
    let g = [comb(&c[0..4], u), comb(&c[4..8], u), comb(&c[8..12], u), comb(&c[12..16], u)];
    let lane = |k: usize| dot_p([g[0][k], g[1][k], g[2][k], g[3][k]], v);
    [lane(0), lane(1), lane(2), F(0.0)]
}

impl WorldLine {
    /// +0x50 (0x25df50): a ground-probe hit on a patch moved onto the patch's surface. From the quad's middle
    /// (u, v) with P(u, v) = Σ (coefficient row k combined by (u³, u², u, 1)) · (v³, v², v, 1), its derivatives
    /// (by (3u², 2u, 1, 0) and (3v², 2v, 1, 0)), four Newton steps on P(u, v) − λ·dir = start, each solved by the
    /// 3 × 3 cofactors over the determinant: (du, dv, dλ) = M · (start − P). A step leaving 0..1 in u or v stops
    /// with nothing changed; after the fourth the point (w 1) and unit(∂P/∂v × ∂P/∂u) replace the hit's (its t
    /// stays). In the PS2's floats (a degenerate patch's determinant can be 0).
    pub fn refine(&self, u0: f32, v0: f32, coeffs: &[V4; 16], hit: &mut Hit) {
        let c: [P4; 16] = coeffs.map(pf);
        let one = F(1.0);
        let (mut u, mut v) = (F(u0), F(v0));
        let basis = |s: F| {
            let s2 = s * s;
            [s2 * s, s2, s, one]
        };
        let slope = |s: F| [(s * F(3.0)) * s, s + s, one, F(0.0)];
        let eval = |u: F, v: F| {
            let (bu, bv) = (basis(u), basis(v));
            let p = surface(&c, bu, bv);
            let dv = surface(&c, bu, slope(v));
            let du = surface(&c, slope(u), bv);
            ([p[0], p[1], p[2], one], du, dv)
        };
        let (mut p, mut a, mut b) = eval(u, v);
        let start = pf(self.start);
        for _ in 0..4 {
            let r = [start[0] - p[0], start[1] - p[1], start[2] - p[2]];
            let (cx, cy, cz) = (-F(self.dir[0]), -F(self.dir[1]), -F(self.dir[2]));
            let (ax, ay, az) = (a[0], a[1], a[2]);
            let (bx, by, bz) = (b[0], b[1], b[2]);
            let f9 = ay * bz - az * by;
            let f13 = cx * f9;
            let f5 = cy * az - cz * ay;
            let f4 = by * cz - bz * cy;
            let det = (ax * f4 + bx * f5) + f13;
            let inv = one / det;
            let m = [
                f4 * inv,
                f5 * inv,
                f9 * inv,
                (cx * bz - cz * bx) * inv,
                (ax * cz - az * cx) * inv,
                (bx * az - bz * ax) * inv,
                (bx * cy - by * cx) * inv,
                (cx * ay - cy * ax) * inv,
                (ax * by - ay * bx) * inv,
            ];
            let du = (m[0] * r[0] + m[3] * r[1]) + m[6] * r[2];
            let dv = (m[1] * r[0] + m[4] * r[1]) + m[7] * r[2];
            u = u + du;
            v = v + dv;
            if u < F(0.0) || one < u || v < F(0.0) || one < v {
                return;
            }
            (p, a, b) = eval(u, v);
        }
        // vopmula / vopmsub (∂P/∂v × ∂P/∂u), w 0, then vrsqrt and vmulq
        let acc = [b[1] * a[2], b[2] * a[0], b[0] * a[1]];
        let n = [acc[0] - a[1] * b[2], acc[1] - a[2] * b[0], acc[2] - a[0] * b[1], F(0.0)];
        let q = one / dot_p(n, n).sqrt();
        hit.pos = p.map(|x| x.0);
        hit.normal = n.map(|x| (x * q).0);
    }
}

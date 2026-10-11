//! The terrain's collision (board row E7c1): a patch tessellated into a 10 × 10 grid of points by the vector unit
//! (the patch cache's VU0 program), its quads' boxes and triangle normals, and the shared patch test
//! (`cWorldIntersectType` +0x48, 0x25c7c8) that finds the first triangle a query hits, trying the last hit's quad
//! first. Checked against the game in the function runner (`tests/terrain.rs`).

use crate::worldline::{Hit, WorldLine};

type V4 = [f32; 4];

/// The power basis the patch program reads (VU0 data qwords 16..25, uploaded by the chain at 0x31d1d0):
/// (t³, t², t, 1) for t = k/9 as the file has them.
pub const BASIS: [V4; 10] = [
    [0.0, 0.0, 0.0, 1.0],
    [f32::from_bits(0x3ab3cc07), f32::from_bits(0x3c4a4588), f32::from_bits(0x3de38e39), 1.0],
    [f32::from_bits(0x3c33cc07), f32::from_bits(0x3d4a4588), f32::from_bits(0x3e638e39), 1.0],
    [f32::from_bits(0x3d17b426), f32::from_bits(0x3de38e39), f32::from_bits(0x3eaaaaaa), 1.0],
    [f32::from_bits(0x3db3cc07), f32::from_bits(0x3e4a4588), f32::from_bits(0x3ee38e39), 1.0],
    [f32::from_bits(0x3e2f953f), f32::from_bits(0x3e9e0652), f32::from_bits(0x3f0e38e3), 1.0],
    [f32::from_bits(0x3e97b426), f32::from_bits(0x3ee38e39), f32::from_bits(0x3f2aaaaa), 1.0],
    [f32::from_bits(0x3ef0e65d), f32::from_bits(0x3f1add3c), f32::from_bits(0x3f471c71), 1.0],
    [f32::from_bits(0x3f33cc07), f32::from_bits(0x3f4a4588), f32::from_bits(0x3f638e39), 1.0],
    [1.0, 1.0, 1.0, 1.0],
];

/// vmula.x / vmadda.y / vmadda.z / vmadd.w: a·b.x + c·b.y + d·b.z + e·b.w, lane by lane, in that order.
fn comb(v: [V4; 4], b: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((v[0][i] * b[0] + v[1][i] * b[1]) + v[2][i] * b[2]) + v[3][i] * b[3])
}

/// The patch program (VU0 micro address 0, `FUN_002576a8` loads the patch's 16 coefficient vectors (+0x50) into
/// vf1..vf16): for each row i, four vectors C_k = vf(4k+1..4k+4) combined by basis i, then each point the C's
/// combined by basis j; x, y, z written (w stays the basis' 1). Point i·10 + j.
pub fn tessellate(c: &[V4; 16]) -> [V4; 100] {
    let mut out = [[0.0f32; 4]; 100];
    for i in 0..10 {
        let b = BASIS[i];
        // vf19..vf22 get x, y, z by move.xyz (their w lanes never reach a point: only x, y, z are written)
        let cols: [V4; 4] = [0, 1, 2, 3].map(|k| {
            let g = |m: usize| {
                let v = c[4 * k + m];
                [v[0], v[1], v[2], 0.0]
            };
            comb([g(0), g(1), g(2), g(3)], b)
        });
        for j in 0..10 {
            let p = comb(cols, BASIS[j]);
            out[i * 10 + j] = [p[0], p[1], p[2], 1.0];
        }
    }
    out
}

/// One quad's cached record (0x40 bytes): the normals of triangles A (p11, p10, p01) and B (p01, p10, p00), and
/// its box. Each is worked out the first time it is needed (1e10 marks "not yet").
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Quad {
    pub normal_a: Option<V4>,
    pub normal_b: Option<V4>,
    pub min: Option<[f32; 3]>,
    pub max: Option<[f32; 3]>,
}

/// A patch's cache entry (0x1cd0 bytes): +0 the patch, +0x10 the points, +0x650 the 9 × 9 fine quads, +0x1a90 the
/// 3 × 3 coarse ones (every third point).
#[derive(Clone, Debug, PartialEq)]
pub struct PatchGrid {
    /// the patch's coefficients (+0x50), for the kind-2 refinement
    pub coeffs: [V4; 16],
    pub points: [V4; 100],
    pub fine: [Quad; 81],
    pub coarse: [Quad; 9],
}

impl PatchGrid {
    pub fn new(coeffs: &[V4; 16]) -> PatchGrid {
        PatchGrid { coeffs: *coeffs, points: tessellate(coeffs), fine: [Quad::default(); 81], coarse: [Quad::default(); 9] }
    }
}

/// The last patch hit (the caller's record, e.g. the rider's +0x430): +0 the patch, +4 / +6 the quad's row and
/// column, +8 1 for triangle A, +0xc the query's kind; +0x10 / +0x12 / +0x14 the cell, INode and list index
/// `World_RayCast` notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LastHit {
    pub patch: u32,
    pub row: i16,
    pub col: i16,
    pub tri_a: u32,
    pub kind: u32,
    pub cell: i16,
    pub node: i16,
    pub index: i16,
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// vopmula / vopmsub, then vsub.w: a × b with w 0.
fn cross(a: V4, b: V4) -> V4 {
    let acc = [a[1] * b[2], a[2] * b[0], a[0] * b[1]];
    [acc[0] - b[1] * a[2], acc[1] - b[2] * a[0], acc[2] - b[0] * a[1], 0.0]
}

/// vmul, (x + y) + 1·z + 1·w, vrsqrt (1/√0 is the largest float), vmulq.
fn unit(v: V4) -> V4 {
    let p = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let s = ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3];
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    v.map(|x| x * q)
}

/// The box of four points: each lane the least (greatest) of (p00 or p10) and (p01 or p11), as the game compares.
fn bounds(p00: V4, p10: V4, p01: V4, p11: V4) -> ([f32; 3], [f32; 3]) {
    let lo = |k: usize| {
        let a = if p10[k] <= p00[k] { p10[k] } else { p00[k] };
        let b = if p11[k] <= p01[k] { p11[k] } else { p01[k] };
        if b <= a {
            b
        } else {
            a
        }
    };
    let hi = |k: usize| {
        let a = if p00[k] <= p10[k] { p10[k] } else { p00[k] };
        let b = if p01[k] <= p11[k] { p11[k] } else { p01[k] };
        if a <= b {
            b
        } else {
            a
        }
    };
    ([lo(0), lo(1), lo(2)], [hi(0), hi(1), hi(2)])
}

/// A patch hit: the hit (for a kind-2 query refined by its +0x50 at the quad's middle (u, v), row E7c2, which is
/// also given).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchHit {
    pub hit: Hit,
    pub refine: Option<(f32, f32)>,
}

/// `cWorldIntersectType` +0x48 (0x25c7c8) for a line: kind 0 tests the coarse quads (3 × 3, every third point),
/// any other kind the fine ones (9 × 9). With `last` naming this patch and kind, its quad's triangle is tried
/// first (no box test); a miss clears `last` and every quad is tried in order (rows, then columns): its box (made
/// the first time) against the line's, then triangle A (p11, p10, p01), then B (p01, p10, p00), each with its
/// normal (made the first time: unit((p11 − p10) × (p10 − p01)), unit((p01 − p10) × (p10 − p00))). The first hit
/// is the answer (noted in `last`); kind 2 then refines it at the quad's middle ([`WorldLine::refine`]).
pub fn patch_test(q: &WorldLine, grid: &mut PatchGrid, patch: u32, mut last: Option<&mut LastHit>) -> Option<PatchHit> {
    let coarse = q.kind == 0;
    let (step, n, third, sixth) = if coarse { (3usize, 3usize, 0.333_333_34f32, 0.166_666_67f32) } else { (1, 9, 0.111_111_11, 0.055_555_556) };
    let pts = grid.points;
    let corners = |i: usize, j: usize| {
        let p = |r: usize, c: usize| pts[r * 10 + c];
        (p(i * step, j * step), p(i * step + step, j * step), p(i * step, j * step + step), p(i * step + step, j * step + step))
    };
    let coeffs = grid.coeffs;
    let done = |mut hit: Hit, i: usize, j: usize| {
        let at = if q.kind == 2 { Some((i as f32 * third + sixth, j as f32 * third + sixth)) } else { None };
        if let Some((u, v)) = at {
            q.refine(u, v, &coeffs, &mut hit);
        }
        PatchHit { hit, refine: at }
    };
    let quads: &mut [Quad] = if coarse { &mut grid.coarse } else { &mut grid.fine };
    let fill_box = |qd: &mut Quad, c: (V4, V4, V4, V4)| {
        if qd.min.is_none() {
            let (lo, hi) = bounds(c.0, c.1, c.2, c.3);
            qd.min = Some(lo);
            qd.max = Some(hi);
        }
    };
    let try_a = |qd: &mut Quad, c: (V4, V4, V4, V4)| {
        let (_, p10, p01, p11) = c;
        let n = *qd.normal_a.get_or_insert_with(|| unit(cross(sub(p11, p10), sub(p10, p01))));
        let r = q.triangle(p11, p10, p01, n);
        if r.hit {
            let (pos, normal) = r.at.unwrap();
            Some(Hit { pos, normal, t: r.t.unwrap() })
        } else {
            None
        }
    };
    let try_b = |qd: &mut Quad, c: (V4, V4, V4, V4)| {
        let (p00, p10, p01, _) = c;
        let n = *qd.normal_b.get_or_insert_with(|| unit(cross(sub(p01, p10), sub(p10, p00))));
        let r = q.triangle(p01, p10, p00, n);
        if r.hit {
            let (pos, normal) = r.at.unwrap();
            Some(Hit { pos, normal, t: r.t.unwrap() })
        } else {
            None
        }
    };
    if let Some(l) = last.as_deref_mut() {
        if l.patch == patch && l.kind == q.kind {
            let (i, j) = (l.row as u16 as usize, l.col as u16 as usize);
            let c = corners(i, j);
            let qd = &mut quads[i * n + j];
            fill_box(qd, c);
            let h = if l.tri_a == 0 { try_b(qd, c) } else { try_a(qd, c) };
            if let Some(hit) = h {
                return Some(done(hit, i, j));
            }
            *l = LastHit { patch: 0, row: 0, col: 0, tri_a: 0, kind: 0, cell: -1, node: -1, index: -1 };
        }
    }
    let (lo, hi) = (q.lo, q.hi);
    for i in 0..n {
        for j in 0..n {
            let c = corners(i, j);
            let qd = &mut quads[i * n + j];
            fill_box(qd, c);
            let (mn, mx) = (qd.min.unwrap(), qd.max.unwrap());
            if !(0..3).all(|k| mn[k] < hi[k] && lo[k] < mx[k]) {
                continue;
            }
            for a in [true, false] {
                let h = if a { try_a(qd, c) } else { try_b(qd, c) };
                if let Some(hit) = h {
                    if let Some(l) = last.as_deref_mut() {
                        l.patch = patch;
                        l.kind = q.kind;
                        l.row = i as i16;
                        l.col = j as i16;
                        l.tri_a = a as u32;
                    }
                    return Some(done(hit, i, j));
                }
            }
        }
    }
    None
}

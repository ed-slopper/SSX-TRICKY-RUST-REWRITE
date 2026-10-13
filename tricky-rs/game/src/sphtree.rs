//! Sphere trees in the world (board row E7a4): an object's octree of spheres (`.ssf`, row E3c2) as the casts test
//! the rider's collision body against it: `SphereTree::SphereTree` 0x239ba8 (the eight child directions from the
//! table 0x3439d0, filled by start-up code 0x23bab0), `SphTree_SetTransform` 0x239d60, `SphTree_RleDecode`
//! 0x239b20 (a packed tree's occupancy bytes, `SphereTreeCache_Get` 0x238f50), `SphTree_SphereRecurse` 0x23a728
//! and `Collide_BoarderSphTree` 0x2394a8. Checked in the function runner (`tests/sphtree.rs`).

use crate::collbody::CollBody;

type V4 = [f32; 4];

/// 0x3439d0: the octants' directions (±1 on each axis; x the high bit of the index).
pub const DIRS: [V4; 8] = [
    [-1.0, -1.0, -1.0, 0.0],
    [-1.0, -1.0, 1.0, 0.0],
    [-1.0, 1.0, -1.0, 0.0],
    [-1.0, 1.0, 1.0, 0.0],
    [1.0, -1.0, -1.0, 0.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, 1.0, -1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0],
];

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| x * k)
}

fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn through(m: [V4; 4], v: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((m[0][i] * v[0] + m[1][i] * v[1]) + m[2][i] * v[2]) + m[3][i] * v[3])
}

/// `SphTree_RleDecode` 0x239b20: runs of a count byte c: below 0, the next −c bytes as they are; above 0, the next
/// byte c + 1 times; 0 ends.
pub fn rle_decode(src: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    let mut p = 0;
    loop {
        let c = *src.get(p).unwrap_or(&0) as i8;
        p += 1;
        if c < 0 {
            for _ in 0..-(c as i32) {
                out.push(*src.get(p).unwrap_or(&0));
                p += 1;
            }
        } else if c == 0 {
            return out;
        } else {
            let b = *src.get(p).unwrap_or(&0);
            for _ in 0..c as i32 + 1 {
                out.push(b);
            }
            p += 1;
        }
    }
}

/// A sphere tree in the world (0x150 bytes): +0x10 the children's directions, +0x90 its centre (w 1), +0xa0 its
/// header (+8 packed, +0xc depth, +0x20 the levels (radius, half-size, child stride), +0x28 the occupancy bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct SphereTree {
    pub dirs: [V4; 8],
    pub centre: V4,
    /// the file's centre (header +0x14), what `SphTree_SetTransform` moves
    pub home: V4,
    pub depth: u32,
    pub levels: Vec<(f32, f32, i32)>,
    /// one byte per node: which of its eight children exist
    pub masks: Vec<u8>,
}

impl SphereTree {
    /// `SphereTree::SphereTree` then `SphTree_Load` 0x239340 (and, for a packed tree, the cache's decoding).
    pub fn new(t: &tricky_data::ssf::SphTree) -> SphereTree {
        let home = [t.centre[0], t.centre[1], t.centre[2], 1.0];
        let masks = if t.flag == 0 { t.data.clone() } else { rle_decode(&t.data) };
        SphereTree { dirs: DIRS, centre: home, home, depth: t.depth, levels: t.levels.clone(), masks }
    }

    /// `SphTree_SetTransform` 0x239d60: directions 0..3 put through `m` (4..7 their opposites), the centre the
    /// file's put through `m`.
    pub fn set_transform(&mut self, m: [V4; 4]) {
        for k in 0..4 {
            self.dirs[k] = through(m, DIRS[k]);
            self.dirs[7 - k] = scale(self.dirs[k], -1.0);
        }
        self.centre = through(m, self.home);
    }

    /// `SphTree_SphereRecurse` 0x23a728: a sphere (centre `c`, radius `r`) against node `index` of `level` at
    /// `node`: within r + the level's radius, either its children (when the level is above the depth and the node
    /// has any: centre + direction · the next level's half-size, child k at (k + 1)·stride + index) until one
    /// answers, or it is a leaf: the penetration n·(R − |c − node|) and the contact c − n·(r − that).
    pub fn sphere(&self, r: f32, c: V4, node: V4, level: usize, index: i32) -> Option<(V4, V4)> {
        let d = sub(c, node);
        let rr = r + self.levels.get(level).map_or(0.0, |l| l.0);
        let dd = dot(d, d);
        if !(dd < rr * rr) {
            return None;
        }
        let occ = *self.masks.get(index.max(0) as usize).unwrap_or(&0);
        if (level as u32) < self.depth && occ != 0 {
            let half = self.levels.get(level + 1).map_or(0.0, |l| l.1);
            let stride = self.levels.get(level).map_or(0, |l| l.2);
            for k in 0..8 {
                if (occ >> k) & 1 == 0 {
                    continue;
                }
                let child = add(node, scale(self.dirs[k], half));
                let at = (k as i32 + 1).wrapping_mul(stride).wrapping_add(index);
                if let Some(h) = self.sphere(r, c, child, level + 1, at) {
                    return Some(h);
                }
            }
            return None;
        }
        let q = if dd == 0.0 { f32::MAX } else { 1.0 / dd.sqrt() };
        let n = scale(d, q);
        let f = rr - dd.sqrt();
        Some((scale(n, f), sub(c, scale(n, r - f))))
    }

    /// `Collide_BoarderSphTree` 0x2394a8: the body's bounding sphere, then (when it touches and the body has
    /// spheres) each sphere in turn, every one, mask or no: the first that touches.
    pub fn body(&self, b: &CollBody) -> Option<(V4, V4)> {
        let first = self.sphere(b.radius, b.center, self.centre, 0, 0)?;
        if b.count == 0 {
            return Some(first);
        }
        for (c, r) in b.spheres.iter().take(b.count.max(0) as usize) {
            if let Some(h) = self.sphere(*r, *c, self.centre, 0, 0) {
                return Some(h);
            }
        }
        None
    }
}

/// `Sort_ShellPairs` 0x2cfd90: a shell sort (gaps 1, 4, 13, …) of `keys`, largest first, carrying `vals`.
fn shell_sort(keys: &mut [i32], vals: &mut [usize]) {
    let n = keys.len() as i32;
    const GAPS: [i32; 10] = [1, 4, 13, 40, 121, 364, 1093, 3280, 9841, 29524];
    let mut g = 1usize;
    if GAPS[1] < n {
        g = 2;
        while GAPS[g] < n {
            g += 1;
        }
    }
    loop {
        g -= 1;
        let gap = GAPS[g];
        let mut i = gap;
        while i < n {
            let (k, v) = (keys[i as usize], vals[i as usize]);
            let mut j = i - gap;
            loop {
                if k <= keys[j as usize] {
                    break;
                }
                keys[(j + gap) as usize] = keys[j as usize];
                vals[(j + gap) as usize] = vals[j as usize];
                j -= gap;
                if j < 0 {
                    break;
                }
            }
            keys[(j + gap) as usize] = k;
            vals[(j + gap) as usize] = v;
            i += 1;
        }
        if gap < 2 {
            return;
        }
    }
}

/// A segment from `a` along `e` misses a sphere (|w|² = `ww`, r² = `r2`): f = (w · e)/|e| below 0, or
/// |w|² − f² beyond r², or |e| < f − √(r² − (|w|² − f²)).
fn edge_misses(ww: f32, w: V4, e: V4, r2: f32) -> bool {
    let len = dot(e, e).sqrt();
    let f = dot(w, e) / len;
    if !(0.0 <= f) {
        return true;
    }
    let q = ww - f * f;
    if !(q <= r2) {
        return true;
    }
    len < f - (r2 - q).sqrt()
}

/// What a tree test leaves: a penetration and contact, or (some tree-against-tree paths) a yes with nothing
/// written.
pub type TreeHit = Option<(V4, V4)>;

impl SphereTree {
    /// `SphTree_SortOctants` 0x23b398: the order the children are tried in for a triangle, by the directions'
    /// (int)(d · (−1000·n)) (the opposite directions get the negatives), largest first.
    pub fn octant_order(&self, n: V4) -> [usize; 8] {
        let m = scale(n, -1000.0);
        let mut keys = [0i32; 8];
        let mut vals = [0usize; 8];
        for k in 0..4 {
            let d = dot(self.dirs[k], m);
            let i = if d.is_nan() {
                0
            } else if d >= i32::MAX as f32 {
                i32::MAX
            } else if d <= i32::MIN as f32 {
                i32::MIN
            } else {
                d.trunc() as i32
            };
            keys[k] = i;
            keys[7 - k] = i.wrapping_neg();
            vals[k] = k;
            vals[7 - k] = 7 - k;
        }
        shell_sort(&mut keys, &mut vals);
        vals
    }

    /// `SphTree_IntersectTriangle` 0x23a390: the deepest leaf through the triangle (best from 0: only
    /// penetrations count), as `SphTree_TriangleRecurse` 0x23ada0 finds it.
    pub fn triangle(&self, a: V4, b: V4, c: V4, n: V4) -> Option<(V4, V4)> {
        let order = self.octant_order(n);
        let mut best = 0.0f32;
        let mut out = None;
        self.tri_recurse(a, b, c, n, self.centre, 0, 0, &order, &mut best, &mut out);
        if best < 0.0 {
            out
        } else {
            None
        }
    }

    /// `SphTree_TriangleRecurse` 0x23ada0: nothing for the root behind the plane, nor for a node that cannot beat
    /// the best (radius + best ≤ d). A node is a leaf at the depth, past level 2 or with no children. Its foot on
    /// the plane inside the triangle goes on; else (not for a leaf, nor a radius under 20) the corner a inside it,
    /// or an edge ab, ac or bc through it. A leaf keeps d − r when deeper than the best; others try their children
    /// in `order`.
    #[allow(clippy::too_many_arguments)]
    fn tri_recurse(&self, a: V4, b: V4, c: V4, n: V4, node: V4, level: usize, index: i32, order: &[usize; 8], best: &mut f32, out: &mut Option<(V4, V4)>) {
        let d = dot(n, sub(node, a));
        let r = self.levels.get(level).map_or(0.0, |l| l.0);
        if level == 0 && d < 0.0 {
            return;
        }
        if r + *best <= d {
            return;
        }
        let occ = *self.masks.get(index.max(0) as usize).unwrap_or(&0);
        let leaf = self.depth as usize <= level || 2 < level || occ == 0;
        let p = sub(node, scale(n, d));
        if !crate::worldline::tri_contains_point(a, b, c, p) {
            if leaf || r < 20.0 {
                return;
            }
            let w = sub(node, a);
            let r2 = r * r;
            let ww = dot(w, w);
            if !(ww <= r2) && edge_misses(ww, w, sub(b, a), r2) && edge_misses(ww, w, sub(c, a), r2) {
                let w2 = sub(node, b);
                if edge_misses(dot(w2, w2), w2, sub(c, b), r2) {
                    return;
                }
            }
        }
        if leaf {
            let depth = d - r;
            if depth < *best {
                *out = Some((scale(n, -depth), p));
                *best = depth;
            }
            return;
        }
        let half = self.levels.get(level + 1).map_or(0.0, |l| l.1);
        let stride = self.levels.get(level).map_or(0, |l| l.2);
        for &k in order {
            if (occ >> k) & 1 == 0 {
                continue;
            }
            let child = add(node, scale(self.dirs[k], half));
            let at = (k as i32 + 1).wrapping_mul(stride).wrapping_add(index);
            self.tri_recurse(a, b, c, n, child, level + 1, at, order, best, out);
        }
    }

    /// `SphTree_TouchesTriangle` 0x2395c0: when the centre is within the tree's radius of the plane, the
    /// triangle test with the normal turned to face the centre.
    pub fn triangle_either_side(&self, a: V4, b: V4, c: V4, n: V4) -> Option<(V4, V4)> {
        let d = dot(n, sub(self.centre, a));
        let r = self.levels.first().map_or(0.0, |l| l.0);
        if !(d.abs() <= r) {
            return None;
        }
        let n = if d < 0.0 { scale(n, -1.0) } else { n };
        self.triangle(a, b, c, n)
    }

    /// `SphTree_IntersectBox` 0x23a210 / `SphTree_BoxRecurse` 0x23a9a0: each node sphere against the box
    /// (`Collide_SphereBox`); the first leaf that touches: penetration normal·depth, contact the face point.
    pub fn boxed(&self, lo: V4, hi: V4) -> Option<(V4, V4)> {
        self.box_recurse(lo, hi, self.centre, 0, 0)
    }

    fn box_recurse(&self, lo: V4, hi: V4, node: V4, level: usize, index: i32) -> Option<(V4, V4)> {
        let r = self.levels.get(level).map_or(0.0, |l| l.0);
        let (p, nn, depth) = crate::worldboarder::sphere_box(r, lo, hi, node)?;
        let occ = *self.masks.get(index.max(0) as usize).unwrap_or(&0);
        if (level as u32) < self.depth && occ != 0 {
            let half = self.levels.get(level + 1).map_or(0.0, |l| l.1);
            let stride = self.levels.get(level).map_or(0, |l| l.2);
            for k in 0..8 {
                if (occ >> k) & 1 == 0 {
                    continue;
                }
                let child = add(node, scale(self.dirs[k], half));
                let at = (k as i32 + 1).wrapping_mul(stride).wrapping_add(index);
                if let Some(h) = self.box_recurse(lo, hi, child, level + 1, at) {
                    return Some(h);
                }
            }
            return None;
        }
        Some((scale(nn, depth), p))
    }

    /// `SphTree_IntersectTree` 0x23a490 / `SphTree_TreeRecurse` 0x23a570: this tree nodes (no test at its root)
    /// against `other` by `SphTree_SphereRecurse`, each child as a sphere of this level radius, recursing where it
    /// touches; a node with no children, at the depth or past level 3 answers yes without writing anything (the
    /// last touch written stays).
    pub fn tree(&self, other: &SphereTree) -> Option<TreeHit> {
        let mut last = None;
        if self.tree_recurse(other, self.centre, 0, 0, &mut last) {
            Some(last)
        } else {
            None
        }
    }

    fn tree_recurse(&self, other: &SphereTree, node: V4, level: usize, index: i32, last: &mut TreeHit) -> bool {
        let occ = *self.masks.get(index.max(0) as usize).unwrap_or(&0);
        if !((level as u32) < self.depth) || 3 < level || occ == 0 {
            return true;
        }
        let half = self.levels.get(level + 1).map_or(0.0, |l| l.1);
        let r = self.levels.get(level).map_or(0.0, |l| l.0);
        let stride = self.levels.get(level).map_or(0, |l| l.2);
        for k in 0..8 {
            if (occ >> k) & 1 == 0 {
                continue;
            }
            let child = add(node, scale(self.dirs[k], half));
            if let Some(h) = other.sphere(r, child, other.centre, 0, 0) {
                *last = Some(h);
                let at = (k as i32 + 1).wrapping_mul(stride).wrapping_add(index);
                if self.tree_recurse(other, child, level + 1, at, last) {
                    return true;
                }
            }
        }
        false
    }

    /// `SphTree_Transform` 0x239f88: directions 0..3 as they are now put through `m` (4..7 their opposites), the
    /// centre as it is now.
    pub fn transform(&mut self, m: [V4; 4]) {
        for k in 0..4 {
            self.dirs[k] = through(m, self.dirs[k]);
            self.dirs[7 - k] = scale(self.dirs[k], -1.0);
        }
        self.centre = through(m, self.centre);
    }
}

/// `cWorldSphTree` (vtable 0x3a85f8): a sphere tree as the query of a cast (the rollers'): +0 its kind (0), +4 in
/// a frame, +0x10 / +0x20 the tree's box (centre ± its radius, w 1), +0x30 its centre, +0x50 the tree in use (+0x54
/// the world's, +0x60 a copy put in a frame).
#[derive(Clone, Debug, PartialEq)]
pub struct WorldSphTree {
    pub kind: u32,
    pub local: u32,
    pub hi: V4,
    pub lo: V4,
    pub mid: V4,
    pub tree: SphereTree,
    pub world: SphereTree,
}

impl WorldSphTree {
    /// `cWorldSphTree::cWorldSphTree` 0x261638(tree)
    pub fn new(t: &SphereTree) -> WorldSphTree {
        let r = t.levels.first().map_or(0.0, |l| l.0);
        let c = t.centre;
        WorldSphTree { kind: 0, local: 0, hi: [c[0] + r, c[1] + r, c[2] + r, 1.0], lo: [c[0] - r, c[1] - r, c[2] - r, 1.0], mid: c, tree: t.clone(), world: t.clone() }
    }

    fn hit(pen: V4, contact: V4) -> crate::worldline::Hit {
        let depth = dot(pen, pen).sqrt();
        let q = if depth == 0.0 { f32::MAX } else { 1.0 / depth };
        crate::worldline::Hit { pos: contact, normal: scale(pen, q), t: depth }
    }

    /// +0x28 (0x261770): `SphTree_IntersectTriangle`
    pub fn triangle(&self, a: V4, b: V4, c: V4, n: V4) -> Option<crate::worldline::Hit> {
        self.tree.triangle(a, b, c, n).map(|(p, q)| Self::hit(p, q))
    }

    /// +0x30 (0x261818): `SphTree_TouchesTriangle`
    pub fn triangle_either_side(&self, a: V4, b: V4, c: V4, n: V4) -> Option<crate::worldline::Hit> {
        self.tree.triangle_either_side(a, b, c, n).map(|(p, q)| Self::hit(p, q))
    }

    /// +0x38 (0x2618c0): `SphTree_IntersectTree`; `Some(None)` is the game's yes with nothing written (its
    /// outputs are then whatever the stack held).
    pub fn tree_hit(&self, other: &SphereTree) -> Option<Option<crate::worldline::Hit>> {
        self.tree.tree(other).map(|h| h.map(|(p, q)| Self::hit(p, q)))
    }
}

impl crate::world::Shape for WorldSphTree {
    /// +0x18 (0x261a10): kind 0, so −1
    fn target(&self) -> f32 {
        if self.kind == 0 {
            -1.0
        } else {
            let r = self.tree.levels.first().map_or(0.0, |l| l.0);
            r + r
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
    /// +0x08 (0x261700): the tree in use copied and put through `m`
    fn push(&mut self, m: [V4; 4]) {
        self.tree.transform(m);
        self.local = 1;
    }
    /// +0x10 (0x261760)
    fn pop(&mut self) {
        self.local = 0;
        self.tree = self.world.clone();
    }
    fn reject(&self, _: V4) -> bool {
        false
    }
    /// +0x40 (0x261968)
    fn boxed(&self, lo: V4, hi: V4) -> Option<crate::worldline::Hit> {
        self.tree.boxed(lo, hi).map(|(p, q)| Self::hit(p, q))
    }
    /// +0x38: a yes with nothing written counts as no here
    fn sphere_tree(&self, other: &SphereTree) -> Option<crate::worldline::Hit> {
        self.tree_hit(other).flatten()
    }
    fn mesh(&self, m: &crate::world::CollMesh, either_side: bool) -> Option<crate::worldline::Hit> {
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
    fn patch(&self, grid: &mut crate::terrain::PatchGrid, id: u32, last: Option<&mut crate::terrain::LastHit>) -> Option<crate::terrain::PatchHit> {
        crate::terrain::patch_test(self, grid, id, last)
    }
}

impl crate::terrain::TriQuery for WorldSphTree {
    fn kind(&self) -> u32 {
        self.kind
    }
    fn bounds(&self) -> (V4, V4) {
        (self.lo, self.hi)
    }
    fn tri(&self, a: V4, b: V4, c: V4, n: V4) -> Option<crate::worldline::Hit> {
        self.triangle(a, b, c, n)
    }
    fn refine(&self, _: f32, _: f32, _: &[V4; 16], _: &mut crate::worldline::Hit) {}
}

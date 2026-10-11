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

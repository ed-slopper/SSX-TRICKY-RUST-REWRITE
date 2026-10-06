//! Triangle-soup collision in Bevy space (metres, Y up): a uniform XZ grid of triangles with
//! "what is the ground under this point" and "push this sphere out of walls" queries.

use bevy::math::Vec3;
use std::collections::HashMap;

/// Terrain faces at least this upright (normal.y) can be ridden; anything steeper is a wall.
pub const GROUND_MIN_Y: f32 = 0.42;
/// Objects (rocks, buildings, ramps) need to be flatter than terrain before you can ride on them,
/// so that boulders and tree trunks deflect you instead of being climbed.
pub const OBJECT_GROUND_MIN_Y: f32 = 0.72;
const CELL: f32 = 6.0;

pub struct Tri {
    pub p: [Vec3; 3],
    /// per-vertex normals (smooth for terrain, the face normal for object meshes)
    pub n: [Vec3; 3],
    pub face: Vec3,
    /// can be ridden on (otherwise it is a wall)
    pub rideable: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GroundHit { pub y: f32, pub normal: Vec3 }

#[derive(Default)]
pub struct CollisionWorld {
    tris: Vec<Tri>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    pub min_y: f32,
}

fn cell(v: f32) -> i32 { (v / CELL).floor() as i32 }

impl CollisionWorld {
    pub fn len(&self) -> usize { self.tris.len() }

    /// `normals`: smooth vertex normals, or None to use the face normal. `terrain` faces are
    /// flipped to point up if needed (patch winding is not consistent).
    pub fn add(&mut self, p: [Vec3; 3], normals: Option<[Vec3; 3]>, terrain: bool) {
        let mut face = (p[1] - p[0]).cross(p[2] - p[0]);
        if face.length_squared() < 1e-10 { return; }
        face = face.normalize();
        if terrain && face.y < 0.0 { face = -face; }
        // Smooth normals come from the Bezier patch; where a patch is pinched to a point they are
        // garbage, so fall back to the flat normal whenever they disagree with the triangle.
        let n = normals.map(|n| n.map(|v| if v.dot(face) > 0.75 { v } else { face })).unwrap_or([face; 3]);
        let id = self.tris.len() as u32;
        let (min, max) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
        self.min_y = self.min_y.min(min.y);
        for cx in cell(min.x)..=cell(max.x) {
            for cz in cell(min.z)..=cell(max.z) {
                self.grid.entry((cx, cz)).or_default().push(id);
            }
        }
        let rideable = face.y >= if terrain { GROUND_MIN_Y } else { OBJECT_GROUND_MIN_Y };
        self.tris.push(Tri { p, n, face, rideable });
    }

    /// Highest rideable surface under/around `pos` whose height lies in [pos.y - down, pos.y + up].
    pub fn ground(&self, pos: Vec3, up: f32, down: f32) -> Option<GroundHit> {
        let ids = self.grid.get(&(cell(pos.x), cell(pos.z)))?;
        let mut best: Option<GroundHit> = None;
        for &id in ids {
            let t = &self.tris[id as usize];
            if !t.rideable { continue; }
            // barycentric coordinates in the XZ plane
            let (a, b, c) = (t.p[0], t.p[1], t.p[2]);
            let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
            if det.abs() < 1e-9 { continue; }
            let w0 = ((b.z - c.z) * (pos.x - c.x) + (c.x - b.x) * (pos.z - c.z)) / det;
            let w1 = ((c.z - a.z) * (pos.x - c.x) + (a.x - c.x) * (pos.z - c.z)) / det;
            let w2 = 1.0 - w0 - w1;
            const E: f32 = -1e-4;
            if w0 < E || w1 < E || w2 < E { continue; }
            let y = a.y * w0 + b.y * w1 + c.y * w2;
            if y > pos.y + up || y < pos.y - down { continue; }
            if best.is_none_or(|h| y > h.y) {
                let mut n = (t.n[0] * w0 + t.n[1] * w1 + t.n[2] * w2).normalize_or(t.face);
                if n.y < 0.0 { n = -n; }
                best = Some(GroundHit { y, normal: n });
            }
        }
        best
    }

    /// First thing a straight line from `from` to `to` runs into (terrain or object, any side).
    /// Returns the distance along the line, if anything is in the way.
    pub fn raycast(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let d = to - from;
        let len = d.length();
        if len < 1e-4 { return None; }
        let dir = d / len;
        let steps = (len / (CELL * 0.5)).ceil() as i32;
        let mut seen: Vec<(i32, i32)> = Vec::new();
        let mut best: Option<f32> = None;
        for i in 0..=steps {
            let p = from + d * (i as f32 / steps as f32);
            for (ox, oz) in [(0.0, 0.0), (0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6)] {
                let key = (cell(p.x + ox), cell(p.z + oz));
                if seen.contains(&key) { continue; }
                seen.push(key);
                let Some(ids) = self.grid.get(&key) else { continue };
                for &id in ids {
                    let t = &self.tris[id as usize];
                    // Moller-Trumbore
                    let (e1, e2) = (t.p[1] - t.p[0], t.p[2] - t.p[0]);
                    let h = dir.cross(e2);
                    let a = e1.dot(h);
                    if a.abs() < 1e-7 { continue; }
                    let f = 1.0 / a;
                    let sv = from - t.p[0];
                    let u = f * sv.dot(h);
                    if !(0.0..=1.0).contains(&u) { continue; }
                    let q = sv.cross(e1);
                    let v = f * dir.dot(q);
                    if v < 0.0 || u + v > 1.0 { continue; }
                    let dist = f * e2.dot(q);
                    if dist > 0.02 && dist < len && best.is_none_or(|b| dist < b) { best = Some(dist); }
                }
            }
        }
        best
    }

    /// Push a sphere out of every wall it overlaps. Returns the corrected centre and the
    /// normals of the walls that were touched.
    pub fn push_out(&self, mut center: Vec3, radius: f32) -> (Vec3, Vec<Vec3>) {
        let mut contacts = Vec::new();
        for _ in 0..2 {
            let mut moved = false;
            for cx in cell(center.x - radius)..=cell(center.x + radius) {
                for cz in cell(center.z - radius)..=cell(center.z + radius) {
                    let Some(ids) = self.grid.get(&(cx, cz)) else { continue };
                    for &id in ids {
                        let t = &self.tris[id as usize];
                        if t.rideable || t.face.y < -0.6 { continue; }
                        let q = closest_point(center, t.p[0], t.p[1], t.p[2]);
                        let d = center - q;
                        let dist2 = d.length_squared();
                        if dist2 >= radius * radius || dist2 < 1e-10 { continue; }
                        let dist = dist2.sqrt();
                        let n = d / dist;
                        center += n * (radius - dist);
                        contacts.push(n);
                        moved = true;
                    }
                }
            }
            if !moved { break; }
        }
        (center, contacts)
    }
}

/// Closest point on triangle abc to p (Ericson, Real-Time Collision Detection).
fn closest_point(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 { return a; }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 { return b; }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { return a + ab * (d1 / (d1 - d3)); }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 { return c; }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { return a + ac * (d2 / (d2 - d6)); }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 { return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))); }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

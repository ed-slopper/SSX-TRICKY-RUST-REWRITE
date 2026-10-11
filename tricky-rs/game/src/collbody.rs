//! A rider's collision body (board row E8d2): eighteen spheres on the skeleton and one around them all, as
//! `CollBody_InitRider` 0x236050, `CollBody_UpdateFromBones` 0x2361b0, `CollBody_Translate` 0x236448 and
//! `CollBody_OverlapSpheres` 0x236a00 build, move and test them (cm, the game's z up), checked bit for bit in the
//! function runner (`tests/collbody.rs`). `Boarder_RiderCollisions` 0x123fc8 pushes two riders apart by 0.55 of the
//! overlap these give.

use crate::jump::{add, dot, normalize, scale, sub};

type V4 = [f32; 4];

/// The body (rider+0x470): the bounding sphere (+0x10 centre, +0x20 radius), +0x24, the sphere count (+0x28) and
/// the spheres (+0x30, 0x20 each: centre, radius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollBody {
    pub center: V4,
    pub radius: f32,
    pub x24: i32,
    pub count: i32,
    pub spheres: [(V4, f32); 18],
}

/// The spheres' radii (cm) as `CollBody_InitRider` sets them.
pub const RADII: [f32; 18] = [25.0, 20.0, 15.0, 15.0, 15.0, 15.0, 10.0, 10.0, 25.0, 25.0, 20.0, 20.0, 30.0, 30.0, 20.0, 20.0, 20.0, 20.0];

/// The skeleton bone each of the first fourteen spheres sits on (`CollBody_UpdateFromBones`: bone k's translation
/// at skeleton + 0x30 + 0x40·k).
pub const BONES: [usize; 14] = [2, 4, 6, 10, 7, 11, 8, 12, 13, 16, 14, 17, 15, 18];

impl CollBody {
    /// An empty body (all zero), to be set up by [`CollBody::init_rider`].
    pub fn new() -> CollBody {
        CollBody { center: [0.0; 4], radius: 0.0, x24: 0, count: 0, spheres: [([0.0; 4], 0.0); 18] }
    }

    /// `CollBody_InitRider` 0x236050: eighteen spheres at the origin with their radii, the bounding radius
    /// `bound` m (× 100); the bounding centre is left as it was.
    pub fn init_rider(&mut self, bound: f32) {
        self.radius = bound * 100.0;
        self.x24 = -1;
        self.count = 18;
        for (s, r) in self.spheres.iter_mut().zip(RADII) {
            *s = ([0.0; 4], r);
        }
    }

    /// `CollBody_UpdateFromBones` 0x2361b0: the spheres on the skeleton's bone positions (`bones[k]`, the
    /// translation of bone k), the board's four spheres 70 and 45 cm either side of the middle of spheres 12 and 13
    /// along the line between them, the bounding centre halfway between spheres 8 and 9.
    pub fn update_from_bones(&mut self, bones: &[V4]) {
        for (s, b) in self.spheres.iter_mut().zip(BONES) {
            s.0 = bones[b];
        }
        self.center = scale(add(self.spheres[8].0, self.spheres[9].0), 0.5);
        let (a, b) = (self.spheres[12].0, self.spheres[13].0);
        let dir = normalize(sub(a, b));
        let mid = scale(add(a, b), 0.5);
        for (k, d) in [70.0f32, -70.0, 45.0, -45.0].iter().enumerate() {
            self.spheres[14 + k].0 = add(mid, scale(dir, *d));
        }
    }

    /// `CollBody_Translate` 0x236448: the body moved by `v`.
    pub fn translate(&mut self, v: V4) {
        self.center = add(self.center, v);
        for s in self.spheres.iter_mut().take(self.count.max(0) as usize) {
            s.0 = add(s.0, v);
        }
    }
}

/// `CollBody_OverlapSpheres` 0x236a00: if the bounding spheres meet, the first pair of spheres (a's in order, then
/// b's) that overlap gives the push from a to b (unit direction × the overlap) and the point on a's sphere where
/// they touch; `None` if none do.
pub fn overlap(a: &CollBody, b: &CollBody) -> Option<(V4, V4)> {
    let d = sub(b.center, a.center);
    let r = b.radius + a.radius;
    if !(dot(d, d) < r * r) {
        return None;
    }
    for i in 0..a.count.max(0) as usize {
        for j in 0..b.count.max(0) as usize {
            let (ca, ra) = a.spheres[i];
            let (cb, rb) = b.spheres[j];
            let d = sub(cb, ca);
            let r = rb + ra;
            let dd = dot(d, d);
            if dd < r * r {
                let n = normalize(d);
                let pen = r - dd.sqrt();
                return Some((scale(n, pen), sub(ca, scale(n, ra - pen))));
            }
        }
    }
    None
}

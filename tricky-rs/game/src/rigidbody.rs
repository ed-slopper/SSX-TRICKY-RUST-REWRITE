//! The game's rigid bodies (board row E8c): `cRigidBody` as the knockable objects (`cRollerNode`) and the wipe-out
//! body use it: `RigidBody_SetPosition` 0x153808 (in fact: set the spin, the angular momentum from it),
//! `RigidBody_UpdateVelocity` 0x153ad8, `RigidBody_Integrate` 0x153c50, `RigidBody_ContactImpulse` 0x153ee8 and
//! `RigidBody_AddImpulse` 0x154338; and a collision body's `CollBody_Transform` 0x236498 and
//! `CollBody_PointMassInertia` 0x236530. In the PS2's floats (`ps2f::F`), in the game's order of operations (the
//! VU0 parts lane by lane, its dot products ((x + y) + z) + w), checked in the function runner (`tests/rigidbody.rs`).

use crate::collbody::CollBody;
use crate::ps2f::F;

type V4 = [f32; 4];

/// A rigid body (`cRigidBody`): +8 the inverse mass, +0xc the inverse inertia (3 × 3, column by column: ω = I⁻¹·L
/// takes ωx = (i0·Lx + i3·Ly) + i6·Lz), +0x30 the position, +0x40 the orientation (w, x, y, z), +0x50 the
/// momentum, +0x60 the angular momentum, +0x70 the velocity, +0x80 the spin.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RigidBody {
    pub inv_mass: f32,
    pub inv_inertia: [f32; 9],
    pub pos: V4,
    pub q: V4,
    pub p: V4,
    pub l: V4,
    pub v: V4,
    pub w: V4,
}

fn ld(a: V4) -> [F; 4] {
    a.map(F)
}
fn st(a: [F; 4]) -> V4 {
    a.map(|x| x.0)
}
fn add(a: [F; 4], b: [F; 4]) -> [F; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}
fn sub(a: [F; 4], b: [F; 4]) -> [F; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}
/// `vmulx.xyzw` (and `Vec4_Scale` 0x102f50): every lane times s
fn scale(a: [F; 4], s: F) -> [F; 4] {
    a.map(|x| x * s)
}
/// `vopmula.xyz` / `vopmsub.xyz` / `vsub.w`: a × b, w 0
fn cross(a: [F; 4], b: [F; 4]) -> [F; 4] {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], F(0.0)]
}
/// The VU0 dot product of all four lanes: ((x + y) + 1·z) + 1·w
fn dot(a: [F; 4], b: [F; 4]) -> F {
    let t = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((t[0] + t[1]) + F(1.0) * t[2]) + F(1.0) * t[3]
}
/// A 3 × 3 (column by column) times the vector's x, y, z; w as given
fn mat(m: &[f32; 9], a: [F; 4], w: F) -> [F; 4] {
    let m = m.map(F);
    [(m[0] * a[0] + m[3] * a[1]) + m[6] * a[2], (m[1] * a[0] + m[4] * a[1]) + m[7] * a[2], (m[2] * a[0] + m[5] * a[1]) + m[8] * a[2], w]
}

/// |ω| is held to this (rad/s) by `RigidBody_UpdateVelocity`
pub const MAX_SPIN: f32 = 23.561945;

impl RigidBody {
    /// The velocity tail of `RigidBody_AddImpulse` and `RigidBody_UpdateVelocity`: v = p / m, ω = I⁻¹·L (w 0).
    fn derive(&mut self) {
        self.v = st(scale(ld(self.p), F(self.inv_mass)));
        self.w = st(mat(&self.inv_inertia, ld(self.l), F(0.0)));
    }

    /// `RigidBody_SetPosition` 0x153808 (despite its name): the spin set to `w` (all four lanes) and the angular
    /// momentum from it, L = (I⁻¹)⁻¹·ω by cofactors (w 0).
    pub fn set_spin(&mut self, w: V4) {
        self.w = w;
        let m = self.inv_inertia.map(F);
        let c0 = m[4] * m[8] - m[5] * m[7];
        let c1 = m[7] * m[2] - m[8] * m[1];
        let c2 = m[1] * m[5] - m[2] * m[4];
        let det = (m[0] * c0 + m[3] * c1) + m[6] * c2;
        let inv = F(1.0) / det;
        let c3 = m[6] * m[5] - m[8] * m[3];
        let c4 = m[0] * m[8] - m[2] * m[6];
        let c5 = m[3] * m[2] - m[5] * m[0];
        let c6 = m[3] * m[7] - m[4] * m[6];
        let c7 = m[6] * m[1] - m[7] * m[0];
        let c8 = m[0] * m[4] - m[1] * m[3];
        let n = [c0, c1, c2, c3, c4, c5, c6, c7, c8].map(|c| (c * inv).0);
        self.l = st(mat(&n, ld(w), F(0.0)));
    }

    /// `RigidBody_UpdateVelocity` 0x153ad8: v and ω from the momenta, the spin held to [`MAX_SPIN`] (ω and L
    /// scaled down together).
    pub fn update_velocity(&mut self) {
        self.derive();
        let w = ld(self.w);
        let s = dot(w, w).sqrt();
        if F(MAX_SPIN) < s {
            let k = F(MAX_SPIN) / s;
            self.w = st(scale(w, k));
            self.l = st(scale(ld(self.l), k));
        }
    }

    /// `RigidBody_Integrate` 0x153c50(dt, force, torque): the position by the velocity, the orientation by the
    /// spin (q += ½ (0, ω) q dt, normalised; a zero quaternion becomes w 1), then the momenta by the force and
    /// torque. The velocities are left for `update_velocity`.
    pub fn integrate(&mut self, dt: f32, force: V4, torque: V4) {
        let dt = F(dt);
        self.pos = st(add(ld(self.pos), scale(ld(self.v), dt)));
        let [ox, oy, oz] = [F(self.w[0]), F(self.w[1]), F(self.w[2])];
        let [qw, qx, qy, qz] = ld(self.q);
        let h = F(0.5);
        let d = [
            ((-ox * qx - oy * qy) - oz * qz) * h,
            ((ox * qw + oy * qz) - oz * qy) * h,
            ((oy * qw + oz * qx) - ox * qz) * h,
            ((oz * qw + ox * qy) - oy * qx) * h,
        ];
        let q = [qw + d[0] * dt, qx + d[1] * dt, qy + d[2] * dt, qz + d[3] * dt];
        let len = (((q[1] * q[1] + q[2] * q[2]) + q[3] * q[3]) + q[0] * q[0]).sqrt();
        self.q = if len == F(0.0) {
            [1.0, q[1].0, q[2].0, q[3].0]
        } else {
            let inv = F(1.0) / len;
            st([q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv])
        };
        self.p = st(add(ld(self.p), scale(ld(force), dt)));
        self.l = st(add(ld(self.l), scale(ld(torque), dt)));
    }

    /// `RigidBody_AddImpulse` 0x154338(r, j): impulse `j` at `r` from the centre: p += j, L += r × j, then v and ω.
    pub fn add_impulse(&mut self, r: V4, j: V4) {
        let (r, j) = (ld(r), ld(j));
        self.p = st(add(ld(self.p), j));
        self.l = st(add(ld(self.l), cross(r, j)));
        self.derive();
    }

    /// The velocity of the body's point at `r` from the centre (v + ω × r), as the contact computes it.
    fn point_velocity(&self, r: [F; 4]) -> [F; 4] {
        add(ld(self.v), cross(ld(self.w), r))
    }

    /// `RigidBody_ContactImpulse` 0x153ee8(e, mu, point, n): the caller has found the point moving into the
    /// surface. With friction (mu not 0): an impulse against the sliding, −mu·|v_t| / (1/m + t·((I⁻¹(r × t)) × r))
    /// along the sliding direction t; then if the point no longer moves into the surface (v·n > 0) that is all.
    /// Then the bounce: −(1 + e)·v·n / (1/m + n·((I⁻¹(r × n)) × r)) along n. Dot products take all four lanes.
    pub fn contact_impulse(&mut self, e: f32, mu: f32, point: V4, n: V4) {
        let r = sub(ld(point), ld(self.pos));
        let n4 = ld(n);
        let mut vn = dot(self.point_velocity(r), n4);
        if mu != 0.0 {
            let vp = self.point_velocity(r);
            let vt = sub(vp, scale(n4, vn));
            let len = dot(vt, vt).sqrt();
            let q = F(1.0) / len;
            let t = scale(vt, q);
            let f = -F(mu) * len;
            let it = mat(&self.inv_inertia, cross(r, t), F(0.0));
            let c = cross(it, r);
            let j = f / (F(self.inv_mass) + dot(t, c));
            self.add_impulse(st(r), st(scale(t, j)));
            let r2 = sub(ld(point), ld(self.pos));
            vn = dot(self.point_velocity(r2), n4);
            if F(0.0) < vn {
                return;
            }
        }
        let rn = cross(r, n4);
        let ir = mat(&self.inv_inertia, rn, F(0.0));
        let c = cross(ir, r);
        let f = -(F(e) + F(1.0)) * vn;
        let j = f / (F(self.inv_mass) + dot(n4, c));
        self.add_impulse(st(r), st(scale(n4, j)));
    }
}

/// The per-sphere masses `CollBody_PointMassInertia` weighs a body's spheres by (0x3a3f68).
pub const SPHERE_MASS: [f32; 18] = [4.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 4.0, 4.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0];

/// `CollBody_Transform` 0x236498: the bounding centre and each sphere's centre through the matrix `m` (four
/// columns: ((c0·x + c1·y) + c2·z) + c3·w lane by lane).
pub fn transform(b: &mut CollBody, m: &[V4; 4]) {
    let c = m.map(ld);
    let tf = |p: V4| {
        let p = ld(p);
        let mut o = [F(0.0); 4];
        for k in 0..4 {
            o[k] = ((c[0][k] * p[0] + c[1][k] * p[1]) + c[2][k] * p[2]) + c[3][k] * p[3];
        }
        st(o)
    };
    b.center = tf(b.center);
    for s in b.spheres.iter_mut().take(b.count.max(0) as usize) {
        s.0 = tf(s.0);
    }
}

/// `CollBody_PointMassInertia` 0x236530: the spheres as point masses ([`SPHERE_MASS`]): the total mass, the centre
/// of mass (all four lanes) and the inertia about it (3 × 3, row by row).
pub fn point_mass_inertia(b: &CollBody) -> (f32, V4, [f32; 9]) {
    let n = b.count.max(0) as usize;
    let mut mass = F(0.0);
    let mut com = [F(0.0); 4];
    for k in 0..n {
        let m = F(SPHERE_MASS[k]);
        mass = mass + m;
        com = add(com, scale(ld(b.spheres[k].0), m));
    }
    let com = scale(com, F(1.0) / mass);
    let mut i = [F(0.0); 9];
    for k in 0..n {
        let d = sub(ld(b.spheres[k].0), com);
        let m = F(SPHERE_MASS[k]);
        let (x2, y2, z2) = (d[0] * d[0], d[1] * d[1], d[2] * d[2]);
        let (yz, xz, xy) = (m * (d[1] * d[2]), m * (d[0] * d[2]), m * (d[0] * d[1]));
        i[0] = i[0] + m * (y2 + z2);
        i[4] = i[4] + m * (z2 + x2);
        i[8] = i[8] + m * (x2 + y2);
        i[1] = i[1] - xy;
        i[2] = i[2] - xz;
        i[5] = i[5] - yz;
        i[3] = i[3] - xy;
        i[6] = i[6] - xz;
        i[7] = i[7] - yz;
    }
    (mass.0, st(com), i.map(|x| x.0))
}

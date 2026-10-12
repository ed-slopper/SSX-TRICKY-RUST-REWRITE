//! The wipe-out body (board row G6): the rider tumbling as one rigid body made of its collision spheres, as
//! `Ragdoll_ComputeMassProps` 0x10d6f8, `Ragdoll_InitFromBoarder` 0x10dad8, `Ragdoll_Step` 0x10dca8,
//! `Ragdoll_Contacts` 0x10e190, `Ragdoll_HitRoller` 0x10ed68 and `Ragdoll_GroundProbe` 0x10fd58 move it. In the
//! PS2's floats, in the game's order of operations; checked in the function runner (`tests/ragdoll.rs`).

use crate::collbody::CollBody;
use crate::probe::fall_line;
use crate::ps2f::F;
use crate::rigidbody::{add, cross, dot, ld, mat, point_mass_inertia, scale, st, sub, RigidBody};
use crate::roller::{RollerBody, RollerEnv};
use crate::walls::Contact;
use crate::worldboarder::WorldBoarder;

type V4 = [f32; 4];

/// Gravity on the tumbling body (cm/s², 1900.8359)
pub const GRAVITY: f32 = 1900.8359;
/// The pull toward the course while the body has touched something in the last 0.15 s (cm/s²)
pub const PULL: f32 = 600.0;

/// The rider as the wipe-out body reads and writes it: +0x140 the position, +0x150 the velocity, +0x170 the
/// orientation (w, x, y, z), +0x1a0 the board's up (its z axis), +0x1bc the height above the ground, +0x290 the
/// surface type (6 and 10 are the bad ones), +0x2a0 the ground's normal, +0x2b0 the fall line, +0x2d0 the ground's
/// point, +0x2e0 the kind of the last touch (3 none), +0x2f0 the crash normal, +0x320 the up along the ground, +0x350 the nearest course point, +0x470 the collision body; the hip bone's
/// place in the rider's frame (the skeleton's bone at +0x44d8: rider + 0x394 · index + 0x8c8), and the rider's id.
#[derive(Clone, Debug, PartialEq)]
pub struct RagRider {
    pub id: u32,
    pub pos: V4,
    pub vel: V4,
    pub q: V4,
    pub up: V4,
    pub height: f32,
    pub surface: u32,
    pub ground: V4,
    pub fall: V4,
    pub ground_at: V4,
    pub ground_kind: u32,
    pub crash_n: V4,
    pub side: V4,
    pub course: V4,
    pub body: CollBody,
    pub hip: V4,
}

/// The wipe-out motion's body: +8 seconds since the last touch, +0x20 the crash point, +0x40 the crash normal,
/// +0x70 the rigid body.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Ragdoll {
    pub t_air: f32,
    pub crash_at: V4,
    pub crash_n: V4,
    pub rb: RigidBody,
}

/// A cast's result record: +0 its kind, +0x10 the point, +0x20 the normal, +0x30..+0x3b (+0x34 the
/// surface type, +0x38 the object).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CastOut {
    pub kind: u32,
    pub pos: V4,
    pub normal: V4,
    pub x30: [u32; 3],
}

/// What the wipe-out body asks of the world and the rider.
pub trait RagEnv: RollerEnv {
    /// the global word `Ragdoll_Contacts` puts in the body's +0x24 while it casts (0x31d314)
    fn mask(&self) -> i32;
    /// `World_RayCast`(course, the shape, out, 0)
    fn cast_body(&mut self, q: &WorldBoarder) -> (f32, CastOut);
    /// `World_QueryInstances`(course, the shape, out)
    fn query_body(&mut self, q: &WorldBoarder) -> (f32, CastOut);
    /// `World_RayCast`(course, `cWorldLine`(0.5, from, to, 1), out, 0)
    fn cast_line(&mut self, from: V4, to: V4) -> (f32, CastOut);
    /// `Boarder_AccumulateImpact` 0x125c48(rider, n)
    fn accumulate_impact(&mut self, n: V4);
    /// the rider's virtual +0x64(point, velocity, normal)
    fn rider_contact(&mut self, point: V4, vel: V4, n: V4);
    /// an object's flags (+0xe8)
    fn object_flags(&self, object: u32) -> u32;
    /// `Instance_OnPlayerContact` 0x13bd40(object, rider, contact, out)
    fn player_contact(&mut self, object: u32, c: &Contact, out: &CastOut);
    /// `Boarder_UpdateOrientation` 0x129008(rider)
    fn update_orientation(&mut self, r: &mut RagRider);
    /// `Collide_BoarderSphTree`(the roller's tree, the rider's body): the push and the point
    fn roller_touch(&mut self, roller: &RollerBody, body: &CollBody) -> Option<(V4, V4)> {
        roller.tree.body(body)
    }
}

/// v times 1/√(v·v) (`vrsqrt`)
fn unit(v: [F; 4]) -> [F; 4] {
    scale(v, F(1.0) / dot(v, v).sqrt())
}

/// The quaternion (w, x, y, z) turning c (`sign` 1) or turning it back (−1): (c ± 2w(q × c)) + 2(q × (q × c)), w 0.
fn turn(q: V4, c: [F; 4], back: bool) -> [F; 4] {
    let [w, x, y, z] = ld(q);
    let t = [y * c[2] - z * c[1], z * c[0] - x * c[2], x * c[1] - y * c[0]];
    let u = [y * t[2] - z * t[1], z * t[0] - x * t[2], x * t[1] - y * t[0]];
    let s = w + w;
    let two = F(2.0);
    let a = if back { [c[0] - t[0] * s, c[1] - t[1] * s, c[2] - t[2] * s] } else { [c[0] + t[0] * s, c[1] + t[1] * s, c[2] + t[2] * s] };
    [a[0] + u[0] * two, a[1] + u[1] * two, a[2] + u[2] * two, F(0.0)]
}

impl Ragdoll {
    /// `Ragdoll_ComputeMassProps` 0x10d6f8: the rider's spheres as point masses; the inverse mass, the inverse
    /// inertia 0.8 · (1.2 · I)⁻¹ (by cofactors), v and ω from the momenta. Returns the centre of mass.
    pub fn mass_props(&mut self, r: &RagRider) -> V4 {
        let (mass, com, inertia) = point_mass_inertia(&r.body);
        let m = inertia.map(|x| F(x) * F(1.2));
        let c0 = m[4] * m[8] - m[5] * m[7];
        let c1 = m[7] * m[2] - m[8] * m[1];
        let c2 = m[1] * m[5] - m[2] * m[4];
        let inv = F(1.0) / ((m[0] * c0 + m[3] * c1) + m[6] * c2);
        let n = [
            c0 * inv,
            c1 * inv,
            c2 * inv,
            (m[6] * m[5] - m[8] * m[3]) * inv,
            (m[0] * m[8] - m[2] * m[6]) * inv,
            (m[3] * m[2] - m[5] * m[0]) * inv,
            (m[3] * m[7] - m[4] * m[6]) * inv,
            (m[6] * m[1] - m[7] * m[0]) * inv,
            (m[0] * m[4] - m[1] * m[3]) * inv,
        ];
        self.rb.inv_mass = (F(1.0) / F(mass)).0;
        self.rb.inv_inertia = n.map(|x| (x * F(0.8)).0);
        self.rb.update_velocity();
        com
    }

    /// `Ragdoll_InitFromBoarder` 0x10dad8: the body at the centre of mass with the rider's orientation and
    /// velocity and no spin; the crash normal turned round (and kept at rider+0x2f0); if the crash point closes on
    /// it, one bounce (e 0.15, no friction) there.
    pub fn init(&mut self, r: &mut RagRider) {
        let com = self.mass_props(r);
        self.rb.pos = com;
        self.rb.q = r.q;
        self.rb.p = st(scale(ld(r.vel), F(1.0) / F(self.rb.inv_mass)));
        self.rb.v = r.vel;
        self.rb.set_spin([0.0; 4]);
        self.crash_n = st(scale(ld(self.crash_n), F(-1.0)));
        r.crash_n = self.crash_n;
        let rr = sub(ld(self.crash_at), ld(self.rb.pos));
        let vp = add(ld(self.rb.v), cross(ld(self.rb.w), rr));
        if dot(vp, ld(self.crash_n)) < F(0.0) {
            self.rb.contact_impulse(f32::from_bits(0x3e19_999a), 0.0, self.crash_at, self.crash_n);
        }
    }

    /// `Ragdoll_Step` 0x10dca8(dt): gravity and half the momenta as drag, while the body has touched something in
    /// the last 0.15 s a pull of 600 cm/s² toward the course along the ground; the step, v and ω, the contacts;
    /// then the rider put where the body is (its offset from the centre of mass kept in its own frame), the
    /// rider's velocity the body's, its orientation brought up to date.
    pub fn step<E: RagEnv>(&mut self, dt: f32, r: &mut RagRider, rec: &mut CastOut, rollers: &mut [(u32, RollerBody)], env: &mut E) {
        let com = self.mass_props(r);
        let im = F(self.rb.inv_mass);
        let grav = [F(0.0), F(0.0), F(-GRAVITY) / im, F(0.0)];
        let mut force = sub(grav, scale(ld(self.rb.p), F(0.5)));
        let torque = sub([F(0.0); 4], scale(ld(self.rb.l), F(0.5)));
        if self.t_air < 0.15 {
            let d = sub(ld(r.course), ld(com));
            let g = ld(r.ground);
            let d = unit(sub(d, scale(g, dot(d, g))));
            force = add(force, scale(d, F(PULL) / im));
        }
        self.rb.integrate(dt, st(force), st(torque));
        self.rb.update_velocity();
        self.contacts(r, rec, rollers, env);
        let c = [F(com[0]), F(com[1]), F(com[2]), F(1.0)];
        let u = sub(c, ld(r.pos));
        let local = turn(r.q, u, true);
        r.q = self.rb.q;
        let v = turn(r.q, local, false);
        r.pos = st(sub(ld(self.rb.pos), v));
        r.vel = self.rb.v;
        env.update_orientation(r);
    }

    /// The body moved by `d` with the rider's collision body (`Body_Translate`, `CollBody_Translate`).
    fn translate(&mut self, d: [F; 4], r: &mut RagRider) {
        self.rb.pos = st(add(ld(self.rb.pos), d));
        r.body.translate(st(d));
    }

    /// The point's velocity v + ω × (p − x).
    fn point_velocity(&self, p: V4) -> [F; 4] {
        add(ld(self.rb.v), cross(ld(self.rb.w), sub(ld(p), ld(self.rb.pos))))
    }

    /// A ground bounce: e = 0.8 − 0.7|n_z| and μ = 0.15|n_z| from 100 cm/s up (else none), then the momenta × 0.95
    /// (v = p/m, ω = I⁻¹L, w 0).
    fn bounce(&mut self, out: &CastOut, n: V4) {
        let v = ld(self.rb.v);
        let speed = dot(v, v).sqrt();
        let (mut e, mut mu) = (F(0.0), F(0.0));
        if F(100.0) <= speed {
            let a = F(n[2].abs());
            mu = a * F(0.15);
            e = F(0.8) - a * F(0.7);
        }
        self.rb.contact_impulse(e.0, mu.0, out.pos, n);
        let k = F(f32::from_bits(0x3f73_3333));
        let p = scale(ld(self.rb.p), k);
        self.rb.p = st(p);
        self.rb.v = st(scale(p, F(self.rb.inv_mass)));
        let l = scale(ld(self.rb.l), k);
        self.rb.l = st(l);
        self.rb.w = st(mat(&self.rb.inv_inertia, l, F(0.0)));
    }

    /// `Ragdoll_Contacts` 0x10e190: the rider's body (shape built once, with the global word in its +0x24) against
    /// the world: pushed out, and if the point closes, `Boarder_AccumulateImpact`, the rider's +0x64, a bounce and
    /// the record kept; again (a second normal facing the first is made to slide along it), the same without the
    /// rider's +0x64; then the objects (the normal along the ground; pushed out only by objects with flag 0x80):
    /// a closing point is a contact (`Instance_OnPlayerContact`), and with flag 0x80 an impact and a bounce
    /// (e 0.5) as well, and the sound; then each roller among the rider's broadphase pairs.
    pub fn contacts<E: RagEnv>(&mut self, r: &mut RagRider, rec: &mut CastOut, rollers: &mut [(u32, RollerBody)], env: &mut E) {
        rec.kind = 3;
        r.body.x24 = env.mask();
        let shape = WorldBoarder::new(&r.body, 0);
        let (t, out) = env.cast_body(&shape);
        if 0.0 < t {
            let n = out.normal;
            self.translate(scale(ld(n), F(t)), r);
            if dot(self.point_velocity(out.pos), ld(n)) < F(0.0) {
                env.accumulate_impact(n);
                env.rider_contact(out.pos, self.rb.v, n);
                self.bounce(&out, n);
                *rec = out;
            }
            let (t2, mut out2) = env.cast_body(&shape);
            if 0.0 < t2 {
                let d = dot(ld(out2.normal), ld(n));
                if d < F(0.0) {
                    out2.normal = st(unit(sub(ld(out2.normal), scale(ld(n), d))));
                }
                let n2 = out2.normal;
                self.translate(scale(ld(n2), F(t2)), r);
                if dot(self.point_velocity(out2.pos), ld(n2)) < F(0.0) {
                    env.accumulate_impact(n2);
                    self.bounce(&out2, n2);
                    *rec = out2;
                }
            }
        }
        let (t, mut out) = env.query_body(&shape);
        if 0.0 < t {
            let g = ld(r.ground);
            let n4 = ld(out.normal);
            let n = st(unit(sub(n4, scale(g, dot(n4, g)))));
            out.normal = n;
            let object = out.x30[2];
            if env.object_flags(object) & 0x80 != 0 {
                self.translate(scale(ld(n), F(t)), r);
            }
            if dot(self.point_velocity(out.pos), ld(n)) < F(0.0) {
                let v = ld(self.rb.v);
                let speed = dot(v, v).sqrt();
                let c = Contact { pos: out.pos, dir: st(unit(v)), normal: n, speed_in: speed.0 };
                env.player_contact(object, &c, &out);
                if env.object_flags(object) & 0x80 != 0 {
                    env.accumulate_impact(n);
                    self.rb.contact_impulse(0.5, 0.0, out.pos, n);
                }
                env.sfx(r.id, object);
            }
        }
        for (object, roller) in rollers.iter_mut() {
            let touch = env.roller_touch(roller, &r.body);
            self.hit_roller_touch(roller, *object, touch, env);
        }
        r.body.x24 = -1;
    }

    /// `Ragdoll_HitRoller` 0x10ed68: a roller's tree against the rider's body; if the points close along the push
    /// n, j = −1.5·closing / (1/m + 1/m_roller + n·((I⁻¹(r × n)) × r) + n·((I_r⁻¹(r_r × n)) × r_r)) to the body and
    /// against the roller, which is woken.
    pub fn hit_roller(&mut self, r: &RagRider, roller: &mut RollerBody, object: u32, env: &mut impl RollerEnv) {
        let touch = roller.tree.body(&r.body);
        self.hit_roller_touch(roller, object, touch, env);
    }

    /// [`Ragdoll::hit_roller`] after `Collide_BoarderSphTree`'s answer (the push and the point).
    pub fn hit_roller_touch(&mut self, roller: &mut RollerBody, object: u32, touch: Option<(V4, V4)>, env: &mut impl RollerEnv) {
        let Some((pen, contact)) = touch else { return };
        let n = unit(ld(pen));
        let c = ld(contact);
        let va = add(ld(self.rb.v), cross(ld(self.rb.w), sub(c, ld(self.rb.pos))));
        let vb = add(ld(roller.rb.v), cross(ld(roller.rb.w), sub(c, ld(roller.rb.pos))));
        let rel = dot(sub(va, vb), n);
        if !(rel < F(0.0)) {
            return;
        }
        let ra = sub(c, ld(self.rb.pos));
        let rb = sub(c, ld(roller.rb.pos));
        let ka = dot(n, cross(mat(&self.rb.inv_inertia, cross(ra, n), F(0.0)), ra));
        let kb = dot(n, cross(mat(&roller.rb.inv_inertia, cross(rb, n), F(0.0)), rb));
        let j = (rel * F(-1.5)) / (((F(self.rb.inv_mass) + F(roller.rb.inv_mass)) + ka) + kb);
        let jn = scale(n, j);
        self.rb.add_impulse(st(ra), st(jn));
        roller.rb.add_impulse(st(rb), st(scale(jn, F(-1.0))));
        roller.on_hit(object, None, env);
    }
}

/// `Ragdoll_GroundProbe` 0x10fd58: a line 400 cm either side along the ground's normal through the hip (turned
/// by the rider's orientation, taken onto the plane through the rider along the normal). No ground: the normal
/// straight up, the fall line the board's up, kind 3. Ground: its kind, its surface type, normal, point, the height
/// above it and the fall line (`Boarder_GetFallLine`). Either way +0x320 the board's up along the (new) ground.
pub fn ground_probe(r: &mut RagRider, env: &mut impl RagEnv) -> bool {
    let down = scale(ld(r.ground), F(400.0));
    let h = [F(r.hip[0]), F(r.hip[1]), F(r.hip[2]), F(0.0)];
    let rot = turn(r.q, h, false);
    let p = add(ld(r.pos), rot);
    let p = sub(p, scale(ld(r.ground), dot(rot, ld(r.ground))));
    let from = sub(p, down);
    let to = add(p, down);
    let (t, out) = env.cast_line(st(from), st(to));
    let side = |r: &RagRider| {
        let f = ld(r.up);
        let g = ld(r.ground);
        st(unit(sub(f, scale(g, dot(f, g)))))
    };
    if t < 0.0 {
        r.ground = [0.0, 0.0, 1.0, 0.0];
        r.side = side(r);
        r.fall = r.up;
        r.ground_kind = 3;
    } else {
        r.ground_kind = out.kind;
        r.surface = out.x30[1];
        r.ground = out.normal;
        r.side = side(r);
        r.height = dot(sub(ld(r.pos), ld(out.pos)), ld(r.ground)).0;
        r.fall = fall_line(r.ground, r.up);
        r.ground_at = out.pos;
    }
    0.0 <= t
}

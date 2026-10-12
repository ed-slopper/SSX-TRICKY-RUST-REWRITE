//! The knockable objects' bodies (board row E8c2): `cRollerNode` above its controller layer
//! (`controllers::Roller`): the rigid body (`rigidbody`), its world inverse inertia (`RollerNode_UpdateTransform`
//! 0x13e140), its sphere tree placed from the body (`FUN_0013e6a0` 0x13e6a0, `Mat4_FromPosQuat` 0x250428), the world
//! pushing back (`RollerNode_Collide` 0x13eda8, `RollerNode_Translate` 0x13ed30), two rollers knocking
//! (`Roller_CheckCollision` 0x13f5c8), waking (`Roller_OnHit` 0x13f448), the rider's kick (the rollers' loop at the
//! end of `Boarder_InstanceCollisions` 0x125088), the step of `cRollerNode::Update` 0x13e8a8 after its forces and
//! the constructor's set-up and first push (`cRollerNode::cRollerNode` 0x13d6c0, `FUN_0013dd40`, `FUN_0013dbe8`). In
//! the PS2's floats, in the game's order of operations; checked in the function runner (`tests/roller.rs`).

use crate::collbody::CollBody;
use crate::controllers::{CtrlEnv, Roller};
use crate::ps2f::F;
use crate::rigidbody::{add, cross, dot, ld, mat, scale, st, sub, RigidBody};
use crate::sphtree::{SphereTree, WorldSphTree};
use crate::walls::{impact_check, Contact, WallEnv, WallRider};

type V4 = [f32; 4];

/// The integration step `cRollerNode::Update` hands `RigidBody_Integrate` (0x3c888889, 1/60)
pub const DT: f32 = f32::from_bits(0x3c88_8889);
/// What a frame adds to the knock (+0xe8) when the body hits something (0.016666668)
pub const TICK: f32 = f32::from_bits(0x3c88_8889);
/// The damping of a push out of the world (0x3f777777)
pub const DAMP: f32 = f32::from_bits(0x3f77_7777);
/// The speed under which a roller does not bounce off the ground (+0xe0 as the constructor sets it, 0x438ae38e)
pub const BOUNCE_SPEED: f32 = f32::from_bits(0x438a_e38e);

/// A knockable object's body (`cRollerNode` from +0x30): +0x30 the rigid body, +0xc8 the energy average, +0xcc the
/// mass, +0xd0 the physics record's next value, +0xd4, +0xd8 the bounce, +0xdc the friction, +0xe0 the least speed
/// that bounces off the ground, +0xe4 seconds awake, +0xe8 the knock, +0xf0, +0x100 the body inverse inertia (the
/// model's, × 0.075), +0x130 the centre of mass in the object's frame, +0x140 asleep, +0x150 the sphere tree,
/// +0x2a0 / +0x2b0 its box (the broadphase's).
#[derive(Clone, Debug, PartialEq)]
pub struct RollerBody {
    pub rb: RigidBody,
    pub energy: f32,
    pub mass: f32,
    pub xd0: f32,
    pub xd4: f32,
    pub e: f32,
    pub mu: f32,
    pub bounce_speed: f32,
    pub awake: f32,
    pub knock: f32,
    pub xf0: V4,
    pub body_inertia: [f32; 9],
    pub com: V4,
    pub asleep: u32,
    pub tree: SphereTree,
    pub lo: V4,
    pub hi: V4,
}

/// What a roller asks of the world.
pub trait RollerEnv {
    /// `World_RayCast`(course, `cWorldSphTree`(tree, 1), out, 0): the depth (> 0 a hit), the point and the normal
    fn ray_cast(&mut self, q: &WorldSphTree) -> (f32, V4, V4);
    /// `World_QueryInstances`(course, `cWorldSphTree`(tree, 1), out): the same against the objects
    fn query_instances(&mut self, q: &WorldSphTree) -> (f32, V4, V4);
    /// `Broadphase_MoveBox` 0x14fe68 (race +0x30's broadphase, the box +0x2c0 over +0x2a0 / +0x2b0)
    fn move_box(&mut self, lo: V4, hi: V4);
    /// `Instance_GetEventScript` 0x25fe78(course, object, event): −1 for none
    fn event_script(&self, object: u32, event: u32) -> i32;
    /// a `cScriptNode`(3, script, object) made (0xf0 bytes from the pool)
    fn start_script(&mut self, script: i32, object: u32);
    /// `Sfx_InstanceCollision` 0x216c50(rider, object)
    fn sfx(&mut self, rider: u32, object: u32);
}

/// 3 × 3 (column by column) times 3 × 3, each element (x_i·y_3c + x_3+i·y_3c+1) + x_6+i·y_3c+2
fn mul3(x: &[F; 9], y: &[F; 9]) -> [F; 9] {
    let mut o = [F(0.0); 9];
    for c in 0..3 {
        for i in 0..3 {
            o[3 * c + i] = (x[i] * y[3 * c] + x[3 + i] * y[3 * c + 1]) + x[6 + i] * y[3 * c + 2];
        }
    }
    o
}

/// The rotation of a quaternion (w, x, y, z) as `RollerNode_UpdateTransform` builds it, column by column.
fn rotation(q: V4) -> [F; 9] {
    let [w, x, y, z] = ld(q);
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let xx = x * x2;
    let yy = y * y2;
    let wz = w * z2;
    let xz = x * z2;
    let wx = w * x2;
    let wy = w * y2;
    let yz = y * z2;
    let one_xx = F(1.0) - xx;
    let xy = x * y2;
    let zz = z * z2;
    let one_yy = F(1.0) - yy;
    [one_yy - zz, xy + wz, xz - wy, xy - wz, one_xx - zz, yz + wx, xz + wy, yz - wx, one_xx - yy]
}

/// `Mat4_FromPosQuat` 0x250428: the matrix (four columns) of a position and a quaternion (w, x, y, z).
pub fn mat4_from_pos_quat(p: V4, q: V4) -> [V4; 4] {
    let [w, x, y, z] = ld(q);
    let (y2, z2) = (y + y, z + z);
    let one = F(1.0);
    let m4 = x * y2 - w * z2;
    let wx2 = w * (x + x);
    let m0 = (one - y * y2) - z * z2;
    let one_xx = one - x * (x + x);
    let m8 = x * z2 + w * y2;
    let m1 = x * y2 + w * z2;
    let m5 = one_xx - z * z2;
    let m9 = y * z2 - wx2;
    let m2 = x * z2 - w * y2;
    let m6 = y * z2 + wx2;
    let m10 = one_xx - y * y2;
    [[m0.0, m1.0, m2.0, 0.0], [m4.0, m5.0, m6.0, 0.0], [m8.0, m9.0, m10.0, 0.0], [p[0], p[1], p[2], 1.0]]
}

/// A quaternion (w, x, y, z) turning `c`: (c + 2w(q × c)) + 2(q × (q × c)), w 0 (`FUN_0013e6a0`, the constructor)
fn turn(q: V4, c: V4) -> [F; 4] {
    let [w, x, y, z] = ld(q);
    let c = ld(c);
    let t = [y * c[2] - z * c[1], z * c[0] - x * c[2], x * c[1] - y * c[0]];
    let u = [y * t[2] - z * t[1], z * t[0] - x * t[2], x * t[1] - y * t[0]];
    let s = w + w;
    let a = [c[0] + t[0] * s, c[1] + t[1] * s, c[2] + t[2] * s];
    let two = F(2.0);
    [a[0] + u[0] * two, a[1] + u[1] * two, a[2] + u[2] * two, F(0.0)]
}

/// The point of the contact's velocity: v + ω × (p − x)
fn point_velocity(b: &RigidBody, r: [F; 4]) -> [F; 4] {
    add(ld(b.v), cross(ld(b.w), r))
}

/// The energy the controller layer averages: v·p + ω·L (VU0 order)
fn energy(b: &RigidBody) -> f32 {
    (dot(ld(b.v), ld(b.p)) + dot(ld(b.w), ld(b.l))).0
}

/// A vector times 1/√(v·v) (`vrsqrt`)
fn unit(v: [F; 4]) -> [F; 4] {
    scale(v, F(1.0) / dot(v, v).sqrt())
}

impl RollerBody {
    /// `RollerNode_UpdateTransform` 0x13e140: the world inverse inertia R·B·Rᵀ from the orientation and the body's
    /// inverse inertia (+0x100).
    pub fn update_transform(&mut self) {
        let r = rotation(self.rb.q);
        let mut t = [F(0.0); 9];
        for c in 0..3 {
            for i in 0..3 {
                t[3 * c + i] = r[3 * i + c];
            }
        }
        let b = self.body_inertia.map(F);
        self.rb.inv_inertia = mul3(&mul3(&r, &b), &t).map(|x| x.0);
    }

    /// `FUN_0013e6a0` 0x13e6a0: where the object is (the body's position less its centre of mass turned by the
    /// orientation) and the orientation.
    pub fn frame(&self) -> (V4, V4) {
        let neg = scale(turn(self.rb.q, self.com), F(-1.0));
        (st(add(ld(self.rb.pos), neg)), self.rb.q)
    }

    /// The tree put where the object is (`SphTree_SetTransform` of `Mat4_FromPosQuat` of [`RollerBody::frame`])
    /// and the box around it, the tree's centre ± its radius (w as the centre's).
    pub fn place_tree(&mut self) {
        let (p, q) = self.frame();
        self.tree.set_transform(mat4_from_pos_quat(p, q));
        self.set_box();
    }

    /// +0x2a0 / +0x2b0: the tree's centre ∓ its radius (`SphTree_GetRadius`), w less / plus 0
    fn set_box(&mut self) {
        let r = F(self.tree.levels.first().map_or(0.0, |l| l.0));
        let d = [r, r, r, F(0.0)];
        self.lo = st(sub(ld(self.tree.centre), d));
        self.hi = st(add(ld(self.tree.centre), d));
    }

    /// `RollerNode_Translate` 0x13ed30: the body, the tree and its box moved by `d`, the broadphase told.
    pub fn translate(&mut self, d: V4, env: &mut impl RollerEnv) {
        let d4 = ld(d);
        self.rb.pos = st(add(ld(self.rb.pos), d4));
        self.tree.centre = st(add(ld(self.tree.centre), d4));
        self.lo = st(add(ld(self.lo), d4));
        self.hi = st(add(ld(self.hi), d4));
        env.move_box(self.lo, self.hi);
    }

    /// After a push out: L and v × 0.96666664, ω = I⁻¹·L (w 0), p = v·(1/(1/m)).
    fn damp(&mut self) {
        let l = scale(ld(self.rb.l), F(DAMP));
        self.rb.l = st(l);
        self.rb.w = st(mat(&self.rb.inv_inertia, l, F(0.0)));
        let v = scale(ld(self.rb.v), F(DAMP));
        let m = F(1.0) / F(self.rb.inv_mass);
        self.rb.p = st(scale(v, m));
        self.rb.v = st(v);
    }

    /// `RollerNode_Collide` 0x13eda8: the tree against the world (`World_RayCast`), then against the objects
    /// (`World_QueryInstances`). A hit pushes the body out by its depth (at least 2), damps it, and if the point
    /// is moving into the surface bounces it (`RigidBody_ContactImpulse`; off the ground with no bounce under
    /// +0xe0's speed) and adds a frame to the knock; a ground hit after five seconds awake sends it to sleep (10 s).
    pub fn collide(&mut self, env: &mut impl RollerEnv) {
        let q = WorldSphTree::new(&self.tree);
        let (t, point, n) = env.ray_cast(&q);
        if 0.0 < t {
            self.push(t, point, n, true, env);
        }
        let q = WorldSphTree::new(&self.tree);
        let (t, point, n) = env.query_instances(&q);
        if 0.0 < t {
            self.push(t, point, n, false, env);
        }
    }

    fn push(&mut self, t: f32, point: V4, n: V4, ground: bool, env: &mut impl RollerEnv) {
        let t = if t < 2.0 { 2.0 } else { t };
        self.translate(st(scale(ld(n), F(t))), env);
        self.damp();
        let r = sub(ld(point), ld(self.rb.pos));
        let vp = point_velocity(&self.rb, r);
        if dot(vp, ld(n)) < F(0.0) {
            let mut e = self.e;
            if ground {
                let v = ld(self.rb.v);
                if dot(v, v).sqrt() < F(self.bounce_speed) {
                    e = 0.0;
                }
            }
            self.rb.contact_impulse(e, self.mu, point, n);
            self.knock = (F(self.knock) + F(TICK)).0;
            if ground && 5.0 < self.awake {
                self.awake = 10.0;
            }
        }
    }

    /// `cRollerNode::Update` 0x13e8a8 from the integration on: the body stepped 1/60 s with the controller's
    /// force and torque, the inertia turned, v and ω, the tree and its box placed, the broadphase told, the world
    /// pushing back.
    pub fn step(&mut self, force: V4, torque: V4, env: &mut impl RollerEnv) {
        self.rb.integrate(DT, force, torque);
        self.update_transform();
        self.rb.update_velocity();
        self.place_tree();
        env.move_box(self.lo, self.hi);
        self.collide(env);
    }

    /// The controller layer as `controllers::Roller` holds it.
    pub fn controller(&self, object: u32) -> Roller {
        Roller {
            object,
            inv_mass: self.rb.inv_mass,
            inv_inertia: self.rb.inv_inertia,
            momentum: self.rb.p,
            ang_momentum: self.rb.l,
            vel: self.rb.v,
            ang_vel: self.rb.w,
            energy: self.energy,
            awake: self.awake,
            knock: self.knock,
            asleep: self.asleep,
        }
    }

    /// `cRollerNode::Update` 0x13e8a8 whole: the controller layer (sleeping, the forces), then [`RollerBody::step`].
    pub fn update<E: CtrlEnv + RollerEnv>(&mut self, object: u32, env: &mut E) {
        let mut c = self.controller(object);
        let forces = c.update(env);
        self.rb.inv_inertia = c.inv_inertia;
        self.rb.p = c.momentum;
        self.rb.l = c.ang_momentum;
        self.rb.v = c.vel;
        self.rb.w = c.ang_vel;
        self.energy = c.energy;
        self.awake = c.awake;
        self.knock = c.knock;
        self.asleep = c.asleep;
        if let Some((f, t)) = forces {
            self.step(f, t, env);
        }
    }

    /// `Roller_OnHit` 0x13f448(rider or none): an asleep roller wakes: awake and knock 0, the energy from its
    /// motion; its event-1 script starts, or without one it is set awake (+0x140 = 0); a rider's hit makes the
    /// sound. An awake roller is left alone.
    pub fn on_hit(&mut self, object: u32, rider: Option<u32>, env: &mut impl RollerEnv) {
        if self.asleep == 0 {
            return;
        }
        self.awake = 0.0;
        self.knock = 0.0;
        self.energy = energy(&self.rb);
        let s = env.event_script(object, 1);
        if s == -1 {
            self.asleep = 0;
        } else {
            env.start_script(s, object);
        }
        if let Some(r) = rider {
            env.sfx(r, object);
        }
    }

    /// `FUN_0013dd40`: the constructor's push from a rider's contact (+0x10 the direction, +0x20 the spin axis,
    /// +0x30 the speed): v = dir·speed·½ + (0, 0, min(5000/m + 100, 600), 0), p = v·m, the spin 10 × the axis.
    pub fn kick_contact(&mut self, dir: V4, axis: V4, speed: f32) {
        self.knock = 0.0;
        self.xf0 = [0.0; 4];
        let mut up = F(5000.0) / F(self.mass) + F(100.0);
        if F(600.0) < up {
            up = F(600.0);
        }
        let t = scale(scale(ld(dir), F(speed)), F(1.0) / F(2.0));
        let v = add(t, [F(0.0), F(0.0), up, F(0.0)]);
        self.rb.p = st(scale(v, F(1.0) / F(self.rb.inv_mass)));
        self.rb.v = st(v);
        self.rb.set_spin(st(scale(ld(axis), F(10.0))));
        self.energy = energy(&self.rb);
    }

    /// `FUN_0013dbe8`: the constructor's push with a velocity (the physics record's +0x18): v as given, p = v·m,
    /// the spin 10 × the unit vector against it.
    pub fn kick_velocity(&mut self, vel: V4) {
        self.knock = 0.0;
        self.xf0 = [0.0; 4];
        let v = ld(vel);
        self.rb.p = st(scale(v, F(1.0) / F(self.rb.inv_mass)));
        self.rb.v = vel;
        let d = unit(scale(v, F(-1.0)));
        self.rb.set_spin(st(scale(d, F(10.0))));
        self.energy = energy(&self.rb);
    }

    /// `cRollerNode::cRollerNode` 0x13d6c0's set-up of the body from the object (its place `pos` and orientation
    /// `q`), the mass, the model's centre of mass and inverse inertia (× 0.075): the body at the object's place
    /// plus the turned centre of mass, its inertia and velocities, the bounce 0, the friction 0.5, the box from
    /// the tree as it is. Then one of the pushes ([`RollerBody::kick_contact`], [`RollerBody::kick_velocity`]).
    pub fn init(&mut self, mass: f32, xd0: f32, com: V4, inertia: [f32; 9], pos: V4, q: V4) {
        self.awake = 0.0;
        self.asleep = 0;
        self.mass = mass;
        self.xd0 = xd0;
        self.rb.inv_mass = (F(1.0) / F(mass)).0;
        self.com = [com[0], com[1], com[2], 0.0];
        self.body_inertia = inertia.map(|x| (F(x) * F(0.075)).0);
        self.rb.pos = st(add(ld(pos), turn(q, self.com)));
        self.rb.q = q;
        self.update_transform();
        self.rb.update_velocity();
        self.e = 0.0;
        self.xd4 = 0.0;
        self.mu = 0.5;
        self.bounce_speed = BOUNCE_SPEED;
        self.set_box();
    }
}

/// `Roller_CheckCollision` 0x13f5c8(a, b): the trees' touch (`SphTree_IntersectTree`: the push and the point);
/// if the points close on each other along the push, an impulse −(1 + (e_a + e_b)/2)·closing /
/// (1/m_a + 1/m_b + n·((I_a⁻¹(r_a × n)) × r_a) + n·((I_b⁻¹(r_b × n)) × r_b)) along it, to a and against b, and b
/// is woken. A touch with nothing written (the game reads whatever its stack held) is skipped.
pub fn pair(a: &mut RollerBody, b: &mut RollerBody, b_object: u32, env: &mut impl RollerEnv) {
    let Some(Some((pen, contact))) = a.tree.tree(&b.tree) else { return };
    let n = unit(ld(pen));
    let c = ld(contact);
    let va = point_velocity(&a.rb, sub(c, ld(a.rb.pos)));
    let vb = point_velocity(&b.rb, sub(c, ld(b.rb.pos)));
    let rel = dot(sub(va, vb), n);
    if !(rel < F(0.0)) {
        return;
    }
    let ra = sub(c, ld(a.rb.pos));
    let rb = sub(c, ld(b.rb.pos));
    let ia = mat(&a.rb.inv_inertia, cross(ra, n), F(0.0));
    let ka = dot(n, cross(ia, ra));
    let ib = mat(&b.rb.inv_inertia, cross(rb, n), F(0.0));
    let kb = dot(n, cross(ib, rb));
    let e = -((F(a.e) + F(b.e)) * F(0.5) + F(1.0));
    let j = (e * rel) / (((F(a.rb.inv_mass) + F(b.rb.inv_mass)) + ka) + kb);
    let jn = scale(n, j);
    a.rb.add_impulse(st(ra), st(jn));
    b.rb.add_impulse(st(rb), st(scale(jn, F(-1.0))));
    b.on_hit(b_object, None, env);
}

/// A rider against a roller (`Boarder_InstanceCollisions` 0x125088's rollers' loop, a node of type 0): the
/// roller's tree against the rider's body (`Collide_BoarderSphTree`); on a touch, with n the push made unit, the
/// rider's speed into it over 0 goes to `Boarder_WallImpactCheck` (the record: the point, the rider's direction, n,
/// n · (the rider's velocity − the roller's point's)); if that closing speed is below 0 the roller takes an
/// impulse 1.3·closing / (n·((I⁻¹(r × n)) × r) + 1/m + 1/(the rider's speed stat)) along n and is hit by the rider.
pub fn rider_kick<E: WallEnv + RollerEnv>(roller: &mut RollerBody, object: u32, rider: &mut WallRider, rider_id: u32, body: &CollBody, env: &mut E) {
    let touch = roller.tree.body(body);
    rider_kick_touch(roller, object, rider, rider_id, touch, env);
}

/// [`rider_kick`] after `Collide_BoarderSphTree`'s answer (the push and the point).
pub fn rider_kick_touch<E: WallEnv + RollerEnv>(roller: &mut RollerBody, object: u32, rider: &mut WallRider, rider_id: u32, touch: Option<(V4, V4)>, env: &mut E) {
    let Some((pen, contact)) = touch else { return };
    let vel = ld(rider.vel);
    let dir = unit(vel);
    let pn = unit(ld(pen));
    let c = ld(contact);
    let vp = point_velocity(&roller.rb, sub(c, ld(roller.rb.pos)));
    let closing = dot(pn, sub(vel, vp));
    let into = dot(pn, vel);
    if F(0.0) < into {
        let rec = Contact { pos: contact, dir: st(dir), normal: st(pn), speed_in: closing.0 };
        impact_check(rider, into.0, &rec, st(pn), env);
    }
    if !(closing < F(0.0)) {
        return;
    }
    let r = sub(c, ld(roller.rb.pos));
    let rn = cross(r, pn);
    let f = closing * F(1.3);
    let i = mat(&roller.rb.inv_inertia, rn, F(0.0));
    let k_vec = [i[1] * r[2] - i[2] * r[1], i[2] * r[0] - i[0] * r[2], i[0] * r[1] - i[1] * r[0], F(0.0)];
    let k = dot(pn, k_vec);
    let stat = env.speed_stat(rider);
    let j = f / ((k + F(roller.rb.inv_mass)) + F(1.0) / F(stat));
    roller.rb.add_impulse(st(r), st(scale(pn, j)));
    roller.on_hit(object, Some(rider_id), env);
}

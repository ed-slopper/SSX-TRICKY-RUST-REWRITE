//! The wipe-out as the original does it: `WipeoutMotion_Update` 0x10c2f0 (motion 5, state 0x13)
//! and `GetUpMotion_Update` 0x1102c8 (motion 6, state 0x14).
//!
//! The rider becomes one rigid body (`Ragdoll_InitFromBoarder` 0x10dad8) that tumbles under
//! gravity and drag (`Ragdoll_Step` 0x10dca8) and bounces off the world (`Ragdoll_Contacts`
//! 0x10e190). In the air it rights itself toward where it will land and can come out of it
//! (xA_FALLRECOVER); on the snow it rolls onto its back or front and slides (cmCC_BUTTSLIDE /
//! FACESLIDE) until slow and flat, then gets up on the spot (`Wipeout_StartGetUp` 0x10f178) with
//! one of six clips chosen by how it lies and where the course is, turning toward the course as
//! it does. Coming to rest upright with the board along the course it simply rides on.
//!
//! The body frame is the boarder's: X the board's nose (Bevy local -Z), Y its left (local -X),
//! Z up (local +Y). Units here are metres; the original's cm constants are divided by 100 (and
//! torques, which are angular momentum per mass, by 100^2).
use super::*;
use glam::{Mat3, Quat};
use std::sync::OnceLock;

/// A stand-in for the rider's collision spheres. `CollBody_PointMassInertia` 0x236530 weighs the
/// spheres 4,2,2,2,2,2,2,2,4,4,2,2,2,2,3,3 (table 0x3a3f68, 40 in all); where they sit comes from
/// each character's model, which we do not have, so these are placed on a 1.7 m rider standing
/// on a 1.5 m board: (mass, centre in the body frame in Bevy axes (x right, y up, z tail), radius).
const RAG: [(f32, [f32; 3], f32); 16] = [
    (4.0, [0.0, 0.95, 0.0], 0.18),   // pelvis
    (2.0, [0.0, 1.20, 0.0], 0.17),   // belly
    (2.0, [0.0, 1.42, 0.0], 0.18),   // chest
    (2.0, [0.0, 1.68, 0.0], 0.13),   // head
    (2.0, [-0.22, 1.30, 0.0], 0.08), // arms
    (2.0, [0.22, 1.30, 0.0], 0.08),
    (2.0, [-0.28, 1.00, 0.0], 0.07), // hands
    (2.0, [0.28, 1.00, 0.0], 0.07),
    (4.0, [0.0, 0.50, -0.20], 0.11), // knees (feet across the board, along its length)
    (4.0, [0.0, 0.50, 0.20], 0.11),
    (2.0, [0.0, 0.12, -0.25], 0.08), // feet
    (2.0, [0.0, 0.12, 0.25], 0.08),
    (2.0, [0.0, 0.05, -0.48], 0.06), // board
    (2.0, [0.0, 0.05, 0.48], 0.06),
    (3.0, [0.0, 0.06, -0.75], 0.06), // nose and tail
    (3.0, [0.0, 0.06, 0.75], 0.06),
];

/// Mass, centre of mass in the body frame, and the inverse inertia the original uses:
/// `Ragdoll_ComputeMassProps` 0x10d6f8 takes the point-mass inertia ×1.2, inverts it and scales
/// the inverse by 0.8 (so 1.5× the point masses').
struct RagProps { m: f32, c: Vec3, inv_i: Mat3 }
fn rag() -> &'static RagProps {
    static P: OnceLock<RagProps> = OnceLock::new();
    P.get_or_init(|| {
        let m: f32 = RAG.iter().map(|s| s.0).sum();
        let c = RAG.iter().map(|s| Vec3::from(s.1) * s.0).sum::<Vec3>() / m;
        let mut i = Mat3::ZERO;
        for s in RAG {
            let r = Vec3::from(s.1) - c;
            i += (Mat3::IDENTITY * r.length_squared() - Mat3::from_cols(r * r.x, r * r.y, r * r.z)) * s.0;
        }
        RagProps { m, c, inv_i: (i * 1.2).inverse() * 0.8 }
    })
}

// the original's clip ids for the wipe-out (game clip id → name via `AnimClip_ResolveAnmIndex`)
pub const CLIP_BUTTSLIDE: u16 = 0x2dc;
pub const CLIP_FACESLIDE: u16 = 0x2dd;
pub const CLIP_FALLING: u16 = 0x2de;
pub const CLIP_CRUNCH: u16 = 0x2df;
pub const CLIP_CRUNCH2STRETCH: u16 = 0x2e0;
pub const CLIP_FALLRECOVER: u16 = 0x224;
const IMPACTS: [&str; 6] = ["BUTT", "FACE", "FEET", "HEAD", "LEFT", "RIGHT"];

/// The name of a wipe-out clip ("bx" names are looked up in the board type's own set first).
pub fn clip_name(id: u16) -> String {
    match id {
        CLIP_BUTTSLIDE => "cmCC_BUTTSLIDE".into(),
        CLIP_FACESLIDE => "cmCC_FACESLIDE".into(),
        CLIP_FALLING => "cmF_FALLINGCYCLE".into(),
        CLIP_CRUNCH => "cmCC_CRUNCHCYCLE".into(),
        CLIP_CRUNCH2STRETCH => "cmCT_CRUNCH2STRETCH".into(),
        0x2e1..=0x2e6 => format!("cmCI_{}", IMPACTS[(id - 0x2e1) as usize]),
        0x2e8..=0x2ed => format!("cmI_{}", IMPACTS[(id - 0x2e8) as usize]),
        0x24a => "bxCT_ROLLBWD2BASE".into(),
        0x24b => "bxGU_FROMBUTTFWD".into(),
        0x24c => "bxGU_FROMBUTTRIGHT".into(),
        0x24d => "bxGU_FROMFACEBWD".into(),
        0x24e => "bxGU_FROMFACEFWD".into(),
        0x24f => "bxGU_FROMFACERIGHT".into(),
        0x250 => "bxCT_CRUNCH2BASE".into(),
        0x251 => "bxCT_STRETCH2BASE".into(),
        0x252 => "bxGU_FROMBUTTRIGHT2FAKIE".into(),
        0x253 => "bxGU_FROMFACERIGHT2FAKIE".into(),
        CLIP_FALLRECOVER => "bxA_FALLRECOVER".into(),
        _ => String::new(),
    }
}
/// Frames in a clip (30 a second) for a board type (0 bx, 1 fr, 2 ex), from the exported sets.
pub fn clip_frames(id: u16, kind: u8) -> f32 {
    let k = kind.min(2) as usize;
    let f: [f32; 3] = match id {
        CLIP_BUTTSLIDE => [22.0; 3], CLIP_FACESLIDE => [47.0; 3], CLIP_FALLING => [20.0; 3], CLIP_CRUNCH => [23.0; 3],
        CLIP_CRUNCH2STRETCH => [14.0; 3],
        0x2e1 | 0x2e3 => [19.0; 3], 0x2e2 => [16.0; 3], 0x2e4..=0x2e6 => [21.0; 3],
        0x2ea => [34.0; 3], 0x2e8..=0x2ed => [30.0; 3],
        0x24a => [51.0, 57.0, 56.0], 0x24b => [35.0, 35.0, 30.0], 0x24c => [35.0, 35.0, 33.0],
        0x24d => [41.0, 31.0, 33.0], 0x24e => [30.0, 35.0, 33.0], 0x24f => [35.0, 35.0, 33.0],
        0x250 | 0x251 => [21.0, 21.0, 10.0], 0x252 | 0x253 => [33.0; 3],
        CLIP_FALLRECOVER => [18.0; 3],
        _ => [30.0; 3],
    };
    f[k]
}
/// Clip category (descriptor table 0x325fa8): 17 the curled-up ("crunch") clips, 18 the
/// stretched-out ones.
fn category(id: u16) -> u8 { if matches!(id, CLIP_CRUNCH | CLIP_CRUNCH2STRETCH | 0x2e1..=0x2e6) { 17 } else { 18 } }

/// The body while down.
#[derive(Clone, Debug)]
pub struct Wipe {
    /// orientation (Bevy: local -Z the board's nose, +Y up) and angular momentum (per the body's units)
    pub q: Quat,
    pub l: Vec3,
    /// seconds down, since the body last touched anything, and in contact with the snow
    pub t: f32,
    pub t_air: f32,
    pub t_ground: f32,
    /// the original's clip id playing and seconds into it
    pub clip: u16,
    pub clip_t: f32,
    /// the last ground normal (probe or contact) and whether the probe found ground
    n: Vec3,
    probe: bool,
    /// getting up: the get-up clip and seconds into it
    pub getup: Option<(u16, f32)>,
}

/// A frame from the board's nose and up.
fn frame(fwd: Vec3, up: Vec3) -> Quat {
    let up = up.normalize_or(Vec3::Y);
    let f = (fwd - up * fwd.dot(up)).normalize_or(up.any_orthonormal_vector());
    let back = -f;
    Quat::from_mat3(&Mat3::from_cols(up.cross(back), up, back)).normalize()
}
fn ax_x(q: Quat) -> Vec3 { q * Vec3::NEG_Z }
fn ax_y(q: Quat) -> Vec3 { q * Vec3::NEG_X }
fn ax_z(q: Quat) -> Vec3 { q * Vec3::Y }
fn omega(q: Quat, l: Vec3) -> Vec3 {
    let w = q * (rag().inv_i * (q.inverse() * l));
    // |ω| is held to 23.56 rad/s (`RigidBody_UpdateDerived`)
    let s = w.length();
    if s > 23.56 { w * (23.56 / s) } else { w }
}
/// `RigidBody_ContactImpulse` 0x153ee8: restitution `e` and friction `mu` at `point`, normal `n`
/// pointing out of what was hit; only if the point is moving into it.
fn contact_impulse(com: Vec3, v: &mut Vec3, q: Quat, l: &mut Vec3, point: Vec3, n: Vec3, e: f32, mu: f32) -> bool {
    let p = rag();
    let r = point - com;
    let w = omega(q, *l);
    let vp = *v + w.cross(r);
    let vn = vp.dot(n);
    if vn >= 0.0 { return false; }
    let inv_iw = |x: Vec3| q * (p.inv_i * (q.inverse() * x));
    let k = |d: Vec3| 1.0 / p.m + d.dot(inv_iw(r.cross(d)).cross(r));
    let jn = -(1.0 + e) * vn / k(n);
    let mut j = n * jn;
    let vt = vp - n * vn;
    if vt.length() > 1e-4 && mu > 0.0 {
        let t = vt.normalize();
        j -= t * (vt.length() / k(t)).min(mu * jn);
    }
    *v += j / p.m;
    *l += r.cross(j);
    true
}

impl Rider {
    /// `WipeoutMotion_Enter` 0x10c198 with `Ragdoll_InitFromBoarder` 0x10dad8: the body takes the
    /// rider's velocity and no spin, then one bounce off what was hit (e 0.15) sets it turning;
    /// the surface is taken as 10 until something is touched, and an impact clip plays (cmI_*).
    pub(super) fn wipe_enter(&mut self) {
        let up = if self.grounded { self.normal } else { Vec3::Y };
        let q = frame(heading(self.yaw), up) * Quat::from_rotation_x(-self.flip);
        let (point, n) = self.crash_at.take().unwrap_or((self.pos, self.normal));
        let p = rag();
        let com = self.pos + q * p.c;
        let mut v = self.vel;
        let mut l = Vec3::ZERO;
        contact_impulse(com, &mut v, q, &mut l, point, n, 0.15, 0.0);
        self.vel = v;
        self.switch = false;
        self.surface = 10;
        self.clear_air();
        self.air_time = 0.0;
        let mut w = Wipe { q, l, t: 0.0, t_air: 0.0, t_ground: 0.0, clip: CLIP_CRUNCH, clip_t: 0.0, n, probe: self.grounded, getup: None };
        w.clip = impact_clip(&w, n, false);
        self.wipe = Some(w);
    }

    /// One step of motion 5 / 6.
    pub(super) fn wipe_step(&mut self, world: &CollisionWorld, dt: f32) {
        if self.wipe.is_none() { self.wipe_enter(); }
        let Some(mut w) = self.wipe.take() else { return };
        self.down_t += dt;
        w.t += dt;
        w.clip_t += dt;
        // clips that end chain on (end actions 5 and 6: impacts → crunch cycle, crunch-to-stretch → falling)
        if w.getup.is_none() && w.clip_t >= (clip_frames(w.clip, self.stats.kind) - 1.0) / 30.0 {
            match w.clip {
                0x2e1..=0x2ed => { w.clip = CLIP_CRUNCH; w.clip_t = 0.0; }
                CLIP_CRUNCH2STRETCH => { w.clip = CLIP_FALLING; w.clip_t = 0.0; }
                _ => {}
            }
        }
        // state 0x13 / 0x14 (`GetUpState_Update` 0x108198): out of bounds (surface 0) is a reset at once
        if self.surface == 0 { let at = self.safe; self.respawn(at); return; }
        let (was_clip, dbg) = (w.clip, std::env::var("TRICKY_WIPEDBG").is_ok());
        let keep = if w.getup.is_some() { self.getup_tick(&mut w, world, dt) }
            else if matches!(w.clip, CLIP_BUTTSLIDE | CLIP_FACESLIDE) { self.slide_tick(&mut w, world, dt) }
            else { self.tumble_tick(&mut w, world, dt) };
        if dbg && (w.clip != was_clip || !keep || ((w.t * 4.0) as i32) != (((w.t - dt) * 4.0) as i32)) {
            println!("    wipe t {:.2} clip {} air {:.2} ground {:.2} speed {:.1} grounded {} spin {:.1} up.n {:.2} surf {} pos {:.1?}{}", w.t, clip_name(w.clip), w.t_air, w.t_ground, self.vel.length(), self.grounded,
                omega(w.q, w.l).length(), ax_z(w.q).dot(w.n), self.surface, self.pos, if keep { "" } else { "  -> out" });
        }
        if !keep { return; }
        let f = ax_x(w.q);
        let flat = Vec3::new(f.x, 0.0, f.z);
        if flat.length() > 0.2 { self.yaw = f32::atan2(-flat.x, -flat.z); }
        self.wipe = Some(w);
        // out of the world, or into one of the course's out-of-bounds boxes
        if self.pos.y < world.min_y - 30.0 || world.in_reset(self.pos + Vec3::Y * 0.8, BODY_RADIUS) { let at = self.safe; self.respawn(at); }
    }

    /// The tumble (`WipeoutMotion_Update`, any clip but the slides). Returns false once the
    /// rider is out of the wipe-out.
    fn tumble_tick(&mut self, w: &mut Wipe, world: &CollisionWorld, dt: f32) -> bool {
        let p = rag();
        let ticks = dt * 60.0;
        let mut com = self.pos + w.q * p.c;
        let mut v = self.vel;
        // ---- Ragdoll_Step: gravity, drag of half the momentum a second (and of the spin), and
        // for 0.15 s after touching anything a pull of 6 m/s² toward the race line along the snow
        let mut a = Vec3::NEG_Y * 19.0084 - v * 0.5;
        if w.t_air < 0.15 {
            if let Some((near, _)) = self.course_pts {
                let d = near - com;
                let d = d - w.n * d.dot(w.n);
                if d.length() > 1e-3 { a += d.normalize() * 6.0; }
            }
        }
        // RigidBody_Integrate 0x153c50: explicit Euler, position and orientation from the old rates
        let om = omega(w.q, w.l);
        com += v * dt;
        w.q = (w.q + Quat::from_xyzw(om.x, om.y, om.z, 0.0) * w.q * (0.5 * dt)).normalize();
        v += a * dt;
        w.l -= w.l * (0.5 * dt);
        // ---- Ragdoll_Contacts: the deepest touch, twice; e = 0.8 - 0.7|n.up| and mu = 0.15|n.up|
        // (both 0 under 1 m/s), then momentum and spin ×0.95
        let mut contact: Option<(Vec3, u8)> = None;
        for _ in 0..2 {
            let mut best: Option<(f32, Vec3, Vec3, Vec3, u8)> = None; // depth, push, normal, point, surface
            for s in RAG {
                let c = com + w.q * (Vec3::from(s.1) - p.c);
                if let Some(h) = world.ground(c, s.2 + 0.5, s.2 + 0.5) {
                    let d = (c.y - h.y) * h.normal.y;
                    if d < s.2 && best.is_none_or(|b| s.2 - d > b.0) { best = Some((s.2 - d, h.normal * (s.2 - d), h.normal, c - h.normal * d, h.surface)); }
                }
                let (c2, ns) = world.push_out(c, s.2);
                if !ns.is_empty() {
                    let push = c2 - c;
                    let depth = push.length();
                    if depth > 1e-5 && best.is_none_or(|b| depth > b.0) { let n = push / depth; best = Some((depth, push, n, c - n * (s.2 - depth), 10)); }
                }
            }
            let Some((_, push, n, point, surface)) = best else { break };
            com += push;
            let (e, mu) = if v.length() >= 1.0 { (0.8 - 0.7 * n.y.abs(), 0.15 * n.y.abs()) } else { (0.0, 0.0) };
            if contact_impulse(com, &mut v, w.q, &mut w.l, point, n, e, mu) {
                let k = 0.95f32.powf(ticks);
                v *= k;
                w.l *= k;
            }
            if contact.is_none_or(|c| n.y > c.0.y) { contact = Some((n, surface)); }
        }
        self.pos = com - w.q * p.c;
        self.vel = v;
        // ---- Ragdoll_GroundProbe 0x10fd58: straight down through the hips
        let hip = com + w.q * (Vec3::from(RAG[0].1) - p.c);
        let probe = world.ground(hip, 0.5, 4.0);
        w.probe = probe.is_some();
        match probe { Some(h) => { w.n = h.normal; self.surface = h.surface; } None => w.n = Vec3::Y }
        let bad = |s: u8| s == 6 || s == 10;
        match contact {
            None => {
                self.grounded = false;
                if w.t_air > 0.1 {
                    // in the air: curled up turns to stretched out (cmCT_CRUNCH2STRETCH) and the body
                    // rights itself toward where it will land unless that is under 0.1 s away
                    if w.clip == CLIP_CRUNCH { w.clip = CLIP_CRUNCH2STRETCH; w.clip_t = 0.0; }
                    match predict_landing(world, com, v) {
                        Some((tl, nl, vl, sl)) if tl > 0.1 && !bad(sl) => {
                            let (x, z) = (ax_x(w.q), ax_z(w.q));
                            let d = vl - nl * vl.dot(nl);
                            let d = d.normalize_or(x);
                            // Z toward the landing normal (2509.5 m²/s² per unit mass), the board's
                            // nose (or tail) toward the way it will slide (1254.8)
                            let along = if x.dot(d) > 0.0 { d } else { -d };
                            w.l += (z.cross(nl) * 25.095134 + x.cross(along) * 12.547567) * p.m * dt;
                            w.l *= 1.0 - 3.3475056 * dt;
                            if z.dot(nl) > 0.9 && omega(w.q, w.l).length() < 4.0 {
                                // xA_FALLRECOVER: back to an ordinary jump (state 0xD), landed as usual
                                self.end_wipe(w.clone(), CLIP_FALLRECOVER, false);
                                return false;
                            }
                            w.t_ground = 0.0;
                        }
                        _ => { w.t_ground = 0.0; w.l *= 1.0 - 3.3475056 * dt; }
                    }
                } else {
                    if w.probe { w.t_ground += dt; } else { w.t_ground = 0.0; }
                    w.l *= 1.0 - 3.3475056 * dt;
                }
                w.t_air += dt;
            }
            Some((cn, cs)) => {
                self.grounded = true;
                if !w.probe || cn.y < w.n.y { w.n = cn; }
                self.surface = cs;
                // landing from a tumble plays an impact clip: curled (cmCI_*) or stretched (cmI_*)
                if (w.t_ground == 0.0 && w.clip == CLIP_CRUNCH) || w.clip == CLIP_FALLING || w.clip == CLIP_CRUNCH2STRETCH {
                    w.clip = impact_clip(w, w.n, category(w.clip) == 17);
                    w.clip_t = 0.0;
                }
                w.t_air = 0.0;
                w.t_ground += dt;
                w.l *= 1.0 - 3.3475056 * dt;
            }
        }
        self.normal = w.n;
        let n = w.n;
        if w.probe && w.t_ground > 0.25 && !bad(self.surface) && n.y > 0.34906584 {
            // after a quarter second on the snow: onto the back (nose up) or the front and slide,
            // or roll over until one or the other (40 m²/s² per unit mass)
            let xn = ax_x(w.q).dot(n);
            if xn > 0.75 { w.clip = CLIP_BUTTSLIDE; w.clip_t = 0.0; }
            else if xn < -0.75 { w.clip = CLIP_FACESLIDE; w.clip_t = 0.0; }
            else { w.l += ax_x(w.q).cross(n) * (40.0 * xn.signum() * p.m * dt); }
        }
        // come to rest upright with the board along the course: ride on (bxCT_CRUNCH2BASE / STRETCH2BASE)
        if w.t > 0.5 && w.t_ground > 0.1 && !bad(self.surface) && omega(w.q, w.l).length() < 7.0 && n.y > 0.34906584 {
            let d = self.course_pts.map(|(_, ahead)| (ahead - self.pos).normalize_or_zero()).unwrap_or(heading(self.yaw));
            if ax_x(w.q).dot(d).abs() > 0.8 && ax_z(w.q).dot(n) > 0.85 && world.ground(self.pos, 1.0, 1.0).is_some() {
                let clip = if category(w.clip) == 17 { 0x250 } else { 0x251 };
                self.end_wipe(w.clone(), clip, true);
                return false;
            }
        }
        // too long: three seconds on the snow or seven down. A computer rider gets up where it
        // is; the player is put back on the course.
        if w.t_ground >= 3.0 || w.t > 7.0 {
            if self.ai {
                w.clip = if ax_x(w.q).dot(n) > 0.0 { CLIP_BUTTSLIDE } else { CLIP_FACESLIDE };
                self.start_getup(w);
            } else { let at = self.safe; self.respawn(at); return false; }
        }
        true
    }

    /// The slide (clips 0x2dc / 0x2dd): the boarder slides on the snow under gravity, losing
    /// 1/60 of its speed a tick, and lies down flat; slow and flat enough it gets up.
    fn slide_tick(&mut self, w: &mut Wipe, world: &CollisionWorld, dt: f32) -> bool {
        w.t_ground += dt;
        self.slide_physics(world, dt);
        let Some(h) = world.ground(self.pos, 2.0, 4.0).filter(|h| h.normal.y >= 0.3 && h.surface != 6 && h.surface != 10) else {
            // off the edge or onto something unskiable: tumble again (cmF_FALLINGCYCLE), the
            // body re-made with a bounce off the snow
            w.clip = CLIP_FALLING;
            w.clip_t = 0.0;
            let p = rag();
            let com = self.pos + w.q * p.c;
            let mut v = self.vel;
            w.l = Vec3::ZERO;
            contact_impulse(com, &mut v, w.q, &mut w.l, self.pos, w.n, 0.15, 0.0);
            self.vel = v;
            self.grounded = false;
            return true;
        };
        w.n = h.normal;
        self.normal = h.normal;
        self.surface = h.surface;
        self.grounded = true;
        // lie flat: the nose (butt slide) or the tail (face slide) to the snow's normal, 1/12 a tick
        let x = if w.clip == CLIP_FACESLIDE { -ax_x(w.q) } else { ax_x(w.q) };
        let a = x.cross(w.n);
        let s = a.length();
        if s > 1e-5 {
            let ang = s.min(0.99999).asin() * (1.0 - (11.0f32 / 12.0).powf(dt * 60.0));
            w.q = (Quat::from_axis_angle(a / s, ang) * w.q).normalize();
        }
        // up when flat and slower than min(80 km/h, 30 km/h + 25 km/h for each second sliding)
        let lim = (8.333333 + 6.944444 * w.t_ground).min(22.222222);
        if s < 0.1 && self.vel.length() < lim && ax_x(w.q).dot(w.n).abs() > 0.9 { self.start_getup(w); }
        true
    }

    /// The slide's motion, shared by the get-up: position on, gravity along the slope, ×0.98333
    /// a tick, held to the snow (at most 8.3 cm a tick), nothing into the snow; walls stop it.
    fn slide_physics(&mut self, world: &CollisionWorld, dt: f32) {
        let ticks = dt * 60.0;
        let g = SURFACES[(self.surface as usize).min(19)][0] * 0.01;
        let n = self.normal;
        self.pos += self.vel * dt;
        self.vel += Vec3::new(n.x * n.y * g, n.y * g - g, n.z * n.y * g) * dt;
        self.vel *= 0.98333335f32.powf(ticks);
        if let Some(h) = world.ground(self.pos, 2.0, 4.0) {
            let d = ((self.pos.y - h.y) * h.normal.y).clamp(-0.08333333 * ticks, 0.08333333 * ticks);
            self.pos -= h.normal * d;
            self.normal = h.normal;
        }
        let n = self.normal;
        self.vel -= n * self.vel.dot(n);
        let lift = Vec3::Y * 0.35;
        let (c, contacts) = world.push_out(self.pos + lift, 0.3);
        self.pos = c - lift;
        for n in contacts { let vn = self.vel.dot(n); if vn < 0.0 { self.vel -= n * vn; } }
    }

    /// `Wipeout_StartGetUp` 0x10f178: stand the body up where it lies and pick the get-up by
    /// which side it lies on and where the course is (the point 8 m ahead): head toward it → roll
    /// back over (CT_ROLLBWD2BASE) or push up forwards (GU_FROMFACEFWD); feet toward it → sit up
    /// (GU_FROMBUTTFWD) or GU_FROMFACEBWD; off to a side → GU_FROM*RIGHT, ending switch when the
    /// course is to the left (alpine boards have their own *2FAKIE clips for that).
    fn start_getup(&mut self, w: &mut Wipe) {
        let d = self.course_pts.map(|(_, ahead)| (ahead - self.pos).normalize_or_zero()).unwrap_or(heading(self.yaw));
        let (y, z) = (ax_y(w.q), ax_z(w.q));
        let (f, r) = (z.dot(d), y.dot(d));
        let face = w.clip == CLIP_FACESLIDE;
        // stand up: Z to where the nose (butt) or tail (face) points
        let up_to = if face { -ax_x(w.q) } else { ax_x(w.q) };
        let a = z.cross(up_to);
        if a.length() > 1e-5 { w.q = (Quat::from_axis_angle(a.normalize(), z.angle_between(up_to)) * w.q).normalize(); }
        let zz = ax_z(w.q);
        let half = |w: &mut Wipe, ang: f32| w.q = (Quat::from_axis_angle(zz, ang) * w.q).normalize();
        let alpine = self.stats.kind == 2;
        let clip = if f > std::f32::consts::FRAC_1_SQRT_2 {
            if face { 0x24e } else { half(w, std::f32::consts::PI); 0x24a }
        } else if r > std::f32::consts::FRAC_1_SQRT_2 {
            if !alpine { self.switch = true; }
            if alpine { if face { 0x253 } else { 0x252 } } else if face { 0x24f } else { 0x24c }
        } else if f < -std::f32::consts::FRAC_1_SQRT_2 {
            if face { half(w, std::f32::consts::PI); 0x24d } else { 0x24b }
        } else if face { 0x24f } else { 0x24c };
        // the board's nose turned a quarter toward the course for the side get-ups
        if matches!(clip, 0x24c | 0x24f | 0x252 | 0x253) {
            let x0 = ax_x(w.q);
            let ql = Quat::from_axis_angle(zz, std::f32::consts::FRAC_PI_2) * w.q;
            let qr = Quat::from_axis_angle(zz, -std::f32::consts::FRAC_PI_2) * w.q;
            w.q = if ax_x(ql).dot(d) >= ax_x(qr).dot(d) { ql } else { qr }.normalize();
            let _ = x0;
        }
        w.getup = Some((clip, 0.0));
        w.clip = clip;
        w.clip_t = 0.0;
        self.getup = 0.0;
    }

    /// `GetUpMotion_Update` 0x1102c8: the slide's motion while the get-up clip plays, the body
    /// turning (1/12 a tick) to stand on the snow facing the point 8 m along the course; riding
    /// resumes 2n - 1 ticks after the n-frame clip starts, at the speed left.
    fn getup_tick(&mut self, w: &mut Wipe, world: &CollisionWorld, dt: f32) -> bool {
        self.slide_physics(world, dt);
        self.grounded = true;
        if let Some(h) = world.ground(self.pos, 2.0, 4.0) { self.surface = h.surface; w.n = h.normal; }
        let d = self.course_pts.map(|(_, ahead)| ahead - self.pos).unwrap_or(heading(self.yaw));
        let target = frame(d, self.normal);
        w.q = w.q.slerp(target, 1.0 - (11.0f32 / 12.0).powf(dt * 60.0)).normalize();
        let (clip, t) = w.getup.unwrap_or((0x24e, 0.0));
        let t = t + dt;
        w.getup = Some((clip, t));
        self.getup = t;
        if t >= (2.0 * clip_frames(clip, self.stats.kind) - 1.0) / 60.0 {
            self.end_wipe(w.clone(), 0, true);
            return false;
        }
        true
    }

    /// Back to riding (on the snow) or to an ordinary jump, with the clip that brings the body
    /// back to its base pose (0 for none).
    fn end_wipe(&mut self, w: Wipe, clip: u16, on_snow: bool) {
        let f = ax_x(w.q);
        let flat = Vec3::new(f.x, 0.0, f.z);
        if flat.length() > 1e-3 { self.yaw = f32::atan2(-flat.x, -flat.z); }
        self.crashed = 0.0;
        self.getup = -1.0;
        self.wipe = None;
        self.grounded = on_snow;
        self.spin = 0.0;
        self.flip = 0.0;
        self.clear_air();
        self.air_time = 0.0;
        self.charge = 0.0;
        if on_snow { self.normal = w.n; self.vel -= w.n * self.vel.dot(w.n).min(0.0); }
        self.cap = SPEED_CAP.max(self.vel.length());
        self.recover = if clip != 0 { Some((clip, 0.0)) } else { None };
    }
}

/// `Wipeout_PlayImpactClip` 0x10f9a0: which way the body meets the snow (feet, head, a side,
/// back or front), curled (cmCI_*) or stretched (cmI_*).
fn impact_clip(w: &Wipe, n: Vec3, crunch: bool) -> u16 {
    let (x, y, z) = (ax_x(w.q).dot(n), ax_y(w.q).dot(n), ax_z(w.q).dot(n));
    let i = if z < -0.7 { 3 } else if z > 0.7 { 2 } else if y > 0.7 { 4 } else if y < -0.7 { 5 } else if x > 0.0 { 0 } else { 1 };
    if crunch { 0x2e1 + i } else { 0x2e8 + i }
}

/// `AirPredict_Tick` 0x123ec8: where a body thrown from `p` at `v` (gravity, drag 0.5/s) comes
/// down: (seconds, ground normal, velocity, surface).
fn predict_landing(world: &CollisionWorld, mut p: Vec3, mut v: Vec3) -> Option<(f32, Vec3, Vec3, u8)> {
    let h = 1.0 / 20.0;
    let mut t = 0.0;
    while t < 4.0 {
        v += (Vec3::NEG_Y * 19.0084 - v * 0.5) * h;
        let q = p + v * h;
        t += h;
        if let Some(g) = world.ground(q, (q.y - p.y).abs() + 0.6, 0.6) {
            if g.y >= q.y - 0.5 { return Some((t, g.normal, v, g.surface)); }
        }
        p = q;
    }
    None
}

/// The nearest point of a polyline course to `pos` (searched near `idx`) and the point 8 m
/// further along it (the original's 0x350 / 0x360, `fn 0x119d30`).
pub fn course_points(line: &[Vec3], idx: usize, pos: Vec3) -> Option<(Vec3, Vec3)> {
    if line.len() < 2 { return None; }
    let lo = idx.saturating_sub(10).min(line.len() - 2);
    let hi = (idx + 10).min(line.len() - 2);
    let mut best = (f32::MAX, lo, 0.0f32, line[lo]);
    for i in lo..=hi {
        let (a, b) = (line[i], line[i + 1]);
        let ab = b - a;
        let t = if ab.length_squared() > 1e-8 { ((pos - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + ab * t;
        let d = (q - pos).length_squared();
        if d < best.0 { best = (d, i, t, q); }
    }
    let (_, mut i, t, near) = best;
    let mut left = 8.0f32;
    let mut at = near;
    let mut seg_rest = (line[i + 1] - near).length();
    let _ = t;
    loop {
        if seg_rest >= left { at += (line[i + 1] - at).normalize_or_zero() * left; break; }
        left -= seg_rest;
        at = line[i + 1];
        i += 1;
        if i + 1 >= line.len() { break; }
        seg_rest = (line[i + 1] - at).length();
    }
    Some((near, at))
}

//! Things on the course that get knocked away when a rider hits them (path markers, crash
//! bags, small billboards). Each is a ball as far as physics goes: it flies, bounces and rolls
//! to a stop on the snow.

use crate::collide::CollisionWorld;
use crate::rider::{Rider, BODY_RADIUS, GRAVITY};
use bevy::math::{Quat, Vec3};

/// What touching it does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pickup { None, Multiplier(u32), SpeedBoost, TrickBoost }
#[derive(Clone, PartialEq, Debug)]
pub enum Kind {
    /// knocked away and left lying around
    Knock,
    /// vanishes on contact (glass, fences, pickups) and releases its broken pieces
    Touch { pieces: Vec<usize>, pickup: Pickup },
    /// a broken piece: hidden until its owner is touched, then thrown
    Piece,
}

pub struct Prop {
    pub kind: Kind,
    pub hidden: bool,
    /// box around the mesh in the prop's own frame
    lo: Vec3,
    hi: Vec3,
    /// resting place
    pub home: (Vec3, Quat),
    /// where the mesh origin is now
    pub pos: Vec3,
    pub rot: Quat,
    vel: Vec3,
    spin: Vec3,
    /// centre of the ball relative to the mesh origin, in the prop's own frame
    centre: Vec3,
    pub radius: f32,
    pub moving: bool,
}

impl Prop {
    pub fn new(pos: Vec3, rot: Quat, centre: Vec3, radius: f32) -> Self {
        Self { kind: Kind::Knock, hidden: false, lo: centre - Vec3::splat(radius), hi: centre + Vec3::splat(radius),
               home: (pos, rot), pos, rot, vel: Vec3::ZERO, spin: Vec3::ZERO, centre, radius: radius.clamp(0.25, 1.6), moving: false }
    }
    pub fn with_box(kind: Kind, pos: Vec3, rot: Quat, lo: Vec3, hi: Vec3) -> Self {
        let size = hi - lo;
        let mut p = Self::new(pos, rot, (lo + hi) * 0.5, (size.x + size.y + size.z) / 6.0);
        p.hidden = kind == Kind::Piece;
        p.kind = kind;
        (p.lo, p.hi) = (lo, hi);
        p
    }
    pub fn reset(&mut self) {
        (self.pos, self.rot) = self.home;
        self.vel = Vec3::ZERO; self.spin = Vec3::ZERO; self.moving = false;
        self.hidden = self.kind == Kind::Piece;
    }
    /// Is the rider's body inside this prop's box?
    fn touching(&self, r: &Rider) -> bool {
        let q = self.rot.inverse() * (r.pos + Vec3::Y * 0.8 - self.pos);
        let m = Vec3::splat(BODY_RADIUS + 0.2);
        q.cmpge(self.lo - m).all() && q.cmple(self.hi + m).all()
    }
    /// Glass, fences and pickups: gone the moment a rider reaches them. Returns the pieces to throw.
    pub fn touched_by(&mut self, r: &mut Rider) -> Option<Vec<usize>> {
        let Kind::Touch { pieces, pickup } = &self.kind else { return None };
        if self.hidden || !self.touching(r) { return None; }
        self.hidden = true;
        match pickup {
            Pickup::Multiplier(m) => r.pick_multiplier(*m),
            Pickup::SpeedBoost => r.pick_boost(0.5, true),
            Pickup::TrickBoost => r.pick_boost(1.0, false),
            Pickup::None => {}
        }
        Some(pieces.clone())
    }
    /// A broken piece set loose by a rider going through its owner.
    pub fn throw(&mut self, rider_vel: Vec3, seed: usize) {
        if self.kind != Kind::Piece { return; }
        self.hidden = false;
        // spread the pieces a little differently each
        let a = seed as f32 * 2.399;
        let side = Vec3::new(a.cos(), 0.0, a.sin());
        self.vel = rider_vel * (0.45 + 0.1 * (seed % 4) as f32) + side * 2.5 + Vec3::Y * (2.0 + (seed % 3) as f32);
        self.spin = side * 5.0 + Vec3::Y * 2.0;
        self.moving = true;
    }
    fn ball(&self) -> Vec3 { self.pos + self.rot * self.centre }

    /// If the rider is touching this prop, send it flying. Returns true on a hit.
    pub fn hit_by(&mut self, r: &mut Rider) -> bool {
        if self.kind != Kind::Knock || self.hidden { return false; }
        let body = r.pos + Vec3::Y * 0.8;
        let d = self.ball() - body;
        let reach = self.radius + BODY_RADIUS;
        if d.length_squared() > reach * reach { return false; }
        let speed = r.vel.length();
        // only something moving towards it knocks it; a prop already flying away is left alone
        if speed < 1.5 || (self.moving && (self.vel - r.vel).dot(d) > 0.0) { return false; }
        let away = Vec3::new(d.x, 0.0, d.z).normalize_or(r.vel.normalize_or(Vec3::X));
        self.vel = r.vel * 0.85 + away * (2.0 + speed * 0.12) + Vec3::Y * (3.5 + speed * 0.15);
        self.spin = Vec3::Y.cross(self.vel).normalize_or(Vec3::X) * (4.0 + speed * 0.25) + away * 2.0;
        self.moving = true;
        r.vel *= 0.94;
        true
    }

    pub fn step(&mut self, world: &CollisionWorld, dt: f32) {
        if !self.moving { return; }
        self.vel.y -= GRAVITY * dt;
        self.vel /= 1.0 + 0.02 * self.vel.length() * dt;
        self.pos += self.vel * dt;
        self.rot = (Quat::from_scaled_axis(self.spin * dt) * self.rot).normalize();
        let c = self.ball();
        if let Some(hit) = world.ground(c, self.radius + 1.0, self.radius) {
            if c.y - self.radius < hit.y {
                self.pos.y += hit.y + self.radius - c.y;
                let vn = self.vel.dot(hit.normal);
                if vn < 0.0 {
                    // bounce, lose some speed along the snow, slow the tumble
                    let tangent = self.vel - hit.normal * vn;
                    self.vel = tangent * 0.82 - hit.normal * vn * 0.35;
                    self.spin *= 0.75;
                }
                if self.vel.length() < 0.7 { self.moving = false; self.vel = Vec3::ZERO; self.spin = Vec3::ZERO; }
            }
        }
        if self.pos.y < world.min_y - 40.0 { self.moving = false; }
    }
}

/// Riders shove each other apart instead of passing through.
pub fn bump(a: &mut Rider, b: &mut Rider) {
    let d = b.pos - a.pos;
    if d.y.abs() > 1.6 { return; }
    let flat = Vec3::new(d.x, 0.0, d.z);
    let dist = flat.length();
    let reach = 0.95;
    if dist >= reach || dist < 1e-4 { return; }
    let n = flat / dist;
    let push = (reach - dist) * 0.5;
    a.pos -= n * push;
    b.pos += n * push;
    // trade the part of the speed that is driving them together
    let closing = (a.vel - b.vel).dot(n);
    if closing > 0.0 {
        a.vel -= n * closing * 0.75;
        b.vel += n * closing * 0.75;
    }
}

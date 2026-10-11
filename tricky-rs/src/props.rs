//! Things on the course that get knocked away when a rider hits them (path markers, crash
//! bags, small billboards). Each is a ball as far as physics goes: it flies, bounces and rolls
//! to a stop on the snow.
//!
//! STANDIN: a ball's physics, for the game's own object collision and bounce (port-notes/world.md)

use crate::collide::CollisionWorld;
use crate::rider::{Rider, BODY_RADIUS};
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
    /// scenery the level scripts switch on and off by event (race-only barriers, the start gate)
    Static,
    /// riding a spline (trains, ski lift chairs): which mover and which copy
    Mover(usize, usize),
}

pub struct Prop {
    pub kind: Kind,
    /// the level instance it was made from
    pub inst: usize,
    /// how it flies when broken off (the level script's debris settings), if it has them
    pub debris: Option<crate::logic::DebrisDef>,
    /// a pickup that stays where it is when touched (the multiplier gems, speed boost pads)
    pub stays: bool,
    /// broken by the level scripts (cracked glass, `worldanim`), not by touching it
    pub scripted: bool,
    gravity: f32,
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
    /// thrown on a fixed path that ignores the ground and vanishes (cMeshAnimNode: path markers,
    /// broken pieces) rather than a rolling rigid body (cRollerNode: crash bags, cans)
    pub ballistic: bool,
    /// seconds since it was set moving, and how long a thrown piece lasts
    t: f32,
    life: f32,
    pub mass: f32,
    seed: u32,
    /// knocked or broken by a rider this frame, at this speed (m/s): for its collision sound
    pub knock: Option<f32>,
}

impl Prop {
    pub fn new(pos: Vec3, rot: Quat, centre: Vec3, radius: f32) -> Self {
        Self { kind: Kind::Knock, inst: usize::MAX, debris: None, stays: false, scripted: false, gravity: 5.88, hidden: false, lo: centre - Vec3::splat(radius), hi: centre + Vec3::splat(radius),
               home: (pos, rot), pos, rot, vel: Vec3::ZERO, spin: Vec3::ZERO, centre, radius: radius.clamp(0.25, 1.6), moving: false,
               ballistic: false, t: 0.0, life: 2.0, mass: 5.0, seed: (pos.x.to_bits() ^ pos.z.to_bits()).wrapping_mul(2654435761) | 1, knock: None }
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
        self.vel = Vec3::ZERO; self.spin = Vec3::ZERO; self.moving = false; self.t = 0.0;
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
        if self.hidden || self.scripted || !self.touching(r) { return None; }
        if self.stays {
            // the scripts run on every touch; the pickups only ever raise what the rider has
            match pickup {
                Pickup::Multiplier(m) => if r.multiplier < *m { r.pick_multiplier(*m) },
                Pickup::SpeedBoost => if r.speed_timer < 4.5 { r.pick_speed(5.0) },
                Pickup::TrickBoost => if r.spin_timer < 14.5 { r.pick_spin(15.0) },
                Pickup::None => {}
            }
            return None;
        }
        self.hidden = true;
        self.knock = Some(r.vel.length());
        match pickup {
            Pickup::Multiplier(m) => r.pick_multiplier(*m),
            Pickup::SpeedBoost => r.pick_speed(5.0),
            Pickup::TrickBoost => r.pick_spin(15.0),
            Pickup::None => {}
        }
        Some(pieces.clone())
    }
    /// A broken piece set loose by a rider going through its owner.
    pub fn throw(&mut self, rider_vel: Vec3, seed: usize) {
        if self.kind != Kind::Piece { return; }
        self.hidden = false;
        // cMeshAnimNode: half the rider's velocity plus a random spread (2 m/s each way across,
        // up to 4 m/s up), a random tumble, a light pull down; gone after two seconds
        self.seed ^= (seed as u32).wrapping_mul(0x9E37_79B9) | 1;
        match self.debris {
            Some(d) => {
                // cMeshAnimNode: the script's velocity (or the rider's) times its multiplier, a random
                // spread (either way across, up only), its own gravity and life
                let base = if d.base.iter().all(|v| *v == 0.0) { rider_vel } else { Vec3::new(d.base[0], d.base[2], -d.base[1]) * 0.01 };
                self.launch_ballistic(base * d.mult);
                let (a, b, c) = (self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0, self.rnd());
                self.vel = base * d.mult + Vec3::new(a * d.spread[0], c * d.spread[2], -b * d.spread[1]) * 0.01;
                self.gravity = d.gravity * 9.8;
                if d.life > 0.0 { self.life = d.life; }
            }
            None => self.launch_ballistic(rider_vel * 0.5),
        }
    }
    fn rnd(&mut self) -> f32 { self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5; (self.seed % 10000) as f32 / 10000.0 }
    fn launch_ballistic(&mut self, base: Vec3) {
        let (a, b, c) = (self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0, self.rnd());
        self.vel = base + Vec3::new(a * 2.0, c * 4.0, b * 2.0);
        let axis = Vec3::new(self.rnd() - 0.5, self.rnd() - 0.5, self.rnd() - 0.5).normalize_or(Vec3::X);
        self.spin = axis * (2.0 + 8.0 * self.rnd());
        self.ballistic = true;
        self.moving = true;
        self.t = 0.0;
        self.life = 2.0;
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
        let n = d.normalize_or(r.vel.normalize_or(Vec3::X));
        let s = r.vel.dot(n).max(0.0);
        let dir = r.vel.normalize_or(n);
        if self.ballistic || self.mass <= 0.0 {
            // a path marker: its pieces fly off and vanish; the rider stumbles (mass 0)
            self.launch_ballistic(dir * 0.5 * s);
            r.stumble_by(false);
        } else {
            // cRollerNode_LaunchFromHit: half the approach speed along the rider's heading and a
            // pop of up to 6 m/s (lighter goes higher); spin 10 rad/s; the rider keeps his speed
            self.vel = dir * 0.5 * s + Vec3::Y * (1.0 + 50.0 / self.mass.max(0.1)).min(6.0);
            self.spin = n * 10.0;
            self.moving = true;
            self.t = 0.0;
        }
        self.knock = Some(speed);
        true
    }

    pub fn step(&mut self, world: &CollisionWorld, dt: f32) {
        if !self.moving { return; }
        self.t += dt;
        self.pos += self.vel * dt;
        self.rot = (Quat::from_scaled_axis(self.spin * dt) * self.rot).normalize();
        if self.ballistic {
            // no ground: it falls (58800 x 0.01 cm/s2 = 5.88 m/s2) and is gone after its time
            self.vel.y -= self.gravity * dt;
            if self.t >= self.life { self.moving = false; self.hidden = true; }
            return;
        }
        // a rolling body: real gravity, drag that grows the longer it moves, no bounce on the snow
        self.vel.y -= 9.8 * dt;
        let c = self.ball();
        let mut touching = false;
        if let Some(hit) = world.ground(c, self.radius + 1.0, self.radius) {
            if c.y - self.radius < hit.y {
                touching = true;
                self.pos.y += (hit.y + self.radius - c.y).max(0.02);
                let vn = self.vel.dot(hit.normal);
                let k = 0.96667f32.powf(dt * 60.0);
                self.vel *= k;
                self.spin *= k;
                if vn < 0.0 {
                    // friction takes half the sliding speed at the contact, then no bounce (e = 0)
                    let tangent = self.vel - hit.normal * vn;
                    self.vel = tangent * 0.5f32.powf(dt * 60.0 * 0.25);
                }
            }
        }
        if touching { let c = 0.833 * (1.0 + 2.0 * self.t); self.vel /= 1.0 + c * dt; self.spin /= 1.0 + c * dt; }
        // asleep below about 1 m/s or after ten seconds, where it lies
        if (touching && self.vel.length() < 1.0) || self.t >= 10.0 { self.moving = false; self.vel = Vec3::ZERO; self.spin = Vec3::ZERO; }
        if self.pos.y < world.min_y - 40.0 { self.moving = false; }
    }
}

/// Riders shove each other apart instead of passing through.
/// Two riders running into each other (`Boarder_RiderCollisions` / `Boarder_RiderHit`): pushed
/// apart, and each takes the closing speed weighted by the other's mass. A hard enough hit puts a
/// rider down; if only one goes down the other gets a full meter.
pub fn bump(a: &mut Rider, b: &mut Rider) {
    let d = b.pos - a.pos;
    if d.y.abs() > 1.6 { return; }
    let flat = Vec3::new(d.x, 0.0, d.z);
    let dist = flat.length();
    let reach = 0.95;
    if dist >= reach || dist < 1e-4 { return; }
    let n = flat / dist;
    let push = (reach - dist) * 0.55;
    a.pos -= n * push * 0.5;
    b.pos += n * push * 0.5;
    let closing = (a.vel - b.vel).dot(n);
    if closing <= 0.0 { return; }
    // boosting makes a rider up to nine times as hard to stop (Boarder_Mass: x (1 + 8 x boost level))
    let lvl = |r: &Rider| if r.boosting { r.boost_level() } else { 0.0 };
    let (ma, mb) = (a.stats.mass().max(1.0) * (1.0 + 8.0 * lvl(a)), b.stats.mass().max(1.0) * (1.0 + 8.0 * lvl(b)));
    let (da, db) = (closing * mb / (ma + mb), closing * ma / (ma + mb));
    let a_was = a.crashed > 0.0;
    let b_was = b.crashed > 0.0;
    let a_down = !a_was && !b_was && a.take_hit(-n * da, false);
    let b_down = !a_was && !b_was && b.take_hit(n * db, false);
    if a_was || b_was { return; }
    match (a_down, b_down) {
        (true, false) => b.add_meter(1.0),
        (false, true) => a.add_meter(1.0),
        (true, true) => { a.add_meter(-0.1); b.add_meter(-0.1); }
        _ => {}
    }
}

/// A shove (clip 0x15): the push a rider gives one alongside, as a change of velocity for them.
pub fn shove_push(from: &Rider, to: &Rider) -> Vec3 {
    let d = to.pos - from.pos;
    let dir = Vec3::new(d.x, 0.0, d.z).normalize_or_zero();
    let (m, mo) = (from.stats.mass().max(1.0), to.stats.mass().max(1.0));
    let closing = (from.vel - to.vel).dot(dir).max(0.0);
    let j = (1.0858 + closing * m / (m + mo)) * (0.54663557 + 0.739932 * from.stats.stability);
    dir * j
}

/// Tokyo Megaplex's lap tube (level effects cLapBoostNode, cZBoostNode, cTubeEndBoostNode): at the
/// bottom of the course a rider with laps to go is drawn into the glass tube and blown upward at
/// 25 m/s, put straight up to the top of the course (z = 0) and thrown out toward the start.
pub struct Tube { lap: (Vec3, Vec3), z: (Vec3, Vec3), end: (Vec3, Vec3), top_y: f32, out: [(Vec3, f32); 3] }
impl Tube {
    /// From the level's instances (game coordinates); None on courses without the tube.
    pub fn find(level: &crate::level::Level) -> Option<Self> {
        let at = |name: &str| level.instances.iter().find(|i| i.instance_name.starts_with(name)).map(|i| Vec3::from(i.location));
        let (lap, z, end) = (at("Mdl_Endboost_Lap")?, at("Mdl_Endboost_Z")?, at("Mdl_Endboost_End")?);
        // game (x, y, z) cm -> Bevy (x, z, -y) m; boxes as (centre, half size) from the meshes
        let b = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * 0.01;
        let half = |x: f32, y: f32, z: f32| Vec3::new(x, z, y) * 0.01;
        Some(Self {
            lap: (b(lap), half(945.0, 878.0, 1540.0)),
            z: (b(z), half(1332.0, 1365.0, 1628.0)),
            end: (b(end), half(1400.0, 1400.0, 1637.0)),
            top_y: 0.0,
            // the way out of the top for each way in (riding in along the floor, or from either side)
            out: [(b(Vec3::new(0.15899076, 0.9539445, 0.2543852)).normalize(), 27.0),
                  (b(Vec3::new(0.22610782, 0.9044313, 0.3617725)).normalize(), 35.0),
                  (b(Vec3::new(0.09245004, 0.92450035, 0.36980015)).normalize(), 35.0)],
        })
    }
    fn inside(p: Vec3, (c, h): (Vec3, Vec3)) -> bool { let d = (p - c).abs(); d.x <= h.x && d.y <= h.y && d.z <= h.z }
    /// Returns true when the rider was put up to the top this step.
    pub fn apply(&self, r: &mut Rider, laps_left: u32, dt: f32) -> bool {
        let p = r.pos + Vec3::Y * 0.5;
        // riding up to the tube with laps to go: its floor (bounce, unskiable) no longer puts the
        // rider down, the lift takes over
        let (c, h) = self.lap;
        let near = laps_left > 0 && Vec3::new(p.x - c.x, 0.0, p.z - c.z).length() < 16.0 && (p.y - c.y).abs() < h.y + 2.0;
        if near && r.tube_class == u8::MAX { r.tube_class = 0; }
        if laps_left > 0 && (Self::inside(p, self.lap) || near) {
            if r.tube_class == u8::MAX {
                // low in the box: rode in along the floor; higher: came in over one side or the other
                let floor = self.lap.0.y - self.lap.1.y;
                r.tube_class = if p.y - floor <= 10.0 { 0 } else if p.x > self.lap.0.x { 2 } else { 1 };
            }
            // draw in to the middle, kill the sideways speed, blow upward toward 25 m/s
            let to = Vec3::new(self.lap.0.x - p.x, 0.0, self.lap.0.z - p.z);
            // (the lap volume is not solid, PlayerBounce off: a rider held off by the platforms
            // either side of the mouth is still touching it)
            let touching = Self::inside(p, (self.lap.0, self.lap.1 + Vec3::splat(3.0)));
            if to.length() <= 10.0 || touching {
                r.vel.x *= 0.92f32.powf(dt * 60.0);
                r.vel.z *= 0.92f32.powf(dt * 60.0);
                r.pos += to.normalize_or_zero() * (0.2 * 60.0 * dt).min(to.length());
                let up = 25.0 - r.vel.y;
                if up > 0.0 { r.vel.y += up * 5.0 * dt; }
                if r.grounded { r.grounded = false; r.air_time = 0.2; }
            }
        }
        if Self::inside(p, self.z) && r.pos.y < self.top_y {
            r.pos.y = self.top_y;
            return true;
        }
        if Self::inside(p, self.end) {
            let (d, speed) = self.out[(r.tube_class as usize).min(2)];
            let need = speed - r.vel.dot(d);
            if need > 0.0 { r.vel += d * need * 2.0 * dt; }
        } else if r.tube_class != u8::MAX && r.pos.y > self.end.0.y + self.end.1.y + 5.0 || r.grounded && r.pos.y > self.top_y - 1.0 {
            r.tube_class = u8::MAX;
        }
        false
    }
}

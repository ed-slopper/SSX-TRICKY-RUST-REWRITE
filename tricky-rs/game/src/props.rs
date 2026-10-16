//! Things on the course that get knocked away when a rider hits them (path markers, crash
//! bags, small billboards). Each is a ball as far as physics goes: it flies, bounces and rolls
//! to a stop on the snow.
//!
//! STANDIN: a ball's physics, for the game's own object collision and bounce (port-notes/world.md)

use crate::collide::CollisionWorld;
use crate::rider::{Rider, BODY_RADIUS};
use glam::{Quat, Vec3};

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
    pub debris: Option<tricky_data::logic::DebrisDef>,
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
    /// knocked or broken by a rider this frame, at this speed (m/s): for its collision sound
    pub knock: Option<f32>,
}

/// One piece's random numbers when a `cMeshAnimNode` starts (`FUN_00146f40` 0x146f40, all from `Rand_Fx`): the tumble's
/// rate (2..10 rad/s) and axis (three draws of 2u − 1, normalised), then the spread: across and along (a draw u and a
/// sign draw each: ±u) and up (u).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draws { pub spin: f32, pub axis: Vec3, pub across: f32, pub along: f32, pub up: f32 }

/// The draws for `n` pieces in the game's order: the first loop over all the pieces (rate, axis x, y, z), then the
/// second loop (across, its sign, along, its sign, up): 9 draws a piece.
pub fn debris_draws(n: usize) -> Vec<Draws> {
    use crate::rand::Rand;
    let mut s = crate::rand::streams();
    let mut first = Vec::with_capacity(n);
    for _ in 0..n {
        let spin = Rand::unit(s.fx()) * 8.0 + 2.0;
        let (x, y, z) = (Rand::unit(s.fx()), Rand::unit(s.fx()), Rand::unit(s.fx()));
        first.push((spin, Vec3::new(x + x - 1.0, y + y - 1.0, z + z - 1.0)));
    }
    first.into_iter().map(|(spin, axis)| {
        let a = Rand::unit(s.fx());
        let a = if s.fx() & 1 != 0 { -a } else { a };
        let b = Rand::unit(s.fx());
        let b = if s.fx() & 1 != 0 { -b } else { b };
        let up = Rand::unit(s.fx());
        Draws { spin, axis, across: a, along: b, up }
    }).collect()
}

/// Pieces set loose together (one owner's, normally): each with the rider's velocity it was broken at.
pub fn throw_all(props: &mut [Prop], thrown: &[(usize, Vec3)]) {
    let draws = debris_draws(thrown.len());
    for (&(i, vel), d) in thrown.iter().zip(draws) {
        if let Some(p) = props.get_mut(i) { p.throw(vel, d); }
    }
}

impl Prop {
    pub fn new(pos: Vec3, rot: Quat, centre: Vec3, radius: f32) -> Self {
        Self { kind: Kind::Knock, inst: usize::MAX, debris: None, stays: false, scripted: false, gravity: 5.88, hidden: false, lo: centre - Vec3::splat(radius), hi: centre + Vec3::splat(radius),
               home: (pos, rot), pos, rot, vel: Vec3::ZERO, spin: Vec3::ZERO, centre, radius: radius.clamp(0.25, 1.6), moving: false,
               ballistic: false, t: 0.0, life: 2.0, mass: 5.0, knock: None }
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
    /// A broken piece set loose by a rider going through its owner. `d` is the piece's share of the node's draws
    /// ([`debris_draws`]).
    fn throw(&mut self, rider_vel: Vec3, d: Draws) {
        if self.kind != Kind::Piece { return; }
        self.hidden = false;
        match self.debris {
            Some(deb) => {
                // cMeshAnimNode: the script's velocity (or the rider's) times its multiplier, a random
                // spread (either way across, up only), its own gravity and life
                let base = if deb.base.iter().all(|v| *v == 0.0) { rider_vel } else { Vec3::new(deb.base[0], deb.base[2], -deb.base[1]) * 0.01 };
                self.launch_ballistic(base * deb.mult, d);
                self.vel = base * deb.mult + Vec3::new(d.across * deb.spread[0], d.up * deb.spread[2], -d.along * deb.spread[1]) * 0.01;
                self.gravity = deb.gravity * 9.8;
                if deb.life > 0.0 { self.life = deb.life; }
            }
            None => self.launch_ballistic(rider_vel * 0.5, d),
        }
    }
    /// A path marker's pieces fly off: the same start as a cMeshAnimNode's, for one piece.
    fn fly_alone(&mut self, base: Vec3) {
        let d = debris_draws(1)[0];
        self.launch_ballistic(base, d);
    }
    fn launch_ballistic(&mut self, base: Vec3, d: Draws) {
        self.vel = base + Vec3::new(d.across * 2.0, d.up * 4.0, d.along * 2.0);
        self.spin = d.axis.normalize_or(Vec3::X) * d.spin;
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
            self.fly_alone(dir * 0.5 * s);
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

/// Two riders running into each other (`Boarder_RiderCollisions` 0x123fc8): pushed apart by 0.55 of the overlap,
/// then (once every three ticks) the closing speed is split by their speed stats (`hit::pair_speeds`) and each
/// takes it as a hit (`Boarder_RiderHit`, exact). If one goes down the other scores a knock-down
/// (`Boarder_ScoreKnockdown`: a full meter) and the fallen one a crash (`Boarder_ScoreCrash`); if both, each a
/// crash and its tenth of the meter. A rider finished and stopped is not moved and weighs 1e30.
/// STANDIN: one sphere per rider (the game's collision body spheres, `CollBody_OverlapSpheres`, are not ported)
/// and a wait per rider instead of the game's per pair.
pub fn bump(a: &mut Rider, b: &mut Rider) {
    let d = b.pos - a.pos;
    if d.y.abs() > 1.6 { return; }
    let flat = Vec3::new(d.x, 0.0, d.z);
    let dist = flat.length();
    let reach = 0.95;
    if dist >= reach || dist < 1e-4 { return; }
    let n = flat / dist;
    let push = (reach - dist) * 0.55;
    let (sa, sb) = (a.finished_and_stopped(), b.finished_and_stopped());
    if !sa { a.pos -= n * push * if sb { 1.0 } else { 0.5 }; }
    if !sb { b.pos += n * push * if sa { 1.0 } else { 0.5 }; }
    if a.hit_wait > 0 || b.hit_wait > 0 { return; }
    a.hit_wait = 3;
    b.hit_wait = 3;
    let g = |v: Vec3| [v.x * 100.0, v.z * 100.0, v.y * 100.0, 0.0];
    let ma = if sa { 1e30 } else { a.speed_stat() };
    let mb = if sb { 1e30 } else { b.speed_stat() };
    let (ha, hb) = crate::hit::pair_speeds([n.x, n.z, n.y, 0.0], g(a.vel), g(b.vel), ma, mb);
    let a_was = a.crashed > 0.0;
    let b_was = b.crashed > 0.0;
    let a_down = a.hit(ha, n, false);
    let b_down = b.hit(hb, n, false);
    match (a_down, b_down) {
        (false, true) if !a_was => { a.knocked_someone(); let (_, c) = b.sc.crash(false); b.score_calls(&c); }
        (true, false) if !b_was => { b.knocked_someone(); let (_, c) = a.sc.crash(false); a.score_calls(&c); }
        (true, true) => {
            for r in [&mut *b, &mut *a] {
                let (m, c) = r.sc.crash(false);
                r.score_calls(&c);
                r.add_meter(m);
            }
        }
        _ => {}
    }
}

/// A shove (clip 0x15, `Boarder_RiderCollisions` 0x123fc8, exact through `hit::shove`): if `to` is in reach of
/// `from`'s shove (150 cm along its up, 180 cm across, 55° of the side it shoves to, `from.shove_side`), the hit it
/// takes, as a change of velocity (m/s). STANDIN: the rider's frame (rider+0x4aa0..0x4ad0) as our heading, right,
/// world up and position; the shove's heading (rider+0x1e0) 0 to the right, π to the left; never knockdown practice.
pub fn shove_push(from: &Rider, to: &Rider) -> Option<Vec3> {
    let g = |v: Vec3, k: f32, w: f32| [v.x * k, v.z * k, v.y * k, w];
    let side = Vec3::new(from.yaw.cos(), 0.0, -from.yaw.sin());
    let me = crate::hit::ShoveRider {
        pos: g(from.pos, 100.0, 1.0),
        vel: g(from.vel, 100.0, 0.0),
        fwd: g(crate::rider::heading(from.yaw), 1.0, 0.0),
        side: g(side, 1.0, 0.0),
        up: [0.0, 0.0, 1.0, 0.0],
        at: g(from.pos, 100.0, 1.0),
        heading: if from.shove_side >= 0.0 { 0.0 } else { std::f32::consts::PI },
        speed_stat: from.speed_stat(),
        stability: from.stats.bytes[crate::stats::STABILITY],
    };
    let other = crate::hit::ShoveRider { pos: g(to.pos, 100.0, 1.0), vel: g(to.vel, 100.0, 0.0), at: g(to.pos, 100.0, 1.0), speed_stat: to.speed_stat(), ..me };
    let (speed, n) = crate::hit::shove(&me, &other, false)?;
    Some(Vec3::new(n[0], n[2], n[1]) * (speed / 100.0))
}

/// Tokyo Megaplex's lap tube (level effects cLapBoostNode, cZBoostNode, cTubeEndBoostNode): at the
/// bottom of the course a rider with laps to go is drawn into the glass tube and blown upward at
/// 25 m/s, put straight up to the top of the course (z = 0) and thrown out toward the start.
pub struct Tube { lap: (Vec3, Vec3), z: (Vec3, Vec3), end: (Vec3, Vec3), top_y: f32, out: [(Vec3, f32); 3] }
impl Tube {
    /// From the level's instances (game coordinates); None on courses without the tube.
    pub fn find(level: &tricky_data::level::Level) -> Option<Self> {
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

#[cfg(test)]
mod draw_tests {
    //! Row E1b2b: a broken piece's tumble and spread are `Rand_Fx` draws in the order of `FUN_00146f40` 0x146f40.
    use super::*;
    use crate::rand::{streams, Rand};
    use std::sync::Mutex;

    /// the test below shares the global effects stream
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    #[test]
    fn pieces_draw_their_tumble_for_all_then_their_spread_for_all() {
        let _g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let seed = 0x5eed_0b2b;
        let mut want = Rand::seeded(seed);
        let mut d = || want.draw();
        // two pieces: rate, axis x, y, z for the first, then the same for the second; then across, its sign, along, its sign,
        // up for the first, and the same for the second
        let first: Vec<u32> = (0..8).map(|_| d()).collect();
        let second: Vec<u32> = (0..10).map(|_| d()).collect();
        streams().seed_fx(seed);
        let got = debris_draws(2);
        let u = Rand::unit;
        assert_eq!(got[0].spin, u(first[0]) * 8.0 + 2.0);
        assert_eq!(got[1].spin, u(first[4]) * 8.0 + 2.0);
        assert_eq!(got[0].axis, Vec3::new(u(first[1]) * 2.0 - 1.0, u(first[2]) * 2.0 - 1.0, u(first[3]) * 2.0 - 1.0));
        let sign = |x: u32| if x & 1 != 0 { -1.0 } else { 1.0 };
        assert_eq!(got[0].across, u(second[0]) * sign(second[1]));
        assert_eq!(got[0].along, u(second[2]) * sign(second[3]));
        assert_eq!(got[0].up, u(second[4]));
        assert_eq!(got[1].across, u(second[5]) * sign(second[6]));
        assert_eq!(got[1].up, u(second[9]));
        // nine draws a piece and none more
        assert_eq!(streams().fx(), d());
    }
}

//! Snowboarder physics. Plain functions over plain data so the same code runs in the game and
//! in the headless self-test. Bevy space: metres, seconds, Y up.
//!
//! The speeds, gravity, drag, turning, jump and rotation numbers are the original game's.

use crate::collide::CollisionWorld;
use crate::rails::Rails;
use bevy::math::Vec3;

// The numbers below are the original game's, read out of the decompiled SLUS_203.26 (converted from
// its cm and 1/60 s frames to metres and seconds), for a rider with middling stats on ordinary snow.
/// gravity on the snow (surface table row 1), and in the air: gentle on the way up, hard on the way down
pub const GRAVITY: f32 = 13.0;
pub const GRAVITY_RISING: f32 = 8.5;
pub const GRAVITY_FALLING: f32 = 19.0;
/// the air only slows the horizontal part of the velocity (1/s)
pub const AIR_DRAG: f32 = 0.2;
/// drag on the snow as a rate (1/s): DRAG_LIN + DRAG_STAND when not crouched + DRAG_V * v + DRAG_V2 * v^2
pub const DRAG_LIN: f32 = 0.0019;
pub const DRAG_STAND: f32 = 0.1057;
pub const DRAG_V: f32 = 0.000196;
pub const DRAG_V2: f32 = 0.0000856;
/// braking adds this much drag (1/s)
pub const BRAKE: f32 = 1.88;
/// hard speed limits (m/s): 100.4 km/h on the snow, up to 120.5 km/h boosting or in the air
pub const SPEED_CAP: f32 = 27.89;
pub const BOOST_CAPS: [f32; 3] = [29.32, 30.72, 33.47];
pub const AIR_CAP: f32 = 33.47;
/// once the boost stops the limit comes back down this fast (m/s^2)
pub const CAP_DECAY: f32 = 2.08;
/// the rider pushes along by himself up to 51.85 km/h
pub const PUSH_TO: f32 = 14.4;
pub const PUSH: f32 = 1.75;
pub const PUSH_MAX: f32 = 5.0;
/// sideways acceleration of a full carve: g * tan(58.3 deg * 0.905); below TURN_FULL_SPEED it scales with speed
pub const TURN_ACCEL: f32 = 17.1;
pub const TURN_FULL_SPEED: f32 = 11.3;
/// rotation in the air (rad/s): what a jump starts with, with no wind-up and with a full one (a wound-up flip gets 2/3 of the spin's)
pub const AIR_SPIN: f32 = 4.63;
pub const AIR_SPIN_WOUND: f32 = 10.16;
pub const FLIP_RATE: f32 = AIR_SPIN;
pub const FLIP_WOUND: f32 = AIR_SPIN_WOUND * 2.0 / 3.0;
/// the rotation dies away through the jump (x0.70 each second) down to this floor
pub const SPIN_DECAY: f32 = 0.70;
pub const SPIN_FLOOR: f32 = 1.396;
/// seconds for the wind-up to follow the stick while crouched for a jump
pub const WIND_TIME: f32 = 0.5;
/// jump: 6.31 m/s at least, a little more with a full crouch (which takes 0.66 s)
pub const JUMP_MIN: f32 = 6.31;
pub const JUMP_FULL: f32 = 7.2;
pub const CHARGE_TIME: f32 = 0.66;
pub const BOOST_ACCEL: f32 = 3.75;
/// seconds of boost in a full meter
pub const BOOST_SECONDS: f32 = 22.0;
/// how fast sideways slip is killed (1/s); this is what makes the board carve
pub const EDGE_GRIP: f32 = 7.0;
pub const BODY_RADIUS: f32 = 0.45;
pub const WALL_BOUNCE: f32 = 0.25;
/// how close (m) the board has to pass to a rail, while in the air, to lock on
pub const RAIL_REACH: f32 = 0.9;
pub const RAIL_FRICTION: f32 = 0.03;
/// how fast the board turns on a rail when steered (rad/s)
pub const RAIL_TWIST: f32 = 5.5;
/// trick points that fill the boost meter
pub const BOOST_POINTS: f32 = 2500.0;
pub const UBER_POINTS: u32 = 3000;
/// how long a wipe-out lasts: the bail and the get-up
pub const CRASH_SECONDS: f32 = 1.95;

#[derive(Clone, Copy, Default)]
pub struct Input {
    /// -1 = left, +1 = right
    pub steer: f32,
    pub tuck: bool,
    pub brake: bool,
    pub jump: bool,
    /// in the air: +1 = front flip, -1 = back flip
    pub flip: f32,
    /// in the air: 0 = none, 1 = front-hand grab, 2 = back-hand grab, 3 = both arms up
    pub grab: u8,
    pub boost: bool,
    /// in the air with a full meter: start the next uber trick
    pub uber: bool,
}

#[derive(Clone)]
pub struct Rider {
    pub pos: Vec3,
    pub vel: Vec3,
    /// heading around world Y; forward is (-sin yaw, 0, -cos yaw)
    pub yaw: f32,
    pub grounded: bool,
    pub normal: Vec3,
    pub charge: f32,
    pub air_time: f32,
    pub spawn: (Vec3, f32),
    pub safe: (Vec3, f32),
    safe_timer: f32,
    pub respawns: u32,
    /// grinding: (rail index, distance along it, signed speed along it)
    pub rail: Option<(usize, f32, f32)>,
    rail_cooldown: f32,
    /// what the controls were on the last step (the pose follows it)
    pub input: Input,
    // ---- tricks
    /// signed turns of spin and flip made since leaving the snow (radians)
    pub spin: f32,
    pub flip: f32,
    flip_dir: f32,
    /// wind-up before a jump (-1..1) for spin and flip, and the rotation rates the jump started with
    pub wind: f32,
    pub wind_flip: f32,
    spin_rate: f32,
    flip_rate: f32,
    /// the speed limit right now (it lags behind when a boost ends)
    cap: f32,
    steer: f32,
    pub grab: u8,
    grab_time: [f32; 4],
    grind_time: f32,
    /// on a rail: the board's angle to the direction of travel, how far it has been turned in all,
    /// and how long it has been ridden sideways
    pub rail_twist: f32,
    rail_spun: f32,
    rail_side: f32,
    pub score: u32,
    /// boost meter, 0..1
    pub boost: f32,
    pub boosting: bool,
    /// name and points of the last trick, and seconds left to show it
    pub last_trick: String,
    pub trick_timer: f32,
    /// seconds left recovering from a bad landing
    pub crashed: f32,
    // ---- uber tricks
    /// this rider's signature moves: (clip name, length in frames at 30 a second)
    pub ubers: Vec<(String, f32)>,
    /// frames into the current uber trick (0 = not doing one) and which one
    pub uber: f32,
    pub uber_id: usize,
    uber_done: bool,
    uber_held: bool,
    /// uber tricks landed this run; six spell TRICKY and make boost unlimited
    pub letters: u8,
    /// from a multiplier pickup: applied to the next trick landed
    pub multiplier: u32,
}

pub fn heading(yaw: f32) -> Vec3 { Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) }

impl Rider {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self { pos, vel: Vec3::ZERO, yaw, grounded: false, normal: Vec3::Y, charge: 0.0, air_time: 0.0,
               spawn: (pos, yaw), safe: (pos, yaw), safe_timer: 0.0, respawns: 0, rail: None, rail_cooldown: 0.0, input: Input::default(),
               spin: 0.0, flip: 0.0, flip_dir: 0.0, wind: 0.0, wind_flip: 0.0, spin_rate: AIR_SPIN, flip_rate: FLIP_RATE, cap: SPEED_CAP, steer: 0.0, grab: 0, grab_time: [0.0; 4], grind_time: 0.0, rail_twist: 0.0, rail_spun: 0.0, rail_side: 0.0, score: 0, boost: 0.0, boosting: false,
               last_trick: String::new(), trick_timer: 0.0, crashed: 0.0,
               ubers: Vec::new(), uber: 0.0, uber_id: 0, uber_done: false, uber_held: false, letters: 0, multiplier: 1 }
    }
    pub fn respawn(&mut self, at: (Vec3, f32)) {
        self.pos = at.0 + Vec3::Y * 0.5;
        self.yaw = at.1;
        self.vel = Vec3::ZERO;
        self.grounded = false;
        self.charge = 0.0;
        self.air_time = 0.0;
        self.rail = None;
        self.spin = 0.0;
        self.flip = 0.0;
        self.flip_dir = 0.0;
        self.wind = 0.0;
        self.wind_flip = 0.0;
        self.cap = SPEED_CAP;
        self.grab = 0;
        self.grab_time = [0.0; 4];
        self.grind_time = 0.0;
        self.uber = 0.0;
        self.uber_done = false;
        self.respawns += 1;
    }
    /// Called when the board touches down (snow or rail): score what was done in the air.
    fn land(&mut self) {
        let half_turns = (self.spin.abs() / std::f32::consts::PI).round() as u32;
        let flips = (self.flip.abs() / std::f32::consts::TAU).round() as u32;
        let tau = std::f32::consts::TAU;
        let off = (self.flip.rem_euclid(tau)).min(tau - self.flip.rem_euclid(tau));
        let grabbed: f32 = self.grab_time.iter().sum();
        let uber_cut_short = self.uber > 0.0 && !self.uber_done;
        let tried = flips > 0 || off > 1.4 || grabbed > 0.0 || half_turns > 0 || self.uber_done;
        if off > 1.4 || (self.grab != 0 && grabbed > 0.25) || uber_cut_short {
            // upside down, or still holding the board: wipe out
            self.last_trick = "CRASH".into();
            self.trick_timer = 2.0;
            self.vel *= 0.35;
            self.crashed = CRASH_SECONDS;
        } else if tried {
            let mut names: Vec<String> = Vec::new();
            let mut pts = 0.0;
            if half_turns > 0 { names.push(format!("{} Spin", half_turns * 180)); pts += half_turns as f32 * 180.0; }
            if flips > 0 { names.push(format!("{}{}flip", if flips > 1 { format!("{flips}x ") } else { String::new() }, if self.flip > 0.0 { "Front" } else { "Back" })); pts += flips as f32 * 550.0; }
            for (g, name) in [(1, "Nose Grab"), (2, "Tail Grab"), (3, "Method")] {
                if self.grab_time[g] > 0.12 { names.push(name.into()); pts += 120.0 + self.grab_time[g].min(2.5) * 420.0; }
            }
            if self.uber_done {
                let name = self.ubers.get(self.uber_id).map(|u| u.0.clone()).unwrap_or_default();
                let short = name.split("UT_").last().unwrap_or("").to_string();
                names.push(format!("UBER {short}"));
                pts += UBER_POINTS as f32;
                self.letters = (self.letters + 1).min(6);
            }
            if !names.is_empty() {
                // doing several things in one jump is worth more than the sum
                let pts = (pts * (1.0 + 0.25 * (names.len() as f32 - 1.0))).round() as u32 * self.multiplier;
                if self.multiplier > 1 { names.push(format!("x{}", self.multiplier)); self.multiplier = 1; }
                self.score += pts;
                self.boost = (self.boost + pts as f32 / BOOST_POINTS).min(1.0);
                self.last_trick = format!("{}  +{pts}", names.join(" + "));
                self.trick_timer = 3.0;
            }
        }
        self.spin = 0.0;
        self.flip = 0.0;
        self.flip_dir = 0.0;
        self.grab = 0;
        self.grab_time = [0.0; 4];
        self.uber = 0.0;
        self.uber_done = false;
    }
    pub fn pick_multiplier(&mut self, m: u32) {
        self.multiplier = self.multiplier.max(m);
        self.last_trick = format!("x{m} on your next trick");
        self.trick_timer = 2.0;
    }
    pub fn pick_boost(&mut self, amount: f32, kick: bool) {
        self.boost = (self.boost + amount).min(1.0);
        if kick { let v = self.vel.normalize_or(heading(self.yaw)); self.vel += v * 6.0; }
        self.last_trick = if kick { "Speed boost".into() } else { "Boost meter filled".into() };
        self.trick_timer = 2.0;
    }
    /// Six landed uber tricks spell TRICKY: boost no longer runs out.
    pub fn tricky(&self) -> bool { self.letters >= 6 }
    fn end_grind(&mut self) {
        if self.grind_time > 0.4 {
            let turns = (self.rail_spun / std::f32::consts::PI).round() as u32;
            let slide = self.rail_side > self.grind_time * 0.5;
            let pts = (self.grind_time * if slide { 340.0 } else { 260.0 }).round() as u32 + turns * 180;
            self.score += pts;
            self.boost = (self.boost + pts as f32 / BOOST_POINTS).min(1.0);
            let spun = if turns > 0 { format!(" + {} on the rail", turns * 180) } else { String::new() };
            self.last_trick = format!("{:.1} s {}{spun}  +{pts}", self.grind_time, if slide { "Boardslide" } else { "50-50 Grind" });
            self.trick_timer = 3.0;
        }
        self.grind_time = 0.0;
        self.rail_spun = 0.0;
        self.rail_side = 0.0;
    }

    fn jump_speed(&self) -> f32 { JUMP_MIN.max(self.charge * self.charge * JUMP_FULL) }
    /// Leaving the snow: the wind-up decides how fast this jump can spin and flip.
    fn take_off(&mut self) {
        self.spin_rate = AIR_SPIN.max(AIR_SPIN_WOUND * self.wind.abs());
        self.flip_rate = FLIP_RATE.max(FLIP_WOUND * self.wind_flip.abs());
        self.wind = 0.0;
        self.wind_flip = 0.0;
        self.charge = 0.0;
        self.grounded = false;
        self.air_time = 0.0;
    }
    /// Spin or flip rate this far into the jump.
    fn air_rate(&self, start: f32) -> f32 { (start * SPIN_DECAY.powf(self.air_time)).max(SPIN_FLOOR) }
    fn crouch(&mut self, input: &Input, dt: f32) {
        if input.jump {
            self.charge = (self.charge + dt / CHARGE_TIME).min(1.0);
            let k = (dt / WIND_TIME).min(1.0);
            self.wind += (input.steer - self.wind) * k;
            self.wind_flip += (input.flip - self.wind_flip) * k;
        }
    }

    /// How long the current grab has been held (seconds).
    pub fn grab_held(&self) -> f32 { self.grab_time[(self.grab as usize).min(3)] }

    /// Board direction along the current surface.
    pub fn forward(&self) -> Vec3 {
        let h = heading(self.yaw);
        (h - self.normal * h.dot(self.normal)).normalize_or(h)
    }

    pub fn step(&mut self, world: &CollisionWorld, rails: &Rails, input: Input, dt: f32) {
        let g = Vec3::NEG_Y * GRAVITY;
        let mut input = input;
        self.trick_timer = (self.trick_timer - dt).max(0.0);
        self.crashed = (self.crashed - dt).max(0.0);
        if self.crashed > 0.0 { input = Input { steer: input.steer * 0.3, ..Input::default() }; }
        self.input = input;
        self.boosting = false;
        self.rail_cooldown = (self.rail_cooldown - dt).max(0.0);

        // ---- grinding a rail: slide along it under gravity until it ends or we hop off
        if let Some((ri, mut d, mut s)) = self.rail {
            let rail = &rails.0[ri];
            let (_, tan) = rail.at(d);
            s += g.dot(tan) * dt;
            s /= 1.0 + (DRAG_V * s.abs() + RAIL_FRICTION) * dt;
            s = s.clamp(-AIR_CAP, AIR_CAP);
            d += s * dt;
            let (p, tan) = rail.at(d);
            let travel = tan * s.signum();
            self.pos = p;
            self.vel = tan * s;
            // steering turns the board on the rail; left alone it settles square or sideways
            let turn = input.steer * RAIL_TWIST * dt;
            self.rail_twist += turn;
            self.rail_spun += turn.abs();
            if input.steer == 0.0 {
                let q = std::f32::consts::FRAC_PI_2;
                let rest = (self.rail_twist / q).round() * q;
                self.rail_twist += (rest - self.rail_twist) * (1.0 - (-6.0 * dt).exp());
            }
            if self.rail_twist.sin().abs() > 0.7 { self.rail_side += dt; }
            self.yaw = f32::atan2(-travel.x, -travel.z) - self.rail_twist;
            self.normal = (Vec3::Y - travel * travel.y).normalize_or(Vec3::Y);
            self.grounded = false;
            self.air_time = 0.0;
            self.grind_time += dt;
            let hop = !input.jump && self.charge > 0.0;
            self.crouch(&input, dt);
            if hop || d <= 0.0 || d >= rail.len() || s.abs() < 0.7 {
                if hop {
                    // hop off, sideways if steering
                    let side = Vec3::new(-travel.z, 0.0, travel.x);
                    self.vel += Vec3::Y * self.jump_speed() + side * input.steer * 2.5;
                }
                self.take_off();
                self.rail = None;
                self.rail_cooldown = 0.45;
                self.end_grind();
            } else {
                self.rail = Some((ri, d, s));
            }
            return;
        }

        let speed = self.vel.length();
        let mut jumped = false;

        // the stick does not act at once
        self.steer += (input.steer - self.steer) * (dt / 0.12).min(1.0);
        let turn_rate = TURN_ACCEL / speed.max(TURN_FULL_SPEED) * if input.brake { 1.3 } else { 1.0 };

        if self.grounded {
            let n = self.normal;
            self.yaw -= self.steer * turn_rate * dt;

            // gravity along the slope
            self.vel += (g - n * g.dot(n)) * dt;

            // split into along-the-board and sideways; the edge kills the sideways part and turns it
            // into travel along the board (a carve costs no speed in the original)
            let f = self.forward();
            let l = n.cross(f).normalize_or(Vec3::X);
            let (mut vf, mut vl) = (self.vel.dot(f), self.vel.dot(l));
            let before = (vf * vf + vl * vl).sqrt();
            vl *= (-EDGE_GRIP * if input.brake { 0.5 } else { 1.0 } * dt).exp();
            if !input.brake {
                let after = (vf * vf + vl * vl).sqrt();
                if after > 0.5 { vf *= before / after; vl *= before / after; }
            }
            let crouched = input.tuck || input.jump;
            // the rider pushes himself along when slow
            if !input.brake && vf.abs() < PUSH_TO { vf += (PUSH * (PUSH_TO - vf.abs())).min(PUSH_MAX) * dt; }
            let mut cap = SPEED_CAP;
            if input.boost && self.boost > 0.0 {
                let tier = if self.boost > 0.666 || self.tricky() { 2 } else if self.boost > 0.334 { 1 } else { 0 };
                let level = [0.25, 0.6, 1.0][tier];
                vf += BOOST_ACCEL * level * (1.0 - self.steer.abs()).max(0.0) * dt;
                cap = BOOST_CAPS[tier];
                if !self.tricky() { self.boost = (self.boost - dt / BOOST_SECONDS).max(0.0); }
                self.boosting = true;
            }
            self.vel = f * vf + l * vl;

            let s = self.vel.length();
            let mut k = DRAG_LIN + DRAG_V * s + DRAG_V2 * s * s;
            if !crouched { k += DRAG_STAND; }
            if input.brake { k += BRAKE; }
            self.vel /= 1.0 + k * dt;

            // the speed limit: up at once with a boost, back down slowly afterwards
            self.cap = cap.max(self.cap - CAP_DECAY * dt);
            let s = self.vel.length();
            if s > self.cap { self.vel *= self.cap / s; }

            if input.jump {
                self.crouch(&input, dt);
            } else if self.charge > 0.0 {
                self.vel += (n + f * 0.2).normalize() * self.jump_speed();
                self.take_off();
                jumped = true;
            } else {
                self.wind = 0.0;
                self.wind_flip = 0.0;
            }
        } else {
            self.vel.y -= if self.vel.y > 0.0 { GRAVITY_RISING } else { GRAVITY_FALLING } * dt;
            let h = 1.0 / (1.0 + AIR_DRAG * dt);
            self.vel.x *= h;
            self.vel.z *= h;
            let s = self.vel.length();
            if s > AIR_CAP { self.vel *= AIR_CAP / s; }
            // the first instant off the snow (a bump, a facet edge) still steers like the ground;
            // only real air time spins
            let spin = if self.air_time < 0.15 { turn_rate } else { self.air_rate(self.spin_rate) };
            let steer = if self.air_time < 0.15 { self.steer } else { input.steer };
            self.yaw -= steer * spin * dt;
            self.air_time += dt;
            if self.air_time >= 0.15 {
                self.spin += input.steer * spin * dt;
                let rate = self.air_rate(self.flip_rate);
                if input.flip != 0.0 {
                    self.flip += input.flip * rate * dt;
                    self.flip_dir = input.flip.signum();
                } else if self.flip_dir != 0.0 {
                    // let go and the flip carries on round to upright by itself
                    let tau = std::f32::consts::TAU;
                    // (a flip that is already within a twentieth of a turn of upright just settles there)
                    let goal = if self.flip_dir > 0.0 { (self.flip / tau - 0.05).ceil() * tau } else { (self.flip / tau + 0.05).floor() * tau };
                    let step = rate * dt;
                    if (goal - self.flip) * self.flip_dir <= step { self.flip = goal; self.flip_dir = 0.0; } else { self.flip += self.flip_dir * step; }
                }
                self.grab = input.grab.min(3);
                if self.grab != 0 { self.grab_time[self.grab as usize] += dt; }
                // uber tricks: need a full meter (or TRICKY), one press each, and enough air to finish
                if input.uber && !self.uber_held && self.uber == 0.0 && !self.ubers.is_empty() && (self.boost >= 0.99 || self.tricky()) {
                    self.uber = 0.01;
                    self.uber_done = false;
                    self.uber_id = (self.uber_id + 1) % self.ubers.len();
                }
                if self.uber > 0.0 && !self.uber_done {
                    self.uber += dt * 30.0;
                    self.grab = 0;
                    if self.uber >= self.ubers[self.uber_id].1 - 1.0 { self.uber_done = true; }
                }
                self.uber_held = input.uber;
            }
            if input.jump { self.charge = (self.charge + dt / CHARGE_TIME).min(1.0); }
        }

        let mut p = self.pos + self.vel * dt;

        // walls: a sphere around the body
        let lift = Vec3::Y * (BODY_RADIUS + 0.35);
        let (c, contacts) = world.push_out(p + lift, BODY_RADIUS);
        p = c - lift;
        for n in contacts {
            let vn = self.vel.dot(n);
            if vn < 0.0 { self.vel -= n * vn * (1.0 + WALL_BOUNCE); }
        }

        let s = self.vel.length();
        if self.grounded {
            // stay on the snow unless it falls away from under us (a lip, a cliff)
            let snap = 0.25 + s * dt * 0.4;
            match world.ground(p, 1.2, snap) {
                Some(hit) if !(s > 6.0 && self.vel.dot(hit.normal) / s > 0.17) => {
                    p.y = hit.y;
                    let vn = self.vel.dot(hit.normal);
                    self.vel -= hit.normal * vn;
                    // riding through a dip bends the direction of travel; it does not cost speed
                    let after = self.vel.length();
                    if vn < 0.0 && after > 0.5 { self.vel *= s / after; }
                    let blend = 1.0 - (-18.0 * dt).exp();
                    self.normal = self.normal.lerp(hit.normal, blend).normalize_or(hit.normal);
                }
                _ => { let c = self.charge; self.take_off(); self.charge = c; }
            }
        } else if !jumped {
            // landing: is there snow at or above our feet, and are we moving into it?
            if let Some(hit) = world.ground(p, 1.2, 0.02) {
                let vn = self.vel.dot(hit.normal);
                if vn <= 0.5 {
                    p.y = hit.y;
                    self.vel -= hit.normal * vn.min(0.0);
                    // the original turns the whole fall into speed along the slope (up to the limit)
                    let after = self.vel.length();
                    if after > 0.5 { self.vel *= s.min(self.cap.max(SPEED_CAP)) / after; }
                    self.normal = hit.normal;
                    self.grounded = true;
                    self.land();
                }
            }
        }
        self.pos = p;

        // dropping onto a rail from the air locks the board onto it
        if !self.grounded && !jumped && self.rail_cooldown <= 0.0 && self.air_time > 0.08 && self.vel.y < 2.5 {
            if let Some((ri, d, _)) = rails.nearest(self.pos, RAIL_REACH) {
                let (rp, tan) = rails.0[ri].at(d);
                let along = self.vel.dot(tan);
                if along.abs() > 3.0 && along.abs() > 0.45 * s {
                    self.pos = rp;
                    // the board keeps the angle it came in at
                    let travel = tan * along.signum();
                    let mut tw = f32::atan2(-travel.x, -travel.z) - self.yaw;
                    while tw > std::f32::consts::PI { tw -= std::f32::consts::TAU; }
                    while tw < -std::f32::consts::PI { tw += std::f32::consts::TAU; }
                    self.rail = Some((ri, d, along));
                    self.charge = 0.0;
                    self.land();
                    self.crashed = 0.0;
                    self.rail_twist = tw;
                    self.rail_spun = 0.0;
                    self.rail_side = 0.0;
                }
            }
        }

        // remember somewhere sane to come back to, and come back if we fall out of the world
        if self.grounded && s > 3.0 {
            self.safe_timer += dt;
            if self.safe_timer > 1.5 { self.safe = (self.pos, self.yaw); self.safe_timer = 0.0; }
        }
        if self.pos.y < world.min_y - 30.0 || (!self.grounded && self.air_time > 9.0) {
            let at = self.safe;
            self.respawn(at);
        }
    }
}

/// Follows the course's racing line. Used by the self-test and by the opponents.
pub struct AiDriver {
    pub idx: usize,
    last_idx: usize,
    stuck_for: f32,
    /// how far to the side of the line this rider likes to be (m)
    pub offset: f32,
    /// 0..1: how much of the time it tucks
    pub skill: f32,
    pub finished: Option<f32>,
    pub stuck: u32,
    clock: f32,
    /// the trick picked for the current jump: spin direction and grab
    trick: (f32, u8),
    jumps: u32,
}
impl AiDriver {
    pub fn new(offset: f32, skill: f32) -> Self { Self { idx: 0, last_idx: 0, stuck_for: 0.0, offset, skill, finished: None, stuck: 0, clock: 0.0, trick: (0.0, 0), jumps: 0 } }
    pub fn progress(&self, line: &[Vec3]) -> f32 { if line.len() < 2 { 0.0 } else { self.idx as f32 / (line.len() - 1) as f32 } }
    /// Decide the controls for this step. Puts the rider back on the line if it wedges itself.
    /// `clear` is how far the rider is above the snow.
    pub fn drive(&mut self, r: &mut Rider, line: &[Vec3], clear: f32, dt: f32) -> Input {
        if line.len() < 4 { return Input::default(); }
        self.clock += dt;
        if self.finished.is_some() { return Input { brake: true, ..Input::default() }; }
        let mut best = (self.idx, f32::MAX);
        for i in self.idx..(self.idx + 12).min(line.len()) {
            let d = (line[i] - r.pos).length_squared();
            if d < best.1 { best = (i, d); }
        }
        self.idx = best.0;
        if self.idx + 2 >= line.len() { self.finished = Some(self.clock); }
        if self.idx > self.last_idx { self.last_idx = self.idx; self.stuck_for = 0.0; } else { self.stuck_for += dt; }
        if self.stuck_for > 7.0 && self.clock > 12.0 {
            self.idx = (self.idx + 2).min(line.len() - 1);
            let nxt = line[(self.idx + 1).min(line.len() - 1)] - line[self.idx];
            r.respawn((line[self.idx] + Vec3::Y * 1.0, f32::atan2(-nxt.x, -nxt.z)));
            self.stuck += 1;
            self.stuck_for = 0.0;
        }
        let a = line[(self.idx + 3).min(line.len() - 1)];
        let b = line[(self.idx + 4).min(line.len() - 1)];
        let along = (b - a).normalize_or(Vec3::NEG_Z);
        let target = a + Vec3::new(-along.z, 0.0, along.x) * self.offset;
        let to = target - r.pos;
        let want = f32::atan2(-to.x, -to.z);
        let mut diff = want - r.yaw;
        while diff > std::f32::consts::PI { diff -= std::f32::consts::TAU; }
        while diff < -std::f32::consts::PI { diff += std::f32::consts::TAU; }
        if !r.grounded {
            // in the air: with enough room, spin and grab, and let go in time to land
            if r.rail.is_some() { return Input::default(); }
            if r.air_time < 0.05 {
                self.jumps += 1;
                let n = self.jumps + (self.offset.abs() * 10.0) as u32;
                self.trick = (if n % 2 == 0 { 1.0 } else { -1.0 }, (n % 3 + 1) as u8);
            }
            let room = clear > 3.5 || (r.vel.y > 1.0 && clear > 1.5);
            let half = (r.spin.abs() / std::f32::consts::PI).fract();
            let spinning = room || (r.spin.abs() > 0.5 && half > 0.12 && half < 0.88 && clear > 1.0);
            return Input { steer: if spinning && self.skill > 0.6 { self.trick.0 } else { 0.0 }, grab: if room { self.trick.1 } else { 0 }, ..Input::default() };
        }
        // tuck in bursts, more often the better the rider
        let tuck = ((self.clock * 0.35 + self.offset).sin() * 0.5 + 0.5) < self.skill;
        let steer = (-diff * 2.5).clamp(-1.0, 1.0);
        Input { steer, tuck, boost: r.boost > 0.25 && steer.abs() < 0.3, ..Input::default() }
    }
}

/// Timing a run from the gate to the end of the course's main race line.
#[derive(Clone, Copy, PartialEq)]
pub enum RaceState { Countdown, Running, Finished }
pub struct Race { pub line: Vec<Vec3>, pub idx: usize, pub time: f32, pub state: RaceState, pub best: Option<f32>, pub countdown: f32 }
impl Race {
    pub fn new(line: Vec<Vec3>) -> Self { Self { line, idx: 0, time: 0.0, state: RaceState::Countdown, best: None, countdown: 3.4 } }
    pub fn restart(&mut self) { self.idx = 0; self.time = 0.0; self.state = RaceState::Countdown; self.countdown = 3.4; }
    pub fn progress(&self) -> f32 { if self.line.len() < 2 { 0.0 } else { self.idx as f32 / (self.line.len() - 1) as f32 } }
    pub fn update(&mut self, r: &Rider, dt: f32) {
        if self.line.len() < 4 { return; }
        match self.state {
            RaceState::Countdown => { self.countdown -= dt; if self.countdown <= 0.0 { self.state = RaceState::Running; } }
            RaceState::Running => {
                self.time += dt;
                // follow our place along the line; look a little ahead only, so a shortcut that
                // passes near a later part of the course does not count as being there
                let mut best = (self.idx, f32::MAX);
                for i in self.idx..(self.idx + 14).min(self.line.len()) {
                    let d = (self.line[i] - r.pos).length_squared();
                    if d < best.1 { best = (i, d); }
                }
                self.idx = best.0;
                let end = *self.line.last().unwrap();
                if self.idx + 2 >= self.line.len() || (self.progress() > 0.8 && (end - r.pos).length() < 35.0) {
                    self.state = RaceState::Finished;
                    self.best = Some(self.best.map_or(self.time, |b| b.min(self.time)));
                }
            }
            RaceState::Finished => {}
        }
    }
}

/// Third-person camera that trails the rider. Plain data so the self-test can measure it.
pub struct ChaseCam { pub pos: Vec3, dir: Vec3, boom: f32 }
impl Default for ChaseCam { fn default() -> Self { Self { pos: Vec3::ZERO, dir: Vec3::ZERO, boom: 99.0 } } }
impl ChaseCam {
    /// Returns (camera position, point to look at).
    pub fn update(&mut self, world: &CollisionWorld, r: &Rider, dt: f32) -> (Vec3, Vec3) {
        // look where we are going; fall back to where the board points when nearly stopped
        let flat = Vec3::new(r.vel.x, 0.0, r.vel.z);
        let want = if flat.length() > 3.0 { r.vel.normalize() } else { heading(r.yaw) };
        let k = 1.0 - (-4.0 * dt).exp();
        self.dir = if self.dir == Vec3::ZERO { want } else { self.dir.lerp(want, k).normalize_or(want) };
        let back = Vec3::new(self.dir.x, self.dir.y * 0.5, self.dir.z).normalize_or(heading(r.yaw));
        let head = r.pos + Vec3::Y * 1.5;
        let look = r.pos + Vec3::Y * 1.1 + back * 3.0;
        let mut target = r.pos - back * 6.5 + Vec3::Y * 2.6;
        // On a steep slope the snow behind the rider is higher than the rider. Keep the camera's
        // resting place above it. Searching down from just over head height means a roof far
        // overhead is not mistaken for ground.
        if let Some(h) = world.ground(Vec3::new(target.x, head.y + 3.0, target.z), 0.0, 40.0) {
            target.y = target.y.max(h.y + 1.7);
        }
        // If something solid is still in the way (a wall, a low roof), shorten the boom. Pull in
        // quickly, let out slowly, so a post flashing past does not make the view pump.
        let full = (target - head).length().max(0.1);
        let clear = world.raycast(head, target).map(|d| (d - 0.4).max(1.0)).unwrap_or(full);
        let rate = if clear < self.boom { 25.0 } else { 2.5 };
        self.boom = self.boom.min(full) + (clear - self.boom.min(full)) * (1.0 - (-rate * dt).exp());
        let target = head + (target - head) / full * self.boom.min(full);
        let snap = self.pos.distance(target) > 40.0;
        let mut pos = if snap { target } else { self.pos.lerp(target, 1.0 - (-10.0 * dt).exp()) };
        // last resort: never below the snow directly underneath
        if let Some(hit) = world.ground(pos, 0.7, 2.0) { pos.y = pos.y.max(hit.y + 0.4); }
        self.pos = pos;
        (pos, look)
    }
}

/// Headless check: ride the course on autopilot along the race line and report what happens.
pub fn self_test(world: &CollisionWorld, rails: &Rails, line: &[Vec3], start_yaw: f32, seconds: f32) {
    if std::env::var("TRICKY_TRICKTEST").is_ok() {
        let Some(&start) = line.first() else { return };
        let tau = std::f32::consts::TAU;
        // (name, seconds of flip, seconds of spin, seconds of grab, hold the grab into the landing?)
        for (name, flip_s, spin_s, grab_s, hold) in [
            ("one backflip, wound up", 1.0, 0.0, 0.0, false),
            ("360 spin + nose grab", 0.0, tau / AIR_SPIN, 0.8, false),
            ("flip started too late (lands upside down)", -1.0, 0.0, 0.0, false),
            ("grab held into the landing", 0.0, 0.0, 9.0, true),
            ("nothing", 0.0, 0.0, 0.0, false),
        ] {
            let mut r = Rider::new(start + Vec3::Y * 22.0, start_yaw);
            r.vel = heading(start_yaw) * 4.0;
            if flip_s > 0.0 { r.flip_rate = FLIP_WOUND; }
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            while t < 8.0 && !(r.grounded && t > 0.5) {
                let air = (r.air_time - 0.15).max(0.0);
                let input = Input {
                    flip: if (air < flip_s && flip_s > 0.0) || (flip_s < 0.0 && air > 0.7) { -1.0 } else { 0.0 },
                    steer: if air < spin_s && spin_s > 0.0 { 1.0 } else { 0.0 },
                    grab: if air < grab_s || hold { 1 } else { 0 },
                    ..Input::default()
                };
                r.step(world, rails, if r.grounded { Input::default() } else { input }, dt);
                t += dt;
            }
            println!("{name}: landed after {t:.2} s -> \"{}\"  score {}  boost {:.0}%{}", r.last_trick, r.score, r.boost * 100.0, if r.crashed > 0.0 { "  (wiped out)" } else { "" });
        }
        // uber tricks: a two-second signature move started with a full meter
        for (name, height, full) in [("uber with enough air", 45.0, true), ("uber without enough air", 12.0, true), ("uber without a full meter", 45.0, false)] {
            let mut r = Rider::new(start + Vec3::Y * height, start_yaw);
            r.ubers = vec![("bxUT_TEST".into(), 60.0), ("bxUT_OTHER".into(), 60.0)];
            r.boost = if full { 1.0 } else { 0.5 };
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            while t < 9.0 && !(r.grounded && t > 0.5) {
                r.step(world, rails, if r.grounded { Input::default() } else { Input { uber: true, ..Input::default() } }, dt);
                t += dt;
            }
            println!("{name}: landed after {t:.2} s -> \"{}\"  score {}  letters {}{}", r.last_trick, r.score, r.letters, if r.crashed > 0.0 { "  (wiped out)" } else { "" });
        }
        return;
    }
    if std::env::var("TRICKY_RAILTEST").is_ok() {
        // drop onto the five longest rails a little way in, moving along them, and see what happens
        let mut order: Vec<usize> = (0..rails.0.len()).collect();
        order.sort_by(|a, b| rails.0[*b].len().total_cmp(&rails.0[*a].len()));
        for &ri in order.iter().take(5) {
            let rail = &rails.0[ri];
            let (p, tan) = rail.at(rail.len() * 0.1);
            let dir = if tan.y <= 0.0 { tan } else { -tan };
            let mut r = Rider::new(p + Vec3::Y * 1.2, f32::atan2(-dir.x, -dir.z));
            r.vel = dir * 12.0;
            let (mut t, mut on, mut first, mut top) = (0.0f32, 0.0f32, None, 0.0f32);
            while t < 60.0 {
                let steer = if std::env::var("TRICKY_RAILSTEER").is_ok() && r.rail.is_some() && on < 0.5 { 1.0 } else { 0.0 };
                r.step(world, rails, Input { steer, ..Input::default() }, 1.0 / 120.0);
                t += 1.0 / 120.0;
                if r.rail.is_some() { on += 1.0 / 120.0; first.get_or_insert(t); top = top.max(r.vel.length()); }
                else if first.is_some() { break; }
            }
            println!("rail {ri}: {:.0} m long, locked on after {:.2} s, rode it for {:.1} s, top {:.0} km/h, left it {}, twist {:.0} deg, \"{}\"",
                rail.len(), first.unwrap_or(-1.0), on, top * 3.6, if r.grounded { "onto snow" } else { "into the air" }, r.rail_twist.to_degrees(), r.last_trick);
        }
        return;
    }
    if let Ok(pr) = std::env::var("TRICKY_PROBE") {
        let v: Vec<f32> = pr.split(',').filter_map(|x| x.parse().ok()).collect();
        let c = Vec3::new(v[0], v[1], v[2]);
        println!("heights relative to {c:?} (rows = z from -20 to +20, cols = x from -20 to +20, step 2 m; '..' = no rideable ground within +-15 m; W = wall contact at body height)");
        for dz in -10..=10 {
            let mut row = String::new();
            for dx in -10..=10 {
                let p = c + Vec3::new(dx as f32 * 2.0, 0.0, dz as f32 * 2.0);
                match world.ground(p, 15.0, 15.0) {
                    Some(h) => { let w = !world.push_out(Vec3::new(p.x, h.y + 0.8, p.z), BODY_RADIUS).1.is_empty(); row += &format!("{:4.0}{}", h.y - c.y, if w { "W" } else { " " }); }
                    None => row += "  .. ",
                }
            }
            println!("{row}");
        }
        for (i, p) in line.iter().enumerate().filter(|(_, p)| (**p - c).length() < 40.0) { println!("line {i}: {:?}", *p - c); }
        return;
    }
    let Some(&start) = line.first() else { println!("no race line"); return };
    let mut r = Rider::new(start + Vec3::Y * 1.0, start_yaw);
    let dt = 1.0 / 120.0;
    let (mut t, mut next_log, mut top, mut air, mut dist) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut finished = None;
    let mut cam = ChaseCam::default();
    let (mut prev_off, mut prev_d, mut jumps, mut frames, mut sub) = (Vec3::ZERO, Vec3::ZERO, Vec::<f32>::new(), 0u32, 0u32);
    let (mut was_grounded, mut flips) = (false, 0u32);
    let (mut prev_len, mut pumps, mut prev_resp, mut prev_vel, mut rider_jerk) = (0.0f32, 0u32, 0u32, Vec3::ZERO, Vec::<(f32, f32)>::new());
    let (mut stuck_for, stuck, mut last_idx) = (0.0f32, 0u32, 0usize);
    let mut grind = 0.0f32;
    let lane: Vec<f32> = std::env::var("TRICKY_LANE").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let mut ai = AiDriver::new(lane.first().copied().unwrap_or(0.0), lane.get(1).copied().unwrap_or(1.0));
    while t < seconds {
        let input = ai.drive(&mut r, line, 0.0, dt);
        let idx = ai.idx;
        let best = (idx, (line[idx.min(line.len() - 1)] - r.pos).length_squared());
        let before = r.pos;
        r.step(world, rails, input, dt);
        if r.rail.is_some() { grind += dt; }
        dist += (r.pos - before).length();
        if r.grounded != was_grounded { flips += 1; was_grounded = r.grounded; }
        // a 60 fps camera frame every second physics step: how unevenly does the view move?
        sub += 1;
        if sub % 2 == 0 {
            let (cp, _) = cam.update(world, &r, dt * 2.0);
            let off = cp - r.pos;
            let len = off.length();
            if frames > 2 && (len - prev_len).abs() > 0.25 && r.respawns == prev_resp { pumps += 1; }
            if frames > 2 { rider_jerk.push(((r.vel - prev_vel).length() * dt * 2.0, t)); }
            prev_len = len; prev_resp = r.respawns; prev_vel = r.vel;
            let d = off - prev_off;
            if frames > 2 { jumps.push((d - prev_d).length()); }
            prev_off = off; prev_d = d; frames += 1;
        }
        if !r.grounded { air += dt; }
        top = top.max(r.vel.length());
        t += dt;
        if t >= next_log {
            println!("t={:5.1}s  line {:3}/{}  off-line {:5.1} m  speed {:5.1} km/h  {}  pos ({:.0}, {:.0}, {:.0})",
                t, idx, line.len(), best.1.sqrt(), r.vel.length() * 3.6, if r.grounded { "ground" } else { "air   " }, r.pos.x, r.pos.y, r.pos.z);
            next_log += 10.0;
        }
        if ai.finished.is_some() { finished = Some(t); break; }
        let _ = (&mut stuck_for, &mut last_idx);
    }
    match finished {
        Some(t) => println!("FINISHED in {t:.1} s"),
        None => println!("did not finish in {seconds:.0} s (reached line point {}/{})", ai.idx, line.len()),
    }
    jumps.sort_by(|a, b| a.total_cmp(b));
    if !jumps.is_empty() {
        let q = |f: f32| jumps[((jumps.len() - 1) as f32 * f) as usize];
        println!("camera jerk per 60 fps frame (m): median {:.3}, 99% {:.3}, worst {:.2}; frames with a visible jolt (>0.15 m): {}",
            q(0.5), q(0.99), q(1.0), jumps.iter().filter(|j| **j > 0.15).count());
    }
    println!("frames where the camera distance jumped by more than 25 cm: {pumps}");
    println!("frames where the rider's own speed changed abruptly (>0.15 m/frame): {}", rider_jerk.iter().filter(|j| j.0 > 0.15).count());
    println!("ground/air switches: {flips} ({:.1} per second)", flips as f32 / t.max(1.0));
    println!("distance {:.0} m, top speed {:.0} km/h, airborne {:.0} s, on rails {:.0} s, autopilot stuck {} times, fell out of the world {} times", dist, top * 3.6, air, grind, ai.stuck + stuck, r.respawns - ai.stuck - stuck);
}

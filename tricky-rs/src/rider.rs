//! The race around the rider: the AI driver, the race state and the chase camera, plus the headless
//! self-test. The rider's physics itself is in the Bevy-free `tricky-game` crate (row F11a); it is
//! re-exported here so `crate::rider::...` paths keep working.

pub use tricky_game::rider::*;
use crate::collide::CollisionWorld;
use crate::rails::Rails;
use bevy::math::Vec3;
#[derive(Default, Clone)]
pub struct AiCourse { pub speed: Vec<f32>, pub crouch: Vec<bool>, /// the jump zone's value at each point (bit 0 flip, bit 1 spin), or -1
    pub jump: Vec<i32> }
impl AiCourse {
    /// Speed and jump zones along one AI path from its own events (distances along the path):
    /// the target speed is the one in force 6 m ahead, a jump zone counts from 3 m before it.
    pub fn from_path(l: &crate::course::Line, from: usize) -> Self {
        let mut speeds: Vec<&crate::course::Event> = l.events.iter().filter(|e| e.kind == 101).collect();
        speeds.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut c = AiCourse::default();
        for i in from..l.pts.len() {
            let a = l.at[i];
            let v = speeds.iter().rev().find(|e| e.start <= a + 6.0).map(|e| e.value).unwrap_or(0);
            c.speed.push(if v > 0 { v as f32 / 3.6 } else { 99.0 });
            let j = l.events.iter().find(|e| e.kind == 100 && e.start <= a + 3.0 && e.end >= a);
            c.crouch.push(j.is_some());
            c.jump.push(j.map_or(-1, |e| e.value));
        }
        c
    }
    fn extend(&mut self, o: AiCourse) { self.speed.extend(o.speed); self.crouch.extend(o.crouch); self.jump.extend(o.jump); }
    /// `events` are (start, end, type, value) in Bevy space, from `Level::ai_events`.
    pub fn build(line: &[Vec3], events: &[(Vec3, Vec3, i32, i32)]) -> Self {
        let n = line.len();
        let nearest = |p: Vec3| (0..n).min_by(|&a, &b| (line[a] - p).length_squared().total_cmp(&(line[b] - p).length_squared())).filter(|&i| (line[i] - p).length() < 25.0);
        let mut set: Vec<Option<f32>> = vec![None; n];
        let mut crouch = vec![false; n];
        let mut jump = vec![-1; n];
        for (a, b, kind, value) in events {
            let (Some(i), Some(j)) = (nearest(*a), nearest(*b)) else { continue };
            match kind {
                101 if *value > 0 => set[i] = Some(*value as f32 / 3.6),
                100 => {
                    for k in i.min(j)..=i.max(j) { crouch[k] = true; jump[k] = *value; }
                    if i == j && i > 0 { crouch[i - 1] = true; jump[i - 1] = *value; }
                }
                _ => {}
            }
        }
        // a target speed holds until the next one
        let mut cur = 99.0;
        let speed = set.iter().map(|s| { if let Some(v) = s { cur = *v; } cur }).collect();
        Self { speed, crouch, jump }
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
    /// seconds spent alongside the player, and until this rider may shove again
    pub beside: f32,
    seen_respawns: u32,
    /// how far above the snow the racing line is at each point (it flies over the gaps)
    over: Vec<f32>,
    slow_for: f32,
    pub shove_wait: f32,
    /// the trick planned for the current jump (AIComputer_PlanJumpTrick): spin and flip direction
    /// (-1, 0, +1), the grab (shoulder mask, 0 none), letting go late, and whether it is planned
    plan: (f32, f32, u8, bool),
    planned: bool,
    rng: u32,
    jumps: u32,
    /// the original's catch-up: this rider's physics runs this much faster or slower (0.7..1.5)
    pub time_scale: f32,
    was_crouching: bool,
    /// the way this rider is going through the AI path network: the points so far, their speeds
    /// and jumps, the last path added, and whether the route has run out of paths
    pub route: Vec<Vec3>,
    route_course: AiCourse,
    route_last: Option<usize>,
    route_done: bool,
}
impl AiDriver {
    pub fn new(offset: f32, skill: f32) -> Self { Self { idx: 0, last_idx: 0, stuck_for: 0.0, offset, skill, finished: None, stuck: 0, clock: 0.0, beside: 0.0, seen_respawns: 0, over: Vec::new(), slow_for: 0.0, shove_wait: 4.0, plan: (0.0, 0.0, 0, false), planned: false, rng: 0x9E37_79B9 ^ ((offset * 1000.0) as i32 as u32).wrapping_mul(2654435761) ^ ((skill * 977.0) as u32), jumps: 0, time_scale: 1.0, was_crouching: false, route: Vec::new(), route_course: AiCourse::default(), route_last: None, route_done: false } }
    /// rand() % 100 - 16, as the original rolls it against the tricks stat (0..255)
    fn roll(&mut self) -> i32 {
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5;
        (self.rng % 100) as i32 - 16
    }
    fn rand(&mut self) -> u32 { self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5; self.rng }
    /// `AIComputer_PlanJumpTrick` 0x139d58: whether to spin/flip (only where the jump allows), which way, a
    /// random grab of the rider's fifteen, and whether to hold it late.
    fn plan_trick(&mut self, r: &Rider, value: i32) {
        let b = (r.stats.tricks * 255.0) as i32;
        let (spin, flip) = (value & 2 != 0, value & 1 != 0);
        let trick = (spin || flip) && self.roll() < b;
        let k = if self.skill >= 1.0 { 0.0 } else { 1.0 - self.skill / 14.454015 };
        let late = (self.rand() % 100) as f32 * k > 20.0;
        let grab = if self.roll() < b { crate::trickdata::COMBOS[(self.rand() % 15) as usize] } else { 0 };
        let mut sd = if trick && spin { 1.0 } else { 0.0 };
        let mut fd = if trick && flip { 1.0 } else { 0.0 };
        if self.rand() & 1 != 0 { sd = -sd; }
        if self.rand() & 1 != 0 { fd = -fd; }
        self.plan = (sd, fd, grab, late);
        self.planned = true;
    }
    /// Our jumps do not always carry as far as the original's: drop a flip or spin that the
    /// predicted flight (jump straight off from here) has no time for, so the rider is not left
    /// upside down. The prediction steps the flight against the snow, as AirPredict_AtTakeoff does.
    fn trim_plan(&mut self, r: &Rider, world: &CollisionWorld) {
        let mut p = r.pos + Vec3::Y * 0.1;
        let mut v = r.vel + r.normal * JUMP_MIN;
        let (mut t, step) = (0.0f32, 1.0 / 30.0);
        while t < 4.0 {
            v.y -= if v.y > 0.0 { GRAVITY_RISING } else { GRAVITY_FALLING } * step;
            let q = p + v * step;
            t += step;
            if t > 0.1 && v.y < 0.0 && world.ground(q, 1.5, 0.0).is_some() { break; }
            p = q;
        }
        let k = 11.517 * r.stats.spin_scale();
        let need_flip = std::f32::consts::TAU / (k * 2.0 / 3.0) + 0.25;
        let need_spin = std::f32::consts::PI / k + 0.2;
        if t < need_flip { self.plan.1 = 0.0; }
        if t < need_spin { self.plan.0 = 0.0; }
    }
    pub fn progress(&self, line: &[Vec3]) -> f32 { if line.len() < 2 { 0.0 } else { self.idx as f32 / (line.len() - 1) as f32 } }
    /// Decide the controls for this step. Puts the rider back on the line if it wedges itself.
    /// `clear` is how far the rider is above the snow.
    /// Look the course over once: where does the line leave the ground?
    /// What this rider wants at a fork (the original's U3 preference): freestyle riders low on
    /// boost go for the trick lines (100), BX riders likewise; riders in the middle of the pack
    /// take the safe lines (0); alpine riders in the lead too.
    fn pref(r: &Rider, place: usize) -> f32 {
        match r.stats.kind {
            1 => if r.boost < 0.31187 { 100.0 } else if place == 3 || place == 4 { 0.0 } else { 50.0 },
            0 => if r.boost < 0.10338 { 100.0 } else if place == 2 || place == 3 { 0.0 } else { 50.0 },
            _ => if place == 0 { 0.0 } else { 50.0 },
        }
    }
    /// Drive along the AI path network (the original's opponents), choosing at each fork; with no
    /// paths, along the race line.
    pub fn drive_paths(&mut self, r: &mut Rider, world: &CollisionWorld, paths: &crate::course::AiPaths, line: &[Vec3], course: &AiCourse, place: usize, dt: f32) -> Input {
        if paths.is_empty() { self.learn(world, line); return self.drive(r, world, line, course, dt); }
        if self.route.is_empty() {
            match paths.select(r.pos, heading(r.yaw), Some(Self::pref(r, place)), None) {
                Some((pi, s)) => {
                    let l = &paths.paths[pi];
                    let from = l.at.iter().position(|a| *a >= s).unwrap_or(0);
                    self.route = l.pts[from..].to_vec();
                    self.route_course = AiCourse::from_path(l, from);
                    self.route_last = Some(pi);
                }
                None => { self.learn(world, line); return self.drive(r, world, line, course, dt); }
            }
        }
        // keep a stretch of route ahead: at the end of a path, pick the next one from where it ends
        while !self.route_done && self.idx + 12 >= self.route.len() {
            let n = self.route.len();
            let end = self.route[n - 1];
            let dir = end - self.route[n.saturating_sub(2)];
            match paths.select(end, Vec3::new(dir.x, 0.0, dir.z), Some(Self::pref(r, place)), self.route_last) {
                Some((pi, s)) if paths.paths[pi].point_at(s).distance(end) < 25.0 => {
                    let l = &paths.paths[pi];
                    let from = l.at.iter().position(|a| *a > s).unwrap_or(l.pts.len());
                    if from >= l.pts.len() { self.route_done = true; break; }
                    self.route.extend_from_slice(&l.pts[from..]);
                    self.route_course.extend(AiCourse::from_path(l, from));
                    self.route_last = Some(pi);
                }
                _ => {
                    // the network runs out (near the finish): carry on down the race line
                    let k = (0..line.len()).min_by(|a, b| (line[*a] - end).length_squared().total_cmp(&(line[*b] - end).length_squared())).unwrap_or(0);
                    let k = (k + 1).min(line.len());
                    self.route.extend_from_slice(&line[k..]);
                    let c = AiCourse { speed: course.speed.get(k..).unwrap_or(&[]).to_vec(), crouch: course.crouch.get(k..).unwrap_or(&[]).to_vec(), jump: course.jump.get(k..).unwrap_or(&[]).to_vec() };
                    self.route_course.extend(c);
                    self.route_done = true;
                }
            }
        }
        let route = std::mem::take(&mut self.route);
        let rc = std::mem::take(&mut self.route_course);
        self.learn(world, &route);
        let input = self.drive(r, world, &route, &rc, dt);
        self.route = route;
        self.route_course = rc;
        input
    }
    /// Never rejoin the line in, or just short of, something that puts the rider straight back (a
    /// shut iris door): on to the first line point with the next 12 m clear of it (the original
    /// puts a rider back on a respawnable path, which runs round the door).
    fn clear_of_resets(&mut self, world: &CollisionWorld, line: &[Vec3]) {
        let blocked = |i: usize| {
            let mut run = 0.0;
            let mut k = i;
            loop {
                if world.in_reset(line[k] + Vec3::Y * 1.8, BODY_RADIUS) { return true; }
                if k + 1 >= line.len() { return false; }
                let (a, b) = (line[k], line[k + 1]);
                // between line points too: they can be metres apart
                let n = ((b - a).length() / 1.0).ceil().max(1.0) as usize;
                for j in 1..n { if world.in_reset(a.lerp(b, j as f32 / n as f32) + Vec3::Y * 1.8, BODY_RADIUS) { return true; } }
                run += (b - a).length();
                k += 1;
                if run > 12.0 { return false; }
            }
        };
        let from = self.idx;
        while self.idx + 1 < line.len() && blocked(self.idx) && self.idx < from + 40 { self.idx += 1; }
        if blocked(self.idx) { self.idx = from; }
    }
    /// Moved far in one go (the Megaplex tube, a reset): find a new way through the paths.
    pub fn reroute(&mut self) { self.route.clear(); self.route_course = AiCourse::default(); self.route_last = None; self.route_done = false; self.idx = 0; self.last_idx = 0; self.over.clear(); }
    pub fn learn(&mut self, world: &CollisionWorld, line: &[Vec3]) {
        if self.over.len() == line.len() { return; }
        self.over = line.iter().map(|p| world.ground(*p + Vec3::Y * 2.0, 2.0, 60.0).map_or(60.0, |h| p.y - h.y)).collect();
    }
    pub fn drive(&mut self, r: &mut Rider, world: &CollisionWorld, line: &[Vec3], course: &AiCourse, dt: f32) -> Input {
        let clear = if r.grounded { 0.0 } else { world.ground(r.pos, 0.0, 60.0).map_or(60.0, |h| r.pos.y - h.y) };
        if line.len() < 4 { return Input::default(); }
        self.clock += dt;
        if self.finished.is_some() { return Input { brake: true, ..Input::default() }; }
        let mut best = (self.idx, f32::MAX);
        for (i, p) in line.iter().enumerate().take(self.idx + 12).skip(self.idx) {
            let d = (*p - r.pos).length_squared();
            if d < best.1 { best = (i, d); }
        }
        self.idx = best.0;
        if self.idx + 2 >= line.len() { self.finished = Some(self.clock); }
        if self.idx > self.last_idx { self.last_idx = self.idx; self.stuck_for = 0.0; } else { self.stuck_for += dt; }
        // wedged (hardly moving for a couple of seconds) or going nowhere: back onto the line
        self.slow_for = if r.vel.length() < 1.5 && r.crashed <= 0.0 { self.slow_for + dt } else { 0.0 };
        if (self.stuck_for > 7.0 || self.slow_for > 2.5) && self.clock > 12.0 {
            if std::env::var("TRICKY_RESETS").is_ok() { println!("stuck at t {:.0}: pos {:.1?} speed {:.1} grounded {} line point {} at {:.1?}, next {:.1?}", self.clock, r.pos, r.vel.length(), r.grounded, self.idx, line[self.idx], line[(self.idx + 1).min(line.len() - 1)]); }
            self.idx = (self.idx + 2).min(line.len() - 1);
            self.clear_of_resets(world, line);
            let nxt = line[(self.idx + 1).min(line.len() - 1)] - line[self.idx];
            r.respawn((line[self.idx] + Vec3::Y * 1.0, f32::atan2(-nxt.x, -nxt.z)));
            self.stuck += 1;
            self.stuck_for = 0.0;
            self.slow_for = 0.0;
            self.seen_respawns = r.respawns;
        }
        // sent back by a reset zone or a fall: rejoin on the racing line, not wherever the trouble started
        if r.respawns != self.seen_respawns {
            if self.clock > 1.0 {
                self.clear_of_resets(world, line);
                let nxt = line[(self.idx + 1).min(line.len() - 1)] - line[self.idx];
                r.respawn((line[self.idx] + Vec3::Y * 1.0, f32::atan2(-nxt.x, -nxt.z)));
            }
            self.seen_respawns = r.respawns;
        }
        // aim further down the line the faster we go
        let speed = r.vel.length();
        let last = line.len() - 1;
        let mut ahead = self.idx + 1;
        let mut run = 0.0;
        while ahead < last && run < (speed * 0.9).max(14.0) { run += (line[ahead + 1] - line[ahead]).length(); ahead += 1; }
        // how sharply the course bends over the next stretch decides how fast it can be taken
        let dir = |i: usize| { let d = line[(i + 1).min(last)] - line[i.min(last)]; Vec3::new(d.x, 0.0, d.z).normalize_or_zero() };
        let (mut bend, mut len, mut i) = (0.0f32, 0.0f32, self.idx);
        while i < last && len < (speed * 2.2).max(30.0) { bend += dir(i).angle_between(dir(i + 1)); len += (line[i + 1] - line[i]).length(); i += 1; }
        let safe_speed = if bend > 0.05 { (TURN_ACCEL * 0.75 * len / bend).sqrt() } else { 99.0 };
        let a = line[ahead.min(last)];
        let b = line[(ahead + 1).min(last)];
        let along = (b - a).normalize_or(Vec3::NEG_Z);
        let target = a + Vec3::new(-along.z, 0.0, along.x) * self.offset;
        let to = target - r.pos;
        let want = f32::atan2(-to.x, -to.z);
        let mut diff = want - r.yaw;
        while diff > std::f32::consts::PI { diff -= std::f32::consts::TAU; }
        while diff < -std::f32::consts::PI { diff += std::f32::consts::TAU; }
        if !r.grounded {
            if r.rail.is_some() {
                // AIComputer_RailInput: balance; once steady, the trick roll starts quarter turns
                self.planned = false;
                let b = (-2.0 * r.rail_off).clamp(-1.0, 1.0);
                if b.abs() > 0.1 { return Input { steer: b, ..Input::default() }; }
                if self.roll() < (r.stats.tricks * 255.0) as i32 {
                    if self.plan.0 == 0.0 { self.plan.0 = if b >= 0.0 { 1.0 } else { -1.0 }; }
                    return Input { steer: -self.plan.0, boost: true, ..Input::default() };
                }
                return Input::default();
            }
            if r.air_time < 0.05 { self.jumps += 1; }
            if std::env::var("TRICKY_AIDBG").is_ok() && r.air_time > 0.0 && r.air_time <= 1.0 / 120.0 + 1e-4 { println!("  takeoff: plan {:?} planned {} clear {:.1} vy {:.1} wind {:.2}/{:.2} spinw {:.1} flipw {:.1}", self.plan, self.planned, clear, r.vel.y, r.spin_ax.w, r.flip_ax.w, r.spin_ax.hi, r.flip_ax.hi); }
            // time left in the air (AirPredict): rise and fall onto snow at the height below now
            let vy = r.vel.y;
            let left = if vy > 0.0 {
                let peak = clear + vy * vy / (2.0 * GRAVITY_RISING);
                vy / GRAVITY_RISING + (2.0 * peak.max(0.0) / GRAVITY_FALLING).sqrt()
            } else {
                (vy + (vy * vy + 2.0 * GRAVITY_FALLING * clear.max(0.0)).sqrt()) / GRAVITY_FALLING
            };
            let thr = 0.8 / (r.stats.tricks * 0.8057834 + 0.78212035);
            // a plain jump (no zone): a grab once it has been up a while with time to spare
            if !self.planned && r.air_time > thr && left > thr {
                let g = crate::trickdata::COMBOS[(self.rand() % 15) as usize];
                self.plan = (0.0, 0.0, g, false);
                self.planned = true;
            }
            let (mut spin_dir, mut flip_dir, grab, late) = self.plan;
            if !self.planned { spin_dir = 0.0; flip_dir = 0.0; }
            // AIComputer_AirTrickInput: keep turning; near a whole turn stop if another will not fit
            let flip_deg = r.flip.abs().to_degrees() % 360.0;
            if flip_dir != 0.0 && !(5.0..=340.0).contains(&flip_deg) && r.flip.abs() > 0.5 {
                let w = r.flip_ax.w.abs().max(0.5);
                if left < std::f32::consts::TAU / w { flip_dir = 0.0; self.plan.1 = 0.0; }
            }
            let p = if r.stats.alpine { 360.0 } else { 180.0 };
            let spin_deg = r.spin.abs().to_degrees() % p;
            if spin_dir != 0.0 && (spin_deg > p - 20.0 || spin_deg < 5.0) && r.spin.abs() > 0.5 {
                let w = r.spin_ax.w.abs().max(0.5);
                if left < p.to_radians() / w { spin_dir = 0.0; self.plan.0 = 0.0; }
            }
            // the grab: held until the landing is near (later for a late let-go)
            let hold = grab != 0 && left > if late { 0.3 } else { thr };
            if !hold && grab != 0 && left <= thr { self.plan.2 = 0; }
            if !self.planned || (spin_dir == 0.0 && flip_dir == 0.0 && !hold) {
                // nothing to do: steer for the line
                return Input::default();
            }
            return Input { steer: spin_dir, flip: flip_dir, grab: if hold { grab } else { 0 }, ..Input::default() };
        }
        // steering (0x1372f8): proportional to the angle off, scaled by skill, a dead zone, a limit
        let out = diff.abs() * 6.2897 * self.skill / 1.0241;
        let steer = if out < 0.2 { 0.0 } else { -diff.signum() * out.min(0.9706) };
        // cruising (0x1375c8): the course's AI paths give a target speed; the line's bends cap it
        let target = course.speed.get(self.idx + 1).copied().unwrap_or(99.0).min(safe_speed);
        let gap = (self.idx + 1..(self.idx + 7).min(self.over.len())).any(|i| self.over[i] > 5.0) && self.over.get(self.idx).is_some_and(|o| *o < 5.0);
        let brake = !gap && speed > target + 1.389;
        let slow = gap || speed < (target - 1.389) * self.skill / 1.4103;
        let tuck = slow || speed < 8.33;
        let boost = (slow || r.boost >= 0.99) && steer.abs() < 0.08 && r.boost > 0.0;
        // jumps the AI paths mark: crouch through the zone, let go where it ends
        let crouch = course.crouch.get(self.idx).copied().unwrap_or(false) || course.crouch.get(self.idx + 1).copied().unwrap_or(false);
        let value = course.jump.get(self.idx).copied().filter(|v| *v >= 0).or(course.jump.get(self.idx + 1).copied().filter(|v| *v >= 0)).unwrap_or(0);
        // committing to the jump (AIComputer_GroundJumpApproach): close to the line and steady
        if crouch && !self.was_crouching { self.planned = false; }
        if crouch && !self.planned && steer.abs() < 0.536 { self.plan_trick(r, value); self.trim_plan(r, world); }
        if !crouch && r.grounded && self.planned && r.air_time <= 0.0 && !self.was_crouching { self.planned = false; }
        self.was_crouching = crouch;
        // winding up in the zone: the stick goes the way the trick will turn
        let (wind_s, wind_f) = if crouch && self.planned { (self.plan.0, self.plan.1) } else { (0.0, 0.0) };
        Input { steer, wind: Some(wind_s), flip: wind_f, tuck, brake, boost, jump: crouch && !brake, ..Input::default() }
    }
}

/// Timing a run from the gate to the end of the course's main race line.
#[derive(Clone, Copy, PartialEq)]
pub enum RaceState { Countdown, Running, Finished }
pub struct Race { pub line: Vec<Vec3>, pub idx: usize, pub time: f32, pub state: RaceState, pub best: Option<f32>, pub countdown: f32, pub course: AiCourse,
    /// the course's race lines, the player's progress along them, and seconds the show-off clock has gained
    pub lines: crate::course::Lines, pub prog: crate::course::Progress, pub bonus: f32, pub checkpoints: u32,
    /// the opponents' AI path network
    pub paths: crate::course::AiPaths,
    /// Tokyo Megaplex's lap tube, if the course has one
    pub tube: Option<crate::props::Tube>,
    /// the course's camera scripts, and whether the intro is playing (the countdown waits for it)
    pub cml: Option<std::sync::Arc<crate::intro::Cml>>, pub intro: bool,
    /// the riders' pre-race scene, and the course's start stage area (game units: where, how turned)
    pub scene: Option<crate::intro::SceneState>, pub stage: Option<(Vec3, bevy::prelude::Quat)>,
    /// the finish area (scene frame 2), and the post-race scenes: 0 not yet, 1 asked for, 2 playing,
    /// 3 over (results); the scripts and who plays what
    pub finish_area: Option<(Vec3, bevy::prelude::Quat)>, pub post: u8, pub post_scripts: Vec<String>, pub cast: Option<crate::intro::Cast> }
impl Race {
    /// The original's countdown: GO after 2.5 s (beeps at 0.5, 1.0, 1.5 and 2.0 s).
    pub fn new(line: Vec<Vec3>) -> Self { Self { line, idx: 0, time: 0.0, state: RaceState::Countdown, best: None, countdown: 2.5, course: AiCourse::default(), lines: Default::default(), prog: Default::default(), bonus: 0.0, checkpoints: 0, paths: Default::default(), tube: None, cml: None, intro: false, scene: None, stage: None, finish_area: None, post: 0, post_scripts: Vec::new(), cast: None } }
    pub fn restart(&mut self) { self.idx = 0; self.time = 0.0; self.state = RaceState::Countdown; self.countdown = 2.5; self.prog = Default::default(); self.bonus = 0.0; self.checkpoints = 0; self.post = 0; self.post_scripts.clear(); self.cast = None; }
    /// Metres of racing line still to go from a line point.
    pub fn remaining(&self, idx: usize) -> f32 { self.line.windows(2).skip(idx).map(|w| (w[1] - w[0]).length()).sum() }
    pub fn progress(&self) -> f32 { if self.line.len() < 2 { 0.0 } else { self.idx as f32 / (self.line.len() - 1) as f32 } }
    pub fn update(&mut self, r: &Rider, dt: f32) {
        if self.line.len() < 4 { return; }
        match self.state {
            RaceState::Countdown => {
                if !self.intro { self.countdown -= dt; }
                if self.countdown <= 0.0 { self.state = RaceState::Running; }
            }
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
                // the finish: the race lines' finish event when the course has one, else the line's end
                // (or, as a fallback, the end of the racing line the riders follow)
                let mut crossed = !self.lines.has_laps() && (self.idx + 2 >= self.line.len() || (self.progress() > 0.8 && (end - r.pos).length() < 35.0));
                for e in self.lines.update(&mut self.prog, r.pos, r.vel) {
                    if e.kind == crate::course::EV_CHECKPOINT { self.bonus += e.value as f32; self.checkpoints += 1; }
                    if e.kind == crate::course::EV_FINISH { crossed = true; }
                }
                if crossed {
                    self.state = RaceState::Finished;
                    self.best = Some(self.best.map_or(self.time, |b| b.min(self.time)));
                }
            }
            RaceState::Finished => {}
        }
    }
}

/// Third-person camera that trails the rider. Plain data so the self-test can measure it.
/// One of the original's named chase cameras (COMMONOB.CML), in metres and radians.
pub struct CamDef { pub name: &'static str, dist: f32, pitch: f32, div_down: f32, div_up: f32, lag: f32, look_up: f32, base_up: f32, pitch_off: f32, pub fov: f32, look_lag: f32, roll_div: f32, roll: bool }
/// The four the player cycles through, in the original's order: board, near, far, over.
pub const CAMS: [CamDef; 4] = [
    CamDef { name: "chase board", dist: 1.5, pitch: -0.5, div_down: 1.5, div_up: 1.0, lag: 6.0, look_up: 0.0, base_up: 0.0, pitch_off: 0.27, fov: 1.20, look_lag: 4.0, roll_div: 2.0, roll: true },
    CamDef { name: "chase near", dist: 1.8, pitch: -0.5, div_down: 2.0, div_up: 1.0, lag: 16.0, look_up: 1.0, base_up: 0.35, pitch_off: -0.16, fov: 1.65, look_lag: 5.0, roll_div: 4.0, roll: true },
    CamDef { name: "chase far", dist: 5.0, pitch: -0.7, div_down: 2.0, div_up: 1.5, lag: 15.0, look_up: 1.0, base_up: 0.0, pitch_off: 0.03, fov: 1.65, look_lag: 5.0, roll_div: 8.0, roll: true },
    CamDef { name: "chase over", dist: 10.0, pitch: -0.85, div_down: 2.0, div_up: 2.0, lag: 15.0, look_up: 1.0, base_up: 0.0, pitch_off: 0.15, fov: 1.65, look_lag: 25.0, roll_div: 4.0, roll: false },
];
/// A per-tick (60 Hz) fraction `1/div` as a fraction for a step of `dt` seconds.
fn tick_k(div: f32, dt: f32) -> f32 { 1.0 - (1.0 - 1.0 / div.max(1.0)).powf(dt * 60.0) }

/// The race camera (`cBxCamera_EvalNamedCam`): orbits behind the rider's filtered direction of travel.
pub struct ChaseCam { pub pos: Vec3, pub mode: usize, dir: Vec3, pitch: f32, yaw: f32, raw: Vec3, smooth: Vec3, head: f32, tilt: f32, pub roll: f32, pub fov: f32, blend: f32, cut: bool, dip: f32, dip_t: f32, was_air: bool, last_v: Vec3 }
impl Default for ChaseCam { fn default() -> Self { Self { pos: Vec3::ZERO, mode: 2, dir: Vec3::ZERO, pitch: 0.0, yaw: 0.0, raw: Vec3::ZERO, smooth: Vec3::ZERO, head: 0.0, tilt: 0.0, roll: 0.0, fov: 1.65, blend: 0.0, cut: true, dip: 0.0, dip_t: 1.0, was_air: false, last_v: Vec3::ZERO } } }
impl ChaseCam {
    pub fn cycle(&mut self, step: i32) { self.mode = (self.mode as i32 + step).rem_euclid(CAMS.len() as i32) as usize; self.cut = true; }
    pub fn cut(&mut self) { self.cut = true; }
    pub fn name(&self) -> &'static str { CAMS[self.mode].name }
    /// Returns (camera position, point to look at); `roll` and `fov` are left on the camera.
    pub fn update(&mut self, world: &CollisionWorld, r: &Rider, dt: f32) -> (Vec3, Vec3) {
        let c = &CAMS[self.mode];
        let down = r.crashed > 0.0;
        // the direction followed: velocity, filtered 5% a tick (1% while tumbling); the board when stopped
        let want = if r.vel.length() > 0.1 { r.vel.normalize() } else { heading(r.yaw) };
        let flat_len = |v: Vec3| Vec3::new(v.x, 0.0, v.z).length();
        if self.cut || self.dir == Vec3::ZERO || flat_len(self.dir) < 0.5 { self.dir = want; }
        else {
            let k = tick_k(if down { 100.0 } else { 20.0 }, dt);
            self.dir = (self.dir * (1.0 - k) + want * k).normalize_or(want);
        }
        let d = self.dir;
        let e = d.y.atan2(flat_len(d));
        let p_t = c.pitch + e / if d.y < 0.0 { c.div_down } else { c.div_up };
        let y_t = d.x.atan2(d.z);
        if self.cut { self.pitch = p_t; self.yaw = y_t; }
        else {
            let k = tick_k(c.lag, dt);
            self.pitch += (p_t - self.pitch) * k;
            self.yaw += wrap(y_t - self.yaw) * k;
        }
        let pivot = r.pos + Vec3::Y * (0.9 + c.base_up);
        let dist = c.dist;
        let back = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let mut cam = pivot - back * dist * self.pitch.cos() - Vec3::Y * dist * self.pitch.sin();
        // collision (Cam_CollidePivotToCamera): stop at whatever is in the way, pushed off it
        let off = if dist > 5.0 { 0.6 } else { 0.4 };
        if let Some(t) = world.raycast(pivot, cam) {
            let dir = (cam - pivot).normalize_or_zero();
            cam = pivot + dir * (t - off).max(0.3);
        }
        if let Some(h) = world.ground(cam + Vec3::Y * 0.5, 0.5, 3.0) { cam.y = cam.y.max(h.y + off); }
        // while the rider is down the camera all but stops where it is
        let kb = (dt * 60.0).min(4.0);
        self.blend = if down { (self.blend + 0.0291667 * kb).min(0.98) } else { (self.blend - 0.003 * kb).max(0.0) };
        if self.cut || self.blend <= 0.0 { self.raw = cam; } else { self.raw += (cam - self.raw) * (1.0 - self.blend.powf(dt * 60.0)); }
        // landing dip: a hard landing pulls the view down for five ticks
        if !r.grounded && r.rail.is_none() { self.was_air = true; self.last_v = r.vel; }
        else if self.was_air {
            self.was_air = false;
            let s = -self.last_v.y;
            if s > 10.0 { self.dip = 2.0 * ((s - 10.0) / 30.0).min(1.0); self.dip_t = 0.0; }
        }
        self.dip_t += dt;
        let mut look = pivot + Vec3::Y * c.look_up;
        if self.dip_t < 5.0 / 60.0 { look.y -= self.dip; }
        // the view: angles from the camera to the target, eased
        let to = look - self.raw;
        let head_t = to.x.atan2(to.z);
        let tilt_t = to.y.atan2(flat_len(to)) + c.pitch_off;
        let mut roll_t = 0.0;
        if c.roll && r.grounded && r.rail.is_none() && !down {
            let f = to.normalize_or(Vec3::Z);
            let u = (r.normal - f * r.normal.dot(f)).normalize_or(Vec3::Y);
            let z = (Vec3::Y - f * f.y).normalize_or(Vec3::Y);
            let a = z.dot(u).clamp(-1.0, 1.0).acos();
            roll_t = a.copysign(f.cross(z).dot(u)) / c.roll_div;
        }
        let look_lag = if self.blend > 0.0 && c.look_lag < 50.0 { 10.0 + 20.0 * self.blend } else { c.look_lag };
        if self.cut {
            self.smooth = self.raw; self.head = head_t; self.tilt = tilt_t; self.roll = roll_t; self.fov = c.fov;
        } else {
            let k = 1.0 - 0.8f32.powf(dt * 60.0);
            self.smooth += (self.raw - self.smooth) * k;
            let kl = tick_k(look_lag, dt);
            self.head += wrap(head_t - self.head) * kl;
            self.tilt += wrap(tilt_t - self.tilt) * kl;
            self.roll += wrap(roll_t - self.roll) * tick_k(25.0, dt);
            self.fov += (c.fov - self.fov) * kl;
        }
        self.cut = false;
        // a snap if the rider was moved far (a reset)
        if self.smooth.distance(pivot) > dist * 4.0 + 20.0 { self.cut = true; }
        self.pos = self.smooth;
        let view = Vec3::new(self.head.sin() * self.tilt.cos(), self.tilt.sin(), self.head.cos() * self.tilt.cos());
        (self.pos, self.pos + view * 5.0)
    }
}




/// Headless check: ride the course on autopilot along the race line and report what happens.
pub fn self_test(world: &mut CollisionWorld, rails: &Rails, line: &[Vec3], course: &AiCourse, lines: &crate::course::Lines, paths: &crate::course::AiPaths, tube: Option<&crate::props::Tube>, start_yaw: f32, seconds: f32) {
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
            // as if it had left the snow with a full wind-up for the trick being tested
            let k = 11.517 * r.stats.spin_scale();
            if flip_s > 0.0 { r.flip_ax = Axis::start(-k * 2.0 / 3.0); }
            if spin_s > 0.0 { r.spin_ax = Axis::start(k); }
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
        // a Late trick: spin, let it settle, then press again in the same jump
        {
            let mut r = Rider::new(start + Vec3::Y * 60.0, start_yaw);
            let k = 11.517 * r.stats.spin_scale();
            r.spin_ax = Axis::start(k);
            let (mut t, dt, mut seen, mut second) = (0.0f32, 1.0 / 120.0, Vec::<String>::new(), None::<f32>);
            while t < 9.0 && !(r.grounded && t > 0.5) {
                let air = r.air_time;
                let settled = r.air_mode2 && r.spin_ax.w.abs() < 0.7 && air > 1.2;
                if second.is_none() && settled { second = Some(air); }
                let steer = if air < 0.35 || second.is_some_and(|s| air < s + 0.35) { 1.0 } else { 0.0 };
                r.step(world, rails, Input { steer, ..Input::default() }, dt);
                if !r.last_trick.is_empty() && seen.last() != Some(&r.last_trick) { seen.push(r.last_trick.clone()); }
                t += dt;
            }
            println!("late trick: {:?}", seen);
        }
        // uber tricks: a two-second signature move started with a full meter
        for (name, height, full) in [("uber with enough air", 95.0, true), ("uber without enough air", 12.0, true), ("uber without a full meter", 95.0, false)] {
            let mut r = Rider::new(start + Vec3::Y * height, start_yaw);
            r.ubers = vec![("bxUT_MTINDY".into(), 60.0), ("bxUT_OTHER".into(), 60.0)];
            if full { r.add_meter(1.0); } else { r.add_meter(0.4); }
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            while t < 9.0 && !(r.grounded && t > 0.5) {
                r.step(world, rails, if r.grounded { Input::default() } else { Input { grab: if r.air_time < 1.2 { 1 } else { 0 }, tweak: r.air_time > 0.6, ..Input::default() } }, dt);
                t += dt;
            }
            println!("{name}: landed after {t:.2} s -> \"{}\"  score {}  letters {}{}", r.last_trick, r.score, r.letters, if r.crashed > 0.0 { "  (wiped out)" } else { "" });
        }
        return;
    }
    if std::env::var("TRICKY_GATESIM").is_ok() {
        // the gate: hold forward, then rock back n ticks before GO; how fast and how soon out?
        for back in [0, 10, 20, 26, 30, 40, 60] {
            let mut r = Rider::new(Vec3::ZERO, 0.0);
            let dt = 1.0 / 60.0;
            for tick in 0..150 { r.gate_anticipate(if tick >= 150 - back { -1 } else { 1 }, dt); }
            r.gate_go();
            let mut t = 0;
            while r.gate_launch(dt) && t < 600 { t += 1; }
            println!("rock back {back:2} ticks before GO: launch {:.2} m/s after {} ticks", r.vel.length(), t);
        }
        return;
    }
    if std::env::var("TRICKY_JUMPTEST").is_ok() {
        // ride off the start, hold Space for a while, let go, and see how high it goes
        let Some(&start) = line.first() else { return };
        for hold in [0.1f32, 0.4, 0.8] {
            let mut r = Rider::new(start + Vec3::Y * 0.3, start_yaw);
            let (mut t, dt) = (0.0f32, 1.0 / 120.0);
            let (mut top, mut left_at, mut air) = (0.0f32, None, 0.0f32);
            while t < 6.0 {
                let jump = t > 2.0 && t < 2.0 + hold;
                r.step(world, rails, Input { jump, ..Input::default() }, dt);
                if t > 2.0 && !r.grounded { if let Some(g) = world.ground(r.pos, 0.0, 50.0) { top = top.max(r.pos.y - g.y); } }
                if t > 2.0 && !r.grounded { air = air.max(r.air_time); if left_at.is_none() { left_at = Some(t - 2.0 - hold); } }
                t += dt;
            }
            println!("hold {hold:.1} s: left the snow {:?} s after letting go, peak {:.2} m above the snow, air {:.2} s", left_at, top, air);
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
        if let Some(h) = world.ground(c, 15.0, 15.0) { world.explain(Vec3::new(c.x, h.y + 0.8, c.z), BODY_RADIUS); }
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
    let (mut crashes, mut was_down, mut stumbles, mut was_stumble) = (0u32, false, 0u32, false);
    let mut last_seen = String::new();
    let mut prog = crate::course::Progress::default();
    let mut tube_t = 101.0f32;
    let lane: Vec<f32> = std::env::var("TRICKY_LANE").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let mut ai = AiDriver::new(lane.first().copied().unwrap_or(0.0), lane.get(1).copied().unwrap_or(1.0));
    let crash_at: Option<f32> = std::env::var("TRICKY_CRASHAT").ok().and_then(|v| v.parse().ok());
    while t < seconds {
        r.course_dir = { let l: &[Vec3] = if ai.route.len() > 2 { &ai.route } else { line }; let i = ai.idx.min(l.len().saturating_sub(2)); let j = (i + 4).min(l.len() - 1); Some((l[j] - l[i]).normalize_or_zero()) };
        r.course_pts = { let l: &[Vec3] = if ai.route.len() > 2 { &ai.route } else { line }; course_points(l, ai.idx, r.pos) };
        if crash_at.is_some_and(|c| t <= c && t + dt > c) && r.crashed <= 0.0 { r.force_wipe(); }
        // TRICKY_LINE: follow the race line instead of the AI path network
        let input = if std::env::var("TRICKY_LINE").is_ok() { ai.learn(world, line); ai.drive(&mut r, world, line, course, dt) }
            else { ai.drive_paths(&mut r, world, paths, line, course, 2, dt) };
        if r.crashed > 0.0 && !was_down { crashes += 1; println!("  wiped out at t {t:.1} ({}) speed {:.0} km/h", r.cause, r.vel.length() * 3.6); }
        if r.crashed > 0.0 && !was_down && std::env::var("TRICKY_AIDBG").is_ok() { println!("    flip {:.2} spin {:.2} air {:.2}", r.flip, r.spin, r.air_time); }
        if was_down && r.crashed <= 0.0 { println!("    back up after {:.1} s at {:.0} km/h", r.down_t, r.vel.length() * 3.6); }
        was_down = r.crashed > 0.0;
        if r.stumble > 0.0 && !was_stumble { stumbles += 1; }
        was_stumble = r.stumble > 0.0;
        if r.last_trick != last_seen { if !r.last_trick.is_empty() && std::env::var("TRICKY_TRICKLOG").is_ok() { println!("  t {t:.1}: {}", r.last_trick); } last_seen = r.last_trick.clone(); }
        if std::env::var("TRICKY_RESETS").is_ok() && world.in_reset(r.pos + Vec3::Y * 0.8, BODY_RADIUS) { println!("reset at t {t:.1} pos {:.1?} idx {} grounded {} speed {:.0}", r.pos, ai.idx, r.grounded, r.vel.length()); }
        if std::env::var("TRICKY_TRACE").is_ok() && ((t / 0.5) as i32) != (((t - dt) / 0.5) as i32) { println!("t {t:.1} pos {:.1?} vel {:.1?} speed {:.1} grounded {} normal {:.2?} surface {} steer {:.2} yaw {:.2} idx {}", r.pos, r.vel, r.vel.length(), r.grounded, r.normal, r.surface, input.steer, r.yaw, ai.idx); }
        let idx = ai.idx;
        let best = (idx, (line[idx.min(line.len() - 1)] - r.pos).length_squared());
        let before = r.pos;
        let air_before = r.air_time;
        if !paths.is_empty() && r.grounded && r.crashed <= 0.0 && r.rail.is_none() {
            if let Some((p, yaw)) = paths.respawn_point(r.pos) {
                let ground = world.ground(p + Vec3::Y * 3.0, 3.0, 20.0).map_or(p.y, |h| h.y);
                r.safe = (Vec3::new(p.x, ground, p.z), yaw);
                r.safe_external = true;
            }
        }
        // no scripts here: the iris doors' buttons open them directly
        let shut: Vec<bool> = world.gates.iter().map(|g| g.t.is_none()).collect();
        world.touch_gates(&[r.pos + Vec3::Y * 0.8]);
        if std::env::var("TRICKY_RESETS").is_ok() { for (g, s) in world.gates.iter().zip(shut) { if s && g.t.is_some() { println!("  t {t:.1}: door {} opens (open {:.1}..{:.1} s of {:.1})", g.inst, g.open.0, g.open.1, g.len); } } }
        world.tick_gates(dt);
        r.step(world, rails, input, dt);
        if let Some(tb) = tube { if tb.apply(&mut r, prog.laps, dt) { ai.reroute(); tube_t = t; println!("  t {t:.1}: up the tube, {} laps to go", prog.laps); } }
        if std::env::var("TRICKY_TUBE").is_ok() && ((t > tube_t - 2.0 && t < tube_t) || (t - tube_t < 3.0 && t >= tube_t)) && ((t * 10.0) as i32) != (((t - dt) * 10.0) as i32) { println!("    t {t:.1} pos {:.1?} vel {:.1?} grounded {} class {} down {:.1}", r.pos, r.vel, r.grounded, r.tube_class, r.crashed); }
        if std::env::var("TRICKY_RESETS").is_ok() && (r.pos - before).length() > 3.0 { println!("put back at t {t:.1}: was at {:.1?} (line point {} at {:.1?}), air {:.1} s, {}", before, ai.idx, line[ai.idx.min(line.len() - 1)], air_before, if air_before > 8.9 { "in the air too long" } else if before.y < world.min_y - 29.0 { "below the world" } else { "reset zone or autopilot" }); }
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
        for e in lines.update(&mut prog, r.pos, r.vel) {
            println!("  t {t:.1}: course event {} (value {}) on line {}, {:.0} m to go", e.kind, e.value, prog.line, prog.dtf);
            if e.kind == crate::course::EV_FINISH && finished.is_none() { println!("  finish event at t {t:.1}"); }
        }
        if ai.finished.is_some() { finished = Some(t); break; }
        let _ = (&mut stuck_for, &mut last_idx);
    }
    match finished {
        Some(t) => {
            println!("FINISHED in {t:.1} s");
            // over the line: the finish state brakes the rider to a stop
            r.start_finish(true);
            let v0 = r.vel.length();
            let mut stop_at = None;
            for i in 0..(8 * 120) {
                let inp = r.finish_input(Input::default());
                r.step(world, rails, inp, 1.0 / 120.0);
                r.finish_after(1.0 / 120.0);
                if stop_at.is_none() && r.finished_and_stopped() { stop_at = Some(i as f32 / 120.0); }
            }
            println!("finish state: {:.1} m/s at the line, state {:?}, results after {:?} s", v0, r.fin.map(|f| f.0), stop_at);
        }
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
    println!("wipe-outs: {crashes}, stumbles: {stumbles}, score {}, last trick \"{}\"", r.score, r.last_trick);
    println!("ground/air switches: {flips} ({:.1} per second)", flips as f32 / t.max(1.0));
    println!("distance {:.0} m, top speed {:.0} km/h, airborne {:.0} s, on rails {:.0} s, autopilot stuck {} times, fell out of the world {} times", dist, top * 3.6, air, grind, ai.stuck + stuck, r.respawns - ai.stuck - stuck);
}

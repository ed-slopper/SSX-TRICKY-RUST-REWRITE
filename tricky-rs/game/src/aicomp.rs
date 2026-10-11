//! The computer rider's target and steering helpers exactly (board row G4b): `AIComputer_SteerTowards` 0x1372f8,
//! `AIComputer_SetThrottle` 0x139f60, `AIComputer_SetTuck` 0x13a028, `AIComputer_IsRivalTarget` 0x139928 and
//! `AIComputer_PickTargetRider` 0x1396c8, in the game's units (cm, z up) and the vector unit's order of operations,
//! checked bit for bit in the function runner (`tests/aicomp.rs`). The computer object (cComputer) keeps its rider
//! (+0), its skill scale (+0x86c), its target rider (+0x890).

use crate::jump::{add, dot, normalize, scale, sub};

type V4 = [f32; 4];

/// The rider's frame as the AI reads it: position (+0x140), forward (+0x4aa0), side (+0x4ab0), up (+0x4ac0).
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub pos: V4,
    pub fwd: V4,
    pub side: V4,
    pub up: V4,
}

/// `AIComputer_SteerTowards` 0x1372f8: the steer (−0.9705659..0.9705659) toward `target`: its direction flattened
/// onto the rider's plane (normal = up), the angle −atan2(d·fwd, d·side) (the game's inline atan2 around
/// `Math_Atan`), × 6.2897024, × skill / 1.0241041 when the skill scale is not 1; under 0.2 nothing.
pub fn steer_towards(skill: f32, f: &Frame, target: V4) -> f32 {
    let d = normalize(sub(target, f.pos));
    let h = dot(d, f.up);
    let p = normalize(sub(d, scale(f.up, h)));
    let s = dot(p, f.side);
    let fw = dot(p, f.fwd);
    let angle = if s == 0.0 {
        if fw != 0.0 {
            if 0.0 <= fw { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 }
        } else {
            -fw
        }
    } else {
        let mut t = crate::math::atan(fw / s);
        if s < 0.0 {
            t = if 0.0 < fw { t + std::f32::consts::PI } else { t - std::f32::consts::PI };
        }
        -t
    };
    let mut v = angle.abs() * 6.289_702_4;
    if skill != 1.0 {
        v *= skill / 1.024_104_1;
    }
    if v < 0.2 {
        return 0.0;
    }
    if 0.970_565_9 < v {
        v = 0.970_565_9;
    }
    if angle < 0.0 {
        -v
    } else {
        v
    }
}

/// `AIComputer_SetThrottle` 0x139f60: the pad word's crouch stick (bits 11–16) full, or skill / 2.211699 when the
/// skill scale is not 1 and the rider is at 833.3333 cm/s or more.
pub fn set_throttle(skill: f32, vel: V4, word: u32) -> u32 {
    let mut k = 1.0;
    if skill != 1.0 {
        k = skill / 2.211_699;
    }
    if dot(vel, vel).sqrt() < 833.333_3 {
        k = 1.0;
    }
    word & 0xfffe_07ff | (((k * 31.0) as i32 as u32) & 0x3f) << 11
}

/// `AIComputer_SetTuck` 0x13a028: as [`set_throttle`], with the tuck bit (8) set.
pub fn set_tuck(skill: f32, vel: V4, word: u32) -> u32 {
    set_throttle(skill, vel, word) | 8
}

/// The grudge fields `AIComputer_IsRivalTarget` reads (rider+0x118 on, +0x120 against whom, +0x124 level, +0x128
/// cooled).
#[derive(Clone, Copy, Debug, Default)]
pub struct GrudgeOf {
    pub on: i32,
    pub target: i32,
    pub level: i32,
    pub cooled: i32,
}

/// `AIComputer_IsRivalTarget` 0x139928: rider `i` is this one's rival: in knockdown practice (game mode 0) always;
/// else a grudge against it at level 2, or at level 1 not yet cooled.
pub fn is_rival_target(practice: bool, g: &GrudgeOf, i: i32) -> bool {
    if practice {
        return true;
    }
    g.on != 0 && g.target == i && if g.level == 1 { g.cooled == 0 } else { g.level == 2 }
}

/// `AIComputer_PickTargetRider` 0x1396c8: whether the computer rider has a rider to ride at (`target`, +0x890, −1
/// for none) and if it is a rival (`rival`, written only when a rider is picked). A target is dropped past 1050 cm
/// (the pair record) or when rider+0x348 is over 600. With none, under 540 of rider+0x348 and on every 12th tick, the
/// riders within 700 cm are weighed: distance × (1 − cos bearing) (ahead is nearest), a rival's × (1 + |cos
/// bearing|); the lightest (ties to the later) is taken. `recs[i]` = the pair record [distance, bearing],
/// `heading` rider+0x1b0, `me` rider+0x460, `n` the riders in the race.
#[allow(clippy::too_many_arguments)]
pub fn pick_target_rider(target: &mut i32, rival: &mut bool, recs: &[[f32; 2]], heading: f32, x348: f32, me: i32, n: i32, tick: i32, is_rival: impl Fn(i32) -> bool) -> bool {
    if 0 <= *target && (1050.0 < recs[*target as usize][0] || 600.0 < x348) {
        *target = -1;
    }
    if 0 <= *target {
        return true;
    }
    if !(x348 < 540.0) || tick % 12 != 0 {
        return false;
    }
    let mut best = 1e10f32;
    for i in 0..n {
        if i == me {
            continue;
        }
        let dist = recs[i as usize][0];
        if !(dist <= 700.0) {
            continue;
        }
        let r = is_rival(i);
        let a = recs[i as usize][1] - heading;
        let k = a * 0.159_154_94 + 0.5;
        let mut fl = k as i32 as f32;
        if k < fl {
            fl -= 1.0;
        }
        let c = crate::math::cos(a - fl * 6.283_185_5);
        let w = if r { c.abs() + 1.0 } else { 1.0 - c };
        let v = dist * w;
        if v <= best {
            *rival = r;
            *target = i;
            best = v;
        }
    }
    0 <= *target
}

/// `AIComputer_FindNearbyRider` 0x1394e0 (called by `AIComputer_ControlsDispatch` 0x1389c8 every tick): the shove.
/// Not while the rider's layer-1 clip is a shove (type 0x15), only on every 6th tick: the first rider (in index
/// order) within 200 cm (the pair record) and alongside (the bearing from the rider's heading, wrapped, between 30°
/// and 150° either side) that the rider holds a grudge against at level 1 or 2 (cooled or not). `Some(true)` when
/// that rider is to the left of the heading (the bearing difference above 0), else `Some(false)`; the dispatcher
/// puts it in the pad word's shove field (bits 23–24: 3 for true, 1 for false).
pub fn find_nearby_rider(shoving: bool, tick: i32, recs: &[[f32; 2]], heading: f32, me: i32, n: i32, g: &GrudgeOf) -> Option<bool> {
    if shoving || tick % 6 != 0 {
        return None;
    }
    for i in 0..n {
        if i == me {
            continue;
        }
        let r = recs[i as usize];
        if !(r[0] < 200.0) {
            continue;
        }
        let a = heading - r[1];
        let k = a * 0.159_154_94 + 0.5;
        let mut fl = k as i32 as f32;
        if k < fl {
            fl -= 1.0;
        }
        let a = a - fl * 6.283_185_5;
        let m = a.abs();
        if !(0.523_598_8 < m) || !(m < 2.617_993_8) {
            continue;
        }
        if g.on != 0 && g.target == i && (g.level == 1 || g.level == 2) {
            return Some(0.0 < a);
        }
    }
    None
}

/// The shove field `AIComputer_ControlsDispatch` 0x1389c8 writes from [`find_nearby_rider`].
pub fn shove_bits(word: u32, side: Option<bool>) -> u32 {
    match side {
        Some(true) => word & 0xfe7f_ffff | 0x180_0000,
        Some(false) => word & 0xfe7f_ffff | 0x80_0000,
        None => word,
    }
}

// ---- the riding modes (board row G4d)

/// The riding modes (cComputer+0x89c; the method table at 0x31d318 holds them in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `AIComputer_RideToJump` 0x137858
    ToJump = 0,
    /// `AIComputer_RideLine` 0x1375c8
    Line = 1,
    /// `AIComputer_RideNearRider` 0x137dc0
    NearRider = 2,
    /// `AIComputer_RideAtRival` 0x137a18
    AtRival = 3,
}

/// What a riding mode reads of the computer and its rider.
#[derive(Clone, Debug)]
pub struct Ai {
    /// cComputer +0x86c the skill scale, +0x868 the speed to ride at (cm/s; the course queries set it), +0x890 the
    /// target rider, +0x89c the mode
    pub skill: f32,
    pub speed: f32,
    pub target: i32,
    pub mode: Mode,
    /// the rider: its frame, velocity (+0x150), the point on its line (+0x360), its travel heading (+0x370),
    /// heading (+0x1b0), rider+0x348, the meter (+0x1c), index (+0x460), pair records, grudge
    pub frame: Frame,
    pub vel: V4,
    pub line: V4,
    pub travel: f32,
    pub heading: f32,
    pub x348: f32,
    pub meter: f32,
    pub me: i32,
    pub recs: Vec<[f32; 2]>,
    pub grudge: GrudgeOf,
}

/// What the riding modes ask of the course and the race.
pub struct World<'a> {
    /// `AIComputer_FindJumpMarker` 0x1392a8: a jump marker ahead (three words) or none; may set the speed
    pub jump_marker: &'a mut dyn FnMut(&mut f32) -> Option<[i32; 3]>,
    /// `AIComputer_FindSpeedHint` 0x1393e0: a speed hint (its flag) or none; may set the speed
    pub speed_hint: &'a mut dyn FnMut(&mut f32) -> Option<i32>,
    /// the target rider's position (+0x140) and velocity (+0x150) (`Race_GetRider`)
    pub other: &'a dyn Fn(i32) -> (V4, V4),
    /// the race's tick, its riders, knockdown practice
    pub tick: i32,
    pub n: i32,
    pub practice: bool,
}

/// What a riding mode calls besides its own helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeCall {
    /// `AIComputer_PlanJumpTrick` 0x139d58 with the jump marker's second and third words
    PlanJumpTrick(i32, i32),
}

fn speed_of(v: V4) -> f32 {
    dot(v, v).sqrt()
}

fn steer_bits(word: u32, s: f32) -> u32 {
    word & 0xffff_f81f | (((s * 31.0) as i32 as u32) & 0x3f) << 5
}

const BRAKE: u32 = 0x3e_0000;

fn brake(word: u32) -> u32 {
    word & 0xff81_ffff | BRAKE
}

fn wrap(a: f32) -> f32 {
    let k = a * 0.159_154_94 + 0.5;
    let mut fl = k as i32 as f32;
    if k < fl {
        fl -= 1.0;
    }
    a - fl * 6.283_185_5
}

impl Ai {
    fn pick(&mut self, w: &World, rival: &mut bool) -> bool {
        let g = self.grudge;
        let practice = w.practice;
        pick_target_rider(&mut self.target, rival, &self.recs, self.heading, self.x348, self.me, w.n, w.tick, |i| is_rival_target(practice, &g, i))
    }

    /// One tick of the current mode (the call `AIComputer_ControlsDispatch` makes): the pad word's steer, crouch,
    /// tuck, brake and jump bits, the next mode.
    pub fn ride(&mut self, w: &mut World, word: u32, calls: &mut Vec<ModeCall>) -> u32 {
        match self.mode {
            Mode::ToJump => self.ride_to_jump(w, word, calls),
            Mode::Line => self.ride_line(w, word),
            Mode::NearRider => self.ride_near_rider(w, word),
            Mode::AtRival => self.ride_at_rival(w, word),
        }
    }

    /// `AIComputer_RideLine` 0x1375c8: steer at the line; with a speed hint hold the speed (tuck or crouch under
    /// it less 138.89 cm/s (× skill / 1.4103198), brake over it plus 138.89 if the hint says so); without, crouch, and
    /// boost (bit 3) with a full meter (× skill / 1.4103198). Then a jump marker ahead → to the jump; a target rider →
    /// near it, or at it if a rival.
    pub fn ride_line(&mut self, w: &mut World, word: u32) -> u32 {
        let s = steer_towards(self.skill, &self.frame, self.line);
        let mut word = steer_bits(word, s);
        if let Some(flag) = (w.speed_hint)(&mut self.speed) {
            let my = speed_of(self.vel);
            if self.speed + 138.888_89 < my {
                if flag != 0 {
                    word = brake(word);
                }
            } else {
                let mut lim = self.speed - 138.888_89;
                if self.skill != 1.0 {
                    lim *= self.skill / 1.410_319_8;
                }
                if my < lim {
                    word = if flag == 0 { set_throttle(self.skill, self.vel, word) } else { set_tuck(self.skill, self.vel, word) };
                }
            }
        } else {
            let full = if self.skill == 1.0 { 1.0 <= self.meter } else { 1.0 <= self.meter * (self.skill / 1.410_319_8) };
            if full {
                word |= 8;
            }
            word = set_throttle(self.skill, self.vel, word);
        }
        if (w.jump_marker)(&mut self.speed).is_some() {
            self.target = -1;
            self.mode = Mode::ToJump;
        } else {
            let mut rival = false;
            if self.pick(w, &mut rival) {
                self.mode = if rival { Mode::AtRival } else { Mode::NearRider };
            }
        }
        word
    }

    /// `AIComputer_RideToJump` 0x137858: with no jump marker ahead, back to the line (and ride it this tick); else
    /// steer at the line, and on a marker that wants a trick (its first word), under 153.10213 of rider+0x348 and
    /// steering under 0.53616524, plan the trick and jump (bit 1); else hold the marker's speed (tuck under it less
    /// 138.89, brake over it plus 138.89).
    pub fn ride_to_jump(&mut self, w: &mut World, word: u32, calls: &mut Vec<ModeCall>) -> u32 {
        let Some(m) = (w.jump_marker)(&mut self.speed) else {
            self.mode = Mode::Line;
            return self.ride_line(w, word);
        };
        let s = steer_towards(self.skill, &self.frame, self.line);
        let word = steer_bits(word, s);
        let my = speed_of(self.vel);
        if m[0] != 0 && !(153.102_13 <= self.x348) && s.abs() < 0.536_165_24 {
            calls.push(ModeCall::PlanJumpTrick(m[1], m[2]));
            return word | 2;
        }
        let v = self.speed;
        if my < v - 138.888_89 {
            return set_tuck(self.skill, self.vel, word);
        }
        if my <= v + 138.888_89 {
            return word;
        }
        brake(word)
    }

    /// `AIComputer_RideNearRider` 0x137dc0: the target behind (over 90° off the travel heading +0x370): steer at
    /// the line, tucking to keep up if slower. Within the angle a 150 cm wide body takes at its distance (atan 150 /
    /// d) and faster than it: steer past it, at the line turned by the angle out of that cone, and tuck. Else steer at
    /// the line and crouch. Then a jump marker → to the jump; no target → the line; a rival → at it.
    pub fn ride_near_rider(&mut self, w: &mut World, word: u32) -> u32 {
        let (_, ov) = (w.other)(self.target);
        let rec = self.recs[self.target as usize];
        let a = wrap(rec[1] - self.travel);
        let os = speed_of(ov);
        let my = speed_of(self.vel);
        let m = a.abs();
        let steer;
        let mut word = word;
        let mut tucked = false;
        if std::f32::consts::FRAC_PI_2 < m {
            steer = steer_towards(self.skill, &self.frame, self.line);
            if my < os {
                word = set_tuck(self.skill, self.vel, word);
                tucked = true;
            }
        } else {
            let d = rec[0];
            let mut cone = std::f32::consts::FRAC_PI_2;
            if d != 0.0 {
                cone = crate::math::atan(150.0 / d);
                if d < 0.0 {
                    cone += std::f32::consts::PI;
                }
            }
            if m <= cone && os < my {
                let turn = if 0.0 < a { a - cone } else { cone - a };
                let (sn, cs) = crate::math::sin_cos(turn);
                let p = self.frame.pos;
                let (px, py) = (self.line[0] - p[0], self.line[1] - p[1]);
                let at = [(p[0] + px * cs) - py * sn, p[1] + px * sn + py * cs, self.line[2], 1.0];
                steer = steer_towards(self.skill, &self.frame, at);
                word = set_tuck(self.skill, self.vel, word);
                tucked = true;
            } else {
                steer = steer_towards(self.skill, &self.frame, self.line);
            }
        }
        if !tucked {
            word = set_throttle(self.skill, self.vel, word);
        }
        let word = steer_bits(word, steer);
        if (w.jump_marker)(&mut self.speed).is_some() {
            self.target = -1;
            self.mode = Mode::ToJump;
        } else {
            let mut rival = false;
            if !self.pick(w, &mut rival) {
                self.mode = Mode::Line;
            } else if rival {
                self.mode = Mode::AtRival;
            }
        }
        word
    }

    /// `AIComputer_RideAtRival` 0x137a18: the rival behind (over 90°): steer at the line, braking when over 200 cm
    /// away and faster. Else aim 300 cm ahead of it along its velocity: within 15° tuck, 15°–50° crouch, beyond that
    /// crouch if slower and brake if faster. Then a jump marker → to the jump; no target → the line; a target no
    /// longer a rival → near it.
    pub fn ride_at_rival(&mut self, w: &mut World, word: u32) -> u32 {
        let (op, ov) = (w.other)(self.target);
        let rec = self.recs[self.target as usize];
        let a = wrap(rec[1] - self.travel);
        let my = speed_of(self.vel);
        let os = speed_of(ov);
        let m = a.abs();
        let mut word = word;
        let steer;
        if std::f32::consts::FRAC_PI_2 < m {
            steer = steer_towards(self.skill, &self.frame, self.line);
            if 200.0 < rec[0] && os + 0.0 < my {
                word = brake(word);
            }
        } else {
            let aim = add(op, scale(normalize(ov), 300.0));
            if m < 0.872_664_63 {
                word = if 0.261_799_4 <= m { set_throttle(self.skill, self.vel, word) } else { set_tuck(self.skill, self.vel, word) };
            } else if my < os - 0.0 {
                word = set_throttle(self.skill, self.vel, word);
            } else if os + 0.0 < my {
                word = brake(word);
            }
            steer = steer_towards(self.skill, &self.frame, aim);
        }
        let word = steer_bits(word, steer);
        if (w.jump_marker)(&mut self.speed).is_some() {
            self.target = -1;
            self.mode = Mode::ToJump;
        } else {
            let mut rival = true;
            if !self.pick(w, &mut rival) {
                self.mode = Mode::Line;
            } else if !rival {
                self.mode = Mode::NearRider;
            }
        }
        word
    }
}

// ---- the course queries (board row G4e)

/// One event of an AI path as the game keeps it in memory: its type as the place in the event table (0x19 a
/// jump marker, file code 100; 0x1a a speed hint, 101; `tricky_data::aip::EVENT_TYPES`), value, start, end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathEvent {
    pub kind: u32,
    pub value: u32,
    pub start: f32,
    pub end: f32,
}

/// `cPath::vf0` 0x197910 (`cAIPath`'s query, vtable +0xc): the events overlapping `from..to` (none when `to` is
/// before `from`), in order, and in `mask` a bit for the type of each that begins after `from` or runs past `to`.
pub fn events_between(events: &[PathEvent], from: f32, to: f32, mask: &mut u32) -> Vec<usize> {
    if to < from {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (k, e) in events.iter().enumerate() {
        if e.start <= to && from <= e.end {
            if !(e.start <= from) || to < e.end {
                *mask |= 1 << (e.kind & 0x1f);
            }
            out.push(k);
        }
    }
    out
}

/// `AIComputer_FindJumpMarker` 0x1392a8: the first jump marker (type 0x19) on the rider's path from rider+0x340
/// (`from`) to 300 cm past rider+0x344 (`at`, its distance along). Sets the speed to ride at to (value >> 2) ×
/// 27.777777 cm/s (km/h) and gives [inside it already, value bit 1, value bit 0].
pub fn find_jump_marker(events: &[PathEvent], from: f32, at: f32, speed: &mut f32) -> Option<[i32; 3]> {
    let mut mask = 0;
    for k in events_between(events, from, at + 300.0, &mut mask) {
        let e = events[k];
        if e.kind == 0x19 {
            *speed = ((e.value as i32) >> 2) as f32 * 27.777_777;
            return Some([(e.start <= at) as i32, (e.value as i32) >> 1 & 1, (e.value & 1) as i32]);
        }
    }
    None
}

/// `AIComputer_FindSpeedHint` 0x1393e0: the first speed hint (type 0x1a) from rider+0x340 to 600 cm past the
/// rider: the speed to ride at value × 27.777777 cm/s (km/h), and whether the rider is inside it already.
pub fn find_speed_hint(events: &[PathEvent], from: f32, at: f32, speed: &mut f32) -> Option<i32> {
    let mut mask = 0;
    for k in events_between(events, from, at + 600.0, &mut mask) {
        let e = events[k];
        if e.kind == 0x1a {
            *speed = e.value as i32 as f32 * 27.777_777;
            return Some((e.start <= at) as i32);
        }
    }
    None
}

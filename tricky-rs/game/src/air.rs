//! The air step exactly as the original computes it (`Air_IntegrateRK4` 0x12b340): its constants, its units
//! (cm, cm/s, cm/s², the game's z up) and its order of operations on the vector unit, so it gives the same
//! floats, bit for bit, as the game's own code run in the function runner (board row F4d2). `Rider::step`
//! converts to and from its metres (y up).

/// Gravity rising and falling, cm/s² (0xc4548f73, 0xc4ed9ac0), and the horizontal drag (0xbe4cd34c).
pub const GRAVITY_RISING: f32 = -850.2414;
pub const GRAVITY_FALLING: f32 = -1900.8359;
const DRAG: f32 = -0.20002478;

/// The speed cap in the air, cm/s: the game writes it to rider+0x1c4 on every call (0x4551338e).
pub fn cap() -> f32 {
    f32::from_bits(0x4551_338e)
}

/// The acceleration at velocity `v`, built as the game builds it: (0, 0, g, 0) + (vx·drag, vy·drag, 0, 0).
fn accel(v: [f32; 4]) -> [f32; 4] {
    let g = if 0.0 < v[2] { GRAVITY_RISING } else { GRAVITY_FALLING };
    [0.0 + v[0] * DRAG, 0.0 + v[1] * DRAG, g + 0.0, 0.0 + 0.0]
}

fn add(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(a: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

/// `Air_IntegrateRK4` 0x12b340: steps position `pos` and velocity `vel` (four floats each, as the vector unit
/// holds them) through `dt` seconds with one classic Runge-Kutta step, then caps the speed at [`cap`].
///
/// With `substeps` (the game's fourth argument, set for its flight predictions) the time goes in steps of at
/// most 0.2 s; without it the whole `dt` is one step, but it is still repeated once for every 0.2 s of `dt`,
/// as the game's loop does (riding always passes a frame, so once).
pub fn integrate_rk4(dt: f32, pos: &mut [f32; 4], vel: &mut [f32; 4], substeps: bool) {
    let cap = cap();
    // 1/2, 1/6 and 1/3 come from vdiv (1 over 2.0, 6.0, 3.0)
    let (half, sixth, third) = (1.0f32 / 2.0, 1.0f32 / 6.0, 1.0f32 / 3.0);
    let mut left = dt;
    let mut h = dt;
    while 0.0 < left {
        if substeps {
            h = if 0.2 < left { 0.2 } else { left };
        }
        let (p, v) = (*pos, *vel);
        let k1p = scale(v, h);
        let k1v = scale(accel(v), h);
        let v2 = add(v, scale(k1v, half));
        let k2p = scale(v2, h);
        let k2v = scale(accel(v2), h);
        let v3 = add(v, scale(k2v, half));
        let k3p = scale(v3, h);
        let k3v = scale(accel(v3), h);
        let v4 = add(v, k3v);
        let k4p = scale(v4, h);
        let k4v = scale(accel(v4), h);
        let p1 = add(p, scale(add(k1p, k4p), sixth));
        let v1 = add(v, scale(add(k1v, k4v), sixth));
        *pos = add(p1, scale(add(k2p, k3p), third));
        *vel = add(v1, scale(add(k2v, k3v), third));
        // speed on the vector unit (0x12b9f0-0x12ba00): sqrt(((x² + y²) + 1·z²) + 1·w²)
        let sq = [vel[0] * vel[0], vel[1] * vel[1], vel[2] * vel[2], vel[3] * vel[3]];
        let speed = (((sq[0] + sq[1]) + 1.0 * sq[2]) + 1.0 * sq[3]).sqrt();
        if cap < speed {
            *vel = scale(*vel, cap / speed);
        }
        left -= 0.2;
    }
}

/// What the world ray (`cWorldLine`(0, from, to, 0), `World_RayCast` 0x25aff8) answers along one step of a
/// predicted flight: the fraction along it (negative: nothing hit), and the hit's point, normal and surface word
/// (the hit record's +0x10, +0x20, +0x34).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RayHit {
    pub point: [f32; 4],
    pub normal: [f32; 4],
    pub word: u32,
}

/// The flight prediction (`cAirPredict`, rider+0x57b0): +8 the time it reached (the landing time), +0x10 the
/// landing point, +0x20 the velocity there, +0x30 the ground's normal there, +0x40 its surface word (−1 none),
/// +0x44 the landing time again, +0x48 the time to the top of the arc, +0x4c (0), +0x50 the top's time when a
/// landing was found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Predict {
    pub t: f32,
    pub point: [f32; 4],
    pub vel: [f32; 4],
    pub normal: [f32; 4],
    pub word: u32,
    pub t_land: f32,
    pub t_top: f32,
    pub x4c: f32,
    pub x50: f32,
}

impl Predict {
    /// `AirPredict_Tick` 0x123ec8: +0x4c += 1/60; +8 = +0x44 − it, +0x50 = +0x48 − it.
    pub fn tick(&mut self) {
        self.x4c += 0.016666668;
        self.t = self.t_land - self.x4c;
        self.x50 = self.t_top - self.x4c;
    }
}

/// `AirPredict_AtTakeoff` 0x123b10, from the rider's position (+0x140), velocity (+0x150) and normal (+0x2a0):
/// - rising: the time to the top by halving (from 2 s between 0 and 4, whole flights of [`integrate_rk4`] without
///   substeps; at most 10 tries, or until the bracket is under 0.01);
/// - then the flight from 0.13333334 s on, in steps growing ×1.28 from 1/60 s (25 at most), each step's segment
///   (stretched 2% at both ends) cast against the world (`ray`); the first hit gives the landing: the velocity and
///   time interpolated to the hit's fraction, its point, normal and word. No hit: the rider's own position,
///   velocity and normal, word −1, times 0.
pub fn predict_at_takeoff(pos: [f32; 4], vel: [f32; 4], normal: [f32; 4], ray: &mut dyn FnMut([f32; 4], [f32; 4]) -> (f32, RayHit)) -> Predict {
    let mut pr = Predict { normal: [0.0, 0.0, 1.0, 0.0], ..Default::default() };
    if 0.0 < vel[2] {
        let (mut hi, mut lo, mut t) = (4.0f32, 0.0f32, 2.0f32);
        let mut tries = 0;
        loop {
            let (mut p, mut v) = (pos, vel);
            integrate_rk4(t, &mut p, &mut v, false);
            let mut m = t;
            if v[2] < 0.0 {
                hi = t;
                m = lo;
            }
            tries += 1;
            if 9 < tries {
                pr.t_top = t;
                break;
            }
            t = (hi + m) * 0.5;
            if hi - m < 0.01 {
                pr.t_top = t;
                break;
            }
            lo = m;
        }
    }
    let (mut p, mut v) = (pos, vel);
    pr.t = f32::from_bits(0x3e08_8889);
    integrate_rk4(pr.t, &mut p, &mut v, true);
    let mut mult = 1.0f32;
    let tick = f32::from_bits(0x3c88_8889);
    let stretch = f32::from_bits(0x3ca3_d70a);
    for _ in 0..0x19 {
        let step = mult * tick;
        pr.t += step;
        let (p0, v0) = (p, v);
        integrate_rk4(step, &mut p, &mut v, true);
        let d = [p[0] - p0[0], p[1] - p0[1], p[2] - p0[2], p[3] - p0[3]];
        let e = scale(d, stretch);
        let from = [p0[0] - e[0], p0[1] - e[1], p0[2] - e[2], p0[3] - e[3]];
        let to = add(p, e);
        let (h, hit) = ray(from, to);
        if 0.0 <= h {
            let rest = 1.0 - h;
            pr.vel = add(scale(v, h), scale(v0, rest));
            pr.x50 = pr.t_top;
            let t = pr.t - step * rest;
            pr.point = hit.point;
            pr.normal = hit.normal;
            pr.word = hit.word;
            pr.t_land = t;
            pr.t = t;
            return pr;
        }
        mult *= f32::from_bits(0x3fa3_d70a);
    }
    pr.t = 0.0;
    pr.point = pos;
    pr.vel = vel;
    pr.normal = normal;
    pr.word = 0xffff_ffff;
    pr.t_land = 0.0;
    pr.x50 = 0.0;
    pr
}

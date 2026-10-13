//! The carve trail behind the board (board row G7f): `Fx_BoardTrail_Update` 0x12fef0 with
//! `Fx_BoardTrail_Push` 0x130960 and `Fx_BoardTrail_SetPoint` 0x1306b0. The object is words: +0 whether a trail
//! is open, +4 how many segments it has made, +0x10 the last point, +0x20 the newest centre, +0x30 a random
//! walk (±0.3), +0x34 the bank, +0x38 the strength, +0x40 the heading, +0x50 / +0x54 how many segments and the
//! newest of 75 (0xa0 bytes from +0x60: four points, four velocities, the colour, +0x90 a phase, +0x94 the age,
//! +0x9c a flag).

pub const BYTES: usize = 0x2f40;

type V4 = [f32; 4];

#[derive(Clone, Debug, PartialEq)]
pub struct Trail {
    pub w: Vec<u32>,
}

impl Default for Trail {
    fn default() -> Self {
        Trail { w: vec![0; BYTES / 4] }
    }
}

impl Trail {
    fn f(&self, b: usize) -> f32 {
        f32::from_bits(self.w[b / 4])
    }
    fn set(&mut self, b: usize, v: f32) {
        self.w[b / 4] = v.to_bits();
    }
    fn v(&self, b: usize) -> V4 {
        [0, 1, 2, 3].map(|k| self.f(b + 4 * k))
    }
    fn setv(&mut self, b: usize, v: V4) {
        for k in 0..4 {
            self.set(b + 4 * k, v[k]);
        }
    }
    fn i(&self, b: usize) -> i32 {
        self.w[b / 4] as i32
    }
}

/// What the trail reads of the rider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailIn {
    pub motion: u32,
    pub hit: u32,
    /// +0x1fc, +0x214
    pub f1fc: f32,
    pub f214: f32,
    /// +0x1b4
    pub flipped: u32,
    /// +0x4a60
    pub w4a60: V4,
    /// +0x2a0, +0x150, +0x2d0
    pub up: V4,
    pub vel: V4,
    pub ground: V4,
    pub colour: V4,
    /// the floats at +0x58, +0x5c, +0x60 of the rider's record in the surface table
    pub cfg: [f32; 3],
}

fn dot4(a: V4, b: V4) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

/// The PS2 never makes an infinity: an overflow stops at the largest float.
fn cl(x: f32) -> f32 {
    if x.is_infinite() {
        f32::MAX.copysign(x)
    } else {
        x
    }
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| cl(x * k))
}

fn add(a: V4, b: V4) -> V4 {
    [cl(a[0] + b[0]), cl(a[1] + b[1]), cl(a[2] + b[2]), cl(a[3] + b[3])]
}

fn sub(a: V4, b: V4) -> V4 {
    [cl(a[0] - b[0]), cl(a[1] - b[1]), cl(a[2] - b[2]), cl(a[3] - b[3])]
}

fn unit(v: V4) -> V4 {
    let s = dot4(v, v);
    scale(v, if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() })
}

fn cross(a: V4, b: V4) -> V4 {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], 0.0]
}

fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    a / b
}

fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if 1.0 < x {
        1.0
    } else {
        x
    }
}

const TICK: f32 = f32::from_bits(0x3c88_8889);
const SEG: usize = 0xa0;

fn seg(t: &Trail, n: i32) -> usize {
    0x60 + SEG * ((t.i(0x54) + n).rem_euclid(0x4b)) as usize
}

/// `Fx_BoardTrail_Push` 0x130960(this, colour, flag): a new segment at the front (the oldest goes when 75), the
/// walk stepped, the colour toward grey by 0.8.
fn push(t: &mut Trail, colour: V4, flag: u32, rand_fx: &mut dyn FnMut() -> u32) {
    let r = rand_fx();
    let f = f32::from_bits((r & 0x7f_ffff) | 0x3f80_0000);
    let step = (f - 1.0) * f32::from_bits(0x3e88_8889) + f32::from_bits(0xbe08_8889);
    let w = t.f(0x30) + step;
    t.set(0x30, w);
    if f32::from_bits(0x3e99_999a) < w {
        t.set(0x30, f32::from_bits(0x3f19_999a) - w);
    }
    let w = t.f(0x30);
    if w < f32::from_bits(0xbe99_999a) {
        t.set(0x30, f32::from_bits(0xbf19_999a) - w);
    }
    let count = t.i(0x50);
    if count < 0x4b {
        t.w[0x50 / 4] = (count + 1) as u32;
    }
    let start = (t.i(0x54) + 0x4a).rem_euclid(0x4b);
    t.w[0x54 / 4] = start as u32;
    let b = 0x60 + SEG * start as usize;
    t.setv(b + 0x80, colour);
    t.w[(b + 0x9c) / 4] = flag;
    let n = t.i(4);
    let f4 = n as f32;
    let (c1, c2, c3) = (colour[1], colour[2], colour[3]);
    let mean = ((c1 + c2) + c3) * f32::from_bits(0x3eaa_aaab);
    let f4 = f4 * 0.25f32;
    let k = f4 * f32::from_bits(0x3c80_0000);
    let fl = (k as i32) as f32;
    let d3 = (mean - c3) * f32::from_bits(0x3f4c_cccd);
    let d1 = (mean - c1) * f32::from_bits(0x3f4c_cccd);
    let d2 = (mean - c2) * f32::from_bits(0x3f4c_cccd);
    let phase = f4 - (fl * 64.0f32 + 32.0f32);
    t.w[4 / 4] = (n + 1) as u32;
    t.set(b + 0x84, c1 + d1);
    t.set(b + 0x88, c2 + d2);
    t.set(b + 0x8c, c3 + d3);
    t.set(b + 0x90, phase);
}

/// `Fx_BoardTrail_SetPoint` 0x1306b0(size, alpha, this, pos, side, dir, vel): fills the newest segment: its alpha
/// and colour (clamped to 0..1), four velocities along the heading, four points across the track.
fn set_point(t: &mut Trail, size: f32, alpha: f32, pos: V4, side: V4, dir: V4, vel: V4) {
    let walk = t.f(0x30);
    let k1 = walk + 1.0;
    let k2 = walk * f32::from_bits(0x3f99_999a) + 1.0;
    let b = seg(t, 0);
    let alpha = alpha * k2;
    t.w[(b + 0x94) / 4] = 0;
    t.set(b + 0x80, alpha);
    let age = t.f(b + 0x94);
    let _ = age;
    let c = t.v(b + 0x80);
    let fix = [clamp01(c[1]), clamp01(c[2]), clamp01(c[3]), clamp01(c[0])];
    t.set(b + 0x84, fix[0]);
    t.set(b + 0x88, fix[1]);
    t.set(b + 0x8c, fix[2]);
    t.set(b + 0x80, fix[3]);
    let f22 = size * k1;
    let speed = dot4(vel, vel).sqrt();
    let f20 = ps2_div(100.0, speed);
    let mut sp = [0.0; 4];
    for i in 0..4 {
        let k = (i as f32 * 0.25f32) * f22;
        let along = scale(dir, k);
        t.setv(b + 0x40 + 16 * i, along);
        let across = scale(side, 50.0);
        let base = sub(pos, across);
        let moved = scale(along, f20);
        sp = add(base, moved);
        t.setv(b + 16 * i, sp);
    }
    let _ = sp;
    let across = scale(side, 50.0);
    t.setv(0x20, add(pos, across));
}

/// `Fx_BoardTrail_Update` 0x12fef0.
pub fn update(t: &mut Trail, r: &TrailIn, rand_fx: &mut dyn FnMut() -> u32) {
    // age the segments and move their points
    let count = t.i(0x50);
    if count > 0 {
        let mut i = 0i32;
        let b = seg(t, 0);
        let a = t.f(b + 0x94) + TICK;
        t.set(b + 0x94, a);
        if f32::from_bits(0x3f66_6666) < a {
            t.w[0x50 / 4] = 0;
        } else {
            loop {
                let b = seg(t, i);
                for k in 0..4usize {
                    let kk = k as f32 * 0.25f32;
                    let acc = add(scale(t.v(b + 0x40 + 16 * k), f32::from_bits(0xc060_0000)), [0.0, 0.0, (kk * f32::from_bits(0xc52f_0000)) * kk, 0.0]);
                    let dp = scale(t.v(b + 0x40 + 16 * k), TICK);
                    let p = add(t.v(b + 16 * k), dp);
                    t.setv(b + 16 * k, p);
                    let dv = scale(acc, TICK);
                    let v = add(t.v(b + 0x40 + 16 * k), dv);
                    t.setv(b + 0x40 + 16 * k, v);
                }
                if !(i + 1 < t.i(0x50)) {
                    break;
                }
                let nb = seg(t, i + 1);
                let a = t.f(nb + 0x94) + TICK;
                t.set(nb + 0x94, a);
                if !(a <= f32::from_bits(0x3f66_6666)) {
                    t.w[0x50 / 4] = (i + 1) as u32;
                    break;
                }
                i += 1;
            }
        }
    }
    let mut f20 = r.f214 * (1.0 - r.f1fc.abs());
    if r.flipped != 0 {
        f20 = -f20;
    }
    let w60 = r.w4a60;
    let side0 = cross(w60, r.up);
    let mut f12 = r.cfg[1];
    if !(0.0 < f20) {
        f12 = -f12;
    }
    let sum = add(side0, scale(r.up, f12));
    let speed = dot4(r.vel, r.vel).sqrt();
    let f5 = f20.abs();
    let dirn = unit(sum);
    let ground = r.ground;
    let mut f21 = r.cfg[0] * (f5 - f32::from_bits(0x3e4c_cccd));
    let f4 = (r.cfg[2] * speed) * f20;
    if f21 < 0.0 {
        f21 = 0.0;
    }
    let side = |flip: u32| if flip != 0 { scale(w60, -1.0) } else { w60 };
    if t.i(0) == 0 {
        if r.motion != 2 || !(f32::from_bits(0x3e99_999a) < f5) || r.hit == 2 {
            return;
        }
        t.setv(0x40, dirn);
        t.set(0x34, 0.0);
        t.set(0x38, f21);
        t.w[4 / 4] = 0;
        push(t, r.colour, 1, rand_fx);
        let s = side(r.flipped);
        let d = t.v(0x40);
        set_point(t, t.f(0x34), 0.0, ground, s, d, r.vel);
        t.setv(0x10, ground);
        push(t, r.colour, 0, rand_fx);
        let s = side(r.flipped);
        let d = t.v(0x40);
        set_point(t, t.f(0x34), t.f(0x38), ground, s, d, r.vel);
        t.w[0] = 1;
        return;
    }
    let f3 = t.f(0x34);
    let same = if f3 == 0.0 { true } else { (f4 < 0.0) == (f3 < 0.0) };
    if !same {
        t.w[0] = 0;
        let s = side(r.flipped);
        let f1 = t.f(0x34);
        let d = t.v(0x40);
        set_point(t, t.f(0x30) * f1 + f1, 0.0, ground, s, d, r.vel);
        return;
    }
    let f1 = f4 * f32::from_bits(0x3d08_8889);
    let f2 = f3 * f32::from_bits(0x3f77_7777);
    let third = scale(dirn, f32::from_bits(0x3e2a_aaab));
    t.set(0x34, f1 + f2);
    let old = scale(t.v(0x40), f32::from_bits(0x3f55_5555));
    t.setv(0x40, unit(add(third, old)));
    if !(r.motion == 2 && r.hit != 2) {
        f21 = 0.0;
    }
    let str_ = t.f(0x38);
    let f0 = f21 * f32::from_bits(0x3eaa_aaab) + str_ * f32::from_bits(0x3f2a_aaaa);
    t.set(0x38, f0);
    if f32::from_bits(0x3ca3_d70a) < f0 {
        let d = sub(ground, t.v(0x10));
        let len = dot4(d, d).sqrt();
        if f32::from_bits(0x41c8_0000) < len {
            push(t, r.colour, 0, rand_fx);
            t.setv(0x10, ground);
        }
        let s = side(r.flipped);
        let dd = t.v(0x40);
        set_point(t, t.f(0x34), t.f(0x38), ground, s, dd, r.vel);
    } else {
        t.w[0] = 0;
        let s = side(r.flipped);
        let dd = t.v(0x40);
        set_point(t, t.f(0x34), 0.0, ground, s, dd, r.vel);
    }
}

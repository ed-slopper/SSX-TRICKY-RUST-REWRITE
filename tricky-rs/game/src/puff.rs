//! The board's puffs and the speed dust (board row G7f): `Fx_BoardPuff_Update` 0x12eca8 and
//! `Fx_SpeedDust_Update` 0x12f4c0. Each is a ring of 64 particles in a rider's effect block, aged every tick
//! (1/60) and dropped from the old end when too old; a new one is made when the board has moved far enough on the
//! right ground. The objects are words, as the game has them.

type V4 = [f32; 4];

fn f(w: &[u32], byte: usize) -> f32 {
    f32::from_bits(w[byte / 4])
}

fn set(w: &mut [u32], byte: usize, v: f32) {
    w[byte / 4] = v.to_bits();
}

fn set4(w: &mut [u32], byte: usize, v: V4) {
    for k in 0..4 {
        set(w, byte + 4 * k, v[k]);
    }
}

fn dot4(a: V4, b: V4) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

fn clamp(x: f32, hi: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if hi < x {
        hi
    } else {
        x
    }
}

fn unit_one(r: u32) -> f32 {
    f32::from_bits((r & 0x7f_ffff) | 0x3f80_0000)
}

/// The colour toward its grey by half: (c0 → 0, c1, c2, c3).
fn wash(c: V4, hi: f32) -> V4 {
    let mean = ((c[1] + c[2]) + c[3]) * f32::from_bits(0x3eaa_aaab);
    [0.0, clamp(c[1] + (mean - c[1]) * 0.5, hi), clamp(c[2] + (mean - c[2]) * 0.5, hi), clamp(c[3] + (mean - c[3]) * 0.5, hi)]
}

/// The ring's two words (count, start) at `at`: ages every particle (`stride` bytes from `first`, the age the first
/// float) newest to oldest by 1/60 until one is older than `limit`, which drops it and all older.
fn age(w: &mut [u32], at: usize, first: usize, stride: usize, limit: f32) {
    let count = w[at / 4] as i32;
    let start = w[at / 4 + 1] as i32;
    let mut i = count - 1;
    while i >= 0 {
        let sum = start + i;
        let idx = sum.rem_euclid(64) as usize;
        let o = first + stride * idx;
        let a = f(w, o) + f32::from_bits(0x3c88_8889);
        set(w, o, a);
        if limit < a {
            w[at / 4] = (count - 1 - i) as u32;
            w[at / 4 + 1] = (sum + 1).rem_euclid(64) as u32;
            break;
        }
        i -= 1;
    }
}

/// The slot for a new particle: drops the oldest when the ring is full.
fn take(w: &mut [u32], at: usize) -> usize {
    let mut count = w[at / 4] as i32;
    if count == 0x40 {
        count = 0x3f;
        w[at / 4] = 0x3f;
        w[at / 4 + 1] = ((w[at / 4 + 1] as i32 + 1).rem_euclid(64)) as u32;
    }
    let start = w[at / 4 + 1] as i32;
    w[at / 4] = (count + 1) as u32;
    (start + count).rem_euclid(64) as usize
}

pub const PUFF_BYTES: usize = 0x1c18;

#[derive(Clone, Debug, PartialEq)]
pub struct Puffs {
    pub w: Vec<u32>,
}

impl Default for Puffs {
    fn default() -> Self {
        Puffs { w: vec![0; PUFF_BYTES / 4] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PuffIn {
    pub hit: u32,
    pub motion: u32,
    pub surface: u32,
    /// +0x214
    pub turn: f32,
    /// +0x4a90
    pub at: V4,
    /// +0x1a0
    pub axis: V4,
    /// the colour at +0x470 of the light set
    pub colour: V4,
}

/// `Fx_BoardPuff_Update` 0x12eca8.
pub fn puff_update(p: &mut Puffs, r: &PuffIn, rand_fx: &mut dyn FnMut() -> u32) {
    let w = &mut p.w[..];
    age(w, 0x1c10, 0x10, 0x70, 2.0);
    let soft = r.surface.wrapping_sub(3) < 2;
    if r.hit == 2 || r.motion != 2 {
        return;
    }
    // surfaces 3 and 4 puff always; the others only on a hard turn
    if !soft && !(0.2 < r.turn.abs()) {
        return;
    }
    let d = [r.at[0] - f(w, 0), r.at[1] - f(w, 4), r.at[2] - f(w, 8), r.at[3] - f(w, 12)];
    let len = dot4(d, d).sqrt();
    if !(70.0 < len) {
        return;
    }
    let size = if soft { 1.0 } else { r.turn.abs() };
    let slot = take(w, 0x1c10);
    let o = 0x10 + 0x70 * slot;
    let r1 = rand_fx();
    let r2 = rand_fx();
    set(w, o, 0.0);
    set4(w, o + 0x20, r.at);
    set4(w, o + 0x30, r.axis);
    set(w, o + 4, (r1 & 1) as f32);
    set(w, o + 8, (r2 & 1) as f32);
    set(w, o + 0xc, if r1 & 1 == 0 { 1.0 } else { 0.0 });
    set(w, o + 0x10, if r2 & 1 == 0 { 1.0 } else { 0.0 });
    w[(o + 0x40) / 4] = rand_fx() & 3;
    set(w, o + 0x44, size * 0.2f32);
    set4(w, o + 0x48, wash(r.colour, 1.0));
    set(w, o + 0x58, if soft { 0.0 } else { f32::from_bits(0x3e4c_cccd) });
    set(w, o + 0x5c, 2.0);
    set(w, o + 0x60, 80.0);
    set(w, o + 0x64, 400.0);
    set4(w, 0, r.at);
}

pub const DUST_BYTES: usize = 0xd58;

#[derive(Clone, Debug, PartialEq)]
pub struct Dust {
    pub w: Vec<u32>,
}

impl Default for Dust {
    fn default() -> Self {
        Dust { w: vec![0; DUST_BYTES / 4] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DustIn {
    pub hit: u32,
    pub motion: u32,
    pub surface: u32,
    /// +0x2d0 (motion 2) or +0x140
    pub ground: V4,
    pub other: V4,
    /// +0x150
    pub vel: V4,
    /// +0x2a0
    pub up: V4,
    pub colour: V4,
}

/// `Fx_SpeedDust_Update` 0x12f4c0.
pub fn dust_update(d: &mut Dust, r: &DustIn, rand_fx: &mut dyn FnMut() -> u32) {
    let w = &mut d.w[..];
    age(w, 0xd50, 0x50, 0x34, 1.0);
    let c = wash(r.colour, 0.7);
    set4(w, 0x40, c);
    set4(w, 0, if r.motion == 2 { r.ground } else { r.other });
    let k = dot4(r.vel, r.up);
    let along = r.up.map(|x| x * k);
    let flat = [r.vel[0] - along[0], r.vel[1] - along[1], r.vel[2] - along[2], r.vel[3] - along[3]];
    let s = dot4(flat, flat);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    let dir = flat.map(|x| x * q);
    set4(w, 0x10, dir);
    let (a, b) = (dir, r.up);
    set4(w, 0x20, [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], 0.0]);
    set4(w, 0x30, r.up);
    if r.hit == 2 || r.motion != 2 || !(r.surface.wrapping_sub(3) < 2) {
        return;
    }
    let speed = dot4(r.vel, r.vel).sqrt();
    let mut n = (speed / 1270.6951f32 + 0.5) as i32;
    if n < 1 {
        n = 1;
    }
    while n > 0 {
        let slot = take(w, 0xd50);
        let o = 0x50 + 0x34 * slot;
        let r1 = rand_fx();
        let r2 = rand_fx();
        set(w, o, 0.0);
        set(w, o + 0x14, (unit_one(rand_fx()) - 1.0) * 88.33769f32 + -44.168846f32);
        set(w, o + 0x18, (unit_one(rand_fx()) - 1.0) * 88.33769f32 + -44.168846f32);
        set(w, o + 4, (r1 & 1) as f32);
        set(w, o + 8, (r2 & 1) as f32);
        set(w, o + 0xc, if r1 & 1 == 0 { 1.0 } else { 0.0 });
        set(w, o + 0x10, if r2 & 1 == 0 { 1.0 } else { 0.0 });
        w[(o + 0x1c) / 4] = rand_fx() & 3;
        set(w, o + 0x24, 0.0);
        w[(o + 0x20) / 4] = 0x3e2e_0c4d;
        let mut big = ((unit_one(rand_fx()) - 1.0) * 0.40000004f32 + 0.8f32) * 0.44652596f32;
        if speed / 972.22217f32 <= 1.0 {
            big *= speed / 972.22217f32;
        }
        set(w, o + 0x28, big);
        set(w, o + 0x2c, ((unit_one(rand_fx()) - 1.0) * 0.40000004f32 + 0.8f32) * 18.493296f32);
        set(w, o + 0x30, ((unit_one(rand_fx()) - 1.0) * 0.40000004f32 + 0.8f32) * 113.950874f32);
        n -= 1;
    }
}

//! The particle emitter's setters (board row G7f6): `ParticleSys_InitEmitter` 0x1d50b0, `_SetSizeRange` 0x1d52e8,
//! `_SetLifeRange` 0x1d5300, `_SetSpawnLine` 0x1d5380, `_SetSpawnQuad` 0x1d5418, `_ClearForce` 0x1d5548,
//! `_SetVelocityBasis` 0x1d5790, `_SetColors` 0x1d58e0, `_SetTime` 0x1d59e0 and `_AdvanceTime` 0x1d59f0. An emitter is
//! 0x140 bytes of words (the functions take it as a word slice); +0x3c is its time scale. `_RandomizeFactors`
//! 0x1d5180 is `fxrand::emitter_factors`.

pub type V4 = [f32; 4];

/// The PS2 never makes an infinity: an overflow stops at the largest float.
pub fn cl(x: f32) -> f32 {
    if x.is_infinite() {
        f32::MAX.copysign(x)
    } else {
        x
    }
}

pub fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| cl(x * k))
}

pub fn add(a: V4, b: V4) -> V4 {
    [cl(a[0] + b[0]), cl(a[1] + b[1]), cl(a[2] + b[2]), cl(a[3] + b[3])]
}

pub fn sub(a: V4, b: V4) -> V4 {
    [cl(a[0] - b[0]), cl(a[1] - b[1]), cl(a[2] - b[2]), cl(a[3] - b[3])]
}

/// The vector unit's four-lane dot: `((a0b0 + a1b1) + 1·a2b2) + 1·a3b3`.
pub fn dot4(a: V4, b: V4) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

/// A division that, like the PS2's, answers the largest float (signed) for a zero divisor.
pub fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    cl(a / b)
}

fn f(e: &[u32], b: usize) -> f32 {
    f32::from_bits(e[b / 4])
}

fn set(e: &mut [u32], b: usize, v: f32) {
    e[b / 4] = v.to_bits();
}

fn v(e: &[u32], b: usize) -> V4 {
    [0, 1, 2, 3].map(|k| f(e, b + 4 * k))
}

fn setv(e: &mut [u32], b: usize, x: V4) {
    for k in 0..4 {
        set(e, b + 4 * k, x[k]);
    }
}

/// `ParticleSys_InitEmitter` 0x1d50b0(f12 = rate, f13, f14 = time scale, e, n, a2, a3 = a vector).
#[allow(clippy::too_many_arguments)]
pub fn init(e: &mut [u32], f12: f32, f13: f32, f14: f32, n: i32, a2: i32, a3: V4) {
    let f12 = f12 * f14;
    let f13 = f13 * f14;
    e[0] = n as u32;
    set(e, 0xc, ps2_div(f12, n as f32));
    set(e, 0x20, ps2_div(1.0, a2 as f32));
    let q = ps2_div(1.0, f14);
    let v1 = scale(a3, q);
    set(e, 0x24, f13);
    set(e, 0x3c, f14);
    let v2 = scale(v1, q);
    setv(e, 0x50, v2);
    e[4 / 4] = a2 as u32;
    setv(e, 0x120, [0.0; 4]);
    setv(e, 0x130, [1.0, 1.0, 0.0, 0.0]);
}

/// `ParticleSys_SetSizeRange` 0x1d52e8.
pub fn size_range(e: &mut [u32], min: f32, max: f32) {
    let d = max - min;
    set(e, 0x10, d);
    set(e, 0x18, min - d);
}

/// `ParticleSys_SetLifeRange` 0x1d5300.
pub fn life_range(e: &mut [u32], min: f32, max: f32) {
    let d = max - min;
    let k = f(e, 0x3c);
    let lo = min * k;
    let d = d * k;
    set(e, 0x14, d);
    set(e, 0x1c, lo - d);
}

/// `ParticleSys_SetSpawnLine` 0x1d5380(e, p0, p1).
pub fn spawn_line(e: &mut [u32], p0: V4, p1: V4) {
    setv(e, 0xa0, sub(scale(p0, 2.0), p1));
    setv(e, 0xb0, sub(p1, p0));
    setv(e, 0xc0, [0.0; 4]);
}

/// `ParticleSys_SetSpawnQuad` 0x1d5418(e, p, a, b).
pub fn spawn_quad(e: &mut [u32], p: V4, a: V4, b: V4) {
    let s = scale(add(a, b), 3.0);
    setv(e, 0xa0, sub(p, s));
    setv(e, 0xb0, scale(a, 2.0));
    setv(e, 0xc0, scale(b, 2.0));
}

/// `ParticleSys_ClearForce` 0x1d5548.
pub fn clear_force(e: &mut [u32]) {
    setv(e, 0xd0, [0.0; 4]);
}

/// `ParticleSys_SetVelocityBasis` 0x1d5790(e, v, u1, u2, u3).
pub fn velocity_basis(e: &mut [u32], vel: V4, u1: V4, u2: V4, u3: V4) {
    let q = ps2_div(1.0, f(e, 0x3c));
    let a = scale(vel, q);
    setv(e, 0x60, a);
    setv(e, 0x70, scale(u1, q));
    setv(e, 0x80, scale(u2, q));
    setv(e, 0x90, scale(u3, q));
    let sum = add(add(v(e, 0x70), v(e, 0x80)), v(e, 0x90));
    setv(e, 0x60, sub(a, scale(sum, 1.5)));
}

/// `ParticleSys_SetColors` 0x1d58e0(f12 = life, e, c0, c1, c2, c3).
pub fn colors(e: &mut [u32], life: f32, c0: V4, c1: V4, c2: V4, c3: V4) {
    let g = scale(add(c2, c3), 1.5);
    setv(e, 0xe0, sub(c0, g));
    let life = life * f(e, 0x3c);
    let d = sub(c1, c0);
    let q = ps2_div(1.0, life);
    setv(e, 0x110, scale(d, q));
    setv(e, 0xf0, c2);
    setv(e, 0x100, c3);
}

/// `ParticleSys_SetTime` 0x1d59e0.
pub fn set_time(e: &mut [u32], t: f32) {
    let k = f(e, 0x3c);
    set(e, 0x8, t * k);
}

/// `ParticleSys_AdvanceTime` 0x1d59f0.
pub fn advance_time(e: &mut [u32], dt: f32) {
    let k = f(e, 0x3c);
    let t = f(e, 0x8);
    set(e, 0x8, t + dt * k);
}

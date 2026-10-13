//! The brake spray's per-tick update (board row G7f): `Fx_BrakeSpray_Update` 0x12e6e8 over the spray pool of
//! [`crate::brakespray`] with the fan beside it (`Fx_BrakeFan_Update` 0x12d108 and `Fx_BrakeFan_Emit` 0x12c4d0
//! over the rings of [`crate::splash`], the splash object at +0x970 of the pool).

use crate::brakespray::{spawn, Pool, SprayEnv};
use crate::splash::{Splash, RING_WORDS};

type V4 = [f32; 4];

fn dot4(a: V4, b: V4) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| x * k)
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    a / b
}

fn unit(v: V4) -> V4 {
    let s = dot4(v, v);
    scale(v, if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() })
}

fn cross(a: V4, b: V4) -> V4 {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], 0.0]
}

fn put4(ring: &mut [u32; RING_WORDS], at: usize, v: V4) {
    for k in 0..4 {
        ring[at + k] = v[k].to_bits();
    }
}

fn get4(ring: &[u32; RING_WORDS], at: usize) -> V4 {
    [0, 1, 2, 3].map(|k| f32::from_bits(ring[at + k]))
}

/// `Fx_BrakeFan_Emit` 0x12c4d0(size, k_width, k_fade... (f12..f15), ring, lift, along, side, across): the
/// fan's points in a ring (`ring[237]` of them), swung through ±(0.52..0.96) rad round `across`... with a random
/// walk of the width.
pub fn fan_emit(
    ring: &mut [u32; RING_WORDS],
    f12: f32,
    f13: f32,
    f14: f32,
    f15: f32,
    lift: V4,
    along: V4,
    side: V4,
    across: V4,
    rand_fx: &mut dyn FnMut() -> u32,
) {
    let one = |r: u32| f32::from_bits((r & 0x7f_ffff) | 0x3f80_0000);
    let spread = (one(rand_fx()) - 1.0) * f32::from_bits(0x3edf_66f2) + f32::from_bits(0x3f06_0a92);
    let n = ring[237] as i32;
    let mut walk = 1.0f32;
    for i in 0..n.max(0) {
        let f = (one(rand_fx()) - 1.0) * 0.6f32 + -0.3f32;
        walk += f;
        if walk < 0.8f32 {
            walk = 1.6f32 - walk;
        }
        if 1.2f32 < walk {
            walk = 2.4f32 - walk;
        }
        let t = ps2_div(i as f32, n as f32 - 1.0);
        let t = (t + t) - 1.0;
        let t2 = t * t;
        let (s, c) = crate::math::sin_cos(t * spread);
        let fade = 1.0 - t2 * t2;
        let sum = add(scale(along, c), scale(side, s));
        let a = scale(scale(across, f13), t);
        ring[4 * i as usize..4 * i as usize + 4].copy_from_slice(&a.map(f32::to_bits));
        let wide = (walk * f12) * fade;
        let b = scale(add(sum, scale(lift, f15)), wide);
        put4(ring, 100 + 4 * i as usize, b);
        ring[208 + i as usize] = ((f14 * walk) * fade).to_bits();
    }
}

/// What the fan and the spray read of the rider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FanIn {
    pub hit: u32,
    pub motion: u32,
    /// +0x1fc
    pub f1fc: f32,
    /// the float at +0x58 of the rider's record in the surface table
    pub cfg58: f32,
    /// +0x150
    pub vel: V4,
    /// +0x2a0
    pub up: V4,
    /// +0x2d0
    pub ground: V4,
    /// +0x4a60
    pub w4a60: V4,
    pub colour: V4,
}

const TICK: f32 = f32::from_bits(0x3c88_8889);

/// `Fx_BrakeFan_Update` 0x12d108.
pub fn fan_update(s: &mut Splash, r: &FanIn, rand_fx: &mut dyn FnMut() -> u32) {
    s.gathered *= f32::from_bits(0x3f2a_aaaa);
    let zero = 0.0f32;
    let mut n = s.count;
    if r.hit == 2 {
        s.timer = 0.0;
    } else if 0.7 < r.f1fc.abs() {
        if r.motion == 2 {
            if s.timer <= 0.0 || r.cfg58 < f32::from_bits(0x3ca3_d70a) {
                let speed = dot4(r.vel, r.vel).sqrt();
                let mut size = speed * f32::from_bits(0x3f0a_27e6);
                if size <= f32::from_bits(0x428f_e001) {
                    n = s.count;
                } else {
                    if f32::from_bits(0x4418_15ed) < size {
                        size = f32::from_bits(0x4418_15ed);
                    }
                    if s.count < 5 {
                        s.count += 1;
                    }
                    s.slot = (s.slot + 4) % 5;
                    let ring = &mut s.rings[s.slot as usize];
                    let along = scale(r.up, dot4(r.vel, r.up));
                    let flat = sub(r.vel, along);
                    let w = r.w4a60;
                    let wperp = sub(w, scale(r.up, dot4(w, r.up)));
                    let wu = unit(wperp);
                    let fu = unit(flat);
                    let mut side = cross(fu, r.up);
                    if dot4(wu, side) < zero {
                        side = scale(side, -1.0);
                    }
                    ring[238] = zero.to_bits();
                    ring[237] = 0xc;
                    ring[239] = ((size * f32::from_bits(0x3aac_5db5) + 0.2f32) * f32::from_bits(0x3f7e_b7a0)).to_bits();
                    put4(ring, 233, r.colour);
                    put4(ring, 204, flat);
                    put4(ring, 200, r.ground);
                    fan_emit(ring, size, 80.0, r.cfg58 * 0.4f32, f32::from_bits(0x3ecc_cccd), r.up, fu, side, wu, rand_fx);
                    let one = f32::from_bits((rand_fx() & 0x7f_ffff) | 0x3f80_0000);
                    s.timer = s.timer + ((1.0 - size * f32::from_bits(0x3a81_4648)) * 0.25f32 + ((one - 1.0) * 0.08f32 + zero));
                    n = s.count;
                }
            } else {
                s.timer -= TICK;
                n = s.count;
            }
        } else {
            s.timer = 0.0;
            n = s.count;
        }
    } else {
        s.timer = 0.0;
        n = s.count;
    }
    if n <= 0 {
        return;
    }
    let max = f32::from_bits(0x3f7e_b7a0);
    let mut i = 0i32;
    {
        let ring = &mut s.rings[(s.slot % 5) as usize];
        let a = f32::from_bits(ring[238]) + TICK;
        ring[238] = a.to_bits();
        if max < a {
            s.count = 0;
            return;
        }
    }
    loop {
        let ring = &mut s.rings[((s.slot + i) % 5) as usize];
        let v = scale(get4(ring, 204), f32::from_bits(0x3f7b_62f9));
        put4(ring, 204, v);
        i += 1;
        let step = scale(get4(ring, 204), TICK);
        let p = add(get4(ring, 200), step);
        put4(ring, 200, p);
        if s.count <= i {
            return;
        }
        let ring = &mut s.rings[((s.slot + i) % 5) as usize];
        let a = f32::from_bits(ring[238]) + TICK;
        ring[238] = a.to_bits();
        if !(a <= max) {
            break;
        }
    }
    s.count = i;
}

/// The pool and the splash beside it, as the game lays them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BrakeFx {
    pub pool: Pool,
    pub splash: Splash,
}

/// `Fx_BrakeSpray_Update` 0x12e6e8.
pub fn brake_update(b: &mut BrakeFx, r: &FanIn, env: &mut dyn SprayEnv) {
    {
        let mut rand = || env.rand_fx();
        fan_update(&mut b.splash, r, &mut rand);
    }
    let speed = dot4(r.vel, r.vel).sqrt();
    let lvl = if r.hit == 2 {
        b.pool.level
    } else if 0.7 < r.f1fc.abs() && r.motion == 2 && f32::from_bits(0x438a_e38e) < speed {
        let mut l = b.pool.level + 0.05f32;
        if 1.0 < l {
            l = 1.0;
        }
        b.pool.level = l;
        l
    } else {
        b.pool.level
    };
    if 0.0 < lvl {
        if b.pool.recent < lvl * 0.1f32 + 0.5f32 {
            spawn(&mut b.pool, (lvl * 0.1f32) * 0.5f32, r.colour, r.ground, r.up, r.vel, env);
        }
        b.pool.level -= TICK;
    }
    let count = b.pool.count;
    b.pool.recent *= f32::from_bits(0x3f2a_aaaa);
    let mut i = count - 1;
    while i >= 0 {
        let sum = b.pool.head + i;
        let e = &mut b.pool.entries[(sum % 30) as usize];
        let a = f32::from_bits(e[16]) + TICK;
        e[16] = a.to_bits();
        if 2.0 < a {
            b.pool.count = (count - 1) - i;
            b.pool.head = (sum + 1) % 30;
            return;
        }
        i -= 1;
    }
}

//! The ground spray and the sparks (board row G7f6): `Fx_SurfaceSpray` 0x1311b0. The pool (0x34c0 bytes) is: +0
//! how many emitters are live, +4 the oldest, +8 whether the snow-plough spray has begun, +0xc its strength,
//! +0x10 the last spark point, +0x20 whether sparks have begun, +0x24 the light's tint; then 40 emitters of 0x150
//! bytes from +0x40 (0x140 bytes of emitter, +0x140 the sprite, +0x144 the draw mode).

use crate::emitter::*;
use crate::fxrand::one_plus_unit;

pub const BYTES: usize = 0x34c0;

#[derive(Clone, Debug, PartialEq)]
pub struct Spray {
    pub w: Vec<u32>,
}

impl Default for Spray {
    fn default() -> Self {
        Spray { w: vec![0; BYTES / 4] }
    }
}

impl Spray {
    fn f(&self, b: usize) -> f32 {
        f32::from_bits(self.w[b / 4])
    }
    fn set(&mut self, b: usize, v: f32) {
        self.w[b / 4] = v.to_bits();
    }
    fn v(&self, b: usize) -> V4 {
        [0, 1, 2, 3].map(|k| self.f(b + 4 * k))
    }
    fn setv(&mut self, b: usize, x: V4) {
        for k in 0..4 {
            self.set(b + 4 * k, x[k]);
        }
    }
    fn i(&self, b: usize) -> i32 {
        self.w[b / 4] as i32
    }
    fn slot(&mut self, idx: i32) -> &mut [u32] {
        let b = 0x10 + 0x54 * idx as usize;
        &mut self.w[b..b + 0x54]
    }
}

/// The light object's three floats the spray eases (+0xc the tint level, +0x10 its speed, +0x14 its target).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub f0c: f32,
    pub f10: f32,
    pub f14: f32,
}

/// What the spray reads of the rider; `cfg` is the surface record's words +0x34 .. +0x54.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprayIn {
    pub vel: V4,
    pub f214: f32,
    pub flipped: u32,
    pub surface: u32,
    pub hit: u32,
    pub ground: V4,
    pub motion: u32,
    pub f13c: u32,
    pub w4a60: V4,
    pub w4a70: V4,
    pub w4a80: V4,
    pub w4a90: V4,
    pub v180: V4,
    pub v190: V4,
    pub v1a0: V4,
    pub colour: V4,
    pub cfg: [u32; 9],
}

const TICK: f32 = f32::from_bits(0x3c88_8889);

fn cfgf(r: &SprayIn, off: usize) -> f32 {
    f32::from_bits(r.cfg[(off - 0x34) / 4])
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

fn unit_draw(rand_fx: &mut dyn FnMut() -> u32) -> f32 {
    (f32::from_bits(one_plus_unit(rand_fx())) - 1.0) + 0.0
}

/// A slot to take for a new emitter: the pool drops its oldest at 40, answers the new one's index.
fn take_slot(s: &mut Spray) -> i32 {
    let mut cnt = s.i(0);
    if cnt == 40 {
        s.w[0] = 39;
        cnt = 39;
        let st = (s.i(4) + 1).rem_euclid(40);
        s.w[1] = st as u32;
    }
    let start = s.i(4);
    s.w[0] = (cnt + 1) as u32;
    (start + cnt).rem_euclid(40)
}

/// `Fx_SurfaceSpray` 0x1311b0.
pub fn update(s: &mut Spray, r: &SprayIn, light: &mut Light, sprites: &[u32], rand_fx: &mut dyn FnMut() -> u32) {
    let cnt = s.i(0);
    for i in 0..cnt {
        let idx = (s.i(4) + i).rem_euclid(40);
        advance_time(s.slot(idx), TICK);
    }
    let v1e0 = r.w4a70;
    let v1d0 = r.w4a80;
    let v1c0 = r.w4a60;
    let speed = dot4(r.vel, r.vel).sqrt();
    let mut mode: u32 = 5;
    let mut sprite: u32 = 0;
    let mut f20 = 400.0f32;
    let mut f31 = 400.0f32;
    let mut f30 = 50.0f32;
    let mut f27 = 2.0f32;
    let mut f29 = 4.0f32;
    let mut f28 = f32::from_bits(0x3ecc_cccd);
    let mut f24 = f32::from_bits(0x3f19_999a);
    let mut f25 = 0.25f32;
    let mut f22 = f32::from_bits(0x3f33_3333);
    let mut f23 = f32::from_bits(0x3e4c_cccd);
    let mut f21 = 0.0f32;
    let mut p: V4 = [0.0; 4];
    let motion = r.motion;
    if motion == 2 && r.hit != 2 {
        let surf = r.surface;
        if surf == 9 || surf == 0xd || surf == 0x12 || surf == 0x13 {
            p = r.vel;
            sprite = 0xd;
            f21 = 0.0;
            s.w[2] = 0;
        } else {
            f21 = speed * f32::from_bits(0x3bc4_9ba6);
            let mut f3 = r.f214;
            if r.flipped != 0 {
                f3 = -f3;
            }
            s.w[2] = 0;
            let f1 = f3 * f3;
            let f0 = f3 * f32::from_bits(0x3f8c_cccd);
            let f1 = f1 * 149.0;
            let f0 = f0 * speed;
            let mut va = scale(v1d0, f0);
            let f1 = f1 * f21;
            p = add(r.vel, va);
            f21 = f1 + f21;
            let f2 = dot4(p, v1d0);
            let f12 = f2.abs();
            let f450 = 450.0f32;
            if f12 < f450 {
                f20 = 300.0;
            } else {
                f20 = f12 - 150.0;
                if f2 < 0.0 {
                    va = scale(v1d0, (f2 + f450) * 0.5);
                    p = sub(p, va);
                } else {
                    va = scale(v1d0, (f450 - f2) * 0.5);
                    p = add(p, va);
                }
            }
            let d = dot4(p, v1c0);
            f31 = d;
            f30 = 0.0;
            va = scale(v1c0, d * 0.5);
            p = sub(p, va);
            mode = r.cfg[(0x54 - 0x34) / 4];
            sprite = r.cfg[(0x44 - 0x34) / 4];
            let c38 = cfgf(r, 0x38);
            f29 = c38 * f32::from_bits(0x3f99_999a);
            f21 *= cfgf(r, 0x34);
            f27 = c38 * f32::from_bits(0x3f4c_cccd);
            f28 = cfgf(r, 0x3c);
            f24 = cfgf(r, 0x40);
            f25 = cfgf(r, 0x50);
            f22 = cfgf(r, 0x48);
            f23 = cfgf(r, 0x4c);
        }
    } else if motion == 1 {
        let mut f2;
        if s.i(8) == 0 {
            f2 = 70.0f32;
            s.set(0xc, f2);
        } else {
            f2 = s.f(0xc);
            if 10.0 < f2 {
                f2 *= f32::from_bits(0x3f72_58bf);
                s.set(0xc, f2);
            }
        }
        let f0 = ps2_div(speed, f32::from_bits(0x438a_e38e));
        s.w[2] = 1;
        if !(1.0 < f0) {
            f2 *= f0;
        }
        p = r.vel;
        sprite = 0xd;
        f21 = f2;
    } else if motion == 3 {
        s.w[2] = 0;
        let surf = r.surface;
        p = r.vel;
        f21 = if surf == 0xd || surf == 0x12 || surf == 0x13 { 0.0 } else { 30.0 };
        sprite = 0xd;
    } else {
        s.w[2] = 0;
    }
    // the new emitters
    let n = (f21 * TICK + unit_draw(rand_fx)) as i32;
    if n != 0 {
        let idx = take_slot(s);
        let pos = if motion == 2 { r.ground } else { r.w4a90 };
        let c = r.colour;
        let (c1, c2, c3) = (c[1], c[2], c[3]);
        let mean = ((c1 + c2) + c3) * f32::from_bits(0x3eaa_aaab);
        let n1 = (c1 + f22 * (mean - c1)) + f23;
        let n3 = (c3 + f22 * (mean - c3)) + f23;
        let n2 = (c2 + f22 * (mean - c2)) + f23;
        let k = [clamp01(c[0]), clamp01(n1), clamp01(n2), clamp01(n3)];
        let b60 = [k[1] * 128.0, k[2] * 128.0, k[3] * 128.0, f25 * 128.0];
        let b70 = [k[1] * 128.0, k[2] * 128.0, k[3] * 128.0, 0.0];
        let slot = s.slot(idx);
        init(slot, f32::from_bits(0x3ccc_cccd), f32::from_bits(0x3ba3_d70a), 5.0, n, 1, [0.0, 0.0, -100.0, 0.0]);
        for (o, w) in crate::fxrand::EMITTER_FACTORS.iter().zip(crate::fxrand::emitter_factors(rand_fx)) {
            slot[o / 4] = w;
        }
        size_range(slot, f27, f29);
        life_range(slot, f28, f24);
        let qa = scale(v1d0, 15.0);
        let qb = scale(v1c0, 90.0);
        spawn_quad(slot, pos, qa, qb);
        let u1 = scale(v1d0, f20);
        let u2 = scale(v1e0, f30);
        let u3 = scale(v1c0, f31);
        velocity_basis(slot, p, u1, u2, u3);
        clear_force(slot);
        colors(slot, f24, b60, b70, [0.0; 4], [0.0; 4]);
        set_time(slot, 0.0);
        slot[0x51] = mode;
        slot[0x50] = sprites[sprite as usize];
    }
    // the light's tint
    if r.f13c == 0 {
        light.f14 = 0.0;
        light.f0c = 0.0;
        light.f10 = 0.0;
    } else {
        if light.f0c <= 0.0 {
            light.f10 = 0.0;
            light.f14 = 1.0;
        } else if 1.0 <= light.f0c {
            light.f10 = 0.0;
            light.f14 = 0.0;
        }
        let f1 = light.f10 * TICK;
        let f4 = light.f14;
        let f3 = light.f0c;
        let mut f2 = f4;
        if f4 + f1 < f3 {
            f2 = f3 - f1;
        } else if f3 < f4 - f1 {
            f2 = f3 + f1;
        }
        light.f0c = f2;
        s.setv(0x24, [0.0, f2, f2, f2]);
    }
    // the sparks
    let mut last = s.v(0x10);
    if motion == 2 || motion == 3 {
        let surf = r.surface;
        if surf == 0xd || surf == 0x12 || surf == 0x13 || surf == 9 {
            let mut pos = add(r.ground, scale(r.vel, f32::from_bits(0x3ca3_d70a)));
            if motion == 3 {
                pos = add(pos, scale(r.vel, f32::from_bits(0x3ca3_d70a)));
            }
            if s.i(0x20) == 0 {
                s.setv(0x10, pos);
            }
            s.w[0x20 / 4] = 1;
            let mut rate = 50.0f32;
            let mut life = 0.25f32;
            let mut mul = 2.0f32;
            if unit_draw(rand_fx) < TICK {
                s.setv(0x24, [0.0, f32::from_bits(0x3f4c_cccd), f32::from_bits(0x3f19_999a), f32::from_bits(0x3e4c_cccd)]);
                rate = 1050.0;
                life = f32::from_bits(0x3f26_6666);
                mul = 3.0;
            }
            let n2 = (rate * TICK + unit_draw(rand_fx)) as i32;
            if n2 != 0 {
                let idx = take_slot(s);
                let cap = f32::from_bits(0x44d0_5555);
                let k = if speed < cap { mul * speed } else { mul * cap };
                let hi = k * f32::from_bits(0x3eb3_3333);
                let kk = if hi < f32::from_bits(0x448a_e38e) { hi } else { f32::from_bits(0x448a_e38e) };
                let last_pt = s.v(0x10);
                let slot = s.slot(idx);
                init(slot, f32::from_bits(0x3ccc_cccd), f32::from_bits(0x3be5_6042), 12.0, n2, 7, [0.0, 0.0, -2000.0, 0.0]);
                for (o, w) in crate::fxrand::EMITTER_FACTORS.iter().zip(crate::fxrand::emitter_factors(rand_fx)) {
                    slot[o / 4] = w;
                }
                size_range(slot, f32::from_bits(0x3f40_0000), f32::from_bits(0x3fa0_0000));
                life_range(slot, f32::from_bits(0x3dcc_cccd), life);
                if motion == 3 {
                    spawn_line(slot, pos, last_pt);
                } else {
                    spawn_quad(slot, pos, scale(v1d0, 15.0), scale(v1c0, 90.0));
                }
                let va = scale(r.v1a0, k * f32::from_bits(0x3d4c_cccd));
                let vb = scale(r.v180, k * f32::from_bits(0xbdcc_cccd));
                let v = add(va, vb);
                let u1 = scale(r.v190, kk);
                let f20b = k * f32::from_bits(0x3e4c_cccd);
                let u2 = scale(r.v180, f20b);
                let u3 = scale(r.v1a0, f20b);
                velocity_basis(slot, v, u1, u2, u3);
                clear_force(slot);
                colors(slot, life, [200.0, 90.0, 0.0, 80.0], [200.0, 90.0, 90.0, 0.0], [0.0, 50.0, 0.0, 0.0], [0.0, 0.0, 50.0, 0.0]);
                set_time(slot, f32::from_bits(0x3cf5_c28f));
                slot[0x51] = 5;
                slot[0x50] = sprites[0];
            }
            last = pos;
            s.setv(0x10, last);
        } else {
            s.w[0x20 / 4] = 0;
        }
    } else {
        s.w[0x20 / 4] = 0;
    }
    // the tint fades 1/12 a tick
    let dec = f32::from_bits(0xbdaa_aaab);
    let t0 = s.f(0x24) + 0.0;
    s.set(0x24, t0);
    let t1 = s.f(0x28) + dec;
    s.set(0x28, t1);
    let t2 = s.f(0x2c) + dec;
    s.set(0x2c, t2);
    let t3 = s.f(0x30) + dec;
    s.set(0x30, t3);
    let c1 = clamp01(t1);
    s.set(0x28, c1);
    let c2 = clamp01(t2);
    s.set(0x2c, c2);
    let c3 = clamp01(t3);
    s.set(0x30, c3);
    let c0 = clamp01(t0);
    s.set(0x24, c0);
    let _ = last;
}

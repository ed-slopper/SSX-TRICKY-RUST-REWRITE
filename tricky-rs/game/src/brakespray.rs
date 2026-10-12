//! The brake spray's particle pool (board row G19): `Fx_Spray_SpawnParticle` 0x12e330 and `Fx_BrakeSpray_Emit`
//! 0x12e008. The pool is 30 entries of 0x50 bytes at the start of the effect object, then +0x960 how many are
//! live, +0x964 the oldest, +0x968 a count of recent spawns, +0x96c a level that climbs by half a step each emit
//! to 2.0, and +0x970 the splash emitter (`Fx_Splash_EmitAtPoint` 0x12c7e0, handed back to the caller by
//! [`SprayEnv::splash`]).

pub const POOL: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct Pool {
    /// 20 words each: +0 position, +0x10 direction, +0x20 colour (0, r, g, b), +0x30 two signs as floats,
    /// +0x38 their complements, +0x40 zero, +0x44 a random quarter, +0x48 the size, +0x4c untouched.
    pub entries: [[u32; 20]; POOL],
    pub count: i32,
    pub head: i32,
    pub recent: f32,
    pub level: f32,
}

impl Default for Pool {
    fn default() -> Self {
        Pool { entries: [[0; 20]; POOL], count: 0, head: 0, recent: 0.0, level: 0.0 }
    }
}

pub trait SprayEnv {
    /// `Rand_Fx` 0x23d4a8
    fn rand_fx(&mut self) -> u32;
    /// `Fx_Splash_EmitAtPoint` 0x12c7e0 on the pool's emitter, with the three vectors it was handed.
    fn splash(&mut self, a2: [f32; 4], a3: [f32; 4], a4: [f32; 4]);
}

fn v4_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
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

/// `Fx_Spray_SpawnParticle` 0x12e330(this, size, colour, pos, dir, from): takes the next entry (the oldest goes
/// when full), puts it at `pos` plus the part of `from` square to `dir` (a random 0.1..0.2 of it), and washes
/// the colour half way to its grey.
pub fn spawn(pool: &mut Pool, size: f32, colour: [f32; 4], pos: [f32; 4], dir: [f32; 4], from: [f32; 4], env: &mut dyn SprayEnv) {
    let mut live = pool.count;
    if live == POOL as i32 {
        pool.head = (pool.head + 1) % POOL as i32;
        pool.count = POOL as i32 - 1;
        live = POOL as i32 - 1;
    }
    let idx = ((pool.head + live) % POOL as i32) as usize;
    pool.count = live + 1;
    let s0 = env.rand_fx() & 1;
    let s5 = env.rand_fx() & 1;
    let e = &mut pool.entries[idx];
    e[16] = 0;
    let dot = v4_dot(from, dir);
    let proj = dir.map(|x| x * dot);
    let perp = [from[0] - proj[0], from[1] - proj[1], from[2] - proj[2], from[3] - proj[3]];
    let r3 = env.rand_fx();
    let f = f32::from_bits((r3 & 0x7f_ffff) | 0x3f80_0000);
    let m = (f - 1.0) * 0.1f32 + 0.1f32;
    for i in 0..4 {
        e[i] = (pos[i] + perp[i] * m).to_bits();
        e[4 + i] = dir[i].to_bits();
    }
    e[12] = (s0 as i32 as f32).to_bits();
    e[13] = (s5 as i32 as f32).to_bits();
    e[14] = (if s0 == 0 { 1.0f32 } else { 0.0 }).to_bits();
    e[15] = (if s5 == 0 { 1.0f32 } else { 0.0 }).to_bits();
    e[17] = env.rand_fx() & 3;
    e[18] = size.to_bits();
    let (c1, c2, c3) = (colour[1], colour[2], colour[3]);
    let mean = ((c1 + c2) + c3) * f32::from_bits(0x3eaa_aaab);
    let d3 = (mean - c3) * 0.5;
    let d1 = (mean - c1) * 0.5;
    let d2 = (mean - c2) * 0.5;
    e[8] = 0;
    e[9] = clamp01(c1 + d1).to_bits();
    e[10] = clamp01(c2 + d2).to_bits();
    e[11] = clamp01(c3 + d3).to_bits();
    pool.recent += 1.0;
}

/// `Fx_BrakeSpray_Emit` 0x12e008(this, rider, pos, from, dir): the splash always; then, unless the rider's
/// +0x2e0 is 2, a spray particle when fewer than 0.7 are recent, and the level up by 0.5 (to 2.0).
pub fn emit(pool: &mut Pool, mode: u32, colour: [f32; 4], pos: [f32; 4], from: [f32; 4], dir: [f32; 4], env: &mut dyn SprayEnv) {
    env.splash(pos, from, dir);
    if mode == 2 {
        return;
    }
    if pool.recent < f32::from_bits(0x3f33_3333) {
        spawn(pool, f32::from_bits(0x3dcc_cccd), colour, pos, dir, from, env);
    }
    let mut l = pool.level + 0.5;
    if 2.0 < l {
        l = 2.0;
    }
    pool.level = l;
}

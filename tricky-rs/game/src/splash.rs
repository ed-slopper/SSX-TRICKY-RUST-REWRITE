//! The splash rings (board row G19b): `Fx_Splash_EmitAtPoint` 0x12c7e0 and `Fx_Splash_EmitRing` 0x12c190. The
//! splash object (+0x970 of the brake spray's effect object) holds +0 the last point, +0x10 the distance
//! gathered since (in hundredths of a step), +0x18 how many rings have started (to 5), +0x1c the next ring slot
//! (counting down), and from +0x20 five rings of 0x3c0 bytes.
//!
//! A ring, as words: 0..100 twenty-five points (zero), 100..200 twenty-five offsets, 200..204 the centre,
//! 204..208 the spread, 208..233 twenty-five widths, 233..237 the colour, 237 how many points, 238 zero, 239 the
//! fade (0.9922).

pub const RING_WORDS: usize = 240;
pub const RINGS: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Splash {
    pub last: [f32; 4],
    pub gathered: f32,
    /// +0x14: the brake fan's timer
    pub timer: f32,
    pub count: i32,
    pub slot: i32,
    pub rings: [[u32; RING_WORDS]; RINGS],
}

impl Default for Splash {
    fn default() -> Self {
        Splash { last: [0.0; 4], gathered: 0.0, timer: 0.0, count: 0, slot: 0, rings: [[0; RING_WORDS]; RINGS] }
    }
}

/// What the splash reads of the rider: +0x2e0 its mode, +0x150 its position, the colour at +0x5ae0's +0x470, and
/// the float at +0x58 of its record in the table the game reaches through 0x337c58 (+0x730, +0x24, 0x64 a rider).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplashRider {
    pub mode: u32,
    pub pos: [f32; 4],
    pub colour: [f32; 4],
    pub scale: f32,
}

fn v4_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

fn scale(v: [f32; 4], k: f32) -> [f32; 4] {
    v.map(|x| x * k)
}

fn add(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// `vopmula`/`vopmsub` and a zero w: a × b.
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], 0.0]
}

fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    a / b
}

/// A random float in 1..2 from a word.
fn unit_one(r: u32) -> f32 {
    f32::from_bits((r & 0x7f_ffff) | 0x3f80_0000)
}

fn put4(ring: &mut [u32; RING_WORDS], at: usize, v: [f32; 4]) {
    for k in 0..4 {
        ring[at + k] = v[k].to_bits();
    }
}

fn get4(ring: &[u32; RING_WORDS], at: usize) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| f32::from_bits(ring[at + k]))
}

/// `Fx_Splash_EmitRing` 0x12c190(ring, n, e2, u1, k_size, k_fade, k_width, k_lift): a ring of `ring[237]` points
/// round the axis (cos of the angle on `e2`, sin on `u1`), a random walk of 0.8..1.2 round the size, the last
/// point back on the first. `ring[237]` must be at least 1.
pub fn emit_ring(
    ring: &mut [u32; RING_WORDS],
    n: [f32; 4],
    e2: [f32; 4],
    u1: [f32; 4],
    k_size: f32,
    k_fade: f32,
    k_width: f32,
    k_lift: f32,
    rand_fx: &mut dyn FnMut() -> u32,
) {
    let count = ring[237] as i32;
    let mut walk = 1.0f32;
    let mut i = 0i32;
    while i < count - 1 {
        let f = (unit_one(rand_fx()) - 1.0) * 0.6f32 + -0.3f32;
        walk += f;
        if walk < 0.8f32 {
            walk = 1.6f32 - walk;
        }
        if 1.2f32 < walk {
            walk = 2.4f32 - walk;
        }
        let pi = f32::from_bits(0x4049_0fdb);
        let a = ((2 * i) as f32 * pi) / (count as f32 - 1.0);
        let t = a * f32::from_bits(0x3e22_f983) + 0.5;
        let mut fl = (t as i32) as f32;
        if t < fl {
            fl -= 1.0;
        }
        let tri = ((a - fl * f32::from_bits(0x40c9_0fdb)) / pi).abs();
        let tri = (tri + tri) - 1.0;
        let (s, c) = crate::math::sin_cos(a);
        let w = add(scale(e2, c), scale(u1, s));
        let lift = scale(n, k_lift);
        let out = scale(add(w, lift), ((1.0 - k_fade * tri) * walk) * k_size);
        put4(ring, 4 * i as usize, [0.0; 4]);
        put4(ring, 100 + 4 * i as usize, out);
        ring[208 + i as usize] = (k_width * walk).to_bits();
        i += 1;
    }
    let n_ = count as isize;
    let first = get4(ring, 0);
    let at = ((n_ - 1) * 4) as usize;
    put4(ring, at, first);
    let off = get4(ring, 100);
    put4(ring, 100 + at, off);
    ring[208 + (n_ - 1) as usize] = ring[208];
}

/// `Fx_Splash_EmitAtPoint` 0x12c7e0(this, rider, pos, v, n): a ring of splash where the board touches a surface
/// of normal `n` (up enough: n.z ≥ 0.3), no more often than every 0.3 of gathered distance.
pub fn emit_at_point(s: &mut Splash, rider: &SplashRider, pos: [f32; 4], v: [f32; 4], n: [f32; 4], rand_fx: &mut dyn FnMut() -> u32) {
    if rider.mode == 2 || rider.scale < f32::from_bits(0x3ca3_d70a) || n[2] < f32::from_bits(0x3e99_999a) {
        return;
    }
    let d = sub(pos, s.last);
    let len = v4_dot(d, d).sqrt();
    let gate = f32::from_bits(0x3e99_999a);
    if s.count != 0 {
        if len == 0.0 || gate < s.gathered {
            return;
        }
    }
    s.last = pos;
    s.gathered += ps2_div(100.0, len);
    if s.count < 5 {
        s.count += 1;
    }
    s.slot = (s.slot + 4) % 5;
    let ring = &mut s.rings[s.slot as usize];
    let p = sub(v, scale(n, v4_dot(v, n)));
    let c = cross(p, n);
    let q = {
        let sq = v4_dot(c, c);
        let q = if sq == 0.0 { f32::MAX } else { 1.0 / sq.sqrt() };
        scale(c, q)
    };
    let u1 = q;
    let e2 = cross(n, u1);
    ring[239] = f32::from_bits(0x3f7e_b7a0).to_bits();
    ring[237] = 0x19;
    ring[238] = 0;
    put4(ring, 233, rider.colour);
    put4(ring, 200, add(pos, scale(n, 5.0)));
    let r = sub(rider.pos, scale(n, v4_dot(rider.pos, n)));
    let l = v4_dot(r, r).sqrt() * f32::from_bits(0x3f33_3333);
    let spread = if f32::from_bits(0x4450_5555) < l {
        add(scale(r, ps2_div(f32::from_bits(0x4450_5555), l)), scale(n, f32::from_bits(0x4269_5555)))
    } else {
        add(r, scale(n, l * f32::from_bits(0x3d8f_5c29)))
    };
    put4(ring, 204, spread);
    let size = (unit_one(rand_fx()) - 1.0) * 150.0f32 + 250.0f32;
    let fade = (unit_one(rand_fx()) - 1.0) * f32::from_bits(0x3e99_999a) + 0.0;
    emit_ring(ring, n, e2, u1, size, fade, rider.scale * f32::from_bits(0x3f4c_cccd), f32::from_bits(0x3ecc_cccd), rand_fx);
}

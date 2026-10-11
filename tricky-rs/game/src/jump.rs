//! The jump exactly as the original computes it (`Jump_ApplyImpulse` 0x1284e0, board row G1a1): its constants,
//! its units (cm, cm/s, the game's z up) and the vector unit's order of operations, so it gives the same floats,
//! bit for bit, as the game's own code in the function runner (`tests/jump.rs`).

type V4 = [f32; 4];

/// `Math_Cos` and `Math_Sin` of the jump's fixed angles, as the game's own routines return them (read by running
/// `Math_Cos` 0x250e98 / `Math_Sin` 0x250d60 in the function runner): cos 50°, cos 70°, sin 20°.
const COS_50: f32 = f32::from_bits(0x3f24_8dbb);
const COS_70: f32 = f32::from_bits(0x3eaf_1d45);
const SIN_0: f32 = 0.0;
const SIN_20: f32 = f32::from_bits(0x3eaf_1d43);

/// A dot product as the vector unit forms it (vmul, then vaddabc / vmaddabc / vmaddbc): ((x + y) + z) + w.
fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

/// `Vec4_Scale` 0x102f50 and vmulbc: every lane times `k`.
fn scale(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// Normalised by vrsqrt (Q = 1 / sqrt(|a|²)) and vmulq.
fn normalize(a: V4) -> V4 {
    scale(a, 1.0 / dot(a, a).sqrt())
}

/// What `Jump_ApplyImpulse` reads from the rider.
#[derive(Clone, Copy, Debug, Default)]
pub struct JumpState {
    /// crouch (rider+0x1c8) and the amount taken off it (rider+0x208)
    pub crouch: f32,
    pub x208: f32,
    /// velocity, cm/s (rider+0x150)
    pub vel: V4,
    /// the jump stat byte (stats+0x17)
    pub jump_stat: u8,
    /// the motion (rider+0x424): 3 is the rail
    pub motion: u32,
    /// the board's up (rider+0x1a0), the ground normal (rider+0x2a0), the board's forward (rider+0x320)
    pub up: V4,
    pub normal: V4,
    pub forward: V4,
}

/// `Jump_ApplyImpulse` 0x1284e0: the rider's velocity after the jump, and whether it went off a steep lip (the
/// flag the game passes on to `Score_StartTrickChain`). `min` is the smallest jump (cm/s, the caller's $f12:
/// 630.8786 from `JumpControl_Update`).
pub fn jump_impulse(min: f32, s: &JumpState) -> (V4, bool) {
    let c = s.crouch - s.x208;
    let c = if c < 0.0 { 0.0 } else { c };
    let speed = dot(s.vel, s.vel).sqrt();
    let by_speed = if speed < 993.4801 { speed * 0.88093346 + 24.678448 } else { 899.86835 };
    let j = s.jump_stat as f32 * 0.78700274 * 0.003921569 + 0.09851468;
    // the machine code multiplies c·(c·J), then by the speed term (0x128584, 0x1285c4, 0x128600)
    let mut dv = c * (c * j) * by_speed;
    if dv < min {
        dv = min;
    }
    if s.motion == 3 {
        // off a rail: straight along the board's up
        return (add(s.vel, scale(s.up, dv)), false);
    }
    // how steep the lip is: 0 at 50° from flat, 1 at 70°
    let t = (s.normal[2] - COS_50) * (1.0 / (COS_70 - COS_50));
    if s.forward[2] < 0.0 || t < 0.0 || s.vel[2] < 0.0 {
        // the usual jump: along the normal tipped a fifth toward the board's forward
        let d = normalize(add(s.normal, scale(s.forward, 0.2)));
        return (add(s.vel, scale(d, dv)), false);
    }
    // a steep lip: the jump leans toward the board's forward along the lip, up to 0.9
    let h = normalize([s.normal[0], s.normal[1], 0.0, 0.0]);
    let along = normalize(sub(s.forward, scale(h, dot(s.forward, h))));
    let b = t * (along[2] - SIN_0) * (1.0 / (SIN_20 - SIN_0));
    let b = if 0.0 <= b { if 0.9 < b { 0.9 } else { b } } else { 0.0 };
    let a = 1.0 - b;
    let dir = normalize(add(scale(s.normal, a), scale(along, b)));
    let v = add(scale(s.vel, a), scale(along, b * speed));
    (add(scale(v, 0.5), scale(dir, dv)), true)
}

//! The rider-driven update modes of the animation controller (board row G5b): `AnimMode_*`, which
//! `AnimCtl_UpdateClip` (row G5a2) calls for clips whose update mode is 4..11. Instead of advancing by time, each
//! sets the sequence's sources, their weights (+0x54..) and times (+0x48..) from the rider's state, by board class
//! (rider+0x420). The tables are the exe's (clip ids by class). Checked in the function runner (`tests/animmode.rs`).
//!
//! The rider's words read: +0x1b4 riding switch (mirrors the inputs of the brake and the two direction modes), +0x1dc
//! a row index (the crouch-steer and wind-up tables), +0x1f0, +0x1fc the brake, +0x208 the crouch, +0x214 the steer,
//! +0x220 / +0x22c the spin and flip wind-ups, +0x238 the start gate's lean, +0x268, +0x27c / +0x280 the direction.

use crate::animseq::{self as seq, div, sqrt, SeqEnv};
use crate::pf::Mem;
use crate::ps2f::F;

/// The tables (0x379180 mode 8, 0x379190 mode 9, 0x3791d0 mode 5, 0x3791e8 mode 4, 0x379260 mode 10, 0x3794b8
/// mode 7), clip ids by class
pub const T_CLASS_FRAME: u32 = 0x379180;
pub const T_DIRECTION: u32 = 0x379190;
pub const T_CROUCH: u32 = 0x3791d0;
pub const T_BRAKE: u32 = 0x3791e8;
pub const T_WIND_UP: u32 = 0x379260;
pub const T_CROUCH_STEER: u32 = 0x3794b8;

const HALF_PI: F = F(std::f32::consts::FRAC_PI_2);
const PI: F = F(std::f32::consts::PI);

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn abs(v: F) -> F {
    F(f32::from_bits(v.0.to_bits() & 0x7fff_ffff))
}
/// Source i's last frame (its clip header's length − 1)
fn last(m: &Mem, s: u32, i: u32) -> F {
    let t = m.r32(s + 0x78 + 4 * i);
    F((m.r16(m.r32(t + 8) + 0xa) as i32 - 1) as f32)
}
fn rider(m: &Mem, ctl: u32) -> u32 {
    m.r32(ctl + 0x24)
}

/// `Math_Atan` 0x2510f8 with the EE's division and square root (toward zero)
fn atan(x: F) -> F {
    F(crate::math::atan_with(x.0, |a, b| div(F(a), F(b)).0, |a| sqrt(F(a)).0))
}

/// The angle of (x, y) as the two direction modes write it around `Math_Atan`: ±π/2 on the y axis, 0 at the
/// origin, ±π added on the left
fn angle(x: F, y: F) -> F {
    let z = F(0.0);
    if x.ee() == z.ee() {
        if y.ee() == z.ee() {
            return y;
        }
        return if z.ee() <= y.ee() { HALF_PI } else { -HALF_PI };
    }
    let t = atan(div(y, x));
    if x.ee() < z.ee() {
        if z.ee() < y.ee() {
            t + PI
        } else {
            t - PI
        }
    } else {
        t
    }
}

/// The stick's length kept in 0..1
fn magnitude(x: F, y: F) -> F {
    let r = sqrt(y * y + x * x);
    if r.ee() < 0.0 {
        F(0.0)
    } else if 1.0 < r.ee() {
        F(1.0)
    } else {
        r
    }
}

/// `AnimMode_BoardClassFrame` 0x160140 (mode 8): the class's clip as the one source, weight 1, at rider+0x1f0 of
/// its length
pub fn board_class_frame(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32) {
    let r = rider(m, ctl);
    let f = rf(m, r + 0x1f0);
    let clip = m.r32(T_CLASS_FRAME.wrapping_add(m.r32(r + 0x420).wrapping_mul(4)));
    seq::set_source(m, env, s, 0, clip);
    wf(m, s + 0x54, F(1.0));
    m.w32(s + 0x44, 1);
    let t = f * last(m, s, 0);
    wf(m, s + 0x48, t);
}

/// `AnimMode_DirectionBlend` 0x1601d0 (mode 9): the direction (rider+0x27c, +0x280; mirrored riding switch) as a
/// blend of the class's two clips around it (one per quarter turn: words 0..3 of its five), each at the stick's
/// length of its clip; no direction: word 4 alone at frame 0
pub fn direction_blend(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32) {
    let r = rider(m, ctl);
    let (x, y) = (rf(m, r + 0x27c), rf(m, r + 0x280));
    let row = T_DIRECTION.wrapping_add(m.r32(r + 0x420).wrapping_mul(0x14));
    if x.ee() == 0.0 && y.ee() == 0.0 {
        seq::set_source(m, env, s, 0, m.r32(row + 0x10));
        m.w32(s + 0x44, 1);
        m.w32(s + 0x48, 0);
        wf(m, s + 0x54, F(1.0));
        return;
    }
    let a = if m.r32(r + 0x1b4) != 0 { angle(-x, -y) } else { angle(x, y) };
    let (off, w0, w1) = if HALF_PI.ee() <= a.ee() {
        (HALF_PI, 0, 8)
    } else if 0.0 <= a.ee() {
        (F(0.0), 8, 4)
    } else if a.ee() <= (-HALF_PI).ee() {
        (-HALF_PI, 0, 0xc)
    } else {
        (F(0.0), 0xc, 4)
    };
    let rest = a - off;
    seq::set_source(m, env, s, 0, m.r32(row + w0));
    seq::set_source(m, env, s, 1, m.r32(row + w1));
    let mag = magnitude(x, y);
    let k = abs(rest) * F(f32::from_bits(0x3f22_f983));
    m.w32(s + 0x44, 2);
    wf(m, s + 0x54, k);
    wf(m, s + 0x58, F(1.0) - k);
    let t0 = mag * last(m, s, 0);
    let t1 = mag * last(m, s, 1);
    wf(m, s + 0x48, t0);
    wf(m, s + 0x4c, t1);
}

/// `AnimMode_CrouchBlend` 0x1605a0(dt) (mode 5): the class's standing clip advanced by time with weight 1 − crouch,
/// then its crouched clip at the same phase with weight crouch
pub fn crouch_blend(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, dt: F) {
    let r = rider(m, ctl);
    let row = T_CROUCH.wrapping_add(m.r32(r + 0x420).wrapping_mul(8));
    let c = rf(m, r + 0x208);
    seq::set_source(m, env, s, 0, m.r32(row));
    m.w32(s + 0x44, 1);
    wf(m, s + 0x54, F(1.0) - c);
    seq::advance(m, s, dt);
    seq::set_source(m, env, s, 1, m.r32(row + 4));
    wf(m, s + 0x58, c);
    let p = seq::phase(m, s);
    m.w32(s + 0x44, 2);
    let t = p * last(m, s, 1);
    wf(m, s + 0x4c, t);
}

/// `AnimMode_BrakeBlend` 0x160680(dt) (mode 4): the brake (rider+0x1fc) and rider+0x268 (mirrored riding switch)
/// blended over the class's ten clips (`T_BRAKE`, 0x28 a class).
/// - Neither: the crouch blend of words 4 and 5, held and advanced by time (on the trick board, class 2, only at
///   a crouch of exactly 0 or 1; otherwise as the next case).
/// - No brake: words 1 and 3 when +0x268 is above 0, else 0 and 2, by the crouch; on class 2 four clips in two
///   halves of the crouch (words 1 / 9 / 3 or 0 / 8 / 2). Both at |+0x268| of their length.
/// - Braking: word 1 or 0 (by +0x268's sign) with weight 1 − |brake| at |+0x268|, and word 6 or 7 (by the brake's
///   sign) with weight |brake| at |brake|.
pub fn brake_blend(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, dt: F) {
    let r = rider(m, ctl);
    let mut side = rf(m, r + 0x268);
    let mut brake = rf(m, r + 0x1fc);
    let side_abs = abs(side);
    let brake_abs = abs(brake);
    let c = rf(m, r + 0x208);
    let class = m.r32(r + 0x420);
    if m.r32(r + 0x1b4) != 0 {
        brake = -brake;
        side = -side;
    }
    let row = T_BRAKE.wrapping_add(class.wrapping_mul(0x28));
    let w = |m: &Mem, o: u32| m.r32(row + o);
    let c2 = |m: &Mem, o: u32| m.r32(T_BRAKE + 0x50 + o);
    if side.ee() == 0.0 && brake_abs.ee() == 0.0 && (class != 2 || c.ee() == 1.0 || c.ee() == 0.0) {
        m.w32(s + 0x44, 2);
        wf(m, s + 0x58, c);
        wf(m, s + 0x54, F(1.0) - c);
        seq::set_source(m, env, s, 0, w(m, 0x10));
        seq::set_source(m, env, s, 1, w(m, 0x14));
        m.w32(s + 0x20, 1);
        seq::advance(m, s, dt);
        return;
    }
    let (t0, t1);
    if brake_abs.ee() == 0.0 {
        let right = 0.0 < side.ee();
        let (a, b) = if class == 2 {
            if c.ee() < 0.5 {
                let c2x = c + c;
                wf(m, s + 0x58, c2x);
                wf(m, s + 0x54, F(1.0) - c2x);
                if right { (c2(m, 4), c2(m, 0x24)) } else { (c2(m, 0), c2(m, 0x20)) }
            } else {
                let c2x = c + c;
                wf(m, s + 0x54, F(2.0) - c2x);
                wf(m, s + 0x58, c2x - F(1.0));
                if right { (c2(m, 0x24), c2(m, 0xc)) } else { (c2(m, 0x20), c2(m, 8)) }
            }
        } else {
            wf(m, s + 0x58, c);
            wf(m, s + 0x54, F(1.0) - c);
            if right { (w(m, 4), w(m, 0xc)) } else { (w(m, 0), w(m, 8)) }
        };
        seq::set_source(m, env, s, 0, a);
        seq::set_source(m, env, s, 1, b);
        m.w32(s + 0x44, 2);
        t0 = side_abs * last(m, s, 0);
        t1 = side_abs * last(m, s, 1);
    } else {
        wf(m, s + 0x58, brake_abs);
        m.w32(s + 0x44, 2);
        wf(m, s + 0x54, F(1.0) - brake_abs);
        let a = if 0.0 < side.ee() { w(m, 4) } else { w(m, 0) };
        seq::set_source(m, env, s, 0, a);
        let b = if 0.0 < brake.ee() { w(m, 0x18) } else { w(m, 0x1c) };
        seq::set_source(m, env, s, 1, b);
        t0 = side_abs * last(m, s, 0);
        t1 = brake_abs * last(m, s, 1);
    }
    wf(m, s + 0x48, t0);
    wf(m, s + 0x4c, t1);
}

/// `AnimMode_GateLeanFrame` 0x160af0 (mode 11): source 0 at the start gate's lean (rider+0x238) of its length
pub fn gate_lean_frame(m: &mut Mem, ctl: u32, s: u32) {
    let t = rf(m, rider(m, ctl) + 0x238) * last(m, s, 0);
    wf(m, s + 0x48, t);
}

/// `AnimMode_WindUpBlend` 0x160b20 (mode 10): the spin and flip wind-ups (rider+0x220, +0x22c; mirrored riding
/// switch) as a direction in eighth turns over the row's clips (`T_WIND_UP`, 0xc8 a class, the row rider+0x1dc),
/// weighted by the crouch, with the row's word 0 as a third source at frame 0 weighted 1 − crouch; no wind-up: word
/// 0 at frame 0 and word 5 at the crouch of its length
pub fn wind_up_blend(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32) {
    let r = rider(m, ctl);
    let x = rf(m, r + 0x220);
    let y = rf(m, r + 0x22c);
    let c = rf(m, r + 0x208);
    let class = m.r32(r + 0x420);
    let at = T_WIND_UP.wrapping_add(class.wrapping_mul(0xc8)).wrapping_add(m.r32(r + 0x1dc).wrapping_mul(4));
    let w = |m: &Mem, o: u32| m.r32(at.wrapping_add(o));
    if x.ee() == 0.0 && y.ee() == 0.0 {
        seq::set_source(m, env, s, 0, w(m, 0));
        m.w32(s + 0x48, 0);
        seq::set_source(m, env, s, 1, w(m, 0x14));
        wf(m, s + 0x54, F(1.0) - c);
        m.w32(s + 0x44, 2);
        wf(m, s + 0x58, c);
        let t = c * last(m, s, 1);
        wf(m, s + 0x4c, t);
        return;
    }
    let a = if m.r32(r + 0x1b4) != 0 { angle(-x, -y) } else { angle(x, y) };
    let q = F(f32::from_bits(0x3f49_0fdb));
    let tq = F(f32::from_bits(0x4016_cbe4));
    let (off, a0, a1) = if tq.ee() <= a.ee() {
        (tq, 0x28, 0x3c)
    } else if HALF_PI.ee() <= a.ee() {
        (HALF_PI, 0x3c, 0x50)
    } else if q.ee() <= a.ee() {
        (q, 0x50, 0x64)
    } else if 0.0 <= a.ee() {
        (F(0.0), 0x64, 0x78)
    } else if a.ee() <= (-tq).ee() {
        (-tq, 0x28, 0xb4)
    } else if a.ee() <= (-HALF_PI).ee() {
        (-HALF_PI, 0xb4, 0xa0)
    } else if a.ee() <= (-q).ee() {
        (-q, 0xa0, 0x8c)
    } else {
        (F(0.0), 0x8c, 0x78)
    };
    let rest = a - off;
    seq::set_source(m, env, s, 0, w(m, a0));
    seq::set_source(m, env, s, 1, w(m, a1));
    let mag = magnitude(x, y);
    let k = abs(rest) * F(f32::from_bits(0x3fa2_f983));
    wf(m, s + 0x54, c * k);
    wf(m, s + 0x48, mag * last(m, s, 0));
    wf(m, s + 0x58, c * (F(1.0) - k));
    wf(m, s + 0x4c, mag * last(m, s, 1));
    seq::set_source(m, env, s, 2, w(m, 0));
    m.w32(s + 0x44, 3);
    m.w32(s + 0x50, 0);
    wf(m, s + 0x5c, F(1.0) - c);
}

/// `AnimMode_CrouchSteerBlend` 0x161118(dt) (mode 7): two sources weighted by the crouch (1 − crouch, crouch) from
/// the row's clips (`T_CROUCH_STEER`, 0x78 a class, the row rider+0x1dc): with no steer (rider+0x214), or row 1 or
/// 2, words 0x50 / 0x64 held and advanced by time; else 0 / 0x14 steering right, 0x28 / 0x3c left, both at |steer| of
/// their length
pub fn crouch_steer_blend(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, dt: F) {
    let r = rider(m, ctl);
    m.w32(s + 0x44, 2);
    let c = rf(m, r + 0x208);
    let steer = rf(m, r + 0x214);
    let st = abs(steer);
    let class = m.r32(r + 0x420);
    let rown = m.r32(r + 0x1dc);
    wf(m, s + 0x58, c);
    wf(m, s + 0x54, F(1.0) - c);
    let at = T_CROUCH_STEER.wrapping_add(rown.wrapping_mul(4)).wrapping_add(class.wrapping_mul(0x78));
    let w = |m: &Mem, o: u32| m.r32(at.wrapping_add(o));
    if st.ee() == 0.0 || rown == 1 || rown == 2 {
        seq::set_source(m, env, s, 0, w(m, 0x50));
        seq::set_source(m, env, s, 1, w(m, 0x64));
        m.w32(s + 0x20, 1);
        seq::advance(m, s, dt);
        return;
    }
    let (a, b) = if 0.0 < steer.ee() { (0, 0x14) } else { (0x28, 0x3c) };
    seq::set_source(m, env, s, 0, w(m, a));
    seq::set_source(m, env, s, 1, w(m, b));
    let t0 = st * last(m, s, 0);
    let t1 = st * last(m, s, 1);
    wf(m, s + 0x48, t0);
    wf(m, s + 0x4c, t1);
}

/// `AnimMode_CrouchFrame` 0x161300 (mode 6): source 0 at (1 − crouch) of its length
pub fn crouch_frame(m: &mut Mem, ctl: u32, s: u32) {
    let c = rf(m, rider(m, ctl) + 0x208);
    let t = (F(1.0) - c) * last(m, s, 0);
    wf(m, s + 0x48, t);
}

/// A mode by its function's address (the controller's dispatch, `crate::animctl::MODES`); false for the plain
/// advance and anything else
pub fn run(m: &mut Mem, env: &mut dyn SeqEnv, f: u32, ctl: u32, s: u32, dt: F) -> bool {
    match f {
        0x160680 => brake_blend(m, env, ctl, s, dt),
        0x1605a0 => crouch_blend(m, env, ctl, s, dt),
        0x161300 => crouch_frame(m, ctl, s),
        0x161118 => crouch_steer_blend(m, env, ctl, s, dt),
        0x160140 => board_class_frame(m, env, ctl, s),
        0x1601d0 => direction_blend(m, env, ctl, s),
        0x160b20 => wind_up_blend(m, env, ctl, s),
        0x160af0 => gate_lean_frame(m, ctl, s),
        _ => return false,
    }
    true
}

//! Spin and flip in the air exactly as the original computes them (`SpinState_Update` 0x100908, board row
//! G1a5): the landing alignment, the stick-driven rates and their band, the auto-complete after the stick is let
//! go, the half-pipe turn (a half turn in the air that toggles switch), and the clip, lean and reset calls in the
//! game's order. `tests/spin.rs` checks it bit for bit against the original in the function runner.
//!
//! Field names are the spin object's (`st`, the handler's `this`) and the rider's (`rider` = st+0x70) offsets.

use crate::jump::pad_dir;
use crate::math::sin_cos;

type V4 = [f32; 4];

const PI: f32 = f32::from_bits(0x4049_0fdb);
const TAU: f32 = 6.283_185_5;

/// The spin object (`SpinState_Update`'s `this`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpinObj {
    /// +8: 0 starting, 1 spinning on the stick, 2 completing (stick let go)
    pub mode: u32,
    /// +0xc: the half-pipe clip's last layer phase; +0x10: the spin it still owes
    pub phase: f32,
    pub owed: f32,
    /// +0x14 / +0x18: the spin band (upper, lower), +0x1c its floor
    pub spin_hi: f32,
    pub spin_lo: f32,
    pub spin_floor: f32,
    /// +0x20 / +0x24 / +0x28: the flip band and floor
    pub flip_hi: f32,
    pub flip_lo: f32,
    pub flip_floor: f32,
    /// +0x2c / +0x30: spin and flip rates, rad/s
    pub w_spin: f32,
    pub w_flip: f32,
    /// +0x34: angle still to turn towards the landing slope, about the axis at +0x40
    pub align: f32,
    pub align_axis: [f32; 3],
    /// +0x50 / +0x54: the largest error the auto-complete may still use (spin, flip)
    pub emax_spin: f32,
    pub emax_flip: f32,
    /// +0x58: started from the stick (the second pair of direction fields is read)
    pub x58: u32,
    /// +0x5c: the late wind-up started; +0x60 its flip sign; +0x64 toggled by the half-pipe turn
    pub x5c: u32,
    pub x60: u32,
    pub x64: u32,
}

/// What `SpinState_Update` reads and writes of the rider (st+0x70).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpinRider {
    /// +0x12c time scale; +0x57b8 time to touch down (s)
    pub timescale: f32,
    pub land: f32,
    /// +0x170 orientation quaternion (w, x, y, z)
    pub quat: V4,
    /// +0x1d0 / +0x1d4 spin and flip so far (rad)
    pub spin: f32,
    pub flip: f32,
    /// stats bytes +0x1a (tricks) and +0x10, stats word +0x40 (scores the spin mirrored when set)
    pub tricks: u8,
    pub stat10: u8,
    pub mirror: u32,
    /// +0x1b4 riding switch
    pub switch: u32,
    /// +0x274 / +0x278 the late wind-up (spin, flip), in thirds of a turn
    pub wind_spin: f32,
    pub wind_flip: f32,
    /// +0x1f0, +0x1f4, +0x1f8
    pub x1f0: f32,
    pub x1f4: f32,
    pub x1f8: f32,
    /// +0x420 board class (2 alpine)
    pub class: u32,
    /// +0x57d0, +0x57e0, +0x4ab0: the half-pipe's facing, its axis and the rider's place on it
    pub hp_face: V4,
    pub hp_axis: V4,
    pub hp_at: V4,
    /// +0x150 velocity, +0x140 position (cm)
    pub vel: V4,
    pub pos: V4,
    /// the bone the turn pivots on: rider + bone index (+0x44d8) × 0x394 + 0x8c8, x y z
    pub bone: [f32; 3],
    /// +0x46d4, +0x46e0: the animation's stance and heading
    pub x46d4: u32,
    pub x46e0: f32,
    /// +0x5800
    pub x5800: f32,
}

/// What the animation controller (rider+0x46c0) and the trick clips answer when asked.
#[derive(Clone, Copy, Debug, Default)]
pub struct Anim {
    /// `AnimCtl_GetClip`(layer 0), `AnimCtl_GetClipType`(layer 0), `AnimCtl_GetLayerPhase`(layer 0)
    pub clip: u32,
    pub clip_type: u32,
    pub phase: f32,
    /// `SpinState_UpdateTrickClips`' and `SpinState_StartFromStick`'s results
    pub trick_clips: u32,
    pub start_from_stick: u32,
}

/// The calls `SpinState_Update` makes, in order (floats as bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinCall {
    /// `Boarder_UpdateOrientation` 0x129008
    Orientation,
    /// `Score_UpdateSpin` 0x155e00 / `Score_UpdateFlip` 0x155e68 with whole degrees
    ScoreSpin(i32),
    ScoreFlip(i32),
    /// `SpinState_PlayExitClip` 0x102800
    ExitClip,
    /// `SpinState_UpdateTrickClips` 0x1272e8(pad bit 9, the pad's low byte)
    TrickClips(u32, i32),
    /// `Boarder_UpdateLeanAnim` 0x127918(x, y)
    Lean(u32, u32),
    /// `AnimCtl_PlayClip` 0x15fcf8(clip, 0, 5)
    PlayClip(u32),
    /// `AnimCtl_TurnAllLayers` 0x15eef8(angle)
    TurnLayers(u32),
    /// `SpinState_StartFromStick` 0x102158
    StartFromStick,
    /// `SpinControl_CalcLandingAlign` 0x11db00
    LandingAlign,
    /// 0x127d08 (the wind-up clip's mask)
    WindMask,
    /// `Boarder_RequestReset` 0x118f10(delay)
    Reset(u32),
}

/// Floor as the game takes it: truncate, then one less if that went up.
fn floor(x: f32) -> f32 {
    let f = x as i32 as f32;
    if x < f { f - 1.0 } else { f }
}

/// The angle wrapped by whole turns: x − floor(x/2π + ½)·2π.
fn wrap(x: f32) -> f32 {
    x - floor(x * 0.159_154_94 + 0.5) * TAU
}

/// `Quat_Multiply` 0x102e80 (w, x, y, z), in its order of operations.
pub fn quat_mul(a: V4, b: V4) -> V4 {
    [
        ((a[0] * b[0] - a[1] * b[1]) - a[2] * b[2]) - a[3] * b[3],
        ((a[0] * b[1] + a[1] * b[0]) + a[2] * b[3]) - a[3] * b[2],
        ((a[0] * b[2] + a[2] * b[0]) + a[3] * b[1]) - a[1] * b[3],
        ((a[0] * b[3] + a[3] * b[0]) + a[1] * b[2]) - a[2] * b[1],
    ]
}

/// A dot product as the vector unit forms it (vmul, then vaddabc / vmaddabc / vmaddbc).
fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn scale4(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

/// `Vec3_Scale` 0x102f80
fn scale3(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// v rotated by the quaternion q (w, x, y, z): v + 2w(u × v) + 2u × (u × v), as the game spells it out.
fn rotate(q: V4, v: [f32; 3]) -> [f32; 3] {
    let (w, ux, uy, uz) = (q[0], q[1], q[2], q[3]);
    let c = [uy * v[2] - uz * v[1], uz * v[0] - ux * v[2], ux * v[1] - uy * v[0]];
    let d = [uy * c[2] - uz * c[1], uz * c[0] - ux * c[2], ux * c[1] - uy * c[0]];
    let s1 = scale3(c, w + w);
    let t = [v[0] + s1[0], v[1] + s1[1], v[2] + s1[2]];
    let s2 = scale3(d, 2.0);
    [t[0] + s2[0], t[1] + s2[1], t[2] + s2[2]]
}

/// Clamped to [lo, hi] in the game's order: below lo gives lo, else above hi gives hi.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if lo <= x {
        if x <= hi { x } else { hi }
    } else {
        lo
    }
}

/// x moved towards `to` by at most `step`, as the game writes it.
fn approach(to: f32, x: f32, step: f32) -> f32 {
    if to + step < x {
        x - step
    } else if x < to - step {
        x + step
    } else {
        to
    }
}

/// `SpinState_Update` 0x100908 for one tick with the pad word `pad`; returns the calls it makes, in order.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the game's own comparisons, NaN behaviour included
pub fn spin_update(pad: u32, st: &mut SpinObj, r: &mut SpinRider, anim: &Anim) -> Vec<SpinCall> {
    let mut calls = Vec::new();
    let dt = r.timescale * 0.016_666_668;
    // turning towards the landing slope, at most 80°/s of what time is left
    if 0.0 < r.land && st.align != 0.0 {
        let a = st.align;
        let step = clamp(a, r.land * -1.396_263_4, r.land * 1.396_263_4);
        let d = if dt <= r.land { r.land } else { dt };
        let step = step * (dt / d);
        st.align = a - step;
        let (s, c) = sin_cos(step * 0.5);
        let q = [c, s * st.align_axis[0], s * st.align_axis[1], s * st.align_axis[2]];
        r.quat = quat_mul(r.quat, q);
        calls.push(SpinCall::Orientation);
    }
    r.spin += st.w_spin * dt;
    r.flip += st.w_flip * dt;
    let spun = if r.mirror != 0 { -r.spin } else { r.spin };
    calls.push(SpinCall::ScoreSpin((spun * 57.295_776) as i32));
    calls.push(SpinCall::ScoreFlip((r.flip * 57.295_776) as i32));
    let (mut sx, mut sy) = (pad_dir(pad, 0x12), pad_dir(pad, 0x10));
    if st.x58 != 0 && pad & 0x400 == 0 {
        sx = pad_dir(pad, 0xe);
        sy = pad_dir(pad, 0xc);
    }
    let (spin_hi, spin_lo, flip_hi, flip_lo);
    if (sx == 0.0 && sy == 0.0) || st.mode == 2 {
        if st.mode == 2 {
            // the half-pipe clips (0x23c, 0x23d) turn the rider with their own phase
            if anim.clip.wrapping_sub(0x23c) < 2 {
                let old = st.phase;
                st.phase = anim.phase;
                let mut d = (anim.phase - old) * PI;
                if r.switch != 0 {
                    d = -d;
                }
                if anim.clip == 0x23d {
                    d = -d;
                }
                r.spin += d;
                st.owed -= d;
            }
        } else {
            // the stick let go: from now on the rotation completes itself
            calls.push(SpinCall::ExitClip);
            st.mode = 2;
            st.x5c = 0;
            r.x1f8 = 0.0;
            r.x1f0 = 0.0;
            r.x1f4 = 0.0;
            if st.spin_hi < r.x1f4 {
                st.spin_hi = r.x1f4;
            } else {
                st.spin_lo = r.x1f4;
            }
            st.spin_floor = 0.0;
            if st.flip_hi < st.spin_floor {
                st.flip_hi = st.spin_floor;
            } else {
                st.flip_lo = st.spin_floor;
            }
            st.flip_floor = 0.0;
            if anim.clip.wrapping_sub(0x23c) < 2 {
                st.phase = 0.0;
                st.owed = if r.switch != 0 { -PI } else { PI };
                if anim.clip == 0x23d {
                    st.owed = -st.owed;
                }
            } else {
                st.owed = 0.0;
            }
            st.emax_flip = TAU;
            st.emax_spin = TAU;
        }
        // the spin's error: back to 0 within 40° of it, on to the half turn within 40° of 180°, else on to the
        // next half turn on the band's side
        let a = r.spin + st.owed;
        let b = a - r.wind_spin * 2.094_395_2;
        let mut e_spin = wrap(a);
        let off = (b - floor(b * 0.159_154_94 + 0.5) * TAU).abs();
        if off < 0.698_131_7 {
            e_spin = -e_spin;
        } else if 2.443_461 < off {
            let h = e_spin - PI;
            e_spin = -(h - floor(h * 0.159_154_94 + 0.5) * TAU);
        } else if 0.0 < st.spin_hi {
            if e_spin < 0.0 {
                e_spin += PI;
            }
        } else if 0.0 < e_spin {
            e_spin -= PI;
        }
        // the flip's: back within 40° of upright, else on to the next whole turn
        let f = r.flip;
        let c = f - r.wind_flip * 2.094_395_2;
        let mut e_flip = wrap(f);
        if (c - floor(c * 0.159_154_94 + 0.5) * TAU).abs() < 0.698_131_7 {
            e_flip = -e_flip;
        } else if st.flip_hi <= 0.0 {
            if 0.0 < e_flip {
                e_flip -= TAU;
            }
        } else if e_flip < 0.0 {
            e_flip += TAU;
        }
        if e_spin.abs() < st.emax_spin {
            st.emax_spin = e_spin.abs();
        }
        let cs = clamp(e_spin, -st.emax_spin, st.emax_spin);
        if e_flip.abs() < st.emax_flip {
            st.emax_flip = e_flip.abs();
        }
        let cf = clamp(e_flip, -st.emax_flip, st.emax_flip);
        let g = (r.tricks as f32 * 0.674_178_1 * 0.003_921_569 + 0.544_913_9) * 150.536_65;
        st.w_flip = g * cf * dt;
        st.w_spin = g * cs * dt;
        spin_hi = st.spin_hi;
        spin_lo = st.spin_lo;
        flip_hi = st.flip_hi;
        flip_lo = st.flip_lo;
    } else {
        // the stick held: the rates move towards the band's width at 75 rad/s²
        spin_lo = st.spin_lo;
        spin_hi = st.spin_hi;
        flip_lo = st.flip_lo;
        flip_hi = st.flip_hi;
        let ts = sx * (spin_lo + spin_hi).abs();
        let tf = sy * -(flip_lo + flip_hi).abs();
        st.w_spin = approach(ts, st.w_spin, dt * 75.0);
        st.w_flip = approach(tf, st.w_flip, dt * 75.0);
    }
    // the band narrows every tick and is held off zero by its floor on its own side
    let k = r.tricks as f32 * 0.002_014_339 * 0.003_921_569 + 0.993_011_7;
    st.spin_lo = spin_lo * k;
    st.spin_hi = spin_hi * k;
    st.flip_lo = flip_lo * k;
    st.flip_hi = flip_hi * k;
    if st.spin_lo < 0.0 {
        let n = -st.spin_floor;
        if n < st.spin_hi {
            st.spin_hi = n;
            if n < st.spin_lo {
                st.spin_lo = n;
            }
        }
    } else if st.spin_lo < st.spin_floor {
        st.spin_lo = st.spin_floor;
        if st.spin_hi < st.spin_floor {
            st.spin_hi = st.spin_floor;
        }
    }
    if st.flip_lo < 0.0 {
        let n = -st.flip_floor;
        if n < st.flip_hi {
            st.flip_hi = n;
            if n < st.flip_lo {
                st.flip_lo = n;
            }
        }
    } else if st.flip_lo < st.flip_floor {
        st.flip_lo = st.flip_floor;
        if st.flip_hi < st.flip_floor {
            st.flip_hi = st.flip_floor;
        }
    }
    st.w_spin = clamp(st.w_spin, st.spin_lo, st.spin_hi);
    st.w_flip = clamp_upper_strict(st.w_flip, st.flip_lo, st.flip_hi);
    let lean = |calls: &mut Vec<SpinCall>| calls.push(SpinCall::Lean(pad_dir(pad, 6).to_bits(), pad_dir(pad, 4).to_bits()));
    let trick_clips = SpinCall::TrickClips(((pad as i32) >> 9 & 1) as u32, pad as u8 as i32);
    match st.mode {
        1 => {
            calls.push(trick_clips);
            if anim.trick_clips == 0 {
                lean(&mut calls);
            } else {
                r.x1f4 = f32::from_bits(0x4055_5555);
                r.x1f8 = 0.0;
                r.wind_flip = 0.0;
                r.wind_spin = 0.0;
            }
        }
        0 => {
            calls.push(trick_clips);
            if anim.trick_clips == 0 && anim.clip_type != 0xc {
                lean(&mut calls);
            } else {
                st.mode = 1;
            }
        }
        2 => {
            calls.push(trick_clips);
            if anim.trick_clips == 0 {
                lean(&mut calls);
                if anim.clip.wrapping_sub(0x23c) >= 2 && anim.clip != 0x225 {
                    let settled = st.w_spin.abs() < 0.7 && st.w_flip.abs() < 0.7;
                    if settled {
                        calls.push(SpinCall::StartFromStick);
                        if anim.start_from_stick != 0 {
                            return calls;
                        }
                        if st.x5c == 0 {
                            if half_pipe_turn(st, r, &mut calls) {
                                return reset(pad, calls);
                            }
                            calls.push(SpinCall::WindMask);
                            if anim.clip == 0x223 {
                                if 0.0 < r.vel[2] {
                                    r.x1f8 = 0.0;
                                    r.x1f4 = f32::from_bits(0x400a_aaab);
                                } else {
                                    let f = r.land - 0.25;
                                    r.x1f4 = f32::from_bits(0x4055_5555);
                                    r.x1f8 = if 0.0 < f { 1.0 - f / (f + r.x5800.abs()) } else { 1.0 };
                                }
                            }
                        }
                    }
                    late_windup(pad, settled, dt, st, r, &mut calls);
                }
            } else {
                st.x5c = 0;
                r.x1f4 = f32::from_bits(0x4055_5555);
                r.x1f8 = 0.0;
            }
        }
        _ => {}
    }
    reset(pad, calls)
}

/// The flip's clamp: the game keeps the lower bound unless the rate is above it, then the upper unless the rate
/// is at most that (`flip_lo <= w`, then `flip_hi < w`).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp_upper_strict(x: f32, lo: f32, hi: f32) -> f32 {
    if lo <= x {
        if hi < x { hi } else { x }
    } else {
        lo
    }
}

/// Holding the stick again while it winds down: the late wind-up (in thirds of a turn) moves at the rider's
/// stat rate and turns the rider with it; the first press starts it (clip 0x222).
fn late_windup(pad: u32, settled: bool, dt: f32, st: &mut SpinObj, r: &mut SpinRider, calls: &mut Vec<SpinCall>) {
    let wx = pad_dir(pad, 10);
    let wy = pad_dir(pad, 8);
    if st.x5c != 0 {
        let rate = (r.stat10 as f32 * 0.654_503_1 * 0.003_921_569 + 0.547_417_64) * 1.375_008_3 * dt;
        if wx != 0.0 {
            let old = r.wind_spin;
            let v = approach(wx, old, rate);
            r.wind_spin = v;
            r.spin += (v - old) * 2.094_395_2;
        }
        if wy != 0.0 {
            let mut y = wy;
            if st.x60 != 0 {
                y = -y;
            }
            y = -y;
            let old = r.wind_flip;
            let v = approach(y, old, rate);
            r.wind_flip = v;
            r.flip += (v - old) * 2.094_395_2;
        }
        calls.push(SpinCall::WindMask);
        return;
    }
    if wx == 0.0 && wy == 0.0 {
        return;
    }
    r.wind_flip = 0.0;
    r.wind_spin = 0.0;
    calls.push(SpinCall::PlayClip(0x222));
    calls.push(SpinCall::WindMask);
    st.x5c = 1;
    let mut v = 0;
    if settled && 1.570_796_4 < wrap(r.spin).abs() {
        v = 1;
    }
    st.x60 = v;
    if st.x64 != 0 {
        st.x60 = v ^ 1;
    }
}

/// The half-pipe turn: settled, facing back into the pipe (its facing · the outward direction < 0) and slower
/// than 277.78 cm/s across it, the rider turns a half turn about the pipe's up, pivoting on the bone, and rides
/// on switch. Returns whether it turned.
fn half_pipe_turn(st: &mut SpinObj, r: &mut SpinRider, calls: &mut Vec<SpinCall>) -> bool {
    if !(r.x1f0 == 0.0 && r.class != 2) {
        return false;
    }
    let d = dot(r.hp_at, r.hp_axis);
    let out = sub(r.hp_at, scale4(r.hp_axis, d));
    let q = 1.0 / dot(out, out).sqrt();
    let n = scale4(out, q);
    if !(dot(n, r.hp_face) < 0.0) {
        return false;
    }
    if !(dot(r.hp_at, r.vel) < 277.777_77) {
        return false;
    }
    let (s1, c1) = sin_cos(r.spin * -0.5);
    let (s2, c2) = sin_cos(r.flip * -0.5);
    let t = quat_mul([c1, 0.0, 0.0, s1], [c2, s2, 0.0, 0.0]);
    let t = quat_mul(t, [0.0, 0.0, 0.0, 1.0]);
    let t = quat_mul(t, [c2, -s2, 0.0, 0.0]);
    let t = quat_mul(t, [c1, 0.0, 0.0, -s1]);
    r.quat = quat_mul(r.quat, t);
    calls.push(SpinCall::Orientation);
    // keep the bone where it was: rotate its height by the turn, take the difference, carry it into the world
    let p = [0.0, 0.0, r.bone[2]];
    let turned = rotate(t, p);
    let moved = [p[0] - turned[0], p[1] - turned[1], p[2] - turned[2]];
    let world = rotate(r.quat, moved);
    r.pos = sub(r.pos, [world[0], world[1], world[2], 0.0]);
    r.switch ^= 1;
    calls.push(SpinCall::TurnLayers(PI.to_bits()));
    let yaw = if r.switch != 0 { PI } else { 0.0 };
    r.x46d4 = r.switch;
    r.x46e0 = wrap(yaw);
    calls.push(SpinCall::PlayClip(0x225));
    st.x64 ^= 1;
    if st.align != 0.0 {
        calls.push(SpinCall::LandingAlign);
    }
    true
}

/// The pad's reset bit (0x100) asks for a reset after 101.46 ticks' delay.
fn reset(pad: u32, mut calls: Vec<SpinCall>) -> Vec<SpinCall> {
    if pad & 0x100 != 0 {
        calls.push(SpinCall::Reset(0x42ca_ebe0));
    }
    calls
}

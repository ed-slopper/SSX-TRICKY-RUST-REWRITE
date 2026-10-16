//! The 3D transition record (board row G9b2e) on the game's bytes: the screen fades and the blend of the renderer's
//! four values (its +0x130 hands them over, its +0x128 takes them back), every call out through [`FeEnv`]; checked in
//! the function runner (`tests/fe_transition.rs`).
//!
//! The record (gApp+0x72c): +0 active, +4 the mode (0 blend in, 1 blend out, 2 fade, 4 done), +8 the fade's phase
//! (0 in, 1 hold, 2 out, 3 done), +0xc / +0x10 / +0x14 the in, hold and out times (seconds), +0x18 the clock, +0x1c
//! the renderer's saved values (a kind word and four floats), +0x30 the current ones, +0x44..+0x60 the targets and
//! their deltas (pairs: target, delta), +0x64 set while a blend's values are held, +0x68 the fade's colour (three
//! words), +0x74 the fade's alpha, +0x78 the fade's blend word.

use crate::fe3dstate::{FeEnv, GAPP};
use crate::pf::Mem;
use crate::ps2f::F;

/// The fade's draw calls: the colour set (0x18df60: the drawer at 0x337fe8, the colour, f12 the alpha) and the
/// quad drawn (0x19b108: the drawer, f12..f15 the rectangle)
pub const DRAWER: u32 = 0x337fe8;
pub const SET_COLOUR: u32 = 0x18df60;
pub const DRAW_QUAD: u32 = 0x19b108;

const ONE: f32 = 1.0;

/// A word as the PS2's float
fn pf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

/// `a <= b` as `c.ole.s` compares them (the values as the EE reads them)
fn ole(a: F, b: F) -> bool {
    a.ee() <= b.ee()
}

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

/// The renderer (gApp+0x724; its vtable at +0x10): a virtual's function and the object adjusted
fn renderer_slot(m: &Mem, off: u32) -> (u32, u32) {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    (m.r32(vt.wrapping_add(off + 4)), r.wrapping_add(adj))
}

fn renderer_call(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let (f, obj) = renderer_slot(m, off);
    let mut a = vec![obj];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// 0x1fecc8 (`FE3DTransition_Construct`): phase 3, mode 4, not active, no blend held; answers the record
pub fn construct(m: &mut Mem, t: u32) -> u32 {
    m.w32(t + 8, 3);
    m.w32(t + 4, 4);
    m.w32(t, 0);
    m.w32(t + 0x64, 0);
    t
}

/// The saved values (+0x1c, five words) copied to the current ones (+0x30)
fn current_from_saved(m: &mut Mem, t: u32) {
    for k in 0..5 {
        let v = m.r32(t + 0x1c + 4 * k);
        m.w32(t + 0x30 + 4 * k, v);
    }
}

/// The blend finished: the current values the targets, phase 3, mode 4, not active
fn blend_done(m: &mut Mem, t: u32) {
    for (k, at) in [0x44u32, 0x4c, 0x54, 0x5c].iter().enumerate() {
        let v = m.r32(t + at);
        m.w32(t + 0x34 + 4 * k as u32, v);
    }
    m.w32(t + 8, 3);
    m.w32(t + 4, 4);
    m.w32(t, 0);
}

/// 0x1fece8 (`FE3DTransition_Update`, once a tick): while active, the clock on by 1/60. A blend in (0) or out (1)
/// starts from the saved values each tick and adds the elapsed fraction of the deltas (in: t × delta; out: the
/// targets less (1 - t) × delta), or ends on the targets (the out also lets the values go, +0x64), then hands them to
/// the renderer (+0x128). A fade (2) runs its phases: alpha up over the in time, 1 over the hold, down over the out
/// time, each phase handing its overshoot to the next, the last ending the record.
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t) == 0 {
        return;
    }
    let clock = pf(m, t + 0x18) + F(fl(0x3c88_8889));
    m.w32(t + 0x18, clock.0.to_bits());
    match m.r32(t + 4) {
        0 => {
            current_from_saved(m, t);
            let span = pf(m, t + 0xc);
            if ole(span, clock) {
                blend_done(m, t);
            } else {
                let f = clock / span;
                for (k, d) in [0x48u32, 0x50, 0x58, 0x60].iter().enumerate() {
                    let at = t + 0x34 + 4 * k as u32;
                    let v = pf(m, at) + f * pf(m, t + d);
                    m.w32(at, v.0.to_bits());
                }
            }
            renderer_call(m, e, 0x128, &[t + 0x30]);
        }
        1 => {
            current_from_saved(m, t);
            let span = pf(m, t + 0x14);
            if ole(span, clock) {
                blend_done(m, t);
                m.w32(t + 0x64, 0);
            } else {
                let f = F(ONE) - clock / span;
                for (k, d) in [0x48u32, 0x50, 0x58, 0x60].iter().enumerate() {
                    let at = t + 0x34 + 4 * k as u32;
                    let v = pf(m, at) - f * pf(m, t + d);
                    m.w32(at, v.0.to_bits());
                }
            }
            renderer_call(m, e, 0x128, &[t + 0x30]);
        }
        2 => match m.r32(t + 8) {
            0 => {
                let span = pf(m, t + 0xc);
                let a = if ole(span, clock) {
                    m.w32(t + 0x18, (clock - span).0.to_bits());
                    m.w32(t + 8, 1);
                    F(ONE)
                } else {
                    clock / span
                };
                m.w32(t + 0x74, a.0.to_bits());
            }
            1 => {
                let span = pf(m, t + 0x10);
                if ole(span, clock) {
                    m.w32(t + 0x18, (clock - span).0.to_bits());
                    m.w32(t + 8, 2);
                }
                m.w32(t + 0x74, ONE.to_bits());
            }
            2 => {
                let span = pf(m, t + 0x14);
                if ole(span, clock) {
                    m.w32(t + 8, 3);
                    m.w32(t + 4, 4);
                    m.w32(t + 0x18, (clock - span).0.to_bits());
                    m.w32(t + 0x74, 0);
                    m.w32(t, 0);
                } else {
                    let a = F(ONE) - clock / span;
                    m.w32(t + 0x74, a.0.to_bits());
                }
            }
            _ => {}
        },
        _ => {}
    }
}

/// An unsigned word to a float as the game converts it (halved with the low bit kept, then doubled, when its top bit
/// is set)
fn u32_to_f32(v: u32) -> f32 {
    if (v as i32) >= 0 {
        v as i32 as f32
    } else {
        let h = ((v >> 1) | (v & 1)) as i32 as f32;
        h + h
    }
}

/// 0x1ff000 (the fade's draw; indexed as `VideoStream_Draw`; `buf` 0x10 bytes): during a fade, the renderer set for a
/// full-screen quad (+0xd8, +0x1a0, the viewport from its size (+0x58, +0x60) by +0x98, the 640 × 480 projection
/// with depths 0..1000 by +0xb0, +0x108, +0x1d0(0x13), +0x1c0(1), +0x1c8(2, 0, 0.08), the blend +0x1e0(+0x78),
/// +0x1f0(-1, 0)), the fade's colour and alpha (0x18df60), the quad (0x19b108: 0, 0, 640, 480), +0x1a8 and +0xe0
pub fn draw_fade(m: &mut Mem, e: &mut dyn FeEnv, t: u32, buf: u32) {
    if m.r32(t) == 0 || m.r32(t + 4) != 2 {
        return;
    }
    renderer_call(m, e, 0xd8, &[]);
    renderer_call(m, e, 0x1a0, &[]);
    let (vp, vp_obj) = renderer_slot(m, 0x98);
    let w = u32_to_f32(renderer_call(m, e, 0x58, &[]));
    let h = u32_to_f32(renderer_call(m, e, 0x60, &[]));
    e.call(m, vp, &[vp_obj, 0, 0, w.to_bits(), h.to_bits()]);
    renderer_call(m, e, 0xb0, &[0, 0, 640.0f32.to_bits(), 480.0f32.to_bits(), 0, 1000.0f32.to_bits()]);
    renderer_call(m, e, 0x108, &[]);
    renderer_call(m, e, 0x1d0, &[0x13]);
    renderer_call(m, e, 0x1c0, &[1]);
    renderer_call(m, e, 0x1c8, &[2, 0, 0.08f32.to_bits()]);
    let blend = m.r32(t + 0x78);
    renderer_call(m, e, 0x1e0, &[blend]);
    renderer_call(m, e, 0x1f0, &[u32::MAX, 0]);
    for k in 0..3 {
        let v = m.r32(t + 0x68 + 4 * k);
        m.w32(buf + 4 * k, v);
    }
    let alpha = m.r32(t + 0x74);
    e.call(m, SET_COLOUR, &[DRAWER, buf, alpha]);
    e.call(m, DRAW_QUAD, &[DRAWER, 0, 0, 640.0f32.to_bits(), 480.0f32.to_bits()]);
    renderer_call(m, e, 0x1a8, &[]);
    renderer_call(m, e, 0xe0, &[]);
}

/// 0x1ff6a0 (`FE3DTransition_StopFade`): a fade or its end (mode 2 or 3) no longer active
pub fn stop_fade(m: &mut Mem, t: u32) {
    let mode = m.r32(t + 4) as i32;
    if (2..4).contains(&mode) {
        m.w32(t, 0);
    }
}

/// 0x1ff2b0 (`FE3DTransition_StartIn`; f12 the time): a blend in. While a blend's values are held (+0x64) only an
/// active blend out turns round (its values kept); otherwise any fade is stopped, the renderer's values saved (+0x130)
/// and taken as the current ones. The targets by the saved kind word: 0 everything toward (0.125, 0.75, 0.75, 0.75),
/// 1 the saved values with no change, 2 the first two toward (0.125, 0.75) and the last two kept
pub fn start_in(m: &mut Mem, e: &mut dyn FeEnv, t: u32, time: f32) {
    let mut turning = false;
    if m.r32(t + 0x64) != 0 {
        if m.r32(t) == 0 || m.r32(t + 4) != 1 {
            return;
        }
        turning = true;
    }
    if m.r32(t) != 0 && !turning {
        stop_fade(m, t);
    }
    m.w32(t + 0xc, time.to_bits());
    m.w32(t + 8, 0);
    m.w32(t + 4, 0);
    m.w32(t, 1);
    m.w32(t + 0x64, 1);
    m.w32(t + 0x18, 0);
    if !turning {
        let v = renderer_call(m, e, 0x130, &[]);
        for k in 0..5 {
            let x = m.r32(v.wrapping_add(4 * k));
            m.w32(t + 0x1c + 4 * k, x);
        }
        current_from_saved(m, t);
    }
    let s = |m: &Mem, k: u32| pf(m, t + 0x20 + 4 * k);
    let (eighth, three_q) = (F(0.125), F(0.75));
    match m.r32(t + 0x1c) {
        0 => {
            let d = [eighth - s(m, 0), three_q - s(m, 1), three_q - s(m, 2), three_q - s(m, 3)];
            for (k, (tg, dl)) in [(0x44u32, 0x48u32), (0x4c, 0x50), (0x54, 0x58), (0x5c, 0x60)].iter().enumerate() {
                let target = if k == 0 { eighth } else { three_q };
                m.w32(t + tg, target.0.to_bits());
                m.w32(t + dl, d[k].0.to_bits());
            }
        }
        1 => {
            for (k, (tg, dl)) in [(0x44u32, 0x48u32), (0x4c, 0x50), (0x54, 0x58), (0x5c, 0x60)].iter().enumerate() {
                let v = m.r32(t + 0x20 + 4 * k as u32);
                m.w32(t + tg, v);
                m.w32(t + dl, 0);
            }
        }
        2 => {
            let d0 = eighth - s(m, 0);
            let d1 = three_q - s(m, 1);
            let (s2, s3) = (m.r32(t + 0x28), m.r32(t + 0x2c));
            m.w32(t + 0x54, s2);
            m.w32(t + 0x48, d0.0.to_bits());
            m.w32(t + 0x50, d1.0.to_bits());
            m.w32(t + 0x5c, s3);
            m.w32(t + 0x60, 0);
            m.w32(t + 0x44, eighth.0.to_bits());
            m.w32(t + 0x4c, three_q.0.to_bits());
            m.w32(t + 0x58, 0);
        }
        _ => {}
    }
}

/// 0x1ff4c8 (`FE3DTransition_StartOut`; f12 the time): with a blend's values held, a blend out back to the saved
/// values from the current ones (a fade is left running; any other active mode but a blend in is stopped as a fade)
pub fn start_out(m: &mut Mem, t: u32, time: f32) {
    if m.r32(t + 0x64) == 0 {
        return;
    }
    if m.r32(t) != 0 {
        match m.r32(t + 4) {
            0 => {}
            2 => return,
            _ => stop_fade(m, t),
        }
    }
    let s: Vec<u32> = (0..4).map(|k| m.r32(t + 0x20 + 4 * k)).collect();
    let c: Vec<u32> = (0..4).map(|k| m.r32(t + 0x34 + 4 * k)).collect();
    for (k, dl) in [0x48u32, 0x50, 0x58, 0x60].iter().enumerate() {
        m.w32(t + dl, (F(fl(s[k])) - F(fl(c[k]))).0.to_bits());
    }
    m.w32(t + 8, 2);
    m.w32(t, 1);
    m.w32(t + 0x14, time.to_bits());
    m.w32(t + 4, 1);
    m.w32(t + 0x18, 0);
    for (k, tg) in [0x44u32, 0x4c, 0x54, 0x5c].iter().enumerate() {
        m.w32(t + tg, s[k]);
    }
}

/// 0x1ff590 (`buf` 0x14 bytes): with a blend's values held, the renderer given the saved values with the second 0
/// and the third 1.0
pub fn hand_saved(m: &mut Mem, e: &mut dyn FeEnv, t: u32, buf: u32) {
    if m.r32(t + 0x64) == 0 {
        return;
    }
    for k in 0..5 {
        let v = m.r32(t + 0x1c + 4 * k);
        m.w32(buf + 4 * k, v);
    }
    m.w32(buf + 8, ONE.to_bits());
    m.w32(buf + 4, 0);
    renderer_call(m, e, 0x128, &[buf]);
}

/// 0x1ff608: with a blend's values held, the renderer given the current values
pub fn hand_current(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t + 0x64) == 0 {
        return;
    }
    renderer_call(m, e, 0x128, &[t + 0x30]);
}

/// 0x1ff648 (`FE3DTransition_Start`; f12 / f13 / f14 the in, hold and out times, the colour (three words), the blend
/// word): a fade when none is running
pub fn start(m: &mut Mem, t: u32, times: [f32; 3], colour: u32, blend: u32) {
    if m.r32(t) != 0 {
        return;
    }
    m.w32(t + 4, 2);
    m.w32(t, 1);
    for (k, v) in times.iter().enumerate() {
        m.w32(t + 0xc + 4 * k as u32, v.to_bits());
    }
    m.w32(t + 0x18, 0);
    m.w32(t + 8, 0);
    for k in 0..3 {
        let v = m.r32(colour.wrapping_add(4 * k));
        m.w32(t + 0x68 + 4 * k, v);
    }
    m.w32(t + 0x78, blend);
}

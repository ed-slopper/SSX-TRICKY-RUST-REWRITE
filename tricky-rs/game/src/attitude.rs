//! The attitude icon over the camera's rider (board row G7g5): `AttitudeIcon_Draw` 0x134830. On the bytes at
//! their addresses ([`Mem`]); the camera's rider (`Camera_GetFocusRider` 0x174008), the career's rider of a
//! player (`Career_FindRiderOfPlayer` 0x166598) and the renderer's virtuals are calls out
//! (`riderdraw::RiderDrawEnv`). Checked in the function runner (`tests/attitude.rs`).
//!
//! **The icon**: +0..+0x3f four points it moves along, then two slots of 0x80 bytes (slot 1 when the camera's
//! rider is not the career player's). **A slot**: +0x50 how far along the points (0..3) and +0x54 the
//! squash's two scales (as the draw makes them), +0x60 where it is, +0x70 its size, +0x78 and +0x80 two words
//! for the sprite, +0x88 its colour (the alpha first), +0x98 whether it is shown, +0xa0 whether it is
//! squashing, +0xa4 how far, +0xac whether it is moving, +0xb0 how far, +0xb8 a settle, +0xbc whether it is
//! fixed, +0xc0 its kind (1: the second point, 2: the fourth, else the first).
//!
//! The game works in a few stack vectors; the port in local values (no stack bytes are handed on).

use crate::animseq::div;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::riderdraw::{RiderDrawEnv, APP, CAREER, FOCUS_RIDER};

type V = [F; 4];

pub const FIND_RIDER_OF_PLAYER: u32 = 0x166598;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rq(m: &Mem, a: u32) -> V {
    [0, 1, 2, 3].map(|k| rf(m, a.wrapping_add(4 * k)))
}
fn f(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn vcall(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let func = m.r32(vt + off + 4);
    env.vcall(m, func, this, args)
}
fn sub(a: V, b: V) -> V {
    [0, 1, 2, 3].map(|k| a[k] - b[k])
}
fn add(a: V, b: V) -> V {
    [0, 1, 2, 3].map(|k| a[k] + b[k])
}
/// `Vec4_Scale` 0x102f50: every lane times `s`
fn scale(a: V, s: F) -> V {
    a.map(|x| x * s)
}

/// The point `u` (0..3) along the icon's four points: between the first two below 1, the next two below 2, else
/// the last two
fn along(m: &Mem, icon: u32, u: F) -> V {
    let one = F(1.0);
    let (k, t) = if u < one {
        (0, u)
    } else if u < F(2.0) {
        (1, u - one)
    } else {
        (2, u - F(2.0))
    };
    let a = rq(m, icon + 0x10 * k);
    let b = rq(m, icon + 0x10 * (k + 1));
    add(a, scale(sub(b, a), t))
}

/// The point the kind picks: 1 the second, 2 the fourth, else the first
fn kind_point(m: &Mem, icon: u32, kind: u32) -> V {
    match kind as i32 {
        1 => rq(m, icon + 0x10),
        2 => rq(m, icon + 0x30),
        _ => rq(m, icon),
    }
}

/// `AttitudeIcon_Draw` 0x134830(icon): the slot's sprite, when it is shown and moving or fixed, and its place is
/// before the camera and nearer than 3700:
/// - **moving**: its fade t (+0xb0) is t / 0.1 up to 0.1, then down from 1 over the next 0.9. Its place is the
///   point +0x50 along the icon's points, or (fixed as well) that pulled from the kind's point by the fade, its
///   scales 1 + 0.3 × fade. The colour's first lane is 0.7778 × the fade, or moved toward 0.7778 by it;
/// - **squashing** (+0xa0): at the kind's point, the two scales in three steps over +0xa4 (the settle cleared);
/// - **settling**: at the kind's point, both scales 1 + 0.3408 × the settle, the first lane moved toward 0.7778
///   by it.
///
/// The colour is put at the slot's +0x88. The size (+0x70, +0x74) is 33.2, less nearer than 500 and more past
/// 3000 (both as depth / the bound), times the scales. Past 2777.4 the alpha falls to 0 at 3700. The renderer
/// gets +0x1a0, +0x1c8(0, 2, 0.01), +0x1e8(2, 0), texture app +0x30 → +0x94, then +0x240 with the slot's
/// +0x60, +0x70, +0x78, +0x80, +0x88, and +0x1a8.
pub fn draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, icon: u32) {
    let game = m.r32(m.r32(APP) + 0x730);
    let k = m.r32(game + 0x290) << 7;
    let who = m.r32(game.wrapping_add(k) + 0xa0);
    let focus = env.call(m, FOCUS_RIDER, &[who]);
    let player = env.call(m, FIND_RIDER_OF_PLAYER, &[CAREER, 0]);
    let slot = icon + ((m.r32(focus + 0x460) != player) as u32) * 0x80;
    if m.r32(slot + 0x98) == 0 {
        return;
    }
    if m.r32(slot + 0xbc) == 0 && m.r32(slot + 0xac) == 0 {
        return;
    }
    let r = m.r32(m.r32(APP) + 0x724);
    let view = vcall(m, env, r, 0xf8, &[]);
    let v = [0, 1, 2, 3].map(|i| rq(m, view.wrapping_add(0x10 * i)));
    let p = rq(m, slot + 0x60);
    let z = ((v[0][2] * p[0] + v[1][2] * p[1]) + v[2][2] * p[2]) + v[3][2] * p[3];
    if z <= F(0.0) || f(0x4567_4141) <= z {
        return;
    }
    let one = F(1.0);
    let top = f(0x3f47_21d5);
    let mut scales = [one, one];
    let mut c: V;
    if m.r32(slot + 0xac) != 0 {
        let t = rf(m, slot + 0xb0);
        let tenth = f(0x3dcc_cccd);
        let fade = if t < tenth { div(t, tenth) } else { one - div(t - tenth, f(0x3f66_6666)) };
        if m.r32(slot + 0xbc) == 0 {
            let at = along(m, icon, rf(m, slot + 0x50));
            c = at;
            c[0] = fade * top;
        } else {
            let q = kind_point(m, icon, m.r32(slot + 0xc0));
            let at = along(m, icon, rf(m, slot + 0x50));
            c = add(q, scale(sub(at, q), fade));
            let s = fade * f(0x3e99_9998) + one;
            scales = [s, s];
            c[0] = c[0] + fade * (top - c[0]);
        }
    } else if m.r32(slot + 0xa0) != 0 {
        c = kind_point(m, icon, m.r32(slot + 0xc0));
        let t = rf(m, slot + 0xa4);
        let third = f(0x3eaa_aaab);
        let (a, b) = if t < third {
            let s = t * F(3.0);
            (s * F(0.0) + f(0x3e4c_cccd), s * f(0x3f8d_0964) + f(0x3e99_999a))
        } else if t < f(0x3f2a_aaab) {
            let s = (t - third) * F(3.0);
            (s * f(0x3fb2_faf3) + f(0x3e4c_cccd), s * f(0xbf77_b315) + f(0x3fb3_6fca))
        } else {
            let s = (t - f(0x3f2a_aaab)) * F(3.0);
            (s * f(0xbf19_291a) + f(0x3fcc_948d), s * f(0x3f10_d381) + f(0x3ede_58fe))
        };
        scales = [a, b];
        m.w32(slot + 0xb8, 0);
    } else {
        c = kind_point(m, icon, m.r32(slot + 0xc0));
        let settle = rf(m, slot + 0xb8);
        let s = settle * f(0x3eae_83f4) + one;
        scales = [s, s];
        c[0] = c[0] + settle * (top - c[0]);
    }
    for k in 0..4 {
        wf(m, slot + 0x88 + 4 * k, c[k as usize]);
    }
    vcall(m, env, r, 0x1a0, &[]);
    vcall(m, env, r, 0x1c8, &[0, 2, 0x3c23_d70a]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let tex = m.r32(m.r32(m.r32(APP) + 0x30) + 0x94);
    vcall(m, env, r, 0x1f0, &[tex, 0]);
    let size = f(0x4204_d137);
    let near = F(500.0);
    let far = f(0x453b_8000);
    let s = if z <= near {
        div(z, near) * size
    } else if far <= z {
        div(z, far) * size
    } else {
        size
    };
    wf(m, slot + 0x74, s);
    wf(m, slot + 0x70, s);
    let x = rf(m, slot + 0x70) * scales[0];
    wf(m, slot + 0x70, x);
    let y = rf(m, slot + 0x74) * scales[1];
    wf(m, slot + 0x74, y);
    let fadeout = f(0x452d_966a);
    if fadeout < z {
        let k = one - div(z - fadeout, f(0x4466_ab5c));
        let a = rf(m, slot + 0x88) * k;
        wf(m, slot + 0x88, a);
    }
    vcall(m, env, r, 0x240, &[slot + 0x60, slot + 0x70, slot + 0x78, slot + 0x80, slot + 0x88]);
    vcall(m, env, r, 0x1a8, &[]);
}

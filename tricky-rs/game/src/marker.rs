//! The player marker over a rider in a race of two or more players (board row G7g5): `PlayerMarker_Draw`
//! 0x133c20. On the bytes at their addresses ([`Mem`]); the renderer's virtuals are calls out
//! (`particledraw::ParticleDrawEnv`). Checked in the function runner (`tests/marker.rs`).
//!
//! **A marker**: +0..+0x3f its four points, +0x20 also where it is (the third point), +0xc0 its eight vertices
//! (0x30 each, the places at +0x20: the corners are written there), +0x24c its height.

use crate::animseq::div;
use crate::particledraw::{vcall, ParticleDrawEnv, APP, BIAS};
use crate::pf::Mem;
use crate::ps2f::F;

type V = [F; 4];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn rq(m: &Mem, a: u32) -> V {
    [0, 1, 2, 3].map(|k| rf(m, a.wrapping_add(4 * k)))
}
fn wq(m: &mut Mem, a: u32, v: V) {
    for k in 0..4 {
        m.w32(a + 4 * k, v[k as usize].0.to_bits());
    }
}
fn f(bits: u32) -> F {
    F(f32::from_bits(bits))
}

/// `PlayerMarker_Draw` 0x133c20(marker): the marker's texture (the app's +0x30 → +0x84) and the renderer's state
/// for it, then the marker as a strip of eight corners facing the camera:
/// - its size by its depth (the third point, +0x20, through the view's third row): 76.49 at 0 or nearer, then
///   76.49 + 63.37 × z / 31128.5, 139.86 from 31128.5 on;
/// - the camera's first column times the size, and its third column times the height (+0x24c), each w 0;
/// - each point less then plus the first, plus the second, written at the vertices' places (+0xe0, +0x110, …);
/// - the renderer's +0x220 (8, the vertices, 0), then the state back.
pub fn draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, marker: u32) {
    let app = m.r32(APP);
    let r = m.r32(app + 0x724);
    let tex = m.r32(m.r32(app + 0x30) + 0x84);
    vcall(m, env, r, 0x1f0, &[tex, 0]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    vcall(m, env, r, 0x1e0, &[3]);
    let view = vcall(m, env, r, 0xf8, &[]);
    let v = [0, 1, 2, 3].map(|i| rq(m, view.wrapping_add(0x10 * i)));
    let p = rq(m, marker + 0x20);
    let z = ((p[0] * v[2][0] + p[1] * v[2][1]) + p[2] * v[2][2]) + p[3] * v[2][3];
    let zero = F(0.0);
    let size = if z <= zero {
        f(0x4298_f9a5)
    } else if f(0x46f3_3115) <= z {
        f(0x430b_db37)
    } else {
        div(z, f(0x46f3_3115)) * f(0x427d_7992) + f(0x4298_f9a5)
    };
    // the first column ((1, 0, 0, 0) through the view, the 1 folded) and the third ((0, 0, 1, 0))
    let side: V = v.map(|row| ((row[0] + row[1] * zero) + row[2] * zero) + row[3] * zero);
    let up: V = v.map(|row| ((zero * row[0] + zero * row[1]) + F(1.0) * row[2]) + row[3] * zero);
    let mut side = side.map(|x| x * size);
    side[3] = zero;
    let height = rf(m, marker + 0x24c);
    let mut up = up.map(|x| x * height);
    up[3] = zero;
    for k in 0..4u32 {
        let pt = rq(m, marker + 0x10 * k);
        let a: V = [0, 1, 2, 3].map(|j| (pt[j] - side[j]) + up[j]);
        wq(m, marker + 0xe0 + 0x60 * k, a);
        let pt = rq(m, marker + 0x10 * k);
        let b: V = [0, 1, 2, 3].map(|j| (pt[j] + side[j]) + up[j]);
        wq(m, marker + 0x110 + 0x60 * k, b);
    }
    vcall(m, env, r, 0x220, &[8, marker + 0xc0, 0]);
    vcall(m, env, r, 0x1e0, &[3]);
    vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
    vcall(m, env, r, 0x1c0, &[0]);
    vcall(m, env, r, 0x1e8, &[0, 0]);
}

//! The rider's streaks (board row G7g5): `Streak_Draw` 0x1f08d0, the hand and board streaks the rider's draw
//! hands two pairs of points (`riderdraw`). On the bytes at their addresses ([`Mem`]); the renderer's virtuals
//! are calls out (`particledraw::ParticleDrawEnv`). Checked in the function runner (`tests/streak.rs`).
//!
//! **A streak**: 7 entries of 0x40 bytes (the places of the two strips' two edges: +0, +0x10 the first strip's,
//! +0x20, +0x30 the second's), +0x1c0 the newest's place in the ring, +0x1c4 how many are live, +0x1c8 the colour
//! (four floats: the edge's alpha, then red, green, blue).
//!
//! The game builds the strips' vertices on its stack (0x30 bytes each, as the renderer's +0x220 takes them: +0x10
//! the colour as integers, +0x20 the place; the first 0x10 bytes are never written). The port builds them in
//! `scratch` (STANDIN: the frame's place; the same bytes at the same offsets, from +0x10; 0x310 bytes for the most a strip has, 16 vertices).

use crate::particledraw::{vcall, ParticleDrawEnv, APP, BIAS};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::sndlib::cvt_w;

/// The fade along a streak (8 floats: 0.65 × 0.7^k, then 0)
pub const FADE: u32 = 0x33f398;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn byte(x: F) -> u32 {
    cvt_w((x * F(128.0)).0) as u32
}
fn copy(m: &mut Mem, to: u32, from: u32) {
    for k in 0..4 {
        let w = m.r32(from + 4 * k);
        m.w32(to + 4 * k, w);
    }
}

/// `Streak_Draw` 0x1f08d0(streak, points): the streak's texture (the app's +0x30 → +0x74, or −1 with none), the
/// renderer's state for it, then two strips. Each starts at the two points it is handed (`points` +0 and +0x10,
/// then +0x20 and +0x30), its inner edge with the fade's alpha from 7 − the live count on, its outer with the
/// colour's own. Then each live entry from the newest back (the ring of 7 wrapping): its inner edge (+0, then +0x20)
/// with the next fade, its outer (+0x10, then +0x30). Each strip goes to the renderer's +0x220 (its vertices, the
/// vertices, 0); then the state goes back.
pub fn streak_draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, streak: u32, points: u32, scratch: u32) {
    let app = m.r32(APP);
    let r = m.r32(app + 0x724);
    let tex = m.r32(app + 0x30);
    let sprite = if tex == 0 { u32::MAX } else { m.r32(tex + 0x74) };
    vcall(m, env, r, 0x1f0, &[sprite, 0]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1e0, &[3]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let c = streak + 0x1c8;
    for strip in 0..2u32 {
        let n = m.r32(streak + 0x1c4) as i32;
        let fade = FADE.wrapping_add((7i32.wrapping_sub(n) << 2) as u32);
        let (red, green, blue) = (rf(m, c + 4), rf(m, c + 8), rf(m, c + 0xc));
        let inner = |m: &Mem, at: u32| [byte(red), byte(green), byte(blue), byte(rf(m, at))];
        let outer = |m: &Mem| [byte(rf(m, c + 4)), byte(rf(m, c + 8)), byte(rf(m, c + 0xc)), byte(rf(m, c))];
        let put = |m: &mut Mem, k: u32, colour: [u32; 4], place: u32| {
            let v = scratch + 0x10 + 0x30 * k;
            for (j, w) in colour.into_iter().enumerate() {
                m.w32(v + 0x10 + 4 * j as u32, w);
            }
            copy(m, v + 0x20, place);
        };
        let col = inner(m, fade);
        put(m, 0, col, points + 0x20 * strip);
        let col = outer(m);
        put(m, 1, col, points + 0x20 * strip + 0x10);
        let mut count = 2u32;
        let mut at = (m.r32(streak + 0x1c0) as i32).wrapping_sub(1);
        let mut k = 0i32;
        while n > 0 {
            if !(-1 < at) {
                at = 6;
            }
            let entry = streak.wrapping_add((at << 6) as u32);
            let col = inner(m, fade.wrapping_add(4 * k as u32));
            put(m, count, col, entry + 0x20 * strip);
            let col = outer(m);
            put(m, count + 1, col, entry + 0x20 * strip + 0x10);
            count += 2;
            at = at.wrapping_sub(1);
            k += 1;
            if !(k < m.r32(streak + 0x1c4) as i32) {
                break;
            }
        }
        vcall(m, env, r, 0x220, &[count, scratch + 0x10, 0]);
    }
    vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
    vcall(m, env, r, 0x1c0, &[0]);
    vcall(m, env, r, 0x1e8, &[0, 0]);
}

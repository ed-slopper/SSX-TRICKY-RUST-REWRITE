//! The 2D quad helpers (board row G7m) that the HUD, the front end and the replay screens draw with:
//! `Draw_FilledRect` 0x19b108, `FUN_0019b2a0` (corner colours), `Sprite_SubmitGradientQuad` 0x19b4d8,
//! `Draw_TexturedRect` 0x19b730, `Sprite_SubmitQuad` 0x19bdf8 and `Sprite_SubmitGradientQuadRotated` 0x19bf90.
//! With them the line draws (board row G7p): `Draw_Line` 0x19c1e8, `FUN_0019c310` (a colour each end),
//! `FUN_0019c458` (a rectangle's outline) and `FUN_0019c620` (a triangle). On the bytes at their addresses
//! ([`Mem`]); the renderer's +0x220 (strips) and +0x238 (lines) are calls out ([`ParticleDrawEnv`]). Checked in
//! the function runner (`tests/quad2d.rs`).
//!
//! All six are one quad: the rectangle (x, y, w, h) as four vertices, (x, y), (x + w, y), (x, y + h),
//! (x + w, y + h), handed to the renderer's +0x220 (4, the vertices, 0). A vertex is as `fxdraw`'s: u, v, 1.0
//! (+0xc unwritten); the colour (the floats at +4, +8, +0xc, +0 × 128 as integers); the place (x, y, z, 1). z is
//! the first colour's +0x10. They differ in their colours (one, or one a corner) and their texture coordinates.
//! Each first waits in an empty loop of four rounds, which the port leaves out (nothing can see it).
//!
//! The game builds the vertices on its stack; the port builds them in `scratch` (STANDIN: 0xc0 bytes, the same
//! bytes at the same offsets).

use crate::particledraw::{vcall, ParticleDrawEnv, APP};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::sndlib::cvt_w;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}

/// The texture coordinates: f16..f19 as the game passes them
pub type Tex = [F; 4];

/// The corners' u, v: 0 to 1
const WHOLE: [(F, F); 4] = [(F(0.0), F(0.0)), (F(1.0), F(0.0)), (F(0.0), F(1.0)), (F(1.0), F(1.0))];
/// u from f17 to f18, v from f16 to f19
fn straight(t: Tex) -> [(F, F); 4] {
    [(t[1], t[0]), (t[2], t[0]), (t[1], t[3]), (t[2], t[3])]
}
/// The same rectangle a quarter turned: u f18 then f17 down the sides, v f16 then f19 across
fn turned(t: Tex) -> [(F, F); 4] {
    [(t[2], t[0]), (t[2], t[3]), (t[1], t[0]), (t[1], t[3])]
}

/// The quad: corner k coloured by `colours[k]`, z from `zsrc`'s +0x10
fn quad(m: &mut Mem, env: &mut dyn ParticleDrawEnv, zsrc: u32, colours: [u32; 4], x: F, y: F, w: F, h: F, uv: [(F, F); 4], scratch: u32) {
    let z = rf(m, zsrc + 0x10);
    let (x1, y1) = (x + w, y + h);
    let places = [(x, y), (x1, y), (x, y1), (x1, y1)];
    for k in 0..4 {
        let at = scratch + 0x30 * k as u32;
        wf(m, at, uv[k].0);
        wf(m, at + 4, uv[k].1);
        wf(m, at + 8, F(1.0));
        for (j, c) in [4u32, 8, 0xc, 0].into_iter().enumerate() {
            let v = cvt_w((rf(m, colours[k] + c) * F(128.0)).0) as u32;
            m.w32(at + 0x10 + 4 * j as u32, v);
        }
        for (j, v) in [places[k].0, places[k].1, z, F(1.0)].into_iter().enumerate() {
            wf(m, at + 0x20 + 4 * j as u32, v);
        }
    }
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, 0x220, &[4, scratch, 0]);
}

/// `Draw_FilledRect` 0x19b108(colour; x, y, w, h): one colour, u, v from 0 to 1
pub fn filled_rect(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, x: F, y: F, w: F, h: F, scratch: u32) {
    quad(m, env, colour, [colour; 4], x, y, w, h, WHOLE, scratch);
}

/// `FUN_0019b2a0`(zsrc, c0, c1, c2, c3; x, y, w, h): a colour a corner (in the corners' order), z from the first
/// word's +0x10, u, v from 0 to 1
pub fn gradient_rect(m: &mut Mem, env: &mut dyn ParticleDrawEnv, zsrc: u32, c: [u32; 4], x: F, y: F, w: F, h: F, scratch: u32) {
    quad(m, env, zsrc, c, x, y, w, h, WHOLE, scratch);
}

/// `Sprite_SubmitGradientQuad` 0x19b4d8(zsrc, c0, c1, c2, c3; x, y, w, h, f16..f19): as `gradient_rect`, with
/// u from f17 to f18 and v from f16 to f19
pub fn gradient_quad(m: &mut Mem, env: &mut dyn ParticleDrawEnv, zsrc: u32, c: [u32; 4], x: F, y: F, w: F, h: F, t: Tex, scratch: u32) {
    quad(m, env, zsrc, c, x, y, w, h, straight(t), scratch);
}

/// `Draw_TexturedRect` 0x19b730(colour; x, y, w, h, f16..f19): one colour, u from f17 to f18, v from f16 to f19
pub fn textured_rect(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, x: F, y: F, w: F, h: F, t: Tex, scratch: u32) {
    quad(m, env, colour, [colour; 4], x, y, w, h, straight(t), scratch);
}

/// `Sprite_SubmitQuad` 0x19bdf8(colour; x, y, w, h, f16..f19): one colour, the texture a quarter turned (u
/// f18 on the left, f17 on the right; v f16 at the top, f19 at the bottom, swapped to run along the sides)
pub fn submit_quad(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, x: F, y: F, w: F, h: F, t: Tex, scratch: u32) {
    quad(m, env, colour, [colour; 4], x, y, w, h, turned(t), scratch);
}

/// `Sprite_SubmitGradientQuadRotated` 0x19bf90(zsrc, c0, c1, c2, c3; x, y, w, h, f16..f19): the texture turned as
/// `submit_quad`'s and the colours with it: the corners take c1, c3, c0, c2
pub fn gradient_quad_turned(m: &mut Mem, env: &mut dyn ParticleDrawEnv, zsrc: u32, c: [u32; 4], x: F, y: F, w: F, h: F, t: Tex, scratch: u32) {
    quad(m, env, zsrc, [c[1], c[3], c[0], c[2]], x, y, w, h, turned(t), scratch);
}

/// Vertices from their corners, colours and u, v (as `quad`'s, z from `zsrc`'s +0x10), to the renderer's `slot`
/// (the count, the vertices, 0)
fn shape(m: &mut Mem, env: &mut dyn ParticleDrawEnv, slot: u32, zsrc: u32, corners: &[(F, F, u32, (F, F))], scratch: u32) {
    let z = rf(m, zsrc + 0x10);
    for (k, &(x, y, colour, (u, v))) in corners.iter().enumerate() {
        let at = scratch + 0x30 * k as u32;
        wf(m, at, u);
        wf(m, at + 4, v);
        wf(m, at + 8, F(1.0));
        for (j, c) in [4u32, 8, 0xc, 0].into_iter().enumerate() {
            let q = cvt_w((rf(m, colour + c) * F(128.0)).0) as u32;
            m.w32(at + 0x10 + 4 * j as u32, q);
        }
        for (j, w) in [x, y, z, F(1.0)].into_iter().enumerate() {
            wf(m, at + 0x20 + 4 * j as u32, w);
        }
    }
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, slot, &[corners.len() as u32, scratch, 0]);
}

/// `Draw_Line` 0x19c1e8(colour; x0, y0, x1, y1): two vertices (u, v (0, 0) and (1, 1)) to the renderer's
/// lines (+0x238)
pub fn line(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, x0: F, y0: F, x1: F, y1: F, scratch: u32) {
    let ends = [(x0, y0, colour, (F(0.0), F(0.0))), (x1, y1, colour, (F(1.0), F(1.0)))];
    shape(m, env, 0x238, colour, &ends, scratch);
}

/// `FUN_0019c310`(zsrc, c0, c1; x0, y0, x1, y1): as `line`, a colour each end, z from the first word
pub fn gradient_line(m: &mut Mem, env: &mut dyn ParticleDrawEnv, zsrc: u32, c0: u32, c1: u32, x0: F, y0: F, x1: F, y1: F, scratch: u32) {
    let ends = [(x0, y0, c0, (F(0.0), F(0.0))), (x1, y1, c1, (F(1.0), F(1.0)))];
    shape(m, env, 0x238, zsrc, &ends, scratch);
}

/// `FUN_0019c458`(colour; x, y, w, h): the rectangle's outline, five vertices round it from (x, y) and back
/// (u, v round 0 to 1), to the renderer's lines
pub fn rect_outline(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, x: F, y: F, w: F, h: F, scratch: u32) {
    let (x1, y1) = (x + w, y + h);
    let (o, i) = (F(0.0), F(1.0));
    let round = [(x, y, colour, (o, o)), (x1, y, colour, (i, o)), (x1, y1, colour, (i, i)), (x, y1, colour, (o, i)), (x, y, colour, (o, o))];
    shape(m, env, 0x238, colour, &round, scratch);
}

/// `FUN_0019c620`(colour; x0, y0, x1, y1, x2, y2): a triangle (u, v (0, 0), (1, 0), (1, 1)) as a strip of three
/// (+0x220)
pub fn triangle(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, p: [(F, F); 3], scratch: u32) {
    let (o, i) = (F(0.0), F(1.0));
    let corners = [(p[0].0, p[0].1, colour, (o, o)), (p[1].0, p[1].1, colour, (i, o)), (p[2].0, p[2].1, colour, (i, i))];
    shape(m, env, 0x220, colour, &corners, scratch);
}

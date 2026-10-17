//! The rider's shadow (board rows G7g1, G7g2): its set-up (`BoarderShadow_Init` 0x1c8c38), its texture
//! (`BoarderShadow_AllocTexture` 0x1c8c70) and its draw (`BoarderShadow_Draw` 0x1c8bc8). On the bytes at their
//! addresses ([`Mem`]); the renderer's virtuals are calls out ([`ParticleDrawEnv`]). Checked in the function
//! runner (`tests/shadow.rs`).
//!
//! **The shadow** (as these use it): +0xc its place (x, y, z), +0x90 and +0xd0 two blocks the renderer's draw
//! takes, +0x170 its texture (−1 none yet), +0x174 / +0x178 the texture's width and height.
//!
//! The draw hands the renderer the place with w = 1 from the game's stack; the port builds it in `scratch`
//! (STANDIN: the frame's place; the same 16 bytes).

use crate::particledraw::{ParticleDrawEnv, APP};
use crate::pf::Mem;

/// `gRace_EntryCount`: one rider gets the larger texture
pub const ENTRY_COUNT: u32 = 0x32db6c;

fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}
fn vcall(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}

/// `BoarderShadow_Init` 0x1c8c38(shadow): no texture yet; 256 × 256 for a race of one rider, else 128 × 128;
/// the shadow handed back
pub fn init(m: &mut Mem, shadow: u32) -> u32 {
    m.w32(shadow + 0x170, u32::MAX);
    let size = if m.r32(ENTRY_COUNT) == 1 { 0x100 } else { 0x80 };
    m.w32(shadow + 0x178, size);
    m.w32(shadow + 0x174, size);
    shadow
}

/// `BoarderShadow_AllocTexture` 0x1c8c70(shadow): when it has none (+0x170 below 0), its texture from the
/// renderer's +0x170 (width, height, 2)
pub fn alloc_texture(m: &mut Mem, env: &mut dyn ParticleDrawEnv, shadow: u32) {
    if (m.r32(shadow + 0x170) as i32) >= 0 {
        return;
    }
    let r = renderer(m);
    let (w, h) = (m.r32(shadow + 0x174), m.r32(shadow + 0x178));
    let tex = vcall(m, env, r, 0x170, &[w, h, 2]);
    m.w32(shadow + 0x170, tex);
}

/// `BoarderShadow_Draw` 0x1c8bc8(shadow): the renderer's +0x2d8 with the place (w = 1), +0x90, +0xd0, the
/// texture and 1.0 (f12; last in the words)
pub fn draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, shadow: u32, scratch: u32) {
    let r = renderer(m);
    for k in 0..3 {
        let v = m.r32(shadow + 0xc + 4 * k);
        m.w32(scratch + 4 * k, v);
    }
    m.w32(scratch + 0xc, 1f32.to_bits());
    let tex = m.r32(shadow + 0x170);
    vcall(m, env, r, 0x2d8, &[scratch, shadow + 0x90, shadow + 0xd0, tex, 1f32.to_bits()]);
}

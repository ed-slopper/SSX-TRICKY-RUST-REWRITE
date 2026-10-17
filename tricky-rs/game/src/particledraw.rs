//! The particle systems' draw, the EE side (board row G7e1): `ParticleSys_Draw` 0x1d8bc8, which sets the
//! renderer's state for a system and hands it to the renderer's +0x258; and the renderer's virtuals that it uses:
//! +0x1e0 0x1e93e8 (the blend) and the two particle packets, +0x250 0x1e2a60 (VU1 program 6) and +0x258 0x1e2e60
//! (program 7). On the bytes at their addresses ([`Mem`]); the frame's matrices are `cull::frame_matrices`; the
//! renderer's other virtuals and the packet's submit are calls out ([`ParticleDrawEnv`]). Checked in the function
//! runner (`tests/particledraw.rs`).
//!
//! **A particle system** (as drawn): +4 its texture's place in the app's table (app +0x30), +8 its kind (the blend
//! by the table at 0x392cf8, the kind held to 0..10), +0x10 the block that goes to VU1 (9 quadwords for program
//! 7, 20 for program 6), +0x150 / +0x160 its box (min, max).
//!
//! **The renderer's texture state** (+0x470): +0 a word cleared with the program, +4 the blend's second half,
//! +0xe the blend, +0x10 the VU1 program. +0x474 is set whenever the state changes.

use crate::cull::{self, CullEnv};
use crate::pf::Mem;

/// The app (its renderer at +0x724, its texture table at +0x30)
pub const APP: u32 = 0x337c58;
/// The blend for each kind 0..10
pub const BLENDS: u32 = 0x392cf8;
pub const SUBMIT: u32 = 0x1e8838;
/// 0.08, the depth bias the draw sets (+0x1c8's float)
pub const BIAS: u32 = 0x3da3_d70a;

/// The calls out (with the frame's two and the submit, as `cull::CullEnv`)
pub trait ParticleDrawEnv: CullEnv {
    /// A renderer virtual (the function found in the vtable) with these words; its answer. +0x1c8's float
    /// (f12) comes last in the words.
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
}

fn vfn(m: &Mem, r: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(r + 0x10);
    (r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
pub(crate) fn vcall(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let (this, f) = vfn(m, r, off);
    env.vcall(m, f, this, args)
}
fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}

/// `ParticleSys_Draw` 0x1d8bc8(ps): nothing when the renderer's box test (+0x280) answers 1 (all out); else
/// +0x1c0(1), the depth bias +0x1c8(0, 0, 0.08), the kind's blend (+0x1e0), the system's texture (+0x1f0 with
/// it, 0), the system drawn (+0x258 with its block), the bias back (+0x1c8(0, 2, 0.08)) and +0x1c0(0).
pub fn particle_sys_draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, ps: u32) {
    let r = m.r32(m.r32(APP) + 0x724);
    if vcall(m, env, r, 0x280, &[ps + 0x150, ps + 0x160]) == 1 {
        return;
    }
    let kind = m.r32(ps + 8) as i32;
    let kind = if kind < 0 { 0 } else { kind.min(10) };
    let blend = m.r32(BLENDS + 4 * kind as u32);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1e0, &[blend]);
    let table = m.r32(m.r32(APP) + 0x30);
    let tex = m.r32(table.wrapping_add(m.r32(ps + 4) << 2));
    vcall(m, env, r, 0x1f0, &[tex, 0]);
    vcall(m, env, r, 0x258, &[ps + 0x10]);
    vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
    vcall(m, env, r, 0x1c0, &[0]);
}

/// The renderer's +0x1e0, `0x1e93e8`(r, blend, half): when either differs from the texture state's (+0xe, the
/// half at +4), both set and the state marked changed (+0x474)
pub fn set_blend(m: &mut Mem, r: u32, blend: u32, half: u32) {
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0xe) as u32 != blend || m.r16(ts + 4) as u32 != half {
        m.w16(ts + 4, half as u16);
        m.w32(r + 0x474, 1);
        m.w8(ts + 0xe, blend as u8);
    }
}

/// The two particle programs: the VU1 program, the packet's quadwords, the block's unpack code (V4-32 at 0,
/// flagged), and the block's quadwords less one
const PROG6: (u8, u32, u32, u32) = (6, 0x17, 0xd829, 0x13);
const PROG7: (u8, u32, u32, u32) = (7, 0xc, 0xd813, 8);

/// The renderer's +0x250 0x1e2a60(r, block): the block drawn by VU1 program 6 ([`particle_packet`])
pub fn draw_program6(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, block: u32) {
    particle_packet(m, env, r, block, PROG6);
}
/// The renderer's +0x258 0x1e2e60(r, block): the block drawn by VU1 program 7 ([`particle_packet`])
pub fn draw_program7(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, block: u32) {
    particle_packet(m, env, r, block, PROG7);
}

/// The body of both: the texture state's program set (clearing its word 0; marked changed if either differed),
/// the renderer's +0x1e8(2, 0); at the packet place (+0x680) a `cnt` tag with the block's unpack, the block
/// (doublewords), MSCAL 0 and FLUSHA (the place left 0x10 past the packet's end); the frame's matrices if not
/// yet made (+0x17d0, read before the packet is written); the packet submitted (0x1e8838: r, start, end, 0),
/// +0x1e8(0, 0) and the program cleared as it was set.
fn particle_packet(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, block: u32, (prog, qwc, unpack, n): (u8, u32, u32, u32)) {
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != prog || m.r32(ts) != 0 {
        m.w8(ts + 0x10, prog);
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
    }
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let start = m.r32(r + 0x680);
    let made = m.r32(r + 0x17d0);
    w64(m, start, 0x1000_0000 | qwc as u64);
    w64(m, start + 8, 0);
    w64(m, start + 0x10, 0);
    w64(m, start + 0x18, (unpack as u64) << 47);
    let mut p = start + 0x20;
    for k in 0..=n {
        for d in [0, 8] {
            let v = m.r32(block + 0x10 * k + d) as u64 | (m.r32(block + 0x10 * k + d + 4) as u64) << 32;
            w64(m, p + d, v);
        }
        p += 0x10;
    }
    w64(m, p, 0);
    w64(m, p + 8, 0x1400_0000u64 << 32);
    w64(m, p + 0x10, 0x9800u64 << 45);
    w64(m, p + 0x18, 0);
    m.w32(r + 0x680, p + 0x30);
    if made == 0 {
        cull::frame_matrices(m, env, r);
    }
    env.call(m, SUBMIT, &[r, start, p + 0x20, 0]);
    vcall(m, env, r, 0x1e8, &[0, 0]);
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != 0 || m.r32(ts) != 0 {
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
        m.w8(ts + 0x10, 0);
    }
}

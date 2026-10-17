//! The renderer's draws behind its state virtuals (board row G7l): what the effect, streak, marker, icon and
//! quad draws hand their vertices to. On the bytes at their addresses ([`Mem`]); the matrix update and the
//! submit are calls out (`riderdraw::RiderDrawEnv`). Checked in the function runner (`tests/renderdraw.rs`).
//!
//! **A vertex** (0x30 bytes, as the draws build them): +0 u, v, q (and a word the draws leave as it was), +0x10
//! the colour as four integers, +0x20 the place.

use crate::pf::Mem;
use crate::riderdraw::RiderDrawEnv;

pub const MATRIX_UPDATE: u32 = 0x1e85b0;
pub const SUBMIT: u32 = 0x1e8838;

fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}

/// The texture state's VU1 program set (its word 0 cleared; marked changed if either differed)
fn set_program(m: &mut Mem, r: u32, prog: u8) {
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != prog || m.r32(ts) != 0 {
        m.w8(ts + 0x10, prog);
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
    }
}

/// The program cleared again, as [`set_program`] set it
fn clear_program(m: &mut Mem, r: u32) {
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != 0 || m.r32(ts) != 0 {
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
        m.w8(ts + 0x10, 0);
    }
}

/// The renderer's +0x220 0x1de7c8(r, n, vertices): `n` vertices as a strip, on VU1 program 5 (the texture
/// state's program set around it, the matrices made). In chunks: the first up to 73 vertices, each later one
/// up to 71 more after the two before them again (so the strip goes on). Each chunk at the packet place
/// (+0x680) is a `cnt` (3 quadwords a vertex + 4) with an UNPACK (V4-32 to 0, flagged) of a GIF tag (NLOOP the
/// vertices, EOP, PRE with the primitive 0x5c textured or 0x4c not, by the first texture +8 being 0 or more;
/// three registers, 0x412 or 0x41f), the vertex count × 3, the vertices (all 12 words), and MSCAL 0x5e0. The
/// whole is submitted (start, end, 0).
pub fn strips(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, n: u32, vertices: u32) {
    set_program(m, r, 5);
    env.call(m, MATRIX_UPDATE, &[r]);
    let ts = m.r32(r + 0x470);
    let (prim, regs) = if (m.r16(ts + 8) as i16) < 0 { (0x4cu64, 0x41fu64) } else { (0x5c, 0x412) };
    let start = m.r32(r + 0x680);
    let mut left = n as i32;
    let mut first = true;
    let mut index = 0i32;
    let mut p = start;
    while left != 0 {
        let count = if first {
            let c = if left < 0x4a { left } else { 0x49 };
            left -= c;
            c
        } else {
            let c = if left < 0x48 { left } else { 0x47 };
            index -= 2;
            left -= c;
            c + 2
        } as u32;
        w64(m, p, 0x1000_0000 | (count * 3 + 4) as u64);
        w64(m, p + 8, 0);
        w64(m, p + 0x10, 0);
        w64(m, p + 0x18, (((count * 3 + 2) << 16 | 0x6c00_8000) as u64) << 32);
        w64(m, p + 0x20, count as u64 | 0x8000 | 0x3u64 << 60 | prim << 47 | 1u64 << 46);
        w64(m, p + 0x28, regs);
        w64(m, p + 0x30, (count * 3) as u64);
        w64(m, p + 0x38, 0);
        let data = p + 0x40;
        for k in 0..count {
            let from = vertices.wrapping_add((index as u32).wrapping_mul(0x30));
            for j in 0..12 {
                let w = m.r32(from + 4 * j);
                m.w32(data + 0x30 * k + 4 * j, w);
            }
            index += 1;
        }
        let end = data + 0x30 * count;
        w64(m, end, 0);
        w64(m, end + 8, ((0x5e0u64 >> 3) | 0x1400_0000) << 32);
        p = end + 0x10;
        first = false;
    }
    m.w32(r + 0x680, p + 0x10);
    env.call(m, SUBMIT, &[r, start, p, 0]);
    clear_program(m, r);
}

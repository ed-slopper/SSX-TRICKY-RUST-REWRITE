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

fn rf(m: &Mem, a: u32) -> crate::ps2f::F {
    crate::ps2f::F(f32::from_bits(m.r32(a)))
}
/// A word as the EE holds it after `lw` or `mfc1`: sign-extended to 64 bits
fn sx(v: u32) -> u64 {
    v as i32 as i64 as u64
}
/// `cvt.w.s` then `mfc1`: the integer, sign-extended
fn int(x: crate::ps2f::F) -> u64 {
    sx(crate::sndlib::cvt_w(x.0) as u32)
}

/// The outcode of a corner against the screen's box (+0x17b0.. +0x17c8): x below the left 2, beyond the right 1,
/// y 8 / 4; with w below 0 all of those flipped (^ 0x2f) and z not tested, else z beyond the near 0x20, before
/// the far 0x10
fn outcode(m: &Mem, r: u32, c: [crate::ps2f::F; 4]) -> u32 {
    let mut o = 0;
    if c[0] < rf(m, r + 0x17b0) {
        o = 2;
    }
    if rf(m, r + 0x17c0) < c[0] {
        o |= 1;
    }
    if c[1] < rf(m, r + 0x17b4) {
        o |= 8;
    }
    if rf(m, r + 0x17c4) < c[1] {
        o |= 4;
    }
    if c[3] < crate::ps2f::F(0.0) {
        return o ^ 0x2f;
    }
    if rf(m, r + 0x17b8) < c[2] {
        o |= 0x20;
    }
    if c[2] < rf(m, r + 0x17c8) {
        o |= 0x10;
    }
    o
}

/// The renderer's +0x240 0x1dd5c8(r, place, size, st0, st1, colour): one sprite. The matrices are made; the place
/// goes through the screen matrix (+0x1770) and over w (x, y; z kept, w 1 / w), the half size by the projection's
/// scales (+0x1730, +0x1744) over w. The two corners (less and plus the half size) are outcoded (`outcode`), and
/// nothing is drawn when they share a side. Else a `cnt` of 5 at the packet place: DIRECT 4 of a
/// REGLIST tag (NLOOP 1, EOP; PRIM, ST, RGBAQ, XYZ2, ST, XYZ2) with the primitive 0x56, the first ST, the colour
/// (×128 as integers: R the second float, G the third, B the fourth, A the first; Q 1), the first corner (x, y,
/// z × w as integers), the second ST and corner; submitted (start, start + 0x60, 0). The fields are or-ed from
/// sign-extended words, as the EE has them, so a negative one fills the bits above it.
pub fn sprite(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, place: u32, size: u32, st0: u32, st1: u32, colour: u32) {
    use crate::ps2f::F;
    env.call(m, MATRIX_UPDATE, &[r]);
    let rows = [0, 1, 2, 3].map(|i| [0, 1, 2, 3].map(|j| rf(m, r + 0x1770 + 0x10 * i + 4 * j)));
    let pl = [0, 1, 2, 3].map(|j| rf(m, place + 4 * j));
    let p = [0, 1, 2, 3].map(|c| ((rows[0][c] * pl[0] + rows[1][c] * pl[1]) + rows[2][c] * pl[2]) + rows[3][c] * pl[3]);
    let inv = crate::animseq::div(F(1.0), p[3]);
    let hx = rf(m, r + 0x1730) * rf(m, size) * inv;
    let hy = rf(m, r + 0x1744) * rf(m, size + 4) * inv;
    let a = [p[0] * inv, p[1] * inv, p[2], inv];
    let h = [hx, hy, F(0.0), F(0.0)];
    let lo = [0, 1, 2, 3].map(|k| a[k] - h[k]);
    let hi = [0, 1, 2, 3].map(|k| a[k] + h[k]);
    if outcode(m, r, lo) & outcode(m, r, hi) != 0 {
        return;
    }
    let t = m.r32(r + 0x680);
    let st = |m: &Mem, at: u32| sx(m.r32(at)) | (m.r32(at + 4) as u64) << 32;
    let c = |k: u32| int(rf(m, colour + 4 * k) * F(128.0));
    let rgbaq = (1f32.to_bits() as u64) << 32 | c(1) | c(3) << 16 | c(2) << 8 | c(0) << 24;
    let xyz = |q: [F; 4]| int(q[0]) | int(q[1]) << 16 | int(q[2] * q[3]) << 32;
    let fields = [
        0x1000_0005u64,
        0,
        0,
        0x5000_0004u64 << 32,
        0x6400_0000u64 << 32 | 0x8001,
        0x52_5120,
        0x56,
        st(m, st0),
        rgbaq,
        xyz(lo),
        st(m, st1),
        xyz(hi),
    ];
    for (k, v) in fields.into_iter().enumerate() {
        w64(m, t + 8 * k as u32, v);
    }
    m.w32(r + 0x680, t + 0x70);
    env.call(m, SUBMIT, &[r, t, t + 0x60, 0]);
}

//! The renderer's draws behind its state virtuals (board row G7l): what the effect, streak, marker, icon and
//! quad draws hand their vertices to. On the bytes at their addresses ([`Mem`]); the matrix update and the
//! submit are calls out (`riderdraw::RiderDrawEnv`). Checked in the function runner (`tests/renderdraw.rs`).
//!
//! **A vertex** (0x30 bytes, as the draws build them): +0 u, v, q (and a word the draws leave as it was), +0x10
//! the colour as four integers, +0x20 the place.

use crate::pf::Mem;
use crate::riderdraw::{RiderDrawEnv, APP};

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

/// The renderer's +0x2f0 0x1dfe30(r, _, _, matrix, a, b): VU1 program 9's data (the shadow on the rider, from
/// `RiderSkin_SetBoardMatrix`): at the packet place a `ret` tag of 7 quadwords (FLUSHE, an UNPACK of 6 to VU1 0)
/// with the matrix and the two vectors (as doublewords), the place moved on 0x80; the texture state's program
/// set to 9 with that packet as its word 0 (marked changed if either differed). Nothing is submitted.
pub fn program9_data(m: &mut Mem, r: u32, matrix: u32, a: u32, b: u32) {
    let p = m.r32(r + 0x680);
    w64(m, p, 0x6000_0007);
    w64(m, p + 8, 0);
    w64(m, p + 0x10, 0x8000u64 << 45);
    w64(m, p + 0x18, 0xd80cu64 << 47);
    let mut at = p + 0x20;
    for (from, n) in [(matrix, 8u32), (a, 2), (b, 2)] {
        for k in 0..n {
            let v = m.r32(from + 8 * k) as u64 | (m.r32(from + 8 * k + 4) as u64) << 32;
            w64(m, at, v);
            at += 8;
        }
    }
    m.w32(r + 0x680, p + 0x80);
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != 9 || m.r32(ts) != p {
        m.w32(ts, p);
        m.w32(r + 0x474, 1);
        m.w8(ts + 0x10, 9);
    }
}

/// The renderer's +0x310 0x1e7ac0(r, matrices, table, bones, n, out): the skin's blend. For each of the `n`
/// bones (12 bytes each: +0 its influences, +4 them, 4 bytes each: a weight half, then a byte of bone and one of
/// part), its matrix at `out` (r + 0x3f8 when `out` is 0) is the sum over its influences of the influence's matrix
/// (`matrices` at (the part's base in `table` + the bone) × 0x40) times its weight / 100, from the first on.
pub fn skin_blend(m: &mut Mem, r: u32, matrices: u32, table: u32, bones: u32, n: u32, out: u32) {
    use crate::ps2f::F;
    let out = if out == 0 { m.r32(r + 0x3f8) } else { out };
    let src = |m: &Mem, e: u32| {
        let k = m.r32(table.wrapping_add((m.r8(e + 3) as u32) << 2)).wrapping_add(m.r8(e + 2) as u32);
        matrices.wrapping_add(k << 6)
    };
    let weight = |m: &Mem, e: u32| crate::animseq::div(crate::animseq::cvt_s_w(m.r16(e) as i16 as i32), F(100.0));
    let rows = |m: &Mem, at: u32| [0, 1, 2, 3].map(|i| [0, 1, 2, 3].map(|j| rf(m, at + 0x10 * i + 4 * j)));
    let put = |m: &mut Mem, at: u32, x: [[F; 4]; 4]| {
        for a in 0..4u32 {
            for b in 0..4u32 {
                m.w32(at + 0x10 * a + 4 * b, x[a as usize][b as usize].0.to_bits());
            }
        }
    };
    let mut i = 0i32;
    while i < n as i32 {
        let d = bones.wrapping_add((i as u32).wrapping_mul(12));
        let mut e = m.r32(d + 4);
        let to = out.wrapping_add((i as u32) << 6);
        // the first influence copied to its place and scaled there
        let first = rows(m, src(m, e));
        put(m, to, first);
        let w = weight(m, e);
        let scaled = rows(m, to).map(|r| r.map(|x| x * w));
        put(m, to, scaled);
        let mut k = 1i32;
        while k < m.r32(d) as i32 {
            e += 4;
            let w = weight(m, e);
            let more = rows(m, src(m, e)).map(|r| r.map(|x| x * w));
            let sum = rows(m, to);
            put(m, to, [0, 1, 2, 3].map(|a| [0, 1, 2, 3].map(|b| sum[a][b] + more[a][b])));
            k += 1;
        }
        i += 1;
    }
}

fn vcall(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}

/// The renderer's +0x2f8 0x1dff88(r, slot1, _, skin, colour): the board drawn over the rider in the shadow's
/// colour (from `RiderSkin_SetBoardMatrix`). The rider's bones are skinned (`skin::skin_parts`, its parts' table in
/// `scratch`: STANDIN, the game's stack, 0x100 bytes), the board's model (+0x3f20) blended by the renderer's
/// +0x310 (its LOD 0). Then each of the board's parts (0x48 each; one in slot 1 only when `slot1` is set) and
/// each of its mesh's batches (+0x40 of them, 0x14 bytes each from the part's +0x34): both textures off
/// (+0x1f0(−1, 0), (−1, 1)), and when the batch's flag (bit 0) is set, each of its sub-batches (+0xc; their
/// matrix lists 8 bytes each in the part's +0x30, counted on over the part): the renderer's +0x328 with the
/// list's blended matrices (r + 0x3f8 + the bone's 0x40), then a `cnt` of 3 (DIRECT 2: the colour as RGBAQ,
/// ×128 as integers with Q 1) and a `call` of the sub-batch's packet, submitted (from before +0x328, follows: 1).
pub fn board_shadow(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, slot1: u32, skin: u32, colour: u32, scratch: u32) {
    use crate::ps2f::F;
    crate::skin::skin_parts(m, skin, scratch);
    let board = m.r32(skin + 0x3f20);
    let word = m.r32(board + 0x50);
    vcall(m, env, r, 0x310, &[crate::skin::SKINNED, scratch, board + 0x54, word, 0]);
    let parts = m.r32(board) as i32;
    let mut i = 0i32;
    while i < parts {
        let part = m.r32(board + 4).wrapping_add((i * 0x48) as u32);
        let mesh = m.r32(part + 0xc);
        i += 1;
        if m.r8(mesh + 0x4a) as i8 == 1 && slot1 == 0 {
            continue;
        }
        let mut done = 0u32;
        let mut batch = m.r32(part + 0x34);
        let mut j = 0i32;
        while j < m.r16(m.r32(part + 0xc) + 0x40) as i16 as i32 {
            vcall(m, env, r, 0x1f0, &[u32::MAX, 0]);
            vcall(m, env, r, 0x1f0, &[u32::MAX, 1]);
            if m.r32(batch) & 1 != 0 {
                let mut k = 0i32;
                while k < m.r32(batch + 0xc) as i32 {
                    let e = m.r32(part + 0x30).wrapping_add(done.wrapping_add(k as u32) << 3);
                    let (count, list) = (m.r32(e), m.r32(e + 4));
                    let mut t = 0i32;
                    while t < count as i32 {
                        let idx = m.r32(list + 4 * t as u32);
                        let bone = m.r32(m.r32(part + 0x2c).wrapping_add(idx.wrapping_mul(12)) + 8);
                        let at = m.r32(r + 0x3f8).wrapping_add(bone << 6);
                        m.w32(scratch + 0x80 + 4 * t as u32, at);
                        t += 1;
                    }
                    let start = m.r32(r + 0x680);
                    vcall(m, env, r, 0x328, &[count, scratch + 0x80]);
                    let c = |m: &Mem, k: u32| int(rf(m, colour + 4 * k) * F(128.0));
                    let rgbaq = (1f32.to_bits() as u64) << 32 | c(m, 1) | c(m, 3) << 16 | c(m, 2) << 8 | c(m, 0) << 24;
                    let p = m.r32(r + 0x680);
                    let call = m.r32(m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3))) as u64;
                    let fields = [0x1000_0003u64, 0, 0, 0x5000_0002u64 << 32, 0x1000_0000u64 << 32 | 0x8001, 0xe, rgbaq, 1, call << 32 | 0x5000_0000, 0];
                    for (n, v) in fields.into_iter().enumerate() {
                        w64(m, p + 8 * n as u32, v);
                    }
                    m.w32(r + 0x680, p + 0x60);
                    env.call(m, SUBMIT, &[r, start, p + 0x50, 1]);
                    k += 1;
                }
                done = done.wrapping_add(m.r32(batch + 0xc));
            } else {
                done = done.wrapping_add(m.r32(batch + 0xc));
            }
            batch += 0x14;
            j += 1;
        }
    }
}

/// The packet place's next doublewords written (the place left after them)
fn put(m: &mut Mem, r: u32, words: &[u64]) -> u32 {
    let p = m.r32(r + 0x680);
    for (k, v) in words.iter().enumerate() {
        w64(m, p + 8 * k as u32, *v);
    }
    m.w32(r + 0x680, p + 8 * words.len() as u32);
    p + 8 * words.len() as u32
}

/// A sub-batch's matrix list (8 bytes in the part's +0x30: a count and the list) handed to the renderer's +0x328
/// as the blended matrices (r + 0x3f8 + the bone's 0x40), in `scratch` (STANDIN: the game's stack)
fn matrices(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, part: u32, at: u32, scratch: u32) {
    let e = m.r32(part + 0x30).wrapping_add(at << 3);
    let (count, list) = (m.r32(e), m.r32(e + 4));
    let mut t = 0i32;
    while t < count as i32 {
        let idx = m.r32(list + 4 * t as u32);
        let bone = m.r32(m.r32(part + 0x2c).wrapping_add(idx.wrapping_mul(12)) + 8);
        let w = m.r32(r + 0x3f8).wrapping_add(bone << 6);
        m.w32(scratch + 4 * t as u32, w);
        t += 1;
    }
    vcall(m, env, r, 0x328, &[count, scratch]);
}

/// The DMA `call` of the packet whose address is the first word at `at`, with `qwc` quadwords
fn call_of(m: &Mem, at: u32, qwc: u64) -> u64 {
    (m.r32(at) as u64) << 32 | 0x5000_0000 | qwc
}

/// The renderer's +0x2e0 0x1df478(r, lod): the rider's skinned model drawn (from `RiderSkin_SkinMatrices`). The
/// renderer's +0x330 with the first part's +0x38, the matrices made; then each part (0x48 bytes) and each of its
/// mesh's batches (+0x40, 0x14 bytes each from the part's +0x34):
/// - the batch's texture (+4) as the first (+0x1f0); the environment map as the second: without the outfit flag
///   (+0x1d90), +0x644 or (the batch's +8 below 0) +0x648, the texture state's +0xd the +8's magnitude's low
///   byte, blend mode 5 (+0x200), +0x1f8(1, 1); with it, the app's texture +0x30 → +0x90, +0x1f8(0, 1), mode 4,
///   +0xd = 0x21; the test +0x1c8(1, 2, 0.08); +0x1e8(1 or, without the flag 0x8000, 0, 0);
/// - flag 1, the base pass: each sub-batch (+0xc; the part's matrix lists counted on over its batches) its
///   matrices to +0x328 and a `call` of its packet (the batch's +0x10 list), all submitted;
/// - flag 0x100, the layer pass (the part's material +0, its layer weights at +0x210): each sub-batch's matrices,
///   then either (the material's +0x288 is 0) a `call` of its packet and, for each further layer record (12
///   bytes), a `call` of 1 with MSCAL 0x4b0, ended by a `cnt` of 1 with MSCAL; or the layer records in pairs (an
///   odd count's last paired with itself): each pair's two `call`s with a row of the first weight and the part's
///   +0x38 × 2560, then for each further layer of the mesh (+0x48) whose weight is above 0.01 the pair's packets
///   that far on (+4 + the layer × +8) with that weight, ended by a `cnt` of 1 with FLUSH and MSCAL; all
///   submitted.
pub fn skinned_draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, lod: u32, scratch: u32) {
    use crate::ps2f::F;
    let mscal = (0x4b0u64 >> 3) | 0x1400_0000;
    let parts = m.r32(lod) as i32;
    let first = m.r32(m.r32(lod + 4) + 0x38);
    vcall(m, env, r, 0x330, &[first]);
    env.call(m, MATRIX_UPDATE, &[r]);
    let mut i = 0i32;
    while i < parts {
        let part = m.r32(lod + 4).wrapping_add((i * 0x48) as u32);
        i += 1;
        let height = rf(m, part + 0x38) * F(2560.0);
        if m.r16(m.r32(part + 0xc) + 0x40) as i16 <= 0 {
            continue;
        }
        let mut done = 0u32;
        let mut batch = m.r32(part + 0x34);
        let mut j = 0i32;
        loop {
            let tex = m.r32(batch + 4);
            vcall(m, env, r, 0x1f0, &[tex, 0]);
            let envmap;
            if m.r32(r + 0x1d90) == 0 {
                let word = m.r32(batch + 8) as i32;
                let v = word.wrapping_abs() as u32 & 0xff;
                let ts = m.r32(r + 0x470);
                if m.r8(ts + 0xd) as u32 != v {
                    m.w8(ts + 0xd, v as u8);
                    m.w32(r + 0x474, 1);
                }
                envmap = if (m.r32(batch + 8) as i32) < 0 { m.r32(r + 0x648) } else { m.r32(r + 0x644) };
                vcall(m, env, r, 0x200, &[5, 1]);
                vcall(m, env, r, 0x1f8, &[1, 1]);
            } else {
                envmap = m.r32(m.r32(m.r32(APP) + 0x30) + 0x90);
                vcall(m, env, r, 0x1f8, &[0, 1]);
                vcall(m, env, r, 0x200, &[4, 1]);
                let ts = m.r32(r + 0x470);
                if m.r8(ts + 0xd) != 0x21 {
                    m.w8(ts + 0xd, 0x21);
                    m.w32(r + 0x474, 1);
                }
            }
            vcall(m, env, r, 0x1f0, &[envmap, 1]);
            vcall(m, env, r, 0x1c8, &[1, 2, 0x3da3_d70a]);
            let flag = if m.r32(batch) & 0x8000 != 0 { 1 } else { 0 };
            vcall(m, env, r, 0x1e8, &[flag, 0]);
            if m.r32(batch) & 1 != 0 {
                let start = m.r32(r + 0x680);
                let mut end = start;
                let mut k = 0i32;
                while k < m.r32(batch + 0xc) as i32 {
                    matrices(m, env, r, part, done.wrapping_add(k as u32), scratch);
                    let c = call_of(m, m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3)), 0);
                    end = put(m, r, &[c, 0]);
                    k += 1;
                }
                m.w32(r + 0x680, end + 0x10);
                env.call(m, SUBMIT, &[r, start, end, 0]);
            }
            if m.r32(batch) & 0x100 != 0 {
                let weights = m.r32(part) + 0x210;
                let start = m.r32(r + 0x680);
                let mut k = 0i32;
                while k < m.r32(batch + 0xc) as i32 {
                    matrices(m, env, r, part, done.wrapping_add(k as u32), scratch);
                    let entry = m.r32(batch + 0x10).wrapping_add((k as u32) << 3);
                    if m.r32(m.r32(part) + 0x288) == 0 {
                        let c = call_of(m, m.r32(entry), 0);
                        put(m, r, &[c, 0]);
                        let mut t = 1u32;
                        while (t as i32) < m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3) + 4) as i32 {
                            let rec = m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3)).wrapping_add(12 * t);
                            let c = call_of(m, rec, 1);
                            put(m, r, &[c, 0, 0, mscal << 32]);
                            t += 1;
                        }
                        put(m, r, &[0x1000_0001, 0, 0, mscal << 32]);
                    } else {
                        let n = m.r32(entry + 4) as i32;
                        let pairs = if n & 1 != 0 { n + 1 } else { n };
                        let even = n & 1 == 0;
                        let mut t = 0i32;
                        while t < pairs {
                            let base = m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3));
                            let l0 = base.wrapping_add((t as u32).wrapping_mul(12));
                            let (a0, a8) = (m.r32(l0 + 4), m.r32(l0 + 8));
                            let w0 = rf(m, weights);
                            let c0 = call_of(m, l0, 0);
                            let row = (height.0.to_bits() as u64) << 32 | w0.0.to_bits() as u64;
                            put(m, r, &[c0, 0, (a0 as u64) << 32 | 0x5000_0001, 0, 0x6401_03ffu64 << 32, row]);
                            let second = if !even && t == pairs - 2 { t } else { t + 1 };
                            let base = m.r32(m.r32(batch + 0x10).wrapping_add((k as u32) << 3));
                            let l1 = base.wrapping_add((second as u32).wrapping_mul(12));
                            let (b0, b8) = (m.r32(l1 + 4), m.r32(l1 + 8));
                            let c1 = call_of(m, l1, 0);
                            put(m, r, &[c1, 0, (b0 as u64) << 32 | 0x5000_0000, 0, 0x1000_0001, 0, 0, 0]);
                            let mut layer = 1u32;
                            while (layer as i32) < m.r16(m.r32(part + 0xc) + 0x48) as i16 as i32 {
                                let w = rf(m, weights + 4 * layer);
                                if F(f32::from_bits(0x3c23_d70a)) < w {
                                    let p0 = a0.wrapping_add(layer.wrapping_mul(a8));
                                    let p1 = b0.wrapping_add(layer.wrapping_mul(b8));
                                    let row = (height.0.to_bits() as u64) << 32 | w.0.to_bits() as u64;
                                    put(m, r, &[(p0 as u64) << 32 | 0x5000_0001, 0, 0x6401_03ffu64 << 32, row]);
                                    put(m, r, &[(p1 as u64) << 32 | 0x5000_0000, 0, 0x1000_0001, 0, 0, 0]);
                                }
                                layer += 1;
                            }
                            put(m, r, &[0x1000_0001, 0, mscal, mscal | 0x8800u64 << 45]);
                            t += 2;
                        }
                    }
                    k += 1;
                }
                let end = m.r32(r + 0x680);
                m.w32(r + 0x680, end + 0x10);
                env.call(m, SUBMIT, &[r, start, end, 0]);
            }
            done = done.wrapping_add(m.r32(batch + 0xc));
            batch += 0x14;
            j += 1;
            if !(j < m.r16(m.r32(part + 0xc) + 0x40) as i16 as i32) {
                break;
            }
        }
    }
}

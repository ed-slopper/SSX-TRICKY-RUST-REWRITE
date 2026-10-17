//! The texture manager (board rows G7q1..G7q4): `TexMan_*` 0x1c0cd8..0x1c4088, the renderer's +0x47c. It keeps a
//! record for each texture, finds them room in the GS's memory and uploads them, and picks their detail levels.
//! On the bytes at their addresses ([`Mem`]); the C library, the heaps and the GS are calls out
//! ([`TextBoxEnv`]: words and floats in, a word out). Checked in the function runner (`tests/texman.rs`).
//!
//! **The manager** (+0x47c): +0 / +4 / +8 the detail levels' parameters (`calc_lod_params`); +0xc how many
//! records, +0x10 their table (0xb800 bytes: 512 of them); +0x2c..+0x40 the VRAM blocks (+0x34, 0x18 bytes each)
//! and slots (+0x3c, 0x14 bytes); +0x44..+0x67 the mipmaps' settings; +0x68 / +0x6c the CLUT slots (16 bytes);
//! +0x70 / +0x74 the texture bound in GS context 1 / 2; +0x78 the player count last seen.
//!
//! **A load list** (what `register` and `lock` fill, `init_load_nodes` sets up) has the same first words, then
//! +0x18 the heap flags textures are taken with (bit 30 the pool 0x345e48, else `EAMem_Alloc` with the low 26
//! bits; bit 31 clears them to 0, else 0xdeadc0ed), +0x1c how many records, +0x20 how many 8-bit ones, +0x24 the
//! records, +0x28 how many kept ones.
//!
//! **A record** (0x5c bytes): +0 kept (a word given, 3 for a drawn-into one); +4 the name (8 characters); +0xc
//! its size in 256-byte pages; +0x10 / +0x14 the width and its log2 (−1 if not a power of two up to 512);
//! +0x18 / +0x1c the height and its; +0x20 the GS format (0 32-bit, 2 16-bit, 0x13 8-bit); +0x28 / +0x2c the
//! pixels as taken and on a 128-byte boundary; +0x34 / +0x38 the palette's; +0x3c, +0x40 0; +0x44 3; +0x48 /
//! +0x4c / +0x50 / +0x54 its detail levels (by its name: `wrld` 0, `plr` 1, others 2; see `register`).

use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const STRNCMP: u32 = 0x2fd478;
pub const STRNCPY: u32 = 0x2fd630;
pub const POOL_ALLOC: u32 = 0x23d3b0;
pub const ALLOC: u32 = 0x2ccf70;
pub const MEM_SET: u32 = 0x2cc808;
pub const MEM_COPY: u32 = 0x2ce114;
pub const PRINTF: u32 = 0x23d5d0;
pub const LOGF: u32 = 0x2f2a48;
pub const FREE: u32 = 0x14a040;
/// The pool bit 30 of the heap flags takes from
pub const POOL: u32 = 0x345e48;
/// The names: the table's heap name (`mTexNodes`), the set-up's message, the two prefixes (`wrld`, `plr`), a
/// drawn-into one's (`LOCK`)
pub const TABLE_NAME: u32 = 0x390930;
pub const INIT_MESSAGE: u32 = 0x390940;
pub const PREFIX_SHARED: u32 = 0x390950;
pub const PREFIX_FRONT: u32 = 0x390958;
pub const LOCKED_NAME: u32 = 0x390960;
/// A load list's table's heap name (`mLoadTexNodes`)
pub const LOAD_NODES_NAME: u32 = 0x3909c0;
const APP: u32 = 0x337c58;
const RECORD: u32 = 0x5c;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

/// `TexMan_Init` 0x1c0cd8(tm): the table (0xb800 bytes, 0xdeadc0ed) taken; no records, no upload; the set-up's
/// message; then `calc_lod_params` on the manager itself
pub fn init(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32) {
    m.w32(tm + 0xc, 0);
    m.w32(tm + 0x14, 0);
    let table = env.call(m, ALLOC, &[TABLE_NAME, 0xb800, 0], &[]);
    if table != 0 {
        env.call(m, MEM_SET, &[table, 0xdead_c0ed, 0xb800], &[]);
    }
    m.w32(tm + 0x10, table);
    m.w32(tm + 0x70, 0xffff_ffff);
    m.w32(tm + 0x74, 0xffff_ffff);
    for o in [0x18, 0x1c, 0x20, 0x24, 0x2c, 0x30, 0x34, 0x68, 0x6c] {
        m.w32(tm + o, 0);
    }
    env.call(m, PRINTF, &[INIT_MESSAGE], &[]);
    for o in [0x4c, 0x44, 0x48] {
        m.w32(tm + o, 0xffff_ffff);
    }
    for o in (0x50..0x68).step_by(4) {
        m.w32(tm + o, 0);
    }
    m.w32(tm + 0x78, 1);
    calc_lod_params(m, env, tm);
}

/// `TexMan_CalcLodParams` 0x1c0da8(p): from the course's settings (the game +0x730, its +0x2c, while its +0x68
/// is set): +0 0, +4 the shift (+0x70, or +0x78 with other than one player), +8 −16 × log2(the distance (+0x6c /
/// +0x74)) × 2^shift as an integer. With no game, course or settings: 1, 0, 0.
pub fn calc_lod_params(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32) {
    let game = m.r32(m.r32(APP) + 0x730);
    let set = if game != 0 { m.r32(game + 0x2c) } else { 0 };
    if game == 0 || set == 0 || m.r32(set + 0x68) == 0 {
        m.w32(p + 8, 0);
        m.w32(p, 1);
        m.w32(p + 4, 0);
        return;
    }
    m.w32(p, 0);
    let (dist, shift) = if m.r32(game + 0x298) == 1 { (rf(m, set + 0x6c), m.r32(set + 0x70)) } else { (rf(m, set + 0x74), m.r32(set + 0x78)) };
    m.w32(p + 4, shift);
    let ln = F(f32::from_bits(env.call(m, LOGF, &[], &[dist])));
    let log2 = crate::animseq::div(ln, F(f32::from_bits(0x3f31_7218)));
    let scale = crate::animseq::cvt_s_w(1i32.wrapping_shl(m.r32(p + 4)));
    let v = -(log2 * scale) * F(16.0);
    m.w32(p + 8, crate::sndlib::cvt_w(v.0) as u32);
}

/// `FUN_001c0e78`(tm): the manager torn down: an upload under way dropped (+0x2c..+0x3c), the slots (+0x44..+0x4c
/// −1, +0x50..+0x67 0), +0x6c cleared, the table freed
pub fn shut_down(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32) {
    if m.r32(tm + 0x2c) != 0 {
        for o in [0x30, 0x34, 0x3c, 0x2c, 0x38] {
            m.w32(tm + o, 0);
        }
    }
    for o in [0x4c, 0x44, 0x48] {
        m.w32(tm + o, 0xffff_ffff);
    }
    for o in (0x50..0x68).step_by(4) {
        m.w32(tm + o, 0);
    }
    if m.r32(tm + 0x6c) != 0 {
        m.w32(tm + 0x6c, 0);
    }
    let table = m.r32(tm + 0x10);
    if table != 0 {
        env.call(m, FREE, &[table], &[]);
        m.w32(tm + 0x10, 0);
    }
}

/// The log2 of a power of two up to 512, else −1
fn log2(v: u32) -> u32 {
    (0..10).find(|&k| v == 1 << k).map_or(0xffff_ffff, |k| k)
}

/// Pages of 256 bytes, rounded up (as the game's signed sum)
fn pages(size: u32) -> u32 {
    let s = size as i32;
    let t = if s.wrapping_add(0xff) > -1 { s.wrapping_add(0xff) } else { s.wrapping_add(0x1fe) };
    (t >> 8) as u32
}

/// Memory taken with the manager's heap flags (+0x18) and filled (0 with bit 31, else 0xdeadc0ed)
fn take(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, name: u32, size: u32) -> u32 {
    let flags = m.r32(tm + 0x18);
    let p = if flags & 0x4000_0000 != 0 {
        env.call(m, POOL_ALLOC, &[POOL, size, flags, name, 0, 0], &[])
    } else {
        env.call(m, ALLOC, &[name, size, flags & 0x03ff_ffff], &[])
    };
    if p != 0 {
        let fill = if (flags as i32) < 0 { 0 } else { 0xdead_c0ed };
        env.call(m, MEM_SET, &[p, fill, size], &[]);
    }
    p
}

fn record(m: &Mem, tm: u32) -> u32 {
    m.r32(tm + 0x24).wrapping_add(m.r32(tm + 0x1c).wrapping_mul(RECORD))
}

/// The record's name and sizes
fn name_and_size(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, name: u32, n: u32, w: u32, h: u32) {
    let r = record(m, tm);
    env.call(m, STRNCPY, &[r + 4, name, n], &[]);
    let r = record(m, tm);
    m.w32(r + 0x10, w);
    m.w32(r + 0x14, log2(w));
    m.w32(r + 0x18, h);
    m.w32(r + 0x1c, log2(h));
}

/// The next record's number, the count then moved on
fn counted(m: &mut Mem, tm: u32) -> u32 {
    let n = m.r32(tm + 0x1c);
    m.w32(tm + 0x1c, n.wrapping_add(1));
    n
}

/// `TexMan_RegisterTexture` 0x1c0ef8(tm, pixels, w, h, -, format, name, palette; the stack's palette kind and
/// kept): a record for the texture, its pixels copied in (taken with the manager's heap, on a 128-byte boundary).
/// - Its detail levels by its name: the world's (`wrld`, 0) and the players' (`plr`, 1) take the manager's +0 /
///   +4 / +8, others (2) 1, 0, 0.
/// - Kept ones are counted (+0x28).
/// - Formats 0 (32-bit, 4 bytes a pixel) and 2 (16-bit, 2) as they are; 4 (8-bit, 1) only with a palette kind of
///   1: its palette too (0x47f bytes taken, on a 128-byte boundary, the 256 colours copied in blocks of 0x20 with
///   each 0x80's middle two swapped, as the GS stores them) and counted (+0x20). Other formats make no record.
///
/// The record's number (the count moved on either way).
pub fn register(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, pixels: u32, w: u32, h: u32, format: u32, name: u32, palette: u32, kind: u32, kept: u32) -> u32 {
    let (mut lod, mut level, mut a, mut b) = (1u32, 2u32, 0u32, 0u32);
    if env.call(m, STRNCMP, &[name, PREFIX_SHARED, 4], &[]) == 0 {
        (level, lod, a, b) = (0, m.r32(tm), m.r32(tm + 4), m.r32(tm + 8));
    } else if env.call(m, STRNCMP, &[name, PREFIX_FRONT, 3], &[]) == 0 {
        (level, lod, a, b) = (1, m.r32(tm), m.r32(tm + 4), m.r32(tm + 8));
    }
    if kept != 0 {
        let k = m.r32(tm + 0x28).wrapping_add(1);
        m.w32(tm + 0x28, k);
    }
    let (bytes, psm) = match format {
        0 => (4, 0),
        2 => (2, 2),
        4 if kind == 1 => (1, 0x13),
        _ => return counted(m, tm),
    };
    name_and_size(m, env, tm, name, 8, w, h);
    let size = w.wrapping_mul(h).wrapping_mul(bytes);
    let r = record(m, tm);
    m.w32(r + 0x20, psm);
    m.w32(r + 0x4c, lod);
    m.w32(r + 0x48, level);
    m.w32(r + 0x50, a);
    m.w32(r + 0x54, b);
    m.w32(r + 0x40, 0);
    m.w32(r, kept);
    m.w32(r + 0xc, pages(size));
    m.w32(r + 0x44, 3);
    m.w32(r + 0x3c, 0);
    let p = take(m, env, tm, name, size.wrapping_add(0x7f));
    m.w32(r + 0x28, p);
    let r = record(m, tm);
    let at = m.r32(r + 0x28).wrapping_add(0x7f) & 0xffff_ff80;
    m.w32(r + 0x2c, at);
    env.call(m, MEM_COPY, &[at, pixels, size], &[]);
    if psm != 0x13 {
        let r = record(m, tm);
        m.w32(r + 0x38, 0);
        m.w32(r + 0x34, 0);
        return counted(m, tm);
    }
    let r = record(m, tm);
    let c = take(m, env, tm, name, 0x47f);
    m.w32(r + 0x34, c);
    let r = record(m, tm);
    let at = m.r32(r + 0x34).wrapping_add(0x7f) & 0xffff_ff80;
    m.w32(r + 0x38, at);
    let (mut d, mut s) = (at, palette);
    for _ in 0..8 {
        env.call(m, MEM_COPY, &[d, s, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x20, s + 0x40, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x40, s + 0x20, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x60, s + 0x60, 0x20], &[]);
        s += 0x80;
        d += 0x80;
    }
    let k = m.r32(tm + 0x20).wrapping_add(1);
    m.w32(tm + 0x20, k);
    counted(m, tm)
}

/// `TexMan_LockTexture` 0x1c17c0(tm, w, h, format): a record for a texture drawn into (no pixels, named
/// `LOCK`): 32-bit (0) or 16-bit (2), kept (3), detail level 2 with lod 1. Other formats make none. Kept ones
/// counted (+0x28) either way; the record's number.
pub fn lock(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, w: u32, h: u32, format: u32) -> u32 {
    let (bytes, psm) = match format {
        0 => (4, 0),
        2 => (2, 2),
        _ => {
            let n = counted(m, tm);
            let k = m.r32(tm + 0x28).wrapping_add(1);
            m.w32(tm + 0x28, k);
            return n;
        }
    };
    name_and_size(m, env, tm, LOCKED_NAME, 5, w, h);
    let size = w.wrapping_mul(h).wrapping_mul(bytes);
    let r = record(m, tm);
    m.w32(r + 0x20, psm);
    m.w32(r + 0x4c, 1);
    m.w32(r + 0x48, 2);
    for o in [0x50, 0x54, 0xc, 0x40] {
        m.w32(r + o, 0);
    }
    m.w32(r, 3);
    let p = m.r32(r + 0xc).wrapping_add(pages(size));
    m.w32(r + 0x44, 3);
    m.w32(r + 0x3c, 0);
    m.w32(r + 0xc, p);
    for o in [0x38, 0x28, 0x2c, 0x34] {
        m.w32(r + o, 0);
    }
    let n = counted(m, tm);
    let k = m.r32(tm + 0x28).wrapping_add(1);
    m.w32(tm + 0x28, k);
    n
}

pub const TEX_SWIZZLE: u32 = 0x1fbe98;

fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}
/// A word as the EE holds it: sign-extended to 64 bits
fn sx(v: u32) -> i64 {
    v as i32 as i64
}
/// The record `n` of the table (+0x10) the uploads use
fn slot_record(m: &Mem, tm: u32, n: u32) -> u32 {
    m.r32(tm + 0x10).wrapping_add(n.wrapping_mul(RECORD))
}

/// `FUN_001c1ab8`(tm, n, &packet): at the packet place (moved on by 0x60) a `cnt` of 5 and A+D settings that draw
/// into texture `n` at its VRAM slot (+0x3c's 0x14-byte slots, the record's +0x24): FRAME (the slot's address /
/// 32, the width / 64 at 16, the format at 24), SCISSOR (width − 1 at 16, height − 1 at 48) and XYOFFSET (the
/// offset 2048 less half the size, × 16), after a DIRECT-less header (0x9800 << 45, the `cnt`'s VIF word
/// 0x50000004 << 32) and the GIF tag (NLOOP 3, EOP, A+D)
pub fn draw_into_packet(m: &mut Mem, tm: u32, n: u32, place: u32) {
    let p = m.r32(place);
    let r = slot_record(m, tm, n);
    let (w, h, psm) = (m.r32(r + 0x10), m.r32(r + 0x18), m.r32(r + 0x20));
    let slot = m.r32(tm + 0x3c).wrapping_add(m.r32(r + 0x24).wrapping_mul(0x14));
    let frame = (sx(m.r32(slot)) >> 5) | ((sx(w) >> 6) << 16) | (sx(psm) << 24);
    let scissor = (sx(w.wrapping_sub(1)) << 16) | (sx(h.wrapping_sub(1)) << 48);
    let ox = sx(0x800u32.wrapping_sub((w as i32 >> 1) as u32) << 4);
    let oy = sx(0x800u32.wrapping_sub((h as i32 >> 1) as u32) << 4);
    let offset = ox | (oy << 32);
    w64(m, p, 0x1000_0005);
    let q = p + 8;
    let words: [u64; 11] = [
        0,
        0x9800u64 << 45,
        0x5000_0004u64 << 32,
        0x1000_0000_0000_8003,
        0xe,
        frame as u64,
        0x4c,
        scissor as u64,
        0x40,
        offset as u64,
        0x18,
    ];
    for (k, v) in words.into_iter().enumerate() {
        w64(m, q + 8 * k as u32, v);
    }
    m.w32(place, q + 0x58);
}

/// `FUN_001c1bd0`(tm, n, colour, &packet): at the packet place (moved on by 0x70) a `cnt` of 6, the VIF's DIRECT of
/// 5, and a sprite filling texture `n` in the colour (the floats at +4, +8, +0xc, +0 × 128 as R, G, B, A; Q 1):
/// its corners the offset 2048 less and plus half the size (× 16, z 1), then SCISSOR (width − 2 at 16, height − 2
/// at 48, and 1 at 0 and 32)
pub fn fill_packet(m: &mut Mem, tm: u32, n: u32, colour: u32, place: u32) {
    let p = m.r32(place);
    let k = |m: &Mem, o: u32| crate::sndlib::cvt_w((rf(m, colour + o) * F(128.0)).0) as u32;
    let (red, blue, green, alpha) = (k(m, 4), k(m, 0xc), k(m, 8), k(m, 0));
    let rgbaq = sx(red) | (sx(1f32.to_bits()) << 32) | (sx(blue) << 16) | (sx(green) << 8) | (sx(alpha) << 24);
    let rec = slot_record(m, tm, n);
    let (h, w) = (m.r32(rec + 0x18), m.r32(rec + 0x10));
    let (hh, hw) = (h as i32 >> 1, w as i32 >> 1);
    let y0 = sx(0x800u32.wrapping_sub(hh as u32) << 4);
    let y1 = sx((hh as u32).wrapping_add(0x801) << 4);
    let x0 = sx(0x800u32.wrapping_sub(hw as u32) << 4);
    let x1 = sx((hw as u32).wrapping_add(0x801) << 4);
    let z = 1i64 << 32;
    let lo = x0 | (y0 << 16) | z;
    let hi = x1 | (y1 << 16) | z;
    let scissor = (sx(w.wrapping_sub(2)) << 16) | (sx(h.wrapping_sub(2)) << 48) | (1i64 << 32 | 1);
    w64(m, p, 0x1000_0006);
    let q = p + 8;
    let words: [u64; 13] = [
        0,
        0,
        0x5000_0005u64 << 32,
        0x4400_0000_0000_0001,
        0x5510,
        0x10e,
        rgbaq as u64,
        lo as u64,
        hi as u64,
        0x1000_0000_0000_8001,
        0xe,
        scissor as u64,
        0x40,
    ];
    for (k, v) in words.into_iter().enumerate() {
        w64(m, q + 8 * k as u32, v);
    }
    m.w32(place, q + 0x68);
}

/// `TexMan_UploadPixels` 0x1c1d58(tm, pixels, palette, n): texture `n`'s pixels replaced (copied in: 32-bit 4
/// bytes a pixel, 16-bit 2; 8-bit 1, or swizzled into place by `Tex_Swizzle` 0x1fbe98 when the record's +0x3c is
/// set; others not); it no longer the one being uploaded (+0x70 −1 if it was), its VRAM slot (if any, above 0)
/// let go; an 8-bit one's palette copied in too (as `register` does) and its CLUT slot (+0x6c's 16-byte ones)
/// let go
pub fn upload_pixels(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, pixels: u32, palette: u32, n: u32) {
    let r = slot_record(m, tm, n);
    let (w, h, at) = (m.r32(r + 0x10), m.r32(r + 0x18), m.r32(r + 0x2c));
    match m.r32(r + 0x20) {
        0 => {
            env.call(m, MEM_COPY, &[at, pixels, w.wrapping_mul(h) << 2], &[]);
        }
        2 => {
            env.call(m, MEM_COPY, &[at, pixels, w.wrapping_mul(h) << 1], &[]);
        }
        0x13 => {
            if m.r32(r + 0x3c) != 0 {
                env.call(m, TEX_SWIZZLE, &[tm, w, h, pixels, at], &[]);
            } else {
                env.call(m, MEM_COPY, &[at, pixels, w.wrapping_mul(h)], &[]);
            }
        }
        _ => {}
    }
    if m.r32(tm + 0x70) == n {
        m.w32(tm + 0x70, 0xffff_ffff);
    }
    let slots = m.r32(tm + 0x3c);
    if slots != 0 {
        let r = slot_record(m, tm, n);
        let s = m.r32(r + 0x24);
        if s as i32 > 0 {
            m.w32(slots.wrapping_add(s.wrapping_mul(0x14)) + 4, 0xffff_ffff);
            m.w32(r + 0x24, 0xffff_ffff);
        }
    }
    let r = slot_record(m, tm, n);
    if m.r32(r + 0x20) != 0x13 {
        return;
    }
    let (mut d, mut s) = (m.r32(r + 0x38), palette);
    for _ in 0..8 {
        env.call(m, MEM_COPY, &[d, s, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x20, s + 0x40, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x40, s + 0x20, 0x20], &[]);
        env.call(m, MEM_COPY, &[d + 0x60, s + 0x60, 0x20], &[]);
        s += 0x80;
        d += 0x80;
    }
    let cluts = m.r32(tm + 0x6c);
    if cluts != 0 {
        let r = slot_record(m, tm, n);
        let c = m.r32(r + 0x30);
        if (c as i32) >= 0 {
            m.w32(cluts.wrapping_add(c << 4) + 0xc, 0xffff_ffff);
            m.w32(r + 0x30, 0xffff_ffff);
        }
    }
}

/// `TexMan_UpdateLod` 0x1c3348(tm): every record's use (+0x58) × 0.8; when the race's player count (the game
/// +0x298) is not the one last seen (+0x78), it is kept, the detail levels worked out again (`calc_lod_params`)
/// and given to the world's and the players' records (level 0 / 1: the manager's +0 / +4 / +8) and the others'
/// (level 2: 1, the same +0x50, 0); records of no level (below 0) or above 2 are left
pub fn update_lod(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32) {
    let n = m.r32(tm + 0xc) as i32;
    for i in 0..n.max(0) as u32 {
        let r = slot_record(m, tm, i);
        let used = rf(m, r + 0x58) * F(f32::from_bits(0x3f4c_cccd));
        m.w32(r + 0x58, used.0.to_bits());
    }
    let game = m.r32(m.r32(APP) + 0x730);
    let mut changed = false;
    if game != 0 && m.r32(game + 0x2c) != 0 {
        let players = m.r32(game + 0x298);
        if m.r32(tm + 0x78) != players {
            m.w32(tm + 0x78, players);
            changed = true;
        }
    }
    if !changed {
        return;
    }
    calc_lod_params(m, env, tm);
    let mut i = 0i32;
    while i < m.r32(tm + 0xc) as i32 {
        let r = slot_record(m, tm, i as u32);
        let level = m.r32(r + 0x48) as i32;
        if (0..2).contains(&level) {
            m.w32(r + 0x4c, m.r32(tm));
            m.w32(r + 0x50, m.r32(tm + 4));
            m.w32(r + 0x54, m.r32(tm + 8));
        } else if level == 2 {
            m.w32(r + 0x54, 0);
            m.w32(r + 0x4c, 1);
        }
        i += 1;
    }
}

/// `TexMan_InitLoadNodes` 0x1c3480(list, flags): a load list emptied (+0x1c, +0x20, +0x28) and its records'
/// table taken (0xb800 bytes, named `mTexLoadNodes`) with the flags but bit 4 turned over (bit 30 the pool, bit
/// 31 cleared to 0, else 0xdeadc0ed); the flags kept (+0x18), the table at +0x24
pub fn init_load_nodes(m: &mut Mem, env: &mut dyn TextBoxEnv, list: u32, flags: u32) {
    let f = flags ^ 0x10;
    for o in [0x1c, 0x20, 0x28] {
        m.w32(list + o, 0);
    }
    let p = if f & 0x4000_0000 != 0 {
        env.call(m, POOL_ALLOC, &[POOL, 0xb800, f, LOAD_NODES_NAME, 0, 0], &[])
    } else {
        env.call(m, ALLOC, &[LOAD_NODES_NAME, 0xb800, f & 0x03ff_ffff], &[])
    };
    if p != 0 {
        let fill = if (f as i32) < 0 { 0 } else { 0xdead_c0ed };
        env.call(m, MEM_SET, &[p, fill, 0xb800], &[]);
    }
    m.w32(list + 0x18, flags);
    m.w32(list + 0x24, p);
}

/// `TexMan_FreeAll` 0x1c3560(tm): every record's pixels and palette freed; no records; an upload under way
/// dropped; no CLUT slots (+0x68, +0x6c) and none being uploaded (+0x70, +0x74 −1)
pub fn free_all(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32) {
    let mut i = 0i32;
    while i < m.r32(tm + 0xc) as i32 {
        for o in [0x28, 0x34] {
            let r = slot_record(m, tm, i as u32);
            let p = m.r32(r + o);
            if p != 0 {
                env.call(m, FREE, &[p], &[]);
                let r = slot_record(m, tm, i as u32);
                m.w32(r + o, 0);
            }
        }
        i += 1;
    }
    m.w32(tm + 0xc, 0);
    m.w32(tm + 0x14, 0);
    if m.r32(tm + 0x2c) != 0 {
        for o in [0x30, 0x34, 0x3c, 0x2c, 0x38] {
            m.w32(tm + o, 0);
        }
    }
    m.w32(tm + 0x6c, 0);
    m.w32(tm + 0x74, 0xffff_ffff);
    m.w32(tm + 0x68, 0);
    m.w32(tm + 0x70, 0xffff_ffff);
}

/// `TexMan_FindVramSlot` 0x1c3650(tm, pages, avoid): a VRAM slot for a texture of `pages`. The blocks (+0x34, 0x18
/// bytes; from +0x40 to +0x2c) of that size, or bigger ones holding 2 or more slots, are looked through (their
/// slots a list from +0x14: +4 the record, +8 the slot, +0xc the next): an empty slot is the answer at once; else
/// the one whose texture was used least (its +0x58 × the block's size), never `avoid`. That one's texture is let
/// go (the slot's +4 and its record's +0x24 −1; with none found, the slot before the first). The slot.
pub fn find_vram_slot(m: &mut Mem, tm: u32, pages: u32, avoid: u32) -> u32 {
    let end = m.r32(tm + 0x2c) as i32;
    let mut best = 0xffff_ffffu32;
    let mut least = F(0.0);
    let mut b = m.r32(tm + 0x40) as i32;
    while b < end {
        let blk = m.r32(tm + 0x34).wrapping_add((b as u32).wrapping_mul(0x18));
        let size = m.r32(blk + 4);
        let fits = size == pages || ((pages as i32) < size as i32 && m.r32(blk + 8) as i32 >= 2);
        let mut node = if fits { m.r32(blk + 0x14) } else { 0 };
        while node != 0 {
            let rec = m.r32(node + 4);
            if (rec as i32) < 0 {
                return m.r32(node + 8);
            }
            let r = slot_record(m, tm, rec);
            let cost = rf(m, r + 0x58) * crate::animseq::cvt_s_w(m.r32(blk + 4) as i32);
            if (best as i32) < 0 || cost < least {
                let slot = m.r32(node + 8);
                if slot != avoid {
                    best = slot;
                    least = cost;
                }
            }
            node = m.r32(node + 0xc);
        }
        b += 1;
    }
    let s = m.r32(tm + 0x3c).wrapping_add(best.wrapping_mul(0x14));
    let rec = m.r32(s + 4);
    let r = slot_record(m, tm, rec);
    m.w32(r + 0x24, 0xffff_ffff);
    m.w32(s + 4, 0xffff_ffff);
    best
}

/// `TexMan_FindClutSlot` 0x1c3760(tm, avoid): a CLUT slot (+0x6c's 16-byte ones, +0x68 of them): an empty one
/// (its +0xc below 0) at once; else the one whose texture was used least, never `avoid`, its texture let go (the
/// slot's +0xc and its record's +0x30 −1; with none, the slot before the first). The slot.
pub fn find_clut_slot(m: &mut Mem, tm: u32, avoid: u32) -> u32 {
    let mut best = 0xffff_ffffu32;
    let mut least = F(0.0);
    let n = m.r32(tm + 0x68) as i32;
    let mut i = 0i32;
    while i < n {
        let e = m.r32(tm + 0x6c).wrapping_add((i as u32) << 4);
        let rec = m.r32(e + 0xc);
        if (rec as i32) < 0 {
            return i as u32;
        }
        if i as u32 != avoid {
            let used = rf(m, slot_record(m, tm, rec) + 0x58);
            if (best as i32) < 0 || used < least {
                least = used;
                best = i as u32;
            }
        }
        i += 1;
    }
    let e = m.r32(tm + 0x6c).wrapping_add(best << 4);
    let rec = m.r32(e + 0xc);
    let r = slot_record(m, tm, rec);
    m.w32(r + 0x30, 0xffff_ffff);
    m.w32(e + 0xc, 0xffff_ffff);
    best
}

pub const FLUSH_CACHE: u32 = 0x306a60;
pub const DMA_SEND: u32 = 0x2408c8;
/// The scratchpad the upload's packet is built in
pub const SPR: u32 = 0x7000_0000;

fn put(m: &mut Mem, at: u32, words: &[(u32, u64)]) {
    for &(o, v) in words {
        w64(m, at + o, v);
    }
}
/// BITBLTBUF for a texture at `vram` (in the high word) of `w` pixels (the buffer's width, at least 1 × 64) and
/// GS format `psm` (as both formats)
fn bitbltbuf(vram: u32, w: u32, psm: u32) -> u64 {
    let tbw = (w as i32 >> 6).max(1) as u32;
    let v = (sx(vram) << 32) | (sx(tbw) << 48) | (sx(psm) << 24) | (sx(psm) << 56);
    v as u64
}

/// `TexMan_UploadTexture` 0x1c3ba8(tm, n, vram): texture `n` sent to VRAM by DMA: its transfer built in the
/// scratchpad (0x70000000: a `cnt` of 6 with BITBLTBUF, TRXPOS 0, TRXREG (width, height), TRXDIR 0, an IMAGE tag of
/// the pixels' quadwords, a `ref` to them, an `end` of 2 with the closing A+D), the cache flushed (0x306a60(0)) and
/// the 6 quadwords sent (0x2408c8). The bytes a pixel by its format: 4 (0, 1), 2 (2), 1 (0x13, 0x1b), else 0.
pub fn upload_texture(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, n: u32, vram: u32) {
    let r = slot_record(m, tm, n);
    let psm = m.r32(r + 0x20);
    let bpp = match psm {
        0 | 1 => 4,
        2 => 2,
        0x13 | 0x1b => 1,
        _ => 0,
    };
    let (w, h, pixels) = (m.r32(r + 0x10), m.r32(r + 0x18), m.r32(r + 0x2c));
    let qwc = (w.wrapping_mul(bpp).wrapping_mul(h) as i32 >> 4) as u32;
    let image = (sx(qwc) | (1i64 << 59)) as u64;
    let refer = (sx(qwc | 0x3000_0000) | (sx(pixels) << 32)) as u64;
    let trxreg = (sx(w) | (sx(h) << 32)) as u64;
    put(m, SPR, &[
        (0x98, 0xe), (0x18, 0xe), (0, 0x1000_0006), (0x10, 0x1000_0000_0000_0004), (0x20, bitbltbuf(vram, w, psm)),
        (0x28, 0x50), (0x38, 0x51), (0x40, trxreg), (0x48, 0x52), (0x58, 0x53), (0x60, image), (0x70, refer),
        (0x80, 0x7000_0002), (0x90, 0x1000_0000_0000_8001), (0xa8, 0x3f), (8, 0), (0x30, 0), (0x50, 0), (0x68, 0),
        (0x78, 0), (0x88, 0), (0xa0, 0),
    ]);
    env.call(m, FLUSH_CACHE, &[0], &[]);
    env.call(m, DMA_SEND, &[SPR, 6], &[]);
}

/// `FUN_001c3d88`(tm, n, &packet, vram): at the packet place (moved on by 0xe0) the transfer of texture `n` to
/// VRAM: as `upload_texture`'s, in the frame's packet (a `cnt` of 1 with FLUSHA, then a `cnt` of 6 with the
/// settings, the `ref` to the pixels, a `cnt` of 3 to close). An 8-bit texture goes as 32-bit at half the width and
/// height (4 bytes for each 2 × 2); a 16-bit one 2 bytes a pixel, 32-bit 4; others none.
pub fn vram_packet(m: &mut Mem, tm: u32, n: u32, place: u32, vram: u32) {
    let r = slot_record(m, tm, n);
    let (mut psm, mut w, mut h) = (m.r32(r + 0x20), m.r32(r + 0x10), m.r32(r + 0x18));
    let row = if psm == 2 {
        w.wrapping_mul(2)
    } else if (psm as i32) < 3 {
        if (psm as i32) < 0 { 0 } else { w.wrapping_mul(4) }
    } else if psm == 0x13 {
        psm = 0;
        w = (w as i32 >> 1) as u32;
        h = (h as i32 >> 1) as u32;
        w.wrapping_mul(4)
    } else {
        0
    };
    let p = m.r32(place);
    let pixels = m.r32(r + 0x2c);
    let qwc = (row.wrapping_mul(h) as i32 >> 4) as u32;
    let image = (sx(qwc) | (1i64 << 59)) as u64;
    let refer = (sx(qwc | 0x3000_0000) | (sx(pixels) << 32)) as u64;
    let trxreg = (sx(w) | (sx(h) << 32)) as u64;
    let vif = (sx(qwc.wrapping_add(6) | 0x5000_0000) << 32) as u64;
    w64(m, p, 0x1000_0001);
    let q = p + 8;
    put(m, q, &[
        (0xb0, 0x5000_0002u64 << 32), (0xb8, 0x1000_0000_0000_8001), (0xc0, 0xe), (0xd0, 0x3f), (0x30, 0xe), (0x10, vif),
        (0x18, 0x1000_0006), (0x28, 0x1000_0000_0000_0004), (0x38, bitbltbuf(vram, w, psm)), (0x40, 0x50), (0x50, 0x51),
        (0x58, trxreg), (0x60, 0x52), (0x70, 0x53), (0x78, image), (0x88, refer), (0x98, 0x1000_0003), (0, 0), (8, 0),
        (0x20, 0), (0x48, 0), (0x68, 0), (0x80, 0), (0x90, 0), (0xa0, 0), (0xa8, 0), (0xc8, 0),
    ]);
    m.w32(place, q + 0xd8);
}

/// `FUN_001c3f68`(tm, n, &packet, cbp): at the packet place (moved on by 0xc0) the transfer of texture `n`'s
/// palette (16 × 16, 32-bit, 0x40 quadwords from its +0x38) to the CLUT slot at `cbp`
pub fn clut_packet(m: &mut Mem, tm: u32, n: u32, place: u32, cbp: u32) {
    let p = m.r32(place);
    let r = slot_record(m, tm, n);
    w64(m, p, 0x1000_0007);
    let q = p + 8;
    let refer = ((m.r32(r + 0x38) as u64) << 32) | 0x3000_0040;
    put(m, q, &[
        (0x18, 0x1000_0000_0000_0004), (0x28, (sx(cbp) << 32 | 1i64 << 48) as u64), (0x30, 0x50), (0x20, 0xe),
        (0x10, 0x5000_0048u64 << 32), (0x40, 0x51), (0x48, 0x10u64 << 32 | 0x10), (0x50, 0x52), (0x60, 0x53),
        (0x68, 0x0800_0000_0000_0040), (0, 0), (8, 0), (0x38, 0), (0x58, 0), (0x70, 0), (0xa0, 0xe), (0x88, 0x1000_0002),
        (0x98, 0x1000_0000_0000_8001), (0x78, refer), (0xb0, 0x3f), (0x80, 0), (0x90, 0), (0xa8, 0),
    ]);
    m.w32(place, q + 0xb8);
}

/// `FUN_001c3818`(tm, n, context, &packet): texture `n` bound for drawing in GS context 1 (else 2). Its use +1.
/// Already bound there: 0. Else it is the context's (+0x70 / +0x74); the other context's texture's slots are kept
/// clear of. At the packet place a `cnt` of 1 (FLUSH); then:
/// - without a VRAM slot, one found (`find_vram_slot` for its pages) and its pixels sent (`vram_packet`): 2, else 1;
/// - an 8-bit (0x13 / 0x1b) one's CLUT slot the same (`find_clut_slot`, `clut_packet`);
/// - TEX1 (the lod (+0x4c), the mipmaps (+0x40) × 4, linear filtering 0x160, L (+0x50) at 19, K (+0x54) at 32) and
///   TEX0 (the slot, the buffer width at 14, the format at 20, the size's log2s at 26 and 30, colour with alpha, the
///   CLUT's slot at 37, loaded at 61), and with mipmaps MIPTBP1 (+0x50.. by +0x44) as A+D.
pub fn bind(m: &mut Mem, tm: u32, n: u32, context: u32, place: u32) -> u32 {
    let r = slot_record(m, tm, n);
    let used = rf(m, r + 0x58) + F(1.0);
    m.w32(r + 0x58, used.0.to_bits());
    let other;
    if context == 1 {
        if m.r32(tm + 0x70) == n {
            return 0;
        }
        other = m.r32(tm + 0x74);
        m.w32(tm + 0x70, n);
    } else {
        if m.r32(tm + 0x74) == n {
            return 0;
        }
        other = m.r32(tm + 0x70);
        m.w32(tm + 0x74, n);
    }
    let (avoid_vram, avoid_clut) = if (other as i32) >= 0 {
        let o = slot_record(m, tm, other);
        (m.r32(o + 0x24), m.r32(o + 0x30))
    } else {
        (0xffff_ffff, 0xffff_ffff)
    };
    let p = m.r32(place);
    w64(m, p, 0x1000_0001);
    w64(m, p + 0x10, 0x8800u64 << 45);
    m.w32(place, p + 0x20);
    w64(m, p + 8, 0);
    w64(m, p + 0x18, 0);
    let mut answer = 1;
    if (m.r32(r + 0x24) as i32) < 0 {
        answer = 2;
        let s = find_vram_slot(m, tm, m.r32(r + 0xc), avoid_vram);
        m.w32(r + 0x24, s);
        let slots = m.r32(tm + 0x3c);
        m.w32(slots.wrapping_add(s.wrapping_mul(0x14)) + 4, n);
        let vram = m.r32(slots.wrapping_add(m.r32(r + 0x24).wrapping_mul(0x14)));
        vram_packet(m, tm, n, place, vram);
    }
    let psm = m.r32(r + 0x20);
    let mut cbp = 0;
    if psm == 0x13 || psm == 0x1b {
        if (m.r32(r + 0x30) as i32) < 0 {
            let c = find_clut_slot(m, tm, avoid_clut);
            m.w32(r + 0x30, c);
            let cluts = m.r32(tm + 0x6c);
            m.w32(cluts.wrapping_add(c << 4) + 0xc, n);
            let at = m.r32(cluts.wrapping_add(m.r32(r + 0x30) << 4));
            clut_packet(m, tm, n, place, at);
        }
        cbp = m.r32(m.r32(tm + 0x6c).wrapping_add(m.r32(r + 0x30) << 4));
    }
    let mips = m.r32(r + 0x40);
    let p = m.r32(place);
    let (count, nloop) = if mips != 0 { (0x1000_0005, 0x5000_0004u64) } else { (0x1000_0004, 0x5000_0003u64) };
    w64(m, p, count);
    let q = p + 8;
    put(m, q, &[(8, 0x8800u64 << 45), (0x10, nloop << 32), (0x18, 0x1000_0000_0000_8000 | if mips != 0 { 3 } else { 2 }), (0x20, 0xe), (0, 0)]);
    m.w32(place, q + 0x28);
    let (tex0_reg, tex1_reg, mip_reg) = if context == 1 { (6u64, 0x14u64, 0x34u64) } else { (7, 0x15, 0x35) };
    let w = m.r32(r + 0x10);
    let w = if 63 < (w as i32) { w } else { 0x40 };
    let tbw = (w as i32 >> 6) as u32;
    let slot = m.r32(m.r32(tm + 0x3c).wrapping_add(m.r32(r + 0x24).wrapping_mul(0x14)));
    let tex1 = sx(m.r32(r + 0x4c)) | (sx(m.r32(r + 0x54)) << 32) | (sx(m.r32(r + 0x50)) << 19) | 0x160 | (sx(mips) << 2);
    let tex0 = sx(slot) | (1i64 << 61) | (sx(m.r32(r + 0x14)) << 26) | (1i64 << 34) | (sx(tbw) << 14) | (sx(psm) << 20) | (sx(m.r32(r + 0x1c)) << 30) | (sx(cbp) << 37);
    let t = m.r32(place);
    put(m, t, &[(0, tex1 as u64), (8, tex1_reg), (0x10, tex0 as u64), (0x18, tex0_reg)]);
    m.w32(place, t + 0x20);
    if mips != 0 {
        m.w32(place, t + 0x30);
        let at = tm.wrapping_add(m.r32(r + 0x44) << 3);
        let mip = (m.r32(at + 0x54) as u64) << 32 | m.r32(at + 0x50) as u64;
        put(m, t, &[(0x28, mip_reg), (0x20, mip)]);
    }
    answer
}

pub const MEM_MOVE: u32 = 0x2ce228;
pub const EAMEM_FREE: u32 = 0x2ccfc0;
pub const GROUPS_NAME: u32 = 0x390968;
pub const UPLOAD_MESSAGE: u32 = 0x390970;
pub const KEPT_MESSAGE: u32 = 0x390988;
pub const SWIZZLE_NAME: u32 = 0x3909b0;
/// Where the manager's VRAM blocks (then their slots) and CLUT slots are laid out
pub const BLOCK_TABLE: u32 = 0x3ce820;
pub const CLUT_TABLE: u32 = 0x3d0820;

/// A group ("bucket") of the upload's plan (16 bytes): +0 its textures' pages, +4 its kind's flags (0x21 kept, 4
/// drawn into, 2 locked, 8 eight-bit, 0x20 a player's, 0x10 in the high8 area), +8 how many textures, +0xc how many
/// slots
fn grp(groups: u32, k: u32) -> u32 {
    groups.wrapping_add(k << 4)
}
fn kind_flags(kind: u32) -> u32 {
    match kind {
        1 => 0x21,
        3 => 4,
        2 => 2,
        _ => 0,
    }
}
/// The first slot of a block's list with no texture
fn free_node(m: &Mem, blk: u32) -> u32 {
    let mut node = m.r32(blk + 0x14);
    while node != 0 && (m.r32(node + 4) as i32) >= 0 {
        node = m.r32(node + 0xc);
    }
    node
}

/// `TexMan_UploadTextures` 0x1c1f50(tm, vram, pages, high8_vram, high8_pages): the level's textures given VRAM and
/// uploaded; `high8_*` is the area 8-bit textures share as the top bytes of a 32-bit buffer (PSMT8H, 0x1b). (The
/// steps as in the port notes, G7q3.)
pub fn upload_textures(m: &mut Mem, env: &mut dyn TextBoxEnv, tm: u32, vram: u32, pages: u32, high8_vram: u32, high8_pages: u32) {
    // the load list taken over
    let list = m.r32(tm + 0x24);
    if list != 0 {
        let n = m.r32(tm + 0x1c);
        m.w32(tm + 0x14, m.r32(tm + 0x20));
        m.w32(tm + 0xc, n);
        env.call(m, MEM_COPY, &[m.r32(tm + 0x10), list, n.wrapping_mul(RECORD)], &[]);
        let list = m.r32(tm + 0x24);
        if list != 0 {
            env.call(m, FREE, &[list], &[]);
        }
        m.w32(tm + 0x24, 0);
        m.w32(tm + 0x1c, 0);
        m.w32(tm + 0x20, 0);
    }
    let n = m.r32(tm + 0xc);
    if n == 0 {
        return;
    }
    if m.r32(m.r32(APP) + 0x730) == 0 {
        for o in (0x50..0x68).step_by(4) {
            m.w32(tm + o, 0);
        }
        for o in [0x4c, 0x44, 0x48] {
            m.w32(tm + o, 0xffff_ffff);
        }
    }
    if m.r32(tm + 0x2c) != 0 {
        for o in [0x30, 0x34, 0x3c, 0x2c, 0x38] {
            m.w32(tm + o, 0);
        }
    }
    if m.r32(tm + 0x6c) != 0 {
        m.w32(tm + 0x6c, 0);
    }
    let groups = env.call(m, ALLOC, &[GROUPS_NAME, n << 4, 0x10], &[]);
    if groups != 0 {
        env.call(m, MEM_SET, &[groups, 0xdead_c0ed, n << 4], &[]);
    }
    let mut count = 0u32;
    let mut total = 0u32;
    let cluts = (m.r32(tm + 0x14) as i32).min(0x50) as u32;
    m.w32(tm + 0x68, cluts);
    let budget = pages.wrapping_sub(cluts << 2);
    let clut_vram = vram.wrapping_add(budget);
    env.call(m, PRINTF, &[UPLOAD_MESSAGE], &[]);

    // each texture into its group (by pages, ascending; then its kind's flags)
    let mut i = 0u32;
    while (i as i32) < m.r32(tm + 0xc) as i32 {
        let r = slot_record(m, tm, i);
        m.w32(r + 0x30, 0xffff_ffff);
        m.w32(r + 0x24, 0xffff_ffff);
        total = total.wrapping_add(m.r32(r + 0xc));
        m.w32(r + 0x58, 0);
        let mut k = 0u32;
        loop {
            let g = grp(groups, k);
            let r = slot_record(m, tm, i);
            let rp = m.r32(r + 0xc);
            if (rp as i32) < m.r32(g) as i32 || k == count {
                // a new group here
                env.call(m, MEM_MOVE, &[g + 0x10, g, count.wrapping_sub(k) << 4], &[]);
                let r = slot_record(m, tm, i);
                m.w32(g, m.r32(r + 0xc));
                m.w32(g + 4, 0);
                let f = kind_flags(m.r32(r));
                if f != 0 {
                    m.w32(g + 4, f);
                }
                if m.r32(r + 0x20) == 0x13 {
                    m.w32(g + 4, m.r32(g + 4) | 8);
                }
                if m.r32(r + 0x20) == 0x1b {
                    m.w32(r + 0x20, 0x13);
                    m.w32(g + 4, m.r32(g + 4) | 8);
                }
                if m.r32(r + 0x48) == 1 {
                    m.w32(g + 4, m.r32(g + 4) | 0x20);
                }
                m.w32(g + 0xc, 1);
                count += 1;
                m.w32(g + 8, 1);
                break;
            }
            let mut f = kind_flags(m.r32(r));
            let psm = m.r32(r + 0x20);
            if psm == 0x13 {
                f |= 8;
            }
            if psm == 0x1b {
                m.w32(r + 0x20, 0x13);
                f |= 8;
            }
            if m.r32(r + 0x48) == 1 {
                f |= 0x20;
            }
            if m.r32(r + 0xc) == m.r32(g) && f == m.r32(g + 4) {
                m.w32(g + 8, m.r32(g + 8).wrapping_add(1));
                m.w32(g + 0xc, m.r32(g + 0xc).wrapping_add(1));
                break;
            }
            k += 1;
        }
        i += 1;
    }

    // the high8 area: the locked eight-bit groups that fit whole, then the other eight-bit ones, whole or in part
    if high8_pages != 0 {
        let mut left = high8_pages >> 2;
        let start = left;
        for k in 0..count {
            let g = grp(groups, k);
            let f = m.r32(g + 4);
            if f & 0x3a == 0xa {
                let need = m.r32(g + 8).wrapping_mul(m.r32(g));
                if (need as i32) < left as i32 {
                    left = left.wrapping_sub(need);
                    m.w32(g + 4, f | 0x10);
                    env.call(m, PRINTF, &[KEPT_MESSAGE], &[]);
                }
            }
        }
        for k in 0..count {
            let g = grp(groups, k);
            let f = m.r32(g + 4);
            if f & 0x38 == 8 {
                let need = m.r32(g + 8).wrapping_mul(m.r32(g));
                if (need as i32) < left as i32 {
                    left = left.wrapping_sub(need);
                    m.w32(g + 4, f | 0x10);
                }
            }
        }
        if count as i32 > 0 && left != 0 {
            let mut at = grp(groups, count);
            let mut k = 0u32;
            loop {
                let g = grp(groups, k);
                if m.r32(g + 4) & 0x38 == 8 {
                    let fit = (left as i32).wrapping_div(m.r32(g) as i32);
                    if fit > 0 {
                        for o in [0u32, 4, 8, 0xc] {
                            m.w32(at + o, m.r32(g + o));
                        }
                        left = left.wrapping_sub((fit as u32).wrapping_mul(m.r32(g)));
                        count += 1;
                        m.w32(at + 8, fit as u32);
                        m.w32(at + 0xc, fit as u32);
                        m.w32(at + 4, m.r32(at + 4) | 0x10);
                        at += 0x10;
                        m.w32(g + 8, m.r32(g + 8).wrapping_sub(fit as u32));
                        m.w32(g + 0xc, m.r32(g + 0xc).wrapping_sub(fit as u32));
                    }
                }
                k += 1;
                if !((k as i32) < count as i32) || left == 0 {
                    break;
                }
            }
        }
        total = total.wrapping_sub(start.wrapping_sub(left));
    }

    // the plain groups of a size merged
    let mut s = 0u32;
    while (s as i32) < count as i32 {
        let g = grp(groups, s);
        if m.r32(g + 4) & 0x17 == 0 && m.r32(g + 8) != 0 {
            loop {
                let mut hit = None;
                for a in 0..count {
                    if a == s {
                        continue;
                    }
                    let o = grp(groups, a);
                    if m.r32(o + 4) & 0x17 != 0 {
                        continue;
                    }
                    let g = grp(groups, s);
                    if m.r32(o) == m.r32(g) {
                        m.w32(g + 8, m.r32(g + 8).wrapping_add(m.r32(o + 8)));
                        m.w32(g + 0xc, m.r32(g + 0xc).wrapping_add(m.r32(o + 0xc)));
                        m.w32(o + 0xc, 0);
                        m.w32(o + 8, 0);
                        hit = Some(a);
                        break;
                    }
                }
                let Some(a) = hit else { break };
                count -= 1;
                env.call(m, MEM_COPY, &[grp(groups, a), grp(groups, a) + 0x10, count.wrapping_sub(a) << 4], &[]);
            }
        }
        s += 1;
    }

    // slots taken away while over the budget: from the group whose loss costs least
    while (budget as i32) < total as i32 {
        let last_single = m.r32(grp(groups, count).wrapping_sub(4)) == 1;
        let ends = count.wrapping_sub(last_single as u32);
        let mut best = 0xffff_ffffu32;
        let mut least = F(0.0);
        for a in 0..(ends as i32).max(0) as u32 {
            let g = grp(groups, a);
            if (m.r32(g + 0xc) as i32) < 2 || m.r32(g + 4) & 0x17 != 0 {
                continue;
            }
            let mut sum = 0i32;
            let mut score = F(0.0);
            for b in 0..count {
                let o = grp(groups, b);
                let slots = m.r32(o + 0xc).wrapping_sub((a == b) as u32);
                sum = sum.wrapping_add(m.r32(o + 8).wrapping_sub(slots) as i32);
                if slots != 0 {
                    score = score + crate::animseq::div(crate::animseq::cvt_s_w(sum), crate::animseq::cvt_s_w(slots as i32));
                }
            }
            if (best as i32) < 0 || score < least {
                best = a;
                least = score;
            }
        }
        let g = grp(groups, best);
        total = total.wrapping_sub(m.r32(g));
        m.w32(g + 0xc, m.r32(g + 0xc).wrapping_sub(1));
    }
    // and given back while they fit
    loop {
        let mut grew = false;
        if count as i32 > 0 && (m.r32(groups) as i32) < budget.wrapping_sub(total) as i32 {
            let mut a = 0u32;
            loop {
                let g = grp(groups, a);
                if (m.r32(g + 0xc) as i32) < m.r32(g + 8) as i32 {
                    grew = true;
                    m.w32(g + 0xc, m.r32(g + 0xc).wrapping_add(1));
                    total = total.wrapping_add(m.r32(g));
                }
                a += 1;
                if !((a as i32) < count as i32) {
                    break;
                }
                if !((m.r32(grp(groups, a)) as i32) < budget.wrapping_sub(total) as i32) {
                    break;
                }
            }
        }
        if !grew {
            break;
        }
    }

    // the special groups (kept, drawn into, locked, high8) to the front
    let mut front = 0u32;
    for s in 0..count {
        let g = grp(groups, s);
        if m.r32(g + 4) & 0x17 == 0 {
            continue;
        }
        if s > 0 && front < s {
            let held = [0u32, 4, 8, 0xc].map(|o| m.r32(g + o));
            env.call(m, MEM_MOVE, &[groups + 0x10, groups, s << 4], &[]);
            for (k, v) in held.into_iter().enumerate() {
                m.w32(groups + 4 * k as u32, v);
            }
        }
        front += 1;
    }
    m.w32(tm + 0x40, front);
    m.w32(tm + 0x38, 0);
    for k in 0..count {
        let v = m.r32(tm + 0x38).wrapping_add(m.r32(grp(groups, k) + 0xc));
        m.w32(tm + 0x38, v);
    }

    // the blocks (0x18 bytes) and their slots (0x14: +0 the VRAM, +4 the texture, +8 the number, +0xc / +0x10 the
    // next / last)
    m.w32(tm + 0x30, BLOCK_TABLE);
    m.w32(tm + 0x34, BLOCK_TABLE);
    m.w32(tm + 0x2c, count);
    m.w32(tm + 0x3c, BLOCK_TABLE.wrapping_add(count.wrapping_mul(0x18)));
    let mut number = 0u32;
    let mut b = 0u32;
    while (b as i32) < m.r32(tm + 0x2c) as i32 {
        let blk = m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18));
        let g = grp(groups, b);
        m.w32(blk + 4, m.r32(g));
        m.w32(blk, 0);
        m.w32(blk + 8, m.r32(g + 0xc));
        let f = m.r32(g + 4);
        if f & 0x10 != 0 {
            m.w32(blk, 2);
        }
        let f = m.r32(g + 4);
        let extra = if f & 1 != 0 { 4 } else if f & 2 != 0 { 8 } else if f & 4 != 0 { 0x10 } else { 0 };
        if extra != 0 {
            m.w32(blk, m.r32(blk) | extra);
        }
        let first = m.r32(tm + 0x3c).wrapping_add(number.wrapping_mul(0x14));
        m.w32(blk + 0x14, first);
        m.w32(blk + 0xc, first);
        m.w32(blk + 0x10, first);
        let mut k = 0u32;
        while (k as i32) < m.r32(blk + 8) as i32 {
            let s = first.wrapping_add(k.wrapping_mul(0x14));
            m.w32(s + 8, number);
            m.w32(s + 4, 0xffff_ffff);
            number += 1;
            m.w32(s, 0xffff_ffff);
            if k == 0 {
                m.w32(first + 0x10, 0);
            } else {
                m.w32(s + 0x10, s - 0x14);
            }
            if k == m.r32(blk + 8).wrapping_sub(1) {
                m.w32(s + 0xc, 0);
            } else {
                m.w32(s + 0xc, s + 0x14);
            }
            k += 1;
        }
        b += 1;
    }
    // their VRAM (a block's flags: 2 high8, 4 kept, 8 locked, 0x10 drawn into): the high8 ones in the high8 area
    // (4 to the page), then from the start the drawn-into ones, the kept ones, the plain ones (locked ones outside
    // the high8 area get none)
    let mut at = high8_vram;
    for (mask, want, scale, from_start) in [(2u32, 2u32, 2u32, false), (0x10, 0x10, 0, true), (4, 4, 0, false), (0x1e, 0, 0, false)] {
        if from_start {
            at = vram;
        }
        for b in 0..m.r32(tm + 0x2c) {
            let blk = m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18));
            let f = m.r32(blk) & mask;
            if (want == 0 && f != 0) || (want != 0 && f == 0) {
                continue;
            }
            let mut node = m.r32(blk + 0x14);
            while node != 0 {
                m.w32(node, at);
                node = m.r32(node + 0xc);
                at = at.wrapping_add(m.r32(blk + 4) << scale);
            }
        }
    }

    // the GS idle, then the uploads into the slots
    loop {
        let r = m.r32(m.r32(APP) + 0x724);
        let vt = m.r32(r + 0x10);
        let this = r.wrapping_add(m.r16(vt + 0x78) as i16 as i32 as u32);
        let func = m.r32(vt + 0x7c);
        if env.call(m, func, &[this], &[]) != 0 {
            break;
        }
    }
    let mut kept_left = m.r32(tm + 0x28);
    for (want, kind) in [(0xau32, 2u32), (2, 0)] {
        for b in 0..m.r32(tm + 0x2c) {
            let blk = m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18));
            if m.r32(blk) & 0xa != want {
                continue;
            }
            let mut node = m.r32(blk + 0x14);
            while node != 0 {
                let mut i = 0u32;
                while (i as i32) < m.r32(tm + 0xc) as i32 {
                    let r = slot_record(m, tm, i);
                    let fits = (m.r32(r + 0x24) as i32) < 0
                        && m.r32(r + 0xc) == m.r32(m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18)) + 4)
                        && m.r32(r + 0x20) == 0x13
                        && m.r32(r) == kind
                        && (kind != 0 || m.r32(r + 0x48) != 1);
                    if fits {
                        m.w32(node + 4, i);
                        m.w32(r + 0x20, 0x1b);
                        m.w32(r + 0x24, m.r32(node + 8));
                        if kind == 2 {
                            kept_left = kept_left.wrapping_sub(1);
                        }
                        let v = m.r32(node);
                        upload_texture(m, env, tm, i, v);
                        break;
                    }
                    i += 1;
                }
                node = m.r32(node + 0xc);
            }
        }
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(tm + 0xc) as i32 && kept_left != 0 {
        let r = slot_record(m, tm, i);
        if m.r32(r) == 3 {
            kept_left = kept_left.wrapping_sub(1);
            for b in 0..m.r32(tm + 0x2c) {
                let blk = m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18));
                if m.r32(blk) & 0x10 == 0 || m.r32(blk + 4) != m.r32(slot_record(m, tm, i) + 0xc) {
                    continue;
                }
                let node = free_node(m, blk);
                if node != 0 {
                    m.w32(node + 4, i);
                    let r = slot_record(m, tm, i);
                    m.w32(r + 0x24, m.r32(node + 8));
                    break;
                }
            }
        }
        let r = slot_record(m, tm, i);
        if m.r32(r) == 1 {
            kept_left = kept_left.wrapping_sub(1);
            for b in 0..m.r32(tm + 0x2c) {
                let blk = m.r32(tm + 0x34).wrapping_add(b.wrapping_mul(0x18));
                if m.r32(blk) & 4 == 0 || m.r32(blk + 4) != m.r32(slot_record(m, tm, i) + 0xc) {
                    continue;
                }
                let node = free_node(m, blk);
                if node != 0 {
                    m.w32(node + 4, i);
                    let r = slot_record(m, tm, i);
                    m.w32(r + 0x24, m.r32(node + 8));
                    let v = m.r32(node);
                    upload_texture(m, env, tm, i, v);
                    break;
                }
            }
        }
        i += 1;
    }

    // the CLUT slots, 4 to a slot after the budget
    m.w32(tm + 0x6c, CLUT_TABLE);
    let mut at = clut_vram;
    let mut k = 0u32;
    while (k as i32) < m.r32(tm + 0x68) as i32 {
        let e = CLUT_TABLE + (k << 4);
        m.w32(e, at);
        m.w32(e + 0xc, 0xffff_ffff);
        m.w32(e + 4, 4);
        k += 1;
        at = at.wrapping_add(4);
    }
    // the detail levels
    calc_lod_params(m, env, tm);
    let mut i = 0u32;
    while (i as i32) < m.r32(tm + 0xc) as i32 {
        let r = slot_record(m, tm, i);
        let level = m.r32(r + 0x48) as i32;
        if (0..2).contains(&level) {
            m.w32(r + 0x4c, m.r32(tm));
            m.w32(r + 0x50, m.r32(tm + 4));
            m.w32(r + 0x54, m.r32(tm + 8));
        } else if level == 2 {
            m.w32(r + 0x54, 0);
            m.w32(r + 0x4c, 1);
        }
        i += 1;
    }
    // with a race, the three mipmap textures' places (+0x50 / +0x58 / +0x60: the slot and the buffer width at 14)
    // and the world's and players' textures mipmapped (+0x40 1, by format which mipmap +0x44: 8-bit 1, 32-bit 2,
    // 0x1b 0), the others not
    if m.r32(m.r32(APP) + 0x730) != 0 {
        for (o, d) in [(0x44u32, 0x50u32), (0x48, 0x58), (0x4c, 0x60)] {
            let idx = m.r32(tm + o);
            if (idx as i32) >= 0 {
                let r = slot_record(m, tm, idx);
                let slot = m.r32(tm + 0x3c).wrapping_add(m.r32(r + 0x24).wrapping_mul(0x14));
                let tbw = (m.r32(r + 0x10) as i32 >> 6).max(1);
                w64(m, tm + d, (sx(m.r32(slot)) | (sx(tbw as u32) << 14)) as u64);
            }
        }
        let mut i = 0u32;
        while (i as i32) < m.r32(tm + 0xc) as i32 {
            let r = slot_record(m, tm, i);
            let level = m.r32(r + 0x48) as i32;
            if (0..2).contains(&level) {
                m.w32(r + 0x40, 1);
                match m.r32(r + 0x20) {
                    0x13 => m.w32(r + 0x44, 1),
                    0 => m.w32(r + 0x44, 2),
                    0x1b => m.w32(r + 0x44, 0),
                    _ => {}
                }
            } else if level == 2 {
                m.w32(r + 0x40, 0);
            }
            i += 1;
        }
    }
    // every eight-bit texture swizzled into place (through a buffer as big as the biggest)
    let mut most = 0i32;
    let mut i = 0u32;
    while (i as i32) < m.r32(tm + 0xc) as i32 {
        let r = slot_record(m, tm, i);
        i += 1;
        if m.r32(r + 0x20) == 0x13 {
            let a = m.r32(r + 0x10).wrapping_mul(m.r32(r + 0x18)) as i32;
            if most < a {
                most = a;
            }
        }
    }
    if most != 0 {
        let buf = env.call(m, ALLOC, &[SWIZZLE_NAME, most as u32, 0x10], &[]);
        let mut i = 0u32;
        while (i as i32) < m.r32(tm + 0xc) as i32 {
            let r = slot_record(m, tm, i);
            if m.r32(r + 0x20) == 0x13 {
                m.w32(r + 0x3c, 1);
                let (w, h, at) = (m.r32(r + 0x10), m.r32(r + 0x18), m.r32(r + 0x2c));
                env.call(m, MEM_COPY, &[buf, at, w.wrapping_mul(h)], &[]);
                let r = slot_record(m, tm, i);
                env.call(m, TEX_SWIZZLE, &[tm, m.r32(r + 0x10), m.r32(r + 0x18), buf, m.r32(r + 0x2c)], &[]);
            }
            i += 1;
        }
        env.call(m, EAMEM_FREE, &[buf], &[]);
    }
    if groups != 0 {
        env.call(m, FREE, &[groups], &[]);
    }
}

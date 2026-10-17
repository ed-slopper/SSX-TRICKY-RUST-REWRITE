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

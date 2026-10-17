//! The animation bank's bookkeeping and clip queries (board row G5c1): `AnimData` (the bank at 0x3211a8 that the
//! controller's sequences take their clips from), its memory bank, its files' records, the clip markers and the
//! clip names' hashes. The loads (the BIG file, the clip tables) are row G5c2, the pose row G5c3. The bytes are kept
//! at their addresses ([`Mem`]); the allocator, the free, the BIG file object and delete are calls out. Checked in the
//! function runner (`tests/animdata.rs`).
//!
//! **The bank:**
//! - +0 the memory bank: +0 its block, +4 the next free byte, +8 the bytes used, +0xc the block's size;
//! - +0x10 (4), +0x14;
//! - +0x18 0x1be words;
//! - +0x710 the BIG file object (`cAsyncBigFile`);
//! - +0x724 0x1be files of 0x18 bytes: +0 the bank, +4, +8 the clip headers (0x24 bytes each), +0xc, +0x10,
//!   +0x14 (−1).
//!
//! **A clip header (0x24 bytes):** +9 the child count, +0xa the length in frames, then three lists of four shorts:
//! +0xc the markers, +0x14 the marker frames the race scenes set, +0x1c the marker sounds.
//!
//! **A clip's key** (the key table 0x3bc530, a word per animation) is two shorts, the file and the clip in it.

use crate::animseq::{div, SeqEnv};
use crate::pf::Mem;
use crate::ps2f::F;

/// The bank
pub const BANK: u32 = 0x3211a8;
/// The files' records (0x1be of 0x18 from +0x724)
pub const FILES: u32 = 0x724;
pub const NFILE: u32 = 0x1be;
/// The calls out: `Mem_Free` 0x14a040, `EAMem_Alloc` 0x2ccf70(name, size, 0), `cAsyncBigFile`'s constructor
/// 0x23bd10, destructor 0x23bd90(file, flags) and its virtual 0x23bea0, `operator delete` 0x114f80
pub const MEM_FREE: u32 = 0x14a040;
pub const ALLOC: u32 = 0x2ccf70;
pub const ALLOC_NAME: u32 = 0x378fa0;
pub const BIG_CTOR: u32 = 0x23bd10;
pub const BIG_DTOR: u32 = 0x23bd90;
pub const BIG_RESET: u32 = 0x23bea0;
pub const DELETE: u32 = crate::animseq::DELETE;
/// The clip names (0x3242a0, 0x584 string pointers) and their hashes (0x3bdb40)
pub const NAMES: u32 = 0x3242a0;
pub const HASHES: u32 = 0x3bdb40;
pub const NCLIP: u32 = 0x584;

/// `AnimMemBank` 0x157d58: no block
pub fn mem_bank_new(m: &mut Mem, b: u32) -> u32 {
    m.w32(b, 0);
    b
}

/// 0x157d68: the block freed (`Mem_Free`) and forgotten
pub fn mem_bank_free(m: &mut Mem, env: &mut dyn SeqEnv, b: u32) {
    let p = m.r32(b);
    if p != 0 {
        env.call(m, MEM_FREE, &[p]);
        m.w32(b, 0);
    }
}

/// `AnimMemBank_Init` 0x157da0(size): a block of size + 0x80 (filled with 0xdeadc0ed by `EAMem_Set` when given),
/// then the start aligned
pub fn mem_bank_init(m: &mut Mem, env: &mut dyn SeqEnv, b: u32, size: u32) {
    let n = size.wrapping_add(0x80);
    m.w32(b + 0xc, n);
    let p = env.call(m, ALLOC, &[ALLOC_NAME, n, 0]);
    if p != 0 {
        crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, n);
    }
    m.w32(b, p);
    mem_bank_align_start(m, b);
}

/// `AnimMemBank_Destruct` 0x157e10(flags): the block freed; deleted with flag 1
pub fn mem_bank_destruct(m: &mut Mem, env: &mut dyn SeqEnv, b: u32, flags: u32) {
    let p = m.r32(b);
    if p != 0 {
        env.call(m, MEM_FREE, &[p]);
    }
    if flags & 1 != 0 {
        env.call(m, DELETE, &[b]);
    }
}

/// 128 times `v` / 128 toward zero
fn trunc128(v: i32) -> i32 {
    v / 128 * 128
}

/// `AnimMemBank_Alloc` 0x157e68(size): the next free byte; the bank moves on by the size rounded to 128 toward zero
/// plus 128
pub fn mem_bank_alloc(m: &mut Mem, b: u32, size: u32) -> u32 {
    let step = trunc128(size as i32).wrapping_add(0x80) as u32;
    let at = m.r32(b + 4);
    m.w32(b + 4, at.wrapping_add(step));
    m.w32(b + 8, m.r32(b + 8).wrapping_add(step));
    at
}

/// `AnimMemBank_AlignStart` 0x157ea8: the next free byte the block's start rounded up to 128 (as the game rounds:
/// + 0x7f, toward zero), the bytes used what that skipped
pub fn mem_bank_align_start(m: &mut Mem, b: u32) {
    let base = m.r32(b);
    let at = trunc128((base as i32).wrapping_add(0x7f)) as u32;
    m.w32(b + 4, at);
    m.w32(b + 8, at.wrapping_sub(base));
}

/// `AnimFile_Reset` 0x158af0: the file's record empty (+0x14 −1, the bank kept)
pub fn file_reset(m: &mut Mem, f: u32) {
    m.w32(f + 4, 0);
    m.w32(f + 0x14, 0xffff_ffff);
    m.w32(f + 8, 0);
    m.w32(f + 0xc, 0);
    m.w32(f + 0x10, 0);
}

/// `AnimFile_Destruct` 0x15ac48(flags)
pub fn file_destruct(m: &mut Mem, env: &mut dyn SeqEnv, f: u32, flags: u32) {
    file_reset(m, f);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[f]);
    }
}

/// Clip `c`'s header in a file
fn header(m: &Mem, f: u32, c: u32) -> u32 {
    m.r32(f + 8).wrapping_add(c.wrapping_mul(0x24))
}

/// `AnimData_GetMarkerPhase` 0x158798(file, clip, k): marker k over the clip's last frame (at least 1)
pub fn marker_phase(m: &Mem, f: u32, c: u32, k: u32) -> F {
    let h = header(m, f, c);
    let mut last = (m.r16(h + 0xa) as i32).wrapping_sub(1);
    if last <= 0 {
        last = 1;
    }
    let mk = m.r16(h.wrapping_add(k.wrapping_mul(2)) + 0xc) as i32;
    div(F(mk as f32), F(last as f32))
}

/// `AnimData_GetMarkerFrame` 0x1587e8(file, clip, k): the second list's frame k, −1 when unset
pub fn marker_frame(m: &Mem, f: u32, c: u32, k: u32) -> i32 {
    let v = m.r16(header(m, f, c).wrapping_add(k.wrapping_mul(2)) + 0x14);
    if v == 0xffff {
        -1
    } else {
        v as i32
    }
}

/// `AnimData_SetMarker` 0x158818(file, clip, k, v): −1 clears the second list's frame k; any other value goes to
/// the third list (the marker sounds) at k, as the game writes it
pub fn set_marker(m: &mut Mem, f: u32, c: u32, k: u32, v: u32) {
    let h = header(m, f, c).wrapping_add(k.wrapping_mul(2));
    if v == u32::MAX {
        m.w16(h + 0x14, 0xffff);
    } else {
        m.w16(h + 0x1c, v as u16);
    }
}

/// `AnimData_GetClipId` 0x158d68(bank, animation): the animation's key (the key table's word: file, clip)
pub fn clip_id(m: &Mem, anim: u32) -> u32 {
    m.r32(crate::animseq::KEY_TABLE.wrapping_add(anim.wrapping_mul(4)))
}

/// `Name_Hash` 0x240e80 / 0x240ed8: the name's hash (h × 16 + each signed byte; the top four bits, when set, folded
/// back in at bit 23 and cleared)
pub fn name_hash(m: &Mem, mut p: u32) -> u32 {
    let mut h = 0u32;
    loop {
        let c = m.r8(p) as i8;
        if c == 0 {
            return h;
        }
        h = (h << 4).wrapping_add(c as i32 as u32);
        let g = h & 0xf000_0000;
        if g != 0 {
            h = h ^ (g >> 23) ^ g;
        }
        p = p.wrapping_add(1);
    }
}

/// `StaticInit_AnimNameHashes` 0x159820: every clip name's hash
pub fn hash_names(m: &mut Mem) {
    for i in 0..NCLIP {
        let h = name_hash(m, m.r32(NAMES + 4 * i));
        m.w32(HASHES + 4 * i, h);
    }
}

/// `AnimData_Construct` 0x158c38: no block; the BIG file object made; every file empty with no bank; the names
/// hashed; +0x14 0, +0x10 4, the 0x1be words at +0x18 cleared
pub fn construct(m: &mut Mem, env: &mut dyn SeqEnv, b: u32) -> u32 {
    mem_bank_new(m, b);
    env.call(m, BIG_CTOR, &[b + 0x710]);
    for i in 0..NFILE {
        let f = b + FILES + 0x18 * i;
        for o in [0, 4, 8, 0xc, 0x10] {
            m.w32(f + o, 0);
        }
        m.w32(f + 0x14, 0xffff_ffff);
    }
    hash_names(m);
    m.w32(b + 0x14, 0);
    m.w32(b + 0x10, 4);
    for k in 0..0x6f8 {
        m.w8(b + 0x18 + k, 0);
    }
    b
}

/// `AnimData_Clear` 0x158cc8: the block freed; no file has a bank
pub fn clear(m: &mut Mem, env: &mut dyn SeqEnv, b: u32) {
    mem_bank_free(m, env, b);
    for i in 0..NFILE {
        m.w32(b + FILES + 0x18 * i, 0);
    }
}

/// `AnimData_Init` 0x158d18(size): the block made; every file's bank this one
pub fn init(m: &mut Mem, env: &mut dyn SeqEnv, b: u32, size: u32) {
    mem_bank_init(m, env, b, size);
    for i in 0..NFILE {
        m.w32(b + FILES + 0x18 * i, b);
    }
}

/// `AnimData_Reset` 0x159070: the block's start aligned again; every file empty and the +0x18 words 0; +0x10 4,
/// +0x14 0; the BIG file's virtual (0x23bea0) when it has a first word; every key none
pub fn reset(m: &mut Mem, env: &mut dyn SeqEnv, b: u32) {
    mem_bank_align_start(m, b);
    for i in 0..NFILE {
        file_reset(m, b + FILES + 0x18 * i);
        m.w32(b + 0x18 + 4 * i, 0);
    }
    let big = m.r32(b + 0x710);
    m.w32(b + 0x10, 4);
    m.w32(b + 0x14, 0);
    if big != 0 {
        env.call(m, BIG_RESET, &[b + 0x710]);
    }
    let none = m.r32(crate::animseq::NO_KEY);
    for i in 0..NCLIP {
        m.w32(crate::animseq::KEY_TABLE + 4 * i, none);
    }
}

/// `AnimData_Destruct` 0x15a8c0(flags): the files destroyed from the last, the BIG file object (flags 2) and the
/// memory bank (2); deleted with flag 1
pub fn destruct(m: &mut Mem, env: &mut dyn SeqEnv, b: u32, flags: u32) {
    for i in (0..NFILE).rev() {
        file_destruct(m, env, b + FILES + 0x18 * i, 0);
    }
    env.call(m, BIG_DTOR, &[b + 0x710, 2]);
    mem_bank_destruct(m, env, b, 2);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[b]);
    }
}

//! The renderer's draw lists (board row G7h1): `PS2Draw_*` 0x1c4220..0x1c4638 and 0x1c4d30..0x1c4fdc,
//! 0x1c6f98..0x1c720c, and `PS2Render_Submit` / `_FlushState` 0x1e8838 / 0x1e88c8. Every packet the draws hand the
//! renderer (`PS2Render_Submit`) is put in a batch under the state it is drawn with; the batches are found by
//! texture page and state, and sorted by a key before the flush (row G7h2) writes them out. On the bytes at their
//! addresses ([`Mem`]); the sort is a call out ([`DrawListEnv`]). Checked in the function runner
//! (`tests/drawlist.rs`).
//!
//! **The list** (the renderer's +0x480): +0 / +4 the texture pages (0x10 each: the two page words, the head of
//! its batches, the next in its hash chain), +8 / +0xc the batches (0x40 each), +0x3224 the 512 hash heads,
//! +0x3a24 the sort count, +0x3a28 the sorted batches, +0x7a28 their keys, +0xba28 the sort's room. From
//! +0xfa28: the current state (0x18 bytes), +0xfa40 a word kept with it, +0xfa44 / +0xfa48 the texture page,
//! +0xfa4c the page found, +0xfa50 that page, +0xfa54 the batch found, +0xfa58 that batch, +0xfa5c / +0xfa60 two
//! words a batch is also kept apart by, +0xfa64 / +0xfa68 two more set by 0x1c4638.
//!
//! **A state** (`tPS2DrawState`, 0x18 bytes): +0 a word, +4 a half (the texture), +6 a half, +8 / +0xa the texture
//! page, +0xc..+0x17 bytes (+0xe the blend kind, +0xf the layer, +0x10 the VU1 program).
//!
//! **A batch**: +0..+0x17 its state, +0x18 a word left as it was, +0x1c / +0x20 the two words, +0x24 its page,
//! +0x28 its key, +0x2c / +0x30 its DMA start and the end of its last packet, +0x34 whether it follows another,
//! +0x38 the one that follows it, +0x3c the next of its page.

use crate::pf::Mem;

/// The library sort 0x2cfc00(count, keys, room): the keys sorted by their low halves
pub const SORT: u32 = 0x2cfc00;
const S: u32 = 0xfa28;

/// The calls out
pub trait DrawListEnv {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]);
    /// A virtual with these words; its answer
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
}

/// `PS2DrawState_Init` 0x1c4220(_, s): the default state (layer 3, +0x12 1, +0x13 2, +6 12; the rest 0; the
/// texture page left)
pub fn state_init(m: &mut Mem, s: u32) {
    m.w8(s + 0xf, 3);
    m.w8(s + 0x12, 1);
    m.w8(s + 0x13, 2);
    m.w16(s + 6, 12);
    m.w8(s + 0xd, 0);
    m.w8(s + 0xe, 0);
    m.w16(s + 4, 0);
    m.w8(s + 0x10, 0);
    m.w32(s, 0);
    for at in [0x11, 0x14, 0x15, 0xc, 0x16, 0x17] {
        m.w8(s + at, 0);
    }
}

/// `tPS2DrawState::tPS2DrawState` 0x1c71b0(s): the default state with no texture page (−1, −1)
pub fn state_construct(m: &mut Mem, s: u32) {
    m.w16(s + 0xa, 0xffff);
    m.w8(s + 0xf, 3);
    m.w8(s + 0x12, 1);
    m.w8(s + 0x13, 2);
    m.w16(s + 6, 12);
    m.w8(s + 0xe, 0);
    m.w16(s + 4, 0);
    m.w8(s + 0x10, 0);
    m.w32(s, 0);
    for at in [0x11, 0x14, 0x15, 0xc, 0x16, 0x17, 0xd] {
        m.w8(s + at, 0);
    }
    m.w16(s + 8, 0xffff);
}

/// `PS2Draw_ClearBuckets` 0x1c4270(d): the 512 hash heads emptied (from the last)
pub fn clear_buckets(m: &mut Mem, d: u32) {
    for k in 0..0x200u32 {
        m.w32(d + 0x3a20 - 4 * k, 0);
    }
}

/// `PS2Draw_SortBuckets` 0x1c42a0(d): the keys sorted, each then made its batch's index over the key's high half,
/// and sorted again
pub fn sort_buckets(m: &mut Mem, env: &mut dyn DrawListEnv, d: u32) {
    env.call(m, SORT, &[m.r32(d + 0x3a24), d + 0x7a28, d + 0xba28]);
    let mut k = 0u32;
    while (k as i32) < m.r32(d + 0x3a24) as i32 {
        let e = d + 0x7a28 + 4 * k;
        let i = m.r16(e + 2) as i16 as i32 as u32;
        let b = m.r32(d.wrapping_add(0x3a28).wrapping_add(i.wrapping_mul(4)));
        let hi = m.r16(b + 0x2a) as u32;
        m.w32(e, (i << 16) | hi);
        k += 1;
    }
    env.call(m, SORT, &[m.r32(d + 0x3a24), d + 0x7a28, d + 0xba28]);
}

/// `PS2Draw_SetTexturePage` 0x1c4d30(d, a, b): a new page (and no page found) when either word differs
pub fn set_texture_page(m: &mut Mem, d: u32, a: u32, b: u32) {
    if m.r32(d + 0xfa44) != a || m.r32(d + 0xfa48) != b {
        m.w32(d + 0xfa44, a);
        m.w32(d + 0xfa48, b);
        m.w32(d + 0xfa4c, 0);
    }
}

/// `PS2Draw_SetState` 0x1c4d68(d, s): each of the state's fields (bar the texture page) taken where it differs,
/// and then no batch found
pub fn set_state(m: &mut Mem, d: u32, s: u32) {
    let c = d + S;
    // (offset, width) in the game's order
    for (at, w) in [(0xe, 1), (4, 2), (0xf, 1), (0x10, 1), (0, 4), (0x11, 1), (0x12, 1), (0x13, 1), (6, 2), (0x15, 1), (0x14, 1), (0xc, 1), (0x17, 1), (0x16, 1), (0xd, 1)] {
        let (new, old) = match w {
            1 => (m.r8(s + at) as u32, m.r8(c + at) as u32),
            2 => (m.r16(s + at) as u32, m.r16(c + at) as u32),
            _ => (m.r32(s + at), m.r32(c + at)),
        };
        if new != old {
            match w {
                1 => m.w8(c + at, new as u8),
                2 => m.w16(c + at, new as u16),
                _ => m.w32(c + at, new),
            }
            m.w32(d + 0xfa54, 0);
        }
    }
}

/// `PS2Draw_BindTexture` 0x1c4ee8(d, s): the state, then its texture page (+8, +0xa as signed halves)
pub fn bind_texture(m: &mut Mem, d: u32, s: u32) {
    set_state(m, d, s);
    let a = m.r16(s + 8) as i16 as i32 as u32;
    let b = m.r16(s + 0xa) as i16 as i32 as u32;
    set_texture_page(m, d, a, b);
}

/// 0x1c7068(page, a, b): whether a page is the one with these words
pub fn page_is(m: &Mem, p: u32, a: u32, b: u32) -> bool {
    m.r32(p) == a && m.r32(p + 4) == b
}

/// `PS2Draw_StateEquals` 0x1c6f98(a, b): the states alike in everything bar the texture page
pub fn state_equals(m: &Mem, a: u32, b: u32) -> bool {
    m.r16(a + 4) == m.r16(b + 4)
        && m.r32(a + 0xc) & 0xffff_0000 == m.r32(b + 0xc) & 0xffff_0000
        && m.r8(a + 0x10) == m.r8(b + 0x10)
        && m.r32(a) == m.r32(b)
        && m.r32(a + 0x10) & 0xffff_ff00 == m.r32(b + 0x10) & 0xffff_ff00
        && m.r16(a + 6) == m.r16(b + 6)
        && m.r32(a + 0x14) & 0xffff == m.r32(b + 0x14) & 0xffff
        && m.r8(a + 0xc) == m.r8(b + 0xc)
        && m.r32(b + 0x14) & 0xffff_0000 == m.r32(a + 0x14) & 0xffff_0000
        && m.r8(a + 0xd) == m.r8(b + 0xd)
}

/// 0x1c4340(d): the current texture page found in its hash chain (its first word, negative as 0, mod 512), or
/// made at the chain's head; either way found, and no batch found
pub fn find_page(m: &mut Mem, d: u32) {
    let a = m.r32(d + 0xfa44) as i32;
    let key = if a > -1 { a } else { 0 };
    let head = d + 0x3224 + 4 * (key as u32 & 0x1ff);
    let first = m.r32(head);
    let mut p = first;
    while p != 0 {
        if page_is(m, p, m.r32(d + 0xfa44), m.r32(d + 0xfa48)) {
            m.w32(d + 0xfa50, p);
            m.w32(d + 0xfa4c, 1);
            m.w32(d + 0xfa54, 0);
            return;
        }
        p = m.r32(p + 0xc);
    }
    let n = m.r32(d);
    let p = m.r32(d + 4).wrapping_add(n << 4);
    m.w32(d, n.wrapping_add(1));
    m.w32(p + 0xc, first);
    let a = m.r32(d + 0xfa44);
    m.w32(head, p);
    m.w32(p, a);
    m.w32(d + 0xfa50, p);
    let b = m.r32(d + 0xfa48);
    m.w32(d + 0xfa4c, 1);
    m.w32(p + 4, b);
    m.w32(d + 0xfa54, 0);
    m.w32(p + 8, 0);
}

/// `PS2Draw_FindOrAddBatch` 0x1c4468(d, follows): the page's batch with the current state and words (one that
/// doesn't follow another), or a new one at the head of the page's batches. A new one that follows (with a batch
/// before it) is hung off that batch; any other is keyed ([`pack_sort_key`]) into the sort list.
pub fn find_or_add_batch(m: &mut Mem, d: u32, follows: u32) {
    let page = m.r32(d + 0xfa50);
    let last = m.r32(d + 0xfa58);
    let first = m.r32(page + 8);
    let mut b = first;
    while b != 0 {
        if m.r32(b + 0x1c) == m.r32(d + 0xfa5c) && m.r32(b + 0x20) == m.r32(d + 0xfa60) && m.r32(b + 0x34) == 0 && state_equals(m, b, d + S) {
            m.w32(d + 0xfa58, b);
            m.w32(d + 0xfa54, 1);
            return;
        }
        b = m.r32(b + 0x3c);
    }
    let n = m.r32(d + 8);
    let b = m.r32(d + 0xc).wrapping_add(n << 6);
    m.w32(d + 8, n.wrapping_add(1));
    m.w32(b + 0x3c, first);
    let kept = m.r32(b + 0x18);
    let page = m.r32(d + 0xfa50);
    m.w32(page + 8, b);
    let state: Vec<u8> = (0..0x1c).map(|k| m.r8(d + S + k)).collect();
    for (k, v) in state.into_iter().enumerate() {
        m.w8(b + k as u32, v);
    }
    m.w32(b + 0x1c, m.r32(d + 0xfa5c));
    let page = m.r32(d + 0xfa50);
    let w = m.r32(d + 0xfa60);
    m.w32(b + 0x34, follows);
    m.w32(b + 0x38, 0);
    m.w32(b + 0x18, kept);
    m.w32(b + 0x20, w);
    m.w32(b + 0x24, page);
    m.w32(b + 0x2c, 0);
    m.w32(b + 0x30, 0);
    m.w32(d + 0xfa58, b);
    m.w32(d + 0xfa54, 1);
    if follows != 0 && last != 0 {
        m.w32(last + 0x34, 1);
        m.w32(last + 0x38, b);
        return;
    }
    let key = pack_sort_key(m, d);
    m.w32(b + 0x28, key);
    let n = m.r32(d + 0x3a24);
    let e = d.wrapping_add(n.wrapping_mul(4));
    m.w32(e + 0x7a28, (n << 16) | (key & 0xffff));
    m.w32(e + 0x3a28, b);
    let n = m.r32(d + 0x3a24);
    m.w32(d + 0x3a24, n.wrapping_add(1));
}

/// The sort key's program rank by the VU1 program (+0x10; 11 and over 0)
const RANK: [u32; 11] = [0, 7, 5, 4, 6, 3, 1, 1, 2, 2, 1];

/// `PS2Draw_PackSortKey` 0x1c7088(d): the current batch's key: (0x13 − its layer) << 27, its blend's order << 17
/// (none 0x3ff, kind 1 0x3fe, kind 2 its texture, others 0), its program's rank << 14, its page's first word << 5
/// and its +0x1c (both negative as 0)
pub fn pack_sort_key(m: &Mem, d: u32) -> u32 {
    let b = m.r32(d + 0xfa58);
    let layer = 0x13u32.wrapping_sub(m.r8(b + 0xf) as u32);
    let blend = match m.r8(b + 0xe) {
        0 => 0x3ff,
        1 => 0x3fe,
        2 => m.r16(b + 4) as u32,
        _ => 0,
    };
    let b = m.r32(d + 0xfa58);
    let prog = m.r8(b + 0x10) as usize;
    let rank = if prog < 11 { RANK[prog] } else { 0 };
    let b = m.r32(d + 0xfa58);
    let page = m.r32(d + 0xfa50);
    let w = (m.r32(b + 0x1c) as i32).max(0) as u32;
    let a = (m.r32(page) as i32).max(0) as u32;
    (layer << 27) | w | (rank << 14) | (blend << 17) | (a << 5)
}

/// 0x1c4f28(d, start, end, follows): a packet into the current batch (the page and batch found first when they
/// aren't): its first packet's start, or a DMA `next` to it at the end of the last one; the end kept
pub fn add_packet(m: &mut Mem, d: u32, start: u32, end: u32, follows: u32) {
    if m.r32(d + 0xfa4c) == 0 {
        find_page(m, d);
    }
    if m.r32(d + 0xfa54) == 0 {
        find_or_add_batch(m, d, follows);
    }
    let b = m.r32(d + 0xfa58);
    if m.r32(b + 0x2c) == 0 {
        m.w32(b + 0x2c, start);
        let b = m.r32(d + 0xfa58);
        m.w32(b + 0x30, end);
    } else {
        let at = m.r32(b + 0x30);
        m.w32(at + 8, 0);
        m.w32(at + 0xc, 0);
        m.w32(at, 0x2000_0000);
        m.w32(at + 4, start);
        let b = m.r32(d + 0xfa58);
        m.w32(b + 0x30, end);
    }
}

/// 0x1c4638(d, a, b): +0xfa64 = a, +0xfa68 = b
pub fn set_words(m: &mut Mem, d: u32, a: u32, b: u32) {
    m.w32(d + 0xfa68, b);
    m.w32(d + 0xfa64, a);
}

/// The renderer's pending state into its list: the texture state (+0x470) when +0x474 is set, its texture page
/// when +0x478 is
fn flush(m: &mut Mem, r: u32) {
    if m.r32(r + 0x474) != 0 {
        let (d, s) = (m.r32(r + 0x480), m.r32(r + 0x470));
        set_state(m, d, s);
        m.w32(r + 0x474, 0);
    }
    if m.r32(r + 0x478) != 0 {
        let s = m.r32(r + 0x470);
        let d = m.r32(r + 0x480);
        let (a, b) = (m.r16(s + 8) as i16 as i32 as u32, m.r16(s + 0xa) as i16 as i32 as u32);
        set_texture_page(m, d, a, b);
        m.w32(r + 0x478, 0);
    }
}

/// `PS2Render_Submit` 0x1e8838(r, start, end, follows): the pending state, then the packet into the list
pub fn submit(m: &mut Mem, r: u32, start: u32, end: u32, follows: u32) {
    flush(m, r);
    let d = m.r32(r + 0x480);
    add_packet(m, d, start, end, follows);
}

/// `PS2Render_FlushState` 0x1e88c8(r): the pending state alone
pub fn flush_state(m: &mut Mem, r: u32) {
    flush(m, r);
}

// ---- The flush (row G7h2) ----

/// The texture upload 0x1c3818(cache, texture, context, &packet)
pub const UPLOAD: u32 = 0x1c3818;
/// `PS2Draw_WriteTexState` 0x1c5c88(list, program, how, word, &packet) (row G7h3)
pub const WRITE_TEX_STATE: u32 = 0x1c5c88;
/// `BxList_DrawAll` 0x17d9d8(lists, list)
pub const DRAW_ALL: u32 = 0x17d9d8;
/// The app (its +0x724 the renderer, +0x730 an object asked whether to upload +0x30's +0x70)
pub const APP: u32 = 0x337c58;

fn sd(m: &mut Mem, at: u32, v: u64) {
    m.w32(at, v as u32);
    m.w32(at.wrapping_add(4), (v >> 32) as u32);
}

/// A GS register packet being built (`b`; the game's is on its stack): +0 the registers, +4 where the next goes,
/// +8 the packet's start (its header's place). One register: its value, then its number.
fn gs_put(m: &mut Mem, b: u32, value: u64, reg: u64) {
    let p = m.r32(b + 4);
    sd(m, p, value);
    m.w32(b + 4, p.wrapping_add(8));
    sd(m, p.wrapping_add(8), reg);
    m.w32(b + 4, p.wrapping_add(0x10));
}

/// `GsPkt_SetScissor` 0x1c0340(b, x, w, y, h): SCISSOR_1 and _2 (x .. x + w − 1, y .. y + h − 1)
pub fn gs_scissor(m: &mut Mem, b: u32, x: u32, w: u32, y: u32, h: u32) {
    let sx = |v: u32| v as i32 as i64 as u64;
    let x1 = sx(x.wrapping_sub(1).wrapping_add(w));
    let y1 = sx(y.wrapping_sub(1).wrapping_add(h));
    let v = sx(x) | (y1 << 48) | ((sx(y) << 32) | (x1 << 16));
    let n = m.r32(b);
    gs_put(m, b, v, 0x40);
    gs_put(m, b, v, 0x41);
    m.w32(b, n.wrapping_add(2));
}

/// 0x1c03b8(b): TEXFLUSH
pub fn gs_texflush(m: &mut Mem, b: u32) {
    let n = m.r32(b);
    gs_put(m, b, 0, 0x3f);
    m.w32(b, n.wrapping_add(1));
}

/// A register by context: the second for 2, else the first
fn context(ctx: u32, first: u64) -> u64 {
    if ctx == 2 {
        first + 1
    } else {
        first
    }
}

/// `GsPkt_SetAlpha` 0x1c03e8(b, mode, fix, context): ALPHA_1 / _2 by the blend mode (9 and 10 with the fixed
/// alpha; past 10 none, though counted)
pub fn gs_alpha(m: &mut Mem, b: u32, mode: u32, fix: u32, ctx: u32) {
    const MODES: [u64; 9] = [0x2a, 0x6a, 0x89, 0x44, 0x46, 0x48, 0x81, 0x42, 0x49];
    let reg = context(ctx, 0x42);
    match mode {
        0..=8 => gs_put(m, b, MODES[mode as usize], reg),
        9 | 10 => gs_put(m, b, (((fix & 0xff) as u64) << 32) | if mode == 9 { 0x64 } else { 0x68 }, reg),
        _ => {}
    }
    let n = m.r32(b);
    m.w32(b, n.wrapping_add(1));
}

/// One register with the place moved on before the value is written (the clamp's and z buffer's order)
fn gs_put_late(m: &mut Mem, b: u32, value: u64, reg: u64) {
    let p = m.r32(b + 4);
    m.w32(b + 4, p.wrapping_add(8));
    sd(m, p, value);
    let n = m.r32(b);
    let q = m.r32(b + 4);
    sd(m, q, reg);
    m.w32(b, n.wrapping_add(1));
    m.w32(b + 4, q.wrapping_add(8));
}

/// `GsPkt_SetClamp` 0x1c04f8(b, clamp, context): CLAMP_1 / _2, clamped both ways (5) when 1, else repeat
pub fn gs_clamp(m: &mut Mem, b: u32, clamp: u32, ctx: u32) {
    gs_put_late(m, b, if clamp == 1 { 5 } else { 0 }, context(ctx, 8));
}

/// `GsPkt_SetZBuf` 0x1c0560(b, base, format, no_write, context): ZBUF_1 / _2 (the format less 0x30 at bit 24, the
/// mask bit 32 when 1)
pub fn gs_zbuf(m: &mut Mem, b: u32, base: u32, format: u32, mask: u32, ctx: u32) {
    let mut v = base as u64 | ((format.wrapping_sub(0x30) as u64) << 24);
    if mask == 1 {
        v |= 1 << 32;
    }
    gs_put_late(m, b, v, context(ctx, 0x4e));
}

/// `GsPkt_SetTest` 0x1c05d8(b, depth, alpha, aref, context): TEST_1 / _2 with the alpha and depth tests on: the
/// alpha test 2 for 1, 6 for 2, else 1; the depth test 2 for 1, 3 for 0, else 1
pub fn gs_test(m: &mut Mem, b: u32, depth: u32, alpha: u32, aref: u32, ctx: u32) {
    let reg = context(ctx, 0x47);
    let atst: u64 = match alpha as i32 {
        1 => 2,
        2 => 6,
        _ => 1,
    };
    let ztst: u64 = match depth as i32 {
        1 => 2,
        0 => 3,
        _ => 1,
    };
    let v = (atst << 1) | (ztst << 17) | ((aref as i32 as i64 as u64) << 4) | 1 | 0x10000;
    let n = m.r32(b);
    gs_put(m, b, v, reg);
    m.w32(b, n.wrapping_add(1));
}

/// `GsPkt_Close` 0x1c06a0(b, &packet, how): nothing when no registers; else the header (a DMA tag over them, a VIF
/// code and a GIF tag for A+D). How 0: a `ret` packet at 0x33d6c0 called from the packet; 1: a `cnt` at the
/// start with VIF DIRECT, the packet moved on to the registers' end; 2: as 1 with VIF UNPACK to VU1 0x3f2. Any
/// other how, nothing.
pub fn gs_close(m: &mut Mem, b: u32, pkt: u32, how: u32) {
    let n = m.r32(b);
    if n == 0 {
        return;
    }
    let gif = (n | 0x8000) as u64 | 1 << 60;
    match how {
        0 => {
            let h = 0x33d6c0;
            sd(m, h, (n.wrapping_add(2) | 0x6000_0000) as u64);
            sd(m, h + 8, 0);
            sd(m, h + 0x10, 0);
            sd(m, h + 0x18, ((n.wrapping_add(1) | 0x5000_0000) as u64) << 32);
            sd(m, h + 0x20, gif);
            sd(m, h + 0x28, 0xe);
            m.w32(b + 4, h + 0x30);
            let p = m.r32(pkt);
            sd(m, p, ((h as u64) << 32) | 0x5000_0000);
            sd(m, p.wrapping_add(8), 0);
            m.w32(pkt, p.wrapping_add(0x10));
        }
        1 | 2 => {
            let end = m.r32(b + 4);
            m.w32(pkt, end);
            let h = m.r32(b + 8);
            let vif = if how == 1 { n.wrapping_add(1) | 0x5000_0000 } else { (n.wrapping_add(1) << 16) | 0x6c00_03f2 };
            sd(m, h, (n.wrapping_add(2) | 0x1000_0000) as u64);
            sd(m, h.wrapping_add(8), 0);
            sd(m, h.wrapping_add(0x10), 0);
            sd(m, h.wrapping_add(0x18), (vif as u64) << 32);
            sd(m, h.wrapping_add(0x20), gif);
            sd(m, h.wrapping_add(0x28), 0xe);
            m.w32(b + 4, h.wrapping_add(0x30));
        }
        _ => {}
    }
}

/// A packet's opening: a DMA `cnt` of one quadword with an empty GIF tag after it (0x20 bytes)
fn open(m: &mut Mem, pkt: u32) {
    let p = m.r32(pkt);
    sd(m, p, 0x1000_0001);
    sd(m, p.wrapping_add(0x18), 1 << 60);
    m.w32(pkt, p.wrapping_add(0x20));
    sd(m, p.wrapping_add(8), 0);
    sd(m, p.wrapping_add(0x10), 0);
}

/// A register packet begun at the packet's place, its registers 0x30 on (the builder at `b`; the game's is on
/// its stack: STANDIN)
fn begin(m: &mut Mem, b: u32, pkt: u32) {
    let p = m.r32(pkt);
    m.w32(b, 0);
    m.w32(b + 4, p.wrapping_add(0x30));
    m.w32(b + 8, p);
}

/// `PS2Draw_WriteFullState` 0x1c51f0(d, &packet): the default state written whole: its VU1 program (how 0), then
/// both contexts' clamp, alpha, z buffer (the list's +0xfa68 / +0xfa64) and test. `scratch` holds the default
/// state and the builder (the game's stack: STANDIN; 0x30 bytes).
pub fn write_full_state(m: &mut Mem, env: &mut dyn DrawListEnv, d: u32, pkt: u32, scratch: u32) {
    let s = scratch;
    state_init(m, s);
    open(m, pkt);
    let (prog, word) = (m.r8(s + 0x10) as u32, m.r32(s));
    env.call(m, WRITE_TEX_STATE, &[d, prog, 0, word, pkt]);
    let b = scratch + 0x20;
    begin(m, b, pkt);
    for ctx in [1, 2] {
        let (clamp, alpha, fix) = if ctx == 1 { (0x14, 0x15, 0xc) } else { (0x16, 0x17, 0xd) };
        let c = m.r8(s + clamp) as u32;
        gs_clamp(m, b, c, ctx);
        let (a, f) = (m.r8(s + alpha) as u32, m.r8(s + fix) as u32);
        gs_alpha(m, b, a, f, ctx);
        let (zb, zf, zm) = (m.r32(d + 0xfa68), m.r32(d + 0xfa64), m.r8(s + 0x11) as u32);
        gs_zbuf(m, b, zb, zf, zm, ctx);
        let (t1, t2, t3) = (m.r8(s + 0x12) as u32, m.r8(s + 0x13) as u32, m.r16(s + 6) as i16 as i32 as u32);
        gs_test(m, b, t1, t2, t3, ctx);
    }
    gs_close(m, b, pkt, 1);
}

/// `PS2Draw_WriteStateChanges` 0x1c5368(d, batch, word, view, &packet): what of the batch's state differs from
/// the list's, written and taken into the list's: its VU1 program (how 0 for a new program, 1 for a new word, 2
/// for a new state word; none for program 0), its view's scissor (when the view changed and isn't negative), the
/// z buffer, the tests, each context's clamp (the second's not written when 2) and alpha. `scratch` holds the
/// builder (STANDIN; 0x10 bytes).
pub fn write_state_changes(m: &mut Mem, env: &mut dyn DrawListEnv, d: u32, bt: u32, word: u32, view: u32, pkt: u32, scratch: u32) {
    let c = d + S;
    let prog = m.r8(bt + 0x10);
    open(m, pkt);
    if prog != 0 {
        let how = if m.r8(c + 0x10) != prog {
            Some(0)
        } else if m.r32(d + 0xfa5c) != word {
            Some(1)
        } else {
            None
        };
        if let Some(how) = how {
            m.w8(c + 0x10, prog);
            let w = m.r32(bt);
            m.w32(d + 0xfa5c, word);
            m.w32(c, w);
            env.call(m, WRITE_TEX_STATE, &[d, prog as u32, how, w, pkt]);
        }
        let w = m.r32(bt);
        if m.r32(c) != w {
            let p = m.r8(bt + 0x10);
            m.w32(c, w);
            m.w8(c + 0x10, p);
            m.w32(d + 0xfa5c, word);
            env.call(m, WRITE_TEX_STATE, &[d, p as u32, 2, w, pkt]);
        }
    }
    let b = scratch;
    begin(m, b, pkt);
    if view != m.r32(d + 0xfa60) {
        m.w32(d + 0xfa60, view);
        if (view as i32) >= 0 {
            let v = d.wrapping_add(view << 3);
            let (x, y, w, h) = (m.r16(v + 0x3024) as u32, m.r16(v + 0x3026) as u32, m.r16(v + 0x3028) as u32, m.r16(v + 0x302a) as u32);
            gs_scissor(m, b, x, w, y, h);
        }
    }
    if m.r8(c + 0x11) != m.r8(bt + 0x11) {
        let z = m.r8(bt + 0x11);
        m.w8(c + 0x11, z);
        for ctx in [1, 2] {
            let (zb, zf, zm) = (m.r32(d + 0xfa68), m.r32(d + 0xfa64), m.r8(c + 0x11) as u32);
            gs_zbuf(m, b, zb, zf, zm, ctx);
        }
    }
    if m.r16(c + 0x12) != m.r16(bt + 0x12) || m.r16(c + 6) != m.r16(bt + 6) {
        let (t1, t2) = (m.r8(bt + 0x12), m.r8(bt + 0x13));
        m.w8(c + 0x12, t1);
        m.w8(c + 0x13, t2);
        let r = m.r16(bt + 6);
        m.w16(c + 6, r);
        gs_test(m, b, t1 as u32, t2 as u32, r as i16 as i32 as u32, 1);
        let (t1, t2, r) = (m.r8(c + 0x12) as u32, m.r8(c + 0x13) as u32, m.r16(c + 6) as i16 as i32 as u32);
        gs_test(m, b, t1, t2, r, 2);
    }
    if m.r8(c + 0x14) != m.r8(bt + 0x14) {
        let v = m.r8(bt + 0x14);
        m.w8(c + 0x14, v);
        let v = m.r8(c + 0x14) as u32;
        gs_clamp(m, b, v, 1);
    }
    if m.r8(c + 0x15) != m.r8(bt + 0x15) || m.r8(c + 0xc) != m.r8(bt + 0xc) {
        let a = m.r8(bt + 0x15);
        m.w8(c + 0x15, a);
        let f = m.r8(bt + 0xc);
        m.w8(c + 0xc, f);
        gs_alpha(m, b, a as u32, f as u32, 1);
    }
    let v = m.r8(bt + 0x16);
    if m.r8(c + 0x16) != v {
        m.w8(c + 0x16, v);
        if v != 2 {
            gs_clamp(m, b, v as u32, 2);
        }
    }
    if m.r8(c + 0x17) != m.r8(bt + 0x17) || m.r8(c + 0xd) != m.r8(bt + 0xd) {
        let a = m.r8(bt + 0x17);
        m.w8(c + 0x17, a);
        let f = m.r8(bt + 0xd);
        m.w8(c + 0xd, f);
        gs_alpha(m, b, a as u32, f as u32, 2);
    }
    gs_close(m, b, pkt, 1);
}

/// `PS2Draw_FlushBuckets` 0x1c4fe0(d, &packet): the list written out: no words, the default state written whole,
/// the sort; then each sorted batch and those that follow it: its page's textures uploaded when the page changed
/// (each word that isn't negative, as context 1 and 2), its state changes, its packets closed with a `ret` and
/// called. Then the app's +0x730 asked (its +0x20) whether to upload the texture at the app's +0x30's +0x70, and
/// the packet ended. The answer is the packet's start. `scratch` as for [`write_full_state`] (0x30 bytes).
pub fn flush_buckets(m: &mut Mem, env: &mut dyn DrawListEnv, d: u32, pkt: u32, scratch: u32) -> u32 {
    let app = m.r32(APP);
    let start = m.r32(pkt);
    let r = m.r32(app + 0x724);
    m.w32(d + 0xfa60, u32::MAX);
    let cache = m.r32(r + 0x47c);
    m.w32(d + 0xfa5c, u32::MAX);
    state_init(m, d + S);
    write_full_state(m, env, d, pkt, scratch);
    sort_buckets(m, env, d);
    let mut last_page = 0u32;
    let mut k = 0u32;
    while (k as i32) < m.r32(d + 0x3a24) as i32 {
        let i = m.r16(d + 0x7a28 + 4 * k + 2) as i16 as i32 as u32;
        let mut bt = m.r32(d.wrapping_add(0x3a28).wrapping_add(i.wrapping_mul(4)));
        k += 1;
        while bt != 0 {
            let page = m.r32(bt + 0x24);
            if page != last_page {
                last_page = page;
                for (at, ctx) in [(0, 1), (4, 2)] {
                    let t = m.r32(page + at);
                    if (t as i32) >= 0 {
                        env.call(m, UPLOAD, &[cache, t, ctx, pkt]);
                    }
                }
            }
            let (w, v) = (m.r32(bt + 0x1c), m.r32(bt + 0x20));
            write_state_changes(m, env, d, bt, w, v, pkt, scratch);
            let end = m.r32(bt + 0x30);
            sd(m, end, 0x6000_0000);
            sd(m, end.wrapping_add(8), 0);
            let p = m.r32(pkt);
            let first = m.r32(bt + 0x2c);
            bt = m.r32(bt + 0x38);
            sd(m, p, ((first as u64) << 32) | 0x5000_0000);
            sd(m, p.wrapping_add(8), 0);
            m.w32(pkt, p.wrapping_add(0x10));
        }
    }
    let app = m.r32(APP);
    let o = m.r32(app + 0x730);
    if o != 0 {
        let vt = m.r32(o + 8);
        let this = (o + 8).wrapping_add(m.r16(vt + 0x20) as i16 as i32 as u32);
        let f = m.r32(vt + 0x24);
        if env.vcall(m, f, this, &[]) != 0 {
            let app = m.r32(APP);
            let t = m.r32(m.r32(app + 0x30) + 0x70);
            env.call(m, UPLOAD, &[cache, t, 1, pkt]);
        }
    }
    let p = m.r32(pkt);
    sd(m, p, 0x7000_0000);
    m.w32(pkt, p.wrapping_add(0x10));
    sd(m, p.wrapping_add(8), 0);
    start
}

/// `Game_DrawLists` 0x1c7290: the thirteen object lists at 0x33d7d8 drawn (`BxList_DrawAll` on 0x337c60)
pub fn game_draw_lists(m: &mut Mem, env: &mut dyn DrawListEnv) {
    for k in 0..13 {
        let l = m.r32(0x33d7d8 + 4 * k);
        env.call(m, DRAW_ALL, &[0x337c60, l]);
    }
}

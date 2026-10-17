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

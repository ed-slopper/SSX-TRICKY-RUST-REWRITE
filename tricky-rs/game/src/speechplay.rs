//! The speech library's effect player, part two (board row G10l): the small helpers of 0x2bbcf8..0x2bd078 that
//! read the sound banks' event headers and the voice records. Every call to another function of the library goes
//! through a [`LoadEnv`] by address; the data is words at the game's own addresses on a [`Mem`].
//!
//! A sound header (bank entry) is: +4 a byte, low nibble n, bits 4..6 the number of key words m, bit 7 a flag;
//! +5 a byte, the count of rows; +6 a byte; the rows start at +0xc, each n + 2 bytes; then m big-endian key words
//! (the top three bits a kind, the low thirteen a value), then (when the flag bit 7 is set) a bitmap of the rows.

use crate::speechlib::Mem;
use crate::speechload::LoadEnv;

pub const POST_EVENT: u32 = 0x2bba80;
pub const RELEASE_TAIL: u32 = 0x2badc0;
pub const BANK_ID: u32 = 0x2bb180;
pub const BANK_SLOT: u32 = 0x2bb1f0;
pub const BANK_HAS: u32 = 0x2bb270;
pub const TEST_BIT: u32 = 0x2bc9a0;
pub const TEST_ENTRY: u32 = 0x2bc9d8;

const GROUPS: u32 = 0x348e48;
const TABLE: u32 = 0x348dcc;
const CALLBACK: u32 = 0x348dec;
const WEIGHTS: u32 = 0x348930;
const ENTRIES: u32 = 0x3494b0;
const SLOTS: u32 = 0x3494f8;
const TOTAL: u32 = 0x3494f4;

/// 0x2bbcf8 (`SpeechLib_PostEventArgs`): the event and its `n` arguments (the first six arrive in registers, saved
/// in eight-byte slots) are put in a word array on the frame `sp` and posted.
pub fn post_event_args(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, event: u32, n: i32, args: &[u32]) {
    m.w32(sp, event);
    for k in 0..n.max(0) as usize {
        m.w32(sp.wrapping_add(4 + 4 * k as u32), args.get(k).copied().unwrap_or(0));
    }
    e.call(m, POST_EVENT, &[sp]);
}

/// 0x2bbd68: two words cleared.
pub fn clear_pair(m: &mut dyn Mem, p: u32) {
    m.w32(p + 4, 0);
    m.w32(p, 0);
}

/// 0x2bbd78: `id` (16 bits) is among the `list` object's words: +0 the count, +4 the pointer to the halfwords.
pub fn list_contains(m: &dyn Mem, id: u32, list: u32) -> bool {
    let n = m.r32(list) as i32;
    let id = id & 0xffff;
    if n <= 0 {
        return false;
    }
    let p = m.r32(list + 4);
    (0..n as u32).any(|k| m.r16(p.wrapping_add(2 * k)) as u32 == id)
}

/// 0x2bc170: every voice record out of use (the group counts down by one each), the eight group counts that are
/// not zero cleared, then the library's tail 0x2badc0.
pub fn release_all(m: &mut dyn Mem, e: &mut dyn LoadEnv) {
    for i in 0..16u32 {
        let r = GROUPS + 0x60 * i;
        if m.r8(r + 0x6a) != 0 {
            let g = m.r8(r + 0x6b) as u32;
            m.w8(r + 0x6a, 0);
            let c = m.r32(GROUPS + 4 * g).wrapping_sub(1);
            m.w32(GROUPS + 4 * g, c);
        }
    }
    for k in 0..8 {
        if m.r32(GROUPS + 4 * k) != 0 {
            m.w32(GROUPS + 4 * k, 0);
        }
    }
    e.call(m, RELEASE_TAIL, &[]);
}

/// 0x2bc1e8: the count of group `g`.
pub fn group_count(m: &dyn Mem, g: u32) -> u32 {
    m.r32(GROUPS.wrapping_add(g << 2))
}

/// 0x2bc490: the checking callback (the function whose address is at 0x348dec) on `arg`, 1 without one.
pub fn callback_check(m: &mut dyn Mem, e: &mut dyn LoadEnv, arg: u32) -> u32 {
    let f = m.r32(CALLBACK);
    if f != 0 {
        e.call(m, f, &[arg])
    } else {
        1
    }
}

/// 0x2bc720: slot `i` of the word table at 0x3494f8.
pub fn set_slot(m: &mut dyn Mem, v: u32, i: u32) {
    m.w32(SLOTS.wrapping_add(i << 2), v);
}

/// 0x2bc7a0: the rows' masks all agree. `a` has the bit numbers (bytes at +0xc), `b` a count at +4 (a byte, up to
/// 4), the row's values (bytes at +8) and the masks (words at +0x10); `t` is a table of words indexed by a row's
/// value (0 means none, 0xff and 0xfe pass).
pub fn masks_agree(m: &dyn Mem, a: u32, b: u32, t: u32) -> bool {
    let n = m.r8(b + 4) as i8 as i32;
    if n >= 5 {
        return false;
    }
    let mut ok;
    for j in 0..n.max(0) as u32 {
        let bit = m.r8(a.wrapping_add(0xc + j)) as u32;
        if bit >= 0x20 {
            return false;
        }
        let mask = 1u32 << bit;
        ok = false;
        if m.r32(b + 0x10 + 4 * j) & mask != 0 {
            let v = m.r8(b + 8 + j) as u32;
            ok = true;
            if v != 0 && v != 0xff && v != 0xfe {
                ok = m.r32(t.wrapping_add(v << 2)) & mask != 0;
            }
        }
        if !ok {
            return false;
        }
    }
    true
}

/// 0x2bc850: the two ids a header names (`out`: two halfwords, -1 when none) and whether the first was found.
pub fn lookup_ids(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32, table: u32, out: u32) -> bool {
    let kind_idx = m.r8(p + 2) as u32;
    let id = m.r16(p) as u32;
    let kind = m.r8(p + 3);
    m.w16(out + 2, 0xffff);
    m.w16(out, 0xffff);
    if kind == 1 {
        if kind_idx >= 0x14 {
            m.w16(out, 0xffff);
        } else {
            let a1 = m.r16(table.wrapping_add(kind_idx << 2)) as u32;
            let v = e.call(m, BANK_SLOT, &[id, a1]);
            m.w16(out, v as u16);
        }
    } else if kind == 0 {
        let v = e.call(m, BANK_ID, &[id]);
        m.w16(out, v as u16);
    } else if kind == 2 {
        let mut s0 = e.call(m, BANK_ID, &[id]);
        let w = m.r32(table.wrapping_add(kind_idx << 2));
        if e.call(m, BANK_HAS, &[s0, w]) == 0 {
            s0 = 0xffff_ffff;
        } else {
            let h = m.r16(table.wrapping_add(kind_idx << 2));
            m.w16(out + 2, h);
        }
        m.w16(out, s0 as u16);
    }
    (m.r16(out) as i16 as i32) > -1
}

/// 0x2bc938: clear bit `n` of the row bitmap that follows the key words of header `p`.
pub fn clear_row_bit(m: &mut dyn Mem, p: u32, n: i32) {
    let b4 = m.r8(p + 4) as u32;
    let b5 = m.r8(p + 5) as u32;
    let q = if n > -1 { n >> 3 } else { n.wrapping_add(7) >> 3 };
    let bit = n.wrapping_sub(q << 3) as u32;
    let a = p
        .wrapping_add(b5.wrapping_mul((b4 & 0xf) + 2).wrapping_add(0xc))
        .wrapping_add((b4 & 0x70) >> 3)
        .wrapping_add((q + 1) as u32);
    let v = m.r8(a) as u32 & !(1u32.wrapping_shl(bit));
    m.w8(a, v as u8);
}

/// 0x2bc9d8: bit `idx + rows * i16(+2 of q)` of header `p`'s bitmap, 0 when the row index is out of range.
pub fn test_entry(m: &mut dyn Mem, e: &mut dyn LoadEnv, q: u32, p: u32, idx: i32) -> u32 {
    let t0 = m.r16(q + 2) as i16 as i32;
    if !(t0 < m.r16(p + 0xa) as i32) {
        return 0;
    }
    let b4 = m.r8(p + 4) as u32;
    let b5 = m.r8(p + 5) as u32;
    let rows = b5.wrapping_mul((b4 & 0xf) + 2);
    let bits = b5.wrapping_mul(t0 as u32).wrapping_add(idx as u32);
    let base = p.wrapping_add(rows.wrapping_add(0xc)).wrapping_add((b4 & 0x70) >> 3);
    let mut t1 = 0u32;
    if b4 & 0x80 != 0 {
        let a = b5.wrapping_add(7) as i32;
        let v = if -1 < a { a } else { (b5 as i32).wrapping_add(0xe) };
        t1 = (v >> 3).wrapping_add(1) as u32;
    }
    let mut a0 = base.wrapping_add(t1);
    let v0 = m.r8(p + 6) as u32;
    if v0 != 0 {
        a0 = a0.wrapping_add(v0 + 1);
    }
    e.call(m, TEST_BIT, &[a0, bits])
}

/// 0x2bca80: header `p` has the row `idx` (the row count is a byte at +5), per [`test_entry`] when `q` names a
/// variant (+2 not -1), then the second bitmap when bit 7 of +4 is set.
pub fn row_allowed(m: &mut dyn Mem, e: &mut dyn LoadEnv, q: u32, p: u32, idx: i32) -> u32 {
    if !((m.r8(p + 5) as i32) < idx) {
        let mut a1 = 1;
        if m.r16(q + 2) as i16 != -1 {
            a1 = test_entry(m, e, q, p, idx);
            if a1 == 0 {
                return 0;
            }
        }
        let b4 = m.r8(p + 4) as u32;
        if b4 & 0x80 != 0 {
            let a0 = (m.r8(p + 5) as u32).wrapping_mul((b4 & 0xf) + 2).wrapping_add(0xc);
            let a0 = p.wrapping_add(a0).wrapping_add((b4 & 0x70) >> 3).wrapping_add(1);
            a1 = e.call(m, TEST_BIT, &[a0, idx as u32]);
        }
        a1
    } else {
        0
    }
}

/// 0x2bccd0: `x` as a percent of the total at 0x3494f4 (0 without one).
pub fn percent(m: &dyn Mem, x: i32) -> i32 {
    let t = m.r32(TOTAL) as i32;
    if t == 0 {
        0
    } else {
        x.wrapping_mul(100).wrapping_div(t)
    }
}

/// 0x2bcdb0: the low five bits of `v` times the weight of its top three (words from 0x348930).
pub fn weight(m: &dyn Mem, v: u32) -> u32 {
    let v = v & 0xff;
    (v & 0x1f).wrapping_mul(m.r32(WEIGHTS + ((v >> 5) << 2)))
}

/// 0x2bcf48: header `p` (id +0, a signed byte at +8) is fine against entry `i` of the table at 0x3494b0 (id, count):
/// a different id, or no count, passes; the same id passes only while its count is below the byte.
pub fn entry_ok(m: &dyn Mem, p: u32, i: u32) -> bool {
    let en = ENTRIES.wrapping_add(i << 3);
    if m.r16(p) != m.r16(en) {
        return true;
    }
    let c = m.r8(p + 8) as i8 as i32;
    if c <= 0 {
        return true;
    }
    (m.r32(en + 4) as i32) < c
}

/// 0x2bcf90: the mode (the low two bits of +3 of `p`): 0 passes, 1 only for 2, 2 for all but 2, 3 never.
pub fn mode_ok(m: &dyn Mem, p: u32, v: u32) -> u32 {
    match m.r8(p + 3) & 3 {
        0 => 1,
        1 => (v == 2) as u32,
        2 => (v != 2) as u32,
        _ => 0,
    }
}

/// 0x2bcfd8: the value of the key of kind `kind` in bank `bank`'s first sound header (13 bits), -1 without one.
pub fn find_key(m: &dyn Mem, bank: u32, kind: u32) -> i32 {
    let t = m.r32(TABLE);
    let b = m.r32(t.wrapping_add(bank << 2));
    let b4 = m.r8(b + 4) as u32;
    let b5 = m.r8(b + 5) as u32;
    let n = (b4 & 0x70) >> 4;
    let base = b.wrapping_add(b5.wrapping_mul((b4 & 0xf) + 2)).wrapping_add(0xc);
    for k in 0..n {
        let w = ((m.r8(base + 2 * k) as u32) << 8) | m.r8(base + 2 * k + 1) as u32;
        if w >> 13 == kind {
            return (w & 0x1fff) as i32;
        }
    }
    -1
}

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

// ---- the voice picker (0x2bbdd0..0x2bc5e0)

pub const CLOCK: u32 = 0x2bebb0;
pub const FREE_VOICE: u32 = 0x2bc200;
pub const FIND_EVENT_BANK: u32 = 0x2bb650;
pub const CONTAINS: u32 = 0x2bbd78;
pub const FIND_EVENT: u32 = 0x2bb6b0;
pub const FIND_VICTIM: u32 = 0x2bbdd0;
pub const CLEAR_PAIR: u32 = 0x2bbd68;
pub const PICK_VOICE: u32 = 0x2bc048;
pub const CLAIM: u32 = 0x2bc320;
pub const CALLBACK_CHECK: u32 = 0x2bc490;
pub const GROUP_LIMITS: u32 = 0x2bba50;
pub const CLEAR_QUEUE: u32 = 0x2bdae0;
pub const START_ONE: u32 = 0x2be140;
pub const QUEUED_NOW: u32 = 0x2bdc70;
pub const QUEUE_RUN: u32 = 0x2bdc98;
pub const START_VOICE: u32 = 0x2bc5e0;
pub const TRY_START: u32 = 0x2bc548;

const CLOCK_BASE: u32 = 0x349518;
const BUSY: u32 = 0x348900;
const LIMIT_B: u32 = 0x348e88;

fn rec(i: u32) -> u32 {
    GROUPS + 0x60 * i
}

/// 0x2bbdd0: the voice of group `group` to take over for a new sound (the filter list, when there is one, names
/// the sound ids that may be replaced): a voice that is in use, in the group and idle is looked at; one past its
/// minimum age is released on the spot; otherwise the one whose sound is largest (+4) wins, then the oldest, then the
/// one with the larger +0x68. -1 for none. `sp` is the frame (the sound key goes at its start, the bank slot
/// pointer at +0x10).
pub fn find_victim(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, filter: u32, group: u32) -> i32 {
    let now = e.call(m, CLOCK, &[]).wrapping_add(m.r32(CLOCK_BASE));
    let slot_at = SLOTS.wrapping_add(group << 2);
    let mut best_idx: i32 = -1;
    let mut best_size: u32 = 0;
    let mut best_tie: u32 = 0;
    let mut best_age: u64 = u64::MAX;
    for i in 0..16u32 {
        let r = rec(i);
        if m.r8(r + 0x6a) == 0 || m.r8(r + 0x6b) as u32 != group || m.r16(r + 0x6c) as i16 != 0 {
            continue;
        }
        let s0 = m.r32(r + 0x70);
        let fp = if filter != 0 {
            let id = m.r16(s0) as u32;
            e.call(m, CONTAINS, &[id, filter])
        } else {
            1
        };
        let age = now.wrapping_sub(m.r32(r + 0x64)) as i32 as i64 as u64;
        let min_age = m.r16(s0 + 2) as u64;
        let s4 = min_age != 0 && min_age < age;
        let s3 = m.r32(slot_at);
        let mut s2 = 0u32;
        if s3 == 1 && m.r8(s0 + 0xa) & 1 != 0 {
            let key = m.r32(r + 0x74);
            m.w32(sp, key);
            if e.call(m, FIND_EVENT_BANK, &[sp, sp + 0x10]) != 0 {
                let a1 = m.r32(sp + 0x10);
                let v0 = m.r32(a1);
                s2 = (m.r16(s0 + 4) < m.r16(v0.wrapping_add(0x16))) as u32;
            }
        }
        if s4 {
            e.call(m, FREE_VOICE, &[i]);
            continue;
        }
        if fp == 0 || s2 != 0 {
            continue;
        }
        let v1 = m.r16(s0 + 4) as u32;
        let take = if (best_size as i32) < v1 as i32 {
            true
        } else if v1 == best_size {
            if age < best_age {
                true
            } else if age != best_age {
                false
            } else {
                best_tie < m.r16(r + 0x68) as u32
            }
        } else {
            false
        };
        if take {
            best_idx = i as i32;
            best_size = v1;
            best_age = age;
            best_tie = m.r16(r + 0x68) as u32;
        }
    }
    best_idx
}

/// 0x2bc048: a voice for a new sound of group `group`: the event record in table 0x3494b0 (id 0xffff for none)
/// gives the sound whose header (flag 0x10 of +0xa) may name a filter list of ids; with one, [`find_victim`] tries
/// it first and then without. `sp` is the frame (the list object, count then pointer, at its start).
pub fn pick_voice(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, group: u32) -> i32 {
    e.call(m, CLEAR_PAIR, &[sp]);
    let en = ENTRIES.wrapping_add(group << 3);
    let mut has = false;
    if m.r16(en) != 0xffff {
        let a2 = e.call(m, FIND_EVENT, &[en]);
        if a2 != 0 {
            let t1 = m.r8(a2 + 0xa) as u32;
            if (t1 >> 4) & 1 != 0 {
                let n6 = m.r8(a2 + 6) as u32;
                let n7 = m.r8(a2 + 7) as i8 as i32 as u32;
                let t0 = n6 << 1;
                let a0 = (t0.wrapping_add(3) & !3).wrapping_add(0xc);
                let v0 = n7.wrapping_mul(3).wrapping_add(3) & !3;
                let mut a1 = a2.wrapping_add(a0).wrapping_add(v0);
                if (t1 >> 3) & 1 != 0 {
                    a1 = a1.wrapping_add(t0.wrapping_add(7) & !3);
                }
                let v1 = m.r8(a1) as u32;
                m.w32(sp + 4, a1.wrapping_add(2));
                m.w32(sp, v1);
                has = true;
            }
        }
    }
    let mut v = e.call(m, FIND_VICTIM, &[if has { sp } else { 0 }, group]) as i32;
    if has && v < 0 {
        v = e.call(m, FIND_VICTIM, &[0, group]) as i32;
    }
    v
}

/// 0x2bc320: voice `i` is about to start: the flag word at 0x348ea8 is cleared, then every other idle voice of the
/// same group that started later (or at the same time with a smaller +0x68) is released unless its sound's +0xa has
/// bit 2, and a voice whose start is not earlier and whose sound's byte +9 is 100 raises the flag.
pub fn claim_voice(m: &mut dyn Mem, e: &mut dyn LoadEnv, i: u32) {
    let r0 = rec(i);
    let group = m.r8(r0 + 0x6b);
    let start = m.r32(r0 + 0x64);
    let prio = m.r16(r0 + 0x68) as u32;
    m.w32(GROUPS + 0x60, 0);
    for j in 0..16u32 {
        if j == i {
            continue;
        }
        let r = rec(j);
        if m.r8(r + 0x6a) == 0 || m.r8(r + 0x6b) != group || m.r16(r + 0x6c) as i16 != 0 {
            continue;
        }
        let a0 = m.r32(r + 0x64);
        let v1 = m.r16(r + 0x68) as u32;
        if a0 < start || (a0 == start && v1 < prio) {
            let s = m.r32(r + 0x70);
            if (m.r8(s + 0xa) >> 2) & 1 == 0 {
                e.call(m, FREE_VOICE, &[j]);
            }
            continue;
        }
        let s = m.r32(r + 0x70);
        if m.r8(s + 9) as i8 as i32 == 0x64 && m.r8(r + 0x6b) == group {
            m.w32(GROUPS + 0x60, 1);
        }
    }
}

/// 0x2bc4c0: when the group's limits allow, its kept voice (the second limit) is started and freed, and the queue
/// is run for the group.
pub fn run_group(m: &mut dyn Mem, e: &mut dyn LoadEnv, group: u32) {
    if e.call(m, GROUP_LIMITS, &[group]) == 0 {
        return;
    }
    let s1 = m.r32(LIMIT_B.wrapping_add(group << 2));
    e.call(m, CLEAR_QUEUE, &[]);
    let s0 = e.call(m, START_ONE, &[s1]);
    e.call(m, FREE_VOICE, &[s1]);
    if s0 != 0 {
        e.call(m, QUEUE_RUN, &[group]);
    }
}

/// 0x2bc548: run the queue for `id` once at a time (the word at 0x348900 guards it): a queued event or a started
/// voice lets the queue run.
pub fn try_start(m: &mut dyn Mem, e: &mut dyn LoadEnv, id: u32) -> u32 {
    if m.r32(BUSY) != 0 {
        m.w32(BUSY, 0);
        return 0;
    }
    m.w32(BUSY, 1);
    let mut r = 0;
    if e.call(m, QUEUED_NOW, &[id]) != 0 || e.call(m, START_VOICE, &[id]) != 0 {
        r = e.call(m, QUEUE_RUN, &[id]);
    }
    m.w32(BUSY, 0);
    r
}

/// 0x2bc5d8: [`try_start`] for 0.
pub fn try_start_zero(m: &mut dyn Mem, e: &mut dyn LoadEnv) -> u32 {
    try_start_call(m, e, 0)
}

fn try_start_call(m: &mut dyn Mem, e: &mut dyn LoadEnv, id: u32) -> u32 {
    e.call(m, TRY_START, &[id])
}

/// 0x2bc700: [`start_voice`] for group 0.
pub fn start_voice_zero(m: &mut dyn Mem, e: &mut dyn LoadEnv) -> u32 {
    e.call(m, START_VOICE, &[0])
}

/// 0x2bc5e0: start a voice for group `id` when it has any: pick one (its callback check must pass, else it is
/// freed and another picked), claim it, empty the queue and start it and the voices chained after it. `sp` is the
/// frame (the sound key goes at its start). Returns the last start's answer.
pub fn start_voice(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, id: u32) -> u32 {
    if m.r32(GROUPS.wrapping_add(id << 2)) == 0 {
        return 0;
    }
    let mut s1;
    loop {
        s1 = e.call(m, PICK_VOICE, &[id]) as i32;
        if s1 < 0 {
            return 0;
        }
        let key = m.r32(rec(s1 as u32) + 0x74);
        m.w32(sp, key);
        if e.call(m, CALLBACK_CHECK, &[sp]) == 1 {
            break;
        }
        e.call(m, FREE_VOICE, &[s1 as u32]);
    }
    e.call(m, CLAIM, &[s1 as u32]);
    e.call(m, CLEAR_QUEUE, &[]);
    let mut s0 = s1;
    let mut s3;
    loop {
        s3 = e.call(m, START_ONE, &[s0 as u32]);
        s0 = m.r16(rec(s0 as u32) + 0x6e) as i16 as i32;
        if s0 == -1 || s3 == 0 {
            break;
        }
    }
    e.call(m, FREE_VOICE, &[s1 as u32]);
    s3
}

// ---- the event row builder (0x2bea20..0x2bd370)


/// 0x2bea20: a row description cleared (+0 and +8 zero, +4 -1, bytes +0xc..+0xe and +0xf 0xff).
pub fn clear_row(m: &mut dyn Mem, p: u32) {
    m.w8(p + 0xf, 0xff);
    m.w32(p + 4, 0xffff_ffff);
    m.w32(p, 0);
    m.w32(p + 8, 0);
    m.w8(p + 0xc, 0xff);
    m.w8(p + 0xd, 0xff);
    m.w8(p + 0xe, 0xff);
}

/// 0x2bea48: describes row `k` of header `hdr` in `out`: +8 the byte count n of a row (low nibble of +4), +0xc.. its
/// n bytes (after the two of the weight), +4 the row's weight value (two bytes, big endian, times the scale
/// `(hdr[7] + 1) << 8`) and +0 how much longer the next row that is not an alias (bit 7 of the first byte) starts
/// (the header's halfword at +8 after the last). A row with bit 7 of its first byte set names another row in its
/// second byte. False when `k` is past the row count.
pub fn row_info(m: &mut dyn Mem, hdr: u32, k: i32, out: u32) -> bool {
    clear_row(m, out);
    let count = m.r8(hdr + 5) as i32;
    if !(k < count) {
        return false;
    }
    let n = m.r8(hdr + 4) as u32 & 0xf;
    let t1 = n + 2;
    let base = hdr.wrapping_add(0xc);
    m.w32(out + 8, n);
    let mut row = base.wrapping_add((k as u32).wrapping_mul(t1));
    for j in 0..n {
        let v = m.r8(row.wrapping_add(2 + j));
        m.w8(out + 0xc + j, v);
    }
    let mut s1 = k as u32;
    if m.r8(row) & 0x80 != 0 {
        s1 = m.r8(row + 1) as u32;
        row = base.wrapping_add(s1.wrapping_mul(t1));
    }
    let mut next = row.wrapping_add(t1);
    let scale = ((m.r8(hdr + 7) as u32) + 1) << 8;
    let this = (((m.r8(row) as u32) << 8).wrapping_add(m.r8(row + 1) as u32)).wrapping_mul(scale);
    m.w32(out + 4, this);
    let mut a1 = s1.wrapping_add(1) as i32;
    let end;
    loop {
        if a1 < count {
            let b = m.r8(next) as u32;
            if b & 0x80 == 0 {
                end = ((b << 8).wrapping_add(m.r8(next + 1) as u32)).wrapping_mul(scale);
                break;
            }
            next = next.wrapping_add(t1);
            a1 += 1;
        } else {
            end = (m.r16(hdr + 8) as u32).wrapping_mul(scale);
            break;
        }
    }
    m.w32(out, end.wrapping_sub(this));
    true
}

/// 0x2bcb30: the rows of part `ent` (a record of the event at `ctx`: +0 the bank id, +4 the first of its row indices
/// in the list at ctx+0x62, +5 their count, +6 the one in use) that fit: for each row of the bank's header, the
/// row's description (frame `sp`) agrees with `b` / `t` per [`masks_agree`], the row is allowed per [`row_allowed`];
/// rows not heard lately (history 0) are appended to the list (up to 100), of the heard ones the one heard longest
/// ago is appended only when there is none else. The number appended.
pub fn collect_rows(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, ent: u32, b: u32, t: u32) -> u32 {
    let id = m.r16(ent) as i16 as i32 as u32;
    let hdr = m.r32(m.r32(TABLE).wrapping_add(id << 2));
    let n = m.r8(hdr + 5) as i32;
    m.w32(sp + 0x10, b);
    m.w32(sp + 0x14, t);
    if n != 0 {
        m.w32(sp + 0x18, ctx.wrapping_add(0x62));
    }
    let (mut best_w, mut best, mut count) = (0i32, -1i32, 0u32);
    for k in 0..n {
        if !row_info(m, hdr, k, sp) {
            continue;
        }
        let (b2, t2) = (m.r32(sp + 0x10), m.r32(sp + 0x14));
        if !masks_agree(m, sp, b2, t2) {
            continue;
        }
        if row_allowed(m, e, ent, hdr, k) == 0 {
            continue;
        }
        let w = crate::speechlib::history_find(m, hdr, k as u64);
        if w > 0 {
            if best_w < w {
                best_w = w;
                best = k;
            }
        } else {
            let cnt = m.r8(ctx + 0x61) as u32;
            if cnt < 0x64 {
                let p = m.r32(sp + 0x18);
                m.w8(p.wrapping_add(cnt), k as u8);
                m.w8(ctx + 0x61, (cnt + 1) as u8);
                count += 1;
            }
        }
    }
    if count == 0 && best != -1 {
        let cnt = m.r8(ctx + 0x61) as u32;
        count = 0;
        if cnt < 0x64 {
            m.w8(ctx.wrapping_add(cnt).wrapping_add(0x62), best as u8);
            m.w8(ctx + 0x61, (cnt + 1) as u8);
            count = 1;
        }
    }
    count
}

/// 0x2bcd08: the event's rows in use (the part count is the top six bits of hdr[3]) weighted: the sum of their
/// weights as a percent ([`percent`]).
pub fn weight_sum(m: &mut dyn Mem, sp: u32, hdr: u32, ctx: u32) -> i32 {
    let n = m.r8(hdr + 3) as u32 >> 2;
    let mut sum = 0u32;
    for i in 0..n {
        let ent = ctx.wrapping_add(8 * i);
        let id = m.r16(ent) as i16 as i32 as u32;
        let h = m.r32(m.r32(TABLE).wrapping_add(id << 2));
        let idx = m.r8(ctx.wrapping_add(0x62).wrapping_add(m.r8(ent + 6) as u32)) as i32;
        if row_info(m, h, idx, sp) {
            sum = sum.wrapping_add(m.r32(sp));
        }
    }
    percent(m, sum as i32)
}

/// 0x2bcdd8: the order of the rows of a variant header `hdr` (a count at +6, halfword indices from +0xc of
/// entries whose first byte gives a weight): bytes into `out`, drawn by weight without replacement (a number below
/// the remaining sum from `pick_variant` with tag -1), then the rows of weight zero in their own order. The
/// weights are words on the frame `sp`.
pub fn order_rows(m: &mut dyn Mem, sp: u32, hdr: u32, out: u32) {
    let s4 = m.r8(hdr + 6) as u32;
    let mut s5 = 0i32;
    for k in 0..s4 {
        let v = m.r16(hdr + 0xc + 2 * k) as u32;
        let s0 = hdr.wrapping_add(v << 2);
        let w = weight(m, m.r8(s0) as u32);
        m.w32(sp + 4 * k, w);
        s5 = s5.wrapping_add(weight(m, m.r8(s0) as u32) as i32);
    }
    let mut s3 = 0u32;
    while s5 > 0 {
        let v = crate::speechlib::pick_variant(m, s5, 0xffff);
        let mut s1 = 0u32;
        let mut v1 = (v as i32).wrapping_sub(m.r32(sp) as i32);
        if v1 >= 0 {
            loop {
                s1 += 1;
                if !(s1 < s4) {
                    break;
                }
                v1 = v1.wrapping_sub(m.r32(sp + 4 * s1) as i32);
                if v1 < 0 {
                    break;
                }
            }
        }
        m.w8(out.wrapping_add(s3), s1 as u8);
        s3 += 1;
        let w = m.r32(sp + 4 * s1);
        s5 = s5.wrapping_sub(w as i32);
        m.w32(sp + 4 * s1, 0);
    }
    for s1 in 0..s4 {
        let v = m.r16(hdr + 0xc + 2 * s1) as u32;
        let s0 = hdr.wrapping_add(v << 2);
        if weight(m, m.r8(s0) as u32) == 0 {
            m.w8(out.wrapping_add(s3), s1 as u8);
            s3 += 1;
        }
    }
}

/// 0x2bd078: builds the event record `ctx` for header `hdr` (parts: the top six bits of hdr[3], at most twelve):
/// needs [`mode_ok`] for `mode`, [`crate::speechlib::mask_match`] for `a3` and `t0`, and for each part (its index a
/// byte at hdr+4+i, the part a header at four times that) the ids of [`lookup_ids`] with `b` and a non-empty
/// [`collect_rows`]. `sp` is this function's frame.
pub fn build_event(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, hdr: u32, b: u32, a3: u32, t0: u32, mode: u32) -> u32 {
    let n = m.r8(hdr + 3) as u32 >> 2;
    if !(n < 0xd) || mode_ok(m, hdr, mode) == 0 || !crate::speechlib::mask_match(m, a3 as u8, t0 as u8, hdr) {
        return 0;
    }
    m.w8(ctx + 0x60, n as u8);
    for i in 0..n {
        let ent = ctx.wrapping_add(8 * i);
        let c = m.r8(ctx + 0x61);
        m.w8(ent + 4, c);
        let part = hdr.wrapping_add((m.r8(hdr + 4 + i) as u32) << 2);
        if !lookup_ids(m, e, part, b, ent) {
            return 0;
        }
        let v = collect_rows(m, e, sp.wrapping_sub(0xc0), ctx, ent, part, b);
        if v == 0 {
            return 0;
        }
        m.w8(ent + 5, v as u8);
    }
    1
}

/// 0x2bd218: every part's list of row indices shuffled: `count / 2` swaps of the next index with the one
/// `pick_variant` names (tag the part's bank id).
pub fn shuffle_rows(m: &mut dyn Mem, hdr: u32, ctx: u32) {
    let n = m.r8(hdr + 3) as u32 >> 2;
    for i in 0..n {
        let ent = ctx.wrapping_add(8 * i);
        let cnt = m.r8(ent + 5) as u32;
        let base = ctx.wrapping_add(0x62).wrapping_add(m.r8(ent + 4) as u32);
        for k in 0..cnt >> 1 {
            let tag = m.r16(ent);
            let v = crate::speechlib::pick_variant(m, cnt as i32, tag);
            let a = m.r8(base.wrapping_add(k));
            let b = m.r8(base.wrapping_add(v));
            m.w8(base.wrapping_add(k), b);
            m.w8(base.wrapping_add(v), a);
        }
    }
}

/// 0x2bd2e8: the next combination of the parts' current rows (an odometer: each part's +6 counts from +4 up to +4
/// plus +5 and carries into the part before it); true once it has gone round.
pub fn next_combo(m: &mut dyn Mem, hdr: u32, ctx: u32) -> u32 {
    let n = (m.r8(hdr + 3) >> 2) as i32;
    let mut i = n - 1;
    let mut ent = ctx.wrapping_add((i as u32) << 3);
    let mut limit = m.r8(ent + 4) as u32 + m.r8(ent + 5) as u32;
    loop {
        let v = (m.r8(ent + 6) as u32).wrapping_add(1);
        m.w8(ent + 6, v as u8);
        if (v & 0xff) < limit {
            return 0;
        }
        let b4 = m.r8(ent + 4);
        i -= 1;
        m.w8(ent + 6, b4);
        ent = ent.wrapping_sub(8);
        if i < 0 {
            return 1;
        }
        limit = m.r8(ent + 4) as u32 + m.r8(ent + 5) as u32;
    }
}

/// 0x2bd370: looks for a combination of the parts' rows whose [`weight_sum`] is below `thr`: every part's +6 starts
/// at +4, then [`next_combo`] steps until one fits (true) or it has gone round (false). `sp` is this function's frame.
pub fn find_combo(m: &mut dyn Mem, sp: u32, hdr: u32, ctx: u32, thr: i32) -> u32 {
    let n = m.r8(hdr + 3) as u32 >> 2;
    for i in 0..n {
        let ent = ctx.wrapping_add(8 * i);
        let v = m.r8(ent + 4);
        m.w8(ent + 6, v);
    }
    loop {
        if weight_sum(m, sp.wrapping_sub(0x70), hdr, ctx) < thr {
            return 1;
        }
        if next_combo(m, hdr, ctx) != 0 {
            return 0;
        }
    }
}

// ---- the starter (0x2bd430..0x2be468)

pub const KIND: u32 = 0x2bb750;
pub const EVENT_WORD: u32 = 0x2bb700;
pub const BANK_SIZE: u32 = 0x2bb7d0;
const CB0: u32 = 0x348de0;
const CB1: u32 = 0x348de4;
const CB2: u32 = 0x348de8;
const CB4: u32 = 0x348df0;
const CLEAR_ON_PLAY: u32 = 0x348dd8;
const FLAG_EA8: u32 = 0x348ea8;
const Q_REC0: u32 = 0x34895c;
const Q_COUNT_A: u32 = 0x348954;
const Q_RECS_A: u32 = 0x348adc;
const KEYS: u32 = 0x348ebc;

fn copy4(m: &mut dyn Mem, from: u32, to: u32) {
    for k in 0..4 {
        let v = m.r8(from.wrapping_add(k));
        m.w8(to.wrapping_add(k), v);
    }
}

fn bit_of(b: u8) -> u32 {
    if b == 0xff {
        0
    } else {
        1u32.wrapping_shl(b as u32)
    }
}

/// 0x2bd430: picks the row a part `ent` of the event at `ctx` uses (`b` the part's record: +4 the parameter count,
/// bytes +8 the parameters; 0xfe at one: a random row, remembered as pending under the bank's key; 0xff: the first
/// of the shuffled rows whose bit is pending; any other parameter is not a choice). With no choice at all a random
/// row. The row in ent[6]; true when one was picked. `sp` is this function's frame (the row description at sp).
pub fn select_row(m: &mut dyn Mem, sp: u32, ctx: u32, b: u32, ent: u32) -> u32 {
    let id = m.r16(ent) as i16 as i32 as u32;
    let bank = m.r32(m.r32(TABLE).wrapping_add(id << 2));
    let count = m.r8(b + 4) as i8 as i32;
    let (mut found, mut flag) = (0u32, false);
    if count > 0 {
        let mut j = 0i32;
        loop {
            let v = m.r8(b.wrapping_add(8).wrapping_add(j as u32));
            if v == 0xfe {
                flag = true;
                let n = m.r8(ent + 5) as i32;
                let r = crate::speechlib::pick_variant(m, n, id as u16);
                let s = (m.r8(ent + 4) as u32).wrapping_add(r);
                m.w8(ent + 6, s as u8);
                let row = m.r8(ctx.wrapping_add(0x62).wrapping_add(s & 0xff)) as i32;
                let key = find_key(m, id, j as u32);
                if row_info(m, bank, row, sp) {
                    let bit = bit_of(m.r8(sp + 0xc + j as u32));
                    crate::speechlib::set_pending(m, key as u32, bit);
                }
                found = 1;
            } else if v == 0xff {
                flag = true;
                let key = find_key(m, id, j as u32);
                let pend = crate::speechlib::pending(m, key as u32);
                let n = m.r8(ent + 5) as i32;
                let n = if n < 0x65 { n } else { 0 };
                let at = ctx.wrapping_add(0x62).wrapping_add(m.r8(ent + 4) as u32);
                crate::speechlib::shuffle(m, at, n);
                for k in 0..n as u32 {
                    let s = (m.r8(ent + 4) as u32).wrapping_add(k) & 0xff;
                    let row = m.r8(ctx.wrapping_add(0x62).wrapping_add(s)) as i32;
                    if row_info(m, bank, row, sp) && bit_of(m.r8(sp + 0xc + j as u32)) & pend != 0 {
                        m.w8(ent + 6, s as u8);
                        found = 1;
                    }
                }
            }
            j += 1;
            if !(j < count) || found != 0 {
                break;
            }
        }
    }
    if found != 0 || flag {
        return found;
    }
    let n = m.r8(ent + 5) as i32;
    let r = crate::speechlib::pick_variant(m, n, id as u16);
    let s = (m.r8(ent + 4) as u32).wrapping_add(r);
    m.w8(ent + 6, s as u8);
    1
}

/// 0x2bd6a8: the rows of event `ctx` (header `hdr`) chosen for `mode`: nothing pending to begin with; mode 1 shuffles
/// the rows and looks for a combination below `thr` ([`find_combo`]); else every part needs rows ([`select_row`]).
pub fn select_rows(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, hdr: u32, mode: u32, thr: u32) -> u32 {
    crate::speechlib::clear_pending(m);
    if mode == 1 {
        shuffle_rows(m, hdr, ctx);
        return find_combo(m, sp.wrapping_sub(0x50), hdr, ctx, thr as i32);
    }
    let _ = e;
    let n = m.r8(hdr + 3) as u32 >> 2;
    for i in 0..n {
        let ent = ctx.wrapping_add(8 * i);
        if m.r8(ent + 5) == 0 {
            return 0;
        }
        let part = hdr.wrapping_add((m.r8(hdr + 4 + i) as u32) << 2);
        if select_row(m, sp.wrapping_sub(0xf0), ctx, part, ent) == 0 {
            return 0;
        }
    }
    1
}

/// 0x2be208: the base the clock counts from (the word at 0x349518).
pub fn set_clock_base(m: &mut dyn Mem, v: u32) {
    m.w32(CLOCK_BASE, v);
}

/// 0x2be218: `val` is the id of a part's record (+2) or one of its four parameters (+8..+0xb) in any part of `hdr`.
pub fn has_value(m: &dyn Mem, hdr: u32, val: u32) -> u32 {
    let n = m.r8(hdr + 3) as u32 >> 2;
    for i in 0..n {
        let p = hdr.wrapping_add((m.r8(hdr + 4 + i) as u32) << 2);
        if m.r8(p + 2) as u32 == val {
            return 1;
        }
        for k in 0..4 {
            if m.r8(p + 8 + k) as u32 == val {
                return 1;
            }
        }
    }
    0
}

fn records(m: &dyn Mem, hdr: u32) -> u32 {
    hdr.wrapping_add((((m.r8(hdr + 6) as u32) << 1).wrapping_add(3)) & 0x3fc).wrapping_add(0xc)
}

/// 0x2be2b0: the first byte of record `i` (three bytes each, after the row list) of an event header, -1 past eight.
pub fn get_record(m: &dyn Mem, hdr: u32, i: u32) -> u32 {
    if i >= 8 {
        return 0xffff_ffff;
    }
    m.r8(records(m, hdr).wrapping_add(3 * i)) as u32
}

/// 0x2be310: for every record of event `hdr` of kind 0 or 3 whose parameter is used by the row `i` names (per
/// [`has_value`]), the callback at 0x348de8 gets (ctx, the record's value, the word `table[parameter]`, the kind
/// word the library stores through [`KIND`]). `sp` is this function's frame.
pub fn record_callback(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, hdr: u32, i: u32, table: u32) {
    if m.r32(CB2) == 0 {
        return;
    }
    e.call(m, KIND, &[ctx, sp + 0x10]);
    let n6 = m.r8(hdr + 6) as u32;
    let s3 = m.r8(hdr + 7) as i8 as i32;
    let row = hdr.wrapping_add((m.r16(hdr.wrapping_add(2 * i).wrapping_add(0xc)) as u32) << 2);
    let s5 = hdr.wrapping_add((n6 << 1).wrapping_add(3) & 0x3fc).wrapping_add(0xc);
    if s3 <= 0 {
        return;
    }
    for s1 in 0..s3 as u32 {
        let rec = s5.wrapping_add(3 * s1);
        let (v, a, kind) = (m.r8(rec) as u32, m.r8(rec + 1) as u32, m.r8(rec + 2) as u32);
        m.w32(sp, v);
        m.w32(sp + 4, a);
        m.w32(sp + 8, kind);
        if (kind == 0 || kind == 3) && has_value(m, row, a) != 0 {
            let cb = m.r32(CB2);
            let w = m.r32(table.wrapping_add(4 * a));
            let k = m.r32(sp + 0x10);
            e.call(m, cb, &[ctx, v, w, k]);
        }
    }
}

/// 0x2be468: the bits of the parameters an event `hdr` takes from `arr` (a word per parameter number 1..0x14, the
/// 0x14th for records with none): for every record (bit 7 - its number) naming the parameter, kind 4 gives a set bit
/// when its `arr` entry is not 0; the others ask the callback at 0x348de4 (none: -1): nonzero > 0 sets the bit,
/// below 0 sets it in `*out` instead. The set bits. `sp` is this function's frame.
pub fn param_masks(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, hdr: u32, arr: u32, out: u32) -> u32 {
    let (mut fp, mut s6) = (0u32, 0u32);
    e.call(m, KIND, &[ctx, sp + 0x10]);
    let s7 = m.r8(hdr + 7) as i8 as i32;
    let base = records(m, hdr);
    let mut s4 = 1u32;
    loop {
        for s3 in 0..s7.max(0) as u32 {
            let rec = base.wrapping_add(3 * s3);
            let (v, a, kind) = (m.r8(rec) as u32, m.r8(rec + 1) as u32, m.r8(rec + 2) as u32);
            m.w32(sp, v);
            m.w32(sp + 4, a);
            m.w32(sp + 8, kind);
            let a2;
            if s4 == 0x14 {
                if a != 0 {
                    continue;
                }
                a2 = 0;
            } else {
                if a != s4 {
                    continue;
                }
                a2 = m.r32(arr.wrapping_add(s4 << 2));
            }
            let s0 = 1u32.wrapping_shl(7u32.wrapping_sub(s3)) & 0xff;
            let mut s2 = 0;
            if kind == 4 {
                if m.r32(arr.wrapping_add(a << 2)) != 0 {
                    s2 = s0;
                }
            } else {
                let cb = m.r32(CB1);
                let r = if cb != 0 {
                    let k = m.r32(sp + 0x10);
                    e.call(m, cb, &[ctx, v, a2, k]) as i32
                } else {
                    -1
                };
                if r < 0 {
                    s6 |= s0;
                } else if r > 0 {
                    s2 = s0;
                }
            }
            fp = (fp | s2) & 0xff;
        }
        s4 += 1;
        if !(s4 < 0x15) {
            break;
        }
    }
    m.w8(out, s6 as u8);
    fp
}

/// 0x2bd798: for every part of event `hdr` (queue entries from `base`) and each of the part's four parameter
/// bytes that is not 0xff: the callback at 0x348de8 gets (ctx, the record `key` holds for the parameter, the bit the
/// row's description names, the library's kind word). `sp` is this function's frame.
pub fn fire_params(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, ctx: u32, key: u32, hdr: u32, base: u32) {
    if m.r32(CB2) == 0 {
        return;
    }
    e.call(m, KIND, &[ctx, sp + 0x10]);
    let n = m.r8(hdr + 3) as u32 >> 2;
    for i in 0..n {
        let ent = Q_REC0.wrapping_add(base.wrapping_add(i) << 3);
        let part = hdr.wrapping_add((m.r8(hdr + 4 + i) as u32) << 2);
        for k in 0..4u32 {
            let b = m.r8(part + 0xc + k);
            if b == 0xff {
                continue;
            }
            let s1 = get_record(m, key, b as u32);
            let id = m.r16(ent) as i16 as i32 as u32;
            let h = m.r32(m.r32(TABLE).wrapping_add(id << 2));
            let row = m.r16(ent + 4) as i32;
            if row_info(m, h, row, sp) {
                let a3 = m.r32(sp + 0x10);
                let cb = m.r32(CB2);
                let mask = 1u32.wrapping_shl(m.r8(sp + 0xc + k) as u32);
                e.call(m, cb, &[ctx, s1, mask, a3]);
            }
        }
    }
}

/// 0x2bd928: for every part of the queued event (entries from `base`): its row's percent weight is summed, the
/// row is described to the callback at 0x348de0 (a block of eight words on the frame `sp`: bank id, the row's
/// threshold plus the entry's halfword times the scale, the weight, the key, the library's two words and the
/// bank's halfword) and put in the bank's history. The summed percent; 0 without the callback.
pub fn play_rows(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, hdr: u32, key: u32, base: u32) -> i32 {
    m.w32(sp + 0x34, 0);
    if m.r32(CB0) == 0 {
        return 0;
    }
    e.call(m, KIND, &[key, sp + 0x30]);
    e.call(m, EVENT_WORD, &[key, sp + 0x34]);
    let n = m.r8(hdr + 3) as u32 >> 2;
    let mut s6 = 0u32;
    let mut ent = Q_REC0.wrapping_add(base << 3);
    for _ in 0..n {
        let s3 = m.r16(ent) as i16 as i32 as u32;
        let h = m.r32(m.r32(TABLE).wrapping_add(s3 << 2));
        let s2 = m.r16(ent + 4) as u32;
        if m.r8(h + 4) & 0x80 != 0 && m.r32(CLEAR_ON_PLAY) != 0 {
            clear_row_bit(m, h, s2 as i32);
        }
        if row_info(m, h, s2 as i32, sp) {
            let scale = ((m.r8(h + 7) as u32) + 1) << 8;
            let v1 = (m.r16(h + 8) as u32).wrapping_mul(scale);
            let mut a1 = m.r32(sp + 4);
            let a2 = m.r16(ent + 2) as i16 as i32;
            if a2 != -1 {
                a1 = (a2 as u32).wrapping_mul(v1).wrapping_add(a1);
            }
            let w = m.r32(sp);
            m.w32(sp + 0x14, a1);
            m.w32(sp + 0x10, s3);
            s6 = s6.wrapping_add(w);
            m.w32(sp + 0x18, w);
            copy4(m, key, sp + 0x1c);
            let a2h = m.r16(h + 2) as u32;
            let (v30, v34) = (m.r32(sp + 0x30), m.r32(sp + 0x34));
            m.w32(sp + 0x28, v30);
            m.w32(sp + 0x20, v34);
            m.w32(sp + 0x24, a2h);
            let cb = m.r32(CB0);
            e.call(m, cb, &[sp + 0x10]);
            crate::speechlib::history_push(m, h, s2 as u8);
        }
        ent = ent.wrapping_add(8);
    }
    percent(m, s6 as i32)
}

/// 0x2bdc98: plays the events queued for `id`: for each queued record, the record callback, the parameter
/// callbacks and the row callback ([`record_callback`], [`fire_params`], [`play_rows`] with the 4-byte key on this
/// frame `sp`); the first one also keeps its key and a count in the group table at 0x3494b0 (a count of one more
/// when the id is the same). The queue is emptied; the number of parts played.
pub fn queue_run(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, id: u32) -> u32 {
    let mut s3 = 0u32;
    if crate::speechlib::has_queued(m, id as i32) {
        let n = m.r32(Q_COUNT_A) as i32;
        for i in 0..n.max(0) as u32 {
            let rec = Q_RECS_A.wrapping_add(0x5c * i);
            copy4(m, rec + 0xc, sp);
            let (a, hdr) = (m.r32(rec), m.r32(rec + 4));
            let flag = m.r8(rec + 8) as u32;
            record_callback(m, e, sp.wrapping_sub(0xc0), sp, a, flag, rec + 0xc);
            fire_params(m, e, sp.wrapping_sub(0xd0), sp, a, hdr, s3);
            play_rows(m, e, sp.wrapping_sub(0xe0), hdr, sp, s3);
            let parts = m.r8(m.r32(rec + 4) + 3) as u32 >> 2;
            if i == 0 {
                let a0 = id << 3;
                let slot = ENTRIES.wrapping_add(a0);
                if m.r16(sp) != m.r16(slot) {
                    copy4(m, sp, slot);
                    m.w32(ENTRIES + 4 + a0, 1);
                } else {
                    let c = m.r32(ENTRIES + 4 + a0).wrapping_add(1);
                    m.w32(ENTRIES + 4 + a0, c);
                }
            }
            s3 = s3.wrapping_add(parts);
        }
    }
    crate::speechlib::clear_queue(m);
    s3
}

/// 0x2bddf0: parameter `i` of the two-bit table `bits`: 1 asks for a non-zero `val`, 2 for one that is not -1.
pub fn param_test(bits: u32, val: u32, i: u32) -> u32 {
    match bits.wrapping_shr(i << 1) & 3 {
        1 => (val != 0) as u32,
        2 => (val != 0xffff_ffff) as u32,
        _ => 0,
    }
}

/// 0x2bde40: starts the event named by the key at `keyp` (a voice's +0x74): its header found, an entry for its
/// bank's slot (from the table at 0x3494f8, the flag at 0x3488a8 asking for one more, up to two) and its masks,
/// then the first row (in the order of [`order_rows`], and the mask word when the header has bit 3 of +0xa) that
/// the event builder and [`select_rows`] accept is queued. -1 when the header has no rows, 1 when queued. `sp` is
/// this function's frame.
pub fn start_rec(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, keyp: u32) -> u32 {
    m.w8(sp + 0x150, 0);
    copy4(m, keyp, sp + 0x140);
    m.w32(sp + 0x158, 0);
    let s2 = e.call(m, FIND_EVENT, &[sp + 0x140]);
    m.w32(sp + 0x160, 0);
    m.w32(sp + 0x168, 0);
    if s2 == 0 {
        return 0;
    }
    if e.call(m, EVENT_WORD, &[sp + 0x140, sp + 0x154]) == 0 {
        return 0;
    }
    let w2 = m.r32(sp + 0x154);
    if !entry_ok(m, s2, w2) {
        return 0;
    }
    let mut s5 = m.r32(SLOTS.wrapping_add(m.r32(sp + 0x154) << 2)) as i32;
    if m.r32(FLAG_EA8) == 1 {
        s5 = s5.wrapping_add(1);
        if !(s5 < 3) {
            s5 = 2;
        }
    }
    if m.r8(s2 + 0xa) & 1 == 0 && s5 == 1 {
        s5 = 0;
    }
    if s5 != 0 {
        let v = e.call(m, BANK_SIZE, &[sp + 0x140]);
        m.w32(sp + 0x168, v);
    }
    let masks = param_masks(m, e, sp.wrapping_sub(0x120), sp + 0x140, s2, keyp, sp + 0x150);
    m.w32(sp + 0x15c, masks);
    order_rows(m, sp.wrapping_sub(0x220), s2, sp);
    let s6 = m.r8(s2 + 6) as i32;
    if s6 <= 0 {
        return 0xffff_ffff;
    }
    m.w32(sp + 0x16c, 1);
    let f3 = (m.r8(s2 + 0xa) as u32 >> 3) & 1;
    m.w32(sp + 0x164, f3);
    let base2 = records(m, s2).wrapping_add(((m.r8(s2 + 7) as i8 as i32 as u32).wrapping_mul(3).wrapping_add(3)) & !3);
    if f3 != 0 {
        let s7 = m.r32(base2);
        let (mut bit, mut mask) = (1u32, 0u32);
        for s1 in 0..0x14u32 {
            let a1 = m.r32(keyp.wrapping_add(4).wrapping_add(4 * s1));
            if param_test(s7, a1, s1) != 0 {
                mask |= bit;
            }
            bit = (bit << 1) & 0xfffe;
        }
        m.w32(sp + 0x160, mask);
    }
    let mut s4 = 0u32;
    loop {
        let j = m.r32(sp + 0x158);
        let s1 = m.r8(sp.wrapping_add(j)) as u32;
        if !((s1 as i32) < s6) {
            return s4;
        }
        let a2 = s1 << 1;
        let rowh = s2.wrapping_add((m.r16(s2.wrapping_add(0xc).wrapping_add(a2)) as u32) << 2);
        let mut a3 = true;
        if m.r32(sp + 0x164) != 0 {
            let v = (m.r16(base2.wrapping_add(a2).wrapping_add(4)) as u32) ^ m.r32(sp + 0x160);
            if v != 0 {
                a3 = false;
            }
        }
        if a3 {
            let ctx = sp + 0x70;
            crate::speechlib::clear_flags(m, ctx);
            let t0 = m.r8(sp + 0x150) as u32;
            let masks = m.r32(sp + 0x15c);
            let v = build_event(m, e, sp.wrapping_sub(0xa0), ctx, rowh, keyp, masks, t0, s5 as u32);
            if (v as i32) > 0 {
                let thr = m.r32(sp + 0x168);
                s4 = select_rows(m, e, sp.wrapping_sub(0x80), ctx, rowh, s5 as u32, thr);
                if s4 != 0 {
                    let id = m.r32(sp + 0x154) as i32;
                    crate::speechlib::queue_event(m, ctx, id, s2, rowh, s1 as u8, keyp);
                }
            }
        }
        m.w32(sp + 0x158, j.wrapping_add(1));
        if s4 != 0 || !((j.wrapping_add(1) as i32) < s6) {
            return s4;
        }
    }
}

/// 0x2be144 (entered at 0x2be140): starts voice `i`'s event (key at +0x74 of its record); when that gives nothing,
/// the callback at 0x348df0 is asked for each number 0, 1, ... (while it answers above 0) and a non-negative answer
/// tries again. The count of the start, 0 when it is -1. `sp` is this function's frame.
pub fn start_entry(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, i: u32) -> u32 {
    let s3 = KEYS.wrapping_add(0x60u32.wrapping_mul(i));
    let mut s1 = start_rec(m, e, sp.wrapping_sub(0x210), s3);
    if s1 == 0 && m.r32(CB4) != 0 {
        let mut s2 = 0u32;
        loop {
            let cb = m.r32(CB4);
            let s0 = e.call(m, cb, &[s2, s3]) as i32;
            if s0 >= 0 {
                s1 = start_rec(m, e, sp.wrapping_sub(0x210), s3);
            }
            s2 += 1;
            if s1 != 0 || !(s0 > 0) {
                break;
            }
        }
    }
    if (s1 as i32) > -1 {
        s1
    } else {
        0
    }
}

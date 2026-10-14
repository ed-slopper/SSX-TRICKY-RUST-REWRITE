//! The speech library's bank and event tables (board row G10k): the part of the effect player at 0x2bb018..0x2bba50
//! that keeps which sound banks are registered and which event banks are open, and picks a free voice. It is all
//! words in the game's memory (the globals at 0x348dc8..0x349530), so these functions work on a [`Mem`] at the
//! game's own addresses; the three calls it makes out (the clock, a voice's release and the stop-all) are
//! [`Calls`].
//!
//! * 0x348dcc / 0x348dd0: the bank table (a pointer to `count` words, each the address of a bank or 0).
//! * 0x348e08: eight event-bank slots of two words (the bank, a second word read by [`event_word`]).
//! * 0x348e48 / 0x348e68 / 0x348e88: eight words each, per voice group: the count in use, and two limits.
//! * 0x348eac: sixteen voice records of 0x60 bytes (+0 start time, +4 u16, +6 in-use flag, +7 group, +8 u16 state,
//!   +0xc the playing sound's header).
//! * 0x3494f0: the number the bank's history bitmap is stirred with.

pub trait Mem {
    fn r8(&self, a: u32) -> u8;
    fn r16(&self, a: u32) -> u16;
    fn r32(&self, a: u32) -> u32;
    fn w8(&mut self, a: u32, v: u8);
    fn w16(&mut self, a: u32, v: u16);
    fn w32(&mut self, a: u32, v: u32);
}

/// What the library calls out for.
pub trait Calls {
    /// 0x2bebb0: the clock voices are timed with.
    fn now(&mut self) -> i32;
    /// 0x2bc200: release voice `slot`.
    fn release(&mut self, slot: i32);
    /// 0x2baed0: stop every voice (`SpeechLib_Shutdown`).
    fn stop_all(&mut self);
}

const MAGIC: u32 = 0x348dc8;
const TABLE: u32 = 0x348dcc;
const COUNT: u32 = 0x348dd0;
const EVENT_BANKS: u32 = 0x348e08;
const GROUPS: u32 = 0x348e48;
const LIMIT_A: u32 = 0x348e68;
const LIMIT_B: u32 = 0x348e88;
const VOICES: u32 = 0x348eac;
const STIR: u32 = 0x3494f0;

fn g(a: u32) -> u32 {
    a
}

/// 0x2bb098: `count` words at `table` are the bank table, cleared.
pub fn set_bank_table(m: &mut dyn Mem, count: i32, table: u32) {
    m.w32(TABLE, table);
    m.w32(COUNT, count as u32);
    if table != 0 {
        let mut n = count;
        let mut p = table;
        while 0 < n {
            m.w32(p, 0);
            n -= 1;
            p = p.wrapping_add(4);
        }
    }
}

/// 0x2bb060 and 0x2bb078: no table.
pub fn clear_bank_table(m: &mut dyn Mem) {
    m.w32(TABLE, 0);
    m.w32(COUNT, 0);
}

/// 0x2bb110: the first empty slot of the table, or -1.
pub fn find_free_bank_slot(m: &dyn Mem) -> i32 {
    let count = m.r32(COUNT) as i32;
    if count <= 0 {
        return -1;
    }
    let table = m.r32(TABLE);
    let mut i = 0;
    while m.r32(table.wrapping_add(4 * i as u32)) != 0 {
        i += 1;
        if count <= i {
            return -1;
        }
    }
    i
}

/// 0x2bb180: the slot whose bank starts with the id `id`, or -1.
pub fn find_bank_by_id(m: &dyn Mem, id: i16) -> i32 {
    let table = m.r32(TABLE);
    let count = m.r32(COUNT) as i32;
    if table == 0 {
        return -1;
    }
    let mut i = 0i32;
    while i < count {
        let b = m.r32(table.wrapping_add(4 * i as u32));
        if b != 0 && m.r16(b) as i16 == id {
            return i;
        }
        i = (((i + 1) << 16) >> 16) as i32;
    }
    -1
}

/// 0x2bb1f0: the slot whose bank starts with the two ids, or -1.
pub fn find_bank_by_ids(m: &dyn Mem, id: i16, id2: i16) -> i32 {
    let table = m.r32(TABLE);
    let count = m.r32(COUNT) as i32;
    if table == 0 {
        return -1;
    }
    let mut i = 0i32;
    while i < count {
        let b = m.r32(table.wrapping_add(4 * i as u32));
        if b != 0 && m.r16(b) as i16 == id && m.r16(b + 2) as i16 == id2 {
            return i;
        }
        i = (((i + 1) << 16) >> 16) as i32;
    }
    -1
}

/// 0x2bb270: slot `slot` is in the table and `n` is below its bank's sound count (+10; 0xffff: none).
pub fn bank_has_sound(m: &dyn Mem, slot: i64, n: u64) -> bool {
    let table = m.r32(TABLE);
    if table == 0 || slot < 0 {
        return false;
    }
    let b = m.r32(table.wrapping_add((slot as i32 as u32).wrapping_mul(4)));
    let c = m.r16(b.wrapping_add(10)) as u64;
    if c == 0xffff {
        return false;
    }
    n < c
}

/// 0x2bb2b0: empty slot `slot`.
pub fn clear_bank_slot(m: &mut dyn Mem, slot: i32) {
    let table = m.r32(TABLE);
    if table != 0 {
        m.w32((slot as u32).wrapping_mul(4).wrapping_add(table), 0);
    }
}

/// Where a bank keeps its history ring: after the entries (`bank[5]` of them, `(bank[4] & 0xf) + 2` bytes each), the
/// header bytes `(bank[4] & 0x70) >> 3` and, when bank[4] has bit 7, a bitmap of one bit an entry.
fn history(m: &dyn Mem, b: u32) -> u32 {
    let f = m.r8(b + 4) as u32;
    let n = m.r8(b + 5) as u32;
    let bitmap = if f & 0x80 != 0 { ((n as i32 + 7) >> 3) as u32 + 1 } else { 0 };
    b.wrapping_add(n * ((f & 0xf) + 2)).wrapping_add(0xc).wrapping_add((f & 0x70) >> 3).wrapping_add(bitmap)
}

/// 0x2bb2d8: push `v` on the bank's history ring (`bank[6]` long; the write position is its first byte).
pub fn history_push(m: &mut dyn Mem, b: u32, v: u8) {
    let len = m.r8(b + 6);
    if len == 0 {
        return;
    }
    let p = history(m, b);
    let pos = m.r8(p);
    if pos < len {
        m.w8(p + pos as u32 + 1, v);
    }
    let mut next = pos as u32 + 1;
    if len as u32 <= (next & 0xff) {
        next = 0;
    }
    m.w8(p, next as u8);
}

/// 0x2bb370: how far back `v` was pushed (1 the latest), or 0.
pub fn history_find(m: &dyn Mem, b: u32, v: u64) -> i32 {
    let len = m.r8(b + 6) as u32;
    if len == 0 {
        return 0;
    }
    let p = history(m, b);
    let mut back = 0;
    let mut at = m.r8(p) as u32;
    while back < len as i32 {
        at = at.wrapping_sub(1);
        if (at as i32) < 0 {
            at = len - 1;
        }
        if m.r8(p + at + 1) as u64 == v {
            return back + 1;
        }
        back += 1;
    }
    0
}

/// 0x2bb430: sets the bank's bitmap bits for the slice of the entries that the stirring number (0x3494f0) picks.
pub fn stir_bitmap(m: &mut dyn Mem, b: u32) {
    let f = m.r8(b + 4) as u32;
    let n = m.r8(b + 5) as u32;
    let mut p = b.wrapping_add(n * ((f & 0xf) + 2)).wrapping_add(0xc).wrapping_add((f & 0x70) >> 3);
    let k = m.r8(p) as u32;
    if k == 0 {
        return;
    }
    // the divisions are unsigned (`divu`)
    let rem = m.r32(STIR) % k;
    let first = rem.wrapping_mul(n) / k;
    let mut left = (rem.wrapping_mul(n).wrapping_add(n) / k).wrapping_sub(first) as i32;
    let mut bit = first & 7;
    if 0 < left {
        p = p.wrapping_add(first >> 3).wrapping_add(1);
        let mut cur = m.r8(p);
        loop {
            let sh = bit & 0x1f;
            bit += 1;
            m.w8(p, cur | (1u32 << sh) as u8);
            if bit == 8 {
                bit = 0;
                p = p.wrapping_add(1);
            }
            left -= 1;
            if left == 0 {
                break;
            }
            cur = m.r8(p);
        }
    }
}

/// 0x2bb4f0: put the bank in the first free slot (-1 when there is none, no table, or its history is not clean: the
/// write position 0 and every byte 0xff).
pub fn register_bank(m: &mut dyn Mem, b: u32) -> i64 {
    if m.r32(TABLE) == 0 {
        return -1;
    }
    let len = m.r8(b + 6) as u32;
    if len != 0 {
        let p = history(m, b);
        if m.r8(p) != 0 {
            return -1;
        }
        let mut i = 0;
        while i < len {
            if m.r8(p + i + 1) != 0xff {
                return -1;
            }
            i += 1;
        }
    }
    let slot = find_free_bank_slot(m) as i64;
    if -1 < slot {
        if m.r8(b + 4) & 0x80 != 0 {
            stir_bitmap(m, b);
        }
        let table = m.r32(TABLE);
        m.w32((slot as i32 as u32).wrapping_mul(4).wrapping_add(table), b);
    }
    slot
}

/// 0x2bb610: the entry of the event bank `b` with id `id` (the list of u16 offsets, in words, from +0x18, `b[0x10]`
/// of them), or 0.
pub fn bank_event(m: &dyn Mem, b: u32, id: u64) -> u32 {
    let n = m.r16(b + 0x10) as i32;
    let mut i = 0;
    let mut at = b.wrapping_add(0x18);
    while i < n {
        let e = b.wrapping_add(m.r16(at) as u32 * 4);
        i += 1;
        if m.r16(e) as u64 == id {
            return e;
        }
        at = at.wrapping_add(2);
    }
    0
}

/// 0x2bb650: the open event-bank slot (its address) for an event record `ev`: the slot's bank has +9 equal to
/// `ev[2]` and +8 equal to `ev[3]`.
pub fn find_event_bank(m: &dyn Mem, ev: u32) -> Option<u32> {
    for i in 0..8 {
        let slot = EVENT_BANKS + 8 * i;
        let b = m.r32(slot);
        if b != 0 && m.r8(b + 9) == m.r8(ev + 2) && m.r8(b + 8) == m.r8(ev + 3) {
            return Some(slot);
        }
    }
    None
}

/// 0x2bb6b0: the entry of event record `ev` (its id is the u16 at +0), or 0.
pub fn find_event(m: &dyn Mem, ev: u32) -> u32 {
    match find_event_bank(m, ev) {
        Some(slot) => bank_event(m, m.r32(slot), m.r16(ev) as u64),
        None => 0,
    }
}

/// 0x2bb700: the second word of the slot `ev` finds.
pub fn event_word(m: &dyn Mem, ev: u32) -> Option<u32> {
    find_event_bank(m, ev).map(|slot| m.r32(slot + 4))
}

/// 0x2bb750: the bank's byte +9.
pub fn event_bank_kind(m: &dyn Mem, ev: u32) -> Option<u32> {
    find_event_bank(m, ev).map(|slot| m.r8(m.r32(slot) + 9) as u32)
}

/// 0x2bb7d0: the bank's u16 at +0x14, or 0.
pub fn event_bank_size(m: &dyn Mem, ev: u32) -> u16 {
    match find_event_bank(m, ev) {
        Some(slot) => m.r16(m.r32(slot) + 0x14),
        None => 0,
    }
}

/// 0x2bb7a0: close every event bank.
pub fn clear_event_banks(m: &mut dyn Mem) {
    for k in 0..16 {
        m.w32(EVENT_BANKS + 4 * k, 0);
    }
}

/// 0x2bb810: every voice free: the per-group counts 0, the limits -1, the records cleared (group byte 0xff).
pub fn reset_voices(m: &mut dyn Mem) {
    for k in 0..8 {
        m.w32(GROUPS + 4 * k, 0);
        m.w32(LIMIT_A + 4 * k, 0xffff_ffff);
        m.w32(LIMIT_B + 4 * k, 0xffff_ffff);
    }
    m.w32(0x348ea8, 0);
    for i in 0..16u32 {
        let r = 0x348eb2 + 0x60 * i;
        m.w8(r, 0);
        m.w32(VOICES + 0x60 * i, 0);
        m.w32(0x348eb8 + 0x60 * i, 0);
        m.w8(r + 1, 0xff);
        m.w16(0x348eb0 + 0x60 * i, 0);
        for k in 0..20 {
            m.w32(0x348f08 + 0x60 * i - 4 * k, 0);
        }
    }
    m.w32(0x34951c, 0);
    m.w16(0x349520, 0);
}

/// 0x2bb8d8: a voice for a sound of size `size` in group `group` (`limited`: the group's second limit is the slot
/// to leave alone): a free record; else the oldest finished one whose sound's minimum age has passed (released
/// first); else a record of the same group big enough, which is taken over; -1 for none.
pub fn alloc_voice(m: &mut dyn Mem, c: &mut dyn Calls, size: u64, group: u64, limited: i64) -> i32 {
    for i in 0..16 {
        if m.r8(0x348eb2 + 0x60 * i) == 0 {
            return i as i32;
        }
    }
    let mut keep: i32 = -1;
    if limited != 0 {
        keep = m.r32(g(LIMIT_B).wrapping_add((group as i32 as u32).wrapping_mul(4))) as i32;
    }
    let now = c.now();
    let mut i = 0i32;
    loop {
        let r = VOICES + 0x60 * i as u32;
        let sound = m.r32(r + 0xc);
        let busy = m.r16(r + 8) as i16 != 0;
        let skip = busy
            || m.r16(sound.wrapping_add(2)) == 0
            || (now.wrapping_sub(m.r32(r) as i32) as u32) <= m.r16(sound.wrapping_add(2)) as u32
            || keep == i;
        if !skip {
            c.release(i);
            return i;
        }
        i += 1;
        if 0xf < i {
            break;
        }
    }
    let mut i = 0i32;
    loop {
        let r = 0x348e48 + 0x60 * i as u32;
        if m.r16(r + 0x6c) as i16 == 0 {
            let sound = m.r32(r + 0x70);
            let grp = m.r8(r + 0x6b);
            if m.r16(sound.wrapping_add(4)) as u64 <= size && grp as u64 == group && i != keep {
                m.w8(r + 0x6a, 0);
                let at = GROUPS + 4 * grp as u32;
                let v = m.r32(at).wrapping_sub(1);
                m.w32(at, v);
                return i;
            }
        }
        i += 1;
        if 0xf < i {
            return -1;
        }
    }
}

/// 0x2bba50: both limits of group `p` are below 0x10.
pub fn group_limits_ok(m: &dyn Mem, p: i32) -> bool {
    let off = (p as u32).wrapping_mul(4);
    m.r32(LIMIT_B.wrapping_add(off)) < 0x10 && m.r32(LIMIT_A.wrapping_add(off)) < 0x10
}

/// 0x2bb018: when the library is up (0x348dc8 holds 0x1789a34): down, with every voice stopped and no table or
/// event banks.
pub fn shutdown(m: &mut dyn Mem, c: &mut dyn Calls) {
    if m.r32(MAGIC) == 0x178_9a34 {
        m.w32(MAGIC, 0);
        c.stop_all();
        clear_bank_table(m);
        clear_event_banks(m);
    }
}

/// 0x2bb090: `v << 2`.
pub fn shl2(v: i32) -> i32 {
    v << 2
}

// ---- the library's random numbers and the variant history (0x2be650..0x2be908)

const RAND: u32 = 0x349528;
const HIST_POS: u32 = 0x349540;
const HIST: u32 = 0x349548;

/// 0x2be650: `((mask_value[2] ^ a) & mask_value[1] & !b) == 0` on bytes (`p` the record).
pub fn mask_match(m: &dyn Mem, a: u8, b: u8, p: u32) -> bool {
    (m.r8(p + 2) ^ a) & m.r8(p + 1) & !b == 0
}

/// 0x2be678: the generator's step: six words at 0x349528..0x34953c added along with their carries (the last word
/// counts up by one, rolling over into the others); returns the first word.
pub fn rand(m: &mut dyn Mem) -> u32 {
    let w = |m: &dyn Mem, k: u32| m.r32(RAND + 4 * k);
    let (a, b, c, d, e, f) = (w(m, 0), w(m, 1), w(m, 2), w(m, 3), w(m, 4), w(m, 5));
    let u1 = f.wrapping_add(e);
    let carry = (u1 < f || u1 < e) as u32;
    let u2 = u1.wrapping_add(d).wrapping_add(carry);
    let u3 = u2.wrapping_add(c).wrapping_add((u2 < d) as u32);
    let f2 = f.wrapping_add(1);
    let u4 = u3.wrapping_add(b).wrapping_add((u3 < c) as u32);
    let mut a2 = u4.wrapping_add(a).wrapping_add((u4 < b) as u32);
    let (mut b2, mut c2, mut d2, mut e2) = (u4, u3, u2, u1);
    if f2 == 0 {
        e2 = u1.wrapping_add(1);
        if e2 == 0 {
            d2 = u2.wrapping_add(1);
            if d2 == 0 {
                c2 = u3.wrapping_add(1);
                if c2 == 0 {
                    b2 = u4.wrapping_add(1);
                    if b2 == 0 {
                        a2 = a2.wrapping_add(1);
                    }
                }
            }
        }
    }
    for (k, v) in [a2, b2, c2, d2, e2, f2].into_iter().enumerate() {
        m.w32(RAND + 4 * k as u32, v);
    }
    a2
}

/// 0x2be768: seed the generator.
pub fn seed_rand(m: &mut dyn Mem, seed: i32) {
    let k = [-0xdd2f1aai32, -0x77ced917, -0x39db22d1, 0x702c49c, -0x61cac083, 0x6fdf3b64];
    for (i, d) in k.into_iter().enumerate() {
        m.w32(RAND + 4 * i as u32, seed.wrapping_add(d) as u32);
    }
}

/// 0x2be880: remember (id, tag) in the ring of 32.
pub fn push_history(m: &mut dyn Mem, id: u16, tag: u16) {
    let mut pos = (m.r32(HIST_POS) as i32).wrapping_add(1);
    if 0x1f < pos {
        pos = 0;
    }
    m.w32(HIST_POS, pos as u32);
    m.w16(HIST.wrapping_add((pos as u32).wrapping_mul(4)), id);
    m.w16(HIST.wrapping_add((pos as u32).wrapping_mul(4)).wrapping_add(2), tag);
}

/// 0x2be8c8: empty the ring (every id and tag 0xffff) and seed the generator with `seed · 0x10001`.
pub fn init_history(m: &mut dyn Mem, seed: i32) {
    for k in 0..32 {
        m.w16(HIST + 4 * k, 0xffff);
        m.w16(HIST + 4 * k + 2, 0xffff);
    }
    m.w32(HIST_POS, 0);
    seed_rand(m, seed.wrapping_mul(0x10001));
}

/// 0x2be7d8: how many entries back (of the last `span`, at most 32, counting only those with this tag) `id` was
/// picked, or -1.
pub fn find_in_history(m: &dyn Mem, id: i16, span: i32, tag: i16) -> i32 {
    let limit = span.min(0x20);
    let mut result = -1;
    let mut count = 0;
    let mut iter = 0;
    let mut done = false;
    let mut idx = m.r32(HIST_POS) as i32;
    loop {
        let at = HIST.wrapping_add((idx as u32).wrapping_mul(4));
        let mut next = count;
        if count < limit {
            if m.r16(at + 2) as i16 == tag {
                next = count + 1;
                if m.r16(at) as i16 == id {
                    done = true;
                    result = count;
                }
            }
        } else {
            done = true;
        }
        count = next;
        iter += 1;
        if 0x20 < iter {
            done = true;
        }
        let prev = idx - 1;
        idx = if prev < 0 { idx + 0x1f } else { prev };
        if done {
            return result;
        }
    }
}

/// 0x2be908: a variant number below `n`, drawn so that it was not heard lately for this `tag`: up to 32 draws
/// (`(rand >> 16) · n >> 16`), keeping the one that was heard longest ago, stopping at one not in the last `n / 2`
/// (at most 10); the pick is pushed on the ring. Returns the last draw.
pub fn pick_variant(m: &mut dyn Mem, n: i32, tag: u16) -> u32 {
    let span = (n / 2).min(10);
    let mut best: i64 = -1;
    let mut done = false;
    let mut tries = 0;
    let mut keep = ((rand(m) >> 16).wrapping_mul(n as u32)) >> 16;
    loop {
        let draw = ((rand(m) >> 16).wrapping_mul(n as u32)) >> 16;
        let found = find_in_history(m, draw as i16, span, tag as i16) as i64;
        if found == -1 {
            // not heard lately: keep this draw (the best stays as it was)
            done = true;
            keep = draw;
        } else if best < found {
            best = found;
            keep = draw;
        }
        tries += 1;
        if 0x1f < tries {
            done = true;
        }
        if done {
            push_history(m, keep as u16, tag);
            return draw;
        }
    }
}

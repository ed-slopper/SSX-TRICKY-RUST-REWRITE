//! The race's broadphase and its contacts (board row E7d): the sorted-axis sweep the course's objects and the
//! riders are paired by, 0x14f608..0x150a6c. On the bytes at their addresses ([`Mem`]); the allocator, the fill and
//! the frees are the game's (through [`TextBoxEnv`]), and so is the owners' virtual the pairs are sorted by.
//! Checked in the function runner (`tests/broadphase.rs`).
//!
//! The broadphase (gApp's race +0x30): +0 the most boxes (0x100), +4 the free ids (a stack: +0 its size, +4 how many
//! are taken, +8 the ids), +0x10 the pair pool (+0 its size, +4 how many are taken, +8 the 32-byte pairs, +0xc the
//! first free one, a free pair's next at +0x18), +0x20 / +0x24 the oldest and newest live pair (a pair's newer one
//! at +0x18, its older at +0x1c), +0x28 the pair table, +0x2c the three axes' lists of ends (head, tail).
//!
//! The pair table has a word for each two boxes i < j (at i(n − 1) − i(i + 1)/2 + j − 1): its low three bits say on
//! which axes the two overlap, bits 3..15 the pair's place in the pool once they overlap on all three.
//!
//! A box (`BroadphaseProxy`): +0 / +4 the last and first of its pairs, +8 + 0x14·axis its low end and +0x44 + 0x14·axis
//! its high end, +0x80 its id, +0x84 its owner. An end: +0 its box, +4 where its value is, +8 1 for a low end, +0xc
//! the next end on the axis and +0x10 the one before. A pair: +0 / +4 its two boxes, and for each of them (by
//! which side it is) +8 / +0xc the pair before it in that box's list and +0x10 / +0x14 the one after.

use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

/// `EAMem_Alloc` 0x2ccf70(name, size, flags)
pub const ALLOC: u32 = 0x2ccf70;
/// `EAMem_Set` 0x2cc808(at, value, size)
pub const MEMSET: u32 = 0x2cc808;
/// `Mem_Free` 0x14a040(at)
pub const FREE: u32 = 0x14a040;
/// `Mem_Delete` 0x114f80(at)
pub const DELETE: u32 = 0x114f80;
/// The allocations' names: the pairs, the ids, the table
pub const NAME_PAIRS: u32 = 0x36f028;
pub const NAME_IDS: u32 = 0x36f038;
pub const NAME_TABLE: u32 = 0x36f048;
/// What fresh memory is filled with
const FRESH: u32 = 0xdead_c0ed;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}

/// `ContactPool_Init` 0x14f608(pool, n): n pairs of 32 bytes taken and filled, each free one's next the one after
pub fn pool_init(m: &mut Mem, env: &mut dyn TextBoxEnv, pool: u32, n: u32) -> u32 {
    m.w32(pool, n);
    m.w32(pool + 4, 0);
    let size = n << 5;
    let p = call(m, env, ALLOC, &[NAME_PAIRS, size, 0]);
    if p != 0 {
        call(m, env, MEMSET, &[p, FRESH, size]);
    }
    let n = m.r32(pool) as i32;
    m.w32(pool + 0xc, p);
    m.w32(pool + 8, p);
    let mut i = 0i32;
    while i < n - 1 {
        let e = m.r32(pool + 8).wrapping_add((i as u32) << 5);
        m.w32(e + 0x18, e + 0x20);
        i += 1;
    }
    let last = m.r32(pool + 8).wrapping_add((n as u32) << 5);
    m.w32(last.wrapping_sub(8), 0);
    pool
}

/// 0x14f6c8 (the pool's and 0x14f8c8 the ids' destructor: the same code)(it, flags): the array freed, and the
/// thing itself when flags has bit 0
pub fn array_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, it: u32, flags: u32) {
    let a = m.r32(it + 8);
    if a != 0 {
        call(m, env, FREE, &[a]);
    }
    if flags & 1 != 0 {
        call(m, env, DELETE, &[it]);
    }
}

/// 0x14f720: the first free pair taken
pub fn pool_take(m: &mut Mem, pool: u32) -> u32 {
    let e = m.r32(pool + 0xc);
    let used = m.r32(pool + 4);
    m.w32(pool + 0xc, m.r32(e + 0x18));
    m.w32(pool + 4, used.wrapping_add(1));
    e
}

/// 0x14f740: a pair given back (first of the free ones)
pub fn pool_give(m: &mut Mem, pool: u32, e: u32) {
    let used = m.r32(pool + 4);
    m.w32(e + 0x18, m.r32(pool + 0xc));
    m.w32(pool + 0xc, e);
    m.w32(pool + 4, used.wrapping_sub(1));
}

/// `ContactPool_Get` 0x14f760(pool, i): pair i
pub fn pool_at(m: &Mem, pool: u32, i: u32) -> u32 {
    m.r32(pool + 8).wrapping_add(i << 5)
}

/// `ContactPool_IndexOf` 0x14f770(pool, e)
pub fn pool_index(m: &Mem, pool: u32, e: u32) -> u32 {
    (e.wrapping_sub(m.r32(pool + 8)) as i32 >> 5) as u32
}

/// `Contact_ListAppend` 0x14f780(list, end): onto the tail
pub fn list_append(m: &mut Mem, list: u32, e: u32) {
    let tail = m.r32(list + 4);
    if tail == 0 {
        m.w32(list, e);
    } else {
        m.w32(tail + 0xc, e);
    }
    m.w32(e + 0x10, m.r32(list + 4));
    m.w32(e + 0xc, 0);
    m.w32(list + 4, e);
}

/// `Contact_ListRemove` 0x14f7a8(list, end)
pub fn list_remove(m: &mut Mem, list: u32, e: u32) {
    let prev = m.r32(e + 0x10);
    let next = m.r32(e + 0xc);
    if prev != 0 {
        m.w32(prev + 0xc, next);
    } else {
        m.w32(list, next);
    }
    let next = m.r32(e + 0xc);
    let prev = m.r32(e + 0x10);
    if next != 0 {
        m.w32(next + 0x10, prev);
    } else {
        m.w32(list + 4, prev);
    }
}

/// `Broadphase_SwapWithPrev` 0x14f7e0(list, end): the end and the one after it swapped
pub fn swap_next(m: &mut Mem, list: u32, a: u32) {
    let b = m.r32(a + 0xc);
    let after = m.r32(b + 0xc);
    if after == 0 {
        m.w32(list + 4, a);
    } else {
        m.w32(after + 0x10, a);
    }
    let before = m.r32(a + 0x10);
    if before == 0 {
        m.w32(list, b);
    } else {
        m.w32(before + 0xc, b);
    }
    let after = m.r32(b + 0xc);
    let before = m.r32(a + 0x10);
    m.w32(a + 0xc, after);
    m.w32(b + 0x10, before);
    m.w32(a + 0x10, b);
    m.w32(b + 0xc, a);
}

/// `ContactIndex_Init` 0x14f820(ids, n): n words taken and filled, then 0..n
pub fn ids_init(m: &mut Mem, env: &mut dyn TextBoxEnv, ids: u32, n: u32) -> u32 {
    m.w32(ids, n);
    m.w32(ids + 4, 0);
    let size = n << 2;
    let p = call(m, env, ALLOC, &[NAME_IDS, size, 0]);
    if p != 0 {
        call(m, env, MEMSET, &[p, FRESH, size]);
    }
    m.w32(ids + 8, p);
    let mut i = 0i32;
    while i < m.r32(ids) as i32 {
        m.w32(p.wrapping_add((i as u32) << 2), i as u32);
        i += 1;
    }
    ids
}

/// 0x14f920: the next free id taken
pub fn ids_take(m: &mut Mem, ids: u32) -> u32 {
    let used = m.r32(ids + 4);
    let v = m.r32(m.r32(ids + 8).wrapping_add(used << 2));
    m.w32(ids + 4, used.wrapping_add(1));
    v
}

/// 0x14f940: an id given back
pub fn ids_give(m: &mut Mem, ids: u32, id: u32) {
    let used = m.r32(ids + 4).wrapping_sub(1);
    m.w32(ids + 4, used);
    m.w32(m.r32(ids + 8).wrapping_add(used << 2), id);
}

/// `ContactMgr_Init` 0x14f960(bp): 0x100 ids and pairs, the table (0x1fe00 bytes) taken and cleared. (The live
/// list and the axes' lists are left as they are.)
pub fn init(m: &mut Mem, env: &mut dyn TextBoxEnv, bp: u32) -> u32 {
    ids_init(m, env, bp + 4, 0x100);
    pool_init(m, env, bp + 0x10, 0x100);
    m.w32(bp, 0x100);
    let p = call(m, env, ALLOC, &[NAME_TABLE, 0x1fe00, 0]);
    if p != 0 {
        call(m, env, MEMSET, &[p, FRESH, 0x1fe00]);
    }
    m.w32(bp + 0x28, p);
    call(m, env, MEMSET, &[p, 0, 0x1fe00]);
    bp
}

/// 0x14fa00 (the broadphase's destructor)(bp, flags): the table freed, the pool's and the ids' arrays, and the
/// broadphase itself when flags has bit 0
pub fn delete(m: &mut Mem, env: &mut dyn TextBoxEnv, bp: u32, flags: u32) {
    let t = m.r32(bp + 0x28);
    if t != 0 {
        call(m, env, FREE, &[t]);
    }
    array_delete(m, env, bp + 0x10, 2);
    array_delete(m, env, bp + 4, 2);
    if flags & 1 != 0 {
        call(m, env, DELETE, &[bp]);
    }
}

/// `Broadphase_FitBox` 0x14fa70(bp, box): an id given it, its six ends put on the tails of the axes' lists, then
/// moved into place (`move_box`); the id
pub fn fit_box(m: &mut Mem, bp: u32, b: u32) -> u32 {
    let id = ids_take(m, bp + 4);
    m.w32(b + 0x80, id);
    for axis in 0..3 {
        let list = bp + 0x2c + 8 * axis;
        list_append(m, list, b + 8 + 0x14 * axis);
        list_append(m, list, b + 0x44 + 0x14 * axis);
    }
    move_box(m, bp, b);
    id
}

fn other_box(m: &Mem, pair: u32, b: u32) -> u32 {
    if m.r32(pair + 4) == b { m.r32(pair) } else { m.r32(pair + 4) }
}

/// The table's word at k: the pair it names (if they overlap on all three) unlinked from the other box and freed;
/// the overlap bits cleared
fn drop_word(m: &mut Mem, bp: u32, b: u32, k: u32) {
    let at = m.r32(bp + 0x28).wrapping_add(k << 2);
    let w = m.r32(at);
    if w & 7 == 7 {
        let pair = pool_at(m, bp + 0x10, (w >> 3) & 0x1fff);
        let other = other_box(m, pair, b);
        unlink(m, other, pair);
        let w = m.r32(m.r32(bp + 0x28).wrapping_add(k << 2));
        let pair = pool_at(m, bp + 0x10, (w >> 3) & 0x1fff);
        free_pair(m, bp, pair);
    }
    let at = m.r32(bp + 0x28).wrapping_add(k << 2);
    m.w32(at, m.r32(at) & !7);
}

/// `Contact_RemoveAllForNode` 0x14fb38(bp, box): every pair of the box dropped (its table words with every lower
/// id, then every higher), its ends taken off the axes, its id given back
pub fn remove_box(m: &mut Mem, bp: u32, b: u32) {
    let id = m.r32(b + 0x80) as i32;
    let mut k = id - 1;
    let mut step = m.r32(bp) as i32 - 2;
    let mut j = 0;
    while j < id {
        drop_word(m, bp, b, k as u32);
        k += step;
        j += 1;
        step -= 1;
    }
    let mut j = id + 1;
    while j < m.r32(bp) as i32 {
        k += 1;
        drop_word(m, bp, b, k as u32);
        j += 1;
    }
    for axis in 0..3 {
        let list = bp + 0x2c + 8 * axis;
        list_remove(m, list, b + 8 + 0x14 * axis);
        list_remove(m, list, b + 0x44 + 0x14 * axis);
    }
    ids_give(m, bp + 4, id as u32);
}

/// `Contact_FreePair` 0x14fd80(bp, pair): off the live list and back to the pool
pub fn free_pair(m: &mut Mem, bp: u32, pair: u32) {
    let newer = m.r32(pair + 0x18);
    let older = m.r32(pair + 0x1c);
    if newer == 0 {
        m.w32(bp + 0x24, older);
    } else {
        m.w32(newer + 0x1c, older);
    }
    let older = m.r32(pair + 0x1c);
    let newer = m.r32(pair + 0x18);
    if older == 0 {
        m.w32(bp + 0x20, newer);
    } else {
        m.w32(older + 0x18, newer);
    }
    pool_give(m, bp + 0x10, pair);
}

/// `Contact_AllocPair` 0x14fdb8(bp): a pair from the pool put on the live list as the newest
pub fn alloc_pair(m: &mut Mem, bp: u32) -> u32 {
    let e = pool_take(m, bp + 0x10);
    let newest = m.r32(bp + 0x24);
    if newest != 0 {
        m.w32(newest + 0x18, e);
        m.w32(e + 0x18, 0);
        m.w32(e + 0x1c, m.r32(bp + 0x24));
        m.w32(bp + 0x24, e);
    } else {
        m.w32(bp + 0x24, e);
        m.w32(bp + 0x20, e);
        m.w32(e + 0x1c, 0);
        m.w32(e + 0x18, 0);
    }
    e
}

/// `Broadphase_SortAll` 0x14fe18(bp): the three axes sorted
pub fn sort_all(m: &mut Mem, bp: u32) {
    for axis in 0..3 {
        sort_axis(m, bp, axis);
    }
}

/// `Broadphase_MoveBox` 0x14fe68(bp, box): the box's ends moved into place on the three axes
pub fn move_box(m: &mut Mem, bp: u32, b: u32) {
    for axis in 0..3 {
        move_box_on_axis(m, bp, axis, b);
    }
}

fn val(m: &Mem, e: u32) -> F {
    F(f32::from_bits(m.r32(m.r32(e + 4))))
}

/// `e` passed by the end after it: the overlap toggled when one is a low end and the other a high one, then the
/// two swapped
fn pass(m: &mut Mem, bp: u32, axis: u32, e: u32, after: u32) {
    if m.r32(e + 8) != m.r32(after + 8) {
        toggle_overlap(m, bp, axis, e);
    }
    swap_next(m, bp + 0x2c + 8 * axis, e);
}

/// `Broadphase_SortAxis` 0x14fec8(bp, axis): an insertion sort from the tail: each end, from the one before the
/// tail to the head, carried on past every end after it with a smaller value
pub fn sort_axis(m: &mut Mem, bp: u32, axis: u32) {
    let tail = m.r32(bp + 0x30 + 8 * axis);
    if tail == 0 {
        return;
    }
    let mut e = m.r32(tail + 0x10);
    while e != 0 {
        let before = m.r32(e + 0x10);
        loop {
            let next = m.r32(e + 0xc);
            if next == 0 || !(val(m, next) < val(m, e)) {
                break;
            }
            pass(m, bp, axis, e, next);
        }
        e = before;
    }
}

/// `Broadphase_MoveBoxOnAxis` 0x14ffc8(bp, axis, box): the box's high end carried on past smaller values, its low
/// end back past larger ones, then the high end back and the low end on
pub fn move_box_on_axis(m: &mut Mem, bp: u32, axis: u32, b: u32) {
    let hi = b + 0x44 + 0x14 * axis;
    let lo = b + 8 + 0x14 * axis;
    loop {
        let next = m.r32(hi + 0xc);
        if next == 0 || !(val(m, next) < val(m, hi)) {
            break;
        }
        pass(m, bp, axis, hi, next);
    }
    loop {
        let prev = m.r32(lo + 0x10);
        if prev == 0 || !(val(m, lo) < val(m, prev)) {
            break;
        }
        pass(m, bp, axis, prev, lo);
    }
    loop {
        let prev = m.r32(hi + 0x10);
        if prev == 0 || !(val(m, hi) < val(m, prev)) {
            break;
        }
        pass(m, bp, axis, prev, hi);
    }
    loop {
        let next = m.r32(lo + 0xc);
        if next == 0 || !(val(m, next) < val(m, lo)) {
            break;
        }
        pass(m, bp, axis, lo, next);
    }
}

/// `Broadphase_ToggleOverlap` 0x150248(bp, axis, end): the overlap on the axis of the end's box and the box of the
/// end after it toggled in the table. When that ends their overlap on all three their pair is unlinked from both
/// and freed; when it makes it, a pair is made (its place in the table), given the two boxes and linked to both
pub fn toggle_overlap(m: &mut Mem, bp: u32, axis: u32, e: u32) {
    let a = m.r32(e);
    let b = m.r32(m.r32(e + 0xc));
    let at = pair_flags(m, bp, m.r32(a + 0x80) as i32, m.r32(b + 0x80) as i32);
    let w = m.r32(at);
    let nw = (w & !7) | (((1u32 << axis) ^ w) & 7);
    m.w32(at, nw);
    if w & 7 == 7 {
        let pair = pool_at(m, bp + 0x10, (nw >> 3) & 0x1fff);
        unlink(m, m.r32(pair), pair);
        unlink(m, m.r32(pair + 4), pair);
        free_pair(m, bp, pair);
    } else if nw & 7 == 7 {
        let pair = alloc_pair(m, bp);
        let i = pool_index(m, bp + 0x10, pair);
        m.w32(at, (m.r32(at) & 0xffff_0007) | ((i & 0x1fff) << 3));
        m.w32(pair, m.r32(e));
        m.w32(pair + 4, m.r32(m.r32(e + 0xc)));
        link(m, m.r32(e), pair);
        link(m, m.r32(m.r32(e + 0xc)), pair);
    }
}

/// `Broadphase_PairIndex` 0x1503b0(bp, i, j): i(n − 1) − i(i + 1)/2 + j − 1, for i < j
pub fn pair_index(m: &Mem, bp: u32, i: i32, j: i32) -> i32 {
    let n = m.r32(bp) as i32;
    let t = i.wrapping_mul(i).wrapping_add(i) / 2;
    n.wrapping_sub(1).wrapping_mul(i).wrapping_sub(t).wrapping_add(j).wrapping_sub(1)
}

/// `Broadphase_GetPairFlags` 0x1503e0(bp, a, b): the table's word for two ids (either order)
pub fn pair_flags(m: &Mem, bp: u32, a: i32, b: i32) -> u32 {
    let k = if a < b { pair_index(m, bp, a, b) } else { pair_index(m, bp, b, a) };
    m.r32(bp + 0x28).wrapping_add((k as u32) << 2)
}

/// The owner's key a pair is sorted by: the owner's (box +0x84) virtual at its +8 table's 0x30
fn key(m: &mut Mem, env: &mut dyn TextBoxEnv, b: u32) -> u32 {
    let o = m.r32(b + 0x84);
    let vt = m.r32(o + 8);
    let this = o.wrapping_add(m.r16(vt + 0x30) as i16 as u32);
    let func = m.r32(vt + 0x34);
    env.call(m, func, &[this], &[])
}

/// `Contact_SortPairs` 0x150428(bp): every box's own list of pairs sorted (`sort_box_pairs`, from the low ends on
/// axis 0), then the live list: an insertion sort from the one before the newest, each pair carried towards the
/// newest past every pair whose two owners' keys (the smaller high, the larger low, unsigned) are smaller
pub fn sort_pairs(m: &mut Mem, env: &mut dyn TextBoxEnv, bp: u32) {
    let mut e = m.r32(bp + 0x2c);
    while e != 0 {
        if m.r32(e + 8) != 0 {
            sort_box_pairs(m, env, m.r32(e));
        }
        e = m.r32(e + 0xc);
    }
    let newest = m.r32(bp + 0x24);
    if newest == 0 {
        return;
    }
    let mut p = m.r32(newest + 0x1c);
    while p != 0 {
        let older = m.r32(p + 0x1c);
        let mut before = older;
        let mut q = m.r32(p + 0x18);
        let k = pair_key(m, env, p);
        while q != 0 {
            if !(pair_key(m, env, q) < k) {
                break;
            }
            let after = m.r32(q + 0x18);
            if before != 0 {
                m.w32(before + 0x18, q);
            } else {
                m.w32(bp + 0x20, q);
            }
            if after != 0 {
                m.w32(after + 0x1c, p);
            } else {
                m.w32(bp + 0x24, p);
            }
            m.w32(p + 0x1c, q);
            m.w32(p + 0x18, after);
            m.w32(q + 0x1c, before);
            before = q;
            m.w32(q + 0x18, p);
            q = after;
        }
        p = older;
    }
}

fn pair_key(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32) -> u64 {
    let a = key(m, env, m.r32(p));
    let b = key(m, env, m.r32(p + 4));
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    ((lo as u64) << 32).wrapping_add(hi as u64)
}

/// `Obj_HasId` 0x110838(pair, box): whether the box is the pair's second (+4)
pub fn has_id(m: &Mem, pair: u32, b: u32) -> u32 {
    (m.r32(pair + 4) == b) as u32
}

fn side(m: &Mem, pair: u32, b: u32) -> u32 {
    has_id(m, pair, b) << 2
}

/// `Contact_LinkToNode` 0x1506d8(box, pair): the pair put first in the box's list
pub fn link(m: &mut Mem, b: u32, pair: u32) {
    let s = side(m, pair, b);
    m.w32(pair + 0x10 + s, m.r32(b + 4));
    m.w32(pair + 8 + s, 0);
    let first = m.r32(b + 4);
    if first == 0 {
        m.w32(b, pair);
    } else {
        let t = side(m, first, b);
        m.w32(first + 8 + t, pair);
    }
    m.w32(b + 4, pair);
}

/// `Contact_UnlinkFromNode` 0x150740(box, pair): the pair taken out of the box's list
pub fn unlink(m: &mut Mem, b: u32, pair: u32) {
    let s = side(m, pair, b);
    let next = m.r32(pair + 0x10 + s);
    if next != 0 {
        let t = side(m, next, b);
        m.w32(next + 8 + t, m.r32(pair + 8 + s));
    } else {
        m.w32(b, m.r32(pair + 8 + s));
    }
    let prev = m.r32(pair + 8 + s);
    if prev != 0 {
        let t = side(m, prev, b);
        m.w32(prev + 0x10 + t, m.r32(pair + 0x10 + s));
    } else {
        m.w32(b + 4, m.r32(pair + 0x10 + s));
    }
}

/// `BroadphaseProxy_Init` 0x150670(box): no pairs; each axis's ends its own, values unset, the low ends marked
pub fn proxy_init(m: &mut Mem, b: u32) -> u32 {
    m.w32(b + 4, 0);
    m.w32(b, 0);
    for axis in 0..3 {
        let (lo, hi) = (b + 8 + 0x14 * axis, b + 0x44 + 0x14 * axis);
        m.w32(lo + 4, 0);
        m.w32(lo, b);
        m.w32(lo + 8, 1);
        m.w32(hi + 4, 0);
        m.w32(hi, b);
        m.w32(hi + 8, 0);
    }
    b
}

/// `BroadphaseProxy_SetBox` 0x1507e8 (and `_SetBoxB` 0x150818, the same)(box, min, max): the ends' values the
/// corners' x, y, z
pub fn proxy_set_box(m: &mut Mem, b: u32, min: u32, max: u32) {
    m.w32(b + 0x70, max + 8);
    m.w32(b + 0x20, min + 4);
    m.w32(b + 0x5c, max + 4);
    m.w32(b + 0x34, min + 8);
    m.w32(b + 0xc, min);
    m.w32(b + 0x48, max);
}

/// `Contact_SortBodyPairList` 0x150848(box): the box's list of pairs sorted, from its second on: each carried
/// towards the first past every pair whose other box's owner's key is smaller than its own other's
pub fn sort_box_pairs(m: &mut Mem, env: &mut dyn TextBoxEnv, b: u32) {
    let first = m.r32(b + 4);
    if first == 0 {
        return;
    }
    let mut p = m.r32(first + 0x10 + side(m, first, b));
    while p != 0 {
        let mut q = m.r32(p + 8 + side(m, p, b));
        let next = m.r32(p + 0x10 + side(m, p, b));
        let mut after = next;
        let k = key(m, env, other_box(m, p, b));
        while q != 0 {
            if !(key(m, env, other_box(m, q, b)) < k) {
                break;
            }
            let before = m.r32(q + 8 + side(m, q, b));
            if after != 0 {
                let s = side(m, after, b);
                m.w32(after + 8 + s, q);
            } else {
                m.w32(b, q);
            }
            if before != 0 {
                let s = side(m, before, b);
                m.w32(before + 0x10 + s, p);
            } else {
                m.w32(b + 4, p);
            }
            let s = side(m, p, b);
            m.w32(p + 8 + s, before);
            m.w32(p + 0x10 + s, q);
            let s = side(m, q, b);
            m.w32(q + 8 + s, p);
            m.w32(q + 0x10 + s, after);
            after = q;
            q = before;
        }
        p = next;
    }
}

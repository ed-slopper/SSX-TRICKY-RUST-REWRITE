//! The world's node lists and the node base classes (board row E9k), 1:1 on the game's bytes at their addresses
//! ([`Mem`], as for Pathfinder and the replay manager), checked in the runner (`tests/bxlist.rs`).
//!
//! A list set (the world's at 0x337c60: +0 the count, +4 the lists) holds lists of 0x34 bytes: +0 the flags (bit 0
//! sorted), +4 / +0x10 the head and tail sentinel nodes of the live list, +0x1c / +0x28 those of the pending list
//! (sorted lists take new nodes there until `BxList_MergePending`). A node starts with its next and previous links
//! and its vtable (+8); a vtable slot is a this-adjustment (a short) and the function, 8 bytes per slot from +8:
//! vf0 (+0xc) the destructor (flag 3 deletes), vf1 (+0x14) update, vf2 (+0x1c) draw, vf3 (+0x24) "goes before",
//! vf4 (+0x2c) "replaces". The two snapshot counters at 0x337c68 / 0x337c6c number the sorted nodes as they are
//! made (type 5 the first, the others the second).

pub use crate::pf::Mem;

/// The world's list set and the sorted nodes' counters
pub const WORLD: u32 = 0x337c60;
pub const COUNTER5: u32 = 0x337c68;
pub const COUNTER: u32 = 0x337c6c;
/// The base classes' vtables
pub const OBJ_VTABLE: u32 = 0x386520;
pub const TYPE_VTABLE: u32 = 0x3864c0;
pub const SORT_VTABLE: u32 = 0x386460;
/// gApp
pub const GAPP: u32 = 0x337c58;

/// The nodes' virtual calls and the allocator
pub trait NodeEnv {
    /// The function at a vtable slot called on `this` (the node with its adjustment) with `args`
    fn virtual_call(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// `EAMem_Alloc` 0x2ccf70(size), `Mem_Free` 0x14a040, `Mem_Delete` 0x114f80
    fn alloc(&mut self, m: &mut Mem, size: u32) -> u32;
    fn free(&mut self, m: &mut Mem, p: u32);
    fn delete(&mut self, m: &mut Mem, p: u32);
}

fn list(m: &Mem, set: u32, l: u32) -> u32 {
    l.wrapping_mul(0x34).wrapping_add(m.r32(set + 4))
}

/// A node's virtual at a slot (+0xc vf0, +0x14 vf1, ...)
pub fn call_virtual(m: &mut Mem, env: &mut dyn NodeEnv, node: u32, slot: u32, args: &[u32]) -> u32 {
    let vt = m.r32(node + 8);
    let func = m.r32(vt + slot);
    let this = node.wrapping_add(m.r16(vt + slot - 4) as i16 as i32 as u32);
    env.virtual_call(m, func, this, args)
}

fn link_after(m: &mut Mem, node: u32, at: u32) {
    m.w32(node, m.r32(at));
    m.w32(m.r32(at) + 4, node);
    m.w32(node + 4, at);
    m.w32(at, node);
}

/// `FUN_0017d2e0`: a set's constructor (empty)
pub fn set_new(m: &mut Mem, set: u32) {
    m.w32(set + 4, 0);
    m.w32(set, 0);
}

/// `BxList_Init` 0x17d2f0(n): n lists allocated (0x34 each after a 0x10-byte header holding n; filled with
/// 0xdeadc0ed), their sentinels made (`cBxObjNode` vtables) and linked empty
pub fn init(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, n: u32) {
    let size = n.wrapping_mul(0x34).wrapping_add(0x10);
    let p = env.alloc(m, size);
    if p != 0 {
        crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, size);
    }
    let lists = p.wrapping_add(0x10);
    m.w32(p, n);
    for i in 0..n {
        let l = lists + 0x34 * i;
        for k in [1, 2, 4, 5, 7, 8, 10, 11] {
            m.w32(l + 4 * k, 0);
        }
        for k in [3, 6, 9, 12] {
            m.w32(l + 4 * k, OBJ_VTABLE);
        }
    }
    m.w32(set + 4, lists);
    m.w32(set, n);
    let mut i = 0;
    while (i as i32) < n as i32 {
        let l = lists + 0x34 * i;
        m.w32(l, 0);
        m.w32(l + 4, l + 0x10);
        m.w32(l + 0x14, l + 4);
        m.w32(l + 0x1c, l + 0x28);
        m.w32(l + 0x2c, l + 0x1c);
        m.w32(l + 8, 0);
        m.w32(l + 0x10, 0);
        m.w32(l + 0x20, 0);
        m.w32(l + 0x28, 0);
        i += 1;
    }
}

/// `BxList_Free` 0x17d440: every list's nodes deleted, the sentinels destroyed (from the last list back; their
/// destructor unlinks them from the world's set), the lists freed
pub fn free(m: &mut Mem, env: &mut dyn NodeEnv, set: u32) {
    let mut i = 0;
    while (i as i32) < m.r32(set) as i32 {
        delete_all(m, env, set, i);
        i += 1;
    }
    let lists = m.r32(set + 4);
    if lists != 0 {
        let mut l = (m.r32(lists - 0x10).wrapping_mul(0x34)).wrapping_add(lists);
        while lists != l {
            l = l.wrapping_sub(0x34);
            list_destroy(m, env, l, 0);
        }
        env.free(m, m.r32(set + 4) - 0x10);
    }
    m.w32(set, 0);
    m.w32(set + 4, 0);
}

/// `FUN_0017de58`(list, flag): its four sentinels destroyed (`~cBxObjNode` flag 2), deleted with flag bit 0
pub fn list_destroy(m: &mut Mem, env: &mut dyn NodeEnv, l: u32, flag: u32) {
    for off in [0x28, 0x1c, 0x10, 4] {
        node_destroy(m, env, l + off, 2);
    }
    if flag & 1 != 0 {
        env.delete(m, l);
    }
}

/// `BxList_Add` 0x17d4e8(node, list): at the head of the live list (the pending one when sorted)
pub fn add(m: &mut Mem, set: u32, node: u32, l: u32) {
    let at = list(m, set, l);
    if m.r32(at) & 1 == 0 {
        link_after(m, node, at + 4);
    } else {
        link_after(m, node, at + 0x1c);
    }
}

/// `BxList_MergePending` 0x17d548(list): each pending node (first to last) put in the live list: from the tail
/// back, before... after the first node it goes before (vf3), in its place when it replaces it (vf4: that node is
/// destroyed), or at the head when none
pub fn merge_pending(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, l: u32) {
    let mut n = pending_first(m, set, l);
    while n != 0 {
        let nn = pending_next(m, set, n, l);
        let (next, prev) = (m.r32(n), m.r32(n + 4));
        m.w32(prev, next);
        m.w32(next + 4, prev);
        let mut c = sorted_first(m, set, l);
        let mut placed = false;
        while c != 0 {
            if call_virtual(m, env, n, 0x24, &[c]) != 0 {
                if call_virtual(m, env, n, 0x2c, &[]) == 0 {
                    m.w32(n, m.r32(c));
                    m.w32(m.r32(c) + 4, n);
                    m.w32(n + 4, c);
                    m.w32(c, n);
                } else {
                    let vt = m.r32(c + 8);
                    m.w32(n, m.r32(c));
                    m.w32(m.r32(c) + 4, n);
                    m.w32(n + 4, m.r32(c + 4));
                    m.w32(m.r32(c + 4), n);
                    m.w32(c, 0);
                    m.w32(c + 4, 0);
                    let func = m.r32(vt + 0xc);
                    let this = c.wrapping_add(m.r16(vt + 8) as i16 as i32 as u32);
                    env.virtual_call(m, func, this, &[3]);
                }
                placed = true;
                break;
            }
            c = sorted_next(m, set, c, l);
        }
        if !placed {
            let at = list(m, set, l) + 4;
            link_after(m, n, at);
        }
        n = nn;
    }
}

/// `BxList_Unlink` 0x17d6e0: out of its list (when linked both ways)
pub fn unlink(m: &mut Mem, node: u32) {
    let (next, prev) = (m.r32(node), m.r32(node + 4));
    if next != 0 && prev != 0 {
        m.w32(prev, next);
        m.w32(next + 4, prev);
    }
}

/// `BxList_Remove` 0x17d710: unlinked and its links cleared
pub fn remove(m: &mut Mem, node: u32) {
    unlink(m, node);
    m.w32(node + 4, 0);
    m.w32(node, 0);
}

/// `BxList_Move` 0x17d740(node, list)
pub fn move_to(m: &mut Mem, set: u32, node: u32, l: u32) {
    unlink(m, node);
    add(m, set, node, l);
}

/// `BxList_MoveAll` 0x17d788(to, from): every node of one list to the other (merged when it is sorted)
pub fn move_all(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, to: u32, from: u32) {
    let mut n = first(m, set, from);
    while n != 0 {
        let nn = next_or_null(m, set, n, from);
        unlink(m, n);
        add(m, set, n, to);
        n = nn;
    }
    if m.r32(list(m, set, to)) & 1 != 0 {
        merge_pending(m, env, set, to);
    }
}

/// `BxList_First` 0x17d860 / `BxList_NextOrNull` 0x17d8b0: the live list head to tail
pub fn first(m: &Mem, set: u32, l: u32) -> u32 {
    let at = list(m, set, l);
    let n = m.r32(at + 4);
    if n == at + 0x10 { 0 } else { n }
}
pub fn next_or_null(m: &Mem, set: u32, node: u32, l: u32) -> u32 {
    let n = m.r32(node);
    if n == list(m, set, l) + 0x10 { 0 } else { n }
}

/// `BxList_SortedFirst` 0x17d888 / `BxList_SortedNextOrNull` 0x17d8d8: the live list tail to head
pub fn sorted_first(m: &Mem, set: u32, l: u32) -> u32 {
    let at = list(m, set, l);
    let n = m.r32(at + 0x14);
    if n == at + 4 { 0 } else { n }
}
pub fn sorted_next(m: &Mem, set: u32, node: u32, l: u32) -> u32 {
    let n = m.r32(node + 4);
    if n == list(m, set, l) + 4 { 0 } else { n }
}

/// `BxList_PendingFirst` 0x17d900 / `BxList_PendingNextOrNull` 0x17d928
pub fn pending_first(m: &Mem, set: u32, l: u32) -> u32 {
    let at = list(m, set, l);
    let n = m.r32(at + 0x1c);
    if n == at + 0x28 { 0 } else { n }
}
pub fn pending_next(m: &Mem, set: u32, node: u32, l: u32) -> u32 {
    let n = m.r32(node);
    if n == list(m, set, l) + 0x28 { 0 } else { n }
}

/// `BxList_UpdateAll` 0x17d950: each node's vf1 (the next taken first)
pub fn update_all(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, l: u32) {
    let mut n = first(m, set, l);
    while n != 0 {
        let nn = next_or_null(m, set, n, l);
        call_virtual(m, env, n, 0x14, &[]);
        n = nn;
    }
}

/// `BxList_DrawAll` 0x17d9d8: each node's vf2 (the next taken after)
pub fn draw_all(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, l: u32) {
    let mut n = first(m, set, l);
    while n != 0 {
        call_virtual(m, env, n, 0x1c, &[]);
        n = next_or_null(m, set, n, l);
    }
}

/// `BxList_DeleteAll` 0x17da50: each node's vf0 with 3 (the next taken first)
pub fn delete_all(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, l: u32) {
    let mut n = first(m, set, l);
    while n != 0 {
        let nn = next_or_null(m, set, n, l);
        call_virtual(m, env, n, 0xc, &[3]);
        n = nn;
    }
}

/// `BxList_CullToCount` 0x17db00(set, l, n): while the live list holds n nodes or more, the one with the least vf6
/// (+0x3c, unsigned; the last of equal ones) is deleted (vf0 with 3) and the list walked again. Nothing in the game
/// calls it
pub fn cull_to_count(m: &mut Mem, env: &mut dyn NodeEnv, set: u32, l: u32, n: i32) {
    loop {
        let mut count = 0i32;
        let mut least = first(m, set, l);
        let mut node = least;
        while node != 0 {
            count += 1;
            let key = call_virtual(m, env, node, 0x3c, &[]);
            let least_key = call_virtual(m, env, least, 0x3c, &[]);
            if least_key >= key {
                least = node;
            }
            node = next_or_null(m, set, node, l);
        }
        if count < n {
            return;
        }
        if least != 0 {
            call_virtual(m, env, least, 0xc, &[3]);
        }
        if count - 1 < n {
            return;
        }
    }
}

/// `BxList_SetSorted` 0x17dae0
pub fn set_sorted(m: &mut Mem, set: u32, l: u32) {
    let at = list(m, set, l);
    m.w32(at, m.r32(at) | 1);
}

/// `BxList_SaveCounters` 0x17d260 / `BxList_LoadCounters` 0x17d2a0: the two counters into / out of a replay stream
pub fn save_counters(m: &mut Mem, s: u32) {
    crate::replaymgr::stream_write_mem(m, s, COUNTER, 4);
    crate::replaymgr::stream_write_mem(m, s, COUNTER5, 4);
}
pub fn load_counters(m: &mut Mem, s: u32) {
    crate::replaymgr::stream_read_mem(m, s, COUNTER, 4);
    crate::replaymgr::stream_read_mem(m, s, COUNTER5, 4);
}

/// `cBxObjNode::cBxObjNode` 0x17cf50 (and its copy 0x17cf98)(list): the base vtable, added to the world's list
pub fn node_new(m: &mut Mem, node: u32, l: u32) {
    m.w32(node + 8, OBJ_VTABLE);
    add(m, WORLD, node, l);
}

/// `cBxObjNode::~cBxObjNode` 0x17cfe0(flag): the base vtable back, unlinked (from the world's set), deleted with
/// flag bit 0
pub fn node_destroy(m: &mut Mem, env: &mut dyn NodeEnv, node: u32, flag: u32) {
    m.w32(node + 8, OBJ_VTABLE);
    unlink(m, node);
    if flag & 1 != 0 {
        env.delete(m, node);
    }
}

/// `cBxTypeObjNode::cBxTypeObjNode` 0x17d048(list, type) / 0x17d090(list, stream: +0xc..+0x18 read back)
pub fn type_node_new(m: &mut Mem, node: u32, l: u32, ty: u32) {
    node_new(m, node, l);
    m.w32(node + 0x14, ty);
    m.w32(node + 8, TYPE_VTABLE);
}
pub fn type_node_load(m: &mut Mem, node: u32, l: u32, s: u32) {
    node_new(m, node, l);
    m.w32(node + 8, TYPE_VTABLE);
    crate::replaymgr::stream_read_mem(m, s, node + 0xc, 0xc);
}

/// `cBxSortObjNode::cBxSortObjNode` 0x17d0e0(list, type): numbered by its counter (type 5's or the other), the race's
/// +0x18 kept (0 without a game) / 0x17d178(list, stream: +0xc..+0x28 read back)
pub fn sort_node_new(m: &mut Mem, node: u32, l: u32, ty: u32) {
    type_node_new(m, node, l, ty);
    m.w32(node + 8, SORT_VTABLE);
    let c = if ty == 5 { COUNTER5 } else { COUNTER };
    let v = m.r32(c);
    m.w32(c, v.wrapping_add(1));
    m.w32(node + 0x20, v);
    let game = m.r32(m.r32(GAPP) + 0x730);
    let kept = if game == 0 { 0 } else { m.r32(m.r32(game + 0x1c) + 0x18) };
    m.w32(node + 0x24, kept);
}
pub fn sort_node_load(m: &mut Mem, node: u32, l: u32, s: u32) {
    type_node_load(m, node, l, s);
    m.w32(node + 8, SORT_VTABLE);
    crate::replaymgr::stream_read_mem(m, s, node + 0x18, 0x10);
}

/// `FUN_0017d1c8`: the node 0x30 back
pub fn back_0x30(p: u32) -> u32 {
    p.wrapping_sub(0x30)
}

/// `cBxTypeObjNode::SaveSnapshot` 0x17d1d8 / `cBxSortObjNode::SaveSnapshot` 0x17d208: a tagged header (its length
/// and 0xdeadbefc / 0xdeadbefb) over the node's words, written into the stream
pub fn type_node_save(m: &mut Mem, node: u32, s: u32) {
    m.w32(node + 0xc, 0xc);
    m.w32(node + 0x10, 0xdead_befc);
    crate::replaymgr::stream_write_mem(m, s, node + 0xc, 0xc);
}
pub fn sort_node_save(m: &mut Mem, node: u32, s: u32) {
    type_node_save(m, node, s);
    m.w32(node + 0x18, 0x10);
    m.w32(node + 0x1c, 0xdead_befb);
    crate::replaymgr::stream_write_mem(m, s, node + 0x18, 0x10);
}

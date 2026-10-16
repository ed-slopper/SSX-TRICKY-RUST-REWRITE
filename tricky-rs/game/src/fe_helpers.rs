//! The front end's small helpers (board row G9b2a) on the game's bytes: its intrusive lists, the things' tweens, a
//! widget's place, the screen stack, the screen's player bits, the help bar's buttons and the pad's key queries, every
//! call out through [`FeEnv`]; checked in the function runner (`tests/fe_helpers.rs`).
//!
//! A list node has its previous node at +4, its next at +8 and its vtable at +0xc (+0 is the node's id, which the
//! widgets use as their item number). A list is a head node with a tail node at +0x10 (both ids -1) and the list's
//! own vtable at +0x20: the head's next is the first entry and the tail's previous the last; the head points back at
//! itself and the tail on to itself, which is how a walk knows it has reached an end.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::pf::Mem;

/// The node's and the list's vtables
pub const NODE_VTABLE: u32 = 0x3b5b00;
pub const LIST_VTABLE: u32 = 0x3b5ae8;
/// The calls: a player's pad record (0x17a830: the pads (gApp+0x24), the player's pad number); the world list's add
/// (0x2abc68: the list, the widget); a group's event (0x2ac220: the group's +0xa0, the event; 0x269440 the event's
/// destructor (event, 2), its vtable 0x3a9fe0)
pub const PAD_RECORD: u32 = 0x17a830;
pub const WORLD_ADD: u32 = 0x2abc68;
pub const GROUP_EVENT: u32 = 0x2ac220;
pub const EVENT_DTOR: u32 = 0x269440;
pub const EVENT_VTABLE: u32 = 0x3a9fe0;

/// 0x2abf10 (`ListNode_IsSentinel`): 1 for a list's head or tail (a node whose next or previous is itself)
pub fn is_sentinel(m: &Mem, n: u32) -> u32 {
    (m.r32(n.wrapping_add(8)) == n || m.r32(n.wrapping_add(4)) == n) as u32
}

/// 0x2abec0 (`ListNode_Index`): the node's number in its list (-1 for the head), counted by walking back to the head
pub fn index(m: &Mem, n: u32) -> u32 {
    let mut n = n;
    let mut k = u32::MAX;
    while is_sentinel(m, n) == 0 {
        n = m.r32(n + 4);
        k = k.wrapping_add(1);
    }
    k
}

/// 0x2abf38 (`ListNode_Unlink`): the node taken out of its list (its neighbours joined, a neighbour that is the node
/// itself or none left alone) and pointed at itself both ways
pub fn unlink(m: &mut Mem, n: u32) {
    let prev = m.r32(n.wrapping_add(4));
    let next = m.r32(n.wrapping_add(8));
    if prev != 0 && prev != n {
        m.w32(prev.wrapping_add(8), if next != n { next } else { prev });
    }
    if next != 0 && next != n {
        m.w32(next.wrapping_add(4), if prev != n { prev } else { next });
    }
    m.w32(n.wrapping_add(4), n);
    m.w32(n.wrapping_add(8), n);
}

/// 0x2abf80 (`cListNode` vf2; the node, an object whose vtable is at its +0): the object's +0x10 given the node
pub fn node_vf2(m: &mut Mem, e: &mut dyn FeEnv, n: u32, obj: u32) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(0x10)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(0x14));
    e.call(m, f, &[obj.wrapping_add(adj), n])
}

/// 0x2ac160 (`List_Reset`): the head and tail joined (the head's previous itself, the tail's next itself)
pub fn reset(m: &mut Mem, l: u32) {
    let t = l.wrapping_add(0x10);
    m.w32(l.wrapping_add(4), l);
    m.w32(l.wrapping_add(8), t);
    m.w32(t.wrapping_add(4), l);
    m.w32(t.wrapping_add(8), t);
}

/// 0x2abfb0 (`List_Init`): the list's vtable, the head and tail (ids -1, the node vtable) and reset; answers the list
pub fn init(m: &mut Mem, l: u32) -> u32 {
    let t = l.wrapping_add(0x10);
    m.w32(l.wrapping_add(0x20), LIST_VTABLE);
    m.w32(l.wrapping_add(4), l);
    m.w32(l.wrapping_add(8), l);
    m.w32(l.wrapping_add(0xc), NODE_VTABLE);
    m.w32(t, u32::MAX);
    m.w32(l, u32::MAX);
    m.w32(t.wrapping_add(0xc), NODE_VTABLE);
    m.w32(t.wrapping_add(8), t);
    m.w32(t.wrapping_add(4), t);
    reset(m, l);
    l
}

/// 0x2ac018 (`List_First`): the first entry, or 0 for an empty list
pub fn first(m: &Mem, l: u32) -> u32 {
    let n = m.r32(l.wrapping_add(8));
    if m.r32(n.wrapping_add(8)) == n {
        0
    } else {
        n
    }
}

/// 0x2ac030 (`List_Last`): the last entry, or 0 for an empty list
pub fn last(m: &Mem, l: u32) -> u32 {
    let n = m.r32(l.wrapping_add(0x14));
    if m.r32(n.wrapping_add(4)) == n {
        0
    } else {
        n
    }
}

/// 0x2ac048 (`FE3DGroup_GetChild`; the list, k): the k-th entry (k below 1 the first), or 0 past the end
pub fn nth(m: &Mem, l: u32, k: u32) -> u32 {
    let mut n = first(m, l);
    let mut k = k as i32;
    while k > 0 {
        if n == 0 {
            return 0;
        }
        if is_sentinel(m, n) != 0 {
            break;
        }
        n = m.r32(n + 8);
        k -= 1;
    }
    if n == 0 || is_sentinel(m, n) != 0 {
        0
    } else {
        n
    }
}

/// 0x2ac0c0 (`FE3DGroup_FindById`; the list, an id): the first entry with that id (+0), or 0
pub fn find_by_id(m: &Mem, l: u32, id: u32) -> u32 {
    let mut n = first(m, l);
    loop {
        if n == 0 || is_sentinel(m, n) != 0 {
            return 0;
        }
        if m.r32(n) == id {
            return n;
        }
        n = m.r32(n + 8);
    }
}

/// 0x2ac120 (`List_PushFront`; the list, a node): the node put first
pub fn push_front(m: &mut Mem, l: u32, n: u32) {
    let f = m.r32(l.wrapping_add(8));
    m.w32(n.wrapping_add(8), f);
    m.w32(n.wrapping_add(4), m.r32(f.wrapping_add(4)));
    m.w32(f.wrapping_add(4), n);
    m.w32(l.wrapping_add(8), n);
}

/// 0x2ac140 (`List_PushBack`; the list, a node): the node put last
pub fn push_back(m: &mut Mem, l: u32, n: u32) {
    let t = l.wrapping_add(0x10);
    let p = m.r32(t.wrapping_add(4));
    m.w32(n.wrapping_add(4), p);
    m.w32(n.wrapping_add(8), m.r32(p.wrapping_add(8)));
    m.w32(p.wrapping_add(8), n);
    m.w32(t.wrapping_add(4), n);
}

/// 0x2ac178 (`List_DeleteAll`): every entry destroyed (its +8(3) through its vtable at +0xc), then the list reset
pub fn delete_all(m: &mut Mem, e: &mut dyn FeEnv, l: u32) {
    let mut n = m.r32(l.wrapping_add(8));
    let mut next = m.r32(n.wrapping_add(8));
    if next != n {
        loop {
            if n != 0 {
                vcall(m, e, n, 8, &[3]);
            }
            n = next;
            let v = m.r32(n.wrapping_add(8));
            if v == n {
                break;
            }
            next = v;
        }
    }
    reset(m, l);
}

/// 0x2ac1e8 (`List_Count`): the entries before the tail
pub fn count(m: &Mem, l: u32) -> u32 {
    let mut n = m.r32(l.wrapping_add(8));
    let mut next = m.r32(n.wrapping_add(8));
    let mut k = 0u32;
    while next != n {
        n = next;
        k = k.wrapping_add(1);
        next = m.r32(n.wrapping_add(8));
    }
    k
}

/// `cFE3DGroup` vf12 0x2abe60 (`ev` 0x10 bytes for the event): while shown (+0x10), an event (vtable 0x3a9fe0) taken
/// from the group's +0xa0 (0x2ac220), the group's +0x48, the event destroyed (0x269440(event, 2))
pub fn group_vf12(m: &mut Mem, e: &mut dyn FeEnv, g: u32, ev: u32) {
    if m.r32(g + 0x10) == 0 {
        return;
    }
    m.w32(ev, EVENT_VTABLE);
    e.call(m, GROUP_EVENT, &[g + 0xa0, ev]);
    vcall(m, e, g, 0x48, &[]);
    e.call(m, EVENT_DTOR, &[ev, 2]);
}

/// A 16-byte copy as the game's quadword load and store do it: both addresses taken down to 16
fn copy_q(m: &mut Mem, to: u32, from: u32) {
    let (to, from) = (to & !0xf, from & !0xf);
    for k in 0..4 {
        let v = m.r32(from + 4 * k);
        m.w32(to + 4 * k, v);
    }
}

/// A 16-byte copy at any alignment (track 5's)
fn copy_u(m: &mut Mem, to: u32, from: u32) {
    for k in 0..4 {
        let v = m.r32(from.wrapping_add(4 * k));
        m.w32(to.wrapping_add(4 * k), v);
    }
}

/// 0x2ac348 (`FE3DThing_SetTweenStart`; the thing, the track, a vector): for tracks 1..6 the tween's start (+0x40)
/// and +0x1c 1
pub fn tween_start(m: &mut Mem, t: u32, track: u32, v: u32) {
    let k = track as i32;
    if !(1..=6).contains(&k) {
        return;
    }
    if k == 5 {
        copy_u(m, t.wrapping_add(0x40), v);
    } else {
        copy_q(m, t.wrapping_add(0x40), v);
    }
    m.w32(t.wrapping_add(0x1c), 1);
}

/// 0x2ac3b0 (`FE3DThing_StartTween`; the thing, the track, the end vector, the frames, the mode): for tracks 1..6 the
/// tween's end (+0x50), its track (+0x20), frames (+0x28) and mode (+0x2c), the frame (+0x24), +0x18 and +0x30 cleared
pub fn tween(m: &mut Mem, t: u32, track: u32, v: u32, frames: u32, mode: u32) {
    let k = track as i32;
    if !(1..=6).contains(&k) {
        return;
    }
    if k == 5 {
        copy_u(m, t.wrapping_add(0x50), v);
    } else {
        copy_q(m, t.wrapping_add(0x50), v);
    }
    m.w32(t.wrapping_add(0x20), track);
    m.w32(t.wrapping_add(0x28), frames);
    m.w32(t.wrapping_add(0x2c), mode);
    m.w32(t.wrapping_add(0x18), 0);
    m.w32(t.wrapping_add(0x24), 0);
    m.w32(t.wrapping_add(0x30), 0);
}

/// 0x2acb20 (`FE3DWidget_SetWorldPos`; the widget, a position; `buf` 0x10 bytes): the widget's move (+0x70) by the
/// position less its own (+0x80; x, y, z through the VU, w 1)
pub fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, buf: u32) {
    let (p, q) = (at & !0xf, w.wrapping_add(0x80) & !0xf);
    for k in 0..3 {
        let d = f32::from_bits(m.r32(p + 4 * k)) - f32::from_bits(m.r32(q + 4 * k));
        m.w32(buf + 4 * k, d.to_bits());
    }
    m.w32(buf + 0xc, 0x3f80_0000);
    vcall(m, e, w, 0x70, &[buf]);
}

/// 0x275a70 (the stack's list): each screen's +0x88 from the first on (no screens reads the empty list's tail)
pub fn stack_each(m: &mut Mem, e: &mut dyn FeEnv, l: u32) {
    let mut n = first(m, l);
    while is_sentinel(m, n) == 0 {
        vcall(m, e, n, 0x88, &[]);
        n = m.r32(n + 8);
    }
}

/// 0x275ac8 (the front end's stacks): the top screen's +0x90, then the finished screens (+0x24) deleted
pub fn stack_close(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    let top = last(m, fe);
    if top != 0 {
        vcall(m, e, top, 0x90, &[]);
    }
    delete_all(m, e, fe + 0x24);
}

/// 0x275b10 (`FE_PushState`; the stack, a screen): the top screen told of the new one (+0x80), the screen put on top
/// and told of the one below (+0x78)
pub fn push(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, s: u32) {
    let top = last(m, fe);
    if top != 0 {
        vcall(m, e, top, 0x80, &[s]);
    }
    push_back(m, fe, s);
    vcall(m, e, s, 0x78, &[top]);
}

/// 0x275b90 (`FE_PopState`): the top screen taken off and told of the one now on top (+0x80), which is told of it
/// (+0x78); answers the screen taken off
pub fn pop(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) -> u32 {
    let top = last(m, fe);
    unlink(m, top);
    let next = last(m, fe);
    vcall(m, e, top, 0x80, &[next]);
    if next != 0 {
        vcall(m, e, next, 0x78, &[top]);
    }
    top
}

/// 0x275c10 (`FE_FinishState`): the top screen popped, its +0x68, and kept on the finished list (+0x24)
pub fn finish(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    let s = pop(m, e, fe);
    vcall(m, e, s, 0x68, &[]);
    push_back(m, fe + 0x24, s);
}

/// 0x275c60 (`FEStateStack_Clear`): both lists' screens deleted
pub fn stack_clear(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    delete_all(m, e, fe);
    delete_all(m, e, fe + 0x24);
}

/// 0x271868 (`FE3DState_HasFlag`; the screen, a player): the player's bit in +0x2d
pub fn has_player(m: &Mem, s: u32, p: u32) -> u32 {
    let p = p as u8 as i8 as u32;
    (m.r8(s + 0x2d) as u32 & 1u32.wrapping_shl(p) != 0) as u32
}

/// 0x271888 (`FE3DState_AddPlayer`): the player's bit set in +0x2d (a player below 0 sets every bit)
pub fn add_player(m: &mut Mem, s: u32, p: u32) {
    let p = p as u8 as i8;
    if p < 0 {
        m.w8(s + 0x2d, 0xff);
        return;
    }
    let bit = (0x0100_0000u32.wrapping_shl(p as u32) as i32 >> 24) as u8;
    let v = m.r8(s + 0x2d) | bit;
    m.w8(s + 0x2d, v);
}

/// 0x2718c0: the player's bit cleared in +0x2d
pub fn remove_player(m: &mut Mem, s: u32, p: u32) {
    let p = p as u8 as i8 as u32;
    let mask = !1u32.wrapping_shl(p) as u8;
    let v = m.r8(s + 0x2d) & mask;
    m.w8(s + 0x2d, v);
}

/// 0x271848 (`FE3DButtonBar_FindButton`; the screen, an id): the help bar's (+0x48) button with that id
pub fn find_button(m: &Mem, s: u32, id: u32) -> u32 {
    let bar = m.r32(s + 0x48);
    find_by_id(m, bar.wrapping_add(0x118), id)
}

/// 0x2718f0 (`FE3DButtonBar_SetVisible`; the screen, a mask of button ids): each button shown (+0x28(1)) when its id's
/// bits meet the mask, else hidden, its bits taken off the mask as it goes; then the bar laid out (+0x140)
pub fn set_buttons(m: &mut Mem, e: &mut dyn FeEnv, s: u32, mask: u32) {
    let mut mask = mask;
    let bar = m.r32(s + 0x48);
    let mut n = nth(m, bar.wrapping_add(0x118), 0);
    while n != 0 && is_sentinel(m, n) == 0 {
        let on = (m.r32(n) & mask != 0) as u32;
        vcall(m, e, n, 0x28, &[on]);
        let id = m.r32(n);
        n = m.r32(n + 8);
        mask &= !id;
    }
    let bar = m.r32(s + 0x48);
    vcall(m, e, bar, 0x140, &[]);
}

/// The pad's key queries (a key event, its code at +0): accept (0x64 or 0xd, 0x26ce08), back (0x62 or 0x1b,
/// 0x26ce30), and single codes: 0x61 (0x26ce58), 0x63 (0x26ce68), up 0x4800 (0x26ce78), down 0x5000 (0x26ce88), left
/// 0x4b00 (0x26ce98), right 0x4d00 (0x26cea8), 0x72 (0x26ceb8), 0x52 (0x26cec8), 0x6c (0x26ced8), 0x4c (0x26cee8)
pub const KEY_QUERIES: [(u32, [u32; 2]); 12] = [
    (0x26ce08, [0x64, 0xd]),
    (0x26ce30, [0x62, 0x1b]),
    (0x26ce58, [0x61, 0x61]),
    (0x26ce68, [0x63, 0x63]),
    (0x26ce78, [0x4800, 0x4800]),
    (0x26ce88, [0x5000, 0x5000]),
    (0x26ce98, [0x4b00, 0x4b00]),
    (0x26cea8, [0x4d00, 0x4d00]),
    (0x26ceb8, [0x72, 0x72]),
    (0x26cec8, [0x52, 0x52]),
    (0x26ced8, [0x6c, 0x6c]),
    (0x26cee8, [0x4c, 0x4c]),
];

/// A key query (`codes` from [`KEY_QUERIES`]): 1 when the event's code is one of them
pub fn key_is(m: &Mem, ev: u32, codes: [u32; 2]) -> u32 {
    let c = m.r32(ev);
    (c == codes[0] || c == codes[1]) as u32
}

/// 0x26cef8 (`FEPlayer_IsPadConnected`; a player): the player's pad record's +4
pub fn pad_connected(m: &mut Mem, e: &mut dyn FeEnv, p: u32) -> u32 {
    let pads = m.r32(m.r32(GAPP).wrapping_add(0x24));
    let r = e.call(m, PAD_RECORD, &[pads, m.r32(p + 0x40)]);
    m.r32(r.wrapping_add(4))
}

/// 0x26cf28 (a player, a value): the player's pad record's +8 set
pub fn set_pad_word(m: &mut Mem, e: &mut dyn FeEnv, p: u32, v: u32) {
    let pads = m.r32(m.r32(GAPP).wrapping_add(0x24));
    let r = e.call(m, PAD_RECORD, &[pads, m.r32(p + 0x40)]);
    m.w32(r.wrapping_add(8), v);
}

/// 0x2763f0 (`FE3D_AddToWorldList`; the screen, a widget): the widget added to the screen's world list (+0x34,
/// 0x2abc68)
pub fn add_widget(m: &mut Mem, e: &mut dyn FeEnv, s: u32, w: u32) {
    let l = m.r32(s + 0x34);
    e.call(m, WORLD_ADD, &[l, w]);
}

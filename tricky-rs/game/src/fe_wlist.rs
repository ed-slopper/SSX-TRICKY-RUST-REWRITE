//! The front end's menu list (board row G9x3b) on the game's bytes: `cFE3DWList`'s own virtuals over the menu
//! (`cFE3DMenu`, whose code is G9b's and is called out), `FE3DMenu_Scroll` (the list moved to show the chosen item),
//! the frame's centring offset (0x2b5958) and `StaticInit_FE3DWList`. Every call out goes through [`FeEnv`]; the
//! functions of the row that call each other, and the widget's own (`fe_wgroup`, `fe_widgets`), are called directly.
//! Checked in the function runner (`tests/fe_wlist.rs`).
//!
//! The menu: its items in a list at +0x118, +0x13c whether it wraps, +0x140 the chosen item, +0x148 / +0x14c the two
//! arrows, +0x150 the way it runs (0 across, else down), +0x154 the alignment in the box (bit 0 to the right / bottom
//! edge, bit 1 as it is, else centred; bits 2 and 3 for y), +0x15c the gap, +0x170 / +0x174 the box, +0x190 the mode
//! (1 plain, 2 one item at a time, 3 in a frame, 4 a window of items), +0x194 the frame, +0x198 / +0x19c the first
//! and last items shown, +0x1a0 how many are shown.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_wgroup::{list_apply, widget_slide_in_dir, OP_OFFSET};
use crate::fe_widgets::{icon_init, vcall_class, ICON_VTABLE};
use crate::pf::Mem;
use crate::ps2f::F;

/// The list's table and the name its arrows are allocated under
pub const LIST_VTABLE: u32 = 0x3b7fe0;
pub const ALLOC_NAME: u32 = 0x3b7fd8;
/// The menu's code (G9b): the next / previous item that can be chosen (0x26e538 / 0x26e5c8: the menu, the item),
/// two item alignments (0x26dd60, 0x26df58: the menu, the item, the new one), the arrows' update (0x26d8c8), its
/// vf12 (0x26eb98), vf11 (0x26ea98), vf41 (0x26e658), vf42 (0x26e7b8), vf39 (0x26d660), vf36 (0x26ec40), vf37
/// (0x26efd8), vf1 (0x26e9f8), vf5 (0x26e918), destructor (0x26f9d0) and constructor (0x26d3b8)
pub const MENU_NEXT: u32 = 0x26e538;
pub const MENU_PREV: u32 = 0x26e5c8;
pub const MENU_ALIGN: u32 = 0x26dd60;
pub const MENU_ALIGN_LAST: u32 = 0x26df58;
pub const MENU_ARROWS: u32 = 0x26d8c8;
pub const MENU_VF12: u32 = 0x26eb98;
pub const MENU_VF11: u32 = 0x26ea98;
pub const MENU_VF41: u32 = 0x26e658;
pub const MENU_VF42: u32 = 0x26e7b8;
pub const MENU_VF39: u32 = 0x26d660;
pub const MENU_VF36: u32 = 0x26ec40;
pub const MENU_VF37: u32 = 0x26efd8;
pub const MENU_VF1: u32 = 0x26e9f8;
pub const MENU_VF5: u32 = 0x26e918;
pub const MENU_DTOR: u32 = 0x26f9d0;
pub const MENU_CTOR: u32 = 0x26d3b8;
/// The list helpers: count (0x2ac1e8), first (0x2ac018), last (0x2ac030), the n-th (0x2ac048), push to the front
/// (0x2ac120) and back (0x2ac140), a node's unlink (0x2abf38) and index (0x2abec0); the heap (0x2ccf70: the name,
/// the size, 0) and its fill (0x2cc808)
pub const LIST_COUNT: u32 = 0x2ac1e8;
pub const LIST_FIRST: u32 = 0x2ac018;
pub const LIST_LAST: u32 = 0x2ac030;
pub const LIST_NTH: u32 = 0x2ac048;
pub const LIST_PUSH_FRONT: u32 = 0x2ac120;
pub const LIST_PUSH_BACK: u32 = 0x2ac140;
pub const NODE_UNLINK: u32 = 0x2abf38;
pub const NODE_INDEX: u32 = 0x2abec0;
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
/// The colours `StaticInit_FE3DWList` sets up
pub const COLOURS: u32 = 0x3d3760;

const ONE: u32 = 0x3f80_0000;
const HALF: F = F(0.5);

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// `c.olt` as the EE does it: no NaN (an exponent of all ones is a huge value), denormals as 0
fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

/// `c.eq` with 0 as the EE does it
fn zero(a: F) -> bool {
    a.ee() == 0.0
}

fn min(a: F, b: F) -> F {
    if lt(b, a) { b } else { a }
}

fn max(a: F, b: F) -> F {
    if lt(a, b) { b } else { a }
}

/// A quadword's four floats (the address taken down to 16)
fn q(m: &Mem, at: u32) -> [F; 4] {
    let a = at & !0xf;
    [rf(m, a), rf(m, a + 4), rf(m, a + 8), rf(m, a + 12)]
}

fn put(m: &mut Mem, at: u32, v: [F; 4]) {
    let a = at & !0xf;
    for (k, x) in v.iter().enumerate() {
        wf(m, a + 4 * k as u32, *x);
    }
}

fn rcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// The offset that takes `item` from its position (+0x80) to `at`, its w 1, written at `out`
fn offset_to(m: &mut Mem, item: u32, at: [F; 4], out: u32) {
    let p = q(m, item + 0x80);
    put(m, out, [at[0] - p[0], at[1] - p[1], at[2] - p[2], F(1.0)]);
}

/// 0x2b5958(out, l; `buf` 0x20 bytes): the offset that centres the frame (+0x194) on the chosen item: the item's
/// position less the frame's, less half the difference of their sizes
pub fn frame_offset(m: &mut Mem, e: &mut dyn FeEnv, out: u32, l: u32, buf: u32) -> u32 {
    let item = m.r32(l + 0x140);
    let frame = m.r32(l + 0x194);
    vcall(m, e, frame, 0x130, &[buf]);
    vcall(m, e, item, 0x130, &[buf + 0x10]);
    let (a, b) = (q(m, buf), q(m, buf + 0x10));
    let d = [0, 1, 2, 3].map(|k| (a[k] - b[k]) * HALF);
    let frame = m.r32(l + 0x194);
    let (pi, pf) = (q(m, item + 0x80), q(m, frame + 0x80));
    let v = [0, 1, 2, 3].map(|k| (pi[k] - pf[k]) - d[k]);
    put(m, out, v);
    out
}

/// `cFE3DWList` vf44 0x2b51d8 (its arrows made; `buf` 0x40 bytes): one item at a time, no wrap, a gap of 15; two
/// icons (0x120 bytes each, filled with 0xdeadc0de) owned by the list's owner with the list as their parent, each 10
/// by 10, slid in from anchors 3 and 4, shown raised (+0x70 1), kept at +0x148 and +0x14c; +0x154 0
pub fn make_arrows(m: &mut Mem, e: &mut dyn FeEnv, l: u32, buf: u32) {
    m.w32(l + 0x13c, 0);
    m.w32(l + 0x190, 2);
    wf(m, l + 0x15c, F(15.0));
    for (at, dir) in [(0x148u32, 3u32), (0x14c, 4)] {
        let o = e.call(m, ALLOC, &[ALLOC_NAME, 0x120, 0]);
        if o != 0 {
            e.call(m, FILL, &[o, 0xdead_c0de, 0x120]);
        }
        let owner = m.r32(l + 0xec);
        icon_init(m, e, o, owner, l);
        for (k, v) in [F(10.0), F(10.0), F(0.0), F(0.0)].iter().enumerate() {
            wf(m, buf + 4 * k as u32, *v);
        }
        vcall_class(m, e, ICON_VTABLE, o, 0x138, &[buf]);
        widget_slide_in_dir(m, e, o, dir, buf + 0x20);
        if at == 0x148 {
            m.w32(l + at, o);
            m.w32(o + 0x70, 1);
        } else {
            m.w32(o + 0x70, 1);
            m.w32(l + at, o);
        }
    }
    m.w32(l + 0x154, 0);
}

/// `FE3DMenu_Scroll` 0x2b5668(l, old, item, dir; `buf` 0x40 bytes), by the mode: 2 the item moved into the box at
/// the list's position; 3 the frame centred on the chosen item; 4 the window moved on one way or the other (down
/// when rolling forward without wrap or back with it), its ends aligned and the items moved back by the frame's
/// offset; then the arrows' update (for every mode but the others)
pub fn scroll(m: &mut Mem, e: &mut dyn FeEnv, l: u32, item: u32, dir: u32, buf: u32) {
    match m.r32(l + 0x190) {
        2 => {
            let mut at = q(m, l + 0x80);
            vcall(m, e, item, 0x130, &[buf + 0x10]);
            let size = [rf(m, buf + 0x10), rf(m, buf + 0x14)];
            let bw = rf(m, l + 0x170);
            if !zero(bw) {
                let centre = if m.r32(l + 0x150) != 0 {
                    true
                } else {
                    let a = m.r32(l + 0x154);
                    if a & 1 != 0 {
                        at[0] = at[0] + (bw - size[0]);
                        false
                    } else {
                        (a & 3) == 0
                    }
                };
                if centre {
                    at[0] = at[0] + (bw - size[0]) * HALF;
                }
            }
            let bh = rf(m, l + 0x174);
            if !zero(bh) {
                let centre = if m.r32(l + 0x150) != 1 {
                    true
                } else {
                    let a = m.r32(l + 0x154);
                    if (a >> 3) & 1 != 0 {
                        at[1] = at[1] + (bh - size[1]);
                        false
                    } else {
                        (a & 0xc) == 0
                    }
                };
                if centre {
                    at[1] = at[1] + (bh - size[1]) * HALF;
                }
            }
            offset_to(m, item, at, buf + 0x20);
            vcall(m, e, item, 0x70, &[buf + 0x20]);
        }
        1 => {}
        3 => {
            frame_offset(m, e, buf, l, buf + 0x20);
            let frame = m.r32(l + 0x194);
            vcall(m, e, frame, 0x70, &[buf]);
        }
        4 => {
            let wraps = m.r32(l + 0x13c) != 0;
            let forward = (dir == 1 && !wraps) || (dir == u32::MAX && wraps);
            let (first, last);
            if forward {
                first = e.call(m, MENU_NEXT, &[l, m.r32(l + 0x198)]);
                last = e.call(m, MENU_NEXT, &[l, m.r32(l + 0x19c)]);
                let old_last = m.r32(l + 0x19c);
                if last != old_last {
                    e.call(m, MENU_ALIGN_LAST, &[l, old_last, last]);
                }
            } else {
                first = e.call(m, MENU_PREV, &[l, m.r32(l + 0x198)]);
                last = e.call(m, MENU_PREV, &[l, m.r32(l + 0x19c)]);
                let old_first = m.r32(l + 0x198);
                if first != old_first {
                    e.call(m, MENU_ALIGN, &[l, old_first, first]);
                }
            }
            m.w32(l + 0x198, first);
            m.w32(l + 0x19c, last);
            frame_offset(m, e, buf + 0x10, l, buf + 0x30);
            // (`Vec4_Scale` 0x102f50 by -1: every lane negated)
            let v = q(m, buf + 0x10).map(|x| x * F(-1.0));
            put(m, buf, v);
            m.w32(buf + 0x10, OP_OFFSET.0);
            put(m, buf + 0x20, v);
            list_apply(m, e, l + 0x118, buf + 0x10);
            e.call(m, OP_OFFSET.1, &[buf + 0x10, 2]);
        }
        _ => return,
    }
    e.call(m, MENU_ARROWS, &[l]);
}

/// `cFE3DWList` vf12 0x2b5a30: the menu's, then the frame's update (+0x68)
pub fn list_update(m: &mut Mem, e: &mut dyn FeEnv, l: u32) {
    e.call(m, MENU_VF12, &[l]);
    let f = m.r32(l + 0x194);
    if f != 0 {
        vcall(m, e, f, 0x68, &[]);
    }
}

/// `cFE3DWList` vf10 0x2b5a78(l, matrix) (rendered: only while +0x14 is clear): with the renderer's state raised,
/// by the mode: 2 the chosen item, 4 the items shown, then (while chosen) the arrows; 1 nothing; the others put the
/// state back and raise it again; then (while chosen) the frame; the state put back
pub fn list_render(m: &mut Mem, e: &mut dyn FeEnv, l: u32, matrix: u32) {
    if m.r32(l + 0x14) != 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(l + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let arrows = match m.r32(l + 0x190) {
        2 => {
            let c = m.r32(l + 0x140);
            if c != 0 {
                vcall(m, e, c, 0x58, &[matrix]);
            }
            true
        }
        4 => {
            each_shown(m, e, l, &mut |m, e, item| {
                vcall(m, e, item, 0x58, &[matrix]);
            });
            true
        }
        1 => false,
        _ => {
            rcall(m, e, 0x1d0, &[state]);
            let raise = m.r32(l + 0x70);
            rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
            false
        }
    };
    if arrows && m.r32(l + 0xa0) != 0 {
        for at in [0x148, 0x14c] {
            let a = m.r32(l + at);
            if a != 0 {
                vcall(m, e, a, 0x58, &[matrix]);
            }
        }
    }
    if m.r32(l + 0xa0) != 0 {
        let f = m.r32(l + 0x194);
        if f != 0 {
            vcall(m, e, f, 0x58, &[matrix]);
        }
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// The items shown, from the first (+0x198), as many as +0x1a0 (read again each time), each the menu's next
fn each_shown(m: &mut Mem, e: &mut dyn FeEnv, l: u32, f: &mut dyn FnMut(&mut Mem, &mut dyn FeEnv, u32)) {
    if (m.r32(l + 0x1a0) as i32) <= 0 {
        return;
    }
    let mut item = m.r32(l + 0x198);
    let mut k = 0i32;
    loop {
        f(m, e, item);
        k += 1;
        item = e.call(m, MENU_NEXT, &[l, item]);
        if k >= m.r32(l + 0x1a0) as i32 {
            break;
        }
    }
}

/// `cFE3DWList` vf11 0x2b5c80 (its text): while shown, with the state raised, by the mode: 2 the chosen item's
/// text, 4 the items shown, then (while chosen) the arrows'; the others put the state back and run the menu's vf11;
/// then (while chosen) the frame's; the state put back
pub fn list_render_text(m: &mut Mem, e: &mut dyn FeEnv, l: u32) {
    if m.r32(l + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(l + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let arrows = match m.r32(l + 0x190) {
        2 => {
            let c = m.r32(l + 0x140);
            if c != 0 {
                vcall(m, e, c, 0x60, &[]);
            }
            true
        }
        4 => {
            each_shown(m, e, l, &mut |m, e, item| {
                vcall(m, e, item, 0x60, &[]);
            });
            true
        }
        _ => {
            rcall(m, e, 0x1d0, &[state]);
            e.call(m, MENU_VF11, &[l]);
            false
        }
    };
    if arrows && m.r32(l + 0xa0) != 0 {
        for at in [0x148, 0x14c] {
            let a = m.r32(l + at);
            if a != 0 {
                vcall(m, e, a, 0x60, &[]);
            }
        }
    }
    if m.r32(l + 0xa0) != 0 {
        let f = m.r32(l + 0x194);
        if f != 0 {
            vcall(m, e, f, 0x60, &[]);
        }
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DWList` vf41 0x2b5e40 / vf42 0x2b5e98 (`buf` for the scroll): the menu's next / previous choice, then the
/// list scrolled (forward / back) when it changed
pub fn list_step(m: &mut Mem, e: &mut dyn FeEnv, l: u32, forward: bool, buf: u32) {
    let old = m.r32(l + 0x140);
    e.call(m, if forward { MENU_VF41 } else { MENU_VF42 }, &[l]);
    let new = m.r32(l + 0x140);
    if old != new {
        scroll(m, e, l, new, if forward { 1 } else { u32::MAX }, buf);
    }
}

/// `cFE3DWList` vf39 0x2b5ef0 (laid out; `buf` 0x40 bytes), by the mode: 2 the menu's layout, then the list's +0x160
/// with the chosen item's index; 4 the window's count settled (all the items when unset or more than there are), the
/// list turned so the chosen first item is in the middle (the last half moved to the front), the first and last
/// shown, the menu's layout, the list turned back and the arrows updated; the others the menu's layout. Then the
/// frame centred on the chosen item
pub fn list_layout(m: &mut Mem, e: &mut dyn FeEnv, l: u32, buf: u32) {
    let list = l + 0x118;
    match m.r32(l + 0x190) {
        2 => {
            let c = m.r32(l + 0x140);
            e.call(m, MENU_VF39, &[l]);
            if c != 0 {
                let i = e.call(m, NODE_INDEX, &[c]);
                vcall(m, e, l, 0x160, &[i]);
            }
        }
        4 => {
            if (m.r32(l + 0x1a0) as i32) <= 0 {
                let n = e.call(m, LIST_COUNT, &[list]);
                m.w32(l + 0x1a0, n);
            }
            let n = e.call(m, LIST_COUNT, &[list]);
            if (n as i32) < m.r32(l + 0x1a0) as i32 {
                let n = e.call(m, LIST_COUNT, &[list]);
                m.w32(l + 0x1a0, n);
            }
            let half = (m.r32(l + 0x1a0) as i32) / 2;
            let first = e.call(m, LIST_FIRST, &[list]);
            m.w32(l + 0x140, first);
            for _ in 0..half.max(0) {
                let node = e.call(m, LIST_LAST, &[list]);
                e.call(m, NODE_UNLINK, &[node]);
                e.call(m, LIST_PUSH_FRONT, &[list, node]);
            }
            let first = e.call(m, LIST_FIRST, &[list]);
            m.w32(l + 0x198, first);
            let k = m.r32(l + 0x1a0).wrapping_sub(1);
            let last = e.call(m, LIST_NTH, &[list, k]);
            m.w32(l + 0x19c, last);
            e.call(m, MENU_VF39, &[l]);
            for _ in 0..half.max(0) {
                let node = e.call(m, LIST_FIRST, &[list]);
                e.call(m, NODE_UNLINK, &[node]);
                e.call(m, LIST_PUSH_BACK, &[list, node]);
            }
            e.call(m, MENU_ARROWS, &[l]);
        }
        _ => {
            e.call(m, MENU_VF39, &[l]);
        }
    }
    let f = m.r32(l + 0x194);
    if f != 0 && m.r32(l + 0x140) != 0 {
        frame_offset(m, e, buf, l, buf + 0x20);
        vcall(m, e, f, 0x70, &[buf]);
    }
}

/// `cFE3DWList` vf36 0x2b60f0(l, lo, hi; `buf` 0x20 bytes): the menu's bounds, widened by the frame's
pub fn list_bounds(m: &mut Mem, e: &mut dyn FeEnv, l: u32, lo: u32, hi: u32, buf: u32) {
    e.call(m, MENU_VF36, &[l, lo, hi]);
    let f = m.r32(l + 0x194);
    if f == 0 {
        return;
    }
    vcall(m, e, f, 0x128, &[buf, buf + 0x10]);
    for k in 0..3 {
        let a = min(rf(m, lo + 4 * k), rf(m, buf + 4 * k));
        wf(m, lo + 4 * k, a);
    }
    for k in 0..3 {
        let a = max(rf(m, hi + 4 * k), rf(m, buf + 0x10 + 4 * k));
        wf(m, hi + 4 * k, a);
    }
}

/// `cFE3DWList` vf37 0x2b6210(l, out; `buf` 0x10 bytes): its size, by the mode: 2 the chosen item's (or the first's)
/// size within the box, the box with neither; 1 and 3 the menu's; 4 the shown items' sizes laid along the list (each
/// with the box's size that way when set, and the gap; the gap taken off the end), the box the other way or the
/// largest item; the others nothing
pub fn list_size(m: &mut Mem, e: &mut dyn FeEnv, l: u32, out: u32, buf: u32) {
    for k in 0..3 {
        m.w32(out + 4 * k, 0);
    }
    match m.r32(l + 0x190) {
        2 => {
            for k in 0..4 {
                m.w32(buf + 4 * k, 0);
            }
            let mut c = m.r32(l + 0x140);
            if c == 0 {
                c = e.call(m, LIST_FIRST, &[l + 0x118]);
            }
            if c == 0 {
                let v = q(m, l + 0x170);
                put(m, buf, v);
            } else {
                vcall(m, e, c, 0x130, &[buf]);
                let bw = rf(m, l + 0x170);
                if lt(bw, rf(m, buf)) {
                    wf(m, buf, bw);
                }
                let bh = rf(m, l + 0x174);
                if lt(bh, rf(m, buf + 4)) {
                    wf(m, buf + 4, bh);
                }
            }
            let v = q(m, buf);
            put(m, out, v);
        }
        1 | 3 => {
            e.call(m, MENU_VF37, &[l, out]);
        }
        4 => {
            let down = m.r32(l + 0x150) != 0;
            if down {
                let v = rf(m, l + 0x170);
                wf(m, out, v);
            } else {
                let v = rf(m, l + 0x174);
                wf(m, out + 4, v);
            }
            if (m.r32(l + 0x1a0) as i32) > 0 {
                let mut item = m.r32(l + 0x198);
                let mut k = 0i32;
                loop {
                    if m.r32(item + 0x14) == 0 {
                        k -= 1;
                    } else {
                        vcall(m, e, item, 0x130, &[buf]);
                        let (along, across) = if m.r32(l + 0x150) == 0 { (0u32, 4u32) } else { (4, 0) };
                        if along == 4 {
                            let v = max(rf(m, out), rf(m, buf));
                            wf(m, out, v);
                        }
                        let b = rf(m, l + 0x170 + along);
                        let gap = rf(m, l + 0x15c);
                        let step = if zero(b) { rf(m, buf + along) + gap } else { b + gap };
                        let v = rf(m, out + along) + step;
                        wf(m, out + along, v);
                        if along == 0 {
                            let v = max(rf(m, out + across), rf(m, buf + across));
                            wf(m, out + across, v);
                        }
                    }
                    item = e.call(m, MENU_NEXT, &[l, item]);
                    k += 1;
                    if item == m.r32(l + 0x198) || k >= m.r32(l + 0x1a0) as i32 {
                        break;
                    }
                }
            }
            let gap = rf(m, l + 0x15c);
            let at = if m.r32(l + 0x150) == 0 { out } else { out + 4 };
            let v = rf(m, at) - gap;
            wf(m, at, v);
        }
        _ => {}
    }
}

/// `StaticInit_FE3DWList` 0x2b64b8: the list's colours: (1, 0.9645, 0) at 0x3d3760, (1, 0.4746, 0) at 0x3d3770,
/// (1, 0.0758, 0.002) at 0x3d3780, and at 0x3d3790 -1, 1, 1, 0, 0, 1, 1
pub fn static_init(m: &mut Mem) {
    let one = ONE;
    for (at, v) in [
        (0x3d3760u32, [one, 0x3f76_e979, 0]),
        (0x3d3770, [one, 0x3ef2_fec5, 0]),
        (0x3d3780, [one, 0x3d9b_3d08, 0x3b03_126f]),
    ] {
        for (k, x) in v.iter().enumerate() {
            m.w32(at + 4 * k as u32, *x);
        }
    }
    for (k, x) in [u32::MAX, one, one, 0, 0, one, one].iter().enumerate() {
        m.w32(0x3d3790 + 4 * k as u32, *x);
    }
}

/// `cFE3DWList` vf17 0x2b6560: 1
pub fn list_one() -> u32 {
    1
}

/// `cFE3DWList` vf43 0x2b6568(l, k; `buf` for the scroll): the k-th item chosen (the old one told +0xf0 with 0, the
/// new one with 1 while the list is chosen); then scrolled to it: from the first item in mode 2, from the old one in
/// the others but 1 when it changed
pub fn list_choose(m: &mut Mem, e: &mut dyn FeEnv, l: u32, k: u32, buf: u32) {
    let old = m.r32(l + 0x140);
    let item = e.call(m, LIST_NTH, &[l + 0x118, k]);
    if item != 0 {
        let c = m.r32(l + 0x140);
        if c != 0 {
            vcall(m, e, c, 0xf0, &[0]);
        }
        if m.r32(l + 0xa0) != 0 {
            vcall(m, e, item, 0xf0, &[1]);
        }
        m.w32(l + 0x140, item);
    }
    match m.r32(l + 0x190) {
        2 => {
            e.call(m, LIST_FIRST, &[l + 0x118]);
            let c = m.r32(l + 0x140);
            scroll(m, e, l, c, 1, buf);
        }
        1 => {}
        _ => {
            let c = m.r32(l + 0x140);
            if c != old {
                scroll(m, e, l, c, 1, buf);
            }
        }
    }
}

/// `cFE3DWList` vf1 0x2b6680: the frame destroyed (+8 with 3) and cleared, the menu's vf1, the window cleared
pub fn list_vf1(m: &mut Mem, e: &mut dyn FeEnv, l: u32) {
    let f = m.r32(l + 0x194);
    if f != 0 {
        vcall(m, e, f, 8, &[3]);
    }
    m.w32(l + 0x194, 0);
    e.call(m, MENU_VF1, &[l]);
    m.w32(l + 0x19c, 0);
    m.w32(l + 0x1a0, 0);
    m.w32(l + 0x198, 0);
}

/// `cFE3DWList` vf5 0x2b66d8(l, a): the menu's, then the frame's +0x30 (its second register left holding the frame)
pub fn list_vf5(m: &mut Mem, e: &mut dyn FeEnv, l: u32, a: u32) {
    e.call(m, MENU_VF5, &[l, a]);
    let f = m.r32(l + 0x194);
    if f != 0 {
        vcall(m, e, f, 0x30, &[f]);
    }
}

/// `cFE3DWList` vf13 0x2b6720(l, v; `op` 0x20 bytes): the items moved by the vector (an offset operation over the
/// list), its own position by its x, y and z, the arrows and the frame by the vector
pub fn list_move(m: &mut Mem, e: &mut dyn FeEnv, l: u32, v: u32, op: u32) {
    m.w32(op, OP_OFFSET.0);
    let vv = q(m, v);
    put(m, op + 0x10, vv);
    list_apply(m, e, l + 0x118, op);
    let vv = q(m, v);
    let p = q(m, l + 0x80);
    put(m, l + 0x80, [p[0] + vv[0], p[1] + vv[1], p[2] + vv[2], p[3] + F(0.0)]);
    for at in [0x148, 0x14c] {
        let a = m.r32(l + at);
        if a != 0 {
            vcall(m, e, a, 0x70, &[v]);
        }
    }
    e.call(m, OP_OFFSET.1, &[op, 2]);
    let f = m.r32(l + 0x194);
    if f != 0 {
        vcall(m, e, f, 0x70, &[v]);
    }
}

/// `cFE3DWList::~cFE3DWList` 0x2b6838(l, flags): its table, its vf1, then the menu's destructor
pub fn list_dtor(m: &mut Mem, e: &mut dyn FeEnv, l: u32, flags: u32) -> u32 {
    m.w32(l + 0xc, LIST_VTABLE);
    list_vf1(m, e, l);
    e.call(m, MENU_DTOR, &[l, flags])
}

/// `cFE3DWList::cFE3DWList` 0x2b68e0(l): the menu (0, 0, 1, 2, 0), then its table
pub fn list_ctor(m: &mut Mem, e: &mut dyn FeEnv, l: u32) -> u32 {
    e.call(m, MENU_CTOR, &[l, 0, 0, 1, 2, 0]);
    m.w32(l + 0xc, LIST_VTABLE);
    l
}

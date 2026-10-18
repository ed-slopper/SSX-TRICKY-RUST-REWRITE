//! The front end's menu (board row G9y1) on the game's bytes: `cFE3DMenu`, a widget holding a list of items (+0x118)
//! that the pad moves between: its constructor and destructor, its layout (the items formatted and laid along the
//! menu by `cListFE3DWMenuFormatOp`, or sized to the largest), its arrows (`FE3DMenu_UpdateArrows`), the next and
//! previous items that can be chosen, the alignment of an item before or after another, its input (moves, accept,
//! back and two more keys, each with its sound), bounds, size, render, update and move, and the small operations it
//! applies over its items. Every call out goes through [`FeEnv`]; the functions of the row that call each other,
//! and the widget's own (`fe_widget`, `fe_wgroup`), are called directly. Checked in the function runner
//! (`tests/fe_menu.rs`).
//!
//! The menu: +0x110..+0x116 its sounds (no move, a move, accept, back, the 0x63 key chosen, the 0x61 key chosen,
//! either key on an item that can't be chosen), +0x13c the pad's way reversed, +0x140 the chosen item, +0x144 the
//! arrows left where they are, +0x148 / +0x14c the arrows before and after, +0x150 the way it runs (0 across, 1
//! down), +0x154 the items' alignment in their boxes, +0x158 whether the choice wraps round, +0x15c the gap, +0x160
//! whether its box is the largest item, +0x170 its box (or the items' boxes), +0x180 whether its items lay
//! themselves out. An item can be chosen while shown (+0x14) and enabled (+0xa4 clear).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_widget::{destroy, set_flag, vf5 as widget_vf5, vf7 as widget_vf7};
use crate::fe_wgroup::{
    list_apply, list_dtor, node_vf1, FREE, LIST_DELETE_ALL, LIST_INIT, OP_FORMAT, OP_OFFSET, OP_RENDER_TEXT,
    OP_RESET_ACTIVITY, OP_UPDATE,
};
use crate::fe_widgets::widget_init;
use crate::pf::Mem;
use crate::ps2f::F;

/// The menu's table; the item format operation's and the set-active one's tables; the list operator's
pub const MENU_VTABLE: u32 = 0x3acd30;
pub const FORMAT_OP: u32 = 0x3acd18;
pub const ACTIVE_OP: u32 = 0x3aceb0;
pub const OPERATOR_VTABLE: u32 = 0x3aa708;
/// The list helpers: first (0x2ac018), last (0x2ac030), the n-th (0x2ac048), push to the back (0x2ac140), the
/// index of an entry (0x2ac0c0), whether a node is the end (0x2abf10)
pub const LIST_FIRST: u32 = 0x2ac018;
pub const LIST_LAST: u32 = 0x2ac030;
pub const LIST_NTH: u32 = 0x2ac048;
pub const LIST_PUSH_BACK: u32 = 0x2ac140;
pub const LIST_FIND: u32 = 0x2ac0c0;
pub const NODE_IS_END: u32 = 0x2abf10;
/// The pad: left, right, up, down, accept, back and the keys 0x63 (0x26ce68) and 0x61 (0x26ce58)
pub const INPUT_LEFT: u32 = 0x26ce98;
pub const INPUT_RIGHT: u32 = 0x26cea8;
pub const INPUT_UP: u32 = 0x26ce78;
pub const INPUT_DOWN: u32 = 0x26ce88;
pub const INPUT_ACCEPT: u32 = 0x26ce08;
pub const INPUT_BACK: u32 = 0x26ce30;
pub const INPUT_KEY63: u32 = 0x26ce68;
pub const INPUT_KEY61: u32 = 0x26ce58;
/// The operations' destructors: each the base's (`cListOperator::~cListOperator` 0x269560)
pub const OPERATOR_DTORS: [u32; 14] = [
    0x269388, 0x269390, 0x269398, 0x2693a0, 0x2693a8, 0x2693b0, 0x2693b8, 0x269440, 0x2694c8, 0x269558, 0x26f4c0,
    0x26fb78, 0x270210, 0x269560,
];

const ONE: u32 = 0x3f80_0000;
const HALF: F = F(0.5);

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// `c.olt` / `c.eq` with 0 as the EE does them: no NaN (an exponent of all ones is a huge value), denormals as 0
fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

fn zero(a: F) -> bool {
    a.ee() == 0.0
}

fn min(a: F, b: F) -> F {
    if lt(b, a) { b } else { a }
}

fn max(a: F, b: F) -> F {
    if lt(a, b) { b } else { a }
}

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

fn putw(m: &mut Mem, at: u32, v: [u32; 4]) {
    let a = at & !0xf;
    for (k, x) in v.iter().enumerate() {
        m.w32(a + 4 * k as u32, *x);
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

/// The front end's sound player (its +0xa4; the table at +0, the call at +0x50) given the menu's sound at `at`
fn sound(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, menu: u32, at: u32) {
    let snd = m.r32(fe + 0xa4);
    let id = m.r8(menu + at) as i8 as i32 as u32;
    let vt = m.r32(snd);
    let adj = m.r16(vt + 0x50) as i16 as u32;
    let f = m.r32(vt + 0x54);
    e.call(m, f, &[snd.wrapping_add(adj), id]);
}

fn front_end(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x734)
}

/// Whether an item can be chosen: shown and enabled
fn choosable(m: &Mem, item: u32) -> bool {
    m.r32(item + 0x14) != 0 && m.r32(item + 0xa4) == 0
}

/// An operation over the menu's items: its table at `op`, applied, then destroyed (its destructor with 2)
fn apply_op(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, table: (u32, u32), op: u32) {
    m.w32(op, table.0);
    list_apply(m, e, menu + 0x118, op);
}

// ---- small ones

/// `FE_SetRumbleEnabled` 0x26d1c0(p, v): the player's rumble (+0x48)
pub fn set_rumble_enabled(m: &mut Mem, p: u32, v: u32) {
    m.w32(p + 0x48, v);
}

/// `cFE3DMenu::cFE3DMenu` 0x26d3b8(menu, owner, parent, way, alignment, wrap): the widget, its table, its list set
/// up; the way it runs, the alignment and the wrap kept; its box the largest item, nothing chosen, no arrows, no
/// gap, the box (0, 0, 0, 1); its sounds (7, 4, 5, 6, 5, 5, 7)
pub fn menu_ctor(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, owner: u32, parent: u32, way: u32, align: u32, wrap: u32) -> u32 {
    widget_init(m, e, menu, owner, parent);
    m.w32(menu + 0xc, MENU_VTABLE);
    e.call(m, LIST_INIT, &[menu + 0x118]);
    m.w32(menu + 0x150, way);
    m.w32(menu + 0x154, align);
    m.w32(menu + 0x158, wrap);
    m.w32(menu + 0x160, 1);
    for o in [0x13cu32, 0x140, 0x144, 0x148, 0x14c, 0x15c, 0x170] {
        m.w32(menu + o, 0);
    }
    m.w32(menu + 0x17c, ONE);
    m.w32(menu + 0x178, 0);
    m.w32(menu + 0x174, 0);
    m.w32(menu + 0x180, 0);
    for k in (0..7).rev() {
        m.w8(menu + 0x110 + k, 0xff);
    }
    for (k, v) in [(1u32, 4u8), (3, 6), (5, 5), (6, 7), (0, 7), (2, 5), (4, 5)] {
        m.w8(menu + 0x110 + k, v);
    }
    menu
}

/// `cFE3DMenu` vf40 0x26d658(menu, w): the widget put last in the list
pub fn menu_add(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, w: u32) -> u32 {
    e.call(m, LIST_PUSH_BACK, &[menu + 0x118, w])
}

/// `cFE3DMenu` vf18 0x26f4c8: 1
pub fn menu_one() -> u32 {
    1
}

/// 0x26f688(menu, v): the box (+0x170) from a vector
pub fn set_box(m: &mut Mem, menu: u32, v: u32) {
    let x: Vec<u32> = (0..4).map(|k| m.r32((v & !0xf) + 4 * k)).collect();
    putw(m, menu + 0x170, [x[0], x[1], x[2], x[3]]);
}

/// 0x26f698(menu; w, h; `buf`): the box (w, h, 0, 0)
pub fn set_box_wh(m: &mut Mem, menu: u32, w: F, h: F, buf: u32) {
    putw(m, buf, [w.0.to_bits(), h.0.to_bits(), 0, 0]);
    set_box(m, menu, buf);
}

/// 0x26f7c0(menu): while chosen, the arrows' text (+0x60)
pub fn arrows_text(m: &mut Mem, e: &mut dyn FeEnv, menu: u32) {
    if m.r32(menu + 0xa0) == 0 {
        return;
    }
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x60, &[]);
        }
    }
}

/// 0x26f830(menu, matrix): while chosen, the arrows rendered (+0x58)
pub fn arrows_render(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, matrix: u32) {
    if m.r32(menu + 0xa0) == 0 {
        return;
    }
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x58, &[matrix]);
        }
    }
}

/// `cFE3DMenu` vf43 0x26f8d8(menu, k): the k-th item chosen (the old one told +0xf0 with 0, the new one with 1 while
/// the menu is chosen)
pub fn choose(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, k: u32) {
    let item = e.call(m, LIST_NTH, &[menu + 0x118, k]);
    if item == 0 {
        return;
    }
    let c = m.r32(menu + 0x140);
    if c != 0 {
        vcall(m, e, c, 0xf0, &[0]);
    }
    if m.r32(menu + 0xa0) != 0 {
        vcall(m, e, item, 0xf0, &[1]);
    }
    m.w32(menu + 0x140, item);
}

/// 0x26f970(menu, k): the k-th item
pub fn nth(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, k: u32) -> u32 {
    e.call(m, LIST_NTH, &[menu + 0x118, k])
}

/// 0x26f990(menu, id): the item with that id
pub fn find(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, id: u32) -> u32 {
    e.call(m, LIST_FIND, &[menu + 0x118, id])
}

/// `cListOperator::~cListOperator` 0x269560 and every operation's destructor that jumps to it (`OPERATOR_DTORS`;
/// op, flags): its table, freed on flag 1
pub fn operator_dtor(m: &mut Mem, e: &mut dyn FeEnv, op: u32, flags: u32) {
    m.w32(op, OPERATOR_VTABLE);
    if flags & 1 != 0 {
        e.call(m, FREE, &[op, 1]);
    }
}

/// `ListFE3DFormatOp_Apply1` 0x26fa70(op, item): the item formatted (+0x140); 0
pub fn op_format(m: &mut Mem, e: &mut dyn FeEnv, item: u32) -> u32 {
    vcall(m, e, item, 0x140, &[]);
    0
}

/// `cListFE3DSetActiveDefaultStartDataOp` vf1 0x26faf8(op, item): the item's +0x30 (its second register left
/// holding the item); 0
pub fn op_set_active(m: &mut Mem, e: &mut dyn FeEnv, item: u32) -> u32 {
    vcall(m, e, item, 0x30, &[item]);
    0
}

// ---- the choice

/// `FE3DMenu_NextSelectable` 0x26e538 / `PrevSelectable` 0x26e5c8(menu, item): the next / previous item that can be
/// chosen (+8 / +4); at the list's end, round to its first / last when the menu wraps, else `item`
pub fn next_choosable(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, item: u32, forward: bool) -> u32 {
    let link = if forward { 8 } else { 4 };
    let mut s = m.r32(item + link);
    loop {
        if e.call(m, NODE_IS_END, &[s]) == 0 && choosable(m, s) {
            return s;
        }
        if e.call(m, NODE_IS_END, &[s]) != 0 {
            if m.r32(menu + 0x158) == 0 {
                return item;
            }
            s = e.call(m, if forward { LIST_FIRST } else { LIST_LAST }, &[menu + 0x118]);
        } else {
            s = m.r32(s + link);
        }
    }
}

/// `cFE3DMenu` vf41 0x26e658 / vf42 0x26e7b8 (`buf` for the arrows): the next / previous choice (the other way
/// with +0x13c). None: the no-move sound. Else the move sound, the old item told it is left, the owner told (+0xb8:
/// the old, the new), the new chosen, the arrow on that side restarted (+0x40, then +0x20 with 1), the arrows
/// updated and the new item told it is chosen
pub fn step(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, forward: bool, buf: u32) {
    let reversed = m.r32(menu + 0x13c) != 0;
    let cur = m.r32(menu + 0x140);
    let next = next_choosable(m, e, menu, cur, forward != reversed);
    let fe = front_end(m);
    if next == m.r32(menu + 0x140) {
        if fe != 0 {
            sound(m, e, fe, menu, 0x110);
        }
        return;
    }
    if fe != 0 {
        sound(m, e, fe, menu, 0x111);
    }
    let old = m.r32(menu + 0x140);
    vcall(m, e, old, 0xf0, &[0]);
    let owner = m.r32(menu + 0xec);
    if owner != 0 {
        let old = m.r32(menu + 0x140);
        vcall(m, e, owner, 0xb8, &[old, next]);
    }
    let arrow = m.r32(menu + if forward { 0x14c } else { 0x148 });
    m.w32(menu + 0x140, next);
    if arrow != 0 {
        vcall(m, e, arrow, 0x40, &[]);
        vcall(m, e, arrow, 0x20, &[1]);
    }
    update_arrows(m, e, menu, buf);
    let c = m.r32(menu + 0x140);
    vcall(m, e, c, 0xf0, &[1]);
}

/// `cFE3DMenu` vf28 0x26e150(menu, input): for a pressed input, while the menu is enabled (and, while active, its
/// owner's update leaves the owner in state 2): the chosen item's own input first; the pad's way along the menu
/// steps back (+0x158) or on (+0x150); accept (its sound by whether the item can be chosen) tells the owner (+0xd8)
/// when it can; back (its sound when the owner allows, +0x1c) tells the owner (+0xb0); the keys 0x63 and 0x61 their
/// sounds and the owner (+0xc8 / +0xd0) when the item is enabled, answering 0
pub fn on_input(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, input: u32) -> u32 {
    if m.r32(input) == 0 {
        return 0;
    }
    if m.r32(menu + 0x10) != 0 {
        let owner = m.r32(menu + 0xec);
        if owner != 0 {
            vcall(m, e, owner, 0x48, &[]);
            let owner = m.r32(menu + 0xec);
            if m.r32(owner + 0x10) != 2 {
                return 0;
            }
        }
    }
    if m.r32(menu + 0xa4) != 0 {
        return 0;
    }
    let c = m.r32(menu + 0x140);
    if m.r32(c + 0xa8) != 0 && m.r32(c + 0xa4) == 0 && vcall(m, e, c, 0xe8, &[input]) != 0 {
        return 1;
    }
    let way = |m: &Mem| m.r32(menu + 0x150);
    let back = (e.call(m, INPUT_LEFT, &[input]) != 0 && way(m) == 0)
        || (e.call(m, INPUT_UP, &[input]) != 0 && way(m) == 1);
    // (the calls are made in the game's order: left, then up only when left didn't step)
    if back {
        vcall(m, e, menu, 0x158, &[]);
        return 1;
    }
    let on = (e.call(m, INPUT_RIGHT, &[input]) != 0 && way(m) == 0)
        || (e.call(m, INPUT_DOWN, &[input]) != 0 && way(m) == 1);
    if on {
        vcall(m, e, menu, 0x150, &[]);
        return 1;
    }
    let ok = |m: &Mem| {
        let c = m.r32(menu + 0x140);
        m.r32(c + 0xa8) != 0 && m.r32(c + 0xa4) == 0
    };
    if e.call(m, INPUT_ACCEPT, &[input]) != 0 {
        let fe = front_end(m);
        if fe != 0 {
            let at = if ok(m) { 0x112 } else { 0x110 };
            sound(m, e, fe, menu, at);
        }
        let owner = m.r32(menu + 0xec);
        if owner != 0 && ok(m) {
            let c = m.r32(menu + 0x140);
            vcall(m, e, owner, 0xd8, &[c]);
        }
        return 1;
    }
    if e.call(m, INPUT_BACK, &[input]) != 0 {
        let fe = front_end(m);
        let owner = m.r32(menu + 0xec);
        if fe != 0 {
            if owner == 0 {
                return 1;
            }
            if m.r32(owner + 0x1c) != 0 {
                sound(m, e, fe, menu, 0x113);
            }
        }
        let owner = m.r32(menu + 0xec);
        if owner != 0 {
            let c = m.r32(menu + 0x140);
            vcall(m, e, owner, 0xb0, &[c]);
        }
        return 1;
    }
    for (key, good, bad, call) in [(INPUT_KEY63, 0x114, 0x116, 0xc8), (INPUT_KEY61, 0x115, 0x110, 0xd0)] {
        if e.call(m, key, &[input]) == 0 {
            continue;
        }
        let fe = front_end(m);
        if fe != 0 {
            let at = if ok(m) { good } else { bad };
            sound(m, e, fe, menu, at);
        }
        let owner = m.r32(menu + 0xec);
        if owner != 0 {
            let c = m.r32(menu + 0x140);
            if m.r32(c + 0xa4) == 0 {
                vcall(m, e, owner, call, &[c]);
            }
        }
        return 0;
    }
    0
}

// ---- layout

/// `FE3DMenu_AlignItem` 0x26dd60 / 0x26df58(menu, ref, item; `buf` 0x40 bytes): `item` moved to sit before / after
/// `ref` along the menu (by the box when set, else by `ref`'s size, and the gap) and centred on it across (by the box
/// when set)
pub fn align(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, rf_: u32, item: u32, after: bool, buf: u32) {
    let mut p = q(m, rf_ + 0x80);
    vcall(m, e, item, 0x130, &[buf + 0x10]);
    vcall(m, e, rf_, 0x130, &[buf + 0x20]);
    let si = [rf(m, buf + 0x10), rf(m, buf + 0x14)];
    let sr = [rf(m, buf + 0x20), rf(m, buf + 0x24)];
    let gap = rf(m, menu + 0x15c);
    let (along, across) = if m.r32(menu + 0x150) == 0 { (0usize, 1usize) } else { (1, 0) };
    let boxes = [rf(m, menu + 0x170), rf(m, menu + 0x174)];
    // (across first when the menu runs down, as the game does it)
    let do_across = |m: &Mem, p: &mut [F; 4]| {
        let _ = m;
        if !zero(boxes[across]) {
            p[across] = p[across] + (sr[across] - si[across]) * HALF;
        }
    };
    if along == 1 {
        do_across(m, &mut p);
    }
    let b = boxes[along];
    let step = if zero(b) {
        sr[along] + gap
    } else {
        let d = if after { sr[along] - si[along] } else { si[along] - sr[along] };
        b + d * HALF + gap
    };
    p[along] = if after { p[along] + step } else { p[along] - step };
    if along == 0 {
        do_across(m, &mut p);
    }
    offset_to(m, item, p, buf + 0x30);
    vcall(m, e, item, 0x70, &[buf + 0x30]);
}

/// `cListFE3DWMenuFormatOp` vf1 0x26f2a0(op, item; `buf` 0x30 bytes): a shown item placed at the running position
/// (+0x10) in its box (+0x30, by the alignment +0x24), and the position moved on along the menu (+0x20 the way) by
/// the box (or the item's size) and the gap (+0x28); 0
pub fn format_op_apply(m: &mut Mem, e: &mut dyn FeEnv, op: u32, item: u32, buf: u32) -> u32 {
    if m.r32(item + 0x14) == 0 {
        return 0;
    }
    let mut p = q(m, op + 0x10);
    vcall(m, e, item, 0x130, &[buf]);
    let size = [rf(m, buf), rf(m, buf + 4)];
    let along = if m.r32(op + 0x20) == 0 { 0 } else { 1 };
    let b = rf(m, op + 0x30 + 4 * along as u32);
    let gap = rf(m, op + 0x28);
    let step = if zero(b) { size[along] + gap } else { b + gap };
    let at = op + 0x10 + 4 * along as u32;
    let v = rf(m, at) + step;
    wf(m, at, v);
    let a = m.r32(op + 0x24);
    let (bw, bh) = (rf(m, op + 0x30), rf(m, op + 0x34));
    if a & 1 != 0 {
        p[0] = p[0] + (bw - size[0]);
    } else if a & 2 == 0 {
        p[0] = p[0] + (bw - size[0]) * HALF;
    }
    if a & 8 != 0 {
        p[1] = p[1] + (bh - size[1]);
    } else if a & 4 == 0 {
        p[1] = p[1] + (bh - size[1]) * HALF;
    }
    offset_to(m, item, p, buf + 0x20);
    vcall(m, e, item, 0x70, &[buf + 0x20]);
    0
}

/// `cFE3DMenu` vf39 0x26d660 (laid out; `buf` 0x60 bytes): the chosen item kept when it can be chosen, else the
/// first that can; told it is chosen while the menu is. With items that lay themselves out, each formatted and the
/// arrows updated. Else the box set to the largest item (each formatted) or the items formatted, then the items
/// laid along the menu from its position (the format operation: the way, the alignment, the gap, the box) and the
/// arrows updated
pub fn layout(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, buf: u32) {
    let list = menu + 0x118;
    let c = m.r32(menu + 0x140);
    if c == 0 || m.r32(c + 0xa4) != 0 || m.r32(c + 0x14) == 0 {
        let mut a = e.call(m, LIST_FIRST, &[list]);
        if !choosable(m, a) {
            a = next_choosable(m, e, menu, a, true);
        }
        m.w32(menu + 0x140, a);
    }
    if m.r32(menu + 0xa0) != 0 {
        let c = m.r32(menu + 0x140);
        vcall(m, e, c, 0xf0, &[1]);
    }
    if m.r32(menu + 0x180) != 0 {
        apply_op(m, e, menu, OP_FORMAT, buf);
        update_arrows(m, e, menu, buf + 0x10);
        operator_dtor(m, e, buf, 2);
        return;
    }
    if m.r32(menu + 0x160) != 0 {
        let mut s = e.call(m, LIST_FIRST, &[list]);
        putw(m, menu + 0x170, [0, 0, 0, ONE]);
        while s != 0 && e.call(m, NODE_IS_END, &[s]) == 0 {
            vcall(m, e, s, 0x140, &[]);
            vcall(m, e, s, 0x130, &[buf]);
            for k in 0..3 {
                let v = max(rf(m, menu + 0x170 + 4 * k), rf(m, buf + 4 * k));
                wf(m, menu + 0x170 + 4 * k, v);
            }
            s = m.r32(s + 8);
        }
    } else {
        apply_op(m, e, menu, OP_FORMAT, buf);
        operator_dtor(m, e, buf, 2);
    }
    let op = buf + 0x20;
    m.w32(op, FORMAT_OP);
    let pos: Vec<u32> = (0..4).map(|k| m.r32(((menu + 0x80) & !0xf) + 4 * k)).collect();
    putw(m, op + 0x10, [pos[0], pos[1], pos[2], pos[3]]);
    let (way, align) = (m.r32(menu + 0x150), m.r32(menu + 0x154));
    m.w32(op + 0x20, way);
    m.w32(op + 0x24, align);
    m.w32(op + 0x28, m.r32(menu + 0x15c));
    let bx: Vec<u32> = (0..4).map(|k| m.r32(((menu + 0x170) & !0xf) + 4 * k)).collect();
    putw(m, op + 0x30, [bx[0], bx[1], bx[2], bx[3]]);
    list_apply(m, e, list, op);
    update_arrows(m, e, menu, buf + 0x40);
    // (0x26f4c0, the format operation's destructor: the base's)
    operator_dtor(m, e, op, 2);
}

/// `FE3DMenu_UpdateArrows` 0x26d8c8(menu; `buf` 0x60 bytes): with arrows, each shown unless the choice can't go
/// that way (without wrap), and put beside the chosen item's bounds (5 apart, centred on it across): across a menu
/// running across, before and after it; for one running down, both before it, side by side (the first unless they
/// are left where they are, +0x144, only for a menu running across)
pub fn update_arrows(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, buf: u32) {
    if m.r32(menu + 0x148) == 0 && m.r32(menu + 0x14c) == 0 {
        return;
    }
    let c = m.r32(menu + 0x140);
    vcall(m, e, c, 0x128, &[buf, buf + 0x10]);
    let cy = (rf(m, buf + 4) + rf(m, buf + 0x14)) * HALF;
    let lo_x = rf(m, buf);
    let five = F(5.0);
    let across = m.r32(menu + 0x150) == 0;
    // (whether the arrow is shown: the choice can go its way)
    let can = |m: &mut Mem, e: &mut dyn FeEnv, forward: bool| -> bool {
        if m.r32(menu + 0x158) != 0 {
            return true;
        }
        let cur = m.r32(menu + 0x140);
        next_choosable(m, e, menu, cur, forward) != cur
    };
    let mut x_down = lo_x;
    for (at, forward) in [(0x148u32, false), (0x14c, true)] {
        let a = m.r32(menu + at);
        if a == 0 {
            if forward {
                return;
            }
            continue;
        }
        if !can(m, e, forward) {
            let a = m.r32(menu + at);
            vcall(m, e, a, 0x28, &[0]);
            if forward {
                return;
            }
            continue;
        }
        let a = m.r32(menu + at);
        vcall(m, e, a, 0x28, &[1]);
        if across && m.r32(menu + 0x144) != 0 {
            if forward {
                return;
            }
            continue;
        }
        let a = m.r32(menu + at);
        vcall(m, e, a, 0x130, &[buf + 0x20]);
        let (sx, sy) = (rf(m, buf + 0x20), rf(m, buf + 0x24));
        let mut p = q(m, menu + 0x80);
        p[0] = if across {
            if forward { rf(m, buf + 0x10) + five } else { lo_x - sx - five }
        } else {
            x_down = x_down - sx;
            x_down - five
        };
        p[1] = cy - sy * HALF;
        let a = m.r32(menu + at);
        offset_to(m, a, p, buf + 0x40);
        vcall(m, e, a, 0x70, &[buf + 0x40]);
    }
}

// ---- the virtuals over the items

/// `cFE3DMenu` vf5 0x26e918(menu; `op`): the set-active operation over the items, the widget's vf5, each arrow's
/// +0x30 (its second register left holding the arrow); the operation destroyed (0x26fb78)
pub fn menu_vf5(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, op: u32) {
    m.w32(op, ACTIVE_OP);
    list_apply(m, e, menu + 0x118, op);
    widget_vf5(m, e, menu, op + 0x10);
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x30, &[a]);
        }
    }
    operator_dtor(m, e, op, 2);
}

/// `cFE3DMenu` vf7 0x26e9a8(menu; `op`): the reset over the items, then the widget's (vf7)
pub fn menu_vf7(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, op: u32) {
    apply_op(m, e, menu, OP_RESET_ACTIVITY, op);
    widget_vf7(m, e, menu);
    operator_dtor(m, e, op, 2);
}

/// `cFE3DMenu` vf1 0x26e9f8: its items deleted, the node's vf1, nothing chosen, both arrows destroyed (+8 with 3)
/// and cleared, the box, the wrap and the gap cleared
pub fn menu_vf1(m: &mut Mem, e: &mut dyn FeEnv, menu: u32) {
    e.call(m, LIST_DELETE_ALL, &[menu + 0x118]);
    node_vf1(m, e, menu);
    m.w32(menu + 0x140, 0);
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 8, &[3]);
        }
    }
    m.w32(menu + 0x148, 0);
    m.w32(menu + 0x14c, 0);
    putw(m, menu + 0x170, [0; 4]);
    m.w32(menu + 0x158, 0);
    m.w32(menu + 0x15c, 0);
}

/// `cFE3DMenu` vf11 0x26ea98 (its text; `op`): while shown, with the renderer's state raised, the items' text, the
/// arrows' while chosen; the state put back, the operation destroyed
pub fn menu_render_text(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, op: u32) {
    if m.r32(menu + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(menu + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    apply_op(m, e, menu, OP_RENDER_TEXT, op);
    arrows_text(m, e, menu);
    rcall(m, e, 0x1d0, &[state]);
    operator_dtor(m, e, op, 2);
}

/// `cFE3DMenu` vf12 0x26eb98 (`op`): the items' update, its own +0x48 while active, the arrows' update (+0x68)
pub fn menu_update(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, op: u32) {
    apply_op(m, e, menu, OP_UPDATE, op);
    if m.r32(menu + 0x10) != 0 {
        vcall(m, e, menu, 0x48, &[]);
    }
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x68, &[]);
        }
    }
    operator_dtor(m, e, op, 2);
}

/// `cFE3DMenu` vf36 0x26ec40(menu, lo, hi; `buf` 0x30 bytes): the bounds from (99999, ...) and (-99999, ...),
/// widened by each shown item's bounds (all four lanes), then by each arrow's (three)
pub fn menu_bounds(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, lo: u32, hi: u32, buf: u32) {
    let mut s = e.call(m, LIST_FIRST, &[menu + 0x118]);
    putw(m, lo, [0x47c3_4f80; 4]);
    putw(m, hi, [0xc7c3_4f80; 4]);
    let widen = |m: &mut Mem, lanes: u32, a: u32, b: u32| {
        for k in 0..lanes {
            let v = min(rf(m, lo + 4 * k), rf(m, a + 4 * k));
            wf(m, lo + 4 * k, v);
        }
        for k in 0..lanes {
            let v = max(rf(m, hi + 4 * k), rf(m, b + 4 * k));
            wf(m, hi + 4 * k, v);
        }
    };
    while e.call(m, NODE_IS_END, &[s]) == 0 {
        if m.r32(s + 0x14) != 0 {
            vcall(m, e, s, 0x128, &[buf + 0x10, buf + 0x20]);
            widen(m, 4, buf + 0x10, buf + 0x20);
        }
        s = m.r32(s + 8);
    }
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x128, &[buf, buf + 0x10]);
            widen(m, 3, buf, buf + 0x10);
        }
    }
}

/// `cFE3DMenu` vf37 0x26efd8(menu, out; `buf` 0x20 bytes): its size: its own bounds' span when its items lay
/// themselves out; else the shown items laid along it (each the box that way when set, else its size, and the gap;
/// the largest across), less one gap when there was any, and the box across when set
pub fn menu_size(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, out: u32, buf: u32) {
    if m.r32(menu + 0x180) != 0 {
        vcall(m, e, menu, 0x128, &[buf, buf + 0x10]);
        let (lo, hi) = (q(m, buf), q(m, buf + 0x10));
        put(m, out, [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], hi[3] - lo[3]]);
        return;
    }
    let mut s = e.call(m, LIST_FIRST, &[menu + 0x118]);
    putw(m, out, [0; 4]);
    while e.call(m, NODE_IS_END, &[s]) == 0 {
        if m.r32(s + 0x14) != 0 {
            vcall(m, e, s, 0x130, &[buf]);
            let (along, across) = if m.r32(menu + 0x150) == 0 { (0u32, 4u32) } else { (4, 0) };
            let b = rf(m, menu + 0x170 + along);
            let gap = rf(m, menu + 0x15c);
            let step = if zero(b) { rf(m, buf + along) + gap } else { b + gap };
            let v = rf(m, out + along) + step;
            wf(m, out + along, v);
            if lt(rf(m, out + across), rf(m, buf + across)) {
                let v = rf(m, buf + across);
                wf(m, out + across, v);
            }
        }
        s = m.r32(s + 8);
    }
    let (along, across) = if m.r32(menu + 0x150) == 0 { (0u32, 4u32) } else { (4, 0) };
    if lt(F(0.0), rf(m, out + along)) {
        let v = rf(m, out + along) - rf(m, menu + 0x15c);
        wf(m, out + along, v);
    }
    let b = rf(m, menu + 0x170 + across);
    if !zero(b) {
        wf(m, out + across, b);
    }
}

/// `cFE3DMenu` vf29 0x26f4d0(menu, v): chosen or not (the widget's flag), the chosen item told, and when chosen the
/// owner told (+0xb8 with none and the item; the owner is not checked for); then, unless active and on track 5, its
/// colour (+0x50) from the set its flags pick: chosen +0xac (+0xdc while disabled), else +0xbc (+0xcc disabled)
pub fn menu_set_chosen(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, v: u32) {
    set_flag(m, e, menu, 0xa0, v);
    let c = m.r32(menu + 0x140);
    if c != 0 {
        vcall(m, e, c, 0xf0, &[v]);
        if v != 0 {
            let owner = m.r32(menu + 0xec);
            let c = m.r32(menu + 0x140);
            vcall(m, e, owner, 0xb8, &[0, c]);
        }
    }
    if m.r32(menu + 0x10) != 0 && m.r32(menu + 0x20) == 5 {
        return;
    }
    let enabled = m.r32(menu + 0xa8) != 0;
    let set = if m.r32(menu + 0xa0) != 0 {
        if enabled { 0xac } else { 0xdc }
    } else if enabled {
        0xbc
    } else {
        0xcc
    };
    vcall(m, e, menu, 0x50, &[menu + set]);
}

/// `cFE3DMenu` vf13 0x26f5c0(menu, v; `op` 0x20 bytes): the items moved by the vector, its own position by its x, y
/// and z, the arrows by the vector
pub fn menu_move(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, v: u32, op: u32) {
    m.w32(op, OP_OFFSET.0);
    let vv: Vec<u32> = (0..4).map(|k| m.r32((v & !0xf) + 4 * k)).collect();
    putw(m, op + 0x10, [vv[0], vv[1], vv[2], vv[3]]);
    list_apply(m, e, menu + 0x118, op);
    let vv = q(m, v);
    let p = q(m, menu + 0x80);
    put(m, menu + 0x80, [p[0] + vv[0], p[1] + vv[1], p[2] + vv[2], p[3] + F(0.0)]);
    for at in [0x148, 0x14c] {
        let a = m.r32(menu + at);
        if a != 0 {
            vcall(m, e, a, 0x70, &[v]);
        }
    }
    operator_dtor(m, e, op, 2);
}

/// `cFE3DMenu::~cFE3DMenu` 0x26f9d0(menu, flags): its table, its vf1, its list's destructor, then the widget's
pub fn menu_dtor(m: &mut Mem, e: &mut dyn FeEnv, menu: u32, flags: u32) -> u32 {
    m.w32(menu + 0xc, MENU_VTABLE);
    menu_vf1(m, e, menu);
    list_dtor(m, e, menu + 0x118, 2);
    destroy(m, e, menu, flags)
}

/// `cListFE3DGetExtentOp` vf1 0x269620(op, item; `buf` 0x20 bytes): a shown item's bounds widening the low and high
/// corners the operation points at (+4, +8; three lanes); 0
pub fn get_extent_op(m: &mut Mem, e: &mut dyn FeEnv, op: u32, item: u32, buf: u32) -> u32 {
    if m.r32(item + 0x14) == 0 {
        return 0;
    }
    vcall(m, e, item, 0x128, &[buf, buf + 0x10]);
    let lo = m.r32(op + 4);
    for k in 0..3 {
        let v = rf(m, buf + 4 * k);
        if lt(v, rf(m, lo + 4 * k)) {
            wf(m, lo + 4 * k, v);
        }
    }
    let hi = m.r32(op + 8);
    for k in 0..3 {
        let v = rf(m, buf + 0x10 + 4 * k);
        if lt(rf(m, hi + 4 * k), v) {
            wf(m, hi + 4 * k, v);
        }
    }
    0
}

/// `cListFE3DFindHashNameOp` vf1 0x2701a8(op, item): the item when its name's hash (+0x74) is the one looked for
/// (+4), else 0
pub fn find_hash_op(m: &Mem, op: u32, item: u32) -> u32 {
    if m.r32(item + 0x74) == m.r32(op + 4) { item } else { 0 }
}

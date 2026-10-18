//! The front end's entries, list box, name entry and button (board row G9x3a) on the game's bytes: `cFE3DWEntry` (a
//! widget holding two children, +0x110 and an optional +0x114, that it lays out side by side and passes most of its
//! virtuals on to), `FE3DButton_Layout`, `cFE3DWListBox` (a list of widgets at +0x114 showing the one at +0x110),
//! `cFE3DWNameEntry` (the on-screen keyboard of 42 characters) and `FE3D_GetSpriteRect`. Every call out goes through
//! [`FeEnv`]; the functions of the row that call each other, and the widget's own (`fe_widget`, `fe_wgroup`), are
//! called directly. Checked in the function runner (`tests/fe_wentry.rs`).
//!
//! An entry: +0x118 the gap after the first child, +0x11c and +0x130 the two children's alignments in their boxes
//! (bit 1 to the left / top edge as it is, bit 0 to the right, else centred; bits 2 and 3 the same for y), +0x120 and
//! +0x140 the boxes' sizes (0 for the child's own).
//!
//! The name entry: +0x110 the chosen character, +0x114 the characters to a row, +0x118 the scale, +0x11c the cursor
//! widget, +0x120..+0x122 the sounds for a move, an accept and a back.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_venuesel::LOC_STRING;
use crate::fe_widget::{destroy, set_flag, DRAWER, DRAWER_COLOUR};
use crate::fe_wgroup::{list_dtor, node_vf1, widget_place, LIST_DELETE_ALL, LIST_INIT};
use crate::fe_widgets::widget_init;
use crate::pf::Mem;
use crate::ps2f::F;

/// The tables: the entry, the list box, the name entry
pub const ENTRY_VTABLE: u32 = 0x3b6bb0;
pub const LISTBOX_VTABLE: u32 = 0x3b6a68;
pub const NAME_VTABLE: u32 = 0x3b6920;
/// The sprite rectangles (0x1f9e50: the sprites at gApp+0x728, the index); a list node's index (0x2abec0), whether
/// it is the list's end (0x2abf10), a list's first (0x2ac018) and last (0x2ac030); a widget put at a world position
/// (0x2acb20); a sprite drawn scaled (0x14eed0: the rectangle, the colour index; x, y, sx, sy); the font's centred
/// shadowed print and printf (0x19d770, 0x19d6e0: the font, the string; scale, x, y)
pub const GET_RECT: u32 = 0x1f9e50;
pub const NODE_INDEX: u32 = 0x2abec0;
pub const NODE_IS_END: u32 = 0x2abf10;
pub const LIST_FIRST: u32 = 0x2ac018;
pub const LIST_LAST: u32 = 0x2ac030;
pub const SET_WORLD_POS: u32 = 0x2acb20;
pub const SPRITE_DRAW: u32 = 0x14eed0;
pub const PRINT_CENTRED: u32 = 0x19d770;
pub const PRINTF_CENTRED: u32 = 0x19d6e0;
/// The pad: left (0x26ce98), right (0x26cea8), up (0x26ce78), down (0x26ce88), accept (0x26ce08), back (0x26ce30)
pub const INPUT_LEFT: u32 = 0x26ce98;
pub const INPUT_RIGHT: u32 = 0x26cea8;
pub const INPUT_UP: u32 = 0x26ce78;
pub const INPUT_DOWN: u32 = 0x26ce88;
pub const INPUT_ACCEPT: u32 = 0x26ce08;
pub const INPUT_BACK: u32 = 0x26ce30;
/// The name entry's 42 characters, its cell width and height, and the end key's scale
pub const CHARS: u32 = 0x3b7f58;
pub const CELL_W: u32 = 0x348464;
pub const CELL_H: u32 = 0x348468;
pub const END_SCALE: u32 = 0x34846c;

const ONE: u32 = 0x3f80_0000;
const HALF: F = F(0.5);

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// `cvt.s.w`: toward zero
fn fl(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

/// `c.olt` / `c.ole` / `c.eq` with 0 as the EE does them: no NaN (an exponent of all ones is a huge value),
/// denormals as 0
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

fn rcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// A quadword's copy (both addresses taken down to 16)
fn quad(m: &mut Mem, to: u32, from: u32) {
    let v: Vec<u32> = (0..4).map(|k| m.r32((from & !0xf) + 4 * k)).collect();
    for (k, x) in v.into_iter().enumerate() {
        m.w32((to & !0xf) + 4 * k as u32, x);
    }
}

/// The offset that takes `child` from its position (+0x80) to `at` (x, y, z; w 1), written at `out`
fn offset_to(m: &mut Mem, child: u32, at: [F; 3], out: u32) {
    let p = (child + 0x80) & !0xf;
    for k in 0..3 {
        let v = at[k] - rf(m, p + 4 * k as u32);
        wf(m, out + 4 * k as u32, v);
    }
    m.w32(out + 0xc, ONE);
}

/// `FE3D_GetSpriteRect` 0x2ac4b0(w, index): the sprites' rectangle (the widget is not used)
pub fn sprite_rect(m: &mut Mem, e: &mut dyn FeEnv, index: u32) -> u32 {
    let sprites = m.r32(m.r32(GAPP).wrapping_add(0x728));
    e.call(m, GET_RECT, &[sprites, index])
}

// ---- the entry

/// A virtual of both children: the first's, then the second's when there is one
fn both(m: &mut Mem, e: &mut dyn FeEnv, w: u32, off: u32, args: &[u32]) {
    let a = m.r32(w + 0x110);
    vcall(m, e, a, off, args);
    let b = m.r32(w + 0x114);
    if b != 0 {
        vcall(m, e, b, off, args);
    }
}

/// `cFE3DWEntry` vf9 0x2af0b8 / vf32 0x2af140 / vf33 0x2af1c8 / vf34 0x2af250 / vf35 0x2af2d8 (w, c): the colour
/// (+0x60) or a colour set (+0xac, +0xbc, +0xcc, +0xdc) from `c`, and the children's same virtual (+0x50, +0x108,
/// +0x110, +0x118, +0x120) with it
pub fn entry_set_colour(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, off: u32, c: u32) {
    for k in 0..4 {
        m.w32(w + at + 4 * k, m.r32(c + 4 * k));
    }
    both(m, e, w, off, &[c]);
}

/// `cFE3DWEntry` vf3 0x2af438 / vf4 0x2af360 (w, v): +0x10 / +0x14, and the children's +0x20 / +0x28
pub fn entry_set_word(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, off: u32, v: u32) {
    m.w32(w + at, v);
    both(m, e, w, off, &[v]);
}

/// `cFE3DWEntry` vf29 0x2af3c8 / vf31 0x2af4a0 (w, v): the widget's flag (+0xa0 / +0xa8), and the children's +0xf0
/// / +0x100
pub fn entry_set_flag(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, off: u32, v: u32) {
    set_flag(m, e, w, at, v);
    both(m, e, w, off, &[v]);
}

/// `cFE3DWEntry` vf28 0x2af510(w, input): the second child's input (+0xe8), 0 without one
pub fn entry_on_input(m: &mut Mem, e: &mut dyn FeEnv, w: u32, input: u32) -> u32 {
    let b = m.r32(w + 0x114);
    if b == 0 {
        return 0;
    }
    vcall(m, e, b, 0xe8, &[input])
}

/// `cFE3DWEntry` vf1 0x2af550(w): each child destroyed (+8 with 3), both cleared, then the node's vf1
pub fn entry_vf1(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    for at in [0x110, 0x114] {
        let c = m.r32(w + at);
        if c != 0 {
            vcall(m, e, c, 8, &[3]);
        }
    }
    m.w32(w + 0x110, 0);
    m.w32(w + 0x114, 0);
    node_vf1(m, e, w);
}

/// `cFE3DWEntry` vf36 0x2af5c0(w, lo, hi; `buf` 0x20 bytes): the bounds: from the position to the position plus the
/// box (+0x120), widened by the first child's bounds; with a second child, the high corner pushed out to the second
/// box (x: the first box or the first child's high x, plus +0x140 and the gap; y: +0x144 from the position) and
/// widened by its bounds
pub fn entry_bounds(m: &mut Mem, e: &mut dyn FeEnv, w: u32, lo: u32, hi: u32, buf: u32) {
    quad(m, lo, w + 0x80);
    let l = lo & !0xf;
    let mut t = [F(0.0); 4];
    for k in 0..4u32 {
        t[k as usize] = rf(m, l + 4 * k) + rf(m, ((w + 0x120) & !0xf) + 4 * k);
    }
    for (k, v) in t.iter().enumerate() {
        wf(m, (hi & !0xf) + 4 * k as u32, *v);
        wf(m, buf + 0x10 + 4 * k as u32, *v);
        wf(m, buf + 4 * k as u32, *v);
    }
    let widen = |m: &mut Mem| {
        for k in 0..3 {
            let a = min(rf(m, lo + 4 * k), rf(m, buf + 4 * k));
            wf(m, lo + 4 * k, a);
        }
        for k in 0..3 {
            let a = max(rf(m, hi + 4 * k), rf(m, buf + 0x10 + 4 * k));
            wf(m, hi + 4 * k, a);
        }
    };
    let a = m.r32(w + 0x110);
    vcall(m, e, a, 0x128, &[buf, buf + 0x10]);
    widen(m);
    let b = m.r32(w + 0x114);
    if b == 0 {
        return;
    }
    let bx = rf(m, w + 0x140);
    if !zero(bx) {
        let hi0 = rf(m, hi);
        let base = if zero(rf(m, w + 0x120)) { hi0 } else { rf(m, w + 0x80) + rf(m, w + 0x120) };
        let x = bx + base + rf(m, w + 0x118);
        wf(m, hi, max(hi0, x));
    }
    let by = rf(m, w + 0x144);
    if !zero(by) {
        let y = rf(m, w + 0x84) + by;
        let v = max(rf(m, hi + 4), y);
        wf(m, hi + 4, v);
    }
    vcall(m, e, b, 0x128, &[buf, buf + 0x10]);
    widen(m);
}

/// One axis of a child's size in its box: the box when set, else by the alignment bits (`edge` the child's size,
/// `right` 0, else half)
fn fit(m: &Mem, boxv: F, mode: u32, edge_bit: u32, mask: u32, size: F) -> Option<F> {
    if !zero(boxv) {
        Some(boxv)
    } else if (mode >> edge_bit) & 1 != 0 {
        Some(size)
    } else if mode & mask != 0 {
        None
    } else {
        let _ = m;
        Some(size * HALF)
    }
}

/// `cFE3DWEntry` vf37 0x2af8c0(w, out; `buf` 0x20 bytes): its size: the first child's (at least the box) fitted by
/// the alignment, plus the gap; with a second child, its size fitted the same way added across and taken as the
/// height when taller
pub fn entry_size(m: &mut Mem, e: &mut dyn FeEnv, w: u32, out: u32, buf: u32) {
    for k in 0..3 {
        m.w32(out + 4 * k, 0);
    }
    let a = m.r32(w + 0x110);
    vcall(m, e, a, 0x130, &[buf]);
    let bx = rf(m, w + 0x120);
    if lt(rf(m, buf), bx) {
        wf(m, buf, bx);
    }
    let by = rf(m, w + 0x124);
    if lt(rf(m, buf + 4), by) {
        wf(m, buf + 4, by);
    }
    let mode = m.r32(w + 0x11c);
    if let Some(v) = fit(m, bx, mode, 1, 3, rf(m, buf)) {
        wf(m, out, v);
    }
    let by = rf(m, w + 0x124);
    if let Some(v) = fit(m, by, mode, 2, 0xc, rf(m, buf + 4)) {
        wf(m, out + 4, v);
    }
    let x = rf(m, out) + rf(m, w + 0x118);
    wf(m, out, x);
    let b = m.r32(w + 0x114);
    if b == 0 {
        return;
    }
    vcall(m, e, b, 0x130, &[buf + 0x10]);
    let mode = m.r32(w + 0x130);
    let bx = rf(m, w + 0x140);
    if let Some(v) = fit(m, bx, mode, 1, 3, rf(m, buf + 0x10)) {
        let x = rf(m, out) + v;
        wf(m, out, x);
    }
    let by = rf(m, w + 0x144);
    if let Some(v) = fit(m, by, mode, 2, 0xc, rf(m, buf + 0x14)) {
        let y = max(v, rf(m, out + 4));
        wf(m, out + 4, y);
    }
}

/// Where a child goes in its box from `at`: by bit 1 (as is), bit 0 (to the right: box − size), else centred; y the
/// same with bits 2 and 3
fn place(m: &Mem, mode: u32, at: [F; 3], boxv: [F; 2], size: [F; 2]) -> [F; 3] {
    let x = if (mode >> 1) & 1 != 0 {
        at[0]
    } else if mode & 1 != 0 {
        at[0] + boxv[0] - size[0]
    } else {
        at[0] + (boxv[0] - size[0]) * HALF
    };
    let y = if (mode >> 2) & 1 != 0 {
        at[1]
    } else if (mode >> 3) & 1 != 0 {
        at[1] + boxv[1] - size[1]
    } else {
        at[1] + (boxv[1] - size[1]) * HALF
    };
    let _ = m;
    [x, y, at[2]]
}

/// `cFE3DWEntry` vf39 0x2afb30 (laid out; `buf` 0x60 bytes): the first child formatted (+0x140), sized (+0x130) and
/// moved into its box at the position; the second the same in its box after the first box (or the first child's
/// size by the alignment) and the gap
pub fn entry_layout(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    let p = (w + 0x80) & !0xf;
    let pos = [rf(m, p), rf(m, p + 4), rf(m, p + 8)];
    let a = m.r32(w + 0x110);
    vcall(m, e, a, 0x140, &[]);
    let a = m.r32(w + 0x110);
    vcall(m, e, a, 0x130, &[buf + 0x10]);
    let mode = m.r32(w + 0x11c);
    let size_a = [rf(m, buf + 0x10), rf(m, buf + 0x14)];
    let at = place(m, mode, pos, [rf(m, w + 0x120), rf(m, w + 0x124)], size_a);
    let a = m.r32(w + 0x110);
    offset_to(m, a, at, buf + 0x30);
    vcall(m, e, a, 0x70, &[buf + 0x30]);
    let b = m.r32(w + 0x114);
    if b == 0 {
        return;
    }
    vcall(m, e, b, 0x140, &[]);
    let bx = rf(m, w + 0x120);
    let mut x = pos[0];
    if !zero(bx) {
        x = x + bx;
    } else if (mode >> 1) & 1 != 0 {
        x = x + size_a[0];
    } else if (mode & 3) == 0 {
        x = x + size_a[0] * HALF;
    }
    let x = x + rf(m, w + 0x118);
    let b = m.r32(w + 0x114);
    vcall(m, e, b, 0x130, &[buf + 0x30]);
    let mode = m.r32(w + 0x130);
    let size_b = [rf(m, buf + 0x30), rf(m, buf + 0x34)];
    let at = place(m, mode, [x, pos[1], pos[2]], [rf(m, w + 0x140), rf(m, w + 0x144)], size_b);
    let b = m.r32(w + 0x114);
    offset_to(m, b, at, buf + 0x50);
    vcall(m, e, b, 0x70, &[buf + 0x50]);
}

/// `cFE3DWEntry` vf13 0x2afe48(w, v): the position moved by the vector's x, y and z, and the children by the vector
pub fn entry_move(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: u32) {
    let at = v & !0xf;
    let add = [rf(m, at), rf(m, at + 4), rf(m, at + 8), F(0.0)];
    let to = (w + 0x80) & !0xf;
    for (k, a) in add.into_iter().enumerate() {
        let p = to + 4 * k as u32;
        let s = rf(m, p) + a;
        wf(m, p, s);
    }
    both(m, e, w, 0x70, &[v]);
}

/// `cFE3DWEntry` vf11 0x2afec8 (drawn): while shown, the children's text (+0x60) with the renderer's state raised
pub fn entry_draw(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    if m.r32(w + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(w + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    both(m, e, w, 0x60, &[]);
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DWEntry` vf12 0x2b30d8: when active its +0x48, then each child's +0x68
pub fn entry_update(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    if m.r32(w + 0x10) != 0 {
        vcall(m, e, w, 0x48, &[]);
    }
    for at in [0x110, 0x114] {
        let c = m.r32(w + at);
        if c != 0 {
            vcall(m, e, c, 0x68, &[]);
        }
    }
}

/// `cFE3DWEntry` vf22 0x2b30d0 and `cFE3DWNameEntry` vf26 0x2b2990: 1
pub fn entry_one() -> u32 {
    1
}

/// `cFE3DWEntry::~cFE3DWEntry` 0x2b3358(w, flags): its table, its vf1, then the widget's destructor
pub fn entry_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, ENTRY_VTABLE);
    entry_vf1(m, e, w);
    destroy(m, e, w, flags)
}

/// `cFE3DWEntry::cFE3DWEntry` 0x2b35d0(w): the widget with no owner or parent, then its table
pub fn entry_ctor(m: &mut Mem, e: &mut dyn FeEnv, w: u32) -> u32 {
    widget_init(m, e, w, 0, 0);
    m.w32(w + 0xc, ENTRY_VTABLE);
    w
}

/// `FE3DButton_Layout` 0x2ae098(w, at, text; `buf` 0x40 bytes): the icon (+0x110) placed at the sprite's size (at most
/// 17 each way) and coloured white; the gap 5; the label (+0x114) given its text (+0x150), scale 0.7, no border, and
/// coloured (1, 0, 0, 0); the boxes (0, the height, 0, 1); the first child to the left, the second centred; the gap
/// 4; then laid out (+0x140)
pub fn button_layout(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, text: u32, buf: u32) {
    let icon = m.r32(w + 0x110);
    let r = sprite_rect(m, e, at);
    let mut x = rf(m, r.wrapping_add(4));
    let r = sprite_rect(m, e, at);
    let mut y = rf(m, r.wrapping_add(8));
    let top = F(17.0);
    if lt(top, x) {
        x = top;
    }
    if lt(top, y) {
        y = top;
    }
    widget_place(m, e, icon, at, x, y, buf + 0x20);
    for k in 0..4 {
        m.w32(buf + 4 * k, ONE);
    }
    vcall(m, e, icon, 0x50, &[buf]);
    let label = m.r32(w + 0x114);
    wf(m, w + 0x118, F(5.0));
    vcall(m, e, label, 0x150, &[text]);
    m.w32(label + 0x118, 0x3f33_3333);
    m.w32(label + 0x114, 0);
    for (k, v) in [ONE, 0, 0, 0].into_iter().enumerate() {
        m.w32(buf + 4 * k as u32, v);
    }
    vcall(m, e, label, 0x50, &[buf]);
    let v = [0, y.0.to_bits(), 0, ONE];
    for (k, x) in v.iter().enumerate() {
        m.w32(((w + 0x120) & !0xf) + 4 * k as u32, *x);
        m.w32(((w + 0x140) & !0xf) + 4 * k as u32, *x);
    }
    m.w32(w + 0x11c, 1);
    m.w32(w + 0x130, 2);
    wf(m, w + 0x118, F(4.0));
    vcall(m, e, w, 0x140, &[]);
}

// ---- the list box

/// `FE3DList_GetSelectedIndex` 0x2affd0(lb): the shown node's index (0x2abec0), -1 with none
pub fn selected_index(m: &mut Mem, e: &mut dyn FeEnv, lb: u32) -> u32 {
    let node = m.r32(lb + 0x110);
    if node == 0 {
        return u32::MAX;
    }
    e.call(m, NODE_INDEX, &[node])
}

/// `cFE3DWListBox` vf28 0x2b0000(lb, input): left and right move to the node before / after (round to the last /
/// first at the end), the old one told it is left (+0xf0 with 0), the new one chosen (+0xf0 with 1), and the owner
/// told (+0xc0); 1 when moved
pub fn listbox_on_input(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, input: u32) -> u32 {
    if m.r32(lb + 0x110) == 0 {
        return 0;
    }
    let (link, wrap) = if e.call(m, INPUT_LEFT, &[input]) != 0 {
        (4, LIST_LAST)
    } else if e.call(m, INPUT_RIGHT, &[input]) != 0 {
        (8, LIST_FIRST)
    } else {
        return 0;
    };
    let cur = m.r32(lb + 0x110);
    vcall(m, e, cur, 0xf0, &[0]);
    let next = m.r32(m.r32(lb + 0x110) + link);
    m.w32(lb + 0x110, next);
    if e.call(m, NODE_IS_END, &[next]) != 0 {
        let v = e.call(m, wrap, &[lb + 0x114]);
        m.w32(lb + 0x110, v);
    }
    let cur = m.r32(lb + 0x110);
    vcall(m, e, cur, 0xf0, &[1]);
    let owner = m.r32(lb + 0xec);
    if owner != 0 {
        vcall(m, e, owner, 0xc0, &[lb]);
    }
    1
}

/// `cFE3DWListBox` vf1 0x2b0120(lb): its list emptied, none shown, then the node's vf1
pub fn listbox_vf1(m: &mut Mem, e: &mut dyn FeEnv, lb: u32) {
    e.call(m, LIST_DELETE_ALL, &[lb + 0x114]);
    m.w32(lb + 0x110, 0);
    node_vf1(m, e, lb);
}

/// `cFE3DWListBox` vf11 0x2b0150 (drawn; `buf` 0x10 bytes): while shown, the shown widget moved to the list box's
/// position (+0x70), given its colour (+0x50 with +0x60) and its text drawn (+0x60)
pub fn listbox_draw(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, buf: u32) {
    if m.r32(lb + 0x14) == 0 {
        return;
    }
    let c = m.r32(lb + 0x110);
    if c == 0 {
        return;
    }
    let p = (lb + 0x80) & !0xf;
    let at = [rf(m, p), rf(m, p + 4), rf(m, p + 8)];
    offset_to(m, c, at, buf);
    vcall(m, e, c, 0x70, &[buf]);
    let c = m.r32(lb + 0x110);
    vcall(m, e, c, 0x50, &[lb + 0x60]);
    let c = m.r32(lb + 0x110);
    vcall(m, e, c, 0x60, &[]);
}

/// `cFE3DWListBox` vf37 0x2b01f8(lb, out): the shown widget's size
pub fn listbox_size(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, out: u32) {
    let c = m.r32(lb + 0x110);
    if c != 0 {
        vcall(m, e, c, 0x130, &[out]);
    }
}

/// `cFE3DWListBox::~cFE3DWListBox` 0x2b2cb0(lb, flags): its table, its vf1, its list's destructor, then the
/// widget's
pub fn listbox_dtor(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, flags: u32) -> u32 {
    m.w32(lb + 0xc, LISTBOX_VTABLE);
    listbox_vf1(m, e, lb);
    list_dtor(m, e, lb + 0x114, 2);
    destroy(m, e, lb, flags)
}

/// `cFE3DWListBox::cFE3DWListBox` 0x2b2d00(lb, owner, parent): the widget, none shown, its table, its list set up
pub fn listbox_ctor(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, owner: u32, parent: u32) -> u32 {
    widget_init(m, e, lb, owner, parent);
    m.w32(lb + 0x110, 0);
    m.w32(lb + 0xc, LISTBOX_VTABLE);
    e.call(m, LIST_INIT, &[lb + 0x114]);
    lb
}

// ---- the name entry

/// `cFE3DWNameEntry::cFE3DWNameEntry` 0x2b0230(ne): the widget with no owner or parent, its table, no sounds
/// (+0x120..+0x122 -1)
pub fn name_ctor(m: &mut Mem, e: &mut dyn FeEnv, ne: u32) -> u32 {
    widget_init(m, e, ne, 0, 0);
    m.w32(ne + 0xc, NAME_VTABLE);
    for k in (0..3).rev() {
        m.w8(ne + 0x120 + k, 0xff);
    }
    ne
}

/// `NameEntry_GetChar` 0x2b0410(ne): the chosen character
pub fn name_get_char(m: &Mem, ne: u32) -> u32 {
    m.r8(m.r32(ne + 0x110).wrapping_add(CHARS)) as i8 as i32 as u32
}

/// `NameEntry_SetChar` 0x2b0428(ne, c): the character chosen (the last of the 42 that matches)
pub fn name_set_char(m: &mut Mem, ne: u32, c: u32) {
    let c = c as u8 as i8;
    for i in 0..0x2a {
        if m.r8(CHARS + i) as i8 == c {
            m.w32(ne + 0x110, i);
        }
    }
}

/// `cFE3DWNameEntry` vf37 0x2b0468(ne, out): its size: the row's cells less half a cell across, the rows down, at the
/// scale
pub fn name_size(m: &mut Mem, ne: u32, out: u32) {
    let cols = m.r32(ne + 0x114);
    let rows = ((cols as i32).wrapping_add(0x29) / cols as i32) as u32;
    let (cw, scale) = (rf(m, CELL_W), rf(m, ne + 0x118));
    let x = fl(cols) * cw * scale - cw * HALF * scale;
    wf(m, out, x);
    let y = fl(rows) * rf(m, CELL_H) * rf(m, ne + 0x118);
    wf(m, out + 4, y);
}

/// `cFE3DWNameEntry` vf1 0x2b04e0(ne): its cursor destroyed (+8 with 3) and cleared
pub fn name_vf1(m: &mut Mem, e: &mut dyn FeEnv, ne: u32) {
    let c = m.r32(ne + 0x11c);
    if c != 0 {
        vcall(m, e, c, 8, &[3]);
        m.w32(ne + 0x11c, 0);
    }
}

/// `cFE3DWNameEntry` vf11 0x2b0528 (drawn; `buf` 0x30 bytes): while shown, each of the 42 characters in its cell
/// (column and row from the row length, at the scale from the position): the chosen one, while chosen, with the
/// cursor put over it and drawn and in the chosen colour (+0xac), the others in the enabled one (+0xbc); 'e' is the
/// end key (string 0x112 printed smaller), 'd' the delete key (sprite 0x3b), the rest printed
pub fn name_draw(m: &mut Mem, e: &mut dyn FeEnv, ne: u32, buf: u32) {
    if m.r32(ne + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(ne + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let p = (ne + 0x80) & !0xf;
    let pos = [rf(m, p), rf(m, p + 4)];
    for i in 0..0x2au32 {
        let cols = m.r32(ne + 0x114) as i32;
        let (q, r) = ((i as i32) / cols, (i as i32) % cols);
        let scale = rf(m, ne + 0x118);
        let ch = m.r8(CHARS + i);
        m.w8(buf, ch);
        m.w8(buf + 1, 0);
        let x = fl(r as u32) * rf(m, CELL_W) * scale + pos[0];
        let y = fl(q as u32) * rf(m, CELL_H) * scale + pos[1];
        if i == m.r32(ne + 0x110) && m.r32(ne + 0xa0) != 0 {
            let cur = m.r32(ne + 0x11c);
            if cur != 0 {
                let font = m.r32(m.r32(GAPP) + 0x60);
                let fh = fl(m.r32(font + 0xc));
                wf(m, buf + 0x10, x + F(1.0));
                m.w32(buf + 0x18, 0);
                let v = y + fh * scale * HALF + F(2.0);
                wf(m, buf + 0x14, v);
                m.w32(buf + 0x1c, ONE);
                e.call(m, SET_WORLD_POS, &[cur, buf + 0x10]);
                let cur = m.r32(ne + 0x11c);
                vcall(m, e, cur, 0x60, &[]);
            }
            e.call(m, DRAWER_COLOUR, &[DRAWER, ne + 0xac]);
        } else {
            e.call(m, DRAWER_COLOUR, &[DRAWER, ne + 0xbc]);
        }
        let c = m.r8(buf) as i8;
        let font = m.r32(m.r32(GAPP) + 0x60);
        if c == 0x65 {
            let k = rf(m, END_SCALE);
            let s = rf(m, ne + 0x118);
            let fh = fl(m.r32(font + 0xc));
            let size = s * k;
            let yy = y + fh * (s - k) * HALF;
            let loc = m.r32(m.r32(GAPP) + 0x50);
            let text = e.call(m, LOC_STRING, &[loc, 0x112]);
            e.call(m, PRINT_CENTRED, &[font, text, size.0.to_bits(), x.0.to_bits(), yy.0.to_bits()]);
        } else if c == 0x64 {
            let s = rf(m, ne + 0x118);
            let fh = fl(m.r32(font + 0xc));
            let r = sprite_rect(m, e, 0x3b);
            let h = rf(m, r.wrapping_add(8));
            let yy = y + fh * s - h;
            let r = sprite_rect(m, e, 0x3b);
            e.call(m, SPRITE_DRAW, &[r, 0x20, x.0.to_bits(), yy.0.to_bits(), ONE, ONE]);
        } else {
            let s = m.r32(ne + 0x118);
            e.call(m, PRINTF_CENTRED, &[font, buf, s, x.0.to_bits(), y.0.to_bits()]);
        }
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// The front end's sound for a move, an accept or a back (the sound player at the front end's +0xa4; its table at
/// +0, the call at +0x50), when there is a front end
fn name_sound(m: &mut Mem, e: &mut dyn FeEnv, ne: u32, at: u32) -> u32 {
    let fe = m.r32(m.r32(GAPP) + 0x734);
    if fe == 0 {
        return 1;
    }
    let snd = m.r32(fe + 0xa4);
    let id = m.r8(ne + at) as i8 as i32 as u32;
    let vt = m.r32(snd);
    let adj = m.r16(vt + 0x50) as i16 as u32;
    let f = m.r32(vt + 0x54);
    e.call(m, f, &[snd.wrapping_add(adj), id]);
    1
}

/// `cFE3DWNameEntry` vf28 0x2b0850(ne, input): for the front end's player (the input's +0x40 its +5 less one) and a
/// pressed input: left / right move along the row (round to its other end), up / down between rows (from the top
/// row to the last row's same column, or the last character; from the last row to the top), each with the move
/// sound; accept and back tell the owner (+0xd8 / +0xb0) with their sounds; 1 when handled
pub fn name_on_input(m: &mut Mem, e: &mut dyn FeEnv, ne: u32, input: u32) -> u32 {
    let fe = m.r32(m.r32(GAPP) + 0x734);
    let player = (m.r8(fe + 5) as i8 as i32).wrapping_sub(1) as u32;
    if m.r32(input + 0x40) != player || m.r32(input) == 0 {
        return 0;
    }
    if e.call(m, INPUT_LEFT, &[input]) != 0 {
        let cols = m.r32(ne + 0x114) as i32;
        if cols >= 2 {
            let cur = m.r32(ne + 0x110) as i32;
            let v = if cur % cols == 0 { (cur - 1 + cols).min(0x29) } else { cur - 1 };
            m.w32(ne + 0x110, v as u32);
        }
        return name_sound(m, e, ne, 0x120);
    }
    if e.call(m, INPUT_RIGHT, &[input]) != 0 {
        let cols = m.r32(ne + 0x114) as i32;
        if cols >= 2 {
            let cur = m.r32(ne + 0x110) as i32;
            let r = cur % cols;
            let v = if r == cols - 1 {
                (cur + 1 - cols).max(0)
            } else if cur == 0x29 {
                cur - r
            } else {
                cur + 1
            };
            m.w32(ne + 0x110, v as u32);
        }
        return name_sound(m, e, ne, 0x120);
    }
    if e.call(m, INPUT_UP, &[input]) != 0 {
        let cols = m.r32(ne + 0x114) as i32;
        if (cols + 0x29) / cols >= 2 {
            let cur = m.r32(ne + 0x110) as i32;
            let (q, r) = (cur / cols, cur % cols);
            let v = if q != 0 {
                cur - cols
            } else {
                let last = 0x29 % cols;
                if r < last { 0x2a - (last + 1) + r } else { 0x29 }
            };
            m.w32(ne + 0x110, v as u32);
        }
        return name_sound(m, e, ne, 0x120);
    }
    if e.call(m, INPUT_DOWN, &[input]) != 0 {
        let cols = m.r32(ne + 0x114) as i32;
        let rows = (cols + 0x29) / cols;
        if rows >= 2 {
            let cur = m.r32(ne + 0x110) as i32;
            let (q, r) = (cur / cols, cur % cols);
            let v = if q < rows - 1 { (cur + cols).min(0x29) } else { r };
            m.w32(ne + 0x110, v as u32);
        }
        return name_sound(m, e, ne, 0x120);
    }
    if e.call(m, INPUT_ACCEPT, &[input]) != 0 {
        let owner = m.r32(ne + 0xec);
        vcall(m, e, owner, 0xd8, &[ne]);
        return name_sound(m, e, ne, 0x121);
    }
    if e.call(m, INPUT_BACK, &[input]) != 0 {
        let owner = m.r32(ne + 0xec);
        vcall(m, e, owner, 0xb0, &[ne]);
        return name_sound(m, e, ne, 0x122);
    }
    0
}

/// `cFE3DWNameEntry` vf12 0x2b2998: when active its +0x48; while chosen, its cursor's update (+0x68)
pub fn name_update(m: &mut Mem, e: &mut dyn FeEnv, ne: u32) {
    if m.r32(ne + 0x10) != 0 {
        vcall(m, e, ne, 0x48, &[]);
    }
    if m.r32(ne + 0xa0) == 0 {
        return;
    }
    let c = m.r32(ne + 0x11c);
    if c != 0 {
        vcall(m, e, c, 0x68, &[]);
    }
}

/// `cFE3DWNameEntry::~cFE3DWNameEntry` 0x2b2a30(ne, flags): its table, its vf1, then the widget's destructor
pub fn name_dtor(m: &mut Mem, e: &mut dyn FeEnv, ne: u32, flags: u32) -> u32 {
    m.w32(ne + 0xc, NAME_VTABLE);
    name_vf1(m, e, ne);
    destroy(m, e, ne, flags)
}

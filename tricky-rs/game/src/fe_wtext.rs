//! The front end's text widgets (board row G9x2) on the game's bytes: the wrapped text (`cFE3DWText` and its
//! `FE3DText_*` setters, a text box at +0x12c), the buffered single-line text (`cFE3DWBufferedSingleLineText`: its
//! own copy of the string at +0x130), the two-coloured text (`cFE3DWColouredText`: the first word in a second
//! colour) and the scrolling list of lines (`cFE3DScrollingText` / `FE3DScrollingText_*`, the credits' roll). Every
//! call out goes through [`FeEnv`]; the functions of the row that call each other, and the single-line text's own
//! (`fe_widget`), are called directly. Checked in the function runner (`tests/fe_wtext.rs`).
//!
//! The wrapped text: +0x110 the font, +0x114 1, +0x118 1, +0x11c the wrap mode given to the box, +0x120 whether
//! the text came from an id (1) or not (0), +0x124 the id (or -1), the box at +0x12c (its +0x14 the scale).
//!
//! The scrolling text: line ids at +0x110 (up to 30; -1 for none), +0x188 the font, +0x18c the scale, +0x190 the
//! alignment (2 centred, 1 to the right, else to the left), +0x194 the next id, +0x198 / +0x19c the first and last
//! ids, +0x1a0 the way it rolls (0 up, 1 down), +0x1a4 how far the lines have moved, +0x1a8 the speed, +0x1ac the
//! gap under a line, +0x1b0 how many lines fit, +0x1b4 / +0x1b8 a plain line's and a marked line's border.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_venuesel::LOC_STRING;
use crate::fe_widget::{
    destroy, text_construct, text_destroy, text_set_id, text_set_narrow, text_set_wide, text_size, text_vf12, text_vf39,
    DRAWER, DRAWER_COLOUR, DRAW_NARROW, DRAW_NARROW_BORDER, DRAW_WIDE, DRAW_WIDE_BORDER, WIDTH_NARROW, WIDTH_WIDE,
};
use crate::fe_widgets::widget_init;
use crate::pf::Mem;
use crate::ps2f::F;

/// The tables: the wrapped text, the buffered and the coloured single-line texts, the scrolling text
pub const WTEXT_VTABLE: u32 = 0x3b7510;
pub const BUFFERED_VTABLE: u32 = 0x3b7238;
pub const COLOURED_VTABLE: u32 = 0x3b70d0;
pub const SCROLL_VTABLE: u32 = 0x3b67d8;
/// The text box's calls (the box first): set up (0x1957b0), its font (0x196028), its rectangle (0x196070, x, y, w,
/// h as f12..f15), cleared (0x195ff0), a wide text added (0x196088) or a narrow one (0x196128), drawn (0x1957d8,
/// -1, 0), its line height (0x1961e8, f12), its scale (0x1961f0, f12 and f13), its lines wrapped (0x196218, the
/// mode), its widest line (0x1964d8) and its height (0x196530), destroyed (0x14f390, 2)
pub const TB_INIT: u32 = 0x1957b0;
pub const TB_SET_FONT: u32 = 0x196028;
pub const TB_SET_RECT: u32 = 0x196070;
pub const TB_CLEAR: u32 = 0x195ff0;
pub const TB_APPEND: u32 = 0x196088;
pub const TB_APPEND_NARROW: u32 = 0x196128;
pub const TB_DRAW: u32 = 0x1957d8;
pub const TB_SET_LINE_HEIGHT: u32 = 0x1961e8;
pub const TB_SET_SCALE: u32 = 0x1961f0;
pub const TB_WRAP: u32 = 0x196218;
pub const TB_MAX_WIDTH: u32 = 0x1964d8;
pub const TB_HEIGHT: u32 = 0x196530;
pub const TB_DELETE: u32 = 0x14f390;
/// The string copies: wide (0x17cc80), wide with a limit (0x17cd30), narrow (0x2fd14c), narrow with a limit
/// (0x2fd630); the pad's up (0x26ce78) and down (0x26ce88)
pub const WSTR_COPY: u32 = 0x17cc80;
pub const WSTR_COPY_N: u32 = 0x17cd30;
pub const STRCPY: u32 = 0x2fd14c;
pub const STRNCPY: u32 = 0x2fd630;
pub const INPUT_UP: u32 = 0x26ce78;
pub const INPUT_DOWN: u32 = 0x26ce88;
/// The fill (0x2cc808: at, word, size); the narrow string the coloured text measures (0x3b67d0); the two characters
/// the scrolling text looks at first: one that hides a line (0x337e50) and one that marks it (0x337e48)
pub const FILL: u32 = 0x2cc808;
pub const SPACE_TEXT: u32 = 0x3b67d0;
pub const HIDE_CHAR: u32 = 0x337e50;
pub const MARK_CHAR: u32 = 0x337e48;

const ONE: u32 = 0x3f80_0000;

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

/// `cvt.w.s`: toward zero, saturating
fn int(v: F) -> u32 {
    v.ee() as i32 as u32
}

/// `c.olt` / `c.ole` / `c.eq` with 0 as the EE does them: no NaN (an exponent of all ones is a huge value),
/// denormals as 0
fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

fn le(a: F, b: F) -> bool {
    a.ee() <= b.ee()
}

fn abs(v: F) -> F {
    F(f32::from_bits(v.0.to_bits() & 0x7fff_ffff))
}

/// The renderer (gApp+0x724; its table at +0x10)
fn rcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// A string by id (0x2c5160 on gApp+0x50)
fn loc(m: &mut Mem, e: &mut dyn FeEnv, id: u32) -> u32 {
    let l = m.r32(m.r32(GAPP).wrapping_add(0x50));
    e.call(m, LOC_STRING, &[l, id])
}

/// A quadword's copy (`lq` / `sq`: both addresses taken down to 16)
fn quad(m: &mut Mem, to: u32, from: u32) {
    let v: Vec<u32> = (0..4).map(|k| m.r32((from & !0xf) + 4 * k)).collect();
    for (k, x) in v.into_iter().enumerate() {
        m.w32((to & !0xf) + 4 * k as u32, x);
    }
}

/// The box's rectangle from the widget's position (+0x80, +0x84) and size (+0x100, +0x104)
fn set_rect(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    let r = [m.r32(w + 0x80), m.r32(w + 0x84), m.r32(w + 0x100), m.r32(w + 0x104)];
    e.call(m, TB_SET_RECT, &[w + 0x12c, r[0], r[1], r[2], r[3]]);
}

// ---- the wrapped text

/// `FE3DTextBox_Init` 0x2b0b90(w, owner, parent, font; `buf` 0x10 bytes): the widget, then the font (+0x110), no
/// id (+0x124 -1), +0x114 1, +0x120 0, the table; the box set up with the font; its size (640, 50) through vf38;
/// the box's +0x20 and +0x24 cleared, +0x118 1, the wrap mode 0
pub fn text_box_init(m: &mut Mem, e: &mut dyn FeEnv, w: u32, owner: u32, parent: u32, font: u32, buf: u32) -> u32 {
    widget_init(m, e, w, owner, parent);
    m.w32(w + 0x110, font);
    m.w32(w + 0x124, u32::MAX);
    m.w32(w + 0x114, 1);
    m.w32(w + 0x120, 0);
    m.w32(w + 0xc, WTEXT_VTABLE);
    let b = w + 0x12c;
    e.call(m, TB_INIT, &[b]);
    let font = m.r32(w + 0x110);
    e.call(m, TB_SET_FONT, &[b, font]);
    for (k, v) in [640.0f32.to_bits(), 50.0f32.to_bits(), 0, 0].into_iter().enumerate() {
        m.w32(buf + 4 * k as u32, v);
    }
    wtext_vf38(m, e, w, buf);
    m.w32(b + 0x24, 0);
    m.w32(b + 0x20, 0);
    m.w32(w + 0x118, 1);
    m.w32(w + 0x11c, 0);
    w
}

/// `cFE3DWText` vf11 0x2b0dd8 (drawn): while shown (+0x14), the box drawn with the renderer's state raised by +0x70
pub fn wtext_draw(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    if m.r32(w + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(w + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    e.call(m, TB_DRAW, &[w + 0x12c, u32::MAX, 0]);
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DWText` vf12 0x2b0e78: its +0x48
pub fn wtext_vf12(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    vcall(m, e, w, 0x48, &[]);
}

/// `cFE3DWText` vf37 0x2b0ea0 (its size into `out`): the box's widest line and its height, 0, 0
pub fn wtext_size(m: &mut Mem, e: &mut dyn FeEnv, w: u32, out: u32) {
    let x = fl(e.call(m, TB_MAX_WIDTH, &[w + 0x12c]));
    let y = fl(e.call(m, TB_HEIGHT, &[w + 0x12c]));
    let at = out & !0xf;
    for (k, v) in [x.0.to_bits(), y.0.to_bits(), 0, 0].into_iter().enumerate() {
        m.w32(at + 4 * k as u32, v);
    }
}

/// `FE3DText_SetWideText` 0x2b0f18(w, s): the box cleared, the wide text set up, no id (+0x124 -1), +0x120 1
pub fn set_wide_text(m: &mut Mem, e: &mut dyn FeEnv, w: u32, s: u32) {
    e.call(m, TB_CLEAR, &[w + 0x12c]);
    setup_text_box(m, e, w, s);
    m.w32(w + 0x124, u32::MAX);
    m.w32(w + 0x120, 1);
}

/// `FE3DText_SetStringId` 0x2b0f68(w, id): the box cleared, the id's string loaded
pub fn set_string_id(m: &mut Mem, e: &mut dyn FeEnv, w: u32, id: u32) {
    e.call(m, TB_CLEAR, &[w + 0x12c]);
    load_string_id(m, e, w, id);
    m.w32(w + 0x124, id);
    m.w32(w + 0x120, 1);
}

/// `FE3DText_SetText` 0x2b0fb8(w, s): the box cleared, the narrow text laid out
pub fn set_text(m: &mut Mem, e: &mut dyn FeEnv, w: u32, s: u32) {
    e.call(m, TB_CLEAR, &[w + 0x12c]);
    layout(m, e, w, s);
    m.w32(w + 0x120, 0);
    m.w32(w + 0x124, u32::MAX);
}

/// `FE3DText_SetupTextBox` 0x2b1008(w, s): the box's font and rectangle, then the wide text added
pub fn setup_text_box(m: &mut Mem, e: &mut dyn FeEnv, w: u32, s: u32) {
    let font = m.r32(w + 0x110);
    e.call(m, TB_SET_FONT, &[w + 0x12c, font]);
    set_rect(m, e, w);
    e.call(m, TB_APPEND, &[w + 0x12c, s]);
}

/// `FE3DText_LoadStringId` 0x2b1070(w, id): the id's string set up, the id kept, +0x120 1
pub fn load_string_id(m: &mut Mem, e: &mut dyn FeEnv, w: u32, id: u32) {
    let s = loc(m, e, id);
    setup_text_box(m, e, w, s);
    m.w32(w + 0x124, id);
    m.w32(w + 0x120, 1);
}

/// `FE3DText_Layout` 0x2b10c8(w, s): the box's font and rectangle, no id, then the narrow text added
pub fn layout(m: &mut Mem, e: &mut dyn FeEnv, w: u32, s: u32) {
    let font = m.r32(w + 0x110);
    e.call(m, TB_SET_FONT, &[w + 0x12c, font]);
    set_rect(m, e, w);
    m.w32(w + 0x120, 0);
    m.w32(w + 0x124, u32::MAX);
    e.call(m, TB_APPEND_NARROW, &[w + 0x12c, s]);
}

/// `cFE3DWText` vf24 0x2b4cb8: 1
pub fn wtext_vf24() -> u32 {
    1
}

/// `FE3DText_SetScale` 0x2b4cc0(w; f12): the box's line height
pub fn set_scale(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: F) {
    e.call(m, TB_SET_LINE_HEIGHT, &[w + 0x12c, v.0.to_bits()]);
}

/// `FE3DText_ApplyScale` 0x2b4cc8(w; f12): the box's scale, with its +0x14 as the second
pub fn apply_scale(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: F) {
    let b = w + 0x12c;
    let second = m.r32(b + 0x14);
    e.call(m, TB_SET_SCALE, &[b, v.0.to_bits(), second]);
}

/// `cFE3DWText` vf13 0x2b4ce8(w, v): the position (+0x80) moved by the vector's x, y and z, the box's rectangle set
pub fn wtext_vf13(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: u32) {
    let at = v & !0xf;
    let add = [rf(m, at), rf(m, at + 4), rf(m, at + 8), F(0.0)];
    let to = (w + 0x80) & !0xf;
    for (k, a) in add.into_iter().enumerate() {
        let p = to + 4 * k as u32;
        let s = rf(m, p) + a;
        wf(m, p, s);
    }
    set_rect(m, e, w);
}

/// `cFE3DWText` vf38 0x2b4d38(w, v): the size (+0x100) from the vector, the box's rectangle set and its lines
/// wrapped
pub fn wtext_vf38(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: u32) {
    quad(m, w + 0x100, v);
    set_rect(m, e, w);
    let mode = m.r32(w + 0x11c);
    e.call(m, TB_WRAP, &[w + 0x12c, mode]);
}

/// `cFE3DWText` vf39 0x2b4d90(w): the box's rectangle set and its lines wrapped
pub fn wtext_vf39(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    set_rect(m, e, w);
    let mode = m.r32(w + 0x11c);
    e.call(m, TB_WRAP, &[w + 0x12c, mode]);
}

/// `cFE3DWText` vf9 0x2b4df0(w, c): the colour (+0x60) from `c`, and the box's (+0x168) from it
pub fn wtext_vf9(m: &mut Mem, w: u32, c: u32) {
    for k in 0..4 {
        m.w32(w + 0x60 + 4 * k, m.r32(c + 4 * k));
    }
    for k in 0..4 {
        m.w32(w + 0x168 + 4 * k, m.r32(w + 0x60 + 4 * k));
    }
}

/// `cFE3DWText::~cFE3DWText` 0x2b4ed0(w, flags): its table, the box destroyed, then the widget's destructor
pub fn wtext_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, WTEXT_VTABLE);
    e.call(m, TB_DELETE, &[w + 0x12c, 2]);
    destroy(m, e, w, flags)
}

// ---- the buffered single-line text

/// `cFE3DWBufferedSingleLineText` vf40 0x2b1650(t, s): the wide string copied to +0x130, and set from there
pub fn buffered_set_wide(m: &mut Mem, e: &mut dyn FeEnv, t: u32, s: u32) {
    e.call(m, WSTR_COPY, &[t + 0x130, s]);
    text_set_wide(m, t, t + 0x130);
}

/// `cFE3DWBufferedSingleLineText` vf41 0x2b1690(t, id): the buffer emptied, then the id's string set
pub fn buffered_set_id(m: &mut Mem, e: &mut dyn FeEnv, t: u32, id: u32) {
    m.w16(t + 0x130, 0);
    text_set_id(m, e, t, id);
}

/// `cFE3DWBufferedSingleLineText` vf42 0x2b16a0(t, s): the narrow string copied to +0x130, and set from there
pub fn buffered_set_narrow(m: &mut Mem, e: &mut dyn FeEnv, t: u32, s: u32) {
    e.call(m, STRCPY, &[t + 0x130, s]);
    text_set_narrow(m, t, t + 0x130);
}

/// `cFE3DWBufferedSingleLineText` vf44 0x2b4940 and `cFE3DWColouredText` vf25 0x2b4828: 1
pub fn text_one() -> u32 {
    1
}

/// `cFE3DWBufferedSingleLineText::~cFE3DWBufferedSingleLineText` 0x2b4948 and the coloured text's 0x2b1758: the
/// class's table, then the single-line text's destructor
pub fn derived_dtor(m: &mut Mem, e: &mut dyn FeEnv, t: u32, vtable: u32, flags: u32) -> u32 {
    m.w32(t + 0xc, vtable);
    text_destroy(m, e, t, flags)
}

/// `cFE3DWBufferedSingleLineText::cFE3DWBufferedSingleLineText` 0x2b4998 and the coloured text's 0x2b48b0: the
/// single-line text with no owner, parent or font, then the class's table
pub fn derived_ctor(m: &mut Mem, e: &mut dyn FeEnv, t: u32, vtable: u32) -> u32 {
    text_construct(m, e, t, 0, 0, 0);
    m.w32(t + 0xc, vtable);
    t
}

// ---- the coloured text

/// 0x2b1a68 (the coloured text's split, +0x140): where the string's first space is, or -1 (for no string, an empty
/// one, or no space)
pub fn coloured_find_split(m: &mut Mem, t: u32) {
    m.w32(t + 0x140, u32::MAX);
    let s = m.r32(t + 0x124);
    let wide = m.r32(t + 0x11c) != 0;
    let ch = |m: &Mem, at: u32| if wide { m.r16(at) as u32 } else { m.r8(at) as u32 };
    if s == 0 || ch(m, s) == 0 {
        return;
    }
    let step = if wide { 2 } else { 1 };
    let mut at = s;
    let mut n = u32::MAX;
    while ch(m, at) != 0 {
        n = n.wrapping_add(1);
        m.w32(t + 0x140, n);
        if ch(m, at) == 0x20 {
            return;
        }
        at = at.wrapping_add(step);
    }
    m.w32(t + 0x140, u32::MAX);
}

/// `cFE3DWColouredText` vf11 0x2b1770 (drawn; `buf` 0x200 bytes): while shown, with the renderer's state raised by
/// +0x70: when the split is past the start, the string up to it copied to `buf` and drawn in the second colour
/// (+0x130), x moved on by its width, the colour back to +0x60; else the colour +0x130. Then the string after the
/// split (the whole of it for none) at x
pub fn coloured_draw(m: &mut Mem, e: &mut dyn FeEnv, t: u32, buf: u32) {
    if m.r32(t + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(t + 0x70);
    let mut x = rf(m, t + 0x80);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let draw = |m: &mut Mem, e: &mut dyn FeEnv, s: u32, x: F| {
        let wide = m.r32(t + 0x11c) != 0;
        let border = m.r32(t + 0x114) != 0;
        let font = m.r32(t + 0x110);
        let (scale, y) = (m.r32(t + 0x118), m.r32(t + 0x84));
        let x = x.0.to_bits();
        match (wide, border) {
            (true, true) => e.call(m, DRAW_WIDE_BORDER, &[font, s, scale, x, y]),
            (true, false) => e.call(m, DRAW_WIDE, &[font, s, x, y, scale]),
            (false, true) => e.call(m, DRAW_NARROW_BORDER, &[font, s, scale, x, y]),
            (false, false) => e.call(m, DRAW_NARROW, &[font, s, x, y, scale]),
        };
    };
    if (m.r32(t + 0x140) as i32) > 0 {
        e.call(m, DRAWER_COLOUR, &[DRAWER, t + 0x130]);
        let wide = m.r32(t + 0x11c) != 0;
        let (s, n) = (m.r32(t + 0x124), m.r32(t + 0x140));
        if wide {
            e.call(m, WSTR_COPY_N, &[buf, s, n]);
            let n = m.r32(t + 0x140);
            m.w16(buf.wrapping_add(n << 1), 0);
        } else {
            e.call(m, STRNCPY, &[buf, s, n]);
            let n = m.r32(t + 0x140);
            m.w8(buf.wrapping_add(n), 0);
        }
        draw(m, e, buf, x);
        let font = m.r32(t + 0x110);
        let scale = m.r32(t + 0x118);
        let f = if wide { WIDTH_WIDE } else { WIDTH_NARROW };
        let width = e.call(m, f, &[font, buf, 0, scale]);
        x = x + fl(width);
        e.call(m, DRAWER_COLOUR, &[DRAWER, t + 0x60]);
    } else {
        e.call(m, DRAWER_COLOUR, &[DRAWER, t + 0x130]);
    }
    let (s, n) = (m.r32(t + 0x124), m.r32(t + 0x140));
    let rest = if m.r32(t + 0x11c) != 0 {
        s.wrapping_add(n << 1).wrapping_add(2)
    } else {
        s.wrapping_add(n).wrapping_add(1)
    };
    draw(m, e, rest, x);
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DWColouredText` vf12 0x2b19f8: the single-line text's
pub fn coloured_vf12(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    text_vf12(m, e, t);
}

/// `cFE3DWColouredText` vf37 0x2b1a00(t, out): the single-line text's size, less a space's narrow width when split
pub fn coloured_size(m: &mut Mem, e: &mut dyn FeEnv, t: u32, out: u32) {
    text_size(m, e, t, out);
    if (m.r32(t + 0x140) as i32) <= 0 {
        return;
    }
    let font = m.r32(t + 0x110);
    let scale = m.r32(t + 0x118);
    let w = e.call(m, WIDTH_NARROW, &[font, SPACE_TEXT, 0, scale]);
    let v = rf(m, out) - fl(w);
    wf(m, out, v);
}

/// `cFE3DWColouredText` vf39 0x2b4830(t): the single-line text's, then the split found
pub fn coloured_vf39(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    text_vf39(m, e, t);
    coloured_find_split(m, t);
}

/// 0x2b4858(t, c): the second colour (+0x130) from `c`
pub fn coloured_set_colour(m: &mut Mem, t: u32, c: u32) {
    for k in 0..4 {
        m.w32(t + 0x130 + 4 * k, m.r32(c + 4 * k));
    }
}

// ---- the scrolling text

fn slot(t: u32, i: u32) -> u32 {
    (t + 0x110).wrapping_add(i << 2)
}

/// A line's height: the font's (+0xc) times the scale, and the gap
fn line_height(m: &Mem, t: u32) -> F {
    let font = m.r32(t + 0x188);
    fl(m.r32(font.wrapping_add(0xc))) * rf(m, t + 0x18c) + rf(m, t + 0x1ac)
}

/// `cFE3DScrollingText::cFE3DScrollingText` 0x2b1b20(t, owner, parent, font): the widget, then the font, scale 1,
/// centred, from id 0, rolling up at 0.3 with a gap of 3, 10 lines, a marked line's border; the line slots filled
/// with -1 (0x78 bytes) and cleared
pub fn scroll_ctor(m: &mut Mem, e: &mut dyn FeEnv, t: u32, owner: u32, parent: u32, font: u32) -> u32 {
    widget_init(m, e, t, owner, parent);
    m.w32(t + 0x188, font);
    m.w32(t + 0x18c, ONE);
    m.w32(t + 0x1a4, 0);
    m.w32(t + 0x1b8, 1);
    for o in [0x194u32, 0x198, 0x19c, 0x1a0, 0x1b4] {
        m.w32(t + o, 0);
    }
    m.w32(t + 0xc, SCROLL_VTABLE);
    m.w32(t + 0x190, 2);
    m.w32(t + 0x1a8, 0x3e99_999a);
    m.w32(t + 0x1ac, 3.0f32.to_bits());
    m.w32(t + 0x1b0, 10);
    e.call(m, FILL, &[t + 0x110, u32::MAX, 0x78]);
    clear_lines(m, t);
    t
}

/// `cFE3DScrollingText` vf39 0x2b1d60(t): back to the first id, as many lines as fit the height (+0x104), cleared
pub fn scroll_vf39(m: &mut Mem, t: u32) {
    let h = line_height(m, t);
    let first = m.r32(t + 0x198);
    m.w32(t + 0x194, first);
    let n = int(rf(m, t + 0x104) / h);
    m.w32(t + 0x1b0, n);
    clear_lines(m, t);
}

/// `FE3DScrollingText_AddLine` 0x2b1da0(t, id): the id put in the first empty slot from the last one on (onward,
/// past the last, until one is found or the index turns negative)
pub fn add_line(m: &mut Mem, t: u32, id: u32) {
    let mut i = m.r32(t + 0x1b0).wrapping_sub(1);
    while (i as i32) >= 0 {
        if m.r32(slot(t, i)) == u32::MAX {
            m.w32(slot(t, i), id);
            return;
        }
        i = i.wrapping_add(1);
    }
}

/// `FE3DScrollingText_ShiftUp` 0x2b1de8(t): every line up one, the last slot emptied
pub fn shift_up(m: &mut Mem, t: u32) {
    let mut i = 1u32;
    while (i as i32) < m.r32(t + 0x1b0) as i32 {
        let v = m.r32(slot(t, i));
        m.w32(slot(t, i - 1), v);
        i += 1;
    }
    let last = m.r32(t + 0x1b0).wrapping_sub(1);
    m.w32(slot(t, last), u32::MAX);
}

/// `FE3DScrollingText_ShiftDown` 0x2b1e58(t): every line down one, the first slot emptied
pub fn shift_down(m: &mut Mem, t: u32) {
    let mut i = m.r32(t + 0x1b0).wrapping_sub(1);
    while (i as i32) > 0 {
        let v = m.r32(slot(t, i - 1));
        m.w32(slot(t, i), v);
        i -= 1;
    }
    m.w32(t + 0x110, u32::MAX);
}

/// `FE3DScrollingText_CountLines` 0x2b1ea0(t): the slots that hold a line
pub fn count_lines(m: &Mem, t: u32) -> u32 {
    let n = m.r32(t + 0x1b0) as i32;
    (0..n.max(0) as u32).filter(|&i| m.r32(slot(t, i)) != u32::MAX).count() as u32
}

/// `FE3DScrollingText_Advance` 0x2b1ee8(t): with room and the next id within the first and last, it is added and
/// moved on (up, or down when rolling down); once no lines are left, back to the first id (the last rolling down)
/// and the slots cleared
pub fn advance(m: &mut Mem, t: u32) {
    if (count_lines(m, t) as i32) < m.r32(t + 0x1b0) as i32 {
        let cur = m.r32(t + 0x194) as i32;
        if cur >= m.r32(t + 0x198) as i32 && cur <= m.r32(t + 0x19c) as i32 {
            let next = if m.r32(t + 0x1a0) == 0 { cur + 1 } else { cur - 1 };
            m.w32(t + 0x194, next as u32);
            add_line(m, t, cur as u32);
        }
    }
    if count_lines(m, t) != 0 {
        return;
    }
    let first = if m.r32(t + 0x1a0) != 0 { m.r32(t + 0x19c) } else { m.r32(t + 0x198) };
    m.w32(t + 0x194, first);
    clear_lines(m, t);
}

/// `FE3DScrollingText_ClearLines` 0x2b1fa8(t): every slot emptied
pub fn clear_lines(m: &mut Mem, t: u32) {
    let mut i = 0u32;
    while (i as i32) < m.r32(t + 0x1b0) as i32 {
        m.w32(slot(t, i), u32::MAX);
        i += 1;
    }
}

/// `FE3DScrollingText_SetSpeed` 0x2b1ff0(t; f12): the speed, no less than 0.1 and no more than 0.35 of the font's
/// height
pub fn set_speed(m: &mut Mem, t: u32, v: F) {
    let font = m.r32(t + 0x188);
    let top = fl(m.r32(font.wrapping_add(0xc))) * F(f32::from_bits(0x3eb3_3333));
    let low = F(f32::from_bits(0x3dcc_cccd));
    let v = if lt(v, low) {
        low
    } else if lt(top, v) {
        top
    } else {
        v
    };
    wf(m, t + 0x1a8, v);
}

/// `cFE3DScrollingText` vf38 0x2b2048(t, v): as many lines as fit the vector's y, the size (+0x100) from it
pub fn scroll_vf38(m: &mut Mem, t: u32, v: u32) {
    let h = line_height(m, t);
    let n = int(rf(m, v + 4) / h);
    m.w32(t + 0x1b0, n);
    quad(m, t + 0x100, v);
}

/// `cFE3DScrollingText` vf11 0x2b2080 (drawn; `buf` 0x10 bytes): while shown, with the renderer's state raised by
/// +0x70, each slot's line (its string by id; a string starting with the hiding character is skipped) one line
/// height apart from the position less the roll. The first line fades out and the last fades in as they roll; a
/// line starting with the marking character is drawn without it in the chosen colour (+0xac), the others in the
/// enabled one (+0xbc), each with its border flag; aligned by the font's wide width against the size (+0x100)
pub fn scroll_draw(m: &mut Mem, e: &mut dyn FeEnv, t: u32, buf: u32) {
    if m.r32(t + 0x14) == 0 {
        return;
    }
    let mut font = m.r32(t + 0x188);
    let fh = fl(m.r32(font.wrapping_add(0xc))) * rf(m, t + 0x18c);
    let mut y = rf(m, t + 0x84) - rf(m, t + 0x1a4);
    let lh = fh + rf(m, t + 0x1ac);
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(t + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let mut i = 0u32;
    while (i as i32) < m.r32(t + 0x1b0) as i32 {
        let id = m.r32(slot(t, i));
        if id != u32::MAX {
            let s = loc(m, e, id);
            if m.r16(s) != m.r16(HIDE_CHAR) {
                let mut alpha = F(1.0);
                if i == 0 {
                    alpha = F(1.0) - abs(rf(m, t + 0x1a4)) / lh;
                } else if i + 1 == m.r32(t + 0x1b0) {
                    alpha = abs(rf(m, t + 0x1a4)) / fl(m.r32(font.wrapping_add(0xc)));
                }
                if lt(alpha, F(0.0)) {
                    alpha = F(0.0);
                } else if le(F(1.0), alpha) {
                    alpha = F(1.0);
                }
                if m.r32(t + 0x1a0) == 1 {
                    alpha = F(1.0) - alpha;
                }
                let s2 = loc(m, e, m.r32(slot(t, i)));
                let marked = m.r16(s2) == m.r16(MARK_CHAR);
                let mut s = loc(m, e, m.r32(slot(t, i)));
                if marked {
                    font = m.r32(t + 0x188);
                    s = s.wrapping_add(2);
                }
                let mut x = rf(m, t + 0x80);
                let align = m.r32(t + 0x190);
                if align == 2 || align == 1 {
                    let scale = m.r32(t + 0x18c);
                    let w = fl(e.call(m, WIDTH_WIDE, &[font, s, 0, scale]));
                    let room = rf(m, t + 0x100) - w;
                    x = x + if align == 2 { room * F(0.5) } else { room };
                }
                let (colour, flag) = if marked { (0xac, 0x1b8) } else { (0xbc, 0x1b4) };
                for k in 0..4 {
                    m.w32(buf + 4 * k, m.r32(t + colour + 4 * k));
                }
                wf(m, buf, alpha);
                e.call(m, DRAWER_COLOUR, &[DRAWER, buf]);
                let scale = m.r32(t + 0x18c);
                if m.r32(t + flag) != 0 {
                    e.call(m, DRAW_WIDE_BORDER, &[font, s, scale, x.0.to_bits(), y.0.to_bits()]);
                } else {
                    e.call(m, DRAW_WIDE, &[font, s, x.0.to_bits(), y.0.to_bits(), scale]);
                }
            }
        }
        i += 1;
        y = y + lh;
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DScrollingText` vf12 0x2b2420(t): while shown, rolled on by the speed (down when rolling up, the other way
/// down); once a whole line height has gone by, the lines shifted, the roll brought back by a line, and the next
/// one added; then its +0x48
pub fn scroll_update(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t + 0x14) != 0 {
        let dir = m.r32(t + 0x1a0);
        let off = if dir == 0 {
            rf(m, t + 0x1a4) + rf(m, t + 0x1a8)
        } else {
            rf(m, t + 0x1a4) - rf(m, t + 0x1a8)
        };
        wf(m, t + 0x1a4, off);
        let h = line_height(m, t);
        if lt(h, abs(rf(m, t + 0x1a4))) {
            if dir == 0 {
                shift_up(m, t);
            } else {
                shift_down(m, t);
            }
            let off = rf(m, t + 0x1a4);
            let off = if lt(F(0.0), off) { off - h } else { off + h };
            wf(m, t + 0x1a4, off);
            advance(m, t);
        }
    }
    vcall(m, e, t, 0x48, &[]);
}

/// `cFE3DScrollingText` vf28 0x2b2510(t, input): up speeds the roll up (slows it rolling down), down the other way,
/// by 0.1; 1 when either was pressed
pub fn scroll_on_input(m: &mut Mem, e: &mut dyn FeEnv, t: u32, input: u32) -> u32 {
    let faster = if e.call(m, INPUT_UP, &[input]) != 0 {
        m.r32(t + 0x1a0) == 0
    } else if e.call(m, INPUT_DOWN, &[input]) != 0 {
        m.r32(t + 0x1a0) == 1
    } else {
        return 0;
    };
    let step = F(f32::from_bits(0x3dcc_cccd));
    let v = rf(m, t + 0x1a8);
    set_speed(m, t, if faster { v + step } else { v - step });
    1
}

/// `cFE3DScrollingText` vf37 0x2b26c8(t, out): its size (+0x100)
pub fn scroll_size(m: &mut Mem, t: u32, out: u32) {
    quad(m, out, t + 0x100);
}

/// `cFE3DScrollingText::~cFE3DScrollingText` 0x2b2750(t, flags): its table, then the widget's destructor
pub fn scroll_dtor(m: &mut Mem, e: &mut dyn FeEnv, t: u32, flags: u32) -> u32 {
    m.w32(t + 0xc, SCROLL_VTABLE);
    destroy(m, e, t, flags)
}

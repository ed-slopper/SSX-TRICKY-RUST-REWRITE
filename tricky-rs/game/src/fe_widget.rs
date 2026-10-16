//! The front end's widget and its single-line text (board row G9b2c) on the game's bytes: `cFE3DWidget`'s virtuals
//! (its tweens' start and step, its colours, bounds and small accessors) and `cFE3DWSingleLineText` (the constructor
//! the screens call as 0x2b1138, its draw, size and string setters), every call out through [`FeEnv`]; checked in the
//! function runner (`tests/fe_widget.rs`).
//!
//! A thing's tween (+0x18 running, +0x1c its start taken, +0x20 the track, +0x24 the frame, +0x28 the frames, +0x2c
//! the mode: 0 once, 2 back and forth; +0x30 running backwards; +0x40 the start, +0x50 the end) moves one of: 1 the
//! position (+0x80), 2 the position by an offset, 3 the size, 4 the size by a factor, 5 the colour (+0x60). A widget's
//! colour comes from its flags: +0xa0 chosen, +0xa8 enabled; its colour sets are +0xac (chosen), +0xbc (enabled),
//! +0xcc (disabled) and +0xdc (chosen while disabled).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_helpers::tween_start;
use crate::fe_venuesel::LOC_STRING;
use crate::fe_widgets::{widget_init, UPDATE_COLOR, WIDGET_VTABLE};
use crate::pf::Mem;
use crate::ps2f::F;

/// The single-line text's vtable
pub const TEXT_VTABLE: u32 = 0x3b73a8;
/// The calls: the thing's destructor (0x2acd10: the thing, the flag); the drawer's colour (0x14eee8: the drawer at
/// 0x337fe8, the colour); a font's text drawn (wide with a border 0x19d568, wide 0x19d0c0, narrow with a border
/// 0x19d3f0, narrow 0x19cf70: the font, the string, f12.. the scale and position) and measured (wide 0x19dc78, narrow
/// 0x19da30: the font, the string, 0, f12 the scale); the widget event's record (0x347e10)
pub const THING_DTOR: u32 = 0x2acd10;
pub const DRAWER: u32 = 0x337fe8;
pub const DRAWER_COLOUR: u32 = 0x14eee8;
pub const DRAW_WIDE_BORDER: u32 = 0x19d568;
pub const DRAW_WIDE: u32 = 0x19d0c0;
pub const DRAW_NARROW_BORDER: u32 = 0x19d3f0;
pub const DRAW_NARROW: u32 = 0x19cf70;
pub const WIDTH_WIDE: u32 = 0x19dc78;
pub const WIDTH_NARROW: u32 = 0x19da30;
pub const EVENT: u32 = 0x347e10;

const ONE: u32 = 0x3f80_0000;

fn pf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// 0x2ac288 (the thing's tween start from where it is; `buf` 0x10 bytes): unless taken (+0x1c), by the track: 1 and
/// 2 the position (+0x80), 3 and 6 zero, 4 (1, 1, 1, 0), 5 the colour (+0x60)
pub fn thing_start_from_current(m: &mut Mem, w: u32, buf: u32) {
    if m.r32(w + 0x1c) != 0 {
        return;
    }
    let track = m.r32(w + 0x20);
    match track {
        1 | 2 => tween_start(m, w, track, w + 0x80),
        3 | 6 => {
            put4(m, buf, [0; 4]);
            tween_start(m, w, track, buf);
        }
        4 => {
            put4(m, buf, [ONE, ONE, ONE, 0]);
            tween_start(m, w, track, buf);
        }
        5 => tween_start(m, w, track, w + 0x60),
        _ => {}
    }
}

/// `cFE3DWidget` vf5 0x2ad250 (`buf` 0x10 bytes): the tween's start from where the widget is: for the size tracks
/// (3, 4) its size (+0x130), else as the thing's
pub fn vf5(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    let track = m.r32(w + 0x20) as i32;
    if (3..5).contains(&track) {
        vcall(m, e, w, 0x130, &[buf]);
        let track = m.r32(w + 0x20);
        tween_start(m, w, track, buf);
    } else {
        thing_start_from_current(m, w, buf);
    }
}

/// `cFE3DWidget` vf7 0x2ad2c0 (the tween started): an offset (2) becomes a move to the start + offset, a factor (4) a
/// size of start × factor; then running (+0x18), from frame 0 forwards, and its first step (+0x48). For track 2 the
/// end's w (+0x5c) takes the VU register's leftover w (STANDIN: kept as it was)
pub fn vf7(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    match m.r32(w + 0x20) {
        2 => {
            m.w32(w + 0x20, 1);
            for k in 0..3 {
                let v = pf(m, w + 0x50 + 4 * k) + pf(m, w + 0x40 + 4 * k);
                m.w32(w + 0x50 + 4 * k, v.0.to_bits());
            }
        }
        4 => {
            for k in 0..3 {
                let v = pf(m, w + 0x50 + 4 * k) * pf(m, w + 0x40 + 4 * k);
                m.w32(w + 0x50 + 4 * k, v.0.to_bits());
            }
            m.w32(w + 0x20, 3);
        }
        0 | 1 | 3 | 5 => {}
        _ => return,
    }
    m.w32(w + 0x18, 1);
    m.w32(w + 0x24, 0);
    m.w32(w + 0x30, 0);
    vcall(m, e, w, 0x48, &[]);
}

/// The tween's blend of two vectors (x, y, z) as the VU makes it: `a` × (1 - t) + `b` × t
fn blend(m: &Mem, a: u32, b: u32, t: F) -> [F; 3] {
    let s = F(1.0) - t;
    let mut v = [F(0.0); 3];
    for (k, x) in v.iter_mut().enumerate() {
        let k = k as u32;
        *x = pf(m, a + 4 * k) * s + pf(m, b + 4 * k) * t;
    }
    v
}

/// A value kept in 0..1 as the code's two `c.olt.s` tests keep it (a NaN passes)
fn clamp01(v: F) -> F {
    if v.ee() < 0.0 {
        F(0.0)
    } else if 1.0 < v.ee() {
        F(1.0)
    } else {
        v
    }
}

/// `cFE3DWidget` vf8 0x2ad380 (the tween's step; `buf` 0x50 bytes): while shown (+0x10) with a track: its start taken
/// first (+0x30, then +0x40) or, not running, started (+0x40); the step's fraction (frame / frames, from the end when
/// running backwards): the position moved to the blend (1), the size set to it (3), or the colour set to each part's
/// blend kept in 0..1 (5); then the next frame, and at the end frame 0 and the mode's end (0: hidden (+0x20(0)); 2:
/// the direction turned). The size's w is the VU register's leftover; the test leaves it out.
pub fn vf8(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    if m.r32(w + 0x10) == 0 || m.r32(w + 0x20) == 0 {
        return;
    }
    if m.r32(w + 0x1c) == 0 {
        vcall(m, e, w, 0x30, &[]);
        vcall(m, e, w, 0x40, &[]);
    } else if m.r32(w + 0x18) == 0 {
        vcall(m, e, w, 0x40, &[]);
    }
    let track = m.r32(w + 0x20);
    let t = F(m.r32(w + 0x24) as i32 as f32) / F(m.r32(w + 0x28) as i32 as f32);
    let (a, b) = if m.r32(w + 0x30) == 0 { (w + 0x40, w + 0x50) } else { (w + 0x50, w + 0x40) };
    match track {
        1 => {
            let p = blend(m, a, b, t);
            let mut d = [0u32; 4];
            for k in 0..3 {
                d[k] = (p[k] - pf(m, w + 0x80 + 4 * k as u32)).0.to_bits();
            }
            d[3] = ONE;
            put4(m, buf + 0x10, d);
            vcall(m, e, w, 0x70, &[buf + 0x10]);
        }
        3 => {
            vcall(m, e, w, 0x130, &[buf]);
            let v = blend(m, a, b, t);
            put4(m, buf + 0x10, [v[0].0.to_bits(), v[1].0.to_bits(), v[2].0.to_bits(), 0]);
            vcall(m, e, w, 0x138, &[buf + 0x10]);
        }
        5 => {
            let mut c = [0u32; 4];
            for (k, x) in c.iter_mut().enumerate() {
                let k = k as u32;
                let (p, q) = (pf(m, a + 4 * k), pf(m, b + 4 * k));
                *x = clamp01(p + (q - p) * t).0.to_bits();
            }
            put4(m, buf, c);
            vcall(m, e, w, 0x50, &[buf]);
        }
        _ => return,
    }
    let frame = m.r32(w + 0x24).wrapping_add(1);
    m.w32(w + 0x24, frame);
    if !((m.r32(w + 0x28) as i32) < frame as i32) {
        return;
    }
    m.w32(w + 0x24, 0);
    match m.r32(w + 0x2c) {
        0 => {
            vcall(m, e, w, 0x20, &[0]);
        }
        2 => {
            let back = (m.r32(w + 0x30) == 0) as u32;
            m.w32(w + 0x30, back);
        }
        _ => {}
    }
}

/// `cFE3DWidget` vf15 0x2ad7d8: 1
pub fn vf15() -> u32 {
    1
}

/// 0x2ad7e0: the widget's owner (+0xec)
pub fn owner(m: &Mem, w: u32) -> u32 {
    m.r32(w + 0xec)
}

/// 0x2ad7e8 (the widget, f12..f15; `buf` 0x10 bytes): its size (+0x138) from four floats
pub fn size_from(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: [u32; 4], buf: u32) {
    put4(m, buf, v);
    vcall(m, e, w, 0x138, &[buf]);
}

/// `cFE3DWidget` vf38 0x2ad828 / vf37 0x2ad838: the vector at +0x100 set from / copied to `v` (quadword moves)
pub fn vf38(m: &mut Mem, w: u32, v: u32) {
    for k in 0..4 {
        let x = m.r32((v & !0xf) + 4 * k);
        m.w32(((w + 0x100) & !0xf) + 4 * k, x);
    }
}
pub fn vf37(m: &mut Mem, w: u32, v: u32) {
    for k in 0..4 {
        let x = m.r32(((w + 0x100) & !0xf) + 4 * k);
        m.w32((v & !0xf) + 4 * k, x);
    }
}

/// `cFE3DWidget` vf36 0x2ad848 (the widget, the low corner, the high corner; `buf` 0x10 bytes): its position, and its
/// position + its size (+0x130)
pub fn vf36(m: &mut Mem, e: &mut dyn FeEnv, w: u32, lo: u32, hi: u32, buf: u32) {
    for k in 0..3 {
        let v = m.r32(w + 0x80 + 4 * k);
        m.w32(lo + 4 * k, v);
    }
    vcall(m, e, w, 0x130, &[buf]);
    for k in 0..3 {
        let v = pf(m, w + 0x80 + 4 * k) + pf(m, buf + 4 * k);
        m.w32(hi + 4 * k, v.0.to_bits());
    }
}

/// `cFE3DWidget` vf32..vf35 (0x2ad960, 0x2ad938, 0x2ad910, 0x2ad8e8): a colour set (+0xac, +0xbc, +0xcc, +0xdc) from
/// four words
pub fn set_colour_set(m: &mut Mem, w: u32, at: u32, v: u32) {
    for k in 0..4 {
        let x = m.r32(v.wrapping_add(4 * k));
        m.w32(w + at + 4 * k, x);
    }
}

/// 0x2ad988 (`FE3DWidget_UpdateColor`): unless a colour tween is running, the colour (+0x50) from the flags: chosen
/// and enabled +0xac, chosen +0xdc, enabled +0xbc, else +0xcc
pub fn update_colour(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    if m.r32(w + 0x10) != 0 && m.r32(w + 0x20) == 5 {
        return;
    }
    let enabled = m.r32(w + 0xa8) != 0;
    let set = if m.r32(w + 0xa0) != 0 {
        if enabled {
            0xac
        } else {
            0xdc
        }
    } else if enabled {
        0xbc
    } else {
        0xcc
    };
    vcall(m, e, w, 0x50, &[w + set]);
}

/// `cFE3DWidget` vf31 / vf30 / vf29 (0x2ada20, 0x2ada28, 0x2ada30): +0xa8 / +0xa4 / +0xa0 set, then the colour
pub fn set_flag(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, v: u32) {
    m.w32(w + at, v);
    e.call(m, UPDATE_COLOR, &[w]);
}

/// 0x2ada38 / 0x2ada40 / 0x2ada48 / 0x2ada50: +0xa8, +0xa4, +0xa0, the parent +0xf0
pub fn flag(m: &Mem, w: u32, at: u32) -> u32 {
    m.r32(w + at)
}

/// 0x2ada58 (the widget, a kind, a value): the event record (0x347e10: +0 the kind, +0x40 the value, +0x3c 1) handed
/// to the widget's +0xe8
pub fn post_event(m: &mut Mem, e: &mut dyn FeEnv, w: u32, kind: u32, v: u32) {
    m.w32(EVENT, kind);
    m.w32(EVENT + 0x40, v);
    m.w32(EVENT + 0x3c, 1);
    vcall(m, e, w, 0xe8, &[EVENT]);
}

/// `cFE3DWidget` vf28 0x2adaa0: 0
pub fn vf28() -> u32 {
    0
}

/// `cFE3DWidget::~cFE3DWidget` 0x2adaa8: the widget's vtable, then the thing's destructor
pub fn destroy(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flag: u32) -> u32 {
    m.w32(w + 0xc, WIDGET_VTABLE);
    e.call(m, THING_DTOR, &[w, flag])
}

/// 0x2b1138 (`cFE3DWSingleLineText::cFE3DWSingleLineText`; the text, its owner, its parent, its font): the widget,
/// then the font (+0x110), a border (+0x114 1), the text's vtable, its scale (+0x118 1.0), no text id (+0x120 -1),
/// narrow (+0x11c 0) and no string (+0x124)
pub fn text_construct(m: &mut Mem, e: &mut dyn FeEnv, t: u32, owner: u32, parent: u32, font: u32) -> u32 {
    widget_init(m, e, t, owner, parent);
    m.w32(t + 0x110, font);
    m.w32(t + 0x114, 1);
    m.w32(t + 0xc, TEXT_VTABLE);
    m.w32(t + 0x118, ONE);
    m.w32(t + 0x120, u32::MAX);
    m.w32(t + 0x11c, 0);
    m.w32(t + 0x124, 0);
    t
}

/// `cFE3DWSingleLineText::~cFE3DWSingleLineText` 0x2b1328: its vtable, then the widget's destructor
pub fn text_destroy(m: &mut Mem, e: &mut dyn FeEnv, t: u32, flag: u32) -> u32 {
    m.w32(t + 0xc, TEXT_VTABLE);
    destroy(m, e, t, flag)
}

/// `cFE3DWSingleLineText` vf39 0x2b1340 (laid out): a wide text from an id fetched again (+0x150)
pub fn text_vf39(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t + 0x11c) == 0 {
        return;
    }
    let id = m.r32(t + 0x120);
    if (id as i32) < 0 {
        return;
    }
    vcall(m, e, t, 0x150, &[id]);
}

/// The renderer (gApp+0x724; its vtable at +0x10)
fn renderer_call(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// `cFE3DWSingleLineText` vf11 0x2b1388 (drawn): while shown (+0x14), the renderer's state (+0x1d8) raised by the
/// widget's +0x70 (+0x1d0), the drawer's colour (the widget's +0x60), the string drawn by the font (wide or narrow,
/// with or without the border: with it the scale first, else the position first), the state put back
pub fn text_draw(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t + 0x14) == 0 {
        return;
    }
    let state = renderer_call(m, e, 0x1d8, &[]);
    let raise = m.r32(t + 0x70);
    renderer_call(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    e.call(m, DRAWER_COLOUR, &[DRAWER, t + 0x60]);
    let (font, s) = (m.r32(t + 0x110), m.r32(t + 0x124));
    let (scale, x, y) = (m.r32(t + 0x118), m.r32(t + 0x80), m.r32(t + 0x84));
    let border = m.r32(t + 0x114) != 0;
    let wide = m.r32(t + 0x11c) != 0;
    let f = match (wide, border) {
        (true, true) => DRAW_WIDE_BORDER,
        (true, false) => DRAW_WIDE,
        (false, true) => DRAW_NARROW_BORDER,
        (false, false) => DRAW_NARROW,
    };
    if border {
        e.call(m, f, &[font, s, scale, x, y]);
    } else {
        e.call(m, f, &[font, s, x, y, scale]);
    }
    renderer_call(m, e, 0x1d0, &[state]);
}

/// `cFE3DWSingleLineText` vf12 0x2b14b8: its +0x48
pub fn text_vf12(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    vcall(m, e, t, 0x48, &[]);
}

/// `cFE3DWSingleLineText` vf37 0x2b14e0 (its size into `out`): (0, 0, 0, 1) for no string or an empty one; else the
/// font's width of the string at the scale, and the font's height (+0xc) × the scale; 2 more each way with the border
pub fn text_size(m: &mut Mem, e: &mut dyn FeEnv, t: u32, out: u32) {
    // (the first store is a quadword's, at the address taken down to 16; the others are words at `out`)
    put4(m, out & !0xf, [0, 0, 0, ONE]);
    let s = m.r32(t + 0x124);
    let empty = if s == 0 {
        true
    } else if m.r32(t + 0x11c) != 0 {
        m.r16(s) == 0
    } else {
        m.r8(s) == 0
    };
    if empty {
        return;
    }
    let font = m.r32(t + 0x110);
    let scale = m.r32(t + 0x118);
    let f = if m.r32(t + 0x11c) != 0 { WIDTH_WIDE } else { WIDTH_NARROW };
    let w = e.call(m, f, &[font, s, 0, scale]);
    let mut wf = F(w as i32 as f32);
    m.w32(out, wf.0.to_bits());
    let font = m.r32(t + 0x110);
    let mut h = F(m.r32(font.wrapping_add(0xc)) as i32 as f32) * pf(m, t + 0x118);
    m.w32(out + 4, h.0.to_bits());
    if m.r32(t + 0x114) != 0 {
        h = h + F(2.0);
        wf = pf(m, out) + F(2.0);
        m.w32(out + 4, h.0.to_bits());
        m.w32(out, wf.0.to_bits());
    }
}

/// `cFE3DWSingleLineText` vf40 0x2b15d0 (a wide string): the string, no id, wide
pub fn text_set_wide(m: &mut Mem, t: u32, s: u32) {
    m.w32(t + 0x124, s);
    m.w32(t + 0x120, u32::MAX);
    m.w32(t + 0x11c, 1);
}

/// `cFE3DWSingleLineText` vf41 0x2b15e8 (a text id): the id's string (0x2c5160 on gApp+0x50), the id, wide
pub fn text_set_id(m: &mut Mem, e: &mut dyn FeEnv, t: u32, id: u32) {
    let loc = m.r32(m.r32(GAPP).wrapping_add(0x50));
    let s = e.call(m, LOC_STRING, &[loc, id]);
    m.w32(t + 0x120, id);
    m.w32(t + 0x124, s);
    m.w32(t + 0x11c, 1);
}

/// `cFE3DWSingleLineText` vf42 0x2b1638 (a narrow string): the string, no id, narrow
pub fn text_set_narrow(m: &mut Mem, t: u32, s: u32) {
    m.w32(t + 0x124, s);
    m.w32(t + 0x120, u32::MAX);
    m.w32(t + 0x11c, 0);
}

/// `cFE3DWSingleLineText` vf43 0x2b4a28: 1
pub fn text_vf43() -> u32 {
    1
}

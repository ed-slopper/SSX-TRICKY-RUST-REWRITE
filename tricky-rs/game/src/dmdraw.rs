//! The debug menu's drawing (board row G9a4c): the frame of a menu's draw, the boxes and arrows, the labels' measure
//! and draw, the string, colour and RGB items' own, the colours' tables and the global manager, the item kinds'
//! destructors and small setters. On the game's bytes ([`Mem`]); the font, the line, rect and colour calls, the
//! renderer's virtuals and the C library are [`TextBoxEnv`] calls by address. Checked in the function runner
//! (`tests/dmdraw.rs`).
//!
//! A menu's font is its manager's +0x38. The colours (three floats each, filled at start): 0x3cde38 a disabled
//! item's text, 0x3cde48 the picked item's text, 0x3cde58 the others' text, 0x3cde68 a disabled item's box, 0x3cde78
//! the picked item's box (and the arrows'), 0x3cde88 the others' box.
//!
//! Where the game builds on its stack (boxes, colours, a formatted value), the port uses `scratch` (STANDIN: the
//! place, not the bytes).

use crate::animseq::cvt_s_w;
use crate::debugmenu::{item_delete, item_flag8, manager_delete, manager_new, menu_current_item, menu_delete};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const DRAW_COLOUR: u32 = 0x337fe8;
/// The debug menu's manager
pub const MANAGER: u32 = 0x337e08;
pub const TEXT_DISABLED: u32 = 0x3cde38;
pub const TEXT_PICKED: u32 = 0x3cde48;
pub const TEXT: u32 = 0x3cde58;
pub const BOX_DISABLED: u32 = 0x3cde68;
pub const BOX_PICKED: u32 = 0x3cde78;
pub const BOX: u32 = 0x3cde88;
/// The string item's widest value, the colour item's channel names (three strings) and its widest value, "%d"
pub const STRING_WIDEST: u32 = 0x3899c0;
pub const CHANNEL_NAMES: u32 = 0x38a088;
pub const COLOUR_WIDEST: u32 = 0x3899d8;
pub const INT_FORMAT: u32 = 0x389998;

pub const MEASURE_ASCII: u32 = 0x19da30;
pub const PRINT: u32 = 0x19cf70;
pub const LINE: u32 = 0x19c1e8;
pub const SET_RGB: u32 = 0x182040;
pub const SET_VEC4: u32 = 0x14f5f0;
pub const FILLED_RECT: u32 = 0x19b108;
pub const GRADIENT_RECT: u32 = 0x19b2a0;
pub const SPRINTF: u32 = 0x2fca28;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn fcall(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32], f: &[F]) -> u32 {
    env.call(m, func, args, f)
}
fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn cv(v: u32) -> F {
    cvt_s_w(v as i32)
}
fn copy(m: &mut Mem, to: u32, from: u32, words: u32) {
    for i in 0..words {
        m.w32(to + 4 * i, m.r32(from + 4 * i));
    }
}
/// A virtual of the renderer (the app's +0x724)
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32], f: &[F]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let me = r.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, f)
}
const ONE: F = F(1.0);
const HALF: F = F(0.5);

/// 0x18e720(item): its menu's font
pub fn item_font(m: &Mem, item: u32) -> u32 {
    menu_font(m, m.r32(item + 4))
}
/// 0x18e730(menu): its manager's font
pub fn menu_font(m: &Mem, menu: u32) -> u32 {
    m.r32(m.r32(menu + 4) + 0x38)
}
/// 0x18e740(this): its +4 (a menu's manager, an item's menu)
pub fn get4(m: &Mem, this: u32) -> u32 {
    m.r32(this + 4)
}
/// 0x18e788(this) / 0x18e790(this, v): a manager's depth
pub fn get0(m: &Mem, this: u32) -> u32 {
    m.r32(this)
}
pub fn set0(m: &mut Mem, this: u32, v: u32) {
    m.w32(this, v);
}

fn measure(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, s: u32) -> u32 {
    fcall(m, env, MEASURE_ASCII, &[font, s, 0], &[ONE])
}
fn print(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, s: u32, x: u32, y: u32) {
    fcall(m, env, PRINT, &[font, s], &[cv(x), cv(y), ONE]);
}
fn line(m: &mut Mem, env: &mut dyn TextBoxEnv, x0: F, y0: F, x1: F, y1: F) {
    fcall(m, env, LINE, &[DRAW_COLOUR], &[x0, y0, x1, y1]);
}

/// `Color_SetRGBV` 0x181a48(colour, v) (and 0x18df70, the same): the colour's red, green and blue from three floats
pub fn set_rgbv(m: &mut Mem, env: &mut dyn TextBoxEnv, c: u32, v: u32) {
    let f = [rf(m, v), rf(m, v + 4), rf(m, v + 8)];
    fcall(m, env, SET_RGB, &[c, v], &f);
}

/// `Draw_RectI` 0x18df38(colour, rect): a filled rect from four ints
pub fn rect_i(m: &mut Mem, env: &mut dyn TextBoxEnv, c: u32, r: u32) {
    let f = [cv(m.r32(r)), cv(m.r32(r + 4)), cv(m.r32(r + 8)), cv(m.r32(r + 0xc))];
    fcall(m, env, FILLED_RECT, &[c, r], &f);
}

/// `Vec4_SetFromVec3` 0x18df60(colour, v; a): the colour (a, then the three floats)
pub fn vec4_from_vec3(m: &mut Mem, env: &mut dyn TextBoxEnv, c: u32, v: u32, a: F) {
    let f = [a, rf(m, v), rf(m, v + 4), rf(m, v + 8)];
    fcall(m, env, SET_VEC4, &[c, v], &f);
}

/// `DebugMenu_BeginDraw` 0x18c288(menu): the renderer's texture kept in the menu's +0, texture 12, the blend (2, 2;
/// 0.08)
pub fn begin_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, menu: u32) {
    let t = rcall(m, env, 0x1d8, &[], &[]);
    m.w32(menu, t);
    rcall(m, env, 0x1d0, &[0xc], &[]);
    rcall(m, env, 0x1c8, &[2, 2], &[k(0x3da3_d70a)]);
}

/// `DebugMenu_EndDraw` 0x18c318(menu): the texture back, the blend (1, 2; 0.08)
pub fn end_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, menu: u32) {
    rcall(m, env, 0x1d0, &[m.r32(menu)], &[]);
    rcall(m, env, 0x1c8, &[1, 2], &[k(0x3da3_d70a)]);
}

/// `DebugMenu_DrawBox` 0x18c380(rect, colour): on texture 11, untextured, mode 3: the box filled in the colour at
/// 0.8, its top and left edges 1.4 times lighter (inside 0..1), its bottom and right edges 0.7 times, half a pixel in
pub fn draw_box(m: &mut Mem, env: &mut dyn TextBoxEnv, r: u32, c: u32, scratch: u32) {
    let a = k(0x3f4c_cccd);
    rcall(m, env, 0x1d0, &[0xb], &[]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    rcall(m, env, 0x1e0, &[3], &[]);
    let (t, t2) = (scratch, scratch + 0x10);
    copy(m, t, c, 3);
    vec4_from_vec3(m, env, DRAW_COLOUR, t, a);
    rect_i(m, env, DRAW_COLOUR, r);
    let lit = |v: F| {
        let v = v * k(0x3fb3_3333);
        if v.ee() < 0.0 {
            F(0.0)
        } else if 1.0 < v.ee() {
            ONE
        } else {
            v
        }
    };
    for i in 0..3 {
        let v = lit(rf(m, c + 4 * i));
        wf(m, t2 + 4 * i, v);
        wf(m, t + 4 * i, v);
    }
    vec4_from_vec3(m, env, DRAW_COLOUR, t2, a);
    let (x, y, w, h) = (m.r32(r), m.r32(r + 4), m.r32(r + 8), m.r32(r + 0xc));
    let (l, tp) = (cv(x) + HALF, cv(y) + HALF);
    let (rt, b) = (cv(x.wrapping_add(w)) - HALF, cv(y.wrapping_add(h)) - HALF);
    line(m, env, l, tp, rt, tp);
    line(m, env, l, tp, l, b);
    let t3 = scratch + 0x20;
    for i in 0..3 {
        let v = rf(m, c + 4 * i) * k(0x3f33_3333);
        wf(m, t3 + 4 * i, v);
        wf(m, t2 + 4 * i, v);
    }
    vec4_from_vec3(m, env, DRAW_COLOUR, t3, a);
    line(m, env, l, b, rt, b);
    line(m, env, rt, tp, rt, b);
    rcall(m, env, 0x1d0, &[0xc], &[]);
}

fn arrow_box(m: &mut Mem, env: &mut dyn TextBoxEnv, r: u32, scratch: u32) {
    let (rc, c) = (scratch + 0x40, scratch + 0x50);
    copy(m, rc, r, 4);
    copy(m, c, BOX_PICKED, 3);
    draw_box(m, env, rc, c, scratch);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    rcall(m, env, 0x1e0, &[0], &[]);
    set_rgbv(m, env, DRAW_COLOUR, TEXT);
}

/// `DebugMenu_DrawUpArrow` 0x18c728(menu, rect): a picked box with an up arrow drawn twice (4 and 3..5 in)
pub fn draw_up_arrow(m: &mut Mem, env: &mut dyn TextBoxEnv, _menu: u32, r: u32, scratch: u32) {
    arrow_box(m, env, r, scratch);
    let (x, y, w, h) = (m.r32(r), m.r32(r + 4), m.r32(r + 8), m.r32(r + 0xc));
    let mid = cv(x) + cv(w) * HALF;
    let (top, bot, l, rt) = (cv(y), cv(y.wrapping_add(h)), cv(x), cv(x.wrapping_add(w)));
    let (f4, f3, f5) = (F(4.0), F(3.0), F(5.0));
    line(m, env, l + f4, bot - f4, mid, top + f4);
    line(m, env, mid, top + f4, rt - f4, bot - f4);
    line(m, env, l + f4, bot - f5, mid, top + f3);
    line(m, env, mid, top + f3, rt - f4, bot - f5);
}

/// `DebugMenu_DrawDownArrow` 0x18c9d0(menu, rect): the same, pointing down
pub fn draw_down_arrow(m: &mut Mem, env: &mut dyn TextBoxEnv, _menu: u32, r: u32, scratch: u32) {
    arrow_box(m, env, r, scratch);
    let (x, y, w, h) = (m.r32(r), m.r32(r + 4), m.r32(r + 8), m.r32(r + 0xc));
    let mid = cv(x) + cv(w) * HALF;
    let (top, bot, l, rt) = (cv(y), cv(y.wrapping_add(h)), cv(x), cv(x.wrapping_add(w)));
    let (f4, f3, f5) = (F(4.0), F(3.0), F(5.0));
    line(m, env, l + f4, top + f4, mid, bot - f4);
    line(m, env, mid, bot - f4, rt - f4, top + f4);
    line(m, env, l + f4, top + f5, mid, bot - f3);
    line(m, env, mid, bot - f3, rt - f4, top + f5);
}

/// `MenuItem_MeasureLabel` 0x18cc78(out, item, title, label, value): the width: with all three the label's and
/// twice the wider of the title and value, + 40; else the sum of those given, + 20 when the title has a label or
/// value beside it (or, without a title, a label and a value); at least 200, + 20; the height the font's + 4
pub fn measure_label(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, item: u32, title: u32, label: u32, value: u32) -> u32 {
    let font = |m: &Mem| item_font(m, item);
    let w = if title != 0 && label != 0 && value != 0 {
        let wl = measure(m, env, font(m), label);
        let wt = measure(m, env, font(m), title);
        let wv = measure(m, env, font(m), value);
        let s = if (wv as i32) < wt as i32 { title } else { value };
        let w = measure(m, env, font(m), s);
        wl.wrapping_add(w << 1).wrapping_add(0x28)
    } else {
        let mut s = if title != 0 { measure(m, env, font(m), title) } else { 0 };
        if label != 0 {
            s = s.wrapping_add(measure(m, env, font(m), label));
        }
        if value != 0 {
            s = s.wrapping_add(measure(m, env, font(m), value));
        }
        let gap = if title != 0 { label != 0 || value != 0 } else { label != 0 && value != 0 };
        if gap { s.wrapping_add(0x14) } else { s }
    };
    let h = m.r32(font(m) + 0xc);
    let w = if 0xc7 < w as i32 { w } else { 0xc8 };
    m.w32(out, 0);
    m.w32(out + 4, 0);
    m.w32(out + 8, w.wrapping_add(0x14));
    m.w32(out + 0xc, h.wrapping_add(4));
    out
}

/// `DebugMenu_DrawItemBox` 0x18ceb8(item, rect, title, label, value, box colour, text colour): the box, then in the
/// text colour the title 10 in at the left, the label centred, the value 10 in at the right (all a pixel down)
pub fn draw_item_box(
    m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32, r: u32, title: u32, label: u32, value: u32, bc: u32, tc: u32, scratch: u32,
) {
    let (rc, c) = (scratch + 0x60, scratch + 0x70);
    copy(m, rc, r, 4);
    copy(m, c, bc, 3);
    draw_box(m, env, rc, c, scratch);
    set_rgbv(m, env, DRAW_COLOUR, tc);
    let y = m.r32(r + 4).wrapping_add(1);
    if title != 0 {
        let f = item_font(m, item);
        print(m, env, f, title, m.r32(r).wrapping_add(0xa), y);
    }
    if label != 0 {
        let w = measure(m, env, item_font(m, item), label);
        let x = m.r32(r).wrapping_add(((m.r32(r + 8).wrapping_sub(w) as i32) / 2) as u32);
        let f = item_font(m, item);
        print(m, env, f, label, x, y);
    }
    if value != 0 {
        let w = measure(m, env, item_font(m, item), value);
        let x = m.r32(r).wrapping_add(m.r32(r + 8).wrapping_sub(w)).wrapping_sub(0xa);
        let f = item_font(m, item);
        print(m, env, f, value, x, y);
    }
}

/// `MenuItem_DrawLabel` 0x18d0c8(item, rect, title, label, value): its box in the disabled colours (flag 8), else the
/// picked or plain ones
pub fn draw_label(m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32, r: u32, title: u32, label: u32, value: u32, scratch: u32) {
    let picked = menu_current_item(m, env, m.r32(item + 4)) == item;
    let (rc, bc, tc) = (scratch + 0x80, scratch + 0x90, scratch + 0xa0);
    copy(m, rc, r, 4);
    let (b, t) = if item_flag8(m, item) != 0 {
        (BOX_DISABLED, TEXT_DISABLED)
    } else if picked {
        (BOX_PICKED, TEXT_PICKED)
    } else {
        (BOX, TEXT)
    };
    copy(m, bc, b, 3);
    copy(m, tc, t, 3);
    draw_item_box(m, env, item, rc, title, label, value, bc, tc, scratch);
}

/// `cStringMenuItem::Measure` 0x18d280(out, this): the title + 20 + the widest value, at least 200, + 20
pub fn string_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    let w = measure(m, env, item_font(m, this), m.r32(this + 0x10)).wrapping_add(0x14);
    let w = w.wrapping_add(measure(m, env, item_font(m, this), STRING_WIDEST));
    let w = if 0xc7 < w as i32 { w } else { 0xc8 };
    let h = m.r32(item_font(m, this) + 0xc);
    m.w32(out, 0);
    m.w32(out + 4, 0);
    m.w32(out + 8, w.wrapping_add(0x14));
    m.w32(out + 0xc, h.wrapping_add(4));
    out
}

/// The box and text colours for the picked item or the others
fn colours(picked: bool) -> (u32, u32) {
    if picked { (BOX_PICKED, TEXT_PICKED) } else { (BOX, TEXT) }
}

/// `cStringMenuItem::Draw` 0x18d360(this, rect): its box, the title, the string right-aligned; when picked the cursor
/// underlined (from the string's start to the cursor, to the wider of 2 more and the cursor's character's end)
pub fn string_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, r: u32, scratch: u32) {
    let picked = menu_current_item(m, env, m.r32(this + 4)) == this;
    let (b, t) = colours(picked);
    let (rc, c) = (scratch + 0x60, scratch + 0x70);
    copy(m, rc, r, 4);
    copy(m, c, b, 3);
    draw_box(m, env, rc, c, scratch);
    let tc = scratch + 0x80;
    copy(m, tc, t, 3);
    set_rgbv(m, env, DRAW_COLOUR, tc);
    let y = m.r32(r + 4).wrapping_add(1);
    let f = item_font(m, this);
    print(m, env, f, m.r32(this + 0x10), m.r32(r).wrapping_add(0xa), y);
    let buf = m.r32(this + 0x14);
    let ws = measure(m, env, item_font(m, this), buf);
    let x = m.r32(r).wrapping_add(m.r32(r + 8).wrapping_sub(ws)).wrapping_sub(0xa);
    let f = item_font(m, this);
    print(m, env, f, m.r32(this + 0x14), x, y);
    let at = m.r32(this + 0x14).wrapping_add(m.r32(this + 0x1c));
    let ch = m.r8(at);
    m.w8(at, 0);
    let a = measure(m, env, item_font(m, this), m.r32(this + 0x14));
    let mut e = a.wrapping_add(2);
    let at = m.r32(this + 0x14).wrapping_add(m.r32(this + 0x1c));
    m.w8(at, ch);
    let at = m.r32(this + 0x1c).wrapping_add(m.r32(this + 0x14));
    let ch = m.r8(at + 1);
    m.w8(at + 1, 0);
    let bw = measure(m, env, item_font(m, this), m.r32(this + 0x14));
    if (e as i32) < bw as i32 {
        e = bw;
    }
    let at = m.r32(this + 0x1c).wrapping_add(m.r32(this + 0x14));
    m.w8(at + 1, ch);
    if picked {
        for dy in [F(5.0), F(6.0)] {
            let base = m.r32(r + 8).wrapping_sub(ws);
            let (x, y) = (cv(m.r32(r)), cv(m.r32(r + 4)) + dy);
            let x0 = x + cv(base.wrapping_add(a)) - F(10.0);
            let x1 = x + cv(base.wrapping_add(e)) - F(10.0);
            line(m, env, x0, y, x1, y);
        }
    }
}

/// `cColorMenuItem::Measure` 0x18d6a0(out, this): the widest channel name + the widest value + 242; the font's height
/// + 4
pub fn colour_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    let n = |i: u32, m: &Mem| m.r32(CHANNEL_NAMES + 4 * i);
    let font = |m: &Mem| item_font(m, this);
    let h = m.r32(font(m) + 0xc).wrapping_add(4);
    let w2 = measure(m, env, font(m), n(2, m));
    let w0 = measure(m, env, font(m), n(0, m));
    let w1 = measure(m, env, font(m), n(1, m));
    let past2 = if (w1 as i32) < w0 as i32 {
        let v = measure(m, env, font(m), n(0, m));
        (w2 as i32) < v as i32
    } else {
        let v = measure(m, env, font(m), n(1, m));
        (w2 as i32) < v as i32
    };
    let wn = if past2 {
        let w0 = measure(m, env, font(m), n(0, m));
        let w1 = measure(m, env, font(m), n(1, m));
        let s = if (w1 as i32) < w0 as i32 { n(0, m) } else { n(1, m) };
        measure(m, env, font(m), s)
    } else {
        measure(m, env, font(m), n(2, m))
    };
    let wv = measure(m, env, font(m), COLOUR_WIDEST);
    m.w32(out, 0);
    m.w32(out + 4, 0);
    m.w32(out + 8, wn.wrapping_add(wv).wrapping_add(0xf2));
    m.w32(out + 0xc, h);
    out
}

/// `cColorMenuItem::Draw` 0x18d8d8(this, rect): its box, the channel's name, its value (· 255.99, "%d") right-aligned
/// at 212 from the right, a gradient bar (182 by 11, 192 from the right, 13 up from the bottom) from the colour with
/// the channel at 0 to it at 1, and a marker over the bar at the value
pub fn colour_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, r: u32, scratch: u32) {
    // (the game's frame: the rect's copy at +0 (then the text colour), the box colour at +0x10 (then the value's
    // text), its copy +0x20, the bar's rect +0x30, its ends' colours +0x40 / +0x50 and the vectors +0x60 / +0x70)
    let (rc, c0, c, tc, text) = (scratch, scratch + 0x10, scratch + 0x20, scratch, scratch + 0x10);
    copy(m, rc, r, 4);
    let picked = menu_current_item(m, env, m.r32(this + 4)) == this;
    copy(m, c0, colours(picked).0, 3);
    copy(m, c, c0, 3);
    draw_box(m, env, rc, c, scratch + 0x200);
    let right = m.r32(r).wrapping_add(m.r32(r + 8));
    let (vx, bx) = (right.wrapping_sub(0xd4), right.wrapping_sub(0xc0));
    let picked = menu_current_item(m, env, m.r32(this + 4)) == this;
    copy(m, tc, colours(picked).1, 3);
    set_rgbv(m, env, DRAW_COLOUR, tc);
    let y = m.r32(r + 4).wrapping_add(1);
    let ch = m.r32(this + 0x14);
    let name = m.r32(CHANNEL_NAMES + (ch << 2));
    let f = item_font(m, this);
    print(m, env, f, name, m.r32(r).wrapping_add(0xa), y);
    let col = m.r32(this + 0x10);
    let v = rf(m, col.wrapping_add(m.r32(this + 0x14) << 2)) * k(0x437f_fd71);
    call(m, env, SPRINTF, &[text, INT_FORMAT, crate::sndlib::cvt_w(v.0) as u32]);
    let font = item_font(m, this);
    let w = measure(m, env, font, text);
    print(m, env, font, text, vx.wrapping_sub(w), m.r32(r + 4).wrapping_add(1));
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    rcall(m, env, 0x1e0, &[0], &[]);
    let (br, e0, e1, lo, hi) = (scratch + 0x30, scratch + 0x40, scratch + 0x50, scratch + 0x60, scratch + 0x70);
    let col = m.r32(this + 0x10);
    let chn = m.r32(this + 0x14);
    m.w32(br, bx);
    m.w32(br + 4, m.r32(r + 4).wrapping_add(m.r32(r + 0xc)).wrapping_sub(0xd));
    m.w32(br + 8, 0xb6);
    m.w32(br + 0xc, 0xb);
    copy(m, e0, col, 3);
    copy(m, e1, col, 3);
    m.w32(e0 + (chn << 2), 0);
    wf(m, e1 + (chn << 2), ONE);
    wf(m, hi, ONE);
    wf(m, lo, ONE);
    copy(m, lo + 4, e0, 3);
    copy(m, hi + 4, e1, 3);
    let f = [cv(m.r32(br)), cv(m.r32(br + 4)), cv(m.r32(br + 8)), cv(m.r32(br + 0xc))];
    fcall(m, env, GRADIENT_RECT, &[DRAW_COLOUR, lo, hi, hi, lo], &f);
    let col = m.r32(this + 0x10);
    let v = rf(m, col.wrapping_add(m.r32(this + 0x14) << 2));
    let at = crate::sndlib::cvt_w((cv(m.r32(br)) + cv(m.r32(br + 8)) * v).0);
    let mx = cvt_s_w(at);
    let by = cv(m.r32(br + 4));
    line(m, env, mx, by - F(2.0), mx - F(5.0), by - F(7.0));
    let by = cv(m.r32(br + 4));
    line(m, env, mx, by - F(2.0), mx + F(5.0), by - F(7.0));
}

/// `cRGBTitleMenuItem::Measure` 0x18dce0(out, this): its label (+0x14) + 53 wide, at least 27 high
pub fn rgb_title_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32, scratch: u32) -> u32 {
    measure_label(m, env, scratch, this, m.r32(this + 0x14), 0, 0);
    let h = m.r32(scratch + 0xc);
    let h = if 0x1a < h as i32 { h } else { 0x1b };
    m.w32(out, m.r32(scratch));
    m.w32(out + 4, m.r32(scratch + 4));
    m.w32(out + 8, m.r32(scratch + 8).wrapping_add(0x35));
    m.w32(out + 0xc, h);
    out
}

/// `cRGBTitleMenuItem::Draw` 0x18dd58(this, rect): its label, then a 23 by 23 swatch of the colour 33 in from the
/// right, 4 down
pub fn rgb_title_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, r: u32, scratch: u32) {
    let rc = scratch + 0xd0;
    copy(m, rc, r, 4);
    draw_label(m, env, this, rc, m.r32(this + 0x14), 0, 0, scratch);
    set_rgbv(m, env, DRAW_COLOUR, m.r32(this + 0x10));
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    rcall(m, env, 0x1e0, &[0], &[]);
    let sw = scratch + 0xe0;
    m.w32(sw + 0xc, 0x17);
    m.w32(sw, m.r32(r).wrapping_add(m.r32(r + 8)).wrapping_sub(0x21));
    m.w32(sw + 4, m.r32(r + 4).wrapping_add(4));
    m.w32(sw + 8, 0x17);
    rect_i(m, env, DRAW_COLOUR, sw);
}

/// 0x18de50: the debug menu's manager destroyed (at exit)
pub fn manager_at_exit(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    manager_delete(m, env, MANAGER, 2);
}

/// 0x18de60: the debug menu's manager made, the six colours filled
pub fn static_init(m: &mut Mem) {
    manager_new(m, MANAGER);
    let one = ONE.0.to_bits();
    for (a, v) in [
        (BOX + 8, 0x3eeb_020c),
        (BOX_DISABLED, 0x3f20_96bc),
        (TEXT_PICKED + 8, one),
        (TEXT_DISABLED + 8, 0x3ca3_0553),
        (BOX, 0),
        (BOX_PICKED, one),
        (BOX_PICKED + 8, 0),
        (BOX_DISABLED + 4, 0x3f20_346e),
        (BOX_DISABLED + 8, one),
        (TEXT, one),
        (TEXT + 8, 0),
        (TEXT_PICKED, 0),
        (TEXT_DISABLED, 0),
        (BOX + 4, 0),
        (BOX_PICKED + 4, one),
        (TEXT + 4, one),
        (TEXT_PICKED + 4, 0),
        (TEXT_DISABLED + 4, 0),
    ] {
        m.w32(a, v);
    }
}

/// `cErrorMenu::~cErrorMenu` 0x18dfd0(this, flags): its pop, space and null items, then the menu
pub fn error_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    for o in [0x144, 0x130, 0x118] {
        item_delete(m, env, this + o, 2);
    }
    menu_delete(m, env, this, flags);
}

/// 0x18e028(this, label): the RGB item's title's label
pub fn rgb_set_title(m: &mut Mem, this: u32, label: u32) {
    m.w32(this + 0x5a8, label);
}

/// `cRGBMenuItem::~cRGBMenuItem` 0x18e080(this, flags): its title and three channels, then the expand item
pub fn rgb_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    for o in [0x594, 0x574, 0x554, 0x534] {
        item_delete(m, env, this + o, 2);
    }
    expand_delete(m, env, this, flags);
}

/// `cExpandMenuItem::~cExpandMenuItem` 0x18e198(this, flags): its menu, then the item
pub fn expand_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    menu_delete(m, env, this + 0x14, 2);
    item_delete(m, env, this, flags);
}

/// The plain kinds' destructors (0x18e140 the RGB title's, 0x18e280 the colour's, 0x18e2d8 the string's, 0x18e360
/// the angle's, 0x18e3e8 the float's, 0x18e450 the int's, 0x18e4a8 the space's, 0x18e510 the null's, 0x18e578 the
/// command's, 0x18e5d8 the pop's, 0x18e638 the sub's, 0x18e6b0 the enum's, 0x18e718 the bool's): the item's; and the
/// expand item's menu's 0x18e228: the menu's
pub fn plain_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    item_delete(m, env, this, flags);
}

/// A word set: the kinds' setters (0x18e0e8 +0x14, 0x18e2f0 +0x10, 0x18e308 +0x14, 0x18e378 +0x10, 0x18e390 +0x14,
/// 0x18e3f0 +0x10, 0x18e3f8 +0x14, 0x18e4b0 +0x14, 0x18e4b8 +0x10, 0x18e518 +0x10, 0x18e520 +0x14, 0x18e580 +0x10,
/// 0x18e5e0 +0x14, 0x18e640 +0x10, 0x18e648 +0x1c, 0x18e6b8 +0x10, 0x18e6c0 +0x1c)
pub fn set_word(m: &mut Mem, this: u32, off: u32, v: u32) {
    m.w32(this + off, v);
}

/// 0x18e2e0 / 0x18e368(this; scale, offset): a float or angle item's scale and offset
pub fn float_set_scale(m: &mut Mem, this: u32, scale: F, offset: F) {
    wf(m, this + 0x24, offset);
    wf(m, this + 0x20, scale);
}

/// 0x18e2f8 / 0x18e380(this; min, max): its range
pub fn float_set_range(m: &mut Mem, this: u32, min: F, max: F) {
    wf(m, this + 0x1c, max);
    wf(m, this + 0x18, min);
}

/// 0x18e650(this, count, list): an enum's list
pub fn enum_set_count_list(m: &mut Mem, this: u32, count: u32, list: u32) {
    m.w32(this + 0x18, list);
    m.w32(this + 0x14, count);
}

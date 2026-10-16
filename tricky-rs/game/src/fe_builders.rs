//! The front end's shared builders (board row G9b2b) on the game's bytes: the screen's menu list, its standard
//! widgets laid out and slid in, and the help bar with its buttons; every call out through [`FeEnv`], the widgets
//! made in place as [`crate::fe_widgets`]; checked in the function runner (`tests/fe_builders.rs`).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, GAPP, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use crate::fe_charselect::SPRITE_RECT;
use crate::fe_customboard::HELP_LAYOUT_SCALED;
use crate::fe_title::TEXT_CTOR;
use crate::fe_trickbook::{LIST_CTOR, LIST_VTABLE};
use crate::fe_username::{ICON_COLOUR, ICON_FRAMES};
use crate::fe_widgets::{icon_init, text_entry_init, vcall_class, ICON_VTABLE};
use crate::pf::Mem;

/// The help bar's buttons (17 of 12 bytes: the button's bit, its sprite, its text id) and a button's sprite and text
/// set (0x2ae230: the button, the sprite, the text); a help button's vtable
pub const HELP_BUTTONS_TABLE: u32 = 0x347f20;
pub const SET_BUTTON: u32 = 0x2ae230;
pub const HELP_BUTTON_VTABLE: u32 = 0x3ae690;

const ONE: u32 = 0x3f80_0000;

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// A widget moved by `d` (x, y, z) through its +0x70 (w 1)
fn shift(m: &mut Mem, e: &mut dyn FeEnv, w: u32, d: [f32; 3], buf: u32) {
    put4(m, buf, [d[0].to_bits(), d[1].to_bits(), d[2].to_bits(), ONE]);
    vcall(m, e, w, 0x70, &[buf]);
}

fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32, x: f32, y: f32) {
    put4(m, buf, [x.to_bits(), y.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[w, buf]);
}

/// 0x274108 (`FE_CreateList2`; the screen, the menu's parent; `buf` 0x10 bytes): the screen's menu (+0x3c) made (a list
/// of 0x1b0 bytes, 0x26d3b8 with (1, 2... as 0, 0, 1), the list's vtable): +0x150, +0x190 and +0x158 1, +0x15c 4.0,
/// +0x194..+0x1a0, +0x13c and +0x154 cleared, placed at (320, 93), +0x70 1
pub fn build_menu(m: &mut Mem, e: &mut dyn FeEnv, s: u32, parent: u32, buf: u32) {
    let l = alloc(m, e, 0x3ae5d8, 0x1b0);
    e.call(m, LIST_CTOR, &[l, s, parent, 0, 0, 1]);
    m.w32(l.wrapping_add(0x150), 1);
    m.w32(l.wrapping_add(0x190), 1);
    m.w32(l.wrapping_add(0x158), 1);
    m.w32(s + 0x3c, l);
    m.w32(l.wrapping_add(0xc), LIST_VTABLE);
    m.w32(l.wrapping_add(0x15c), 4.0f32.to_bits());
    for o in [0x194u32, 0x198, 0x19c, 0x1a0, 0x13c, 0x154] {
        m.w32(l.wrapping_add(o), 0);
    }
    place(m, e, l, buf, 320.0, 93.0);
    m.w32(m.r32(s + 0x3c) + 0x70, 1);
}

/// 0x2742e8 (the screen's widgets laid out and slid in; `buf` 0x50 bytes):
/// - the menu (+0x3c): laid out (+0x140), centred on its size (moved by -0.5 × +0x170, through the VU's multiply);
///   with arrows (+0x144) its left and right arrows (+0x148, +0x14c) taken to x 138 and 485, level with the menu
///   (its y + half its height less the arrow's, + 2); when its items slide (+0x150 1), each item not already moving
///   (+0x20) fades from white to the highlight (track 5, 30 frames, mode 2) and is hidden; the menu shown and the
///   widgets reset onto it;
/// - the title (+0x38) put above the top by its height; the prompt (+0x40) put below by its extent and slid back up
///   (track 1, 30 frames); the help bar (+0x44) laid out (0x2755e8 with the sprite 7's height + 75), put at y 480 and
///   slid up by its extent;
/// - the world group (+0x34) and the screen shown (+0x20)
pub fn layout(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (b0, b10, b20, b30) = (buf, buf + 0x10, buf + 0x20, buf + 0x30);
    let menu = m.r32(s + 0x3c);
    if menu != 0 {
        vcall(m, e, menu, 0x140, &[]);
        let menu = m.r32(s + 0x3c);
        let d = [-0.5 * fl(m.r32(menu + 0x170)), -0.5 * fl(m.r32(menu + 0x174)), -0.5 * fl(m.r32(menu + 0x178))];
        shift(m, e, menu, d, b10);
        let menu = m.r32(s + 0x3c);
        let left = m.r32(menu + 0x148);
        if m.r32(menu + 0x144) != 0 && left != 0 {
            vcall(m, e, left, 0x130, &[b0]);
            let menu = m.r32(s + 0x3c);
            let y = fl(m.r32(menu + 0x84)) + (fl(m.r32(menu + 0x174)) - fl(m.r32(b0 + 4))) * 0.5 + 2.0;
            let left = m.r32(menu + 0x148);
            let to = |m: &Mem, w: u32, x: f32| {
                [x - fl(m.r32(w + 0x80)), y - fl(m.r32(w + 0x84)), 0.0 - fl(m.r32(w + 0x88))]
            };
            let d = to(m, left, 138.0);
            shift(m, e, left, d, b30);
            let right = m.r32(m.r32(s + 0x3c) + 0x14c);
            let d = to(m, right, 485.0);
            shift(m, e, right, d, b30);
        }
        let menu = m.r32(s + 0x3c);
        if m.r32(menu + 0x150) == 1 {
            put4(m, b0, [ONE; 4]);
            put4(m, b10, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
            let mut n = e.call(m, LIST_NTH, &[menu + 0x118, 0]);
            while n != 0 && crate::fe_helpers::is_sentinel(m, n) == 0 {
                if m.r32(n + 0x20) == 0 {
                    e.call(m, ICON_COLOUR, &[n, 5, b0]);
                    e.call(m, ICON_FRAMES, &[n, 5, b10, 0x1e, 2]);
                    vcall(m, e, n, 0x20, &[0]);
                }
                n = m.r32(n + 8);
            }
        }
        let menu = m.r32(s + 0x3c);
        vcall(m, e, menu, 0x20, &[1]);
        let menu = m.r32(s + 0x3c);
        e.call(m, WIDGETS_RESET, &[s, menu]);
    }
    let t = m.r32(s + 0x38);
    if t != 0 {
        vcall(m, e, t, 0x130, &[b0]);
        let h = fl(m.r32(b0 + 4));
        let t = m.r32(s + 0x38);
        place(m, e, t, b10, 0.0, -h);
    }
    let p = m.r32(s + 0x40);
    if p != 0 {
        vcall(m, e, p, 0x128, &[b0, b10]);
        let dy = fl(m.r32(b0 + 4)) - fl(m.r32(b10 + 4));
        let p = m.r32(s + 0x40);
        place(m, e, p, b20, 0.0, dy);
        put4(m, b20, [0, 0, 0, ONE]);
        let p = m.r32(s + 0x40);
        e.call(m, ICON_FRAMES, &[p, 1, b20, 0x1e, 0]);
        let p = m.r32(s + 0x40);
        vcall(m, e, p, 0x20, &[0]);
    }
    let h = m.r32(s + 0x44);
    if h != 0 {
        let r = e.call(m, SPRITE_RECT, &[s, 7]);
        let f12 = fl(m.r32(r.wrapping_add(8))) + 40.0 + 35.0;
        e.call(m, HELP_LAYOUT_SCALED, &[s, f12.to_bits()]);
        let h = m.r32(s + 0x44);
        place(m, e, h, b0, 0.0, 480.0);
        let h = m.r32(s + 0x44);
        vcall(m, e, h, 0x128, &[b0, b10]);
        let d = fl(m.r32(b10 + 4)) - fl(m.r32(b0 + 4));
        put4(m, b20, [0, (480.0 - d).to_bits(), 0, ONE]);
        let h = m.r32(s + 0x44);
        e.call(m, ICON_FRAMES, &[h, 1, b20, 0x1e, 0]);
        let h = m.r32(s + 0x44);
        vcall(m, e, h, 0x20, &[0]);
    }
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x20, &[1]);
    vcall(m, e, s, 0x20, &[1]);
}

/// 0x2719c0 (`FE3D_BuildButtonBar`; the screen, the buttons' bits, the bar's parent; `buf` 0x30 bytes): with any
/// bits, the help bar (+0x48: a list, 0x26d3b8 with (1, 2, 0), its +0xf8(1)) and one button for each bit of the table
/// at 0x347f20 asked for: an entry (0x150 bytes, 15.0 high) with its text (the small font) and its icon (15 × 15),
/// the help button's vtable, its sprite and text (0x2ae230), its bit as its id, white, the text's +0x114 1, added to
/// the bar; the bar's spacing 2.0, laid out and placed at (595, 37)
pub fn build_help_bar(m: &mut Mem, e: &mut dyn FeEnv, s: u32, bits: u32, parent: u32, buf: u32) {
    let (b0, b10, b20) = (buf, buf + 0x10, buf + 0x20);
    let mut bits = bits;
    if bits == 0 {
        return;
    }
    let at = alloc(m, e, 0x3ae558, 0x190);
    let bar = e.call(m, LIST_CTOR, &[at, s, parent, 1, 2, 0]);
    m.w32(s + 0x48, bar);
    vcall(m, e, bar, 0xf8, &[1]);
    for k in 0..17u32 {
        let row = HELP_BUTTONS_TABLE + 12 * k;
        let bit = m.r32(row);
        if bit == 0 || bit & bits == 0 {
            continue;
        }
        let en = alloc(m, e, 0x3ae568, 0x150);
        let bar = m.r32(s + 0x48);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
        text_entry_init(m, e, en, s, bar);
        let t = alloc(m, e, 0x3ae4e0, 0x130);
        let t = e.call(m, TEXT_CTOR, &[t, s, bar, font]);
        let old = m.r32(en.wrapping_add(0x110));
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(en.wrapping_add(0x110), t);
        m.w32(en.wrapping_add(0xc), HELP_BUTTON_VTABLE);
        let ic = alloc(m, e, 0x3ae510, 0x120);
        icon_init(m, e, ic, s, bar);
        put4(m, b20, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[b20]);
        let old = m.r32(en.wrapping_add(0x114));
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(en.wrapping_add(0x114), ic);
        put4(m, b10, [15.0f32.to_bits(), 15.0f32.to_bits(), 0, 0]);
        vcall(m, e, ic, 0x138, &[b10]);
        let sprite = m.r32(row + 4);
        let text = m.r32(row + 8);
        e.call(m, SET_BUTTON, &[en, sprite, text]);
        m.w32(en, m.r32(row));
        put4(m, b0, [ONE; 4]);
        vcall(m, e, en, 0x50, &[b0]);
        let bar = m.r32(s + 0x48);
        m.w32(m.r32(en.wrapping_add(0x110)) + 0x114, 1);
        vcall(m, e, bar, 0x148, &[en]);
        bits &= !m.r32(row);
    }
    let bar = m.r32(s + 0x48);
    m.w32(bar + 0x15c, 2.0f32.to_bits());
    vcall(m, e, bar, 0x140, &[]);
    let bar = m.r32(s + 0x48);
    vcall(m, e, bar, 0x130, &[b0]);
    let bar = m.r32(s + 0x48);
    place(m, e, bar, b10, 595.0, 37.0);
}

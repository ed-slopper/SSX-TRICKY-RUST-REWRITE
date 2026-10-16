//! The front end's title screen (board row G9c): `cFE3DStateTitle`'s methods on the game's bytes, every call through
//! [`FeEnv`] (the screen base's methods it calls run as ported: [`crate::fe3dstate`]), checked in the function runner
//! (`tests/fe_title.rs`).
//!
//! The title screen (0x5c bytes) adds to the base: +0x48 the press-start prompt, +0x54 the EA logo, +0x58 the
//! copyright line. Its menu is the title text (+0x3c): two items, "start" (0) and "extras" (1), the selected one at its
//! +0x140's first word.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::pf::Mem;

/// The vtables the title writes: its own, and the two screens it can go to
pub const VTABLE: u32 = 0x3b3350;
pub const MODE_SELECT_VTABLE: u32 = 0x3b3190;
pub const DVD_CONTENT_VTABLE: u32 = 0x3b3270;
/// The screen base's constructor (`cFE3DState::cFE3DState` 0x2715c8)
pub const BASE_CTOR: u32 = 0x2715c8;
/// The two menu items' text ids (a pair of words at 0x3b2108)
pub const ITEM_TEXTS: u32 = 0x3b2108;
/// The widget constructor (`cFEText` 0x2b1138: the memory, the screen, 0, the font word), the screen helpers
/// (0x274108 build the base widgets, 0x2763f0 add a widget, 0x2719c0 / 0x2718f0 the help bar's buttons, 0x271848 a
/// help bar button, 0x2742e8 the backdrop), the music events (0x262438 the title loop on, 0x262a68 off) and the front
/// end's "booted to the title" query 0x265af8
pub const TEXT_CTOR: u32 = 0x2b1138;
pub const BUILD_BASE: u32 = 0x274108;
pub const ADD_WIDGET: u32 = 0x2763f0;
pub const HELP_BUTTONS: u32 = 0x2719c0;
pub const HELP_LAYOUT: u32 = 0x2718f0;
pub const HELP_BUTTON: u32 = 0x271848;
pub const BACKDROP: u32 = 0x2742e8;
pub const MUSIC_ON: u32 = 0x262438;
pub const MUSIC_OFF: u32 = 0x262a68;
pub const BOOTED: u32 = 0x265af8;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn scene_vcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) {
    let scene = m.r32(fe(m) + 0xa4);
    let vt = m.r32(scene);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![scene.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a);
}

/// `cFE3DStateTitle::cFE3DStateTitle` 0x293390: the base built (0x2715c8), its vtable, accept and back off
pub fn construct(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    e.call(m, BASE_CTOR, &[s]);
    m.w32(s + 0x1c, 0);
    m.w32(s + 0xc, VTABLE);
    m.w32(s + 0x18, 0);
    s
}

/// `cFE3DStateTitle::CreateNextState` 0x2933d0: by the menu's item, "start" the mode select (0x54 bytes), "extras" the
/// DVD content screen (0xb8 bytes, its fields cleared, the 3D scene's +0xc0 and +0x88); else none. Each allocated
/// (the pools 0x3b20f0 / 0x3b2100), filled and built on the base
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let item = m.r32(m.r32(m.r32(s + 0x3c) + 0x140));
    if item == 0 {
        let at = e.call(m, ALLOC, &[0x3b20f0, 0x54, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x54]);
        }
        e.call(m, BASE_CTOR, &[at]);
        m.w32(at.wrapping_add(0xc), MODE_SELECT_VTABLE);
        return at;
    }
    if item != 1 {
        return 0;
    }
    let at = e.call(m, ALLOC, &[0x3b2100, 0xb8, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, 0xb8]);
    }
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0x54), 0);
    m.w32(at.wrapping_add(0xc), DVD_CONTENT_VTABLE);
    m.w32(at.wrapping_add(0x58), 0);
    for o in [0x60u32, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x90, 0x94, 0x98, 0x9c] {
        m.w32(at.wrapping_add(o), 0);
    }
    scene_vcall(m, e, 0xc0, &[]);
    scene_vcall(m, e, 0x88, &[]);
    at
}

/// A text widget made for the title: allocated from `pool` (0x130 bytes), filled, built (`cFEText`) with the font word
/// at gApp + `font`
fn new_text(m: &mut Mem, e: &mut dyn FeEnv, s: u32, pool: u32, font: u32) -> u32 {
    let at = e.call(m, ALLOC, &[pool, 0x130, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
    }
    let word = m.r32(m.r32(GAPP).wrapping_add(font));
    e.call(m, TEXT_CTOR, &[at, s, 0, word])
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// `cFE3DStateTitle` vf11 0x293528, the screen built (`buf` 0x20 bytes of scratch): the base widgets (0x274108); the
/// menu (the title text, its +0x154 2) with its two items (the texts at 0x3b2108, numbered 0 and 1); the EA logo
/// (+0x54, text 0x6b) and the copyright line (+0x58, text 0xdd8), each white, its +0x118 0.8 and +0x114 1, sized
/// (+0x130) and placed at (38, 410) and (20, 428); the help bar's two buttons; the press-start prompt (+0x48) at
/// (its x, 428), its button's +0x118 0.8; the backdrop; the menu at (20, 330)
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (col, pos) = (buf, buf + 0x10);
    let one = 1.0f32.to_bits();
    let k08 = 0x3f4c_cccd;
    e.call(m, BUILD_BASE, &[s, 0]);
    let t = m.r32(s + 0x3c);
    m.w32(t + 0x154, 2);
    e.call(m, ADD_WIDGET, &[s, t]);
    let texts = [m.r32(ITEM_TEXTS), m.r32(ITEM_TEXTS + 4)];
    for (k, id) in texts.iter().enumerate() {
        let w = new_text(m, e, s, 0x3b2110, 0x5c);
        m.w32(w, k as u32);
        vcall(m, e, w, 0x150, &[*id]);
        vcall(m, e, t, 0x148, &[w]);
    }
    for (field, text, at) in [(0x54u32, 0x6bu32, [0x4218_0000u32, 0x43cd_0000]), (0x58, 0xdd8, [0x41a0_0000, 0x43d6_0000])] {
        let w = new_text(m, e, s, 0x3b2120, 0x60);
        m.w32(s + field, w);
        vcall(m, e, w, 0x150, &[text]);
        put4(m, col, [one; 4]);
        let w = m.r32(s + field);
        vcall(m, e, w, 0x50, &[col]);
        let w = m.r32(s + field);
        if field == 0x54 {
            m.w32(w + 0x118, k08);
            m.w32(w + 0x114, 1);
        } else {
            m.w32(w + 0x114, 1);
            m.w32(w + 0x118, k08);
        }
        vcall(m, e, w, 0x130, &[col]);
        put4(m, pos, [at[0], at[1], 0, one]);
        let w = m.r32(s + field);
        e.call(m, base::WIDGET_PLACE, &[w, pos]);
        let w = m.r32(s + field);
        e.call(m, ADD_WIDGET, &[s, w]);
    }
    e.call(m, HELP_BUTTONS, &[s, 2, 0]);
    e.call(m, HELP_LAYOUT, &[s, 2]);
    let p = m.r32(s + 0x48);
    let x = m.r32(p + 0x80);
    m.w32(p + 0x70, 2);
    put4(m, col, [x, 0x43d6_0000, 0, one]);
    e.call(m, base::WIDGET_PLACE, &[p, col]);
    let b = e.call(m, HELP_BUTTON, &[s, 2]);
    let p = m.r32(s + 0x48);
    m.w32(m.r32(b + 0x110) + 0x118, k08);
    vcall(m, e, p, 0x140, &[]);
    let p = m.r32(s + 0x48);
    e.call(m, ADD_WIDGET, &[s, p]);
    e.call(m, BACKDROP, &[s]);
    put4(m, col, [0x41a0_0000, 0x43a5_0000, 0, one]);
    let t = m.r32(s + 0x3c);
    e.call(m, base::WIDGET_PLACE, &[t, col]);
}

/// `cFE3DStateTitle::Activate` 0x293938: the front end's +0xf8 cleared and both players' +0x38 set, the title music
/// (0x262438 on the front end's +0x70), the menu, logo and copyright line taking input (+0x28, 1), the base's
/// `Activate`; when the game booted to the title (0x265af8), the menu's +0x160(0) and the screen's +0x40
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    m.w32(f + 0xf8, 0);
    for k in 0..2 {
        let p = m.r32(f + 0x74 + 4 * k);
        m.w32(p + 0x38, 1);
    }
    let f = fe(m);
    e.call(m, MUSIC_ON, &[f + 0x70]);
    for o in [0x3cu32, 0x54, 0x58] {
        let w = m.r32(s + o);
        vcall(m, e, w, 0x28, &[1]);
    }
    base::activate(m, e, s);
    let f = fe(m);
    if e.call(m, BOOTED, &[f]) != 0 {
        let t = m.r32(s + 0x3c);
        vcall(m, e, t, 0x160, &[0]);
        vcall(m, e, s, 0x40, &[]);
    }
}

/// `cFE3DStateTitle::Deactivate` 0x293a58: both players' +0x38 cleared, the widget group's +0x28(0), then the base's
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let g = m.r32(s + 0x34);
    let f = fe(m);
    for k in 0..2 {
        let p = m.r32(f + 0x74 + 4 * k);
        m.w32(p + 0x38, 0);
    }
    vcall(m, e, g, 0x28, &[0]);
    base::deactivate(m, e, s);
}

/// `cFE3DStateTitle` vf19 0x293ad8: the 3D scene's +0x10(0)
pub fn vf19(m: &mut Mem, e: &mut dyn FeEnv) {
    scene_vcall(m, e, 0x10, &[0]);
}

/// `cFE3DStateTitle` vf20 0x293b18: the widget group's +0x28(0); with "start" chosen the title music off (0x262a68)
pub fn vf20(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x28, &[0]);
    if m.r32(m.r32(m.r32(s + 0x3c) + 0x140)) == 0 {
        let f = fe(m);
        e.call(m, MUSIC_OFF, &[f + 0x70]);
    }
}

/// `cFE3DStateTitle` vf26 0x293b80 (the item picked, or 0): the fade out 2 for the "extras" item (its word 1), else
/// none; then the screen's +0x30
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let k = if item != 0 && m.r32(item) == 1 { 2 } else { 0 };
    m.w32(s + 0x24, k);
    vcall(m, e, s, 0x30, &[]);
}

/// `cFE3DStateTitle::LayoutWidgets` 0x293ba0: the logo's, copyright line's, menu's and prompt's +0x140
pub fn layout_widgets(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    for o in [0x54u32, 0x58, 0x3c, 0x48] {
        let w = m.r32(s + o);
        vcall(m, e, w, 0x140, &[]);
    }
}

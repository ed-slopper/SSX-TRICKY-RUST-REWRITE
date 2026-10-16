//! The front end's user name screen (board row G9r): `cFE3DStateUserName`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the widgets the builder makes in
//! place as [`crate::fe_widgets`]), checked in the function runner (`tests/fe_username.rs`).
//!
//! The screen edits the player's name: +0x54 the name (up to nine characters and its end), +0x70 the on-screen
//! keyboard, +0x74 the name's text widget, +0x68 set while the name shown is the stored one (the first key typed
//! replaces it), +0x6c set for the World Circuit (done goes on to the character accept). +0x2e is the player.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, WIDGETS_RESET, WIDGET_PLACE};
use crate::fe_characcept::{CAREER, PLAYER_SLOT};
use crate::fe_charselect::{PRINT, SPRITE_RECT};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_title::{BACKDROP, BASE_CTOR, BOOTED, TEXT_CTOR};
use crate::fe_widgets::{icon_init, vcall_class, widget_init, GROUP_ADD, ICON_VTABLE};
use crate::pf::Mem;

pub const ACCEPT_VTABLE: u32 = 0x3b02d0;
pub const KEYBOARD_VTABLE: u32 = 0x3b6920;
/// The keyboard's key pressed (keyboard; 0x64 delete, 0x65 done, else a character), its keys' range set (keyboard,
/// 0x65); a string's length; the career's name (career, slot) and its name set (career, slot, name); an icon's sprite
/// colour (icon, sprite, colour) and its colour and frames (icon, sprite, colour, 0x1e, 2)
pub const KEY_PRESSED: u32 = 0x2b0410;
pub const KEYS_SET: u32 = 0x2b0428;
pub const STRLEN: u32 = 0x2fd260;
pub const CAREER_NAME: u32 = 0x164918;
pub const SET_NAME: u32 = 0x164968;
pub const ICON_COLOUR: u32 = 0x2ac348;
pub const ICON_FRAMES: u32 = 0x2ac3b0;

const ONE: u32 = 0x3f80_0000;
const HIGHLIGHT: [u32; 4] = [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687];

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
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

/// `cFE3DStateUserName::CreateNextState` 0x29c2d8: for the World Circuit (+0x6c) the character accept (0x54 bytes) with
/// the three bytes handed on, else none
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    if m.r32(s + 0x6c) == 0 {
        return 0;
    }
    let at = alloc(m, e, 0x3b2b68, 0x54);
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0xc), ACCEPT_VTABLE);
    for o in [0x2d, 0x2c, 0x2e] {
        let b = m.r8(s + o);
        m.w8(at.wrapping_add(o), b);
    }
    at
}

/// The name's text centred across 640 at its own height (+0x84)
fn centre_name(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let t = m.r32(s + 0x74);
    vcall(m, e, t, 0x130, &[buf]);
    let x = (640.0 - fl(m.r32(buf))) * 0.5;
    let t = m.r32(s + 0x74);
    let y = m.r32(t + 0x84);
    put4(m, buf + 0x10, [x.to_bits(), y, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[t, buf + 0x10]);
}

/// The player's stored name copied in (0x164918 for the player's slot, printed into +0x54) and centred
fn load_name(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let p = byte(m, s + 0x2e) as u32;
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
    let sb = byte(m, slot) as u32;
    let name = e.call(m, CAREER_NAME, &[CAREER, sb]);
    e.call(m, PRINT, &[s + 0x54, name]);
    centre_name(m, e, s, buf);
}

/// `cFE3DStateUserName` vf11 0x29c370, the screen built (`buf` 0x20 bytes of scratch):
/// - the base widgets with the title (0x274208: 300.0, text 0xdb1, 0x200a) and one menu item (text 0xe1c);
/// - the keyboard (no parent; +0x114 14, +0x118 1.0, white, keys up to 0x65), centred and 73 above y 300 by its size,
///   +0x70 2, its bytes +0x120 0xc and +0x121 / +0x122 0xd;
/// - its cursor icon (sprite 0x2a sized 1.1 × the sprite, the colours and frames, opened, +0x114 0x60, +0x70 -1,
///   kept at the keyboard's +0x11c);
/// - the name's text (under the title, the name at +0x54, 0.7 high, highlighted, +0x70 2, at (320, 100));
/// - the name box (245 × 30 at (320, 94), sprite 0x2c sized to the sprite at x 197.5 and 442.5 for its ends);
/// - the keyboard under the title, the backdrop, the widgets reset onto the keyboard.
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    e.call(m, BUILD_TITLED, &[300.0f32.to_bits(), s, 0xdb1, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let at = alloc(m, e, 0x3b2a48, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
    let word = m.r32(0x3ae520);
    m.w32(w + 0x118, word);
    vcall(m, e, w, 0x150, &[0xe1c]);
    vcall(m, e, menu, 0x148, &[w]);

    let (b0, b10) = (buf, buf + 0x10);
    let kb = alloc(m, e, 0x3b2b78, 0x130);
    widget_init(m, e, kb, s, 0);
    m.w32(kb.wrapping_add(0x114), 0xe);
    m.w32(kb.wrapping_add(0x118), ONE);
    m.w32(kb.wrapping_add(0xc), KEYBOARD_VTABLE);
    m.w32(kb.wrapping_add(0x110), 0);
    m.w32(kb.wrapping_add(0x11c), 0);
    m.w32(s + 0x70, kb);
    put4(m, b0, [ONE; 4]);
    vcall_class(m, e, KEYBOARD_VTABLE, kb, 0x108, &[b0]);
    let kb = m.r32(s + 0x70);
    put4(m, b0, [ONE; 4]);
    vcall(m, e, kb, 0x110, &[b0]);
    let kb = m.r32(s + 0x70);
    e.call(m, KEYS_SET, &[kb, 0x65]);
    let kb = m.r32(s + 0x70);
    vcall(m, e, kb, 0x130, &[b0]);
    let x = (640.0 - fl(m.r32(b0))) * 0.5;
    let y = 300.0 - fl(m.r32(b0 + 4)) - 73.0;
    let kb = m.r32(s + 0x70);
    put4(m, b10, [x.to_bits(), y.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[kb, b10]);
    let kb = m.r32(s + 0x70);
    m.w8(kb + 0x120, 0xc);
    m.w32(kb + 0x70, 2);
    m.w8(kb + 0x121, 0xd);
    let kb = m.r32(s + 0x70);
    m.w8(kb + 0x122, 0xd);

    // the cursor
    let ic = alloc(m, e, 0x3b2b88, 0x120);
    let kb = m.r32(s + 0x70);
    icon_init(m, e, ic, s, kb);
    put4(m, b10, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[b10]);
    let r = e.call(m, SPRITE_RECT, &[s, 0x2a]);
    let wd = fl(m.r32(r.wrapping_add(4))) * 1.1;
    let r = e.call(m, SPRITE_RECT, &[s, 0x2a]);
    let ht = fl(m.r32(r.wrapping_add(8))) * 1.1;
    put4(m, b0, [wd.to_bits(), ht.to_bits(), 0, 0]);
    vcall(m, e, ic, 0x138, &[b0]);
    let tenth = 0.1f32.to_bits();
    put4(m, b0, [0x3f61_47ae, tenth, tenth, tenth]);
    e.call(m, ICON_COLOUR, &[ic, 5, b0]);
    put4(m, b10, [ONE; 4]);
    put4(m, b0, [ONE; 4]);
    e.call(m, ICON_FRAMES, &[ic, 5, b0, 0x1e, 2]);
    vcall(m, e, ic, 0x20, &[1]);
    let kb = m.r32(s + 0x70);
    m.w32(ic + 0x114, 0x60);
    m.w32(ic + 0x70, u32::MAX);
    m.w32(kb + 0x11c, ic);
    m.w32(ic + 0x110, 0x2a);

    // the name
    let title = m.r32(s + 0x38);
    let t = alloc(m, e, 0x3b2b98, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let tt = e.call(m, TEXT_CTOR, &[t, s, title, font]);
    m.w32(s + 0x74, tt);
    vcall(m, e, tt, 0x158, &[s + 0x54]);
    let tt = m.r32(s + 0x74);
    m.w32(tt + 0x118, 0.7f32.to_bits());
    put4(m, b0, HIGHLIGHT);
    vcall(m, e, tt, 0x50, &[b0]);
    let tt = m.r32(s + 0x74);
    m.w32(tt + 0x70, 2);
    m.w32(tt + 0x114, 0);
    put4(m, b0, [320.0f32.to_bits(), 100.0f32.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[tt, b0]);
    let tt = m.r32(s + 0x74);
    e.call(m, GROUP_ADD, &[title, tt]);

    // the name box and its two ends
    let dark = 0.477f32;
    let _ = dark;
    for (tag, sprite, colour, at_x, frames) in [
        (0x3b2ba8u32, None, [0.7f32.to_bits(), 0x3ef4_c986, 0x3ef4_c986, 0x3ef4_c986], 320.0f32, 0x20u32),
        (0x3b2bb8, Some(0x2c), [0.7f32.to_bits(), 0.3f32.to_bits(), 0.3f32.to_bits(), 0.3f32.to_bits()], 197.5, 8),
        (0x3b2bc8, Some(0x2c), [0.7f32.to_bits(), 0.3f32.to_bits(), 0.3f32.to_bits(), 0.3f32.to_bits()], 442.5, 1),
    ] {
        let ic = alloc(m, e, tag, 0x120);
        icon_init(m, e, ic, s, title);
        put4(m, b10, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[b10]);
        if let Some(sp) = sprite {
            m.w32(ic + 0x110, sp);
        }
        put4(m, b0, colour);
        vcall(m, e, ic, 0x50, &[b0]);
        let size = match sprite {
            None => [245.0f32.to_bits(), 30.0f32.to_bits(), 0, 0],
            Some(sp) => {
                let r = e.call(m, SPRITE_RECT, &[s, sp]);
                [m.r32(r.wrapping_add(4)), 30.0f32.to_bits(), 0, 0]
            }
        };
        put4(m, b0, size);
        vcall(m, e, ic, 0x138, &[b0]);
        m.w32(ic + 0x114, frames);
        put4(m, b0, [at_x.to_bits(), 94.0f32.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[ic, b0]);
        m.w32(ic + 0x70, 1);
        e.call(m, GROUP_ADD, &[title, ic]);
    }
    let title = m.r32(s + 0x38);
    let kb = m.r32(s + 0x70);
    e.call(m, GROUP_ADD, &[title, kb]);
    e.call(m, BACKDROP, &[s]);
    let kb = m.r32(s + 0x70);
    e.call(m, WIDGETS_RESET, &[s, kb]);
}

/// `cFE3DStateUserName::Activate` 0x29d2d0 (the screen, the one before; `buf` 0x20 bytes): the stored name loaded and
/// centred, +0x68 set, the base's `Activate`, the screen's +0x40 when booted
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    load_name(m, e, s, buf);
    m.w32(s + 0x68, 1);
    base::activate(m, e, s);
    let f = m.r32(m.r32(GAPP).wrapping_add(0x734));
    if e.call(m, BOOTED, &[f]) != 0 {
        vcall(m, e, s, 0x40, &[]);
    }
}

/// `cFE3DStateUserName` vf21 0x29d3e8 (backing out; `buf` 0x20 bytes): the stored name loaded back and centred, then
/// the base's vf21
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    load_name(m, e, s, buf);
    base::vf21(m, e, s);
}

/// `cFE3DStateUserName` vf26 0x29d4c8 (a key; `buf` 0x20 bytes): delete (0x64) drops the last character; done (0x65)
/// stores the name for the player's slot (0x164968) and leaves (+0x30 for the World Circuit, else +0x28); any other
/// key is typed after the name (onto an empty one while the stored name shows) while it is under nine characters;
/// the name re-centred after an edit
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let kb = m.r32(s + 0x70);
    let key = e.call(m, KEY_PRESSED, &[kb]);
    let name = s + 0x54;
    match key {
        0x64 => {
            let n = e.call(m, STRLEN, &[name]) as i32;
            if n > 0 {
                m.w8(name.wrapping_add(n as u32).wrapping_sub(1), 0);
            }
            centre_name(m, e, s, buf);
            m.w32(s + 0x68, 0);
        }
        0x65 => {
            let p = byte(m, s + 0x2e) as u32;
            let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
            let sb = byte(m, slot) as u32;
            e.call(m, SET_NAME, &[CAREER, sb, name]);
            let off = if m.r32(s + 0x6c) != 0 { 0x30 } else { 0x28 };
            vcall(m, e, s, off, &[]);
        }
        _ => {
            let mut n = e.call(m, STRLEN, &[name]) as i32;
            if m.r32(s + 0x68) != 0 {
                n = 0;
            }
            m.w32(s + 0x68, 0);
            if n < 9 {
                m.w8(name.wrapping_add(n as u32), key as u8);
                m.w8(name.wrapping_add(n as u32).wrapping_add(1), 0);
                centre_name(m, e, s, buf);
            }
        }
    }
}

/// 0x29d7a8: the World Circuit flag (+0x6c)
pub fn set_world_circuit(m: &mut Mem, s: u32, v: u32) {
    m.w32(s + 0x6c, v);
}

//! The front end's hint book (board row G9q): `cFE3DStateHintBook`'s methods on the game's bytes, every call through
//! [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function runner
//! (`tests/fe_hintbook.rs`).

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_title::{BACKDROP, TEXT_CTOR};
use crate::pf::Mem;

/// The menu's text ids (one word at 0x3afb58)
pub const ITEM_TEXTS: u32 = 0x3afb58;

/// `cFE3DStateHintBook` vf11 0x28c560, the screen built: the base widgets with the title (0x274208: 105.0, text
/// 0xe19, 0x200a); the menu's one item (its text the word at 0x3afb58, +0x118 1.0, numbered 0); the backdrop
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xe19, 0x200a]);
    let menu = m.r32(s + 0x3c);
    for k in 0..1u32 {
        let at = e.call(m, ALLOC, &[0x3af520, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        let id = m.r32(ITEM_TEXTS + 4 * k);
        m.w32(w + 0x118, 1.0f32.to_bits());
        vcall(m, e, w, 0x150, &[id]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, BACKDROP, &[s]);
}

/// `cFE3DStateHintBook` vf26 0x28c688: the screen's +0x30 (as the base's vf26)
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    base::vf26(m, e, s);
}

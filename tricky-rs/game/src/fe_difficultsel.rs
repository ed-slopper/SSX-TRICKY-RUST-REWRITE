//! The front end's difficulty select (board row G9h): `cFE3DStateDifficultSel`'s methods on the game's bytes, every
//! call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function
//! runner (`tests/fe_difficultsel.rs`).
//!
//! The menu (+0x3c) has three items (difficulties 0, 1, 2); the one highlighted is the difficulty word (0x32dea4).
//! +0x4c is the help line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_eventsel::VENUE_SEL_VTABLE;
use crate::fe_numplayers::{BUILD_TITLED, SET_TEXT};
use crate::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use crate::pf::Mem;

/// The difficulty word
pub const DIFFICULTY: u32 = 0x32dea4;
/// The music: 0x262338 the difficulty select's loop, the three items' cues (0x2631b0, 0x2631f0, 0x263230)
pub const MUSIC_LOOP: u32 = 0x262338;
pub const MUSIC_ITEMS: [u32; 3] = [0x2631b0, 0x2631f0, 0x263230];
pub const HELP: u32 = 0x29b9f8;
/// The three items' text ids
pub const ITEM_TEXTS: [u32; 3] = [0xe03, 0xe09, 0xe08];

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// `cFE3DStateDifficultSel::CreateNextState` 0x29b640: the venue select (0x7c bytes, vtable 0x3b2e10)
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv) -> u32 {
    let at = e.call(m, ALLOC, &[0x3b2a88, 0x7c, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, 0x7c]);
    }
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0xc), VENUE_SEL_VTABLE);
    at
}

/// `cFE3DStateDifficultSel` vf11 0x29b6a8, the screen built (`buf` 0x10 bytes of scratch): the base widgets with the
/// title (0x274278: 224.0, text 0xda9, 0x200a); the three items (texts 0xe03, 0xe09, 0xe08; +0x118 1.0; the
/// highlight colour (1, 0.953, 0.729, 0.106; +0x108), white (+0x110 and +0x50); numbered); the backdrop; no fade in
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let one = 1.0f32.to_bits();
    e.call(m, BUILD_TITLED, &[224.0f32.to_bits(), s, 0xda9, 0x200a]);
    let menu = m.r32(s + 0x3c);
    for (k, id) in ITEM_TEXTS.iter().enumerate() {
        let at = e.call(m, ALLOC, &[0x3b2a48, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w + 0x118, one);
        vcall(m, e, w, 0x150, &[*id]);
        put4(m, buf, [one, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
        vcall(m, e, w, 0x108, &[buf]);
        put4(m, buf, [one; 4]);
        vcall(m, e, w, 0x110, &[buf]);
        put4(m, buf, [one; 4]);
        vcall(m, e, w, 0x50, &[buf]);
        m.w32(w, k as u32);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
}

/// `cFE3DStateDifficultSel::Activate` 0x29b8c8: the difficulty select's music (0x262338), the base's `Activate`, the
/// help line
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    base::activate(m, e, s);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, HELP, &[s, chosen]);
}

/// `cFE3DStateDifficultSel` vf22 0x29b920 (the item that goes, the one that comes): the base's vf22; with an item
/// going, the coming item's cue and the difficulty set to it (items 0..2); the help line
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        let k = m.r32(to);
        if let Some(&cue) = MUSIC_ITEMS.get(k as usize) {
            let f = fe(m);
            e.call(m, cue, &[f + 0x70]);
            m.w32(DIFFICULTY, k);
        }
    }
    e.call(m, HELP, &[s, to]);
}

/// 0x29b9f8 (the screen, the item): the help line's text (+0x4c; 0x2b0f68) by the item: 0xcc8, 0xcc9, 0xcca; none
/// past 2
pub fn help(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    const TEXTS: [u32; 3] = [0xcc8, 0xcc9, 0xcca];
    if let Some(&t) = TEXTS.get(m.r32(item) as usize) {
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, t]);
    }
}

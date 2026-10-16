//! The front end's custom body screen (board row G9m): `cFE3DStateCustomBody`'s methods on the game's bytes, every
//! call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function
//! runner (`tests/fe_custombody.rs`).
//!
//! The screen picks one of the player's rider's seven outfits. +0x2e is the player (a signed byte); the player's
//! rider is the front end's +0x20 list at (the player's pick, +0x7d + 2 × player) mod 12 with C's signed remainder;
//! +0x54 keeps the rider's outfit (its +0x159) the screen opened on; +0x4c is the help line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_characcept::{CAREER, PLAYER_SLOT, SAVE_SELECTION};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{BACKDROP, TEXT_CTOR};
use crate::pf::Mem;

/// The front end's calls: 0x267cc0 whether an outfit is available (rider, player, outfit), 0x267c48 an outfit tried
/// on (rider, outfit), 0x26bcb8 the overlay's rider view (overlay, on), 0x262300 the custom screens' music, 0x28de90
/// the help line
pub const OUTFIT_AVAILABLE: u32 = 0x267cc0;
pub const TRY_OUTFIT: u32 = 0x267c48;
pub const RIDER_VIEW: u32 = 0x26bcb8;
pub const MUSIC_LOOP: u32 = 0x262300;
pub const HELP: u32 = 0x28de90;
/// The riders' outfit names (0x38 bytes a rider, a word every 8 bytes) and the help texts by outfit (six words)
pub const OUTFIT_NAMES: u32 = 0x332c88;
pub const HELP_TEXTS: u32 = 0x3afba0;
/// The item widgets' +0x118 word
pub const ITEM_WORD: u32 = 0x3ae520;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
}

/// The player's career slot asked for (0x164318; the answer is the caller's) and the player's rider
fn rider(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> (u32, u32) {
    let p = byte(m, s + 0x2e);
    let f = fe(m);
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p as u32]);
    let pick = byte(m, f.wrapping_add((2 * p) as u32).wrapping_add(0x7d));
    (slot, m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20)))
}

/// `cFE3DStateCustomBody` vf11 0x28d9c0, the screen built: the base widgets with the title (0x274208: 105.0, text
/// 0xb6, 0x200a); seven items, one per outfit (numbered, +0x118 the word at 0x3ae520, added to the menu, then the
/// rider's outfit name as text (+0x158) and whether the outfit is available (0x267cc0) to +0x100); the backdrop; no
/// fade in; the rider's outfit kept (+0x54) and the menu set on it (+0x160)
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let p = byte(m, s + 0x2e);
    let (_, r) = rider(m, e, s);
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xb6, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let word = m.r32(ITEM_WORD);
    for k in 0..7u32 {
        let at = e.call(m, ALLOC, &[0x3af520, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w, k);
        m.w32(w + 0x118, word);
        vcall(m, e, menu, 0x148, &[w]);
        let set = m.r32(r + 0xb0).wrapping_mul(0x38);
        let name = m.r32(set.wrapping_add(OUTFIT_NAMES).wrapping_add(8 * k));
        vcall(m, e, w, 0x158, &[name]);
        let open = e.call(m, OUTFIT_AVAILABLE, &[r, p as u32, k]);
        vcall(m, e, w, 0x100, &[open]);
    }
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
    let b = m.r8(r + 0x159);
    m.w8(s + 0x54, b);
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x160, &[b as i8 as i32 as u32]);
}

/// `cFE3DStateCustomBody::Activate` 0x28dbb8: the menu set on the rider's outfit (+0x160 with its +0x159), the help
/// line, the overlay's rider view off (0x26bcb8 with 0), the custom screens' music (0x262300), the base's `Activate`
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let (_, r) = rider(m, e, s);
    let menu = m.r32(s + 0x3c);
    let b = byte(m, r + 0x159) as u32;
    vcall(m, e, menu, 0x160, &[b]);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, HELP, &[s, chosen]);
    let o = m.r32(fe(m) + 0x50);
    e.call(m, RIDER_VIEW, &[o, 0]);
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    base::activate(m, e, s);
}

/// `cFE3DStateCustomBody` vf18 0x28dca0: the overlay's rider view on
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv) {
    let o = m.r32(fe(m) + 0x50);
    e.call(m, RIDER_VIEW, &[o, 1]);
}

/// `cFE3DStateCustomBody` vf21 0x28dcb8 (backing out): the rider put back in the outfit the screen opened on (0x267c48
/// with +0x54), then the base's vf21
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let (_, r) = rider(m, e, s);
    let b = byte(m, s + 0x54) as u32;
    e.call(m, TRY_OUTFIT, &[r, b]);
    base::vf21(m, e, s);
}

/// `cFE3DStateCustomBody` vf22 0x28dd50 (the item that goes, the one that comes): the base's vf22; with an item going,
/// the rider tries the coming outfit on (its signed first byte); the help line
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        let (_, r) = rider(m, e, s);
        let b = byte(m, to) as u32;
        e.call(m, TRY_OUTFIT, &[r, b]);
    }
    e.call(m, HELP, &[s, to]);
}

/// `cFE3DStateCustomBody` vf26 0x28ddf8 (accepting): the rider saves its selection (0x266190) with the first byte of
/// the player's career slot, then the base's vf21 (the screen's accept)
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let (slot, r) = rider(m, e, s);
    let b = byte(m, slot) as u32;
    e.call(m, SAVE_SELECTION, &[r, b]);
    base::vf21(m, e, s);
}

/// 0x28de90 (`FECustomBody_UpdateDesc`; the screen, the item): the help line's text: 0xcab for a locked item (+0xa8),
/// else the text for outfits 1..6 (the words at 0x3afba0) and 0xcab for any other; then the line's +0x140
pub fn help(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let text = if m.r32(item + 0xa8) != 0 {
        0xcab
    } else {
        let k = m.r32(item).wrapping_sub(1);
        if k < 6 {
            m.r32(HELP_TEXTS + 4 * k)
        } else {
            0xcab
        }
    };
    let h = m.r32(s + 0x4c);
    e.call(m, SET_TEXT, &[h, text]);
    let h = m.r32(s + 0x4c);
    vcall(m, e, h, 0x140, &[]);
}

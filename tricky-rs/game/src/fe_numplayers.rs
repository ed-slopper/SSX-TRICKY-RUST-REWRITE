//! The front end's number-of-players screen (board row G9d): `cFE3DStateNumPlayers`'s methods on the game's bytes,
//! every call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the
//! function runner (`tests/fe_numplayers.rs`).
//!
//! The menu is the title text (+0x3c): one player (item 0) or two (item 1), the chosen one at its +0x140's first word;
//! +0x4c is the help line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, LIST_NTH};
use crate::pf::Mem;

/// The front end's calls the screen makes: 0x263670 (the players, a byte), 0x265970 the next screen for them,
/// 0x26be38 the overlay's mode, 0x263270 a player's reset, 0x261b38 the front end's mode, 0x29ad28 the menu's
/// highlight, 0x164288 the profile's player count (0x32b390), the music (0x262cd8 the menu loop, 0x2628e0 / 0x262920
/// one or two players' cue), 0x2b0f68 a text widget's text, 0x274278 the base widgets with a title
pub const SET_PLAYERS: u32 = 0x263670;
pub const NEXT_FOR_PLAYERS: u32 = 0x265970;
pub const OVERLAY_MODE: u32 = 0x26be38;
pub const PLAYER_RESET: u32 = 0x263270;
pub const FE_MODE: u32 = 0x261b38;
pub const HIGHLIGHT: u32 = 0x29ad28;
pub const PROFILE_PLAYERS: u32 = 0x164288;
pub const MUSIC_MENU: u32 = 0x262cd8;
pub const MUSIC_ONE: u32 = 0x2628e0;
pub const MUSIC_TWO: u32 = 0x262920;
pub const SET_TEXT: u32 = 0x2b0f68;
pub const BUILD_TITLED: u32 = 0x274278;
/// The two items' text ids (a pair of words at 0x3b2a80) and their +0x118 word (0x3ae520)
pub const ITEM_TEXTS: u32 = 0x3b2a80;
pub const ITEM_WORD: u32 = 0x3ae520;

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

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// `cFE3DStateNumPlayers::CreateNextState` 0x29a8c8: the players set (the item + 1, as a signed byte), then the front
/// end's next screen for them
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let item = m.r32(m.r32(m.r32(s + 0x3c) + 0x140));
    let n = item.wrapping_add(1) as u8 as i8 as i32 as u32;
    let f = fe(m);
    e.call(m, SET_PLAYERS, &[f, n]);
    let f = fe(m);
    e.call(m, NEXT_FOR_PLAYERS, &[f, s])
}

/// `cFE3DStateNumPlayers` vf11 0x29a928, the screen built (`buf` 0x10 bytes of scratch): the base widgets with the
/// title (0x274278: 194.0, text 0xdac, 0x200a); the menu's two items (the texts at 0x3b2a80, their +0x118 the word
/// at 0x3ae520), each with its highlight colour (1, 0.953, 0.729, 0.106; +0x108), white (+0x110 and +0x50) and
/// numbered; the backdrop; no fade in, the pause overlay allowed
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let one = 1.0f32.to_bits();
    e.call(m, BUILD_TITLED, &[194.0f32.to_bits(), s, 0xdac, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let texts = [m.r32(ITEM_TEXTS), m.r32(ITEM_TEXTS + 4)];
    let word = m.r32(ITEM_WORD);
    for (k, id) in texts.iter().enumerate() {
        let at = e.call(m, ALLOC, &[0x3b2a48, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, crate::fe_title::TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w + 0x118, word);
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
    e.call(m, crate::fe_title::BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
    m.w32(s + 0x28, 1);
}

/// `cFE3DStateNumPlayers::Activate` 0x29ab48: the overlay's mode 2, the base's `Activate`, the fade out 1; when the
/// game booted here (0x265af8) the menu's +0x160 by the profile's player count (one: 0, else 1), the base's vf22
/// from the menu's first item to the chosen one, and the screen's +0x40; then the menu's highlight (0x29ad28)
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let o = m.r32(fe(m) + 0x50);
    e.call(m, OVERLAY_MODE, &[o, 2]);
    base::activate(m, e, s);
    m.w32(s + 0x24, 1);
    let f = fe(m);
    if e.call(m, crate::fe_title::BOOTED, &[f]) != 0 {
        let n = e.call(m, PROFILE_PLAYERS, &[0x32b390]);
        let menu = m.r32(s + 0x3c);
        vcall(m, e, menu, 0x160, &[if n == 1 { 0 } else { 1 }]);
        let menu = m.r32(s + 0x3c);
        let first = e.call(m, LIST_NTH, &[menu + 0x118, 0]);
        let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
        base::vf22(m, e, first, chosen);
        vcall(m, e, s, 0x40, &[]);
    }
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, HIGHLIGHT, &[s, chosen]);
}

/// `cFE3DStateNumPlayers` vf19 0x29ac50: the menu music (0x262cd8), the 3D scene's +0x10(0xd)
pub fn vf19(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    e.call(m, MUSIC_MENU, &[f + 0x70]);
    scene_vcall(m, e, 0x10, &[0xd]);
}

/// `cFE3DStateNumPlayers` vf20 0x29ac68: both players reset (0x263270 on the front end's +0x7c and +0x7e), the front
/// end's mode (0, 1, 4), the overlay's mode 6, the 3D scene's +0x78(1)
pub fn vf20(m: &mut Mem, e: &mut dyn FeEnv) {
    for k in 0..2u32 {
        let f = fe(m);
        e.call(m, PLAYER_RESET, &[f + 0x7c + 2 * k]);
    }
    let f = fe(m);
    e.call(m, FE_MODE, &[f, 0, 1, 4]);
    let o = m.r32(fe(m) + 0x50);
    e.call(m, OVERLAY_MODE, &[o, 6]);
    scene_vcall(m, e, 0x78, &[1]);
}

/// `cFE3DStateNumPlayers` vf22 0x29ac80 (the item that goes, the one that comes): the base's vf22; with an item going,
/// the one or two players' music cue by the one that comes; the help line for it ([`highlight`], 0x29ad28)
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        let f = fe(m);
        let cue = if m.r32(to) == 0 { MUSIC_ONE } else { MUSIC_TWO };
        e.call(m, cue, &[f + 0x70]);
    }
    e.call(m, HIGHLIGHT, &[s, to]);
}

/// 0x29ad28 (the screen, the item): the help line's text (+0x4c; 0x2b0f68) for the item, 0xca6 for one player,
/// 0xca7 for two
pub fn highlight(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let help = m.r32(s + 0x4c);
    let text = if m.r32(item) == 0 { 0xca6 } else { 0xca7 };
    e.call(m, SET_TEXT, &[help, text]);
}

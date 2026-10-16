//! The front end's character accept (board row G9j): `cFE3DStateCharacterAccept`'s methods on the game's bytes, every
//! call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function
//! runner (`tests/fe_characcept.rs`).
//!
//! The screen confirms one player's rider: +0x2e is the player (a signed byte), +0x2c and +0x2d the bytes the
//! character screens hand on; +0x4c is the help line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use crate::pf::Mem;

pub const MODIFY_VTABLE: u32 = 0x3b03b0;
/// The career block and its calls: 0x164318 a player's slot, 0x1637f8 the players' records merged, 0x161a98 the
/// profile's audio settings applied; the profile word at 0x32e5f0
pub const CAREER: u32 = 0x32b390;
pub const PLAYER_SLOT: u32 = 0x164318;
pub const MERGE_RECORDS: u32 = 0x1637f8;
pub const APPLY_AUDIO: u32 = 0x161a98;
pub const PROFILE_WORD: u32 = 0x32e5f0;
/// The front end's calls: 0x262228 the accept music, 0x266190 a rider's selection saved, 0x27a240 the help line
pub const MUSIC_LOOP: u32 = 0x262228;
pub const SAVE_SELECTION: u32 = 0x266190;
pub const HELP: u32 = 0x27a240;
/// The menu's text ids (one word at 0x3af6b8)
pub const ITEM_TEXTS: u32 = 0x3af6b8;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
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

/// `cFE3DStateCharacterAccept::CreateNextState` 0x279e30: the character modify screen (0x54 bytes, vtable 0x3b03b0)
/// with the screen's three bytes (+0x2d, +0x2c, +0x2e) handed on
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let at = e.call(m, ALLOC, &[0x3af618, 0x54, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, 0x54]);
    }
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0xc), MODIFY_VTABLE);
    for o in [0x2d, 0x2c, 0x2e] {
        let b = m.r8(s + o);
        m.w8(at.wrapping_add(o), b);
    }
    at
}

/// `cFE3DStateCharacterAccept` vf11 0x279ec0, the screen built: the base widgets with the title (0x274208: 105.0;
/// text 0xdae for player one, 0xdaf for the second; 0x200a); the menu's one item (its text the word at 0x3af6b8,
/// +0x118 1.0, numbered 0); the backdrop; a fade in
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let title = if m.r8(s + 0x2e) != 0 { 0xdaf } else { 0xdae };
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, title, 0x200a]);
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
    m.w32(s + 0x20, 1);
}

/// `cFE3DStateCharacterAccept::Activate` 0x27a000: the accept music (0x262228), the 3D scene's +0x58(1), the help
/// line, the base's `Activate`; the profile word 0x10 becomes 2
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    scene_vcall(m, e, 0x58, &[1]);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, HELP, &[s, chosen]);
    base::activate(m, e, s);
    if m.r32(PROFILE_WORD) == 0x10 {
        m.w32(PROFILE_WORD, 2);
    }
}

/// `cFE3DStateCharacterAccept::Deactivate` 0x27a0a0: the player's rider (the front end's +0x20 list at the player's
/// pick, its +0x7d + 2 × player, mod 12) saves the selection (0x266190) with the first byte of the player's career
/// slot; the 3D scene's +0x58(0); the base's `Deactivate`
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let p = byte(m, s + 0x2e);
    let f = fe(m);
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p as u32]);
    let pick = byte(m, f.wrapping_add((2 * p) as u32).wrapping_add(0x7d));
    let rider = m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20));
    let b = byte(m, slot) as u32;
    e.call(m, SAVE_SELECTION, &[rider, b]);
    scene_vcall(m, e, 0x58, &[0]);
    base::deactivate(m, e, s);
}

/// `cFE3DStateCharacterAccept` vf22 0x27a158: the help line, then the base's vf22
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    e.call(m, HELP, &[s, to]);
    base::vf22(m, e, from, to);
}

/// `cFE3DStateCharacterAccept` vf26 0x27a1a8: the player's career slot asked (0x164318); when this is the last player
/// (the front end's count at +4, a signed byte), the players' records merged and the profile's audio applied; the
/// base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    let p = byte(m, s + 0x2e);
    e.call(m, PLAYER_SLOT, &[CAREER, p as u32]);
    if byte(m, s + 0x2e) == byte(m, f + 4) - 1 {
        e.call(m, MERGE_RECORDS, &[CAREER]);
        e.call(m, APPLY_AUDIO, &[CAREER]);
    }
    base::vf26(m, e, s);
}

/// 0x27a240 (the screen): the help line's text 0xca9 (0x2b0f68), then its +0x140
pub fn help(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let h = m.r32(s + 0x4c);
    e.call(m, SET_TEXT, &[h, 0xca9]);
    let h = m.r32(s + 0x4c);
    vcall(m, e, h, 0x140, &[]);
}

//! The front end's mode select (board row G9e): `cFE3DStateModeSelect`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function runner
//! (`tests/fe_modeselect.rs`).
//!
//! The menu (+0x3c) has three items: 0 the World Circuit and single events, 1 free ride / practice, 2 the trick book
//! and two players; +0x4c is the help line. The game mode word (0x32de8c) picks the item the screen opens on.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_numplayers::{MUSIC_MENU, NEXT_FOR_PLAYERS, OVERLAY_MODE, SET_PLAYERS, SET_TEXT};
use crate::fe_title::{BACKDROP, BASE_CTOR, BOOTED, TEXT_CTOR};
use crate::pf::Mem;

pub const EVENT_SEL_VTABLE: u32 = 0x3b2fd0;
/// The game mode word
pub const GAME_MODE: u32 = 0x32de8c;
/// The front end's calls: 0x265a58 the number-of-players screen, 0x265870 the players joined, the music (0x2623d8 the
/// mode select's loop, 0x262bd0 / 0x262cd8 / 0x262ad0 the cues of vf18 / vf19 / vf20, 0x262840 / 0x2627f0 /
/// 0x262890 the three items' cues), 0x274208 the base widgets with a title, 0x29a7a8 the help line
pub const NUM_PLAYERS_SCREEN: u32 = 0x265a58;
pub const PLAYERS_JOINED: u32 = 0x265870;
pub const MUSIC_LOOP: u32 = 0x2623d8;
pub const MUSIC_VF18: u32 = 0x262bd0;
pub const MUSIC_VF20: u32 = 0x262ad0;
pub const MUSIC_ITEM0: u32 = 0x2627f0;
pub const MUSIC_ITEM1: u32 = 0x262840;
pub const MUSIC_ITEM2: u32 = 0x262890;
pub const BUILD_TITLED: u32 = 0x274208;
pub const HELP: u32 = 0x29a7a8;
/// The three items' text ids
pub const ITEM_TEXTS: [u32; 3] = [0xe0a, 0xe0b, 0xe0c];

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn item_of(m: &Mem, s: u32) -> u32 {
    m.r32(m.r32(m.r32(s + 0x3c) + 0x140))
}

/// `cFE3DStateModeSelect::CreateNextState` 0x29a258: item 0 the event select (0x54 bytes, vtable 0x3b2fd0), item 1
/// the front end's number-of-players screen (0x265a58), item 2 one player set and the front end's next screen for it
/// (the character select); else none
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    match item_of(m, s) {
        1 => {
            let f = fe(m);
            e.call(m, NUM_PLAYERS_SCREEN, &[f, s])
        }
        0 => {
            let at = e.call(m, ALLOC, &[0x3b2a28, 0x54, 0]);
            if at != 0 {
                e.call(m, FILL, &[at, 0xdead_c0de, 0x54]);
            }
            e.call(m, BASE_CTOR, &[at]);
            m.w32(at.wrapping_add(0xc), EVENT_SEL_VTABLE);
            at
        }
        2 => {
            let f = fe(m);
            e.call(m, SET_PLAYERS, &[f, 1]);
            let f = fe(m);
            e.call(m, NEXT_FOR_PLAYERS, &[f, s])
        }
        _ => 0,
    }
}

/// `cFE3DStateModeSelect` vf11 0x29a330, the screen built: the base widgets with the title (0x274208: 105.0, text
/// 0xdab, 0x200a); the menu's layout bytes (+0x110..+0x116: 3, 0, 1, 2, 1, 1, 3); its three items (texts 0xe0a,
/// 0xe0b, 0xe0c, +0x118 1.0, numbered); the backdrop; the pause overlay off; the menu's +0x160(1)
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xdab, 0x200a]);
    for (o, b) in [(0x111u32, 0u8), (0x110, 3), (0x116, 3), (0x112, 1), (0x113, 2), (0x114, 1), (0x115, 1)] {
        let menu = m.r32(s + 0x3c);
        m.w8(menu + o, b);
    }
    let menu = m.r32(s + 0x3c);
    for (k, id) in ITEM_TEXTS.iter().enumerate() {
        let at = e.call(m, ALLOC, &[0x3b2a48, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w + 0x118, 1.0f32.to_bits());
        vcall(m, e, w, 0x150, &[*id]);
        m.w32(w, k as u32);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x28, 0);
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x160, &[1]);
}

/// `cFE3DStateModeSelect::Activate` 0x29a4e0: the mode select's music, the base's `Activate`; when the game booted
/// here (0x265af8), the menu opened on the item the game mode gives (1, 6: item 0; 2, 3, 7: item 1; 4, 5: item 2;
/// else none) and the screen's +0x40; then the screen's +0xb8 with the chosen item twice, and the help line
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    base::activate(m, e, s);
    let f = fe(m);
    if e.call(m, BOOTED, &[f]) != 0 {
        let item = match m.r32(GAME_MODE) {
            1 | 6 => Some(0),
            2 | 3 | 7 => Some(1),
            4 | 5 => Some(2),
            _ => None,
        };
        if let Some(k) = item {
            let menu = m.r32(s + 0x3c);
            vcall(m, e, menu, 0x160, &[k]);
        }
        vcall(m, e, s, 0x40, &[]);
    }
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    vcall(m, e, s, 0xb8, &[chosen, chosen]);
    e.call(m, HELP, &[s, chosen]);
}

/// 0x29a7a8 (the screen, the item): the help line's text (+0x4c; 0x2b0f68) for the item: 0 0xca3, 1 0xca1, 2 0xca2
pub fn help(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let text = match m.r32(item) as i32 {
        0 => 0xca3,
        1 => 0xca1,
        2 => 0xca2,
        _ => return,
    };
    let h = m.r32(s + 0x4c);
    e.call(m, SET_TEXT, &[h, text]);
}

/// `cFE3DStateModeSelect` vf18 0x29a600: the music's cue 0x262bd0
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    e.call(m, MUSIC_VF18, &[f + 0x70]);
}

/// `cFE3DStateModeSelect` vf19 0x29a618: the menu music (0x262cd8)
pub fn vf19(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    e.call(m, MUSIC_MENU, &[f + 0x70]);
}

/// `cFE3DStateModeSelect` vf20 0x29a630: leaving on item 2, or item 1 with fewer than two players joined (0x265870),
/// the music's cue 0x262ad0
pub fn vf20(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let item = item_of(m, s) as i32;
    if item != 2 {
        if item != 1 {
            return;
        }
        let f = fe(m);
        if 1 < e.call(m, PLAYERS_JOINED, &[f]) as i32 {
            return;
        }
    }
    let f = fe(m);
    e.call(m, MUSIC_VF20, &[f + 0x70]);
}

/// `cFE3DStateModeSelect` vf22 0x29a6b0 (the item that goes, the one that comes): with none going, item 1's cue and
/// the front end's +0xf0 0; else by the item that comes: 0 (+0xf0 1, overlay mode 4, its cue), 1 (+0xf0 0, mode 5,
/// its cue), 2 (+0xf0 2, mode 3, its cue); then the help line and the base's vf22
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    let f = fe(m);
    if from == 0 {
        e.call(m, MUSIC_ITEM1, &[f + 0x70]);
        m.w32(f + 0xf0, 0);
    } else {
        let k = m.r32(to) as i32;
        let o = m.r32(f + 0x50);
        match k {
            1 => {
                m.w32(f + 0xf0, 0);
                e.call(m, OVERLAY_MODE, &[o, 5]);
                e.call(m, MUSIC_ITEM1, &[f + 0x70]);
            }
            0 => {
                m.w32(f + 0xf0, 1);
                e.call(m, OVERLAY_MODE, &[o, 4]);
                e.call(m, MUSIC_ITEM0, &[f + 0x70]);
            }
            2 => {
                m.w32(f + 0xf0, 2);
                e.call(m, OVERLAY_MODE, &[o, 3]);
                e.call(m, MUSIC_ITEM2, &[f + 0x70]);
            }
            _ => {}
        }
    }
    e.call(m, HELP, &[s, to]);
    base::vf22(m, e, from, to);
}

/// `cFE3DStateModeSelect` vf26 0x29a800 (the item picked): item 1 fades out and keeps the pause overlay with fewer
/// than two players joined; item 0 no fade out; item 2 both on; then the base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    match m.r32(item) as i32 {
        1 => {
            let f = fe(m);
            let n = e.call(m, PLAYERS_JOINED, &[f]) as i32;
            m.w32(s + 0x24, (n < 2) as u32);
            let n = e.call(m, PLAYERS_JOINED, &[f]) as i32;
            m.w32(s + 0x28, (n < 2) as u32);
        }
        0 => m.w32(s + 0x24, 0),
        2 => {
            m.w32(s + 0x28, 1);
            m.w32(s + 0x24, 1);
        }
        _ => {}
    }
    base::vf26(m, e, s);
}

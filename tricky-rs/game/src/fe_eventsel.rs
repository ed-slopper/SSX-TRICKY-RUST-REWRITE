//! The front end's event select (board row G9f): `cFE3DStateEventSel`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function runner
//! (`tests/fe_eventsel.rs`).
//!
//! The menu (+0x3c) holds the items the game type allows (the front end's +0xf0, as the mode select set it: 0 gives
//! items 2, 3, 0; 1 gives 1, 4; 2 gives 2, 3), each item numbered 0..4 with its own text, camera, game mode and help
//! line. +0x4c is the help line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_modeselect::{MUSIC_VF20, NUM_PLAYERS_SCREEN, PLAYERS_JOINED};
use crate::fe_numplayers::{BUILD_TITLED, NEXT_FOR_PLAYERS, OVERLAY_MODE, SET_PLAYERS, SET_TEXT};
use crate::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use crate::pf::Mem;

pub const DIFFICULT_SEL_VTABLE: u32 = 0x3b2ef0;
pub const VENUE_SEL_VTABLE: u32 = 0x3b2e10;
/// The game mode word
pub const GAME_MODE: u32 = crate::fe_modeselect::GAME_MODE;
/// The music: 0x262388 the event select's loop, 0x262c60 vf18's cue, the five items' cues (0x262160, 0x2620e0,
/// 0x262060, 0x2620a0, 0x262120)
pub const MUSIC_LOOP: u32 = 0x262388;
pub const MUSIC_VF18: u32 = 0x262c60;
pub const MUSIC_ITEMS: [u32; 5] = [0x262160, 0x2620e0, 0x262060, 0x2620a0, 0x262120];
pub const HELP: u32 = 0x29b5d0;
/// The items' text ids by item number
pub const ITEM_TEXTS: [u32; 5] = [0xe85, 0xe04, 0xe05, 0xe06, 0xe07];
/// The item lists by game type (practice 0x3b2ad0, single event 0x3b2ae0, World Circuit 0x3b2ae8)
pub const LISTS: [u32; 3] = [0x3b2ad0, 0x3b2ae0, 0x3b2ae8];

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

fn item_of(m: &Mem, s: u32) -> u32 {
    m.r32(m.r32(m.r32(s + 0x3c) + 0x140))
}

fn new_screen(m: &mut Mem, e: &mut dyn FeEnv, size: u32, vtable: u32) -> u32 {
    let at = e.call(m, ALLOC, &[0x3b2a88, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0xc), vtable);
    at
}

/// `cFE3DStateEventSel::CreateNextState` 0x29ad48: the 3D scene's +0x10 with the item's camera, the game mode word
/// and the next screen by the item and the game type (the front end's +0xf0, read first):
/// - 0: camera 0xc, mode 7, game type 0;
/// - 1: camera 10, mode 1, game type 4, the number-of-players screen (0x265a58);
/// - 2: camera 8; practice: mode 2 and the difficulty select (0x54 bytes, vtable 0x3b2ef0); World Circuit: mode 4;
/// - 3: camera 9; practice: mode 3; World Circuit: mode 5;
/// - 4: camera 0xb, mode 6, game type 3, one player set and the front end's next screen for it.
///
/// With no screen made by then (or any other item), the venue select (0x7c bytes, vtable 0x3b2e10).
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let f = fe(m);
    let kind = m.r32(f + 0xf0) as i32;
    let mut next = 0;
    match item_of(m, s) {
        0 => {
            scene_vcall(m, e, 0x10, &[0xc]);
            m.w32(GAME_MODE, 7);
            m.w32(f + 0xf0, 0);
        }
        1 => {
            scene_vcall(m, e, 0x10, &[10]);
            m.w32(GAME_MODE, 1);
            m.w32(f + 0xf0, 4);
            next = e.call(m, NUM_PLAYERS_SCREEN, &[f, s]);
        }
        2 => {
            scene_vcall(m, e, 0x10, &[8]);
            match kind {
                0 => {
                    m.w32(GAME_MODE, 2);
                    next = new_screen(m, e, 0x54, DIFFICULT_SEL_VTABLE);
                }
                2 => m.w32(GAME_MODE, 4),
                _ => {}
            }
        }
        3 => {
            scene_vcall(m, e, 0x10, &[9]);
            match kind {
                0 => m.w32(GAME_MODE, 3),
                2 => m.w32(GAME_MODE, 5),
                _ => {}
            }
        }
        4 => {
            scene_vcall(m, e, 0x10, &[0xb]);
            m.w32(GAME_MODE, 6);
            m.w32(f + 0xf0, 3);
            e.call(m, SET_PLAYERS, &[f, 1]);
            let f = fe(m);
            next = e.call(m, NEXT_FOR_PLAYERS, &[f, s]);
        }
        _ => {}
    }
    if next == 0 {
        next = new_screen(m, e, 0x7c, VENUE_SEL_VTABLE);
    }
    next
}

/// `cFE3DStateEventSel` vf11 0x29b000, the screen built (`buf` 0x10 bytes of scratch): the game type's item list
/// (practice three, single event and World Circuit two, any other none); the base widgets with the title
/// (0x274278: 224.0 for three items, else 194.0; text 0xdaa, 0x200a); each item (its text, +0x118 1.0, the
/// highlight colour (1, 0.953, 0.729, 0.106; +0x108), white (+0x110 and +0x50), its number); the backdrop
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (n, list) = match m.r32(fe(m) + 0xf0) as i32 {
        0 => (3, LISTS[0]),
        1 => (2, LISTS[1]),
        2 => (2, LISTS[2]),
        _ => (0, 0),
    };
    let title = if n == 3 { 224.0f32 } else { 194.0 };
    e.call(m, BUILD_TITLED, &[title.to_bits(), s, 0xdaa, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let one = 1.0f32.to_bits();
    for k in 0..n {
        let at = e.call(m, ALLOC, &[0x3b2a48, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        let item = m.r32(list + 4 * k);
        m.w32(w + 0x118, one);
        vcall(m, e, w, 0x150, &[ITEM_TEXTS[item as usize]]);
        put4(m, buf, [one, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
        vcall(m, e, w, 0x108, &[buf]);
        put4(m, buf, [one; 4]);
        vcall(m, e, w, 0x110, &[buf]);
        put4(m, buf, [one; 4]);
        vcall(m, e, w, 0x50, &[buf]);
        let item = m.r32(list + 4 * k);
        m.w32(w, item);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, BACKDROP, &[s]);
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// Item 4, or item 1 with fewer than two players joined (0x265870): the items that leave the front end's 3D
/// scene (Activate's overlay mode 2, vf20's music)
fn leaves_scene(m: &mut Mem, e: &mut dyn FeEnv, item: u32) -> bool {
    match item {
        4 => true,
        1 => {
            let f = fe(m);
            (e.call(m, PLAYERS_JOINED, &[f]) as i32) < 2
        }
        _ => false,
    }
}

/// `cFE3DStateEventSel::Activate` 0x29b2c8 (the screen, the one before): the overlay's mode 2 when the chosen item
/// leaves the scene; no fade in (+0x20 0) when the screen before answers 1 from its +0x70; the event select's music
/// (0x262388); the base's `Activate`; the help line (0x29b5d0)
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    let item = item_of(m, s);
    if leaves_scene(m, e, item) {
        let o = m.r32(fe(m) + 0x50);
        e.call(m, OVERLAY_MODE, &[o, 2]);
    }
    if prev != 0 && vcall(m, e, prev, 0x70, &[]) == 1 {
        m.w32(s + 0x20, 0);
    }
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    base::activate(m, e, s);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, HELP, &[s, chosen]);
}

/// `cFE3DStateEventSel` vf18 0x29b3a8: for practice and the World Circuit, the music's cue 0x262c60
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    let kind = m.r32(f + 0xf0);
    if kind == 0 || kind == 2 {
        e.call(m, MUSIC_VF18, &[f + 0x70]);
    }
}

/// `cFE3DStateEventSel` vf20 0x29b3d8: leaving on an item that leaves the scene, the music's 0x262ad0
pub fn vf20(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let item = item_of(m, s);
    if leaves_scene(m, e, item) {
        let f = fe(m);
        e.call(m, MUSIC_VF20, &[f + 0x70]);
    }
}

/// `cFE3DStateEventSel` vf22 0x29b458 (the item that goes, the one that comes): the base's vf22; with an item going,
/// the cue of the one that comes; the help line
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        if let Some(&cue) = MUSIC_ITEMS.get(m.r32(to) as usize) {
            let f = fe(m);
            e.call(m, cue, &[f + 0x70]);
        }
    }
    e.call(m, HELP, &[s, to]);
}

/// `cFE3DStateEventSel` vf26 0x29b550 (the item picked): fade out (+0x24) only for an item that leaves the scene;
/// then the base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    m.w32(s + 0x24, 0);
    let k = m.r32(item);
    if leaves_scene(m, e, k) {
        m.w32(s + 0x24, 1);
    }
    base::vf26(m, e, s);
}

/// 0x29b5d0 (the screen, the item): the help line's text (+0x4c; 0x2b0f68) by the item: 0xccc, 0xca4, 0xcc7, 0xccb,
/// 0xca5; none past 4
pub fn help(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    const TEXTS: [u32; 5] = [0xccc, 0xca4, 0xcc7, 0xccb, 0xca5];
    if let Some(&t) = TEXTS.get(m.r32(item) as usize) {
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, t]);
    }
}

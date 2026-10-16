//! The front end's character modify screen (board row G9k): `cFE3DStateCharacterModify`'s methods on the game's
//! bytes, every call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in
//! the function runner (`tests/fe_charmodify.rs`).
//!
//! The menu has six items: 0 done, and 1..5 the screens that change the rider (the custom body, the custom board and
//! three more, each made here). +0x2e is the player (a signed byte); +0x2c and +0x2d are handed on to the screen made.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_characcept::CAREER;
use crate::fe_eventsel::VENUE_SEL_VTABLE;
use crate::fe_modeselect::EVENT_SEL_VTABLE;
use crate::fe_numplayers::{BUILD_TITLED, NEXT_FOR_PLAYERS, SET_TEXT};
use crate::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use crate::fe_venuesel::SETUP_RACE;
use crate::pf::Mem;

/// The screens items 1..5 make: their heap tags, sizes, the 3D scene's camera for each, and their vtables (item 2's
/// is made by its own constructor, 0x28c690)
pub const BODY_VTABLE: u32 = 0x3afe70;
pub const ITEM3_VTABLE: u32 = 0x3b0110;
pub const ITEM4_VTABLE: u32 = 0x3b01f0;
pub const USER_NAME_VTABLE: u32 = 0x3b2d30;
pub const BOARD_CTOR: u32 = 0x28c690;
/// 0x23bb70 a list's set up (item 4's +0x68), 0x164950 a player's rider slot set (career, player, slot)
pub const LIST_SETUP: u32 = 0x23bb70;
pub const SET_RIDER_SLOT: u32 = 0x164950;
/// The front end's calls: 0x262260 the modify music, 0x261b38 the selected rider set (fe, player, players, pick),
/// 0x26b9f0 the overlay's rider shot, 0x267b18 a rider's choice restored, 0x2661e8 a rider's show state, 0x262b78
/// vf20's music, and the five items' lines (0x261f20, 0x261f60, 0x261fa0, 0x261fe0, 0x262020)
pub const MUSIC_LOOP: u32 = 0x262260;
pub const SET_SELECTED: u32 = 0x261b38;
pub const RIDER_SHOT: u32 = 0x26b9f0;
pub const RESTORE_CHOICE: u32 = 0x267b18;
pub const RIDER_SHOW: u32 = 0x2661e8;
pub const MUSIC_VF20: u32 = 0x262b78;
pub const ITEM_LINES: [u32; 5] = [0x261f20, 0x261f60, 0x261fa0, 0x261fe0, 0x262020];
/// The six items' text ids
pub const ITEM_TEXTS: u32 = 0x3af688;
/// The item widgets' +0x118 word
pub const ITEM_WORD: u32 = 0x3ae520;

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

fn new_screen(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    e.call(m, BASE_CTOR, &[at]);
    at
}

/// The three bytes handed on (+0x2d, +0x2e, +0x2c, in that order)
fn hand_on(m: &mut Mem, s: u32, at: u32) {
    for o in [0x2d, 0x2e, 0x2c] {
        let b = m.r8(s + o);
        m.w8(at.wrapping_add(o), b);
    }
}

/// `cFE3DStateCharacterModify::CreateNextState` 0x279468: by the item chosen:
/// - 0 (done): for the last player (+0x2e + 1 is the front end's count at +4), by the game type (+0xf0): 4 the venue
///   select (0x7c bytes), 3 the 3D scene's +0x60 and +0x10(-1) and the race set up (no screen), else the event select
///   (0x54 bytes). For any other player, the front end's next screen (0x265970) for the next player, its +0x2e that
///   player, its rider slot set (0x164950) and its +0x2d: -2 after a +0x2d of 1 (slot 1), else !+0x2d (slot 0);
/// - 1..5: the 3D scene's camera (0x1e, 0x1f, 0x20, 0x22, 0x23) and the item's screen, with the three bytes handed
///   on;
/// - any other: none.
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let f = fe(m);
    let item = m.r32(m.r32(m.r32(s + 0x3c) + 0x140));
    match item {
        0 => {
            let next_player = byte(m, s + 0x2e) + 1;
            if next_player == byte(m, f + 4) {
                match m.r32(f + 0xf0) {
                    4 => {
                        let at = new_screen(m, e, 0x3af5f8, 0x7c);
                        m.w32(at.wrapping_add(0xc), VENUE_SEL_VTABLE);
                        at
                    }
                    3 => {
                        scene_vcall(m, e, 0x60, &[]);
                        scene_vcall(m, e, 0x10, &[u32::MAX]);
                        e.call(m, SETUP_RACE, &[f]);
                        0
                    }
                    _ => {
                        let at = new_screen(m, e, 0x3af608, 0x54);
                        m.w32(at.wrapping_add(0xc), EVENT_SEL_VTABLE);
                        at
                    }
                }
            } else {
                let v = next_player as i8;
                let at = e.call(m, NEXT_FOR_PLAYERS, &[f, s]);
                m.w8(at.wrapping_add(0x2e), v as u8);
                let b = byte(m, s + 0x2d);
                if b == 1 {
                    e.call(m, SET_RIDER_SLOT, &[CAREER, v as i32 as u32, 1]);
                    m.w8(at.wrapping_add(0x2d), 0xfe);
                } else {
                    e.call(m, SET_RIDER_SLOT, &[CAREER, v as i32 as u32, 0]);
                    m.w8(at.wrapping_add(0x2d), !(b as u8));
                }
                at
            }
        }
        1 => {
            scene_vcall(m, e, 0x10, &[0x1e]);
            let at = new_screen(m, e, 0x3af658, 0x58);
            m.w8(at.wrapping_add(0x54), 0);
            m.w32(at.wrapping_add(0xc), BODY_VTABLE);
            hand_on(m, s, at);
            at
        }
        2 => {
            scene_vcall(m, e, 0x10, &[0x1f]);
            let raw = e.call(m, ALLOC, &[0x3af648, 0x90, 0]);
            if raw != 0 {
                e.call(m, FILL, &[raw, 0xdead_c0de, 0x90]);
            }
            let at = e.call(m, BOARD_CTOR, &[raw]);
            hand_on(m, s, at);
            at
        }
        3 => {
            scene_vcall(m, e, 0x10, &[0x20]);
            let at = new_screen(m, e, 0x3af638, 0x50c);
            m.w32(at.wrapping_add(0x54), 0);
            m.w32(at.wrapping_add(0x4ec), 0);
            m.w32(at.wrapping_add(0xc), ITEM3_VTABLE);
            hand_on(m, s, at);
            at
        }
        4 => {
            scene_vcall(m, e, 0x10, &[0x22]);
            let at = new_screen(m, e, 0x3af628, 0x114);
            for o in [0x54u32, 0x58, 0x5c] {
                m.w32(at.wrapping_add(o), 0);
            }
            m.w32(at.wrapping_add(0xc), ITEM4_VTABLE);
            m.w32(at.wrapping_add(0x64), u32::MAX);
            m.w32(at.wrapping_add(0x60), u32::MAX);
            e.call(m, LIST_SETUP, &[at.wrapping_add(0x68)]);
            for k in 0..7u32 {
                m.w32(at.wrapping_add(0xf4 + 4 * k), 0);
            }
            hand_on(m, s, at);
            at
        }
        5 => {
            scene_vcall(m, e, 0x10, &[0x23]);
            let at = new_screen(m, e, 0x3af618, 0x78);
            m.w32(at.wrapping_add(0x68), 1);
            m.w32(at.wrapping_add(0x6c), 0);
            m.w32(at.wrapping_add(0xc), USER_NAME_VTABLE);
            m.w32(at.wrapping_add(0x70), 0);
            for o in [0x2d, 0x2c, 0x2e] {
                let b = m.r8(s + o);
                m.w8(at.wrapping_add(o), b);
            }
            m.w32(at.wrapping_add(0x6c), 0);
            at
        }
        _ => 0,
    }
}

/// `cFE3DStateCharacterModify` vf11 0x279908, the screen built (`buf` 0x10 bytes of scratch): the base widgets with
/// the title (0x274278: 314.0; text 0xdae for the first player, 0xdaf for the second; 0x200a); six items (the texts
/// at 0x3af688, +0x118 the word at 0x3ae520, white (+0x50), numbered); the backdrop; no fade in
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let title = if m.r8(s + 0x2e) != 0 { 0xdaf } else { 0xdae };
    e.call(m, BUILD_TITLED, &[314.0f32.to_bits(), s, title, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let word = m.r32(ITEM_WORD);
    for k in 0..6u32 {
        let at = e.call(m, ALLOC, &[0x3af520, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        let id = m.r32(ITEM_TEXTS + 4 * k);
        m.w32(w + 0x118, word);
        vcall(m, e, w, 0x150, &[id]);
        for j in 0..4 {
            m.w32(buf + 4 * j, 1.0f32.to_bits());
        }
        vcall(m, e, w, 0x50, &[buf]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
}

/// `cFE3DStateCharacterModify::Activate` 0x279a80 (the screen, the one before): coming back from a screen that answers
/// 7 from its +0x70, in game type 3 with that screen's +0xf4 clear, only the race set up (0x2657a8). Else the modify
/// music, the selected rider set (the player, the count, the player's pick) and the overlay's rider shot of the pick;
/// after another player's screen (its +0x2e not this player), all twelve riders' choices restored first when it
/// answers 4 from its +0x70, then the pick's rider shown (0x2661e8 with 2); then the base's `Activate`
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    if vcall(m, e, prev, 0x70, &[]) == 7 {
        let f = fe(m);
        if m.r32(f + 0xf0) == 3 && m.r32(prev + 0xf4) == 0 {
            e.call(m, SETUP_RACE, &[f]);
            return;
        }
    }
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    let p = byte(m, s + 0x2e);
    let n = byte(m, f + 4);
    let pick = byte(m, f.wrapping_add((2 * p) as u32).wrapping_add(0x7d));
    let f2 = fe(m);
    e.call(m, SET_SELECTED, &[f2, p as u32, n as u32, pick as u32]);
    let o = m.r32(f + 0x50);
    e.call(m, RIDER_SHOT, &[o, pick as u32]);
    if prev != 0 && byte(m, prev + 0x2e) != p {
        if vcall(m, e, prev, 0x70, &[]) == 4 {
            for k in 0..12u32 {
                let r = m.r32(f + 0x20 + 4 * k);
                e.call(m, RESTORE_CHOICE, &[r, p as u32]);
            }
        }
        let r = m.r32(f.wrapping_add(0x20).wrapping_add((4 * pick) as u32));
        e.call(m, RIDER_SHOW, &[r, 2]);
    }
    base::activate(m, e, s);
}

/// `cFE3DStateCharacterModify` vf20 0x279c10: leaving on "done" as the last player, outside game type 3, the 3D
/// scene's +0x58(0) and the music's 0x262b78
pub fn vf20(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r32(m.r32(m.r32(s + 0x3c) + 0x140)) != 0 {
        return;
    }
    let f = fe(m);
    if byte(m, s + 0x2e) + 1 != byte(m, f + 4) || m.r32(f + 0xf0) == 3 {
        return;
    }
    scene_vcall(m, e, 0x58, &[0]);
    let f = fe(m);
    e.call(m, MUSIC_VF20, &[f + 0x70]);
}

/// `cFE3DStateCharacterModify` vf22 0x279ca8 (the item that goes, the one that comes): the base's vf22; the help line
/// by the coming item (0: the 3D scene's +0x10(-1) and 0xca8; 1..5: 0xcaa, 0xcb2, 0xcbe, 0xcc4, 0xcc6 with the item's
/// line), then the help line's +0x140
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    let k = m.r32(to);
    if k == 0 {
        scene_vcall(m, e, 0x10, &[u32::MAX]);
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, 0xca8]);
    } else if k < 6 {
        const TEXTS: [u32; 5] = [0xcaa, 0xcb2, 0xcbe, 0xcc4, 0xcc6];
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, TEXTS[k as usize - 1]]);
        let f = fe(m);
        e.call(m, ITEM_LINES[k as usize - 1], &[f + 0x70]);
    }
    let h = m.r32(s + 0x4c);
    vcall(m, e, h, 0x140, &[]);
}

/// `cFE3DStateCharacterModify` vf26 0x279e18 (the item picked): fade out (+0x24) only for "done", then the base's
/// vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    m.w32(s + 0x24, (m.r32(item) == 0) as u32);
    base::vf26(m, e, s);
}

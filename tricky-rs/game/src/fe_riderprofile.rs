//! The front end's rider profile (board row G9n): `cFE3DStateRiderProfile`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the profile's panels are built
//! by the front end's own builders, called out), checked in the function runner (`tests/fe_riderprofile.rs`).
//!
//! The menu has five items, each with a panel (a widget group at +0x4f0 + 4k) shown while it is chosen; +0x4ec is the
//! picture whose sprite follows the item, +0x504 the rank list and +0x508 its marker; +0x54 is set once the rider's
//! textures have loaded. +0x2e is the player (a signed byte).

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use crate::fe_characcept::{CAREER, PLAYER_SLOT};
use crate::fe_charselect::{PRINT, RANK, SLOT_PROFILE};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{ADD_WIDGET, BACKDROP, BASE_CTOR, HELP_BUTTON, HELP_LAYOUT, TEXT_CTOR};
use crate::fe_widgets::group_init;
use crate::pf::Mem;

/// The video screen made for a rider's film (0x160 bytes)
pub const VIDEO_VTABLE: u32 = 0x3b5010;
/// The front end's calls: 0x27e918 the profile's panels built, then 0x27f600, 0x284f60, 0x282d30, 0x285230 and
/// 0x284c70 the five panels filled; 0x1f9d28 whether the sprite set's textures are in (gApp+0x728), 0x1f9c18 a rider's
/// profile textures loaded (sprite set, picture, 0); 0x2abb58 a video screen's film name
pub const BUILD_PANELS: u32 = 0x27e918;
pub const FILL_PANELS: [u32; 5] = [0x27f600, 0x284f60, 0x282d30, 0x285230, 0x284c70];
pub const TEXTURES_IN: u32 = 0x1f9d28;
pub const LOAD_TEXTURES: u32 = 0x1f9c18;
pub const SET_FILM: u32 = 0x2abb58;
/// The items' text ids, the riders' profile pictures by character, the film name's format ("data/video/rr_%s")
pub const ITEM_TEXTS: u32 = 0x3afac0;
pub const PROFILE_SPRITES: [u32; 12] = [0x11, 0x14, 0x13, 0x15, 0x16, 0x17, 0x12, 0x10, 0x18, 0x19, 0x1a, 0x1b];
pub const FILM_FORMAT: u32 = 0x3afaf8;

const ONE: u32 = 0x3f80_0000;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

fn scene_vcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) {
    let scene = m.r32(fe(m) + 0xa4);
    let vt = m.r32(scene);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let fun = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![scene.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, fun, &a);
}

/// `cFE3DStateRiderProfile` vf11 0x28b968, the screen built (`buf` 0x10 bytes of scratch): the base widgets with the
/// title (0x274208: 105.0, text 0xda6, 12); the help bar's layout 12 (0x2718f0); no back; the panels' builder; five
/// items (the texts at 0x3afac0, +0x118 the word at 0x3ae520, numbered), each with its panel (a group with no parent,
/// +0x70 4, added to the screen); the five fillers; each panel laid out (+0x140) and hidden, the first shown; the
/// title +0x70 4, the help bar's and backdrop's 8; the help line sized 420 × 40; the backdrop
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xda6, 0xc]);
    e.call(m, HELP_LAYOUT, &[s, 0xc]);
    m.w32(s + 0x18, 0);
    e.call(m, BUILD_PANELS, &[s]);
    let menu = m.r32(s + 0x3c);
    let word = m.r32(0x3ae520);
    for k in 0..5u32 {
        let at = e.call(m, ALLOC, &[0x3af520, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w + 0x118, word);
        let id = m.r32(ITEM_TEXTS + 4 * k);
        vcall(m, e, w, 0x150, &[id]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
        let g = e.call(m, ALLOC, &[0x3afad8, 0x140, 0]);
        if g != 0 {
            e.call(m, FILL, &[g, 0xdead_c0de, 0x140]);
        }
        group_init(m, e, g, s, 0);
        m.w32(s + 0x4f0 + 4 * k, g);
        m.w32(g.wrapping_add(0x70), 4);
        e.call(m, ADD_WIDGET, &[s, g]);
    }
    for f in FILL_PANELS {
        e.call(m, f, &[s]);
    }
    for k in 0..5u32 {
        let g = m.r32(s + 0x4f0 + 4 * k);
        vcall(m, e, g, 0x140, &[]);
        let g = m.r32(s + 0x4f0 + 4 * k);
        vcall(m, e, g, 0x28, &[0]);
    }
    let g = m.r32(s + 0x4f0);
    vcall(m, e, g, 0x28, &[1]);
    let t = m.r32(s + 0x38);
    m.w32(t + 0x70, 4);
    let h = m.r32(s + 0x40);
    m.w32(h + 0x70, 8);
    let b = m.r32(s + 0x44);
    m.w32(b + 0x70, 8);
    let line = m.r32(s + 0x4c);
    put4(m, buf, [420.0f32.to_bits(), 40.0f32.to_bits(), 0, 0]);
    vcall(m, e, line, 0x138, &[buf]);
    e.call(m, BACKDROP, &[s]);
}

/// `cFE3DStateRiderProfile::Update` 0x28bdd0 (`buf` for the base's): until the textures are in (0x1f9d28), +0x54 waits;
/// then the base's `Update`
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    if m.r32(s + 0x54) == 0 {
        let set = m.r32(m.r32(GAPP).wrapping_add(0x728));
        if e.call(m, TEXTURES_IN, &[set]) != 0 {
            m.w32(s + 0x54, 1);
        }
    }
    base::update(m, e, s, buf);
}

/// `cFE3DStateRiderProfile::CreateNextState` 0x28be20 (`buf` 0x100 bytes for the film's name): the help bar's fourth
/// button asked (0x271848); a video screen (0x160 bytes; +0x158 1, +0x154 and +0x15c 0) playing the rider's film
/// ("data/video/rr_" and the name at the slot's character's record +8)
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    e.call(m, HELP_BUTTON, &[s, 4]);
    let at = e.call(m, ALLOC, &[0x3afae8, 0x160, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, 0x160]);
    }
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0x158), 1);
    m.w32(at.wrapping_add(0xc), VIDEO_VTABLE);
    m.w32(at.wrapping_add(0x154), 0);
    m.w32(at.wrapping_add(0x15c), 0);
    let p = byte(m, s + 0x2e) as u32;
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
    let sb = byte(m, slot) as u32;
    let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
    let name = m.r32(c.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(8));
    e.call(m, PRINT, &[buf, FILM_FORMAT, name]);
    e.call(m, SET_FILM, &[at, buf]);
    at
}

/// `cFE3DStateRiderProfile::Activate` 0x28bf08 (the screen, the one before; `buf` 0x40 bytes of scratch): the 3D
/// scene's +0xa8; the player's slot; until the textures are in, the slot's character's profile textures loaded
/// (pictures by character from 0x11.. for characters 0..11); the rank list set on the slot's rank (+0x160) and its
/// list's +0xf0(1); the marker placed level with the chosen line (the list's line +0x84 plus half the difference of the
/// line's height (getter) and the list's +0x174, less 2); the base's `Activate`
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    scene_vcall(m, e, 0xa8, &[]);
    let p = byte(m, s + 0x2e) as u32;
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
    let sb = byte(m, slot) as u32;
    if m.r32(s + 0x54) == 0 {
        let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
        // STANDIN: characters past 11 pass a register the caller left; here 0x11
        let pic = PROFILE_SPRITES.get(c as usize).copied().unwrap_or(0x11);
        let set = m.r32(m.r32(GAPP).wrapping_add(0x728));
        e.call(m, LOAD_TEXTURES, &[set, pic, 0]);
    }
    let rank = e.call(m, RANK, &[CAREER, sb]);
    let w = m.r32(s + 0x504);
    vcall(m, e, w, 0x160, &[rank]);
    let w = m.r32(s + 0x504);
    let list = m.r32(w + 0x140);
    vcall(m, e, list, 0xf0, &[1]);
    let w = m.r32(s + 0x504);
    let list = m.r32(w + 0x140);
    let line_y = m.r32(list.wrapping_add(0x84));
    let h0 = m.r32(w.wrapping_add(0x174));
    let list = m.r32(w + 0x140);
    vcall(m, e, list, 0x130, &[buf + 0x20]);
    let d = (f32::from_bits(m.r32(buf + 0x24)) - f32::from_bits(h0)) * 0.5;
    let y = f32::from_bits(line_y) + d - 2.0;
    put4(m, buf + 0x30, [0, y.to_bits(), 0, ONE]);
    let mk = m.r32(s + 0x508);
    e.call(m, WIDGET_PLACE, &[mk, buf + 0x30]);
    base::activate(m, e, s);
}

/// `cFE3DStateRiderProfile` vf26 0x28c128: picking the third item ("film") only with the help bar's fourth button
/// enabled (its +0xa8), then the base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r32(m.r32(m.r32(s + 0x3c) + 0x140)) == 2 {
        let b = e.call(m, HELP_BUTTON, &[s, 4]);
        if m.r32(b.wrapping_add(0xa8)) != 0 {
            base::vf26(m, e, s);
        }
    }
}

/// `cFE3DStateRiderProfile` vf21 0x28c198: backing out once the textures are in (+0x54 is 1)
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r32(s + 0x54) == 1 {
        base::vf21(m, e, s);
    }
}

/// `cFE3DStateRiderProfile` vf22 0x28c1b8 (the item that goes, the one that comes): the base's vf22; every panel hidden
/// and closed (+0x28(0), +0x20(0)); the help bar's layout (12 for the third item, its fourth button enabled while the
/// rank list is on its first line; else 8); for items 0..4 the help text 0xcbf + item, the picture's colour (white;
/// item 2 half-dark) and sprite (0x7c, 0x7d, 0x7d, 0x7e, 0x7f), the item's panel shown and opened; the help line's
/// +0x140
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32, buf: u32) {
    base::vf22(m, e, from, to);
    for k in 0..5u32 {
        let g = m.r32(s + 0x4f0 + 4 * k);
        vcall(m, e, g, 0x28, &[0]);
        let g = m.r32(s + 0x4f0 + 4 * k);
        vcall(m, e, g, 0x20, &[0]);
    }
    if m.r32(to) == 2 {
        e.call(m, HELP_LAYOUT, &[s, 0xc]);
        let w = m.r32(s + 0x504);
        let on = m.r32(m.r32(w + 0x140)) == 0;
        let b = e.call(m, HELP_BUTTON, &[s, 4]);
        vcall(m, e, b, 0x100, &[on as u32]);
    } else {
        e.call(m, HELP_LAYOUT, &[s, 8]);
    }
    let k = m.r32(to);
    if k < 5 {
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, 0xcbf + k]);
        let pic = m.r32(s + 0x4ec);
        let half = 0.5f32.to_bits();
        put4(m, buf, if k == 2 { [ONE, half, half, half] } else { [ONE; 4] });
        vcall(m, e, pic, 0x50, &[buf]);
        let g = m.r32(s + 0x4f0 + 4 * k);
        let pic = m.r32(s + 0x4ec);
        m.w32(pic + 0x110, [0x7c, 0x7d, 0x7d, 0x7e, 0x7f][k as usize]);
        vcall(m, e, g, 0x28, &[1]);
        let g = m.r32(s + 0x4f0 + 4 * k);
        vcall(m, e, g, 0x20, &[1]);
    }
    let h = m.r32(s + 0x4c);
    vcall(m, e, h, 0x140, &[]);
}

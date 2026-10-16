//! The front end's character select (board row G9i): `cFE3DStateCharacterSelect`'s methods on the game's bytes, every
//! call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the widgets the builder
//! makes in place as [`crate::fe_widgets`]), checked in the function runner (`tests/fe_charselect.rs`).
//!
//! The menu (+0x3c) lists the twelve riders (the front end's character order, 0x265788); +0x2e is the player (a signed
//! byte). The screen keeps: +0x84 the twelve riders' records (0x38 bytes each from 0x331ea8), +0xb4 the four stats'
//! text ids, +0xc4 the stats panel's blink timer, +0xc8 the four stats' bars and +0xd8 their numbers, +0xe8 the
//! panel's heading entry, +0xec its rider name, +0xf0 the ten rank icons and +0x118 the rank marker, +0x11c the
//! profile line, +0x120 the stats panel, +0x124 the twelve menu items.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, PAD_PAUSE, WIDGET_PLACE};
use crate::fe_characcept::{CAREER, PLAYER_SLOT};
use crate::fe_charmodify::{RESTORE_CHOICE, RIDER_SHOW, SET_SELECTED};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{BACKDROP, BASE_CTOR, BOOTED, TEXT_CTOR};
use crate::fe_widgets::{
    group_init, icon_init, progress_init, text_entry_init, vcall_class, BUFFERED_TEXT_VTABLE, GROUP_ADD, ICON_VTABLE,
    PROGRESS_VTABLE,
};
use crate::pf::Mem;

pub const VTABLE: u32 = 0x3b0490;
pub const ACCEPT_VTABLE: u32 = 0x3b02d0;
pub const USER_NAME_VTABLE: u32 = 0x3b2d30;
/// The front end's character order (fe, place) and its place of a character (fe, character); the riders' records
pub const CHAR_AT: u32 = 0x265788;
pub const FIND_CHAR: u32 = 0x2658e8;
pub const RIDERS: u32 = 0x331ea8;
/// The characters' table (0xf0 bytes each; +8 the kind) and the menu order of the locked texts (twelve words)
pub const CHARACTERS: u32 = 0x332148;
pub const LOCK_ORDER: u32 = 0x3352c0;
pub const LOCK_TEXTS: u32 = 0x3af5d8;
/// The career's calls: 0x163b10 a character unlocked, 0x164330 a slot's profile, 0x1635b8 the rider seen (career,
/// player, -1, character), 0x1646b0 the four stats (career, slot, outfit, out), 0x164840 the rank, 0x164730 the rank's
/// name
pub const CHAR_UNLOCKED: u32 = 0x163b10;
pub const SLOT_PROFILE: u32 = 0x164330;
pub const SEEN: u32 = 0x1635b8;
pub const STATS: u32 = 0x1646b0;
pub const RANK: u32 = 0x164840;
pub const RANK_NAME: u32 = 0x164730;
pub const SET_RIDER_SLOT: u32 = crate::fe_charmodify::SET_RIDER_SLOT;
/// The screen's own calls: 0x276d78 the locks, 0x278840 the stats panel, 0x2759c8 a sprite's rectangle (screen,
/// sprite), 0x2b3950 a bar's value (bar, f12), 0x2fca28 a number printed (out, format, value), 0x2b0fb8 a text's
/// string, 0x271888 a player added, the overlay's shots (0x26b760 the camera, 0x26b8a8 the rider, 0x26b4e8 a pan),
/// the music (0x2621a0 the select's start, 0x262c20 vf18's cue, 0x261ee0 a rider's line)
pub const REFRESH_LOCKS: u32 = 0x276d78;
pub const FILL_STATS: u32 = 0x278840;
pub const SPRITE_RECT: u32 = 0x2759c8;
pub const BAR_VALUE: u32 = 0x2b3950;
pub const PRINT: u32 = 0x2fca28;
pub const SET_STRING: u32 = 0x2b0fb8;
pub const ADD_PLAYER: u32 = 0x271888;
pub const CAMERA_SHOT: u32 = 0x26b760;
pub const RIDER_SHOT: u32 = 0x26b8a8;
pub const PAN_TO_RIDER: u32 = 0x26b4e8;
pub const MUSIC_START: u32 = 0x2621a0;
pub const MUSIC_VF18: u32 = 0x262c20;
pub const SAY_LINE: u32 = 0x261ee0;

const ONE: u32 = 0x3f80_0000;
const HIGHLIGHT: [u32; 4] = [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687];

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
}

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
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

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// `cFE3DStateCharacterSelect::cFE3DStateCharacterSelect` 0x276be8: the base constructor, the vtable, the stats' bars
/// and numbers cleared, the four stats' text ids (0x25, 0x27, 0x28, 0x29), no stats panel
pub fn construct(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    e.call(m, BASE_CTOR, &[s]);
    m.w32(s + 0xc, VTABLE);
    for k in 0..4 {
        m.w32(s + 0xc8 + 4 * k, 0);
        m.w32(s + 0xd8 + 4 * k, 0);
    }
    put4(m, s + 0xb4, [0x25, 0x27, 0x28, 0x29]);
    m.w32(s + 0x120, 0);
    s
}

/// `cFE3DStateCharacterSelect::CreateNextState` 0x276c70: for the World Circuit (game type 2) the user name screen
/// (0x78 bytes; +0x68 1, +0x6c 1, +0x70 0), else the character accept (0x54 bytes); the three bytes handed on (+0x2d,
/// +0x2c, +0x2e)
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let at;
    if m.r32(fe(m) + 0xf0) == 2 {
        at = alloc(m, e, 0x3af500, 0x78);
        e.call(m, BASE_CTOR, &[at]);
        m.w32(at.wrapping_add(0x68), 1);
        m.w32(at.wrapping_add(0x6c), 0);
        m.w32(at.wrapping_add(0xc), USER_NAME_VTABLE);
        m.w32(at.wrapping_add(0x6c), 1);
        m.w32(at.wrapping_add(0x70), 0);
    } else {
        at = alloc(m, e, 0x3af510, 0x54);
        e.call(m, BASE_CTOR, &[at]);
        m.w32(at.wrapping_add(0xc), ACCEPT_VTABLE);
    }
    for o in [0x2d, 0x2c, 0x2e] {
        let b = m.r8(s + o);
        m.w8(at.wrapping_add(o), b);
    }
    at
}

/// 0x276d78 (`FE3DCharSel_RefreshLocks`): each of the twelve items' +0x100 with whether its character is unlocked
/// (0x163b10)
pub fn refresh_locks(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    for k in 0..12u32 {
        let f = fe(m);
        let c = e.call(m, CHAR_AT, &[f, k]);
        let w = m.r32(s + 0x124 + 4 * k);
        let vt = m.r32(w.wrapping_add(0xc));
        let open = e.call(m, CHAR_UNLOCKED, &[CAREER, c]);
        let adj = m.r16(vt.wrapping_add(0x100)) as i16 as u32;
        let fun = m.r32(vt.wrapping_add(0x104));
        e.call(m, fun, &[w.wrapping_add(adj), open]);
    }
}

/// `cFE3DStateCharacterSelect` vf11 0x276e28, the screen built (`buf` 0x40 bytes of scratch for its vectors):
/// - the base widgets with the title (0x274208: 105.0; 0xda7 for player one, 0xda8 for the second; 0x200a) and the
///   menu's layout bytes as the mode select's, +0x158 1;
/// - the twelve riders' items (no parent; +0x118 the word at 0x3ae520; the rider's name as text; numbered; the rider's
///   record kept at +0x84), then the locks;
/// - the stats panel (a group under the help bar, +0x120);
/// - four stats, each a text entry at (170, 45 + 15k) holding the stat's name (0.7 high, +0x70 3) and a progress bar
///   (100 × 30, sprites 0x1f / 0x1e / 0x1d, placed at (250, 10)), and a buffered number at (450, centred on the row);
/// - the rider name (0.8 high, under the heading, hidden), the heading entry (text 0xdd, at (250 − the sprite's width
///   − 6, 75), hidden), the ten rank icons (sprite 0x1c, 10 × 10 sized to the sprite, along the heading, hidden) and
///   the rank marker (sprite -5 then 3 × (height + 9), hidden);
/// - the profile line (on the help bar at the word at 0x3ae538, 18), the backdrop.
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let title = if m.r8(s + 0x2e) != 0 { 0xda8 } else { 0xda7 };
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, title, 0x200a]);
    let item_word = m.r32(0x3ae520);
    for (o, b) in [(0x111u32, 0u8), (0x110, 3), (0x116, 3), (0x112, 1), (0x113, 2), (0x114, 1), (0x115, 1)] {
        let menu = m.r32(s + 0x3c);
        m.w8(menu + o, b);
    }
    let menu = m.r32(s + 0x3c);
    m.w32(menu + 0x158, 1);
    for k in 0..12u32 {
        let f = fe(m);
        let c = e.call(m, CHAR_AT, &[f, k]);
        let at = alloc(m, e, 0x3af520, 0x130);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, 0, font]);
        let rec = c.wrapping_mul(0x38).wrapping_add(RIDERS);
        m.w32(s + 0x124 + 4 * k, w);
        m.w32(w + 0x118, item_word);
        let name = m.r32(rec.wrapping_add(4));
        vcall(m, e, w, 0x158, &[name]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
        m.w32(s + 0x84 + 4 * k, rec);
    }
    e.call(m, REFRESH_LOCKS, &[s]);

    // the stats panel
    let g = alloc(m, e, 0x3af530, 0x140);
    let bar = m.r32(s + 0x44);
    group_init(m, e, g, s, bar);
    m.w32(s + 0x120, g);
    let bar = m.r32(s + 0x44);
    e.call(m, GROUP_ADD, &[bar, g]);

    let (v0, v1) = (buf, buf + 0x10);
    for k in 0..4u32 {
        let y = k as i32 as f32 * 15.0 + 45.0;
        let grp = m.r32(s + 0x120);
        let en = alloc(m, e, 0x3af548, 0x150);
        let font1 = m.r32(m.r32(GAPP).wrapping_add(0x60));
        text_entry_init(m, e, en, s, grp);
        let t = alloc(m, e, 0x3af4c0, 0x130);
        let txt = e.call(m, TEXT_CTOR, &[t, s, grp, font1]);
        let old = m.r32(en + 0x110);
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(en + 0x110, txt);
        let pb = alloc(m, e, 0x3af558, 0x190);
        progress_init(m, e, pb, s, en);
        put4(m, v1, [100.0f32.to_bits(), 30.0f32.to_bits(), 0, 0]);
        vcall_class(m, e, PROGRESS_VTABLE, pb, 0x138, &[v1]);
        let old = m.r32(en + 0x114);
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(en + 0x114, pb);
        let txt = m.r32(en + 0x110);
        m.w32(txt + 0x118, 0.7f32.to_bits());
        put4(m, v0, [170.0f32.to_bits(), y.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[en, v0]);
        m.w32(en + 0x11c, 1);
        m.w32(en + 0x130, 2);
        let txt = m.r32(en + 0x110);
        let id = m.r32(s + 0xb4 + 4 * k);
        vcall(m, e, txt, 0x150, &[id]);
        let txt = m.r32(en + 0x110);
        put4(m, v0, [ONE; 4]);
        vcall(m, e, txt, 0x50, &[v0]);
        m.w32(en + 0x70, 3);
        let txt = m.r32(en + 0x110);
        m.w32(en + 0x118, 8.0f32.to_bits());
        m.w32(pb + 0x16c, 0x1f);
        m.w32(txt + 0x114, 1);
        m.w32(pb + 0x170, 0x1e);
        m.w32(pb + 0x174, 0x1d);
        m.w32(pb + 0x180, 100.0f32.to_bits());
        m.w32(pb + 0x184, ONE);
        put4(m, v0, [250.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall(m, e, pb, 0x138, &[v0]);
        m.w32(pb + 0x150, 100.0f32.to_bits());
        e.call(m, BAR_VALUE, &[pb, 0]);
        put4(m, pb + 0x140, [ONE, ONE, 0, 0]);
        vcall(m, e, en, 0x140, &[]);
        // the stat's number (a buffered single-line text: the constructor's object is the one allocated)
        let num = alloc(m, e, 0x3af520, 0x330);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
        e.call(m, TEXT_CTOR, &[num, s, grp, font]);
        m.w16(num.wrapping_add(0x130), 0);
        m.w32(num.wrapping_add(0xc), BUFFERED_TEXT_VTABLE);
        vcall_class(m, e, BUFFERED_TEXT_VTABLE, num, 0x158, &[0x3af560]);
        m.w32(num.wrapping_add(0x118), 0.7f32.to_bits());
        put4(m, v0, [ONE; 4]);
        vcall(m, e, num, 0x50, &[v0]);
        m.w32(num.wrapping_add(0x70), 3);
        let fo = m.r32(m.r32(GAPP).wrapping_add(0x60));
        let h = m.r32(fo.wrapping_add(0xc)) as i32 as f32;
        m.w32(num.wrapping_add(0x114), 1);
        let ny = y - h * fl(m.r32(num.wrapping_add(0x118))) * 0.5;
        put4(m, v0, [450.0f32.to_bits(), ny.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[num, v0]);
        m.w32(s + 0xd8 + 4 * k, num);
        m.w32(s + 0xc8 + 4 * k, pb);
        e.call(m, GROUP_ADD, &[grp, en]);
        e.call(m, GROUP_ADD, &[grp, num]);
    }

    // the rider name
    let grp = m.r32(s + 0x120);
    let r = e.call(m, SPRITE_RECT, &[s, 0x1c]);
    let sw = fl(m.r32(r.wrapping_add(4)));
    let r = e.call(m, SPRITE_RECT, &[s, 0x1c]);
    let sh = fl(m.r32(r.wrapping_add(8)));
    let t = alloc(m, e, 0x3af568, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let name = e.call(m, TEXT_CTOR, &[t, s, grp, font]);
    m.w32(name + 0x114, 1);
    vcall(m, e, name, 0x158, &[0x3af578]);
    m.w32(name + 0x118, 0.8f32.to_bits());
    put4(m, v0, [ONE; 4]);
    vcall(m, e, name, 0x50, &[v0]);
    vcall(m, e, name, 0x130, &[v0]);
    let ny = 75.0 - sh - fl(m.r32(v0 + 4)) - 5.0;
    put4(m, v1, [250.0f32.to_bits(), ny.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[name, v1]);
    vcall(m, e, name, 0x28, &[0]);
    m.w32(name + 0x70, 3);
    e.call(m, GROUP_ADD, &[grp, name]);
    m.w32(s + 0xec, name);

    // the heading entry
    let en = alloc(m, e, 0x3af588, 0x150);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    text_entry_init(m, e, en, s, grp);
    let t = alloc(m, e, 0x3af4c0, 0x130);
    let txt = e.call(m, TEXT_CTOR, &[t, s, grp, font]);
    let old = m.r32(en + 0x110);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(s + 0xe8, en);
    m.w32(en + 0x110, txt);
    vcall(m, e, txt, 0x150, &[0xdd]);
    let en = m.r32(s + 0xe8);
    put4(m, v0, [ONE; 4]);
    vcall(m, e, en, 0x50, &[v0]);
    let en = m.r32(s + 0xe8);
    let txt = m.r32(en + 0x110);
    m.w32(txt + 0x118, 0.8f32.to_bits());
    m.w32(txt + 0x114, 1);
    vcall(m, e, en, 0x28, &[0]);
    let en = m.r32(s + 0xe8);
    m.w32(en + 0x11c, 1);
    put4(m, v0, [(250.0 - sw - 6.0).to_bits(), 75.0f32.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[en, v0]);
    let en = m.r32(s + 0xe8);
    m.w32(en + 0x70, 3);
    vcall(m, e, en, 0x140, &[]);
    let en = m.r32(s + 0xe8);
    e.call(m, GROUP_ADD, &[grp, en]);

    // the ten rank icons
    for k in 0..10u32 {
        let ic = alloc(m, e, 0x3af598, 0x120);
        icon_init(m, e, ic, s, grp);
        put4(m, v1, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[v1]);
        put4(m, v0, [ONE; 4]);
        vcall(m, e, ic, 0x50, &[v0]);
        let x = k as i32 as f32 * (sw + 6.0) + 250.0;
        put4(m, v0, [x.to_bits(), 75.0f32.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[ic, v0]);
        put4(m, v0, [sw.to_bits(), sh.to_bits(), 0, 0]);
        vcall(m, e, ic, 0x138, &[v0]);
        m.w32(ic + 0x114, 0x60);
        m.w32(ic + 0x110, 0x1c);
        vcall(m, e, ic, 0x28, &[0]);
        m.w32(ic + 0x70, 4);
        e.call(m, GROUP_ADD, &[grp, ic]);
        m.w32(s + 0xf0 + 4 * k, ic);
    }

    // the rank marker
    let ic = alloc(m, e, 0x3af5a8, 0x120);
    icon_init(m, e, ic, s, grp);
    put4(m, v1, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[v1]);
    m.w32(ic + 0x110, (-5i32) as u32);
    m.w32(s + 0x118, ic);
    put4(m, v0, [250.0f32.to_bits(), 73.0f32.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[ic, v0]);
    let r = e.call(m, SPRITE_RECT, &[s, 0x1b]);
    let h = fl(m.r32(r.wrapping_add(8))) + 9.0;
    let ic = m.r32(s + 0x118);
    put4(m, v0, [3.0f32.to_bits(), h.to_bits(), 0, 0]);
    vcall(m, e, ic, 0x138, &[v0]);
    let ic = m.r32(s + 0x118);
    put4(m, v0, [ONE, 0, 0, 0]);
    vcall(m, e, ic, 0x50, &[v0]);
    let ic = m.r32(s + 0x118);
    m.w32(ic + 0x114, 0x60);
    vcall(m, e, ic, 0x28, &[0]);
    let ic = m.r32(s + 0x118);
    m.w32(ic + 0x70, 3);
    e.call(m, GROUP_ADD, &[grp, ic]);

    // the profile line
    let t = alloc(m, e, 0x3af5b8, 0x130);
    let bar = m.r32(s + 0x44);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let pl = e.call(m, TEXT_CTOR, &[t, s, bar, font]);
    m.w32(s + 0x11c, pl);
    let hgt = m.r32(0x3ae540);
    m.w32(pl + 0x118, hgt);
    m.w32(pl + 0x70, 8);
    put4(m, v0, [ONE; 4]);
    vcall(m, e, pl, 0x50, &[v0]);
    let pl = m.r32(s + 0x11c);
    m.w32(pl + 0x114, 1);
    let x = m.r32(0x3ae538);
    put4(m, v0, [x, 18.0f32.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[pl, v0]);
    let pl = m.r32(s + 0x11c);
    vcall(m, e, pl, 0x158, &[0x3af5c8]);
    let bar = m.r32(s + 0x44);
    let pl = m.r32(s + 0x11c);
    e.call(m, GROUP_ADD, &[bar, pl]);
    e.call(m, BACKDROP, &[s]);
}

/// `cFE3DStateCharacterSelect::Activate` 0x278598 (the screen, the one before):
/// - the locks; no pause overlay;
/// - the player's rider: when booted (0x265af8) from the player's career slot's profile (0x164330, 0x2658e8), else
///   the player's pick (+0x7d + 2 × player); the menu set on it (+0x160) and the selected rider set (0x261b38);
/// - the stats panel for the chosen item; the overlay's shot of it (booted: the rider shot, else the camera shot);
/// - after a screen answering 6 from its +0x70 whose player is lower, every rider's choice restored for this player;
/// - each of the twelve riders shown (0x2661e8): 1 for the pick when booted, else 0;
/// - the 3D scene's +0x58(1); not booted, the select's music start (0x2621a0 with the screen before); the base's
///   `Activate`; booted, the screen's +0x40.
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    e.call(m, REFRESH_LOCKS, &[s]);
    m.w32(s + 0x28, 0);
    let f = fe(m);
    let p = byte(m, s + 0x2e);
    let pick = if e.call(m, BOOTED, &[f]) != 0 {
        let slot = e.call(m, PLAYER_SLOT, &[CAREER, p as u32]);
        let sb = byte(m, slot) as u32;
        let prof = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
        e.call(m, FIND_CHAR, &[f, prof as u8 as i8 as i32 as u32])
    } else {
        byte(m, f.wrapping_add((2 * p) as u32).wrapping_add(0x7d)) as u32
    };
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x160, &[pick]);
    let n = byte(m, f + 4) as u32;
    e.call(m, SET_SELECTED, &[f, p as u32, n, pick]);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, FILL_STATS, &[s, chosen]);
    let ov = m.r32(f + 0x50);
    let it = m.r32(m.r32(s + 0x3c) + 0x140);
    let f2 = fe(m);
    let booted = e.call(m, BOOTED, &[f2]);
    let b = byte(m, it) as u32;
    e.call(m, if booted != 0 { RIDER_SHOT } else { CAMERA_SHOT }, &[ov, b]);
    if vcall(m, e, prev, 0x70, &[]) == 6 && byte(m, prev + 0x2e) < byte(m, s + 0x2e) {
        for k in 0..12u32 {
            let r = m.r32(f + 0x20 + 4 * k);
            let q = byte(m, s + 0x2e) as u32;
            e.call(m, RESTORE_CHOICE, &[r, q]);
        }
    }
    for k in 0..12u32 {
        let f2 = fe(m);
        let booted = e.call(m, BOOTED, &[f2]);
        let r = m.r32(f + 0x20 + 4 * k);
        let on = booted != 0 && k == pick;
        e.call(m, RIDER_SHOW, &[r, on as u32]);
    }
    let scene = m.r32(f + 0xa4);
    let vt = m.r32(scene);
    let adj = m.r16(vt.wrapping_add(0x58)) as i16 as u32;
    let fun = m.r32(vt.wrapping_add(0x5c));
    e.call(m, fun, &[scene.wrapping_add(adj), 1]);
    let f2 = fe(m);
    if e.call(m, BOOTED, &[f2]) == 0 {
        e.call(m, MUSIC_START, &[f + 0x70, prev]);
    }
    base::activate(m, e, s);
    let f2 = fe(m);
    if e.call(m, BOOTED, &[f2]) != 0 {
        vcall(m, e, s, 0x40, &[]);
    }
}

/// `cFE3DStateCharacterSelect::LayoutWidgets` 0x278810 (`buf` 0x30 bytes for the base's): the base's, then the stats
/// panel for the chosen item
pub fn layout_widgets(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    base::layout_widgets(m, e, s, buf);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, FILL_STATS, &[s, chosen]);
}

/// 0x278840 (`FE3D_FillProfileStats`; the screen, the item, `buf` 0x80 bytes of scratch: +0 the four stats, +0x40,
/// +0x50, +0x60, +0x70 vectors):
/// - an unlocked rider (the item's +0xa8): marked seen (0x1635b8); the four stats (0x1646b0) for the player's slot and
///   rider's outfit, each bar sized to its maximum × 5 by its own height, its range and value set and its number
///   printed (the format at 0x3af5d0, value + bonus); the rank icons (sprite 0x1b at the rank, else 0x1c), the marker
///   moved to the rank's icon, the rank's name centred over it and again over the icon 9 − rank, the icons up to
///   9 − rank shaded from the highlight towards (1, 1, 0, 0) by tenths (sprite 0x1b) and the rest white (0x1c), the
///   marker at the last icon shaded, the panel shown, the profile line's string 0x3af5c8;
/// - a locked rider: the panel hidden, and the help line the locked text for the rider's place in 0x3352c0 (the
///   eight words at 0x3af5d8 for places 4..11);
/// - then the help line's +0x140, and the profile line's text by the character's kind (1: 0x14, 2: 0x12, any other
///   0x13).
pub fn fill_stats(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    let f = fe(m);
    let ib = byte(m, item) as u32;
    let c = e.call(m, CHAR_AT, &[f, ib]);
    let (v40, v50, v60, v70) = (buf + 0x40, buf + 0x50, buf + 0x60, buf + 0x70);
    let line;
    if m.r32(item + 0xa8) != 0 {
        let p = byte(m, s + 0x2e) as u32;
        e.call(m, SEEN, &[CAREER, p, u32::MAX, c]);
        let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
        let sb = byte(m, slot) as u32;
        let pick = byte(m, f.wrapping_add(2 * p).wrapping_add(0x7d));
        let rider = m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20));
        let outfit = byte(m, rider.wrapping_add(0x158)) as u32;
        e.call(m, STATS, &[CAREER, sb, outfit, buf]);
        for j in 0..4u32 {
            let rec = buf + 0x10 * j;
            let pb = m.r32(s + 0xc8 + 4 * j);
            let max = m.r32(rec + 0xc) as i32 as f32;
            let h = m.r32(pb.wrapping_add(0x104));
            put4(m, v40, [(max * 5.0).to_bits(), h, 0, 0]);
            vcall(m, e, pb, 0x138, &[v40]);
            let pb = m.r32(s + 0xc8 + 4 * j);
            m.w32(pb.wrapping_add(0x150), (m.r32(rec + 0xc) as i32 as f32).to_bits());
            let val = m.r32(rec + 4) as i32 as f32;
            e.call(m, BAR_VALUE, &[pb, val.to_bits()]);
            let (a, b, mx) = (m.r32(rec + 4), m.r32(rec + 8), m.r32(rec + 0xc));
            let pb = m.r32(s + 0xc8 + 4 * j);
            m.w32(pb.wrapping_add(0x184), ONE);
            m.w32(pb.wrapping_add(0x180), (mx as i32 as f32).to_bits());
            m.w32(pb.wrapping_add(0x158), (a as i32 as f32 + b as i32 as f32).to_bits());
            e.call(m, PRINT, &[v40, 0x3af5d0, a.wrapping_add(b)]);
            let num = m.r32(s + 0xd8 + 4 * j);
            vcall(m, e, num, 0x158, &[v40]);
        }
        let rank = e.call(m, RANK, &[CAREER, sb]) as i32;
        for k in 0..10i32 {
            let ic = m.r32(s + 0xf0 + 4 * k as u32);
            m.w32(ic.wrapping_add(0x110), if k == rank { 0x1b } else { 0x1c });
        }
        let mk = m.r32(s + 0x118);
        let at = m.r32(s.wrapping_add(0xf0).wrapping_add((4 * rank) as u32));
        let mut x = fl(m.r32(at.wrapping_add(0x80)));
        let y = m.r32(mk.wrapping_add(0x84));
        put4(m, v40, [x.to_bits(), y, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[mk, v40]);
        let nm = m.r32(s + 0xec);
        let vt = m.r32(nm.wrapping_add(0xc));
        let n = e.call(m, RANK_NAME, &[CAREER, sb]);
        let adj = m.r16(vt.wrapping_add(0x150)) as i16 as u32;
        let fun = m.r32(vt.wrapping_add(0x154));
        e.call(m, fun, &[nm.wrapping_add(adj), n]);
        let nm = m.r32(s + 0xec);
        vcall(m, e, nm, 0x130, &[v40]);
        x -= fl(m.r32(v40)) * 0.5;
        let nm = m.r32(s + 0xec);
        let ny = m.r32(nm.wrapping_add(0x84));
        put4(m, v50, [x.to_bits(), ny, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[nm, v50]);
        let n2 = e.call(m, RANK_NAME, &[CAREER, sb]);
        let r2 = e.call(m, RANK, &[CAREER, sb]) as i32;
        let nm = m.r32(s + 0xec);
        let vt = m.r32(nm.wrapping_add(0xc));
        let last = 9 - r2;
        let lx = fl(m.r32(m.r32(s.wrapping_add(0xf0).wrapping_add((4 * last) as u32)).wrapping_add(0x80)));
        let adj = m.r16(vt.wrapping_add(0x150)) as i16 as u32;
        let fun = m.r32(vt.wrapping_add(0x154));
        e.call(m, fun, &[nm.wrapping_add(adj), n2]);
        let nm = m.r32(s + 0xec);
        vcall(m, e, nm, 0x130, &[v40]);
        let nm = m.r32(s + 0xec);
        let ny = m.r32(nm.wrapping_add(0x84));
        put4(m, v50, [(lx - fl(m.r32(v40)) * 0.5).to_bits(), ny, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[nm, v50]);
        put4(m, v50, HIGHLIGHT);
        put4(m, v60, [ONE, ONE, 0, 0]);
        let mut next = last + 1;
        if last >= 0 {
            let mut j = 0i32;
            loop {
                let t = j as f32 / 10.0;
                let mut col = [0u32; 4];
                for (q, c) in col.iter_mut().enumerate() {
                    let a = fl(m.r32(v50 + 4 * q as u32));
                    let b = fl(m.r32(v60 + 4 * q as u32));
                    *c = (a + (b - a) * t).to_bits();
                }
                put4(m, v70, col);
                let ic = m.r32(s.wrapping_add(0xf0).wrapping_add((4 * j) as u32));
                j += 1;
                vcall(m, e, ic, 0x50, &[v70]);
                let ic = m.r32(s.wrapping_add(0xf0).wrapping_add((4 * (j - 1)) as u32));
                m.w32(ic.wrapping_add(0x110), 0x1b);
                if last < j {
                    break;
                }
            }
            next = last + 1;
        }
        while next < 10 {
            let ic = m.r32(s.wrapping_add(0xf0).wrapping_add((4 * next) as u32));
            next += 1;
            m.w32(v70, ONE);
            put4(m, v70, [ONE; 4]);
            vcall(m, e, ic, 0x50, &[v70]);
            let ic = m.r32(s.wrapping_add(0xf0).wrapping_add((4 * (next - 1)) as u32));
            m.w32(ic.wrapping_add(0x110), 0x1c);
        }
        let mk = m.r32(s + 0x118);
        let my = m.r32(mk.wrapping_add(0x84));
        put4(m, v70, [lx.to_bits(), my, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[mk, v70]);
        let g = m.r32(s + 0x120);
        vcall(m, e, g, 0x28, &[1]);
        let h = m.r32(s + 0x4c);
        e.call(m, SET_STRING, &[h, 0x3af5c8]);
        line = m.r32(s + 0x4c);
    } else {
        let g = m.r32(s + 0x120);
        vcall(m, e, g, 0x28, &[0]);
        let mut k = 0u32;
        while k < 12 && m.r32(LOCK_ORDER + 4 * k) != c {
            k += 1;
        }
        // STANDIN: places 0..3 and 12 (not found) read the game's own stack below the eight texts; here 0. The
        // order's first four (riders 3, 4, 7, 0) are the starting riders, never locked
        let text = if (4..12).contains(&k) { m.r32(LOCK_TEXTS + 4 * (k - 4)) } else { 0 };
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, text]);
        line = m.r32(s + 0x4c);
    }
    vcall(m, e, line, 0x140, &[]);
    let kind = m.r32(c.wrapping_mul(0xf0).wrapping_add(CHARACTERS).wrapping_add(8)) as i32;
    let text = match kind {
        1 => 0x14,
        2 => 0x12,
        _ => 0x13,
    };
    let pl = m.r32(s + 0x11c);
    vcall(m, e, pl, 0x150, &[text]);
}

/// `cFE3DStateCharacterSelect::Update` 0x278f60 (`buf` for the base's): with the stats panel shown, its blink timer
/// (+0xc4) counts down; at zero the rider name, heading, rank icons and marker swap with the four stats' bars and
/// numbers (shown for 0x10e frames when the name was shown, else 0x78); then the base's `Update`
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let g = m.r32(s + 0x120);
    if m.r32(g + 0x14) != 0 {
        let t = m.r32(s + 0xc4);
        let next = if t != 0 {
            t
        } else {
            let nm = m.r32(s + 0xec);
            let shown = m.r32(nm + 0x14) != 0;
            let (a, b) = if shown { (0, 1) } else { (1, 0) };
            vcall(m, e, nm, 0x28, &[a]);
            let en = m.r32(s + 0xe8);
            vcall(m, e, en, 0x28, &[a]);
            for k in 0..10u32 {
                let ic = m.r32(s + 0xf0 + 4 * k);
                vcall(m, e, ic, 0x28, &[a]);
                let mk = m.r32(s + 0x118);
                vcall(m, e, mk, 0x28, &[a]);
            }
            for j in 0..4u32 {
                let pb = m.r32(s + 0xc8 + 4 * j);
                let row = m.r32(pb + 0xf0);
                vcall(m, e, row, 0x28, &[b]);
                let num = m.r32(s + 0xd8 + 4 * j);
                vcall(m, e, num, 0x28, &[b]);
            }
            let v = if shown { 0x10e } else { 0x78 };
            m.w32(s + 0xc4, v);
            v
        };
        m.w32(s + 0xc4, next.wrapping_sub(1));
    }
    base::update(m, e, s, buf);
}

/// `cFE3DStateCharacterSelect` vf18 0x2791c8: for the first player, the music's 0x262c20
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r8(s + 0x2e) == 0 {
        let f = fe(m);
        e.call(m, MUSIC_VF18, &[f + 0x70]);
    }
}

/// `cFE3DStateCharacterSelect` vf22 0x2791f0 (the item that goes, the one that comes): the base's vf22; the coming
/// item's +0x20(1) and the going one's +0x20(0); the blink timer restarted (0x78 with the name shown, else 0x10e);
/// with an item going, the selected rider set, the stats panel, the rider's line (0x261ee0 with the character) and
/// the overlay's pan to it (0x26b4e8)
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    vcall(m, e, to, 0x20, &[1]);
    if from != 0 {
        vcall(m, e, from, 0x20, &[0]);
    }
    let nm = m.r32(s + 0xec);
    let t = if m.r32(nm + 0x14) != 0 { 0x78 } else { 0x10e };
    m.w32(s + 0xc4, t);
    if from != 0 {
        let k = m.r32(to);
        let f = fe(m);
        let p = byte(m, s + 0x2e) as u32;
        let n = byte(m, f + 4) as u32;
        e.call(m, SET_SELECTED, &[f, p, n, k]);
        e.call(m, FILL_STATS, &[s, to]);
        let f2 = fe(m);
        let tb = byte(m, to) as u32;
        let c = e.call(m, CHAR_AT, &[f2, tb]);
        e.call(m, SAY_LINE, &[f + 0x70, c]);
        let ov = m.r32(f + 0x50);
        e.call(m, PAN_TO_RIDER, &[ov, k as u8 as i8 as i32 as u32]);
    }
}

/// `cFE3DStateCharacterSelect` vf26 0x279318 (the item picked): each player pressing (0x26ce08) becomes the screen's
/// chooser (+0x2c the player + 1, the career's rider slot set for this screen's player, +0x2d 0, the player added
/// (0x271888)); the 3D scene's +0x10(-1); the pause overlay on; this player's pick (+0x7c + 2 × player) the item; the
/// rider at the other byte's pick shown (0x2661e8 with 1); the overlay's rider shot; the screen's +0x30
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    for k in 0..2u32 {
        let f = fe(m);
        let p = m.r32(f + 0x74 + 4 * k);
        if e.call(m, PAD_PAUSE, &[p]) != 0 {
            m.w8(s + 0x2c, (k + 1) as u8);
            let me = byte(m, s + 0x2e) as u32;
            e.call(m, SET_RIDER_SLOT, &[CAREER, me, k]);
            m.w8(s + 0x2d, 0);
            e.call(m, ADD_PLAYER, &[s, k]);
        }
    }
    scene_vcall(m, e, 0x10, &[u32::MAX]);
    m.w32(s + 0x28, 1);
    let f = fe(m);
    let p = byte(m, s + 0x2e);
    let at = f.wrapping_add((2 * p) as u32);
    let b = m.r8(item);
    m.w8(at.wrapping_add(0x7c), b);
    let pick = byte(m, at.wrapping_add(0x7d));
    let r = m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20));
    e.call(m, RIDER_SHOW, &[r, 1]);
    let ov = m.r32(f + 0x50);
    let ib = byte(m, item) as u32;
    e.call(m, RIDER_SHOT, &[ov, ib]);
    vcall(m, e, s, 0x30, &[]);
}

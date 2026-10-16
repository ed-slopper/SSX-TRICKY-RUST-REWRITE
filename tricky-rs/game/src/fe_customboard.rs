//! The front end's custom board screen (board row G9l): `cFE3DStateCustomBoard`'s methods on the game's bytes, every
//! call through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the widgets the builder
//! makes in place as [`crate::fe_widgets`]), checked in the function runner (`tests/fe_customboard.rs`).
//!
//! The screen picks one of the player's rider's twelve boards. +0x2e is the player (a signed byte); the rider is the
//! front end's +0x20 list at (the player's pick, +0x7d + 2 × player) mod 12; +0x54 keeps the board the screen opened
//! on. Like the character select it has a stats panel (+0x8c) of four rows: +0x58 the stats' text ids, +0x68 their
//! bars, +0x78 their numbers; +0x88 is the profile line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use crate::fe_characcept::{CAREER, PLAYER_SLOT, SAVE_SELECTION};
use crate::fe_charselect::{BAR_VALUE, CHARACTERS, PRINT, SET_STRING, SLOT_PROFILE, STATS};
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use crate::fe_widgets::{
    group_init, progress_init, text_entry_init, vcall_class, BUFFERED_TEXT_VTABLE, GROUP_ADD, PROGRESS_VTABLE,
};
use crate::pf::Mem;

pub const VTABLE: u32 = 0x3aff50;
/// The front end's calls: 0x267c70 whether a board is unlocked (rider, player, board), 0x267c20 a board tried on
/// (rider, board), 0x26bb38 the overlay's board view (overlay, on), 0x2622c8 the custom screens' board music, 0x28d668
/// the stats panel; 0x2755e8 a layout of the help bar's widgets (the screen, f12 the float at 0x348024)
pub const BOARD_UNLOCKED: u32 = 0x267c70;
pub const TRY_BOARD: u32 = 0x267c20;
pub const BOARD_VIEW: u32 = 0x26bb38;
pub const MUSIC_LOOP: u32 = 0x2622c8;
pub const FILL_PROGRESS: u32 = 0x28d668;
pub const HELP_LAYOUT_SCALED: u32 = 0x2755e8;
pub const SCALE_WORD: u32 = 0x348024;
/// The locked boards' texts (ten words for boards 2..11)
pub const LOCK_TEXTS: u32 = 0x3afb78;

const ONE: u32 = 0x3f80_0000;

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

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// The player's career slot asked for (0x164318) and the player's rider
fn rider(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> (u32, u32) {
    let p = byte(m, s + 0x2e);
    let f = fe(m);
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p as u32]);
    let pick = byte(m, f.wrapping_add((2 * p) as u32).wrapping_add(0x7d));
    (slot, m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20)))
}

/// `cFE3DStateCustomBoard::cFE3DStateCustomBoard` 0x28c690: the base constructor, +0x54 0, no profile line or panel,
/// the bars and numbers cleared, the four stats' text ids (0x25, 0x27, 0x28, 0x29)
pub fn construct(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    e.call(m, BASE_CTOR, &[s]);
    m.w8(s + 0x54, 0);
    m.w32(s + 0x88, 0);
    m.w32(s + 0xc, VTABLE);
    m.w32(s + 0x8c, 0);
    for k in 0..4 {
        m.w32(s + 0x68 + 4 * k, 0);
        m.w32(s + 0x78 + 4 * k, 0);
    }
    put4(m, s + 0x58, [0x25, 0x27, 0x28, 0x29]);
    s
}

/// `cFE3DStateCustomBoard` vf11 0x28c718, the screen built (`buf` 0x30 bytes of scratch): the base widgets with the
/// title (0x274208: 105.0, text 0xb5, 0x200a); the twelve boards' items (numbered, +0x118 the word at 0x3ae520, the
/// board's name from the rider's character's table as text, whether unlocked (0x267c70) to +0x100); the stats panel
/// (a group under the help bar) with four rows as the character select's (the text's colour set before its id, +0x70
/// 4); the help bar laid out (0x2755e8) with the float at 0x348024; the profile line; the backdrop; no fade in; the
/// board the rider has kept (+0x54) and the menu set on it; the stats panel for the chosen item
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let p = byte(m, s + 0x2e) as u32;
    let (_, r) = rider(m, e, s);
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xb5, 0x200a]);
    let menu = m.r32(s + 0x3c);
    let word = m.r32(0x3ae520);
    for k in 0..12u32 {
        let at = alloc(m, e, 0x3af520, 0x130);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w, k);
        m.w32(w + 0x118, word);
        vcall(m, e, menu, 0x148, &[w]);
        let set = m.r32(r.wrapping_add(0xb0)).wrapping_mul(0xf0);
        let name = m.r32(set.wrapping_add(k * 0x14).wrapping_add(CHARACTERS));
        vcall(m, e, w, 0x158, &[name]);
        let vt = m.r32(w + 0xc);
        let open = e.call(m, BOARD_UNLOCKED, &[r, p, k]);
        let adj = m.r16(vt.wrapping_add(0x100)) as i16 as u32;
        let fun = m.r32(vt.wrapping_add(0x104));
        e.call(m, fun, &[w.wrapping_add(adj), open]);
    }

    let g = alloc(m, e, 0x3afb60, 0x140);
    let bar = m.r32(s + 0x44);
    group_init(m, e, g, s, bar);
    m.w32(s + 0x8c, g);
    let bar = m.r32(s + 0x44);
    e.call(m, GROUP_ADD, &[bar, g]);

    let (v0, v1) = (buf, buf + 0x10);
    for k in 0..4u32 {
        let grp = m.r32(s + 0x8c);
        let y = k as i32 as f32 * 15.0 + 45.0;
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
        let txt = m.r32(en + 0x110);
        m.w32(en + 0x114, pb);
        m.w32(txt + 0x118, 0.7f32.to_bits());
        put4(m, v0, [170.0f32.to_bits(), y.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[en, v0]);
        let txt = m.r32(en + 0x110);
        m.w32(en + 0x11c, 1);
        m.w32(en + 0x130, 2);
        put4(m, v0, [ONE; 4]);
        vcall(m, e, txt, 0x50, &[v0]);
        let id = m.r32(s + 0x58 + 4 * k);
        let txt = m.r32(en + 0x110);
        m.w32(txt + 0x114, 1);
        vcall(m, e, txt, 0x150, &[id]);
        m.w32(en + 0x70, 4);
        m.w32(en + 0x118, 8.0f32.to_bits());
        m.w32(pb + 0x16c, 0x1f);
        m.w32(pb + 0x174, 0x1d);
        m.w32(pb + 0x170, 0x1e);
        m.w32(pb + 0x180, 100.0f32.to_bits());
        m.w32(pb + 0x184, ONE);
        put4(m, v0, [250.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall(m, e, pb, 0x138, &[v0]);
        m.w32(pb + 0x150, 100.0f32.to_bits());
        e.call(m, BAR_VALUE, &[pb, 0]);
        put4(m, pb + 0x140, [ONE, ONE, 0, 0]);
        vcall(m, e, en, 0x140, &[]);
        let num = alloc(m, e, 0x3af520, 0x330);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
        e.call(m, TEXT_CTOR, &[num, s, grp, font]);
        m.w16(num.wrapping_add(0x130), 0);
        m.w32(num.wrapping_add(0xc), BUFFERED_TEXT_VTABLE);
        vcall_class(m, e, BUFFERED_TEXT_VTABLE, num, 0x158, &[0x3af560]);
        m.w32(num.wrapping_add(0x118), 0.7f32.to_bits());
        put4(m, v0, [ONE; 4]);
        vcall(m, e, num, 0x50, &[v0]);
        let hgt = fl(m.r32(num.wrapping_add(0x118)));
        m.w32(num.wrapping_add(0x70), 4);
        let fo = m.r32(m.r32(GAPP).wrapping_add(0x60));
        m.w32(num.wrapping_add(0x114), 1);
        let h = m.r32(fo.wrapping_add(0xc)) as i32 as f32;
        let ny = y - h * hgt * 0.5;
        put4(m, v0, [450.0f32.to_bits(), ny.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[num, v0]);
        m.w32(s + 0x68 + 4 * k, pb);
        m.w32(s + 0x78 + 4 * k, num);
        e.call(m, GROUP_ADD, &[grp, en]);
        e.call(m, GROUP_ADD, &[grp, num]);
    }
    let scale = m.r32(SCALE_WORD);
    e.call(m, HELP_LAYOUT_SCALED, &[s, scale]);

    let t = alloc(m, e, 0x3af5b8, 0x130);
    let bar = m.r32(s + 0x44);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let pl = e.call(m, TEXT_CTOR, &[t, s, bar, font]);
    m.w32(s + 0x88, pl);
    let hgt = m.r32(0x3ae540);
    m.w32(pl + 0x118, hgt);
    m.w32(pl + 0x70, 8);
    put4(m, v0, [ONE; 4]);
    vcall(m, e, pl, 0x50, &[v0]);
    let pl = m.r32(s + 0x88);
    m.w32(pl + 0x114, 1);
    let x = m.r32(0x3ae538);
    put4(m, v0, [x, 18.0f32.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[pl, v0]);
    let bar = m.r32(s + 0x44);
    let pl = m.r32(s + 0x88);
    e.call(m, GROUP_ADD, &[bar, pl]);
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
    let b = m.r8(r.wrapping_add(0x158));
    m.w8(s + 0x54, b);
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x160, &[b as i8 as i32 as u32]);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, FILL_PROGRESS, &[s, chosen]);
}

/// `cFE3DStateCustomBoard::Activate` 0x28d458: the overlay's board view off, the board music (0x2622c8), the base's
/// `Activate`
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let o = m.r32(fe(m) + 0x50);
    e.call(m, BOARD_VIEW, &[o, 0]);
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
    base::activate(m, e, s);
}

/// `cFE3DStateCustomBoard` vf18 0x28d4c0: the overlay's board view on
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv) {
    let o = m.r32(fe(m) + 0x50);
    e.call(m, BOARD_VIEW, &[o, 1]);
}

/// `cFE3DStateCustomBoard` vf21 0x28d4d8 (backing out): the rider put back on the board the screen opened on, then
/// the base's vf21
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let (_, r) = rider(m, e, s);
    let b = byte(m, s + 0x54) as u32;
    e.call(m, TRY_BOARD, &[r, b]);
    base::vf21(m, e, s);
}

/// `cFE3DStateCustomBoard` vf22 0x28d570 (the item that goes, the one that comes): the base's vf22; with an item
/// going, the stats panel for the coming one
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        e.call(m, FILL_PROGRESS, &[s, to]);
    }
}

/// `cFE3DStateCustomBoard` vf26 0x28d5d0 (accepting): the rider saves its selection with the first byte of the
/// player's career slot, then the base's vf21 (the screen's accept)
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let (slot, r) = rider(m, e, s);
    let b = byte(m, slot) as u32;
    e.call(m, SAVE_SELECTION, &[r, b]);
    base::vf21(m, e, s);
}

/// 0x28d668 (`FE3D_FillProgressText`; the screen, the item, `buf` 0x50 bytes of scratch: +0 the four stats, +0x40 a
/// vector and the printed number): the rider tries the item's board on (0x267c20); an unlocked board (+0xa8): its
/// four stats (0x1646b0) for the player's slot and the board, each bar and number as the character select's, the
/// panel shown and the help line's string 0x3af5c8; a locked one: the panel hidden and the help line the locked text
/// for boards 2..11 (0x3afb78); then the help line's +0x140 and the profile line's text by the board's kind in the
/// slot's character's table (1: 0x14, 2: 0x12, any other 0x13)
pub fn fill_progress(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    let p = byte(m, s + 0x2e) as u32;
    let board = m.r32(item);
    let slot = e.call(m, PLAYER_SLOT, &[CAREER, p]);
    let sb = byte(m, slot) as u32;
    let f = fe(m);
    let pick = byte(m, f.wrapping_add(2 * p).wrapping_add(0x7d));
    let r = m.r32(f.wrapping_add((4 * (pick % 12)) as u32).wrapping_add(0x20));
    e.call(m, TRY_BOARD, &[r, board as u8 as i8 as i32 as u32]);
    let v40 = buf + 0x40;
    let line;
    if m.r32(item + 0xa8) != 0 {
        e.call(m, STATS, &[CAREER, sb, board, buf]);
        for j in 0..4u32 {
            let rec = buf + 0x10 * j;
            let pb = m.r32(s + 0x68 + 4 * j);
            let max = m.r32(rec + 0xc) as i32 as f32;
            let h = m.r32(pb.wrapping_add(0x104));
            put4(m, v40, [(max * 5.0).to_bits(), h, 0, 0]);
            vcall(m, e, pb, 0x138, &[v40]);
            let pb = m.r32(s + 0x68 + 4 * j);
            m.w32(pb.wrapping_add(0x150), (m.r32(rec + 0xc) as i32 as f32).to_bits());
            let val = m.r32(rec + 4) as i32 as f32;
            e.call(m, BAR_VALUE, &[pb, val.to_bits()]);
            let (a, b, mx) = (m.r32(rec + 4), m.r32(rec + 8), m.r32(rec + 0xc));
            let pb = m.r32(s + 0x68 + 4 * j);
            m.w32(pb.wrapping_add(0x184), ONE);
            m.w32(pb.wrapping_add(0x180), (mx as i32 as f32).to_bits());
            m.w32(pb.wrapping_add(0x158), (a as i32 as f32 + b as i32 as f32).to_bits());
            e.call(m, PRINT, &[v40, 0x3af5d0, a.wrapping_add(b)]);
            let num = m.r32(s + 0x78 + 4 * j);
            vcall(m, e, num, 0x158, &[v40]);
        }
        let g = m.r32(s + 0x8c);
        vcall(m, e, g, 0x28, &[1]);
        let h = m.r32(s + 0x4c);
        e.call(m, SET_STRING, &[h, 0x3af5c8]);
        line = m.r32(s + 0x4c);
    } else {
        let g = m.r32(s + 0x8c);
        vcall(m, e, g, 0x28, &[0]);
        let k = board.wrapping_sub(2);
        // STANDIN: boards 0 and 1 read the game's own stack below the ten texts; here 0. They are the starting
        // boards, never locked
        let text = if k < 10 { m.r32(LOCK_TEXTS + 4 * k) } else { 0 };
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, text]);
        line = m.r32(s + 0x4c);
    }
    vcall(m, e, line, 0x140, &[]);
    let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
    let at = board.wrapping_mul(0x14).wrapping_add(CHARACTERS).wrapping_add(c.wrapping_mul(0xf0));
    let text = match m.r32(at.wrapping_add(8)) as i32 {
        1 => 0x14,
        2 => 0x12,
        _ => 0x13,
    };
    let pl = m.r32(s + 0x88);
    vcall(m, e, pl, 0x150, &[text]);
}

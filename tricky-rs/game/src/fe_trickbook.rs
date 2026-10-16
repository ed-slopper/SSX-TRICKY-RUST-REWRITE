//! The front end's trick book (board row G9p): `cFE3DStateTrickBook`'s methods on the game's bytes, every call through
//! [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the widgets the builder makes in place as
//! [`crate::fe_widgets`]), checked in the function runner (`tests/fe_trickbook.rs`).
//!
//! The menu (+0x3c) holds the six chapters (items 0..5), and its trick list (+0x104, a list widget) the chapter's five
//! tricks (items 7..11). The screen keeps: +0x54 the textures' state (2 the book's loading, 1 a chapter's loading, 0
//! ready), +0x58 the tricks a page (5), +0x5c the chapter's first sprite, +0x60 the chapter shown (-1 none), +0x64 the
//! trick film playing (-1 none), +0x68 the films' archive, +0xf4 set when backing out, +0xfc the picture, +0x100 the
//! rank colour bar, +0x108 the page count, +0x10c the trick's name, +0x110 the book's cover. +0x2e is the player.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use crate::fe_characcept::{CAREER, PLAYER_SLOT};
use crate::fe_charselect::{PRINT, SEEN, SET_STRING, SLOT_PROFILE, SPRITE_RECT};
use crate::fe_numplayers::SET_TEXT;
use crate::fe_riderprofile::TEXTURES_IN;
use crate::fe_modeselect::BUILD_TITLED;
use crate::fe_title::{ADD_WIDGET, BACKDROP, HELP_BUTTON, HELP_LAYOUT, TEXT_CTOR};
use crate::fe_username::{ICON_COLOUR, ICON_FRAMES};
use crate::fe_widgets::{group_init, icon_init, vcall_class, BUFFERED_TEXT_VTABLE, GROUP_ADD, ICON_VTABLE};
use crate::fe_venuesel::{LOC_STRING, WSTR_FORMAT};
use crate::pf::Mem;

pub const VTABLE: u32 = 0x3b01f0;
/// The calls: the career's trick book (career, slot), its trick done (book, trick), its complete pages (book), the
/// chapter's tricks done (book, chapter); a trick's id (character, trick) and its name printed wide (out, trick); the
/// rank colour (chapter, out); a player's slot cleared (career, slot); the trick definitions loaded / freed; the
/// sprite set's textures loaded (set, first sprite, 1), the book's (set, book, 1) and a sprite's texture set (set,
/// sprite, its rectangle); the films (the video manager gApp's front end +0x124: open (manager, name, 0x80, 0x80, 1,
/// 0xf), finished, released, all released, a film's texture); the films' archive opened (archive, name, 0x10), closed
/// and deleted (archive, 2); the base's destructor
pub const CAREER_BOOK: u32 = 0x163f18;
pub const TRICK_DONE: u32 = 0x164258;
pub const PAGES_COMPLETE: u32 = 0x164060;
pub const PAGE_DONE_COUNT: u32 = 0x164178;
pub const TRICK_ID: u32 = 0x154638;
pub const TRICK_NAME: u32 = 0x1551c0;
pub const RANK_COLOUR: u32 = 0x19e840;
pub const CLEAR_SLOT: u32 = 0x1637d0;
pub const TRICKS_LOAD: u32 = 0x154580;
pub const TRICKS_FREE: u32 = 0x1545b8;
pub const CHAPTER_TEXTURES: u32 = 0x1f9b08;
pub const BOOK_TEXTURES: u32 = 0x1f9b90;
pub const SPRITE_TEXTURE: u32 = 0x1f9ee8;
pub const FILM_OPEN: u32 = 0x1fff30;
pub const FILM_FINISHED: u32 = 0x2002e8;
pub const FILM_RELEASE: u32 = 0x200098;
pub const FILMS_RELEASE: u32 = 0x200148;
pub const FILM_TEXTURE: u32 = 0x2001a0;
pub const ARCHIVE_OPEN: u32 = 0x23bc28;
pub const ARCHIVE_CLOSE: u32 = 0x23bcc0;
pub const ARCHIVE_DELETE: u32 = 0x23bb98;
pub const BASE_DTOR: u32 = 0x2717a0;
/// The screen's own: the chapter select and its page count
pub const SELECT_CHAPTER: u32 = 0x27a288;
pub const PAGE_COUNT: u32 = 0x27a4a8;
/// The films' archive name ("data/video/tb/tb_%s.big") and a film's ("data\video\tb\%s_%03d.ss")
pub const ARCHIVE_FORMAT: u32 = 0x3af7f0;
pub const FILM_FORMAT: u32 = 0x3af808;
/// The chapters' first sprites by character, and the book's texture set by character
pub const CHAPTER_SPRITES: [u32; 12] = [0x60, 0x108, 0xd0, 0x140, 0x178, 0x1b0, 0x98, 0x28, 0x1e8, 0x220, 0x258, 0x290];
pub const BOOK_SETS: [u32; 12] = [5, 8, 7, 9, 10, 11, 6, 4, 12, 13, 14, 15];

const ONE: u32 = 0x3f80_0000;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

fn byte(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

fn vcall0(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let fun = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, fun, &a)
}

/// The player's slot (its first byte, signed)
fn slot(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let p = byte(m, s + 0x2e) as u32;
    let at = e.call(m, PLAYER_SLOT, &[CAREER, p]);
    byte(m, at) as u32
}

/// The picture back on the trick list's chosen line's sprite
fn picture_to_list(m: &mut Mem, s: u32) {
    let line = m.r32(m.r32(s + 0x104) + 0x140);
    let pic = m.r32(s + 0xfc);
    let sp = m.r32(line + 0x110);
    m.w32(pic + 0x110, sp);
}

/// The film playing stopped (0x200098 on the front end's video manager)
fn stop_film(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let vm = m.r32(fe(m) + 0x124);
    let h = m.r32(s + 0x64);
    e.call(m, FILM_RELEASE, &[vm, h]);
    m.w32(s + 0x64, u32::MAX);
}

/// 0x27a288 (`FETrickBook_SelectChapter`; the screen, the chapter; `buf` 0x10 bytes): a different chapter: the widgets
/// reset off; the chapter shown; for chapters 0..5, the chapter's textures loaded (the character's first sprite +
/// chapter), loading (+0x54 1), its first sprite (+0x5c) and five tricks, the picture's sprite -5 in red, each trick
/// line's sprite -5 and its +0x40, its +0x30 and +0x24 cleared and closed; the rank colour bar's colour; the page count
pub fn select_chapter(m: &mut Mem, e: &mut dyn FeEnv, s: u32, ch: u32, buf: u32) {
    if m.r32(s + 0x60) == ch {
        return;
    }
    e.call(m, WIDGETS_RESET, &[s, 0]);
    m.w32(s + 0x60, ch);
    if ch >= 6 {
        return;
    }
    let sb = slot(m, e, s);
    let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
    // STANDIN: characters past 11 use a register the caller left; here the first character's
    let first = CHAPTER_SPRITES.get(c as usize).copied().unwrap_or(CHAPTER_SPRITES[0]);
    let set = m.r32(m.r32(GAPP).wrapping_add(0x728));
    e.call(m, CHAPTER_TEXTURES, &[set, first.wrapping_add(ch), 1]);
    m.w32(s + 0x54, 1);
    m.w32(s + 0x5c, first.wrapping_add(ch * 5 + 6));
    m.w32(s + 0x58, 5);
    let pic = m.r32(s + 0xfc);
    m.w32(pic + 0x110, (-5i32) as u32);
    put4(m, buf, [ONE, 0, 0, 0]);
    vcall(m, e, pic, 0x50, &[buf]);
    for k in 0..5u32 {
        let list = m.r32(s + 0x104);
        let w = e.call(m, LIST_NTH, &[list + 0x118, k]);
        m.w32(w + 0x110, (-5i32) as u32);
        vcall(m, e, w, 0x40, &[]);
        m.w32(w + 0x30, 0);
        m.w32(w + 0x24, 0);
        vcall(m, e, w, 0x28, &[0]);
    }
    e.call(m, RANK_COLOUR, &[ch, buf]);
    let bar = m.r32(s + 0x100);
    vcall(m, e, bar, 0x50, &[buf]);
    e.call(m, PAGE_COUNT, &[s, ch]);
}

/// 0x27a4a8 (`FETrickBook_UpdatePageCount`; the screen, the chapter; `buf` 0x90 bytes): "done / of" (string 0xe6b
/// formatted with the chapter's tricks done and the tricks a page) on the page count, centred across 640
pub fn page_count(m: &mut Mem, e: &mut dyn FeEnv, s: u32, ch: u32, buf: u32) {
    let sb = slot(m, e, s);
    let book = e.call(m, CAREER_BOOK, &[CAREER, sb]);
    let done = e.call(m, PAGE_DONE_COUNT, &[book, ch]);
    let loc = m.r32(m.r32(GAPP).wrapping_add(0x50));
    let fmt = e.call(m, LOC_STRING, &[loc, 0xe6b]);
    let per = m.r32(s + 0x58);
    e.call(m, WSTR_FORMAT, &[buf + 0x10, fmt, done, per]);
    let t = m.r32(s + 0x108);
    vcall(m, e, t, 0x148, &[buf + 0x10]);
    let t = m.r32(s + 0x108);
    vcall(m, e, t, 0x130, &[buf]);
    let x = (640.0 - fl(m.r32(buf))) * 0.5;
    let t = m.r32(s + 0x108);
    let y = m.r32(t + 0x84);
    put4(m, buf + 0x80, [x.to_bits(), y, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[t, buf + 0x80]);
}

/// `cFE3DStateTrickBook::Update` 0x27dc58 (`buf` 0x20 bytes, and the base's): a film that has finished released and the
/// picture back on the list's sprite; the book's textures in (state 2): the cover's sprite 0x86, closed, white, ready,
/// chapter 0 shown; a chapter's textures in (state 1): each trick line opened, unhidden, enabled when done, its sprite
/// 0x7c + k; the list centred across 640 by its line width (+0x15c + 90) and the tricks a page, its arrows placed
/// either side, the picture on the chosen line's sprite and colour, ready, the widgets reset onto the menu unless the
/// screen is fading out; then the base's `Update`
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let sb = slot(m, e, s);
    if (m.r32(s + 0x64) as i32) >= 0 {
        let vm = m.r32(fe(m) + 0x124);
        let h = m.r32(s + 0x64);
        if e.call(m, FILM_FINISHED, &[vm, h]) != 0 {
            stop_film(m, e, s);
            picture_to_list(m, s);
        }
    }
    match m.r32(s + 0x54) {
        2 => {
            let tex = m.r32(m.r32(GAPP).wrapping_add(0x728));
            if e.call(m, TEXTURES_IN, &[tex]) != 0 {
                let cover = m.r32(s + 0x110);
                m.w32(cover + 0x110, 0x86);
                vcall(m, e, cover, 0x20, &[0]);
                let cover = m.r32(s + 0x110);
                put4(m, buf, [ONE; 4]);
                vcall(m, e, cover, 0x50, &[buf]);
                m.w32(s + 0x54, 0);
                e.call(m, SELECT_CHAPTER, &[s, 0]);
            }
        }
        1 => {
            let tex = m.r32(m.r32(GAPP).wrapping_add(0x728));
            if e.call(m, TEXTURES_IN, &[tex]) != 0 {
                let book = e.call(m, CAREER_BOOK, &[CAREER, sb]);
                let first = m.r32(m.r32(m.r32(s + 0x3c) + 0x140)).wrapping_mul(5);
                let mut k = 0u32;
                while (k as i32) < m.r32(s + 0x58) as i32 {
                    let list = m.r32(s + 0x104);
                    let w = e.call(m, LIST_NTH, &[list + 0x118, k]);
                    vcall(m, e, w, 0x28, &[1]);
                    vcall(m, e, w, 0x20, &[0]);
                    let done = e.call(m, TRICK_DONE, &[book, first.wrapping_add(k)]);
                    vcall(m, e, w, 0x100, &[(done != 0) as u32]);
                    m.w32(w + 0x110, k + 0x7c);
                    k += 1;
                }
                let list = m.r32(s + 0x104);
                vcall(m, e, list, 0x160, &[0]);
                let list = m.r32(s + 0x104);
                let n = m.r32(s + 0x58) as i32 as f32;
                let lw = fl(m.r32(list + 0x15c));
                let step = lw + 90.0;
                let mut x = (640.0 - n * step) * 0.5;
                let y = m.r32(list + 0x84);
                put4(m, buf, [x.to_bits(), y, 0, ONE]);
                e.call(m, WIDGET_PLACE, &[list, buf]);
                let left = m.r32(m.r32(s + 0x104) + 0x148);
                vcall(m, e, left, 0x130, &[buf]);
                let ly = m.r32(left + 0x84);
                put4(m, buf + 0x10, [(x - fl(m.r32(buf)) - lw).to_bits(), ly, 0, ONE]);
                e.call(m, WIDGET_PLACE, &[left, buf + 0x10]);
                let n = m.r32(s + 0x58) as i32 as f32;
                let right = m.r32(m.r32(s + 0x104) + 0x14c);
                x += n * step;
                let ry = m.r32(right + 0x84);
                put4(m, buf + 0x10, [x.to_bits(), ry, 0, ONE]);
                e.call(m, WIDGET_PLACE, &[right, buf + 0x10]);
                let line = m.r32(m.r32(s + 0x104) + 0x140);
                let pic = m.r32(s + 0xfc);
                let sp = m.r32(line + 0x110);
                m.w32(pic + 0x110, sp);
                vcall(m, e, pic, 0x50, &[line + 0x60]);
                m.w32(s + 0x54, 0);
                if m.r32(s + 0x30) == 0 {
                    let menu = m.r32(s + 0x3c);
                    e.call(m, WIDGETS_RESET, &[s, menu]);
                }
            }
        }
        _ => {}
    }
    base::update(m, e, s, buf);
}

/// `cFE3DStateTrickBook::Activate` 0x27e008 (the screen, the one before; `buf` 0x100 bytes): the 3D scene's +0xa8; not
/// backing out; the trick definitions loaded; the book's textures loaded first time (the cover still -5; set by
/// character, state 2), else chapter 0 shown if none is; the films' archive opened ("data/video/tb/tb_" and the
/// character's name); the chapters up to the complete pages opened, the rest closed; the menu laid out; the base's
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let scene = m.r32(fe(m) + 0xa4);
    vcall0(m, e, scene, 0xa8, &[]);
    m.w32(s + 0xf4, 0);
    e.call(m, TRICKS_LOAD, &[]);
    let sb = slot(m, e, s);
    let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
    let cover = m.r32(s + 0x110);
    if m.r32(cover + 0x110) == (-5i32) as u32 {
        // STANDIN: characters past 11 use a register the caller left; here the first character's set
        let set = BOOK_SETS.get(c as usize).copied().unwrap_or(BOOK_SETS[0]);
        let tex = m.r32(m.r32(GAPP).wrapping_add(0x728));
        e.call(m, BOOK_TEXTURES, &[tex, set, 1]);
        m.w32(s + 0x54, 2);
    } else if m.r32(s + 0x60) == u32::MAX && m.r32(s + 0x54) == 0 {
        e.call(m, SELECT_CHAPTER, &[s, 0]);
    }
    let name = m.r32(c.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(8));
    e.call(m, PRINT, &[buf, ARCHIVE_FORMAT, name]);
    e.call(m, ARCHIVE_OPEN, &[s + 0x68, buf, 0x10]);
    let book = e.call(m, CAREER_BOOK, &[CAREER, sb]);
    let done = e.call(m, PAGES_COMPLETE, &[book]) as i32;
    for k in 0..6i32 {
        let menu = m.r32(s + 0x3c);
        let w = e.call(m, LIST_NTH, &[menu + 0x118, k as u32]);
        vcall(m, e, w, 0x28, &[(k <= done) as u32]);
    }
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x140, &[]);
    base::activate(m, e, s);
}

/// `cFE3DStateTrickBook::Deactivate` 0x27e2a8: the trick definitions freed; a film playing stopped and the picture back
/// on the list's sprite; all films released; the archive closed; the base's
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, TRICKS_FREE, &[]);
    if (m.r32(s + 0x64) as i32) >= 0 {
        stop_film(m, e, s);
        picture_to_list(m, s);
    }
    let vm = m.r32(fe(m) + 0x124);
    e.call(m, FILMS_RELEASE, &[vm]);
    e.call(m, ARCHIVE_CLOSE, &[s + 0x68]);
    base::deactivate(m, e, s);
}

/// `cFE3DStateTrickBook` vf21 0x27e338 (backing out, the item): nothing while loading; from a trick (7..11) back to the
/// chapters (the widgets reset onto the menu, the picture on the list's sprite, the name line's string 0x3af5c8); else
/// backing out of the book (+0xf4) with the base's vf21
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    if m.r32(s + 0x54) != 0 {
        return;
    }
    let k = m.r32(item) as i32;
    if (7..12).contains(&k) {
        let menu = m.r32(s + 0x3c);
        e.call(m, WIDGETS_RESET, &[s, menu]);
        picture_to_list(m, s);
        let name = m.r32(s + 0x10c);
        e.call(m, SET_STRING, &[name, 0x3af5c8]);
    } else {
        m.w32(s + 0xf4, 1);
        base::vf21(m, e, s);
    }
}

/// `cFE3DStateTrickBook` vf26 0x27e3c0 (picking, the item; `buf` 0x120 bytes): nothing while loading; a chapter (0..5):
/// shown (with the list on its first line) when it isn't, the widgets reset onto the trick list, the picture on the
/// chosen line's sprite and colour; a trick (7..11): its film (the character's name and chapter × 5 + trick) opened
/// unless one is playing (0x1fff30), the film's texture onto the picture's sprite 0x7b (the sprite's rectangle with
/// the texture and 128 × 128); anything else the base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    if m.r32(s + 0x54) != 0 {
        return;
    }
    let sb = slot(m, e, s);
    let k = m.r32(item) as i32;
    if (0..6).contains(&k) {
        if m.r32(s + 0x60) != k as u32 {
            e.call(m, SELECT_CHAPTER, &[s, k as u32]);
            let list = m.r32(s + 0x104);
            vcall(m, e, list, 0x160, &[0]);
        }
        let list = m.r32(s + 0x104);
        e.call(m, WIDGETS_RESET, &[s, list]);
        let line = m.r32(m.r32(s + 0x104) + 0x140);
        let pic = m.r32(s + 0xfc);
        let sp = m.r32(line + 0x110);
        m.w32(pic + 0x110, sp);
        vcall(m, e, pic, 0x50, &[line + 0x60]);
    } else if (7..12).contains(&k) {
        let idx = m.r32(s + 0x60).wrapping_mul(5).wrapping_add((k - 7) as u32);
        let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
        let name = m.r32(c.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(8));
        e.call(m, PRINT, &[buf, FILM_FORMAT, name, idx]);
        if (m.r32(s + 0x64) as i32) >= 0 {
            return;
        }
        let vm = m.r32(fe(m) + 0x124);
        let h = e.call(m, FILM_OPEN, &[vm, buf, 0x80, 0x80, 1, 0xf]);
        m.w32(s + 0x64, h);
        let r = e.call(m, SPRITE_RECT, &[s, 0x7b]);
        for w in 0..7u32 {
            let v = m.r32(r.wrapping_add(4 * w));
            m.w32(buf + 0x100 + 4 * w, v);
        }
        let vm = m.r32(fe(m) + 0x124);
        let h = m.r32(s + 0x64);
        let tex = e.call(m, FILM_TEXTURE, &[vm, h]);
        m.w32(buf + 0x100, tex);
        m.w32(buf + 0x108, 128.0f32.to_bits());
        m.w32(buf + 0x104, 128.0f32.to_bits());
        let set = m.r32(m.r32(GAPP).wrapping_add(0x728));
        e.call(m, SPRITE_TEXTURE, &[set, 0x7b, buf + 0x100]);
        let pic = m.r32(s + 0xfc);
        m.w32(pic + 0x110, 0x7b);
    } else {
        base::vf26(m, e, s);
    }
}

/// `cFE3DStateTrickBook` vf22 0x27e5d0 (the item that goes, the one that comes; `buf` 0x80 bytes): a film playing
/// stopped; a chapter coming: its page count, the help bar's layout 0x800a, shown once the book is in and ready, the
/// help text 0xdd9; a trick coming: the picture on its sprite and colour, its name (the trick's id 0x154638 for the
/// character and chapter × 5 + trick, printed wide), the rank colour bar's colour for its chapter, the help bar's
/// layout 0x800c with its fourth button enabled for a done trick (+0xa8), the help text 0xdda
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32, buf: u32) {
    let _ = from;
    if (m.r32(s + 0x64) as i32) >= 0 {
        stop_film(m, e, s);
    }
    let sb = slot(m, e, s);
    let k = m.r32(to) as i32;
    if (0..6).contains(&k) {
        e.call(m, PAGE_COUNT, &[s, k as u32]);
        e.call(m, HELP_LAYOUT, &[s, 0x800a]);
        let cover = m.r32(s + 0x110);
        if m.r32(cover + 0x110) != (-5i32) as u32 && m.r32(s + 0x54) == 0 {
            let k = m.r32(to);
            e.call(m, SELECT_CHAPTER, &[s, k]);
        }
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, 0xdd9]);
    } else if (7..12).contains(&k) {
        let pic = m.r32(s + 0xfc);
        let sp = m.r32(to + 0x110);
        m.w32(pic + 0x110, sp);
        vcall(m, e, pic, 0x50, &[to + 0x60]);
        let idx = (m.r32(s + 0x60) as i32).wrapping_mul(5).wrapping_add(k - 7);
        let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
        let t = e.call(m, TRICK_ID, &[c, idx as u32]);
        e.call(m, TRICK_NAME, &[buf, t]);
        e.call(m, RANK_COLOUR, &[(idx / 5) as u32, buf + 0x70]);
        let bar = m.r32(s + 0x100);
        vcall(m, e, bar, 0x50, &[buf + 0x70]);
        let name = m.r32(s + 0x10c);
        e.call(m, SET_STRING, &[name, buf]);
        let name = m.r32(s + 0x10c);
        vcall(m, e, name, 0x140, &[]);
        e.call(m, HELP_LAYOUT, &[s, 0x800c]);
        let on = m.r32(to + 0xa8) != 0;
        let b = e.call(m, HELP_BUTTON, &[s, 4]);
        vcall(m, e, b, 0x100, &[on as u32]);
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, 0xdda]);
    }
}

/// `cFE3DStateTrickBook` vf24 0x27e7f8 (the book left for a lesson): the second player's slot cleared after its rider
/// is marked seen with the first's, or the first's alone; the game mode 6 (the trick book event); the 3D scene's
/// +0x10(-1); the front end's game type 3; the screen's +0x28 and +0x48
pub fn vf24(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if byte(m, s + 0x2e) == 1 {
        let at = e.call(m, PLAYER_SLOT, &[CAREER, 1]);
        let sb = byte(m, at) as u32;
        let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
        e.call(m, CLEAR_SLOT, &[CAREER, 0]);
        e.call(m, CLEAR_SLOT, &[CAREER, 1]);
        e.call(m, SEEN, &[CAREER, 0, sb, c]);
    } else {
        e.call(m, CLEAR_SLOT, &[CAREER, 1]);
    }
    m.w32(crate::fe_modeselect::GAME_MODE, 6);
    let scene = m.r32(fe(m) + 0xa4);
    vcall0(m, e, scene, 0x10, &[u32::MAX]);
    let f = fe(m);
    m.w32(f + 0xf0, 3);
    vcall(m, e, s, 0x28, &[3]);
    vcall(m, e, s, 0x48, &[]);
}

/// `cFE3DStateTrickBook::~cFE3DStateTrickBook` 0x28e360 (the screen, the delete flag): the vtable, the films' archive
/// deleted, the base's destructor
pub fn destroy(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flag: u32) {
    m.w32(s + 0xc, VTABLE);
    e.call(m, ARCHIVE_DELETE, &[s + 0x68, 2]);
    e.call(m, BASE_DTOR, &[s, flag]);
}

/// The builder's other calls: a text box made (0x2b0b90: the object, the screen, its parent, the font) and its scale
/// (0x2b4cc8, f12), the trick list made (0x26d3b8: the object, the screen, its parent, 0, 0, 1); the chapters' texts
/// (six words at 0x3af6f0), the page count's first string (0x3af7d8), the cover's frame time (the word at 0x348020);
/// the tiled icon's and the list's vtables
pub const TEXT_BOX: u32 = 0x2b0b90;
pub const TEXT_BOX_SCALE: u32 = 0x2b4cc8;
pub const LIST_CTOR: u32 = 0x26d3b8;
pub const CHAPTER_TEXTS: u32 = 0x3af6f0;
pub const PAGE_COUNT_TEXT: u32 = 0x3af7d8;
pub const FRAME_TIME: u32 = 0x348020;
pub const TILED_VTABLE: u32 = 0x3b6e40;
pub const LIST_VTABLE: u32 = 0x3b7fe0;

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// An icon made in place (`tag`, `size` bytes) under `parent`, sized 10 × 10 through the icon class (`b10` its vector)
fn icon(m: &mut Mem, e: &mut dyn FeEnv, s: u32, tag: u32, size: u32, parent: u32, b10: u32) -> u32 {
    let ic = alloc(m, e, tag, size);
    icon_init(m, e, ic, s, parent);
    put4(m, b10, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[b10]);
    ic
}

fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, b: u32, x: f32, y: f32) {
    put4(m, b, [x.to_bits(), y.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[w, b]);
}

fn size(m: &mut Mem, e: &mut dyn FeEnv, w: u32, b: u32, x: f32, y: f32) {
    put4(m, b, [x.to_bits(), y.to_bits(), 0, 0]);
    vcall(m, e, w, 0x138, &[b]);
}

fn colour(m: &mut Mem, e: &mut dyn FeEnv, w: u32, b: u32, c: [u32; 4]) {
    put4(m, b, c);
    vcall(m, e, w, 0x50, &[b]);
}

/// `cFE3DStateTrickBook` vf11 0x27a5b8, the screen built (`buf` 0x30 bytes of scratch):
/// - the base widgets with the title (0x274208: 105.0, text 0xdb0, 0x800e), the menu's bytes +0x112 / +0x114 / +0x116
///   4, the help bar's layout 0x800a, no back; the six chapters' items (the texts at 0x3af6f0);
/// - a backdrop (640 × 480 tinted (1, 1.11, 0.86, 0)); the page band (the tiled sprite 0x3c, 640 × 80 at y 320); the
///   cover (+0x110: 195 × 195 at y 150, its colour cycle, hidden behind sprite -5) and the character's name over it;
///   the rank colour bar (+0x100, sprite 0x2b at (480, 105)); the trick name box (+0x10c, 150 × 175 at (460, 142));
///   the picture (+0xfc, 257 × 158 at (320, 205)) and its frame (eight pieces of sprite 0x29);
/// - the trick page (+0xf8, a group at y 305): three lines of sprite 0x29, the trick list (+0x104 at (68, 10): five
///   trick icons numbered 7..11 with their colour cycles, its arrows at x 29 and 563), its cursor (+0x194, sprite
///   0x2e) and the page count (+0x108, at y 65 under the help bar);
/// - no chapter shown; the backdrop; the cursor moved onto the list's first line; the widgets reset off.
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (b0, b10, b20) = (buf, buf + 0x10, buf + 0x20);
    let half = 0.5f32.to_bits();
    let tenth = 0.1f32.to_bits();
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xdb0, 0x800e]);
    let menu = m.r32(s + 0x3c);
    m.w8(menu + 0x112, 4);
    m.w8(menu + 0x116, 4);
    m.w8(menu + 0x114, 4);
    e.call(m, HELP_LAYOUT, &[s, 0x800a]);
    m.w32(s + 0x18, 0);
    let sb = slot(m, e, s);
    let menu = m.r32(s + 0x3c);
    let word = m.r32(0x3ae520);
    for k in 0..6u32 {
        let at = alloc(m, e, 0x3af520, 0x130);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        let id = m.r32(CHAPTER_TEXTS + 4 * k);
        m.w32(w + 0x118, word);
        vcall(m, e, w, 0x150, &[id]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
    }

    // the backdrop
    let ic = icon(m, e, s, 0x3af708, 0x120, 0, b10);
    colour(m, e, ic, b0, [ONE, 0x3f8e_8db9, 0x3f5c_d9e8, 0]);
    size(m, e, ic, b0, 640.0, 480.0);
    place(m, e, ic, b0, 0.0, 0.0);
    e.call(m, ADD_WIDGET, &[s, ic]);

    // the page band (a tiled icon, 0x130 bytes)
    let ib = icon(m, e, s, 0x3af718, 0x130, 0, b10);
    m.w32(ib + 0x70, 1);
    m.w32(ib + 0x120, ONE);
    m.w32(ib + 0x124, ONE);
    m.w32(ib + 0xc, TILED_VTABLE);
    put4(m, b0, [0x3ec8_c155, ONE, ONE, ONE]);
    vcall_class(m, e, TILED_VTABLE, ib, 0x50, &[b0]);
    m.w32(ib + 0x110, 0x3c);
    m.w32(ib + 0x120, 6.0f32.to_bits());
    m.w32(ib + 0x124, ONE);
    size(m, e, ib, b0, 640.0, 80.0);
    place(m, e, ib, b0, 0.0, 320.0);
    m.w32(ib + 0x114, 4);
    e.call(m, ADD_WIDGET, &[s, ib]);

    // the cover
    let ic = icon(m, e, s, 0x3af728, 0x120, 0, b10);
    colour(m, e, ic, b0, [0x3f61_47ae, tenth, tenth, tenth]);
    m.w32(ic + 0x70, 2);
    size(m, e, ic, b0, 195.0, 195.0);
    place(m, e, ic, b0, 0.0, 150.0);
    put4(m, b0, [ONE, half, half, 0.8f32.to_bits()]);
    put4(m, b10, [ONE; 4]);
    e.call(m, ICON_COLOUR, &[ic, 5, b0]);
    let t = m.r32(FRAME_TIME);
    e.call(m, ICON_FRAMES, &[ic, 5, b10, t, 2]);
    vcall(m, e, ic, 0x100, &[1]);
    vcall(m, e, ic, 0x20, &[1]);
    m.w32(s + 0x110, ic);
    m.w32(ic + 0x110, (-5i32) as u32);
    e.call(m, ADD_WIDGET, &[s, ic]);

    // the character's name, centred over the cover
    let at = alloc(m, e, 0x3af738, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let tt = e.call(m, TEXT_CTOR, &[at, s, 0, font]);
    m.w32(tt + 0x118, ONE);
    put4(m, b0, [ONE; 4]);
    vcall(m, e, tt, 0x110, &[b0]);
    colour(m, e, tt, b0, [ONE; 4]);
    m.w32(tt + 0x70, 3);
    let c = e.call(m, SLOT_PROFILE, &[CAREER, sb]);
    let name = m.r32(c.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(4));
    vcall(m, e, tt, 0x158, &[name]);
    let cover = m.r32(s + 0x110);
    vcall(m, e, cover, 0x130, &[b0]);
    vcall(m, e, tt, 0x130, &[b10]);
    let cx = fl(m.r32(m.r32(s + 0x110) + 0x80));
    let x = cx + (fl(m.r32(b0)) - fl(m.r32(b10))) * 0.5;
    place(m, e, tt, b20, x, 105.0);
    e.call(m, ADD_WIDGET, &[s, tt]);

    // the rank colour bar
    let r = e.call(m, SPRITE_RECT, &[s, 0x2b]);
    let rw = fl(m.r32(r.wrapping_add(4)));
    let r = e.call(m, SPRITE_RECT, &[s, 0x2b]);
    let rh = fl(m.r32(r.wrapping_add(8)));
    let ic = icon(m, e, s, 0x3af748, 0x120, 0, b10);
    m.w32(ic + 0x110, 0x2b);
    m.w32(s + 0x100, ic);
    colour(m, e, ic, b0, [ONE; 4]);
    let bar = m.r32(s + 0x100);
    m.w32(bar + 0x70, 2);
    size(m, e, bar, b0, rw, rh);
    let bar = m.r32(s + 0x100);
    place(m, e, bar, b0, 480.0, 105.0);
    let bar = m.r32(s + 0x100);
    e.call(m, ADD_WIDGET, &[s, bar]);

    // the trick name box
    let at = alloc(m, e, 0x3af758, 0x15a0);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let tb = e.call(m, TEXT_BOX, &[at, s, 0, font]);
    m.w32(s + 0x10c, tb);
    size(m, e, tb, b0, 150.0, 175.0);
    let tb = m.r32(s + 0x10c);
    place(m, e, tb, b0, 460.0, 142.0);
    let tb = m.r32(s + 0x10c);
    m.w32(tb + 0x70, 2);
    colour(m, e, tb, b0, [ONE, 0, 0, 0]);
    let tb = m.r32(s + 0x10c);
    e.call(m, TEXT_BOX_SCALE, &[tb, 0.85f32.to_bits()]);
    let tb = m.r32(s + 0x10c);
    m.w32(tb + 0x114, 0);
    m.w32(tb + 0x198, 0);
    m.w32(tb + 0x11c, 1);
    e.call(m, ADD_WIDGET, &[s, tb]);

    // the picture
    let ic = icon(m, e, s, 0x3af768, 0x120, 0, b10);
    m.w32(s + 0xfc, ic);
    colour(m, e, ic, b0, [ONE, 0, 0, 0]);
    let pic = m.r32(s + 0xfc);
    m.w32(pic + 0x70, 4);
    size(m, e, pic, b0, 257.0, 158.0);
    let pic = m.r32(s + 0xfc);
    place(m, e, pic, b0, 320.0, 205.0);
    let pic = m.r32(s + 0xfc);
    m.w32(pic + 0x114, 0x60);
    e.call(m, ADD_WIDGET, &[s, pic]);

    // the picture's frame: eight pieces of sprite 0x29, half the sprite's height high
    let r = e.call(m, SPRITE_RECT, &[s, 0x29]);
    let fh = fl(m.r32(r.wrapping_add(8))) * 0.5;
    for (w, x, y, frames) in [
        (128.5f32, 191.5f32, 126.0f32, 5u32),
        (128.5, 320.0, 126.0, 4),
        (79.0, 448.5, 126.0, 0x12),
        (79.0, 448.5, 205.0, 0x13),
        (128.5, 320.0, 284.0, 2),
        (128.5, 191.5, 284.0, 3),
        (79.0, 191.5, 205.0, 0x19),
        (79.0, 191.5, 126.0, 0x18),
    ] {
        let ic = icon(m, e, s, 0x3af778, 0x120, 0, b10);
        m.w32(ic + 0x70, 5);
        colour(m, e, ic, b0, [ONE, 0, 0, 0]);
        m.w32(ic + 0x110, 0x29);
        size(m, e, ic, b0, w, fh);
        place(m, e, ic, b0, x, y);
        m.w32(ic + 0x114, frames);
        e.call(m, ADD_WIDGET, &[s, ic]);
    }

    // the trick page: a group, its three lines of sprite 0x29
    let g = alloc(m, e, 0x3af788, 0x140);
    group_init(m, e, g, s, 0);
    m.w32(g + 0x70, 3);
    let r = e.call(m, SPRITE_RECT, &[s, 0x29]);
    let lh = fl(m.r32(r.wrapping_add(8)));
    let ic = icon(m, e, s, 0x3af708, 0x120, g, b10);
    m.w32(ic + 0x110, 0x29);
    colour(m, e, ic, b0, [ONE, 0, 0, 0]);
    size(m, e, ic, b0, 320.0, lh);
    m.w32(ic + 0x114, 1);
    place(m, e, ic, b0, 0.0, 0.0);
    e.call(m, GROUP_ADD, &[g, ic]);
    let ic = icon(m, e, s, 0x3af708, 0x120, g, b10);
    m.w32(ic + 0x110, 0x29);
    colour(m, e, ic, b0, [ONE, 0, 0, 0]);
    size(m, e, ic, b0, 320.0, lh);
    place(m, e, ic, b0, 320.0, 0.0);
    e.call(m, GROUP_ADD, &[g, ic]);
    let ic = icon(m, e, s, 0x3af708, 0x120, g, b10);
    m.w32(ic + 0x110, (-5i32) as u32);
    colour(m, e, ic, b0, [ONE, 0, 0, 0]);
    size(m, e, ic, b0, 640.0, 165.0);
    place(m, e, ic, b0, 0.0, lh);
    e.call(m, GROUP_ADD, &[g, ic]);

    // the trick list (made by its constructor; the object used is the one allocated)
    let list = alloc(m, e, 0x3af798, 0x1b0);
    e.call(m, LIST_CTOR, &[list, s, g, 0, 0, 1]);
    m.w8(list.wrapping_add(0x112), 4);
    m.w8(list.wrapping_add(0x116), 4);
    m.w8(list.wrapping_add(0x114), 4);
    m.w32(list.wrapping_add(0x190), 2);
    m.w32(list.wrapping_add(0xc), LIST_VTABLE);
    for o in [0x194u32, 0x198, 0x19c, 0x1a0] {
        m.w32(list.wrapping_add(o), 0);
    }
    vcall_class(m, e, LIST_VTABLE, list, 0x168, &[]);
    m.w32(list.wrapping_add(0x190), 3);
    m.w32(list.wrapping_add(0x15c), 9.0f32.to_bits());
    m.w32(list.wrapping_add(0x13c), 0);
    place(m, e, list, b0, 68.0, 10.0);
    m.w32(list.wrapping_add(0x158), 0);
    vcall(m, e, list, 0xf0, &[0]);
    let arrow_w = f32::from_bits(0x41f0_0001);
    let left = m.r32(list.wrapping_add(0x148));
    m.w32(list.wrapping_add(0x70), 1);
    size(m, e, left, b0, arrow_w, 40.0);
    let right = m.r32(list.wrapping_add(0x14c));
    size(m, e, right, b0, arrow_w, 40.0);
    let left = m.r32(list.wrapping_add(0x148));
    place(m, e, left, b0, 29.0, 15.0);
    let right = m.r32(list.wrapping_add(0x14c));
    place(m, e, right, b0, 563.0, 15.0);
    m.w32(list.wrapping_add(0x144), 1);
    let pale = 0.3f32.to_bits();
    for k in 0..5u32 {
        let ic = alloc(m, e, 0x3af7a8, 0x120);
        icon_init(m, e, ic, s, list);
        put4(m, b10, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
        vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[b10]);
        size(m, e, ic, b0, 90.0, 50.0);
        m.w32(ic, k + 7);
        for (slot, c) in [(0x108u32, [ONE; 4]), (0x110, [ONE; 4]), (0x118, [ONE, ONE, pale, pale]), (0x120, [ONE, ONE, pale, pale])] {
            put4(m, b0, c);
            vcall(m, e, ic, slot, &[b0]);
        }
        put4(m, b0, [ONE, 0, 0, 0]);
        put4(m, b10, [ONE, half, half, 0.8f32.to_bits()]);
        e.call(m, ICON_COLOUR, &[ic, 5, b0]);
        let t = m.r32(FRAME_TIME);
        e.call(m, ICON_FRAMES, &[ic, 5, b10, t, 1]);
        vcall(m, e, ic, 0x100, &[1]);
        vcall(m, e, list, 0x148, &[ic]);
    }
    e.call(m, GROUP_ADD, &[g, list]);
    vcall(m, e, g, 0x20, &[1]);
    m.w32(s + 0x104, list);

    // the list's cursor
    let cur = alloc(m, e, 0x3af7b8, 0x120);
    let list = m.r32(s + 0x104);
    icon_init(m, e, cur, s, list);
    put4(m, b10, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, cur, 0x138, &[b10]);
    size(m, e, cur, b0, 100.0, 60.0);
    m.w32(cur + 0x70, 2);
    colour(m, e, cur, b0, [ONE; 4]);
    put4(m, b0, [ONE; 4]);
    m.w32(cur + 0x110, 0x2e);
    put4(m, b10, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
    e.call(m, ICON_COLOUR, &[cur, 5, b0]);
    e.call(m, ICON_FRAMES, &[cur, 5, b10, 0x1e, 2]);
    vcall(m, e, cur, 0x20, &[1]);
    let list = m.r32(s + 0x104);
    m.w32(list + 0x194, cur);

    // the page count, under the help bar, centred across 640 at y 65
    let pc = alloc(m, e, 0x3af7c8, 0x330);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    e.call(m, TEXT_CTOR, &[pc, s, g, font]);
    m.w32(s + 0x108, pc);
    m.w16(pc.wrapping_add(0x130), 0);
    m.w32(pc.wrapping_add(0xc), BUFFERED_TEXT_VTABLE);
    vcall_class(m, e, BUFFERED_TEXT_VTABLE, pc, 0x158, &[PAGE_COUNT_TEXT]);
    let pc = m.r32(s + 0x108);
    m.w32(pc + 0x118, 0.7f32.to_bits());
    put4(m, b0, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
    vcall(m, e, pc, 0x110, &[b0]);
    let pc = m.r32(s + 0x108);
    colour(m, e, pc, b0, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
    let pc = m.r32(s + 0x108);
    m.w32(pc + 0x70, 4);
    vcall(m, e, pc, 0x130, &[b0]);
    let x = (640.0 - fl(m.r32(b0))) * 0.5;
    let pc = m.r32(s + 0x108);
    place(m, e, pc, b10, x, 65.0);
    let bar = m.r32(s + 0x44);
    let pc = m.r32(s + 0x108);
    e.call(m, GROUP_ADD, &[bar, pc]);

    // the page at y 305
    m.w32(s + 0xf8, g);
    vcall(m, e, g, 0x140, &[]);
    place(m, e, g, b0, 0.0, 305.0);
    e.call(m, ADD_WIDGET, &[s, g]);
    m.w32(s + 0x60, u32::MAX);
    e.call(m, BACKDROP, &[s]);

    // the cursor moved by (the first line − 5, − 5) from where it is (w 1)
    let list = m.r32(s + 0x104);
    let first = e.call(m, LIST_NTH, &[list + 0x118, 0]);
    let cur = m.r32(m.r32(s + 0x104) + 0x194);
    let d = [
        (fl(m.r32(first + 0x80)) - 5.0) - fl(m.r32(cur + 0x80)),
        (fl(m.r32(first + 0x84)) - 5.0) - fl(m.r32(cur + 0x84)),
        fl(m.r32(first + 0x88)) - fl(m.r32(cur + 0x88)),
    ];
    put4(m, b10, [d[0].to_bits(), d[1].to_bits(), d[2].to_bits(), ONE]);
    vcall(m, e, cur, 0x70, &[b10]);
    e.call(m, WIDGETS_RESET, &[s, 0]);
}

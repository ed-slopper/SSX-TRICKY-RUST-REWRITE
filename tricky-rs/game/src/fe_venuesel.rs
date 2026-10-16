//! The front end's venue select (board row G9g): `cFE3DStateVenueSel`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]), checked in the function runner
//! (`tests/fe_venuesel.rs`).
//!
//! The menu (+0x3c) has the ten courses in the menu order of the byte table at 0x3a8af0 (each item's number is its
//! place in that table; the screen keeps the ten text widgets at +0x54..+0x7c). +0x4c is the help line, which here
//! carries the event's title.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, TRANSITION_START};
use crate::fe_eventsel::MUSIC_VF18;
use crate::fe_modeselect::{BUILD_TITLED, GAME_MODE};
use crate::fe_numplayers::SET_TEXT;
use crate::fe_title::{BACKDROP, TEXT_CTOR};
use crate::pf::Mem;

/// The courses in menu order (signed bytes)
pub const COURSE_ORDER: u32 = 0x3a8af0;
/// The chosen course word
pub const GAME_COURSE: u32 = 0x32de88;
/// The course table (0x1c bytes a course; +4 its name)
pub const COURSES: u32 = 0x332f28;
/// The career block (`Career_*`'s object)
pub const CAREER: u32 = 0x32b390;
/// The calls: 0x2657a8 the race set up from the menu, 0x163bb8 whether a course is unlocked (career, mode, course),
/// 0x269be8 a course marker's fade (f12 the level, the marker, 2), 0x240ed8 a name's hash, 0x29baa0 the locks,
/// 0x29c010 the event title; the music side (0x262d80 the medals shown, 0x263000 a course's video, 0x2624b8 the
/// venue select's loop, 0x263150 the venue chosen); the overlay's camera (0x26c1a8 the next venue, 0x26c090 a pan to
/// a venue, 0x26c2c0 the venue's shot); the title's text (0x16b8e0 the World Circuit event's, 0x16b860 the course's
/// name, 0x2c5160 a string by id, 0x17cc80 a wide copy, 0x17ce38 a wide format, 0x2b0f18 a text widget's wide text)
pub const SETUP_RACE: u32 = 0x2657a8;
pub const COURSE_UNLOCKED: u32 = 0x163bb8;
pub const MARKER_FADE: u32 = 0x269be8;
pub const NAME_HASH: u32 = 0x240ed8;
pub const UPDATE_LOCKS: u32 = 0x29baa0;
pub const FILL_TITLE: u32 = 0x29c010;
pub const SHOW_MEDALS: u32 = 0x262d80;
pub const COURSE_VIDEO: u32 = 0x263000;
pub const MUSIC_LOOP: u32 = 0x2624b8;
pub const SET_VENUE: u32 = 0x263150;
pub const CAMERA_NEXT: u32 = 0x26c1a8;
pub const CAMERA_PAN: u32 = 0x26c090;
pub const CAMERA_SHOT: u32 = 0x26c2c0;
pub const CIRCUIT_TITLE: u32 = 0x16b8e0;
pub const COURSE_NAME: u32 = 0x16b860;
pub const LOC_STRING: u32 = 0x2c5160;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const WSTR_FORMAT: u32 = 0x17ce38;
pub const SET_WIDE_TEXT: u32 = 0x2b0f18;
/// The item widgets' +0x118 word
pub const ITEM_WORD: u32 = 0x3ae520;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn course_at(m: &Mem, k: u32) -> u32 {
    m.r8(COURSE_ORDER.wrapping_add(k)) as i8 as i32 as u32
}

fn byte(m: &Mem, at: u32) -> u32 {
    m.r8(at) as i8 as i32 as u32
}

/// `cFE3DStateVenueSel::CreateNextState` 0x29ba50: the chosen course (the item's place in the order table) to
/// 0x32de88, the race set up (0x2657a8); no next screen
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let f = fe(m);
    let item = m.r32(m.r32(m.r32(s + 0x3c) + 0x140));
    let c = course_at(m, item);
    m.w32(GAME_COURSE, c);
    e.call(m, SETUP_RACE, &[f]);
    0
}

/// 0x29baa0 (`FEVenueSel_UpdateLocks`): each of the ten courses' lock for the game mode (0x163bb8) to its widget's
/// +0x100 and its marker's fade (0x269be8: 1.0 unlocked, else 0, on the front end's +0xfc + 4k, 2)
pub fn update_locks(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    for k in 0..10u32 {
        let c = course_at(m, k);
        let mode = m.r32(GAME_MODE);
        let open = e.call(m, COURSE_UNLOCKED, &[CAREER, mode, c]);
        let w = m.r32(s + 0x54 + 4 * k);
        vcall(m, e, w, 0x100, &[open]);
        let marker = m.r32(fe(m) + 0xfc + 4 * k);
        let level = if open != 0 { 1.0f32 } else { 0.0 };
        e.call(m, MARKER_FADE, &[level.to_bits(), marker, 2]);
    }
}

/// `cFE3DStateVenueSel` vf11 0x29bb90, the screen built: the base widgets with the title (0x274208: 105.0, text
/// 0xdad, 0x200a); the menu's layout bytes as the mode select's and its +0x158 1; the ten courses' items (each the
/// course's name as its text (+0x158), +0x118 the word at 0x3ae520, +0x74 the name's hash, numbered, kept at
/// +0x54 + 4k); the locks (0x29baa0); the backdrop; the menu on the overlay's venue (its +0x185, a signed byte, via
/// +0x160); the pause overlay off
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    e.call(m, BUILD_TITLED, &[105.0f32.to_bits(), s, 0xdad, 0x200a]);
    let word = m.r32(ITEM_WORD);
    for (o, b) in [(0x111u32, 0u8), (0x110, 3), (0x116, 3), (0x112, 1), (0x113, 2), (0x114, 1), (0x115, 1)] {
        let menu = m.r32(s + 0x3c);
        m.w8(menu + o, b);
    }
    let menu = m.r32(s + 0x3c);
    m.w32(menu + 0x158, 1);
    for k in 0..10u32 {
        let c = course_at(m, k);
        let name = m.r32(COURSES.wrapping_add(c.wrapping_mul(0x1c)).wrapping_add(4));
        let at = e.call(m, ALLOC, &[0x3b2a48, 0x130, 0]);
        if at != 0 {
            e.call(m, FILL, &[at, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(s + 0x54 + 4 * k, w);
        m.w32(w + 0x118, word);
        vcall(m, e, w, 0x158, &[name]);
        let h = e.call(m, NAME_HASH, &[name]);
        m.w32(w + 0x74, h);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
    }
    e.call(m, UPDATE_LOCKS, &[s]);
    e.call(m, BACKDROP, &[s]);
    let venue = byte(m, m.r32(f + 0x50) + 0x185);
    let menu = m.r32(s + 0x3c);
    vcall(m, e, menu, 0x160, &[venue]);
    m.w32(s + 0x28, 0);
}

/// `cFE3DStateVenueSel::Activate` 0x29bd88 (the screen, the one before): the locks, the medals shown; a fade in
/// (+0x20) only after a screen answering 6 from its +0x70; the base's `Activate`; the event title; the chosen
/// course's video (its signed first byte and +0xa8); the venue select's music
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    e.call(m, UPDATE_LOCKS, &[s]);
    let f = fe(m);
    e.call(m, SHOW_MEDALS, &[f + 0x70, 1]);
    let fade = prev != 0 && vcall(m, e, prev, 0x70, &[]) == 6;
    m.w32(s + 0x20, fade as u32);
    base::activate(m, e, s);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, FILL_TITLE, &[s, chosen]);
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    if chosen != 0 {
        let f = fe(m);
        let c = byte(m, chosen);
        let v = m.r32(chosen + 0xa8);
        e.call(m, COURSE_VIDEO, &[f + 0x70, c, v]);
    }
    let f = fe(m);
    e.call(m, MUSIC_LOOP, &[f + 0x70]);
}

/// `cFE3DStateVenueSel::Deactivate` 0x29be58: the medals hidden, ten "no video" calls (-1, 0), the base's
/// `Deactivate`
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    e.call(m, SHOW_MEDALS, &[f + 0x70, 0]);
    for _ in 0..10 {
        let f = fe(m);
        e.call(m, COURSE_VIDEO, &[f + 0x70, u32::MAX, 0]);
    }
    base::deactivate(m, e, s);
}

/// `cFE3DStateVenueSel` vf18 0x29bee0: for game type 4, the music's cue 0x262c60
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    if m.r32(f + 0xf0) == 4 {
        e.call(m, MUSIC_VF18, &[f + 0x70]);
    }
}

/// `cFE3DStateVenueSel::LayoutWidgets` 0x29bf10 (`buf` 0x30 bytes of scratch for the base's): the event title, then
/// the base's `LayoutWidgets`
pub fn layout_widgets(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let chosen = m.r32(m.r32(s + 0x3c) + 0x140);
    e.call(m, FILL_TITLE, &[s, chosen]);
    base::layout_widgets(m, e, s, buf);
}

/// `cFE3DStateVenueSel` vf22 0x29bf40 (the item that goes, the one that comes): the base's vf22; with an item going,
/// the overlay's camera to the coming item's venue (each item's signed first byte): the "next" move (0x26c1a8) when
/// it is one on from the going one, or from 9 to 0, else a pan (0x26c090); then its video and the event title
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 {
        let a = byte(m, from) as i32;
        let b = byte(m, to) as i32;
        let o = m.r32(fe(m) + 0x50);
        let venue = byte(m, to);
        let next = (a < b && a == b - 1) || (b == 0 && a == 9);
        e.call(m, if next { CAMERA_NEXT } else { CAMERA_PAN }, &[o, venue]);
    }
    let f = fe(m);
    let c = byte(m, to);
    let v = m.r32(to + 0xa8);
    e.call(m, COURSE_VIDEO, &[f + 0x70, c, v]);
    e.call(m, FILL_TITLE, &[s, to]);
}

/// 0x29c010 (`FEVenue_FillEventTitle`; the screen, the item, `buf` 0x260 bytes for the wide text): the help line's
/// text for the item:
/// - a World Circuit item (+0xa8 set): the career's event title (0x16b8e0) by id;
/// - courses 6 and 7 (by the order table): their name (0x16b860) while locked, else 0xde7 / 0xde8;
/// - else by the game mode: 1 and 7 the name with 0xd31, 2 and 4 with 0x31, 3 and 5 with 0x32 (wide format); other
///   modes leave it.
pub fn fill_event_title(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    let c = course_at(m, m.r32(item));
    if m.r32(item + 0xa8) != 0 {
        let id = e.call(m, CIRCUIT_TITLE, &[CAREER]);
        let h = m.r32(s + 0x4c);
        e.call(m, SET_TEXT, &[h, id]);
        return;
    }
    if c == 6 || c == 7 {
        let id = if e.call(m, COURSE_UNLOCKED, &[CAREER, 1, c]) == 0 {
            e.call(m, COURSE_NAME, &[CAREER, c])
        } else if c == 6 {
            0xde7
        } else {
            0xde8
        };
        let loc = m.r32(m.r32(GAPP).wrapping_add(0x50));
        let text = e.call(m, LOC_STRING, &[loc, id]);
        e.call(m, WSTR_COPY, &[buf, text]);
    } else {
        let extra = match m.r32(GAME_MODE) {
            1 | 7 => 0xd31,
            2 | 4 => 0x31,
            3 | 5 => 0x32,
            _ => return,
        };
        let loc = m.r32(m.r32(GAPP).wrapping_add(0x50));
        let name = e.call(m, COURSE_NAME, &[CAREER, c]);
        let fmt = e.call(m, LOC_STRING, &[loc, name]);
        let loc = m.r32(m.r32(GAPP).wrapping_add(0x50));
        let arg = e.call(m, LOC_STRING, &[loc, extra]);
        e.call(m, WSTR_FORMAT, &[buf, fmt, arg]);
    }
    let h = m.r32(s + 0x4c);
    e.call(m, SET_WIDE_TEXT, &[h, buf]);
}

/// `cFE3DStateVenueSel` vf26 0x29c238 (the item picked; `buf` 0xc bytes of scratch): the overlay's shot of the venue
/// and the venue chosen (the item's signed first byte), a transition (0x1ff648: 1.0, 10.0, 0, gApp+0x72c, zero
/// vector, 3), the base's vf26
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    let o = m.r32(fe(m) + 0x50);
    let v = byte(m, item);
    e.call(m, CAMERA_SHOT, &[o, v]);
    let f = fe(m);
    let v = byte(m, item);
    e.call(m, SET_VENUE, &[f + 0x70, v]);
    for k in 0..3 {
        m.w32(buf + 4 * k, 0);
    }
    let word = m.r32(m.r32(GAPP).wrapping_add(0x72c));
    e.call(m, TRANSITION_START, &[1.0f32.to_bits(), 10.0f32.to_bits(), 0, word, buf, 3]);
    base::vf26(m, e, s);
}

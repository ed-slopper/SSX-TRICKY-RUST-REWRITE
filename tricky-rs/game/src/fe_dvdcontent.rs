//! The front end's DVD extras screen (board row G9t): `cFE3DStateDVDContent`'s methods on the game's bytes, every call
//! through [`FeEnv`] (the base's methods it calls run as ported, [`crate::fe3dstate`]; the widgets the builder makes in
//! place as [`crate::fe_widgets`]; its four sub-menus are built by the front end's own builders, called out), checked
//! in the function runner (`tests/fe_dvdcontent.rs`).
//!
//! The screen's menu (+0x3c) has nine items; four of them open a sub-menu: the characters (+0x68), the demos (+0x6c),
//! the jukebox (+0x70, two columns of songs) and the promos (+0x74). The screen keeps:
//! - +0xa8 the state of a menu swap in progress (0 none), +0x94 the widget being stepped through while it runs;
//! - +0x54 the menu's cursor (a group of five pieces), +0x98 and +0x9c the jukebox's cursor and its marker;
//! - +0x58 and +0x5c two caption entries, +0x60 the description entry (its text at +0x110, a second at +0x114),
//!   +0x64 the description's heading;
//! - +0x78 and +0x7c two pictures (the characters' and the menu's; +0xac and +0xb0 their next sprites), +0x80..+0x8c
//!   four decorations;
//! - +0xa0 and +0xa4 the song's title and artist (entries), +0xb4 the song playing.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, LIST_END, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use crate::fe_charselect::{PRINT, SPRITE_RECT};
use crate::fe_options::LIST_AT;
use crate::fe_riderprofile::{SET_FILM, TEXTURES_IN, VIDEO_VTABLE};
use crate::fe_title::{ADD_WIDGET, BASE_CTOR, BUILD_BASE, HELP_BUTTON, HELP_BUTTONS, HELP_LAYOUT, TEXT_CTOR};
use crate::fe_username::{ICON_COLOUR, ICON_FRAMES};
use crate::fe_widgets::{group_init, icon_init, text_entry_init, vcall_class, GROUP_ADD, ICON_VTABLE};
use crate::pf::Mem;

pub const VTABLE: u32 = 0x3b3270;
/// The calls: a list entry's number in its list (0x2abec0); the sprite set's textures loaded (set, which, 1); the
/// menu's sub-menus built (the characters' list 0x296870 (screen, item), the jukebox, the characters' menu, the demos'
/// menu); the base's destructor
pub const LIST_INDEX: u32 = 0x2abec0;
pub const LOAD_SET: u32 = 0x1f9ca0;
pub const BUILD_CHARACTER_LIST: u32 = 0x296870;
pub const BUILD_JUKEBOX: u32 = 0x297178;
pub const BUILD_CHARACTER_MENU: u32 = 0x297fa0;
pub const BUILD_DEMO_MENU: u32 = 0x298368;
pub const BASE_DTOR: u32 = 0x2717a0;
/// The music's cues (each a 3D scene's +0x10 with its code: 2, 0x3e, 0x41, 0x42)
pub const CUE_START: u32 = 0x262508;
pub const CUE_CHARACTERS: u32 = 0x262548;
pub const CUE_JUKEBOX: u32 = 0x262588;
pub const CUE_PROMOS: u32 = 0x2625c8;
/// This screen's helpers, ported below: the song shown (0x299338), the captions' texts set (0x2994c8 by id, 0x2995a8 by
/// string) and the captions shown (0x299688)
pub const SHOW_SONG: u32 = 0x299338;
pub const CAPTIONS_ID: u32 = 0x2994c8;
pub const CAPTIONS_STRING: u32 = 0x2995a8;
pub const CAPTIONS_SHOW: u32 = 0x299688;
/// The films (the menu's items 0..3, 6, 5; the characters' format by name; the promos'), the video screen's tag
pub const FILMS: [u32; 9] = [0x3b2138, 0x3b2150, 0x3b2168, 0x3b2180, 0, 0x3b21b0, 0x3b2198, 0, 0];
pub const CHARACTER_FILM: u32 = 0x3b21c8;
pub const PROMO_FILMS: [u32; 2] = [0x3b21e0, 0x3b21f8];
/// The tables: the characters' descriptions (a word for each of 12, 0 none), the demos' strings, the songs (12 bytes
/// each: the title, the artist), the menu's text ids, the decorations' sprites
pub const CHARACTER_TEXTS: u32 = 0x348068;
pub const DEMO_TEXTS: u32 = 0x348098;
pub const SONGS: u32 = 0x3480c8;
pub const MENU_TEXTS: u32 = 0x3b2420;
pub const DECOR_SPRITES: u32 = 0x3b2400;

const ONE: u32 = 0x3f80_0000;

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

/// The FPU's divide: by zero it gives the largest float with the operands' signs combined, not an infinity or NaN
fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    a / b
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

/// The 3D scene (the front end's +0xa4; its vtable at +0)
fn scene_vcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let scene = m.r32(m.r32(m.r32(GAPP).wrapping_add(0x734)).wrapping_add(0xa4));
    let vt = m.r32(scene);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![scene.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn fe_cue(m: &mut Mem, e: &mut dyn FeEnv, cue: u32) {
    let fe = m.r32(m.r32(GAPP).wrapping_add(0x734));
    e.call(m, cue, &[fe.wrapping_add(0x70)]);
}

fn sprite_set(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x728))
}

/// A widget's mover (+0x70) taking it to `to` from where it is (x, y, z; w 1)
fn move_to(m: &mut Mem, e: &mut dyn FeEnv, w: u32, to: [f32; 3], buf: u32) {
    let d = [to[0] - fl(m.r32(w + 0x80)), to[1] - fl(m.r32(w + 0x84)), to[2] - fl(m.r32(w + 0x88))];
    put4(m, buf, [d[0].to_bits(), d[1].to_bits(), d[2].to_bits(), ONE]);
    vcall(m, e, w, 0x70, &[buf]);
}

/// A cursor taken onto a line: the line's position, its y down by half its height (its +0x130), its x less `back`
fn cursor_to_line(m: &mut Mem, e: &mut dyn FeEnv, cur: u32, line: u32, back: f32, buf: u32) {
    let p = [fl(m.r32(line + 0x80)), fl(m.r32(line + 0x84)), fl(m.r32(line + 0x88))];
    vcall(m, e, line, 0x130, &[buf + 0x10]);
    let y = p[1] + fl(m.r32(buf + 0x14)) * 0.5;
    move_to(m, e, cur, [p[0] - back, y, p[2]], buf + 0x20);
}

/// 0x299338 (`FEJukebox_ShowSong`; the screen, the song's line; `buf` 0x30 bytes): the 3D scene plays the line's song
/// (its +0xe8), +0xb4 the song, the jukebox's cursor moved onto the line (5 back), the title and artist entries given
/// the song's strings and laid out
pub fn show_song(m: &mut Mem, e: &mut dyn FeEnv, s: u32, line: u32, buf: u32) {
    let k = m.r32(line);
    scene_vcall(m, e, 0xe8, &[k]);
    m.w32(s + 0xb4, m.r32(line));
    let cur = m.r32(s + 0x9c);
    cursor_to_line(m, e, cur, line, 5.0, buf);
    let k = m.r32(line);
    let title = m.r32(SONGS.wrapping_add(k.wrapping_mul(12)));
    let t = m.r32(m.r32(s + 0xa0) + 0x114);
    vcall(m, e, t, 0x158, &[title]);
    let k = m.r32(line);
    let artist = m.r32(SONGS.wrapping_add(k.wrapping_mul(12)).wrapping_add(4));
    let t = m.r32(m.r32(s + 0xa4) + 0x114);
    vcall(m, e, t, 0x158, &[artist]);
    let t = m.r32(s + 0xa0);
    vcall(m, e, t, 0x140, &[]);
    let t = m.r32(s + 0xa4);
    vcall(m, e, t, 0x140, &[]);
}

/// The captions (+0x58, +0x5c) given a text by its +0x150 (an id) or +0x158 (a string) unless the first is moving
/// (+0x10): each text set, the caption shown (+0x20(1)), its +0x30 cleared and laid out (+0x140)
fn captions(m: &mut Mem, e: &mut dyn FeEnv, s: u32, slot: u32, v: u32) {
    if m.r32(m.r32(s + 0x58) + 0x10) != 0 {
        return;
    }
    for at in [0x58u32, 0x5c] {
        let t = m.r32(m.r32(s + at) + 0x110);
        vcall(m, e, t, slot, &[v]);
        let c = m.r32(s + at);
        vcall(m, e, c, 0x20, &[1]);
        let c = m.r32(s + at);
        m.w32(c + 0x30, 0);
        vcall(m, e, c, 0x140, &[]);
    }
}

/// 0x2994c8: the captions' text id
pub fn captions_id(m: &mut Mem, e: &mut dyn FeEnv, s: u32, id: u32) {
    captions(m, e, s, 0x150, id);
}

/// 0x2995a8: the captions' string
pub fn captions_string(m: &mut Mem, e: &mut dyn FeEnv, s: u32, text: u32) {
    captions(m, e, s, 0x158, text);
}

/// 0x299688: both captions shown (+0x20(1), +0x30 1)
pub fn captions_show(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let c = m.r32(s + 0x58);
    vcall(m, e, c, 0x20, &[1]);
    let c2 = m.r32(s + 0x5c);
    m.w32(m.r32(s + 0x58) + 0x30, 1);
    vcall(m, e, c2, 0x20, &[1]);
    let c2 = m.r32(s + 0x5c);
    m.w32(c2 + 0x30, 1);
}

/// `cFE3DStateDVDContent::CreateNextState` 0x293c20 (`buf` 0x100 bytes for a film's name): a video screen (0x160
/// bytes: the base constructor, +0x154 0, +0x158 1, +0x15c 0) playing the chosen film: from the menu its item's film
/// (items 4, 7, 8 none); from the characters' sub-menu the character's ("data/video/cv_" and the name at the chosen
/// character's record +8); from the promos' its line's (0, 1). Any other list or line: the screen without a film.
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    let item = m.r32(m.r32(s + 0x30) + 0x140);
    let at = alloc(m, e, 0x3b2130, 0x160);
    e.call(m, BASE_CTOR, &[at]);
    m.w32(at.wrapping_add(0x154), 0);
    m.w32(at.wrapping_add(0xc), VIDEO_VTABLE);
    m.w32(at.wrapping_add(0x158), 1);
    m.w32(at.wrapping_add(0x15c), 0);
    let list = m.r32(s + 0x30);
    if list == m.r32(s + 0x3c) {
        let k = m.r32(item);
        if let Some(&f) = FILMS.get(k as usize) {
            if f != 0 {
                e.call(m, SET_FILM, &[at, f]);
            }
        }
        return at;
    }
    if list == m.r32(s + 0x6c) {
        let c = m.r32(m.r32(m.r32(s + 0x68) + 0x140));
        let name = m.r32(c.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(8));
        e.call(m, PRINT, &[buf, CHARACTER_FILM, name]);
        e.call(m, SET_FILM, &[at, buf]);
        return at;
    }
    if list != m.r32(s + 0x74) {
        return at;
    }
    let k = m.r32(item);
    if k < 2 {
        e.call(m, SET_FILM, &[at, PROMO_FILMS[k as usize]]);
    }
    at
}

/// An icon made in place under `parent`, sized 10 × 10 through the icon class (`v` its vector)
fn icon(m: &mut Mem, e: &mut dyn FeEnv, s: u32, tag: u32, parent: u32, v: u32) -> u32 {
    let ic = alloc(m, e, tag, 0x120);
    icon_init(m, e, ic, s, parent);
    put4(m, v, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, ic, 0x138, &[v]);
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

/// A caption entry (0x150 bytes) with its text (0x130 bytes, in `font`), any text it had destroyed (+8(3))
fn entry_with_text(m: &mut Mem, e: &mut dyn FeEnv, s: u32, tag: u32, font: u32) -> u32 {
    let en = alloc(m, e, tag, 0x150);
    text_entry_init(m, e, en, s, 0);
    let t = alloc(m, e, 0x3b20b0, 0x130);
    let t = e.call(m, TEXT_CTOR, &[t, s, 0, font]);
    let old = m.r32(en.wrapping_add(0x110));
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(en.wrapping_add(0x110), t);
    en
}

/// `cFE3DStateDVDContent` vf11 0x293e10, the screen built (`buf` 0xb0 bytes of scratch):
/// - a backdrop icon (sprite 0x2f, 640 × 480, its colour cycle); the help bar's buttons (0xe), buttons 2, 4 and 8
///   white with +0x114 0, the help layout 10, the bar moved to y 425 (+0x70 2);
/// - the logo (sprite 0x30, 145 × 85 at (15, 10), +0x70 3);
/// - two captions (+0x58 in the small font, +0x5c in the large) reading 0xcfc, 1.0 high, sliding in from x 1250 to
///   their place at y 20 (the second 20 right of the first's end); the second faded (0.2);
/// - four decorations (+0x80.., sprites 0x31..0x34) growing from half size in turn, along the bottom at y 425;
/// - the base widgets; the menu's nine items (the texts at 0x3b2420; 0.9 high, red highlight, sliding in from x 740)
///   and the menu at (-640, 132) with +0x15c the float at 0x348030;
/// - the two pictures (+0x7c 262 × 224 at (355, 124), +0x78 297 × 283 at (349, 133); sprite 0x76, hidden);
/// - the menu's cursor (+0x54): five pieces of sprite 0x35 side by side, each starting its cycle later;
/// - the jukebox's cursor (+0x98, sprite 0x35) and marker (+0x9c, sprite 0x37);
/// - the description (+0x60, an entry with two texts, at (-620, 440) sliding in from x 640); the sub-menus' builders;
/// - the heading (+0x64, string 0x3b24f0 at (90, 94), 0.9 high, hidden);
/// - the swap's state 1 from the menu's first item, the menu laid out, the widgets reset off, the menu's cursor
///   hidden, +0x20 2.
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (b0, b10, b20, b30) = (buf, buf + 0x10, buf + 0x20, buf + 0x30);
    let half = 0.5f32.to_bits();
    m.w32(s + 0x18, 0);
    let ic = icon(m, e, s, 0x3b23b8, 0, b10);
    size(m, e, ic, b0, 640.0, 480.0);
    place(m, e, ic, b0, 0.0, 0.0);
    m.w32(ic + 0x110, 0x2f);
    colour(m, e, ic, b0, [ONE; 4]);
    put4(m, b0, [0, ONE, ONE, ONE]);
    e.call(m, ICON_COLOUR, &[ic, 5, b0]);
    put4(m, b10, [ONE; 4]);
    put4(m, b0, [ONE; 4]);
    let t = m.r32(0x3ae51c);
    e.call(m, ICON_FRAMES, &[ic, 5, b0, t, 1]);
    e.call(m, ADD_WIDGET, &[s, ic]);
    e.call(m, HELP_BUTTONS, &[s, 0xe, 0]);
    for (k, b) in [2u32, 4, 8].iter().enumerate() {
        if k == 0 {
            m.w32(m.r32(s + 0x48) + 0x70, 2);
        }
        let h = e.call(m, HELP_BUTTON, &[s, *b]);
        let icn = m.r32(h + 0x110);
        colour(m, e, icn, b0, [ONE; 4]);
        m.w32(m.r32(h + 0x110) + 0x114, 0);
    }
    e.call(m, HELP_LAYOUT, &[s, 0xa]);
    let bar = m.r32(s + 0x48);
    let x = fl(m.r32(bar + 0x80));
    move_to(m, e, bar, [x, 425.0, 0.0], b20);
    let bar = m.r32(s + 0x48);
    e.call(m, ADD_WIDGET, &[s, bar]);

    // the logo
    let ic = icon(m, e, s, 0x3b23c8, 0, b10);
    m.w32(ic + 0x70, 3);
    m.w32(ic + 0x110, 0x30);
    colour(m, e, ic, b0, [ONE; 4]);
    size(m, e, ic, b0, 145.0, 85.0);
    place(m, e, ic, b0, 15.0, 10.0);
    e.call(m, ADD_WIDGET, &[s, ic]);

    // the captions
    let small = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let c1 = entry_with_text(m, e, s, 0x3b23d8, small);
    m.w32(s + 0x58, c1);
    let large = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let c2 = entry_with_text(m, e, s, 0x3b23e8, large);
    m.w32(s + 0x5c, c2);
    let high = m.r32(0x3ae524);
    let t = m.r32(m.r32(s + 0x58) + 0x110);
    vcall(m, e, t, 0x150, &[0xcfc]);
    let c1 = m.r32(s + 0x58);
    m.w32(m.r32(c1 + 0x110) + 0x118, high);
    colour(m, e, c1, b0, [ONE; 4]);
    let c1 = m.r32(s + 0x58);
    m.w32(m.r32(c1 + 0x110) + 0x114, 0);
    m.w32(c1 + 0x11c, 5);
    let t = m.r32(m.r32(s + 0x5c) + 0x110);
    vcall(m, e, t, 0x150, &[0xcfc]);
    let c2 = m.r32(s + 0x5c);
    m.w32(m.r32(c2 + 0x110) + 0x118, high);
    colour(m, e, c2, b0, [0.2f32.to_bits(), ONE, ONE, ONE]);
    let c2 = m.r32(s + 0x5c);
    m.w32(c2 + 0x11c, 5);
    m.w32(m.r32(c2 + 0x110) + 0x114, 0);
    put4(m, b0, [(-640.0f32).to_bits(), 0, 0, 0]);
    let c1 = m.r32(s + 0x58);
    e.call(m, ICON_FRAMES, &[c1, 2, b0, 0x23, 0]);
    let c2 = m.r32(s + 0x5c);
    e.call(m, ICON_FRAMES, &[c2, 2, b0, 0x23, 0]);
    let c1 = m.r32(s + 0x58);
    vcall(m, e, c1, 0x130, &[b10]);
    let x1 = 1250.0 - fl(m.r32(b10));
    let c1 = m.r32(s + 0x58);
    place(m, e, c1, b20, x1, 20.0);
    let c2 = m.r32(s + 0x5c);
    vcall(m, e, c2, 0x130, &[b10]);
    let x2 = (1250.0 - fl(m.r32(b10))) + 20.0;
    let c2 = m.r32(s + 0x5c);
    place(m, e, c2, b20, x2, 20.0);
    let c1 = m.r32(s + 0x58);
    e.call(m, ADD_WIDGET, &[s, c1]);
    let c2 = m.r32(s + 0x5c);
    e.call(m, ADD_WIDGET, &[s, c2]);

    // the four decorations
    for k in 0..4u32 {
        let sp = m.r32(DECOR_SPRITES + 4 * k);
        let ic = icon(m, e, s, 0x3b2410, 0, b20);
        m.w32(ic + 0x114, 0x60);
        m.w32(ic + 0x110, sp);
        m.w32(ic + 0x70, 2);
        let r = e.call(m, SPRITE_RECT, &[s, sp]);
        let w = fl(m.r32(r.wrapping_add(4)));
        let r = e.call(m, SPRITE_RECT, &[s, sp]);
        let h = fl(m.r32(r.wrapping_add(8)));
        put4(m, b10, [(w * 0.5).to_bits(), (h * 0.5).to_bits(), 0, 0]);
        put4(m, b20, [w.to_bits(), h.to_bits(), 0, 0]);
        let (from, to) = if k & 1 != 0 { (b10, b20) } else { (b20, b10) };
        e.call(m, ICON_COLOUR, &[ic, 3, from]);
        e.call(m, ICON_FRAMES, &[ic, 3, to, 0x78, 2]);
        vcall(m, e, ic, 0x138, &[from]);
        vcall(m, e, ic, 0x20, &[1]);
        let x = k as i32 as f32 * w * 0.7 + 80.0;
        let y = 425.0 - (k as i32 % 2) as f32 * 0.3 * h;
        put4(m, b30, [x.to_bits(), y.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[ic, b30]);
        m.w32(s + 0x80 + 4 * k, ic);
        e.call(m, ADD_WIDGET, &[s, ic]);
    }

    // the menu
    e.call(m, BUILD_BASE, &[s, 0]);
    let menu = m.r32(s + 0x3c);
    let spacing = m.r32(0x348030);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let pale = [ONE, half, half, half];
    for k in 0..9u32 {
        let at = alloc(m, e, 0x3b2448, 0x130);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x60));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        m.w32(w, k);
        let id = m.r32(MENU_TEXTS + 4 * k);
        vcall(m, e, w, 0x150, &[id]);
        m.w32(w + 0x118, 0.9f32.to_bits());
        m.w32(w + 0x114, 0);
        for (slot, c) in [(0x108u32, [ONE, 0, 0, 0]), (0x110, [ONE, 0, 0, 0]), (0x120, pale), (0x118, pale)] {
            put4(m, b0, c);
            vcall(m, e, w, slot, &[b0]);
        }
        put4(m, b0, [740.0f32.to_bits(), 0, 0, 0]);
        e.call(m, ICON_FRAMES, &[w, 2, b0, 0x28, 0]);
        vcall(m, e, w, 0x20, &[0]);
        vcall(m, e, w, 0x100, &[1]);
        vcall(m, e, menu, 0x148, &[w]);
    }
    let _ = font;
    m.w32(menu + 0x70, 2);
    m.w32(menu + 0x190, 1);
    m.w32(menu + 0x13c, 0);
    m.w32(menu + 0x154, 2);
    place(m, e, menu, b0, -640.0, 132.0);
    m.w8(menu + 0x112, 0xff);
    m.w8(menu + 0x114, 0xff);
    m.w8(menu + 0x115, 0xff);
    m.w32(menu + 0x15c, spacing);
    vcall(m, e, menu, 0x140, &[]);
    e.call(m, ADD_WIDGET, &[s, menu]);

    // the two pictures
    for (at, w, h, x, y) in [(0x7cu32, 262.0f32, 224.0f32, 355.0f32, 124.0f32), (0x78, 297.0, 283.0, 349.0, 133.0)] {
        let ic = icon(m, e, s, 0x3b2460, 0, b10);
        m.w32(ic + 0x110, 0x76);
        m.w32(ic + 0x70, 2);
        m.w32(s + at, ic);
        size(m, e, ic, b0, w, h);
        let p = m.r32(s + at);
        place(m, e, p, b0, x, y);
        put4(m, b0, [0, ONE, ONE, ONE]);
        put4(m, b10, [ONE; 4]);
        let p = m.r32(s + at);
        e.call(m, ICON_COLOUR, &[p, 5, b0]);
        let p = m.r32(s + at);
        e.call(m, ICON_FRAMES, &[p, 5, b10, 0xf, 0]);
        let p = m.r32(s + at);
        vcall(m, e, p, 0x20, &[0]);
        let p = m.r32(s + at);
        vcall(m, e, p, 0x28, &[0]);
        let p = m.r32(s + at);
        e.call(m, ADD_WIDGET, &[s, p]);
    }

    // the menu's cursor: five pieces of sprite 0x35
    let g = alloc(m, e, 0x3b2470, 0x140);
    group_init(m, e, g, s, 0);
    m.w32(s + 0x54, g);
    let r = e.call(m, SPRITE_RECT, &[s, 0x35]);
    let w = fl(m.r32(r.wrapping_add(4)));
    let r = e.call(m, SPRITE_RECT, &[s, 0x35]);
    let h = fl(m.r32(r.wrapping_add(8)));
    let fade = [ONE, ONE, 0, 0];
    for k in 0..5u32 {
        let parent = m.r32(s + 0x54);
        let ic = icon(m, e, s, 0x3b2480, parent, b30);
        m.w32(ic + 0x110, 0x35);
        place(m, e, ic, b20, k as i32 as f32 * w * 0.8, 0.0);
        size(m, e, ic, b20, w, h);
        put4(m, b0, fade);
        e.call(m, ICON_COLOUR, &[ic, 5, b0]);
        put4(m, b10, [ONE; 4]);
        e.call(m, ICON_FRAMES, &[ic, 5, b10, 0x3c, 1]);
        vcall(m, e, ic, 0x40, &[]);
        let lag = (k as i32 as f32 / 5.0 * 60.0) as i32;
        m.w32(ic + 0x24, (0x3c - lag) as u32);
        vcall(m, e, ic, 0x20, &[1]);
        m.w32(ic + 0x114, 0x40);
        let g = m.r32(s + 0x54);
        e.call(m, GROUP_ADD, &[g, ic]);
    }
    let g = m.r32(s + 0x54);
    vcall(m, e, g, 0x20, &[1]);
    let g = m.r32(s + 0x54);
    m.w32(g + 0x70, 3);
    e.call(m, ADD_WIDGET, &[s, g]);

    // the jukebox's cursor and marker
    for (at, sp, tag, frames, mode, z, extra) in
        [(0x98u32, 0x35u32, 0x3b2490u32, 0x23u32, 2u32, 4u32, 0x48u32), (0x9c, 0x37, 0x3b24a0, 0, 0, 3, 0x49)]
    {
        let r = e.call(m, SPRITE_RECT, &[s, sp]);
        let w = fl(m.r32(r.wrapping_add(4)));
        let r = e.call(m, SPRITE_RECT, &[s, sp]);
        let h = fl(m.r32(r.wrapping_add(8)));
        let ic = icon(m, e, s, tag, 0, b30);
        m.w32(ic + 0x110, sp);
        m.w32(s + at, ic);
        size(m, e, ic, b20, w, h);
        let c = m.r32(s + at);
        if at == 0x98 {
            put4(m, b0, fade);
            e.call(m, ICON_COLOUR, &[c, 5, b0]);
            put4(m, b10, [ONE; 4]);
            let c = m.r32(s + at);
            e.call(m, ICON_FRAMES, &[c, 5, b10, frames, mode]);
            let c = m.r32(s + at);
            vcall(m, e, c, 0x40, &[]);
        } else {
            colour(m, e, c, b0, [ONE; 4]);
        }
        let c = m.r32(s + at);
        vcall(m, e, c, 0x20, &[1]);
        let c = m.r32(s + at);
        m.w32(c + 0x114, extra);
        m.w32(c + 0x70, z);
        e.call(m, ADD_WIDGET, &[s, c]);
    }

    // the description: an entry with two texts in the large font
    let large = m.r32(m.r32(GAPP).wrapping_add(0x60));
    let d = entry_with_text(m, e, s, 0x3b24b0, large);
    let at = alloc(m, e, 0x3b24c8, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let t2 = e.call(m, TEXT_CTOR, &[at, s, d, font]);
    let tint = 0x3f08_8659;
    let small_h = 0.8f32.to_bits();
    vcall(m, e, t2, 0x158, &[0x3b24d8]);
    m.w32(t2 + 0x118, small_h);
    m.w32(t2 + 0x114, 0);
    vcall(m, e, d, 0xf8, &[1]);
    let t1 = m.r32(d.wrapping_add(0x110));
    vcall(m, e, t1, 0x158, &[0x3b24d8]);
    let t1 = m.r32(d.wrapping_add(0x110));
    m.w32(t1 + 0x118, small_h);
    m.w32(t1 + 0x114, 0);
    put4(m, b0, [ONE, ONE, tint, 0]);
    vcall(m, e, d, 0x110, &[b0]);
    put4(m, b0, [ONE, ONE, tint, 0]);
    vcall(m, e, d, 0x50, &[b0]);
    let old = m.r32(d.wrapping_add(0x114));
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(d.wrapping_add(0x114), t2);
    m.w32(d.wrapping_add(0x11c), 2);
    m.w32(d.wrapping_add(0x130), 2);
    place(m, e, d, b0, -620.0, 440.0);
    put4(m, b0, [640.0f32.to_bits(), 0, 0, 0]);
    e.call(m, ICON_FRAMES, &[d, 2, b0, 0x28, 0]);
    vcall(m, e, d, 0x20, &[0]);
    m.w32(s + 0x60, d);
    m.w32(d.wrapping_add(0x70), 3);
    e.call(m, ADD_WIDGET, &[s, d]);
    e.call(m, BUILD_JUKEBOX, &[s]);
    e.call(m, BUILD_CHARACTER_MENU, &[s]);
    e.call(m, BUILD_DEMO_MENU, &[s]);

    // the heading
    let at = alloc(m, e, 0x3b24e0, 0x130);
    let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
    let hd = e.call(m, TEXT_CTOR, &[at, s, 0, font]);
    m.w32(s + 0x64, hd);
    place(m, e, hd, b0, 90.0, 94.0);
    let hd = m.r32(s + 0x64);
    m.w32(hd + 0x70, 2);
    m.w32(hd + 0x118, 0.9f32.to_bits());
    vcall(m, e, hd, 0x158, &[0x3b24f0]);
    let hd = m.r32(s + 0x64);
    vcall(m, e, hd, 0x28, &[0]);
    let hd = m.r32(s + 0x64);
    e.call(m, ADD_WIDGET, &[s, hd]);

    // the swap from the menu's first item
    m.w32(s + 0xa8, 1);
    let menu = m.r32(s + 0x3c);
    let w = e.call(m, LIST_NTH, &[menu + 0x118, 0]);
    m.w32(s + 0x94, w);
    vcall(m, e, w, 0x20, &[1]);
    let menu = m.r32(s + 0x3c);
    m.w32(m.r32(s + 0x94) + 0x30, 0);
    vcall(m, e, menu, 0x140, &[]);
    e.call(m, WIDGETS_RESET, &[s, 0]);
    let g = m.r32(s + 0x54);
    vcall(m, e, g, 0x28, &[0]);
    m.w32(s + 0x20, 2);
}

/// The four decorations' +0x28 (shown or hidden)
fn decorations(m: &mut Mem, e: &mut dyn FeEnv, s: u32, on: u32) {
    for k in 0..4u32 {
        let d = m.r32(s + 0x80 + 4 * k);
        vcall(m, e, d, 0x28, &[on]);
    }
}

/// The swap's end shared by several states: the captions shown, the menu's cursor shown with +0x30 1
fn back_to_menu_tail(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, CAPTIONS_SHOW, &[s]);
    let g = m.r32(s + 0x60);
    vcall(m, e, g, 0x20, &[1]);
    let g = m.r32(s + 0x60);
    m.w32(g + 0x30, 1);
}

/// The description hidden (+0x20(1) then +0x30 0)
fn description_off(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let d = m.r32(s + 0x60);
    vcall(m, e, d, 0x20, &[1]);
    let d = m.r32(s + 0x60);
    m.w32(d + 0x30, 0);
}

/// A picture's swap: once its sprite set is in (its +0x14 clear: once the textures are in, +0x28(1) first), its next
/// sprite (`next`) set and shown, +0x30 cleared, +0x40; while it is fading out (+0x30 1) only once it has finished
/// (+0x24 = +0x28)
fn picture_swap(m: &mut Mem, e: &mut dyn FeEnv, s: u32, at: u32, next: u32) {
    let p = m.r32(s + at);
    if m.r32(p + 0x14) == 0 {
        if e.call(m, TEXTURES_IN, &[sprite_set(m)]) == 0 {
            return;
        }
        let p = m.r32(s + at);
        vcall(m, e, p, 0x28, &[1]);
        let p = m.r32(s + at);
        m.w32(p + 0x110, m.r32(s + next));
        vcall(m, e, p, 0x20, &[1]);
    } else {
        if m.r32(p + 0x30) != 1 || m.r32(p + 0x24) != m.r32(p + 0x28) {
            return;
        }
        m.w32(p + 0x110, m.r32(s + next));
        vcall(m, e, p, 0x20, &[1]);
    }
    let p = m.r32(s + at);
    m.w32(p + 0x30, 0);
    vcall(m, e, p, 0x40, &[]);
}

/// `cFE3DStateDVDContent::Update` 0x2986c0 (`buf` 0x40 bytes, and the base's):
/// - **a swap running** (+0xa8): its step widget (+0x94) waits until it is half through (+0x24 / +0x28 at 0.5) when
///   it moves (+0x10); at the step list's end the state's work, else the next widget in the list takes over (shown,
///   with the last one's +0x30; in states 8 and 9 the jukebox's second column's line at the same place too);
/// - **no swap:** on the jukebox, when the 3D scene's song has ended (+0xf8), the next song (+0xb4 + 1 mod 21) found
///   in the first column or else the second, the columns' lines set (+0x160), and shown; on the characters or the
///   menu, the picture's swap (+0x78 with +0xac, +0x7c with +0xb0);
/// - then the menu's cursor, the jukebox's cursor and its marker hidden, the cursor taken onto the active list's line
///   (the menu's cursor 80 back, unless the active list is the demos'; on the jukebox its cursor 5 back and the
///   marker shown); the base's `Update`.
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    if m.r32(s + 0xa8) != 0 {
        update_swap(m, e, s);
    } else {
        let list = m.r32(s + 0x30);
        if list == m.r32(s + 0x70) {
            if scene_vcall(m, e, 0xf8, &[]) != 0 {
                let k = (m.r32(s + 0xb4).wrapping_add(1) as i32 % 21) as u32;
                let jb = m.r32(s + 0x70);
                let c0 = e.call(m, LIST_NTH, &[jb + 0x118, 0]);
                let jb = m.r32(s + 0x70);
                let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
                let mut line = e.call(m, LIST_AT, &[c0 + 0x118, k]);
                if line != 0 {
                    let i = e.call(m, LIST_INDEX, &[line]);
                    vcall(m, e, c0, 0x160, &[i]);
                    let jb = m.r32(s + 0x70);
                    vcall(m, e, jb, 0x160, &[0]);
                } else {
                    line = e.call(m, LIST_AT, &[c1 + 0x118, k]);
                    let i = e.call(m, LIST_INDEX, &[line]);
                    vcall(m, e, c1, 0x160, &[i]);
                    let jb = m.r32(s + 0x70);
                    vcall(m, e, jb, 0x160, &[1]);
                }
                e.call(m, SHOW_SONG, &[s, line]);
            }
        } else if list == m.r32(s + 0x68) {
            picture_swap(m, e, s, 0x78, 0xac);
        } else if list == m.r32(s + 0x3c) {
            picture_swap(m, e, s, 0x7c, 0xb0);
        }
    }
    for at in [0x54u32, 0x98, 0x9c] {
        let c = m.r32(s + at);
        vcall(m, e, c, 0x28, &[0]);
    }
    let list = m.r32(s + 0x30);
    if list == m.r32(s + 0x70) {
        let line = m.r32(m.r32(m.r32(s + 0x70) + 0x140) + 0x140);
        if line != 0 {
            let c = m.r32(s + 0x98);
            cursor_to_line(m, e, c, line, 5.0, buf);
            let c = m.r32(s + 0x98);
            vcall(m, e, c, 0x28, &[1]);
        }
        let c = m.r32(s + 0x9c);
        vcall(m, e, c, 0x28, &[1]);
    } else if list != 0 && list != m.r32(s + 0x6c) {
        let line = m.r32(list + 0x140);
        if line != 0 {
            let c = m.r32(s + 0x54);
            cursor_to_line(m, e, c, line, 80.0, buf);
            let c = m.r32(s + 0x54);
            vcall(m, e, c, 0x28, &[1]);
        }
    }
    base::update(m, e, s, buf);
}

/// Opening a sub-menu from the menu's item (`state`, its list, its cue): the swap's state, its first item the step
fn open_sub(m: &mut Mem, e: &mut dyn FeEnv, s: u32, state: u32, list: u32) -> u32 {
    m.w32(s + 0xa8, state);
    let l = m.r32(s + list);
    let w = e.call(m, LIST_NTH, &[l + 0x118, 0]);
    m.w32(s + 0x94, w);
    w
}

fn update_swap(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let w = m.r32(s + 0x94);
    let t = ps2_div(m.r32(w + 0x24) as i32 as f32, m.r32(w + 0x28) as i32 as f32);
    if m.r32(w + 0x10) != 0 && !(0.5 <= t) {
        return;
    }
    if e.call(m, LIST_END, &[m.r32(w + 8)]) == 0 {
        let last = m.r32(s + 0x94);
        let next = m.r32(last + 8);
        m.w32(s + 0x94, next);
        vcall(m, e, next, 0x20, &[1]);
        m.w32(m.r32(s + 0x94) + 0x30, m.r32(last + 0x30));
        if m.r32(s + 0xa8).wrapping_sub(8) < 2 {
            let jb = m.r32(s + 0x70);
            let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
            let i = e.call(m, LIST_INDEX, &[m.r32(s + 0x94)]);
            let other = e.call(m, LIST_NTH, &[c1 + 0x118, i]);
            if other != 0 {
                vcall(m, e, other, 0x20, &[1]);
                m.w32(other + 0x30, m.r32(last + 0x30));
            }
        }
        return;
    }
    match m.r32(s + 0xa8) {
        // the menu in: once the textures are in, done and the widgets reset onto it; its captions, the decorations
        1 => {
            if e.call(m, TEXTURES_IN, &[sprite_set(m)]) != 0 {
                m.w32(s + 0xa8, 0);
                let menu = m.r32(s + 0x3c);
                e.call(m, WIDGETS_RESET, &[s, menu]);
            }
            e.call(m, CAPTIONS_ID, &[s, 0xcf3]);
            decorations(m, e, s, 1);
        }
        // the menu out: the chosen item's sub-menu opens (4 the characters, 7 the jukebox, 8 the promos), the
        // decorations hidden, the step shown
        2 => {
            let k = m.r32(m.r32(m.r32(s + 0x3c) + 0x140));
            match k {
                4 => {
                    open_sub(m, e, s, 3, 0x68);
                    e.call(m, LOAD_SET, &[sprite_set(m), 3, 1]);
                    fe_cue(m, e, CUE_CHARACTERS);
                }
                7 => {
                    m.w32(s + 0xa8, 8);
                    let jb = m.r32(s + 0x70);
                    let c0 = e.call(m, LIST_NTH, &[jb + 0x118, 0]);
                    let w = e.call(m, LIST_NTH, &[c0 + 0x118, 0]);
                    m.w32(s + 0x94, w);
                    fe_cue(m, e, CUE_JUKEBOX);
                    let jb = m.r32(s + 0x70);
                    let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
                    let w1 = e.call(m, LIST_NTH, &[c1 + 0x118, 0]);
                    vcall(m, e, w1, 0x20, &[1]);
                    let jb = m.r32(s + 0x70);
                    let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
                    let w1 = e.call(m, LIST_NTH, &[c1 + 0x118, 0]);
                    m.w32(w1 + 0x30, 0);
                }
                8 => {
                    open_sub(m, e, s, 0xa, 0x74);
                    fe_cue(m, e, CUE_PROMOS);
                }
                _ => {}
            }
            decorations(m, e, s, 0);
            let w = m.r32(s + 0x94);
            vcall(m, e, w, 0x20, &[1]);
            m.w32(m.r32(s + 0x94) + 0x30, 0);
        }
        // the characters in: once the textures are in, done, the widgets reset onto them, their captions
        3 => {
            if e.call(m, TEXTURES_IN, &[sprite_set(m)]) == 0 {
                return;
            }
            m.w32(s + 0xa8, 0);
            m.w32(s + 0x94, 0);
            let l = m.r32(s + 0x68);
            e.call(m, WIDGETS_RESET, &[s, l]);
            e.call(m, CAPTIONS_ID, &[s, 0xcf8]);
            description_off(m, e, s);
        }
        // the characters out to the demos: their first item the step, the heading shown, the captions
        4 => {
            let w = open_sub(m, e, s, 6, 0x6c);
            vcall(m, e, w, 0x20, &[1]);
            let hd = m.r32(s + 0x64);
            m.w32(m.r32(s + 0x94) + 0x30, 0);
            vcall(m, e, hd, 0x28, &[1]);
            back_to_menu_tail(m, e, s);
        }
        // back to the menu (from the characters, the jukebox or the promos): its first item the step, the captions
        // (and, unless leaving the promos, the description) shown, state 1, the song texts hidden
        5 | 9 | 11 => {
            let w = open_sub_keep(m, e, s, 0x3c);
            vcall(m, e, w, 0x20, &[1]);
            m.w32(m.r32(s + 0x94) + 0x30, 0);
            e.call(m, CAPTIONS_SHOW, &[s]);
            if m.r32(s + 0xa8) != 11 {
                let d = m.r32(s + 0x60);
                vcall(m, e, d, 0x20, &[1]);
                m.w32(m.r32(s + 0x60) + 0x30, 1);
            }
            let a = m.r32(s + 0xa0);
            m.w32(s + 0xa8, 1);
            vcall(m, e, a, 0x28, &[0]);
            let b = m.r32(s + 0xa4);
            vcall(m, e, b, 0x28, &[0]);
        }
        // the demos in: done, the widgets reset onto them, the captions the chosen character's demo string
        6 => {
            m.w32(s + 0xa8, 0);
            m.w32(s + 0x94, 0);
            let l = m.r32(s + 0x6c);
            e.call(m, WIDGETS_RESET, &[s, l]);
            let c = m.r32(m.r32(m.r32(s + 0x68) + 0x140));
            let text = m.r32(DEMO_TEXTS.wrapping_add(c.wrapping_mul(4)));
            e.call(m, CAPTIONS_STRING, &[s, text]);
            description_off(m, e, s);
        }
        // the demos back to the characters
        7 => {
            let w = open_sub(m, e, s, 3, 0x68);
            vcall(m, e, w, 0x20, &[1]);
            m.w32(m.r32(s + 0x94) + 0x30, 0);
            back_to_menu_tail(m, e, s);
        }
        // the jukebox in: once the textures are in, done, the widgets reset onto it, the song texts shown, the 3D
        // scene's +0xe0, the chosen song shown
        8 => {
            if e.call(m, TEXTURES_IN, &[sprite_set(m)]) != 0 {
                m.w32(s + 0xa8, 0);
                m.w32(s + 0x94, 0);
                let l = m.r32(s + 0x70);
                e.call(m, WIDGETS_RESET, &[s, l]);
                let a = m.r32(s + 0xa0);
                vcall(m, e, a, 0x28, &[1]);
                let b = m.r32(s + 0xa4);
                vcall(m, e, b, 0x28, &[1]);
                scene_vcall(m, e, 0xe0, &[]);
                let line = m.r32(m.r32(m.r32(s + 0x70) + 0x140) + 0x140);
                e.call(m, SHOW_SONG, &[s, line]);
            }
            e.call(m, CAPTIONS_ID, &[s, 0xcfc]);
            description_off(m, e, s);
        }
        // the promos in
        10 => {
            m.w32(s + 0xa8, 0);
            m.w32(s + 0x94, 0);
            let l = m.r32(s + 0x74);
            e.call(m, WIDGETS_RESET, &[s, l]);
            e.call(m, CAPTIONS_ID, &[s, 0xcfd]);
        }
        _ => {}
    }
}

/// `open_sub` for the menu without a state change (the states 5, 9 and 11 set state 1 afterwards)
fn open_sub_keep(m: &mut Mem, e: &mut dyn FeEnv, s: u32, list: u32) -> u32 {
    let l = m.r32(s + list);
    let w = e.call(m, LIST_NTH, &[l + 0x118, 0]);
    m.w32(s + 0x94, w);
    w
}

/// `cFE3DStateDVDContent::Activate` 0x299270 (the screen, the one before): unless coming back from a film (the one
/// before answers 0xc from its +0x70), the start cue and the sprite set's textures (2); the base's `Activate`
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    if vcall(m, e, prev, 0x70, &[]) != 0xc {
        fe_cue(m, e, CUE_START);
        e.call(m, LOAD_SET, &[sprite_set(m), 2, 1]);
    }
    base::activate(m, e, s);
}

/// `cFE3DStateDVDContent::Deactivate` 0x2992f0: the sprite set's textures asked for (0x1f9d28, its answer unused), the
/// base's `Deactivate`
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, TEXTURES_IN, &[sprite_set(m)]);
    base::deactivate(m, e, s);
}

/// `cFE3DStateDVDContent` vf22 0x299700 (the item that goes, the one that comes): the base's vf22; then by the active
/// list:
/// - **the characters:** the coming character's cue (the 3D scene's +0x10) and picture (+0xac: sprites 0x71..0x7a by
///   character, -5 for 1 and 6), the picture shown (moved on to its fade's end when fading), the description's id
///   0xdd0 and the character's text (0x348068);
/// - **the demos:** the description's texts the character's (0x348068) and its record's +4;
/// - **the menu:** the coming item's cue and picture (+0xb0: 0x68..0x70), the picture shown as the characters';
/// - **the jukebox:** the coming column's line kept level with the going one's (stepping back past lines the column
///   lacks) when the going item is a column (+0x90), and the description's id 0xdd4 and the song's artist;
///
/// a picture not on the active list is hidden (+0x28(0)).
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if m.r32(s + 0x30) == m.r32(s + 0x68) {
        m.w32(s + 0xac, (-5i32) as u32);
        let k = m.r32(to);
        let cue = [Some((0x47, 0x71)), None, Some((0x46, 0x72)), Some((0x43, 0x73)), Some((0x44, 0x74)),
            Some((0x4b, 0x75)), None, Some((0x4a, 0x76)), Some((0x4c, 0x77)), Some((0x4d, 0x78)), Some((0x4e, 0x79)),
            Some((0x49, 0x7a))];
        if let Some(Some((c, sp))) = cue.get(k as usize) {
            scene_vcall(m, e, 0x10, &[*c]);
            m.w32(s + 0xac, *sp);
        }
        let p = m.r32(s + 0x78);
        fade_to_end(m, p);
        vcall(m, e, p, 0x20, &[1]);
        let d = m.r32(s + 0x60);
        let t = m.r32(d + 0x110);
        vcall(m, e, t, 0x150, &[0xdd0]);
        let k = m.r32(to);
        let text = m.r32(CHARACTER_TEXTS.wrapping_add(k.wrapping_mul(4)));
        let t = m.r32(m.r32(s + 0x60) + 0x114);
        vcall(m, e, t, 0x158, &[text]);
        let d = m.r32(s + 0x60);
        vcall(m, e, d, 0x140, &[]);
    } else {
        let p = m.r32(s + 0x78);
        vcall(m, e, p, 0x28, &[0]);
    }
    if m.r32(s + 0x30) == m.r32(s + 0x6c) {
        let k = m.r32(to);
        let text = m.r32(CHARACTER_TEXTS.wrapping_add(k.wrapping_mul(4)));
        let t = m.r32(m.r32(s + 0x60) + 0x110);
        vcall(m, e, t, 0x158, &[text]);
        let k = m.r32(to);
        let name = m.r32(k.wrapping_mul(0x38).wrapping_add(0x331ea8).wrapping_add(4));
        let t = m.r32(m.r32(s + 0x60) + 0x114);
        vcall(m, e, t, 0x158, &[name]);
        let d = m.r32(s + 0x60);
        vcall(m, e, d, 0x140, &[]);
    }
    if m.r32(s + 0x30) == m.r32(s + 0x3c) {
        let k = m.r32(to);
        let cue = [(0x3a, 0x68), (0x3c, 0x6a), (0x3b, 0x69), (0x3d, 0x6b), (0x3e, 0x6c), (0x3f, 0x6d), (0x40, 0x6e),
            (0x41, 0x6f), (0x42, 0x70)];
        if let Some((c, sp)) = cue.get(k as usize) {
            scene_vcall(m, e, 0x10, &[*c]);
            m.w32(s + 0xb0, *sp);
        }
        let p = m.r32(s + 0x7c);
        vcall(m, e, p, 0x20, &[1]);
        let p = m.r32(s + 0x7c);
        fade_to_end(m, p);
    } else {
        let p = m.r32(s + 0x7c);
        vcall(m, e, p, 0x28, &[0]);
    }
    if m.r32(s + 0x30) != m.r32(s + 0x70) {
        return;
    }
    let mut line = to;
    if vcall(m, e, to, 0x90, &[]) != 0 {
        if from != 0 {
            let mut k = e.call(m, LIST_INDEX, &[m.r32(from + 0x140)]) as i32;
            while k >= 0 {
                if e.call(m, LIST_NTH, &[to + 0x118, k as u32]) != 0 {
                    break;
                }
                k -= 1;
            }
            vcall(m, e, to, 0x160, &[k as u32]);
        }
        line = m.r32(to + 0x140);
    }
    let d = m.r32(s + 0x60);
    let t = m.r32(d + 0x110);
    vcall(m, e, t, 0x150, &[0xdd4]);
    let k = m.r32(line);
    let artist = m.r32(SONGS.wrapping_add(k.wrapping_mul(12)).wrapping_add(4));
    let t = m.r32(m.r32(s + 0x60) + 0x114);
    vcall(m, e, t, 0x158, &[artist]);
    let d = m.r32(s + 0x60);
    vcall(m, e, d, 0x140, &[]);
}

/// A picture that is moving (+0x14) set fading (+0x30 1) and, when not already near its end, put two frames from it
fn fade_to_end(m: &mut Mem, p: u32) {
    if m.r32(p + 0x14) == 0 {
        return;
    }
    m.w32(p + 0x30, 1);
    let n = m.r32(p + 0x28) as i32;
    if !((m.r32(p + 0x24) as i32) < n - 1) {
        m.w32(p + 0x24, (n - 2) as u32);
    }
}

/// `cFE3DStateDVDContent` vf26 0x299e10 (an item picked): nothing during a swap. From the menu, items 4, 7 and 8 start
/// the menu's swap out (state 2: the first item the step, the widgets reset off, the captions and the menu picture
/// shown, the 3D scene's +0x50(1)); the others go to the base's vf26 (a film). From the characters, the demos open
/// (state 4: the characters' list built for the item (0x296870), the help layout 12, the description shown). From
/// the jukebox, the line's song shown; from the demos and the promos the base's vf26.
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    if m.r32(s + 0xa8) != 0 {
        return;
    }
    let list = m.r32(s + 0x30);
    if list == m.r32(s + 0x3c) {
        let k = m.r32(item);
        if k >= 9 {
            return;
        }
        if ![4u32, 7, 8].contains(&k) {
            base::vf26(m, e, s);
            return;
        }
        m.w32(s + 0xa8, 2);
        let w = e.call(m, LIST_NTH, &[list + 0x118, 0]);
        m.w32(s + 0x94, w);
        m.w32(w + 0x30, 1);
        vcall(m, e, w, 0x20, &[1]);
        e.call(m, WIDGETS_RESET, &[s, 0]);
        e.call(m, CAPTIONS_SHOW, &[s]);
        let p = m.r32(s + 0x7c);
        m.w32(p + 0x30, 1);
        vcall(m, e, p, 0x20, &[1]);
        scene_vcall(m, e, 0x50, &[1]);
    } else if list == m.r32(s + 0x68) {
        let k = m.r32(item);
        m.w32(s + 0xa8, 4);
        let w = e.call(m, LIST_NTH, &[list + 0x118, 0]);
        m.w32(s + 0x94, w);
        m.w32(w + 0x30, 1);
        vcall(m, e, w, 0x20, &[1]);
        e.call(m, WIDGETS_RESET, &[s, 0]);
        e.call(m, BUILD_CHARACTER_LIST, &[s, k]);
        e.call(m, HELP_LAYOUT, &[s, 0xc]);
        let d = m.r32(s + 0x78);
        m.w32(d + 0x30, 1);
        vcall(m, e, d, 0x20, &[1]);
    } else if list == m.r32(s + 0x70) {
        e.call(m, SHOW_SONG, &[s, item]);
    } else if list == m.r32(s + 0x6c) || list == m.r32(s + 0x74) {
        base::vf26(m, e, s);
    }
}

/// `cFE3DStateDVDContent` vf21 0x299ff0 (backing out): from the menu the base's vf21; from a sub-menu its swap back
/// (the characters state 5 with the menu picture shown and the textures (2); the demos state 7 with the heading
/// hidden and the help layout 10; the jukebox state 9 with the 3D scene's +0x100 and both columns' first lines
/// stepped; the promos state 11), its first item the step; then the widgets reset off
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let list = m.r32(s + 0x30);
    if list == m.r32(s + 0x3c) {
        base::vf21(m, e, s);
        return;
    }
    if list == m.r32(s + 0x68) {
        m.w32(s + 0xa8, 5);
        step_from(m, e, s, list);
        let p = m.r32(s + 0x78);
        m.w32(p + 0x30, 1);
        vcall(m, e, p, 0x20, &[1]);
        e.call(m, LOAD_SET, &[sprite_set(m), 2, 1]);
    } else if list == m.r32(s + 0x6c) {
        m.w32(s + 0xa8, 7);
        step_from(m, e, s, list);
        let hd = m.r32(s + 0x64);
        vcall(m, e, hd, 0x28, &[0]);
        e.call(m, HELP_LAYOUT, &[s, 0xa]);
    } else if list == m.r32(s + 0x70) {
        scene_vcall(m, e, 0x100, &[]);
        m.w32(s + 0xa8, 9);
        let jb = m.r32(s + 0x70);
        let c0 = e.call(m, LIST_NTH, &[jb + 0x118, 0]);
        let w = e.call(m, LIST_NTH, &[c0 + 0x118, 0]);
        m.w32(s + 0x94, w);
        m.w32(w + 0x30, 1);
        vcall(m, e, w, 0x20, &[1]);
        let jb = m.r32(s + 0x70);
        let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
        let w1 = e.call(m, LIST_NTH, &[c1 + 0x118, 0]);
        let jb = m.r32(s + 0x70);
        m.w32(w1 + 0x30, 1);
        let c1 = e.call(m, LIST_NTH, &[jb + 0x118, 1]);
        let w1 = e.call(m, LIST_NTH, &[c1 + 0x118, 0]);
        vcall(m, e, w1, 0x20, &[1]);
    } else if list == m.r32(s + 0x74) {
        m.w32(s + 0xa8, 0xb);
        step_from(m, e, s, list);
    }
    e.call(m, WIDGETS_RESET, &[s, 0]);
}

/// The swap's step from a list's first item (+0x30 1, shown)
fn step_from(m: &mut Mem, e: &mut dyn FeEnv, s: u32, list: u32) {
    let w = e.call(m, LIST_NTH, &[list + 0x118, 0]);
    m.w32(s + 0x94, w);
    m.w32(w + 0x30, 1);
    vcall(m, e, w, 0x20, &[1]);
}

/// `cFE3DStateDVDContent` vf13 0x29dc30: the screen's kind, 0xb
pub fn vf13() -> u32 {
    0xb
}

/// `cFE3DStateDVDContent::~cFE3DStateDVDContent` 0x29dbb0 (the screen, the delete flag): the vtable, the base's
/// destructor
pub fn destroy(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flag: u32) {
    m.w32(s + 0xc, VTABLE);
    e.call(m, BASE_DTOR, &[s, flag]);
}

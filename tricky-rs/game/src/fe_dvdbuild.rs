//! The DVD screen's two big lists (board row G9y8d) on the game's bytes: `FE_BuildDvdCharacterList` (a character's
//! credits as headings and lines) and `FE_BuildDvdJukebox`. The widgets are made in place as the game inlines them
//! ([`crate::fe_widgets`]); every other call goes out through [`FeEnv`]. Checked in the function runner
//! (`tests/fe_dvdbuild.rs`).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use crate::fe_widgets::{icon_init, vcall_class, BUFFERED_TEXT_VTABLE, ICON_VTABLE};
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: the menu's constructor (0x26d3b8), the single-line text's (0x2b1138), a localised string
/// (0x2c5160: the app's strings, the id), a widget's colour run (0x2ac3b0), the list's n-th (0x2ac048) and its end
/// (0x2abf10), a widget added to the state (0x2763f0)
pub const MENU_CTOR: u32 = 0x26d3b8;
pub const TEXT_CTOR: u32 = 0x2b1138;
pub const LOC_STRING: u32 = 0x2c5160;
pub const COLOUR_RUN: u32 = 0x2ac3b0;
pub const LIST_NTH: u32 = 0x2ac048;
pub const LIST_END: u32 = 0x2abf10;
pub const STATE_ADD: u32 = 0x2763f0;
/// The characters' titles (by item), the heading marker (a wide character at 0x337e48), the list's table
pub const CHARACTER_TITLES: u32 = 0x348038;
pub const HEADING_MARK: u32 = 0x337e48;
const MENU_LIST_VTABLE: u32 = 0x3b7fe0;
/// Each character's string range (none for items 1, 6 and past 11)
const CREDITS: [(u32, u32); 12] = [
    (0xd33, 0xd3f),
    (0, 0),
    (0xd40, 0xd49),
    (0xd4a, 0xd52),
    (0xd53, 0xd5c),
    (0xd5d, 0xd68),
    (0, 0),
    (0xd69, 0xd74),
    (0xd75, 0xd7f),
    (0xd80, 0xd89),
    (0xd8a, 0xd94),
    (0xd95, 0xda2),
];

const ONE: u32 = 0x3f80_0000;

fn fb(v: f32) -> u32 {
    v.to_bits()
}

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn put(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32) -> u32 {
    let a = e.call(m, ALLOC, &[name, size, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0xdead_c0de, size]);
    }
    a
}

/// `FE_BuildDvdCharacterList` 0x296870(s, item; `buf` 0x50 bytes): the character list (+0x6c, made the first time:
/// a list of 0x1b0 bytes, +0x70 2, else emptied through its +0x10) with its bytes +0x110..+0x116 -1, at the origin;
/// the title (+0x64) given the character's title; then each string of the character's range: a heading (it starts
/// with the marker: a buffered text in the second font, white, 0.6, 20 or 25 below the line before) or a line (red,
/// 0.7, 15 or 20 below), each moved to the running position (90, 130 on) through its +0x70 and added; a marker (an
/// icon, sprite -5, at (100, 400), the item as its id); the list laid out (+0x180 1, +0x154 6), put at -640 and added
/// to the state, each of its widgets sliding in from 640 (track 2, 15 frames) and hidden
pub fn character_list(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, buf: u32) {
    let l = m.r32(s + 0x6c);
    if l != 0 {
        vcall(m, e, l, 0x10, &[]);
    } else {
        let l = alloc(m, e, 0x3b24f8, 0x1b0);
        e.call(m, MENU_CTOR, &[l, s, 0, 0, 0, 1]);
        for (at, v) in [(0x70u32, 2u32), (0x190, 2), (0xc, MENU_LIST_VTABLE), (0x194, 0), (0x198, 0), (0x19c, 0), (0x1a0, 0)] {
            m.w32(l + at, v);
        }
        m.w32(s + 0x6c, l);
    }
    let list = m.r32(s + 0x6c);
    m.w8(list + 0x111, 0xff);
    for at in [0x110u32, 0x116, 0x112, 0x114, 0x115] {
        let l = m.r32(s + 0x6c);
        m.w8(l + at, 0xff);
    }
    put(m, buf, [0, 0, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[list, buf]);
    m.w8(list + 0x115, 0xff);
    m.w8(list + 0x114, 0xff);
    let title = m.r32(s + 0x64);
    let name = m.r32(CHARACTER_TITLES.wrapping_add(item.wrapping_mul(4)));
    vcall(m, e, title, 0x158, &[name]);
    let (mut k, end) = if item < 12 { CREDITS[item as usize] } else { (0, 0) };
    let pos = buf + 0x10;
    put(m, buf, [ONE, 0, 0, 0]);
    put(m, pos, [fb(90.0), fb(130.0), 0, 0]);
    // the line before: -1 none, 1 a heading, 0 a line
    let mut before: i32 = -1;
    let mut scale: F;
    let add_y = |m: &mut Mem, dy: f32| {
        let y = rf(m, pos + 4) + F(dy);
        m.w32(pos + 4, y.0.to_bits());
    };
    while !((end as i32) < (k as i32)) {
        let strings = m.r32(m.r32(GAPP) + 0x50);
        let str = e.call(m, LOC_STRING, &[strings, k]);
        let heading = m.r16(str) == m.r16(HEADING_MARK);
        let t;
        if heading {
            let a = alloc(m, e, 0x3b2508, 0x330);
            let font = m.r32(m.r32(GAPP) + 0x5c);
            e.call(m, TEXT_CTOR, &[a, s, list, font]);
            t = a;
            m.w16(t + 0x130, 0);
            m.w32(t + 0xc, BUFFERED_TEXT_VTABLE);
            vcall_class(m, e, BUFFERED_TEXT_VTABLE, t, 0x148, &[str.wrapping_add(2)]);
            match before {
                0 => add_y(m, 25.0),
                1 => add_y(m, 20.0),
                _ => {}
            }
            before = 1;
            scale = F(f32::from_bits(0x3f19_999a));
            put(m, buf, [ONE; 4]);
        } else {
            let a = alloc(m, e, 0x3b2508, 0x130);
            let font = m.r32(m.r32(GAPP) + 0x60);
            t = e.call(m, TEXT_CTOR, &[a, s, list, font]);
            vcall(m, e, t, 0x150, &[k]);
            match before {
                1 => add_y(m, 20.0),
                0 => add_y(m, 15.0),
                _ => {}
            }
            before = 0;
            scale = F(f32::from_bits(0x3f33_3333));
            put(m, buf, [ONE, 0, 0, 0]);
        }
        m.w32(t + 0x114, heading as u32);
        vcall(m, e, t, 0x110, &[buf]);
        k = k.wrapping_add(1);
        let p = [rf(m, pos), rf(m, pos + 4), rf(m, pos + 8), rf(m, pos + 12)];
        let q = [rf(m, t + 0x80), rf(m, t + 0x84), rf(m, t + 0x88), rf(m, t + 0x8c)];
        put(m, buf + 0x20, [(p[0] - q[0]).0.to_bits(), (p[1] - q[1]).0.to_bits(), (p[2] - q[2]).0.to_bits(), ONE]);
        vcall(m, e, t, 0x70, &[buf + 0x20]);
        m.w32(t + 0x118, scale.0.to_bits());
        vcall(m, e, t, 0xf8, &[1]);
        vcall(m, e, list, 0x148, &[t]);
    }
    // the marker
    let i = alloc(m, e, 0x3b2518, 0x120);
    icon_init(m, e, i, s, list);
    put(m, buf + 0x20, [fb(10.0), fb(10.0), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, i, 0x138, &[buf + 0x20]);
    m.w32(i + 0x110, 0xffff_fffb);
    put(m, buf + 0x10, [0; 4]);
    vcall(m, e, i, 0x108, &[buf + 0x10]);
    put(m, buf + 0x10, [0; 4]);
    vcall(m, e, i, 0x110, &[buf + 0x10]);
    put(m, buf + 0x10, [fb(10.0), fb(10.0), 0, 0]);
    vcall(m, e, i, 0x138, &[buf + 0x10]);
    put(m, buf + 0x10, [fb(100.0), fb(400.0), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[i, buf + 0x10]);
    m.w32(i, item);
    vcall(m, e, list, 0x148, &[i]);
    for (at, v) in [(0x180u32, 1u32), (0x154, 6), (0x190, 1), (0x150, 1), (0x13c, 0)] {
        m.w32(list + at, v);
    }
    vcall(m, e, list, 0x140, &[]);
    put(m, buf + 0x10, [fb(-640.0), 0, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[list, buf + 0x10]);
    e.call(m, STATE_ADD, &[s, list]);
    let l = m.r32(s + 0x6c);
    let mut n = e.call(m, LIST_NTH, &[l + 0x118, 0]);
    put(m, buf + 0x10, [fb(640.0), 0, 0, 0]);
    while n != 0 && e.call(m, LIST_END, &[n]) == 0 {
        e.call(m, COLOUR_RUN, &[n, 2, buf + 0x10, 0xf, 0]);
        vcall(m, e, n, 0x20, &[0]);
        n = m.r32(n + 8);
    }
}

/// The jukebox's tables: the songs' strings (21, 12 bytes apart), the lines' scale, the gap, the entries' text
pub const SONGS: u32 = 0x3480c8;
pub const SONG_SCALE: u32 = 0x3481c4;
pub const LIST_GAP: u32 = 0x348030;
pub const NOW_PLAYING: u32 = 0x3b28f0;

/// The entry's child at `at` replaced: the old one destroyed (its +8 with 3)
fn set_child(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, c: u32) {
    let old = m.r32(w + at);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(w + at, c);
}

/// A menu list (0x1b0 bytes) made for the screen: the menu's constructor with (0, 0, 0, 1), +0x190 2, cleared
/// +0x194..+0x1a0, the list's table
fn menu_list(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32) -> u32 {
    let l = alloc(m, e, name, 0x1b0);
    e.call(m, MENU_CTOR, &[l, s, 0, 0, 0, 1]);
    m.w32(l + 0x190, 2);
    m.w32(l + 0xc, MENU_LIST_VTABLE);
    for at in [0x194u32, 0x198, 0x19c, 0x1a0] {
        m.w32(l + at, 0);
    }
    l
}

/// One of the two now-playing entries (0x150 bytes, a text entry with no parent) with its text
fn jukebox_entry(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32) -> u32 {
    let w = alloc(m, e, name, 0x150);
    let font = m.r32(m.r32(GAPP) + 0x60);
    crate::fe_widgets::text_entry_init(m, e, w, s, 0);
    let t = alloc(m, e, 0x3b20b0, 0x130);
    let t = e.call(m, TEXT_CTOR, &[t, s, 0, font]);
    set_child(m, e, w, 0x110, t);
    w
}

/// `FE_BuildDvdJukebox` 0x297178(s; `buf` 0x50 bytes): the jukebox (+0x70): a menu holding two lists of the 21 songs
/// (the first 11 sliding in from 640, the rest from -640; red with grey outlines, ids 0..20) placed at -600 and 1020,
/// the menu at (0, 160); the two now-playing entries (+0xa0, +0xa4: strings 0xdd1 / 0xdd2, red, 0.9, each with a
/// second text 0x3b28f0) at (250, 115) and (250, 145), laid out, hidden and added
pub fn jukebox(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let half = 0x3f00_0000;
    put(m, buf, [ONE, 0, 0, 0]);
    put(m, buf + 0x10, [ONE, 0, 0, 0]);
    put(m, buf + 0x20, [ONE, half, half, half]);
    put(m, buf + 0x30, [ONE, half, half, half]);
    let gap = m.r32(LIST_GAP);
    let menu = menu_list(m, e, s, 0x3b2860);
    let a = menu_list(m, e, s, 0x3b2870);
    let b = menu_list(m, e, s, 0x3b2888);
    m.w32(a + 0x15c, gap);
    m.w32(b + 0x15c, gap);
    vcall(m, e, menu, 0x148, &[a]);
    vcall(m, e, menu, 0x148, &[b]);
    for i in 0..21u32 {
        let (l, x) = if i < 11 { (a, 640.0f32) } else { (b, -640.0) };
        let t = alloc(m, e, 0x3b28a0, 0x130);
        let font = m.r32(m.r32(GAPP) + 0x60);
        let t = e.call(m, TEXT_CTOR, &[t, s, l, font]);
        put(m, buf + 0x40, [fb(x), 0, 0, 0]);
        e.call(m, COLOUR_RUN, &[t, 2, buf + 0x40, 0x1e, 0]);
        vcall(m, e, t, 0x20, &[0]);
        vcall(m, e, t, 0x100, &[1]);
        vcall(m, e, l, 0x148, &[t]);
        m.w32(t, i);
        let song = m.r32(SONGS + 12 * i);
        vcall(m, e, t, 0x158, &[song]);
        vcall(m, e, t, 0x108, &[buf]);
        vcall(m, e, t, 0x110, &[buf + 0x10]);
        vcall(m, e, t, 0x118, &[buf + 0x30]);
        vcall(m, e, t, 0x120, &[buf + 0x20]);
        vcall(m, e, t, 0x50, &[buf + 0x10]);
        let sc = m.r32(SONG_SCALE);
        m.w32(t + 0x114, 0);
        m.w32(t + 0x118, sc);
    }
    for (l, x) in [(a, -600.0f32), (b, 1020.0)] {
        put(m, buf + 0x40, [fb(x), 0, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[l, buf + 0x40]);
    }
    for l in [a, b] {
        for (at, v) in [(0x154u32, 6u32), (0x190, 1), (0x150, 1), (0x13c, 0), (0x158, 1)] {
            m.w32(l + at, v);
        }
    }
    for (at, v) in [(0x154u32, 6u32), (0x190, 1), (0x13c, 0), (0x150, 0), (0x158, 1), (0x70, 2)] {
        m.w32(menu + at, v);
    }
    put(m, buf + 0x40, [0, fb(160.0), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[menu, buf + 0x40]);
    m.w32(menu + 0x180, 1);
    for (l, at) in [(a, 0x115u32), (a, 0x114), (b, 0x115), (b, 0x114), (menu, 0x114), (menu, 0x115)] {
        m.w8(l + at, 0xff);
    }
    vcall(m, e, menu, 0x140, &[]);
    m.w32(s + 0x70, menu);
    e.call(m, STATE_ADD, &[s, menu]);
    // the now-playing entries
    let e1 = jukebox_entry(m, e, s, 0x3b28b0);
    m.w32(s + 0xa0, e1);
    let e2 = jukebox_entry(m, e, s, 0x3b28c8);
    m.w32(s + 0xa4, e2);
    for (at, sid) in [(0xa0u32, 0xdd1u32), (0xa4, 0xdd2)] {
        let w = m.r32(s + at);
        let t = m.r32(w + 0x110);
        vcall(m, e, t, 0x150, &[sid]);
    }
    let (w1, w2) = (m.r32(s + 0xa0), m.r32(s + 0xa4));
    m.w32(w1 + 0x11c, 9);
    m.w32(w2 + 0x11c, 9);
    m.w32(w1 + 0x130, 10);
    m.w32(w2 + 0x130, 10);
    for at in [0xa0u32, 0xa4] {
        let w = m.r32(s + at);
        put(m, buf + 0x40, [ONE, 0, 0, 0]);
        vcall(m, e, w, 0x50, &[buf + 0x40]);
    }
    let nine = 0x3f66_6666;
    for at in [0xa0u32, 0xa4] {
        let t = m.r32(m.r32(s + at) + 0x110);
        m.w32(t + 0x118, nine);
        m.w32(t + 0x114, 0);
    }
    for (at, name) in [(0xa0u32, 0x3b28e0u32), (0xa4, 0x3b28f8)] {
        let t = alloc(m, e, name, 0x130);
        let parent = m.r32(s + at);
        let font = m.r32(m.r32(GAPP) + 0x60);
        let t = e.call(m, TEXT_CTOR, &[t, s, parent, font]);
        m.w32(t + 0x114, 0);
        put(m, buf + 0x40, [ONE, 0, 0, 0]);
        vcall(m, e, t, 0x50, &[buf + 0x40]);
        vcall(m, e, t, 0x158, &[NOW_PLAYING]);
        let w = m.r32(s + at);
        m.w32(t + 0x118, nine);
        set_child(m, e, w, 0x114, t);
    }
    let y = F(115.0);
    for (at, dy) in [(0xa0u32, None), (0xa4, Some(30.0f32))] {
        let w = m.r32(s + at);
        let yy = match dy {
            Some(d) => y + F(d),
            None => y,
        };
        put(m, buf + 0x40, [fb(250.0), yy.0.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[w, buf + 0x40]);
    }
    for at in [0xa0u32, 0xa4] {
        let w = m.r32(s + at);
        vcall(m, e, w, 0x140, &[]);
    }
    for at in [0xa0u32, 0xa4] {
        let w = m.r32(s + at);
        vcall(m, e, w, 0x28, &[0]);
    }
    for at in [0xa0u32, 0xa4] {
        let w = m.r32(s + at);
        e.call(m, STATE_ADD, &[s, w]);
    }
}

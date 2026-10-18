//! The front end's world group and its helpers (board row G9y5) on the game's bytes: `FE3D_BuildWorldGroup`, which
//! builds the two groups every screen shares (the top bar with the screen's title and its frame pieces, and the bottom
//! bar with its pieces, the help bar's buttons and the scrolling text), `FE_CreateList` (a screen's menu list) and
//! `FE3DWorld_FindByHash`. The widgets are made in place as the game inlines them ([`crate::fe_widgets`]); every
//! other call goes out through [`FeEnv`]. Checked in the function runner (`tests/fe_world.rs`).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use crate::fe_charselect::{SET_STRING, SPRITE_RECT};
use crate::fe_title::{ADD_WIDGET, HELP_BUTTONS, TEXT_CTOR};
use crate::fe_trickbook::{TEXT_BOX, TEXT_BOX_SCALE};
use crate::fe_widgets::{group_init, icon_init, vcall_class, GROUP_ADD, ICON_VTABLE};
use crate::pf::Mem;
use crate::ps2f::F;

/// The scaled icon's vtable (an icon whose +0x120 and +0x124 scale it), the text's scale set (0x2b4cc0, f12), the
/// menu's constructor (0x26d3b8), the menu list's vtable, the find operation's vtable and destructor (0x270210), the
/// scrolling text's string (0x3ae5c8)
pub const SCALED_ICON_VTABLE: u32 = 0x3b6e40;
pub const TEXT_SET_SCALE: u32 = 0x2b4cc0;
pub const MENU_CTOR: u32 = 0x26d3b8;
pub const MENU_LIST_VTABLE: u32 = 0x3b7fe0;
pub const FIND_HASH_OP_VTABLE: u32 = 0x3ad778;
pub const FIND_HASH_OP_DTOR: u32 = 0x270210;
pub const SCROLL_TEXT: u32 = 0x3ae5c8;

const ONE: u32 = 0x3f80_0000;

fn b(v: F) -> u32 {
    v.0.to_bits()
}

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn put(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// `cvt.s.w`: toward zero
fn fl(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

/// An allocation (0x2ccf70 with the tag and size), filled with 0xdeadc0de when it came back
fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, 0]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// The widget placed at `v` (0x2acb20)
fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: [u32; 4], buf: u32) {
    put(m, buf, v);
    e.call(m, WIDGET_PLACE, &[w, buf]);
}

/// The widget's own virtual at `off` handed the vector `v`
fn vcall_v(m: &mut Mem, e: &mut dyn FeEnv, w: u32, off: u32, v: [u32; 4], buf: u32) {
    put(m, buf, v);
    vcall(m, e, w, off, &[buf]);
}

const WHITE: [u32; 4] = [ONE; 4];
/// (1, 0.8, 0.8, 0.8)
const GREY: [u32; 4] = [ONE, 0x3f4c_cccd, 0x3f4c_cccd, 0x3f4c_cccd];

/// An icon (made in place) in `group`: its size 10 × 10 (by the icon's class), its sprite; then by the kind: scaled
/// (the scaled icon's vtable, +0x120 and +0x124 1.0)
fn icon(m: &mut Mem, e: &mut dyn FeEnv, s: u32, group: u32, tag: u32, size: u32, sprite: u32, buf: u32) -> u32 {
    let w = alloc(m, e, tag, size);
    icon_init(m, e, w, s, group);
    put(m, buf, [10.0f32.to_bits(), 10.0f32.to_bits(), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, w, 0x138, &[buf]);
    m.w32(w.wrapping_add(0x110), sprite);
    w
}

fn scaled(m: &mut Mem, w: u32) {
    m.w32(w.wrapping_add(0xc), SCALED_ICON_VTABLE);
    m.w32(w.wrapping_add(0x120), ONE);
    m.w32(w.wrapping_add(0x124), ONE);
}

/// `FE3D_BuildWorldGroup` 0x2720f8(s, title, buttons; `buf` 0x30 bytes): the heights of the sprites 9, 5 and 13 (5
/// asked again, unused), then:
/// - the top group (0x140 bytes, +0x40): the title text (0x2b1138 with the menu font; white, +0x118 1.0, its string,
///   at (20, 20), +0x70 5) and its size (w, h); the title's two end pieces (sprites 0x21 and 0x20, 4 at +0x114, at
///   y h + 27 and x 0 / w + 20, sized (w + 20 / the sprite 0x20's width, h + 27), grey, +0x70 2); the frame across
///   (scaled sprites 0xd and 0xe at x 0 and 320, 320 by sprite 13's height) and the band under it (sprites 5 and 6 at
///   sprite 13's height, x 0 and 320, 320 by sprite 5's height), white;
/// - the bottom group (0x140 bytes, +0x44): the scaled sprites 0xb and 0xc (sized 320 by sprites 13's and 9's heights,
///   at sprite 5's height, x 0 / 320) and the sprites 7 and 8 (320 by sprite 5's height, at x 0 / 320), grey; the help
///   bar's buttons when asked for (0x2719c0, +0x48 then +0x70 3, in the group); the scrolling text (0x15a0 bytes,
///   0x2b0b90 with the menu font, +0x4c: the highlight colour, +0x198 and +0x114 1, at (20, 37), +0x70 3, sized
///   480 × 40, its string 0x3ae5c8, scaled 0.8, its scale the font's height) in the group;
/// - both groups +0x70 6 and added to the world (0x2763f0)
pub fn build_world_group(m: &mut Mem, e: &mut dyn FeEnv, s: u32, title: u32, buttons: u32, buf: u32) {
    let height = |m: &mut Mem, e: &mut dyn FeEnv, k: u32| {
        let r = e.call(m, SPRITE_RECT, &[s, k]);
        rf(m, r.wrapping_add(8))
    };
    let h9 = height(m, e, 9);
    let h5 = height(m, e, 5);
    let h13 = height(m, e, 0xd);
    e.call(m, SPRITE_RECT, &[s, 5]);
    let (size, v, sz) = (buf, buf + 0x10, buf + 0x20);
    let zero = F(0.0);
    let twenty = F(20.0);

    // the top group
    let top = alloc(m, e, 0x3ae578, 0x140);
    group_init(m, e, top, s, 0);
    let t = alloc(m, e, 0x3ae580, 0x130);
    let font = m.r32(m.r32(GAPP) + 0x60);
    let t = e.call(m, TEXT_CTOR, &[t, s, top, font]);
    vcall_v(m, e, t, 0x50, WHITE, v);
    m.w32(t + 0x118, ONE);
    vcall(m, e, t, 0x150, &[title]);
    place(m, e, t, [b(twenty), b(twenty), 0, ONE], v);
    m.w32(t + 0x70, 5);
    e.call(m, GROUP_ADD, &[top, t]);
    vcall(m, e, t, 0x130, &[size]);
    let y = rf(m, size + 4) + twenty + F(7.0);
    let h = y - zero;

    let w = icon(m, e, s, top, 0x3ae588, 0x120, 0x21, sz);
    m.w32(w + 0x114, 4);
    place(m, e, w, [0, b(y), 0, ONE], v);
    let x = rf(m, size) + twenty;
    vcall_v(m, e, w, 0x138, [b(x), b(h), 0, 0], v);
    vcall_v(m, e, w, 0x50, GREY, v);
    m.w32(w + 0x70, 2);
    e.call(m, GROUP_ADD, &[top, w]);

    let w = icon(m, e, s, top, 0x3ae598, 0x120, 0x20, sz);
    m.w32(w + 0x114, 4);
    let x = rf(m, size) + twenty;
    place(m, e, w, [b(x), b(y), 0, ONE], v);
    let r = e.call(m, SPRITE_RECT, &[s, 0x20]);
    let rw = m.r32(r.wrapping_add(4));
    vcall_v(m, e, w, 0x138, [rw, b(h), 0, 0], v);
    vcall_v(m, e, w, 0x50, GREY, v);
    m.w32(w + 0x70, 2);
    e.call(m, GROUP_ADD, &[top, w]);

    let half = F(320.0);
    for (sprite, at, tag, size_, scale, h_) in [
        (0xd, [0, 0], 0x3ae5a8, 0x130, true, h13),
        (0xe, [b(half), 0], 0x3ae5a8, 0x130, true, h13),
        (5, [0, b(h13)], 0x3ae5b0, 0x120, false, h5),
        (6, [b(half), b(h13)], 0x3ae5b0, 0x120, false, h5),
    ] {
        let w = icon(m, e, s, top, tag, size_, sprite, sz);
        if scale {
            scaled(m, w);
        }
        place(m, e, w, [at[0], at[1], 0, ONE], v);
        vcall_v(m, e, w, 0x138, [b(half), b(h_), 0, 0], v);
        vcall_v(m, e, w, 0x50, WHITE, v);
        e.call(m, GROUP_ADD, &[top, w]);
    }
    m.w32(s + 0x40, top);

    // the bottom group
    let bottom = alloc(m, e, 0x3ae578, 0x140);
    group_init(m, e, bottom, s, 0);
    let tall = h13 + h9;
    for (sprite, x) in [(0xb, 0), (0xc, b(half))] {
        let w = icon(m, e, s, bottom, 0x3ae5a8, 0x130, sprite, sz);
        scaled(m, w);
        put(m, v, [b(half), b(tall), 0, 0]);
        vcall_class(m, e, SCALED_ICON_VTABLE, w, 0x138, &[v]);
        place(m, e, w, [x, b(h5), 0, ONE], v);
        vcall_v(m, e, w, 0x50, GREY, v);
        e.call(m, GROUP_ADD, &[bottom, w]);
    }
    for (sprite, x) in [(7, 0), (8, b(half))] {
        let w = icon(m, e, s, bottom, 0x3ae5b0, 0x120, sprite, sz);
        place(m, e, w, [x, 0, 0, ONE], v);
        vcall_v(m, e, w, 0x138, [b(half), b(h5), 0, 0], v);
        vcall_v(m, e, w, 0x50, GREY, v);
        e.call(m, GROUP_ADD, &[bottom, w]);
    }
    if buttons != 0 {
        e.call(m, HELP_BUTTONS, &[s, buttons, bottom]);
        let bar = m.r32(s + 0x48);
        m.w32(bar + 0x70, 3);
        e.call(m, GROUP_ADD, &[bottom, bar]);
    }
    m.w32(s + 0x44, bottom);

    // the scrolling text
    let tb = alloc(m, e, 0x3ae5b8, 0x15a0);
    let parent = m.r32(s + 0x44);
    let font = m.r32(m.r32(GAPP) + 0x60);
    let tb = e.call(m, TEXT_BOX, &[tb, s, parent, font]);
    m.w32(s + 0x4c, tb);
    vcall_v(m, e, tb, 0x50, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687], v);
    let tb = m.r32(s + 0x4c);
    m.w32(tb + 0x198, 1);
    m.w32(tb + 0x114, 1);
    place(m, e, tb, [b(twenty), 37.0f32.to_bits(), 0, ONE], v);
    let tb = m.r32(s + 0x4c);
    m.w32(tb + 0x70, 3);
    vcall_v(m, e, tb, 0x138, [480.0f32.to_bits(), 40.0f32.to_bits(), 0, 0], v);
    let tb = m.r32(s + 0x4c);
    e.call(m, SET_STRING, &[tb, SCROLL_TEXT]);
    let tb = m.r32(s + 0x4c);
    e.call(m, TEXT_BOX_SCALE, &[tb, 0x3f4c_cccd]);
    let font = m.r32(m.r32(GAPP) + 0x60);
    let fh = fl(m.r32(font + 0xc));
    let tb = m.r32(s + 0x4c);
    e.call(m, TEXT_SET_SCALE, &[tb, b(fh)]);
    let (bottom, tb) = (m.r32(s + 0x44), m.r32(s + 0x4c));
    e.call(m, GROUP_ADD, &[bottom, tb]);
    let (top, bottom) = (m.r32(s + 0x40), m.r32(s + 0x44));
    m.w32(top + 0x70, 6);
    m.w32(bottom + 0x70, 6);
    e.call(m, ADD_WIDGET, &[s, top]);
    let bottom = m.r32(s + 0x44);
    e.call(m, ADD_WIDGET, &[s, bottom]);
}

/// `FE_CreateList` 0x274000(s, parent; `buf` 0x10 bytes): the screen's menu (+0x3c) made (0x1b0 bytes, the menu's
/// constructor with (0, 0, 1)), +0x190 2, +0x194..+0x1a0 cleared, the menu list's vtable and its +0x168 (by the
/// class); +0x144 1, +0x158 0, placed at (320, 63), +0x70 1
pub fn create_list(m: &mut Mem, e: &mut dyn FeEnv, s: u32, parent: u32, buf: u32) {
    let l = alloc(m, e, 0x3ae5d8, 0x1b0);
    e.call(m, MENU_CTOR, &[l, s, parent, 0, 0, 1]);
    m.w32(l.wrapping_add(0x190), 2);
    m.w32(s + 0x3c, l);
    m.w32(l.wrapping_add(0x194), 0);
    m.w32(l.wrapping_add(0xc), MENU_LIST_VTABLE);
    for at in [0x198u32, 0x19c, 0x1a0] {
        m.w32(l.wrapping_add(at), 0);
    }
    vcall_class(m, e, MENU_LIST_VTABLE, l, 0x168, &[]);
    let menu = m.r32(s + 0x3c);
    m.w32(menu + 0x144, 1);
    m.w32(menu + 0x158, 0);
    place(m, e, menu, [320.0f32.to_bits(), 63.0f32.to_bits(), 0, ONE], buf);
    let menu = m.r32(s + 0x3c);
    m.w32(menu + 0x70, 1);
}

/// `FE3DWorld_FindByHash` 0x26fe50(w, hash; `buf` 8 bytes): a find operation (its vtable, the hash) handed to the
/// widget's +0x10 (its table at +0x20), then the operation's destructor (0x270210 with 2); what the find answered
pub fn find_by_hash(m: &mut Mem, e: &mut dyn FeEnv, w: u32, hash: u32, buf: u32) -> u32 {
    m.w32(buf, FIND_HASH_OP_VTABLE);
    m.w32(buf + 4, hash);
    let vt = m.r32(w + 0x20);
    let adj = m.r16(vt.wrapping_add(0x10)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(0x14));
    let r = e.call(m, f, &[w.wrapping_add(adj), buf]);
    e.call(m, FIND_HASH_OP_DTOR, &[buf, 2]);
    r
}

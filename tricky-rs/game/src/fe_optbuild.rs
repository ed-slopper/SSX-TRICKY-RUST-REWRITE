//! The options screen's big builders (board rows G9y8e..g) on the game's bytes: `FE_BuildControllerConfig` (the
//! controller page), then the sound page and the screen itself. The widgets are made in place as the game inlines
//! them ([`crate::fe_widgets`]); every other call goes out through [`FeEnv`]. Checked in the function runner
//! (`tests/fe_optbuild.rs`).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use crate::fe_widgets::{group_init, icon_init, text_entry_init, vcall_class, ICON_VTABLE};
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: a sprite's rectangle (0x2759c8), a group's widget added (0x2762e0), the menu's constructor
/// (0x26d3b8), the single-line text's (0x2b1138), a group's widget by id (0x2ac0c0), the pages' layout (0x2a9488)
pub const SPRITE_RECT: u32 = 0x2759c8;
pub const GROUP_ADD: u32 = 0x2762e0;
pub const MENU_CTOR: u32 = 0x26d3b8;
pub const TEXT_CTOR: u32 = 0x2b1138;
pub const FIND: u32 = 0x2ac0c0;
pub const PAGES_LAYOUT: u32 = 0x2a9488;
/// The controller page's set-up kept once (a flag and the pictures' x), the pictures (3 of 16 bytes: x, y, -, the
/// sprite), the pictures' y, the three groups' label layouts (as `fe_misc::CONTROLLER_LAYOUTS`)
pub const CONTROLLER_ONCE: u32 = 0x348460;
pub const CONTROLLER_X: u32 = 0x3d35a8;
pub const CONTROLLER_PICTURES: u32 = 0x3483c8;
pub const CONTROLLER_Y: u32 = 0x34845c;
pub const CONTROLLER_LABELS: [(u32, u32); 3] = [(7, 0x3482c8), (9, 0x348338), (7, 0x348258)];
const MENU_LIST_VTABLE: u32 = 0x3b7fe0;

const ONE: u32 = 0x3f80_0000;
/// (1, 0.953, 0.729, 0.106)
const HIGHLIGHT: [u32; 4] = [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687];

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

fn menu_font(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x60)
}

/// The entry's child at `at` replaced: the old one destroyed (its +8 with 3)
fn set_child(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, c: u32) {
    let old = m.r32(w + at);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(w + at, c);
}

fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, x: F, y: F, buf: u32) {
    put(m, buf, [x.0.to_bits(), y.0.to_bits(), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[w, buf]);
}

/// A text entry (0x150 bytes, made in place, in the page) with its text (0x3b3f80); white, the highlight colour
/// and white (+0x110, +0x108, +0x50)
fn page_entry(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32, page: u32) -> u32 {
    let w = alloc(m, e, name, 0x150);
    let font = menu_font(m);
    text_entry_init(m, e, w, s, page);
    let t = alloc(m, e, 0x3b3f80, 0x130);
    let t = e.call(m, TEXT_CTOR, &[t, s, page, font]);
    set_child(m, e, w, 0x110, t);
    w
}

fn entry_colours(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    put(m, buf, [ONE; 4]);
    vcall(m, e, w, 0x110, &[buf]);
    put(m, buf, HIGHLIGHT);
    vcall(m, e, w, 0x108, &[buf]);
    put(m, buf, [ONE; 4]);
    vcall(m, e, w, 0x50, &[buf]);
}

/// `FE_BuildControllerConfig` 0x2a36e8(s; `buf` 0x20 bytes): the controller page:
/// - once (the flag at 0x348460): the pictures' x, centred by sprite 0x3d's width at 0.8 across 640;
/// - three pad pictures (icons of the sprites in 0x3483c8, sized by their sprites at 0.8) at that x and the y at
///   0x34845c plus their own, in the group at +0x90;
/// - three label groups (+0xc0..+0xc8: groups of 0x140 bytes in +0x90, each with its labels from its layout at 0.8,
///   hidden), placed at that x and y;
/// - the third shown, the pages laid out (0x2a9488);
/// - the page's (+0x8c) four entries (ids 0..3; strings 0xeb, 0xec, 0xb2, 0xb3), each with a sub-menu (0x1b0 bytes,
///   the same id) as its second child, and an end entry (0xdcd, the id 0xabcd1234);
/// - the first two sub-menus given the choices 4 / 5 (0xd5, 0xd7), the next two 6 / 7 (0xb7, 0xda), each laid out;
///   the page laid out
pub fn controller_config(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let grp = m.r32(s + 0x90);
    let page = m.r32(s + 0x8c);
    if m.r32(CONTROLLER_ONCE) == 0 {
        let r = e.call(m, SPRITE_RECT, &[s, 0x3d]);
        let w = rf(m, r.wrapping_add(4)) * F(0.8);
        m.w32(CONTROLLER_ONCE, 1);
        let x = (F(640.0) - w) * F(0.5);
        m.w32(CONTROLLER_X, x.0.to_bits());
    }
    for p in 0..3u32 {
        let i = alloc(m, e, 0x3b4090, 0x120);
        icon_init(m, e, i, s, 0);
        put(m, buf + 0x10, [fb(10.0), fb(10.0), 0, 0]);
        vcall_class(m, e, ICON_VTABLE, i, 0x138, &[buf + 0x10]);
        let entry = CONTROLLER_PICTURES + 16 * p;
        let sprite = m.r32(entry + 0xc);
        let r = e.call(m, SPRITE_RECT, &[s, sprite]);
        let w = rf(m, r.wrapping_add(4)) * F(0.8);
        let r = e.call(m, SPRITE_RECT, &[s, sprite]);
        let h = rf(m, r.wrapping_add(8)) * F(0.8);
        m.w32(i + 0x110, sprite);
        put(m, buf, [w.0.to_bits(), h.0.to_bits(), 0, 0]);
        vcall(m, e, i, 0x138, &[buf]);
        let x = rf(m, CONTROLLER_X) + rf(m, entry);
        let y = rf(m, CONTROLLER_Y) + rf(m, entry + 4);
        place(m, e, i, x, y, buf);
        e.call(m, GROUP_ADD, &[grp, i]);
    }
    for (p, &(count, table)) in CONTROLLER_LABELS.iter().enumerate() {
        let at = s + 0xc0 + 4 * p as u32;
        let g = alloc(m, e, 0x3b40a0, 0x140);
        group_init(m, e, g, s, grp);
        m.w32(at, g);
        for k in 0..count {
            let t = alloc(m, e, 0x3b40b0, 0x130);
            let font = menu_font(m);
            let parent = m.r32(at);
            let t = e.call(m, TEXT_CTOR, &[t, s, parent, font]);
            m.w32(t + 0x70, 0);
            let entry = table + 16 * k;
            let sid = m.r32(entry + 0xc);
            vcall(m, e, t, 0x150, &[sid]);
            place(m, e, t, rf(m, entry), rf(m, entry + 4), buf);
            m.w32(t + 0x118, 0x3f4c_cccd);
            let g = m.r32(at);
            e.call(m, GROUP_ADD, &[g, t]);
        }
        let g = m.r32(at);
        vcall(m, e, g, 0x28, &[0]);
        let (x, y) = (rf(m, CONTROLLER_X), rf(m, CONTROLLER_Y));
        let g = m.r32(at);
        place(m, e, g, x, y, buf);
        let g = m.r32(at);
        e.call(m, GROUP_ADD, &[grp, g]);
    }
    let third = m.r32(s + 0xc8);
    vcall(m, e, third, 0x28, &[1]);
    e.call(m, PAGES_LAYOUT, &[s]);
    // the entries and their sub-menus
    for (id, sid) in [(0u32, 0xebu32), (1, 0xec), (2, 0xb2), (3, 0xb3)] {
        let w = page_entry(m, e, s, 0x3b3fd8, page);
        let l = alloc(m, e, 0x3b40c0, 0x1b0);
        e.call(m, MENU_CTOR, &[l, s, page, 0, 0, 1]);
        m.w8(l + 0x112, 0xff);
        m.w32(l + 0xc, MENU_LIST_VTABLE);
        m.w8(l + 0x114, 0xff);
        m.w8(l + 0x115, 0xff);
        for (at, v) in [(0x154u32, 2u32), (0x190, 2), (0x1a0, 0), (0x194, 0), (0x198, 0), (0x19c, 0)] {
            m.w32(l + at, v);
        }
        m.w32(w + 0x11c, 5);
        entry_colours(m, e, w, buf);
        let t = m.r32(w + 0x110);
        vcall(m, e, t, 0x150, &[sid]);
        m.w32(w, id);
        let old = m.r32(w + 0x114);
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(l, id);
        m.w32(w + 0x114, l);
        vcall(m, e, page, 0x148, &[w]);
    }
    let w = page_entry(m, e, s, 0x3b4070, page);
    let t = m.r32(w + 0x110);
    vcall(m, e, t, 0x150, &[0xdcd]);
    entry_colours(m, e, w, buf);
    m.w32(w + 0x11c, 5);
    m.w32(w, 0xabcd_1234);
    vcall(m, e, page, 0x148, &[w]);
    let list = page + 0x118;
    for (first, choices) in [(0u32, [(4u32, 0xd5u32), (5, 0xd7)]), (2, [(6, 0xb7), (7, 0xda)])] {
        for k in 0..2 {
            let item = e.call(m, FIND, &[list, first + k]);
            let sub = m.r32(item + 0x114);
            for (id, sid) in choices {
                let t = alloc(m, e, 0x3b4080, 0x130);
                let font = menu_font(m);
                let t = e.call(m, TEXT_CTOR, &[t, s, sub, font]);
                m.w32(t, id);
                vcall(m, e, t, 0x150, &[sid]);
                vcall(m, e, sub, 0x148, &[t]);
            }
            vcall(m, e, sub, 0x140, &[]);
        }
    }
    vcall(m, e, page, 0x140, &[]);
}

// ---- the display page (0x2a53d0)

/// The display brightness (0x17b970: the app; f0) and a bar's value set (0x2b3950, f12)
pub const BRIGHTNESS: u32 = 0x17b970;
pub const BAR_VALUE: u32 = 0x2b3950;
/// The display page's choice strings by item: (item, id, string)
const DISPLAY_CHOICES: [(u32, u32, u32); 9] =
    [(0, 7, 0xbc), (0, 8, 0xcd), (1, 9, 0xd5), (1, 10, 0xde0), (1, 11, 0xde1), (3, 14, 0xd2), (3, 15, 0x78), (2, 12, 0xd7), (2, 13, 0xd5)];
/// The fifth item's choices' strings (five, 8 bytes apart), set through +0x158
pub const DISPLAY_NAMES: u32 = 0x3b40e0;

/// A choice widget's kind set-up: a slider (its +0xc0 answers) is given +0x174 0x1a, +0x16c 0x19 and white at
/// +0x130, +0x110 and +0x120; otherwise, when its +0x90 answers, its bytes +0x112, +0x114 and +0x115 are -1
fn choice_kind(m: &mut Mem, e: &mut dyn FeEnv, w: u32, slider: u32) {
    if slider != 0 {
        m.w32(w + 0x174, 0x1a);
        m.w32(w + 0x16c, 0x19);
        for at in [0x130u32, 0x110, 0x120] {
            put(m, w + at, [ONE; 4]);
        }
    } else if vcall(m, e, w, 0x90, &[]) != 0 {
        for at in [0x112u32, 0x114, 0x115] {
            m.w8(w + at, 0xff);
        }
    }
}

/// The entry finished: +0x11c 5, its colours, its text's string, its id, its second child `c` (with the same id)
fn display_entry_done(m: &mut Mem, e: &mut dyn FeEnv, w: u32, c: u32, id: u32, sid: u32, page: u32, buf: u32) {
    m.w32(w + 0x11c, 5);
    entry_colours(m, e, w, buf);
    let t = m.r32(w + 0x110);
    vcall(m, e, t, 0x150, &[sid]);
    m.w32(w, id);
    let old = m.r32(w + 0x114);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(c, id);
    m.w32(w + 0x114, c);
    vcall(m, e, page, 0x148, &[w]);
}

/// The display page's builder 0x2a53d0(s; `buf` 0x30 bytes): the page (+0x98) given five entries (ids 0..4; strings
/// 0xe3, 0xed, 0x7c, 0xdc, 0xc9), each with a sub-menu (0x1b0 bytes; its kind set up) as its second child; a sixth
/// (id 5, 0x80) with a progress bar (0x190 bytes, 100 × 30); an end entry (0xdcd, the id 0xabcd1234); the sub-menus'
/// choices (+0x154 2) and the fifth's five names; each sub-menu laid out; the bar shown from 0 by 0.02 at the display
/// brightness; the page laid out
pub fn display_page(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let page = m.r32(s + 0x98);
    for (id, sid) in [(0u32, 0xe3u32), (1, 0xed), (2, 0x7c), (3, 0xdc), (4, 0xc9)] {
        let w = page_entry(m, e, s, 0x3b3fd8, page);
        let l = alloc(m, e, 0x3b40c0, 0x1b0);
        e.call(m, MENU_CTOR, &[l, s, page, 0, 0, 1]);
        m.w32(l + 0x190, 2);
        m.w32(l + 0x194, 0);
        m.w32(l + 0xc, MENU_LIST_VTABLE);
        for at in [0x198u32, 0x19c, 0x1a0] {
            m.w32(l + at, 0);
        }
        let slider = vcall_class(m, e, MENU_LIST_VTABLE, l, 0xc0, &[]);
        choice_kind(m, e, l, slider);
        display_entry_done(m, e, w, l, id, sid, page, buf);
    }
    // the brightness bar
    let w = page_entry(m, e, s, 0x3b3fd8, page);
    let b = alloc(m, e, 0x3b40c8, 0x190);
    crate::fe_widgets::progress_init(m, e, b, s, page);
    put(m, buf + 0x20, [fb(100.0), fb(30.0), 0, 0]);
    vcall_class(m, e, crate::fe_widgets::PROGRESS_VTABLE, b, 0x138, &[buf + 0x20]);
    let slider = vcall(m, e, b, 0xc0, &[]);
    choice_kind(m, e, b, slider);
    display_entry_done(m, e, w, b, 5, 0x80, page, buf);
    // the end
    let w = page_entry(m, e, s, 0x3b4070, page);
    let t = m.r32(w + 0x110);
    vcall(m, e, t, 0x150, &[0xdcd]);
    entry_colours(m, e, w, buf);
    m.w32(w + 0x11c, 5);
    m.w32(w, 0xabcd_1234);
    vcall(m, e, page, 0x148, &[w]);
    // the choices
    let list = page + 0x118;
    for (k, id, sid) in DISPLAY_CHOICES {
        let item = e.call(m, FIND, &[list, k]);
        let sub = m.r32(item + 0x114);
        m.w32(sub + 0x154, 2);
        let t = alloc(m, e, 0x3b40d0, 0x130);
        let font = menu_font(m);
        let t = e.call(m, TEXT_CTOR, &[t, s, sub, font]);
        m.w32(t, id);
        vcall(m, e, t, 0x150, &[sid]);
        vcall(m, e, sub, 0x148, &[t]);
    }
    let item = e.call(m, FIND, &[list, 4]);
    let sub = m.r32(item + 0x114);
    m.w32(sub + 0x154, 2);
    for i in 0..5u32 {
        let t = alloc(m, e, 0x3b4080, 0x130);
        let font = menu_font(m);
        let t = e.call(m, TEXT_CTOR, &[t, s, sub, font]);
        m.w32(t, 0x10 + i);
        vcall(m, e, t, 0x158, &[DISPLAY_NAMES + 8 * i]);
        vcall(m, e, sub, 0x148, &[t]);
    }
    for k in 0..5u32 {
        let item = e.call(m, FIND, &[list, k]);
        let sub = m.r32(item + 0x114);
        vcall(m, e, sub, 0x140, &[]);
    }
    let item = e.call(m, FIND, &[list, 5]);
    let bar = m.r32(item + 0x114);
    vcall(m, e, bar, 0xf8, &[0]);
    let x = m.r32(bar + 0x100);
    put(m, buf, [x, fb(15.0), 0, 0]);
    vcall(m, e, bar, 0x138, &[buf]);
    m.w32(bar + 0x150, ONE);
    m.w32(bar + 0x164, 0x3ca3_d70a);
    m.w32(bar + 0x154, 0);
    let app = m.r32(GAPP);
    let v = e.call(m, BRIGHTNESS, &[app]);
    e.call(m, BAR_VALUE, &[bar, v]);
    vcall(m, e, page, 0x140, &[]);
}

//! The front end's options screen (board row G9o): `cFE3DStateOptions`'s methods and its display settings' apply
//! (0x29f720) on the game's bytes, every call through [`FeEnv`] (the base's methods it calls run as ported,
//! [`crate::fe3dstate`]; its pages' own helpers are calls out), checked in the function runner (`tests/fe_options.rs`).
//!
//! The screen's menu (+0x3c) lists five pages (+0x7c + 12k: the page's text id, +0x80 its widget, +0x84 its group);
//! +0x78 is the page open (-1 none). Page 3 (+0xa4) is the one whose choice opens another screen. A page's item whose
//! first word is 0xabcd1234 is its "accept" line.

use crate::fe3dstate::{self as base, vcall, FeEnv, ALLOC, FILL, GAPP, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use crate::fe_numplayers::BUILD_TITLED;
use crate::fe_title::{BACKDROP, BASE_CTOR, HELP_BUTTON, TEXT_CTOR};
use crate::fe_widgets::{group_init, GROUP_ADD};
use crate::pf::Mem;

pub const VTABLE: u32 = 0x3b42e8;
/// A page's accept line's mark
pub const ACCEPT: u32 = 0xabcd_1234;
/// The screen's helpers (calls out): 0x2a8520 the title's extras; the five pages' builders (0x29fb70, 0x2a36e8,
/// 0x2a53d0, 0x2a7fd8, 0x2a8380); 0x26d3b8 a page widget made (page, screen, group, 1, 2, 0); 0x2a9488 the pages'
/// layout; 0x29fb40 the values read back; 0x2a9350 a page's line chosen; 0x2aa998 the help line; the pages' item moves
/// (0x2a9720, 0x2a9848, 0x2a99a0, 0x2a9c50); the pages opened (0x29e988, 0x29ebc0, 0x29ed50), closed (0x29ea90,
/// 0x29ec70, 0x29ee30) and their values kept (0x29f178, 0x29f508, 0x29f720 the display's); 0x26d8c8 and 0x2ae3c0 a
/// slider's and a list's arrows shown; 0x17b980 the sound volume (gApp, f12); 0x28f260 the memory card screen made
pub const TITLE_EXTRAS: u32 = 0x2a8520;
pub const PAGE_BUILDERS: [u32; 5] = [0x29fb70, 0x2a36e8, 0x2a53d0, 0x2a7fd8, 0x2a8380];
pub const PAGE_WIDGET: u32 = 0x26d3b8;
pub const PAGES_LAYOUT: u32 = 0x2a9488;
pub const VALUES_READ: u32 = 0x29fb40;
pub const LINE_CHOSEN: u32 = 0x2a9350;
pub const HELP: u32 = 0x2aa998;
pub const ITEM_MOVES: [u32; 4] = [0x2a9720, 0x2a9848, 0x2a99a0, 0x2a9c50];
pub const PAGE_OPEN: [u32; 3] = [0x29e988, 0x29ebc0, 0x29ed50];
pub const PAGE_CLOSE: [u32; 3] = [0x29ea90, 0x29ec70, 0x29ee30];
pub const PAGE_KEEP: [u32; 3] = [0x29f178, 0x29f508, 0x29f720];
pub const SLIDER_ARROWS: u32 = 0x26d8c8;
pub const LIST_ARROWS: u32 = 0x2ae3c0;
pub const SET_VOLUME: u32 = 0x17b980;
pub const CARD_SCREEN: u32 = 0x28f260;
/// The music: 0x262608 the options loop; the pages' cues (0x2627b0, 0x262670, 0x2626f0, 0x262770, 0x2626b0)
pub const MUSIC_LOOP: u32 = 0x262608;
pub const PAGE_CUES: [u32; 5] = [0x2627b0, 0x262670, 0x2626f0, 0x262770, 0x2626b0];
/// The base's +0x28c08 clean up (0x2717e8, the base's vf12)
pub const BASE_VF12: u32 = 0x2717e8;
/// The career's display settings (words at 0x32ca10..0x32ca4c) and the display (gApp+0x724; its vtable at +0x10)
pub const SETTINGS: u32 = 0x32b398;
/// A list's k-th entry by number (list, k), as 0x2ac048 but walking from the head
pub const LIST_AT: u32 = 0x2ac0c0;

const ONE: u32 = 0x3f80_0000;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn fl(v: u32) -> f32 {
    f32::from_bits(v)
}

fn put4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32, align: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, align]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

fn page_group(m: &Mem, s: u32) -> u32 {
    let k = m.r32(s + 0x78);
    m.r32(s.wrapping_add(k.wrapping_mul(12)).wrapping_add(0x84))
}

/// A virtual call on an object whose vtable is at its +0 (the 3D scene)
fn vcall0(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let fun = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, fun, &a)
}

fn clear_pages(m: &mut Mem, s: u32) {
    for k in 0..5u32 {
        m.w32(s + 0x84 + 12 * k, 0);
        m.w32(s + 0x80 + 12 * k, 0);
    }
}

/// `cFE3DStateOptions::cFE3DStateOptions` 0x29e920: the base constructor, no page open, the vtable, no back, the pages'
/// widgets and groups cleared
pub fn construct(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    e.call(m, BASE_CTOR, &[s]);
    m.w32(s + 0x78, u32::MAX);
    m.w32(s + 0xc, VTABLE);
    m.w32(s + 0x18, 0);
    clear_pages(m, s);
    s
}

/// `cFE3DStateOptions` vf12 0x2a9208: the base's vf12, the pages' widgets and groups cleared
pub fn vf12(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    base::vf12(m, e, s);
    clear_pages(m, s);
}

/// `cFE3DStateOptions` vf11 0x2a8a30, the screen built (`buf` 0x50 bytes of scratch):
/// - the base widgets with the title (0x274278: 450.0, text 0xda5, 10), the title's colour (0.88, 0.1, 0.1, 0.1) and
///   its extras (0x2a8520);
/// - the pages' text ids (0xe18, 0xb0, 0xba, 0xcc, 0xb4) and the menu's +0x114 / +0x115 bytes -1;
/// - for each page: its menu item (the text, +0x118 the word at 0x3ae520, the highlight, white, numbered); its group
///   (no parent, +0x70 1, numbered, hidden then opened, under the title) holding its title text centred at (320, 60);
///   its page widget (0x26d3b8; +0x114 / +0x115 -1, +0x158 1; page 3's +0x154 0);
/// - the five pages' builders; then each page placed (its size, the help bar's extent, the title's height: centred
///   below the title, with page-specific x and y for pages 1, 3, 4) and laid out (+0x140), each group closed;
/// - the backdrop; no fade in.
pub fn build(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    e.call(m, BUILD_TITLED, &[450.0f32.to_bits(), s, 0xda5, 0xa]);
    let t = m.r32(s + 0x38);
    let tenth = 0.1f32.to_bits();
    put4(m, buf, [0x3f61_47ae, tenth, tenth, tenth]);
    vcall(m, e, t, 0x50, &[buf]);
    e.call(m, TITLE_EXTRAS, &[s]);
    let menu = m.r32(s + 0x3c);
    for (k, id) in [0xe18u32, 0xb0, 0xba, 0xcc, 0xb4].iter().enumerate() {
        m.w32(s + 0x7c + 12 * k as u32, *id);
    }
    m.w8(menu + 0x115, 0xff);
    m.w8(menu + 0x114, 0xff);
    let (b0, b10) = (buf, buf + 0x10);
    for k in 0..5u32 {
        let at = alloc(m, e, 0x3b4120, 0x130, 0);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let w = e.call(m, TEXT_CTOR, &[at, s, menu, font]);
        let word = m.r32(0x3ae520);
        m.w32(w + 0x118, word);
        let id = m.r32(s + 0x7c + 12 * k);
        vcall(m, e, w, 0x150, &[id]);
        put4(m, b0, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
        vcall(m, e, w, 0x108, &[b0]);
        put4(m, b0, [ONE; 4]);
        vcall(m, e, w, 0x110, &[b0]);
        put4(m, b0, [ONE; 4]);
        vcall(m, e, w, 0x50, &[b0]);
        m.w32(w, k);
        vcall(m, e, menu, 0x148, &[w]);
        let g = alloc(m, e, 0x3b4130, 0x140, 0);
        group_init(m, e, g, s, 0);
        let t2 = alloc(m, e, 0x3b4140, 0x130, 0x10);
        let font = m.r32(m.r32(GAPP).wrapping_add(0x5c));
        let tt = e.call(m, TEXT_CTOR, &[t2, s, g, font]);
        let id = m.r32(s + 0x7c + 12 * k);
        vcall(m, e, tt, 0x150, &[id]);
        vcall(m, e, tt, 0x130, &[b0]);
        let x = 320.0 - fl(m.r32(b0)) * 0.5;
        put4(m, b10, [x.to_bits(), 60.0f32.to_bits(), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[tt, b10]);
        e.call(m, GROUP_ADD, &[g, tt]);
        m.w32(g.wrapping_add(0x70), 1);
        m.w32(g, k);
        m.w32(s + 0x84 + 12 * k, g);
        vcall(m, e, g, 0x28, &[0]);
        let pp = alloc(m, e, 0x3b4160, 0x190, 0);
        let page = e.call(m, PAGE_WIDGET, &[pp, s, g, 1, 2, 0]);
        m.w32(s + 0x80 + 12 * k, page);
        m.w8(page.wrapping_add(0x114), 0xff);
        let page = m.r32(s + 0x80 + 12 * k);
        m.w8(page.wrapping_add(0x115), 0xff);
        if k == 3 {
            let p3 = m.r32(s + 0xa4);
            m.w32(p3.wrapping_add(0x154), 0);
        }
        let page = m.r32(s + 0x80 + 12 * k);
        m.w32(page.wrapping_add(0x158), 1);
        e.call(m, GROUP_ADD, &[g, page]);
        vcall(m, e, g, 0x20, &[1]);
        let t = m.r32(s + 0x38);
        e.call(m, GROUP_ADD, &[t, g]);
    }
    for f in PAGE_BUILDERS {
        e.call(m, f, &[s]);
    }
    let (b20, b30, b40) = (buf + 0x20, buf + 0x30, buf + 0x40);
    for j in 0..5u32 {
        let page = m.r32(s + 0x80 + 12 * j);
        vcall(m, e, page, 0x130, &[b20]);
        let hb = m.r32(s + 0x40);
        vcall(m, e, hb, 0x128, &[b0, b10]);
        let mut y = fl(m.r32(b10 + 4)) - fl(m.r32(b0 + 4));
        let t = m.r32(s + 0x38);
        vcall(m, e, t, 0x130, &[b30]);
        let th = fl(m.r32(m.r32(s + 0x38).wrapping_add(0x154)));
        y -= 70.0;
        let room = 450.0 - th;
        let page = m.r32(s + 0x80 + 12 * j);
        if page != 0 {
            let d = (room - fl(m.r32(b20 + 4))) * 0.5;
            m.w32(page + 0x15c, 6.0f32.to_bits());
            y += d;
            let mut x = 320.0f32;
            if j == 3 {
                x = 320.0 - fl(m.r32(b20)) * 0.5;
            } else if j == 4 {
                y = 105.0;
            } else if j == 1 {
                let p1 = m.r32(s + 0x8c);
                y = 290.0;
                m.w32(p1.wrapping_add(0x15c), 2.0f32.to_bits());
            }
            let page = m.r32(s + 0x80 + 12 * j);
            put4(m, b40, [x.to_bits(), y.to_bits(), 0, 0]);
            vcall(m, e, page, 0x70, &[b40]);
            let page = m.r32(s + 0x80 + 12 * j);
            vcall(m, e, page, 0x140, &[]);
        }
        let g = m.r32(s + 0x84 + 12 * j);
        vcall(m, e, g, 0x28, &[0]);
    }
    e.call(m, BACKDROP, &[s]);
    m.w32(s + 0x20, 0);
}

/// `cFE3DStateOptions::Activate` 0x2a9258 (the screen, the one before): nothing while the front end's +6 is set; else
/// the base's `Activate`, the open page's group opened, the values read back (0x29fb40); after the memory card screen
/// (its +0x70 answers 0xd) the screen's +0x48, else the options music; then page 1's line chosen (0x2a9350)
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32, prev: u32) {
    if m.r8(fe(m) + 6) != 0 {
        return;
    }
    base::activate(m, e, s);
    if m.r32(s + 0x78) != u32::MAX {
        let g = page_group(m, s);
        vcall(m, e, g, 0x28, &[1]);
    }
    e.call(m, VALUES_READ, &[s]);
    if vcall(m, e, prev, 0x70, &[]) == 0xd {
        vcall(m, e, s, 0x48, &[]);
    } else {
        let f = fe(m);
        e.call(m, MUSIC_LOOP, &[f + 0x70]);
    }
    let p1 = m.r32(s + 0x8c);
    let line = m.r32(p1.wrapping_add(0x140));
    e.call(m, LINE_CHOSEN, &[s, line]);
}

/// `cFE3DStateOptions::CreateNextState` 0x2a9c58: on page 3 only, the memory card screen (0x2c84 bytes, 16-aligned;
/// 0x28f260 with (1, 1) for the page's line 0, (1, 0) for line 1, else (0, 0)), its +0x42c (none if it failed)
pub fn create_next_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    if m.r32(s + 0x78) != 3 {
        return 0;
    }
    let p3 = m.r32(s + 0xa4);
    let (a, b) = match m.r32(m.r32(p3 + 0x140)) {
        0 => (1, 1),
        1 => (1, 0),
        _ => (0, 0),
    };
    let at = alloc(m, e, 0x3b4238, 0x2c84, 0x10);
    let r = e.call(m, CARD_SCREEN, &[at, a, b]);
    if r == 0 {
        0
    } else {
        r.wrapping_add(0x42c)
    }
}

/// `cFE3DStateOptions` vf22 0x2a9d38 (the item that goes, the one that comes): the base's vf22; the going item's
/// control's arrows hidden and the coming one's shown (a slider's via 0x26d8c8, a list's via 0x2ae3c0, the screen's
/// two arrows handed over); the help line (0x2aa998); the help bar's second button enabled on an accept line, with
/// no page or page 3 open, else disabled; then with a page open: an item without a control moves the page's own way
/// (0x2a9720.. by page), and on page 1 the line chosen (0x2a9350)
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, s: u32, from: u32, to: u32) {
    base::vf22(m, e, from, to);
    if from != 0 && vcall(m, e, from, 0xb8, &[]) != 0 && m.r32(from + 0x114) != 0 {
        let c = m.r32(from + 0x114);
        let hide = if vcall(m, e, c, 0x90, &[]) != 0 {
            let c = m.r32(from + 0x114);
            m.w32(c + 0x14c, 0);
            m.w32(c + 0x148, 0);
            true
        } else {
            let c = m.r32(from + 0x114);
            if vcall(m, e, c, 0xc0, &[]) != 0 {
                let c = m.r32(from + 0x114);
                m.w32(c + 0x18c, 0);
                m.w32(c + 0x188, 0);
                true
            } else {
                false
            }
        };
        if hide {
            let a = m.r32(s + 0xb8);
            vcall(m, e, a, 0x28, &[0]);
            let b = m.r32(s + 0xbc);
            vcall(m, e, b, 0x28, &[0]);
        }
    }
    if to != 0 && vcall(m, e, to, 0xb8, &[]) != 0 && m.r32(to + 0x114) != 0 {
        let c = m.r32(to + 0x114);
        if vcall(m, e, c, 0x90, &[]) != 0 {
            let c = m.r32(to + 0x114);
            let (a, b) = (m.r32(s + 0xb8), m.r32(s + 0xbc));
            m.w32(c + 0x148, a);
            m.w32(c + 0x14c, b);
            e.call(m, SLIDER_ARROWS, &[c]);
        } else {
            let c = m.r32(to + 0x114);
            if vcall(m, e, c, 0xc0, &[]) != 0 {
                let c = m.r32(to + 0x114);
                let (a, b) = (m.r32(s + 0xb8), m.r32(s + 0xbc));
                m.w32(c + 0x188, a);
                m.w32(c + 0x18c, b);
                e.call(m, LIST_ARROWS, &[c]);
            }
        }
    }
    e.call(m, HELP, &[s, to]);
    let open = m.r32(s + 0x78);
    let on = m.r32(to) == ACCEPT || open == u32::MAX || open == 3;
    let b = e.call(m, HELP_BUTTON, &[s, 2]);
    vcall(m, e, b, 0x100, &[on as u32]);
    if m.r32(to) == ACCEPT {
        return;
    }
    let open = m.r32(s + 0x78);
    if open == u32::MAX {
        return;
    }
    if from != 0 && vcall(m, e, to, 0xb8, &[]) == 0 {
        let open = m.r32(s + 0x78);
        if let Some(&f) = ITEM_MOVES.get(open as usize) {
            e.call(m, f, &[s, from, to]);
        }
        return;
    }
    if m.r32(s + 0x78) == 1 {
        e.call(m, LINE_CHOSEN, &[s, to]);
    }
}

/// `cFE3DStateOptions` vf23 0x2aa098 (an item's value changed): on page 0, the 3D scene's setting for lines 2..6
/// (+0x158, +0x178, +0x198, +0x168, +0x1a8 with the item's +0x160) and its +0x50 cue (0x12, 0x13, 0x15, none, 0x16);
/// on page 2, line 5 sets the sound volume
pub fn vf23(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    match m.r32(s + 0x78) {
        0 => {
            let scene = m.r32(fe(m) + 0xa4);
            let k = m.r32(item).wrapping_sub(2);
            let v = m.r32(item + 0x160);
            let (slot, cue) = match k {
                0 => (0x158, Some(0x12)),
                1 => (0x178, Some(0x13)),
                2 => (0x198, Some(0x15)),
                3 => (0x168, None),
                4 => (0x1a8, Some(0x16)),
                _ => return,
            };
            vcall0(m, e, scene, slot, &[v]);
            if let Some(c) = cue {
                vcall0(m, e, scene, 0x50, &[c]);
            }
        }
        2 => {
            if m.r32(item) == 5 {
                let v = m.r32(item + 0x160);
                let app = m.r32(GAPP);
                e.call(m, SET_VOLUME, &[app, v]);
            }
        }
        _ => {}
    }
}

/// The going item's control's arrows hidden when the screen closes a page: as vf22's, the control read at `ctl_off`
/// (+0xf0 first, then the item's own list at `list_off`)
fn close_controls(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, list_off: u32) {
    let c = m.r32(item + 0xf0);
    if c != 0 && vcall(m, e, c, 0x90, &[]) != 0 {
        let c = m.r32(item + 0xf0);
        m.w32(c + 0x14c, 0);
        m.w32(c + 0x148, 0);
    } else {
        if vcall(m, e, item, 0xb8, &[]) == 0 {
            return;
        }
        let c = m.r32(item + list_off);
        if list_off == 0x114 && c == 0 {
            return;
        }
        if vcall(m, e, c, 0xc0, &[]) == 0 {
            return;
        }
        let c = m.r32(item + list_off);
        m.w32(c + 0x18c, 0);
        m.w32(c + 0x188, 0);
    }
    let a = m.r32(s + 0xb8);
    vcall(m, e, a, 0x28, &[0]);
    let b = m.r32(s + 0xbc);
    vcall(m, e, b, 0x28, &[0]);
}

/// `cFE3DStateOptions` vf26 0x2aa1f8 (an item picked):
/// - with no page open: the page opens (+0x78), the widgets reset onto it (0x275468), its cue and opener (page 1 also
///   its line chosen, on its third line for an accept line; page 4 the screen's +0xb4 opened and the help bar's second
///   button disabled), the menu closed and the page's group opened;
/// - on page 3: the screen's +0x30 and +0x48;
/// - on another page's accept line: the page's values kept (0: 0x29f178, 1: 0x29f508, 2: the display's), page 4 does
///   nothing; unless the item is the menu, its control's arrows hidden; the page closed (the menu opened, the group
///   closed), none open, the widgets reset onto the menu.
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let open = m.r32(s + 0x78);
    if open == u32::MAX {
        let k = m.r32(item);
        m.w32(s + 0x78, k);
        let page = m.r32(s.wrapping_add(k.wrapping_mul(12)).wrapping_add(0x80));
        e.call(m, WIDGETS_RESET, &[s, page]);
        let f = fe(m);
        match m.r32(s + 0x78) {
            0 => {
                e.call(m, PAGE_CUES[0], &[f + 0x70]);
                e.call(m, PAGE_OPEN[0], &[s]);
            }
            1 => {
                e.call(m, PAGE_CUES[1], &[f + 0x70]);
                e.call(m, PAGE_OPEN[1], &[s]);
                let mut line = m.r32(page.wrapping_add(0x140));
                if m.r32(line) == ACCEPT {
                    line = e.call(m, LIST_AT, &[page.wrapping_add(0x118), 2]);
                }
                e.call(m, LINE_CHOSEN, &[s, line]);
            }
            2 => {
                e.call(m, PAGE_CUES[2], &[f + 0x70]);
                e.call(m, PAGE_OPEN[2], &[s]);
            }
            3 => {
                e.call(m, PAGE_CUES[3], &[f + 0x70]);
            }
            4 => {
                let x = m.r32(s + 0xb4);
                vcall(m, e, x, 0x20, &[1]);
                let f = fe(m);
                e.call(m, PAGE_CUES[4], &[f + 0x70]);
                let b = e.call(m, HELP_BUTTON, &[s, 2]);
                vcall(m, e, b, 0x100, &[0]);
            }
            _ => {}
        }
        let menu = m.r32(s + 0x3c);
        vcall(m, e, menu, 0x28, &[0]);
        let g = page_group(m, s);
        vcall(m, e, g, 0x28, &[1]);
        return;
    }
    if open == 3 {
        vcall(m, e, s, 0x30, &[]);
        vcall(m, e, s, 0x48, &[]);
        return;
    }
    if m.r32(item) != ACCEPT {
        return;
    }
    match open {
        0 => {
            e.call(m, PAGE_KEEP[0], &[s]);
        }
        1 => {
            e.call(m, PAGE_KEEP[1], &[s]);
        }
        2 => {
            e.call(m, PAGE_KEEP[2], &[s]);
        }
        4 => return,
        _ => {}
    }
    close_page(m, e, s, item, 0x110, false);
}

/// The end of vf26 / vf21: the item's control's arrows hidden (unless the item is the menu), the menu opened and the
/// page's group closed (vf21 also the 3D scene's +0x10(0x24)), none open, the widgets reset onto the menu
fn close_page(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32, list_off: u32, back: bool) {
    let menu = m.r32(s + 0x3c);
    if item != menu {
        close_controls(m, e, s, item, list_off);
    }
    let menu = m.r32(s + 0x3c);
    if m.r32(s + 0x78) != u32::MAX {
        vcall(m, e, menu, 0x28, &[1]);
        let g = page_group(m, s);
        vcall(m, e, g, 0x28, &[0]);
        if back {
            let scene = m.r32(fe(m) + 0xa4);
            vcall0(m, e, scene, 0x10, &[0x24]);
        }
    }
    let menu = m.r32(s + 0x3c);
    m.w32(s + 0x78, u32::MAX);
    e.call(m, WIDGETS_RESET, &[s, menu]);
}

/// `cFE3DStateOptions` vf21 0x2aa5c0 (backing out): with no page open, the base's vf21; else the page closed without
/// keeping its values (0: 0x29ea90 then 0x29f178, 1: 0x29ec70 then 0x29f508, 2: 0x29ee30 then the display's; page 4
/// the screen's +0xb4 closed and the help bar's second button disabled), the item's control's arrows hidden, the
/// page's group closed with the 3D scene's +0x10(0x24), none open
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    match m.r32(s + 0x78) {
        u32::MAX => {
            base::vf21(m, e, s);
            return;
        }
        0 => {
            e.call(m, PAGE_CLOSE[0], &[s]);
            e.call(m, PAGE_KEEP[0], &[s]);
        }
        1 => {
            e.call(m, PAGE_CLOSE[1], &[s]);
            e.call(m, PAGE_KEEP[1], &[s]);
        }
        2 => {
            e.call(m, PAGE_CLOSE[2], &[s]);
            e.call(m, PAGE_KEEP[2], &[s]);
        }
        4 => {
            let x = m.r32(s + 0xb4);
            vcall(m, e, x, 0x20, &[0]);
            let b = e.call(m, HELP_BUTTON, &[s, 2]);
            vcall(m, e, b, 0x100, &[0]);
        }
        _ => {}
    }
    close_page(m, e, s, item, 0x114, true);
}

/// `cFE3DStateOptions::LayoutWidgets` 0x2aa820 (`buf` 0x40 bytes): the base's; each page group's title centred across
/// 640; the pages' layout (0x2a9488); page 3 centred across 640 by the help bar's extent
pub fn layout_widgets(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    base::layout_widgets(m, e, s, buf);
    let (b0, b10, b20) = (buf, buf + 0x10, buf + 0x20);
    for k in 0..5u32 {
        let g = m.r32(s + 0x84 + 12 * k);
        let w = e.call(m, LIST_NTH, &[g.wrapping_add(0x110), 0]);
        vcall(m, e, w, 0x130, &[b0]);
        let x = (640.0 - fl(m.r32(b0))) * 0.5;
        let y = m.r32(w.wrapping_add(0x84));
        put4(m, b10, [x.to_bits(), y, 0, ONE]);
        e.call(m, WIDGET_PLACE, &[w, b10]);
    }
    e.call(m, PAGES_LAYOUT, &[s]);
    let p3 = m.r32(s + 0xa4);
    vcall(m, e, p3, 0x128, &[b0, b10]);
    let wdt = fl(m.r32(b10)) - fl(m.r32(b0));
    let x = (640.0 - wdt) * 0.5;
    let y = m.r32(p3.wrapping_add(0x84));
    put4(m, b20, [x.to_bits(), y, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[p3, b20]);
}

/// An unsigned word to a float as the game converts it (halved with the low bit kept, then doubled, when its top bit
/// is set)
fn u32_to_f32(v: u32) -> f32 {
    if (v as i32) >= 0 {
        v as i32 as f32
    } else {
        let h = ((v >> 1) | (v & 1)) as i32 as f32;
        h + h
    }
}

/// 0x29f720 (`FEOptions_ApplyDisplay`): the display page's lines (+0x98's list, each line's control's chosen word)
/// into the career's settings: line 0 (8: widescreen), line 1 (9, 10, 11: the screen mode 0, 1, 2 with the display's
/// +0x120 mode set and its picture offset (+0x98) reset to (0, 0) with its size (+0x58, +0x60)), line 2 (0xc), line 3
/// (0xf), line 4 (0x10..0x14: 2..6), and line 5's value to the sound volume
pub fn apply_display(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let list = m.r32(s + 0x98).wrapping_add(0x118);
    let disp = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let chosen = |m: &Mem, line: u32| m.r32(m.r32(m.r32(line.wrapping_add(0x114)).wrapping_add(0x140)));
    let l = e.call(m, LIST_AT, &[list, 0]);
    m.w32(SETTINGS + 0x1678, (chosen(m, l) == 8) as u32);
    let l = e.call(m, LIST_AT, &[list, 1]);
    let mode = match chosen(m, l) {
        9 => Some((0, 0)),
        10 => Some((1, 1)),
        11 => Some((1, 2)),
        _ => None,
    };
    if let Some((flag, mode)) = mode {
        m.w32(SETTINGS + 0x167c, flag);
        m.w32(SETTINGS + 0x167c, mode);
        m.w32(SETTINGS + 0x1680, 0);
        let vt = m.r32(disp + 0x10);
        let adj = m.r16(vt + 0x120) as i16 as u32;
        let fun = m.r32(vt + 0x124);
        e.call(m, fun, &[disp.wrapping_add(adj), mode]);
        let vt = m.r32(disp + 0x10);
        let at = disp.wrapping_add(m.r16(vt + 0x98) as i16 as u32);
        let off_fn = m.r32(vt + 0x9c);
        let adj = m.r16(vt + 0x58) as i16 as u32;
        let fun = m.r32(vt + 0x5c);
        let w = u32_to_f32(e.call(m, fun, &[disp.wrapping_add(adj)]));
        let vt = m.r32(disp + 0x10);
        let adj = m.r16(vt + 0x60) as i16 as u32;
        let fun = m.r32(vt + 0x64);
        let h = u32_to_f32(e.call(m, fun, &[disp.wrapping_add(adj)]));
        e.call(m, off_fn, &[at, 0, 0, w.to_bits(), h.to_bits()]);
    }
    let l = e.call(m, LIST_AT, &[list, 2]);
    m.w32(SETTINGS + 0x169c, (chosen(m, l) == 0xc) as u32);
    let l = e.call(m, LIST_AT, &[list, 3]);
    m.w32(SETTINGS + 0x16b4, (chosen(m, l) == 0xf) as u32);
    let l = e.call(m, LIST_AT, &[list, 4]);
    let v = chosen(m, l).wrapping_sub(0x10);
    if v < 5 {
        m.w32(SETTINGS + 0x16b0, v + 2);
    }
    let l = e.call(m, LIST_AT, &[list, 5]);
    let vol = m.r32(m.r32(l.wrapping_add(0x114)).wrapping_add(0x160));
    let app = m.r32(GAPP);
    e.call(m, SET_VOLUME, &[app, vol]);
}

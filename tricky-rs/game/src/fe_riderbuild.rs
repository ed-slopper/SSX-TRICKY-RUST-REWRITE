//! The rider profile screen's builders (board row G9y8) on the game's bytes: `FE_BuildRiderProfileScreen` (the
//! profile's frame: its backdrop, picture, side bar and two bands), then the panels' builders. The widgets are made
//! in place as the game inlines them ([`crate::fe_widgets`]); every other call goes out through [`FeEnv`]. Checked in
//! the function runner (`tests/fe_riderbuild.rs`).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, WIDGET_PLACE};
use crate::fe_widgets::{icon_init, vcall_class, ICON_VTABLE};
use crate::fe_world::SCALED_ICON_VTABLE;
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: a sprite's rectangle (0x2759c8), a widget added to the state (0x2763f0)
pub const SPRITE_RECT: u32 = 0x2759c8;
pub const STATE_ADD: u32 = 0x2763f0;
/// The frame's pieces' allocation names
pub const BACKDROP_NAME: u32 = 0x3af828;
pub const PICTURE_NAME: u32 = 0x3af838;
pub const SIDE_NAME: u32 = 0x3af848;
pub const BAND_NAME: u32 = 0x3af858;

const ONE: u32 = 0x3f80_0000;
/// (1, 0.624, 0.839, 0.827): the backdrop's colour
const PALE: [u32; 4] = [ONE, 0x3f1f_9db2, 0x3f56_d5d0, 0x3f53_d07d];
/// (1, 0.391, 0.527, 0.519): the side bar's
const DARK: [u32; 4] = [ONE, 0x3ec8_4b5e, 0x3f06_c8b4, 0x3f04_e3bd];

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

/// An icon (made in place, of `size` bytes) for the screen, sized 10 × 10 by the class
fn icon(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32, size: u32, buf: u32) -> u32 {
    let i = e.call(m, ALLOC, &[name, size, 0]);
    if i != 0 {
        e.call(m, FILL, &[i, 0xdead_c0de, size]);
    }
    icon_init(m, e, i, s, 0);
    put(m, buf + 0x10, [fb(10.0), fb(10.0), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, i, 0x138, &[buf + 0x10]);
    i
}

fn vcall_v(m: &mut Mem, e: &mut dyn FeEnv, w: u32, off: u32, v: [u32; 4], buf: u32) {
    put(m, buf, v);
    vcall(m, e, w, off, &[buf]);
}

fn place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, x: u32, y: u32, buf: u32) {
    put(m, buf, [x, y, 0, ONE]);
    e.call(m, WIDGET_PLACE, &[w, buf]);
}

/// `FE_BuildRiderProfileScreen` 0x27e918(s; `buf` 0x20 bytes): the profile's frame, each piece added to the state:
/// - the backdrop (pale, 460 × 480 at (180, 0), +0x70 0);
/// - the picture (+0x4ec: 180 × 364 at (0, 58), white, +0x70 2, sprite 0x7c);
/// - the side bar (a scaled icon of sprite 0x28: as wide as the sprite by 364, dark, at (180 less its width, 58),
///   +0x70 3, scaled 1 by 480 over the sprite's height);
/// - the two bands (red, sprite -5, 640 × 58 at y 0 and 422, +0x70 2)
pub fn build_profile_frame(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    let i = icon(m, e, s, BACKDROP_NAME, 0x120, buf);
    vcall_v(m, e, i, 0x50, PALE, buf);
    vcall_v(m, e, i, 0x138, [fb(460.0), fb(480.0), 0, 0], buf);
    place(m, e, i, fb(180.0), 0, buf);
    m.w32(i + 0x70, 0);
    e.call(m, STATE_ADD, &[s, i]);

    let i = icon(m, e, s, PICTURE_NAME, 0x120, buf);
    vcall_v(m, e, i, 0x138, [fb(180.0), fb(364.0), 0, 0], buf);
    place(m, e, i, 0, fb(58.0), buf);
    vcall_v(m, e, i, 0x50, [ONE; 4], buf);
    m.w32(i + 0x70, 2);
    m.w32(i + 0x110, 0x7c);
    e.call(m, STATE_ADD, &[s, i]);
    m.w32(s + 0x4ec, i);

    let i = icon(m, e, s, SIDE_NAME, 0x130, buf);
    m.w32(i + 0x120, ONE);
    m.w32(i + 0x124, ONE);
    m.w32(i + 0xc, SCALED_ICON_VTABLE);
    m.w32(i + 0x110, 0x28);
    let r = e.call(m, SPRITE_RECT, &[s, 0x28]);
    let w = m.r32(r.wrapping_add(4));
    vcall_v(m, e, i, 0x138, [w, fb(364.0), 0, 0], buf);
    vcall_v(m, e, i, 0x50, DARK, buf);
    let r = e.call(m, SPRITE_RECT, &[s, 0x28]);
    let x = F(180.0) - rf(m, r.wrapping_add(4));
    place(m, e, i, x.0.to_bits(), fb(58.0), buf);
    m.w32(i + 0x70, 3);
    let r = e.call(m, SPRITE_RECT, &[s, 0x28]);
    let scale = F(480.0) / rf(m, r.wrapping_add(8));
    m.w32(i + 0x120, ONE);
    m.w32(i + 0x124, scale.0.to_bits());
    e.call(m, STATE_ADD, &[s, i]);

    let mut answer = 0;
    for y in [0, fb(422.0)] {
        let i = icon(m, e, s, BAND_NAME, 0x120, buf);
        vcall_v(m, e, i, 0x50, [ONE, 0, 0, 0], buf);
        m.w32(i + 0x110, 0xffff_fffb);
        place(m, e, i, 0, y, buf);
        vcall_v(m, e, i, 0x138, [fb(640.0), fb(58.0), 0, 0], buf);
        m.w32(i + 0x70, 2);
        answer = e.call(m, STATE_ADD, &[s, i]);
    }
    answer
}

// ---- the records panel (0x282d30)

/// The records panel's calls out: the career's rider record (0x164318) and bio line (0x164330), the menu's
/// constructor (0x26d3b8), the single-line text's (0x2b1138), an event's name (0x16b650), a record's value
/// (0x163bb8), a medal (0x163e28), a child placed (0x28e020, f12 f13), a wide copy (0x17ccb8), a number written
/// (0x1a6ba0), the format (0x2fca28), a group's widget added (0x2762e0)
pub const RIDER_RECORD: u32 = 0x164318;
pub const RIDER_BIO_LINE: u32 = 0x164330;
pub const MENU_CTOR: u32 = 0x26d3b8;
pub const TEXT_CTOR: u32 = 0x2b1138;
pub const EVENT_NAME: u32 = 0x16b650;
pub const RECORD_VALUE: u32 = 0x163bb8;
pub const MEDAL: u32 = 0x163e28;
pub const PLACE_CHILD: u32 = 0x28e020;
pub const WSTR_COPY: u32 = 0x17ccb8;
pub const WRITE_NUMBER: u32 = 0x1a6ba0;
pub const FORMAT: u32 = 0x2fca28;
pub const GROUP_ADD: u32 = 0x2762e0;
/// The career, its counts by rider (0x1c4 apart) for the left and right columns, the events in order (10), the
/// dash shown for none, the count's format
pub const CAREER: u32 = 0x32b390;
pub const COUNTS_LEFT: u32 = 0x32b58c;
pub const COUNTS_RIGHT: u32 = 0x32b5c0;
pub const RECORD_EVENTS: u32 = 0x3af8d0;
pub const DASH: u32 = 0x3af950;
pub const COUNT_FORMAT: u32 = 0x3af970;
const TEXT_ICON_VTABLE: u32 = 0x3ae690;
const MENU_LIST_VTABLE: u32 = 0x3b7fe0;

fn menu_font(m: &Mem) -> u32 {
    m.r32(m.r32(crate::fe3dstate::GAPP) + 0x60)
}

/// A single-line text (0x130 bytes) made for the screen in `parent`
fn text(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32, parent: u32, font: u32) -> u32 {
    let t = e.call(m, ALLOC, &[name, 0x130, 0]);
    if t != 0 {
        e.call(m, FILL, &[t, 0xdead_c0de, 0x130]);
    }
    e.call(m, TEXT_CTOR, &[t, s, parent, font])
}

/// The entry's child at `at` replaced: the old one destroyed (its +8 with 3)
fn set_child(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, c: u32) {
    let old = m.r32(w + at);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(w + at, c);
}

/// A text entry (0x150 bytes, made in place) in `parent` with a text (0x3af4c0) as its first child
fn text_entry(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32, parent: u32, font: u32) -> u32 {
    let w = e.call(m, ALLOC, &[name, 0x150, 0]);
    if w != 0 {
        e.call(m, FILL, &[w, 0xdead_c0de, 0x150]);
    }
    crate::fe_widgets::text_entry_init(m, e, w, s, parent);
    let t = text(m, e, s, 0x3af4c0, parent, font);
    set_child(m, e, w, 0x110, t);
    w
}

/// A text icon (a text entry with the text icon's table and an icon, 15 x 15, as its second child)
fn text_icon(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32, parent: u32, font: u32, buf: u32) -> u32 {
    let w = text_entry(m, e, s, name, parent, font);
    m.w32(w + 0xc, TEXT_ICON_VTABLE);
    let i = e.call(m, ALLOC, &[0x3af4f0, 0x120, 0]);
    if i != 0 {
        e.call(m, FILL, &[i, 0xdead_c0de, 0x120]);
    }
    icon_init(m, e, i, s, parent);
    put(m, buf + 0x30, [fb(10.0), fb(10.0), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, i, 0x138, &[buf + 0x30]);
    set_child(m, e, w, 0x114, i);
    put(m, buf + 0x20, [fb(15.0), fb(15.0), 0, 0]);
    vcall(m, e, i, 0x138, &[buf + 0x20]);
    w
}

/// The medal icon placed by the career's medal (0x163e28 with `kind`): 0 sprite 0x3a, 1 0x39, 2 0x38, any other
/// `other` with the icon hidden
fn medal(m: &mut Mem, e: &mut dyn FeEnv, w: u32, kind: u32, id: u32, other: u32) {
    let r = e.call(m, MEDAL, &[CAREER, kind, id]);
    let (sprite, hide) = match r {
        0 => (0x3a, false),
        1 => (0x39, false),
        2 => (0x38, false),
        _ => (other, true),
    };
    e.call(m, PLACE_CHILD, &[w, sprite, fb(20.0), fb(15.0)]);
    if hide {
        let i = m.r32(w + 0x114);
        vcall(m, e, i, 0x28, &[0]);
    }
}

/// `FE_BuildRiderRecordsScreen` 0x282d30(s; `buf` 0xa0 bytes, laid out as the game's frame from +0x30): the records
/// panel in the group at +0x4f8 for the screen's rider (+0x2e): two lists (0x190 bytes each, the menu's
/// constructor with (1, 2, 0)); for each of the 10 events (0x3af8d0) a row in the left list: the event's name
/// (0.8, red, white outline) and a text icon with its record (events 6 and 7: the career's value, the name 0xddd if
/// it has one, else the dash, the icon hidden) or its medal and count (the left counts; the dash for none); for
/// the others also a row in the right list with the second medal and count (formatted, at the screen's +0x364);
/// the lists placed at (320, 150) and (470, 150) with a gap of 6; six headings; the rank list (+0x504: ten lines 0x17.. in the second
/// font) with its marker (+0x508)
pub fn build_records(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let b = buf + 0x30;
    let grp = m.r32(s + 0x4f8);
    let rider = m.r8(s + 0x2e) as i8 as i32 as u32;
    let rec = e.call(m, RIDER_RECORD, &[CAREER, rider]);
    let first = m.r8(rec) as i8 as i32 as u32;
    let mk_list = |m: &mut Mem, e: &mut dyn FeEnv, name: u32| {
        let l = e.call(m, ALLOC, &[name, 0x190, 0]);
        if l != 0 {
            e.call(m, FILL, &[l, 0xdead_c0de, 0x190]);
        }
        e.call(m, MENU_CTOR, &[l, s, grp, 1, 2, 0])
    };
    let l1 = mk_list(m, e, 0x3af8f8);
    let l2 = mk_list(m, e, 0x3af908);
    let k = e.call(m, RIDER_BIO_LINE, &[CAREER, first]);
    let red = [ONE, 0, 0, 0];
    for i in 0..10u32 {
        let id = m.r32(RECORD_EVENTS + 4 * i);
        let font = menu_font(m);
        let e1 = text_entry(m, e, s, 0x3af918, l1, font);
        let e2 = text_icon(m, e, s, 0x3af930, l1, font, b);
        let font = menu_font(m);
        let t3 = text(m, e, s, 0x3af940, l1, font);
        let name = e.call(m, EVENT_NAME, &[CAREER, id]);
        vcall(m, e, t3, 0x150, &[name]);
        m.w32(t3 + 0x118, 0x3f4c_cccd);
        m.w32(t3 + 0x114, 0);
        put(m, b, red);
        vcall(m, e, t3, 0x110, &[b]);
        put(m, b, [ONE; 4]);
        vcall(m, e, t3, 0x118, &[b]);
        set_child(m, e, e1, 0x110, t3);
        set_child(m, e, e1, 0x114, e2);
        m.w32(e1 + 0x11c, 5);
        m.w32(e1 + 0x130, 6);
        m.w32(e2 + 0x11c, 6);
        m.w32(e2 + 0x130, 6);
        put(m, e2 + 0x120, [fb(70.0), 0, 0, 0]);
        let t2 = m.r32(e2 + 0x110);
        m.w32(t2 + 0x114, 0);
        let special = id.wrapping_sub(6) < 2;
        if special {
            let v = if id == 6 {
                e.call(m, RECORD_VALUE, &[CAREER, 3, 6])
            } else {
                e.call(m, RECORD_VALUE, &[CAREER, 1, id])
            };
            vcall(m, e, t3, 0x100, &[v]);
            let t2 = m.r32(e2 + 0x110);
            if m.r32(t3 + 0xa8) != 0 {
                vcall(m, e, t2, 0x150, &[0xddd]);
            } else {
                vcall(m, e, t2, 0x158, &[DASH]);
            }
            e.call(m, PLACE_CHILD, &[e2, 0x38, fb(20.0), fb(15.0)]);
            let i2 = m.r32(e2 + 0x114);
            vcall(m, e, i2, 0x28, &[0]);
        } else {
            medal(m, e, e2, 4, id, 0x38);
            let at = s + 0x58 + 0x3c * id;
            let cnt = m.r32(COUNTS_LEFT + k.wrapping_mul(0x1c4) + 4 * id);
            if (cnt as i32) > 0 {
                e.call(m, WRITE_NUMBER, &[cnt, at]);
            } else {
                e.call(m, WSTR_COPY, &[at, DASH]);
            }
            let t2 = m.r32(e2 + 0x110);
            vcall(m, e, t2, 0x148, &[at]);
            vcall(m, e, t3, 0x100, &[1]);
        }
        let t2 = m.r32(e2 + 0x110);
        m.w32(t2 + 0x118, 0x3f4c_cccd);
        put(m, b, red);
        vcall(m, e, t2, 0x50, &[b]);
        if !special {
            let font = menu_font(m);
            let e3 = text_icon(m, e, s, 0x3af958, l2, font, b);
            medal(m, e, e3, 5, id, 0x3a);
            m.w32(e3 + 0x11c, 6);
            m.w32(e3 + 0x130, 6);
            put(m, e3 + 0x120, [fb(70.0), 0, 0, 0]);
            let t = m.r32(e3 + 0x110);
            m.w32(t + 0x114, 0);
            let at = s + 0x364 + 0x1e * id;
            let cnt = m.r32(COUNTS_RIGHT + k.wrapping_mul(0x1c4) + 4 * id);
            if (cnt as i32) > 0 {
                e.call(m, FORMAT, &[at, COUNT_FORMAT, cnt]);
            } else {
                let d = m.r32(DASH);
                for (n, x) in d.to_le_bytes().into_iter().enumerate() {
                    m.w8(at + n as u32, x);
                }
            }
            let t = m.r32(e3 + 0x110);
            vcall(m, e, t, 0x158, &[at]);
            let t = m.r32(e3 + 0x110);
            m.w32(t + 0x118, 0x3f4c_cccd);
            put(m, b, red);
            vcall(m, e, t, 0x50, &[b]);
            vcall(m, e, l2, 0x148, &[e3]);
        }
        vcall(m, e, l1, 0x148, &[e1]);
    }
    for (l, x) in [(l1, 320.0), (l2, 470.0)] {
        m.w32(l + 0x150, 1);
        put(m, b, [fb(x), fb(150.0), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[l, b]);
        m.w32(l + 0x154, 2);
        m.w32(l + 0x15c, fb(6.0));
        e.call(m, GROUP_ADD, &[grp, l]);
    }
    let heads = [(0xc70u32, 320.0, 110.0), (0xc72, 320.0, 125.0), (0xc73, 390.0, 125.0), (0xc71, 470.0, 110.0), (0xc74, 470.0, 125.0), (0xc73, 540.0, 125.0)];
    for (sid, x, y) in heads {
        let font = menu_font(m);
        let t = text(m, e, s, 0x3af978, grp, font);
        vcall(m, e, t, 0x150, &[sid]);
        put(m, b, red);
        vcall(m, e, t, 0x50, &[b]);
        m.w32(t + 0x114, 0);
        put(m, b, [fb(x), fb(y), 0, ONE]);
        e.call(m, WIDGET_PLACE, &[t, b]);
        m.w32(t + 0x118, 0x3f4c_cccd);
        e.call(m, GROUP_ADD, &[grp, t]);
    }
    // the rank list
    put(m, b, [fb(120.0), fb(20.0), 0, 0]);
    let ml = e.call(m, ALLOC, &[0x3af988, 0x1b0, 0]);
    if ml != 0 {
        e.call(m, FILL, &[ml, 0xdead_c0de, 0x1b0]);
    }
    e.call(m, MENU_CTOR, &[ml, s, grp, 0, 0, 1]);
    let words = [(0x154u32, 1u32), (0x194, 0), (0xc, MENU_LIST_VTABLE), (0x198, 0), (0x150, 1), (0x190, 1), (0x19c, 0), (0x1a0, 0), (0x13c, 0), (0x160, 0), (0x15c, fb(7.0))];
    for (at, v) in words {
        m.w32(ml + at, v);
    }
    let v = [m.r32(b), m.r32(b + 4), m.r32(b + 8), m.r32(b + 12)];
    put(m, ml + 0x170, v);
    vcall_class(m, e, MENU_LIST_VTABLE, ml, 0xf8, &[1]);
    let half = 0x3f00_0000;
    put(m, b + 0x10, [ONE, half, half, half]);
    for i in 0..10u32 {
        let font = m.r32(m.r32(crate::fe3dstate::GAPP) + 0x5c);
        let t = text(m, e, s, 0x3af998, ml, font);
        put(m, b + 0x20, red);
        vcall(m, e, t, 0x108, &[b + 0x20]);
        vcall(m, e, t, 0x110, &[b + 0x10]);
        vcall(m, e, t, 0x50, &[b + 0x10]);
        m.w32(t, i);
        m.w32(t + 0x114, 0);
        m.w32(t + 0x118, 0x3f33_3333);
        m.w32(t + 0x70, 1);
        vcall(m, e, t, 0x150, &[0x17 + i]);
        vcall(m, e, ml, 0x148, &[t]);
    }
    // its marker
    let mk = e.call(m, ALLOC, &[0x3af9a8, 0x120, 0]);
    if mk != 0 {
        e.call(m, FILL, &[mk, 0xdead_c0de, 0x120]);
    }
    icon_init(m, e, mk, s, ml);
    put(m, b + 0x30, [fb(10.0), fb(10.0), 0, 0]);
    vcall_class(m, e, ICON_VTABLE, mk, 0x138, &[b + 0x30]);
    m.w32(mk + 0x110, 0xffff_fffb);
    put(m, b + 0x20, [0x3f33_3333, 0x3fc8_0000, 0x3f9f_fcb9, 0x3e89_930c]);
    vcall(m, e, mk, 0x50, &[b + 0x20]);
    put(m, b + 0x20, [fb(180.0), fb(30.0), 0, 0]);
    vcall(m, e, mk, 0x138, &[b + 0x20]);
    m.w32(mk + 0x70, 0xffff_fffe);
    e.call(m, GROUP_ADD, &[grp, mk]);
    put(m, b + 0x20, [fb(45.0), fb(100.0), 0, ONE]);
    m.w32(s + 0x508, mk);
    e.call(m, WIDGET_PLACE, &[ml, b + 0x20]);
    e.call(m, GROUP_ADD, &[grp, ml]);
    m.w32(s + 0x504, ml);
}

// ---- the bio panel (0x27f600)

/// Each bio line's first string (by the rider's bio line, 15 strings apart; none past 11)
const BIO_BASES: [u32; 12] = [0xbac, 0xbbb, 0xbca, 0xbd9, 0xbe8, 0xbf7, 0xc06, 0xc15, 0xc24, 0xc33, 0xc42, 0xc51];

/// `FE_BuildRiderBioScreen` 0x27f600(s; `buf` 0x10 bytes): the bio panel in the group at +0x4f0 for the screen's
/// rider (+0x2e): a list (0x190 bytes, the menu's constructor with (1, 2, 0)) of 15 rows, each a text entry (0x150
/// bytes) holding a heading (0x24a + row) and the rider's line (the bio line's first string + row), both red at 0.8;
/// the list at (350, 111) with a gap of 4, added to the group, the group shown
pub fn build_bio(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let rider = m.r8(s + 0x2e) as i8 as i32 as u32;
    let grp = m.r32(s + 0x4f0);
    let rec = e.call(m, RIDER_RECORD, &[CAREER, rider]);
    let first = m.r8(rec) as i8 as i32 as u32;
    let l = e.call(m, ALLOC, &[0x3af868, 0x190, 0]);
    if l != 0 {
        e.call(m, FILL, &[l, 0xdead_c0de, 0x190]);
    }
    let list = e.call(m, MENU_CTOR, &[l, s, grp, 1, 2, 0]);
    let k = e.call(m, RIDER_BIO_LINE, &[CAREER, first]);
    let base = if k < 12 { BIO_BASES[k as usize] } else { 0 };
    let red = [ONE, 0, 0, 0];
    for j in 0..15u32 {
        let font = menu_font(m);
        let w = text_entry(m, e, s, 0x3af878, grp, font);
        let font = menu_font(m);
        let t2 = text(m, e, s, 0x3af890, grp, font);
        let t1 = m.r32(w + 0x110);
        vcall(m, e, t1, 0x150, &[0x24a + j]);
        vcall(m, e, t2, 0x150, &[base.wrapping_add(j)]);
        m.w32(w + 0x11c, 5);
        let old = m.r32(w + 0x114);
        if old != 0 {
            vcall(m, e, old, 8, &[3]);
        }
        m.w32(w + 0x130, 6);
        m.w32(w + 0x114, t2);
        put(m, buf, red);
        vcall(m, e, w, 0x50, &[buf]);
        put(m, buf, red);
        vcall(m, e, t2, 0x50, &[buf]);
        let t1 = m.r32(w + 0x110);
        m.w32(t2 + 0x118, 0x3f4c_cccd);
        m.w32(t1 + 0x114, 0);
        m.w32(t2 + 0x114, 0);
        m.w32(t1 + 0x118, 0x3f4c_cccd);
        vcall(m, e, list, 0x148, &[w]);
    }
    m.w32(list + 0x150, 1);
    put(m, buf, [fb(350.0), fb(111.0), 0, ONE]);
    e.call(m, WIDGET_PLACE, &[list, buf]);
    m.w32(list + 0x154, 2);
    m.w32(list + 0x15c, fb(4.0));
    e.call(m, GROUP_ADD, &[grp, list]);
    vcall(m, e, grp, 0x20, &[1]);
}

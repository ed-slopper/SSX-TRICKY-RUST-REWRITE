//! The front end's widget set up as the game inlines it into its screen builders (the character select's, the custom
//! board's, ...): `cFE3DThing`'s and `cFE3DWidget`'s constructors (0x267f58 has the same body out of line) and the
//! parts of the group, entry, text entry, progress bar and icon constructors the builders write in place. Each writes
//! the words the game writes, leaving the rest of the object as it was; the one call out is the widget's colour
//! update (0x2ad988).

use crate::fe3dstate::FeEnv;
use crate::pf::Mem;

/// The widget classes' vtables
pub const THING_VTABLE: u32 = 0x3b5a00;
pub const WIDGET_VTABLE: u32 = 0x3b5e18;
pub const GROUP_VTABLE: u32 = 0x3a9e98;
pub const ENTRY_VTABLE: u32 = 0x3b6bb0;
pub const TEXT_ENTRY_VTABLE: u32 = 0x3ae7f0;
pub const PROGRESS_VTABLE: u32 = 0x3b6cf8;
pub const ICON_VTABLE: u32 = 0x3b6f88;
pub const BUFFERED_TEXT_VTABLE: u32 = 0x3b7238;
/// 0x2ad988 a widget's colour update, 0x2abfb0 a list's set up, 0x2762e0 a group's widget added (group, widget)
pub const UPDATE_COLOR: u32 = 0x2ad988;
pub const LIST_INIT: u32 = 0x2abfb0;
pub const GROUP_ADD: u32 = 0x2762e0;

const ONE: u32 = 0x3f80_0000;
const HALF: u32 = 0x3f00_0000;

fn fill(m: &mut Mem, at: u32, from: u32, to: u32, v: u32) {
    let mut o = from;
    while o < to {
        m.w32(at.wrapping_add(o), v);
        o += 4;
    }
}

/// `cFE3DThing` then `cFE3DWidget` (the owning screen, the parent widget): the thing's links (+0 -1, +4 and +8 the
/// object), its flags and vectors, the highlight colour (1, 0.953, 0.729, 0.106; +0xac), white (+0xbc), the two
/// half-grey colours (+0xcc, +0xdc), the widget's vtable, owner (+0xec) and parent (+0xf0), the position (0, 0, 0, 1;
/// +0x100), then its colour update
pub fn widget_init(m: &mut Mem, e: &mut dyn FeEnv, o: u32, owner: u32, parent: u32) {
    m.w32(o, u32::MAX);
    m.w32(o.wrapping_add(4), o);
    m.w32(o.wrapping_add(8), o);
    m.w32(o.wrapping_add(0xc), THING_VTABLE);
    m.w32(o.wrapping_add(0x10), 0);
    m.w32(o.wrapping_add(0x14), 1);
    fill(m, o, 0x18, 0x28, 0);
    m.w32(o.wrapping_add(0x28), 1);
    fill(m, o, 0x2c, 0x34, 0);
    fill(m, o, 0x40, 0x60, 0);
    fill(m, o, 0x60, 0x70, ONE);
    fill(m, o, 0x70, 0x78, 0);
    fill(m, o, 0x80, 0x8c, 0);
    m.w32(o.wrapping_add(0x8c), ONE);
    fill(m, o, 0x90, 0x9c, 0);
    for (k, v) in [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687].iter().enumerate() {
        m.w32(o.wrapping_add(0xac + 4 * k as u32), *v);
    }
    fill(m, o, 0xbc, 0xcc, ONE);
    for base in [0xcc, 0xdc] {
        m.w32(o.wrapping_add(base), ONE);
        fill(m, o, base + 4, base + 0x10, HALF);
    }
    m.w32(o.wrapping_add(0xc), WIDGET_VTABLE);
    m.w32(o.wrapping_add(0xa0), 0);
    m.w32(o.wrapping_add(0xa4), 0);
    m.w32(o.wrapping_add(0xa8), 1);
    m.w32(o.wrapping_add(0xec), owner);
    m.w32(o.wrapping_add(0xf0), parent);
    fill(m, o, 0x100, 0x10c, 0);
    m.w32(o.wrapping_add(0x10c), ONE);
    e.call(m, UPDATE_COLOR, &[o]);
}

/// A widget group (0x140 bytes): the widget, its vtable and its list (+0x110) set up
pub fn group_init(m: &mut Mem, e: &mut dyn FeEnv, o: u32, owner: u32, parent: u32) {
    widget_init(m, e, o, owner, parent);
    m.w32(o.wrapping_add(0xc), GROUP_VTABLE);
    e.call(m, LIST_INIT, &[o.wrapping_add(0x110)]);
}

/// A text entry (0x150 bytes): the widget, then the entry's (+0x110 and +0x114 none, +0x118 15.0, +0x11c 2, the
/// colours (0, 0, 0, 1) at +0x120 and +0x140, +0x130 6) and the text entry's vtable
pub fn text_entry_init(m: &mut Mem, e: &mut dyn FeEnv, o: u32, owner: u32, parent: u32) {
    widget_init(m, e, o, owner, parent);
    m.w32(o.wrapping_add(0x110), 0);
    m.w32(o.wrapping_add(0xc), ENTRY_VTABLE);
    m.w32(o.wrapping_add(0x114), 0);
    m.w32(o.wrapping_add(0x11c), 2);
    m.w32(o.wrapping_add(0x118), 15.0f32.to_bits());
    for base in [0x120, 0x140] {
        fill(m, o, base, base + 0xc, 0);
        m.w32(o.wrapping_add(base + 0xc), ONE);
    }
    m.w32(o.wrapping_add(0x130), 6);
    m.w32(o.wrapping_add(0xc), TEXT_ENTRY_VTABLE);
}

/// A progress bar (0x190 bytes): the widget, then the bar's colours (+0x110 white, +0x120 white with no alpha, +0x130
/// (1, 0.2, 0.2, 0.2), +0x140 (1, 1, 0, 0)), its range (+0x150 100, +0x154 0, +0x158 and +0x15c -9999), +0x160 0,
/// +0x164 1, +0x168 0, the three sprites (+0x16c..+0x174) none, +0x178..+0x184 1, +0x188 and +0x18c 0, and its vtable
pub fn progress_init(m: &mut Mem, e: &mut dyn FeEnv, o: u32, owner: u32, parent: u32) {
    widget_init(m, e, o, owner, parent);
    let q = 0.2f32.to_bits();
    for (off, v) in [
        (0x110, ONE),
        (0x114, ONE),
        (0x118, ONE),
        (0x11c, 0),
        (0x120, ONE),
        (0x124, ONE),
        (0x128, ONE),
        (0x12c, 0),
        (0x130, ONE),
        (0x134, q),
        (0x138, q),
        (0x13c, q),
        (0x140, ONE),
        (0x144, ONE),
        (0x148, 0),
        (0x14c, 0),
        (0x158, 0xc61c_3c00),
        (0x15c, 0xc61c_3c00),
        (0xc, PROGRESS_VTABLE),
        (0x150, 100.0f32.to_bits()),
        (0x154, 0),
        (0x160, 0),
        (0x164, ONE),
        (0x168, 0),
        (0x16c, u32::MAX),
        (0x170, u32::MAX),
        (0x174, u32::MAX),
        (0x178, ONE),
        (0x17c, ONE),
        (0x180, ONE),
        (0x184, ONE),
        (0x188, 0),
        (0x18c, 0),
    ] {
        m.w32(o.wrapping_add(off), v);
    }
}

/// An icon (0x120 bytes): the widget, its vtable, its sprite (+0x110) none and +0x114 0
pub fn icon_init(m: &mut Mem, e: &mut dyn FeEnv, o: u32, owner: u32, parent: u32) {
    widget_init(m, e, o, owner, parent);
    m.w32(o.wrapping_add(0xc), ICON_VTABLE);
    m.w32(o.wrapping_add(0x110), u32::MAX);
    m.w32(o.wrapping_add(0x114), 0);
}

/// A virtual call through a class's vtable named in the code rather than the object's (the builders' size and text
/// calls on the objects they just made): the adjust at `vt + off`, the function at `vt + off + 4`
pub fn vcall_class(m: &mut Mem, e: &mut dyn FeEnv, vt: u32, obj: u32, off: u32, args: &[u32]) -> u32 {
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

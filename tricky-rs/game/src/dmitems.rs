//! The debug menu's item kinds (board row G9a4b): the bool, enum, sub-menu, pop, command, null, space, int, float,
//! angle, string, colour, expand and RGB items and the error menu, on the framework of [`crate::debugmenu`]. On the game's bytes ([`Mem`]); the menus' virtuals, the label's
//! measure and draw (G9a4c) and the C library are [`TextBoxEnv`] calls by address. Checked in the function runner
//! (`tests/dmitems.rs`).
//!
//! Every kind keeps a `cMenuItem` (+0 .. +0xc) and adds from +0x10. A label is measured and drawn by
//! `MenuItem_MeasureLabel` 0x18cc78(out, item, title, label, value) and `MenuItem_DrawLabel` 0x18d0c8(item, box,
//! title, label, value): a title with a value beside it, or a lone label. A menu's +0x18 virtual runs a command
//! (item, its argument) and its +0x20 hears that an item's value changed (item).
//!
//! Where the game builds on its stack (boxes, a formatted value), the port uses `scratch` (STANDIN: the place, not
//! the bytes).

use crate::debugmenu::{item_buttons, item_flag8, item_handle_input, item_key, item_new, manager_pop, manager_push};
use crate::debugmenu::{menu_add_item, menu_clear, menu_current_item, menu_enter, menu_leave, menu_new, menu_remove_item};
use crate::debugmenu::{menu_select_next, menu_select_prev, menu_set_selection, menu_skip_unselectable};
use crate::ps2f::F;
use crate::pf::Mem;
use crate::textbox::TextBoxEnv;

pub const BOOL_VTABLE: u32 = 0x389e00;
pub const ENUM_VTABLE: u32 = 0x389dc0;
pub const SUB_VTABLE: u32 = 0x389d80;
pub const POP_VTABLE: u32 = 0x389d40;
pub const COMMAND_VTABLE: u32 = 0x389d00;
pub const NULL_VTABLE: u32 = 0x389cc0;
pub const SPACE_VTABLE: u32 = 0x389c80;
pub const INT_VTABLE: u32 = 0x389c40;
/// An enum's value with no name ("?"), the int's widest value and its format
pub const ENUM_UNKNOWN: u32 = 0x389978;
pub const INT_WIDEST: u32 = 0x389988;
pub const INT_FORMAT: u32 = 0x389998;

pub const MEASURE_LABEL: u32 = 0x18cc78;
pub const DRAW_LABEL: u32 = 0x18d0c8;
pub const SPRINTF: u32 = 0x2fca28;

pub const KEY_LEFT: u32 = 0x4b00;
pub const KEY_RIGHT: u32 = 0x4d00;
pub const KEY_UP: u32 = 0x4800;
pub const KEY_DOWN: u32 = 0x5000;
pub const KEY_SELECT: u32 = 0xd;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
/// A virtual of the item's menu (+4; its table +0x114)
fn menu_call(m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32, off: u32, args: &[u32]) -> u32 {
    let menu = m.r32(item + 4);
    let vt = m.r32(menu + 0x114);
    let me = menu.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[])
}
/// The item's value changed: its menu's +0x20
fn changed(m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32) {
    menu_call(m, env, item, 0x20, &[item]);
}
fn copy4(m: &mut Mem, to: u32, from: u32) {
    for k in 0..4 {
        m.w32(to + 4 * k, m.r32(from + 4 * k));
    }
}
/// The label drawn in a copy of `rect` (at `scratch`)
fn draw_label(m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32, rect: u32, title: u32, label: u32, value: u32, scratch: u32) {
    copy4(m, scratch, rect);
    call(m, env, DRAW_LABEL, &[item, scratch, title, label, value]);
}
/// The base's answer to the key, and whether the item goes on to its own keys (not flag 8, no answer)
fn base_input(m: &mut Mem, this: u32) -> (u32, bool) {
    let r = item_handle_input(m, this);
    (r, item_flag8(m, this) == 0 && r == 0)
}

/// `cBoolMenuItem::cBoolMenuItem` 0x189948(this, label, value, title, on, off): +0x10 the title, +0x14 / +0x18 the
/// words for true and false, +0x1c the bool; `this`
pub fn bool_new(m: &mut Mem, this: u32, label: u32, value: u32, title: u32, on: u32, off: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, title);
    m.w32(this + 0x14, on);
    m.w32(this + 0x18, off);
    m.w32(this + 0x1c, value);
    m.w32(this + 0xc, BOOL_VTABLE);
    this
}

/// `cBoolMenuItem::HandleInput` 0x1899c0(this): select (or, with both words, left or right) flips the bool
pub fn bool_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    let both = m.r32(this + 0x14) != 0 && m.r32(this + 0x18) != 0;
    let side = both && (item_key(m, this) == KEY_LEFT || item_key(m, this) == KEY_RIGHT);
    if side || item_key(m, this) == KEY_SELECT {
        let v = m.r32(this + 0x1c);
        m.w32(v, m.r32(v) ^ 1);
        changed(m, env, this);
    }
    r
}

/// `cBoolMenuItem::Measure` 0x189a90(out, this): with both words the wider of the title beside each; else the title
/// alone
pub fn bool_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32, scratch: u32) -> u32 {
    let (a, b, t) = (scratch, scratch + 0x10, scratch + 0x20);
    if m.r32(this + 0x14) != 0 && m.r32(this + 0x18) != 0 {
        call(m, env, MEASURE_LABEL, &[t, this, m.r32(this + 0x10), 0, m.r32(this + 0x14)]);
        copy4(m, a, t);
        call(m, env, MEASURE_LABEL, &[t, this, m.r32(this + 0x10), 0, m.r32(this + 0x18)]);
        copy4(m, b, t);
        let wide = if (m.r32(b + 8) as i32) < m.r32(a + 8) as i32 { a } else { b };
        copy4(m, out, wide);
    } else {
        call(m, env, MEASURE_LABEL, &[out, this, 0, m.r32(this + 0x10), 0]);
    }
    out
}

/// `cBoolMenuItem::Draw` 0x189b98(this, rect): the title beside the bool's word, or the title alone
pub fn bool_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let (on, off) = (m.r32(this + 0x14), m.r32(this + 0x18));
    if on != 0 && off != 0 {
        let word = if m.r32(m.r32(this + 0x1c)) == 0 { off } else { on };
        draw_label(m, env, this, rect, m.r32(this + 0x10), 0, word, scratch);
    } else {
        draw_label(m, env, this, rect, 0, m.r32(this + 0x10), 0, scratch);
    }
}

fn enum_zero(m: &mut Mem, this: u32) {
    m.w32(this + 0x10, 0);
    m.w32(this + 0xc, ENUM_VTABLE);
    m.w32(this + 0x1c, 0);
    m.w32(this + 0x14, 0);
}

/// `cEnumMenuItem::cEnumMenuItem` 0x189c38(this): no label, title, values or list; `this`
pub fn enum_new0(m: &mut Mem, this: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    enum_zero(m, this);
    this
}

/// `cEnumMenuItem::cEnumMenuItem` 0x189c80(this, label)
pub fn enum_new1(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, label);
    enum_zero(m, this);
    this
}

/// `cEnumMenuItem::cEnumMenuItem` 0x189cc0(this, label, value, title, count, list): +0x10 the title, +0x14 the
/// count, +0x18 the list (8 bytes each: the name, the value), +0x1c the value
pub fn enum_new(m: &mut Mem, this: u32, label: u32, value: u32, title: u32, count: u32, list: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, title);
    m.w32(this + 0x1c, value);
    m.w32(this + 0x14, count);
    m.w32(this + 0x18, list);
    m.w32(this + 0xc, ENUM_VTABLE);
    this
}

/// 0x189d38(this, list): the list and its count (up to the entry with no name)
pub fn enum_set_list(m: &mut Mem, this: u32, list: u32) {
    m.w32(this + 0x18, list);
    m.w32(this + 0x14, 0);
    let mut n = 0u32;
    while m.r32(list + 8 * n) != 0 {
        n += 1;
        m.w32(this + 0x14, n);
    }
}

/// `cEnumMenuItem::HandleInput` 0x189d78(this): select runs the menu's command (with 0); left / up and right / down
/// step the value round the list (a value not in the list: to the first or the last)
pub fn enum_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    if item_key(m, this) == KEY_SELECT {
        return menu_call(m, env, this, 0x18, &[this, 0]);
    }
    let n = m.r32(this + 0x14);
    let mut i = 0u32;
    while i != n {
        let list = m.r32(this + 0x18);
        if m.r32(m.r32(this + 0x1c)) == m.r32(list + 8 * i + 4) {
            break;
        }
        i += 1;
    }
    let back = item_key(m, this) == KEY_LEFT || item_key(m, this) == KEY_UP;
    let on = !back && (item_key(m, this) == KEY_RIGHT || item_key(m, this) == KEY_DOWN);
    if !back && !on {
        return r;
    }
    let list = m.r32(this + 0x18);
    let at = if i == n {
        if back { 0 } else { n.wrapping_sub(1) }
    } else {
        let n = m.r32(this + 0x14) as i32;
        assert!(n != 0, "an enum with no values (the game's `div` traps)");
        let s = if back { (i as i32).wrapping_add(n).wrapping_sub(1) } else { (i as i32).wrapping_add(1) };
        s.wrapping_rem(n) as u32
    };
    let v = m.r32(list.wrapping_add(at << 3).wrapping_add(4));
    m.w32(m.r32(this + 0x1c), v);
    changed(m, env, this);
    r
}

/// `cEnumMenuItem::Measure` 0x18a000(out, this): the widest of the title beside "?" and beside each name
pub fn enum_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32, scratch: u32) -> u32 {
    let (best, t) = (scratch, scratch + 0x10);
    call(m, env, MEASURE_LABEL, &[t, this, m.r32(this + 0x10), 0, ENUM_UNKNOWN]);
    copy4(m, best, t);
    let mut i = 0i32;
    while i < m.r32(this + 0x14) as i32 {
        let name = m.r32(m.r32(this + 0x18) + 8 * i as u32);
        call(m, env, MEASURE_LABEL, &[t, this, m.r32(this + 0x10), 0, name]);
        if (m.r32(best + 8) as i32) < m.r32(t + 8) as i32 {
            copy4(m, best, t);
        }
        i += 1;
    }
    copy4(m, out, best);
    out
}

/// `cEnumMenuItem::Draw` 0x18a118(this, rect): the title beside the value's name ("?" when it has none)
pub fn enum_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let n = m.r32(this + 0x14);
    let mut i = 0u32;
    while i != n {
        let e = m.r32(this + 0x18) + 8 * i;
        i += 1;
        if m.r32(m.r32(this + 0x1c)) == m.r32(e + 4) {
            draw_label(m, env, this, rect, m.r32(this + 0x10), 0, m.r32(e), scratch);
            return;
        }
    }
    draw_label(m, env, this, rect, m.r32(this + 0x10), 0, ENUM_UNKNOWN, scratch);
}

/// `cSubMenuItem::cSubMenuItem` 0x18a1d0(this, menu, label): +0x10 the label, +0x14 the menu it opens; `this`
pub fn sub_new(m: &mut Mem, this: u32, menu: u32, label: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, label);
    m.w32(this + 0x14, menu);
    m.w32(this + 0xc, SUB_VTABLE);
    this
}

fn manager_of(m: &Mem, this: u32) -> u32 {
    m.r32(m.r32(this + 4) + 4)
}

/// `cSubMenuItem::HandleInput` 0x18a228(this): select pushes its menu
pub fn sub_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if go && item_key(m, this) == KEY_SELECT {
        let mgr = manager_of(m, this);
        manager_push(m, env, mgr, m.r32(this + 0x14));
    }
    r
}

/// The label (+0x10) alone: the sub, pop, null items' measure (0x18a298, 0x18a3d8, 0x18a708)
pub fn label_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    call(m, env, MEASURE_LABEL, &[out, this, 0, m.r32(this + 0x10), 0]);
    out
}

/// The label (+0x10) alone: their draw (0x18a2d0, 0x18a410, 0x18a740)
pub fn label_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    draw_label(m, env, this, rect, 0, m.r32(this + 0x10), 0, scratch);
}

/// `cPopMenuItem::cPopMenuItem` 0x18a320(this, label); `this`
pub fn pop_new(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, label);
    m.w32(this + 0xc, POP_VTABLE);
    this
}

/// `cPopMenuItem::HandleInput` 0x18a368(this): select pops the menu
pub fn pop_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if go && item_key(m, this) == KEY_SELECT {
        let mgr = manager_of(m, this);
        manager_pop(m, env, mgr);
    }
    r
}

/// `cCommandMenuItem::cCommandMenuItem` 0x18a460(this): the table only; `this`
pub fn command_new0(m: &mut Mem, this: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0xc, COMMAND_VTABLE);
    this
}

/// `cCommandMenuItem::cCommandMenuItem` 0x18a498(this, label): no title, value or argument
pub fn command_new1(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, 0);
    m.w32(this + 0xc, COMMAND_VTABLE);
    m.w32(this + 0x18, 0);
    m.w32(this + 0x1c, 0);
    m.w32(this + 0x14, 0);
    this
}

/// `cCommandMenuItem::cCommandMenuItem` 0x18a4e0(this, label, title, argument): +0x10 the title, +0x14 a value
/// shown beside it (none), +0x18 the argument
pub fn command_new(m: &mut Mem, this: u32, label: u32, title: u32, arg: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, title);
    m.w32(this + 0x18, arg);
    m.w32(this + 0xc, COMMAND_VTABLE);
    m.w32(this + 0x14, 0);
    this
}

/// 0x18a538(this, title, argument)
pub fn command_set(m: &mut Mem, this: u32, title: u32, arg: u32) {
    m.w32(this + 0x10, title);
    m.w32(this + 0x18, arg);
    m.w32(this + 0x14, 0);
}

/// `cCommandMenuItem::HandleInput` 0x18a548(this): select runs the menu's command with the argument
pub fn command_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if go && item_key(m, this) == KEY_SELECT {
        return menu_call(m, env, this, 0x18, &[this, m.r32(this + 0x18)]);
    }
    r
}

/// `cCommandMenuItem::Measure` 0x18a5d0(out, this): the title beside its value, or alone
pub fn command_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    let v = m.r32(this + 0x14);
    if v != 0 {
        call(m, env, MEASURE_LABEL, &[out, this, m.r32(this + 0x10), 0, v]);
    } else {
        call(m, env, MEASURE_LABEL, &[out, this, 0, m.r32(this + 0x10), 0]);
    }
    out
}

/// `cCommandMenuItem::Draw` 0x18a628(this, rect)
pub fn command_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let v = m.r32(this + 0x14);
    if v != 0 {
        draw_label(m, env, this, rect, m.r32(this + 0x10), 0, v, scratch);
    } else {
        draw_label(m, env, this, rect, 0, m.r32(this + 0x10), 0, scratch);
    }
}

/// `cNullMenuItem::cNullMenuItem` 0x18a6b8(this, label); `this`
pub fn null_new(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, label);
    m.w32(this + 0xc, NULL_VTABLE);
    this
}

/// `cNullMenuItem::IsSelectable` 0x18a700 and `cSpaceMenuItem::IsSelectable` 0x18a7d8: never
pub fn never_selectable() -> u32 {
    0
}

/// `cSpaceMenuItem::cSpaceMenuItem` 0x18a790(this, height); `this`
pub fn space_new(m: &mut Mem, this: u32, height: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, height);
    m.w32(this + 0xc, SPACE_VTABLE);
    this
}

/// `cSpaceMenuItem::Measure` 0x18a7e0(out, this): its height only
pub fn space_measure(m: &mut Mem, out: u32, this: u32) -> u32 {
    let h = m.r32(this + 0x10);
    m.w32(out, 0);
    m.w32(out + 0xc, h);
    m.w32(out + 4, 0);
    m.w32(out + 8, 0);
    out
}

/// `cSpaceMenuItem::Draw` 0x18a818: nothing
pub fn space_draw() {}

fn int_clear(m: &mut Mem, this: u32) {
    for o in [0x24, 0x20, 0x2c, 0x28] {
        m.w32(this + o, 0);
    }
}

/// `cIntMenuItem::cIntMenuItem` 0x18a820(this, label): all clear; `this`
pub fn int_new1(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, label);
    for o in [0x10, 0x14, 0x18, 0x1c] {
        m.w32(this + o, 0);
    }
    m.w32(this + 0xc, INT_VTABLE);
    int_clear(m, this);
    this
}

/// `cIntMenuItem::cIntMenuItem` 0x18a878(this, label, value, min, max, title): +0x10 the title, +0x14 the int, +0x18
/// / +0x1c its range, +0x20 the frames the pad has been held, +0x24 the way it is held, +0x28 the step's fraction,
/// +0x2c an int kept equal
pub fn int_new(m: &mut Mem, this: u32, label: u32, value: u32, min: u32, max: u32, title: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, title);
    m.w32(this + 0x14, value);
    m.w32(this + 0x18, min);
    m.w32(this + 0x1c, max);
    m.w32(this + 0xc, INT_VTABLE);
    int_clear(m, this);
    this
}

/// `IntMenuItem_SetRange` 0x18a900(this, min, max): the int brought inside
pub fn int_set_range(m: &mut Mem, this: u32, min: u32, max: u32) {
    let v = m.r32(this + 0x14);
    m.w32(this + 0x1c, max);
    m.w32(this + 0x18, min);
    if v == 0 {
        return;
    }
    if (m.r32(v) as i32) < min as i32 {
        m.w32(v, min);
    }
    let max = m.r32(this + 0x1c);
    if (max as i32) < m.r32(v) as i32 {
        m.w32(v, max);
    }
}

/// `cIntMenuItem::HandleInput` 0x18a940(this): held left (−1) or right (+1, the pad's buttons 8 / 4) steps the int
/// faster the longer it is held (the step 1 + held² / 288 · 0.1, its whole part taken, the fraction kept), inside
/// the range; the kept int set; the menu told
pub fn int_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    let dir = if item_buttons(m, env, this, 8) != 0 { 0xffff_ffff } else { (item_buttons(m, env, this, 4) != 0) as u32 };
    if dir != m.r32(this + 0x24) {
        m.w32(this + 0x20, 0);
        m.w32(this + 0x28, crate::ps2f::F(1.0).0.to_bits());
        m.w32(this + 0x24, dir);
    } else {
        m.w32(this + 0x20, m.r32(this + 0x20).wrapping_add(1));
    }
    if dir == 0 {
        return r;
    }
    use crate::ps2f::F;
    let held = m.r32(this + 0x20) as i32;
    let t = held.wrapping_mul(held) / 0x120;
    let f = F(f32::from_bits(m.r32(this + 0x28))) + crate::animseq::cvt_s_w(t) * F(f32::from_bits(0x3dcc_cccd));
    let whole = crate::sndlib::cvt_w(f.0);
    let v = m.r32(this + 0x14);
    let n = m.r32(v).wrapping_add(dir.wrapping_mul(whole as u32));
    m.w32(v, n);
    m.w32(this + 0x28, (f - crate::animseq::cvt_s_w(whole)).0.to_bits());
    let min = m.r32(this + 0x18);
    if (n as i32) < min as i32 {
        m.w32(v, min);
    }
    let max = m.r32(this + 0x1c);
    if (max as i32) < m.r32(v) as i32 {
        m.w32(v, max);
    }
    let kept = m.r32(this + 0x2c);
    if kept != 0 {
        m.w32(kept, m.r32(v));
    }
    changed(m, env, this);
    r
}

/// `cIntMenuItem::Measure` 0x18aab0(out, this): the title beside the widest value
pub fn int_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    call(m, env, MEASURE_LABEL, &[out, this, m.r32(this + 0x10), 0, INT_WIDEST]);
    out
}

/// `cIntMenuItem::Draw` 0x18aae8(this, rect): the title beside the int ("%d")
pub fn int_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let text = scratch + 0x20;
    call(m, env, SPRINTF, &[text, INT_FORMAT, m.r32(m.r32(this + 0x14))]);
    draw_label(m, env, this, rect, m.r32(this + 0x10), 0, text, scratch);
}

pub const FLOAT_VTABLE: u32 = 0x389c00;
pub const ANGLE_VTABLE: u32 = 0x389bc0;
pub const STRING_VTABLE: u32 = 0x389b80;
pub const COLOUR_VTABLE: u32 = 0x389b40;
pub const EXPAND_MENU_VTABLE: u32 = 0x389b00;
pub const EXPAND_VTABLE: u32 = 0x389ab0;
pub const RGB_TITLE_VTABLE: u32 = 0x389a70;
pub const RGB_VTABLE: u32 = 0x389a20;
pub const ERROR_VTABLE: u32 = 0x3899e0;
/// The float's and the angle's widest value, and their formats
pub const FLOAT_WIDEST: u32 = 0x3899a0;
pub const FLOAT_FORMAT: u32 = 0x3899b0;
pub const ANGLE_FORMAT: u32 = 0x3899b8;
pub const STRLEN: u32 = 0x2fd260;
/// A float made a double for the format's variable arguments
pub const TO_DOUBLE: u32 = 0x2f7aa8;


fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}

fn float_init(m: &mut Mem, this: u32, label: u32, value: u32, title: u32, min: F, max: F, vtable: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0x10, title);
    m.w32(this + 0x14, value);
    wf(m, this + 0x18, min);
    wf(m, this + 0x1c, max);
    m.w32(this + 0xc, vtable);
    wf(m, this + 0x20, F(1.0));
    for o in [0x2c, 0x28, 0x30, 0x24] {
        m.w32(this + o, 0);
    }
    this
}

/// `cFloatMenuItem::cFloatMenuItem` 0x18ab68(this, label, value, title; min, max): +0x10 the title, +0x14 the float,
/// +0x18 / +0x1c its range, +0x20 / +0x24 the scale and offset it is shown with, +0x28 the frames held, +0x2c the
/// way held (±1), +0x30 a float kept equal; `this`
pub fn float_new(m: &mut Mem, this: u32, label: u32, value: u32, title: u32, min: F, max: F) -> u32 {
    float_init(m, this, label, value, title, min, max, FLOAT_VTABLE)
}

/// `cAngleMenuItem::cAngleMenuItem` 0x18ae80: as the float's
pub fn angle_new(m: &mut Mem, this: u32, label: u32, value: u32, title: u32, min: F, max: F) -> u32 {
    float_init(m, this, label, value, title, min, max, ANGLE_VTABLE)
}

/// `cFloatMenuItem::HandleInput` 0x18abf8 (and `cAngleMenuItem`'s 0x18af10, the same): held left or right moves the
/// float by the range · 0.00005, faster once held long ((held² · 50 / 60 · 50 / 60 / 200) times that from 1 on),
/// inside the range; the kept float set; the menu told
pub fn float_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    let dir = if item_buttons(m, env, this, 8) != 0 {
        F(-1.0)
    } else if item_buttons(m, env, this, 4) != 0 {
        F(1.0)
    } else {
        F(0.0)
    };
    if dir.ee() == rf(m, this + 0x2c).ee() {
        let h = rf(m, this + 0x28) + F(1.0);
        wf(m, this + 0x28, h);
    } else {
        m.w32(this + 0x28, 0);
        wf(m, this + 0x2c, dir);
    }
    if dir.ee() == 0.0 {
        return r;
    }
    use crate::animseq::div;
    let t = rf(m, this + 0x28);
    let f = div(div(div(t * t * F(50.0), F(60.0)) * F(50.0), F(60.0)), F(200.0));
    let base = k(0x3851_b717);
    let rate = if f.ee() < 1.0 { base } else { f * base };
    let v = m.r32(this + 0x14);
    let n = rf(m, v) + dir * (rf(m, this + 0x1c) - rf(m, this + 0x18)) * rate;
    wf(m, v, n);
    let min = rf(m, this + 0x18);
    if n.ee() < min.ee() {
        wf(m, v, min);
    }
    let (cur, max) = (rf(m, v), rf(m, this + 0x1c));
    if max.ee() < cur.ee() {
        wf(m, v, max);
    }
    let kept = m.r32(this + 0x30);
    if kept != 0 {
        m.w32(kept, m.r32(v));
    }
    changed(m, env, this);
    r
}

/// `cFloatMenuItem::Measure` 0x18ada8 (and the angle's 0x18b0c0): the title beside the widest value
pub fn float_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32) -> u32 {
    call(m, env, MEASURE_LABEL, &[out, this, m.r32(this + 0x10), 0, FLOAT_WIDEST]);
    out
}

fn float_shown(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, v: F, fmt: u32, scratch: u32) {
    let text = scratch + 0x20;
    let d = env.call(m, TO_DOUBLE, &[], &[v]);
    call(m, env, SPRINTF, &[text, fmt, d]);
    draw_label(m, env, this, rect, m.r32(this + 0x10), 0, text, scratch);
}

/// `cFloatMenuItem::Draw` 0x18ade0(this, rect): the title beside the float · the scale + the offset
pub fn float_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let v = rf(m, m.r32(this + 0x14)) * rf(m, this + 0x20) + rf(m, this + 0x24);
    float_shown(m, env, this, rect, v, FLOAT_FORMAT, scratch);
}

/// `cAngleMenuItem::Draw` 0x18b0f8(this, rect): the title beside the angle in degrees
pub fn angle_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let v = rf(m, m.r32(this + 0x14)) * k(0x4265_2ee0);
    float_shown(m, env, this, rect, v, ANGLE_FORMAT, scratch);
}

/// `cStringMenuItem::cStringMenuItem` 0x18b198(this, label); `this`
pub fn string_new1(m: &mut Mem, this: u32, label: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0xc, STRING_VTABLE);
    this
}

/// `cStringMenuItem::cStringMenuItem` 0x18b1d0(this, label, title, buffer, size)
pub fn string_new(m: &mut Mem, this: u32, label: u32, title: u32, buf: u32, size: u32) -> u32 {
    item_new(m, this, label);
    m.w32(this + 0xc, STRING_VTABLE);
    string_set(m, this, title, buf, size);
    this
}

/// `StringMenuItem_Set` 0x18b240(this, title, buffer, size): +0x10 the title, +0x14 the buffer, +0x18 its size,
/// +0x1c the cursor
pub fn string_set(m: &mut Mem, this: u32, title: u32, buf: u32, size: u32) {
    m.w32(this + 0x10, title);
    m.w32(this + 0x14, buf);
    m.w32(this + 0x18, size);
    m.w32(this + 0x1c, 0);
}

/// A name's character: a letter, a digit or '_'
fn name_char(c: i8) -> bool {
    (c as u8).is_ascii_alphanumeric() || c == b'_' as i8
}
/// The next name character from `c` by `step` (round the signed byte)
fn next_char(c: i8, step: i8) -> i8 {
    let mut c = c;
    loop {
        c = c.wrapping_add(step);
        if name_char(c) {
            return c;
        }
    }
}

/// `cStringMenuItem::HandleInput` 0x18b258(this): edits the name at the cursor: left / right move it (right past the
/// end appends '0' while there is room), 'a' inserts '0', 'A' deletes, 'c' and select step the character up and
/// down through the name characters; every edit tells the menu
pub fn string_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    let buf = |m: &Mem| m.r32(this + 0x14);
    let cur = |m: &Mem| m.r32(this + 0x1c) as i32;
    match item_key(m, this) {
        KEY_LEFT => {
            let c = m.r32(this + 0x1c);
            if c != 0 {
                m.w32(this + 0x1c, c - 1);
            }
            return r;
        }
        KEY_RIGHT => {
            let len = call(m, env, STRLEN, &[buf(m)]) as i32;
            if cur(m) < len - 1 {
                m.w32(this + 0x1c, (cur(m) + 1) as u32);
                return r;
            }
            if len >= m.r32(this + 0x18) as i32 - 1 {
                return r;
            }
            m.w32(this + 0x1c, len as u32);
            m.w8(buf(m).wrapping_add(len as u32), next_char(0, 1) as u8);
            m.w8(buf(m).wrapping_add(len as u32 + 1), 0);
        }
        0x61 => {
            let len = call(m, env, STRLEN, &[buf(m)]) as i32;
            if len >= m.r32(this + 0x18) as i32 - 1 {
                return r;
            }
            m.w8(buf(m).wrapping_add(len as u32 + 1), 0);
            if cur(m) < len {
                let mut k = len - 1;
                loop {
                    let c = m.r8(buf(m).wrapping_add(k as u32));
                    m.w8(buf(m).wrapping_add(k as u32 + 1), c);
                    if cur(m) >= k {
                        break;
                    }
                    k -= 1;
                }
            }
            let at = buf(m).wrapping_add(cur(m) as u32);
            m.w8(at, next_char(0, 1) as u8);
        }
        0x63 | KEY_SELECT => {
            let step = if item_key(m, this) == KEY_SELECT { -1 } else { 1 };
            let at = buf(m).wrapping_add(cur(m) as u32);
            let c = m.r8(at) as i8;
            m.w8(at, next_char(c, step) as u8);
        }
        0x41 => {
            let mut a = cur(m);
            loop {
                let c = m.r8(buf(m).wrapping_add(a as u32 + 1));
                m.w8(buf(m).wrapping_add(a as u32), c);
                if c == 0 {
                    break;
                }
                a += 1;
            }
            if cur(m) >= a {
                m.w32(this + 0x1c, (a - 1) as u32);
            }
        }
        _ => return r,
    }
    changed(m, env, this);
    r
}

/// `cColorMenuItem::cColorMenuItem` 0x18b660(this, colour, channel): +0x10 the colour (floats), +0x14 the channel,
/// +0x18 the frames held, +0x1c the way held; `this`
pub fn colour_new(m: &mut Mem, this: u32, colour: u32, channel: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, colour);
    m.w32(this + 0x14, channel);
    m.w32(this + 0xc, COLOUR_VTABLE);
    this
}

/// `cColorMenuItem::HandleInput` 0x18b6b8(this): held left or right moves the channel by 0.0004 · max(1, held² /
/// 288), inside 0..1; the menu told
pub fn colour_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (r, go) = base_input(m, this);
    if !go {
        return r;
    }
    let dir = if item_buttons(m, env, this, 8) != 0 { 0xffff_ffff } else { (item_buttons(m, env, this, 4) != 0) as u32 };
    if dir != m.r32(this + 0x1c) {
        m.w32(this + 0x18, 0);
        m.w32(this + 0x1c, dir);
    } else {
        m.w32(this + 0x18, m.r32(this + 0x18).wrapping_add(1));
    }
    if dir == 0 {
        return r;
    }
    use crate::animseq::cvt_s_w;
    let held = m.r32(this + 0x18) as i32;
    let t = held.wrapping_mul(held) / 0x120;
    let step = if 0 < t { t } else { 1 };
    let d = cvt_s_w(dir as i32) * (cvt_s_w(step) * k(0x39cc_cccd));
    let c = m.r32(this + 0x10).wrapping_add(m.r32(this + 0x14) << 2);
    let mut v = rf(m, c) + d;
    if v.ee() < 0.0 {
        v = F(0.0);
    }
    if 1.0 < v.ee() {
        v = F(1.0);
    }
    wf(m, c, v);
    changed(m, env, this);
    r
}

/// The expand item's menu's virtual `off` of its owner (+0x118), the owner after the arguments
fn owner_call(m: &mut Mem, env: &mut dyn TextBoxEnv, menu: u32, off: u32, args: &[u32]) -> u32 {
    let owner = m.r32(menu + 0x118);
    let vt = m.r32(owner + 0xc);
    let me = owner.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[])
}

/// `cExpandMenuItemMenu` vf2 0x18b810(this, item, argument): a command: the owner's +0x40
pub fn expand_menu_command(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32, arg: u32) -> u32 {
    owner_call(m, env, this, 0x40, &[item, arg])
}

/// `cExpandMenuItemMenu` vf3 0x18b840(this, item): a change: the owner's +0x48
pub fn expand_menu_changed(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32) {
    owner_call(m, env, this, 0x48, &[item]);
}

fn expand_init(m: &mut Mem, this: u32, label: u32, header: u32) {
    item_new(m, this, label);
    m.w32(this + 0x10, header);
    m.w32(this + 0xc, EXPAND_VTABLE);
    menu_new(m, this + 0x14);
    m.w32(this + 0x14 + 0x114, EXPAND_MENU_VTABLE);
    m.w32(this + 0x12c, this);
}

/// `cExpandMenuItem::cExpandMenuItem` 0x18b870(this): an item holding a menu (+0x14, its owner +0x12c this), with
/// no header (+0x10); `this`
pub fn expand_new0(m: &mut Mem, this: u32) -> u32 {
    expand_init(m, this, 0xffff_ffff, 0);
    this
}

/// `cExpandMenuItem::cExpandMenuItem` 0x18b8f8(this, label, header): the header its menu's first item
pub fn expand_new(m: &mut Mem, this: u32, label: u32, header: u32) -> u32 {
    expand_init(m, this, label, header);
    if m.r32(this + 0x10) != 0 {
        expand_add(m, this, header, -1);
    }
    this
}

/// `ExpandMenuItem_AddItem` 0x18b9a0(this, item, at)
pub fn expand_add(m: &mut Mem, this: u32, item: u32, at: i32) {
    menu_add_item(m, this + 0x14, item, at);
}

/// 0x18b9a8(this, at): an item removed from its menu
pub fn expand_remove(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, at: i32) {
    menu_remove_item(m, env, this + 0x14, at);
}

/// 0x18b9b0(this): its menu cleared, the header put back
pub fn expand_reset(m: &mut Mem, this: u32) {
    menu_clear(m, this + 0x14);
    let h = m.r32(this + 0x10);
    if h != 0 {
        expand_add(m, this, h, -1);
    }
}

/// `ExpandMenuItem_ReplaceItem` 0x18b9f8(this, header): the old header taken out, the new one first
pub fn expand_replace(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, header: u32) {
    let mut i = 0i32;
    while i < m.r32(this + 0x1c) as i32 {
        if m.r32(this + 0x28 + 4 * i as u32) == m.r32(this + 0x10) {
            menu_remove_item(m, env, this + 0x14, i);
            break;
        }
        i += 1;
    }
    m.w32(this + 0x10, header);
    expand_add(m, this, header, 0);
}

/// `cExpandMenuItem::HandleInput` 0x18ba68(this): its menu's picked item answers; down / up move inside its menu, and
/// past its ends (back where it was) answer down / up to the outer menu
pub fn expand_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let menu = this + 0x14;
    menu_skip_unselectable(m, env, menu);
    let old = m.r32(this + 0x20) as i32;
    let it = m.r32(this + 0x28 + ((old as u32) << 2));
    let vt = m.r32(it + 0xc);
    let me = it.wrapping_add(m.r16(vt + 0x10) as i16 as u32);
    let f = m.r32(vt + 0x14);
    let r = env.call(m, f, &[me], &[]);
    if item_flag8(m, this) != 0 {
        return r;
    }
    match r {
        3 => {
            menu_select_next(m, env, menu);
            if old < m.r32(this + 0x20) as i32 {
                return 0;
            }
            menu_set_selection(m, env, menu, old);
            3
        }
        4 => {
            menu_select_prev(m, env, menu);
            if (m.r32(this + 0x20) as i32) < old {
                return 0;
            }
            menu_set_selection(m, env, menu, old);
            4
        }
        r => r,
    }
}

/// `cExpandMenuItem` vf7 0x18bb58(this, item, argument): a command passed to its own menu (+0x18)
pub fn expand_command(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32, arg: u32) -> u32 {
    menu_call(m, env, this, 0x18, &[item, arg])
}

/// `cExpandMenuItem` vf8 0x18bb88(this, item): a change passed on (+0x20)
pub fn expand_changed(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32) {
    menu_call(m, env, this, 0x20, &[item]);
}

/// `cExpandMenuItem::Measure` 0x18bbb8(out, this): its menu's shown items measured (boxes at +0x130, 16 bytes
/// each): the height of them all when the item is picked in the outer menu, else the header's; the width the widest
/// (the others 20 in from the header)
pub fn expand_measure(m: &mut Mem, env: &mut dyn TextBoxEnv, out: u32, this: u32, scratch: u32) -> u32 {
    let (hdr, t) = (scratch, scratch + 0x10);
    m.w32(this + 0x18, m.r32(m.r32(this + 4) + 4));
    for o in [0xc, 8, 4, 0] {
        m.w32(hdr + o, 0);
    }
    let mut i = 0i32;
    while i < m.r32(this + 0x1c) as i32 {
        let it = m.r32(this + 0x28 + 4 * i as u32);
        let b = this + 0x130 + 0x10 * i as u32;
        if crate::debugmenu::item_is_shown(m, it) != 0 {
            let vt = m.r32(it + 0xc);
            let me = it.wrapping_add(m.r16(vt + 0x20) as i16 as u32);
            let f = m.r32(vt + 0x24);
            env.call(m, f, &[t, me], &[]);
            copy4(m, b, t);
            let cur = menu_current_item(m, env, m.r32(this + 4));
            let header = m.r32(this + 0x10);
            if cur == this || it == header {
                m.w32(hdr + 0xc, m.r32(hdr + 0xc).wrapping_add(m.r32(b + 0xc)));
            }
            let w = m.r32(b + 8);
            let w = if it == header { w } else { w.wrapping_add(0x14) };
            if (m.r32(hdr + 8) as i32) < w as i32 {
                m.w32(hdr + 8, w);
            }
        } else {
            for o in 0..4 {
                m.w32(t + 4 * o, 0);
            }
            copy4(m, b, t);
        }
        i += 1;
    }
    copy4(m, out, hdr);
    out
}

/// `cExpandMenuItem::Draw` 0x18bd78(this, rect): its shown items (only the header unless the item is picked in the
/// outer menu) placed down the box (the others 20 in), drawn; the box's y moved past each
pub fn expand_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, rect: u32, scratch: u32) {
    let mut i = 0i32;
    while i < m.r32(this + 0x1c) as i32 {
        let it = m.r32(this + 0x28 + 4 * i as u32);
        if crate::debugmenu::item_is_shown(m, it) != 0 {
            let cur = menu_current_item(m, env, m.r32(this + 4));
            let header = m.r32(this + 0x10);
            if cur == this || it == header {
                let inset = if it == header { 0 } else { 0x14 };
                let b = this + 0x130 + 0x10 * i as u32;
                m.w32(b, m.r32(rect).wrapping_add(inset));
                m.w32(b + 4, m.r32(rect + 4));
                m.w32(b + 8, m.r32(rect + 8).wrapping_sub(inset));
                copy4(m, scratch, b);
                let vt = m.r32(it + 0xc);
                let me = it.wrapping_add(m.r16(vt + 0x28) as i16 as u32);
                let f = m.r32(vt + 0x2c);
                env.call(m, f, &[me, scratch], &[]);
                m.w32(rect + 4, m.r32(rect + 4).wrapping_add(m.r32(b + 0xc)));
            }
        }
        i += 1;
    }
}

/// `cExpandMenuItem` vf5 0x18beb0(this, how): picked: its menu from its first item (by the next key) or its last
/// (by the previous one), entered with the outer menu's manager
pub fn expand_picked(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, how: u32) {
    let menu = this + 0x14;
    match how {
        0 => menu_set_selection(m, env, menu, 0),
        1 => menu_set_selection(m, env, menu, -1),
        _ => {}
    }
    let mgr = m.r32(m.r32(this + 4) + 4);
    menu_enter(m, env, menu, mgr);
}

/// `cExpandMenuItem` vf6 0x18bf28(this): left: its menu left
pub fn expand_left(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    menu_leave(m, env, this + 0x14);
}

/// `cRGBTitleMenuItem::cRGBTitleMenuItem` 0x18bf50(this, label, colour): +0x10 the colour, +0x14 the label; `this`
pub fn rgb_title_new(m: &mut Mem, this: u32, label: u32, colour: u32) -> u32 {
    item_new(m, this, 0xffff_ffff);
    m.w32(this + 0x10, colour);
    m.w32(this + 0x14, label);
    m.w32(this + 0xc, RGB_TITLE_VTABLE);
    this
}

/// `cRGBMenuItem::cRGBMenuItem` 0x18bfb0(this, label, title, colour): an expand item with a title (+0x594) as its
/// header and a colour item for each of red, green and blue (+0x534, +0x554, +0x574); +0x530 the colour, +0x5ac a
/// colour kept equal (none)
pub fn rgb_new(m: &mut Mem, this: u32, label: u32, title: u32, colour: u32) -> u32 {
    let (r, g, b, t) = (this + 0x534, this + 0x554, this + 0x574, this + 0x594);
    expand_new(m, this, label, t);
    m.w32(this + 0x530, colour);
    m.w32(this + 0xc, RGB_VTABLE);
    colour_new(m, r, colour, 0);
    colour_new(m, g, colour, 1);
    colour_new(m, b, colour, 2);
    rgb_title_new(m, t, title, colour);
    m.w32(this + 0x5ac, 0);
    for it in [r, g, b] {
        expand_add(m, this, it, -1);
    }
    this
}

/// `cRGBMenuItem::HandleInput` 0x18c0a8(this): the expand item's; then the colour's three words copied to the kept
/// one
pub fn rgb_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let r = expand_handle_input(m, env, this);
    if item_flag8(m, this) != 0 {
        return r;
    }
    let kept = m.r32(this + 0x5ac);
    if kept != 0 {
        let c = m.r32(this + 0x530);
        for k in 0..3 {
            m.w32(kept + 4 * k, m.r32(c + 4 * k));
        }
    }
    r
}

fn error_init(m: &mut Mem, this: u32, message: u32, gap: u32, button: u32) -> u32 {
    menu_new(m, this);
    m.w32(this + 0x114, ERROR_VTABLE);
    let (text, space, pop) = (this + 0x118, this + 0x130, this + 0x144);
    null_new(m, text, message);
    space_new(m, space, gap);
    pop_new(m, pop, button);
    for it in [text, space, pop] {
        menu_add_item(m, this, it, -1);
    }
    this
}

/// `cErrorMenu::cErrorMenu` 0x18c118(this): a menu of a blank line, a gap of 10 and a pop item; `this`
pub fn error_new0(m: &mut Mem, this: u32) -> u32 {
    error_init(m, this, 0, 10, 0)
}

/// `cErrorMenu::cErrorMenu` 0x18c1c8(this, message, button): the message, a gap of 30 and the button
pub fn error_new(m: &mut Mem, this: u32, message: u32, button: u32) -> u32 {
    error_init(m, this, message, 0x1e, button)
}

//! The debug menu's framework (board row G9a4a): the menu item base `cMenuItem`, the menu `cMenu` (its items, the
//! selection, its layout and draw) and the menu manager `cMenuManager` (a stack of menus, the key of the frame). On
//! the game's bytes ([`Mem`]); the virtuals (through the objects' tables), the pad and the draws are [`TextBoxEnv`]
//! calls by address. Checked in the function runner (`tests/debugmenu.rs`). The menu is the leftover developer
//! menu (`cDebugMenu`, G9a4); the item kinds and the drawing are G9a4b and G9a4c.
//!
//! An item: +0 its label, +4 its menu, +8 flags (1 shown, 8 not selectable), +0xc its table (+0x10 the key's
//! handler, +0x18 whether it can be picked, +0x20 its size (a 16-byte box: x, y, w, h, returned through a pointer
//! first), +0x28 its draw (with its box), +0x30 picked (how: 0 by the next key, 1 by the previous one, 2 set), +0x38
//! left). A menu: +4 its manager, +8 the count, +0xc the item picked, +0x10 the first shown, +0x14.. the items,
//! +0x114 its table (+0x10 the key's handler, +0x28 its draw, +0x30 entered (with the manager), +0x38 left). The
//! manager: +0 the depth, +4.. the menus, +0x2c the pad, +0x30 the frame's key, +0x34 active.
//!
//! Where the game builds on its stack (the items' boxes), the port uses `scratch` (STANDIN: the place, not the
//! bytes).

use crate::pf::Mem;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const ITEM_VTABLE: u32 = 0x389e80;
pub const MENU_VTABLE: u32 = 0x389e40;
pub const MEM_DELETE: u32 = 0x114f80;
pub const INPUT_DEVICE: u32 = 0x17a830;
pub const INPUT_PRESSED: u32 = 0x17a840;
pub const BEGIN_DRAW: u32 = 0x18c288;
pub const END_DRAW: u32 = 0x18c318;
pub const UP_ARROW: u32 = 0x18c728;
pub const DOWN_ARROW: u32 = 0x18c9d0;

/// The keys the menus act on
pub const KEY_BACK: u32 = 8;
pub const KEY_SELECT: u32 = 0xd;
pub const KEY_ESCAPE: u32 = 0x1b;
pub const KEY_UP: u32 = 0x4800;
pub const KEY_DOWN: u32 = 0x5000;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
/// A virtual of `o` through the table at `vt`: `this` moved by the half word at `off`, the function at `off + 4`
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, o: u32, vt: u32, off: u32, args: &[u32]) -> u32 {
    let me = o.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[])
}
/// An item's virtual (its table at +0xc)
fn icall(m: &mut Mem, env: &mut dyn TextBoxEnv, item: u32, off: u32, args: &[u32]) -> u32 {
    vcall(m, env, item, m.r32(item + 0xc), off, args)
}
/// A menu's virtual (its table at +0x114)
fn mcall(m: &mut Mem, env: &mut dyn TextBoxEnv, menu: u32, off: u32, args: &[u32]) -> u32 {
    vcall(m, env, menu, m.r32(menu + 0x114), off, args)
}
fn item_at(m: &Mem, menu: u32, i: u32) -> u32 {
    m.r32(menu + 0x14 + (i << 2))
}

/// `cMenuItem::cMenuItem` 0x1889b8(this, label): shown; `this`
pub fn item_new(m: &mut Mem, this: u32, label: u32) -> u32 {
    m.w32(this, label);
    m.w32(this + 0xc, ITEM_VTABLE);
    m.w32(this + 8, 1);
    this
}

/// `cMenuItem::~cMenuItem` 0x1889d8(this, flags): the table; freed with flag 1 (the free given the flag alone)
pub fn item_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this + 0xc, ITEM_VTABLE);
    if flags & 1 != 0 {
        call(m, env, MEM_DELETE, &[this, flags & 1]);
    }
}

/// `cMenuItem::HandleInput` 0x188a00(this): the frame's key as the menu's answer: 1 back, 2 escape, 3 down, 4 up,
/// else 0
pub fn item_handle_input(m: &Mem, this: u32) -> u32 {
    match item_key(m, this) {
        KEY_ESCAPE => 2,
        KEY_BACK => 1,
        KEY_UP => 4,
        KEY_DOWN => 3,
        _ => 0,
    }
}

/// `cMenuItem::IsSelectable` 0x188a80(this): shown and without flag 8
pub fn item_is_selectable(m: &Mem, this: u32) -> u32 {
    if item_is_shown(m, this) == 0 {
        return 0;
    }
    (item_flag8(m, this) == 0) as u32
}

/// `cMenuItem` vf5 0x188ac8 and vf6 0x188ad0 (picked, left): nothing
pub fn item_nothing() {}

/// `MenuItem_IsDisabled` 0x188ad8(this): the flags' bit 1 (set when the item is shown)
pub fn item_is_shown(m: &Mem, this: u32) -> u32 {
    m.r32(this + 8) & 1
}

/// 0x188ae8(this, on): the bit 1 set or cleared
pub fn item_set_shown(m: &mut Mem, this: u32, on: u32) {
    let f = m.r32(this + 8);
    m.w32(this + 8, if on != 0 { f | 1 } else { f & !1 });
}

/// `MenuItem_HasFlag8` 0x188b10(this)
pub fn item_flag8(m: &Mem, this: u32) -> u32 {
    m.r32(this + 8) & 8
}

/// 0x188b20(this, on): the bit 8 set or cleared
pub fn item_set_flag8(m: &mut Mem, this: u32, on: u32) {
    let f = m.r32(this + 8);
    m.w32(this + 8, if on != 0 { f | 8 } else { f & !8 });
}

fn item_manager(m: &Mem, this: u32) -> u32 {
    m.r32(m.r32(this + 4) + 4)
}

/// `MenuItem_GetKey` 0x188b48(this): its manager's key
pub fn item_key(m: &Mem, this: u32) -> u32 {
    let mgr = item_manager(m, this);
    manager_key(m, mgr)
}

/// `MenuItem_IsPadKey` 0x188b68(this, key)
pub fn item_is_pad_key(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, key: u32) -> u32 {
    let mgr = item_manager(m, this);
    manager_is_pad_key(m, env, mgr, key)
}

/// `MenuItem_GetButtons` 0x188b88(this, mask)
pub fn item_buttons(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, mask: u32) -> u32 {
    let mgr = item_manager(m, this);
    manager_buttons(m, env, mgr, mask)
}

/// `cMenu::cMenu` 0x188ba8(this): empty, no manager; `this`
pub fn menu_new(m: &mut Mem, this: u32) -> u32 {
    m.w32(this + 4, 0);
    m.w32(this + 0x114, MENU_VTABLE);
    m.w32(this + 8, 0);
    m.w32(this + 0xc, 0);
    m.w32(this + 0x10, 0);
    this
}

/// `cMenu::~cMenu` 0x188bd0(this, flags)
pub fn menu_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this + 0x114, MENU_VTABLE);
    if flags & 1 != 0 {
        call(m, env, MEM_DELETE, &[this, flags & 1]);
    }
}

/// `Menu_SkipUnselectable` 0x188bf8(this): off an item that can't be picked, to the next
pub fn menu_skip_unselectable(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let it = item_at(m, this, m.r32(this + 0xc));
    if icall(m, env, it, 0x18, &[]) == 0 {
        menu_select_next(m, env, this);
    }
}

/// The selection moved on (`back`: to the previous) round the list to the first item that can be picked (back where
/// it was: nothing); the old one left, the new one picked (`back` as how)
fn menu_move(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, back: bool) {
    let old = m.r32(this + 0xc);
    let mut v = old as i32;
    loop {
        let n = m.r32(this + 8) as i32;
        assert!(n != 0, "a menu with no items (the game's `div` traps)");
        let s = if back { v.wrapping_add(n).wrapping_sub(1) } else { v.wrapping_add(1) };
        let next = s.wrapping_rem(n) as u32;
        m.w32(this + 0xc, next);
        if next == old {
            return;
        }
        let it = item_at(m, this, next);
        if icall(m, env, it, 0x18, &[]) != 0 {
            break;
        }
        v = m.r32(this + 0xc) as i32;
    }
    let it = item_at(m, this, old);
    icall(m, env, it, 0x38, &[]);
    let it = item_at(m, this, m.r32(this + 0xc));
    icall(m, env, it, 0x30, &[back as u32]);
}

/// `Menu_SelectNext` 0x188c50(this)
pub fn menu_select_next(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    menu_move(m, env, this, false);
}

/// `Menu_SelectPrev` 0x188d20(this)
pub fn menu_select_prev(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    menu_move(m, env, this, true);
}

/// `cMenu` vf1 0x188df0(this): the picked item's answer to the key; down and up move the selection (and answer 0)
pub fn menu_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    menu_skip_unselectable(m, env, this);
    let it = item_at(m, this, m.r32(this + 0xc));
    match icall(m, env, it, 0x10, &[]) {
        3 => {
            menu_select_next(m, env, this);
            0
        }
        4 => {
            menu_select_prev(m, env, this);
            0
        }
        r => r,
    }
}

/// `cMenu` vf2 0x188e78: 0; vf3 0x188e80: nothing
pub fn menu_vf2() -> u32 {
    0
}

/// `cMenu` vf4 0x188e88(this): the layout and draw. Each shown item measured (its box from its size virtual; the
/// others none), the widest and the total height; the list centred across (640) and, when it fits in 421, down (480)
/// from the top; else from 30, the first shown (+0x10) moved so the picked item is on the screen. Then from the
/// first shown the items that fit above 451 (the picked one always) placed at the list's x and the running y, the
/// widest's width, and drawn; the arrows when there is more above or below
pub fn menu_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    // the game's frame: +0 x, +4 y, +8 the widest, +0xc the height; the boxes from +0x10, 16 bytes each; a box
    // handed over at +0x410
    let (hdr, boxes, out) = (scratch, scratch + 0x10, scratch + 0x410);
    let bx = |i: u32| boxes + 0x10 * i;
    for o in [0xc, 8, 4, 0] {
        m.w32(hdr + o, 0);
    }
    let n = m.r32(this + 8) as i32;
    let mut i = 0i32;
    while i < n {
        let it = item_at(m, this, i as u32);
        if item_is_shown(m, it) != 0 {
            // (the size virtual's box comes back through a hidden first pointer, before `this`)
            let vt = m.r32(it + 0xc);
            let me = it.wrapping_add(m.r16(vt + 0x20) as i16 as u32);
            let f = m.r32(vt + 0x24);
            env.call(m, f, &[out, me], &[]);
            for k in 0..4 {
                m.w32(bx(i as u32) + 4 * k, m.r32(out + 4 * k));
            }
            let h = m.r32(bx(i as u32) + 0xc);
            m.w32(hdr + 0xc, m.r32(hdr + 0xc).wrapping_add(h));
            let w = m.r32(bx(i as u32) + 8) as i32;
            if (m.r32(hdr + 8) as i32) < w {
                m.w32(hdr + 8, w as u32);
            }
        } else {
            m.w32(bx(i as u32) + 0xc, 0);
            m.w32(bx(i as u32) + 8, 0);
        }
        i += 1;
    }
    let half = |v: i32| v / 2;
    m.w32(hdr, half(0x280i32.wrapping_sub(m.r32(hdr + 8) as i32)) as u32);
    let total = m.r32(hdr + 0xc) as i32;
    let y = if total < 0x1a5 {
        m.w32(this + 0x10, 0);
        half(0x1e0i32.wrapping_sub(total))
    } else {
        let sel = m.r32(this + 0xc) as i32;
        let mut top = m.r32(this + 0x10) as i32;
        if sel < top {
            m.w32(this + 0x10, sel as u32);
            top = sel;
        }
        let mut sum = 0i32;
        let mut a = top;
        while a <= sel {
            sum = sum.wrapping_add(m.r32(bx(a as u32) + 0xc) as i32);
            a += 1;
        }
        let mut a = top;
        if a < sel && sum >= 0x1a5 {
            loop {
                let h = m.r32(bx(a as u32) + 0xc) as i32;
                a += 1;
                m.w32(this + 0x10, a as u32);
                sum = sum.wrapping_sub(h);
                if a >= sel || sum < 0x1a5 {
                    break;
                }
            }
        }
        0x1e
    };
    m.w32(hdr + 4, y as u32);
    call(m, env, BEGIN_DRAW, &[this]);
    let mut i = m.r32(this + 0x10) as i32;
    while i < m.r32(this + 8) as i32 {
        let it = item_at(m, this, i as u32);
        if item_is_shown(m, it) == 0 {
            i += 1;
            continue;
        }
        let b = bx(i as u32);
        if i != m.r32(this + 0xc) as i32 && (m.r32(hdr + 4).wrapping_add(m.r32(b + 0xc)) as i32) >= 0x1c3 {
            break;
        }
        m.w32(b, m.r32(hdr));
        m.w32(b + 4, m.r32(hdr + 4));
        m.w32(b + 8, m.r32(hdr + 8));
        m.w32(hdr + 4, m.r32(hdr + 4).wrapping_add(m.r32(b + 0xc)));
        for k in 0..4 {
            m.w32(out + 4 * k, m.r32(b + 4 * k));
        }
        icall(m, env, it, 0x28, &[out]);
        i += 1;
    }
    if m.r32(this + 0x10) != 0 {
        for (k, v) in [0x135u32, 8, 0x16, 0xf].into_iter().enumerate() {
            m.w32(out + 4 * k as u32, v);
        }
        call(m, env, UP_ARROW, &[this, out]);
    }
    if i != m.r32(this + 8) as i32 {
        for (k, v) in [0x135u32, 0x1c9, 0x16, 0xf].into_iter().enumerate() {
            m.w32(out + 4 * k as u32, v);
        }
        call(m, env, DOWN_ARROW, &[this, out]);
    }
    call(m, env, END_DRAW, &[this]);
}

/// `cMenu::AddItem` 0x189240(this, item, at): at `at` (the end when negative), the items after moved up; with a
/// manager the selection kept on its item
pub fn menu_add_item(m: &mut Mem, this: u32, item: u32, at: i32) {
    m.w32(item + 4, this);
    let n = m.r32(this + 8) as i32;
    let at = if at < 0 { n } else { at };
    let mut k = n;
    while at < k {
        m.w32(this + 0x14 + ((k as u32) << 2), item_at(m, this, (k - 1) as u32));
        k -= 1;
    }
    m.w32(this + 0x14 + ((at as u32) << 2), item);
    if m.r32(this + 4) != 0 {
        let sel = m.r32(this + 0xc) as i32;
        if sel >= at && n != 0 {
            m.w32(this + 0xc, (sel + 1) as u32);
        }
    }
    m.w32(this + 8, (n + 1) as u32);
}

/// `Menu_RemoveItem` 0x1892e8(this, at): the items after moved down; the selection kept on its item, inside the list,
/// and (when it was the one removed) off an item that can't be picked
pub fn menu_remove_item(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, at: i32) {
    let n = m.r32(this + 8) as i32;
    let mut k = at;
    while k < n - 1 {
        m.w32(this + 0x14 + ((k as u32) << 2), item_at(m, this, (k + 1) as u32));
        k += 1;
    }
    let mut sel = m.r32(this + 0xc) as i32;
    m.w32(this + 8, (n - 1) as u32);
    if at < sel {
        sel -= 1;
        m.w32(this + 0xc, sel as u32);
    }
    let n = m.r32(this + 8) as i32;
    if sel >= n {
        sel = n - 1;
        m.w32(this + 0xc, sel as u32);
    }
    if at == sel {
        menu_skip_unselectable(m, env, this);
    }
}

/// 0x189390(this): no items, the first picked
pub fn menu_clear(m: &mut Mem, this: u32) {
    m.w32(this + 8, 0);
    m.w32(this + 0xc, 0);
}

/// `Menu_SetSelection` 0x1893a0(this, i): (from the end when negative) the old item left, the new one picked (2)
pub fn menu_set_selection(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, i: i32) {
    let i = if i < 0 { i.wrapping_add(m.r32(this + 8) as i32) } else { i };
    let sel = m.r32(this + 0xc);
    if i as u32 == sel {
        return;
    }
    let it = item_at(m, this, sel);
    icall(m, env, it, 0x38, &[]);
    m.w32(this + 0xc, i as u32);
    let it = item_at(m, this, i as u32);
    icall(m, env, it, 0x30, &[2]);
    menu_skip_unselectable(m, env, this);
}

/// `Menu_GetSelectedIndex` 0x189450(this)
pub fn menu_selected_index(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    menu_skip_unselectable(m, env, this);
    m.r32(this + 0xc)
}

/// `Menu_GetCurrentItem` 0x189478(this)
pub fn menu_current_item(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let i = menu_selected_index(m, env, this);
    item_at(m, this, i)
}

/// `cMenu` vf5 0x1894a8(this, manager): entered: the manager kept, the selection inside the list and picked (2)
pub fn menu_enter(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, mgr: u32) {
    m.w32(this + 4, mgr);
    let n = m.r32(this + 8) as i32;
    if m.r32(this + 0xc) as i32 >= n {
        m.w32(this + 0xc, (n - 1) as u32);
    }
    let it = item_at(m, this, m.r32(this + 0xc));
    icall(m, env, it, 0x30, &[2]);
    menu_skip_unselectable(m, env, this);
}

/// `cMenu` vf6 0x189518(this): left: the picked item left, no manager
pub fn menu_leave(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let it = item_at(m, this, m.r32(this + 0xc));
    icall(m, env, it, 0x38, &[]);
    m.w32(this + 4, 0);
}

/// `cMenuManager::cMenuManager` 0x189560(this): `this`
pub fn manager_new(m: &mut Mem, this: u32) -> u32 {
    m.w32(this, 0);
    manager_reset(m, this);
    this
}

/// 0x189590(this, flags): the manager's destructor: freed with flag 1
pub fn manager_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    if flags & 1 != 0 {
        call(m, env, MEM_DELETE, &[this, flags & 1]);
    }
}

/// `cMenuManager::Reset` 0x1895b0(this): no menus, key or activity
pub fn manager_reset(m: &mut Mem, this: u32) {
    m.w32(this, 0);
    m.w32(this + 0x30, 0);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
}

/// `cMenuManager::IsActive` 0x1895c8(this): active with a menu
pub fn manager_is_active(m: &Mem, this: u32) -> u32 {
    if m.r32(this) == 0 {
        0
    } else {
        (m.r32(this + 0x34) != 0) as u32
    }
}

/// `cMenuManager::SetActive` 0x1895e0(this, on)
pub fn manager_set_active(m: &mut Mem, this: u32, on: u32) {
    m.w32(this + 0x34, on);
}

/// `cMenuManager::Update` 0x1895e8(this): the frame's key (the pressed device's, from the app's input +0x24): in
/// the app's mode 4 13 and 100 are back (8) and 99 select (13), else 100 is select and 98 back; then the top
/// menu's answer: 1 pops it, 2 all of them
pub fn manager_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let app = m.r32(APP);
    let key = call(m, env, INPUT_PRESSED, &[m.r32(app + 0x24), 0]);
    m.w32(this + 0x30, key);
    if m.r32(m.r32(APP) + 0x58) == 4 {
        if key == 0xd || key == 0x64 {
            m.w32(this + 0x30, 8);
        }
        if m.r32(this + 0x30) == 0x63 {
            m.w32(this + 0x30, 0xd);
        }
    } else {
        if key == 0x64 {
            m.w32(this + 0x30, 0xd);
        }
        if m.r32(this + 0x30) == 0x62 {
            m.w32(this + 0x30, 8);
        }
    }
    let depth = m.r32(this);
    if depth == 0 {
        return;
    }
    let top = m.r32(this + 4 + ((depth.wrapping_sub(1)) << 2));
    match mcall(m, env, top, 0x10, &[]) {
        1 => manager_pop(m, env, this),
        2 => {
            while m.r32(this) != 0 {
                manager_pop(m, env, this);
            }
        }
        _ => {}
    }
}

/// `cMenuManager::GetKey` 0x189718(this)
pub fn manager_key(m: &Mem, this: u32) -> u32 {
    m.r32(this + 0x30)
}

fn device(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let app = m.r32(APP);
    call(m, env, INPUT_DEVICE, &[m.r32(app + 0x24), m.r32(this + 0x2c)])
}

/// `cMenuManager::IsPadKey` 0x189720(this, key): the pad's key (its device's +0x2c) is `key`
pub fn manager_is_pad_key(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, key: u32) -> u32 {
    let d = device(m, env, this);
    (m.r32(d + 0x2c) == key) as u32
}

/// `cMenuManager::GetButtons` 0x189760(this, mask): the pad's buttons (+0x10) in `mask`
pub fn manager_buttons(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, mask: u32) -> u32 {
    let d = device(m, env, this);
    m.r32(d + 0x10) & mask
}

/// `cMenuManager::SetPad` 0x1897a0(this, pad)
pub fn manager_set_pad(m: &mut Mem, this: u32, pad: u32) {
    m.w32(this + 0x2c, pad);
}

/// 0x1897a8(this): the pad
pub fn manager_pad(m: &Mem, this: u32) -> u32 {
    m.r32(this + 0x2c)
}

/// `cMenuManager::Draw` 0x1897b0(this): the top menu's draw (with no menu, the word before the stack is taken as
/// one)
pub fn manager_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let top = m.r32(this + 4 + (m.r32(this).wrapping_sub(1) << 2));
    mcall(m, env, top, 0x28, &[]);
}

/// `cMenuManager::Push` 0x1897f0(this, menu): the top menu left, `menu` on top and entered; active
pub fn manager_push(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, menu: u32) {
    let depth = m.r32(this);
    if depth != 0 {
        let top = m.r32(this + 4 + ((depth - 1) << 2));
        mcall(m, env, top, 0x38, &[]);
    }
    let depth = m.r32(this);
    m.w32(this + 4 + (depth << 2), menu);
    m.w32(this, depth.wrapping_add(1));
    mcall(m, env, menu, 0x30, &[this]);
    manager_set_active(m, this, 1);
}

/// `cMenuManager::Pop` 0x189898(this): the top menu left, the one under it entered; inactive when none is left
pub fn manager_pop(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let depth = m.r32(this).wrapping_sub(1);
    m.w32(this, depth);
    let top = m.r32(this + 4 + (depth << 2));
    mcall(m, env, top, 0x38, &[]);
    let depth = m.r32(this);
    if depth != 0 {
        let top = m.r32(this + 4 + ((depth - 1) << 2));
        mcall(m, env, top, 0x30, &[this]);
        if m.r32(this) != 0 {
            return;
        }
    }
    manager_set_active(m, this, 0);
}

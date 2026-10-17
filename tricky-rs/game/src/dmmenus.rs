//! The debug menus (board row G9a4d): `cDebugMenu` (the developer menu's top page), its sub-pages `cGameOptionsMenu`,
//! `cRenderOptionsMenu` (with `cBezierOptionsMenu` and `cResolutionMenu` under it) and `cSoundVolumeMenu`, their
//! handlers and destructors, on the framework and item kinds of [`crate::debugmenu`] and [`crate::dmitems`]. On the
//! game's bytes ([`Mem`]); the game's settings (the app, the race, the renderer and its objects, the sound system,
//! the career) are [`TextBoxEnv`] calls by address. Checked in the function runner (`tests/dmmenus.rs`).
//!
//! An item's +0 is its id here (the menus' handlers switch on it); the pages keep the values their items edit from
//! +0x118.

use crate::debugmenu::{item_delete, manager_set_active, menu_add_item, menu_delete, menu_enter, menu_handle_input, menu_new};
use crate::debugmenu::menu_set_selection;
use crate::dmdraw::MANAGER as MANAGER_ADDR;
use crate::dmitems::{bool_new, command_new, command_new0, enum_new, enum_new1, float_new, int_new, int_set_range, null_new};
use crate::dmitems::{space_new, sub_new};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const CAREER: u32 = 0x32b390;
pub const MUSIC: u32 = 0x343050;
/// The developer menu's version line (formatted at start)
pub const VERSION_LINE: u32 = 0x337de8;

pub const DEBUG_VTABLE: u32 = 0x389070;
pub const SOUND_VTABLE: u32 = 0x3890b0;
pub const GAME_VTABLE: u32 = 0x3890f0;
pub const RENDER_VTABLE: u32 = 0x389130;
pub const RESOLUTION_VTABLE: u32 = 0x389170;
pub const BEZIER_VTABLE: u32 = 0x3891b0;

/// The strings and lists the pages are built with (titles, the bools' words, the enums' lists)
pub const S_RESOLUTION: u32 = 0x388c60;
pub const L_FILTER: u32 = 0x388ca0;
pub const S_RENDER: u32 = 0x388cb8;
pub const S_BEZIER: u32 = 0x388cc8;
pub const S_SHOW_FPS: u32 = 0x388cd8;
pub const S_ON: u32 = 0x388ce8;
pub const S_OFF: u32 = 0x388cf0;
pub const S_SPLIT: u32 = 0x388cf8;
pub const S_SPLIT_ON: u32 = 0x388d10;
pub const S_SPLIT_OFF: u32 = 0x388d18;
pub const S_GAMMA: u32 = 0x388d20;
pub const S_FILTER: u32 = 0x388d38;
pub const S_RES_MENU: u32 = 0x388d48;
pub const S_BEZ: [u32; 5] = [0x388d58, 0x388d68, 0x388d78, 0x388d80, 0x388d90];
pub const S_GAME: u32 = 0x388db8;
pub const S_GAME_BOOL: u32 = 0x388dc8;
pub const S_GAME_INT: u32 = 0x388dd8;
pub const S_GAME_CMD: u32 = 0x388df0;
pub const S_DEBUG: u32 = 0x388e00;
pub const S_BACK: u32 = 0x388e10;
pub const S_DEBUG_CMD2: u32 = 0x388e20;
pub const S_DEBUG_CMD1: u32 = 0x388e30;
pub const S_SOUND: u32 = 0x388e40;
pub const S_DEBUG_CMD0: u32 = 0x388e50;
pub const S_VERSION_FMT: u32 = 0x388e60;
pub const S_CLOSED: u32 = 0x388e68;
pub const L_SOUND_8: u32 = 0x388e88;
pub const L_SOUND_10: u32 = 0x388ed8;
pub const L_SOUND_9: u32 = 0x388f60;
pub const S_VOL: [u32; 7] = [0x388f90, 0x388fa0, 0x388fb0, 0x388fc0, 0x388fd8, 0x388fe8, 0x388ff8];
pub const S_SOUND_ENUM: [u32; 3] = [0x389008, 0x389018, 0x389020];
pub const S_SONG: u32 = 0x389030;
/// Static colours (white-gold, gold, red-gold, a sprite record), as the loading screens' at 0x3d2c88
pub const STATIC_COLOURS: u32 = 0x3cdde8;

pub const SET_MODE: u32 = 0x180350;
pub const INPUT_RESET: u32 = 0x17a658;
pub const SET_BRIGHT: u32 = 0x17b980;
pub const GET_SETTING: u32 = 0x17f078;
pub const RESET_RIDER: u32 = 0x11da40;
pub const CAREER_SAVE_A: u32 = 0x161a98;
pub const CAREER_SAVE_B: u32 = 0x161840;
pub const QUIT_RACE: u32 = 0x180a10;
pub const OVERLAY_SHOW: u32 = 0x1a7848;
pub const OVERLAY_GET: u32 = 0x1a7be0;
pub const OVERLAY_SET: u32 = 0x1ef990;
pub const SND_RESUME: u32 = 0x2153f8;
pub const SND_PAUSE: u32 = 0x215458;
pub const SND_STOP: u32 = 0x215488;
pub const SET_VOL: u32 = 0x211db0;
pub const GET_VOL: u32 = 0x211f20;
pub const SET_PRESET: u32 = 0x21bca8;
pub const START_MUSIC: u32 = 0x215220;
pub const MUSIC_COUNT: u32 = 0x225588;
pub const MUSIC_SELECT: u32 = 0x2255c0;
pub const MUSIC_NEXT: u32 = 0x225660;
pub const SPRINTF: u32 = 0x2fca28;
pub const PRINTF: u32 = 0x188990;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn fcall(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32], f: &[F]) -> u32 {
    env.call(m, func, args, f)
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, o: u32, vt: u32, off: u32, args: &[u32], f: &[F]) -> u32 {
    let me = o.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, f)
}
/// A virtual of the renderer (the app's +0x724; its table +0x10)
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32], f: &[F]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, m.r32(r + 0x10), off, args, f)
}
/// A virtual of the renderer's +0x20 object (its table at +0)
fn ocall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32], f: &[F]) -> u32 {
    let o = rcall(m, env, 0x20, &[], &[]);
    vcall(m, env, o, m.r32(o), off, args, f)
}
fn race(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x730)
}
fn sys(m: &Mem) -> u32 {
    m.r32(MUSIC)
}
fn add(m: &mut Mem, menu: u32, item: u32) {
    menu_add_item(m, menu, item, -1);
}
fn id(m: &Mem, item: u32) -> u32 {
    m.r32(item)
}

/// `cResolutionMenu::cResolutionMenu` 0x186bb0(this): a title, a gap, 128 command items (+0x144, 0x20 each); one
/// shown for each of the renderer's modes (its +0x50 the count, +0x48 a mode's name), the mode its id; `this`
pub fn resolution_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    menu_new(m, this);
    m.w32(this + 0x114, RESOLUTION_VTABLE);
    null_new(m, this + 0x118, S_RESOLUTION);
    space_new(m, this + 0x130, 0xa);
    for i in 0..0x80 {
        command_new0(m, this + 0x144 + 0x20 * i);
    }
    add(m, this, this + 0x118);
    add(m, this, this + 0x130);
    let mut i = 0u32;
    while (i as i32) < rcall(m, env, 0x50, &[], &[]) as i32 {
        let name = rcall(m, env, 0x48, &[i], &[]);
        let it = this + 0x144 + 0x20 * i;
        m.w32(it, i);
        m.w32(it + 0x10, name);
        add(m, this, it);
        i += 1;
    }
    this
}

/// `cResolutionMenu` vf5 0x186cf8(this, manager): entered; the item of the renderer's mode (+0x40) picked
pub fn resolution_enter(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, mgr: u32) {
    menu_enter(m, env, this, mgr);
    let mode = rcall(m, env, 0x40, &[], &[]);
    let mut i = 0i32;
    while i < m.r32(this + 8) as i32 {
        let it = m.r32(this + 0x14 + 4 * i as u32);
        let vt = m.r32(it + 0xc);
        if vcall(m, env, it, vt, 0x18, &[], &[]) != 0 && id(m, it) == mode {
            menu_set_selection(m, env, this, i);
        }
        i += 1;
    }
}

/// `cResolutionMenu` vf2 0x186dd0(this, item, argument): the mode set (the renderer's +0x28), the race's mode and
/// the input reset; 1
pub fn resolution_command(m: &mut Mem, env: &mut dyn TextBoxEnv, _this: u32, item: u32, _arg: u32) -> u32 {
    rcall(m, env, 0x28, &[id(m, item)], &[]);
    call(m, env, SET_MODE, &[race(m)]);
    call(m, env, INPUT_RESET, &[m.r32(m.r32(APP) + 0x24)]);
    1
}

/// `cRenderOptionsMenu::cRenderOptionsMenu` 0x186e30(this): the render page: a title, a gap, the bezier page and the
/// resolutions behind sub items, the frame-rate and split-screen bools, the gamma float (0.5..2) and the filter enum;
/// their values from the race (+0x74), 1, the renderer's gamma (+0x67c) and its filter (its +0x130's word); the
/// split-screen bool only with two players; `this`
pub fn render_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (title, gap, bez, bez_item, fps, split, res, gamma, filter, res_item) = (
        this + 0x128,
        this + 0x140,
        this + 0x154,
        this + 0x380,
        this + 0x398,
        this + 0x3bc,
        this + 0x3e0,
        this + 0x1524,
        this + 0x1558,
        this + 0x1578,
    );
    menu_new(m, this);
    m.w32(this + 0x114, RENDER_VTABLE);
    null_new(m, title, S_RENDER);
    space_new(m, gap, 0xa);
    bezier_new(m, env, bez);
    sub_new(m, bez_item, bez, S_BEZIER);
    bool_new(m, fps, 0, this + 0x118, S_SHOW_FPS, S_ON, S_OFF);
    bool_new(m, split, 1, this + 0x11c, S_SPLIT, S_SPLIT_ON, S_SPLIT_OFF);
    resolution_new(m, env, res);
    float_new(m, gamma, 2, this + 0x120, S_GAMMA, F(0.5), F(2.0));
    enum_new(m, filter, 3, this + 0x124, S_FILTER, 3, L_FILTER);
    sub_new(m, res_item, res, S_RES_MENU);
    let rend = m.r32(m.r32(APP) + 0x724);
    m.w32(this + 0x118, m.r32(race(m) + 0x74));
    m.w32(this + 0x11c, 1);
    m.w32(this + 0x120, m.r32(rend + 0x67c));
    let p = rcall(m, env, 0x130, &[], &[]);
    m.w32(this + 0x124, m.r32(p));
    for it in [title, gap, res_item, bez_item, fps] {
        add(m, this, it);
    }
    if m.r32(race(m) + 0x298) == 2 {
        add(m, this, split);
    }
    add(m, this, gamma);
    add(m, this, filter);
    this
}

/// `cRenderOptionsMenu` vf3 0x1870a0(this, item): a value changed: the frame rate shown (the race's +0x74), the
/// split screen (+0x29c, the mode set again), the gamma (the app's brightness), the filter (the renderer's +0x120)
pub fn render_changed(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32) {
    match id(m, item) {
        0 => {
            let r = race(m);
            m.w32(r + 0x74, m.r32(this + 0x118));
        }
        1 => {
            let r = race(m);
            m.w32(r + 0x29c, m.r32(this + 0x11c));
            call(m, env, SET_MODE, &[race(m)]);
        }
        2 => {
            let v = k(m.r32(this + 0x120));
            fcall(m, env, SET_BRIGHT, &[m.r32(APP)], &[v]);
        }
        3 => {
            rcall(m, env, 0x120, &[m.r32(this + 0x124)], &[]);
        }
        _ => {}
    }
}

/// `cBezierOptionsMenu::cBezierOptionsMenu` 0x187170(this): three bools and two floats (1000..3000, 5000..15000), from
/// the renderer's +0x20 object (+0x60, +0x68, +0x70, +0x30, +0x38); `this`
pub fn bezier_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (title, gap) = (this + 0x12c, this + 0x144);
    let items = [this + 0x158, this + 0x17c, this + 0x1a0, this + 0x1c4, this + 0x1f8];
    menu_new(m, this);
    m.w32(this + 0x114, BEZIER_VTABLE);
    null_new(m, title, S_BEZIER);
    space_new(m, gap, 0xa);
    for i in 0..3 {
        bool_new(m, items[i], i as u32, this + 0x118 + 4 * i as u32, S_BEZ[i], S_ON, S_OFF);
    }
    float_new(m, items[3], 3, this + 0x124, S_BEZ[3], k(0x447a_0000), k(0x453b_8000));
    float_new(m, items[4], 4, this + 0x128, S_BEZ[4], k(0x459c_4000), k(0x466a_6000));
    for (o, off) in [(0x118u32, 0x60u32), (0x11c, 0x68), (0x120, 0x70), (0x124, 0x30), (0x128, 0x38)] {
        let v = ocall(m, env, off, &[], &[]);
        m.w32(this + o, v);
    }
    for it in [title, gap].into_iter().chain(items) {
        add(m, this, it);
    }
    this
}

/// `cBezierOptionsMenu` vf3 0x187468(this, item): the value set on the renderer's +0x20 object (+0x48, +0x50, +0x58,
/// and the floats +0x20, +0x28)
pub fn bezier_changed(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32) {
    match id(m, item) {
        0 => {
            let v = m.r32(this + 0x118);
            ocall_with(m, env, 0x48, v);
        }
        1 => {
            let v = m.r32(this + 0x11c);
            ocall_with(m, env, 0x50, v);
        }
        2 => {
            let v = m.r32(this + 0x120);
            ocall_with(m, env, 0x58, v);
        }
        3 => {
            let o = rcall(m, env, 0x20, &[], &[]);
            let v = k(m.r32(this + 0x124));
            vcall(m, env, o, m.r32(o), 0x20, &[], &[v]);
        }
        4 => {
            let o = rcall(m, env, 0x20, &[], &[]);
            let v = k(m.r32(this + 0x128));
            vcall(m, env, o, m.r32(o), 0x28, &[], &[v]);
        }
        _ => {}
    }
}
fn ocall_with(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, v: u32) {
    let o = rcall(m, env, 0x20, &[], &[]);
    vcall(m, env, o, m.r32(o), off, &[v], &[]);
}

/// `cGameOptionsMenu::cGameOptionsMenu` 0x1875d0(this): a bool (from 0x17f078 on the race), the riders int (1..6,
/// its top the race's riders' count) and a restart command; `this`
pub fn game_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (title, gap, b, i, c) = (this + 0x120, this + 0x138, this + 0x14c, this + 0x170, this + 0x1a0);
    menu_new(m, this);
    m.w32(this + 0x114, GAME_VTABLE);
    null_new(m, title, S_GAME);
    space_new(m, gap, 0xa);
    bool_new(m, b, 0, this + 0x118, S_GAME_BOOL, S_ON, S_OFF);
    int_new(m, i, 1, this + 0x11c, 1, 6, S_GAME_INT);
    command_new(m, c, 2, S_GAME_CMD, 0);
    let v = call(m, env, GET_SETTING, &[race(m)]);
    m.w32(this + 0x11c, 1);
    m.w32(this + 0x118, v);
    let top = m.r32(m.r32(race(m) + 0x1c) + 0x88);
    int_set_range(m, i, 1, top);
    for it in [title, gap, b, i, c] {
        add(m, this, it);
    }
    this
}

/// `cGameOptionsMenu` vf2 0x187750(this, item, argument): the restart (id 2): every rider reset, then the race's
/// table (+8) +0xa0 with the riders int − 1; 2 (else 0)
pub fn game_command(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32, _arg: u32) -> u32 {
    if id(m, item) != 2 {
        return 0;
    }
    let mut i = 0i32;
    while i < m.r32(m.r32(race(m) + 0x1c) + 0x88) as i32 {
        let rd = m.r32(m.r32(race(m) + 0x1c) + 4 * i as u32 + 0xc4);
        call(m, env, RESET_RIDER, &[rd]);
        i += 1;
    }
    let r = m.r32(race(m) + 0x1c);
    let n = m.r32(this + 0x11c).wrapping_sub(1);
    vcall(m, env, r, m.r32(r + 8), 0xa0, &[n], &[]);
    2
}

/// `cGameOptionsMenu` vf3 0x187828(this, item): the bool (id 0) to the race's +0x48
pub fn game_changed(m: &mut Mem, this: u32, item: u32) {
    if id(m, item) == 0 {
        let r = race(m);
        m.w32(r + 0x48, m.r32(this + 0x118));
    }
}

/// `cDebugMenu::cDebugMenu` 0x187850(this): the developer menu: its title, the version line (formatted into
/// 0x337de8), a gap, a back item, three commands and the game, render and sound pages behind sub items (the first
/// command only in a race with +0x40 and not in the career's mode 6); `this`
pub fn debug_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let (title, ver, gap, back) = (this + 0x118, this + 0x130, this + 0x148, this + 0x15c);
    let (c2, c1, s_render, s_game, s_sound, c0) =
        (this + 0x170, this + 0x190, this + 0x1b0, this + 0x1c8, this + 0x1e0, this + 0x1f8);
    let (game, render, sound) = (this + 0x218, this + 0x3d8, this + 0x1968);
    menu_new(m, this);
    m.w32(this + 0x114, DEBUG_VTABLE);
    null_new(m, title, S_DEBUG);
    null_new(m, ver, VERSION_LINE);
    space_new(m, gap, 0xa);
    crate::dmitems::pop_new(m, back, S_BACK);
    command_new(m, c2, 2, S_DEBUG_CMD2, 0);
    command_new(m, c1, 1, S_DEBUG_CMD1, 0);
    sub_new(m, s_render, render, S_RENDER);
    sub_new(m, s_game, game, S_GAME);
    sub_new(m, s_sound, sound, S_SOUND);
    command_new(m, c0, 0, S_DEBUG_CMD0, 0);
    game_new(m, env, game);
    render_new(m, env, render);
    sound_new(m, env, sound);
    call(m, env, SPRINTF, &[VERSION_LINE, S_VERSION_FMT]);
    m.w32(ver + 0x10, VERSION_LINE);
    for it in [title, ver, gap, back] {
        add(m, this, it);
    }
    if m.r32(race(m) + 0x40) != 0 && m.r32(CAREER + 0x2afc) != 6 {
        add(m, this, c2);
    }
    for it in [c1, s_render, s_game, s_sound, c0] {
        add(m, this, it);
    }
    install_stub(this);
    this
}

/// `cDebugMenu::InstallStub` 0x187ad0: nothing
pub fn install_stub(_this: u32) {}

/// `cDebugMenu` vf2 0x187ad8(this, item, argument): the commands: 0 the career saved (its +0x3704 1, +0x3260 2, the
/// race's +0x54 1) — 1; 1 the race quit — 1; 2 the menu closed (the manager inactive) and the race's overlay 9 shown
/// — 0
pub fn debug_command(m: &mut Mem, env: &mut dyn TextBoxEnv, _this: u32, item: u32, _arg: u32) -> u32 {
    match id(m, item) {
        1 => {
            call(m, env, QUIT_RACE, &[race(m)]);
            1
        }
        0 => {
            m.w32(CAREER + 0x3704, 1);
            call(m, env, CAREER_SAVE_A, &[CAREER]);
            m.w32(CAREER + 0x3260, 2);
            let r = race(m);
            m.w32(r + 0x54, 1);
            call(m, env, CAREER_SAVE_B, &[CAREER]);
            1
        }
        2 => {
            manager_set_active(m, MANAGER_ADDR, 0);
            call(m, env, OVERLAY_SHOW, &[m.r32(race(m) + 0x38), 9, 0xffff_ffff]);
            let o = call(m, env, OVERLAY_GET, &[m.r32(race(m) + 0x38)]);
            call(m, env, OVERLAY_SET, &[o, 1]);
            0
        }
        _ => 0,
    }
}

/// `cDebugMenu` vf5 0x187bd0(this, manager): the sound resumed and paused, then entered
pub fn debug_enter(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, mgr: u32) {
    call(m, env, SND_RESUME, &[sys(m)]);
    call(m, env, SND_PAUSE, &[sys(m)]);
    menu_enter(m, env, this, mgr);
}

/// `cDebugMenu` vf6 0x187c20(this): left: when it was the last menu and the race isn't ending (+0x54), a line printed
/// and the sound stopped; the race's overlay 9 shown when its +0x40 has a +0x4c
pub fn debug_leave(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if m.r32(m.r32(this + 4)) == 0 && m.r32(race(m) + 0x54) == 0 {
        // (`Debug_Printf2` 0x188990 with S_CLOSED: it prints nothing)
        debug_printf();
        call(m, env, SND_STOP, &[sys(m)]);
    }
    let o = m.r32(race(m) + 0x40);
    if o == 0 || m.r32(o + 0x4c) == 0 {
        return;
    }
    call(m, env, OVERLAY_SHOW, &[m.r32(race(m) + 0x38), 9, 0xffff_ffff]);
}

/// `cSoundVolumeMenu::cSoundVolumeMenu` 0x187cb8(this): seven volume ints (0..11), three enums and the song enum
/// (its list built from the songs on); the volumes read from the sound system (as ints of its floats) and its
/// preset; `this`
pub fn sound_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let ints = [this + 0x244, this + 0x274, this + 0x2a4, this + 0x2d4, this + 0x304, this + 0x334, this + 0x364];
    let ids = [0u32, 1, 2, 4, 5, 6, 7];
    let (e8, e9, e10, song) = (this + 0x394, this + 0x3b4, this + 0x3d4, this + 0x3f4);
    menu_new(m, this);
    m.w32(this + 0x114, SOUND_VTABLE);
    for i in 0..7 {
        int_new(m, ints[i], ids[i], this + 0x118 + 4 * i as u32, 0, 0xb, S_VOL[i]);
    }
    enum_new(m, e8, 8, this + 0x138, S_SOUND_ENUM[0], 3, L_SOUND_8);
    enum_new(m, e9, 9, this + 0x13c, S_SOUND_ENUM[1], 6, L_SOUND_9);
    enum_new(m, e10, 0xa, this + 0x134, S_SOUND_ENUM[2], 5, L_SOUND_10);
    enum_new1(m, song, 0xb);
    let s = sys(m);
    m.w32(this + 0x13c, m.r32(s + 0x3834));
    read_volumes(m, env, this);
    build_music_list(m, env, this);
    for it in [ints[6], ints[0], ints[1], ints[2], e9, song] {
        add(m, this, it);
    }
    this
}

/// The four volumes the page shows, as ints (channels 0, 1, 4, 2 to +0x130, +0x118, +0x11c, +0x120)
fn read_volumes(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    for (o, ch) in [(0x130u32, 0u32), (0x118, 1), (0x11c, 4), (0x120, 2)] {
        let v = call(m, env, GET_VOL, &[sys(m), ch]);
        m.w32(this + o, crate::sndlib::cvt_w(f32::from_bits(v)) as u32);
    }
}

/// `SoundMenu_BuildMusicList` 0x187f58(this): the songs on as the song enum's list (+0x144, 8 bytes each: the name,
/// its index), the value (+0x140) the first
pub fn build_music_list(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let n = call(m, env, MUSIC_COUNT, &[sys(m)]) as i32;
    let mut i = 0i32;
    while i < n {
        let s = sys(m);
        m.w32(this + 0x148 + 8 * i as u32, i as u32);
        let name = s.wrapping_add(m.r32(s + 0x36e8) << 5).wrapping_add(0x12e0);
        m.w32(this + 0x144 + 8 * i as u32, name);
        call(m, env, MUSIC_NEXT, &[s]);
        i += 1;
    }
    m.w32(this + 0x140, 0);
    call(m, env, MUSIC_SELECT, &[sys(m), 0]);
    let song = this + 0x3f4;
    m.w32(song + 0x14, n as u32);
    m.w32(song + 0x18, this + 0x144);
    m.w32(song + 0x1c, this + 0x140);
    m.w32(song + 0x10, S_SONG);
}

/// `cSoundVolumeMenu` vf1 0x188060(this): the menu's key handling
pub fn sound_handle_input(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    menu_handle_input(m, env, this)
}

/// `cSoundVolumeMenu` vf3 0x188080(this, item): a volume set (its int / 11, inside 0..1, on its channel), the preset
/// set (and the volumes read back), or the song picked and started
pub fn sound_changed(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, item: u32) {
    let vol = |m: &mut Mem, env: &mut dyn TextBoxEnv, o: u32, ch: u32| {
        let v = crate::animseq::cvt_s_w(m.r32(this + o) as i32) * k(0x3dba_2e8c);
        let v = if v.ee() < 0.0 {
            F(0.0)
        } else if 1.0 < v.ee() {
            F(1.0)
        } else {
            v
        };
        fcall(m, env, SET_VOL, &[sys(m), ch], &[v]);
    };
    match id(m, item) {
        0 => vol(m, env, 0x118, 1),
        1 => vol(m, env, 0x11c, 4),
        2 => vol(m, env, 0x120, 2),
        7 => vol(m, env, 0x130, 0),
        9 => {
            call(m, env, SET_PRESET, &[sys(m), m.r32(this + 0x13c)]);
            read_volumes(m, env, this);
        }
        0xb => {
            call(m, env, MUSIC_SELECT, &[sys(m), m.r32(this + 0x140)]);
            call(m, env, START_MUSIC, &[sys(m)]);
        }
        _ => {}
    }
}

/// 0x188310: four static colour records at 0x3cdde8 (as `loadscreen::static_init`'s)
pub fn static_init(m: &mut Mem) {
    let c = STATIC_COLOURS;
    let one = F(1.0).0.to_bits();
    for (o, v) in [
        (0x38u32, 0u32), (0x48, 0x3b03_126f), (0x18, one), (0x40, one), (0x44, 0x3d9b_3d08), (0x30, one), (0x34, 0x3ef2_fec5),
        (0x20, one), (0x24, 0x3f76_e979), (0x28, 0), (0, 0xffff_ffff), (0x10, 0), (4, one), (8, one), (0xc, 0), (0x14, one),
    ] {
        m.w32(c + o, v);
    }
}

/// 0x1883b8(this): the developer menu's sound page
pub fn debug_sound_page(this: u32) -> u32 {
    this + 0x1968
}

fn delete_items(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, offs: &[u32]) {
    for o in offs {
        item_delete(m, env, this + o, 2);
    }
}

/// `cDebugMenu::~cDebugMenu` 0x188410(this, flags)
pub fn debug_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    sound_delete(m, env, this + 0x1968, 2);
    render_delete(m, env, this + 0x3d8, 2);
    game_delete(m, env, this + 0x218, 2);
    delete_items(m, env, this, &[0x1f8, 0x1e0, 0x1c8, 0x1b0, 0x190, 0x170, 0x15c, 0x148, 0x130, 0x118]);
    menu_delete(m, env, this, flags);
}

/// `cSoundVolumeMenu::~cSoundVolumeMenu` 0x188530(this, flags)
pub fn sound_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    delete_items(m, env, this, &[0x3f4, 0x3d4, 0x3b4, 0x394, 0x364, 0x334, 0x304, 0x2d4, 0x2a4, 0x274, 0x244]);
    menu_delete(m, env, this, flags);
}

/// `cGameOptionsMenu::~cGameOptionsMenu` 0x188638(this, flags)
pub fn game_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    delete_items(m, env, this, &[0x1a0, 0x170, 0x14c, 0x138, 0x120]);
    menu_delete(m, env, this, flags);
}

/// `cRenderOptionsMenu::~cRenderOptionsMenu` 0x1886f8(this, flags)
pub fn render_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    delete_items(m, env, this, &[0x1578, 0x1558, 0x1524]);
    resolution_delete(m, env, this + 0x3e0, 2);
    delete_items(m, env, this, &[0x3bc, 0x398, 0x380]);
    bezier_delete(m, env, this + 0x154, 2);
    delete_items(m, env, this, &[0x140, 0x128]);
    menu_delete(m, env, this, flags);
}

/// `cResolutionMenu::~cResolutionMenu` 0x1887f8(this, flags): the 128 commands through their tables' destructors
/// (+8, with 0), last first
pub fn resolution_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    let first = this + 0x144;
    let mut it = this + 0x1144;
    while it != first {
        it -= 0x20;
        let vt = m.r32(it + 0xc);
        vcall(m, env, it, vt, 8, &[0], &[]);
    }
    delete_items(m, env, this, &[0x130, 0x118]);
    menu_delete(m, env, this, flags);
}

/// `cBezierOptionsMenu::~cBezierOptionsMenu` 0x188908(this, flags)
pub fn bezier_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    delete_items(m, env, this, &[0x1f8, 0x1c4, 0x1a0, 0x17c, 0x158, 0x144, 0x12c]);
    menu_delete(m, env, this, flags);
}

/// `Debug_Printf2` 0x188990: nothing (a stripped print)
pub fn debug_printf() {}

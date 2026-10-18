//! The game object (`cGame`, a module the application runs: the race and the screens around it) on the game's
//! bytes (board row E1d): its constructor and destructor, its frame (`cGame::Update` and the results switch that
//! makes the next module), its draw, its pause test and object lists, and `Game_Main`. Every call out goes through
//! [`FeEnv`]. Checked in the function runner (`tests/game_main.rs`).

use crate::fe3dstate::{FeEnv, GAPP};
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out. The game's own: paused (0x17f078), the cameras' update (0x17f390), the views laid out (0x180350),
/// the lesson's update (0x191298), the frame drawn (`Game_DrawFrame` 0x1c72e8), the world's lists' update (0x17d950)
/// and pass (0x17d548) and their destruction (0x17d440). The pause menu's (on 0x337e08): open (0x1895c8), its
/// update (0x1895e8), opened for a player (0x1897a0) and its page (0x1895e0). The overlays' (on +0x38): busy
/// (0x1a77a8), the player (0x1a7878), a page (0x1a7848), the manager (0x1a7be0); the race menu's state (0x1ef990)
/// and its pause (0x1efdb0). The input's pause button (0x17a840 on the app's +0x24), the race's player check
/// (0x110f28). The fades' update (0x1fece8) and draw (0x1ff000) on the app's +0x72c, the sound's tick (0x20ee58 on
/// 0x343050) and the speech's (0x22d370 on 0x34370c). The career's: back to the front end (0x163f08), a two-player
/// game (0x164288), the race's reset (0x1668a0). The app's mode (`App_SetMode` 0x17c6b8), the allocation and fill.
pub const IS_PAUSED: u32 = 0x17f078;
pub const UPDATE_CAMERAS: u32 = 0x17f390;
pub const LAYOUT_VIEWS: u32 = 0x180350;
pub const LESSON_UPDATE: u32 = 0x191298;
pub const DRAW_FRAME: u32 = 0x1c72e8;
pub const LIST_UPDATE: u32 = 0x17d950;
pub const LIST_PASS: u32 = 0x17d548;
pub const LISTS: u32 = 0x337c60;
pub const PAUSE_MENU: u32 = 0x337e08;
pub const PAUSE_OPEN: u32 = 0x1895c8;
pub const PAUSE_UPDATE: u32 = 0x1895e8;
pub const PAUSE_OPEN_FOR: u32 = 0x1897a0;
pub const PAUSE_PAGE: u32 = 0x1895e0;
pub const OVERLAYS_BUSY: u32 = 0x1a77a8;
pub const OVERLAYS_PLAYER: u32 = 0x1a7878;
pub const OVERLAYS_PAGE: u32 = 0x1a7848;
pub const OVERLAYS_MANAGER: u32 = 0x1a7be0;
pub const RACE_MENU_STATE: u32 = 0x1ef990;
pub const RACE_MENU_PAUSE: u32 = 0x1efdb0;
pub const INPUT_PAUSE: u32 = 0x17a840;
pub const RACE_PLAYER: u32 = 0x110f28;
pub const FADE_UPDATE: u32 = 0x1fece8;
pub const FADE_DRAW: u32 = 0x1ff000;
pub const SOUND_TICK: u32 = 0x20ee58;
pub const SOUND: u32 = 0x343050;
pub const SPEECH_TICK: u32 = 0x22d370;
pub const SPEECH: u32 = 0x34370c;
pub const CAREER: u32 = 0x32b390;
pub const CAREER_TO_FRONT_END: u32 = 0x163f08;
pub const CAREER_TWO_PLAYERS: u32 = 0x164288;
pub const APP_SET_MODE: u32 = 0x17c6b8;
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
/// The modules the results switch makes: (the constructor, its size); the front end's takes a flag
pub const FRONT_END_MODULE: (u32, u32) = (0x24c748, 0x1e68);
pub const MODULE_NAME: u32 = 0x3868f8;
pub const RESULTS_MODULES: [(u32, u32); 5] = [(0x24ea20, 0xda8), (0x245840, 0xec4), (0x247a60, 0xd64), (0x249758, 0xf18), (0x24b028, 0x11d8)];
/// The quit module (4 bytes: its table)
pub const QUIT_MODULE: (u32, u32, u32) = (0x3868e8, 4, 0x386900);
pub const GAME_VTABLE: u32 = 0x386980;

fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x724))
}

/// A virtual call through a table at `at` of the object
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn rend(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, off, args)
}

/// `cGame::cGame` 0x17e0f8(g): its table (+8), +0xc 0 and the career's +0x3708 0
pub fn game_ctor(m: &mut Mem, g: u32) -> u32 {
    m.w32(g + 8, GAME_VTABLE);
    m.w32(g + 0xc, 0);
    m.w32(CAREER + 0x3708, 0);
    g
}

/// `Game_UpdateObjectLists` 0x1814c0(g): while paused the three lists at 0x337ca0; otherwise the eleven at 0x337c70,
/// the pass of the one at 0x337cb0 and list 0xc
pub fn update_object_lists(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    if e.call(m, IS_PAUSED, &[g]) != 0 {
        for k in 0..3u32 {
            let l = m.r32(0x337ca0 + 4 * k);
            e.call(m, LIST_UPDATE, &[LISTS, l]);
        }
        return;
    }
    for k in 0..11u32 {
        let l = m.r32(0x337c70 + 4 * k);
        e.call(m, LIST_UPDATE, &[LISTS, l]);
    }
    let l = m.r32(0x337cb0);
    e.call(m, LIST_PASS, &[LISTS, l]);
    e.call(m, LIST_UPDATE, &[LISTS, 0xc]);
}

/// `Game_CanPause` 0x1815a0(g): not while the overlays are busy; by the race's mode (+0x1c, its +0x1c): not in 1 or
/// 6, always in modes but 5, and in 5 only with a lesson (+0x40) whose step (+0x20) is 1..9
pub fn can_pause(m: &mut Mem, e: &mut dyn FeEnv, g: u32) -> u32 {
    let ov = m.r32(g + 0x38);
    if e.call(m, OVERLAYS_BUSY, &[ov]) != 0 {
        return 0;
    }
    let mode = m.r32(m.r32(g + 0x1c) + 0x1c);
    if mode == 1 || mode == 6 {
        return 0;
    }
    if mode != 5 {
        return 1;
    }
    let l = m.r32(g + 0x40);
    if l == 0 {
        return 0;
    }
    let step = m.r32(l + 0x20) as i32;
    (step != 0 && step <= 9) as u32
}

/// The end of a frame: the lists' 9, the cameras, the frame count (+0x50) up
fn frame_end(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    e.call(m, LIST_UPDATE, &[LISTS, 9]);
    e.call(m, UPDATE_CAMERAS, &[g]);
    let n = m.r32(g + 0x50);
    m.w32(g + 0x50, n.wrapping_add(1));
}

/// 0x181878(g): the frame's end alone (the lesson's pause)
pub fn update_paused(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    frame_end(m, e, g);
}

/// `cGame::Update` 0x181638(g; `buf` 4 bytes for the player): the lesson's update, the object lists; the pause menu
/// updated when open; otherwise, when the game can pause: a rider (in a game without a lesson running) whose
/// controller's +4 is clear pauses for its player (its record's +0x65), else the pause button for a player in the
/// race; then the race menu's pause when the overlay manager's page is up, else the pause menu opened for the
/// player with its page (or the overlays' page 6 when it has none); then the frame's end
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, g: u32, buf: u32) {
    let lesson = m.r32(g + 0x40);
    if lesson != 0 {
        e.call(m, LESSON_UPDATE, &[lesson]);
    }
    update_object_lists(m, e, g);
    if e.call(m, PAUSE_OPEN, &[PAUSE_MENU]) != 0 {
        e.call(m, PAUSE_UPDATE, &[PAUSE_MENU]);
        frame_end(m, e, g);
        return;
    }
    if can_pause(m, e, g) == 0 {
        frame_end(m, e, g);
        return;
    }
    m.w32(buf, 0);
    let mut found = false;
    let l = m.r32(g + 0x40);
    if l == 0 || m.r32(l + 0x4c) == 0 {
        let race = m.r32(g + 0x1c);
        let n = m.r32(race + 0x7c) as i32;
        for i in 0..n.max(0) as u32 {
            let r = m.r32(race + 0x8c + 4 * i);
            let c = m.r32(m.r32(r + 0x870));
            if m.r32(c + 4) == 0 {
                found = true;
                let rec = m.r32(m.r32(r) + 0x464);
                m.w32(buf, m.r8(rec + 0x65) as i8 as i32 as u32);
                break;
            }
        }
    }
    let pressed = |m: &mut Mem, e: &mut dyn FeEnv| -> bool {
        let input = m.r32(m.r32(GAPP) + 0x24);
        if e.call(m, INPUT_PAUSE, &[input, buf]) != 0xd {
            return false;
        }
        let race = m.r32(g + 0x1c);
        let p = m.r32(buf);
        e.call(m, RACE_PLAYER, &[race, p]) != 0
    };
    if !found && !pressed(m, e) {
        frame_end(m, e, g);
        return;
    }
    let ovm = m.r32(m.r32(m.r32(GAPP) + 0x730) + 0x38);
    let v = e.call(m, OVERLAYS_MANAGER, &[ovm]);
    if m.r32(m.r32(v + 0x42c) + 0xc) == 0 {
        let p = m.r32(buf);
        e.call(m, PAUSE_OPEN_FOR, &[PAUSE_MENU, p]);
        let ov = m.r32(g + 0x38);
        let p = m.r32(buf);
        e.call(m, OVERLAYS_PLAYER, &[ov, p]);
        if m.r32(PAUSE_MENU) != 0 {
            e.call(m, PAUSE_PAGE, &[PAUSE_MENU, 1]);
        } else {
            let ov = m.r32(g + 0x38);
            e.call(m, OVERLAYS_PAGE, &[ov, 6, u32::MAX]);
        }
    } else {
        if found && !pressed(m, e) {
            frame_end(m, e, g);
            return;
        }
        let ovm = m.r32(m.r32(m.r32(GAPP) + 0x730) + 0x38);
        let v = e.call(m, OVERLAYS_MANAGER, &[ovm]);
        let p = m.r32(buf);
        e.call(m, RACE_MENU_PAUSE, &[v, p]);
    }
    frame_end(m, e, g);
}

/// A module allocated (in 16s), filled and made by its constructor (with `arg` for the front end's)
fn make_module(m: &mut Mem, e: &mut dyn FeEnv, ctor: u32, size: u32, arg: Option<u32>) -> u32 {
    let a = e.call(m, ALLOC, &[MODULE_NAME, size, 0x10]);
    if a != 0 {
        e.call(m, FILL, &[a, 0xdead_c0de, size]);
    }
    match arg {
        Some(x) => e.call(m, ctor, &[a, x]),
        None => e.call(m, ctor, &[a]),
    }
}

/// `Game_UpdateOrShowResults` 0x180e00(g; `buf` as [`update`]'s): while the race runs (+0x54 clear) the fades and the frame (the lesson's
/// pause's when +0x4c); once over, the next module made, once (+0x58): the quit module when the career goes back
/// (its +4 cleared), else by the career's flags (+0x3710, +0x3704, +0x2afc mode 6, +0x370c, +0x3708) and the
/// players: the front end (with 1 or 0), or one of the five results modules
pub fn update_or_show_results(m: &mut Mem, e: &mut dyn FeEnv, g: u32, buf: u32) {
    if m.r32(g + 0x54) == 0 {
        let fades = m.r32(m.r32(GAPP) + 0x72c);
        e.call(m, FADE_UPDATE, &[fades]);
        if m.r32(g + 0x4c) != 0 {
            update_paused(m, e, g);
        } else {
            update(m, e, g, buf);
        }
        return;
    }
    if m.r32(g + 0x58) != 0 {
        return;
    }
    let c = CAREER;
    let app = m.r32(GAPP);
    if e.call(m, CAREER_TO_FRONT_END, &[c]) != 0 {
        m.w32(c + 4, 0);
        let (name, size, vt) = QUIT_MODULE;
        let a = e.call(m, ALLOC, &[name, size, 0x10]);
        if a != 0 {
            e.call(m, FILL, &[a, 0xdead_c0de, size]);
        }
        m.w32(a, vt);
        e.call(m, APP_SET_MODE, &[app, a]);
        m.w32(g + 0x58, 1);
        return;
    }
    let (fe_ctor, fe_size) = FRONT_END_MODULE;
    let module = if m.r32(c + 0x3710) == 1 {
        make_module(m, e, fe_ctor, fe_size, Some(1))
    } else if m.r32(c + 0x3704) == 1 || m.r32(c + 0x2afc) == 6 {
        make_module(m, e, fe_ctor, fe_size, Some(0))
    } else if m.r32(c + 0x370c) == 1 {
        let (k, s) = RESULTS_MODULES[0];
        make_module(m, e, k, s, None)
    } else if m.r32(c + 0x3708) == 1 {
        let (k, s) = if e.call(m, CAREER_TWO_PLAYERS, &[c]) == 1 { RESULTS_MODULES[1] } else { RESULTS_MODULES[2] };
        make_module(m, e, k, s, None)
    } else if m.r32(c + 0x2afc) == 1 {
        make_module(m, e, fe_ctor, fe_size, Some(0))
    } else {
        let (k, s) = if e.call(m, CAREER_TWO_PLAYERS, &[c]) == 1 { RESULTS_MODULES[3] } else { RESULTS_MODULES[4] };
        make_module(m, e, k, s, None)
    };
    let app = m.r32(GAPP);
    e.call(m, APP_SET_MODE, &[app, module]);
    m.w32(g + 0x58, 1);
}

/// The renderer's virtual at `off` handed the renderer itself (as the game leaves it in a1)
fn rend_self(m: &mut Mem, e: &mut dyn FeEnv, off: u32) -> u32 {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, off, &[r])
}

/// An object's destructor through its table at `at` (+8 with 3), when there is one
fn destroy(m: &mut Mem, e: &mut dyn FeEnv, o: u32, at: u32) {
    if o != 0 {
        vcall_at(m, e, o, at, 8, &[3]);
    }
}

/// More calls out: the race's restart (0x187ad0 on +0x2a0), the replay's reset (0x115dc0 on +0x14), the lesson's
/// restart (0x158ea0 on 0x3211a8), the career's race reset (0x1668a0), the speech's reset (0x22e338) and the
/// sound's (0x2116d8)
pub const RACE_RESTART: u32 = 0x187ad0;
pub const REPLAY_RESET: u32 = 0x115dc0;
pub const LESSON_RESTART: u32 = 0x158ea0;
pub const LESSON: u32 = 0x3211a8;
pub const CAREER_RACE_RESET: u32 = 0x1668a0;
pub const SPEECH_RESET: u32 = 0x22e338;
pub const SOUND_START: u32 = 0x2116d8;

/// `cGame::Restart` 0x180c70(g): the race restarted, its +0x78 (1, 0; its table at +8), the replay reset; with a
/// lesson the career's race reset and, when the lesson runs (+0x4c), its restart (1, 1, 0, 0)
pub fn restart(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    let r = m.r32(g + 0x2a0);
    e.call(m, RACE_RESTART, &[r]);
    let race = m.r32(g + 0x1c);
    vcall_at(m, e, race, 8, 0x78, &[1, 0]);
    let rp = m.r32(g + 0x14);
    e.call(m, REPLAY_RESET, &[rp]);
    if m.r32(g + 0x40) == 0 {
        return;
    }
    e.call(m, CAREER_RACE_RESET, &[CAREER]);
    if m.r32(m.r32(g + 0x40) + 0x4c) != 0 {
        e.call(m, LESSON_RESTART, &[LESSON, 1, 1, 0, 0]);
    }
}

/// `cGame` vf4 0x180d08(g), the enter: the renderer's +0x68 with the view at +0x5c and its +0x140, the speech and
/// sound started; with a running lesson the overlays' player (the word at 0x32e5ec) and page 9 and the race menu's
/// state 1; the views laid out, +0xc 1 and +0x6c the app's +0x10
pub fn enter(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    rend(m, e, 0x68, &[g + 0x5c]);
    let r = renderer(m);
    let vt = m.r32(r + 0x10);
    let leftover = m.r32(vt + 0x144);
    rend(m, e, 0x140, &[leftover]);
    let sp = m.r32(SPEECH);
    e.call(m, SPEECH_RESET, &[sp]);
    let so = m.r32(SOUND);
    e.call(m, SOUND_START, &[so]);
    let l = m.r32(g + 0x40);
    if l != 0 && m.r32(l + 0x4c) != 0 {
        let ov = m.r32(g + 0x38);
        let p = m.r32(0x32e5ec);
        e.call(m, OVERLAYS_PLAYER, &[ov, p]);
        let ov = m.r32(g + 0x38);
        e.call(m, OVERLAYS_PAGE, &[ov, 9, u32::MAX]);
        let ov = m.r32(g + 0x38);
        let v = e.call(m, OVERLAYS_MANAGER, &[ov]);
        e.call(m, RACE_MENU_STATE, &[v, 1]);
    }
    e.call(m, LAYOUT_VIEWS, &[g]);
    m.w32(g + 0xc, 1);
    let t = m.r32(m.r32(GAPP) + 0x10);
    m.w32(g + 0x6c, t);
}

/// 0x180df0(g), the leave: +0xc cleared; 1
pub fn leave(m: &mut Mem, g: u32) -> u32 {
    m.w32(g + 0xc, 0);
    1
}

/// `cvt.s.w`: toward zero
fn int_float(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

/// An unsigned word as a float (`cvt.s.w` of half with the low bit kept, doubled, past 2^31)
fn unsigned_float(v: u32) -> F {
    if (v as i32) >= 0 {
        int_float(v)
    } else {
        let h = int_float((v >> 1) | (v & 1));
        h + h
    }
}

/// The sound's tick with the frames since the last (+0x50) in seconds, the count cleared
fn sound_tick(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    let t = int_float(m.r32(g + 0x50)) / F(60.0);
    let so = m.r32(SOUND);
    e.call(m, SOUND_TICK, &[so, 1, t.0.to_bits()]);
    m.w32(g + 0x50, 0);
}

/// More calls out for the draw: the font state's colour (0x181a48) and size (0x19b108) on 0x337fe8
pub const FONT_COLOUR: u32 = 0x181a48;
pub const FONT_SIZE: u32 = 0x19b108;
pub const FONT_STATE: u32 = 0x337fe8;

/// `cGame` vf6 0x1811a0(g; `buf` 0x10 bytes), the draw -> whether it drew: nothing unless the renderer is ready
/// (+0x70); while racing the fades, the frame, the sound and the speech; otherwise the results' screen set up: the
/// viewport over the whole display (its +0x58 by +0x60, unsigned), the depth range (0, 0, 1, 1, 0, 1000), the
/// clear, fog 0.08 at state 2, state 6, the font's scale 0.01, texture mode 3, the alpha, the font black and sized 1
/// and the frame begun; then the sound
pub fn draw(m: &mut Mem, e: &mut dyn FeEnv, g: u32, buf: u32) -> u32 {
    if rend_self(m, e, 0x70) == 0 {
        return 0;
    }
    if m.r32(g + 0x54) == 0 {
        let fades = m.r32(m.r32(GAPP) + 0x72c);
        e.call(m, FADE_DRAW, &[fades]);
        e.call(m, DRAW_FRAME, &[g]);
        sound_tick(m, e, g);
        let sp = m.r32(SPEECH);
        e.call(m, SPEECH_TICK, &[sp]);
        return 1;
    }
    let w = unsigned_float(rend(m, e, 0x58, &[]));
    let h = unsigned_float(rend(m, e, 0x60, &[]));
    let (z, one) = (0u32, 0x3f80_0000u32);
    rend(m, e, 0x98, &[z, z, w.0.to_bits(), h.0.to_bits()]);
    rend(m, e, 0xb0, &[z, z, one, one, z, F(1000.0).0.to_bits()]);
    rend(m, e, 0x108, &[]);
    rend(m, e, 0x1c8, &[2, 0, 0x3da3_d70a]);
    rend(m, e, 0x1d0, &[6]);
    m.w32(FONT_STATE + 0x10, 0x3c23_d70a);
    rend(m, e, 0x1e0, &[3]);
    rend(m, e, 0x1f0, &[u32::MAX, 0]);
    for k in 0..3u32 {
        m.w32(buf + 4 * k, 0);
    }
    e.call(m, FONT_COLOUR, &[FONT_STATE, buf]);
    e.call(m, FONT_SIZE, &[FONT_STATE, z, z, one, one]);
    rend(m, e, 0x80, &[]);
    sound_tick(m, e, g);
    1
}

/// 0x1818c0: the renderer's +0x108 and the font's scale 0.002
pub fn results_font(m: &mut Mem, e: &mut dyn FeEnv) {
    rend_self(m, e, 0x108);
    m.w32(FONT_STATE + 0x10, 0x3b03_126f);
}

/// `cGameModule::~cGameModule` 0x181eb0(o, flags): its table put back, and freed when bit 0 is set
pub const MODULE_VTABLE: u32 = 0x386f80;
pub const FREE: u32 = 0x114f80;

pub fn module_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, MODULE_VTABLE);
    if flags & 1 != 0 {
        e.call(m, FREE, &[o]);
    }
}

/// 0x182060 and 0x182068 (the modules' vf4): nothing
pub fn nothing() {}

/// The destructor's calls out: the sound's stops (0x211960, 0x211b18), the lesson's destructor (0x18e9e8), the
/// fonts' release (0x19c8d0), the replay's stop (0x115de8), the race's free (0x1802b8), the world's (0x2575b0 on
/// +0x24) and the riders' (0x259028 on +0x28) destruction, the tricks' (0x1545b8), the world lists' (0x17d440), the
/// pool's reset (0x23d368), the HUD's destructor (0x14fa00), the course's unload (0x25f498), the effects'
/// (0x145ae8), the lesson object's reset (0x159070, 0x158cc8)
pub const SOUND_STOP: [u32; 2] = [0x211960, 0x211b18];
pub const LESSON_DTOR: u32 = 0x18e9e8;
pub const FONT_RELEASE: u32 = 0x19c8d0;
pub const REPLAY_STOP: u32 = 0x115de8;
pub const RACE_FREE: u32 = 0x1802b8;
pub const WORLD_DTOR: u32 = 0x2575b0;
pub const RIDERS_DTOR: u32 = 0x259028;
pub const TRICKS_FREE: u32 = 0x1545b8;
pub const LISTS_DTOR: u32 = 0x17d440;
pub const POOL_RESET: u32 = 0x23d368;
pub const POOL: u32 = 0x345e48;
pub const HUD_DTOR: u32 = 0x14fa00;
pub const COURSE_UNLOAD: u32 = 0x25f498;
pub const WORLD: u32 = 0x346488;
pub const EFFECTS_FREE: u32 = 0x145ae8;
pub const LESSON_RESET: [u32; 2] = [0x159070, 0x158cc8];

/// Until the renderer is idle (its +0x78)
fn wait_idle(m: &mut Mem, e: &mut dyn FeEnv) {
    while rend_self(m, e, 0x78) == 0 {}
}

/// `cGame` vf0 0x17ecf8(g, flags; `buf` 0x10 bytes), the destructor: its table put back, the app's mode 0, the
/// renderer waited for; the speech reset and the sound stopped; the lesson, the overlays (+0x3c cleared), the pause
/// menu's +0x38, the fonts; the renderer's +0x148, mode 1 and its +0x68 with 0; the replay stopped and destroyed,
/// +0x18, the race (its +0x70 first), the race's free, +0x44, the world (+0x24, +0x2c), the riders (+0x28), +0x20,
/// the tricks, +0x34, +0x80; the world lists; the renderer waited for again; the pool, the HUD (+0x30), the effects
/// loader, the course and the effects; +0x2a0 (its table at +0x114); the renderer's +0x6f0 1 and the lesson object
/// reset; mode 2, the app's +0x730 cleared, the module's part, and freed when bit 0 is set
pub fn game_dtor(m: &mut Mem, e: &mut dyn FeEnv, g: u32, flags: u32, buf: u32) {
    m.w32(g + 8, GAME_VTABLE);
    let app = m.r32(GAPP);
    m.w32(app + 8, 0);
    wait_idle(m, e);
    let sp = m.r32(SPEECH);
    e.call(m, SPEECH_RESET, &[sp]);
    for f in SOUND_STOP {
        let so = m.r32(SOUND);
        e.call(m, f, &[so]);
    }
    let l = m.r32(g + 0x40);
    if l != 0 {
        e.call(m, LESSON_DTOR, &[l, 3]);
    }
    let ov = m.r32(g + 0x38);
    m.w32(g + 0x3c, 0);
    destroy(m, e, ov, 8);
    m.w32(PAUSE_MENU + 0x38, 0);
    for at in [0x5cu32, 0x60] {
        let f = m.r32(m.r32(GAPP) + at);
        e.call(m, FONT_RELEASE, &[f]);
    }
    rend_self(m, e, 0x148);
    let app = m.r32(GAPP);
    m.w32(app + 8, 1);
    for k in 0..3u32 {
        m.w32(buf + 4 * k, 0);
    }
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0x68, &[buf]);
    let rp = m.r32(g + 0x14);
    e.call(m, REPLAY_STOP, &[rp]);
    let rp = m.r32(g + 0x14);
    destroy(m, e, rp, 8);
    let x = m.r32(g + 0x18);
    destroy(m, e, x, 8);
    let race = m.r32(g + 0x1c);
    vcall_at(m, e, race, 8, 0x70, &[race]);
    let race = m.r32(g + 0x1c);
    destroy(m, e, race, 8);
    e.call(m, RACE_FREE, &[g]);
    let x = m.r32(g + 0x44);
    destroy(m, e, x, 8);
    let w = m.r32(g + 0x24);
    e.call(m, WORLD_DTOR, &[w]);
    let w = m.r32(g + 0x24);
    e.call(m, FREE, &[w]);
    m.w32(g + 0x2c, 0);
    let rd = m.r32(g + 0x28);
    e.call(m, RIDERS_DTOR, &[rd]);
    let rd = m.r32(g + 0x28);
    e.call(m, FREE, &[rd]);
    let x = m.r32(g + 0x20);
    m.w32(g + 0x28, 0);
    destroy(m, e, x, 8);
    e.call(m, TRICKS_FREE, &[]);
    let x = m.r32(g + 0x34);
    destroy(m, e, x, 8);
    let x = m.r32(g + 0x80);
    destroy(m, e, x, 8);
    m.w32(g + 0x80, 0);
    e.call(m, LISTS_DTOR, &[LISTS]);
    wait_idle(m, e);
    e.call(m, POOL_RESET, &[POOL]);
    let hud = m.r32(g + 0x30);
    if hud != 0 {
        e.call(m, HUD_DTOR, &[hud, 3]);
    }
    let app = m.r32(GAPP);
    crate::app::free_effects_loader(m, e, app);
    e.call(m, COURSE_UNLOAD, &[WORLD, 0]);
    e.call(m, EFFECTS_FREE, &[]);
    let x = m.r32(g + 0x2a0);
    destroy(m, e, x, 0x114);
    let r = renderer(m);
    m.w32(r + 0x6f0, 1);
    let app = m.r32(GAPP);
    e.call(m, LESSON_RESET[0], &[LESSON, app]);
    e.call(m, LESSON_RESET[1], &[LESSON]);
    let app = m.r32(GAPP);
    m.w32(app + 8, 2);
    m.w32(app + 0x730, 0);
    module_dtor(m, e, g + 8, 0);
    if flags & 1 != 0 {
        e.call(m, FREE, &[g]);
    }
}

/// `Game_Main` 0x1820b0(argc, argv) -> the run's answer: the C library's set-up (0x2f44c0), the execution manager
/// made (0x182ba8), the application (0x7cc bytes, zeroed) made on it and run (`App_Run` 0x17c570), then destroyed
pub const LIBC_INIT: u32 = 0x2f44c0;
pub const EXEC_CREATE: u32 = 0x182ba8;
pub const APP_RUN: u32 = 0x17c570;
pub const APP_NAME: u32 = 0x3871a8;

pub fn game_main(m: &mut Mem, e: &mut dyn FeEnv, argc: u32, argv: u32) -> u32 {
    e.call(m, LIBC_INIT, &[argc, argv]);
    let exec = e.call(m, EXEC_CREATE, &[]);
    let a = e.call(m, ALLOC, &[APP_NAME, 0x7cc, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0, 0x7cc]);
    }
    let app = crate::app::app_ctor(m, e, a, exec);
    let r = e.call(m, APP_RUN, &[app, argc, argv]);
    if app != 0 {
        vcall_at(m, e, app, 0x2c, 8, &[3]);
    }
    r
}

/// `cGame::Init`'s calls out (board row E1h): the world's reset (0x257630 on 0x346480) and its lists' (0x17d2f0,
/// 0x17dae0), the input's device (0x17a830), the game's seed (0x23d4c8), the pool's set up (0x23d160 on 0x345e48),
/// the lesson's (0x158d18), a world node made with its type (0x17cf50), two lists (0x23bd10) and a third (0x201b50)
/// in the race, the riders (0x18e850), the world (0x256188), the tricks (0x14f960), the courses (0x257dc0), the replay
/// (0x115db8), the cameras (0x180030), the HUD's fades (0x1ffd80, 0x1ffe70), the overlays (0x1d8e30), the snow
/// (0x1c8db8, its rate 0x1c90d0 and drift 0x1c91c0), the pause menu (0x1895b0), the course's world (0x25edf0 on
/// 0x346488), the heap's mark (0x2cdfd8), the course's music (0x224cf8), the tricks' table (0x154580), the speech
/// (0x1a6fb8), the lesson's helper (0x14aaa0), the replay's camera (0x1c7900), the controllers (0x1ea408), the
/// shadows (0x1c9b20), the effects (0x1458e0), the HUD option and viewport mode (0x17f158 / 0x17f168, 0x17f190 /
/// 0x17f1a0), the brightness (0x17b970, 0x17b980), the race menu (0x187850), the sound's start (0x211098)
pub mod init {
    pub const WORLD_RESET: u32 = 0x257630;
    pub const WORLD: u32 = 0x346480;
    pub const DEVICE: u32 = 0x17a830;
    pub const SEED_GAME: u32 = 0x23d4c8;
    pub const POOL_INIT: u32 = 0x23d160;
    pub const POOL: u32 = 0x345e48;
    pub const POOL_NAME: u32 = 0x386748;
    pub const LISTS_INIT: u32 = 0x17d2f0;
    pub const LISTS_LAYERS: u32 = 0x17dae0;
    pub const LESSON_INIT: u32 = 0x158d18;
    pub const LESSON_FUNC: u32 = 0x16e360;
    pub const NODE_CTOR: u32 = 0x17cf50;
    pub const LIST_CTOR: u32 = 0x23bd10;
    pub const RACE_LIST_CTOR: u32 = 0x201b50;
    pub const RIDERS_CTOR: u32 = 0x18e850;
    pub const WORLD_CTOR: u32 = 0x256188;
    pub const TRICKS_CTOR: u32 = 0x14f960;
    pub const COURSES_CTOR: u32 = 0x257dc0;
    pub const REPLAY_INIT: u32 = 0x115db8;
    pub const CAMERAS: u32 = 0x180030;
    pub const HUD_CTOR: u32 = 0x1ffd80;
    pub const HUD_SIZE: u32 = 0x1ffe70;
    pub const OVERLAYS_CTOR: u32 = 0x1d8e30;
    pub const RAND_FX: u32 = 0x23d4a8;
    pub const SNOW_CTOR: u32 = 0x1c8db8;
    pub const SNOW_RATE: u32 = 0x1c90d0;
    pub const SNOW_DRIFT: u32 = 0x1c91c0;
    pub const PAUSE_INIT: u32 = 0x1895b0;
    pub const COURSE_WORLD: u32 = 0x25edf0;
    pub const COURSE_WORLD_OBJ: u32 = 0x346488;
    pub const HEAP_MARK: u32 = 0x2cdfd8;
    pub const MUSIC: u32 = 0x224cf8;
    pub const TRICK_TABLE: u32 = 0x154580;
    pub const SPEECH_CTOR: u32 = 0x1a6fb8;
    pub const HELPER_CTOR: u32 = 0x14aaa0;
    pub const REPLAY_CAM_CTOR: u32 = 0x1c7900;
    pub const CONTROLLERS: u32 = 0x1ea408;
    pub const SHADOWS_CTOR: u32 = 0x1c9b20;
    pub const EFFECTS: u32 = 0x1458e0;
    pub const HUD_OPTION_GET: u32 = 0x17f158;
    pub const HUD_OPTION_SET: u32 = 0x17f168;
    pub const VIEW_MODE_GET: u32 = 0x17f190;
    pub const VIEW_MODE_SET: u32 = 0x17f1a0;
    pub const BRIGHTNESS_GET: u32 = 0x17b970;
    pub const BRIGHTNESS_SET: u32 = 0x17b980;
    pub const RACE_MENU_CTOR: u32 = 0x187850;
    pub const SOUND_START: u32 = 0x211098;
    pub const FONT_LOAD: u32 = 0x19c858;
    pub const STRCPY: u32 = 0x2fd14c;
    /// The pools' names, in the order the objects are made
    pub const NAMES: [u32; 18] = [
        0x3867a8, 0x3867b0, 0x3867b8, 0x3867c0, 0x3867d0, 0x3867e0, 0x3867f0, 0x386800, 0x386810, 0x386818, 0x386830,
        0x386870, 0x386880, 0x386890, 0x3868a0, 0x3868a8, 0x3868b8, 0x3868c0,
    ];
    pub const FONT_NAMES: [u32; 2] = [0x386840, 0x386858];
    /// The race's tables: its own (+8) and its parts' (+0x24..+0x5c)
    pub const RACE_TABLES: [(u32, u32); 12] = [
        (8, 0x364738), (0x24, 0x364860), (0x2c, 0x364820), (0x30, 0x364840), (0x34, 0x364800), (0x38, 0x3647e0),
        (0x48, 0x36e780), (0x4c, 0x36e760), (0x50, 0x36e740), (0x54, 0x364e48), (0x58, 0x36e720), (0x5c, 0x364e28),
    ];
}

/// A block from pool `name` (`fill` its words when there is one)
fn block(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32, fill: u32) -> u32 {
    let a = e.call(m, ALLOC, &[name, size, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, fill, size]);
    }
    a
}

/// A world node from pool `name` made with `kind`, given the table `vt`
fn node(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32, fill: u32, kind: u32, vt: u32) -> u32 {
    let a = block(m, e, name, size, fill);
    e.call(m, init::NODE_CTOR, &[a, kind]);
    m.w32(a + 8, vt);
    a
}

/// A random float in [0, 1) from the effects' stream: its 23 low bits as a mantissa over 1, less 1, plus 0
fn fx_unit(m: &mut Mem, e: &mut dyn FeEnv) -> F {
    let r = e.call(m, init::RAND_FX, &[]);
    F(f32::from_bits(r & 0x7f_ffff | 0x3f80_0000)) - F(1.0) + F(0.0)
}

/// `cGame::Init` 0x17e148(g; `buf` 0x20 bytes), the race set up: the app keeps the game (+0x730) and counts its
/// steps (+8, 0 to 10). The world reset, the renderer's +0x138, both pads' vibration from the career's settings
/// (+0x1678, +0x167c), the game's seed (the career's +0x2b04), the pool, the world's lists and the lesson; then the
/// objects in the game's order: the replay node (+0x14), the race (+0x1c, its tables and lists), a node (+0x18), the
/// riders (+0x40, none in mode 6; with the riders the career's list counts), the world (+0x24), the tricks (+0x30),
/// the course (+0x10, from the course table), the courses (+0x28; this one's +0x2c and its fog colour /255 at +0x5c),
/// the replay, the race's +0x58, the cameras, the effects loader, the HUD's fades (+0x44), a node (+0x20), the
/// overlays (+0x80), the attract flag (+0x74: a random draw under the course's +0x54, outside modes 2 and 4 and with
/// one player and fewer than three AI), the snow (its rate 0.05..0.2 at +0x78, its drift), the two fonts sized, the
/// pause menu, the course's archive and world, the music by the course's name upper-cased, the speech (+0x38), the
/// lesson's helper (+0x34, mode 6), three more objects, the controllers (+0x3c), the shadows, the HUD option, the
/// brightness again, the viewport mode, the archives closed, the race menu (+0x2a0), the restart and the sound
pub fn game_init(m: &mut Mem, e: &mut dyn FeEnv, g: u32, buf: u32) {
    use init::*;
    let app = m.r32(GAPP);
    let c = CAREER;
    let s6 = c + 0x27dc;
    m.w32(app + 0x730, g);
    m.w32(app + 8, 0);
    e.call(m, WORLD_RESET, &[WORLD]);
    rend(m, e, 0x138, &[0]);
    let step = |m: &mut Mem, n: u32| {
        let app = m.r32(GAPP);
        m.w32(app + 8, n);
    };
    for k in 0..2u32 {
        let app = m.r32(GAPP);
        if k == 0 {
            m.w32(app + 8, 1);
        }
        let input = m.r32(app + 0x24);
        let d = e.call(m, DEVICE, &[input, k]);
        let on = (m.r32(c + 0x1678 + 4 * k) != 0) as u32;
        vcall_at(m, e, d, 0x38, 0x30, &[on]);
    }
    m.w32(g + 0x68, 0);
    m.w32(g + 0x48, 0);
    m.w32(g + 0x70, 0xbf80_0000);
    let seed = m.r32(s6 + 0x328);
    e.call(m, SEED_GAME, &[seed]);
    m.w32(g + 0x54, 0);
    step(m, 2);
    m.w32(g + 0x58, 0);
    m.w32(g + 0x50, 0);
    e.call(m, POOL_INIT, &[POOL, 0xc, POOL_NAME]);
    e.call(m, LISTS_INIT, &[LISTS, 0xe]);
    e.call(m, LISTS_LAYERS, &[LISTS, 3]);
    e.call(m, LESSON_INIT, &[LESSON, LESSON_FUNC]);
    let replay = block(m, e, NAMES[0], 0x51c, 0);
    e.call(m, NODE_CTOR, &[replay, 2]);
    m.w32(g + 0x14, replay);
    m.w32(replay + 8, 0x365798);
    let race = block(m, e, NAMES[1], 0x504, 0);
    e.call(m, NODE_CTOR, &[race, 7]);
    for (at, vt) in RACE_TABLES {
        m.w32(race + at, vt);
    }
    e.call(m, LIST_CTOR, &[race + 0xfc]);
    e.call(m, LIST_CTOR, &[race + 0x110]);
    e.call(m, RACE_LIST_CTOR, &[race + 0x124]);
    m.w32(g + 0x1c, race);
    let n18 = block(m, e, NAMES[2], 0xc, 0);
    e.call(m, NODE_CTOR, &[n18, 5]);
    m.w32(g + 0x18, n18);
    m.w32(n18 + 8, 0x3646e8);
    let n = m.r32(c + 0x27dc) as i32;
    let mut listed = 0u32;
    for k in 0..n.max(0) as u32 {
        if (m.r8(s6 + 0x84 * k + 0x69) as i8) >= 0 {
            listed += 1;
        }
    }
    if m.r32(s6 + 0x320) != 6 {
        let a = block(m, e, NAMES[3], 0x40fc, 0);
        let count = m.r32(s6);
        let x = e.call(m, RIDERS_CTOR, &[a, count, listed, 4]);
        m.w32(g + 0x40, x);
    } else {
        m.w32(g + 0x40, 0);
    }
    let w = block(m, e, NAMES[4], 0x7d0, 0);
    m.w32(g + 0x24, w);
    e.call(m, WORLD_CTOR, &[w]);
    let a = block(m, e, NAMES[5], 0x44, 0);
    let x = e.call(m, TRICKS_CTOR, &[a]);
    let app = m.r32(GAPP);
    let id = m.r32(s6 + 0x31c);
    m.w32(g + 0x30, x);
    let course = crate::app::table_find_by_id(m, app + 0x64, id);
    m.w32(g + 0x10, course);
    step(m, 3);
    let cs = block(m, e, NAMES[6], 0x784, 0);
    m.w32(g + 0x28, cs);
    e.call(m, COURSES_CTOR, &[cs]);
    let k = m.r32(c + 0x2af8) as i32;
    let k = if k < 0 { 0 } else { k.min(12) } as u32;
    let entry = m.r32(g + 0x28).wrapping_add(0x94 * k);
    m.w32(g + 0x2c, entry);
    let fog = |m: &Mem, off: u32| int_float(m.r32(entry + off)) / F(255.0);
    let (r, gr, b) = (fog(m, 0x48), fog(m, 0x4c), fog(m, 0x50));
    m.w32(g + 0x5c, r.0.to_bits());
    m.w32(g + 0x60, gr.0.to_bits());
    m.w32(g + 0x64, b.0.to_bits());
    let rp = m.r32(g + 0x14);
    e.call(m, REPLAY_INIT, &[rp]);
    let race = m.r32(g + 0x1c);
    vcall_at(m, e, race, 8, 0x58, &[]);
    m.w32(g + 0x29c, 0);
    e.call(m, CAMERAS, &[g]);
    let app = m.r32(GAPP);
    m.w32(app + 8, 4);
    crate::app::create_effects_loader(m, e, app);
    let a = block(m, e, NAMES[7], 0x28, 0xdead_c0de);
    let hud = e.call(m, HUD_CTOR, &[a, 2]);
    m.w32(g + 0x44, hud);
    e.call(m, HUD_SIZE, &[hud, 0x80, 0x40]);
    let n20 = block(m, e, NAMES[8], 0xc, 0xdead_c0de);
    e.call(m, NODE_CTOR, &[n20, 0xb]);
    m.w32(g + 0x20, n20);
    m.w32(n20 + 8, 0x364698);
    let a = block(m, e, NAMES[9], 0xb60, 0xdead_c0de);
    let ov = e.call(m, OVERLAYS_CTOR, &[a, 2]);
    m.w32(g + 0x80, ov);
    let mode = m.r32(s6 + 0x320);
    let race = m.r32(g + 0x1c);
    let attract = if mode == 2 || mode == 4 || m.r32(race + 0x7c) as i32 >= 2 || m.r32(race + 0x88) as i32 >= 3 {
        0
    } else {
        let f = fx_unit(m, e);
        let lim = F(f32::from_bits(m.r32(m.r32(g + 0x2c) + 0x54)));
        (f.ee() < lim.ee()) as u32
    };
    m.w32(g + 0x74, attract);
    let a = block(m, e, NAMES[10], 0x4d0, 0xdead_c0de);
    let snow = e.call(m, SNOW_CTOR, &[a, 2]);
    let rate = fx_unit(m, e) * F(f32::from_bits(0x3e19_999a)) + F(f32::from_bits(0x3d4c_cccd));
    m.w32(g + 0x78, rate.0.to_bits());
    e.call(m, SNOW_RATE, &[snow, rate.0.to_bits()]);
    let x = fx_unit(m, e) + F(5.0);
    let y = F(7.0) - fx_unit(m, e);
    e.call(m, SNOW_DRIFT, &[snow, x.0.to_bits(), y.0.to_bits()]);
    step(m, 5);
    let app = m.r32(GAPP);
    let f0 = m.r32(app + 0x5c);
    e.call(m, FONT_LOAD, &[f0, FONT_NAMES[0], 0]);
    let app = m.r32(GAPP);
    let f0 = m.r32(app + 0x5c);
    let f1 = m.r32(app + 0x60);
    m.w32(f0 + 0x1c, 0x3fa6_6666);
    m.w32(f0 + 0x18, 0x3fb3_3333);
    e.call(m, FONT_LOAD, &[f1, FONT_NAMES[1], 0]);
    let f1 = m.r32(m.r32(GAPP) + 0x60);
    m.w32(f1 + 0x1c, 0x3fb3_3333);
    m.w32(f1 + 0x18, 0x3fe6_6666);
    e.call(m, PAUSE_INIT, &[PAUSE_MENU]);
    let app = m.r32(GAPP);
    let course = m.r32(g + 0x10);
    let f1 = m.r32(app + 0x60);
    m.w32(PAUSE_MENU + 0x38, f1);
    crate::app::course_open_big(m, e, app, course);
    step(m, 6);
    let course = m.r32(g + 0x10);
    e.call(m, COURSE_WORLD, &[COURSE_WORLD_OBJ, course]);
    e.call(m, HEAP_MARK, &[]);
    step(m, 7);
    let app = m.r32(GAPP);
    let course = m.r32(g + 0x10);
    let name = crate::app::table_info(app + 0x64, course);
    let s = buf + 0x10;
    e.call(m, STRCPY, &[s, name]);
    let mut p = s;
    while m.r8(p) != 0 {
        let ch = m.r8(p);
        let up = if ch.wrapping_sub(0x61) < 0x1a { ch.wrapping_sub(0x20) } else { ch };
        m.w8(p, up);
        p += 1;
    }
    let so = m.r32(SOUND);
    e.call(m, MUSIC, &[so, s]);
    e.call(m, TRICK_TABLE, &[]);
    step(m, 8);
    let a = block(m, e, NAMES[11], 0x5c, 0xdead_c0de);
    let sp = e.call(m, SPEECH_CTOR, &[a, 0xc]);
    m.w32(g + 0x38, sp);
    if m.r32(s6 + 0x320) == 6 {
        let a = block(m, e, NAMES[12], 0x540, 0xdead_c0de);
        let race = m.r32(g + 0x1c);
        let x = e.call(m, HELPER_CTOR, &[a, race + 0x124, 6]);
        m.w32(g + 0x34, x);
    } else {
        m.w32(g + 0x34, 0);
    }
    node(m, e, NAMES[13], 0xc, 0xdead_c0de, 1, 0x390c48);
    let a = block(m, e, NAMES[14], 0x10, 0xdead_c0de);
    e.call(m, REPLAY_CAM_CTOR, &[a, 0]);
    node(m, e, NAMES[15], 0xc, 0xdead_c0de, 0, 0x390c98);
    let x = e.call(m, CONTROLLERS, &[1, 0]);
    m.w32(g + 0x3c, x);
    let a = block(m, e, NAMES[16], 0x110, 0xdead_c0de);
    e.call(m, SHADOWS_CTOR, &[a, 1]);
    e.call(m, EFFECTS, &[]);
    let v = e.call(m, HUD_OPTION_GET, &[g]);
    e.call(m, HUD_OPTION_SET, &[g, v]);
    let app = m.r32(GAPP);
    let br = e.call(m, BRIGHTNESS_GET, &[app]);
    e.call(m, BRIGHTNESS_SET, &[app, br]);
    let v = e.call(m, VIEW_MODE_GET, &[g]);
    e.call(m, VIEW_MODE_SET, &[g, v]);
    let app = m.r32(GAPP);
    m.w32(app + 8, 9);
    crate::app::course_close_big(m, e, app);
    let a = block(m, e, NAMES[17], 0x1d7c, 0);
    let x = e.call(m, RACE_MENU_CTOR, &[a]);
    m.w32(g + 0x2a0, x);
    restart(m, e, g);
    let so = m.r32(SOUND);
    e.call(m, SOUND_START, &[so]);
    step(m, 10);
}

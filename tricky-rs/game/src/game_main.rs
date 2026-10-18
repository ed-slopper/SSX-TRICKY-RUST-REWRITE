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

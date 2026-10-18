//! The front end's 3D scene code below 0x26c000 (board rows G9z1..) on the game's bytes. First the screens' audio,
//! voice-line and world helpers (0x261b38..0x263270): they go through the front end's sound player (`cFE3D` +0xa4,
//! its table at +0), its world (+0x80, found by name hash), its riders (+0x20, twelve) and its course films (+0xfc,
//! ten). Then the characters (`cFE3DCharacter`: the rider model, its loads, its poses and lights) and the dialog box
//! (`cFE3DDialog`) with the group's own virtuals, then the geometry, its films and the animations, and the cameras.
//! Every call out goes through [`FeEnv`]. Checked in the function runner (`tests/fe_scene.rs`, `tests/fe_scene_char.rs`,
//! `tests/fe_scene_dialog.rs`, `tests/fe_scene_anim.rs`, `tests/fe_scene_cam.rs`).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: a world script run by name (0x13bad8), a name's hash (0x240ed8), the world's thing by hash
/// (0x26fe50), the camera's mode (0x26be38), the career's medal (0x163e28), a film opened (0x1fff30) and released
/// (0x200098)
pub const RUN_SCRIPT: u32 = 0x13bad8;
pub const NAME_HASH: u32 = 0x240ed8;
pub const FIND_BY_HASH: u32 = 0x26fe50;
pub const CAMERA_SET_MODE: u32 = 0x26be38;
pub const MEDAL: u32 = 0x163e28;
pub const VIDEO_OPEN: u32 = 0x1fff30;
pub const VIDEO_RELEASE: u32 = 0x200098;
/// The career, the riders' world names (the picture and the marker, 12 each), the venues' (10: their medal marks),
/// the venues' scripts (8 bytes each: open, close), the course films' names (10), the venues' music events (bytes)
pub const CAREER: u32 = 0x32b390;
pub const RIDER_NAMES: u32 = 0x3470c8;
pub const MARKER_NAMES: u32 = 0x347100;
pub const MEDAL_NAMES: u32 = 0x347158;
pub const VENUE_SCRIPTS: u32 = 0x347180;
pub const FILM_NAMES: u32 = 0x3471d0;
pub const VENUE_MUSIC: u32 = 0x3a8af0;
/// The world panels' scripts: closing panel 0, 1, 2, then opening panel 0, 1, 2
pub const PANEL_SCRIPTS: [u32; 6] = [0x3a8ca0, 0x3a8cb0, 0x3a8cc0, 0x3a8cd0, 0x3a8ce0, 0x3a8cf0];
pub const TITLE_OPEN_SCRIPT: u32 = 0x3a8d00;
pub const TITLE_STOP_SCRIPT: u32 = 0x3a8d10;

const ONE: u32 = 0x3f80_0000;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

fn sound(m: &Mem) -> u32 {
    m.r32(fe(m).wrapping_add(0xa4))
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

/// The sound player's virtual at `off` (the one-call functions are [`SOUND_CALLS`] and [`SOUND_FLAGS`])
pub fn snd(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let s = sound(m);
    vcall_at(m, e, s, 0, off, args)
}

/// A voice line played (the sound player's +0x10; -1 stops the line). The many one-line callers are
/// [`SAY_LINES`].
pub fn say_line(m: &mut Mem, e: &mut dyn FeEnv, line: u32) {
    snd(m, e, 0x10, &[line]);
}

/// The functions that only play a voice line: (the address, the line)
pub const SAY_LINES: [(u32, u32); 28] = [
    (0x261f20, 0x1e),
    (0x261f60, 0x1f),
    (0x261fa0, 0x20),
    (0x261fe0, 0x22),
    (0x262020, 0x23),
    (0x262060, 0x08),
    (0x2620a0, 0x09),
    (0x2620e0, 0x0a),
    (0x262120, 0x0b),
    (0x262160, 0x0c),
    (0x262478, 0x00),
    (0x262508, 0x02),
    (0x262548, 0x3e),
    (0x262588, 0x41),
    (0x2625c8, 0x42),
    (0x262670, 0x27),
    (0x2626b0, 0x29),
    (0x2626f0, 0x25),
    (0x262730, 0x2a),
    (0x262770, u32::MAX),
    (0x2627b0, 0x26),
    (0x2628e0, 0x0e),
    (0x262920, 0x0f),
    (0x262f80, 0x01),
    (0x262fc0, 0x02),
    (0x2631b0, 0x2c),
    (0x2631f0, 0x2d),
    (0x263230, 0x2e),
];

/// The functions that only call one of the sound player's virtuals with nothing: (the address, the virtual)
pub const SOUND_CALLS: [(u32, u32); 6] =
    [(0x262228, 0xa8), (0x2622c8, 0xa8), (0x262300, 0xa8), (0x262ca0, 0xa8), (0x262d48, 0x98), (0x262f48, 0xb0)];

/// The functions that only call one of the sound player's virtuals with a flag: (the address, the virtual, the flag)
pub const SOUND_FLAGS: [(u32, u32, u32); 2] = [(0x262c20, 0x70, 0), (0x262c60, 0x78, 0)];

/// `FE3DCharSel_SayIndexedLine` 0x261ee0(-, line): the line (a byte) through the sound player's +0x18
pub fn say_indexed_line(m: &mut Mem, e: &mut dyn FeEnv, line: u32) {
    snd(m, e, 0x18, &[line as u8 as i8 as i32 as u32]);
}

/// The character select's audio 0x2621a0: line 0x10, then the sound player's +0x60 and +0xa8
pub fn charsel_start_audio(m: &mut Mem, e: &mut dyn FeEnv) {
    say_line(m, e, 0x10);
    snd(m, e, 0x60, &[]);
    snd(m, e, 0xa8, &[]);
}

/// The character modify's audio 0x262260: line 0x1d, then +0xa8
pub fn charmodify_start_audio(m: &mut Mem, e: &mut dyn FeEnv) {
    say_line(m, e, 0x1d);
    snd(m, e, 0xa8, &[]);
}

/// The options' audio 0x262608: line 0x24, then +0xb8
pub fn options_start_audio(m: &mut Mem, e: &mut dyn FeEnv) {
    say_line(m, e, 0x24);
    snd(m, e, 0xb8, &[]);
}

/// The difficulty, event and venue selects' audio (0x262338, 0x262388, 0x2624b8): their line (0x2b, 7, 0x2f), then
/// the music's third event (`FE3DSound_SetMusicEvent3` 0x262f48: the sound player's +0xb0)
pub fn line_then_music(m: &mut Mem, e: &mut dyn FeEnv, line: u32) {
    say_line(m, e, line);
    snd(m, e, 0xb0, &[]);
}

/// `FE3DModeSelect_SetCamera` 0x2623d8: the camera (`cFE3D` +0x50) to mode 2
pub fn modeselect_set_camera(m: &mut Mem, e: &mut dyn FeEnv) {
    let cam = m.r32(fe(m) + 0x50);
    e.call(m, CAMERA_SET_MODE, &[cam, 2]);
}

/// 0x2623f0: the title stopped ([`title_stop`]), then line 0xd
pub fn title_stop_then_line(m: &mut Mem, e: &mut dyn FeEnv) {
    title_stop(m, e);
    say_line(m, e, 0xd);
}

/// `FE3DTitle_SetCameraAndMusic` 0x262438: the camera to mode 1, then the menu music resumed (0x262d48: +0x98)
pub fn title_set_camera_and_music(m: &mut Mem, e: &mut dyn FeEnv) {
    let cam = m.r32(fe(m) + 0x50);
    e.call(m, CAMERA_SET_MODE, &[cam, 1]);
    snd(m, e, 0x98, &[]);
}

/// The mode select's lines 0x2627f0, 0x262840, 0x262890 (6, 4, 5): the line, then the world's panel 0, 1 or 2
/// opened ([`set_open_panel`])
pub fn line_then_panel(m: &mut Mem, e: &mut dyn FeEnv, p: u32, line: u32, panel: u32) {
    say_line(m, e, line);
    set_open_panel(m, e, p, panel);
}

/// `FEWorld_SetOpenPanel` 0x262960(p, panel): when the open panel (the byte at `p`) is another, its closing script
/// runs (for panels 0..2), then the new one's opening script (0..2), and the byte is set
pub fn set_open_panel(m: &mut Mem, e: &mut dyn FeEnv, p: u32, panel: u32) {
    let new = panel as u8 as i8 as i32;
    let cur = m.r8(p) as i8 as i32;
    if cur == new {
        return;
    }
    if (0..3).contains(&cur) {
        e.call(m, RUN_SCRIPT, &[PANEL_SCRIPTS[cur as usize]]);
    }
    if (0..3).contains(&new) {
        e.call(m, RUN_SCRIPT, &[PANEL_SCRIPTS[3 + new as usize]]);
    }
    m.w8(p, new as u8);
}

/// `FEWorld_OnTitleOpen` 0x262a68: the camera to mode 2, the title's opening script, the sound player's +0x70 with 1
pub fn title_open(m: &mut Mem, e: &mut dyn FeEnv) {
    let cam = m.r32(fe(m) + 0x50);
    e.call(m, CAMERA_SET_MODE, &[cam, 2]);
    e.call(m, RUN_SCRIPT, &[TITLE_OPEN_SCRIPT]);
    snd(m, e, 0x70, &[1]);
}

/// 0x262ad0: both players' choices reset (0x263270 on `cFE3D` +0x7c and +0x7e), the fifth rider selected for player
/// 0, the camera to mode 6, the sound player's +0x78 with 1
pub fn charsel_open(m: &mut Mem, e: &mut dyn FeEnv) {
    for k in 0..2u32 {
        let p = fe(m) + 0x7c + 2 * k;
        reset_choice(m, p);
    }
    let f = fe(m);
    set_selected(m, e, f, 0, 1, 4);
    let cam = m.r32(fe(m) + 0x50);
    e.call(m, CAMERA_SET_MODE, &[cam, 6]);
    snd(m, e, 0x78, &[1]);
}

/// 0x262b78: the camera to mode 0xb, the sound player's +0x80 with 1
pub fn camera_b(m: &mut Mem, e: &mut dyn FeEnv) {
    let cam = m.r32(fe(m) + 0x50);
    e.call(m, CAMERA_SET_MODE, &[cam, 0xb]);
    snd(m, e, 0x80, &[1]);
}

/// 0x262bd0(p): the sound player's +0x68 with 0, then the world's panel 3 ([`set_open_panel`]: none opens)
pub fn panel_3(m: &mut Mem, e: &mut dyn FeEnv, p: u32) {
    snd(m, e, 0x68, &[0]);
    set_open_panel(m, e, p, 3);
}

/// `FEWorld_OnTitleStop` 0x262cd8: the title's closing script, the sound player's +0xa0, line 3
pub fn title_stop(m: &mut Mem, e: &mut dyn FeEnv) {
    e.call(m, RUN_SCRIPT, &[TITLE_STOP_SCRIPT]);
    snd(m, e, 0xa0, &[]);
    say_line(m, e, 3);
}

/// 0x263270(p): the choice's byte at +1 reset to 4
pub fn reset_choice(m: &mut Mem, p: u32) {
    m.w8(p + 1, 4);
}

/// `FE3DCharacter_SetSelected` 0x2670e0(c, v): +0x154
pub fn character_set_selected(m: &mut Mem, c: u32, v: u32) {
    m.w32(c + 0x154, v);
}

/// 0x269be8(g, k, f12): the thing's instance (+0xa0) and its controller (+0xe4), when both are there, handed `k`
/// and the float through the controller's +0xc0 (its table at +8)
pub fn thing_set_param(m: &mut Mem, e: &mut dyn FeEnv, g: u32, k: u32, f: u32) {
    let inst = m.r32(g + 0xa0);
    if inst == 0 {
        return;
    }
    let c = m.r32(inst + 0xe4);
    if c != 0 {
        vcall_at(m, e, c, 8, 0xc0, &[k, f]);
    }
}

/// A world thing by name (its hash), or 0
fn find(m: &mut Mem, e: &mut dyn FeEnv, name: u32) -> u32 {
    let world = fe(m) + 0x80;
    let h = e.call(m, NAME_HASH, &[name]);
    e.call(m, FIND_BY_HASH, &[world, h])
}

/// `FE3DCharSel_SetSelected` 0x261b38(f, player, -, sel): the twelve riders (+0x20) hidden, then `sel` and its two
/// neighbours each side (mod 12) shown; the player's old choice's picture (by name at 0x3470c8; the byte at +0x7d
/// + 2 · player) faded out (0) and the new one's in (1); every rider deselected and its marker (0x347100) out; the
/// chosen rider selected, the byte set and its marker in
pub fn set_selected(m: &mut Mem, e: &mut dyn FeEnv, f: u32, player: u32, _a2: u32, sel: u32) {
    let pick = (player as u8 as i8 as i32 as u32).wrapping_mul(2).wrapping_add(f).wrapping_add(0x7d);
    let riders = f + 0x20;
    for i in 0..12u32 {
        let r = m.r32(riders + 4 * i);
        vcall(m, e, r, 0x28, &[0]);
    }
    let r = m.r32(riders.wrapping_add(sel.wrapping_mul(4)));
    vcall(m, e, r, 0x28, &[1]);
    for k in [1i32, 2, 11, 10] {
        let i = (sel as i32).wrapping_add(k).wrapping_rem(12);
        let r = m.r32(riders.wrapping_add((i as u32).wrapping_mul(4)));
        vcall(m, e, r, 0x28, &[1]);
    }
    let old = m.r8(pick) as i8 as i32 as u32;
    let name = m.r32(RIDER_NAMES.wrapping_add(old.wrapping_mul(4)));
    let t = find(m, e, name);
    if t != 0 {
        thing_set_param(m, e, t, 2, 0);
    }
    let name = m.r32(RIDER_NAMES.wrapping_add(sel.wrapping_mul(4)));
    let t = find(m, e, name);
    if t != 0 {
        thing_set_param(m, e, t, 2, ONE);
    }
    for i in 0..12u32 {
        let r = m.r32(riders + 4 * i);
        character_set_selected(m, r, 0);
        let name = m.r32(MARKER_NAMES + 4 * i);
        let t = find(m, e, name);
        if t != 0 {
            thing_set_param(m, e, t, 2, 0);
        }
    }
    let r = m.r32(riders.wrapping_add(sel.wrapping_mul(4)));
    character_set_selected(m, r, 1);
    m.w8(pick, sel as u8);
    let name = m.r32(MARKER_NAMES.wrapping_add(sel.wrapping_mul(4)));
    let t = find(m, e, name);
    if t != 0 {
        thing_set_param(m, e, t, 2, ONE);
    }
}

/// `VenueSel_ClearVenue` 0x261e98(v): the shown venue (the byte at +1), when there is one, has its closing script
/// run and is cleared (-1)
pub fn clear_venue(m: &mut Mem, e: &mut dyn FeEnv, v: u32) {
    let k = m.r8(v + 1) as i8 as i32;
    if k >= 0 {
        let s = m.r32(VENUE_SCRIPTS.wrapping_add((k as u32).wrapping_mul(8)) + 4);
        e.call(m, RUN_SCRIPT, &[s]);
        m.w8(v + 1, 0xff);
    }
}

/// `VenueSel_SetVenue` 0x263150(v, k): the byte at +1 set, the venue's opening script run, the sound player's +0x20
/// with 0
pub fn set_venue(m: &mut Mem, e: &mut dyn FeEnv, v: u32, k: u32) {
    let k = k as u8 as i8 as i32 as u32;
    m.w8(v + 1, k as u8);
    let s = m.r32(VENUE_SCRIPTS.wrapping_add(k.wrapping_mul(8)));
    e.call(m, RUN_SCRIPT, &[s]);
    snd(m, e, 0x20, &[0]);
}

/// The thing's float parameter `k`
fn set(m: &mut Mem, e: &mut dyn FeEnv, g: u32, k: u32, v: f32) {
    thing_set_param(m, e, g, k, v.to_bits());
}

/// `FEVenueSel_ShowMedals` 0x262d80(-, show): each of the ten venues' medal marks (by name at 0x347158) that is in
/// the world: when the mode (`cFE3D` +0xf0) is 2 and `show`, the career's medal for the player (0x32de8c) at the
/// venue (its music byte) picks the mark: 0 gold (2), 1 silver (1), 2 bronze (0), each made visible (parameter 4
/// at 1); any other medal, or not shown, hides it (4 at 0)
pub fn show_medals(m: &mut Mem, e: &mut dyn FeEnv, show: u32) {
    for i in 0..10u32 {
        let name = m.r32(MEDAL_NAMES + 4 * i);
        let g = find(m, e, name);
        if g == 0 {
            continue;
        }
        let mode = m.r32(fe(m) + 0xf0);
        if mode != 2 || show == 0 {
            set(m, e, g, 4, 0.0);
            continue;
        }
        let who = m.r32(CAREER + 0x27dc + 0x320);
        let at = m.r8(VENUE_MUSIC + i) as i8 as i32 as u32;
        let r = e.call(m, MEDAL, &[CAREER, who, at]);
        match r {
            0 => {
                set(m, e, g, 2, 2.0);
                set(m, e, g, 4, 1.0);
            }
            1 => {
                set(m, e, g, 2, 1.0);
                set(m, e, g, 4, 1.0);
            }
            2 => {
                set(m, e, g, 2, 0.0);
                set(m, e, g, 4, 1.0);
            }
            _ => set(m, e, g, 4, 0.0),
        }
    }
}

/// `FEWorld_OpenCourseVideo` 0x269c70(g, name): the film (+0xb0) opened when there is none (128 × 128, 1, 0xf on the
/// front end's film player, `cFE3D` +0x124)
pub fn open_course_video(m: &mut Mem, e: &mut dyn FeEnv, g: u32, name: u32) {
    if (m.r32(g + 0xb0) as i32) < 0 {
        let player = m.r32(fe(m) + 0x124);
        let h = e.call(m, VIDEO_OPEN, &[player, name, 0x80, 0x80, 1, 0xf]);
        m.w32(g + 0xb0, h);
    }
}

/// `FE3DMovie_Stop` 0x269cc8(g): the film (+0xb0), when there is one, released and cleared (-1)
pub fn movie_stop(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    let h = m.r32(g + 0xb0);
    if (h as i32) >= 0 {
        let player = m.r32(fe(m) + 0x124);
        e.call(m, VIDEO_RELEASE, &[player, h]);
        m.w32(g + 0xb0, u32::MAX);
    }
}

/// `FEWorld_ShowCourseVideo` 0x263000(-, course, open): the ten course films (`cFE3D` +0xfc) stopped and hidden;
/// for a course 0..9 its film opened (when `open`, by name at 0x3471d0) and shown, and the venue's music (its byte
/// at 0x3a8af0) through the sound player's +0x28
pub fn show_course_video(m: &mut Mem, e: &mut dyn FeEnv, course: u32, open: u32) {
    for i in 0..10u32 {
        let g = m.r32(fe(m) + 0xfc + 4 * i);
        movie_stop(m, e, g);
        let g = m.r32(fe(m) + 0xfc + 4 * i);
        vcall(m, e, g, 0x28, &[0]);
    }
    let c = course as u8 as i8 as i32 as u32;
    if c & 0xff >= 10 {
        return;
    }
    if open != 0 {
        let name = m.r32(FILM_NAMES.wrapping_add(c.wrapping_mul(4)));
        let g = m.r32(fe(m).wrapping_add(0xfc).wrapping_add(c.wrapping_mul(4)));
        open_course_video(m, e, g, name);
        let g = m.r32(fe(m).wrapping_add(0xfc).wrapping_add(c.wrapping_mul(4)));
        vcall(m, e, g, 0x28, &[1]);
    }
    let music = m.r8(VENUE_MUSIC.wrapping_add(c)) as i8 as i32 as u32;
    snd(m, e, 0x28, &[music]);
}

// ---- the characters and their lighting (0x265da0..0x267670)

/// The characters' calls out: an object freed (0x114f80), the scene's base destructor (0x2ace78), the thing's
/// (0x2acd10), the allocation and its fill (0x2ccf70, 0x2cc808), the rider model's constructor (0x116828), its
/// parts set (0x116a88), its rider set (0x116a68) and freed (0x116a18), the loader's request made (0x2010c8) and
/// run (0x202478), a load's options (0x201ad8 / 0x201a38), started (0x201e68), done (0x201d30), finished (0x2021f8)
/// and released (0x201c20), the career's choices kept (0x164e78 the outfit, 0x164df8 the board) and the rider's
/// parts (0x16b1d0, 0x16b220, 0x16b1f8, 0x16b270, 0x16b248), the model's animation: playing (0x15ff40), played
/// (0x15fcf8), its speed (0x15f390), stepped (0x15f700), stopped (0x15ee30), the screen's mode (0x265758)
pub const FREE: u32 = 0x114f80;
pub const SCENE_BASE_DTOR: u32 = 0x2ace78;
pub const THING_DTOR: u32 = 0x2acd10;
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
pub const MODEL_CTOR: u32 = 0x116828;
pub const MODEL_SET_PARTS: u32 = 0x116a88;
pub const MODEL_SET_RIDER: u32 = 0x116a68;
pub const MODEL_FREE_RIDER: u32 = 0x116a18;
pub const LOAD_REQUEST: u32 = 0x2010c8;
pub const LOAD_RUN: u32 = 0x202478;
pub const LOAD_OPTIONS: u32 = 0x201ad8;
pub const LOAD_OPTIONS_ASYNC: u32 = 0x201a38;
pub const LOAD_START: u32 = 0x201e68;
pub const LOAD_DONE: u32 = 0x201d30;
pub const LOAD_FINISH: u32 = 0x2021f8;
pub const LOAD_RELEASE: u32 = 0x201c20;
pub const KEEP_OUTFIT: u32 = 0x164e78;
pub const KEEP_BOARD: u32 = 0x164df8;
pub const RIDER_PARTS: [u32; 5] = [0x16b1d0, 0x16b220, 0x16b1f8, 0x16b270, 0x16b248];
pub const ANIM_PLAYING: u32 = 0x15ff40;
pub const ANIM_PLAY: u32 = 0x15fcf8;
pub const ANIM_SPEED: u32 = 0x15f390;
pub const ANIM_STEP: u32 = 0x15f700;
pub const ANIM_STOP: u32 = 0x15ee30;
pub const SCREEN_MODE: u32 = 0x265758;
/// The scene's and the character's tables, the request's own (handed to the loader), the riders' boards (0x14 a
/// board, 0xf0 a rider) and the riders (0x38 each)
pub const SCENE_VTABLE: u32 = 0x3a9100;
pub const CHARACTER_VTABLE: u32 = 0x3a99f8;
pub const REQUEST_VTABLE: u32 = 0x39e280;
pub const RIDER_BOARDS: u32 = 0x332148;
pub const RIDERS: u32 = 0x331ea8;
const THING_VTABLE: u32 = 0x3b5a00;

fn fl(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

/// 0x265da0(o, flags): `o` freed when bit 0 is set
pub fn scene_delete(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, FREE, &[o]);
    }
}

/// `cFE3DScene::~cFE3DScene` 0x265dc0(o, flags): its table at +0x20 put back, then the base's destructor
pub fn scene_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o + 0x20, SCENE_VTABLE);
    e.call(m, SCENE_BASE_DTOR, &[o, flags]);
}

/// The thing's part of the character's set-up (as `cFE3DThing`'s): the links, flags and vectors
fn thing_init(m: &mut Mem, o: u32) {
    m.w32(o, u32::MAX);
    m.w32(o + 4, o);
    m.w32(o + 8, o);
    m.w32(o + 0xc, THING_VTABLE);
    m.w32(o + 0x10, 0);
    m.w32(o + 0x14, 1);
    for at in (0x18..0x28).step_by(4) {
        m.w32(o + at, 0);
    }
    m.w32(o + 0x28, 1);
    for at in [0x2cu32, 0x30] {
        m.w32(o + at, 0);
    }
    for at in (0x40..0x60).step_by(4) {
        m.w32(o + at, 0);
    }
    for at in (0x60..0x70).step_by(4) {
        m.w32(o + at, ONE);
    }
    for at in [0x70u32, 0x74, 0x80, 0x84, 0x88] {
        m.w32(o + at, 0);
    }
    m.w32(o + 0x8c, ONE);
    for at in [0x90u32, 0x94, 0x98] {
        m.w32(o + at, 0);
    }
}

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32) -> u32 {
    let a = e.call(m, ALLOC, &[name, size, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0xdead_c0de, size]);
    }
    a
}

/// The loader's request for the model's textures (`buf`: 0 and the request's table) made and run with 2
fn load_model(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    m.w32(buf, 0);
    m.w32(buf + 4, REQUEST_VTABLE);
    let model = m.r32(c + 0xac);
    e.call(m, LOAD_REQUEST, &[buf, model, 0]);
    e.call(m, LOAD_RUN, &[buf, 2]);
}

/// `FE3DCharacter_Init` 0x265ed8(c, rider; `buf` 0xd0 bytes): the thing's set-up, then the character's: its table,
/// the shown and wanted poses (+0xa0, +0xa1) 3, +0xa4 1, the rider (+0xb0), no load (+0xb4, +0xb8 -1), no
/// changes (+0x15c, +0x160); the rider model (0x5b00 bytes, its parts set with 1; +0xac) and its rider record
/// (0x84 bytes; +0xa8); the model's textures loaded; not selected, lit fully (+0x150 1) and the lights worked out
/// ([`update_lighting`])
pub fn character_init(m: &mut Mem, e: &mut dyn FeEnv, c: u32, rider: u32, buf: u32) -> u32 {
    thing_init(m, c);
    m.w32(c + 0xc, CHARACTER_VTABLE);
    m.w8(c + 0xa1, 3);
    m.w32(c + 0xa4, 1);
    m.w32(c + 0xb0, rider);
    m.w32(c + 0xb8, u32::MAX);
    m.w8(c + 0xa0, 3);
    m.w32(c + 0xb4, u32::MAX);
    m.w32(c + 0x15c, 0);
    m.w32(c + 0x160, 0);
    let a = alloc(m, e, 0x3a99d8, 0x5b00);
    let model = e.call(m, MODEL_CTOR, &[a]);
    m.w32(c + 0xac, model);
    e.call(m, MODEL_SET_PARTS, &[model, 1]);
    let info = alloc(m, e, 0x3a99e8, 0x84);
    m.w32(c + 0xa8, info);
    m.w32(buf, 0);
    m.w32(buf + 4, REQUEST_VTABLE);
    let model = m.r32(c + 0xac);
    e.call(m, LOAD_REQUEST, &[buf, model, 0]);
    m.w32(c + 0x154, 0);
    m.w32(c + 0x150, ONE);
    update_lighting(m, c, buf + 0x10);
    e.call(m, LOAD_RUN, &[buf, 2]);
    c
}

/// `cFE3DCharacter::~cFE3DCharacter` 0x266110(c, flags): its table put back, the model destroyed (its +8 with 3,
/// its table at +0) and the rider record freed, then the thing's destructor
pub fn character_dtor(m: &mut Mem, e: &mut dyn FeEnv, c: u32, flags: u32) {
    m.w32(c + 0xc, CHARACTER_VTABLE);
    let model = m.r32(c + 0xac);
    if model != 0 {
        vcall_at(m, e, model, 0, 8, &[3]);
        m.w32(c + 0xac, 0);
    }
    let info = m.r32(c + 0xa8);
    if info != 0 {
        e.call(m, FREE, &[info]);
        m.w32(c + 0xa8, 0);
    }
    e.call(m, THING_DTOR, &[c, flags]);
}

/// `FE3DRider_SaveSelection` 0x266190(c, player): the character's outfit (the byte at +0x158) and board (+0x159)
/// kept in the career for the player
pub fn save_selection(m: &mut Mem, e: &mut dyn FeEnv, c: u32, player: u32) {
    let outfit = m.r8(c + 0x158) as i8 as i32 as u32;
    e.call(m, KEEP_OUTFIT, &[CAREER, player, outfit]);
    let board = m.r8(c + 0x159) as i8 as i32 as u32;
    e.call(m, KEEP_BOARD, &[CAREER, player, board]);
}

/// 0x2661e8(c, pose): the wanted pose (+0xa1)
pub fn set_pose(m: &mut Mem, c: u32, pose: u32) {
    m.w8(c + 0xa1, pose as u8);
}

/// 0x2661f0(c): the wanted pose 3 and the model's animation stopped
pub fn stop_pose(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    let model = m.r32(c + 0xac);
    m.w8(c + 0xa1, 3);
    e.call(m, ANIM_STOP, &[model + 0x46c0]);
}

/// `FE3DCharacter_FillRiderInfo` 0x266208(c): the rider record (+0xa8) filled for the rider (+0xb0) in its outfit
/// (+0x158) and board (+0x159): +0x40 0, +0x44 the board's word (0x332148), the five parts from the career (+0x48,
/// +0x50, +0x4c, +0x58, +0x54), and from the rider's row (0x331ea8) its first word (+0x38, +0x3c), +0x24 (+0x60)
/// and the byte +0x34 (+0x64)
pub fn fill_rider_info(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    let outfit = m.r8(c + 0x158) as i8 as i32 as u32;
    let rider = m.r32(c + 0xb0);
    let info = m.r32(c + 0xa8);
    m.w32(info + 0x40, 0);
    let at = RIDER_BOARDS.wrapping_add(outfit.wrapping_mul(0x14)).wrapping_add(rider.wrapping_mul(0xf0));
    let v = m.r32(at.wrapping_add(8));
    m.w32(info + 0x44, v);
    let board = |m: &Mem| m.r8(c + 0x159) as i8 as i32 as u32;
    let part = |m: &mut Mem, e: &mut dyn FeEnv, f: u32, arg: u32, at: u32| {
        let rider = m.r32(c + 0xb0);
        let v = e.call(m, f, &[CAREER, rider, arg]);
        let info = m.r32(c + 0xa8);
        m.w32(info + at, v);
    };
    let o = m.r8(c + 0x158) as i8 as i32 as u32;
    part(m, e, RIDER_PARTS[0], o, 0x48);
    part(m, e, RIDER_PARTS[1], 0, 0x50);
    let b = board(m);
    part(m, e, RIDER_PARTS[2], b, 0x4c);
    part(m, e, RIDER_PARTS[3], 0, 0x58);
    part(m, e, RIDER_PARTS[4], 0, 0x54);
    let rider = m.r32(c + 0xb0);
    let info = m.r32(c + 0xa8);
    let row = RIDERS.wrapping_add(rider.wrapping_mul(0x38));
    m.w32(info + 0x38, m.r32(row));
    m.w32(info + 0x60, m.r32(row.wrapping_add(0x24)));
    m.w32(info + 0x3c, m.r32(row));
    m.w8(info + 0x64, m.r8(row.wrapping_add(0x34)));
}

/// `FE3DCharacter_LoadRiderAsync` 0x266320(c; `buf` 0x10 bytes): the rider record filled, the model given the
/// rider, and its load started on the front end's loader (`cFE3D` +0x18) with the load options' first word; the
/// shown pose 3, no changes
pub fn load_rider_async(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    fill_rider_info(m, e, c);
    e.call(m, LOAD_OPTIONS_ASYNC, &[buf]);
    let (model, rider, info) = (m.r32(c + 0xac), m.r32(c + 0xb0), m.r32(c + 0xa8));
    e.call(m, MODEL_SET_RIDER, &[model, rider, info]);
    let loader = m.r32(fe(m) + 0x18);
    let model = m.r32(c + 0xac);
    let opts = m.r32(buf);
    let h = e.call(m, LOAD_START, &[loader, model, 1, opts]);
    m.w32(c + 0xb4, h);
    m.w8(c + 0xa0, 3);
    m.w32(c + 0x15c, 0);
    m.w32(c + 0x160, 0);
}

/// `FE3DCharacter_FreeRider` 0x266390(c): the model's rider freed (with 1) and the load, when there is one,
/// released
pub fn free_rider(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    let model = m.r32(c + 0xac);
    e.call(m, MODEL_FREE_RIDER, &[model, 1]);
    let h = m.r32(c + 0xb4);
    if (h as i32) >= 0 {
        let loader = m.r32(fe(m) + 0x18);
        e.call(m, LOAD_RELEASE, &[loader, h]);
        m.w32(c + 0xb4, u32::MAX);
    }
}

/// 0x2663e8(c): the model's +0x46b0 (whether it has its rider)
pub fn has_model(m: &Mem, c: u32) -> u32 {
    m.r32(m.r32(c + 0xac).wrapping_add(0x46b0))
}

/// `cFE3DCharacter` vf12 0x266df8(c; `buf` 0xc0 bytes), the update:
/// - an outfit or board change (+0x15c, +0x160): a load in flight is released; otherwise the rider is reloaded with
///   the load options' bits 10 and 11 set from the two changes, and the changes cleared;
/// - a load in flight that is done is finished and released;
/// - when the model has its rider: a pose that finished playing (pose 1) goes to 2; a new wanted pose (+0xa1) is
///   played (0: 0x2f9; 1: by the screen's mode 0 0x2fa, 1 0x2fb, 2 0x2fc, else 0x2fd; 2: 0x2fe; others none) and
///   becomes the shown one (+0xa0); the animation at speed 1 and stepped;
/// - the lights worked out
pub fn character_update(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    if m.r32(c + 0x160) != 0 || m.r32(c + 0x15c) != 0 {
        let h = m.r32(c + 0xb4);
        if (h as i32) >= 0 {
            let loader = m.r32(fe(m) + 0x18);
            e.call(m, LOAD_RELEASE, &[loader, h]);
            m.w32(c + 0xb4, u32::MAX);
        } else {
            e.call(m, LOAD_OPTIONS, &[buf]);
            let mut v = m.r32(buf);
            v = (v & !0x400) | ((m.r32(c + 0x15c) & 1) << 10);
            v = (v & !0x800) | ((m.r32(c + 0x160) & 1) << 11);
            m.w32(buf, v);
            fill_rider_info(m, e, c);
            let (model, rider, info) = (m.r32(c + 0xac), m.r32(c + 0xb0), m.r32(c + 0xa8));
            e.call(m, MODEL_SET_RIDER, &[model, rider, info]);
            let loader = m.r32(fe(m) + 0x18);
            let model = m.r32(c + 0xac);
            let opts = m.r32(buf);
            let h = e.call(m, LOAD_START, &[loader, model, 1, opts]);
            m.w32(c + 0xb4, h);
            m.w32(c + 0x15c, 0);
            m.w32(c + 0x160, 0);
        }
    }
    let h = m.r32(c + 0xb4);
    if (h as i32) >= 0 {
        let loader = m.r32(fe(m) + 0x18);
        if e.call(m, LOAD_DONE, &[loader, h]) != 0 {
            let h = m.r32(c + 0xb4);
            e.call(m, LOAD_FINISH, &[loader, h]);
            let h = m.r32(c + 0xb4);
            e.call(m, LOAD_RELEASE, &[loader, h]);
            m.w32(c + 0xb4, u32::MAX);
        }
    }
    let model = m.r32(c + 0xac);
    if m.r32(model + 0x46b0) != 0 {
        let anim = model + 0x46c0;
        if e.call(m, ANIM_PLAYING, &[anim, 0]) != 0 && m.r8(c + 0xa0) as i8 == 1 {
            m.w8(c + 0xa1, 2);
        }
        let want = m.r8(c + 0xa1) as i8;
        if want != m.r8(c + 0xa0) as i8 {
            let clip = match want {
                0 => Some(0x2f9),
                1 => {
                    let f = fe(m);
                    Some(match e.call(m, SCREEN_MODE, &[f]) {
                        0 => 0x2fa,
                        1 => 0x2fb,
                        2 => 0x2fc,
                        _ => 0x2fd,
                    })
                }
                2 => Some(0x2fe),
                _ => None,
            };
            if let Some(clip) = clip {
                let model = m.r32(c + 0xac);
                e.call(m, ANIM_PLAY, &[model + 0x46c0, clip, 0, 5]);
            }
            m.w8(c + 0xa0, m.r8(c + 0xa1));
        }
        let model = m.r32(c + 0xac);
        e.call(m, ANIM_SPEED, &[model + 0x46c0, ONE]);
        let model = m.r32(c + 0xac);
        e.call(m, ANIM_STEP, &[model + 0x46c0]);
    }
    update_lighting(m, c, buf);
}

/// `FE3DCharacter_LoadRider` 0x2670a0(c; `buf` 0x10 bytes): the model's textures loaded
pub fn load_rider(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    load_model(m, e, c, buf);
}

/// The lights: (1, 0.25, 0.25, 0.25) ambient, two lights of 1.25 at 60° and 120° (and 0), none third
const LIGHTS_A: ([u32; 4], [[u32; 4]; 3], [u32; 3], [u32; 3]) = (
    [ONE, 0x3e80_0000, 0x3e80_0000, 0x3e80_0000],
    [[0, 0x3fa0_0000, 0x3fa0_0000, 0x3fa0_0000], [0, 0x3fa0_0000, 0x3fa0_0000, 0x3fa0_0000], [0; 4]],
    [0x4270_0000, 0x42f0_0000, 0],
    [0, 0, 0],
);
/// (1, 0.1, 0.1, 0.1) ambient, lights of 10 at (75°, 58°), (2.1, 1.9, 1.7) at (345°, 20°), (1, 1, 2) at (285°, -20°)
const LIGHTS_B: ([u32; 4], [[u32; 4]; 3], [u32; 3], [u32; 3]) = (
    [ONE, 0x3dcc_cccd, 0x3dcc_cccd, 0x3dcc_cccd],
    [[0, 0x4120_0000, 0x4120_0000, 0x4120_0000], [0, 0x4006_6666, 0x3ff3_3333, 0x3fd9_999a], [0, ONE, ONE, 0x4000_0000]],
    [0x4296_0000, 0x43ac_8000, 0x438e_8000],
    [0x4268_0000, 0x41a0_0000, 0xc1a0_0000],
);

fn fill_lights(m: &mut Mem, l: u32, s: &([u32; 4], [[u32; 4]; 3], [u32; 3], [u32; 3])) {
    for k in 0..4u32 {
        m.w32(l + 4 * k, s.0[k as usize]);
    }
    for i in 0..3u32 {
        for k in 0..4u32 {
            m.w32(l + 0x28 + 0x10 * i + 4 * k, s.1[i as usize][k as usize]);
        }
        m.w32(l + 0x10 + 4 * i, s.2[i as usize]);
        m.w32(l + 0x1c + 4 * i, s.3[i as usize]);
    }
}

/// `FE3DCharacter_FillLightsA` 0x2673f8(c, l): the unselected lights ([`LIGHTS_A`]) into `l` (the ambient, then
/// each light's colour at +0x28 + 0x10 k and angles at +0x10 + 4 k and +0x1c + 4 k)
pub fn fill_lights_a(m: &mut Mem, l: u32) {
    fill_lights(m, l, &LIGHTS_A);
}

/// `FE3DCharacter_FillLightsB` 0x267508(c, l): the selected lights ([`LIGHTS_B`])
pub fn fill_lights_b(m: &mut Mem, l: u32) {
    fill_lights(m, l, &LIGHTS_B);
}

/// `FE3DLighting_Lerp` 0x267670(c, out, a, b; t): `out` = `a` · t + `b` · (1 − t) over the ambient, the angles and
/// the colours
pub fn lighting_lerp(m: &mut Mem, out: u32, a: u32, b: u32, t: u32) {
    let t = F(f32::from_bits(t));
    let u = F(1.0) - t;
    let mix = |m: &mut Mem, at: u32| {
        let v = fl(m, a + at) * t + fl(m, b + at) * u;
        m.w32(out + at, v.0.to_bits());
    };
    for at in [0u32, 4, 8, 0xc] {
        mix(m, at);
    }
    for k in 0..3u32 {
        for at in [0x10 + 4 * k, 0x1c + 4 * k, 0x28 + 0x10 * k, 0x2c + 0x10 * k, 0x30 + 0x10 * k, 0x34 + 0x10 * k] {
            mix(m, at);
        }
    }
}

/// v / |v| on VU0 (the sum of four squares, `vrsqrt`, `vmulq`); 1/√0 is the largest float
fn vu_unit(v: [f32; 4]) -> [f32; 4] {
    let m = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let s = ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3];
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    v.map(|x| x * q)
}

/// `FE3DCharacter_UpdateLighting` 0x2670e8(c; `buf` 0xc0 bytes for the two sets mixed): the light (+0x150) moved a fifteenth toward 1 when selected (+0x154)
/// or toward 0 when not, kept in 0..1; when it moved, the lights (+0xc0) are the unselected ones at 0, the selected
/// ones at 1 or the mix between, and each light's direction (+0x120 + 0x10 k) is worked out from its angles in
/// degrees (a at +0xd0 + 4 k, b at +0xdc + 4 k): (cos a cos b, sin b, sin a cos b), made unit length with w 0
pub fn update_lighting(m: &mut Mem, c: u32, buf: u32) {
    let step = F(f32::from_bits(0x3d88_8889));
    let t = fl(m, c + 0x150);
    let moved = if m.r32(c + 0x154) != 0 {
        if t.ee() < 1.0 {
            let v = t + step;
            let v = if v.ee() < 0.0 { F(0.0) } else if 1.0 < v.ee() { F(1.0) } else { v };
            m.w32(c + 0x150, v.0.to_bits());
            true
        } else {
            false
        }
    } else if 0.0 < t.ee() {
        let v = t - step;
        let v = if v.ee() < 0.0 { F(0.0) } else if 1.0 < v.ee() { F(1.0) } else { v };
        m.w32(c + 0x150, v.0.to_bits());
        true
    } else {
        false
    };
    if !moved {
        return;
    }
    let t = fl(m, c + 0x150);
    let l = c + 0xc0;
    if t.ee() == 0.0 {
        fill_lights_a(m, l);
    } else if t.ee() == 1.0 {
        fill_lights_b(m, l);
    } else {
        let (sel, unsel) = (buf, buf + 0x60);
        fill_lights_b(m, sel);
        fill_lights_a(m, unsel);
        lighting_lerp(m, l, sel, unsel, t.0.to_bits());
    }
    let deg = F(f32::from_bits(0x3c8e_fa35));
    for k in 0..3u32 {
        let a = (fl(m, c + 0xd0 + 4 * k) * deg).0;
        let b = (fl(m, c + 0xdc + 4 * k) * deg).0;
        let x = (F(crate::math::cos(a)) * F(crate::math::cos(b))).0;
        let y = crate::math::sin(b);
        let z = (F(crate::math::sin(a)) * F(crate::math::cos(b))).0;
        let d = vu_unit([x, y, z, 0.0]);
        let at = c + 0x120 + 0x10 * k;
        for (i, v) in d.iter().enumerate() {
            m.w32(at + 4 * i as u32, v.to_bits());
        }
        m.w32(at + 0xc, 0);
    }
}

// ---- the dialog box, the group's own virtuals and two memory helpers (0x267f08..0x2692e8)

/// The dialog's calls out: the sprite by index (0x2ac4b0), a sprite drawn (0x1fc178: the sprite, its anchor, then
/// x, y, w, h), the font's colour (0x14eee8 on the font state at 0x337fe8), a vector set (0x14f5f0, four floats),
/// a list's operation applied (0x2ac220) and the operations' destructors (the extent's 0x269388, the update's
/// 0x269440, the text render's 0x2694c8, the render's 0x269558), the list's emptying (0x2ac178) and destructor
/// (0x2ace78), the thing's unlink (0x2acf28), the widget's destructor (0x2adaa8), a pool's free (0x23d420) and the
/// heap's (0x2ccfc0)
pub const SPRITE_RECT: u32 = 0x2ac4b0;
pub const SPRITE_DRAW: u32 = 0x1fc178;
pub const FONT_SET_COLOUR: u32 = 0x14eee8;
pub const FONT_STATE: u32 = 0x337fe8;
pub const VEC4_SET: u32 = 0x14f5f0;
pub const LIST_APPLY: u32 = 0x2ac220;
pub const EXTENT_OP_DTOR: u32 = 0x269388;
pub const UPDATE_OP_DTOR: u32 = 0x269440;
pub const RENDER_TEXT_OP_DTOR: u32 = 0x2694c8;
pub const RENDER_OP_DTOR: u32 = 0x269558;
pub const LIST_DELETE_ALL: u32 = 0x2ac178;
pub const LIST_DTOR: u32 = 0x2ace78;
pub const THING_UNLINK: u32 = 0x2acf28;
pub const WIDGET_DTOR: u32 = 0x2adaa8;
pub const POOL_FREE: u32 = 0x23d420;
pub const HEAP_FREE: u32 = 0x2ccfc0;
/// The operations' tables, the dialog's and the group's tables, the pool (its base and size)
pub const EXTENT_OP: u32 = 0x3aa660;
pub const UPDATE_OP: u32 = 0x3a9fe0;
pub const RENDER_TEXT_OP: u32 = 0x3a9ff8;
pub const RENDER_OP: u32 = 0x3aa010;
pub const DIALOG_VTABLE: u32 = 0x3a9d50;
pub const GROUP_VTABLE: u32 = 0x3a9e98;
pub const POOL: u32 = 0x345e48;
/// (0.88, 0.1, 0.1, 0.1): the dialog's colour
const DIALOG_COLOUR: [u32; 4] = [0x3f61_47ae, 0x3dcc_cccd, 0x3dcc_cccd, 0x3dcc_cccd];
const HALF: F = F(0.5);

fn put(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, *x);
    }
}

/// The renderer (the app's +0x724, its table at +0x10)
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x724))
}

fn render_state(m: &mut Mem, e: &mut dyn FeEnv) -> u32 {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0x1d8, &[])
}

fn set_render_state(m: &mut Mem, e: &mut dyn FeEnv, v: u32) {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0x1d0, &[v]);
}

/// 0x267f08(p): 0x108 bytes of `p` zeroed (the fill)
pub fn zero_108(m: &mut Mem, e: &mut dyn FeEnv, p: u32) {
    e.call(m, FILL, &[p, 0, 0x108]);
}

/// 0x267f18(p): `p` freed to the pool at 0x345e48 when it lies inside it (past its base, within its size), else to
/// the heap
pub fn free_block(m: &mut Mem, e: &mut dyn FeEnv, p: u32) {
    let d = p.wrapping_sub(m.r32(POOL)) as i32;
    let inside = d > 0 && d < m.r32(POOL + 4) as i32;
    if inside {
        e.call(m, POOL_FREE, &[POOL, p]);
    } else {
        e.call(m, HEAP_FREE, &[p]);
    }
}

/// `cFE3DDialog`'s constructor 0x267f58(d, owner, parent; `buf` 0x10 bytes) (the index's `cFE3DThing::cFE3DThing`):
/// a group, then the dialog's table, its five sprites (+0x140..+0x150) none, sized 100 x 30 by its class, its four
/// colours (0.88, 0.1, 0.1, 0.1), the sides 0x11 and 0x12 (+0x150, +0x14c), the edge (+0x154) the side sprite's
/// height, white for the corners (+0x158) and its own colour (+0x60)
pub fn dialog_ctor(m: &mut Mem, e: &mut dyn FeEnv, d: u32, owner: u32, parent: u32, buf: u32) -> u32 {
    crate::fe_widgets::group_init(m, e, d, owner, parent);
    m.w32(d + 0xc, DIALOG_VTABLE);
    for k in 0..5u32 {
        m.w32(d + 0x150 - 4 * k, u32::MAX);
    }
    put(m, buf, [F(100.0).0.to_bits(), F(30.0).0.to_bits(), 0, 0]);
    crate::fe_widgets::vcall_class(m, e, DIALOG_VTABLE, d, 0x138, &[buf]);
    for at in [0xacu32, 0xbc, 0xcc, 0xdc] {
        put(m, d + at, DIALOG_COLOUR);
    }
    m.w32(d + 0x150, 0x11);
    m.w32(d + 0x14c, 0x12);
    let r = e.call(m, SPRITE_RECT, &[d, 0x11]);
    m.w32(d + 0x154, m.r32(r.wrapping_add(8)));
    put(m, d + 0x158, [ONE; 4]);
    put(m, d + 0x60, DIALOG_COLOUR);
    d
}

/// Whether the dialog has its top corners (+0x144, +0x148; 0x268db0) or its bottom ones (+0x150, +0x14c; 0x268d90)
pub fn dialog_has_top(m: &Mem, d: u32) -> bool {
    (m.r32(d + 0x144) as i32) >= 0 && (m.r32(d + 0x148) as i32) >= 0
}

pub fn dialog_has_bottom(m: &Mem, d: u32) -> bool {
    (m.r32(d + 0x150) as i32) >= 0 && (m.r32(d + 0x14c) as i32) >= 0
}

fn draw(m: &mut Mem, e: &mut dyn FeEnv, d: u32, k: u32, anchor: u32, r: [F; 4]) {
    let spr = e.call(m, SPRITE_RECT, &[d, k]);
    let [x, y, w, h] = r.map(|v| v.0.to_bits());
    e.call(m, SPRITE_DRAW, &[spr, anchor, x, y, w, h]);
}

/// 0x268308(d): the dialog's frame drawn in the font's colour (its own, +0x60): the body (sprite +0x140) over its
/// box (+0x80, +0x84; +0x100 x +0x104) less the edge (+0x154, less a sixth of it) at a top and a bottom with
/// corners; unless the edge is 0, the edges' middles (0.36 of the half width in from the sides) above and below;
/// then in the corners' colour (+0x158) the corners (half wide, the edge high) with their anchors (0, 8, 0xc, 4).
/// The renderer's state is raised by one between the two passes and put back after (not when the edge is 0).
pub fn dialog_draw_frame(m: &mut Mem, e: &mut dyn FeEnv, d: u32) {
    let st = render_state(m, e);
    e.call(m, FONT_SET_COLOUR, &[FONT_STATE, d + 0x60]);
    let w = fl(m, d + 0x100);
    let edge = fl(m, d + 0x154);
    let half_w = w * HALF;
    let half_e = edge * HALF;
    let sixth = edge * F(f32::from_bits(0x3e2a_ab8a));
    let inset = half_w * F(f32::from_bits(0x3eb8_00a8));
    let x = fl(m, d + 0x80);
    let mut y = fl(m, d + 0x84);
    let mut h = fl(m, d + 0x104);
    if dialog_has_top(m, d) {
        let cut = edge - sixth;
        h = h - cut;
        y = y + cut;
    }
    if dialog_has_bottom(m, d) {
        let cut = edge - sixth;
        h = h - cut;
    }
    let body = m.r32(d + 0x140);
    draw(m, e, d, body, 0, [x, y, w, h]);
    let edge = fl(m, d + 0x154);
    if edge.ee() == 0.0 {
        return;
    }
    let y = (fl(m, d + 0x84) + edge) - half_e;
    let x = fl(m, d + 0x80) + inset;
    let w = fl(m, d + 0x100) - (inset + inset);
    let h = half_e - sixth;
    if dialog_has_top(m, d) {
        let body = m.r32(d + 0x140);
        draw(m, e, d, body, 0, [x, y, w, h]);
    }
    if dialog_has_bottom(m, d) {
        let y = ((fl(m, d + 0x84) + fl(m, d + 0x104)) - fl(m, d + 0x154)) + sixth;
        let body = m.r32(d + 0x140);
        draw(m, e, d, body, 0, [x, y, w, h]);
    }
    set_render_state(m, e, st.wrapping_add(1));
    e.call(m, FONT_SET_COLOUR, &[FONT_STATE, d + 0x158]);
    let w = fl(m, d + 0x100) * HALF;
    let h = fl(m, d + 0x154);
    if dialog_has_top(m, d) {
        let k = m.r32(d + 0x144);
        draw(m, e, d, k, 0, [fl(m, d + 0x80), fl(m, d + 0x84), w, h]);
        let k = m.r32(d + 0x148);
        draw(m, e, d, k, 8, [fl(m, d + 0x80) + fl(m, d + 0x100), fl(m, d + 0x84), w, h]);
    }
    if dialog_has_bottom(m, d) {
        let k = m.r32(d + 0x14c);
        let (x, y) = (fl(m, d + 0x80) + fl(m, d + 0x100), fl(m, d + 0x84) + fl(m, d + 0x104));
        draw(m, e, d, k, 0xc, [x, y, w, h]);
        let k = m.r32(d + 0x150);
        let (x, y) = (fl(m, d + 0x80), fl(m, d + 0x84) + fl(m, d + 0x104));
        draw(m, e, d, k, 4, [x, y, w, h]);
    }
    set_render_state(m, e, st);
}

/// The dialog's look shared by its two layouts: its colour through its +0x50, white corners, sized 640 x `height`
fn dialog_look(m: &mut Mem, e: &mut dyn FeEnv, d: u32, height: F, buf: u32) {
    put(m, buf, DIALOG_COLOUR);
    vcall(m, e, d, 0x50, &[buf]);
    put(m, d + 0x158, [ONE; 4]);
    put(m, buf, [F(640.0).0.to_bits(), height.0.to_bits(), 0, 0]);
    vcall(m, e, d, 0x138, &[buf]);
}

/// 0x2686c0(d; height; `buf`): the dialog with all four corners (0x17, 0x18, 0x16, 0x15), centred down the screen
/// (y = (480 - height) / 2), its edge the bottom-left corner's height
pub fn dialog_layout_boxed(m: &mut Mem, e: &mut dyn FeEnv, d: u32, height: u32, buf: u32) {
    let height = F(f32::from_bits(height));
    m.w32(d + 0x144, 0x17);
    m.w32(d + 0x148, 0x18);
    m.w32(d + 0x150, 0x15);
    m.w32(d + 0x14c, 0x16);
    dialog_look(m, e, d, height, buf);
    let y = (F(480.0) - height) * HALF;
    put(m, buf, [0, y.0.to_bits(), 0, ONE]);
    e.call(m, crate::fe3dstate::WIDGET_PLACE, &[d, buf]);
    let k = m.r32(d + 0x150);
    let r = e.call(m, SPRITE_RECT, &[d, k]);
    m.w32(d + 0x154, m.r32(r.wrapping_add(8)));
}

/// 0x268808(d; height; `buf`): the dialog as a bar at the top: no top corners, the bottom ones 0x11 and 0x12, its
/// edge that sprite's height (set before and after), at (0, 0)
pub fn dialog_layout_bar(m: &mut Mem, e: &mut dyn FeEnv, d: u32, height: u32, buf: u32) {
    let height = F(f32::from_bits(height));
    m.w32(d + 0x148, u32::MAX);
    m.w32(d + 0x150, 0x11);
    m.w32(d + 0x14c, 0x12);
    m.w32(d + 0x144, u32::MAX);
    let r = e.call(m, SPRITE_RECT, &[d, 0x11]);
    m.w32(d + 0x154, m.r32(r.wrapping_add(8)));
    dialog_look(m, e, d, height, buf);
    put(m, buf, [0, 0, 0, ONE]);
    e.call(m, crate::fe3dstate::WIDGET_PLACE, &[d, buf]);
    let r = e.call(m, SPRITE_RECT, &[d, 0x11]);
    m.w32(d + 0x154, m.r32(r.wrapping_add(8)));
}

/// `cFE3DDialog` vf10 0x268940(d, matrix; `buf`): when shown (+0x14), the children drawn with the matrix at the
/// renderer's state raised by one and then by the dialog's +0x70; the state put back
pub fn dialog_render(m: &mut Mem, e: &mut dyn FeEnv, d: u32, matrix: u32, buf: u32) {
    if m.r32(d + 0x14) == 0 {
        return;
    }
    let st = render_state(m, e);
    set_render_state(m, e, st.wrapping_add(1));
    if m.r32(d + 0x14) != 0 {
        let s = render_state(m, e);
        let lift = m.r32(d + 0x70);
        set_render_state(m, e, s.wrapping_add(lift));
        m.w32(buf + 4, matrix);
        m.w32(buf, RENDER_OP);
        e.call(m, LIST_APPLY, &[d + 0x110, buf]);
        set_render_state(m, e, s);
        e.call(m, RENDER_OP_DTOR, &[buf, 2]);
    }
    set_render_state(m, e, st);
}

/// `cFE3DDialog` vf11 0x268a80(d; `buf`): when shown, the frame drawn at the state raised by the dialog's +0x70,
/// then (still shown) the children's texts at the state raised by one more and the +0x70; the state put back
pub fn dialog_render_text(m: &mut Mem, e: &mut dyn FeEnv, d: u32, buf: u32) {
    if m.r32(d + 0x14) == 0 {
        return;
    }
    let st = render_state(m, e);
    let lift = m.r32(d + 0x70);
    set_render_state(m, e, st.wrapping_add(lift));
    dialog_draw_frame(m, e, d);
    set_render_state(m, e, st.wrapping_add(1));
    if m.r32(d + 0x14) != 0 {
        let s = render_state(m, e);
        let lift = m.r32(d + 0x70);
        set_render_state(m, e, s.wrapping_add(lift));
        m.w32(buf, RENDER_TEXT_OP);
        e.call(m, LIST_APPLY, &[d + 0x110, buf]);
        set_render_state(m, e, s);
        e.call(m, RENDER_TEXT_OP_DTOR, &[buf, 2]);
    }
    set_render_state(m, e, st);
}

/// `cFE3DDialog` vf12 0x268bd0(d; `buf`): the children updated, then when active (+0x10) its own +0x48
pub fn dialog_update(m: &mut Mem, e: &mut dyn FeEnv, d: u32, buf: u32) {
    m.w32(buf, UPDATE_OP);
    e.call(m, LIST_APPLY, &[d + 0x110, buf]);
    if m.r32(d + 0x10) != 0 {
        vcall(m, e, d, 0x48, &[]);
    }
    e.call(m, UPDATE_OP_DTOR, &[buf, 2]);
}

/// The scene's colours set up once (0x268c38): (1, 0.076, 0.002) at 0x3d2eb8, (1, 0.965, 0) at 0x3d2e98, (1, 0.474,
/// 0) at 0x3d2ea8, and at 0x3d2ec8 -1 then (1, 1, 0, 0, 1, 1)
pub fn scene_colours_init(m: &mut Mem) {
    for (at, v) in [
        (0x3d2eb8u32, [ONE, 0x3d9b_3d08, 0x3b03_126f]),
        (0x3d2e98, [ONE, 0x3f76_e979, 0]),
        (0x3d2ea8, [ONE, 0x3ef2_fec5, 0]),
    ] {
        for (k, x) in v.iter().enumerate() {
            m.w32(at + 4 * k as u32, *x);
        }
    }
    m.w32(0x3d2ec8, u32::MAX);
    for (k, x) in [ONE, ONE, 0, 0, ONE, ONE].iter().enumerate() {
        m.w32(0x3d2ecc + 4 * k as u32, *x);
    }
}

/// 0x268ce0(out, v): `out` set from the four floats at `v`
pub fn vec4_set_from(m: &mut Mem, e: &mut dyn FeEnv, out: u32, v: u32) {
    let f = [0u32, 4, 8, 0xc].map(|k| m.r32(v + k));
    e.call(m, VEC4_SET, &[out, f[0], f[1], f[2], f[3]]);
}

/// The dialog's small accessors: the edge (0x268d00 set, 0x268d08 get), the corners' colour (0x268d10 set,
/// 0x268d38 got into `out`), a sprite by slot (0x268d60 set, 0x268d70 get; the body is slot 0, 0x268d80 / 0x268d88)
pub fn dialog_set_edge(m: &mut Mem, d: u32, v: u32) {
    m.w32(d + 0x154, v);
}

pub fn dialog_edge(m: &Mem, d: u32) -> u32 {
    m.r32(d + 0x154)
}

pub fn dialog_set_corner_colour(m: &mut Mem, d: u32, v: u32) {
    for k in 0..4u32 {
        let x = m.r32(v + 4 * k);
        m.w32(d + 0x158 + 4 * k, x);
    }
}

pub fn dialog_corner_colour(m: &mut Mem, out: u32, d: u32) -> u32 {
    for k in 0..4u32 {
        let x = m.r32(d + 0x158 + 4 * k);
        m.w32(out + 4 * k, x);
    }
    out
}

pub fn dialog_set_sprite(m: &mut Mem, d: u32, k: u32, v: u32) {
    m.w32(d.wrapping_add(k.wrapping_mul(4)).wrapping_add(0x140), v);
}

pub fn dialog_sprite(m: &Mem, d: u32, k: u32) -> u32 {
    m.r32(d.wrapping_add(k.wrapping_mul(4)).wrapping_add(0x140))
}

/// `cFE3DDialog::~cFE3DDialog` 0x268dd0(d, flags): its table put back, then the group's destructor
pub fn dialog_dtor(m: &mut Mem, e: &mut dyn FeEnv, d: u32, flags: u32) {
    m.w32(d + 0xc, DIALOG_VTABLE);
    group_dtor(m, e, d, flags);
}

/// `cFE3DWGroup::~cFE3DWGroup` 0x2692e8(g, flags): its table put back, the children freed (0x2691c8: the list
/// emptied, then itself unlinked), the list destroyed with 2, then the widget's destructor
pub fn group_dtor(m: &mut Mem, e: &mut dyn FeEnv, g: u32, flags: u32) {
    m.w32(g + 0xc, GROUP_VTABLE);
    e.call(m, LIST_DELETE_ALL, &[g + 0x110]);
    e.call(m, THING_UNLINK, &[g]);
    e.call(m, LIST_DTOR, &[g + 0x110, 2]);
    e.call(m, WIDGET_DTOR, &[g, flags]);
}

/// `cFE3DWGroup` vf36 0x2691f8(g, min, max; `buf` 0x10 bytes): the extent of the children: `min` from 9999 and
/// `max` from -9999 on x, y and z, the extent operation applied
pub fn group_extent(m: &mut Mem, e: &mut dyn FeEnv, g: u32, min: u32, max: u32, buf: u32) {
    let big = F(9999.0).0.to_bits();
    for k in [0u32, 8, 4] {
        m.w32(min + k, big);
    }
    for k in [0u32, 8, 4] {
        m.w32(max + k, (-F(9999.0)).0.to_bits());
    }
    for (k, v) in [EXTENT_OP, min, max].iter().enumerate() {
        m.w32(buf + 4 * k as u32, *v);
    }
    e.call(m, LIST_APPLY, &[g + 0x110, buf]);
    e.call(m, EXTENT_OP_DTOR, &[buf, 2]);
}

// ---- the geometry, its films and the animations (0x26957c..0x26a4b8)

/// The geometry's calls out: the world's instance by name hash (0x2550c8 on the world at 0x346488), a matrix made
/// (0x1ce278), an instance drawn (0x200680), the geometry's destructor (0x26cc40), a film's texture (0x2001c8),
/// the draw's lighting set up (0x1ce140)
pub const FIND_INSTANCE: u32 = 0x2550c8;
pub const WORLD: u32 = 0x346488;
pub const MATRIX_MAKE: u32 = 0x1ce278;
pub const DRAW_INSTANCE: u32 = 0x200680;
pub const GEOMETRY_DTOR: u32 = 0x26cc40;
pub const VIDEO_TEXTURE: u32 = 0x2001c8;
pub const LIGHTING_SETUP: u32 = 0x1ce140;
/// The film geometry's table
pub const MOVIE_GEOMETRY_VTABLE: u32 = 0x3abae0;

/// 0x26957c: nothing
pub fn nothing() {}

/// 0x269608(out; x, y, z, w): `out` set (the copy of `Vec4_Set`)
pub fn vec4_set(m: &mut Mem, out: u32, v: [u32; 4]) {
    m.w32(out + 0xc, v[3]);
    m.w32(out, v[0]);
    m.w32(out + 4, v[1]);
    m.w32(out + 8, v[2]);
}

/// `FE3DGeometry_BindInstance` 0x269a70(g, name): the world's instance of that name (+0xa0); when there is one,
/// its flags (+0xe8) lose bit 1 and gain bit 2
pub fn bind_instance(m: &mut Mem, e: &mut dyn FeEnv, g: u32, name: u32) {
    let h = e.call(m, NAME_HASH, &[name]);
    let inst = e.call(m, FIND_INSTANCE, &[WORLD, h]);
    m.w32(g + 0xa0, inst);
    if inst != 0 {
        let f = m.r32(inst + 0xe8);
        m.w32(inst + 0xe8, (f & 0xffff_fffd) | 4);
    }
}

/// `FE3DGeometry_GetPos` 0x269ad0(g, out): the instance's position (+0x30) with w 1, when there is an instance
pub fn geometry_pos(m: &mut Mem, g: u32, out: u32) {
    let inst = m.r32(g + 0xa0);
    if inst == 0 {
        return;
    }
    m.w32(out, m.r32(inst + 0x30));
    m.w32(out + 4, m.r32(inst + 0x34));
    m.w32(out + 0xc, ONE);
    m.w32(out + 8, m.r32(inst + 0x38));
}

/// `cFE3DGeometry` vf10 0x269b08(g, -; `buf` 0xc0 bytes): when it has an instance and is shown, the instance drawn
/// (a matrix made, then the instance's own copied beside it) at the renderer's state raised by its +0x70
pub fn geometry_render(m: &mut Mem, e: &mut dyn FeEnv, g: u32, buf: u32) {
    if m.r32(g + 0xa0) == 0 || m.r32(g + 0x14) == 0 {
        return;
    }
    let st = render_state(m, e);
    let lift = m.r32(g + 0x70);
    set_render_state(m, e, st.wrapping_add(lift));
    e.call(m, MATRIX_MAKE, &[buf]);
    let inst = m.r32(g + 0xa0);
    for k in 0..16u32 {
        let v = m.r32(inst + 4 * k);
        m.w32(buf + 0x80 + 4 * k, v);
    }
    e.call(m, DRAW_INSTANCE, &[WORLD, inst, buf + 0x80, buf]);
    set_render_state(m, e, st);
}

/// `cFE3DMovieGeometry::~cFE3DMovieGeometry` 0x269c28(g, flags): its table put back, its film stopped, then the
/// geometry's destructor
pub fn movie_geometry_dtor(m: &mut Mem, e: &mut dyn FeEnv, g: u32, flags: u32) {
    m.w32(g + 0xc, MOVIE_GEOMETRY_VTABLE);
    movie_stop(m, e, g);
    e.call(m, GEOMETRY_DTOR, &[g, flags]);
}

/// `cFE3DMovieGeometry` vf10 0x269d10(g, a; `buf` 0xc0 bytes): when it has an instance and is shown, at the state
/// raised by its +0x70: with a film playing (+0xb0), the film's texture drawn on the instance (the renderer's +0x1e0
/// with 3, the lighting (1, 0.5, 0.5, 0.5) / (0, 0, 0, 1) / 0 set up, then its +0x2a8 with the instance's +0xc0, the
/// texture, the instance and the lighting); without, the plain geometry's draw
pub fn movie_geometry_render(m: &mut Mem, e: &mut dyn FeEnv, g: u32, a: u32, buf: u32) {
    if m.r32(g + 0xa0) == 0 || m.r32(g + 0x14) == 0 {
        return;
    }
    let st = render_state(m, e);
    let lift = m.r32(g + 0x70);
    set_render_state(m, e, st.wrapping_add(lift));
    let h = m.r32(g + 0xb0);
    if (h as i32) >= 0 {
        let player = m.r32(fe(m) + 0x124);
        let tex = e.call(m, VIDEO_TEXTURE, &[player, h]);
        let r = renderer(m);
        vcall_at(m, e, r, 0x10, 0x1e0, &[3]);
        let half = HALF.0.to_bits();
        put(m, buf + 0x80, [ONE, half, half, half]);
        put(m, buf + 0x90, [0, 0, 0, ONE]);
        put(m, buf + 0xa0, [0, 0, 0, 0]);
        let (c, p, z) = (buf + 0x80, buf + 0x90, buf + 0xa0);
        e.call(m, LIGHTING_SETUP, &[buf, c, p, z, p, z, p, z]);
        let inst = m.r32(g + 0xa0);
        let model = m.r32(inst + 0xc0);
        let r = renderer(m);
        vcall_at(m, e, r, 0x10, 0x2a8, &[model, tex, inst, buf, 0, 0]);
    } else {
        let _ = a;
        geometry_render(m, e, g, buf);
    }
    set_render_state(m, e, st);
}

/// 0x269ea0(a): the animation's rate (+0xa8) one over its seconds (+0xa4) at 60 a second, squared as the game has it
/// (1 / (s · 60 · 60))
pub fn anim_set_rate(m: &mut Mem, a: u32) {
    let s = fl(m, a + 0xa4) * F(60.0) * F(60.0);
    m.w32(a + 0xa8, (F(1.0) / s).0.to_bits());
}

/// `cFE3DAnim` vf12 0x269ed0(a): the time (+0xa0) moved on by the rate (+0xa8)
pub fn anim_step(m: &mut Mem, a: u32) {
    let t = fl(m, a + 0xa0) + fl(m, a + 0xa8);
    m.w32(a + 0xa0, t.0.to_bits());
}

fn rows(m: &Mem, at: u32) -> [[F; 4]; 4] {
    [0u32, 1, 2, 3].map(|r| [0u32, 1, 2, 3].map(|k| fl(m, at + 0x10 * r + 4 * k)))
}

/// Four rows times a row vector on VU0 (vmulax / vmadday / vmaddaz / vmaddw): the sum of rows[k] x v[k]
fn mat_vec(rows: &[[F; 4]; 4], v: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|i| ((rows[0][i] * v[0] + rows[1][i] * v[1]) + rows[2][i] * v[2]) + rows[3][i] * v[3])
}

/// The rotation by `angle` about the unit axis `a` (x, y or z), as the game builds it from the sine and cosine:
/// rows (t·x·x + c, t·y·x − s·z, t·z·x + s·y), (t·x·y + s·z, t·y·y + c, t·z·y − s·x), (t·x·z − s·y, t·y·z + s·x,
/// t·z·z + c) with t = 1 − c, and (0, 0, 0, 1)
fn axis_rotation(angle: F, a: [F; 3]) -> [[F; 4]; 4] {
    let (s, c) = crate::math::sin_cos(angle.0);
    let (s, c) = (F(s), F(c));
    let [ax, ay, az] = a;
    let t = F(1.0) - c;
    let (tz, tx, ty) = (t * az, t * ax, t * ay);
    let (sz, sx, sy) = (s * az, s * ax, s * ay);
    let z0 = (tz * ax) + sy;
    let y0 = (ty * ax) - sz;
    let z1 = (tz * ay) - sx;
    let z2 = (tz * az) + c;
    let x0 = (tx * ax) + c;
    let x1 = (tx * ay) + sz;
    let x2 = (tx * az) - sy;
    let y1 = (ty * ay) + c;
    let y2 = (ty * az) + sx;
    let o = F(0.0);
    [[x0, y0, z0, o], [x1, y1, z1, o], [x2, y2, z2, o], [o, o, o, F(1.0)]]
}

/// `cFE3DAnimRotate` vf10 0x269ee8(a, mat): unless it drives another thing (+0xac), `mat` turned by the angles at
/// +0xb0, +0xb4 and +0xb8 about x, y and z in that order (each rotation times the matrix on VU0)
pub fn anim_rotate_apply(m: &mut Mem, a: u32, mat: u32) {
    if m.r32(a + 0xac) != 0 {
        return;
    }
    let mut cur = rows(m, mat);
    let one = F(1.0);
    let o = F(0.0);
    for (k, axis) in [[one, o, o], [o, one, o], [o, o, one]].iter().enumerate() {
        let r = axis_rotation(fl(m, a + 0xb0 + 4 * k as u32), *axis);
        cur = r.map(|row| mat_vec(&cur, row));
    }
    for (r, row) in cur.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            m.w32(mat + 0x10 * r as u32 + 4 * k as u32, v.0.to_bits());
        }
    }
}

/// `cFE3DAnimRotate` vf12 0x26a370(a): the time stepped, then the angles from +0xb0 toward +0xbc by the time
/// (start + (end − start) · t) put on the driven thing's +0x90 (+0xac) or its own
pub fn anim_rotate_update(m: &mut Mem, a: u32) {
    anim_step(m, a);
    let t = fl(m, a + 0xa0);
    let v = [0u32, 4, 8].map(|k| (fl(m, a + 0xbc + k) - fl(m, a + 0xb0 + k)) * t + fl(m, a + 0xb0 + k));
    let target = m.r32(a + 0xac);
    let at = if target != 0 { target + 0x90 } else { a + 0x90 };
    for (k, x) in v.iter().enumerate() {
        m.w32(at + 4 * k as u32, x.0.to_bits());
    }
}

/// `cFE3DAnimTranslate` vf10 0x26a430(a, mat): unless it drives another thing, `mat`'s last row moved through the
/// matrix by the offset at +0xb0 (the matrix times the offset on VU0)
pub fn anim_translate_apply(m: &mut Mem, a: u32, mat: u32) {
    if m.r32(a + 0xac) != 0 {
        return;
    }
    let cur = rows(m, mat);
    let v = [0u32, 4, 8, 0xc].map(|k| fl(m, a + 0xb0 + k));
    let r = mat_vec(&cur, v);
    for (k, x) in r.iter().enumerate() {
        m.w32(mat + 0x30 + 4 * k as u32, x.0.to_bits());
    }
}

// ---- the cameras (0x26a4b8..0x26c08c)

/// The cameras' calls out: a shot played (`FE3DCamera_PlayShot` 0x26c370, ported in `fe_misc`), the world drawn
/// (0x260278 then 0x260458 on the world at 0x346488), a pool's allocation (0x23d3b0 on the pool at 0x345e48), a
/// camera animation's frame made (`CamAnimFrame_Create` 0x1482c0) and its shot set (`CamAnim_SetShot` 0x148350), a
/// vector scaled (0x102f50) and the arctangent (0x2510f8)
pub const PLAY_SHOT: u32 = 0x26c370;
pub const WORLD_DRAW_BEGIN: u32 = 0x260278;
pub const WORLD_DRAW: u32 = 0x260458;
pub const POOL_ALLOC: u32 = 0x23d3b0;
pub const CAM_FRAME_CREATE: u32 = 0x1482c0;
pub const CAM_SET_SHOT: u32 = 0x148350;
pub const VEC_SCALE: u32 = 0x102f50;
pub const ATAN: u32 = 0x2510f8;
/// The cameras' tables, the camera animation's name and its parameters' table, the shots (0x14 bytes each), the
/// identity the camera's view starts from
pub const CAMERA_VTABLE: u32 = 0x3ab738;
pub const SCRIPTED_VTABLE: u32 = 0x3ab648;
pub const COMBINED_VTABLE: u32 = 0x3ab558;
pub const CAM_ANIM_NAME: u32 = 0x3aa938;
pub const CAM_ANIM_PARAMS: u32 = 0x3aa918;
pub const SHOTS: u32 = 0x347208;
pub const IDENTITY: u32 = 0x346440;

/// `cFE3DCamera`'s constructor 0x26a4b8(c): the thing's set-up, its table, the field of view (pi/4, +0xa0) and the
/// near and far planes (20, 20000)
pub fn camera_ctor(m: &mut Mem, c: u32) -> u32 {
    thing_init(m, c);
    m.w32(c + 0xc, CAMERA_VTABLE);
    m.w32(c + 0xa0, 0x3f49_0fdb);
    m.w32(c + 0xa4, F(20.0).0.to_bits());
    m.w32(c + 0xa8, F(20000.0).0.to_bits());
    c
}

/// `FE3DCamera_DrawWorld` 0x26a5d0(c, pos): the world drawn from `pos` out to the far plane (+0xa8): its cells up to
/// (far - 1) / 10000 + 1 (chopped) away first, then the draw itself with the far plane
pub fn camera_draw_world(m: &mut Mem, e: &mut dyn FeEnv, c: u32, pos: u32) {
    let far = fl(m, c + 0xa8);
    let n = ((far - F(1.0)).ee() as i32) / 10000 + 1;
    e.call(m, WORLD_DRAW_BEGIN, &[WORLD, pos, n as u32]);
    let far = fl(m, c + 0xa8);
    e.call(m, WORLD_DRAW, &[WORLD, pos, far.0.to_bits()]);
}

/// A camera animation's frame (0xc8 bytes from the pool, filled) made with 3 and the parameters at `buf`
fn cam_frame(m: &mut Mem, e: &mut dyn FeEnv, buf: u32) -> u32 {
    let a = e.call(m, POOL_ALLOC, &[POOL, 0xc8, 0x4000_0000, CAM_ANIM_NAME, 0, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0xdead_c0de, 0xc8]);
    }
    e.call(m, CAM_FRAME_CREATE, &[a, 3, buf])
}

/// The camera animation's frame (+0xb0), when there is one, destroyed (its +8 with 3, its table at +0xb4 for the
/// part at +0xac)
fn drop_frame(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    let f = m.r32(c + 0xb0);
    if f != 0 {
        let vt = m.r32(f + 0xb4);
        let o = f + 0xac;
        let adj = m.r16(vt.wrapping_add(8)) as i16 as u32;
        let func = m.r32(vt.wrapping_add(0xc));
        e.call(m, func, &[o.wrapping_add(adj), 3]);
    }
}

/// `FE3DCameraScripted_Init` 0x26ab60(c; `buf` 0x20 bytes): a camera, then the scripted camera's: shot 0x9a (+0xb4),
/// +0xb8 0, its table, and its animation's frame made from (0, -1, 30, 0) and the parameters' table (0x3aa918)
pub fn scripted_init(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) -> u32 {
    camera_ctor(m, c);
    m.w32(c + 0xb4, 0x9a);
    m.w32(c + 0xb0, 0);
    m.w32(c + 0xb8, 0);
    m.w32(c + 0xc, SCRIPTED_VTABLE);
    put(m, buf, [0, F(-1.0).0.to_bits(), F(30.0).0.to_bits(), 0]);
    m.w32(buf + 0x10, CAM_ANIM_PARAMS);
    let f = cam_frame(m, e, buf);
    m.w32(c + 0xb0, f);
    c
}

/// 0x26ac30(c): the frame's +0x10 times 100
pub fn scripted_scale_frame(m: &mut Mem, c: u32) {
    let f = m.r32(c + 0xb0);
    let v = fl(m, f + 0x10) * F(100.0);
    m.w32(f + 0x10, v.0.to_bits());
}

/// `cFE3DCameraScripted` vf1 0x26ac50(c): its frame destroyed and cleared
pub fn scripted_free(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    if m.r32(c + 0xb0) != 0 {
        drop_frame(m, e, c);
        m.w32(c + 0xb0, 0);
    }
}

/// `cFE3DCameraScripted` vf12 0x26b2b0(c): its own +0x48, then +0xb8 the frame's +0x18
pub fn scripted_update(m: &mut Mem, e: &mut dyn FeEnv, c: u32) {
    vcall(m, e, c, 0x48, &[]);
    let f = m.r32(c + 0xb0);
    m.w32(c + 0xb8, m.r32(f + 0x18));
}


/// `FE3DCameraScripted_SetShot` 0x26b2f0(c, k; `buf` 0x20 bytes): unless the shot (+0xb4) is already `k`, the frame
/// remade from the shot's row (0x347208 + 0x14 k: its floats at +4, +8 and +0xc, its byte at +0x10 and its first
/// word) and given that shot
pub fn scripted_set_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32, buf: u32) {
    let row = SHOTS.wrapping_add(k.wrapping_mul(0x14));
    m.w32(buf + 0xc, m.r8(row + 0x10) as i8 as i32 as u32);
    m.w32(buf + 0x10, m.r32(row));
    m.w32(buf + 8, m.r32(row + 0xc));
    if m.r32(c + 0xb4) == k {
        return;
    }
    m.w32(buf, m.r32(row + 4));
    m.w32(buf + 4, m.r32(row + 8));
    drop_frame(m, e, c);
    let f = cam_frame(m, e, buf);
    m.w32(c + 0xb0, f);
    e.call(m, CAM_SET_SHOT, &[f, buf]);
    m.w32(c + 0xb4, k);
}

/// `cFE3DCameraCombined`'s constructor 0x26b3f8(c; `buf` 0x20 bytes): a camera with its table and a scripted camera
/// inside it (+0xc0, also +0xb0), no mode (+0x180) and no rider or flag (the bytes +0x184, +0x185)
pub fn combined_ctor(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) -> u32 {
    camera_ctor(m, c);
    m.w32(c + 0xc, COMBINED_VTABLE);
    scripted_init(m, e, c + 0xc0, buf);
    m.w32(c + 0xb0, c + 0xc0);
    m.w32(c + 0x180, 0);
    m.w8(c + 0x184, 0);
    m.w8(c + 0x185, 0);
    c
}

/// `cFE3DCameraCombined` vf12 0x26b450 and vf28 0x26b4b8(c): the inner camera's +0x68 or +0xe8 (handed itself)
pub fn combined_inner(m: &mut Mem, e: &mut dyn FeEnv, c: u32, off: u32) {
    let s = m.r32(c + 0xb0);
    vcall(m, e, s, off, &[s]);
}

/// `cFE3DCameraCombined` vf10 0x26b480(c, a): when shown, the inner camera's +0x58 (handed `a`)
pub fn combined_render(m: &mut Mem, e: &mut dyn FeEnv, c: u32, a: u32) {
    if m.r32(c + 0x14) != 0 {
        let s = m.r32(c + 0xb0);
        vcall(m, e, s, 0x58, &[a]);
    }
}

/// The camera's shot played (0x26c370)
fn shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32) {
    e.call(m, PLAY_SHOT, &[c, k]);
}

/// `FE3DCamera_PanToRider` 0x26b4e8(c, k): the rider after the last (+0x184, mod 12) plays shot 0xf + k, any other
/// 0x1b for rider 0 and 0x27 - k for the rest (riders 0..11); the rider kept, mode 7
pub fn pan_to_rider(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32) {
    let k = k as u8 as i8 as i32;
    let next = (m.r8(c + 0x184) as i8 as i32 + 1) % 12;
    if (k as u32) < 12 {
        if k == next {
            shot(m, e, c, 0xf + k as u32);
        } else {
            shot(m, e, c, if k == 0 { 0x1b } else { 0x27 - k as u32 });
        }
    }
    m.w8(c + 0x184, k as u8);
    m.w32(c + 0x180, 7);
}

/// The screens' rider shots: `CharSelect_PlayCameraShot` 0x26b760 (0x27 + k, mode 8), `FECharSel_PlayRiderShot`
/// 0x26b8a8 (0x33 + k, mode 9), `CharModify_PlayCameraShot` 0x26b9f0 (0x3f + k, mode 0xa): for riders 0..11 the shot,
/// then the rider kept and the mode set
pub fn rider_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32, base: u32, mode: u32) {
    let k = k as u8 as i8 as i32;
    if (k as u32) < 12 {
        shot(m, e, c, base + k as u32);
    }
    m.w8(c + 0x184, k as u8);
    m.w32(c + 0x180, mode);
}

/// `CustomBoard_PlayCameraShot` 0x26bb38 (0x4b or, with `alt`, 0x57) and `CustomBody_PlayCameraShot` 0x26bcb8 (0x63
/// or 0x6f)(c, alt): the kept rider's (+0x184, 0..11) shot
pub fn custom_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, alt: u32, base: u32, alt_base: u32) {
    let k = m.r8(c + 0x184) as i8 as i32;
    if (k as u32) < 12 {
        let b = if alt != 0 { alt_base } else { base };
        shot(m, e, c, b + k as u32);
    }
}

/// `FE3DCamera_SetMode` 0x26be38(c, mode) -> whether it moved from a mode (1 only): unless already in `mode` (+0x180):
/// - 1: shot 0; 2: shot 1 from mode 1, else by the front end's screen kind (`cFE3D` +0xf0: 0 shot 3 and mode 5, 1, 3
///   and 4 shot 4 and mode 4, 2 shot 2 and mode 3); 3: shot 5; 4: shot 6; 5: shot 7 from mode 3, shot 8 from mode 4;
/// - 6: by the screen kind (0 shot 0xd, 1, 3, 4 shot 0xe, 2 shot 0xc; 0xa, 0xb, 9 instead unless the app's +0x58 is
///   4); 11: shot 0x7b and the flag (+0x185) cleared;
/// - then the mode (or the one the screen kind chose) kept
pub fn set_mode(m: &mut Mem, e: &mut dyn FeEnv, c: u32, mode: u32) -> u32 {
    let cur = m.r32(c + 0x180);
    if cur == mode {
        return 0;
    }
    let f = fe(m);
    let mut keep = mode;
    let mut moved = 0;
    let app58 = |m: &Mem| m.r32(m.r32(GAPP) + 0x58) == 4;
    match mode.wrapping_sub(1) {
        0 => {
            moved = (cur != 0) as u32;
            shot(m, e, c, 0);
        }
        1 => {
            if cur == 1 {
                shot(m, e, c, 1);
            } else {
                match m.r32(f + 0xf0) {
                    0 => {
                        shot(m, e, c, 3);
                        keep = 5;
                    }
                    1 | 3 | 4 => {
                        shot(m, e, c, 4);
                        keep = 4;
                    }
                    2 => {
                        shot(m, e, c, 2);
                        keep = 3;
                    }
                    _ => {}
                }
            }
        }
        2 => shot(m, e, c, 5),
        3 => shot(m, e, c, 6),
        4 => {
            if cur == 3 {
                shot(m, e, c, 7);
            } else if cur == 4 {
                shot(m, e, c, 8);
            } else {
                m.w32(c + 0x180, keep);
                return moved;
            }
        }
        5 => match m.r32(f + 0xf0) {
            0 => {
                let k = if app58(m) { 0xd } else { 0xa };
                shot(m, e, c, k);
            }
            1 | 3 | 4 => {
                let k = if app58(m) { 0xe } else { 0xb };
                shot(m, e, c, k);
            }
            2 => {
                let k = if app58(m) { 0xc } else { 9 };
                shot(m, e, c, k);
            }
            _ => {}
        },
        10 => {
            shot(m, e, c, 0x7b);
            m.w8(c + 0x185, 0);
        }
        _ => {}
    }
    m.w32(c + 0x180, keep);
    moved
}

/// The view both cameras build: the identity (0x346440) with y and z swapped, turned about x by `angles[0]` - pi/2,
/// about z by `angles[2]` and about y by `angles[1]` (each rotation times the matrix on VU0), its last row the
/// matrix times `t`
fn view_matrix(m: &Mem, angles: [F; 3], t: [F; 4]) -> [[F; 4]; 4] {
    let mut cur = rows(m, IDENTITY);
    cur[1][1] = F(0.0);
    cur[1][2] = F(1.0);
    cur[2][1] = F(1.0);
    cur[2][2] = F(0.0);
    let (one, o) = (F(1.0), F(0.0));
    let half_pi = F(f32::from_bits(0x3fc9_0fdb));
    for (angle, axis) in [(angles[0] - half_pi, [one, o, o]), (angles[2], [o, o, one]), (angles[1], [o, one, o])] {
        let r = axis_rotation(angle, axis);
        cur = r.map(|row| mat_vec(&cur, row));
    }
    cur[3] = mat_vec(&cur, t);
    cur
}

/// The renderer's projection (+0xa8: field of view, near, far) and view (+0xf0: the matrix on `buf`) set
fn set_view(m: &mut Mem, e: &mut dyn FeEnv, c: u32, fov: F, view: &[[F; 4]; 4], buf: u32) {
    let (near, far) = (m.r32(c + 0xa4), m.r32(c + 0xa8));
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0xa8, &[fov.0.to_bits(), near, far]);
    for (i, row) in view.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            m.w32(buf + 0x10 * i as u32 + 4 * k as u32, v.0.to_bits());
        }
    }
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0xf0, &[buf]);
}

/// `cFE3DCamera` vf10 0x26a658(c; `buf` 0x40 bytes): when shown, the view from its angles (+0x90..+0x98) and
/// position (+0x80), its projection, and the world drawn from there
pub fn camera_render(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    if m.r32(c + 0x14) == 0 {
        return;
    }
    let angles = [0u32, 4, 8].map(|k| fl(m, c + 0x90 + k));
    let t = [0u32, 4, 8, 0xc].map(|k| fl(m, c + 0x80 + k));
    let view = view_matrix(m, angles, t);
    let fov = fl(m, c + 0xa0);
    set_view(m, e, c, fov, &view, buf);
    camera_draw_world(m, e, c, c + 0x80);
}

/// `cFE3DCameraScripted` vf10 0x26ac98(c; `buf` 0x50 bytes): when shown, from its frame's shot: the angles in
/// degrees (+0x74..+0x7c) and position (+0x68, w 1); the field of view 2 atan(8.997951 / lens) for a positive lens
/// (+0x94), else pi/4; the view from the angles and the position negated (`Vec4_Scale` by -1, w 1), kept as the
/// camera's (+0x90.., +0xa0, +0x80); its projection, and the world drawn from the position
pub fn scripted_render(m: &mut Mem, e: &mut dyn FeEnv, c: u32, buf: u32) {
    if m.r32(c + 0x14) == 0 {
        return;
    }
    let f = m.r32(c + 0xb0) + 0x1c;
    let deg = F(f32::from_bits(0x3c8e_fa35));
    let angles = [0u32, 4, 8].map(|k| fl(m, f + 0x58 + k) * deg);
    let pos = [fl(m, f + 0x4c), fl(m, f + 0x50), fl(m, f + 0x54), F(1.0)];
    let mut t = pos.map(|v| v * F(-1.0));
    t[3] = F(1.0);
    let lens = fl(m, f + 0x78);
    let fov = if 0.0 < lens.ee() {
        let a = F(crate::math::atan((F(f32::from_bits(0x410f_f79b)) / lens).0));
        a + a
    } else {
        F(f32::from_bits(0x3f49_0fdb))
    };
    let view = view_matrix(m, angles, t);
    for (k, v) in angles.iter().enumerate() {
        m.w32(c + 0x90 + 4 * k as u32, v.0.to_bits());
    }
    m.w32(c + 0xa0, fov.0.to_bits());
    for (k, v) in t.iter().enumerate() {
        m.w32(c + 0x80 + 4 * k as u32, v.0.to_bits());
    }
    set_view(m, e, c, fov, &view, buf);
    for (k, v) in pos.iter().enumerate() {
        m.w32(buf + 0x40 + 4 * k as u32, v.0.to_bits());
    }
    camera_draw_world(m, e, c, buf + 0x40);
}

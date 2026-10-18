//! The front end's 3D scene code below 0x26c000 (board rows G9z1..) on the game's bytes. First the screens' audio,
//! voice-line and world helpers (0x261b38..0x263270): they go through the front end's sound player (`cFE3D` +0xa4,
//! its table at +0), its world (+0x80, found by name hash), its riders (+0x20, twelve) and its course films (+0xfc,
//! ten). Every call out goes through [`FeEnv`]. Checked in the function runner (`tests/fe_scene.rs`).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::pf::Mem;

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

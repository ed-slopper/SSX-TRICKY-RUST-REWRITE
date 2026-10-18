//! The pause menu's draws, its constructor and its own virtuals (board rows G9a3, G9a5): `cPauseMenuOverlay`'s
//! constructor and virtuals, the main page's
//! draw, the options page's items and its draw. On the game's bytes ([`Mem`]); every callee (the fonts, the HUD's
//! sprites and boxes, the menu list, the trick book, the music system, the renderer's virtuals) is a
//! [`TextBoxEnv`] call by address. Checked in the function runner (`tests/pausedraw.rs`). The page stack and the
//! pages' updates are `pausemenu` (G9a1, G9a2).
//!
//! The overlay (`this`): +0x428 / +0x42c its two tables' words (+0x42c the backdrop overlay it draws on: +0 the
//! sprite set, +4 / +8 the menu and title fonts, +0x28 its table, +0x2c its width, +0x70 its alpha), +0x438 the
//! main page and +0x570 the options page (each: +0x14 the item picked, +0x18 the count, +0x1c.. the items, 0x18
//! bytes each: +0 the kind, +8 the value, +0xc its top, +0x10 1 when it is not a slider, +0x14 the label; +0x10c
//! the page's heading), +0x7e8, +0x7f0 whether the trick book's page shows, +0x7f4 the trick shown, +0x7f8 the
//! trick's info (a `cGameTrickInfo`), +0x820 the backdrop.
//!
//! Where the game builds on its stack (colours, strings, the menu list's arrays, a table copy), the port uses
//! `scratch` (STANDIN: the place, not the bytes).

use crate::animseq::{cvt_s_w, div};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const DRAW_COLOUR: u32 = 0x337fe8;
pub const CAREER: u32 = 0x32b390;
pub const CHARACTERS: u32 = 0x331ea8;
/// The music system's pointer
pub const MUSIC: u32 = 0x343050;
/// The three camera names' strings (0x1d8, 0x1da, 0x1d9), filled by the constructor
pub const CAMERA_NAMES: u32 = 0x338230;
/// The backdrop's table for the overlay's view of it, the overlay's own second table
pub const BACKDROP_VTABLE: u32 = 0x38f0c8;
pub const VTABLE2: u32 = 0x38f160;
/// The options page's 16 bytes of colour (copied to the page's +0x110) and the ":" drawn after each label
pub const OPTIONS_COLOUR: u32 = 0x38e380;
pub const COLON: u32 = 0x38e390;

pub const LOC: u32 = 0x2c5160;
pub const SET_COLOUR: u32 = 0x14eee8;
pub const MEMSET: u32 = 0x2cc808;
pub const DRAW_MENU_LIST: u32 = 0x1aa630;
pub const LABEL_BOX: u32 = 0x1a9928;
pub const DRAW_SPRITE: u32 = 0x1c0068;
pub const PRINTF_SHADOWED: u32 = 0x19d3f0;
pub const PRINTF_SHADOWED_CENTRED: u32 = 0x19d6e0;
pub const PRINT_SHADOWED_CENTRED: u32 = 0x19d770;
pub const PRINTF_SHADOWED_A: u32 = 0x19d568;
pub const PRINT_SHADOWED_RIGHT: u32 = 0x19d868;
pub const PRINT_ASCII_SHADOWED_RIGHT: u32 = 0x19d800;
pub const MEASURE_WIDE: u32 = 0x19dc78;
pub const MEASURE_ASCII: u32 = 0x19da30;
pub const WSTR_FORMAT: u32 = 0x17ce38;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const STRLEN: u32 = 0x2fd260;
pub const TRICK_BOOK: u32 = 0x163f18;
pub const COMPLETE_PAGES: u32 = 0x164060;
pub const PLAYER_PROFILE: u32 = 0x164330;
pub const PAGE_FIRST: u32 = 0x164110;
pub const TRICK_DONE: u32 = 0x164258;
pub const FIND_RIDER: u32 = 0x1148e8;
pub const TRICK_GET_INFO: u32 = 0x154638;
pub const FORMAT_TRICK_NAME: u32 = 0x1551c0;
pub const TRICK_INFO_SET: u32 = 0x154688;
pub const TRICK_INFO_DRAW: u32 = 0x154b18;
pub const TRICK_INFO_NEW: u32 = 0x154d20;
pub const GET_RECT: u32 = 0x1f9e50;
pub const SPRITE_TILED: u32 = 0x1fc728;
pub const MUSIC_COUNT: u32 = 0x225588;
pub const MUSIC_SELECT: u32 = 0x2255c0;
pub const MUSIC_NEXT: u32 = 0x225660;
pub const BACKDROP_INIT: u32 = 0x1a8900;
pub const SAVE_INIT: u32 = 0x1b8568;
pub const BUILD_MAIN_PAGE: u32 = 0x1ac678;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn fcall(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32], f: &[F]) -> u32 {
    env.call(m, func, args, f)
}
fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn put4(m: &mut Mem, at: u32, v: [F; 4]) {
    for (i, x) in v.into_iter().enumerate() {
        wf(m, at + 4 * i as u32, x);
    }
}
/// A virtual of the renderer (the app's +0x724): its table +0x10, `this` moved by the half word at `off`, the
/// function at `off + 4`
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32]) {
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let me = r.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[]);
}
fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}

const ONE: F = F(1.0);
const WHITE: [F; 4] = [ONE; 4];
fn gold() -> [F; 4] {
    [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]
}
fn colour(m: &mut Mem, env: &mut dyn TextBoxEnv, at: u32, c: [F; 4]) {
    put4(m, at, c);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, at]);
}
/// The drawer's red (its first word) by the backdrop's alpha
fn fade(m: &mut Mem, hud: u32) {
    let v = rf(m, DRAW_COLOUR) * rf(m, hud + 0x70);
    wf(m, DRAW_COLOUR, v);
}
/// The page's left edge: half of what the backdrop leaves of 480
fn left(m: &Mem, hud: u32) -> F {
    (F(480.0) - rf(m, hud + 0x2c)) * F(0.5)
}

/// `cPauseMenuOverlay::cPauseMenuOverlay` 0x1ab9f8(this, whole): the backdrop (when `whole`: the overlay's own at
/// +0x820, made) and the save overlay's start; the backdrop's table set to the overlay's view of it (when not
/// `whole`, a copy on the stack with its six `this` adjustments moved to where the backdrop is); the second table,
/// the trick's info, the camera names, both pages cleared and built, +0x7e8 −1, +0x7f8 the app's +0x60 and three of
/// the career's words cleared; `this`
pub fn new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, scratch: u32) -> u32 {
    if whole != 0 {
        m.w32(this + 0x42c, this + 0x820);
        call(m, env, BACKDROP_INIT, &[this + 0x820]);
    }
    call(m, env, SAVE_INIT, &[this, 0]);
    let hud = m.r32(this + 0x42c);
    m.w32(hud + 0x28, BACKDROP_VTABLE);
    if whole == 0 {
        for i in 0..0x98 / 4 {
            m.w32(scratch + 4 * i, m.r32(BACKDROP_VTABLE + 4 * i));
        }
        let d = this.wrapping_sub(hud.wrapping_sub(0x820));
        for o in [8u32, 0x18, 0x68, 0x70, 0x78, 0x80] {
            let h = (m.r16(BACKDROP_VTABLE + o) as u32).wrapping_add(d);
            m.w16(scratch + o, h as u16);
        }
        m.w32(hud + 0x28, scratch);
    }
    m.w32(this + 0x428, VTABLE2);
    call(m, env, TRICK_INFO_NEW, &[this + 0x7f8]);
    for (i, id) in [0x1d8u32, 0x1da, 0x1d9].into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(CAMERA_NAMES + 4 * i as u32, s);
    }
    call(m, env, MEMSET, &[this + 0x438, 0, 0x3a8]);
    call(m, env, BUILD_MAIN_PAGE, &[this]);
    build_options_page(m, env, this);
    m.w32(this + 0x7e8, 0xffff_ffff);
    m.w32(this + 0x7f8, m.r32(m.r32(APP) + 0x60));
    for o in [0x370c, 0x3704, 0x3708] {
        m.w32(CAREER + o, 0);
    }
    this
}

/// `PauseMenu_DrawMainPage` 0x1ac7e0(this): the main page's list (all items on) and heading box; with the trick
/// book's page (+0x7f0) also the rider's name, the page's number ("%d", 0xe64, 6 pages: the last shown as 5), the
/// trick's number (0xea7), its name (smaller past 49 characters), the trick's info drawn, the arrows to the tricks
/// before and after on the page, and 0xc93 when the trick is done
pub fn draw_main_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let page = this + 0x438;
    let (labels, on) = (scratch + 0x10, scratch + 0x40);
    let n = m.r32(page + 0x18) as i32;
    for i in 0..n.max(0) as u32 {
        m.w32(on + 4 * i, 1);
        m.w32(labels + 4 * i, m.r32(this + 0x468 + 0x18 * i));
    }
    rcall(m, env, 0x1d0, &[0x10]);
    let x0 = left(m, m.r32(this + 0x42c));
    let s092 = k(0x3f6b_851f);
    if m.r32(this + 0x7f0) == 0 {
        let hud = m.r32(this + 0x42c);
        let (cnt, sel) = (m.r32(page + 0x18), m.r32(page + 0x14));
        fcall(m, env, DRAW_MENU_LIST, &[hud, 8, 0, labels, cnt, sel, 0x74, 1, 1, on], &[k(0x4369_0000), F(240.0), s092]);
        let (hud, l) = (m.r32(this + 0x42c), m.r32(page + 0x10c));
        fcall(m, env, LABEL_BOX, &[hud, l, 0x10], &[x0 + F(-8.0)]);
        return;
    }
    let col = scratch + 0x70;
    let player = m.r32(m.r32(m.r32(m.r32(APP) + 0x730) + 0x38) + 0xc);
    let book = call(m, env, TRICK_BOOK, &[CAREER, player]);
    let pages = call(m, env, COMPLETE_PAGES, &[book]);
    let shown = if pages != 6 { pages } else { 5 };
    let next = shown.wrapping_add(1);
    let profile = call(m, env, PLAYER_PROFILE, &[CAREER, player]);
    let (hud, cnt, sel) = (m.r32(this + 0x42c), m.r32(page + 0x18), m.r32(page + 0x14));
    fcall(m, env, DRAW_MENU_LIST, &[hud, 8, 0, labels, cnt, sel, 0xffff_ffff, 1, 0, on], &[k(0x4369_0000), x0 + F(95.0), s092]);
    let (hud, l) = (m.r32(this + 0x42c), m.r32(page + 0x10c));
    fcall(m, env, LABEL_BOX, &[hud, l, 0x10], &[x0 + F(-8.0)]);
    colour(m, env, col, gold());
    let (s09, y, cx) = (k(0x3f66_6666), F(242.0), F(320.0));
    let name = m.r32(profile.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    let title = m.r32(m.r32(this + 0x42c) + 8);
    fcall(m, env, PRINTF_SHADOWED, &[title, name], &[s09, F(40.0), y]);
    let s = loc(m, env, 0xe64);
    call(m, env, WSTR_FORMAT, &[scratch + 0x80, s, next]);
    let title = m.r32(m.r32(this + 0x42c) + 8);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[title, scratch + 0x80], &[s09, cx, y]);
    colour(m, env, col, gold());
    let s = loc(m, env, 0xea7);
    call(m, env, WSTR_FORMAT, &[scratch + 0xa0, s, m.r32(this + 0x7f4).wrapping_add(1)]);
    let title = m.r32(m.r32(this + 0x42c) + 8);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[title, scratch + 0xa0], &[k(0x3f42_8f5c), k(0x4409_8000), F(245.0)]);
    colour(m, env, col, WHITE);
    let name = scratch + 0xe0;
    let info = call(m, env, TRICK_GET_INFO, &[profile, m.r32(this + 0x7f4)]);
    call(m, env, FORMAT_TRICK_NAME, &[name, info]);
    let len = call(m, env, STRLEN, &[name]);
    let menu = m.r32(m.r32(this + 0x42c) + 4);
    if len < 0x32 {
        fcall(m, env, PRINTF_SHADOWED_CENTRED, &[menu, name], &[k(0x3f4c_cccd), cx, k(0x4388_8000)]);
    } else {
        fcall(m, env, PRINTF_SHADOWED_CENTRED, &[menu, name], &[k(0x3f32_2d0f), cx, k(0x4389_8000)]);
    }
    let race = m.r32(m.r32(m.r32(APP) + 0x730) + 0x1c);
    let rider = call(m, env, FIND_RIDER, &[race, player]);
    let ti = this + 0x7f8;
    call(m, env, TRICK_INFO_SET, &[ti, m.r32(rider + 0x870), profile, m.r32(this + 0x7f4)]);
    fcall(m, env, TRICK_INFO_DRAW, &[ti, m.r32(rider + 0x870)], &[cx, k(0x4397_8000)]);
    colour(m, env, col, gold());
    let first = call(m, env, PAGE_FIRST, &[shown]);
    if (first as i32) < m.r32(this + 0x7f4) as i32 {
        let hud = m.r32(this + 0x42c);
        fcall(m, env, DRAW_SPRITE, &[hud, 0x2b, 0], &[F(30.0), k(0x438d_0000), ONE, ONE]);
    }
    let first = call(m, env, PAGE_FIRST, &[next]);
    if (m.r32(this + 0x7f4) as i32) < first.wrapping_sub(1) as i32 {
        let hud = m.r32(this + 0x42c);
        fcall(m, env, DRAW_SPRITE, &[hud, 0x2c, 0], &[k(0x4415_8000), k(0x438d_0000), ONE, ONE]);
    }
    colour(m, env, col, [ONE, k(0x3f7a_fb7f), k(0x3f48_c7e3), k(0x3e2c_a57a)]);
    if call(m, env, TRICK_DONE, &[book, m.r32(this + 0x7f4)]) != 0 {
        let hud = m.r32(this + 0x42c);
        let s = loc(m, env, 0xc93);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(hud + 4), s], &[k(0x3f4c_cccd), cx, F(330.0)]);
    }
}

/// An options item at `page + 0x1c + 0x18 i`
fn item(m: &mut Mem, page: u32, i: u32, kind: u32, value: u32, top: u32, on: u32, label: u32) {
    let e = page + 0x1c + 0x18 * i;
    m.w32(e, kind);
    m.w32(e + 8, value);
    m.w32(e + 0xc, top);
    m.w32(e + 0x10, on);
    m.w32(e + 0x14, label);
}

/// `PauseMenu_DrawMusicList` 0x1ad198(this): (it draws nothing) the options page's items: the heading 0xeb9; the
/// brightness slider (kind 8, of 50) and the camera (9, of 3); with a player (the race's +0x298) the view (0xb)
/// and with two the second player's (0xc); the song (0xd, of the songs on) and five volumes (0xe, 0x10, 0x13, 0xf,
/// 0x15, of 50). Its measures of the camera names and the songs are thrown away. The page's colour copied in
pub fn build_options_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let page = this + 0x570;
    let heading = loc(m, env, 0xeb9);
    let bright = loc(m, env, 0x1e3);
    loc(m, env, 0x1e5);
    let camera = loc(m, env, 0x1e4);
    let view = loc(m, env, 0x1e1);
    let view2 = loc(m, env, 0x1e2);
    let mut l = [0u32; 6];
    for (i, id) in [0x1e9u32, 0x1e6, 0x1ea, 0x1e8, 0x1e7, 0xc99].into_iter().enumerate() {
        l[i] = loc(m, env, id);
    }
    item(m, page, 0, 8, 0, 0x32, 0, bright);
    item(m, page, 1, 9, 0, 3, 1, camera);
    m.w32(page + 0x10c, heading);
    let race = m.r32(m.r32(APP) + 0x730);
    let mut n = 2u32;
    if m.r32(race + 0x298) as i32 > 0 {
        n = 3;
        let s08 = k(0x3f4c_cccd);
        for id in [0x14cu32, 0x14e, 0x14a, 0x14d] {
            let hud = m.r32(this + 0x42c);
            let s = loc(m, env, id);
            fcall(m, env, MEASURE_WIDE, &[m.r32(hud + 4), s, 0], &[s08]);
        }
        item(m, page, 2, 0xb, 0, 0, 1, view);
        let race = m.r32(m.r32(APP) + 0x730);
        if m.r32(race + 0x298) as i32 >= 2 {
            item(m, page, 3, 0xc, 0, 0, 1, view2);
            n = 4;
        }
    }
    let songs = call(m, env, MUSIC_COUNT, &[m.r32(MUSIC)]);
    call(m, env, MUSIC_SELECT, &[m.r32(MUSIC), 0]);
    for _ in 0..(songs as i32).max(0) {
        let sys = m.r32(MUSIC);
        let hud = m.r32(this + 0x42c);
        let name = sys.wrapping_add(m.r32(sys + 0x36e8) << 5).wrapping_add(0x12e0);
        fcall(m, env, MEASURE_ASCII, &[m.r32(hud + 4), name, 0], &[k(0x3f4c_cccd)]);
        call(m, env, MUSIC_NEXT, &[m.r32(MUSIC)]);
    }
    call(m, env, MUSIC_SELECT, &[m.r32(MUSIC), 0]);
    item(m, page, n, 0xd, 0, songs, 1, l[0]);
    for (j, kind) in [0xeu32, 0x10, 0x13, 0xf, 0x15].into_iter().enumerate() {
        item(m, page, n + 1 + j as u32, kind, 0, 0x32, 0, l[j + 1]);
    }
    m.w32(page + 0x18, n + 6);
    for i in 0..4 {
        m.w32(page + 0x110 + 4 * i, m.r32(OPTIONS_COLOUR + 4 * i));
    }
}

/// `PauseMenu_DrawOptionsPage` 0x1ad628(this): the heading box, then each item a row (the rows centred on 240, the
/// menu font's height · 0.8 + 6 apart): its label and ":" right-aligned (gold when picked), its value at 305 (the
/// camera's name, the song's name, or for the views 0x14a / 0x14d / 0x14c / 0x14e by the value), or a slider (its
/// value out of its top, tiled at 135 · the share); the picked row's arrows either side. Every colour but the
/// slider's by the backdrop's alpha
pub fn draw_options_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let page = this + 0x570;
    let text = scratch + 0x10;
    colour(m, env, scratch, WHITE);
    fade(m, m.r32(this + 0x42c));
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1f8, &[1, 0]);
    rcall(m, env, 0x1d0, &[0x10]);
    let hud = m.r32(this + 0x42c);
    let x = left(m, hud) - F(8.0);
    fcall(m, env, LABEL_BOX, &[hud, m.r32(page + 0x10c), 0x10], &[x]);
    let hud = m.r32(this + 0x42c);
    let s08 = k(0x3f4c_cccd);
    let step = cvt_s_w(m.r32(m.r32(hud + 4) + 0xc) as i32) * s08 + F(6.0);
    let r = call(m, env, GET_RECT, &[m.r32(hud), 0x56]);
    let rect_w = rf(m, r + 8);
    let hud = m.r32(this + 0x42c);
    let r = call(m, env, GET_RECT, &[m.r32(hud), 0x56]);
    let rect_h = rf(m, r + 4);
    let n = m.r32(page + 0x18) as i32;
    let mut y = F(240.0) - step * cvt_s_w(n) * F(0.5);
    if n <= 0 {
        return;
    }
    let (vx, bar) = (k(0x4398_8000), F(135.0));
    let mut i = 0u32;
    loop {
        let e = page + 0x1c + 0x18 * i;
        let mut w = bar;
        let picked = i == m.r32(page + 0x14);
        colour(m, env, scratch, if picked { gold() } else { WHITE });
        let hud = m.r32(this + 0x42c);
        fade(m, hud);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[m.r32(hud + 4), m.r32(e + 0x14)], &[s08, F(264.0), y]);
        let hud = m.r32(this + 0x42c);
        fcall(m, env, PRINT_ASCII_SHADOWED_RIGHT, &[m.r32(hud + 4), COLON], &[s08, F(270.0), y]);
        colour(m, env, scratch, WHITE);
        let hud = m.r32(this + 0x42c);
        fade(m, hud);
        let kind = m.r32(e) as i32;
        if kind < 0xd {
            if kind >= 0xb {
                let id = match m.r32(e + 8) {
                    2 => 0x14c,
                    3 => 0x14e,
                    1 => 0x14d,
                    0 => 0x14a,
                    _ => 0x14d,
                };
                let s = loc(m, env, id);
                call(m, env, WSTR_COPY, &[text, s]);
                let hud = m.r32(this + 0x42c);
                let v = fcall(m, env, MEASURE_WIDE, &[m.r32(hud + 4), text, 0], &[s08]);
                w = cvt_s_w(v as i32);
                let hud = m.r32(this + 0x42c);
                fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(hud + 4), text], &[s08, vx, y]);
            } else if kind == 9 {
                let name = m.r32(CAMERA_NAMES + (m.r32(e + 8) << 2));
                fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(hud + 4), name], &[s08, vx, y]);
                let hud = m.r32(this + 0x42c);
                let name = m.r32(CAMERA_NAMES + (m.r32(e + 8) << 2));
                let v = fcall(m, env, MEASURE_WIDE, &[m.r32(hud + 4), name, 0], &[s08]);
                w = cvt_s_w(v as i32);
            }
        } else if kind == 0xd {
            let sys = m.r32(MUSIC);
            let name = sys.wrapping_add(m.r32(sys + 0x36e8) << 5).wrapping_add(0x12e0);
            fcall(m, env, PRINTF_SHADOWED, &[m.r32(hud + 4), name], &[s08, vx, y]);
            let hud = m.r32(this + 0x42c);
            let v = fcall(m, env, MEASURE_ASCII, &[m.r32(hud + 4), name, 0], &[s08]);
            w = cvt_s_w(v as i32);
        }
        if m.r32(e + 0x10) == 0 {
            let top = cvt_s_w(m.r32(e + 0xc) as i32);
            let share = div(cvt_s_w(m.r32(e + 8) as i32), top);
            let by = y + F(3.0);
            colour(m, env, scratch, WHITE);
            let s055 = k(0x3f0c_cccd);
            rcall(m, env, 0x1d0, &[0x10]);
            let hud = m.r32(this + 0x42c);
            fcall(m, env, DRAW_SPRITE, &[hud, 0x56, 0], &[vx, by, div(bar, rect_h), s055]);
            rcall(m, env, 0x1d0, &[0x11]);
            let hud = m.r32(this + 0x42c);
            let r = call(m, env, GET_RECT, &[m.r32(hud), 0x57]);
            fcall(m, env, SPRITE_TILED, &[r, 0], &[vx, by, share * bar, rect_w * s055, share, ONE]);
        }
        if i == m.r32(page + 0x14) {
            let ay = y + ONE;
            colour(m, env, scratch, gold());
            let hud = m.r32(this + 0x42c);
            fade(m, hud);
            fcall(m, env, DRAW_SPRITE, &[hud, 0x2b, 0], &[k(0x438d_0000), ay, s08, s08]);
            let hud = m.r32(this + 0x42c);
            fcall(m, env, DRAW_SPRITE, &[hud, 0x2c, 0], &[w + vx + F(5.0), ay, s08, s08]);
        }
        i += 1;
        y = y + step;
        if i as i32 >= m.r32(page + 0x18) as i32 {
            break;
        }
    }
}

// --- G9a5: the pause overlay's own virtuals (the index has them as `cCountdownOverlay`'s) ---

pub const BACKDROP_RESET: u32 = 0x1aa628;
pub const START_FRAME_ANIM: u32 = 0x1a89b0;
pub const START_CHILD_SLIDE: u32 = 0x1a8ad8;
pub const START_EDGE_SLIDE: u32 = 0x1a8b70;
pub const BACKDROP_UPDATE: u32 = 0x1a8bd8;
pub const BACKDROP_FRAME: u32 = 0x1a9bb8;
pub const PAUSE_RESUME: u32 = 0x1ac410;
pub const POP_LEVEL: u32 = 0x1ac580;
pub const FIRST_UNDONE: u32 = 0x1641f8;
pub const GET_VOL: u32 = 0x211f20;
pub const GET_BRIGHT: u32 = 0x17b970;
pub const RACE_MODE_A: u32 = 0x17f158;
pub const RACE_MODE_B: u32 = 0x17f190;
pub const GET_CAMERA: u32 = 0x181470;
pub const CAM_GET: u32 = 0x173e08;
pub const DIALOG_DRAW: u32 = 0x1aa268;
pub const DIALOG_POLL: u32 = 0x1aac08;
pub const MC_DRAW: u32 = 0x1cf628;
pub const MC_UPDATE: u32 = 0x1ceee0;
pub const MC_IS_DONE: u32 = 0x1d21a0;
pub const MC_RESULT: u32 = 0x1d21c0;
pub const MC_DESTROY: u32 = 0x1ceeb0;
pub const MC_CREATE: u32 = 0x1cee68;
pub const MC_BEGIN: u32 = 0x1ce8b0;
pub const MC_START_SAVE: u32 = 0x1ce778;
pub const MC_RESTART: u32 = 0x1d2140;
pub const PLAYER_ID: u32 = 0x166580;
pub const CLEAR_FLAGS: u32 = 0x165400;
pub const CAREER_SAVE_A: u32 = 0x161a98;
pub const CAREER_SAVE_B: u32 = 0x161840;
pub const LESSON_END: u32 = 0x14b638;
pub const QUIT_RACE: u32 = 0x180a10;

/// A page's member-function pointer at `pmf` called on the overlay (GCC 2's layout: +0 the `this` shift, +2 the
/// index (0 none; above 0 a virtual: its table at `this` + the half word at +4, the entry index · 8 − 8 with its own
/// shift in its low half word and the function in its high word), +4 the function itself when the index is below 0)
fn member_call(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, pmf: u32) {
    let idx = m.r16(pmf + 2) as i16;
    if idx == 0 {
        return;
    }
    let delta = m.r16(pmf) as i16 as u32;
    let (func, me) = if idx > 0 {
        let vt = m.r32(this.wrapping_add(m.r16(pmf + 4) as i16 as u32));
        let e = vt.wrapping_add(((idx as i32 as u32) << 3).wrapping_sub(8));
        let shift = m.r16(e) as i16 as u32;
        (m.r32(e + 4), this.wrapping_add(shift.wrapping_add(delta)))
    } else {
        (m.r32(pmf + 4), this.wrapping_add(delta))
    };
    env.call(m, func, &[me], &[]);
}
fn page_of(m: &Mem, this: u32) -> u32 {
    m.r32(this + 0x7e0 + (m.r32(this + 0x7e8) << 2))
}

/// `cPauseMenuOverlay` vf34 0x1abc00(this): opened: the backdrop reset, the pages back to the main one, the frame's
/// animations started; the trick book's page from the first trick not done (else page 5's first), and the options
/// page's values read: the brightness · 50, the race's two modes, the two players' cameras, the song, the five
/// volumes · 50 (kinds 8..21; the others 0); no card operation, no trick book shown
pub fn open(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let z = F(0.0);
    call(m, env, BACKDROP_RESET, &[m.r32(this + 0x42c)]);
    call(m, env, PAUSE_RESUME, &[this, 0]);
    fcall(m, env, START_FRAME_ANIM, &[m.r32(this + 0x42c), 0x40], &[z]);
    fcall(m, env, START_FRAME_ANIM, &[m.r32(this + 0x42c), 2], &[k(0x3f61_47ae)]);
    fcall(m, env, START_CHILD_SLIDE, &[m.r32(this + 0x42c), 2], &[z]);
    fcall(m, env, START_EDGE_SLIDE, &[m.r32(this + 0x42c), 2], &[z]);
    m.w32(this + 0x7ec, 1);
    let player = m.r32(m.r32(m.r32(m.r32(APP) + 0x730) + 0x38) + 0xc);
    let book = call(m, env, TRICK_BOOK, &[CAREER, player]);
    let t = call(m, env, FIRST_UNDONE, &[book]);
    m.w32(this + 0x7f4, t);
    if t == 0xffff_ffff || (t as i32) >= 0x1e {
        let f = call(m, env, PAGE_FIRST, &[5]);
        m.w32(this + 0x7f4, f);
    }
    let page = this + 0x570;
    let mut i = 0i32;
    while i < m.r32(page + 0x18) as i32 {
        let e = page + 0x1c + 0x18 * i as u32;
        let vol = |m: &mut Mem, env: &mut dyn TextBoxEnv, ch: u32| {
            let v = call(m, env, GET_VOL, &[m.r32(MUSIC), ch]);
            crate::sndlib::cvt_w((F(f32::from_bits(v)) * F(50.0)).0) as u32
        };
        let v = match m.r32(e) {
            8 => {
                let b = call(m, env, GET_BRIGHT, &[m.r32(APP)]);
                crate::sndlib::cvt_w((F(f32::from_bits(b)) * F(50.0)).0) as u32
            }
            9 => call(m, env, RACE_MODE_A, &[m.r32(m.r32(APP) + 0x730)]),
            0xa => call(m, env, RACE_MODE_B, &[m.r32(m.r32(APP) + 0x730)]),
            0xb | 0xc => {
                let c = call(m, env, GET_CAMERA, &[m.r32(e) - 0xb]);
                call(m, env, CAM_GET, &[c])
            }
            0xd => m.r32(m.r32(MUSIC) + 0x36e8),
            0xe => vol(m, env, 0),
            0xf => vol(m, env, 1),
            0x10 => vol(m, env, 2),
            0x13 => vol(m, env, 4),
            0x15 => vol(m, env, 3),
            _ => 0,
        };
        m.w32(e + 8, v);
        i += 1;
    }
    m.w32(this + 0x430, 0);
    m.w32(this + 0x7f0, 0);
}

/// `cPauseMenuOverlay` vf31 0x1abe70(this): the dialog when one is up; else (no card operation) the backdrop's frame
/// as high as the page's items need (the menu font's height · 0.92 + 4 each, from 100 (95 on the main page), 120
/// more with the trick book), on texture 16 the page's draw (its member function +0x118), and the help line 0x1d7
/// (once the frame is open)
pub fn draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let idx = m.r32(this + 0x7e8);
    let hud = m.r32(this + 0x42c);
    let page = page_of(m, this);
    if m.r32(hud + 0x328) != 0 {
        call(m, env, DIALOG_DRAW, &[hud]);
        return;
    }
    if m.r32(this + 0x430) == 2 {
        return;
    }
    let top = if idx == 0 { F(95.0) } else { F(100.0) };
    let font = m.r32(hud + 4);
    let rows = cvt_s_w(m.r32(page + 0x18) as i32);
    let each = cvt_s_w(m.r32(font + 0xc) as i32) * k(0x3f6b_851f) + F(4.0);
    let mut h = rows * each + top;
    if m.r32(this + 0x7f0) != 0 {
        h = h + F(120.0);
    }
    fcall(m, env, BACKDROP_FRAME, &[hud, 0xe], &[h]);
    rcall(m, env, 0x1d0, &[0x10]);
    member_call(m, env, this, page + 0x118);
    let y = (h + F(480.0)) * F(0.5) - F(40.0);
    let s = loc(m, env, 0x1d7);
    let (labels, on) = (scratch + 0x10, scratch + 0x14);
    m.w32(labels, s);
    let hud = m.r32(this + 0x42c);
    m.w32(on, 1);
    if rf(m, hud + 0x34).ee() == rf(m, hud + 0x2c).ee() {
        fcall(m, env, DRAW_MENU_LIST, &[hud, 3, 0, labels, 1, 0xffff_ffff, 0x73, 1, 0, on], &[F(600.0), y, k(0x3f3a_e148)]);
    }
}

/// `cPauseMenuOverlay` vf32 0x1ac080(this): unless a dialog is up: the card operation's draw while one runs, else
/// the page's text (its member function +0x120)
pub fn draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if m.r32(m.r32(this + 0x42c) + 0x328) != 0 {
        return;
    }
    if m.r32(this + 0x430) == 2 {
        call(m, env, MC_DRAW, &[this, this]);
        return;
    }
    let page = page_of(m, this);
    member_call(m, env, this, page + 0x120);
}

fn save_career(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    m.w32(CAREER + 0x3704, 1);
    call(m, env, CAREER_SAVE_A, &[CAREER]);
    call(m, env, CAREER_SAVE_B, &[CAREER]);
    m.w32(CAREER + 0x3260, 2);
    let r = m.r32(m.r32(APP) + 0x730);
    m.w32(r + 0x54, 1);
}
fn clear_players(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    for p in 0..4 {
        let id = call(m, env, PLAYER_ID, &[CAREER, p]);
        call(m, env, CLEAR_FLAGS, &[CAREER, id]);
    }
}

/// `cPauseMenuOverlay` vf33 0x1ac130(this): the backdrop's update; a card operation run to its end (a good result
/// saves the career); a dialog's answer: no (2) closes it, yes (1) to the save question starts the save (+0x434 0x1c4,
/// the operation 0x27d4), else the race is left (the career saved when +0x7ec, else the players' flags cleared and
/// the lesson ended or the race quit) and the page popped; no dialog: the page's update (+0x110)
pub fn update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_UPDATE, &[m.r32(this + 0x42c)]);
    if m.r32(this + 0x430) == 2 {
        call(m, env, MC_UPDATE, &[this]);
        if call(m, env, MC_IS_DONE, &[this]) == 0 {
            return;
        }
        if call(m, env, MC_RESULT, &[this]) != 0 {
            save_career(m, env);
        }
        m.w32(this + 0x430, 0);
        call(m, env, MC_DESTROY, &[this]);
        return;
    }
    let hud = m.r32(this + 0x42c);
    if m.r32(hud + 0x328) == 0 {
        let page = page_of(m, this);
        member_call(m, env, this, page + 0x110);
        return;
    }
    let r = call(m, env, DIALOG_POLL, &[hud, 0xffff_ffff]);
    if r == 0 {
        return;
    }
    if r == 3 && m.r32(this + 0x430) != 1 {
        return;
    }
    if r == 2 {
        m.w32(this + 0x430, 0);
        return;
    }
    if r == 1 && m.r32(this + 0x430) == 1 {
        m.w32(this + 0x430, 2);
        clear_players(m, env);
        m.w32(this + 0x434, 0x1c4);
        call(m, env, MC_CREATE, &[this]);
        call(m, env, MC_BEGIN, &[this, 0x27d4]);
        call(m, env, MC_START_SAVE, &[this, 1]);
        call(m, env, MC_RESTART, &[this]);
        return;
    }
    if m.r32(this + 0x7ec) != 0 {
        save_career(m, env);
    } else {
        clear_players(m, env);
        let r = m.r32(m.r32(APP) + 0x730);
        let lesson = m.r32(r + 0x34);
        if lesson != 0 {
            call(m, env, LESSON_END, &[lesson]);
        } else {
            call(m, env, QUIT_RACE, &[r]);
        }
    }
    call(m, env, POP_LEVEL, &[this]);
}

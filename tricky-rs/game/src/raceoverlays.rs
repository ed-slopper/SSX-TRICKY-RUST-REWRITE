//! The race's other overlays, part one (board row G8n1): the dialog's draw, the results overlay's constructor and
//! its title helpers, the countdown overlay and the async load's "please wait". On the game's bytes ([`Mem`]); the
//! backdrop's frame, the HUD's sprites, the fonts, the renderer's virtuals and the race's queries are
//! [`TextBoxEnv`] calls by address. Checked in the function runner (`tests/raceoverlays.rs`).
//!
//! An overlay (`this`) begins as a `cScreenOverlay` (+0 its sprite set or backdrop, +4 / +8 the menu and title
//! fonts); a dialog keeps two lines (+0x328, +0x32c), three button words (+0x330, +0x338, +0x334), its size
//! (+0x33c, +0x340) and its buttons' indent (+0x344), as `Dialog_Set` lays them out.
//!
//! Where the game builds on its stack (a colour, a table copy), the port uses `scratch` (STANDIN: the place, not
//! the bytes).

use crate::animseq::{cvt_s_w, div};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const DRAW_COLOUR: u32 = 0x337fe8;
pub const CAREER: u32 = 0x32b390;
/// The results overlay's three words (0xe1e, 0xe1f, 0xe1e), its titles' x and rows (17 floats), and two buffers
/// cleared at the start of the results
pub const RESULT_WORDS: u32 = 0x3384c0;
pub const TITLE_X: u32 = 0x3385bc;
pub const TITLE_ROWS: u32 = 0x338550;
pub const RESULT_BUF_A: u32 = 0x33c5c0;
pub const RESULT_BUF_B: u32 = 0x3385c0;
/// The backdrop's table for the results overlay's view of it
pub const RESULTS_BACKDROP_VTABLE: u32 = 0x38f880;

pub const LOC: u32 = 0x2c5160;
pub const SET_COLOUR: u32 = 0x14eee8;
pub const MEMSET: u32 = 0x2cc808;
pub const FILLED_RECT: u32 = 0x19b108;
pub const MEASURE_WIDE: u32 = 0x19dc78;
pub const PRINTF_SHADOWED_A: u32 = 0x19d568;
pub const PRINT_SHADOWED_CENTRED: u32 = 0x19d770;
pub const DRAW_SPRITE: u32 = 0x1c0068;
pub const BACKDROP_FRAME: u32 = 0x1a9bb8;
pub const BACKDROP_INIT: u32 = 0x1a8900;
pub const GET_RECT: u32 = 0x1f9e50;
pub const COURSE_NAME: u32 = 0x16b650;
pub const OVERLAY_HIDDEN: u32 = 0x1a77f8;
pub const OVERLAY_HIDE: u32 = 0x1a7860;
pub const RACE_PLAYERS: u32 = 0x17f158;
pub const COUNT_DIGITS: u32 = 0x1a82e8;
pub const SIN: u32 = 0x250d60;

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
/// A virtual of the renderer (the app's +0x724)
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let me = r.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[])
}
fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}
fn race(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x730)
}
const ONE: F = F(1.0);

/// A dialog's button: its pad sprite, then its word 23 on and 2 down; the next button 40 past the word
fn button(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, sprite: u32, word: u32, x: F, y: F, last: bool) -> F {
    fcall(m, env, DRAW_SPRITE, &[this, sprite, 0], &[x, y, ONE, ONE]);
    let step = F(23.0);
    if last {
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(this + 4), word], &[ONE, x + step, y + F(2.0)]);
        fcall(m, env, MEASURE_WIDE, &[m.r32(this + 4), word, 0], &[ONE]);
        return x;
    }
    let x = x + step;
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(this + 4), word], &[ONE, x, y + F(2.0)]);
    let w = fcall(m, env, MEASURE_WIDE, &[m.r32(this + 4), word, 0], &[ONE]);
    x + (cvt_s_w(w as i32) + F(40.0) - step)
}

/// `Dialog_Draw` 0x1aa268(this): untextured mode 3, the backdrop's frame (layer 17, 165 high) on texture 18, then on
/// texture 19 in white the (up to two) lines centred in the box from 20 down, 30 apart, and the buttons in a row
/// from the indent: A (sprite 0x74, +0x330), B (0x72, +0x338), C (0x73, +0x334); the texture put back
pub fn dialog_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1f8, &[1, 0]);
    let tex = rcall(m, env, 0x1d8, &[]);
    rcall(m, env, 0x1d0, &[0x12]);
    let half = F(0.5);
    let x0 = F(320.0) - rf(m, this + 0x33c) * half;
    let y0 = F(240.0) - rf(m, this + 0x340) * half;
    fcall(m, env, BACKDROP_FRAME, &[this, 0x11], &[k(0x4325_0000)]);
    rcall(m, env, 0x1d0, &[0x13]);
    wf(m, DRAW_COLOUR + 0xc, ONE);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 4, ONE);
    wf(m, DRAW_COLOUR + 8, ONE);
    let mut y = y0 + F(20.0);
    for i in 0..2u32 {
        let s = m.r32(this + 0x328 + 4 * i);
        if s == 0 {
            break;
        }
        let w = fcall(m, env, MEASURE_WIDE, &[m.r32(this + 4), s, 0], &[ONE]);
        let x = x0 + (rf(m, this + 0x33c) - cvt_s_w(w as i32)) * half;
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(this + 4), s], &[ONE, x, y]);
        y = y + F(30.0);
    }
    let mut x = x0 + rf(m, this + 0x344);
    for (sprite, o) in [(0x74u32, 0x330u32), (0x72, 0x338)] {
        let word = m.r32(this + o);
        if word != 0 {
            x = button(m, env, this, sprite, word, x, y, false);
        }
    }
    let word = m.r32(this + 0x334);
    if word != 0 {
        button(m, env, this, 0x73, word, x, y, true);
    }
    rcall(m, env, 0x1d0, &[tex]);
}

/// `ResultsOverlay_Init` 0x1aacb0(this, whole): the backdrop (when `whole`, its own at +0x54, made) given the
/// results' view of its table (when not, a copy on the stack with three `this` adjustments moved); the three words;
/// +0x3c..+0x50 cleared; `this`
pub fn results_init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, scratch: u32) -> u32 {
    if whole != 0 {
        m.w32(this, this + 0x54);
        call(m, env, BACKDROP_INIT, &[this + 0x54]);
    }
    let b = m.r32(this);
    m.w32(b + 0x28, RESULTS_BACKDROP_VTABLE);
    if whole == 0 {
        for i in 0..0x98 / 4 {
            m.w32(scratch + 4 * i, m.r32(RESULTS_BACKDROP_VTABLE + 4 * i));
        }
        let d = this.wrapping_sub(b.wrapping_sub(0x54));
        for o in [0x78u32, 8, 0x70] {
            let h = (m.r16(RESULTS_BACKDROP_VTABLE + o) as u32).wrapping_add(d);
            m.w16(scratch + o, h as u16);
        }
        m.w32(b + 0x28, scratch);
    }
    for (i, id) in [0xe1eu32, 0xe1f, 0xe1e].into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(RESULT_WORDS + 4 * i as u32, s);
    }
    for o in (0x3c..=0x50).step_by(4) {
        m.w32(this + o, 0);
    }
    this
}

/// 0x1aae10(this; x, gap): the results' titles' x kept, and their rows from x + the menu font's height · 0.85 + 12,
/// each the height · 0.85 + `gap` below the last (17 in all)
pub fn title_rows(m: &mut Mem, this: u32, x: F, gap: F) {
    let font = m.r32(m.r32(this) + 4);
    wf(m, TITLE_X, x);
    let h = cvt_s_w(m.r32(font + 0xc) as i32) * k(0x3f59_999a);
    let step = h + gap;
    wf(m, TITLE_ROWS, x + h + F(12.0));
    for i in 1..17 {
        let v = rf(m, TITLE_ROWS + 4 * (i - 1)) + step;
        wf(m, TITLE_ROWS + 4 * i, v);
    }
}

/// 0x1aae88: the results' two buffers cleared (0x400 and 0x4000 bytes)
pub fn clear_buffers(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, MEMSET, &[RESULT_BUF_A, 0, 0x400]);
    call(m, env, MEMSET, &[RESULT_BUF_B, 0, 0x4000]);
}

/// `Overlay_DrawCourseTitle` 0x1aaec0(this; height): the backdrop's frame (layer 5), then on texture 7 the course's
/// name in a pale blue by the backdrop's alpha, at 116 and 7 below the frame's top (its rect 0x37's +8 added)
pub fn draw_course_title(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, height: F, scratch: u32) {
    let b = m.r32(this);
    let r = call(m, env, GET_RECT, &[m.r32(b), 0x37]);
    let top = (F(480.0) - height) * F(0.5) + rf(m, r + 8);
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this), 5], &[height]);
    rcall(m, env, 0x1d0, &[7]);
    put4(m, scratch, [ONE, k(0x3f09_78d5), k(0x3f45_e354), k(0x3f53_b646)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let b = m.r32(this);
    let v = rf(m, DRAW_COLOUR) * rf(m, b + 0x70);
    let y = top + F(7.0);
    let t = m.r32(m.r32(APP) + 0x50);
    wf(m, DRAW_COLOUR, v);
    let name = call(m, env, COURSE_NAME, &[CAREER, m.r32(CAREER + 0x2af8)]);
    let s = call(m, env, LOC, &[t, name]);
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b + 8), s], &[k(0x3f47_ae14), F(116.0), y]);
}

/// 0x1ab028(this): texture 5, white, untextured mode 3: a divider (3 wide), longer and further left in the
/// career's mode 3
pub fn draw_divider(m: &mut Mem, env: &mut dyn TextBoxEnv, _this: u32, scratch: u32) {
    rcall(m, env, 0x1d0, &[5]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    rcall(m, env, 0x1e0, &[3]);
    if m.r32(CAREER + 0x2afc) == 3 {
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[k(0x438f_4000), F(155.0), F(3.0), F(260.0)]);
    } else {
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[k(0x439e_4000), F(120.0), F(3.0), F(214.0)]);
    }
}

/// The count's whole seconds to go and the fraction from the race's clock (its +0x14 in 60ths): +0x30 1 − the whole,
/// +0x34 1 − the clock + the whole
fn count_from_clock(m: &mut Mem, this: u32, race1c: u32) {
    let f = rf(m, race1c + 0x14) * k(0x3c88_8889);
    let v = crate::sndlib::cvt_w(f.0).wrapping_neg();
    m.w32(this + 0x30, v.wrapping_add(1) as u32);
    wf(m, this + 0x34, ONE - f - cvt_s_w(v));
}

/// `cCountdownOverlay::Reset` 0x1ae350(this): 90 frames of "GO", the count from the race's clock
pub fn countdown_reset(m: &mut Mem, this: u32) {
    m.w32(this + 0x2c, 0x5a);
    let r1 = m.r32(race(m) + 0x1c);
    count_from_clock(m, this, r1);
}

/// `cCountdownOverlay::Update` 0x1ae3b8(this): unless hidden, the count from the clock while the race is counting
/// down (+0x1c 3); after, "GO" fading over its 90 frames, then the overlay (item 4) hidden
pub fn countdown_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if call(m, env, OVERLAY_HIDDEN, &[m.r32(race(m) + 0x38)]) != 0 {
        return;
    }
    let r = race(m);
    let r1 = m.r32(r + 0x1c);
    if m.r32(r1 + 0x1c) == 3 {
        count_from_clock(m, this, r1);
        return;
    }
    let c = m.r32(this + 0x2c).wrapping_sub(1);
    m.w32(this + 0x2c, c);
    if (c as i32) < 0 {
        call(m, env, OVERLAY_HIDE, &[m.r32(r + 0x38), 4, 0xffff_ffff]);
        return;
    }
    m.w32(this + 0x30, 0);
    wf(m, this + 0x34, ONE - div(cvt_s_w(c as i32), F(90.0)));
}

/// `cCountdownOverlay::Draw` 0x1ae4b0(this): unless hidden or in a two-player race, on texture 19: "GO" (0xeaf)
/// growing and fading (its first half full) in magenta-white, else the count's digits (0x1a82e8)
pub fn countdown_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if call(m, env, OVERLAY_HIDDEN, &[m.r32(race(m) + 0x38)]) != 0 {
        return;
    }
    if call(m, env, RACE_PLAYERS, &[race(m)]) == 2 {
        return;
    }
    rcall(m, env, 0x1d0, &[0x13]);
    let pale = k(0x3e4c_cccd);
    if m.r32(this + 0x30) == 0 {
        let t = rf(m, this + 0x34);
        let a = if F(0.5).ee() < t.ee() { F(2.0) - (t + t) } else { ONE };
        wf(m, DRAW_COLOUR, a);
        wf(m, DRAW_COLOUR + 0xc, pale);
        wf(m, DRAW_COLOUR + 4, pale);
        wf(m, DRAW_COLOUR + 8, ONE);
        let t = rf(m, this + 0x34);
        let scale = t * k(0x4060_0000);
        let y = F(215.0) - (t - ONE) * F(13.0);
        let s = loc(m, env, 0xeaf);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(this + 8), s], &[scale, F(320.0), y]);
    } else {
        wf(m, DRAW_COLOUR, ONE);
        wf(m, DRAW_COLOUR + 0xc, pale);
        wf(m, DRAW_COLOUR + 8, ONE);
        wf(m, DRAW_COLOUR + 4, pale);
        let n = m.r32(this + 0x30);
        fcall(m, env, COUNT_DIGITS, &[this, n, 0x60], &[F(320.0), F(215.0), rf(m, this + 0x34), F(0.0)]);
    }
}

/// `cPopAsyncLoadOverlay::Update` 0x1ae650(this): its frames
pub fn pop_async_update(m: &mut Mem, this: u32) {
    m.w32(this + 0x2c, m.r32(this + 0x2c).wrapping_add(1));
}

/// `cPopAsyncLoadOverlay::Draw` 0x1ae660(this): on texture 19 in white, 0x175 right-aligned at 600 on 440 (the
/// app's +0x60 font); a sine of its frames taken and not used
pub fn pop_async_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    rcall(m, env, 0x1d0, &[0x13]);
    let font = m.r32(m.r32(APP) + 0x60);
    let s = loc(m, env, 0x175);
    let w = fcall(m, env, MEASURE_WIDE, &[font, s, 0], &[ONE]);
    let x = F(600.0) - cvt_s_w(w as i32);
    for o in [0, 4, 8, 0xc] {
        wf(m, DRAW_COLOUR + o, ONE);
    }
    let s = loc(m, env, 0x175);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s], &[ONE, x, F(440.0)]);
    let a = cvt_s_w(m.r32(this + 0x2c) as i32) * k(0x3c08_8889) * k(0x4049_0fdb);
    fcall(m, env, SIN, &[], &[a]);
    for o in [0xc, 0, 4, 8] {
        wf(m, DRAW_COLOUR + o, ONE);
    }
}

/// `cPopAsyncLoadOverlay::DrawText` 0x1ae7a0: nothing
pub fn pop_async_draw_text() {}

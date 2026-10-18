//! The race's other overlays (board rows G8n1, G8n2): the dialog's draw, the results overlay's constructor and
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

// --- G8n2: the pop finish overlay and the black fade ---

pub const MUSIC: u32 = 0x343050;
/// The race's event table (the career's +0x27dc: +0x31c its course, +0x320 its mode, +0x32c its target time)
pub const RACE_TABLE: u32 = 0x32db6c;
/// The display mode (3 the small screen) and each player's finishing time
pub const DISPLAY_MODE: u32 = 0x32ca50;
pub const FINISH_TIMES: u32 = 0x3ce368;
pub const FMT_PLACE_TIME: u32 = 0x38e200;
pub const FMT_PLUS: u32 = 0x38e400;
pub const FMT_MEDAL: u32 = 0x38e408;

pub const OVERLAY_GET: u32 = 0x1a7be0;
pub const SND_STOP_ONE: u32 = 0x228d10;
pub const PRINT_SHADOWED_RIGHT: u32 = 0x19d868;
pub const WSTR_FORMAT: u32 = 0x17ce70;
pub const FORMAT_TIME: u32 = 0x1a6c90;
pub const FORMAT_SCORE: u32 = 0x1a6ba0;
pub const RIDER_SCORE: u32 = 0x11f2a0;
pub const RIDER_TIME: u32 = 0x11ea28;
pub const CAREER_BEST: u32 = 0x163468;
pub const TIME_MEDAL: u32 = 0x16baa8;
pub const PLACE_MEDAL: u32 = 0x16bb58;
pub const CAREER_STATE: u32 = 0x1635b0;
pub const FADE_SHOWS: u32 = 0x193ce0;

/// A player's record in the pop finish overlay (0x3c each from +0x2c): +0 shown, +4 the frames to wait, +8 waiting,
/// +0xc the time trial's, +0x10 its medal sprite (−1 none), +0x14 the place's character, +0x16 the score's text, +0x38
/// its alpha
fn pop(this: u32, i: u32) -> u32 {
    this + 0x2c + 0x3c * i
}
fn rider(m: &Mem, i: u32) -> u32 {
    m.r32(race(m).wrapping_add(i << 7) + 0xa4)
}
fn medal_sprite(v: u32) -> u32 {
    match v {
        1 => 0x28,
        0 => 0x29,
        2 => 0x27,
        _ => 0xffff_ffff,
    }
}

/// `cPopFinishOverlay::Reset` 0x1ae7a8(this, player): (a player below 0 the first) waiting 8 frames, unseen
pub fn pop_finish_reset(m: &mut Mem, this: u32, player: u32) {
    let p = if (player as i32) > -1 { player } else { 0 };
    let r = pop(this, p);
    wf(m, r + 0x38, F(0.0));
    m.w32(r + 4, 8);
    m.w32(r + 8, 1);
}

/// `cPopFinishOverlay` vf16 0x1ae7e0: the sound system's channel 0 stopped
pub fn pop_finish_vf16(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, SND_STOP_ONE, &[m.r32(MUSIC), 0]);
}

/// `cPopFinishOverlay::Update` 0x1ae7f0(this): unless hidden, each waiting player's wait run down; then, by the
/// career's mode: the race's (modes 2, 4: the place's medal and character, the score), the time trial's (3, 5: the
/// time ("%s" of the rider's +0x5910) and, in a single race, its medal), the free ride's (7: nothing shown, its own
/// text), the show-off's (9: the score)
pub fn pop_finish_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if call(m, env, OVERLAY_HIDDEN, &[m.r32(race(m) + 0x38)]) != 0 {
        return;
    }
    for i in 0..2u32 {
        let p = pop(this, i);
        if m.r32(p + 8) == 0 {
            continue;
        }
        let t = m.r32(p + 4).wrapping_sub(1);
        m.w32(p + 4, t);
        if t != 0 {
            continue;
        }
        m.w32(p + 8, 0);
        let mode = m.r32(CAREER + 0x2afc);
        m.w32(p, 0);
        m.w16(p + 0x16, 0);
        m.w32(p + 0xc, 0);
        match mode {
            3 | 5 => {
                let r = rider(m, i);
                if cvt_s_w(m.r32(r + 0x418) as i32).ee() <= 0.0 {
                    continue;
                }
                let s = m.r32(r + 0x5910);
                call(m, env, WSTR_FORMAT, &[p + 0x16, FMT_PLACE_TIME, s]);
                if (m.r32(RACE_TABLE) as i32) < 2 {
                    let v = call(m, env, TIME_MEDAL, &[s, m.r32(RACE_TABLE + 0x31c)]);
                    let ch = match v {
                        1 => 0x32u8,
                        0 => 0x31,
                        2 => 0x33,
                        _ => 0,
                    };
                    m.w8(p + 0x14, ch);
                    if ch != 0 {
                        m.w32(p, 1);
                    }
                    m.w32(p + 0x10, medal_sprite(v));
                }
            }
            9 => {
                let r = rider(m, i);
                m.w32(p, 1);
                let sc = call(m, env, RIDER_SCORE, &[r]);
                call(m, env, FORMAT_SCORE, &[sc, p + 0x16]);
            }
            2 | 4 => {
                if mode == 2 || call(m, env, CAREER_STATE, &[CAREER]) == 2 {
                    m.w32(p, 1);
                }
                let r = rider(m, i);
                let place = m.r32(r + 0x110);
                let v = call(m, env, PLACE_MEDAL, &[place]);
                m.w8(p + 0x14, place.wrapping_add(0x31) as u8);
                m.w32(p + 0x10, medal_sprite(v));
                let sc = call(m, env, RIDER_SCORE, &[r]);
                call(m, env, FORMAT_SCORE, &[sc, p + 0x16]);
            }
            7 => {
                m.w32(p, 0);
                m.w32(p + 0xc, 1);
            }
            _ => {}
        }
    }
}

/// `cPopFinishOverlay::Draw` 0x1aeb88(this): unless hidden or under the pause menu, the viewed player's alpha up by
/// 0.05 (to 1); once shown, its medal sprite at 72 (211 or, on the small screen... 217) by 1.3
pub fn pop_finish_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if call(m, env, OVERLAY_HIDDEN, &[m.r32(race(m) + 0x38)]) != 0 {
        return;
    }
    let o = call(m, env, OVERLAY_GET, &[m.r32(race(m) + 0x38)]);
    if m.r32(m.r32(o + 0x42c) + 0xc) != 0 {
        return;
    }
    let p = pop(this, m.r32(race(m) + 0x290));
    if (m.r32(p + 4) as i32) > 0 {
        return;
    }
    let a = rf(m, p + 0x38) + k(0x3d4c_cccd);
    wf(m, p + 0x38, a);
    if ONE.ee() < a.ee() {
        wf(m, p + 0x38, ONE);
    }
    rcall_f(m, env, 0x1c8, &[0, 2], &[F(0.0)]);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1d0, &[0xd]);
    if m.r32(p) == 0 {
        return;
    }
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let v = rf(m, DRAW_COLOUR) * rf(m, p + 0x38);
    wf(m, DRAW_COLOUR, v);
    let s = m.r32(p + 0x10);
    if s == 0xffff_ffff {
        return;
    }
    let y = if m.r32(DISPLAY_MODE) == 3 { F(211.0) } else { F(217.0) };
    let sc = k(0x3fa6_6666);
    fcall(m, env, DRAW_SPRITE, &[this, s, 0], &[F(72.0), y, sc, sc]);
}
fn rcall_f(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32], f: &[F]) {
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let me = r.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, f);
}
fn gold() -> [F; 4] {
    [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]
}
fn faded(m: &mut Mem, env: &mut dyn TextBoxEnv, c: [F; 4], alpha_at: u32, scratch: u32) {
    put4(m, scratch, c);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let v = rf(m, DRAW_COLOUR) * rf(m, alpha_at);
    wf(m, DRAW_COLOUR, v);
}

/// `cPopFinishOverlay::DrawText` 0x1aed50(this): once the viewed player's wait is over: in the free ride's mode 7
/// the time trial's text (0xe43, the time and its difference from the event's target, green under, red over); else
/// 0xe43 / 0xe5b (finished or not) when not shown, else the medal's line ("%c%s %s" of the place, 0xe44 / 0xe54 /
/// 0xe58 / 0xe45, 0xe4f); then the score's text
pub fn pop_finish_draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let i = m.r32(race(m) + 0x290);
    let p = pop(this, i);
    if (m.r32(p + 4) as i32) > 0 {
        return;
    }
    let (col, t1, t2, t3) = (scratch, scratch + 0x10, scratch + 0x40, scratch + 0x70);
    let font = |m: &Mem| m.r32(this + 8);
    if m.r32(RACE_TABLE + 0x320) == 7 && m.r32(p + 0xc) != 0 {
        faded(m, env, gold(), p + 0x38, col);
        if rf(m, p + 0x38).ee() != 0.0 {
            rcall(m, env, 0x1d0, &[7]);
        }
        let s = loc(m, env, 0xe43);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[font(m), s], &[k(0x4006_6666), F(320.0), F(158.0)]);
        let t = call(m, env, RIDER_TIME, &[rider(m, i)]);
        m.w32(FINISH_TIMES + 4 * i, t);
        call(m, env, FORMAT_TIME, &[t, t1]);
        faded(m, env, [ONE; 4], p + 0x38, col);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[font(m), t1], &[k(0x3f86_6666), k(0x4398_8000), F(220.0)]);
        call(m, env, CAREER_BEST, &[CAREER, 2, m.r32(RACE_TABLE + 0x31c)]);
        let d = m.r32(FINISH_TIMES + 4 * i).wrapping_sub(m.r32(RACE_TABLE + 0x32c));
        call(m, env, FORMAT_TIME, &[d, t2]);
        let text = if (d as i32) >= 0 {
            put4(m, col, [ONE, ONE, F(0.0), F(0.0)]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
            call(m, env, WSTR_FORMAT, &[t3, FMT_PLUS, t2]);
            t3
        } else {
            put4(m, col, [ONE, F(0.0), ONE, F(0.0)]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
            t2
        };
        let v = rf(m, DRAW_COLOUR) * rf(m, p + 0x38);
        wf(m, DRAW_COLOUR, v);
        fcall(m, env, PRINTF_SHADOWED_A, &[font(m), text], &[k(0x3f66_6667), k(0x43a7_8000), F(223.0)]);
        return;
    }
    if m.r32(p) == 0 {
        faded(m, env, gold(), p + 0x38, col);
        let r = rider(m, m.r32(race(m) + 0x290));
        let id = if (m.r32(r + 0x418) as i32) > 0 { 0xe43 } else { 0xe5b };
        let s = loc(m, env, id);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[font(m), s], &[k(0x4006_6666), F(320.0), F(225.0)]);
    } else if m.r8(p + 0x14) != 0 {
        faded(m, env, gold(), p + 0x38, scratch + 0x30);
        let id = match m.r8(p + 0x14) as i8 {
            0x32 => 0xe54,
            0x31 => 0xe44,
            0x33 => 0xe58,
            _ => 0xe45,
        };
        let ch = m.r8(p + 0x14) as i8 as i32 as u32;
        let a = loc(m, env, id);
        let b = loc(m, env, 0xe4f);
        call(m, env, WSTR_FORMAT, &[scratch, FMT_MEDAL, ch, a, b]);
        let sc = if m.r32(CAREER + 0x16c0) == 3 { k(0x3fe1_47ae) } else { F(2.0) };
        fcall(m, env, PRINTF_SHADOWED_A, &[font(m), scratch], &[sc, F(175.0), F(225.0)]);
    }
    if m.r16(p + 0x16) != 0 {
        faded(m, env, gold(), p + 0x38, col);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[font(m), p + 0x16], &[ONE, F(320.0), F(185.0)]);
    }
}

/// `cBlackFadeOverlay::Draw` 0x1af320(this): while the viewed rider fades (+0x44c above 0) and its race lets it (the
/// race's +0x40 between 1 and 9 asks 0x193ce0): the screen filled in black by the fade, untextured, mode 3, its
/// blend off then on
pub fn black_fade_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, _this: u32) {
    let r = rider(m, m.r32(race(m) + 0x290));
    if !(0.0 < rf(m, r + 0x44c).ee()) {
        return;
    }
    let o = m.r32(race(m) + 0x40);
    if o != 0 {
        let v = m.r32(o + 0x20);
        let asks = v != 0 && (v as i32) < 0xa;
        if asks && call(m, env, FADE_SHOWS, &[o, r]) == 0 {
            return;
        }
    }
    rcall(m, env, 0x1d0, &[0xb]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    rcall(m, env, 0x1e0, &[3]);
    wf(m, DRAW_COLOUR, rf(m, r + 0x44c));
    wf(m, DRAW_COLOUR + 0xc, ONE);
    wf(m, DRAW_COLOUR + 4, ONE);
    wf(m, DRAW_COLOUR + 8, ONE);
    rcall(m, env, 0x1c0, &[0]);
    fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[F(0.0), F(0.0), F(640.0), F(480.0)]);
    rcall(m, env, 0x1c0, &[1]);
}

// --- G8n3a: the results table's draw and the high scores overlay ---

/// The results table: its title (a wide string), its columns' alignments (0 left, 1 centred, 2 right) and x
/// (from 0x338598: column c spans x[c]..x[c+1]), the rows' and columns' highlights (−1 none), the cells
/// (wide strings, 0x80 bytes; column c's at 0x3385c0 + 0x800 c, its row r's 0x80 r on) and the columns' heads
pub const TABLE_TITLE: u32 = 0x33c9c0;
pub const TABLE_ALIGN: u32 = 0x3384d0;
pub const TABLE_X: u32 = 0x338598;
pub const ROW_MARK: u32 = 0x338510;
pub const COL_MARK: u32 = 0x3384f0;
pub const TABLE_HEADS: u32 = 0x33c5c0;
pub const TABLE_CELLS: u32 = 0x3385c0;

pub const BACKDROP_RESET: u32 = 0x1aa628;
pub const SET_CELL_INT: u32 = 0x1bfa08;
pub const SET_CELL_ASCII: u32 = 0x1bfa50;
pub const SET_ROW_NAME: u32 = 0x1bfab0;
pub const SET_TITLE: u32 = 0x1bfb10;
pub const RESULTS_UPDATE: u32 = 0x1ab148;
pub const NEW_BUTTONS: u32 = 0x1a8688;
pub const SELECT_SOUND: u32 = 0x1a8668;
pub const HUD_SHOW: u32 = 0x1a7848;
pub const DRAW_MENU_LIST: u32 = 0x1aa630;
pub const CHARACTERS: u32 = 0x331ea8;

/// `cViewHiScoresOverlay::DrawText` 0x1ab1a0 (the results' table, which every results overlay draws): on texture 7
/// the title in white by the backdrop's alpha (0.8 and, in the app's mode 2, at 76 for a two-player table (+0x48));
/// the heads in gold, placed by their columns' alignment (0.6 and 2 lower for the third in a long table (+0x50));
/// then each row's cells (0.8 in a career two-race event, else 0.85), coloured by the row's or column's highlight
/// (0 gold, 1 yellow, 2 white), placed by the alignment; then the overlay's own menu (+0x34 items from +0x10, +0x38
/// picked)
pub fn draw_table(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let col = scratch + 0x10;
    let b = |m: &Mem| m.r32(this);
    let r = call(m, env, GET_RECT, &[m.r32(b(m)), 0x37]);
    let top = (F(480.0) - rf(m, b(m) + 0x2c)) * F(0.5) + rf(m, r + 8);
    rcall(m, env, 0x1d0, &[7]);
    faded(m, env, [ONE; 4], b(m) + 0x70, col);
    let two = m.r32(this + 0x48) != 0;
    let sc = if two { k(0x3f4c_cccd) } else { ONE };
    let x = if m.r32(m.r32(APP) + 0x58) == 2 && two { F(76.0) } else { F(116.0) };
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), TABLE_TITLE], &[sc, x, top + F(7.0) + F(23.0)]);
    let xs = |m: &Mem, c: u32| rf(m, TABLE_X + 4 * c);
    // STANDIN: a column of another alignment keeps the last x (at the first, the caller's $f21)
    let mut cx = F(0.0);
    let mut c = 0u32;
    while (c as i32) < m.r32(this + 8) as i32 {
        let head = TABLE_HEADS + (c << 7);
        let align = m.r32(TABLE_ALIGN + 4 * c);
        let small = k(0x3f33_3333);
        if align == 2 || (c == 0 && m.r32(this + 0x4c) != 0) {
            let w = fcall(m, env, MEASURE_WIDE, &[m.r32(b(m) + 8), head, 0], &[small]);
            cx = xs(m, c + 1) - cvt_s_w(w as i32);
        } else if align == 0 {
            cx = xs(m, c);
        } else if align == 1 {
            let w = fcall(m, env, MEASURE_WIDE, &[m.r32(b(m) + 8), head, 0], &[small]);
            cx = (xs(m, c + 1) + xs(m, c) - cvt_s_w(w as i32)) * F(0.5);
        }
        faded(m, env, gold(), b(m) + 0x70, col);
        let (sc, dy) = if c == 2 && m.r32(this + 0x50) != 0 { (k(0x3f19_999a), F(2.0)) } else { (small, F(0.0)) };
        let y = rf(m, TITLE_X) + F(-8.0) + dy;
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), head], &[sc, cx, y]);
        c += 1;
    }
    let sc = if m.r32(CAREER + 0x2afc) == 1 && m.r32(CAREER + 0x27dc) == 2 { k(0x3f4c_cccd) } else { k(0x3f59_999a) };
    let mut row = 0u32;
    while (row as i32) < m.r32(this + 4) as i32 {
        let mut c = 0u32;
        while (c as i32) < m.r32(this + 8) as i32 {
            faded(m, env, [ONE; 4], b(m) + 0x70, col);
            let rm = m.r32(ROW_MARK + 4 * row);
            let cm = m.r32(COL_MARK + 4 * c);
            if !(rm == 0xffff_ffff && cm == 0xffff_ffff) {
                let v = if (cm as i32) < rm as i32 { rm } else { cm };
                match v {
                    1 => {
                        wf(m, DRAW_COLOUR + 0xc, F(0.0));
                        wf(m, DRAW_COLOUR, ONE);
                        wf(m, DRAW_COLOUR + 4, ONE);
                        wf(m, DRAW_COLOUR + 8, F(0.5));
                        let v = ONE * rf(m, b(m) + 0x70);
                        wf(m, DRAW_COLOUR, v);
                    }
                    0 => faded(m, env, gold(), b(m) + 0x70, col),
                    // (2 sets a flag that picks between three identical prints)
                    2 => faded(m, env, [ONE; 4], b(m) + 0x70, col),
                    _ => {}
                }
            }
            let cell = TABLE_CELLS + (row << 7) + (c << 11);
            let align = m.r32(TABLE_ALIGN + 4 * c);
            let font = m.r32(b(m) + 4);
            let cx = match align {
                1 => {
                    let w = fcall(m, env, MEASURE_WIDE, &[font, cell, 0], &[sc]);
                    (xs(m, c + 1) + xs(m, c) - cvt_s_w(w as i32)) * F(0.5)
                }
                2 => {
                    let w = fcall(m, env, MEASURE_WIDE, &[font, cell, 0], &[sc]);
                    xs(m, c + 1) - cvt_s_w(w as i32)
                }
                _ => xs(m, c),
            };
            fcall(m, env, PRINTF_SHADOWED_A, &[font, cell], &[sc, cx, rf(m, TITLE_ROWS + 4 * row)]);
            c += 1;
        }
        row += 1;
    }
    let n = m.r32(this + 0x34) as i32;
    if n > 0 {
        let (labels, on) = (scratch + 0x20, scratch + 0x50);
        for i in 0..n as u32 {
            m.w32(labels + 4 * i, m.r32(this + 0x10 + 8 * i));
            m.w32(on + 4 * i, 1);
        }
        let b0 = b(m);
        let y = (rf(m, b0 + 0x2c) + F(480.0)) * F(0.5) - F(40.0);
        let (cnt, sel) = (m.r32(this + 0x34), m.r32(this + 0x38));
        fcall(m, env, DRAW_MENU_LIST, &[b0, 3, 0, labels, cnt, sel, 0x74, 1, 1, on], &[F(600.0), y, k(0x3f3a_e148)]);
    }
}

/// `cViewHiScoresOverlay::Reset` 0x1b3a00(this): the backdrop reset, the table cleared and laid out (four columns:
/// rank, name, rider, the score right-aligned; no highlights), headed (unless in the free ride's mode 7), and
/// filled from the course's best times or scores (the career's +0x2af8; the scores in modes 3 and 5): the title
/// formatted with the count, a row each (its rank, name, rider, and the score as a score or a number)
pub fn hiscores_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this)]);
    clear_buffers(m, env);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    let x = rf(m, r + 8) + F(10.0) + F(7.0) + F(23.0) + F(39.0);
    title_rows(m, this, x, F(3.0));
    m.w32(TABLE_ALIGN + 0xc, 2);
    for (i, v) in [131.0f32, 214.0, 342.0, 415.0, 505.0].into_iter().enumerate() {
        wf(m, TABLE_X + 4 * i as u32, F(v));
    }
    for o in [0u32, 4, 8] {
        m.w32(TABLE_ALIGN + o, 0);
    }
    for i in (0..16).rev() {
        m.w32(ROW_MARK + 4 * i, 0xffff_ffff);
    }
    for i in (0..8).rev() {
        m.w32(COL_MARK + 4 * i, 0xffff_ffff);
    }
    if m.r32(RACE_TABLE + 0x320) != 7 {
        for (c, id) in [0xe5fu32, 0xe5d, 0x1cd, 0xe61].into_iter().enumerate() {
            let s = loc(m, env, id);
            call(m, env, SET_ROW_NAME, &[this, c as u32, s]);
        }
    }
    m.w32(this + 8, 4);
    let mode = m.r32(RACE_TABLE + 0x320);
    // in the other modes the game reads its table from address 0
    let best = match mode {
        3 | 5 | 2 | 4 | 9 => {
            let (kind, fmt) = if mode == 3 || mode == 5 { (1, m.r32(RESULT_WORDS + 4)) } else { (0, m.r32(RESULT_WORDS)) };
            let best = call(m, env, CAREER_BEST, &[CAREER, kind, m.r32(CAREER + 0x2af8)]);
            call(m, env, crate::pausedraw::WSTR_FORMAT, &[TABLE_TITLE, fmt, m.r32(best + 0x64)]);
            best
        }
        _ => 0,
    };
    m.w32(this + 4, m.r32(best + 0x64));
    let id = if m.r32(best + 0x6c) != 0 { 0xe61 } else { 0xe5e };
    let s = loc(m, env, id);
    call(m, env, SET_ROW_NAME, &[this, 3, s]);
    for i in (0..8).rev() {
        m.w32(COL_MARK + 4 * i, 0xffff_ffff);
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(best + 0x64) as i32 {
        let e = best + 0x14 * i;
        m.w32(ROW_MARK + 4 * i, 0xffff_ffff);
        call(m, env, SET_CELL_INT, &[this, 0, i, i + 1]);
        call(m, env, SET_CELL_ASCII, &[this, 1, i, e]);
        let name = m.r32(m.r32(e + 0xc).wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
        call(m, env, SET_CELL_ASCII, &[this, 2, i, name]);
        if m.r32(best + 0x6c) != 0 {
            call(m, env, FORMAT_SCORE, &[m.r32(e + 0x10), TABLE_CELLS + (i << 7) + 0x1800]);
        } else {
            call(m, env, SET_CELL_INT, &[this, 3, i, m.r32(e + 0x10)]);
        }
        i += 1;
    }
}

/// `cViewHiScoresOverlay::Draw` 0x1b3e30(this): the course's title (460 high) and the help line 0x1d7
pub fn hiscores_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    draw_course_title(m, env, this, F(460.0), scratch);
    let s = loc(m, env, 0x1d7);
    let (labels, on) = (scratch + 0x10, scratch + 0x14);
    m.w32(labels, s);
    m.w32(on, 1);
    fcall(m, env, DRAW_MENU_LIST, &[m.r32(this), 3, 0, labels, 1, 0xffff_ffff, 0x73, 1, 1, on], &[F(600.0), F(430.0), k(0x3f3a_e148)]);
}

/// `cViewHiScoresOverlay::Update` 0x1b3ec8(this): the results overlay's update; the select button (0x20) closes it
/// (its sound, overlay item 11 hidden, item 1 shown)
pub fn hiscores_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, RESULTS_UPDATE, &[this]);
    let b = call(m, env, NEW_BUTTONS, &[m.r32(this), 0xffff_ffff]);
    if b == 0 || b & 0x20 == 0 {
        return;
    }
    call(m, env, SELECT_SOUND, &[m.r32(this)]);
    let r = race(m);
    call(m, env, OVERLAY_HIDE, &[m.r32(r + 0x38), 0xb, 0xffff_ffff]);
    let r = race(m);
    call(m, env, HUD_SHOW, &[m.r32(r + 0x38), 1, 0xffff_ffff]);
}

// --- G8n3b: the next race overlay and the results' builders ---

/// The standings' titles (a word per career state) and help lines (0x390838), the hi-score page's three sprites
/// (0x390848), its targets' format (0x38e200) and each course's three target strings
pub const STANDINGS_TITLES: u32 = 0x338290;
pub const STANDINGS_HELP: u32 = 0x390838;
pub const TARGET_SPRITES: u32 = 0x390848;
pub const FMT_TARGET: u32 = 0x38e200;
pub const PROFILE_PLAYERS: u32 = 0x164288;
pub const PLAYER_ID: u32 = 0x166580;
pub const FIND_RIDER: u32 = 0x166598;
pub const PLAYER_NAME: u32 = 0x164918;
pub const PLAYER_PROFILE: u32 = 0x164330;
pub const PRINTF_SHADOWED: u32 = 0x19d3f0;
pub const ATTITUDE_LABEL: u32 = 0x1a8fa0;
pub const SPEECH_NEXT_RACE: u32 = 0x231ff8;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
pub const SPRINTF: u32 = 0x2fca28;
pub const TB_INIT: u32 = 0x1957b0;
pub const TB_SET_FONT: u32 = 0x196028;
pub const TB_RESET: u32 = 0x195ee8;
pub const TB_SET_SCALE: u32 = 0x1961f0;
pub const TB_SET_RECT: u32 = 0x196070;
pub const TB_APPEND: u32 = 0x196088;
pub const TB_WRAP: u32 = 0x196218;
pub const TB_DRAW: u32 = 0x1957d8;
pub const TB_DELETE: u32 = 0x14f390;

/// `cNextRaceOverlay::Reset` 0x1b4120(this): the backdrop reset, the buffers cleared, no menu (+0x34, +0x38, +0xc),
/// the help 0x18d (+0x10), no highlights, no targets (+0x1d8); the page built by the career's mode (2, 4, 9 the
/// standings, 3, 5 the hi-score page, 7 the time challenge's); then the speech's next race (0x343050's word)
pub fn next_race_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this)]);
    clear_buffers(m, env);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
    let s = loc(m, env, 0x18d);
    m.w32(this + 0x10, s);
    m.w32(this + 0xc, 0);
    for i in (0..16).rev() {
        m.w32(ROW_MARK + 4 * i, 0xffff_ffff);
    }
    for i in (0..8).rev() {
        m.w32(COL_MARK + 4 * i, 0xffff_ffff);
    }
    m.w32(this + 0x1d8, 0);
    match m.r32(CAREER + 0x2afc) {
        2 | 4 | 9 => build_standings(m, env, this),
        3 | 5 => build_hiscore_page(m, env, this, scratch),
        7 => build_time_challenge(m, env, this),
        _ => {}
    }
    call(m, env, SPEECH_NEXT_RACE, &[m.r32(MUSIC)]);
}

/// The next race overlay's vf16 0x1b4240: the sound system's channel 0 stopped
pub fn next_race_vf16(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, SND_STOP_ONE, &[m.r32(MUSIC), 0]);
}

/// `Results_BuildStandingsHeader` 0x1b4250(this): the rows laid out (one player: 137, 23 apart; two columns at 131,
/// 259, 434; else 142, 29 apart; three at 116, 244, 419, 594), no note (+0x54), the title and help (the race's
/// 0x207 / 0x1c5 in event modes 2 and 9, else by the career's state), a row for each computer rider (its name; the
/// rows the riders less the players)
pub fn build_standings(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if call(m, env, PROFILE_PLAYERS, &[CAREER]) == 1 {
        title_rows(m, this, F(137.0), F(23.0));
        wf(m, TABLE_X, F(131.0));
        m.w32(TABLE_ALIGN, 0);
        wf(m, TABLE_X + 8, F(434.0));
        wf(m, TABLE_X + 4, F(259.0));
        m.w32(TABLE_ALIGN + 4, 0);
        m.w32(this + 8, 2);
    } else {
        title_rows(m, this, F(142.0), F(29.0));
        wf(m, TABLE_X, F(116.0));
        m.w32(TABLE_ALIGN, 0);
        wf(m, TABLE_X + 0xc, F(594.0));
        wf(m, TABLE_X + 4, F(244.0));
        wf(m, TABLE_X + 8, F(419.0));
        m.w32(TABLE_ALIGN + 8, 0);
        m.w32(this + 8, 3);
        m.w32(TABLE_ALIGN + 4, 0);
    }
    m.w16(this + 0x54, 0);
    let mode = m.r32(RACE_TABLE + 0x320);
    let help = if mode == 2 || mode == 9 {
        let s = loc(m, env, 0x207);
        call(m, env, SET_TITLE, &[this, s]);
        0x1c5
    } else {
        let st = call(m, env, CAREER_STATE, &[CAREER]);
        let t = m.r32(STANDINGS_TITLES + (st << 2));
        call(m, env, SET_TITLE, &[this, t]);
        m.r32(STANDINGS_HELP.wrapping_add(st << 2))
    };
    let s = loc(m, env, help);
    m.w32(this + 0x114, s);
    let players = call(m, env, PROFILE_PLAYERS, &[CAREER]);
    m.w32(this + 4, m.r32(RACE_TABLE).wrapping_sub(players));
    let mut row = 0u32;
    let mut i = 0u32;
    while (i as i32) < m.r32(RACE_TABLE) as i32 {
        let e = RACE_TABLE + 8 + 0x84 * i;
        let who = m.r8(RACE_TABLE + 0x69 + 0x84 * i) as i8 as i32 as u32;
        if who == 0xffff_ffff {
            call(m, env, SET_CELL_ASCII, &[this, 0, row, e]);
            m.w32(ROW_MARK + 4 * row, who);
            row += 1;
        }
        i += 1;
    }
}

/// Each course's three target strings (the hi-score page's; the courses not listed leave the frame's words)
fn course_targets(course: u32) -> Option<[u32; 3]> {
    Some(match course {
        0 => [0x38e4c0, 0x38e4c8, 0x38e4d0],
        1 => [0x38e4d8, 0x38e4e0, 0x38e4e8],
        2 | 3 => [0x38e4f0, 0x38e4f8, 0x38e500],
        4 => [0x38e508, 0x38e510, 0x38e518],
        5 => [0x38e510, 0x38e530, 0x38e500],
        8 => [0x38e520, 0x38e4f0, 0x38e528],
        11 => [0x38e538, 0x38e540, 0x38e4f8],
        _ => return None,
    })
}

/// `Results_BuildHiScorePage` 0x1b44c0(this): one column (131..410) and no rows; the note 0xe3f with the course's
/// best score (+0x54); the targets shown (+0x1d8, at 50 (+0x1dc)); in event mode 3 the title 0xe67 and help 0x1c6
/// in a one-rider event, else 0x1c7 with the targets hidden at 148; in the others 0xe6e and 0x1d4. Then the three
/// targets (+0x118, +0x158, +0x198) by 0x185: on course 6 of mode 3 the numbers 800000, 500000, 250000, else the
/// course's strings
pub fn build_hiscore_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    wf(m, TABLE_X, F(131.0));
    m.w32(this + 8, 1);
    m.w32(this + 4, 0);
    wf(m, TABLE_X + 4, F(410.0));
    let best = call(m, env, CAREER_BEST, &[CAREER, 1, m.r32(RACE_TABLE + 0x31c)]);
    let s = loc(m, env, 0xe3f);
    call(m, env, crate::pausedraw::WSTR_FORMAT, &[this + 0x54, s, m.r32(best + 0x10)]);
    wf(m, this + 0x1dc, F(50.0));
    m.w32(this + 0x1d8, 1);
    if m.r32(RACE_TABLE + 0x320) == 3 {
        let s = loc(m, env, 0xe67);
        call(m, env, SET_TITLE, &[this, s]);
        if m.r32(CAREER + 0x27dc) == 1 {
            let s = loc(m, env, 0x1c6);
            m.w32(this + 0x114, s);
        } else {
            let s = loc(m, env, 0x1c7);
            m.w32(this + 0x114, s);
            wf(m, this + 0x1dc, F(148.0));
            m.w32(this + 0x1d8, 0);
        }
    } else {
        let s = loc(m, env, 0xe6e);
        call(m, env, SET_TITLE, &[this, s]);
        let s = loc(m, env, 0x1d4);
        m.w32(this + 0x114, s);
    }
    let course = m.r32(RACE_TABLE + 0x31c);
    let fmt = loc(m, env, 0x185);
    if m.r32(RACE_TABLE + 0x320) == 3 && m.r32(RACE_TABLE + 0x31c) == 6 {
        let buf = scratch + 0x10;
        for (i, n) in [800_000u32, 500_000, 250_000].into_iter().enumerate() {
            call(m, env, SPRINTF, &[buf, FMT_TARGET, n]);
            call(m, env, crate::pausedraw::WSTR_FORMAT, &[this + 0x118 + 0x40 * i as u32, fmt, buf]);
        }
        return;
    }
    if let Some(t) = course_targets(course) {
        for (i, s) in t.into_iter().enumerate() {
            m.w32(scratch + 4 * i as u32, s);
        }
    }
    for i in 0..3 {
        let s = m.r32(scratch + 4 * i);
        call(m, env, crate::pausedraw::WSTR_FORMAT, &[this + 0x118 + 0x40 * i, fmt, s]);
    }
}

/// `Results_BuildTimeChallengeHeader` 0x1b4890(this): the course's best (kind 2): its score (+0x1e0), name (+0x208)
/// and rider's name (+0x230); the title 0xe96 and help 0xc75
pub fn build_time_challenge(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let best = call(m, env, CAREER_BEST, &[CAREER, 2, m.r32(CAREER + 0x2af8)]);
    call(m, env, FORMAT_SCORE, &[m.r32(best + 0x10), this + 0x1e0]);
    call(m, env, WSTR_SET_ASCII, &[this + 0x208, best]);
    let name = m.r32(m.r32(best + 0xc).wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    call(m, env, WSTR_SET_ASCII, &[this + 0x230, name]);
    let s = loc(m, env, 0xe96);
    call(m, env, SET_TITLE, &[this, s]);
    let s = loc(m, env, 0xc75);
    m.w32(this + 0x114, s);
}

/// The two players' riders' attitude labels on a standings row (the second only with two players)
fn attitude_labels(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, base: u32, y: F) {
    let r = call(m, env, FIND_RIDER, &[CAREER, 0]);
    let a = m.r8(RACE_TABLE.wrapping_add(base).wrapping_add(r << 2) + 0x24) as i8 as i32;
    let r = call(m, env, FIND_RIDER, &[CAREER, 0]);
    let b = m.r8(RACE_TABLE.wrapping_add(base).wrapping_add(r << 2) + 0x26) as u32;
    fcall(m, env, ATTITUDE_LABEL, &[m.r32(this), b], &[rf(m, TABLE_X + 4), y + F(3.0), cvt_s_w(a), cvt_s_w(b as i32)]);
    if call(m, env, PROFILE_PLAYERS, &[CAREER]) != 2 {
        return;
    }
    let r = call(m, env, FIND_RIDER, &[CAREER, 1]);
    let a = m.r8(RACE_TABLE.wrapping_add(base).wrapping_add(r << 2) + 0x24) as i8 as i32;
    let r = call(m, env, FIND_RIDER, &[CAREER, 1]);
    let b = m.r8(RACE_TABLE.wrapping_add(base).wrapping_add(r << 2) + 0x26) as u32;
    // (its a1 is left at the 1 the rider's lookup took)
    fcall(m, env, ATTITUDE_LABEL, &[m.r32(this), 1], &[rf(m, TABLE_X + 8), y + F(3.0), cvt_s_w(a), cvt_s_w(b as i32)]);
}

/// `cNextRaceOverlay::Draw` 0x1b4950(this): with the backdrop's +0x50 at 0.8 meanwhile: the course's title (410
/// high in mode 7, else 460) and in modes 3 and 5 the targets' three medals (sprite 0x2e at +0x1dc: 150 gold, 215
/// pale, 280 yellow); in modes 2 and 4 each computer rider's row its attitudes to the players; the three target
/// sprites (0x390848) at 426 from 189, 40 apart, when the targets show
pub fn next_race_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let kept = rf(m, m.r32(this) + 0x50);
    wf(m, m.r32(this) + 0x50, k(0x3f4c_cccd));
    let mode = m.r32(CAREER + 0x2afc);
    if mode == 5 || mode == 3 {
        draw_course_title(m, env, this, F(460.0), scratch);
        let two = F(2.0);
        for (c, y) in [
            (gold(), F(150.0)),
            ([ONE, ONE, k(0x3ef1_26e9), k(0x3d1f_be77)], F(215.0)),
            ([ONE, ONE, F(0.0), F(0.0)], F(280.0)),
        ] {
            faded(m, env, c, m.r32(this) + 0x70, scratch);
            fcall(m, env, DRAW_SPRITE, &[m.r32(this), 0x2e, 0], &[rf(m, this + 0x1dc), y, two, two]);
        }
    } else if mode == 7 {
        draw_course_title(m, env, this, F(410.0), scratch);
    } else {
        draw_course_title(m, env, this, F(460.0), scratch);
    }
    if mode == 4 || mode == 2 {
        let mut row = 0u32;
        let mut i = 0u32;
        while (i as i32) < m.r32(RACE_TABLE) as i32 {
            let base = 0x84 * i;
            if m.r8(RACE_TABLE + base + 0x69) as i8 == -1 {
                let y = rf(m, TITLE_ROWS + 4 * row);
                attitude_labels(m, env, this, base, y);
                row += 1;
            }
            i += 1;
        }
    }
    if m.r32(this + 0x1d8) != 0 {
        faded(m, env, [ONE; 4], m.r32(this) + 0x70, scratch);
        let mut y = F(189.0);
        let s = k(0x3f19_999a);
        for i in 0..3 {
            let sprite = m.r32(TARGET_SPRITES + 4 * i);
            fcall(m, env, DRAW_SPRITE, &[m.r32(this), sprite, 0], &[F(426.0), y, s, s]);
            y = y + F(40.0);
        }
    }
    wf(m, m.r32(this) + 0x50, kept);
}

/// A colour set with the backdrop's alpha in place of its first (not times it)
fn with_alpha(m: &mut Mem, env: &mut dyn TextBoxEnv, c: [F; 4], this: u32, scratch: u32) {
    put4(m, scratch, c);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let a = rf(m, m.r32(this) + 0x70);
    wf(m, DRAW_COLOUR, a);
}

/// A player's line of the standings' head: 0xeb7 with the player's rider's name, at `x`, 23 above the first row
fn standings_name(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, player: u32, buf: u32, x: F) {
    let s = loc(m, env, 0xeb7);
    let r = call(m, env, FIND_RIDER, &[CAREER, player]);
    let e = RACE_TABLE.wrapping_add(r.wrapping_mul(0x84)) + 8;
    call(m, env, crate::pausedraw::WSTR_FORMAT, &[buf, s, e]);
    let y = rf(m, TITLE_ROWS) - F(23.0);
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(m.r32(this) + 8), buf], &[k(0x3ef0_a3d7), x, y]);
}

/// `cNextRaceOverlay::DrawText` 0x1b4d98(this): in event mode 7 the time challenge's head (the players' names and
/// riders at 117 / 330, the headings 0xea2..0xea4 and the best's score, name and rider); modes 4 and 5 mark the
/// table two-player (+0x48); in modes 2 and 4 the standings' heads (0xe60, 0xe5c and the players' riders); the
/// table; the note (+0x54, not in mode 7); the help (+0x114) in a box (346 in modes 3 / 5, 320 in 7, else 352);
/// in modes 3 and 5 the targets' labels 0x204..0x206; the targets (+0x118..) when shown
pub fn next_race_draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    let top = (F(480.0) - rf(m, m.r32(this) + 0x2c)) * F(0.5) + rf(m, r + 8);
    let mode = m.r32(RACE_TABLE + 0x320);
    let b = |m: &Mem| m.r32(this);
    if mode == 7 {
        with_alpha(m, env, [ONE; 4], this, scratch);
        let (y0, y1) = (top + F(90.0), top + F(120.0));
        let (s8, s85) = (k(0x3f4c_cccd), k(0x3f59_999a));
        let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
        let name = call(m, env, PLAYER_NAME, &[CAREER, p]);
        fcall(m, env, PRINTF_SHADOWED, &[m.r32(b(m) + 8), name], &[s8, F(117.0), y0]);
        let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
        let c = call(m, env, PLAYER_PROFILE, &[CAREER, p]);
        let rider = m.r32(c.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
        fcall(m, env, PRINTF_SHADOWED, &[m.r32(b(m) + 8), rider], &[s85, F(117.0), y1]);
        if call(m, env, PROFILE_PLAYERS, &[CAREER]) == 2 {
            let p = call(m, env, PLAYER_ID, &[CAREER, 1]);
            let name = call(m, env, PLAYER_NAME, &[CAREER, p]);
            fcall(m, env, PRINTF_SHADOWED, &[m.r32(b(m) + 8), name], &[s8, F(330.0), y0]);
            let p = call(m, env, PLAYER_ID, &[CAREER, 1]);
            let c = call(m, env, PLAYER_PROFILE, &[CAREER, p]);
            let rider = m.r32(c.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
            fcall(m, env, PRINTF_SHADOWED, &[m.r32(b(m) + 8), rider], &[s85, F(330.0), y1]);
        }
        with_alpha(m, env, gold(), this, scratch);
        let y = top + F(170.0);
        let s75 = k(0x3f40_0000);
        let x = if m.r32(m.r32(APP) + 0x58) == 2 { F(47.0) } else { F(117.0) };
        let s = loc(m, env, 0xea2);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s75, x, y]);
        let s = loc(m, env, 0xea3);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s75, F(345.0), y]);
        let s = loc(m, env, 0xea4);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s75, F(480.0), y]);
        with_alpha(m, env, [ONE; 4], this, scratch);
        let y = top + F(200.0);
        let s92 = k(0x3f6b_851f);
        for (o, x) in [(0x1e0u32, F(117.0)), (0x208, F(345.0)), (0x230, F(480.0))] {
            fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 4), this + o], &[s92, x, y]);
        }
    }
    if mode.wrapping_sub(4) < 2 {
        m.w32(this + 0x48, 1);
    }
    if mode == 4 || mode == 2 {
        faded(m, env, gold(), b(m) + 0x70, scratch);
        let y = rf(m, TITLE_ROWS) - F(48.0);
        let s76 = k(0x3f42_8f5c);
        let s = loc(m, env, 0xe60);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s76, rf(m, TABLE_X), y]);
        if call(m, env, PROFILE_PLAYERS, &[CAREER]) == 1 {
            let s = loc(m, env, 0xe5c);
            let y = rf(m, TITLE_ROWS) - F(48.0);
            fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s76, rf(m, TABLE_X + 4), y]);
            let x = rf(m, TABLE_X + 4);
            standings_name(m, env, this, 0, scratch + 0x10, x);
        } else {
            let x = rf(m, TABLE_X + 4) + F(65.0);
            let y = rf(m, TITLE_ROWS) - F(48.0);
            let s = loc(m, env, 0xe5c);
            fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), s], &[s76, x, y]);
            let x = rf(m, TABLE_X + 4);
            standings_name(m, env, this, 0, scratch, x);
            let x = rf(m, TABLE_X + 8);
            standings_name(m, env, this, 1, scratch, x);
        }
    }
    draw_table(m, env, this, scratch + 0x1600);
    if m.r16(this + 0x54) != 0 && mode != 7 {
        faded(m, env, gold(), b(m) + 0x70, scratch);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 8), this + 0x54], &[k(0x3f33_3333), F(116.0), F(118.0)]);
    }
    let tb = scratch + 0x30;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, m.r32(b(m) + 4)]);
    call(m, env, TB_RESET, &[tb]);
    let s88 = k(0x3f61_47ae);
    fcall(m, env, TB_SET_SCALE, &[tb], &[s88, s88]);
    // the box's colour: gold, its first the backdrop's alpha (set on the frame and copied, not through the drawer)
    put4(m, scratch, gold());
    for i in 0..4 {
        m.w32(tb + 0x3c + 4 * i, m.r32(scratch + 4 * i));
    }
    m.w32(tb + 0x38, m.r32(b(m) + 0x70));
    let y = match mode {
        5 | 3 => F(346.0),
        7 => F(320.0),
        _ => F(352.0),
    };
    fcall(m, env, TB_SET_RECT, &[tb], &[F(116.0), y, F(380.0), F(100.0)]);
    call(m, env, TB_APPEND, &[tb, m.r32(this + 0x114)]);
    call(m, env, TB_WRAP, &[tb, 0]);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    if mode == 5 || mode == 3 {
        let a = loc(m, env, 0x204);
        let bb = loc(m, env, 0x205);
        let c = loc(m, env, 0x206);
        faded(m, env, [ONE; 4], b(m) + 0x70, scratch);
        let x = rf(m, this + 0x1dc) + F(60.0);
        let s = if m.r32(m.r32(APP) + 0x58) == 2 { k(0x3f53_f7cf) } else { k(0x3f6b_851f) };
        for (t, y) in [(a, F(170.0)), (bb, F(235.0)), (c, F(300.0))] {
            fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 4), t], &[s, x, y]);
        }
    }
    if m.r32(this + 0x1d8) != 0 {
        faded(m, env, [ONE; 4], b(m) + 0x70, scratch);
        let mut y = F(194.0);
        for i in 0..3 {
            fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b(m) + 4), this + 0x118 + (i << 6)], &[k(0x3f6b_851f), F(486.0), y]);
            y = y + F(40.0);
        }
    }
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cNextRaceOverlay::Update` 0x1b5878(this): the results' update; the menu shown (+0x34 1) once the race's +0x1c
/// object's +0x4f8 is clear
pub fn next_race_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, RESULTS_UPDATE, &[this]);
    let r = race(m);
    if m.r32(m.r32(r + 0x1c) + 0x4f8) == 0 {
        m.w32(this + 0x34, 1);
    }
}

// --- G8n4a: the unlocked overlay ---

/// The unlocks' formats by kind: the plural (a count) and the single
pub const AWARD_PLURAL: u32 = 0x338240;
pub const AWARD_SINGLE: u32 = 0x338268;
/// The race's unlocks: a count and the list (0xc each: the player, the kind, the value)
pub const UNLOCKS: u32 = 0x3353f8;
pub const UNLOCK_LIST: u32 = 0x335400;
pub const UNLOCKED_TITLE: u32 = 0x38e220;
pub const FMT_PLAYER_LINE: u32 = 0x38e548;
pub const SET_TITLE_ASCII: u32 = 0x1bfb20;
pub const FIND_SLOT: u32 = 0x166550;
pub const RACE_FINAL_UNLOCK: u32 = 0x1618c0;
pub const SHOWOFF_FINAL_UNLOCK: u32 = 0x1618e0;
pub const CAREER_BOOK: u32 = 0x163f18;
pub const PAGES_COMPLETE: u32 = 0x164060;
pub const ANIM_RESET: u32 = 0x1f1258;
pub const ANIM_ADD_BAR: u32 = 0x1f11c0;
pub const ANIM_VALUE: u32 = 0x1f1448;
pub const WSTR_COUNT: u32 = 0x17cc48;
pub const SPEECH_UNLOCKS: u32 = 0x232330;
pub const TS_INIT: u32 = 0x19dec0;
pub const TS_SET_FONT: u32 = 0x19e380;
pub const TS_DESTRUCT: u32 = 0x19ded0;
pub const DRAW_ALIGNED: u32 = 0x19df90;

/// `Results_AddAwardLine` 0x1b5bf8(this, kind, player): the player's unlocks of a kind as the table's next row (+0x1a4)
/// with its sprite (+0x1f8 by row): a count of 2 or more by the plural format; one by the single, with the kind's
/// own words (0, 1 a course's name; 2 the course 6 or 7 only, sprite 0x58 or none; 3 a rider; 5 the player's
/// complete trick book pages); none, nothing. The sprites are the kind's (+0x1d0)
pub fn add_award_line(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, kind: u32, player: u32) {
    let rec = player.wrapping_mul(0x28);
    let at = this + 0x54 + (kind << 2) + rec;
    let n = m.r32(at);
    let cell = |m: &Mem| TABLE_CELLS + (m.r32(this + 0x1a4) << 7);
    let wf_ = crate::pausedraw::WSTR_FORMAT;
    let mark = if (n as i32) >= 2 {
        call(m, env, wf_, &[cell(m), m.r32(AWARD_PLURAL + (kind << 2)), n, player]);
        m.r32(this + (kind << 2) + 0x1d0)
    } else if (n as i32) <= 0 {
        return;
    } else if kind == 3 {
        let c = m.r32(this + rec + 0xc + 0xf4);
        let name = m.r32(c.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
        call(m, env, wf_, &[cell(m), m.r32(AWARD_SINGLE + 0xc), m.r32(at), name]);
        m.r32(this + 0x1dc)
    } else if kind == 2 {
        let course = m.r32(this + rec + 8 + 0xf4);
        if course != 6 && course != 7 {
            return;
        }
        let c = cell(m);
        let t = m.r32(m.r32(APP) + 0x50);
        let id = call(m, env, COURSE_NAME, &[CAREER, course]);
        let s = call(m, env, LOC, &[t, id]);
        call(m, env, wf_, &[c, m.r32(AWARD_SINGLE + 8), m.r32(at), s]);
        if course == 6 { 0x58 } else { 0xffff_ffff }
    } else if kind < 2 {
        let c = cell(m);
        let t = m.r32(m.r32(APP) + 0x50);
        let id = call(m, env, COURSE_NAME, &[CAREER, m.r32(this + (kind << 2) + rec + 0xf4)]);
        let s = call(m, env, LOC, &[t, id]);
        call(m, env, wf_, &[c, m.r32(AWARD_SINGLE + (kind << 2)), m.r32(at), s]);
        m.r32(this + (kind << 2) + 0x1d0)
    } else if kind == 5 {
        let p = call(m, env, PLAYER_ID, &[CAREER, player]);
        let book = call(m, env, CAREER_BOOK, &[CAREER, p]);
        let pages = call(m, env, PAGES_COMPLETE, &[book]);
        call(m, env, wf_, &[cell(m), m.r32(AWARD_SINGLE + 0x14), m.r32(this + 0x54 + rec + 0x14), pages]);
        m.r32(this + 0x1e4)
    } else {
        call(m, env, wf_, &[cell(m), m.r32(AWARD_SINGLE + (kind << 2)), n, player]);
        m.r32(this + (kind << 2) + 0x1d0)
    };
    let row = m.r32(this + 0x1a4);
    m.w32(this + (row << 2) + 0x1f8, mark);
    m.w32(this + 0x1a4, row.wrapping_add(1));
}

/// `cUnlockedOverlay::Reset` 0x1b5ef8(this): the buffers cleared, no menu, help 0x18d, every highlight 2 (white),
/// the title 0x38e220; the final unlock (the race's in the career's mode 4 (+0x1a8), else the show-off's (+0x1ac));
/// the players' unlocks gathered from the race's list (the count per kind, the bonus kind 9 as its value; the
/// last value; the player flagged); each flagged player's lines (kinds 5, 9, 8, 4, 3, 2, 1, 0, 6), name and, with
/// a kind 4 or 5, rider; the rows laid out (one column at 156..436, the first rows 13 apart, 10 with two players)
/// and each revealed in turn by the backdrop's animation (+0x94); the speech's unlock lines
pub fn unlocked_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let (names, riders, flags) = (this + 0x1b0, this + 0x1c0, this + 0x194);
    let (counts, values) = (this + 0x54, this + 0xf4);
    clear_buffers(m, env);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
    let s = loc(m, env, 0x18d);
    m.w32(this + 0x10, s);
    m.w32(this + 0xc, 0);
    for i in (0..16).rev() {
        m.w32(ROW_MARK + 4 * i, 2);
    }
    for i in (0..8).rev() {
        m.w32(COL_MARK + 4 * i, 2);
    }
    call(m, env, SET_TITLE_ASCII, &[this, UNLOCKED_TITLE]);
    m.w32(this + 0x1a8, 0);
    m.w32(this + 0x1ac, 0);
    if m.r32(CAREER + 0x2afc) == 4 {
        let v = call(m, env, RACE_FINAL_UNLOCK, &[CAREER]);
        m.w32(this + 0x1a8, v);
    } else {
        let v = call(m, env, SHOWOFF_FINAL_UNLOCK, &[CAREER]);
        m.w32(this + 0x1ac, v);
    }
    let n = m.r32(UNLOCKS);
    for p in 0..4 {
        m.w32(names + 4 * p, 0);
        m.w32(riders + 4 * p, 0);
        m.w32(flags + 4 * p, 0);
        for j in 0..10 {
            m.w32(counts + 0x28 * p + 4 * j, 0);
            m.w32(values + 0x28 * p + 4 * j, 0);
        }
    }
    for i in (0..28).rev() {
        m.w32(this + 0x1f8 + 4 * i, 0xffff_ffff);
    }
    if (n as i32) > 0 {
        let mut e = UNLOCK_LIST;
        let mut left = n;
        loop {
            let slot = call(m, env, FIND_SLOT, &[CAREER, m.r32(e)]);
            let p = if (slot as i32) > -1 { slot } else { 0 };
            let kind = m.r32(e + 4);
            let rec = p.wrapping_mul(0x28);
            if kind == 9 {
                m.w32(counts.wrapping_add(rec) + 0x24, m.r32(e + 8));
            } else {
                let at = counts.wrapping_add((kind << 2).wrapping_add(rec));
                m.w32(at, m.r32(at).wrapping_add(1));
            }
            let kind = m.r32(e + 4);
            m.w32(flags.wrapping_add(p << 2), 1);
            m.w32(values.wrapping_add((kind << 2).wrapping_add(rec)), m.r32(e + 8));
            e += 0xc;
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    m.w32(this + 4, 0);
    m.w32(this + 8, 1);
    m.w32(this + 0x1a4, 0);
    for p in 0..4u32 {
        if m.r32(flags + 4 * p) == 0 {
            continue;
        }
        for kind in [5u32, 9, 8, 4, 3, 2, 1, 0, 6] {
            add_award_line(m, env, this, kind, p);
        }
        let id = call(m, env, PLAYER_ID, &[CAREER, p]);
        let name = call(m, env, PLAYER_NAME, &[CAREER, id]);
        m.w32(names + 4 * p, name);
        let rec = this + 0x28 * p;
        if (m.r32(rec + 0x68) as i32) > 0 || (m.r32(rec + 0x64) as i32) > 0 {
            let id = call(m, env, PLAYER_ID, &[CAREER, p]);
            let c = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
            m.w32(riders + 4 * p, m.r32(c.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4));
        }
        let row = m.r32(this + 0x1a4);
        if (m.r32(this + 4) as i32) < row as i32 {
            m.w32(this + 4, row);
        }
        m.w32(this + 0x1a4, 7);
    }
    wf(m, TABLE_X, F(156.0));
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    let top = rf(m, r + 8) + F(10.0);
    let gap = if call(m, env, PROFILE_PLAYERS, &[CAREER]) == 2 { F(10.0) } else { F(13.0) };
    title_rows(m, this, top + F(7.0) + F(23.0) + F(51.0), gap);
    let x = rf(m, TABLE_X) + F(280.0);
    m.w32(TABLE_ALIGN, 0);
    wf(m, TABLE_X + 4, x);
    call(m, env, ANIM_RESET, &[m.r32(this) + 0x94]);
    if (m.r32(this + 4) as i32) > 0 {
        let mut i = 1u32;
        loop {
            let cell = TABLE_CELLS + (i << 7) - 0x80;
            let pad = if call(m, env, WSTR_COUNT, &[cell]) == 0 { 0 } else { 0x50 };
            let anim = m.r32(this) + 0x94;
            let len = call(m, env, WSTR_COUNT, &[cell]);
            call(m, env, ANIM_ADD_BAR, &[anim, i, i, 1, 0, 0, len.wrapping_add(pad)]);
            i += 1;
            if (m.r32(this + 4) as i32) < i as i32 {
                break;
            }
        }
    }
    call(m, env, SPEECH_UNLOCKS, &[m.r32(MUSIC)]);
}

/// The unlocked overlay's vf16 0x1b6458: the sound system's channel 0 stopped
pub fn unlocked_vf16(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, SND_STOP_ONE, &[m.r32(MUSIC), 0]);
}

/// The unlocked row's sprite (white, at 44, 160) when it has one
fn row_sprite(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, at: u32, scratch: u32) -> bool {
    if (m.r32(at) as i32) <= 0 {
        return false;
    }
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this), m.r32(at), 0], &[F(44.0), F(160.0), ONE, ONE]);
    true
}

/// The unlocked overlay's `Draw` 0x1b6468(this): the course's title (460 high); with one player the sprite of the
/// row the animation is revealing (+0x94 the row + 1), or, when that has none, the last row's
pub fn unlocked_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    draw_course_title(m, env, this, F(460.0), scratch);
    if call(m, env, PROFILE_PLAYERS, &[CAREER]) == 2 {
        return;
    }
    let mut none = true;
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 4) as i32 {
        if m.r32(m.r32(this) + 0x94) == i + 1 && row_sprite(m, env, this, this + 0x1f8 + 4 * i, scratch) {
            none = false;
        }
        i += 1;
    }
    if m.r32(m.r32(this) + 0x94) == 0 || !none {
        return;
    }
    row_sprite(m, env, this, (this + 0x1f8).wrapping_add(i.wrapping_sub(1) << 2), scratch);
}

/// `cUnlockedOverlay::DrawText` 0x1b65f0(this): in gold each player's name line ("%s %s" of 0xdf6 and the name,
/// 0.656) and 0xdf7 28 under it, 148 apart; the rows (0.85, by the backdrop's animation's count of each row);
/// the final unlock's note (0xc9c the race's, else 0xc9d the show-off's) in a white box at 65, 330; the menu
pub fn unlocked_draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    let mut y = rf(m, r + 8) + F(10.0) + F(7.0) + F(23.0) + F(5.0);
    let (st, buf, line) = (scratch + 0x10, scratch + 0x20, scratch + 0x40);
    faded(m, env, gold(), m.r32(this) + 0x70, st);
    for i in 0..4 {
        let name = m.r32(this + 0x1b0 + 4 * i);
        if name == 0 {
            continue;
        }
        let y2 = y + F(28.0);
        call(m, env, WSTR_SET_ASCII, &[buf, name]);
        let s = loc(m, env, 0xdf6);
        call(m, env, WSTR_FORMAT, &[line, FMT_PLAYER_LINE, s, buf]);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(m.r32(this) + 8), line], &[k(0x3f27_ef9e), F(116.0), y]);
        y = y + F(148.0);
        let s = loc(m, env, 0xdf7);
        fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(m.r32(this) + 8), s], &[k(0x3f4c_cccd), F(116.0), y2]);
    }
    call(m, env, TS_INIT, &[st]);
    call(m, env, TS_SET_FONT, &[st, m.r32(m.r32(this) + 4)]);
    m.w32(st + 4, 1);
    put4(m, buf, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, buf]);
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 4) as i32 {
        let cell = TABLE_CELLS + (i << 7);
        let n = call(m, env, ANIM_VALUE, &[m.r32(this) + 0x94, i + 1]);
        let y = rf(m, TITLE_ROWS + 4 * i);
        fcall(m, env, DRAW_ALIGNED, &[st, 0, cell, n, 1, 1], &[k(0x3f59_999a), rf(m, TABLE_X), y]);
        i += 1;
    }
    if m.r32(this + 0x1a8) != 0 || m.r32(this + 0x1ac) != 0 {
        let tb = scratch + 0x1d0;
        call(m, env, TB_INIT, &[tb]);
        call(m, env, TB_SET_FONT, &[tb, m.r32(m.r32(this) + 4)]);
        call(m, env, TB_RESET, &[tb]);
        put4(m, buf, [ONE; 4]);
        for i in 0..4 {
            m.w32(tb + 0x3c + 4 * i, m.r32(buf + 4 * i));
        }
        let s85 = k(0x3f59_999a);
        fcall(m, env, TB_SET_SCALE, &[tb], &[s85, s85]);
        fcall(m, env, TB_SET_RECT, &[tb], &[F(65.0), F(330.0), F(450.0), F(100.0)]);
        let id = if m.r32(this + 0x1a8) != 0 { 0xc9c } else { 0xc9d };
        let s = loc(m, env, id);
        call(m, env, TB_APPEND, &[tb, s]);
        call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
        call(m, env, TB_DELETE, &[tb, 2]);
    }
    let n = m.r32(this + 0x34) as i32;
    if n > 0 {
        let (labels, on) = (buf, scratch + 0x50);
        for i in 0..n as u32 {
            m.w32(labels + 4 * i, m.r32(this + 0x10 + 8 * i));
            m.w32(on + 4 * i, 1);
        }
        let b0 = m.r32(this);
        let y = (rf(m, b0 + 0x2c) + F(480.0)) * F(0.5) - F(40.0);
        let (cnt, sel) = (m.r32(this + 0x34), m.r32(this + 0x38));
        fcall(m, env, DRAW_MENU_LIST, &[b0, 3, 0, labels, cnt, sel, 0x74, 1, 1, on], &[F(600.0), y, k(0x3f3a_e148)]);
    }
    call(m, env, TS_DESTRUCT, &[st, 2]);
}

/// `cUnlockedOverlay::Update` 0x1b6ac0(this): the results' update; the menu shown once the race's +0x1c object's
/// +0x4f8 is clear; the button 0x800 plays the select sound and, unless the picked item's word (+0xc + 8 the pick)
/// is set, hides overlay item 13 and shows item 1
pub fn unlocked_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, RESULTS_UPDATE, &[this]);
    let r = race(m);
    if m.r32(m.r32(r + 0x1c) + 0x4f8) == 0 {
        m.w32(this + 0x34, 1);
    }
    let b = call(m, env, NEW_BUTTONS, &[m.r32(this), 0xffff_ffff]);
    if b == 0 || b & 0x800 == 0 {
        return;
    }
    call(m, env, SELECT_SOUND, &[m.r32(this)]);
    if m.r32(this.wrapping_add(m.r32(this + 0x38) << 3) + 0xc) != 0 {
        return;
    }
    let r = race(m);
    call(m, env, OVERLAY_HIDE, &[m.r32(r + 0x38), 0xd, 0xffff_ffff]);
    let r = race(m);
    call(m, env, HUD_SHOW, &[m.r32(r + 0x38), 1, 0xffff_ffff]);
}

// --- G8n4b: the high-score record overlay ---

/// The name entry's grid: three rows of 14 characters ('d' delete, 'e' end, 0 none), copied from 0x38e550; its
/// x (the first column, the step) and y (the first row, the step)
pub const NAME_GRID: u32 = 0x3382a0;
pub const NAME_GRID_SRC: u32 = 0x38e550;
pub const GRID_X0: u32 = 0x3ce788;
pub const GRID_DX: u32 = 0x338218;
pub const GRID_DY: u32 = 0x33821c;
pub const GRID_Y0: u32 = 0x338224;
/// The high-score record's backdrop table and the race's new records (0x10 each: the player's pad or −1, _, the
/// best table, the new entry)
pub const RECORD_BACKDROP_VTABLE: u32 = 0x38f4f0;
pub const NEW_RECORDS: u32 = 0x32eca0;
pub const EMPTY: u32 = 0x38e220;
pub const STRNCPY: u32 = 0x2fd630;
pub const STRLEN: u32 = 0x2fd260;
pub const STRCPY: u32 = 0x2fd14c;
pub const LIBC_MEMSET: u32 = 0x2fb468;
pub const MEASURE_ASCII: u32 = 0x19da30;
pub const DRAW_STRING_A: u32 = 0x19d0c0;
pub const DRAW_STRING_RAW: u32 = 0x19cf70;
pub const PRINTF_SHADOWED_CENTERED: u32 = 0x19d6e0;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const MOVE_SOUND: u32 = 0x1a8648;
pub const BACK_SOUND: u32 = 0x1a8658;
pub const SET_PLAYER_NAME: u32 = 0x164968;
pub const SET_PLAYER_PAD: u32 = 0x1a7878;
pub const SPEECH_HISCORE: u32 = 0x232240;
pub const SET_ROW_NAME_ASCII: u32 = 0x1bfac8;

/// `HiScoreRecordOverlay_Init` 0x1b6b90(this, whole): as the results overlay's (its own backdrop at +0x118 when
/// `whole`), with the record's backdrop table (copied to the frame, its six shifts moved by the overlay's place,
/// when not); the name grid cleared and copied in (0x2a bytes; the game also copies the grid to its frame unused);
/// +0x44 set
pub fn hiscore_record_init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, scratch: u32) -> u32 {
    if whole != 0 {
        m.w32(this, this + 0x118);
        call(m, env, BACKDROP_INIT, &[this + 0x118]);
    }
    results_init(m, env, this, 0, scratch + 0x200);
    let b = m.r32(this);
    m.w32(b + 0x28, RECORD_BACKDROP_VTABLE);
    if whole == 0 {
        for i in 0..0x98 / 4 {
            m.w32(scratch + 4 * i, m.r32(RECORD_BACKDROP_VTABLE + 4 * i));
        }
        let d = this.wrapping_sub(b.wrapping_sub(0x118));
        for o in [8u32, 0x68, 0x70, 0x78, 0x80, 0x88] {
            let h = (m.r16(RECORD_BACKDROP_VTABLE + o) as u32).wrapping_add(d);
            m.w16(scratch + o, h as u16);
        }
        m.w32(b + 0x28, scratch);
    }
    for i in 0..0x2b {
        m.w8(scratch + 0xa0 + i, m.r8(NAME_GRID_SRC + i));
    }
    call(m, env, MEMSET, &[NAME_GRID, 0, 0x2a]);
    call(m, env, STRNCPY, &[NAME_GRID, NAME_GRID_SRC, 0x2a]);
    m.w32(this + 0x44, 1);
    this
}

/// `cHiScoreRecordOverlay::Reset` 0x1b6d60(this): the backdrop reset, the buffers cleared, the high scores' table
/// layout (rows from the frame's top + 79, columns at 131, 214, 342, 415, 505, the score right-aligned; no
/// highlights; the name +0x64 cleared (0xb bytes); the entering flag +0x114 set); the title by the mode (the
/// scores' 0x3384c4 in 3 and 5, the times' 0x3384c0 in 2, 4 and 9, 0xea1 in 7 with the best kind 2; others no
/// table, read from address 0); the rows the table's count; the heads (not in mode 7); then the first record
/// submitted (+0x110 0)
pub fn hiscore_record_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this)]);
    clear_buffers(m, env);
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    let x = rf(m, r + 8) + F(10.0) + F(7.0) + F(23.0) + F(39.0);
    title_rows(m, this, x, F(3.0));
    m.w32(TABLE_ALIGN + 0xc, 2);
    for (i, v) in [131.0f32, 214.0, 342.0, 415.0, 505.0].into_iter().enumerate() {
        wf(m, TABLE_X + 4 * i as u32, F(v));
    }
    m.w32(TABLE_ALIGN, 0);
    m.w32(this + 8, 4);
    m.w32(this + 0x114, 1);
    m.w32(TABLE_ALIGN + 4, 0);
    m.w32(TABLE_ALIGN + 8, 0);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
    for i in (0..16).rev() {
        m.w32(ROW_MARK + 4 * i, 0xffff_ffff);
    }
    for i in (0..8).rev() {
        m.w32(COL_MARK + 4 * i, 0xffff_ffff);
    }
    call(m, env, MEMSET, &[this + 0x64, 0, 0xb]);
    let course = m.r32(CAREER + 0x2af8);
    let best = match m.r32(CAREER + 0x2afc) {
        3 | 5 => {
            let best = call(m, env, CAREER_BEST, &[CAREER, 1, course]);
            call(m, env, crate::pausedraw::WSTR_FORMAT, &[TABLE_TITLE, m.r32(RESULT_WORDS + 4), m.r32(best + 0x64)]);
            best
        }
        2 | 4 | 9 => {
            let best = call(m, env, CAREER_BEST, &[CAREER, 0, course]);
            call(m, env, crate::pausedraw::WSTR_FORMAT, &[TABLE_TITLE, m.r32(RESULT_WORDS), m.r32(best + 0x64)]);
            best
        }
        7 => {
            let best = call(m, env, CAREER_BEST, &[CAREER, 2, course]);
            let s = loc(m, env, 0xea1);
            call(m, env, WSTR_COPY, &[TABLE_TITLE, s]);
            best
        }
        // (no table: the game reads it from address 0)
        _ => 0,
    };
    m.w32(this + 4, m.r32(best + 0x64));
    if m.r32(CAREER + 0x2afc) != 7 {
        for (c, id) in [0xe5fu32, 0xe5d, 0x1cd, 0xe5e].into_iter().enumerate() {
            let s = loc(m, env, id);
            call(m, env, SET_ROW_NAME, &[this, c as u32, s]);
        }
    }
    m.w32(this + 0x110, 0);
    hiscore_submit_entry(m, env, this);
}

/// The high-score record's vf16 0x1b7068: the sound system's channel 0 stopped
pub fn hiscore_record_vf16(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, SND_STOP_ONE, &[m.r32(MUSIC), 0]);
}

/// The high-score record's `Draw` 0x1b7078(this): the course's title (410 high in mode 7, else 460); the
/// renderer's +0x1e0 (3) and texture 7; the grid's cursor (sprite 0x36 frame 0x20, white at 0.5 of the alpha while
/// entering (+0x3c), else 0.2) at the picked character (+0x58 the row, +0x5c the column; its measure unused); in
/// mode 7 a faint bar (213, 287, 200 × 25), else the name's caret (sprite 0x32 at the second column − 15, the
/// entry's row + 2; its measure of the name and "." unused)
pub fn hiscore_record_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mode = m.r32(CAREER + 0x2afc);
    let h = if mode == 7 { F(410.0) } else { F(460.0) };
    draw_course_title(m, env, this, h, scratch + 0x40);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1d0, &[7]);
    let (row, col) = (m.r32(this + 0x58), m.r32(this + 0x5c));
    let ch = m.r8(NAME_GRID.wrapping_add(col).wrapping_add(row.wrapping_mul(0xe)));
    m.w8(scratch + 1, 0);
    m.w8(scratch, ch);
    fcall(m, env, MEASURE_ASCII, &[m.r32(m.r32(this) + 4), scratch, 0], &[ONE]);
    let col_ = scratch + 0x10;
    put4(m, col_, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col_]);
    let f = if m.r32(this + 0x3c) != 0 { F(0.5) } else { k(0x3e4c_cccd) };
    let a = rf(m, m.r32(this) + 0x70) * f;
    wf(m, DRAW_COLOUR, rf(m, DRAW_COLOUR) * a);
    let y = cvt_s_w(m.r32(this + 0x58) as i32) * rf(m, GRID_DY);
    let yy = rf(m, GRID_Y0) + y;
    let x = cvt_s_w(m.r32(this + 0x5c) as i32) * rf(m, GRID_DX);
    let xx = rf(m, GRID_X0) + ONE + x;
    fcall(m, env, DRAW_SPRITE, &[m.r32(this), 0x36, 0x20], &[xx, yy - ONE, k(0x3f8f_5c29), k(0x3f8c_cccd)]);
    if mode == 7 {
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
        put4(m, col_, [ONE; 4]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col_]);
        wf(m, DRAW_COLOUR, rf(m, DRAW_COLOUR) * k(0x3e99_999a));
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[F(213.0), k(0x438f_8000), F(200.0), F(25.0)]);
        return;
    }
    let name = this + 0x64;
    let len = call(m, env, STRLEN, &[name]);
    let end = name.wrapping_add(len);
    m.w8(end, b'.');
    m.w8(len.wrapping_add(name) + 1, 0);
    fcall(m, env, MEASURE_ASCII, &[m.r32(m.r32(this) + 4), name, 0], &[k(0x3f59_999a)]);
    m.w8(end, 0);
    put4(m, col_, [ONE, ONE, ONE, F(0.0)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col_]);
    let a = if m.r32(this + 0x3c) != 0 {
        rf(m, m.r32(this) + 0x70)
    } else {
        rf(m, m.r32(this) + 0x70) * k(0x3ecc_cccd)
    };
    wf(m, DRAW_COLOUR, rf(m, DRAW_COLOUR) * a);
    let x = rf(m, TABLE_X + 4) - F(15.0);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this), 0x32, 0], &[x, rf(m, this + 0x54) + F(2.0), k(0x3f99_999a), k(0x3f99_999a)]);
}

/// `cHiScoreRecordOverlay::DrawText` 0x1b73e0(this): the table; the grid's characters (white by the alpha;
/// 'e' the word 0x112 centred small, 'd' the delete sprite 0x32, others centred), a row at a time until a 0;
/// in mode 7 the time challenge's box (0xea0 in gold at 132, 135), its three labels (0xe9e, 0xe9d, 0xe9f)
/// right-aligned at 300, the best's score and the event's target in white and their difference in magenta at
/// 400, 0xe21 centred, and the name typed (when any) in gold; otherwise the name typed at the entry's row; the
/// help (0x113, or 0x18d on the end key) once the race's +0x1c's +0x4f8 is clear; the two prompts 0xe20 / 0xe21
/// (not in mode 7)
pub fn hiscore_record_draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mode = m.r32(CAREER + 0x2afc);
    draw_table(m, env, this, scratch + 0x1800);
    let ch = scratch + 0x10;
    let col = scratch + 0x20;
    call(m, env, LIBC_MEMSET, &[ch, 0, 2]);
    put4(m, col, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    wf(m, DRAW_COLOUR, rf(m, DRAW_COLOUR) * rf(m, m.r32(this) + 0x70));
    let mut y = rf(m, GRID_Y0);
    let small = k(0x3f0c_cccd);
    for row in 0..3u32 {
        let mut x = rf(m, GRID_X0);
        for c in 0..14u32 {
            let v = m.r8(NAME_GRID + row * 0xe + c);
            m.w8(ch, v);
            if v == 0 {
                break;
            }
            match v {
                b'e' => {
                    let s = loc(m, env, 0x112);
                    let w = fcall(m, env, MEASURE_WIDE, &[m.r32(m.r32(this) + 4), s, 0], &[small]);
                    let yy = y + F(4.0);
                    let xx = x - cvt_s_w(w as i32) * F(0.5);
                    let s = loc(m, env, 0x112);
                    fcall(m, env, DRAW_STRING_A, &[m.r32(m.r32(this) + 4), s], &[xx, yy, small]);
                }
                b'd' => {
                    let s = k(0x3f9d_70a4);
                    fcall(m, env, DRAW_SPRITE, &[m.r32(this), 0x32, 1], &[x - F(5.0), y + F(3.0), s, s]);
                }
                _ => {
                    let w = fcall(m, env, MEASURE_ASCII, &[m.r32(m.r32(this) + 4), ch, 0], &[ONE]);
                    let xx = x - cvt_s_w(w as i32) * F(0.5);
                    fcall(m, env, DRAW_STRING_RAW, &[m.r32(m.r32(this) + 4), ch], &[xx, y, ONE]);
                }
            }
            x = x + rf(m, GRID_DX);
        }
        y = y + rf(m, GRID_DY);
    }
    if mode == 7 {
        let best = call(m, env, CAREER_BEST, &[CAREER, 2, m.r32(RACE_TABLE + 0x31c)]);
        let tb = scratch + 0x30;
        call(m, env, TB_INIT, &[tb]);
        call(m, env, TB_SET_FONT, &[tb, m.r32(m.r32(this) + 8)]);
        call(m, env, TB_RESET, &[tb]);
        put4(m, col, gold());
        for i in 0..4 {
            m.w32(tb + 0x3c + 4 * i, m.r32(col + 4 * i));
        }
        m.w32(tb + 0x6c, 1);
        m.w32(tb + 0x20, 0);
        m.w32(tb + 0x24, 0);
        m.w32(tb + 0x20, 1);
        m.w32(tb + 0x38, m.r32(m.r32(this) + 0x70));
        let s7 = k(0x3f33_3333);
        fcall(m, env, TB_SET_SCALE, &[tb], &[s7, s7]);
        fcall(m, env, TB_SET_RECT, &[tb], &[F(132.0), F(135.0), F(370.0), F(100.0)]);
        let s = loc(m, env, 0xea0);
        let text = scratch + 0x14a0;
        call(m, env, crate::pausedraw::WSTR_FORMAT, &[text, s]);
        call(m, env, TB_APPEND, &[tb, text]);
        call(m, env, TB_WRAP, &[tb, 0]);
        call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
        let s65 = k(0x3f26_6666);
        with_alpha(m, env, [ONE, ONE, k(0x3ef1_26e9), k(0x3d1f_be77)], this, col);
        for (id, y) in [(0xe9eu32, F(190.0)), (0xe9d, F(210.0)), (0xe9f, F(230.0))] {
            let s = loc(m, env, id);
            fcall(m, env, PRINT_SHADOWED_RIGHT, &[m.r32(m.r32(this) + 8), s], &[s65, F(300.0), y]);
        }
        with_alpha(m, env, [ONE; 4], this, col);
        let s9 = k(0x3f66_6666);
        let (a, b2, c) = (scratch + 0x16a0, scratch + 0x16d0, scratch + 0x1700);
        call(m, env, FORMAT_SCORE, &[m.r32(best + 0x10), a]);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[m.r32(m.r32(this) + 4), a], &[s9, F(400.0), F(190.0)]);
        call(m, env, FORMAT_SCORE, &[m.r32(RACE_TABLE + 0x32c), b2]);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[m.r32(m.r32(this) + 4), b2], &[s9, F(400.0), F(210.0)]);
        call(m, env, FORMAT_TIME, &[m.r32(best + 0x10).wrapping_sub(m.r32(RACE_TABLE + 0x32c)), c]);
        with_alpha(m, env, [ONE, F(0.0), ONE, F(0.0)], this, col);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[m.r32(m.r32(this) + 4), c], &[s9, F(400.0), F(230.0)]);
        with_alpha(m, env, gold(), this, col);
        let s = loc(m, env, 0xe21);
        fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(m.r32(this) + 8), s], &[s7, F(320.0), F(260.0)]);
        if m.r8(this + 0x64) != 0 {
            rcall(m, env, 0x1d0, &[0x13]);
            faded(m, env, gold(), m.r32(this) + 0x70, col);
            fcall(m, env, PRINTF_SHADOWED_CENTERED, &[m.r32(m.r32(this) + 4), this + 0x64], &[ONE, F(320.0), F(290.0)]);
        }
        call(m, env, TB_DELETE, &[tb, 2]);
    } else if m.r8(this + 0x64) != 0 {
        faded(m, env, gold(), m.r32(this) + 0x70, col);
        fcall(m, env, PRINTF_SHADOWED, &[m.r32(m.r32(this) + 4), this + 0x64], &[k(0x3f59_999a), rf(m, TABLE_X + 4), rf(m, this + 0x54)]);
    }
    let r = race(m);
    if m.r32(m.r32(r + 0x1c) + 0x4f8) == 0 {
        faded(m, env, [ONE; 4], m.r32(this) + 0x70, col);
        let (label, on) = (scratch + 0x1730, scratch + 0x1734);
        let s = loc(m, env, 0x113);
        m.w32(label, s);
        if m.r32(this + 0x58) == 2 && m.r32(this + 0x5c) == 0xd {
            let s = loc(m, env, 0x18d);
            m.w32(label, s);
        }
        let s73 = k(0x3f3a_e148);
        m.w32(on, 1);
        fcall(m, env, MEASURE_WIDE, &[m.r32(m.r32(this) + 4), m.r32(label), 0], &[s73]);
        let y = if mode == 7 { k(0x43ca_8000) } else { F(430.0) };
        fcall(m, env, DRAW_MENU_LIST, &[m.r32(this), 3, 0, label, 1, 0xffff_ffff, 0x74, 1, 0, on], &[F(600.0), y, s73]);
    }
    faded(m, env, gold(), m.r32(this) + 0x70, col);
    if mode == 7 {
        return;
    }
    let s7 = k(0x3f33_3333);
    let s = loc(m, env, 0xe20);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(m.r32(this) + 8), s], &[s7, F(320.0), F(256.0)]);
    let s = loc(m, env, 0xe21);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(m.r32(this) + 8), s], &[s7, F(320.0), F(280.0)]);
}

fn grid_at(m: &Mem, row: u32, col: u32) -> i8 {
    m.r8(NAME_GRID.wrapping_add(col).wrapping_add(row.wrapping_mul(0xe))) as i8
}

/// `cHiScoreRecordOverlay::Update` 0x1b7d98(this): the results' update; the entering player's pad's new buttons:
/// up / down (0x20001, 0x40002) move the row (up wrapping 0 to 2, down by 3) and left / right (0x100008, 0x80004)
/// the column (left wrapping 0 to 13, right by 14), past empty cells (up and left on without wrapping), each with
/// the move sound; select (0x800) takes the picked character with the select sound: 'd' deletes the last
/// (the back sound; no longer entering), 'e' ends (entering; the next record submitted, or when there is none
/// overlay item 2 hidden and item 13 shown if any unlock isn't kind 7, else 1); any other is typed (a fresh name
/// when entering), the tenth on replacing the ninth
pub fn hiscore_record_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, RESULTS_UPDATE, &[this]);
    let r = race(m);
    let pad = m.r32(m.r32(r + 0x38) + 0xc);
    let b = call(m, env, NEW_BUTTONS, &[m.r32(this), pad]);
    if b == 0 {
        return;
    }
    if b & 0x20001 != 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this)]);
        let v = m.r32(this + 0x58).wrapping_sub(1);
        m.w32(this + 0x58, if (v as i32) < 0 { 2 } else { v });
        while grid_at(m, m.r32(this + 0x58), m.r32(this + 0x5c)) == 0 {
            let v = m.r32(this + 0x58).wrapping_sub(1);
            m.w32(this + 0x58, v);
        }
    }
    if b & 0x40002 != 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this)]);
        loop {
            let v = (m.r32(this + 0x58) as i32).wrapping_add(1).wrapping_rem(3) as u32;
            m.w32(this + 0x58, v);
            if grid_at(m, v, m.r32(this + 0x5c)) != 0 {
                break;
            }
        }
    }
    if b & 0x100008 != 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this)]);
        let v = m.r32(this + 0x5c).wrapping_sub(1);
        m.w32(this + 0x5c, if (v as i32) < 0 { 0xd } else { v });
        while grid_at(m, m.r32(this + 0x58), m.r32(this + 0x5c)) == 0 {
            let v = m.r32(this + 0x5c).wrapping_sub(1);
            m.w32(this + 0x5c, v);
        }
    }
    if b & 0x80004 != 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this)]);
        loop {
            let v = (m.r32(this + 0x5c) as i32).wrapping_add(1).wrapping_rem(0xe) as u32;
            m.w32(this + 0x5c, v);
            if grid_at(m, m.r32(this + 0x58), v) != 0 {
                break;
            }
        }
    }
    if b & 0x800 == 0 {
        return;
    }
    let c = grid_at(m, m.r32(this + 0x58), m.r32(this + 0x5c));
    call(m, env, SELECT_SOUND, &[m.r32(this)]);
    if c == b'd' as i8 {
        call(m, env, BACK_SOUND, &[m.r32(this)]);
        if m.r32(this + 0x114) != 0 {
            m.w32(this + 0x114, 0);
        }
        let n = m.r32(this + 0x60);
        if (n as i32) > 0 {
            let n = n - 1;
            m.w32(this + 0x60, n);
            m.w8(this.wrapping_add(n) + 0x64, 0);
        }
        return;
    }
    if c == b'e' as i8 {
        m.w32(this + 0x114, 1);
        if hiscore_submit_entry(m, env, this) != 0 {
            return;
        }
        let n = m.r32(UNLOCKS);
        let mut others = 0u32;
        if (n as i32) > 0 {
            let mut e = UNLOCK_LIST + 4;
            let mut left = n;
            loop {
                if m.r32(e) != 7 {
                    others += 1;
                }
                e += 0xc;
                left = left.wrapping_sub(1);
                if left == 0 {
                    break;
                }
            }
        }
        let r = race(m);
        call(m, env, OVERLAY_HIDE, &[m.r32(r + 0x38), 2, 0xffff_ffff]);
        let item = if others != 0 { 0xd } else { 1 };
        let r = race(m);
        call(m, env, HUD_SHOW, &[m.r32(r + 0x38), item, 0xffff_ffff]);
        return;
    }
    let name = this + 0x64;
    if m.r32(this + 0x114) != 0 {
        m.w32(this + 0x114, 0);
        call(m, env, MEMSET, &[name, 0, 0xb]);
        m.w32(this + 0x60, 0);
    }
    let n = m.r32(this + 0x60);
    if (n as i32) < 9 {
        m.w8(name.wrapping_add(n), c as u8);
        m.w32(this + 0x60, m.r32(this + 0x60).wrapping_add(1));
    } else {
        m.w8(n.wrapping_add(name).wrapping_sub(1), c as u8);
    }
}

/// `HiScore_SubmitEntry` 0x1b81b8(this) -> whether there was a record: the race's new record at +0x110 (0x10 each
/// from 0x32eca0); past the first, the name typed is set as the entering player's (0x164968) and copied over the
/// last record's entry. A record with a player (+0 not negative) gets the table rebuilt: the cells cleared, the
/// overlay given the player's pad, a row for each of the best table's entries (its rank; the new one highlighted
/// (2) with the entry's row kept (+0x54), its speech, and while entering the name copied in and the line 0x114;
/// others named), the rider, the score or the time, blanked in mode 7; the fourth head; the cursor on the end key
/// (row 2, column 13), the name's length, the next record
pub fn hiscore_submit_entry(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let n = m.r32(this + 0x110);
    let rec = NEW_RECORDS.wrapping_add(n << 4);
    let table = m.r32(rec + 8);
    let name = this + 0x64;
    if (n as i32) > 0 {
        let r = race(m);
        let pad = m.r32(m.r32(r + 0x38) + 0xc);
        let s = call(m, env, SET_PLAYER_NAME, &[CAREER, pad, name]);
        call(m, env, STRCPY, &[name, s]);
        let last = m.r32(NEW_RECORDS.wrapping_add(m.r32(this + 0x110) << 4).wrapping_sub(4));
        call(m, env, STRCPY, &[last, s]);
    }
    let mode = m.r32(CAREER + 0x2afc);
    if (m.r32(rec) as i32) < 0 {
        return 0;
    }
    call(m, env, MEMSET, &[TABLE_CELLS, 0, 0x4000]);
    let r = race(m);
    call(m, env, SET_PLAYER_PAD, &[m.r32(r + 0x38), m.r32(rec)]);
    let mut i = 0u32;
    while (i as i32) < m.r32(m.r32(rec + 8) + 0x64) as i32 {
        let e = table.wrapping_add(i.wrapping_mul(0x14));
        call(m, env, SET_CELL_INT, &[this, 0, i, i + 1]);
        if e == m.r32(rec + 0xc) {
            m.w32(ROW_MARK + 4 * i, 2);
            wf(m, this + 0x54, rf(m, TITLE_ROWS + 4 * i));
            call(m, env, SPEECH_HISCORE, &[m.r32(MUSIC), i]);
            if m.r32(this + 0x114) != 0 {
                call(m, env, STRCPY, &[name, e]);
                let s = loc(m, env, 0x114);
                call(m, env, crate::pausedraw::WSTR_FORMAT, &[this + 0x70, s, name]);
            }
        } else {
            m.w32(ROW_MARK + 4 * i, 0xffff_ffff);
            call(m, env, SET_CELL_ASCII, &[this, 1, i, e]);
        }
        let rider = m.r32(m.r32(e + 0xc).wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
        call(m, env, SET_CELL_ASCII, &[this, 2, i, rider]);
        if m.r32(m.r32(rec + 8) + 0x6c) != 0 {
            call(m, env, FORMAT_SCORE, &[m.r32(e + 0x10), TABLE_CELLS + 0x1800 + (i << 7)]);
        } else {
            call(m, env, SET_CELL_INT, &[this, 3, i, m.r32(e + 0x10)]);
        }
        if mode == 7 {
            for c in 0..4 {
                call(m, env, SET_CELL_ASCII, &[this, c, i, EMPTY]);
            }
        }
        i += 1;
    }
    let id = if m.r32(m.r32(rec + 8) + 0x6c) != 0 { 0xe61 } else { 0xe5e };
    let s = loc(m, env, id);
    call(m, env, SET_ROW_NAME, &[this, 3, s]);
    if mode == 7 {
        call(m, env, SET_ROW_NAME_ASCII, &[this, 3, EMPTY]);
    }
    m.w32(this + 0x58, 2);
    m.w32(this + 0x5c, 0xd);
    let len = call(m, env, STRLEN, &[name]);
    m.w32(this + 0x60, len);
    m.w32(this + 0x110, m.r32(this + 0x110).wrapping_add(1));
    1
}

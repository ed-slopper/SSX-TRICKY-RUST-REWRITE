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

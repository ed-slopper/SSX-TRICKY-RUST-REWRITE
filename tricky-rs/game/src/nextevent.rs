//! The next event overlay's card screens (board row G8n5a): `cSelectNextEventOverlay`'s draws, which are the
//! memory card UI's (`cMCOverlayManager` is its base; the overlay `o` as in [`crate::mcoverlay`]: +0x10 / +0x18 /
//! +0x1c the options' flags, +0x38 the slot picked, +0x48 the progress, +0x4c the message's alpha, +0x54 the state,
//! +0x168 + 0x58 i / +0x174 + 0x58 i a slot's two flags, +0x218 / +0x21c the entry count and pick, +0x224 the card
//! system, +0x428 the overlay's second vtable, +0x42c its backdrop, +0x434 its title's id). The index's names
//! (vf16, vf17, vf23, vf26, `DrawText`, `Reset`) are its slots'; what each draws is in its comment. Every callee is
//! a stand-in through [`TextBoxEnv`]; the card system's and the overlay's own virtuals are called through their
//! tables (GCC 2's: the `this` shift a half word at the slot, the function the word after).

use crate::animseq::cvt_s_w;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const DRAW_COLOUR: u32 = 0x337fe8;
pub const LOC: u32 = 0x2c5160;
pub const SET_COLOUR: u32 = 0x14eee8;
pub const FILLED_RECT: u32 = 0x19b108;
pub const DRAW_LINE: u32 = 0x19c1e8;
pub const MEASURE_WIDE: u32 = 0x19dc78;
pub const PRINTF_SHADOWED_A: u32 = 0x19d568;
pub const DRAW_STRING_A: u32 = 0x19d0c0;
pub const DRAW_SPRITE: u32 = 0x1c0068;
pub const BACKDROP_FRAME: u32 = 0x1a9bb8;
pub const LABEL_BOX: u32 = 0x1a9928;
pub const DRAW_BAR: u32 = 0x1a9f80;
pub const GET_RECT: u32 = 0x1f9e50;
pub const WSTR_FORMAT: u32 = 0x17ce38;
pub const WSTR_FORMAT_B: u32 = 0x17ce70;
pub const WSTR_COUNT: u32 = 0x17cc48;
pub const FMT_NUMBER: u32 = 0x38e200;
pub const TB_INIT: u32 = 0x1957b0;
pub const TB_SET_FONT: u32 = 0x196028;
pub const TB_SET_RECT: u32 = 0x196070;
pub const TB_APPEND: u32 = 0x196088;
pub const TB_SET_LINE_HEIGHT: u32 = 0x1961e8;
pub const TB_LINE_COUNT: u32 = 0x1964a0;
pub const TB_DRAW: u32 = 0x1957d8;
pub const TB_DELETE: u32 = 0x14f390;
pub const SLOT_LABEL: u32 = 0x1d2d50;
pub const SLOT_COUNT: u32 = 0x1d1eb8;
pub const MC_NEXT: u32 = 0x1d21c8;
pub const MC_PREV: u32 = 0x1d2200;
pub const MC_SLOT_NEXT: u32 = 0x1d2278;
pub const MC_SLOT_PREV: u32 = 0x1d2230;
pub const MOVE_SOUND: u32 = 0x1a8648;
pub const BACK_SOUND: u32 = 0x1a8658;
pub const SELECT_SOUND: u32 = 0x1a8668;

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
fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}
/// A virtual through the table at `obj` + `at` (the `this` shift a half word at the slot, the function after)
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj + at);
    let me = obj.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, &[])
}
/// A virtual of the renderer (the app's +0x724, its table at +0x10)
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, 0x10, off, args)
}
/// A virtual of the card system (`o` + 0x224, its table at +0)
fn card(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, off: u32, args: &[u32]) -> u32 {
    let c = m.r32(this + 0x224);
    vcall(m, env, c, 0, off, args)
}
/// The drawer's colour set straight (its four words, not through `Color_Set`)
fn colour(m: &mut Mem, c: [F; 4]) {
    put4(m, DRAW_COLOUR, c);
}
const ONE: F = F(1.0);

/// A label box's width: under 200 is 200, over 384 is 320, over 512 is 384
fn clamp_width(w: F) -> F {
    if w.ee() < F(200.0).ee() {
        F(200.0)
    } else if !(F(384.0).ee() < w.ee()) {
        w
    } else if F(512.0).ee() < w.ee() {
        F(384.0)
    } else {
        F(320.0)
    }
}

/// `cSelectNextEventOverlay` vf17 0x1b8718(this): the progress screen: the backdrop's frame (item 0xe, 290 high);
/// the bar's box (texture 15, 104..516 across, its rect 0x2a's height · 0.8 tall, centred on 240) with 14 dark
/// divider lines 27.47 apart from 131.47; the progress (+0x48, when above 0: a purple bar 14 dividers · the share
/// long); the bar's frame (texture 18); the line 0x1c3 (with the overlay's +0xd8) in a box sized to it (by the
/// widths' clamp) at 144; then the overlay's own virtual +0xd8
pub fn progress_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let half = F(0.5);
    let x0 = F(104.0);
    rcall(m, env, 0x1e0, &[3]);
    let step = k(0x41db_bbbc);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    let mut x = k(0x4303_7778);
    colour(m, [ONE; 4]);
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 0xe], &[F(290.0)]);
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this + 0x42c)), 0x2a]);
    let h = rf(m, r + 8) * k(0x3f4c_cccd);
    let top = F(240.0) - h * half;
    rcall(m, env, 0x1d0, &[0xf]);
    colour(m, [ONE; 4]);
    let bottom = top + h;
    fcall(m, env, DRAW_BAR, &[m.r32(this + 0x42c), 1], &[x0, top, F(412.0), h]);
    rcall(m, env, 0x1d0, &[0x10]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    colour(m, [ONE, F(0.0), F(0.0), F(0.0)]);
    for _ in 0..14 {
        fcall(m, env, DRAW_LINE, &[DRAW_COLOUR], &[x, top + half, x, bottom - ONE]);
        x = x + step;
    }
    rcall(m, env, 0x1d0, &[0x11]);
    let p = rf(m, this + 0x48);
    if F(0.0).ee() < p.ee() {
        let len = step * F(14.0) * p + step;
        colour(m, [k(0x3f4c_cccd), F(0.0), k(0x3f66_6666), F(0.0)]);
        fcall(m, env, DRAW_BAR, &[m.r32(this + 0x42c), 1], &[x0 - ONE, top, len, h]);
    }
    rcall(m, env, 0x1d0, &[0x12]);
    colour(m, [ONE; 4]);
    fcall(m, env, DRAW_BAR, &[m.r32(this + 0x42c), 2], &[x0, top - half, F(412.0), h]);
    let font = m.r32(m.r32(APP) + 0x60);
    let tb = scratch + 0x200;
    call(m, env, TB_INIT, &[tb]);
    let s = loc(m, env, 0x1c3);
    call(m, env, WSTR_FORMAT, &[scratch, s, this + 0xd8]);
    let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[font, scratch, 0], &[ONE]) as i32);
    let w = clamp_width(w) + F(10.0);
    colour(m, [ONE; 4]);
    call(m, env, TB_SET_FONT, &[tb, font]);
    let x = F(320.0) - w * half;
    fcall(m, env, TB_SET_RECT, &[tb], &[x, F(144.0), w, cvt_s_w(m.r32(font + 0xc) as i32)]);
    call(m, env, TB_APPEND, &[tb, scratch]);
    m.w32(tb + 0x20, 0);
    m.w32(tb + 0x24, 0);
    m.w32(tb + 0x20, 1);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    vcall(m, env, this, 0x428, 0xd8, &[]);
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cSelectNextEventOverlay` vf26 0x1b8bb8(this): the card system's message (0x15e when its +0x1a0 says so, else
/// 0xdeb) in a box at 128, 312 (384 × 96), on texture 19, its alpha the overlay's +0x4c
pub fn message_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let id = if card(m, env, this, 0x1a0, &[]) != 0 { 0x15e } else { 0xdeb };
    let s = loc(m, env, id);
    rcall(m, env, 0x1d0, &[0x13]);
    let tb = scratch;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, m.r32(m.r32(APP) + 0x60)]);
    m.w32(tb + 0x20, 1);
    m.w32(tb + 0x24, 0);
    fcall(m, env, TB_SET_RECT, &[tb], &[F(128.0), F(312.0), F(384.0), F(96.0)]);
    call(m, env, TB_APPEND, &[tb, s]);
    let c = scratch + 0x1470;
    put4(m, c, [rf(m, this + 0x4c), ONE, ONE, ONE]);
    for i in 0..4 {
        m.w32(tb + 0x3c + 4 * i, m.r32(c + 4 * i));
    }
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cSelectNextEventOverlay` vf16 0x1b8d08(this): the card screen: the backdrop's frame (item 5, 420 high) and
/// title (+0x434, item 0xe, 25); its colours (white, a dim white, yellow, the pick's purple; their alphas 0.4 out of
/// state 7); the card system's name (+0x1e0 with +0x21c) centred at 86.4 and its description (+0x1b0) in a box
/// under it; each entry of the list (+0x1d0 the count, +0x70 its name) numbered at 204.8 and named at 236.8, 36
/// apart from 168 (only the first while the overlay's word 0 is 1), the pick (+0x38, when named) on a purple bar;
/// the three options (0x1bb, 0x154, 0x157 at 448; dim unless +0x10 / +0x1c / +0x18) with their pad sprites 0x74,
/// 0x73, 0x72 at 420; out of state 7 the screen shaded (0.8 black)
pub fn card_screen_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let half = F(0.5);
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 5], &[F(420.0)]);
    let s = loc(m, env, m.r32(this + 0x434));
    fcall(m, env, LABEL_BOX, &[m.r32(this + 0x42c), s, 0xe], &[F(25.0)]);
    let (c0, c1, c2, c3) = (scratch, scratch + 0x10, scratch + 0x20, scratch + 0x30);
    put4(m, c0, [ONE; 4]);
    put4(m, c1, [half, ONE, ONE, ONE]);
    put4(m, c2, [ONE, ONE, k(0x3f52_0000), F(0.0)]);
    let pick = [k(0x3f33_3333), k(0x3e20_0000), k(0x3e96_0000), k(0x3f48_0000)];
    put4(m, c3, pick);
    if m.r32(this + 0x54) != 7 {
        let dim = k(0x3ecc_cccd);
        let t = scratch + 0x40;
        put4(m, t, [ONE, dim, dim, dim]);
        for i in 0..4 {
            m.w32(c0 + 4 * i, m.r32(t + 4 * i));
        }
        put4(m, t, [dim, ONE, k(0x3f52_0000), F(0.0)]);
        for i in 0..4 {
            m.w32(c2 + 4 * i, m.r32(t + 4 * i));
        }
        put4(m, t, [dim, pick[1], pick[2], pick[3]]);
        for i in 0..4 {
            m.w32(c3 + 4 * i, m.r32(t + 4 * i));
        }
    }
    let font = m.r32(m.r32(APP) + 0x60);
    rcall(m, env, 0x1d0, &[0xb]);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1f8, &[1, 0]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    rcall(m, env, 0x1d0, &[0xd]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c0]);
    let buf = scratch + 0x50;
    card(m, env, this, 0x1e0, &[m.r32(this + 0x21c), buf, 0x80]);
    let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[font, buf, 0], &[ONE]) as i32);
    let mut y = k(0x42ac_cccd);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, buf], &[ONE, F(320.0) - w * half, y]);
    let tb = scratch + 0x150;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, font]);
    card(m, env, this, 0x1b0, &[buf]);
    let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[font, buf, 0], &[ONE]) as i32);
    let fh = cvt_s_w(m.r32(font + 0xc) as i32);
    y = fh + y;
    fcall(m, env, TB_SET_RECT, &[tb], &[F(320.0) - w * half, y + F(7.0), w, fh * F(3.0)]);
    y = F(168.0);
    call(m, env, TB_APPEND, &[tb, buf]);
    m.w32(tb + 0x20, 0);
    m.w32(tb + 0x24, 0);
    m.w32(tb + 0x20, 1);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    rcall(m, env, 0x1d0, &[0xe]);
    let name = scratch + 0x15c0;
    let mut i = 0u32;
    while (i as i32) < card(m, env, this, 0x1d0, &[]) as i32 {
        card(m, env, this, 0x70, &[i, name]);
        if i == m.r32(this + 0x38) && call(m, env, WSTR_COUNT, &[name]) != 0 {
            rcall(m, env, 0x1d0, &[0xa]);
            rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, c3]);
            fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[F(192.0), y - k(0x4128_f5c2), k(0x43b9_9999), F(36.0)]);
            rcall(m, env, 0x1d0, &[0xe]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, c2]);
        }
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, c0]);
        call(m, env, WSTR_FORMAT_B, &[buf, FMT_NUMBER, i + 1]);
        fcall(m, env, PRINTF_SHADOWED_A, &[font, buf], &[ONE, k(0x434c_cccc), y]);
        fcall(m, env, PRINTF_SHADOWED_A, &[font, name], &[ONE, k(0x436c_cccd), y]);
        i += 1;
        if m.r32(this) == 1 {
            break;
        }
        y = y + F(36.0);
    }
    let base = F(288.0);
    let s0 = loc(m, env, 0x1bb);
    let s1 = loc(m, env, 0x154);
    let s2 = loc(m, env, 0x157);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    let hh = cvt_s_w(m.r32(font + 0xc) as i32) + F(10.0);
    let mut y1 = hh + hh + base;
    let mut y3 = hh * F(4.0) + base;
    let mut y2 = hh * F(3.0) + base;
    let pick = |m: &Mem, o: u32| if m.r32(this + o) != 0 { c0 } else { c1 };
    let up = F(-2.0);
    let c = pick(m, 0x10);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s0], &[ONE, F(448.0), y1]);
    y1 = y1 + up;
    let c = pick(m, 0x1c);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s1], &[ONE, F(448.0), y2]);
    y2 = y2 + up;
    let c = pick(m, 0x18);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s2], &[ONE, F(448.0), y3]);
    y3 = y3 + up;
    let c = pick(m, 0x10);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x74, 0], &[F(420.0), y1, ONE, ONE]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c0]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x73, 0], &[F(420.0), y2, ONE, ONE]);
    let c = pick(m, 0x18);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x72, 0], &[F(420.0), y3, ONE, ONE]);
    if m.r32(this + 0x54) != 7 {
        rcall(m, env, 0x1e0, &[3]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
        colour(m, [k(0x3f4c_cccd), F(0.0), F(0.0), F(0.0)]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[F(0.0), F(0.0), F(640.0), F(480.0)]);
    }
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cSelectNextEventOverlay::DrawText` 0x1b9530(this, text, first, second, swapped): a question: the backdrop's
/// frame (item 0x10, 300 high), the text (when any) in a box across the screen at 204 (120 high; its line height +
/// 4), its lines centred (on texture 19, mode 2), and the answers with their pad sprites (0x73 then 0x74, swapped
/// when `swapped`) on 295: both side by side about the middle, or the one centred (its measure of the text is
/// unused)
pub fn question_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, text: u32, first: u32, second: u32, swapped: u32, scratch: u32) {
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 0x10], &[F(300.0)]);
    rcall(m, env, 0x1d0, &[0x12]);
    let font = m.r32(m.r32(APP) + 0x60);
    let tb = scratch;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, font]);
    fcall(m, env, MEASURE_WIDE, &[font, text, 0], &[ONE]);
    let width = F(640.0);
    let mid = F(320.0);
    fcall(m, env, TB_SET_RECT, &[tb], &[F(0.0), F(204.0), width, F(120.0)]);
    if text != 0 {
        call(m, env, TB_APPEND, &[tb, text]);
        let lh = rf(m, tb + 0x14) + F(4.0);
        fcall(m, env, TB_SET_LINE_HEIGHT, &[tb], &[lh]);
    }
    m.w32(tb + 0x24, 0);
    m.w32(tb + 0x20, 1);
    call(m, env, TB_LINE_COUNT, &[tb]);
    colour(m, [ONE; 4]);
    rcall(m, env, 0x1d0, &[0x13]);
    rcall(m, env, 0x1e0, &[2]);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    let (a, b) = if swapped != 0 { (0x74u32, 0x73u32) } else { (0x73, 0x74) };
    let y = k(0x4393_8000);
    let gap = F(24.0);
    if first != 0 && second != 0 {
        let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[font, second, 0], &[ONE]) as i32);
        let ty = F(296.0);
        let d0 = width * k(0x3e05_1eb8);
        let d1 = width * k(0x3e8a_3d71);
        let l = mid - d1;
        let span = (mid + d0 + gap + w - l) * F(0.5);
        let x1 = mid - span - l + mid;
        let x = x1 - d1;
        fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), a, 0], &[x, y, ONE, ONE]);
        colour(m, [ONE; 4]);
        let x2 = x1 + d0;
        fcall(m, env, PRINTF_SHADOWED_A, &[font, first], &[ONE, x + gap, ty]);
        fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), b, 0], &[x2, y, ONE, ONE]);
        colour(m, [ONE; 4]);
        fcall(m, env, PRINTF_SHADOWED_A, &[font, second], &[ONE, x2 + gap, ty]);
    } else if first != 0 || second != 0 {
        let s = if first != 0 { first } else { second };
        let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[font, s, 0], &[ONE]) as i32);
        let x = mid - (w + gap) * F(0.5);
        fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), a, 0], &[x, y, ONE, ONE]);
        colour(m, [ONE; 4]);
        fcall(m, env, PRINTF_SHADOWED_A, &[font, s], &[ONE, x + gap, y + ONE]);
    }
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cSelectNextEventOverlay::Reset` 0x1b9968(this, text): a notice: the backdrop's frame (item 0x10, 300 high),
/// the text in a box sized to it (by the widths' clamp, + 10) one line high about 240 (its y cut to whole), its
/// line height + 4, centred, white
pub fn notice_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, text: u32, scratch: u32) {
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 0x10], &[F(300.0)]);
    rcall(m, env, 0x1d0, &[0x12]);
    let font = m.r32(m.r32(APP) + 0x60);
    let mut w = F(0.0);
    if text != 0 {
        let n = fcall(m, env, MEASURE_WIDE, &[font, text, 0], &[ONE]);
        w = cvt_s_w(n as i32) + w;
    }
    let w = clamp_width(w);
    let h = cvt_s_w(m.r32(font + 0xc) as i32);
    let bw = w + F(10.0);
    let y = cvt_s_w(crate::sndlib::cvt_w((F(240.0) - h * F(0.5)).0));
    let x = F(320.0) - bw * F(0.5);
    let tb = scratch;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, font]);
    fcall(m, env, TB_SET_RECT, &[tb], &[x, y, bw, h]);
    m.w32(tb + 0x24, 0);
    m.w32(tb + 0x20, 1);
    if text != 0 {
        call(m, env, TB_APPEND, &[tb, text]);
        let lh = rf(m, tb + 0x14) + F(4.0);
        fcall(m, env, TB_SET_LINE_HEIGHT, &[tb], &[lh]);
    }
    call(m, env, TB_LINE_COUNT, &[tb]);
    colour(m, [ONE; 4]);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// Whether slot `i` is usable (both its flags, +0x174 and +0x168 of its 0x58 bytes)
fn slot_on(m: &Mem, this: u32, i: u32) -> bool {
    let o = i.wrapping_mul(0x58);
    m.r32((this + 0x174).wrapping_add(o)) != 0 && m.r32(this.wrapping_add(o) + 0x168) != 0
}

/// `cSelectNextEventOverlay` vf23 0x1b9b80(this): the slots screen: the backdrop's frame (item 5, 380 high), the
/// title (+0x434) at 38.4, 19.2 (1.3); each card slot (the card system's +0x1d8 the count, `MCSlot_GetLabel` its
/// label) at 96, 26.4 apart from 96, white when usable else dim, the pick (+0x21c) on a purple bar; the two
/// options 0x155 (dim unless the pick is usable) and 0x154 at 512 with their pad sprites 0x74 / 0x73 at 484
pub fn slots_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let pick = scratch + 0x200;
    put4(m, pick, [k(0x3f33_3333), k(0x3e20_0000), k(0x3e96_0000), k(0x3f48_0000)]);
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 5], &[F(380.0)]);
    let mut y = F(96.0);
    let font = m.r32(m.r32(APP) + 0x60);
    rcall(m, env, 0x1d0, &[0xb]);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1f8, &[1, 0]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    let s = loc(m, env, m.r32(this + 0x434));
    fcall(m, env, DRAW_STRING_A, &[font, s], &[k(0x4219_9999), k(0x4199_9999), k(0x3fa6_6666)]);
    let (c1, c2) = (scratch + 0x210, scratch + 0x220);
    let mut i = 0u32;
    while (i as i32) < card(m, env, this, 0x1d8, &[]) as i32 {
        if i == m.r32(this + 0x21c) {
            rcall(m, env, 0x1d0, &[0xa]);
            rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, pick]);
            fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[k(0x4266_6668), y - F(3.0), k(0x4403_3333), k(0x41e6_6666)]);
            rcall(m, env, 0x1d0, &[0xe]);
            put4(m, c1, [k(0x3f33_3333), k(0x3e20_0000), k(0x3e96_0000), k(0x3f48_0000)]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, c1]);
        }
        call(m, env, SLOT_LABEL, &[this, i, scratch, 0x100]);
        if slot_on(m, this, i) {
            put4(m, c1, [ONE; 4]);
        } else {
            put4(m, c1, [ONE, F(0.5), F(0.5), F(0.5)]);
        }
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, c1]);
        fcall(m, env, PRINTF_SHADOWED_A, &[font, scratch], &[k(0x3f8c_cccd), F(96.0), y]);
        i += 1;
        y = y + k(0x41d3_3333);
    }
    put4(m, c1, [ONE; 4]);
    put4(m, c2, [F(0.5), ONE, ONE, ONE]);
    let s0 = loc(m, env, 0x155);
    let base = F(288.0);
    let mut x = F(512.0);
    let s2 = loc(m, env, 0x154);
    let h = cvt_s_w(m.r32(font + 0xc) as i32) + F(4.0);
    let sel = m.r32(this + 0x21c);
    let mut y2 = h * F(3.0) + base;
    let mut y1 = h + h + base;
    let c = if slot_on(m, this, sel) { c1 } else { c2 };
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s0], &[ONE, F(512.0), y1]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c1]);
    fcall(m, env, PRINTF_SHADOWED_A, &[font, s2], &[ONE, F(512.0), y2]);
    let sel = m.r32(this + 0x21c);
    y2 = y2 + F(-2.0);
    x = x - F(28.0);
    y1 = y1 + F(-2.0);
    let c = if slot_on(m, this, sel) { c1 } else { c2 };
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x74, 0], &[x, y1, ONE, ONE]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, c1]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x73, 0], &[x, y2, ONE, ONE]);
}

/// vf47 0x1ba0f0(this): the move sound unless the pick (+0x21c) is the last entry (+0x218), then the base's next
pub fn vf47(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if (m.r32(this + 0x21c) as i32) < m.r32(this + 0x218).wrapping_sub(1) as i32 {
        call(m, env, MOVE_SOUND, &[m.r32(this + 0x42c)]);
    }
    call(m, env, MC_NEXT, &[this]);
}

/// vf48 0x1ba138(this): the move sound unless the pick is the first, then the base's previous
pub fn vf48(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if (m.r32(this + 0x21c) as i32) > 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this + 0x42c)]);
    }
    call(m, env, MC_PREV, &[this]);
}

/// vf49 0x1ba170 / vf52 0x1ba1f8(this): the select sound
pub fn select_sound(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, SELECT_SOUND, &[m.r32(this + 0x42c)]);
}

/// vf50 0x1ba178(this): the move sound unless the slot picked (+0x38) is the last of the slots (0x1d1eb8), then
/// the base's next slot
pub fn vf50(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let n = call(m, env, SLOT_COUNT, &[this]);
    if (m.r32(this + 0x38) as i32) < n.wrapping_sub(1) as i32 {
        call(m, env, MOVE_SOUND, &[m.r32(this + 0x42c)]);
    }
    call(m, env, MC_SLOT_NEXT, &[this]);
}

/// vf51 0x1ba1c0(this): the move sound unless the slot picked is the first, then the base's previous slot
pub fn vf51(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if (m.r32(this + 0x38) as i32) > 0 {
        call(m, env, MOVE_SOUND, &[m.r32(this + 0x42c)]);
    }
    call(m, env, MC_SLOT_PREV, &[this]);
}

/// vf53 0x1ba200(this): the back sound
pub fn back_sound(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACK_SOUND, &[m.r32(this + 0x42c)]);
}

// --- G8n5b: the point distribution overlay ---

pub const CAREER: u32 = 0x32b390;
pub const CHARACTERS: u32 = 0x331ea8;
pub const MUSIC: u32 = 0x343050;
pub const EMPTY: u32 = 0x38e220;
/// The four attributes' name ids (set by the reset), the "%s" of the video's name, the attribute line's format
pub const ATTRIB_NAMES: u32 = 0x33d6b0;
pub const FMT_VIDEO: u32 = 0x38e5a0;
pub const FMT_ATTRIB: u32 = 0x38e5b8;
/// Whether the exit waits (1) and, once seen, the points the screen opened with
pub const EXIT_WAITS: u32 = 0x32ea9c;
pub const POINTS_SEEN: u32 = 0x338228;
pub const POINTS_AT_OPEN: u32 = 0x3ce370;
pub const BACKDROP_RESET: u32 = 0x1aa628;
pub const BACKDROP_UPDATE: u32 = 0x1a8bd8;
pub const DIALOG_DRAW: u32 = 0x1aa268;
pub const DIALOG_POLL: u32 = 0x1aac08;
pub const DIALOG_SET: u32 = 0x1aa090;
pub const MC_UPDATE: u32 = 0x1ceee0;
pub const MC_IS_DONE: u32 = 0x1d21a0;
pub const MC_RESULT: u32 = 0x1d21c0;
pub const MC_DESTROY: u32 = 0x1ceeb0;
pub const MC_CREATE: u32 = 0x1cee68;
pub const MC_BEGIN: u32 = 0x1ce8b0;
pub const MC_START_SAVE: u32 = 0x1ce778;
pub const MC_RESTART: u32 = 0x1d2140;
pub const MC_DRAW: u32 = 0x1cf628;
pub const CAREER_SAVE_A: u32 = 0x161a98;
pub const SAVE_TO_CARD: u32 = 0x1d2078;
pub const NEW_BUTTONS: u32 = 0x1a8688;
pub const TICK_SOUND: u32 = 0x1a8638;
pub const ERROR_SOUND: u32 = 0x1a8678;
pub const PLAYER_ID: u32 = 0x166580;
pub const PLAYER_PROFILE: u32 = 0x164330;
pub const PLAYER_LEVEL: u32 = 0x164840;
pub const LEVEL_NAME: u32 = 0x164730;
pub const POINTS: u32 = 0x164368;
pub const REFUND_POINT: u32 = 0x1644e8;
pub const SPEND_POINT: u32 = 0x164398;
pub const ATTRIBUTES: u32 = 0x164638;
pub const SET_PENDING_VIDEO: u32 = 0x163f10;
pub const END_RACE: u32 = 0x113bb0;
pub const SPEECH_LEVEL: u32 = 0x2314e0;
pub const SND_STOP_ONE: u32 = 0x228d10;
pub const OVERLAY_HIDE: u32 = 0x1a7860;
pub const HUD_SHOW: u32 = 0x1a7848;
pub const PRINT_SHADOWED_CENTRED: u32 = 0x19d770;
pub const PRINTF_SHADOWED_CENTERED: u32 = 0x19d6e0;
pub const PRINT_SHADOWED_RIGHT: u32 = 0x19d868;
pub const PRINTF_SHADOWED: u32 = 0x19d3f0;
pub const DRAW_MENU_LIST: u32 = 0x1aa630;
pub const SPRINTF: u32 = 0x2fca28;
pub const STRCPY: u32 = 0x2fd14c;
pub const TB_RESET: u32 = 0x195ee8;
pub const TB_SET_SCALE: u32 = 0x1961f0;
pub const TB_WRAP: u32 = 0x196218;
pub const THING_VTABLE: u32 = 0x3b5a00;
pub const WIDGET_VTABLE: u32 = 0x3b5e18;
pub const PROGRESS_VTABLE: u32 = 0x3b6cf8;
pub const UPDATE_COLOR: u32 = 0x2ad988;
pub const BAR_SET_VALUE: u32 = 0x2b3950;
pub const BAR_SET_POS: u32 = 0x2acb20;
pub const BAR_DRAW: u32 = 0x2ae880;
pub const BAR_DELETE: u32 = 0x2b3a20;

fn gold4() -> [F; 4] {
    [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]
}
fn race(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x730)
}

/// The point distribution overlay's `Reset` 0x1ba208(this) (not in the index): the backdrop reset, the attributes'
/// names (0x25, 0x27, 0x28, 0x29), the player's points kept (+0x438, +0x43c), no card operation, no exit timer, no
/// pending video; the career's +4 cleared and, in the career's +0x370c mode 1, the speech for the player's level
pub fn points_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this + 0x42c)]);
    for (i, id) in [0x25u32, 0x27, 0x28, 0x29].into_iter().enumerate() {
        m.w32(ATTRIB_NAMES + 4 * i as u32, id);
    }
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let n = call(m, env, POINTS, &[CAREER, p]);
    m.w32(this + 0x430, 0);
    m.w32(this + 0x444, 0);
    m.w32(this + 0x448, 0);
    m.w32(this + 0x43c, n);
    m.w32(this + 0x438, n);
    call(m, env, SET_PENDING_VIDEO, &[CAREER, EMPTY]);
    let mode = m.r32(CAREER + 0x370c);
    m.w32(CAREER + 4, 0);
    if mode != 1 {
        return;
    }
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let level = call(m, env, PLAYER_LEVEL, &[CAREER, p]);
    call(m, env, SPEECH_LEVEL, &[m.r32(MUSIC), level]);
}

/// The point distribution overlay's vf16 0x1ba2f0 (not in the index): the sound system's channel 0 stopped
pub fn points_vf16(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, SND_STOP_ONE, &[m.r32(MUSIC), 0]);
}

/// `AttribPointsOverlay_StartExitTimer` 0x1ba300(this) -> the timer's flag: when the exit waits (0x32ea9c is 1), the
/// timer set (+0x444 1, +0x448 5 seconds)
pub fn start_exit_timer(m: &mut Mem, this: u32) -> u32 {
    if m.r32(EXIT_WAITS) == 1 {
        m.w32(this + 0x444, 1);
        wf(m, this + 0x448, F(5.0));
    }
    m.r32(this + 0x444)
}

/// `AttribPointsOverlay_Update` 0x1ba330(this): the backdrop's update; a card operation (+0x430 2) run to its end (a
/// good result saves the career and, unless the exit waits, leaves); the exit timer counted down (1/60 a frame while
/// no dialog is up) to leave at 0; a dialog's answer: 3 (no) leaves unless the exit waits, 1 (yes) to the save
/// question starts the save (+0x434 0x1c4, the operation 0x27d4); no dialog: the pad's new buttons: up / down the
/// attribute (+0x440, wrapping in 0..3), left / right a point refunded / spent (the tick sound, else the error's),
/// select with no points left asks to save (0x1c1, with 0x1d6 / 0x188 / 0x154). Leaving hides overlay item 15 and,
/// in the career's +0x370c mode 1, ends the race (the video "%s" of the player's rider set pending, +0x3260 0x30,
/// the race's +0x54 and the career's +4 set), else shows item 17
pub fn points_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mut leave = false;
    call(m, env, BACKDROP_UPDATE, &[m.r32(this + 0x42c)]);
    if m.r32(this + 0x430) == 2 {
        call(m, env, MC_UPDATE, &[this]);
        if call(m, env, MC_IS_DONE, &[this]) == 0 {
            return;
        }
        if call(m, env, MC_RESULT, &[this]) != 0 {
            call(m, env, CAREER_SAVE_A, &[CAREER]);
            leave = start_exit_timer(m, this) == 0;
        }
        m.w32(this + 0x430, 0);
        call(m, env, MC_DESTROY, &[this]);
    }
    if m.r32(this + 0x444) != 0 {
        let mut t = rf(m, this + 0x448);
        if F(0.0).ee() < t.ee() && m.r32(m.r32(this + 0x42c) + 0x328) == 0 {
            t = t - k(0x3c88_8889);
            wf(m, this + 0x448, t);
        }
        if t.ee() <= F(0.0).ee() {
            leave = true;
        }
    } else if m.r32(m.r32(this + 0x42c) + 0x328) != 0 {
        let r = call(m, env, DIALOG_POLL, &[m.r32(this + 0x42c), 0xffff_ffff]);
        if r == 2 || r == 0 {
            return;
        }
        if r == 3 {
            m.w32(this + 0x430, 0);
            if start_exit_timer(m, this) == 0 {
                leave = true;
            }
        } else if r == 1 && m.r32(this + 0x430) == 1 {
            m.w32(this + 0x430, 2);
            m.w32(this + 0x434, 0x1c4);
            call(m, env, MC_CREATE, &[this]);
            call(m, env, MC_BEGIN, &[this, 0x27d4]);
            call(m, env, MC_START_SAVE, &[this, 1]);
            call(m, env, MC_RESTART, &[this]);
        }
    } else {
        let b = call(m, env, NEW_BUTTONS, &[m.r32(this + 0x42c), 0xffff_ffff]);
        if b != 0 {
            if b & 0x20001 != 0 {
                call(m, env, TICK_SOUND, &[m.r32(this + 0x42c)]);
                let v = m.r32(this + 0x440).wrapping_sub(1);
                m.w32(this + 0x440, if (v as i32) < 0 { 3 } else { v });
            }
            if b & 0x40002 != 0 {
                call(m, env, TICK_SOUND, &[m.r32(this + 0x42c)]);
                let v = m.r32(this + 0x440).wrapping_add(1);
                m.w32(this + 0x440, if (v as i32) < 4 { v } else { 0 });
            }
            for (bits, f) in [(0x100008u32, REFUND_POINT), (0x80004, SPEND_POINT)] {
                if b & bits != 0 {
                    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
                    let sound = if call(m, env, f, &[CAREER, p, m.r32(this + 0x440)]) != 0 { TICK_SOUND } else { ERROR_SOUND };
                    call(m, env, sound, &[m.r32(this + 0x42c)]);
                }
            }
            if b & 0x800 != 0 {
                let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
                if call(m, env, POINTS, &[CAREER, p]) == 0 {
                    m.w32(this + 0x430, 1);
                    let q = loc(m, env, 0x1c1);
                    let a = loc(m, env, 0x1d6);
                    let b2 = loc(m, env, 0x188);
                    let c = loc(m, env, 0x154);
                    call(m, env, DIALOG_SET, &[m.r32(this + 0x42c), q, 0, a, b2, c]);
                }
            }
        }
    }
    if !leave {
        return;
    }
    let r = race(m);
    call(m, env, OVERLAY_HIDE, &[m.r32(r + 0x38), 0xf, 0xffff_ffff]);
    let mode = m.r32(CAREER + 0x370c);
    if mode != 1 {
        let r = race(m);
        call(m, env, HUD_SHOW, &[m.r32(r + 0x38), 0x11, 0xffff_ffff]);
        return;
    }
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    call(m, env, PLAYER_LEVEL, &[CAREER, p]);
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let c = call(m, env, PLAYER_PROFILE, &[CAREER, p]);
    let rider = c.wrapping_mul(0x38).wrapping_add(CHARACTERS);
    call(m, env, END_RACE, &[m.r32(race(m) + 0x1c)]);
    m.w32(CAREER + 0x3260, 0x30);
    let r = race(m);
    m.w32(r + 0x54, mode);
    call(m, env, SPRINTF, &[scratch, FMT_VIDEO, m.r32(rider + 8)]);
    call(m, env, SET_PENDING_VIDEO, &[CAREER, scratch]);
    m.w32(CAREER + 4, mode);
}

fn wu(m: &mut Mem, a: u32, v: u32) {
    m.w32(a, v);
}

/// The progress bar's virtual +0x138 (its table at +0xc) with a rectangle at `rect`
fn bar_rect(m: &mut Mem, env: &mut dyn TextBoxEnv, bar: u32, rect: u32) {
    let vt = m.r32(bar + 0xc);
    let me = bar.wrapping_add(m.r16(vt + 0x138) as i16 as u32);
    let func = m.r32(vt + 0x13c);
    env.call(m, func, &[me, rect], &[]);
}

/// A rectangle's four words at `at`
fn rect4(m: &mut Mem, at: u32, v: [u32; 4]) {
    for (i, w) in v.into_iter().enumerate() {
        wu(m, at + 4 * i as u32, w);
    }
}

/// The progress bar the points screen builds in place (0x190 bytes at `o`): a `cFE3DThing` made a widget (its
/// colour updated) and then a `cFE3DProgressBar` 100 by 30, its colours white, pale and yellow
fn build_bar(m: &mut Mem, env: &mut dyn TextBoxEnv, o: u32, tmp: u32) {
    let one = ONE.0.to_bits();
    let half = 0x3f00_0000u32;
    let gold = [one, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687];
    wu(m, o, 0xffff_ffff);
    for off in [0x10u32, 0x20, 0x24] {
        wu(m, o + off, 0);
    }
    wu(m, o + 0x28, 1);
    wu(m, o + 0x14, 1);
    wu(m, o + 4, o);
    wu(m, o + 8, o);
    for i in 0..8 {
        wu(m, o + 0x40 + 4 * i, 0);
    }
    for off in [0x60u32, 0x6c, 0x64, 0x68] {
        wu(m, o + off, one);
    }
    wu(m, o + 0xc, THING_VTABLE);
    for off in [0x190u32, 0x194, 0x198, 0x18, 0x1c, 0x2c, 0x30, 0x70, 0x74] {
        wu(m, o + off, 0);
    }
    wu(m, o + 0x19c, one);
    for i in 0..4 {
        let v = m.r32(o + 0x190 + 4 * i);
        wu(m, o + 0x80 + 4 * i, v);
    }
    wu(m, o + 0x190, 0);
    let z = m.r32(o + 0x190);
    wu(m, o + 0x194, 0);
    wu(m, o + 0x198, 0);
    for off in [0x90u32, 0x94, 0x98] {
        wu(m, o + off, z);
    }
    for (off, v) in [(0xacu32, gold[0]), (0xb8, gold[3]), (0xb0, gold[1]), (0xb4, gold[2])] {
        wu(m, o + off, v);
    }
    for off in [0xbcu32, 0xc8, 0xc0, 0xc4, 0xcc] {
        wu(m, o + off, one);
    }
    for off in [0xd8u32, 0xd0, 0xd4] {
        wu(m, o + off, half);
    }
    wu(m, o + 0xdc, one);
    wu(m, o + 0xa0, 0);
    wu(m, o + 0xa4, 0);
    wu(m, o + 0xe8, half);
    wu(m, o + 0xa8, 1);
    wu(m, o + 0xe0, half);
    wu(m, o + 0xe4, half);
    wu(m, o + 0xc, WIDGET_VTABLE);
    for off in [0xecu32, 0xf0] {
        wu(m, o + off, 0);
    }
    for off in [0x100u32, 0x104, 0x108] {
        wu(m, o + off, z);
    }
    wu(m, o + 0x10c, one);
    call(m, env, UPDATE_COLOR, &[o]);
    let fifth = 0x3e4c_cccdu32;
    let far = 0xc61c_3c00u32;
    let hundred = F(100.0).0.to_bits();
    for (off, v) in [
        (0x110u32, one), (0x118, one), (0x114, one), (0x11c, z), (0x120, one), (0x128, one), (0x124, one), (0x12c, z),
        (0x130, one), (0x13c, fifth), (0x134, fifth), (0x138, fifth), (0x140, one), (0x144, one), (0x148, z),
        (0x14c, z), (0x150, hundred), (0x154, z), (0x15c, far), (0x158, far), (0x160, z), (0x168, 0),
        (0x174, 0xffff_ffff), (0xc, PROGRESS_VTABLE), (0x188, 0), (0x18c, 0), (0x164, one), (0x16c, 0xffff_ffff),
        (0x170, 0xffff_ffff), (0x178, one), (0x17c, one), (0x180, one), (0x184, one),
    ] {
        wu(m, o + off, v);
    }
    rect4(m, o + 0x1a0, [hundred, F(30.0).0.to_bits(), z, z]);
    bar_rect(m, env, o, o + 0x1a0);
    for off in [0u32, 4, 8, 0xc] {
        wu(m, tmp + off, one);
    }
    wu(m, o + 0x16c, 0x44);
    wu(m, o + 0x174, 0x42);
    wu(m, o + 0x170, 0x43);
    for (at, c) in [(0x110u32, [one; 4]), (0x120, [one; 4]), (0x140, [one, one, z, z])] {
        rect4(m, tmp, c);
        for i in 0..4 {
            let v = m.r32(tmp + 4 * i);
            wu(m, o + at + 4 * i, v);
        }
    }
}

/// `AttribPointsOverlay_Draw` 0x1ba788(this): the dialog when one is up; the card operation's draw while it saves;
/// with the exit timer the level screen (0xd25 / 0xd26, the player's level's name (+1) in gold at 1.2, 0xd27 and the
/// next level's, 0x175 right-aligned); otherwise the points screen: the backdrop's frame (item 0xc, 460 high), the
/// heading 0xea6, a progress bar built on the stack: the points left (`Career_GetAttributePoints`, out of the points
/// the screen opened with +0x438 · 6 wide) at 335, 244 and its label 0xc7b; each attribute's row 19 apart from 270
/// (the picked one in gold with the arrows 0x2b / 0x2c): its name, its bar (its maximum · 5 wide, its value the base
/// + the bonus out of the maximum) at 200 and "%d/%d" of it; the player's rider's name (1.3, 320, 113) and 0xe50;
/// the help 0xc78 in a box at 130, 165; then with no points the help line 0x113, else 0xc79 in a box at 115, 370
/// (its loading of 0x197 unused); the bar deleted. The first time it opens the points are noted (0x338228, 0x3ce370)
pub fn points_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let sp = scratch;
    let b = m.r32(this + 0x42c);
    if m.r32(b + 0x328) != 0 {
        call(m, env, DIALOG_DRAW, &[b]);
        return;
    }
    if m.r32(this + 0x430) == 2 {
        call(m, env, MC_DRAW, &[this]);
        return;
    }
    let gold = gold4();
    let tmp = sp + 0x10;
    if m.r32(this + 0x444) != 0 {
        fcall(m, env, BACKDROP_FRAME, &[b, 0xc], &[F(300.0)]);
        rcall(m, env, 0x1d0, &[0xf]);
        let (s9, s12) = (k(0x3f66_6666), k(0x3f99_999a));
        let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
        let level = call(m, env, LEVEL_NAME, &[CAREER, p]);
        let lines = [
            ([ONE; 4], 0xd25u32, s9, F(165.0)),
            ([ONE; 4], 0xd26, s9, F(200.0)),
            (gold, level.wrapping_add(1), s12, F(230.0)),
            ([ONE; 4], 0xd27, s9, k(0x4384_8000)),
            (gold, level, s12, k(0x4393_8000)),
        ];
        for (i, (c, id, s, y)) in lines.into_iter().enumerate() {
            if i != 1 {
                put4(m, tmp, c);
                call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
            }
            let f = m.r32(m.r32(this + 0x42c) + 4);
            let t = loc(m, env, id);
            fcall(m, env, PRINT_SHADOWED_CENTRED, &[f, t], &[s, F(320.0), y]);
        }
        put4(m, tmp, [ONE; 4]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
        let f = m.r32(m.r32(this + 0x42c) + 4);
        let t = loc(m, env, 0x175);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[f, t], &[k(0x3f5e_b852), F(600.0), k(0x43a1_8000)]);
        return;
    }
    fcall(m, env, BACKDROP_FRAME, &[b, 0xc], &[F(460.0)]);
    put4(m, tmp, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
    let b = m.r32(this + 0x42c);
    let t = loc(m, env, 0xea6);
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b + 8), t], &[k(0x3f45_1eb8), F(115.0), F(48.0)]);
    rcall(m, env, 0x1d0, &[0xf]);
    let bar = sp + 0x20;
    build_bar(m, env, bar, tmp);
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let pts = call(m, env, POINTS, &[CAREER, p]);
    m.w32(sp + 0x2afc, pts);
    if m.r32(POINTS_SEEN) == 0 {
        m.w32(POINTS_SEEN, 1);
        m.w32(POINTS_AT_OPEN, pts);
    }
    let z = 0u32;
    let w = cvt_s_w(m.r32(this + 0x438) as i32) * F(6.0);
    rect4(m, tmp, [w.0.to_bits(), m.r32(bar + 0x104), z, z]);
    bar_rect(m, env, bar, tmp);
    rect4(m, tmp, [m.r32(bar + 0x100), F(10.0).0.to_bits(), z, z]);
    bar_rect(m, env, bar, tmp);
    let top = cvt_s_w(m.r32(this + 0x438) as i32);
    wf(m, bar + 0x180, top);
    wf(m, bar + 0x184, ONE);
    wf(m, bar + 0x150, top);
    fcall(m, env, BAR_SET_VALUE, &[bar], &[cvt_s_w(pts as i32)]);
    rect4(m, tmp, [k(0x43a7_8000).0.to_bits(), F(244.0).0.to_bits(), z, ONE.0.to_bits()]);
    call(m, env, BAR_SET_POS, &[bar, tmp]);
    call(m, env, BAR_DRAW, &[bar]);
    put4(m, tmp, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
    let s88 = k(0x3f61_47ae);
    let f = m.r32(m.r32(this + 0x42c) + 4);
    let t = loc(m, env, 0xc7b);
    fcall(m, env, PRINT_SHADOWED_RIGHT, &[f, t], &[s88, F(328.0), F(240.0)]);
    let stats = sp + 0x1d0;
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    call(m, env, ATTRIBUTES, &[CAREER, p, stats]);
    let mut y = F(270.0);
    let arrow = k(0x3f2e_147b);
    for i in 0..4u32 {
        m.w32(sp + 0x2b00, i);
        let name = loc(m, env, m.r32(ATTRIB_NAMES + 4 * i));
        wf(m, tmp, ONE);
        let ty = if m.r32(this + 0x440) == i {
            wf(m, tmp + 4, k(0x3f73_f7cf));
            wf(m, tmp + 8, k(0x3f3a_9fbe));
            wf(m, tmp + 0xc, k(0x3dd9_1687));
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
            let ay = y - F(3.0);
            fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x2b, 0], &[F(182.0), ay, arrow, arrow]);
            fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), 0x2c, 0], &[k(0x43e3_8000), ay, arrow, arrow]);
            y - F(5.0)
        } else {
            for o in [4u32, 8, 0xc] {
                wf(m, tmp + o, ONE);
            }
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
            y - F(5.0)
        };
        let f = m.r32(m.r32(this + 0x42c) + 4);
        fcall(m, env, PRINT_SHADOWED_RIGHT, &[f, name], &[s88, F(177.0), ty]);
        let row = stats + 0x10 * i;
        let max = m.r32(row + 0xc);
        let w = cvt_s_w(max as i32) * F(5.0);
        rect4(m, tmp, [w.0.to_bits(), m.r32(bar + 0x104), 0, 0]);
        bar_rect(m, env, bar, tmp);
        rect4(m, tmp, [m.r32(bar + 0x100), F(10.0).0.to_bits(), 0, 0]);
        bar_rect(m, env, bar, tmp);
        let fm = cvt_s_w(m.r32(row + 0xc) as i32);
        wf(m, bar + 0x184, ONE);
        wf(m, bar + 0x150, fm);
        wf(m, bar + 0x180, fm);
        fcall(m, env, BAR_SET_VALUE, &[bar], &[cvt_s_w(m.r32(row + 4) as i32)]);
        let sum = cvt_s_w(m.r32(row + 4) as i32) + cvt_s_w(m.r32(row + 8) as i32);
        rect4(m, tmp, [F(200.0).0.to_bits(), y.0.to_bits(), 0, ONE.0.to_bits()]);
        wf(m, bar + 0x158, sum);
        call(m, env, BAR_SET_POS, &[bar, tmp]);
        let total = m.r32(row + 4).wrapping_add(m.r32(row + 8));
        call(m, env, SPRINTF, &[tmp, FMT_ATTRIB, total, m.r32(row + 0xc)]);
        let f = m.r32(m.r32(this + 0x42c) + 4);
        fcall(m, env, PRINTF_SHADOWED, &[f, tmp], &[s88, F(470.0), ty]);
        call(m, env, BAR_DRAW, &[bar]);
        y = y + F(19.0);
    }
    let p = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let c = call(m, env, PLAYER_PROFILE, &[CAREER, p]);
    let name = sp + 0x1b0;
    call(m, env, STRCPY, &[name, m.r32(c.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4)]);
    put4(m, tmp, [ONE, ONE, k(0x3ef1_26e9), k(0x3d1f_be77)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
    let f = m.r32(m.r32(this + 0x42c) + 8);
    fcall(m, env, PRINTF_SHADOWED_CENTERED, &[f, name], &[k(0x3fa6_6666), F(320.0), F(113.0)]);
    put4(m, tmp, gold);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, tmp]);
    let f = m.r32(m.r32(this + 0x42c) + 8);
    let t = loc(m, env, 0xe50);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[f, t], &[s88, F(320.0), F(75.0)]);
    let tb = sp + 0x210;
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, m.r32(m.r32(this + 0x42c) + 4)]);
    call(m, env, TB_RESET, &[tb]);
    put4(m, tmp, [ONE; 4]);
    for i in 0..4 {
        wu(m, tb + 0x3c + 4 * i, m.r32(tmp + 4 * i));
    }
    wu(m, tb + 0x6c, 1);
    wu(m, tb + 0x38, m.r32(m.r32(this + 0x42c) + 0x70));
    let s9 = k(0x3f66_6666);
    fcall(m, env, TB_SET_SCALE, &[tb], &[s9, s9]);
    fcall(m, env, TB_SET_RECT, &[tb], &[F(130.0), F(165.0), F(400.0), F(100.0)]);
    let t = loc(m, env, 0xc78);
    call(m, env, TB_APPEND, &[tb, t]);
    call(m, env, TB_WRAP, &[tb, 0]);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    if m.r32(sp + 0x2afc) == 0 {
        let (label, on) = (sp + 0x2af0, sp + 0x2af4);
        let t = loc(m, env, 0x113);
        m.w32(label, t);
        m.w32(on, 1);
        fcall(m, env, DRAW_MENU_LIST, &[m.r32(this + 0x42c), 3, 0, label, 1, 0, 0x74, 1, 0, on], &[F(600.0), F(430.0), k(0x3f3a_e148)]);
    } else {
        loc(m, env, 0x197);
        let tb2 = sp + 0x1680;
        call(m, env, TB_INIT, &[tb2]);
        call(m, env, TB_SET_FONT, &[tb2, m.r32(m.r32(this + 0x42c) + 4)]);
        call(m, env, TB_RESET, &[tb2]);
        put4(m, tmp, [ONE; 4]);
        for i in 0..4 {
            wu(m, tb2 + 0x3c + 4 * i, m.r32(tmp + 4 * i));
        }
        wu(m, tb2 + 0x38, m.r32(m.r32(this + 0x42c) + 0x70));
        let s8 = k(0x3f4c_cccd);
        fcall(m, env, TB_SET_SCALE, &[tb2], &[s8, s8]);
        fcall(m, env, TB_SET_RECT, &[tb2], &[F(115.0), F(370.0), F(400.0), F(50.0)]);
        let t = loc(m, env, 0xc79);
        call(m, env, TB_APPEND, &[tb2, t]);
        // (the alignment words written are the first box's)
        wu(m, tb + 0x20, 0);
        wu(m, tb + 0x24, 0);
        wu(m, tb + 0x20, 1);
        call(m, env, TB_WRAP, &[tb2, 0]);
        call(m, env, TB_DRAW, &[tb2, 0xffff_ffff, 0]);
        call(m, env, TB_DELETE, &[tb2, 2]);
    }
    call(m, env, TB_DELETE, &[tb, 2]);
    call(m, env, BAR_DELETE, &[bar, 2]);
}

/// The point distribution overlay's vf3 0x1bb670(this): the career saved to the card (`Career_SaveToCard`)
pub fn points_vf3(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, SAVE_TO_CARD, &[this]);
}

// --- G8n5c: the venue and event choices and the race help overlay ---

/// The venue lists' counts (the race courses, the free ride's, the others') and the courses' records (0x1c each,
/// +4 the name), the venue texts' buffers (0x50 each), the venue text's format
pub const VENUE_COUNTS: u32 = 0x37ed20;
pub const COURSE_RECORDS: u32 = 0x332f28;
pub const VENUE_TEXTS: u32 = 0x3ce378;
pub const FMT_VENUE: u32 = 0x38e5f0;
pub const VENUE_RACE: u32 = 0x16b9f0;
pub const VENUE_FREE: u32 = 0x16b9a0;
pub const VENUE_OTHER: u32 = 0x16b9c8;
pub const COURSE_UNLOCKED: u32 = 0x163bb8;
pub const COURSE_MEDAL: u32 = 0x163e28;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const STRLEN: u32 = 0x2fd260;
pub const RESULTS_UPDATE: u32 = 0x1ab148;
pub const VEC4_SET: u32 = 0x14f5f0;
pub const MEMSET: u32 = 0x2cc808;

/// An unsigned word as a float (the game's halving for the top bit)
fn u_to_f(v: u32) -> F {
    if (v as i32) < 0 {
        let h = (v >> 1) | (v & 1);
        let f = cvt_s_w(h as i32);
        f + f
    } else {
        cvt_s_w(v as i32)
    }
}

/// The select next venue overlay's `Reset` 0x1bb678(this) (not in the index): the backdrop reset, no pick (+0x558);
/// the venues of the career's +0x3818 mode (1 the race courses `0x16b9f0`, 2 / 4 / 7 the free ride's `0x16b9a0`,
/// others `0x16b9c8`; mode 3 adds course 6 at the third list's count) at +0x440 (+0x43c of them, +0x438 the menu's
/// items with two more), their sprites (0x45 + the venue, for venues under 12) at +0x4ec, their texts' buffers at
/// +0x474, whether each is unlocked (+0x4b0); the menu's top (+0x560, about 240) and left (+0x564, 320 less half
/// the longest name); the names copied in, the second to last item empty and the last 0x18f
pub fn venue_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this + 0x42c)]);
    m.w32(this + 0x558, 0);
    let mode = m.r32(CAREER + 0x3818);
    let (count_at, f) = match mode {
        1 => (VENUE_COUNTS + 8, VENUE_RACE),
        2 | 4 | 7 => (VENUE_COUNTS, VENUE_FREE),
        _ => (VENUE_COUNTS + 4, VENUE_OTHER),
    };
    let n = m.r32(count_at);
    m.w32(this + 0x43c, n);
    m.w32(this + 0x438, n.wrapping_add(2));
    let mut i = 0u32;
    while (i as i32) < m.r32(count_at) as i32 {
        let v = call(m, env, f, &[i]);
        m.w32(this + 0x440 + 4 * i, v);
        i += 1;
    }
    if mode == 3 {
        let at = m.r32(VENUE_COUNTS + 4);
        m.w32(this + 0x438, m.r32(this + 0x438).wrapping_add(1));
        m.w32(this.wrapping_add(at << 2) + 0x440, 6);
        m.w32(this + 0x43c, m.r32(this + 0x43c).wrapping_add(1));
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0x43c) as i32 {
        let v = m.r32(this + 0x440 + 4 * i);
        if v < 12 {
            m.w32(this + 0x4ec + 4 * i, 0x45 + v);
        }
        i += 1;
    }
    let texts = this + 0x474;
    let n = m.r32(this + 0x438);
    if (n as i32) > 0 {
        for i in 0..n {
            m.w32(texts + 4 * i, VENUE_TEXTS + 0x50 * i);
        }
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0x43c) as i32 {
        let v = call(m, env, COURSE_UNLOCKED, &[CAREER, mode, m.r32(this + 0x440 + 4 * i)]);
        m.w32(this + 0x4b0 + 4 * i, v);
        i += 1;
    }
    let items = cvt_s_w(m.r32(this + 0x438) as i32);
    let font = m.r32(m.r32(this + 0x42c) + 4);
    let h = cvt_s_w(m.r32(font + 0xc) as i32) * k(0x3f3a_e148);
    wf(m, this + 0x564, F(640.0));
    wf(m, this + 0x560, (F(480.0) - items * h) * F(0.5));
    let mut w = F(0.0);
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0x43c) as i32 {
        let rec = m.r32(this + 0x440 + 4 * i).wrapping_mul(0x1c).wrapping_add(COURSE_RECORDS);
        call(m, env, WSTR_SET_ASCII, &[m.r32(texts + 4 * i), m.r32(rec + 4)]);
        let len = u_to_f(call(m, env, STRLEN, &[m.r32(rec + 4)]));
        if w.ee() < len.ee() {
            w = len;
        }
        i += 1;
    }
    let n = m.r32(this + 0x438);
    call(m, env, WSTR_SET_ASCII, &[m.r32(texts.wrapping_add(n.wrapping_sub(2) << 2)), EMPTY]);
    let last = texts.wrapping_add(m.r32(this + 0x438).wrapping_sub(1) << 2);
    let s = loc(m, env, 0x18f);
    call(m, env, WSTR_COPY, &[m.r32(last), s]);
    let x = rf(m, this + 0x564) - w * F(0.5);
    wf(m, this + 0x564, x);
}

/// The dialog when one is up, the card operation's draw while it saves: whether either drew
fn dialog_or_card(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> bool {
    let b = m.r32(this + 0x42c);
    if m.r32(b + 0x328) != 0 {
        call(m, env, DIALOG_DRAW, &[b]);
        return true;
    }
    if m.r32(this + 0x430) == 2 {
        call(m, env, MC_DRAW, &[this]);
        return true;
    }
    false
}

/// A choice screen's head: texture 16, the backdrop's frame (item 0xe, `h` high), the heading `id` (white by the
/// alpha) at 116, 7 below the frame's top + `drop`, and its text box (gold by the alpha, 0.62) at 134 under it; the
/// heading's y
fn choice_head(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, h: F, drop: F, id: u32, gap: F, tb: u32, col: u32) {
    rcall(m, env, 0x1d0, &[0x10]);
    fcall(m, env, BACKDROP_FRAME, &[m.r32(this + 0x42c), 0xe], &[h]);
    let r = call(m, env, GET_RECT, &[m.r32(m.r32(this + 0x42c)), 0x37]);
    let mut y = rf(m, r + 8) + drop;
    put4(m, col, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let b = m.r32(this + 0x42c);
    let a = rf(m, DRAW_COLOUR) * rf(m, b + 0x70);
    y = y + F(7.0);
    let s = loc(m, env, id);
    wf(m, DRAW_COLOUR, a);
    fcall(m, env, PRINTF_SHADOWED_A, &[m.r32(b + 8), s], &[ONE, F(116.0), y]);
    call(m, env, TB_INIT, &[tb]);
    call(m, env, TB_SET_FONT, &[tb, m.r32(m.r32(this + 0x42c) + 8)]);
    call(m, env, TB_RESET, &[tb]);
    put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    for i in 0..4 {
        m.w32(tb + 0x3c + 4 * i, m.r32(col + 4 * i));
    }
    m.w32(tb + 0x38, m.r32(m.r32(this + 0x42c) + 0x70));
    let s = k(0x3f1e_b852);
    fcall(m, env, TB_SET_SCALE, &[tb], &[s, s]);
    fcall(m, env, TB_SET_RECT, &[tb], &[F(134.0), y + gap, F(390.0), F(150.0)]);
}

/// The venue / event text: "0xe76 `id` `tail`" through the format 0x38e5f0, into the box
fn choice_text(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, id: u32, tail: u32, buf: u32) {
    let a = loc(m, env, 0xe76);
    let b = loc(m, env, id);
    let c = loc(m, env, tail);
    call(m, env, WSTR_FORMAT_B, &[buf, FMT_VENUE, a, b, c]);
    call(m, env, TB_APPEND, &[tb, buf]);
}

/// `NextEventOverlay_DrawVenue` 0x1bbf20(this): the dialog or the card operation's draw; else the head (0xe4e, its
/// text by the career's +0x3818 mode: 0xea9, 0xeaa, 0xe7b, 0xeab, 0xeac for 4, 5, 1, 2, 3, else 0xead); the venues'
/// menu (4, from +0x474, +0x438 items, the pick +0x558, unlocked by +0x4b0) at 320, 170 with the pick's sprite (when
/// unlocked) at 116; the help line 0x1d7 right-aligned at 600, 430; in modes 4 and 5 the medals' heading 0x156 and
/// each venue's medal (0x135, 0x12f, 0x12c for 1, 0, 2, else 0x132; dim when locked) at 450
pub fn draw_venue(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if dialog_or_card(m, env, this) {
        return;
    }
    let (col, tb) = (scratch + 0x10, scratch + 0x20);
    choice_head(m, env, this, F(460.0), F(10.0), 0xe4e, F(40.0), tb, col);
    let mode = m.r32(CAREER + 0x3818);
    let id = match mode {
        4 => 0xea9,
        5 => 0xeaa,
        1 => 0xe7b,
        2 => 0xeab,
        3 => 0xeac,
        _ => 0xead,
    };
    choice_text(m, env, tb, id, 0xe77, scratch + 0x1490);
    let y = F(170.0);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    let s92 = k(0x3f6b_851f);
    let (cnt, sel) = (m.r32(this + 0x438), m.r32(this + 0x558));
    fcall(m, env, DRAW_MENU_LIST, &[m.r32(this + 0x42c), 4, 0, this + 0x474, cnt, sel, 0x74, 1, 1, this + 0x4b0], &[F(320.0), y, s92]);
    let p = m.r32(this + 0x558);
    if (p as i32) < m.r32(this + 0x43c) as i32 && m.r32((this + 0x4b0).wrapping_add(p << 2)) != 0 {
        let sprite = m.r32(this.wrapping_add(p << 2) + 0x4ec);
        fcall(m, env, DRAW_SPRITE, &[m.r32(this + 0x42c), sprite, 0], &[F(116.0), y, ONE, ONE]);
    }
    help_line(m, env, this, F(430.0), scratch + 0x1640, true);
    if mode.wrapping_sub(4) >= 2 {
        call(m, env, TB_DELETE, &[tb, 2]);
        return;
    }
    put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let f = m.r32(m.r32(this + 0x42c) + 4);
    let s = loc(m, env, 0x156);
    fcall(m, env, PRINTF_SHADOWED_A, &[f, s], &[k(0x3f66_6666), F(450.0), F(148.0)]);
    let name = scratch + 0x1620;
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0x43c) as i32 {
        let medal = call(m, env, COURSE_MEDAL, &[CAREER, m.r32(CAREER + 0x3818), m.r32(this + 0x440 + 4 * i)]);
        let id = match medal {
            1 => 0x135,
            0 => 0x12f,
            2 => 0x12c,
            _ => 0x132,
        };
        let s = loc(m, env, id);
        call(m, env, WSTR_COPY, &[name, s]);
        put4(m, col, [ONE; 4]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        if m.r32(this + 0x4b0 + 4 * i) == 0 {
            let d = k(0x3ecc_cccd);
            put4(m, col, [ONE, d, d, d]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        }
        let f = m.r32(m.r32(this + 0x42c) + 4);
        let h = cvt_s_w(m.r32(f + 0xc) as i32);
        let y = (h * s92 + F(4.0)) * cvt_s_w(i as i32) + F(170.0);
        fcall(m, env, PRINTF_SHADOWED_A, &[f, name], &[s92, F(450.0), y]);
        i += 1;
    }
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// The help line 0x1d7 (its measure the second loading) right-aligned at 600, `y` (the venue's on texture 2)
fn help_line(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, y: F, at: u32, always: bool) {
    let s73 = k(0x3f3a_e148);
    let s = loc(m, env, 0x1d7);
    m.w32(at, s);
    m.w32(at + 4, 1);
    let f = m.r32(m.r32(this + 0x42c) + 4);
    let s = loc(m, env, 0x1d7);
    let w = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[f, s, 0], &[s73]) as i32);
    let x = F(600.0) - w;
    if always || m.r32(this + 0x464) != 0 {
        fcall(m, env, DRAW_MENU_LIST, &[m.r32(this + 0x42c), 2, 0, at, 1, 0xffff_ffff, 0x73, 1, 0, at + 4], &[x, y, s73]);
    }
}

/// The venue / event overlays' vf3 0x1bc6a0 / 0x1bd188(this): the career saved to the card
pub fn venue_vf3(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, SAVE_TO_CARD, &[this]);
}

/// The select next event overlay's `Reset` 0x1bc6a8(this) (not in the index): the backdrop reset, the five items
/// (0x19b, 0x1c9, 0xc7a, 0x1c8, 0x18f at +0x438), all on (+0x44c..; the third off in the career's modes 4 and 5),
/// the pick (+0x460: 2 in mode 7, 1 in 3 and 5, else 0), the menu at scale 1 about 240 (+0x468) and 320 less half
/// the widest item (+0x46c); no pending video, the career's +4 cleared
pub fn event_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this + 0x42c)]);
    for (i, id) in [0x19bu32, 0x1c9, 0xc7a, 0x1c8, 0x18f].into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(this + 0x438 + 4 * i as u32, s);
    }
    let mode = m.r32(CAREER + 0x2afc);
    for i in 0..5 {
        m.w32(this + 0x44c + 4 * i, 1);
    }
    if mode.wrapping_sub(4) < 2 {
        m.w32(this + 0x454, 0);
    }
    let pick = match mode {
        7 => 2,
        3 | 5 => 1,
        _ => 0,
    };
    m.w32(this + 0x460, pick);
    let font = m.r32(m.r32(this + 0x42c) + 4);
    let h = cvt_s_w(m.r32(font + 0xc) as i32) + F(4.0);
    wf(m, this + 0x470, ONE);
    wf(m, this + 0x46c, F(320.0));
    wf(m, this + 0x468, (F(480.0) - h * F(5.0)) * F(0.5));
    let mut w = F(0.0);
    for i in 0..5 {
        let f = m.r32(m.r32(this + 0x42c) + 4);
        let n = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[f, m.r32(this + 0x438 + 4 * i), 0], &[rf(m, this + 0x470)]) as i32);
        if w.ee() < n.ee() {
            w = n;
        }
    }
    let x = rf(m, this + 0x46c) - w * F(0.5);
    wf(m, this + 0x46c, x);
    call(m, env, SET_PENDING_VIDEO, &[CAREER, EMPTY]);
    m.w32(CAREER + 4, 0);
}

/// `NextEventOverlay_DrawEvent` 0x1bcd28(this): the dialog or the card operation's draw; else the head (300 high,
/// 0xe49 90 lower; its text 0xe79 in the career's modes 4 and 5, 0xe7b in 1, else 0xe7a, with 0xe78); the help line
/// (when +0x464) at 350; the five items' menu (4, the pick +0x460, on by +0x44c) at 320, 240
pub fn draw_event(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if dialog_or_card(m, env, this) {
        return;
    }
    let (col, tb) = (scratch + 0x10, scratch + 0x20);
    choice_head(m, env, this, F(300.0), F(90.0), 0xe49, F(32.0), tb, col);
    let mode = m.r32(CAREER + 0x2afc);
    let id = if mode.wrapping_sub(4) < 2 {
        0xe79
    } else if mode == 1 {
        0xe7b
    } else {
        0xe7a
    };
    choice_text(m, env, tb, id, 0xe78, scratch + 0x1490);
    call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    help_line(m, env, this, k(0x43af_0000), scratch + 0x1620, false);
    let sel = m.r32(this + 0x460);
    fcall(m, env, DRAW_MENU_LIST, &[m.r32(this + 0x42c), 4, 0, this + 0x438, 5, sel, 0x74, 1, 1, this + 0x44c], &[F(320.0), F(240.0), k(0x3f6b_851f)]);
    call(m, env, TB_DELETE, &[tb, 2]);
}

/// `cHelpRaceOverlay::Reset` 0x1bd2b0(this): the backdrop reset, the buffers cleared, no menu, the help 0x18d
pub fn help_reset(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BACKDROP_RESET, &[m.r32(this)]);
    crate::raceoverlays::clear_buffers(m, env);
    m.w32(this + 0x38, 0);
    m.w32(this + 0x34, 0);
    let s = loc(m, env, 0x18d);
    m.w32(this + 0x10, s);
    m.w32(this + 0xc, 0);
}

/// The race help overlay's empty virtual 0x1bd308 (not in the index)
pub fn help_empty() {}

/// `cHelpRaceOverlay::Update` 0x1bd310(this): the results' update; the menu shown once the race's +0x1c object's
/// +0x4f8 is clear
pub fn help_update(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, RESULTS_UPDATE, &[this]);
    let r = race(m);
    if m.r32(m.r32(r + 0x1c) + 0x4f8) == 0 {
        m.w32(this + 0x34, 1);
    }
}

/// `cHelpRaceOverlay::Draw` 0x1bd358(this): the backdrop's frame (item 0xe, 420 high) with its +0x50 at 0.8 for it
pub fn help_draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let b = m.r32(this);
    let kept = rf(m, b + 0x50);
    wf(m, b + 0x50, k(0x3f4c_cccd));
    fcall(m, env, BACKDROP_FRAME, &[b, 0xe], &[F(420.0)]);
    wf(m, m.r32(this) + 0x50, kept);
}

/// `cHelpRaceOverlay::DrawText` 0x1bd3b8(this): on texture 17 the heading 0xebd centred in gold; the lines 0xded,
/// 0xdee, 0xdef (then 0xdf0 after it, 44 on) and 0xdf1 50 apart from 170 at 160 (115 in the app's modes 2 and 3)
/// with a bullet square 18 left of each; the help line 0x155 under the frame's middle; the pad sprites 0x71 and
/// 0x75 after the third and fifth lines (its rect read unused)
pub fn help_draw_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mut x = F(160.0);
    call(m, env, GET_RECT, &[m.r32(m.r32(this)), 0x37]);
    rcall(m, env, 0x1d0, &[0x11]);
    let mode = m.r32(m.r32(APP) + 0x58);
    if mode == 2 || mode == 3 {
        x = F(115.0);
    }
    let col = scratch + 0x10;
    put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let b = m.r32(this);
    wf(m, DRAW_COLOUR, rf(m, b + 0x70));
    let s = loc(m, env, 0xebd);
    fcall(m, env, PRINT_SHADOWED_CENTRED, &[m.r32(b + 8), s], &[k(0x3f68_f5c3), F(320.0), F(105.0)]);
    let s9 = k(0x3f66_6666);
    put4(m, col, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let b = m.r32(this);
    wf(m, DRAW_COLOUR, rf(m, b + 0x70));
    for (id, y) in [(0xdedu32, F(170.0)), (0xdee, F(220.0)), (0xdef, F(270.0))] {
        let f = m.r32(m.r32(this) + 4);
        let s = loc(m, env, id);
        fcall(m, env, PRINTF_SHADOWED_A, &[f, s], &[s9, x, y]);
    }
    let f = m.r32(m.r32(this) + 4);
    let s = loc(m, env, 0xdef);
    let w1 = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[f, s, 0], &[s9]) as i32);
    let f = m.r32(m.r32(this) + 4);
    let s = loc(m, env, 0xdf0);
    fcall(m, env, PRINTF_SHADOWED_A, &[f, s], &[s9, x + w1 + F(44.0), F(270.0)]);
    let f = m.r32(m.r32(this) + 4);
    let s = loc(m, env, 0xdf1);
    fcall(m, env, PRINTF_SHADOWED_A, &[f, s], &[s9, x, F(320.0)]);
    let f = m.r32(m.r32(this) + 4);
    let s = loc(m, env, 0xdf1);
    let w2 = cvt_s_w(fcall(m, env, MEASURE_WIDE, &[f, s, 0], &[s9]) as i32);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0]);
    let bx = x - F(18.0);
    for y in [F(175.0), F(225.0), k(0x4389_8000), k(0x43a2_8000)] {
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[bx, y, F(7.0), F(7.0)]);
    }
    let y = (rf(m, m.r32(this) + 0x2c) + F(480.0)) * F(0.5) - F(40.0);
    let s = loc(m, env, 0x155);
    let (label, on) = (scratch + 0x20, scratch + 0x24);
    m.w32(label, s);
    m.w32(on, 1);
    fcall(m, env, DRAW_MENU_LIST, &[m.r32(this), 3, 0, label, 1, 0, 0x74, 1, 1, on], &[F(600.0), y, k(0x3f3a_e148)]);
    put4(m, col, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let b = m.r32(this);
    wf(m, DRAW_COLOUR, rf(m, b + 0x70));
    fcall(m, env, DRAW_SPRITE, &[b, 0x71, 0], &[x + F(8.0) + w1, k(0x4383_0000), ONE, ONE]);
    fcall(m, env, DRAW_SPRITE, &[m.r32(this), 0x75, 0], &[x + F(10.0) + w2, F(320.0), ONE, ONE]);
}

/// `StaticInit_HelpRaceOverlay` 0x1bd898: the race help's static vectors and colours (0x3ce790..0x3ce81c,
/// 0x3381e8..0x338207) and the name grid's x (0x3ce788, the static 0x338220 + 131)
pub fn help_static_init(m: &mut Mem) {
    let (one, half, zero) = (ONE.0.to_bits(), 0x3f00_0000u32, 0u32);
    m.w32(0x3ce808, 0);
    let z = m.r32(0x3ce808);
    for (a, v) in [
        (0x3ce818u32, 0x3b03_126f), (0x3ce798, half), (0x3ce810, one), (0x3ce814, 0x3d9b_3d08), (0x3ce800, one),
        (0x3ce804, 0x3ef2_fec5), (0x3ce7f0, one), (0x3ce7f4, 0x3f76_e979), (0x3ce7f8, zero), (0x3ce7b0, half),
        (0x3ce7c0, one), (0x3ce7b8, half), (0x3ce7a0, z), (0x3ce7e8, one), (0x3ce7c8, one), (0x3ce7a8, one),
        (0x3ce790, z), (0x3381e8, one), (0x3381f4, one), (0x3ce7d0, 0xffff_ffff), (0x3ce7d4, one), (0x3ce7d8, one),
        (0x3ce7dc, z), (0x3ce7e0, z), (0x3ce7e4, one), (0x3ce7c4, z), (0x3ce7b4, z), (0x3ce7a4, z), (0x3ce794, z),
        (0x3381ec, z), (0x3381f0, z), (0x3381f8, one),
    ] {
        m.w32(a, v);
    }
    let x = rf(m, 0x338220) + F(131.0);
    m.w32(0x338204, z);
    m.w32(0x3381fc, one);
    m.w32(0x338200, z);
    wf(m, 0x3ce788, x);
}

/// 0x1bda18(v, from) (not in the index): `Vec4_Set` of the four floats at `from`
pub fn vec4_from(m: &mut Mem, env: &mut dyn TextBoxEnv, v: u32, from: u32) {
    fcall(m, env, VEC4_SET, &[v], &[rf(m, from), rf(m, from + 4), rf(m, from + 8), rf(m, from + 0xc)]);
}

/// 0x1bda30(this, v) (not in the index): the event overlay's help line on or off (+0x464)
pub fn set_help(m: &mut Mem, this: u32, v: u32) {
    m.w32(this + 0x464, v);
}

/// 0x1bda38(this) (not in the index): `this`
pub fn identity(this: u32) -> u32 {
    this
}

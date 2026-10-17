//! The text box (board row G7n): `TextBox_*` 0x1957b0..0x1965b4, the wrapped, aligned block of wide text the
//! front end, the HUD and the trick book draw their paragraphs with. On the bytes at their addresses ([`Mem`]);
//! the font and string functions are calls out ([`TextBoxEnv`]). Checked in the function runner
//! (`tests/textbox.rs`).
//!
//! **The box** (0x1470 bytes):
//! - +0 the font (its +0xc the glyphs' height, an integer); +4, +8 the place; +0xc, +0x10 the size.
//! - +0x14 the line height; +0x18 the scale; +0x1c the first line drawn.
//! - +0x20 centred, +0x24 right-aligned; +0x28 every line's indent but the first; +0x2c, +0x30 the first line's
//!   place (as integers, from the last draw); +0x34 a shift on x (`draw_vcentered`); +0x38 the alpha's scale.
//! - +0x3c the colour; +0x4c the highlight's; +0x5c the shadow's; +0x6c the shadow (on or off).
//! - +0x70 the lines (128 pointers into +0x270); +0x270 the text cut into lines; +0xa70 the text; +0x1270 the
//!   lines' widths (128 integers).
//!
//! A text style the draws build on the game's stack (0x10 bytes) the port builds in `scratch` (STANDIN).

use crate::animseq::cvt_s_w;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::sndlib::cvt_w;

pub const WSTR_COUNT: u32 = 0x17cc48;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
pub const WSTR_APPEND: u32 = 0x17ce00;
pub const STRLEN: u32 = 0x2fd260;
pub const ALLOC: u32 = 0x2ccf70;
pub const MEM_SET: u32 = 0x2cc808;
pub const FREE: u32 = 0x14a040;
pub const SET_COLOR: u32 = 0x14eee8;
pub const DRAW_STRING: u32 = 0x19d0c0;
pub const MEASURE: u32 = 0x19dc78;
pub const STYLE_INIT: u32 = 0x19dec0;
pub const STYLE_DESTRUCT: u32 = 0x19ded0;
pub const DRAW_ALIGNED: u32 = 0x19df90;
pub const STYLE_SET_FONT: u32 = 0x19e380;
pub const STYLE_CONSTRUCT: u32 = 0x19e388;
pub const STYLE_DESTRUCT_THUNK: u32 = 0x19e3b0;
pub const DRAW_WITH_CURSOR: u32 = 0x19e460;
/// The font drawer (its colour first)
pub const DRAWER: u32 = 0x337fe8;
/// The string a line break is appended with (`AppendText`)
pub const BREAK: u32 = 0x337e58;
/// The heap `TextWidget_SetAscii` takes its buffer from
pub const HEAP: u32 = 0x38aaf0;

const LINES: u32 = 0x70;
const CUT: u32 = 0x270;
const TEXT: u32 = 0xa70;
const WIDTHS: u32 = 0x1270;

/// The calls out: a function with these words (a0..) and floats (f12..); its answer
pub trait TextBoxEnv {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32;
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn wq(m: &mut Mem, a: u32, v: [F; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        wf(m, a + 4 * k as u32, x);
    }
}
fn clear_lines(m: &mut Mem, tb: u32) {
    for k in 0..128 {
        m.w32(tb + LINES + 4 * k, 0);
    }
}
fn font_height(m: &Mem, tb: u32) -> F {
    cvt_s_w(m.r32(m.r32(tb) + 0xc) as i32)
}

/// `TextBox_Init` 0x1957b0(tb): reset; the box
pub fn init(m: &mut Mem, tb: u32) -> u32 {
    reset(m, tb);
    tb
}

/// `TextBox_Reset` 0x195ee8(tb): no text, no lines, left-aligned at (0, 0) in 100 × 100, line height 9, scale 1,
/// alpha scale 1, the first line 0, no indent or shift, white, the shadow (1, 0, 0, 0) and on
pub fn reset(m: &mut Mem, tb: u32) {
    m.w16(tb + CUT, 0);
    m.w16(tb + TEXT, 0);
    m.w32(tb + 0x20, 0);
    m.w32(tb + 0x24, 0);
    wq(m, tb + 0x3c, [F(1.0); 4]);
    wq(m, tb + 0x5c, [F(1.0), F(0.0), F(0.0), F(0.0)]);
    m.w32(tb + 4, 0);
    m.w32(tb + 8, 0);
    m.w32(tb + 0x6c, 1);
    wf(m, tb + 0x10, F(100.0));
    wf(m, tb + 0xc, F(100.0));
    set_line_height(m, tb, F(9.0));
    wf(m, tb + 0x38, F(1.0));
    m.w32(tb + 0x1c, 0);
    wf(m, tb + 0x18, F(1.0));
    m.w32(tb + 0x28, 0);
    m.w32(tb + 0x34, 0);
    clear_lines(m, tb);
}

/// `TextBox_ClearText` 0x195ff0(tb): no text, no lines
pub fn clear_text(m: &mut Mem, tb: u32) {
    m.w16(tb + CUT, 0);
    m.w16(tb + TEXT, 0);
    clear_lines(m, tb);
}

/// `TextBox_SetFont` 0x196028(tb, font): the font, the line height its height + 3, the text cut again; the
/// lines
pub fn set_font(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, font: u32) -> u32 {
    m.w32(tb, font);
    let h = font_height(m, tb) + F(3.0);
    set_line_height(m, tb, h);
    wrap_lines(m, env, tb, 0)
}

/// `Color_Set4` 0x196070(c; f12..f15): +4..+0x10 the four floats
pub fn color_set4(m: &mut Mem, c: u32, v: [F; 4]) {
    wf(m, c + 0x10, v[3]);
    wf(m, c + 4, v[0]);
    wf(m, c + 8, v[1]);
    wf(m, c + 0xc, v[2]);
}

/// `TextBox_AppendText` 0x196088(tb, s): the wide string after the text (joined by the break string 0x337e58
/// unless the text ends in `\`), or the text when there is none; from the first line, cut again; the lines
pub fn append_text(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, s: u32) -> u32 {
    let text = tb + TEXT;
    if m.r16(text) != 0 {
        let n = env.call(m, WSTR_COUNT, &[text], &[]);
        let last = text.wrapping_add(n.wrapping_sub(1) << 1);
        if m.r16(last) != 0x5c {
            env.call(m, WSTR_APPEND, &[text, BREAK], &[]);
        }
        env.call(m, WSTR_APPEND, &[text, s], &[]);
    } else {
        env.call(m, WSTR_COPY, &[text, s], &[]);
    }
    m.w32(tb + 0x1c, 0);
    wrap_lines(m, env, tb, 0)
}

/// `TextWidget_SetAscii` 0x196128(tb, ascii): the narrow string appended as wide (in a buffer of twice its
/// length from the heap 0x38aaf0, filled with 0xdeadc0ed, freed after); an empty one only cuts the text again;
/// the lines
pub fn set_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, ascii: u32) -> u32 {
    let n = env.call(m, STRLEN, &[ascii], &[]) as i32;
    if n <= 0 {
        return wrap_lines(m, env, tb, 0);
    }
    let size = (n as u32) << 1;
    let buf = env.call(m, ALLOC, &[HEAP, size, 0x10], &[]);
    if buf != 0 {
        env.call(m, MEM_SET, &[buf, 0xdead_c0ed, size], &[]);
    }
    env.call(m, WSTR_SET_ASCII, &[buf, ascii], &[]);
    let lines = append_text(m, env, tb, buf);
    if buf != 0 {
        env.call(m, FREE, &[buf], &[]);
    }
    lines
}

/// `TextBox_SetLineHeight` 0x1961e8(tb, f12)
pub fn set_line_height(m: &mut Mem, tb: u32, h: F) {
    wf(m, tb + 0x14, h);
}

/// `TextBox_SetScale` 0x1961f0(tb, f12 scale, f13 k): the scale, the line height (the font's height + 3) × k
pub fn set_scale(m: &mut Mem, tb: u32, scale: F, k: F) {
    wf(m, tb + 0x18, scale);
    let h = (font_height(m, tb) + F(3.0)) * k;
    set_line_height(m, tb, h);
}

/// `TextBox_WrapLines` 0x196218(tb, again): the text copied to +0x270 and cut into lines no wider (by
/// `Font_MeasureWide` at the scale, plus the indent after the first line) than the box's width as an integer.
/// The text is walked to each space, `\` or its end:
/// - **too wide** (or a `\`): unless the last place cut at is a space (and this no `\`), the line ends here;
///   else it ends at that space, the next starting after it, and is measured again (with `again`, the walk goes
///   back to the next line's start); at the text's end then the rest is a line of its own too;
/// - **it fits**: a space becomes the last place cut at; the end ends the line.
///
/// A line's end is written as a 0; at most 128 lines, from the first line 0. The lines (`line_count`).
pub fn wrap_lines(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, again: u32) -> u32 {
    m.w32(tb + 0x1c, 0);
    clear_lines(m, tb);
    let (cut, lines, widths) = (tb + CUT, tb + LINES, tb + WIDTHS);
    env.call(m, WSTR_COPY, &[cut, tb + TEXT], &[]);
    if m.r16(cut) != 0 {
        let width = cvt_s_w(cvt_w(rf(m, tb + 0xc).0));
        let (mut s0, mut s7, mut s1, mut s2) = (cut, cut, cut, lines);
        let mut n = 0i32;
        loop {
            let c = m.r16(s0);
            let end = c == 0;
            let brk = c == 0x5c;
            let next;
            if c == 0x20 || end || brk {
                let at = widths.wrapping_add((n as u32) << 2);
                m.w16(s0, 0);
                let w = env.call(m, MEASURE, &[m.r32(tb), s7, 0], &[rf(m, tb + 0x18)]);
                m.w32(at, w);
                let indent = if n != 0 { rf(m, tb + 0x28) } else { F(0.0) };
                if width < cvt_s_w(w as i32) + indent || brk {
                    if m.r16(s1) != 0x20 || brk {
                        m.w32(s2, s7);
                        next = s0 + 2;
                        s2 += 4;
                        n += 1;
                        s1 = next;
                        s7 = next;
                    } else {
                        m.w16(s1, 0);
                        let line = n;
                        s1 += 2;
                        s2 += 4;
                        n += 1;
                        let w = env.call(m, MEASURE, &[m.r32(tb), s7, 0], &[rf(m, tb + 0x18)]);
                        m.w32(lines.wrapping_add((line as u32) << 2), s7);
                        m.w16(s0, c);
                        s7 = s1;
                        m.w32(at, w);
                        if again != 0 {
                            s0 = s1;
                        }
                        if end && n < 128 {
                            let w = env.call(m, MEASURE, &[m.r32(tb), s1, 0], &[rf(m, tb + 0x18)]);
                            m.w32(s2, s1);
                            let k = n;
                            s2 += 4;
                            n += 1;
                            m.w32(widths.wrapping_add((k as u32) << 2), w);
                        }
                        next = s0 + 2;
                    }
                } else if !end {
                    m.w16(s0, c);
                    s1 = s0;
                    next = s1 + 2;
                } else {
                    next = s0 + 2;
                    m.w32(lines.wrapping_add((n as u32) << 2), s7);
                    s2 += 4;
                    n += 1;
                }
            } else {
                next = s0 + 2;
            }
            s0 = next;
            if end || n >= 128 {
                break;
            }
        }
    }
    line_count(m, tb)
}

/// `TextBox_LineCount` 0x1964a0(tb): the lines before the first empty slot (at most 128)
pub fn line_count(m: &Mem, tb: u32) -> u32 {
    let mut n = 0u32;
    while n < 128 && m.r32(tb + LINES + 4 * n) != 0 {
        n += 1;
    }
    n
}

/// `TextBox_GetMaxLineWidth` 0x1964d8(tb): the widest of the lines (0 for none)
pub fn max_line_width(m: &Mem, tb: u32) -> u32 {
    let mut best = 0i32;
    let mut k = 0u32;
    while k < 128 && m.r32(tb + LINES + 4 * k) != 0 {
        let w = m.r32(tb + WIDTHS + 4 * k) as i32;
        if best < w {
            best = w;
        }
        k += 1;
    }
    best as u32
}

/// `TextBox_GetHeight` 0x196530(tb): 0 for no lines, else the line height × (lines − 1) as an integer plus the
/// font's height × the scale, as an integer
pub fn height(m: &Mem, tb: u32) -> u32 {
    let n = line_count(m, tb);
    if n == 0 {
        return 0;
    }
    let spread = rf(m, tb + 0x14) * cvt_s_w(n as i32 - 1);
    let glyphs = font_height(m, tb) * rf(m, tb + 0x18);
    cvt_w((cvt_s_w(cvt_w(spread.0)) + glyphs).0) as u32
}

/// A line's x: the box's, plus (centred) half the width less half the line's, (right-aligned) the width less
/// the line's, or (after the first line) the indent
fn line_x(m: &Mem, tb: u32, line: i32, width_at: u32) -> F {
    let x = rf(m, tb + 4);
    if m.r32(tb + 0x20) != 0 {
        x + (rf(m, tb + 0xc) * F(0.5) - cvt_s_w(m.r32(width_at) as i32) * F(0.5))
    } else if m.r32(tb + 0x24) != 0 {
        x + (rf(m, tb + 0xc) - cvt_s_w(m.r32(width_at) as i32))
    } else if line == 0 {
        x
    } else {
        x + rf(m, tb + 0x28)
    }
}

/// `TextBox_Draw` 0x1957d8(tb, count, cursor): the lines from the first line on, each a line height under the
/// last, at most `count` characters in all (−1 for every one). An empty line is skipped. Each line in the
/// colour (`Font_SetColorV` into the drawer 0x337fe8, its first float then × the alpha scale), through a text
/// style on the stack: with `cursor`, `Font_DrawWithCursor` (the style built by `TextStyle_Construct`), else
/// `Font_DrawAligned` (by `TextStyle_Init`), each (style, 0, the line, its characters, the shadow, 1; the scale,
/// x, y), the style then destroyed (2). The first line's place is kept (+0x2c, +0x30, as integers).
pub fn draw(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, count: i32, cursor: u32, scratch: u32) {
    let mut line = m.r32(tb + 0x1c) as i32;
    if line >= 128 {
        return;
    }
    let mut y = rf(m, tb + 8);
    let mut shown = 0i32;
    if m.r32(tb + LINES + ((line as u32) << 2)) == 0 {
        return;
    }
    loop {
        let at = (line as u32) << 2;
        let x = line_x(m, tb, line, tb + WIDTHS + at);
        let s = m.r32(tb + LINES + at);
        if m.r16(s) != 0 {
            let n = env.call(m, WSTR_COUNT, &[s], &[]) as i32;
            let chars;
            if count == -1 {
                shown = shown.wrapping_add(n);
                chars = -1;
            } else {
                let left = count.wrapping_sub(shown);
                chars = if left < n { left } else { n };
                shown = shown.wrapping_add(chars);
            }
            env.call(m, SET_COLOR, &[DRAWER, tb + 0x3c], &[]);
            let a = rf(m, DRAWER) * rf(m, tb + 0x38);
            wf(m, DRAWER, a);
            let font = m.r32(tb);
            let (scale, shadow) = (rf(m, tb + 0x18), m.r32(tb + 0x6c));
            let args = [scratch, 0, s, chars as u32, shadow, 1];
            if cursor != 0 {
                env.call(m, STYLE_CONSTRUCT, &[scratch], &[]);
                env.call(m, STYLE_SET_FONT, &[scratch, font], &[]);
                env.call(m, DRAW_WITH_CURSOR, &args, &[scale, x, y]);
                env.call(m, STYLE_DESTRUCT_THUNK, &[scratch, 2], &[]);
            } else {
                env.call(m, STYLE_INIT, &[scratch], &[]);
                env.call(m, STYLE_SET_FONT, &[scratch, font], &[]);
                env.call(m, DRAW_ALIGNED, &args, &[scale, x, y]);
                env.call(m, STYLE_DESTRUCT, &[scratch, 2], &[]);
            }
            if line == m.r32(tb + 0x1c) as i32 {
                m.w32(tb + 0x2c, cvt_w(x.0) as u32);
                m.w32(tb + 0x30, cvt_w(y.0) as u32);
            }
        }
        let step = rf(m, tb + 0x14);
        line += 1;
        if line >= 128 {
            return;
        }
        y = y + step;
        if m.r32(tb + LINES + ((line as u32) << 2)) == 0 {
            return;
        }
    }
}

/// 0x195a48(tb): the lines as `draw` lays them, with `Font_DrawStringA` (the font, the line; x, y, the scale) and
/// no limit or alpha scale. With the shadow on, each first in the shadow's colour 2 right and down. A line
/// starting `~` is drawn (bar the `~`) in the highlight, which is set to (1, 0, 0, 1) first; others in the colour.
pub fn draw_highlighted(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32) {
    let mut line = m.r32(tb + 0x1c) as i32;
    if line >= 128 {
        return;
    }
    let mut y = rf(m, tb + 8);
    if m.r32(tb + LINES + ((line as u32) << 2)) == 0 {
        return;
    }
    loop {
        let at = (line as u32) << 2;
        let x = line_x(m, tb, line, tb + WIDTHS + at);
        let s = m.r32(tb + LINES + at);
        let c = m.r16(s);
        if c != 0 {
            let lit = c == 0x7e;
            let text = if lit { s + 2 } else { s };
            if m.r32(tb + 0x6c) != 0 {
                env.call(m, SET_COLOR, &[DRAWER, tb + 0x5c], &[]);
                let (font, scale) = (m.r32(tb), rf(m, tb + 0x18));
                env.call(m, DRAW_STRING, &[font, text], &[x + F(2.0), y + F(2.0), scale]);
            }
            if lit {
                wq(m, tb + 0x4c, [F(1.0), F(0.0), F(0.0), F(1.0)]);
                env.call(m, SET_COLOR, &[DRAWER, tb + 0x4c], &[]);
            } else {
                env.call(m, SET_COLOR, &[DRAWER, tb + 0x3c], &[]);
            }
            let (font, scale) = (m.r32(tb), rf(m, tb + 0x18));
            env.call(m, DRAW_STRING, &[font, text], &[x, y, scale]);
            if line == m.r32(tb + 0x1c) as i32 {
                m.w32(tb + 0x2c, cvt_w(x.0) as u32);
                m.w32(tb + 0x30, cvt_w(y.0) as u32);
            }
        }
        let step = rf(m, tb + 0x14);
        line += 1;
        if line >= 128 {
            return;
        }
        y = y + step;
        if m.r32(tb + LINES + ((line as u32) << 2)) == 0 {
            return;
        }
    }
}

/// The text's height for the centring draws: the line height × (lines − 1) plus the font's height
fn block_height(m: &Mem, tb: u32) -> F {
    let n = line_count(m, tb);
    rf(m, tb + 0x14) * cvt_s_w(n as i32 - 1) + font_height(m, tb)
}

/// `TextBox_DrawCentered` 0x195c88(tb, count, cursor): `draw` with the text moved down to the box's middle when
/// shorter than it, and (left-aligned) right to its middle when narrower; the place put back after. With no
/// lines only `draw`.
pub fn draw_centered(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, count: i32, cursor: u32, scratch: u32) {
    if line_count(m, tb) == 0 {
        return draw(m, env, tb, count, cursor, scratch);
    }
    let (y, x) = (rf(m, tb + 8), rf(m, tb + 4));
    let h = block_height(m, tb);
    let box_h = rf(m, tb + 0x10);
    if h < box_h {
        let moved = rf(m, tb + 8) + (box_h - h) * F(0.5);
        wf(m, tb + 8, moved);
    }
    let w = cvt_s_w(max_line_width(m, tb) as i32);
    if m.r32(tb + 0x20) == 0 && m.r32(tb + 0x24) == 0 {
        let box_w = rf(m, tb + 0xc);
        if w < box_w {
            let moved = rf(m, tb + 4) + (box_w - w) * F(0.5);
            wf(m, tb + 4, moved);
        }
    }
    draw(m, env, tb, count, cursor, scratch);
    wf(m, tb + 4, x);
    wf(m, tb + 8, y);
}

/// `TextBox_DrawVCentered` 0x195de0(tb, count, cursor): `draw` with the text moved down to the box's middle when
/// shorter than it, and right by the shift (+0x34); the place put back after (only x with no lines)
pub fn draw_vcentered(m: &mut Mem, env: &mut dyn TextBoxEnv, tb: u32, count: i32, cursor: u32, scratch: u32) {
    if line_count(m, tb) == 0 {
        let x = rf(m, tb + 4);
        wf(m, tb + 4, x + rf(m, tb + 0x34));
        draw(m, env, tb, count, cursor, scratch);
        wf(m, tb + 4, x);
        return;
    }
    let (y, x) = (rf(m, tb + 8), rf(m, tb + 4));
    let h = block_height(m, tb);
    let box_h = rf(m, tb + 0x10);
    if h < box_h {
        let moved = rf(m, tb + 8) + (box_h - h) * F(0.5);
        wf(m, tb + 8, moved);
    }
    let shifted = rf(m, tb + 4) + rf(m, tb + 0x34);
    wf(m, tb + 4, shifted);
    draw(m, env, tb, count, cursor, scratch);
    wf(m, tb + 4, x);
    wf(m, tb + 8, y);
}

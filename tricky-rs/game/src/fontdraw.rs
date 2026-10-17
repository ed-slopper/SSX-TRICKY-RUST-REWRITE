//! The font's draws and set-up and the wide strings (board row G7o): `Font_Init` / `_Construct` / `_Delete` /
//! `_Free` 0x19c788..0x19c90c, the centred, right-aligned and formatted draws 0x19d210..0x19d9dc, the glyphs'
//! widths 0x19d9e0 / 0x19da08, the text styles and `Font_DrawAligned` / `_DrawAlignedAscii` /
//! `_DrawWithCursor` (and its narrow form 0x19e3b8) 0x19dec0..0x19e698, and the wide strings `WStr_*`
//! 0x17cc48..0x17ce30. On the bytes at their addresses ([`Mem`]); the glyph drawing, measuring and formatting
//! are calls out ([`TextBoxEnv`]). Checked in the function runner (`tests/fontdraw.rs`).
//!
//! A text style (`TextStyle_*`) is two words: +0 the font, +4 whether a typed character sounds. What the draws
//! build on the game's stack (the copies of their strings, a measure's record, a format's arguments) the port
//! builds in `scratch` (STANDIN; the sizes at each function).

use crate::animseq::cvt_s_w;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::{TextBoxEnv, DRAWER, DRAW_STRING, MEASURE};

pub const DELETE: u32 = 0x114f80;
pub const MEM_FREE: u32 = 0x14a040;
pub const PARSE_SFN: u32 = 0x19c858;
pub const FIND_GLYPH: u32 = 0x19cc10;
pub const DRAW_RAW: u32 = 0x19cf70;
pub const SHADOWED: u32 = 0x19d568;
pub const SHADOWED_CENTRED: u32 = 0x19d770;
pub const SHADOWED_RIGHT: u32 = 0x19d868;
pub const MEASURE_ASCII: u32 = 0x19da30;
pub const FILLED_RECT: u32 = 0x19b108;
pub const TYPE_SOUND: u32 = 0x214dd0;
pub const VSPRINTF: u32 = 0x301ca0;
pub const VSWPRINTF: u32 = 0x2c48e8;
/// The cursor's blink: the frames left (from 70) and whether it shows
pub const BLINK: u32 = 0x338000;
pub const BLINK_ON: u32 = 0x337ffc;
/// The typed character's sound (its argument)
pub const SOUND: u32 = 0x343050;
/// The byte the narrow copy of a typed line starts as
pub const NARROW_START: u32 = 0x38ca18;
const APP: u32 = 0x337c58;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}

/// `WStr_Count` 0x17cc48(s): the characters before the 0
pub fn wstr_count(m: &Mem, s: u32) -> u32 {
    let mut n = 0u32;
    while m.r16(s + 2 * n) != 0 {
        n += 1;
    }
    n
}
/// `WStr_Copy` 0x17cc80(d, s): `d`
pub fn wstr_copy(m: &mut Mem, d: u32, s: u32) -> u32 {
    let (mut d1, mut s1) = (d, s);
    loop {
        let c = m.r16(s1);
        if c == 0 {
            break;
        }
        m.w16(d1, c);
        d1 += 2;
        s1 += 2;
    }
    m.w16(d1, 0);
    d
}
/// `WStr_SetAscii` 0x17ccb8(d, ascii): each byte sign-extended; `d`
pub fn wstr_set_ascii(m: &mut Mem, d: u32, a: u32) -> u32 {
    let (mut d1, mut a1) = (d, a);
    while m.r8(a1) != 0 {
        m.w16(d1, m.r8(a1) as i8 as i16 as u16);
        d1 += 2;
        a1 += 1;
    }
    m.w16(d1, 0);
    d
}
/// `WStr_ToAscii` 0x17ccf8(d, s): each character's low byte; `d`
pub fn wstr_to_ascii(m: &mut Mem, d: u32, s: u32) -> u32 {
    let (mut d1, mut s1) = (d, s);
    while m.r16(s1) != 0 {
        m.w8(d1, m.r8(s1));
        d1 += 1;
        s1 += 2;
    }
    m.w8(d1, 0);
    d
}
/// `WStr_CopyN` 0x17cd30(d, s, n): at most `n` characters; `d`
pub fn wstr_copy_n(m: &mut Mem, d: u32, s: u32, n: i32) -> u32 {
    let (mut d1, mut s1, mut n) = (d, s, n);
    while m.r16(s1) != 0 && n > 0 {
        m.w16(d1, m.r16(s1));
        d1 += 2;
        s1 += 2;
        n -= 1;
    }
    m.w16(d1, 0);
    d
}
/// 0x17cd70(d, ascii, n): as `WStr_SetAscii`, at most `n` characters; `d`
pub fn wstr_set_ascii_n(m: &mut Mem, d: u32, a: u32, n: i32) -> u32 {
    let (mut d1, mut a1, mut n) = (d, a, n);
    while m.r8(a1) != 0 && n > 0 {
        m.w16(d1, m.r8(a1) as i8 as i16 as u16);
        a1 += 1;
        n -= 1;
        d1 += 2;
    }
    m.w16(d1, 0);
    d
}
/// `WStr_ToAsciiN` 0x17cdb8(d, s, n): as `WStr_ToAscii`, at most `n` characters; `d`
pub fn wstr_to_ascii_n(m: &mut Mem, d: u32, s: u32, n: i32) -> u32 {
    let (mut d1, mut s1, mut n) = (d, s, n);
    while m.r16(s1) != 0 && n > 0 {
        m.w8(d1, m.r8(s1));
        n -= 1;
        s1 += 2;
        d1 += 1;
    }
    m.w8(d1, 0);
    d
}
/// `WStr_Append` 0x17ce00(d, s): `s` copied to `d`'s end; that end (not `d`)
pub fn wstr_append(m: &mut Mem, d: u32, s: u32) -> u32 {
    let mut end = d;
    while m.r16(end) != 0 {
        end += 2;
    }
    wstr_copy(m, end, s)
}

/// `TextStyle_Init` 0x19dec0(s): +4 cleared; `s`. (`TextStyle_Construct` 0x19e388 only calls it.)
pub fn style_init(m: &mut Mem, s: u32) -> u32 {
    m.w32(s + 4, 0);
    s
}
/// `TextStyle_SetFont` 0x19e380(s, font)
pub fn style_set_font(m: &mut Mem, s: u32, font: u32) {
    m.w32(s, font);
}
/// `TextStyle_Destruct` 0x19ded0(s, flags): deleted (0x114f80) with flag 1
pub fn style_destruct(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, flags: u32) {
    if flags & 1 != 0 {
        env.call(m, DELETE, &[s], &[]);
    }
}

fn font_fields(m: &mut Mem, f: u32) {
    m.w32(f + 8, 0);
    m.w32(f + 0x20, 0);
    m.w32(f + 0x24, 0);
    m.w32(f + 0x38, 0xffff_ffff);
    wf(m, f + 0x1c, F(1.0));
    wf(m, f + 0x18, F(1.0));
}
/// `Font_Init` 0x19c788(f, sfn): +0 the font file, no texture (+8), scales 1 (+0x18, +0x1c), +0x20, +0x24 0,
/// +0x38 −1; `f`
pub fn font_init(m: &mut Mem, f: u32, sfn: u32) -> u32 {
    font_fields(m, f);
    m.w32(f, sfn);
    f
}
/// `Font_Construct` 0x19c7b8(f, sfn): the same fields, then `Font_ParseSfn`(f, sfn, 0); `f`
pub fn font_construct(m: &mut Mem, env: &mut dyn TextBoxEnv, f: u32, sfn: u32) -> u32 {
    font_fields(m, f);
    env.call(m, PARSE_SFN, &[f, sfn, 0], &[]);
    f
}
/// `Font_Free` 0x19c8d0(f): its texture (+8) freed when there is one; +4 and +8 cleared
pub fn font_free(m: &mut Mem, env: &mut dyn TextBoxEnv, f: u32) {
    let t = m.r32(f + 8);
    if t != 0 {
        env.call(m, MEM_FREE, &[t], &[]);
    }
    m.w32(f + 4, 0);
    m.w32(f + 8, 0);
}
/// `Font_Delete` 0x19c808(f, flags): freed, and deleted with flag 1
pub fn font_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, f: u32, flags: u32) {
    font_free(m, env, f);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[f], &[]);
    }
}

/// `Font_GetGlyphWidth` 0x19d9e0(f, c): the glyph of `c`'s low byte's +8 (signed), 0 for none
pub fn glyph_width(m: &mut Mem, env: &mut dyn TextBoxEnv, f: u32, c: u32) -> u32 {
    let g = env.call(m, FIND_GLYPH, &[f, c & 0xff], &[]);
    if g == 0 { 0 } else { m.r8(g + 8) as i8 as i32 as u32 }
}
/// `Font_GlyphAdvance` 0x19da08(f, c): as `glyph_width` for the low 16 bits
pub fn glyph_advance(m: &mut Mem, env: &mut dyn TextBoxEnv, f: u32, c: u32) -> u32 {
    let g = env.call(m, FIND_GLYPH, &[f, c & 0xffff], &[]);
    if g == 0 { 0 } else { m.r8(g + 8) as i8 as i32 as u32 }
}

/// `Font_DrawCentered` 0x19d278(font, s; x, y, scale): measured into a record (`scratch`, 0x10 bytes; the width
/// at +8), drawn (`Font_DrawStringA`) half that left of x
pub fn draw_centered(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, s: u32, x: F, y: F, scale: F, scratch: u32) {
    env.call(m, MEASURE, &[font, s, scratch], &[scale]);
    let w = cvt_s_w(m.r32(scratch + 8) as i32);
    env.call(m, DRAW_STRING, &[font, s], &[x - w * F(0.5), y, scale]);
}
/// `Font_DrawCenteredAscii` 0x19d210: the narrow string made wide (`scratch`, 0x190 bytes) and centred
pub fn draw_centered_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, a: u32, x: F, y: F, scale: F, scratch: u32) {
    wstr_set_ascii(m, scratch, a);
    draw_centered(m, env, font, scratch, x, y, scale, scratch + 0x190);
}
/// `Font_DrawRightAligned` 0x19d370(font, s; x, y, scale): drawn its measured width left of x
pub fn draw_right(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, s: u32, x: F, y: F, scale: F) {
    let w = env.call(m, MEASURE, &[font, s, 0], &[scale]) as i32;
    env.call(m, DRAW_STRING, &[font, s], &[x - cvt_s_w(w), y, scale]);
}
/// `Font_DrawRightAscii` 0x19d308: the narrow string made wide (`scratch`, 0x190 bytes) and right-aligned
pub fn draw_right_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, a: u32, x: F, y: F, scale: F, scratch: u32) {
    wstr_set_ascii(m, scratch, a);
    draw_right(m, env, font, scratch, x, y, scale);
}

/// The variable arguments a2..t3 as the game spills them (doublewords) at `at`
fn spill(m: &mut Mem, at: u32, va: [u64; 6]) {
    for (k, v) in va.into_iter().enumerate() {
        m.w32(at + 8 * k as u32, v as u32);
        m.w32(at + 8 * k as u32 + 4, (v >> 32) as u32);
    }
}
/// `Font_DrawfRaw` 0x19d8e0(font, format, …; scale, x, y): formatted (`vsprintf` 0x301ca0, into `scratch`, 0x800
/// bytes; the arguments spilled after it) and drawn narrow (`Font_DrawStringRaw` 0x19cf70: x, y, scale)
pub fn drawf_raw(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, fmt: u32, va: [u64; 6], scale: F, x: F, y: F, scratch: u32) {
    let args = scratch + 0x800;
    spill(m, args, va);
    env.call(m, VSPRINTF, &[scratch, fmt, args], &[]);
    env.call(m, DRAW_RAW, &[font, scratch], &[x, y, scale]);
}
/// `Font_DrawfWide` 0x19d960: as `drawf_raw`, wide (0x2c48e8 into 0x1000 bytes, `Font_DrawStringA`)
pub fn drawf_wide(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, fmt: u32, va: [u64; 6], scale: F, x: F, y: F, scratch: u32) {
    let args = scratch + 0x1000;
    spill(m, args, va);
    env.call(m, VSWPRINTF, &[scratch, fmt, args], &[]);
    env.call(m, DRAW_STRING, &[font, scratch], &[x, y, scale]);
}

/// The draw `Font_DrawAligned` picks: left (0) and centred (1) are `Font_DrawStringA` unless shadowed (the
/// shadowed draw 0x19d568 / the shadowed centred one 0x19d770), right (2) `Font_DrawRightAligned` or the shadowed
/// right one 0x19d868; any other nothing
fn aligned(m: &mut Mem, env: &mut dyn TextBoxEnv, font: u32, align: u32, shadow: u32, s: u32, scale: F, x: F, y: F) {
    match (align, shadow != 0) {
        (0, true) => {
            env.call(m, SHADOWED, &[font, s], &[scale, x, y]);
        }
        (1, true) => {
            env.call(m, SHADOWED_CENTRED, &[font, s], &[scale, x, y]);
        }
        (2, true) => {
            env.call(m, SHADOWED_RIGHT, &[font, s], &[scale, x, y]);
        }
        (0 | 1, false) => {
            env.call(m, DRAW_STRING, &[font, s], &[x, y, scale]);
        }
        (2, false) => draw_right(m, env, font, s, x, y, scale),
        _ => {}
    }
}

/// `Font_DrawAligned` 0x19df90(style, align, s, count, shadow; scale, x, y): with `count` −1 the string drawn
/// as aligned. Else its first `count` characters (a copy in `scratch`, 0x160 bytes) drawn, then the next one
/// (or none: at `scratch` + 0x160) at half the drawer's alpha, after the first part's measured width (and 6 ×
/// the scale more when that part ends in a space), measured too; the alpha then put back as it was.
pub fn draw_aligned(m: &mut Mem, env: &mut dyn TextBoxEnv, style: u32, align: u32, s: u32, count: i32, shadow: u32, scale: F, x: F, y: F, scratch: u32) {
    let alpha = rf(m, DRAWER);
    if count == -1 {
        let font = m.r32(style);
        aligned(m, env, font, align, shadow, s, scale, x, y);
        return;
    }
    let n = wstr_count(m, s) as i32;
    let next = scratch + 0x160;
    m.w16(next, 0);
    wstr_copy_n(m, scratch, s, count);
    if count > 0 && count < n {
        m.w16(next + 2, 0);
        m.w16(next, m.r16(s.wrapping_add((count as u32) << 1)));
    }
    let font = m.r32(style);
    aligned(m, env, font, align, shadow, scratch, scale, x, y);
    let font = m.r32(style);
    let mut w = cvt_s_w(env.call(m, MEASURE, &[font, scratch, 0], &[scale]) as i32);
    if count > 0 && m.r16(scratch.wrapping_add(((count - 1) as u32) << 1)) == 0x20 {
        w = w + scale * F(6.0);
    }
    let half = rf(m, DRAWER) * F(0.5);
    wf(m, DRAWER, half);
    let font = m.r32(style);
    aligned(m, env, font, align, shadow, next, scale, x + w, y);
    let font = m.r32(style);
    env.call(m, MEASURE, &[font, next, 0], &[scale]);
    wf(m, DRAWER, alpha);
}

/// `Font_DrawAlignedAscii` 0x19def0: the narrow string made wide (`scratch`, 0x160 bytes) and drawn aligned
pub fn draw_aligned_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, style: u32, align: u32, a: u32, count: i32, shadow: u32, scale: F, x: F, y: F, scratch: u32) {
    wstr_set_ascii(m, scratch, a);
    draw_aligned(m, env, style, align, scratch, count, shadow, scale, x, y, scratch + 0x160);
}

/// `Font_DrawWithCursor` 0x19e460(style, align, s, count, shadow, cursor; scale, x, y): with `count` −1 the
/// string drawn left-aligned (shadowed or not). Else drawn aligned, then the cursor:
/// - the blink (0x338000) counted down: below 0 it goes back to 70 and the cursor (0x337ffc) flips;
/// - the typed part's narrow copy (`scratch`, 0x100 bytes; its first byte from 0x38ca18 before) measured
///   (`Font_MeasureAscii`), 6 × the scale more after a space when the line goes on;
/// - no texture (the renderer's +0x1f0(−1, 0)); with the cursor showing and wanted, a filled rectangle in the
///   drawer's colour 3 after the typed part, 10 wide and the font's height × the scale high;
/// - the typed character's sound (0x214dd0 with 0x343050's word) while the line goes on and the style's +4 is
///   set.
pub fn draw_with_cursor(m: &mut Mem, env: &mut dyn TextBoxEnv, style: u32, align: u32, s: u32, count: i32, shadow: u32, cursor: u32, scale: F, x: F, y: F, scratch: u32) {
    if count == -1 {
        let font = m.r32(style);
        if shadow != 0 {
            env.call(m, SHADOWED, &[font, s], &[scale, x, y]);
        } else {
            env.call(m, DRAW_STRING, &[font, s], &[x, y, scale]);
        }
        return;
    }
    let n = wstr_count(m, s) as i32;
    m.w8(scratch, m.r8(NARROW_START));
    wstr_to_ascii_n(m, scratch, s, count);
    draw_aligned(m, env, style, align, s, count, shadow, scale, x, y, scratch + 0x100);
    let left = m.r32(BLINK).wrapping_sub(1);
    m.w32(BLINK, left);
    if (left as i32) < 0 {
        m.w32(BLINK, 70);
        let on = m.r32(BLINK_ON) ^ 1;
        m.w32(BLINK_ON, on);
    }
    let font = m.r32(style);
    let mut w = cvt_s_w(env.call(m, MEASURE_ASCII, &[font, scratch, 0], &[scale]) as i32);
    if count > 0 && count < n && m.r8(scratch.wrapping_add(count as u32 - 1)) as i8 == 0x20 {
        w = w + scale * F(6.0);
    }
    let h = cvt_s_w(m.r32(m.r32(style) + 0xc) as i32) * scale;
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + 0x1f0) as i16 as i32 as u32);
    let func = m.r32(vt + 0x1f4);
    env.call(m, func, &[this, 0xffff_ffff, 0], &[]);
    if m.r32(BLINK_ON) != 0 && cursor != 0 {
        env.call(m, FILLED_RECT, &[DRAWER], &[x + w + F(3.0), y, F(10.0), h]);
    }
    if count > 0 && !(n < count) && m.r32(style + 4) != 0 {
        let arg = m.r32(SOUND);
        env.call(m, TYPE_SOUND, &[arg], &[]);
    }
}

/// 0x19e3b8 (`Font_DrawWithCursor`'s narrow form): the narrow string made wide (`scratch`, 0x160 bytes) and drawn
/// with the cursor
pub fn draw_with_cursor_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, style: u32, align: u32, a: u32, count: i32, shadow: u32, cursor: u32, scale: F, x: F, y: F, scratch: u32) {
    wstr_set_ascii(m, scratch, a);
    draw_with_cursor(m, env, style, align, scratch, count, shadow, cursor, scale, x, y, scratch + 0x160);
}

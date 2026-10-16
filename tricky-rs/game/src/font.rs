//! The game's fonts in memory (board row G8k): the font object `Font_ParseGlyphs` 0x19c910 builds from an SFN
//! file, `Font_FindGlyph` 0x19cc10 and `Font_MeasureAscii` 0x19da30 / `Font_MeasureWide` 0x19dc78, on the game's
//! bytes (`pf::Mem`), checked in the function runner (`tests/font.rs`).
//!
//! The font object: +0 the allocation flags, +4 the glyph count, +8 the glyph table, +0xc the line height (the
//! tallest h + y offset), +0x10 / +0x14 the pen's origin, +0x18 / +0x1c the scales (1.0 from `Font_ParseSfn`, the
//! title and menu fonts' own set after), +0x28 the first glyph's code and +0x2c the last of the run of consecutive
//! codes from it. A glyph (12 bytes): +0 the code, +2 w, +3 h, +4 / +6 its place on the texture, +8 the advance, +9
//! the x offset, +10 the y offset.

use crate::pf::Mem;
use crate::ps2f::F;

fn s8(v: u8) -> i32 {
    v as i8 as i32
}

/// `Font_ParseGlyphs` 0x19c910 (the font, the SFN file, an advance added to every glyph, a y added to the origin):
/// the object's fields and its glyph table at `table` (the game allocates it; `Font_BuildTexture` 0x19cca8, the
/// texture, follows and is not part of this). The SFN: +8 the version (records of 11 bytes before 200, 12 from it),
/// +0xa the count, +0x10 / +0x11 the origin, +0x14 the records' offset
pub fn parse_glyphs(m: &mut Mem, font: u32, sfn: &[u8], advance: u8, y_add: i32, table: u32) {
    let b = |o: usize| sfn.get(o).copied().unwrap_or(0);
    let h16 = |o: usize| b(o) as u16 | (b(o + 1) as u16) << 8;
    let records = (h16(0x14) as u32 | (h16(0x16) as u32) << 16) as usize;
    let version = h16(8) as i16;
    m.w32(font + 0x10, s8(b(0x10)) as u32);
    m.w32(font + 0x14, s8(b(0x11)).wrapping_add(y_add) as u32);
    let count = h16(0xa) as i16 as i32;
    m.w32(font + 4, count as u32);
    m.w32(font + 8, table);
    let size = if version < 200 { 11 } else { 12 };
    for i in 0..count.max(0) as usize {
        let (p, t) = (records + size * i, table + 12 * i as u32);
        m.w16(t, h16(p));
        m.w8(t + 2, b(p + 2));
        m.w8(t + 3, b(p + 3));
        m.w16(t + 4, h16(p + 4));
        m.w16(t + 6, h16(p + 6));
        m.w8(t + 8, b(p + 8).wrapping_add(advance));
        m.w8(t + 9, b(p + 9));
        m.w8(t + 10, b(p + 10));
    }
    m.w32(font + 0xc, 0);
    let first = m.r16(table) as u32;
    m.w32(font + 0x2c, first);
    m.w32(font + 0x28, first);
    for i in 0..count.max(0) as u32 {
        let g = table + 12 * i;
        let code = m.r16(g) as u32;
        if i != 0 && code == m.r32(font + 0x2c).wrapping_add(1) {
            m.w32(font + 0x2c, code);
        }
        let line = (m.r8(g + 3) as i32).wrapping_add(s8(m.r8(g + 10)));
        if (m.r32(font + 0xc) as i32) < line {
            m.w32(font + 0xc, line as u32);
        }
    }
}

/// `Font_FindGlyph` 0x19cc10 (the font, the code): the glyph, or 0. Codes in the run from the first are looked up
/// directly, the rest by a binary search over the table past it
pub fn find_glyph(m: &Mem, font: u32, code: u32) -> u32 {
    let first = m.r32(font + 0x28) as i32;
    let code = code as i32;
    if code < first {
        return 0;
    }
    let table = m.r32(font + 8);
    if code <= m.r32(font + 0x2c) as i32 {
        return table.wrapping_add((code as u32).wrapping_mul(12)).wrapping_sub((first as u32).wrapping_mul(12));
    }
    let count = m.r32(font + 4) as i32;
    let mut lo = (m.r32(font + 0x2c) as i32).wrapping_sub(first);
    let mut sum = lo.wrapping_add(count);
    let mut hi = count;
    loop {
        let mut mid = sum >> 1;
        let g = (mid as u32).wrapping_mul(12).wrapping_add(table);
        let c = m.r16(g) as i32;
        if code == c {
            return g;
        }
        if c <= code {
            lo = mid + 1;
            mid = hi;
        }
        sum = lo.wrapping_add(mid);
        hi = mid;
        if lo >= mid {
            return 0;
        }
    }
}

/// `Font_MeasureAscii` 0x19da30 (`wide` false) / `Font_MeasureWide` 0x19dc78 (the scale; the font, the string, the
/// size out or 0): the width. Each glyph's box from the pen (at the origin, moving by its advance) is floored at its
/// left and top and ceiled at its right and bottom, all in the font's scales times the scale, into a box that
/// starts at 0, 0, 0, 0; the size is left, top, width, height
pub fn measure(m: &mut Mem, wide: bool, scale: f32, font: u32, s: u32, out: u32) -> i32 {
    let sx = F(scale) * F(f32::from_bits(m.r32(font + 0x18)));
    let sy = F(scale) * F(f32::from_bits(m.r32(font + 0x1c)));
    let mut pen = F(m.r32(font + 0x10) as i32 as f32) * sx;
    let top = F(m.r32(font + 0x14) as i32 as f32) * sy;
    let mut bx = [0i32; 4];
    let floor = |v: F| {
        let t = F(v.0 as i32 as f32);
        (if v.0 < t.0 { t - F(1.0) } else { t }).0 as i32
    };
    let ceil = |v: F| {
        let t = F(v.0 as i32 as f32);
        (if t.0 < v.0 { t + F(1.0) } else { t }).0 as i32
    };
    let mut at = s;
    loop {
        let c = if wide { m.r16(at) as u32 } else { m.r8(at) as u32 };
        if c == 0 {
            break;
        }
        let g = find_glyph(m, font, c);
        if g != 0 {
            let (w, h, xo, yo) = (m.r8(g + 2) as i32, m.r8(g + 3) as i32, s8(m.r8(g + 9)), s8(m.r8(g + 10)));
            let l = floor(pen + F(xo as f32) * sx);
            bx[0] = if bx[0] < l { bx[0] } else { l };
            let t = floor(top + F((yo + h) as f32) * sy);
            bx[1] = if bx[1] < t { bx[1] } else { t };
            let r = ceil(pen + F((xo + w) as f32) * sx);
            bx[2] = if r < bx[2] { bx[2] } else { r };
            let b = ceil(top + F(yo as f32) * sy);
            bx[3] = if b < bx[3] { bx[3] } else { b };
            pen = pen + F(s8(m.r8(g + 8)) as f32) * sx;
        }
        at += if wide { 2 } else { 1 };
    }
    let w = bx[2].wrapping_sub(bx[0]);
    let h = bx[3].wrapping_sub(bx[1]);
    if out != 0 {
        for (k, v) in [bx[0], bx[1], w, h].into_iter().enumerate() {
            m.w32(out + 4 * k as u32, v as u32);
        }
    }
    w
}

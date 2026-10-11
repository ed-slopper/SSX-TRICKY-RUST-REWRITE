//! Bitmap fonts: `DATA/FONTS/*.SFN` (`FNTS`; board row E5e).
//!
//! Proven from the game:
//! - `cGame::Init` 0x17e148 loads `data/fonts/title.sfn` into `gApp+0x5c` with scale 1.4 x 1.3 and
//!   `data/fonts/menu.sfn` into `gApp+0x60` with 1.8 x 1.4 (`TITLE_SCALE`, `MENU_SCALE`).
//! - `Font_ParseGlyphs` 0x19c910: +8 version, +0xa glyph count, +0x10 / +0x11 the pen's origin (s8), +0x14 the
//!   glyph records (12 bytes from version 200, 11 before: u16 code, u8 w, h, u16 x, y, s8 advance, x offset,
//!   y offset), +0x1c the bitmap block.
//! - `Font_BuildTexture` 0x19cca8: the bitmap block is +4 u16 width, +6 u16 height, 4-bit pixels from +0x10
//!   (low nibble first), copied into a square texture whose side is the larger of the width and the smallest
//!   power of two above the height; its 16 colours are RGB 0x80 with alpha `i * 128 / 15` (the GS's
//!   0-0x80).

pub const TITLE_SCALE: (f32, f32) = (1.4, 1.3);
pub const MENU_SCALE: (f32, f32) = (1.8, 1.4);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SfnGlyph {
    pub code: u16,
    pub w: u8,
    pub h: u8,
    pub x: u16,
    pub y: u16,
    pub advance: i8,
    pub xoff: i8,
    pub yoff: i8,
}

pub struct SfnFont {
    pub version: u16,
    pub origin: (i8, i8),
    pub glyphs: Vec<SfnGlyph>,
    /// The bitmap's size and its 4-bit pixels, one byte each.
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl SfnFont {
    pub fn open(d: &[u8]) -> Result<SfnFont, String> {
        if d.get(..4) != Some(b"FNTS") {
            return Err("not an FNTS font".into());
        }
        let u16_at = |o: usize| d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("short font");
        let u32_at = |o: usize| d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or("short font");
        let version = u16_at(8)?;
        let count = u16_at(0xa)? as usize;
        let origin = (d[0x10] as i8, d[0x11] as i8);
        let records = u32_at(0x14)? as usize;
        let size = if version < 200 { 11 } else { 12 };
        let mut glyphs = Vec::with_capacity(count);
        for i in 0..count {
            let o = records + i * size;
            let r = d.get(o..o + 11).ok_or("short glyph table")?;
            glyphs.push(SfnGlyph {
                code: u16::from_le_bytes([r[0], r[1]]),
                w: r[2],
                h: r[3],
                x: u16::from_le_bytes([r[4], r[5]]),
                y: u16::from_le_bytes([r[6], r[7]]),
                advance: r[8] as i8,
                xoff: r[9] as i8,
                yoff: r[10] as i8,
            });
        }
        let block = u32_at(0x1c)? as usize;
        let width = u16_at(block + 4)? as i16 as usize;
        let height = u16_at(block + 6)? as i16 as usize;
        let mut pixels = Vec::with_capacity(width * height);
        let packed = d.get(block + 0x10..).ok_or("short bitmap")?;
        for y in 0..height {
            for x in 0..width / 2 {
                let b = *packed.get(y * (width / 2) + x).ok_or("short bitmap")?;
                pixels.push(b & 0xf);
                pixels.push(b >> 4);
            }
        }
        Ok(SfnFont { version, origin, glyphs, width, height, pixels })
    }

    /// The texture's side (`Font_BuildTexture`).
    pub fn texture_size(&self) -> usize {
        let mut p = 1;
        while p <= self.height {
            p <<= 1;
        }
        self.width.max(p)
    }

    /// The bitmap as RGBA in the full 0-255 range (colours 0x80 and alpha `i * 128 / 15`, each doubled
    /// and capped at 255, as `ssh::Image::rgba_full_range`): white text with the font's coverage as alpha.
    pub fn rgba_full_range(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &v in &self.pixels {
            let a = ((v as u32) << 7) / 15;
            out.extend([255, 255, 255, (a * 2).min(255) as u8]);
        }
        out
    }

    /// The tallest glyph's bottom (`h + yoff`), the line height the HUD anchors with.
    pub fn line_height(&self) -> i32 {
        self.glyphs.iter().map(|g| g.h as i32 + g.yoff as i32).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the player's own image and the exported fonts: the same glyphs as `chars/fonts/*.json`.
    #[test]
    fn fonts_match_the_export() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chars/fonts");
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        for name in ["title", "menu"] {
            let font = SfnFont::open(&files.load(&format!("data/fonts/{name}.sfn")).unwrap().unwrap()).unwrap();
            let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.json"))) else { continue };
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["line_height"].as_i64().unwrap() as i32, font.line_height(), "{name}");
            let glyphs = v["glyphs"].as_object().unwrap();
            assert_eq!(glyphs.len(), font.glyphs.len(), "{name}");
            for g in &font.glyphs {
                let e = &glyphs[&g.code.to_string()];
                let n = |k: &str| e[k].as_i64().unwrap();
                assert_eq!((n("x"), n("y"), n("w"), n("h"), n("xoff"), n("yoff"), n("advance")),
                    (g.x as i64, g.y as i64, g.w as i64, g.h as i64, g.xoff as i64, g.yoff as i64, g.advance as i64), "{name} {}", g.code);
            }
        }
    }
}

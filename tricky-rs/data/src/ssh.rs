//! Textures: EA's `SHPS` shape files (`.ssh`; board row E3b).
//!
//! Proven from the game's shape library (`lib-shape`): the directory (`Shape_GetCount` 0x2c2660, `Shape_GetEntry`
//! 0x2c266c: +8 count, from +0x10 one (4-character name, offset) pair per shape), the block chain (each block's
//! first word: the type in the low byte, the offset to the next block of the same shape in the other 24 bits,
//! 0 for the last; `Shape_GetLongName` 0x2c2610 walks it for type 0x70 'p', whose name starts at +4), and the
//! name lookup (`Shape_FindByName` 0x2c2528: the long name if the shape has one, else the 4-character name,
//! compared with `strcmp`). Packed blocks (type bit 0x80, unpacked with the compression codecs by 0x2c2318)
//! and the converted types 0x60-0x62 do not occur on the disc and are not read.
//!
//! The pixels go to the GS as they are (`TexMan_UploadTexture` 0x1c3ba8 uploads PSMT8 / PSMCT32 directly), so
//! the formats are the GS's (inferred from the data, as every texture on the disc decodes cleanly): type 2 is
//! one byte per pixel into a 256-colour palette (block 0x21: RGBA, 8 bits each, stored in order), type 5 is
//! RGBA. Colours and alpha are the GS's: stored as 0-0x80 (0x80 = full; see [`Image::rgba_full_range`]).

/// One shape's image.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// RGBA as stored (GS: 0x80 = 1.0 for colour and alpha).
    pub rgba: Vec<u8>,
}

impl Image {
    /// The image in the full 0-255 range, the way the textures exported before were made (colour
    /// `c * 255 / 128` rounded down, alpha doubled, both capped at 255; proven equal to those exports).
    /// STANDIN: for the GS's texture function, which modulates the stored 0-0x80 colours by the draw's own
    /// colour (the renderer's rows): the stored values are kept in `rgba`.
    pub fn rgba_full_range(&self) -> Vec<u8> {
        let mut out = self.rgba.clone();
        for (i, c) in out.iter_mut().enumerate() {
            *c = if i % 4 == 3 { (*c as u16 * 2).min(255) as u8 } else { (*c as u32 * 255 / 128).min(255) as u8 };
        }
        out
    }
}

/// A shape: its directory name, its long name (block 0x70) if it has one, and its image.
#[derive(Clone, Debug)]
pub struct Shape {
    pub short: String,
    pub long: Option<String>,
    pub image: Option<Image>,
}

impl Shape {
    /// The name `Shape_FindByName` compares: the long one if there is one.
    pub fn name(&self) -> &str { self.long.as_deref().unwrap_or(&self.short) }
}

/// An `.ssh` file: `SHPS`, total size, count, version (`G277`, ...), then the shapes.
pub struct SshFile {
    pub version: String,
    pub shapes: Vec<Shape>,
}

fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).into_owned()
}

impl SshFile {
    pub fn open(d: &[u8]) -> Result<SshFile, String> {
        let u32_at = |o: usize| d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| format!("short file at {o:#x}"));
        let u16_at = |o: usize| d.get(o..o + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap()) as usize).ok_or_else(|| format!("short file at {o:#x}"));
        if d.get(..4) != Some(b"SHPS") {
            return Err("not an SHPS file".into());
        }
        let count = u32_at(8)? as usize;
        let version = cstr(&d[12..16]);
        let mut shapes = Vec::with_capacity(count);
        for i in 0..count {
            let short = cstr(d.get(0x10 + i * 8..0x14 + i * 8).ok_or("short directory")?);
            let first = u32_at(0x14 + i * 8)? as usize;
            let (mut long, mut image, mut palette) = (None, None, None::<&[u8]>);
            let mut pixels: Option<(u8, usize, usize, &[u8])> = None;
            let mut o = first;
            loop {
                let w = u32_at(o)?;
                let kind = (w & 0xff) as u8;
                let next = (w >> 8) as usize;
                let end = if next == 0 { d.len() } else { o + next };
                let body = d.get(o + 0x10..end.min(d.len())).unwrap_or(&[]);
                match kind {
                    0x02 | 0x05 => pixels = Some((kind, u16_at(o + 4)?, u16_at(o + 6)?, body)),
                    0x21 => palette = Some(body),
                    0x70 => long = Some(cstr(d.get(o + 4..end.min(d.len())).unwrap_or(&[]))),
                    _ => {}
                }
                if next == 0 {
                    break;
                }
                o += next;
            }
            if let Some((kind, w, h, px)) = pixels {
                let mut rgba = Vec::with_capacity(w * h * 4);
                match kind {
                    2 => {
                        let pal = palette.ok_or_else(|| format!("{short}: 8-bit image without a palette"))?;
                        for &ix in px.get(..w * h).ok_or_else(|| format!("{short}: short pixels"))? {
                            rgba.extend_from_slice(pal.get(ix as usize * 4..ix as usize * 4 + 4).ok_or_else(|| format!("{short}: index {ix} past the palette"))?);
                        }
                    }
                    _ => rgba.extend_from_slice(px.get(..w * h * 4).ok_or_else(|| format!("{short}: short pixels"))?),
                }
                image = Some(Image { width: w, height: h, rgba });
            }
            shapes.push(Shape { short, long, image });
        }
        Ok(SshFile { version, shapes })
    }

    /// `Shape_FindByName` 0x2c2528.
    pub fn find(&self, name: &str) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the player's own image: every `.ssh` of the character archive decodes.
    #[test]
    fn character_textures_decode() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let big = files.bigs.iter().find(|b| b.path == "data/char/texps2.big").unwrap();
        let names: Vec<String> = crate::big::entries(&big.header).into_iter().map(|e| e.name).collect();
        let mut images = 0;
        for n in names {
            let bytes = files.load(&format!("data/char/texps2.big|{n}")).unwrap().unwrap();
            let ssh = SshFile::open(&bytes).unwrap_or_else(|e| panic!("{n}: {e}"));
            images += ssh.shapes.iter().filter(|s| s.image.is_some()).count();
        }
        assert!(images > 300);
    }
}

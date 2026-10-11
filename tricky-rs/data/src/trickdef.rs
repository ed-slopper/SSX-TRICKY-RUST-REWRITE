//! The trick book: `DATA/TUTORIAL/TRICKDEF.DAT` (board row E5d).
//!
//! Proven: `TrickDef_Load` 0x154580 reads `data/tutorial/trickdef.dat` whole into 0x2760 bytes, 12 riders of
//! 0x348 (30 tricks of 28 bytes: six chapters of five), and a trick's name is `Score_FormatTrickName`
//! 0x1551c0: fourteen strings printed with `%s` from the pointer tables at 0x320970 (`trick_names`), picked by
//! bit fields of the record's two words. The other fields' meanings come from the tool that decoded the file
//! before (`tools/trickbook`) and are not yet checked against the book's code (`TrickBook_*`).

use crate::trick_names as t;

/// One trick of the book.
#[derive(Clone, Copy, Debug)]
pub struct TrickDef {
    /// The trick id and its flags, as `Score_FormatTrickName` reads them.
    pub words: [u32; 2],
    /// Flips (whole turns) and spin (half turns), signed.
    pub flip: i16,
    pub spin: i16,
    pub rail: i16,
    pub plus: [i16; 2],
    pub slot: [u32; 2],
}

/// The table word at `base + 4 * index`, as a string (`(null)` for a null pointer, as the C library prints
/// one).
fn word(base: u32, index: u32) -> &'static str {
    let i = ((base - t::BASE) / 4 + index) as usize;
    t::WORDS.get(i).copied().flatten().unwrap_or("(null)")
}

impl TrickDef {
    /// `Score_FormatTrickName` 0x1551c0.
    pub fn name(&self) -> String {
        let [w0, w1] = self.words;
        [
            word(t::RAIL_TO, (w1 >> 12) & 0xf),
            word(t::RAIL_SPIN, (w1 >> 4) & 0xf),
            word(t::STANCE, (w1 >> 16) & 0xf),
            word(t::SPIN_DIR, (w1 >> 27) & 7),
            word(t::SPIN, (w0 >> 24) & 0xf),
            word(t::FLIP_COUNT, (w1 >> 24) & 7),
            word(t::FLIP_KIND, (w1 >> 8) & 0xf),
            word(t::COMBO, w0 & 0xff),
            word(t::SPIN_AFTER, w0 >> 28),
            word(t::GRAB, (w0 >> 8) & 0xff),
            word(t::LATE, w1 >> 30),
            word(t::GRAB, (w0 >> 16) & 0xff),
            word(t::ENDING, (w1 >> 20) & 0xf),
            word(t::CRASH, w1 & 0xf),
        ]
        .concat()
    }
}

/// The book: 12 riders (the game's character order: Eddie, Kaori, Luther, Mac, Moby, Zoe, JP, Elise,
/// Psymon, Seeiah, Brodi, Marisol) of 30 tricks.
pub fn read(bytes: &[u8]) -> Result<Vec<Vec<TrickDef>>, String> {
    if bytes.len() < 12 * 0x348 {
        return Err(format!("trickdef.dat: {} bytes, not {}", bytes.len(), 12 * 0x348));
    }
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let i16_at = |o: usize| i16::from_le_bytes(bytes[o..o + 2].try_into().unwrap());
    Ok((0..12)
        .map(|r| {
            (0..30)
                .map(|k| {
                    let o = r * 0x348 + k * 28;
                    TrickDef {
                        words: [u32_at(o), u32_at(o + 4)],
                        flip: i16_at(o + 8),
                        spin: i16_at(o + 10),
                        rail: i16_at(o + 12),
                        plus: [i16_at(o + 16), i16_at(o + 18)],
                        slot: [u32_at(o + 20), u32_at(o + 24)],
                    }
                })
                .collect()
        })
        .collect())
}

/// The riders' names in the book's order (for our folder names).
pub const RIDERS: [&str; 12] = ["eddie", "kaori", "luther", "mac", "moby", "zoe", "jp", "elise", "psymon", "seeiah", "brodi", "marisol"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the player's own image and the exported `chars/trickbook.json`: the same 360 names.
    #[test]
    fn names_match_the_export() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let json = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chars/trickbook.json");
        let Ok(text) = std::fs::read_to_string(&json) else { return };
        let v: std::collections::HashMap<String, Vec<serde_json::Value>> = serde_json::from_str(&text).unwrap();
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let book = read(&files.load("data/tutorial/trickdef.dat").unwrap().unwrap()).unwrap();
        let mut n = 0;
        for (set, list) in v {
            let r: usize = set[3..set.find('_').unwrap()].parse().unwrap();
            for (k, e) in list.iter().enumerate() {
                assert_eq!(book[r][k].name().trim(), e["name"].as_str().unwrap(), "{set} {k}");
                n += 1;
            }
        }
        assert_eq!(n, 360);
    }
}

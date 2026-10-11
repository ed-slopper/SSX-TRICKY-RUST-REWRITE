//! Text: the `.loc` string tables of `DATA/LANG` (board row E5a), read as `Loc_GetString` 0x2c5160 reads them.
//!
//! `Loc_LoadForRegion` 0x17c150 loads `data/lang/american.loc` (region 0) or `data/lang/british.loc` (region 1,
//! not on the NTSC disc) whole. All fields are little-endian:
//! - +8 flags; with bit 0, +4 is the offset of an id map that turns a string id into an index first
//!   (`Loc_MapId` 0x2c50b0: `bsearch` over +8 count (key u16, value u16) pairs from +0x10, comparing
//!   `key - entry` with 0x2c5070);
//! - +0xe u16: which string block, its offset the u32 at +0x10 + 4 * that;
//! - a block: +0xc u32 string count, from +0x10 one u32 offset per string (from the block's start); a string
//!   is 16-bit characters ending with 0. An index past the count gives no string.

pub struct LocFile {
    bytes: Vec<u8>,
}

impl LocFile {
    pub fn new(bytes: Vec<u8>) -> LocFile { LocFile { bytes } }

    fn u32_at(&self, o: usize) -> Option<u32> { self.bytes.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) }
    fn u16_at(&self, o: usize) -> Option<u16> { self.bytes.get(o..o + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap())) }

    /// `Loc_MapId` 0x2c50b0.
    fn map_id(&self, id: u16, map: usize) -> i32 {
        let Some(count) = self.u32_at(map + 8) else { return -1 };
        let (mut lo, mut hi) = (0usize, count as usize);
        while lo < hi {
            let mid = (lo + hi) >> 1;
            let Some(key) = self.u16_at(map + 0x10 + mid * 4) else { return -1 };
            let r = id as i32 - key as i32;
            if r < 0 {
                hi = mid;
            } else if r == 0 {
                return self.u16_at(map + 0x10 + mid * 4 + 2).map_or(-1, |v| v as i32);
            } else {
                lo = mid + 1;
            }
        }
        -1
    }

    /// The string `id` as stored (16-bit characters, without the ending 0): `Loc_GetString` 0x2c5160.
    pub fn get_raw(&self, id: u32) -> Option<Vec<u16>> {
        let flags = self.u32_at(8)?;
        let mut index = id as i32;
        if flags & 1 != 0 {
            index = self.map_id(id as u16, self.u32_at(4)? as usize);
        }
        let block = self.u32_at(0x10 + self.u16_at(0xe)? as usize * 4)? as usize;
        if index < 0 || index as u32 >= self.u32_at(block + 0xc)? {
            return None;
        }
        let mut at = block + self.u32_at(block + 0x10 + index as usize * 4)? as usize;
        let mut out = Vec::new();
        loop {
            let c = self.u16_at(at)?;
            if c == 0 {
                return Some(out);
            }
            out.push(c);
            at += 2;
        }
    }

    /// The string as text (the characters read as Unicode code points).
    pub fn get(&self, id: u32) -> Option<String> {
        self.get_raw(id).map(|s| s.iter().map(|&c| char::from_u32(c as u32).unwrap_or('\u{fffd}')).collect())
    }

    /// The number of strings in the block the file selects.
    pub fn count(&self) -> usize {
        (|| {
            let block = self.u32_at(0x10 + self.u16_at(0xe)? as usize * 4)? as usize;
            self.u32_at(block + 0xc)
        })().unwrap_or(0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the player's own image: the strings the port notes name are where the game asks for them.
    #[test]
    fn american_strings() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let loc = LocFile::new(files.load("data/lang/american.loc").unwrap().unwrap());
        assert!(loc.count() > 100);
        let all: Vec<String> = (0..loc.count() as u32).filter_map(|i| loc.get(i)).collect();
        assert!(all.iter().any(|s| s.contains("BIG AIR")), "no BIG AIR text");
    }
}

//! Animation clips: the `.afl` files of `DATA/CHAR/ANM.BIG` (board row E4b). Format notes in
//! `notes/animation-format.md`.
//!
//! Proven from the game: the file header as `AnimFile_Bind` 0x158c10 reads it (+2 u16 header count, +4
//! pointer table, +8 data, headers from +0xc), the frame count at a clip header's +10 (`AnimClip_Advance`
//! 0x15a030), a clip's group headers following it (+9 count, `SkelAnim_Create` per group in 0x158dc0), the
//! curve evaluators 0x157ed8 / 0x158030, and the clip names (`Name_ElfHash` 0x240e80 of the names at 0x3242a0).

use crate::anim_names;

/// `Name_ElfHash` 0x240e80: `h = h*16 + c`, the top nibble folded back with a shift of 23 (not ELF's 24).
pub fn name_hash(s: &str) -> u32 {
    let mut h: u32 = 0;
    for &c in s.as_bytes() {
        h = h.wrapping_mul(16).wrapping_add(c as i8 as i32 as u32);
        let g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 23 ^ g;
        }
    }
    h
}

/// The clip id of a hash (`anim_names::CLIPS`), if the exe names it.
pub fn clip_name(hash: u32) -> Option<&'static str> {
    use std::sync::OnceLock;
    static BY_HASH: OnceLock<std::collections::HashMap<u32, &'static str>> = OnceLock::new();
    BY_HASH.get_or_init(|| anim_names::CLIPS.iter().map(|n| (name_hash(n), *n)).collect()).get(&hash).copied()
}

/// One header (36 bytes).
#[derive(Clone, Copy, Debug)]
pub struct AflHeader {
    pub hash: u32,
    pub first_curve: u32,
    pub kind: u8,
    pub groups: u8,
    /// +10: a clip's frame count, then event frames (0xffff = none): +0xc..+0x14 and +0x14..+0x1c are the
    /// two sets of four `AnimClip_Advance` turns into event bits.
    pub words: [u16; 11],
}

/// A clip: its header and its groups' headers (type 0 body: 60 curves; type 1 board: 6; the cut-scene
/// files' 4-15: faces, props, cameras).
#[derive(Clone, Debug)]
pub struct AflClip {
    pub header: AflHeader,
    pub groups: Vec<AflHeader>,
}

impl AflClip {
    pub fn frames(&self) -> u16 { self.header.words[0] }
    pub fn name(&self) -> Option<&'static str> { clip_name(self.header.hash) }
    pub fn group(&self, kind: u8) -> Option<&AflHeader> { self.groups.iter().find(|g| g.kind == kind) }
}

pub struct AflFile<'a> {
    bytes: &'a [u8],
    pointers: usize,
    data: usize,
    pub clips: Vec<AflClip>,
}

impl<'a> AflFile<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<AflFile<'a>, String> {
        let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("short file");
        let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or("short file");
        let count = u16_at(2)? as usize;
        let pointers = u32_at(4)? as usize;
        let data = u32_at(8)? as usize;
        let mut headers = Vec::with_capacity(count);
        for i in 0..count {
            let o = 0xc + i * 36;
            let b = bytes.get(o..o + 36).ok_or("short header")?;
            headers.push(AflHeader {
                hash: u32::from_le_bytes(b[0..4].try_into().unwrap()),
                first_curve: u32::from_le_bytes(b[4..8].try_into().unwrap()),
                kind: b[8],
                groups: b[9],
                words: std::array::from_fn(|k| u16::from_le_bytes([b[10 + k * 2], b[11 + k * 2]])),
            });
        }
        let mut clips = Vec::new();
        let mut i = 0;
        while i < headers.len() {
            let n = headers[i].groups as usize;
            let groups = headers.get(i + 1..i + 1 + n).ok_or("clip groups past the headers")?.to_vec();
            clips.push(AflClip { header: headers[i], groups });
            i += 1 + n;
        }
        Ok(AflFile { bytes, pointers, data, clips })
    }

    /// The bytes of curve number `k` of the pointer table.
    pub fn curve(&self, k: usize) -> Option<&'a [u8]> {
        let p = self.bytes.get(self.pointers + k * 4..self.pointers + k * 4 + 4)?;
        self.bytes.get(self.data + u32::from_le_bytes(p.try_into().unwrap()) as usize..)
    }

    /// Curve `first + k` at frame `t` (`AnimCurve_Eval` 0x157ed8).
    pub fn eval(&self, first: u32, k: usize, t: f32) -> f32 {
        self.curve(first as usize + k).map_or(0.0, |c| eval(c, t))
    }
}

/// A 24-bit float: the top three bytes, the low one 0x80.
fn f24(b: &[u8]) -> f32 { f32::from_bits(0x80 | (b[0] as u32) << 8 | (b[1] as u32) << 16 | (b[2] as u32) << 24) }

/// The curve's u16 head: low 4 bits the kind, the rest a signed 12-bit count.
fn head(c: &[u8]) -> (u8, i32) {
    let h = u16::from_le_bytes([c[0], c[1]]);
    ((h & 0xf) as u8, ((h as i16) >> 4) as i32)
}

/// `AnimCurve_Eval` 0x157ed8: a kind-5 curve is a run of pieces, each covering its count minus one frames
/// (`t` taken from the piece's start); any other kind is evaluated whole. 0 past the last piece.
pub fn eval(c: &[u8], t: f32) -> f32 {
    let (kind, count) = head(c);
    if kind != 5 {
        return eval_piece(c, t);
    }
    let mut t = t;
    let mut p = 2;
    for _ in 0..count {
        let Some(piece) = c.get(p..) else { return 0.0 };
        let (k, n) = head(piece);
        if t <= (n - 1) as f32 {
            return eval_piece(piece, t);
        }
        p += match k {
            0..=3 => 2 + 3 * (k as usize + 1),
            4 => n as usize * 4 + 2,
            6 => n as usize + 8,
            7 => n as usize * 2 + 8,
            _ => 0,
        };
        t -= (n - 1) as f32;
    }
    0.0
}

/// `AnimCurve_EvalPiece` 0x158030: 0-3 a polynomial of that degree in `t` (24-bit coefficients, highest
/// power first); 4 floats per frame; 6 bytes and 7 u16s per frame, scaled and offset (24-bit offset, then
/// scale). Between frames the table kinds interpolate linearly.
fn eval_piece(c: &[u8], t: f32) -> f32 {
    let (kind, _) = head(c);
    let at = |i: usize| f24(&c[2 + i * 3..5 + i * 3]);
    match kind {
        0 => at(0),
        1 => t * at(0) + at(1),
        2 => t * (t * at(0) + at(1)) + at(2),
        3 => t * (t * (t * at(0) + at(1)) + at(2)) + at(3),
        4 => {
            let i = t as i32 as usize;
            let frac = t - i as f32;
            let f = |i: usize| f32::from_le_bytes(c[2 + i * 4..6 + i * 4].try_into().unwrap());
            let v = f(i);
            if frac == 0.0 { v } else { v + frac * (f(i + 1) - v) }
        }
        6 | 7 => {
            let i = t as i32 as usize;
            let frac = t - i as f32;
            let (offset, scale) = (at(0), at(1));
            let raw = |i: usize| if kind == 6 { c[8 + i] as f32 } else { u16::from_le_bytes([c[8 + i * 2], c[9 + i * 2]]) as f32 };
            let v = raw(i);
            if frac == 0.0 { v * scale + offset } else { (v + frac * (raw(i + 1) - v)) * scale + offset }
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(anim_names::CLIPS.len() + anim_names::FILES.len(), 1858);
        assert_eq!(clip_name(name_hash("bxRL_CROUCHCYCLE")), Some("bxRL_CROUCHCYCLE"));
    }

    #[test]
    fn pieces() {
        // a kind-5 curve of two constant pieces: 3 frames (covers 2) then constant
        let one = 1.0f32.to_bits().to_le_bytes();
        let two = 2.0f32.to_bits().to_le_bytes();
        let mut c = vec![0x25, 0x00];
        c.extend([0x30, 0x00, one[1], one[2], one[3]]);
        c.extend([0x30, 0x00, two[1], two[2], two[3]]);
        assert_eq!(eval(&c, 0.0), f24(&one[1..]));
        assert_eq!(eval(&c, 2.0), f24(&one[1..]));
        assert_eq!(eval(&c, 2.5), f24(&two[1..]));
    }
}

//! A course's `.adl` file (`data/models/<course>.adl`), the audio links, as the game reads it (board row E3e):
//! `AudioTGroups_Bind` 0x22a5f0 ties each entry to the course's instance with its name hash
//! (`Course_FindInstanceByHash`; the instance's +0xf4 points back at the entry), `AudioTGroup_GetBounds` 0x22aaa0
//! (with `FUN_0022acc0`) gives each entry's box, and `AudioTGroups_AddToGrid` 0x22b008 files it in the grid squares
//! it covers. Checked against the game's functions in the function runner (`tricky-game/tests/ltg.rs`).

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

/// One sound shape: its type and words (type 0: 7 words, 1 and 2: 12, 3: 6; word 1 unknown, words 2..4 a position
/// relative to the instance, word 5 a radius for 0, 2, 3; for 1 words 5..7 half-sizes and 8..10 a direction).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: u32,
    pub words: Vec<u32>,
}

/// A record the entries point at: word 0, then its shapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub word0: u32,
    pub shapes: Vec<Shape>,
}

/// The file: +0 a zero byte, +4 1.0 (else the game ignores it), +8 the number of entries, +0xc the entries (the
/// instance's name hash, the offset of its zone).
#[derive(Clone, Debug, PartialEq)]
pub struct Adl {
    pub entries: Vec<(u32, u32)>,
    pub zones: Vec<(u32, Zone)>,
}

impl Zone {
    fn parse(b: &[u8], at: usize) -> Option<Zone> {
        let n = word(b, at + 4)? as i32;
        let mut p = at + 8;
        let mut shapes = vec![];
        for _ in 0..n.max(0) {
            let kind = word(b, p)?;
            let len = match kind {
                0 => 7,
                1 | 2 => 12,
                3 => 6,
                // the game stays on an unknown shape (and reads it again); none is on the disc
                _ => return None,
            };
            let words = (0..len).map(|k| word(b, p + 4 * k)).collect::<Option<Vec<_>>>()?;
            shapes.push(Shape { kind, words });
            p += 4 * len;
        }
        Some(Zone { word0: word(b, at)?, shapes })
    }

    /// `AudioTGroup_GetBounds` 0x22aaa0: the box around the shapes, from ±10000 cm, moved by the instance's
    /// position (+0x30). Returns (min, max).
    pub fn bounds(&self, at: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [10000.0f32; 3];
        let mut hi = [-10000.0f32; 3];
        let mut s_lo = [-10000.0f32; 3];
        let mut s_hi = [0.0f32; 3];
        for s in &self.shapes {
            let f = |k: usize| f32::from_bits(s.words[k]);
            match s.kind {
                1 => {
                    let (d0, d1, d2) = (f(8), f(9), f(10));
                    // side = d × up (0, 0, 1), normalised; up' = d × side
                    let sx = d2 * 0.0 - d1 * 1.0;
                    let sy = d0 * 1.0 - d2 * 0.0;
                    let sz = d1 * 0.0 - d0 * 0.0;
                    let len2 = sx * sx + sy * sy + sz * sz;
                    let k = 1.0 / len2.sqrt();
                    let (sx, sy, sz) = (sx * k, sy * k, sz * k);
                    let ux = d1 * sz - d2 * sy;
                    let uy = d0 * sy - d1 * sx;
                    let uz = d2 * sx - d0 * sz;
                    let (e0, e1, e2) = (f(5), f(6), f(7));
                    s_lo = [f(2), f(3), f(4)];
                    s_hi = s_lo;
                    s_lo[0] -= d0 * e0 + sx * e1 + ux * e2;
                    s_lo[2] -= d2 * e0 + sz * e1 + uy * e2;
                    s_lo[1] -= d1 * e0 + sy * e1 + uz * e2;
                    s_hi[2] += d2 * e0 + sz * e1 + uy * e2;
                    s_hi[0] += d0 * e0 + sx * e1 + ux * e2;
                    s_hi[1] += d1 * e0 + sy * e1 + uz * e2;
                }
                0 | 2 | 3 => {
                    let r = f(5);
                    s_lo = [f(2) - r, f(3) - r, f(4) - r];
                    s_hi = [f(2) + r, f(3) + r, f(4) + r];
                }
                _ => {}
            }
            for k in 0..3 {
                if s_lo[k] < lo[k] {
                    lo[k] = s_lo[k];
                }
            }
            for k in 0..3 {
                if hi[k] < s_hi[k] {
                    hi[k] = s_hi[k];
                }
            }
        }
        for k in 0..3 {
            lo[k] += at[k];
        }
        for k in 0..3 {
            hi[k] += at[k];
        }
        (lo, hi)
    }
}

impl Adl {
    /// `None` when the game would not take the file (first byte not 0, or +4 not 1.0) or it is cut short.
    pub fn parse(b: &[u8]) -> Option<Adl> {
        if *b.first()? != 0 || f32::from_bits(word(b, 4)?) != 1.0 {
            return None;
        }
        let n = word(b, 8)? as i32;
        let entries = (0..n.max(0) as usize).map(|i| Some((word(b, 0xc + 8 * i)?, word(b, 0x10 + 8 * i)?))).collect::<Option<Vec<_>>>()?;
        let mut zones: Vec<(u32, Zone)> = vec![];
        for (_, off) in &entries {
            if !zones.iter().any(|(o, _)| o == off) {
                zones.push((*off, Zone::parse(b, *off as usize)?));
            }
        }
        zones.sort_by_key(|z| z.0);
        Some(Adl { entries, zones })
    }

    pub fn zone(&self, off: u32) -> Option<&Zone> {
        self.zones.iter().find(|z| z.0 == off).map(|z| &z.1)
    }
}

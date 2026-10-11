//! Character and board models: the `.mpf` files of `DATA/CHAR/MDLPS2.BIG` and `BRDPS2.BIG` (board row E4a).
//!
//! Proven from the game's loader (`MpfFile_Open` 0x208468, `MpfFile_AddPart` 0x208488, `SkinPart_Load`
//! 0x205080, `SkinPart_ResolveTextureNames` 0x2055c0, `SkinPart_ResolveMaterials` 0x205670): the file header,
//! the 0x50-byte part directory, and the tables of a part (texture sets, bones, materials, weight sets,
//! weight palettes, mesh packet lists). Inferred from the data (see `port-notes/files.md`): the meshes are PS2
//! DMA chains of VIF packets the game sends to the vector unit as they are, and their layout is read here
//! from the packets themselves, because the vector unit's program is not decompiled. Checked against the
//! models exported before (`chars/*/model.json`): same triangles, UVs, normals, weights and winding.

use std::fmt;

/// Strings in the part tables are fixed-size, zero-terminated.
fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).into_owned()
}

#[derive(Debug)]
pub struct MpfError(pub String);

impl fmt::Display for MpfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl std::error::Error for MpfError {}

fn err<T>(s: impl Into<String>) -> Result<T, MpfError> { Err(MpfError(s.into())) }

/// A little-endian reader over a part's data that fails instead of panicking.
struct R<'a>(&'a [u8]);

impl R<'_> {
    fn bytes(&self, at: usize, n: usize) -> Result<&[u8], MpfError> {
        self.0.get(at..at + n).ok_or_else(|| MpfError(format!("read past the end at {at:#x}")))
    }
    fn u32(&self, at: usize) -> Result<u32, MpfError> { Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap())) }
    fn i16(&self, at: usize) -> Result<i16, MpfError> { Ok(i16::from_le_bytes(self.bytes(at, 2)?.try_into().unwrap())) }
    fn f32(&self, at: usize) -> Result<f32, MpfError> { Ok(f32::from_bits(self.u32(at)?)) }
}

/// One entry of the part directory (0x50 bytes).
#[derive(Clone, Debug)]
pub struct PartEntry {
    pub name: String,
    pub raw: [u8; 0x50],
}

impl PartEntry {
    fn u32(&self, at: usize) -> u32 { u32::from_le_bytes(self.raw[at..at + 4].try_into().unwrap()) }
    fn u16(&self, at: usize) -> u16 { u16::from_le_bytes(self.raw[at..at + 2].try_into().unwrap()) }
    /// Offset of the part's data from the file's data area (+0x10) and its size (+0x14).
    pub fn data(&self) -> (usize, usize) { (self.u32(0x10) as usize, self.u32(0x14) as usize) }
    /// The skin slot the part goes to (+0x4a): `SkinLod_AddPart` keeps one part per slot.
    pub fn slot(&self) -> i8 { self.raw[0x4a] as i8 }
}

/// An open `.mpf` file: `MpfFile_Open` 0x208468. +4 s16 part count, +6 s16 directory offset, +8 u32 offset of
/// the parts' data.
pub struct MpfFile<'a> {
    bytes: &'a [u8],
    data_at: usize,
    pub parts: Vec<PartEntry>,
}

impl<'a> MpfFile<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<MpfFile<'a>, MpfError> {
        let r = R(bytes);
        let count = r.i16(4)?.max(0) as usize;
        let dir = r.i16(6)? as u16 as usize;
        let data_at = r.u32(8)? as usize;
        let mut parts = Vec::with_capacity(count);
        for i in 0..count {
            let raw: [u8; 0x50] = r.bytes(dir + i * 0x50, 0x50)?.try_into().unwrap();
            parts.push(PartEntry { name: cstr(&raw[..0x10]), raw });
        }
        Ok(MpfFile { bytes, data_at, parts })
    }

    /// `MpfFile_AddPart` 0x208488's search: the first part whose name matches on at most 15 characters,
    /// ignoring case (`strnicmp` 0x2fd398).
    pub fn find(&self, name: &str) -> Option<&PartEntry> {
        self.parts.iter().find(|p| strnicmp15(p.name.as_bytes(), name.as_bytes()))
    }

    /// The part's tables and meshes.
    pub fn part(&self, e: &PartEntry) -> Result<MpfPart, MpfError> {
        let (off, size) = e.data();
        let data = self.bytes.get(self.data_at + off..self.data_at + off + size).ok_or_else(|| MpfError(format!("{}: data past the end", e.name)))?;
        MpfPart::read(e, data)
    }
}

fn strnicmp15(a: &[u8], b: &[u8]) -> bool {
    for i in 0..15 {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if !x.eq_ignore_ascii_case(&y) {
            return false;
        }
        if x == 0 {
            return true;
        }
    }
    true
}

/// A texture set (0x20 bytes): five 4-character names (the texture first, the gloss map fourth) and the
/// gloss strength at +0x14 (`SkinPart_ResolveMaterials` 0x205670 turns it into `strength * 127`).
#[derive(Clone, Debug)]
pub struct TexSet {
    pub names: [String; 5],
    pub strength: f32,
}

impl TexSet {
    pub fn texture(&self) -> &str { &self.names[0] }
}

/// A bone (0x54 bytes): name, the parent as +0x10 slot and +0x12 index in that slot's part (-1, -1 for the
/// root; slot 0 is the body), +0x18 position (cm) and +0x24 rotation (Euler angles of the inverse rotation,
/// as the exports keep them).
#[derive(Clone, Debug)]
pub struct MpfBone {
    pub name: String,
    pub parent_slot: i16,
    pub parent: i16,
    pub pos: [f32; 3],
    pub rot: [f32; 3],
}

/// A vertex as the packets hold it.
#[derive(Clone, Debug, PartialEq)]
pub struct MpfVertex {
    pub pos: [f32; 3],
    /// 4.12 fixed point in the packet.
    pub uv: [f32; 2],
    /// s16 / 32767 in the packet.
    pub nrm: [f32; 3],
    /// Index into the part's weight sets.
    pub weights: usize,
}

/// One material: its packet lists. Each list has its own weight palette and one or more records, each a
/// packet chain drawn in turn (the face's 18 records are 18 pieces of it, every one a chain ending in
/// `ret` and followed by data not read yet, likely its expressions).
#[derive(Clone, Debug)]
pub struct MpfMesh {
    /// Index into the part's texture sets.
    pub texture: usize,
    /// Material +0: 1, 0x100 for the face, 0x11 for the shadows.
    pub flags: u32,
    /// Per list, per record, the record's triangle strips. Empty for a shadow (see `is_shadow`).
    pub lists: Vec<Vec<Vec<Vec<MpfVertex>>>>,
}

impl MpfMesh {
    /// A shadow mesh (flags 0x10): its packets are positions only, for another vector unit program; not
    /// decoded yet (row E4d).
    pub fn is_shadow(&self) -> bool { self.flags & 0x10 != 0 }

    /// The strips of every record.
    pub fn strips(&self) -> impl Iterator<Item = &Vec<MpfVertex>> {
        self.lists.iter().flatten().flatten()
    }

    /// `strips` as triangles, odd triangles turned round, the degenerate ones (two equal corners: the
    /// joins the packets use) left out, as the GS draws nothing for them.
    pub fn triangles(&self) -> Vec<[&MpfVertex; 3]> {
        let mut out = Vec::new();
        for s in self.strips() {
            for t in 0..s.len().saturating_sub(2) {
                let (a, b, c) = (&s[t], &s[t + 1], &s[t + 2]);
                if a.pos == b.pos || b.pos == c.pos || a.pos == c.pos {
                    continue;
                }
                out.push(if t % 2 == 0 { [a, b, c] } else { [b, a, c] });
            }
        }
        out
    }
}

/// A part with its tables (`SkinPart_Load` 0x205080) and decoded meshes.
#[derive(Clone, Debug)]
pub struct MpfPart {
    pub name: String,
    pub slot: i8,
    pub textures: Vec<TexSet>,
    pub bones: Vec<MpfBone>,
    /// Weight sets: (bone, percent) pairs; entry+0x30 list of (count, pointer, ...) 0xc records.
    pub weights: Vec<Vec<(u16, u16)>>,
    pub meshes: Vec<MpfMesh>,
}

impl MpfPart {
    fn read(e: &PartEntry, data: &[u8]) -> Result<MpfPart, MpfError> {
        let r = R(data);
        let materials = e.u16(0x40) as usize;
        let bones_n = e.u16(0x42) as usize;
        let textures_n = e.u16(0x44) as usize;
        let weights_n = e.u16(0x3c) as usize;
        let palettes_n = e.u16(0x3e) as usize;
        let mut textures = Vec::with_capacity(textures_n);
        for i in 0..textures_n {
            let b = r.bytes(i * 0x20, 0x20)?;
            textures.push(TexSet { names: std::array::from_fn(|k| cstr(&b[k * 4..k * 4 + 4])), strength: r.f32(i * 0x20 + 0x14)? });
        }
        let bones_at = e.u32(0x18) as usize;
        let mut bones = Vec::with_capacity(bones_n);
        for i in 0..bones_n {
            let at = bones_at + i * 0x54;
            let v = |o: usize| -> Result<[f32; 3], MpfError> { Ok([r.f32(at + o)?, r.f32(at + o + 4)?, r.f32(at + o + 8)?]) };
            bones.push(MpfBone { name: cstr(r.bytes(at, 0x10)?), parent_slot: r.i16(at + 0x10)?, parent: r.i16(at + 0x12)?, pos: v(0x18)?, rot: v(0x24)? });
        }
        let weights_at = e.u32(0x30) as usize;
        let mut weights = Vec::with_capacity(weights_n);
        for i in 0..weights_n {
            let n = r.u32(weights_at + i * 0xc)? as usize;
            let p = r.u32(weights_at + i * 0xc + 4)? as usize;
            let mut set = Vec::with_capacity(n);
            for k in 0..n {
                let v = r.u32(p + k * 4)?;
                set.push(((v >> 16) as u16, v as u16));
            }
            weights.push(set);
        }
        let palettes_at = e.u32(0x2c) as usize;
        let mut palettes = Vec::with_capacity(palettes_n);
        for i in 0..palettes_n {
            let n = r.u32(palettes_at + i * 8)? as usize;
            let p = r.u32(palettes_at + i * 8 + 4)? as usize;
            palettes.push((0..n).map(|k| r.u32(p + k * 4).map(|v| v as usize)).collect::<Result<Vec<_>, _>>()?);
        }
        // materials (0x14): +4 texture set, +0xc count and +0x10 pointer of (pointer, count) pairs, each
        // pointing at `count` 0xc-byte records whose first word is a packet chain
        let materials_at = e.u32(0x20) as usize;
        let mut meshes = Vec::with_capacity(materials);
        let mut chunk = 0;
        for m in 0..materials {
            let at = materials_at + m * 0x14;
            let flags = r.u32(at)?;
            let texture = r.u32(at + 4)? as usize;
            let n_lists = r.u32(at + 0xc)? as usize;
            if flags & 0x10 != 0 {
                // a shadow keeps its lists' palettes in the count
                chunk += n_lists;
                meshes.push(MpfMesh { texture, flags, lists: Vec::new() });
                continue;
            }
            let lists_at = r.u32(at + 0x10)? as usize;
            let mut lists = Vec::with_capacity(n_lists);
            for l in 0..n_lists {
                let p = r.u32(lists_at + l * 8)? as usize;
                let n = r.u32(lists_at + l * 8 + 4)? as usize;
                let palette = palettes.get(chunk).ok_or_else(|| MpfError(format!("{}: no palette {chunk}", e.name)))?;
                let mut records = Vec::with_capacity(n);
                for k in 0..n {
                    let start = r.u32(p + k * 0xc)? as usize;
                    let mut strips = Vec::new();
                    decode_chain(&r, start, palette, weights.len(), &mut strips).map_err(|x| MpfError(format!("{}: {}", e.name, x.0)))?;
                    records.push(strips);
                }
                lists.push(records);
                chunk += 1;
            }
            meshes.push(MpfMesh { texture, flags, lists });
        }
        Ok(MpfPart { name: e.name.clone(), slot: e.slot(), textures, bones, weights, meshes })
    }
}

/// The vector unit address the weight palette starts at: a vertex's weight set is
/// `palette[(uv.z - 14) / 4]` (inferred: every vertex of every shipped model fits it).
const PALETTE_BASE: i16 = 14;

/// Walks a DMA chain (tags `cnt` until `end`/`ret`) of VIF packets. Each batch is four unpacks and a
/// program start (`MSCAL`): a V4-32 header (a GS tag, then the strip count and one strip length per
/// quadword), and per vertex, interleaved three quadwords apart (`STCYCL` 3/1): V4-16 UV (x, y in 4.12;
/// z: the weight palette address), V3-16 normal (/32767), V3-32 position. Inferred from the data.
fn decode_chain(r: &R, mut at: usize, palette: &[usize], weight_sets: usize, strips: &mut Vec<Vec<MpfVertex>>) -> Result<(), MpfError> {
    let mut unpacks: Vec<(u32, &[u8])> = Vec::new();
    for _ in 0..100_000 {
        let lo = r.u32(at)?;
        let qwc = (lo & 0xffff) as usize;
        let id = (lo >> 28) & 7;
        let body = r.bytes(at + 8, 8 + qwc * 16)?;
        let mut i = 0;
        while i + 4 <= body.len() {
            let w = u32::from_le_bytes(body[i..i + 4].try_into().unwrap());
            i += 4;
            let cmd = (w >> 24) & 0x7f;
            let num = ((w >> 16) & 0xff) as usize;
            match cmd {
                0x60..=0x7f => {
                    let comps = ((cmd >> 2) & 3) as usize + 1;
                    let bits = [32, 16, 8, 5][(cmd & 3) as usize];
                    let n = ((bits * comps * num + 31) / 32) * 4;
                    let d = body.get(i..i + n).ok_or_else(|| MpfError("unpack past the packet".into()))?;
                    unpacks.push((cmd & 0x0f, d));
                    i += n;
                }
                0x30 | 0x31 => i += 16,
                0x20 => i += 4,
                0x14 | 0x15 | 0x17 => {
                    flush(&mut unpacks, palette, weight_sets, strips)?;
                }
                _ => {}
            }
        }
        at += 16 + qwc * 16;
        if id == 7 || id == 6 {
            // a record called as a subroutine (the face's) ends with `ret` and no program start
            return flush(&mut unpacks, palette, weight_sets, strips);
        }
    }
    err("packet chain without an end")
}

/// Decodes the batches collected since the last program start, four unpacks each.
fn flush(unpacks: &mut Vec<(u32, &[u8])>, palette: &[usize], weight_sets: usize, strips: &mut Vec<Vec<MpfVertex>>) -> Result<(), MpfError> {
    if unpacks.len() % 4 != 0 {
        return err(format!("{} unpacks before a program start", unpacks.len()));
    }
    for b in unpacks.chunks(4) {
        batch(b, palette, weight_sets, strips)?;
    }
    unpacks.clear();
    Ok(())
}

fn batch(u: &[(u32, &[u8])], palette: &[usize], weight_sets: usize, strips: &mut Vec<Vec<MpfVertex>>) -> Result<(), MpfError> {
    // V4-32 = 0xc, V4-16 = 0xd, V3-16 = 0x9, V3-32 = 0x8
    let (hdr, uv, nrm, pos) = match u {
        [(0xc, h), (0xd, a), (0x9, b), (0x8, c)] => (*h, *a, *b, *c),
        _ => return err(format!("unexpected batch layout {:?}", u.iter().map(|x| x.0).collect::<Vec<_>>())),
    };
    let word = |b: &[u8], i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
    let half = |b: &[u8], i: usize| i16::from_le_bytes(b[i * 2..i * 2 + 2].try_into().unwrap());
    let count = pos.len() / 12;
    let n_strips = word(hdr, 4) as usize;
    let mut v = 0;
    for s in 0..n_strips {
        let len = word(hdr, 8 + s * 4) as usize;
        let mut strip = Vec::with_capacity(len);
        for _ in 0..len {
            if v >= count {
                return err("strip longer than the batch");
            }
            let z = half(uv, v * 4 + 2);
            let slot = ((z - PALETTE_BASE) / 4) as usize;
            let weights = *palette.get(slot).ok_or_else(|| MpfError(format!("weight address {z}")))?;
            if weights >= weight_sets {
                return err(format!("weight set {weights}"));
            }
            let f = |i: usize| f32::from_bits(word(pos, v * 3 + i));
            strip.push(MpfVertex {
                pos: [f(0), f(1), f(2)],
                uv: [half(uv, v * 4) as f32 / 4096.0, half(uv, v * 4 + 1) as f32 / 4096.0],
                nrm: [half(nrm, v * 3) as f32 / 32767.0, half(nrm, v * 3 + 1) as f32 / 32767.0, half(nrm, v * 3 + 2) as f32 / 32767.0],
                weights,
            });
            v += 1;
        }
        strips.push(strip);
    }
    Ok(())
}

/// The part names the game builds a rider from (`BdrLoad_BuildSkin` 0x201138), read out of the exe:
/// per rider and level of detail (0 is the closest, 3 the shadow) up to three body parts (table 0x341318)
/// and five head parts (table 0x341588), and the board's (0x341998: by board kind 0 Bx, 1 Fr, 2 Al and
/// stance, the shadows from +0x18).
pub mod names {
    /// The riders in the game's order (`|data/char/<name>_body.mpf` at 0x3419c8, `_head.mpf` at 0x341a00).
    pub const RIDERS: [&str; 13] = ["eddie", "kaori", "luther", "mac", "moby", "zoe", "jp", "elise", "psymon", "seeiah", "brodi", "marisol", "zz_mmm"];

    const NO: Option<&str> = None;

    pub const BODY: [[[Option<&str>; 3]; 4]; 13] = {
        const PLAIN: [[Option<&str>; 3]; 4] = [[Some("Body3000"), NO, NO], [Some("Body1500"), NO, NO], [Some("Body750"), NO, NO], [Some("BodyShdw750"), NO, NO]];
        const PACK: [[Option<&str>; 3]; 4] = [
            [Some("Body3000"), Some("KaoriPack3000"), NO],
            [Some("Body1500"), Some("KaoriPack1500"), NO],
            [Some("Body750"), Some("KaoriPack750"), NO],
            [Some("BodyShdw750"), Some("KaoriPackShdw750"), NO],
        ];
        [PLAIN, PACK, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PLAIN, PACK]
    };

    macro_rules! head {
        ($g:expr, $hair:expr, $head:expr) => {
            [
                [$g, $hair[0], $head, Some("Face3000"), Some("Eyes3000")],
                [NO, $hair[1], Some("Head1500"), NO, NO],
                [NO, $hair[2], Some("Head750"), NO, NO],
                [NO, $hair[3], Some("HeadShdw750"), NO, NO],
            ]
        };
    }
    const fn hair(a: &'static str, b: &'static str, c: &'static str, d: &'static str) -> [Option<&'static str>; 4] { [Some(a), Some(b), Some(c), Some(d)] }
    const NOHAIR: [Option<&str>; 4] = [NO, NO, NO, NO];
    const G: Option<&str> = Some("Goggles3000");
    const H: Option<&str> = Some("Head3000");

    pub const HEAD: [[[Option<&str>; 5]; 4]; 13] = [
        head!(G, hair("EddieHair3000", "EddieHair1500", "EddieHair750", "EddieHairShdw750"), H),
        head!(NO, hair("KaoriHair3000", "KaoriHair1500", "KaoriHair750", "KaoriHairShdw750"), H),
        head!(G, hair("LutherHair3000", "LutherHair1500", "LutherHair750", "LutherHairShdw750"), H),
        head!(Some("Goggles23000"), NOHAIR, H),
        head!(G, NOHAIR, H),
        head!(G, NOHAIR, H),
        head!(G, hair("JPHair3000", "JPHair1500", "JPHair750", "JPHairShdw750"), NO),
        head!(G, NOHAIR, H),
        head!(NO, hair("PsymonHair3000", "PsymonHair1500", "PsymonHair750", "PsymonHairShdw750"), NO),
        head!(G, NOHAIR, NO),
        head!(G, hair("BrodiHair3000", "BrodiHair1500", "BrodiHair750", "BrodiHairShdw750"), NO),
        head!(G, hair("MarisolHair3000", "MarisolHair1500", "MarisolHair750", "MarisolHairShdw750"), H),
        head!(NO, NOHAIR, NO),
    ];

    /// 0x341998: Bx, Fr, Al, then the goofy ones, then the shadows (as the table has them, `ShdwBx` twice).
    pub const BOARD: [&str; 12] = ["Bx", "Fr", "Al", "BxGoofy", "FrGoofy", "AlGoofy", "ShdwBx", "ShdwFr", "ShdwAl", "ShdwBx", "ShdwFr", "ShdwAlGoofy"];
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the player's own image: every part of every model decodes, and the game's part names are found.
    #[test]
    fn every_model_decodes() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        for (r, rider) in names::RIDERS.iter().enumerate() {
            for (kind, table) in [("body", names::BODY[r].as_flattened()), ("head", names::HEAD[r].as_flattened())] {
                let bytes = files.load(&format!("|data/char/{rider}_{kind}.mpf")).unwrap().unwrap();
                let mpf = MpfFile::open(&bytes).unwrap();
                for e in &mpf.parts {
                    let p = mpf.part(e).unwrap_or_else(|x| panic!("{rider} {kind}: {x}"));
                    assert!(p.meshes.iter().all(|m| m.texture < p.textures.len()));
                }
                for n in table.iter().flatten() {
                    assert!(mpf.find(n).is_some(), "{rider} {kind}: {n}");
                }
            }
        }
        let bytes = files.load("|data/char/board.mpf").unwrap().unwrap();
        let mpf = MpfFile::open(&bytes).unwrap();
        for n in names::BOARD {
            let e = mpf.find(n).unwrap_or_else(|| panic!("board {n}"));
            mpf.part(e).unwrap();
        }
    }
}

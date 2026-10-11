//! A course's `.ltg` file (`data/models/<course>.ltg`), the world grid, as the game reads it (board row E3e):
//! `cCourseResolve_Common::ResolveTGroupHeader` 0x252470, `ResolveTGroups` 0x252748, `ResolveTGroup` 0x2527e0 and
//! `ResolveINode` 0x252b88 (their debug prints name the fields). The world is a grid of "TGroups" (100 m squares on
//! the disc), each listing the `.pbd`'s patches, instances, splines, lights and particle instances in it and split
//! into 4 × 4 "INodes" with their own sub-lists. `WorldGrid_CellAt` 0x260ba8 finds a cell's INode. Checked against
//! the game's resolver in the function runner (`tricky-game/tests/ltg.rs`).

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}
fn half(b: &[u8], at: usize) -> Option<i16> {
    b.get(at..at + 2).map(|w| i16::from_le_bytes([w[0], w[1]]))
}
fn f32s<const N: usize>(b: &[u8], at: usize) -> Option<[f32; N]> {
    let mut v = [0.0; N];
    for (k, x) in v.iter_mut().enumerate() {
        *x = f32::from_bits(word(b, at + 4 * k)?);
    }
    Some(v)
}
fn list(b: &[u8], at: usize, n: i16) -> Option<Vec<u32>> {
    (0..n.max(0) as usize).map(|i| word(b, at + 4 * i)).collect()
}

/// A bounding box as the file gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub centre: [f32; 3],
}

impl BBox {
    fn at(b: &[u8], at: usize) -> Option<BBox> {
        Some(BBox { min: f32s(b, at)?, max: f32s(b, at + 12)?, centre: f32s(b, at + 24)? })
    }
}

/// An INode (0x4c bytes): its box, then counts (+0x24 patches, +0x26 models, +0x28 models and gems, +0x2a splines,
/// +0x2c lights inside, +0x2e lights crossing, +0x30 particle models) and the offsets of its lists from its TGroup
/// (+0x34 patches, +0x38 models — also the models and gems, +0x3c splines, +0x40 lights inside, +0x44 lights
/// crossing, +0x48 particle models; 0 none). Its lists are runs of its TGroup's lists.
#[derive(Clone, Debug, PartialEq)]
pub struct INode {
    pub bbox: BBox,
    pub counts: [i16; 7],
    pub offsets: [u32; 6],
}

/// A TGroup: its box, counts (+0x24 patches, +0x26 instances, +0x28 splines, +0x2a lights inside, +0x2c lights
/// crossing, +0x2e particle instances, +0x30, +0x32, +0x34 elements), its lists of `.pbd` indices (offsets +0x3c
/// patches, +0x40 instances, +0x44 splines, +0x48 lights inside, +0x4c lights crossing, +0x50 particle instances;
/// +0x38 its INodes) and its 16 INodes (from +0x54).
#[derive(Clone, Debug, PartialEq)]
pub struct TGroup {
    pub bbox: BBox,
    pub counts: [i16; 9],
    pub inodes_offset: u32,
    pub offsets: [u32; 6],
    pub patches: Vec<u32>,
    pub instances: Vec<u32>,
    pub splines: Vec<u32>,
    pub lights_inside: Vec<u32>,
    pub lights_crossing: Vec<u32>,
    pub particles: Vec<u32>,
    pub inodes: Vec<INode>,
}

/// The file. Header: +0 the version (three bytes) and +3 big-endian, +4 the world's box, +0x28 the TGroup size,
/// +0x2c / +0x30 the grid's width and height in TGroups, +0x34 their number, +0x38 used, +0x3c empty, +0x40 the
/// INode size, +0x44 INodes across a TGroup, +0x48 INodes in one, +0x4c the TGroup table's offset (one word each:
/// the TGroup's offset, 0 empty), +0x50 where the TGroups start.
#[derive(Clone, Debug, PartialEq)]
pub struct Ltg {
    pub version: [u8; 3],
    pub big_endian: bool,
    pub bbox: BBox,
    pub tgroup_size: f32,
    pub width: i32,
    pub height: i32,
    pub num: i32,
    pub used: i32,
    pub empty: i32,
    pub inode_size: f32,
    pub inode_wide: i32,
    pub inode_num: i32,
    pub entry_offset: u32,
    pub tgroup_offset: u32,
    /// per grid square (row by row), its TGroup's offset and the TGroup
    pub tgroups: Vec<Option<(u32, TGroup)>>,
}

impl Ltg {
    pub fn parse(b: &[u8]) -> Option<Ltg> {
        let w = |o: usize| word(b, o);
        let i = |o: usize| w(o).map(|v| v as i32);
        let f = |o: usize| w(o).map(f32::from_bits);
        let num = i(0x34)?;
        let entry_offset = w(0x4c)?;
        let tgroups = (0..num.max(0) as usize)
            .map(|k| {
                let off = w(entry_offset as usize + 4 * k)?;
                if off == 0 {
                    return Some(None);
                }
                Some(Some((off, TGroup::parse(b, off as usize)?)))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Ltg {
            version: [*b.first()?, *b.get(1)?, *b.get(2)?],
            big_endian: *b.get(3)? != 0,
            bbox: BBox::at(b, 4)?,
            tgroup_size: f(0x28)?,
            width: i(0x2c)?,
            height: i(0x30)?,
            num,
            used: i(0x38)?,
            empty: i(0x3c)?,
            inode_size: f(0x40)?,
            inode_wide: i(0x44)?,
            inode_num: i(0x48)?,
            entry_offset,
            tgroup_offset: w(0x50)?,
            tgroups,
        })
    }

    /// `WorldGrid_CellAt` 0x260ba8: the INode at INode coordinates (x, y), or `None` outside the grid or in an
    /// empty TGroup.
    pub fn inode_at(&self, x: i32, y: i32) -> Option<&INode> {
        let n = self.inode_wide;
        if x < 0 || x >= self.width * n || y < 0 || y >= self.height * n || n == 0 {
            return None;
        }
        let (tx, ty) = ((x / n).clamp(0, self.width - 1), (y / n).clamp(0, self.height - 1));
        let (_, t) = self.tgroups.get((ty * self.width + tx) as usize)?.as_ref()?;
        t.inodes.get(((y - (y / n) * n) * n + (x - (x / n) * n)) as usize)
    }
}

impl TGroup {
    fn parse(b: &[u8], at: usize) -> Option<TGroup> {
        let mut counts = [0i16; 9];
        for (k, c) in counts.iter_mut().enumerate() {
            *c = half(b, at + 0x24 + 2 * k)?;
        }
        let mut offsets = [0u32; 6];
        for (k, o) in offsets.iter_mut().enumerate() {
            *o = word(b, at + 0x3c + 4 * k)?;
        }
        let l = |k: usize, n: i16| if offsets[k] == 0 { Some(vec![]) } else { list(b, at + offsets[k] as usize, n) };
        let inodes = (0..16)
            .map(|k| {
                let p = at + 0x54 + 0x4c * k;
                let mut c = [0i16; 7];
                for (j, v) in c.iter_mut().enumerate() {
                    *v = half(b, p + 0x24 + 2 * j)?;
                }
                let mut o = [0u32; 6];
                for (j, v) in o.iter_mut().enumerate() {
                    *v = word(b, p + 0x34 + 4 * j)?;
                }
                Some(INode { bbox: BBox::at(b, p)?, counts: c, offsets: o })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(TGroup {
            bbox: BBox::at(b, at)?,
            inodes_offset: word(b, at + 0x38)?,
            patches: l(0, counts[0])?,
            instances: l(1, counts[1])?,
            splines: l(2, counts[2])?,
            lights_inside: l(3, counts[3])?,
            lights_crossing: l(4, counts[4])?,
            particles: l(5, counts[5])?,
            counts,
            offsets,
            inodes,
        })
    }
}

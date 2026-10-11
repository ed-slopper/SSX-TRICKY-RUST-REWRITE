//! A course's `.pbd` file (`data/models/<course>.pbd`), the level's world data, as the game reads it (board row
//! E3a): `cCourseResolve_PS2::ResolveDataHeader` 0x255630 and the `cCourseResolve_Common` resolvers it calls
//! (their debug prints name the fields). This reads the layout: the header's counts and offsets, every table's
//! records, the models with their hierarchies, meshes, mesh contexts and animations, the particle models, the cameras
//! and the hash table. What the records' fields mean to drawing and collision belongs to those rows (G7, E7, E4).
//! Checked by running the game's resolver on every course in the function runner (`tricky-game/tests/pbd.rs`).
//!
//! Note the game's names for the two spline tables: header +0x20 / +0x5c are "splines" (0x28-byte records, what
//! `Course_GetSplineSegment` 0x254b20 returns and the `.ssf` calls segments), +0x24 / +0x60 "spline segments" (0x80
//! bytes, `Course_GetSpline` 0x254b50).

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}
fn words<const N: usize>(b: &[u8], at: usize) -> Option<[u32; N]> {
    let mut v = [0u32; N];
    for (k, x) in v.iter_mut().enumerate() {
        *x = word(b, at + 4 * k)?;
    }
    Some(v)
}

/// Record sizes.
pub const PATCH: usize = 0x1c0;
pub const INSTANCE: usize = 0x100;
pub const PARTICLE_INSTANCE: usize = 0x70;
pub const MATERIAL: usize = 0x48;
pub const LIGHT: usize = 0x5c;
pub const SPLINE: usize = 0x28;
pub const SEGMENT: usize = 0x80;

/// The header (0x88 bytes): +0 the version (three bytes), +3 the format (0 generic, 1 PS2, 2 Xbox, 3 GameCube); the
/// counts +4 player starts, +8 patches, +0xc instances, +0x10 particle instances, +0x14 materials, +0x18 material
/// blocks, +0x1c lights, +0x20 splines, +0x24 spline segments, +0x28 texture flips, +0x2c models, +0x30 particle
/// models, +0x34 textures, +0x38 cameras, +0x3c the light map size; the offsets +0x40 player starts, +0x44 patches,
/// +0x48 instances, +0x4c particle instances, +0x50 materials, +0x54 material blocks, +0x58 lights, +0x5c splines,
/// +0x60 spline segments, +0x64 texture flips, +0x68 model pointers, +0x6c models, +0x70 particle model pointers,
/// +0x74 particle models, +0x78 camera pointers, +0x7c cameras, +0x80 the hash table, +0x84 the VIF data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: [u8; 3],
    pub format: u8,
    /// +4..+0x3c
    pub counts: [u32; 15],
    /// +0x40..+0x84
    pub offsets: [u32; 18],
}

impl Header {
    pub fn count(&self, at: usize) -> u32 {
        self.counts[(at - 4) / 4]
    }
    pub fn offset(&self, at: usize) -> u32 {
        self.offsets[(at - 0x40) / 4]
    }
}

/// A mesh context (`ResolveMeshContext` 0x255c20): +0 its length, +4 its material, +0xc its VIF data (from the
/// file's VIF block, +0x84).
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    pub offset: u32,
    pub words: [u32; 4],
}

/// A mesh (`ResolveMesh` 0x2539b0): +0 length, +4 / +0x10 its box, +0x1c flags, +0x20 contexts, +0x24 faces,
/// +0x28 the contexts' table (each slot an offset from itself).
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub offset: u32,
    pub head: [u32; 11],
    pub contexts: Vec<Context>,
}

/// An animation (`ResolveAnimation` 0x254078): +0x1c channels, +0x20 their table (offsets from the animation); a
/// channel's +4 is an offset from the channel (`ResolveChannel` 0x254120).
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub offset: u32,
    pub head: [u32; 9],
    /// each channel's offset and its +4 word
    pub channels: Vec<(u32, u32)>,
}

/// A model's hierarchy entry (`ResolveModelHierarchies` 0x2537d8, 6 words): the parent, the high / medium / low
/// detail meshes (offsets from the entry, 0 none; the same mesh may serve several), an animation, the matrix (−1:
/// the identity at 0x346440).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub offset: u32,
    pub words: [u32; 6],
    pub animation: Option<Animation>,
}

/// A model (`ResolveModel` 0x253630): +0 length, +4 nodes, +8 the hierarchy's offset, +0xc the material block,
/// +0x14 the animation time, +0x18 the vertex scale, +0x24 contexts, +0x2c strips, +0x30 strip vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub offset: u32,
    pub head: [u32; 13],
    pub nodes: Vec<Node>,
    /// every mesh the nodes name, once each (by offset)
    pub meshes: Vec<Mesh>,
}

/// A particle model's mesh (`ResolveParticleMesh` 0x255cc0): +4 / +0x10 its box, +0x1c flags, +0x20 volumes, +0x24
/// its data (an offset from the mesh).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleMesh {
    pub offset: u32,
    pub words: [u32; 10],
}

/// A particle model (`ResolveParticleModel` 0x253c50): +0 length, +4 nodes, +8 their offset, +0x10 the animation
/// time; a node (4 words): parent, mesh (offset from the node), animation, matrix (−1 identity).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleModel {
    pub offset: u32,
    pub head: [u32; 5],
    pub nodes: Vec<([u32; 4], Option<ParticleMesh>, Option<Animation>)>,
}

/// A camera (`ResolveCamera` 0x254398): length, translation, rotation, type, focal length, aspect, aperture, clip
/// planes, interest point, up vector (words 0..0x13), +0x54 an animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub offset: u32,
    pub words: [u32; 22],
    pub animation: Option<Animation>,
}

/// The hash table (`ResolveHashTable` 0x2546a8): its length, then six (count, offset) lists of (hash, index):
/// patches, instances, models (by id), lights, splines, cameras.
#[derive(Clone, Debug, PartialEq)]
pub struct HashTable {
    pub length: u32,
    pub lists: [Vec<(u32, u32)>; 6],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pbd {
    pub header: Header,
    pub patches: Vec<Vec<u8>>,
    pub instances: Vec<[u32; 64]>,
    pub particle_instances: Vec<[u32; 28]>,
    pub materials: Vec<[u32; 18]>,
    /// each block: its material indices
    pub material_blocks: Vec<Vec<u32>>,
    pub lights: Vec<[u32; 23]>,
    pub splines: Vec<[u32; 10]>,
    pub segments: Vec<[u32; 32]>,
    /// each flip: its words after the count
    pub texture_flips: Vec<Vec<u32>>,
    /// the model pointer table (offsets from the models block)
    pub model_pointers: Vec<u32>,
    pub models: Vec<Model>,
    pub particle_model_pointers: Vec<u32>,
    pub particle_models: Vec<ParticleModel>,
    pub camera_pointers: Vec<u32>,
    pub cameras: Vec<Camera>,
    pub hash: Option<HashTable>,
}

fn table<const N: usize>(b: &[u8], off: u32, n: u32, size: usize) -> Option<Vec<[u32; N]>> {
    if off == 0 {
        return Some(vec![]);
    }
    (0..n as usize).map(|i| words::<N>(b, off as usize + size * i)).collect()
}

fn runs(b: &[u8], off: u32, n: u32) -> Option<Vec<Vec<u32>>> {
    if off == 0 {
        return Some(vec![]);
    }
    let mut p = off as usize;
    let mut out = vec![];
    for _ in 0..n {
        let c = word(b, p)? as usize;
        out.push((0..c).map(|k| word(b, p + 4 + 4 * k)).collect::<Option<Vec<_>>>()?);
        p += 4 * (c + 1);
    }
    Some(out)
}

fn animation(b: &[u8], at: usize) -> Option<Animation> {
    let head = words::<9>(b, at)?;
    let n = head[7] as i32;
    let t = at + head[8] as usize;
    let channels = (0..n.max(0) as usize)
        .map(|k| {
            let c = at as u32 + word(b, t + 4 * k)?;
            Some((c, word(b, c as usize + 4)?))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Animation { offset: at as u32, head, channels })
}

fn mesh(b: &[u8], at: usize) -> Option<Mesh> {
    let head = words::<11>(b, at)?;
    let t = at + head[10] as usize;
    let contexts = (0..(head[8] as i32).max(0) as usize)
        .map(|k| {
            let slot = t + 4 * k;
            let c = slot as u32 + word(b, slot)?;
            Some(Context { offset: c, words: words::<4>(b, c as usize)? })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Mesh { offset: at as u32, head, contexts })
}

impl Pbd {
    pub fn parse(b: &[u8]) -> Option<Pbd> {
        let header = Header { version: [*b.first()?, *b.get(1)?, *b.get(2)?], format: *b.get(3)?, counts: words::<15>(b, 4)?, offsets: words::<18>(b, 0x40)? };
        let (c, o) = (|a| header.count(a), |a| header.offset(a));
        let patches = if o(0x44) == 0 { vec![] } else { (0..c(8) as usize).map(|i| b.get(o(0x44) as usize + PATCH * i..o(0x44) as usize + PATCH * (i + 1)).map(|s| s.to_vec())).collect::<Option<Vec<_>>>()? };
        let model_pointers = if o(0x6c) == 0 { vec![] } else { (0..c(0x2c) as usize).map(|i| word(b, o(0x68) as usize + 4 * i)).collect::<Option<Vec<_>>>()? };
        // the models follow one another (each +0 its length)
        let mut models = vec![];
        if o(0x6c) != 0 {
            let mut p = o(0x6c) as usize;
            for _ in 0..c(0x2c) {
                let head = words::<13>(b, p)?;
                let h = p + head[2] as usize;
                let mut nodes = vec![];
                let mut meshes: Vec<Mesh> = vec![];
                for k in 0..(head[1] as i32).max(0) as usize {
                    let e = h + 24 * k;
                    let w = words::<6>(b, e)?;
                    for (j, lod) in w[1..4].iter().enumerate() {
                        if *lod != 0 {
                            let m = e as u32 + lod;
                            let dup = w[1..1 + j].iter().any(|x| *x != 0 && e as u32 + x == m);
                            if !dup && !meshes.iter().any(|x| x.offset == m) {
                                meshes.push(mesh(b, m as usize)?);
                            }
                        }
                    }
                    let anim = if w[4] != 0 { Some(animation(b, e + w[4] as usize)?) } else { None };
                    nodes.push(Node { offset: e as u32, words: w, animation: anim });
                }
                models.push(Model { offset: p as u32, head, nodes, meshes });
                p += head[0] as usize;
            }
        }
        let particle_model_pointers = if o(0x74) == 0 { vec![] } else { (0..c(0x30) as usize).map(|i| word(b, o(0x70) as usize + 4 * i)).collect::<Option<Vec<_>>>()? };
        let mut particle_models = vec![];
        if o(0x74) != 0 {
            let mut p = o(0x74) as usize;
            for _ in 0..c(0x30) {
                let head = words::<5>(b, p)?;
                let h = p + head[2] as usize;
                let nodes = (0..(head[1] as i32).max(0) as usize)
                    .map(|k| {
                        let e = h + 16 * k;
                        let w = words::<4>(b, e)?;
                        let m = if w[1] != 0 { Some(ParticleMesh { offset: e as u32 + w[1], words: words::<10>(b, e + w[1] as usize)? }) } else { None };
                        let a = if w[2] != 0 { Some(animation(b, e + w[2] as usize)?) } else { None };
                        Some((w, m, a))
                    })
                    .collect::<Option<Vec<_>>>()?;
                particle_models.push(ParticleModel { offset: p as u32, head, nodes });
                p += head[0] as usize;
            }
        }
        let camera_pointers = if o(0x7c) == 0 { vec![] } else { (0..c(0x38) as usize).map(|i| word(b, o(0x78) as usize + 4 * i)).collect::<Option<Vec<_>>>()? };
        let mut cameras = vec![];
        if o(0x7c) != 0 {
            let mut p = o(0x7c) as usize;
            for _ in 0..c(0x38) {
                let w = words::<22>(b, p)?;
                let a = if w[0x15] != 0 { Some(animation(b, p + w[0x15] as usize)?) } else { None };
                cameras.push(Camera { offset: p as u32, words: w, animation: a });
                p += w[0] as usize;
            }
        }
        let hash = if o(0x80) == 0 {
            None
        } else {
            let h = o(0x80) as usize;
            let mut lists: [Vec<(u32, u32)>; 6] = Default::default();
            for (k, l) in lists.iter_mut().enumerate() {
                let n = word(b, h + 4 + 8 * k)? as i32;
                let t = h + word(b, h + 8 + 8 * k)? as usize;
                *l = (0..n.max(0) as usize).map(|i| Some((word(b, t + 8 * i)?, word(b, t + 8 * i + 4)?))).collect::<Option<Vec<_>>>()?;
            }
            Some(HashTable { length: word(b, h)?, lists })
        };
        Some(Pbd {
            patches,
            instances: table::<64>(b, o(0x48), c(0xc), INSTANCE)?,
            particle_instances: table::<28>(b, o(0x4c), c(0x10), PARTICLE_INSTANCE)?,
            materials: table::<18>(b, o(0x50), c(0x14), MATERIAL)?,
            material_blocks: runs(b, o(0x54), c(0x18))?,
            lights: table::<23>(b, o(0x58), c(0x1c), LIGHT)?,
            splines: table::<10>(b, o(0x5c), c(0x20), SPLINE)?,
            segments: table::<32>(b, o(0x60), c(0x24), SEGMENT)?,
            texture_flips: runs(b, o(0x64), c(0x28))?,
            model_pointers,
            models,
            particle_model_pointers,
            particle_models,
            camera_pointers,
            cameras,
            hash,
            header,
        })
    }
}

/// A patch record (0x1c0 bytes, row E3a2): +0 its rectangle in its lightmap page (x, y, width, height), +0x10 the
/// texture coordinates of its four corners (u, v, 1, 1), +0x50 the surface's 16 polynomial coefficients (x, y, z, 1
/// each; highest power first: [15 − (4a + b)] multiplies sᵃ·tᵇ), +0x150 its box (min, max), +0x168 its four corners
/// (control points 0, 3, 12, 15), +0x1a8 the surface type, +0x1ac flags (bit 31: show-off only), +0x1b0 the texture
/// (low half) and lightmap page (high half), +0x1b4 −1 on the disc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchRec {
    pub lightmap: [f32; 4],
    pub uv: [[f32; 4]; 4],
    pub coeffs: [[f32; 4]; 16],
    pub bbox: [f32; 6],
    pub corners: [[f32; 4]; 4],
    pub surface: u32,
    pub flags: u32,
    pub texture: u16,
    pub lightmap_page: u16,
    pub x1b4: u32,
}

impl PatchRec {
    pub fn parse(r: &[u8]) -> Option<PatchRec> {
        let f = |o: usize| word(r, o).map(f32::from_bits);
        let v4 = |o: usize| Some([f(o)?, f(o + 4)?, f(o + 8)?, f(o + 12)?]);
        let mut uv = [[0.0; 4]; 4];
        for (k, x) in uv.iter_mut().enumerate() {
            *x = v4(0x10 + 16 * k)?;
        }
        let mut coeffs = [[0.0; 4]; 16];
        for (k, x) in coeffs.iter_mut().enumerate() {
            *x = v4(0x50 + 16 * k)?;
        }
        let mut bbox = [0.0; 6];
        for (k, x) in bbox.iter_mut().enumerate() {
            *x = f(0x150 + 4 * k)?;
        }
        let mut corners = [[0.0; 4]; 4];
        for (k, x) in corners.iter_mut().enumerate() {
            *x = v4(0x168 + 16 * k)?;
        }
        let t = word(r, 0x1b0)?;
        Some(PatchRec { lightmap: v4(0)?, uv, coeffs, bbox, corners, surface: word(r, 0x1a8)?, flags: word(r, 0x1ac)?, texture: t as u16, lightmap_page: (t >> 16) as u16, x1b4: word(r, 0x1b4)? })
    }

    /// The coefficient of sᵃ·tᵇ.
    pub fn coeff(&self, a: usize, b: usize) -> [f32; 3] {
        let c = self.coeffs[15 - (4 * a + b)];
        [c[0], c[1], c[2]]
    }

    /// The patch's 16 Bézier control points (row-major, s by rows), converted from the coefficients (in double
    /// precision; the game evaluates the coefficients themselves).
    pub fn bezier(&self) -> [[f32; 3]; 16] {
        // power → Bézier: P = M · C · Mᵀ with M the inverse of the Bernstein-to-power matrix
        const M: [[f64; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [1.0, 1.0 / 3.0, 0.0, 0.0], [1.0, 2.0 / 3.0, 1.0 / 3.0, 0.0], [1.0, 1.0, 1.0, 1.0]];
        let mut out = [[0.0f32; 3]; 16];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..3 {
                    let mut s = 0.0f64;
                    for a in 0..4 {
                        for b in 0..4 {
                            s += M[i][a] * M[j][b] * self.coeff(a, b)[k] as f64;
                        }
                    }
                    out[4 * i + j][k] = s as f32;
                }
            }
        }
        out
    }

    pub fn showoff_only(&self) -> bool {
        self.flags & 0x8000_0000 != 0
    }
}

/// An instance record (0x100 bytes, row E3a2): +0 its matrix (rows: the axes scaled, then the position), +0x40 the
/// three light directions as columns (+0x70 the ambient's (0, 0, 0, 1)), +0x80 / +0x90 / +0xa0 the three lights'
/// colours, +0xb0 the ambient colour (128 = 1.0), +0xc0 its model id, +0xc4 / +0xc8 the previous and next instance
/// (-1 none), +0xcc its box (min, max), +0xf0 its material block. Its collision, flags and scripts are in the
/// `.ssf` physics record for it ([`crate::ssf::Physics`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceRec {
    pub matrix: [[f32; 4]; 4],
    pub light_dirs: [[f32; 4]; 4],
    pub light_colours: [[f32; 4]; 3],
    pub ambient: [f32; 4],
    pub model: i32,
    pub prev: i32,
    pub next: i32,
    pub bbox: [f32; 6],
    pub material_block: u32,
}

impl InstanceRec {
    pub fn from_words(w: &[u32; 64]) -> InstanceRec {
        let f = |k: usize| f32::from_bits(w[k]);
        let v4 = |k: usize| [f(k), f(k + 1), f(k + 2), f(k + 3)];
        InstanceRec {
            matrix: [v4(0), v4(4), v4(8), v4(12)],
            light_dirs: [v4(16), v4(20), v4(24), v4(28)],
            light_colours: [v4(32), v4(36), v4(40)],
            ambient: v4(44),
            model: w[48] as i32,
            prev: w[49] as i32,
            next: w[50] as i32,
            bbox: [f(51), f(52), f(53), f(54), f(55), f(56)],
            material_block: w[60],
        }
    }

    /// Light `k`: its direction (column k of the light matrix).
    pub fn light_dir(&self, k: usize) -> [f32; 3] {
        [self.light_dirs[0][k], self.light_dirs[1][k], self.light_dirs[2][k]]
    }
}

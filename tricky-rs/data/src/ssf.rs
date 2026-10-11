//! A course's `.ssf` file (`data/models/<course>.ssf` in its archive) as `Course_LoadSphereTrees` 0x25f598 reads it
//! (board row E3c1): the trigger scripts, each object's event slots and physics record, the collision meshes, the
//! sphere trees' places and the spline segments' words. Checked by running the game's loader in the function runner
//! on every course (`tricky-game/tests/ssf.rs`).
//!
//! The header (19 words; offsets from the start of the file): +4 the version (u16 0x1500); +0xc/+0x10 event slots
//! (0x1c bytes each); +0x14/+0x18 sphere-tree groups and +0x1c/+0x20 collision groups (12 bytes: offset, bytes,
//! count); +0x24/+0x28 effect headers (8 bytes: op count, offset from the end of this table); +0x2c/+0x30 script
//! functions (0x18 bytes: op count, offset as the headers', a 16-byte name); +0x34/+0x38 physics records (0x18
//! bytes); +0x3c/+0x40 each object's physics record (one word an object; must equal the course's object count);
//! +0x44/+0x48 spline segments (8 bytes).

use std::fmt;

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

/// One script op as the file has it: word 0 its number, word 1 its size in bytes (the run steps by it), then its
/// arguments (`TriggerScript_ExecOp` 0x13bfd0 reads words 2, 3, 4 …).
#[derive(Clone, PartialEq, Eq)]
pub struct Op {
    pub op: u32,
    pub size: u32,
    /// the words after the first two (size / 4 − 2 of them)
    pub args: Vec<u32>,
}

impl fmt::Debug for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "op {} {:x?}", self.op, self.args)
    }
}

/// A script function (`World_FindScript` 0x260dd8 finds it by name, case-blind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub ops: Vec<Op>,
}

/// An object's physics record (0x18 bytes; instance+0xec points at it, its word +8 is copied to instance+0xe8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Physics(pub [u32; 6]);

impl Physics {
    /// +8: the flags (bit 0x2000 of its upper half marks the object's model, model+0x10 |= 0x10)
    pub fn flags(&self) -> u32 {
        self.0[2]
    }
    /// +0x14 (signed half): the event-slot record (`Instance_GetEventScript` 0x25fe78), −1 none
    pub fn slot(&self) -> i16 {
        self.0[5] as u16 as i16
    }
    /// +0: the mass (1e30 for things that never move)
    pub fn mass(&self) -> f32 {
        f32::from_bits(self.0[0])
    }
    /// +4: how much a rider bounces off it
    pub fn bounce(&self) -> f32 {
        f32::from_bits(self.0[1])
    }
    /// the flags' defaults (upper half): 0x1 shown, 0x20 riders collide, 0x80 riders bounce off, 0x1000 (unknown),
    /// 0x2000 UV-scrolled (row E3a2)
    pub fn defaults(&self) -> u16 {
        (self.0[2] >> 16) as u16
    }
    /// +0xc: the surface type its collision uses (-1: never ground)
    pub fn surface(&self) -> i32 {
        self.0[3] as i32
    }
    /// +0x10 (half): the collision mode (3: a rigid body that gets knocked about)
    pub fn collision_mode(&self) -> i16 {
        self.0[4] as u16 as i16
    }
    /// +0x12 (half): with mode 3 its sphere-tree group (the physics), else its collision-mesh group (-1 none)
    pub fn group(&self) -> i16 {
        (self.0[4] >> 16) as u16 as i16
    }
}

/// A collision mesh: `tris` triangles of three vertex indices, the vertices, and one plane (normal, w) per triangle.
#[derive(Clone, Debug, PartialEq)]
pub struct CollMesh {
    pub tris: Vec<[u32; 3]>,
    /// bytes after the triangles before the vertices (keeps them 16-byte aligned)
    pub pad: u32,
    pub verts: Vec<[f32; 4]>,
    pub planes: Vec<[f32; 4]>,
}

/// A group of the sphere trees or collision meshes: where its data starts in the file, its size in bytes and how
/// many it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub offset: u32,
    pub bytes: u32,
    pub count: u32,
}

/// The file as the loader leaves it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ssf {
    /// +0: the first word (0x1e0200 on the disc), +8 (a build stamp)
    pub magic: u32,
    pub stamp: u32,
    /// each event slot record: the effect header run for events 0..6 (−1 none; `Instance_GetEventScript`'s third
    /// argument picks one)
    pub slots: Vec<[i32; 7]>,
    pub sphere_groups: Vec<Group>,
    /// each sphere-tree group's trees (`SphTree_Load` 0x239340, one after another)
    pub sphere_trees: Vec<Vec<SphTree>>,
    pub coll_groups: Vec<Group>,
    /// each collision group's meshes
    pub collision: Vec<Vec<CollMesh>>,
    pub effects: Vec<Vec<Op>>,
    pub functions: Vec<Function>,
    pub physics: Vec<Physics>,
    /// each object's physics record (an index into `physics`)
    pub object_physics: Vec<u32>,
    /// spline segments: word 0 (the loader swaps its halves into segment+0x18), word 1 (segment+0x24)
    pub spline_segments: Vec<[u32; 2]>,
}

/// Why the loader would not take the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SsfError {
    /// not version 0x1500
    Version(u16),
    /// +0x3c is not the course's object count (the loader then frees it and the course runs without one)
    Objects(u32),
    /// a table or op runs past the end
    Truncated,
}

fn ops(b: &[u8], at: usize, count: u32) -> Option<Vec<Op>> {
    let mut out = Vec::new();
    let mut p = at;
    for _ in 0..count {
        let op = word(b, p)?;
        let size = word(b, p + 4)?;
        let n = (size as usize / 4).saturating_sub(2);
        let args = (0..n).map(|k| word(b, p + 8 + 4 * k)).collect::<Option<Vec<u32>>>()?;
        out.push(Op { op, size, args });
        p = p.checked_add(size as usize)?;
    }
    Some(out)
}

impl Ssf {
    /// Reads the file for a course of `objects` objects.
    pub fn parse(b: &[u8], objects: u32) -> Result<Ssf, SsfError> {
        let version = b.get(4..6).map(|v| u16::from_le_bytes([v[0], v[1]])).ok_or(SsfError::Truncated)?;
        if version != 0x1500 {
            return Err(SsfError::Version(version));
        }
        let h = |o: usize| word(b, o).ok_or(SsfError::Truncated);
        if h(0x3c)? != objects {
            return Err(SsfError::Objects(h(0x3c)?));
        }
        let t = SsfError::Truncated;
        let table = |count: u32, off: u32, size: usize| -> Vec<usize> { (0..count as usize).map(|i| off as usize + i * size).collect() };
        let slots = table(h(0xc)?, h(0x10)?, 0x1c)
            .into_iter()
            .map(|o| {
                let mut s = [0i32; 7];
                for (k, v) in s.iter_mut().enumerate() {
                    *v = word(b, o + 4 * k)? as i32;
                }
                Some(s)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(t)?;
        let groups = |count: u32, off: u32| {
            table(count, off, 12).into_iter().map(|o| Some(Group { offset: word(b, o)?, bytes: word(b, o + 4)?, count: word(b, o + 8)? })).collect::<Option<Vec<_>>>()
        };
        let sphere_groups = groups(h(0x14)?, h(0x18)?).ok_or(t)?;
        let sphere_trees = sphere_groups
            .iter()
            .map(|g| {
                let mut p = g.offset as usize;
                (0..g.count)
                    .map(|_| {
                        let tree = SphTree::parse(b, p)?;
                        p += tree.size();
                        Some(tree)
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(t)?;
        let coll_groups = groups(h(0x1c)?, h(0x20)?).ok_or(t)?;
        let collision = coll_groups
            .iter()
            .map(|g| {
                let mut p = g.offset as usize;
                (0..g.count)
                    .map(|_| {
                        let n = word(b, p)? as usize;
                        let m = word(b, p + 4)? as usize;
                        let pad = word(b, p + 8)?;
                        p += 12;
                        let tris = (0..n).map(|i| Some([word(b, p + 12 * i)?, word(b, p + 12 * i + 4)?, word(b, p + 12 * i + 8)?])).collect::<Option<Vec<_>>>()?;
                        p += pad as usize + n * 12;
                        let v4 = |p: usize, k: usize| -> Option<Vec<[f32; 4]>> {
                            (0..k).map(|i| Some([0, 1, 2, 3].map(|j| f32::from_bits(word(b, p + 16 * i + 4 * j).unwrap_or(0))))).collect::<Option<Vec<_>>>().filter(|_| b.len() >= p + 16 * k)
                        };
                        let verts = v4(p, m)?;
                        p += m * 16;
                        let planes = v4(p, n)?;
                        p += n * 16;
                        Some(CollMesh { tris, pad, verts, planes })
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(t)?;
        let (n_eff, eff) = (h(0x24)?, h(0x28)?);
        let base = eff as usize + n_eff as usize * 8;
        let effects = table(n_eff, eff, 8).into_iter().map(|o| ops(b, base + word(b, o + 4)? as usize, word(b, o)?)).collect::<Option<Vec<_>>>().ok_or(t)?;
        let functions = table(h(0x2c)?, h(0x30)?, 0x18)
            .into_iter()
            .map(|o| {
                let name = b.get(o + 8..o + 0x18)?;
                let name = String::from_utf8_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(16)]).to_string();
                Some(Function { name, ops: ops(b, base + word(b, o + 4)? as usize, word(b, o)?)? })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(t)?;
        let physics = table(h(0x34)?, h(0x38)?, 0x18)
            .into_iter()
            .map(|o| {
                let mut r = [0u32; 6];
                for (k, v) in r.iter_mut().enumerate() {
                    *v = word(b, o + 4 * k)?;
                }
                Some(Physics(r))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(t)?;
        let object_physics = table(objects, h(0x40)?, 4).into_iter().map(|o| word(b, o)).collect::<Option<Vec<_>>>().ok_or(t)?;
        let spline_segments = table(h(0x44)?, h(0x48)?, 8).into_iter().map(|o| Some([word(b, o)?, word(b, o + 4)?])).collect::<Option<Vec<_>>>().ok_or(t)?;
        Ok(Ssf { magic: h(0)?, stamp: h(8)?, slots, sphere_groups, sphere_trees, coll_groups, collision, effects, functions, physics, object_physics, spline_segments })
    }

    /// `World_FindScript` 0x260dd8: the function named `name` (case-blind; the first of that name), else −1.
    pub fn find_function(&self, name: &str) -> i32 {
        self.functions.iter().position(|f| f.name.eq_ignore_ascii_case(name)).map_or(-1, |i| i as i32)
    }

    /// `Instance_GetEventScript` 0x25fe78: the effect header object `object` runs on event `event`, −1 none.
    pub fn event_script(&self, object: usize, event: usize) -> i32 {
        let Some(p) = self.object_physics.get(object).and_then(|p| self.physics.get(*p as usize)) else { return -1 };
        let s = p.slot();
        if s < 0 {
            return -1;
        }
        self.slots.get(s as usize).map_or(-1, |r| r[event])
    }

    /// Segment+0x18 as the loader writes it: word 0 with its halves swapped (upper half kept in place, the lower
    /// half the upper half moved down).
    pub fn segment_x18(w0: u32) -> u32 {
        (w0 & 0xffff_0000) | (w0 >> 16)
    }
}

/// A sphere tree as `SphTree_Load` 0x239340 reads it: an octree of spheres around an object's shape, with the
/// object's mass properties. Header (0x70 bytes): +0 and +4 the sizes of the two data blocks after the levels,
/// +8 a flag (0: the occupancy bytes are the data itself; else the tree's cache builds them), +0xc the depth, +0x10
/// the centre, +0x1c a second point (the centre of mass), +0x28 and +0x4c two 3×3 matrices (the inertia tensor and
/// its inverse); then one 12-byte record per level (depth + 1: the spheres' radius, the cells' half-size, the index
/// stride of the level's children) and the data.
#[derive(Clone, Debug, PartialEq)]
pub struct SphTree {
    pub size_a: u32,
    pub size_b: u32,
    pub flag: u32,
    pub depth: u32,
    pub centre: [f32; 3],
    pub mass_centre: [f32; 3],
    pub inertia: [f32; 9],
    pub inv_inertia: [f32; 9],
    /// per level (radius, half-size, child stride)
    pub levels: Vec<(f32, f32, i32)>,
    /// the `size_a + size_b` bytes after the levels
    pub data: Vec<u8>,
}

impl SphTree {
    fn parse(b: &[u8], at: usize) -> Option<SphTree> {
        let w = |o: usize| word(b, at + o);
        let f = |o: usize| w(o).map(f32::from_bits);
        let v3 = |o: usize| Some([f(o)?, f(o + 4)?, f(o + 8)?]);
        let m9 = |o: usize| -> Option<[f32; 9]> {
            let mut m = [0.0; 9];
            for (k, v) in m.iter_mut().enumerate() {
                *v = f(o + 4 * k)?;
            }
            Some(m)
        };
        let (size_a, size_b, flag, depth) = (w(0)?, w(4)?, w(8)?, w(0xc)?);
        let levels = (0..depth as usize + 1).map(|i| Some((f(0x70 + 12 * i)?, f(0x74 + 12 * i)?, w(0x78 + 12 * i)? as i32))).collect::<Option<Vec<_>>>()?;
        let d0 = at + 0x70 + (depth as usize + 1) * 12;
        let data = b.get(d0..d0 + size_a as usize + size_b as usize)?.to_vec();
        Some(SphTree { size_a, size_b, flag, depth, centre: v3(0x10)?, mass_centre: v3(0x1c)?, inertia: m9(0x28)?, inv_inertia: m9(0x4c)?, levels, data })
    }

    /// The bytes the tree takes (what `SphTree_Load` returns).
    pub fn size(&self) -> usize {
        0x70 + (self.depth as usize + 1) * 12 + self.size_a as usize + self.size_b as usize
    }
}

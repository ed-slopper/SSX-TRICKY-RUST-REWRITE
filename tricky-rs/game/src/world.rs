//! The world's casts (board row E7a2): `World_RayCast` 0x25aff8 (the hit nearest the query's target among the
//! patches and solid objects of the grid cells around it), `World_RayCastInstance` 0x25bf48 (one object, part by
//! part in each part's frame) and `World_QueryInstances` 0x25b878 (the first touch object hit), on the course's
//! `.ltg` grid, `.pbd` records and `.ssf` physics and collision meshes. Checked against the game in the function
//! runner on the disc's courses (`tests/world.rs`). The query is a [`Shape`] (`cWorldLine` here, row E7a); a patch's
//! own test goes through the patch cache (row E7c) and is the caller's ([`WorldEnv::patch`]).

pub use crate::terrain::LastHit;
use crate::sphtree::SphereTree;
use crate::terrain::{PatchGrid, PatchHit};
use crate::worldline::{Hit, WorldLine};

type V4 = [f32; 4];

/// A query's virtual functions (row E7a).
pub trait Shape {
    /// +0x18
    fn target(&self) -> f32;
    /// +0x10 / +0x20 / +0x30: its box (max, min) and middle
    fn hi(&self) -> V4;
    fn lo(&self) -> V4;
    fn mid(&self) -> V4;
    /// +0x08: into a frame (the inverse of an object part's)
    fn push(&mut self, m: [V4; 4]);
    /// +0x10 (vtable): back to the world
    fn pop(&mut self);
    /// +0x20: whether to throw a hit away (given the hit's normal)
    fn reject(&self, normal: V4) -> bool;
    /// +0x40
    fn boxed(&self, lo: V4, hi: V4) -> Option<Hit>;
    /// +0x38: a sphere tree
    fn sphere_tree(&self, tree: &SphereTree) -> Option<Hit>;
    /// `World_RayCastMeshTris` with this shape's +0x28 / +0x30
    fn mesh(&self, m: &CollMesh, either_side: bool) -> Option<Hit>;
    /// +0x48: a patch's own test on its cache entry (`terrain::patch_test`, row E7c1); `id` is what `last` notes
    fn patch(&self, grid: &mut PatchGrid, id: u32, last: Option<&mut LastHit>) -> Option<PatchHit>;
}

impl Shape for WorldLine {
    fn target(&self) -> f32 {
        self.target
    }
    fn hi(&self) -> V4 {
        self.hi
    }
    fn lo(&self) -> V4 {
        self.lo
    }
    fn mid(&self) -> V4 {
        self.mid
    }
    fn push(&mut self, m: [V4; 4]) {
        WorldLine::push(self, m)
    }
    fn pop(&mut self) {
        WorldLine::pop(self)
    }
    fn reject(&self, _: V4) -> bool {
        false
    }
    fn boxed(&self, lo: V4, hi: V4) -> Option<Hit> {
        WorldLine::boxed(self, lo, hi)
    }
    fn sphere_tree(&self, _: &SphereTree) -> Option<Hit> {
        None
    }
    fn mesh(&self, m: &CollMesh, either_side: bool) -> Option<Hit> {
        WorldLine::mesh(self, &m.tris, &m.verts, &m.planes, either_side)
    }
    fn patch(&self, grid: &mut PatchGrid, id: u32, last: Option<&mut LastHit>) -> Option<PatchHit> {
        crate::terrain::patch_test(self, grid, id, last)
    }
}

/// An `.ssf` collision mesh (row E3c1).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CollMesh {
    pub tris: Vec<[u32; 3]>,
    pub verts: Vec<V4>,
    pub planes: Vec<V4>,
}

/// An INode of the grid: its box (+0, +0xc), counts (+0x24 patches, +0x26 solid objects, +0x28 with the
/// show-off ones), its patch and object lists (indices), and whether `World_QueryInstances` passes it by (the
/// words at +0x24, +0x28, the half at +0x2c and the word at +0x48 all zero).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Node {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub patches: Vec<usize>,
    pub objects: Vec<usize>,
    pub n_objects: i16,
    pub n_objects_showoff: i16,
    pub blank: bool,
}

/// A grid cell (a TGroup): its box and INodes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Cell {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub nodes: Vec<Node>,
}

/// A patch as the casts see it: show-off only (+0x1ac bit 31), surface (+0x1a8), box (+0x150, +0x15c).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Patch {
    pub show_off: bool,
    pub surface: i32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// A model part (node): its parent, whether an animation drives it (+0x10), its matrix (+0x14; none: identity),
/// and its mesh (+4): the mesh's box (+4, +0x10) and flags (+0x1c; bit 0: either side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub parent: i32,
    pub animated: bool,
    pub matrix: Option<[V4; 4]>,
    pub mesh: Option<PartMesh>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartMesh {
    pub lo: V4,
    pub hi: V4,
    pub flags: u32,
}

/// An object (instance): its matrix, box (+0xcc, +0xd8), model's parts, the flags at +0xe8, its physics record
/// (+0xec: surface +0xc, collision mode +0x10, group +0x12) and, with a controller (+0xe4), that controller's +0x14.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub rows: [V4; 4],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub model: usize,
    pub flags: u32,
    pub surface: i32,
    pub mode: i16,
    pub group: i16,
    pub controller: Option<u32>,
}

/// The course as the casts see it (the object at 0x346488): the grid (+0: the `.ltg`: +4 / +8 its corner, +0x28
/// the cell size, +0x2c / +0x30 width and height, +0x48 INodes a cell), patches, objects, models, collision groups.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct World {
    pub origin: [f32; 2],
    pub size: f32,
    pub width: i32,
    pub height: i32,
    pub nodes_per_cell: i32,
    pub cells: Vec<Option<Cell>>,
    pub patches: Vec<Patch>,
    pub objects: Vec<Object>,
    pub models: Vec<Vec<Part>>,
    pub collision: Vec<Vec<CollMesh>>,
    /// the sphere-tree groups (course+0x18), for collision mode 3
    pub trees: Vec<Vec<SphereTree>>,
}

/// The result record a cast fills (15 words): +0 what was hit (0 ground, 1 a touch object, 2 an animated part),
/// +0x10 the point, +0x20 the normal, +0x30 the sphere tree (group, n), +0x34 the surface (−1 for a touch object),
/// +0x38 the object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastHit {
    pub kind: u32,
    pub pos: V4,
    pub normal: V4,
    pub tree: Option<(usize, usize)>,
    pub surface: i32,
    pub object: Option<usize>,
}

/// What the casts need from outside.
pub trait WorldEnv {
    /// The shape's +0x48 for patch `i` (through the patch cache, `World_LookupCollCache`; `terrain::patch_test`),
    /// given the caller's last-hit record.
    fn patch(&mut self, q: &dyn Shape, i: usize, last: Option<&mut LastHit>) -> Option<Hit>;
    /// An animated object's `n`-th animated part's frame (`MeshAnimNode_GetFrameMatrix` 0x1998d0).
    fn frame(&mut self, object: usize, n: usize) -> [V4; 4];
}

const IDENTITY: [V4; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Rows `m` through the frame `f`.
fn through(f: [V4; 4], m: [V4; 4]) -> [V4; 4] {
    m.map(|v| apply(f, v))
}

fn apply(f: [V4; 4], v: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((f[0][i] * v[0] + f[1][i] * v[1]) + f[2][i] * v[2]) + f[3][i] * v[3])
}

/// min < the query's max and the query's min < max, on x, y and z.
fn overlaps(min: [f32; 3], max: [f32; 3], q: &dyn Shape) -> bool {
    let (lo, hi) = (q.lo(), q.hi());
    (0..3).all(|k| min[k] < hi[k] && lo[k] < max[k])
}

impl World {
    /// The cells the casts look at: around the query's middle, from two cells below to the one it is in
    /// (index (p − corner + size) / size, so one either side), clamped to the grid.
    fn around(&self, mid: V4) -> (i32, i32, i32, i32) {
        let cx = (((mid[0] - self.origin[0]) + self.size) / self.size) as i32;
        let cy = (((mid[1] - self.origin[1]) + self.size) / self.size) as i32;
        let x_hi = cx.max(0).min(self.width - 1);
        let x_lo = cx.wrapping_sub(2).max(0);
        let y_hi = cy.max(0).min(self.height - 1);
        let y_lo = cy.wrapping_sub(2).max(0);
        (x_lo, x_hi, y_lo, y_hi)
    }

    fn cell_at(&self, x: i32, y: i32) -> (i32, Option<&Cell>) {
        let cx = if x < 0 { 0 } else { x.min(self.width - 1) };
        let cy = if y < 0 { 0 } else { y.min(self.height - 1) };
        let i = cy * self.width + cx;
        (i, self.cells.get(i as usize).and_then(|c| c.as_ref()))
    }

    /// `World_RayCast` 0x25aff8: for each cell (x outer, y inner) whose box meets the query's, each INode with
    /// patches or objects (the show-off count in show-off modes 3 / 5) whose box meets it: its patches (not
    /// show-off-only ones outside those modes, nor surface 17) whose boxes meet it, through the shape's patch
    /// test (each hit noted in `last`), then its solid objects (flag 0x20, surface not −1) whose boxes meet it,
    /// through [`Self::ray_cast_instance`]. A hit nearer the target than the best (and not rejected) is kept;
    /// with a target below 0 the first one is taken. −1 for none.
    pub fn ray_cast(&self, q: &mut dyn Shape, show_off: bool, mut last: Option<&mut LastHit>, env: &mut dyn WorldEnv) -> (f32, Option<CastHit>) {
        let target = q.target();
        let mut best = 1e20f32;
        let mut out = None;
        let (x_lo, x_hi, y_lo, y_hi) = self.around(q.mid());
        let mut x = x_lo;
        while x <= x_hi {
            let mut y = y_lo;
            while y <= y_hi {
                let (ci, cell) = self.cell_at(x, y);
                if let Some(cell) = cell.filter(|c| overlaps(c.min, c.max, q)) {
                    for k in 0..self.nodes_per_cell.max(0) as usize {
                        let Some(node) = cell.nodes.get(k) else { break };
                        let n_obj = if show_off { node.n_objects_showoff } else { node.n_objects };
                        if (node.patches.is_empty() && n_obj == 0) || !overlaps(node.min, node.max, q) {
                            continue;
                        }
                        for (pi, &p) in node.patches.iter().enumerate() {
                            let patch = &self.patches[p];
                            if patch.show_off && !show_off {
                                continue;
                            }
                            if patch.surface == 0x11 || !overlaps(patch.min, patch.max, q) {
                                continue;
                            }
                            let Some(h) = env.patch(q, p, last.as_deref_mut()) else { continue };
                            if let Some(l) = last.as_deref_mut() {
                                let (cx, cy) = (if x < 0 { 0 } else { x.min(self.width - 1) }, if y < 0 { 0 } else { y.min(self.height - 1) });
                                l.cell = (cy as i16).wrapping_mul(self.width as i16).wrapping_add(cx as i16);
                                l.node = k as i16;
                                l.index = pi as i16;
                            }
                            let _ = ci;
                            if (h.t - target).abs() < (best - target).abs() && !q.reject(h.normal) {
                                out = Some(CastHit { kind: 0, pos: h.pos, normal: h.normal, tree: None, surface: patch.surface, object: None });
                                best = h.t;
                                if target < 0.0 {
                                    return (h.t, out);
                                }
                            }
                        }
                        for &o in node.objects.iter().take(n_obj.max(0) as usize) {
                            let obj = &self.objects[o];
                            if obj.surface == -1 || obj.flags & 0x20 == 0 || !overlaps(obj.min, obj.max, q) {
                                continue;
                            }
                            let (t, h) = self.ray_cast_instance(q, o, env);
                            if 0.0 <= t && (t - target).abs() < (best - target).abs() {
                                let h = h.unwrap();
                                if !q.reject(h.normal) {
                                    out = Some(h);
                                    best = t;
                                    if target < 0.0 {
                                        return (t, out);
                                    }
                                }
                            }
                        }
                    }
                }
                y += 1;
            }
            x += 1;
        }
        (if best != 1e20 { best } else { -1.0 }, out)
    }

    /// `World_RayCastInstance` 0x25bf48: an object with collision mode 1 (its `.ssf` meshes, one per part with a
    /// mesh), 2 (each part's mesh box) or 3 (sphere trees); else −1. Each part's world matrix (its matrix or the
    /// identity, or with an animated controller (+0x14 bit 0x100) the animation's frame for animated parts,
    /// through its parent's), times the object's, is inverted as a rotation (3 × 3 transposed, position
    /// −position turned) and the shape put into it; hits are put back through the part's matrix. The nearest
    /// to the target; the first with a target below 0. The shape is back in the world after.
    pub fn ray_cast_instance(&self, q: &mut dyn Shape, o: usize, env: &mut dyn WorldEnv) -> (f32, Option<CastHit>) {
        let obj = &self.objects[o];
        if !(obj.mode == 3 || obj.mode == 1 || obj.mode == 2) {
            return (-1.0, None);
        }
        let target = q.target();
        let mut best = 1e20f32;
        let mut out = None;
        let parts = &self.models[obj.model];
        let animate = obj.controller.is_some_and(|c| c >> 8 & 1 != 0);
        let mut world = vec![[[0.0f32; 4]; 4]; parts.len()];
        let (mut n_anim, mut n_mesh) = (0usize, 0usize);
        for (k, p) in parts.iter().enumerate() {
            let local = if !p.animated || !animate {
                p.matrix.unwrap_or(IDENTITY)
            } else {
                n_anim += 1;
                env.frame(o, n_anim - 1)
            };
            world[k] = if p.parent == -1 { local } else { through(world.get(p.parent as usize).copied().unwrap_or_default(), local) };
            let m = through(obj.rows, world[k]);
            let mut inv = m;
            for (i, j) in [(0, 1), (0, 2), (1, 2)] {
                inv[i][j] = m[j][i];
                inv[j][i] = m[i][j];
            }
            let back = [-m[3][0], -m[3][1], -m[3][2], 0.0];
            let r3 = apply(inv, back);
            inv[3] = [r3[0], r3[1], r3[2], 1.0];
            if k != 0 {
                q.pop();
            }
            q.push(inv);
            let Some(mesh) = p.mesh else { continue };
            let mut tree = None;
            let hit = match obj.mode {
                2 => q.boxed(mesh.lo, mesh.hi),
                3 => {
                    tree = Some((obj.group as usize, n_mesh));
                    n_mesh += 1;
                    self.trees.get(obj.group as usize).and_then(|g| g.get(n_mesh - 1)).and_then(|t| q.sphere_tree(t))
                }
                _ => {
                    let cm = self.collision.get(obj.group as usize).and_then(|g| g.get(n_mesh));
                    n_mesh += 1;
                    cm.and_then(|cm| q.mesh(cm, mesh.flags & 1 != 0))
                }
            };
            if let Some(h) = hit {
                if (h.t - target).abs() < (best - target).abs() {
                    let (kind, surface) = if obj.surface == -1 { (1, -1) } else { (if p.animated { 2 } else { 0 }, obj.surface) };
                    out = Some(CastHit { kind, pos: apply(m, h.pos), normal: apply(m, h.normal), tree, surface, object: Some(o) });
                    best = h.t;
                    if target < 0.0 {
                        q.pop();
                        return (h.t, out);
                    }
                }
            }
        }
        q.pop();
        (if best != 1e20 { best } else { -1.0 }, out)
    }

    /// `World_QueryInstances` 0x25b878: as [`Self::ray_cast`]'s walk, over INodes not blank, for touch objects
    /// (flag 0x20, surface −1) whose box meets the query's (an animated one's box made a cube about its position
    /// from its box's least and greatest offsets); the first that [`Self::ray_cast_instance`] hits. −1 for none.
    pub fn query_instances(&self, q: &mut dyn Shape, show_off: bool, env: &mut dyn WorldEnv) -> (f32, Option<CastHit>) {
        let (x_lo, x_hi, y_lo, y_hi) = self.around(q.mid());
        let mut x = x_lo;
        while x <= x_hi {
            let mut y = y_lo;
            while y <= y_hi {
                let (_, cell) = self.cell_at(x, y);
                if let Some(cell) = cell.filter(|c| overlaps(c.min, c.max, q)) {
                    for k in 0..self.nodes_per_cell.max(0) as usize {
                        let Some(node) = cell.nodes.get(k) else { break };
                        if node.blank || !overlaps(node.min, node.max, q) {
                            continue;
                        }
                        let n_obj = if show_off { node.n_objects_showoff } else { node.n_objects };
                        for &o in node.objects.iter().take(n_obj.max(0) as usize) {
                            let obj = &self.objects[o];
                            if obj.flags & 0x20 == 0 || obj.surface != -1 {
                                continue;
                            }
                            let (min, max) = if obj.controller.is_some_and(|c| c >> 8 & 1 != 0) {
                                let pos = obj.rows[3];
                                let a = [obj.min[0] - pos[0], obj.min[1] - pos[1], obj.min[2] - pos[2]];
                                let b = [obj.max[0] - pos[0], obj.max[1] - pos[1], obj.max[2] - pos[2]];
                                let m1 = if a[0] <= a[1] { a[0] } else { a[1] };
                                let lo = if a[2] < m1 { a[2] } else { m1 };
                                let h1 = if b[1] <= b[0] { b[0] } else { b[1] };
                                let hi = if h1 < b[2] { b[2] } else { h1 };
                                ([lo + pos[0], lo + pos[1], lo + pos[2]], [hi + pos[0], hi + pos[1], hi + pos[2]])
                            } else {
                                (obj.min, obj.max)
                            };
                            if !overlaps(min, max, q) {
                                continue;
                            }
                            let (t, h) = self.ray_cast_instance(q, o, env);
                            if 0.0 <= t {
                                return (t, h);
                            }
                        }
                    }
                }
                y += 1;
            }
            x += 1;
        }
        (-1.0, None)
    }
}

fn word(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}
fn f4_at(b: &[u8], at: usize) -> V4 {
    [0, 1, 2, 3].map(|k| f32::from_bits(word(b, at + 4 * k)))
}

impl World {
    /// The course object (0x346488) as the game's loaders leave it, from the course's files: `ResolveDataHeader`
    /// 0x255630 (the `.pbd`: patches, objects, models), `ResolveTGroupHeader` 0x252470 (the `.ltg` grid: an INode's
    /// lists are runs of its TGroup's, at offsets from the TGroup), the `.ssf` loader 0x25f598 (each object's
    /// physics record, the collision meshes) and `Course_LoadSphereTrees`; each object's flags (+0xe8) reset as a
    /// course start leaves them (the defaults in the upper half copied down, `controllers::restore_flags`). A
    /// part's matrix of −1 is the identity (0x346440). Checked against what the game builds in the function runner
    /// (`tests/world.rs`).
    /// STANDIN: an object's box (+0xcc..+0xe0, zero in the file and filled at run time by code not yet found, row
    /// E7a6) is its parts' mesh boxes put through the parts' and the object's matrices; and no object has an
    /// animated controller (+0xe4), for the course's controllers (row E7b2).
    pub fn from_course(pbd_bytes: &[u8], pbd: &tricky_data::pbd::Pbd, ltg_bytes: &[u8], ltg: &tricky_data::ltg::Ltg, ssf: &tricky_data::ssf::Ssf) -> World {
        use tricky_data::pbd::{InstanceRec, PatchRec};
        let patches = pbd
            .patches
            .iter()
            .map(|r| {
                let p = PatchRec::parse(r).unwrap_or_else(|| PatchRec::parse(&[0; 0x1c0]).unwrap());
                Patch { show_off: p.showoff_only(), surface: p.surface as i32, min: [p.bbox[0], p.bbox[1], p.bbox[2]], max: [p.bbox[3], p.bbox[4], p.bbox[5]] }
            })
            .collect();
        let models: Vec<Vec<Part>> = pbd
            .models
            .iter()
            .map(|m| {
                m.nodes
                    .iter()
                    .map(|n| {
                        let at = n.offset as usize;
                        let mesh = n.words[1];
                        Part {
                            parent: n.words[0] as i32,
                            animated: n.words[4] != 0,
                            matrix: match n.words[5] {
                                0 => None,
                                u32::MAX => Some(IDENTITY),
                                o => Some([0, 1, 2, 3].map(|k| f4_at(pbd_bytes, at + o as usize + 16 * k))),
                            },
                            mesh: (mesh != 0).then(|| {
                                let ma = at + mesh as usize;
                                PartMesh { lo: f4_at(pbd_bytes, ma + 4), hi: f4_at(pbd_bytes, ma + 0x10), flags: word(pbd_bytes, ma + 0x1c) }
                            }),
                        }
                    })
                    .collect()
            })
            .collect();
        let objects = pbd
            .instances
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let r = InstanceRec::from_words(w);
                let phys = ssf.object_physics.get(i).and_then(|&k| ssf.physics.get(k as usize)).copied().unwrap_or(tricky_data::ssf::Physics([0; 6]));
                let model = r.model.max(0) as usize;
                let (min, max) = object_box(r.matrix, models.get(model).map_or(&[][..], |v| &v[..]), &[]);
                Object {
                    rows: r.matrix,
                    min,
                    max,
                    model,
                    flags: crate::controllers::restore_flags(phys.flags(), 0xffff_0000),
                    surface: phys.surface(),
                    mode: phys.collision_mode(),
                    group: phys.group(),
                    controller: None,
                }
            })
            .collect();
        let per = ltg.inode_num.max(0) as usize;
        let cells = ltg
            .tgroups
            .iter()
            .map(|t| {
                let (off, tg) = t.as_ref()?;
                let list = |o: u32, n: i16| -> Vec<usize> { (0..n.max(0) as usize).map(|j| word(ltg_bytes, (off + o) as usize + 4 * j) as usize).collect() };
                let nodes = tg
                    .inodes
                    .iter()
                    .take(per)
                    .map(|n| {
                        let c = n.counts;
                        let n_obj = c[1].max(c[2]);
                        Node {
                            min: n.bbox.min,
                            max: n.bbox.max,
                            patches: list(n.offsets[0], c[0]),
                            objects: list(n.offsets[1], n_obj),
                            n_objects: c[1],
                            n_objects_showoff: c[2],
                            blank: c[0] == 0 && c[1] == 0 && c[2] == 0 && c[3] == 0 && c[4] == 0 && n.offsets[5] == 0,
                        }
                    })
                    .collect();
                Some(Cell { min: tg.bbox.min, max: tg.bbox.max, nodes })
            })
            .collect();
        let collision = ssf.collision.iter().map(|g| g.iter().map(|m| CollMesh { tris: m.tris.clone(), verts: m.verts.clone(), planes: m.planes.clone() }).collect()).collect();
        let trees = ssf.sphere_trees.iter().map(|g| g.iter().map(SphereTree::new).collect()).collect();
        World {
            trees,
            origin: [ltg.bbox.min[0], ltg.bbox.min[1]],
            size: ltg.tgroup_size,
            width: ltg.width,
            height: ltg.height,
            nodes_per_cell: ltg.inode_num,
            cells,
            patches,
            objects,
            models,
            collision,
        }
    }
}

/// STANDIN: an object's box from its parts' mesh boxes through the parts' (parent first) and the object's
/// matrices, for the game's run-time box (row E7a6); the object's position when it has no mesh.
/// `frames`: the animated parts' frames (as [`WorldEnv::frame`]), none for the rest pose.
pub fn object_box(rows: [V4; 4], parts: &[Part], frames: &[[V4; 4]]) -> ([f32; 3], [f32; 3]) {
    let mut world: Vec<[V4; 4]> = Vec::with_capacity(parts.len());
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    let mut n = 0;
    for (k, p) in parts.iter().enumerate() {
        let own = if p.animated && !frames.is_empty() {
            n += 1;
            frames.get(n - 1).copied().unwrap_or(IDENTITY)
        } else {
            p.matrix.unwrap_or(IDENTITY)
        };
        let parent = usize::try_from(p.parent).ok().filter(|&i| i < k).map_or(rows, |i| world[i]);
        let m = through(parent, own);
        world.push(m);
        let Some(mesh) = p.mesh else { continue };
        let (a, b) = ([mesh.lo[0], mesh.lo[1], mesh.lo[2]], [mesh.hi[0], mesh.hi[1], mesh.hi[2]]);
        for c in 0..8 {
            let v = [if c & 1 == 0 { a[0] } else { b[0] }, if c & 2 == 0 { a[1] } else { b[1] }, if c & 4 == 0 { a[2] } else { b[2] }, 1.0];
            let w = apply(m, v);
            for i in 0..3 {
                lo[i] = lo[i].min(w[i]);
                hi[i] = hi[i].max(w[i]);
            }
        }
    }
    if lo[0] > hi[0] {
        let p = [rows[3][0], rows[3][1], rows[3][2]];
        return (p, p);
    }
    (lo, hi)
}

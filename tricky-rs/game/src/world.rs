//! The world's casts (board row E7a2): `World_RayCast` 0x25aff8 (the hit nearest the query's target among the
//! patches and solid objects of the grid cells around it), `World_RayCastInstance` 0x25bf48 (one object, part by
//! part in each part's frame) and `World_QueryInstances` 0x25b878 (the first touch object hit), on the course's
//! `.ltg` grid, `.pbd` records and `.ssf` physics and collision meshes. Checked against the game in the function
//! runner on the disc's courses (`tests/world.rs`). The query is a [`Shape`] (`cWorldLine` here, row E7a); a patch's
//! own test goes through the patch cache (row E7c) and is the caller's ([`WorldEnv::patch`]).

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
    /// +0x38: sphere tree `n` of group `group`
    fn sphere_tree(&self, group: usize, n: usize) -> Option<Hit>;
    /// `World_RayCastMeshTris` with this shape's +0x28 / +0x30
    fn mesh(&self, m: &CollMesh, either_side: bool) -> Option<Hit>;
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
    fn sphere_tree(&self, _: usize, _: usize) -> Option<Hit> {
        None
    }
    fn mesh(&self, m: &CollMesh, either_side: bool) -> Option<Hit> {
        WorldLine::mesh(self, &m.tris, &m.verts, &m.planes, either_side)
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

/// Where the last patch hit was (the caller's +0x10 / +0x12 / +0x14 halves: cell, INode, patch in the INode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LastPatch {
    pub cell: i16,
    pub node: i16,
    pub patch: i16,
}

/// What the casts need from outside.
pub trait WorldEnv {
    /// The shape's +0x48 for patch `i` (through the patch cache, `World_LookupCollCache`, row E7c).
    fn patch(&mut self, q: &dyn Shape, i: usize) -> Option<Hit>;
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
    pub fn ray_cast(&self, q: &mut dyn Shape, show_off: bool, mut last: Option<&mut LastPatch>, env: &mut dyn WorldEnv) -> (f32, Option<CastHit>) {
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
                            let Some(h) = env.patch(q, p) else { continue };
                            if let Some(l) = last.as_deref_mut() {
                                let (cx, cy) = (if x < 0 { 0 } else { x.min(self.width - 1) }, if y < 0 { 0 } else { y.min(self.height - 1) });
                                *l = LastPatch { cell: (cy as i16).wrapping_mul(self.width as i16).wrapping_add(cx as i16), node: k as i16, patch: pi as i16 };
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
                    q.sphere_tree(obj.group as usize, n_mesh - 1)
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

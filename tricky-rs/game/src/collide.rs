//! Triangle-soup collision in Bevy space (metres, Y up): a uniform XZ grid of triangles with
//! "what is the ground under this point" and "push this sphere out of walls" queries.

use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;

/// Faces at least this upright (normal.y) can be ridden; anything steeper is a wall. The same for
/// terrain and for objects with a surface type of their own (`World_RayCast` treats both alike).
pub const GROUND_MIN_Y: f32 = 0.42;
const CELL: f32 = 6.0;

pub struct Tri {
    pub p: [Vec3; 3],
    /// per-vertex normals (smooth for terrain, the face normal for object meshes)
    pub n: [Vec3; 3],
    pub face: Vec3,
    /// part of a show-off ramp (only solid in show-off events)
    pub trick_only: bool,
    /// can be ridden on (otherwise it is a wall)
    pub rideable: bool,
    /// row of the game's surface table (1 groomed snow, 3/4 powder, 5 ice, ...)
    pub surface: u8,
    /// the level instance it belongs to when the level scripts can switch it off (else u32::MAX)
    pub owner: u32,
    /// the level instance it came from (u32::MAX for the terrain)
    pub inst: u32,
    /// an object with no surface type (-1) that bounces riders (`Boarder_InstanceBounce`): never
    /// ground, every face pushes the rider out
    pub bounce: bool,
    /// an object with no surface type and no bounce: not solid, only touched (cracking glass)
    pub ghost: bool,
}

/// How a placed object's triangles take part. The instance's SurfaceType decides
/// (`World_RayCast` 0x25aff8 / `World_QueryInstances` 0x25b878): a surface type (>= 0) puts the
/// object in the ground ray like terrain with that surface; -1 leaves it to the bounce test,
/// which pushes riders off only when PlayerBounce is set (and the mass U0 is not 0).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjKind { Ground, Bounce, Ghost }

fn make_tri(p: [Vec3; 3], normals: Option<[Vec3; 3]>, terrain: bool, surface: u8, trick_only: bool, kind: ObjKind, owner: u32, inst: u32) -> Option<Tri> {
    let mut face = (p[1] - p[0]).cross(p[2] - p[0]);
    if face.length_squared() < 1e-10 { return None; }
    face = face.normalize();
    if terrain && face.y < 0.0 { face = -face; }
    // Smooth normals come from the Bezier patch; where a patch is pinched to a point they are
    // garbage, so fall back to the flat normal whenever they disagree with the triangle.
    let n = normals.map(|n| n.map(|v| if v.dot(face) > 0.75 { v } else { face })).unwrap_or([face; 3]);
    let rideable = kind == ObjKind::Ground && face.y >= GROUND_MIN_Y;
    Some(Tri { p, n, face, rideable, surface, trick_only, owner, inst, bounce: kind == ObjKind::Bounce, ghost: kind == ObjKind::Ghost })
}

/// A model part of a keyframed object: its parent, its rest matrix and its curves (game space).
pub struct MoverObj { pub parent: i32, pub rest: Mat4, pub anim: Option<crate::anim::ObjAnim> }
/// A keyframed object's own clock (`cAnimObjectNode`): mode 0 once, 2 back and forth, else loop;
/// seconds. A clip a touch script plays waits (not running) until it is played.
#[derive(Clone, Copy, Debug)]
pub struct Clip { pub mode: i32, pub start: f32, pub len: f32, pub rate: f32, pub dir: f32, pub running: bool }
/// A keyframed object the riders collide with (`World_RayCastInstance` poses each part's collision
/// with `cAnimObjectNode_GetFrameMatrix` every query). Its triangles are kept in the parts' own
/// frames and re-posed whenever its clock moves.
pub struct Mover {
    pub inst: u32,
    pub surface: u8,
    pub kind: ObjKind,
    /// the instance's matrix (game space)
    pub inst_m: Mat4,
    pub objs: Vec<MoverObj>,
    /// per model part: its triangles in the part's frame (game cm)
    pub parts: Vec<(usize, Vec<[Vec3; 3]>)>,
    /// its own clock, when nothing drives it from outside (None: it stays at `t`)
    pub clip: Option<Clip>,
    pub t: f32,
    /// the game's keyframed objects set `t` (so the collision follows what is drawn)
    pub external: bool,
    /// a reset zone (puts the rider back) rather than something solid
    pub reset: bool,
    posed: Option<f32>,
    tris: Vec<Tri>,
    lo: Vec3,
    hi: Vec3,
}
impl Mover {
    pub fn new(inst: u32, surface: u8, kind: ObjKind, inst_m: Mat4, objs: Vec<MoverObj>, parts: Vec<(usize, Vec<[Vec3; 3]>)>, clip: Option<Clip>, t: f32) -> Self {
        Self { inst, surface, kind, inst_m, objs, parts, clip, t, external: false, reset: false, posed: None, tris: Vec::new(), lo: Vec3::MAX, hi: Vec3::MIN }
    }
    pub fn tri_count(&self) -> usize { self.parts.iter().map(|p| p.1.len()).sum() }
    /// Where its triangles are now (Bevy space).
    pub fn bounds(&self) -> (Vec3, Vec3) { (self.lo, self.hi) }
    /// The parts' matrices at the clock's time (game space, before the instance's).
    pub fn part_matrices(&self) -> Vec<Mat4> {
        let mut w: Vec<Mat4> = Vec::with_capacity(self.objs.len());
        for (k, o) in self.objs.iter().enumerate() {
            let local = o.anim.as_ref().map_or(o.rest, |a| a.local(self.t));
            let m = if o.parent >= 0 && (o.parent as usize) < k { w[o.parent as usize] * local } else { local };
            w.push(m);
        }
        w
    }
    /// A script plays the clip (from the start; a clip already running carries on).
    pub fn play(&mut self) {
        let Some(c) = self.clip.as_mut() else { return };
        if c.running { return; }
        c.running = true;
        c.dir = 1.0;
        self.t = c.start;
    }
    fn tick(&mut self, dt: f32) {
        let Some(c) = self.clip.as_mut() else { return };
        if self.external || !c.running { return; }
        self.t += c.rate * dt * c.dir;
        match c.mode {
            0 => { if self.t >= c.len || self.t < c.start { c.running = false; } self.t = self.t.clamp(c.start, c.len.max(c.start)); }
            2 => {
                if self.t > c.len { self.t = 2.0 * c.len - self.t; c.dir = -c.dir; }
                if self.t < c.start { self.t = 2.0 * c.start - self.t; c.dir = -c.dir; }
            }
            _ => { let span = (c.len - c.start).max(1e-3); if self.t > c.len || self.t < c.start { self.t = c.start + (self.t - c.start).rem_euclid(span); } }
        }
    }
    fn pose(&mut self) {
        if self.posed == Some(self.t) { return; }
        self.posed = Some(self.t);
        // game (cm, Z up) -> Bevy (m, Y up)
        let a = Mat4::from_cols(Vec4::new(0.01, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, -0.01, 0.0), Vec4::new(0.0, 0.01, 0.0, 0.0), Vec4::W);
        let w = self.part_matrices();
        self.tris.clear();
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for (k, ts) in &self.parts {
            let m = a * self.inst_m * w[*k];
            for t in ts {
                let q = t.map(|v| m.transform_point3(v));
                if let Some(tri) = make_tri(q, None, false, self.surface, false, self.kind, u32::MAX, self.inst) {
                    for v in q { lo = lo.min(v); hi = hi.max(v); }
                    self.tris.push(tri);
                }
            }
        }
        (self.lo, self.hi) = (lo, hi);
    }
}

/// A box the level scripts act on when a rider is in it: a boost or a teleport.
#[derive(Clone, Copy, Debug)]
pub enum PadKind { Boost { dir: Vec3, target: f32, gain: f32 }, Teleport { to: Vec3, yaw: f32 }, Pickup { mult: Option<f32>, speed: Option<f32>, spin: Option<f32> } }
#[derive(Clone, Copy, Debug)]
pub struct Pad { pub lo: Vec3, pub hi: Vec3, pub kind: PadKind }

#[derive(Clone, Copy, Debug)]
pub struct GroundHit { pub y: f32, pub normal: Vec3, pub surface: u8 }

#[derive(Default)]
pub struct CollisionWorld {
    tris: Vec<Tri>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    pub min_y: f32,
    /// show-off event: the show-off ramps are solid
    pub showoff: bool,
    /// instances switched off by the event's scripts (their triangles are left out of queries)
    pub off: std::collections::HashSet<u32>,
    /// the instance the next added triangles belong to
    pub owner: u32,
    /// the instance the next added triangles come from (for its collision sound)
    pub inst: u32,
    pub pads: Vec<Pad>,
    /// objects whose touch sets off particle emitters, and their boxes
    pub emit_boxes: std::collections::HashMap<usize, (Vec3, Vec3)>,
    /// per event (race, show-off, free ride): the instances its scripts switch off
    pub mode_off: [std::collections::HashSet<u32>; 3],
    /// the course's out-of-bounds surfaces: touching one puts the rider back on the course
    resets: Vec<([Vec3; 3], u32)>,
    reset_grid: HashMap<(i32, i32), Vec<u32>>,
    /// instances gone for the rest of the run (broken glass): back on a restart
    pub dead: std::collections::HashSet<u32>,
    /// doors a script opens with a keyframed clip (megaplex's irises): their (reset) triangles
    /// are out of the way while the clip holds them open
    pub gates: Vec<Gate>,
    /// the trigger boxes that open them (box, gate instance), for a run without the scripts (the sim)
    pub gate_triggers: Vec<(Vec3, Vec3, u32)>,
    /// keyframed objects: their collision moves with them
    pub movers: Vec<Mover>,
    /// the trigger boxes that play a keyframed object's clip (box, instance), for the sim
    pub mover_triggers: Vec<(Vec3, Vec3, u32)>,
}

/// A door opened by a keyframed clip (`cAnimObjectNode`, sub-type 256 mode 0, played once): the
/// instance, the clip's length (s), the stretch of it the door stands open, and how far it has run.
#[derive(Clone, Debug)]
pub struct Gate { pub inst: u32, pub len: f32, pub open: (f32, f32), pub t: Option<f32> }
impl Gate {
    pub fn is_open(&self) -> bool { self.t.is_some_and(|t| t >= self.open.0 && t < self.open.1) }
}

fn cell(v: f32) -> i32 { (v / CELL).floor() as i32 }

impl CollisionWorld {
    pub fn len(&self) -> usize { self.tris.len() }

    /// Left out of queries: show-off ramps outside show-off, objects the event switched off,
    /// broken glass, open doors.
    #[inline]
    fn skip(&self, t: &Tri) -> bool {
        t.ghost || self.hidden(t)
    }
    #[inline]
    fn hidden(&self, t: &Tri) -> bool {
        (t.trick_only && !self.showoff) || (t.owner != u32::MAX && self.gone(t.owner))
    }
    /// The keyframed objects' triangles (as they are posed now) whose box meets [lo, hi].
    fn movers_in(&self, lo: Vec3, hi: Vec3) -> impl Iterator<Item = &Tri> + '_ {
        self.movers.iter().filter(move |m| !m.reset && m.lo.cmple(hi).all() && m.hi.cmpge(lo).all()).flat_map(|m| m.tris.iter())
    }
    /// The game's keyframed object `inst` is at `t` (s) on its clip.
    pub fn set_mover_time(&mut self, inst: u32, t: f32) {
        for m in self.movers.iter_mut().filter(|m| m.inst == inst) { m.external = true; m.t = t; m.pose(); }
    }
    /// Run the keyframed objects' own clocks and pose their collision.
    pub fn tick_movers(&mut self, dt: f32) {
        for m in self.movers.iter_mut() { m.tick(dt); m.pose(); }
    }
    #[inline]
    fn gone(&self, owner: u32) -> bool {
        self.off.contains(&owner) || self.dead.contains(&owner) || self.gates.iter().any(|g| g.inst == owner && g.is_open())
    }
    /// A script plays the door's clip: from the start, or (already open) it stays open a while longer.
    pub fn open_gate(&mut self, inst: u32) {
        let Some(g) = self.gates.iter_mut().find(|g| g.inst == inst) else { return };
        g.t = match g.t { Some(t) if t < g.open.1 => Some(t.min(g.open.0)), _ => Some(0.0) };
    }
    pub fn gate_time(&self, inst: u32) -> Option<f32> { self.gates.iter().find(|g| g.inst == inst).and_then(|g| g.t) }
    /// Run the doors' clips; a clip that has run out leaves the door shut. (The other keyframed
    /// objects' clocks run here too.)
    pub fn tick_gates(&mut self, dt: f32) {
        for g in self.gates.iter_mut() { if let Some(t) = g.t.as_mut() { *t += dt; if *t >= g.len { g.t = None; } } }
        self.tick_movers(dt);
    }
    /// Without the scripts (the sim): a body in a trigger box opens its door.
    pub fn touch_gates(&mut self, bodies: &[Vec3]) {
        let inside = |lo: &Vec3, hi: &Vec3| bodies.iter().any(|b| b.cmpge(*lo - 0.3).all() && b.cmple(*hi + 0.3).all());
        let hit: Vec<u32> = self.gate_triggers.iter().filter(|(lo, hi, _)| inside(lo, hi)).map(|t| t.2).collect();
        for g in hit { self.open_gate(g); }
        // (and the other clips a touch plays: flippers, ramps, bumpers)
        let hit: Vec<u32> = self.mover_triggers.iter().filter(|(lo, hi, _)| inside(lo, hi)).map(|t| t.2).collect();
        for i in hit { for m in self.movers.iter_mut().filter(|m| m.inst == i && !m.external) { m.play(); } }
    }
    /// A restart: broken things back, doors shut.
    pub fn reset_run(&mut self) {
        self.dead.clear();
        for g in self.gates.iter_mut() { g.t = None; }
        for m in self.movers.iter_mut() { if let Some(c) = m.clip.as_mut().filter(|c| c.mode == 0) { c.running = false; m.t = c.start; } }
    }
    /// Is a rider at `pos` (feet) in contact with one of `owner`'s triangles: standing on it (or
    /// on something just above it, within `below` m of the feet) or touching it with the body
    /// sphere? Returns the triangle's face normal.
    pub fn contact(&self, owner: u32, pos: Vec3, radius: f32, below: f32) -> Option<Vec3> {
        if self.gone(owner) { return None; }
        let body = pos + Vec3::Y * 0.8;
        let r = radius.max(0.8);
        for cx in cell(pos.x - r)..=cell(pos.x + r) {
            for cz in cell(pos.z - r)..=cell(pos.z + r) {
                let Some(ids) = self.grid.get(&(cx, cz)) else { continue };
                for &id in ids {
                    let t = &self.tris[id as usize];
                    if t.owner != owner || self.hidden(t) { continue; }
                    // under the feet
                    let (a, b, c) = (t.p[0], t.p[1], t.p[2]);
                    let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
                    if det.abs() > 1e-9 {
                        let w0 = ((b.z - c.z) * (pos.x - c.x) + (c.x - b.x) * (pos.z - c.z)) / det;
                        let w1 = ((c.z - a.z) * (pos.x - c.x) + (a.x - c.x) * (pos.z - c.z)) / det;
                        let w2 = 1.0 - w0 - w1;
                        if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                            let y = a.y * w0 + b.y * w1 + c.y * w2;
                            if y <= pos.y + 0.1 && y >= pos.y - below { return Some(t.face); }
                        }
                    }
                    if (body - closest_point(body, a, b, c)).length_squared() < radius * radius { return Some(t.face); }
                }
            }
        }
        None
    }

    /// `normals`: smooth vertex normals, or None to use the face normal. `terrain` faces are
    /// flipped to point up if needed (patch winding is not consistent).
    /// A placed object's triangle (see `ObjKind`).
    pub fn add_obj(&mut self, p: [Vec3; 3], surface: u8, kind: ObjKind) { self.push(make_tri(p, None, false, surface, false, kind, self.owner, self.inst)) }
    pub fn add_ex(&mut self, p: [Vec3; 3], normals: Option<[Vec3; 3]>, terrain: bool, surface: u8, trick_only: bool) {
        self.push(make_tri(p, normals, terrain, surface, trick_only, ObjKind::Ground, self.owner, self.inst))
    }
    fn push(&mut self, t: Option<Tri>) {
        let Some(t) = t else { return };
        let p = t.p;
        let id = self.tris.len() as u32;
        let (min, max) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
        self.min_y = self.min_y.min(min.y);
        for cx in cell(min.x)..=cell(max.x) {
            for cz in cell(min.z)..=cell(max.z) {
                self.grid.entry((cx, cz)).or_default().push(id);
            }
        }
        self.tris.push(t);
    }

    /// Highest rideable surface under/around `pos` whose height lies in [pos.y - down, pos.y + up].
    pub fn ground(&self, pos: Vec3, up: f32, down: f32) -> Option<GroundHit> {
        let ids = self.grid.get(&(cell(pos.x), cell(pos.z))).map_or(&[][..], |v| &v[..]);
        let mut best: Option<GroundHit> = None;
        let near = self.movers_in(pos - Vec3::Y * down, pos + Vec3::Y * up);
        for t in ids.iter().map(|&id| &self.tris[id as usize]).chain(near) {
            if self.skip(t) { continue; }
            if !t.rideable { continue; }
            // barycentric coordinates in the XZ plane
            let (a, b, c) = (t.p[0], t.p[1], t.p[2]);
            let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
            if det.abs() < 1e-9 { continue; }
            let w0 = ((b.z - c.z) * (pos.x - c.x) + (c.x - b.x) * (pos.z - c.z)) / det;
            let w1 = ((c.z - a.z) * (pos.x - c.x) + (a.x - c.x) * (pos.z - c.z)) / det;
            let w2 = 1.0 - w0 - w1;
            const E: f32 = -1e-4;
            if w0 < E || w1 < E || w2 < E { continue; }
            let y = a.y * w0 + b.y * w1 + c.y * w2;
            if y > pos.y + up || y < pos.y - down { continue; }
            if best.is_none_or(|h| y > h.y) {
                let mut n = (t.n[0] * w0 + t.n[1] * w1 + t.n[2] * w2).normalize_or(t.face);
                if n.y < 0.0 { n = -n; }
                best = Some(GroundHit { y, normal: n, surface: t.surface });
            }
        }
        best
    }

    /// First thing a straight line from `from` to `to` runs into (terrain or object, any side).
    /// Returns the distance along the line, if anything is in the way.
    pub fn raycast(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let d = to - from;
        let len = d.length();
        if len < 1e-4 { return None; }
        let dir = d / len;
        let steps = (len / (CELL * 0.5)).ceil() as i32;
        let mut seen: Vec<(i32, i32)> = Vec::new();
        let mut best: Option<f32> = None;
        // Moller-Trumbore
        let mut test = |t: &Tri| {
            if self.skip(t) { return; }
            let (e1, e2) = (t.p[1] - t.p[0], t.p[2] - t.p[0]);
            let h = dir.cross(e2);
            let a = e1.dot(h);
            if a.abs() < 1e-7 { return; }
            let f = 1.0 / a;
            let sv = from - t.p[0];
            let u = f * sv.dot(h);
            if !(0.0..=1.0).contains(&u) { return; }
            let q = sv.cross(e1);
            let v = f * dir.dot(q);
            if v < 0.0 || u + v > 1.0 { return; }
            let dist = f * e2.dot(q);
            if dist > 0.02 && dist < len && best.is_none_or(|b| dist < b) { best = Some(dist); }
        };
        for i in 0..=steps {
            let p = from + d * (i as f32 / steps as f32);
            for (ox, oz) in [(0.0, 0.0), (0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6)] {
                let key = (cell(p.x + ox), cell(p.z + oz));
                if seen.contains(&key) { continue; }
                seen.push(key);
                let Some(ids) = self.grid.get(&key) else { continue };
                for &id in ids { test(&self.tris[id as usize]); }
            }
        }
        for t in self.movers_in(from.min(to), from.max(to)) { test(t); }
        best
    }

    /// Debug: describe every wall triangle within `radius` of a point.
    pub fn explain(&self, center: Vec3, radius: f32) {
        let r = Vec3::splat(radius);
        for t in self.near(center - r, center + r) {
            if self.skip(t) || t.rideable || t.face.y < -0.6 { continue; }
            let q = closest_point(center, t.p[0], t.p[1], t.p[2]);
            if (center - q).length() < radius { println!("  wall tri (instance {}): {:.1?} face {:.2?} surface {}{}", t.inst as i32, t.p, t.face, t.surface, if t.bounce { " bounce" } else { "" }); }
        }
    }
    /// Every triangle in the grid cells over [lo, hi] (a triangle can come more than once), then
    /// the keyframed objects' there.
    fn near(&self, lo: Vec3, hi: Vec3) -> impl Iterator<Item = &Tri> + '_ {
        let cells = (cell(lo.x)..=cell(hi.x)).flat_map(move |cx| (cell(lo.z)..=cell(hi.z)).map(move |cz| (cx, cz)));
        cells.filter_map(|k| self.grid.get(&k)).flat_map(|ids| ids.iter().map(|&id| &self.tris[id as usize])).chain(self.movers_in(lo, hi))
    }
    /// The instance of a bounce object the sphere touches (Sfx_InstanceCollision's target, from
    /// `World_QueryInstances`), if any.
    pub fn wall_instance(&self, center: Vec3, radius: f32) -> Option<u32> {
        let r = Vec3::splat(radius);
        for t in self.near(center - r, center + r) {
            if t.inst == u32::MAX || !t.bounce || self.skip(t) { continue; }
            let q = closest_point(center, t.p[0], t.p[1], t.p[2]);
            if (center - q).length() < radius { return Some(t.inst); }
        }
        None
    }
    pub fn add_reset(&mut self, p: [Vec3; 3]) {
        let id = self.resets.len() as u32;
        let lo = p[0].min(p[1]).min(p[2]);
        let hi = p[0].max(p[1]).max(p[2]);
        for cx in cell(lo.x)..=cell(hi.x) { for cz in cell(lo.z)..=cell(hi.z) { self.reset_grid.entry((cx, cz)).or_default().push(id); } }
        self.resets.push((p, self.owner));
    }
    pub fn in_reset(&self, c: Vec3, radius: f32) -> bool {
        for cx in cell(c.x - radius)..=cell(c.x + radius) {
            for cz in cell(c.z - radius)..=cell(c.z + radius) {
                let Some(ids) = self.reset_grid.get(&(cx, cz)) else { continue };
                for &id in ids {
                    let (t, owner) = &self.resets[id as usize];
                    if *owner != u32::MAX && self.gone(*owner) { continue; }
                    if (c - closest_point(c, t[0], t[1], t[2])).length_squared() < radius * radius { return true; }
                }
            }
        }
        let r = Vec3::splat(radius);
        self.movers.iter().filter(|m| m.reset && m.lo.cmple(c + r).all() && m.hi.cmpge(c - r).all())
            .flat_map(|m| m.tris.iter()).any(|t| (c - closest_point(c, t.p[0], t.p[1], t.p[2])).length_squared() < radius * radius)
    }
    /// Push a sphere out of every wall it overlaps. Returns the corrected centre and the
    /// normals of the walls that were touched.
    pub fn push_out(&self, mut center: Vec3, radius: f32) -> (Vec3, Vec<Vec3>) {
        let mut contacts = Vec::new();
        let r = Vec3::splat(radius);
        for _ in 0..2 {
            let mut moved = false;
            for t in self.near(center - r, center + r) {
                if self.skip(t) || t.rideable || t.face.y < -0.6 { continue; }
                let q = closest_point(center, t.p[0], t.p[1], t.p[2]);
                let d = center - q;
                let dist2 = d.length_squared();
                if dist2 >= radius * radius || dist2 < 1e-10 { continue; }
                let dist = dist2.sqrt();
                let n = d / dist;
                center += n * (radius - dist);
                contacts.push(n);
                moved = true;
            }
            if !moved { break; }
        }
        (center, contacts)
    }
}

/// Closest point on triangle abc to p (Ericson, Real-Time Collision Detection).
fn closest_point(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 { return a; }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 { return b; }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { return a + ab * (d1 / (d1 - d3)); }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 { return c; }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { return a + ac * (d2 / (d2 - d6)); }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 { return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))); }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

//! World animations the level scripts (SSFLogic.json) set going: texture flipbooks
//! (`cTexFlipNode`, sub-type 11: signs, check-point tops, the start lights), texture scrolls
//! (`cUVScrollNode`, 10: boost pads, rivers, LCD scan lines, conveyors), waving flags
//! (`cFlagNode`, 13) and keyframed clips that run only as far as a script lets them
//! (`cAnimDeltaNode`, 257: falling trees, sewer gates), cracked glass (`cCrackedNode`, 14:
//! megaplex's glass floors), crowd boxes (`cCrowdBoxNode`, 17) and counters (`cCounterNode`, 6:
//! merqury's ten garbage cans light the strike sign). The scripts run as threads at play time
//! (`TriggerScript_Run`): from the wake-up slot when the level starts, from the collision slot
//! when a rider touches the object, from the EffectTriggerSlot when a counter or crack reaches
//! its goal, and StartCountDown when the countdown starts. Controllers tick at 60 Hz as in the game.

use crate::level::{load_obj, quat4, Level, ObjMesh};
use crate::logic::{FlagDef, FlipDef, ScrollDef, WOp, WorldScript};
use crate::props::Kind;
use crate::rider::RaceState;
use crate::{g2b, material, ui, AnimObjects, LevelEntity, LevelRes, MeshBuf, Opponents, Props, RaceRes, RiderRes, SmoothDt, TexCache, World};
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::{HashMap, HashSet};

const TICK: f32 = 1.0 / 60.0;

/// A material some animation drives: the instance and model it belongs to, the flipbook frames
/// (empty: not flipped) and the base texture, and what it shows now (frame, uv offset).
struct WaMat { handle: Handle<StandardMaterial>, inst: usize, model: usize, frames: Vec<Handle<Image>>, base: Option<Handle<Image>>, shown: (i32, Vec2) }

/// A running TexFlip (update 0x1430b8): frame, accumulator and rate (steps a tick), the base rate,
/// state (0 cycle, 1/2 blink on/off), ticks to live (0 for ever), frame count.
struct Flip { def: FlipDef, frame: i32, acc: f32, rate: f32, base: f32, state: i32, life: i32, count: i32 }
/// A running UVScroll (update 0x1422e0), one per model (`cUVScrollNode_ShareModelSlot`).
struct Scroll { def: ScrollDef, d: Vec2, uv: Vec2, tau: f32, running: bool, life: i32 }
/// A flag (`cFlagNode_BuildStrip` 0x144ec8): 8 columns × 2 rows between the quad's corners
/// (model space), the model→game matrix, phase and its step a tick, amplitude, ticks to live.
struct FlagRt { inst: usize, def: FlagDef, active: bool, mesh: Handle<Mesh>, rows: [[Vec3; 8]; 2], m: Mat4, phase: f32, rate: f32, amp: f32, amp0: f32, life: i32 }

/// Cracked glass (`cCrackedNode`, update 0x1455a0): ticks to live (< 0 for ever), ticks before
/// another impact counts (30), health, the flipbook frame it shows, whether it has gone off.
struct Crack { life: i32, cool: i32, health: f32, frame: i32, fired: bool }
/// A counter (`cCounterNode`, update 0x13b178): hits still needed, the hits seen (SetParam 1/2
/// bits), ticks to live (< 0 for ever).
struct Counter { count: i32, bits: u16, life: i32 }
/// A crowd box (`cCrowdBoxNode`): where it is (Bevy space), how many cells it steps, each cell's
/// phase (< 0 pausing) and animation sequence, the excitement (0 frozen .. 7), and the frame each
/// cell shows.
struct Crowd { pos: Vec3, n: usize, phase: [i32; 16], anim: [usize; 16], exc: i32, frame: [usize; 16] }

/// The crowd's four animation sequences over the 16 crowd frames (ELF 0x31d378).
const CROWD_SEQ: [[usize; 16]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [0, 1, 2, 3, 2, 1, 0, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [0, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1],
];
/// Chance out of 256 that a cell pauses at the end of a sequence, by excitement (ELF 0x31d478).
const CROWD_PAUSE: [u32; 8] = [256, 0, 50, 100, 200, 100, 50, 0];

#[derive(Clone, Copy)]
enum Src { Header(usize), Function(usize) }
struct Thread { src: Src, pc: usize, inst: Option<usize>, wait: f32 }

#[derive(Resource, Default)]
pub struct WorldAnim {
    mats: Vec<WaMat>, keys: HashMap<(usize, usize, usize, bool), usize>,
    scroll_models: HashSet<usize>,
    flips: HashMap<usize, Flip>, scrolls: HashMap<usize, Scroll>, flags: Vec<FlagRt>,
    /// objects with a controller that keeps their touch script from running (ticks; -1 for good)
    busy: HashMap<usize, i32>,
    threads: Vec<Thread>, boxes: Vec<(usize, Vec3, Vec3)>,
    started: bool, counting: bool, acc: f32, seed: u32,
    cracks: HashMap<usize, Crack>, counters: HashMap<usize, Counter>, crowds: HashMap<usize, Crowd>,
    /// crowd boxes' cell entities (cell, entity) and the 16 crowd frames' materials (CROWD.SSH)
    crowd_cells: HashMap<usize, Vec<(usize, Entity)>>, crowd_mats: Option<Vec<Handle<StandardMaterial>>>,
    /// cracked glass and what breaks with it (solid, touched for real), and the iris doors
    glass: Vec<usize>, gates: HashSet<usize>,
    /// doors a script just played the opening clip on
    opened: Vec<usize>,
    /// the run restarts: everything back as the level woke (set by `ride`)
    pub restart: bool,
    /// the impact (cm/s) of the touch whose script is running, and the toucher's velocity
    impact: f32, impact_vel: Vec3,
    /// controllers that reached their goal this tick (their EffectTriggerSlot runs), objects the
    /// scripts killed (DeadNode) for the props to hide, and broken glass waiting for a restart
    fire: Vec<usize>, killed: Vec<usize>, broken: HashSet<usize>,
    ticks: u32,
}

/// The first flipbook among the model's materials (its length is the frame count).
fn flipbook(level: &Level, model: usize) -> Option<&Vec<String>> {
    level.world.models.get(model)?.model_objects.iter().flat_map(|o| o.mesh_data.iter().flatten())
        .find_map(|p| level.world.materials.get(p.material_id.max(0) as usize)?.texture_flipbook.as_ref().filter(|f| f.len() > 1))
}

/// Does a script flip this object's textures (a TexFlip, or cracked glass's frames)?
fn flipped(level: &Level, i: usize) -> bool { level.logic.flips.contains(&i) || level.logic.cracks.contains_key(&i) }

impl WorldAnim {
    pub fn new(level: &Level) -> Self {
        let scroll_models = level.logic.scrolls.iter().filter_map(|i| level.instances.get(*i)).filter(|i| i.model_id >= 0).map(|i| i.model_id as usize).collect();
        Self { scroll_models, seed: 0x2545_F491, ..default() }
    }
    fn next(&mut self) -> u32 { self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5; self.seed }
    fn rand(&mut self) -> f32 { (self.next() >> 8) as f32 / 16_777_216.0 }
    pub fn summary(&self) -> String {
        format!("world animations: {} materials, {} scrolling models, {} flags, {} crowd boxes", self.mats.len(), self.scroll_models.len(), self.flags.len(), self.crowd_cells.len())
    }
    /// Is this instance drawn with its own materials (its textures flip or scroll, or it waves)?
    pub fn owns(&self, level: &Level, i: usize) -> bool {
        let Some(inst) = level.instances.get(i).filter(|x| x.model_id >= 0) else { return false };
        let m = inst.model_id as usize;
        self.scroll_models.contains(&m) || level.logic.flags.contains_key(&i) || level.logic.crowds.contains_key(&i) || (flipped(level, i) && flipbook(level, m).is_some())
    }
    /// The material for one of an owned instance's meshes: shared by the model unless its
    /// textures flip (then the instance's own).
    #[allow(clippy::too_many_arguments)]
    pub fn material(&mut self, level: &Level, inst: usize, mid: usize, variant: bool, base: StandardMaterial,
        materials: &mut Assets<StandardMaterial>, tex: &mut TexCache, images: &mut Assets<Image>) -> Handle<StandardMaterial> {
        let model = level.instances[inst].model_id.max(0) as usize;
        let names = level.world.materials.get(mid).and_then(|m| m.texture_flipbook.as_ref()).filter(|f| f.len() > 1 && flipped(level, inst));
        let key = (model, if names.is_some() { inst } else { usize::MAX }, mid, variant);
        if let Some(k) = self.keys.get(&key) { return self.mats[*k].handle.clone(); }
        let frames = names.map(|n| n.iter().filter_map(|n| tex.get(n, images).map(|t| t.handle)).collect()).unwrap_or_default();
        let base_tex = base.base_color_texture.clone();
        let handle = materials.add(base);
        self.keys.insert(key, self.mats.len());
        self.mats.push(WaMat { handle: handle.clone(), inst, model, frames, base: base_tex, shown: (-1, Vec2::ZERO) });
        handle
    }
    /// Spawn an owned static instance (baked meshes per material, Bevy space); a flag's quad
    /// becomes an 8 × 2 strip of its own.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_static(&mut self, commands: &mut Commands, level: &Level, i: usize, parts: Vec<(usize, String, MeshBuf)>,
        meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, tex: &mut TexCache) {
        let inst = &level.instances[i];
        if level.logic.crowds.contains_key(&i) && self.crowd_frames(level, images, materials) { self.spawn_crowd(commands, level, i, parts, meshes); return; }
        let mut parts = parts;
        if let Some(def) = level.logic.flags.get(&i).copied() {
            if let Some(k) = self.spawn_flag(commands, level, i, def, &parts, meshes, images, materials, tex) { parts.remove(k); }
        }
        let mut merged: HashMap<usize, MeshBuf> = HashMap::new();
        for (mid, _, buf) in parts { merged.entry(mid).or_default().append(buf); }
        let mut kids = Vec::new();
        for (mid, buf) in merged {
            let t = tex.get(&level.world.materials[mid].texture_path, images);
            let mat = self.material(level, i, mid, false, material(t.as_ref(), true), materials, tex, images);
            kids.push((meshes.add(buf.build()), mat));
        }
        commands.spawn((Name::new(inst.instance_name.clone()), LevelEntity, Transform::IDENTITY, Visibility::Inherited)).with_children(|p| {
            for (mesh, mat) in kids { p.spawn((Mesh3d(mesh), MeshMaterial3d(mat))); }
        });
    }
    /// The 16 global crowd frames (DATA/TEXTURES/CROWD.SSH, as chars/crowd/crowdNN.png), loaded
    /// once; false if they are missing (crowd boxes are then drawn as they are).
    fn crowd_frames(&mut self, level: &Level, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> bool {
        if self.crowd_mats.is_none() {
            let mut tex = TexCache::new(crate::chars_dir(&level.dir).map(|d| d.join("crowd")).unwrap_or_default());
            let mats: Vec<_> = (0..16).map_while(|k| tex.get(&format!("crowd{k:02}.png"), images)).map(|t| materials.add(material(Some(&t), true))).collect();
            if mats.len() < 16 { eprintln!("crowd frames (chars/crowd/crowdNN.png) not found"); }
            self.crowd_mats = Some(mats);
        }
        self.crowd_mats.as_ref().is_some_and(|m| m.len() == 16)
    }
    /// A crowd box: one entity per cell mesh (the model's meshes in order are cells 0..15), each
    /// showing one of the crowd frames. Until its controller starts, each cell shows a random
    /// frame (`Crowd_LoadTexturesRandomize` 0x1458e0).
    fn spawn_crowd(&mut self, commands: &mut Commands, level: &Level, i: usize, parts: Vec<(usize, String, MeshBuf)>, meshes: &mut Assets<Mesh>) {
        let inst = &level.instances[i];
        let order: Vec<&str> = level.world.models.get(inst.model_id.max(0) as usize).into_iter()
            .flat_map(|m| m.model_objects.iter()).flat_map(|o| o.mesh_data.iter().flatten()).map(|p| p.mesh_path.as_str()).collect();
        let mut kids = Vec::new();
        for (_, path, buf) in parts {
            let cell = order.iter().position(|p| *p == path).unwrap_or(0).min(15);
            let frame = (self.next() & 15) as usize;
            kids.push((cell, meshes.add(buf.build()), self.crowd_mats.as_ref().map(|m| m[frame].clone()).unwrap_or_default()));
        }
        let mut cells = Vec::new();
        commands.spawn((Name::new(inst.instance_name.clone()), LevelEntity, Transform::IDENTITY, Visibility::Inherited)).with_children(|p| {
            for (cell, mesh, mat) in kids { cells.push((cell, p.spawn((Mesh3d(mesh), MeshMaterial3d(mat))).id())); }
        });
        self.crowd_cells.insert(i, cells);
    }
    /// The flag's quad (the first 4-vertex mesh, in strip order as stored): rows v0→v1 and
    /// v2→v3, columns k/7 along them (`cFlagNode_Init` 0x144938). Returns which part it was.
    #[allow(clippy::too_many_arguments)]
    fn spawn_flag(&mut self, commands: &mut Commands, level: &Level, i: usize, def: FlagDef, parts: &[(usize, String, MeshBuf)],
        meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, tex: &mut TexCache) -> Option<usize> {
        let set = &level.world;
        let inst = &level.instances[i];
        let model = set.models.get(inst.model_id.max(0) as usize)?;
        let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
        let (obj_m, path, mid) = model.model_objects.iter().zip(set.object_matrices(model)).find_map(|(o, m)| {
            o.mesh_data.iter().flatten().find(|p| quad(&set.dir.join("Meshes").join(&p.mesh_path)).is_some()).map(|p| (m, p.mesh_path.clone(), p.material_id.max(0) as usize))
        })?;
        let (v, uv) = quad(&set.dir.join("Meshes").join(&path))?;
        let k = parts.iter().position(|p| p.1 == path)?;
        let col = parts[k].2.col.first().copied().unwrap_or([1.0; 4]);
        let lerp = |a: usize, b: usize, c: usize| (v[a] + (v[b] - v[a]) * c as f32 / 7.0, uv[a] + (uv[b] - uv[a]) * c as f32 / 7.0);
        let mut rows = [[Vec3::ZERO; 8]; 2];
        let mut uvs = Vec::new();
        for (r, (a, b)) in [(0, 1), (2, 3)].into_iter().enumerate() {
            for (c, cell) in rows[r].iter_mut().enumerate() { let (p, t) = lerp(a, b, c); *cell = p; uvs.push([t.x, t.y]); }
        }
        let idx: Vec<u32> = (0..7u32).flat_map(|c| [c, c + 1, 8 + c, c + 1, 9 + c, 8 + c]).collect();
        let m = inst_m * obj_m;
        let pos: Vec<[f32; 3]> = rows.iter().flatten().map(|p| g2b(m.transform_point3(*p)).to_array()).collect();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 16])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![col; 16]);
        mesh.insert_indices(Indices::U32(idx));
        let mesh = meshes.add(mesh);
        let t = tex.get(&set.materials.get(mid)?.texture_path, images);
        let mat = self.material(level, i, mid, false, material(t.as_ref(), true), materials, tex, images);
        commands.spawn((Name::new(format!("{} (flag)", inst.instance_name)), LevelEntity, Mesh3d(mesh.clone()), MeshMaterial3d(mat), Transform::IDENTITY));
        // amplitude in model units: the game divides it by the instance's scale (at least 1)
        let amp = def.amp / inst.scale[0].max(1.0);
        self.flags.push(FlagRt { inst: i, def, active: false, mesh, rows, m, phase: 0.0, rate: 0.0, amp, amp0: amp, life: 0 });
        Some(k)
    }

    // ------------------------------------------------------------ scripts

    /// Run a script from `pc` until its first wait (`TriggerScript_Run`); op 7 runs another
    /// object's script straight away as a thread of its own.
    fn run(&mut self, w: &WorldScript, level: &Level, anims: &mut AnimObjects, src: Src, mut pc: usize, inst: Option<usize>, depth: u32) {
        let ops = match src { Src::Header(h) => w.headers.get(h), Src::Function(f) => w.functions.get(f).map(|f| &f.1) };
        let Some(ops) = ops else { return };
        while let Some(op) = ops.get(pc) {
            pc += 1;
            match *op {
                WOp::Wait(t) => { self.threads.push(Thread { src, pc, inst, wait: t.max(TICK) }); return; }
                WOp::Run(i, h) => if depth < 12 { self.run(w, level, anims, Src::Header(h), 0, Some(i), depth + 1) },
                WOp::Call(f) => if depth < 12 { self.run(w, level, anims, Src::Function(f), 0, None, depth + 1) },
                _ => if let Some(i) = inst { self.apply(level, anims, i, *op) },
            }
        }
    }
    fn apply(&mut self, level: &Level, anims: &mut AnimObjects, i: usize, op: WOp) {
        match op {
            WOp::Debounce(t) => { self.busy.insert(i, if t > 0.0 { (t * 60.0) as i32 } else { -1 }); }
            WOp::Dead => { self.busy.insert(i, -1); self.killed.push(i); }
            // cAnimObjectNode on a door: its clip opens it (the collision follows, `CollisionWorld::gates`)
            WOp::Anim(_) if self.gates.contains(&i) => self.opened.push(i),
            // on anything else (flippers, ramps, bumpers) the clip plays from the start: once (mode
            // 0, it ends where it rests) or on and on; a clip already running carries on
            WOp::Anim(_) => for o in anims.0.iter_mut().filter(|o| o.inst == i && o.prop.is_none()) {
                if o.budget.is_some_and(|b| b > 0.0) { continue; }
                let start = o.def.start.max(0.0) / 30.0;
                o.t = start;
                o.dir = 1.0;
                o.budget = Some(if o.def.mode == 0 { (o.len - start).max(0.0) } else { f32::INFINITY });
            },
            WOp::Flip(d) => {
                // cTexFlipNode ctor 0x142e90: a start frame < 0 is random (ceil(8 r))
                let Some(count) = level.instances.get(i).and_then(|x| flipbook(level, x.model_id.max(0) as usize)).map(|f| f.len() as i32) else { return };
                let frame = if d.start < 0 { (self.rand() * 8.0).ceil() as i32 } else { d.start }.min(count - 1);
                self.flips.insert(i, Flip { def: d, frame, acc: 0.0, rate: d.speed * TICK, base: d.speed * TICK, state: d.mode, life: (d.life * 60.0) as i32, count });
            }
            WOp::Scroll(d) => if let Some(x) = level.instances.get(i).filter(|x| x.model_id >= 0) {
                self.scrolls.entry(x.model_id as usize).or_insert(Scroll { def: d, d: Vec2::new(d.du, d.dv), uv: Vec2::ZERO, tau: 0.0, running: true, life: (d.life * 60.0) as i32 });
            },
            WOp::Flag(d) => {
                let r = (self.rand(), self.rand(), self.rand());
                if let Some(f) = self.flags.iter_mut().find(|f| f.inst == i) {
                    // -1: random rate 0.5..2 waves a second, amplitude 5..50 cm; a random phase
                    f.active = true;
                    f.rate = if d.rate == -1.0 { 0.5 + 1.5 * r.0 } else { d.rate } * TICK;
                    if d.amp == -1.0 { f.amp = 5.0 + 45.0 * r.1; }
                    f.phase = r.2;
                    f.life = (d.life * 60.0) as i32;
                }
            }
            // cCrackedNode ctor 0x145458: shows frame max(1, ceil((n - 1) r)) (TexFlip_BindInstance
            // with -1: the cracked texture), counts the touch that made it as an impact
            WOp::Crack(d) => if !self.cracks.contains_key(&i) {
                let n = level.instances.get(i).and_then(|x| flipbook(level, x.model_id.max(0) as usize)).map_or(1, |f| f.len() as i32);
                let frame = if n > 1 { ((self.rand() * (n - 1) as f32).ceil() as i32).max(1) } else { 0 };
                let mut c = Crack { life: (d.life * 60.0) as i32, cool: 30, health: d.health, frame, fired: false };
                c.impact(self.impact);
                self.cracks.insert(i, c);
            },
            WOp::Counter(d) => { self.counters.entry(i).or_insert(Counter { count: d.count, bits: 0, life: (d.life * 60.0) as i32 }); }
            // cCrowdBoxNode_Init 0x145c00: each cell a random phase (0..15) and sequence (0..3)
            WOp::Crowd(d) => if !self.crowds.contains_key(&i) && self.crowd_cells.contains_key(&i) {
                let mut c = Crowd { pos: g2b(level.instances[i].location.into()), n: (d.rows * d.cols).clamp(0, 16) as usize, phase: [0; 16], anim: [0; 16], exc: 0, frame: [0; 16] };
                for k in 0..16 { c.phase[k] = (self.next() & 15) as i32; c.anim[k] = (self.next() & 3) as usize; }
                self.step_crowd(&mut c);
                self.crowds.insert(i, c);
            },
            WOp::Delta(_) => for o in anims.0.iter_mut().filter(|o| o.inst == i && o.budget.is_some()) {
                o.budget = Some(0.0);
                o.t = o.def.start.max(0.0) / 30.0;
                o.dir = 1.0;
            },
            // SetParam (TexFlip 0x1433d8: 1 rate, 2 frame, 3 step, 4 visible; UVScroll: 1/2 du/dv,
            // 3/4 run/pause s, 5/6 the u/v offset, 7/8 an object flag; Counter 0x13b2f0; AnimDelta:
            // 2 adds N/30 s)
            WOp::Param(k, v) => {
                let model = level.instances.get(i).filter(|x| x.model_id >= 0).map(|x| x.model_id as usize);
                if let Some(s) = model.and_then(|m| self.scrolls.get_mut(&m)).filter(|_| !self.flips.contains_key(&i)) {
                    match k {
                        1 if v.abs() <= 1.0 => s.d.x = v,
                        2 if v.abs() <= 1.0 => s.d.y = v,
                        3 if (0.0..=60.0).contains(&v) => s.def.run = v,
                        4 if (0.0..=60.0).contains(&v) => s.def.pause = v,
                        5 if v.abs() <= 4.0 => s.uv.x = v,
                        6 if v.abs() <= 4.0 => s.uv.y = v,
                        _ => {}
                    }
                } else if let Some(c) = self.counters.get_mut(&i) {
                    // 1: hit N counts once; 2: only once N - 1 has been hit; 3: always counts
                    let bit = if (1.0..=10.0).contains(&v) && v.fract() == 0.0 { 1u16 << (v as u32 - 1) } else { 0 };
                    let counts = match k {
                        1 => c.bits & bit == 0 && { c.bits |= bit; true },
                        2 => c.bits & bit == 0 && (v == 1.0 || c.bits & (bit >> 1) != 0) && { c.bits |= if v == 1.0 { 1 } else { bit }; true },
                        3 => true,
                        _ => false,
                    };
                    if counts && c.count > 0 { c.count -= 1; }
                } else if let Some(f) = self.flips.get_mut(&i) {
                    match k {
                        1 => f.rate = v,
                        2 => if (0.0..9.0).contains(&v) { f.frame = (v.ceil() as i32).min(f.count - 1) },
                        3 => { f.frame += 1; if f.frame >= f.count { f.frame = 1.min(f.count - 1); } }
                        _ => {}
                    }
                } else if k == 2 {
                    for o in anims.0.iter_mut().filter(|o| o.inst == i) { if let Some(b) = o.budget.as_mut() { *b += v / 30.0; } }
                }
            }
            _ => {}
        }
    }

    /// A restart: every controller gone (glass uncracked, counters full, flips, scrolls and flags
    /// stopped, crowds still), script threads dropped; the level wakes again on the next frame and
    /// its always-on scripts set things going as at the start. Keyframed clips go back to their
    /// start (AnimDelta clips wait for their script again).
    pub fn reset_run(&mut self, anims: &mut AnimObjects, delta: &HashMap<usize, crate::logic::AnimDef>) {
        self.flips.clear();
        self.scrolls.clear();
        self.cracks.clear();
        self.counters.clear();
        self.crowds.clear();
        self.busy.clear();
        self.threads.clear();
        self.fire.clear();
        self.killed.clear();
        self.broken.clear();
        self.opened.clear();
        self.impact = 0.0;
        self.started = false;
        self.counting = false;
        for f in self.flags.iter_mut() { f.active = false; f.phase = 0.0; f.rate = 0.0; f.amp = f.amp0; f.life = 0; }
        for o in anims.0.iter_mut() {
            if o.prop.is_some() || (o.budget.is_none() && o.def.random_start) { continue; }
            o.t = if o.def.reverse && o.budget.is_none() { o.len } else { o.def.start.max(0.0) / 30.0 };
            o.dir = if o.def.reverse && o.budget.is_none() { -1.0 } else { 1.0 };
            if o.budget.is_some() || delta.contains_key(&o.inst) { o.budget = Some(0.0); }
        }
    }

    // ------------------------------------------------------------ controllers (one 60 Hz tick)

    fn tick(&mut self) {
        let mut dead = Vec::new();
        let mut r = Vec::new();
        for _ in 0..self.flips.len() { r.push(self.rand()); }
        for ((&i, f), r) in self.flips.iter_mut().zip(r) {
            if f.life > 0 { f.life -= 1; if f.life == 0 { dead.push(i); continue; } }
            if f.rate <= 0.0 { continue; }
            f.acc += f.rate;
            if f.acc < 1.0 { continue; }
            f.acc -= 1.0;
            match f.state {
                0 => {
                    if f.def.start == -2 && f.count > 1 { f.frame = (r * (f.count - 1) as f32).ceil() as i32; }
                    else if !f.def.back { f.frame += 1; if f.frame >= f.count { f.frame = 0; } }
                    else { f.frame -= 1; if f.frame < 0 { f.frame = f.count - 1; } }
                }
                // blink: the next frame for 6 ticks, then on for 60 / (Speed (0.25 + 0.75 r)) ticks
                1 => { f.state = 2; f.frame += 1; f.rate = 1.0 / 6.0; if f.frame >= f.count { f.frame = 0; } }
                2 => { f.state = 1; f.frame += 1; f.rate = f.base * (0.25 + 0.75 * r); if f.frame >= f.count { f.frame = 0; } }
                _ => {}
            }
        }
        for i in dead { self.flips.remove(&i); }
        self.scrolls.retain(|_, s| {
            if s.life > 0 { s.life -= 1; if s.life == 0 { return false; } }
            let (run, pause) = (s.def.run, s.def.pause);
            if run <= 0.0 && pause <= 0.0 { return true; }
            s.tau += TICK;
            if !s.running {
                if s.tau >= pause { s.tau = 0.0; if run > 0.0 { s.running = true; } }
                return true;
            }
            if run <= s.tau {
                s.tau = 0.0;
                if pause > 0.0 { s.running = false; }
                if s.def.mode == 1 || s.def.mode == 2 { s.d = -s.d; }
                if !s.running { return true; }
            }
            // mode 1 eases in and out over the run
            s.uv += if s.def.mode == 1 { s.d * s.tau.min(run - s.tau) / run } else { s.d };
            for c in [&mut s.uv.x, &mut s.uv.y] { if *c > 1.0 { *c -= 1.0 } else if *c < -1.0 { *c += 1.0 } }
            true
        });
        // cCrackedNode update 0x1455a0: out of health, its EffectTriggerSlot runs (no slot: it
        // just ends); out of time, it ends
        let mut ended = Vec::new();
        for (&i, c) in self.cracks.iter_mut() {
            if c.cool > 0 { c.cool -= 1; }
            if c.life > 0 { c.life -= 1; if c.life == 0 { ended.push(i); continue; } }
            if c.health <= 0.0 && !c.fired { c.fired = true; self.fire.push(i); }
        }
        for i in ended { self.cracks.remove(&i); }
        // cCounterNode update 0x13b178: counted down, its EffectTriggerSlot runs and it ends
        let fire = &mut self.fire;
        self.counters.retain(|&i, c| {
            if c.count < 1 { fire.push(i); return false; }
            !(c.life > 0 && { c.life -= 1; c.life == 0 })
        });
        // cCrowdBoxNode update 0x145c88: every 5 frames
        if self.ticks.is_multiple_of(5) {
            let mut crowds = std::mem::take(&mut self.crowds);
            for c in crowds.values_mut() { self.step_crowd(c); }
            self.crowds = crowds;
        }
        self.ticks = self.ticks.wrapping_add(1);
        for f in self.flags.iter_mut().filter(|f| f.active) {
            if f.life > 0 { f.life -= 1; if f.life == 0 { f.amp = 0.0; } }
            f.phase = (f.phase + f.rate).fract();
        }
        self.busy.retain(|_, t| { if *t > 0 { *t -= 1; } *t != 0 });
    }
    /// `cCrowdBoxNode_StepCells` 0x145e08: unless the crowd is frozen (excitement 0) each cell's
    /// phase steps; at the end of a sequence a cell picks a new one and may pause (by excitement,
    /// up to 63 steps); cell k shows frame SEQ[anim][(phase + k) & 15] (the first while paused).
    fn step_crowd(&mut self, c: &mut Crowd) {
        if c.exc != 0 {
            for k in 0..16 {
                c.phase[k] += 1;
                if c.phase[k] > 15 && c.phase[k] & 15 == 0 {
                    let r = self.next();
                    c.anim[k] = (r & 3) as usize;
                    if c.exc < 7 && (r & 0xff) < CROWD_PAUSE[c.exc as usize] { c.phase[k] = -((r & 0x3f) as i32); }
                }
            }
        }
        for k in 0..c.n {
            c.frame[k] = CROWD_SEQ[c.anim[k]][if c.phase[k] < 0 { 0 } else { (c.phase[k] as usize + k) & 15 }];
        }
    }
}

impl Crack {
    /// `cCrackedNode_ApplyImpact` 0x145808: health -= |impact| × 0.036, then 30 ticks before the next.
    fn impact(&mut self, impact: f32) {
        if self.health > 0.0 { self.cool = 30; self.health -= impact.abs() * 0.036; }
    }
}

/// How hard a rider hits glass (`Boarder_InstanceCollisions`' contact record: the direction of
/// travel · the normal, times the velocity · the normal), cm/s.
fn impact(vel: Vec3, normal: Vec3) -> f32 {
    let v = vel * 100.0;
    let l = v.length();
    if l < 1e-3 { 0.0 } else { (v / l).dot(normal) * v.dot(normal) }
}

/// The 4 vertices (in file order) and their uvs of a one-quad OBJ, or None.
fn quad(path: &std::path::Path) -> Option<([Vec3; 4], [Vec2; 4])> {
    let text = std::fs::read_to_string(path).ok()?;
    let (mut v, mut vt, mut map) = (Vec::new(), Vec::new(), [usize::MAX; 4]);
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => v.push(Vec3::from_array(std::array::from_fn(|_| it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0)))),
            Some("vt") => { let a: Vec<f32> = it.take(2).filter_map(|x| x.parse().ok()).collect(); vt.push(Vec2::new(a.first().copied().unwrap_or(0.0), 1.0 - a.get(1).copied().unwrap_or(0.0))); }
            Some("f") => for w in it {
                let mut s = w.split('/');
                let (a, b) = (s.next().and_then(|x| x.parse::<usize>().ok()), s.next().and_then(|x| x.parse::<usize>().ok()));
                if let (Some(a), Some(b)) = (a, b) { if (1..=4).contains(&a) { map[a - 1] = b - 1; } }
            },
            _ => {}
        }
    }
    if v.len() != 4 { return None; }
    let uv = std::array::from_fn(|k| vt.get(map[k]).copied().unwrap_or(Vec2::ZERO));
    Some(([v[0], v[1], v[2], v[3]], uv))
}

/// An object's box (Bevy space) from its model and collision meshes.
fn bounds(level: &Level, i: usize, cache: &mut HashMap<std::path::PathBuf, ObjMesh>) -> Option<(Vec3, Vec3)> {
    let inst = level.instances.get(i)?;
    let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
    let mut parts: Vec<(std::path::PathBuf, Mat4)> = inst.collsion_model_paths.iter().flatten().map(|p| (level.dir.join("Collision").join(p), inst_m)).collect();
    if let Some(model) = level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0) {
        for (obj, obj_m) in model.model_objects.iter().zip(level.world.object_matrices(model)) {
            for part in obj.mesh_data.iter().flatten() { parts.push((level.dir.join("Meshes").join(&part.mesh_path), inst_m * obj_m)); }
        }
    }
    let mut b = (Vec3::MAX, Vec3::MIN);
    for (key, m) in parts {
        let mesh = cache.entry(key.clone()).or_insert_with(|| load_obj(&key));
        for p in &mesh.positions { let v = g2b(m.transform_point3(Vec3::from(*p))); b.0 = b.0.min(v); b.1 = b.1.max(v); }
    }
    (b.0.x <= b.1.x).then_some(b)
}

#[allow(clippy::too_many_arguments)]
pub fn world_anim(
    time: Res<SmoothDt>, game: Res<ui::Game>, level: Res<LevelRes>, race: Res<RaceRes>, rider: Res<RiderRes>, opponents: Res<Opponents>,
    wa: Option<ResMut<WorldAnim>>, mut anims: ResMut<AnimObjects>, mut materials: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>,
    props: Option<ResMut<Props>>, mut cell_mats: Query<&mut MeshMaterial3d<StandardMaterial>>, mut world: ResMut<World>,
) {
    let Some(mut wa) = wa else { return };
    let level = &level.0;
    let w = &level.logic.world;
    // a restart (World_ResetInstances 0x115fe8 with the AI world's cells dropped, 0x115df0): every
    // object's controller goes and its flags come back as loaded, the script threads are cleared
    // (0x149a88), and the cells waking again run the always-on scripts once more
    if wa.restart {
        wa.restart = false;
        wa.reset_run(&mut anims, &level.logic.anim_delta);
        world.0.reset_run();
        if std::env::var("TRICKY_WADBG").is_ok() { eprintln!("worldanim: run restarted, level state reset"); }
    }
    if game.screen == ui::Screen::Paused { return; }
    let dt = time.dt.min(0.1);
    // the level wakes: every always-on script (World_CellActivate_RunPersistant 0x25fed0)
    if !wa.started {
        wa.started = true;
        let mut cache = HashMap::new();
        wa.boxes = w.touch.keys().filter_map(|&i| bounds(level, i, &mut cache).map(|(lo, hi)| (i, lo, hi))).collect();
        let mut glass: Vec<usize> = level.glass().into_iter().collect();
        glass.sort();
        wa.glass = glass;
        wa.gates = level.gates().into_iter().collect();
        for &(i, h) in &w.wake { wa.run(w, level, &mut anims, Src::Header(h), 0, Some(i), 0); }
    }
    // the countdown starts: StartCountDown (which calls CountDownStart, the lights)
    let r = &race.0;
    if r.countdown >= 2.5 { wa.counting = false; }
    if r.state == RaceState::Countdown && !r.intro && game.screen == ui::Screen::Playing && r.countdown < 2.5 && !wa.counting {
        wa.counting = true;
        if game.event != ui::Event::FreeRide {
            if let Some(f) = w.function("StartCountDown").or_else(|| w.function("CountDownStart")) { wa.run(w, level, &mut anims, Src::Function(f), 0, None, 0); }
        }
    }
    // a rider touches a trigger with no controller on it: its collision script
    let riders: Vec<(Vec3, Vec3)> = std::iter::once((rider.0.pos, rider.0.vel)).chain(opponents.0.iter().map(|o| (o.rider.pos, o.rider.vel))).collect();
    let bodies: Vec<Vec3> = riders.iter().map(|(p, _)| *p + Vec3::Y * 0.8).collect();
    let inside = |b: Vec3, lo: Vec3, hi: Vec3| b.cmpge(lo - 0.3).all() && b.cmple(hi + 0.3).all();
    // TRICKY_WATOUCH=i[+j...][,impact]: instances i, j.. count as touched (for checks; glass takes
    // that impact in cm/s, default 0: it only cracks)
    let watouch = std::env::var("TRICKY_WATOUCH").unwrap_or_default();
    let (ids, forced_impact) = watouch.split_once(',').map_or((watouch.as_str(), 0.0), |(a, b)| (a, b.parse().unwrap_or(0.0)));
    let forced: Vec<usize> = ids.split('+').filter_map(|v| v.parse().ok()).filter(|i| !wa.busy.contains_key(i) && w.touch.contains_key(i)).collect();
    // glass is solid: a rider in contact with its triangles (riding on it, the pane just under the
    // surface he rides, or the body against it) hits it (Instance_OnPlayerContact 0x13bd40). On a
    // pane the first contact's collision script makes the crack, after that each contact is an
    // impact (cCrackedNode vf17 0x1457e8, at most one every 30 ticks); the underside's script
    // breaks the lot.
    let mut glass = Vec::new();
    for &i in wa.glass.iter().filter(|i| w.touch.contains_key(i) && !wa.busy.contains_key(i)) {
        let touch = riders.iter().find_map(|(p, v)| world.0.contact(i as u32, *p, crate::rider::BODY_RADIUS + 0.05, 0.35).map(|n| (impact(*v, n), *v)));
        let touch = touch.or_else(|| forced.contains(&i).then_some((forced_impact, rider.0.vel)));
        if let Some(t) = touch { glass.push((i, t)); }
    }
    for (i, (imp, vel)) in glass {
        wa.impact_vel = vel;
        if let Some(c) = wa.cracks.get_mut(&i) { if c.cool == 0 { c.impact(imp); } continue; }
        wa.impact = imp;
        if !level.logic.cracks.contains_key(&i) { wa.busy.insert(i, 60); }
        if let Some(&h) = w.touch.get(&i) { wa.run(w, level, &mut anims, Src::Header(h), 0, Some(i), 0); }
        wa.impact = 0.0;
    }
    // anything else: its collision script, then not again for a second
    let is_glass = |i: &usize| level.logic.cracks.contains_key(i) || wa.glass.contains(i);
    let hit: Vec<usize> = forced.iter().copied().filter(|i| !is_glass(i))
        .chain(wa.boxes.iter().filter(|(i, lo, hi)| !wa.busy.contains_key(i) && !is_glass(i) && bodies.iter().any(|b| inside(*b, *lo, *hi))).map(|b| b.0)).collect();
    for i in hit {
        wa.busy.insert(i, 60);
        if let Some(&h) = w.touch.get(&i) { wa.run(w, level, &mut anims, Src::Header(h), 0, Some(i), 0); }
    }
    // threads waiting
    let mut ready = Vec::new();
    wa.threads.retain_mut(|t| { t.wait -= dt; if t.wait <= 0.0 { ready.push((t.src, t.pc, t.inst)); false } else { true } });
    for (src, pc, inst) in ready { wa.run(w, level, &mut anims, src, pc, inst, 0); }
    // the iris doors: a script opened them; their clip runs, and the drawn door follows it
    for i in std::mem::take(&mut wa.opened) { world.0.open_gate(i as u32); }
    world.0.tick_gates(dt);
    for o in anims.0.iter_mut().filter(|o| wa.gates.contains(&o.inst)) {
        o.t = o.def.start.max(0.0) / 30.0 + world.0.gate_time(o.inst as u32).unwrap_or(0.0);
        o.budget = Some(0.0);
    }
    // controllers at 60 Hz
    wa.acc = (wa.acc + dt).min(0.25);
    while wa.acc >= TICK { wa.acc -= TICK; wa.tick(); }
    // controllers that reached their goal: the object's EffectTriggerSlot (a crack or counter with
    // no such script just ends)
    for _ in 0..4 {
        let fire = std::mem::take(&mut wa.fire);
        if fire.is_empty() { break; }
        for i in fire {
            match w.trigger.get(&i) {
                Some(&h) => wa.run(w, level, &mut anims, Src::Header(h), 0, Some(i), 0),
                None => { wa.cracks.remove(&i); }
            }
        }
    }
    // objects the scripts killed: broken glass goes (and its pieces fly) like a pane a rider
    // went through; after a restart puts it back, it can crack again
    if let Some(mut props) = props {
        let props = &mut props.0;
        let killed = std::mem::take(&mut wa.killed);
        let vel = wa.impact_vel;
        let mut thrown = Vec::new();
        for i in killed {
            world.0.dead.insert(i as u32);
            for p in props.iter_mut().filter(|p| p.inst == i && p.scripted && !p.hidden) {
                p.hidden = true;
                p.knock = Some(vel.length());
                if let Kind::Touch { pieces, .. } = &p.kind { thrown.extend(pieces.iter().copied()); }
                wa.broken.insert(i);
            }
        }
        for (n, k) in thrown.into_iter().enumerate() { if let Some(p) = props.get_mut(k) { p.throw(vel, n); } }
        let back: Vec<usize> = wa.broken.iter().copied().filter(|i| props.iter().any(|p| p.inst == *i && !p.hidden)).collect();
        for i in back { wa.broken.remove(&i); wa.cracks.remove(&i); wa.busy.remove(&i); }
    } else {
        for i in std::mem::take(&mut wa.killed) { world.0.dead.insert(i as u32); }
    }
    // the crowds: excited (4, mood 0: CrowdSys_GetMood + 4) while a rider is within 200 m,
    // otherwise frozen
    for c in wa.crowds.values_mut() {
        c.exc = if riders.iter().any(|(p, _)| p.distance(c.pos) <= 200.0) { 4 } else { 0 };
    }
    if std::env::var("TRICKY_WADBG").is_ok() {
        let f: Vec<String> = wa.flips.iter().filter(|(i, _)| level.instances[**i].instance_name.contains("StartLights")).map(|(i, f)| format!("{} frame {} life {}", level.instances[*i].instance_name, f.frame, f.life)).collect();
        for o in anims.0.iter().filter(|o| o.budget.is_some()) { eprintln!("worldanim: delta {} t {:.2} budget {:.2}", level.instances[o.inst].instance_name, o.t, o.budget.unwrap_or(0.0)); }
        for (i, c) in wa.cracks.iter() { eprintln!("worldanim: crack {} frame {} health {:.2} fired {}", level.instances[*i].instance_name, c.frame, c.health, c.fired); }
        for (i, c) in wa.counters.iter() { eprintln!("worldanim: counter {} count {} bits {:#x}", level.instances[*i].instance_name, c.count, c.bits); }
        eprintln!("worldanim: countdown {:.2} counting {} threads {} {:?}", r.countdown, r.state == RaceState::Countdown, wa.threads.len(), f);
    }
    // show it
    let wa = &mut *wa;
    for m in wa.mats.iter_mut() {
        let frame = if m.frames.is_empty() { -1 } else { wa.flips.get(&m.inst).map(|f| f.frame).or_else(|| wa.cracks.get(&m.inst).map(|c| c.frame)).unwrap_or(-1) };
        let uv = wa.scrolls.get(&m.model).map_or(Vec2::ZERO, |s| s.uv);
        if (frame, uv) == m.shown { continue; }
        m.shown = (frame, uv);
        let Some(mat) = materials.get_mut(&m.handle) else { continue };
        mat.base_color_texture = if frame >= 0 { m.frames.get(frame as usize % m.frames.len()).cloned() } else { m.base.clone() };
        mat.uv_transform = Affine2::from_translation(uv);
    }
    if let Some(mats) = wa.crowd_mats.as_ref().filter(|m| m.len() == 16) {
        for (i, c) in wa.crowds.iter() {
            for &(cell, e) in wa.crowd_cells.get(i).into_iter().flatten().filter(|(cell, _)| *cell < c.n) {
                let want = &mats[c.frame[cell]];
                if let Ok(mut m) = cell_mats.get_mut(e) { if m.0 != *want { m.0 = want.clone(); } }
            }
        }
    }
    for f in wa.flags.iter().filter(|f| f.active) {
        // column k of 1..7 swings d = A k/8 sin(2π (k/8 − phase)) along the model's X; U0 = 1
        // keeps the other end still
        let d = |k: usize| f.amp * (k as f32 / 8.0) * (std::f32::consts::TAU * (k as f32 / 8.0 - f.phase)).sin();
        let pos: Vec<[f32; 3]> = f.rows.iter().flat_map(|row| row.iter().enumerate().map(|(c, p)| {
            let k = if f.def.mirror { 7 - c } else { c };
            g2b(f.m.transform_point3(*p + Vec3::X * if k == 0 { 0.0 } else { d(k) })).to_array()
        })).collect();
        if let Some(mesh) = meshes.get_mut(&f.mesh) { mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos); }
    }
}

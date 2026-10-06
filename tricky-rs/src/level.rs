#![allow(dead_code)]
//! Loader for a level project folder as produced by SSX-Library / the SSX Multitool
//! (Patches.json, Instances.json, Models.json, Materials.json, Splines.json, Meshes/*.obj, ...).
//! Everything here stays in the game's own coordinate system: X/Y horizontal, Z up.

use bevy::math::{Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Patch {
    pub patch_name: String,
    /// x, y, width, height of this patch's rectangle inside its lightmap page (0..1)
    pub light_map_point: [f32; 4],
    #[serde(rename = "UVPoints")]
    pub uv_points: [[f32; 2]; 4],
    /// 4x4 bicubic Bezier control points, row-major
    pub points: Vec<[f32; 3]>,
    #[serde(default)]
    pub surface_type: i32,
    pub texture_path: String,
    #[serde(rename = "LightmapID")]
    pub lightmap_id: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PatchFile { patches: Vec<Patch> }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Instance {
    pub instance_name: String,
    pub location: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub light_vector1: [f32; 4],
    pub light_vector2: [f32; 4],
    pub light_vector3: [f32; 4],
    pub light_colour1: [f32; 4],
    pub light_colour2: [f32; 4],
    pub light_colour3: [f32; 4],
    pub ambent_light_colour: [f32; 4],
    #[serde(rename = "ModelID")]
    pub model_id: i32,
    #[serde(default = "yes")]
    pub visable: bool,
    #[serde(default)]
    pub player_collision: bool,
    #[serde(default)]
    pub collsion_mode: i32,
    #[serde(default)]
    pub collsion_model_paths: Option<Vec<String>>,
    /// 1e30 on things that never move; small on things that get knocked over
    #[serde(rename = "U0", default = "immovable")]
    pub u0: f32,
    #[serde(default = "minus_one")]
    pub effect_slot_index: i32,
}
fn minus_one() -> i32 { -1 }

// The parts of SSFLogic.json that say what happens when a rider touches something.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EffectSlot { #[serde(default = "minus_one")] collision_effect_slot: i32, #[serde(default = "minus_one")] effect_trigger_slot: i32 }
#[derive(Deserialize)]
struct Type0 { #[serde(rename = "SubType", default)] sub_type: i32 }
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceRef { instance_index: i32 }
#[derive(Deserialize)]
struct Effect {
    #[serde(rename = "MainType", default)] main_type: i32,
    #[serde(default)] type0: Option<Type0>,
    #[serde(rename = "Instance", default)] instance: Option<InstanceRef>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EffectHeader { #[serde(default)] effects: Vec<Effect> }
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct LogicFile { #[serde(default)] effect_slots: Vec<EffectSlot>, #[serde(default)] effect_headers: Vec<EffectHeader> }
fn immovable() -> f32 { 1e30 }
impl Instance {
    /// Path markers, crash bags and the like: knocked away when a rider hits them.
    pub fn knockable(&self) -> bool { self.player_collision && self.collsion_mode == 3 && self.u0 < 1e20 }
}
fn yes() -> bool { true }
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstanceFile { instances: Vec<Instance> }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshRef {
    pub mesh_path: String,
    #[serde(rename = "MaterialID")]
    pub material_id: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ModelObject {
    #[serde(rename = "ParentID")]
    pub parent_id: i32,
    #[serde(default)]
    pub mesh_data: Option<Vec<MeshRef>>,
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    pub model_name: String,
    pub model_objects: Vec<ModelObject>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ModelFile { models: Vec<Model> }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Material {
    #[serde(default, deserialize_with = "null_string")]
    pub material_name: String,
    /// empty for the odd untextured material
    #[serde(default, deserialize_with = "null_string")]
    pub texture_path: String,
}
fn null_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MaterialFile { materials: Vec<Material> }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Segment { pub points: Vec<[f32; 3]> }
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Spline {
    pub spline_name: String,
    pub segments: Vec<Segment>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SplineFile { splines: Vec<Spline> }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaceLine { pub path_pos: [f32; 3], pub path_points: Vec<[f32; 3]> }
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AiFile { #[serde(default)] pub race_lines: Vec<RaceLine> }

/// Triangle soup read from an OBJ file.
#[derive(Default, Clone)]
pub struct ObjMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
}

pub fn load_obj(path: &Path) -> ObjMesh {
    let text = fs::read_to_string(path).unwrap_or_default();
    let (mut v, mut vt, mut vn) = (Vec::new(), Vec::new(), Vec::new());
    let mut out = ObjMesh::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => v.push(read3(&mut it)),
            Some("vn") => vn.push(read3(&mut it)),
            Some("vt") => { let a = read3(&mut it); vt.push([a[0], a[1]]); }
            Some("f") => {
                let corners: Vec<&str> = it.collect();
                // fan-triangulate, in case a face has more than three corners
                for i in 1..corners.len().saturating_sub(1) {
                    for c in [corners[0], corners[i], corners[i + 1]] {
                        let mut idx = c.split('/').map(|s| s.parse::<usize>().unwrap_or(0));
                        let (pi, ti, ni) = (idx.next().unwrap_or(0), idx.next().unwrap_or(0), idx.next().unwrap_or(0));
                        out.positions.push(v.get(pi.wrapping_sub(1)).copied().unwrap_or([0.0; 3]));
                        // OBJ puts V=0 at the bottom of the image, textures have row 0 at the top
                        let t = vt.get(ti.wrapping_sub(1)).copied().unwrap_or([0.0; 2]);
                        out.uvs.push([t[0], 1.0 - t[1]]);
                        out.normals.push(vn.get(ni.wrapping_sub(1)).copied().unwrap_or([0.0, 0.0, 1.0]));
                    }
                }
            }
            _ => {}
        }
    }
    out
}
fn read3<'a>(it: &mut impl Iterator<Item = &'a str>) -> [f32; 3] {
    let mut a = [0.0f32; 3];
    for x in a.iter_mut() { *x = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0); }
    a
}

pub struct ModelSet {
    pub dir: PathBuf,
    pub models: Vec<Model>,
    pub materials: Vec<Material>,
}
impl ModelSet {
    pub fn load(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            models: read::<ModelFile>(&dir.join("Models.json")).map(|f| f.models).unwrap_or_else(|e| { eprintln!("models not read: {e}"); Vec::new() }),
            materials: read::<MaterialFile>(&dir.join("Materials.json")).map(|f| f.materials).unwrap_or_else(|e| { eprintln!("materials not read: {e}"); Vec::new() }),
        }
    }
    /// Local matrix of every object of a model, parents applied.
    pub fn object_matrices(&self, model: &Model) -> Vec<Mat4> {
        let local: Vec<Mat4> = model.model_objects.iter().map(|o| {
            Mat4::from_scale_rotation_translation(
                o.scale.map(Vec3::from).unwrap_or(Vec3::ONE),
                o.rotation.map(quat4).unwrap_or(Quat::IDENTITY),
                o.position.map(Vec3::from).unwrap_or(Vec3::ZERO),
            )
        }).collect();
        (0..local.len()).map(|i| {
            let (mut m, mut p, mut guard) = (local[i], model.model_objects[i].parent_id, 0);
            while p >= 0 && (p as usize) < local.len() && guard < 64 {
                m = local[p as usize] * m;
                p = model.model_objects[p as usize].parent_id;
                guard += 1;
            }
            m
        }).collect()
    }
}

pub struct Level {
    pub dir: PathBuf,
    pub patches: Vec<Patch>,
    pub instances: Vec<Instance>,
    pub world: ModelSet,
    pub sky: ModelSet,
    pub splines: Vec<Spline>,
    pub ai: AiFile,
    /// Instances that vanish when touched (glass, pickups, fences), each with the instances its
    /// script points at (the broken pieces).
    pub touch: std::collections::HashMap<usize, Vec<usize>>,
    /// Hidden instances that are the broken pieces of something in `touch`.
    pub pieces: std::collections::HashSet<usize>,
}

impl Level {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let patches = read::<PatchFile>(&dir.join("Patches.json"))?.patches;
        let instances = read::<InstanceFile>(&dir.join("Instances.json")).map(|f| f.instances).unwrap_or_default();
        // An object "dies" on contact when its collision script (or the trigger it fires) contains
        // effect type 0 / sub-type 5 ("dead node"). Type 7 effects name other instances to set off.
        let logic = read::<LogicFile>(&dir.join("SSFLogic.json")).unwrap_or_else(|e| { eprintln!("level scripts not read: {e}"); LogicFile::default() });
        let mut touch: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
        for (i, inst) in instances.iter().enumerate() {
            let Some(slot) = logic.effect_slots.get(inst.effect_slot_index.max(0) as usize).filter(|_| inst.effect_slot_index >= 0) else { continue };
            let effects: Vec<&Effect> = [slot.collision_effect_slot, slot.effect_trigger_slot].iter()
                .filter_map(|h| logic.effect_headers.get((*h).max(0) as usize).filter(|_| *h >= 0)).flat_map(|h| h.effects.iter()).collect();
            if effects.iter().any(|e| e.main_type == 0 && e.type0.as_ref().is_some_and(|t| t.sub_type == 5)) {
                let linked = effects.iter().filter(|e| e.main_type == 7).filter_map(|e| e.instance.as_ref())
                    .map(|r| r.instance_index).filter(|r| *r >= 0 && (*r as usize) < instances.len() && *r as usize != i).map(|r| r as usize).collect();
                touch.insert(i, linked);
            }
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            patches,
            pieces: touch.values().flatten().copied().filter(|i| !instances[*i].visable).collect(),
            instances,
            touch,
            world: ModelSet::load(dir),
            sky: ModelSet::load(&dir.join("Skybox")),
            splines: read::<SplineFile>(&dir.join("Splines.json")).map(|f| f.splines).unwrap_or_default(),
            ai: read::<AiFile>(&dir.join("AIP.json")).unwrap_or_default(),
        })
    }
    /// Instances that move, break or get picked up: handled one by one, not as static scenery.
    pub fn is_dynamic(&self, i: usize) -> bool {
        self.instances[i].knockable() || self.touch.contains_key(&i) || self.pieces.contains(&i)
    }
    /// The main route down the course in game coordinates: race line 0, then whichever line
    /// starts where the previous one ended, and so on.
    pub fn main_line(&self) -> Vec<Vec3> {
        let lines: Vec<Vec<Vec3>> = self.ai.race_lines.iter().map(|r| {
            let mut p = Vec3::from(r.path_pos);
            let mut v = vec![p];
            for d in &r.path_points { p += Vec3::from(*d); v.push(p); }
            v
        }).collect();
        let mut out: Vec<Vec3> = Vec::new();
        let mut used = vec![false; lines.len()];
        let mut cur = 0usize;
        while cur < lines.len() && !used[cur] {
            used[cur] = true;
            out.extend(lines[cur].iter().copied());
            let end = *out.last().unwrap();
            cur = (0..lines.len()).filter(|&i| !used[i])
                .min_by(|&a, &b| (lines[a][0] - end).length().total_cmp(&(lines[b][0] - end).length()))
                .filter(|&i| (lines[i][0] - end).length() < 2500.0)
                .unwrap_or(lines.len());
        }
        out
    }
    /// Start of the first race line and the direction it heads in (game coordinates).
    pub fn start(&self) -> (Vec3, Vec3) {
        if let Some(r) = self.ai.race_lines.first() {
            let pos = Vec3::from(r.path_pos);
            let dir = r.path_points.iter().take(3).fold(Vec3::ZERO, |a, p| a + Vec3::from(*p));
            return (pos, dir.normalize_or(Vec3::Y));
        }
        let p = self.patches.first().map(|p| Vec3::from(p.points[0])).unwrap_or(Vec3::ZERO);
        (p + Vec3::Z * 500.0, Vec3::Y)
    }
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn quat4(q: [f32; 4]) -> Quat {
    let q = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
    if q.length_squared() < 1e-8 { Quat::IDENTITY } else { q.normalize() }
}

/// Evaluate a bicubic Bezier patch. `s` runs along a row (columns 0..3), `t` down the rows.
pub fn bezier_patch(p: &[[f32; 3]], s: f32, t: f32) -> (Vec3, Vec3) {
    let b = |x: f32| { let i = 1.0 - x; [i * i * i, 3.0 * i * i * x, 3.0 * i * x * x, x * x * x] };
    let d = |x: f32| { let i = 1.0 - x; [-3.0 * i * i, 3.0 * i * i - 6.0 * i * x, 6.0 * i * x - 3.0 * x * x, 3.0 * x * x] };
    let (bs, bt, ds, dt) = (b(s), b(t), d(s), d(t));
    let (mut pos, mut du, mut dv) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
    for r in 0..4 {
        for c in 0..4 {
            let cp = Vec3::from(p[r * 4 + c]);
            pos += cp * (bs[c] * bt[r]);
            du += cp * (ds[c] * bt[r]);
            dv += cp * (bs[c] * dt[r]);
        }
    }
    let mut n = du.cross(dv).normalize_or(Vec3::Z);
    if n.z < 0.0 { n = -n; }
    (pos, n)
}

/// Texture coordinate at (s, t) on a patch. The four stored corners go *down the rows first*:
/// [0] = first row, first column; [1] = last row, first column; [2] = first row, last column;
/// [3] = last row, last column (this is how IceSaw maps them too).
pub fn bilerp(c: &[[f32; 2]; 4], s: f32, t: f32) -> Vec2 {
    let first_col = Vec2::from(c[0]).lerp(Vec2::from(c[1]), t);
    let last_col = Vec2::from(c[2]).lerp(Vec2::from(c[3]), t);
    first_col.lerp(last_col, s)
}

pub fn cubic(p: &[[f32; 3]], t: f32) -> Vec3 {
    let i = 1.0 - t;
    Vec3::from(p[0]) * (i * i * i) + Vec3::from(p[1]) * (3.0 * i * i * t) + Vec3::from(p[2]) * (3.0 * i * t * t) + Vec3::from(p[3]) * (t * t * t)
}

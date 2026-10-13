#![allow(dead_code)]
//! Loader for a level project folder as produced by SSX-Library / the SSX Multitool
//! (Patches.json, Instances.json, Models.json, Materials.json, Splines.json, Meshes/*.obj, ...).
//! Everything here stays in the game's own coordinate system: X/Y horizontal, Z up.
//!
//! STANDIN: SSX-Library's JSON project folder, for the game's own level files and their readers (DATA/MODELS/*.pbd and the rest)

use glam::{Mat4, Quat, Vec2, Vec3};
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
    /// show-off ramps: only there (drawn and solid) in show-off events
    #[serde(default)]
    pub trick_only_patch: bool,
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
    /// flag 0x80: touching it bounces the rider off (`Boarder_InstanceBounce`); without it an
    /// object with no surface type is not solid (its touch only runs its scripts)
    #[serde(default)]
    pub player_bounce: bool,
    #[serde(default)]
    pub collsion_mode: i32,
    #[serde(default)]
    pub collsion_model_paths: Option<Vec<String>>,
    /// 1e30 on things that never move; small on things that get knocked over
    #[serde(rename = "U0", default = "immovable")]
    pub u0: f32,
    #[serde(default = "minus_one")]
    pub effect_slot_index: i32,
    /// row of the surface table its collision uses: the object is ground like the terrain; -1: it
    /// is never ground, only bounced off (PlayerBounce) or touched
    #[serde(default = "minus_one")]
    pub surface_type: i32,
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
    /// keyframe curves (`cMeshAnimFrame`): base pose U1..U6, channel mask, cubic segments
    #[serde(default)]
    pub animation: Option<ObjAnim>,
}
pub use crate::anim::ObjAnim;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    pub model_name: String,
    /// the animation's length in frames (30 a second)
    #[serde(default)]
    pub anim_time: f32,
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
    /// the frames a TexFlip controller (`cTexFlipNode`) steps through, frame 0 first
    #[serde(default)]
    pub texture_flipbook: Option<Vec<String>>,
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
pub struct RaceLine { pub path_pos: [f32; 3], pub path_points: Vec<[f32; 3]>, #[serde(default)] pub distance_to_finish: f32, #[serde(default)] pub path_events: Vec<PathEvent> }
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AiFile { #[serde(default)] pub race_lines: Vec<RaceLine>, #[serde(default, rename = "AIPaths")] pub ai_paths: Vec<AiPath> }
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AiPath { pub path_pos: [f32; 3], pub path_points: Vec<[f32; 3]>, #[serde(default)] pub path_events: Vec<PathEvent>, #[serde(default = "fifty", rename = "U3")] pub u3: f32, #[serde(default = "yes")] pub respawnable: bool }
fn fifty() -> f32 { 50.0 }

impl AiFile {
    /// The paths of a course's `.aip` (row E3d, as the game loads it) in the level folder's form: race lines from
    /// the event paths (their points as offsets: direction × length; the distance to the finish from parameter 0),
    /// AI paths with their U3 and respawnable flag; the events with the file's type codes.
    pub fn from_aip(a: &crate::aip::Aip) -> AiFile {
        let pts = |p: &crate::aip::AipPath| p.points.iter().map(|(d, l)| [d[0] * l, d[1] * l, d[2] * l]).collect::<Vec<_>>();
        let evs = |p: &crate::aip::AipPath| p.events.iter().map(|e| PathEvent { event_type: e.code, event_value: e.value as i32, event_start: e.start, event_end: e.end }).collect::<Vec<_>>();
        AiFile {
            race_lines: a.event_paths.iter().map(|p| RaceLine { path_pos: p.start, path_points: pts(p), distance_to_finish: p.param(0).map_or(0.0, f32::from_bits), path_events: evs(p) }).collect(),
            ai_paths: a.ai_paths.iter().map(|p| AiPath { path_pos: p.start, path_points: pts(p), path_events: evs(p), u3: p.u3() as f32, respawnable: p.respawnable() != 0 }).collect(),
        }
    }
}
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "PascalCase")]
pub struct PathEvent { pub event_type: i32, pub event_value: i32, pub event_start: f32, pub event_end: f32 }

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
    /// what the level scripts do (event hides, resets, teleports, boosts)
    pub logic: crate::logic::LevelLogic,
    /// instances the event scripts switch on and off
    pub switchable: std::collections::HashSet<usize>,
    /// The meshes read from the disc (`from_disc`), by the path the folder would have them at; empty for a folder.
    pub meshes: std::collections::HashMap<PathBuf, ObjMesh>,
    /// The textures and lightmaps read from the disc (`from_disc`), by the path the folder would have them at
    /// (`<dir>/Textures/<n>.png`, `<dir>/Lightmaps/<n>.png`, `<dir>/Skybox/Textures/<n>.png`); empty for a folder.
    pub images: std::collections::HashMap<PathBuf, crate::ssh::Image>,
}

impl Level {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let patches = read::<PatchFile>(&dir.join("Patches.json"))?.patches;
        let instances = read::<InstanceFile>(&dir.join("Instances.json")).map(|f| f.instances).unwrap_or_default();
        // An object "dies" on contact when its collision script (or the trigger it fires) contains
        // effect type 0 / sub-type 5 ("dead node"). Type 7 effects name other instances to set off.
        let logic = read::<LogicFile>(&dir.join("SSFLogic.json")).unwrap_or_else(|e| { eprintln!("level scripts not read: {e}"); LogicFile::default() });
        let touch = touch_effects(&instances, &logic);
        let slot_of: Vec<i32> = instances.iter().map(|i| i.effect_slot_index).collect();
        let logic = crate::logic::LevelLogic::load(dir, &slot_of);
        let switchable = logic.switchable();
        Ok(Self {
            logic, switchable,
            dir: dir.to_path_buf(),
            patches,
            pieces: touch.values().flatten().copied().filter(|i| !instances[*i].visable).collect(),
            instances,
            touch,
            world: ModelSet::load(dir),
            sky: ModelSet::load(&dir.join("Skybox")),
            splines: read::<SplineFile>(&dir.join("Splines.json")).map(|f| f.splines).unwrap_or_default(),
            ai: read::<AiFile>(&dir.join("AIP.json")).unwrap_or_default(),
            meshes: Default::default(),
            images: Default::default(),
        })
    }
    /// A mesh by its path in the level (`<dir>/Meshes/<n>.obj`, `<dir>/Skybox/Meshes/<n>.obj`,
    /// `<dir>/Collision/<n>.obj`): the disc's when the level came from it, else the folder's file.
    pub fn mesh(&self, path: &Path) -> ObjMesh {
        self.meshes.get(path).cloned().unwrap_or_else(|| load_obj(path))
    }

    /// The level from the player's disc (row E3a4), as `Course_LoadRenderData` 0x25edf0 names its files: the
    /// course's archive `data/models/<file>.big` mounted, then `|data/models/<file>.pbd` (`tricky_data::pbd`, row
    /// E3a), `.ssf` (the objects' physics records and collision meshes, rows E3c1 / E3a5), `.aip` (the paths, row
    /// E3d), `<file>_sky.pbd` (`Course_LoadSkies` 0x185b18) and the `.map` (the tools' names; the game never reads
    /// it). The records are put in the folder's form, field for field what SSX-Library exported from the same files
    /// (checked: tests/level_disc.rs), and the meshes are kept under the paths the folder has them at.
    ///
    /// `scripts`: the level's folder (its `dir`, under which the meshes and textures are kept, row E3a7), which need
    /// not exist. STANDIN: its SSFLogic.json for what the scripts do (`logic`, `touch`), until the app runs the
    /// `.ssf`'s scripts itself (row E9b2); without one the level has no scripted behaviour.
    pub fn from_disc(files: &mut crate::big::Files, folder: &str, scripts: Option<&Path>) -> Result<Self, String> {
        let name = crate::aip::big_name(folder).ok_or_else(|| format!("{folder}: not a course"))?;
        files.mount(&format!("data/models/{name}.big")).map_err(|e| e.to_string())?;
        let mut get = |ext: &str| files.load(&format!("|data/models/{name}{ext}")).map_err(|e| e.to_string())?.ok_or_else(|| format!("{name}{ext}: not on the disc"));
        let pbd_bytes = get(".pbd")?;
        let ssf_bytes = get(".ssf")?;
        let sky_bytes = get("_sky.pbd")?;
        let map = crate::map::Map::parse(&String::from_utf8_lossy(&get(".map").unwrap_or_default()));
        let pbd = crate::pbd::Pbd::parse(&pbd_bytes).ok_or("the .pbd does not parse")?;
        let sky = crate::pbd::Pbd::parse(&sky_bytes).ok_or("the sky's .pbd does not parse")?;
        let ssf = crate::ssf::Ssf::parse(&ssf_bytes, pbd.instances.len() as u32).map_err(|e| format!("the .ssf: {e:?}"))?;
        let ai = crate::aip::from_disc(files, folder, "aip").map(|a| AiFile::from_aip(&a)).unwrap_or_default();
        let dir = scripts.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(format!("disc:{folder}")));
        let named = |section: &str, k: usize| map.name(section, k).unwrap_or_default().to_string();

        let patches = pbd.patches.iter().enumerate().map(|(k, raw)| {
            let p = crate::pbd::PatchRec::parse(raw).ok_or("a patch record is short")?;
            Ok(Patch {
                patch_name: named("PATCHES", k),
                light_map_point: p.lightmap,
                uv_points: p.uv.map(|c| [c[0], c[1]]),
                points: p.bezier().to_vec(),
                surface_type: p.surface as i32,
                trick_only_patch: p.showoff_only(),
                texture_path: format!("{:04}.png", p.texture),
                lightmap_id: p.lightmap_page as usize,
            })
        }).collect::<Result<Vec<_>, String>>()?;

        // the collision meshes, numbered as the folder's Collision/<n>.obj: group by group, mesh by mesh
        let mut meshes = std::collections::HashMap::new();
        let mut first_of_group = vec![];
        for g in &ssf.collision {
            first_of_group.push(meshes.len());
            for m in g {
                let mut o = ObjMesh::default();
                for (t, pl) in m.tris.iter().zip(&m.planes) {
                    for i in t {
                        let v = m.verts[*i as usize];
                        o.positions.push([v[0], v[1], v[2]]);
                        o.normals.push([pl[0], pl[1], pl[2]]);
                        o.uvs.push([0.0, 1.0]);
                    }
                }
                meshes.insert(dir.join("Collision").join(format!("{}.obj", meshes.len())), o);
            }
        }
        let instances = pbd.instances.iter().enumerate().map(|(k, w)| {
            let i = crate::pbd::InstanceRec::from_words(w);
            let p = ssf.physics[ssf.object_physics[k] as usize];
            let d = p.defaults();
            // the folder keeps the matrix as a location, a rotation and a scale (equal to float noise)
            let (scale, rot, loc) = Mat4::from_cols_array_2d(&i.matrix).to_scale_rotation_translation();
            let lv = |n: usize| { let v = i.light_dir(n); [v[0], v[1], v[2], 0.0] };
            let group = (p.collision_mode() != 3).then_some(p.group()).filter(|g| *g >= 0).map(|g| g as usize);
            Instance {
                instance_name: named("INTERNAL INSTANCES", k),
                location: loc.into(),
                rotation: rot.into(),
                scale: scale.into(),
                light_vector1: lv(0),
                light_vector2: lv(1),
                light_vector3: lv(2),
                light_colour1: i.light_colours[0],
                light_colour2: i.light_colours[1],
                light_colour3: i.light_colours[2],
                ambent_light_colour: i.ambient,
                model_id: i.model,
                visable: d & 1 != 0,
                player_collision: d & 0x20 != 0,
                player_bounce: d & 0x80 != 0,
                collsion_mode: p.collision_mode() as i32,
                collsion_model_paths: group.map(|g| (0..ssf.collision[g].len()).map(|j| format!("{}.obj", first_of_group[g] + j)).collect()),
                u0: p.mass(),
                effect_slot_index: p.slot() as i32,
                surface_type: p.surface(),
            }
        }).collect::<Vec<_>>();

        let world = model_set(&pbd, &pbd_bytes, &map, dir.clone(), &mut meshes)?;
        let sky = model_set(&sky, &sky_bytes, &crate::map::Map::default(), dir.join("Skybox"), &mut meshes)?;
        let splines = pbd.splines.iter().enumerate().map(|(k, w)| {
            let s = crate::pbd::SplineRec(*w);
            let segments = (0..s.segments() as usize).map(|n| Segment { points: crate::pbd::SegmentRec(pbd.segments[s.first() as usize + n]).bezier().to_vec() }).collect();
            Spline { spline_name: named("SPLINES", k), segments }
        }).collect();

        // the textures: `Course_RemapTextures` 0x2606f8 / `Course_RemapLightmapTextures` 0x260898 load the course's
        // `.ssh` and its lightmaps' (`World_LoadTextures`) and point the materials' and patches' texture numbers at
        // the shapes in file order; the lightmaps' file is the name cut to 6 letters, then `_L` (`Course_LoadRenderData`
        // 0x25edf0); the sky's is `<file>_sky.ssh` (`Course_LoadSkies` 0x185b18)
        let lightmaps = format!("{}_L", &name[..name.len().min(6)]);
        let mut images = std::collections::HashMap::new();
        for (file, sub) in [(name.to_string(), "Textures"), (lightmaps, "Lightmaps"), (format!("{name}_sky"), "Skybox/Textures")] {
            let bytes = files.load(&format!("|data/models/{file}.ssh")).map_err(|e| e.to_string())?.ok_or_else(|| format!("{file}.ssh: not on the disc"))?;
            let ssh = crate::ssh::SshFile::open(&bytes).map_err(|e| format!("{file}.ssh: {e}"))?;
            for (k, shape) in ssh.shapes.iter().enumerate() {
                if let Some(img) = &shape.image { images.insert(dir.join(sub).join(format!("{k:04}.png")), img.clone()); }
            }
        }

        // the scripts' part, as `load` does it (STANDIN: from the folder's SSFLogic.json, see above)
        let logic_file = scripts.and_then(|d| read::<LogicFile>(&d.join("SSFLogic.json")).ok()).unwrap_or_default();
        let touch = touch_effects(&instances, &logic_file);
        let slot_of: Vec<i32> = instances.iter().map(|i| i.effect_slot_index).collect();
        let logic = scripts.map(|d| crate::logic::LevelLogic::load(d, &slot_of)).unwrap_or_default();
        let switchable = logic.switchable();
        Ok(Self {
            pieces: touch.values().flatten().copied().filter(|i| !instances[*i].visable).collect(),
            logic, switchable, dir, patches, instances, touch, world, sky, splines, ai, meshes, images,
        })
    }
    /// Instances that move, break or get picked up: handled one by one, not as static scenery.
    pub fn is_dynamic(&self, i: usize) -> bool {
        self.instances[i].knockable() || self.touch.contains_key(&i) || self.pieces.contains(&i) || self.logic.movers.contains_key(&i)
    }
    /// Cracked glass and what breaks with it (megaplex's glass floors): the pane, the invisible
    /// surface riders ride on and the underside. Solid until the scripts break them.
    pub fn glass(&self) -> std::collections::HashSet<usize> {
        let cracks = &self.logic.cracks;
        let mut g: std::collections::HashSet<usize> = cracks.keys().copied().collect();
        for c in cracks.keys() { if let Some(l) = self.touch.get(c) { g.extend(l.iter().copied()); } }
        for (j, l) in &self.touch { if l.iter().any(|x| cracks.contains_key(x)) { g.insert(*j); } }
        g.retain(|i| self.instances.get(*i).is_some_and(|x| x.player_collision && x.collsion_mode == 1));
        g
    }
    /// Doors a touch script opens with a keyframed clip and that put a rider back while shut
    /// (megaplex's iris doors).
    pub fn gates(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.logic.anim_on.keys().copied().filter(|i| self.is_gate(*i)).collect();
        v.sort();
        v
    }
    /// A door: its clip is played once (mode 0) by a touch script, and touching it while shut puts
    /// the rider back. (Merqury's subway trains loop their clip: not doors.)
    pub fn is_gate(&self, i: usize) -> bool {
        self.logic.anim_on.get(&i).is_some_and(|d| d.mode == 0) && self.logic.resets.contains(&i) && !self.logic.anims.contains_key(&i)
    }
    /// The main route down the course in game coordinates: race line 0, then whichever line
    /// starts where the previous one ended, and so on.
    /// The AI path events the opponents ride by, placed in the world (game space): (where it
    /// starts, where it ends, file event type, value). Type 101 = target speed (km/h), 100 = jump
    /// (value >> 2 = speed in km/h, bit 1 = spin, bit 0 = flip). (File type = runtime id + 75.)
    pub fn ai_events(&self) -> Vec<(Vec3, Vec3, i32, i32)> {
        let mut out = Vec::new();
        for path in &self.ai.ai_paths {
            let mut p = Vec3::from(path.path_pos);
            let mut pts = vec![p];
            for d in &path.path_points { p += Vec3::from(*d); pts.push(p); }
            let at = |dist: f32| {
                let mut left = dist;
                for w in pts.windows(2) {
                    let l = (w[1] - w[0]).length();
                    if left <= l { return w[0].lerp(w[1], if l > 0.0 { left / l } else { 0.0 }); }
                    left -= l;
                }
                *pts.last().unwrap()
            };
            for e in &path.path_events {
                if e.event_type == 100 || e.event_type == 101 { out.push((at(e.event_start), at(e.event_end), e.event_type, e.event_value)); }
            }
        }
        out
    }
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
    /// Every race line with its distance to the finish and its events (game units).
    pub fn race_lines(&self) -> crate::course::Lines {
        let raw: Vec<_> = self.ai.race_lines.iter().map(|r| (Vec3::from(r.path_pos), r.path_points.iter().map(|p| Vec3::from(*p)).collect::<Vec<_>>(), r.distance_to_finish,
            r.path_events.iter().map(|e| crate::course::Event { kind: e.event_type, value: e.event_value, start: e.event_start, end: e.event_end }).collect::<Vec<_>>())).collect();
        crate::course::Lines::build(&raw, |v| Vec3::new(v.x, v.z, -v.y) * 0.01)
    }
    /// The opponents' AI path network.
    pub fn ai_paths(&self) -> crate::course::AiPaths {
        let raw: Vec<_> = self.ai.ai_paths.iter().map(|r| (Vec3::from(r.path_pos), r.path_points.iter().map(|p| Vec3::from(*p)).collect::<Vec<_>>(), r.u3, r.respawnable,
            r.path_events.iter().map(|e| crate::course::Event { kind: e.event_type, value: e.event_value, start: e.event_start, end: e.event_end }).collect::<Vec<_>>())).collect();
        crate::course::AiPaths::build(&raw, |v| Vec3::new(v.x, v.z, -v.y) * 0.01)
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

/// An object "dies" on contact when its collision script (or the trigger it fires) contains effect type 0 / sub-type
/// 5 ("dead node"). Type 7 effects name other instances to set off.
fn touch_effects(instances: &[Instance], logic: &LogicFile) -> std::collections::HashMap<usize, Vec<usize>> {
    let mut touch = std::collections::HashMap::new();
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
    touch
}

/// A `.pbd`'s models and materials in the folder's form (row E3a4): each model's name (the `.map`), animation time
/// (+0x14) and hierarchy nodes: parent, the high-detail mesh's contexts (each one mesh, `<dir>/Meshes/<n>.obj`
/// numbered context by context through all the models, its material the model's material block's entry for the
/// context's +4), the node's matrix (−1 none) as a position, rotation and scale, and its animation (base pose, the
/// channel mask, each channel's cubic segments: count, offset from the channel, then a, b, c, d, start, end).
fn model_set(pbd: &crate::pbd::Pbd, b: &[u8], map: &crate::map::Map, dir: PathBuf, meshes: &mut std::collections::HashMap<PathBuf, ObjMesh>) -> Result<ModelSet, String> {
    let f = |at: usize| b.get(at..at + 4).map(|w| f32::from_le_bytes([w[0], w[1], w[2], w[3]])).ok_or("past the end of the .pbd");
    let u = |at: usize| f(at).map(f32::to_bits);
    let vif = pbd.header.offset(0x84) as usize;
    let mut k = 0usize;
    let mut models = vec![];
    for (mi, m) in pbd.models.iter().enumerate() {
        let scale = [6, 7, 8].map(|i| f32::from_bits(m.head[i]));
        let block = pbd.material_blocks.get(m.head[3] as usize);
        let mut objects = vec![];
        for n in &m.nodes {
            let mut mesh_data = None;
            if n.words[1] != 0 {
                let mesh = m.meshes.iter().find(|x| x.offset == n.offset + n.words[1]).ok_or("a node's mesh is missing")?;
                let mut parts = vec![];
                for cx in &mesh.contexts {
                    let strips = crate::pbd::context_strips(b, vif + cx.words[3] as usize, scale).map_err(|e| format!("model {mi}: {e}"))?;
                    let mut o = ObjMesh::default();
                    for s in &strips {
                        for t in crate::pbd::strip_triangles(s.len()) {
                            for i in t {
                                o.positions.push(s[i].pos);
                                o.uvs.push(s[i].uv);
                                o.normals.push(s[i].nrm);
                            }
                        }
                    }
                    let path = format!("{k}.obj");
                    meshes.insert(dir.join("Meshes").join(&path), o);
                    parts.push(MeshRef { mesh_path: path, material_id: block.and_then(|bl| bl.get(cx.words[1] as usize)).map_or(-1, |x| *x as i32) });
                    k += 1;
                }
                mesh_data = Some(parts);
            }
            let (mut position, mut rotation, mut scale) = (None, None, None);
            if n.words[5] != u32::MAX {
                let at = (n.offset + n.words[5]) as usize;
                let mut c = [0f32; 16];
                for (j, x) in c.iter_mut().enumerate() { *x = f(at + 4 * j)?; }
                let (s, r, t) = Mat4::from_cols_array(&c).to_scale_rotation_translation();
                (position, rotation, scale) = (Some(t.into()), Some(r.into()), Some(s.into()));
            }
            let animation = match &n.animation {
                None => None,
                Some(a) => {
                    let h = a.head.map(f32::from_bits);
                    let mut entries = vec![];
                    for (c, _) in &a.channels {
                        let c = *c as usize;
                        let (count, first) = (u(c)? as usize, c + u(c + 4)? as usize);
                        let segs = (0..count).map(|s| {
                            let v = |j: usize| f(first + 24 * s + 4 * j);
                            Ok(crate::anim::AnimSeg { a: v(0)?, b: v(1)?, c: v(2)?, d: v(3)?, t0: v(4)?, t1: v(5)? })
                        }).collect::<Result<Vec<_>, &str>>()?;
                        entries.push(crate::anim::AnimEntry { segs });
                    }
                    Some(ObjAnim { u1: h[0], u2: h[1], u3: h[2], u4: h[3], u5: h[4], u6: h[5], action: a.head[6], entries })
                }
            };
            objects.push(ModelObject { parent_id: n.words[0] as i32, mesh_data, position, rotation, scale, animation });
        }
        models.push(Model { model_name: map.name("MODELS", mi).unwrap_or_default().to_string(), anim_time: f32::from_bits(m.head[5]), model_objects: objects });
    }
    let materials = pbd.materials.iter().enumerate().map(|(k, w)| {
        let m = crate::pbd::MaterialRec(*w);
        let name = |t: u32| format!("{t:04}.png");
        Material {
            material_name: map.name("MATERIALS", k).unwrap_or_default().to_string(),
            texture_path: if m.texture() == 0xffff { String::new() } else { name(m.texture() as u32) },
            texture_flipbook: usize::try_from(m.flip()).ok().and_then(|x| pbd.texture_flips.get(x)).map(|v| v.iter().map(|t| name(*t)).collect()),
        }
    }).collect();
    Ok(ModelSet { dir, models, materials })
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

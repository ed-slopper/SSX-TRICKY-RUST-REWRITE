//! Rider models: the game's own character meshes and skeleton, skinned on the CPU.
//!
//! The original animation data (.afl) has not been decoded by anyone yet, so the pose is built
//! here from a handful of joint angles that follow what the rider is doing.
//!
//! STANDIN: models, skeletons and clips read from our Python exports (chars/*.json, anims/*.json), for the game's own character and animation files
//! (models and clips come from the player's disc instead when it is given: `rider_from_disc`, row E4a; `afl_from_disc`, row E4b)
//! STANDIN: the pose built from joint angles (when no clip is there), for the game's animation of the rider (G5)
//!
//! Model space (as stored): centimetres, hips at the origin, +X = the character's left,
//! -Y = the way the character faces, +Z = up.

use glam::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize)]
struct RawBone { name: String, parent: i32, pos: [f32; 3], rot: [f32; 3] }
#[derive(Deserialize)]
struct RawMesh { #[serde(default)] shadow: bool, pos: Vec<f32>, nrm: Vec<f32>, uv: Vec<f32>, weights: Vec<Vec<i32>>, mat: Vec<i32> }
#[derive(Deserialize)]
struct RawModel { bones: Vec<RawBone>, materials: Vec<String>, meshes: Vec<RawMesh> }

pub struct Bone { pub name: String, pub parent: i32, local: Mat4, world_rot: Quat, inv_bind: Mat4 }
pub struct Vert { pub pos: Vec3, pub nrm: Vec3, pub uv: Vec2, pub w: [(u16, f32); 4] }
/// All triangles that share one texture ("suit", "boot", "head", "helm", "bord").
pub struct Part { pub texture: String, pub verts: Vec<Vert> }
pub struct CharModel { pub bones: Vec<Bone>, pub parts: Vec<Part> }

/// What the rider is doing, as far as the pose cares.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// 0 = standing tall, 1 = fully tucked
    pub crouch: f32,
    /// 0 = on the snow, 1 = in the air
    pub air: f32,
    /// -1 = leaning left, +1 = leaning right
    pub lean: f32,
    /// trick pose blend (0..1) and which one
    pub trick: f32,
    pub trick_id: u8,
}

impl CharModel {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw: RawModel = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self::from_raw(&raw))
    }

    /// A rider or board built from the game's own model parts (row E4a): the parts `BdrLoad_BuildSkin`
    /// 0x201138 puts together (body first). The skeleton is the body's bones followed by every other
    /// part's own (goggles, eyes, hair); weights and parents name a bone as `slot << 8 | index in its part`,
    /// the slot being the part's (+0x4a; the body's is 0).
    pub fn from_mpf(parts: &[&crate::mpf::MpfPart]) -> Self {
        let mut raw = RawModel { bones: Vec::new(), materials: Vec::new(), meshes: Vec::new() };
        let mut bone_of: HashMap<u16, usize> = HashMap::new();
        for p in parts {
            for (i, b) in p.bones.iter().enumerate() {
                let key = (p.slot as u8 as u16) << 8 | i as u16;
                if bone_of.contains_key(&key) { continue; }
                bone_of.insert(key, raw.bones.len());
                let parent = if b.parent < 0 { -1 } else { bone_of.get(&((b.parent_slot as u8 as u16) << 8 | b.parent as u16)).map_or(-1, |&g| g as i32) };
                raw.bones.push(RawBone { name: b.name.clone(), parent, pos: b.pos, rot: b.rot });
            }
        }
        for p in parts {
            for m in &p.meshes {
                let tex = p.textures.get(m.texture).map(|t| t.texture().to_string()).unwrap_or_default();
                // materials come in threes (plain, gloss, environment) per texture, as in the exports
                let mat = match raw.materials.iter().position(|x| *x == tex) {
                    Some(i) => i,
                    None => { raw.materials.extend([tex.clone(), format!("{tex} gloss"), format!("{tex} envr")]); raw.materials.len() - 3 }
                };
                let mut mesh = RawMesh { shadow: false, pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), weights: Vec::new(), mat: Vec::new() };
                for t in m.triangles() {
                    for v in t {
                        mesh.pos.extend(v.pos);
                        mesh.nrm.extend(v.nrm);
                        mesh.uv.extend(v.uv);
                        mesh.weights.push(p.weights[v.weights].iter().flat_map(|&(b, pct)| [bone_of.get(&b).map_or(0, |&g| g as i32), pct as i32]).collect());
                    }
                    mesh.mat.push(mat as i32);
                }
                raw.meshes.push(mesh);
            }
        }
        Self::from_raw(&raw)
    }

    /// A rider from the player's disc (row E4a): the closest level of detail of `BdrLoad_BuildSkin`
    /// 0x201138's parts, from `|data/char/<rider>_body.mpf` and `_head.mpf`. `rider` is the game's name
    /// (`crate::mpf::names::RIDERS`).
    pub fn rider_from_disc(files: &mut crate::big::Files, rider: &str) -> Result<Self, String> {
        use crate::mpf::{names, MpfFile};
        let r = names::RIDERS.iter().position(|n| n.eq_ignore_ascii_case(rider)).ok_or_else(|| format!("no rider {rider}"))?;
        let mut parts = Vec::new();
        for (kind, table) in [("body", &names::BODY[r][0][..]), ("head", &names::HEAD[r][0][..])] {
            let path = format!("|data/char/{}_{kind}.mpf", names::RIDERS[r]);
            let bytes = files.load(&path).map_err(|e| format!("{path}: {e}"))?.ok_or_else(|| format!("{path}: not on the disc"))?;
            let mpf = MpfFile::open(&bytes).map_err(|e| format!("{path}: {e}"))?;
            for n in table.iter().flatten() {
                let e = mpf.find(n).ok_or_else(|| format!("{path}: no part {n}"))?;
                parts.push(mpf.part(e).map_err(|e| format!("{path}: {e}"))?);
            }
        }
        Ok(Self::from_mpf(&parts.iter().collect::<Vec<_>>()))
    }

    /// A board shape from `|data/char/board.mpf` by its part name (`Bx`, `FrGoofy`, ...; matched as the
    /// game matches, see `MpfFile::find`), or the file's first part with `None`.
    pub fn board_from_disc(files: &mut crate::big::Files, part: Option<&str>) -> Result<Self, String> {
        let path = "|data/char/board.mpf";
        let bytes = files.load(path).map_err(|e| format!("{path}: {e}"))?.ok_or_else(|| format!("{path}: not on the disc"))?;
        let mpf = crate::mpf::MpfFile::open(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let e = match part { Some(n) => mpf.find(n), None => mpf.parts.first() }.ok_or_else(|| format!("{path}: no part {part:?}"))?;
        Ok(Self::from_mpf(&[&mpf.part(e).map_err(|e| format!("{path}: {e}"))?]))
    }

    fn from_raw(raw: &RawModel) -> Self {
        let mut bones: Vec<Bone> = Vec::new();
        let mut world: Vec<Mat4> = Vec::new();
        for b in &raw.bones {
            // the files store Euler angles for the inverse rotation
            let q = Quat::from_euler(EulerRot::ZYX, -b.rot[2], -b.rot[1], -b.rot[0]);
            let local = Mat4::from_rotation_translation(q, b.pos.into());
            let w = if b.parent >= 0 { world[b.parent as usize] * local } else { local };
            world.push(w);
            bones.push(Bone { name: b.name.clone(), parent: b.parent, local, world_rot: Quat::from_mat4(&w), inv_bind: w.inverse() });
        }
        let mut parts: Vec<Part> = Vec::new();
        for m in raw.meshes.iter().filter(|m| !m.shadow) {
            for (i, w) in m.weights.iter().enumerate() {
                // material ids come in threes (plain, gloss, environment) per texture
                let mat = m.mat.get(i / 3).copied().unwrap_or(0).max(0) as usize;
                let texture = raw.materials.get(mat / 3 * 3).or(raw.materials.get(mat)).cloned().unwrap_or_default();
                let part = match parts.iter().position(|p| p.texture == texture) {
                    Some(p) => p,
                    None => { parts.push(Part { texture, verts: Vec::new() }); parts.len() - 1 }
                };
                let mut ws = [(0u16, 0.0f32); 4];
                let mut total = 0.0;
                for (k, pair) in w.as_chunks::<2>().0.iter().take(4).enumerate() {
                    ws[k] = ((pair[0].max(0) as usize).min(bones.len().saturating_sub(1)) as u16, pair[1] as f32);
                    total += pair[1] as f32;
                }
                if total <= 0.0 { ws[0] = (0, 1.0); total = 1.0; }
                for x in ws.iter_mut() { x.1 /= total; }
                parts[part].verts.push(Vert {
                    pos: Vec3::new(m.pos[i * 3], m.pos[i * 3 + 1], m.pos[i * 3 + 2]),
                    nrm: Vec3::new(m.nrm[i * 3], m.nrm[i * 3 + 1], m.nrm[i * 3 + 2]),
                    uv: Vec2::new(m.uv[i * 2], m.uv[i * 2 + 1]),
                    w: ws,
                });
            }
        }
        Self { bones, parts }
    }

    fn bone(&self, name: &str) -> Option<usize> { self.bones.iter().position(|b| b.name == name) }

    /// Skinning matrices (model space, cm) for a pose, plus how far to lift the model so the
    /// feet sit on the board.
    pub fn skin(&self, pose: &Pose) -> (Vec<Mat4>, f32) {
        let d = |deg: f32| deg.to_radians();
        let c = pose.crouch.clamp(0.0, 1.0);
        let a = pose.air.clamp(0.0, 1.0);
        let t = pose.trick.clamp(0.0, 1.0);
        // extra rotation per joint, given around *model* axes: X left, Y back, Z up
        let mut extra: Vec<Quat> = vec![Quat::IDENTITY; self.bones.len()];
        let mut set = |name: &str, q: Quat| if let Some(i) = self.bone(name) { extra[i] = q; };
        let (rx, ry, rz) = (Quat::from_rotation_x as fn(f32) -> Quat, Quat::from_rotation_y as fn(f32) -> Quat, Quat::from_rotation_z as fn(f32) -> Quat);
        let knee = 22.0 + 42.0 * c + 25.0 * t;
        let spread = 23.0;
        set("l_thigh", ry(d(-spread)) * rx(d(-knee)));
        set("r_thigh", ry(d(spread)) * rx(d(-knee)));
        set("l_calf", rx(d(knee * 1.9)));
        set("r_calf", rx(d(knee * 1.9)));
        set("l_foot", ry(d(spread)) * rx(d(-knee * 0.9)));
        set("r_foot", ry(d(-spread)) * rx(d(-knee * 0.9)));
        set("lower_back", rz(d(18.0)) * rx(d(8.0 + 24.0 * c)));
        set("upper_back", rz(d(16.0)) * rx(d(4.0 + 10.0 * c)));
        set("neck", rz(d(20.0)));
        set("head", rz(d(28.0)) * rx(d(-(8.0 + 22.0 * c))));
        // arms: down from the T-pose, out a little for balance, up when airborne
        let (mut l_arm, mut r_arm) = (34.0 - 28.0 * a + 8.0 * c, 40.0 - 32.0 * a + 8.0 * c);
        let mut reach = 0.0;
        match pose.trick_id {
            1 => { l_arm = l_arm + (95.0 - l_arm) * t; reach = 35.0 * t; }               // front hand grabs the board
            2 => { r_arm = r_arm + (95.0 - r_arm) * t; reach = 35.0 * t; }               // back hand grabs
            3 => { l_arm = l_arm + (-60.0 - l_arm) * t; r_arm = r_arm + (-60.0 - r_arm) * t; } // both arms up
            _ => {}
        }
        set("l_bicep", ry(d(l_arm)) * rz(d(-6.0)));
        set("r_bicep", ry(d(-r_arm)) * rz(d(6.0)));
        set("l_forearm", rz(d(-(16.0 + reach))));
        set("r_forearm", rz(d(16.0 + reach)));

        let mut posed: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            // turn the model-axis rotation into the joint's own frame
            let local = b.local * Mat4::from_quat(b.world_rot.inverse() * extra[i] * b.world_rot);
            posed.push(if b.parent >= 0 { posed[b.parent as usize] * local } else { local });
        }
        let foot = |n: &str| self.bone(n).map(|i| posed[i].w_axis.z).unwrap_or(-90.0);
        let lift = -foot("l_foot").min(foot("r_foot")) + 11.0;
        (posed.iter().zip(&self.bones).map(|(p, b)| *p * b.inv_bind).collect(), lift)
    }
}

/// Model space (cm; X = rider's left = direction of travel, -Y = facing, Z up) to the rider's
/// local Bevy frame (metres; -Z = direction of travel, Y up, X right).
pub fn model_to_local(v: Vec3) -> Vec3 { Vec3::new(-v.y, v.z, -v.x) * 0.01 }
pub fn model_dir_to_local(v: Vec3) -> Vec3 { Vec3::new(-v.y, v.z, -v.x) }

/// Skin one part. Returns positions and normals in the rider's local Bevy frame.
pub fn skin_part(part: &Part, mats: &[Mat4], lift: f32, pos: &mut Vec<[f32; 3]>, nrm: &mut Vec<[f32; 3]>) {
    pos.clear();
    nrm.clear();
    for v in &part.verts {
        let (mut p, mut n) = (Vec3::ZERO, Vec3::ZERO);
        for (bone, w) in v.w {
            if w <= 0.0 { continue; }
            let m = &mats[bone as usize];
            p += m.transform_point3(v.pos) * w;
            n += Mat3::from_mat4(*m) * v.nrm * w;
        }
        p.z += lift;
        pos.push(model_to_local(p).into());
        nrm.push(model_dir_to_local(n.normalize_or(Vec3::Z)).into());
    }
}

// ---------------------------------------------------------------- the game's own animations
//
// Decoded from the .afl files (see notes/animation-format.md). Animation space: centimetres,
// board at the origin lying along Y, +Y = direction of travel, +X = the rider's toe side, +Z up.

pub const ANIM_BONES: usize = 19;
pub const ANIM_FPS: f32 = 30.0;

#[derive(Clone, Copy)]
pub struct Sample { pub hips: Vec3, pub rot: [Quat; ANIM_BONES], pub board_pos: Vec3, pub board_rot: Quat }

impl Sample {
    pub fn blend(&self, to: &Sample, t: f32) -> Sample {
        let t = t.clamp(0.0, 1.0);
        let mut rot = self.rot;
        for (r, o) in rot.iter_mut().zip(&to.rot) { *r = r.slerp(*o, t); }
        Sample { hips: self.hips.lerp(to.hips, t), rot, board_pos: self.board_pos.lerp(to.board_pos, t), board_rot: self.board_rot.slerp(to.board_rot, t) }
    }
}

pub struct Clip { pub name: String, frames: Vec<Sample> }
impl Clip {
    pub fn len(&self) -> f32 { self.frames.len() as f32 }
    /// Pose at a (fractional) frame; `looped` wraps around, otherwise it holds the last frame.
    pub fn at(&self, frame: f32, looped: bool) -> Sample {
        let n = self.frames.len();
        if n == 1 { return self.frames[0]; }
        let f = if looped { frame.rem_euclid(n as f32) } else { frame.clamp(0.0, (n - 1) as f32) };
        let i = f.floor() as usize;
        let j = if looped { (i + 1) % n } else { (i + 1).min(n - 1) };
        self.frames[i.min(n - 1)].blend(&self.frames[j], f - i as f32)
    }
}

#[derive(Deserialize)]
struct RawClip { name: String, frames: usize, data: Vec<f32> }
#[derive(Deserialize)]
struct RawClips { clips: Vec<RawClip> }

fn euler(r: &[f32]) -> Quat { Quat::from_euler(EulerRot::ZYX, -r[2], -r[1], -r[0]) }

/// The riders' three letters in the uber files' names (`bxEddUber` ... at 0x378d60), by our folder names.
pub const UBER_RIDERS: [(&str, &str); 12] = [
    ("eddie", "Edd"), ("elise", "Eli"), ("mac", "Mac"), ("jp", "JP"), ("zoe", "Zoe"), ("moby", "Mob"),
    ("luther", "Lut"), ("kaori", "Kao"), ("psymon", "Psy"), ("seeiah", "See"), ("brodi", "Bro"), ("marisol", "Mar"),
];

#[derive(Default)]
pub struct AnimSet { clips: Vec<Clip>, index: HashMap<String, usize> }
impl AnimSet {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw: RawClips = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self::from_clips(raw.clips))
    }

    /// The riding clips of an `.afl` file (row E4b): every clip with a body group (type 0: hips position,
    /// 19 bone rotations) and a board group (type 1: position, rotation), sampled at each whole frame with
    /// the game's curve evaluator, named from the exe's clip names (`anim_<n>` for the few it does not name).
    pub fn from_afl(bytes: &[u8]) -> Result<Self, String> {
        let afl = crate::afl::AflFile::open(bytes)?;
        let mut clips = Vec::new();
        for (i, c) in afl.clips.iter().enumerate() {
            let (Some(body), Some(board)) = (c.group(0), c.group(1)) else { continue };
            let frames = c.frames() as usize;
            if frames == 0 { continue; }
            let mut data = Vec::with_capacity(frames * 66);
            for f in 0..frames {
                let t = f as f32;
                data.extend((0..60).map(|k| afl.eval(body.first_curve, k, t)));
                data.extend((0..6).map(|k| afl.eval(board.first_curve, k, t)));
            }
            clips.push(RawClip { name: c.name().map_or_else(|| format!("anim_{i}"), str::to_string), frames, data });
        }
        Ok(Self::from_clips(clips))
    }

    /// A clip file from the player's disc: `|data/char/<file>.afl` (`anim_names::FILES`; opened as the
    /// game opens it, 0x379000).
    pub fn afl_from_disc(files: &mut crate::big::Files, file: &str) -> Result<Self, String> {
        let path = format!("|data/char/{file}.afl");
        let bytes = files.load(&path).map_err(|e| format!("{path}: {e}"))?.ok_or_else(|| format!("{path}: not on the disc"))?;
        Self::from_afl(&bytes).map_err(|e| format!("{path}: {e}"))
    }

    /// A rider's uber tricks for a board kind (`bx`, `fr`, `ex`): the file `<kind><Rid>Uber`, the rider's
    /// three letters as the exe's file names have them.
    pub fn uber_from_disc(files: &mut crate::big::Files, kind: &str, rider: &str) -> Result<Self, String> {
        let short = UBER_RIDERS.iter().find(|(n, _)| n.eq_ignore_ascii_case(rider)).ok_or_else(|| format!("no uber file for {rider}"))?.1;
        Self::afl_from_disc(files, &format!("{kind}{short}Uber"))
    }

    /// Every clip of the files that are neither a board kind's set (`bxanim`, `franim`, `exanim`,
    /// `cmanim`) nor a rider's uber tricks: the scenes (gates, pre-race, finish, cut-scenes), merged in the
    /// game's file order.
    pub fn scenes_from_disc(files: &mut crate::big::Files) -> Result<Self, String> {
        let mut set = AnimSet::default();
        for f in crate::anim_names::FILES {
            if f.ends_with("anim") || f.ends_with("Uber") { continue; }
            set.merge(Self::afl_from_disc(files, f)?);
        }
        Ok(set)
    }

    fn from_clips(raw: Vec<RawClip>) -> Self {
        let mut set = AnimSet::default();
        for c in raw {
            if c.frames == 0 || c.data.len() < c.frames * 66 { continue; }
            let mut frames: Vec<Sample> = (0..c.frames).map(|f| {
                let d = &c.data[f * 66..f * 66 + 66];
                let mut rot = [Quat::IDENTITY; ANIM_BONES];
                for (b, r) in rot.iter_mut().enumerate() { *r = euler(&d[3 + b * 3..6 + b * 3]); }
                Sample { hips: Vec3::new(d[0], d[1], d[2]), rot, board_pos: Vec3::new(d[60], d[61], d[62]), board_rot: euler(&d[63..66]) }
            }).collect();
            // Air and trick clips are authored hovering above the origin; bring them down so that
            // every clip starts with the board where the riding clips keep it. The gate start is the
            // other way round: it ends where riding begins, so it is lined up on its last frame.
            let anchor = if c.name.ends_with("G_GATESTART") { frames[frames.len() - 1].board_pos } else { frames[0].board_pos };
            let off = anchor - Vec3::new(0.1, 0.0, -3.1);
            if off.length() > 10.0 { for s in frames.iter_mut() { s.hips -= off; s.board_pos -= off; } }
            set.index.insert(c.name.clone(), set.clips.len());
            set.clips.push(Clip { name: c.name, frames });
        }
        set
    }
    /// Add another file's clips (later ones win on a name clash).
    pub fn merge(&mut self, other: AnimSet) {
        for c in other.clips { self.index.insert(c.name.clone(), self.clips.len()); self.clips.push(c); }
    }
    pub fn len(&self) -> usize { self.clips.len() }
    pub fn get(&self, name: &str) -> Option<&Clip> { self.index.get(name).map(|i| &self.clips[*i]) }
    pub fn nth(&self, i: usize) -> Option<&Clip> { self.clips.get(i) }
    pub fn list(&self) -> Vec<(String, f32)> { self.clips.iter().map(|c| (c.name.clone(), c.len())).collect() }
}

/// Animation space to the rider's local Bevy frame (same axes as the level: Y forward, Z up).
pub fn anim_to_local(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) * 0.01 }
pub fn anim_dir_to_local(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) }

impl CharModel {
    /// Skinning matrices for a sampled animation pose (animation space, cm).
    pub fn skin_sample(&self, s: &Sample) -> Vec<Mat4> {
        let mut posed: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let local = if i == 0 { Mat4::from_rotation_translation(s.rot[0], s.hips) }
                else if i < ANIM_BONES { Mat4::from_rotation_translation(s.rot[i], b.local.w_axis.truncate()) }
                else { b.local };
            posed.push(if b.parent >= 0 { posed[b.parent as usize] * local } else { local });
        }
        posed.iter().zip(&self.bones).map(|(p, b)| *p * b.inv_bind).collect()
    }
    /// The board follows its own animated node.
    pub fn skin_board(&self, s: &Sample) -> Vec<Mat4> {
        let m = Mat4::from_rotation_translation(s.board_rot, s.board_pos);
        self.bones.iter().map(|b| m * b.inv_bind).collect()
    }
}

/// Skin one part that is posed in animation space.
pub fn skin_part_anim(part: &Part, mats: &[Mat4], pos: &mut Vec<[f32; 3]>, nrm: &mut Vec<[f32; 3]>) {
    pos.clear();
    nrm.clear();
    for v in &part.verts {
        let (mut p, mut n) = (Vec3::ZERO, Vec3::ZERO);
        for (bone, w) in v.w {
            if w <= 0.0 { continue; }
            let m = &mats[(bone as usize).min(mats.len() - 1)];
            p += m.transform_point3(v.pos) * w;
            n += Mat3::from_mat4(*m) * v.nrm * w;
        }
        // the animations keep the board's underside a little below their origin
        p.z += 4.8;
        pos.push(anim_to_local(p).into());
        nrm.push(anim_dir_to_local(n.normalize_or(Vec3::Z)).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpf::{names, MpfFile};

    /// The game's own models (row E4a) against the models exported before: needs `TRICKY_ISO` and the
    /// exported `chars/` folder (both the player's own, outside the repo).
    #[test]
    fn disc_models_match_the_exports() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let chars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chars");
        if !chars.join("board.json").exists() { return; }
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let compare = |name: &str, ours: &CharModel, theirs: &CharModel| {
            assert_eq!(ours.bones.len(), theirs.bones.len(), "{name}: bones");
            for b in &theirs.bones {
                let o = ours.bones.iter().find(|x| x.name == b.name).unwrap_or_else(|| panic!("{name}: bone {}", b.name));
                let parent = |m: &CharModel, p: i32| if p < 0 { String::new() } else { m.bones[p as usize].name.clone() };
                assert_eq!(parent(ours, o.parent), parent(theirs, b.parent), "{name}: parent of {}", b.name);
                assert!(o.local.abs_diff_eq(b.local, 1e-3), "{name}: bone {}", b.name);
            }
            for t in &theirs.parts {
                let o = ours.parts.iter().find(|p| p.texture == t.texture).unwrap_or_else(|| panic!("{name}: part {}", t.texture));
                // position (0.01 cm), uv (1/1000), normal (1/100), weights by bone name (whole percent)
                let key = |m: &CharModel, v: &Vert| {
                    let mut k: Vec<i64> = (v.pos * 100.0).round().to_array().iter().chain((v.uv * 1000.0).round().to_array().iter()).chain((v.nrm * 100.0).round().to_array().iter()).map(|&x| x as i64).collect();
                    let mut w: Vec<(String, i64)> = v.w.iter().filter(|x| x.1 > 0.0).map(|&(b, f)| (m.bones[b as usize].name.clone(), (f * 100.0).round() as i64)).collect();
                    w.sort();
                    k.extend(w.iter().flat_map(|(n, f)| [n.bytes().fold(0i64, |h, c| h * 31 + c as i64), *f]));
                    k
                };
                let mut a: Vec<_> = o.verts.iter().map(|v| key(ours, v)).collect();
                let mut b: Vec<_> = t.verts.iter().map(|v| key(theirs, v)).collect();
                a.sort();
                b.sort();
                assert_eq!(a.len(), b.len(), "{name}: {} vertices", t.texture);
                let off = a.iter().zip(&b).filter(|(x, y)| x.len() != y.len() || x.iter().zip(y.iter()).any(|(p, q)| (p - q).abs() > 1)).count();
                assert_eq!(off, 0, "{name}: {} vertices differ", t.texture);
            }
        };
        let mut checked = 0;
        for (r, rider) in names::RIDERS.iter().enumerate() {
            let json = chars.join(rider).join("model.json");
            if !json.exists() { continue; }
            let body = files.load(&format!("|data/char/{rider}_body.mpf")).unwrap().unwrap();
            let head = files.load(&format!("|data/char/{rider}_head.mpf")).unwrap().unwrap();
            let (body, head) = (MpfFile::open(&body).unwrap(), MpfFile::open(&head).unwrap());
            let mut parts = Vec::new();
            for n in names::BODY[r][0].iter().flatten() { parts.push(body.part(body.find(n).unwrap()).unwrap()); }
            for n in names::HEAD[r][0].iter().flatten() { parts.push(head.part(head.find(n).unwrap()).unwrap()); }
            let ours = CharModel::from_mpf(&parts.iter().collect::<Vec<_>>());
            compare(rider, &ours, &CharModel::load(&json).unwrap());
            checked += 1;
        }
        assert!(checked > 0);
        let board = files.load("|data/char/board.mpf").unwrap().unwrap();
        let board = MpfFile::open(&board).unwrap();
        for (file, part) in [("board_bx", "Bx"), ("board_fr", "Fr"), ("board_al", "Al"), ("board_bx_goofy", "BxGoofy"), ("board_fr_goofy", "FrGoofy"), ("board_al_goofy", "AlGoofy")] {
            let p = board.part(board.find(part).unwrap()).unwrap();
            compare(file, &CharModel::from_mpf(&[&p]), &CharModel::load(&chars.join(format!("{file}.json"))).unwrap());
        }
    }
}

#[cfg(test)]
mod anim_tests {
    use super::*;

    /// The scenes and every rider's uber tricks: the same clips (names and lengths) as the exports.
    #[test]
    fn disc_scenes_and_ubers() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let chars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chars");
        if !chars.join("anims/scenes.json").exists() { return; }
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let same = |what: &str, ours: &AnimSet, theirs: &AnimSet| {
            for c in &theirs.clips {
                let o = ours.get(&c.name).unwrap_or_else(|| panic!("{what}: no clip {}", c.name));
                assert_eq!(o.frames.len(), c.frames.len(), "{what} {}", c.name);
            }
        };
        let scenes = AnimSet::scenes_from_disc(&mut files).unwrap();
        for f in ["scenes", "finish"] { same(f, &scenes, &AnimSet::load(&chars.join(format!("anims/{f}.json"))).unwrap()); }
        for (rider, _) in UBER_RIDERS {
            for (kind, json) in [("bx", "uber"), ("fr", "uber_fr"), ("ex", "uber_ex")] {
                let p = chars.join(rider).join(format!("{json}.json"));
                if !p.exists() { continue; }
                same(&format!("{rider} {kind}"), &AnimSet::uber_from_disc(&mut files, kind, rider).unwrap(), &AnimSet::load(&p).unwrap());
            }
        }
        println!("scenes: {} clips", scenes.len());
    }

    /// The game's clips (row E4b) against the clips exported before: needs `TRICKY_ISO` and `chars/anims`.
    #[test]
    fn disc_clips_match_the_exports() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let anims = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chars/anims");
        if !anims.join("bx.json").exists() { return; }
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        for (json, afl) in [("bx", "bxanim"), ("fr", "franim"), ("ex", "exanim"), ("cm", "cmanim")] {
            let ours = AnimSet::from_afl(&files.load(&format!("|data/char/{afl}.afl")).unwrap().unwrap()).unwrap();
            let theirs = AnimSet::load(&anims.join(format!("{json}.json"))).unwrap();
            let (mut clips, mut frames, mut off, mut worst) = (0, 0, 0, (0.0f32, String::new()));
            for c in &theirs.clips {
                let o = ours.get(&c.name).unwrap_or_else(|| panic!("{json}: no clip {}", c.name));
                assert_eq!(o.frames.len(), c.frames.len(), "{json} {}: frames", c.name);
                clips += 1;
                for (a, b) in o.frames.iter().zip(&c.frames) {
                    frames += 1;
                    let mut d = a.hips.distance(b.hips).max(a.board_pos.distance(b.board_pos));
                    for (p, q) in a.rot.iter().zip(&b.rot).chain([(&a.board_rot, &b.board_rot)]) {
                        // about the angle in degrees, without acos' f32 noise near 1
                        let e = (*p - *q).length().min((*p + *q).length());
                        d = d.max(e * 114.59);
                    }
                    if d > 0.05 { off += 1; }
                    if d > worst.0 { worst = (d, c.name.clone()); }
                }
            }
            // which of the two moves more smoothly frame to frame (a wrong curve reading jumps)
            let jerk = |set: &AnimSet| -> f32 {
                let mut tot = 0.0;
                for c in &theirs.clips {
                    let Some(c) = set.get(&c.name) else { continue };
                    for w in c.frames.windows(2) {
                        let mut d: f32 = 0.0;
                        for (p, q) in w[0].rot.iter().zip(&w[1].rot) { d = d.max((*p - *q).length().min((*p + *q).length())); }
                        tot += d;
                    }
                }
                tot
            };
            println!("{json}: frame-to-frame motion, ours {:.1}, exported {:.1}", jerk(&ours), jerk(&theirs));
            // the exports read a piece of a multi-piece curve as `count` frames long instead of the game's
            // `count - 1` (`AnimCurve_Eval` 0x157ed8), so they jump at piece joins and move more
            assert!(jerk(&ours) < jerk(&theirs), "{json}: ours moves more than the exports");
            let _ = &mut files;
            println!("{json}: {clips} clips ({} ours), {frames} frames, {off} differ by more than 0.05 (cm or degrees), worst {:.3} in {}", ours.len(), worst.0, worst.1);
        }
    }
}

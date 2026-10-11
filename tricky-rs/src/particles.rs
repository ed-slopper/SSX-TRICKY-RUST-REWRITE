//! The level's particle emitters (op 2 sub-type 0, `Emitter_InitFromRecord`, `PBurst_*`, drawn by
//! VU1 microcode): torches, snow plumes, smoke, the sparkle bursts when a gem or a sign is hit.
//! Each particle is worked out from its birth time alone, as the game does:
//!   P = M (origin + u1 A + u2 B),  V = M (vel + u3 S1 + u4 S2 + u5 S3),  W = accel / k
//!   pos(t) = P + W t + (V - W) / k * f(min(k t, 2.7)),  f(x) = 0.73 x - 0.113 x^2
//! (STANDIN: f(x), for the game's drag towards the terminal velocity W). Colour runs from c0 to c1 over the
//! longest life, plus random amounts of c2 and c3; trails repeat the particle a little earlier.

use crate::logic::EmitterDef;
use crate::{ui, LevelRes, Opponents, RiderRes, SmoothDt, World};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

/// One emitter running: its settings, the owning instance's matrix (game space), time since it
/// started, a seed, and whether it ends (a one-shot burst).
struct Node { def: EmitterDef, m: Mat4, t: f32, seed: u32, one_shot: bool }

#[derive(Resource, Default)]
pub struct Particles { nodes: Vec<Node>,
    /// trigger objects touched (for their script sounds; drained by `worldsound`)
    pub fired: Vec<usize>, key: Option<std::path::PathBuf>, cool: std::collections::HashMap<usize, f32>, seed: u32,
    /// one mesh per (texture, blend): the game's particle sprites when found, else a soft dot
    buckets: std::collections::HashMap<(u32, u32), Handle<Mesh>>, textures: Vec<Option<Handle<Image>>>, dot: Handle<Image>, loaded: bool }

/// The particle sheet's names in the game's order (table 0x33edf0), as `chars/particles/<name>.png`.
const TEX_NAMES: [&str; 38] = ["part", "snfl", "clod", "spry", "halo", "brk1", "brk2", "brk3", "ndl1", "ndl2", "swd1", "swd2", "swp1", "swp2", "cnf1", "cnf2",
    "blb1", "blb2", "str1", "str2", "str3", "nois", "strk", "tral", "ex06", "ex07", "ex08", "ex09", "lens", "blnk", "mip1", "mip1", "mip2", "beam", "fog0", "spec", "envr", "exlm"];

#[derive(Component)]
pub struct ParticleMesh;

fn empty() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m.insert_indices(Indices::U32(vec![0, 1, 2]));
    m
}

pub fn setup_particles(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    // a soft round sprite (the game's particle.ssh sheet is not extracted)
    let n = 32u32;
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let a = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0).powf(1.5);
            px.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    let img = images.add(Image::new(bevy::render::render_resource::Extent3d { width: n, height: n, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2, px, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()));
    let _ = (&mut meshes, &mut materials);
    commands.insert_resource(Particles { dot: img, seed: 12345, ..default() });
}

fn hash(mut x: u32) -> u32 { x ^= x >> 16; x = x.wrapping_mul(0x7feb_352d); x ^= x >> 15; x = x.wrapping_mul(0x846c_a68b); x ^ (x >> 16) }
/// uniform in [-0.5, 0.5) for particle `i`, life `cycle`, channel `c`
fn u(seed: u32, i: u32, cycle: u32, c: u32) -> f32 { (hash(seed ^ hash(i.wrapping_mul(97) ^ hash(cycle.wrapping_mul(131) ^ c))) >> 8) as f32 / 16_777_216.0 - 0.5 }
fn v3(a: [f32; 3]) -> Vec3 { Vec3::from(a) }
fn g2b(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) * 0.01 }

fn inst_matrix(inst: &crate::level::Instance) -> Mat4 {
    Mat4::from_scale_rotation_translation(inst.scale.into(), crate::level::quat4(inst.rotation), inst.location.into())
}

#[allow(clippy::too_many_arguments)]
pub fn particles(
    time: Res<SmoothDt>, game: Res<ui::Game>, level: Res<LevelRes>, world: Res<World>, rider: Res<RiderRes>, opponents: Res<Opponents>,
    mut ps: ResMut<Particles>, mut meshes: ResMut<Assets<Mesh>>, mut commands: Commands, lib: Res<crate::CharLib>,
    mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>,
    cam: Query<&GlobalTransform, With<Camera3d>>, mut holders: Query<&mut Transform, With<ParticleMesh>>,
) {
    let level = &level.0;
    // the game's particle sprites (PARTICLE.SSH, made into PNGs)
    if !ps.loaded {
        ps.loaded = true;
        let dir = lib.dir.join("particles");
        ps.textures = TEX_NAMES.iter().map(|n| {
            let img = image::open(dir.join(format!("{n}.png"))).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            Some(images.add(Image::new(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2, img.into_raw(), bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())))
        }).collect();
        println!("particle sprites: {} of {}", ps.textures.iter().flatten().count(), TEX_NAMES.len());
    }
    // a new level: start its always-on emitters
    if ps.key.as_ref() != Some(&level.dir) {
        ps.key = Some(level.dir.clone());
        ps.cool.clear();
        let seed = ps.seed;
        ps.nodes = level.logic.emit_wake.iter().enumerate().filter_map(|(k, (i, def))| {
            let inst = level.instances.get(*i)?;
            Some(Node { def: *def, m: inst_matrix(inst), t: 0.0, seed: hash(seed ^ k as u32), one_shot: def.dur >= 0.0 })
        }).collect();
    }
    let dt = if game.screen == ui::Screen::Paused { 0.0 } else { time.dt.min(0.1) };
    // touching an object sets off its emitters (once a second at most)
    for v in ps.cool.values_mut() { *v -= dt; }
    let bodies: Vec<Vec3> = std::iter::once(rider.0.pos).chain(opponents.0.iter().map(|o| o.rider.pos)).map(|p| p + Vec3::Y * 0.8).collect();
    let mut fire: Vec<usize> = Vec::new();
    for (&inst, &(lo, hi)) in &world.0.emit_boxes {
        if ps.cool.get(&inst).is_some_and(|c| *c > 0.0) { continue; }
        if bodies.iter().any(|b| b.cmpge(lo - 0.3).all() && b.cmple(hi + 0.3).all()) { fire.push(inst); }
    }
    for inst in fire {
        ps.cool.insert(inst, 1.0);
        ps.fired.push(inst);
        if ps.fired.len() > 64 { ps.fired.remove(0); }
        let Some(list) = level.logic.emit_touch.get(&inst) else { continue };
        for (owner, def) in list.clone() {
            let Some(oi) = level.instances.get(owner) else { continue };
            ps.seed = hash(ps.seed.wrapping_add(1));
            let seed = ps.seed;
            ps.nodes.push(Node { def, m: inst_matrix(oi), t: 0.0, seed, one_shot: true });
        }
    }
    for n in ps.nodes.iter_mut() { n.t += dt; }
    ps.nodes.retain(|n| !n.one_shot || n.t < n.def.dur.max(0.0) + n.def.life + n.def.life_sp * 0.5 + 0.1);

    let Ok(cam) = cam.single() else { return };
    let (origin, right, up) = (cam.translation(), cam.right().as_vec3(), cam.up().as_vec3());
    for mut tf in holders.iter_mut() { tf.translation = origin; }
    type Buf = (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<[f32; 4]>, Vec<u32>);
    let mut out: std::collections::HashMap<(u32, u32), Buf> = std::collections::HashMap::new();
    for n in &ps.nodes {
        let d = &n.def;
        let at = g2b(n.m.w_axis.truncate());
        if at.distance(origin) > 250.0 || d.n == 0 { continue; }
        let tmax = (d.life + d.life_sp * 0.5).max(1e-3);
        let k = d.k;
        let span = if d.dur >= 0.0 { d.dur } else { tmax };
        let w = if k.abs() > 1e-4 { v3(d.accel) / k } else { Vec3::ZERO };
        let blend = match d.blend { 0 => 0, 4 => 4, _ => 1 };
        let tex = if ps.textures.get(d.texture as usize).is_some_and(|t| t.is_some()) { d.texture } else { u32::MAX };
        let buf = out.entry((tex, blend)).or_default();
        let copies = d.trail.clamp(1, 4);
        for i in 0..d.n {
            let born = i as f32 * span / d.n as f32;
            // a continuous emitter starts full and recycles each particle every `tmax`
            let (age, cycle) = if d.dur >= 0.0 { (n.t - born, 0) } else { let a = n.t + tmax - born; (a.rem_euclid(tmax), (a / tmax).floor() as u32) };
            let r = |c: u32| u(n.seed, i, cycle, c);
            let life = d.life + d.life_sp * r(10);
            if age < 0.0 || age >= life { continue; }
            let p0 = n.m.transform_point3(v3(d.origin) + v3(d.area_a) * r(1) + v3(d.area_b) * r(2));
            let v0 = n.m.transform_vector3(v3(d.vel) + v3(d.vs[0]) * r(3) + v3(d.vs[1]) * r(4) + v3(d.vs[2]) * r(5));
            let size = (d.size + d.size_sp * r(9)).max(1.0) * 0.01 * 0.5;
            for j in 0..copies {
                let a = age - j as f32 * d.trail_dt;
                if a < 0.0 { break; }
                let pos = if k.abs() > 1e-4 {
                    let x = (k * a).min(2.7);
                    p0 + w * a + (v0 - w) / k * (0.73 * x - 0.113 * x * x)
                } else { p0 + v0 * a + v3(d.accel) * 0.5 * a * a };
                let mut c = [0.0f32; 4];
                for (ch, v) in c.iter_mut().enumerate() { *v = ((d.c[0][ch] + (d.c[1][ch] - d.c[0][ch]) * a / tmax + r(6) * d.c[2][ch] + r(7) * d.c[3][ch]) * 128.0).clamp(0.0, 255.0) / 128.0; }
                let fade = 1.0 - j as f32 / copies as f32;
                let col = if d.blend == 4 { let k = 1.0 - (c[3] * fade).min(1.0); [k, k, k, 1.0] } else { [c[0].min(2.0), c[1].min(2.0), c[2].min(2.0), (c[3] * fade).min(1.0)] };
                let ctr = g2b(pos) - origin;
                let base = buf.0.len() as u32;
                for (dx, dy, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                    buf.0.push((ctr + (right * dx + up * dy) * size).into());
                    buf.1.push(uv);
                    buf.2.push(col);
                }
                buf.3.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    if std::env::var("TRICKY_PARTDBG").is_ok() { eprintln!("particles: nodes {} buckets {} quads {}", ps.nodes.len(), out.len(), out.values().map(|b| b.3.len() / 6).sum::<usize>()); }
    // a mesh for every texture and blend in use (made the first time it is needed)
    for key in out.keys().copied().collect::<Vec<_>>() {
        if ps.buckets.contains_key(&key) { continue; }
        let tex = ps.textures.get(key.0 as usize).cloned().flatten().unwrap_or(ps.dot.clone());
        let mode = match key.1 { 0 => AlphaMode::Add, 4 => AlphaMode::Multiply, _ => AlphaMode::Blend };
        let h = meshes.add(empty());
        commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(materials.add(StandardMaterial { base_color_texture: Some(tex), unlit: true, alpha_mode: mode, double_sided: true, cull_mode: None, ..default() })),
            Transform::from_translation(origin), bevy::render::view::NoFrustumCulling, ParticleMesh));
        ps.buckets.insert(key, h);
    }
    let buckets: Vec<((u32, u32), Handle<Mesh>)> = ps.buckets.iter().map(|(k, h)| (*k, h.clone())).collect();
    for (key, h) in buckets {
        let (pos, uv, col, idx) = out.remove(&key).unwrap_or_default();
        let Some(m) = meshes.get_mut(&h) else { continue };
        if idx.is_empty() { *m = empty(); continue; }
        let n = pos.len();
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        m.insert_indices(Indices::U32(idx));
    }
}

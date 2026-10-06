//! tricky-rs, stage 2: load an extracted SSX Tricky level project and ride it.
//!
//!   tricky-rs [path/to/level/project]        (default: levels/gari)
//!
//! Ride:  A/D or arrows steer, W/Up tuck (and push off), S/Down brake, hold Space to crouch and
//!        release to jump, steer in the air to spin. Backspace: back to last safe spot. Enter: restart.
//!        Gamepad: left stick, A/Cross jump, right trigger tuck, X/Square brake.
//! Tab switches to the free camera: click to look, WASD, Space/Ctrl up/down, Shift fast, +/- speed.
//! R rails overlay, F1 help.

mod character;
mod collide;
mod level;
mod props;
mod ui;
mod rails;
mod rider;

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use character::{AnimSet, CharModel, Pose, Sample};
use collide::CollisionWorld;
use level::*;
use props::{Kind, Pickup, Prop};
use rails::{Rail, Rails};
use rider::{heading, AiDriver, ChaseCam, Input, Race, RaceState, Rider};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Game units per metre-ish. The world root scales everything down by this.
const WORLD_SCALE: f32 = 0.01;
/// Subdivisions per patch edge.
const PATCH_SEGS: usize = 8;
/// How strongly the terrain lightmap darkens (1.0 = as stored in the extracted PNGs).
const LIGHTMAP_STRENGTH: f32 = 1.0;

#[derive(Resource)]
struct LevelRes(Level);
#[derive(Resource, Default)]
struct Overlay { rails: bool, help: bool }
#[derive(Component)]
struct FlyCam { yaw: f32, pitch: f32, speed: f32 }
#[derive(Component)]
struct SkyRoot;
#[derive(Component)]
struct HelpText;
#[derive(Resource)]
struct Shot { path: String, frame: u32 }
#[derive(Resource)]
struct World(CollisionWorld);
#[derive(Resource)]
struct RiderRes(Rider);
#[derive(Resource, Default)]
struct CamRes(ChaseCam);
#[derive(Resource)]
struct RailsRes(Rails);
#[derive(Resource)]
struct RaceRes(Race);
/// Every level project found next to the one we were started with.
#[derive(Resource)]
struct LevelList { dirs: Vec<PathBuf>, current: usize }
/// Everything spawned for the current level, so it can be swapped out.
#[derive(Component)]
struct LevelEntity;

/// Knockable objects of the current level, and the entity drawing each.
#[derive(Resource, Default)]
struct Props(Vec<Prop>);
#[derive(Component)]
struct PropVisual(usize);

/// Computer-controlled opponents.
struct Opponent { rider: Rider, driver: AiDriver, character: usize }
#[derive(Resource, Default)]
struct Opponents(Vec<Opponent>);

fn make_opponents(start: Vec3, yaw: f32, player_char: usize, n_chars: usize, count: usize, round: usize) -> Opponents {
    if n_chars == 0 { return Opponents::default(); }
    let side = Vec3::new(-yaw.cos(), 0.0, yaw.sin());
    Opponents((0..count).map(|i| {
        // lined up either side of the player, each on its own lane, each a different character;
        // later heats have better riders
        let lane = [-1.0f32, 1.0, -2.0, 2.0, -3.0, 3.0][i % 6];
        let skill = ([0.84f32, 0.76, 0.68, 0.60, 0.52, 0.5][i % 6] + round as f32 * 0.08).min(0.98);
        Opponent {
            rider: Rider::new(start + side * lane * 1.5 + Vec3::Y * 0.5, yaw),
            driver: AiDriver::new(lane * 1.5, skill),
            character: (player_char + 1 + i * if n_chars > 6 { 2 } else { 1 }) % n_chars,
        }
    }).collect())
}

/// "mac" -> "Mac"
fn proper(name: &str) -> String { let mut c = name.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

/// Everyone in finishing order: (name, time if finished, is the player).
fn standings(race: &Race, opponents: &[Opponent], lib: &CharLib, me: &str) -> Vec<(String, Option<f32>, bool)> {
    let mut all: Vec<(String, Option<f32>, bool, f32)> = vec![(me.to_string(), (race.state == RaceState::Finished).then_some(race.time), true, race.progress())];
    for o in opponents {
        let name = lib.chars.get(o.character).map(|c| proper(&c.name)).unwrap_or_else(|| "Rider".into());
        all.push((name, o.driver.finished, false, o.driver.progress(&race.line)));
    }
    all.sort_by(|a, b| match (a.1, b.1) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.3.total_cmp(&a.3),
    });
    all.into_iter().map(|a| (a.0, a.1, a.2)).collect()
}

/// Whose body a visual shows.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Who { Player, Ai(usize) }
/// One character: model plus a material per texture.
struct CharEntry { name: String, model: CharModel, mats: HashMap<String, Handle<StandardMaterial>>, ubers: Option<AnimSet> }
/// Every character found in the `chars` folder, and the board they all share.
#[derive(Resource, Default)]
struct CharLib { chars: Vec<CharEntry>, board: Option<CharModel>, player: usize, anims: Option<AnimSet> }
/// Root of a rider's body. Holds the smoothed pose so it moves fluidly.
#[derive(Component)]
struct RiderVisual { who: Who, character: usize, pose: Pose, roll: f32, base: Quat, anim: AnimState }
/// Where a rider's body is in its animations.
#[derive(Default)]
struct AnimState { cur: Option<Sample>, cycle: f32, was_on_snow: bool, land: f32, air: f32, grab: u8, grab_frame: f32 }
/// One textured piece of a body, re-skinned every frame.
#[derive(Component)]
struct BodyPart { part: usize, board: bool }

/// A level and everything derived from it that the game needs.
struct Loaded { level: Level, world: CollisionWorld, rails: Rails, line: Vec<Vec3>, start: Vec3, yaw: f32 }

fn open_level(dir: &Path) -> Result<Loaded, String> {
    let level = Level::load(dir)?;
    let world = build_collision(&level);
    let rails = Rails(level.splines.iter().filter_map(|sp| {
        let mut pts: Vec<Vec3> = Vec::new();
        for seg in sp.segments.iter().filter(|s| s.points.len() >= 4) {
            for i in 0..=8 {
                let p = g2b(cubic(&seg.points, i as f32 / 8.0));
                if pts.last().is_none_or(|l| l.distance(p) > 0.01) { pts.push(p); }
            }
        }
        Rail::new(pts)
    }).collect());
    let line: Vec<Vec3> = level.main_line().into_iter().map(g2b).collect();
    let (start, yaw) = match (line.first(), line.get(3)) {
        (Some(&a), Some(&b)) => (a, f32::atan2(-(b - a).x, -(b - a).z)),
        _ => { let (p, d) = level.start(); (g2b(p), { let d = g2b(d); f32::atan2(-d.x, -d.z) }) }
    };
    println!("{}: {} patches, {} instances, {} models, {} rails, {} collision triangles", dir.display(),
        level.patches.len(), level.instances.len(), level.world.models.len(), rails.0.len(), world.len());
    Ok(Loaded { level, world, rails, line, start, yaw })
}
/// Frame time averaged over the last few frames. The display refreshes at a steady rate but the
/// measured time between frames jitters by a few milliseconds; moving the rider and camera by the
/// raw value makes fast motion look juddery even at a solid 60 fps.
#[derive(Resource, Default)]
struct SmoothDt { recent: Vec<f32>, dt: f32, worst_ms: f32, worst_acc: f32, clock: f32 }

fn smooth_dt(time: Res<Time>, mut s: ResMut<SmoothDt>) {
    let raw = time.delta_secs();
    // a real hitch (loading, alt-tab) should not be averaged in
    if raw < 0.1 { s.recent.push(raw); }
    if s.recent.len() > 12 { s.recent.remove(0); }
    s.dt = if s.recent.is_empty() { raw.min(0.05) } else { s.recent.iter().sum::<f32>() / s.recent.len() as f32 };
    s.worst_acc = s.worst_acc.max(raw * 1000.0);
    s.clock += raw;
    if s.clock >= 1.0 { s.worst_ms = s.worst_acc; s.worst_acc = 0.0; s.clock = 0.0; }
}
#[derive(Resource, PartialEq, Clone, Copy)]
enum Mode { Ride, Fly }
#[derive(Component)]
struct RiderModel;

/// Game coordinates (X/Y ground, Z up, centimetres) to Bevy (Y up, metres).
fn g2b(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) * WORLD_SCALE }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first() {
        Some(a) => PathBuf::from(a),
        None => {
            // look next to the working directory and next to the executable
            let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
            [PathBuf::from("levels/gari"), PathBuf::from("../levels/gari"), exe.join("levels/gari"), exe.join("../levels/gari")]
                .into_iter().find(|p| p.join("Patches.json").exists()).unwrap_or_else(|| PathBuf::from("levels/gari"))
        }
    };
    let loaded = match open_level(&dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not load level project: {e}\nUsage: tricky-rs <folder containing Patches.json>");
            if cfg!(windows) { eprintln!("(closing in 15 seconds)"); std::thread::sleep(std::time::Duration::from_secs(15)); }
            std::process::exit(1);
        }
    };
    if std::env::var("TRICKY_PROPTEST").is_ok() {
        // ride straight into a marker standing 12 m down the start line and watch where it goes
        let dir = heading(loaded.yaw);
        let mut at = loaded.start + dir * 12.0;
        if let Some(h) = loaded.world.ground(at + Vec3::Y * 3.0, 0.0, 30.0) { at.y = h.y; }
        let mut prop = Prop::new(at, Quat::IDENTITY, Vec3::Y * 0.5, 0.5);
        let mut r = Rider::new(loaded.start + Vec3::Y * 0.5, loaded.yaw);
        let (mut t, dt, mut hit_at, mut top, mut before) = (0.0f32, 1.0 / 120.0, None, at.y, 0.0);
        while t < 12.0 {
            r.step(&loaded.world, &loaded.rails, Input { tuck: true, boost: true, ..Input::default() }, dt);
            r.boost = 1.0;
            let speed = r.vel.length();
            if prop.hit_by(&mut r) && hit_at.is_none() { hit_at = Some(t); before = speed; println!("hit at {t:.2} s: rider {:.0} -> {:.0} km/h", before * 3.6, r.vel.length() * 3.6); }
            prop.step(&loaded.world, dt);
            top = top.max(prop.pos.y);
            t += dt;
            if hit_at.is_some() && !prop.moving { break; }
        }
        // the same run through a pane of glass with two broken pieces and a x3 pickup behind it
        let mut props = vec![
            Prop::with_box(Kind::Touch { pieces: vec![1, 2], pickup: Pickup::None }, at, Quat::IDENTITY, Vec3::new(-2.0, 0.0, -0.1), Vec3::new(2.0, 3.0, 0.1)),
            Prop::with_box(Kind::Piece, at, Quat::IDENTITY, Vec3::splat(-0.3), Vec3::splat(0.3)),
            Prop::with_box(Kind::Piece, at + Vec3::Y, Quat::IDENTITY, Vec3::splat(-0.3), Vec3::splat(0.3)),
            Prop::with_box(Kind::Touch { pieces: vec![], pickup: Pickup::Multiplier(3) }, at + dir * 6.0, Quat::IDENTITY, Vec3::splat(-0.5), Vec3::splat(0.5)),
        ];
        let mut r2 = Rider::new(loaded.start + Vec3::Y * 0.5, loaded.yaw);
        let (mut t2, mut broke) = (0.0f32, None);
        while t2 < 6.0 {
            r2.step(&loaded.world, &loaded.rails, Input { tuck: true, ..Input::default() }, dt);
            let mut thrown = Vec::new();
            for p in props.iter_mut() { if let Some(list) = p.touched_by(&mut r2) { thrown.extend(list); if broke.is_none() { broke = Some((t2, r2.vel.length())); } } p.step(&loaded.world, dt); }
            for (n, i) in thrown.into_iter().enumerate() { props[i].throw(r2.vel, n); }
            t2 += dt;
        }
        println!("glass: {} ; pane hidden {}, pieces shown {} and moved {:.1} m / {:.1} m, pickup taken {}, rider multiplier x{}",
            broke.map_or("never reached".into(), |(t, v)| format!("broke at {t:.2} s at {:.0} km/h", v * 3.6)), props[0].hidden, !props[1].hidden && !props[2].hidden,
            (props[1].pos - at).length(), (props[2].pos - at - Vec3::Y).length(), props[3].hidden, r2.multiplier);
        match hit_at {
            Some(h) => println!("marker flew {:.1} m, rose {:.1} m, {} after {:.1} s", (prop.pos - at).length(), top - at.y, if prop.moving { "still moving" } else { "came to rest" }, t - h),
            None => println!("rider never reached the marker"),
        }
        return;
    }
    if let Ok(secs) = std::env::var("TRICKY_SIM") {
        rider::self_test(&loaded.world, &loaded.rails, &loaded.line, loaded.yaw, secs.parse().unwrap_or(300.0));
        return;
    }
    let mode = if std::env::var("TRICKY_CAM").is_ok() { Mode::Fly } else { Mode::Ride };
    // the other level projects sitting next to this one
    let full = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
    let mut dirs: Vec<PathBuf> = full.parent().and_then(|p| std::fs::read_dir(p).ok()).into_iter().flatten().flatten()
        .map(|e| e.path()).filter(|p| p.join("Patches.json").exists()).collect();
    dirs.sort();
    if dirs.is_empty() { dirs.push(full.clone()); }
    let current = dirs.iter().position(|d| *d == full).unwrap_or(0);
    let Loaded { level, world, rails, line, start, yaw: start_yaw } = loaded;

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title: "tricky-rs".into(), resolution: (1280.0_f32, 720.0_f32).into(), ..default() }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.55, 0.70, 0.90)))
    .insert_resource(LevelRes(level))
    .insert_resource(Overlay { rails: false, help: true })
    .insert_resource(World(world))
    .insert_resource(RiderRes(Rider::new(start + Vec3::Y * 0.5, start_yaw)))
    .insert_resource(mode)
    .insert_resource(RailsRes(rails))
    .insert_resource(RaceRes(Race::new(line)))
    .insert_resource(LevelList { dirs, current })
    .init_resource::<CamRes>()
    .insert_resource(match std::env::var("TRICKY_SCREEN").as_deref() {
        Ok("play") => ui::Game { screen: ui::Screen::Playing, ..default() },
        Ok("free") => ui::Game { screen: ui::Screen::Playing, event: ui::Event::FreeRide, ..default() },
        Ok("results") => { let mut g = ui::Game { screen: ui::Screen::Playing, ..default() }; g.finish(vec![("Elise".into(), Some(151.2), false), ("You".into(), Some(153.9), true), ("Mac".into(), None, false)]); g }
        _ => ui::Game::default(),
    })
    .init_resource::<SmoothDt>()
    .add_systems(Startup, (setup, ui::setup_ui))
    .add_systems(Update, (
        smooth_dt,
        ui::menus,
        (ride, rider_model, sync_visuals, animate_visuals, chase_camera).chain().run_if(|m: Res<Mode>| *m == Mode::Ride),
        fly_camera.run_if(|m: Res<Mode>| *m == Mode::Fly),
        (switch_level, move_props, sky_follow, draw_rails, toggles, ui::hud, screenshot),
    ).chain());
    if let Ok(path) = std::env::var("TRICKY_SHOT") {
        app.insert_resource(Shot { path, frame: 0 });
    }
    app.run();
}

// ---------------------------------------------------------------- textures

struct Tex { handle: Handle<Image>, has_alpha: bool }

/// Loads PNGs once, builds a mip chain, and remembers whether they use transparency.
struct TexCache { dir: PathBuf, map: HashMap<String, Option<(Handle<Image>, bool)>> }
impl TexCache {
    fn new(dir: PathBuf) -> Self { Self { dir, map: HashMap::new() } }
    fn get(&mut self, name: &str, images: &mut Assets<Image>) -> Option<Tex> {
        let dir = &self.dir;
        self.map.entry(name.to_string()).or_insert_with(|| {
            let img = image::open(dir.join(name)).ok()?.to_rgba8();
            let has_alpha = img.pixels().any(|p| p.0[3] < 250);
            Some((images.add(make_image(img, true)), has_alpha))
        }).clone().map(|(handle, has_alpha)| Tex { handle, has_alpha })
    }
}

fn make_image(img: image::RgbaImage, repeat: bool) -> Image {
    let (w, h) = img.dimensions();
    let mut data = img.as_raw().clone();
    let mut levels = 1;
    // box-filtered mip chain (textures are power-of-two)
    if w.is_power_of_two() && h.is_power_of_two() {
        let (mut cw, mut ch, mut cur) = (w as usize, h as usize, img.into_raw());
        while cw > 1 && ch > 1 {
            let (nw, nh) = (cw / 2, ch / 2);
            let mut next = vec![0u8; nw * nh * 4];
            for y in 0..nh {
                for x in 0..nw {
                    // weight colour by alpha so transparent texels don't bleed dark fringes
                    let mut acc = [0u32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = ((y * 2 + dy) * cw + x * 2 + dx) * 4;
                        let a = cur[i + 3] as u32;
                        for k in 0..3 { acc[k] += cur[i + k] as u32 * a.max(1); }
                        acc[3] += a.max(1);
                    }
                    let o = (y * nw + x) * 4;
                    for k in 0..3 { next[o + k] = (acc[k] / acc[3]) as u8; }
                    next[o + 3] = ((acc[3] + 2) / 4).min(255) as u8;
                }
            }
            data.extend_from_slice(&next);
            cur = next; cw = nw; ch = nh; levels += 1;
        }
    }
    let mut image = Image::new_fill(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(data);
    let mode = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode, address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    image
}

fn srgb_to_linear(c: f32) -> f32 { if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } }

// ---------------------------------------------------------------- scene

#[derive(Default)]
struct MeshBuf { pos: Vec<[f32; 3]>, nrm: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, col: Vec<[f32; 4]>, idx: Vec<u32> }
impl MeshBuf {
    fn build(self) -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        if !self.idx.is_empty() { m.insert_indices(Indices::U32(self.idx)); }
        m
    }
}

/// Static geometry is merged into one mesh per texture per CHUNK x CHUNK metre square, already in
/// Bevy space. That keeps the number of draw calls in the hundreds and lets whole squares be
/// skipped when they are off screen.
const CHUNK: f32 = 160.0;
type Chunks<'a> = HashMap<(&'a str, bool, i32, i32), MeshBuf>;
fn chunk_of(p: Vec3) -> (i32, i32) { ((p.x / CHUNK).floor() as i32, (p.z / CHUNK).floor() as i32) }

fn spawn_chunks(
    commands: &mut Commands, label: &str, chunks: Chunks, tex: &mut TexCache,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) -> usize {
    let mut mats: HashMap<(String, bool), Handle<StandardMaterial>> = HashMap::new();
    let n = chunks.len();
    for ((name, cutout, cx, cz), buf) in chunks {
        let mat = mats.entry((name.to_string(), cutout)).or_insert_with(|| {
            let t = tex.get(name, images);
            materials.add(material(t.as_ref(), cutout))
        }).clone();
        commands.spawn((Name::new(format!("{label} {name} {cx},{cz}")), LevelEntity, Mesh3d(meshes.add(buf.build())), MeshMaterial3d(mat)));
    }
    n
}

fn material(tex: Option<&Tex>, cutout: bool) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: tex.map(|t| t.handle.clone()),
        unlit: true,
        cull_mode: None,
        double_sided: true,
        alpha_mode: if cutout && tex.is_some_and(|t| t.has_alpha) { AlphaMode::Mask(0.4) } else { AlphaMode::Opaque },
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    level: Res<LevelRes>,
    rider: Res<RiderRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let level = &level.0;
    // Game space is Z-up; rotate it into Bevy's Y-up and scale it down.
    let to_bevy = Transform { rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), scale: Vec3::splat(WORLD_SCALE), ..default() };
    spawn_level(&mut commands, level, &mut meshes, &mut images, &mut materials);

    // camera at the start of the first race line, looking down the course
    let (pos, dir) = level.start();
    let mut eye = to_bevy.transform_point(pos + Vec3::Z * 300.0);
    let mut fwd = (to_bevy.rotation * dir).normalize();
    let (mut yaw, mut pitch) = (f32::atan2(-fwd.x, -fwd.z), fwd.y.asin().clamp(-1.5, 1.5) - 0.15);
    if let Ok(s) = std::env::var("TRICKY_CAM") {
        let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() >= 5 { eye = Vec3::new(v[0], v[1], v[2]); yaw = v[3].to_radians(); pitch = v[4].to_radians(); }
    }
    fwd = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 60f32.to_radians(), near: 0.2, far: 30_000.0, ..default() }),
        Tonemapping::None,
        Msaa::Sample4,
        Transform::from_translation(eye).looking_to(fwd, Vec3::Y),
        FlyCam { yaw, pitch, speed: 40.0 },
    ));
    // the real characters, if they have been extracted next to the levels
    let lib = load_chars(&level.dir, &mut images, &mut materials);
    let have_chars = !lib.chars.is_empty();
    commands.insert_resource(make_opponents(rider.0.spawn.0 - Vec3::Y * 0.5, rider.0.spawn.1, lib.player, lib.chars.len(), ui::HEAT - 1, 0));
    commands.insert_resource(lib);
    // otherwise a placeholder rider: a board with a body on it
    if !have_chars {
    let mut flat = |c: Color| materials.add(StandardMaterial { base_color: c, unlit: true, ..default() });
    let (board, jacket, skin) = (flat(Color::srgb(0.95, 0.35, 0.1)), flat(Color::srgb(0.15, 0.25, 0.75)), flat(Color::srgb(0.9, 0.72, 0.6)));
    commands.spawn((Name::new("Rider"), RiderModel, Transform::default(), Visibility::default())).with_children(|p| {
        p.spawn((Mesh3d(meshes.add(Cuboid::new(0.28, 0.04, 1.55))), MeshMaterial3d(board), Transform::from_xyz(0.0, 0.02, 0.0)));
        p.spawn((Mesh3d(meshes.add(Capsule3d::new(0.19, 0.75))), MeshMaterial3d(jacket), Transform::from_xyz(0.0, 0.78, 0.0)));
        p.spawn((Mesh3d(meshes.add(Sphere::new(0.14))), MeshMaterial3d(skin), Transform::from_xyz(0.0, 1.5, 0.0)));
    });
    }
    commands.spawn((
        Text::new("A/D steer   W crouch   S brake   Space (hold, release) jump   Shift boost   on a rail: A/D turn the board\nHold A/D or Q/E while crouched on Space to wind up; in the air: A/D spin   Q/E flip   J K L grabs   U uber trick (full meter)\nEnter restart   Esc menu   Backspace unstick   [ ] track   Tab free camera   R rails   F2 smooth edges   F1 hide this"),
        TextFont { font_size: 13.0, ..default() },
        Node { position_type: PositionType::Absolute, bottom: Val::Px(22.0), left: Val::Px(10.0), ..default() },
        HelpText,
    ));
}

fn spawn_level(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) {
    let t = spawn_terrain(commands, level, meshes, images, materials);
    let o = spawn_instances(commands, level, meshes, images, materials);
    println!("draw batches: {t} terrain, {o} objects");
    spawn_sky(commands, level, meshes, images, materials);
    let props = spawn_props(commands, level, meshes, images, materials);
    commands.insert_resource(props);
}

/// Knockable objects get their own entity (and mesh in their own frame) so they can move.
fn spawn_props(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) -> Props {
    let set = &level.world;
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut mats: HashMap<String, Handle<StandardMaterial>> = HashMap::new();
    let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let mut props: Vec<Prop> = Vec::new();
    // instance index -> prop index, so a broken thing can find its pieces
    let mut prop_of: HashMap<usize, usize> = HashMap::new();
    let mut links: Vec<(usize, Vec<usize>)> = Vec::new();
    for (inst_index, inst) in level.instances.iter().enumerate() {
        if !level.is_dynamic(inst_index) || inst.model_id < 0 { continue; }
        let piece = level.pieces.contains(&inst_index);
        if !inst.visable && !piece { continue; }
        let name = &inst.instance_name;
        let kind = if piece { Kind::Piece } else if level.touch.contains_key(&inst_index) {
            let pickup = if name.contains("MultiplierYellow") || name.contains("YellowX2") { Pickup::Multiplier(2) }
                else if name.contains("OrangeX3") { Pickup::Multiplier(3) }
                else if name.contains("RedX5") { Pickup::Multiplier(5) }
                else if name.contains("SpeedBoost") { Pickup::SpeedBoost }
                else if name.contains("TrickBoost") || name.contains("TrickB00st") { Pickup::TrickBoost }
                else { Pickup::None };
            Kind::Touch { pieces: Vec::new(), pickup }
        } else { Kind::Knock };
        let Some(model) = set.models.get(inst.model_id as usize) else { continue };
        let rot_g = quat4(inst.rotation);
        let ambient = v3(inst.ambent_light_colour);
        let lights = [(v3(inst.light_vector1), v3(inst.light_colour1)), (v3(inst.light_vector2), v3(inst.light_colour2)), (v3(inst.light_vector3), v3(inst.light_colour3))];
        // one mesh per texture, in the object's own frame (already in Bevy axes and metres)
        let mut bufs: HashMap<String, MeshBuf> = HashMap::new();
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for (obj, obj_m) in model.model_objects.iter().zip(set.object_matrices(model)) {
            let m = Mat4::from_scale(inst.scale.into()) * obj_m;
            for part in obj.mesh_data.iter().flatten() {
                let Some(mat) = set.materials.get(part.material_id.max(0) as usize) else { continue };
                let src = load_obj(&set.dir.join("Meshes").join(&part.mesh_path));
                let buf = bufs.entry(mat.texture_path.clone()).or_default();
                for ((p, n), uv) in src.positions.iter().zip(&src.normals).zip(&src.uvs) {
                    let local = g2b(m.transform_point3(Vec3::from(*p)));
                    let n_world = (rot_g * (Mat3::from_mat4(m) * Vec3::from(*n))).normalize_or(Vec3::Z);
                    let mut c = ambient;
                    for (dir, colour) in lights { if dir.length_squared() > 1e-6 { c += colour * n_world.dot(dir.normalize()).max(0.0); } }
                    let c = (c / 128.0).clamp(Vec3::ZERO, Vec3::splat(2.0));
                    lo = lo.min(local); hi = hi.max(local);
                    buf.pos.push(local.into());
                    buf.nrm.push([n[0], n[2], -n[1]]);
                    buf.uv.push(*uv);
                    buf.col.push([c.x.powf(2.2), c.y.powf(2.2), c.z.powf(2.2), 1.0]);
                }
            }
        }
        if bufs.is_empty() { continue; }
        let pos = g2b(inst.location.into());
        let rot = basis * rot_g * basis.inverse();
        let index = props.len();
        prop_of.insert(inst_index, index);
        let hidden = kind == Kind::Piece;
        // glass is see-through, everything else keeps the hard cut-out
        let glassy = kind != Kind::Knock;
        if let (Kind::Touch { .. }, Some(linked)) = (&kind, level.touch.get(&inst_index)) { links.push((index, linked.clone())); }
        props.push(Prop::with_box(kind, pos, rot, lo, hi));
        commands.spawn((Name::new(inst.instance_name.clone()), LevelEntity, PropVisual(index),
            Transform { translation: pos, rotation: rot, ..default() }, if hidden { Visibility::Hidden } else { Visibility::Inherited })).with_children(|p| {
            for (name, buf) in bufs {
                let mat = mats.entry(format!("{name}{glassy}")).or_insert_with(|| {
                    let t = tex.get(&name, images);
                    let mut m = material(t.as_ref(), true);
                    if glassy && t.as_ref().is_some_and(|t| t.has_alpha) { m.alpha_mode = AlphaMode::Blend; }
                    materials.add(m)
                }).clone();
                p.spawn((Mesh3d(meshes.add(buf.build())), MeshMaterial3d(mat)));
            }
        });
    }
    for (prop, linked) in links {
        let list: Vec<usize> = linked.iter().filter_map(|i| prop_of.get(i).copied()).filter(|p| props[*p].kind == Kind::Piece).collect();
        if let Kind::Touch { pieces, .. } = &mut props[prop].kind { *pieces = list; }
    }
    let count = |f: fn(&Kind) -> bool| props.iter().filter(|p| f(&p.kind)).count();
    println!("knockable objects: {}, breakable or pickups: {}, broken pieces: {}",
        count(|k| *k == Kind::Knock), count(|k| matches!(k, Kind::Touch { .. })), count(|k| *k == Kind::Piece));
    Props(props)
}

fn move_props(props: Res<Props>, mut q: Query<(&PropVisual, &mut Transform, &mut Visibility)>) {
    for (v, mut tf, mut vis) in &mut q {
        if let Some(p) = props.0.get(v.0) {
            tf.translation = p.pos;
            tf.rotation = p.rot;
            let want = if p.hidden { Visibility::Hidden } else { Visibility::Inherited };
            if *vis != want { *vis = want; }
        }
    }
}

/// [ and ] (or PageUp / PageDown) load the previous / next level project.
fn switch_level(
    mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut list: ResMut<LevelList>,
    old: Query<Entity, With<LevelEntity>>,
    mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>,
    mut cam: ResMut<CamRes>, lib: Res<CharLib>,
) {
    let step = (keys.just_pressed(KeyCode::BracketRight) || keys.just_pressed(KeyCode::PageDown)) as i32
        - (keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::PageUp)) as i32;
    if step == 0 || list.dirs.len() < 2 { return; }
    let n = list.dirs.len() as i32;
    let next = ((list.current as i32 + step) % n + n) % n;
    let loaded = match open_level(&list.dirs[next as usize]) {
        Ok(l) => l,
        Err(e) => { eprintln!("could not load {}: {e}", list.dirs[next as usize].display()); return; }
    };
    list.current = next as usize;
    for e in &old { commands.entity(e).despawn(); }
    spawn_level(&mut commands, &loaded.level, &mut meshes, &mut images, &mut materials);
    commands.insert_resource(RiderRes(Rider::new(loaded.start + Vec3::Y * 0.5, loaded.yaw)));
    commands.insert_resource(make_opponents(loaded.start, loaded.yaw, lib.player, lib.chars.len(), 0, 0));
    commands.insert_resource(World(loaded.world));
    commands.insert_resource(RailsRes(loaded.rails));
    commands.insert_resource(RaceRes(Race::new(loaded.line)));
    commands.insert_resource(LevelRes(loaded.level));
    cam.0 = ChaseCam::default();
}

fn spawn_terrain(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) -> usize {
    // lightmap pages, sampled on the CPU into vertex colours (each patch owns an 8x8 texel square,
    // so a 9x9 vertex grid captures all of it)
    let mut pages: HashMap<usize, Option<image::RgbaImage>> = HashMap::new();
    let mut tex = TexCache::new(level.dir.join("Textures"));
    let mut chunks: Chunks = HashMap::new();
    let n = PATCH_SEGS;
    for p in &level.patches {
        if p.points.len() < 16 { continue; }
        let page = pages.entry(p.lightmap_id).or_insert_with(|| {
            image::open(level.dir.join("Lightmaps").join(format!("{:04}.png", p.lightmap_id))).ok().map(|i| i.to_rgba8())
        });
        let (cx, cz) = chunk_of(g2b(bezier_patch(&p.points, 0.5, 0.5).0));
        let buf = chunks.entry((p.texture_path.as_str(), false, cx, cz)).or_default();
        let base = buf.pos.len() as u32;
        let [lx, ly, lw, lh] = p.light_map_point;
        for j in 0..=n {
            for i in 0..=n {
                let (s, t) = (i as f32 / n as f32, j as f32 / n as f32);
                let (pos, nrm) = bezier_patch(&p.points, s, t);
                buf.pos.push(g2b(pos).into());
                buf.nrm.push([nrm.x, nrm.z, -nrm.y]);
                buf.uv.push(bilerp(&p.uv_points, s, t).into());
                let col = match page {
                    Some(img) => {
                        let (w, h) = (img.width() as f32, img.height() as f32);
                        // stay half a texel inside the patch's square
                        let (tw, th) = ((lw * w).max(1.0), (lh * h).max(1.0));
                        let x = lx * w + 0.5 + s * (tw - 1.0);
                        let y = ly * h + 0.5 + t * (th - 1.0);
                        let c = bilinear(img, x - 0.5, y - 0.5);
                        // The PS2 can't multiply by a lightmap, so the game *subtracts* it: the pages hold
                        // orange where there is shade, which leaves the snow blue there. Multiplying by
                        // (1 - lightmap) gives the same result on white snow and never goes negative.
                        let f = |v: f32| srgb_to_linear((1.0 - v * LIGHTMAP_STRENGTH).clamp(0.0, 1.0));
                        [f(c[0]), f(c[1]), f(c[2]), 1.0]
                    }
                    None => [1.0; 4],
                };
                buf.col.push(col);
            }
        }
        let row = (n + 1) as u32;
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let a = base + j * row + i;
                buf.idx.extend_from_slice(&[a, a + row, a + 1, a + 1, a + row, a + row + 1]);
            }
        }
    }
    spawn_chunks(commands, "Terrain", chunks, &mut tex, meshes, images, materials)
}

fn bilinear(img: &image::RgbaImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: i32, yi: i32| {
        let p = img.get_pixel(xi.clamp(0, w - 1) as u32, yi.clamp(0, h - 1) as u32).0;
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
    };
    let (a, b, c, d) = (px(x0 as i32, y0 as i32), px(x0 as i32 + 1, y0 as i32), px(x0 as i32, y0 as i32 + 1), px(x0 as i32 + 1, y0 as i32 + 1));
    let mut o = [0.0; 3];
    for k in 0..3 { o[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy; }
    o
}

fn spawn_instances(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) -> usize {
    let set = &level.world;
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut objs: HashMap<String, ObjMesh> = HashMap::new();
    let mut chunks: Chunks = HashMap::new();
    for (index, inst) in level.instances.iter().enumerate() {
        if !inst.visable || inst.model_id < 0 || level.is_dynamic(index) { continue; }
        let Some(model) = set.models.get(inst.model_id as usize) else { continue };
        let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
        let lights = [
            (v3(inst.light_vector1), v3(inst.light_colour1)),
            (v3(inst.light_vector2), v3(inst.light_colour2)),
            (v3(inst.light_vector3), v3(inst.light_colour3)),
        ];
        let ambient = v3(inst.ambent_light_colour);
        let (cx, cz) = chunk_of(g2b(inst.location.into()));
        for (obj, obj_m) in model.model_objects.iter().zip(set.object_matrices(model)) {
            let Some(parts) = &obj.mesh_data else { continue };
            let m = inst_m * obj_m;
            let normal_m = Mat3::from_mat4(m).inverse().transpose();
            for part in parts {
                let src = objs.entry(part.mesh_path.clone()).or_insert_with(|| load_obj(&set.dir.join("Meshes").join(&part.mesh_path)));
                if src.positions.is_empty() { continue; }
                let Some(mat) = set.materials.get(part.material_id.max(0) as usize) else { continue };
                let buf = chunks.entry((mat.texture_path.as_str(), true, cx, cz)).or_default();
                for ((p, n), uv) in src.positions.iter().zip(&src.normals).zip(&src.uvs) {
                    let n = (normal_m * Vec3::from(*n)).normalize_or(Vec3::Z);
                    // The game lights each object with up to three directional lights plus ambient,
                    // then multiplies by the texture with 128 meaning "unchanged". Bake that per vertex.
                    let mut c = ambient;
                    for (dir, colour) in lights {
                        if dir.length_squared() > 1e-6 { c += colour * n.dot(dir.normalize()).max(0.0); }
                    }
                    let c = (c / 128.0).clamp(Vec3::ZERO, Vec3::splat(2.0));
                    buf.pos.push(g2b(m.transform_point3(Vec3::from(*p))).into());
                    buf.nrm.push([n.x, n.z, -n.y]);
                    buf.uv.push(*uv);
                    buf.col.push([c.x.powf(2.2), c.y.powf(2.2), c.z.powf(2.2), 1.0]);
                }
            }
        }
    }
    spawn_chunks(commands, "Objects", chunks, &mut tex, meshes, images, materials)
}

fn v3(a: [f32; 4]) -> Vec3 { Vec3::new(a[0], a[1], a[2]) }

fn spawn_sky(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) {
    let set = &level.sky;
    if set.models.is_empty() { return; }
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut parts = Vec::new();
    let mut radius = 1.0f32;
    for model in &set.models {
        for (obj, obj_m) in model.model_objects.iter().zip(set.object_matrices(model)) {
            for part in obj.mesh_data.iter().flatten() {
                let src = load_obj(&set.dir.join("Meshes").join(&part.mesh_path));
                for p in &src.positions { radius = radius.max(obj_m.transform_point3(Vec3::from(*p)).length()); }
                parts.push((src, obj_m, part.material_id));
            }
        }
    }
    // The sky dome follows the camera; scale it to sit well inside the far plane.
    let sky = commands.spawn((
        Name::new("Sky"), SkyRoot, LevelEntity, Visibility::default(),
        Transform { rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), scale: Vec3::splat(12_000.0 / radius), ..default() },
    )).id();
    for (src, m, mat_id) in parts {
        if src.positions.is_empty() { continue; }
        let n = src.positions.len();
        let t = set.materials.get(mat_id.max(0) as usize).and_then(|m| tex.get(&m.texture_path, images));
        let mesh = MeshBuf { pos: src.positions, nrm: src.normals, uv: src.uvs, col: vec![[1.0; 4]; n], idx: vec![] }.build();
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(materials.add(material(t.as_ref(), true))), Transform::from_matrix(m), ChildOf(sky)));
    }
}

// ---------------------------------------------------------------- systems

fn sky_follow(cam: Single<&Transform, (With<FlyCam>, Without<SkyRoot>)>, mut sky: Query<&mut Transform, With<SkyRoot>>) {
    for mut t in &mut sky { t.translation = cam.translation; }
}

fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut cam: Single<(&mut Transform, &mut FlyCam)>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
    let (ref mut tf, ref mut fly) = *cam;
    if window.cursor_options.grab_mode != CursorGrabMode::None {
        fly.yaw -= motion.delta.x * 0.0025;
        fly.pitch = (fly.pitch - motion.delta.y * 0.0025).clamp(-1.55, 1.55);
    }
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += *tf.forward(); }
    if keys.pressed(KeyCode::KeyS) { dir -= *tf.forward(); }
    if keys.pressed(KeyCode::KeyD) { dir += *tf.right(); }
    if keys.pressed(KeyCode::KeyA) { dir -= *tf.right(); }
    if keys.pressed(KeyCode::Space) { dir += Vec3::Y; }
    if keys.pressed(KeyCode::ControlLeft) { dir -= Vec3::Y; }
    if keys.just_pressed(KeyCode::Equal) { fly.speed *= 1.5; }
    if keys.just_pressed(KeyCode::Minus) { fly.speed /= 1.5; }
    let boost = if keys.pressed(KeyCode::ShiftLeft) { 5.0 } else { 1.0 };
    tf.translation += dir.normalize_or_zero() * fly.speed * boost * time.delta_secs();
}

fn toggles(
    keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<Overlay>, mut help: Query<&mut Visibility, With<HelpText>>,
    mut mode: ResMut<Mode>, mut cam: Single<(&Transform, &mut FlyCam)>, mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut model: Query<&mut Visibility, (With<RiderModel>, Without<HelpText>)>,
    mut msaa: Query<&mut Msaa>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        *mode = if *mode == Mode::Ride { Mode::Fly } else { Mode::Ride };
        // carry the current view direction over to the free camera
        let (yaw, pitch, _) = cam.0.rotation.to_euler(EulerRot::YXZ);
        cam.1.yaw = yaw;
        cam.1.pitch = pitch;
        if *mode == Mode::Ride {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
        for mut v in &mut model { *v = Visibility::Inherited; }
    }
    if keys.just_pressed(KeyCode::KeyR) { overlay.rails = !overlay.rails; }
    if keys.just_pressed(KeyCode::F2) {
        for mut m in &mut msaa { *m = if *m == Msaa::Off { Msaa::Sample4 } else { Msaa::Off }; }
    }
    if keys.just_pressed(KeyCode::F1) {
        overlay.help = !overlay.help;
        for mut v in &mut help { *v = if overlay.help { Visibility::Inherited } else { Visibility::Hidden }; }
    }
}

fn draw_rails(overlay: Res<Overlay>, level: Res<LevelRes>, mut gizmos: Gizmos) {
    if !overlay.rails { return; }
    let rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for spline in &level.0.splines {
        for seg in &spline.segments {
            if seg.points.len() < 4 { continue; }
            let pts = (0..=8).map(|i| rot * cubic(&seg.points, i as f32 / 8.0) * WORLD_SCALE);
            gizmos.linestrip(pts, Color::srgb(1.0, 0.2, 0.3));
        }
    }
}

/// With TRICKY_SHOT=file.png set, save one frame and quit (used for automated checks).
fn screenshot(mut commands: Commands, shot: Option<ResMut<Shot>>, mut exit: EventWriter<AppExit>) {
    let Some(mut shot) = shot else { return };
    shot.frame += 1;
    if shot.frame == 30 { commands.spawn(Screenshot::primary_window()).observe(save_to_disk(Path::new(&shot.path).to_path_buf())); }
    if shot.frame == 45 { exit.write(AppExit::Success); }
}

// ---------------------------------------------------------------- riding

/// Everything the rider can touch: the terrain (same tessellation as what is drawn) and the
/// collision meshes of placed objects.
fn build_collision(level: &Level) -> CollisionWorld {
    let mut world = CollisionWorld::default();
    let n = PATCH_SEGS;
    for p in &level.patches {
        if p.points.len() < 16 { continue; }
        let mut grid = Vec::with_capacity((n + 1) * (n + 1));
        for j in 0..=n {
            for i in 0..=n {
                let (pos, nrm) = bezier_patch(&p.points, i as f32 / n as f32, j as f32 / n as f32);
                grid.push((g2b(pos), Vec3::new(nrm.x, nrm.z, -nrm.y)));
            }
        }
        let row = n + 1;
        for j in 0..n {
            for i in 0..n {
                let a = j * row + i;
                for t in [[a, a + row, a + 1], [a + 1, a + row, a + row + 1]] {
                    world.add([grid[t[0]].0, grid[t[1]].0, grid[t[2]].0], Some([grid[t[0]].1, grid[t[1]].1, grid[t[2]].1]), true);
                }
            }
        }
    }
    // Objects. Collision mode 1 has its own collision mesh; mode 3 (start/finish platforms, crash
    // bags, signs, snow cats) collides with the mesh that is drawn. Mode 2 (stands, billboards,
    // pickups) is a bounding-box test in the game and is not solid here yet. Water is not solid.
    let mut cache: HashMap<String, ObjMesh> = HashMap::new();
    for (index, inst) in level.instances.iter().enumerate() {
        // the start gate's cover is the barrier that holds riders until "GO"; we start with it open
        if !inst.player_collision || inst.instance_name.contains("StartGate") || level.is_dynamic(index) { continue; }
        let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
        let mut add = |world: &mut CollisionWorld, path: PathBuf, m: Mat4| {
            let key = path.to_string_lossy().to_string();
            let mesh = cache.entry(key).or_insert_with(|| load_obj(&path));
            for t in mesh.positions.chunks_exact(3) {
                let v = |i: usize| g2b(m.transform_point3(Vec3::from(t[i])));
                world.add([v(0), v(1), v(2)], None, false);
            }
        };
        match inst.collsion_mode {
            1 => for path in inst.collsion_model_paths.iter().flatten() {
                add(&mut world, level.dir.join("Collision").join(path), inst_m);
            },
            3 if !inst.instance_name.contains("Water") => {
                let Some(model) = level.world.models.get(inst.model_id.max(0) as usize) else { continue };
                for (obj, obj_m) in model.model_objects.iter().zip(level.world.object_matrices(model)) {
                    for part in obj.mesh_data.iter().flatten() {
                        add(&mut world, level.dir.join("Meshes").join(&part.mesh_path), inst_m * obj_m);
                    }
                }
            }
            _ => {}
        }
    }
    world
}

fn ride(
    time: Res<SmoothDt>, keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>,
    world: Res<World>, rails: Res<RailsRes>, mut rider: ResMut<RiderRes>, mut race: ResMut<RaceRes>,
    mut opponents: ResMut<Opponents>, lib: Res<CharLib>, mut props: ResMut<Props>, mut game: ResMut<ui::Game>,
) {
    let r = &mut rider.0;
    let key = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let up = key(KeyCode::KeyW, KeyCode::ArrowUp);
    let down = key(KeyCode::KeyS, KeyCode::ArrowDown);
    let mut input = Input {
        steer: (key(KeyCode::KeyD, KeyCode::ArrowRight) as i32 - key(KeyCode::KeyA, KeyCode::ArrowLeft) as i32) as f32,
        tuck: up,
        brake: down,
        jump: keys.pressed(KeyCode::Space),
        flip: keys.pressed(KeyCode::KeyE) as i32 as f32 - keys.pressed(KeyCode::KeyQ) as i32 as f32,
        grab: if keys.pressed(KeyCode::KeyJ) { 1 } else if keys.pressed(KeyCode::KeyK) { 2 } else if keys.pressed(KeyCode::KeyL) { 3 } else { 0 },
        boost: key(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        uber: keys.pressed(KeyCode::KeyU),
    };
    let playing = game.screen == ui::Screen::Playing;
    let mut restart = keys.just_pressed(KeyCode::Enter) && playing && game.since > 0.3;
    let mut unstick = keys.just_pressed(KeyCode::Backspace);
    for pad in &pads {
        let x = pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        let y = pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
        if x.abs() > 0.15 { input.steer = x; }
        input.steer += pad.pressed(GamepadButton::DPadRight) as i32 as f32 - pad.pressed(GamepadButton::DPadLeft) as i32 as f32;
        input.tuck |= pad.pressed(GamepadButton::DPadUp) || y > 0.6;
        input.brake |= pad.pressed(GamepadButton::DPadDown) || y < -0.6;
        let ry = pad.get(GamepadAxis::RightStickY).unwrap_or(0.0);
        if ry.abs() > 0.5 { input.flip = ry.signum(); }
        input.jump |= pad.pressed(GamepadButton::South);
        input.boost |= pad.pressed(GamepadButton::West);
        input.uber |= pad.pressed(GamepadButton::North);
        if pad.pressed(GamepadButton::LeftTrigger) || pad.pressed(GamepadButton::LeftTrigger2) { input.grab = 1; }
        if pad.pressed(GamepadButton::RightTrigger) || pad.pressed(GamepadButton::RightTrigger2) { input.grab = 2; }
        if pad.pressed(GamepadButton::East) { input.grab = 3; }
        restart |= pad.just_pressed(GamepadButton::Start) && playing && game.since > 0.3;
        unstick |= pad.just_pressed(GamepadButton::Select);
    }
    input.steer = input.steer.clamp(-1.0, 1.0);
    input.flip = input.flip.clamp(-1.0, 1.0);
    restart |= game.lineup;
    game.lineup = false;
    if restart {
        let at = r.spawn;
        r.respawn(at);
        r.score = 0;
        r.boost = 0.0;
        r.letters = 0;
        r.last_trick.clear();
        race.0.restart();
        *opponents = make_opponents(at.0 - Vec3::Y * 0.5, at.1, lib.player, lib.chars.len(), game.opponents(), game.round);
        for p in props.0.iter_mut() { p.reset(); }
    }
    if unstick { let at = r.safe; r.respawn(at); }
    // the player's signature moves come with the character
    if let Some(list) = lib.chars.get(lib.player).and_then(|c| c.ubers.as_ref()).map(|u| u.list()) {
        if r.ubers.len() != list.len() || r.ubers.first().map(|u| &u.0) != list.first().map(|u| &u.0) { r.ubers = list; r.uber_id = 0; }
    }
    if game.screen == ui::Screen::Menu { race.0.restart(); }
    let waiting = race.0.state == RaceState::Countdown;
    if waiting { input = Input::default(); }
    if game.screen == ui::Screen::Results { input = Input { brake: true, ..Input::default() }; }
    // fixed small steps so the physics does not depend on frame rate
    let mut left = time.dt.min(0.1);
    while left > 1e-5 {
        let dt = left.min(1.0 / 120.0);
        r.step(&world.0, &rails.0, input, dt);
        if waiting { r.vel = Vec3::new(0.0, r.vel.y.min(0.0), 0.0); }
        race.0.update(r, dt);
        for o in opponents.0.iter_mut() {
            let ai = if waiting { Input::default() } else { {
                let clear = if o.rider.grounded { 0.0 } else { world.0.ground(o.rider.pos, 0.0, 40.0).map_or(40.0, |h| o.rider.pos.y - h.y) };
                o.driver.drive(&mut o.rider, &race.0.line, clear, dt)
            } };
            o.rider.step(&world.0, &rails.0, ai, dt);
            if waiting { o.rider.vel = Vec3::new(0.0, o.rider.vel.y.min(0.0), 0.0); }
        }
        if !waiting {
            // riders shove each other, and anyone can send a marker or crash bag flying
            for i in 0..opponents.0.len() {
                props::bump(r, &mut opponents.0[i].rider);
                let (head, tail) = opponents.0.split_at_mut(i + 1);
                for other in tail { props::bump(&mut head[i].rider, &mut other.rider); }
            }
            let mut thrown: Vec<(usize, Vec3)> = Vec::new();
            for p in props.0.iter_mut() {
                p.hit_by(r);
                if let Some(pieces) = p.touched_by(r) { thrown.extend(pieces.into_iter().map(|i| (i, r.vel))); }
                for o in opponents.0.iter_mut() {
                    p.hit_by(&mut o.rider);
                    if let Some(pieces) = p.touched_by(&mut o.rider) { thrown.extend(pieces.into_iter().map(|i| (i, o.rider.vel))); }
                }
                p.step(&world.0, dt);
            }
            for (n, (i, vel)) in thrown.into_iter().enumerate() {
                if let Some(p) = props.0.get_mut(i) { p.throw(vel, n); }
            }
        }
        left -= dt;
    }
    // across the line: the heat is over
    if playing && race.0.state == RaceState::Finished && game.event != ui::Event::FreeRide {
        let me = lib.chars.get(lib.player).map(|c| proper(&c.name)).unwrap_or_else(|| "Rider".into());
        game.finish(standings(&race.0, &opponents.0, &lib, &me));
    }
    // TRICKY_FORCE=grab,flip holds the player in mid-air in a given state (for close-up checks)
    if let Ok(f) = std::env::var("TRICKY_FORCE") {
        let v: Vec<f32> = f.split(',').filter_map(|x| x.parse().ok()).collect();
        let r = &mut rider.0;
        r.pos = r.spawn.0 + Vec3::Y * 1.6;
        r.vel = Vec3::ZERO;
        r.grounded = false;
        r.air_time = 1.0;
        r.grab = v.first().copied().unwrap_or(0.0) as u8;
        r.flip = v.get(1).copied().unwrap_or(0.0);
        r.crashed = v.get(2).copied().unwrap_or(0.0);
        if let Some(u) = v.get(3) { r.uber = *u; r.uber_id = 1; }
        race.0.state = RaceState::Running;
        race.0.time = 10.0;
    }
}

/// Keep exactly one body per rider, of the right character.
fn sync_visuals(
    mut commands: Commands, lib: Res<CharLib>, opponents: Res<Opponents>,
    existing: Query<(Entity, &RiderVisual)>, mut meshes: ResMut<Assets<Mesh>>,
) {
    if lib.chars.is_empty() { return; }
    let mut want: Vec<(Who, usize)> = vec![(Who::Player, lib.player)];
    want.extend(opponents.0.iter().enumerate().map(|(i, o)| (Who::Ai(i), o.character)));
    let mut have: Vec<(Who, usize)> = existing.iter().map(|(_, v)| (v.who, v.character)).collect();
    let key = |w: &(Who, usize)| (match w.0 { Who::Player => 0, Who::Ai(i) => i + 1 }, w.1);
    want.sort_by_key(key);
    have.sort_by_key(key);
    if want == have { return; }
    for (e, _) in &existing { commands.entity(e).despawn(); }
    for (who, character) in want { spawn_visual(&mut commands, who, character, &lib, &mut meshes); }
}

fn load_chars(level_dir: &Path, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> CharLib {
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    let Some(dir) = [level_dir.join("../../chars"), exe.join("../chars"), exe.join("chars"), PathBuf::from("chars")]
        .into_iter().find(|d| d.join("board.json").exists() || d.join("mac/model.json").exists()) else { return CharLib::default() };
    let mut lib = CharLib { board: CharModel::load(&dir.join("board.json")).ok(), ..default() };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.join("model.json").exists()).collect();
    dirs.sort();
    for d in dirs {
        let Ok(model) = CharModel::load(&d.join("model.json")) else { continue };
        let mut mats = HashMap::new();
        for name in model.parts.iter().map(|p| p.texture.as_str()).chain(["bord"]) {
            let file = d.join(format!("{name}.png"));
            let tex = image::open(&file).ok().map(|i| images.add(make_image(i.to_rgba8(), true)));
            mats.insert(name.to_string(), materials.add(StandardMaterial {
                base_color_texture: tex, unlit: true, cull_mode: None, double_sided: true, ..default()
            }));
        }
        let ubers = AnimSet::load(&d.join("uber.json")).ok().filter(|a| a.len() > 0);
        lib.chars.push(CharEntry { name: d.file_name().unwrap().to_string_lossy().to_string(), model, mats, ubers });
    }
    lib.player = lib.chars.iter().position(|c| c.name == "mac").unwrap_or(0);
    lib.anims = AnimSet::load(&dir.join("anims/bx.json")).ok().filter(|a| a.len() > 0);
    println!("animations: {}", lib.anims.as_ref().map_or(0, |a| a.len()));
    println!("characters: {}", lib.chars.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
    lib
}

fn spawn_visual(commands: &mut Commands, who: Who, character: usize, lib: &CharLib, meshes: &mut Assets<Mesh>) -> Entity {
    let entry = &lib.chars[character];
    let mut root = commands.spawn((Name::new(format!("Rider {}", entry.name)), RiderVisual { who, character, pose: Pose::default(), roll: 0.0, base: Quat::IDENTITY, anim: AnimState::default() },
        Transform::default(), Visibility::default()));
    if who == Who::Player { root.insert(RiderModel); }
    root.with_children(|p| {
        let mut piece = |verts: &[character::Vert], mat: Option<&Handle<StandardMaterial>>, part: usize, board: bool| {
            let n = verts.len();
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; n])
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, verts.iter().map(|v| [v.uv.x, v.uv.y]).collect::<Vec<_>>())
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n]);
            if let Some(mat) = mat {
                p.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat.clone()), BodyPart { part, board }, bevy::render::view::NoFrustumCulling));
            }
        };
        for (i, part) in entry.model.parts.iter().enumerate() { piece(&part.verts, entry.mats.get(&part.texture), i, false); }
        if let Some(board) = &lib.board {
            for (i, part) in board.parts.iter().enumerate() { piece(&part.verts, entry.mats.get("bord"), i, true); }
        }
    });
    root.id()
}

/// Move every rider's body to its rider, pose it, and re-skin the meshes.
fn animate_visuals(
    time: Res<SmoothDt>, rider: Res<RiderRes>, opponents: Res<Opponents>, lib: Res<CharLib>, race: Res<RaceRes>,
    mut roots: Query<(&mut RiderVisual, &mut Transform, &Children)>,
    parts: Query<(&BodyPart, &Mesh3d)>, mut meshes: ResMut<Assets<Mesh>>,
    mut scratch: Local<(Vec<[f32; 3]>, Vec<[f32; 3]>)>,
) {
    let dt = time.dt;
    for (mut vis, mut tf, children) in &mut roots {
        let r: &Rider = match vis.who { Who::Player => &rider.0, Who::Ai(i) => match opponents.0.get(i) { Some(o) => &o.rider, None => continue } };
        let Some(entry) = lib.chars.get(vis.character) else { continue };
        // where the body is and which way it faces
        let on_snow = r.grounded || r.air_time < 0.15 || r.rail.is_some();
        let up = if on_snow { r.normal } else { Vec3::Y };
        let h = heading(r.yaw);
        let fwd = (h - up * h.dot(up)).normalize_or(h);
        let k = |rate: f32| 1.0 - (-rate * dt).exp();
        let want_roll = if r.grounded { -r.input.steer * 0.32 * (r.vel.length() / 12.0).min(1.0) } else { 0.0 };
        vis.roll += (want_roll - vis.roll) * k(8.0);
        let base = Transform::from_translation(r.pos).looking_to(fwd, up).rotation * Quat::from_rotation_z(vis.roll);
        tf.translation = r.pos;
        vis.base = vis.base.slerp(base, k(14.0));
        // flips turn the whole body about its middle, on top of the smoothed heading
        let flip = Quat::from_rotation_x(-r.flip);
        tf.rotation = vis.base * flip;
        if r.flip != 0.0 { tf.translation += vis.base * (Vec3::Y * 0.9 - flip * Vec3::Y * 0.9); }
        // what the body is doing
        let want = Pose {
            crouch: if r.crashed > 0.0 { 1.0 } else if r.charge > 0.0 { 0.55 + 0.45 * r.charge } else if r.flip != 0.0 { 0.85 }
                else if r.input.tuck || r.boosting { 0.7 } else if r.input.brake { 0.1 } else { 0.3 },
            air: if on_snow { 0.0 } else { 1.0 },
            lean: r.input.steer,
            trick: if r.grab != 0 { 1.0 } else { 0.0 }, trick_id: if r.grab != 0 { r.grab } else { vis.pose.trick_id },
        };
        vis.pose.crouch += (want.crouch - vis.pose.crouch) * k(9.0);
        vis.pose.air += (want.air - vis.pose.air) * k(7.0);
        vis.pose.lean += (want.lean - vis.pose.lean) * k(8.0);
        vis.pose.trick += (want.trick - vis.pose.trick) * k(12.0);
        vis.pose.trick_id = want.trick_id;
        // frames since "GO" while the gate-start clip is still playing (negative during the countdown)
        let gate = match race.0.state {
            RaceState::Countdown => Some(-1.0),
            RaceState::Running if race.0.time < 31.0 / character::ANIM_FPS => Some(race.0.time * character::ANIM_FPS),
            _ => None,
        };
        // the game's own animation if we have it, the code-built pose otherwise
        let sample = lib.anims.as_ref().and_then(|set| animate(set, entry.ubers.as_ref(), gate, r, &mut *vis, on_snow, dt));
        let (mats, lift) = match &sample { Some(sm) => (entry.model.skin_sample(sm), 0.0), None => entry.model.skin(&vis.pose) };
        let board_mats = match (&sample, &lib.board) { (Some(sm), Some(b)) => b.skin_board(sm), _ => vec![Mat4::IDENTITY] };
        let sun = (tf.rotation.inverse() * Vec3::new(0.35, 0.8, 0.45)).normalize();
        for child in children.iter() {
            let Ok((part, mesh)) = parts.get(child) else { continue };
            let Some(mesh) = meshes.get_mut(&mesh.0) else { continue };
            let s = &mut *scratch;
            let (pos, nrm) = (&mut s.0, &mut s.1);
            let (model, mats) = if part.board { (lib.board.as_ref(), &board_mats) } else { (Some(&entry.model), &mats) };
            let Some(p) = model.and_then(|m| m.parts.get(part.part)) else { continue };
            if sample.is_some() { character::skin_part_anim(p, mats, pos, nrm); }
            else { character::skin_part(p, mats, if part.board { 0.0 } else { lift }, pos, nrm); }
            let col: Vec<[f32; 4]> = nrm.iter().map(|n| {
                let l = (0.58 + 0.6 * Vec3::from(*n).dot(sun).max(0.0)).min(1.25).powf(2.2);
                [l, l, l, 1.0]
            }).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        }
    }
}

/// Pick and blend the animation clips that match what the rider is doing.
fn animate(set: &AnimSet, ubers: Option<&AnimSet>, gate: Option<f32>, r: &Rider, vis: &mut RiderVisual, on_snow: bool, dt: f32) -> Option<Sample> {
    let fps = character::ANIM_FPS;
    let crouch = vis.pose.crouch;
    let lean = vis.pose.lean;
    let a = &mut vis.anim;
    a.cycle += dt * fps;
    if on_snow && !a.was_on_snow && a.air > 0.35 { a.land = 0.0; }
    a.air = if on_snow { 0.0 } else { a.air + dt };
    a.was_on_snow = on_snow;
    a.land += dt * fps;
    if r.grab != 0 { if a.grab != r.grab { a.grab = r.grab; a.grab_frame = 0.0; } }
    let clip = |n: &str| set.get(n);
    let target: Sample = if let Some(g) = gate {
        // in the gate: hold the first frame through the countdown, then push out
        clip("bxG_GATESTART")?.at(g.max(0.0), false)
    } else if r.crashed > 0.0 {
        // wipe out, then get back up
        let (bail, up) = (clip("bxB_FWD")?, clip("bxGU_FROMFACEFWD")?);
        let f = (rider::CRASH_SECONDS - r.crashed) * fps;
        if f < bail.len() { bail.at(f, false) } else { up.at(f - bail.len(), false) }
    } else if r.uber > 0.0 && ubers.and_then(|u| u.nth(r.uber_id)).is_some() {
        ubers?.nth(r.uber_id)?.at(r.uber, false)
    } else if r.rail.is_some() {
        clip("bxRS_CYCLEBASE")?.at(a.cycle, true)
    } else if !on_snow {
        let grab = match if r.grab != 0 { r.grab } else if a.grab_frame > 0.0 { a.grab } else { 0 } { 1 => "bxT_NOSEGRAB", 2 => "bxT_TAILGRAB", 3 => "bxT_METHOD", _ => "" };
        if let Some(c) = clip(grab) {
            // reach in while the button is held, hold at the peak, play out on release
            let peak = c.len() * 0.5;
            if r.grab != 0 { a.grab_frame = (a.grab_frame + dt * fps).min(peak); }
            else { a.grab_frame += dt * fps; if a.grab_frame >= c.len() - 1.0 { a.grab_frame = 0.0; a.grab = 0; } }
            c.at(a.grab_frame.max(0.01), false)
        } else if r.flip.abs() > 0.3 {
            clip(if r.flip > 0.0 { "bxA_FLIPCYCLEFWD" } else { "bxA_FLIPCYCLEBWD" })?.at(a.cycle, true)
        } else if r.spin.abs() > 0.6 {
            clip(if r.spin > 0.0 { "bxA_SPINCYCLEFS" } else { "bxA_SPINCYCLEBS" })?.at(a.cycle, true)
        } else {
            let c = clip("bxJ_TAKEOFF")?;
            c.at(a.air * fps, false)
        }
    } else if a.land < clip("bxL_NORMAL")?.len() - 1.0 {
        clip("bxL_NORMAL")?.at(a.land, false)
    } else {
        // riding: stand / crouch cycles, replaced by the turn clips as the rider leans
        let up = clip("bxRL_BASECYCLEFAST")?.at(a.cycle, true);
        let down = clip("bxRL_CROUCHCYCLE")?.at(a.cycle, true);
        let base = up.blend(&down, ((crouch - 0.3) / 0.5).clamp(0.0, 1.0));
        if lean.abs() > 0.02 {
            let heel = lean < 0.0;
            let stand = clip(if heel { "bxR_TURNHS" } else { "bxR_TURNTS" })?;
            let tuck = clip(if heel { "bxRC_TURNHS" } else { "bxRC_TURNTS" })?;
            let f = lean.abs().min(1.0) * 0.8;
            let turn = stand.at(f * (stand.len() - 1.0), false).blend(&tuck.at(f * (tuck.len() - 1.0), false), ((crouch - 0.3) / 0.5).clamp(0.0, 1.0));
            base.blend(&turn, (lean.abs() * 4.0).min(1.0))
        } else { base }
    };
    if on_snow { a.grab = 0; a.grab_frame = 0.0; }
    // ease from whatever the body was doing into the new pose
    let out = match &a.cur { Some(cur) => cur.blend(&target, 1.0 - (-14.0 * dt).exp()), None => target };
    a.cur = Some(out);
    Some(out)
}

fn rider_model(time: Res<SmoothDt>, rider: Res<RiderRes>, model: Option<Single<&mut Transform, (With<RiderModel>, Without<RiderVisual>)>>) {
    let Some(mut model) = model else { return };
    let r = &rider.0;
    let up = if r.grounded || r.air_time < 0.15 { r.normal } else { Vec3::Y };
    let h = heading(r.yaw);
    let fwd = (h - up * h.dot(up)).normalize_or(h);
    let target = Transform::from_translation(r.pos).looking_to(fwd, up).rotation;
    model.translation = r.pos;
    model.rotation = model.rotation.slerp(target, 1.0 - (-14.0 * time.dt).exp());
}

fn chase_camera(
    time: Res<SmoothDt>, rider: Res<RiderRes>, world: Res<World>, mut chase: ResMut<CamRes>, game: Res<ui::Game>,
    mut cam: Single<&mut Transform, (With<FlyCam>, Without<RiderModel>)>,
) {
    let (mut pos, mut look) = chase.0.update(&world.0, &rider.0, time.dt);
    if game.screen == ui::Screen::Menu {
        // the menu looks at the chosen rider in the gate, circling slowly
        let a = rider.0.yaw + 0.6 + (game.since * 0.25).sin() * 0.9;
        look = rider.0.pos + Vec3::Y * 1.0;
        pos = look - Vec3::new(a.sin(), 0.0, a.cos()) * 4.2 + Vec3::Y * 0.7;
    }
    // TRICKY_ORBIT=distance,angle,height puts the camera around the rider instead (for close-ups)
    if let Ok(o) = std::env::var("TRICKY_ORBIT") {
        let v: Vec<f32> = o.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() == 3 {
            let a = rider.0.yaw + v[1].to_radians();
            look = rider.0.pos + Vec3::Y * 0.9;
            pos = look + Vec3::new(a.sin(), 0.0, a.cos()) * v[0] + Vec3::Y * v[2];
        }
    }
    **cam = Transform::from_translation(pos).looking_at(look, Vec3::Y);
}

fn clock(t: f32) -> String { format!("{}:{:04.1}", (t / 60.0) as u32, t % 60.0) }

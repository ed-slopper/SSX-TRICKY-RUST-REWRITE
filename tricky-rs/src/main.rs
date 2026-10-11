//! tricky-rs, stage 2: load an extracted SSX Tricky level project and ride it.
//!
//!   tricky-rs [path/to/level/project]        (default: levels/gari)
//!
//! Ride:  A/D or arrows steer, W/Up tuck (and push off), S/Down brake, hold Space to crouch and
//!        release to jump, steer in the air to spin. Backspace: back to last safe spot. Enter: restart.
//!        Gamepad: left stick, A/Cross jump, right trigger tuck, X/Square brake.
//! Tab switches to the free camera: click to look, WASD, Space/Ctrl up/down, Shift fast, +/- speed.
//! R rails overlay, F1 help.

// Bevy systems take their resources and queries as arguments, and queries are spelled as types: long
// argument lists and complex types are how Bevy code reads (Bevy itself allows these two lints).
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod character;
use tricky_game::{collide, rails, trickdata};
mod level;
mod props;
mod ui;
mod sound;
mod spray;
mod tracks;
mod book;
mod logic;
mod particles;
mod boardsound;
mod hudsprites;
mod worldsound;
mod sfnfont;
mod coursemusic;
mod pathmusic;
mod editor;
mod course;
mod intro;
mod rivals;
mod rider;
mod worldanim;

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
#[derive(Component)]
struct CamLabel;
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
struct Opponent { rider: Rider, driver: AiDriver, character: usize, prog: course::Progress, /// its grudge against the player (made once the characters are known)
    grudge: Option<rivals::Grudge>, taunt_wait: f32 }
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
            rider: { let mut r = Rider::new(start + side * lane * 1.5 + Vec3::Y * 0.5, yaw); r.ai = true; r },
            driver: AiDriver::new(lane * 1.5, skill),
            character: (player_char + 1 + i * if n_chars > 6 { 2 } else { 1 }) % n_chars,
            prog: Default::default(),
            grudge: None, taunt_wait: 1.0,
        }
    }).collect())
}

/// Going after the player (`AIComputer_AttackTarget`): ahead of us, steer for a spot 3 m along
/// their way and tuck (and boost when lined up); behind us and close, ease off to let them come.
fn attack(mut ai: Input, me: &Rider, them: &Rider) -> Input {
    if !me.grounded || me.rail.is_some() || them.crashed > 0.0 { return ai; }
    let to = them.pos - me.pos;
    let flat = Vec3::new(to.x, 0.0, to.z);
    if flat.length() > 40.0 || to.y.abs() > 6.0 { return ai; }
    let fwd = heading(me.yaw);
    let right = Vec3::new(me.yaw.cos(), 0.0, -me.yaw.sin());
    if flat.dot(fwd) > 0.0 {
        let v = Vec3::new(them.vel.x, 0.0, them.vel.z).normalize_or_zero();
        let aim = (flat + v * 3.0).normalize_or_zero();
        let ang = aim.dot(right).atan2(aim.dot(fwd));
        ai.steer = (ang * 3.0).clamp(-1.0, 1.0);
        ai.wind = Some(0.0);
        let a = ang.abs().to_degrees();
        ai.tuck = a < 50.0;
        ai.boost = a < 15.0 && me.boost > 0.0;
        ai.brake = false;
    } else if flat.length() > 2.0 && me.vel.length() > them.vel.length() {
        ai.tuck = false;
        ai.boost = false;
        ai.brake = true;
    }
    ai
}

/// "mac" -> "Mac"
fn proper(name: &str) -> String { let mut c = name.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

/// Everyone in finishing order: (name, time if finished, is the player).
fn standings(race: &Race, opponents: &[Opponent], lib: &CharLib, me: &str) -> Vec<(String, Option<f32>, bool)> {
    // the original sorts the riders still going by distance to the finish along the race lines
    let by_lines = !race.lines.is_empty();
    let mine = if by_lines { -race.prog.dtf } else { race.progress() };
    let mut all: Vec<(String, Option<f32>, bool, f32)> = vec![(me.to_string(), (race.state == RaceState::Finished).then_some(race.time), true, mine)];
    for o in opponents {
        let name = lib.chars.get(o.character).map(|c| proper(&c.name)).unwrap_or_else(|| "Rider".into());
        all.push((name, o.driver.finished, false, if by_lines { -o.prog.dtf } else { o.driver.progress(&race.line) }));
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
struct CharEntry { name: String, model: CharModel, mats: HashMap<String, Handle<StandardMaterial>>, /// the uber tricks for each board type: BX, freestyle, alpine (each has its own, and the
    /// signature uber is in the rider's own board type's set)
    ubers: [Option<AnimSet>; 3],
    /// the twelve board graphics, and this rider's row in the game's tables
    boards: Vec<Handle<StandardMaterial>>, data: usize }
/// Every character found in the `chars` folder, and the board they all share.
#[derive(Resource, Default)]
struct CharLib { chars: Vec<CharEntry>, board: Option<CharModel>,
    /// board shapes: BX, freestyle, alpine, each regular then goofy
    shapes: Vec<Option<CharModel>>, player: usize, anims: Option<AnimSet>,
    /// the pre-race scene clips (scrSG_*, scrGAT_*), if exported
    scenes: Option<AnimSet>,
    /// the freestyle (fr) and alpine (ex) boards' own riding sets, and the shared wipe-out set (cm)
    fr: Option<AnimSet>, ex: Option<AnimSet>, cm: Option<AnimSet>,
    /// where the characters were found (the trick book lives there too)
    dir: PathBuf }
/// Root of a rider's body. Holds the smoothed pose so it moves fluidly.
#[derive(Component)]
struct RiderVisual { who: Who, character: usize, board: usize, shape: usize, pose: Pose, roll: f32, base: Quat, anim: AnimState }
/// Where a rider's body is in its animations.
#[derive(Default)]
struct AnimState { cur: Option<Sample>, cycle: f32, was_on_snow: bool, land: f32, air: f32, grab: u8, grab_frame: f32, rot: f32, tweak: f32, done: f32,
    /// seconds since a tumbling body last hit the snow (the impact clip, cmI_*), and whether it was down
    impact: f32, was_down: bool }
/// One textured piece of a body, re-skinned every frame.
#[derive(Component)]
struct BodyPart { part: usize, board: bool }

/// A level and everything derived from it that the game needs.
struct Loaded { level: Level, world: CollisionWorld, rails: Rails, line: Vec<Vec3>, start: Vec3, yaw: f32, course: rider::AiCourse, lines: course::Lines, paths: course::AiPaths, tube: Option<props::Tube>, cml: Option<std::sync::Arc<intro::Cml>>, stage: Option<(Vec3, Quat)>, finish_area: Option<(Vec3, Quat)> }

fn open_level(dir: &Path) -> Result<Loaded, String> {
    let level = Level::load(dir)?;
    let mut world = build_collision(&level);
    let mut rails = Rails(level.splines.iter().enumerate().filter_map(|(si, sp)| {
        let mut pts: Vec<Vec3> = Vec::new();
        for seg in sp.segments.iter().filter(|s| s.points.len() >= 4) {
            for i in 0..=8 {
                let p = g2b(cubic(&seg.points, i as f32 / 8.0));
                if pts.last().is_none_or(|l| l.distance(p) > 0.01) { pts.push(p); }
            }
        }
        Rail::new(pts).map(|mut r| { r.spline = si; r })
    }).collect(), level.logic.rails.clone());
    let _ = &mut rails;
    let line: Vec<Vec3> = level.main_line().into_iter().map(g2b).collect();
    let (start, yaw) = match (line.first(), line.get(3)) {
        (Some(&a), Some(&b)) => (a, f32::atan2(-(b - a).x, -(b - a).z)),
        _ => { let (p, d) = level.start(); (g2b(p), { let d = g2b(d); f32::atan2(-d.x, -d.z) }) }
    };
    println!("{}: {} patches, {} instances, {} models, {} rails, {} collision triangles", dir.display(),
        level.patches.len(), level.instances.len(), level.world.models.len(), rails.0.len(), world.len());
    let events: Vec<(Vec3, Vec3, i32, i32)> = level.ai_events().into_iter().map(|(a, b, t, v)| (g2b(a), g2b(b), t, v)).collect();
    let course = rider::AiCourse::build(&line, &events);
    let lines = level.race_lines();
    let paths = level.ai_paths();
    let tube = props::Tube::find(&level);
    // Tokyo Megaplex's tube boosts are handled by the tube itself
    if tube.is_some() { world.pads.retain(|p| !matches!(p.kind, collide::PadKind::Boost { .. })); }
    println!("level scripts: {} objects switched by event, {} script resets, {} spline riders, {} boost pads, {} teleports, {} pickups, {} doors, {} glass pieces",
        level.switchable.len(), level.logic.resets.len(), level.logic.movers.len(), world.pads.iter().filter(|p| matches!(p.kind, collide::PadKind::Boost { .. })).count(), level.logic.teleports.len(), world.pads.iter().filter(|p| matches!(p.kind, collide::PadKind::Pickup { .. })).count(), world.gates.len(), level.glass().len());
    let cml = intro::Cml::load(dir).map(std::sync::Arc::new);
    let stage = level.instances.iter().find(|i| i.instance_name.starts_with("Mdl_StageArea_Start")).map(|i| (Vec3::from(i.location), level::quat4(i.rotation)));
    let finish_area = level.instances.iter().find(|i| i.instance_name.starts_with("Mdl_StageArea_Finish")).map(|i| (Vec3::from(i.location), level::quat4(i.rotation)));
    Ok(Loaded { level, world, rails, line, start, yaw, course, lines, paths, tube, cml, stage, finish_area })
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
    let mut loaded = match open_level(&dir) {
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
        let (mut t, dt, mut hit_at, mut top) = (0.0f32, 1.0 / 120.0, None, at.y);
        while t < 12.0 {
            r.step(&loaded.world, &loaded.rails, Input { tuck: true, boost: true, ..Input::default() }, dt);
            r.boost = 1.0;
            let speed = r.vel.length();
            if prop.hit_by(&mut r) && hit_at.is_none() { hit_at = Some(t); println!("hit at {t:.2} s: rider {:.0} -> {:.0} km/h", speed * 3.6, r.vel.length() * 3.6); }
            prop.step(&loaded.world, dt);
            top = top.max(prop.pos.y);
            t += dt;
            if hit_at.is_some() && !prop.moving { break; }
        }
        // the same run through a pane of glass with two broken pieces and a x3 pickup behind it
        let mut props = [
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
    if std::env::var("TRICKY_GATETEST").is_ok() {
        // line six riders up as a race does and see who gets away
        let mut field = make_opponents(loaded.start, loaded.yaw, 0, 12, 5, 0);
        field.0.push(Opponent { rider: Rider::new(loaded.start + Vec3::Y * 0.5, loaded.yaw), driver: AiDriver::new(0.0, 0.9), character: 0, prog: Default::default(), grudge: None, taunt_wait: 1.0 });
        println!("start {:.1?} yaw {:.2} line[0] {:.1?} line[1] {:.1?}", loaded.start, loaded.yaw, loaded.line[0], loaded.line[1]);
        let from: Vec<Vec3> = field.0.iter().map(|o| o.rider.pos).collect();
        let (mut t, dt) = (0.0f32, 1.0 / 120.0);
        while t < 8.0 {
            for o in field.0.iter_mut() {
                let input = o.driver.drive(&mut o.rider, &loaded.world, &loaded.line, &loaded.course, dt);
                o.rider.step(&loaded.world, &loaded.rails, input, dt);
            }
            t += dt;
        }
        for (o, f) in field.0.iter().zip(&from) {
            println!("lane {:5.1}: went {:5.1} m, now at {:.1?}, speed {:.1}, grounded {}", o.driver.offset, (o.rider.pos - *f).length(), o.rider.pos, o.rider.vel.length(), o.rider.grounded);
            if (o.rider.pos - *f).length() < 20.0 { loaded.world.explain(o.rider.pos + Vec3::Y * 0.8, 0.6); }
        }
        return;
    }
    if let Ok(secs) = std::env::var("TRICKY_SIM") {
        rider::self_test(&mut loaded.world, &loaded.rails, &loaded.line, &loaded.course, &loaded.lines, &loaded.paths, loaded.tube.as_ref(), loaded.yaw, secs.parse().unwrap_or(300.0));
        return;
    }
    // TRICKY_CAM=x,y,z,yaw,pitch flies; x,y,z,tx,ty,tz is a fixed camera while the race runs (chase_camera)
    let mode = if std::env::var("TRICKY_CAM").is_ok_and(|v| v.split(',').count() != 6) { Mode::Fly } else { Mode::Ride };
    // the other level projects sitting next to this one
    let full = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
    let mut dirs: Vec<PathBuf> = full.parent().and_then(|p| std::fs::read_dir(p).ok()).into_iter().flatten().flatten()
        .map(|e| e.path()).filter(|p| p.join("Patches.json").exists()).collect();
    dirs.sort();
    if dirs.is_empty() { dirs.push(full.clone()); }
    let current = dirs.iter().position(|d| *d == full).unwrap_or(0);
    let Loaded { level, world, rails, line, start, yaw: start_yaw, course, lines, paths, tube, cml, stage, finish_area } = loaded;
    // TRICKY_SPAWN=x,y,z,yaw (Bevy metres, radians): start the run there instead (for checks)
    let (start, start_yaw) = std::env::var("TRICKY_SPAWN").ok().map(|v| v.split(',').filter_map(|x| x.parse::<f32>().ok()).collect::<Vec<_>>())
        .filter(|v| v.len() == 4).map_or((start, start_yaw), |v| (Vec3::new(v[0], v[1], v[2]), v[3]));

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
    .insert_resource(RaceRes(Race { course, lines, paths, tube, cml, stage, finish_area, ..Race::new(line) }))
    .insert_resource(LevelList { dirs, current })
    .init_resource::<CamRes>()
    .init_resource::<editor::Editor>()
    .init_resource::<editor::Reload>()
    .insert_resource(ui::Records::load())
    .init_resource::<book::Book>()
    .init_resource::<hudsprites::HudSprites>()
    .init_resource::<sfnfont::SfnFonts>()
    .init_resource::<coursemusic::CourseMusic>()
    .add_plugins(pathmusic::plugin)
    .insert_resource(match std::env::var("TRICKY_SCREEN").as_deref() {
        Ok("play") => ui::Game { screen: ui::Screen::Playing, ..default() },
        Ok("free") => ui::Game { screen: ui::Screen::Playing, event: ui::Event::FreeRide, ..default() },
        Ok("results") => { let mut g = ui::Game { screen: ui::Screen::Playing, ..default() }; g.finish(vec![("Elise".into(), Some(151.2), false), ("You".into(), Some(153.9), true), ("Mac".into(), None, false)]); g }
        _ => ui::Game::default(),
    })
    .init_resource::<SmoothDt>()
    .add_systems(Startup, (setup, ui::setup_ui, sound::setup_sound, spray::setup_spray, tracks::setup_tracks, particles::setup_particles, boardsound::setup_board_sound, editor::setup_editor, worldsound::setup_world_sound))
    .add_systems(Update, (
        smooth_dt,
        ui::menus,
        (trick_only_ramps, ride, rider_model, sync_visuals, animate_visuals, chase_camera).chain().run_if(|m: Res<Mode>| *m == Mode::Ride),
        fly_camera.run_if(|m: Res<Mode>| *m == Mode::Fly),
        (switch_level, move_movers, (worldanim::world_anim, animate_objects).chain(), move_props, sky_follow, draw_rails, toggles, ui::records, ui::trick_book, (ui::hud, hudsprites::hud_sprites, sfnfont::sfn_text).chain(), (sound::course_music, coursemusic::course_music, pathmusic::path_music, sound::sound).chain(), boardsound::board_sound, (particles::particles, worldsound::world_sound).chain(), spray::spray, tracks::tracks, editor::editor, screenshot),
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
    fn append(&mut self, o: MeshBuf) {
        let base = self.pos.len() as u32;
        self.idx.extend(o.idx.iter().map(|i| i + base));
        self.pos.extend(o.pos); self.nrm.extend(o.nrm); self.uv.extend(o.uv); self.col.extend(o.col);
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
) -> usize { spawn_chunks_with(commands, label, chunks, tex, meshes, images, materials, ()) }
#[allow(clippy::too_many_arguments)]
fn spawn_chunks_with<B: Bundle + Clone>(
    commands: &mut Commands, label: &str, chunks: Chunks, tex: &mut TexCache,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, extra: B,
) -> usize {
    let mut mats: HashMap<(String, bool), Handle<StandardMaterial>> = HashMap::new();
    let n = chunks.len();
    for ((name, cutout, cx, cz), buf) in chunks {
        let mat = mats.entry((name.to_string(), cutout)).or_insert_with(|| {
            let t = tex.get(name, images);
            materials.add(material(t.as_ref(), cutout))
        }).clone();
        commands.spawn((Name::new(format!("{label} {name} {cx},{cz}")), LevelEntity, Mesh3d(meshes.add(buf.build())), MeshMaterial3d(mat), extra.clone()));
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
    commands.insert_resource(book::Book::load(&lib.dir));
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
        Text::new("A/D steer   W crouch   S brake   Shift boost\nSpace: hold to crouch, release to jump\nwind up: hold A/D or Q/E while crouched\nair: A/D spin   Q/E flip   J L U O grabs   K tweak / uber\nrail: A/D balance   Shift+A/D turn the board   F shove\nEnter restart   Esc menu   Backspace unstick   Space skips the intro\nM music   N sound   C camera   Tab free camera   F1 hide this"),
        TextFont { font_size: 13.0, ..default() },
        Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(10.0), ..default() },
        HelpText,
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 20.0, ..default() },
        Node { position_type: PositionType::Absolute, bottom: Val::Px(60.0), left: Val::Percent(44.0), ..default() },
        CamLabel,
    ));
}

fn spawn_level(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>,
) {
    let t = spawn_terrain(commands, level, meshes, images, materials);
    let mut wa = worldanim::WorldAnim::new(level);
    let o = spawn_instances(commands, level, meshes, images, materials, &mut wa);
    println!("draw batches: {t} terrain, {o} objects");
    spawn_sky(commands, level, meshes, images, materials);
    let props = spawn_props(commands, level, meshes, images, materials, &mut wa);
    let anims = spawn_animated(commands, level, meshes, images, materials, &props);
    println!("{}", wa.summary());
    commands.insert_resource(wa);
    commands.insert_resource(anims);
    commands.insert_resource(props);
}

/// Is this instance drawn as a keyframed object (its own entity per model part)?
fn animated(level: &Level, i: usize) -> bool {
    (level.logic.anims.contains_key(&i) || level.logic.anim_delta.contains_key(&i) || level.logic.anim_on.contains_key(&i))
        && !level.is_dynamic(i) && !level.switchable.contains(&i)
        && level.instances.get(i).and_then(|inst| level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0))
            .is_some_and(|m| m.model_objects.iter().any(|o| o.animation.is_some()))
}

/// A keyframed object at run time (`cAnimObjectNode`): its play settings, the model's length, its
/// clock and direction, and its parts (child entities) in model order. An AnimDelta clip
/// (`cAnimDeltaNode` 0x199db8) only runs while its budget (s, given by the scripts) lasts.
struct AnimInst { def: logic::AnimDef, model: usize, len: f32, t: f32, dir: f32, rate: f32, parts: Vec<Entity>, /** a prop turned whole (the gems) */ prop: Option<usize>, inst: usize, budget: Option<f32> }
#[derive(Resource, Default)]
struct AnimObjects(Vec<AnimInst>);

fn spawn_animated(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, props: &Props,
) -> AnimObjects {
    let set = &level.world;
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut mats: HashMap<String, Handle<StandardMaterial>> = HashMap::new();
    let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let mut out = AnimObjects::default();
    let mut seed = 0x9E37_79B9u32;
    let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; (seed % 10000) as f32 / 10000.0 };
    for (i, inst) in level.instances.iter().enumerate() {
        if !inst.visable || !animated(level, i) { continue; }
        let Some(model) = set.models.get(inst.model_id as usize) else { continue };
        // an iris door's clip waits for its button (`worldanim` runs it with the door's collision)
        let (def, budget) = match (level.logic.anims.get(&i), level.logic.anim_delta.get(&i)) { (Some(d), _) => (*d, None), (None, Some(d)) => (*d, Some(0.0)), _ => (level.logic.anim_on[&i], Some(0.0)) };
        let rot_g = quat4(inst.rotation);
        let ambient = v3(inst.ambent_light_colour);
        let lights = [(v3(inst.light_vector1), v3(inst.light_colour1)), (v3(inst.light_vector2), v3(inst.light_colour2)), (v3(inst.light_vector3), v3(inst.light_colour3))];
        let pos = g2b(inst.location.into());
        let rot = basis * rot_g * basis.inverse();
        let root = commands.spawn((Name::new(inst.instance_name.clone()), LevelEntity, Transform { translation: pos, rotation: rot, scale: Vec3::from(inst.scale).abs() }, Visibility::Inherited)).id();
        let mut parts = Vec::new();
        for obj in &model.model_objects {
            // each part's mesh in its own frame (game axes -> Bevy, metres); lit as it rests
            let mut bufs: HashMap<String, MeshBuf> = HashMap::new();
            for part in obj.mesh_data.iter().flatten() {
                let Some(mat) = set.materials.get(part.material_id.max(0) as usize) else { continue };
                let src = load_obj(&set.dir.join("Meshes").join(&part.mesh_path));
                let buf = bufs.entry(mat.texture_path.clone()).or_default();
                for ((p, n), uv) in src.positions.iter().zip(&src.normals).zip(&src.uvs) {
                    let n_world = (rot_g * Vec3::from(*n)).normalize_or(Vec3::Z);
                    let mut c = ambient;
                    for (dir, colour) in lights { if dir.length_squared() > 1e-6 { c += colour * n_world.dot(dir.normalize()).max(0.0); } }
                    let c = (c / 128.0).clamp(Vec3::ZERO, Vec3::splat(2.0));
                    buf.pos.push(g2b(Vec3::from(*p)).into());
                    buf.nrm.push([n[0], n[2], -n[1]]);
                    buf.uv.push(*uv);
                    buf.col.push([c.x.powf(2.2), c.y.powf(2.2), c.z.powf(2.2), 1.0]);
                }
            }
            let child = commands.spawn((Transform::IDENTITY, Visibility::Inherited, ChildOf(root))).with_children(|p| {
                for (name, buf) in bufs {
                    let mat = mats.entry(name.clone()).or_insert_with(|| materials.add(material(tex.get(&name, images).as_ref(), true))).clone();
                    p.spawn((Mesh3d(meshes.add(buf.build())), MeshMaterial3d(mat)));
                }
            }).id();
            parts.push(child);
        }
        // the clock (cAnimObjectNode): frames / 30; rate in frames a second / 30
        let len = if def.end < 0.0 { model.anim_time } else { def.end } / 30.0;
        let start = def.start.max(0.0) / 30.0;
        let rate = if def.rate2 != 0.0 { def.rate + (def.rate2 - def.rate) * rnd() } else { def.rate } / 30.0;
        let t = if def.random_start { start + (len - start).max(0.0) * rnd() } else if def.reverse { len } else { start };
        out.0.push(AnimInst { def, model: inst.model_id as usize, len, t, dir: if def.reverse { -1.0 } else { 1.0 }, rate, parts, prop: None, inst: i, budget });
    }
    // props with a keyframed part (the spinning multiplier gems) turn whole with it
    for (pi, p) in props.0.iter().enumerate() {
        let Some(def) = level.logic.anims.get(&p.inst).copied() else { continue };
        let Some(inst) = level.instances.get(p.inst) else { continue };
        let Some(model) = set.models.get(inst.model_id.max(0) as usize) else { continue };
        if !model.model_objects.iter().any(|o| o.animation.is_some()) { continue; }
        let len = if def.end < 0.0 { model.anim_time } else { def.end } / 30.0;
        out.0.push(AnimInst { def, model: inst.model_id as usize, len, t: def.start.max(0.0) / 30.0 + rnd() * len, dir: 1.0, rate: def.rate / 30.0, parts: Vec::new(), prop: Some(pi), inst: p.inst, budget: None });
    }
    println!("keyframed objects: {}", out.0.len());
    out
}

/// Run each keyframed object's clock and pose its parts: world_i = world_parent * local_i(t).
fn animate_objects(time: Res<SmoothDt>, game: Res<ui::Game>, level: Res<LevelRes>, mut objs: ResMut<AnimObjects>, mut q: Query<&mut Transform>, mut props: ResMut<Props>, mut coll: ResMut<World>) {
    if game.screen == ui::Screen::Paused { return; }
    let dt = time.dt.min(0.1);
    // game (cm, Z up) -> Bevy (m, Y up) as a change of basis
    let a = Mat4::from_cols(Vec4::new(0.01, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, -0.01, 0.0), Vec4::new(0.0, 0.01, 0.0, 0.0), Vec4::W);
    let a_inv = a.inverse();
    for o in objs.0.iter_mut() {
        let start = o.def.start.max(0.0) / 30.0;
        let step = match o.budget.as_mut() {
            Some(b) if *b > 0.0 => { let s = (o.rate * dt).abs().min(*b); *b -= s; s }
            Some(_) => 0.0,
            None => o.rate * dt,
        };
        o.t += step * o.dir;
        match o.def.mode {
            0 => o.t = o.t.clamp(start, o.len.max(start)),
            2 => {
                if o.t > o.len { o.t = 2.0 * o.len - o.t; o.dir = -o.dir; }
                if o.t < start { o.t = 2.0 * start - o.t; o.dir = -o.dir; }
            }
            _ => { let span = (o.len - start).max(1e-3); if o.t > o.len || o.t < start { o.t = start + (o.t - start).rem_euclid(span); } }
        }
        let Some(model) = level.0.world.models.get(o.model) else { continue };
        // its collision (if it has any) goes where it is drawn
        if o.prop.is_none() { coll.0.set_mover_time(o.inst as u32, o.t); }
        if let Some(pi) = o.prop {
            // turn the whole prop about its animated part's pivot, as that part moves from rest
            let Some((k, an)) = model.model_objects.iter().enumerate().find_map(|(k, ob)| ob.animation.as_ref().map(|a| (k, a))) else { continue };
            let _ = k;
            let (rest, now) = (an.local(0.0), an.local(o.t));
            let delta = now * rest.inverse();
            let pivot = Vec3::new(an.u1, an.u2, an.u3);
            let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
            let (_, qg, _) = delta.to_scale_rotation_translation();
            let qb = basis * qg * basis.inverse();
            if let Some(p) = props.0.get_mut(pi) {
                if p.moving || p.hidden { continue; }
                let c = g2b(pivot);
                p.rot = p.home.1 * qb;
                p.pos = p.home.0 + p.home.1 * (c - qb * c);
                if std::env::var("TRICKY_ANIMDBG").is_ok() && pi % 20 == 0 { eprintln!("gem prop {pi} t {:.2} yaw {:.2} pos {:?}", o.t, p.rot.to_euler(EulerRot::YXZ).0, p.pos); }
            }
            continue;
        }
        let mut world: Vec<Mat4> = Vec::with_capacity(model.model_objects.len());
        for (k, obj) in model.model_objects.iter().enumerate() {
            let local = match &obj.animation {
                Some(an) => an.local(o.t),
                None => Mat4::from_scale_rotation_translation(obj.scale.map(Vec3::from).unwrap_or(Vec3::ONE), obj.rotation.map(quat4).unwrap_or(Quat::IDENTITY), obj.position.map(Vec3::from).unwrap_or(Vec3::ZERO)),
            };
            let w = if obj.parent_id >= 0 && (obj.parent_id as usize) < k { world[obj.parent_id as usize] * local } else { local };
            world.push(w);
            if let Some(Ok(mut tf)) = o.parts.get(k).map(|e| q.get_mut(*e)) {
                *tf = Transform::from_matrix(a * w * a_inv);
                if std::env::var("TRICKY_ANIMDBG").is_ok() && obj.animation.is_some() { eprintln!("anim {} part {k} t {:.2} at {:?} rot {:?}", model.model_name, o.t, tf.translation, tf.rotation.to_euler(EulerRot::XYZ)); }
            }
        }
    }
}

/// Knockable objects get their own entity (and mesh in their own frame) so they can move.
fn spawn_props(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, wa: &mut worldanim::WorldAnim,
) -> Props {
    let set = &level.world;
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut mats: HashMap<String, Handle<StandardMaterial>> = HashMap::new();
    let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let mut props: Vec<Prop> = Vec::new();
    // instance index -> prop index, so a broken thing can find its pieces
    let mut prop_of: HashMap<usize, usize> = HashMap::new();
    let mut links: Vec<(usize, Vec<usize>)> = Vec::new();
    // the level scripts' spline riders: their splines in Bevy space
    let mut movers = Movers(Vec::new());
    let mut mover_of: HashMap<usize, usize> = HashMap::new();
    let mut defs: Vec<(&usize, &logic::MoverDef)> = level.logic.movers.iter().collect();
    defs.sort_by_key(|d| *d.0);
    for (inst, def) in defs {
        let Some(sp) = level.splines.get(def.spline) else { continue };
        let mut pts: Vec<Vec3> = Vec::new();
        for seg in sp.segments.iter().filter(|s| s.points.len() >= 4) {
            for i in 0..=8 {
                let p = g2b(cubic(&seg.points, i as f32 / 8.0));
                if pts.last().is_none_or(|l| l.distance(p) > 0.01) { pts.push(p); }
            }
        }
        if pts.len() < 2 { continue; }
        let mut cum = vec![0.0];
        for w in pts.windows(2) { cum.push(cum.last().unwrap() + (w[1] - w[0]).length()); }
        mover_of.insert(*inst, movers.0.len());
        movers.0.push(MoverRt { def: *def, pts, cum, t: 0.0 });
    }
    for (inst_index, inst) in level.instances.iter().enumerate() {
        let switchable = level.switchable.contains(&inst_index) && !level.is_dynamic(inst_index);
        if !(level.is_dynamic(inst_index) || switchable) || inst.model_id < 0 { continue; }
        let piece = level.pieces.contains(&inst_index);
        if !inst.visable && !piece { continue; }
        let name = &inst.instance_name;
        let kind = if switchable { Kind::Static } else if piece { Kind::Piece } else if level.touch.contains_key(&inst_index) {
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
        // one mesh per texture (per material when its textures flip or scroll), in the object's
        // own frame (already in Bevy axes and metres)
        let own = wa.owns(level, inst_index);
        let mut bufs: HashMap<(String, usize), MeshBuf> = HashMap::new();
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for (obj, obj_m) in model.model_objects.iter().zip(set.object_matrices(model)) {
            let m = Mat4::from_scale(inst.scale.into()) * obj_m;
            for part in obj.mesh_data.iter().flatten() {
                let Some(mat) = set.materials.get(part.material_id.max(0) as usize) else { continue };
                let src = load_obj(&set.dir.join("Meshes").join(&part.mesh_path));
                let buf = bufs.entry((mat.texture_path.clone(), if own { part.material_id.max(0) as usize } else { usize::MAX })).or_default();
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
        let parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>)> = bufs.into_iter().map(|((name, mid), buf)| {
            let make = |tex: &mut TexCache, images: &mut Assets<Image>| {
                let t = tex.get(&name, images);
                let mut m = material(t.as_ref(), true);
                if glassy && t.as_ref().is_some_and(|t| t.has_alpha) { m.alpha_mode = AlphaMode::Blend; }
                m
            };
            let mat = if mid != usize::MAX {
                let base = make(&mut tex, images);
                wa.material(level, inst_index, mid, glassy, base, materials, &mut tex, images)
            } else {
                mats.entry(format!("{name}{glassy}")).or_insert_with(|| materials.add(make(&mut tex, images))).clone()
            };
            (meshes.add(buf.build()), mat)
        }).collect();
        // a spline rider (cSplinePathNode) is drawn once per copy
        let mover = mover_of.get(&inst_index).copied();
        let copies = mover.map_or(1, |m| movers.0[m].def.count);
        for c in 0..copies {
            let index = props.len();
            let kind = match mover { Some(m) => Kind::Mover(m, c), None => kind.clone() };
                let mut prop = Prop::with_box(kind, pos, rot, lo, hi);
            prop.inst = inst_index;
            prop.debris = level.logic.debris.get(&inst_index).copied();
            prop.stays = level.logic.pickups.contains_key(&inst_index) && !level.logic.self_hide.contains(&inst_index);
            // cracked glass breaks when its script says so (worldanim), not on first touch
            prop.scripted = level.logic.cracks.contains_key(&inst_index);
            // path markers break into pieces that fly off (cMeshAnimNode); crash bags and cans roll
            if inst.instance_name.contains("PathMarker") { prop.ballistic = true; prop.mass = 0.0; }
            props.push(prop);
            commands.spawn((Name::new(inst.instance_name.clone()), LevelEntity, PropVisual(index),
                Transform { translation: pos, rotation: rot, ..default() }, if hidden { Visibility::Hidden } else { Visibility::Inherited })).with_children(|p| {
                for (mesh, mat) in &parts { p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()))); }
            });
        }
    }
    for (prop, linked) in links {
        let list: Vec<usize> = linked.iter().filter_map(|i| prop_of.get(i).copied()).filter(|p| props[*p].kind == Kind::Piece).collect();
        if let Kind::Touch { pieces, .. } = &mut props[prop].kind { *pieces = list; }
    }
    let count = |f: fn(&Kind) -> bool| props.iter().filter(|p| f(&p.kind)).count();
    println!("knockable objects: {}, breakable or pickups: {}, broken pieces: {}",
        count(|k| *k == Kind::Knock), count(|k| matches!(k, Kind::Touch { .. })), count(|k| *k == Kind::Piece));
    commands.insert_resource(movers);
    Props(props)
}

/// A spline rider at run time: its spline (Bevy space), the length along it, and how far it has gone.
struct MoverRt { def: logic::MoverDef, pts: Vec<Vec3>, cum: Vec<f32>, t: f32 }
#[derive(Resource, Default)]
struct Movers(Vec<MoverRt>);

/// Trains and lift chairs along their splines (`cSplinePathNode` update): distance += speed (m/s);
/// the copies spaced evenly; facing along the spline (yaw, and pitch in orientation 0).
fn move_movers(time: Res<SmoothDt>, game: Res<ui::Game>, mut movers: ResMut<Movers>, mut props: ResMut<Props>, mut gizmos: Gizmos) {
    // ski lift cables
    for m in &movers.0 { if let Some(c) = m.def.cable { gizmos.linestrip(m.pts.iter().copied(), Color::srgb(c[0], c[1], c[2])); } }
    if game.screen == ui::Screen::Paused { return; }
    let dt = time.dt.min(0.1);
    for m in movers.0.iter_mut() { m.t += m.def.speed * dt; }
    let basis = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for p in props.0.iter_mut() {
        let props::Kind::Mover(mi, c) = p.kind else { continue };
        let Some(m) = movers.0.get(mi) else { continue };
        let len = *m.cum.last().unwrap();
        if len <= 0.0 { continue; }
        let raw = m.t + len * c as f32 / m.def.count as f32;
        let (d, back) = match m.def.mode {
            0 => (raw.min(len), false),
            2 => { let k = raw.rem_euclid(2.0 * len); if k > len { (2.0 * len - k, true) } else { (k, false) } }
            _ => (raw.rem_euclid(len), false),
        };
        let i = m.cum.partition_point(|x| *x <= d).clamp(1, m.pts.len() - 1);
        let (a, b) = (m.pts[i - 1], m.pts[i]);
        let f = (d - m.cum[i - 1]) / (m.cum[i] - m.cum[i - 1]).max(1e-5);
        p.pos = a.lerp(b, f);
        // the tangent in game axes (x, y, z up) from Bevy (x, y up, -z)
        let tb = (b - a).normalize_or(Vec3::X);
        let t = Vec3::new(tb.x, -tb.z, tb.y);
        let mut yaw = m.def.yaw + std::f32::consts::FRAC_PI_2 - t.y.atan2(t.x);
        if back { yaw += std::f32::consts::PI; }
        let rot_g = match m.def.orient {
            0 => Quat::from_rotation_z(-yaw) * Quat::from_rotation_x(if back { t.z } else { -t.z }),
            1 => Quat::from_rotation_z(-yaw),
            _ => Quat::from_rotation_z(-(m.def.yaw + std::f32::consts::FRAC_PI_2) + if back { std::f32::consts::PI } else { 0.0 }),
        };
        p.rot = basis * rot_g * basis.inverse();
    }
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
    mut cam: ResMut<CamRes>, lib: Res<CharLib>, mut game: ResMut<ui::Game>, mut reload: ResMut<editor::Reload>,
) {
    let step = (keys.just_pressed(KeyCode::BracketRight) || keys.just_pressed(KeyCode::PageDown)) as i32
        - (keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::PageUp)) as i32 + std::mem::take(&mut game.track_step);
    // the editor asks for the same track again after saving
    let again = std::mem::take(&mut reload.0);
    if !again && (step == 0 || list.dirs.len() < 2) { return; }
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
    commands.insert_resource(RaceRes(Race { course: loaded.course, lines: loaded.lines, paths: loaded.paths, tube: loaded.tube, cml: loaded.cml, stage: loaded.stage, finish_area: loaded.finish_area, ..Race::new(loaded.line) }));
    commands.insert_resource(LevelRes(loaded.level));
    let mode = cam.0.mode;
    cam.0 = ChaseCam::default();
    cam.0.mode = mode;
    game.lineup = true;
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
    let mut ramps: Chunks = HashMap::new();
    let n = PATCH_SEGS;
    for p in &level.patches {
        if p.points.len() < 16 { continue; }
        let chunks = if p.trick_only_patch { &mut ramps } else { &mut chunks };
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
    // the show-off ramps are only there in show-off events
    let shown = spawn_chunks(commands, "Terrain", chunks, &mut tex, meshes, images, materials);
    shown + spawn_chunks_with(commands, "ShowOffRamp", ramps, &mut tex, meshes, images, materials, (TrickOnly, Visibility::Hidden))
}
#[derive(Component, Clone, Copy)]
struct TrickOnly;
/// Show-off ramps (trick-only patches) appear, and are solid, only in show-off events.
/// The level scripts' boosts (`cBoostNode`: speed along the pad's direction pulled up to its target
/// at its rate, never slowed) and teleports (op 0x18). Returns true on a teleport.
fn apply_pads(world: &CollisionWorld, r: &mut Rider, dt: f32) -> bool {
    let body = r.pos + Vec3::Y * 0.8;
    for p in &world.pads {
        if body.cmplt(p.lo - 0.3).any() || body.cmpgt(p.hi + 0.3).any() { continue; }
        match p.kind {
            collide::PadKind::Boost { dir, target, gain } => {
                let d = target - r.vel.dot(dir);
                if d > 0.0 && r.crashed <= 0.0 { r.vel += dir * (d * gain * dt).min(d); }
            }
            collide::PadKind::Teleport { to, yaw } => { r.respawn((to, yaw)); return true; }
            collide::PadKind::Pickup { mult, speed, spin } => {
                if let Some(m) = mult { if r.multiplier < m as u32 { r.pick_multiplier(m as u32); } }
                if let Some(s) = speed { if r.speed_timer < s - 0.5 { r.pick_speed(s); } }
                if let Some(s) = spin { if r.spin_timer < s - 0.5 { r.pick_spin(s); } }
            }
        }
    }
    false
}

fn trick_only_ramps(game: Res<ui::Game>, mut world: ResMut<World>, mut rails: ResMut<RailsRes>, mut props: ResMut<Props>, mut q: Query<&mut Visibility, With<TrickOnly>>, mut last: Local<Option<(ui::Event, usize)>>) {
    let on = game.event == ui::Event::ShowOff;
    if world.0.showoff != on { world.0.showoff = on; }
    // the event's start-up scripts (RaceMode / ShowoffMode / FreerideMode): race-only or show-off-only
    // objects, the start gate in free ride, and the rails each event switches
    let k = match game.event { ui::Event::Race => 0, ui::Event::ShowOff => 1, ui::Event::FreeRide => 2 };
    let key = (game.event, world.0.len());
    if *last != Some(key) {
        *last = Some(key);
        world.0.off = world.0.mode_off[k].clone();
        let sw = rails.0.1[k].clone();
        for r in rails.0.0.iter_mut() { r.on = sw.get(&r.spline).copied().unwrap_or(true); }
    }
    for p in props.0.iter_mut().filter(|p| p.kind == props::Kind::Static) { p.hidden = world.0.mode_off[k].contains(&(p.inst as u32)); }
    for mut v in &mut q { let want = if on { Visibility::Inherited } else { Visibility::Hidden }; if *v != want { *v = want; } }
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

/// One static instance's meshes in Bevy space with its lights baked in: (material, mesh file, mesh).
fn bake_instance(set: &ModelSet, inst: &Instance, objs: &mut HashMap<String, ObjMesh>) -> Vec<(usize, String, MeshBuf)> {
    let mut out = Vec::new();
    let Some(model) = set.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0) else { return out };
    let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
    let lights = [
        (v3(inst.light_vector1), v3(inst.light_colour1)),
        (v3(inst.light_vector2), v3(inst.light_colour2)),
        (v3(inst.light_vector3), v3(inst.light_colour3)),
    ];
    let ambient = v3(inst.ambent_light_colour);
    for (obj, obj_m) in model.model_objects.iter().zip(set.object_matrices(model)) {
        let Some(parts) = &obj.mesh_data else { continue };
        let m = inst_m * obj_m;
        let normal_m = Mat3::from_mat4(m).inverse().transpose();
        for part in parts {
            let src = objs.entry(part.mesh_path.clone()).or_insert_with(|| load_obj(&set.dir.join("Meshes").join(&part.mesh_path)));
            if src.positions.is_empty() || set.materials.get(part.material_id.max(0) as usize).is_none() { continue; }
            let mut buf = MeshBuf::default();
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
            out.push((part.material_id.max(0) as usize, part.mesh_path.clone(), buf));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn spawn_instances(
    commands: &mut Commands, level: &Level,
    meshes: &mut Assets<Mesh>, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>, wa: &mut worldanim::WorldAnim,
) -> usize {
    let set = &level.world;
    let mut tex = TexCache::new(set.dir.join("Textures"));
    let mut objs: HashMap<String, ObjMesh> = HashMap::new();
    let mut chunks: Chunks = HashMap::new();
    for (index, inst) in level.instances.iter().enumerate() {
        if !inst.visable || inst.model_id < 0 || level.is_dynamic(index) || level.switchable.contains(&index) || animated(level, index) { continue; }
        let parts = bake_instance(set, inst, &mut objs);
        // flipped, scrolled or waving: drawn on its own with its own materials
        if wa.owns(level, index) { wa.spawn_static(commands, level, index, parts, meshes, images, materials, &mut tex); continue; }
        let (cx, cz) = chunk_of(g2b(inst.location.into()));
        for (mid, _, buf) in parts {
            chunks.entry((set.materials[mid].texture_path.as_str(), true, cx, cz)).or_default().append(buf);
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
fn screenshot(mut commands: Commands, shot: Option<ResMut<Shot>>, mut exit: EventWriter<AppExit>, race: Res<RaceRes>, mut armed: Local<bool>) {
    let Some(mut shot) = shot else { return };
    shot.frame += 1;
    // TRICKY_SHOTAT=seconds: wait until the race clock reaches that time
    let secs: Option<f32> = std::env::var("TRICKY_SHOTAT").ok().and_then(|v| v.parse().ok());
    if let Some(t) = secs { if !*armed { if race.0.time >= t { *armed = true; shot.frame = 30; } else { shot.frame = 0; } } }
    let at = std::env::var("TRICKY_SHOTFRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    if shot.frame == at { commands.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(&shot.path))); }
    if shot.frame == at + 15 { exit.write(AppExit::Success); }
}

// ---------------------------------------------------------------- riding

/// An object's collision meshes, each with the model part it moves with (`World_RayCastInstance`):
/// mode 1 has a collision mesh per model part that has a mesh, in the parts' order; mode 3 collides
/// with the drawn meshes; mode 2 is a box test (not solid here yet).
fn collision_parts(level: &Level, inst: &Instance) -> Vec<(Option<usize>, PathBuf)> {
    let model = level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0);
    match inst.collsion_mode {
        1 => {
            let with_mesh: Vec<usize> = model.map(|m| m.model_objects.iter().enumerate().filter(|(_, o)| o.mesh_data.as_ref().is_some_and(|v| !v.is_empty())).map(|(k, _)| k).collect()).unwrap_or_default();
            inst.collsion_model_paths.iter().flatten().enumerate().map(|(j, p)| (with_mesh.get(j).copied(), level.dir.join("Collision").join(p))).collect()
        }
        3 => model.map(|m| m.model_objects.iter().enumerate().flat_map(|(k, o)| o.mesh_data.iter().flatten().map(move |part| (Some(k), level.dir.join("Meshes").join(&part.mesh_path)))).collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// A keyframed object's collision, kept in its parts' frames (see `collide::Mover`), with its own
/// clock for a run without the scripts: the always-running clips run, a clip a touch plays waits
/// for its trigger, AnimDelta clips (given time by the scripts) stay.
fn make_mover(level: &Level, index: usize, parts: &[(Option<usize>, PathBuf)], surf: u8, kind: collide::ObjKind, inst_m: Mat4, cache: &mut HashMap<String, ObjMesh>) -> Option<collide::Mover> {
    let inst = level.instances.get(index)?;
    let model = level.world.models.get(usize::try_from(inst.model_id).ok()?)?;
    let mut local: Vec<(usize, Vec<[Vec3; 3]>)> = Vec::new();
    for (k, key) in parts {
        let mesh = cache.entry(key.to_string_lossy().to_string()).or_insert_with(|| load_obj(key));
        local.push((k.unwrap_or(0), mesh.positions.as_chunks::<3>().0.iter().map(|t| [Vec3::from(t[0]), Vec3::from(t[1]), Vec3::from(t[2])]).collect()));
    }
    let objs = model.model_objects.iter().map(|o| collide::MoverObj {
        parent: o.parent_id,
        rest: Mat4::from_scale_rotation_translation(o.scale.map(Vec3::from).unwrap_or(Vec3::ONE), o.rotation.map(quat4).unwrap_or(Quat::IDENTITY), o.position.map(Vec3::from).unwrap_or(Vec3::ZERO)),
        anim: o.animation.clone(),
    }).collect();
    let logic = &level.logic;
    let always = logic.anims.contains_key(&index);
    let clip = logic.anims.get(&index).or(logic.anim_delta.get(&index)).or(logic.anim_on.get(&index)).map(|d| {
        let len = if d.end < 0.0 { model.anim_time } else { d.end } / 30.0;
        let start = d.start.max(0.0) / 30.0;
        let rate = if logic.anim_delta.contains_key(&index) && !always { 0.0 } else { d.rate / 30.0 };
        collide::Clip { mode: d.mode, start, len, rate, dir: if d.reverse { -1.0 } else { 1.0 }, running: always }
    });
    let t = clip.map_or(0.0, |c| if c.dir < 0.0 { c.len } else { c.start });
    Some(collide::Mover::new(index as u32, surf, kind, inst_m, objs, local, clip, t))
}

/// Everything the rider can touch: the terrain (same tessellation as what is drawn) and the
/// collision meshes of placed objects.
fn build_collision(level: &Level) -> CollisionWorld {
    let mut world = CollisionWorld::default();
    world.owner = u32::MAX;
    world.inst = u32::MAX;
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
                    world.add_ex([grid[t[0]].0, grid[t[1]].0, grid[t[2]].0], Some([grid[t[0]].1, grid[t[1]].1, grid[t[2]].1]), true, p.surface_type.clamp(0, 19) as u8, p.trick_only_patch);
                }
            }
        }
    }
    // Objects. Collision mode 1 has its own collision mesh; mode 3 (start/finish platforms, crash
    // bags, signs, snow cats) collides with the mesh that is drawn. Mode 2 (stands, billboards,
    // pickups) is a bounding-box test in the game and is not solid here yet. Water is not solid.
    let mut cache: HashMap<String, ObjMesh> = HashMap::new();
    let who = std::env::var("TRICKY_WHO").ok().map(|v| v.split(',').filter_map(|x| x.parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 3).map(|v| Vec3::new(v[0], v[1], v[2]));
    // megaplex's glass floors are solid until they break (each piece its own owner, so its
    // triangles drop out when the scripts kill it); iris doors put a rider back only while shut
    let glass = level.glass();
    let gates = level.gates();
    for (index, inst) in level.instances.iter().enumerate() {
        // the start gate's cover is the barrier that holds riders until "GO"; we start with it open
        // triggers and emitters are not obstacles; reset zones put the rider back instead of stopping him
        let name = inst.instance_name.to_lowercase();
        if !inst.player_collision || name.contains("startgate") || name.contains("trigger") || name.contains("emitter") || (level.is_dynamic(index) && !glass.contains(&index)) { continue; }
        // objects the event scripts switch on and off keep their triangles apart
        world.owner = if level.switchable.contains(&index) || glass.contains(&index) || gates.contains(&index) { index as u32 } else { u32::MAX };
        world.inst = index as u32;
        let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
        let model = level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0);
        let obj_ms = model.map(|m| level.world.object_matrices(m)).unwrap_or_default();
        let at = |k: Option<usize>| k.and_then(|k| obj_ms.get(k)).map_or(inst_m, |m| inst_m * *m);
        let parts = collision_parts(level, inst);
        // a reset zone by name, or anything whose contact script puts the rider back (op 0xd)
        if name.contains("resetzone") || name.contains("_reset_") || name.contains("crowdtrap") || level.logic.resets.contains(&index) {
            let mut parts = parts;
            if parts.is_empty() {
                // no collision mesh of its own: the drawn mesh is the zone
                if let Some(model) = model {
                    for (k, obj) in model.model_objects.iter().enumerate() {
                        for part in obj.mesh_data.iter().flatten() { parts.push((Some(k), level.dir.join("Meshes").join(&part.mesh_path))); }
                    }
                }
            }
            // (one that moves, megaplex's rotating door, moves its zone with it)
            if animated(level, index) && !gates.contains(&index) && !parts.is_empty() {
                if let Some(mut m) = make_mover(level, index, &parts, 0, collide::ObjKind::Ghost, inst_m, &mut cache) { m.reset = true; world.movers.push(m); continue; }
            }
            for (k, key) in parts {
                let m = at(k);
                let mesh = cache.entry(key.to_string_lossy().to_string()).or_insert_with(|| load_obj(&key));
                for t in mesh.positions.as_chunks::<3>().0 {
                    let v = |i: usize| g2b(m.transform_point3(Vec3::from(t[i])));
                    world.add_reset([v(0), v(1), v(2)]);
                }
            }
            continue;
        }
        // what the object is to a rider (`World_RayCast` / `World_QueryInstances`): ground with its
        // surface type, or (-1) something to bounce off; with no bounce (or no mass) it is not solid
        // (glass keeps its triangles so that touching it cracks it)
        let typed = inst.surface_type >= 0;
        let kind = if typed { collide::ObjKind::Ground } else if inst.player_bounce && inst.u0 != 0.0 { collide::ObjKind::Bounce } else { collide::ObjKind::Ghost };
        if kind == collide::ObjKind::Ghost && !glass.contains(&index) { continue; }
        let surf = if typed { inst.surface_type.clamp(0, 19) as u8 } else { 10 };
        if inst.collsion_mode == 3 && inst.instance_name.contains("Water") { continue; }
        // a keyframed object: its collision follows the clip
        if animated(level, index) && !gates.contains(&index) && kind != collide::ObjKind::Ghost && !parts.is_empty() {
            if let Some(m) = make_mover(level, index, &parts, surf, kind, inst_m, &mut cache) { world.movers.push(m); continue; }
        }
        let mut bounds = (Vec3::MAX, Vec3::MIN);
        for (k, path) in parts {
            let m = at(k);
            let key = path.to_string_lossy().to_string();
            let mesh = cache.entry(key).or_insert_with(|| load_obj(&path));
            for t in mesh.positions.as_chunks::<3>().0 {
                let v = |i: usize| g2b(m.transform_point3(Vec3::from(t[i])));
                for k in 0..3 { bounds.0 = bounds.0.min(v(k)); bounds.1 = bounds.1.max(v(k)); }
                world.add_obj([v(0), v(1), v(2)], surf, kind);
            }
        }
        // TRICKY_WHO=x,y,z names every solid object whose box contains that point
        if let Some(p) = who { if p.cmpge(bounds.0 - 0.6).all() && p.cmple(bounds.1 + 0.6).all() { println!("solid here: {} (mode {}, model {}) box {:?} .. {:?}", inst.instance_name, inst.collsion_mode, inst.model_id, bounds.0, bounds.1); } }
    }
    world.owner = u32::MAX;
    world.inst = u32::MAX;
    world.tick_movers(0.0);
    if std::env::var("TRICKY_MOVERDBG").is_ok() {
        for (i, inst) in level.instances.iter().enumerate() {
            let Some(model) = level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0) else { continue };
            if !inst.player_collision || !model.model_objects.iter().any(|o| o.animation.is_some()) { continue; }
            println!("keyframed {i} {}: reset {} animated {} anims {} on {} delta {} dynamic {} switchable {} gate {} mover {}", inst.instance_name, level.logic.resets.contains(&i), animated(level, i), level.logic.anims.contains_key(&i), level.logic.anim_on.contains_key(&i), level.logic.anim_delta.contains_key(&i), level.is_dynamic(i), level.switchable.contains(&i), level.is_gate(i), world.movers.iter().any(|m| m.inst == i as u32));
        }
        // and each moving object's box as it goes (its own clock, 0.25 s steps for 3 s; a clip a
        // touch plays is played)
        let mut w2 = std::mem::take(&mut world.movers);
        for m in w2.iter_mut() {
            m.play();
            let name = &level.instances[m.inst as usize].instance_name;
            let loc = g2b(level.instances[m.inst as usize].location.into());
            let mut row = format!("mover {} {} at {:.1?} ({} tris, {:?}):", m.inst, name, loc, m.tri_count(), m.kind);
            for _ in 0..12 {
                let mut tmp = CollisionWorld::default();
                tmp.movers.push(std::mem::replace(m, collide::Mover::new(0, 0, collide::ObjKind::Ghost, Mat4::IDENTITY, Vec::new(), Vec::new(), None, 0.0)));
                tmp.tick_movers(0.25);
                *m = tmp.movers.pop().unwrap();
                let (lo, hi) = m.bounds();
                row += &format!(" [{:.1} {:.1?}..{:.1?}]", m.t, lo, hi);
            }
            println!("{row}");
        }
        world.movers = w2;
    }
    println!("moving collision: {} keyframed objects, {} triangles", world.movers.len(), world.movers.iter().map(|m| m.tri_count()).sum::<usize>());
    for k in 0..3 { world.mode_off[k] = level.logic.hide[k].iter().map(|i| *i as u32).collect(); }
    // the scripts' boosts and teleports act on whoever is inside the object's box
    let mut boxes = |i: usize| -> Option<(Vec3, Vec3)> {
        let inst = level.instances.get(i)?;
        let inst_m = Mat4::from_scale_rotation_translation(inst.scale.into(), quat4(inst.rotation), inst.location.into());
        let mut b = (Vec3::MAX, Vec3::MIN);
        let mut parts: Vec<(PathBuf, Mat4)> = inst.collsion_model_paths.iter().flatten().map(|p| (level.dir.join("Collision").join(p), inst_m)).collect();
        if let Some(model) = level.world.models.get(inst.model_id.max(0) as usize).filter(|_| inst.model_id >= 0) {
            for (obj, obj_m) in model.model_objects.iter().zip(level.world.object_matrices(model)) {
                for part in obj.mesh_data.iter().flatten() { parts.push((level.dir.join("Meshes").join(&part.mesh_path), inst_m * obj_m)); }
            }
        }
        for (key, m) in parts {
            let mesh = cache.entry(key.to_string_lossy().to_string()).or_insert_with(|| load_obj(&key));
            for p in &mesh.positions { let v = g2b(m.transform_point3(Vec3::from(*p))); b.0 = b.0.min(v); b.1 = b.1.max(v); }
        }
        (b.0.x <= b.1.x).then_some(b)
    };
    for (&i, bd) in &level.logic.boosts {
        if let Some((lo, hi)) = boxes(i) {
            let d = Vec3::from(bd.dir);
            world.pads.push(collide::Pad { lo, hi, kind: collide::PadKind::Boost { dir: Vec3::new(d.x, d.z, -d.y).normalize_or_zero(), target: bd.target * 0.01, gain: bd.gain } });
        }
    }
    // multiplier gems, speed and spin boosts (the gems stay; touching again does nothing more)
    if std::env::var("TRICKY_LOGICDBG").is_ok() { eprintln!("pickups in scripts: {} (dynamic {})", level.logic.pickups.len(), level.logic.pickups.keys().filter(|i| level.is_dynamic(**i)).count()); }
    for (&i, &(mult, speed, spin)) in &level.logic.pickups {
        if level.is_dynamic(i) { continue; }
        if let Some((lo, hi)) = boxes(i) { world.pads.push(collide::Pad { lo, hi, kind: collide::PadKind::Pickup { mult, speed, spin } }); }
    }
    for &i in level.logic.emit_touch.keys() {
        if let Some(b) = boxes(i) { world.emit_boxes.insert(i, b); }
    }
    // the doors' clips, and the boxes whose touch plays them
    for &i in &gates {
        if let Some((len, open)) = gate_window(level, i) {
            world.gates.push(collide::Gate { inst: i as u32, len, open, t: None });
            for &(trig, _) in level.logic.anim_triggers.iter().filter(|t| t.1 == i) {
                if let Some((lo, hi)) = boxes(trig) { world.gate_triggers.push((lo, hi, i as u32)); }
            }
        }
    }
    // the boxes whose touch plays a keyframed object's clip (flippers, ramps)
    for &(trig, target) in &level.logic.anim_triggers {
        if gates.contains(&target) || !world.movers.iter().any(|m| m.inst == target as u32) { continue; }
        if let Some((lo, hi)) = boxes(trig) { world.mover_triggers.push((lo, hi, target as u32)); }
    }
    for (&i, &to) in &level.logic.teleports {
        let (Some((lo, hi)), Some(t)) = (boxes(i), level.instances.get(to)) else { continue };
        let fwd = quat4(t.rotation) * Vec3::Y;
        let f = Vec3::new(fwd.x, fwd.z, -fwd.y);
        world.pads.push(collide::Pad { lo, hi, kind: collide::PadKind::Teleport { to: g2b(t.location.into()), yaw: f32::atan2(-f.x, -f.z) } });
    }
    world
}

/// When a door's clip (played once from the start) holds it open: the clip's length and the
/// stretch where its moving parts are at least half way out (s).
fn gate_window(level: &Level, i: usize) -> Option<(f32, (f32, f32))> {
    let inst = level.instances.get(i)?;
    let def = level.logic.anim_on.get(&i)?;
    let model = level.world.models.get(usize::try_from(inst.model_id).ok()?)?;
    let len = if def.end < 0.0 { model.anim_time } else { def.end } / 30.0;
    let start = def.start.max(0.0) / 30.0;
    // how far each animated part's mesh has moved from where it rests
    let parts: Vec<(&level::ObjAnim, Vec<Vec3>)> = model.model_objects.iter().filter_map(|o| {
        let an = o.animation.as_ref()?;
        let pts = o.mesh_data.iter().flatten().flat_map(|m| load_obj(&level.dir.join("Meshes").join(&m.mesh_path)).positions.into_iter().map(Vec3::from)).collect();
        Some((an, pts))
    }).collect();
    let moved = |t: f32| parts.iter().map(|(an, pts)| {
        let d = an.local(t) * an.local(start).inverse();
        pts.iter().map(|p| (d.transform_point3(*p) - *p).length()).fold(0.0f32, f32::max)
    }).fold(0.0f32, f32::max);
    let n = ((len - start) * 30.0).ceil().max(1.0) as usize;
    let samples: Vec<(f32, f32)> = (0..=n).map(|k| { let t = start + (len - start) * k as f32 / n as f32; (t - start, moved(t)) }).collect();
    let most = samples.iter().map(|s| s.1).fold(0.0f32, f32::max);
    if most < 1.0 { return None; }
    let open: Vec<f32> = samples.iter().filter(|s| s.1 >= most * 0.5).map(|s| s.0).collect();
    Some((len - start, (*open.first()?, *open.last()? + 1.0 / 30.0)))
}

fn ride(
    time: Res<SmoothDt>, keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>,
    world: Res<World>, rails: Res<RailsRes>, mut rider: ResMut<RiderRes>, mut race: ResMut<RaceRes>,
    mut opponents: ResMut<Opponents>, lib: Res<CharLib>, mut props: ResMut<Props>, mut game: ResMut<ui::Game>, list: Res<LevelList>,
    mut skill_clock: Local<f32>, mut auto: Local<Option<rider::AiDriver>>, wa: Option<ResMut<worldanim::WorldAnim>>, mut restarted: Local<bool>,
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
        // the four shoulder buttons: J = L1, L = R1, U = L2, O = R2
        grab: keys.pressed(KeyCode::KeyJ) as u8 | (keys.pressed(KeyCode::KeyL) as u8) << 1 | (keys.pressed(KeyCode::KeyU) as u8) << 2 | (keys.pressed(KeyCode::KeyO) as u8) << 3,
        tweak: keys.pressed(KeyCode::KeyK) || key(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        boost: key(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        wind: None,
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
        input.tweak |= pad.pressed(GamepadButton::West);
        input.grab |= pad.pressed(GamepadButton::LeftTrigger) as u8 | (pad.pressed(GamepadButton::RightTrigger) as u8) << 1
            | (pad.pressed(GamepadButton::LeftTrigger2) as u8) << 2 | (pad.pressed(GamepadButton::RightTrigger2) as u8) << 3;
        restart |= pad.just_pressed(GamepadButton::Start) && playing && game.since > 0.3;
        unstick |= pad.just_pressed(GamepadButton::Select);
    }
    input.steer = input.steer.clamp(-1.0, 1.0);
    input.flip = input.flip.clamp(-1.0, 1.0);
    restart |= game.lineup;
    game.lineup = false;
    // TRICKY_RESTARTAT=s: restart the run once, s seconds into it (for checks)
    if !*restarted && std::env::var("TRICKY_RESTARTAT").ok().and_then(|v| v.parse::<f32>().ok()).is_some_and(|t| race.0.time >= t) { *restarted = true; restart = true; println!("restart at race time {:.1}", race.0.time); }
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
        // the level's scripted state too: glass whole, counters full, doors shut, animations as woken
        if let Some(mut wa) = wa { wa.restart = true; }
    }
    // a reset you ask for costs 0.12 of the meter (Score_ManualReset)
    if unstick { let at = r.safe; r.respawn(at); r.add_meter(-0.12); }
    // F: shove whoever is alongside. Knocking a rival down is worth boost, as in the original.
    let shove = keys.just_pressed(KeyCode::KeyF) || pads.iter().any(|p| p.just_pressed(GamepadButton::RightThumb) || p.just_pressed(GamepadButton::LeftThumb));
    if shove && playing && r.crashed <= 0.0 && race.0.state == RaceState::Running {
        let mut hit = false;
        r.shove = 0.7;
        let right = Vec3::new(r.yaw.cos(), 0.0, -r.yaw.sin());
        let nearest = opponents.0.iter().map(|o| o.rider.pos - r.pos).min_by(|a, b| a.length().total_cmp(&b.length()));
        r.shove_side = nearest.map_or(1.0, |d| if d.dot(right) >= 0.0 { 1.0 } else { -1.0 });
        // the shove reaches a rider within 1.8 m, 1.5 m up or down, and 55 degrees of where you face
        let face = heading(r.yaw);
        for o in opponents.0.iter_mut() {
            let d = o.rider.pos - r.pos;
            let flat = Vec3::new(d.x, 0.0, d.z);
            let ahead = flat.normalize_or_zero().dot(face).abs() < 55f32.to_radians().sin() || flat.length() < 0.6;
            if flat.length() < 1.8 && d.y.abs() < 1.5 && ahead && o.rider.crashed <= 0.0 {
                let push = props::shove_push(r, &o.rider);
                let was = o.rider.stumble > 0.0;
                let down = o.rider.take_hit(push, false);
                if down { hit = true; }
                // the rider remembers it (Rider_OnKnockedDownBy): 30 for a knockdown, 15 for a stumble
                let now = race.0.time;
                if let Some(g) = o.grudge.as_mut() { if down { g.hit(30.0, now); } else if !was && o.rider.stumble > 0.0 { g.hit(15.0, now); } }
            }
        }
        if hit {
            // a knockdown fills the meter, as in the original
            r.add_meter(1.0);
            r.last_trick = "KNOCKDOWN!".into();
            r.trick_timer = 2.5;
        }
    }
    // the player's signature moves come with the character
    if let Some(list) = lib.chars.get(lib.player).and_then(|c| c.ubers[(r.stats.kind as usize).min(2)].as_ref().or(c.ubers[0].as_ref())).map(|u| u.list()) {
        if r.ubers.len() != list.len() || r.ubers.first().map(|u| &u.0) != list.first().map(|u| &u.0) { r.ubers = list; r.uber_id = 0; }
    }
    // each rider's own attributes and trick set, from the game's tables
    let data_of = |c: usize| lib.chars.get(c).and_then(|e| trickdata::RIDERS.iter().find(|d| d.name.eq_ignore_ascii_case(&e.name))).unwrap_or(&trickdata::RIDERS[0]);
    let mine = data_of(lib.player);
    r.data = mine;
    let row_of = |d: &trickdata::RiderData| trickdata::RIDERS.iter().position(|x| std::ptr::eq(x, d)).unwrap_or(0);
    r.stats = rider::Stats::of(mine, game.training(), &trickdata::BOARDS[row_of(mine)][game.board.min(11)]);
    r.showoff = game.event == ui::Event::ShowOff;
    for o in opponents.0.iter_mut() {
        let d = data_of(o.character);
        o.rider.data = d;
        // rivals ride their first board as rookies and the best one as masters
        o.rider.stats = rider::Stats::of(d, game.training(), &trickdata::BOARDS[row_of(d)][if game.master { 11 } else { 0 }]);
    }
    if game.screen == ui::Screen::Menu { race.0.restart(); }
    // paused: nothing moves
    if game.screen == ui::Screen::Paused { return; }
    let waiting = race.0.state == RaceState::Countdown;
    if waiting { input = Input::default(); }
    if game.screen == ui::Screen::Results { input = Input { brake: true, ..Input::default() }; }
    // the original's catch-up (Race_UpdatePlacingsAndRubberBand / AI_RubberBandSpeedScale): each
    // rival's physics runs between 0.7x and 1.5x speed depending on how far it is from the player,
    // inside a lead window per starting slot; and its skill is re-picked every second by who leads
    if game.event == ui::Event::Race && !waiting && race.0.state == RaceState::Running {
        let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (off, end) = match track.as_str() {
            "gari" => (1532.37, 140005.72), "snowdream" => (907.67, 125085.09), "elysium" => (421.18, 159083.5),
            "mesablanca" => (-504.23, 138040.55), "merqury" => (503.47, 142028.88), "aloha" => (46.70, 131061.63),
            "megaplex" => (0.0, 40011.45), "alaska" => (0.0, 160036.53), _ => (0.0, 0.0),
        };
        let level = game.round.min(2);
        let (k, f) = [(0.6, 1.0), (0.4, 0.5), (0.35, 0.0)][level];
        const MAX: [f32; 6] = [5014.84, 4519.08, 4034.01, 4097.28, 3029.93, 2022.42];
        const MIN: [f32; 6] = [-1869.04, -2147.75, -2427.94, -2937.53, -3560.38, -4086.21];
        let pairs = [(0.7549, 0.4048), (0.9092, 0.7084), (1.0, 0.9513)][level];
        let course_len = race.0.remaining(0) * 100.0;
        let rem = race.0.remaining(race.0.idx) * 100.0;
        *skill_clock += time.dt;
        let repick = *skill_clock > 1.0;
        if repick { *skill_clock = 0.0; }
        let player_pos = r.pos;
        let player_dtf = race.0.prog.dtf;
        for (slot, o) in opponents.0.iter_mut().enumerate() {
            let to = player_pos - o.rider.pos;
            let dist = Vec3::new(to.x, 0.0, to.z).length() * 100.0;
            let head = heading(o.rider.yaw);
            let cosang = if dist > 1.0 { Vec3::new(to.x, 0.0, to.z).normalize().dot(head) } else { 1.0 };
            let d = -dist * cosang;
            let base_max = MAX[slot.min(5)] + off * k;
            let base_min = MIN[slot.min(5)] + off * k;
            let max = if rem < end { base_max + rem * (1.0 - f) } else { base_max };
            let min = if rem < end { -course_len } else { base_min };
            let m: f32 = if d < min { ((d - min) / min + 1.0) * 1.1359849 } else if d > max { 2.0487268 / (d - max) } else { 1.0 };
            let m = m.clamp(0.70005476, 1.5022597);
            let step = 0.008446341 * 60.0 * time.dt.min(0.1);
            o.driver.time_scale += (m - o.driver.time_scale).clamp(-step, step);
            if repick { o.driver.skill = if player_dtf < o.prog.dtf { pairs.0 } else { pairs.1 }; }
        }
    } else {
        for o in opponents.0.iter_mut() { o.driver.time_scale = 1.0; }
    }
    // the pre-race scene: riders stand where the scene puts them; put back in the gate after
    if let Some(sc) = race.0.scene.as_mut() {
        let n = 1 + opponents.0.len();
        if sc.saved.len() < n { sc.saved.resize(n, None); }
        for k in 0..n {
            let rr: &mut Rider = if k == 0 { &mut *r } else { &mut opponents.0[k - 1].rider };
            if sc.ended {
                if let Some((p, yaw)) = sc.saved[k].take() { rr.pos = p; rr.yaw = yaw; rr.vel = Vec3::ZERO; rr.grounded = false; }
            } else if let Some(Some((p, yaw))) = sc.slot(k).and_then(|i| sc.spots.get(i)) {
                if sc.saved[k].is_none() { sc.saved[k] = Some((rr.pos, rr.yaw)); }
                // the scene's heights are the stage floor's own (the stage is not part of the snow)
                rr.pos = *p;
                rr.grounded = true;
                rr.yaw = *yaw;
                rr.vel = Vec3::ZERO;
            }
        }
        if sc.ended { race.0.scene = None; }
    }
    // which way the course runs where each rider is (pushing, getting up after a fall)
    let dir_at = |line: &[Vec3], i: usize| -> Option<Vec3> {
        if line.len() < 2 { return None; }
        let i = i.min(line.len() - 2);
        let j = (i + 4).min(line.len() - 1);
        Some((line[j] - line[i]).normalize_or_zero()).filter(|d| *d != Vec3::ZERO)
    };
    r.course_dir = dir_at(&race.0.line, race.0.idx);
    r.course_pts = rider::course_points(&race.0.line, race.0.idx, r.pos);
    // resets go back onto the course's AI paths, as the original places riders, from wherever
    // the rider last was on the snow and riding
    {
        let paths = &race.0.paths;
        let keep = |rr: &mut Rider| {
            if paths.is_empty() || !rr.grounded || rr.crashed > 0.0 || rr.rail.is_some() { return; }
            if let Some((p, yaw)) = paths.respawn_point(rr.pos) {
                let ground = world.0.ground(p + Vec3::Y * 3.0, 3.0, 20.0).map_or(p.y, |h| h.y);
                rr.safe = (Vec3::new(p.x, ground, p.z), yaw);
                rr.safe_external = true;
            }
        };
        keep(r);
        for o in opponents.0.iter_mut() { keep(&mut o.rider); }
    }
    for o in opponents.0.iter_mut() {
        o.rider.course_dir = if o.driver.route.is_empty() { dir_at(&race.0.line, o.driver.idx) } else { dir_at(&o.driver.route, o.driver.idx) };
        o.rider.course_pts = if o.driver.route.is_empty() { rider::course_points(&race.0.line, o.driver.idx, o.rider.pos) } else { rider::course_points(&o.driver.route, o.driver.idx, o.rider.pos) };
    }
    // TRICKY_CRASHAT=s: the player wipes out s seconds into the race (for checks)
    if let Some(at) = std::env::var("TRICKY_CRASHAT").ok().and_then(|v| v.parse::<f32>().ok()) {
        if race.0.state == RaceState::Running && race.0.time >= at && race.0.time - time.dt.min(0.1) < at && r.crashed <= 0.0 { r.force_wipe(); }
    }
    // fixed small steps so the physics does not depend on frame rate
    let mut left = time.dt.min(0.1);
    while left > 1e-5 {
        let dt = left.min(1.0 / 120.0);
        // TRICKY_AUTOPLAY: the player's rider drives itself (for screenshots and checks)
        let input = if !waiting && std::env::var("TRICKY_AUTOPLAY").is_ok() {
            let d = auto.get_or_insert_with(|| rider::AiDriver::new(0.0, 1.0));
            d.drive(r, &world.0, &race.0.line, &race.0.course, dt)
        } else { input };
        // the start gate: lean during the countdown (up forward, down back), push out after GO
        let counting = race.0.state == RaceState::Countdown && !race.0.intro && game.screen == ui::Screen::Playing;
        if race.0.state == RaceState::Countdown && r.gate_state != 8 { r.gate_reset(); }
        if counting { r.gate_anticipate(if up { 1 } else if down { -1 } else { 0 }, dt); }
        if race.0.state == RaceState::Running { r.gate_go(); }
        let held = r.gate_launch(dt);
        let input = if held || r.gate_state == 9 { Input::default() } else { input };
        let input = r.finish_input(input);
        r.step(&world.0, &rails.0, input, dt);
        r.finish_after(dt);
        apply_pads(&world.0, r, dt);
        if held { r.vel = Vec3::new(0.0, r.vel.y.min(0.0), 0.0); }
        if let Some(t) = &race.0.tube { if t.apply(r, race.0.prog.laps, dt) { if let Some(d) = auto.as_mut() { d.reroute(); } } }
        if waiting { r.vel = Vec3::new(0.0, r.vel.y.min(0.0), 0.0); }
        let (cp, bonus, laps) = (race.0.checkpoints, race.0.bonus, race.0.prog.laps);
        race.0.update(r, dt);
        // a lap line: "3 LAPS TO GO" ... "FINAL LAP" (sent while laps are left)
        if race.0.prog.laps < laps && race.0.prog.laps > 0 {
            r.last_trick = if race.0.prog.laps == 1 { "FINAL LAP".into() } else { format!("{} LAPS TO GO", race.0.prog.laps) };
            r.trick_timer = 2.5;
        }
        if race.0.checkpoints > cp && game.event == ui::Event::Race && !opponents.0.is_empty() && !race.0.lines.is_empty() {
            // the split (Race_SplitGapCs): time behind the leader (or ahead of 2nd when leading),
            // the distance between you turned into time at your own average pace
            let len = race.0.lines.lines[0].dtf;
            let mine = len - race.0.prog.dtf;
            let mut others: Vec<f32> = opponents.0.iter().map(|o| len - o.prog.dtf).collect();
            others.sort_by(|a, b| b.total_cmp(a));
            let other = others[0];
            let rate = race.0.time / mine.max(1.0);
            let gap = rate * (other - mine);
            let ahead = gap <= 0.0;
            let t = gap.abs();
            r.last_trick = format!("CHECKPOINT  {}{}:{:05.2}", if ahead { "+" } else { "-" }, (t / 60.0) as u32, t % 60.0);
            r.trick_timer = 5.0;
        }
        if race.0.checkpoints > cp && game.event == ui::Event::ShowOff && race.0.bonus > bonus {
            // a checkpoint gives the show-off clock more time (the event's value, in seconds)
            r.last_trick = format!("TIME BONUS  {:.0} seconds", race.0.bonus - bonus);
            r.trick_timer = 2.5;
        }
        // places by distance to the finish (the AI's choice of path depends on it)
        let dtfs: Vec<f32> = std::iter::once(race.0.prog.dtf).chain(opponents.0.iter().map(|o| o.prog.dtf)).collect();
        let me_idx = rivals::index(lib.chars.get(lib.player).map_or("", |c| c.name.as_str()));
        let opponents_len = opponents.0.len();
        for (oi, o) in opponents.0.iter_mut().enumerate() {
            if o.grudge.is_none() {
                let them = rivals::index(lib.chars.get(o.character).map_or("", |c| c.name.as_str()));
                // a circuit carries the feeling over from heat to heat
                let last = if game.round > 0 { game.attitude.get(&them).copied() } else { None };
                let mut g = rivals::Grudge::carried(them, me_idx, last);
                // TRICKY_GRUDGE: everyone starts out after the player (for testing)
                if std::env::var("TRICKY_GRUDGE").is_ok() { g.level = 2; }
                o.grudge = Some(g);
            }
            let place = dtfs.iter().filter(|d| **d < dtfs[oi + 1]).count();
            let ai = if waiting { Input::default() } else {
                o.driver.drive_paths(&mut o.rider, &world.0, &race.0.paths, &race.0.line, &race.0.course, place, dt)
            };
            let g = o.grudge.as_mut().unwrap();
            g.tick(race.0.time);
            // a rider with a grudge goes after the player (AIComputer_AttackTarget)
            let ai = if !waiting && g.hostile() && race.0.state == RaceState::Running && o.driver.finished.is_none() && game.event == ui::Event::Race {
                attack(ai, &o.rider, r)
            } else { ai };
            // and taunts the player when close and off to one side (AIComputer_Taunt)
            o.taunt_wait = if o.rider.grounded && o.rider.crashed <= 0.0 && o.rider.taunt < 0.0 { o.taunt_wait - dt } else { 1.0253273 };
            if o.taunt_wait <= 0.0 {
                o.taunt_wait = 1.0253273;
                let to = r.pos - o.rider.pos;
                let flat = Vec3::new(to.x, 0.0, to.z);
                // Ride_AITauntCheck: after 1.025 s of riding, a 91% roll, at its vendetta target (any
                // level) within 10 m and more than 60 degrees off its heading
                let cos = flat.normalize_or_zero().dot(heading(o.rider.yaw));
                let roll = ((race.0.time * 977.0 + oi as f32 * 131.0).sin() * 43758.547).fract().abs();
                if g.level >= 0 && o.rider.grounded && o.rider.crashed <= 0.0 && o.rider.taunt < 0.0 && o.rider.shove <= 0.0
                    && flat.length() < 10.0 && cos < 0.5 && roll < 0.91 {
                    let n = 1 + (roll * 1000.0) as u32 % 5;
                    let right = Vec3::new(o.rider.yaw.cos(), 0.0, -o.rider.yaw.sin());
                    // the game compares the angle with 2.3561945 (3π/4, 0x103aa0); cos(3π/4) = −1/√2
                    let behind = cos < -std::f32::consts::FRAC_1_SQRT_2;
                    o.rider.taunt_clip = if behind { format!("bxRT_AITAUNT{n}") } else if flat.dot(right) > 0.0 { format!("bxRT_TAUNTTS{n}") } else { format!("bxRT_TAUNTHS{n}") };
                    o.rider.taunt = 0.0;
                }
            }
            // computer riders in the gate (AIComputer_GateAnticipateInput): forward in the last half
            // second, back the half second before, rocking before that
            if race.0.state == RaceState::Countdown && o.rider.gate_state != 8 { o.rider.gate_reset(); }
            if race.0.state == RaceState::Countdown && !race.0.intro {
                let left = race.0.countdown * 60.0;
                let sc = if o.driver.skill >= 1.0 { 1.0 } else { o.driver.skill / 3.2585914 };
                let y = if left < 30.0 * sc { 1 } else if left < 60.0 * sc { -1 } else { o.rider.gate_rock() };
                o.rider.gate_anticipate(y, dt);
            }
            if race.0.state == RaceState::Running { o.rider.gate_go(); }
            let held = o.rider.gate_launch(dt);
            let ai = if held || o.rider.gate_state == 9 { Input::default() } else { ai };
            let ai = o.rider.finish_input(ai);
            o.rider.step(&world.0, &rails.0, ai, dt * o.driver.time_scale);
            o.rider.finish_after(dt);
            if apply_pads(&world.0, &mut o.rider, dt) { o.driver.reroute(); }
            if held { o.rider.vel = Vec3::new(0.0, o.rider.vel.y.min(0.0), 0.0); }
            if let Some(t) = &race.0.tube { if t.apply(&mut o.rider, o.prog.laps, dt) { o.driver.reroute(); } }
            if waiting { o.rider.vel = Vec3::new(0.0, o.rider.vel.y.min(0.0), 0.0); }
            else if race.0.state == RaceState::Running && o.driver.finished.is_none() {
                // rivals cross the same finish event the player does
                let crossed = race.0.lines.update(&mut o.prog, o.rider.pos, o.rider.vel).iter().any(|e| e.kind == course::EV_FINISH);
                if crossed {
                    o.driver.finished = Some(race.0.time);
                    let n = opponents_len + 1;
                    o.rider.start_finish(place < n.div_ceil(2));
                }
            }
        }
        if !waiting {
            // riders shove each other, and anyone can send a marker or crash bag flying
            for i in 0..opponents.0.len() {
                // a rival looks for someone to shove every 12 ticks: within 2 m and 30-150 degrees
                // off its heading (to the side), as the original's AI does
                let o = &mut opponents.0[i];
                o.driver.shove_wait -= dt;
                let gap = r.pos - o.rider.pos;
                let flat = Vec3::new(gap.x, 0.0, gap.z);
                let side_on = flat.normalize_or_zero().dot(heading(o.rider.yaw)).abs() < 0.866;
                let close = flat.length() < 2.0 && gap.y.abs() < 1.5 && side_on && r.crashed <= 0.0 && o.rider.crashed <= 0.0 && o.rider.rail.is_none() && race.0.time > 3.0;
                o.driver.beside = if close { o.driver.beside + dt } else { 0.0 };
                // only a rider holding a grudge shoves (AIComputer_ShouldShove)
                let grudging = o.grudge.as_ref().is_some_and(|g| g.shoves());
                if close && grudging && o.driver.shove_wait <= 0.0 && game.event == ui::Event::Race {
                    // the shove clip runs about 0.7 s before another can start
                    o.driver.shove_wait = 0.7;
                    o.rider.shove = 0.7;
                    let right = Vec3::new(o.rider.yaw.cos(), 0.0, -o.rider.yaw.sin());
                    o.rider.shove_side = if gap.dot(right) >= 0.0 { 1.0 } else { -1.0 };
                    let push = props::shove_push(&o.rider, r);
                    let was = r.stumble > 0.0;
                    if r.shove <= 0.0 {
                        let down = r.take_hit(push, false);
                        if down { o.rider.add_meter(1.0); r.last_trick = "Shoved!".into(); r.trick_timer = 2.0; }
                        // getting its own back cools the grudge
                        if down || (!was && r.stumble > 0.0) { if let Some(g) = o.grudge.as_mut() { g.got_even(down); } }
                    }
                }
                // running into each other: a rider the player knocks about remembers (30 down, 9 off balance)
                let o = &mut opponents.0[i];
                let (od, os, pd) = (o.rider.crashed > 0.0, o.rider.stumble > 0.0, r.crashed > 0.0);
                props::bump(r, &mut o.rider);
                if let Some(g) = o.grudge.as_mut() {
                    if !od && o.rider.crashed > 0.0 { g.hit(30.0, race.0.time); }
                    else if !os && o.rider.stumble > 0.0 { g.hit(9.0, race.0.time); }
                    if !pd && r.crashed > 0.0 && o.rider.crashed <= 0.0 { g.got_even(true); }
                }
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
    // show-off ends when its clock runs out
    if playing && game.event == ui::Event::ShowOff && race.0.state == RaceState::Running && race.0.time >= ui::showoff(&ui::track_key(&list)).1 + race.0.bonus {
        race.0.state = RaceState::Finished;
    }
    // over the line the rider brakes to a stop and reacts; the results come once it has stood
    // there (4 s after the line, as cRaceHandler_Update waits for Boarder_IsFinishedAndStopped)
    if race.0.state == RaceState::Finished && rider.0.fin.is_none() {
        let n = opponents.0.len() + 1;
        let place = opponents.0.iter().filter(|o| o.driver.finished.is_some_and(|t| t < race.0.time)).count();
        let win = if game.event == ui::Event::ShowOff { true } else { place < n.div_ceil(2) };
        rider.0.start_finish(win);
    }
    // TRICKY_POST=win|lose|rival: jump straight to the end of the race (to look at the post-race scenes)
    if let Ok(p) = std::env::var("TRICKY_POST") {
        if race.0.state == RaceState::Countdown && game.screen == ui::Screen::Playing {
            race.0.state = RaceState::Finished;
            race.0.time = 100.0;
            rider.0.fin = Some((2, 5.0, p != "lose"));
            if p == "rival" { if let Some(o) = opponents.0.first_mut() { o.grudge.get_or_insert_with(|| rivals::Grudge::new(0, 0)).level = 2; } }
        }
    }
    let stopped = rider.0.finished_and_stopped() || std::env::var("TRICKY_SIM").is_ok();
    let quiet = std::env::var("TRICKY_INTRO").is_err() && (std::env::var("TRICKY_SIM").is_ok() || std::env::var("TRICKY_NOINTRO").is_ok() || std::env::var("TRICKY_SHOT").is_ok());
    if playing && stopped && race.0.state == RaceState::Finished && race.0.post == 0 {
        // how each rider now feels about the player, for the next heat
        if game.event == ui::Event::Race {
            if game.round == 0 { game.attitude.clear(); }
            for o in opponents.0.iter() {
                if let Some(g) = &o.grudge { game.attitude.insert(rivals::index(lib.chars.get(o.character).map_or("", |c| c.name.as_str())), g.settled()); }
            }
        }
        // the post-race scenes (cEndRaceHandler_Update): a rival who has it in for the player has a
        // word first, then the player wins or loses at the finish area
        let me = rivals::index(lib.chars.get(lib.player).map_or("", |c| c.name.as_str()));
        let rival = opponents.0.iter().enumerate()
            .filter_map(|(i, o)| o.grudge.as_ref().filter(|g| g.score > 2).map(|g| (g.score, i)))
            .max_by_key(|a| a.0)
            .map(|(_, i)| (rivals::index(lib.chars.get(opponents.0[i].character).map_or("", |c| c.name.as_str())), i + 1));
        match race.0.cml.clone() {
            Some(cml) if !quiet && race.0.finish_area.is_some() && game.event != ui::Event::FreeRide => {
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.subsec_nanos());
                let win = rider.0.fin.is_some_and(|f| f.2);
                let (scripts, cast) = intro::plan_post(&cml, me, win, rival, 1 + opponents.0.len(), seed);
                race.0.post_scripts = scripts;
                race.0.cast = Some(cast);
                race.0.post = 1;
            }
            _ => race.0.post = 3,
        }
    }
    if playing && stopped && race.0.state == RaceState::Finished && race.0.post == 3 && game.event != ui::Event::FreeRide {
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
        if let Some(u) = v.get(3) { if *u > 0.0 { r.uber = *u; r.uber_id = 0; } }
        if let Some(sp) = v.get(5) { r.spin = *sp; }
        if let Some(t) = v.get(4).filter(|t| **t > -900.0) { r.rail = Some((0, 3.0, 0.0)); r.rail_twist = t.to_radians(); r.yaw = r.spawn.1 - r.rail_twist; r.normal = Vec3::Y; }
        race.0.state = RaceState::Running;
        race.0.time = 10.0;
    }
}

/// Keep exactly one body per rider, of the right character.
fn sync_visuals(
    mut commands: Commands, lib: Res<CharLib>, opponents: Res<Opponents>, game: Res<ui::Game>,
    existing: Query<(Entity, &RiderVisual)>, mut meshes: ResMut<Assets<Mesh>>,
) {
    if lib.chars.is_empty() { return; }
    // (who, which rider, which of that rider's boards)
    let mut want: Vec<(Who, usize, usize)> = vec![(Who::Player, lib.player, game.board.min(11))];
    want.extend(opponents.0.iter().enumerate().map(|(i, o)| (Who::Ai(i), o.character, if game.master { 11 } else { 0 })));
    let mut have: Vec<(Who, usize, usize)> = existing.iter().map(|(_, v)| (v.who, v.character, v.board)).collect();
    let key = |w: &(Who, usize, usize)| (match w.0 { Who::Player => 0, Who::Ai(i) => i + 1 }, w.1, w.2);
    want.sort_by_key(key);
    have.sort_by_key(key);
    if want == have { return; }
    for (e, _) in &existing { commands.entity(e).despawn(); }
    for (who, character, board) in want { spawn_visual(&mut commands, who, character, board, &lib, &mut meshes); }
}

/// The characters' folder (riders, boards, and global textures such as the crowd's).
fn chars_dir(level_dir: &Path) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    [level_dir.join("../../chars"), exe.join("../chars"), exe.join("chars"), PathBuf::from("chars")]
        .into_iter().find(|d| d.join("board.json").exists() || d.join("mac/model.json").exists())
}

fn load_chars(level_dir: &Path, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> CharLib {
    let Some(dir) = chars_dir(level_dir) else { return CharLib::default() };
    let mut lib = CharLib { board: CharModel::load(&dir.join("board.json")).ok(), dir: dir.clone(), ..default() };
    lib.shapes = ["bx", "bx_goofy", "fr", "fr_goofy", "al", "al_goofy"].iter().map(|n| CharModel::load(&dir.join(format!("board_{n}.json"))).ok()).collect();
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
        let ubers = ["uber.json", "uber_fr.json", "uber_ex.json"].map(|f| AnimSet::load(&d.join(f)).ok().filter(|a| a.len() > 0));
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        // bord1.png .. bord12.png, the rider's twelve boards (the plain bord.png if they are missing)
        let boards = (1..=12).map(|n| match image::open(d.join(format!("bord{n}.png"))) {
            Ok(i) => materials.add(StandardMaterial { base_color_texture: Some(images.add(make_image(i.to_rgba8(), true))), unlit: true, cull_mode: None, double_sided: true, ..default() }),
            Err(_) => mats.get("bord").cloned().unwrap_or_default(),
        }).collect();
        let data = trickdata::RIDERS.iter().position(|r| r.name.eq_ignore_ascii_case(&name)).unwrap_or(0);
        lib.chars.push(CharEntry { name, model, mats, ubers, boards, data });
    }
    lib.player = lib.chars.iter().position(|c| c.name == "mac").unwrap_or(0);
    lib.anims = AnimSet::load(&dir.join("anims/bx.json")).ok().filter(|a| a.len() > 0);
    lib.scenes = AnimSet::load(&dir.join("anims/scenes.json")).ok().filter(|a| a.len() > 0);
    // the post-race scenes' clips (win, lose, rivals), extracted into finish.json
    if let Ok(f) = AnimSet::load(&dir.join("anims/finish.json")) {
        match lib.scenes.as_mut() { Some(s) => s.merge(f), None => lib.scenes = Some(f) }
    }
    lib.fr = AnimSet::load(&dir.join("anims/fr.json")).ok().filter(|a| a.len() > 0);
    lib.ex = AnimSet::load(&dir.join("anims/ex.json")).ok().filter(|a| a.len() > 0);
    lib.cm = AnimSet::load(&dir.join("anims/cm.json")).ok().filter(|a| a.len() > 0);
    println!("animations: {}", lib.anims.as_ref().map_or(0, |a| a.len()));
    println!("characters: {}", lib.chars.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
    lib
}

fn spawn_visual(commands: &mut Commands, who: Who, character: usize, board: usize, lib: &CharLib, meshes: &mut Assets<Mesh>) -> Entity {
    let entry = &lib.chars[character];
    // the board's shape goes with its kind (and the rider's stance), its graphic with its number
    let info = &trickdata::BOARDS[entry.data][board.min(11)];
    // (the goofy shapes are not used: the riding animations here are not mirrored for goofy riders)
    let shape = (info.kind as usize).min(2) * 2;
    let board_model = lib.shapes.get(shape).and_then(|s| s.as_ref()).or(lib.board.as_ref());
    let mut root = commands.spawn((Name::new(format!("Rider {}", entry.name)), RiderVisual { who, character, board, shape, pose: Pose::default(), roll: 0.0, base: Quat::IDENTITY, anim: AnimState::default() },
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
        if let Some(model) = board_model {
            for (i, part) in model.parts.iter().enumerate() { piece(&part.verts, entry.boards.get(board).or(entry.mats.get("bord")), i, true); }
        }
    });
    root.id()
}

/// Move every rider's body to its rider, pose it, and re-skin the meshes.
fn animate_visuals(
    time: Res<SmoothDt>, rider: Res<RiderRes>, opponents: Res<Opponents>, lib: Res<CharLib>, race: Res<RaceRes>, game: Res<ui::Game>,
    mut roots: Query<(&mut RiderVisual, &mut Transform, &Children)>,
    parts: Query<(&BodyPart, &Mesh3d)>, mut meshes: ResMut<Assets<Mesh>>,
    mut scratch: Local<(Vec<[f32; 3]>, Vec<[f32; 3]>)>,
) {
    // paused: hold every body as it is
    if game.screen == ui::Screen::Paused { return; }
    let dt = time.dt;
    for (mut vis, mut tf, children) in &mut roots {
        let r: &Rider = match vis.who { Who::Player => &rider.0, Who::Ai(i) => match opponents.0.get(i) { Some(o) => &o.rider, None => continue } };
        let Some(entry) = lib.chars.get(vis.character) else { continue };
        // where the body is and which way it faces
        let on_snow = r.grounded || r.air_time < 0.15 || r.rail.is_some();
        let up = if on_snow { r.normal } else { Vec3::Y };
        // riding switch the game swaps which way the feet point and the head still looks ahead (its
        // animation mirror flag, rider+0x46d4): the body is the board's way round, drawn mirrored below
        let mirror = r.switch && r.crashed <= 0.0;
        let h = heading(r.yaw);
        let fwd = (h - up * h.dot(up)).normalize_or(h);
        let k = |rate: f32| 1.0 - (-rate * dt).exp();
        let want_roll = if r.grounded { -r.input.steer * 0.32 * (r.vel.length() / 12.0).min(1.0) } else { 0.0 };
        vis.roll += (want_roll - vis.roll) * k(8.0);
        // down in a wipe-out the body is the tumbling rigid body's frame
        let base = match r.wipe.as_ref().filter(|_| r.crashed > 0.0) { Some(w) => w.q, None => Transform::from_translation(r.pos).looking_to(fwd, up).rotation * Quat::from_rotation_z(vis.roll) };
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
        // the gate clip follows the rider's progress through the gate (AnimUpdate_GateProgress)
        let gate = match race.0.state {
            RaceState::Countdown => Some(r.gate_p * 30.0),
            RaceState::Running if r.gate_state == 9 => Some(r.gate_p * 30.0),
            _ => None,
        };
        // standing up once stopped over the line (finish state 2)
        let finished = r.fin.is_some_and(|f| f.0 >= 2);
        // the game's own animation if we have it, the code-built pose otherwise
        // each board type has its own set of riding clips (bx / fr / ex), wipe-outs share one (cm)
        let kind_set = match r.stats.kind { 1 => lib.fr.as_ref().map(|s| (s, "fr")), 2 => lib.ex.as_ref().map(|s| (s, "ex")), _ => None };
        let mut sample = lib.anims.as_ref().and_then(|set| animate(set, kind_set, lib.cm.as_ref(), entry.ubers[(r.stats.kind as usize).min(2)].as_ref().or(entry.ubers[0].as_ref()), gate, finished, r, &mut vis, on_snow, dt));
        // in a pre-race scene each rider loops the scene's clip for them
        let k = match vis.who { Who::Player => 0, Who::Ai(i) => i + 1 };
        if let (Some(sc), Some(set)) = (race.0.scene.as_ref().filter(|s| !s.ended), lib.scenes.as_ref()) {
            if let Some(c) = sc.slot(k).and_then(|i| sc.clips.get(i)).and_then(|n| set.get(n)) {
                sample = Some(c.at((sc.t * character::ANIM_FPS) % c.len().max(1.0), true));
            }
        }
        let (mats, lift) = match &sample { Some(sm) => (entry.model.skin_sample(sm), 0.0), None => entry.model.skin(&vis.pose) };
        let board_model = lib.shapes.get(vis.shape).and_then(|s| s.as_ref()).or(lib.board.as_ref());
        let board_mats = match (&sample, board_model) { (Some(sm), Some(b)) => b.skin_board(sm), _ => vec![Mat4::IDENTITY] };
        let sun = (tf.rotation.inverse() * Vec3::new(0.35, 0.8, 0.45)).normalize();
        for child in children.iter() {
            let Ok((part, mesh)) = parts.get(child) else { continue };
            let Some(mesh) = meshes.get_mut(&mesh.0) else { continue };
            let s = &mut *scratch;
            let (pos, nrm) = (&mut s.0, &mut s.1);
            let (model, mats) = if part.board { (board_model, &board_mats) } else { (Some(&entry.model), &mats) };
            let Some(p) = model.and_then(|m| m.parts.get(part.part)) else { continue };
            if sample.is_some() { character::skin_part_anim(p, mats, pos, nrm); }
            else { character::skin_part(p, mats, if part.board { 0.0 } else { lift }, pos, nrm); }
            // switch: mirrored across the board's long axis (local x is sideways; the materials draw both faces)
            if mirror {
                for v in pos.iter_mut().chain(nrm.iter_mut()) { v[0] = -v[0]; }
            }
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
#[allow(clippy::too_many_arguments)]
fn animate(set: &AnimSet, kind_set: Option<(&AnimSet, &str)>, cm: Option<&AnimSet>, ubers: Option<&AnimSet>, gate: Option<f32>, finished: bool, r: &Rider, vis: &mut RiderVisual, on_snow: bool, dt: f32) -> Option<Sample> {
    let fps = character::ANIM_FPS;
    let crouch = vis.pose.crouch;
    let lean = vis.pose.lean;
    let a = &mut vis.anim;
    // only while actually turning: the stick is held, or a flip is still coming round to level
    let turning = |a: &mut AnimState| {
        let tau = std::f32::consts::TAU;
        let tilt = r.flip.rem_euclid(tau).min(tau - r.flip.rem_euclid(tau));
        let turning = r.input.steer.abs() > 0.1 || r.input.flip != 0.0 || tilt > 0.35;
        if !turning { a.rot = 0.0; }
        turning
    };
    a.cycle += dt * fps;
    if on_snow && !a.was_on_snow && a.air > 0.35 { a.land = 0.0; }
    a.air = if on_snow { 0.0 } else { a.air + dt };
    a.was_on_snow = on_snow;
    a.land += dt * fps;
    if r.grab != 0 && a.grab != r.grab { a.grab = r.grab; a.grab_frame = 0.0; }
    // a "bx" name is looked up in the board type's own set first (frRL_..., exRL_...)
    let clip = |n: &str| -> Option<&character::Clip> {
        if let (Some((ks, pre)), Some(rest)) = (kind_set, n.strip_prefix("bx")) {
            if let Some(c) = ks.get(&format!("{pre}{rest}")) { return Some(c); }
        }
        set.get(n)
    };
    let target: Sample = if let Some(g) = gate {
        // in the gate: hold the first frame through the countdown, then push out
        clip("bxG_GATESTART")?.at(g.max(0.0), false)
    } else if r.crashed > 0.0 {
        // the original's wipe-out clips (rider::wipeout, `WipeoutMotion_Update`): impacts (cmI_ /
        // cmCI_), the curled and stretched tumbles, the back and front slides, then a get-up
        let _ = (&mut a.impact, &mut a.was_down);
        let Some(w) = r.wipe.as_ref() else { return Some(clip("bxB_FWD")?.at(0.0, false)) };
        let name = rider::wipeout::clip_name(w.clip);
        let c = if name.starts_with("cm") { cm.and_then(|s| s.get(&name)) } else { clip(&name) };
        let f = w.clip_t * fps;
        use rider::wipeout::*;
        match c {
            Some(c) if matches!(w.clip, CLIP_BUTTSLIDE | CLIP_FACESLIDE | CLIP_FALLING | CLIP_CRUNCH) => c.at(f, true),
            Some(c) => c.at(f.min(c.len() - 0.01), false),
            None => clip("bxB_FWD")?.at(f.min(10.0), false),
        }
    } else if let Some(c) = r.recover.and_then(|(id, t)| clip(&rider::wipeout::clip_name(id)).map(|c| (c, t))).filter(|_| r.rail.is_none() && r.uber <= 0.0) {
        // back up without a get-up (CT_CRUNCH2BASE / STRETCH2BASE) or out of a tumble in the air (A_FALLRECOVER)
        c.0.at((c.1 * fps).min(c.0.len() - 0.01), false)
    } else if r.uber > 0.0 && ubers.and_then(|u| u.nth(r.uber_id)).is_some() {
        ubers?.nth(r.uber_id)?.at(r.uber, false)
    } else if r.rail.is_some() {
        // square on the rail, or sideways with the chest (frontside) or the back (backside) leading
        let side = r.rail_twist.sin();
        let base = clip("bxRS_CYCLEBASE")?.at(a.cycle, true);
        match clip(if side > 0.0 { "bxRS_CYCLEBS" } else { "bxRS_CYCLEFS" }) {
            Some(c) if side.abs() > 0.05 => base.blend(&c.at(a.cycle, true), ((side.abs() - 0.2) / 0.6).clamp(0.0, 1.0)),
            _ => base,
        }
    } else if !on_snow {
        // the grab clip at the rider's own frame: reaching in and holding at its second marker,
        // then played out from there when let go (the original's release)
        let shown = if r.grab != 0 { r.grab } else { r.grab_out.map_or(0, |g| g.0) };
        let grab = (shown as usize).checked_sub(1).and_then(|i| r.data.rows.get(i)).map_or("", |row| row.clip);
        if let Some(c) = clip(grab) {
            let _ = c.len();
            let peak = rider::grab_markers(grab).1;
            a.grab_frame = if r.grab != 0 { r.grab_hold } else { r.grab_out.map_or(0.0, |g| g.1) };
            a.grab = shown;
            let held = c.at(a.grab_frame.max(0.01), false);
            // the tweak has its own clip, played out from the held grab
            let tw = grab.strip_prefix("bxT_").and_then(|n| clip(&format!("bxTW_{n}")));
            a.tweak = if r.grab != 0 && r.input.tweak && a.grab_frame >= peak - 0.5 { a.tweak + dt * fps } else { (a.tweak - dt * fps * 2.0).max(0.0) };
            match tw { Some(t) if a.tweak > 0.0 => held.blend(&t.at(a.tweak.min(t.len() * 0.5), false), (a.tweak / 4.0).min(1.0)), _ => held }
        } else if turning(a) {
            // tuck into the rotation, then hold its cycle
            a.rot += dt * fps;
            let tau = std::f32::consts::TAU;
            let flipping = r.input.flip != 0.0 || r.flip.rem_euclid(tau).min(tau - r.flip.rem_euclid(tau)) > 0.35;
            let (into, cycle) = if flipping {
                if r.flip > 0.0 { ("bxA_INTOFLIPFWD", "bxA_FLIPCYCLEFWD") } else { ("bxA_INTOFLIPBWD", "bxA_FLIPCYCLEBWD") }
            } else if r.input.steer > 0.0 { ("bxA_INTOSPINFS", "bxA_SPINCYCLEFS") } else { ("bxA_INTOSPINBS", "bxA_SPINCYCLEBS") };
            match clip(into) {
                Some(c) if a.rot < c.len() - 1.0 => c.at(a.rot, false),
                _ => clip(cycle)?.at(a.cycle, true),
            }
        } else {
            let c = clip("bxJ_TAKEOFF")?;
            c.at(a.air * fps, false)
        }
    } else if a.land < clip("bxL_NORMAL")?.len() - 1.0 {
        // a crooked or tilted landing has its own recovery
        let c = clip(match r.land_kind { 1 => "bxL_HS", 2 => "bxL_TS", 3 => "bxL_ARMS", _ => "bxL_NORMAL" }).or(clip("bxL_NORMAL"))?;
        c.at(a.land * c.len() / clip("bxL_NORMAL")?.len(), false)
    } else if finished && clip("bxR_CRUISE2FINISH").is_some() {
        // over the line: stand up out of the ride
        // then a cheer or a sulk (bxRR_POS1-10 / bxRR_NEG1-9, picked at random), then the finish idle
        a.done += dt * fps;
        let c = clip("bxR_CRUISE2FINISH")?;
        let win = r.fin.is_some_and(|f| f.2);
        let pick = (r.pos.x.abs() * 7.0 + r.pos.z.abs() * 13.0) as usize;
        let react = if win { format!("bxRR_POS{}", 1 + pick % 10) } else { format!("bxRR_NEG{}", 1 + pick % 9) };
        let idle = cm.and_then(|c| c.get("cmFL_FINISHCYCLE"));
        if a.done < c.len() - 1.0 { c.at(a.done, false) }
        else if let Some(rc) = clip(&react).filter(|rc| a.done - c.len() < rc.len() - 1.0) { rc.at(a.done - c.len(), false) }
        else if let Some(ic) = idle { ic.at(a.cycle, true) }
        else { c.at(c.len() - 1.0, false) }
    } else if r.taunt >= 0.0 && clip(&r.taunt_clip).is_some() {
        let c = clip(&r.taunt_clip)?;
        c.at((r.taunt * fps).min(c.len() - 1.0), false)
    } else if r.shove > 0.0 && clip("bxR_PUSHTS").is_some() {
        let c = clip(if r.shove_side > 0.0 { "bxR_PUSHTS" } else { "bxR_ELBOWHS" })?;
        c.at(((0.7 - r.shove) * fps).clamp(0.0, c.len() - 1.0), false)
    } else if r.charge > 0.05 && (r.wind.abs() > 0.1 || r.wind_flip.abs() > 0.1) {
        // crouched and winding up: lean into the spin or the flip that is coming
        let down = clip("bxRL_CROUCHCYCLE")?.at(a.cycle, true);
        let (name, w) = if r.wind.abs() >= r.wind_flip.abs() { (if r.wind > 0.0 { "bxJ_PWDRIGHT" } else { "bxJ_PWDLEFT" }, r.wind.abs()) }
            else { (if r.wind_flip > 0.0 { "bxJ_PWDFWD" } else { "bxJ_PWDBWD" }, r.wind_flip.abs()) };
        match clip(name) { Some(c) => down.blend(&c.at(w.min(1.0) * (c.len() - 1.0), false), (w * 2.0).min(1.0)), None => down }
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
    if on_snow { a.grab = 0; a.grab_frame = 0.0; a.rot = 0.0; a.tweak = 0.0; }
    if !finished { a.done = 0.0; }
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
    mut cam: Single<(&mut Transform, &mut Projection), (With<FlyCam>, Without<RiderModel>)>,
    keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut flash: Local<f32>, mut label: Query<&mut Text, With<CamLabel>>,
    mut race: ResMut<RaceRes>, mut director: Local<Option<intro::Director>>, mut started: Local<bool>, opponents: Res<Opponents>,
) {
    let opponents_n = opponents.0.len();
    // the pre-race intro (the course's camera scripts): a fly-through, then the start gate
    let post = race.0.post;
    if game.screen != ui::Screen::Playing || (race.0.state != RaceState::Countdown && post != 1 && post != 2) { *started = false; if director.is_some() { *director = None; race.0.intro = false; }
        if let Some(sc) = race.0.scene.as_mut() { sc.ended = true; } }
    else if post == 1 {
        // after the race: the rival's scene and the win or lose scene, at the finish area
        if let Some(cml) = race.0.cml.clone() {
            let names: Vec<&str> = race.0.post_scripts.iter().map(|s| s.as_str()).collect();
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.subsec_nanos());
            *director = Some(intro::Director::new(&cml, &names, seed));
            race.0.intro = true;
            race.0.post = 2;
        } else { race.0.post = 3; }
    }
    else if !*started && game.since < 1.0 && race.0.state == RaceState::Countdown {
        *started = true;
        let quiet = std::env::var("TRICKY_INTRO").is_err() && (std::env::var("TRICKY_SIM").is_ok() || std::env::var("TRICKY_NOINTRO").is_ok() || std::env::var("TRICKY_SHOT").is_ok());
        if let (Some(cml), false) = (race.0.cml.clone(), quiet) {
            let fly = match game.event { ui::Event::Race => "RaceLoaded", ui::Event::ShowOff => "ShowLoaded", ui::Event::FreeRide => "FreeLoaded" };
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.subsec_nanos());
            // a race also has the riders' staging scene and the gate scene (one of four each, at random)
            let stg = format!("STG_COM_{}", 1 + seed % 4);
            let gat = format!("GAT_6COM_{}", 1 + (seed / 7) % 4);
            let race_ev = game.event == ui::Event::Race;
            let only = std::env::var("TRICKY_INTRO").unwrap_or_default();
            let scripts: Vec<&str> = match only.as_str() {
                "gate" => vec![if race_ev { gat.as_str() } else { "Gate_Master" }],
                "stage" => vec![stg.as_str(), gat.as_str()],
                _ if race_ev => vec![fly, stg.as_str(), gat.as_str()],
                _ => vec![fly, "Gate_Master"],
            };
            *director = Some(intro::Director::new(&cml, &scripts, seed));
            race.0.intro = true;
        }
    }
    let skip = keys.just_pressed(KeyCode::Space) || pads.iter().any(|p| p.just_pressed(GamepadButton::South));
    let mut intro_view = None;
    if let (Some(d), Some(cml)) = (director.as_mut(), race.0.cml.clone()) {
        d.step(&cml, time.dt);
        if std::env::var("TRICKY_INTRODBG").is_ok() { eprintln!("intro: done {} shot {:?} scene {:?}", d.done, d.shot.map(|s| s.pos), d.scene); }
        // the riders' scene: started and stopped by the script
        let want = if d.done { None } else { d.scene.clone() };
        let stage = race.0.stage;
        let finish_area = race.0.finish_area;
        let cast = race.0.cast.clone();
        let riders = 1 + opponents_n;
        match (&mut race.0.scene, want) {
            (Some(sc), Some(n)) if sc.name == n => { sc.t += time.dt; }
            (cur, Some(n)) => {
                let spots = cml.scene(&n).into_iter().map(|sp| {
                    if !sp.place { return None; }
                    // frame 1: relative to the start stage area, 2: the finish area
                    let area = match sp.frame { 1 => stage, 2 => finish_area, _ => None };
                    let (p, yaw) = match area {
                        Some((loc, rot)) => { let x = rot * Vec3::X; (loc + rot * sp.pos, sp.yaw + x.y.atan2(x.x)) }
                        None => (sp.pos, sp.yaw),
                    };
                    // game (x, y, z) cm, facing (cos yaw, sin yaw) -> Bevy metres and our yaw
                    Some((Vec3::new(p.x, p.z, -p.y) * WORLD_SCALE, yaw - std::f32::consts::FRAC_PI_2))
                }).collect();
                let saved = cur.as_ref().map(|c| c.saved.clone()).unwrap_or_default();
                let (clips, slot_of) = match &cast { Some(c) => { let mut c = c.clone(); c.riders = riders; intro::post_scene(&n, &c) } None => (intro::scene_clips(&n), Vec::new()) };
                *cur = Some(intro::SceneState { clips, name: n, spots, t: 0.0, saved, ended: false, slot_of });
            }
            (Some(sc), None) => { sc.ended = true; }
            (None, None) => {}
        }
        if d.done || skip { if let Some(sc) = race.0.scene.as_mut() { sc.ended = true; } }
        if d.done || skip { *director = None; race.0.intro = false; chase.0.cut(); if race.0.post == 2 { race.0.post = 3; } }
        else if let Some(s) = d.shot { intro_view = Some(s); }
    }
    // C / Triangle: the next camera (board, near, far, over), as in the original's options
    if game.screen == ui::Screen::Playing && (keys.just_pressed(KeyCode::KeyC) || pads.iter().any(|p| p.just_pressed(GamepadButton::North))) {
        chase.0.cycle(1);
        *flash = 1.5;
    }
    *flash = (*flash - time.dt).max(0.0);
    for mut t in label.iter_mut() { t.0 = if *flash > 0.0 { chase.0.name().to_uppercase() } else { String::new() }; }
    let (mut pos, mut look) = chase.0.update(&world.0, &rider.0, time.dt);
    let mut roll = chase.0.roll;
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
            roll = 0.0;
        }
    }
    if game.screen == ui::Screen::Menu { roll = 0.0; }
    // TRICKY_CAM=x,y,z,tx,ty,tz (Bevy metres): a fixed camera (for checks)
    if let Ok(o) = std::env::var("TRICKY_CAM") {
        let v: Vec<f32> = o.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() == 6 { pos = Vec3::new(v[0], v[1], v[2]); look = Vec3::new(v[3], v[4], v[5]); roll = 0.0; }
    }
    let mut fov_h = chase.0.fov;
    if let Some(s) = intro_view {
        // game units (cm, Z up) to Bevy (m, Y up); a fixed camera looks along its yaw and pitch
        // cameras of the shared scenes are placed relative to the course's start stage area
        let area = match s.frame { 1 => race.0.stage, 2 => race.0.finish_area, _ => None };
        let (gp, gyaw) = match area {
            Some((loc, rot)) => { let x = rot * Vec3::X; (loc + rot * s.pos, s.yaw + x.y.atan2(x.x)) }
            None => (s.pos, s.yaw),
        };
        pos = Vec3::new(gp.x, gp.z, -gp.y) * WORLD_SCALE;
        let dir = Vec3::new(s.pitch.cos() * gyaw.cos(), s.pitch.cos() * gyaw.sin(), s.pitch.sin());
        look = if s.at_rider { rider.0.pos + Vec3::Y } else { pos + Vec3::new(dir.x, dir.z, -dir.y) };
        roll = 0.0;
        fov_h = s.fov;
    }
    let (tf, proj) = &mut *cam;
    let mut t = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    t.rotate_local_z(roll);
    **tf = t;
    // the original's field of view is across the width of the picture
    if let Projection::Perspective(p) = &mut **proj {
        let aspect = p.aspect_ratio.max(0.1);
        let h = if game.screen == ui::Screen::Menu { 1.2 } else { fov_h };
        p.fov = 2.0 * ((h * 0.5).tan() / aspect).atan();
    }
}

/// The original's race clock: m:ss.cc (HUD_FormatRaceTime)
fn clock(t: f32) -> String { let cs = (t.max(0.0) * 100.0) as u32; format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100) }

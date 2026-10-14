//! The rider's sounds as the game makes them (`BoardIn_*`, `Voice_*`, `Audio_SurfaceGroup`,
//! DATA/CONFIG/SNOW.INF): the original board samples (zboard.bnk) and effects (zbxsfx.bnk),
//! exported as `audio/bnk/board_<program>.wav` / `sfx_<program>.wav`, and the snow sound programs
//! from `audio/snow.inf` that turn the board's load, slip, dig and lean into volume and bend.
//!
//! Board program = surface group * 8 + slot: 1 landing, 2 takeoff, 3 carve loop, 4 glide loop.
//! Program 0 is the slow scrape. The loops follow the ground every frame; changing surface
//! restarts them with the new group's samples. When these files are there, the stand-in
//! loops and effects in `sound.rs` stand aside.

use crate::rider::Rider;
use crate::{ui, LevelRes, Mode, RiderRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::HashMap;
use tricky_game::sfxcalc::{board_input, run_snow_program, BoardIn};

/// The snow sound programs as `Sfx_LoadSurfaceConfig` compiles them (`tricky_data::snowsfx`): per surface section
/// name, per sub-section (GLIDE, AIGLIDE, CARVE), the game's own bytes, run by `sfxcalc::run_snow_program`.
#[derive(Default)]
struct Programs(HashMap<(String, String), Vec<u8>>);

impl Programs {
    fn parse(text: &str) -> Self {
        use tricky_data::snowsfx::{compile_all, MODES, SURFACES};
        let mut out = HashMap::new();
        for (mi, progs) in compile_all(text.as_bytes()).into_iter().enumerate() {
            for (si, p) in progs.into_iter().enumerate() {
                out.insert((SURFACES[si].to_string(), MODES[mi].to_string()), p);
            }
        }
        Self(out)
    }
    /// Run a program as the game's interpreter does (`FUN_0021ae90` 0x21ae90): (volume, bend), both 0..127
    /// (bend 64 = no change). A program that divides by zero stops there (the original's `break 7`) and gives nothing.
    fn run(&self, surface: &str, part: &str, b: &BoardIn) -> Option<(f32, f32)> {
        let prog = self.0.get(&(surface.to_string(), part.to_string()))?;
        let mut w = [0i32; 0x14];
        let (mut vol, mut bend) = (0i32, 64i32);
        run_snow_program(prog, &mut w, &mut |n| board_input(n, b), &mut vol, &mut bend).ok()?;
        Some(((vol as f32).clamp(0.0, 127.0), (bend as f32).clamp(0.0, 127.0)))
    }
}

/// The rider fields the programs read (`BoardIn_*`): the game-side rider's forward in the ground plane (+0x320),
/// velocity (+0x150, cm/s), forward drag (+0x160) and steer (+0x214); before the first ground tick, the port's.
fn board_in(r: &Rider) -> BoardIn {
    match &r.gm {
        Some(c) => {
            let g = c.rider();
            BoardIn { normal: g.p.plane_fwd, vel: g.vel, f160: g.drag, f214: g.steer }
        }
        None => {
            let (f, v) = (r.forward(), r.vel * 100.0);
            BoardIn { normal: [f.x, f.y, f.z, 0.0], vel: [v.x, v.y, v.z, 0.0], f160: 0.0, f214: r.input.steer }
        }
    }
}

/// The surface groups (`Audio_SurfaceGroup`) and their snow.inf sections.
fn group(surface: u8) -> (usize, &'static str) {
    match surface {
        0 | 3 | 4 => (1, "POWDER"),
        2 => (2, "LOOSE"),
        5 | 6 => (3, "ICE"),
        13 => (4, "METAL"),
        12 => (5, "WOOD"),
        9..=11 => (7, "ROCK"),
        14 => (8, "GLASS"),
        18 | 19 => (9, "CHUTE"),
        _ => (0, "PACK"),
    }
}

/// piecewise-linear curve through points, clamped at the ends
fn curve(pts: &[(f32, f32)], x: f32) -> f32 {
    if x <= pts[0].0 { return pts[0].1; }
    for w in pts.windows(2) { if x <= w[1].0 { return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0); } }
    pts[pts.len() - 1].1
}

#[derive(Component, Clone, Copy, PartialEq)]
pub enum BoardLoop { Scrape, Glide, Carve, Wind, Boost }

#[derive(Resource, Default)]
pub struct BoardSounds { pub on: bool, files: HashMap<String, Handle<AudioSource>>, programs: Programs,
    /// per program (bank tags): root note, bend range (semitones), volume, detune (cents)
    meta: HashMap<String, (f32, f32, f32, f32)> }

/// The sound driver's pitch (`SNDVoice_CalcPitchMult`): cents = detune − (root − 60)·100 +
/// (bend − 64)·range·100/64, ratio 2^(cents/1200); bend does nothing without a range.
#[derive(Component, Clone)]
pub struct Prog(String);

impl BoardSounds {
    fn get(&self, name: &str) -> Option<Handle<AudioSource>> { self.files.get(name).cloned() }
    /// the loop part of a looping sample (else the whole sample)
    fn get_loop(&self, name: &str) -> Option<Handle<AudioSource>> { self.files.get(&format!("{name}_loop")).or(self.files.get(name)).cloned() }
    fn pitch(&self, name: &str, bend: f32) -> f32 {
        let (root, range, _, detune) = self.meta.get(name).copied().unwrap_or((60.0, 0.0, 127.0, 0.0));
        let cents = detune - (root - 60.0) * 100.0 + (bend - 64.0) * range * 100.0 / 64.0;
        2f32.powf(cents / 1200.0)
    }
    fn level(&self, name: &str) -> f32 { self.meta.get(name).map_or(1.0, |m| m.2 / 127.0) }
}

pub fn setup_board_sound(mut commands: Commands, level: Res<LevelRes>, mut assets: ResMut<Assets<AudioSource>>) {
    let audio_of = |p: &std::path::Path| p.parent().and_then(|p| p.parent()).map(|p| p.join("audio"));
    let given = std::path::absolute(&level.0.dir).unwrap_or(level.0.dir.clone());
    let real = std::fs::canonicalize(&level.0.dir).unwrap_or(given.clone());
    let Some(dir) = [audio_of(&given), audio_of(&real)].into_iter().flatten().find(|d| d.join("bnk").is_dir()) else {
        commands.insert_resource(BoardSounds::default());
        return;
    };
    let mut s = BoardSounds::default();
    for e in std::fs::read_dir(dir.join("bnk")).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "wav") {
            if let (Some(stem), Ok(bytes)) = (p.file_stem().map(|x| x.to_string_lossy().to_string()), std::fs::read(&p)) {
                s.files.insert(stem, assets.add(AudioSource { bytes: bytes.into() }));
            }
        }
    }
    s.programs = std::fs::read_to_string(dir.join("snow.inf")).map(|t| Programs::parse(&t)).unwrap_or_default();
    // the banks' program settings (written by tools/bnk/bnk2wav.py)
    for prefix in ["board", "sfx"] {
        let Ok(text) = std::fs::read_to_string(dir.join("bnk").join(format!("{prefix}.json"))) else { continue };
        let Ok(v) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&text) else { continue };
        for (k, m) in v {
            let g = |n: &str, d: f64| m.get(n).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
            s.meta.insert(format!("{prefix}_{k}"), (g("root", 60.0), g("bend", 0.0), g("vol", 127.0), g("detune", 0.0)));
        }
    }
    s.on = s.files.contains_key("board_4") && !s.programs.0.is_empty();
    println!("board sounds: {} samples, {} snow programs{}", s.files.len(), s.programs.0.len(), if s.on { "" } else { " (not used)" });
    commands.insert_resource(s);
}

/// What changed since last frame.
#[derive(Default)]
pub struct Last { vel: Vec3, grounded: bool, crashed: bool, grab: u8, boosting: bool, mult: u32, speed_t: f32, spin_t: f32, respawns: u32, uber: bool, group: Option<usize> }

#[allow(clippy::too_many_arguments)]
pub fn board_sound(
    time: Res<Time>, rider: Res<RiderRes>, game: Res<ui::Game>, mode: Res<Mode>, sounds: Res<crate::sound::Sounds>, bs: Res<BoardSounds>,
    mut commands: Commands, mut loops: Query<(Entity, &BoardLoop, &mut AudioSink, Option<&Prog>)>, mut last: Local<Last>,
) {
    if !bs.on { return; }
    let r: &Rider = &rider.0;
    let dt = time.delta_secs().max(1e-4);
    let fx = if sounds.effects && *mode == Mode::Ride && !matches!(game.screen, ui::Screen::Menu | ui::Screen::Paused) { 1.0 } else { 0.0 };
    let play = |commands: &mut Commands, name: &str, vol: f32, speed: f32| {
        if fx <= 0.0 { return; }
        if let Some(h) = bs.get(name) {
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol * bs.level(name) * 0.9), speed: speed * bs.pitch(name, 64.0), ..default() }));
        }
    };
    // the game values, in cm/s
    let fwd = r.forward();
    let load = r.vel.dot(fwd).abs() * 100.0;
    let inp = board_in(r);
    let on_ground = (r.grounded || r.rail.is_some()) && r.crashed <= 0.0;
    // on a rail: the RAIL programs and samples (group 6)
    let (g, sect) = if r.rail.is_some() { (6, "RAIL") } else { group(r.surface) };
    // the loops: restarted with the new surface's samples when the surface changes
    if last.group != Some(g) {
        last.group = Some(g);
        for (e, kind, _, _) in &loops { if matches!(kind, BoardLoop::Glide | BoardLoop::Carve) { commands.entity(e).despawn(); } }
        for (kind, slot) in [(BoardLoop::Glide, 4), (BoardLoop::Carve, 3)] {
            let name = format!("board_{}", g * 8 + slot);
            if let Some(h) = bs.get_loop(&name) {
                commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, kind, Prog(name)));
            }
        }
    }
    if !loops.iter().any(|l| *l.1 == BoardLoop::Scrape) {
        if let Some(h) = bs.get_loop("board_0") { commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, BoardLoop::Scrape, Prog("board_0".into()))); }
        if let Some(h) = bs.get_loop("sfx_32") { commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, BoardLoop::Wind, Prog("sfx_32".into()))); }
    }
    let glide = bs.programs.run(sect, "GLIDE", &inp).unwrap_or((0.0, 64.0));
    let carve = bs.programs.run(sect, "CARVE", &inp).unwrap_or((0.0, 64.0));
    // big air: the wind comes up as the music ducks (Audio_BigAirMusicDuck)
    let t_air = if !r.grounded && r.rail.is_none() { r.air_time } else { 0.0 };
    let duck = if t_air > 1.5 && r.crashed <= 0.0 { (1.0 - (1.0 - 16.0 / 127.0) * 0.5 * t_air).clamp(16.0 / 127.0, 1.0) } else { 1.0 };
    let crash_fade = if r.crashed > 0.0 { 0.0 } else { 1.0 };
    for (_, kind, mut sink, prog) in &mut loops {
        let name = prog.map_or("", |p| p.0.as_str());
        let (vol, b) = match kind {
            BoardLoop::Scrape if r.grounded && r.rail.is_none() => (curve(&[(10.0, 0.0), (60.0, 127.0), (200.0, 100.0), (300.0, 0.0)], load) / 127.0, curve(&[(10.0, 0.0), (30.0, 30.0), (500.0, 80.0), (900.0, 127.0)], load)),
            BoardLoop::Glide if on_ground => (glide.0 / 127.0, glide.1),
            BoardLoop::Carve if on_ground => (carve.0 / 127.0, carve.1),
            BoardLoop::Wind => (1.0 - duck, 64.0),
            _ => (0.0, 64.0),
        };
        let speed = bs.pitch(name, b);
        let want = vol * bs.level(name) * fx * crash_fade * 0.8;
        let now = sink.volume().to_linear();
        sink.set_volume(Volume::Linear(now + (want - now) * (1.0 - (-dt * 30.0).exp())));
        sink.set_speed(speed.clamp(0.25, 4.0));
    }
    // one-shots
    let landed = r.grounded && !last.grounded && r.crashed <= 0.0;
    if landed {
        // the impact: speed into the snow at touchdown (cm/s)
        let impact = (-last.vel.dot(r.normal)).max(0.0) * 100.0;
        play(&mut commands, &format!("board_{}", g * 8 + 1), curve(&[(100.0, 22.0), (300.0, 60.0), (800.0, 100.0), (1200.0, 127.0)], impact) / 127.0, 1.0);
    }
    if !r.grounded && last.grounded && r.rail.is_none() && r.crashed <= 0.0 {
        play(&mut commands, &format!("board_{}", g * 8 + 2), (load * 127.0 / 1000.0).clamp(64.0, 127.0) / 127.0, 1.0);
    }
    if r.crashed > 0.0 && !last.crashed {
        let impact = last.vel.length() * 100.0;
        let id = if matches!(r.surface, 3 | 4) { 64 } else { 48 + (time.elapsed_secs() * 1000.0) as u32 % 3 };
        play(&mut commands, &format!("sfx_{id}"), curve(&[(100.0, 80.0), (800.0, 90.0), (1200.0, 105.0), (1600.0, 127.0)], impact) / 127.0, 1.0);
    }
    if r.grab != 0 && last.grab == 0 { play(&mut commands, "sfx_119", 0.7, 1.0); }
    if r.multiplier > last.mult { play(&mut commands, &format!("sfx_{}", if r.multiplier >= 5 { 118 } else if r.multiplier >= 3 { 117 } else { 116 }), 1.0, 1.0); }
    if r.speed_timer > last.speed_t + 0.5 { play(&mut commands, "sfx_115", 1.0, 1.0); }
    if r.spin_timer > last.spin_t + 0.5 { play(&mut commands, "sfx_114", 1.0, 1.0); }
    if r.respawns > last.respawns { play(&mut commands, "sfx_123", 1.0, 1.0); }
    if r.uber_ready() && !last.uber { play(&mut commands, "sfx_107", 1.0, 1.0); }
    // boost: a sound by the meter's level for as long as the button is held
    if r.boosting && !last.boosting {
        let id = if r.boost > 0.666 || r.tricky() { 120 } else if r.boost > 0.334 { 121 } else { 122 };
        if let Some(h) = bs.get(&format!("sfx_{id}")) { if fx > 0.0 { commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(0.8), ..default() }, BoardLoop::Boost)); } }
    }
    if !r.boosting && last.boosting {
        for (e, kind, _, _) in &loops { if *kind == BoardLoop::Boost { commands.entity(e).despawn(); } }
        if r.boost <= 0.0 { play(&mut commands, "sfx_108", 0.8, 1.0); }
    }
    *last = Last { vel: r.vel, grounded: r.grounded || r.rail.is_some(), crashed: r.crashed > 0.0, grab: r.grab, boosting: r.boosting, mult: r.multiplier, speed_t: r.speed_timer, spin_t: r.spin_timer,
        respawns: r.respawns, uber: r.uber_ready(), group: last.group };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snow_inf() {
        let text = "[PACK]\n[ICE]\n    [GLIDE]\n        Load Dig\n        Map 127\n        AssignVol\n        Load 64\n        AssignBend\n[POWDER]\n    [GLIDE]\n        Load Slip\n        Bound 0, 10\n        AssignVol\n";
        let p = Programs::parse(text);
        let b = BoardIn { normal: [1.0, 0.0, 0.0, 0.0], vel: [300.0, 0.0, 0.0, 0.0], f160: -50.0, f214: 0.5 };
        let ice = p.run("ICE", "GLIDE", &b).unwrap();
        let pow = p.run("POWDER", "GLIDE", &b).unwrap();
        assert_eq!(ice.1, 64.0);
        assert!(ice.0 >= 0.0 && ice.0 <= 127.0);
        assert!(pow.0 <= 10.0);
    }
}

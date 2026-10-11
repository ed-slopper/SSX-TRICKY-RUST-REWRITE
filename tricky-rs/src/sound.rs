//! Sound. Plays the game's own effects and music if they have been extracted into an `audio`
//! folder next to `levels` (see the README); silent otherwise.

use crate::props::Kind;
use crate::rider::RaceState;
use crate::{ui, LevelRes, Mode, Props, RaceRes, RiderRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::HashMap;

// STANDIN: effect samples picked from the banks by ear (audio/<name>.wav, README "Sound and snow"), and when to play them, for the
// game's own sound programs where boardsound.rs and worldsound.rs have none yet
const SHOTS: [&str; 13] = ["land", "jump", "rail_on", "crash", "glass", "menu_move", "countdown", "pickup", "tricky", "go", "boost", "reset", "levelup"];

#[derive(Component, Clone, Copy, PartialEq)]
pub enum Loop { Ride, Air, Grind, Slide, Crowd, Music(usize) }

#[derive(Resource, Default)]
pub struct Sounds { shots: HashMap<&'static str, Handle<AudioSource>>, pub music: bool, pub effects: bool, songs: usize, pub dir: std::path::PathBuf }

pub fn setup_sound(mut commands: Commands, level: Res<LevelRes>, mut assets: ResMut<Assets<AudioSource>>) {
    // `audio` sits next to `levels`; look from the path as given and from where it really is
    let audio_of = |p: &std::path::Path| p.parent().and_then(|p| p.parent()).map(|p| p.join("audio"));
    let given = std::path::absolute(&level.0.dir).unwrap_or(level.0.dir.clone());
    let real = std::fs::canonicalize(&level.0.dir).unwrap_or(given.clone());
    let dir = [audio_of(&given), audio_of(&real)].into_iter().flatten().find(|d| d.is_dir()).unwrap_or_default();
    let mut load = |name: &str| -> Option<Handle<AudioSource>> {
        let bytes = std::fs::read(dir.join(name)).ok()?;
        Some(assets.add(AudioSource { bytes: bytes.into() }))
    };
    let mut sounds = Sounds { music: true, effects: true, ..default() };
    for n in SHOTS { if let Some(h) = load(&format!("{n}.wav")) { sounds.shots.insert(n, h); } }
    let mut loops = 0;
    for (kind, file) in [(Loop::Ride, "ride.wav"), (Loop::Air, "air.wav"), (Loop::Grind, "grind.wav"), (Loop::Slide, "slide.wav"), (Loop::Crowd, "crowd.wav")] {
        if let Some(h) = load(file) {
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, kind));
            loops += 1;
        }
    }
    println!("sound: {} effects and {} loops from {}", sounds.shots.len(), loops, dir.display());
    sounds.dir = dir;
    commands.insert_resource(sounds);
}

/// Each course has its own songs, kept once each in `audio/music/<song>.ogg`.
pub fn course_music(
    mut commands: Commands, list: Res<crate::LevelList>, mut sounds: ResMut<Sounds>, mut assets: ResMut<Assets<AudioSource>>,
    old: Query<(Entity, &Loop)>, mut current: Local<Option<usize>>,
) {
    if *current == Some(list.current) { return; }
    *current = Some(list.current);
    for (e, kind) in &old { if matches!(kind, Loop::Music(_)) { commands.entity(e).despawn(); } }
    let name = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // the songs DATA/CONFIG/MUSICMAP.INF gives each course, by their file names on the disc
    let songs = course_songs(&name);

    sounds.songs = 0;
    let folder = sounds.dir.join("music");
    for song in songs {
        let Ok(bytes) = std::fs::read(folder.join(format!("{song}.ogg"))) else { continue };
        let h = assets.add(AudioSource { bytes: bytes.into() });
        commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, Loop::Music(sounds.songs)));
        sounds.songs += 1;
    }
    println!("music: {} songs from {}", sounds.songs, folder.display());
}


/// The songs DATA/CONFIG/MUSICMAP.INF gives each course, by their file names on the disc.
pub fn course_songs(track: &str) -> &'static [&'static str] {
    match track {
        "gari" => &["systemover", "smartbomb", "adamsrev"],
        "snowdream" => &["ginandsin", "shakemomma", "songfordot"],
        "elysium" => &["peaktime", "downtime", "superwoman"],
        "mesablanca" => &["topbomb", "hiphop", "bassinvader"],
        "merqury" => &["bburner", "reality", "slayboarder"],
        "megaplex" => &["reality", "hiphop", "downtime"],
        "aloha" => &["kingbeat", "songfordot", "adamsrev"],
        "alaska" => &["moveit", "smartbomb", "systemover"],
        "untracked" => &["finsym"],
        "pipedream" => &["bburner", "slayboarder"],
        "trick" => &["slaybreak"],
        _ => &["smartbomb"],
    }
}

#[derive(Default)]
pub struct Prev { on_snow: bool, air: f32, crashed: bool, rail: bool, uber: bool, gone: usize, cursor: usize, screen: Option<ui::Screen>, count: i32, master: bool, song: usize, boosting: bool, tier: u32, respawns: u32, pause: usize }

pub fn sound(
    time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, rider: Res<RiderRes>, race: Res<RaceRes>, game: Res<ui::Game>, props: Res<Props>, mode: Res<Mode>,
    mut sounds: ResMut<Sounds>, mut sinks: Query<(&Loop, &mut AudioSink)>, mut commands: Commands, mut prev: Local<Prev>,
    (board, world, course, pf): (Res<crate::boardsound::BoardSounds>, Res<crate::worldsound::WorldSounds>, Res<crate::coursemusic::CourseMusic>, Res<crate::pathmusic::PathMusic>),
) {
    // with the game's own board sounds the stand-ins for them stay quiet
    let original = board.on;
    if keys.just_pressed(KeyCode::KeyM) { sounds.music = !sounds.music; }
    if keys.just_pressed(KeyCode::KeyN) { sounds.effects = !sounds.effects; }
    let r = &rider.0;
    let riding = *mode == Mode::Ride && !matches!(game.screen, ui::Screen::Menu | ui::Screen::Paused) && sounds.effects;
    let speed = r.vel.length();
    let fast = (speed / 28.0).min(1.0);
    let on_snow = r.grounded && r.rail.is_none();
    let in_air = !r.grounded && r.rail.is_none();
    let near_crowd = game.screen != ui::Screen::Playing || race.0.state != RaceState::Running || race.0.progress() < 0.04 || race.0.progress() > 0.94;
    // a big air ducks the music and brings up the wind (the original's music factor:
    // clamp(1 - 0.437 T, 16/127, 1), T the seconds into the big part of the air)
    let big = if in_air && r.crashed <= 0.0 { (prev.air - 1.0).max(0.0) } else { 0.0 };
    let duck = (1.0 - 0.437 * big).clamp(16.0 / 127.0, 1.0);
    for (kind, mut sink) in &mut sinks {
        let (vol, pitch) = match kind {
            Loop::Ride | Loop::Air | Loop::Grind | Loop::Slide if original => (0.0, 1.0),
            Loop::Ride if riding && on_snow && r.crashed <= 0.0 => {
                let carve = r.input.steer.abs() * 0.3 * (speed / 10.0).min(1.0) + if r.input.brake { 0.3 * (speed / 6.0).min(1.0) } else { 0.0 };
                ((fast * 0.45 + carve).min(0.9), 0.75 + 0.5 * fast - 0.1 * r.input.steer.abs())
            }
            Loop::Air if riding && in_air => ((0.12 + 0.25 * fast) * (2.0 - duck), 0.8 + 0.4 * fast),
            Loop::Grind if riding && r.rail.is_some() => (0.55, 0.8 + 0.4 * fast),
            Loop::Slide if riding && on_snow && r.crashed <= 0.0 && r.input.brake => (0.55 * (speed / 8.0).min(1.0), 0.85 + 0.3 * fast),
            Loop::Crowd if sounds.effects && *mode == Mode::Ride && near_crowd && !world.on => (if game.screen == ui::Screen::Menu { 0.18 } else { 0.4 }, 1.0),
            Loop::Music(i) if sounds.music && *i == prev.song % sounds.songs.max(1) && !course.holding && !pf.active => (if game.screen == ui::Screen::Menu { 0.5 } else { 0.4 * duck }, 1.0),
            _ => (0.0, 1.0),
        };
        // ease towards the level wanted so nothing clicks on or off
        let now = sink.volume().to_linear();
        let k = 1.0 - (-12.0 * time.delta_secs()).exp();
        sink.set_volume(Volume::Linear(now + (vol - now) * k));
        if !matches!(kind, Loop::Music(_) | Loop::Crowd) { sink.set_speed(pitch); }
    }

    // one-off sounds, from what changed since the last frame
    let mut play_at = |name: &str, vol: f32, speed: f32| {
        if !sounds.effects { return; }
        if original && matches!(name, "land" | "jump" | "rail_on" | "crash" | "pickup" | "boost" | "reset" | "tricky") { return; }
        if let Some(h) = sounds.shots.get(name) {
            commands.spawn((AudioPlayer::new(h.clone()), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol), speed, ..default() }));
        }
    };
    let tier = (r.boost * 3.0 + 0.001) as u32;
    let mut go = false;
    let gone = props.0.iter().filter(|p| p.hidden && matches!(p.kind, Kind::Touch { .. })).count();
    if game.screen != ui::Screen::Menu && *mode == Mode::Ride {
        if on_snow && !prev.on_snow && prev.air > 0.3 && r.crashed <= 0.0 { play_at("land", (0.4 + prev.air * 0.3).min(1.0), 1.0); }
        if !r.grounded && prev.on_snow && r.vel.y > 3.0 { play_at("jump", 0.7, 1.0); }
        if r.rail.is_some() && !prev.rail { play_at("rail_on", 0.8, 1.0); }
        if r.crashed > 0.0 && !prev.crashed { play_at("crash", 1.0, 1.0); }
        if r.uber_ready() && !prev.uber { play_at("tricky", 0.9, 1.0); }
        if gone > prev.gone {
            let glass = props.0.iter().any(|p| p.hidden && matches!(&p.kind, Kind::Touch { pieces, .. } if !pieces.is_empty()) && (p.home.0 - r.pos).length() < 12.0);
            play_at(if glass { "glass" } else { "pickup" }, 0.8, 1.0);
        }
        // the countdown beeps every half second, four times (0.5, 1.0, 1.5, 2.0 s), and GO comes at 2.5 s
        let count = if race.0.state == RaceState::Countdown { (race.0.countdown * 2.0).ceil() as i32 } else { 0 };
        // the boost sound goes with how full the meter is
        if r.boosting && !prev.boosting { play_at("boost", 0.6 + 0.1 * r.boost_level(), 0.9 + 0.1 * r.boost_level()); }
        if tier > prev.tier { play_at("levelup", 0.8, 1.0); }
        if r.respawns > prev.respawns && prev.screen == Some(game.screen) && race.0.state == RaceState::Running { play_at("reset", 0.8, 1.0); }
        if count != prev.count && game.screen == ui::Screen::Playing { if count == 0 && prev.count > 0 { go = true; } else if (1..=4).contains(&count) { play_at("countdown", 0.9, 1.0); } }
        prev.count = count;
    }
    if game.cursor + game.row * 10 + game.board * 100 != prev.cursor || game.master != prev.master { play_at("menu_move", 0.7, 1.0); }
    if prev.screen.is_some() && prev.screen != Some(game.screen) { play_at("menu_move", 0.9, 1.0); }
    if game.screen == ui::Screen::Paused && game.pause_cursor != prev.pause { play_at("menu_move", 0.7, 1.0); }
    // GO: its own sound if one has been found (the original has none of its own: the crowd and the music carry it)
    if go && sounds.shots.contains_key("go") { play_at("go", 0.9, 1.0); }
    // each new run moves on to the course's next song
    let song = prev.song + (prev.screen == Some(ui::Screen::Menu) && game.screen == ui::Screen::Playing) as usize;
    prev.air = if r.grounded || r.rail.is_some() { 0.0 } else { prev.air + time.delta_secs() };
    *prev = Prev { on_snow, air: prev.air, crashed: r.crashed > 0.0, rail: r.rail.is_some(), uber: r.uber_ready(), gone, cursor: game.cursor + game.row * 10 + game.board * 100, screen: Some(game.screen), count: prev.count, master: game.master, song, boosting: r.boosting, tier, respawns: r.respawns, pause: game.pause_cursor };
}

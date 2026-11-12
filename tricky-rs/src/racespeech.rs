//! The race's speech heard (board row E1b2g7d3b3): the game's speech system (`tricky_game::speechhost`, the voice
//! banks from the player's disc, TRICKY_DISC) runs the announcer's calls the race queues (`race::Race::speech`)
//! through `speech::Announcer`, and the lines it starts are decoded from the disc's speech files
//! (`tricky_game::sndmt`, MicroTalk) and played, each stream's parts one after another as the game's stream queue
//! plays them (`SndStream_Queue` 0x2d2728).

use crate::rider::RaceState;
use crate::{ui, Mode, RaceRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tricky_game::race::SpeechCall;
use tricky_game::speech::{Announcer, Event, PlaceRace, SpeechEnv, SpeechRider, Trigger};
use tricky_game::speechhost::SpeechHost;

type Files = tricky_data::big::Files;

fn disc() -> Option<Files> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    Some(files)
}

/// The speech system and its files, loaded off the main thread (115 banks).
struct Loaded {
    host: SpeechHost,
    files: Files,
}

/// A stream's parts: the one playing (its sound and seconds left) and those queued behind it.
#[derive(Default)]
struct Stream {
    playing: Option<(Entity, f32)>,
    queued: VecDeque<(Handle<AudioSource>, f32)>,
}

#[derive(Resource, Default)]
pub struct RaceSpeech {
    tried: bool,
    loading: Option<Arc<Mutex<Option<Option<Loaded>>>>>,
    sys: Option<Loaded>,
    announcer: Announcer,
    /// the race's speech calls already run
    sent: usize,
    streams: HashMap<u32, Stream>,
}

#[derive(Component)]
pub struct SpeechPlayer;

pub fn plugin(app: &mut App) {
    app.init_resource::<RaceSpeech>();
}

/// The speech level `Announcer::roll` reads (+0x383c, `Audio_SetSpeechLevel` 0x212210 clamps it to 0..1) and its cap
/// (+0x6e68).
// STANDIN: 0 and 0, for the profile's speech setting (`Profile_ApplyAudioSettings` 0x161a98) and the cap's writer
// (not found yet); with a level under 1 the roll reads the chance table's first column either way.
fn announcer() -> Announcer {
    Announcer { level: 0.0, cap: 0, ..Default::default() }
}

/// What the announcer's race-line handlers ask of the game (`SpeechEnv`) for the riders of `race`.
struct Env<'a> {
    host: &'a mut SpeechHost,
    mode: u32,
    course: u32,
    clock: i32,
}

/// The rider object's id the speech system compares (the listener, the game object's rider words).
// STANDIN: the race index + 1, for the rider object's address.
fn rider_id(k: usize) -> u32 {
    k as u32 + 1
}

impl SpeechEnv for Env<'_> {
    fn rand_fx(&mut self) -> u32 {
        tricky_game::rand::rand_fx()
    }
    // STANDIN: the app has no replays
    fn replay_playing(&mut self) -> bool {
        false
    }
    /// `Audio_GetListenerRider`(−1): the player (rider 0), whom the sound follows
    fn listener(&mut self) -> u32 {
        rider_id(0)
    }
    fn mode(&mut self) -> u32 {
        self.mode
    }
    fn post(&mut self, e: Event) -> u32 {
        self.host.post(&e.words)
    }
    fn course(&mut self) -> u32 {
        self.course
    }
    fn race_clock(&mut self) -> i32 {
        self.clock
    }
    fn race_rider(&mut self, i: u32) -> u32 {
        rider_id(i as usize)
    }
    // STANDIN: the rest serve the announcer's other handlers (tricks, wipe-outs, the riders' own voices, row
    // E1b2g7d3b4), which the race does not call yet; they answer "nothing there".
    fn can_play(&mut self, _: u32) -> bool {
        false
    }
    fn find_near(&mut self, _: u32) -> i32 {
        -1
    }
    fn relation(&mut self, _: u32, _: u32) -> i32 {
        0
    }
    fn line_state(&mut self, _: u32) -> Option<Option<i32>> {
        None
    }
    fn handle_done(&mut self, _: u32) -> bool {
        true
    }
    fn trigger(&mut self, _: Trigger) -> Vec<u32> {
        Vec::new()
    }
    fn app_mode(&mut self) -> u32 {
        0
    }
    fn slot_hold(&mut self, _: u32, _: u32) {}
    fn clear_event(&mut self, _: u32) {}
    fn record_after(&mut self, _: usize) -> i32 {
        -1
    }
}

/// What the handlers read of rider `k` (`speech::SpeechRider`): +0x418 the finish time (below 0 still racing: the
/// handlers' "player" test), +0x41c 1 for a local player, +0x38 the race index.
// STANDIN: the character's level (+0x464 → +0x74, the line's grade) 0, for the character entry's.
fn speech_rider(race: &tricky_game::race::Race, k: usize) -> SpeechRider {
    let finish = race.line_riders.get(k).map_or(-1, |r| r.line.finish);
    SpeechRider { id: rider_id(k), controller: finish, kind: (k == 0) as i32, index: k as i32, ..Default::default() }
}

/// A part's samples as a WAV file Bevy plays.
fn wav(s: &tricky_game::sndmt::Sound) -> Vec<u8> {
    let data = (s.samples.len() * 2) as u32;
    let ch = s.channels as u16;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&ch.to_le_bytes());
    w.extend_from_slice(&s.rate.to_le_bytes());
    w.extend_from_slice(&(s.rate * 2 * ch as u32).to_le_bytes());
    w.extend_from_slice(&(2 * ch).to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for x in &s.samples {
        w.extend_from_slice(&x.to_le_bytes());
    }
    w
}

/// The announcer during a race: its calls run, the speech system updated, its lines played.
#[allow(clippy::too_many_arguments)]
pub fn race_speech(
    mut rs: ResMut<RaceSpeech>, sounds: Res<crate::sound::Sounds>, game: Res<ui::Game>, mode: Res<Mode>, race: Res<RaceRes>, time: Res<Time>,
    mut audio: ResMut<Assets<AudioSource>>, mut commands: Commands,
) {
    if !rs.tried {
        rs.tried = true;
        rs.announcer = announcer();
        if std::env::var("TRICKY_DISC").is_ok() {
            let slot = Arc::new(Mutex::new(None));
            let s2 = slot.clone();
            std::thread::spawn(move || {
                let loaded = (|| {
                    let mut files = disc()?;
                    let inf = files.load("data/config/speech.inf").ok().flatten()?;
                    let mut banks = disc()?;
                    let host = SpeechHost::new(&inf, Box::new(move |p: &str| banks.load(p).ok().flatten()));
                    Some(Loaded { host, files })
                })();
                *s2.lock().unwrap() = Some(loaded);
            });
            rs.loading = Some(slot);
        }
    }
    if let Some(slot) = &rs.loading {
        let done = slot.lock().unwrap().take();
        match done {
            Some(l) => {
                println!("speech: {}", if l.is_some() { "the game's speech system on the disc's banks" } else { "the disc's speech could not be loaded" });
                rs.sys = l;
                rs.loading = None;
            }
            None => return,
        }
    }
    let rs = &mut *rs;
    let Some(sys) = &mut rs.sys else { return };
    let calls = &race.0.speech[..];
    let racing = *mode == Mode::Ride && game.screen != ui::Screen::Menu && race.0.state != RaceState::Countdown;
    // a new run: the calls start again, the lines stop
    if calls.len() < rs.sent || !racing {
        rs.sent = calls.len();
        for s in rs.streams.values_mut() {
            if let Some((e, _)) = s.playing.take() {
                commands.entity(e).try_despawn();
            }
            s.queued.clear();
        }
        if !racing {
            return;
        }
    }
    if game.screen == ui::Screen::Paused {
        return;
    }
    let r = &race.0;
    let mut env = Env { host: &mut sys.host, mode: r.mode, course: r.course_no, clock: r.line_frame };
    for &c in &calls[rs.sent..] {
        if std::env::var("TRICKY_SPEECHLOG").is_ok() {
            println!("speech: {c:?}");
        }
        match c {
            SpeechCall::Checkpoint(k) => rs.announcer.on_checkpoint(&mut env, &speech_rider(r, k)),
            SpeechCall::PlaceChange(k, old, new) => {
                // STANDIN: the game object's two rider words (gApp+0x730: +0xa4, +0x124) none
                let place = PlaceRace { count: r.line_riders.len() as i32, a4: 0, a124: 0 };
                rs.announcer.on_place_change(&mut env, &speech_rider(r, k), place, old, new as i64)
            }
            SpeechCall::LocalLine(k, kind) => rs.announcer.on_local_line(&mut env, &speech_rider(r, k), kind as i64),
            SpeechCall::LocalLine2(k) => rs.announcer.on_local_line2(&mut env, &speech_rider(r, k)),
            SpeechCall::ListenerLine(k) => rs.announcer.on_listener_line(&mut env, &speech_rider(r, k)),
            // the crowd's cheer and the placing sound are the sound queue's (row E1b2g7d3b5)
            SpeechCall::Cheer(_) | SpeechCall::PlacingChange { .. } => {}
        }
    }
    rs.sent = calls.len();
    // `SpeechSys_UpdateAll` 0x228998, once a frame; its lines decoded and queued on their streams
    let dt = time.delta_secs();
    sys.host.update();
    for s in sys.host.take_stops() {
        if let Some(st) = rs.streams.get_mut(&s) {
            if let Some((e, _)) = st.playing.take() {
                commands.entity(e).try_despawn();
            }
            st.queued.clear();
        }
    }
    for p in sys.host.take_plays() {
        let sound = sys.files.load(&p.file).ok().flatten().and_then(|b| tricky_game::sndmt::decode(&b, p.offset as usize));
        let Some(s) = sound else {
            println!("speech: {} at {:#x} could not be decoded", p.file, p.offset);
            sys.host.line_started(p.handle, 0.0);
            continue;
        };
        let secs = (s.samples.len() / s.channels.max(1)) as f32 / s.rate as f32;
        if std::env::var("TRICKY_SPEECHLOG").is_ok() {
            println!("speech: stream {} plays {} at {:#x}, {secs:.2} s", p.stream, p.file, p.offset);
        }
        sys.host.line_started(p.handle, secs * 1000.0);
        let h = audio.add(AudioSource { bytes: wav(&s).into() });
        rs.streams.entry(p.stream).or_default().queued.push_back((h, secs));
    }
    sys.host.frame(dt);
    let vol = if sounds.effects { 1.0 } else { 0.0 };
    for st in rs.streams.values_mut() {
        if let Some((e, left)) = &mut st.playing {
            *left -= dt;
            if *left <= 0.0 {
                commands.entity(*e).try_despawn();
                st.playing = None;
            }
        }
        if st.playing.is_none() {
            if let Some((h, secs)) = st.queued.pop_front() {
                let e = commands.spawn((AudioPlayer(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol), ..default() }, SpeechPlayer)).id();
                st.playing = Some((e, secs));
            }
        }
    }
}

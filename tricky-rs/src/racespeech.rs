//! The race's speech heard (board row E1b2g7d3b3): the game's speech system (`tricky_game::speechhost`, the voice
//! banks from the player's disc, TRICKY_DISC) runs the announcer's calls the race queues (`race::Race::speech`)
//! through `speech::Announcer`, and the lines it starts are decoded from the disc's speech files
//! (`tricky_game::sndmt`, MicroTalk) and played, each stream's parts one after another as the game's stream queue
//! plays them (`SndStream_Queue` 0x2d2728). The placing sound (`Sfx_OnPlacingChange` 0x219a48) is played from the
//! main bank (zbxsfx, kind 0) as `boardsound` plays it, and kept on the audio system's ring of queued sounds
//! (`sndqueue::SoundRing`).

use crate::rider::RaceState;
use crate::{ui, Mode, RaceRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tricky_game::race::SpeechCall;
use tricky_game::rider::RiderSpeech;
use tricky_game::speech::{Announcer, Event, PlaceRace, SpeechEnv, SpeechRider, Trigger};
use tricky_game::sndqueue::{self, PlacingChangeEnv, Queue, QueueEnv, RingEnv, SoundRing};
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
    /// DATA/CONFIG/BANKS.INF
    banks: Option<Vec<u8>>,
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
    /// the audio system's pending-sound queue and its ring of queued sounds (+0x6e24)
    queue: Queue,
    ring: SoundRing,
    /// the rider event bits' sound (+0x7420)
    event: sndqueue::EventSound,
    /// the course's ambient banks (BANKS.INF's `SWAP` list, +0x38e0) and the one in the bank manager's slot 6
    ambient: sndqueue::AmbientBanks,
    slot6: Slot6,
    /// the course the banks are for, and its `BANK` (slot 2)
    course: Option<usize>,
    course_bank: Vec<u8>,
    /// the banks' samples (`audio/world/`)
    world: std::path::PathBuf,
    /// the bank sounds playing, by handle, and the last handle given
    bank: HashMap<u32, Entity>,
    handle: u32,
}

#[derive(Component)]
pub struct PlacingSound;

#[derive(Component)]
pub struct SpeechPlayer;

pub fn plugin(app: &mut App) {
    app.insert_resource(RaceSpeech { queue: Queue::with_capacity(8), ..default() });
}

/// Whether bank sound `h` still plays (`BankMonitor_Find`), and its stop (`SndSys_StopBankSound`).
struct Bank<'a> {
    bank: &'a mut HashMap<u32, Entity>,
    alive: &'a std::collections::HashSet<Entity>,
    stops: Vec<Entity>,
}

impl RingEnv for Bank<'_> {
    fn find(&mut self, h: u32) -> bool {
        self.bank.get(&h).is_some_and(|e| self.alive.contains(e))
    }
    fn stop(&mut self, h: u32) {
        if let Some(e) = self.bank.remove(&h) {
            self.stops.push(e);
        }
    }
}

/// What the event sound (`sndqueue::EventSound`) asks of the audio system.
struct EventSfx<'a, 'b> {
    bank: Bank<'a>,
    handle: &'b mut u32,
    ambient: &'b mut sndqueue::AmbientBanks,
    slot6: &'b mut Slot6,
}

/// The bank manager's slot 6, the ambient bank in it.
// STANDIN: the bank manager (`BankManager_LoadBank` 0x20a520 / `BankManager_UnloadBank` 0x20a508): a load is in at
// once (the bank's samples are read from `audio/world/` when played)
#[derive(Default)]
struct Slot6 {
    state: i32,
    name: Vec<u8>,
}

impl sndqueue::AmbientEnv for Slot6 {
    fn state(&mut self) -> i32 {
        self.state
    }
    fn unload(&mut self) {
        self.state = 0;
        self.name.clear();
    }
    fn load(&mut self, name: &[u8]) {
        self.state = 1;
        self.name = name.to_vec();
    }
}

impl QueueEnv for EventSfx<'_, '_> {
    // STANDIN: the sound manager's slots (`SndMgr_GetSlot` 0x211f38)
    fn get_slot(&mut self, _: u32) -> u32 {
        0
    }
    fn rider_listener(&mut self) -> u32 {
        rider_id(0)
    }
    /// `SndSys_PlayPending` 0x20c948: a handle given (the entry is played and taken off by the caller, [`play_bank_sound`])
    fn play_pending(&mut self) -> u64 {
        *self.handle += 1;
        *self.handle as u64
    }
}

impl sndqueue::EventSoundEnv for EventSfx<'_, '_> {
    /// `Audio_FindBankName` 0x21a640 then `Audio_LoadAmbientBank` 0x21a548
    fn load_ambient_bank(&mut self, name: &str) -> i64 {
        let index = self.ambient.find(name.as_bytes());
        // (not in the list the game would read the word before the flags; every course lists both banks)
        if index < 0 {
            return -1;
        }
        self.ambient.load(index as usize, self.slot6)
    }
    fn stop_bank_sound(&mut self, h: u32) {
        self.bank.stop(h);
    }
}

/// The bank file a queue entry's kind names (`SndSys_PlayPending` 0x20c948 plays from the kind's bank): kind 2 the
/// course's `BANK` of BANKS.INF, kind 6 the ambient bank in slot 6.
fn kind_bank(kind: u32, course: &[u8], slot6: &Slot6) -> Option<String> {
    let path = match kind {
        2 => course,
        6 if slot6.state == 1 => &slot6.name,
        _ => return None,
    };
    let s = String::from_utf8_lossy(path);
    let file = s.rsplit(['\\', '/']).next()?;
    Some(file.trim_end_matches(".bnk").trim_end_matches(".BNK").to_string())
}

/// The queue's top entry played and taken off (`SndSys_PlayPending` 0x20c948): sound `id` of its kind's bank at the
/// record's volume byte, looping when the bank's program loops.
// STANDIN: the sound driver (`SND_PlayBankSound` 0x2d1718), for the bank's sample exported to `audio/world/<bank>/`
// (tools/bnk/levelaudio.py on AUDIO.BIG), its pitch and level from bnk2wav's json beside it
fn play_bank_sound(q: &mut Queue, bank: Option<String>, dir: &std::path::Path, vol: f32, audio: &mut Assets<AudioSource>, commands: &mut Commands) -> Option<Entity> {
    let i = q.count.max(0) as usize;
    q.count -= 1;
    let (id, volume) = (q.id[i], q.record[i][0]);
    let bank = bank?;
    let folder = dir.join(&bank);
    let looped = folder.join(format!("{bank}_{id}_loop.wav"));
    let (file, mode) = if looped.is_file() { (looped, PlaybackMode::Loop) } else { (folder.join(format!("{bank}_{id}.wav")), PlaybackMode::Despawn) };
    if std::env::var("TRICKY_SPEECHLOG").is_ok() {
        println!("speech: the event sound plays {} at volume {volume}", file.display());
    }
    let bytes = std::fs::read(&file).ok()?;
    let (mut root, mut level, mut detune) = (60.0, 127.0, 0.0);
    if let Some(e) = std::fs::read_to_string(folder.join(format!("{bank}.json"))).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v.get(id.to_string()).cloned()) {
        let g = |k: &str, d: f64| e.get(k).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
        (root, level, detune) = (g("root", 60.0), g("vol", 127.0), g("detune", 0.0));
    }
    let speed = 2f32.powf((detune - (root - 60.0) * 100.0) / 1200.0);
    let settings = PlaybackSettings { mode, volume: Volume::Linear(vol * volume as f32 / 127.0 * level / 127.0), speed, ..default() };
    Some(commands.spawn((AudioPlayer(audio.add(AudioSource { bytes: bytes.into() })), settings, PlacingSound)).id())
}

/// What `Sfx_OnPlacingChange` asks of the audio system, with the announcer's passing line behind it.
struct Placing<'a, 'b> {
    bank: Bank<'a>,
    ring: &'a mut SoundRing,
    handle: &'a mut u32,
    /// the handle `play_pending` gave, its sound started once the queue is back
    played: Option<u32>,
    announcer: &'a mut Announcer,
    env: Env<'b>,
    race: &'a tricky_game::race::Race,
}

impl QueueEnv for Placing<'_, '_> {
    // STANDIN: the sound manager's slots (`SndMgr_GetSlot` 0x211f38), for the volume `SndSys_PlayPending` scales by
    fn get_slot(&mut self, _: u32) -> u32 {
        0
    }
    fn rider_listener(&mut self) -> u32 {
        rider_id(0)
    }
    /// `SndSys_PlayPending` 0x20c948: the queue's top sound played ([`play_top`]); its handle.
    fn play_pending(&mut self) -> u64 {
        *self.handle += 1;
        self.played = Some(*self.handle);
        *self.handle as u64
    }
}

/// The queue's top sound played and taken off (`SndSys_PlayPending` 0x20c948): kind 0 is the main bank, zbxsfx's
/// program played as `boardsound` plays it.
// STANDIN: the sound driver (`SND_PlayBankSound` 0x2d1718), for the bank's exported sample (tools/bnk/bnk2wav.py)
fn play_top(q: &mut Queue, sfx: Option<&crate::boardsound::BoardSounds>, vol: f32, commands: &mut Commands) -> Option<Entity> {
    let i = q.count.max(0) as usize;
    q.count -= 1;
    let name = format!("sfx_{}", q.id[i]);
    if std::env::var("TRICKY_SPEECHLOG").is_ok() {
        println!("speech: the placing sound {name}");
    }
    let bs = sfx?;
    let h = bs.get(&name)?;
    let settings = PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol * bs.level(&name)), speed: bs.pitch(&name, 64.0), ..default() };
    Some(commands.spawn((AudioPlayer(h), settings, PlacingSound)).id())
}

impl PlacingChangeEnv for Placing<'_, '_> {
    fn listener(&mut self) -> u32 {
        rider_id(0)
    }
    fn is_queued(&mut self) -> bool {
        self.ring.is_queued(0, &mut self.bank)
    }
    fn queue_sounds(&mut self, played: u64, id: u32) {
        self.ring.queue_sounds(0, played as u32, id, 0, &mut self.bank);
    }
    /// `Speech_OnPlacingChange` 0x2347c8 on the riders (0: none)
    fn speech_placing(&mut self, a: u32, b: u32) {
        let r = |id: u32| if id == 0 { missing_rider() } else { speech_rider(self.race, id as usize - 1) };
        let (a, b) = (r(a), r(b));
        self.announcer.on_placing_change(&mut self.env, &a, &b);
    }
}

/// What `Sfx_OnWipeout` 0x2181e8 reaches: the crowd and the rider's wipe-out line.
struct Wipeout<'a, 'b> {
    announcer: &'a Announcer,
    env: &'a mut Env<'b>,
    rider: &'a SpeechRider,
}

impl sndqueue::WipeoutEnv for Wipeout<'_, '_> {
    fn replay_is_playing(&mut self) -> bool {
        false
    }
    /// `Audio_GetListenerRider` 0x20ed20: the player's rider (the game object's +0xa4 for player 0; for −1 the
    /// first lead or focus rider, the player's too with one player)
    fn listener_rider(&mut self, _: u32) -> i64 {
        rider_id(0) as i64
    }
    // STANDIN: no crowd boxes in the app yet (row E1b2g7d3b7)
    fn crowd_off(&mut self, _: u32) {}
    fn crowd_level(&mut self, _: u32, _: u32) {}
    fn wipeout_voice(&mut self) {
        self.announcer.on_wipeout(self.env, self.rider);
    }
}

/// A rider passed as 0 (none ahead or behind).
// STANDIN: the original reads the null rider's words (the kernel's low memory); taken here as not racing
fn missing_rider() -> SpeechRider {
    SpeechRider { id: 0, controller: 0, ..Default::default() }
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
    /// the riders' positions (the race's order, the game's axes in cm: the rider's +0x140)
    pos: Vec<[f32; 3]>,
    /// the view's position (`Audio_GetRiderPos` 0x21c010: the race's +0xb0, one view in the app)
    // STANDIN: the camera's position, for the race's +0xb0 (what writes it is not traced)
    view: [f32; 3],
}

/// The game's axes in cm of a position in the app's (metres, y up)
fn game_pos(v: Vec3) -> [f32; 3] {
    [v.x * 100.0, -v.z * 100.0, v.y * 100.0]
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
    fn can_play(&mut self, slot: u32) -> bool {
        self.host.can_play(slot)
    }
    /// `Audio_FindRiderNear` 0x20f9b0 (`DAT_00343998` 30 m, the rider's position) over the app's one view
    fn find_near(&mut self, rider: u32) -> i32 {
        let Some(&at) = self.pos.get(rider.wrapping_sub(1) as usize) else { return -1 };
        let view = self.view;
        sndqueue::find_rider_near(sndqueue::SPEECH_NEAR, &mut || 1, &mut |_| view, at)
    }
    /// `RelTable_GetRelation` 0x167e00 on the table `RelTable_Init` 0x167208 fills
    fn relation(&mut self, a: u32, b: u32) -> i32 {
        tricky_game::rivals::relation(a as usize, b as usize) as i32
    }
    fn line_state(&mut self, slot: u32) -> Option<Option<i32>> {
        self.host.line_state(slot)
    }
    fn handle_done(&mut self, handle: u32) -> bool {
        self.host.handle_done(handle)
    }
    fn trigger(&mut self, t: Trigger) -> Vec<u32> {
        self.host.trigger(t)
    }
    fn slot_hold(&mut self, slot: u32, v: u32) {
        self.host.slot_hold(slot, v);
    }
    fn clear_event(&mut self, slot: u32) {
        self.host.clear_event(slot);
    }
    // STANDIN: gApp+0x58, the app's mode word (0), for the game's
    fn app_mode(&mut self) -> u32 {
        0
    }
    // STANDIN: no career records past the new-record list (the pre-race selection object is not kept)
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
    mut audio: ResMut<Assets<AudioSource>>, mut commands: Commands, sfx: Option<Res<crate::boardsound::BoardSounds>>,
    playing: Query<Entity, With<PlacingSound>>, mut rider: ResMut<crate::RiderRes>, mut opponents: ResMut<crate::Opponents>,
    cam: Query<&GlobalTransform, With<Camera3d>>, music: Res<crate::pathmusic::PathMusic>, list: Res<crate::LevelList>,
    level: Res<crate::LevelRes>,
) {
    // what the riders asked of the audio object this frame, taken whatever happens (rider `k` of the race: the player, then the others)
    let mut asks = vec![(0, std::mem::take(&mut rider.0.speech))];
    asks.extend(opponents.0.iter_mut().enumerate().map(|(i, o)| (i + 1, std::mem::take(&mut o.rider.speech))));
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
                    let banks = files.load("data/config/banks.inf").ok().flatten();
                    Some(Loaded { host, files, banks })
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
    // `Audio_LoadInGameConfig` 0x211098's BANKS.INF half for the course (`tricky_data::audiocfg::ingame_banks`): its
    // `SWAP` list and its `BANK`; the course's key is its archive name upper-cased, as the music's
    if rs.course != Some(list.current) {
        rs.course = Some(list.current);
        let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let key = tricky_data::aip::big_name(&track).unwrap_or("").to_ascii_uppercase();
        let banks = sys.banks.as_deref().map(|b| tricky_data::audiocfg::ingame_banks(b, key.as_bytes())).unwrap_or_default();
        rs.ambient = sndqueue::AmbientBanks { names: banks.swaps.into_iter().take(200).collect(), ..Default::default() };
        rs.course_bank = banks.loads.iter().rfind(|l| l.slot == 2).map(|l| l.path.clone()).unwrap_or_default();
        rs.slot6 = Slot6::default();
        let given = std::path::absolute(&level.0.dir).unwrap_or(level.0.dir.clone());
        rs.world = given.parent().and_then(|p| p.parent()).map(|p| p.join("audio").join("world")).unwrap_or_default();
        if std::env::var("TRICKY_SPEECHLOG").is_ok() {
            println!("speech: course {key}: {} ambient banks, bank {:?}, samples in {}", rs.ambient.names.len(), String::from_utf8_lossy(&rs.course_bank), rs.world.display());
        }
    }
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
        for (_, e) in rs.bank.drain() {
            commands.entity(e).try_despawn();
        }
        rs.ring = SoundRing::default();
        rs.event = Default::default();
        rs.slot6 = Slot6::default();
        rs.ambient.clear_loaded();
        if !racing {
            return;
        }
    }
    if game.screen == ui::Screen::Paused {
        return;
    }
    let r = &race.0;
    // +0x6e50, the music zone playing (the music system's, as `Speech_OnRiderReset` 0x233a18 reads it)
    rs.announcer.resets_on = music.zone();
    let mut alive: std::collections::HashSet<Entity> = playing.iter().collect();
    rs.bank.retain(|_, e| alive.contains(e));
    let mut stops = Vec::new();
    let vol = if sounds.effects { 1.0 } else { 0.0 };
    let pos = std::iter::once(rider.0.pos).chain(opponents.0.iter().map(|o| o.rider.pos)).map(game_pos).collect();
    let view = cam.iter().next().map_or([0.0; 3], |t| game_pos(t.translation()));
    let mut env = Env { host: &mut sys.host, mode: r.mode, course: r.course_no, clock: r.line_frame, pos, view };
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
            SpeechCall::TimeUp => rs.announcer.on_time_up(&mut env),
            SpeechCall::PlacingChange { rider, next, prev, old, new } => {
                let id = |k: Option<usize>| k.map_or(0, rider_id);
                let mut p = Placing {
                    bank: Bank { bank: &mut rs.bank, alive: &alive, stops: Vec::new() },
                    ring: &mut rs.ring,
                    handle: &mut rs.handle,
                    played: None,
                    announcer: &mut rs.announcer,
                    env,
                    race: r,
                };
                // STANDIN: the race clock, for the audio system's tick (+0x80)
                sndqueue::on_placing_change(&mut rs.queue, r.line_frame as u32, rider_id(rider), (rider == 0) as u32, id(next), id(prev), old as i64, new as i64, &mut p);
                stops.append(&mut p.bank.stops);
                let played = p.played;
                env = p.env;
                if let Some(h) = played {
                    if let Some(e) = play_top(&mut rs.queue, sfx.as_deref(), vol, &mut commands) {
                        rs.bank.insert(h, e);
                        alive.insert(e);
                    }
                }
            }
            // the crowd's cheer needs the crowd boxes in the app (row E1b2g7d3b7)
            SpeechCall::Cheer(_) => {}
            SpeechCall::EventSound(Some(kind)) => rs.event.queue(kind),
            SpeechCall::EventSound(None) => {
                let mut e = EventSfx { bank: Bank { bank: &mut rs.bank, alive: &alive, stops: Vec::new() }, handle: &mut rs.handle, ambient: &mut rs.ambient, slot6: &mut rs.slot6 };
                rs.event.stop(&mut e);
                stops.append(&mut e.bank.stops);
            }
        }
    }
    rs.sent = calls.len();
    for (k, list) in asks {
        let who = speech_rider(r, k);
        for a in list {
            if std::env::var("TRICKY_SPEECHLOG").is_ok() {
                println!("speech: rider {k} {a:?}");
            }
            match a {
                RiderSpeech::Trick { id, repeat, points } => {
                    if tricky_game::score::announce_speaks_trick(repeat, r.mode) {
                        rs.announcer.announce_trick(&mut env, &who, id, points);
                    } else {
                        rs.announcer.on_trick_landed(&mut env, &who);
                    }
                }
                // `Sfx_OnBoost` 0x2197d8: for the player's rider (+0x41c 1) its events cancelled, sound 0x6c queued
                // and played, then `Speech_OnBoost` 0x233e78 (STANDIN: the sound is `boardsound`'s sfx_108)
                RiderSpeech::BoostEmpty => {
                    if k == 0 {
                        rs.announcer.on_boost(&mut env, &who);
                    }
                }
                // `Audio_TrickyTrigger` 0x21c610: the audio system's +0x3844, +0x36f0 and +0x3700 set (STANDIN: taken as
                // set), no replay, the rider racing (+0x418 < 0) and the listener's; then +0x6e7c the rider, +0x6e84
                // 0x7f (0x5a while +0x6e50), `Music_StartTricky` (the music's: `pathmusic`) and `Speech_OnTricky` 0x2337c8
                RiderSpeech::Tricky { finished, ubers } => {
                    if !finished && k == 0 {
                        rs.announcer.on_tricky(&mut env, &SpeechRider { tricky: ubers, ..who });
                    }
                }
                RiderSpeech::Hit { by, which, crash } => {
                    let other = speech_rider(r, by as usize);
                    tricky_game::speech::voices::rider_hit_line(&mut env, &other, &who, which as i64, crash as i64);
                    if crash {
                        rs.announcer.on_rider_hit(&mut env, &who, &other);
                    }
                }
                RiderSpeech::Grudge { at } => rs.announcer.on_grudge(&mut env, &who, &speech_rider(r, at as usize)),
                RiderSpeech::Shoved => rs.announcer.on_rider_collision(&mut env, &who),
                RiderSpeech::Reset => rs.announcer.on_rider_reset(&mut env, &who),
                RiderSpeech::AirTime => rs.announcer.on_air_time(&mut env, &who),
                RiderSpeech::AirEnter { kind, air, up } => {
                    let r = SpeechRider { air, vel: [0.0, 0.0, up], ..who };
                    if 5.0 < air {
                        rs.announcer.on_air_enter(&mut env, &r, kind as i64);
                    }
                    tricky_game::speech::voices::on_air_nearby(&mut env, &r, kind as i64);
                }
                RiderSpeech::TenSecondsLeft => rs.announcer.on_ten_seconds_left(&mut env, &who),
                RiderSpeech::Wipeout(level) => {
                    let air = sndqueue::AirRider { flag_41c: (k == 0) as u32, player: 0, air: 0.0, emitter: 0 };
                    let mut w = Wipeout { announcer: &rs.announcer, env: &mut env, rider: &who };
                    sndqueue::on_wipeout(level, rider_id(k) as i64, &air, &mut w);
                }
            }
        }
    }
    // `SndSys_UpdateQueuedSounds` 0x215658, from `Audio_InGameUpdate`
    let mut bank = Bank { bank: &mut rs.bank, alive: &alive, stops };
    rs.ring.update(r.line_frame, &mut bank);
    // `Audio_PlayEventSound` 0x21d0c0, from `Audio_InGameUpdate` at 0x20ef24
    let given = rs.handle;
    let mut ev = EventSfx { bank, handle: &mut rs.handle, ambient: &mut rs.ambient, slot6: &mut rs.slot6 };
    rs.event.update(&mut rs.queue, &mut ev);
    let played = *ev.handle != given;
    let bank = ev.bank;
    if played {
        let i = rs.queue.count.max(0) as usize;
        let file = kind_bank(rs.queue.kind[i], &rs.course_bank, &rs.slot6);
        if let Some(e) = play_bank_sound(&mut rs.queue, file, &rs.world, vol, &mut audio, &mut commands) {
            bank.bank.insert(rs.handle, e);
        }
    }
    for e in bank.stops {
        commands.entity(e).try_despawn();
    }
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

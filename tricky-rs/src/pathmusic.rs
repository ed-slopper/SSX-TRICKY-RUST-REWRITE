//! The race songs as the game plays them: EA's "Pathfinder" interactive music (`PF_*` lib
//! 0x2bec00-0x2c2040, `Audio_StartRaceMusic` 0x215220, `Music_SetPathVar` 0x2253e8,
//! `Audio_MusicEvent` 0x225318), and the in-air loops over them (`Music_PickAirLoopPhrase`
//! 0x2246a8, `Music_ScheduleAirLoop` 0x225878, `Audio_TrickyTrigger` 0x21c610).
//!
//! Each song is a graph of nodes, one segment (a few beats) of audio each. About half a second
//! before a node ends the next is chosen: a pending event's target, else the first branch whose
//! range holds the path variable (0-99: the rider's place with rivals, how full the boost meter
//! is alone), through the node's router; markers with no audio are walked through, and looping
//! sections count down. Events: 0 start, 1-6 the shortcut zones, 10 the finish (the outro, then
//! silence). Joins are sample-exact.
//!
//! Air loops: from the song's LOOPDATA bank, one sample a beat in step with the song, phrase
//! `rand % 3` (Tricky: phrase 3), program `phrase * 16 + beat`; they come up as the big-air duck
//! takes the song down.
//!
//! Data: `audio/pf/<song>/{song.ogg, graph.json, loops/}` and `audio/pf/songs.json`, made by
//! tools/music/musicbig.py + pack.py. Without them the plain songs in `sound.rs` play.

use crate::rider::RaceState;
use crate::{ui, LevelList, Mode, Opponents, RaceRes, RiderRes};

use bevy::audio::{AddAudioSource, Decodable, PlaybackMode, Source, Volume};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct PNode { seg: i32, col: usize, lp: u8, router: usize, br: Vec<(i32, i32, i32)> }

pub struct Song {
    rate: u32,
    ch: u16,
    pcm: Vec<i16>,
    segs: HashMap<i32, (usize, usize)>,
    nodes: HashMap<i32, PNode>,
    events: Vec<Vec<usize>>,
    actions: Vec<(u8, i32)>,
    routers: Vec<Vec<(i32, i32)>>,
}

/// What the game tells the playing song, and what it reports back.
#[derive(Default)]
pub struct Shared {
    var: AtomicU8,
    events: Mutex<VecDeque<u8>>,
    /// frames into the node now playing
    node_pos: AtomicU32,
    ended: AtomicBool,
}

/// The walker's state (PF_AdvanceNode / PF_ResolveNode).
struct Walk { loop_node: i32, loop_cnt: u8 }

impl Song {
    fn node(&self, n: i32) -> Option<&PNode> { self.nodes.get(&n) }
    fn branch(&self, w: &mut Walk, n: i32, v: u8) -> i32 {
        let Some(node) = self.node(n) else { return -1 };
        let mut v = v as i32;
        if n == w.loop_node {
            v = (w.loop_cnt & 0x7f) as i32;
            if node.lp != 0 { w.loop_cnt = w.loop_cnt.wrapping_sub(1); if w.loop_cnt == 0xff { w.loop_node = -1; } }
        }
        node.br.iter().find(|b| b.0 <= v && v <= b.1).map_or(-1, |b| b.2)
    }
    fn route(&self, cur: i32, mut x: i32) -> i32 {
        if let Some(r) = self.node(cur).map(|n| n.router).filter(|r| *r > 0) {
            for &(s, d) in self.routers.get(r - 1).into_iter().flatten() { if x == s { x = d; } }
        }
        x
    }
    fn resolve(&self, w: &mut Walk, cur: i32, x: i32, v: u8) -> i32 {
        let mut x = self.route(cur, x);
        for _ in 0..4096 {
            let Some(n) = self.node(x) else { return -1 };
            if n.seg >= 1 { return x; }
            if n.seg != 0 && n.lp != 0 && x != w.loop_node { w.loop_cnt = n.lp; w.loop_node = x; }
            x = self.route(cur, self.branch(w, x, v));
            if x < 0 { return -1; }
        }
        -1
    }
    /// An event's action in the column of the node playing: (immediate, target) or None for a no-op.
    fn action(&self, cur: i32, e: u8) -> Option<(bool, i32)> {
        let col = self.node(cur).map_or(0, |n| n.col);
        let a = *self.events.get(e as usize)?.get(col).or(self.events.get(e as usize)?.first())?;
        let (flags, target) = *self.actions.get(a)?;
        if flags & 3 == 0 && flags & 0x80 == 0 { return None; }
        if target < 0 { return if flags & 2 != 0 { Some((flags & 0x80 != 0, -1)) } else { None }; }
        Some((flags & 0x80 != 0, target))
    }
    pub fn load(dir: &std::path::Path) -> Option<Song> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("graph.json")).ok()?).ok()?;
        let bytes = std::fs::read(dir.join("song.ogg")).ok()?;
        let dec = rodio::Decoder::new(std::io::Cursor::new(bytes)).ok()?;
        let ch = rodio::Source::channels(&dec);
        let pcm: Vec<i16> = dec.collect();
        let int = |x: &serde_json::Value| x.as_i64().unwrap_or(0);
        let mut segs = HashMap::new();
        for (k, s) in v.get("segments")?.as_object()? {
            let a = s.as_array()?;
            segs.insert(k.parse().ok()?, (int(&a[0]) as usize, int(&a[1]) as usize));
        }
        let mut nodes = HashMap::new();
        for (k, n) in v.get("nodes")?.as_object()? {
            let br = n.get("br")?.as_array()?.iter().filter_map(|b| { let b = b.as_array()?; Some((int(&b[0]) as i32, int(&b[1]) as i32, int(&b[2]) as i32)) }).collect();
            nodes.insert(k.parse().ok()?, PNode { seg: int(&n["seg"]) as i32, col: int(&n["col"]) as usize, lp: int(&n["loop"]) as u8, router: int(&n["router"]) as usize, br });
        }
        let events = v.get("events")?.as_array()?.iter().map(|e| e.as_array().map(|c| c.iter().map(|x| int(x) as usize).collect()).unwrap_or_default()).collect();
        let actions = v.get("actions")?.as_array()?.iter().map(|a| (int(&a[0]) as u8, int(&a[1]) as i32)).collect();
        let routers = v.get("routers")?.as_array()?.iter().map(|r| r.as_array().map(|l| l.iter().map(|p| (int(&p[0]) as i32, int(&p[1]) as i32)).collect()).unwrap_or_default()).collect();
        let rate = int(&v["rate"]) as u32;
        let total = int(&v["total"]) as usize;
        if pcm.len() / (ch as usize) < total { println!("music: {} decoded short ({} of {total} frames)", dir.display(), pcm.len() / ch as usize); }
        Some(Song { rate, ch, pcm, segs, nodes, events, actions, routers })
    }
}

/// The playing song, as an audio source.
#[derive(Asset, TypePath, Clone)]
pub struct PfStream { song: Arc<Song>, shared: Arc<Shared> }

pub struct PfDecoder { song: Arc<Song>, shared: Arc<Shared>, walk: Walk, cur: i32, next: Option<i32>, pos: usize, len: usize, start: usize, chan: u16, frame: usize, stopped: bool }

impl PfDecoder {
    fn enter(&mut self, n: i32) {
        self.cur = n;
        self.next = None;
        self.pos = 0;
        match self.song.node(n).and_then(|nd| self.song.segs.get(&nd.seg)) {
            Some(&(s, l)) => { self.start = s; self.len = l; }
            None => { self.stopped = true; self.shared.ended.store(true, Ordering::Relaxed); }
        }
    }
    /// Every so often: an immediate event cuts now; near the end the next node is picked.
    fn check(&mut self) {
        let var = self.shared.var.load(Ordering::Relaxed);
        let mut q = self.shared.events.lock().unwrap();
        if let Some(&e) = q.front() {
            match self.song.action(self.cur, e) {
                None => { q.pop_front(); }
                Some((true, t)) => {
                    q.pop_front();
                    drop(q);
                    let n = if t < 0 { -1 } else { self.song.resolve(&mut self.walk, self.cur, t, var) };
                    self.enter(n);
                    return;
                }
                Some((false, t)) if self.next.is_none() && self.len.saturating_sub(self.pos) < self.song.rate as usize / 2 => {
                    q.pop_front();
                    self.next = Some(if t < 0 { -1 } else { self.song.resolve(&mut self.walk, self.cur, t, var) });
                }
                _ => {}
            }
        }
        drop(q);
        if self.next.is_none() && self.len.saturating_sub(self.pos) < self.song.rate as usize / 2 {
            let b = self.song.branch(&mut self.walk, self.cur, var);
            self.next = Some(self.song.resolve(&mut self.walk, self.cur, b, var));
        }
    }
}

impl Iterator for PfDecoder {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        if self.chan == 0 {
            if self.frame.is_multiple_of(256) && !self.stopped { self.check(); self.shared.node_pos.store(self.pos as u32, Ordering::Relaxed); }
            self.frame += 1;
            if !self.stopped && self.pos >= self.len {
                let n = match self.next { Some(n) => n, None => { let v = self.shared.var.load(Ordering::Relaxed); let b = self.song.branch(&mut self.walk, self.cur, v); self.song.resolve(&mut self.walk, self.cur, b, v) } };
                self.enter(n);
            }
        }
        let ch = self.song.ch as usize;
        let s = if self.stopped { 0 } else { self.song.pcm.get((self.start + self.pos) * ch + self.chan as usize).copied().unwrap_or(0) };
        self.chan += 1;
        if self.chan as usize >= ch { self.chan = 0; self.pos += 1; }
        Some(s)
    }
}

impl Source for PfDecoder {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.song.ch }
    fn sample_rate(&self) -> u32 { self.song.rate }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

impl Decodable for PfStream {
    type DecoderItem = i16;
    type Decoder = PfDecoder;
    fn decoder(&self) -> PfDecoder {
        // event 0 starts the song (PF_ApplyEvent: an immediate go-to the first node)
        let var = self.shared.var.load(Ordering::Relaxed);
        let mut d = PfDecoder { song: self.song.clone(), shared: self.shared.clone(), walk: Walk { loop_node: -1, loop_cnt: 0 }, cur: -1, next: None, pos: 0, len: 0, start: 0, chan: 0, frame: 1, stopped: false };
        let t = self.song.action(-1, 0).map_or(0, |a| a.1.max(0));
        let n = d.song.resolve(&mut d.walk, -1, t, var);
        d.enter(n);
        d
    }
}

pub fn plugin(app: &mut App) { app.add_audio_source::<PfStream>().init_resource::<PathMusic>(); }

/// MUSIC.INF settings of a song.
#[derive(Clone, Default)]
struct Params { bpm: f32, path_level: f32, async_level: f32, phrases: HashMap<u32, Vec<u32>> }

#[derive(Resource, Default)]
pub struct PathMusic {
    /// the original songs are there: the plain ones stay quiet
    pub active: bool,
    tried: bool,
    dir: std::path::PathBuf,
    params: HashMap<String, Params>,
    want: String,
    loading: Option<(String, Arc<Mutex<Option<Option<Song>>>>)>,
    song: Option<(String, Arc<Song>)>,
    shared: Arc<Shared>,
    playing: Option<Entity>,
    run: u32,
    finished_sent: bool,
    seed: u32,
    loops: HashMap<String, Handle<AudioSource>>,
    /// the air-loop layer: (phrase, next beat index, seconds to the next beat, beats left for Tricky)
    air: Option<(u32, u32, f32, i32)>,
    tricky_was: bool,
}

#[derive(Component)]
pub struct PfPlayer;

/// The race song and its air loops (`Audio_StartRaceMusic` 0x215220, `Music_SetPathVar` 0x2253e8, `Audio_MusicEvent`
/// 0x225318, `Music_ScheduleAirLoop` 0x225878, `Audio_TrickyTrigger` 0x21c610).
#[allow(clippy::too_many_arguments)]
pub fn path_music(
    mut pm: ResMut<PathMusic>, sounds: Res<crate::sound::Sounds>, list: Res<LevelList>, game: Res<ui::Game>, mode: Res<Mode>, race: Res<RaceRes>,
    rider: Res<RiderRes>, opponents: Res<Opponents>, lib: Res<crate::CharLib>, course: Res<crate::coursemusic::CourseMusic>, time: Res<Time>,
    mut streams: ResMut<Assets<PfStream>>, mut audio: ResMut<Assets<AudioSource>>, mut commands: Commands, mut sinks: Query<&mut AudioSink, With<PfPlayer>>,
    mut last: Local<Option<(ui::Screen, RaceState, usize)>>,
) {
    if !pm.tried {
        pm.tried = true;
        pm.dir = sounds.dir.join("pf");
        pm.seed = 0x1234567;
        if let Ok(v) = std::fs::read_to_string(pm.dir.join("songs.json")).map_err(|_| ()).and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(&t).map_err(|_| ())) {
            for s in v {
                let Some(key) = s.get("key").and_then(|k| k.as_str()) else { continue };
                let f = |k: &str, d: f64| s.get(k).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
                let mut phrases = HashMap::new();
                if let Some(ph) = s.pointer("/loops/0/phrases").and_then(|p| p.as_object()) {
                    for (k, l) in ph { if let (Ok(k), Some(l)) = (k.parse::<u32>(), l.as_array()) { phrases.insert(k, l.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()); } }
                }
                pm.params.insert(key.to_string(), Params { bpm: f("BPM", 120.0), path_level: f("PathLevel", 100.0) / 100.0, async_level: f("AsyncLevel", 100.0) / 100.0, phrases });
            }
        }
        println!("music: {} interactive songs{}", pm.params.len(), if pm.params.is_empty() { " (audio/pf not found)" } else { "" });
    }
    if pm.params.is_empty() { return; }
    let r = &rider.0;
    let st = race.0.state;
    let now = (game.screen, st, list.current);
    let was = last.replace(now);
    let menu = game.screen == ui::Screen::Menu || *mode != Mode::Ride;
    // a new run (from the menu, a restart, another course): a new song from the course's list
    let new_run = was.is_none_or(|w| w.2 != now.2 || (w.1 != RaceState::Countdown && st == RaceState::Countdown) || (w.0 == ui::Screen::Menu && !menu));
    if new_run && !menu { pm.run = pm.run.wrapping_add(1); }
    let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let want = if menu { "ssxmenu".to_string() } else {
        let songs = crate::sound::course_songs(&track);
        let have: Vec<&&str> = songs.iter().filter(|s| pm.params.contains_key(**s)).collect();
        if have.is_empty() { String::new() } else if new_run || !have.iter().any(|s| **s == pm.want) {
            pm.seed ^= pm.seed << 13; pm.seed ^= pm.seed >> 17; pm.seed ^= pm.seed << 5;
            have[pm.seed as usize % have.len()].to_string()
        } else { pm.want.clone() }
    };
    pm.active = !want.is_empty() && pm.params.contains_key(&want);
    // stop the song on a change of song or a new run
    if want != pm.want || new_run {
        if let Some(e) = pm.playing.take() { commands.entity(e).try_despawn(); }
        pm.finished_sent = false;
        pm.air = None;
    }
    pm.want = want.clone();
    if !pm.active { return; }
    // load it (decoding takes a moment, so off the main thread)
    if pm.song.as_ref().is_none_or(|s| s.0 != want) {
        if pm.loading.as_ref().is_none_or(|l| l.0 != want) {
            let slot = Arc::new(Mutex::new(None));
            let (dir, s2) = (pm.dir.join(&want), slot.clone());
            std::thread::spawn(move || { let s = Song::load(&dir); *s2.lock().unwrap() = Some(s); });
            pm.loading = Some((want.clone(), slot));
        }
        let done = pm.loading.as_ref().and_then(|l| l.1.lock().unwrap().take());
        match done {
            Some(Some(s)) => { pm.song = Some((want.clone(), Arc::new(s))); pm.loading = None; }
            Some(None) => { println!("music: {want} could not be loaded"); pm.params.remove(&want); pm.loading = None; return; }
            None => return,
        }
    }
    let params = pm.params.get(&want).cloned().unwrap_or_default();
    // the path variable: by place with rivals, by the boost meter alone (start: 80)
    // (the original's place is 0-based in a formula written for 1-based: first and second both get 99)
    let var = if menu { 80 } else if !opponents.0.is_empty() {
        let n = opponents.0.len() as i32 + 1;
        let place = crate::standings(&race.0, &opponents.0, &lib, "").iter().position(|t| t.2).unwrap_or(0) as i32;
        (99 - ((place - 1) * 99) / (n - 1)).clamp(0, 99) as u8
    } else { ((r.boost * 1.5).clamp(0.0, 1.0) * 99.0) as u8 };
    pm.shared.var.store(var, Ordering::Relaxed);
    // the song starts at GO (held back during the intro and countdown), in the menus at once
    let start = menu || (st != RaceState::Countdown && !course.holding);
    if pm.playing.is_none() && start && matches!(game.screen, ui::Screen::Menu | ui::Screen::Playing | ui::Screen::Results) {
        let shared = Arc::new(Shared::default());
        shared.var.store(var, Ordering::Relaxed);
        pm.shared = shared.clone();
        let song = pm.song.as_ref().unwrap().1.clone();
        let h = streams.add(PfStream { song, shared });
        let e = commands.spawn((AudioPlayer::<PfStream>(h), PlaybackSettings { mode: PlaybackMode::Once, volume: Volume::Linear(0.0), ..default() }, PfPlayer)).id();
        pm.playing = Some(e);
    }
    // the finish: the outro
    if st == RaceState::Finished && !pm.finished_sent && !menu {
        pm.finished_sent = true;
        pm.shared.events.lock().unwrap().push_back(10);
    }
    // volume: the music level, PathLevel, and the big-air duck (Audio_BigAirMusicDuck)
    let air = if !r.grounded && r.rail.is_none() && r.crashed <= 0.0 { r.air_time } else { 0.0 };
    let floor = if track == "untracked" { 50.0 / 127.0 } else { 16.0 / 127.0 };
    let duck = if air > 1.5 && !menu { (1.0 - (1.0 - floor) * air * 0.5).clamp(floor, 1.0) } else { 1.0 };
    let base = if !sounds.music { 0.0 } else if menu { 0.5 } else if game.screen == ui::Screen::Paused { 0.15 } else { 0.4 };
    let vol = base * params.path_level * duck;
    for mut s in &mut sinks {
        let now = s.volume().to_linear();
        s.set_volume(Volume::Linear(now + (vol - now) * (1.0 - (-10.0 * time.delta_secs()).exp())));
    }

    // the air loops, one sample a beat in time with the song
    if menu || pm.playing.is_none() || game.screen != ui::Screen::Playing { pm.air = None; return; }
    let beat = 60.0 / params.bpm.max(30.0);
    // Tricky: its phrase twice through when the meter fills (Audio_TrickyTrigger), longer in big air
    let tricky_now = r.tricky() && !pm.tricky_was;
    pm.tricky_was = r.tricky();
    let big = air > 1.5;
    let pos = pm.shared.node_pos.load(Ordering::Relaxed) as f32 / pm.song.as_ref().map_or(36000.0, |s| s.1.rate as f32);
    match pm.air {
        None if big || tricky_now => {
            pm.seed ^= pm.seed << 13; pm.seed ^= pm.seed >> 17; pm.seed ^= pm.seed << 5;
            let phrase = if tricky_now { 3 } else { pm.seed % 3 };
            let k = ((pos / beat).floor() as u32 + 1) % 8;
            pm.air = Some((phrase, k, beat - pos % beat, if tricky_now { 16 } else { -1 }));
        }
        Some((_, _, _, left)) if left < 0 && !big => pm.air = None,
        Some((_, _, _, 0)) if !big => pm.air = None,
        _ => {}
    }
    if let Some((phrase, k, wait, left)) = pm.air {
        let wait = wait - time.delta_secs();
        if wait <= 0.0 {
            let prog = phrase * 16 + k;
            let has = params.phrases.get(&phrase).is_some_and(|l| l.contains(&k));
            if has {
                let name = format!("{want}/loops/loops_{prog}.wav");
                let h = match pm.loops.get(&name) { Some(h) => Some(h.clone()), None => std::fs::read(pm.dir.join(&name)).ok().map(|b| { let h = audio.add(AudioSource { bytes: b.into() }); pm.loops.insert(name.clone(), h.clone()); h }) };
                let lvl = if left >= 0 { 0.85 } else { 1.0 - duck };
                if let Some(h) = h { commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(base * params.async_level * lvl), ..default() })); }
            }
            pm.air = Some((phrase, (k + 1) % 8, wait + beat, if left > 0 { left - 1 } else { left }));
        } else {
            pm.air = Some((phrase, k, wait, left));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// TRICKY_PFTEST=audio/pf/<song>: walk the song at path value 80 and print the nodes it plays.
    #[test]
    fn walk() {
        let Ok(dir) = std::env::var("TRICKY_PFTEST") else { return };
        let song = Arc::new(Song::load(std::path::Path::new(&dir)).expect("song"));
        let shared = Arc::new(Shared::default());
        shared.var.store(80, Ordering::Relaxed);
        let s = PfStream { song: song.clone(), shared: shared.clone() };
        let mut d = s.decoder();
        let mut path = vec![d.cur];
        let secs = 400;
        for i in 0..(song.rate as usize * secs * song.ch as usize) {
            d.next();
            if i == song.rate as usize * 300 * song.ch as usize { shared.events.lock().unwrap().push_back(10); }
            if *path.last().unwrap() != d.cur { path.push(d.cur); }
            if d.stopped { break; }
        }
        println!("path ({} nodes, stopped {}): {:?}", path.len(), d.stopped, path);
    }
}

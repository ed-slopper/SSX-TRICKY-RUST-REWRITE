//! The rider's sounds as the game makes them (`BoardIn_*`, `Voice_*`, `Audio_SurfaceGroup`,
//! DATA/CONFIG/SNOW.INF): the original board samples (zboard.bnk) and effects (zbxsfx.bnk),
//! exported as `audio/bnk/board_<program>.wav` / `sfx_<program>.wav`, and the snow sound programs
//! from `audio/snow.inf` that turn the board's load, slip, dig and lean into volume and bend.
//!
//! Board program = surface group * 8 + slot: 1 landing, 2 takeoff, 3 carve loop, 4 glide loop.
//! Program 0 is the slow scrape. The three loops are the game's (row E1b2g7d5i): started by
//! `sndqueue::start_rider_loops` and run every frame by `sndqueue::update_loop`, which restart
//! them with the new group's samples when the surface changes. When these files are there, the
//! stand-in loops and effects in `sound.rs` stand aside.

use crate::rider::Rider;
use crate::{ui, LevelRes, Mode, RiderRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::HashMap;
use tricky_game::sfxcalc::{board_input, run_snow_program, BoardIn};
use tricky_game::bankmon;
use tricky_game::ps2f::F;
use tricky_game::sndqueue::{self, LoopInstance, LoopRider, Queue, QueueEnv};

/// The snow sound programs as `Sfx_LoadSurfaceConfig` compiles them (`tricky_data::snowsfx`): per mode (GLIDE,
/// AIGLIDE, CARVE), per surface sound type (PACK .. CHUTE), the game's own bytes, run by `sfxcalc::run_snow_program`.
#[derive(Default)]
struct Programs(Vec<Vec<Vec<u8>>>);

impl Programs {
    fn parse(text: &str) -> Self {
        Self(tricky_data::snowsfx::compile_all(text.as_bytes()))
    }
    /// `Sfx_RunSnowProgram` 0x21ae90 on the record at cBXAudio + `table` + `surface` · 0x50 (`sndqueue::GLIDE_PROGRAMS`
    /// and the others), its words `w` kept between runs as the record keeps them. A program that divides by zero
    /// stops there, `vol` and `bend` as far as it got.
    // STANDIN: the stop, for the original's `break 7` (a crash on the console)
    fn run(&self, table: u32, surface: u32, w: &mut [i32; 0x14], b: &BoardIn, vol: &mut i32, bend: &mut i32) {
        let mode = match table {
            sndqueue::GLIDE_PROGRAMS => 0,
            sndqueue::AI_GLIDE_PROGRAMS => 1,
            _ => 2,
        };
        if let Some(prog) = self.0.get(mode).and_then(|m| m.get(surface as usize)) {
            let _ = run_snow_program(prog, w, &mut |n| board_input(n, b), vol, bend);
        }
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

/// The rider's motion as the audio system reads it (+0x424: 1 air, 2 ground, 3 a rail, 5 wiped out, 6 recovering).
// STANDIN: the app's rider has no motion word (`Boarder_EnterMotionState` 0x11c710); read from its own state
fn motion(r: &Rider) -> u32 {
    if r.crashed > 0.0 {
        5
    } else if r.recover.is_some() {
        6
    } else if r.rail.is_some() {
        3
    } else if r.grounded {
        2
    } else {
        1
    }
}

/// The rider's sound loops as the audio system runs them (row E1b2g7d5i): `Audio_InitRiderSounds` 0x210a38 starts
/// them when a ride starts (`sndqueue::start_rider_loops`: the scrape, glide and carve loops), and every frame each
/// loop's bank instance is updated by the callback its queue entry carries (`sndqueue::update_loop`), its volume
/// (+0x64) and bend heard on the loop's sample, through the bank monitor's fall-off from the listener
/// (`BankInstance_UpdatePositional` 0x20b700). A surface change restarts a loop with the new surface's sample.
// STANDIN: the player's rider only; the opponents' AI glide loops are row E1b2g7d5j
#[derive(Resource, Default)]
pub struct RiderLoops {
    /// the audio system's queue the starts push on (`cSndPlayOpts::cSndPlayOpts` 0x226848's header)
    q: Option<Queue>,
    rider: LoopRider,
    /// the playing loops
    voices: Vec<Voice>,
    /// the snow program records' words (cBXAudio + table + surface · 0x50), kept between runs
    words: HashMap<(u32, u32), [i32; 0x14]>,
    handle: u32,
    /// the level the loops were started for; none while not riding
    ride: Option<usize>,
}

/// A playing loop: its bank instance (the address the handle) and what the monitor keeps of it.
struct Voice {
    inst: LoopInstance,
    /// the update its entry carried (+0x44)
    update: u32,
    name: String,
    e: Entity,
    /// the bend last set (`BankInstance_SetPitchBend` 0x21e7e0)
    bend: i64,
    /// the instance's fall-off (+0x88) and cull (+0x8c) distances, metres
    far: F,
    cull: F,
    /// the distance last seen (+0x5c, −1 unknown) and whether the voice plays (+8)
    last: F,
    playing: bool,
}

/// The player's rider's address as the loops carry it (the instance's owner, +0x80).
// STANDIN: a token, the app's rider has no address
const RIDER: u32 = 0x0100_0000;

/// What the loops' starts and updates ask of the audio system and the rider.
struct LoopEnv<'a> {
    bs: &'a BoardSounds,
    words: &'a mut HashMap<(u32, u32), [i32; 0x14]>,
    inp: BoardIn,
    /// the rider's motion, its surface (+0x290) and the audio system's rail word (+0x6e40, the rail's word
    /// `Rail_TryCapture` 0x125cb8 stores)
    motion: u32,
    surface: u32,
    rail: u32,
    handle: &'a mut u32,
    /// the handles `play_pending` gave, in order, with the pending volume's two distances set before each
    played: Vec<(u32, (f32, f32))>,
    pending: (f32, f32),
    reset: Vec<u32>,
    bends: Vec<(u32, i64)>,
    cancel_break: bool,
}

impl QueueEnv for LoopEnv<'_> {
    // STANDIN: the sound manager's slots (`SndMgr_GetSlot` 0x211f38), for the volume `SndSys_PlayPending` scales by
    fn get_slot(&mut self, _: u32) -> u32 {
        0
    }
    // STANDIN: the listener's rider (`Audio_GetRiderListener` 0x20fc60); the app has one listener, the player
    fn rider_listener(&mut self) -> u32 {
        1
    }
    /// `SndSys_PlayPending` 0x20c948: a handle; the queue's top entry is played once the call is back
    fn play_pending(&mut self) -> u64 {
        *self.handle += 1;
        self.played.push((*self.handle, self.pending));
        *self.handle as u64
    }
}

impl sndqueue::RiderLoopEnv for LoopEnv<'_> {
    /// `Sfx_SurfaceSoundType` 0x20f908
    fn surface_sound_type(&mut self, _: u32) -> u32 {
        tricky_game::sfxcalc::surface_sound_type(self.motion, self.rail, self.surface)
    }
    /// `SndSys_SetPendingVolume` 0x226dd8: kept for the entry `play_pending` plays next
    fn set_pending_volume(&mut self, cull: f32, far: f32) {
        self.pending = (cull, far);
    }
}

impl sndqueue::LoopUpdateEnv for LoopEnv<'_> {
    /// `Sfx_ImpactSpeed` 0x21a6a8
    fn impact_speed(&mut self, _: u32) -> i32 {
        board_input(0, &self.inp)
    }
    /// `Sfx_EvalCurve` 0x20d928 on the executable's curves
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64 {
        tricky_game::sfxcalc::eval_slot(slot, x as i64)
    }
    /// `Music_CancelBreak` 0x2257d8, sent to the race's music once the update is back
    fn cancel_break(&mut self) {
        self.cancel_break = true;
    }
    /// `BankInstance_Reset` 0x20a9d8: stopped and freed once the update is back
    fn reset_instance(&mut self, inst: u32) {
        self.reset.push(inst);
    }
    fn run_snow_program(&mut self, table: u32, surface: u32, _: u32, vol: &mut i32, bend: &mut i32) {
        let w = self.words.entry((table, surface)).or_insert([0; 0x14]);
        self.bs.programs.run(table, surface, w, &self.inp, vol, bend);
    }
    /// `BankInstance_SetPitchBend` 0x21e7e0
    fn set_pitch_bend(&mut self, inst: u32, bend: i64) {
        self.bends.push((inst, bend));
    }
}

impl RiderLoops {
    /// The queue's entries `played` started, top first, made into looping voices (`SND_PlayBankSound` 0x2d1718,
    /// `BankMonitor_AddInstance` 0x20bd68: the fall-off and cull distances from the entry, the distance known at
    /// once since the entry has a position, `d`).
    // STANDIN: the sound driver, for the bank's exported loop sample (tools/bnk/bnk2wav.py), silent until updated
    fn take_played(&mut self, played: Vec<(u32, (f32, f32))>, d: F, bs: &BoardSounds, commands: &mut Commands) {
        let q = self.q.as_mut().expect("the queue");
        for (h, (cull, far)) in played.into_iter().rev() {
            let i = q.count.max(0) as usize;
            // what `SndSys_SetPendingVolume` did to the entry before it was played
            sndqueue::set_pending_volume(q, cull, far);
            let (far, cull) = (F(f32::from_bits(q.a6c[i])), F(f32::from_bits(q.a70[i])));
            q.count -= 1;
            let (id, update) = (q.id[i], q.a44[i][1]);
            let name = format!("board_{id}");
            if let Some(src) = bs.get_loop(&name) {
                let e = commands.spawn((AudioPlayer::new(src), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, RiderLoop)).id();
                if std::env::var("TRICKY_BOARDLOG").is_ok() {
                    println!("board loop {name}: handle {h}, update {update:#x}");
                }
                self.voices.push(Voice { inst: LoopInstance { addr: h, volume: 0 }, update, name, e, bend: 0x40, far, cull, last: d, playing: true });
            }
        }
    }
    fn stop(&mut self, commands: &mut Commands) {
        for v in self.voices.drain(..) {
            commands.entity(v.e).try_despawn();
        }
        self.q = None;
    }

    /// One frame of the loops: started for a new ride, else each updated, and their voices set. `ear` is the
    /// listener (the camera), `dt` the monitor's step.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        &mut self, ride: Option<usize>, r: &Rider, ear: Option<&GlobalTransform>, dt: f32, bs: &BoardSounds, fx: f32, pm: &mut crate::pathmusic::PathMusic,
        commands: &mut Commands, sinks: &mut Query<&mut AudioSink, With<RiderLoop>>,
    ) {
        if ride != self.ride {
            self.stop(commands);
            self.ride = ride;
        }
        if self.ride.is_none() {
            return;
        }
        // the rider's motion; a change mutes or unmutes the loops as the motions' enter and exit routines do
        let (old, new) = (self.rider.motion, motion(r));
        self.rider.motion = new;
        for muted in tricky_game::states::motion_change_sfx_mutes(old, new) {
            tricky_game::control::set_sfx_muted(&mut self.rider.w5ae4, &mut self.rider.w5af0, muted);
        }
        // the rider's emitter (+0x140) where the listener hears it (`cBXAudio::ListenerLocate` 0x20e730)
        // STANDIN: the app's camera for the race's listener matrix (race +0xc0 + 0x80 per view), in the game's
        // view axes as we read them (x right, y down, z ahead, cm: inferred), and the pan off (career +0x1668, which
        // the app's host could not pan by anyway)
        let d = match ear {
            Some(cam) => {
                let v = cam.affine().inverse().transform_point3(r.pos) * 100.0;
                let id = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
                let l = tricky_game::sfxcalc::listener_locate(Some(id), [v.x, -v.y, -v.z], false);
                F(l.map_or(1.0, |l| l.d2)).sqrt()
            }
            None => F(1.0),
        };
        let start = self.q.is_none();
        if start {
            self.q = Some(Queue { header: sndqueue::play_opts_header(), ..Queue::with_capacity(8) });
            self.rider = LoopRider { addr: RIDER, flag_41c: 1, motion: self.rider.motion, w5ae4: self.rider.w5ae4, w5af0: self.rider.w5af0, ..Default::default() };
        }
        let q = self.q.as_mut().expect("the queue");
        let mut env = LoopEnv {
            bs, words: &mut self.words, inp: board_in(r), motion: self.rider.motion, surface: r.surface as u32, rail: r.rail_word,
            handle: &mut self.handle, played: Vec::new(), pending: (0.0, 0.0), reset: Vec::new(), bends: Vec::new(), cancel_break: false,
        };
        if start {
            sndqueue::start_rider_loops(q, &mut self.rider, &mut env);
        } else {
            for v in self.voices.iter_mut() {
                sndqueue::update_loop(v.update, q, &mut v.inst, &mut self.rider, &mut env);
            }
        }
        let LoopEnv { played, reset, bends, cancel_break, .. } = env;
        for (h, bend) in bends {
            if let Some(v) = self.voices.iter_mut().find(|v| v.inst.addr == h) {
                v.bend = bend;
            }
        }
        self.voices.retain(|v| {
            let keep = !reset.contains(&v.inst.addr);
            if !keep {
                commands.entity(v.e).try_despawn();
            }
            keep
        });
        self.take_played(played, d, bs, commands);
        if cancel_break {
            pm.cancel_break();
        }
        for v in self.voices.iter_mut() {
            // the monitor's positional update (`BankInstance_UpdatePositional` 0x20b700, positions and Doppler on):
            // culled beyond the cull distance and started again within it, the Doppler pitch of the distance
            // closed, the volume falling off to none at the fall-off distance (30 when 0)
            if v.cull.ee() != 0.0 {
                v.playing = if v.playing { d.ee() <= v.cull.ee() } else { d.ee() < v.cull.ee() };
            }
            let (mut vol, mut pitch) = (0, F(1.0));
            if v.playing {
                if v.last.ee() != -1.0 {
                    pitch = bankmon::doppler_pitch(v.last - d, F(dt));
                }
                let far = if v.far.ee() == 0.0 { F(30.0) } else { v.far };
                vol = bankmon::instance_distance_volume(v.inst.volume as u32, d, far).min(0x7f);
            }
            v.last = d;
            if let Ok(mut sink) = sinks.get_mut(v.e) {
                // the volume (0..127) on the bank program's own level; the bend as the driver bends the voice
                // (`SndHandle_SetPitchBend` 0x2d6a18 → `SNDVoice_CalcPitchMult`) on the monitor's pitch (4096 = 1)
                // STANDIN: 0.8, for the sound manager's slot 4 volume (`SndMgr_GetSlot` 0x211f38)
                sink.set_volume(Volume::Linear(vol as f32 / 127.0 * bs.level(&v.name) * fx * 0.8));
                let p = tricky_game::sndlib::cvt_w((pitch * F(4096.0)).ee()).clamp(0, 0x3fff) as f32 / 4096.0;
                sink.set_speed((bs.pitch(&v.name, v.bend as f32) * p).clamp(0.25, 4.0));
            }
            if std::env::var("TRICKY_BOARDLOG").is_ok() && v.inst.volume != 0 {
                println!("board loop {}: volume {} heard {vol} at {} m, bend {} motion {}", v.name, v.inst.volume, d.ee(), v.bend, self.rider.motion);
            }
        }
    }
}

/// A voice of the rider's loops.
#[derive(Component)]
pub struct RiderLoop;

/// piecewise-linear curve through points, clamped at the ends
fn curve(pts: &[(f32, f32)], x: f32) -> f32 {
    if x <= pts[0].0 { return pts[0].1; }
    for w in pts.windows(2) { if x <= w[1].0 { return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0); } }
    pts[pts.len() - 1].1
}

#[derive(Component, Clone, Copy, PartialEq)]
pub enum BoardLoop { Wind, Boost }

#[derive(Resource, Default)]
pub struct BoardSounds { pub on: bool, files: HashMap<String, Handle<AudioSource>>, programs: Programs,
    /// per program (bank tags): root note, bend range (semitones), volume, detune (cents)
    meta: HashMap<String, (f32, f32, f32, f32)> }

impl BoardSounds {
    pub fn get(&self, name: &str) -> Option<Handle<AudioSource>> { self.files.get(name).cloned() }
    /// the loop part of a looping sample (else the whole sample)
    fn get_loop(&self, name: &str) -> Option<Handle<AudioSource>> { self.files.get(&format!("{name}_loop")).or(self.files.get(name)).cloned() }
    /// The sound driver's pitch (`SNDVoice_CalcPitchMult`): cents = detune − (root − 60)·100 +
    /// (bend − 64)·range·100/64, ratio 2^(cents/1200); bend does nothing without a range.
    pub fn pitch(&self, name: &str, bend: f32) -> f32 {
        let (root, range, _, detune) = self.meta.get(name).copied().unwrap_or((60.0, 0.0, 127.0, 0.0));
        let cents = detune - (root - 60.0) * 100.0 + (bend - 64.0) * range * 100.0 / 64.0;
        2f32.powf(cents / 1200.0)
    }
    pub fn level(&self, name: &str) -> f32 { self.meta.get(name).map_or(1.0, |m| m.2 / 127.0) }
}

pub fn setup_board_sound(mut commands: Commands, level: Res<LevelRes>, mut assets: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(RiderLoops::default());
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
    s.on = s.files.contains_key("board_4") && s.programs.0.iter().any(|m| !m.is_empty());
    println!("board sounds: {} samples, {} snow programs{}", s.files.len(), s.programs.0.iter().map(Vec::len).sum::<usize>(), if s.on { "" } else { " (not used)" });
    commands.insert_resource(s);
}

/// What changed since last frame.
#[derive(Default)]
pub struct Last { vel: Vec3, grounded: bool, crashed: bool, grab: u8, boosting: bool, mult: u32, speed_t: f32, spin_t: f32, respawns: u32, uber: bool }

#[allow(clippy::too_many_arguments)]
pub fn board_sound(
    time: Res<Time>, rider: Res<RiderRes>, game: Res<ui::Game>, mode: Res<Mode>, sounds: Res<crate::sound::Sounds>, bs: Res<BoardSounds>,
    list: Res<crate::LevelList>, mut rl: ResMut<RiderLoops>, mut pm: ResMut<crate::pathmusic::PathMusic>,
    mut commands: Commands, mut loops: Query<(Entity, &BoardLoop, &mut AudioSink), Without<RiderLoop>>, mut sinks: Query<&mut AudioSink, With<RiderLoop>>, mut last: Local<Last>,
    cam: Query<&GlobalTransform, With<Camera3d>>,
) {
    if !bs.on { return; }
    let r: &Rider = &rider.0;
    let dt = time.delta_secs().max(1e-4);
    // STANDIN: silent in the menus and while paused, for the game's pause of every voice (`Audio_PauseAll`)
    let fx = if sounds.effects && *mode == Mode::Ride && !matches!(game.screen, ui::Screen::Menu | ui::Screen::Paused) { 1.0 } else { 0.0 };
    let play = |commands: &mut Commands, name: &str, vol: f32, speed: f32| {
        if fx <= 0.0 { return; }
        if let Some(h) = bs.get(name) {
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol * bs.level(name) * 0.9), speed: speed * bs.pitch(name, 64.0), ..default() }));
        }
    };
    // the rider's loops, the game's
    let ride = (*mode == Mode::Ride && game.screen != ui::Screen::Menu).then_some(list.current);
    rl.frame(ride, r, cam.single().ok(), dt, &bs, fx, &mut pm, &mut commands, &mut sinks);
    // the game values, in cm/s
    let fwd = r.forward();
    let load = r.vel.dot(fwd).abs() * 100.0;
    // the surface's sound group (`Sfx_SurfaceSoundType` 0x20f908) for the one-shots below
    let g = tricky_game::sfxcalc::surface_sound_type(motion(r), r.rail_word, r.surface as u32);
    // big air: the wind comes up as the music ducks (Audio_BigAirMusicDuck)
    // STANDIN: our wind loop (zbxsfx 32) and its level, for the game's big-air sound
    if !loops.iter().any(|l| *l.1 == BoardLoop::Wind) {
        if let Some(h) = bs.get_loop("sfx_32") { commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), ..default() }, BoardLoop::Wind)); }
    }
    let t_air = if !r.grounded && r.rail.is_none() { r.air_time } else { 0.0 };
    let duck = if t_air > 1.5 && r.crashed <= 0.0 { (1.0 - (1.0 - 16.0 / 127.0) * 0.5 * t_air).clamp(16.0 / 127.0, 1.0) } else { 1.0 };
    for (_, kind, mut sink) in &mut loops {
        if *kind == BoardLoop::Wind {
            let want = (1.0 - duck) * bs.level("sfx_32") * fx * 0.8;
            let now = sink.volume().to_linear();
            sink.set_volume(Volume::Linear(now + (want - now) * (1.0 - (-dt * 30.0).exp())));
        }
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
        for (e, kind, _) in &loops { if *kind == BoardLoop::Boost { commands.entity(e).despawn(); } }
        if r.boost <= 0.0 { play(&mut commands, "sfx_108", 0.8, 1.0); }
    }
    *last = Last { vel: r.vel, grounded: r.grounded || r.rail.is_some(), crashed: r.crashed > 0.0, grab: r.grab, boosting: r.boosting, mult: r.multiplier, speed_t: r.speed_timer, spin_t: r.spin_timer,
        respawns: r.respawns, uber: r.uber_ready() };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snow_inf() {
        let text = "[PACK]\n[ICE]\n    [GLIDE]\n        Load Dig\n        Map 127\n        AssignVol\n        Load 64\n        AssignBend\n[POWDER]\n    [GLIDE]\n        Load Slip\n        Bound 0, 10\n        AssignVol\n";
        let p = Programs::parse(text);
        let b = BoardIn { normal: [1.0, 0.0, 0.0, 0.0], vel: [300.0, 0.0, 0.0, 0.0], f160: -50.0, f214: 0.5 };
        let run = |surface: u32| {
            let (mut w, mut vol, mut bend) = ([0; 0x14], 0, 64);
            p.run(sndqueue::GLIDE_PROGRAMS, surface, &mut w, &b, &mut vol, &mut bend);
            (vol, bend)
        };
        let (ice, pow) = (run(3), run(1));
        assert_eq!(ice.1, 64);
        assert!((0..=127).contains(&ice.0));
        assert!(pow.0 <= 10);
    }
}

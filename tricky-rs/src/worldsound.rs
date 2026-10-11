//! The course's own sounds: ambient emitters on the level's objects (crowds, snow cats, birds,
//! rivers: `WorldEmitter_GatherForListener` 0x22b370 / `WorldEmitter_Update` 0x22bf60), the
//! knock an object makes when a rider runs into it (`Sfx_InstanceCollision` 0x216c50) and the
//! level scripts' SoundPlay (op 8, `Sfx_ScriptSoundPlay` 0x2165c8: the fireworks).
//!
//! Data: `audio/world/` (every bank of DATA/AUDIO/AUDIO.BIG as wav + `worldsounds.json`, made by
//! tools/bnk/levelaudio.py + worldsounds.py) and each level's `audio/levelaudio.json` placements.
//!
//! Emitters: a sphere at Location + offset; inside its radius the volume is curve(d / radius)
//! (six curves, U6), with no other distance loss; one that drops out fades over 0.25 s; at most
//! 40 at once, and only one "swap" bank (the single-sample loops) sounds at a time.
//! Knocks and script sounds: one-shots scaled by ((30 - max(d - 0.5, 0)) / 30)^2 from the
//! camera (metres), none past 30 m; a knock's volume follows the rider's speed.

use crate::{ui, LevelRes, Mode, Props, RiderRes, World};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone)]
struct Emitter { pos: Vec3, radius: f32, curve: u8, id: u32, wav: Option<String>, looped: bool, swap: bool }

#[derive(Resource, Default)]
pub struct WorldSounds {
    pub on: bool,
    dir: PathBuf,
    files: HashMap<String, Handle<AudioSource>>,
    /// per file: (root note, volume 0..127, detune cents, pitch randomness cents)
    meta: HashMap<String, (f32, f32, f32, f32)>,
    speech: Vec<String>,
    kinds: HashMap<u32, (String, bool)>,
    level: Option<PathBuf>,
    emitters: Vec<Emitter>,
    collision: HashMap<usize, String>,
    /// trigger instance -> (where, sample) of the sounds its scripts play
    script: HashMap<usize, Vec<(Vec3, String)>>,
    /// emitters sounding: emitter -> (entity, current gain, wait before a one-shot repeats)
    active: HashMap<usize, (Entity, f32, f32)>,
    cool: HashMap<usize, f32>,
    seed: u32,
}

#[derive(Component)]
pub struct WorldLoop;

fn g2b(v: Vec3) -> Vec3 { Vec3::new(v.x, v.z, -v.y) * 0.01 }

/// The emitters' volume curves (x = distance / radius).
pub fn curve(kind: u8, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    match kind {
        0 => 1.0 - x * x,
        1 => 1.0 - x / (1.5 - 0.5 * x),
        3 => (1.0 - x) / (1.0 + 0.5 * x),
        4 => (1.0 - x) * (1.0 - x),
        5 => if x < 0.7 { 1.0 } else { (1.0 - x) / 0.3 },
        _ => 1.0 - x,
    }
}

/// How loud a knock is for the rider's speed (cm/s): the table in Sfx_InstanceCollision.
fn knock_level(cms: f32) -> f32 {
    let pts = [(200.0, 33.0), (400.0, 70.0), (550.0, 100.0), (800.0, 127.0)];
    if cms <= pts[0].0 { return pts[0].1 / 127.0; }
    for w in pts.windows(2) {
        if cms <= w[1].0 { return (w[0].1 + (w[1].1 - w[0].1) * (cms - w[0].0) / (w[1].0 - w[0].0)) / 127.0; }
    }
    1.0
}

/// The one-shots' loss with distance from the listener (metres).
fn falloff(d: f32) -> f32 { let k = ((30.0 - (d - 0.5).max(0.0)) / 30.0).max(0.0); k * k }

impl WorldSounds {
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        (self.seed % 10000) as f32 / 10000.0
    }
    fn get(&mut self, assets: &mut Assets<AudioSource>, name: &str) -> Option<Handle<AudioSource>> {
        if let Some(h) = self.files.get(name) { return Some(h.clone()); }
        let bytes = std::fs::read(self.dir.join(name)).ok()?;
        let h = assets.add(AudioSource { bytes: bytes.into() });
        self.files.insert(name.to_string(), h.clone());
        Some(h)
    }
    /// Pitch and level of a bank sample from its program settings (bnk2wav's json beside it).
    fn tune(&mut self, name: &str) -> (f32, f32) {
        if !self.meta.contains_key(name) {
            // "Bank/Bank_12_loop.wav" -> Bank/Bank.json, program 12
            let mut m = (60.0, 127.0, 0.0, 0.0);
            let path = std::path::Path::new(name);
            if let (Some(folder), Some(stem)) = (path.parent(), path.file_stem().map(|s| s.to_string_lossy().to_string())) {
                let bank = folder.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let rest = stem.strip_prefix(&format!("{bank}_")).unwrap_or(&stem);
                let prog = rest.split('_').next().unwrap_or("");
                if let Ok(text) = std::fs::read_to_string(self.dir.join(folder).join(format!("{bank}.json"))) {
                    if let Ok(v) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&text) {
                        if let Some(e) = v.get(prog) {
                            let g = |k: &str, d: f64| e.get(k).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
                            // a premixed set of layers already carries its levels and pitches
                            m = if rest.ends_with("mix") { (60.0, 127.0, 0.0, g("pitch_rand", 0.0)) } else { (g("root", 60.0), g("vol", 127.0), g("detune", 0.0), g("pitch_rand", 0.0)) };
                        }
                    }
                }
            }
            self.meta.insert(name.to_string(), m);
        }
        let (root, vol, detune, rand) = self.meta[name];
        let r = (self.rand() - 0.5) * rand;
        (2f32.powf((detune + r - (root - 60.0) * 100.0) / 1200.0), vol / 127.0)
    }
    fn load_level(&mut self, dir: &std::path::Path) {
        self.emitters.clear();
        self.collision.clear();
        self.script.clear();
        self.cool.clear();
        let Ok(text) = std::fs::read_to_string(dir.join("audio").join("levelaudio.json")) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(p) = v.get("placements") else { return };
        let vec3 = |x: Option<&serde_json::Value>| -> Option<Vec3> {
            let a = x?.as_array()?;
            Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
        };
        for e in p.get("emitters").and_then(|e| e.as_array()).into_iter().flatten() {
            let Some(pos) = vec3(e.get("pos")) else { continue };
            let id = e.get("id").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            // only the sphere records play here (the rest are not on the courses that ship)
            if e.get("type").and_then(|x| x.as_u64()).unwrap_or(0) != 0 { continue; }
            let (kind, looped) = self.kinds.get(&id).cloned().unwrap_or_default();
            let wav = e.get("wav").and_then(|x| x.as_str()).map(String::from);
            if wav.is_none() && kind != "speech" { continue; }
            self.emitters.push(Emitter {
                pos: g2b(pos), radius: e.get("radius").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32 * 0.01,
                curve: e.get("curve").and_then(|x| x.as_f64()).unwrap_or(2.0) as u8, id, wav, looped, swap: kind == "swap",
            });
        }
        let ids: HashMap<String, String> = p.get("collision_ids").and_then(|c| c.as_object()).map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.get("wav")?.as_str()?.to_string()))).collect()).unwrap_or_default();
        for (inst, id) in p.get("collision").and_then(|c| c.as_object()).into_iter().flatten() {
            let (Ok(i), Some(id)) = (inst.parse::<usize>(), id.as_u64()) else { continue };
            if matches!(id, 0x10 | 0x1c | 0x39) { continue; } // only once the object has been hit by a script
            if let Some(w) = ids.get(&id.to_string()) { self.collision.insert(i, w.clone()); }
        }
        for s in p.get("script_sounds").and_then(|e| e.as_array()).into_iter().flatten() {
            let (Some(by), Some(pos), Some(wav)) = (s.get("triggered_by").and_then(|x| x.as_u64()), vec3(s.get("pos")), s.get("wav").and_then(|x| x.as_str())) else { continue };
            self.script.entry(by as usize).or_default().push((g2b(pos), wav.to_string()));
        }
        println!("world sounds: {} emitters, {} objects with knocks, {} script triggers", self.emitters.len(), self.collision.len(), self.script.len());
    }
}

pub fn setup_world_sound(mut commands: Commands, level: Res<LevelRes>) {
    let audio_of = |p: &std::path::Path| p.parent().and_then(|p| p.parent()).map(|p| p.join("audio").join("world"));
    let given = std::path::absolute(&level.0.dir).unwrap_or(level.0.dir.clone());
    let real = std::fs::canonicalize(&level.0.dir).unwrap_or(given.clone());
    let mut ws = WorldSounds { seed: 0x2545f491, ..default() };
    if let Some(dir) = [audio_of(&given), audio_of(&real)].into_iter().flatten().find(|d| d.join("worldsounds.json").is_file()) {
        if let Ok(v) = std::fs::read_to_string(dir.join("worldsounds.json")).map_err(|_| ()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|_| ())) {
            for (k, e) in v.get("world_sounds").and_then(|w| w.as_object()).into_iter().flatten() {
                let Ok(id) = k.parse::<u32>() else { continue };
                let kind = e.get("kind").and_then(|x| x.as_str()).unwrap_or("").to_string();
                ws.kinds.insert(id, (kind, e.get("loop").and_then(|x| x.as_bool()).unwrap_or(false)));
            }
            ws.speech = v.get("speech").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            ws.on = true;
        }
        ws.dir = dir;
    }
    println!("world sounds: {}", if ws.on { ws.dir.display().to_string() } else { "not found (audio/world/worldsounds.json)".into() });
    commands.insert_resource(ws);
}

#[allow(clippy::too_many_arguments)]
pub fn world_sound(
    time: Res<Time>, level: Res<LevelRes>, world: Res<World>, game: Res<ui::Game>, mode: Res<Mode>, sounds: Res<crate::sound::Sounds>,
    mut ws: ResMut<WorldSounds>, mut rider: ResMut<RiderRes>, mut props: ResMut<Props>, mut parts: ResMut<crate::particles::Particles>,
    mut assets: ResMut<Assets<AudioSource>>, mut commands: Commands, cam: Query<&GlobalTransform, With<Camera3d>>,
    mut sinks: Query<&mut AudioSink, With<WorldLoop>>,
) {
    if !ws.on { return; }
    if ws.level.as_deref() != Some(level.0.dir.as_path()) {
        for (_, (e, _, _)) in ws.active.drain() { commands.entity(e).try_despawn(); }
        ws.level = Some(level.0.dir.clone());
        let dir = level.0.dir.clone();
        ws.load_level(&dir);
        let boxed = ws.script.keys().filter(|t| world.0.emit_boxes.contains_key(t)).count();
        if boxed < ws.script.len() { println!("world sounds: {} of {} script-sound triggers have no touch box", ws.script.len() - boxed, ws.script.len()); }
    }
    let dt = time.delta_secs().clamp(1e-4, 0.1);
    let fx = if sounds.effects && *mode == Mode::Ride && !matches!(game.screen, ui::Screen::Menu | ui::Screen::Paused) { 1.0 } else { 0.0 };
    let Ok(cam) = cam.single() else { return };
    let ear = cam.translation();

    // one-shots: knocks against walls and props, and the scripts' sounds
    for v in ws.cool.values_mut() { *v -= dt; }
    let mut shots: Vec<(String, f32)> = Vec::new();
    if let Some((c, speed)) = rider.0.wall_hit.take() {
        if let Some(i) = world.0.wall_instance(c, crate::rider::BODY_RADIUS + 0.05) {
            let i = i as usize;
            if let Some(w) = ws.collision.get(&i).cloned() {
                if ws.cool.get(&i).is_none_or(|t| *t <= 0.0) {
                    ws.cool.insert(i, 0.8);
                    shots.push((w, knock_level(speed * 100.0) * falloff(c.distance(ear))));
                }
            }
        }
    }
    for p in props.0.iter_mut() {
        let Some(speed) = p.knock.take() else { continue };
        if let Some(w) = ws.collision.get(&p.inst).cloned() { shots.push((w, knock_level(speed * 100.0) * falloff(p.pos.distance(ear)))); }
    }
    let fired: Vec<usize> = std::mem::take(&mut parts.fired);
    for t in fired {
        let Some(list) = ws.script.get(&t).cloned() else { continue };
        // a trigger can set off a whole row of objects; the game's voices run out after a few
        let mut near: Vec<(f32, String)> = list.into_iter().map(|(p, w)| (p.distance(ear), w)).filter(|(d, _)| *d < 30.0).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, w) in near.into_iter().take(4) { shots.push((w, falloff(d))); }
    }
    if fx > 0.0 {
        for (w, vol) in shots {
            if vol <= 0.01 { continue; }
            let (speed, lvl) = ws.tune(&w);
            if let Some(h) = ws.get(&mut assets, &w) {
                commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol * lvl * 0.9), speed, ..default() }));
            }
        }
    }

    // the emitters around the listener
    let swap_now: Option<u32> = ws.active.keys().filter_map(|i| ws.emitters.get(*i)).find(|e| e.swap).map(|e| e.id);
    let mut want: Vec<(usize, f32)> = Vec::new();
    for (i, e) in ws.emitters.iter().enumerate() {
        let d = e.pos.distance(ear);
        if d < e.radius { want.push((i, curve(e.curve, d / e.radius.max(1e-3)))); }
    }
    want.sort_by(|a, b| b.1.total_cmp(&a.1));
    want.truncate(40);
    let wanted: HashMap<usize, f32> = want.iter().copied().collect();
    let mut swap_now = swap_now;
    for (i, g) in want {
        if ws.active.contains_key(&i) || fx <= 0.0 { continue; }
        let e = ws.emitters[i].clone();
        if e.swap { if swap_now.is_some_and(|s| s != e.id) { continue; } swap_now = Some(e.id); }
        let file = match &e.wav { Some(w) => w.clone(), None => { if ws.speech.is_empty() { continue; } let k = (ws.rand() * ws.speech.len() as f32) as usize; ws.speech[k.min(ws.speech.len() - 1)].clone() } };
        let (speed, lvl) = ws.tune(&file);
        let Some(h) = ws.get(&mut assets, &file) else { continue };
        let mode = if e.looped { PlaybackMode::Loop } else { PlaybackMode::Despawn };
        let ent = commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode, volume: Volume::Linear(g * lvl * 0.6), speed, ..default() }, WorldLoop)).id();
        ws.active.insert(i, (ent, g * lvl, 0.0));
    }
    // follow the gains; fade out what has left its sphere (0.25 s); a sample that does not loop
    // comes back after a pause while the listener stays near
    let keys: Vec<usize> = ws.active.keys().copied().collect();
    for i in keys {
        let (ent, gain, wait) = ws.active[&i];
        let lvl = ws.emitters[i].wav.as_ref().map(|w| ws.meta.get(w).map_or(1.0, |m| m.1 / 127.0)).unwrap_or(1.0);
        let target = wanted.get(&i).map_or(0.0, |g| g * lvl) * fx;
        let g = if target < gain { (gain - dt / 0.25).max(target) } else { target };
        if let Ok(mut sink) = sinks.get_mut(ent) {
            // playing (wait < 0 marks "has started")
            sink.set_volume(Volume::Linear(g * 0.6));
            if g <= 0.0 && target <= 0.0 { commands.entity(ent).try_despawn(); ws.active.remove(&i); } else { ws.active.insert(i, (ent, g, -1.0)); }
        } else if wait < 0.0 {
            // a one-shot that has finished: rest a while before it may start again
            let w = 3.0 + 6.0 * ws.rand();
            ws.active.insert(i, (ent, g, w));
        } else if wait > 0.0 {
            let w = wait - dt;
            if w <= 0.0 || target <= 0.0 { ws.active.remove(&i); } else { ws.active.insert(i, (ent, g, w)); }
        } else if target <= 0.0 {
            // left before it got going
            commands.entity(ent).try_despawn();
            ws.active.remove(&i);
        }
    }
}

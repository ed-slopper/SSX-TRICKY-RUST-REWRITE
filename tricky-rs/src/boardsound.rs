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

#[derive(Clone, Debug)]
enum Op { Load(String), Num(f32), Map(f32), Square, Sqrt, Add(f32), Sub(f32), Mul(f32), Div(f32), Bound(f32, f32), Push, Pop, SAdd, SSub, SMul, SDiv, Neg, Abs, Vol, Bend, Nop }

/// The snow sound programs: per surface section name, per sub-section (GLIDE, AIGLIDE, CARVE).
#[derive(Default)]
struct Programs(HashMap<(String, String), Vec<Op>>);

impl Programs {
    fn parse(text: &str) -> Self {
        let mut out = HashMap::new();
        let mut tops: Vec<String> = Vec::new();
        let mut last_was_top = false;
        let mut sub: Option<String> = None;
        let mut ops: Vec<Op> = Vec::new();
        let flush = |tops: &Vec<String>, sub: &Option<String>, ops: &mut Vec<Op>, out: &mut HashMap<(String, String), Vec<Op>>| {
            if let Some(s) = sub { for t in tops { out.insert((t.clone(), s.clone()), ops.clone()); } }
            ops.clear();
        };
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("");
            let t = line.trim();
            if t.is_empty() { continue; }
            if t.starts_with('[') && t.ends_with(']') {
                let name = t[1..t.len() - 1].trim().to_uppercase();
                let indented = line.starts_with(' ') || line.starts_with('\t');
                flush(&tops, &sub, &mut ops, &mut out);
                if indented { sub = Some(name); last_was_top = false; }
                else {
                    if !last_was_top { tops.clear(); }
                    tops.push(name);
                    sub = None;
                    last_was_top = true;
                }
                continue;
            }
            last_was_top = false;
            let mut w = t.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty());
            let word = w.next().unwrap_or("").to_ascii_lowercase();
            let a: Vec<f32> = w.clone().filter_map(|x| x.parse().ok()).collect();
            let arg = a.first().copied().unwrap_or(0.0);
            ops.push(match word.as_str() {
                "load" => match w.next() { Some(x) => match x.parse::<f32>() { Ok(n) => Op::Num(n), Err(_) => Op::Load(x.to_ascii_lowercase()) }, None => Op::Nop },
                "loadconst" => Op::Num(arg),
                "map" => Op::Map(arg), "square" => Op::Square, "sqrt" => Op::Sqrt,
                "add" => Op::Add(arg), "sub" => Op::Sub(arg), "mul" => Op::Mul(arg), "div" => Op::Div(arg),
                "bound" => Op::Bound(arg, a.get(1).copied().unwrap_or(127.0)),
                "push" => Op::Push, "pop" => Op::Pop, "sadd" => Op::SAdd, "ssub" => Op::SSub, "smul" => Op::SMul, "sdiv" => Op::SDiv,
                "neg" => Op::Neg, "abs" => Op::Abs, "assignvol" => Op::Vol, "assignbend" => Op::Bend,
                _ => Op::Nop,
            });
        }
        flush(&tops, &sub, &mut ops, &mut out);
        Self(out)
    }
    /// Run a program: (volume, bend), both 0..127 (bend 64 = no change).
    fn run(&self, surface: &str, part: &str, inp: &Inputs) -> Option<(f32, f32)> {
        let ops = self.0.get(&(surface.to_string(), part.to_string()))?;
        let (mut acc, mut st, mut vol, mut bend) = (0.0f32, Vec::<f32>::new(), 0.0f32, 64.0f32);
        for op in ops {
            match op {
                Op::Load(n) => acc = match n.as_str() { "slip" => inp.slip, "dig" => inp.dig, "lean" => inp.lean, "board" | "load" => inp.load, "bend" => inp.bend, _ => 0.0 },
                Op::Num(v) => acc = *v,
                Op::Map(k) => acc = if *k != 0.0 { (acc * 127.0 / k).clamp(0.0, 127.0) } else { 0.0 },
                Op::Square => acc *= acc,
                Op::Sqrt => acc = acc.max(0.0).sqrt(),
                Op::Add(v) => acc += v, Op::Sub(v) => acc -= v, Op::Mul(v) => acc *= v,
                Op::Div(v) => if *v != 0.0 { acc /= v },
                Op::Bound(a, b) => acc = acc.clamp(a.min(*b), b.max(*a)),
                Op::Push => st.push(acc),
                Op::Pop => acc = st.pop().unwrap_or(0.0),
                Op::SAdd | Op::SSub | Op::SMul | Op::SDiv => {
                    let (b, a) = (st.pop().unwrap_or(0.0), st.pop().unwrap_or(0.0));
                    st.push(match op { Op::SAdd => a + b, Op::SSub => a - b, Op::SMul => a * b, _ => if b != 0.0 { a / b } else { 0.0 } });
                }
                Op::Neg => acc = -acc, Op::Abs => acc = acc.abs(),
                Op::Vol => vol = acc, Op::Bend => bend = acc,
                Op::Nop => {}
            }
        }
        Some((vol.clamp(0.0, 127.0), bend.clamp(0.0, 127.0)))
    }
}

/// The game values the programs read (`BoardIn_*`), in cm/s and cm/s².
#[derive(Default, Clone, Copy)]
struct Inputs { load: f32, slip: f32, dig: f32, lean: f32, bend: f32 }

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
pub struct Last { vel: Vec3, grounded: bool, crashed: bool, grab: u8, boosting: bool, mult: u32, speed_t: f32, spin_t: f32, respawns: u32, uber: bool, group: Option<usize>, fwd_speed: f32, slip: f32 }

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
    // slip: how hard the snow is slowing the board along its length (the forward drag)
    let decel = ((last.fwd_speed - load) / dt).max(0.0);
    last.slip += (decel - last.slip) * (1.0 - (-dt * 20.0).exp());
    last.fwd_speed = load;
    let dig = r.input.steer.abs() * 127.0;
    let lean = load / 4.0 + 2.0 * last.slip;
    let inp = Inputs { load, slip: last.slip, dig, lean, bend: lean + load };
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
        respawns: r.respawns, uber: r.uber_ready(), group: last.group, fwd_speed: last.fwd_speed, slip: last.slip };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snow_inf() {
        let text = "[PACK]\n[ICE]\n    [GLIDE]\n        Load Dig\n        Map 127\n        AssignVol\n        Load 64\n        AssignBend\n[POWDER]\n    [GLIDE]\n        Load Slip\n        Bound 0, 10\n        AssignVol\n";
        let p = Programs::parse(text);
        let inp = Inputs { dig: 100.0, slip: 50.0, ..Default::default() };
        assert_eq!(p.run("ICE", "GLIDE", &inp), Some((100.0, 64.0)));
        assert_eq!(p.run("PACK", "GLIDE", &inp), Some((100.0, 64.0)));
        assert_eq!(p.run("POWDER", "GLIDE", &inp).map(|v| v.0), Some(10.0));
    }
}

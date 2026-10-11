//! The pre-race intro: the original's camera director scripts from the course's .cml files
//! (`cBxCamera_RunDirectorScript`, `cPreRaceHandler`). A fly-through of the course, then a shot of
//! the start gate, then the chase camera and the countdown.
//!
//! A .cml file: word 0 a hash, then the first record of each type (0 cameras, 2 director scripts,
//! 4 camera regions, ...). Every record has a 0x20-byte header (+0 next of the type, +8 body,
//! +0x10 name) and a body of fields; 0xDEADC0ED marks a field left unset.
use glam::Vec3;
use std::path::Path;

const NONE: u32 = 0xDEAD_C0ED;

/// The course's camera files: its own, then the shared ones (searched in that order).
pub struct Cml { files: Vec<Vec<u8>> }

/// A fixed camera: where it is (game units), where it looks (yaw/pitch, radians, or at the rider),
/// and its field of view (radians, across the picture).
#[derive(Clone, Copy, Debug)]
pub struct Shot { pub pos: Vec3, pub yaw: f32, pub pitch: f32, pub fov: f32, pub at_rider: bool, /// 1: relative to the start stage area
    pub frame: u32 }

/// One rider's part in a scene (`cBxCamera_StartScene`): where to stand (game units; `frame` 0 =
/// absolute, 1 = relative to the course's start stage area, 2 = finish area) and whether to be put there.
#[derive(Clone, Copy, Debug)]
pub struct SceneSpot { pub place: bool, pub frame: u32, pub pos: Vec3, pub yaw: f32 }

#[derive(Clone, Debug)]
struct Entry { name: String, op: u32, curve: u32, p: f32, t0: f32, t1: f32 }

fn word(d: &[u8], o: usize) -> u32 { d.get(o..o + 4).map_or(NONE, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) }
fn float(d: &[u8], o: usize) -> Option<f32> { let w = word(d, o); (w != NONE).then(|| f32::from_bits(w)) }
fn name(d: &[u8], o: usize) -> String {
    let b = d.get(o..o + 16).unwrap_or(&[]);
    String::from_utf8_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())]).into_owned()
}

impl Cml {
    /// `dir/Camera/{track,commonob,scripts}.cml`, copied from the game disc's DATA/CAMERA.
    pub fn load(dir: &Path) -> Option<Self> {
        let files: Vec<Vec<u8>> = ["track.cml", "commonob.cml", "scripts.cml"].iter()
            .filter_map(|f| std::fs::read(dir.join("Camera").join(f)).ok()).collect();
        (!files.is_empty()).then_some(Self { files })
    }
    fn find(&self, kind: usize, want: &str) -> Option<(&[u8], usize)> {
        for d in &self.files {
            let mut r = word(d, 4 + 4 * kind);
            let mut guard = 0;
            while r != 0xFFFF_FFFF && (r as usize) + 0x20 < d.len() && guard < 10_000 {
                if name(d, r as usize + 0x10).eq_ignore_ascii_case(want) { return Some((d, word(d, r as usize + 8) as usize)); }
                r = word(d, r as usize);
                guard += 1;
            }
        }
        None
    }
    pub fn has(&self, script: &str) -> bool { self.find(2, script).is_some() }
    /// A named camera; None for one that rides along with a rider (the chase cameras).
    pub fn camera(&self, n: &str) -> Option<Shot> {
        let (d, b) = self.find(0, n)?;
        if word(d, b) == 1 { return None; }
        Some(Shot {
            pos: Vec3::new(float(d, b + 0x3c)?, float(d, b + 0x68)?, float(d, b + 0x94)?),
            at_rider: word(d, b + 0x120) == 1,
            yaw: float(d, b + 0x134).unwrap_or(0.0),
            pitch: float(d, b + 0x138).unwrap_or(0.0),
            fov: float(d, b + 0x140).unwrap_or(1.0),
            frame: match word(d, b + 0x30) { f @ (1 | 2) => f, _ => 0 },
        })
    }
    /// A scene's riders in order (up to eight).
    pub fn scene(&self, n: &str) -> Vec<SceneSpot> {
        let Some((d, b)) = self.find(5, n) else { return Vec::new() };
        (0..8).map(|i| b + i * 0x30).filter(|e| word(d, e + 0x10) != 0 && word(d, e + 0x10) != NONE).map(|e| SceneSpot {
            place: word(d, e + 0x18) == 1, frame: word(d, e + 0x1c),
            pos: Vec3::new(float(d, e + 0x20).unwrap_or(0.0), float(d, e + 0x24).unwrap_or(0.0), float(d, e + 0x28).unwrap_or(0.0)),
            yaw: float(d, e + 0x2c).unwrap_or(0.0),
        }).collect()
    }
    fn script(&self, n: &str) -> Option<Vec<Entry>> {
        let (d, b) = self.find(2, n)?;
        let count = (word(d, b + 0x370) as usize).min(20);
        Some((0..count).map(|i| {
            let e = b + i * 0x2c;
            Entry { name: if word(d, e) == NONE { String::new() } else { name(d, e) }, op: word(d, e + 0x10), curve: word(d, e + 0x14),
                p: float(d, e + 0x1c).unwrap_or(100.0), t0: float(d, e + 0x20).unwrap_or(0.0), t1: float(d, e + 0x24).unwrap_or(0.0) }
        }).collect())
    }
}

/// The blend curves (`Hermite1D(p0, p1, m0, m1)`): 0 linear, 1..3 eased.
fn curve(kind: u32, u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let (m0, m1) = match kind { 1 => (0.0, 1.0), 2 => (0.0, 0.0), 3 => (1.0, 0.0), _ => return u };
    let (u2, u3) = (u * u, u * u * u);
    (-2.0 * u3 + 3.0 * u2) + (u3 - 2.0 * u2 + u) * m0 + (u3 - u2) * m1
}
fn wrap(a: f32) -> f32 { (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI }
fn mix(a: &Shot, b: &Shot, u: f32) -> Shot {
    Shot { pos: a.pos.lerp(b.pos, u), yaw: a.yaw + wrap(b.yaw - a.yaw) * u, pitch: a.pitch + (b.pitch - a.pitch) * u, fov: a.fov + (b.fov - a.fov) * u, at_rider: b.at_rider, frame: b.frame }
}

/// Runs director scripts one after another.
pub struct Director {
    queue: Vec<String>,
    script: Vec<Entry>,
    stack: Vec<(Vec<Entry>, usize, f32)>,
    i: usize,
    t: f32,
    blend_from: Option<Shot>,
    pub shot: Option<Shot>,
    /// the scene the riders are playing (director op 0xE starts one, 0xF stops it), and a white flash
    pub scene: Option<String>,
    pub flash: f32,
    rng: u32,
    pub done: bool,
}
impl Director {
    pub fn new(cml: &Cml, scripts: &[&str], seed: u32) -> Self {
        let mut d = Self { queue: scripts.iter().rev().map(|s| s.to_string()).collect(), script: Vec::new(), stack: Vec::new(), i: 0, t: 0.0, blend_from: None, shot: None, scene: None, flash: 0.0, rng: seed | 1, done: false };
        d.next_script(cml);
        d
    }
    fn rand100(&mut self) -> f32 { self.rng ^= self.rng << 13; self.rng ^= self.rng >> 17; self.rng ^= self.rng << 5; (self.rng % 100) as f32 }
    fn next_script(&mut self, cml: &Cml) {
        while let Some(n) = self.queue.pop() {
            if let Some(s) = cml.script(&n) { self.script = s; self.i = 0; self.t = 0.0; return; }
        }
        self.done = true;
    }
    fn end_script(&mut self, cml: &Cml) {
        if let Some((s, i, t)) = self.stack.pop() { self.script = s; self.i = i; self.t = t; } else { self.next_script(cml); }
    }
    /// Advance by `dt` seconds; the current camera is left in `shot`.
    pub fn step(&mut self, cml: &Cml, dt: f32) {
        if self.done { return; }
        self.t += dt;
        self.flash = (self.flash - dt).max(0.0);
        for _ in 0..64 {
            if self.done { return; }
            let Some(e) = self.script.get(self.i).cloned() else { self.end_script(cml); continue };
            if e.t0 > self.t + 1e-4 { return; }
            match e.op {
                0 => { if let Some(s) = cml.camera(&e.name) { self.shot = Some(s); } else { self.done = true; return; } self.i += 1; }
                1 | 2 | 3 | 0x1c => {
                    let Some(to) = cml.camera(&e.name) else { self.i += 1; continue };
                    let from = *self.blend_from.get_or_insert(self.shot.unwrap_or(to));
                    if self.t < e.t1 && e.t1 > e.t0 {
                        self.shot = Some(mix(&from, &to, curve(e.curve, (self.t - e.t0) / (e.t1 - e.t0))));
                        return;
                    }
                    self.shot = Some(to);
                    self.blend_from = None;
                    self.i += 1;
                }
                // a random branch: go to that script with probability p (the last one always)
                10 => {
                    if self.rand100() < e.p { if let Some(s) = cml.script(&e.name) { self.script = s; self.i = 0; self.t = 0.0; continue; } }
                    self.i += 1;
                }
                11 => { if let Some(s) = cml.script(&e.name) { self.stack.push((std::mem::take(&mut self.script), self.i + 1, self.t)); self.script = s; self.i = 0; self.t = 0.0; } else { self.i += 1; } }
                // scenes: the riders act something out; 0x17 is a white flash (0.6 s)
                0xe => { self.scene = Some(e.name.clone()); self.i += 1; }
                0xf => { self.scene = None; self.i += 1; }
                0x17 => { self.flash = 0.6; self.i += 1; }
                // hold, back to the chase camera, end
                7 | 0xd | 0x20 => { self.end_script(cml); }
                _ => { self.i += 1; }
            }
        }
    }
}

/// The riders acting out a scene: each rider's looping clip, where each is to stand (Bevy space,
/// yaw), the time into the scene, and where they were before (put back when it ends).
pub struct SceneState { pub name: String, pub clips: Vec<String>, pub spots: Vec<Option<(Vec3, f32)>>, pub t: f32, pub saved: Vec<Option<(Vec3, f32)>>, pub ended: bool,
    /// which scene slot each rider (0 = the player, then the opponents) plays; None: not in it
    pub slot_of: Vec<Option<usize>> }
impl SceneState {
    pub fn slot(&self, rider: usize) -> Option<usize> { if self.slot_of.is_empty() { Some(rider) } else { self.slot_of.get(rider).copied().flatten() } }
}

/// The clips each rider plays in the original's staging (S_COMn) and gate (6COM_n) scenes.
pub fn scene_clips(name: &str) -> Vec<String> {
    let v = |a: &[&str]| a.iter().map(|s| format!("scrSG_{s}")).collect::<Vec<_>>();
    match name {
        "S_COM1" => v(&["GLIDE1", "MAD", "POLISHBOARD", "TALK1", "SITLEGSTRETCH", "BINDING"]),
        "S_COM2" => v(&["CROUCHING", "TALKA1", "TALKA2", "TOUCHTOES", "WALLSTRETCH1", "WEIGHTSHIFT"]),
        "S_COM3" => v(&["CROUCHSTRETCH", "POLISHBOARDWALL", "TWIST", "TALKB1", "TALKB2", "TALK4"]),
        "S_COM4" => v(&["GLIDE2", "SITNERVOUS", "SQUATSTRETCH", "STRETCH1", "TALKC1", "TALKC2"]),
        n if n.starts_with("6COM_") => { let k = &n[5..]; ["a", "b", "c", "d", "e", "f"].iter().map(|c| format!("scrGAT_6COM{k}{c}")).collect() }
        _ => Vec::new(),
    }
}

/// The characters' short names in the post-race clips and scripts (scrEdd_WIN_1, FL_EDD_VS_USR), in
/// the original's character order (Eddie, Kaori, Luther, Mac, Moby, Zoe, JP, Elise, Psymon, Seeiah, Brodi, Marisol).
pub const CODES: [&str; 12] = ["Edd", "Kao", "Lut", "Mac", "Mob", "Zoe", "JP", "Eli", "Psy", "See", "Bro", "Mar"];

/// Who plays what after the race: the player's character and clip number, and the rival (character,
/// rider index, clip number) if one has it in for the player.
#[derive(Clone, Debug)]
pub struct Cast { pub player: usize, pub variant: u32, pub rival: Option<(usize, usize, u32)>, pub riders: usize }

/// The win and lose rows (`cEndRaceHandler_Update`, tables 0x37c1b8 / 0x37c4b0): per character, the
/// player's clip number and the script (FL_WIN_n / FL_LOS_n) it goes with; one is picked at random.
const WIN_ROWS: [&[(u32, u32)]; 12] = [
    &[(1, 3), (2, 3), (4, 3)], &[(1, 1), (2, 1), (3, 1)], &[(2, 1), (3, 1), (4, 1)], &[(1, 1), (3, 3), (4, 1)],
    &[(1, 2), (2, 2)], &[(1, 3), (3, 1), (4, 2)], &[(2, 2), (3, 2), (4, 2)], &[(1, 2), (2, 1), (4, 3)],
    &[(1, 2), (2, 2), (3, 2)], &[(2, 2), (3, 1), (4, 1)], &[(2, 1), (3, 1), (4, 1)], &[(1, 2), (2, 2), (3, 2)],
];
const LOSE_ROWS: [&[(u32, u32)]; 12] = [
    &[(1, 3), (2, 3), (3, 2), (4, 2)], &[(1, 2), (2, 2), (3, 2), (4, 2)], &[(1, 3), (2, 2), (3, 2), (4, 2)], &[(1, 2), (2, 2), (3, 2), (4, 1)],
    &[(1, 2), (2, 2), (3, 2), (4, 2)], &[(1, 2), (2, 2), (3, 2), (4, 2)], &[(1, 2), (2, 2), (3, 2), (4, 2)], &[(1, 2), (2, 3), (3, 2), (4, 3)],
    &[(1, 1), (2, 3), (3, 2), (4, 3)], &[(1, 2), (2, 2), (3, 2), (4, 2)], &[(1, 1), (2, 3), (3, 3), (4, 3)], &[(1, 2), (2, 2), (3, 2), (4, 2)],
];

/// The post-race scripts: the rival's taunt (FL_<RIV>_VS_USR) if there is one, then the win or lose scene.
pub fn plan_post(cml: &Cml, player: usize, win: bool, rival: Option<(usize, usize)>, riders: usize, seed: u32) -> (Vec<String>, Cast) {
    let player = player.min(11);
    let rows = if win { WIN_ROWS[player] } else { LOSE_ROWS[player] };
    let (variant, script) = rows[(seed % rows.len() as u32) as usize];
    let mut scripts = Vec::new();
    let rival = rival.and_then(|(ch, k)| {
        let name = format!("FL_{}_VS_USR", CODES[ch.min(11)].to_uppercase());
        if !cml.has(&name) { return None; }
        scripts.push(name);
        // the rival pair's clip number: four of each, fewer for some (Psymon's, Elise's)
        let choices: &[u32] = if player == 8 { &[1, 3] } else if ch == 8 || player == 7 { &[1, 2, 3] } else { &[1, 2, 3, 4] };
        Some((ch, k, choices[((seed / 5) % choices.len() as u32) as usize]))
    });
    scripts.push(format!("FL_{}_{script}", if win { "WIN" } else { "LOS" }));
    (scripts, Cast { player, variant, rival, riders })
}

/// The clips of a post-race scene by slot, and which slot each rider plays (the player first, the
/// rival second in a rival scene, everyone else in order).
pub fn post_scene(name: &str, cast: &Cast) -> (Vec<String>, Vec<Option<usize>>) {
    let me = CODES[cast.player];
    let common = |n: usize| (1..=n).map(|i| format!("scrFL_Common{i}")).collect::<Vec<_>>();
    let mut slot_of = vec![None; cast.riders];
    if name.eq_ignore_ascii_case("WIN_1") || name.eq_ignore_ascii_case("LOS_1") {
        let kind = if name.to_ascii_uppercase().starts_with("WIN") { "WIN" } else { "LOS" };
        let mut clips = vec![format!("scr{me}_{kind}_{}", cast.variant)];
        clips.extend(common(5));
        for (k, s) in slot_of.iter_mut().enumerate() { if k < 6 { *s = Some(k); } }
        (clips, slot_of)
    } else if let Some((ch, rk, n)) = cast.rival {
        let mut clips = vec![format!("scr{me}_RCON_{n}"), format!("scr{}_CON_{n}", CODES[ch])];
        clips.extend(common(4));
        let mut next = 2;
        for (k, s) in slot_of.iter_mut().enumerate() {
            *s = if k == 0 { Some(0) } else if k == rk { Some(1) } else if next < 6 { next += 1; Some(next - 1) } else { None };
        }
        (clips, slot_of)
    } else { (scene_clips(name), Vec::new()) }
}

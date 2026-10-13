//! The camera script machine's path steps (board row E11f): `Camera_Cut` 0x177db0, `CamScript_EndPath` 0x175c70,
//! `CamScript_UpdatePath` 0x175cb0, `CamScript_StartPath` 0x175ba0 and `CamPath_EvalStart` 0x170510, on the camera
//! object's words (`obj`; the path lives at +0x1e8, the wanted record at +0x484). Checked in the function runner
//! (`tests/camscript.rs`).

use crate::campath::{eval, Angles, KeyEnv, Owner, PathState};

/// The path inside the camera object: +0x1e4 the running flag, +0x1e8 the path (word 0 the class, then `idx`, `t`,
/// `dt`, the last point, the speed, the easing mode, the owner's pointer).
const RUNNING: usize = 0x1e4 / 4;
const PATH: usize = 0x1e8 / 4;
const REC: usize = 0x484 / 4;

/// What the path steps call.
pub trait ScriptEnv: KeyEnv {
    /// The path owner's keyframe count and default speed (`*(path + 0x24)`).
    fn owner(&mut self, ptr: u32) -> Owner;
    /// `Camera_SmoothOutput` 0x174238 and `Camera_UpdateCutFade` 0x174b38 on the object.
    fn smooth(&mut self, obj: &mut [u32]);
    fn cut_fade(&mut self, obj: &mut [u32]);
    /// The sound call `Camera_Cut` ends in (0x20c3e8 on the sound manager).
    fn audio_cut(&mut self);
    /// `CamPath::CamPath` 0x170470: the owner it looks up (`Cml_FindObject` 0x16e518 with 3).
    fn find_owner(&mut self) -> u32;
    /// `CamPath_EvalStart`'s first point: the owner's first camera, its position evaluated for `list` (the object's
    /// +0x4e8) with the cut flag 1.
    fn first_position(&mut self, owner: u32) -> [f32; 3];
}

fn read_path(obj: &[u32]) -> PathState {
    let f = f32::from_bits;
    PathState {
        idx: obj[PATH + 1] as i32,
        t: f(obj[PATH + 2]),
        dt: f(obj[PATH + 3]),
        last: [f(obj[PATH + 4]), f(obj[PATH + 5]), f(obj[PATH + 6])],
        speed: f(obj[PATH + 7]),
        mode: obj[PATH + 8] as i32,
    }
}

fn write_path(obj: &mut [u32], p: &PathState) {
    obj[PATH + 1] = p.idx as u32;
    obj[PATH + 2] = p.t.to_bits();
    obj[PATH + 3] = p.dt.to_bits();
    for k in 0..3 {
        obj[PATH + 4 + k] = p.last[k].to_bits();
    }
    obj[PATH + 7] = p.speed.to_bits();
    obj[PATH + 8] = p.mode as u32;
}

/// The wanted record `CamPath_Eval` hands back: position, the three angles, the fourth value and 100.0, the rest zero
/// (the original leaves those words of its stack record as they were; the camera object takes them as they come).
pub fn record_words(a: &Angles) -> [u32; 22] {
    let mut w = [0u32; 22];
    for k in 0..3 {
        w[k] = a.pos[k].to_bits();
        w[3 + k] = a.ang[k].to_bits();
    }
    w[6] = a.w.to_bits();
    w[7] = 100.0f32.to_bits();
    w
}

/// `Camera_Cut` 0x177db0: flags the object's cut (+4 and +8) and tells the sound manager.
pub fn camera_cut(obj: &mut [u32], env: &mut dyn ScriptEnv) {
    obj[2] = 1;
    obj[1] = 1;
    env.audio_cut();
}

/// `CamScript_EndPath` 0x175c70: a running path ends in a cut.
pub fn end_path(obj: &mut [u32], env: &mut dyn ScriptEnv) {
    if obj[RUNNING] == 1 {
        camera_cut(obj, env);
    }
    obj[RUNNING] = 0;
}

/// `CamScript_UpdatePath` 0x175cb0: one tick of the running path into the object's wanted record, then the
/// smoothing and the cut fade; a path that reached its end ends.
pub fn update_path(obj: &mut [u32], env: &mut dyn ScriptEnv) {
    if obj[RUNNING] != 1 {
        return;
    }
    obj[1] = 1;
    let mut p = read_path(obj);
    let owner = env.owner(obj[PATH + 9]);
    let (a, ended) = eval(&mut p, &owner, env);
    write_path(obj, &p);
    let rec = record_words(&a);
    obj[REC..REC + 22].copy_from_slice(&rec);
    env.smooth(obj);
    env.cut_fade(obj);
    if ended {
        end_path(obj, env);
    }
}

/// `CamPath_EvalStart` 0x170510: a path with keyframes starts at its first segment, `dt` 0.05, at the first
/// camera's position. (An owner without keyframes leaves the path as it was.)
pub fn eval_start(obj: &mut [u32], env: &mut dyn ScriptEnv) {
    let owner = obj[PATH + 9];
    if env.owner(owner).count <= 0 {
        return;
    }
    obj[PATH + 2] = 0;
    obj[PATH + 1] = 0;
    obj[PATH + 3] = 0.05f32.to_bits();
    let pos = env.first_position(owner);
    for k in 0..3 {
        obj[PATH + 4 + k] = pos[k].to_bits();
    }
}

/// `CamScript_StartPath` 0x175ba0(speed, object, mode): the path is made anew for the owner it finds, started, and
/// given the speed and easing mode.
pub fn start_path(obj: &mut [u32], speed: f32, mode: i32, env: &mut dyn ScriptEnv) {
    obj[RUNNING] = 1;
    obj[PATH + 9] = env.find_owner();
    eval_start(obj, env);
    obj[PATH + 8] = mode as u32;
    obj[PATH + 7] = speed.to_bits();
}

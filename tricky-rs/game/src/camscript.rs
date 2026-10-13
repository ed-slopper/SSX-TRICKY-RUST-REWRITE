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
    /// `FUN_00231318(DAT_00343050)`: the sound manager's camera-cut call that `CamScript_Stop` makes first.
    fn audio_stop(&mut self);
    /// `gRace_EntryCount` (0x32db6c) and, for rider `j`, `AnimCtl_Reset(Race_GetRider(race, j) + 0x46c0)`.
    fn entry_count(&mut self) -> i32;
    fn reset_rider_anim(&mut self, j: i32);
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

/// The camera names (`obj + 0x24` the current one, `obj + 0x50` the script's) are 16-byte strings in the object.
const CURRENT_NAME: usize = 0x24;
const SCRIPT_NAME: usize = 0x50;

fn name_at(obj: &[u32], byte: usize) -> Vec<u8> {
    let mut v = vec![];
    for k in 0..0x10 {
        let b = (obj[(byte + k) / 4] >> (8 * ((byte + k) % 4))) as u8;
        if b == 0 {
            break;
        }
        v.push(b);
    }
    v
}

/// `strcpy` into the object: the bytes and the NUL.
pub fn copy_name(obj: &mut [u32], byte: usize, name: &[u8]) {
    for (k, b) in name.iter().chain(std::iter::once(&0u8)).enumerate() {
        let (w, sh) = ((byte + k) / 4, 8 * ((byte + k) % 4));
        obj[w] = (obj[w] & !(0xff << sh)) | (*b as u32) << sh;
    }
}

/// `CamScript_ResetActorAnims` 0x176f38: when a scene's actors were started (+0x210), each started actor slot
/// (0x40 bytes from +0x230) is cleared and every rider's animation control is reset.
pub fn reset_actor_anims(obj: &mut [u32], env: &mut dyn ScriptEnv) {
    if obj[0x210 / 4] == 0 {
        return;
    }
    for i in 0..8 {
        let w = (0x230 + i * 0x40) / 4;
        if obj[w] != 0 {
            let mut j = 0;
            if 0 < env.entry_count() {
                loop {
                    env.reset_rider_anim(j);
                    j += 1;
                    if j >= env.entry_count() {
                        break;
                    }
                }
            }
            obj[w] = 0;
        }
    }
    obj[0x210 / 4] = 0;
}

/// `CamScript_SetName` 0x175e88: the script to run (time zero, its first step).
pub fn set_name(obj: &mut [u32], name: &[u8]) {
    obj[0x4c / 4] = 0;
    copy_name(obj, SCRIPT_NAME, name);
    obj[0x60 / 4] = 0;
}

/// `CamScript_Start` 0x175de0(object, name, flag): with the flag, ends any path, clears the actors and the script's
/// state and starts the named script.
pub fn start(obj: &mut [u32], name: &[u8], flag: bool, env: &mut dyn ScriptEnv) {
    if !flag {
        return;
    }
    end_path(obj, env);
    reset_actor_anims(obj, env);
    obj[0xc / 4] = 0;
    obj[0x20 / 4] = 0;
    obj[0x38 / 4] = 0;
    obj[0x420 / 4] = 0;
    obj[0x44 / 4] = 0;
    obj[0x34 / 4] = 1;
    set_name(obj, name);
    obj[0x48 / 4] = 0;
}

/// `CamScript_Stop` 0x175eb8(object, flag): a running script ends in a cut unless the current camera is "chase near"
/// and the mode word (+0x428) is 4; the actors and the path are cleared and the script's state zeroed.
pub fn stop(obj: &mut [u32], flag: bool, env: &mut dyn ScriptEnv) {
    if obj[0x34 / 4] == 1 {
        if flag {
            env.audio_stop();
        }
        if name_at(obj, CURRENT_NAME) != b"chase near" || obj[0x428 / 4] != 4 {
            camera_cut(obj, env);
        }
    }
    reset_actor_anims(obj, env);
    end_path(obj, env);
    for w in [0x44, 0xeb0, 0x34, 0x38, 0x3c, 0x420] {
        obj[w / 4] = 0;
    }
}

/// `CamScript_IsRunning` 0x175e50: the script's state, except 0 once it holds on a still camera (+0x3c and +0x40
/// both 1).
pub fn is_running(obj: &[u32]) -> u32 {
    if obj[0x34 / 4] == 1 && obj[0x3c / 4] == 1 && obj[0x40 / 4] == 1 {
        return 0;
    }
    obj[0x34 / 4]
}

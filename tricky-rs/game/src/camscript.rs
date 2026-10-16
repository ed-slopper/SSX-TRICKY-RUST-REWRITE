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

/// One 0x2c-byte step of a camera director script: +0 the camera name, +0x10 the kind, +0x14 and +0x18 the
/// arguments (a mode and a speed), +0x1c a float, +0x20 and +0x24 the start and end times, +0x28 a word.
#[derive(Clone, Debug, Default)]
pub struct Step {
    pub name: Vec<u8>,
    pub kind: u32,
    pub a: u32,
    pub b: f32,
    pub c: f32,
    pub start: f32,
    pub end: f32,
    pub w28: u32,
}

/// What `CamScript_Update` calls. A director is `CamDirector::CamDirector` 0x16e6b0 on the script's name: `dp` is
/// the data pointer it hands back (0 when the script is missing), `i` a step of it.
pub trait UpdateEnv: ScriptEnv {
    fn director_open(&mut self, name: &[u8]) -> u32;
    fn director_close(&mut self, dp: u32);
    fn step(&mut self, dp: u32, i: i32) -> Step;
    /// The director's step count (`dp + 0x370`).
    fn count(&mut self, dp: u32) -> i32;
    /// `FUN_00230ff8(DAT_00343050)`.
    fn wait(&mut self) -> u32;
    /// `Rand_Fx` 0x23d4a8.
    fn rand(&mut self) -> u32;
    /// `CamScript_CutTo` 0x174c98 to the current camera name (+0x24) or to a step.
    fn cut_to_current(&mut self, obj: &mut [u32]);
    fn cut_to_step(&mut self, obj: &mut [u32], dp: u32, i: i32);
    /// `CamScript_Blend` 0x175a08 (returns whether the step is done).
    #[allow(clippy::too_many_arguments)]
    fn blend(&mut self, obj: &mut [u32], dp: u32, i: i32, mode: u32, orbit: u32, flip: u32, start: f32, now: f32, end: f32) -> u32;
    /// `CamScript_StartPath` 0x175ba0 given the step.
    fn start_path_step(&mut self, obj: &mut [u32], dp: u32, i: i32, mode: u32, speed: f32);
    /// `Camera_SetMode` 0x1776d0.
    fn set_mode(&mut self, obj: &mut [u32], mode: u32);
    /// `CamScript_StartScene` 0x176850.
    fn start_scene(&mut self, obj: &mut [u32], dp: u32, i: i32);
    /// `IsRiderAnimDone` 0x16db88 on the rider animation struct (+0x220).
    fn rider_anim_done(&mut self) -> u32;
    /// The sound manager's calls: 0x214e08 and 0x214e00 (kinds 0x11 and 0x12), 0x2312b0 (is it busy), 0x230ee0 with
    /// a word, and 0x2310c8 with the list pointer and 0, 2 or 1.
    fn sound_a(&mut self);
    fn sound_b(&mut self);
    fn sound_busy(&mut self) -> u32;
    fn sound_set(&mut self, w: u32);
    fn sound_list(&mut self, list: u32, mode: u32);
    /// The three statics at 0x3365f8, 0x3365fc and 0x336600 that kind 0x17 sets (1, 0 and 0.5).
    fn set_statics(&mut self);
    /// `World_RunScript` 0x13bad8 on "CameraScript_%d".
    fn run_script(&mut self, name: &[u8]);
    /// The race player's flag (`*(gApp + 0x730) + 0x4c`).
    fn race_flag(&mut self, v: u32);
    /// A word of the list (obj +0x4e8) record: +0x114.
    fn list_flag(&mut self, list: u32) -> u32;
}

const ELAPSED: usize = 0x4c / 4;

/// `CamScript_Update` 0x175f50: one tick of the camera script. Time advances 1/60; the director's steps whose start
/// has passed run one after another (each kind by its own rule), a finished step moves on to the next and the end of
/// a script pops the stack of scripts or stops.
pub fn update(obj: &mut [u32], env: &mut dyn UpdateEnv) {
    let old = f32::from_bits(obj[ELAPSED]);
    obj[0x3c / 4] = 0;
    if obj[0x34 / 4] != 1 || obj[RUNNING] != 0 {
        return;
    }
    let time = |obj: &[u32]| f32::from_bits(obj[ELAPSED]);
    obj[ELAPSED] = (old + f32::from_bits(0x3c888889)).to_bits();
    let dp = env.director_open(&name_at(obj, SCRIPT_NAME));
    if dp == 0 {
        stop(obj, true, env);
        env.director_close(dp);
        return;
    }
    let i = obj[0x60 / 4] as i32;
    if !(env.step(dp, i).start < time(obj)) {
        env.director_close(dp);
        return;
    }
    let mut advanced = false;
    loop {
        let mut done = false;
        if obj[0x44 / 4] != 0 {
            done = env.wait() != 0;
            if done {
                obj[0x44 / 4] = 0;
            }
        }
        if !done {
            let i = obj[0x60 / 4] as i32;
            let st = env.step(dp, i);
            match st.kind {
                0 => {
                    if advanced || (i == 0 && old == 0.0) {
                        if name_at(obj, CURRENT_NAME) != st.name {
                            camera_cut(obj, env);
                        }
                    }
                    env.cut_to_step(obj, dp, i);
                }
                1 | 28 | 2 | 3 => {
                    let (orbit, flip) = match st.kind {
                        2 => (1, 0),
                        3 => (1, 1),
                        _ => (0, 0),
                    };
                    done = env.blend(obj, dp, i, st.a, orbit, flip, st.start, time(obj), st.end) != 0;
                }
                4 => {
                    if advanced || (i == 0 && old == 0.0) {
                        env.start_path_step(obj, dp, i, st.a, st.b);
                        env.director_close(dp);
                        return;
                    }
                }
                5 => env.set_mode(obj, 4),
                6 => env.set_mode(obj, 5),
                7 => {
                    obj[0x3c / 4] = 1;
                    obj[0x40 / 4] = 1;
                    env.cut_to_current(obj);
                    env.director_close(dp);
                    return;
                }
                8 => obj[0x38 / 4] = 1,
                9 => obj[0x38 / 4] = 0,
                10 => {
                    let r = env.rand() % 100;
                    if (r as f32) < st.c.abs() {
                        set_name(obj, &st.name);
                        update(obj, env);
                        env.director_close(dp);
                        return;
                    }
                }
                11 => {
                    let depth = obj[0x48 / 4] as i32;
                    if depth >= 0x10 {
                        stop(obj, true, env);
                        env.director_close(dp);
                        return;
                    }
                    let at = 0x64 / 4 + depth as usize * 6;
                    for k in 0..6 {
                        obj[at + k] = obj[ELAPSED + k];
                    }
                    obj[0x48 / 4] = (depth + 1) as u32;
                    set_name(obj, &st.name);
                    update(obj, env);
                    env.director_close(dp);
                    return;
                }
                12 => {
                    if obj[0x38 / 4] == 0 {
                        env.cut_to_current(obj);
                    }
                }
                13 => camera_cut(obj, env),
                14 => env.start_scene(obj, dp, i),
                15 => reset_actor_anims(obj, env),
                17 => env.sound_a(),
                18 => env.sound_b(),
                22 => {
                    obj[0x420 / 4] = 1;
                    obj[0x424 / 4] = st.c.to_bits();
                }
                23 => env.set_statics(),
                24 => obj[0xeac / 4] = 1,
                25 => obj[0xeac / 4] = 0,
                27 => {
                    let list = obj[0x4e8 / 4];
                    if list != 0 && env.list_flag(list) == 0 {
                        stop(obj, true, env);
                        env.director_close(dp);
                        return;
                    }
                }
                30 => obj[0xeb0 / 4] = 1,
                31 => {
                    if env.rider_anim_done() == 0 {
                        obj[0x40 / 4] = 0;
                        obj[0x3c / 4] = 1;
                        env.cut_to_current(obj);
                        env.director_close(dp);
                        return;
                    }
                }
                32 => {
                    obj[0x34 / 4] = 0;
                    stop(obj, true, env);
                    env.director_close(dp);
                    return;
                }
                33 => {
                    if env.sound_busy() == 0 {
                        env.sound_set(st.w28);
                    }
                }
                34 => env.audio_stop(),
                37 => {
                    env.run_script(format!("CameraScript_{}", st.w28 as i32).as_bytes());
                }
                38 => env.race_flag(1),
                39 => env.race_flag(0),
                40 => obj[0x44 / 4] = 1,
                41 => env.sound_list(obj[0x4e8 / 4], 0),
                42 => env.sound_list(obj[0x4e8 / 4], 2),
                43 => env.sound_list(obj[0x4e8 / 4], 1),
                _ => {}
            }
        }
        let i = obj[0x60 / 4] as i32;
        if env.step(dp, i).end <= time(obj) {
            done = true;
        }
        if !done {
            env.director_close(dp);
            return;
        }
        if obj[0xc / 4] == 3 {
            obj[0xc / 4] = 0;
        }
        advanced = true;
        if i < env.count(dp) - 1 {
            obj[0x60 / 4] = (i + 1) as u32;
            if env.step(dp, i + 1).start < time(obj) {
                continue;
            }
            env.director_close(dp);
            return;
        }
        // the end of the script: back to the script that started this one, or stop
        loop {
            let depth = obj[0x48 / 4] as i32;
            if depth == 0 {
                stop(obj, false, env);
                env.director_close(dp);
                return;
            }
            let at = ELAPSED + depth as usize * 6;
            for k in 0..6 {
                obj[ELAPSED + k] = obj[at + k];
            }
            obj[0x48 / 4] = (depth - 1) as u32;
            let dp2 = env.director_open(&name_at(obj, SCRIPT_NAME));
            let j = obj[0x60 / 4] as i32;
            obj[0x60 / 4] = (j + 1) as u32;
            if j < env.count(dp2) - 1 {
                update(obj, env);
                env.director_close(dp2);
                env.director_close(dp);
                return;
            }
            env.director_close(dp2);
        }
    }
}

/// What `CamScript_CutTo` calls: the camera class record of a named camera (`CamClass` 0x16dd68 / dtor 0x16ded0:
/// `cls` is the data pointer), its position evaluation 0x16e900 and angle evaluation 0x1722e8 into the object's wanted
/// record (+0x484, with +0xb68 the second record).
pub trait CutEnv: ScriptEnv {
    fn class_open(&mut self, name: &[u8]) -> u32;
    fn class_close(&mut self, cls: u32);
    /// The class record's words at +0x170 and +0x168.
    fn class_word_170(&mut self, cls: u32) -> u32;
    fn class_f168(&mut self, cls: u32) -> f32;
    /// `0x16e900(.., obj + 0x484, obj + 0xb68, flag, 0)`: writes the record's position; returns its result.
    fn eval_pos(&mut self, obj: &mut [u32], flag: u32) -> u32;
    /// `0x1722e8(.., obj + 0x484, obj + 0xb68, flag, obj + 0x10, obj[0xeac], t2, 0.0)`: writes the record's angles.
    fn eval_ang(&mut self, obj: &mut [u32], flag: u32, eac: u32, t2: u32);
}

/// `Camera_GotoNamed` 0x174888: the wanted record for the named camera. An uncut object (+4 clear) first blends the
/// new position with the old by `1 - obj[8]` (or does nothing when +0x1c is set).
pub fn goto_named(obj: &mut [u32], name: &[u8], env: &mut dyn CutEnv) {
    let cls = env.class_open(name);
    if obj[1] != 0 {
        let f = obj[1];
        env.eval_pos(obj, f);
    } else {
        if obj[7] != 0 {
            env.class_close(cls);
            return;
        }
        let old = [f32::from_bits(obj[REC]), f32::from_bits(obj[REC + 1]), f32::from_bits(obj[REC + 2])];
        let r = env.eval_pos(obj, 0);
        obj[0xeb4 / 4] = r;
        let w = 1.0f32 - f32::from_bits(obj[8]);
        for k in 0..3 {
            let new = f32::from_bits(obj[REC + k]);
            obj[REC + k] = (old[k] + (new - old[k]) * w).to_bits();
        }
    }
    let mode = obj[0x428 / 4] as i32;
    let t2 = (mode < 7 && mode >= 2) as u32;
    let (f, eac) = (obj[1], obj[0xeac / 4]);
    env.eval_ang(obj, f, eac, t2);
    env.smooth(obj);
    env.class_close(cls);
}

/// `CamScript_CutTo` 0x174c98(object, name): the name becomes the current camera, the cut counter (+0xc, up to 3)
/// counts a class with +0x170 set (else clears), the camera goes there and the fade level (+0x448) takes the class's.
pub fn cut_to(obj: &mut [u32], name: &[u8], env: &mut dyn CutEnv) {
    copy_name(obj, CURRENT_NAME, name);
    let cls = env.class_open(name);
    if env.class_word_170(cls) == 0 {
        obj[0xc / 4] = 0;
    } else {
        let v = (obj[0xc / 4] as i32).wrapping_add(1);
        obj[0xc / 4] = v as u32;
        if !(v < 4) {
            obj[0xc / 4] = 3;
        }
    }
    goto_named(obj, name, env);
    obj[0x448 / 4] = env.class_f168(cls).to_bits();
    env.cut_fade(obj);
    env.class_close(cls);
}

/// What `IsRiderAnimDone` calls: `Race_GetRider(race, i)` 0x1814a8 (0 when there is none) and the rider's animation
/// control's progress (0x15fe70 on `rider + 0x46c0` with 0).
pub trait RiderEnv {
    fn rider(&mut self, i: u32) -> u32;
    fn anim_progress(&mut self, rider: u32) -> f32;
}

/// `IsRiderAnimDone` 0x16db88 on the rider animation struct (its word 3 the rider's index): done when the rider is
/// gone or its animation has reached 1.0.
pub fn is_rider_anim_done(index: u32, env: &mut dyn RiderEnv) -> u32 {
    let r = env.rider(index);
    if r == 0 {
        return 1;
    }
    (!(env.anim_progress(r) < 1.0)) as u32
}

/// What `RaceScene_UpdateCastClip` calls besides [`RiderEnv`]
pub trait CastClipEnv: RiderEnv {
    /// `Boarder_PlaceAt` 0x11cae8 (the heading, height 0; the rider, the place, a zero velocity, flag 1)
    fn place_at(&mut self, rider: u32, heading: f32, place: [u32; 4]);
    /// 0x169be8 (0x32edc0, the rider, the clip)
    fn play_clip(&mut self, rider: u32, clip: u32);
}

/// `RaceScene_UpdateCastClip` 0x16daa8 on a rider animation struct (+4 its clip list, ending below 0; +8 the clip
/// playing; +0xc the rider's index; +0x14 hold the rider in place; +0x20 the place; +0x30 the heading): the rider
/// (`Race_GetRider`, not checked for none) held at the place when asked, and once its clip has played through
/// (progress 1.0) the next clip of the list, back to the first past its end
pub fn update_cast_clip(m: &mut crate::pf::Mem, env: &mut dyn CastClipEnv, s: u32) {
    let r = env.rider(m.r32(s + 0xc));
    if m.r32(s + 0x14) != 0 {
        let place = [m.r32(s + 0x20), m.r32(s + 0x24), m.r32(s + 0x28), m.r32(s + 0x2c)];
        env.place_at(r, f32::from_bits(m.r32(s + 0x30)), place);
    }
    if 1.0 <= env.anim_progress(r) {
        let i = m.r32(s + 8).wrapping_add(1);
        m.w32(s + 8, i);
        if (m.r32(m.r32(s + 4).wrapping_add(i.wrapping_mul(4))) as i32) < 0 {
            m.w32(s + 8, 0);
        }
        let clip = m.r32(m.r32(s + 4).wrapping_add(m.r32(s + 8).wrapping_mul(4)));
        env.play_clip(r, clip);
    }
}

/// `RaceScene_UpdateCastClips` 0x177020 (the camera script): with its cast on (+0x210), each of its eight rider
/// animation structs (0x40 bytes from +0x220) in use (+0x10) updated by [`update_cast_clip`]
pub fn update_cast_clips(m: &mut crate::pf::Mem, env: &mut dyn CastClipEnv, cam: u32) {
    if m.r32(cam + 0x210) == 0 {
        return;
    }
    for k in 0..8 {
        let s = cam + 0x220 + 0x40 * k;
        if m.r32(s + 0x10) != 0 {
            update_cast_clip(m, env, s);
        }
    }
}

/// One 0x30-byte actor record of a camera scene: +0x10 in use, +0x14 the kind (which rider), +0x18 whether the rider
/// is placed first, +0x1c whether the position comes from the scene, +0x20 the position, +0x2c the heading.
#[derive(Clone, Debug, Default)]
pub struct SceneRec {
    pub in_use: u32,
    pub kind: u32,
    pub place: u32,
    pub from_scene: u32,
    pub pos: [u32; 3],
    pub heading: u32,
}

/// What `0x16d9d0` / `0x16d990` build for an actor: the words copied into its slot (words 1 to 5, 8 to 11 and 12).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActorLocal {
    pub w: [u32; 5],
    pub q: [u32; 4],
    pub f: u32,
}

/// The scene's own position for an actor (`0x172100`): whether it has one, the point and the heading offset.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScenePoint {
    pub ok: bool,
    pub vec: [u32; 4],
    pub out: f32,
}

/// What `CamScript_StartScene` calls.
pub trait SceneEnv: ScriptEnv {
    /// `0x1720c0` (scene made from the step) then `0x172238`: the words it leaves at +0x218 and +0x21c.
    fn scene_open(&mut self, step: u32) -> [u32; 2];
    fn scene_rec(&mut self, ptr: u32, i: usize) -> SceneRec;
    /// `[[race + 0x1c] + 0x88]`, the riders in the race.
    fn rider_count(&mut self) -> i32;
    /// For `Race_GetRider(race, j)`: the rider's word at +0x38, and the word at +0x38 of the record at its +0x464.
    fn rider_slot(&mut self, j: i32) -> u32;
    fn rider_char(&mut self, j: i32) -> u32;
    /// The pointer at `0x336650 + 4 * char`.
    fn actor_name(&mut self, ch: u32) -> u32;
    /// `0x16d9d0(local, rec, name)` (name set) or `0x16d990(local, rec)`, and `0x16dcb8(local, 2)`.
    fn actor_init(&mut self, name: u32, ptr: u32, i: usize) -> ActorLocal;
    fn actor_close(&mut self);
    /// `0x172100(obj + 0x214, i, vec, out)`.
    fn scene_point(&mut self, i: i32) -> ScenePoint;
    /// `Race_GetRider` then `0x11cae8(rider, pos, zero, 1, heading, 0)`.
    fn place_rider(&mut self, j: i32, pos: [u32; 4], zero: [u32; 4], heading: f32);
    /// `0x16da10(slot, j, placed, pos, heading)`.
    fn start_anim(&mut self, slot: usize, j: i32, placed: u32, pos: [u32; 4], heading: f32);
}

/// `0x176720`: the riders by their slot (+0x38): the last with 0, the last with 1, the one before the first and the
/// one after it (-1 for none).
fn find_riders(env: &mut dyn SceneEnv) -> [i32; 4] {
    let mut p = [-1i32; 4];
    let count = env.rider_count();
    if count > 0 {
        for i in 0..count {
            if env.rider_slot(i) == 0 {
                p[0] = i;
            }
            if env.rider_slot(i) == 1 {
                p[1] = i;
            }
        }
    }
    let count = env.rider_count();
    if p[0] > 0 {
        p[2] = p[0] - 1;
    }
    if p[0] < count - 1 {
        p[3] = p[0] + 1;
    }
    p
}

/// The first rider whose character (kind - 14) matches, or -1.
fn find_char(id: u32, env: &mut dyn SceneEnv) -> i32 {
    if env.rider_count() > 0 {
        let mut j = 0;
        loop {
            if env.rider_char(j) == id {
                return j;
            }
            j += 1;
            if j >= env.rider_count() {
                return -1;
            }
        }
    }
    -1
}

/// `CamScript_StartScene` 0x176850(object, step): the actors of a camera scene. The old actors are cleared and the
/// scene's records read from the step; each record names a rider (the first or second slot, its neighbours, a
/// character, or the next one no record named) and that rider's actor slot (0x40 bytes from +0x220) is built, the
/// rider placed first when the record says so, and its animation started at the record's position and heading.
pub fn start_scene(obj: &mut [u32], step: u32, env: &mut dyn SceneEnv) {
    reset_actor_anims(obj, env);
    let old = obj[0x214 / 4];
    obj[0x210 / 4] = 1;
    let [w1, w2] = env.scene_open(step);
    obj[0x218 / 4] = w1;
    obj[0x21c / 4] = w2;
    obj[0x214 / 4] = old;
    let p = find_riders(env);
    // the stack's two tables of eight: the riders no record named (list `a`, then marks `b`); a mark at -1 lands on
    // the last of `a`
    let mut t = [-1i32; 16];
    for k in 8..16 {
        t[k] = 0;
    }
    let ptr = w2;
    for i in 0..8 {
        let rec = env.scene_rec(ptr, i);
        if rec.in_use == 0 {
            continue;
        }
        let ix = match rec.kind {
            0 => p[0],
            10 => p[1],
            11 => p[2],
            12 => p[3],
            14..=25 => find_char(rec.kind - 14, env),
            _ => -1,
        };
        if ix != -1 || rec.kind == 0 {
            t[(8 + ix) as usize] = 1;
        }
    }
    let count = env.rider_count();
    if count > 0 {
        let mut cur = 0;
        for k in 0..count {
            if t[8 + k as usize] == 0 {
                t[cur] = k;
                cur += 1;
            }
        }
    }
    let (mut cursor, mut n) = (0usize, 1usize);
    for i in 0..8 {
        let rec = env.scene_rec(ptr, i);
        if rec.in_use == 0 {
            obj[(0x230 + i * 0x40) / 4] = 0;
            continue;
        }
        let s5 = match rec.kind {
            0 => p[0],
            10 => p[1],
            11 => p[2],
            12 => p[3],
            13 => {
                let v = t[cursor / 4];
                if v != -1 {
                    cursor = n * 4;
                    n += 1;
                }
                v
            }
            14..=25 => find_char(rec.kind - 14, env),
            _ => -1,
        };
        if s5 == -1 {
            obj[(0x230 + i * 0x40) / 4] = 0;
            continue;
        }
        let ch = env.rider_char(s5);
        let name = env.actor_name(ch);
        let local = env.actor_init(name, ptr, i);
        let slot = (0x220 + i * 0x40) / 4;
        obj[slot + 1..slot + 6].copy_from_slice(&local.w);
        obj[slot + 8..slot + 12].copy_from_slice(&local.q);
        obj[slot + 12] = local.f;
        env.actor_close();
        let rec = env.scene_rec(ptr, i);
        let mut heading = f32::from_bits(rec.heading);
        let own = [rec.pos[0], rec.pos[1], rec.pos[2], 1.0f32.to_bits()];
        let mut vec = own;
        if rec.from_scene != 0 {
            let sp = env.scene_point(i as i32);
            if sp.ok {
                vec = sp.vec;
            }
            heading = crate::math::angle_diff(heading + sp.out, 0.0);
        }
        let rec = env.scene_rec(ptr, i);
        if rec.place != 0 {
            env.place_rider(s5, vec, [0; 4], heading);
        }
        env.start_anim(i, s5, (rec.place != 0) as u32, vec, heading);
    }
}

//! The pre-race sequence (board row G11c): `cPreRaceHandler` (Enter 0x112b88, Update 0x112d10, Exit 0x113468)
//! runs the race's three intro scenes in turn (each: select, prepare, play, wait or skip), then the countdown's
//! screens, then places the riders at the start. The scene object is `scene::RaceScene` (0x32edc0); the scenes'
//! scripts are words of the career block (the race's +0x33c, career +0x2b18, 100 apart; 0x39 "Null" skips one).
//! What the engine does for it (cameras, HUD, sound, riders) is [`PreRaceEnv`].
//! Checked in the function runner (`tests/prerace.rs`).

use crate::career::Career;
use crate::scene::{RaceScene, SceneEnv, Selection, SCENES};

/// The camera script the handler starts with ("PreLoad", 0x364620)
pub const PRELOAD: &str = "PreLoad";
/// The world script run on courses without a start gate ("NoCountdown", 0x364628)
pub const NO_COUNTDOWN: &str = "NoCountdown";
/// The script number that means no scene ("Null")
pub const NULL_SCENE: i32 = 0x39;

/// The race object as the handler reads it: +0x88 riders, +0x7c players, +0x40 → +0x4c (one pad for the race),
/// +0xdc and +0xe0 (how the race was entered)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaceView {
    pub riders: i32,
    pub players: i32,
    pub one_pad: bool,
    pub restarted: u32,
    pub replayed: u32,
}

/// The engine's calls
pub trait PreRaceEnv: SceneEnv {
    /// `Race_UpdateRiderStreaming`
    fn update_streaming(&mut self);
    /// The race is still loading (+0x4f8)
    fn loading(&mut self) -> bool;
    /// `CamScript_Stop`(the race's camera, 1)
    fn cam_stop(&mut self);
    /// `CamScript_Start`(the race's camera, script, 1)
    fn cam_start(&mut self, script: &str);
    /// `CamScript_IsRunning`
    fn cam_running(&mut self) -> bool;
    /// `FUN_0022e400`(`DAT_0034370c`, a, b)
    fn speech_mode(&mut self, a: u32, b: u32);
    /// `Speech_InitCharScriptSamples`
    fn init_char_samples(&mut self);
    /// `Speech_CheckBigScore`(the speech system, v) as `RaceScene_Prepare` calls it with each cast member
    fn cast_sample(&mut self, v: i32);
    /// `SpeechSys_StopCharScript`
    fn stop_char_script(&mut self);
    /// `Speech_FreeCharScriptSamples`
    fn free_char_samples(&mut self);
    /// `FE3DTransition_StartIn` 0x1ff2b0(1.0, gApp+0x72c)
    fn fade_in(&mut self);
    /// `FE3DTransition_StartOut` 0x1ff4c8(1.0, gApp+0x72c)
    fn fade_out(&mut self);
    /// The rider's virtual +0x4c (before the third scene)
    fn rider_reset(&mut self, i: usize);
    /// `Boarder_PlaceAtStart`(rider, gate)
    fn place_at_start(&mut self, i: usize, gate: bool);
    /// `HUD_ShowItem` / `HUD_HideItem`(the race's HUD, item, −1)
    fn hud_show(&mut self, item: u32);
    fn hud_hide(&mut self, item: u32);
    /// `Audio_StopIntroStream`
    fn stop_intro_stream(&mut self);
    /// The race's virtual +0x9c (the start) and +0x6c (on enter)
    fn race_start(&mut self);
    fn race_entered(&mut self);
    /// The player of the race's k-th player rider (its character entry +0x65)
    fn player_of(&mut self, k: usize) -> i8;
    /// `Overlay_HideAll`, `Game_UseSingleView`, `Game_RestoreViews`
    fn overlays_hidden(&mut self);
    fn single_view(&mut self);
    fn restore_views(&mut self);
    /// `World_RunScript`
    fn run_world_script(&mut self, name: &str);
    /// The input devices (gApp+0x24 → +0x14), `Input_GetDevicePressed`, and a device's latched (+0xc) and held
    /// (+0x10) buttons as `RaceScene_CheckSkip` takes them (it zeroes +0x2c), and the latch it writes back
    fn device_count(&mut self) -> i32;
    fn device_pressed(&mut self, i: i32) -> bool;
    fn take_device(&mut self, i: i32) -> (u32, u32);
    fn latch_device(&mut self, i: i32, latched: u32);
    /// gApp+0x58
    fn app_mode(&mut self) -> u32;
}

/// The scenes' words in the career block
pub fn selection(c: &Career) -> Selection {
    let mut s = [[0i32; 25]; SCENES];
    for (i, scene) in s.iter_mut().enumerate() {
        for (k, w) in scene.iter_mut().enumerate() {
            *w = c.r32(0x2b18 + 100 * i + 4 * k) as i32;
        }
    }
    s
}

/// `Race_HasStartGate` 0x112848: not on Untracked (7); the race modes 0, 2, 3, 4, 5, 7, 9
pub fn has_start_gate(c: &Career) -> bool {
    if c.r32(0x2af8) == 7 {
        return false;
    }
    matches!(c.r32(0x2afc), 0 | 2 | 3 | 4 | 5 | 7 | 9)
}

/// `Race_IsPlayerPad` 0x110f28(pad): with one pad for the race, the career's pad (+0x325c); else a race rider's
/// player (+0x69)
pub fn is_player_pad(c: &Career, race: &RaceView, pad: i32) -> bool {
    if race.one_pad {
        return pad == c.r32(0x325c) as i32;
    }
    (0..race.riders.max(0) as usize).any(|i| c.r8(0x2845 + 0x84 * i) as i8 as i32 == pad)
}

/// `Race_ResetRiderRenders` 0x1128b8 is `Boarder_ResetRender` on each rider (the engine's)

/// `Race_AnyRiderLevelAbove7` 0x112af0: a player rider whose player's level is over 7
pub fn any_level_above_7(env: &mut dyn PreRaceEnv, c: &Career, race: &RaceView) -> bool {
    (0..race.players.max(0) as usize).any(|k| {
        let p = env.player_of(k);
        7 < c.player_level(p)
    })
}

/// `RaceScene_StopSpeech` 0x110980
pub fn stop_speech(env: &mut dyn PreRaceEnv) {
    env.stop_char_script();
    env.free_char_samples();
}

/// `RaceScene_Prepare` 0x1109b0: with a cast, the samples set up and each member passed on; always false
pub fn prepare(env: &mut dyn PreRaceEnv, scene: &RaceScene) -> bool {
    let n = scene.cast_count();
    if n != 0 {
        env.init_char_samples();
        for k in 0..n {
            env.cast_sample(scene.cast[k]);
        }
    }
    false
}

/// `RaceScene_Play` 0x110a40: the scene's script on the camera, faded in
pub fn play(env: &mut dyn PreRaceEnv, scene: &RaceScene, sel: &Selection) {
    // (a script number past the table reads past it in the game)
    let name = scene.script_name(sel).unwrap_or("");
    env.cam_stop();
    env.cam_start(name);
    env.fade_in();
}

/// `RaceScene_CheckSkip` 0x110ab8: the first player pad pressed this frame; its new buttons among X / triangle
/// (0x880; 0xc0 when gApp+0x58 is 4) skip, latching them; a skip stops the speech and the camera
pub fn check_skip(env: &mut dyn PreRaceEnv, c: &Career, race: &RaceView) -> bool {
    let mut skip = false;
    let mut i = 0;
    while i < env.device_count() {
        if is_player_pad(c, race, i) && env.device_pressed(i) {
            let (latched, held) = env.take_device(i);
            let mask = if env.app_mode() == 4 { 0xc0 } else { 0x880 };
            if held & !latched & mask != 0 {
                skip = true;
                env.latch_device(i, latched | (held & mask));
            }
            break;
        }
        i += 1;
    }
    if skip {
        stop_speech(env);
        env.cam_stop();
    }
    skip
}

/// `cPreRaceHandler` (+4 its state)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreRace {
    pub state: u32,
}

impl PreRace {
    /// `cPreRaceHandler::Enter` 0x112b88: overlays hidden, one view; a race entered again (+0xdc) goes straight to
    /// the countdown's end (0x12); a replayed one (+0xe0) to the start (0); a first one to the first scene (2),
    /// counting the course started for each profile's character; the PreLoad camera; NoCountdown without a gate
    pub fn enter(&mut self, env: &mut dyn PreRaceEnv, c: &mut Career, race: &RaceView) {
        env.overlays_hidden();
        env.single_view();
        if race.restarted == 0 {
            if race.replayed == 0 {
                self.state = 2;
                let mut i = 0;
                while i < c.count_profiles() as usize {
                    let ch = c.r32(0x30fc + 0x44 * i + 0x1c) as usize;
                    let course = c.r32(0x2af8) as usize;
                    c.count_course_start(ch, course);
                    i += 1;
                }
                env.speech_mode(0, 0);
            } else {
                self.state = 0;
                env.speech_mode(0, 0);
                env.speech_mode(1, 1);
            }
        } else {
            self.state = 0x12;
        }
        env.cam_stop();
        env.cam_start(PRELOAD);
        if !has_start_gate(c) {
            env.run_world_script(NO_COUNTDOWN);
        }
        env.race_entered();
    }

    /// One scene's select and prepare: `Null` skips to `skip`; else `ready` (prepare said yes, never) or `play`
    fn scene_step(&mut self, env: &mut dyn PreRaceEnv, c: &Career, scene: &mut RaceScene, k: usize, play: u32, skip: u32) {
        if c.r32(0x2b18 + 100 * k) as i32 == NULL_SCENE {
            self.state = skip;
        } else {
            scene.select(env, k as i32);
            self.state = if prepare(env, scene) { play - 1 } else { play };
        }
    }

    /// `cPreRaceHandler::Update` 0x112d10: one frame of the sequence (states 0..0x15, each falling through to the
    /// next when it can)
    pub fn update(&mut self, env: &mut dyn PreRaceEnv, c: &Career, race: &RaceView, scene: &mut RaceScene) {
        env.update_streaming();
        let sel = selection(c);
        if self.state == 0 {
            env.cam_stop();
            env.cam_start(PRELOAD);
            self.state = 1;
        }
        if self.state == 1 && !env.loading() {
            self.state = 2;
        }
        // the first scene
        if self.state == 2 {
            self.scene_step(env, c, scene, 0, 4, 6);
        }
        if self.state == 3 && !env.loading() {
            self.state = 4;
        }
        if self.state == 4 {
            env.speech_mode(1, 1);
            play(env, scene, &sel);
            self.state = 5;
        }
        if self.state == 5 && (!env.cam_running() || check_skip(env, c, race)) {
            self.state = 6;
        }
        // the second
        if self.state == 6 {
            env.speech_mode(1, 1);
            self.scene_step(env, c, scene, 1, 8, 10);
        }
        if self.state == 7 && !env.loading() {
            self.state = 8;
        }
        if self.state == 8 {
            play(env, scene, &sel);
            self.state = 9;
        }
        if self.state == 9 {
            if !env.cam_running() {
                self.state = 10;
            } else if check_skip(env, c, race) {
                // (a skip here skips the third scene too)
                self.state = 0xe;
            }
        }
        // the third, the riders reset first
        if self.state == 10 {
            if c.r32(0x2b18 + 200) as i32 == NULL_SCENE {
                self.state = 0xe;
            } else {
                for i in 0..race.riders.max(0) as usize {
                    env.rider_reset(i);
                }
                scene.select(env, 2);
                self.state = if prepare(env, scene) { 0xb } else { 0xc };
            }
        }
        if self.state == 0xb && !env.loading() {
            self.state = 0xc;
        }
        if self.state == 0xc {
            play(env, scene, &sel);
            self.state = 0xd;
        }
        if self.state == 0xd && (!env.cam_running() || check_skip(env, c, race)) {
            self.state = 0xe;
        }
        // the riders to the start, then the countdown's screens
        if self.state == 0xe {
            env.fade_out();
            let gate = has_start_gate(c);
            for i in 0..race.riders.max(0) as usize {
                env.place_at_start(i, gate);
            }
            self.state = if env.loading() { 0xf } else { self.countdown(env, c, race) };
        }
        if self.state == 0xf && !env.loading() {
            self.state = self.countdown(env, c, race);
        }
        if self.state == 0x10 && check_skip(env, c, race) {
            env.hud_hide(0x12);
            if c.r32(0x2afc) == 1 {
                self.state = 0x12;
            } else {
                env.hud_show(3);
                self.state = 0x11;
            }
        }
        if self.state == 0x11 && check_skip(env, c, race) {
            env.hud_hide(0x12);
            env.hud_hide(3);
            self.state = 0x12;
        }
        if self.state == 0x12 {
            env.stop_intro_stream();
            env.speech_mode(1, 0);
            self.state = if env.loading() { 0x13 } else { 0x14 };
        }
        if self.state == 0x13 && !env.loading() {
            self.state = 0x14;
        }
        if self.state == 0x14 {
            env.cam_stop();
            stop_speech(env);
            let gate = has_start_gate(c);
            for i in 0..race.riders.max(0) as usize {
                env.place_at_start(i, gate);
            }
            env.race_start();
            self.state = 0x15;
        }
    }

    /// The countdown's first screen: item 0x12 when a player is over level 7 (0x10), else item 3 (0x11), none in
    /// mode 1 (0x12)
    fn countdown(&self, env: &mut dyn PreRaceEnv, c: &Career, race: &RaceView) -> u32 {
        if any_level_above_7(env, c, race) {
            env.hud_show(0x12);
            0x10
        } else if c.r32(0x2afc) == 1 {
            0x12
        } else {
            env.hud_show(3);
            0x11
        }
    }

    /// `cPreRaceHandler::Exit` 0x113468
    pub fn exit(&mut self, env: &mut dyn PreRaceEnv) {
        env.restore_views();
    }
}

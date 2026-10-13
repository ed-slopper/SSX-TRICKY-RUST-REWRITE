//! The boarder's control states' updates exactly as the original runs them (board row G1f): cruising (with the look
//! back), changing to fakie, bumps, hard and normal landings, natural air, sitting at the start and getting up. Each
//! reads the pad word the way `cPlayer::GetControls` packs it and calls the boarder's helpers (already ported and
//! checked: steer, crouch, brake, boost, lean, reset) through [`ControlEnv`]. Checked bit for bit in the function
//! runner (`tests/control.rs`).

use crate::ps2f::F;
use crate::rigidbody::{add, dot, ld, scale, st};

type V4 = [f32; 4];

/// The rider fields the updates read and write
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    /// velocity (+0x150), the board's forward (+0x180)
    pub vel: V4,
    pub board: V4,
    /// +0x1f0, +0x1f4, +0x1f8 (the air's crouch-out and its rates)
    pub x1f0: f32,
    pub x1f4: f32,
    pub x1f8: f32,
    /// brake now, rate, target (+0x1fc..+0x204); crouch now, rate, target (+0x208..+0x210); steer now, rate,
    /// target (+0x214..+0x21c)
    pub brake: f32,
    pub brake_rate: f32,
    pub brake_target: f32,
    pub crouch: f32,
    pub crouch_rate: f32,
    pub crouch_target: f32,
    pub steer: f32,
    pub steer_rate: f32,
    pub steer_target: f32,
    /// two more now/rate/target triples (+0x220..+0x228, +0x22c..+0x234) and the lean's (+0x268..+0x270)
    pub x220: f32,
    pub x224: f32,
    pub x228: f32,
    pub x22c: f32,
    pub x230: f32,
    pub x234: f32,
    pub lean: f32,
    pub lean_rate: f32,
    pub lean_target: f32,
    /// the surface (+0x290: 0 off the course), the reset flag (+0x304), the controller (+0x418: below 0 a player),
    /// the board class (+0x420), the air time (+0x57f4)
    pub surface: u32,
    pub reset_flag: u32,
    pub controller: i32,
    pub class: u32,
    pub air: f32,
    /// the race index (+0x38), the slot (+0x460), the heading (+0x1b0), the stance (+0x1b4)
    pub index: i32,
    pub slot: i32,
    pub heading: f32,
    pub stance: u32,
    /// per other rider (+0x40 + 0x20 · k: its distance, +0x44: its bearing)
    pub dist: [f32; 8],
    pub bearing: [f32; 8],
    /// the trick slot (+0x44d8) and the word of each (+0xb50 + 0x394 · slot); the clip filter (+0x46dc)
    pub trick: i32,
    pub trick_words: [i32; 8],
    pub filter: u32,
    /// the control state (+0x428), the stance's copy (+0x46d4) and the facing (+0x46e0), goofy (the character
    /// entry +0x464 → +0x40)
    pub control: u32,
    pub x46d4: u32,
    pub facing: f32,
    pub goofy: bool,
    /// the board object (+0x5ae0): +0x718, +0x76c, +0x770, +0x798
    pub b718: u32,
    pub b76c: u32,
    pub b770: u32,
    pub b798: u32,
}

/// The game's calls
pub trait ControlEnv {
    /// `AnimCtl_GetClip`, `AnimCtl_GetClipType`, `AnimCtl_IsClipAtEnd`, `AnimCtl_GetLayerPhase` (the rider's
    /// animation +0x46c0, a layer)
    fn clip(&mut self, layer: u32) -> i32;
    fn clip_type(&mut self, layer: u32) -> i32;
    fn clip_at_end(&mut self, layer: u32) -> bool;
    fn layer_phase(&mut self, layer: u32) -> f32;
    /// `AnimCtl_PlayClip`(clip, 0, 5) (the clip filter +0x46dc as it is then)
    fn play_clip(&mut self, r: &mut Rider, clip: i32);
    /// `Boarder_SetState`, `Boarder_SetMotion`
    fn set_state(&mut self, r: &mut Rider, s: u32);
    fn set_motion(&mut self, r: &mut Rider, m: u32);
    /// `Boarder_SetSteerTarget`, `Boarder_SetCrouchTarget`, `Boarder_SetBrakeCrouchInput`(brake, crouch),
    /// `Boarder_SetBrakeTarget`, `Boarder_UpdateBoost`(the two buttons), `Boarder_UpdateLeanAnim`(two −1/0/1),
    /// `Boarder_RotateAboutAxis`(angle, about the board's up), `Boarder_RequestReset`(101.46)
    fn set_steer_target(&mut self, r: &mut Rider, v: f32);
    fn set_crouch_target(&mut self, r: &mut Rider, v: f32);
    fn set_brake_crouch_input(&mut self, r: &mut Rider, brake: f32, crouch: f32);
    fn set_brake_target(&mut self, r: &mut Rider, v: f32);
    fn update_boost(&mut self, r: &mut Rider, a: u32, b: u32);
    fn update_lean_anim(&mut self, r: &mut Rider, a: f32, b: f32);
    fn rotate_about_axis(&mut self, r: &mut Rider, angle: f32);
    fn request_reset(&mut self, r: &mut Rider);
    /// `Boarder_PlayCelebrate` 0x128150, `Boarder_PlayDisappointed` 0x127e80, `Boarder_PlayWindUpMasked` 0x127d08,
    /// `Boarder_PlayTakeoffClip` 0x127298, `Ride_AITauntCheck` 0x103660, `CruiseControl_RandomFlourish` 0x103b88
    fn play_celebrate(&mut self, r: &mut Rider);
    fn play_disappointed(&mut self, r: &mut Rider);
    fn play_wind_up(&mut self, r: &mut Rider);
    fn play_takeoff(&mut self, r: &mut Rider);
    fn ai_taunt_check(&mut self, r: &mut Rider);
    fn random_flourish(&mut self, r: &mut Rider);
    /// The rider's virtual +0x7c(1)
    fn stop(&mut self, r: &mut Rider);
    /// The race's riders (gApp+0x730 → +0x1c → +0x88) and its state (→ +0x1c)
    fn race_riders(&mut self) -> i32;
    fn race_state(&mut self) -> i32;
    /// `CamScript_Start`(the rider's camera (race index · 0x80 + gApp+0x730 + 0xa0), script, 0)
    fn cam_start(&mut self, index: i32, script: &str);
    /// The control state's own update (its object in the boarder handler: `BumpControl_Update`, `Cruise…`,
    /// `FinishState_Update`, the gate's, `JumpControl_Update`, the landings, the air, `PrewindState_Update`,
    /// `RailSlideControl_Update`, `SpinState_Update`, the sit, `GetUpState_Update`, `ResetState_Update`)
    fn run_control(&mut self, r: &mut Rider, control: u32);
}

/// A stick field of the pad word: six signed bits at `shl` from the top, over 31
fn stick(pad: u32, shl: u32) -> f32 {
    ((pad << shl) as i32 >> 26) as f32 / 31.0
}

/// A direction field: two signed bits, −1 / 0 / 1
fn dir(pad: u32, shl: u32) -> f32 {
    match (pad << shl) as i32 >> 30 {
        -1 => -1.0,
        1 => 1.0,
        _ => 0.0,
    }
}

/// The end of most updates: off the course or with the reset button (bit 0), the reset flag is the button's absence
/// and a reset is asked for
fn reset_check(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
    if pad & 1 == 0 && r.surface != 0 {
        return;
    }
    r.reset_flag = (pad ^ 1) & 1;
    env.request_reset(r);
}

/// The velocity along the board (`vmul`, the VU sum)
fn along(r: &Rider) -> f32 {
    dot(ld(r.vel), ld(r.board)).0
}

/// `cCruiseControl` (+8 the look-back timer, +0x10 the brake held)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cruise {
    pub look: f32,
    pub braking: u32,
}

impl Cruise {
    /// `CruiseControl_Update` 0x102fc8
    pub fn update(&mut self, env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
        let clip = env.clip(0);
        if 0 <= r.controller && clip != 0x221 {
            env.set_state(r, 6);
            return;
        }
        env.update_boost(r, (pad as i32 >> 3 & 1) as u32, (pad as i32 >> 4 & 1) as u32);
        let c = stick(pad, 0xf);
        let mut crouch = c;
        if r.class == 2 && env.clip_type(1) != 8 {
            crouch = if c < 0.5 { c } else { 0.5 };
        }
        env.set_steer_target(r, stick(pad, 0x15));
        env.set_brake_crouch_input(r, crouch, stick(pad, 9));
        if self.braking == 0 {
            if pad & 2 != 0 {
                self.braking = 1;
            }
        } else {
            self.braking = (pad & 6 != 0) as u32;
        }
        let held = r.b718;
        if held != 0 && self.braking == 0 {
            r.b718 = 0;
        }
        if clip == 0x221 {
            r.lean_target = 0.0;
            r.lean_rate = 2.0;
        } else {
            r.lean_target = r.steer;
            r.lean_rate = 2.0;
            if held == 0 && self.braking != 0 {
                env.set_brake_target(r, 0.0);
                if r.brake == 0.0 {
                    env.set_state(r, 0xe);
                    env.stop(r);
                    return;
                }
            } else if 0.0 < r.crouch && r.steer.abs() < 0.2 && r.brake == 0.0 {
                let v = ld(r.vel);
                if dot(v, v).sqrt().0 < 833.3333 {
                    env.play_clip(r, 0x221);
                }
            }
        }
        env.update_lean_anim(r, dir(pad, 7), dir(pad, 5));
        env.play_celebrate(r);
        env.play_disappointed(r);
        env.ai_taunt_check(r);
        self.look_back(env, r);
        env.random_flourish(r);
        reset_check(env, pad, r);
    }

    /// `CruiseControl_LookBack` 0x1033d0: while the layer-1 clip is a look (type 8), after a second, the first
    /// other rider close behind (under 1477.63 and more than 120° off the heading) turns the head (clip 0x210 on the
    /// side of the stance, else 0x20f) with the trick's filter
    pub fn look_back(&mut self, env: &mut dyn ControlEnv, r: &mut Rider) {
        if env.clip_type(1) != 8 {
            self.look = 0.0;
            return;
        }
        self.look = (F(self.look) + F(0.016666668)).0;
        if self.look < 1.0 {
            return;
        }
        let n = env.race_riders();
        let mut k = 0;
        while k < n {
            if k != r.slot {
                let ku = k as usize;
                let a = F(r.bearing[ku]) - F(r.heading);
                let f = a * F(0.15915494) + F(0.5);
                let mut fl = F(f.0 as i32 as f32);
                if f.0 < fl.0 {
                    fl = fl - F(1.0);
                }
                let a = (a - fl * F(6.2831855)).0;
                if r.dist[ku] < 1477.6294 && 2.0943952 < a.abs() {
                    let bit = |add: i32| if r.trick < 0 { 0 } else { 1u32 << ((r.trick_words[r.trick as usize] + add) as u32 & 31) };
                    r.filter = bit(3) | bit(5) | bit(9) | bit(4);
                    let clip = if (a < 0.0) as u32 == r.stance { 0x210 } else { 0x20f };
                    env.play_clip(r, clip);
                    r.filter = u32::MAX;
                    return;
                }
            }
            k += 1;
        }
    }
}

/// The ends of a fakie change, a hard landing and a bump: the landing clip's next state (0x1ff: cruise 3, or 0xe
/// with the brake button; 0x202: 5)
fn after_clip(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider, clip: i32) {
    if clip == 0x202 {
        env.set_state(r, 5);
    } else if clip == 0x1ff || clip == 0x221 {
        env.set_state(r, if pad & 4 == 0 { 3 } else { 0xe });
    }
}

/// `ChangeFakieControl_Update` 0x104030
pub fn change_fakie(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
    let clip = env.clip(0);
    after_clip(env, pad, r, clip);
    env.set_steer_target(r, stick(pad, 0x15));
    env.set_crouch_target(r, stick(pad, 0xf));
    reset_check(env, pad, r);
}

/// `cBumpControl` (+8 the turn's size, +0xc the turn so far)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bump {
    pub turn: f32,
    pub done: f32,
}

impl Bump {
    /// `BumpControl_Update` 0x1041b0: while the bump clip plays (type 1) the rider turns with its phase
    pub fn update(&mut self, env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
        env.set_steer_target(r, stick(pad, 0x15));
        env.set_brake_crouch_input(r, 0.0, stick(pad, 9));
        env.update_lean_anim(r, dir(pad, 7), dir(pad, 5));
        if env.clip_type(0) == 1 {
            let phase = env.layer_phase(0);
            let now = (F(self.turn) * F(phase)).0;
            env.rotate_about_axis(r, (F(self.done) - F(now)).0);
            self.done = now;
        } else {
            env.set_state(r, if pad & 4 != 0 { 0xe } else { 3 });
        }
        reset_check(env, pad, r);
    }
}

/// `LandHardControl_Update` 0x104800: the clip's next state; the steer turned round when going backward
pub fn land_hard(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
    let clip = env.clip(0);
    if clip == 0x1ff {
        env.set_state(r, if pad & 4 == 0 { 3 } else { 0xe });
    } else if clip == 0x202 {
        env.set_state(r, 5);
    }
    let s = stick(pad, 0x15);
    env.set_steer_target(r, if along(r) < 0.0 { -s } else { s });
    env.set_brake_crouch_input(r, 0.0, stick(pad, 9));
    reset_check(env, pad, r);
}

/// `cLandNormalControl` (+8 the crouch the landing set)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LandNormal {
    pub crouch: f32,
}

impl LandNormal {
    /// `LandNormalControl_Update` 0x104e68: cruise (or 0xe) once crouched back to the landing's; steering unless
    /// braking (turned round going backward); not braking, the crouch target at least the landing's (rate 8.33)
    pub fn update(&mut self, env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
        if self.crouch <= r.crouch {
            env.set_state(r, if pad & 4 == 0 { 3 } else { 0xe });
        }
        if pad & 4 == 0 {
            let s = stick(pad, 0x15);
            env.set_steer_target(r, if along(r) < 0.0 { -s } else { s });
        }
        if r.brake == 0.0 {
            let c = stick(pad, 0xf);
            r.crouch_target = if self.crouch <= c { c } else { self.crouch };
            r.crouch_rate = f32::from_bits(0x41055556);
        }
        reset_check(env, pad, r);
    }
}

/// `cNaturalAirControl` (+8 the steer the air allows)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NaturalAir {
    pub steer: f32,
}

impl NaturalAir {
    /// `NaturalAirControl_Update` 0x1000e0
    pub fn update(&mut self, env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
        let clip = env.clip(0);
        if clip == 0x224 {
            r.crouch_target = 1.0;
            r.crouch = 1.0;
            r.crouch_rate = 0.0;
        } else {
            env.set_crouch_target(r, stick(pad, 0x18));
        }
        env.update_lean_anim(r, dir(pad, 4), dir(pad, 2));
        env.play_wind_up(r);
        if clip == 0x224 {
            r.x1f8 = 0.0;
            r.x1f0 = 0.0;
            r.x1f4 = 0.0;
        } else if r.brake != 0.0 || r.steer != 0.0 || r.lean != 0.0 || r.x220 != 0.0 || r.x22c != 0.0 {
            // still settling: everything eased back to rest
            if pad & 1 == 0 {
                if r.steer_target == 0.0 {
                    r.steer_target = 0.0;
                    r.steer_rate = 4.0;
                    r.lean_rate = 4.0;
                    r.lean_target = 0.0;
                    if r.x220 != 0.0 && stick(pad, 0xc) == 0.0 {
                        r.x224 = 4.0;
                        r.x228 = 0.0;
                    }
                    if r.x22c == 0.0 || stick(pad, 6) != 0.0 {
                        return Self::end(env, pad, r);
                    }
                    r.x234 = 0.0;
                    r.x230 = 4.0;
                } else {
                    env.set_steer_target(r, stick(pad, 0x12));
                    if self.steer.abs() <= r.steer.abs() {
                        r.steer = self.steer;
                    } else {
                        r.steer_target = 0.0;
                        r.steer_rate = 4.0;
                    }
                    if clip == 0x1ff {
                        r.lean_target = r.steer;
                        r.lean_rate = 2.0;
                    } else {
                        r.lean_target = 0.0;
                        r.lean_rate = 2.0;
                    }
                }
            } else {
                r.steer_target = 0.0;
                r.steer_rate = 4.0;
                r.lean_rate = 4.0;
                r.lean_target = 0.0;
                r.x224 = 4.0;
                r.x228 = 0.0;
                r.x230 = 4.0;
                r.x234 = 0.0;
            }
        } else {
            // at rest: unless crouched with no steer and no reset, into the jump's wind-up (state 0x10)
            if stick(pad, 0x12) == 0.0 && 0.5 <= r.crouch && pad & 1 == 0 {
                return Self::end(env, pad, r);
            }
            let out = (F(1.0) - F(r.crouch)).0;
            r.x1f8 = out;
            r.x1f0 = out;
            r.x1f4 = 0.0;
            if 0.35 < r.air {
                env.play_takeoff(r);
            }
            r.b798 = 0;
            r.b76c = 0;
            r.b770 = 0;
            env.set_state(r, 0x10);
        }
        Self::end(env, pad, r);
    }

    /// The air's reset: the second button (bit 1)
    fn end(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
        if pad & 2 != 0 {
            env.request_reset(r);
        }
    }
}

/// The camera script a sitting rider's camera starts at the race's start ("RaceStarted2", 0x360620)
pub const RACE_STARTED2: &str = "RaceStarted2";

/// `SitAndWaitControl_Update` 0x105bc0: when the race state is 4, the rider's camera (when it has a race index)
/// starts RaceStarted2, the get-up clip (0x1fe), motion 6, state 0x12
pub fn sit_and_wait(env: &mut dyn ControlEnv, r: &mut Rider) {
    if env.race_state() != 4 {
        return;
    }
    if 0 <= r.index {
        env.cam_start(r.index, RACE_STARTED2);
    }
    env.play_clip(r, 0x1fe);
    env.set_motion(r, 6);
    env.set_state(r, 0x12);
}

/// `GetupFromSitControl_Update` 0x105c60: still getting up (a clip not at its end), held still; at its end (or on
/// 0x1ff) clip 0x221, then cruise (3) on the ground (2) with a push of 416.67 along the board; the brake and crouch
/// cleared
pub fn getup_from_sit(env: &mut dyn ControlEnv, r: &mut Rider) {
    let clip = env.clip(0);
    let go = if clip == 0x221 {
        true
    } else if clip == 0x1ff || env.clip_at_end(0) {
        env.play_clip(r, 0x221);
        true
    } else {
        r.vel = [0.0; 4];
        false
    };
    if go {
        env.set_state(r, 3);
        env.set_motion(r, 2);
        let push = scale(ld(r.board), F(f32::from_bits(0x43d05555)));
        r.vel = st(add(ld(r.vel), push));
    }
    r.crouch_target = 0.0;
    r.crouch = 0.0;
    r.crouch_rate = 0.0;
    r.brake_target = 0.0;
    r.brake = 0.0;
    r.brake_rate = 0.0;
}

/// `Boarder_UpdateControl` 0x11c9a0: the control state's update by +0x428 (each its own state object in the boarder
/// handler: [`ControlEnv::run_control`]); 5 (the fakie cruise) and 0x13 (only the reset check) run here
pub fn update_control(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
    match r.control {
        5 => fakie_cruise(env, pad, r),
        0x13 => reset_check(env, pad, r),
        2..=4 | 6..=0x12 | 0x14 | 0x16 => env.run_control(r, r.control),
        _ => {}
    }
}

/// The fakie cruise (state 5, inline in `Boarder_UpdateControl`): on the slide-out clips (0x203 / 0x204) the velocity
/// slows by 1%; on the turn clip (0x202) the stick (full for a computer rider; turned for a goofy rider) picks the
/// way round unless the board already faces its way: the stance flips (+0x1b4, +0x46d4, the facing +0x46e0 as π or 0
/// wrapped), then clip 0x203 (right), 0x204 (left) or, near centre, a half turn and 0x201; the landing clips go to
/// cruise; the reset check
pub fn fakie_cruise(env: &mut dyn ControlEnv, pad: u32, r: &mut Rider) {
    let clip = env.clip(0);
    if (clip as u32).wrapping_sub(0x203) < 2 {
        r.vel = st(scale(ld(r.vel), F(f32::from_bits(0x3f7d70a4))));
    } else if clip == 0x202 {
        let mut s = stick(pad, 0x15);
        if 0 <= r.controller {
            s = 1.0;
        }
        if s.abs() <= 0.5 && 0.0 <= dot(ld(r.board), ld(r.vel)).0 {
            return reset_check(env, pad, r);
        }
        r.stance ^= 1;
        let a = if r.stance != 0 { F(std::f32::consts::PI) } else { F(0.0) };
        let f = a * F(0.15915494) + F(0.5);
        let mut fl = F(f.0 as i32 as f32);
        if f.0 < fl.0 {
            fl = fl - F(1.0);
        }
        r.x46d4 = r.stance;
        r.facing = (a - fl * F(6.2831855)).0;
        if r.goofy {
            s = -s;
        }
        if 0.5 < s {
            env.play_clip(r, 0x203);
        } else if -0.5 <= s {
            env.rotate_about_axis(r, std::f32::consts::PI);
            env.play_clip(r, 0x201);
        } else {
            env.play_clip(r, 0x204);
        }
    } else if clip == 0x1ff || clip == 0x221 {
        env.set_state(r, 3);
    }
    reset_check(env, pad, r);
}

/// `Boarder_SetTimeScaleRateLimited` 0x11cff0: the time scale (+0x12c) toward `v` by at most 0.008446341
pub fn set_time_scale_rate_limited(v: f32, scale: &mut f32) {
    let step = F(0.008446341);
    let cur = F(*scale);
    *scale = if (F(v) + step).0 < cur.0 {
        (cur - step).0
    } else if cur.0 < (F(v) - step).0 {
        (cur + step).0
    } else {
        v
    };
}

/// `Boarder_IsDoneFinishing` 0x11d9e0: a computer rider in control state 1, or in 6 with its finish line at phase 4
/// (`FinishLineControl_IsPhase4` 0x1065f0 on the board object's +0x670)
pub fn is_done_finishing(controller: i32, control: u32, finish_phase: i32) -> bool {
    if controller < 0 {
        return false;
    }
    control == 1 || (control == 6 && finish_phase == 4)
}

/// `FinishLineControl_IsDone` 0x1065c8: past phase 1 and 240 ticks
pub fn finish_line_done(phase: i32, ticks: i32) -> bool {
    1 < phase && 0xf0 < ticks
}

/// `Boarder_StartLineOffset` 0x11ea00: the character entry's +0x60 · 90 − 90
pub fn start_line_offset(x60: f32) -> f32 {
    (F(x60) * F(90.0) - F(90.0)).0
}

/// `Boarder_TickSfxFadeIn` 0x11f468: +0x5af0 counted down, its old value (0 once there)
pub fn tick_sfx_fade_in(n: &mut i32) -> i32 {
    let v = *n;
    if 0 < v {
        *n = v - 1;
        v
    } else {
        0
    }
}

/// `FUN_0011f488`(+0x1ec, +0x1e8, the layer-0 phase when asked): +0x1ec above 0, or +0x1e8 = 4 past half its clip
pub fn x1ec_or_half(x1ec: i32, x1e8: i32, phase: &mut dyn FnMut() -> f32) -> bool {
    if 0 < x1ec {
        return true;
    }
    x1e8 == 4 && 0.5 <= phase()
}

/// `Boarder_LaunchFromGate` 0x11a7b0: from the gate's anticipation (8) to its launch (9)
pub fn launch_from_gate(env: &mut dyn ControlEnv, r: &mut Rider) {
    if r.control == 8 {
        env.set_state(r, 9);
    }
}

/// `Boarder_AddSpeedSample` 0x155dc8(speed): the sum (+0xdc, ·0.036) and the count (+0xe0)
pub fn add_speed_sample(speed: f32, sum: &mut f32, count: &mut f32) {
    *sum = (F(*sum) + F(speed) * F(0.036000002)).0;
    *count = (F(*count) + F(1.0)).0;
}

/// The vibration a player's pad gets (`Vibration_Construct` 0x150a70: the device, 0): its level
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rumble {
    pub level: f32,
}

/// The pad's motors (the device's virtuals +0x24 and +0x2c) and whether the race holds them off (the race control
/// +0x40 → +0x20 between 1 and 9)
pub trait RumbleEnv {
    fn held_off(&mut self) -> bool;
    /// +0x24(1, frames): a pulse
    fn pulse(&mut self, frames: i32);
    /// +0x2c(strength, mode): 2 on, −1 off
    fn motor(&mut self, strength: f32, mode: i64);
}

impl Rumble {
    /// `Rumble_Add` 0x150be8(amount): above 0.5 (and not held off) a pulse of amount·1.67 s (0.02..0.1) in frames;
    /// the level up to amount + 0.8 (at most 2, never down)
    pub fn add(&mut self, env: &mut dyn RumbleEnv, amount: f32) {
        if 0.5 < amount && !env.held_off() {
            let t = F(amount) * F(1.6666666);
            let d = if t.0 < 0.02 { F(0.02) } else if 0.1 < t.0 { F(0.1) } else { t };
            env.pulse((d * F(60.0)).0 as i32);
        }
        let v = (F(amount) + F(0.8)).0;
        self.level = if v < self.level { self.level } else if 2.0 < v { 2.0 } else { v };
    }

    /// `Rumble_Update` 0x150d18: a level runs the motor (at most 1) and decays (·0.9833 − 0.025), stopping it at 0;
    /// held off or at 0, the motor off
    pub fn update(&mut self, env: &mut dyn RumbleEnv) {
        if 0.0 < self.level && !env.held_off() {
            env.motor(if 1.0 < self.level { 1.0 } else { self.level }, 2);
            let v = F(self.level) * F(0.98333335) - F(0.025);
            self.level = v.0;
            if v.0 <= 0.0 {
                env.motor(0.0, -1);
            }
        } else {
            env.motor(0.0, -1);
        }
    }
}

/// What the event bits read of the rider: +0x384 last tick's bits, +0x388 this tick's, +0x114 the multipliers to
/// give, +0x418 the controller, +0x10c / +0x110 the place before and now, +0x460 the slot
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EventRider {
    pub prev: u32,
    pub cur: u32,
    pub mult: i32,
    pub controller: i32,
    pub place_prev: i32,
    pub place: i32,
    pub slot: i32,
}

impl EventRider {
    fn on(&self, e: u32) -> bool {
        ((self.cur & !self.prev) as i32 >> (e & 31)) & 1 != 0
    }
    fn off(&self, e: u32) -> bool {
        ((!self.cur & self.prev) as i32 >> (e & 31)) & 1 != 0
    }
}

/// The game the event bits tell
pub trait EventEnv {
    /// a lesson running (gApp+0x730 → +0x34): its event 0x10 (`FUN_0014d928`, the arg) and its end (`FUN_0014d930`)
    fn lesson(&mut self) -> bool;
    fn lesson_event(&mut self, arg: u32);
    fn lesson_end(&mut self);
    /// `Boarder_Finish`, `Scoring_MsgMultiplier`(+0x5820), `Speech_OnCheckpoint`, `Boarder_HitCheckpoint`(arg),
    /// `FUN_00233ba0`(the places), `Game_IsLeadOrFocusRider`(slot), `Audio_MusicEvent`, `Audio_CheerForRider`, the
    /// speech lines `FUN_00234558`(k), `FUN_00234610`(0), `FUN_00233740`; the event sound `FUN_0021c0a0`(event, on);
    /// `Audio_QueueEventSound` 0x21d0b0(kind) / `Audio_StopEventSound` 0x21d5c8
    fn finish(&mut self, r: &mut EventRider);
    fn msg_multiplier(&mut self, r: &mut EventRider);
    fn on_checkpoint(&mut self, r: &mut EventRider);
    fn hit_checkpoint(&mut self, r: &mut EventRider, arg: u32);
    fn placing(&mut self, r: &mut EventRider, before: i32, now: i32);
    fn is_lead(&mut self, slot: i32) -> bool;
    fn music_event(&mut self, e: u32);
    fn cheer(&mut self, r: &mut EventRider);
    fn local_line(&mut self, r: &mut EventRider, k: u32);
    fn local_line2(&mut self, r: &mut EventRider);
    fn listener_line(&mut self, r: &mut EventRider);
    fn event_sound(&mut self, r: &mut EventRider, event: [u32; 2], on: bool);
    fn queue_event_sound(&mut self, kind: u32);
    fn stop_event_sound(&mut self);
}

/// `cBoarder::vf10` 0x11a348(event: its bit and argument): a bit turning on during a lesson goes to the lesson (0x10,
/// and 10 its end); else 10 finishes (no multipliers left), 0xd gives a multiplier, 0xc counts a player's
/// checkpoint; still on (the flags read again), 0xc's speech and place line, and for the lead or focus rider the
/// music events (1 → 0, 9, 10 / 0x12 with a cheer) and speech lines (0x13..0x16); the event's sound on; a bit
/// turning off its sound off; 0xe / 0x17 / 0x18 queue the event sound 0 / 1 / 2 on, stop it off
pub fn event_bits(env: &mut dyn EventEnv, r: &mut EventRider, event: [u32; 2]) {
    let e = event[0];
    if r.on(e) {
        if env.lesson() {
            if e == 0x10 {
                env.lesson_event(event[1]);
            }
            if e == 10 {
                env.lesson_end();
            }
            return;
        }
        if e == 10 && r.mult == 0 {
            env.finish(r);
        }
        if e == 0xd && 0 < r.mult {
            env.msg_multiplier(r);
            r.mult -= 1;
        }
        if e == 0xc && r.controller < 0 {
            env.on_checkpoint(r);
            env.hit_checkpoint(r, event[1]);
        }
        if r.on(e) {
            if e == 0xc {
                env.on_checkpoint(r);
                let (b, n) = (r.place_prev, r.place);
                env.placing(r, b, n);
                r.place_prev = r.place;
            }
            if env.is_lead(r.slot) {
                match e {
                    1 => env.music_event(0),
                    9 => env.music_event(9),
                    10 if r.mult == 0 => env.music_event(10),
                    0x12 if r.mult == 0 => {
                        env.music_event(10);
                        env.cheer(r);
                    }
                    0x13 => env.local_line(r, 0),
                    0x14 => env.local_line(r, 1),
                    0x15 => env.local_line2(r),
                    0x16 => env.listener_line(r),
                    _ => {}
                }
            }
            if r.on(e) {
                env.event_sound(r, event, true);
            }
        }
    }
    if r.off(e) {
        env.event_sound(r, event, false);
    }
    for (bit, kind) in [(0xe, 0), (0x17, 1), (0x18, 2)] {
        if e == bit {
            if r.on(bit) {
                env.queue_event_sound(kind);
            } else if r.off(bit) {
                env.stop_event_sound();
            }
        }
    }
}

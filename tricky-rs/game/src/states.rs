//! The boarder's motion and control state changes exactly as the original makes them (board row G18):
//! `Boarder_SetMotion` 0x11f610 → `Boarder_ChangeMotionState` 0x11c648 and `Boarder_SetState` 0x11f608 →
//! `Boarder_ChangeControlState` 0x11c7d8, with the switches they run (`Boarder_ExitMotionState` 0x11c6b0,
//! `Boarder_EnterMotionState` 0x11c710, `Boarder_ExitControlState` 0x11c840, `Boarder_EnterControlState` 0x11c8e8).
//! Checked for every old and new value in the function runner (`tests/states.rs`).
//!
//! The state manager (rider+0x5ae0) starts with the rider's address; the motion is rider+0x424, the control state
//! rider+0x428. Each state's object sits in the manager at the offset given here.

/// One state's enter or exit routine: its address, name and the offset of its object in the state manager (the
/// routine's a0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFn {
    pub addr: u32,
    pub name: &'static str,
    pub obj: u32,
}

const fn f(addr: u32, name: &'static str, obj: u32) -> Option<StateFn> {
    Some(StateFn { addr, name, obj })
}

/// `Boarder_ExitMotionState` 0x11c6b0: the routine leaving motion `m` runs (none for 0, 4 and over 6).
pub fn motion_exit(m: u32) -> Option<StateFn> {
    match m {
        1 => f(0x108368, "AirMotion_Exit", 0x480),
        2 => f(0x10aec8, "GroundMotion_Exit", 0x4b0),
        3 => f(0x10b090, "RailSlideMotion_Exit", 0x4c0),
        5 => f(0x10d680, "WipeoutMotion_Exit", 0x4f0),
        6 => f(0x110290, "WipeOutRecoverMotion_Exit", 0x5f0),
        _ => None,
    }
}

/// `Boarder_EnterMotionState` 0x11c710: the routine entering motion `m` runs (a1 the old motion).
pub fn motion_enter(m: u32) -> Option<StateFn> {
    match m {
        1 => f(0x108310, "AirMotion_Enter", 0x480),
        2 => f(0x109858, "GroundMotion_Enter", 0x4b0),
        3 => f(0x10b068, "RailSlideMotion_Enter", 0x4c0),
        5 => f(0x10c198, "WipeoutMotion_Enter", 0x4f0),
        6 => f(0x110260, "WipeOutRecoverMotion_Enter", 0x5f0),
        _ => None,
    }
}

/// `Boarder_ExitControlState` 0x11c840: the routine leaving control state `s` runs.
pub fn control_exit(s: u32) -> Option<StateFn> {
    match s {
        2 => f(0x104388, "BumpControl_Exit", 0x610),
        3 => f(0x1033c0, "CruiseControl_Exit", 0x630),
        4 => f(0x104178, "ChangeFakieControl_Exit", 0x650),
        5 => f(0x104688, "FakieCruiseControl_Exit", 0x660),
        8 => f(0x106140, "GateAnticipateControl_Exit", 0x6a0),
        0xb => f(0x104790, "LandHardControl_Exit", 0x6e0),
        0xc => f(0x104e38, "LandNormalControl_Exit", 0x6f0),
        0xe => f(0x1055a8, "PrewindControl_Exit", 0x710),
        0xf => f(0x1075f0, "RailSlideControl_Exit", 0x720),
        0x10 => f(0x1029d0, "SpinControl_Exit", 0x740),
        0x14 => f(0x108280, "WipeOutRecoverControl_Exit", 0x7f0),
        0x16 => f(0x106a48, "ResetWaitControl_Exit", 0x810),
        _ => None,
    }
}

/// `Boarder_EnterControlState` 0x11c8e8: the routine entering control state `s` runs (a1 the old state). Entering
/// the reset state (0x16) runs none: it zeroes the reset timer (manager +0x818) inline.
pub fn control_enter(s: u32) -> Option<StateFn> {
    match s {
        2 => f(0x104190, "BumpControl_Enter", 0x610),
        3 => f(0x102fb0, "CruiseControl_Enter", 0x630),
        4 => f(0x103eb8, "ChangeFakieControl_Enter", 0x650),
        6 => f(0x106600, "FinishLineControl_Enter", 0x670),
        7 => f(0x105d80, "Gate2BaseControl_Enter", 0x690),
        8 => f(0x105f48, "GateAnticipateControl_Enter", 0x6a0),
        9 => f(0x106228, "GateLaunchControl_Enter", 0x6c0),
        0xc => f(0x1049f8, "LandNormalControl_Enter", 0x6f0),
        0xd => f(0x1000b8, "NaturalAirControl_Enter", 0x700),
        0xe => f(0x105070, "PrewindControl_Enter", 0x710),
        0xf => f(0x106dc8, "RailSlideControl_Enter", 0x720),
        0x10 => f(0x100598, "SpinControl_Enter", 0x740),
        0x13 => f(0x108110, "WipeOutControl_Enter", 0x7e0),
        0x15 => f(0x106500, "LessonWaitControl_Enter", 0x800),
        _ => None,
    }
}

/// What a change does, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// the old state's exit routine (the field still holds the old value)
    Exit(StateFn),
    /// the new state's enter routine, a1 the old value (the field already holds the new one)
    Enter(StateFn, u32),
    /// entering state 0x16: the reset timer (manager +0x818, `reset::ResetState::timer`) = 0
    ClearResetTimer,
}

/// `Boarder_SetMotion` 0x11f610: nothing if `new` is the motion already; else exit the old, store, enter the new.
pub fn set_motion(motion: &mut u32, new: u32) -> Vec<Change> {
    if new == *motion {
        return Vec::new();
    }
    let old = *motion;
    let mut out: Vec<Change> = motion_exit(old).map(Change::Exit).into_iter().collect();
    *motion = new;
    out.extend(motion_enter(new).map(|e| Change::Enter(e, old)));
    out
}

/// `Boarder_SetState` 0x11f608: as [`set_motion`] for the control state.
pub fn set_state(state: &mut u32, new: u32) -> Vec<Change> {
    if new == *state {
        return Vec::new();
    }
    let old = *state;
    let mut out: Vec<Change> = control_exit(old).map(Change::Exit).into_iter().collect();
    *state = new;
    if new == 0x16 {
        out.push(Change::ClearResetTimer);
    } else {
        out.extend(control_enter(new).map(|e| Change::Enter(e, old)));
    }
    out
}

type V4 = [f32; 4];

/// The rider fields the small state routines read and write (offsets from the rider).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateRider {
    pub x138: f32,
    pub x13c: f32,
    /// +0x150 (read)
    pub vel: V4,
    /// +0x170..+0x1b0: the attitude (`ChangeFakieControl_Enter` turns it half round about the board's up)
    pub orient: crate::orient::Orient,
    /// +0x1b4: the stance (switch or regular, flipped by a fakie change)
    pub stance: u32,
    pub x1cc: f32,
    /// +0x1dc
    pub kind: u32,
    pub x1e4: u32,
    pub x1e8: u32,
    pub x1ec: u32,
    /// +0x1fc, +0x200, +0x204, +0x208, +0x20c, +0x210
    pub x1fc: [f32; 6],
    /// +0x214: the steer
    pub steer: f32,
    pub x218: f32,
    pub x21c: f32,
    /// +0x244: the lean
    pub lean: f32,
    /// +0x268, +0x26c, +0x270
    pub x268: [f32; 3],
    pub x284: f32,
    pub x288: f32,
    pub x28c: f32,
    /// +0x448, +0x44c: the reset fades
    pub x448: f32,
    pub x44c: f32,
    /// +0x46d4 and +0x46e0 (in the animation controller at +0x46c0)
    pub x46d4: u32,
    pub x46e0: f32,
    /// +0x5ae4 (a sound flag) and +0x5af0
    pub x5ae4: u32,
    pub x5af0: u32,
    /// +0x1f4, +0x1f8
    pub x1f4: f32,
    pub x1f8: f32,
    /// +0x27c, +0x280 (negated with the stance on a rail)
    pub x27c: f32,
    pub x280: f32,
    /// +0x46dc: the animation controller's layer mask
    pub x46dc: u32,
    /// the word at +0xb50 of the pivot bone's record (rider + +0x44d8 × 0x394), none when +0x44d8 < 0
    pub bone_b50: Option<u32>,
    /// read: the camera (+0x38), the board class (+0x420), the control state (+0x428), stats+0x40
    pub cam: i32,
    pub class: u32,
    pub state: u32,
    pub stats40: u32,
}

/// The state object words a routine reads and writes (+8, +0xc, +0x10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateObj {
    pub w8: u32,
    pub wc: u32,
    pub w10: u32,
}

/// What a state routine calls, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineCall {
    /// `AirPredict_AtTakeoff` 0x123b10(rider+0x57b0)
    AirPredictAtTakeoff,
    /// `Sfx_OnAirEnter` 0x2174f8(sound, rider, state == 0xd)
    SfxOnAirEnter(bool),
    /// `Sfx_StopWipeoutLoop` 0x218b20(sound, rider)
    SfxStopWipeoutLoop,
    /// the rider's virtual +0x9c
    Virtual9c,
    /// `Boarder_ProbeGround` 0x128ae8(rider)
    ProbeGround,
    /// `Boarder_UpdateBoost` 0x11d038(rider, 0, 0)
    BoostOff,
    /// `RailSlideControl_PlayQueuedClip` 0x107628(the rail control object)
    PlayQueuedRailClip,
    /// `Boarder_StartDisappointed` 0x11f3f8(rider, 1)
    StartDisappointed,
    /// `HUD_HideItem` 0x1a7860(the HUD, 0xc)
    HideHud,
    /// `CamScript_Start` 0x175de0(the rider camera script, 0x360630, 0)
    StartCamScript,
    /// `AnimCtl_BlendOutAll` 0x15f050(rider+0x46c0)
    BlendOutAll,
    /// `AnimCtl_PlayClip` 0x15fcf8(rider+0x46c0, clip, 0, 5)
    PlayClip(u32),
    /// `AnimCtl_TurnAllLayers` 0x15eef8(π, rider+0x46c0)
    TurnAllLayers,
    /// `AnimCtl_TurnAllLayers` 0x15eef8(angle bits, rider+0x46c0) for an angle other than π
    TurnAllLayersBy(u32),
    /// `Score_UpdateSpin` 0x155e00(rider+0x5820, 0)
    ScoreSpinZero,
    /// `Boarder_SetBrakeTarget` 0x126c78(0, rider)
    BrakeTargetZero,
    /// `Boarder_OnClipQueued` 0x106dc0(rider)
    OnClipQueued,
    /// `AnimCtl_PlayClip` 0x15fcf8(rider+0x46c0, clip, 0, 5) with the layer mask +0x46dc it is played under
    PlayClipMasked(u32, u32),
}

/// `Sfx_ClearRiderFlag` 0x218b68 → 0x11f440(rider, 0): +0x5ae4 cleared, and +0x5af0 = 5 when it was set.
fn clear_sound_flag(r: &mut StateRider) {
    if r.x5ae4 != 0 {
        r.x5ae4 = 0;
        r.x5af0 = 5;
    }
}

/// One of the small state routines the dispatchers call, by its address (`a1` as the dispatcher passes it: the
/// new value to an exit, the old to an enter); `None` for the ones not ported here (`SpinControl_Exit`,
/// `SpinControl_Enter`, `LandNormalControl_Enter`, `RailSlideControl_Enter`, the gate routines in `gate.rs`,
/// `WipeoutMotion_Enter`).
pub fn run_routine(addr: u32, r: &mut StateRider, o: &mut StateObj, a1: u32) -> Option<Vec<RoutineCall>> {
    use RoutineCall as C;
    let mut calls = Vec::new();
    match addr {
        // AirMotion_Exit
        0x108368 => {
            r.x138 = 0.0;
            r.x13c = 0.0;
        }
        // GroundMotion_Exit
        0x10aec8 => r.x1cc = 0.0,
        // RailSlideMotion_Exit
        0x10b090 => {
            r.x28c = -0.25;
            r.x284 = -0.25;
            r.x288 = f32::from_bits(0x3fd5_5555);
            r.x28c = 1.0;
        }
        // WipeoutMotion_Exit: the wipe-out loop stopped; the sound flag cleared unless going on to recover (6)
        0x10d680 => {
            calls.push(C::SfxStopWipeoutLoop);
            if a1 != 6 {
                clear_sound_flag(r);
            }
            calls.push(C::Virtual9c);
        }
        // WipeOutRecoverMotion_Exit
        0x110290 => {
            calls.push(C::ProbeGround);
            o.w8 = 0;
            clear_sound_flag(r);
        }
        // AirMotion_Enter
        0x108310 => {
            calls.push(C::AirPredictAtTakeoff);
            r.x1e4 = 0x1a;
            r.x1e8 = 0;
            r.x1ec = 0;
            calls.push(C::SfxOnAirEnter(r.state == 0xd));
        }
        // GroundMotion_Enter
        0x109858 => {
            o.w8 = 0;
            r.kind = 0;
            clear_sound_flag(r);
        }
        // WipeOutRecoverMotion_Enter
        0x110260 => {
            calls.push(C::ProbeGround);
            o.w8 = 0;
        }
        // RailSlideMotion_Enter
        0x10b068 => {
            o.w8 = 0;
            o.wc = 0;
            r.x288 = f32::from_bits(0x3fd5_5555);
            r.x28c = 0.0;
        }
        // BumpControl_Exit
        0x104388 => {
            r.x1fc = [0.0; 6];
            o.wc = 0;
            o.w8 = 0;
        }
        // CruiseControl_Exit
        0x1033c0 => calls.push(C::BoostOff),
        // ChangeFakieControl_Exit
        0x104178 => {
            r.x1fc[0] = 0.0;
            r.x1fc[1] = 0.0;
            r.x1fc[2] = 0.0;
        }
        // FakieCruiseControl_Exit
        0x104688 => r.x1fc = [0.0; 6],
        // LandHardControl_Exit
        0x104790 => {
            calls.push(C::Virtual9c);
            r.x1fc = [0.0; 6];
            r.x268 = [0.0; 3];
        }
        // LandNormalControl_Exit, WipeOutControl_Enter
        0x104e38 | 0x108110 => calls.push(C::Virtual9c),
        // PrewindControl_Exit
        0x1055a8 => {
            calls.push(C::BoostOff);
            r.x1fc[0] = 0.0;
            r.x1fc[1] = 0.0;
            r.x1fc[2] = 0.0;
        }
        // RailSlideControl_Exit
        0x1075f0 => {
            calls.push(C::BoostOff);
            calls.push(C::PlayQueuedRailClip);
        }
        // WipeOutRecoverControl_Exit
        0x108280 => calls.push(C::StartDisappointed),
        // ResetWaitControl_Exit: the fades back, the HUD item hidden
        0x106a48 => {
            r.x448 = 1.0;
            r.x44c = 0.0;
            if -1 < r.cam {
                calls.push(C::HideHud);
            }
        }
        // BumpControl_Enter
        0x104190 => {
            r.x268[1] = 2.0;
            r.x268[2] = 0.0;
        }
        // CruiseControl_Enter
        0x102fb0 => {
            o.w10 = 0;
            r.kind = 0;
            o.w8 = 0;
            o.wc = 0;
        }
        // FinishLineControl_Enter: +8 set when crossing at 277.77777 cm/s or slower
        0x106600 => {
            let speed = crate::jump::dot(r.vel, r.vel).sqrt();
            o.w8 = if f32::from_bits(0x438a_e38e) < speed { 0 } else { 1 };
            o.wc = 0;
        }
        // Gate2BaseControl_Enter
        0x105d80 => {
            if -1 < r.cam {
                calls.push(C::StartCamScript);
            }
        }
        // NaturalAirControl_Enter
        0x1000b8 => {
            r.x1fc[1] = 4.0;
            r.x1fc[2] = 0.0;
            o.w8 = r.steer.to_bits();
        }
        // PrewindControl_Enter
        0x105070 => {
            let k = f32::from_bits(0x40d5_5555);
            r.x218 = k;
            r.x21c = 0.0;
            r.x268[1] = k;
            r.x268[2] = 0.0;
            r.x1fc[1] = k;
            r.x1fc[2] = 0.0;
            calls.push(C::BlendOutAll);
            calls.push(C::PlayClip(0x217));
            o.w8 = 0;
        }
        // LessonWaitControl_Enter
        0x106500 => {
            calls.push(C::PlayClip(0x1ff));
            o.w8 = 0;
        }
        // ChangeFakieControl_Enter: the stance flips, the board turns half round about its up, the animation
        // facing set to the stance (0 or π, wrapped to −π..π), steer and lean negated
        0x103eb8 => {
            r.stance ^= 1;
            let up = r.orient.up;
            crate::orient::rotate_about_axis(&mut r.orient, f32::from_bits(0x4049_0fdb), up);
            calls.push(C::TurnAllLayers);
            let a = if r.stance != 0 { f32::from_bits(0x4049_0fdb) } else { 0.0 };
            let t = a * f32::from_bits(0x3e22_f983) + 0.5;
            let mut n = t as i32 as f32;
            if t < n {
                n -= 1.0;
            }
            r.x46e0 = a - n * f32::from_bits(0x40c9_0fdb);
            r.steer = -r.steer;
            r.lean = -r.lean;
            r.x268[1] = 2.0;
            r.x268[2] = 0.0;
            if r.class != 2 {
                r.x46d4 = r.stance;
                calls.push(C::PlayClip(0x200));
            } else if r.stance == r.stats40 {
                calls.push(C::PlayClip(0x1ff));
            } else {
                calls.push(C::PlayClip(0x200));
            }
        }
        // LandNormalControl_Enter: past a quarter second, the landing clip under a mask of the pivot bone's
        // layers: straight (0x20c) when the board points within 30° of the travel flattened onto its up, else to the
        // side the travel goes (0x20d, 0x20e; mirrored riding switch); the mask then all set
        0x1049f8 => {
            if f32::from_bits(o.w8) <= 0.25 {
                return Some(calls);
            }
            let mut mask = 0u32;
            for k in [3u32, 6, 10, 12, 4, 9, 11, 5, 7, 8, 1, 2] {
                if let Some(b) = r.bone_b50 {
                    mask |= 1u32 << (b.wrapping_add(k) & 0x1f);
                }
            }
            r.x46dc = mask;
            use crate::jump::{dot, normalize, scale, sub};
            let v = normalize(r.vel);
            let h = normalize(sub(v, scale(r.orient.up, dot(v, r.orient.up))));
            let clip = if dot(r.orient.fwd, h).abs() <= 0.866 {
                let mut side = dot(r.orient.side, h);
                if r.stance != 0 {
                    side = -side;
                }
                if side <= 0.0 { 0x20e } else { 0x20d }
            } else {
                0x20c
            };
            calls.push(C::PlayClipMasked(clip, r.x46dc));
            r.x46dc = 0xffff_ffff;
        }
        // RailSlideControl_Enter: onto a rail. The board is turned to its stance for the rail kind (+0x1dc: 4 and 3
        // across the rail, ±π/2 about its up; 2 and others along it), switching stance where the kind wants the
        // other one (an alpine board keeps its natural stance, stats+0x40, and has kinds 1 and 2 swapped for it),
        // the animation's facing set to match, and the rail clip played (0x257, 0x256, 0x255, 0x254)
        0x106dc8 => {
            o.w8 = 0;
            o.wc = 0;
            calls.push(C::ScoreSpinZero);
            calls.push(C::BrakeTargetZero);
            o.w10 = 0;
            r.x1f4 = f32::from_bits(0x4055_5555);
            r.x1f8 = 0.0;
            calls.push(C::BlendOutAll);
            let alpine = r.class == 2;
            let natural_switch = r.stats40 != 0;
            if alpine && natural_switch {
                if r.kind == 2 {
                    r.kind = 1;
                } else if r.kind == 1 {
                    r.kind = 2;
                }
            }
            let pi = f32::from_bits(0x4049_0fdb);
            let half = f32::from_bits(0x3fc9_0fdb);
            // the facing as the compiler folded it: angle − floor(c)·2π, c the angle's turns + ½
            let face = |angle: f32, c: f32| {
                let mut n = c as i32 as f32;
                if c < n {
                    n -= 1.0;
                }
                angle - n * f32::from_bits(0x40c9_0fdb)
            };
            let clip;
            match r.kind {
                4 | 3 => {
                    let across = if r.kind == 4 { 1.0 } else { -1.0 };
                    let (angle, c) = if alpine && natural_switch {
                        if r.stance == 0 {
                            switch_stance(r);
                            calls.push(C::TurnAllLayers);
                        }
                        (-half * across, if across > 0.0 { 0.25 } else { 0.75 })
                    } else {
                        if r.stance != 0 {
                            switch_stance(r);
                            if alpine {
                                calls.push(C::TurnAllLayers);
                            }
                        }
                        (half * across, if across > 0.0 { 0.75 } else { 0.25 })
                    };
                    let up = r.orient.up;
                    crate::orient::rotate_about_axis(&mut r.orient, angle, up);
                    // the layers turn by +π/2 across (kind 4), −π/2 the other way (kind 3), whatever the board
                    calls.push(C::TurnAllLayersBy((half * across).to_bits()));
                    r.x46e0 = face(angle, c);
                    clip = if r.kind == 4 { 0x257 } else { 0x256 };
                }
                2 => {
                    let wants_switch = !(alpine && natural_switch);
                    if wants_switch {
                        if r.stance == 0 {
                            switch_stance(r);
                            if alpine {
                                r.x46d4 = 0;
                            }
                            calls.push(C::TurnAllLayers);
                            r.x46e0 = face(pi, 1.0);
                        }
                    } else if r.stance != 0 {
                        switch_stance(r);
                        r.x46d4 = 1;
                        calls.push(C::TurnAllLayers);
                        r.x46e0 = face(0.0, 0.5);
                    }
                    clip = 0x255;
                }
                _ => {
                    if alpine && natural_switch {
                        if r.stance == 0 {
                            switch_stance(r);
                            calls.push(C::TurnAllLayers);
                            r.x46e0 = face(pi, 1.0);
                        }
                    } else if r.stance != 0 {
                        switch_stance(r);
                        calls.push(C::TurnAllLayers);
                        r.x46e0 = face(0.0, 0.5);
                    }
                    clip = 0x254;
                }
            }
            calls.push(C::PlayClip(clip));
            calls.push(C::OnClipQueued);
        }
        _ => return None,
    }
    Some(calls)
}

/// `RailSlideControl_SwitchStance` 0x107fc8: the stance flipped, the board turned π about its up, steer, lean and
/// +0x27c, +0x280 negated, the animation's stance +0x46d4 set to the new one.
fn switch_stance(r: &mut StateRider) {
    r.stance ^= 1;
    let up = r.orient.up;
    crate::orient::rotate_about_axis(&mut r.orient, f32::from_bits(0x4049_0fdb), up);
    r.steer = -r.steer;
    r.x46d4 = r.stance;
    r.lean = -r.lean;
    r.x27c = -r.x27c;
    r.x280 = -r.x280;
}

//! Resets exactly as the original runs them (board row G1c3): `Boarder_RequestReset` 0x118f10 (with
//! `Boarder_StartReset` 0x119be0) starting the reset state, `ResetState_Update` 0x106a90 ticking it (the fade, then
//! the rider moved on along its path and placed: `aipath::advance_on_path`, `aipath::place_rider`), and
//! `Gate2BaseControl_Update` 0x105dc0, the control state that asks for one. Checked bit for bit in the function
//! runner (`tests/reset.rs`).

type V4 = [f32; 4];

/// The reset state (the rider's state manager, rider+0x5ae0, at +0x810): +8 the time in it, +0xc the height the
/// rider is placed at, +0x10 how far it is moved on along its path first, +0x14 (−1 from a request).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResetState {
    pub timer: f32,
    pub height: f32,
    pub advance: f32,
    pub x14: i32,
}

/// One rider of the race as a reset request reads it, in race order (`Race_GetRider`): its distance to the finish
/// (+0x374), place (+0x110) and velocity (+0x150).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceRider {
    pub to_finish: f32,
    pub place: i32,
    pub vel: V4,
}

/// What `Boarder_RequestReset` reads.
#[derive(Clone, Debug, Default)]
pub struct Request<'a> {
    /// rider+0x418: the finish time (−1 racing)
    pub finish: i32,
    /// `AIP_CanResetAt`(+0x344, the rider's path)
    pub can_reset: bool,
    /// rider+0x41c (1: the reset sound)
    pub x41c: i32,
    /// the rider's own +0x374 and +0x110
    pub to_finish: f32,
    pub place: i32,
    /// the stats byte +0x65 (stats at rider+0x464) is negative: no speed carried
    pub no_speed: bool,
    /// the game mode word (0x32de8c) and 0x32de88
    pub mode: i32,
    pub x32de88: i32,
    /// every rider in the race (`Race_GetRider` 0..0x32db6c)
    pub riders: &'a [RaceRider],
}

/// A reset that starts: the reset sound (`Sfx_OnRiderReset`) first if `sound`; then the reset state's fields, and
/// the rider set to motion 4 and state 0x16.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Started {
    pub sound: bool,
    pub state: ResetState,
}

/// `Boarder_RequestReset` 0x118f10 (a0 the rider, f12 the height): none once finished or where the path allows no
/// reset. In mode 0 with others in the race: `Boarder_StartReset` with the others' mean distance to the finish (not
/// under 0) as the distance to move on. Otherwise, in modes 2, 4 and 7 with others, over 10000 from the finish and
/// 0x32de88 not 8: 0.8003195 × the speed of the first rider in the next place (the place before for the last; 0 for
/// none, or when the rider's stats say so); else 0.
pub fn request_reset(height: f32, q: &Request) -> Option<Started> {
    if 0 <= q.finish || !q.can_reset {
        return None;
    }
    let sound = q.x41c == 1;
    let n = q.riders.len() as i32;
    let state = |advance: f32| ResetState { timer: 0.0, height, advance, x14: -1 };
    if q.mode == 0 {
        if 1 < n {
            let mut sum = -q.to_finish;
            for r in q.riders {
                sum += r.to_finish;
            }
            let mean = sum / (n - 1) as f32;
            return start_reset(mean, height, q).map(|s| Started { sound, ..s });
        }
        return Some(Started { sound, state: state(0.0) });
    }
    if n < 2 || q.to_finish <= 10000.0 || q.x32de88 == 8 || !(q.mode == 2 || q.mode == 7 || q.mode == 4) {
        return Some(Started { sound, state: state(0.0) });
    }
    let next = if q.place < n - 1 { q.place + 1 } else { q.place - 1 };
    let mut speed = 0.0f32;
    if let Some(r) = q.riders.iter().find(|r| r.place == next) {
        speed = crate::jump::dot(r.vel, r.vel).sqrt();
    }
    if q.no_speed {
        speed = 0.0;
    }
    Some(Started { sound, state: state(speed * 0.8003195) })
}

/// `Boarder_StartReset` 0x119be0 (a0 the rider, f12 the distance to move on, not under 0, f13 the height): the same
/// checks as a request, then the reset state. (Its timer is the state's own, set on entering it.)
pub fn start_reset(advance: f32, height: f32, q: &Request) -> Option<Started> {
    if !(q.finish < 0 && q.can_reset) {
        return None;
    }
    let advance = if advance < 0.0 { 0.0 } else { advance };
    Some(Started { sound: false, state: ResetState { timer: 0.0, height, advance, x14: -1 } })
}

/// What a reset tick reads besides its state: the rider's camera (+0x38, −1 none) and index (+0x460), the race
/// (the game's +0x40: its +0x20 and +0x54), the camera-script player (0x337770), and the camera scripts' state: the
/// shared one (0x3368b0 running, or 0x3368ec set) and the rider's own (the game's +0xa0: running, or its +0x3c set).
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    pub cam: i32,
    pub index: i32,
    pub race: Option<(i32, i32)>,
    pub script_player: i32,
    pub shared_script: bool,
    pub own_script: bool,
}

/// What a reset tick does, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TickCall {
    /// `HUD_ShowItem`(the game's HUD, 0xc)
    ShowHud,
    /// `CamScript_Stop`(0x3368b0, 1)
    StopSharedScript,
    /// `CamScript_Stop`(the game's +0xa0, 1)
    StopOwnScript,
    /// `Boarder_AdvanceOnPath`(advance) then `ResetState_PlaceRider`(height, no spread)
    Place { advance: f32, height: f32 },
}

/// `ResetState_Update` 0x106a90: the timer moves on 1/60; rider+0x448 = clamp((0.5 − t)·2) (the fade out); past
/// 0.2 the HUD item 0xc (with a camera) and rider+0x44c = clamp(1 − (0.5 − t)/0.3); past 0.8 the camera scripts
/// stopped (in a race whose +0x20 is 1..9, for the camera-script player's rider) and the rider moved on and placed;
/// rider+0x44c 0 while the race's +0x54 is set. Returns rider+0x448 and +0x44c (`fade` keeps its value before 0.2).
pub fn reset_update(s: &mut ResetState, t: &Tick, fade: (f32, f32)) -> ((f32, f32), Vec<TickCall>) {
    let mut calls = Vec::new();
    let timer = s.timer + 0.016666668;
    let mut a = 0.5 - timer;
    a = a + a;
    s.timer = timer;
    let clamp = |v: f32| if v < 0.0 { 0.0 } else if 1.0 < v { 1.0 } else { v };
    let x448 = clamp(a);
    let mut x44c = fade.1;
    if 0.2 < s.timer {
        if -1 < t.cam {
            calls.push(TickCall::ShowHud);
        }
        x44c = clamp(1.0 - (0.5 - s.timer) / 0.3);
    }
    if 0.8 < s.timer {
        if let Some((k, _)) = t.race {
            if k != 0 && k < 10 && t.script_player == t.index {
                if t.shared_script {
                    calls.push(TickCall::StopSharedScript);
                }
                if t.own_script {
                    calls.push(TickCall::StopOwnScript);
                }
            }
        }
        calls.push(TickCall::Place { advance: s.advance, height: s.height });
    }
    if let Some((_, x54)) = t.race {
        if x54 != 0 {
            x44c = 0.0;
        }
    }
    ((x448, x44c), calls)
}

/// The start-gate control after the gate opens (`Gate2BaseControl_Update` 0x105dc0), its calls in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateCall {
    /// `Boarder_UpdateBoost`(pad bit 3, pad bit 4)
    Boost(bool, bool),
    /// `Boarder_SetSteerTarget`(pad bits 5..10, signed, / 31)
    SteerTarget(f32),
    /// `Boarder_UpdateLeanAnim`(pad bits 23..24, bits 25..26: each −1, 0 or 1)
    Lean(f32, f32),
    /// motion 2, state 3: off down the course
    Ride,
    /// rider+0x304 = pad bit 0 clear; `Boarder_RequestReset`(101.46)
    Reset(u32),
}

/// The height a gate reset places the rider at (`Gate2BaseControl_Update`, the reset state's +0xc).
pub const GATE_RESET_HEIGHT: f32 = f32::from_bits(0x42ca_ebe0);

/// `Gate2BaseControl_Update` 0x105dc0 on the pad word: boost, steer and lean from it; then, with pad bit 0 clear
/// and the rider on the ground (+0x290), off down the course unless the animation's clip (`AnimCtl_GetClip`
/// (+0x46c0, 0)) is 0x1fd; else a reset at [`GATE_RESET_HEIGHT`].
pub fn gate2_base_control(pad: u32, on_ground: bool, clip: i32) -> Vec<GateCall> {
    let mut calls = vec![GateCall::Boost((pad as i32 >> 3) & 1 != 0, (pad as i32 >> 4) & 1 != 0)];
    calls.push(GateCall::SteerTarget(((pad << 0x15) as i32 >> 0x1a) as f32 / 31.0));
    let unit = |v: i32| if v == -1 { -1.0 } else if v == 1 { 1.0 } else { 0.0 };
    calls.push(GateCall::Lean(unit((pad << 7) as i32 >> 0x1e), unit((pad << 5) as i32 >> 0x1e)));
    if pad & 1 == 0 && on_ground {
        if clip == 0x1fd {
            return calls;
        }
        calls.push(GateCall::Ride);
        return calls;
    }
    calls.push(GateCall::Reset((pad ^ 1) & 1));
    calls
}

//! The camera's presets, chase parameters, modes and focus rider (board row E11g), on the game's bytes (`pf::Mem`),
//! checked in the function runner (`tests/camera_ctl.rs`).
//!
//! The race camera object is the global at 0x3368b0 (`MAIN`):
//! - +0 the mode index (0..3, the pad's camera button), +4 a cut pending, +0x14 the mode before looking back,
//!   +0x34 a script running, +0x428 the mode (2, 4, 5, 6; 3 looking back)
//! - +0x4e8 the rider slots, 0x20 bytes each: slot 0 the focus rider, 1.. the others in race order, 10 / 11
//!   (+0x648 / +0x668) the riders before and after the focus, 14..25 (+0x6a8) by character; each slot's +0 the
//!   rider, the rest its tracker (`Camera_TrackRider`)
//! - +0x828 the slots as last frame (+0x340), +0xea8 the slots as at the last cut (+0x680)
//!
//! A named camera (`CamCamera`, found by name in the camera file) is a block: its view (0x120 bytes, +0xcc the
//! chase distance, +0xdc the heading, +0x100 the pitch) and its lens at +0x120 (0x48 bytes, +0x20 the height
//! ratio). `GetView` / `GetLens` store each part's own address in it (+0x11c / the lens's +0x44).

use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::GAPP;

/// The race camera
pub const MAIN: u32 = 0x3368b0;
/// The chosen preset (1..8)
pub const PRESET: u32 = 0x3368a0;
/// The chase camera's rider (an index into the race's riders)
pub const CHASE_RIDER: u32 = 0x337770;
/// The replay's camera flag (`Camera_SetReplayMode`)
pub const REPLAY_MODE: u32 = 0x337774;
/// The second camera (the other player's), cut with the first
pub const SECOND: u32 = 0x337788;

/// The presets' named cameras (0x336608: the id, the name; up to id 13); others take "replay start" (0x383910)
pub const PRESETS: [(u32, &str); 7] = [
    (1, "chase reverse"),
    (2, "chase board"),
    (3, "chase eyes"),
    (4, "chase near"),
    (5, "chase far"),
    (6, "chase over"),
    (8, "replay start"),
];
/// The camera the presets are copied into (0x383920)
pub const REPLAY_CAM: &str = "replay cam";

/// What the camera's control calls out to
pub trait CamCtlEnv {
    /// `CamCamera::CamCamera` 0x16dd68 (`Cml_FindObject` 0x16e518 with the name): the named camera's block
    fn find_camera(&mut self, name: &str) -> u32;
    /// `Camera_Cut` 0x177db0 (the camera)
    fn cut(&mut self, m: &mut Mem, cam: u32);
    /// `CamScript_Stop` 0x175eb8 (the camera, 1)
    fn script_stop(&mut self, m: &mut Mem, cam: u32);
    /// `Race_GetRider` 0x1814a8 (the race, i)
    fn rider(&mut self, i: u32) -> u32;
    /// `Camera_TrackRider` 0x173270 (a rider slot, the camera's +4)
    fn track_rider(&mut self, m: &mut Mem, slot: u32, cut: u32);
    /// `CamScript_CutTo` 0x174c98 (the camera)
    fn cut_to(&mut self, m: &mut Mem, cam: u32);
}

fn riders(m: &Mem) -> i32 {
    m.r32(m.r32(m.r32(m.r32(GAPP) + 0x730) + 0x1c) + 0x88) as i32
}

/// `CamCamera::GetView` 0x16dda8: the block, its +0x11c set to itself
pub fn view(m: &mut Mem, block: u32) -> u32 {
    m.w32(block + 0x11c, block);
    block
}

/// `CamCamera::GetLens` 0x16ddb8: the block's +0x120, its +0x44 set to itself
pub fn lens(m: &mut Mem, block: u32) -> u32 {
    let l = block + 0x120;
    m.w32(l + 0x44, l);
    l
}

/// `Camera_GetPreset` 0x1786c0
pub fn preset(m: &Mem) -> u32 {
    m.r32(PRESET)
}

/// `Camera_SetMode` 0x1776d0: a change of mode asks for a cut (+4, +8)
pub fn set_mode(m: &mut Mem, cam: u32, mode: u32) {
    if mode != m.r32(cam + 0x428) {
        m.w32(cam + 8, 1);
        m.w32(cam + 4, 1);
    }
    m.w32(cam + 0x428, mode);
}

/// `Camera_SelectPreset` 0x1786d0: the preset (1..8, else 7) as the main camera's mode, its script stopped, the
/// preset's named camera copied whole (view and lens) into "replay cam", and a cut
pub fn select_preset(m: &mut Mem, env: &mut dyn CamCtlEnv, p: u32) {
    let p = if p.wrapping_sub(1) < 8 { p } else { 7 };
    m.w32(PRESET, p);
    set_mode(m, MAIN, p);
    env.script_stop(m, MAIN);
    let mut name = "replay start";
    for (id, n) in PRESETS {
        if id == p {
            name = n;
        }
    }
    let from = env.find_camera(name);
    let to = env.find_camera(REPLAY_CAM);
    let dv = view(m, to);
    let sv = view(m, from);
    for k in 0..0x120 / 4 {
        m.w32(dv + 4 * k, m.r32(sv + 4 * k));
    }
    let dl = lens(m, to);
    let sl = lens(m, from);
    for k in 0..0x48 / 4 {
        m.w32(dl + 4 * k, m.r32(sl + 4 * k));
    }
    env.cut(m, MAIN);
}

/// `Camera_NextPreset` 0x178680: on to the next preset (1..8, skipping 3)
pub fn next_preset(m: &mut Mem, env: &mut dyn CamCtlEnv) {
    let mut p = m.r32(PRESET).wrapping_add(1);
    m.w32(PRESET, p);
    if 8 < p as i32 {
        p = 1;
        m.w32(PRESET, p);
    }
    if p == 3 {
        p = 4;
        m.w32(PRESET, p);
    }
    select_preset(m, env, p);
}

/// The chase camera's parameters: the heading, the pitch, the distance, the height ratio and the rider's index
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chase {
    pub heading: f32,
    pub pitch: f32,
    pub distance: f32,
    pub height: f32,
    pub rider: u32,
}

/// `Camera_GetChaseParams` 0x178968: from "replay cam"
pub fn chase_params(m: &mut Mem, env: &mut dyn CamCtlEnv) -> Chase {
    let b = env.find_camera(REPLAY_CAM);
    let v = view(m, b);
    let v = m.r32(v + 0x11c);
    let l = lens(m, b);
    let l = m.r32(l + 0x44);
    let f = |m: &Mem, a: u32| f32::from_bits(m.r32(a));
    Chase { heading: f(m, v + 0xdc), pitch: f(m, v + 0x100), distance: f(m, v + 0xcc), height: f(m, l + 0x20), rider: m.r32(CHASE_RIDER) }
}

/// `Camera_SetChaseParams` 0x178a28: the pitch within ±1, the distance within 200..1200, the height ratio at least
/// 300 over the distance and within 0.5..1.9; the main camera on the rider; a cut when asked
pub fn set_chase_params(m: &mut Mem, env: &mut dyn CamCtlEnv, c: Chase, cut: bool) {
    let b = env.find_camera(REPLAY_CAM);
    let v = view(m, b);
    let l = lens(m, b);
    let mut pitch = c.pitch;
    if 1.0 < pitch {
        pitch = 1.0;
    } else if pitch < -1.0 {
        pitch = -1.0;
    }
    let mut dist = c.distance;
    if dist < 200.0 {
        dist = 200.0;
    } else if 1200.0 < dist {
        dist = 1200.0;
    }
    let mut h = c.height;
    if (F(h) * F(dist)).0 < 300.0 {
        h = (F(300.0) / F(dist)).0;
    }
    if h < 0.5 {
        h = 0.5;
    } else if f32::from_bits(0x3ff33333) < h {
        h = f32::from_bits(0x3ff33333);
    }
    let v = m.r32(v + 0x11c);
    let l = m.r32(l + 0x44);
    m.w32(v + 0xcc, dist.to_bits());
    m.w32(v + 0xdc, c.heading.to_bits());
    m.w32(v + 0x100, pitch.to_bits());
    m.w32(l + 0x20, h.to_bits());
    let r = env.rider(c.rider);
    set_focus_rider(m, env, MAIN, r);
    m.w32(CHASE_RIDER, c.rider);
    if cut {
        env.cut(m, MAIN);
    }
}

/// `Camera_AdjustChase` 0x178be8 (the step, which): 0 / 1 the heading by ∓2° × step, 2 / 3 the pitch, 4 / 5 the next
/// / previous rider (with a cut of both cameras), 6 / 7 the height ratio by ∓step / 180, 8 / 9 the distance by ∓5 ×
/// step. (It also takes `Math_AngleDiff` of the heading and drops it.)
pub fn adjust_chase(m: &mut Mem, env: &mut dyn CamCtlEnv, step: f32, which: u32) {
    let s5 = F(step) * F(5.0);
    let s180 = F(step) * F(f32::from_bits(0x3bb60b61));
    let deg2 = F(step) * F(f32::from_bits(0x3d0efa35));
    let mut c = chase_params(m, env);
    match which {
        0 => c.heading = (F(c.heading) - deg2).0,
        1 => c.heading = (F(c.heading) + deg2).0,
        2 => c.pitch = (F(c.pitch) - deg2).0,
        3 => c.pitch = (F(c.pitch) + deg2).0,
        4 | 5 => {
            let n = riders(m);
            let mut i = c.rider as i32;
            if which == 4 {
                i += 1;
                if i >= n {
                    i = 0;
                }
            } else {
                i -= 1;
                if i < 0 {
                    i = n - 1;
                }
            }
            c.rider = i as u32;
            env.cut(m, MAIN);
            env.cut(m, m.r32(SECOND));
        }
        6 => c.height = (F(c.height) - s180).0,
        7 => c.height = (F(c.height) + s180).0,
        8 => c.distance = (F(c.distance) - s5).0,
        9 => c.distance = (F(c.distance) + s5).0,
        _ => {}
    }
    set_chase_params(m, env, c, false);
}

/// `Camera_SetLookBack` 0x1776f0 (on): unless a script runs or the replay's camera is on, mode 3 (the mode before
/// kept at +0x14) and back
pub fn set_look_back(m: &mut Mem, cam: u32, on: u32) {
    if m.r32(cam + 0x34) != 0 || m.r32(REPLAY_MODE) != 0 {
        return;
    }
    if on == 1 && m.r32(cam + 0x428) != 3 {
        m.w32(cam + 0x14, m.r32(cam + 0x428));
        m.w32(cam + 0x20, 0);
        set_mode(m, cam, 3);
    } else if on == 0 && m.r32(cam + 0x428) == 3 {
        set_mode(m, cam, m.r32(cam + 0x14));
    }
}

/// `Camera_GetModeIndex` 0x173e08: the mode as the pad's index (2 → 0, 4 → 1, 5 → 2, 6 → 3, others 1)
pub fn mode_index(m: &Mem, cam: u32) -> u32 {
    match m.r32(cam + 0x428) {
        2 => 0,
        5 => 2,
        6 => 3,
        _ => 1,
    }
}

/// `Camera_ApplyModeIndex` 0x173eb8: the index's mode (0 → 2, 1 → 4, 2 → 5, 3 → 6, others 4) written straight to
/// +0x428, the script stopped, a cut asked (+4)
pub fn apply_mode_index(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32) {
    let mode = match m.r32(cam) {
        0 => 2,
        1 => 4,
        2 => 5,
        3 => 6,
        _ => 4,
    };
    m.w32(cam + 0x428, mode);
    env.script_stop(m, cam);
    m.w32(cam + 4, 1);
}

/// `Camera_SetModeIndex` 0x173eb0
pub fn set_mode_index(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32, i: u32) {
    m.w32(cam, i);
    apply_mode_index(m, env, cam);
}

/// `Camera_CycleMode` 0x173e70 (back): the index on (0..3, wrapping) and applied
pub fn cycle_mode(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32, back: bool) {
    let i = m.r32(cam) as i32;
    if !back {
        m.w32(cam, (i + 1) as u32);
        if 3 < i + 1 {
            m.w32(cam, 0);
        }
    } else {
        m.w32(cam, (i - 1) as u32);
        if i - 1 < 0 {
            m.w32(cam, 3);
        }
    }
    apply_mode_index(m, env, cam);
}

/// `Camera_SetFocusRider` 0x173c50: (another camera sets the main one's first) the focus rider (slot 0), the
/// others in race order from slot 1, the riders before and after the focus (slots 10, 11), and each rider by its
/// character (+0x464 → +0x38) in slots 14..25
pub fn set_focus_rider(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32, rider: u32) {
    if cam != MAIN {
        set_focus_rider(m, env, MAIN, rider);
    }
    m.w32(cam + 0x4e8, rider);
    for k in 0..12 {
        m.w32(cam + 0x6a8 + 0x20 * k, 0);
    }
    m.w32(cam + 0x648, 0);
    m.w32(cam + 0x668, 0);
    let mut next = cam + 0x508;
    let mut i = 0i32;
    while i < riders(m) {
        let r = env.rider(i as u32);
        if r != m.r32(cam + 0x4e8) {
            let r = env.rider(i as u32);
            m.w32(next, r);
            next += 0x20;
        } else {
            if i > 0 {
                let r = env.rider((i - 1) as u32);
                m.w32(cam + 0x648, r);
            }
            if i < riders(m) - 1 {
                let r = env.rider((i + 1) as u32);
                m.w32(cam + 0x668, r);
            }
        }
        for s in 14..26u32 {
            let r = env.rider(i as u32);
            if m.r32(m.r32(r + 0x464) + 0x38) == s - 14 {
                let r = env.rider(i as u32);
                m.w32(cam + 0x4e8 + s * 0x20, r);
                break;
            }
        }
        i += 1;
    }
}

/// `Camera_GetFocusRider` 0x174008
pub fn focus_rider(m: &Mem, cam: u32) -> u32 {
    m.r32(cam + 0x4e8)
}

/// `Camera_RiderSlotIndex` 0x173f60 (the slot): the race index of the slot's rider, or 0
pub fn rider_slot_index(m: &Mem, env: &mut dyn CamCtlEnv, cam: u32, slot: u32) -> u32 {
    let mut i = 0;
    while i < riders(m) {
        let r = env.rider(i as u32);
        if m.r32(cam + slot.wrapping_mul(0x20) + 0x4e8) == r {
            return i as u32;
        }
        i += 1;
    }
    0
}

/// `Camera_GetFocusPoint` 0x173c18: the camera's +0x42c (the main camera's while the replay's camera is on)
pub fn focus_point(m: &Mem, cam: u32) -> u32 {
    if m.r32(REPLAY_MODE) != 0 { MAIN + 0x42c } else { cam + 0x42c }
}

/// `Camera_IsFocusRider` 0x193ce0 (the rider): whether the chase camera follows it (its +0x460 the index)
pub fn is_focus_rider(m: &mut Mem, env: &mut dyn CamCtlEnv, rider: u32) -> bool {
    if rider == 0 {
        return false;
    }
    let c = chase_params(m, env);
    m.r32(rider + 0x460) == c.rider
}

/// `Camera_ChaseIfIdle` 0x174c70: a cut to the chase unless a script runs or +0x1e4 is set
pub fn chase_if_idle(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32) {
    if m.r32(cam + 0x34) == 0 && m.r32(cam + 0x1e4) == 0 {
        env.cut_to(m, cam);
    }
}

/// `Camera_Reset` 0x173a98: the mode from the index, then the state cleared and the second record (+0x488..) from
/// the first (+0x42c..), in the game's order
pub fn reset(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32) {
    apply_mode_index(m, env, cam);
    let w = |m: &mut Mem, o: u32, v: u32| m.w32(cam + o, v);
    w(m, 0x20, 0);
    w(m, 8, 1);
    w(m, 0x14, 4);
    w(m, 4, 1);
    w(m, 0x444, 0x3fc00000);
    let v = m.r32(cam + 0x20);
    w(m, 0x484, v);
    for o in [0xc, 0xea8, 0xeb0, 0x18, 0x1c, 0x34, 0x38, 0x3c, 0x1e4, 0x210, 0x420, 0x44, 0xeb4, 0x48, 0xeac, 0x42c, 0x430, 0x434, 0x438, 0x440, 0x43c, 0x478, 0x47c] {
        w(m, o, 0);
    }
    let copies: [(u32, u32); 23] = [
        (0x488, 0x430),
        (0x48c, 0x434),
        (0x490, 0x438),
        (0x494, 0x43c),
        (0x498, 0x440),
        (0x49c, 0x444),
        (0x4a0, 0x448),
        (0x4a4, 0x44c),
        (0x4a8, 0x450),
        (0x4ac, 0x454),
        (0x4b0, 0x458),
        (0x4b4, 0x45c),
        (0x4c8, 0x470),
        (0x4b8, 0x460),
        (0x4cc, 0x474),
        (0x4bc, 0x464),
        (0x4c0, 0x468),
        (0x4c4, 0x46c),
        (0x4d0, 0x478),
        (0x4d4, 0x47c),
        (0x4d8, 0x480),
        (0x4dc, 0x42c),
        (0x4e0, 0x430),
    ];
    for (to, from) in copies {
        let v = m.r32(cam + from);
        w(m, to, v);
    }
    let v = m.r32(cam + 0x434);
    w(m, 0x4e4, v);
}

/// `CamControl_Reset` 0x173a88: the mode index 1 (behind), then `reset`
pub fn control_reset(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32) {
    m.w32(cam, 1);
    reset(m, env, cam);
}

/// `Camera_ResetRiderTrack` 0x1731f8 (a rider slot): its tracker from the rider (+0x4ad0.. the position, +0x1b0)
pub fn reset_rider_track(m: &mut Mem, slot: u32) {
    let r = m.r32(slot);
    if r == 0 {
        return;
    }
    m.w32(slot + 0x14, m.r32(r + 0x4ad0));
    m.w32(slot + 0x18, m.r32(r + 0x4ad4));
    m.w32(slot + 0x1c, m.r32(r + 0x4ad8));
    m.w32(slot + 4, m.r32(m.r32(slot) + 0x1b0));
    m.w32(slot + 8, 0);
    m.w32(slot + 0xc, 0);
    m.w32(slot + 0x10, 0);
}

fn copy_slots(m: &mut Mem, cam: u32, to: u32) {
    for k in 0..26 {
        for j in 0..8 {
            let a = cam + 0x4e8 + 0x20 * k + 4 * j;
            m.w32(a + to, m.r32(a));
        }
    }
}

/// `Camera_UpdateRiderTracks` 0x174010: the slots kept as last frame's, each rider tracked, slot 8 (+0x5e8) the
/// focus with its +0x57c0.. point, slots 4..19 reset from their riders, and the slots kept as the cut's while the
/// camera's +0xc is below 3
pub fn update_rider_tracks(m: &mut Mem, env: &mut dyn CamCtlEnv, cam: u32) {
    copy_slots(m, cam, 0x340);
    m.w32(cam + 0xea8, 1);
    let mut i = 0;
    while i < riders(m) {
        let c = m.r32(cam + 4);
        env.track_rider(m, cam + 0x4e8 + 0x20 * i as u32, c);
        i += 1;
    }
    let f = m.r32(cam + 0x4e8);
    if f != 0 {
        m.w32(cam + 0x5e8, f);
        m.w32(cam + 0x5fc, m.r32(f + 0x57c0));
        m.w32(cam + 0x600, m.r32(f + 0x57c4));
        m.w32(cam + 0x604, m.r32(f + 0x57c8));
    }
    for k in 0..16 {
        reset_rider_track(m, cam + 0x628 + 0x20 * k);
    }
    if (m.r32(cam + 0xc) as i32) < 3 {
        copy_slots(m, cam, 0x680);
    }
}

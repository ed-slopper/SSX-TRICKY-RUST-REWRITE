//! The boarder's small helpers (board row G1f), each the game's and checked in the function runner
//! (`tests/boarder_misc.rs`) with its callees answered and logged: `Boarder_PickIdleClip` 0x11d4e8,
//! `Boarder_HitCheckpoint` 0x11e6f8, `Boarder_StartResetAt` 0x119c90, `Boarder_SetStartState` 0x11a7d0,
//! `Boarder_UpdateMotion` 0x11c770, `Boarder_InitEaseRecords` 0x122f10, `Boarder_GatherWorldEmitters` 0x1166f0.
//! (The race-progress functions are in [`crate::progress`].)

use crate::jump::dot;

type V4 = [f32; 4];

/// What `Boarder_PickIdleClip` reads of the rider.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IdleRider {
    /// +0x428: 1 skips it
    pub state: u32,
    /// +0x150 velocity; +0x4aa0 forward, +0x4ab0 side, +0x4ac0 up (the board's frame)
    pub vel: V4,
    pub fwd: V4,
    pub side: V4,
    pub up: V4,
    /// stats+0x3c
    pub kind: i32,
}

/// `Boarder_PickIdleClip`: every 18th tick the clip the rider rests or glides in on layer 2, played (blend 5) if it is
/// not the one playing. Slow (speed under 138.9): 0x2f0, or 0x2f4 for the kinds 2 and 11 while layer 0 plays clip 0,
/// 0x1fc, 0x1fd or 0x1fe. Under 833.3 the side drift picks 0x2f1 (to the left) or 0x2f4. Faster: by the larger of the
/// speed along the up (above 611.1) and the forward (above 416.7) or the side: 0x2f3 / 0x2f8 up or down, 0x2f7 / 0x2f6
/// forward or back, else 0x2f5 / 0x2f2 right or left. `clip(layer)` is `AnimCtl_GetClip`; returns the clip played.
pub fn pick_idle_clip(r: &IdleRider, tick: i32, clip: &mut dyn FnMut(u32) -> i64) -> Option<u32> {
    if r.state == 1 || tick % 0x12 != 0 {
        return None;
    }
    let speed = (r.vel[0] * r.vel[0] + r.vel[1] * r.vel[1] + r.vel[2] * r.vel[2]).sqrt();
    let want: u32 = if speed < f32::from_bits(0x430a_e38e) {
        let mut w = 0x2f0;
        if (r.kind == 2 || r.kind == 0xb) && matches!(clip(0), 0 | 0x1fe | 0x1fd | 0x1fc) {
            w = 0x2f4;
        }
        w
    } else {
        let s = dot(r.vel, r.side);
        if speed < f32::from_bits(0x4450_5555) {
            if s < 0.0 { 0x2f1 } else { 0x2f4 }
        } else {
            let f = dot(r.vel, r.fwd);
            let u = dot(r.vel, r.up);
            let (fa, ua) = (f.abs(), u.abs());
            if fa < ua && f32::from_bits(0x4418_c71c) < ua {
                if u < 0.0 { 0x2f8 } else { 0x2f3 }
            } else if f32::from_bits(0x43d0_5555) < fa {
                if f < 0.0 { 0x2f6 } else { 0x2f7 }
            } else if s < 0.0 {
                0x2f2
            } else {
                0x2f5
            }
        }
    };
    if clip(2) == want as i64 {
        None
    } else {
        Some(want)
    }
}

/// What `Boarder_HitCheckpoint` does, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CheckpointCall {
    /// `Debug_Printf` 0x23d5d0 (the string at 0x3661d0): a checkpoint after the rider has finished
    Printf,
    /// FUN_00155c60(points, rider+0x5820, flag): the score object takes the points (1) or the time gap (0)
    Score(f32, u32),
    /// FUN_00233ee0(0x343050, rider): the checkpoint sound
    Sound,
}

/// `Boarder_HitCheckpoint`(rider, points) by the game mode (0x32de8c): show-off (3, 5) counts the checkpoint
/// (rider+0x20 += 1) and, when no other rider has as many, scores `points` and adds them to the race clock
/// (race+0x14), then the sound; the points race (2, 4) scores the time gap (`Boarder_GetTimeGap`), then the sound;
/// any other mode just the sound. `others` the other riders' counts (+0x20) in race order; returns the rider's new
/// count and the clock addition.
pub fn hit_checkpoint(mode: u32, finished: bool, count: &mut i32, others: &[i32], points: i32, time_gap: &mut dyn FnMut() -> i32, calls: &mut Vec<CheckpointCall>) -> Option<f32> {
    match mode {
        3 | 5 => {
            if finished {
                calls.push(CheckpointCall::Printf);
                return None;
            }
            *count += 1;
            if others.iter().any(|&o| o >= *count) {
                return None;
            }
            calls.push(CheckpointCall::Score(points as f32, 1));
            calls.push(CheckpointCall::Sound);
            Some(points as f32)
        }
        2 | 4 => {
            let gap = time_gap();
            calls.push(CheckpointCall::Score(gap as f32, 0));
            calls.push(CheckpointCall::Sound);
            None
        }
        _ => {
            calls.push(CheckpointCall::Sound);
            None
        }
    }
}

/// The fields `Boarder_StartResetAt`(a, b, rider, c) sets in the rider's controller (rider+0x5ae0): +0x820 = a,
/// +0x81c = b, +0x824 = c, then `Boarder_SetMotion`(4) and `Boarder_SetState`(0x16); only if the rider has not finished
/// (+0x418 negative) and `AIP_CanResetAt` allows it.
pub fn start_reset_at(finish: i32, can_reset: impl FnOnce() -> bool, a: f32, b: f32, c: u32) -> Option<(f32, f32, u32)> {
    if finish < 0 && can_reset() {
        Some((a, b, c))
    } else {
        None
    }
}

/// `Boarder_SetStartState`(rider, flag): the HUD item cleared, then state 8 for a nonzero flag, else 0x11.
pub fn start_state(flag: bool) -> u32 {
    if flag { 8 } else { 0x11 }
}

/// `Boarder_UpdateMotion`: the motion update that runs for the rider's motion (+0x424) and the offset of its object
/// in the boarder: 1 `AirMotion_Update` 0x108378 at +0x480, 2 `GroundMotion_Update` 0x10a0d8 at +0x4b0,
/// 3 `RailSlideMotion_Update` 0x10b0d0 at +0x4c0, 4 0x10afe0 at +0x4e0, 5 `WipeoutMotion_Update` 0x10c2f0 at
/// +0x4f0, 6 `GetUpMotion_Update` 0x1102c8 at +0x5f0; any other nothing.
pub fn motion_update(motion: u32) -> Option<(u32, u32)> {
    match motion {
        1 => Some((0x108378, 0x480)),
        2 => Some((0x10a0d8, 0x4b0)),
        3 => Some((0x10b0d0, 0x4c0)),
        4 => Some((0x10afe0, 0x4e0)),
        5 => Some((0x10c2f0, 0x4f0)),
        6 => Some((0x1102c8, 0x5f0)),
        _ => None,
    }
}

/// `Boarder_InitEaseRecords`: `EaseRecord_Init`(0) on the twelve records at these offsets of the boarder.
pub const EASE_RECORDS: [u32; 12] = [0x1e0, 0x1ec, 0x1f8, 0x204, 0x210, 0x21c, 0x228, 0x234, 0x240, 0x24c, 0x258, 0x274];

/// `Boarder_GatherWorldEmitters`: for the first rider (slot 0) only, `WorldEmitter_AddListener`-style call 0x22b370 for
/// each of the game's emitters 0..count with its position (the three floats at +0x20 of its 0x80 row at game+0x90).
pub fn gather_world_emitters(slot: u32, positions: &[[f32; 3]]) -> Vec<(usize, [f32; 3])> {
    if slot != 0 {
        return Vec::new();
    }
    positions.iter().copied().enumerate().collect()
}

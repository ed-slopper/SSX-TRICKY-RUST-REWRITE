//! The level scripts' world ops (board row E9d) as the game carries them out: op 2's effect spawn
//! (`TriggerOp_SpawnEffect` 0x13d130), op 24's teleport (`Boarder_TeleportToInstance` 0x11a098) and op 25's rail
//! switch (`RailMan_SetRail` 0x149030). Op 13's reset is `reset::request_reset` (row G1c3); op 8's sound
//! (`Sfx_ScriptSoundPlay` 0x2165c8) is the audio engine's (G10); ops 1 and 23 (`TriggerOp_SpawnCameraNode`,
//! `Cam_Shake`) are the camera's. Checked against the game in the function runner (`tests/worldops.rs`).

use crate::math::atan;

/// What op 2 makes (`TriggerOp_SpawnEffect` 0x13d130), by its sub-type p2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// 0: a `cEmitterNode` on the object (particles)
    Emitter,
    /// 1: a spline model (`SplineModelNode_Create`: objects riding spline p3, e.g. the ski lifts), when p3 ≠ −1
    SplineModel,
    /// 2: a `cCollideEmitterNode` from the rider, when the rider has room for one (`FUN_00148900`)
    CollideEmitter,
}

/// `TriggerOp_SpawnEffect`: nothing on an object whose controller is a dead node (type 5); `controller` is the
/// object's controller type (none: no object or no controller), `p2`, `p3` the op's words, `rider_has_room` what
/// [`collide_room`] says for the thread's rider.
pub fn spawn_effect(controller: Option<u32>, p2: i32, p3: i32, rider_has_room: bool) -> Option<Effect> {
    if controller == Some(5) {
        return None;
    }
    match p2 {
        1 if p3 != -1 => Some(Effect::SplineModel),
        0 => Some(Effect::Emitter),
        2 if rider_has_room => Some(Effect::CollideEmitter),
        _ => None,
    }
}

/// `FUN_00148900`: whether a rider may get another collide emitter: its count (0x31d498 + 4·slot) under 3 for the
/// human (+0x41c = 1), under 1 for the others.
pub fn collide_room(human: bool, count: i32) -> bool {
    if human {
        count < 3
    } else {
        count < 1
    }
}

/// `Boarder_TeleportToInstance` 0x11a098: where a rider lands when sent to an object (its matrix rows): 3 m to the
/// side by the rider's slot (slot 0 left, 1 left-forward, 2 forward, 3 right-forward, 4 right, others at the
/// object), facing the object's second row (yaw from it as `Math_Atan` gives it). Returns (yaw, position); the game
/// then places the rider there at rest (`Boarder_PlaceAt`) and cuts its camera.
pub fn teleport(rows: [[f32; 4]; 4], slot: u32) -> (f32, [f32; 4]) {
    let (ox, oy) = match slot {
        0 => (-1.0, 0.0),
        1 => (-1.0, 1.0),
        2 => (0.0, 1.0),
        3 => (1.0, 1.0),
        4 => (1.0, 0.0),
        _ => (0.0, 0.0),
    };
    // VU0: row0·x, + row1·y, + row2·0, + row3·0 (lane by lane), × 300, + row3
    let mul = |v: [f32; 4]| [0, 1, 2, 3].map(|k| ((rows[0][k] * v[0] + rows[1][k] * v[1]) + rows[2][k] * v[2]) + rows[3][k] * v[3]);
    let side = mul([ox, oy, 0.0, 0.0]).map(|v| v * 300.0);
    let pos = [0, 1, 2, 3].map(|k| rows[3][k] + side[k]);
    let fwd = mul([0.0, 1.0, 0.0, 0.0]);
    let (x, y) = (fwd[0], fwd[1]);
    let yaw = if x == 0.0 {
        if y == 0.0 {
            y
        } else if y < 0.0 {
            f32::from_bits(0xbfc9_0fdb)
        } else {
            f32::from_bits(0x3fc9_0fdb)
        }
    } else {
        let a = atan(y / x);
        if x < 0.0 {
            if 0.0 < y {
                a + f32::from_bits(0x4049_0fdb)
            } else {
                a - f32::from_bits(0x4049_0fdb)
            }
        } else {
            a
        }
    };
    (yaw, pos)
}

/// The rail manager (`cRailManNode`, 0x31d4b0): 128 slots of (spline, state) (+0x28; spline −1 free).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailMan(pub Vec<(i16, i16)>);

impl Default for RailMan {
    fn default() -> Self {
        RailMan(vec![(-1, 0); 128])
    }
}

impl RailMan {
    /// `RailMan_SetRail` 0x149030 (op 25): the spline's slot (or the first free one) takes the state, and the
    /// spline's live bit follows it (`RailMan_ApplyRailState`: segment +0x18 bit 0). Returns the slot, or `None` when
    /// all 128 hold other splines.
    pub fn set(&mut self, spline: i16, state: i16) -> Option<usize> {
        let mut free = None;
        for i in 0..128 {
            if free.is_none() && self.0[i].0 == -1 {
                free = Some(i);
            }
            if self.0[i].0 == spline {
                self.0[i].1 = state;
                return Some(i);
            }
        }
        let i = free?;
        self.0[i] = (spline, state);
        Some(i)
    }

    /// The live bit `RailMan_ApplyRailState` writes for slot `i` into its spline's flags word.
    pub fn apply(&self, i: usize, flags: u32) -> u32 {
        if self.0[i].1 == 0 {
            flags & 0xffff_fffe
        } else {
            flags | 1
        }
    }
}

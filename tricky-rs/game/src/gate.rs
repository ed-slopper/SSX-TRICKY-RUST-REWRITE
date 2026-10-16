//! The start gate exactly as the original runs it (board row G1c2): during the countdown the rider rocks (lean
//! rider+0x238 toward a target from the stick), the rhythm of the last rock sets the launch speed, and at GO the gate
//! launch pushes the rider out along the board. `tests/gate.rs` checks each function bit for bit against the original
//! in the function runner.

/// The rider fields the gate reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    /// the lean now, its rate, its target (rider+0x238, +0x23c, +0x240)
    pub lean: f32,
    pub lean_rate: f32,
    pub lean_target: f32,
    /// rider+0x1c0: the gate's start timer (`Boarder_GetGateStartTimer` 0x11e9d8, copied every tick)
    pub timer: f32,
    /// rider+0x1c8: the launch speed (cm/s)
    pub launch: f32,
    /// velocity (rider+0x150), the board's forward (rider+0x180), the motion (rider+0x424)
    pub vel: [f32; 4],
    pub board: [f32; 4],
    pub motion: u32,
    /// the start stat byte (stats+0x15)
    pub start_stat: u8,
}

/// The rock's own state (state+8 time held still, +0xc the lowest and +0x10 the highest lean reached).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rock {
    pub still: f32,
    pub low: f32,
    pub high: f32,
}

pub(crate) fn k(start_stat: u8) -> f32 {
    start_stat as f32 * 0.718_085_94 * 0.003_921_569 + 0.487_557_1
}

fn approach(now: f32, target: f32, step: f32) -> f32 {
    if target + step < now {
        now - step
    } else if now < target - step {
        now + step
    } else {
        target
    }
}

/// `GateAnticipateControl_Enter` 0x105f48
pub fn anticipate_enter(rock: &mut Rock) {
    *rock = Rock::default();
}

/// `GateAnticipateControl_Update` 0x105f58 with the pad word `pad`: the stick's low field (bits 0..5, /31) leans
/// forward (to the gate clip's marker `marker`, `FUN_0015fde0`(layer 1)), back (to 0) or centred (to half of it), at a
/// rate set by the start stat and the distance; `timer` is `Boarder_GetGateStartTimer`.
pub fn anticipate_update(pad: u32, marker: f32, timer: f32, rock: &mut Rock, r: &mut Rider) {
    let old = r.lean;
    let s = (((pad << 26) as i32) >> 26) as f32 / 31.0;
    let target = if 0.0 < s { marker } else if 0.0 <= s { marker * 0.5 } else { 0.0 };
    r.timer = timer;
    let d = (target - r.lean).abs();
    let d = if 0.214_661_8 <= d { d } else { 0.214_661_8 };
    let rate = k(r.start_stat) * 4.437_673 * d;
    r.lean_target = target;
    let step = rate * 0.016_666_668;
    r.lean_rate = rate;
    r.lean = approach(r.lean, target, step);
    if old == r.lean {
        rock.still += 0.016_666_668;
    } else {
        rock.still = 0.0;
        if old < r.lean {
            rock.high = r.lean;
        } else {
            rock.low = r.lean;
        }
    }
}

/// `GateAnticipateControl_Exit` 0x106140 (at GO): the launch speed. Leaning back (target under 0.5) and still moving
/// (held under 0.4 ticks): (high − low)·2024.8037·(1 − lean)/(held + 1), at least 555.5555 cm/s; else 555.5555.
pub fn anticipate_exit(rock: &Rock, r: &mut Rider) {
    let base = f32::from_bits(0x440a_e38e);
    r.launch = base;
    if r.lean_target < 0.5 && rock.still < 0.006_666_667 {
        let v = (rock.high - rock.low) * 2_024.803_7 * (1.0 - r.lean) / (rock.still + 1.0);
        r.launch = v;
        if v < 555.555_54 {
            r.launch = base;
        }
    }
}

/// `GateLaunchControl_Enter` 0x106228: lean to 1 at the start stat's base rate.
pub fn launch_enter(r: &mut Rider) {
    r.lean_target = 1.0;
    r.lean_rate = k(r.start_stat) * 4.437_673 * 0.214_661_8;
}

/// What `GateLaunchControl_Update` does besides its field writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaunchCalls {
    /// `Boarder_SetMotion`(2) when pushed off while standing, and again on reaching the end lean if not in motion 2
    pub pushed: bool,
    pub ride: bool,
}

/// `GateLaunchControl_Update` 0x1062b0: lean on; while standing still, once the lean passes the launch clip's marker
/// (`marker`, `FUN_0015fde0`(layer 2)) the rider is pushed along the board at the launch speed × the start stat; at a
/// full lean the riding clip 0x1fd plays and the state goes to 7.
pub fn launch_update(marker: f32, timer: f32, r: &mut Rider) -> LaunchCalls {
    let mut c = LaunchCalls::default();
    let step = r.lean_rate * 0.016_666_668;
    r.lean = approach(r.lean, r.lean_target, step);
    let v = r.vel;
    let p = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let speed = (((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]).sqrt();
    if speed == 0.0 && marker <= r.lean {
        let push = k(r.start_stat) * r.launch;
        r.vel = r.board.map(|x| x * push);
        c.pushed = true;
    }
    if r.lean == 1.0 {
        c.ride = true;
    }
    r.timer = timer;
    c
}

//! The level scripts' threads as the game runs them (board row E9b): `TriggerScript_Run` 0x13be78 (also
//! `cScriptNode::Update` 0x13bd38, the same code) and `TriggerScript_ExecOp` 0x13bfd0, on the ops of the course's
//! `.ssf` ([`tricky_data::ssf`]). A thread is a `cScriptNode` (0xf0 bytes): +0x38 the op it is on, +0x3c that op's
//! byte offset, +0x40 the seconds left to wait, +0xe0 its script (an effect header or a function), +0xe4 the object it
//! runs on, +0xe8 the rider, +0x50 / +0x90 two 0x40-byte contexts (+0xd0 / +0xd4: present). Its constructors
//! (0x13b8c0 effect on an object, 0x13bc78 function by index, 0x13b978 function by name) set it up and run it at once;
//! then the world updates it once a frame until it ends. Checked op by op in the function runner (`tests/script.rs`).
//!
//! What the ops do beyond the thread (controllers, pickups, sounds, camera, rails, new threads) goes to
//! [`ScriptEnv`] as the game's calls, in the game's order.

use tricky_data::ssf::Op;

/// Which script a thread runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptRef {
    /// an effect header (`FUN_0025feb8`)
    Effect(u32),
    /// a function by index (`Course_GetScriptEntry`)
    Function(u32),
}

/// A context the thread carries (0x40 bytes, copied into each thread it starts).
pub type Context = [u32; 16];

/// One script thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub script: ScriptRef,
    /// +0x38, +0x3c, +0x40
    pub index: i32,
    pub offset: u32,
    pub wait: f32,
    /// +0xe4: the object (an instance address or handle), 0 none
    pub object: u32,
    /// +0xe8: the rider, 0 none
    pub rider: u32,
    pub ctx_a: Option<Context>,
    pub ctx_b: Option<Context>,
}

impl Thread {
    /// A new thread before its first run (the constructors' fields).
    pub fn new(script: ScriptRef, object: u32, rider: u32, ctx_a: Option<Context>, ctx_b: Option<Context>) -> Thread {
        Thread { script, index: 0, offset: 0, wait: 0.0, object, rider, ctx_a, ctx_b }
    }
}

/// The calls the ops make, in the game's order, with their arguments.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptCall {
    /// op 0: `TriggerScript_Op0_SpawnController` 0x13c5d0(thread, op)
    SpawnController(u32),
    /// op 1: `TriggerOp_SpawnCameraNode` 0x13d0b8(thread, op)
    SpawnCamera(u32),
    /// op 2: `TriggerOp_SpawnEffect` 0x13d130(thread, op)
    SpawnEffect(u32),
    /// ops 3, 9: the controller's vf 0xc4(controller, p2, p3 as float)
    ControllerMessage { controller: u32, p2: u32, p3: u32 },
    /// op 10: the controller's vf 0xb4(controller, 0 or 1)
    ControllerSwitch { controller: u32, on: u32 },
    /// op 11 with p2 0: the controller's vf 0xbc(controller)
    ControllerXbc(u32),
    /// op 6: `Pickup_Points` 0x11e608(rider, p2)
    PickupPoints { rider: u32, points: u32 },
    /// op 7: a new thread (`cScriptNode` 0x13b8c0) running effect p3 on the object, with this thread's rider and
    /// contexts; it runs at once
    StartEffect { effect: u32, object: u32, rider: u32, ctx_a: Option<Context>, ctx_b: Option<Context> },
    /// op 8: `Sfx_ScriptSoundPlay` 0x2165c8(audio, the object, p2)
    ScriptSound { object: u32, sound: u32 },
    /// op 13: rider+0x304 = 1, then `Boarder_RequestReset` 0x118f10(rider, 101.46)
    ResetRider(u32),
    /// op 14: `Pickup_Multiplier` 0x11e660(p2 as float, rider)
    PickupMultiplier { rider: u32, value: u32 },
    /// op 15: `Pickup_Meter` 0x11e958
    PickupMeter { rider: u32, value: u32 },
    /// op 16: `Pickup_Time` 0x11e8a0
    PickupTime { rider: u32, value: u32 },
    /// op 17: `Boarder_AddSpeedBoostTime` 0x11e910, then `Speech_OnScriptCue47` 0x2340b8(audio, rider)
    SpeedBoost { rider: u32, value: u32 },
    SpeechSpeedBoost(u32),
    /// op 18: `Boarder_AddSpinBoostTime` 0x11e930, then `Speech_OnScriptCue48` 0x234308(audio, rider)
    SpinBoost { rider: u32, value: u32 },
    SpeechSpinBoost(u32),
    /// ops 21 and 26: a new thread (`cScriptNode` 0x13bc78) running function p2 (op 26: no rider, no contexts)
    StartFunction { function: u32, rider: u32, ctx_a: Option<Context>, ctx_b: Option<Context> },
    /// op 23: `Cam_Shake` 0x177758(the rider's camera, p3, p2), when `Boarder_GetCamera` 0x11ca88 gives one
    CamShake { camera: u32, p3: u32, p2: u32 },
    /// op 24: `Boarder_TeleportToInstance` 0x11a098(rider, object p2)
    Teleport { rider: u32, object: u32 },
    /// op 25: `RailMan_SetRail` 0x149030(p2, p3)
    SetRail(u32, u32),
    /// the thread's end: its vf 0xc(3)
    End,
}

/// What the ops read from the world, and where their calls go.
pub trait ScriptEnv {
    /// the thread's ops (its script)
    fn ops(&self, s: ScriptRef) -> &[Op];
    /// `Rand_GameRange` 0x23d4f8(0, 1)
    fn rand01(&mut self) -> f32;
    /// the rider's +0x150 (velocity, four words)
    fn rider_velocity(&self, rider: u32) -> [f32; 4];
    /// the rider's +0x41c (nonzero: the human)
    fn rider_x41c(&self, rider: u32) -> i32;
    /// the object's controller (+0xe4), 0 none
    fn controller(&self, object: u32) -> u32;
    /// `Course_GetInstance` 0x254a28(world, index), 0 none
    fn instance(&self, index: u32) -> u32;
    /// `Boarder_GetCamera` 0x11ca88(rider), 0 none
    fn camera(&self, rider: u32) -> u32;
    fn call(&mut self, c: ScriptCall);
}

/// What `TriggerScript_ExecOp` returns: go on, wait (op 4), end the thread (a failed op 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Wait,
    Stop,
}

/// `|v|` as op 5 works it out on VU0: x², then + y², + z², + w² (one by one in the accumulator), square root.
fn speed(v: [f32; 4]) -> f32 {
    let sq = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let acc = sq[0] + sq[1];
    let acc = acc + 1.0 * sq[2];
    let s = acc + 1.0 * sq[3];
    s.sqrt()
}

/// `TriggerScript_ExecOp` 0x13bfd0: the thread's current op.
pub fn exec_op(t: &mut Thread, op: &Op, env: &mut impl ScriptEnv) -> Step {
    let p = |k: usize| op.args.get(k).copied().unwrap_or(0);
    let (p2, p3, p4) = (p(0), p(1), p(2));
    let f = f32::from_bits;
    let op_at = t.offset;
    let message = |env: &mut dyn FnMut(ScriptCall), ctrl: u32| {
        if ctrl != 0 {
            env(ScriptCall::ControllerMessage { controller: ctrl, p2, p3 });
        }
    };
    match op.op {
        0 => env.call(ScriptCall::SpawnController(op_at)),
        1 => env.call(ScriptCall::SpawnCamera(op_at)),
        2 => env.call(ScriptCall::SpawnEffect(op_at)),
        3 => {
            if t.object != 0 {
                let c = env.controller(t.object);
                message(&mut |c| env.call(c), c);
            }
        }
        4 => {
            t.wait = f(p2);
            return Step::Wait;
        }
        5 => match p2 as i32 {
            0 => {
                if t.rider == 0 {
                    return Step::Next;
                }
                let s = speed(env.rider_velocity(t.rider));
                let stop = match p3 {
                    0 => f(p4) < s,
                    1 => s < f(p4) * f32::from_bits(0x41de_38e3),
                    _ => return Step::Next,
                };
                if stop {
                    return Step::Stop;
                }
            }
            1 => match p3 {
                0 => {
                    if f(p4) < env.rand01() {
                        return Step::Stop;
                    }
                }
                1 => {
                    if env.rand01() < f(p4) {
                        return Step::Stop;
                    }
                }
                _ => {}
            },
            2 => {
                if env.rider_x41c(t.rider) == 0 {
                    return Step::Stop;
                }
            }
            3 => {
                if env.controller(t.object) != 0 {
                    return Step::Stop;
                }
            }
            _ => {}
        },
        6 => env.call(ScriptCall::PickupPoints { rider: t.rider, points: p2 }),
        7 => {
            let obj = env.instance(p2);
            if obj != 0 {
                env.call(ScriptCall::StartEffect { effect: p3, object: obj, rider: t.rider, ctx_a: t.ctx_a, ctx_b: t.ctx_b });
            }
        }
        8 => env.call(ScriptCall::ScriptSound { object: t.object, sound: p2 }),
        9 => {
            let c = env.controller(t.object);
            message(&mut |c| env.call(c), c);
        }
        10 => {
            if t.object != 0 {
                let c = env.controller(t.object);
                if c != 0 && p2 < 2 {
                    env.call(ScriptCall::ControllerSwitch { controller: c, on: p2 });
                }
            }
        }
        11 => {
            if t.object != 0 {
                let c = env.controller(t.object);
                if c != 0 && p2 == 0 {
                    env.call(ScriptCall::ControllerXbc(c));
                }
            }
        }
        13 => env.call(ScriptCall::ResetRider(t.rider)),
        14 => env.call(ScriptCall::PickupMultiplier { rider: t.rider, value: p2 }),
        15 => env.call(ScriptCall::PickupMeter { rider: t.rider, value: p2 }),
        16 => env.call(ScriptCall::PickupTime { rider: t.rider, value: p2 }),
        17 => {
            env.call(ScriptCall::SpeedBoost { rider: t.rider, value: p2 });
            env.call(ScriptCall::SpeechSpeedBoost(t.rider));
        }
        18 => {
            env.call(ScriptCall::SpinBoost { rider: t.rider, value: p2 });
            env.call(ScriptCall::SpeechSpinBoost(t.rider));
        }
        21 => {
            if (p2 as i32) >= 0 {
                env.call(ScriptCall::StartFunction { function: p2, rider: t.rider, ctx_a: t.ctx_a, ctx_b: t.ctx_b });
            }
        }
        23 => {
            if env.camera(t.rider) != 0 {
                let cam = env.camera(t.rider);
                env.call(ScriptCall::CamShake { camera: cam, p3, p2 });
            }
        }
        24 => {
            let obj = env.instance(p2);
            if obj != 0 {
                env.call(ScriptCall::Teleport { rider: t.rider, object: obj });
            }
        }
        25 => env.call(ScriptCall::SetRail(p2, p3)),
        26 => {
            if (p2 as i32) >= 0 {
                env.call(ScriptCall::StartFunction { function: p2, rider: 0, ctx_a: None, ctx_b: None });
            }
        }
        _ => {}
    }
    Step::Next
}

/// `TriggerScript_Run` 0x13be78: one frame of the thread. A wait counts down 1/60 s a frame and holds while it is
/// still ≥ 0; then the ops run until one waits (the thread steps past it and stays, unless that was its last op),
/// one stops it, or they run out. Returns true when the thread ended (its vf 0xc(3) called).
pub fn run(t: &mut Thread, env: &mut impl ScriptEnv) -> bool {
    if 0.0 < t.wait {
        let w = t.wait - f32::from_bits(0x3c88_8889);
        t.wait = w;
        if 0.0 <= w {
            return false;
        }
        t.wait = 0.0;
    }
    let ops = env.ops(t.script).to_vec();
    let count = ops.len() as i32;
    while t.index < count {
        let op = &ops[t.index as usize];
        let step = exec_op(t, op, env);
        if step == Step::Stop {
            env.call(ScriptCall::End);
            return true;
        }
        t.index += 1;
        t.offset = t.offset.wrapping_add(op.size);
        if step == Step::Wait {
            if t.index < count {
                return false;
            }
            break;
        }
    }
    env.call(ScriptCall::End);
    true
}

/// Where the world starts threads (each new thread runs at once, [`Thread::new`] then [`run`]):
///
/// - `World_CellActivate_RunPersistant` 0x25fed0, when an object's area wakes: for an object with no controller,
///   its event-0 script (`TriggerScript_Spawn` 0x13d538: no rider, no contexts); an object with a controller has
///   the controller restarted instead (E9e).
/// - `Controller_RunContactScript` 0x13aaf8, on a rider's contact: event 1, with the rider and two contexts.
/// - `Controller_RunFinishScript` 0x13a9f8, when a controller reaches its goal: event 3, no rider.
/// - `World_RunScript` 0x13bad8 (RaceMode, StartCountDown …): a function by name; the first running thread on that
///   function is ended first (`World_DeleteScriptObject` 0x13bbc0).
///
/// `event` is what `Instance_GetEventScript` gave ([`tricky_data::ssf::Ssf::event_script`]); the starters differ
/// in what they take as none: events 0 and 1 any value but −1, event 3 only values ≥ 0.
pub fn event_thread(event: u32, script: i32, object: u32, rider: u32, ctx_a: Option<Context>, ctx_b: Option<Context>) -> Option<Thread> {
    let start = match event {
        3 => script >= 0,
        _ => script != -1,
    };
    let (rider, ctx_a, ctx_b) = if event == 1 { (rider, ctx_a, ctx_b) } else { (0, None, None) };
    start.then(|| Thread::new(ScriptRef::Effect(script as u32), object, rider, ctx_a, ctx_b))
}

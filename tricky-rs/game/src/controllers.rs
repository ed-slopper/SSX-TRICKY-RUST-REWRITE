//! The level scripts' logic controllers (board row E9e): what op 0 puts on an object and how each behaves, as
//! the game's classes do. A controller is a node on the object (object+0xe4; the node's +0x28 is the object, +0x14
//! its type). Its virtuals: Update (once a frame), Kill (+0xbc), Finish(flag) (+0xb4: flag 1 runs the object's
//! event-3 script, `Controller_RunFinishScript` 0x13a9f8), OnMessage(value, kind) (+0xc4, ops 3 and 9), and the
//! deleting destructor (+0xc, 3). Checked in the function runner (`tests/controllers.rs`).
//!
//! - `cDebounceNode` (2): counts down `(int)(p3 · 60)` frames, then finishes.
//! - `cCounterNode` (6): a count of hits (p3, a half) taken from messages; at 0 it runs event 4 and ends; it also
//!   gives up after `(int)(p4 · 60)` frames.
//! - `cTimerNode` (8): runs event 4 and ends after `(int)(p4 · 60)` frames (message kind 1 sets it again), and
//!   finishes after `(int)(p3 · 60)`.
//! - `cTrickTriggerNode` (21, needs a rider): while its rider is inside the object's box, waits for it to land with
//!   `p4` more trick points than when it started; then runs event 4. Leaving or landing short finishes it.
//! - Type 5 (`TriggerScript_ApplyControllerOp` 0x13ac88), by p3: 0 restore the object's flags, 1 finish the
//!   controller, 2 / 3 a `cDeadNode` (hidden, no collision; 3 also sets flag 0x800), 4 a `cRestoreNode` (hidden until
//!   it is killed, which restores the flags).
//!
//! The object's flags (+0xe8): the upper half holds the file's defaults (the `.ssf` physics record), the lower the
//! live state; restoring copies the upper half down (`m = flags & mask; m | (m >> 16) | 2`).

/// What a controller calls outside itself, in the game's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlCall {
    /// a new script thread on the object (`TriggerScript_Spawn` 0x13d538 or `cScriptNode` 0x13b8c0): effect, object
    Start(i32, u32),
    /// the controller's deleting destructor (vf 0xc, 3); the object loses its controller
    Delete,
}

/// The world as a controller sees it.
pub trait CtrlEnv {
    /// the object's flags word (+0xe8)
    fn flags(&mut self, object: u32) -> &mut u32;
    /// `Instance_GetEventScript` 0x25fe78(world, object, event)
    fn event_script(&self, object: u32, event: u32) -> i32;
    /// for a trick trigger: whether `rider` is among the objects overlapping the trigger's box, and its +0x424 and
    /// +0x5910 (trick points) if so
    fn rider_in_box(&self, rider: u32) -> Option<(i32, i32)>;
    fn call(&mut self, c: CtrlCall);
}

/// The controller kinds this row ports, with their fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// +0x2c frames left
    Debounce { ticks: i32 },
    /// +0x34 hits still needed (a half), +0x36 the message bits taken, +0x38 frames left
    Counter { count: i16, bits: u16, life: i32 },
    /// +0x34 frames to finish, +0x38 frames to run event 4
    Timer { finish_in: i32, life: i32 },
    /// +0x34 (p3), +0x38 points needed, +0x3c the rider's points at the start (+0x5910), +0x40 the rider's +0x460,
    /// +0x48 frames left (once the rider is gone), +0x4c the rider (0: gone)
    TrickTrigger { x34: i32, need: i32, start: i32, x40: u32, ticks: i32, rider: u32 },
    /// type 5 (`cDeadNode`, flag 0x800 when p3 was 3)
    Dead,
    /// type 0x3ee
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    pub kind: Kind,
    pub object: u32,
}

/// `(int)(v · 60)` as the constructors work it out (`cvt.w.s`).
fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

/// The flags reset the game writes: the defaults in the upper half copied down, and 2.
pub fn restore_flags(flags: u32, mask: u32) -> u32 {
    let m = flags & mask;
    m | ((m as i32) >> 16) as u32 | 2
}

impl Controller {
    /// The type the node carries (+0x14).
    pub fn type_id(&self) -> u32 {
        match self.kind {
            Kind::Debounce { .. } => 2,
            Kind::Counter { .. } => 6,
            Kind::Timer { .. } => 8,
            Kind::TrickTrigger { .. } => 0x15,
            Kind::Dead => 5,
            Kind::Restore => 0x3ee,
        }
    }

    /// The constructors from op 0's words (`args[0]` the type, then p3, p4, p5 at op+0xc, +0x10, +0x14); for a trick
    /// trigger `rider` with its +0x460 and +0x5910.
    pub fn new(object: u32, args: &[u32], rider: Option<(u32, u32, i32)>) -> Option<Controller> {
        let a = |k: usize| args.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(a(k));
        let kind = match a(0) {
            2 => Kind::Debounce { ticks: frames(f(1)) },
            6 => Kind::Counter { count: a(1) as u16 as i16, bits: 0, life: frames(f(2)) },
            8 => Kind::Timer { finish_in: frames(f(1)), life: frames(f(2)) },
            0x15 => {
                let (r, x460, points) = rider?;
                Kind::TrickTrigger { x34: a(1) as u16 as i16 as i32, need: a(2) as i32, start: points, x40: x460, ticks: frames(f(3)), rider: r }
            }
            _ => return None,
        };
        Some(Controller { kind, object })
    }

    /// `cDeadNode::cDeadNode` 0x13af38: hidden and without collision (flags & 0xffffff0d | 4), | 0x800 when `stop`.
    pub fn dead(object: u32, stop: bool, env: &mut impl CtrlEnv) -> Controller {
        let f = env.flags(object);
        let v = *f & 0xffff_ff0d;
        *f = v | 4 | if stop { 0x800 } else { 0 };
        Controller { kind: Kind::Dead, object }
    }

    /// `cRestoreNode::cRestoreNode` 0x13aff8: hidden the same way, until killed.
    pub fn restore(object: u32, env: &mut impl CtrlEnv) -> Controller {
        let f = env.flags(object);
        *f = *f & 0xffff_ff0d | 4;
        Controller { kind: Kind::Restore, object }
    }

    /// The deleting destructor: `~cDeadNode` 0x13afb8 puts the dead flags back first; all detach from the object.
    pub fn delete(&mut self, env: &mut impl CtrlEnv) {
        if self.kind == Kind::Dead {
            let f = env.flags(self.object);
            *f = *f & 0xffff_ff0d | 4;
        }
        env.call(CtrlCall::Delete);
    }

    /// `Controller_RunFinishScript` 0x13a9f8: the object's event-3 script, if any; whether it ran one.
    fn finish_script(&self, env: &mut impl CtrlEnv) -> bool {
        let s = env.event_script(self.object, 3);
        if s >= 0 {
            env.call(CtrlCall::Start(s, self.object));
        }
        s >= 0
    }

    /// Update (vtable +0x10).
    pub fn update(&mut self, env: &mut impl CtrlEnv) {
        match self.kind {
            Kind::Debounce { ticks } => {
                if ticks > 0 {
                    self.kind = Kind::Debounce { ticks: ticks - 1 };
                    if ticks - 1 == 0 {
                        self.finish(1, env);
                    }
                }
            }
            Kind::Counter { count, bits, life } => {
                if count < 1 {
                    let s = env.event_script(self.object, 4);
                    if s >= 0 {
                        env.call(CtrlCall::Start(s, self.object));
                    }
                    self.finish(0, env);
                } else if life > 0 {
                    self.kind = Kind::Counter { count, bits, life: life - 1 };
                    if life - 1 == 0 {
                        self.finish(1, env);
                    }
                }
            }
            Kind::Timer { life, .. } => {
                let life = life.wrapping_sub(1);
                if let Kind::Timer { finish_in, .. } = self.kind {
                    self.kind = Kind::Timer { finish_in, life };
                }
                if life < 1 {
                    let s = env.event_script(self.object, 4);
                    if s != -1 {
                        env.call(CtrlCall::Start(s, self.object));
                    }
                    self.finish(0, env);
                }
                // the game goes on with the node it may just have deleted
                if let Kind::Timer { finish_in, life } = self.kind {
                    if finish_in > 0 {
                        self.kind = Kind::Timer { finish_in: finish_in - 1, life };
                        if finish_in - 1 == 0 {
                            self.finish(1, env);
                        }
                    }
                }
            }
            Kind::TrickTrigger { x34, need, start, x40, ticks, rider } => {
                if rider == 0 {
                    if ticks > 0 {
                        self.kind = Kind::TrickTrigger { x34, need, start, x40, ticks: ticks - 1, rider };
                        if ticks - 1 == 0 {
                            self.finish(0, env);
                        }
                    }
                    return;
                }
                if let Some((x424, points)) = env.rider_in_box(rider) {
                    if x424 == 1 {
                        return;
                    }
                    if need <= points.wrapping_sub(start) {
                        let s = env.event_script(self.object, 4);
                        if s != -1 {
                            env.call(CtrlCall::Start(s, self.object));
                            self.kind = Kind::TrickTrigger { x34, need, start, x40, ticks, rider: 0 };
                            return;
                        }
                    }
                }
                self.finish(1, env);
            }
            Kind::Dead | Kind::Restore => {}
        }
    }

    /// Kill (vtable +0xb8).
    pub fn kill(&mut self, env: &mut impl CtrlEnv) {
        match self.kind {
            Kind::Debounce { ticks: n } | Kind::Timer { finish_in: n, .. } => {
                if n >= 0 {
                    self.delete(env);
                }
            }
            Kind::Counter { life, .. } => {
                if life >= 0 {
                    let f = env.flags(self.object);
                    *f = restore_flags(*f, 0xffff_0000);
                    self.delete(env);
                }
            }
            // `cRestoreNode::Kill` 0x13a928, also the trick trigger's
            Kind::TrickTrigger { .. } | Kind::Restore => {
                let f = env.flags(self.object);
                *f = restore_flags(*f, 0xffff_0000);
                self.delete(env);
            }
            Kind::Dead => {}
        }
    }

    /// Finish(flag) (vtable +0xb0).
    pub fn finish(&mut self, flag: u32, env: &mut impl CtrlEnv) {
        match self.kind {
            Kind::Debounce { ticks: n } | Kind::Timer { finish_in: n, .. } => {
                if (flag == 0 || !self.finish_script(env)) && n >= 0 {
                    self.delete(env);
                }
            }
            // `cCounterNode::Finish` and `cControllerNode::Finish` 0x13a980 (the trick trigger's) are the same
            Kind::Counter { .. } | Kind::TrickTrigger { .. } => {
                if flag == 0 || !self.finish_script(env) {
                    let f = env.flags(self.object);
                    *f = restore_flags(*f, 0xffff_0100);
                    self.delete(env);
                }
            }
            Kind::Dead | Kind::Restore => {}
        }
    }

    /// OnMessage(value, kind) (vtable +0xc0): ops 3 and 9 send (p3 as a float, p2).
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl CtrlEnv) {
        match self.kind {
            Kind::Counter { count, bits, life } => {
                let bit: u16 = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0].iter().position(|v| *v == value).map_or(0, |i| 1 << i);
                let mut hit = false;
                let mut nb = bits;
                match kind {
                    1 => {
                        if bits & bit == 0 {
                            nb = bits | bit;
                            hit = true;
                        }
                    }
                    2 => {
                        if bits & bit == 0 {
                            if value == 1.0 {
                                nb = bits | 1;
                                hit = true;
                            } else if bits & ((bit as i16) >> 1) as u16 != 0 {
                                nb = bits | bit;
                                hit = true;
                            }
                        }
                    }
                    3 => hit = true,
                    _ => base_message(self.object, kind, env),
                }
                let count = if hit && count > 0 { count - 1 } else { count };
                self.kind = Kind::Counter { count, bits: nb, life };
            }
            Kind::Timer { finish_in, .. } if kind == 1 => self.kind = Kind::Timer { finish_in, life: frames(value) },
            Kind::Debounce { .. } | Kind::Timer { .. } | Kind::TrickTrigger { .. } => base_message(self.object, kind, env),
            Kind::Dead | Kind::Restore => {}
        }
    }
}

/// `cControllerNode::OnMessage` 0x13aa48: kind 7 clears the object's flag 0x800, kind 8 sets it.
fn base_message(object: u32, kind: u32, env: &mut impl CtrlEnv) {
    let f = env.flags(object);
    match kind {
        7 => *f &= 0xffff_f7ff,
        8 => *f |= 0x800,
        _ => {}
    }
}

/// What op 0 does (`TriggerScript_Op0_SpawnController` 0x13c5d0), in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spawn {
    /// the running controller's vf 0x74 (restart)
    Restart,
    /// delete the running controller
    DeleteOld,
    /// a `cMeshAnimNode` that takes over the old controller (type 20 replacing one), then the old is deleted
    MeshAnimReplacing,
    /// construct a controller of this type (5: `TriggerScript_ApplyControllerOp`)
    Create(u32),
}

/// `TriggerScript_Op0_SpawnController` 0x13c5d0: `old` is the object's controller (its type, and its vf 0x64 —
/// 0 for every class of this row), `new` the op's type, `rider` whether the thread has one.
pub fn spawn(old: Option<(u32, bool)>, new: u32, rider: bool) -> Vec<Spawn> {
    let create = |t: u32| -> Vec<Spawn> {
        match t {
            0 | 2 | 5..=0x14 | 0x16..=0x18 | 0x100..=0x103 => vec![Spawn::Create(t)],
            0x15 if rider => vec![Spawn::Create(t)],
            _ => vec![],
        }
    };
    let Some((ty, running)) = old else { return create(new) };
    if ty == 5 {
        return vec![];
    }
    if ty == new {
        return if running { vec![Spawn::Restart] } else { vec![] };
    }
    if new == 0x14 {
        return vec![Spawn::MeshAnimReplacing, Spawn::DeleteOld];
    }
    let mut v = vec![Spawn::DeleteOld];
    v.extend(create(new));
    v
}

/// What type 5 does (`TriggerScript_ApplyControllerOp` 0x13ac88) for mode p3, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Apply {
    DeleteOld,
    /// the object's flags restored (mask 0xffff0100)
    RestoreFlags,
    /// the controller's Finish(1)
    FinishOld,
    Dead { stop: bool },
    Restore,
}

/// `TriggerScript_ApplyControllerOp` 0x13ac88: `old` the object's controller type.
pub fn apply(old: Option<u32>, mode: u32) -> Vec<Apply> {
    let guard = |extra: u32| -> Option<Vec<Apply>> {
        match old {
            Some(t) if t == 5 || t == extra => None,
            Some(_) => Some(vec![Apply::DeleteOld]),
            None => Some(vec![]),
        }
    };
    match mode {
        0 => guard(5).map(|mut v| {
            v.push(Apply::RestoreFlags);
            v
        }),
        1 => Some(if old.is_some() { vec![Apply::FinishOld] } else { vec![] }),
        2 | 3 => guard(5).map(|mut v| {
            v.push(Apply::Dead { stop: mode == 3 });
            v
        }),
        4 => guard(0x3ee).map(|mut v| {
            v.push(Apply::Restore);
            v
        }),
        _ => Some(vec![]),
    }
    .unwrap_or_default()
}

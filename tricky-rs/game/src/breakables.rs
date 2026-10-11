//! The level scripts' breakables (board row E9g): `cCrackedNode` (type 14, glass and ice that crack before they
//! break) and `cFenceNode` (type 12, fences that swing when hit and pass the swing along), as the game runs them.
//! (`cMeshAnimNode`, 20, the falling pieces, is row E9g2.) Checked against the game's own objects in the function
//! runner (`tests/breakables.rs`).

use crate::controllers::{restore_flags, CtrlCall, CtrlEnv};

fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

/// A contact as the breakables read it (the thread's context, 0x40 bytes): +0x10 and +0x20 two vectors, +0x30 a
/// speed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub speed: f32,
}

impl Contact {
    /// From a script thread's context words.
    pub fn from_context(c: &crate::script::Context) -> Contact {
        let f = |k: usize| f32::from_bits(c[k]);
        Contact { a: [f(4), f(5), f(6), f(7)], b: [f(8), f(9), f(10), f(11)], speed: f(12) }
    }
}

/// x·y over four lanes in VU0's order.
fn dot(x: [f32; 4], y: [f32; 4]) -> f32 {
    let m = [x[0] * y[0], x[1] * y[1], x[2] * y[2], x[3] * y[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

/// What the breakables call outside themselves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BreakCall {
    Ctrl(CtrlCall),
    /// `TexFlip_Init` 0x142af0(the node's texture flip, the object, −1)
    TexFlipInit,
    /// `Controller_ReplaceWithDead` 0x13abf0: the node deleted and a dead node put on the object
    ReplacedWithDead,
    /// `FenceNode_UpdateShape` 0x143cd0
    FenceShape,
    /// a new fence node on the neighbour on `side` (1: +0x1b0, 0: +0x1b4) swinging by `swing` (`cFenceNode`
    /// 0x143740)
    NewFence { side: u32, swing: u32 },
    /// `Fence_Swing` 0x143b08 on the neighbour's fence node
    SwingNeighbour { side: u32, swing: u32 },
}

pub trait BreakEnv: CtrlEnv {
    fn breaks(&mut self, c: BreakCall);
    /// the fence's neighbour on `side`, as `Fence_SpawnWobble` sees it
    fn neighbour(&self, side: u32) -> Neighbour;
}

/// `cCrackedNode`: +0x34 frames to live (p3 seconds), +0x38 a 30-frame cooldown between hits, +0x3c health (p4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cracked {
    pub object: u32,
    pub ticks: i32,
    pub cooldown: i32,
    pub health: f32,
}

impl Cracked {
    /// `cCrackedNode::cCrackedNode` 0x145458: the starting hit (the thread's contact) is applied at once; the
    /// object's flags `& 0xfffffffd | 4`.
    pub fn new(object: u32, args: &[u32], contact: Option<&Contact>, env: &mut impl BreakEnv) -> Cracked {
        let a = |k: usize| args.get(k).copied().unwrap_or(0);
        let mut c = Cracked { object, ticks: frames(f32::from_bits(a(1))), cooldown: 30, health: f32::from_bits(a(2)) };
        c.apply_impact(contact);
        let f = env.flags(object);
        *f = *f & 0xffff_fffd | 4;
        env.breaks(BreakCall::TexFlipInit);
        c
    }

    /// `cCrackedNode::ApplyImpact` 0x145808: health −= |a·b · speed| · 0.036 (cm/s to km/h, 0x3d1374bd); the cooldown restarts.
    pub fn apply_impact(&mut self, contact: Option<&Contact>) {
        if let Some(c) = contact {
            if 0.0 < self.health {
                let d = dot(c.a, c.b);
                self.cooldown = 30;
                self.health -= (d * c.speed).abs() * f32::from_bits(0x3d13_74bd);
            }
        }
    }

    /// vtable +0x90 (a rider's hit, 0x1457e8): only once the cooldown is over.
    pub fn hit(&mut self, contact: Option<&Contact>) {
        if self.cooldown == 0 {
            self.apply_impact(contact);
        }
    }

    /// `cCrackedNode::Update` 0x1455a0. While its health is gone it starts the object's event-4 script every frame
    /// (or, without one, gives way to a dead node). Returns whether the node is gone.
    pub fn update(&mut self, env: &mut impl BreakEnv) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
        if self.ticks >= 1 {
            self.ticks -= 1;
            if self.ticks == 0 {
                return self.finish(1, env);
            }
        }
        if self.health <= 0.0 {
            let s = env.event_script(self.object, 4);
            if s == -1 {
                env.breaks(BreakCall::ReplacedWithDead);
                return true;
            }
            env.call(CtrlCall::Start(s, self.object));
        }
        false
    }

    /// Finish 0x1456c0; returns whether the node was deleted.
    pub fn finish(&mut self, flag: u32, env: &mut impl BreakEnv) -> bool {
        let ran = flag != 0 && {
            let s = env.event_script(self.object, 3);
            if s >= 0 {
                env.call(CtrlCall::Start(s, self.object));
            }
            s >= 0
        };
        if !ran && self.ticks >= 0 {
            let f = env.flags(self.object);
            *f = restore_flags(*f, 0xffff_0100);
            env.call(CtrlCall::Delete);
            return true;
        }
        false
    }

    /// Kill 0x145740; returns whether the node was deleted.
    pub fn kill(&mut self, env: &mut impl BreakEnv) -> bool {
        if self.ticks >= 0 {
            let f = env.flags(self.object);
            *f = restore_flags(*f, 0xffff_0000);
            env.call(CtrlCall::Delete);
            return true;
        }
        false
    }

    /// OnMessage 0x1457a8: kind 1 sets the time to live, kind 2 the health; nothing else (not the base's either).
    pub fn on_message(&mut self, value: f32, kind: u32) {
        match kind {
            1 => self.ticks = frames(value),
            2 => self.health = value,
            _ => {}
        }
    }
}

/// The fence's neighbour on one side, as `Fence_SpawnWobble` 0x143928 sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Neighbour {
    /// none on that side, or the fence has no shape (+0x54 = 0)
    None,
    /// a neighbour with no controller
    Free,
    /// a neighbour whose controller is a fence node (type 12)
    Fence,
    /// a neighbour with another controller
    Other,
}

/// `cFenceNode`: +0x34 the angle, +0x38 the swing's size (at most 1), +0x3c its speed, +0x40 / +0x44 frames until
/// the swing is passed to the neighbour on side 1 (+0x1b0) / side 0 (+0x1b4). The bending of its mesh
/// (`FenceNode_InitShape`, `FenceNode_UpdateShape`, +0x54) is drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fence {
    pub object: u32,
    pub angle: f32,
    pub size: f32,
    pub speed: f32,
    pub pass1: i32,
    pub pass0: i32,
}

impl Fence {
    /// `FenceNode_AddSwing` 0x1444d0: the size grows by |by| (to 1); the speed becomes a tenth of the size, in the
    /// direction it was going (from rest, the direction of `by`).
    pub fn add_swing(&mut self, by: f32) {
        self.size += by.abs();
        if 1.0 < self.size {
            self.size = 1.0;
        }
        let neg = if self.speed == 0.0 { by < 0.0 } else { self.speed < 0.0 };
        self.speed = if neg { self.size * -0.1 } else { self.size * 0.1 };
    }

    /// `Fence_Swing` 0x143b08: a neighbour's swing arriving from `side`; it is passed on in 4 frames.
    pub fn swing(&mut self, by: f32, side: u32) {
        self.add_swing(by);
        if side == 0 {
            if self.pass0 == 0 {
                self.pass0 = 4;
            }
        } else if self.pass1 == 0 {
            self.pass1 = 4;
        }
    }

    /// The script's constructor 0x143550, from the thread's contact: the swing is a·b (without a shape) or the
    /// object's first matrix row, normalised, ·b (with one) × |speed| / 1000; it reaches both neighbours at once.
    pub fn new(object: u32, contact: &Contact, row0: Option<[f32; 4]>, env: &mut impl BreakEnv) -> Fence {
        let mut f = Fence { object, angle: 0.0, size: 0.0, speed: 0.0, pass1: 0, pass0: 0 };
        let by = match row0 {
            None => dot(contact.b, contact.a) * (contact.speed.abs() * 0.001),
            Some(r) => {
                let s = dot(r, r);
                let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
                dot(r.map(|v| v * q), contact.a) * (contact.speed.abs() * 0.001)
            }
        };
        f.add_swing(by);
        f.spawn_wobble(1, env);
        f.spawn_wobble(0, env);
        f
    }

    /// The neighbour's constructor 0x143740: swinging by `by`, passing it on to both sides in 4 frames.
    pub fn from_neighbour(object: u32, by: f32) -> Fence {
        let mut f = Fence { object, angle: 0.0, size: 0.0, speed: 0.0, pass1: 0, pass0: 0 };
        f.add_swing(by);
        f.pass0 = 4;
        f.pass1 = 4;
        f
    }

    /// `Fence_SpawnWobble` 0x143928: 0.4 × the size, against the direction of the speed, reaches the neighbour on
    /// `side` when it is at least 0.25: a free neighbour gets a fence node, a fence node gets the swing.
    pub fn spawn_wobble(&mut self, side: u32, env: &mut impl BreakEnv) {
        let n = env.neighbour(side);
        if n == Neighbour::None {
            return;
        }
        let by = self.size * if self.speed < 0.0 { 0.4 } else { -0.4 };
        if by.abs() < 0.25 {
            return;
        }
        match n {
            Neighbour::Free => env.breaks(BreakCall::NewFence { side, swing: by.to_bits() }),
            Neighbour::Fence => env.breaks(BreakCall::SwingNeighbour { side, swing: by.to_bits() }),
            _ => {}
        }
    }

    /// vtable +0x90 (a rider's hit, 0x144428): while the size is under 1, a swing of b·a · speed / 1000, and the
    /// passing on restarted.
    pub fn hit(&mut self, c: &Contact) {
        if self.size < 1.0 {
            self.add_swing(dot(c.b, c.a) * (c.speed * 0.001));
            if self.pass1 == 0 {
                self.pass1 = 4;
            }
            if self.pass0 == 0 {
                self.pass0 = 4;
            }
        }
    }

    /// `cFenceNode::Update` 0x143b68: passes the swing on, moves the angle and bounces it back at half the size at
    /// each end; at rest (size and angle under 0.01) Finish(1) (`cControllerNode::Finish`). Returns whether the node
    /// was deleted.
    pub fn update(&mut self, env: &mut impl BreakEnv) -> bool {
        if self.pass1 > 0 {
            self.pass1 -= 1;
            if self.pass1 == 0 {
                self.spawn_wobble(1, env);
            }
        }
        if self.pass0 > 0 {
            self.pass0 -= 1;
            if self.pass0 == 0 {
                self.spawn_wobble(0, env);
            }
        }
        let size = self.size;
        let a = self.angle + self.speed;
        self.angle = a;
        if size < 0.01 && a.abs() < 0.01 {
            let s = env.event_script(self.object, 3);
            if s >= 0 {
                env.call(CtrlCall::Start(s, self.object));
                return false;
            }
            let f = env.flags(self.object);
            *f = restore_flags(*f, 0xffff_0100);
            env.call(CtrlCall::Delete);
            return true;
        }
        if 0.0 <= self.speed {
            if size <= a {
                self.angle = size;
                let h = -(size * 0.5);
                self.size = size * 0.5;
                self.speed = h * 0.1;
            }
        } else if a <= -size {
            let h = size * 0.5;
            self.angle = -size;
            self.size = h;
            self.speed = h * 0.1;
        }
        env.breaks(BreakCall::FenceShape);
        false
    }
}

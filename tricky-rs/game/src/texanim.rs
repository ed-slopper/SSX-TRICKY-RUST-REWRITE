//! The level scripts' texture animation controllers (board row E9i) as the game runs them: `cUVScrollNode`
//! (type 10, scrolling textures), `cTexFlipNode` (11, flip-books) and `cUVScrollTexFlipNode` (19, both), with the
//! flip-book helper `cTexFlip` (`TexFlip_SetFrame` 0x142c78) and the scrolled-model registry
//! (`UVScrollNode_Register` 0x142220). Drawing the result (the texture offsets and frames) is rendering. Checked
//! against the game's own objects in the function runner (`tests/texanim.rs`).

use crate::controllers::{CtrlCall, CtrlEnv};

/// What these controllers ask of the world besides [`CtrlEnv`].
pub trait AnimEnv: CtrlEnv {
    /// `Rand_Fx` 0x23d4a8: a random word
    fn rand_fx(&mut self) -> u32;
    /// `Controller_RunContactScript` 0x13aaf8 (the rider's contact while the node is not cooling down)
    fn contact_script(&mut self);
}

/// The random word as a float in [0, 1) the way the game makes it.
fn unit(r: u32) -> f32 {
    f32::from_bits(r & 0x7f_ffff | 0x3f80_0000) - 1.0
}

/// `(int)` of the float rounded up, as the game writes it (truncate, then +1 when below).
fn ceil_int(v: f32) -> i32 {
    let t = v as i32 as f32;
    (if t < v { t + 1.0 } else { t }) as i32
}

fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

const SIXTIETH: u32 = 0x3c88_8889;

/// `cTexFlip`: +4 the frame shown, +0xc the frames the object's textures have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TexFlip {
    pub frame: i32,
    pub count: i32,
}

impl TexFlip {
    /// `TexFlip_SetFrame` 0x142c78: a negative frame picks one at random (1 with three or fewer frames); clamped to
    /// the last frame.
    pub fn set_frame(&mut self, f: i32, env: &mut impl AnimEnv) {
        let mut f = f;
        if f < 0 {
            f = 1;
            let n = self.count;
            if 2 < n {
                let r = env.rand_fx();
                let v = (n as f32 - 1.0) * (f32::from_bits(r & 0x7f_ffff | 0x3f80_0000) - 1.0) + 1.0;
                f = ceil_int(v);
            }
        }
        if self.count <= f {
            f = self.count - 1;
        }
        self.frame = f;
    }
}

/// The flip-book stepping shared by `cTexFlipNode` and `cUVScrollTexFlipNode`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flip {
    /// 0 steady (or random each step when `random`), 1 / 2 a flicker (alternating a fast step and a random-length
    /// one)
    pub mode: i32,
    pub backwards: u32,
    pub frame: i32,
    pub acc: f32,
    pub rate: f32,
    pub base_rate: f32,
    pub random: u32,
    pub tex: TexFlip,
}

impl Flip {
    /// From op words `start, backwards, rate (frames a second), mode`; a start below 0 picks one of the first eight
    /// frames at random; −2 also picks at random at every step.
    fn new(start: u32, backwards: u32, rate: u32, mode: u32, count: i32, env: &mut impl AnimEnv) -> Flip {
        let r = f32::from_bits(rate) * f32::from_bits(SIXTIETH);
        let mut frame = start as i32;
        let acc = 0.0f32;
        if (frame as f32) < acc {
            let u = env.rand_fx();
            frame = ceil_int(unit(u) * 8.0 + acc);
        }
        let random = ((start as i32) as f32 == -2.0) as u32;
        let mut tex = TexFlip { frame: 0, count };
        tex.set_frame(frame, env);
        Flip { mode: mode as i32, backwards, frame, acc, rate: r, base_rate: r, random, tex }
    }

    fn random_frame(&mut self, env: &mut impl AnimEnv) -> bool {
        let n = self.tex.count;
        if 1 < n {
            let u = unit(env.rand_fx());
            self.frame = ceil_int((((n - 1) as f32) - 0.0) * u + 0.0);
            return true;
        }
        false
    }

    /// One frame of flipping (the part of the two Updates after the timers).
    fn step(&mut self, env: &mut impl AnimEnv) {
        if self.rate <= 0.0 {
            return;
        }
        let a = self.acc + self.rate;
        self.acc = a;
        if a < 1.0 {
            return;
        }
        self.acc = a - 1.0;
        let n = self.tex.count;
        match self.mode {
            0 => {
                let picked = self.random != 0 && self.random_frame(env);
                if !picked {
                    if self.backwards == 0 {
                        self.frame += 1;
                        if n <= self.frame {
                            self.frame = 0;
                        }
                    } else {
                        self.frame -= 1;
                        if self.frame < 0 {
                            self.frame = n - 1;
                        }
                    }
                }
            }
            1 => {
                self.mode = 2;
                self.frame += 1;
                self.rate = f32::from_bits(0x3e2a_aaab);
                if n <= self.frame {
                    self.frame = 0;
                }
            }
            2 => {
                let u = unit(env.rand_fx());
                self.frame += 1;
                self.mode = 1;
                self.rate = self.base_rate * (u * 0.75 + 0.25);
                if n <= self.frame {
                    self.frame = 0;
                }
            }
            _ => {}
        }
        let f = self.frame;
        self.tex.set_frame(f, env);
    }
}

/// `cTexFlipNode` (11): +0x34.. the flip (mode +0x34, backwards +0x4c, frame +0x48, acc +0x3c, rate +0x40, base
/// +0x44, random +0x54, the `cTexFlip` at +0x58), +0x38 frames to live, +0x50 the contact cooldown, +0x2d0 shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexFlipNode {
    pub object: u32,
    pub flip: Flip,
    pub life: i32,
    pub cooldown: i32,
    pub shown: u32,
}

impl TexFlipNode {
    /// `cTexFlipNode::cTexFlipNode` 0x142e90: op words start, backwards, rate, life (s), mode; `count` the object's
    /// texture frames. The object's flags `& ~2 | 4`.
    pub fn new(object: u32, a: &[u32], count: i32, env: &mut impl AnimEnv) -> TexFlipNode {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let life = frames(f32::from_bits(w(4)));
        let flip = Flip::new(w(1), w(2), w(3), w(5), count, env);
        let f = env.flags(object);
        *f = *f & 0xffff_fffd | 4;
        TexFlipNode { object, flip, life, cooldown: 30, shown: 1 }
    }

    /// Update 0x1430b8.
    pub fn update(&mut self, env: &mut impl AnimEnv) {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
        if self.life > 0 {
            self.life -= 1;
            if self.life == 0 {
                finish(self.object, env);
                return;
            }
        }
        self.flip.step(env);
    }

    /// OnContact 0x143378.
    pub fn on_contact(&mut self, env: &mut impl AnimEnv) {
        if self.cooldown < 1 {
            env.contact_script();
            self.cooldown = 30;
        }
    }

    /// OnMessage 0x1433d8: 1 the rate, 2 a frame (0..9, rounded up), 3 the next frame (wrapping to 1), 4 shown;
    /// others the base's.
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl AnimEnv) {
        match kind {
            2 => {
                if 0.0 <= value && value < 9.0 {
                    let f = ceil_int(value);
                    self.flip.frame = f;
                    self.flip.tex.set_frame(f, env);
                }
            }
            1 => self.flip.rate = value,
            3 => {
                self.flip.frame += 1;
                if self.flip.tex.count <= self.flip.frame {
                    self.flip.frame = 1;
                }
                let f = self.flip.frame;
                self.flip.tex.set_frame(f, env);
            }
            4 => self.shown = (value != 0.0) as u32,
            _ => base_message(self.object, kind, env),
        }
    }
}

/// `cControllerNode::Finish`(1) 0x13a980 (what these nodes inherit).
fn finish(object: u32, env: &mut impl AnimEnv) {
    let s = env.event_script(object, 3);
    if s >= 0 {
        env.call(CtrlCall::Start(s, object));
        return;
    }
    let f = env.flags(object);
    *f = crate::controllers::restore_flags(*f, 0xffff_0100);
    env.call(CtrlCall::Delete);
}

fn base_message(object: u32, kind: u32, env: &mut impl AnimEnv) {
    let f = env.flags(object);
    match kind {
        7 => *f &= 0xffff_f7ff,
        8 => *f |= 0x800,
        _ => {}
    }
}

/// The scrolled-model registry (0x31d938, 128 entries of model, users, a third word): `UVScrollNode_Register`
/// gives a node the entry of its model (a new one in the first free slot), counting its users.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UvRegistry(pub Vec<[u32; 3]>);

impl Default for UvRegistry {
    fn default() -> Self {
        UvRegistry(vec![[0; 3]; 128])
    }
}

impl UvRegistry {
    /// `UVScrollNode_Register` 0x142220: the slot, or −1 when all 128 hold other models.
    pub fn register(&mut self, model: u32) -> i32 {
        let mut slot = -1i32;
        let mut i;
        if self.0[0][0] == 0 {
            slot = 0;
            i = 1;
            // the game goes on looking for its model after taking slot 0 as the free one
            while i < 128 {
                if self.0[i][0] == model && self.0[i][0] != 0 {
                    slot = i as i32;
                    break;
                }
                i += 1;
            }
        } else if self.0[0][0] == model {
            slot = 0;
        } else {
            i = 1;
            while i < 128 {
                if self.0[i][0] == 0 {
                    if slot < 0 {
                        slot = i as i32;
                    }
                } else if self.0[i][0] == model {
                    slot = i as i32;
                    break;
                }
                i += 1;
            }
        }
        if slot >= 0 {
            let e = &mut self.0[slot as usize];
            e[0] = model;
            e[1] = e[1].wrapping_add(1);
        }
        slot
    }
}

/// `cUVScrollNode` (10): +0x34 mode (1: eased back and forth, 2: back and forth), +0x38 frames to live, +0x3c / +0x40
/// the offsets, +0x44 the phase clock, +0x48 moving, +0x4c the contact cooldown, +0x54 / +0x58 the step, +0x5c the
/// moving time, +0x60 the pause, +0x64 the model's vertex count, +0x68 its registry slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvScroll {
    pub object: u32,
    pub mode: i32,
    pub life: i32,
    pub u: f32,
    pub v: f32,
    pub clock: f32,
    pub moving: u32,
    pub cooldown: i32,
    pub du: f32,
    pub dv: f32,
    pub run: f32,
    pub pause: f32,
    pub verts: i32,
    pub slot: i32,
}

impl UvScroll {
    /// `cUVScrollNode::cUVScrollNode` 0x141ff0 with `UVScrollNode_Init` 0x1421a8: op words mode, du, dv, run, pause,
    /// life (s); `verts` the sum of the model's parts' vertex counts; registered by `model`; the object's flags
    /// `& ~2 | 4`.
    pub fn new(object: u32, a: &[u32], verts: i32, model: u32, reg: &mut UvRegistry, env: &mut impl AnimEnv) -> UvScroll {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let mut s = UvScroll {
            object,
            mode: w(1) as i32,
            life: frames(f(6)),
            u: 0.0,
            v: 0.0,
            clock: 0.0,
            moving: 1,
            cooldown: 30,
            du: f(2),
            dv: f(3),
            run: f(4),
            pause: f(5),
            verts,
            slot: -1,
        };
        let fl = env.flags(object);
        *fl = *fl & 0xffff_fffd | 4;
        s.slot = reg.register(model);
        s
    }

    /// The scrolling (Update after the timers).
    fn step(&mut self) {
        let r = self.run;
        if r <= 0.0 && self.pause <= 0.0 {
            return;
        }
        let t = self.clock + f32::from_bits(SIXTIETH);
        self.clock = t;
        if self.moving == 0 {
            if t < self.pause {
                return;
            }
            self.clock = 0.0;
            if r <= 0.0 {
                return;
            }
            self.moving = 1;
            return;
        }
        if r <= t {
            self.clock = 0.0;
            if 0.0 < self.pause {
                self.moving = 0;
            }
            if (self.mode as u32).wrapping_sub(1) < 2 {
                self.du = -self.du;
                self.dv = -self.dv;
            }
        }
        if self.moving == 0 {
            return;
        }
        if self.mode == 1 {
            let r = self.run;
            let t = if self.du < 0.0 { self.clock } else { r - self.clock };
            let k = if t < r * 0.5 { t } else { r - t };
            self.u += self.du * k * (1.0 / r);
            self.v += self.dv * k * (1.0 / r);
        } else {
            self.u += self.du;
            self.v += self.dv;
        }
        if 1.0 < self.u {
            self.u -= 1.0;
        } else if self.u < -1.0 {
            self.u += 1.0;
        }
        if 1.0 < self.v {
            self.v -= 1.0;
        } else if self.v < -1.0 {
            self.v += 1.0;
        }
    }

    /// Update 0x1422e0.
    pub fn update(&mut self, env: &mut impl AnimEnv) {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
        if self.life > 0 {
            self.life -= 1;
            if self.life == 0 {
                finish(self.object, env);
                return;
            }
        }
        self.step();
    }

    /// OnContact 0x142690.
    pub fn on_contact(&mut self, env: &mut impl AnimEnv) {
        if self.cooldown < 1 {
            env.contact_script();
            self.cooldown = 30;
        }
    }

    /// OnMessage 0x142948: 1 / 2 the step (−1..1), 3 / 4 the moving time and pause (0..60 s), 5 / 6 the offsets
    /// (−4..4); others the base's.
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl AnimEnv) {
        match kind {
            1 | 2 => {
                if -1.0 <= value && value <= 1.0 {
                    if kind == 1 {
                        self.du = value
                    } else {
                        self.dv = value
                    }
                }
            }
            3 | 4 => {
                if 0.0 <= value && value <= 60.0 {
                    if kind == 3 {
                        self.run = value
                    } else {
                        self.pause = value
                    }
                }
            }
            5 | 6 => {
                if -4.0 <= value && value <= 4.0 {
                    if kind == 5 {
                        self.u = value
                    } else {
                        self.v = value
                    }
                }
            }
            _ => base_message(self.object, kind, env),
        }
    }
}

/// `cUVScrollTexFlipNode` (19): a [`UvScroll`] (embedded at +0x24) and a [`Flip`] (+8.., the `cTexFlip` at +0x90)
/// from op words 8..11 (start, rate, mode, backwards). The flip has no timers of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvScrollFlip {
    pub scroll: UvScroll,
    pub flip: Flip,
}

impl UvScrollFlip {
    /// 0x1460f0.
    pub fn new(object: u32, a: &[u32], verts: i32, model: u32, count: i32, reg: &mut UvRegistry, env: &mut impl AnimEnv) -> UvScrollFlip {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let scroll = UvScroll::new(object, a, verts, model, reg, env);
        let flip = Flip::new(w(7), w(10), w(8), w(9), count, env);
        UvScrollFlip { scroll, flip }
    }

    /// Update 0x1462d0: the flip, then the scroll's Update.
    pub fn update(&mut self, env: &mut impl AnimEnv) {
        self.flip.step(env);
        self.scroll.update(env);
    }
}

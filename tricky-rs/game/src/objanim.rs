//! The level scripts' object animation controllers (board row E9i2) as the game runs them: `cFlagNode` (13, waving
//! flags), and the keyframed ones built on `cAnimObjectNode` (0x100): `cAnimDeltaNode` (0x101, plays by amounts
//! sent to it), `cAnimComboNode` (0x102, a second clip on top) and `cAnimTexFlipNode` (0x103, with a flip-book). Here
//! is the clip cursor and the timers; sampling the clip onto the object's parts (`MeshAnim_ApplyFrames` 0x199258,
//! `AnimComboNode_ApplyFrames` 0x19a478) and building the flag's strip are drawing, reached through [`ObjAnimEnv`].
//! Checked against the game's own objects in the function runner (`tests/objanim.rs`).

use crate::controllers::{restore_flags, CtrlCall, CtrlEnv};
use crate::texanim::{AnimEnv, TexFlip};

/// What these controllers call outside themselves besides [`CtrlEnv`] and [`AnimEnv`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjAnimCall {
    /// the node's vtable +0xe4: the clip sampled onto the object's parts
    ApplyFrames,
    /// `FUN_00198fc0`: the object's moving box refitted (+0x64)
    MoveBox,
    /// `cFlagNode::BuildStrip` 0x144ec8
    FlagStrip,
    /// the combo's second clip's frames copied for blending (message 3)
    ComboCopy,
}

pub trait ObjAnimEnv: AnimEnv {
    /// `Rand_Game` (through `Rand_GameRange` 0x23d4f8): a random word of the game stream
    fn rand_game(&mut self) -> u32;
    fn anim(&mut self, c: ObjAnimCall);
}

fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

fn unit(r: u32) -> f32 {
    f32::from_bits(r & 0x7f_ffff | 0x3f80_0000) - 1.0
}

fn ceil_int(v: f32) -> i32 {
    let t = v as i32 as f32;
    (if t < v { t + 1.0 } else { t }) as i32
}

const SIXTIETH: u32 = 0x3c88_8889;

/// `Rand_GameRange` 0x23d4f8.
fn game_range(lo: f32, hi: f32, env: &mut impl ObjAnimEnv) -> f32 {
    let u = unit(env.rand_game());
    lo + (hi - lo) * u
}

/// `cControllerNode::Finish`(1) 0x13a980.
fn finish(object: u32, env: &mut impl CtrlEnv) -> bool {
    let s = env.event_script(object, 3);
    if s >= 0 {
        env.call(CtrlCall::Start(s, object));
        return false;
    }
    let f = env.flags(object);
    *f = restore_flags(*f, 0xffff_0100);
    env.call(CtrlCall::Delete);
    true
}

fn base_message(object: u32, kind: u32, env: &mut impl CtrlEnv) {
    let f = env.flags(object);
    match kind {
        7 => *f &= 0xffff_f7ff,
        8 => *f |= 0x800,
        _ => {}
    }
}

/// `cFlagNode` (13): +0x34 frames to live, +0x38 mirrored, +0x3c the phase rate, +0x40 the wave's size over the
/// flag's length, +0x44 the phase, +0x190.. the model's size words (the first ×250, at least 250).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flag {
    pub object: u32,
    pub life: i32,
    pub mirror: u32,
    pub rate: f32,
    pub size: f32,
    pub phase: f32,
    pub dims: [f32; 3],
}

impl Flag {
    /// `cFlagNode::Init` 0x144938 (from the constructor 0x144778): op words mirror, rate (cycles a second), size
    /// (−1: random 5..50 a second), life; `dims` the model's +0x18.. (length, …). A rate of −60 (−1 after the
    /// /60) is random 0.5..2. The phase starts random.
    pub fn new(object: u32, a: &[u32], dims: [f32; 3], env: &mut impl ObjAnimEnv) -> Flag {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let mut fl = Flag { object, life: frames(f(4)), mirror: w(1), rate: f(2) * f32::from_bits(SIXTIETH), size: f(3), phase: 0.0, dims };
        if fl.size == -1.0 {
            let u = unit(env.rand_fx());
            fl.size = (u * 45.0 + 5.0) * f32::from_bits(SIXTIETH);
        }
        if fl.rate == -1.0 {
            let u = unit(env.rand_fx());
            fl.rate = (u * 1.5 + 0.5) * f32::from_bits(SIXTIETH);
        }
        let u = env.rand_fx();
        let fls = env.flags(object);
        *fls = *fls & 0xffff_fffd | 4;
        fl.phase = unit(u) + 0.0;
        let mut l = dims[0];
        if l < 1.0 {
            l = 1.0;
        }
        fl.dims[0] = l * 250.0;
        fl.size /= l * 250.0;
        env.anim(ObjAnimCall::FlagStrip);
        fl
    }

    /// Update 0x144e48.
    pub fn update(&mut self, env: &mut impl ObjAnimEnv) -> bool {
        if self.life > 0 {
            self.life -= 1;
            if self.life == 0 {
                return finish(self.object, env);
            }
        }
        let p = self.phase + self.rate;
        self.phase = if 1.0 <= p { p - 1.0 } else { p };
        env.anim(ObjAnimCall::FlagStrip);
        false
    }
}

/// The clip cursor of `cAnimObjectNode` (fields from its start; its controller part is at +0x30, the object at
/// +0x58): +8 the loop (a half: 0 once, 1 loop, 2 back and forth), +0xc playing, +0x10 done, +0x14 frames of delay,
/// +0x18 the contact re-arm frames, +0x1c the contact cooldown, +0x20 the rate (clip seconds a frame), +0x24 the
/// time, +0x28 / +0x2c the clip's start and end (seconds; the files count 30 frames a second), +0x64 a moving box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimObject {
    pub object: u32,
    pub looping: i16,
    pub playing: u32,
    pub done: u32,
    pub delay: i32,
    pub rearm: i32,
    pub cooldown: i32,
    pub rate: f32,
    pub time: f32,
    pub start: f32,
    pub end: f32,
    pub moving_box: bool,
}

impl AnimObject {
    /// `cAnimObjectNode::cAnimObjectNode` 0x198810: op words loop, start (frames), end (frames; < 0 the model's
    /// +0x14), speed (frames a second), its jitter, re-arm (s), random start, direction (4: backwards); `model_end`
    /// the model's +0x14; `armed` whether the thread had a contact (the cooldown starts full); `apply` samples the
    /// clip at once. The object's flags `& ~2 | 4`.
    pub fn new(object: u32, a: &[u32], model_end: f32, armed: bool, apply: bool, env: &mut impl ObjAnimEnv) -> AnimObject {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let start = if f(2) < 0.0 { 0.0 } else { f(2) };
        let end = if f(3) < 0.0 { model_end } else { f(3) };
        let rearm = frames(f(6));
        let start = start / 30.0;
        let end = end / 30.0;
        let time = if w(7) != 0 { game_range(start, end, env) } else { start };
        let speed = if f(5) == 0.0 {
            f(4) / 30.0
        } else {
            let base = f(4) / 30.0;
            let jitter = f(5) / 30.0;
            base + jitter * game_range(-1.0, 1.0, env)
        };
        let mut rate = speed * f32::from_bits(SIXTIETH);
        if w(8) == 4 {
            rate = -rate;
        }
        let fl = env.flags(object);
        *fl = *fl & 0xffff_fffd | 4;
        if apply {
            env.anim(ObjAnimCall::ApplyFrames);
        }
        AnimObject {
            object,
            looping: w(1) as u16 as i16,
            playing: 1,
            done: 0,
            delay: 0,
            rearm,
            cooldown: if armed { rearm } else { 0 },
            rate,
            time,
            start,
            end,
            moving_box: false,
        }
    }

    /// `AnimNode_TickDelay` 0x199030: the contact cooldown counts down (when re-arming is on).
    pub fn tick_delay(&mut self) {
        if self.rearm >= 0 && self.cooldown > 0 {
            self.cooldown -= 1;
        }
    }

    /// `cAnimObjectNode::Update` 0x199058. Returns whether the node was deleted.
    pub fn update(&mut self, env: &mut impl ObjAnimEnv) -> bool {
        if self.moving_box {
            env.anim(ObjAnimCall::MoveBox);
        }
        self.tick_delay();
        if self.done != 0 {
            return false;
        }
        if self.playing != 0 {
            let d = self.delay;
            if d as f32 <= 0.0 {
                let r = self.rate;
                let t = self.time + r;
                self.time = t;
                let mut ended = false;
                if 0.0 <= r {
                    if self.end <= t {
                        ended = true;
                        match self.looping {
                            2 => {
                                self.rate = -r;
                                self.time = self.end - (t - self.end);
                            }
                            1 => self.time = self.start + (t - self.end),
                            _ => self.time = self.end,
                        }
                    }
                } else if t <= self.start {
                    ended = true;
                    match self.looping {
                        2 => {
                            self.rate = -r;
                            self.time = self.start + (self.start - t);
                        }
                        1 => self.time = self.end - (self.start - t),
                        _ => self.time = self.start,
                    }
                }
                if ended && self.looping == 0 {
                    self.done = 1;
                }
            }
            if 0.0 < d as f32 {
                self.delay = d - 1;
            }
        }
        env.anim(ObjAnimCall::ApplyFrames);
        if self.done != 0 {
            return finish(self.object, env);
        }
        false
    }

    /// OnContact 0x198eb8: when not cooling down, the object's contact script (event 1) with the rider's contacts.
    pub fn on_contact(&mut self, env: &mut impl ObjAnimEnv) {
        if self.cooldown == 0 {
            let s = env.event_script(self.object, 1);
            self.cooldown = self.rearm;
            if s != -1 {
                env.call(CtrlCall::Start(s, self.object));
            }
        }
    }

    /// OnMessage 0x199850: 1 the delay (s), 4 jump to a time (frames, inside the clip); others the base's.
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl ObjAnimEnv) {
        match kind {
            1 => self.delay = frames(value),
            4 => {
                let t = value / 30.0;
                if self.start <= t && t <= self.end {
                    self.time = t;
                }
            }
            _ => base_message(self.object, kind, env),
        }
    }

    /// Kill 0x199200.
    pub fn kill(&mut self, env: &mut impl ObjAnimEnv) {
        let f = env.flags(self.object);
        *f = restore_flags(*f, 0xffff_0000);
        env.call(CtrlCall::Delete);
    }
}

/// `cAnimDeltaNode` (0x101): an [`AnimObject`] that is not playing by itself; message 2 adds an amount (frames) it
/// then plays through (+0x68), at |rate| a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimDelta {
    pub anim: AnimObject,
    pub left: f32,
}

impl AnimDelta {
    /// 0x199d10.
    pub fn new(object: u32, a: &[u32], model_end: f32, armed: bool, env: &mut impl ObjAnimEnv) -> AnimDelta {
        let mut anim = AnimObject::new(object, a, model_end, armed, true, env);
        anim.playing = 0;
        AnimDelta { anim, left: 0.0 }
    }

    /// Update 0x199db8. Returns whether the node was deleted.
    pub fn update(&mut self, env: &mut impl ObjAnimEnv) -> bool {
        if self.anim.done != 0 {
            self.anim.tick_delay();
            return false;
        }
        if 0.0 <= self.left {
            if 0.0 < self.left {
                let l = self.left - self.anim.rate.abs();
                self.left = if l < 0.0 { 0.0 } else { l };
            }
            // (the game sets playing even on a node its Update just deleted)
            let gone = self.anim.update(env);
            self.anim.playing = (0.0 < self.left) as u32;
            gone
        } else {
            self.anim.tick_delay();
            if 0 < self.anim.delay {
                self.anim.delay -= 1;
            }
            if self.anim.done != 0 {
                return finish(self.anim.object, env);
            }
            false
        }
    }

    /// OnMessage 0x199eb0: 2 adds value / 30 to play; others [`AnimObject::on_message`].
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl ObjAnimEnv) {
        if kind == 2 {
            self.left += value / 30.0;
        } else {
            self.anim.on_message(value, kind, env);
        }
    }
}

/// `cAnimComboNode` (0x102): an [`AnimObject`] (at +0x1c) and a second clip blended over it: +8 its rate, +0xc its
/// time, +0x10 / +0x14 its start and end, +0x18 playing it, +0x19 stopped for good, +0x1a stop at its end, +0x1b
/// (the op word's sign) blend while stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimCombo {
    pub anim: AnimObject,
    pub rate2: f32,
    pub time2: f32,
    pub start2: f32,
    pub end2: f32,
    pub second: u8,
    pub stopped: u8,
    pub stop_at_end: u8,
    pub sign: u8,
}

impl AnimCombo {
    /// `AnimComboNode_Init` 0x199f20: op words 9..12: the second clip's start, end (< 0: the first's start / the
    /// model's end), speed, stop flag.
    pub fn new(object: u32, a: &[u32], model_end: f32, armed: bool, env: &mut impl ObjAnimEnv) -> AnimCombo {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let anim = AnimObject::new(object, a, model_end, armed, false, env);
        let s2 = if f(9) < 0.0 { f(3) } else { f(9) };
        let e2 = if f(10) < 0.0 { model_end } else { f(10) };
        let c = AnimCombo {
            anim,
            rate2: (f(11) / 30.0) * f32::from_bits(SIXTIETH),
            time2: 0.0,
            start2: s2 / 30.0,
            end2: e2 / 30.0,
            second: 0,
            stopped: 0,
            stop_at_end: (w(12) != 0) as u8,
            sign: (w(12) >> 31) as u8,
        };
        c.apply_frames(env);
        c
    }

    /// `AnimComboNode_ApplyFrames` 0x19a478 (what it samples is drawing; whether it samples at all is not).
    fn apply_frames(&self, env: &mut impl ObjAnimEnv) {
        env.anim(ObjAnimCall::ApplyFrames);
    }

    /// Update 0x19a218.
    pub fn update(&mut self, env: &mut impl ObjAnimEnv) -> bool {
        if self.stopped != 0 {
            self.anim.tick_delay();
            return false;
        }
        if self.second == 0 {
            return self.anim.update(env);
        }
        self.anim.tick_delay();
        let t = self.time2 + self.rate2;
        self.time2 = t;
        if self.end2 <= t {
            self.time2 = self.end2;
            if self.stop_at_end != 0 {
                self.stopped = 1;
            }
            self.second = 0;
        }
        self.apply_frames(env);
        false
    }

    /// OnMessage 0x19a2d0: 3 starts the second clip from its start (unless it is playing or stopped); others
    /// [`AnimObject::on_message`].
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl ObjAnimEnv) {
        if kind == 3 {
            if self.second == 0 && self.stopped == 0 {
                env.anim(ObjAnimCall::ComboCopy);
                self.second = 1;
                self.time2 = self.start2;
            }
        } else {
            self.anim.on_message(value, kind, env);
        }
    }

    /// Kill 0x19a390: deleted unless stopped for good.
    pub fn kill(&mut self, env: &mut impl ObjAnimEnv) {
        if self.stopped == 0 {
            self.anim.kill(env);
        }
    }
}

/// `cAnimTexFlipNode` (0x103): an [`AnimObject`] (at +0x18) and a flip-book: +8 the step clock, +0xc the rate
/// (frames a second / 60), +0x10 the frame, +0x14 random, the `cTexFlip` at +0x80.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimTexFlip {
    pub anim: AnimObject,
    pub acc: f32,
    pub rate: f32,
    pub frame: i32,
    pub random: u32,
    pub tex: TexFlip,
}

impl AnimTexFlip {
    /// 0x19a5a8: op words 9, 10: the start frame (< 0 random among the first eight; −2 random at each step) and
    /// the rate.
    pub fn new(object: u32, a: &[u32], model_end: f32, armed: bool, count: i32, env: &mut impl ObjAnimEnv) -> AnimTexFlip {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let anim = AnimObject::new(object, a, model_end, armed, true, env);
        let acc = 0.0f32;
        let rate = f32::from_bits(w(10)) * f32::from_bits(SIXTIETH);
        let mut frame = w(9) as i32;
        if (frame as f32) < acc {
            let u = unit(env.rand_fx());
            frame = ceil_int(u * 8.0 + acc);
        }
        let random = ((w(9) as i32) as f32 == -2.0) as u32;
        let mut tex = TexFlip { frame: 0, count };
        tex.set_frame(frame, env);
        AnimTexFlip { anim, acc, rate, frame, random, tex }
    }

    /// Update 0x19a778: the flip-book (forward, or random), then the clip.
    pub fn update(&mut self, env: &mut impl ObjAnimEnv) -> bool {
        if 0.0 < self.rate {
            let a = self.acc + self.rate;
            self.acc = a;
            if 1.0 <= a {
                self.acc = a - 1.0;
                let n = self.tex.count;
                if self.random != 0 && 2 <= n {
                    let u = unit(env.rand_fx());
                    self.frame = ceil_int((((n - 1) as f32) - 0.0) * u + 0.0);
                } else {
                    self.frame += 1;
                    if n <= self.frame {
                        self.frame = 0;
                    }
                }
                let f = self.frame;
                self.tex.set_frame(f, env);
            }
        }
        self.anim.update(env)
    }
}

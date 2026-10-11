//! The level scripts' remaining controllers (board row E9j): `cCrowdBoxNode` (17, the animated crowd boards),
//! `cParticleNode` (22, an emitter with a lifetime) and `cMovieNode` (23, the jumbotron's video). The crowd's cells
//! and the particles themselves are drawn elsewhere; this is what the controllers decide. Checked against the game's
//! own objects in the function runner (`tests/scenery.rs`).

use crate::controllers::{restore_flags, CtrlCall, CtrlEnv};
use crate::texanim::AnimEnv;

/// Each crowd animation set's 16 frames (0x31d378: set × 16 words, frame indices into 0x31d4b8's 0x48-byte frames).
pub const CROWD_SETS: [[i32; 16]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [0, 1, 2, 3, 2, 1, 0, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [0, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1],
];

/// Per excitement level (0..6) the chance out of 256 that a cell sits out a while after its cycle (0x31d478).
pub const CROWD_REST: [u32; 7] = [256, 0, 50, 100, 200, 100, 50];

/// What the crowd box reads from the race.
pub trait CrowdEnv: AnimEnv {
    /// the race's frame counter (race+0x18) and its +0x1c (1: the crowd cheers at 7)
    fn race_frame(&self) -> (i32, i32);
    /// for each rider in the race: (+0x41c human, position +0x140, +0x38)
    fn riders(&self) -> Vec<(i32, [f32; 4], u32)>;
    /// the object's position (+0x30)
    fn object_pos(&self) -> [f32; 4];
    /// `CrowdSys_GetBoxState` 0x2213f8(audio, the rider's +0x38)
    fn box_state(&mut self, x38: u32) -> i32;
}

/// `cCrowdBoxNode`: +0x34 rows, +0x38 columns (op words 2, 3), +0x3c how excited (0..7), +0x40 16, per cell (16)
/// +0x84 a phase counter (below 0: sitting out) and +0xc4 its animation set; +0x44 per shown cell its frame (here the
/// frame index; the game keeps a pointer 0x31d4b8 + 0x48·index).
#[derive(Clone, Debug, PartialEq)]
pub struct Crowd {
    pub object: u32,
    pub rows: i32,
    pub cols: i32,
    pub excited: i32,
    pub phase: [i32; 16],
    pub set: [u32; 16],
    pub frame: Vec<i32>,
}

impl Crowd {
    /// `cCrowdBoxNode::cCrowdBoxNode` 0x145b28 and `Init` 0x145c00: each cell a random phase (0..15) and set (0..3);
    /// then the cells stepped once; the object's flags `& ~2 | 4`.
    pub fn new(object: u32, a: &[u32], env: &mut impl CrowdEnv) -> Crowd {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let mut c = Crowd { object, rows: w(2) as i32, cols: w(3) as i32, excited: 0, phase: [0; 16], set: [0; 16], frame: vec![] };
        for i in 0..16 {
            c.phase[i] = (env.rand_fx() & 0xf) as i32;
            c.set[i] = env.rand_fx() & 3;
        }
        c.step(env);
        let f = env.flags(object);
        *f = *f & 0xffff_fffd | 4;
        c
    }

    /// `cCrowdBoxNode::StepCells` 0x145e08: when excited, each cell's phase moves on; at the end of each cycle the
    /// cell picks a new set and (below excitement 7) may sit out up to 63 frames. Then the shown cells' frames (rows × columns;
    /// past 16 the game reads beyond its cell arrays, not modelled: the cells wrap here).
    pub fn step(&mut self, env: &mut impl CrowdEnv) {
        if self.excited != 0 {
            for i in 0..16 {
                let p = self.phase[i].wrapping_add(1);
                self.phase[i] = p;
                if 0xf < p && p & 0xf == 0 {
                    let r = env.rand_fx();
                    self.set[i] = r & 3;
                    if self.excited < 7 && (r & 0xff) < CROWD_REST[self.excited as usize] {
                        self.phase[i] = -((r & 0x3f) as i32);
                    }
                }
            }
        }
        let n = (self.rows.max(0) * self.cols.max(0)) as usize;
        self.frame = (0..n)
            .map(|i| {
                let set = &CROWD_SETS[self.set[i % 16] as usize];
                let p = self.phase[i % 16];
                if p < 0 {
                    set[0]
                } else {
                    set[((p + i as i32) & 0xf) as usize]
                }
            })
            .collect();
    }

    /// `cCrowdBoxNode::Update` 0x145c88: every fifth race frame: excitement 7 when the race's +0x1c is 1, else from
    /// the human rider within 200 m (`CrowdSys_GetBoxState` + 4, wrapped to 0..7), else 0; then the cells step.
    pub fn update(&mut self, env: &mut impl CrowdEnv) {
        let (frame, mode) = env.race_frame();
        if frame % 5 != 0 {
            return;
        }
        self.excited = 7;
        if mode != 1 {
            self.excited = 0;
            let o = env.object_pos();
            for (human, pos, x38) in env.riders() {
                if human == 1 {
                    let d = [pos[0] - o[0], pos[1] - o[1], pos[2] - o[2], pos[3] - o[3]];
                    let s = ((d[0] * d[0] + d[1] * d[1]) + 1.0 * (d[2] * d[2])) + 1.0 * (d[3] * d[3]);
                    if s.sqrt() <= 20000.0 {
                        let st = env.box_state(x38);
                        self.excited = (st.wrapping_add(4) as u32 & 7) as i32;
                    }
                }
            }
        }
        self.step(env);
    }
}

/// What the particle node drives in its effect (`ParticleSys` at node+0x40).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleCall {
    /// `ParticleSys_SetTime` 0x1d59e0(t)
    SetTime(u32),
    /// `ParticleSys_StepEmitter` 0x1d5a30(1/60): a running emitter
    StepEmitter,
    /// `ParticleSys_AdvanceTime` 0x1d59f0(1/60): one with a lifetime
    AdvanceTime,
}

pub trait ParticleEnv: CtrlEnv {
    fn particles(&mut self, c: ParticleCall);
}

/// `cParticleNode` (22): +0x30 the lifetime left (< 0: emitting until told; 0 stopped), from the op's effect
/// (`ParticleSys_Create` 0x1d8490).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub object: u32,
    pub life: f32,
}

impl Particle {
    /// `cParticleNode::cParticleNode` 0x147f88: a running emitter has its time reset (`ParticleNode_SetLifetime`
    /// with 0).
    pub fn new(object: u32, life: f32, env: &mut impl ParticleEnv) -> Particle {
        let mut p = Particle { object, life };
        if p.life < 0.0 {
            p.set_lifetime(0.0, env);
        }
        p
    }

    /// `ParticleNode_SetLifetime` 0x1d8ac8.
    fn set_lifetime(&mut self, v: f32, env: &mut impl ParticleEnv) {
        if v != 0.0 {
            self.life = v;
            env.particles(ParticleCall::SetTime(v.to_bits()));
        } else {
            env.particles(ParticleCall::SetTime(0));
        }
    }

    /// `ParticleNode_Update` 0x1d8b20.
    fn step(&mut self, env: &mut impl ParticleEnv) {
        if self.life != 0.0 {
            if self.life < 0.0 {
                env.particles(ParticleCall::StepEmitter);
            } else {
                env.particles(ParticleCall::AdvanceTime);
                let l = self.life - f32::from_bits(0x3c88_8889);
                self.life = if l < 0.0 { 0.0 } else { l };
            }
        }
    }

    /// `cParticleNode::Update` 0x148100: a lifetime that runs out finishes the node (Finish(1)). Returns whether the
    /// node was deleted.
    pub fn update(&mut self, env: &mut impl ParticleEnv) -> bool {
        if self.life < 0.0 {
            self.step(env);
            return false;
        }
        self.step(env);
        if self.life <= 0.0 {
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
        false
    }

    /// `cParticleNode::OnMessage` 0x148188: kind 1 gives a running emitter a lifetime (the value, seconds); others the
    /// base's.
    pub fn on_message(&mut self, value: f32, kind: u32, env: &mut impl ParticleEnv) {
        if kind != 1 {
            let f = env.flags(self.object);
            match kind {
                7 => *f &= 0xffff_f7ff,
                8 => *f |= 0x800,
                _ => {}
            }
            return;
        }
        if self.life < 0.0 {
            self.set_lifetime(value, env);
        }
    }
}

/// `MovieNode_OpenJumbotronVideo` 0x148b28: the jumbotron's video for the course being played (0x32de88), opened at
/// 128 × 64 (`VideoPlayer_Open`(…, 0x80, 0x40, 1, 15)).
pub fn jumbotron_video(course: i32) -> &'static str {
    match course.clamp(0, 12) {
        0 => "data/video/jgari.sss",
        1 => "data/video/jsnow.sss",
        2 => "data/video/jelysium.sss",
        3 => "data/video/jmesa.sss",
        4 => "data/video/jmerquer.sss",
        5 => "data/video/jaloha.sss",
        6 => "data/video/jpipe.sss",
        8 => "data/video/jmegaple.sss",
        11 => "data/video/jalaska.sss",
        _ => "data/video/jumbo.sss",
    }
}

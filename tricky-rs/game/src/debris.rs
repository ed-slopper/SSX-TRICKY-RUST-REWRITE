//! The falling pieces of broken objects, `cMeshAnimNode` (type 20, board row E9g2), as the game runs them: each
//! part of the object's model flies off with its own spin and velocity under gravity, from its starting pose
//! ([`starts`], row E9g3: the model's part matrices through its hierarchy or the animation's frames, times the
//! object's, `Mat4_ToPosQuat` 0x2504f0); drawing the pieces is rendering. Checked against the game's own objects in the function runner
//! (`tests/debris.rs`).

use crate::breakables::{BreakCall, BreakEnv, Contact};
use crate::controllers::{restore_flags, CtrlCall};
use crate::math::sin_cos;
use crate::orient::{mat4_to_pos_quat, quat_multiply};

type V4 = [f32; 4];

const SIXTIETH: u32 = 0x3c88_8889;

fn unit(r: u32) -> f32 {
    f32::from_bits(r & 0x7f_ffff | 0x3f80_0000) - 1.0
}

fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

/// A part of the object's model (a 6-word model node): its parent (−1 none), whether an animation drives it (+0x10)
/// and its matrix (+0x14; none: the identity at 0x346440).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub parent: i32,
    pub animated: bool,
    pub matrix: Option<[V4; 4]>,
}

/// Rows `m` through the frame `f` (vmulabc / vmaddabc / vmaddbc: each row the frame's rows weighted by its four
/// lanes, summed in order).
fn through(f: [V4; 4], m: [V4; 4]) -> [V4; 4] {
    m.map(|v| [0, 1, 2, 3].map(|i| ((f[0][i] * v[0] + f[1][i] * v[1]) + f[2][i] * v[2]) + f[3][i] * v[3]))
}

const IDENTITY: [V4; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// The pieces' starting poses, as `cMeshAnimNode::cMeshAnimNode` 0x1466b8 makes them before its spins and
/// velocities: each part's matrix (or the identity) through its parent's world matrix (parts come after their
/// parents; one that does not reads a slot not yet made: zeros here); with an animation whose +0x14 has bit 0x100
/// (`anim_flags`; the game first calls its +0xf4), the animated parts take their frame matrices instead
/// (`MeshAnimNode_GetFrameMatrix` 0x1998d0, `frame(n)` for the n-th animated part). Each world matrix is then put
/// through the object's matrix `rows` and split by `Mat4_ToPosQuat` into (position, rotation).
pub fn starts(parts: &[Part], rows: [V4; 4], anim_flags: Option<u32>, frame: &mut dyn FnMut(usize) -> [V4; 4]) -> Vec<(V4, V4)> {
    let animate = anim_flags.is_some_and(|f| f >> 8 & 1 != 0);
    let mut world = vec![[[0.0f32; 4]; 4]; parts.len()];
    let mut n = 0;
    for (i, p) in parts.iter().enumerate() {
        let local = if animate && p.animated {
            n += 1;
            frame(n - 1)
        } else {
            p.matrix.unwrap_or(IDENTITY)
        };
        world[i] = if p.parent == -1 { local } else { through(world.get(p.parent as usize).copied().unwrap_or_default(), local) };
    }
    world.iter().map(|w| mat4_to_pos_quat(through(rows, *w))).collect()
}

/// What the debris needs beyond [`BreakEnv`].
pub trait DebrisEnv: BreakEnv {
    /// `Rand_Fx` 0x23d4a8
    fn rand_fx(&mut self) -> u32;
}

/// One piece: +0x38 its rotation (w, x, y, z), +0x1c0 its spin axis, +0x340 its spin (radians a frame), +0x3c0 its
/// position, +0x540 its velocity (cm a frame).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Piece {
    pub rot: V4,
    pub axis: V4,
    pub spin: f32,
    pub pos: V4,
    pub vel: V4,
}

/// `cMeshAnimNode`: +0x34 frames to live, +0x6c4 stays (1: the pieces stay where they land when the time is up),
/// +0x6c8 gravity (× −16.333334 cm a frame²), +0x3a0 the random spread, +0x3b0 the base velocity, +0x6cc flying.
#[derive(Clone, Debug, PartialEq)]
pub struct Debris {
    pub object: u32,
    pub life: i32,
    pub stays: u32,
    pub gravity: f32,
    pub spread: V4,
    pub base: V4,
    pub flying: u32,
    pub pieces: Vec<Piece>,
}

/// v / |v| as VU0 does it (1/√0 is the largest float).
fn normalise(v: V4) -> V4 {
    let s = ((v[0] * v[0] + v[1] * v[1]) + 1.0 * (v[2] * v[2])) + 1.0 * (v[3] * v[3]);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    v.map(|x| x * q)
}

impl Debris {
    /// `cMeshAnimNode::cMeshAnimNode` 0x1466b8 then `FUN_00146f40`: op words stays, gravity, life (s), a direction
    /// (p6..p8; zero: the contact's +0x10 × |its speed|, or nothing without a contact), the spread (p9..p11) and a
    /// multiplier for the base velocity (p12); `rows` the object's matrix; `starts` each piece's starting (position,
    /// rotation). Each piece then gets a random spin (2..10 radians a second) about a random axis, and the base
    /// velocity plus a random part of the spread (x, y either way, z up) — all per second, made per frame. The
    /// object's flags `& ~2 | 4`.
    pub fn new(object: u32, a: &[u32], rows: [V4; 4], contact: Option<&Contact>, starts: &[(V4, V4)], env: &mut impl DebrisEnv) -> Debris {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let spread = [f(7), f(8), f(9), 0.0];
        let dir = [f(4), f(5), f(6)];
        let base = if dir[0] == 0.0 && dir[1] == 0.0 && dir[2] == 0.0 {
            match contact {
                Some(c) => {
                    let s = c.speed.abs();
                    c.a.map(|v| v * s)
                }
                None => [0.0; 4],
            }
        } else {
            let v = [dir[0], dir[1], dir[2], 0.0];
            [0, 1, 2, 3].map(|k| ((rows[0][k] * v[0] + rows[1][k] * v[1]) + rows[2][k] * v[2]) + rows[3][k] * v[3])
        };
        let m = f(10);
        let base = base.map(|v| v * m);
        let fl = env.flags(object);
        *fl = *fl & 0xffff_fffd | 4;
        let mut pieces: Vec<Piece> = starts.iter().map(|(p, q)| Piece { rot: *q, axis: [0.0; 4], spin: 0.0, pos: *p, vel: [0.0; 4] }).collect();
        for p in pieces.iter_mut() {
            p.spin = (unit(env.rand_fx()) * 8.0 + 2.0) * f32::from_bits(SIXTIETH);
            let mut ax = [0.0f32; 4];
            for v in ax.iter_mut().take(3) {
                let u = unit(env.rand_fx());
                *v = u + u + -1.0;
            }
            p.axis = normalise(ax);
        }
        for p in pieces.iter_mut() {
            let one = |s: f32, env: &mut dyn FnMut() -> u32| ((s - 0.0) * unit(env()) + 0.0) * f32::from_bits(SIXTIETH);
            let mut r = || env.rand_fx();
            let mut vx = one(spread[0], &mut r);
            if r() & 1 != 0 {
                vx = -vx;
            }
            let mut vy = one(spread[1], &mut r);
            if r() & 1 != 0 {
                vy = -vy;
            }
            let vz = one(spread[2], &mut r);
            let b = base.map(|v| v * f32::from_bits(SIXTIETH));
            p.vel = [b[0] + vx, b[1] + vy, b[2] + vz, b[3] + 0.0];
        }
        Debris { object, life: frames(f(3)), stays: w(1), gravity: f(2), spread, base, flying: 1, pieces }
    }

    /// `cMeshAnimNode::Update` 0x147230: when the time is up the node finishes (unless the pieces stay) and stops;
    /// while flying each piece turns by its spin about its axis (renormalised) and moves, falling.
    pub fn update(&mut self, env: &mut impl DebrisEnv) -> bool {
        let up = if self.life < 1 {
            true
        } else {
            self.life -= 1;
            self.life < 1
        };
        if up {
            let mut gone = false;
            if self.stays != 1 {
                gone = finish(self.object, env);
            }
            self.flying = 0;
            return gone;
        }
        if self.flying == 0 {
            return false;
        }
        for p in self.pieces.iter_mut() {
            let (s, c) = sin_cos(p.spin * 0.5);
            let d = [c, s * p.axis[0], s * p.axis[1], s * p.axis[2]];
            let q = quat_multiply(p.rot, d);
            let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
            let len = (((x * x + y * y) + z * z) + w * w).sqrt();
            p.rot = if len == 0.0 {
                [1.0, x, y, z]
            } else {
                let k = 1.0 / len;
                [w * k, x * k, y * k, z * k]
            };
        }
        let g = self.gravity * -16.333334;
        for p in self.pieces.iter_mut() {
            for k in 0..4 {
                p.pos[k] += p.vel[k];
            }
            let add = [0.0, 0.0, g, 0.0];
            for k in 0..4 {
                p.vel[k] += add[k];
            }
        }
        false
    }

    /// `cMeshAnimNode::Kill` 0x1474a0: pieces that stay are restored and deleted; others give way to a dead node.
    pub fn kill(&mut self, env: &mut impl DebrisEnv) {
        if self.stays == 1 {
            let f = env.flags(self.object);
            *f = restore_flags(*f, 0xffff_0100);
            env.call(CtrlCall::Delete);
        } else {
            env.breaks(BreakCall::ReplacedWithDead);
        }
    }
}

/// `cControllerNode::Finish`(1) 0x13a980.
fn finish(object: u32, env: &mut impl DebrisEnv) -> bool {
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

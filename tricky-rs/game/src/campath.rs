//! The camera path's helpers (board row E11e): `CamPath_SegmentPoint` 0x170610 and `CamPath_SegmentAngles` 0x1708e8
//! (a Catmull-Rom-style Hermite along keyframes: the tangent at each end is the chord of its neighbours scaled by
//! the ratio of the segment's length to that chord's), `Camera_UpdateCutFade` 0x174b38 and `CamScript_UpdateCutFade`
//! 0x174c58. The maths helpers are [`crate::math::hermite`] and [`crate::math::angle_diff`].

use crate::emitter::{cl, ps2_div};
use crate::math::angle_diff;

fn m(a: f32, b: f32) -> f32 {
    cl(a * b)
}
fn ad(a: f32, b: f32) -> f32 {
    cl(a + b)
}
fn sb(a: f32, b: f32) -> f32 {
    cl(a - b)
}

/// `Math_Hermite` 0x1704b0 as the PS2 runs it (every operation stops at the largest float rather than overflow;
/// [`crate::math::hermite`] is the same without that).
pub fn hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = m(t, t);
    let t3 = m(t2, t);
    let f4 = m(t2, 3.0);
    let f2 = ad(t2, t2);
    let f0 = ad(t3, t3);
    let f3 = m(t3, -2.0);
    let f2 = sb(t3, f2);
    let f0 = sb(f0, f4);
    let f3 = ad(f3, f4);
    let f2 = ad(f2, t);
    let f0 = m(f0, p0);
    let f3 = m(f3, p1);
    let f2 = m(f2, m0);
    let f0 = ad(f0, p0);
    let f1 = sb(t3, t2);
    let f0 = ad(f0, f3);
    let f1 = m(f1, m1);
    let f0 = ad(f0, f2);
    ad(f0, f1)
}

/// A keyframe record: +0..+8 position, +0xc / +0x10 / +0x14 three angles, +0x18 a fourth value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub pos: [f32; 3],
    pub ang: [f32; 3],
    pub w: f32,
}

/// Where the path stands: `idx` is the segment (path +4), `count` the number of keyframes (`path[+0x24] + 0x12c`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathAt {
    pub idx: i32,
    pub count: i32,
}

/// What a segment writes through its two out-pointers: the chord ratio at the start (when not the first segment)
/// and at the end (when not near the last).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Ratios {
    pub r0: Option<f32>,
    pub r1: Option<f32>,
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let x = m(a[0], b[0]);
    let y = m(a[1], b[1]);
    let z = m(a[2], b[2]);
    ad(ad(x, y), z)
}

fn diff(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [sb(a[0], b[0]), sb(a[1], b[1]), sb(a[2], b[2])]
}

fn len(a: [f32; 3]) -> f32 {
    dot3(a, a).sqrt()
}

/// The two tangent scales (start, end) of a segment `a`→`b` between `prev` and `next`, and the ratios it writes.
fn tangent_scales(at: PathAt, prev: [f32; 3], a: [f32; 3], b: [f32; 3], next: [f32; 3]) -> ([f32; 2], Ratios) {
    let mut out = Ratios::default();
    let mut s = [1.0f32; 2];
    if at.idx != 0 {
        let l1 = len(diff(b, a));
        let l2 = len(diff(b, prev));
        let r = ps2_div(l1, l2);
        out.r0 = Some(r);
        s[0] = r;
    }
    if at.idx < at.count - 2 {
        let l1 = len(diff(b, a));
        let l3 = len(diff(next, a));
        let r = ps2_div(l1, l3);
        out.r1 = Some(r);
        s[1] = r;
    }
    (s, out)
}

/// `CamPath_SegmentPoint` 0x170610(out, path, prev, a, b, next, &r0, &r1, t): the point at `t` on segment a→b.
pub fn segment_point(at: PathAt, t: f32, prev: &Key, a: &Key, b: &Key, next: &Key) -> ([f32; 3], Ratios) {
    let (s, ratios) = tangent_scales(at, prev.pos, a.pos, b.pos, next.pos);
    let tan0 = if at.idx == 0 {
        diff(b.pos, a.pos)
    } else {
        let d = diff(b.pos, prev.pos);
        [m(d[0], s[0]), m(d[1], s[0]), m(d[2], s[0])]
    };
    let tan1 = if at.idx < at.count - 2 {
        let d = diff(next.pos, a.pos);
        [m(d[0], s[1]), m(d[1], s[1]), m(d[2], s[1])]
    } else {
        diff(b.pos, a.pos)
    };
    let p = [0, 1, 2].map(|k| hermite(a.pos[k], b.pos[k], tan0[k], tan1[k], t));
    (p, ratios)
}

/// What `CamPath_SegmentAngles` 0x1708e8 builds (the first 0x1c bytes of its 0x58-byte answer; the rest is the stack's):
/// the point, the three angles and the fourth value, with the constant 100.0 at +0x1c and zero at +0x20, +0x30, +0x54.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angles {
    pub pos: [f32; 3],
    pub ang: [f32; 3],
    pub w: f32,
}

/// `CamPath_SegmentAngles` 0x1708e8(out, path, prev, a, b, next, t).
pub fn segment_angles(at: PathAt, t: f32, prev: &Key, a: &Key, b: &Key, next: &Key) -> Angles {
    let (pos, ratios) = segment_point(at, t, prev, a, b, next);
    let r0 = ratios.r0.unwrap_or(0.0);
    let r1 = ratios.r1.unwrap_or(0.0);
    let mut t0 = [0.0f32; 3];
    let mut t1 = [0.0f32; 3];
    let w0;
    let w1;
    if at.idx == 0 {
        for k in 0..3 {
            t0[k] = angle_diff(b.ang[k], a.ang[k]);
        }
        w0 = sb(b.w, a.w);
    } else {
        for k in 0..3 {
            t0[k] = m(angle_diff(b.ang[k], prev.ang[k]), r0);
        }
        w0 = sb(b.w, prev.w);
    }
    if at.idx < at.count - 2 {
        for k in 0..3 {
            t1[k] = m(angle_diff(next.ang[k], a.ang[k]), r1);
        }
        w1 = sb(next.w, a.w);
    } else {
        for k in 0..3 {
            t1[k] = angle_diff(b.ang[k], a.ang[k]);
        }
        w1 = sb(b.w, a.w);
    }
    let mut ang = [0.0f32; 3];
    for k in 0..3 {
        let end = ad(a.ang[k], angle_diff(b.ang[k], a.ang[k]));
        ang[k] = hermite(a.ang[k], end, t0[k], t1[k], t);
    }
    let w = hermite(a.w, b.w, w0, w1, t);
    Angles { pos, ang, w }
}

/// The cut fade's three statics (0x3365f4 how many ticks of full black are left, 0x3365f8 whether a fade is running,
/// 0x3365fc its time).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CutFade {
    pub hold: i32,
    pub active: u32,
    pub t: f32,
}

/// `Camera_UpdateCutFade` 0x174b38: answers the camera's fade level (this+0x44c).
pub fn update_cut_fade(f: &mut CutFade) -> f32 {
    let mut level;
    if f.active != 0 {
        let k = f.t / f32::from_bits(0x3f19_999a);
        level = hermite(f32::from_bits(0x3f33_3333), 0.0, f32::from_bits(0x3fcc_cccd), 0.0, k);
        if f32::from_bits(0x3f7d_70a4) < level {
            level = f32::from_bits(0x3f7d_70a4);
        }
        let t = f.t + f32::from_bits(0x3c88_8889);
        f.t = t;
        if f32::from_bits(0x3f19_999a) <= t {
            f.active = 0;
        }
    } else {
        level = 0.0;
    }
    if f.hold > 0 {
        f.hold -= 1;
        level = 1.0;
    }
    level
}

/// What the camera path keeps between ticks: the segment `idx` (+4), the position along it `t` (+8) and its last
/// step `dt` (+0xc), the last point (+0x10), the wanted speed in cm/s (+0x1c; 0 takes the owner's +0x130) and the
/// easing `mode` (+0x20: 1 eases in, 2 in and out, 3 out, by the number of keys).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathState {
    pub idx: i32,
    pub t: f32,
    pub dt: f32,
    pub last: [f32; 3],
    pub speed: f32,
    pub mode: i32,
}

/// The path's owner: its keyframe count (+0x12c) and default speed (+0x130).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Owner {
    pub count: i32,
    pub speed: f32,
}

/// The camera at one keyframe, evaluated (`CamCamera::CamCamera`, `CamCamera_EvalPosition`, `CamCamera_EvalLookAt`).
pub trait KeyEnv {
    fn key(&mut self, i: i32) -> Key;
}

/// A neighbour that does not exist: the position and angles zero, the fourth value 1.6.
pub fn missing_key() -> Key {
    Key { pos: [0.0; 3], ang: [0.0; 3], w: f32::from_bits(0x3fcc_cccd) }
}

/// `CamPath_Eval` 0x170ca8: advances the path by one tick of arc length (`speed / 60` cm, found by a secant search
/// on the parameter, moving to the next segment at 1.0) and answers the eased point and angles, and whether the path
/// has ended.
pub fn eval(p: &mut PathState, owner: &Owner, env: &mut dyn KeyEnv) -> (Angles, bool) {
    let count = owner.count;
    if count < 2 {
        let k = missing_key();
        return (Angles { pos: [0.0; 3], ang: [0.0; 3], w: k.w }, true);
    }
    let mut ended = false;
    let mut first = true;
    let mut point = [0.0f32; 3];
    let mut slots: [Key; 4];
    let mut f21;
    let mut guard = 0u32;
    loop {
        slots = [missing_key(); 4];
        for i in 0..4i32 {
            let k = p.idx + i - 1;
            if k < 0 || k >= count {
                continue;
            }
            slots[i as usize] = env.key(k);
        }
        let at = PathAt { idx: p.idx, count };
        if first {
            first = false;
            let (pt, _) = segment_point(at, p.t, &slots[0], &slots[1], &slots[2], &slots[3]);
            p.last = pt;
        }
        f21 = ad(p.t, p.dt);
        let mut f22 = p.dt;
        let mut f23 = f22.abs();
        let mut s6 = false;
        let mut f25 = 0.0f32;
        let mut done = false;
        loop {
            guard += 1;
            assert!(guard < 1_000_000, "camera path search runs away");
            if 1.0 < f21 {
                f21 = 1.0;
            } else if f21 < 0.0 {
                f21 = 0.0;
            }
            if f23 < f32::from_bits(0x3586_37bd) {
                done = true;
                break;
            }
            let at = PathAt { idx: p.idx, count };
            let (pt, _) = segment_point(at, f21, &slots[0], &slots[1], &slots[2], &slots[3]);
            point = pt;
            let mut f20 = p.speed;
            if f20 == 0.0 {
                f20 = owner.speed;
            }
            let d = diff(pt, p.last);
            let dist = len(d);
            let step = ps2_div(f20, 60.0);
            let err = sb(dist, step);
            let tol = ps2_div(step, 10.0);
            if !(tol < err.abs()) {
                done = true;
                break;
            }
            if 0.0 < err {
                s6 = true;
                f22 = m(-f23, 0.5);
                f23 = f22.abs();
                f21 = ad(f21, f22);
                f25 = err;
                continue;
            }
            if !(f21 < 1.0) {
                break;
            }
            if !s6 {
                f21 = ad(f21, f22);
                f25 = err;
                continue;
            }
            let f0 = ps2_div(m(err, f22), sb(err, f25));
            f22 = f0.abs();
            f23 = f22.abs();
            f21 = ad(f21, f22);
            f25 = err;
        }
        if done {
            break;
        }
        if p.idx < count - 2 {
            p.idx += 1;
            p.dt = f32::from_bits(0x3d4c_cccd);
            p.t = 0.0;
            continue;
        }
        ended = true;
        f21 = 1.0;
        break;
    }
    let dt = sb(f21, p.t);
    p.dt = dt;
    p.t = f21;
    p.last = point;
    // the easing at the ends
    let idx = p.idx;
    let mode = p.mode;
    let mut tt = f21;
    if idx != 0 {
        if !(idx < count - 2) && (2..4).contains(&mode) {
            tt = hermite(0.0, 1.0, 1.0, 0.0, f21);
        }
    } else if mode == 1 {
        tt = hermite(0.0, 1.0, 0.0, 1.0, f21);
    } else if mode == 2 {
        tt = if count >= 3 { hermite(0.0, 1.0, 0.0, 1.0, f21) } else { hermite(0.0, 1.0, 0.0, 0.0, f21) };
    } else if mode == 3 && count < 3 {
        tt = hermite(0.0, 1.0, 1.0, 0.0, f21);
    }
    let at = PathAt { idx, count };
    (segment_angles(at, tt, &slots[0], &slots[1], &slots[2], &slots[3]), ended)
}

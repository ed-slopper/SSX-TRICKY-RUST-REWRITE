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

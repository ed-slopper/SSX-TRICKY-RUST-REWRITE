//! The animation curves and the angle helpers the pose blend uses (board row G5c3a): `AnimCurve_Eval` 0x157ed8 and
//! `AnimCurve_EvalPiece` 0x158030 (a channel's value at a frame), `AnimData_GetRootOffset` 0x1586c0, the Euler
//! angle helpers 0x202520 / `Euler_Lerp` 0x202758 / `Euler_LerpArray` 0x2027f0, `Vec3_Scale` 0x102f80 and the sine and
//! cosine pair 0x250c10. On the bytes at their addresses ([`Mem`]). Checked in the function runner
//! (`tests/animcurve.rs`).
//!
//! **A curve** starts with a 16-bit header (little endian): the low four bits are the kind, the rest (signed) the
//! length in frames. Constants are 24-bit floats: three bytes as the top three of the float, its low byte 0x80.
//! - 0 constant c;
//! - 1 linear t·a + b;
//! - 2 quadratic (t·a + b)·t + c;
//! - 3 cubic ((t·a + b)·t + c)·t + d;
//! - 4 a table of 32-bit floats, one a frame, linearly interpolated;
//! - 5 a run of pieces (each with its own header), the frame counted on through them (0 past the last);
//! - 6 a table of bytes, 7 of 16-bit values, one a frame, interpolated, then × the second constant + the first.
//!
//! Any other kind is 0.

use crate::animseq::{cvt_s_w, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn abs(v: F) -> F {
    F(f32::from_bits(v.0.to_bits() & 0x7fff_ffff))
}
/// `cvt.w.s`: toward zero, saturating
fn int(v: F) -> i32 {
    v.ee() as i32
}

/// A 24-bit float: the three bytes as bits 8..31, bit pattern 0x80 below
pub fn f24(m: &Mem, p: u32) -> F {
    let b = |k: u32| m.r8(p.wrapping_add(k)) as u32;
    F(f32::from_bits(b(2) << 24 | b(1) << 16 | b(0) << 8 | 0x80))
}

/// A little-endian 32-bit float read byte by byte (the curve data is unaligned)
fn f32le(m: &Mem, p: u32) -> F {
    let b = |k: u32| m.r8(p.wrapping_add(k)) as u32;
    F(f32::from_bits(b(3) << 24 | b(2) << 16 | b(1) << 8 | b(0)))
}

fn u16le(m: &Mem, p: u32) -> u32 {
    m.r8(p) as u32 | (m.r8(p.wrapping_add(1)) as u32) << 8
}

/// `AnimCurve_EvalPiece` 0x158030(_, piece, t): one piece's value at frame t (kinds 0..4, 6 and 7; anything else 0)
pub fn eval_piece(m: &Mem, piece: u32, t: F) -> F {
    let p = piece.wrapping_add(2);
    match m.r8(piece) & 0xf {
        0 => f24(m, p),
        1 => t * f24(m, p) + f24(m, p + 3),
        2 => t * (t * f24(m, p) + f24(m, p + 3)) + f24(m, p + 6),
        3 => t * (t * (t * f24(m, p) + f24(m, p + 3)) + f24(m, p + 6)) + f24(m, p + 9),
        4 => {
            let i = int(t);
            let at = p.wrapping_add((i as u32).wrapping_mul(4));
            let frac = t - cvt_s_w(i);
            let v0 = f32le(m, at);
            if frac.ee() == 0.0 {
                v0
            } else {
                v0 + frac * (f32le(m, at + 4) - v0)
            }
        }
        k @ (6 | 7) => {
            let i = int(t);
            let first = f24(m, p);
            let second = f24(m, p + 3);
            let frac = t - cvt_s_w(i);
            let q = |j: u32| {
                if k == 6 {
                    m.r8(p.wrapping_add(6).wrapping_add(i as u32).wrapping_add(j)) as i32
                } else {
                    u16le(m, p.wrapping_add(6).wrapping_add((i as u32).wrapping_mul(2)).wrapping_add(2 * j)) as i32
                }
            };
            let q0 = cvt_s_w(q(0));
            if frac.ee() == 0.0 {
                return q0 * second + first;
            }
            (q0 + frac * (cvt_s_w(q(1)) - q0)) * second + first
        }
        _ => F(0.0),
    }
}

/// A piece's size in bytes past its header's start, by kind and length (the run walks with it)
fn piece_size(kind: u32, len: i32) -> i32 {
    match kind {
        0 => 5,
        1 => 8,
        2 => 0xb,
        3 => 0xe,
        4 => len.wrapping_mul(4).wrapping_add(2),
        6 => len.wrapping_add(8),
        7 => len.wrapping_mul(2).wrapping_add(8),
        _ => 0,
    }
}

/// `AnimCurve_Eval` 0x157ed8 on the curve's data: a single piece, or a run of pieces (kind 5: the count in the
/// header's length field) where each covers its length − 1 frames and the frame is counted on; past them 0
pub fn eval_data(m: &Mem, data: u32, t: F) -> F {
    let h = u16le(m, data);
    if h & 0xf != 5 {
        return eval_piece(m, data, t);
    }
    let count = (h as u16 as i16 as i32) >> 4;
    let mut p = data.wrapping_add(2);
    let mut t = t;
    let mut i = 0i32;
    while i < count {
        let ph = u16le(m, p);
        let len = (ph as u16 as i16 as i32) >> 4;
        if t.ee() <= cvt_s_w(len - 1).ee() {
            return eval_piece(m, p, t);
        }
        p = p.wrapping_add(piece_size(ph & 0xf, len) as u32);
        t = t - cvt_s_w(len - 1);
        i = (i + 1) as i8 as i32;
    }
    F(0.0)
}

/// `AnimCurve_Eval` 0x157ed8(curve, t): the curve is a word holding its data's address
pub fn eval(m: &Mem, curve: u32, t: F) -> F {
    eval_data(m, m.r32(curve), t)
}

/// `AnimData_GetRootOffset` 0x1586c0(file, clip, out): the clip's (or, when it has children, its first child's) three
/// root curves at frame 0 (the header's +4 the first curve's index in the file's offsets at +0xc; the offsets from
/// the file's data at +0x10), w 0
pub fn root_offset(m: &mut Mem, file: u32, clip: u32, out: u32) {
    let v = root_offset_values(m, file, clip);
    for k in 0..4 {
        wf(m, out + 4 * k, v[k as usize]);
    }
}

/// [`root_offset`]'s four values
pub fn root_offset_values(m: &Mem, file: u32, clip: u32) -> [F; 4] {
    let mut h = m.r32(file + 8).wrapping_add(clip.wrapping_mul(0x24));
    if m.r8(h + 9) != 0 {
        h += 0x24;
    }
    let offs = m.r32(file + 0xc).wrapping_add(m.r32(h + 4).wrapping_mul(4));
    let mut v = [F(0.0); 4];
    for k in 0..3 {
        let data = m.r32(file + 0x10).wrapping_add(m.r32(offs + 4 * k));
        v[k as usize] = eval_data(m, data, F(0.0));
    }
    v
}

/// v − 2π·round(v / 2π), the rounding as the game does it (+ ½, toward zero, one down when that overshot)
fn wrap(v: F) -> F {
    let k = v * F(f32::from_bits(0x3e22_f983)) + F(0.5);
    let mut n = cvt_s_w(int(k));
    if k.ee() < n.ee() {
        n = n - F(1.0);
    }
    v - n * F(f32::from_bits(0x40c9_0fdb))
}

/// 0x202520(out, a, b): the Euler delta from a to b, each angle wrapped into a turn, or the other triple for the
/// same rotation (x + π, −b.y − a.y + π, z + π) when its angles are smaller in sum
pub fn euler_delta(m: &Mem, a: u32, b: u32) -> [F; 3] {
    let pi = F(std::f32::consts::PI);
    let dx = rf(m, b) - rf(m, a);
    let x1 = wrap(dx);
    let x2 = wrap(dx + pi);
    let dy = rf(m, b + 4) - rf(m, a + 4);
    let y1 = wrap(dy);
    let y2 = wrap(-rf(m, b + 4) - rf(m, a + 4) + pi);
    let dz = rf(m, b + 8) - rf(m, a + 8);
    let z1 = wrap(dz);
    let z2 = wrap(dz + pi);
    let s1 = abs(x1) + abs(y1) + abs(z1);
    let s2 = abs(x2) + abs(y2) + abs(z2);
    if s1.ee() < s2.ee() {
        [x1, y1, z1]
    } else {
        [x2, y2, z2]
    }
}

/// 0x202520 into memory
pub fn euler_delta_into(m: &mut Mem, out: u32, a: u32, b: u32) {
    let d = euler_delta(m, a, b);
    for k in 0..3 {
        wf(m, out + 4 * k, d[k as usize]);
    }
}

/// `Vec3_Scale` 0x102f80(out, v, k)
pub fn vec3_scale(m: &mut Mem, out: u32, v: u32, k: F) {
    let (x, y, z) = (rf(m, v) * k, rf(m, v + 4) * k, rf(m, v + 8) * k);
    wf(m, out + 8, z);
    wf(m, out, x);
    wf(m, out + 4, y);
}

/// `Euler_Lerp` 0x202758(a, b, out, t): a + t × the delta from a to b ([`euler_delta`])
pub fn euler_lerp(m: &mut Mem, a: u32, b: u32, out: u32, t: F) {
    let d = euler_delta(m, a, b);
    let v = [rf(m, a) + d[0] * t, rf(m, a + 4) + d[1] * t, rf(m, a + 8) + d[2] * t];
    for k in 0..3 {
        wf(m, out + 4 * k, v[k as usize]);
    }
}

/// `Euler_LerpArray` 0x2027f0(as, bs, ts, n): each of the n triples a (pointers in `as`) moved towards its b by its t,
/// in place
pub fn euler_lerp_array(m: &mut Mem, as_: u32, bs: u32, ts: u32, n: i32) {
    for k in 0..n.max(0) as u32 {
        let a = m.r32(as_ + 4 * k);
        let b = m.r32(bs + 4 * k);
        let t = rf(m, ts + 4 * k);
        euler_lerp(m, a, b, a, t);
    }
}

/// 0x250c10(s, c, x): the sine and cosine of x, the sine by its polynomial on the quarter turn's remainder and the
/// cosine as √(1 − sin²) (the EE's square root), swapped and signed by the quarter
pub fn sin_cos(x: F) -> (F, F) {
    let q = if x.ee() < 0.0 { x * F(f32::from_bits(0x3f22_f983)) - F(0.5) } else { x * F(f32::from_bits(0x3f22_f983)) + F(0.5) };
    let n = int(q);
    let r = x - cvt_s_w(n) * F(f32::from_bits(0x3fc9_0fdb));
    let r2 = r * r;
    let c = |b: u32| F(f32::from_bits(b));
    let p = (((r2 * c(0x3638_ef1d) + c(0xb950_0d01)) * r2 + c(0x3c08_8889)) * r2 + c(0xbe2a_aaab)) * r2 + F(1.0);
    let s = p * r;
    let co = sqrt(F(1.0) - s * s);
    match n & 3 {
        0 => (s, co),
        1 => (co, -s),
        2 => (-s, -co),
        _ => (-co, s),
    }
}

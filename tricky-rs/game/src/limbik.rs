//! The rider's bones posed from their angles and the limbs' IK (board row G7r4): `Skel_GetBoneWorldPos`
//! 0x2059f8, `Boarder_PoseBonesFromAngles` 0x205a78, `Boarder_SolveLimbIk` 0x205e98, `Boarder_GetBoneWorldPos`
//! 0x208398 and `Boarder_BuildBoneMatrices` 0x2083f8. On the bytes at their addresses ([`Mem`]); the sine pair and
//! `Math_Asin` are the game's (`animcurve::sin_cos`, `math::asin_ee`: its square root chopped as the EE's), the vector unit's sums, square roots and
//! divides as it makes them. Checked in the function runner (`tests/limbik.rs`).
//!
//! The matrices are a part's (+4, 0x40 each) or given; a bone's angles are the part's +0 (12 bytes each), its
//! offset either 0x108 further on (posing) or its record's +0x18 (the IK); its parent its record's +0x12.

use crate::animcurve::sin_cos;
use crate::animseq::{div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

type V4 = [F; 4];
type M4 = [V4; 4];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rv(m: &Mem, a: u32) -> V4 {
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wv(m: &mut Mem, a: u32, v: V4) {
    for (k, x) in v.into_iter().enumerate() {
        wf(m, a + 4 * k as u32, x);
    }
}
fn rm(m: &Mem, a: u32) -> M4 {
    [0, 1, 2, 3].map(|r| rv(m, a + 0x10 * r))
}
fn wm(m: &mut Mem, a: u32, v: &M4) {
    for (r, row) in v.iter().enumerate() {
        wv(m, a + 0x10 * r as u32, *row);
    }
}
/// vmulax .. vmaddw: `v`'s lanes weighting `a`'s rows
fn apply(a: &M4, v: V4) -> V4 {
    [0, 1, 2, 3].map(|i| ((a[0][i] * v[0] + a[1][i] * v[1]) + a[2][i] * v[2]) + a[3][i] * v[3])
}
fn mul(a: &M4, b: &M4) -> M4 {
    b.map(|row| apply(a, row))
}
/// The sum of the four lanes squared as VU0 makes it ((x + y) + 1·z + 1·w)
fn len2(v: V4) -> F {
    let s = v.map(|x| x * x);
    ((s[0] + s[1]) + F(1.0) * s[2]) + F(1.0) * s[3]
}
/// vopmula / vopmsub: a × b, w cleared
fn cross(a: V4, b: V4) -> V4 {
    [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], F(0.0)]
}
/// The rotation of three angles (x, y, z) with an offset, as the game builds it
fn rotation(ax: F, ay: F, az: F, off: [F; 3]) -> M4 {
    let (sa, ca) = sin_cos(ax);
    let (sb, cb) = sin_cos(ay);
    let (sc, cc) = sin_cos(az);
    let sab = sa * sb;
    [
        [cb * cc, -cb * sc, sb, F(0.0)],
        [sab * cc + ca * sc, ca * cc - sab * sc, -sa * cb, F(0.0)],
        [sa * sc - (ca * sb) * cc, (ca * sc) * sb + sa * cc, ca * cb, F(0.0)],
        [off[0], off[1], off[2], F(1.0)],
    ]
}
fn off3(m: &Mem, a: u32) -> [F; 3] {
    [rf(m, a), rf(m, a + 4), rf(m, a + 8)]
}

/// `Skel_GetBoneWorldPos` 0x2059f8(out, part, i, matrices): point `i` of the part (+0x1c, 16 bytes: x, y, z and
/// its bone) through its bone's matrix
pub fn bone_world_pos(m: &mut Mem, out: u32, part: u32, i: u32, mats: u32) -> u32 {
    let p = m.r32(part + 0x1c).wrapping_add(i << 4);
    let v = [rf(m, p), rf(m, p + 4), rf(m, p + 8), F(1.0)];
    let bone = m.r32(p + 0xc);
    let a = rm(m, mats.wrapping_add(bone << 6));
    wv(m, out, apply(&a, v));
    out
}

/// `Boarder_PoseBonesFromAngles` 0x205a78(part, matrix, left, right): bone 0 the matrix times its own rotation,
/// bones 1 to 12 their parents' times theirs (each its angles with the offset 0x108 on); then the two legs'
/// IK: bones 16, 17, 18 to `left`, 13, 14, 15 to `right`
pub fn pose_bones(m: &mut Mem, part: u32, matrix: u32, left: u32, right: u32) {
    let mats = m.r32(part + 4);
    let angles = m.r32(part);
    let base = rm(m, matrix);
    let r = rotation(rf(m, angles), rf(m, angles + 4), rf(m, angles + 8), off3(m, angles + 0x108));
    wm(m, mats, &mul(&base, &r));
    for b in 1..13u32 {
        let rec = m.r32(part + 0x18).wrapping_add(b * 0x54);
        let parent = m.r16(rec + 0x12) as i16 as i32;
        let p = rm(m, m.r32(part + 4).wrapping_add((parent << 6) as u32));
        let a = m.r32(part).wrapping_add(b * 12);
        let r = rotation(rf(m, a), rf(m, a + 4), rf(m, a + 8), off3(m, a + 0x108));
        let at = m.r32(part + 4).wrapping_add(b << 6);
        wm(m, at, &mul(&p, &r));
    }
    let mats = m.r32(part + 4);
    solve_limb(m, part, 0x10, 0x11, 0x12, left, mats);
    let mats = m.r32(part + 4);
    solve_limb(m, part, 0xd, 0xe, 0xf, right, mats);
}

/// `Boarder_SolveLimbIk` 0x205e98(part, upper, middle, end, target, matrices): a two-bone limb bent to reach the
/// target. The upper bone its parent's matrix times its own rotation (its record's offset). With the bones'
/// lengths (the middle's and end's offsets' x) over the distance to the target, k = l1 / d and k' = l2 / d: if
/// they reach past it (k + k' > 1) the knee's angle is asin(h / k) + asin(h / k') for h = √(2(k² + k'²) −
/// (k² − k'²)² − 1) / 2 (each clamped to ±0.99999; 0 when the root is of a negative); never below 0.2. The end
/// of the bent limb found, the upper bone is turned (about the cross of the target's direction and the end's)
/// onto the target by asin of that cross's length (at most 0.99999; not when it is 0.001 or less). The middle
/// bone the upper's times the knee's turn (−the angle about z, its offset); the end bone's place the middle's
/// times its offset.
pub fn solve_limb(m: &mut Mem, part: u32, upper: u32, middle: u32, end: u32, target: u32, mats: u32) {
    let bones = m.r32(part + 0x18);
    let rec = bones.wrapping_add(upper * 0x54);
    let up_at = mats.wrapping_add(upper << 6);
    let parent = m.r16(rec + 0x12) as i16 as i32;
    let pm = rm(m, mats.wrapping_add((parent << 6) as u32));
    let a = m.r32(part).wrapping_add(upper * 12);
    let r = rotation(rf(m, a), rf(m, a + 4), rf(m, a + 8), off3(m, rec + 0x18));
    let um = mul(&pm, &r);
    wm(m, up_at, &um);
    let joint = um[3];
    let bones = m.r32(part + 0x18);
    let (mid, endr) = (bones.wrapping_add(middle * 0x54), bones.wrapping_add(end * 0x54));
    let t = rv(m, target);
    let d = [0, 1, 2, 3].map(|k| t[k] - joint[k]);
    let inv = div(F(1.0), sqrt(len2(d)));
    let dir = d.map(|x| x * inv);
    let k2 = rf(m, endr + 0x18) * inv;
    let k1 = rf(m, mid + 0x18) * inv;
    let mut angle = F(0.0);
    if !(k1 + k2 <= F(1.0)) {
        let (a2, b2) = (k1 * k1, k2 * k2);
        let s = (a2 + b2) + (a2 + b2);
        let dd = a2 - b2;
        let q = (s - dd * dd) - F(1.0);
        if !(q < F(0.0)) {
            let h = sqrt(q) * F(0.5);
            let lim = F(f32::from_bits(0x3f7f_ff58));
            let clamp = |x: F| if x < -lim { -lim } else if lim < x { lim } else { x };
            let first = clamp(div(h, k1));
            let second = clamp(div(h, k2));
            angle = crate::math::asin_ee(first) + crate::math::asin_ee(second);
        }
    }
    if angle < F(f32::from_bits(0x3e4c_cccd)) {
        angle = F(f32::from_bits(0x3e4c_cccd));
    }
    let (s, c) = sin_cos(angle);
    let up = rm(m, up_at);
    let (mx, ex) = (rf(m, mid + 0x18), rf(m, endr + 0x18));
    let local = [mx + c * ex, s * ex, F(0.0), F(1.0)];
    let tip = apply(&up, local);
    let e = [0, 1, 2, 3].map(|k| tip[k] - joint[k]);
    let q = div(F(1.0), sqrt(len2(e)));
    let en = e.map(|x| x * q);
    let axis = cross(dir, en);
    let alen = sqrt(len2(axis));
    if F(f32::from_bits(0x3a83_126f)) < alen {
        let lim = F(f32::from_bits(0x3f7f_ff58));
        let qn = div(F(1.0), alen);
        let n = axis.map(|x| x * qn);
        let th = crate::math::asin_ee(if alen < lim { alen } else { lim });
        let (s, c) = sin_cos(th);
        let (x, y, z) = (n[0], n[1], n[2]);
        let one_c = F(1.0) - c;
        let f1 = one_c * x;
        let (f8, f9, f6) = (s * z, s * y, s * x);
        let f3 = one_c * y;
        let f4 = one_c * z;
        let rot = [
            [f1 * x + c, f3 * x - f8, f4 * x + f9, F(0.0)],
            [f1 * y + f8, f3 * y + c, f4 * y - f6, F(0.0)],
            [f1 * z - f9, f3 * z + f6, f4 * z + c, F(0.0)],
            [up[3][0], up[3][1], up[3][2], up[3][3]],
        ];
        let bare = [up[0], up[1], up[2], [F(0.0), F(0.0), F(0.0), F(1.0)]];
        wm(m, up_at, &mul(&rot, &bare));
    }
    let up = rm(m, up_at);
    let mid_at = mats.wrapping_add(middle << 6);
    let r2 = rotation(F(0.0), F(0.0), -angle, off3(m, mid + 0x18));
    let mm = mul(&up, &r2);
    wm(m, mid_at, &mm);
    let off = off3(m, endr + 0x18);
    let p = apply(&mm, [off[0], off[1], off[2], F(1.0)]);
    wv(m, mats.wrapping_add(end << 6) + 0x30, p);
}

/// `Boarder_GetBoneWorldPos` 0x208398(out, skin, kind, i, matrices): `bone_world_pos` on the skin's part of that
/// kind (+0x3cf8); `out`
pub fn boarder_bone_world_pos(m: &mut Mem, out: u32, skin: u32, kind: u32, i: u32, mats: u32) -> u32 {
    let part = m.r32(skin.wrapping_add(kind << 2) + 0x3cf8);
    bone_world_pos(m, out, part, i, mats)
}

/// `Boarder_BuildBoneMatrices` 0x2083f8(skin, matrix, left, right): `pose_bones` on the skin's kind-0 part
pub fn build_bone_matrices(m: &mut Mem, skin: u32, matrix: u32, left: u32, right: u32) {
    let part = m.r32(skin + 0x3cf8);
    pose_bones(m, part, matrix, left, right);
}

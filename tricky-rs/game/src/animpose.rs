//! The pose blend (board row G5c3b): `AnimCtl_BlendPose` 0x15f7c8, which hands every weighted source of the
//! controller's sequences (row G5a2) to the rider's pose objects, and the pose object `PoseBlend` (0x394 bytes) that
//! evaluates and mixes them: `PoseBlend_Begin` 0x202868 / `_Add` 0x202930 / `_Finish` 0x202990 / `_EvalCurves`
//! 0x2029d8, the curve binding 0x159910 and the skeleton blend `FUN_00202b00`. On the bytes at their addresses
//! ([`Mem`]); the curves and angles are row G5c3a's ([`crate::animcurve`]). Checked in the function runner
//! (`tests/animpose.rs`).
//!
//! **A pose object:**
//! - +0 each bone's rotation (Euler, 0xc bytes), +0x108 each bone's translation;
//! - +0x210 the channels' values when there is no skeleton;
//! - +0x288 done, +0x28c the sources added (at most 10);
//! - each source's tree +0x290, frame +0x2b8, weight +0x2e0, facing +0x308, stance +0x330, bone mask +0x358;
//! - +0x380 the translations' scale, +0x384 the part id, +0x388 the skeleton (0x54 bytes a bone), +0x38c the bones
//!   or channels, +0x390 the mask's first bit.
//!
//! **A skeleton bone:** +0x10 its parent (−1 the root), +0x14 flags (1 translation curves, 4 translation from the
//! bind pose, 2 rotation curves, 8 rotation from the bind pose), +0x16 its mirror, +0x18 the bind translation, +0x24
//! the bind rotation, +0x30 / +0x3c the mirror's rotation offset and scale, +0x48 the mirror's translation scale.
//!
//! **A source tree** (`SkelAnim`): +8 its clip header, +0xc its children, +0x2c each bone's first channel, +0xa4 each
//! channel's curve, +0x2a4 / +0x2a8 the clip's channel offsets and data until bound.

use crate::animcurve as cv;
use crate::animseq::div;
use crate::pf::Mem;
use crate::ps2f::F;

/// The skeleton blend's scratch (0x3d2170..): the weights, rotations and translations of the masked sources' pass
/// (0) and the others' (1), each source's translations and rotations, and the Euler lerp lists
pub const W0: u32 = 0x3d2170;
pub const R0: u32 = 0x3d21c8;
pub const T0: u32 = 0x3d22d0;
pub const W1: u32 = 0x3d23d8;
pub const R1: u32 = 0x3d2430;
pub const T1: u32 = 0x3d2538;
pub const TP: u32 = 0x3d2640;
pub const TR: u32 = 0x3d2748;
pub const LERP_B: u32 = 0x3d2850;
pub const LERP_A: u32 = 0x3d28a8;
pub const LERP_T: u32 = 0x3d2900;
/// The six static guards the blend sets on its first run (0x341f9c..0x341fb0) and the matrix it turns the root by
/// (0x346440)
pub const GUARDS: u32 = 0x341f9c;
pub const MATRIX: u32 = 0x346440;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn copy3(m: &mut Mem, dst: u32, src: u32) {
    for k in 0..3 {
        m.w32(dst + 4 * k, m.r32(src + 4 * k));
    }
}

/// `PoseBlend_Begin` 0x202868: no sources; with a skeleton each bone starts at its bind rotation and translation,
/// without one every channel at 0
pub fn begin(m: &mut Mem, p: u32) {
    let skel = m.r32(p + 0x388);
    m.w32(p + 0x28c, 0);
    m.w32(p + 0x288, 0);
    let n = m.r32(p + 0x38c) as i32;
    for j in 0..n.max(0) as u32 {
        if skel != 0 {
            let b = skel.wrapping_add(0x54 * j);
            copy3(m, p + 0xc * j, b + 0x24);
            copy3(m, p + 0x108 + 0xc * j, b + 0x18);
        } else {
            m.w32(p + 0x210 + 4 * j, 0);
        }
    }
}

/// `PoseBlend_Add` 0x202930(tree, stance, mask, frame, weight, facing): the source noted (ten at most)
pub fn add(m: &mut Mem, p: u32, tree: u32, stance: u32, mask: u32, t: F, w: F, facing: F) {
    let c = m.r32(p + 0x28c) as i32;
    if c >= 10 {
        return;
    }
    let at = p.wrapping_add((c as u32).wrapping_mul(4));
    wf(m, at + 0x2b8, t);
    wf(m, at + 0x2e0, w);
    m.w32(at + 0x330, stance);
    wf(m, at + 0x308, facing);
    m.w32(at + 0x358, mask);
    m.w32(at + 0x290, tree);
    m.w32(p + 0x28c, (c + 1) as u32);
}

/// 0x159910(tree, skeleton, n): once (while the tree has its clip's offsets and data), each bone's first channel
/// noted and its channels' curves bound: without a skeleton one channel a bone, with one three for translation curves
/// (flag 1) and three for rotation curves (flag 2)
pub fn bind(m: &mut Mem, tree: u32, skel: u32, n: i32) {
    let offs = m.r32(tree + 0x2a4);
    if offs == 0 || m.r32(tree + 0x2a8) == 0 {
        return;
    }
    let mut at = offs.wrapping_add(m.r32(m.r32(tree + 8) + 4).wrapping_mul(4));
    let mut ch = 0u32;
    for j in 0..n.max(0) as u32 {
        m.w32(tree + 0x2c + 4 * j, ch);
        let groups = if skel == 0 {
            1
        } else {
            let f = m.r16(skel.wrapping_add(0x54 * j) + 0x14);
            (f & 1 != 0) as u32 * 3 + (f & 2 != 0) as u32 * 3
        };
        for _ in 0..groups {
            let v = m.r32(tree + 0x2a8).wrapping_add(m.r32(at));
            m.w32(tree.wrapping_add(ch.wrapping_mul(4)) + 0xa4, v);
            ch = ch.wrapping_add(1);
            at = at.wrapping_add(4);
        }
    }
    m.w32(tree + 0x2a8, 0);
    m.w32(tree + 0x2a4, 0);
}

/// A channel's curve value: the tree's curve for channel `ch` at frame t
fn channel(m: &Mem, tree: u32, ch: u32, t: F) -> F {
    cv::eval(m, tree.wrapping_add(ch.wrapping_mul(4)).wrapping_add(0xa4), t)
}

/// `PoseBlend_EvalCurves` 0x2029d8 (no skeleton): each source bound, then each channel's curve × the source's
/// weight added to +0x210; done
pub fn eval_curves(m: &mut Mem, p: u32) {
    let c = m.r32(p + 0x28c) as i32;
    for s in 0..c.max(0) as u32 {
        let tree = m.r32(p + 0x290 + 4 * s);
        bind(m, tree, m.r32(p + 0x388), m.r32(p + 0x38c) as i32);
        let n = m.r32(p + 0x38c) as i32;
        for j in 0..n.max(0) as u32 {
            let tree = m.r32(p + 0x290 + 4 * s);
            let ch = m.r32(tree + 0x2c + 4 * j);
            let v = channel(m, tree, ch, rf(m, p + 0x2b8 + 4 * s));
            let acc = rf(m, p + 0x210 + 4 * j) + rf(m, p + 0x2e0 + 4 * s) * v;
            wf(m, p + 0x210 + 4 * j, acc);
        }
    }
    m.w32(p + 0x288, 1);
}

/// The root's translation turned by the facing: R(facing about (0, 0, 1)) × the matrix at 0x346440 (on VU0), its
/// top 3 × 3 applied to the row vector
fn turn_root(m: &mut Mem, v: u32, facing: F) {
    let mat: Vec<[F; 4]> = (0..4).map(|r| [0, 1, 2, 3].map(|c| rf(m, MATRIX + 0x10 * r + 4 * c))).collect();
    let (ax, ay, az) = (F(0.0), F(0.0), F(1.0));
    let (s, c) = cv::sin_cos(facing);
    let k = F(1.0) - c;
    let (f2, f5, f1) = (k * az, k * ax, k * ay);
    let (f13, f12) = (s * az, s * ax);
    let f9 = f2 * az;
    let f10 = f5 * az;
    let f3 = f2 * ax;
    let f7 = f5 * ax;
    let f4 = f1 * az;
    let f0 = f1 * ax;
    let f8 = s * ay;
    let f2b = f2 * ay;
    let f5b = f5 * ay;
    let f1b = f1 * ay;
    let rot = [
        [f7 + c, f0 - f13, f3 + f8, F(0.0)],
        [f5b + f13, f1b + c, f2b - f12, F(0.0)],
        [f10 - f8, f4 + f12, f9 + c, F(0.0)],
        [F(0.0), F(0.0), F(0.0), F(1.0)],
    ];
    // vmulax / vmadday / vmaddaz / vmaddw: each row of R times the matrix
    let mut a = [[F(0.0); 4]; 4];
    for (r, row) in rot.iter().enumerate() {
        for i in 0..4 {
            a[r][i] = ((mat[0][i] * row[0] + mat[1][i] * row[1]) + mat[2][i] * row[2]) + mat[3][i] * row[3];
        }
    }
    let (x, y, z) = (rf(m, v), rf(m, v + 4), rf(m, v + 8));
    let nx = (a[0][0] * x + a[1][0] * y) + a[2][0] * z;
    let ny = (a[0][1] * x + a[1][1] * y) + a[2][1] * z;
    let nz = (a[0][2] * x + a[1][2] * y) + a[2][2] * z;
    wf(m, v, nx);
    wf(m, v + 4, ny);
    wf(m, v + 8, nz);
}

/// `FUN_00202b00` (with a skeleton): the sources' bones evaluated and mixed. The guards set; both passes' weights
/// cleared. Pass 0 takes the sources with a bone mask (+0x358 not −1), pass 1 the others; for each source (bound
/// first) each bone in its mask and not already fully weighted in pass 0:
/// - the translation from its three curves × the scale (flag 1) or the bind pose (the mirror bone's with a stance);
///   the root's turned by the facing; with a stance × the mirror's scale;
/// - the rotation from its three curves (flag 2) or the bind pose; the root's z plus the facing; with a stance the
///   mirror's offset + rotation × scale;
/// - mixed into the pass: the first sets it and its weight; later ones blend by weight (translations by scaling,
///   rotations by `Euler_LerpArray` after the source's bones).
///
/// Then each bone: pass 0 at weight 1, pass 1 at weight 0, else the translations mixed by pass 0's weight and the
/// rotation moved from pass 1's towards pass 0's by it.
pub fn blend_skeleton(m: &mut Mem, p: u32) {
    for k in 0..6 {
        if m.r32(GUARDS + 4 * k) == 0 {
            m.w32(GUARDS + 4 * k, 1);
        }
    }
    let n = m.r32(p + 0x38c) as i32;
    for j in 0..n.max(0) as u32 {
        m.w32(W0 + 4 * j, 0);
        m.w32(W1 + 4 * j, 0);
    }
    for pass in 0..2u32 {
        let (w, r, t) = if pass == 0 { (W0, R0, T0) } else { (W1, R1, T1) };
        let mut s = 0u32;
        while (s as i32) < m.r32(p + 0x28c) as i32 {
            let mask = m.r32(p + 0x358 + 4 * s);
            if (pass == 0) == (mask == u32::MAX) {
                s += 1;
                continue;
            }
            let tree = m.r32(p + 0x290 + 4 * s);
            bind(m, tree, m.r32(p + 0x388), m.r32(p + 0x38c) as i32);
            let mut queued = 0u32;
            let mut b = 0u32;
            while (b as i32) < m.r32(p + 0x38c) as i32 {
                let shift = b.wrapping_add(m.r32(p + 0x390));
                if (mask as i32 >> (shift & 31)) & 1 == 0 || rf(m, W0 + 4 * b).ee() == 1.0 {
                    b += 1;
                    continue;
                }
                let skel = m.r32(p + 0x388);
                let stance = m.r32(p + 0x330 + 4 * s);
                let bone = skel.wrapping_add(0x54 * b);
                let src = if stance != 0 { m.r16(bone + 0x16) as i16 as i32 as u32 } else { b };
                let flags = m.r16(bone + 0x14) as u32;
                let tree = m.r32(p + 0x290 + 4 * s);
                let mut ch = m.r32(tree.wrapping_add(src.wrapping_mul(4)) + 0x2c);
                let time = rf(m, p + 0x2b8 + 4 * s);
                let tp = TP + 0xc * b;
                let tr = TR + 0xc * b;
                if flags & 5 != 0 {
                    if flags & 1 != 0 {
                        for k in 0..3 {
                            let v = channel(m, tree, ch, time);
                            wf(m, tp + 4 * k, v);
                            ch = ch.wrapping_add(1);
                        }
                        let sc = rf(m, p + 0x380);
                        for k in 0..3 {
                            let v = rf(m, tp + 4 * k) * sc;
                            wf(m, tp + 4 * k, v);
                        }
                    } else {
                        copy3(m, tp, skel.wrapping_add(src.wrapping_mul(0x54)) + 0x18);
                    }
                    if m.r16(bone + 0x10) as i16 == -1 {
                        turn_root(m, tp, rf(m, p + 0x308 + 4 * s));
                    }
                    if stance != 0 {
                        for k in 0..3 {
                            let v = rf(m, tp + 4 * k) * rf(m, bone + 0x48 + 4 * k);
                            wf(m, tp + 4 * k, v);
                        }
                    }
                }
                if flags & 0xa != 0 {
                    if flags & 2 != 0 {
                        for k in 0..3 {
                            let v = channel(m, tree, ch, time);
                            wf(m, tr + 4 * k, v);
                            ch = ch.wrapping_add(1);
                        }
                    } else {
                        copy3(m, tr, skel.wrapping_add(src.wrapping_mul(0x54)) + 0x24);
                    }
                    if m.r16(bone + 0x10) as i16 == -1 {
                        let v = rf(m, tr + 8) + rf(m, p + 0x308 + 4 * s);
                        wf(m, tr + 8, v);
                    }
                    if stance != 0 {
                        for k in 0..3 {
                            let v = rf(m, bone + 0x30 + 4 * k) + rf(m, tr + 4 * k) * rf(m, bone + 0x3c + 4 * k);
                            wf(m, tr + 4 * k, v);
                        }
                    }
                }
                let have = rf(m, w + 4 * b);
                let sw = rf(m, p + 0x2e0 + 4 * s);
                if have.ee() == 0.0 {
                    wf(m, w + 4 * b, sw);
                    if flags & 5 != 0 {
                        copy3(m, t + 0xc * b, tp);
                    }
                    if flags & 0xa != 0 {
                        copy3(m, r + 0xc * b, tr);
                    }
                } else {
                    let total = sw + have;
                    let inv = div(F(1.0), total);
                    wf(m, w + 4 * b, total);
                    let keep = have * inv;
                    let take = sw * inv;
                    if flags & 5 != 0 {
                        let to = t + 0xc * b;
                        let v: Vec<F> = (0..3).map(|k| rf(m, tp + 4 * k) * take + rf(m, to + 4 * k) * keep).collect();
                        for k in 0..3 {
                            wf(m, to + 4 * k, v[k as usize]);
                        }
                    }
                    if flags & 0xa != 0 {
                        m.w32(LERP_B + 4 * queued, tr);
                        m.w32(LERP_A + 4 * queued, r + 0xc * b);
                        wf(m, LERP_T + 4 * queued, take);
                        queued += 1;
                    }
                }
                b += 1;
            }
            if 0 < m.r32(p + 0x38c) as i32 {
                cv::euler_lerp_array(m, LERP_A, LERP_B, LERP_T, queued as i32);
            }
            s += 1;
        }
    }
    let mut queued = 0u32;
    let n = m.r32(p + 0x38c) as i32;
    for j in 0..n.max(0) as u32 {
        let w0 = rf(m, W0 + 4 * j);
        let flags = m.r16(m.r32(p + 0x388).wrapping_add(0x54 * j) + 0x14) as u32;
        let (rot, tr) = (p + 0xc * j, p + 0x108 + 0xc * j);
        if w0.ee() == 1.0 {
            if flags & 5 != 0 {
                copy3(m, tr, T0 + 0xc * j);
            }
            if flags & 0xa != 0 {
                copy3(m, rot, R0 + 0xc * j);
            }
        } else if w0.ee() == 0.0 {
            if flags & 5 != 0 {
                copy3(m, tr, T1 + 0xc * j);
            }
            if flags & 0xa != 0 {
                copy3(m, rot, R1 + 0xc * j);
            }
        } else {
            if flags & 5 != 0 {
                let rest = F(1.0) - w0;
                let v: Vec<F> = (0..3).map(|k| rf(m, T0 + 0xc * j + 4 * k) * w0 + rf(m, T1 + 0xc * j + 4 * k) * rest).collect();
                for k in 0..3 {
                    wf(m, tr + 4 * k, v[k as usize]);
                }
            }
            if flags & 0xa != 0 {
                copy3(m, rot, R1 + 0xc * j);
                m.w32(LERP_B + 4 * queued, R0 + 0xc * j);
                m.w32(LERP_A + 4 * queued, rot);
                wf(m, LERP_T + 4 * queued, w0);
                queued += 1;
            }
        }
    }
    cv::euler_lerp_array(m, LERP_A, LERP_B, LERP_T, queued as i32);
}

/// `PoseBlend_Finish` 0x202990: with sources, the skeleton blend (with a skeleton) or the channels' sums
pub fn finish(m: &mut Mem, p: u32) {
    if m.r32(p + 0x28c) == 0 {
        return;
    }
    if m.r32(p + 0x388) != 0 {
        blend_skeleton(m, p);
    } else {
        eval_curves(m, p);
    }
}

/// `AnimCtl_BlendPose` 0x15f7c8(ctl): the sequences of the three layers with weight above 0 (newest first, layer by
/// layer) listed; then each of the rider's pose objects (rider+0x7bc of them, 0x394 bytes from +0x7c0) begun, given
/// every listed sequence's sources with weight whose children are of its part (+0x384), each with the sequence's
/// stance, mask, the source's frame, the source's weight × the sequence's and the facing, and finished
pub fn blend_pose(m: &mut Mem, ctl: u32) {
    let mut listed = Vec::new();
    for k in 0..3 {
        let l = crate::animctl::layer(ctl, k);
        let mut s = m.r32(l + 0x30);
        while s != l + 0x38 {
            if 0.0 < rf(m, s + 8).ee() {
                listed.push(s);
            }
            s = m.r32(s + 0x30);
        }
    }
    let mut k = 0u32;
    while (k as i32) < m.r32(m.r32(ctl + 0x24) + 0x7bc) as i32 {
        let rider = m.r32(ctl + 0x24);
        let p = rider.wrapping_add(0x7c0).wrapping_add(0x394 * k);
        let part = m.r32(p + 0x384) as i32;
        begin(m, p);
        for &s in &listed {
            let n = m.r32(s + 0x44) as i32;
            for i in 0..n.max(0) as u32 {
                if rf(m, s + 0x54 + 4 * i).ee() == 0.0 {
                    continue;
                }
                let tree = m.r32(s + 0x78 + 4 * i);
                let mut c = 0u32;
                while c < m.r8(m.r32(tree + 8) + 9) as u32 {
                    let child = m.r32(tree + 0xc + 4 * c);
                    if m.r8(m.r32(child + 8) + 8) as i8 as i32 == part {
                        let w = rf(m, s + 0x54 + 4 * i) * rf(m, s + 8);
                        add(m, p, child, m.r32(s + 0x64), m.r32(s + 0x18), rf(m, s + 0x48 + 4 * i), w, rf(m, s + 0x14));
                    }
                    c += 1;
                }
            }
        }
        finish(m, p);
        k += 1;
    }
}

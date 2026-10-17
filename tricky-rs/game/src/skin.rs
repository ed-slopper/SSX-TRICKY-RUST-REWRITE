//! The rider's skinning (board row G7g3): `RiderSkin_SkinMatrices` 0x207c30, `RiderSkin_SetBoardMatrix`
//! 0x207f18, `RiderSkin_CopyBoneMatrices` 0x208400 and the part's own `SkinPart_CopyBoneMatrices` 0x206818. On
//! the bytes at their addresses ([`Mem`]); the renderer's virtuals are calls out ([`ParticleDrawEnv`]). Checked
//! in the function runner (`tests/skin.rs`).
//!
//! **The rider's skin** (as these use it): +0x3cf8 the skeletons the parts copy from, +0x3d00 15 parts (0 for
//! none), +0x3d80 the bones' matrices (0x40 each), +0x3f18 the model, +0x3f20 whether the board is drawn.
//! **A model**: +0 its parts' count, +4 the parts (0x48 each: +8 the bind matrices, +0xc the mesh), and per LOD
//! (0x5900 each) +0x50 a word and +0x54 the LOD's data for the renderer. **A mesh**: +0x42 its bones (a half),
//! +0x4a its slot in the parts' table (a byte). **A skin part** (0x206818's): +0 its bones' angles (12 bytes
//! each) and, 0x108 on, their offsets; +4 its matrices; +0xc the mesh; +0x18 the bones (0x54 each: +0x10 which
//! skeleton, +0x12 which of its bones).
//!
//! What the game builds on its stack for the renderer, the port builds in `scratch` (STANDIN: the frame's
//! place; the same bytes): the parts' table (0x80 bytes) for the skinning, the board's matrix and two vectors
//! (0x60 bytes) for the board.

use crate::animcurve::sin_cos;
use crate::particledraw::{ParticleDrawEnv, APP};
use crate::pf::Mem;
use crate::ps2f::F;

type V = [F; 4];
type M4 = [V; 4];

/// Where the skinned matrices go (0x40 each)
pub const SKINNED: u32 = 0x341fc0;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn rm(m: &Mem, a: u32) -> M4 {
    [0, 1, 2, 3].map(|i| [0, 1, 2, 3].map(|j| rf(m, a + 0x10 * i + 4 * j)))
}
fn wm(m: &mut Mem, a: u32, x: &M4) {
    for i in 0..4 {
        for j in 0..4 {
            m.w32(a + 0x10 * i as u32 + 4 * j as u32, x[i][j].0.to_bits());
        }
    }
}
fn copy(m: &mut Mem, to: u32, from: u32, bytes: u32) {
    for k in 0..bytes / 4 {
        let w = m.r32(from + 4 * k);
        m.w32(to + 4 * k, w);
    }
}
/// Row i of the answer: the rows of `a` by row i of `p` (VU0's matrix product)
fn mat_mul(p: &M4, a: &M4) -> M4 {
    p.map(|b| [0, 1, 2, 3].map(|c| ((a[0][c] * b[0] + a[1][c] * b[1]) + a[2][c] * b[2]) + a[3][c] * b[3]))
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}
fn vcall(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}

/// `RiderSkin_SkinMatrices` 0x207c30(skin, lod): each part's bones times their bind matrices (bone k of part p
/// is bone base(p) + k of +0x3d80, by the part's k-th bind matrix) into 0x341fc0 on from base(p), base(p) the
/// bones of the parts before. The parts' table (32 words, cleared) gets base(p) at the mesh's slot. Then the
/// renderer's +0x310 (0x341fc0, the table, the LOD's data, its word, 0) and +0x2e0 (the model + 0x5900 × lod,
/// +0x3d80).
pub fn skin_matrices(m: &mut Mem, env: &mut dyn ParticleDrawEnv, skin: u32, lod: u32, scratch: u32) {
    skin_parts(m, skin, scratch);
    let r = renderer(m);
    let at = m.r32(skin + 0x3f18).wrapping_add(lod.wrapping_mul(0x5900));
    let word = m.r32(at + 0x50);
    vcall(m, env, r, 0x310, &[SKINNED, scratch, at + 0x54, word, 0]);
    let at = m.r32(skin + 0x3f18).wrapping_add(lod.wrapping_mul(0x5900));
    let bones = m.r32(skin + 0x3d80);
    vcall(m, env, r, 0x2e0, &[at, bones]);
}

/// The skinning both `SkinMatrices` and the renderer's +0x2f8 begin with: the parts' table (32 words) cleared,
/// then each part's bones times their bind matrices into 0x341fc0, the table getting each part's first bone at
/// its mesh's slot
pub fn skin_parts(m: &mut Mem, skin: u32, scratch: u32) {
    for k in 0..0x20 {
        m.w32(scratch + 4 * k, 0);
    }
    let model = m.r32(skin + 0x3f18);
    let mut base = 0u32;
    let mut i = 0i32;
    while i < m.r32(model) as i32 {
        let part = m.r32(model + 4).wrapping_add((i * 0x48) as u32);
        let mesh = m.r32(part + 0xc);
        let slot = m.r8(mesh + 0x4a) as i8 as i32;
        m.w32(scratch.wrapping_add((slot << 2) as u32), base);
        let mut j = 0u32;
        while (j as i32) < m.r16(m.r32(part + 0xc) + 0x42) as i16 as i32 {
            let bone = base.wrapping_add(j) << 6;
            let a = rm(m, m.r32(skin + 0x3d80).wrapping_add(bone));
            let p = rm(m, m.r32(part + 8).wrapping_add(j << 6));
            wm(m, SKINNED.wrapping_add(bone), &mat_mul(&p, &a));
            j += 1;
        }
        base = base.wrapping_add(m.r16(m.r32(part + 0xc) + 0x42) as i16 as i32 as u32);
        i += 1;
    }
}

/// `RiderSkin_SetBoardMatrix` 0x207f18(skin, board, a, b): nothing unless the board is drawn (+0x3f20). The
/// renderer's +0x2f0 (0x19, +0x3d80, a copy of the board's matrix +0x110 and of its +0x150 and +0x160), then
/// +0x2f8 (whether a and b have the same y, 0, the skin, b)
pub fn set_board_matrix(m: &mut Mem, env: &mut dyn ParticleDrawEnv, skin: u32, board: u32, a: u32, b: u32, scratch: u32) {
    if m.r32(skin + 0x3f20) == 0 {
        return;
    }
    copy(m, scratch, board + 0x110, 0x40);
    copy(m, scratch + 0x40, board + 0x150, 0x10);
    copy(m, scratch + 0x50, board + 0x160, 0x10);
    let r = renderer(m);
    let bones = m.r32(skin + 0x3d80);
    vcall(m, env, r, 0x2f0, &[0x19, bones, scratch, scratch + 0x40, scratch + 0x50]);
    let same = (rf(m, a + 4) == rf(m, b + 4)) as u32;
    let r = renderer(m);
    vcall(m, env, r, 0x2f8, &[same, 0, skin, b]);
}

/// `RiderSkin_CopyBoneMatrices` 0x208400(skin): each of the 15 parts there is (+0x3d00) copies its bones
/// ([`part_copy_bone_matrices`], from +0x3cf8)
pub fn copy_bone_matrices(m: &mut Mem, skin: u32) {
    for k in 0..15 {
        let part = m.r32(skin + 0x3d00 + 4 * k);
        if part != 0 {
            part_copy_bone_matrices(m, part, skin + 0x3cf8);
        }
    }
}

/// `SkinPart_CopyBoneMatrices` 0x206818(part, skeletons): each of the part's bones copies the matrix of its
/// skeleton's bone (the word at `skeletons` + 4 × which, its +4 the matrices), then is that times a turn and
/// shift of its own: the angles (x, y, z) and the offset at +0 (12 bytes a bone, the offsets 0x108 on).
pub fn part_copy_bone_matrices(m: &mut Mem, part: u32, skeletons: u32) {
    let mut j = 0u32;
    while (j as i32) < m.r16(m.r32(part + 0xc) + 0x42) as i16 as i32 {
        let e = m.r32(part + 0x18).wrapping_add(j.wrapping_mul(0x54));
        let which = m.r16(e + 0x10) as i16 as i32 as u32;
        let bone = m.r16(e + 0x12) as i16 as i32 as u32;
        let skel = m.r32(skeletons.wrapping_add(which << 2));
        let from = m.r32(skel + 4).wrapping_add(bone << 6);
        let to = m.r32(part + 4).wrapping_add(j << 6);
        copy(m, to, from, 0x40);
        let at = m.r32(part).wrapping_add(j.wrapping_mul(12));
        let (sx, cx) = sin_cos(rf(m, at));
        let (sy, cy) = sin_cos(rf(m, at + 4));
        let (sz, cz) = sin_cos(rf(m, at + 8));
        let (sxsy, cxsy, cxsz) = (sx * sy, cx * sy, cx * sz);
        let z = F(0.0);
        let off = at + 0x108;
        let turn: M4 = [
            [cy * cz, (-cy) * sz, sy, z],
            [sxsy * cz + cxsz, cx * cz - sxsy * sz, (-sx) * cy, z],
            [sx * sz - cxsy * cz, cxsz * sy + sx * cz, cx * cy, z],
            [rf(m, off), rf(m, off + 4), rf(m, off + 8), F(1.0)],
        ];
        let to = m.r32(part + 4).wrapping_add(j << 6);
        let x = mat_mul(&turn, &rm(m, to));
        wm(m, to, &x);
        j += 1;
    }
}

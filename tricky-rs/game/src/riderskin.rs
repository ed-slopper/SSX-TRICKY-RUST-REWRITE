//! The rider skin (board row G7r3): `RiderSkin_*` 0x206f80..0x2083f8 and the `Boarder_*` hand-offs at its end.
//! On the bytes at their addresses ([`Mem`]); the renderer's virtuals, `memset` and the deletes are calls out
//! ([`TextBoxEnv`]); the sine pair, `Vec4_Scale` 0x102f50 and `Mat4_TransformVec4` 0x178078 are done as the
//! game's. Checked in the function runner (`tests/riderskin.rs`).
//!
//! **The skin** (0x3f24 bytes): +0 / +0x10 its two banks; +0x20 how many bone entries, +0x24 the entries (0x394
//! bytes: +0x380 the scale, +0x384 the kind, +0x388 the bones, +0x38c how many, +0x390 the first matrix);
//! +0x3cf8 the part of each kind (17), +0x3d3c the entry of each kind (17); +0x3d80 the base matrices; +0x3f14
//! wanted; +0x3f18 the LODs (0x5900 bytes each, `skinpart`), +0x3f1c how many.
//!
//! What the game builds on its stack (a kind table) the port builds in `scratch` (STANDIN: 0x80 bytes).

use crate::animcurve::sin_cos;
use crate::bdrload::{align, bank_alloc, bank_delete, bank_init, DELETE};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::skinpart::{lod_resolve_materials, lod_resolve_names, lod_scale, part_reset, set_child_matrices};
use crate::textbox::TextBoxEnv;

pub const MEMSET: u32 = 0x2fb468;
/// The skinned matrices the renderer draws from, and the matrix the bind pose is put in
pub const SKINNED: u32 = 0x341fc0;
pub const BIND_MATRIX: u32 = 0x346440;
pub const POSE_BONES: u32 = 0x205a78;
pub const BONE_WORLD_POS: u32 = 0x2059f8;
const APP: u32 = 0x337c58;
const LOD: u32 = 0x5900;
const ENTRY: u32 = 0x394;

type M4 = [[F; 4]; 4];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rm(m: &Mem, a: u32) -> M4 {
    [0, 1, 2, 3].map(|r| [0, 1, 2, 3].map(|c| rf(m, a + 0x10 * r + 4 * c)))
}
fn wm(m: &mut Mem, a: u32, v: &M4) {
    for r in 0..4 {
        for c in 0..4 {
            wf(m, a + 0x10 * r as u32 + 4 * c as u32, v[r][c]);
        }
    }
}
/// vmulax / vmadday / vmaddaz / vmaddw: each of `b`'s rows weighting `a`'s rows
fn mul(a: &M4, b: &M4) -> M4 {
    b.map(|row| [0, 1, 2, 3].map(|i| ((a[0][i] * row[0] + a[1][i] * row[1]) + a[2][i] * row[2]) + a[3][i] * row[3]))
}
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let func = m.r32(vt + off + 4);
    let mut w = vec![this];
    w.extend_from_slice(args);
    env.call(m, func, &w, &[])
}
fn lod_of(m: &Mem, s: u32, i: u32) -> u32 {
    m.r32(s + 0x3f18).wrapping_add(i.wrapping_mul(LOD))
}
fn count(m: &Mem, h: u32, o: u32) -> i32 {
    m.r16(h + o) as i16 as i32
}

/// `RiderSkin_Init` 0x206f80(s): the two banks empty, no entries; the 17 entries scale 1, no kind, unwanted (each
/// after two empty waits); nothing wanted, no LODs; `s`
pub fn init(m: &mut Mem, s: u32) -> u32 {
    bank_init(m, s);
    bank_init(m, s + 0x10);
    m.w32(s + 0x20, 0);
    for k in 0..17u32 {
        let e = s + 0x24 + k * ENTRY;
        m.w32(e + 0x390, 0);
        wf(m, e + 0x380, F(1.0));
        m.w32(e + 0x384, 0xffff_ffff);
    }
    for o in [0x3f14, 0x3f18, 0x3f1c, 0x3f20] {
        m.w32(s + o, 0);
    }
    s
}

/// `RiderSkin_Delete` 0x207040(s, flags): reset, the banks' memory freed (2), deleted with flag 1
pub fn delete(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, flags: u32) {
    reset(m, s);
    bank_delete(m, env, s + 0x10, 2);
    bank_delete(m, env, s, 2);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[s], &[]);
    }
}

/// `RiderSkin_Reset` 0x2070a8(s): no LODs, entries or wish; both banks aligned afresh
pub fn reset(m: &mut Mem, s: u32) {
    for o in [0x3f18, 0x3f1c, 0x20, 0x3f20, 0x3f14] {
        m.w32(s + o, 0);
    }
    align(m, s);
    align(m, s + 0x10);
}

/// `RiderSkin_ClearParts` 0x2070e8(s): each LOD's kind-1 part (if any) reset; no wish, no entries; the second
/// bank aligned afresh
pub fn clear_parts(m: &mut Mem, s: u32) {
    let mut i = 0i32;
    while i < m.r32(s + 0x3f1c) as i32 {
        let lod = lod_of(m, s, i as u32);
        let k = m.r32(lod + 0xc) as i32;
        let part = if k < 0 { 0 } else { m.r32(lod + 4).wrapping_add((k as u32).wrapping_mul(0x48)) };
        if part != 0 {
            part_reset(m, part);
        }
        i += 1;
    }
    m.w32(s + 0x3f14, 0);
    m.w32(s + 0x20, 0);
    align(m, s + 0x10);
}

/// `RiderSkin_Bind` 0x207198(s, base; f12 scale): the skin's parts given their matrices. No part or entry of any
/// kind to start. For each LOD:
/// - each part with bones or a +0x48 takes the entry of its kind (a new one at the end if none), its matrices
///   after the bones of the entries before (+4 in `base`, +8 in the LOD's from +0x4c70); a new entry keeps the
///   kind, and the part as that kind's: its bones (or with a +0x48 none) and their count (or the +0x48), and where
///   its matrices start;
/// - each unscaled part's bones a matrix of their angles (x, y, z) and offset;
/// - each unscaled part's bones attached (+0x10, a kind) to that kind's part: times its parent bone's (+0x12)
///   matrix there;
/// - the bind pose's inverse: the first time (+0x4c) the LOD's 50 matrices, after that only the 20th, each turned
///   into its inverse (the 3 × 3 transposed, the offset brought back through it) and put in the bind matrix
///   (0x346440, its rows × the scale, the scale, the scale, 1).
pub fn bind(m: &mut Mem, s: u32, base: u32, scale: F) {
    m.w32(s + 0x3d80, base);
    for k in 0..17 {
        m.w32(s + 0x3cf8 + 4 * k, 0);
        m.w32(s + 0x3d3c + 4 * k, 0xffff_ffff);
    }
    let mut li = 0i32;
    while li < m.r32(s + 0x3f1c) as i32 {
        let lod = lod_of(m, s, li as u32);
        // the entries
        let mut pi = 0i32;
        while pi < m.r32(lod) as i32 {
            let part = m.r32(lod + 4).wrapping_add((pi as u32) * 0x48);
            let h = m.r32(part + 0xc);
            let (bones, kind, extra) = (count(m, h, 0x42), m.r8(h + 0x4a) as i8 as i32, count(m, h, 0x48));
            pi += 1;
            if bones == 0 && extra == 0 {
                continue;
            }
            let n = m.r32(s + 0x20) as i32;
            let mut a = 0i32;
            let mut start = 0u32;
            loop {
                if a >= n {
                    break;
                }
                let e = s + 0x24 + (a as u32) * ENTRY;
                let took = if m.r32(e + 0x388) != 0 { m.r32(e + 0x38c) } else { 0 };
                start = start.wrapping_add(took);
                if m.r32(e + 0x384) as i32 == kind {
                    break;
                }
                a += 1;
            }
            let e = s + 0x24 + (a as u32) * ENTRY;
            m.w32(part + 4, base.wrapping_add(start << 6));
            m.w32(part + 8, lod.wrapping_add(start << 6).wrapping_add(0x4c70));
            m.w32(part, e);
            if n - 1 < a {
                m.w32(s + 0x20, (n + 1) as u32);
                m.w32(e + 0x384, kind as u32);
                m.w32((s + 0x3d3c).wrapping_add((kind << 2) as u32), a as u32);
                m.w32((s + 0x3cf8).wrapping_add((kind << 2) as u32), part);
                if extra != 0 {
                    m.w32(e + 0x38c, extra as u32);
                    m.w32(e + 0x388, 0);
                } else {
                    m.w32(e + 0x38c, bones as u32);
                    m.w32(e + 0x390, start);
                    m.w32(e + 0x388, m.r32(part + 0x18));
                }
            }
        }
        // each bone's own matrix
        let mut pi = 0i32;
        while pi < m.r32(lod) as i32 {
            let part = m.r32(lod + 4).wrapping_add((pi as u32) * 0x48);
            pi += 1;
            if m.r32(part + 0x3c) != 0 {
                continue;
            }
            let mut b = 0i32;
            while b < count(m, m.r32(part + 0xc), 0x42) {
                let rec = m.r32(part + 0x18).wrapping_add((b as u32) * 0x54);
                let out = m.r32(part + 8).wrapping_add((b as u32) << 6);
                let (sa, ca) = sin_cos(rf(m, rec + 0x24));
                let (sb, cb) = sin_cos(rf(m, rec + 0x28));
                let (sc, cc) = sin_cos(rf(m, rec + 0x2c));
                let sab = sa * sb;
                let casc = ca * sc;
                let local = [
                    [cb * cc, -cb * sc, sb, F(0.0)],
                    [sab * cc + casc, ca * cc - sab * sc, -sa * cb, F(0.0)],
                    [sa * sc - (ca * sb) * cc, casc * sb + sa * cc, ca * cb, F(0.0)],
                    [rf(m, rec + 0x18), rf(m, rec + 0x1c), rf(m, rec + 0x20), F(1.0)],
                ];
                wm(m, out, &local);
                b += 1;
            }
        }
        // attached to their parents' matrices
        let mut pi = 0i32;
        while pi < m.r32(lod) as i32 {
            let part = m.r32(lod + 4).wrapping_add((pi as u32) * 0x48);
            pi += 1;
            if m.r32(part + 0x3c) != 0 {
                continue;
            }
            let mut b = 0i32;
            while b < count(m, m.r32(part + 0xc), 0x42) {
                let rec = m.r32(part + 0x18).wrapping_add((b as u32) * 0x54);
                let to = m.r16(rec + 0x10) as i16 as i32;
                if to >= 0 {
                    let k = m.r32(lod + 8 + ((to as u32) << 2)) as i32;
                    let other = if k < 0 { 0 } else { m.r32(lod + 4).wrapping_add((k as u32).wrapping_mul(0x48)) };
                    let mats = m.r32(other + 8);
                    let parent = m.r16(rec + 0x12) as i16 as i32;
                    let pm = rm(m, mats.wrapping_add((parent << 6) as u32));
                    let at = m.r32(part + 8).wrapping_add((b as u32) << 6);
                    let lm = rm(m, at);
                    let r = mul(&pm, &lm);
                    wm(m, at, &r);
                }
                b += 1;
            }
        }
        // the bind pose's inverse, in the bind matrix
        let (first, n) = if m.r32(lod + 0x4c) == 0 {
            m.w32(lod + 0x4c, 1);
            (0x4c70u32, 50u32)
        } else {
            (0x5130, 1)
        };
        for k in 0..n {
            let at = lod + first + 0x40 * k;
            for (a, b) in [(4u32, 0x10u32), (8, 0x20), (0x18, 0x24)] {
                let (x, y) = (m.r32(at + a), m.r32(at + b));
                m.w32(at + a, y);
                m.w32(at + b, x);
            }
            let t = rm(m, at);
            let v = [t[3][0] * F(-1.0), t[3][1] * F(-1.0), t[3][2] * F(-1.0), F(0.0)];
            let o = [0, 1, 2, 3].map(|i| ((t[0][i] * v[0] + t[1][i] * v[1]) + t[2][i] * v[2]) + t[3][i] * v[3]);
            let inv = [t[0], t[1], t[2], [o[0], o[1], o[2], F(1.0)]];
            let g = rm(m, BIND_MATRIX);
            let sv = [scale, scale, scale, F(1.0)];
            let gs: M4 = [0, 1, 2, 3].map(|r| g[r].map(|x| x * sv[r]));
            let r = mul(&gs, &inv);
            wm(m, at, &r);
        }
        li += 1;
    }
}

/// `RiderSkin_AllocLods` 0x207b58(s, n): `n` LODs from the first bank, each with no parts or material lists, not
/// bound (+0x4c), +0x4c60 1, the skin's two banks (each after empty waits)
pub fn alloc_lods(m: &mut Mem, s: u32, n: u32) {
    let lods = bank_alloc(m, s, n.wrapping_mul(LOD));
    m.w32(s + 0x3f18, lods);
    for i in 0..(n as i32).max(0) as u32 {
        let lod = lods.wrapping_add(i * LOD);
        for o in [0, 4, 0x4c, 0x50] {
            m.w32(lod + o, 0);
        }
        m.w32(lod + 0x4c60, 1);
        m.w32(lod + 0x58f8, s + 0x10);
        m.w32(lod + 0x58f4, s);
    }
    m.w32(s + 0x3f1c, n);
}

/// `FUN_00207e50`(s, n): LOD `n`'s parts with a +0x48 handed to the renderer's +0x2e8 (the part, its entry +
/// 0x210)
pub fn draw_lod(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, n: u32) {
    let mut pi = 0i32;
    while pi < m.r32(lod_of(m, s, n)) as i32 {
        let part = m.r32(lod_of(m, s, n) + 4).wrapping_add((pi as u32) * 0x48);
        if count(m, m.r32(part + 0xc), 0x48) != 0 {
            let at = m.r32(part).wrapping_add(0x210);
            vcall(m, env, 0x2e8, &[part, at]);
        }
        pi += 1;
    }
}

/// `RiderSkin_SkinMatricesFE` 0x208010(s, skip): unless `skip`, the front end's skinning: the first LOD's parts'
/// bones each the base matrix (+0x3d80, counting on over the parts) times the part's, into the skinned matrices
/// (0x341fc0); a table (in `scratch`) of where each kind's bones start; the renderer's +0x310 (the skinned
/// matrices, the table, the LOD's material lists and their count, 0) and +0x2e0 (the LODs, the base)
pub fn skin_matrices_fe(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, skip: u32, scratch: u32) {
    if skip != 0 {
        return;
    }
    env.call(m, MEMSET, &[scratch, 0, 0x80], &[]);
    let lod = m.r32(s + 0x3f18);
    m.w32(lod + 0x58f0, 0);
    let mut first = 0u32;
    let mut pi = 0i32;
    while pi < m.r32(lod) as i32 {
        let part = m.r32(lod + 4).wrapping_add((pi as u32) * 0x48);
        let h = m.r32(part + 0xc);
        m.w32(scratch.wrapping_add(((m.r8(h + 0x4a) as i8 as i32) << 2) as u32), first);
        let mut b = 0i32;
        while b < count(m, m.r32(part + 0xc), 0x42) {
            let k = first.wrapping_add(b as u32) << 6;
            let bm = rm(m, m.r32(s + 0x3d80).wrapping_add(k));
            let pm = rm(m, m.r32(part + 8).wrapping_add((b as u32) << 6));
            let r = mul(&bm, &pm);
            wm(m, SKINNED.wrapping_add(k), &r);
            b += 1;
        }
        first = first.wrapping_add(count(m, m.r32(part + 0xc), 0x42) as u32);
        pi += 1;
    }
    let lists = m.r32(lod + 0x50);
    vcall(m, env, 0x310, &[SKINNED, scratch, lod + 0x54, lists, 0]);
    let (lods, base) = (m.r32(s + 0x3f18), m.r32(s + 0x3d80));
    vcall(m, env, 0x2e0, &[lods, base]);
}

/// `RiderSkin_Scale` 0x208220(s; f12): every entry's scale (17 of them) and every LOD scaled
pub fn scale(m: &mut Mem, s: u32, v: F) {
    for k in 0..17u32 {
        wf(m, s + 0x24 + k * ENTRY + 0x380, v);
    }
    let mut i = 0i32;
    while i < m.r32(s + 0x3f1c) as i32 {
        let lod = lod_of(m, s, i as u32);
        lod_scale(m, lod, v);
        i += 1;
    }
}

/// `RiderSkin_ResolveTextureNames` 0x2082b8 / `RiderSkin_ResolveMaterials` 0x208328(s, list): every LOD's
pub fn resolve_names(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, list: u32) {
    let mut i = 0i32;
    while i < m.r32(s + 0x3f1c) as i32 {
        let lod = lod_of(m, s, i as u32);
        lod_resolve_names(m, env, lod, list);
        i += 1;
    }
}
pub fn resolve_materials(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32, list: u32) {
    let mut i = 0i32;
    while i < m.r32(s + 0x3f1c) as i32 {
        let lod = lod_of(m, s, i as u32);
        lod_resolve_materials(m, env, lod, list);
        i += 1;
    }
}

/// `RiderSkin_SetRootMatrix` 0x2083d8(s, matrix): the kind-1 part's bones from the matrix (`SetChildMatrices`
/// from no parent); `Boarder_SetSkinChildMatrices` 0x2083e8: the kind-0 part's
pub fn set_root_matrix(m: &mut Mem, s: u32, matrix: u32) {
    let p = m.r32(s + 0x3cfc);
    set_child_matrices(m, p, 0xffff_ffff, matrix);
}
pub fn set_skin_child_matrices(m: &mut Mem, s: u32, matrix: u32) {
    let p = m.r32(s + 0x3cf8);
    set_child_matrices(m, p, 0xffff_ffff, matrix);
}

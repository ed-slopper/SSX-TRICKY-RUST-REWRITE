//! The rider skin's parts and LODs (board row G7r2): `SkinPart_*` 0x205018..0x2059f4 (and the bone lookup
//! 0x2054c8) and `SkinLod_*` 0x206ac0..0x206f7c. On the bytes at their addresses ([`Mem`]); the copy, `strncmp`
//! and the sine pair are as the game's (`animcurve::sin_cos`) or calls out ([`TextBoxEnv`]). Checked in the
//! function runner (`tests/skinpart.rs`).
//!
//! **A part** (0x48 bytes): +0 its bones' angles (12 bytes each; their offsets 0x108 on), +4 its bones' matrices
//! (0x40 each), +0xc its header (a copy, 0x50 bytes: +0x14 the data's size, +0x18.. the offsets below, +0x3c /
//! +0x3e / +0x40 / +0x42 / +0x44 / +0x46 counts, +0x4a its kind), +0x10 its data, +0x14 its materials (32 bytes
//! each), +0x18 its bones (0x54 each: +0x12 the parent), +0x1c its points (16 bytes), +0x20 its bone ids (8 bytes),
//! +0x2c its matrix lists (12 bytes), +0x30 (8 bytes), +0x34 its batches (0x14 bytes), +0x38 its scale, +0x3c
//! scaled, +0x40 / +0x44 its banks.
//!
//! **A LOD**: +0 how many parts, +4 the parts, +8 the part of each kind (17, −1 for none), +0x50 how many
//! material lists, +0x54 the lists (12 bytes: the count, the list, its number), +0x58f4 / +0x58f8 the banks.

use crate::animcurve::sin_cos;
use crate::bdrload::{bank_alloc, find_part_name, find_texture, DELETE, PARTS, STRNCMP};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const MEMCPY: u32 = 0x2fb2b4;
const PART: u32 = 0x48;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn header(m: &Mem, p: u32) -> u32 {
    m.r32(p + 0xc)
}
fn count(m: &Mem, h: u32, o: u32) -> i32 {
    m.r16(h + o) as i16 as i32
}

/// `SkinPart_Delete` 0x205018(p, flags): reset; deleted with flag 1
pub fn part_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32, flags: u32) {
    part_reset(m, p);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[p], &[]);
    }
}
/// `SkinPart_Reset` 0x205068(p): no header, scale 1, not scaled
pub fn part_reset(m: &mut Mem, p: u32) {
    m.w32(p + 0xc, 0);
    wf(m, p + 0x38, F(1.0));
    m.w32(p + 0x3c, 0);
}

/// `SkinPart_Load` 0x205080(p, header, data): the header copied into the part's bank (its +0x44 for a kind of 1,
/// else +0x40), the data (its +0x14 bytes) after it (`memcpy`), and the data's offsets made pointers: the
/// materials, bones, points, bone ids, the matrix lists (their lists), the +0x30 entries (their +4), the batches
/// (their sub-batch lists; each sub-batch's layer records, their packets and the second word counted on from them)
pub fn part_load(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32, hdr: u32, data: u32) {
    let bank = if m.r8(hdr + 0x4a) as i8 == 1 { m.r32(p + 0x44) } else { m.r32(p + 0x40) };
    let h = bank_alloc(m, bank, 0x50);
    m.w32(p + 0xc, h);
    for k in 0..0x14 {
        let v = m.r32(hdr + 4 * k);
        m.w32(h + 4 * k, v);
    }
    let size = m.r32(hdr + 0x14);
    let d = bank_alloc(m, bank, size);
    m.w32(p + 0x10, d);
    env.call(m, MEMCPY, &[d, data, m.r32(hdr + 0x14)], &[]);
    let d = m.r32(p + 0x10);
    m.w32(p + 0x18, d.wrapping_add(m.r32(hdr + 0x18)));
    m.w32(p + 0x14, d);
    let list = d.wrapping_add(m.r32(hdr + 0x2c));
    m.w32(p + 0x30, list);
    for i in 0..count(m, hdr, 0x3e).max(0) as u32 {
        let e = list.wrapping_add(i << 3);
        m.w32(e + 4, d.wrapping_add(m.r32(e + 4)));
    }
    let mats = d.wrapping_add(m.r32(hdr + 0x30));
    m.w32(p + 0x2c, mats);
    for i in 0..count(m, hdr, 0x3c).max(0) as u32 {
        let e = mats.wrapping_add(i * 12);
        m.w32(e + 4, d.wrapping_add(m.r32(e + 4)));
    }
    m.w32(p + 0x1c, d.wrapping_add(m.r32(hdr + 0x1c)));
    m.w32(p + 0x20, d.wrapping_add(m.r32(hdr + 0x28)));
    let batches = d.wrapping_add(m.r32(hdr + 0x20));
    m.w32(p + 0x34, batches);
    for b in 0..count(m, hdr, 0x40).max(0) as u32 {
        let e = batches.wrapping_add(b * 0x14);
        m.w32(e + 0x10, d.wrapping_add(m.r32(e + 0x10)));
        let mut s = 0i32;
        while s < m.r32(e + 0xc) as i32 {
            let sub = m.r32(e + 0x10).wrapping_add((s as u32) << 3);
            m.w32(sub, d.wrapping_add(m.r32(sub)));
            let mut k = 0i32;
            while k < m.r32(sub + 4) as i32 {
                let q = m.r32(sub).wrapping_add((k as u32).wrapping_mul(12));
                let at = d.wrapping_add(m.r32(q));
                m.w32(q, at);
                m.w32(q + 4, m.r32(q + 4).wrapping_add(at));
                k += 1;
            }
            s += 1;
        }
    }
}

/// 0x2054c8(p, i, &id): bone `i`'s id written, its data (the part's data + its offset) the answer
pub fn bone(m: &mut Mem, p: u32, i: u32, id: u32) -> u32 {
    let e = m.r32(p + 0x20).wrapping_add(i << 3);
    m.w32(id, m.r32(e));
    m.r32(p + 0x10).wrapping_add(m.r32(e + 4))
}

/// `SkinPart_Scale` 0x2054f0(p; f12 s): once (+0x3c), the scale kept and every bone's offset (+0x18) and point
/// × `s`
pub fn part_scale(m: &mut Mem, p: u32, s: F) {
    if m.r32(p + 0x3c) != 0 {
        return;
    }
    wf(m, p + 0x38, s);
    m.w32(p + 0x3c, 1);
    let mut i = 0i32;
    while i < count(m, header(m, p), 0x42) {
        let v = m.r32(p + 0x18).wrapping_add((i as u32) * 0x54) + 0x18;
        for k in 0..3 {
            let x = rf(m, v + 4 * k) * s;
            wf(m, v + 4 * k, x);
        }
        i += 1;
    }
    let mut i = 0i32;
    while i < count(m, header(m, p), 0x46) {
        let v = m.r32(p + 0x1c).wrapping_add((i as u32) << 4);
        for k in 0..3 {
            let x = rf(m, v + 4 * k) * s;
            wf(m, v + 4 * k, x);
        }
        i += 1;
    }
}

/// `SkinPart_ResolveTextureNames` 0x2055c0(p, list): each material's five names (+0, +4, .. +0x10) put in the
/// shape list (`BdrLoad_FindPartName`), name by name over all the materials
pub fn part_resolve_names(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32, list: u32) {
    for k in 0..5u32 {
        let mut i = 0i32;
        while i < count(m, header(m, p), 0x44) {
            let at = m.r32(p + 0x14).wrapping_add((i as u32) << 5).wrapping_add(k << 2);
            find_part_name(m, env, list, at);
            i += 1;
        }
    }
}

/// `SkinPart_ResolveMaterials` 0x205670(p, list): unless scaled, each batch (+0x34, 0x14 bytes): a `boot`
/// material's flagged (0x8000); its strength (+8) the material's +0x14 (negated with +0x10's flag, 0 with neither)
/// × 127 as an integer; its material number (+4) its texture (`SkinTexList_Find` of the material's name)
pub fn part_resolve_materials(m: &mut Mem, env: &mut dyn TextBoxEnv, p: u32, list: u32) {
    if m.r32(p + 0x3c) != 0 {
        return;
    }
    let batches = m.r32(p + 0x34);
    let mut i = 0i32;
    while i < count(m, header(m, p), 0x40) {
        let b = batches.wrapping_add((i as u32) * 0x14);
        let mat = m.r32(p + 0x14).wrapping_add(m.r32(b + 4) << 5);
        if env.call(m, STRNCMP, &[mat, PARTS[2], 4], &[]) == 0 {
            m.w32(b, m.r32(b) | 0x8000);
        }
        let mat = m.r32(p + 0x14).wrapping_add(m.r32(b + 4) << 5);
        let strength = if m.r8(mat + 0xc) as i8 != 0 {
            rf(m, mat + 0x14)
        } else if m.r8(mat + 0x10) as i8 != 0 {
            -rf(m, mat + 0x14)
        } else {
            F(0.0)
        };
        m.w32(b + 8, crate::sndlib::cvt_w((strength * F(127.0)).0) as u32);
        let t = find_texture(m, env, list, mat);
        m.w32(b + 4, t);
        i += 1;
    }
}

/// `SkinPart_SetChildMatrices` 0x2057a0(p, parent, matrix): each bone whose parent is `parent` given the parent's
/// matrix times its own (the rotation of its three angles, x then y then z, with its offset), then its children
/// the same
pub fn set_child_matrices(m: &mut Mem, p: u32, parent: u32, matrix: u32) {
    let mut i = 0i32;
    while i < count(m, header(m, p), 0x42) {
        let rec = m.r32(p + 0x18).wrapping_add((i as u32) * 0x54);
        if m.r16(rec + 0x12) as i16 as i32 as u32 != parent {
            i += 1;
            continue;
        }
        let out = m.r32(p + 4).wrapping_add((i as u32) << 6);
        let par: [[F; 4]; 4] = [0, 1, 2, 3].map(|r| [0, 1, 2, 3].map(|c| rf(m, matrix + 0x10 * r + 4 * c)));
        for r in 0..4u32 {
            for c in 0..4u32 {
                wf(m, out + 0x10 * r + 4 * c, par[r as usize][c as usize]);
            }
        }
        let a = m.r32(p).wrapping_add((i as u32) * 12);
        let (sa, ca) = sin_cos(rf(m, a));
        let (sb, cb) = sin_cos(rf(m, a + 4));
        let (sc, cc) = sin_cos(rf(m, a + 8));
        let sab = sa * sb;
        let rot = [
            [cb * cc, -cb * sc, sb, F(0.0)],
            [sab * cc + ca * sc, ca * cc - sab * sc, -sa * cb, F(0.0)],
            [sa * sc - (ca * sb) * cc, (ca * sc) * sb + sa * cc, ca * cb, F(0.0)],
            [rf(m, a + 0x108), rf(m, a + 0x10c), rf(m, a + 0x110), F(1.0)],
        ];
        for (r, row) in rot.iter().enumerate() {
            for c in 0..4 {
                let v = ((par[0][c] * row[0] + par[1][c] * row[1]) + par[2][c] * row[2]) + par[3][c] * row[3];
                wf(m, out + 0x10 * r as u32 + 4 * c as u32, v);
            }
        }
        let child = m.r32(p + 4).wrapping_add((i as u32) << 6);
        set_child_matrices(m, p, i as u32, child);
        i += 1;
    }
}

/// `SkinLod_AddPart` 0x206ac0(lod, header, data): the part of the header's kind (or, with none yet, the first
/// part with no header, or the one past the last) loaded with it, kept as that kind's; the materials collected
/// again
pub fn lod_add_part(m: &mut Mem, env: &mut dyn TextBoxEnv, lod: u32, hdr: u32, data: u32) {
    let slot = lod.wrapping_add(8).wrapping_add(((m.r8(hdr + 0x4a) as i8 as i32) << 2) as u32);
    let mut k = m.r32(slot) as i32;
    if k < 0 {
        let n = m.r32(lod) as i32;
        k = 0;
        while k < n && m.r32(m.r32(lod + 4).wrapping_add((k as u32) * PART) + 0xc) != 0 {
            k += 1;
        }
    }
    m.w32(slot, k as u32);
    let part = m.r32(lod + 4).wrapping_add((k as u32).wrapping_mul(PART));
    part_load(m, env, part, hdr, data);
    collect_materials(m, lod);
}

/// `SkinLod_MaterialEquals` 0x206b60(a, b): the same two halfwords
pub fn material_equals(m: &Mem, a: u32, b: u32) -> bool {
    m.r16(a + 2) == m.r16(b + 2) && m.r16(a) == m.r16(b)
}
/// `SkinLod_MaterialListEquals` 0x206b88(a, b): the same count and each material the same
pub fn material_list_equals(m: &Mem, a: u32, b: u32) -> bool {
    if m.r32(a) != m.r32(b) {
        return false;
    }
    let mut i = 0i32;
    while i < m.r32(a) as i32 {
        if !material_equals(m, m.r32(a + 4).wrapping_add((i as u32) << 2), m.r32(b + 4).wrapping_add((i as u32) << 2)) {
            return false;
        }
        i += 1;
    }
    true
}

/// `SkinLod_CollectMaterials` 0x206c10(lod): over the parts up to the first with no header, each matrix list's
/// materials matched to the LOD's lists (a new one kept when none matches), the list's number put in its +8
pub fn collect_materials(m: &mut Mem, lod: u32) {
    let mut p = 0i32;
    while p < m.r32(lod) as i32 {
        let part = m.r32(lod + 4).wrapping_add((p as u32) * PART);
        let h = header(m, part);
        if h == 0 {
            return;
        }
        let mut i = 0i32;
        while i < count(m, h, 0x3c) {
            let e = m.r32(part + 0x2c).wrapping_add((i as u32) * 12);
            let mut s = 0i32;
            while s < m.r32(lod + 0x50) as i32 && !material_list_equals(m, lod + 0x54 + (s as u32) * 12, e) {
                s += 1;
            }
            if s == m.r32(lod + 0x50) as i32 {
                let at = lod + 0x54 + (s as u32) * 12;
                m.w32(at, m.r32(e));
                m.w32(at + 8, s as u32);
                m.w32(at + 4, m.r32(e + 4));
                m.w32(lod + 0x50, m.r32(lod + 0x50).wrapping_add(1));
            }
            m.w32(e + 8, s as u32);
            i += 1;
        }
        p += 1;
    }
}

/// `SkinLod_AllocParts` 0x206d68(lod, n): `n` parts from the LOD's bank, each emptied and given the banks; no part
/// of any kind
pub fn alloc_parts(m: &mut Mem, lod: u32, n: u32) {
    let parts = bank_alloc(m, m.r32(lod + 0x58f4), n.wrapping_mul(PART));
    m.w32(lod + 4, parts);
    for i in 0..(n as i32).max(0) as u32 {
        let p = parts.wrapping_add(i * PART);
        m.w32(p + 0x40, m.r32(lod + 0x58f4));
        m.w32(p + 0x44, m.r32(lod + 0x58f8));
        for o in [0, 4, 0xc, 0x3c] {
            m.w32(p + o, 0);
        }
    }
    m.w32(lod, n);
    for k in 0..17 {
        m.w32(lod + 8 + 4 * k, 0xffff_ffff);
    }
}

/// `SkinLod_Scale` 0x206e30, `SkinLod_ResolveTextureNames` 0x206ea0, `SkinLod_ResolveMaterials` 0x206f10: every
/// part's
pub fn lod_scale(m: &mut Mem, lod: u32, s: F) {
    let mut i = 0i32;
    while i < m.r32(lod) as i32 {
        let p = m.r32(lod + 4).wrapping_add((i as u32) * PART);
        part_scale(m, p, s);
        i += 1;
    }
}
pub fn lod_resolve_names(m: &mut Mem, env: &mut dyn TextBoxEnv, lod: u32, list: u32) {
    let mut i = 0i32;
    while i < m.r32(lod) as i32 {
        let p = m.r32(lod + 4).wrapping_add((i as u32) * PART);
        part_resolve_names(m, env, p, list);
        i += 1;
    }
}
pub fn lod_resolve_materials(m: &mut Mem, env: &mut dyn TextBoxEnv, lod: u32, list: u32) {
    let mut i = 0i32;
    while i < m.r32(lod) as i32 {
        let p = m.r32(lod + 4).wrapping_add((i as u32) * PART);
        part_resolve_materials(m, env, p, list);
        i += 1;
    }
}

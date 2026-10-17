//! The rider skin's banks and shape lists (board row G7r1): `SkinBank_*`, `MdfMemBank_Init`, `MemBank_AlignSize`
//! 0x203e88..0x203fcc, the outfit's shape list `BdrLoad_*` 0x203fd0..0x204594, `SkinTexList_Find` 0x204598 and
//! `BdrLoad_BindTextures` 0x2021f8. On the bytes at their addresses ([`Mem`]); the heaps, files, shapes, the
//! renderer's and texture managers' virtuals and the skin build are calls out ([`TextBoxEnv`]). Checked in the
//! function runner (`tests/bdrload.rs`).
//!
//! **A bank** (`MdfMemBank`): +0 its memory, +4 the next free place (on a 128-byte boundary), +8 how much is
//! taken, +0xc its size.
//!
//! **A shape list** (an outfit's textures by part): +0 how many names; +4 the names, 5 bytes each (4 characters
//! and a 0); +0x130 each name's texture (60 words, −1 for none).

use crate::pf::Mem;
use crate::textbox::TextBoxEnv;

pub const ALLOC: u32 = 0x2ccf70;
pub const MEM_SET: u32 = 0x2cc808;
pub const MEM_COPY: u32 = 0x2ce114;
pub const FREE: u32 = 0x14a040;
pub const DELETE: u32 = 0x114f80;
pub const STRNCMP: u32 = 0x2fd478;
pub const FILE_LOAD: u32 = 0x2c8a70;
pub const EAMEM_FREE: u32 = 0x2ccfc0;
pub const SHAPE_FIND: u32 = 0x2c2528;
pub const RESET_SKIN: u32 = 0x116a18;
pub const CLEAR_PARTS: u32 = 0x2070e8;
pub const BUILD_SKIN: u32 = 0x201138;
/// The names: the bank's heap name (`MdfMemBank`), the five parts (`head`, `helm`, `boot`, `suit`, `bord`), the
/// textures' buffer (`dummy`) and the textures' name (`plr`)
pub const BANK_NAME: u32 = 0x39e4f0;
pub const PARTS: [u32; 5] = [0x39e500, 0x39e508, 0x39e510, 0x39e518, 0x39e520];
pub const DUMMY_NAME: u32 = 0x39e528;
pub const TEXTURE_NAME: u32 = 0x39e530;
const APP: u32 = 0x337c58;

/// A virtual on the object at `obj` (its vtable at +0x10: the this-shift at `off`, the function at `off` + 4)
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj + 0x10);
    let this = obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let func = m.r32(vt + off + 4);
    let mut w = vec![this];
    w.extend_from_slice(args);
    env.call(m, func, &w, &[])
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}

/// `SkinBank_Init` 0x203e88(b): no memory; `b`
pub fn bank_init(m: &mut Mem, b: u32) -> u32 {
    m.w32(b, 0);
    b
}

/// `MdfMemBank_Init` 0x203e98(b, size): `size` + 0x80 taken (`MdfMemBank`, filled with 0xdeadc0ed), then aligned
pub fn mem_bank_init(m: &mut Mem, env: &mut dyn TextBoxEnv, b: u32, size: u32) {
    let total = size.wrapping_add(0x80);
    m.w32(b + 0xc, total);
    let p = env.call(m, ALLOC, &[BANK_NAME, total, 0], &[]);
    if p != 0 {
        env.call(m, MEM_SET, &[p, 0xdead_c0ed, total], &[]);
    }
    m.w32(b, p);
    align(m, b);
}

/// `SkinBank_Delete` 0x203f08(b, flags): its memory freed; deleted with flag 1
pub fn bank_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, b: u32, flags: u32) {
    let p = m.r32(b);
    if p != 0 {
        env.call(m, FREE, &[p], &[]);
    }
    if flags & 1 != 0 {
        env.call(m, DELETE, &[b], &[]);
    }
}

/// `SkinBank_Alloc` 0x203f60(b, n): the next free place, moved on (and the taken grown) by `n` rounded down to
/// 128 bytes plus 128 (toward zero for a negative `n`)
pub fn bank_alloc(m: &mut Mem, b: u32, n: u32) -> u32 {
    let s = n as i32;
    let t = if s > -1 { s } else { s.wrapping_add(0x7f) };
    let step = ((t >> 7) << 7).wrapping_add(0x80) as u32;
    let at = m.r32(b + 4);
    m.w32(b + 4, at.wrapping_add(step));
    m.w32(b + 8, m.r32(b + 8).wrapping_add(step));
    at
}

/// `MemBank_AlignSize` 0x203fa0(b): the next free place the memory rounded up to 128 bytes, the taken what that
/// skips
pub fn align(m: &mut Mem, b: u32) {
    let p = m.r32(b) as i32;
    let t = if p.wrapping_add(0x7f) > -1 { p.wrapping_add(0x7f) } else { p.wrapping_add(0xfe) };
    let at = ((t >> 7) << 7) as u32;
    m.w32(b + 4, at);
    m.w32(b + 8, at.wrapping_sub(p as u32));
}

/// `BdrLoad_ConstructShapeList` 0x203fd0(l): cleared; `l`
pub fn list_construct(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32) -> u32 {
    list_clear(m, env, l);
    l
}
/// `BdrLoad_DestructShapeList` 0x203ff8(l, flags): cleared; deleted with flag 1
pub fn list_destruct(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, flags: u32) {
    list_clear(m, env, l);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[l], &[]);
    }
}
/// `BdrLoad_ClearShapeList` 0x204048(l): no names, every texture −1
pub fn list_clear(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32) {
    m.w32(l, 0);
    env.call(m, MEM_SET, &[l + 0x130, 0xffff_ffff, 0xf0], &[]);
}

/// `BdrLoad_LoadOutfitParts` 0x204068(l): the five parts named (`head`, `helm`, `boot`, `suit`, `bord`) and each
/// given a texture to draw into (the renderer's +0x158: a 64 KB `dummy` buffer, `plr`, 128 × 128 (the suit's
/// 256 × 256), 3, 4, 1; the buffer and 0 on the stack), the buffer freed after
pub fn load_outfit_parts(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32) {
    for name in PARTS {
        find_part_name(m, env, l, name);
    }
    let buf = env.call(m, ALLOC, &[DUMMY_NAME, 0x10000, 0x10], &[]);
    if buf != 0 {
        env.call(m, MEM_SET, &[buf, 0xdead_c0ed, 0x10000], &[]);
    }
    for (k, size) in [0x80u32, 0x80, 0x80, 0x100, 0x80].into_iter().enumerate() {
        let r = renderer(m);
        let t = vcall(m, env, r, 0x158, &[buf, TEXTURE_NAME, size, size, 3, 4, 1, buf, 0]);
        m.w32(l + 0x130 + 4 * k as u32, t);
    }
    if buf != 0 {
        env.call(m, FREE, &[buf], &[]);
    }
}

/// `BdrLoad_RegisterShapeFile` 0x2042a0(l, file): the file loaded (`File_Load`, 0x10), its shapes registered, it
/// freed
pub fn register_shape_file(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, file: u32) {
    let data = env.call(m, FILE_LOAD, &[file, 0x10], &[]);
    register_shapes(m, env, l, data);
    env.call(m, EAMEM_FREE, &[data], &[]);
}

/// `BdrLoad_RegisterShapes` 0x2042e8(l, shapes): each name's shape in the file (`Shape_FindByName`), when there is
/// one, made a texture (the renderer's +0x150: the shape, `plr`, 0)
pub fn register_shapes(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, shapes: u32) {
    let mut i = 0u32;
    while (i as i32) < m.r32(l) as i32 {
        let s = env.call(m, SHAPE_FIND, &[shapes, l + 4 + 5 * i], &[]);
        if s != 0 {
            let r = renderer(m);
            let t = vcall(m, env, r, 0x150, &[s, TEXTURE_NAME, 0]);
            m.w32(l + 0x130 + 4 * i, t);
        }
        i += 1;
    }
}

/// `BdrLoad_RebindShapeFile` 0x2043b0(l, file): as `register_shape_file`, rebinding
pub fn rebind_shape_file(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, file: u32) {
    let data = env.call(m, FILE_LOAD, &[file, 0x10], &[]);
    rebind_shapes(m, env, l, data);
    env.call(m, EAMEM_FREE, &[data], &[]);
}

/// `BdrLoad_RebindShapes` 0x2043f8(l, shapes): each name's texture (if any) given the file's shape of that name
/// (if any; the renderer's +0x160: the texture, the shape). The count is read once.
pub fn rebind_shapes(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, shapes: u32) {
    let n = m.r32(l) as i32;
    let mut i = 0i32;
    while i < n {
        let t = shape_slot(m, l, i as u32);
        let s = env.call(m, SHAPE_FIND, &[shapes, l + 4 + 5 * i as u32], &[]);
        if s != 0 && (t as i32) >= 0 {
            let r = renderer(m);
            vcall(m, env, r, 0x160, &[t, s]);
        }
        i += 1;
    }
}

/// `BdrLoad_FindPartName` 0x2044c0(l, name): the name's place (its first 4 characters, `strncmp`); a new one
/// added at the end (4 copied and a 0); −1 for an empty name
pub fn find_part_name(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, name: u32) -> u32 {
    if m.r8(name) == 0 {
        return 0xffff_ffff;
    }
    let mut i = 0i32;
    while i < m.r32(l) as i32 {
        if env.call(m, STRNCMP, &[l + 4 + 5 * i as u32, name, 4], &[]) == 0 {
            return i as u32;
        }
        i += 1;
    }
    let n = m.r32(l);
    env.call(m, MEM_COPY, &[l.wrapping_add(n.wrapping_mul(5)).wrapping_add(4), name, 4], &[]);
    let n = m.r32(l);
    m.w8(l.wrapping_add(n.wrapping_mul(5)).wrapping_add(8), 0);
    let n = m.r32(l);
    m.w32(l, n.wrapping_add(1));
    n
}

/// `BdrLoad_GetShapeSlot` 0x204588(l, i): name `i`'s texture
pub fn shape_slot(m: &Mem, l: u32, i: u32) -> u32 {
    m.r32(l.wrapping_add(i << 2) + 0x130)
}

/// `SkinTexList_Find` 0x204598(l, name): the texture of the name (its first 4 characters); −1 for none or an
/// empty name
pub fn find_texture(m: &mut Mem, env: &mut dyn TextBoxEnv, l: u32, name: u32) -> u32 {
    if m.r8(name) == 0 {
        return 0xffff_ffff;
    }
    let mut i = 0i32;
    while i < m.r32(l) as i32 {
        if env.call(m, STRNCMP, &[l + 4 + 5 * i as u32, name, 4], &[]) == 0 {
            return shape_slot(m, l, i as u32);
        }
        i += 1;
    }
    0xffff_ffff
}

/// `BdrLoad_BindTextures` 0x2021f8(load, n): rider `n`'s textures (its record, 0x50 bytes from +0x14) bound:
/// - its three ids (+0x10..+0x18) through the loader's texture manager +8 (the third through +0xc when there
///   is one), the manager's +0x68 (the id, 0x10), into +0x1c..+0x24 (0 for an id below 0);
/// - its five more (+0x28..+0x38) through +0x10 into +0x3c..+0x4c;
/// - with the rider's skin (its +4's +0x79c) wanting it (+0x46b0): the skin reset (`Boarder_ResetSkin`, flag
///   bit 13 off) for flag bit 8, else its parts cleared (`RiderSkin_ClearParts`) for bit 9;
///
/// then built (`BdrLoad_BuildSkin` 0x201138: the loader, the record).
pub fn bind_textures(m: &mut Mem, env: &mut dyn TextBoxEnv, load: u32, n: u32) {
    let rec = load.wrapping_add(n.wrapping_mul(0x50)).wrapping_add(0x14);
    let rider = m.r32(rec + 4);
    let skin = rider.wrapping_add(0x79c);
    for k in 0..3u32 {
        let id = m.r32(rec + 0x10 + 4 * k);
        if (id as i32) < 0 {
            m.w32(rec + 0x1c + 4 * k, 0);
            continue;
        }
        let third = m.r32(load + 0xc);
        if third != 0 && k == 2 {
            let t = vcall(m, env, third, 0x68, &[m.r32(rec + 0x18), 0x10]);
            m.w32(rec + 0x24, t);
        } else {
            let tm = m.r32(load + 8);
            let t = vcall(m, env, tm, 0x68, &[m.r32(rec + 0x10 + 4 * k), 0x10]);
            m.w32(rec + 0x1c + 4 * k, t);
        }
    }
    for k in 0..5u32 {
        let id = m.r32(rec + 0x28 + 4 * k);
        let t = if (id as i32) < 0 {
            0
        } else {
            let tm = m.r32(load + 0x10);
            vcall(m, env, tm, 0x68, &[id, 0x10])
        };
        m.w32(rec + 0x3c + 4 * k, t);
    }
    if m.r32(skin + 0x3f14) != 0 {
        let flags = m.r32(rec + 0xc);
        if (flags >> 8) & 1 != 0 {
            env.call(m, RESET_SKIN, &[rider, ((flags as i32 >> 13) as u32 ^ 1) & 1], &[]);
        } else if (flags >> 9) & 1 != 0 {
            env.call(m, CLEAR_PARTS, &[skin], &[]);
        }
    }
    env.call(m, BUILD_SKIN, &[load, rec], &[]);
}

//! The camera files' lookups as the game does them (board row E11i), on the game's bytes (`pf::Mem`), checked in
//! the function runner (`tests/cml.rs`).
//!
//! A loaded camera file is reached through a handle: a word holding the file's base (0 when the file is missing).
//! Every offset in the file is from that base. The file's +4 + 4 × kind is the first record of each kind (-1 none;
//! kind 6, +0x1c, is the root of the node and scope trees). A record is a 0x20-byte header (+0 the next of the
//! kind, +8 the body, +0xc the name's hash, +0x10 the name) before its body; `FirstOfType` / `NextOfType` give
//! bodies. Tree nodes have +0 / +4 the children (-1 none); a node's hash is at +0x10, a scope's at +0xc with its
//! name at +0x24 and its objects' list at +8 (each +4 the next, +8 the body, +0xc the hash, +0x10 the name).
//!
//! The game keeps three handles: the course's file (0x336688), `commonob` (0x336690) and `scripts` (0x336698).

use crate::pf::Mem;

pub const FILES: [u32; 3] = [0x336688, 0x336690, 0x336698];
const NONE: u32 = u32::MAX;

/// `Cml_HashName` 0x16df80 (the name, at most `n` characters; 0 for all): each signed character in, the hash
/// shifted left 23 and xored with itself shifted right 7
pub fn hash_name(m: &Mem, name: u32, n: u32) -> u32 {
    let mut left = if n != 0 { n } else { u32::MAX };
    let mut h = 0u32;
    let mut p = name;
    let mut c = m.r8(p) as i8 as i32 as u32;
    if c == 0 {
        return 0;
    }
    while left != 0 {
        let v = ((h as i32) >> 7) as u32;
        p += 1;
        h = (h << 23) ^ v ^ c;
        c = m.r8(p) as i8 as i32 as u32;
        if c == 0 {
            return h;
        }
        left = left.wrapping_sub(1);
    }
    h
}

/// `Cml_ResolveOffset` 0x16dfd0: the handle's base plus the offset
pub fn resolve(m: &Mem, handle: u32, off: u32) -> u32 {
    m.r32(handle).wrapping_add(off)
}

/// `strcmp` == 0 on memory
fn str_eq(m: &Mem, a: u32, b: u32) -> bool {
    let mut k = 0;
    loop {
        let (x, y) = (m.r8(a + k), m.r8(b + k));
        if x != y {
            return false;
        }
        if x == 0 {
            return true;
        }
        k += 1;
    }
}

/// `CmlFile_FindNodeRec` 0x16dfe0: depth first, the left subtree, the right, then the node itself by its hash (+0x10)
pub fn find_node_rec(m: &Mem, handle: u32, off: u32, name: u32, hash: u32) -> u32 {
    let n = resolve(m, handle, off);
    let l = m.r32(n);
    if l != NONE {
        let r = find_node_rec(m, handle, l, name, hash);
        if r != NONE {
            return r;
        }
    }
    let rt = m.r32(n + 4);
    if rt != NONE {
        let r = find_node_rec(m, handle, rt, name, hash);
        if r != NONE {
            return r;
        }
    }
    if m.r32(n + 0x10) == hash { off } else { NONE }
}

/// `CmlFile_FindScopeRec` 0x16e098: the same for a scope, by its hash (+0xc) and name (+0x24)
pub fn find_scope_rec(m: &Mem, handle: u32, off: u32, name: u32, hash: u32) -> u32 {
    let n = resolve(m, handle, off);
    let l = m.r32(n);
    if l != NONE {
        let r = find_scope_rec(m, handle, l, name, hash);
        if r != NONE {
            return r;
        }
    }
    let rt = m.r32(n + 4);
    if rt != NONE {
        let r = find_scope_rec(m, handle, rt, name, hash);
        if r != NONE {
            return r;
        }
    }
    if m.r32(n + 0xc) == hash && str_eq(m, n + 0x24, name) { off } else { NONE }
}

/// `CmlFile_FindNode` 0x16e168: a node's offset by name from the root (+0x1c), or -1
pub fn find_node(m: &Mem, handle: u32, name: u32) -> u32 {
    let root = m.r32(m.r32(handle) + 0x1c);
    if root == NONE {
        return NONE;
    }
    find_node_rec(m, handle, root, name, hash_name(m, name, 0))
}

/// `CmlFile_FindScope` 0x16e1d8
pub fn find_scope(m: &Mem, handle: u32, name: u32) -> u32 {
    let root = m.r32(m.r32(handle) + 0x1c);
    if root == NONE {
        return NONE;
    }
    find_scope_rec(m, handle, root, name, hash_name(m, name, 0))
}

/// `CmlFile_FindObject` 0x16e248 (the name, the kind): the body of the first record of the kind with the name, or 0
pub fn file_find_object(m: &Mem, handle: u32, name: u32, kind: u32) -> u32 {
    if m.r32(handle) == 0 {
        return 0;
    }
    let mut off = m.r32(m.r32(handle) + kind.wrapping_mul(4) + 4);
    if off == NONE {
        return 0;
    }
    let h = hash_name(m, name, 0);
    loop {
        let r = resolve(m, handle, off);
        if m.r32(r + 0xc) == h && str_eq(m, r + 0x10, name) {
            return resolve(m, handle, m.r32(r + 8));
        }
        off = m.r32(r);
        if off == NONE {
            return 0;
        }
    }
}

/// `CmlFile_FindObjectInScope` 0x16e318 (the name, the scope's name): the body of the scope's object with the name, or 0
pub fn file_find_object_in_scope(m: &Mem, handle: u32, name: u32, scope: u32) -> u32 {
    if m.r32(handle) == 0 {
        return 0;
    }
    let s = find_scope(m, handle, scope);
    if s == NONE {
        return 0;
    }
    let mut it = m.r32(resolve(m, handle, s) + 8);
    if it == NONE {
        return 0;
    }
    let h = hash_name(m, name, 0);
    loop {
        let e = resolve(m, handle, it);
        if m.r32(e + 0xc) == h && str_eq(m, e + 0x10, name) {
            return resolve(m, handle, m.r32(e + 8));
        }
        it = m.r32(e + 4);
        if it == NONE {
            return 0;
        }
    }
}

/// `CmlFile_FirstOfType` 0x16e408: the first body of the kind, or 0
pub fn first_of_type(m: &Mem, handle: u32, kind: u32) -> u32 {
    let base = m.r32(handle);
    if base == 0 {
        return 0;
    }
    let off = m.r32(base + kind.wrapping_mul(4) + 4);
    if off == NONE {
        return 0;
    }
    let r = resolve(m, handle, off);
    resolve(m, handle, m.r32(r + 8))
}

/// `CmlFile_NextOfType` 0x16e468 (a body): the next body of its kind (its header's +0), or 0
pub fn next_of_type(m: &Mem, handle: u32, body: u32) -> u32 {
    if m.r32(handle) == 0 {
        return 0;
    }
    let off = m.r32(body.wrapping_sub(0x20));
    if off == NONE {
        return 0;
    }
    let r = resolve(m, handle, off);
    resolve(m, handle, m.r32(r + 8))
}

/// `Cml_FindObject` 0x16e518 (the name, the kind): the course's file, then `commonob`, then `scripts`
pub fn find_object(m: &Mem, name: u32, kind: u32) -> u32 {
    for f in FILES {
        let r = file_find_object(m, f, name, kind);
        if r != 0 {
            return r;
        }
    }
    0
}

/// `Cml_FindObjectInScope` 0x16e588 (the name, the scope, the kind): in each file's scope, else anywhere of the kind
pub fn find_object_in_scope(m: &Mem, name: u32, scope: u32, kind: u32) -> u32 {
    for f in FILES {
        let r = file_find_object_in_scope(m, f, name, scope);
        if r != 0 {
            return r;
        }
    }
    find_object(m, name, kind)
}

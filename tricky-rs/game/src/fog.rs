//! Fog (board row G7c2): the fog object manager `cFogObjMan` 0x1c9b20..0x1ca2c0, which makes a `cPS2FogVolume` for
//! each of the world's fog definitions and draws the ones near the camera each frame, and the volume
//! 0x1ca2c8..0x1cad68, which turns its points into depth-sorted, distance-faded sprites in a GS packet. On the bytes
//! at their addresses ([`Mem`]); the base object, the allocator, the free, delete, the renderer's per-frame matrix
//! update and its virtuals, and the packet's submit are calls out ([`FogEnv`]). Checked in the function runner
//! (`tests/fog.rs`).
//!
//! **The manager:** +8 its vtable (0x3916f0), +0xc the volumes, +0x10 up to 64 volume objects.
//!
//! **A volume object** (`cPS2FogVolume`, 0x1c bytes):
//! - +0 the vtable (0x3916c8);
//! - +4 its block, and in the block +0xc the points (16 bytes each), +0x18 their screen positions (16), +0x14 their
//!   colours (16), +0x10 their sizes (4);
//! - +8 the point count.
//!
//! **A fog definition** (from the world): +4 its groups, +8 a group (+4 its data, whose +0x20 is the point count and
//! +0x24 the points, 0x1c bytes each: position, three colour channels, size); +0x14 the volume object made for it.
//!
//! **A placed volume** (the world's cells): a matrix at +0, the definition at +0x40, the box +0x44 / +0x50.

use crate::animseq::{cvt_s_w, div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

pub const MAN_VTABLE: u32 = 0x3916f0;
pub const VOL_VTABLE: u32 = 0x3916c8;
pub const BASE_VOL_VTABLE: u32 = 0x391740;
pub const GAPP: u32 = 0x337c58;
/// The world (0x346488)
pub const WORLD: u32 = 0x346488;
/// The volume's sort entries (0x33ead0, 8 bytes: index, depth) and sprites (0x33d810, 0x30 bytes)
pub const SORT: u32 = 0x33ead0;
pub const SPRITES: u32 = 0x33d810;
/// The allocator's names: "FogVolume" (0x3916a8) and "FogVolData" (0x3916b8)
pub const NAME_VOLUME: u32 = 0x3916a8;
pub const NAME_DATA: u32 = 0x3916b8;

/// The calls out
pub trait FogEnv {
    /// The base object's constructor 0x17cf50(this, a1) and destructor 0x17cfe0(this, a1)
    fn base_construct(&mut self, m: &mut Mem, this: u32, a1: u32);
    fn base_destruct(&mut self, m: &mut Mem, this: u32, a1: u32);
    /// `EAMem_Alloc` 0x2ccf70(name, size, 0)
    fn alloc(&mut self, m: &mut Mem, name: u32, size: u32) -> u32;
    /// `Mem_Free` 0x14a040 / `operator delete` 0x114f80
    fn mem_free(&mut self, m: &mut Mem, p: u32);
    fn delete(&mut self, m: &mut Mem, p: u32);
    /// The renderer's per-frame matrices 0x1e85b0(renderer)
    fn update_matrices(&mut self, m: &mut Mem, r: u32);
    /// A renderer virtual (its address read from the vtable) with these words (floats as bits)
    fn renderer(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// The renderer's box test (+0x280) on the box's two corners: 0 inside, 1 outside, else partly
    fn cull_box(&mut self, m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32;
    /// 0x1e8838(renderer, packet, end, 0): the packet sent
    fn submit(&mut self, m: &mut Mem, r: u32, packet: u32, end: u32);
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rq(m: &Mem, a: u32) -> [F; 4] {
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wq(m: &mut Mem, a: u32, v: [F; 4]) {
    for k in 0..4 {
        wf(m, a + 4 * k, v[k as usize]);
    }
}
fn int(v: F) -> i32 {
    v.ee() as i32
}
fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}

fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}
fn vfn(m: &Mem, obj: u32, vt_at: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + vt_at);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn rcall(m: &mut Mem, env: &mut dyn FogEnv, off: u32, args: &[u32]) -> u32 {
    let r = renderer(m);
    let (this, f) = vfn(m, r, 0x10, off);
    env.renderer(m, f, this, args)
}

/// A volume object's virtual (its vtable at +0) by slot offset: the ported ones run here
fn vol_call(m: &mut Mem, env: &mut dyn FogEnv, obj: u32, off: u32, a1: u32, a2: u32, f12: F) {
    let (this, f) = vfn(m, obj, 0, off);
    match f {
        0x1ca358 => volume_destruct(m, env, this, a1),
        0x1ca2e0 => base_volume_destruct(m, env, this, a1),
        0x1ca3a8 => volume_build(m, env, this, a1),
        0x1ca5a0 => volume_draw(m, env, this, a1, a2, f12),
        _ => {}
    }
}

/// 0x252318(world, i): the world's fog definition i (its list at +4's +0x70), 0 past 4000 or without a list
pub fn fog_def(m: &Mem, world: u32, i: u32) -> u32 {
    if (i as i32) >= 4000 {
        return 0;
    }
    let w = m.r32(world + 4);
    if w == 0 {
        return 0;
    }
    let list = m.r32(w + 0x70);
    if list == 0 {
        return 0;
    }
    m.r32(list.wrapping_add(i.wrapping_mul(4)))
}

/// `cFogVolume`'s constructor 0x1ca2c8: its vtable
pub fn base_volume_construct(m: &mut Mem, this: u32) -> u32 {
    m.w32(this, BASE_VOL_VTABLE);
    this
}

/// `cFogVolume`'s destructor 0x1ca2e0(flags): its vtable; deleted with flag 1
pub fn base_volume_destruct(m: &mut Mem, env: &mut dyn FogEnv, this: u32, flags: u32) {
    m.w32(this, BASE_VOL_VTABLE);
    if flags & 1 != 0 {
        env.delete(m, this);
    }
}

/// `cPS2FogVolume`'s constructor 0x1ca320: the base's, then no block and its own vtable
pub fn volume_construct(m: &mut Mem, this: u32) -> u32 {
    base_volume_construct(m, this);
    m.w32(this + 4, 0);
    m.w32(this, VOL_VTABLE);
    this
}

/// `cPS2FogVolume`'s destructor 0x1ca358(flags): its vtable, the block freed, then the base's
pub fn volume_destruct(m: &mut Mem, env: &mut dyn FogEnv, this: u32, flags: u32) {
    m.w32(this, VOL_VTABLE);
    let b = m.r32(this + 4);
    if b != 0 {
        env.mem_free(m, b);
    }
    base_volume_destruct(m, env, this, flags);
}

/// `cPS2FogVolume` vf1 0x1ca3a8(definition): the points counted (the definition's first group's count once for each
/// of its groups: the game reads the same group every time), a block of 0x34 bytes a point (filled with 0xdeadc0ed)
/// split into the points, screen positions, colours and sizes, then each group's points copied in (position with w
/// 1, size, colour 128 and its three channels × 128)
pub fn volume_build(m: &mut Mem, env: &mut dyn FogEnv, this: u32, def: u32) {
    m.w32(this + 8, 0);
    let groups = m.r32(def + 4) as i32;
    let g = m.r32(def + 8);
    let mut n = 0u32;
    for _ in 0..groups.max(0) {
        let d = m.r32(g + 4);
        if d == 0 {
            continue;
        }
        let k = m.r32(d + 0x20);
        if k != 0 {
            n = n.wrapping_add(k);
            m.w32(this + 8, n);
        }
    }
    let size = n.wrapping_mul(0x34);
    let p = env.alloc(m, NAME_DATA, size);
    if p != 0 {
        crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, size);
    }
    let c = m.r32(this + 8);
    m.w32(this + 4, p);
    m.w32(this + 0xc, p);
    m.w32(this + 0x18, p.wrapping_add(c << 4));
    m.w32(this + 0x14, p.wrapping_add(c << 5));
    m.w32(this + 0x10, p.wrapping_add(c.wrapping_mul(0x30)));
    let mut out = 0u32;
    for _ in 0..groups.max(0) {
        let d = m.r32(g + 4);
        if d == 0 {
            continue;
        }
        let k = m.r32(d + 0x20) as i32;
        if k == 0 {
            continue;
        }
        let mut src = m.r32(d + 0x24);
        for _ in 0..k.max(0) {
            let (pts, sizes, cols) = (m.r32(this + 0xc), m.r32(this + 0x10), m.r32(this + 0x14));
            let at = pts.wrapping_add(out << 4);
            let v = [rf(m, src), rf(m, src + 4), rf(m, src + 8)];
            wq(m, at, [v[0], v[1], v[2], F(1.0)]);
            m.w32(sizes.wrapping_add(out << 2), m.r32(src + 0x18));
            let col = cols.wrapping_add(out << 4);
            let ch = [rf(m, src + 0xc), rf(m, src + 0x10), rf(m, src + 0x14)];
            wq(m, col, [F(128.0), ch[0] * F(128.0), ch[1] * F(128.0), ch[2] * F(128.0)]);
            src = src.wrapping_add(0x1c);
            out = out.wrapping_add(1);
        }
    }
}

/// Four rows times a row vector on VU0 (vmulax / vmadday / vmaddaz / vmaddw): Σ rows[k] × v[k]
fn mat_vec(rows: &[[F; 4]; 4], v: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|i| ((rows[0][i] * v[0] + rows[1][i] * v[1]) + rows[2][i] * v[2]) + rows[3][i] * v[3])
}
fn rows(m: &Mem, a: u32) -> [[F; 4]; 4] {
    [0, 1, 2, 3].map(|r| rq(m, a + 0x10 * r))
}

/// The sprite's outcode against the renderer's clip box (+0x17b0.. min, +0x17c0.. max): 2 / 1 left / right, 8 / 4
/// below / above, and with w below 0 the code flipped (^ 0x2f), else 0x20 / 0x10 for z past either end
fn outcode(m: &Mem, r: u32, p: u32) -> u32 {
    let (x, y, w) = (rf(m, p), rf(m, p + 4), rf(m, p + 0xc));
    let mut c = if x.ee() < rf(m, r + 0x17b0).ee() { 2 } else { 0 };
    if rf(m, r + 0x17c0).ee() < x.ee() {
        c |= 1;
    }
    if y.ee() < rf(m, r + 0x17b4).ee() {
        c |= 8;
    }
    if rf(m, r + 0x17c4).ee() < y.ee() {
        c |= 4;
    }
    if w.ee() < 0.0 {
        return c ^ 0x2f;
    }
    let z = rf(m, p + 8);
    if rf(m, r + 0x17b8).ee() < z.ee() {
        c |= 0x20;
    }
    if z.ee() < rf(m, r + 0x17c8).ee() {
        c |= 0x10;
    }
    c
}

/// `cPS2FogVolume` vf3 0x1ca5a0(matrix, clip, alpha): the volume drawn.
/// - The renderer's matrices updated; the matrix's position through the renderer's +0x5b0 rows, its w scaled by
///   +0x630's w, kept in 0..+0x630's x and made an integer: its low 16 bits to the renderer's +0x1e8 (2, that).
/// - Each point through the matrix and the renderer's +0x1770 rows into its screen position; sorted by the
///   screen w (the C library's `qsort` 0x2fb5c8, the comparison `a < b`).
/// - Each point deeper than 500: projected (x, y over w, w its reciprocal), a sprite of its size × the renderer's
///   scale (+0x1730, +0x1744) each way; with `clip` the sprites wholly out on one side dropped; its colour, and its
///   alpha × `alpha`, faded by the square of how far it is between 500 and 2601.95 past 500.
/// - The sprites written as one GIF packet (two vertices each) at the renderer's +0x680 and sent.
pub fn volume_draw(m: &mut Mem, env: &mut dyn FogEnv, this: u32, mat: u32, clip: u32, alpha: F) {
    let r = renderer(m);
    env.update_matrices(m, r);
    let (fthis, ffn) = vfn(m, r, 0x10, 0x1e8);
    let pos = rq(m, mat + 0x30);
    env.update_matrices(m, r);
    let mut v = mat_vec(&rows(m, r + 0x5b0), pos);
    let k = rq(m, r + 0x630);
    v[3] = v[3] * k[3];
    let key = |x: F| {
        let b = x.0.to_bits();
        if b & 0x8000_0000 != 0 { -((b & 0x7fff_ffff) as i64) - 1 } else { b as i64 }
    };
    if key(v[3]) < key(F(0.0)) {
        v[3] = F(0.0);
    }
    if key(k[0]) < key(v[3]) {
        v[3] = k[0];
    }
    let fogv = (v[3].0 as f64).clamp(i32::MIN as f64, i32::MAX as f64) as i32 as u32 & 0xffff;
    env.renderer(m, ffn, fthis, &[2, fogv]);
    // the screen matrix: the volume's rows through the renderer's +0x1770
    let sm = rows(m, r + 0x1770);
    let mrows = rows(m, mat);
    let screen: [[F; 4]; 4] = [0, 1, 2, 3].map(|i| mat_vec(&sm, mrows[i]));
    let n = m.r32(this + 8) as i32;
    for t in 0..n.max(0) as u32 {
        let p = rq(m, m.r32(this + 0xc).wrapping_add(t << 4));
        let s = mat_vec(&screen, p);
        let at = m.r32(this + 0x18).wrapping_add(t << 4);
        m.w32(SORT + 8 * t, t & 0xffff);
        wq(m, at, s);
        let w = rf(m, m.r32(this + 0x18).wrapping_add(t << 4) + 0xc);
        m.w32(SORT + 8 * t + 4, int(w) as u32);
    }
    // the sort: the entries' order as the game's qsort leaves it with this comparison
    let mut entries: Vec<(u32, i32)> = (0..n.max(0) as u32).map(|t| (m.r32(SORT + 8 * t), m.r32(SORT + 8 * t + 4) as i32)).collect();
    tricky_data::disc::qsort(&mut entries, |a, b| (a.1 < b.1) as i32);
    for (t, e) in entries.iter().enumerate() {
        m.w32(SORT + 8 * t as u32, e.0);
        m.w32(SORT + 8 * t as u32 + 4, e.1 as u32);
    }
    let mut out = 0u32;
    for t in 0..n.max(0) as u32 {
        let e = SORT + 8 * t;
        let depth = cvt_s_w(m.r32(e + 4) as i32);
        if !(F(500.0).ee() < depth.ee()) {
            continue;
        }
        let idx = m.r32(e);
        let sp = m.r32(this + 0x18).wrapping_add(idx << 4);
        let inv = div(F(1.0), rf(m, sp + 0xc));
        wf(m, sp + 0xc, inv);
        let x = rf(m, sp) * inv;
        let y = rf(m, sp + 4) * inv;
        wf(m, sp, x);
        wf(m, sp + 4, y);
        let s = rf(m, m.r32(this + 0x10).wrapping_add(idx << 2)) * inv;
        let dy = rf(m, r + 0x1744) * s;
        let dx = rf(m, r + 0x1730) * s;
        let slot = SPRITES + 0x30 * out;
        let c = rq(m, sp);
        wq(m, slot, [c[0] - dx, c[1] - dy, c[2] - F(0.0), c[3] - F(0.0)]);
        let c = rq(m, m.r32(this + 0x18).wrapping_add(idx << 4));
        wq(m, slot + 0x10, [c[0] + dx, c[1] + dy, c[2] + F(0.0), c[3] + F(0.0)]);
        if clip != 0 && outcode(m, r, slot) & outcode(m, r, slot + 0x10) != 0 {
            continue;
        }
        let col = m.r32(this + 0x14).wrapping_add(idx << 4);
        for k in 1..4 {
            m.w32(slot + 0x20 + 4 * k, m.r32(col + 4 * k));
        }
        let a = if depth.ee() < f32::from_bits(0x4522_9f32) {
            let f = div(depth - F(500.0), F(f32::from_bits(0x4503_5f32)));
            rf(m, col) * (f * f) * alpha
        } else {
            rf(m, col) * alpha
        };
        wf(m, slot + 0x20, a);
        out += 1;
    }
    if out == 0 {
        return;
    }
    // the packet: a tag, the registers' settings, then two vertices a sprite (ST, RGBAQ, XYZ2; ST, XYZ2)
    let buf = m.r32(r + 0x680);
    let odd = out & 1;
    let qw = ((out * 5) >> 1) + if odd != 0 { 4 } else { 3 };
    w64(m, buf, ((qw + 1) | 0x1000_0000) as u64);
    w64(m, buf + 8, 0);
    w64(m, buf + 0x10, 0);
    w64(m, buf + 0x18, ((qw | 0x5000_0000) as u64) << 32);
    w64(m, buf + 0x20, 0x1400_0000_0000_8001);
    w64(m, buf + 0x28, 0);
    w64(m, buf + 0x30, 0x56);
    w64(m, buf + 0x38, 0);
    w64(m, buf + 0x40, (out | 0x8000) as u64 | 0x5400_0000_0000_0000);
    w64(m, buf + 0x48, 0x52512);
    let sx = |v: F| int(v) as i64 as u64;
    let one = 1f32.to_bits() as u64;
    let mut at = buf + 0x50;
    for k in 0..out {
        let s = SPRITES + 0x30 * k;
        let (lo, hi) = (rq(m, s), rq(m, s + 0x10));
        let c = rq(m, s + 0x20);
        let rgbaq = (one << 32 | sx(c[1])) | ((sx(c[3]) << 16) | (sx(c[2]) << 8)) | (sx(c[0]) << 24);
        let xyz_lo = (sx(lo[0]) | sx(lo[1]) << 16) | sx(lo[2] * lo[3]) << 32;
        let xyz_hi = (sx(hi[0]) | sx(hi[1]) << 16) | sx(hi[2] * hi[3]) << 32;
        w64(m, at, 0);
        w64(m, at + 8, rgbaq);
        w64(m, at + 0x10, xyz_lo);
        w64(m, at + 0x18, one << 32 | one);
        w64(m, at + 0x20, xyz_hi);
        at += 0x28;
    }
    if odd != 0 {
        w64(m, at, 0);
        at += 8;
    }
    m.w32(r + 0x680, at + 0x10);
    env.submit(m, r, buf, at);
}

/// The manager's constructor 0x1c9b20(this, a1): the base object, the vtable, the volumes built
pub fn manager_construct(m: &mut Mem, env: &mut dyn FogEnv, this: u32, a1: u32) -> u32 {
    env.base_construct(m, this, a1);
    m.w32(this + 8, MAN_VTABLE);
    manager_build(m, env, this);
    this
}

/// The manager's destructor 0x1c9b60(flags): the vtable, every volume deleted, the base's
pub fn manager_destruct(m: &mut Mem, env: &mut dyn FogEnv, this: u32, flags: u32) {
    m.w32(this + 8, MAN_VTABLE);
    manager_delete_all(m, env, this);
    env.base_destruct(m, this, flags);
}

/// `FogObjMan_Build` 0x1c9ba8: the 64 slots cleared; for each of the world's fog definitions (its +0x30 of them) a
/// volume object (0x1c bytes, filled with 0xdeadc0de) constructed, noted in its slot and on the definition, and
/// built from it (vtable slot +0x10)
pub fn manager_build(m: &mut Mem, env: &mut dyn FogEnv, this: u32) {
    for k in (0..0x40u32).rev() {
        m.w32(this + 0x10 + 4 * k, 0);
    }
    let n = m.r32(m.r32(WORLD + 4) + 0x30);
    m.w32(this + 0xc, n);
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0xc) as i32 {
        let def = fog_def(m, WORLD, i);
        let p = env.alloc(m, NAME_VOLUME, 0x1c);
        if p != 0 {
            crate::replaymgr::mem_set_pub(m, p, 0xdead_c0de, 0x1c);
        }
        let obj = volume_construct(m, p);
        m.w32(this + 0x10 + 4 * i, obj);
        m.w32(def + 0x14, obj);
        vol_call(m, env, obj, 0x10, def, 0, F(0.0));
        i += 1;
    }
}

/// `FogObjMan_DeleteAll` 0x1c9cd0: every volume deleted (vtable slot +8 with flags 3)
pub fn manager_delete_all(m: &mut Mem, env: &mut dyn FogEnv, this: u32) {
    let mut i = 0u32;
    while (i as i32) < m.r32(this + 0xc) as i32 {
        let obj = m.r32(this + 0x10 + 4 * i);
        if obj != 0 {
            vol_call(m, env, obj, 8, 3, 0, F(0.0));
        }
        i += 1;
    }
}

/// The manager's draw (vf2 0x1c9d58): the renderer set up; the camera's position in the world (its matrix's last row
/// negated, through the matrix again); then, cell by cell (the world's +0x3c list, up to 0xa2 or the first empty),
/// each of the world's fog slots (its +0x48) the cell's mask (+0x36) has (the cell's records at +0x38, 0x4c bytes):
/// each placed volume tested against the view when the cell's other mask
/// (+0x32) says so (outside: skipped), and, unless 20000.09 away or more, drawn (its definition's object, slot +0x20)
/// with the test's answer and an alpha of 1 within 12500.37, fading to 0 at 20000.09; the renderer put back (+0x1a8).
/// When no test was made the answer passed is the last one made (`last`; the game's is a stack word).
pub fn manager_draw(m: &mut Mem, env: &mut dyn FogEnv, this: u32, last: &mut u32) {
    let _ = this;
    let app = m.r32(GAPP);
    let a88 = m.r32(m.r32(app + 0x30) + 0x88);
    rcall(m, env, 0x1a0, &[]);
    rcall(m, env, 0x1d0, &[4]);
    rcall(m, env, 0x1f0, &[a88, 0]);
    rcall(m, env, 0x1f8, &[1, 0]);
    rcall(m, env, 0x1f0, &[u32::MAX, 1]);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1c0, &[1]);
    rcall(m, env, 0x1c8, &[0, 2, 0]);
    let cam_m = rcall(m, env, 0xf8, &[]);
    let p = rq(m, cam_m + 0x30);
    let neg = p.map(|x| x * F(-1.0));
    let cm = rcall(m, env, 0xf8, &[]);
    let mr = rows(m, cm);
    let dot = |r: [F; 4]| ((neg[0] * r[0] + neg[1] * r[1]) + neg[2] * r[2]) + neg[3] * r[3];
    let cam = [dot(mr[0]), dot(mr[1]), dot(mr[2]), F(1.0)];
    let mut cell_i = 0u32;
    while cell_i < 0xa2 {
        let cell = m.r32(WORLD + 0x3c + 4 * cell_i);
        if cell == 0 {
            break;
        }
        cell_i += 1;
        if m.r16(cell + 0x36) as i16 == 0 {
            continue;
        }
        let mut s = 0u32;
        while (s as i32) < m.r32(m.r32(WORLD) + 0x48) as i32 {
            let rec = m.r32(cell + 0x38).wrapping_add(0x4c * s);
            s += 1;
            let s = s - 1;
            let nv = m.r16(rec + 0x30) as i16 as i32;
            if nv == 0 || (m.r16(cell + 0x36) as i16 as i32 >> (s & 31)) & 1 == 0 {
                continue;
            }
            let test = (m.r16(cell + 0x32) as i16 as i32 >> (s & 31)) & 1 == 0;
            let vols = m.r32(rec + 0x48);
            for k in 0..nv.max(0) as u32 {
                let v = m.r32(vols + 4 * k);
                let (lo, hi) = (rq(m, v + 0x44), rq(m, v + 0x50));
                if test {
                    let min = [lo[0].0.to_bits(), lo[1].0.to_bits(), lo[2].0.to_bits(), 1f32.to_bits()];
                    let max = [hi[0].0.to_bits(), hi[1].0.to_bits(), hi[2].0.to_bits(), 1f32.to_bits()];
                    let r = renderer(m);
                    let (rt, f) = vfn(m, r, 0x10, 0x280);
                    *last = env.cull_box(m, f, rt, min, max);
                    if *last == 1 {
                        continue;
                    }
                }
                let centre = [(lo[0] + hi[0]) * F(0.5), (lo[1] + hi[1]) * F(0.5), (lo[2] + hi[2]) * F(0.5), F(1.0)];
                let d = [0, 1, 2, 3].map(|i| centre[i] - cam[i]);
                let d = [d[0], d[1], d[2], F(0.0)];
                let sq = d.map(|x| x * x);
                let dist = sqrt(((sq[0] + sq[1]) + F(1.0) * sq[2]) + F(1.0) * sq[3]);
                let far = F(f32::from_bits(0x469c_405d));
                if far.ee() <= dist.ee() {
                    continue;
                }
                let a = if dist.ee() < f32::from_bits(0x4643_517f) {
                    F(1.0)
                } else {
                    div(far - dist, F(f32::from_bits(0x45ea_5e76)))
                };
                let obj = m.r32(m.r32(v + 0x40) + 0x14);
                vol_call(m, env, obj, 0x20, v, *last, a);
            }
        }
    }
    rcall(m, env, 0x1a8, &[]);
}

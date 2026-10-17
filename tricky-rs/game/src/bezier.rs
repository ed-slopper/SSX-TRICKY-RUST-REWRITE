//! The Bezier manager's queue (board row G7a3a): `cPS2BezierMan` 0x1d9be0..0x1d9f54 and 0x1db150..0x1db930, the
//! renderer's object (+0x484) that takes the course's patches as the terrain's draw hands them over (the renderer's
//! +0x2d0) and keeps them in lists by tessellation until its vf15 draws them (row G7a3b). On the bytes at their
//! addresses ([`Mem`]); the allocator, the fill, the free, the delete and the Bezier tables' build (row G7a3c) are
//! calls out ([`BezierEnv`]). Checked in the function runner (`tests/bezier.rs`).
//!
//! **The manager**: +0 its vtable (0x392fd8); +0x10 / +0x14 the near and far LOD distances (3000, 15000); +0x18,
//! +0x1c, +0x20, +0x24 four switches; +0x28 8.0; +0x2c the views (0xa0 each from +0x30: a quad, two matrices, a
//! float and a word); +0x530 the tables; +0x544 the patch records (0x18 each, +0x548 of them); +0x54c / +0x550,
//! +0x554 / +0x558, +0x55c / +0x560, +0x564 / +0x568 four lists (a count and a head): the patches tessellated 4, 6
//! and 8 all round, and the mixed ones.
//!
//! **A record**: +0 its list (0..3), +4 3 when clipped, +8 the four edges' steps, +0xc the patch's first short, +0xe
//! its second (−1 when negative or the +0x1c switch is off), +0x10 the patch, +0x14 the next in its list.

use crate::pf::Mem;
use crate::ps2f::F;

pub const VTABLE: u32 = 0x392fd8;
pub const BASE_VTABLE: u32 = 0x3930d8;
/// The patch records' name for the allocator
pub const NAME: u32 = 0x392fb8;
pub const ALLOC: u32 = 0x2ccf70;
pub const MEM_SET: u32 = 0x2cc808;
pub const MEM_FREE: u32 = 0x14a040;
pub const DELETE: u32 = 0x114f80;
pub const BUILD_TABLES: u32 = 0x1db1e8;
pub const BEGIN: u32 = 0x1d9d20;
pub const QUEUE: u32 = 0x1d9d50;

/// The calls out
pub trait BezierEnv {
    /// A function with these words; its answer
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// A virtual not ported here with these words
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
}

fn vfn(m: &Mem, obj: u32, vt_at: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + vt_at);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}

/// The manager's own virtual at `off`: the port's when it is begin or the queue, else a call out
fn own(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, off: u32, args: &[u32]) -> u32 {
    let (t, f) = vfn(m, this, 0, off);
    match f {
        BEGIN => {
            begin(m, t);
            0
        }
        QUEUE if args.len() == 4 => {
            queue(m, t, args[0], args[1], args[2], args[3]);
            0
        }
        _ => env.vcall(m, f, t, args),
    }
}

/// `cPS2BezierMan::cPS2BezierMan` 0x1d9be0: the vtable, then [`init`]
pub fn construct(m: &mut Mem, env: &mut dyn BezierEnv, this: u32) -> u32 {
    m.w32(this, VTABLE);
    init(m, env, this);
    this
}

/// `cPS2BezierMan::~cPS2BezierMan` 0x1d9c40(this, flags): the vtable, [`shutdown`], then `cBezierMan`'s
/// ([`base_destruct`])
pub fn destruct(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, flags: u32) {
    m.w32(this, VTABLE);
    shutdown(m, env, this);
    base_destruct(m, env, this, flags);
}

/// `cBezierMan::~cBezierMan` 0x1db908(this, flags): its vtable; deleted with flag 1
pub fn base_destruct(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, flags: u32) {
    m.w32(this, BASE_VTABLE);
    if flags & 1 != 0 {
        env.call(m, DELETE, &[this, flags & 1]);
    }
}

/// vf1 0x1d9c88: the defaults (+0x24 0, +0x20 1, +0x28 8, the distances 3000 and 15000, +0x18 and +0x1c 1); the
/// patch records ([`patch_alloc`]); no tables; the tables built (`Render_BuildBezierTables`)
pub fn init(m: &mut Mem, env: &mut dyn BezierEnv, this: u32) {
    m.w32(this + 0x24, 0);
    m.w32(this + 0x20, 1);
    m.w32(this + 0x28, 8f32.to_bits());
    m.w32(this + 0x10, 3000f32.to_bits());
    m.w32(this + 0x14, 15000f32.to_bits());
    m.w32(this + 0x18, 1);
    m.w32(this + 0x1c, 1);
    patch_alloc(m, env, this);
    m.w32(this + 0x530, 0);
    env.call(m, BUILD_TABLES, &[this]);
}

/// vf2 0x1d9cf8: the patch records freed (0x1db1c8, when there are any) and the tables (0x1db4f8, then none)
pub fn shutdown(m: &mut Mem, env: &mut dyn BezierEnv, this: u32) {
    let p = m.r32(this + 0x544);
    if p != 0 {
        env.call(m, MEM_FREE, &[p]);
    }
    let t = m.r32(this + 0x530);
    if t != 0 {
        env.call(m, MEM_FREE, &[t]);
        m.w32(this + 0x530, 0);
    }
}

/// `RendPatch_Alloc` 0x1db150: 0xc000 bytes for the patch records (filled with 0xdeadc0ed when got) at +0x544,
/// then begin (vf14, by the vtable)
pub fn patch_alloc(m: &mut Mem, env: &mut dyn BezierEnv, this: u32) {
    let p = env.call(m, ALLOC, &[NAME, 0xc000, 0]);
    if p != 0 {
        env.call(m, MEM_SET, &[p, 0xdead_c0ed, 0xc000]);
    }
    m.w32(this + 0x544, p);
    own(m, env, this, 0x78, &[]);
}

/// vf3 0x1db890 / vf4 0x1db888: the near / far LOD distance set; vf5 0x1db880 / vf6 0x1db878 read
pub fn set_near(m: &mut Mem, this: u32, v: F) {
    m.w32(this + 0x10, v.0.to_bits());
}
pub fn set_far(m: &mut Mem, this: u32, v: F) {
    m.w32(this + 0x14, v.0.to_bits());
}
pub fn near(m: &Mem, this: u32) -> F {
    F(f32::from_bits(m.r32(this + 0x10)))
}
pub fn far(m: &Mem, this: u32) -> F {
    F(f32::from_bits(m.r32(this + 0x14)))
}
/// vf7 0x1db870 (+0x24), vf8 0x1db868 (+0x18), vf9 0x1db860 (+0x1c), vf10 0x1db858 (+0x20) set; vf11 0x1db850
/// (+0x18), vf12 0x1db848 (+0x1c), vf13 0x1db840 (+0x20) read; so do 0x1db8b0..0x1db898 (+0x534..+0x540)
pub fn set_switch(m: &mut Mem, this: u32, at: u32, v: u32) {
    m.w32(this + at, v);
}
pub fn switch(m: &Mem, this: u32, at: u32) -> u32 {
    m.r32(this + at)
}

/// vf14 0x1d9d20 (+0x78): no views, no records, the four lists empty
pub fn begin(m: &mut Mem, this: u32) {
    for at in [0x2c, 0x548, 0x54c, 0x554, 0x55c, 0x564, 0x550, 0x558, 0x560, 0x568] {
        m.w32(this + at, 0);
    }
}

/// vf16 0x1d9d50 (+0x88)(this, patch, steps, shorts, clip): the next record (3 when clipped, the patch, its four
/// steps, its shorts' low halves: the second −1 when negative or with the +0x1c switch off); then into the list of
/// its tessellation when all four steps are one (4, 6 or 8; any other one in none), else the mixed one, at the head
pub fn queue(m: &mut Mem, this: u32, patch: u32, steps: u32, shorts: u32, clip: u32) {
    let n = m.r32(this + 0x548);
    let rec = m.r32(this + 0x544).wrapping_add(n.wrapping_mul(0x18));
    m.w32(this + 0x548, n.wrapping_add(1));
    m.w32(rec + 4, if clip != 0 { 3 } else { 0 });
    m.w32(rec + 0x10, patch);
    for k in 0..4 {
        let b = m.r8(steps + k);
        m.w8(rec + 8 + k, b);
    }
    let s0 = m.r16(shorts);
    let sign = m.r32(shorts + 4) as i32;
    m.w16(rec + 0xc, s0);
    let second = if sign < 0 || m.r32(this + 0x1c) == 0 { 0xffff } else { m.r16(shorts + 4) };
    m.w16(rec + 0xe, second);
    let s = [0, 1, 2, 3].map(|k| m.r8(steps + k));
    let (list, count, head) = if s[0] != s[1] || s[2] != s[3] || s[0] != s[2] {
        (3, 0x564, 0x568)
    } else {
        match s[0] {
            4 => (0, 0x54c, 0x550),
            6 => (1, 0x554, 0x558),
            8 => (2, 0x55c, 0x560),
            _ => return,
        }
    };
    let old = m.r32(this + head);
    m.w32(rec + 0x14, old);
    m.w32(rec, list);
    let c = m.r32(this + count);
    m.w32(this + count, c.wrapping_add(1));
    m.w32(this + head, rec);
}

/// vf17 0x1d9eb8(this, quad, a, b, word, f): the next view: the quad, the two matrices, the float and the word
pub fn add_view(m: &mut Mem, this: u32, quad: u32, a: u32, b: u32, word: u32, f: F) {
    let at = |m: &Mem| this.wrapping_add(m.r32(this + 0x2c).wrapping_mul(0xa0));
    let copy = |m: &mut Mem, to: u32, from: u32, n: u32| {
        for q in 0..n {
            let w = [0, 1, 2, 3].map(|k| m.r32((from & !15) + 0x10 * q + 4 * k));
            for (k, w) in w.into_iter().enumerate() {
                m.w32((to & !15) + 0x10 * q + 4 * k as u32, w);
            }
        }
    };
    let v = at(m);
    copy(m, v + 0x30, quad, 1);
    let v = at(m);
    copy(m, v + 0x40, a, 4);
    let v = at(m);
    copy(m, v + 0x80, b, 4);
    let v = at(m);
    m.w32(v + 0xc4, word);
    m.w32(v + 0xc0, f.0.to_bits());
    let n = m.r32(this + 0x2c);
    m.w32(this + 0x2c, n.wrapping_add(1));
}

/// The renderer's +0x20 (0x1ea068): its Bezier manager (+0x484)
pub fn renderer_bezier(m: &Mem, r: u32) -> u32 {
    m.r32(r + 0x484)
}
/// The renderer's +0x2c0 (0x1e9068): the manager's +0x78 (begin), with the manager as its second word too
pub fn renderer_patch_begin(m: &mut Mem, env: &mut dyn BezierEnv, r: u32) {
    let bz = m.r32(r + 0x484);
    own(m, env, bz, 0x78, &[bz]);
}
/// The renderer's +0x2d0 (0x1e9008)(r, patch, steps, shorts, clip): the manager's +0x88 (the queue)
pub fn renderer_patch_draw(m: &mut Mem, env: &mut dyn BezierEnv, r: u32, patch: u32, steps: u32, shorts: u32, clip: u32) {
    let bz = m.r32(r + 0x484);
    own(m, env, bz, 0x88, &[patch, steps, shorts, clip]);
}

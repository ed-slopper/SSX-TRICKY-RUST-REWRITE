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

// ---- The queue drawn (row G7a3b) ----

/// The renderer's per-frame matrices (0x1e85b0) and a packet sent (0x1e8838)
pub const MATRICES: u32 = 0x1e85b0;
pub const SUBMIT: u32 = 0x1e8838;
/// The four lists' VU1 programs: a word for each clip kind (+4 + 1), for the patch and (+4 + 2) its view's pass
pub const PROGRAMS: [u32; 4] = [0x3d0f58, 0x3d0f40, 0x3d0f28, 0x3d0f10];
/// 0.08, the renderer's +0x1c8's float on the way in and out
const BLEND: u32 = 0x3da3_d70a;

/// The renderer's virtual at `off` (its vtable at +0x10) with these words
fn rv(m: &mut Mem, env: &mut dyn BezierEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let (t, f) = vfn(m, r, 0x10, off);
    env.vcall(m, f, t, args)
}

fn sd(m: &mut Mem, at: u32, v: u64) {
    m.w32(at, v as u32);
    m.w32(at + 4, (v >> 32) as u32);
}

/// The texture state's program check (the renderer's +0x470): when its byte +0x10 isn't `kind` or its word is set,
/// the byte becomes `kind`, the word 0 and the renderer's +0x474 1
fn use_program(m: &mut Mem, r: u32, kind: u32) {
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) as u32 != kind || m.r32(ts) != 0 {
        m.w8(ts + 0x10, kind as u8);
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
    }
}

/// The patch's two textures (+0xc, +0xe) bound when they differ from the last ones (the renderer's +0x1f0 with
/// the slot): a stored short read back as a half, the next compared as signed, so a half of 0x8000 up is always
/// bound again
fn bind_textures(m: &mut Mem, env: &mut dyn BezierEnv, r: u32, rec: u32, last: &mut [u32; 2]) {
    for (k, at) in [0xcu32, 0xe].into_iter().enumerate() {
        let t = m.r16(rec + at) as i16 as i32 as u32;
        if t != last[k] {
            rv(m, env, r, 0x1f0, &[t, k as u32]);
            last[k] = m.r16(rec + at) as u32;
        }
    }
}

/// The patch's corner (its floats ×128, made whole) as the GIF's register word: x << 4, (x + z − 1) << 14 | 0xa,
/// y << 24, (y + z − 1) << 34
fn corner_word(m: &Mem, patch: u32) -> u64 {
    let c = |k: u32| crate::sndlib::cvt_w((F(f32::from_bits(m.r32(patch + 4 * k))) * F(128.0)).0) as i64;
    let (x, y, z) = (c(0), c(1), c(2));
    let sx = |v: i64| v as i32 as i64;
    let a = sx(x + z);
    let b = sx(y + z);
    let hi = (sx(b - 1) as u64) << 34;
    ((x as u64) << 4) | hi | ((sx(a - 1) as u64) << 14) | 0xa | ((y as u64) << 24)
}

/// The program word for a record in a list: `PROGRAMS[list]`'s table at the record's clip kind + `plus`
fn program(m: &Mem, entry: u32, rec: u32, plus: u32) -> u32 {
    let table = m.r32(entry + 0xc);
    m.r32(table.wrapping_add(m.r32(rec + 4).wrapping_add(plus).wrapping_mul(4)))
}

/// A uniform patch's packet at the renderer's +0x680 (0x90 bytes, then sent with its end at +0x80)
fn uniform_packet(m: &mut Mem, r: u32, entry: u32, rec: u32) -> u32 {
    let p = m.r32(r + 0x680);
    let patch = m.r32(rec + 0x10);
    sd(m, p + 0x18, 0xd82b << 47);
    sd(m, p, 0x1000_0001);
    sd(m, p + 8, 0);
    sd(m, p + 0x10, 0);
    sd(m, p + 0x30, 0x1000_0004);
    sd(m, p + 0x40, 0x8000 << 45);
    sd(m, p + 0x48, 0x5000_0002 << 32);
    sd(m, p + 0x20, ((patch as u64) << 32) | 0x3000_0015);
    sd(m, p + 0x50, 0x1000_0000_0000_8001);
    sd(m, p + 0x58, 0xe);
    sd(m, p + 0x28, 0);
    sd(m, p + 0x38, 0);
    let w = corner_word(m, patch);
    let prog = program(m, entry, rec, 1);
    sd(m, p + 0x78, ((prog | 0x1400_0000) as u64) << 32);
    m.w32(r + 0x680, p.wrapping_add(0x90));
    sd(m, p + 0x60, w);
    sd(m, p + 0x68, 9);
    sd(m, p + 0x70, 0);
    p
}

/// A mixed patch's packet (0xa0 bytes: its four steps as words first, then sent with its end at +0x90)
fn mixed_packet(m: &mut Mem, r: u32, entry: u32, rec: u32) -> u32 {
    let p = m.r32(r + 0x680);
    let patch = m.r32(rec + 0x10);
    sd(m, p, 0x1000_0002);
    m.w32(r + 0x680, p.wrapping_add(0x20));
    sd(m, p + 0x18, 0xd82d << 47);
    sd(m, p + 8, 0);
    sd(m, p + 0x10, 0);
    for k in 0..4 {
        let s = m.r8(rec + 8 + k) as u32;
        m.w32(p + 0x20 + 4 * k, s);
    }
    sd(m, p + 0x40, 0x1000_0004);
    sd(m, p + 0x50, 0x8000 << 45);
    sd(m, p + 0x58, 0x5000_0002 << 32);
    sd(m, p + 0x30, ((patch as u64) << 32) | 0x3000_0015);
    sd(m, p + 0x60, 0x1000_0000_0000_8001);
    sd(m, p + 0x68, 0xe);
    sd(m, p + 0x38, 0);
    sd(m, p + 0x48, 0);
    let w = corner_word(m, patch);
    sd(m, p + 0x80, 0);
    let prog = program(m, entry, rec, 1);
    sd(m, p + 0x70, w);
    sd(m, p + 0x88, ((prog | 0x1400_0000) as u64) << 32);
    sd(m, p + 0x78, 9);
    m.w32(r + 0x680, p.wrapping_add(0xa0));
    p
}

/// A view's matrix b (+0x80..+0xc0 of the view) as eight doublewords at `to`
fn view_matrix(m: &mut Mem, view: u32, to: u32) {
    for j in 0..8 {
        let lo = m.r32(view + 0x80 + 8 * j);
        let hi = m.r32(view + 0x84 + 8 * j);
        sd(m, to + 8 * j, ((hi as u64) << 32) | lo as u64);
    }
}

/// A uniform patch's pass for a view (0xb0 bytes: the view's matrix b, then the program at the clip kind + 2; sent
/// with its end at +0xa0)
fn view_packet(m: &mut Mem, r: u32, entry: u32, rec: u32, this: u32, v: u32) -> u32 {
    let p = m.r32(r + 0x680);
    let view = this.wrapping_add(v.wrapping_mul(0xa0));
    sd(m, p, 0x1000_0009);
    sd(m, p + 8, 0);
    sd(m, p + 0x10, 0);
    sd(m, p + 0x18, 0xd809 << 47);
    let prog = program(m, entry, rec, 2);
    view_matrix(m, view, p + 0x20);
    m.w32(r + 0x680, p.wrapping_add(0xb0));
    sd(m, p + 0x60, 0x8000 << 45);
    sd(m, p + 0x68, 0x5000_0002 << 32);
    sd(m, p + 0x70, 0x1000_0000_0000_8001);
    sd(m, p + 0x78, 0xe);
    sd(m, p + 0x80, 5);
    sd(m, p + 0x88, 9);
    sd(m, p + 0x90, ((prog | 0x1500_0000) as u64) << 32);
    sd(m, p + 0x98, 0);
    p
}

/// A mixed patch's pass for a view (0xc0 bytes: its steps, the view's matrix b, the program; end at +0xb0)
fn view_packet_mixed(m: &mut Mem, r: u32, entry: u32, rec: u32, this: u32, v: u32) -> u32 {
    let p = m.r32(r + 0x680);
    let view = this.wrapping_add(v.wrapping_mul(0xa0));
    sd(m, p + 0x18, 0xd80b << 47);
    sd(m, p, 0x1000_000a);
    sd(m, p + 8, 0);
    sd(m, p + 0x10, 0);
    m.w32(r + 0x680, p.wrapping_add(0x20));
    for k in 0..4 {
        let s = m.r8(rec + 8 + k) as u32;
        m.w32(p + 0x20 + 4 * k, s);
    }
    view_matrix(m, view, p + 0x30);
    sd(m, p + 0x78, 0x5000_0002 << 32);
    sd(m, p + 0x70, 0x8000 << 45);
    sd(m, p + 0x80, 0x1000_0000_0000_8001);
    sd(m, p + 0x88, 0xe);
    let prog = program(m, entry, rec, 2);
    sd(m, p + 0x90, 5);
    sd(m, p + 0x98, 9);
    sd(m, p + 0xa8, 0x8000 << 45);
    m.w32(r + 0x680, p.wrapping_add(0xc0));
    sd(m, p + 0xa0, ((prog | 0x1500_0000) as u64) << 32);
    p
}

/// vf15 0x1d9f58(this, renderer): the four lists drawn (the views' first, 0x1da5c8, when +0x20 is set)
///
/// The lists are laid out at +0x56c as four of count, head, program kind (3, 2, 1, 4) and program table; after
/// the frame's matrices, each patch of the uniform three and then the mixed one is bound and sent.
pub fn draw(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, r: u32, scratch: u32) {
    let list = |m: &Mem, k: u32| (m.r32(this + [0x54c, 0x554, 0x55c, 0x564][k as usize]), m.r32(this + [0x550, 0x558, 0x560, 0x568][k as usize]));
    m.w32(this + 0x574, 3);
    let l = [0, 1, 2, 3].map(|k| list(m, k));
    for (k, (count, head)) in l.into_iter().enumerate() {
        let e = this + 0x56c + 0x10 * k as u32;
        m.w32(e + 4, head);
        m.w32(e, count);
        if k > 0 {
            m.w32(e + 8, [3, 2, 1, 4][k]);
        }
        m.w32(e + 0xc, PROGRAMS[k]);
    }
    env.call(m, MATRICES, &[r]);
    if m.r32(this + 0x20) != 0 {
        draw_views(m, env, this, r, scratch);
    }
    rv(m, env, r, 0x1c8, &[1, 0, BLEND]);
    let mut last = [0xffff; 2];
    rv(m, env, r, 0x200, &[1, 1]);
    rv(m, env, r, 0x1f8, &[0, 0]);
    rv(m, env, r, 0x1f8, &[2, 1]);
    for k in 0..4u32 {
        let e = this + 0x56c + 0x10 * k;
        if m.r32(e) == 0 {
            continue;
        }
        let kind = m.r32(e + 8);
        use_program(m, r, kind);
        let mut rec = m.r32(e + 4);
        while rec != 0 {
            bind_textures(m, env, r, rec, &mut last);
            let p = if k < 3 { uniform_packet(m, r, e, rec) } else { mixed_packet(m, r, e, rec) };
            let end = if k < 3 { 0x80 } else { 0x90 };
            env.call(m, SUBMIT, &[r, p, p.wrapping_add(end), 0]);
            rec = m.r32(rec + 0x14);
        }
    }
    finish(m, env, r);
}

/// The draws' close: no textures (the renderer's +0x1f0 with −1 in both slots), +0x1c8 (0, 2, 0.08), +0x1f8
/// (0, 1), and the texture state's program cleared
fn finish(m: &mut Mem, env: &mut dyn BezierEnv, r: u32) {
    rv(m, env, r, 0x1f0, &[u32::MAX, 0]);
    rv(m, env, r, 0x1f0, &[u32::MAX, 1]);
    rv(m, env, r, 0x1c8, &[0, 2, BLEND]);
    rv(m, env, r, 0x1f8, &[0, 1]);
    let ts = m.r32(r + 0x470);
    if m.r8(ts + 0x10) != 0 || m.r32(ts) != 0 {
        m.w32(ts, 0);
        m.w32(r + 0x474, 1);
        m.w8(ts + 0x10, 0);
    }
}

/// 0x1da5c8(this, renderer): the patches seen from the views (a reflection's or a mirror's), drawn once for the
/// main camera and then once for each view whose box test (the renderer's +0x288 on the patch's box, +0x150 and
/// +0x15c with w 1, in the view's first matrix) doesn't answer 1 (outside); a patch drawn so is taken out of its
/// list. The box is built in `scratch` (the game builds it on its stack: STANDIN).
pub fn draw_views(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, r: u32, scratch: u32) {
    if m.r32(this + 0x2c) == 0 {
        return;
    }
    let mut last = [0xffff; 2];
    rv(m, env, r, 0x1f8, &[0, 0]);
    rv(m, env, r, 0x1f8, &[2, 1]);
    env.call(m, MATRICES, &[r]);
    for k in 0..4u32 {
        let e = this + 0x56c + 0x10 * k;
        if m.r32(e) == 0 {
            continue;
        }
        let kind = m.r32(e + 8);
        use_program(m, r, kind);
        let mut prev = 0u32;
        let mut rec = m.r32(e + 4);
        while rec != 0 {
            let mut seen = [0u32; 8];
            let mut n = 0usize;
            let mut v = 0u32;
            while (v as i32) < m.r32(this + 0x2c) as i32 {
                let patch = m.r32(rec + 0x10);
                for (at, from) in [(scratch, 0x150u32), (scratch + 0x10, 0x15c)] {
                    for j in 0..3 {
                        let w = m.r32(patch.wrapping_add(from + 4 * j));
                        m.w32(at + 4 * j, w);
                    }
                    m.w32(at + 0xc, 1f32.to_bits());
                }
                let mat = this.wrapping_add(v.wrapping_mul(0xa0)).wrapping_add(0x40);
                if rv(m, env, r, 0x288, &[scratch, scratch + 0x10, mat]) != 1 {
                    seen[n] = v;
                    n += 1;
                }
                v += 1;
            }
            if n == 0 {
                prev = rec;
                rec = m.r32(rec + 0x14);
                continue;
            }
            bind_textures(m, env, r, rec, &mut last);
            let p = if k < 3 { uniform_packet(m, r, e, rec) } else { mixed_packet(m, r, e, rec) };
            rv(m, env, r, 0x200, &[1, 1]);
            env.call(m, SUBMIT, &[r, p, p.wrapping_add(if k < 3 { 0x80 } else { 0x90 }), 0]);
            rv(m, env, r, 0x200, &[2, 1]);
            for &view in &seen[..n] {
                let p = if k < 3 { view_packet(m, r, e, rec, this, view) } else { view_packet_mixed(m, r, e, rec, this, view) };
                let tex = m.r32(this.wrapping_add(view.wrapping_mul(0xa0)) + 0xc4);
                if tex != last[1] {
                    rv(m, env, r, 0x1f0, &[tex, 1]);
                    last[1] = m.r16(this.wrapping_add(view.wrapping_mul(0xa0)) + 0xc4) as u32;
                }
                env.call(m, SUBMIT, &[r, p, p.wrapping_add(if k < 3 { 0xa0 } else { 0xb0 }), 1]);
            }
            let c = m.r32(e);
            m.w32(e, c.wrapping_sub(1));
            let next = m.r32(rec + 0x14);
            if rec == m.r32(e + 4) {
                m.w32(e + 4, next);
            } else {
                m.w32(prev + 0x14, next);
            }
            rec = m.r32(rec + 0x14);
        }
    }
    finish(m, env, r);
}

// ---- The tables (row G7a3c) ----

/// The tables' name for the allocator
pub const TABLES_NAME: u32 = 0x392fc8;

/// `Render_FillBezierWeights` 0x1db530(this, &at): the cubic's powers (t³, t², t, 1) for t from 0 by 1/7 (8 of
/// them), by 1/5 (6) and by 1/3 (4), t summed; `at` moved on past them (0x120 bytes)
pub fn fill_weights(m: &mut Mem, at: u32) {
    let p0 = m.r32(at);
    let mut p = p0;
    for (step, n) in [(0x3e12_4925u32, 8), (0x3e4c_cccd, 6), (0x3eaa_aaab, 4)] {
        let mut t = F(0.0);
        for _ in 0..n {
            let t2 = t * t;
            let t3 = t2 * t;
            m.w32(p, t3.0.to_bits());
            m.w32(p + 8, t.0.to_bits());
            m.w32(p + 4, t2.0.to_bits());
            t = t + F(f32::from_bits(step));
            m.w32(p + 0xc, 1f32.to_bits());
            p = p.wrapping_add(0x10);
        }
    }
    m.w32(at, p0.wrapping_add(0x120));
}

/// A grid of (u, v, 1, 1) from `at`: `n` × `n` points `d` apart (+0x28 ÷ 128 ÷ `div`), from `start` (0, or `d`
/// for the inside of 8), each summed
fn grid(m: &mut Mem, this: u32, at: u32, n: u32, div: u32, inside: bool) {
    let d = F(f32::from_bits(m.r32(this + 0x28))) * F(1.0 / 128.0) * F(f32::from_bits(div));
    let mut p = m.r32(at);
    let mut u = if inside { d } else { F(0.0) };
    for _ in 0..n {
        let next = u + d;
        let mut v = if inside { d } else { F(0.0) };
        for _ in 0..n {
            m.w32(p, u.0.to_bits());
            m.w32(p + 4, v.0.to_bits());
            m.w32(p + 8, 1f32.to_bits());
            v = v + d;
            m.w32(p + 0xc, 1f32.to_bits());
            p = p.wrapping_add(0x10);
        }
        u = next;
    }
}

/// `Render_BuildBezierTables` 0x1db1e8(this): the old tables freed; 0xe00 bytes (filled with 0xdeadc0ed when got)
/// at +0x530, cut into the weights and the (u, v) grids of 4 (+0x534), 6 (+0x538), 8 (+0x53c) and the inside of 8
/// (+0x540), each pair from its start. The place it fills from is kept at `at` (the game's stack: STANDIN).
pub fn build_tables(m: &mut Mem, env: &mut dyn BezierEnv, this: u32, at: u32) {
    let t = m.r32(this + 0x530);
    if t != 0 {
        env.call(m, MEM_FREE, &[t]);
        m.w32(this + 0x530, 0);
    }
    let s = env.call(m, ALLOC, &[TABLES_NAME, 0xe00, 0]);
    if s != 0 {
        env.call(m, MEM_SET, &[s, 0xdead_c0ed, 0xe00]);
    }
    m.w32(this + 0x530, s);
    m.w32(this + 0x538, s.wrapping_add(0x220));
    m.w32(this + 0x534, s);
    m.w32(this + 0x53c, s.wrapping_add(0x580));
    m.w32(at, s);
    m.w32(this + 0x540, s.wrapping_add(0xaa0));
    for (k, (n, div, inside)) in [(4, 0x3eaa_aaab, false), (6, 0x3e4c_cccd, false), (8, 0x3e12_4925, false), (6, 0x3e12_4925, true)].into_iter().enumerate() {
        if k > 0 {
            let next = m.r32(this + 0x534 + 4 * k as u32);
            m.w32(at, next);
        }
        fill_weights(m, at);
        grid(m, this, at, n, div, inside);
    }
}

/// `StaticInit_Render` 0x1db640: the four lists' VU1 program tables ([`PROGRAMS`]): the six words 0xa, 0, 0x10,
/// 0xd, 5, 0x1c (the mixed list's last 0x30), the micro programs' places in 8 bytes
pub fn static_init(m: &mut Mem) {
    for (k, v) in [0xa, 0, 0x10, 0xd].into_iter().enumerate() {
        for t in PROGRAMS {
            m.w32(t + 4 * k as u32, v);
        }
    }
    for t in PROGRAMS {
        m.w32(t + 0x14, if t == PROGRAMS[3] { 0x30 } else { 0x1c });
    }
    for t in PROGRAMS {
        m.w32(t + 0x10, 5);
    }
}

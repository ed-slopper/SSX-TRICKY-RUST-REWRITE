//! The PS2 lens FX node (board row G7d2e): `cPS2LensFXObjNode` 0x1ea4d0..0x1eb578 and `LensFX_Create` 0x1ea408,
//! the subclass whose virtuals the lens FX callback (`lensfx::callback`) runs: how much of a flare shows (a read-back
//! of the depth buffer under its box) and the GS packets that draw the flares and the sun into the renderer's
//! buffer. On the bytes at their addresses ([`Mem`], the EE's and the GS's hardware registers among them); the
//! allocator, the memory fill, the DMA sends and the waits are calls out ([`Ps2LensEnv`]). Checked in the function
//! runner (`tests/ps2lensfx.rs`).
//!
//! **The packets** go to the renderer's packet buffer (+0x694), started by [`begin`] (the node's +0x3350, the
//! doublewords written at +0x3354): each part a DMA `cnt` tag, VIF `DIRECT`, then GIF tags with their registers
//! (A+D for the settings; `REGLIST` PRIM, RGBAQ, UV, XYZ2 for the sprites; RGBAQ, XYZ2 for the sun's triangles).
//! Screen positions are in the GS's 12.4 fixed point about 2048 less half the screen (+0x698 wide, +0x69c high).

use crate::animseq::div;
use crate::animseq::cvt_s_w;
use crate::pf::Mem;
use crate::ps2f::F;

pub const VTABLE: u32 = 0x393ea8;
pub const GAPP: u32 = 0x337c58;
pub const SIZE: u32 = 0x33c0;
/// The node's class name for the allocator, and the pool its flag 0x4000_0000 asks for
pub const NAME: u32 = 0x393ea0;
pub const POOL: u32 = 0x345e48;
pub const POOL_ALLOC: u32 = 0x23d3b0;
pub const ALLOC: u32 = 0x2ccf70;
pub const MEM_SET: u32 = 0x2cc808;
pub const BASE_CONSTRUCT: u32 = 0x1cbbc8;
pub const BASE_DESTRUCT: u32 = 0x1cbdb8;
pub const FLUSH_CACHE: u32 = 0x306a60;
pub const DMA_SEND_GIF: u32 = 0x2403a8;
pub const SYNC: u32 = 0x2e5650;

/// The calls out
pub trait Ps2LensEnv {
    /// A function with these words; its answer
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// A hardware register read (`lw`, or `ld` when `wide`) and write (the EE's 0x1000_xxxx, the GS's 0x1200_xxxx)
    fn io_read(&mut self, m: &mut Mem, addr: u32, wide: bool) -> u64;
    fn io_write(&mut self, m: &mut Mem, addr: u32, v: u64, wide: bool);
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}
/// A 32-bit result as the EE keeps it: sign-extended to 64 bits
fn sx(v: u32) -> u64 {
    v as i32 as i64 as u64
}
/// `cvt.w.s` then `mfc1`: toward zero, saturating, sign-extended
fn int(v: F) -> u64 {
    sx(v.ee() as i32 as u32)
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}

/// `LensFX_Create` 0x1ea408(a0, flags): 0x33c0 bytes from the pool (flag 0x4000_0000: 0x23d3b0 with the flags and
/// the class name) or the allocator (the low 26 bits of the flags), filled with 0 (flag bit 31) or 0xdeadc0de, then [`construct`] with a0 (even when
/// nothing was got)
pub fn create(m: &mut Mem, env: &mut dyn Ps2LensEnv, a0: u32, flags: u32) -> u32 {
    let p = if flags & 0x4000_0000 != 0 {
        env.call(m, POOL_ALLOC, &[POOL, SIZE, flags, NAME, 0, 0])
    } else {
        env.call(m, ALLOC, &[NAME, SIZE, flags & 0x3ff_ffff])
    };
    if p != 0 {
        let fill = if (flags as i32) < 0 { 0 } else { 0xdead_c0de };
        env.call(m, MEM_SET, &[p, fill, SIZE]);
    }
    construct(m, env, p, a0)
}

/// `cPS2LensFXObjNode::cPS2LensFXObjNode` 0x1ea4d0(this, a1): the lens FX node's (`lensfx::construct`, here a call
/// out), then its vtable
pub fn construct(m: &mut Mem, env: &mut dyn Ps2LensEnv, this: u32, a1: u32) -> u32 {
    env.call(m, BASE_CONSTRUCT, &[this, a1]);
    m.w32(this + 8, VTABLE);
    this
}

/// `cPS2LensFXObjNode::~cPS2LensFXObjNode` 0x1ea508(this, a1): its vtable, then the lens FX node's
pub fn destruct(m: &mut Mem, env: &mut dyn Ps2LensEnv, this: u32, a1: u32) {
    m.w32(this + 8, VTABLE);
    env.call(m, BASE_DESTRUCT, &[this, a1]);
}

/// vf10 0x1ea990 (+0x58)(node): the packet started at the renderer's buffer (+0x694, kept at +0x3350): a DMA
/// `cnt` of 5, `DIRECT` 4, and an A+D GIF tag of three: TEST_1 0x30000, ZBUF_1 (the renderer's +0x1710, its +0x6a4
/// less 0x30 as the format, the mask bit), ALPHA_1 0x48
pub fn begin(m: &mut Mem, node: u32) {
    let r = renderer(m);
    m.w32(node + 0x3354, 0);
    let p = m.r32(r + 0x694);
    let fmt = (sx(m.r32(r + 0x6a4).wrapping_sub(0x30)) << 32) >> 8;
    w64(m, p, 0x1000_0005);
    w64(m, p + 0x20, 0x1000_0000_0000_8003);
    w64(m, p + 0x28, 0xe);
    w64(m, p + 0x18, 0x5000_0004 << 32);
    w64(m, p + 0x30, 0x3_0000);
    w64(m, p + 0x38, 0x47);
    m.w32(node + 0x3350, p);
    w64(m, p + 8, 0);
    w64(m, p + 0x10, 0);
    let zbuf = m.r32(r + 0x1710) as u64;
    w64(m, p + 0x58, 0x42);
    m.w32(node + 0x3354, 0xc);
    w64(m, p + 0x48, 0x4e);
    w64(m, p + 0x40, zbuf | fmt | 1 << 32);
    w64(m, p + 0x50, 0x48);
}

/// vf15 0x1eaa40 (+0x80)(node): the DMA `end` tag
pub fn end(m: &mut Mem, node: u32) {
    let n = m.r32(node + 0x3354);
    let p = m.r32(node + 0x3350).wrapping_add(n.wrapping_mul(8));
    m.w32(node + 0x3354, n.wrapping_add(2));
    w64(m, p, 0x7000_0000);
    w64(m, p + 8, 0);
}

/// vf14 0x1eab88 (+0x78): nothing
pub fn end_view(_m: &mut Mem, _node: u32) {}

/// vf11 0x1eaa68 (+0x60)(node, view): the view's block (its box, camera matrix and three values) kept at +0x3360..;
/// the packet: `cnt` 3, FLUSHE, `DIRECT` 2, SCISSOR_1 to the view's box
pub fn set_view(m: &mut Mem, node: u32, view: u32) {
    for k in 0..4 {
        let w = m.r32(view + 4 * k);
        m.w32(node + 0x3360 + 4 * k, w);
    }
    for k in 0..16 {
        let w = m.r32(((view + 0x10) & !15) + 4 * k);
        m.w32(((node + 0x3370) & !15) + 4 * k, w);
    }
    for k in 0..3 {
        let w = m.r32(view + 0x50 + 4 * k);
        m.w32(node + 0x33b0 + 4 * k, w);
    }
    let (x, y, w, h) = (m.r32(view), m.r32(view + 4), m.r32(view + 8), m.r32(view + 0xc));
    let x1 = sx(x.wrapping_sub(1).wrapping_add(w)) << 16;
    let y1 = sx(y.wrapping_sub(1).wrapping_add(h)) << 48;
    let scissor = sx(x) | y1 | sx(y) << 32 | x1;
    let n = m.r32(node + 0x3354);
    let p = m.r32(node + 0x3350).wrapping_add(n.wrapping_mul(8));
    w64(m, p + 0x18, 0x5000_0002 << 32);
    w64(m, p + 0x38, 0x40);
    m.w32(node + 0x3354, n.wrapping_add(8));
    w64(m, p, 0x1000_0003);
    w64(m, p + 0x10, 1 << 60);
    w64(m, p + 0x20, 0x1000_0000_0000_8001);
    w64(m, p + 0x28, 0xe);
    w64(m, p + 0x30, scissor);
    w64(m, p + 8, 0);
}

/// An RGBAQ from three colour floats (× 128) with Q 1
fn rgbq(r: F, g: F, b: F) -> u64 {
    let k = F(128.0);
    let (r, g, b) = (int(r * k), int(g * k), int(b * k));
    (r | 0x3f80_0000u64 << 32) | (b << 16 | g << 8)
}
/// A sprite's UVs and XYZ2s from a box at `at` (x, y, w, h) and its texture box at `at` + 0x10 (u, v, w, h),
/// about the screen's offsets: (UV top left, XYZ2 top left, UV bottom right, XYZ2 bottom right)
fn sprite(m: &Mem, at: u32, ox: u32, oy: u32, z: u64) -> [u64; 4] {
    let g = |k: u32| m.r32(at + 4 * k);
    let x0 = sx(ox.wrapping_add(g(0)) << 4);
    let x1 = sx(ox.wrapping_add(g(0).wrapping_add(g(2))) << 4);
    let y0 = sx(oy.wrapping_add(g(1)) << 4);
    let y1 = sx(oy.wrapping_add(g(1).wrapping_add(g(3))) << 4);
    let u0 = sx(g(4) << 4);
    let v0 = sx(g(5) << 4);
    let u1 = sx((g(4).wrapping_add(g(6)) << 4).wrapping_sub(0x10));
    let v1 = sx((g(5).wrapping_add(g(7)) << 4).wrapping_sub(0x10));
    [u0 | v0 << 16, (x0 | y0 << 16) | z, u1 | v1 << 16, (x1 | y1 << 16) | z]
}
/// The packet's place for the next doublewords
fn here(m: &Mem, node: u32) -> u32 {
    m.r32(node + 0x3350).wrapping_add(m.r32(node + 0x3354).wrapping_mul(8))
}

/// vf12 0x1eab90 (+0x68)(node, flare): with the flare on (+0): `cnt` 15, `DIRECT` 14, then two textured, blended
/// sprites (PRIM 0x156) for its glow boxes (+0x58.., +0x78..; their texture boxes after them) and a four-corner
/// strip (PRIM 0x154) for its streak (+0x98..; texture box +0xb8..), all in its colour (+0x4c.. × 128) with alphas
/// (+0x48 × the part showing, +0x44) × the sun's three colour scales (+0x2fc4, +0x2fc8, +0x2fcc) × 128; the glows at
/// the flare's depth (+0x20), the streak at 1
pub fn draw_flare(m: &mut Mem, node: u32, e: u32) {
    if m.r32(e) == 0 {
        return;
    }
    let r = renderer(m);
    let a = rf(m, e + 0x48) * rf(m, e + 0x44);
    let k = F(128.0);
    let oy = 0x800u32.wrapping_sub(m.r32(r + 0x69c) >> 1);
    let ox = 0x800u32.wrapping_sub(m.r32(r + 0x698) >> 1);
    let base = rgbq(rf(m, e + 0x4c), rf(m, e + 0x50), rf(m, e + 0x54));
    let al1 = int((a * rf(m, node + 0x2fc4)) * k);
    let al2 = int((a * rf(m, node + 0x2fc8)) * k);
    let al3 = int((a * rf(m, node + 0x2fcc)) * k);
    let z = int(rf(m, e + 0x20)) << 32;
    let p = here(m, node);
    let s1 = sprite(m, e + 0x58, ox, oy, z);
    let s2 = sprite(m, e + 0x78, ox, oy, z);
    let gif6 = 0x6400_0000_0000_8001u64;
    w64(m, p + 0x58, s1[3]);
    w64(m, p + 0x38, base | al1 << 24);
    w64(m, p + 0x18, 0x5000_000e << 32);
    w64(m, p + 0x40, s1[0]);
    w64(m, p + 0x48, s1[1]);
    w64(m, p + 0x50, s1[2]);
    w64(m, p + 0x20, gif6);
    w64(m, p + 8, 0);
    w64(m, p + 0x10, 0);
    w64(m, p, 0x1000_000f);
    w64(m, p + 0x28, 0x53_5310);
    w64(m, p + 0x30, 0x156);
    w64(m, p + 0x60, gif6);
    w64(m, p + 0x68, 0x53_5310);
    w64(m, p + 0x70, 0x156);
    w64(m, p + 0x78, base | al2 << 24);
    w64(m, p + 0x88, s2[1]);
    w64(m, p + 0x80, s2[0]);
    w64(m, p + 0x90, s2[2]);
    w64(m, p + 0x98, s2[3]);
    // the streak's corners (floats, made whole) and its texture box
    let ci = |k: u32| int(rf(m, e + 0x98 + 4 * k)) as u32;
    let xy = |k: usize| {
        let x = sx(ox.wrapping_add(ci(2 * k as u32)) << 4);
        let y = sx(oy.wrapping_add(ci(2 * k as u32 + 1)) << 4);
        (x | y << 16) | 1 << 32
    };
    let g = |k: u32| m.r32(e + 0xb8 + 4 * k);
    let u0 = sx(g(0) << 4);
    let v0 = sx(g(1) << 4);
    let u1 = sx((g(0).wrapping_add(g(2)) << 4).wrapping_sub(0x10));
    let v1 = sx((g(1).wrapping_add(g(3)) << 4).wrapping_sub(0x10));
    let (xyz0, xyz1, xyz2, xyz3) = (xy(0), xy(1), xy(2), xy(3));
    w64(m, p + 0xf8, xyz2);
    let n = m.r32(node + 0x3354);
    m.w32(node + 0x3354, n.wrapping_add(0x20));
    w64(m, p + 0xa0, 0xa400_0000_0000_8001);
    w64(m, p + 0xa8, 0x53_5353_5310);
    w64(m, p + 0xb0, 0x154);
    w64(m, p + 0xb8, base | al3 << 24);
    w64(m, p + 0xc0, u0 | v0 << 16);
    w64(m, p + 0xc8, xyz0);
    w64(m, p + 0xd0, u1 | v0 << 16);
    w64(m, p + 0xd8, xyz1);
    w64(m, p + 0xe0, u0 | v1 << 16);
    w64(m, p + 0xe8, xyz3);
    w64(m, p + 0xf0, u1 | v1 << 16);
}

/// vf13 0x1eb0a8 (+0x70)(node, sun): with the sun on: its glow sprite (`cnt` 5, `DIRECT` 4; its box +0x48.. and
/// texture box +0x58..; colour +0x6c.. × 128, alpha the part showing × +0x2fd0 × 128; at its depth), then its
/// outline as a fan of Gouraud triangles (PRIM 0x14b, untextured; `cnt` and `DIRECT` for 3 per ray pair): each the
/// sun's centre (+0x10..) and two neighbouring outline points (+0x8c..), in its second colour (+0x7c.. × 128) with
/// alphas the part showing × +0x2fd4 × 128 × the first point's length (+0x1f4..), × each point's fade (+0x1f8..)
/// at the points, at depth 1
pub fn draw_sun(m: &mut Mem, node: u32, s: u32) {
    if m.r32(s) == 0 {
        return;
    }
    let r = renderer(m);
    let k = F(128.0);
    let oy = 0x800u32.wrapping_sub(m.r32(r + 0x69c) >> 1);
    let ox = 0x800u32.wrapping_sub(m.r32(r + 0x698) >> 1);
    let shown = rf(m, s + 0x44);
    let base = rgbq(rf(m, s + 0x6c), rf(m, s + 0x70), rf(m, s + 0x74));
    let al = int((shown * rf(m, node + 0x2fd0)) * k);
    let z = int(rf(m, s + 0x20)) << 32;
    let cnt = m.r32(node + 0x3354);
    m.w32(node + 0x3354, cnt.wrapping_add(0xc));
    let start = m.r32(node + 0x3350);
    let p = start.wrapping_add(cnt.wrapping_mul(8));
    let sp = sprite(m, s + 0x48, ox, oy, z);
    let n1 = m.r32(s + 0x88).wrapping_sub(1);
    let tris = n1.wrapping_mul(3);
    w64(m, p + 0x58, sp[3]);
    w64(m, p, 0x1000_0005);
    w64(m, p + 0x18, 0x5000_0004 << 32);
    w64(m, p + 0x20, 0x6400_0000_0000_8001);
    w64(m, p + 0x28, 0x53_5310);
    w64(m, p + 0x30, 0x156);
    w64(m, p + 0x38, base | al << 24);
    w64(m, p + 0x48, sp[1]);
    w64(m, p + 0x40, sp[0]);
    w64(m, p + 0x50, sp[2]);
    w64(m, p + 8, 0);
    w64(m, p + 0x10, 0);
    // the fan's header: a PRIM tag, then a REGLIST of RGBAQ, XYZ2 three times for each pair
    let q = cnt.wrapping_add(0xc).wrapping_mul(8).wrapping_add(start);
    w64(m, q, sx(tris.wrapping_add(4) | 0x1000_0000));
    w64(m, q + 0x30, 0x14b);
    w64(m, q + 0x40, sx(n1 | 0x8000) | 0x6400_0000_0000_0000);
    w64(m, q + 0x48, 0x51_5151);
    m.w32(node + 0x3354, cnt.wrapping_add(0x16));
    w64(m, q + 0x18, sx(tris.wrapping_add(3) | 0x5000_0000) << 32);
    w64(m, q + 0x20, 0x1400_0000_0000_8001);
    w64(m, q + 8, 0);
    w64(m, q + 0x10, 0);
    w64(m, q + 0x28, 0);
    w64(m, q + 0x38, 0);
    let a = (shown * rf(m, node + 0x2fd4)) * k;
    let base2 = {
        let (r2, g2, b2) = (int(rf(m, s + 0x7c) * k), int(rf(m, s + 0x80) * k), int(rf(m, s + 0x84) * k));
        (r2 | 0x3f80_0000u64 << 32) | (b2 << 16 | g2 << 8)
    };
    let cx = sx(ox.wrapping_add(int(rf(m, s + 0x10)) as u32) << 4);
    let cy = sx(oy.wrapping_add(int(rf(m, s + 0x14)) as u32) << 4);
    let mut j = 0u32;
    while (j as i32) < m.r32(s + 0x88).wrapping_sub(1) as i32 {
        let fade0 = rf(m, s + 0x1f8 + 8 * j);
        let fade1 = rf(m, s + 0x1f8 + 8 * (j + 1));
        let len = rf(m, s + 0x1f4 + 8 * j);
        let a0 = (a * fade0) * len;
        let a1 = (a * fade1) * len;
        let ac = a * len;
        let pt = |k: u32| {
            let x = sx(ox.wrapping_add(int(rf(m, s + 0x8c + 8 * k)) as u32) << 4);
            let y = sx(oy.wrapping_add(int(rf(m, s + 0x90 + 8 * k)) as u32) << 4);
            (x | y << 16) | z
        };
        let (p0, p1) = (pt(j), pt(j + 1));
        let n = m.r32(node + 0x3354);
        m.w32(node + 0x3354, n.wrapping_add(6));
        let t = start.wrapping_add(n.wrapping_mul(8));
        w64(m, t + 0x28, p1);
        w64(m, t, base2 | int(ac) << 24);
        w64(m, t + 8, (cx | cy << 16) | z);
        w64(m, t + 0x10, base2 | int(a0) << 24);
        w64(m, t + 0x18, p0);
        w64(m, t + 0x20, base2 | int(a1) << 24);
        j += 1;
    }
}

/// vf9 0x1ea520 (+0x50)(node, box, box2, depth): how much of a box shows, by the depth buffer.
///
/// Its two packets at +0x2fe0 (FINISH) and +0x3000 (BITBLTBUF from the renderer's depth buffer: +0x1710, the
/// frame's +0x80 and +0x8c of +0x654 + 0x94 × +0x650; TRXPOS the box's corner, TRXREG its size, TRXDIR 1: to the
/// EE); the cache flushed, the DMA channels' and VIF1's idle bits and the GS's FIFO waited for, FINISH sent and
/// waited for, the read set up (VIF1 FIFO and the GS bus turned, DMA channel 1 into +0x3050, w × 3h ÷ 16 quadwords)
/// and waited for, both turned back. Then the box's 24-bit depths: of those inside `box2`, the ones not nearer than
/// the flare (its depth made a whole number, `__fixunssfsi`) counted. With `box2` the whole box the answer is that
/// share; otherwise how far beyond half of the box it is, (n − half) ÷ half, at least 0
pub fn visibility(m: &mut Mem, env: &mut dyn Ps2LensEnv, node: u32, bx: u32, b2: u32, depth: F) -> F {
    let r = renderer(m);
    w64(m, node + 0x2fe0, 0x1000_0000_0000_8001);
    w64(m, node + 0x2ff8, 0x61);
    let frame = m.r32(r + 0x654).wrapping_add(m.r32(r + 0x650).wrapping_mul(0x94));
    w64(m, node + 0x3000, 0x1000_0000_0000_8004);
    w64(m, node + 0x3008, 0xe);
    w64(m, node + 0x2fe8, 0xe);
    w64(m, node + 0x2ff0, 0);
    let fbp = ((sx(m.r32(r + 0x1710) << 5) << 32) >> 32) & 0xffff_ffff;
    let psm = m.r32(frame + 0x8c) as u64;
    let fbw = ((sx(m.r32(frame + 0x80)) as i64 >> 6) as u64) << 16;
    let bitblt = (fbp | psm << 56) | (psm << 24 | fbw);
    let (x, y, w, h) = (m.r32(bx), m.r32(bx + 4), m.r32(bx + 8), m.r32(bx + 0xc));
    w64(m, node + 0x3010, bitblt);
    w64(m, node + 0x3018, 0x50);
    w64(m, node + 0x3020, sx(x) | sx(y) << 16);
    w64(m, node + 0x3028, 0x51);
    w64(m, node + 0x3030, sx(w) | sx(h) << 32);
    w64(m, node + 0x3038, 0x52);
    w64(m, node + 0x3040, 1);
    w64(m, node + 0x3048, 0x53);
    env.call(m, FLUSH_CACHE, &[0]);
    while env.io_read(m, 0x1000_9000, false) & 0x100 != 0 {}
    while env.io_read(m, 0x1000_a000, false) & 0x100 != 0 {}
    while env.io_read(m, 0x1000_3c00, false) & 0x1f00_0003 != 0 {}
    while env.io_read(m, 0x1200_1000, true) & 0xc000 != 0x4000 {}
    env.io_write(m, 0x1200_1000, 2, true);
    env.call(m, SYNC, &[0, 0]);
    env.call(m, DMA_SEND_GIF, &[node + 0x2fe0, 2, 6]);
    while env.io_read(m, 0x1200_1000, true) & 2 == 0 {}
    env.io_write(m, 0x1200_1000, 2, true);
    env.call(m, DMA_SEND_GIF, &[node + 0x3000, 5, 6]);
    env.io_write(m, 0x1000_3c00, 0x80_0000, false);
    env.io_write(m, 0x1200_1040, 1, true);
    env.io_write(m, 0x1000_9010, sx(node + 0x3050), false);
    let (w, h) = (m.r32(bx + 8), m.r32(bx + 0xc));
    env.io_write(m, 0x1000_9020, sx(((w.wrapping_mul(h.wrapping_mul(2).wrapping_add(h))) as i32 >> 4) as u32), false);
    env.io_write(m, 0x1000_9000, 0x100, false);
    while env.io_read(m, 0x1000_9000, false) & 0x100 != 0 {}
    env.io_write(m, 0x1000_3c00, 0, false);
    env.io_write(m, 0x1200_1040, 0, true);
    env.call(m, SYNC, &[0, 0]);
    let limit = depth.0 as u32;
    let (mut seen, mut inside) = (0i32, 0i32);
    let rows = m.r32(bx + 0xc) as i32;
    let mut yy = 0i32;
    while yy < rows {
        let cols = m.r32(bx + 8) as i32;
        let mut xx = 0i32;
        while xx < cols {
            let px = (m.r32(bx) as i32).wrapping_add(xx);
            let py = (m.r32(bx + 4) as i32).wrapping_add(yy);
            let (x2, y2) = (m.r32(b2) as i32, m.r32(b2 + 4) as i32);
            if px >= x2 && px < x2.wrapping_add(m.r32(b2 + 8) as i32) && py >= y2 && py < y2.wrapping_add(m.r32(b2 + 0xc) as i32) {
                inside += 1;
                let at = node + 0x3050 + 3 * (yy.wrapping_mul(cols).wrapping_add(xx)) as u32;
                let d = m.r8(at) as u32 | (m.r8(at + 1) as u32) << 8 | (m.r8(at + 2) as u32) << 16;
                if !(limit < d) {
                    seen += 1;
                }
            }
            xx += 1;
        }
        yy += 1;
    }
    let total = (m.r32(bx + 8) as i32).wrapping_mul(rows);
    if inside == total {
        div(cvt_s_w(seen), cvt_s_w(inside))
    } else {
        let half = total >> 1;
        let v = div(cvt_s_w(seen.wrapping_sub(half)), cvt_s_w(half));
        if 0.0 < v.ee() { v } else { F(0.0) }
    }
}

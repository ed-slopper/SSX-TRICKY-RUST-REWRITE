//! The renderer's state virtuals (board row G7k): the small `cPS2GraphicsMan` virtuals the draws call, at their
//! vtable slots (the vtable at 0x393610; slot +off's function is its word at +off + 4). On the bytes at their
//! addresses ([`Mem`]); the texture manager's functions, the matrix update, the submit and the virtuals they
//! call are calls out (`riderdraw::RiderDrawEnv`). Checked in the function runner (`tests/renderstate.rs`).
//!
//! **The texture state** (r + 0x470, 0x1c bytes; a stack of them, r + 0x46c its depth): +4 a half and +0xe a
//! byte (+0x1e8, in `particledraw`), +6 the test's reference (the float × 128) with +0x12 and +0x13 (+0x1c8),
//! +8 and +0xa the two textures (+0x1f0), +0xf a byte (+0x1d0 / +0x1d8), +0x11 (+0x1c0), +0x14 and +0x16
//! (+0x1f8), +0x15 and +0x17 the blend (+0x1e0, +0x200), +0x10 the VU1 program (`particledraw`). A change marks
//! r + 0x474 (r + 0x478 for the textures and the stack).

use crate::animseq::div;
use crate::cull;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::riderdraw::RiderDrawEnv;
use crate::sndlib::cvt_w;

pub const MATRIX_UPDATE: u32 = 0x1e85b0;
pub const LOCK_TEXTURE: u32 = 0x1c17c0;
/// The texture manager's two packet makers +0x178 and +0x188 hand on to
pub const TARGET_PACKET: u32 = 0x1c1ab8;
pub const CLEAR_PACKET: u32 = 0x1c1bd0;
pub const SUBMIT: u32 = 0x1e8838;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn rq(m: &Mem, a: u32) -> [F; 4] {
    [0, 1, 2, 3].map(|k| rf(m, a.wrapping_add(4 * k)))
}
fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}
fn ts(m: &Mem, r: u32) -> u32 {
    m.r32(r + 0x470)
}
/// The view record the renderer is on (+0xe4 its index, 0x34 bytes each from +0x14)
fn view(m: &Mem, r: u32) -> u32 {
    r.wrapping_add(m.r32(r + 0xe4).wrapping_mul(0x34)).wrapping_add(0x14)
}
fn vcall(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}

/// +0xb8 0x1dcc10(r, place, k): the depth offset for things drawn at `place` (the rider's draw: −35.02, then 0
/// back). The matrices are made (0x1e85b0). With k not 0: the place and the place moved k along the camera's
/// forward (its third column) through the clip matrix (+0x5b0), each depth z / w; when both are within −1..1 the
/// offset is +0x5f8 × (the moved one's − the place's), else 0. When the view record's (+0x20) differs, it is
/// kept, and the screen's z offsets set from it: +0x608 and +0x568 = it + 32767 (8388479.5 for the 24-bit z
/// buffer, +0x6a4 = 0x31); the frame's matrices are made again (+0x17d0).
pub fn depth_offset(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, place: u32, k: F) {
    let rec = view(m, r);
    env.call(m, MATRIX_UPDATE, &[r]);
    let zero = F(0.0);
    let mut offset = zero;
    if k != zero {
        let cam = m.r32(r + 0x3f4);
        let fwd = [rf(m, cam + 8), rf(m, cam + 0x18), rf(m, cam + 0x28), zero];
        let rows = [0, 1, 2, 3].map(|i| rq(m, r + 0x5b0 + 0x10 * i));
        let through = |p: [F; 4]| [0, 1, 2, 3].map(|c| ((rows[0][c] * p[0] + rows[1][c] * p[1]) + rows[2][c] * p[2]) + rows[3][c] * p[3]);
        let p = rq(m, place);
        let a = through(p);
        let moved = [0, 1, 2, 3].map(|j| p[j] + fwd[j] * k);
        let b = through(moved);
        let (za, zb) = (div(a[2], a[3]), div(b[2], b[3]));
        let (lo, hi) = (F(-1.0), F(1.0));
        if lo <= za && za <= hi && lo <= zb && zb <= hi {
            offset = rf(m, r + 0x5f8) * (zb - za);
        }
    }
    if offset == rf(m, rec + 0x20) {
        return;
    }
    m.w32(rec + 0x20, offset.0.to_bits());
    let base = F(f32::from_bits(if m.r32(r + 0x6a4) == 0x31 { 0x4aff_ff0c } else { 0x46ff_ff00 }));
    m.w32(r + 0x17d0, 0);
    m.w32(r + 0x608, (base + offset).0.to_bits());
    let kept = rf(m, rec + 0x20);
    m.w32(r + 0x568, (base + kept).0.to_bits());
}

/// +0xc8 0x1e8958(r): the view record's field of view (+0x30)
pub fn fov(m: &Mem, r: u32) -> F {
    rf(m, r.wrapping_add(m.r32(r + 0xe4).wrapping_mul(0x34)).wrapping_add(0x30))
}

/// +0xf8 0x1e9970(r): the camera (+0x3f4)
pub fn camera(m: &Mem, r: u32) -> u32 {
    m.r32(r + 0x3f4)
}

/// +0x170 0x1e1648(r, a, b, c): `TexMan_LockTexture` 0x1c17c0 on the texture manager (+0x47c) with the words;
/// its answer
pub fn lock_texture(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, a: u32, b: u32, c: u32) -> u32 {
    let tm = m.r32(r + 0x47c);
    env.call(m, LOCK_TEXTURE, &[tm, a, b, c])
}

/// +0x178 0x1e14b8(r, texture): the texture manager's packet for drawing into the texture (0x1c1ab8, onto the
/// packet place +0x680), then that packet submitted (from the place before to its end; the place moved on 0x10);
/// 1
pub fn draw_into(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, texture: u32) -> u32 {
    let start = m.r32(r + 0x680);
    let tm = m.r32(r + 0x47c);
    env.call(m, TARGET_PACKET, &[tm, texture, r + 0x680]);
    let end = m.r32(r + 0x680);
    m.w32(r + 0x680, end + 0x10);
    env.call(m, SUBMIT, &[r, start, end, 0]);
    1
}

/// +0x180 0x1e1510(r): a packet putting the screen back (a `cnt` of 5: FLUSHA, DIRECT 4, an A+D tag of 3): FRAME_1
/// (+0x6d0 | +0x6b0 << 16 | +0x6a0 << 24), SCISSOR_1 (from the rectangle at +0x610: x, y, w, h halves) and
/// XYOFFSET_1 ((0x800 − +0x698 / 2) × 16, (0x800 − +0x69c / 2) × 16), submitted (its end, then the place 0x10 on;
/// 1 after); 0
pub fn screen_back(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32) -> u32 {
    let p = m.r32(r + 0x680);
    let h = |m: &Mem, off: u32| m.r16(r + off) as u32;
    let frame = m.r32(r + 0x6d0) as u64 | (m.r32(r + 0x6b0) as u64) << 16 | (m.r32(r + 0x6a0) as u64) << 24;
    let right = (h(m, 0x610).wrapping_sub(1).wrapping_add(h(m, 0x614))) as i32 as i64 as u64;
    let bottom = (h(m, 0x612).wrapping_sub(1).wrapping_add(h(m, 0x616))) as i32 as i64 as u64;
    let scissor = h(m, 0x610) as u64 | bottom << 48 | (h(m, 0x612) as u64) << 32 | right << 16;
    let x = 0x800u32.wrapping_sub(m.r32(r + 0x698) >> 1) << 4;
    let y = 0x800u32.wrapping_sub(m.r32(r + 0x69c) >> 1) << 4;
    let offset = x as u64 | (y as u64) << 32;
    w64(m, p, 0x1000_0005);
    w64(m, p + 8, 0);
    w64(m, p + 0x10, 0x9800u64 << 45);
    w64(m, p + 0x18, 0x5000_0004u64 << 32);
    w64(m, p + 0x20, 0x1000_0000u64 << 32 | 0x8003);
    w64(m, p + 0x28, 0xe);
    w64(m, p + 0x30, frame);
    w64(m, p + 0x38, 0x4c);
    w64(m, p + 0x40, scissor);
    w64(m, p + 0x48, 0x40);
    w64(m, p + 0x50, offset);
    w64(m, p + 0x58, 0x18);
    m.w32(r + 0x680, p + 0x70);
    env.call(m, SUBMIT, &[r, p, p + 0x60, 1]);
    0
}

/// +0x188 0x1e1668(r, texture, colour): the texture set (+0x1f0, slot 0), the texture manager's clearing packet
/// (0x1c1bd0 with the texture and `colour`), submitted (follows: 1)
pub fn clear_texture(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, texture: u32, colour: u32) {
    let start = m.r32(r + 0x680);
    vcall(m, env, r, 0x1f0, &[texture, 0]);
    let tm = m.r32(r + 0x47c);
    env.call(m, CLEAR_PACKET, &[tm, texture, colour, r + 0x680]);
    let end = m.r32(r + 0x680);
    m.w32(r + 0x680, end + 0x10);
    env.call(m, SUBMIT, &[r, start, end, 1]);
}

/// +0x1a0 0x1e9698(r): the texture state pushed: its 0x1c bytes copied to the next place, except that place's
/// last word, which keeps what it held (the copy's last word is put back over); +0x470 moved on, +0x46c up one
pub fn push_state(m: &mut Mem, r: u32) {
    let t = ts(m, r);
    let kept = m.r32(t + 0x34);
    for k in 0..7 {
        let w = m.r32(t + 4 * k);
        m.w32(t + 0x1c + 4 * k, w);
    }
    let t = ts(m, r);
    let depth = m.r32(r + 0x46c);
    m.w32(r + 0x470, t + 0x1c);
    m.w32(r + 0x46c, depth.wrapping_add(1));
    m.w32(t + 0x34, kept);
}

/// +0x1a8 0x1e9670(r): the texture state popped (+0x470 back 0x1c, +0x46c down one), both marks set
pub fn pop_state(m: &mut Mem, r: u32) {
    let (t, depth) = (ts(m, r), m.r32(r + 0x46c));
    m.w32(r + 0x478, 1);
    m.w32(r + 0x470, t.wrapping_sub(0x1c));
    m.w32(r + 0x46c, depth.wrapping_sub(1));
    m.w32(r + 0x474, 1);
}

fn set_byte(m: &mut Mem, r: u32, off: u32, v: u32) {
    let t = ts(m, r);
    if m.r8(t + off) as u32 != v {
        m.w8(t + off, v as u8);
        m.w32(r + 0x474, 1);
    }
}

/// +0x1c0 0x1e94e8(r, v): the texture state's +0x11
pub fn set_11(m: &mut Mem, r: u32, v: u32) {
    set_byte(m, r, 0x11, v);
}

/// +0x1c8 0x1e9478(r, a, b, reference): the test's two bytes (+0x12, +0x13) and its reference (+6, the float
/// × 128 as an integer), unless all three are already so (the reference as its half's float)
pub fn set_test(m: &mut Mem, r: u32, a: u32, b: u32, reference: F) {
    let t = ts(m, r);
    if m.r8(t + 0x12) as u32 == a && m.r8(t + 0x13) as u32 == b && crate::animseq::cvt_s_w(m.r16(t + 6) as i16 as i32) == reference {
        return;
    }
    m.w32(r + 0x474, 1);
    m.w8(t + 0x12, a as u8);
    m.w8(t + 0x13, b as u8);
    m.w16(t + 6, cvt_w((reference * F(128.0)).0) as u16);
}

/// +0x1d0 0x1e9458(r, v): the texture state's +0xf
pub fn set_0f(m: &mut Mem, r: u32, v: u32) {
    set_byte(m, r, 0xf, v);
}

/// +0x1d8 0x1e9448(r): the texture state's +0xf
pub fn get_0f(m: &Mem, r: u32) -> u32 {
    m.r8(ts(m, r) + 0xf) as u32
}

/// +0x1e0 0x1e9418(r, blend): both blend bytes (+0x15, +0x17), unless both are already it
pub fn set_blend(m: &mut Mem, r: u32, blend: u32) {
    let t = ts(m, r);
    if m.r8(t + 0x15) as u32 != blend || m.r8(t + 0x17) as u32 != blend {
        m.w8(t + 0x17, blend as u8);
        m.w32(r + 0x474, 1);
        m.w8(t + 0x15, blend as u8);
    }
}

/// +0x1f0 0x1e9378 `SetTexture`(r, texture, which): the first texture (+8) or, `which` not 0, the second (+0xa),
/// marking +0x478 on a change
pub fn set_texture(m: &mut Mem, r: u32, texture: u32, which: u32) {
    let t = ts(m, r);
    let at = if which == 0 { t + 8 } else { t + 0xa };
    if m.r16(at) as i16 as i32 as u32 != texture {
        m.w16(at, texture as u16);
        m.w32(r + 0x478, 1);
    }
}

/// +0x1f8 0x1e9338(r, v, which): the texture state's +0x14 or, `which` not 0, +0x16
pub fn set_14(m: &mut Mem, r: u32, v: u32, which: u32) {
    set_byte(m, r, if which == 0 { 0x14 } else { 0x16 }, v);
}

/// +0x200 0x1e91e8(r, mode): a blend by mode −1..5 into the blend bytes (+0x15, +0x17): (3, 1), (0, 3), (3, 6),
/// (3, 7), (3, 1), (3, 9), (3, 10); unless already so (nothing for any other mode)
pub fn set_blend_mode(m: &mut Mem, r: u32, mode: u32) {
    let (b15, b17) = match mode.wrapping_add(1) {
        0 | 4 => (3u32, 1u32),
        1 => (0, 3),
        2 => (3, 6),
        3 => (3, 7),
        5 => (3, 9),
        6 => (3, 10),
        _ => return,
    };
    let t = ts(m, r);
    if m.r32(t + 0x14) & 0xff00_ff00 == (b17 << 24 | b15 << 8) {
        return;
    }
    m.w8(t + 0x17, b17 as u8);
    m.w32(r + 0x474, 1);
    m.w8(t + 0x15, b15 as u8);
}

/// +0x218 0x1e9098(r, lights): the light block's two matrices (8 quadwords) to +0x17f0
pub fn set_lights(m: &mut Mem, r: u32, lights: u32) {
    for k in 0..32 {
        let w = m.r32(lights + 4 * k);
        m.w32(r + 0x17f0 + 4 * k, w);
    }
}

/// +0x288 0x1e1e10(r, min, max, matrix): the box test on the matrix given (`cull::box_test`)
pub fn box_test(m: &Mem, min: u32, max: u32, matrix: u32) -> u32 {
    cull::box_test(m, matrix, min, max)
}

/// +0x2b8 0x1e9ef0(r, v): +0x1d90 (the rider's draw's outfit flag)
pub fn set_1d90(m: &mut Mem, r: u32, v: u32) {
    m.w32(r + 0x1d90, v);
}

/// +0x2d8 0x1e8fd8(r, a, b, c, d): the Bezier manager's (+0x484) own +0x90 (its vtable at its +0) with the words;
/// its answer
pub fn bezier_forward(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, args: &[u32]) -> u32 {
    let bz = m.r32(r + 0x484);
    let vt = m.r32(bz);
    let this = bz.wrapping_add(m.r16(vt + 0x90) as i16 as i32 as u32);
    let f = m.r32(vt + 0x94);
    env.vcall(m, f, this, args)
}

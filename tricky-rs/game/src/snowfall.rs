//! The falling snow (board row G7c1): `cSnowFallMan` 0x1c8db8..0x1c9ae8. Eight layers of flakes (0x90 bytes each
//! from +0x10) drawn in cubic cells around the camera, the cells found by a recursive walk that culls whole octants
//! against the view. On the bytes at their addresses ([`Mem`]); the base object's constructor, `Rand_Fx` and the
//! renderer (gApp+0x724, its vtable at +0x10) are calls out ([`SnowEnv`]). Checked in the function runner
//! (`tests/snowfall.rs`).
//!
//! **The manager's fields:**
//! - +8 the vtable (0x3915b8), +0x10 the eight layers;
//! - +0x490 the grid's origin, +0x4a0 the fall velocity (a frame's step is 1/60 of it);
//! - +0x4b0 the flakes' alpha, +0x4b4 the density, +0x4b8 the cell size;
//! - +0x4bc / +0x4c0 the fall range's two ends.
//!
//! **A layer:**
//! - +0 its flake count (density × size³ / 10⁶);
//! - +4 / +8 2a − b and b − a from the range;
//! - +0x10..+0x1c four random factors in [1, 2);
//! - +0x20 the cell's corner;
//! - +0x30 / +0x40 / +0x50 the cell's axes (size on x, y, z);
//! - +0x60 the colour (128, 128, 128, alpha × 128);
//! - +0x70 zero, +0x80 (1, 1, 0, 0).

use crate::animseq::{cvt_s_w, div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

/// The manager's vtable, gApp (0x337c58), the 1/60 step (0x3c888889) and the draw's fog value (0x3da3d70a)
pub const VTABLE: u32 = 0x3915b8;
pub const GAPP: u32 = 0x337c58;

/// The calls out
pub trait SnowEnv {
    /// The base object's constructor 0x17cf50(this, a1)
    fn base_construct(&mut self, m: &mut Mem, this: u32, a1: u32);
    /// `Rand_Fx` 0x23d4a8
    fn rand_fx(&mut self) -> u32;
    /// A renderer virtual (its address read from the vtable) with these words (floats as bits)
    fn renderer(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// The renderer's box test (+0x280) on the box's two corners: 0 inside, 1 outside, else partly
    fn cull_box(&mut self, m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32;
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
/// A layer
fn layer(this: u32, k: u32) -> u32 {
    this + 0x10 + 0x90 * k
}

/// The renderer object and one of its virtuals (adjusted this, function)
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}
fn vfn(m: &Mem, obj: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + 0x10);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn rcall(m: &mut Mem, env: &mut dyn SnowEnv, off: u32, args: &[u32]) -> u32 {
    let r = renderer(m);
    let (this, f) = vfn(m, r, off);
    env.renderer(m, f, this, args)
}

/// The flakes a layer has: density × size³ / 10⁶, toward zero
fn count(m: &Mem, this: u32) -> i32 {
    let size = rf(m, this + 0x4b8);
    let per = div(size * size * size, F(1_000_000.0));
    int(rf(m, this + 0x4b4) * per)
}

/// The manager's constructor 0x1c8db8(this, a1): the base object, the vtable; origin (0, 0, 0, 1), velocity (0, 0,
/// −300, 0); alpha 0.1875, density 0.5, size 350, range 2 to 3; the layers made
pub fn construct(m: &mut Mem, env: &mut dyn SnowEnv, this: u32, a1: u32) -> u32 {
    env.base_construct(m, this, a1);
    m.w32(this + 8, VTABLE);
    wq(m, this + 0x490, [F(0.0), F(0.0), F(0.0), F(1.0)]);
    wq(m, this + 0x4a0, [F(0.0), F(0.0), F(-300.0), F(0.0)]);
    wf(m, this + 0x4b4, F(0.5));
    wf(m, this + 0x4b0, F(0.1875));
    wf(m, this + 0x4b8, F(350.0));
    wf(m, this + 0x4bc, F(2.0));
    wf(m, this + 0x4c0, F(3.0));
    init_layers(m, env, this);
    this
}

/// A `Rand_Fx` draw as a float in [1, 2), through (x − 1) + 1
fn one_to_two(env: &mut dyn SnowEnv) -> F {
    let x = F(f32::from_bits(env.rand_fx() & 0x7f_ffff | 0x3f80_0000));
    (x - F(1.0)) + F(1.0)
}

/// `SnowFall_InitLayers` 0x1c8ec8: each of the eight layers its count, its range words, four random factors (drawn
/// into +0x10, +0x18, +0x1c, +0x14), its axes, its colour and the fixed rows
pub fn init_layers(m: &mut Mem, env: &mut dyn SnowEnv, this: u32) {
    let n = count(m, this);
    for k in 0..8 {
        let l = layer(this, k);
        let (a, b) = (rf(m, this + 0x4bc), rf(m, this + 0x4c0));
        m.w32(l, n as u32);
        wf(m, l + 4, a + a - b);
        let (a, b) = (rf(m, this + 0x4bc), rf(m, this + 0x4c0));
        wf(m, l + 8, b - a);
        for o in [0x10, 0x18, 0x1c, 0x14] {
            let v = one_to_two(env);
            wf(m, l + o, v);
        }
        let size = rf(m, this + 0x4b8);
        wq(m, l + 0x30, [size, F(0.0), F(0.0), F(0.0)]);
        let size = rf(m, this + 0x4b8);
        wq(m, l + 0x40, [F(0.0), size, F(0.0), F(0.0)]);
        let size = rf(m, this + 0x4b8);
        wq(m, l + 0x50, [F(0.0), F(0.0), size, F(0.0)]);
        let alpha = rf(m, this + 0x4b0) * F(128.0);
        wq(m, l + 0x60, [F(128.0), F(128.0), F(128.0), alpha]);
        wq(m, l + 0x70, [F(0.0); 4]);
        wq(m, l + 0x80, [F(1.0), F(1.0), F(0.0), F(0.0)]);
    }
}

/// 0x1c90c8(alpha): +0x4b0
pub fn set_alpha(m: &mut Mem, this: u32, v: F) {
    wf(m, this + 0x4b0, v);
}

/// `SnowFall_SetDensity` 0x1c90d0(density): the density and every layer's count
pub fn set_density(m: &mut Mem, this: u32, d: F) {
    wf(m, this + 0x4b4, d);
    let size = rf(m, this + 0x4b8);
    let n = int(d * div(size * size * size, F(1_000_000.0)));
    for k in 0..8 {
        m.w32(layer(this, k), n as u32);
    }
}

/// `SnowFall_SetScale` 0x1c9130(size): the cell size and every layer's axes; the origin snapped
pub fn set_scale(m: &mut Mem, this: u32, size: F) {
    wf(m, this + 0x4b8, size);
    for k in 0..8 {
        let l = layer(this, k);
        let s = rf(m, this + 0x4b8);
        wq(m, l + 0x30, [s, F(0.0), F(0.0), F(0.0)]);
        let s = rf(m, this + 0x4b8);
        wq(m, l + 0x40, [F(0.0), s, F(0.0), F(0.0)]);
        let s = rf(m, this + 0x4b8);
        wq(m, l + 0x50, [F(0.0), F(0.0), s, F(0.0)]);
    }
    snap_origin(m, this);
}

/// `SnowFall_SetRange` 0x1c91c0(a, b): the range and every layer's two range words
pub fn set_range(m: &mut Mem, this: u32, a: F, b: F) {
    wf(m, this + 0x4bc, a);
    wf(m, this + 0x4c0, b);
    for k in 0..8 {
        let l = layer(this, k);
        let (a, b) = (rf(m, this + 0x4bc), rf(m, this + 0x4c0));
        wf(m, l + 4, a + a - b);
        let (a, b) = (rf(m, this + 0x4bc), rf(m, this + 0x4c0));
        wf(m, l + 8, b - a);
    }
}

/// 0x1c9208(v): the fall velocity
pub fn set_velocity(m: &mut Mem, this: u32, v: u32) {
    for k in 0..4 {
        m.w32(this + 0x4a0 + 4 * k, m.r32(v + 4 * k));
    }
}

/// `SnowFall_SnapOrigin` 0x1c9218: the origin less its whole multiples of twice the cell size (each axis toward zero)
pub fn snap_origin(m: &mut Mem, this: u32) {
    let d = rf(m, this + 0x4b8) + rf(m, this + 0x4b8);
    let inv = div(F(1.0), d);
    let o = rq(m, this + 0x490);
    let nz = cvt_s_w(int(o[2] * inv)) * d;
    let nx = cvt_s_w(int(o[0] * inv)) * d;
    let ny = cvt_s_w(int(o[1] * inv)) * d;
    wq(m, this + 0x490, [o[0] - nx, o[1] - ny, o[2] - nz, o[3] - F(0.0)]);
}

/// The manager's per-frame step (vf1, 0x1c92b8): the origin moved by a 60th of the velocity, then snapped
pub fn step(m: &mut Mem, this: u32) {
    let k = F(f32::from_bits(0x3c88_8889));
    let v = rq(m, this + 0x4a0);
    let o = rq(m, this + 0x490);
    wq(m, this + 0x490, [0, 1, 2, 3].map(|i| o[i] + v[i] * k));
    snap_origin(m, this);
}

/// 0x1c9310(at, layer): a layer with flakes put at the cell around `at` (its corner `at` − the size on each axis) and
/// drawn (the renderer's +0x260 with the layer)
pub fn draw_layer(m: &mut Mem, env: &mut dyn SnowEnv, this: u32, at: [F; 4], l: u32) {
    if m.r32(l) == 0 {
        return;
    }
    let s = rf(m, this + 0x4b8);
    wq(m, l + 0x20, [at[0] - s, at[1] - s, at[2] - s, at[3] - F(0.0)]);
    let r = renderer(m);
    let (rt, f) = vfn(m, r, 0x260);
    env.renderer(m, f, rt, &[l, r]);
}

/// The camera's position (gApp+0x730's camera +0x290: its record at +0x90 + 0x80 × camera, +0x20)
fn camera(m: &Mem) -> [F; 4] {
    let g = m.r32(m.r32(GAPP) + 0x730);
    rq(m, g + 0x90 + 0x80 * m.r32(g + 0x290) + 0x20)
}

/// A remainder by 2 made non-negative
fn half_mod(v: i32) -> u32 {
    let r = v % 2;
    (if r < 0 { r + 2 } else { r }) as u32
}

/// 0x1c9388(at, ix, iy, iz, level, inside): the cells under one octant. At level 4 one cell: the layer of the cell's
/// parity (x 0x240, y 0x120, z 0x90 bytes apart) drawn at the cell when the camera is within 7 sizes, fading out
/// over the last size. Above it, the eight children (half the extent, the bits of the child's number −x, −y, −z),
/// each tested against the view unless the parent was inside (outside: skipped; inside: no more tests), then walked
pub fn walk(m: &mut Mem, env: &mut dyn SnowEnv, this: u32, at: [F; 4], ix: i32, iy: i32, iz: i32, level: i32, inside: u32) {
    if level == 4 {
        let size = rf(m, this + 0x4b8);
        let half = size * F(0.5);
        let corner = [at[0] - half, at[1] - half, at[2] - half, at[3] - F(0.0)];
        let far = size * F(7.0);
        let cam = camera(m);
        let d = [0, 1, 2, 3].map(|i| corner[i] - cam[i]);
        let sq = [0, 1, 2, 3].map(|i| d[i] * d[i]);
        let dist = sqrt(((sq[0] + sq[1]) + F(1.0) * sq[2]) + F(1.0) * sq[3]);
        let l = this + 0x10 + 0x240 * half_mod(ix) + 0x120 * half_mod(iy) + 0x90 * half_mod(iz);
        if !(dist.ee() < far.ee()) {
            return;
        }
        let mut alpha = rf(m, this + 0x4b0);
        if (far - size).ee() < dist.ee() {
            alpha = alpha * div(far - dist, size);
        }
        wq(m, l + 0x60, [F(128.0), F(128.0), F(128.0), alpha * F(128.0)]);
        draw_layer(m, env, this, corner, l);
        return;
    }
    let size = 1i32 << ((3 - level) & 31);
    let up = size >> 1;
    let down = -((size + 1) >> 1);
    let ext = rf(m, this + 0x4b8) * F(0.5) * cvt_s_w(size);
    for c in 0..8u32 {
        let pick = |bit: u32, v: F| if c & bit != 0 { v - ext } else { v + ext };
        let child = [pick(1, at[0]), pick(2, at[1]), pick(4, at[2]), at[3]];
        let mut inner = inside;
        if inside == 0 {
            let min = [0, 1, 2, 3].map(|i| (child[i] - if i < 3 { ext } else { F(0.0) }).0.to_bits());
            let max = [0, 1, 2, 3].map(|i| (child[i] + if i < 3 { ext } else { F(0.0) }).0.to_bits());
            let r = renderer(m);
            let (rt, f) = vfn(m, r, 0x280);
            let v = env.cull_box(m, f, rt, min, max);
            if v == 1 {
                continue;
            }
            inner = if v != 0 { 0 } else { 1 };
        }
        let ox = if c & 1 != 0 { down } else { up };
        let oy = if c & 2 != 0 { down } else { up };
        let oz = if c & 4 != 0 { down } else { up };
        walk(m, env, this, child, ix.wrapping_add(ox), iy.wrapping_add(oy), iz.wrapping_add(oz), level + 1, inner);
    }
}

/// The manager's draw (vf2, 0x1c9740): when it snows (the game's +0x74), the density taken from the camera's weather
/// (+0xa4's +0x18) when it changed; the camera's cell (its offset from the origin over the size, rounded half up);
/// the renderer set up (+0x1c0 1, +0x1c8 (0, 0, 0.08), +0x1e0 5, +0x1f0 (gApp+0x30's +0x4c, 0)); the walk from that
/// cell's centre; the renderer put back (+0x1e0 3, +0x1c8 (0, 2, 0.08), +0x1c0 0)
pub fn draw(m: &mut Mem, env: &mut dyn SnowEnv, this: u32) {
    let app = m.r32(GAPP);
    let g = m.r32(app + 0x730);
    if m.r32(g + 0x74) == 0 {
        return;
    }
    let weather = m.r32(g.wrapping_add(m.r32(g + 0x290) << 7) + 0xa4);
    let d = rf(m, weather + 0x18);
    if d.ee() != rf(m, this + 0x4b4).ee() {
        set_density(m, this, d);
    }
    let size = rf(m, this + 0x4b8);
    let cam = camera(m);
    let o = rq(m, this + 0x490);
    let rel = [0, 1, 2, 3].map(|i| cam[i] - o[i]);
    let cell = |v: F| {
        let k = div(v, size) + F(0.5);
        let mut n = cvt_s_w(int(k));
        if k.ee() < n.ee() {
            n = n - F(1.0);
        }
        int(n)
    };
    let (ix, iy, iz) = (cell(rel[0]), cell(rel[1]), cell(rel[2]));
    let fog = f32::from_bits(0x3da3_d70a).to_bits();
    rcall(m, env, 0x1c0, &[1]);
    rcall(m, env, 0x1c8, &[0, 0, fog]);
    rcall(m, env, 0x1e0, &[5]);
    let a1 = m.r32(m.r32(app + 0x30) + 0x4c);
    rcall(m, env, 0x1f0, &[a1, 0]);
    let sz = rf(m, this + 0x4b8);
    let off = [cvt_s_w(ix) * sz, cvt_s_w(iy) * sz, cvt_s_w(iz) * sz, F(0.0) * sz];
    let o = rq(m, this + 0x490);
    let centre = [0, 1, 2, 3].map(|i| o[i] + off[i]);
    walk(m, env, this, centre, ix, iy, iz, 0, 0);
    rcall(m, env, 0x1e0, &[3]);
    rcall(m, env, 0x1c8, &[0, 2, fog]);
    rcall(m, env, 0x1c0, &[0]);
}

/// `StaticInit_SnowFallMan` 0x1c9a18: three colour rows at 0x3d0da0 / 0x3d0db0 / 0x3d0dc0 (1 and two fractions each)
pub fn static_init(m: &mut Mem) {
    let w = |m: &mut Mem, a: u32, v: u32| m.w32(a, v);
    w(m, 0x3d0dc0 + 8, 0x3b03_126f);
    w(m, 0x3d0dc0, 1f32.to_bits());
    w(m, 0x3d0da0, 1f32.to_bits());
    w(m, 0x3d0dc0 + 4, 0x3d9b_3d08);
    w(m, 0x3d0db0, 1f32.to_bits());
    w(m, 0x3d0db0 + 4, 0x3ef2_fec5);
    w(m, 0x3d0db0 + 8, 0);
    w(m, 0x3d0da0 + 4, 0x3f76_e979);
    w(m, 0x3d0da0 + 8, 0);
}

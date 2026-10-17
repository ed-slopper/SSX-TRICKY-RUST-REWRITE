//! The lens flares' node (board row G7d2a): `cLensFXObjNode` 0x1cbbc8..0x1cc088, `LensFX_AddFlare` 0x1cc088 and
//! the world's `LensFX_Draw` 0x2006e8 that offers it the course's flaring lights. On the bytes at their addresses
//! ([`Mem`]); the base object, the renderer's virtuals and the node's own drawing virtuals (the PS2 node's, row
//! G7d2e) are calls out ([`LensEnv`]). The sun's part (0x1cd040, row G7d2b) is [`sun_part`], the flares' (0x1cc190,
//! G7d2c) [`flares_part`], the renderer's callback that draws them (0x1cdc78, G7d2d) [`callback`]. Checked in the function runner
//! (`tests/lensfx.rs`).
//!
//! **The node** (0x2fdc bytes): +0xc the frame's half (0 / 1, flipped on view 0), +0x10 the half ready to draw,
//! +0x14 the view being drawn, +0x18 / +0x1c the renderer's answers for the flare texture (+0x2fc0), +0x20 the views
//! of each half; +0x30 + 0xc0 half + 0x60 view a view's block (+0x30 the renderer's view quad, +0x40 the camera
//! matrix, +0x80..+0x88 three view values); +0x1b0 + 8 half + 4 view the flares kept, and their entries at +0x1c0 +
//! 0x1040 half + 0x820 view + 0xd0 k (+0 1, +0xc8 the depth, +0xcc the light); +0x2240.. the sun's (G7d2b);
//! +0x2fc4.. the sun's colours × +0x2fd8.

use crate::animcurve::sin_cos;
use crate::animseq::{cvt_s_w, div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

pub const VTABLE: u32 = 0x391bd8;
pub const GAPP: u32 = 0x337c58;
pub const WORLD: u32 = 0x346488;
/// The renderer's callback the node registers (row G7d2d)
pub const CALLBACK: u32 = 0x1cdc78;
pub const BASE_CONSTRUCT: u32 = 0x17cf50;
pub const BASE_DESTRUCT: u32 = 0x17cfe0;

/// The calls out
pub trait LensEnv {
    /// A function with these words (the base object, the node's two parts)
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]);
    /// A renderer virtual with these words
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// The renderer's +0xc0: a quad it returns (through a pointer)
    fn view_quad(&mut self, m: &mut Mem, func: u32, this: u32) -> [u32; 4];
    /// The renderer's +0xd0: three words it writes through its three pointers
    fn view_values(&mut self, m: &mut Mem, func: u32, this: u32) -> [u32; 3];
    /// The renderer's +0x270 on a point (a quad): not 0 when it is not to be flared
    fn hidden(&mut self, m: &mut Mem, func: u32, this: u32, p: [u32; 4]) -> u32;
    /// The renderer's +0x278: a point (a quad) projected to the screen (a quad, through a pointer)
    fn project(&mut self, m: &mut Mem, func: u32, this: u32, p: [u32; 4]) -> [u32; 4];
    /// The node's own virtual (the PS2 node's drawing, row G7d2e) with these words and f12; its float answer's bits
    fn node_vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32], f12: u32) -> u32;
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn vfn(m: &Mem, obj: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + 0x10);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}
fn game(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x730)
}

/// `cLensFXObjNode::cLensFXObjNode` 0x1cbbc8(this, a1): the base object, the vtable; no half ready or drawn
/// (+0xc, +0x10 −1), no flares or views; the renderer's callback (+0x318 with 0x1cdc78 and the node); the colour
/// scale 1 (+0x2fd8) and the flare texture (gApp+0x30's +0x70)
pub fn construct(m: &mut Mem, env: &mut dyn LensEnv, this: u32, a1: u32) -> u32 {
    env.call(m, BASE_CONSTRUCT, &[this, a1]);
    m.w32(this + 8, VTABLE);
    m.w32(this + 0x10, u32::MAX);
    m.w32(this + 0xc, u32::MAX);
    for k in 0..4 {
        m.w32(this + 0x1b0 + 4 * k, 0);
    }
    m.w32(this + 0x24, 0);
    m.w32(this + 0x20, 0);
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x318);
    env.vcall(m, f, t, &[CALLBACK, this]);
    m.w32(this + 0x2fd8, 1f32.to_bits());
    let tex = m.r32(m.r32(m.r32(GAPP) + 0x30) + 0x70);
    m.w32(this + 0x2fc0, tex);
    this
}

/// `cLensFXObjNode::~cLensFXObjNode` 0x1cbdb8(this, a1): the vtable, the callback taken away (+0x318 with 0, 0),
/// the base's
pub fn destruct(m: &mut Mem, env: &mut dyn LensEnv, this: u32, a1: u32) {
    m.w32(this + 8, VTABLE);
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x318);
    env.vcall(m, f, t, &[0, 0]);
    env.call(m, BASE_DESTRUCT, &[this, a1]);
}

/// `LensFX_AddFlare` 0x1cc088(node, light, depth): into the view's ten; when full, in place of the farthest (the
/// last of the deepest; −1 with none beyond 0) if this one is nearer
pub fn add_flare(m: &mut Mem, node: u32, light: u32, depth: F) {
    let view = m.r32(node + 0x14);
    let half = m.r32(node + 0xc);
    let at = node.wrapping_add(0x1b0).wrapping_add(view.wrapping_mul(4).wrapping_add(half.wrapping_mul(8)));
    let n = m.r32(at) as i32;
    let entries = node.wrapping_add(half.wrapping_mul(0x1040)).wrapping_add(0x1c0).wrapping_add(view.wrapping_mul(0x820));
    let slot = if n < 10 {
        m.w32(at, (n + 1) as u32);
        n
    } else {
        let (mut far, mut k) = (F(0.0), -1i32);
        for i in 0..n {
            let d = rf(m, entries.wrapping_add((0xd0 * i) as u32) + 0xc8);
            if far.ee() < d.ee() {
                far = d;
                k = i;
            }
        }
        if !(depth.ee() < far.ee()) {
            return;
        }
        k
    };
    let e = entries.wrapping_add((0xd0 * slot) as u32);
    m.w32(e + 0xcc, light);
    m.w32(e, 1);
    m.w32(e + 0xc8, depth.0.to_bits());
}

/// `LensFX_Draw` 0x2006e8(world, node): each live cell (in order, to the first empty one) and each of its slots its
/// mask names: each light with a flare flag (+4 & 0x70) not hidden (the renderer's +0x270 on its position) whose
/// depth in the camera's frame (+0xf8's rows; the third lane) is below the sun's limit (+0x5c) offered to
/// [`add_flare`]
pub fn lens_draw(m: &mut Mem, env: &mut dyn LensEnv, world: u32, node: u32) {
    let sun = m.r32(game(m) + 0x2c);
    let r = renderer(m);
    let mut c = 0u32;
    while (c as i32) < 0xa2 {
        let cell = m.r32(world + 0x3c + 4 * c);
        c += 1;
        if cell == 0 {
            return;
        }
        if m.r16(cell + 0x36) as i16 == 0 {
            continue;
        }
        let mut k = 0u32;
        while (k as i32) < m.r32(m.r32(world) + 0x48) as i32 {
            let mask = m.r16(cell + 0x36) as i16 as i32;
            let rec = m.r32(cell + 0x38).wrapping_add(0x4c * k);
            k += 1;
            if (mask >> ((k - 1) & 31)) & 1 == 0 {
                continue;
            }
            let n = m.r16(rec + 0x2c) as i16 as i32;
            for i in 0..n.max(0) as u32 {
                let light = m.r32(m.r32(rec + 0x40) + 4 * i);
                if m.r32(light + 4) & 0x70 == 0 {
                    continue;
                }
                let p = [m.r32(light + 0x28), m.r32(light + 0x2c), m.r32(light + 0x30), 1f32.to_bits()];
                let (t, f) = vfn(m, r, 0x270);
                if env.hidden(m, f, t, p) != 0 {
                    continue;
                }
                let (t, f) = vfn(m, r, 0xf8);
                let mat = env.vcall(m, f, t, &[]) & !15;
                let pf = p.map(|w| F(f32::from_bits(w)));
                let row = |j: u32| [0, 1, 2, 3].map(|q| rf(m, mat + 0x10 * j + 4 * q));
                let rows = [row(0), row(1), row(2), row(3)];
                let z = ((rows[0][2] * pf[0] + rows[1][2] * pf[1]) + rows[2][2] * pf[2]) + rows[3][2] * pf[3];
                if z.ee() < rf(m, sun + 0x5c).ee() {
                    add_flare(m, node, light, z);
                }
            }
        }
    }
}

/// `cLensFXObjNode::Draw` 0x1cbe20(this): for the view the game is drawing (+0x290; view 0 flips the half): the
/// half's view count (+0x298), the view's block (the renderer's +0xc0 quad, +0xf8's matrix, +0xd0's three values),
/// no flares yet, the sun's colours (its +0x3c, +0x40, +0x44, +0x24, +0x10 × +0x2fd8), the texture's two answers
/// (+0x190, +0x198); the world's flares (`LensFX_Draw`), the sun's part, the flares' part; after the half's last
/// view, that half ready (+0x10)
pub fn draw(m: &mut Mem, env: &mut dyn LensEnv, this: u32) {
    let g = game(m);
    let view = m.r32(g + 0x290);
    m.w32(this + 0x14, view);
    if view == 0 {
        let h = (m.r32(this + 0xc).wrapping_add(1)) & 1;
        m.w32(this + 0xc, h);
    }
    let r = renderer(m);
    let half = m.r32(this + 0xc);
    let views = m.r32(g + 0x298);
    m.w32((this + 0x20).wrapping_add(half.wrapping_mul(4)), views);
    let block = |m: &Mem| {
        this.wrapping_add(m.r32(this + 0xc).wrapping_mul(0xc0)).wrapping_add(m.r32(this + 0x14).wrapping_mul(0x60))
    };
    // (each block's address taken before its call)
    let b = block(m);
    let (t, f) = vfn(m, r, 0xc0);
    let q = env.view_quad(m, f, t);
    for k in 0..4 {
        m.w32(b + 0x30 + 4 * k, q[k as usize]);
    }
    let b = block(m);
    let (t, f) = vfn(m, r, 0xf8);
    let mat = env.vcall(m, f, t, &[]) & !15;
    for k in 0..16 {
        let w = m.r32(mat + 4 * k);
        m.w32(((b + 0x40) & !15) + 4 * k, w);
    }
    let b = block(m);
    let (t, f) = vfn(m, r, 0xd0);
    let v = env.view_values(m, f, t);
    for k in 0..3 {
        m.w32(b + 0x80 + 4 * k, v[k as usize]);
    }
    let scale = rf(m, this + 0x2fd8);
    let sun = m.r32(game(m) + 0x2c);
    let at = this.wrapping_add(m.r32(this + 0x14).wrapping_mul(4)).wrapping_add(m.r32(this + 0xc).wrapping_mul(8));
    m.w32(at.wrapping_add(0x1b0), 0);
    for (k, o) in [0x3c, 0x40, 0x44, 0x24, 0x10].into_iter().enumerate() {
        let v = scale * rf(m, sun + o);
        m.w32(this + 0x2fc4 + 4 * k as u32, v.0.to_bits());
    }
    let tex = m.r32(this + 0x2fc0);
    let (t, f) = vfn(m, r, 0x190);
    let a = env.vcall(m, f, t, &[tex]);
    m.w32(this + 0x18, a);
    let tex = m.r32(this + 0x2fc0);
    let (t, f) = vfn(m, r, 0x198);
    let a = env.vcall(m, f, t, &[tex]);
    m.w32(this + 0x1c, a);
    lens_draw(m, env, WORLD, this);
    sun_part(m, env, this);
    // (STANDIN: the two size registers as `Draw`'s caller leaves them, taken as 0; they matter only for a flare of
    // another kind first)
    flares_part(m, env, this, [F(0.0); 2]);
    let half = m.r32(this + 0xc);
    let views = m.r32((this + 0x20).wrapping_add(half.wrapping_mul(4)));
    if m.r32(this + 0x14) == views.wrapping_sub(1) {
        m.w32(this + 0x10, half);
    }
}

/// The sun's angles from the screen: 37 (angle, length) pairs at 0x391cf8 (the word before them, 0x391cf4, is read
/// for a corner put first)
pub const SUN_RAYS: u32 = 0x391cf8;

fn rq(m: &Mem, a: u32) -> [F; 4] {
    let a = a & !15;
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn bits(q: [F; 4]) -> [u32; 4] {
    q.map(|x| x.0.to_bits())
}
fn int(v: F) -> u32 {
    v.ee() as i32 as u32
}
/// The matrix's rows (each word as `lwc1` reads it) times a vector: ((v.x m[i][0] + v.y m[i][1]) + v.z m[i][2]) +
/// v.w m[i][3]
fn rows_times(m: &Mem, at: u32, v: [F; 4]) -> [F; 4] {
    let e = |i: u32, j: u32| rf(m, at + 0x10 * i + 4 * j);
    [0, 1, 2, 3].map(|i| ((v[0] * e(i, 0) + v[1] * e(i, 1)) + v[2] * e(i, 2)) + v[3] * e(i, 3))
}
/// The same for (1, z, z, z), its x folded away: ((m[i][0] + m[i][1] z) + m[i][2] z) + m[i][3] z
fn rows_times_x(m: &Mem, at: u32, z: F) -> [F; 4] {
    let e = |i: u32, j: u32| rf(m, at + 0x10 * i + 4 * j);
    [0, 1, 2, 3].map(|i| ((e(i, 0) + e(i, 1) * z) + e(i, 2) * z) + e(i, 3) * z)
}

/// The sun's flare, 0x1cd040(node): its block (+0x2240 + 0x6c0 half + 0x360 view) for the renderer's callback.
///
/// With the course's sun off (its +0): none (block +0 is 0). Otherwise the sun is placed from its two angles (+0x28,
/// +0x2c, in degrees: (cos a cos b, sin a cos b, sin b)) at its distance from the camera's origin in its frame. The
/// distance is +0x30, or the view's +0x88 − 1 when that is nearer, its size (+0x14) shrunk in proportion. Hidden
/// (+0x270): none. Otherwise its block holds:
/// - +0x10 the projected point (+0x278); +0x24.. the sprite box from it (16 × 16, 8 back) and +0x34.. that box
///   clamped to the view (+0x30.. of the view's block); +0x20 its depth.
/// - +0x48.. the glow's box from the projections of the sun moved by its size right (the camera's x row) and down:
///   twice the distance across, half back. +0x58.. halves of the texture's answers.
/// - the colours: +0x84, +0x6c.., +0x7c.. the sun's +0xc, +0x18.., +4.. over 255; +0x68 and +0x78 1.
/// - +0x88 a count and +0x8c.. the outline: rays from the sun in the table's 37 directions, the screen's four
///   corners put in among them by angle (`Math_Atan`), each to the screen's edge (in −1..1). Each gives a point,
///   its length (+0x1f4..) and a fade, 1 − (its length² ÷ 3, at most 1) (+0x1f8..). The first point is repeated at
///   the end.
pub fn sun_part(m: &mut Mem, env: &mut dyn LensEnv, node: u32) {
    let (one, zero) = (F(1.0), F(0.0));
    let half = m.r32(node + 0xc);
    let view = m.r32(node + 0x14);
    let sun = m.r32(game(m) + 0x2c);
    let s = node.wrapping_add(half.wrapping_mul(0x6c0)).wrapping_add(0x2240).wrapping_add(view.wrapping_mul(0x360));
    if m.r32(sun) == 0 {
        m.w32(s, 0);
        return;
    }
    let block = |m: &Mem| {
        node.wrapping_add(m.r32(node + 0x14).wrapping_mul(0x60)).wrapping_add(m.r32(node + 0xc).wrapping_mul(0xc0))
    };
    let b = node.wrapping_add(view.wrapping_mul(0x60)).wrapping_add(half.wrapping_mul(0xc0));
    let lim = rf(m, sun + 0x30);
    let near = rf(m, b + 0x88);
    let (dist, size) = if near.ee() < lim.ee() {
        let d = near - one;
        (d, rf(m, sun + 0x14) * div(d, lim))
    } else {
        (lim, rf(m, sun + 0x14))
    };
    let r = renderer(m);
    let deg = F(f32::from_bits(0x3c8e_fa35));
    let (sa, ca) = sin_cos(rf(m, sun + 0x28) * deg);
    let (sb, cb) = sin_cos(rf(m, sun + 0x2c) * deg);
    let b = block(m);
    let cam = rq(m, b + 0x70).map(|x| x * F(-1.0));
    let o = rows_times(m, b + 0x40, cam);
    let dir = [ca * cb, sa * cb, sb, one];
    let base = [o[0], o[1], o[2], zero];
    let mut p = [0, 1, 2, 3].map(|k| dir[k] * dist + base[k]);
    p[3] = one;
    let (t, f) = vfn(m, r, 0x270);
    if env.hidden(m, f, t, bits(p)) != 0 {
        m.w32(s, 0);
        return;
    }
    m.w32(s, 1);
    let (t, f) = vfn(m, r, 0x278);
    let pr = env.project(m, f, t, bits(p));
    for k in 0..4 {
        m.w32(((s + 0x10) & !15) + 4 * k, pr[k as usize]);
    }
    m.w32(s + 0x30, 0x10);
    let fx = rf(m, s + 0x10) - F(8.0);
    let fy = rf(m, s + 0x14) - F(8.0);
    m.w32(s + 0x2c, 0x10);
    m.w32(s + 0x24, int(fx));
    m.w32(s + 0x28, int(fy));
    for k in 0..4 {
        let w = m.r32(s + 0x24 + 4 * k);
        m.w32(s + 0x34 + 4 * k, w);
    }
    for (at, side) in [(0x34u32, 0x38u32), (0x38, 0x3c)] {
        let b = block(m);
        let v = m.r32(s + at) as i32;
        let hi = (m.r32(b + 0x30).wrapping_add(m.r32(b + side)) as i32).wrapping_sub(0x10);
        m.w32(s + at, if v < 0 { 0 } else { v.min(hi) } as u32);
    }
    let depth = m.r32(s + 0x18);
    m.w32(s + 0x20, depth);
    let b = block(m);
    let right = rows_times_x(m, b + 0x40, zero).map(|x| x * size);
    let down = rows_times(m, b + 0x40, [zero, F(-1.0), zero, zero]).map(|x| x * size);
    let mut right = [0, 1, 2, 3].map(|k| right[k] + p[k]);
    let mut down = [0, 1, 2, 3].map(|k| down[k] + p[k]);
    right[3] = one;
    down[3] = one;
    let (t, f) = vfn(m, r, 0x278);
    let pr_r = env.project(m, f, t, bits(right)).map(|w| F(f32::from_bits(w)));
    let pr_d = env.project(m, f, t, bits(down)).map(|w| F(f32::from_bits(w)));
    let (sx, sy) = (rf(m, s + 0x10), rf(m, s + 0x14));
    let (dx, dy) = (pr_r[0] - sx, pr_d[1] - sy);
    m.w32(s + 0x68, one.0.to_bits());
    m.w32(s + 0x78, one.0.to_bits());
    let (ix, iy) = (int(dx + dx), int(dy + dy));
    m.w32(s + 0x50, ix);
    m.w32(s + 0x54, iy);
    let hx = cvt_s_w(ix as i32) * F(0.5);
    let hy = cvt_s_w(iy as i32) * F(0.5);
    m.w32(s + 0x48, int(sx - hx));
    m.w32(s + 0x4c, int(sy - hy));
    let half_of = |m: &Mem, at: u32, k: F| int(cvt_s_w(m.r32(at) as i32) * k);
    let v = half_of(m, node + 0x18, F(0.5));
    m.w32(s + 0x58, v);
    let v = half_of(m, node + 0x1c, zero);
    m.w32(s + 0x5c, v);
    let v = half_of(m, node + 0x18, F(0.5));
    m.w32(s + 0x60, v);
    let v = half_of(m, node + 0x1c, F(0.5));
    m.w32(s + 0x64, v);
    let k255 = F(255.0);
    let c = |m: &Mem, o: u32| div(cvt_s_w(m.r32(sun + o) as i32), k255);
    for (o, at) in [(0xc, 0x84), (0x18, 0x6c), (0x1c, 0x70), (0x20, 0x74), (4, 0x7c), (8, 0x80)] {
        let v = c(m, o);
        m.w32(s + at, v.0.to_bits());
    }
    let b = block(m);
    let x0 = cvt_s_w(m.r32(b + 0x30) as i32);
    let y0 = cvt_s_w(m.r32(b + 0x34) as i32);
    let w = cvt_s_w(m.r32(b + 0x38) as i32);
    let h = cvt_s_w(m.r32(b + 0x3c) as i32);
    let (ex0, ey0) = (sx - x0, sy - y0);
    let nx = div(ex0 + ex0, w) - one;
    let ny = div(ey0 + ey0, h) - one;
    // the corners' angles from the sun
    let pi = F(f32::from_bits(0x4049_0fdb));
    let atan = |x: F| F(crate::math::atan_with(x.0, |a, b| div(F(a), F(b)).0, |a| sqrt(F(a)).0));
    let mut corner = [zero; 4];
    for (k, (cx, cy)) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        let ddx = F(cx) - nx;
        let ddy = F(cy) - ny;
        let mut a = if 0.0 < ddx.ee() {
            atan(div(ddy, ddx))
        } else if ddx.ee() < 0.0 {
            atan(div(ddy, ddx)) + pi
        } else if 0.0 < ddy.ee() {
            F(f32::from_bits(0x3fc9_0fdb))
        } else {
            F(f32::from_bits(0xbfc9_0fdb))
        };
        if a.ee() < 0.0 {
            a = a + F(f32::from_bits(0x40c9_0fdb));
        }
        corner[k] = a;
    }
    // the rays to the screen's edge
    let (mut ci, mut cnt, mut step, mut e) = (0usize, 0u32, 0i32, 0i32);
    while step < 0x25 {
        let at = SUN_RAYS.wrapping_add((8 * e) as u32);
        let (ang, len) = if ci < 4 && corner[ci].ee() < rf(m, at).ee() {
            let v = (corner[ci], rf(m, at.wrapping_sub(4)));
            ci += 1;
            e -= 1;
            step -= 1;
            v
        } else {
            (rf(m, at), rf(m, at + 4))
        };
        let (sn, cs) = sin_cos(ang);
        let to_edge = |d: F, from: F| {
            if 0.0 < d.ee() {
                div(one - from, d)
            } else if d.ee() < 0.0 {
                div(F(-1.0) - from, d)
            } else {
                F(10.0)
            }
        };
        let tx = to_edge(cs, nx);
        let ty = to_edge(sn, ny);
        let (ex, ey) = if tx.ee() < ty.ee() {
            (if 0.0 < cs.ee() { one } else { F(-1.0) }, ny + tx * sn)
        } else {
            (nx + ty * cs, if 0.0 < sn.ee() { one } else { F(-1.0) })
        };
        let px = x0 + ((ex + one) * F(0.5)) * (w - one);
        let py = y0 + ((ey + one) * F(0.5)) * (h - one);
        let (qx, qy) = (ex - nx, ey - ny);
        let mut d2 = qx * qx + qy * qy;
        let pt = s + 0x8c + 8 * cnt;
        m.w32(pt, px.0.to_bits());
        m.w32(pt + 4, py.0.to_bits());
        let three = F(3.0);
        if three.ee() < d2.ee() {
            d2 = three;
        }
        let fade = div(three - d2, three);
        m.w32(s + 0x1f4 + 8 * cnt, len.0.to_bits());
        m.w32(s + 0x1f8 + 8 * cnt, fade.0.to_bits());
        cnt += 1;
        step += 1;
        e += 1;
    }
    for k in 0..2 {
        let v = m.r32(s + 0x8c + 4 * k);
        m.w32(s + 0x8c + 8 * cnt + 4 * k, v);
    }
    let (l0, f0) = (m.r32(s + 0x1f4), m.r32(s + 0x1f8));
    m.w32(s + 0x1f4 + 8 * cnt, l0);
    m.w32(s + 0x88, cnt + 1);
    m.w32(s + 0x1f8 + 8 * cnt, f0);
}

fn add4(a: [F; 4], b: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|k| a[k] + b[k])
}
fn sub4(a: [F; 4], b: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|k| a[k] - b[k])
}
fn scale4(a: [F; 4], s: F) -> [F; 4] {
    a.map(|x| x * s)
}
fn words(q: [u32; 4]) -> [F; 4] {
    q.map(|w| F(f32::from_bits(w)))
}

/// The flares' part, 0x1cc190(node, sizes): for each flare the view keeps (its entry, +0x1c0 + 0x1040 half +
/// 0x820 view + 0xd0 k), what the renderer's callback draws:
/// - +0x10 the light's projected position (+0x278); +0x24.. its 16 × 8 sprite box (8 and 4 back), +0x34.. that box
///   clamped to the view; +0x20 the depth of the point moved toward the camera by the kind's reach.
/// - the kind (flags & 0x70: 0x10, 0x20, 0x40) gives a size and a reach (100.74 and 300, 200.08 and 500, 500 and
///   800). Any other kind keeps the last one's (`sizes`, the two registers as the caller left them, then as the
///   last flare set them).
/// - +0x58.. a glow box, twice the projection of the size along the camera's x (and −y) axes; +0x78.. a second one,
///   the size scaled by the sun's +0x34 plus 1.0095 × ((√2 − the flare's distance from the screen's middle) ×
///   0.7071)⁴; +0x68.., +0x88.. halves of the texture's answers.
/// - +0x98..+0xb4 a streak's four corners: turned by −(the flare's screen x × π/2) (`Math_Cos`, `Math_Sin`), sized
///   by the sun's +0x38 plus 1.2523 × that fourth power; +0xb8.. texture halves.
/// - +0x4c.. its colour (+0x10.. × +8) normalised (`Vec3_Dot`, `Vec3_Div`; left as it is at length 0); +0x48 1, or
///   past half the sun's limit (+0x5c) fading to 0 at the limit.
pub fn flares_part(m: &mut Mem, env: &mut dyn LensEnv, node: u32, sizes: [F; 2]) {
    let (one, zero, halfk) = (F(1.0), F(0.0), F(0.5));
    let r = renderer(m);
    let n = {
        let at = node.wrapping_add(m.r32(node + 0x14).wrapping_mul(4)).wrapping_add(m.r32(node + 0xc).wrapping_mul(8));
        m.r32(at.wrapping_add(0x1b0)) as i32
    };
    let sun = m.r32(game(m) + 0x2c);
    let (mut size, mut reach) = (sizes[0], sizes[1]);
    let mut k = 0i32;
    while k < n {
        let half = m.r32(node + 0xc);
        let view = m.r32(node + 0x14);
        let e = node
            .wrapping_add(half.wrapping_mul(0x1040))
            .wrapping_add(0x1c0)
            .wrapping_add(view.wrapping_mul(0x820))
            .wrapping_add((k as u32).wrapping_mul(0xd0));
        let light = m.r32(e + 0xcc);
        let p = [rf(m, light + 0x28), rf(m, light + 0x2c), rf(m, light + 0x30), one];
        let (t, f) = vfn(m, r, 0x278);
        let q = env.project(m, f, t, bits(p));
        for j in 0..4 {
            m.w32(((e + 0x10) & !15) + 4 * j, q[j as usize]);
        }
        m.w32(e + 0x30, 8);
        let fx = rf(m, e + 0x10) - F(8.0);
        let fy = rf(m, e + 0x14) - F(4.0);
        m.w32(e + 0x2c, 0x10);
        m.w32(e + 0x24, int(fx));
        m.w32(e + 0x28, int(fy));
        for j in 0..4 {
            let w = m.r32(e + 0x24 + 4 * j);
            m.w32(e + 0x34 + 4 * j, w);
        }
        for (at, side, less) in [(0x34u32, 0x38u32, 0x10i32), (0x38, 0x3c, 8)] {
            let b = node.wrapping_add(m.r32(node + 0x14).wrapping_mul(0x60)).wrapping_add(m.r32(node + 0xc).wrapping_mul(0xc0));
            let v = m.r32(e + at) as i32;
            let hi = (m.r32(b + 0x30).wrapping_add(m.r32(b + side)) as i32).wrapping_sub(less);
            m.w32(e + at, if v < 0 { 0 } else { v.min(hi) } as u32);
        }
        match m.r32(light + 4) & 0x70 {
            0x10 => (size, reach) = (F(f32::from_bits(0x42c9_7ab1)), F(300.0)),
            0x20 => (size, reach) = (F(f32::from_bits(0x4348_1413)), F(500.0)),
            0x40 => (size, reach) = (F(500.0), F(800.0)),
            _ => {}
        }
        let b = node.wrapping_add(m.r32(node + 0x14).wrapping_mul(0x60)).wrapping_add(m.r32(node + 0xc).wrapping_mul(0xc0));
        let mat = b + 0x40;
        let fwd = [rf(m, b + 0x48) * reach, rf(m, b + 0x58) * reach, rf(m, b + 0x68) * reach, zero];
        let (t, f) = vfn(m, r, 0x278);
        let moved = env.project(m, f, t, bits(sub4(p, fwd)));
        m.w32(e + 0x20, moved[2]);
        let x_axis = rows_times_x(m, mat, zero);
        let y_axis = rows_times(m, mat, [zero, F(-1.0), zero, zero]);
        let mut a = add4(p, scale4(x_axis, size));
        let mut c = add4(p, scale4(y_axis, size));
        a[3] = one;
        c[3] = one;
        let (t, f) = vfn(m, r, 0x278);
        let pa = words(env.project(m, f, t, bits(a)));
        let pc = words(env.project(m, f, t, bits(c)));
        let (qx, qy) = (rf(m, e + 0x10), rf(m, e + 0x14));
        let glow = |m: &mut Mem, at: u32, pa: [F; 4], pc: [F; 4]| {
            let (ix, iy) = (int((pa[0] - qx) + (pa[0] - qx)), int((pc[1] - qy) + (pc[1] - qy)));
            m.w32(e + at + 8, ix);
            m.w32(e + at + 0xc, iy);
            m.w32(e + at, int(qx - cvt_s_w(ix as i32) * halfk));
            m.w32(e + at + 4, int(qy - cvt_s_w(iy as i32) * halfk));
        };
        glow(m, 0x58, pa, pc);
        let tex = |m: &mut Mem, at: u32, ks: [F; 4]| {
            for (j, (src, kk)) in [(0x18u32, ks[0]), (0x1c, ks[1]), (0x18, ks[2]), (0x1c, ks[3])].into_iter().enumerate() {
                let v = int(cvt_s_w(m.r32(node + src) as i32) * kk);
                m.w32(e + at + 4 * j as u32, v);
            }
        };
        tex(m, 0x68, [zero, zero, halfk, halfk]);
        let b = node.wrapping_add(m.r32(node + 0x14).wrapping_mul(0x60)).wrapping_add(m.r32(node + 0xc).wrapping_mul(0xc0));
        let x0 = cvt_s_w(m.r32(b + 0x30) as i32);
        let y0 = cvt_s_w(m.r32(b + 0x34) as i32);
        let hh = cvt_s_w(m.r32(b + 0x3c) as i32);
        let (dx, dy) = (qx - x0, qy - y0);
        let ww = cvt_s_w(m.r32(b + 0x38) as i32);
        let nx = div(dx + dx, ww) - one;
        let ny = div(dy + dy, hh) - one;
        let rr = sqrt(nx * nx + ny * ny);
        let mut fall = (F(f32::from_bits(0x3fb5_0529)) - rr) * F(f32::from_bits(0x3f35_04bd));
        fall = fall * fall;
        fall = fall * fall;
        let s2 = size * (rf(m, sun + 0x34) + fall * F(f32::from_bits(0x3f81_39f6)));
        let a = add4(p, scale4(x_axis, s2));
        let c = add4(p, scale4(y_axis, s2));
        let mut a = a;
        let mut c = c;
        a[3] = one;
        c[3] = one;
        let (t, f) = vfn(m, r, 0x278);
        let pa = words(env.project(m, f, t, bits(a)));
        let pc = words(env.project(m, f, t, bits(c)));
        glow(m, 0x78, pa, pc);
        tex(m, 0x88, [halfk, zero, halfk, halfk]);
        // the streak, turned with the flare's place across the screen
        let ang = -((nx * F(f32::from_bits(0x4049_0fdb))) * halfk);
        let (ca, sa) = (F(crate::math::cos(ang.0)), F(crate::math::sin(ang.0)));
        let ang2 = ang + F(f32::from_bits(0x3fc9_0fdb));
        let u = rows_times(m, mat, [ca, sa, zero, zero]);
        let (cb, sb) = (-F(crate::math::cos(ang2.0)), -F(crate::math::sin(ang2.0)));
        let streak = fall * F(f32::from_bits(0x3fa0_49e0));
        let v = rows_times(m, mat, [cb, sb, zero, zero]);
        let s3 = size * (rf(m, sun + 0x38) + streak);
        let mut a = add4(p, scale4(u, s3));
        let mut c = add4(p, scale4(v, s3));
        a[3] = one;
        c[3] = one;
        let (t, f) = vfn(m, r, 0x278);
        // (the projected quad as `lqc2` reads it: rounded down to 16)
        let q4 = rq(m, e + 0x10);
        let su = sub4(words(env.project(m, f, t, bits(a))), q4);
        let sv = sub4(words(env.project(m, f, t, bits(c))), q4);
        for (at, cu, cv) in [(0x98u32, -1.0f32, -1.0f32), (0xa0, 1.0, -1.0), (0xa8, 1.0, 1.0), (0xb0, -1.0, 1.0)] {
            let t = if cu < 0.0 { sub4(q4, su) } else { add4(q4, su) };
            let t = if cv < 0.0 { sub4(t, sv) } else { add4(t, sv) };
            m.w32(e + at, t[0].0.to_bits());
            m.w32(e + at + 4, t[1].0.to_bits());
        }
        tex(m, 0xb8, [zero, halfk, halfk, halfk]);
        // the colour, normalised
        let lk = rf(m, light + 8);
        let col = [rf(m, light + 0x10) * lk, rf(m, light + 0x14) * lk, rf(m, light + 0x18) * lk];
        let len = sqrt((col[0] * col[0] + col[1] * col[1]) + col[2] * col[2]);
        let nc = if len.ee() == 0.0 {
            col
        } else {
            let inv = div(one, len);
            [col[0] * inv, col[1] * inv, col[2] * inv]
        };
        m.w32(e + 0x4c, nc[0].0.to_bits());
        m.w32(e + 0x50, nc[1].0.to_bits());
        m.w32(e + 0x48, one.0.to_bits());
        m.w32(e + 0x54, nc[2].0.to_bits());
        let depth = rf(m, e + 0xc8);
        let lim = rf(m, sun + 0x5c) * halfk;
        if lim.ee() < depth.ee() {
            let v = div(lim - (depth - lim), lim);
            m.w32(e + 0x48, v.0.to_bits());
        }
        k += 1;
    }
}

/// The node's own virtual at `off` (its vtable at +8; the PS2 node's, row G7d2e)
fn node_vfn(m: &Mem, node: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(node + 8);
    (node.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}

/// The renderer's lens FX callback 0x1cdc78 (a jump to 0x1cdc80)(node): with a half ready (+0x10 not negative):
/// how much of the sun and of each kept flare shows, by the node's +0x50 (its sprite box clamped and not, its depth;
/// the answer to +0x44); when any shows: +0x58, then for each view +0x60 (the view's block +0x30), the sun's +0x70,
/// each flare's +0x68 and +0x78; then +0x80
pub fn callback(m: &mut Mem, env: &mut dyn LensEnv, node: u32) {
    let h = m.r32(node + 0x10);
    if (h as i32) < 0 {
        return;
    }
    let views = |m: &Mem| m.r32((node + 0x20).wrapping_add(h.wrapping_mul(4))) as i32;
    let count = |m: &Mem, v: u32| m.r32(node.wrapping_add(0x1b0).wrapping_add(v.wrapping_mul(4)).wrapping_add(h.wrapping_mul(8))) as i32;
    let sun_at = |v: u32| node.wrapping_add(h.wrapping_mul(0x6c0)).wrapping_add(0x2240).wrapping_add(v.wrapping_mul(0x360));
    let flare_at = |v: u32, k: u32| {
        node.wrapping_add(h.wrapping_mul(0x1040)).wrapping_add(0x1c0).wrapping_add(v.wrapping_mul(0x820)).wrapping_add(k.wrapping_mul(0xd0))
    };
    let shows = |m: &mut Mem, env: &mut dyn LensEnv, at: u32| {
        let (t, f) = node_vfn(m, node, 0x50);
        let depth = m.r32(at + 0x20);
        let v = env.node_vcall(m, f, t, &[at + 0x34, at + 0x24], depth);
        m.w32(at + 0x44, v);
        F(f32::from_bits(v)).ee() != 0.0
    };
    let mut any = false;
    let mut v = 0u32;
    while (v as i32) < views(m) {
        let s = sun_at(v);
        if m.r32(s) != 0 && shows(m, env, s) {
            any = true;
        }
        let mut k = 0u32;
        while (k as i32) < count(m, v) {
            let e = flare_at(v, k);
            if m.r32(e) != 0 && shows(m, env, e) {
                any = true;
            }
            k += 1;
        }
        v += 1;
    }
    if !any {
        return;
    }
    let call = |m: &mut Mem, env: &mut dyn LensEnv, off: u32, args: &[u32]| {
        let (t, f) = node_vfn(m, node, off);
        env.node_vcall(m, f, t, args, 0);
    };
    call(m, env, 0x58, &[]);
    let mut v = 0u32;
    while (v as i32) < views(m) {
        call(m, env, 0x60, &[node.wrapping_add(h.wrapping_mul(0xc0)).wrapping_add(0x30).wrapping_add(v.wrapping_mul(0x60))]);
        let s = sun_at(v);
        if m.r32(s) != 0 {
            call(m, env, 0x70, &[s]);
        }
        let mut k = 0u32;
        while (k as i32) < count(m, v) {
            call(m, env, 0x68, &[flare_at(v, k)]);
            k += 1;
        }
        call(m, env, 0x78, &[]);
        v += 1;
    }
    call(m, env, 0x80, &[]);
}

/// 0x1cdfc0(node, k): the colour scale (+0x2fd8) set, clamped to 0..1
pub fn set_scale(m: &mut Mem, node: u32, k: F) {
    let v = if k.ee() < 0.0 {
        F(0.0)
    } else if 1.0 < k.ee() {
        F(1.0)
    } else {
        k
    };
    m.w32(node + 0x2fd8, v.0.to_bits());
}

/// 0x1ce000(node): the colour scale
pub fn scale(m: &Mem, node: u32) -> F {
    rf(m, node + 0x2fd8)
}

/// 0x1ce058(out, v, k): v × k, all four lanes (as `Vec4_Scale` 0x102f50); out
pub fn vec4_scale(m: &mut Mem, out: u32, v: u32, k: F) -> u32 {
    let q = rq(m, v).map(|x| x * k);
    for j in 0..4 {
        m.w32((out & !15) + 4 * j, q[j as usize].0.to_bits());
    }
    out
}

/// 0x1ce088(out, v, d): v's x, y, z × (1 ÷ d) (as `Vec3_Div` 0x137108); out
pub fn vec3_div(m: &mut Mem, out: u32, v: u32, d: F) -> u32 {
    let k = div(F(1.0), d);
    let z = rf(m, v + 8) * k;
    let x = rf(m, v) * k;
    let y = rf(m, v + 4) * k;
    m.w32(out + 8, z.0.to_bits());
    m.w32(out, x.0.to_bits());
    m.w32(out + 4, y.0.to_bits());
    out
}

/// 0x1ce0c0(a, b): (a.x b.x + a.y b.y) + a.z b.z (as `Vec3_Dot` 0x137140)
pub fn vec3_dot(m: &Mem, a: u32, b: u32) -> F {
    (rf(m, a) * rf(m, b) + rf(m, a + 4) * rf(m, b + 4)) + rf(m, a + 8) * rf(m, b + 8)
}

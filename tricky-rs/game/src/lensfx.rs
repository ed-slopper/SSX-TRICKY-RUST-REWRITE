//! The lens flares' node (board row G7d2a): `cLensFXObjNode` 0x1cbbc8..0x1cc088, `LensFX_AddFlare` 0x1cc088 and
//! the world's `LensFX_Draw` 0x2006e8 that offers it the course's flaring lights. On the bytes at their addresses
//! ([`Mem`]); the base object, the renderer's virtuals and the node's two drawing parts (the sun's 0x1cd040, row
//! G7d2b, and the flares' 0x1cc190, G7d2c) are calls out ([`LensEnv`]). Checked in the function runner
//! (`tests/lensfx.rs`).
//!
//! **The node** (0x2fdc bytes): +0xc the frame's half (0 / 1, flipped on view 0), +0x10 the half ready to draw,
//! +0x14 the view being drawn, +0x18 / +0x1c the renderer's answers for the flare texture (+0x2fc0), +0x20 the views
//! of each half; +0x30 + 0xc0 half + 0x60 view a view's block (+0x30 the renderer's view quad, +0x40 the camera
//! matrix, +0x80..+0x88 three view values); +0x1b0 + 8 half + 4 view the flares kept, and their entries at +0x1c0 +
//! 0x1040 half + 0x820 view + 0xd0 k (+0 1, +0xc8 the depth, +0xcc the light); +0x2240.. the sun's (G7d2b);
//! +0x2fc4.. the sun's colours × +0x2fd8.

use crate::pf::Mem;
use crate::ps2f::F;

pub const VTABLE: u32 = 0x391bd8;
pub const GAPP: u32 = 0x337c58;
pub const WORLD: u32 = 0x346488;
/// The renderer's callback the node registers (row G7d2d)
pub const CALLBACK: u32 = 0x1cdc78;
pub const BASE_CONSTRUCT: u32 = 0x17cf50;
pub const BASE_DESTRUCT: u32 = 0x17cfe0;
pub const SUN_PART: u32 = 0x1cd040;
pub const FLARES_PART: u32 = 0x1cc190;

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
    env.call(m, SUN_PART, &[this]);
    env.call(m, FLARES_PART, &[this]);
    let half = m.r32(this + 0xc);
    let views = m.r32((this + 0x20).wrapping_add(half.wrapping_mul(4)));
    if m.r32(this + 0x14) == views.wrapping_sub(1) {
        m.w32(this + 0x10, half);
    }
}

//! The rider's shadow (board rows G7g1, G7g2): its set-up (`BoarderShadow_Init` 0x1c8c38), its texture
//! (`BoarderShadow_AllocTexture` 0x1c8c70) and its draw (`BoarderShadow_Draw` 0x1c8bc8). On the bytes at their
//! addresses ([`Mem`]); the renderer's virtuals are calls out ([`ParticleDrawEnv`]). Checked in the function
//! runner (`tests/shadow.rs`).
//!
//! **The shadow** (as these use it): +0xc its place (x, y, z), +0x90 and +0xd0 two blocks the renderer's draw
//! takes, +0x170 its texture (−1 none yet), +0x174 / +0x178 the texture's width and height.
//!
//! The draw hands the renderer the place with w = 1 from the game's stack; the port builds it in `scratch`
//! (STANDIN: the frame's place; the same 16 bytes).

use crate::animseq::{div, sqrt};
use crate::particledraw::{ParticleDrawEnv, APP};
use crate::pf::Mem;
use crate::ps2f::F;

type V = [F; 4];
type M4 = [V; 4];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rq(m: &Mem, a: u32) -> V {
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wm(m: &mut Mem, a: u32, x: &M4) {
    for i in 0..4 {
        for j in 0..4 {
            wf(m, a + 0x10 * i as u32 + 4 * j as u32, x[i][j]);
        }
    }
}
fn rm(m: &Mem, a: u32) -> M4 {
    [0, 1, 2, 3].map(|i| rq(m, a + 0x10 * i))
}

/// The bones whose places bound the shadow (each 0x40 bytes, the place at +0x30), then the points (0x10 each)
pub const BONES: u32 = 0x391590;
pub const POINTS: u32 = 0x3915a8;

/// `Vec3_Dot` 0x137140 of a vector with itself, its root (`sqrt.s`), and the vector over it (`Vec3_Div`
/// 0x137108: × 1 ÷ the root), or the vector as it is when the root is 0
fn unit(v: [F; 3]) -> [F; 3] {
    let n = sqrt((v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]);
    if n == F(0.0) {
        return v;
    }
    let k = div(F(1.0), n);
    v.map(|x| x * k)
}
/// The matrix's rows each with `p`: ((p.x r.x + p.y r.y) + p.z r.z) + p.w r.w
fn transform(x: &M4, p: V) -> V {
    x.map(|r| ((p[0] * r[0] + p[1] * r[1]) + p[2] * r[2]) + p[3] * r[3])
}
/// `p` times `a` as VU0 makes it: row i of the answer the rows of `a` by row i of `p`
fn mat_mul(p: &M4, a: &M4) -> M4 {
    p.map(|b| [0, 1, 2, 3].map(|c| ((a[0][c] * b[0] + a[1][c] * b[1]) + a[2][c] * b[2]) + a[3][c] * b[3]))
}
fn transpose(x: &M4) -> M4 {
    [0, 1, 2, 3].map(|c| [0, 1, 2, 3].map(|r| x[r][c]))
}

/// `BoarderShadow_Update` 0x1c7e90(shadow, pos, dir, up, bones, points): the shadow's camera and its three
/// matrices for this frame.
/// - **The camera.** The light's way (`dir`, made a unit: +0) and `up` (+0x18). The eye (+0xc) is `pos` less
///   `up` × 0 (as the game has it). +0x24 is −up · eye. Unless the way is `up` or −`up` (then the way's x is
///   only nudged up by 0.01, and the side and the third axis kept from before), the side (+0x34) is the unit
///   of way × up, and the third axis (+0x28) the unit of up × side.
/// - **The view** (+0x50): the rows side, third, up, each with −(row · eye) as its w; then (0, 0, 0, 1).
/// - **The box.** The six bones' places (0x391590) and four points (0x3915a8) through the view, their x and y
///   ranges each widened by 20: +0x40 / +0x44 x's, +0x48 / +0x4c y's top and bottom. The z range is not used.
/// - **Three matrices**, each a projection of the box times the view: +0x90 to clip space (z by a fixed
///   0.000645 and −0.9355), +0xd0 to the texture's 0..1 (y down), +0x110 to −1..1 with y down. Each is
///   transposed where it is at the end (MMI pairs).
/// - **The texture's place**: +0x150 half its width and height, 0, 0; +0x160 2048, 2048, 0, 0.
pub fn update(m: &mut Mem, shadow: u32, pos: u32, dir: u32, up: u32, bones: u32, points: u32) {
    let s = shadow;
    let v3 = |m: &Mem, a: u32| [rf(m, a), rf(m, a + 4), rf(m, a + 8)];
    let way = unit(v3(m, dir));
    for k in 0..3 {
        wf(m, s + 4 * k as u32, way[k]);
    }
    let u = unit(v3(m, up));
    for k in 0..3 {
        wf(m, s + 0x18 + 4 * k as u32, u[k]);
    }
    let p = v3(m, pos);
    let eye = [0, 1, 2].map(|k| p[k] - u[k] * F(0.0));
    for k in 0..3 {
        wf(m, s + 0xc + 4 * k as u32, eye[k]);
    }
    let nu = u.map(|x| -x);
    wf(m, s + 0x24, (nu[0] * eye[0] + nu[1] * eye[1]) + nu[2] * eye[2]);
    let same = |a: [F; 3], b: [F; 3]| a[0] == b[0] && a[1] == b[1] && a[2] == b[2];
    if same(way, u) || same(way, nu) {
        wf(m, s, way[0] + F(f32::from_bits(0x3c23_d70a)));
    } else {
        let side = unit([way[1] * u[2] - way[2] * u[1], way[2] * u[0] - way[0] * u[2], way[0] * u[1] - way[1] * u[0]]);
        for k in 0..3 {
            wf(m, s + 0x34 + 4 * k as u32, side[k]);
        }
        let third = unit([u[1] * side[2] - u[2] * side[1], u[2] * side[0] - u[0] * side[2], u[0] * side[1] - u[1] * side[0]]);
        for k in 0..3 {
            wf(m, s + 0x28 + 4 * k as u32, third[k]);
        }
    }
    let (z, one) = (F(0.0), F(1.0));
    let side = v3(m, s + 0x34);
    let third = v3(m, s + 0x28);
    let mut view: M4 = [
        [side[0], side[1], side[2], z],
        [third[0], third[1], third[2], z],
        [rf(m, s + 0x18), u[1], u[2], z],
        [z, z, z, one],
    ];
    let e = [rf(m, s + 0xc), rf(m, s + 0x10), rf(m, s + 0x14), one];
    let t = transform(&view, e);
    for k in 0..3 {
        view[k][3] = -t[k];
    }
    wm(m, s + 0x50, &view);
    // the box: x (min, max), y, z, from ±f32::MAX
    let big = F(f32::MAX);
    let mut lo = [big; 3];
    let mut hi = [-big; 3];
    let mut grow = |q: V| {
        for k in 0..3 {
            if q[k] < lo[k] {
                lo[k] = q[k];
            }
            if hi[k] < q[k] {
                hi[k] = q[k];
            }
        }
    };
    for k in 0..6 {
        let b = m.r32(BONES + 4 * k);
        grow(transform(&view, rq(m, bones.wrapping_add(b << 6) + 0x30)));
    }
    for k in 0..4 {
        let b = m.r32(POINTS + 4 * k);
        grow(transform(&view, rq(m, points.wrapping_add(b << 4))));
    }
    let margin = F(20.0);
    let (x0, x1, y1, y0) = (lo[0] - margin, hi[0] + margin, hi[1] + margin, lo[1] - margin);
    wf(m, s + 0x40, x0);
    wf(m, s + 0x44, x1);
    wf(m, s + 0x48, y1);
    wf(m, s + 0x4c, y0);
    let two = F(2.0);
    let clip: M4 = [
        [div(two, x1 - x0), z, z, div(-(x1 + x0), x1 - x0)],
        [z, div(two, y1 - y0), z, div(-(y1 + y0), y1 - y0)],
        [z, z, F(f32::from_bits(0x3a29_200b)), F(f32::from_bits(0xbf6f_7bdf))],
        [z, z, z, one],
    ];
    let tex: M4 = [
        [div(one, x1 - x0), z, z, div(-x0, x1 - x0)],
        [z, div(F(-1.0), y1 - y0), z, div(y0, y1 - y0) + one],
        [z, z, z, one],
        [z, z, z, one],
    ];
    let flat: M4 = [
        [div(two, x1 - x0), z, z, div(-(x1 + x0), x1 - x0)],
        [z, div(F(-2.0), y1 - y0), z, div(y1 + y0, y1 - y0)],
        [z, z, z, one],
        [z, z, z, one],
    ];
    let out = [mat_mul(&clip, &view), mat_mul(&tex, &view), mat_mul(&flat, &view)];
    for (k, x) in out.iter().enumerate() {
        wm(m, s + 0x90 + 0x40 * k as u32, x);
    }
    let half = F(0.5);
    wf(m, s + 0x150, crate::animseq::cvt_s_w(m.r32(s + 0x174) as i32) * half);
    wf(m, s + 0x154, crate::animseq::cvt_s_w(m.r32(s + 0x178) as i32) * half);
    for (k, v) in [z, z, F(2048.0), F(2048.0), z, z].into_iter().enumerate() {
        wf(m, s + 0x158 + 4 * k as u32, v);
    }
    for k in 0..3 {
        let at = s + 0x90 + 0x40 * k;
        let x = transpose(&rm(m, at));
        wm(m, at, &x);
    }
}

/// `gRace_EntryCount`: one rider gets the larger texture
pub const ENTRY_COUNT: u32 = 0x32db6c;

fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}
fn vcall(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}

/// `BoarderShadow_Init` 0x1c8c38(shadow): no texture yet; 256 × 256 for a race of one rider, else 128 × 128;
/// the shadow handed back
pub fn init(m: &mut Mem, shadow: u32) -> u32 {
    m.w32(shadow + 0x170, u32::MAX);
    let size = if m.r32(ENTRY_COUNT) == 1 { 0x100 } else { 0x80 };
    m.w32(shadow + 0x178, size);
    m.w32(shadow + 0x174, size);
    shadow
}

/// `BoarderShadow_AllocTexture` 0x1c8c70(shadow): when it has none (+0x170 below 0), its texture from the
/// renderer's +0x170 (width, height, 2)
pub fn alloc_texture(m: &mut Mem, env: &mut dyn ParticleDrawEnv, shadow: u32) {
    if (m.r32(shadow + 0x170) as i32) >= 0 {
        return;
    }
    let r = renderer(m);
    let (w, h) = (m.r32(shadow + 0x174), m.r32(shadow + 0x178));
    let tex = vcall(m, env, r, 0x170, &[w, h, 2]);
    m.w32(shadow + 0x170, tex);
}

/// `BoarderShadow_Draw` 0x1c8bc8(shadow): the renderer's +0x2d8 with the place (w = 1), +0x90, +0xd0, the
/// texture and 1.0 (f12; last in the words)
pub fn draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, shadow: u32, scratch: u32) {
    let r = renderer(m);
    for k in 0..3 {
        let v = m.r32(shadow + 0xc + 4 * k);
        m.w32(scratch + 4 * k, v);
    }
    m.w32(scratch + 0xc, 1f32.to_bits());
    let tex = m.r32(shadow + 0x170);
    vcall(m, env, r, 0x2d8, &[scratch, shadow + 0x90, shadow + 0xd0, tex, 1f32.to_bits()]);
}

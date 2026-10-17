//! The renderer's box test (board row G7c4): `cPS2GraphicsMan::CullBox` 0x1e1b10, the renderer's vtable slot +0x280
//! that the snow, the fog and the world's walks ask whether a box is in view. On the bytes at their addresses
//! ([`Mem`]); the two calls its matrix update makes are calls out ([`CullEnv`]). Checked in the function runner
//! (`tests/cull.rs`), the VU0 micro program run by the runner's micro mode.

use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: 0x1c4650(r+0x480) and 0x1c4c38(r+0x480, r+0x610), after the matrices are made
pub trait CullEnv {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]);
}

pub const VIEW_UPDATE: u32 = 0x1c4650;
pub const VIEW_PLANES: u32 = 0x1c4c38;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn rq(m: &Mem, a: u32) -> [F; 4] {
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wq(m: &mut Mem, a: u32, v: [F; 4]) {
    for k in 0..4 {
        m.w32(a + 4 * k, v[k as usize].0.to_bits());
    }
}
fn rows(m: &Mem, a: u32) -> [[F; 4]; 4] {
    [0, 1, 2, 3].map(|r| rq(m, a + 0x10 * r))
}

/// Four rows times a row vector on VU0 (vmulax / vmadday / vmaddaz / vmaddw): Σ rows[k] × v[k]
fn mat_vec(rows: &[[F; 4]; 4], v: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|i| ((rows[0][i] * v[0] + rows[1][i] * v[1]) + rows[2][i] * v[2]) + rows[3][i] * v[3])
}

/// The matrix at `b` times the one at `a` (each of b's rows through a's rows), written to `out`
fn mat_mul(m: &mut Mem, out: u32, a: u32, b: u32) {
    let (ra, rb) = (rows(m, a), rows(m, b));
    let p = rb.map(|row| mat_vec(&ra, row));
    for (k, row) in p.into_iter().enumerate() {
        wq(m, out + 0x10 * k as u32, row);
    }
}

/// VU clip flags of `v` against its w: x > |w| 1, x < −|w| 2, then y 4 / 8, z 0x10 / 0x20
fn clip(v: [F; 4]) -> u32 {
    let w = v[3].0.abs();
    let mut c = 0;
    for k in 0..3 {
        if v[k].0 > w {
            c |= 1 << (2 * k);
        }
        if v[k].0 < -w {
            c |= 2 << (2 * k);
        }
    }
    c
}

/// The frame's matrices, made by the frame's first box test or model draw (`CullBox`, `DrawModel`): with +0x17d0
/// clear (it is set), +0x1770 the camera's (+0x3f4) times +0x1730; +0x490 a copy of the camera's; +0x510 that times
/// +0x4d0; +0x5b0 that times +0x570; then 0x1c4650 and 0x1c4c38 with +0x480 (and +0x610)
pub fn frame_matrices<E: CullEnv + ?Sized>(m: &mut Mem, env: &mut E, r: u32) {
    if m.r32(r + 0x17d0) != 0 {
        return;
    }
    m.w32(r + 0x17d0, 1);
    let cam = m.r32(r + 0x3f4);
    mat_mul(m, r + 0x1770, r + 0x1730, cam);
    let cam = m.r32(r + 0x3f4);
    for k in 0..4 {
        let row = rq(m, cam + 0x10 * k);
        wq(m, r + 0x490 + 0x10 * k, row);
    }
    mat_mul(m, r + 0x510, r + 0x4d0, r + 0x490);
    mat_mul(m, r + 0x5b0, r + 0x570, r + 0x490);
    let v = m.r32(r + 0x480);
    env.call(m, VIEW_UPDATE, &[v]);
    let v = m.r32(r + 0x480);
    env.call(m, VIEW_PLANES, &[v, r + 0x610]);
}

/// `cPS2GraphicsMan::CullBox` 0x1e1b10(r, min, max): the box's eight corners through the renderer's clip matrix
/// (+0x5b0): 1 when every corner is beyond the same side, 0 when none is beyond any, else 2.
///
/// The first test of a frame makes the frame's matrices ([`frame_matrices`]). The corners (VU0 micro program
/// 0x320: the box's w for each) are min and max mixed in the order (min min min), (min min max), (min max min),
/// (min max max), (max min min), (max min max), (max max min), (max max max); each one's clip flags (`vclipw`)
/// OR-ed and AND-ed.
pub fn cull_box(m: &mut Mem, env: &mut dyn CullEnv, r: u32, min: u32, max: u32) -> u32 {
    frame_matrices(m, env, r);
    let (lo, hi) = (rq(m, min), rq(m, max));
    let clip_rows = rows(m, r + 0x5b0);
    let (mut any, mut all) = (0u32, 63u32);
    for k in 0..8 {
        let pick = |i: usize| if k >> (2 - i) & 1 != 0 { hi[i] } else { lo[i] };
        // (each lane × 1, vf14)
        let corner = [pick(0), pick(1), pick(2), lo[3]].map(|v| F(1.0) * v);
        let c = clip(mat_vec(&clip_rows, corner));
        any |= c;
        all &= c;
    }
    if all != 0 {
        1
    } else if any == 0 {
        0
    } else {
        2
    }
}

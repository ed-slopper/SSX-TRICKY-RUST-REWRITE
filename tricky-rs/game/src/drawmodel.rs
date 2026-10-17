//! The renderer's model draw (board row G7b2): `cPS2GraphicsMan::DrawModel` 0x1e1f70, the renderer's +0x2a8 that
//! every object draw ends in (`objdraw`). It sets each material's texture and blend, and for each of the model's
//! batches puts a small DMA packet in the renderer's buffer: the matrix and the light block for VU1, then a call of
//! the batch's own prebuilt packet. On the bytes at their addresses ([`Mem`]); the frame's matrices are
//! `cull::frame_matrices`; the renderer's virtuals, its matrix update and the packet's submit are calls out
//! ([`DrawEnv`]). Checked in the function runner (`tests/drawmodel.rs`).
//!
//! **A model**: +4 its bones (+8 their records, 0x18 each: +0 the parent, −1 none; +4 the bone's mesh or 0; +0x14
//! its bind pose), +0x18 its scale (x, y, z). **A mesh**: +0x20 its batches (+0x28 their list). **A batch**: +4 its
//! texture's place in the part's table, +0xc its packet (called), +0x10 where the UV scroll goes. **A part's
//! texture** (its table at +4): +0 the texture's id, +0x40 flags (0x40000 fogged, 0x20000 the second blend).
//!
//! The game builds the matrices on its stack; the port builds them in `scratch` (STANDIN: the frame's place; the
//! same bytes), 0x720 bytes.

use crate::cull::{self, CullEnv};
use crate::pf::Mem;
use crate::ps2f::F;

pub const WORLD_MATRIX: u32 = 0x346440;
pub const MATRIX_UPDATE: u32 = 0x1e85b0;
pub const SUBMIT: u32 = 0x1e8838;

/// The calls out (with the frame's two, as `cull::CullEnv`)
pub trait DrawEnv: CullEnv {
    /// A renderer virtual with these words (+0x1e0, +0x1e8, +0x1f0)
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]);
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn rq(m: &Mem, a: u32) -> [F; 4] {
    let a = a & !15;
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wq(m: &mut Mem, a: u32, v: [F; 4]) {
    let a = a & !15;
    for k in 0..4 {
        m.w32(a + 4 * k, v[k as usize].0.to_bits());
    }
}
fn copy_matrix(m: &mut Mem, to: u32, from: u32) {
    for i in 0..4 {
        let q = rq(m, from + 0x10 * i);
        wq(m, to + 0x10 * i, q);
    }
}
fn w64(m: &mut Mem, a: u32, v: u64) {
    m.w32(a, v as u32);
    m.w32(a + 4, (v >> 32) as u32);
}
fn sx(v: u32) -> u64 {
    v as i32 as i64 as u64
}
fn vfn(m: &Mem, r: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(r + 0x10);
    (r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
/// The matrix at `b` times the one at `a` (each of b's rows through a's rows, VU0), written at `out`
fn mat_mul(m: &mut Mem, out: u32, a: u32, b: u32) {
    let ra = [0, 1, 2, 3].map(|i| rq(m, a + 0x10 * i));
    let p = [0, 1, 2, 3].map(|i| {
        let v = rq(m, b + 0x10 * i);
        [0, 1, 2, 3].map(|c| ((ra[0][c] * v[0] + ra[1][c] * v[1]) + ra[2][c] * v[2]) + ra[3][c] * v[3])
    });
    for (i, row) in p.into_iter().enumerate() {
        wq(m, out + 0x10 * i as u32, row);
    }
}
/// The matrix at `a` with its rows scaled by the four lanes of `s` (vmulx, vmuly, vmulz, vmulw), at `out`
fn scale_rows(m: &mut Mem, out: u32, a: u32, s: [F; 4]) {
    for i in 0..4 {
        let row = rq(m, a + 0x10 * i).map(|x| x * s[i as usize]);
        wq(m, out + 0x10 * i, row);
    }
}

/// `cPS2GraphicsMan::DrawModel` 0x1e1f70(r, model, part, matrix, lights, bones, scroll).
///
/// - The frame's matrices (the frame's first draw); the texture state cache reset (+0x470: its +0x10 byte 5 and +0
///   0, with +0x474 1) unless it is so; +0x1e0 (3).
/// - Without bones: the matrix with its rows scaled by the model's scale; and, unless the model has one bone posed
///   at the world's matrix, its bones from their bind poses (each times its parent's).
/// - Each bone with a mesh of batches: with bones, the matrix times the bone (its rows scaled) for it. Each batch:
///   its texture (+0x1f0 with its id, 0), its blend (+0x1e8: fogged (2, the fog of the matrix's place: its w through
///   the clip matrix +0x5b0, × +0x630's w, clamped to 0..+0x630's x, made whole) or (2, 1) without fog (+0x640 0);
///   the second blend (1, 0); else (0, 0)); the UV scroll (× 4096) into the batch's packet when there is one; and
///   its packet at the renderer's +0x680 (`cnt` 12, UNPACK of the matrix and the light block's rows 0..2 and 4..7,
///   then a `call` of the batch's packet), submitted (0x1e8838) and the place moved on 0xf0
#[allow(clippy::too_many_arguments)]
pub fn draw_model(m: &mut Mem, env: &mut dyn DrawEnv, r: u32, model: u32, part: u32, matrix: u32, lights: u32, bones: u32, scroll: u32, scratch: u32) {
    cull::frame_matrices(m, env, r);
    let t = m.r32(r + 0x470);
    if m.r8(t + 0x10) != 5 || m.r32(t) != 0 {
        m.w8(t + 0x10, 5);
        m.w32(t, 0);
        m.w32(r + 0x474, 1);
    }
    let (tt, f) = vfn(m, r, 0x1e0);
    env.vcall(m, f, tt, &[3]);
    let scale = [rf(m, model + 0x18), rf(m, model + 0x1c), rf(m, model + 0x20), F(1.0)];
    let mat = scratch;
    let tmp = scratch + 0x90;
    let built = scratch + 0xd0;
    let tmp2 = scratch + 0x6d0;
    let table = m.r32(model + 8);
    let mut bone_at = bones;
    if bones == 0 {
        scale_rows(m, mat, matrix, scale);
        let n = m.r32(model + 4) as i32;
        if !(n < 2 && m.r32(table + 0x14) == WORLD_MATRIX) {
            bone_at = built;
            let mut k = 0u32;
            while (k as i32) < m.r32(model + 4) as i32 {
                let rec = table.wrapping_add(0x18 * k);
                let parent = m.r32(rec);
                let pose = m.r32(rec + 0x14);
                if parent == u32::MAX {
                    copy_matrix(m, built + 0x40 * k, pose);
                } else {
                    mat_mul(m, tmp, built.wrapping_add(parent << 6), pose);
                    copy_matrix(m, built + 0x40 * k, tmp);
                }
                k += 1;
            }
        }
    }
    let table = m.r32(model + 8);
    let mut k = 0u32;
    while (k as i32) < m.r32(model + 4) as i32 {
        let rec = table.wrapping_add(0x18 * k);
        let mesh = m.r32(rec + 4);
        let kk = k;
        k += 1;
        if mesh == 0 || m.r32(mesh + 0x20) == 0 {
            continue;
        }
        if bone_at != 0 {
            scale_rows(m, tmp2, bone_at.wrapping_add(kk << 6), scale);
            copy_matrix(m, tmp, tmp2);
            mat_mul(m, tmp2, matrix, tmp);
            copy_matrix(m, mat, tmp2);
        }
        let mut b = 0u32;
        while (b as i32) < m.r32(mesh + 0x20) as i32 {
            let batch = m.r32(m.r32(mesh + 0x28).wrapping_add(4 * b));
            b += 1;
            let tex = m.r32((part + 4).wrapping_add(m.r32(batch + 4).wrapping_mul(4)));
            let (tt, f) = vfn(m, r, 0x1f0);
            env.vcall(m, f, tt, &[m.r16(tex) as i16 as i32 as u32, 0]);
            let flags = m.r32(tex + 0x40);
            let (tt, f) = vfn(m, r, 0x1e8);
            if flags & 0x4_0000 != 0 {
                if m.r32(r + 0x640) != 0 {
                    let pos = rq(m, matrix + 0x30);
                    env.call(m, MATRIX_UPDATE, &[r]);
                    let k5 = rq(m, r + 0x630);
                    let rows = [0, 1, 2, 3].map(|i| rq(m, r + 0x5b0 + 0x10 * i));
                    let w = ((rows[0][3] * pos[0] + rows[1][3] * pos[1]) + rows[2][3] * pos[2]) + rows[3][3] * pos[3];
                    let w = w * k5[3];
                    let w = if w.ee() >= 0.0 { w } else { F(0.0) };
                    let w = if w.ee() <= k5[0].ee() { w } else { k5[0] };
                    let v = (w.ee() as i32 as u32) & 0xffff;
                    env.vcall(m, f, tt, &[2, v]);
                } else {
                    env.vcall(m, f, tt, &[2, 1]);
                }
            } else if flags & 0x2_0000 != 0 {
                env.vcall(m, f, tt, &[1, 0]);
            } else {
                env.vcall(m, f, tt, &[0, 0]);
            }
            if scroll != 0 {
                let pkt = m.r32(batch + 0x10);
                w64(m, pkt, 0x3000_0000_0500_0001);
                let k4 = F(4096.0);
                let v = sx((rf(m, scroll + 4) * k4).ee() as i32 as u32);
                let u = sx((rf(m, scroll) * k4).ee() as i32 as u32);
                w64(m, pkt + 8, u | v << 32);
            }
            let p = m.r32(r + 0x680);
            w64(m, p, 0x1000_000c);
            w64(m, p + 0x18, 0xd817u64 << 47);
            w64(m, p + 8, 0);
            w64(m, p + 0x10, 0);
            for j in 0..8 {
                let v = m.r32(mat + 4 * j);
                m.w32(p + 0x20 + 4 * j, v);
                let v = m.r32(mat + 0x20 + 4 * j);
                m.w32(p + 0x40 + 4 * j, v);
            }
            for j in 0..12 {
                let v = m.r32(lights + 4 * j);
                m.w32(p + 0x60 + 4 * j, v);
            }
            for j in 0..16 {
                let v = m.r32(lights + 0x40 + 4 * j);
                m.w32(p + 0x90 + 4 * j, v);
            }
            w64(m, p + 0xd8, 0);
            w64(m, p + 0xd0, (m.r32(batch + 0xc) as u64) << 32 | 0x5000_0000);
            env.call(m, SUBMIT, &[r, p, p + 0xe0, 0]);
            let p = m.r32(r + 0x680);
            m.w32(r + 0x680, p.wrapping_add(0xf0));
        }
    }
}

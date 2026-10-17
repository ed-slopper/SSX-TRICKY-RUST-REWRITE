//! The objects' draws (board row G7b1): the course's instances by cell (`Instance_DrawVisibleCells` 0x200390,
//! `Instance_Draw` 0x200680) and the moving ones that light themselves where they are: `cRollerNode::Draw`
//! 0x13f9f8, `cMeshAnimNode::Draw` 0x147518 (its bones from quaternions) and `cAnimObjectNode` vf28 0x199320 (its
//! bones from the animation and the bind pose). On the bytes at their addresses ([`Mem`]); the lights are
//! `crate::lights` (gathered with `World_GetLightsAt`, built with `Lights_Build`, turned with `Mat4_RotateBy`); the
//! renderer's virtuals (its box test +0x280, the model draw +0x2a8), the objects' matrices and the animation's
//! bones are calls out ([`ObjEnv`]). Checked in the function runner (`tests/objdraw.rs`).
//!
//! **The model draw** (the renderer's +0x2a8, `cPS2GraphicsMan::DrawModel`, row G7b2) takes the model, a part
//! (the model's +0xc, or the instance's +0xf0), a matrix, a light block (`Lights_Build`'s: an instance keeps its
//! own at +0x40) and the bones (or none).
//!
//! The game builds its matrices and light blocks on its stack; the port builds them in `scratch` (STANDIN: the
//! frame's place; the same bytes), 0x800 bytes.

use crate::lights;
use crate::pf::Mem;
use crate::ps2f::F;

pub const GAPP: u32 = 0x337c58;
pub const WORLD: u32 = 0x346488;
/// The matrix the animated meshes are drawn with (the world's)
pub const WORLD_MATRIX: u32 = 0x346440;
/// The show-off mode (3 and 5 count an INode's show-off instances)
pub const SHOW_MODE: u32 = 0x32de8c;
/// 128 entries of 12 bytes whose first words the cell draw clears (from the last)
pub const DRAWN: u32 = 0x31d938;
pub const ROLLER_FRAME: u32 = 0x13e6a0;
pub const POS_QUAT: u32 = 0x250428;
pub const ANIM_BONE: u32 = 0x1998d0;

/// The calls out
pub trait ObjEnv {
    /// The renderer's box test (+0x280) on a box (min, max): 1 outside
    fn cull(&mut self, m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32;
    /// A virtual (the renderer's state calls with their words, f12 as bits; the light manager's +0x78; the
    /// animated object's +0x80)
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// The renderer's model draw (+0x2a8): the model, the part, and the matrix, light block and bones by address
    /// (the bones 0 for none)
    fn draw_model(&mut self, m: &mut Mem, func: u32, this: u32, model: u32, part: u32, matrix: u32, lights: u32, bones: u32);
    /// A function with these words: `RollerNode_PlaceFromBody` 0x13e6a0(out, node) and `Mat4_FromPosQuat`
    /// 0x250428(out, in) write a matrix at out; 0x1998d0(node, k) answers where the k-th animated bone's matrix is
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn vfn(m: &Mem, obj: u32, vt_at: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + vt_at);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}
/// Sixteen words copied as `lq`/`sq` do (both rounded down to 16)
fn copy_quads(m: &mut Mem, to: u32, from: u32, n: u32) {
    for k in 0..4 * n {
        let w = m.r32((from & !15) + 4 * k);
        m.w32((to & !15) + 4 * k, w);
    }
}
/// A box's two corners (x, y, z from `at`, w 1) for the renderer's test
fn corner(m: &Mem, at: u32) -> [u32; 4] {
    [m.r32(at), m.r32(at + 4), m.r32(at + 8), 1f32.to_bits()]
}
fn model_draw(m: &mut Mem, env: &mut dyn ObjEnv, model: u32, part: u32, matrix: u32, lights: u32, bones: u32) {
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x10, 0x2a8);
    env.draw_model(m, f, t, model, part, matrix, lights, bones);
}
fn state(m: &mut Mem, env: &mut dyn ObjEnv, off: u32, args: &[u32]) {
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x10, off);
    env.vcall(m, f, t, args);
}
/// The renderer's box test on the instance's box (+0xcc, +0xd8)
fn culled(m: &mut Mem, env: &mut dyn ObjEnv, inst: u32) -> bool {
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x10, 0x280);
    let (lo, hi) = (corner(m, inst + 0xcc), corner(m, inst + 0xd8));
    env.cull(m, f, t, lo, hi) == 1
}

/// The lights at `pos` (a quad's address) built into a light block at `out` and turned by the matrix at `turn`:
/// `World_GetLightsAt` (the ambient at `work`, the colours at +0x80, the directions at +0xb0), `Lights_Build`,
/// `Mat4_RotateBy`
fn light_here(m: &mut Mem, pos: u32, work: u32, out: u32, turn: u32) {
    let (amb, cols, dirs) = (work, work + 0x80, work + 0xb0);
    lights::get_lights_at(m, WORLD, pos, amb, cols, dirs);
    lights::lights_build(m, out, amb, dirs, cols, dirs + 0x10, cols + 0x10, dirs + 0x20, cols + 0x20);
    lights::mat4_rotate_by(m, out, turn);
}

/// `Instance_Draw` 0x200680(world, inst, matrix, lights): with the instance drawn (+0xe8 bit 0) its model (+0xc0)
/// and the model's +0xc
pub fn instance_draw(m: &mut Mem, env: &mut dyn ObjEnv, inst: u32, matrix: u32, lights: u32) {
    let model = m.r32(inst + 0xc0);
    if m.r32(inst + 0xe8) & 1 == 0 {
        return;
    }
    let part = m.r32(model + 0xc);
    model_draw(m, env, model, part, matrix, lights, 0);
}

/// `Instance_DrawVisibleCells` 0x200390(world): the renderer's +0x1c8 (0, 2, 0.01) and +0x1e0 (3); the drawn
/// marks cleared; then each live cell (to the first empty one) and each of its slots its mask names: its
/// instances (+0x26, or +0x28 in show-off modes 3 and 5) that are on and drawn (+0xe8 bits 0 and 1), each tested
/// (its box, +0x280) unless the cell holds that slot wholly in view (+0x32), drawn with its own matrix (+0) and light
/// block (+0x40); then +0x1c8 (0, 2, 0.08) and the light manager's +0x78 (0)
pub fn draw_visible_cells(m: &mut Mem, env: &mut dyn ObjEnv, world: u32) {
    state(m, env, 0x1c8, &[0, 2, 0x3c23_d70a]);
    state(m, env, 0x1e0, &[3]);
    for i in 0..128u32 {
        m.w32(DRAWN + 0x5fc - 0xc * i, 0);
    }
    let mut c = 0u32;
    while (c as i32) < 0xa2 {
        let cell = m.r32(world + 0x3c + 4 * c);
        c += 1;
        if cell == 0 {
            break;
        }
        if m.r16(cell + 0x36) as i16 == 0 {
            continue;
        }
        let mut k = 0u32;
        while (k as i32) < m.r32(m.r32(world) + 0x48) as i32 {
            let rec = m.r32(cell + 0x38).wrapping_add(0x4c * k);
            let mode = m.r32(SHOW_MODE);
            let n = if mode == 3 || mode == 5 { m.r16(rec + 0x28) } else { m.r16(rec + 0x26) } as i16 as i32;
            let kk = k;
            k += 1;
            if n == 0 || (m.r16(cell + 0x36) as i16 as i32 >> (kk & 31)) & 1 == 0 {
                continue;
            }
            let test = (m.r16(cell + 0x32) as i16 as i32 >> (kk & 31)) & 1 == 0;
            let list = m.r32(rec + 0x38);
            let mut i = 0i32;
            while i < n {
                let inst = m.r32(list.wrapping_add(4 * i as u32));
                i += 1;
                let model = m.r32(inst + 0xc0);
                if m.r32(inst + 0xe8) & 3 != 3 {
                    continue;
                }
                if test && culled(m, env, inst) {
                    continue;
                }
                let part = m.r32(model + 0xc);
                model_draw(m, env, model, part, inst, inst + 0x40, 0);
            }
        }
    }
    state(m, env, 0x1c8, &[0, 2, 0x3da3_d70a]);
    let lm = m.r32(lights::LIGHT_MAN);
    let (t, f) = vfn(m, lm, 0, 0x78);
    env.vcall(m, f, t, &[0]);
}

/// `cRollerNode::Draw` 0x13f9f8(node): with its instance (+0x28) shown (+0xe8 bit 2) and its box (+0x2a0, +0x2b0)
/// not outside: its matrix (`RollerNode_PlaceFromBody`, `Mat4_FromPosQuat`), the lights at its place, the instance
/// drawn with them (`Instance_Draw`)
pub fn roller_draw(m: &mut Mem, env: &mut dyn ObjEnv, node: u32, scratch: u32) {
    let inst = m.r32(node + 0x28);
    if m.r32(inst + 0xe8) & 4 == 0 {
        return;
    }
    let r = renderer(m);
    let (t, f) = vfn(m, r, 0x10, 0x280);
    let (lo, hi) = ([0, 1, 2, 3].map(|k| m.r32(node + 0x2a0 + 4 * k)), [0, 1, 2, 3].map(|k| m.r32(node + 0x2b0 + 4 * k)));
    if env.cull(m, f, t, lo, hi) == 1 {
        return;
    }
    let (frame, mat, block, work, pos) = (scratch + 0xc0, scratch + 0x80, scratch, scratch + 0x200, scratch + 0x140);
    env.call(m, ROLLER_FRAME, &[frame, node]);
    env.call(m, POS_QUAT, &[mat, frame]);
    // (the place, the matrix's last row, copied where the lights are asked about)
    copy_quads(m, pos, mat + 0x30, 1);
    light_here(m, pos, work, block, mat);
    let inst = m.r32(node + 0x28);
    instance_draw(m, env, inst, mat, block);
}

/// The bone matrix of a quaternion (+0x38 + 0x10 k: w, x, y, z) and a position (+0x3c0 + 0x10 k), as
/// `cMeshAnimNode::Draw` writes it
fn quat_bone(m: &mut Mem, node: u32, k: u32, out: u32) {
    let q = node + 0x38 + 0x10 * k;
    let (w, x, y, z) = (rf(m, q), rf(m, q + 4), rf(m, q + 8), rf(m, q + 0xc));
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, yy, wz, xz) = (x * x2, y * y2, w * z2, x * z2);
    let (wx, yz, wy, xy, zz) = (w * x2, y * z2, w * y2, x * y2, z * z2);
    let one = F(1.0);
    let row = [
        [(one - yy) - zz, xy + wz, xz - wy],
        [xy - wz, (one - xx) - zz, yz + wx],
        [xz + wy, yz - wx, (one - xx) - yy],
    ];
    for (i, r) in row.iter().enumerate() {
        for (j, v) in r.iter().enumerate() {
            wf(m, out + 0x10 * i as u32 + 4 * j as u32, *v);
        }
        m.w32(out + 0x10 * i as u32 + 0xc, 0);
    }
    copy_quads(m, out + 0x30, node + 0x3c0 + 0x10 * k, 1);
}

/// `cMeshAnimNode::Draw` 0x147518(node): with it on (+0x6cc): unless already shown (+0x6d0) its instance's box
/// tested (outside: nothing); shown; its bones from its quaternions; drawn with the world's matrix, with the
/// instance's own light block when its model asks (+0x10 bit 1), else with the lights at its place (+0x6c0's
/// matrix, or the instance's) turned by the instance's matrix
pub fn mesh_anim_draw(m: &mut Mem, env: &mut dyn ObjEnv, node: u32, scratch: u32) {
    if m.r32(node + 0x6cc) == 0 {
        return;
    }
    if m.r32(node + 0x6d0) == 0 {
        let inst = m.r32(node + 0x28);
        if culled(m, env, inst) {
            return;
        }
    }
    m.w32(node + 0x6d0, 1);
    let inst = m.r32(node + 0x28);
    let model = m.r32(inst + 0xc0);
    let bones = scratch + 0x20;
    let mut k = 0u32;
    while (k as i32) < m.r32(model + 4) as i32 {
        quat_bone(m, node, k, bones + 0x40 * k);
        k += 1;
    }
    if m.r32(model + 0x10) & 2 != 0 {
        let part = m.r32(inst + 0xf0);
        model_draw(m, env, model, part, WORLD_MATRIX, inst + 0x40, bones);
        return;
    }
    let (block, work, pos) = (scratch + 0x620, scratch + 0x700, scratch + 0x7e0);
    let at = m.r32(node + 0x6c0);
    let src = if at != 0 { at } else { inst };
    copy_quads(m, pos, src + 0x30, 1);
    light_here(m, pos, work, block, m.r32(node + 0x28));
    let inst = m.r32(node + 0x28);
    let part = m.r32(inst + 0xf0);
    model_draw(m, env, model, part, WORLD_MATRIX, block, bones);
}

/// `cAnimObjectNode` vf28 0x199320(node, matrix, test): with `test`, its instance's (+0x58) box tested (outside:
/// nothing). Its bones (the model's +8, 0x18 each: +0 the parent, −1 none; +0x10 animated; +0x14 the bind pose),
/// each the animation's next matrix (0x1998d0) or its bind pose, times its parent's. Drawn (between the renderer's
/// +0x1c8 (0, 2, 0.01) and (0, 2, 0.08)) with the instance's own light block when its +0x30 object's +0x80 says so
/// or its model asks (+0x10 bit 1), else with the lights at the matrix's place turned by it
pub fn anim_object_draw(m: &mut Mem, env: &mut dyn ObjEnv, node: u32, matrix: u32, test: u32, scratch: u32) {
    if test != 0 {
        let inst = m.r32(node + 0x58);
        if culled(m, env, inst) {
            return;
        }
    }
    let inst = m.r32(node + 0x58);
    let model = m.r32(inst + 0xc0);
    let bones = scratch + 0x20;
    let product = scratch + 0x660;
    let mut rec = m.r32(model + 8);
    let mut animated = 0u32;
    let mut k = 0i32;
    while k < m.r32(model + 4) as i32 {
        let out = bones + 0x40 * k as u32;
        let parent = m.r32(rec);
        let src = if m.r32(rec + 0x10) != 0 {
            let at = env.call(m, ANIM_BONE, &[node, animated]);
            animated += 1;
            at
        } else {
            m.r32(rec + 0x14)
        };
        if parent == u32::MAX {
            copy_quads(m, out, src, 4);
        } else {
            mat_mul(m, product, bones + (parent << 6), src);
            copy_quads(m, out, product, 4);
        }
        k += 1;
        rec = rec.wrapping_add(0x18);
    }
    let sub = node + 0x30;
    let (t, f) = vfn(m, sub, 8, 0x80);
    let own = env.vcall(m, f, t, &[]) != 0 || m.r32(model + 0x10) & 2 != 0;
    if own {
        state(m, env, 0x1c8, &[0, 2, 0x3c23_d70a]);
        let inst = m.r32(node + 0x58);
        let part = m.r32(inst + 0xf0);
        model_draw(m, env, model, part, matrix, inst + 0x40, bones);
        state(m, env, 0x1c8, &[0, 2, 0x3da3_d70a]);
        return;
    }
    let (block, work, pos) = (scratch + 0x620, scratch + 0x6a0, scratch + 0x7f0);
    copy_quads(m, pos, matrix + 0x30, 1);
    light_here(m, pos, work, block, matrix);
    state(m, env, 0x1c8, &[0, 2, 0x3c23_d70a]);
    let inst = m.r32(node + 0x58);
    let part = m.r32(inst + 0xf0);
    model_draw(m, env, model, part, matrix, block, bones);
    state(m, env, 0x1c8, &[0, 2, 0x3da3_d70a]);
}

/// The matrix at `b` times the one at `a` (each of b's rows through a's rows, VU0), written at `out`
fn mat_mul(m: &mut Mem, out: u32, a: u32, b: u32) {
    let row = |m: &Mem, at: u32, i: u32| [0, 1, 2, 3].map(|j| rf(m, (at & !15) + 0x10 * i + 4 * j));
    let ra = [row(m, a, 0), row(m, a, 1), row(m, a, 2), row(m, a, 3)];
    let p: Vec<[F; 4]> = (0..4)
        .map(|i| {
            let v = row(m, b, i);
            [0, 1, 2, 3].map(|c| ((ra[0][c] * v[0] + ra[1][c] * v[1]) + ra[2][c] * v[2]) + ra[3][c] * v[3])
        })
        .collect();
    for (i, r) in p.iter().enumerate() {
        for (j, v) in r.iter().enumerate() {
            wf(m, (out & !15) + 0x10 * i as u32 + 4 * j as u32, *v);
        }
    }
}

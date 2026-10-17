//! The terrain's draw (board row G7a1): `cTerrainObjNode::Draw` 0x1c7810 and `Patch_DrawVisibleCells` 0x2008f8,
//! which hands each visible Bezier patch of the course to the renderer's patch draw (+0x2d0, row G7a3) with the
//! tessellation of its four edges chosen by the camera's distance. On the bytes at their addresses ([`Mem`]); the
//! cells' visits (G7a2) and the renderer's and the Bezier manager's virtuals are calls out ([`TerrainEnv`]). Checked
//! in the function runner (`tests/terraindraw.rs`).
//!
//! **A patch** (listed by the slots of the `.ltg` cells, +0x24 a count, +0x34 the list): +0x150 / +0x15c its box,
//! +0x168 + 0x10 k its four corners, +0x1ac negative for a show-off patch, +0x1b0 / +0x1b2 two shorts for the draw.

use crate::pf::Mem;
use crate::ps2f::F;

pub const GAPP: u32 = 0x337c58;
pub const WORLD: u32 = 0x346488;
/// The show-off mode (3 and 5 draw the show-off patches)
pub const SHOW_MODE: u32 = 0x32de8c;
/// The tessellation by LOD 0..3: 4, 4, 6, 8
pub const LOD_STEPS: u32 = 0x39afb8;
pub const VISIT_CELLS_NEAR: u32 = 0x260278;
pub const CULL_VISIBILITY: u32 = 0x260458;

/// The calls out
pub trait TerrainEnv {
    /// A function with these words (and f12 as bits)
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], f12: u32);
    /// A virtual (the renderer's, the Bezier manager's) with these words; its answer (a float's bits for the
    /// manager's two distances)
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// The renderer's box tests (+0x280, +0x290) on a box (min, max)
    fn cull(&mut self, m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32;
    /// The renderer's patch draw (+0x2d0): the patch, its four edges' steps, its two shorts, clipped
    fn draw_patch(&mut self, m: &mut Mem, func: u32, this: u32, patch: u32, steps: [u8; 4], shorts: [u32; 2], clip: u32);
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn vfn(m: &Mem, obj: u32, vt_at: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + vt_at);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x724)
}
fn corner(m: &Mem, at: u32) -> [u32; 4] {
    [m.r32(at), m.r32(at + 4), m.r32(at + 8), 1f32.to_bits()]
}

/// `cTerrainObjNode::Draw` 0x1c7810: the draw distance (30000, or the course's +0x60, +0x64 with two views or
/// more; the game's course read even without a game, from address 0x2c); the cells near the camera (0x260278 with
/// (distance − 1) ÷ 10000 + 1 of them) and their visibility (0x260458 with the distance) visited; the patches drawn
pub fn terrain_draw(m: &mut Mem, env: &mut dyn TerrainEnv) {
    let game = m.r32(m.r32(GAPP) + 0x730);
    let cam = (m.r32(game.wrapping_add(0x290)) << 7).wrapping_add(game).wrapping_add(0xb0);
    let mut dist = F(30000.0);
    let course = m.r32(game.wrapping_add(0x2c));
    if game != 0 || course != 0 {
        dist = if (m.r32(game.wrapping_add(0x298)) as i32) < 2 { rf(m, course.wrapping_add(0x60)) } else { rf(m, course.wrapping_add(0x64)) };
    }
    let n = ((dist - F(1.0)).ee() as i32).wrapping_div(10000).wrapping_add(1);
    env.call(m, VISIT_CELLS_NEAR, &[WORLD, cam, n as u32], 0);
    env.call(m, CULL_VISIBILITY, &[WORLD, cam], dist.0.to_bits());
    patch_draw_visible(m, env, WORLD);
}

/// `Patch_DrawVisibleCells` 0x2008f8(world).
///
/// - The LOD's line: from the Bezier manager (the renderer's +0x20) its near and far distances (+0x30, +0x38): LOD
///   = 3 − 2 (d² − 2 near²) ÷ (2 far² − 2 near²) for a d² summed over two corners.
/// - The camera's place in its own frame (the renderer's +0xf8: its rows times its last row turned back), w 1.
/// - +0x2c0; then each live cell (to the first empty one) and each slot its mask names: its patches (show-off ones,
///   +0x1ac negative, only in show-off modes 3 and 5), each tested unless the cell holds the slot wholly in view
///   (+0x280: outside 1 skips it; partly 2 asks +0x290, clipped when not 0).
/// - Each edge's LOD from its two corners' distances², clamped to 1..3, made whole, as its steps (0x39afb8: 4, 6,
///   8), in the order of the edges 0–1, 2–3, 0–2, 1–3; then +0x2d0
pub fn patch_draw_visible(m: &mut Mem, env: &mut dyn TerrainEnv, world: u32) {
    let steps = [0, 1, 2, 3].map(|k| m.r8(LOD_STEPS + k));
    let r = renderer(m);
    let (one, three) = (F(1.0), F(3.0));
    let lod_dist = |m: &mut Mem, env: &mut dyn TerrainEnv, off: u32| {
        let (t, f) = vfn(m, r, 0x10, 0x20);
        let bz = env.vcall(m, f, t, &[]);
        let (t, f) = vfn(m, bz, 0, off);
        F(f32::from_bits(env.vcall(m, f, t, &[])))
    };
    let near = lod_dist(m, env, 0x30);
    let far = lod_dist(m, env, 0x38);
    let far2 = far * far;
    let near2 = near * near;
    let far2 = far2 + far2;
    let near2 = near2 + near2;
    let k = crate::animseq::div(F(-2.0), far2 - near2);
    let base = three - k * near2;
    let (t, f) = vfn(m, r, 0x10, 0xf8);
    let mat = env.vcall(m, f, t, &[]);
    let back = [0, 1, 2, 3].map(|j| rf(m, ((mat + 0x30) & !15) + 4 * j) * F(-1.0));
    let (t, f) = vfn(m, r, 0x10, 0xf8);
    let mat = env.vcall(m, f, t, &[]);
    let e = |i: u32, j: u32| rf(m, mat + 0x10 * i + 4 * j);
    let p = [0, 1, 2].map(|i| ((back[0] * e(i, 0) + back[1] * e(i, 1)) + back[2] * e(i, 2)) + back[3] * e(i, 3));
    let (t, f) = vfn(m, r, 0x10, 0x2c0);
    env.vcall(m, f, t, &[]);
    let mode = m.r32(SHOW_MODE);
    let hide_show_off = mode != 3 && mode != 5;
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
        let mut s = 0u32;
        while (s as i32) < m.r32(m.r32(world) + 0x48) as i32 {
            let rec = m.r32(cell + 0x38).wrapping_add(0x4c * s);
            let ss = s;
            s += 1;
            let n = m.r16(rec + 0x24) as i16 as i32;
            if n == 0 || (m.r16(cell + 0x36) as i16 as i32 >> (ss & 31)) & 1 == 0 {
                continue;
            }
            let test = (m.r16(cell + 0x32) as i16 as i32 >> (ss & 31)) & 1 == 0;
            let list = m.r32(rec + 0x34);
            for i in 0..n.max(0) as u32 {
                let patch = m.r32(list.wrapping_add(4 * i));
                if (m.r32(patch + 0x1ac) as i32) < 0 && hide_show_off {
                    continue;
                }
                let mut clip = 0;
                if test {
                    let (lo, hi) = (corner(m, patch + 0x150), corner(m, patch + 0x15c));
                    let (t, f) = vfn(m, r, 0x10, 0x280);
                    let v = env.cull(m, f, t, lo, hi);
                    if v == 1 {
                        continue;
                    }
                    clip = if v == 2 {
                        let (t, f) = vfn(m, r, 0x10, 0x290);
                        (env.cull(m, f, t, lo, hi) != 0) as u32
                    } else {
                        (v != 0) as u32
                    };
                }
                let d = [0u32, 1, 2, 3].map(|j| {
                    let q = patch + 0x168 + 0x10 * j;
                    let (x, y, z) = (p[0] - rf(m, q), p[1] - rf(m, q + 4), p[2] - rf(m, q + 8));
                    (x * x + y * y) + z * z
                });
                let lod = |a: F, b: F| {
                    let v = base + k * (a + b);
                    if v.ee() < one.ee() {
                        one
                    } else if three.ee() < v.ee() {
                        three
                    } else {
                        v
                    }
                };
                let edges = [lod(d[0], d[1]), lod(d[2], d[3]), lod(d[0], d[2]), lod(d[1], d[3])];
                let st = edges.map(|v| steps[(v.ee() as i32 as usize) & 3]);
                let shorts = [m.r16(patch + 0x1b0) as i16 as i32 as u32, m.r16(patch + 0x1b2) as i16 as i32 as u32];
                let (t, f) = vfn(m, r, 0x10, 0x2d0);
                env.draw_patch(m, f, t, patch, st, shorts, clip);
            }
        }
    }
}

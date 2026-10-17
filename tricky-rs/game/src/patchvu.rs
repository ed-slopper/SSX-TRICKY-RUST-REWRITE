//! The VU1 patch programs (rows G7a4b..e), on [`Vu`]: the uniform ones, one program in three sizes. The 4-step
//! program (row G7a4b: the chain at 0x30f960, 743 instructions) is the Bezier lists' kind 3 (patches with 4 steps
//! on every edge); the 6-step (G7a4c: 0x3110f0, 821) kind 2; the 8-step (G7a4d: 0x312af0, 860) kind 1. They differ
//! in the grid's size (N × N points from the 1/(N−1) weights), the strips' counts, their unrolled loops' registers,
//! and the far plane: the 6- and 8-step programs clip by it, the 4-step one sends a triangle past it back to its
//! strip. Checked against the game's programs run in the function runner's VU1 (`tests/vu1patch.rs`).
//!
//! **What it is given** (VU1 data memory, in quadwords):
//! - 0..5: the matrix set (0..3 the camera's matrix to the clip frame, 4 the screen's scale, 5 its offset);
//! - 6: the vertex colour (0x80 each);
//! - 7..10: four GIF tags (strips and fans in the two contexts);
//! - 11..28: the cubic's weights (t³, t², t, 1) at 1/7, 1/5 and 1/3 steps (each program reads its own: 11, 19, 25);
//! - 29..: the N × N grid's (u, v, 1, 1);
//! - TOP..TOP+20: the patch (its lightmap rectangle, its four corners' texture coordinates, its 16 coefficients).
//!
//! **Where it works:**
//! - 161.. / 169..: the edges' texture coordinates;
//! - 177..: the grid's points (N a row);
//! - 245..: their texture coordinates;
//! - 93..: their lightmap coordinates (or the view pass's points);
//! - 368.. / 561..: the two strip packets (a GIF tag and three quadwords a vertex);
//! - 314.. / 341..: the clipper's two polygons;
//! - 724.. / 752..: the clipped polygons' packets (313 says which was last);
//! - 900: the return place kept by the routines.
//!
//! **The entries** (the starts `StaticInit_Render` gives the lists' kinds 1..3): 0 the patch evaluated and drawn with
//! its lightmap pass; 0x28 the same for a patch that needs clipping; 0x50 and 0x68 evaluated and drawn without the
//! lightmap pass; 0x80 and 0xe0 a view's pass (its matrix at TOP, the points put through it), unclipped and clipped.

use crate::vu::*;

/// The return place the routines keep (900.x)
const SAVE: u32 = 900;
const ONE: usize = 18;

/// One of the uniform programs (or the mixed one's inner grid): N steps (points a row), where its weights start,
/// the view transform's first result register, the clip handlers' return places (in 8 bytes), whether it clips by
/// the far plane, and where the patch starts after TOP (the mixed program has the edges' steps first)
#[derive(Clone, Copy)]
struct Steps {
    n: u16,
    weights: u16,
    first_result: usize,
    returns: (u16, u16),
    far: bool,
    patch: u32,
}
const STEPS4: Steps = Steps { n: 4, weights: 25, first_result: 10, returns: (376, 422), far: false, patch: 0 };
const STEPS6: Steps = Steps { n: 6, weights: 19, first_result: 13, returns: (394, 440), far: true, patch: 0 };
const STEPS8: Steps = Steps { n: 8, weights: 11, first_result: 13, returns: (413, 459), far: true, patch: 0 };

/// A call of the 4-step program from `start` (bytes), as the program runs it to its end
pub fn run4(v: &mut Vu, start: u32) {
    run(v, STEPS4, start);
}
/// A call of the 6-step program (0x3110f0)
pub fn run6(v: &mut Vu, start: u32) {
    run(v, STEPS6, start);
}
/// A call of the 8-step program (0x312af0)
pub fn run8(v: &mut Vu, start: u32) {
    run(v, STEPS8, start);
}

fn run(v: &mut Vu, st: Steps, start: u32) {
    // (each call's link in vi15, in 8 bytes: the place after the call's delay slot)
    let call = |v: &mut Vu, at: u32| v.vi[15] = ((at + 16) / 8) as u16;
    match start {
        0x00 => {
            call(v, 0x00);
            evaluate(v, st);
            call(v, 0x10);
            lightmap_pass(v, st);
        }
        0x28 => {
            call(v, 0x28);
            evaluate(v, st);
            call(v, 0x38);
            lightmap_pass_clipped(v, st);
        }
        0x50 | 0x68 => {
            call(v, start);
            evaluate(v, st);
        }
        0x80 => {
            view_points(v, st, 0xb8);
            call(v, 0xc8);
            lightmap_pass(v, st);
        }
        0xe0 => {
            view_points(v, st, 0x118);
            for k in 0..5 {
                v.lq(1 + k, k as u32, XYZW);
            }
            call(v, 0x150);
            v.lq(6, 5, XYZW);
            lightmap_pass_clipped(v, st);
        }
        _ => panic!("the {}-step program has no entry at {start:#x}", st.n),
    }
    // the pair after the E bit's: the next entry's first lower op
    match start {
        0x00 => call(v, 0x28),
        0x28 => call(v, 0x50),
        0x50 => call(v, 0x68),
        0x68 | 0x80 => v.vi[1] = v.top as u16,
        _ => {
            let ret = v.vi[15];
            v.isw(ret, SAVE, X);
        }
    }
}

/// 0x80 / 0xe0's start: the view's matrix at TOP, the grid's N × N points (177..) put through it to 93.. (the
/// call at `at`)
fn view_points(v: &mut Vu, st: Steps, at: u32) {
    v.vi[1] = v.top as u16;
    for k in 0..4 {
        v.lq(1 + k, v.top + k as u32, XYZW);
    }
    v.vi[2] = st.n * st.n;
    v.vi[3] = 177;
    v.vi[15] = ((at + 16) / 8) as u16;
    v.vi[4] = 93;
    transform_points(v, st);
}

/// The view transform: vi2 points from vi3.. put through the matrix vf1..vf4 into vi4.., N at a time (point k
/// in vf(5 + k), its result in the program's first result register + k), each loaded while the one before is
/// put through
fn transform_points(v: &mut Vu, st: Steps) {
    let n = st.n as usize;
    let next = |v: &mut Vu, r: usize| {
        let a = v.vi[3];
        v.lq(r, a as u32, XYZW);
        v.vi[3] = a.wrapping_add(1);
    };
    let put = |v: &mut Vu, r: usize| {
        let a = v.vi[4];
        v.sq(r, a as u32, XYZW);
        v.vi[4] = a.wrapping_add(1);
    };
    next(v, 5);
    loop {
        for k in 0..n {
            v.transform(st.first_result + k, 1, 5 + k, XYZW);
            next(v, if k + 1 < n { 6 + k } else { 5 });
            if k > 0 {
                put(v, st.first_result + k - 1);
            }
        }
        v.vi[2] = v.vi[2].wrapping_sub(st.n);
        let more = (v.vi[2] as i16) > 0;
        put(v, st.first_result + n - 1);
        if !more {
            return;
        }
    }
}

/// 0x168: the patch evaluated (its edges, then its N × N grid), the strip packets' tags set, and the strips of
/// quads drawn with the patch's texture in two passes (N/2 strips, then the other N/2 − 1)
fn evaluate(v: &mut Vu, st: Steps) {
    let n = st.n;
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.vi[1] = v.top as u16;
    v.vi[2] = n;
    v.vi[3] = st.weights;
    v.vi[15] = (0x188 + 16) / 8;
    v.vi[4] = st.weights;
    edges(v, st);
    v.vi[2] = st.weights;
    v.vi[3] = st.weights;
    v.vi[15] = (0x1a8 + 16) / 8;
    v.vi[4] = n;
    grid(v, st);
    for k in 0..6 {
        v.lq(1 + k, k as u32, XYZW);
    }
    v.lq(8, 9, XYZW);
    v.lq(7, 7, XYZW);
    v.vi[2] = 0x8000;
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[3] = 752;
    v.isw(752, 313, X);
    // the two packets' vertex counts: N/2 strips of 2N, then N/2 − 1
    v.vi[3] = n * n;
    v.vi[4] = n * n | 0x8000;
    v.isw(v.vi[4], 368, X);
    v.sq(7, 368, YZW);
    v.vi[3] = n * (n - 2);
    v.vi[4] = n * (n - 2) | 0x8000;
    v.isw(v.vi[4], 561, X);
    v.sq(7, 561, YZW);
    let half = n * n / 2;
    v.vi[12] = 368;
    strips(v, st, [177, 177 + n, 245, 245 + n, 369, n / 2], 0x298);
    v.kick(v.vi[12] as u32);
    v.vi[12] = 561;
    strips(v, st, [177 + half, 177 + half + n, 245 + half, 245 + half + n, 562, n / 2 - 1], 0x2e0);
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// The strips' set-up shared by the routines: vi2, vi3 the two rows' points, vi4, vi5 their texture coordinates,
/// vi6 the packet's vertices, vi7 how many strips (the call at `at`)
fn strips(v: &mut Vu, st: Steps, regs: [u16; 6], at: u32) {
    for (k, r) in regs.into_iter().enumerate().take(5) {
        v.vi[2 + k] = r;
    }
    v.vi[15] = ((at + 16) / 8) as u16;
    v.vi[7] = regs[5];
    strip_pass(v, st);
}

/// 0x328: the lightmap pass over the strips 0x168 drew: their tags given the second context's (8, 10), and each
/// vertex's ST made its lightmap coordinate (93..) times its Q, then the two packets sent again
fn lightmap_pass(v: &mut Vu, st: Steps) {
    let (n, half) = (st.n, st.n * st.n / 2);
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.lq(7, 8, XYZW);
    v.lq(8, 10, XYZW);
    v.sq(7, 368, YZW);
    v.sq(7, 561, YZW);
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[12] = 368;
    st_from_lightmap(v, st, [3 * n * n, 369, 93, 93 + n], 0x390);
    v.kick(v.vi[12] as u32);
    v.vi[12] = 561;
    st_from_lightmap(v, st, [3 * n * (n - 2), 562, 93 + half, 93 + half + n], 0x3c8);
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// 0x410: the lightmap pass for a patch that needs clipping: the strips drawn again (with the clipper) from the
/// lightmap coordinates
fn lightmap_pass_clipped(v: &mut Vu, st: Steps) {
    let (n, half) = (st.n, st.n * st.n / 2);
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.lq(7, 8, XYZW);
    v.lq(8, 10, XYZW);
    v.sq(7, 368, YZW);
    v.sq(7, 561, YZW);
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[12] = 368;
    strips(v, st, [177, 177 + n, 93, 93 + n, 369, n / 2], 0x488);
    v.kick(v.vi[12] as u32);
    v.vi[12] = 561;
    strips(v, st, [177 + half, 177 + half + n, 93 + half, 93 + half + n, 562, n / 2 - 1], 0x4d0);
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// 0x518: the edges' texture coordinates: for each of vi2 weights t (from vi3 and vi4), the u along the top and
/// bottom edges and the v along the left and right (the corners at TOP+1..4), as x / w and y / w pairs to vi5..
/// (161) and vi6.. (169)
fn edges(v: &mut Vu, st: Steps) {
    let top = v.vi[1] as u32 + st.patch;
    v.sub(ONE, ONE, ONE, XYZW);
    v.lq(5, top + 1, XYZW);
    v.sub(14, 14, 14, XYZW);
    v.lq(7, top + 3, XYZW);
    v.sub(15, 15, 15, XYZW);
    v.lq(6, top + 2, XYZW);
    v.lq(8, top + 4, XYZW);
    v.add_bc(ONE, ONE, 0, LW, XYZW);
    v.sub(1, 7, 5, X);
    v.sub(2, 6, 5, Y);
    v.sub(10, 8, 6, X);
    v.vi[5] = 161;
    v.sub(12, 8, 7, Y);
    v.vi[6] = 169;
    v.mul_bc(10, 0, 1, LX, W);
    v.mul_bc(12, 0, 2, LY, W);
    v.mul_bc(16, 0, 5, LX, W);
    let a = v.vi[3];
    v.lq(3, a as u32, XYZW);
    v.vi[3] = a.wrapping_add(1);
    v.mul_bc(17, 0, 5, LY, W);
    let a = v.vi[4];
    v.lq(4, a as u32, XYZW);
    v.vi[4] = a.wrapping_add(1);
    v.sub(11, 11, 11, XYZW);
    v.sub(13, 13, 13, XYZW);
    v.sub_bc(16, 6, 16, LW, X);
    v.sub_bc(17, 7, 17, LW, Y);
    v.mul_bc(14, 10, 3, LZ, W);
    v.mul_bc(14, 10, 3, LZ, X);
    v.mul_bc(15, 12, 4, LZ, W);
    v.mul_bc(15, 12, 4, LZ, Y);
    loop {
        v.add_bc(11, 16, 14, LW, W);
        let a = v.vi[3];
        v.lq(3, a as u32, XYZW);
        v.vi[3] = a.wrapping_add(1);
        v.add_bc(13, 17, 15, LW, W);
        let a = v.vi[4];
        v.lq(4, a as u32, XYZW);
        v.vi[4] = a.wrapping_add(1);
        v.suba_bc(14, 14, LW, X);
        v.vi[2] = v.vi[2].wrapping_sub(1);
        v.madd(11, ONE, 16, X);
        v.suba_bc(15, 15, LW, Y);
        v.madd(13, ONE, 17, Y);
        v.mul_bc(14, 10, 3, LZ, W);
        v.mul_bc(15, 12, 4, LZ, W);
        let a = v.vi[5];
        v.sq(11, a as u32, XYZW);
        v.vi[5] = a.wrapping_add(1);
        let more = (v.vi[2] as i16) > 0;
        v.mul_bc(14, 10, 3, LZ, X);
        v.mul_bc(15, 12, 4, LZ, Y);
        let a = v.vi[6];
        v.sq(13, a as u32, XYZW);
        v.vi[6] = a.wrapping_add(1);
        if !more {
            return;
        }
    }
}

/// The grid: for each of vi4 rows (the s weights from vi2), the coefficients (TOP+5..20) collapsed to four rows
/// (vf28..vf31) in s, then each of the row's N points their sum in t (the weights from vi3) to
/// 177..; the texture coordinates across the edges (161.., 169..) to 245..; the grid's (u, v) offset by the
/// lightmap rectangle to 93..
fn grid(v: &mut Vu, st: Steps) {
    let top = v.vi[1] as u32 + st.patch;
    let n = st.n as usize;
    let lqi = |v: &mut Vu, r: usize, ir: usize, dest: u32| {
        let a = v.vi[ir];
        v.lq(r, a as u32, dest);
        v.vi[ir] = a.wrapping_add(1);
    };
    let sqi = |v: &mut Vu, r: usize, ir: usize| {
        let a = v.vi[ir];
        v.sq(r, a as u32, XYZW);
        v.vi[ir] = a.wrapping_add(1);
    };
    lqi(v, 7, 2, XYZW);
    v.sub(5, 0, 0, XYZW);
    v.lq(12, top + 5, XYZW);
    v.sub(6, 0, 0, XYZW);
    v.lq(13, top + 6, XYZW);
    v.lq(14, top + 7, XYZW);
    v.lq(15, top + 8, XYZW);
    v.mul(10, 0, 0, W);
    v.lq(16, top + 9, XYZW);
    v.mul(11, 0, 0, W);
    v.lq(17, top + 10, XYZW);
    v.add_bc(5, 5, 0, LW, ZW);
    v.lq(18, top + 11, XYZW);
    v.add_bc(6, 6, 0, LW, ZW);
    v.lq(19, top + 12, XYZW);
    v.sub(1, 0, 0, XYZW);
    v.vi[6] = 161;
    v.vi[8] = 29;
    v.vi[9] = 177;
    v.vi[10] = 245;
    v.vi[11] = 93;
    loop {
        v.mula_bc(12, 7, LX, XYZW);
        v.lq(20, top + 13, XYZW);
        v.madda_bc(13, 7, LY, XYZW);
        v.lq(21, top + 14, XYZW);
        v.madda_bc(14, 7, LZ, XYZW);
        v.lq(22, top + 15, XYZW);
        v.madd_bc(28, 15, 7, LW, XYZW);
        v.vi[5] = 169;
        v.mula_bc(16, 7, LX, XYZW);
        v.lq(23, top + 16, XYZW);
        v.madda_bc(17, 7, LY, XYZW);
        v.lq(24, top + 17, XYZW);
        v.madda_bc(18, 7, LZ, XYZW);
        v.lq(25, top + 18, XYZW);
        v.madd_bc(29, 19, 7, LW, XYZW);
        v.lq(26, top + 19, XYZW);
        v.mula_bc(20, 7, LX, XYZW);
        v.lq(27, top + 20, XYZW);
        v.madda_bc(21, 7, LY, XYZW);
        lqi(v, 2, 6, XYZW);
        v.madda_bc(22, 7, LZ, XYZW);
        v.vi[7] = v.vi[3];
        v.madd_bc(30, 23, 7, LW, XYZW);
        lqi(v, 8, 7, XYZW);
        v.mula_bc(24, 7, LX, XYZW);
        lqi(v, 3, 5, XYZW);
        v.madda_bc(25, 7, LY, XYZW);
        v.vi[4] = v.vi[4].wrapping_sub(1);
        v.madda_bc(26, 7, LZ, XYZW);
        v.lq(20, top, XYZW);
        v.madd_bc(31, 27, 7, LW, XYZW);
        lqi(v, 21, 8, XYZW);
        // the row's N points: t weights alternating in vf8 / vf9, the coordinates in vf5 / vf6
        for k in 0..n {
            let (w, p) = if k % 2 == 0 { (8, 10) } else { (9, 11) };
            v.transform_rows(p, [28, 29, 30, 31], w, XYZ);
            match (k % 2, k + 1 == n) {
                (0, false) => {
                    v.adda_bc(1, 2, LW, X);
                    lqi(v, 9, 7, XYZW);
                    v.madd_bc(5, 2, 8, LZ, X);
                    lqi(v, 4, 5, XYZW);
                    v.adda_bc(1, 3, LW, Y);
                    lqi(v, 22, 8, XYZW);
                    v.madd_bc(5, 3, 7, LZ, Y);
                    v.add(21, 20, 21, XY);
                }
                (1, false) => {
                    v.adda_bc(1, 2, LW, X);
                    lqi(v, 8, 7, XYZW);
                    v.madd_bc(6, 2, 9, LZ, X);
                    lqi(v, 3, 5, XYZW);
                    v.adda_bc(1, 4, LW, Y);
                    lqi(v, 21, 8, XYZW);
                    v.madd_bc(6, 4, 7, LZ, Y);
                    v.add(22, 20, 22, XY);
                }
                _ => {
                    v.adda_bc(1, 2, LW, X);
                    v.madd_bc(6, 2, 9, LZ, X);
                    v.adda_bc(1, 4, LW, Y);
                    v.madd_bc(6, 4, 7, LZ, Y);
                    v.add(22, 20, 22, XY);
                    lqi(v, 7, 2, XYZW);
                }
            }
            sqi(v, p, 9);
            sqi(v, if k % 2 == 0 { 5 } else { 6 }, 10);
            if k + 1 == n {
                let more = (v.vi[4] as i16) > 0;
                sqi(v, 22, 11);
                if !more {
                    return;
                }
            } else {
                sqi(v, if k % 2 == 0 { 21 } else { 22 }, 11);
            }
        }
    }
}

/// The lightmap rewrite: vi2 / 3 vertices of the packet at vi3: each one's ST made its lightmap coordinate (from
/// vi4.. for a strip's A row and vi5.. for its B row) times its Q (the ST's w). The vertices go in pairs (A, B),
/// a strip's N pairs a turn: pair j in vf7/8, vf9/10, then vf11/12, vf13/14, vf15/16 in turn, each pair two on
/// loaded while it is made, the packet's place moved on after the strip's third pair from last.
fn st_from_lightmap(v: &mut Vu, st: Steps, regs: [u16; 4], at: u32) {
    for (k, r) in regs.into_iter().enumerate().take(3) {
        v.vi[2 + k] = r;
    }
    v.vi[15] = ((at + 16) / 8) as u16;
    v.vi[5] = regs[3];
    let n = st.n as usize;
    let reg = |j: usize| match j {
        0 => 7,
        1 => 9,
        _ => [11, 13, 15][(j - 2) % 3],
    };
    let lqi_xy = |v: &mut Vu, r: usize, ir: usize| {
        let a = v.vi[ir];
        v.lq(r, a as u32, XY);
        v.vi[ir] = a.wrapping_add(1);
    };
    let b = |v: &Vu, off: i32| (v.vi[3] as i32 + off) as u32;
    for j in 0..2 {
        lqi_xy(v, reg(j), 4);
        v.lq(reg(j), b(v, 6 * j as i32), W);
        lqi_xy(v, reg(j) + 1, 5);
        v.lq(reg(j) + 1, b(v, 6 * j as i32 + 3), W);
    }
    v.vi[2] = v.vi[2].wrapping_add(v.vi[3]);
    loop {
        // (the place moves on after pair n − 3: later pairs' offsets are from the next strip)
        let mut base = 0i32;
        for j in 0..n {
            let (ta, tb) = if j % 2 == 0 { (20, 21) } else { (22, 23) };
            let (nj, noff) = if j + 2 < n { (reg(j + 2), 6 * (j + 2) as i32 + base) } else { (reg(j + 2 - n), 6 * (j + 2 - n) as i32) };
            let r = reg(j);
            v.mul_bc(ta, r, r, LW, XY);
            lqi_xy(v, nj, 4);
            v.mul_bc(tb, r + 1, r + 1, LW, XY);
            v.lq(nj, b(v, noff), W);
            lqi_xy(v, nj + 1, 5);
            v.lq(nj + 1, b(v, noff + 3), W);
            v.sq(ta, b(v, 6 * j as i32 + base), XY);
            if j + 1 == n {
                let more = v.vi[3] != v.vi[2];
                v.sq(tb, b(v, 6 * j as i32 + base + 3), XY);
                if !more {
                    return;
                }
            } else {
                v.sq(tb, b(v, 6 * j as i32 + base + 3), XY);
            }
            if j + 3 == n {
                v.vi[3] = v.vi[3].wrapping_add(6 * st.n);
                base = -6 * n as i32;
            }
        }
    }
}

/// 0x868: vi7 strips of quads between two rows of four points (vi2 / vi3, their texture coordinates vi4 / vi5) to
/// the packet at vi6, three quadwords a vertex (ST, the colour, the screen place with the GS's no-draw bit for the
/// first two of a strip); each triangle that crosses the frustum not drawn in the strip but clipped and sent on
/// its own
fn strip_pass(v: &mut Vu, st: Steps) {
    let lqi = |v: &mut Vu, r: usize, ir: usize| {
        let a = v.vi[ir];
        v.lq(r, a as u32, XYZW);
        v.vi[ir] = a.wrapping_add(1);
    };
    let sqi = |v: &mut Vu, r: usize| {
        let a = v.vi[6];
        v.sq(r, a as u32, XYZW);
        v.vi[6] = a.wrapping_add(1);
    };
    lqi(v, 7, 2);
    lqi(v, 8, 3);
    lqi(v, 9, 4);
    lqi(v, 10, 5);
    v.mula_bc(1, 7, LX, XYZW);
    v.lq(17, 6, XYZW);
    v.madda_bc(2, 7, LY, XYZW);
    v.vi[9] = 0xffff;
    v.madda_bc(3, 7, LZ, XYZW);
    v.madd_bc(11, 4, 7, LW, XYZW);
    v.transform(12, 1, 8, XYZW);
    v.div(0, LW, 11, LW);
    v.sub(ONE, ONE, ONE, XYZW);
    v.add_bc(ONE, ONE, 0, LW, XYZW);
    loop {
        // the strip's first pair (A, B): drawn with the no-draw bit
        v.mul_q(13, 11, XYZW);
        lqi(v, 7, 2);
        v.mul_q(14, 9, XYZW);
        v.clipw(11, 11);
        v.div(0, LW, 12, LW);
        v.mula(ONE, 6, XYZW);
        v.madd(13, 13, 5, XYZW);
        v.mula_bc(1, 7, LX, XYZW);
        v.madda_bc(2, 7, LY, XYZW);
        lqi(v, 9, 4);
        v.madda_bc(3, 7, LZ, XYZW);
        v.ftoi4(16, 13, XYZ);
        v.madd_bc(11, 4, 7, LW, XYZW);
        v.mfir(16, 9, W);
        v.mul_q(13, 12, XYZW);
        lqi(v, 8, 3);
        v.mul_q(15, 10, XYZW);
        sqi(v, 14);
        v.clipw(12, 12);
        sqi(v, 17);
        v.mula(ONE, 6, XYZW);
        v.div(0, LW, 11, LW);
        v.madd(13, 13, 5, XYZW);
        sqi(v, 16);
        v.mula_bc(1, 8, LX, XYZW);
        sqi(v, 15);
        v.madda_bc(2, 8, LY, XYZW);
        lqi(v, 10, 5);
        v.madda_bc(3, 8, LZ, XYZW);
        sqi(v, 17);
        v.ftoi4(16, 13, XYZ);
        v.madd_bc(12, 4, 8, LW, XYZW);
        v.vi[7] = v.vi[7].wrapping_sub(1);
        v.vi[8] = st.n - 1;
        sqi(v, 16);
        // the strip's other three pairs: each new A and new B a triangle tested against the frustum
        loop {
            v.mul_q(13, 11, XYZW);
            lqi(v, 7, 2);
            v.mul_q(14, 9, XYZW);
            v.clipw(11, 11);
            v.div(0, LW, 12, LW);
            v.mula(ONE, 6, XYZW);
            v.madd(13, 13, 5, XYZW);
            v.mula_bc(1, 7, LX, XYZW);
            v.vi[8] = v.vi[8].wrapping_sub(1);
            v.madda_bc(2, 7, LY, XYZW);
            lqi(v, 9, 4);
            v.madda_bc(3, 7, LZ, XYZW);
            let clipped = v.fcand(0x3ffff);
            v.ftoi4(16, 13, XYZW);
            v.madd_bc(11, 4, 7, LW, XYZW);
            if clipped {
                clip_triangle(v, st, false);
            }
            v.mul_q(13, 12, XYZW);
            lqi(v, 8, 3);
            v.mul_q(15, 10, XYZW);
            sqi(v, 14);
            v.clipw(12, 12);
            v.div(0, LW, 11, LW);
            v.mula(ONE, 6, XYZW);
            sqi(v, 17);
            v.madd(13, 13, 5, XYZW);
            sqi(v, 16);
            v.mula_bc(1, 8, LX, XYZW);
            sqi(v, 15);
            v.madda_bc(2, 8, LY, XYZW);
            lqi(v, 10, 5);
            v.madda_bc(3, 8, LZ, XYZW);
            let clipped = v.fcand(0x3ffff);
            v.ftoi4(16, 13, XYZW);
            v.madd_bc(12, 4, 8, LW, XYZW);
            if clipped {
                clip_triangle(v, st, true);
            }
            sqi(v, 17);
            let more = (v.vi[8] as i16) > 0;
            sqi(v, 16);
            if !more {
                break;
            }
        }
        if (v.vi[7] as i16) <= 0 {
            return;
        }
    }
}

/// The clip handlers: a strip triangle that touches a clip plane. Its vertex kept from the strip (the no-draw bit
/// set); unless one of its points is past the far plane (4 steps; for 6 and 8 all three), or all three are
/// outside one plane, the triangle (its points
/// put through the matrix again, their texture coordinates and the colour) clipped and sent (0xf60), then Q and
/// the last two clip tests made again for the strip. `second` for the triangle a new B closes.
fn clip_triangle(v: &mut Vu, st: Steps, second: bool) {
    v.mfir(16, 9, W);
    // the six checks, each branch's delay slot the next one (so a check that sends the triangle back still has
    // the next one made)
    let check = |v: &mut Vu, k: usize| match k {
        0 if st.far => v.fcor(0xfe_fbef),
        0 => v.fcand(0x10410),
        _ => v.fcor([0xfd_f7df, 0xff_7df7, 0xff_befb, 0xff_df7d, 0xff_efbe][k - 1]),
    };
    for k in 0..6 {
        if check(v, k) {
            if k < 5 {
                check(v, k + 1);
            }
            return;
        }
    }
    // the triangle's points (positions / texture coordinates): A rows vi2 / vi4, B rows vi3 / vi5
    let (pa, ta, pb, tb) = if second { (3, 5, 2, 4) } else { (2, 4, 3, 5) };
    let at = |v: &Vu, r: usize, off: i32| (v.vi[r] as i32 + off) as u32;
    let (p0, t0, p1, t1, p2, t2) = (at(v, pa, -3), at(v, ta, -3), at(v, pb, -2), at(v, tb, -2), at(v, pa, -2), at(v, ta, -2));
    v.lq(20, p0, XYZW);
    v.lq(23, t0, XYZW);
    v.lq(21, p1, XYZW);
    v.lq(24, t1, XYZW);
    v.mula_bc(1, 20, LX, XYZW);
    v.lq(22, p2, XYZW);
    v.madda_bc(2, 20, LY, XYZW);
    v.lq(25, t2, XYZW);
    v.madda_bc(3, 20, LZ, XYZW);
    v.madd_bc(29, 4, 20, LW, XYZW);
    v.mfir(19, 9, X);
    v.mula_bc(1, 21, LX, XYZW);
    v.mfir(19, 15, Y);
    v.madda_bc(2, 21, LY, XYZW);
    v.mfir(19, 12, Z);
    v.madda_bc(3, 21, LZ, XYZW);
    v.vi[10] = 314;
    v.madd_bc(30, 4, 21, LW, XYZW);
    v.vi[15] = 9;
    v.itof0(26, 17, XYZW);
    v.transform(31, 1, 22, XYZW);
    for (k, r) in [29, 23, 26, 30, 24, 26, 31, 25, 26].into_iter().enumerate() {
        v.sq(r, 314 + k as u32, XYZW);
    }
    v.vi[14] = if second { st.returns.1 } else { st.returns.0 };
    clip_and_send(v, st);
    if second {
        v.div(0, LW, 11, LW);
    } else {
        v.div(0, LW, 12, LW);
    }
    v.clipw(30, 30);
    v.clipw(31, 31);
    v.mtir(9, 19, LX);
    v.mtir(15, 19, LY);
    v.mtir(12, 19, LZ);
}

/// The planes the clipper cuts by, in its order: the near (z ≥ −w), then x ≥ −w, x ≤ w, y ≤ w, y ≥ −w, and (for 6
/// and 8 steps) the far, z ≤ w. For each:
/// the lane; whether the plane is +w (the lane minus w, the cut point the earlier one plus the edge times the
/// fraction) or −w (plus w, minus); the tests' flags for both points out, either out, the earlier out
const PLANES: [(usize, bool, u32, u32, u32); 6] = [
    (LZ, false, 0xff_f7df, 0x820, 0x800),
    (LX, false, 0xff_ff7d, 0x82, 0x80),
    (LX, true, 0xff_ffbe, 0x41, 0x40),
    (LY, true, 0xff_fefb, 0x104, 0x100),
    (LY, false, 0xff_fdf7, 0x208, 0x200),
    (LZ, true, 0xff_fbef, 0x410, 0x400),
];

/// 0xf60: the polygon at 314 (vi15 quadwords, three a point: its clip-frame position, texture coordinates, colour)
/// cut by each plane in turn, from 314 to 341 and back; what is left sent. The polygon's places are
/// kept in vi9..vi13 as the program keeps them: vi12 this point, vi11 the one before, vi13 the quadwords left,
/// vi10 where the next point out goes, vi9 the edge's first one out.
fn clip_and_send(v: &mut Vu, st: Steps) {
    let (mut from, mut to) = (314u16, 341u16);
    let planes = if st.far { 6 } else { 5 };
    for (k, &(lane, plus, both_out, either_out, prev_out)) in PLANES.iter().take(planes).enumerate() {
        if k == 0 {
            v.vi[12] = from;
            v.vi[11] = from + 6;
            v.lq(20, v.vi[11] as u32, XYZW);
            v.lq(21, v.vi[12] as u32, XYZW);
            v.vi[13] = v.vi[15];
            v.vi[10] = to;
            v.clipw(20, 20);
        } else {
            v.vi[11] = v.vi[10].wrapping_sub(3);
            v.vi[12] = from;
            v.lq(20, v.vi[11] as u32, XYZW);
            v.lq(21, v.vi[12] as u32, XYZW);
            v.vi[13] = v.vi[10].wrapping_sub(from);
            v.vi[10] = to;
            v.clipw(20, 20);
            if v.vi[13] == 0 {
                // (the delay slot's test and load before the return)
                v.clipw(21, 21);
                v.lq(26, v.vi[12] as u32 + 1, XYZW);
                return;
            }
        }
        loop {
            let (cur, prev) = (v.vi[12] as u32, v.vi[11] as u32);
            v.clipw(21, 21);
            v.lq(26, cur + 1, XYZW);
            v.lq(24, prev + 1, XYZW);
            v.lq(28, cur + 2, XYZW);
            v.lq(29, prev + 2, XYZW);
            v.sub(22, 20, 21, XYZW);
            v.vi[9] = v.vi[10];
            // f, the lane minus or plus w, for the earlier point (vf23) and this one (vf25)
            if plus {
                v.sub_bc(23, 20, 20, LW, 8 >> lane);
                v.sub_bc(25, 21, 21, LW, 8 >> lane);
            } else {
                v.add_bc(23, 20, 20, LW, 8 >> lane);
                v.add_bc(25, 21, 21, LW, 8 >> lane);
            }
            let both = v.fcor(both_out);
            // (the delay slot, made either way)
            v.mul(26, ONE, ONE, ZW);
            let either = v.fcand(either_out);
            if !both {
                // the edge's f (vf27.w)
                if plus {
                    v.sub_bc(27, 22, 22, lane, W);
                } else {
                    v.add_bc(27, 22, 22, lane, W);
                }
                // (the "both in" branch's delay slot: the earlier point's test, made either way)
                let prev_outside = v.fcand(prev_out);
                if either {
                    if prev_outside {
                        // the earlier point out: the cut point, then this one (kept one place on)
                        v.sub(29, 29, 28, XYZW);
                        v.div(25, lane, 27, LW);
                        v.sub(24, 24, 26, XY);
                        v.sq(21, v.vi[10] as u32 + 3, XYZW);
                        v.mula_bc(21, 0, LW, XYZW);
                        v.vi[10] = v.vi[10].wrapping_add(3);
                    } else {
                        // this point out: the cut point only
                        v.div(23, lane, 27, LW);
                        v.sub(29, 29, 28, XYZW);
                        v.lq(28, prev + 2, XYZW);
                        v.mula_bc(20, 0, LW, XYZW);
                        v.sub(24, 24, 26, XY);
                        v.lq(26, prev + 1, XYZW);
                    }
                    let out = v.vi[10] as u32;
                    let cut = |v: &mut Vu, fd: usize, fs: usize, dest: u32| if plus { v.madd_q(fd, fs, dest) } else { v.msub_q(fd, fs, dest) };
                    cut(v, 21, 22, XYZW);
                    v.mula_bc(26, 0, LW, XYZW);
                    v.sq(26, out + 1, XYZW);
                    cut(v, 26, 24, XY);
                    v.mula_bc(28, 0, LW, XYZW);
                    v.sq(28, out + 2, XYZW);
                    cut(v, 28, 29, XYZW);
                }
                v.vi[10] = v.vi[10].wrapping_add(3);
                for r in [21, 26, 28] {
                    let a = v.vi[9];
                    v.sq(r, a as u32, XYZW);
                    v.vi[9] = a.wrapping_add(1);
                }
            }
            v.lq(20, cur, XYZW);
            v.lq(21, cur + 3, XYZW);
            v.vi[13] = v.vi[13].wrapping_sub(3);
            v.vi[11] = v.vi[12];
            v.vi[12] = v.vi[12].wrapping_add(3);
            if v.vi[13] == 0 {
                break;
            }
        }
        std::mem::swap(&mut from, &mut to);
    }
    send_polygon(v, from);
}

/// The clipped polygon at `at` (341 after five planes, 314 after six; to vi10) divided by w and put on the screen
/// (ST its texture coordinates
/// times Q, the colour made integers), as a GIF packet from 724 or 752 (not the one 313 says was used last,
/// which then says this one), and sent
fn send_polygon(v: &mut Vu, at: u16) {
    v.vi[9] = v.ilw(313, LX);
    v.vi[11] = v.vi[10].wrapping_sub(at);
    if v.vi[11] == 0 {
        return;
    }
    v.vi[13] = v.vi[9].wrapping_sub(724);
    v.vi[13] = if v.vi[13] == 0 { 752 } else { 724 };
    v.isw(v.vi[13], 313, X);
    v.vi[11] = at;
    v.vi[9] = v.vi[13] + 1;
    let p = |v: &Vu| v.vi[11] as u32;
    v.lq(20, p(v), XYZW);
    v.lq(21, p(v) + 1, XYZW);
    v.lq(28, p(v) + 2, XYZW);
    v.lq(22, p(v) + 3, XYZW);
    v.div(0, LW, 20, LW);
    v.ftoi0(29, 28, XYZW);
    v.mul_q(23, 20, XYZW);
    v.mul_q(24, 21, XYZW);
    v.vi[12] = 0;
    let sqi = |v: &mut Vu, r: usize| {
        let a = v.vi[9];
        v.sq(r, a as u32, XYZW);
        v.vi[9] = a.wrapping_add(1);
    };
    loop {
        v.mula(ONE, 6, XYZW);
        v.lq(21, p(v) + 4, XYZW);
        v.madd(25, 23, 5, XYZW);
        v.div(0, LW, 22, LW);
        v.lq(28, p(v) + 5, XYZW);
        v.vf[20] = v.vf[22];
        v.vi[11] = v.vi[11].wrapping_add(3);
        v.lq(22, p(v) + 3, XYZW);
        v.ftoi4(26, 25, XYZW);
        v.vi[12] = v.vi[12].wrapping_add(1);
        sqi(v, 24);
        // (the colour stored before its next one is made)
        sqi(v, 29);
        v.ftoi0(29, 28, XYZW);
        v.mul_q(23, 20, XYZW);
        let more = v.vi[11] != v.vi[10];
        v.mul_q(24, 21, XYZW);
        sqi(v, 26);
        if !more {
            break;
        }
    }
    v.vi[11] = 0x8000 | v.vi[12];
    v.isw(v.vi[11], v.vi[13] as u32, X);
    v.kick(v.vi[13] as u32);
}

// ---- The mixed program (row G7a4e: the chain at 0x314620, 1853 instructions) ----
//
// The Bezier lists' kind 4: patches whose edges have different steps. TOP+0 holds the four edges' steps (4, 6 or
// 8; the edges at s = 0, s = 1, t = 0, t = 1); the patch follows at TOP+1. The inside is a 6 × 6 grid at 1/7 ..
// 6/7 (the "inside of 8" table), drawn as the 6-step program draws (its strip pass, handlers and clipper, the
// same code); each edge's points are evaluated at its own steps (213.., 221.., 229.., 237.., 8 apart; their
// texture coordinates 68 on, lightmap coordinates at 129.., 137.., 145.., 153..) and zipped to the grid's border
// row (the "zipper") into the second packet.

/// The mixed program's inner grid: as the 6-step program's, the patch at TOP+1, its own return places
const MIXED: Steps = Steps { n: 6, weights: 12, first_result: 13, returns: (687, 733), far: true, patch: 1 };

/// A call of the mixed program (0x314620)
pub fn run_mixed(v: &mut Vu, start: u32) {
    let st = MIXED;
    let call = |v: &mut Vu, at: u32| v.vi[15] = ((at + 16) / 8) as u16;
    match start {
        0x00 => {
            call(v, 0x00);
            evaluate_mixed(v);
            call(v, 0x10);
            lightmap_pass_mixed(v);
        }
        0x28 => {
            call(v, 0x28);
            evaluate_mixed(v);
            call(v, 0x38);
            lightmap_pass_mixed_clipped(v);
        }
        0x50 | 0x68 => {
            call(v, start);
            evaluate_mixed(v);
        }
        0x80 | 0x180 => {
            let base = start;
            view_points_mixed(v, st, base);
            if start == 0x80 {
                call(v, 0x168);
                lightmap_pass_mixed(v);
            } else {
                for k in 0..5 {
                    v.lq(1 + k, k as u32, XYZW);
                }
                call(v, 0x290);
                v.lq(6, 5, XYZW);
                lightmap_pass_mixed_clipped(v);
            }
        }
        _ => panic!("the mixed program has no entry at {start:#x}"),
    }
    // the pair after the E bit's
    match start {
        0x00 => call(v, 0x28),
        0x28 => call(v, 0x50),
        0x50 => call(v, 0x68),
        0x68 | 0x80 => v.vi[1] = v.top as u16,
        _ => {
            // 0x180: the next is 0x180's own start's neighbour, the evaluation's first pair
            let ret = v.vi[15];
            v.isw(ret, SAVE, X);
        }
    }
}

/// The view pass's points: the inner 36 through the view's matrix (at TOP+1) to 93.., then each edge's (its steps
/// from TOP+0) to 129.. + 8e
fn view_points_mixed(v: &mut Vu, st: Steps, base: u32) {
    v.vi[1] = v.top as u16;
    for k in 0..4 {
        v.lq(1 + k, v.top + 1 + k as u32, XYZW);
    }
    v.vi[2] = 36;
    v.vi[3] = 177;
    v.vi[15] = ((base + 0x38 + 16) / 8) as u16;
    v.vi[4] = 93;
    transform_points(v, st);
    let top = v.top;
    for (k, r) in [10usize, 11, 12, 13].into_iter().enumerate() {
        v.vi[r] = v.ilw(top, k);
    }
    for e in 0..4u32 {
        v.vi[2] = v.vi[10 + e as usize];
        v.vi[3] = (213 + 8 * e) as u16;
        v.vi[15] = ((base + 0x78 + 0x20 * e + 16) / 8) as u16;
        v.vi[4] = (129 + 8 * e) as u16;
        edge_points_through(v);
    }
}

/// 0x19b8: vi2 edge points from vi3.. through the matrix vf1..vf4 to vi4.., two at a time
fn edge_points_through(v: &mut Vu) {
    let next = |v: &mut Vu, r: usize| {
        let a = v.vi[3];
        v.lq(r, a as u32, XYZW);
        v.vi[3] = a.wrapping_add(1);
    };
    let put = |v: &mut Vu, r: usize| {
        let a = v.vi[4];
        v.sq(r, a as u32, XYZW);
        v.vi[4] = a.wrapping_add(1);
    };
    next(v, 5);
    loop {
        v.transform(13, 1, 5, XYZW);
        next(v, 6);
        v.transform(14, 1, 6, XYZW);
        next(v, 5);
        put(v, 13);
        v.vi[2] = v.vi[2].wrapping_sub(2);
        let more = (v.vi[2] as i16) > 0;
        put(v, 14);
        if !more {
            return;
        }
    }
}

/// 0x2a8: the mixed patch evaluated (the inner grid's edges and grid, then each edge's points), the packets' tags
/// set (the inner five strips' 60 vertices; the edges' 24 + the steps' sum), and drawn with the patch's texture:
/// the inner strips, then the edges zipped to them
fn evaluate_mixed(v: &mut Vu) {
    let st = MIXED;
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.vi[1] = v.top as u16;
    v.vi[2] = 6;
    v.vi[3] = 12;
    v.vi[15] = (0x2c8 + 16) / 8;
    v.vi[4] = 12;
    edges(v, st);
    v.vi[2] = 12;
    v.vi[3] = 12;
    v.vi[15] = (0x2e8 + 16) / 8;
    v.vi[4] = 6;
    grid(v, st);
    v.vi[15] = (0x2f8 + 16) / 8;
    edge_points(v);
    for k in 0..6 {
        v.lq(1 + k, k as u32, XYZW);
    }
    v.lq(8, 9, XYZW);
    v.lq(7, 7, XYZW);
    v.vi[2] = 0x8000;
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[3] = 752;
    v.isw(752, 313, X);
    v.vi[3] = 60;
    v.vi[4] = 60 | 0x8000;
    v.isw(v.vi[4], 368, X);
    v.sq(7, 368, YZW);
    let top = v.top;
    for (k, r) in [10usize, 11, 12, 13].into_iter().enumerate() {
        v.vi[r] = v.ilw(top, k);
    }
    v.vi[14] = v.vi[10].wrapping_add(v.vi[11]);
    v.vi[15] = v.vi[12].wrapping_add(v.vi[13]);
    v.vi[3] = v.vi[14].wrapping_add(v.vi[15]);
    v.vi[3] = v.vi[3].wrapping_add(24);
    v.vi[4] = v.vi[3] | v.vi[2];
    v.isw(v.vi[4], 561, X);
    v.sq(7, 561, YZW);
    v.vi[12] = 368;
    strips(v, st, [177, 183, 245, 251, 369, 5], 0x430);
    v.kick(v.vi[12] as u32);
    v.vi[15] = (0x448 + 16) / 8;
    v.vi[12] = 562;
    zip_edges(v, false);
    v.vi[12] = 561;
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// 0x498: the mixed patch's lightmap pass: the inner strips' ST from the lightmap coordinates (as the 6-step
/// program's), then the edges' (0x1a48)
fn lightmap_pass_mixed(v: &mut Vu) {
    let st = MIXED;
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.lq(7, 8, XYZW);
    v.lq(8, 10, XYZW);
    v.sq(7, 368, YZW);
    v.sq(7, 561, YZW);
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[12] = 368;
    st_from_lightmap(v, st, [180, 369, 93, 99], 0x500);
    v.kick(v.vi[12] as u32);
    v.vi[15] = (0x518 + 16) / 8;
    v.vi[12] = 562;
    edges_st_from_lightmap(v);
    v.vi[12] = 561;
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// 0x568: the lightmap pass for a mixed patch that needs clipping: the inner strips drawn again from the
/// lightmap coordinates, then the edges zipped again from theirs
fn lightmap_pass_mixed_clipped(v: &mut Vu) {
    let st = MIXED;
    let ret = v.vi[15];
    v.isw(ret, SAVE, X);
    v.lq(7, 8, XYZW);
    v.lq(8, 10, XYZW);
    v.sq(7, 368, YZW);
    v.sq(7, 561, YZW);
    v.sq(8, 724, YZW);
    v.sq(8, 752, YZW);
    v.vi[12] = 368;
    strips(v, st, [177, 183, 93, 99, 369, 5], 0x5e0);
    v.kick(v.vi[12] as u32);
    v.vi[15] = (0x5f8 + 16) / 8;
    v.vi[12] = 562;
    zip_edges(v, true);
    v.vi[12] = 561;
    v.kick(v.vi[12] as u32);
    v.vi[15] = v.ilw(SAVE, LX);
}

/// The weights for an edge's steps (as the program picks them, each branch's delay slot setting the next): 8 at
/// 11 (1/7), 6 at 19 (1/5), anything else at 25 (1/3)
fn weights_for(steps: u16) -> u16 {
    if steps == 8 {
        11
    } else if steps == 6 {
        19
    } else {
        25
    }
}

/// 0xa28: each edge's points at its own steps: the 16 coefficients in vf1..vf16 (TOP+6..21), the lightmap
/// rectangle turned (w, h, x, y) in vf24; for each edge its weights' place in vi2 and its steps in vi3, then the
/// edge's routine (its points to 213.., its texture coordinates 68 on, its lightmap coordinates to 129..)
fn edge_points(v: &mut Vu) {
    let top = v.vi[1] as u32;
    v.sub(22, 22, 22, XYZW);
    v.lq(1, top + 6, XYZW);
    v.sub(23, 23, 23, XYZW);
    v.lq(2, top + 7, XYZW);
    v.mul_bc(21, 0, 0, LW, XYZW);
    v.lq(3, top + 8, XYZW);
    v.lq(4, top + 9, XYZW);
    v.add_bc(22, 22, 0, LW, ZW);
    v.lq(5, top + 10, XYZW);
    v.add_bc(23, 23, 0, LW, ZW);
    v.lq(6, top + 11, XYZW);
    v.lq(7, top + 12, XYZW);
    v.lq(8, top + 13, XYZW);
    v.lq(24, top + 1, XYZW);
    v.lq(9, top + 14, XYZW);
    v.lq(10, top + 15, XYZW);
    v.lq(11, top + 16, XYZW);
    v.lq(12, top + 17, XYZW);
    let r = v.vf[24];
    v.set(24, XYZW, [r[1], r[2], r[3], r[0]]);
    v.mul_bc(21, 0, 0, LW, XYZW);
    v.lq(13, top + 18, XYZW);
    v.lq(14, top + 19, XYZW);
    v.lq(15, top + 20, XYZW);
    v.lq(16, top + 21, XYZW);
    let r = v.vf[24];
    v.set(24, XYZW, [r[1], r[2], r[3], r[0]]);
    for (k, r) in [10usize, 11, 12, 13].into_iter().enumerate() {
        v.vi[r] = v.ilw(top, k);
    }
    for e in 0..4usize {
        let steps = v.vi[10 + e];
        v.vi[4] = steps.wrapping_sub(8);
        v.vi[5] = steps.wrapping_sub(6);
        v.vi[2] = weights_for(steps);
        v.vi[14] = ((0xb18 + 0x48 * e as u32 + 16) / 8) as u16;
        v.vi[3] = steps;
        match e {
            0 => edge0(v),
            1 => edge1(v),
            2 => edge2(v),
            _ => edge3(v),
        }
    }
}

/// An edge's point stored: its place, texture coordinates and lightmap coordinates (vi4, vi5, vi6 on one each)
fn put_edge_point(v: &mut Vu) {
    for (r, ir) in [(21, 4), (22, 5), (23, 6)] {
        let a = v.vi[ir];
        v.sq(r, a as u32, XYZW);
        v.vi[ir] = a.wrapping_add(1);
    }
}
fn lqi(v: &mut Vu, r: usize, ir: usize) {
    let a = v.vi[ir];
    v.lq(r, a as u32, XYZW);
    v.vi[ir] = a.wrapping_add(1);
}

/// The edges at s = 0 and s = 1 share their points' loop (0xca0): each point the four collapsed rows (vf17..vf20)
/// summed in t (vf30 / vf31 in turn), its texture coordinates along the edge's corners (vf28 + vf29·t) and its
/// lightmap y (vf23.y = y + h·t; x as the edge's start left it); two points a turn, the one before stored
fn edge_loop_s(v: &mut Vu) {
    loop {
        v.mula_bc(17, 31, LX, XYZ);
        lqi(v, 30, 2);
        v.madda_bc(18, 31, LY, XYZ);
        let a = v.vi[4];
        v.sq(21, a as u32, XYZW);
        v.vi[4] = a.wrapping_add(1);
        v.madda_bc(19, 31, LZ, XYZ);
        let a = v.vi[5];
        v.sq(22, a as u32, XYZW);
        v.vi[5] = a.wrapping_add(1);
        v.madd_bc(21, 20, 31, LW, XYZ);
        let a = v.vi[6];
        v.sq(23, a as u32, XYZW);
        v.vi[6] = a.wrapping_add(1);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, 31, LZ, XY);
        v.vi[3] = v.vi[3].wrapping_sub(2);
        v.adda_bc(0, 24, LW, Y);
        v.madd_bc(23, 24, 31, LZ, Y);
        v.mula_bc(17, 30, LX, XYZ);
        lqi(v, 31, 2);
        v.madda_bc(18, 30, LY, XYZ);
        let a = v.vi[4];
        v.sq(21, a as u32, XYZW);
        v.vi[4] = a.wrapping_add(1);
        v.madda_bc(19, 30, LZ, XYZ);
        let a = v.vi[5];
        v.sq(22, a as u32, XYZW);
        v.vi[5] = a.wrapping_add(1);
        v.madd_bc(21, 20, 30, LW, XYZ);
        let a = v.vi[6];
        v.sq(23, a as u32, XYZW);
        v.vi[6] = a.wrapping_add(1);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, 30, LZ, XY);
        v.adda_bc(0, 24, LW, Y);
        let more = (v.vi[3] as i16) > 0;
        v.madd_bc(23, 24, 30, LZ, Y);
        if !more {
            return;
        }
    }
}

/// 0xc10: the edge at s = 0: the rows c3, c7, c11, c15 (each × 1); the corners' texture coordinates at TOP+2 and
/// TOP+3; the lightmap x = x
fn edge0(v: &mut Vu) {
    let top = v.vi[1] as u32;
    v.add_bc(23, 0, 24, LZ, X);
    v.lq(28, top + 2, XYZW);
    v.lq(29, top + 3, XYZW);
    lqi(v, 30, 2);
    v.vi[4] = 213;
    v.vi[5] = 281;
    v.sub(29, 29, 28, XYZW);
    v.vi[6] = 129;
    v.mul_bc(17, 4, 0, LW, XYZW);
    v.mul_bc(18, 8, 0, LW, XYZW);
    v.mul_bc(19, 12, 0, LW, XYZW);
    v.mul_bc(20, 16, 0, LW, XYZW);
    v.mula_bc(17, 30, LX, XYZ);
    lqi(v, 31, 2);
    v.madda_bc(18, 30, LY, XYZ);
    v.madda_bc(19, 30, LZ, XYZ);
    v.madd_bc(21, 20, 30, LW, XYZ);
    v.mula_bc(28, 0, LW, XY);
    v.madd_bc(22, 29, 30, LZ, XY);
    v.adda_bc(0, 24, LW, Y);
    v.madd_bc(23, 24, 30, LZ, Y);
    edge_loop_s(v);
}

/// 0xd30: the edge at s = 1: the rows summed (c0 + c1 + c2 + c3, and the other three fours); the corners at TOP+4
/// and TOP+5; the lightmap x = x + w. Its first turn is its own, the rest edge 0's loop.
fn edge1(v: &mut Vu) {
    let top = v.vi[1] as u32;
    v.mula_bc(1, 0, LW, XYZW);
    v.lq(28, top + 4, XYZW);
    v.madda_bc(2, 0, LW, XYZW);
    v.lq(29, top + 5, XYZW);
    v.madda_bc(3, 0, LW, XYZW);
    lqi(v, 30, 2);
    v.madd_bc(17, 4, 0, LW, XYZW);
    v.vi[4] = 221;
    v.vi[5] = 289;
    v.sub(29, 29, 28, XYZW);
    v.vi[6] = 137;
    for (row, out) in [(5usize, 18usize), (9, 19), (13, 20)] {
        v.mula_bc(row, 0, LW, XYZW);
        v.madda_bc(row + 1, 0, LW, XYZW);
        v.madda_bc(row + 2, 0, LW, XYZW);
        v.madd_bc(out, row + 3, 0, LW, XYZW);
    }
    v.mula_bc(17, 30, LX, XYZ);
    lqi(v, 31, 2);
    v.madda_bc(18, 30, LY, XYZ);
    v.madda_bc(19, 30, LZ, XYZ);
    v.madd_bc(21, 20, 30, LW, XYZ);
    v.mula_bc(28, 0, LW, XY);
    v.madd_bc(22, 29, 30, LZ, XY);
    v.adda_bc(0, 24, LZ, X);
    v.madd_bc(23, 24, 0, LW, X);
    v.adda_bc(0, 24, LW, Y);
    v.madd_bc(23, 24, 30, LZ, Y);
    // its own first turn (0xe10), then edge 0's (0xca0)
    v.mula_bc(17, 31, LX, XYZ);
    lqi(v, 30, 2);
    v.madda_bc(18, 31, LY, XYZ);
    put_one(v, 21, 4);
    v.madda_bc(19, 31, LZ, XYZ);
    put_one(v, 22, 5);
    v.madd_bc(21, 20, 31, LW, XYZ);
    put_one(v, 23, 6);
    v.mula_bc(28, 0, LW, XY);
    v.madd_bc(22, 29, 31, LZ, XY);
    v.vi[3] = v.vi[3].wrapping_sub(2);
    v.adda_bc(0, 24, LW, Y);
    v.madd_bc(23, 24, 31, LZ, Y);
    v.mula_bc(17, 30, LX, XYZ);
    lqi(v, 31, 2);
    v.madda_bc(18, 30, LY, XYZ);
    put_one(v, 21, 4);
    v.madda_bc(19, 30, LZ, XYZ);
    put_one(v, 22, 5);
    v.madd_bc(21, 20, 30, LW, XYZ);
    put_one(v, 23, 6);
    v.mula_bc(28, 0, LW, XY);
    v.madd_bc(22, 29, 30, LZ, XY);
    v.adda_bc(0, 24, LW, Y);
    let more = (v.vi[3] as i16) > 0;
    v.madd_bc(23, 24, 30, LZ, Y);
    if more {
        edge_loop_s(v);
    }
}

fn put_one(v: &mut Vu, r: usize, ir: usize) {
    let a = v.vi[ir];
    v.sq(r, a as u32, XYZW);
    v.vi[ir] = a.wrapping_add(1);
}

/// 0xea0: the edge at t = 0: the rows c12..c15 themselves; the corners at TOP+2 and TOP+4; the lightmap y = y,
/// x = x + w·s
fn edge2(v: &mut Vu) {
    let top = v.vi[1] as u32;
    v.add_bc(23, 0, 24, LW, Y);
    v.lq(28, top + 2, XYZW);
    v.lq(29, top + 4, XYZW);
    lqi(v, 30, 2);
    v.vi[4] = 229;
    v.vi[5] = 297;
    v.sub(29, 29, 28, XYZW);
    v.vi[6] = 145;
    let point = |v: &mut Vu, w: usize, next: usize| {
        v.mula_bc(13, w, LX, XYZ);
        lqi(v, next, 2);
        v.madda_bc(14, w, LY, XYZ);
        v.madda_bc(15, w, LZ, XYZ);
        v.madd_bc(21, 16, w, LW, XYZ);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, w, LZ, XY);
        v.adda_bc(0, 24, LZ, X);
        v.madd_bc(23, 24, w, LZ, X);
    };
    point(v, 30, 31);
    loop {
        v.mula_bc(13, 31, LX, XYZ);
        lqi(v, 30, 2);
        v.madda_bc(14, 31, LY, XYZ);
        put_one(v, 21, 4);
        v.madda_bc(15, 31, LZ, XYZ);
        put_one(v, 22, 5);
        v.madd_bc(21, 16, 31, LW, XYZ);
        put_one(v, 23, 6);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, 31, LZ, XY);
        v.vi[3] = v.vi[3].wrapping_sub(2);
        v.adda_bc(0, 24, LZ, X);
        v.madd_bc(23, 24, 31, LZ, X);
        v.mula_bc(13, 30, LX, XYZ);
        lqi(v, 31, 2);
        v.madda_bc(14, 30, LY, XYZ);
        put_one(v, 21, 4);
        v.madda_bc(15, 30, LZ, XYZ);
        put_one(v, 22, 5);
        v.madd_bc(21, 16, 30, LW, XYZ);
        put_one(v, 23, 6);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, 30, LZ, XY);
        v.adda_bc(0, 24, LZ, X);
        let more = (v.vi[3] as i16) > 0;
        v.madd_bc(23, 24, 30, LZ, X);
        if !more {
            return;
        }
    }
}

/// 0xfa0: the edge at t = 1: each point the whole polynomial (four rows of four, each summed in s = the point's
/// weights, then the four summed at t = 1); the corners at TOP+3 and TOP+5; the lightmap y = y + h, x = x + w·s
fn edge3(v: &mut Vu) {
    let top = v.vi[1] as u32;
    v.adda_bc(0, 24, LW, Y);
    v.lq(28, top + 3, XYZW);
    v.madd_bc(23, 24, 0, LW, Y);
    v.lq(29, top + 5, XYZW);
    lqi(v, 30, 2);
    v.vi[4] = 237;
    v.vi[5] = 305;
    v.sub(29, 29, 28, XYZW);
    v.vi[6] = 153;
    // the four rows in s at the weights in `w`
    let rows = |v: &mut Vu, w: usize| {
        for (k, out) in [17usize, 18, 19, 20].into_iter().enumerate() {
            v.transform_rows(out, [1 + 4 * k, 2 + 4 * k, 3 + 4 * k, 4 + 4 * k], w, XYZ);
        }
    };
    let finish = |v: &mut Vu, w: usize| {
        v.mula_bc(17, 0, LW, XYZ);
        v.madda_bc(18, 0, LW, XYZ);
        v.madda_bc(19, 0, LW, XYZ);
        v.madd_bc(21, 20, 0, LW, XYZ);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, w, LZ, XY);
        v.adda_bc(0, 24, LZ, X);
        v.madd_bc(23, 24, w, LZ, X);
    };
    rows(v, 30);
    lqi(v, 31, 2);
    finish(v, 30);
    loop {
        // (the stores come as the fourth row is made; nothing they store is touched by it)
        for k in 0..3usize {
            v.transform_rows([17, 18, 19][k], [1 + 4 * k, 2 + 4 * k, 3 + 4 * k, 4 + 4 * k], 31, XYZ);
        }
        put_edge_point(v);
        v.transform_rows(20, [13, 14, 15, 16], 31, XYZ);
        lqi(v, 30, 2);
        finish(v, 31);
        for k in 0..3usize {
            v.transform_rows([17, 18, 19][k], [1 + 4 * k, 2 + 4 * k, 3 + 4 * k, 4 + 4 * k], 30, XYZ);
            if k == 2 {
                v.vi[3] = v.vi[3].wrapping_sub(2);
            }
        }
        put_edge_point(v);
        v.transform_rows(20, [13, 14, 15, 16], 30, XYZ);
        lqi(v, 31, 2);
        v.mula_bc(17, 0, LW, XYZ);
        v.madda_bc(18, 0, LW, XYZ);
        v.madda_bc(19, 0, LW, XYZ);
        v.madd_bc(21, 20, 0, LW, XYZ);
        v.mula_bc(28, 0, LW, XY);
        v.madd_bc(22, 29, 30, LZ, XY);
        v.adda_bc(0, 24, LZ, X);
        let more = (v.vi[3] as i16) > 0;
        v.madd_bc(23, 24, 30, LZ, X);
        if !more {
            return;
        }
    }
}

/// 0x1a48: the edges' lightmap rewrite: for each edge (its steps from TOP+0), the zipped vertices' ST made their
/// lightmap coordinates times Q, the longer row (A: the edge when it has 6 or more steps, else the grid's border
/// row) and the shorter (B) in turn as the zipper laid them out
fn edges_st_from_lightmap(v: &mut Vu) {
    v.vi[1] = v.top as u16;
    let top = v.top;
    for (k, r) in [8usize, 9, 10, 11].into_iter().enumerate() {
        v.vi[r] = v.ilw(top, k);
    }
    // per edge: the border row's lightmap start, the edge's, the border's stride
    let rows: [(u16, u16, u16); 4] = [(93, 129, 1), (123, 137, 1), (93, 145, 6), (98, 153, 6)];
    let sites = [0x1ab8u32, 0x1b00, 0x1b58, 0x1ba0, 0x1bf8, 0x1c40, 0x1c98, 0x1ce0];
    for e in 0..4 {
        let steps = v.vi[8 + e];
        v.vi[13] = steps.wrapping_sub(6);
        let (border, edge, stride) = rows[e];
        let long_edge = (v.vi[13] as i16) >= 0;
        if long_edge {
            v.vi[2] = steps;
            v.vi[3] = 6;
            v.vi[4] = edge;
            v.vi[5] = border;
            v.vi[6] = 1;
            v.vi[14] = ((sites[2 * e + 1] + 16) / 8) as u16;
            v.vi[7] = stride;
        } else {
            v.vi[2] = 6;
            v.vi[3] = steps;
            v.vi[4] = border;
            v.vi[5] = edge;
            v.vi[6] = stride;
            v.vi[14] = ((sites[2 * e] + 16) / 8) as u16;
            v.vi[7] = 1;
        }
        zipped_st_from_lightmap(v);
    }
}

/// 0x1d00: one zipper's vertices from vi12 (A at vi4 by vi6, B at vi5 by vi7; vi2 / vi3 their counts, B 4 or 6):
/// each pair's ST its lightmap coordinate times its Q, then the longer row's two left over (when it is longer)
fn zipped_st_from_lightmap(v: &mut Vu) {
    let pk = |v: &Vu, off: i32| (v.vi[12] as i32 + off) as u32;
    let a_load = |v: &mut Vu, r: usize, off: i32| {
        let a = v.vi[4] as u32;
        v.lq(r, a, XY);
        v.lq(r, (v.vi[12] as i32 + off) as u32, W);
    };
    let b_load = |v: &mut Vu, r: usize, off: i32| {
        let a = v.vi[5] as u32;
        v.lq(r, a, XY);
        v.lq(r, (v.vi[12] as i32 + off) as u32, W);
    };
    let step = |v: &mut Vu| {
        v.vi[4] = v.vi[4].wrapping_add(v.vi[6]);
        v.vi[5] = v.vi[5].wrapping_add(v.vi[7]);
    };
    a_load(v, 7, 0);
    b_load(v, 8, 3);
    step(v);
    a_load(v, 9, 6);
    b_load(v, 10, 9);
    step(v);
    v.vi[2] = v.vi[2].wrapping_sub(v.vi[3]);
    // four pairs
    for (k, (src, nxt)) in [((7usize, 8usize), (11usize, 12usize)), ((9, 10), (13, 14)), ((11, 12), (7, 8)), ((13, 14), (9, 10))].into_iter().enumerate() {
        let (ta, tb) = if k % 2 == 0 { (20, 21) } else { (22, 23) };
        let k = k as i32;
        v.mul_bc(ta, src.0, src.0, LW, XY);
        let a = v.vi[4] as u32;
        v.lq(nxt.0, a, XY);
        v.mul_bc(tb, src.1, src.1, LW, XY);
        v.lq(nxt.0, pk(v, 6 * k + 12), W);
        let b = v.vi[5] as u32;
        v.lq(nxt.1, b, XY);
        v.lq(nxt.1, pk(v, 6 * k + 15), W);
        v.sq(ta, pk(v, 6 * k), XY);
        if k == 3 {
            v.vi[3] = v.vi[3].wrapping_sub(4);
        }
        v.sq(tb, pk(v, 6 * k + 3), XY);
        if k < 3 {
            step(v);
        }
    }
    let four = v.vi[3] == 0;
    v.vi[12] = v.vi[12].wrapping_add(24);
    if four {
        // B had four: the longer row's two left over, when there are any
        if v.vi[2] == 0 {
            return;
        }
        v.mul_bc(20, 7, 7, LW, XY);
        // (the second's xy is vf9's, its Q from vf8.w: the slot's w the program loaded there)
        v.mul_bc(21, 9, 8, LW, XY);
        v.vi[12] = v.vi[12].wrapping_add(6);
        v.sq(20, pk(v, -6), XY);
        v.sq(21, pk(v, -3), XY);
        return;
    }
    // B had six: two more pairs, then the two left over
    v.vi[4] = v.vi[4].wrapping_add(v.vi[6]);
    v.mul_bc(20, 7, 7, LW, XY);
    let a = v.vi[4] as u32;
    v.lq(11, a, XY);
    v.mul_bc(21, 8, 8, LW, XY);
    v.lq(11, pk(v, 12), W);
    v.sq(20, pk(v, 0), XY);
    v.sq(21, pk(v, 3), XY);
    v.vi[4] = v.vi[4].wrapping_add(v.vi[6]);
    v.mul_bc(22, 9, 9, LW, XY);
    let a = v.vi[4] as u32;
    v.lq(12, a, XY);
    v.mul_bc(23, 10, 10, LW, XY);
    v.lq(12, pk(v, 15), W);
    v.vi[12] = v.vi[12].wrapping_add(12);
    v.sq(22, pk(v, -6), XY);
    v.sq(23, pk(v, -3), XY);
    if v.vi[2] == 0 {
        return;
    }
    v.mul_bc(20, 11, 11, LW, XY);
    v.mul_bc(21, 12, 12, LW, XY);
    v.vi[12] = v.vi[12].wrapping_add(6);
    v.sq(20, pk(v, -6), XY);
    v.sq(21, pk(v, -3), XY);
}

/// 0x1f78 / 0x2350: each edge zipped to the inner grid's border row (the textured pass from the texture
/// coordinates, or, `lightmap`, the clipped lightmap pass from the lightmap coordinates): A the longer row (the
/// edge when it has 6 or more steps)
fn zip_edges(v: &mut Vu, lightmap: bool) {
    v.isw(v.vi[15], SAVE, Y);
    // per edge: the border row's points and stride, the edge's points; the border's and edge's texture (or
    // lightmap) coordinates
    let (bt, et): ([u16; 4], [u16; 4]) = if lightmap { ([93, 123, 93, 98], [129, 137, 145, 153]) } else { ([245, 275, 245, 250], [281, 289, 297, 305]) };
    let border = [177u16, 207, 177, 182];
    let edge = [213u16, 221, 229, 237];
    let stride = [1u16, 1, 6, 6];
    let base: u32 = if lightmap { 0x23d8 } else { 0x2000 };
    let site_gap = [0u32, 0x58, 0xe8, 0x140, 0x1d0, 0x228, 0x2b8, 0x310];
    for e in 0..4 {
        v.vi[1] = v.top as u16;
        v.vi[11] = v.ilw(v.top, e);
        v.vi[13] = v.vi[11].wrapping_sub(6);
        let long_edge = (v.vi[13] as i16) >= 0;
        if long_edge {
            v.vi[2] = edge[e];
            v.vi[3] = border[e];
            v.vi[4] = et[e];
            v.vi[5] = bt[e];
            v.vi[6] = v.vi[11];
            v.vi[7] = 6;
            v.vi[8] = 1;
            v.vi[15] = ((base + site_gap[2 * e + 1] + 16) / 8) as u16;
            v.vi[10] = stride[e];
        } else {
            v.vi[2] = border[e];
            v.vi[3] = edge[e];
            v.vi[4] = bt[e];
            v.vi[5] = et[e];
            v.vi[6] = 6;
            v.vi[7] = v.vi[11];
            v.vi[8] = stride[e];
            v.vi[15] = ((base + site_gap[2 * e] + 16) / 8) as u16;
            v.vi[10] = 1;
        }
        zipper(v);
    }
    v.vi[15] = v.ilw(SAVE, LY);
}

/// 0x2728: two rows zipped into a strip at vi12 (A at vi2 / vi4 by vi8, B at vi3 / vi5 by vi10; vi6 / vi7 their
/// counts, A the longer): A and B in turn for B's count, then A's two left over; each new triangle clip-tested,
/// and one that crosses the frustum kept out of the strip and clipped on its own (the four handlers)
fn zipper(v: &mut Vu) {
    let sqi = |v: &mut Vu, r: usize| {
        let a = v.vi[12];
        v.sq(r, a as u32, XYZW);
        v.vi[12] = a.wrapping_add(1);
    };
    let ld = |v: &mut Vu, r: usize, ir: usize| {
        let a = v.vi[ir] as u32;
        v.lq(r, a, XYZW);
    };
    let adv = |v: &mut Vu, ir: usize, by: usize| v.vi[ir] = v.vi[ir].wrapping_add(v.vi[by]);
    ld(v, 8, 2);
    ld(v, 9, 3);
    ld(v, 10, 4);
    ld(v, 11, 5);
    v.mula_bc(1, 8, LX, XYZW);
    v.lq(7, 6, XYZW);
    v.madda_bc(2, 8, LY, XYZW);
    v.madda_bc(3, 8, LZ, XYZW);
    v.madd_bc(12, 4, 8, LW, XYZW);
    v.transform(13, 1, 9, XYZW);
    v.div(0, LW, 12, LW);
    v.sub(ONE, 0, 0, XYZW);
    v.vi[9] = 0xffff;
    v.vi[6] = v.vi[6].wrapping_sub(v.vi[7]);
    adv(v, 2, 8);
    adv(v, 3, 10);
    v.add_bc(ONE, ONE, 0, LW, XYZW);
    adv(v, 4, 8);
    adv(v, 5, 10);
    // the first A and B: drawn with the no-draw bit
    v.mul_q(14, 12, XYZW);
    ld(v, 8, 2);
    v.mul_q(15, 10, XYZW);
    ld(v, 10, 4);
    v.clipw(12, 12);
    v.mula(ONE, 6, XYZW);
    v.div(0, LW, 13, LW);
    v.madd(14, 14, 5, XYZW);
    adv(v, 2, 8);
    v.mula_bc(1, 8, LX, XYZW);
    adv(v, 4, 8);
    v.madda_bc(2, 8, LY, XYZW);
    v.madda_bc(3, 8, LZ, XYZW);
    v.ftoi4(14, 14, XYZ);
    v.madd_bc(12, 4, 8, LW, XYZW);
    v.mfir(14, 9, W);
    v.mul_q(16, 13, XYZW);
    ld(v, 9, 3);
    v.mul_q(17, 11, XYZW);
    ld(v, 11, 5);
    v.clipw(13, 13);
    sqi(v, 15);
    v.mula(ONE, 6, XYZW);
    v.div(0, LW, 12, LW);
    v.madd(16, 16, 5, XYZW);
    sqi(v, 7);
    v.mula_bc(1, 9, LX, XYZW);
    sqi(v, 14);
    v.madda_bc(2, 9, LY, XYZW);
    v.vi[7] = v.vi[7].wrapping_sub(1);
    v.madda_bc(3, 9, LZ, XYZW);
    adv(v, 3, 10);
    v.ftoi4(16, 16, XYZ);
    adv(v, 5, 10);
    v.madd_bc(13, 4, 9, LW, XYZW);
    v.mfir(16, 9, W);
    sqi(v, 17);
    sqi(v, 7);
    sqi(v, 16);
    // the rest of B's count: a new A, then a new B, each a triangle tested
    loop {
        v.mul_q(14, 12, XYZW);
        ld(v, 8, 2);
        v.mul_q(15, 10, XYZW);
        ld(v, 10, 4);
        v.clipw(12, 12);
        v.mula(ONE, 6, XYZW);
        v.div(0, LW, 13, LW);
        v.madd(14, 14, 5, XYZW);
        adv(v, 2, 8);
        v.mula_bc(1, 8, LX, XYZW);
        adv(v, 4, 8);
        v.madda_bc(2, 8, LY, XYZW);
        v.vi[7] = v.vi[7].wrapping_sub(1);
        v.madda_bc(3, 8, LZ, XYZW);
        let clipped = v.fcand(0x3ffff);
        v.ftoi4(14, 14, XYZW);
        v.madd_bc(12, 4, 8, LW, XYZW);
        if clipped {
            zip_clip(v, 0);
        }
        v.mul_q(16, 13, XYZW);
        ld(v, 9, 3);
        v.mul_q(17, 11, XYZW);
        ld(v, 11, 5);
        v.clipw(13, 13);
        sqi(v, 15);
        v.mula(ONE, 6, XYZW);
        v.div(0, LW, 12, LW);
        v.madd(16, 16, 5, XYZW);
        sqi(v, 7);
        v.mula_bc(1, 9, LX, XYZW);
        sqi(v, 14);
        v.madda_bc(2, 9, LY, XYZW);
        adv(v, 3, 10);
        v.madda_bc(3, 9, LZ, XYZW);
        adv(v, 5, 10);
        v.ftoi4(16, 16, XYZW);
        let clipped = v.fcand(0x3ffff);
        v.madd_bc(13, 4, 9, LW, XYZW);
        // (the branch's delay slot: the B's ST stored before the handler)
        sqi(v, 17);
        if clipped {
            zip_clip(v, 1);
        }
        sqi(v, 7);
        let more = (v.vi[7] as i16) > 0;
        sqi(v, 16);
        if !more {
            break;
        }
    }
    if v.vi[6] == 0 {
        return;
    }
    // A's two left over
    v.mul_q(14, 12, XYZW);
    ld(v, 8, 2);
    v.mul_q(15, 10, XYZW);
    ld(v, 10, 4);
    v.clipw(12, 12);
    v.mula(ONE, 6, XYZW);
    v.madd(14, 14, 5, XYZW);
    v.mula_bc(1, 8, LX, XYZW);
    v.madda_bc(2, 8, LY, XYZW);
    v.madda_bc(3, 8, LZ, XYZW);
    let clipped = v.fcand(0x3ffff);
    v.ftoi4(14, 14, XYZW);
    v.madd_bc(12, 4, 8, LW, XYZW);
    if clipped {
        zip_clip(v, 2);
    }
    v.div(0, LW, 12, LW);
    sqi(v, 15);
    sqi(v, 7);
    sqi(v, 14);
    v.mul_q(14, 12, XYZW);
    v.mul_q(15, 10, XYZW);
    v.clipw(12, 12);
    v.mula(ONE, 6, XYZW);
    v.madd(14, 14, 5, XYZW);
    let clipped = v.fcand(0x3ffff);
    v.ftoi4(14, 14, XYZW);
    if clipped {
        zip_clip(v, 3);
    }
    sqi(v, 15);
    sqi(v, 7);
    sqi(v, 14);
}

/// The zipper's clip handlers (0x2a60 a new A, 0x2c10 a new B, 0x2dc0 / 0x2f50 the two left over): the vertex's
/// no-draw bit set; unless all three points are past the far plane or outside one plane, the triangle (its
/// points through the matrix again, its texture coordinates, the colour) clipped and sent; vi9, vi10, vi12 and
/// vi15 kept in vf19 meanwhile; then (for the first two) Q, and the last two tests made again
fn zip_clip(v: &mut Vu, which: usize) {
    v.mfir(if which == 1 { 16 } else { 14 }, 9, W);
    let check = |v: &mut Vu, k: usize| v.fcor([0xfe_fbef, 0xfd_f7df, 0xff_7df7, 0xff_befb, 0xff_df7d, 0xff_efbe][k]);
    for k in 0..6 {
        if check(v, k) {
            if k < 5 {
                check(v, k + 1);
            }
            return;
        }
    }
    let sub = |v: &mut Vu, to: usize, from: usize, by: usize| v.vi[to] = v.vi[from].wrapping_sub(v.vi[by]);
    let subself = |v: &mut Vu, r: usize, by: usize| v.vi[r] = v.vi[r].wrapping_sub(v.vi[by]);
    let ld = |v: &mut Vu, r: usize, ir: usize| {
        let a = v.vi[ir] as u32;
        v.lq(r, a, XYZW);
    };
    // the triangle's points (vf20 / vf23, vf21 / vf24, vf22 / vf25: the place and texture coordinates)
    match which {
        0 | 1 => {
            // the newest row's two back (vf22) and three back (vf20); the other row's two back (vf21)
            let (p, t, s, op, ot, os) = if which == 0 { (2, 4, 8, 3, 5, 10) } else { (3, 5, 10, 2, 4, 8) };
            sub(v, 13, p, s);
            subself(v, 13, s);
            sub(v, 14, t, s);
            subself(v, 14, s);
            ld(v, 22, 13);
            ld(v, 25, 14);
            subself(v, 13, s);
            subself(v, 14, s);
            v.mula_bc(1, 22, LX, XYZW);
            ld(v, 20, 13);
            v.madda_bc(2, 22, LY, XYZW);
            ld(v, 23, 14);
            v.madda_bc(3, 22, LZ, XYZW);
            sub(v, 13, op, os);
            v.madd_bc(31, 4, 22, LW, XYZW);
            subself(v, 13, os);
            v.mula_bc(1, 20, LX, XYZW);
            sub(v, 14, ot, os);
            v.madda_bc(2, 20, LY, XYZW);
            subself(v, 14, os);
            v.madda_bc(3, 20, LZ, XYZW);
            ld(v, 21, 13);
            v.madd_bc(29, 4, 20, LW, XYZW);
            ld(v, 24, 14);
            v.itof0(26, 7, XYZW);
            v.mfir(19, 10, Y);
            v.mfir(19, 15, W);
            v.mula_bc(1, 21, LX, XYZW);
            v.mfir(19, 9, X);
            v.madda_bc(2, 21, LY, XYZW);
            v.mfir(19, 12, Z);
            v.madda_bc(3, 21, LZ, XYZW);
            v.vi[10] = 314;
            v.madd_bc(30, 4, 21, LW, XYZW);
            v.vi[15] = 9;
            for (k, r) in [29, 23, 26, 30, 24, 26, 31, 25, 26].into_iter().enumerate() {
                v.sq(r, 314 + k as u32, XYZW);
            }
        }
        2 => {
            // A one back (vf22) and two back (vf20); B two back (vf21)
            sub(v, 13, 2, 8);
            sub(v, 14, 4, 8);
            ld(v, 22, 13);
            ld(v, 25, 14);
            subself(v, 13, 8);
            subself(v, 14, 8);
            v.mula_bc(1, 22, LX, XYZW);
            ld(v, 20, 13);
            v.madda_bc(2, 22, LY, XYZW);
            ld(v, 23, 14);
            v.madda_bc(3, 22, LZ, XYZW);
            sub(v, 13, 3, 10);
            v.madd_bc(31, 4, 22, LW, XYZW);
            subself(v, 13, 10);
            v.mula_bc(1, 20, LX, XYZW);
            sub(v, 14, 5, 10);
            v.madda_bc(2, 20, LY, XYZW);
            subself(v, 14, 10);
            v.madda_bc(3, 20, LZ, XYZW);
            ld(v, 21, 13);
            v.madd_bc(29, 4, 20, LW, XYZW);
            ld(v, 24, 14);
            v.itof0(26, 7, XYZW);
            v.mfir(19, 10, Y);
            v.mfir(19, 15, W);
            v.mula_bc(1, 21, LX, XYZW);
            v.mfir(19, 9, X);
            v.madda_bc(2, 21, LY, XYZW);
            v.mfir(19, 12, Z);
            v.madda_bc(3, 21, LZ, XYZW);
            v.vi[10] = 314;
            v.madd_bc(30, 4, 21, LW, XYZW);
            v.vi[15] = 9;
            for (k, r) in [29, 23, 26, 30, 24, 26, 31, 25, 26].into_iter().enumerate() {
                v.sq(r, 314 + k as u32, XYZW);
            }
        }
        _ => {
            // A now (vf22) and one back (vf21); B two back (vf20)
            ld(v, 22, 2);
            ld(v, 25, 4);
            sub(v, 13, 2, 8);
            sub(v, 14, 4, 8);
            v.mula_bc(1, 22, LX, XYZW);
            ld(v, 21, 13);
            v.madda_bc(2, 22, LY, XYZW);
            ld(v, 24, 14);
            v.madda_bc(3, 22, LZ, XYZW);
            sub(v, 13, 3, 10);
            v.madd_bc(31, 4, 22, LW, XYZW);
            subself(v, 13, 10);
            v.mula_bc(1, 21, LX, XYZW);
            sub(v, 14, 5, 10);
            v.madda_bc(2, 21, LY, XYZW);
            subself(v, 14, 10);
            v.madda_bc(3, 21, LZ, XYZW);
            ld(v, 20, 13);
            v.madd_bc(30, 4, 21, LW, XYZW);
            ld(v, 23, 14);
            v.itof0(26, 7, XYZW);
            v.mfir(19, 10, Y);
            v.mfir(19, 15, W);
            v.mula_bc(1, 20, LX, XYZW);
            v.mfir(19, 9, X);
            v.madda_bc(2, 20, LY, XYZW);
            v.mfir(19, 12, Z);
            v.madda_bc(3, 20, LZ, XYZW);
            v.vi[10] = 314;
            v.madd_bc(29, 4, 20, LW, XYZW);
            v.vi[15] = 9;
            for (k, r) in [(3u32, 30usize), (4, 24), (5, 26), (0, 29), (1, 23), (2, 26), (6, 31), (7, 25), (8, 26)] {
                v.sq(r, 314 + k, XYZW);
            }
        }
    }
    v.vi[14] = [1403, 1457, 1509, 1557][which];
    clip_and_send(v, MIXED);
    match which {
        0 => v.div(0, LW, 13, LW),
        1 => v.div(0, LW, 12, LW),
        _ => {}
    }
    v.clipw(30, 30);
    v.mtir(9, 19, LX);
    v.clipw(31, 31);
    v.mtir(10, 19, LY);
    v.mtir(12, 19, LZ);
    v.mtir(15, 19, LW);
}

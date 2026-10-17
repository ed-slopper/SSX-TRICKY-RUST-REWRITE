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

/// One of the uniform programs: N steps (points a row), where its weights start, the view transform's first result
/// register, the clip handlers' return places (in 8 bytes), whether it clips by the far plane
#[derive(Clone, Copy)]
struct Steps {
    n: u16,
    weights: u16,
    first_result: usize,
    returns: (u16, u16),
    far: bool,
}
const STEPS4: Steps = Steps { n: 4, weights: 25, first_result: 10, returns: (376, 422), far: false };
const STEPS6: Steps = Steps { n: 6, weights: 19, first_result: 13, returns: (394, 440), far: true };
const STEPS8: Steps = Steps { n: 8, weights: 11, first_result: 13, returns: (413, 459), far: true };

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
    edges(v);
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
fn edges(v: &mut Vu) {
    let top = v.vi[1] as u32;
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
    let top = v.vi[1] as u32;
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

//! The VU1 particle programs (board rows G7e2, G7e3): what the particle packets (`particledraw`) call. Program 7
//! (chain 0x31c790, 89 instructions, the renderer's +0x260) scatters the snowfall's flakes (`snowfall`'s layers)
//! at random in a box and sends each as a sprite.
//! Program 6 (chain 0x31c260, 152 instructions, +0x258) draws the particle systems' trails of sprites, each
//! bending and fading (`ParticleSys_Draw`, the ground spray). Ported on
//! [`Vu`] in the programs' order (each pair's two halves reading the registers as they were before the pair; a
//! flag read seeing the flags of the op four pairs back). Checked call by call against the game's programs in
//! the runner (`tests/vu1particle.rs`).
//!
//! Program 6's block, the frame's words and its packets are with [`run6`].
//!
//! **The frame's words** (VU1 0..6, from `PS2Draw_WriteTexState`): 0..3 the clip matrix (rows), 4 the screen's
//! scale, 5 its offset, 6 the sprites' x and y scale.
//!
//! **A system's block** (at TOP, from the packet): +0 x the count (the program draws one more when it is 0 or
//! less), y and z the size's base and spread; +1 the seed (each lane's mantissa starts R for that lane's
//! coordinate); +2 the box's corner, +3..+5 its edges (each point is the corner plus the edges times R's 1.0 to
//! 2.0); +6 the colour; +7 and +8 the sprite's two texture coordinates (their z set to 1).
//!
//! **The packets**: two buffers, at 25 and 521, whose GIF tags the program's chain loads. Each holds up to 99
//! sprites of five quadwords: ST, the colour, the first corner, ST, the second corner (the corners the point less
//! and plus the size, divided by w, scaled and offset, ×16 as integers). A full buffer is sent and the other
//! filled; what is left is sent at the end. A point beyond any plane is not drawn.

use crate::vu::*;

/// Program 7 called at `start` (only 0 is used)
pub fn run7(v: &mut Vu, _start: u32) {
    v.vi[15] = v.top as u16;
    let top = v.vi[15] as u32;
    // the box's corner and edges through the clip matrix (vf23..vf26)
    v.lq(5, top + 2, XYZW);
    for k in 0..4 {
        v.lq(1 + k, k as u32, XYZW);
    }
    v.transform_rows(23, [1, 2, 3, 4], 5, XYZW);
    v.lq(6, top + 3, XYZW);
    v.lq(13, top + 7, XYZW);
    v.lq(14, top + 8, XYZW);
    v.lq(11, 5, XYZW);
    v.lq(5, top + 4, XYZW);
    v.lq(12, 4, XYZW);
    v.lq(27, top + 1, XYZW);
    v.lq(28, top, YZ);
    v.transform_rows(24, [1, 2, 3, 4], 6, XYZW);
    v.lq(6, top + 5, XYZW);
    v.lq(29, top + 6, XYZW);
    v.lq(30, 6, XY);
    v.lq(27, top + 1, XYZW);
    v.transform_rows(25, [1, 2, 3, 4], 5, XYZW);
    v.vi[10] = 25;
    v.vi[11] = 521;
    v.vi[12] = v.vi[10] + 1;
    v.vi[13] = 0;
    v.transform_rows(26, [1, 2, 3, 4], 6, XYZW);
    v.sub(30, 30, 30, ZW);
    v.vi[14] = 99;
    v.ftoi0(29, 29, XYZW);
    v.vi[9] = v.ilw(top, LX);
    // the first point: each lane's R started from the seed, then stepped once (x, z, w, y)
    v.rinit(27, LX);
    v.mula_bc(23, 0, LW, XYZW);
    v.rnext(27, X);
    v.madda_bc(24, 27, LX, XYZW);
    v.rinit(27, LZ);
    v.rnext(27, Z);
    v.madda_bc(25, 27, LZ, XYZW);
    v.rinit(27, LW);
    v.rnext(27, W);
    v.madd_bc(20, 26, 27, LW, XYZW);
    v.rinit(27, LY);
    size_and_clip(v, true);
    v.mr32(13, 0, Z);
    v.mr32(14, 0, Z);
    v.fcand(0x3f);
    v.sub(8, 20, 21, XYZW);
    v.add(9, 20, 21, XYZW);
    loop {
        // 0x150: this point's corners finished while the next point is made (its R stepped on from the last)
        v.mula_bc(23, 0, LW, XYZW);
        v.rnext(27, X);
        v.madda_bc(24, 27, LX, XYZW);
        v.rinit(27, LZ);
        v.mul_q(8, 8, XYZW);
        v.rnext(27, Z);
        v.mul_q(9, 9, XYZW);
        v.rinit(27, LW);
        v.madda_bc(25, 27, LZ, XYZW);
        v.rnext(27, W);
        v.madd_bc(20, 26, 27, LW, XYZW);
        v.rinit(27, LY);
        v.mula_bc(11, 0, LW, XYZW);
        v.rnext(27, Y);
        v.madd(8, 8, 12, XYZW);
        v.mula_bc(11, 0, LW, XYZW);
        let out = v.vi[1] != 0;
        v.madd(9, 9, 12, XYZW);
        v.div(0, LW, 20, LW);
        if out {
            // 0x298: not drawn
            size_and_clip(v, false);
        } else {
            v.vi[12] = v.vi[12].wrapping_add(5);
            v.vi[13] = v.vi[13].wrapping_add(1);
            let at = v.vi[12] as u32;
            v.mula_bc(28, 0, LW, Y);
            v.ftoi4(8, 8, XYZW);
            v.madd_bc(21, 27, 28, LZ, Y);
            v.sq(29, at.wrapping_sub(4), XYZW);
            v.ftoi4(9, 9, XYZW);
            v.sq(13, at.wrapping_sub(5), XYZW);
            v.clipw(20, 20);
            v.sq(14, at.wrapping_sub(2), XYZW);
            v.sq(8, at.wrapping_sub(3), XYZW);
            v.mul_bc(21, 30, 21, LY, XYZW);
            v.sq(9, at.wrapping_sub(1), XYZW);
            if v.vi[13] == v.vi[14] {
                // a full packet: sent, the other buffer next
                send(v);
                v.vi[1] = v.vi[10];
                v.vi[10] = v.vi[11];
                v.vi[11] = v.vi[1];
                v.vi[13] = 0;
                v.vi[12] = v.vi[10] + 1;
            }
        }
        // 0x228
        v.vi[9] = v.vi[9].wrapping_sub(1);
        v.fcand(0x3f);
        v.sub(8, 20, 21, XYZW);
        let more = (v.vi[9] as i16) > 0;
        v.add(9, 20, 21, XYZW);
        v.rinit(27, LX);
        if !more {
            break;
        }
    }
    if v.vi[13] != 0 {
        send(v);
    } else {
        v.vi[3] = v.vi[13].wrapping_add(0x4000);
    }
    // (the last flags made: the second corner's)
    v.mac = Vu::mac_flags(v.vf[9], XYZW);
}

/// The point's size (the base plus the spread times R's y) as the sprite's half size, and its clip test (vf20);
/// the first time with the y lane's R step between
fn size_and_clip(v: &mut Vu, first: bool) {
    v.mula_bc(28, 0, LW, Y);
    if first {
        v.rnext(27, Y);
    }
    v.madd_bc(21, 27, 28, LZ, Y);
    v.clipw(20, 20);
    if first {
        v.div(0, LW, 20, LW);
    }
    v.mul_bc(21, 30, 21, LY, XYZW);
}

/// The packet's tag's NLOOP and EOP (0x8000 | the count) put in and the packet sent
fn send(v: &mut Vu) {
    v.vi[3] = v.vi[13].wrapping_add(0x4000);
    v.vi[3] = v.vi[3].wrapping_add(0x4000);
    v.isw(v.vi[3], v.vi[10] as u32, X);
    v.kick(v.vi[10] as u32);
}

/// The clip matrix's rows
const M: [usize; 4] = [1, 2, 3, 4];
/// The I bit's immediates: the bend's two terms, the bend's limit, the colour's top
const BEND: [f32; 2] = [-0.73, 0.113];
const LIMIT: f32 = 2.7;
const TOP255: f32 = 255.0;

/// Program 6 called at `start` (only 0 is used).
///
/// **Its block** (20 quadwords at TOP): +0 the trails' count (x), the points' (y), a countdown (z) and its step
/// (w); +1 the size's spread and the fade's, then their bases; +2 the alpha's step, t's step and two seeds; +3, +4
/// seeds; +5 the trail's direction; +6..+9 a box (its corner and three edges); +10 the first trail's start, +11
/// and +12 its spread, +13 the step from trail to trail; +14..+17 the colour (a base, two terms by R, one by the
/// countdown); +18, +19 the sprite's texture coordinates (their z set to 1). +5..+13 go through the clip matrix.
///
/// **Each trail** starts at +10 plus +13 times its number plus +11 and +12 by R. It bends along the direction
/// less a random point of the box. Its colour, size (times the frame's sprite scale) and fade are drawn once.
/// **Each point** has t from the countdown down by t's step: its place is the start plus the direction times t
/// plus the bend times −0.73 m + 0.113 m² (m = t, at most 2.7). Its alpha falls by the alpha's step from the
/// colour's, held to 0..255. It is drawn only when t is below the fade and it is inside the frustum.
///
/// **The ends.** A trail ends after its count of points or when t falls below 0 (the sign flag of the z lane).
/// All of them end after the count of trails or when the countdown, less its step after each trail, falls
/// below 0. The packets are as program 7's (buffers at 47 and 533, 97 sprites each).
pub fn run6(v: &mut Vu, _start: u32) {
    v.vi[15] = v.top as u16;
    let top = v.vi[15] as u32;
    // the block's nine vectors through the clip matrix: vf23 (+5), vf24..vf27 (+10..+13), vf28..vf31 (+6..+9);
    // the texture coordinates' z set to 1 in the block (+18, +19)
    v.lq(5, top + 5, XYZW);
    for k in 0..4 {
        v.lq(1 + k, k as u32, XYZW);
    }
    v.transform_rows(23, M, 5, XYZW);
    v.lq(6, top + 10, XYZW);
    v.lq(7, top + 18, XY);
    v.vi[10] = 47;
    v.vi[11] = 533;
    v.transform_rows(24, M, 6, XYZW);
    v.lq(5, top + 11, XYZW);
    v.mr32(7, 0, Z);
    v.vi[12] = v.vi[10] + 1;
    v.vi[13] = 0;
    v.transform_rows(25, M, 5, XYZW);
    v.lq(6, top + 12, XYZW);
    v.sq(7, top + 18, XYZ);
    v.vi[14] = 97;
    v.lq(20, top + 2, XYZW);
    v.transform_rows(26, M, 6, XYZW);
    v.lq(5, top + 13, XYZW);
    v.lq(7, top + 19, XY);
    v.lq(21, top + 3, XYZ);
    v.lq(22, top + 4, XYZW);
    v.transform_rows(27, M, 5, XYZW);
    v.lq(6, top + 6, XYZW);
    v.mr32(7, 0, Z);
    v.lq(19, top, ZW);
    v.vi[9] = v.ilw(top, LX);
    v.transform_rows(28, M, 6, XYZW);
    v.lq(5, top + 7, XYZW);
    v.sq(7, top + 19, XYZ);
    v.vi[7] = 32;
    v.transform_rows(29, M, 5, XYZW);
    v.lq(6, top + 8, XYZW);
    v.transform_rows(30, M, 6, XYZW);
    v.lq(5, top + 9, XYZW);
    v.sub(12, 12, 12, XYZW);
    v.sub(13, 13, 13, X);
    v.transform_rows(31, M, 5, XYZW);
    // the bend's terms in vf12 (each pair's upper op sees the I loaded the pair before)
    v.i = BEND[0];
    v.add_i(12, 12, X);
    v.i = BEND[1];
    v.add_i(12, 12, Y);
    // the first trail's start (vf18): +10, +13 by the trail's number (vf13.x), +11 and +12 by R
    v.rinit(20, LZ);
    v.mula_bc(24, 0, LW, XYZW);
    v.rnext(20, Z);
    v.madda_bc(27, 13, LX, XYZW);
    v.rinit(20, LW);
    v.madda_bc(25, 20, LZ, XYZW);
    v.rnext(20, W);
    v.madd_bc(18, 26, 20, LW, XYZW);
    v.rinit(21, LX);
    v.mula_bc(28, 0, LW, XYZW);
    v.rnext(21, X);
    loop {
        // 0x198: the trail's bend (vf17: +5 less a random point of the box +6..+9), its colour (vf16: +14,
        // plus +15 and +16 by R, plus +17 by the countdown), its size (vf15.x) and fade (vf15.y) from +1 by R
        v.rinit(21, LY);
        v.madda_bc(29, 21, LX, XYZW);
        v.rnext(21, Y);
        v.madda_bc(30, 21, LY, XYZW);
        v.rinit(21, LZ);
        v.rnext(21, Z);
        v.madd_bc(17, 31, 21, LZ, XYZW);
        v.lq(1, top + 14, XYZW);
        v.lq(2, top + 15, XYZW);
        v.lq(4, top + 17, XYZW);
        v.lq(3, top + 16, XYZW);
        v.mula_bc(1, 0, LW, XYZW);
        v.rinit(22, LX);
        v.lq(1, top + 1, XYZW);
        v.sub(17, 23, 17, XYZW);
        v.rnext(22, X);
        v.madda_bc(2, 22, LX, XYZW);
        v.rinit(22, LY);
        v.lq(2, 6, XYZW);
        v.madda_bc(4, 19, LZ, XYZW);
        v.rnext(22, Y);
        v.madd_bc(16, 3, 22, LY, XYZW);
        v.rinit(22, LZ);
        v.adda_bc(0, 1, LZ, X);
        v.rnext(22, Z);
        v.madd_bc(15, 1, 22, LZ, X);
        v.rinit(22, LW);
        v.sub(14, 0, 0, ZW);
        v.mov(15, 0, W);
        v.adda_bc(0, 1, LW, Y);
        v.rnext(22, W);
        v.madd_bc(15, 1, 22, LW, Y);
        v.vi[8] = v.ilw(top, LY);
        v.mul_bc(14, 2, 15, LX, XY);
        v.mov(15, 19, Z);
        v.i = LIMIT;
        // the first point (t = vf15.z, the countdown): its place, colour, and the tests the next pair reads
        v.mini_i(6, 15, Z);
        v.mov(10, 16, XYZ);
        v.mul_bc(5, 23, 15, LZ, XYZW);
        v.mul(10, 16, 15, W);
        v.mul(11, 6, 6, Z);
        v.add(5, 5, 18, XYZW);
        v.max_bc(10, 10, 0, LX, XYZW);
        bend(v);
        v.sub_bc(15, 15, 20, LX, W);
        v.i = TOP255;
        v.mini_i(10, 10, XYZW);
        v.mula_bc(5, 0, LW, XYZW);
        v.madd_bc(5, 17, 6, LZ, XYZW);
        let mut flags = tests(v);
        loop {
            // 0x2d0: this point finished while the next is made; the flag reads see the tests four pairs back
            v.mul(10, 16, 15, W);
            v.vi[3] = (flags.0 & v.vi[7] as u32) as u16;
            v.sub(8, 5, 14, XYZW);
            v.mov(10, 16, XYZ);
            v.add(9, 5, 14, XYZW);
            v.vi[5] = (flags.1 & v.vi[7] as u32) as u16;
            v.mul_bc(5, 23, 15, LZ, XYZW);
            v.i = LIMIT;
            v.mini_i(6, 15, Z);
            v.fcand(0x3f);
            v.max_bc(10, 10, 0, LX, XYZW);
            v.lq(1, 5, XYZW);
            v.mul_q(8, 8, XYZW);
            v.lq(2, 4, XYZW);
            v.mul_q(9, 9, XYZW);
            v.vi[8] = v.vi[8].wrapping_sub(1);
            v.mul(11, 6, 6, Z);
            v.i = TOP255;
            v.add(5, 5, 18, XYZW);
            v.mula_bc(1, 0, LW, XYZW);
            let shown = v.vi[3] != 0;
            v.madd(8, 8, 2, XYZW);
            v.lq(3, top + 18, XYZW);
            let drawn = shown && {
                v.mula_bc(1, 0, LW, XYZW);
                let out = v.vi[1] != 0;
                v.madd(9, 9, 2, XYZW);
                v.lq(4, top + 19, XYZW);
                !out
            };
            if drawn {
                // 0x340: the sprite stored (ST, colour, corner, ST, corner)
                v.mula_bc(6, 12, LX, Z);
                v.vi[12] = v.vi[12].wrapping_add(5);
                v.ftoi4(8, 8, XYZW);
                v.vi[13] = v.vi[13].wrapping_add(1);
                let at = v.vi[12] as u32;
                v.madd_bc(6, 11, 12, LY, Z);
                v.sq(7, at.wrapping_sub(4), XYZW);
                v.ftoi4(9, 9, XYZW);
                v.sq(3, at.wrapping_sub(5), XYZW);
                v.sub_bc(15, 15, 20, LX, W);
                v.sq(4, at.wrapping_sub(2), XYZW);
                v.mini_i(10, 10, XYZW);
                v.sq(8, at.wrapping_sub(3), XYZW);
                v.mula_bc(5, 0, LW, XYZW);
                let full = v.vi[13] == v.vi[14];
                v.madd_bc(5, 17, 6, LZ, XYZW);
                v.sq(9, at.wrapping_sub(1), XYZW);
                if full {
                    send(v);
                    v.vi[1] = v.vi[10];
                    v.vi[10] = v.vi[11];
                    v.vi[11] = v.vi[1];
                    v.vi[13] = 0;
                    v.vi[12] = v.vi[10] + 1;
                }
            } else {
                // 0x490: not drawn (not shown yet, or beyond a plane)
                bend(v);
                v.sub_bc(15, 15, 20, LX, W);
                v.mini_i(10, 10, XYZW);
                v.mula_bc(5, 0, LW, XYZW);
                v.madd_bc(5, 17, 6, LZ, XYZW);
            }
            // 0x3c8: the trail ends when t went below 0 (the flag read at 0x2e0), or after its count of points
            v.sub_bc(2, 15, 15, LY, Z);
            let shown = Vu::mac_flags(v.vf[2], Z);
            let ended = v.vi[5] == v.vi[7];
            v.ftoi0(7, 10, XYZW);
            if ended {
                break;
            }
            v.sub_bc(15, 15, 20, LY, Z);
            flags = (shown, Vu::mac_flags(v.vf[15], Z));
            let more = (v.vi[8] as i16) > 0;
            v.clipw(5, 5);
            v.div(0, LW, 5, LW);
            if !more {
                break;
            }
        }
        // 0x3e8: the countdown (vf19.z less its step) and the next trail's start; all end when the countdown goes
        // below 0, or after the count of trails
        v.sub_bc(19, 19, 19, LW, Z);
        let gone = Vu::mac_flags(v.vf[19], Z);
        v.rinit(20, LZ);
        v.rnext(20, Z);
        v.add_bc(13, 13, 0, LW, X);
        v.rinit(20, LW);
        v.rnext(20, W);
        v.vi[3] = (gone & v.vi[7] as u32) as u16;
        v.mula_bc(24, 0, LW, XYZW);
        v.vi[9] = v.vi[9].wrapping_sub(1);
        v.madda_bc(27, 13, LX, XYZW);
        let stop = v.vi[3] == v.vi[7];
        v.madda_bc(25, 20, LZ, XYZW);
        v.rinit(21, LX);
        if stop {
            break;
        }
        v.madd_bc(18, 26, 20, LW, XYZW);
        let again = (v.vi[9] as i16) > 0;
        v.mula_bc(28, 0, LW, XYZW);
        v.rnext(21, X);
        if !again {
            break;
        }
    }
    if v.vi[13] != 0 {
        send(v);
    }
    // (the last flags made: ACC's)
    v.mac = Vu::mac_flags(v.acc, XYZW);
}

/// The bend's weight: vf6.z = −0.73 m + 0.113 m² for m = min(t, 2.7) (vf6.z before, vf11.z its square)
fn bend(v: &mut Vu) {
    v.mula_bc(6, 12, LX, Z);
    v.madd_bc(6, 11, 12, LY, Z);
}

/// 0x2b0: the point's two tests' flags (whether t is below the fade's start: shown; whether t less its step
/// went below 0: the trail's end), its colour as integers, its clip test and 1/w
fn tests(v: &mut Vu) -> (u32, u32) {
    v.sub_bc(2, 15, 15, LY, Z);
    let shown = Vu::mac_flags(v.vf[2], Z);
    v.ftoi0(7, 10, XYZW);
    v.sub_bc(15, 15, 20, LY, Z);
    let ended = Vu::mac_flags(v.vf[15], Z);
    v.clipw(5, 5);
    v.div(0, LW, 5, LW);
    (shown, ended)
}

//! The VU1 particle programs (board rows G7e2, G7e3): what the particle packets (`particledraw`) call. Program 7
//! (chain 0x31c790, 89 instructions) scatters a system's particles at random in a box and sends each as a sprite.
//! Ported on [`Vu`] in the program's order (each pair's two halves reading the registers as they were before the
//! pair). Checked call by call against the game's program in the runner (`tests/vu1particle.rs`).
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

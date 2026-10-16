//! The front end's video screen (board row G9s): `cFE3DStateVideo`'s methods on the game's bytes, every call through
//! [`FeEnv`], checked in the function runner (`tests/fe_video.rs`).
//!
//! The screen plays one movie (+0x54 its name, +0x158 a word the player is started with) through a video player made
//! on its first update (+0x154; the player's vtable is at its +0). +0x15c is the screen pushed when it ends.

use crate::fe3dstate::{vcall, FeEnv, GAPP, STACK_PUSH};
use crate::pf::Mem;

/// The video player's maker (0x2b9fd8) and the front end's screen stack pop (0x275b90)
pub const PLAYER_CREATE: u32 = 0x2b9fd8;
pub const STACK_POP_TOP: u32 = 0x275b90;

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

/// A virtual call on an object whose vtable is at its +0 (the video player, the 3D scene)
fn vcall0(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// `cFE3DStateVideo::Update` 0x2ab860: with no player yet, one made (0x2b9fd8), started on the movie (its +0x10 with
/// +0x54 and +0x158) and the screen made the front end's video screen (+0xf8); else the player stepped (+0x18) and,
/// when it answers finished (+0x28), the screen's +0x68
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let p = m.r32(s + 0x154);
    if p == 0 {
        let f = fe(m);
        let p = e.call(m, PLAYER_CREATE, &[]);
        m.w32(s + 0x154, p);
        let word = m.r32(s + 0x158);
        vcall0(m, e, p, 0x10, &[s + 0x54, word]);
        m.w32(f + 0xf8, s);
    } else {
        vcall0(m, e, p, 0x18, &[]);
        let p = m.r32(s + 0x154);
        if vcall0(m, e, p, 0x28, &[]) != 0 {
            vcall(m, e, s, 0x68, &[]);
        }
    }
}

/// `cFE3DStateVideo` vf12 0x2ab918, the screen ended: the player asked (+0x30) and released (+8 with 3), the front
/// end's video screen cleared (+0xf8); when the player answered, the 3D scene's +0x50(6); the screen stack popped
/// (0x275b90) and the screen to follow (+0x15c) pushed; the screen itself released (+8 with 3)
pub fn vf12(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let f = fe(m);
    let p = m.r32(s + 0x154);
    let asked = vcall0(m, e, p, 0x30, &[]) != 0;
    let p = m.r32(s + 0x154);
    m.w32(f + 0xf8, 0);
    if p != 0 {
        vcall0(m, e, p, 8, &[3]);
    }
    m.w32(s + 0x154, 0);
    if asked {
        let scene = m.r32(f + 0xa4);
        vcall0(m, e, scene, 0x50, &[6]);
    }
    e.call(m, STACK_POP_TOP, &[f + 0xa8]);
    let next = m.r32(s + 0x15c);
    if next != 0 {
        e.call(m, STACK_PUSH, &[f + 0xa8, next]);
    }
    if s != 0 {
        vcall(m, e, s, 8, &[3]);
    }
}

/// `cFE3DStateVideo::Activate` 0x2ab9f8: the 3D scene's +0x108
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv) {
    let scene = m.r32(fe(m) + 0xa4);
    vcall0(m, e, scene, 0x108, &[]);
}

/// `cFE3DStateVideo::Deactivate` 0x2aba30: the front end's +0xf4 cleared, the 3D scene's +0x110
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv) {
    let f = fe(m);
    let scene = m.r32(f + 0xa4);
    m.w32(f + 0xf4, 0);
    vcall0(m, e, scene, 0x110, &[]);
}

/// `cFE3DStateVideo` vf27 0x2abb20: the player's +0x20(-1)
pub fn vf27(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let p = m.r32(s + 0x154);
    vcall0(m, e, p, 0x20, &[u32::MAX]);
}

/// 0x2aba70: the word the player is started with (+0x158)
pub fn set_start_word(m: &mut Mem, s: u32, v: u32) {
    m.w32(s + 0x158, v);
}

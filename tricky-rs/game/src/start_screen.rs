//! The start screen (`cStartScreen` and `cStartScreenSingle`, the module the application opens first) on the
//! game's bytes (board rows E1f1, E1f2): its enter, frame and leave, its draws, its state machine and the small
//! functions around it. Every call out goes through [`FeEnv`]. Checked in the function runner
//! (`tests/start_screen.rs`).

use crate::fe3dstate::{FeEnv, GAPP};
use crate::game_main as gm;
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: the font's load (0x19c858), make (0x19c788), destroy (0x19c808) and release (0x19c8d0); the
/// sprite sets' reset (0x1f1580) and pick (0x1f9970) on the app's +0x728; the memory card's make (0x23d6e0) and free
/// (0x23d738); the renderer's colour (0x1e0db8) and alpha (0x1e0fb8); the start module's constructor (0x2455c0); the
/// app's module switch (0x17c150); the career's start (0x163358); the textured rectangle (0x19b730); the sprite set's
/// rectangle (0x1f9e50) and its draw (0x1fc178); the text box's make (0x1957b0) and destroy (0x14f390)
pub const FONT_LOAD: u32 = 0x19c858;
pub const FONT_CTOR: u32 = 0x19c788;
pub const FONT_DTOR: u32 = 0x19c808;
pub const FONT_RELEASE: u32 = 0x19c8d0;
pub const SPRITES_RESET: u32 = 0x1f1580;
pub const SPRITES_PICK: u32 = 0x1f9970;
pub const CARD_NEW: u32 = 0x23d6e0;
pub const CARD_FREE: u32 = 0x23d738;
pub const REND_COLOUR: u32 = 0x1e0db8;
pub const REND_ALPHA: u32 = 0x1e0fb8;
pub const START_MODULE_CTOR: u32 = 0x2455c0;
pub const APP_SWITCH: u32 = 0x17c150;
pub const CAREER_START: u32 = 0x163358;
pub const TEXTURED_RECT: u32 = 0x19b730;
pub const SPRITE_SET_RECT: u32 = 0x1f9e50;
pub const SPRITE_DRAW: u32 = 0x1fc178;
pub const TEXT_BOX_CTOR: u32 = 0x1957b0;
pub const TEXT_BOX_DTOR: u32 = 0x14f390;
pub const FONT_NAME: u32 = 0x387540;
pub const START_MODULE_NAME: u32 = 0x387558;
pub const SINGLE_NAME: u32 = 0x387670;
pub const SCREEN_VTABLE: u32 = 0x387688;
pub const SINGLE_VTABLE: u32 = 0x387950;
/// A records' entry kept at 0x3cdd28
pub const RECORD: u32 = 0x3cdd28;

fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x724))
}

fn sprites(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x728))
}

/// A virtual call through a table at `at` of the object
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// The renderer's virtual at `off` (its table at +0x10)
fn rend(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, off, args)
}

/// `cvt.s.w`: toward zero
fn int_float(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

/// An unsigned word as a float (`cvt.s.w` of half with the low bit kept, doubled, past 2^31)
fn unsigned_float(v: u32) -> F {
    if (v as i32) >= 0 {
        int_float(v)
    } else {
        let h = int_float((v >> 1) | (v & 1));
        h + h
    }
}

const ONE: u32 = 0x3f80_0000;
const W640: u32 = 0x4420_0000;
const H480: u32 = 0x43f0_0000;

/// `cStartScreen` vf1 0x182d08(s), the enter: the renderer's +0x138 with 0, the font loaded at +8 and sized
/// (its +0x18 1.8, +0x1c 1.4), the sprite sets reset and set 0x2c9 picked, the blink (+0x3ca8 1/60.. its step
/// +0x3cac 1) and +0x3cb0 cleared
pub fn enter(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    rend(m, e, 0x138, &[0]);
    let font = s + 8;
    e.call(m, FONT_LOAD, &[font, FONT_NAME, 0]);
    m.w32(font + 0x1c, 0x3fb3_3333);
    m.w32(font + 0x18, 0x3fe6_6666);
    let ss = sprites(m);
    e.call(m, SPRITES_RESET, &[ss]);
    let ss = sprites(m);
    e.call(m, SPRITES_PICK, &[ss, 0x2c9, 0]);
    m.w32(s + 0x3ca8, 0x3c88_8f86);
    m.w32(s + 0x3cac, ONE);
    m.w32(s + 0x3cb0, 0);
}

/// `cStartScreen` vf4 0x182dd8(s; `buf` 0xc bytes), the start: the renderer's +0x140, the memory card made (+4),
/// the colour white and the alpha 0; the counters (+0x14b8, +0x14c8, +0x14cc, +0x14bc, +0x14c0) and +0x44 cleared;
/// the start module made (0x410 bytes, filled with 0xdeadc0de when there is one) and switched to; +0x3ca4 1
pub fn start(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let r = renderer(m);
    vcall_at(m, e, r, 0x10, 0x140, &[r]);
    let card = e.call(m, CARD_NEW, &[]);
    let r = renderer(m);
    m.w32(s + 4, card);
    m.w32(buf + 8, ONE);
    m.w32(buf, ONE);
    m.w32(buf + 4, ONE);
    e.call(m, REND_COLOUR, &[r, buf]);
    e.call(m, REND_ALPHA, &[r, 0]);
    let app = m.r32(GAPP);
    for at in [0x14b8u32, 0x14c8, 0x14cc, 0x14bc, 0x14c0, 0x44] {
        m.w32(s + at, 0);
    }
    let p = e.call(m, gm::ALLOC, &[START_MODULE_NAME, 0x410, 0x10]);
    if p != 0 {
        e.call(m, gm::FILL, &[p, 0xdead_c0de, 0x410]);
    }
    let x = e.call(m, START_MODULE_CTOR, &[p]);
    e.call(m, gm::APP_SET_MODE, &[app, x]);
    m.w32(s + 0x3ca4, 1);
}

/// The screen set up for 640 by 480 (both draws): the viewport over the whole display (its +0x58 by +0x60,
/// unsigned), the depth range (0, 0, 640, 480, 0, 1000), the clear, fog 0.08 at state 2, state 6, the font's scale
/// 0.01, texture mode 3, the alpha, and the font black
fn screen_setup(m: &mut Mem, e: &mut dyn FeEnv, buf: u32) {
    let w = unsigned_float(rend(m, e, 0x58, &[]));
    let h = unsigned_float(rend(m, e, 0x60, &[]));
    let z = 0u32;
    rend(m, e, 0x98, &[z, z, w.0.to_bits(), h.0.to_bits()]);
    rend(m, e, 0xb0, &[z, z, W640, H480, z, F(1000.0).0.to_bits()]);
    rend(m, e, 0x108, &[]);
    rend(m, e, 0x1c8, &[2, 0, 0x3da3_d70a]);
    rend(m, e, 0x1d0, &[6]);
    m.w32(gm::FONT_STATE + 0x10, 0x3c23_d70a);
    rend(m, e, 0x1e0, &[3]);
    rend(m, e, 0x1f0, &[u32::MAX, 0]);
    black(m, e, buf);
}

fn black(m: &mut Mem, e: &mut dyn FeEnv, buf: u32) {
    for k in 0..3u32 {
        m.w32(buf + 4 * k, 0);
    }
    e.call(m, gm::FONT_COLOUR, &[gm::FONT_STATE, buf]);
}

/// `cStartScreen` vf2 0x182ed0(s; `buf` 0xc bytes), the leave -> whether it left: nothing until +0x44 is set.
/// Waits for the renderer (+0x70), sets up the screen and the font's area (0, 0, 640, 480), draws a frame (+0x80)
/// and waits for it (+0x78); then the card's +0x60, the card freed, the app switched (with 0) and +4 cleared; the
/// career started unless +0x3cb0; the font released, the renderer's +0x148 and +0x3ca4 cleared
pub fn leave(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    if m.r32(s + 0x44) == 0 {
        return 0;
    }
    while rend(m, e, 0x70, &[]) == 0 {}
    screen_setup(m, e, buf);
    e.call(m, gm::FONT_SIZE, &[gm::FONT_STATE, 0, 0, W640, H480]);
    rend(m, e, 0x80, &[]);
    while rend(m, e, 0x78, &[]) == 0 {}
    let card = m.r32(s + 4);
    vcall_at(m, e, card, 0, 0x60, &[]);
    let card = m.r32(s + 4);
    e.call(m, CARD_FREE, &[card]);
    let app = m.r32(GAPP);
    m.w32(s + 4, 0);
    e.call(m, APP_SWITCH, &[app, 0]);
    if m.r32(s + 0x3cb0) == 0 {
        e.call(m, CAREER_START, &[gm::CAREER]);
    }
    nothing();
    e.call(m, FONT_RELEASE, &[s + 8]);
    rend(m, e, 0x148, &[]);
    m.w32(s + 0x3ca4, 0);
    1
}

/// `cStartScreen` vf6 0x1831d8(s; `buf` 0xc bytes), the draw -> whether it drew: nothing unless the renderer is
/// ready (+0x70). The screen set up, a white rectangle (0, 0, 1, 1, all ones), the font black again, sprite 0x28 of
/// the sets drawn over (0, 0, 1, 1); the screen's own +0x40 unless +0x44; then the frame (+0x80)
pub fn draw(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    if rend(m, e, 0x70, &[]) == 0 {
        return 0;
    }
    screen_setup(m, e, buf);
    e.call(m, TEXTURED_RECT, &[gm::FONT_STATE, 0, 0, ONE, ONE, ONE, ONE, ONE, ONE]);
    black(m, e, buf);
    let ss = sprites(m);
    let sp = e.call(m, SPRITE_SET_RECT, &[ss, 0x28]);
    e.call(m, SPRITE_DRAW, &[sp, 0, 0, 0, ONE, ONE]);
    if m.r32(s + 0x44) == 0 {
        vcall_at(m, e, s, 0, 0x40, &[]);
    }
    rend(m, e, 0x80, &[]);
    1
}

/// `StartScreenSingle_Create` 0x184318 -> the screen: 0x3cc8 bytes (filled with 0xdeadc0de when there are), its
/// font (+8) and text box (+0x48) made, the single screen's table and +0x3ca4 cleared
pub fn create(m: &mut Mem, e: &mut dyn FeEnv) -> u32 {
    let p = e.call(m, gm::ALLOC, &[SINGLE_NAME, 0x3cc8, 0]);
    if p != 0 {
        e.call(m, gm::FILL, &[p, 0xdead_c0de, 0x3cc8]);
    }
    m.w32(p, SCREEN_VTABLE);
    e.call(m, FONT_CTOR, &[p + 8, 0]);
    e.call(m, TEXT_BOX_CTOR, &[p + 0x48]);
    m.w32(p, SINGLE_VTABLE);
    m.w32(p + 0x3ca4, 0);
    p
}

/// 0x1843a0 (and the card's draw hook 0x184578): nothing
pub fn nothing() {}

/// 0x1843a8: the record at 0x3cdd28 reset (-1, 1, 1, 0, 0, 1, 1)
pub fn reset_record(m: &mut Mem) {
    for (k, v) in [u32::MAX, ONE, ONE, 0, 0, ONE, ONE].into_iter().enumerate() {
        m.w32(RECORD + 4 * k as u32, v);
    }
}

/// 0x1843e0(set, flip; x, y, w, h): the sprite drawn with its width and height scaled by the set's +4 and +8
pub fn draw_scaled(m: &mut Mem, e: &mut dyn FeEnv, set: u32, flip: u32, x: F, y: F, w: F, h: F) {
    let sx = F(f32::from_bits(m.r32(set + 4)));
    let sy = F(f32::from_bits(m.r32(set + 8)));
    let (w, h) = (sx * w, sy * h);
    e.call(m, SPRITE_DRAW, &[set, flip, x.0.to_bits(), y.0.to_bits(), w.0.to_bits(), h.0.to_bits()]);
}

/// `cStartScreen` vf3 0x1843f8(s) -> +0x3ca4 (whether it runs)
pub fn running(m: &Mem, s: u32) -> u32 {
    m.r32(s + 0x3ca4)
}

/// `cStartScreen::~cStartScreen` 0x184450(s, flags) (and the single screen's, 0x1859b8): the text box and font
/// destroyed (with 2), then the module's destructor
pub fn screen_dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flags: u32) {
    e.call(m, TEXT_BOX_DTOR, &[s + 0x48, 2]);
    e.call(m, FONT_DTOR, &[s + 8, 2]);
    gm::module_dtor(m, e, s, flags);
}

/// 0x184508(o, flags): freed when bit 0 is set
pub fn free_if(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, gm::FREE, &[o]);
    }
}

/// 0x184580 (a copy of `Color_SetRGB` 0x182040)(c; r, g, b): alpha 1 then the three
pub fn set_rgb(m: &mut Mem, c: u32, r: F, g: F, b: F) {
    m.w32(c + 0xc, b.0.to_bits());
    m.w32(c, ONE);
    m.w32(c + 4, r.0.to_bits());
    m.w32(c + 8, g.0.to_bits());
}

/// 0x184528(c, rgb): `Color_SetRGB` from three floats
pub fn set_rgb_from(m: &mut Mem, c: u32, rgb: u32) {
    let f = |k: u32| F(f32::from_bits(m.r32(rgb + 4 * k)));
    let (r, g, b) = (f(0), f(1), f(2));
    set_rgb(m, c, r, g, b);
}

/// `cStartScreenSingle` vf4 0x1845a0(s; `buf`): the screen's start, then its state (+0x3cc4), the wait (+0x14c4)
/// and the card's answers (+0x3cb4, +0x3cb8, +0x3cbc, +0x3cc0) cleared
pub fn single_start(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    start(m, e, s, buf);
    for at in [0x3cc0u32, 0x3cc4, 0x14c4, 0x3cb4, 0x3cb8, 0x3cbc] {
        m.w32(s + at, 0);
    }
}

/// `cStartScreenSingle` vf6 0x185410(s; `buf`) -> 0: the screen's draw
pub fn single_draw(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    draw(m, e, s, buf);
    0
}

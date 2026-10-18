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

/// E1f2's calls out: the input's device by index (0x17a830); the save's game code (0x1d2f60) and the card's tags
/// (0x1d2fc8 the set, 0x1d2fa0 the repeat) and `sprintf` (0x2fca28) with the path's format (0x387840); the
/// CRC (0x2ce04c) and copy (0x2ce114) of the career block; the audio's delete (0x20dad0) and create (0x20da50) and the
/// profile's audio settings (0x161a98); the display's brightness (0x1e0eb8); the text box's reset (0x195ee8), font
/// (0x196028) and scale (0x1961f0); the wide strings' format (0x17ce38), ASCII format (0x17ce70) and copy (0x17cc80)
/// with the two formats at 0x3878c0 and 0x3878c8; the localised strings (0x2c5160); the memory card message box
/// (0x183538)
pub const INPUT_DEVICE: u32 = 0x17a830;
pub const GAME_CODE: u32 = 0x1d2f60;
pub const TAG_SET: u32 = 0x1d2fc8;
pub const TAG_REP: u32 = 0x1d2fa0;
pub const SPRINTF: u32 = 0x2fca28;
pub const PATH_FORMAT: u32 = 0x387840;
pub const CRC16: u32 = 0x2ce04c;
pub const MEM_COPY: u32 = 0x2ce114;
pub const AUDIO_DELETE: u32 = 0x20dad0;
pub const AUDIO_CREATE: u32 = 0x20da50;
pub const AUDIO_SETTINGS: u32 = 0x161a98;
pub const BRIGHTNESS: u32 = 0x1e0eb8;
pub const TB_RESET: u32 = 0x195ee8;
pub const TB_FONT: u32 = 0x196028;
pub const TB_SCALE: u32 = 0x1961f0;
pub const WSTR_FORMAT: u32 = 0x17ce38;
pub const WSTR_FORMAT_ASCII: u32 = 0x17ce70;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const TWO_FORMAT: u32 = 0x3878c0;
pub const THREE_FORMAT: u32 = 0x3878c8;
pub const DRAW_BOX: u32 = 0x183538;
/// The career block's size, and the flag cleared when the card's block has none (0x32ca34)
pub const BLOCK_SIZE: u32 = 0x27d4;
pub const NO_BLOCK: u32 = 0x32ca34;

/// The memory card's virtual at `off` (its table at +0 of the screen's +4)
fn card(m: &mut Mem, e: &mut dyn FeEnv, s: u32, off: u32, args: &[u32]) -> u32 {
    let c = m.r32(s + 4);
    vcall_at(m, e, c, 0, off, args)
}

fn i(m: &Mem, a: u32) -> i32 {
    m.r32(a) as i32
}

/// `Input_GetAnyNewButtons` 0x1834a8(-) -> the buttons new this frame on any device (the input's +0x14 of them):
/// each device's +0x10 without its +0xc
pub fn any_new_buttons(m: &mut Mem, e: &mut dyn FeEnv) -> u32 {
    let mut b = 0u32;
    let mut k = 0u32;
    if (m.r32(m.r32(m.r32(GAPP) + 0x24) + 0x14) as i32) > 0 {
        loop {
            let input = m.r32(m.r32(GAPP) + 0x24);
            let d = e.call(m, INPUT_DEVICE, &[input, k]);
            k += 1;
            let held = m.r32(d + 0xc);
            let now = m.r32(d + 0x10);
            let n = m.r32(m.r32(m.r32(GAPP) + 0x24) + 0x14);
            b |= now & !held;
            if (k as i32) >= n as i32 {
                break;
            }
        }
    }
    b
}

/// The card's file name made in `buf` (+0x50 or +0x60 the game code, +0x40 the tag, then `sprintf` at +0)
fn file_name(m: &mut Mem, e: &mut dyn FeEnv, buf: u32, code_at: u32, tag: u32) {
    let code = buf + code_at;
    e.call(m, GAME_CODE, &[code, 0x10]);
    let t = buf + 0x40;
    e.call(m, tag, &[t, 8]);
    e.call(m, SPRINTF, &[buf, PATH_FORMAT, code, t]);
}

/// `StartScreenSingle_SetState` 0x1850e8(s, state; `buf` 0x70 bytes): the state kept (+0x3cc4); then by the state:
/// 0 the wait (+0x14c4) 15; 1 and 11 the timer (+0x14b8) 75, +0x14cc cleared and the card's +0x28 (0); 2 the card
/// asked in turn (+0x170 there, +0x160, +0x178, +0x190; +0x150) for the next state (0x19, 3, 0x11 or 0xd; 0x18 or
/// 0x17); 3 the file name with the set tag, the card's +0x80 (the block's size) and +0x30 (the name); 5 the block
/// (+0x14d0) cleared and the card's +0xb8 into it; 8 the wait 120; 9 the file name with the repeat tag, the card's
/// +0x80 (0x80000) and +0x30, the timer 75; 13, 17, 23 and 24 the timer 180; 14 the timer 180, the card's +0x40 and
/// the blink (+0x3ca8 1, its step 1/60); 15 the timer 120; 27 +0x44 set
pub fn set_state(m: &mut Mem, e: &mut dyn FeEnv, s: u32, st: u32, buf: u32) {
    m.w32(s + 0x3cc4, st);
    match st {
        0 => m.w32(s + 0x14c4, 0xf),
        1 | 11 => {
            m.w32(s + 0x14b8, 0x4b);
            m.w32(s + 0x14cc, 0);
            card(m, e, s, 0x28, &[0]);
        }
        2 => {
            let next = if card(m, e, s, 0x170, &[]) != 0 {
                if card(m, e, s, 0x160, &[]) != 0 {
                    0x19
                } else if card(m, e, s, 0x178, &[]) != 0 {
                    3
                } else if card(m, e, s, 0x190, &[]) == 0 {
                    0x11
                } else {
                    0xd
                }
            } else if card(m, e, s, 0x150, &[]) != 0 {
                0x18
            } else {
                0x17
            };
            set_state(m, e, s, next, buf);
        }
        3 => {
            file_name(m, e, buf, 0x50, TAG_SET);
            card(m, e, s, 0x80, &[BLOCK_SIZE]);
            card(m, e, s, 0x30, &[buf]);
        }
        5 => {
            e.call(m, gm::FILL, &[s + 0x14d0, 0, BLOCK_SIZE]);
            card(m, e, s, 0xb8, &[0, s + 0x14d0, BLOCK_SIZE]);
        }
        8 => m.w32(s + 0x14c4, 0x78),
        9 => {
            file_name(m, e, buf, 0x60, TAG_REP);
            card(m, e, s, 0x80, &[0x80000]);
            card(m, e, s, 0x30, &[buf]);
            m.w32(s + 0x14b8, 0x4b);
        }
        13 | 17 | 23 | 24 => m.w32(s + 0x14b8, 0xb4),
        14 => {
            m.w32(s + 0x14b8, 0xb4);
            card(m, e, s, 0x40, &[]);
            m.w32(s + 0x3ca8, ONE);
            m.w32(s + 0x3cac, 0x3c88_8f86);
        }
        15 => m.w32(s + 0x14b8, 0x78),
        27 => m.w32(s + 0x44, 1),
        _ => {}
    }
}

/// `cStartScreenSingle` vf5 0x1845e0(s; `buf` 0x70 bytes), the frame. The blink (+0x3ca8) moves by its step
/// (+0x3cac) and turns back past 0.2 or 1. Nothing more once +0x44 is set. Otherwise the card's update (+0x18), and
/// when it asks (+0x1a8) +0x14c8 set and its +0x140; the new buttons; the timer (+0x14b8) and the wait (+0x14c4) count
/// down. In state 6 the card's +0xd8 unless it is busy (+0x58) and +0x14c8 clear. When the card is not busy: with
/// +0x14c8 set it is cleared and state 1 taken; in state 9 with time left the card's +0x28 (0); outside the states
/// that wait on it the card is asked whether it changed (+0x1e8) and, when it did, which state it is in now (the same
/// questions as state 2's, here 0x18 / 0x17, then 0x19 (which goes on), 0xd, 0x11 or 0x1a). Then the state's step
/// (see [`state_step`])
pub fn single_frame(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let step = F(f32::from_bits(m.r32(s + 0x3cac)));
    let v = F(f32::from_bits(m.r32(s + 0x3ca8))) + step;
    m.w32(s + 0x3ca8, v.0.to_bits());
    let (lim, past) = if step.ee() < 0.0 {
        let lim = F(f32::from_bits(0x3e4c_cccd));
        (lim, v.ee() < lim.ee())
    } else {
        let lim = F(1.0);
        (lim, lim.ee() < v.ee())
    };
    if past {
        m.w32(s + 0x3ca8, lim.0.to_bits());
        m.w32(s + 0x3cac, step.0.to_bits() ^ 0x8000_0000);
    }
    if m.r32(s + 0x44) != 0 {
        return;
    }
    card(m, e, s, 0x18, &[]);
    if card(m, e, s, 0x1a8, &[]) != 0 {
        m.w32(s + 0x14c8, 1);
        card(m, e, s, 0x140, &[]);
    }
    let buttons = any_new_buttons(m, e);
    for at in [0x14b8u32, 0x14c4] {
        let t = i(m, s + at);
        if t > 0 {
            m.w32(s + at, (t - 1) as u32);
        }
    }
    if m.r32(s + 0x3cc4) == 6 && (card(m, e, s, 0x58, &[]) == 0 || m.r32(s + 0x14c8) != 0) {
        card(m, e, s, 0xd8, &[]);
    }
    if card(m, e, s, 0x58, &[]) == 0 {
        if m.r32(s + 0x14c8) != 0 {
            m.w32(s + 0x14c8, 0);
            set_state(m, e, s, 1, buf);
            return;
        }
        let st = m.r32(s + 0x3cc4);
        if st == 9 && i(m, s + 0x14b8) > 0 {
            card(m, e, s, 0x28, &[0]);
            return;
        }
        if ![1u32, 0x13, 0x15, 0x12, 0x16, 0x14, 0xe, 0xf, 0x10].contains(&st) && card(m, e, s, 0x1e8, &[]) != 0 {
            let (next, goes_on) = if card(m, e, s, 0x170, &[]) == 0 {
                (if card(m, e, s, 0x150, &[]) != 0 { 0x18 } else { 0x17 }, false)
            } else if card(m, e, s, 0x160, &[]) != 0 {
                (0x19, true)
            } else if card(m, e, s, 0x178, &[]) == 0 {
                (0xd, false)
            } else if card(m, e, s, 0x190, &[]) == 0 {
                (0x11, false)
            } else {
                (0x1a, false)
            };
            if goes_on {
                set_state(m, e, s, next, buf);
            } else if st != next {
                set_state(m, e, s, next, buf);
                return;
            }
        }
    }
    state_step(m, e, s, buttons, buf);
}

/// vf5's step by the state (+0x3cc4), with the new buttons `b` (0x880 back, 0x40 and 0x10 the choices, 0x120 yes):
/// - 0: state 1. 1: once the card is idle, state 2 when the timer is out, else the card's +0x28 (0).
/// - 3: once idle, the card's count (+0x68, kept at +0x3cb4) and the block's size (+0x98, +0x3cbc): with files, 0x12
///   when the first is bad (+0x1b8), else 5 (+0x14cc set); without, 0x13 when the card's room (+0x88) is short
///   (+0x14bc the size), else 9.
/// - 5: state 6 when the card has read (+0xc8 1). 6: then its +0xd0; with a block (+0x2b6c) whose CRC is the one kept
///   at +0x3c9c (cleared) the career takes it: the audio set up again when the card's system (+0x2b38) is new, the
///   audio settings, +0x3cb0 set, the brightness (+0x2b7c + 0.5, from 0 to 2) and colour, the renderer's +0x120 with
///   +0x2b4c; a bad CRC goes to 0x12; no block clears 0x32ca34. Then state 7.
/// - 7: state 8 once idle. 8: state 9 when the wait is out. 9: once idle with the timer out: the card's count (+0x68,
///   +0x3cb8) and room for the repeat (+0x98, +0x3cc0); any bad file (+0x1b8) 0x14; four or more 0x16; otherwise
///   +0x14c0 its +0x68 and 0x15 when the room (+0x88) is short (+0x14bc), else 0x16.
/// - 11: state 12 once idle with the timer out. 12: state 2.
/// - 13: back clears the timer; out of time 0x16; else the card asked again (0x18 / 0x17, or 0x19).
/// - 14: once idle: 0x10 when the card agrees (+0x48); with the timer out, 0xf otherwise.
/// - 15: 0x1b when the timer is out. 16: back 0x1b, 0x40 state 1. 17: and 0x10 state 14.
/// - 18, 19: yes 11, back 9. 20, 21, 26: yes 11, back 0x16. 22: 0x1b unless no files, nothing on the card and it is
///   ready (+0xb0 1). 23..25: back clears the timer; out of time 0x16, else 11 when the card is there (+0x170).
pub fn state_step(m: &mut Mem, e: &mut dyn FeEnv, s: u32, b: u32, buf: u32) {
    let busy = |m: &mut Mem, e: &mut dyn FeEnv| card(m, e, s, 0x58, &[]) != 0;
    let next = match m.r32(s + 0x3cc4) {
        0 => 1,
        1 => {
            if busy(m, e) {
                return;
            }
            if i(m, s + 0x14b8) <= 0 {
                2
            } else {
                card(m, e, s, 0x28, &[0]);
                return;
            }
        }
        3 => {
            if busy(m, e) {
                return;
            }
            let n = card(m, e, s, 0x68, &[]);
            m.w32(s + 0x3cb4, n);
            let size = card(m, e, s, 0x98, &[BLOCK_SIZE]);
            m.w32(s + 0x3cbc, size);
            if i(m, s + 0x3cb4) > 0 {
                if card(m, e, s, 0x1b8, &[0]) != 0 {
                    m.w32(s + 0x14cc, 0);
                    0x12
                } else {
                    m.w32(s + 0x14cc, 1);
                    5
                }
            } else {
                m.w32(s + 0x14cc, 0);
                let room = card(m, e, s, 0x88, &[]) as i32;
                let size = m.r32(s + 0x3cbc);
                if room < size as i32 {
                    m.w32(s + 0x14bc, size);
                    0x13
                } else {
                    9
                }
            }
        }
        5 => {
            if card(m, e, s, 0xc8, &[]) != 1 {
                return;
            }
            6
        }
        6 => {
            if card(m, e, s, 0xc8, &[]) != 1 {
                return;
            }
            card(m, e, s, 0xd0, &[]);
            if m.r32(s + 0x2b6c) != 0 {
                let kept = m.r32(s + 0x3c9c);
                m.w32(s + 0x3c9c, 0);
                if e.call(m, CRC16, &[s + 0x14d0, BLOCK_SIZE]) != kept {
                    set_state(m, e, s, 0x12, buf);
                    return;
                }
                e.call(m, MEM_COPY, &[gm::CAREER + 8, s + 0x14d0, BLOCK_SIZE]);
                let sys = m.r32(s + 0x2b38);
                let so = m.r32(gm::SOUND);
                if vcall_at(m, e, so, 4, 0x10, &[sys]) == 1 {
                    e.call(m, AUDIO_DELETE, &[]);
                    e.call(m, AUDIO_CREATE, &[sys]);
                }
                e.call(m, AUDIO_SETTINGS, &[gm::CAREER]);
                let app = m.r32(GAPP);
                m.w32(s + 0x3cb0, 1);
                let r = m.r32(app + 0x724);
                e.call(m, BRIGHTNESS, &[buf, r]);
                let mut f = F(f32::from_bits(m.r32(s + 0x2b7c))) + F(0.5);
                if f.ee() < 0.0 {
                    f = F(0.0);
                } else if 2.0 < f.ee() {
                    f = F(2.0);
                }
                m.w32(r + 0x67c, f.0.to_bits());
                for k in 0..3u32 {
                    let v = m.r32(buf + 4 * k);
                    m.w32(buf + 0x10 + 4 * k, v);
                }
                e.call(m, REND_COLOUR, &[r, buf + 0x10]);
                let w = m.r32(s + 0x2b4c);
                rend(m, e, 0x120, &[w]);
            } else {
                m.w32(NO_BLOCK, 0);
            }
            7
        }
        7 => {
            if busy(m, e) {
                return;
            }
            8
        }
        8 => {
            if i(m, s + 0x14c4) > 0 {
                return;
            }
            9
        }
        9 => {
            if busy(m, e) || i(m, s + 0x14b8) > 0 {
                return;
            }
            let mut bad = false;
            let n = card(m, e, s, 0x68, &[]);
            m.w32(s + 0x3cb8, n);
            let room = card(m, e, s, 0x98, &[0x80000]);
            m.w32(s + 0x3cc0, room);
            if i(m, s + 0x3cb8) > 0 {
                let mut k = 0u32;
                loop {
                    if card(m, e, s, 0x1b8, &[k]) != 0 {
                        bad = true;
                    }
                    k += 1;
                    if (k as i32) >= i(m, s + 0x3cb8) {
                        break;
                    }
                }
            }
            if bad {
                m.w32(s + 0x14cc, 0);
                0x14
            } else if i(m, s + 0x3cb8) >= 4 {
                0x16
            } else {
                let x = card(m, e, s, 0x68, &[]);
                m.w32(s + 0x14c0, x);
                let need = m.r32(s + 0x3cc0);
                if (card(m, e, s, 0x88, &[]) as i32) < need as i32 {
                    m.w32(s + 0x14bc, need);
                    0x15
                } else {
                    0x16
                }
            }
        }
        11 => {
            if busy(m, e) || i(m, s + 0x14b8) > 0 {
                return;
            }
            0xc
        }
        12 => 2,
        13 => {
            if b & 0x880 != 0 {
                m.w32(s + 0x14b8, 0);
            }
            if i(m, s + 0x14b8) <= 0 {
                0x16
            } else if card(m, e, s, 0x170, &[]) == 0 {
                if card(m, e, s, 0x150, &[]) != 0 { 0x18 } else { 0x17 }
            } else if card(m, e, s, 0x160, &[]) != 0 {
                0x19
            } else {
                return;
            }
        }
        14 => {
            if busy(m, e) {
                return;
            }
            if i(m, s + 0x14b8) > 0 {
                if card(m, e, s, 0x48, &[]) == 0 {
                    return;
                }
                0x10
            } else if card(m, e, s, 0x48, &[]) != 0 {
                0x10
            } else {
                0xf
            }
        }
        15 => {
            if i(m, s + 0x14b8) > 0 {
                return;
            }
            0x1b
        }
        16 | 17 => {
            if b & 0x880 != 0 {
                0x1b
            } else if b & 0x40 != 0 {
                1
            } else if m.r32(s + 0x3cc4) == 17 && b & 0x10 != 0 {
                0xe
            } else {
                return;
            }
        }
        18 | 19 => {
            if b & 0x120 != 0 {
                0xb
            } else if b & 0x880 != 0 {
                9
            } else {
                return;
            }
        }
        20 | 21 | 26 => {
            if b & 0x120 != 0 {
                0xb
            } else if b & 0x880 != 0 {
                0x16
            } else {
                return;
            }
        }
        22 => {
            if m.r32(s + 0x3cb4) == 0 && m.r32(s + 0x3cb8) == 0 && card(m, e, s, 0xb0, &[]) == 1 {
                return;
            }
            0x1b
        }
        23 | 24 | 25 => {
            if b & 0x880 != 0 {
                m.w32(s + 0x14b8, 0);
            }
            if i(m, s + 0x14b8) <= 0 {
                0x16
            } else if card(m, e, s, 0x170, &[]) != 0 {
                0xb
            } else {
                return;
            }
        }
        _ => return,
    };
    set_state(m, e, s, next, buf);
}

/// The localised string `id` (the app's strings, +0x50)
fn loc(m: &mut Mem, e: &mut dyn FeEnv, id: u32) -> u32 {
    let strings = m.r32(m.r32(GAPP) + 0x50);
    e.call(m, crate::app::LOC_STRING, &[strings, id])
}

/// `cStartScreenSingle` vf7 0x185430(s; `buf` 0x600 bytes), the message: the text box (+0x48) reset, given the font
/// (+8), its +0x6c cleared and scaled 1; then by the state the message drawn in the card's box (0x183538: the text,
/// the box's kind, whether it waits). Most states centre the box (its +0x20 1, +0x24 0) and show one string: 0x178
/// ("checking", with 1 in it) in 0, 1 (once the wait is out), 3..5 and 9..12; 0x147 in 6..8; 0x180 in 13; 0x164 in
/// 15 (not centred); 0x17f in 23..25. With two: 0x17b or 0x179 and 0x184 in 18 and 26; 0x181 (not unquoted) and
/// 0x184 in 20; 0x17d with the size (+0x14bc) and 0x184 in 19; 0x17e with the size, then 0x183 or 0x182 with the
/// count (+0x14c0, when not 1), and 0x184 in 21; these are kind 5. In 16 (kind 0x22, no wait) 0x166 with 0xde3 when
/// the card is formatted wrong (+0x160), with 0x19a when it is missing (+0x170 0), else alone. In 22 the card's +0xb0
/// unless it has files or anything on it
pub fn single_message(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let t = s + 0x48;
    e.call(m, TB_RESET, &[t]);
    e.call(m, TB_FONT, &[t, s + 8]);
    m.w32(t + 0x6c, 0);
    e.call(m, TB_SCALE, &[t, ONE, ONE]);
    let centre = |m: &mut Mem| {
        m.w32(t + 0x24, 0);
        m.w32(t + 0x20, 1);
    };
    let unq = |m: &mut Mem, e: &mut dyn FeEnv, id: u32| crate::app::loc_unquoted(m, e, id);
    let (b2, b4) = (buf + 0x200, buf + 0x400);
    let (text, kind, wait) = match m.r32(s + 0x3cc4) {
        1 if i(m, s + 0x14c4) > 0 => return,
        0 | 1 | 3 | 4 | 5 | 9 | 10 | 11 | 12 => {
            centre(m);
            let x = unq(m, e, 0x178);
            e.call(m, WSTR_FORMAT, &[b2, x, 1]);
            (b2, 0, 1)
        }
        st @ (6 | 7 | 8 | 13 | 23 | 24 | 25) => {
            centre(m);
            let id = match st {
                6..=8 => 0x147,
                13 => 0x180,
                _ => 0x17f,
            };
            (unq(m, e, id), 0, 1)
        }
        15 => (unq(m, e, 0x164), 0, 1),
        16 => {
            if card(m, e, s, 0x160, &[]) != 0 {
                let a = loc(m, e, 0x166);
                let c = loc(m, e, 0xde3);
                e.call(m, WSTR_FORMAT_ASCII, &[b2, TWO_FORMAT, a, c]);
            } else if card(m, e, s, 0x170, &[]) == 0 {
                let a = loc(m, e, 0x166);
                let c = loc(m, e, 0x19a);
                e.call(m, WSTR_FORMAT_ASCII, &[b2, TWO_FORMAT, a, c]);
            } else {
                let a = loc(m, e, 0x166);
                e.call(m, WSTR_COPY, &[b2, a]);
            }
            (b2, 0x22, 0)
        }
        st @ (18 | 20 | 26) => {
            centre(m);
            let a = match st {
                18 => unq(m, e, 0x17b),
                20 => loc(m, e, 0x181),
                _ => unq(m, e, 0x179),
            };
            let c = unq(m, e, 0x184);
            e.call(m, WSTR_FORMAT_ASCII, &[b2, TWO_FORMAT, a, c]);
            (b2, 5, 1)
        }
        19 => {
            centre(m);
            let a = unq(m, e, 0x17d);
            let n = m.r32(s + 0x14bc);
            e.call(m, WSTR_FORMAT, &[b2, a, n]);
            let c = unq(m, e, 0x184);
            e.call(m, WSTR_FORMAT_ASCII, &[b4, TWO_FORMAT, b2, c]);
            (b4, 5, 1)
        }
        21 => {
            centre(m);
            let a = unq(m, e, 0x17e);
            let n = m.r32(s + 0x14bc);
            e.call(m, WSTR_FORMAT, &[b2, a, n]);
            if m.r32(s + 0x14c0) == 1 {
                let c = unq(m, e, 0x183);
                let d = unq(m, e, 0x184);
                e.call(m, WSTR_FORMAT_ASCII, &[b4, THREE_FORMAT, b2, c, d]);
            } else {
                let c = unq(m, e, 0x182);
                let k = m.r32(s + 0x14c0);
                e.call(m, WSTR_FORMAT, &[buf, c, k]);
                let d = unq(m, e, 0x184);
                e.call(m, WSTR_FORMAT_ASCII, &[b4, THREE_FORMAT, b2, buf, d]);
            }
            (b4, 5, 1)
        }
        22 => {
            if m.r32(s + 0x3cb4) == 0 && m.r32(s + 0x3cb8) == 0 {
                card(m, e, s, 0xb0, &[]);
            }
            return;
        }
        _ => return,
    };
    e.call(m, DRAW_BOX, &[s, text, kind, wait]);
}

/// 0x1858c0, a static initialiser: three colours at 0x3cdd48 (1, 0.9645, 0), 0x3cdd58 (1, 0.4746, 0) and 0x3cdd68
/// (1, 0.0758, 0.002), and a record at 0x3cdd78 (-1, 1, 1, 0, 0, 1, 1)
pub fn static_init(m: &mut Mem) {
    colours_init(m, 0x3cdd48);
}

/// The static initialisers' work (0x1858c0 at 0x3cdd48, 0x186a00 at 0x3cdd98): three colours from `base` 0x10
/// apart and a record after them, in the game's order of stores
pub fn colours_init(m: &mut Mem, base: u32) {
    let words: [(u32, u32); 17] = [
        (0x28, 0x3b03_126f),
        (0, ONE),
        (8, 0),
        (0x48, ONE),
        (0x20, ONE),
        (0x24, 0x3d9b_3d08),
        (0x10, ONE),
        (0x14, 0x3ef2_fec5),
        (0x18, 0),
        (4, 0x3f76_e979),
        (0x30, u32::MAX),
        (0x34, ONE),
        (0x38, ONE),
        (0x3c, 0),
        (0x40, 0),
        (0x44, ONE),
        (0x48, ONE),
    ];
    for (a, v) in words {
        m.w32(base + a, v);
    }
}

/// E1f3's calls out: the text box's rectangle (0x196070), text (0x196088), line height (0x1961e8), layout
/// (0x1964a0), clear (0x195ff0) and draw (0x1957d8); the font's text width (0x19dc78) and print (0x19d0c0); a sprite
/// drawn at a place (0x14eed0)
pub const TB_RECT: u32 = 0x196070;
pub const TB_TEXT: u32 = 0x196088;
pub const TB_LINE: u32 = 0x1961e8;
pub const TB_LAYOUT: u32 = 0x1964a0;
pub const TB_CLEAR: u32 = 0x195ff0;
pub const TB_DRAW: u32 = 0x1957d8;
pub const TEXT_WIDTH: u32 = 0x19dc78;
pub const FONT_PRINT: u32 = 0x19d0c0;
pub const SPRITE_AT: u32 = 0x14eed0;

/// The text box (+0x48) placed at (112, `top`), 416 wide and four of the font's lines (+0x14, chopped) high
fn box_rect(m: &mut Mem, e: &mut dyn FeEnv, s: u32, top: u32) {
    let t = s + 0x48;
    let h = int_float(m.r32(s + 0x14)) * F(4.0);
    e.call(m, TB_RECT, &[t, 0x42e0_0000, top, 0x43d0_0000, h.0.to_bits()]);
}

/// The text box's line height: its +0x14 and 4
fn box_line(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    let l = F(f32::from_bits(m.r32(t + 0x14))) + F(4.0);
    e.call(m, TB_LINE, &[t, l.0.to_bits()]);
}

/// `MemCardMsg_DrawBox` 0x183538(s, text, kind, wait; `buf` 0xc bytes): the text box at (112, 96) with the text,
/// centred and laid out, texture mode 3, the font white, drawn; then the buttons (`MemCardMsg_Draw`) when `kind` has
/// any
pub fn draw_box(m: &mut Mem, e: &mut dyn FeEnv, s: u32, text: u32, kind: u32, wait: u32, buf: u32) {
    let t = s + 0x48;
    box_rect(m, e, s, 0x42c0_0000);
    e.call(m, TB_TEXT, &[t, text]);
    box_line(m, e, t);
    m.w32(t + 0x24, 0);
    m.w32(t + 0x20, 1);
    e.call(m, TB_LAYOUT, &[t]);
    rend(m, e, 0x1e0, &[3]);
    for k in [2u32, 0, 1] {
        m.w32(buf + 4 * k, ONE);
    }
    e.call(m, gm::FONT_COLOUR, &[gm::FONT_STATE, buf]);
    e.call(m, TB_DRAW, &[t, u32::MAX, 0]);
    if kind != 0 {
        draw_buttons(m, e, s, kind, wait, buf);
    }
}

/// `Overlay_DrawDontRemoveCard` 0x183660(s): the text box at (112, 230.4) cleared, string 0x15e in it, centred,
/// texture mode 3, its colour (+0x3c) the blink (+0x3ca8) then three ones, drawn
pub fn draw_dont_remove_card(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let t = s + 0x48;
    box_rect(m, e, s, 0x4366_6666);
    e.call(m, TB_CLEAR, &[t]);
    let x = loc(m, e, 0x15e);
    e.call(m, TB_TEXT, &[t, x]);
    box_line(m, e, t);
    m.w32(t + 0x20, 1);
    m.w32(t + 0x24, 0);
    rend(m, e, 0x1e0, &[3]);
    let blink = m.r32(s + 0x3ca8);
    for (k, v) in [blink, ONE, ONE, ONE].into_iter().enumerate() {
        m.w32(t + 0x3c + 4 * k as u32, v);
    }
    e.call(m, TB_DRAW, &[t, u32::MAX, 0]);
}

/// The width of string `id` in the font (+8) at scale 1, chopped to a float
fn width_of(m: &mut Mem, e: &mut dyn FeEnv, s: u32, id: u32) -> F {
    let x = loc(m, e, id);
    int_float(e.call(m, TEXT_WIDTH, &[s + 8, x, 0, ONE]))
}

/// `MemCardMsg_DrawOptions` 0x183788(s, groups, flags) -> the width of the row of buttons: the strings' widths (in
/// three groups: 0x155 or 0xc9a; 0x1ba; 0x154, 0xc9b or 0x162; with two, the second chosen from flags 4, 0x10 and
/// 8; with one, the first of flags 2, 1, 8, 0x10 and 4, else 0x162), 35 a button and 25 between
pub fn draw_options(m: &mut Mem, e: &mut dyn FeEnv, s: u32, n: u32, flags: u32) -> F {
    let first = if flags & 1 != 0 { 0x155 } else { 0xc9a };
    let mut w = F(0.0);
    match n {
        3 => {
            w = width_of(m, e, s, first);
            w = w + width_of(m, e, s, 0x1ba);
            let id = if flags & 0x10 != 0 {
                0x154
            } else if flags & 8 != 0 {
                0xc9b
            } else {
                0x162
            };
            w = w + width_of(m, e, s, id);
        }
        2 => {
            w = width_of(m, e, s, first);
            let id = if flags & 4 != 0 {
                0x1ba
            } else if flags & 0x10 != 0 {
                0x154
            } else if flags & 8 != 0 {
                0xc9b
            } else {
                0x162
            };
            w = w + width_of(m, e, s, id);
        }
        1 => {
            let id = if flags & 2 != 0 {
                Some(0xc9a)
            } else if flags & 1 != 0 {
                Some(0x155)
            } else if flags & 8 != 0 {
                Some(0xc9b)
            } else if flags & 0x10 != 0 {
                Some(0x154)
            } else if flags & 4 != 0 {
                Some(0x1ba)
            } else {
                None
            };
            w = match id {
                Some(id) => width_of(m, e, s, id),
                None => width_of(m, e, s, 0x162) + w,
            };
        }
        _ => {}
    }
    let buttons = int_float(n) * F(35.0);
    let gaps = int_float(n.wrapping_sub(1)) * F(25.0);
    w + buttons + gaps
}

/// The button's sprite (0x51 for 1 and 2, 0x50 for 4, 8 and 0x10, 0x4f for 0x20, 0x52 for 0x40); any other
/// leaves the caller's a3
fn button_sprite(button: u32, a3: u32) -> u32 {
    match button {
        1 | 2 => 0x51,
        4 | 8 | 0x10 => 0x50,
        0x20 => 0x4f,
        0x40 => 0x52,
        _ => a3,
    }
}

/// `MemCardMsg_DrawButtonPrompt` 0x183ae8(s, button, text, `a3`; x, y; `buf` 0xc bytes) -> the prompt's width:
/// the button's sprite at (x, y) sized 1, the font white, the text at (x + 35, y), its width + 60. `a3` is the
/// sprite for a button the game has none for (whatever the caller left there)
pub fn button_prompt(m: &mut Mem, e: &mut dyn FeEnv, s: u32, button: u32, text: u32, a3: u32, x: F, y: F, buf: u32) -> F {
    let sprite = button_sprite(button, a3);
    let ss = sprites(m);
    let sp = e.call(m, SPRITE_SET_RECT, &[ss, sprite]);
    e.call(m, SPRITE_AT, &[sp, 0, x.0.to_bits(), y.0.to_bits(), ONE, ONE]);
    for k in 0..3u32 {
        m.w32(buf + 4 * k, ONE);
    }
    e.call(m, gm::FONT_COLOUR, &[gm::FONT_STATE, buf]);
    let tx = x + F(35.0);
    e.call(m, FONT_PRINT, &[s + 8, text, tx.0.to_bits(), y.0.to_bits(), ONE]);
    let w = e.call(m, TEXT_WIDTH, &[s + 8, text, 0, ONE]);
    int_float(w) + F(60.0)
}

/// `MemCardMsg_Draw` 0x183c20(s, flags, wait; `buf` 0xc bytes): the buttons the flags ask for in up to three groups
/// (1 / 2 the first, 4 / 0x20 the second, 8 / 0x10 / 0x40 the third). With `wait` they go in a row centred on 320
/// (each one's x moved on by the widths so far, added again each time, as the game does), else down a column at
/// x 160 (30 apart), from y 340. With one group, its buttons all at (x, 340); flag 1 draws its button and then goes
/// on to the others
pub fn draw_buttons(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flags: u32, wait: u32, buf: u32) {
    let mut n = (flags & 3 != 0) as u32;
    if flags & 0x58 != 0 {
        n += 1;
    }
    if flags & 0x24 != 0 {
        n += 1;
    }
    let mut y = F(340.0);
    let mut sum = F(0.0);
    let mut x = if wait != 0 { F(320.0) - draw_options(m, e, s, n, flags) * F(0.5) } else { F(160.0) };
    let prompt = |m: &mut Mem, e: &mut dyn FeEnv, button: u32, id: u32, x: F, y: F| {
        let text = loc(m, e, id);
        button_prompt(m, e, s, button, text, 0, x, y, buf)
    };
    let mut step = |w: F, x: &mut F, y: &mut F| {
        sum = sum + w;
        if wait != 0 {
            *x = *x + sum;
        } else {
            *y = *y + F(30.0);
        }
    };
    let last = |flags: u32| -> (u32, u32) {
        if flags & 0x10 != 0 {
            (0x10, 0x154)
        } else if flags & 8 != 0 {
            (8, 0xc9b)
        } else {
            (0x40, 0x162)
        }
    };
    match n {
        3 => {
            let w = if flags & 1 != 0 { prompt(m, e, 1, 0x155, x, y) } else { prompt(m, e, 2, 0xc9a, x, y) };
            step(w, &mut x, &mut y);
            let b = if flags & 4 != 0 { 4 } else { 0x20 };
            let w = prompt(m, e, b, 0x1ba, x, y);
            step(w, &mut x, &mut y);
            let (b, id) = last(flags);
            prompt(m, e, b, id, x, y);
        }
        2 => {
            let id = if flags & 1 != 0 { 0x155 } else { 0xc9a };
            let w = prompt(m, e, 1, id, x, y);
            step(w, &mut x, &mut y);
            let (b, id) = if flags & 4 != 0 {
                (4, 0x1ba)
            } else if flags & 0x20 != 0 {
                (0x20, 0x1ba)
            } else {
                last(flags)
            };
            prompt(m, e, b, id, x, y);
        }
        1 => {
            if flags & 1 != 0 {
                prompt(m, e, 1, 0x155, x, y);
            }
            let (b, id) = if flags & 2 != 0 {
                (2, 0xc9a)
            } else if flags & 8 != 0 {
                (8, 0xc9b)
            } else if flags & 0x10 != 0 {
                (0x10, 0x154)
            } else if flags & 4 != 0 {
                (4, 0x1ba)
            } else {
                (0x40, 0x162)
            };
            prompt(m, e, b, id, x, y);
        }
        _ => {}
    }
}

//! The loading screens' base `cFlakeSpinnerLoad` and the first load `cFirstLoad` (board row G9w1): what shows while
//! a race or the front end loads (gApp+0x734). On the bytes at their addresses ([`Mem`]); every callee (the files,
//! the fonts, the archive, the renderer's virtuals through its table, the draws) is a [`TextBoxEnv`] call by
//! address. Checked in the function runner (`tests/loadscreen.rs`).
//!
//! The screen (`this`): +0 its table, +4 the app's +0x728, +8 the file being read, +0xc and +0x48 the menu and title
//! fonts, +0x88 the time (in 60ths), +0x8c the fade and +0x90 whether it is fading in, +0x94 / +0xa4 the fade's two
//! colours, +0xb4 the load's step, +0xb8 the HUD's shape, +0xbc.. a sprite's colour and size, +0xd4 the pictures'
//! archive, +0x160 whether the pictures are all in, +0x164 / +0x168 the picture being read and its entry, +0x16c the
//! buffer for packed ones, +0x170 / +0x174 the pictures' list and count (0x34 bytes each: +0 loaded, +4 the name,
//! +0x30 the shape), +0x178 the bars' animation.
//!
//! Where the game builds on its stack (colours, a file's out word), the port uses `scratch` (STANDIN: the place, not
//! the bytes).

use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
/// The text drawer's colour
pub const DRAW_COLOUR: u32 = 0x337fe8;
pub const VTABLE: u32 = 0x3a5338;
pub const FIRST_VTABLE: u32 = 0x3a52d8;
/// "data/textures/hud.ssh", "map4", "data/fonts/title.sfn", "data/fonts/menu.sfn", "LoadPicDataBuffer"
pub const HUD_SSH: u32 = 0x3a4bb0;
pub const MAP4: u32 = 0x3a4bc8;
pub const TITLE_SFN: u32 = 0x3a4bd0;
pub const MENU_SFN: u32 = 0x3a4be8;
pub const PIC_BUFFER_NAME: u32 = 0x3a4c18;
/// The pictures' archive's name
pub const PIC_ARCHIVE: u32 = 0x3463d8;
/// The progress flakes' x steps, y and scales (10 floats each)
pub const FLAKE_X: u32 = 0x3a4c30;
pub const FLAKE_Y: u32 = 0x3a4c58;
pub const FLAKE_SCALE: u32 = 0x3a4c80;
/// A mode word: 3 leaves the first flake out
pub const FLAKE_MODE: u32 = 0x32ca50;
pub const CAREER: u32 = 0x32b390;
/// The first load's picture and its shape's name
pub const FIRST_PIC: u32 = 0x3a4ca8;
pub const FIRST_SHAPE: u32 = 0x3a4cc8;

pub const FONT_INIT: u32 = 0x19c788;
pub const FONT_FREE: u32 = 0x19c8d0;
pub const FONT_DELETE: u32 = 0x19c808;
pub const FONT_PARSE: u32 = 0x19c910;
pub const ARCHIVE_INIT: u32 = 0x23bb70;
pub const ARCHIVE_DELETE: u32 = 0x23bb98;
pub const ARCHIVE_OPEN: u32 = 0x23bc28;
pub const ARCHIVE_CLOSE: u32 = 0x23bcc0;
pub const ANIM_INIT: u32 = 0x1f10c0;
pub const ANIM_DELETE: u32 = 0x1f1128;
pub const ANIM_ADD_BAR: u32 = 0x1f11c0;
pub const ANIM_UPDATE: u32 = 0x1f12d0;
pub const ANIM_EVAL: u32 = 0x1f1488;
pub const LOAD_MODULE_DELETE: u32 = 0x181e48;
pub const FILE_LOAD: u32 = 0x2c5e20;
pub const FILE_IS_DONE: u32 = 0x2c6108;
pub const FILE_FINISH: u32 = 0x2c6160;
pub const FILE_CANCEL: u32 = 0x2c6278;
pub const SHAPE_FIND: u32 = 0x2c25f0;
pub const SHAPE_ENTRY: u32 = 0x2c266c;
pub const UNPACKED_SIZE: u32 = 0x2c2fb0;
pub const DECODE: u32 = 0x2c2f90;
pub const EA_FREE: u32 = 0x2ccfc0;
pub const EA_ALLOC: u32 = 0x2ccf70;
pub const MEMSET: u32 = 0x2cc808;
pub const MEM_FREE: u32 = 0x14a040;
pub const SET_COLOUR: u32 = 0x14eee8;
pub const SPRITE_RECT: u32 = 0x14eed0;
pub const FILLED_RECT: u32 = 0x19b108;
pub const DRAW_RIGHT: u32 = 0x19d370;
pub const PRINTF_SHADOWED_A: u32 = 0x19d568;
pub const SPRITE_TILED: u32 = 0x1fc728;
pub const SPRITE_ANCHORED: u32 = 0x1fc178;
pub const SET_FADE: u32 = 0x1e0fb8;
pub const LOC: u32 = 0x2c5160;
pub const COS: u32 = 0x250e98;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn fcall(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32], f: &[F]) -> u32 {
    env.call(m, func, args, f)
}
fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn put4(m: &mut Mem, at: u32, v: [F; 4]) {
    for (i, x) in v.into_iter().enumerate() {
        wf(m, at + 4 * i as u32, x);
    }
}
/// A virtual of the renderer (the app's +0x724): its table +0x10, `this` moved by the half word at `off`, the
/// function at `off + 4`
fn rcall(m: &mut Mem, env: &mut dyn TextBoxEnv, off: u32, args: &[u32], f: &[F]) -> u32 {
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, m.r32(r + 0x10), off, args, f)
}
/// A virtual of `this` through its own table (+0)
fn scall(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, off: u32) -> u32 {
    vcall(m, env, this, m.r32(this), off, &[], &[])
}
fn vcall(m: &mut Mem, env: &mut dyn TextBoxEnv, o: u32, vt: u32, off: u32, args: &[u32], f: &[F]) -> u32 {
    let me = o.wrapping_add(m.r16(vt + off) as i16 as u32);
    let func = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    env.call(m, func, &a, f)
}

const ONE: F = F(1.0);
const ZERO: F = F(0.0);

/// `FlakeSpinnerLoad_Init` 0x243c78(this): the table, the two fonts (16 glyphs' room each), the HUD's shape −1 and
/// its sprite white and full size, the archive, no pictures, the bars' animation, the app's +0x728, the fade 1 (not
/// fading in) and its colours red, the time 0; `this`
pub fn init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    m.w32(this, VTABLE);
    call(m, env, FONT_INIT, &[this + 0xc, 0x10]);
    call(m, env, FONT_INIT, &[this + 0x48, 0x10]);
    let s = this + 0xb8;
    m.w32(s, 0xffff_ffff);
    wf(m, s + 0x18, ONE);
    wf(m, s + 4, ONE);
    wf(m, s + 8, ONE);
    wf(m, s + 0x14, ONE);
    m.w32(s + 0xc, 0);
    m.w32(s + 0x10, 0);
    call(m, env, ARCHIVE_INIT, &[this + 0xd4]);
    for o in [0x164, 0x168, 0x16c, 0x170] {
        m.w32(this + o, 0);
    }
    m.w8(this + 0x174, 0);
    m.w32(this + 0x160, 1);
    call(m, env, ANIM_INIT, &[this + 0x178]);
    let app = m.r32(APP);
    m.w32(this + 0x90, 0);
    let v = m.r32(app + 0x728);
    m.w32(this + 0xb8, 0xffff_ffff);
    wf(m, this + 0x8c, ONE);
    m.w32(this + 4, v);
    wf(m, this + 0xc4, k(0x3f36_8000));
    wf(m, this + 0xd0, k(0x3f51_8000));
    wf(m, this + 0xc8, k(0x3f33_8000));
    wf(m, this + 0xcc, k(0x3f4b_8000));
    m.w32(this + 0x88, 0);
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    this
}

/// `cFlakeSpinnerLoad::~cFlakeSpinnerLoad` 0x243de8(this, flags): the fonts freed, the renderer's 0x148, the
/// animation, the archive and the fonts deleted, then the load module's destructor
pub fn delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this, VTABLE);
    call(m, env, FONT_FREE, &[this + 0xc]);
    call(m, env, FONT_FREE, &[this + 0x48]);
    rcall(m, env, 0x148, &[], &[]);
    call(m, env, ANIM_DELETE, &[this + 0x178, 2]);
    call(m, env, ARCHIVE_DELETE, &[this + 0xd4, 2]);
    call(m, env, FONT_DELETE, &[this + 0x48, 2]);
    call(m, env, FONT_DELETE, &[this + 0xc, 2]);
    call(m, env, LOAD_MODULE_DELETE, &[this, flags]);
}

/// vf1 0x243ea0(this): the HUD's textures asked for, the step 0, the renderer's 0x138 with 16
pub fn start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let f = call(m, env, FILE_LOAD, &[HUD_SSH, 0]);
    m.w32(this + 8, f);
    m.w32(this + 0xb4, 0);
    rcall(m, env, 0x138, &[0x10], &[]);
}

/// A file read in: its buffer (the out word, in `out`)
fn finish(m: &mut Mem, env: &mut dyn TextBoxEnv, file: u32, out: u32, out2: u32) -> u32 {
    call(m, env, FILE_FINISH, &[file, out, out2]);
    m.r32(out)
}

/// vf2 0x243f00(this): the load's steps, one a call while each file is not yet in (0 until done, then 1):
/// 0 the HUD's textures (their "map4" shape added to the renderer), then the title font; 1 the title font (scale
/// 1.25 by 1), then the menu font; 2 the menu font, then the screen's own +0x50; 3 the pictures one by one (a packed
/// one unpacked into the buffer first; each added to the renderer as its name + 0x29) until none is left, the buffer
/// freed; 4 done when the screen's own +0x58 says so
pub fn poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) -> u32 {
    let sp = scratch;
    match m.r32(this + 0xb4) {
        0 => {
            let done = call(m, env, FILE_IS_DONE, &[m.r32(this + 8)]);
            if done != 1 {
                return 0;
            }
            let buf = finish(m, env, m.r32(this + 8), sp, 0);
            let shape = call(m, env, SHAPE_FIND, &[buf, MAP4]);
            let id = rcall(m, env, 0x150, &[shape, MAP4, 0], &[]);
            m.w32(this + 0xb8, id);
            call(m, env, EA_FREE, &[m.r32(sp)]);
            let f = call(m, env, FILE_LOAD, &[TITLE_SFN, 0]);
            m.w32(this + 0xb4, done);
            m.w32(this + 8, f);
        }
        1 => {
            if call(m, env, FILE_IS_DONE, &[m.r32(this + 8)]) != 1 {
                return 0;
            }
            let buf = finish(m, env, m.r32(this + 8), sp + 4, 0);
            call(m, env, FONT_PARSE, &[this + 0x48, buf, 0, 0]);
            wf(m, this + 0x48 + 0x18, k(0x3fa0_0000));
            wf(m, this + 0x48 + 0x1c, ONE);
            call(m, env, EA_FREE, &[m.r32(sp + 4)]);
            let f = call(m, env, FILE_LOAD, &[MENU_SFN, 0]);
            m.w32(this + 8, f);
            m.w32(this + 0xb4, 2);
        }
        2 => {
            if call(m, env, FILE_IS_DONE, &[m.r32(this + 8)]) != 1 {
                return 0;
            }
            let buf = finish(m, env, m.r32(this + 8), sp + 8, 0);
            call(m, env, FONT_PARSE, &[this + 0xc, buf, 0, 0]);
            wf(m, this + 0xc + 0x18, k(0x3fa0_0000));
            wf(m, this + 0xc + 0x1c, ONE);
            call(m, env, EA_FREE, &[m.r32(sp + 8)]);
            scall(m, env, this, 0x50);
            m.w32(this + 0xb4, 3);
        }
        3 => {
            if m.r32(this + 0x160) != 0 {
                m.w32(this + 0xb4, 4);
                return 0;
            }
            let file = m.r32(this + 0x164);
            if file != 0 {
                let done = call(m, env, FILE_IS_DONE, &[file]);
                if done != 1 {
                    return 0;
                }
                m.w32(sp + 0xc, 0);
                m.w32(sp + 0x10, 0);
                call(m, env, FILE_FINISH, &[m.r32(this + 0x164), sp + 0xc, sp + 0x10]);
                let mut owned = true;
                if call(m, env, UNPACKED_SIZE, &[m.r32(sp + 0xc)]) != 0 {
                    owned = false;
                    call(m, env, DECODE, &[m.r32(sp + 0xc), m.r32(this + 0x16c)]);
                    call(m, env, EA_FREE, &[m.r32(sp + 0xc)]);
                    m.w32(sp + 0xc, m.r32(this + 0x16c));
                }
                let shape = call(m, env, SHAPE_ENTRY, &[m.r32(sp + 0xc), 0]);
                let e = m.r32(this + 0x168);
                let id = rcall(m, env, 0x150, &[shape, e.wrapping_add(0x29), 0], &[]);
                let e = m.r32(this + 0x168);
                m.w32(e, done);
                m.w32(e + 0x30, id);
                if owned {
                    call(m, env, EA_FREE, &[m.r32(sp + 0xc)]);
                }
                m.w32(this + 0x164, 0);
                m.w32(this + 0x168, 0);
                return 0;
            }
            let mut n = m.r8(this + 0x174) as i8 as i32;
            let mut i = 0i32;
            while i < n {
                let e = m.r32(this + 0x170).wrapping_add((i as u32).wrapping_mul(0x34));
                if m.r32(e) == 1 {
                    i += 1;
                    continue;
                }
                let h = call(m, env, FILE_LOAD, &[e + 4, 0]);
                m.w32(this + 0x164, h);
                if h != 0 {
                    m.w32(this + 0x168, e);
                    break;
                }
                n = m.r8(this + 0x174) as i8 as i32;
                i += 1;
            }
            if m.r32(this + 0x164) != 0 {
                return 0;
            }
            let buf = m.r32(this + 0x16c);
            if buf != 0 {
                call(m, env, MEM_FREE, &[buf]);
            }
            m.w32(this + 0xb4, 4);
            m.w32(this + 0x160, 1);
            m.w32(this + 0x16c, 0);
        }
        4 => {
            if scall(m, env, this, 0x58) != 0 {
                return 1;
            }
        }
        _ => {}
    }
    0
}

/// `LoadScreen_LoadPics` 0x244290(this, list, n): the pictures' archive opened, the list kept; with pictures a
/// buffer of 0x11800 bytes (for the packed ones) taken and filled, and not all in yet
pub fn load_pics(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, list: u32, n: u32) {
    call(m, env, ARCHIVE_OPEN, &[this + 0xd4, PIC_ARCHIVE, 0x10]);
    m.w32(this + 0x170, list);
    m.w8(this + 0x174, n as u8);
    if n != 0 {
        let p = call(m, env, EA_ALLOC, &[PIC_BUFFER_NAME, 0x11800, 0]);
        if p != 0 {
            call(m, env, MEMSET, &[p, 0xdead_c0ed, 0x11800]);
        }
        m.w32(this + 0x16c, p);
        m.w32(this + 0x160, 0);
    }
    m.w32(this + 0x168, 0);
    m.w32(this + 0x164, 0);
}

/// `AsyncLoad_CloseArchive` 0x244330(this): a picture being read cancelled (and its buffer freed: all in), the
/// packed buffer freed, the list dropped, the archive closed
pub fn close_archive(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let f = m.r32(this + 0x164);
    if f != 0 {
        m.w32(scratch, 0);
        call(m, env, FILE_CANCEL, &[f]);
        let b = finish(m, env, m.r32(this + 0x164), scratch, 0);
        call(m, env, EA_FREE, &[b]);
        m.w32(this + 0x160, 1);
    }
    let buf = m.r32(this + 0x16c);
    if buf != 0 {
        call(m, env, MEM_FREE, &[buf]);
        m.w32(this + 0x16c, 0);
    }
    m.w32(this + 0x170, 0);
    m.w8(this + 0x174, 0);
    call(m, env, ARCHIVE_CLOSE, &[this + 0xd4]);
}

/// vf4 0x2443b0(this): the renderer's 0x140, its 0x68 with (0, 0, 0), and the bars' animation's three bars (1, 2, 3
/// at 100, 250, 400; 2, 5000, 0x11170)
pub fn start_bars(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    rcall(m, env, 0x140, &[], &[]);
    for o in [0, 4, 8] {
        m.w32(scratch + o, 0);
    }
    rcall(m, env, 0x68, &[scratch], &[]);
    for (bar, at) in [(1u32, 100u32), (2, 250), (3, 400)] {
        call(m, env, ANIM_ADD_BAR, &[this + 0x178, bar, 1, at, 2, 5000, 0x11170]);
    }
}

/// vf3 0x244490(this): fading in from now; whether the fade is at 1
pub fn fade_in(m: &mut Mem, this: u32) -> u32 {
    m.w32(this + 0x90, 1);
    (rf(m, this + 0x8c) == ONE) as u32
}

/// vf5 0x2444c0(this): the time a 60th on; the fade a 30th up (to 1) while fading in, else a 60th down (to 0); the
/// bars' animation stepped
pub fn tick(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let sixtieth = k(0x3c88_8889);
    let t = rf(m, this + 0x88) + sixtieth;
    wf(m, this + 0x88, t);
    if m.r32(this + 0x90) != 0 {
        let v = rf(m, this + 0x8c) + k(0x3d08_8889);
        wf(m, this + 0x8c, v);
        if ONE < rf(m, this + 0x8c) {
            wf(m, this + 0x8c, ONE);
        }
    } else {
        let v = rf(m, this + 0x8c) - sixtieth;
        wf(m, this + 0x8c, v);
        if rf(m, this + 0x8c) < ZERO {
            wf(m, this + 0x8c, ZERO);
        }
    }
    call(m, env, ANIM_UPDATE, &[this + 0x178]);
}

/// `LoadScreen_DrawProgressFlakes` 0x244540(_, sprites, done): ten flakes from x 120 on (the steps from the table;
/// the first left out when the mode word is 3), the first `done` (at most 10) gold and the rest faded white, each
/// the sprite +0xa8 + 0x1c·i at its y and scale
pub fn draw_flakes(m: &mut Mem, env: &mut dyn TextBoxEnv, sprites: u32, done: i32) {
    let first = (m.r32(FLAKE_MODE) == 3) as i32;
    let done = if done < 11 { done } else { 10 };
    let mut x = F(120.0);
    let mut i = first;
    while i < 10 {
        let j = i as u32;
        x = x + rf(m, FLAKE_X + 4 * j);
        if !(i < done) {
            wf(m, DRAW_COLOUR + 0xc, ONE);
            wf(m, DRAW_COLOUR, k(0x3ec2_8f5c));
            wf(m, DRAW_COLOUR + 4, ONE);
            wf(m, DRAW_COLOUR + 8, ONE);
        } else {
            wf(m, DRAW_COLOUR, ONE);
            wf(m, DRAW_COLOUR + 0xc, k(0x3f0e_8e8f));
            wf(m, DRAW_COLOUR + 4, k(0x3f73_f3f4));
            wf(m, DRAW_COLOUR + 8, k(0x3f58_d8d9));
        }
        let y = rf(m, FLAKE_Y + 4 * j);
        let s = rf(m, FLAKE_SCALE + 4 * j);
        fcall(m, env, SPRITE_RECT, &[sprites.wrapping_add(j * 0x1c + 0xa8), 0], &[x, y, ONE, s]);
        i += 1;
    }
}

/// `LoadScreen_DrawTipBox` 0x2447a8(this, _, sheet, text, tex; x, y, alpha): a black shadow 3 to the right and
/// below, a white box (254 by 79) on texture `tex` and `tex + 1`, then texture `tex + 3` and the text right-aligned in
/// black in the title font (at 1.27 by 1.34, 0.6; then back), the colours by `alpha`
pub fn draw_tip_box(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, sheet: u32, text: u32, tex: u32, x: F, y: F, alpha: F, scratch: u32) {
    let font = this + 0x48;
    put4(m, scratch, [ONE, ZERO, ZERO, ZERO]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let (w, h) = (F(254.0), F(79.0));
    let v = rf(m, DRAW_COLOUR) * alpha;
    wf(m, DRAW_COLOUR, v);
    rcall(m, env, 0x1d0, &[tex], &[]);
    rcall(m, env, 0x1f0, &[sheet, 0], &[]);
    let three = F(3.0);
    fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x + three, y + three, w, h]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let v = rf(m, DRAW_COLOUR) * alpha;
    wf(m, DRAW_COLOUR, v);
    rcall(m, env, 0x1d0, &[tex.wrapping_add(1)], &[]);
    fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x, y, w, h]);
    rcall(m, env, 0x1d0, &[tex.wrapping_add(3)], &[]);
    put4(m, scratch, [ONE, ZERO, ZERO, ZERO]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let v = rf(m, DRAW_COLOUR) * alpha;
    wf(m, DRAW_COLOUR, v);
    let (sx, sy) = (rf(m, font + 0x18), rf(m, font + 0x1c));
    wf(m, font + 0x18, k(0x3fa2_8f5c));
    wf(m, font + 0x1c, k(0x3fab_851f));
    if text != 0 {
        fcall(m, env, DRAW_RIGHT, &[font, text], &[x + F(240.0), (y + F(52.0)) + F(5.0), k(0x3f19_999a)]);
    }
    wf(m, font + 0x1c, sy);
    wf(m, font + 0x18, sx);
}

/// `LoadScreen_DrawTicker` 0x2449f8(_, ticker): in white on texture 5, the ticker's sheet (+0x54) tiled across
/// (700 wide, twice its +0x5c high, 6 per tile) at the screen's bottom, then its second strip at (−57, −76)
pub fn draw_ticker(m: &mut Mem, env: &mut dyn TextBoxEnv, ticker: u32, scratch: u32) {
    let sheet = ticker + 0x54;
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    rcall(m, env, 0x1d0, &[5], &[]);
    let hh = rf(m, ticker + 0x5c);
    let h = hh + hh;
    let y = ((F(480.0) - hh) - F(80.0)) + F(18.0);
    let w = F(700.0);
    fcall(m, env, SPRITE_TILED, &[sheet, 0], &[ZERO, y, w, h, F(6.0), ONE]);
    fcall(m, env, SPRITE_TILED, &[sheet, 2], &[k(0xc264_0000), k(0xc298_0000), w, h, F(6.0), ONE]);
}

/// An unsigned word made a float as the compiler does it: halved (keeping the low bit) and doubled when it is past
/// 2^31
fn u32_to_f(v: u32) -> F {
    if (v as i32) < 0 {
        let f = crate::animseq::cvt_s_w(((v >> 1) | (v & 1)) as i32);
        f + f
    } else {
        crate::animseq::cvt_s_w(v as i32)
    }
}

/// vf6 0x244b20(this): the frame, when the renderer's 0x70 lets it: its 0x1a0, 0x88, 0xd8, the viewport 0x98 the
/// screen's size (0x58, 0x60), 0x1e0 (3), 0x1c0 (1), the projection 0xb0 (640 by 480, 0 to 1000), 0x108, texture 4,
/// no fade; the screen's own background (+0x40) and overlay (+0x48); texture 9 in white: "loading" (0x175) at
/// step 4; while the fade is up, a full screen in the fade's colour (texture 0x13); then 0xe0, 0x90, 0x1a8, 0x80
pub fn draw(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if rcall(m, env, 0x70, &[], &[]) == 0 {
        return;
    }
    rcall(m, env, 0x1a0, &[], &[]);
    rcall(m, env, 0x88, &[], &[]);
    rcall(m, env, 0xd8, &[], &[]);
    let w = u32_to_f(rcall(m, env, 0x58, &[], &[]));
    let h = u32_to_f(rcall(m, env, 0x60, &[], &[]));
    rcall(m, env, 0x98, &[], &[ZERO, ZERO, w, h]);
    let (sw, sh) = (F(640.0), F(480.0));
    rcall(m, env, 0x1e0, &[3], &[]);
    rcall(m, env, 0x1c0, &[1], &[]);
    rcall(m, env, 0xb0, &[], &[ZERO, ZERO, sw, sh, ZERO, F(1000.0)]);
    rcall(m, env, 0x108, &[], &[]);
    rcall(m, env, 0x1d0, &[4], &[]);
    let r = m.r32(m.r32(APP) + 0x724);
    fcall(m, env, SET_FADE, &[r], &[ZERO]);
    scall(m, env, this, 0x40);
    scall(m, env, this, 0x48);
    rcall(m, env, 0x1d0, &[9], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    if m.r32(this + 0xb4) == 4 {
        let t = m.r32(m.r32(APP) + 0x50);
        let s = call(m, env, LOC, &[t, 0x175]);
        fcall(m, env, PRINTF_SHADOWED_A, &[this + 0xc, s], &[ONE, F(28.0), F(430.0)]);
    }
    if ZERO < rf(m, this + 0x8c) {
        rcall(m, env, 0x1d0, &[0x13], &[]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        let c = if m.r32(this + 0x90) != 0 { this + 0xa4 } else { this + 0x94 };
        let a = rf(m, this + 0x8c);
        let (c3, c1, c2) = (rf(m, c + 0xc), rf(m, c + 4), rf(m, c + 8));
        wf(m, DRAW_COLOUR, a);
        wf(m, DRAW_COLOUR + 4, c1);
        wf(m, DRAW_COLOUR + 8, c2);
        wf(m, DRAW_COLOUR + 0xc, c3);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[ZERO, ZERO, sw, sh]);
    }
    rcall(m, env, 0xe0, &[], &[]);
    rcall(m, env, 0x90, &[], &[]);
    rcall(m, env, 0x1a8, &[], &[]);
    rcall(m, env, 0x80, &[], &[]);
}

/// 0x244ee8(this; x, angle): the spinner: 45 · cos(angle) wide, centred 30 left of x, at y 420.5, in white on
/// texture 10 (the HUD's sprite +0xb8)
pub fn draw_spinner(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, x: F, angle: F) {
    let h = F(45.0);
    let c = F(crate::math::cos(angle.0));
    let _ = COS;
    let w = c * h;
    let left = (x - F(30.0)) - w * F(0.5);
    rcall(m, env, 0x1d0, &[10], &[]);
    wf(m, DRAW_COLOUR + 0xc, ONE);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 4, ONE);
    wf(m, DRAW_COLOUR + 8, ONE);
    fcall(m, env, SPRITE_ANCHORED, &[this + 0xb8, 0], &[left, k(0x43d2_4000), w, h]);
}

/// `LoadScreen_DrawBars` 0x244fc8(this, sheets): a dark gold full screen on texture 4, then on texture 6 two bands
/// at the top and two at the bottom (8 apart, the second faded to 0.4): each a black strip, then the sheets' end
/// pieces (+0x2a0 at 120 and 520, +0x284 at 640 − the cap's width +0x2a4) with black between them, the band's
/// height the sheets' +0x2a8; then the bars' animation's three lines (white at 0.3, texture 7)
pub fn draw_bars(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, sheets: u32, scratch: u32) {
    let anim = this + 0x178;
    let piece = sheets + 0x2a0;
    let cap = sheets + 0x284;
    let cw = rf(m, sheets + 0x2a4);
    let bh = rf(m, sheets + 0x2a8);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    let full = F(640.0);
    let right = full - cw;
    rcall(m, env, 0x1d0, &[4], &[]);
    put4(m, scratch + 4, [k(0x3fc2_6e98), k(0x3f94_bc6a), k(0x3e2d_0e56), ZERO]);
    wf(m, scratch, ONE);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[ZERO, ZERO, full, F(480.0)]);
    rcall(m, env, 0x1d0, &[6], &[]);
    put4(m, DRAW_COLOUR, [ONE, ZERO, ZERO, ZERO]);
    let (step, a, b) = (F(8.0), F(120.0), F(520.0));
    let mid = F(400.0);
    // the top bands
    let (mut x, mut y) = (ZERO, ZERO);
    for _ in 0..2 {
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        let (xa, xb) = (x + a, x + b);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x, y, full, F(80.0) - bh]);
        let top = (y + F(80.0)) - bh;
        y = y + step;
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x, top, a, bh]);
        fcall(m, env, SPRITE_RECT, &[piece, 2], &[xa, top, ONE, ONE]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[xa + cw, top, mid - cw, bh]);
        fcall(m, env, SPRITE_RECT, &[piece, 2], &[xb, top, ONE, ONE]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[xb + cw, top, (right - b) - cw, bh]);
        fcall(m, env, SPRITE_RECT, &[cap, 2], &[x + right, top, ONE, ONE]);
        x = x + step;
        put4(m, DRAW_COLOUR, [k(0x3ecc_cccd), ZERO, ZERO, ZERO]);
    }
    put4(m, DRAW_COLOUR, [ONE, ZERO, ZERO, ZERO]);
    // the bottom bands
    let (mut x, mut y) = (ZERO, ZERO);
    for _ in 0..2 {
        let top = y + mid;
        let (xa, xb) = (x + a, x + b);
        y = y - step;
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x, top, a, bh]);
        fcall(m, env, SPRITE_RECT, &[piece, 0], &[xa, top, ONE, ONE]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[xa + cw, top, mid - cw, bh]);
        fcall(m, env, SPRITE_RECT, &[piece, 0], &[xb, top, ONE, ONE]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[xb + cw, top, (right - b) - cw, bh]);
        fcall(m, env, SPRITE_RECT, &[cap, 0], &[x + right, top, ONE, ONE]);
        rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[x, top + bh, full, F(80.0) - bh]);
        x = x + step;
        put4(m, DRAW_COLOUR, [k(0x3ecc_cccd), ZERO, ZERO, ZERO]);
    }
    put4(m, DRAW_COLOUR, [k(0x3e99_999a), ONE, ONE, ONE]);
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    rcall(m, env, 0x1d0, &[7], &[]);
    for line in 1..4u32 {
        let y = F(f32::from_bits(call(m, env, ANIM_EVAL, &[anim, line])));
        fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[ZERO, y, full, ONE]);
    }
}

/// `FE_GetSelectedCourse` 0x245598: the career's course (+0x2af8), or the World Circuit's (+0x3814) while its +0x3708
/// is 1
pub fn selected_course(m: &Mem) -> u32 {
    if m.r32(CAREER + 0x3708) == 1 { m.r32(CAREER + 0x3814) } else { m.r32(CAREER + 0x2af8) }
}

/// `cFirstLoad::cFirstLoad` 0x2455c0(this): the base's, its table, both fade colours red, no picture (+0x40c −1)
pub fn first_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    init(m, env, this);
    m.w32(this, FIRST_VTABLE);
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    m.w32(this + 0x40c, 0xffff_ffff);
    this
}

/// `cFirstLoad::~cFirstLoad` 0x245668(this, flags)
pub fn first_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this, FIRST_VTABLE);
    delete(m, env, this, flags);
}

/// `cFirstLoad::StartAssetLoad` 0x245680(this): its picture asked for
pub fn first_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let f = call(m, env, FILE_LOAD, &[FIRST_PIC, 0]);
    m.w32(this + 8, f);
}

/// `cFirstLoad::PollAssetLoad` 0x2456b8(this): 0 until the picture is in; then its first shape added to the renderer
/// (+0x40c) and 1
pub fn first_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) -> u32 {
    if call(m, env, FILE_IS_DONE, &[m.r32(this + 8)]) != 1 {
        return 0;
    }
    let buf = finish(m, env, m.r32(this + 8), scratch, 0);
    let shape = call(m, env, SHAPE_ENTRY, &[buf, 0]);
    let id = rcall(m, env, 0x150, &[shape, FIRST_SHAPE, 0], &[]);
    m.w32(this + 0x40c, id);
    call(m, env, EA_FREE, &[m.r32(scratch)]);
    1
}

/// `cFirstLoad::DrawBackground` 0x245760(this): with its picture, the picture full screen in white (the renderer's
/// 0x1f8 blend on, then off)
pub fn first_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let pic = m.r32(this + 0x40c);
    if (pic as i32) < 0 {
        return;
    }
    rcall(m, env, 0x1f0, &[pic, 0], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    rcall(m, env, 0x1f8, &[1, 0], &[]);
    fcall(m, env, FILLED_RECT, &[DRAW_COLOUR], &[ZERO, ZERO, F(640.0), F(480.0)]);
    rcall(m, env, 0x1f8, &[0, 0], &[]);
}

// --- G9w2: the one-player track load, and the sprite records every screen's poll fills ---

pub const TRACK1_VTABLE: u32 = 0x3a5278;
pub const SPEECH: u32 = 0x343050;
pub const RACE: u32 = 0x32db6c;
pub const CHARACTERS: u32 = 0x331ea8;
/// "ldmo", "ldtr", "ldri" (with their 0): the pictures' shape names
pub const SHAPE_MODE: u32 = 0x3a4f10;
pub const SHAPE_TRACK: u32 = 0x3a4f18;
pub const SHAPE_RIDER: u32 = 0x3a4f20;
/// ".ssh", and "%d"
pub const SSH: u32 = 0x3a4ce0;
pub const FMT_D: u32 = 0x3a4b30;
pub const SPRINTF: u32 = 0x2fca28;
pub const BANK_LOAD: u32 = 0x214878;
pub const BANK_POLL: u32 = 0x2148c8;
pub const LOOP_PLAY: u32 = 0x214910;
pub const LOOP_STOP: u32 = 0x214d10;
pub const BANK_UNLOAD: u32 = 0x214d78;
pub const SPRITES_INIT: u32 = 0x1f7f28;
pub const LOAD_ICONS: u32 = 0x1f9a80;
pub const SPRITES_POLL: u32 = 0x1f9d28;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const WSTR_COUNT: u32 = 0x17cc48;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
pub const WSTR_FORMAT: u32 = 0x17ce38;
pub const WSTR_FORMAT_ASCII: u32 = 0x17ce70;
pub const PLAYER_ID: u32 = 0x166580;
pub const PLAYER_PROFILE: u32 = 0x164330;
pub const LEVEL_NAME: u32 = 0x164808;
pub const TRICK_BOOK: u32 = 0x163f18;
pub const COMPLETE_PAGES: u32 = 0x164060;
pub const EVENT_MEDAL: u32 = 0x163e28;
pub const COURSE_NAME: u32 = 0x16b650;
pub const COURSE_DIFFICULTY: u32 = 0x16b330;
pub const COURSE_LOCATION: u32 = 0x16b3f8;
pub const COURSE_DROP: u32 = 0x16b4c0;
pub const COURSE_LENGTH: u32 = 0x16b588;
pub const MAP_X: u32 = 0x16b6d8;
pub const MAP_Y: u32 = 0x16b7b0;
pub const ANIM_VALUE: u32 = 0x1f1448;
pub const TS_INIT: u32 = 0x19dec0;
pub const TS_CONSTRUCT: u32 = 0x19e388;
pub const TS_SET_FONT: u32 = 0x19e380;
pub const TS_DESTRUCT_THUNK: u32 = 0x19e3b0;
pub const TS_DESTRUCT: u32 = 0x19ded0;
pub const DRAW_ALIGNED: u32 = 0x19df90;
pub const DRAW_CURSOR: u32 = 0x19e460;

/// One of the poll's stores: (offset from the records, a float's bits / the sheet's word at / a whole number)
#[derive(Clone, Copy)]
enum Store {
    K(u32, u32),
    Sheet(u32, u32),
    Int(u32, u32),
}
use Store::{Int, K, Sheet};

/// The stores, in the game's order (the records' +0x310 is the loaded flag)
const SPRITE_TABLE: [Store; 197] = [
    Sheet(0x0000, 0x15e8), Sheet(0x001c, 0x15e8), Sheet(0x0038, 0x15e8), Int(0x0310, 1), K(0x0018, 0x3ee30000),
    K(0x0004, 0x43320000), K(0x0008, 0x42e40000), K(0x0028, 0x3f418000), K(0x002c, 0x3e850000),
    K(0x0030, 0x3f608000), K(0x0020, 0x431f0000), K(0x0024, 0x427c0000), K(0x0044, 0x3cb00000),
    K(0x0050, 0x3e0e0000), K(0x0048, 0x3f5c8000), K(0x004c, 0x3f7c8000), Sheet(0x0054, 0x15e8),
    K(0x0060, 0x3f158000), K(0x0068, 0x3e810000), K(0x0058, 0x42820000), K(0x0014, 0x3f318000),
    K(0x003c, 0x42040000), K(0x0040, 0x41f80000), K(0x000c, 0x3b000000), K(0x0010, 0x3b000000),
    K(0x0034, 0x3f7f8000), K(0x006c, 0x3f7f8000), K(0x0064, 0x3b000000), K(0x005c, 0x42d60000),
    Sheet(0x0070, 0x15e8), Sheet(0x008c, 0x15e8), Sheet(0x00a8, 0x15ec), Sheet(0x00c4, 0x15ec),
    K(0x0088, 0x3f2d8000), K(0x0080, 0x3ef50000), K(0x0084, 0x3f0c8000), K(0x0074, 0x41980000),
    K(0x0078, 0x41a00000), K(0x00a4, 0x3f318000), K(0x009c, 0x3e9b0000), K(0x00a0, 0x3ec70000),
    K(0x0090, 0x41b80000), K(0x00c0, 0x3f3b8000), K(0x00b8, 0x3cd00000), K(0x00bc, 0x3e320000),
    K(0x00b0, 0x41d80000), K(0x00d0, 0x3f1e8000), K(0x00d4, 0x3e620000), K(0x00d8, 0x3ead0000),
    K(0x00c8, 0x41f80000), K(0x007c, 0x3f1a8000), K(0x0098, 0x3f198000), K(0x0094, 0x41c80000),
    K(0x00b4, 0x3f218000), K(0x00ac, 0x421c0000), K(0x00cc, 0x41e00000), K(0x00dc, 0x3f398000),
    Sheet(0x00e0, 0x15ec), Sheet(0x00fc, 0x15ec), Sheet(0x0118, 0x15ec), Sheet(0x0134, 0x15ec),
    K(0x00ec, 0x3f218000), K(0x00f8, 0x3f3c8000), K(0x00f0, 0x3ec50000), K(0x00f4, 0x3f0a8000),
    K(0x00e4, 0x42240000), K(0x00e8, 0x41e00000), K(0x0114, 0x3f3a8000), K(0x010c, 0x3f1a8000),
    K(0x0104, 0x41c80000), K(0x0124, 0x3f198000), K(0x0130, 0x3f458000), K(0x011c, 0x42040000),
    K(0x0120, 0x42340000), K(0x0140, 0x3f408000), K(0x014c, 0x3f668000), K(0x0144, 0x3d380000),
    K(0x0148, 0x3e120000), K(0x0138, 0x41d00000), Sheet(0x0150, 0x15ec), K(0x015c, 0x3f428000),
    K(0x0110, 0x3f458000), K(0x012c, 0x3f718000), K(0x013c, 0x421c0000), K(0x0108, 0x3f228000),
    K(0x0100, 0x42300000), K(0x0128, 0x3f518000), K(0x0168, 0x3f718000), Sheet(0x016c, 0x15ec),
    Sheet(0x0188, 0x15ec), Sheet(0x01a4, 0x15ec), K(0x0160, 0x3e5a0000), K(0x0164, 0x3eb90000),
    K(0x0154, 0x421c0000), K(0x0178, 0x3f438000), K(0x0184, 0x3f748000), K(0x017c, 0x3ed50000),
    K(0x0180, 0x3f118000), K(0x0170, 0x42200000), K(0x0174, 0x42480000), K(0x0194, 0x3f4b8000),
    K(0x01a0, 0x3f7b8000), K(0x018c, 0x423c0000), K(0x0190, 0x42440000), K(0x01b0, 0x3f4d8000),
    K(0x01b8, 0x3f728000), K(0x01a8, 0x42080000), Sheet(0x01c0, 0x15ec), K(0x01cc, 0x3c200000),
    K(0x01d8, 0x3f1b8000), K(0x01d0, 0x3bc00000), K(0x01b4, 0x3f518000), K(0x01ac, 0x42300000),
    K(0x0158, 0x42400000), K(0x0198, 0x3f188000), K(0x019c, 0x3f468000), K(0x01bc, 0x3f788000),
    K(0x01d4, 0x3f804000), Sheet(0x01dc, 0x15e8), Sheet(0x01f8, 0x15e8), Sheet(0x0214, 0x15e8),
    K(0x01c8, 0x431a0000), Sheet(0x0230, 0x15f0), K(0x0234, 0x43800000), K(0x01c4, 0x43800000),
    K(0x0240, 0x3b000000), K(0x01e8, 0x3f188000), K(0x01f4, 0x3f228000), K(0x01ec, 0x3f6f8000),
    K(0x01f0, 0x3f778000), K(0x01e0, 0x41100000), K(0x01e4, 0x41300000), K(0x0204, 0x3f188000),
    K(0x0210, 0x3f228000), K(0x0208, 0x3f788000), K(0x020c, 0x3f788000), K(0x01fc, 0x3f800000),
    K(0x0200, 0x41300000), K(0x0220, 0x3f188000), K(0x022c, 0x3f228000), K(0x0224, 0x3f798000),
    K(0x0228, 0x3f7f8000), K(0x0218, 0x40e00000), K(0x021c, 0x41300000), K(0x023c, 0x3b000000),
    K(0x0248, 0x3f398000), K(0x0244, 0x3f7f8000), K(0x0238, 0x433a0000), Sheet(0x024c, 0x15e8),
    Sheet(0x0268, 0x15e8), Sheet(0x0284, 0x15e8), Sheet(0x02a0, 0x15e8), K(0x0258, 0x3f088000),
    K(0x0264, 0x3f378000), K(0x025c, 0x3f138000), K(0x0260, 0x3f388000), K(0x0274, 0x3f0e8000),
    K(0x0280, 0x3f338000), K(0x0278, 0x3f398000), K(0x027c, 0x3f688000), K(0x026c, 0x42400000),
    K(0x0270, 0x42180000), K(0x0294, 0x3f618000), K(0x02ac, 0x3ee50000), K(0x02b8, 0x3f098000),
    K(0x02b0, 0x3f468000), K(0x02b4, 0x3f5d8000), K(0x02a8, 0x41c00000), K(0x0250, 0x42180000),
    K(0x0254, 0x42400000), K(0x0290, 0x3ee50000), K(0x029c, 0x3f098000), K(0x0298, 0x3f788000),
    K(0x0288, 0x41c00000), K(0x028c, 0x41c00000), K(0x02a4, 0x41c00000), Sheet(0x02bc, 0x15e8),
    K(0x02fc, 0x41300000), Sheet(0x02d8, 0x15e8), K(0x02cc, 0x3f778000), K(0x02d0, 0x3f6f8000),
    K(0x02c0, 0x41100000), K(0x02ec, 0x3f788000), K(0x02dc, 0x3f800000), Sheet(0x02f4, 0x15e8),
    K(0x0300, 0x3f188000), K(0x030c, 0x3f228000), K(0x0304, 0x3f7f8000), K(0x0308, 0x3f798000),
    K(0x02f8, 0x40e00000), K(0x02c8, 0x3f188000), K(0x02d4, 0x3f228000), K(0x02c4, 0x41300000),
    K(0x02e4, 0x3f188000), K(0x02f0, 0x3f228000), K(0x02e8, 0x3f788000), K(0x02e0, 0x41300000),
];

/// The poll most screens share (`c1PTrackLoad::PollAssetLoad` 0x245ec0 and five more, the records at another
/// place in each): 0 until the sound bank and the sprite set are in; then the sprite records (0x1c bytes each from
/// `base`: a sheet, a place, a size, the texture's corners) filled from the table the game builds them by, the
/// sheet words from the set's +0x15e8 / +0x15ec / +0x15f0, and the loaded flag (`base` + 0x310); 1
pub fn poll_sprites(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, base: u32) -> u32 {
    if call(m, env, BANK_POLL, &[m.r32(SPEECH)]) == 0 {
        return 0;
    }
    if call(m, env, SPRITES_POLL, &[m.r32(this + 4)]) == 0 {
        return 0;
    }
    let set = m.r32(this + 4);
    for s in SPRITE_TABLE {
        match s {
            K(o, v) | Int(o, v) => m.w32(this + base + o, v),
            Sheet(o, w) => m.w32(this + base + o, m.r32(set + w)),
        }
    }
    1
}

/// `c1PTrackLoad::c1PTrackLoad` 0x245840(this): the base's, its table, the bars' animation (+0x40c), 28 sprite
/// records (+0xb14) unset and full size, the fade's colours red, the event's summary filled; `this`
pub fn track1_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    init(m, env, this);
    m.w32(this, TRACK1_VTABLE);
    call(m, env, ANIM_INIT, &[this + 0x40c]);
    for i in 0..28 {
        let r = this + 0xb14 + 0x1c * i;
        m.w32(r, 0xffff_ffff);
        wf(m, r + 4, ONE);
        wf(m, r + 8, ONE);
        m.w32(r + 0xc, 0);
        m.w32(r + 0x10, 0);
        wf(m, r + 0x14, ONE);
        wf(m, r + 0x18, ONE);
    }
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    for i in (0..28).rev() {
        m.w32(this + 0xb14 + 0x1c * i, 0xffff_ffff);
    }
    fill_event_summary(m, env, this);
    this
}

/// `c1PTrackLoad::~c1PTrackLoad` 0x245958(this, flags): the sound bank unloaded, the animation deleted, then the
/// base's
pub fn track1_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this, TRACK1_VTABLE);
    call(m, env, BANK_UNLOAD, &[m.r32(SPEECH)]);
    call(m, env, ANIM_DELETE, &[this + 0x40c, 2]);
    delete(m, env, this, flags);
}

/// `LoadScreen_GetModePicName` 0x2459b0(mode, out): "ldmode…%s" with ".ssh" for modes 1 to 5 and 7 (free ride,
/// race, show-off, race, show-off, time challenge); nothing for the rest
pub fn mode_pic_name(m: &mut Mem, env: &mut dyn TextBoxEnv, mode: u32, out: u32) {
    let fmt = match mode {
        1 => 0x3a4d08,
        2 | 4 => 0x3a4cd0,
        3 | 5 => 0x3a4ce8,
        7 => 0x3a4cf8,
        _ => return,
    };
    call(m, env, SPRINTF, &[out, fmt, SSH]);
}

/// `LoadScreen_GetTrackPicName` 0x245a48(course, out): "ldtrack<name>%s" for courses 0 to 8 and 11
pub fn track_pic_name(m: &mut Mem, env: &mut dyn TextBoxEnv, course: u32, out: u32) {
    const T: [u32; 12] = [0x3a4d40, 0x3a4d50, 0x3a4d60, 0x3a4d70, 0x3a4d80, 0x3a4d90, 0x3a4da0, 0x3a4db0, 0x3a4dc0, 0, 0, 0x3a4dd0];
    if course >= 12 || T[course as usize] == 0 {
        return;
    }
    call(m, env, SPRINTF, &[out, T[course as usize], SSH]);
}

/// `LoadScreen_GetRiderPicName` 0x245b68(character, out): "ldrider<name>%s" for the twelve characters
pub fn rider_pic_name(m: &mut Mem, env: &mut dyn TextBoxEnv, ch: u32, out: u32) {
    if ch >= 12 {
        return;
    }
    call(m, env, SPRINTF, &[out, 0x3a4e10 + 0x10 * ch, SSH]);
}

/// The career's event mode: its +0x2afc, or the World Circuit's +0x3818 while its +0x3708 is 1
fn event_mode(m: &Mem) -> u32 {
    if m.r32(CAREER + 0x3708) == 1 { m.r32(CAREER + 0x3818) } else { m.r32(CAREER + 0x2afc) }
}

/// A shape's 4-letter name and its 0 at a picture's +0x29
fn put_shape_name(m: &mut Mem, e: u32, name: u32) {
    for k in 0..5 {
        let b = m.r8(name + k);
        m.w8(e + 0x29 + k, b);
    }
}

/// `c1PTrackLoad::StartAssetLoad` 0x245cb8(this): the sound bank, the sprite set and its icons (0x2cc) asked for,
/// not yet in; three pictures (+0xe28): the event mode's, the course's and the player's rider's, each with its
/// shape's name; then their load started
pub fn track1_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let list = this + 0xe28;
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0xe24, 0);
    for i in 0..3u32 {
        let e = list + 0x34 * i;
        m.w32(e, 0);
        m.w8(e + 4, 0);
        m.w8(e + 0x29, 0);
        m.w32(e + 0x30, 0xffff_ffff);
        match i {
            0 => {
                let mode = event_mode(m);
                mode_pic_name(m, env, mode, e + 4);
                put_shape_name(m, e, SHAPE_MODE);
            }
            1 => {
                let c = selected_course(m);
                track_pic_name(m, env, c, e + 4);
                put_shape_name(m, e, SHAPE_TRACK);
            }
            _ => {
                let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
                let p = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
                rider_pic_name(m, env, p, e + 4);
                put_shape_name(m, e, SHAPE_RIDER);
            }
        }
    }
    load_pics(m, env, this, list, 3);
}

/// `c1PTrackLoad::PollAssetLoad` 0x245ec0(this): [`poll_sprites`] with the records at +0xb14
pub fn track1_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0xb14)
}

fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}
fn copy_loc(m: &mut Mem, env: &mut dyn TextBoxEnv, to: u32, id: u32) {
    let s = loc(m, env, id);
    call(m, env, WSTR_COPY, &[to, s]);
}

/// `FE_FillEventSummary` 0x246700(this): the event's lines (wide strings at +0x6a0.. 0x3c apart) for the screen:
/// the mode's heading (+0x6a0: 0xe2b free ride and lesson, 0xe71 race, show-off and time challenge, 0xe2f
/// otherwise; nothing at all for modes 8 and 9), the mode's line (+0x6dc), the course's name, the labels
/// 0x11d..0x124 and 0x11c, the course's difficulty, location, drop and length, the player's character and level,
/// the trick book's pages ("%d" of the next, or 0xc9e when all 6 are done), the event's medal line; then the bars'
/// animation's lines, each as long as its text
pub fn fill_event_summary(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let mode = event_mode(m);
    let heading = match mode {
        1 | 6 => 0xe2b,
        2 | 3 | 7 => 0xe71,
        8 | 9 => return,
        _ => 0xe2f,
    };
    copy_loc(m, env, this + 0x6a0, heading);
    let line = match mode {
        2 | 4 => 0xe2c,
        3 | 5 => 0xe2d,
        6 => 0xe2e,
        7 => 0xe72,
        _ => 0xe2a,
    };
    copy_loc(m, env, this + 0x6dc, line);
    let t = m.r32(m.r32(APP) + 0x50);
    let c = selected_course(m);
    let id = call(m, env, COURSE_NAME, &[CAREER, c]);
    let s = call(m, env, LOC, &[t, id]);
    call(m, env, WSTR_COPY, &[this + 0x718, s]);
    let pid = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let profile = call(m, env, PLAYER_PROFILE, &[CAREER, pid]);
    for (o, id) in [(0x754u32, 0x11du32), (0x790, 0x11f), (0x7cc, 0x124), (0x808, 0x11e), (0x844, 0x121), (0x880, 0x120), (0x8bc, 0x123), (0x8f8, 0x11c)] {
        copy_loc(m, env, this + o, id);
    }
    for (o, f) in [(0x934u32, COURSE_DIFFICULTY), (0x970, COURSE_LOCATION), (0x9ac, COURSE_DROP), (0x9e8, COURSE_LENGTH)] {
        let c = selected_course(m);
        let s = call(m, env, f, &[CAREER, c]);
        call(m, env, WSTR_COPY, &[this + o, s]);
    }
    let name = m.r32(profile.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    call(m, env, WSTR_SET_ASCII, &[this + 0xa24, name]);
    let pid = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let lv = call(m, env, LEVEL_NAME, &[CAREER, pid]);
    call(m, env, WSTR_COPY, &[this + 0xa60, lv]);
    let pid = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let book = call(m, env, TRICK_BOOK, &[CAREER, pid]);
    let pages = call(m, env, COMPLETE_PAGES, &[book]);
    if pages == 6 {
        let f = loc(m, env, 0xc9e);
        call(m, env, WSTR_FORMAT, &[this + 0xa9c, f, pages]);
    } else {
        call(m, env, WSTR_FORMAT_ASCII, &[this + 0xa9c, FMT_D, pages.wrapping_add(1)]);
    }
    let c = selected_course(m);
    let medal = call(m, env, EVENT_MEDAL, &[CAREER, mode, c]);
    let id = match medal {
        0 => 0x12f,
        1 => 0x135,
        2 => 0x12c,
        _ => 0x132,
    };
    copy_loc(m, env, this + 0xad8, id);
    let anim = this + 0x40c;
    let count = |m: &mut Mem, env: &mut dyn TextBoxEnv, at: u32| call(m, env, WSTR_COUNT, &[at]);
    let bar = |m: &mut Mem, env: &mut dyn TextBoxEnv, a: [u32; 6]| {
        call(m, env, ANIM_ADD_BAR, &[anim, a[0], a[1], a[2], a[3], a[4], a[5]]);
    };
    let n = count(m, env, this + 0x6a0);
    bar(m, env, [1, 1, 2, 0, 0, n]);
    bar(m, env, [2, 1, 0x46, 3, 0, 1000]);
    bar(m, env, [0x13, 1, 1, 0, 0, 0x82]);
    bar(m, env, [4, 1, 0x46, 3, 0, 1000]);
    let n = count(m, env, this + 0xad8);
    bar(m, env, [0x11, 1, 2, 0, 0, n.wrapping_add(0x3c)]);
    let n = count(m, env, this + 0x718);
    bar(m, env, [5, 2, 2, 0, 0, n]);
    bar(m, env, [6, 2, 0x3c, 3, 0, 1000]);
    bar(m, env, [7, 3, 0x32, 2, 0x190, 1000]);
    bar(m, env, [8, 3, 0x46, 3, 0, 1000]);
    for (o, line, row) in [(0x934u32, 9u32, 3u32), (0x970, 0xa, 4), (0x9ac, 0xb, 5), (0x9e8, 0xc, 6)] {
        let n = count(m, env, this + o);
        bar(m, env, [line, row, 2, 0, 0, n.wrapping_add(0x14)]);
    }
    bar(m, env, [0xd, 7, 0x32, 3, 0, 1000]);
    for (o, line, row) in [(0xa24u32, 0xeu32, 7u32), (0xa60, 0xf, 8), (0xa9c, 0x10, 9)] {
        let n = count(m, env, this + o);
        bar(m, env, [line, row, 2, 0, 0, n.wrapping_add(0x14)]);
    }
}

/// vf4 0x246f18(this): the loading loop played, then the bars started
pub fn track1_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    call(m, env, LOOP_PLAY, &[m.r32(SPEECH), 1]);
    start_bars(m, env, this, scratch);
}

/// vf3 0x246f50(this): 1 once the fade is in; else the pictures' archive closed, 0
pub fn track1_vf3(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) -> u32 {
    if fade_in(m, this) != 0 {
        return 1;
    }
    close_archive(m, env, this, scratch);
    0
}

/// vf5 0x246f88(this): the base's tick, the summary's animation, and the loading loop stopped once the app's +8
/// reaches 8
pub fn track1_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    tick(m, env, this);
    call(m, env, ANIM_UPDATE, &[this + 0x40c]);
    if (m.r32(m.r32(APP) + 8) as i32) >= 8 {
        call(m, env, LOOP_STOP, &[m.r32(SPEECH), 1]);
    }
}

fn anim_value(m: &mut Mem, env: &mut dyn TextBoxEnv, anim: u32, line: u32) -> F {
    F(f32::from_bits(call(m, env, ANIM_EVAL, &[anim, line])))
}

/// `c1PTrackLoad::DrawBackground` 0x246fe0(this): once the sprites are in, the bars and the ticker; each picture that
/// is in in a tip box (the mode's with the mode's line at y 100, the course's at 199, the rider's at 300, faded by
/// the animation's lines 4, 8 and 13); the map (sprite +0xb30, 1.8 wide, 80 / its height high) and the course's
/// marker on it (sprite +0xb4c) unless the course is 7; the mode's badge (sprite +0xb14) in modes 4 and 5; the
/// flakes
pub fn track1_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mode = event_mode(m);
    let hscale = crate::animseq::div(F(80.0), rf(m, this + 0xb38));
    if m.r32(this + 0xe24) == 0 {
        return;
    }
    let recs = this + 0xb14;
    let badge = mode.wrapping_sub(4) < 2;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    let anim = this + 0x40c;
    for i in 0..3u32 {
        let e = this + 0xe28 + 0x34 * i;
        if m.r32(e) != 1 {
            continue;
        }
        let (line, y, text) = match i {
            0 => (4, F(100.0), this + 0x6dc),
            1 => (8, k(0x4347_0000), 0),
            _ => (0xd, F(300.0), 0),
        };
        let a = anim_value(m, env, anim, line);
        draw_tip_box(m, env, this, m.r32(e + 0x30), text, 7, F(23.0), y, a, scratch);
    }
    rcall(m, env, 0x1d0, &[8], &[]);
    let wide = k(0x3fe6_6666);
    let x = F(320.0);
    let grey = k(0x3ee6_6666);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 0xc, grey);
    wf(m, DRAW_COLOUR + 4, grey);
    wf(m, DRAW_COLOUR + 8, grey);
    let a = anim_value(m, env, anim, 6);
    let v = rf(m, DRAW_COLOUR) * a;
    wf(m, DRAW_COLOUR, v);
    fcall(m, env, SPRITE_RECT, &[this + 0xb30, 0], &[x, ZERO, wide, hscale]);
    rcall(m, env, 0x1d0, &[9], &[]);
    let mw = rf(m, this + 0xb34) * wide;
    let mh = rf(m, this + 0xb38) * hscale;
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let s = anim_value(m, env, anim, 7);
    if selected_course(m) != 7 {
        let c = selected_course(m);
        let mx = F(f32::from_bits(call(m, env, MAP_X, &[CAREER, c])));
        let px = mw * mx + x;
        let c = selected_course(m);
        let my = F(f32::from_bits(call(m, env, MAP_Y, &[CAREER, c])));
        fcall(m, env, SPRITE_RECT, &[this + 0xb4c, 0x60], &[px, mh * my + ZERO, s, s]);
    }
    rcall(m, env, 0x1d0, &[8], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let a = anim_value(m, env, anim, 2);
    let v = rf(m, DRAW_COLOUR) * a;
    wf(m, DRAW_COLOUR, v);
    if badge {
        let b = k(0x3f66_6666);
        fcall(m, env, SPRITE_RECT, &[recs, 0], &[F(10.0), ZERO, b, b]);
    }
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// `c1PTrackLoad::DrawOverlay` 0x2473d8(this): on texture 10, two text styles (the title and menu fonts, +4 set):
/// in modes 4 and 5 the headings 0xe73 and 0xe74 in gold, else the mode's heading typed in (the animation's line
/// 1) at 1.2; the course's name typed in (line 5) in pink; the labels right-aligned in black (from y 186, 26 apart;
/// and the medal's in modes 4 and 5) and their values typed in with the cursor on the animation's current line
pub fn track1_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mode = event_mode(m);
    let badge = mode.wrapping_sub(4) < 2;
    let (title, menu) = (this + 0x48, this + 0xc);
    let (a, b) = (scratch, scratch + 0x10);
    let col = scratch + 0x20;
    rcall(m, env, 0x1d0, &[0xa], &[]);
    call(m, env, TS_INIT, &[a]);
    call(m, env, TS_CONSTRUCT, &[b]);
    call(m, env, TS_SET_FONT, &[a, title]);
    m.w32(a + 4, 1);
    call(m, env, TS_SET_FONT, &[b, menu]);
    m.w32(b + 4, 1);
    let anim = this + 0x40c;
    let gold = [k(0x3f80_0000), k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)];
    if badge {
        put4(m, col, gold);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let big = k(0x3f8c_cccd);
        let s = loc(m, env, 0xe73);
        fcall(m, env, PRINTF_SHADOWED_A, &[title, s], &[big, F(15.0), F(25.0)]);
        let s = loc(m, env, 0xe74);
        fcall(m, env, PRINTF_SHADOWED_A, &[title, s], &[big, F(46.0), F(54.0)]);
    } else {
        put4(m, col, gold);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let n = call(m, env, ANIM_VALUE, &[anim, 1]);
        fcall(m, env, DRAW_ALIGNED, &[a, 0, this + 0x6a0, n, 0, 1], &[k(0x3f99_999a), F(30.0), F(30.0)]);
    }
    put4(m, col, [ONE, k(0x3f09_78d5), k(0x3f45_e354), k(0x3f53_b646)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let n = call(m, env, ANIM_VALUE, &[anim, 5]);
    fcall(m, env, DRAW_ALIGNED, &[a, 0, this + 0x718, n, 0, 1], &[k(0x3f4c_cccd), k(0x43c5_8000), F(30.0)]);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 0xc, ZERO);
    wf(m, DRAW_COLOUR + 4, ZERO);
    wf(m, DRAW_COLOUR + 8, ZERO);
    let (lx, sc) = (F(460.0), k(0x3f94_7ae1));
    let ys = [k(0x433a_0000), k(0x4354_0000), k(0x436e_0000), k(0x4384_0000), k(0x4397_8000), k(0x43a4_8000), k(0x43b1_8000)];
    for (i, o) in [0x754u32, 0x790, 0x7cc, 0x808, 0x844, 0x880, 0x8bc].into_iter().enumerate() {
        fcall(m, env, DRAW_RIGHT, &[menu, this + o], &[lx, ys[i], sc]);
    }
    let vx = k(0x43ed_8000);
    let step = m.r32(anim);
    if badge {
        let y = k(0x4303_0000);
        fcall(m, env, DRAW_RIGHT, &[menu, this + 0x8f8], &[lx, y, sc]);
        let n = call(m, env, ANIM_VALUE, &[anim, 0x11]);
        let step = m.r32(anim);
        fcall(m, env, DRAW_CURSOR, &[b, 0, this + 0xad8, n, 0, (step == 1) as u32], &[sc, vx, y]);
    }
    let _ = step;
    for (i, (line, o, at)) in [(9u32, 0x934u32, 3u32), (0xa, 0x970, 4), (0xb, 0x9ac, 5), (0xc, 0x9e8, 6), (0xe, 0xa24, 7), (0xf, 0xa60, 8), (0x10, 0xa9c, 9)].into_iter().enumerate() {
        let n = call(m, env, ANIM_VALUE, &[anim, line]);
        let step = m.r32(anim);
        fcall(m, env, DRAW_CURSOR, &[b, 0, this + o, n, 0, (step == at) as u32], &[sc, vx, ys[i]]);
    }
    call(m, env, TS_DESTRUCT_THUNK, &[b, 2]);
    call(m, env, TS_DESTRUCT, &[a, 2]);
}

// --- G9w3: the two-player track load ---

pub const TRACK2_VTABLE: u32 = 0x3a5218;
/// The two players' pictures' shape names, "ldr1" and "ldr2"
pub const SHAPE_R1: u32 = 0x3a4f78;
pub const SHAPE_R2: u32 = 0x3a4f80;
/// The four labels' ids the two-player summary uses (0xe81..0xe84, written there by it)
pub const TRACK2_LABELS: u32 = 0x3463f8;
/// The progress bar's tables (a widget built on the stack): the thing's, the sprite's, the bar's
pub const VT_THING: u32 = 0x3b5a00;
pub const VT_SPRITE: u32 = 0x3b5e18;
pub const VT_BAR: u32 = 0x3b6cf8;
pub const SET_COPY_RECT: u32 = 0x1f9e90;
pub const ATTRIBUTES: u32 = 0x164638;
pub const WIDGET_COLOUR: u32 = 0x2ad988;
pub const WIDGET_POS: u32 = 0x2acb20;
pub const WIDGET_VALUE: u32 = 0x2b3950;
pub const BAR_DRAW: u32 = 0x2ae880;
pub const BAR_DELETE: u32 = 0x2b3a20;
pub const GRADIENT_RECT: u32 = 0x19b2a0;
pub const DRAW_STRING: u32 = 0x19d0c0;
pub const DRAW_CENTRED: u32 = 0x19d278;
pub const MEASURE_WIDE: u32 = 0x19dc78;

/// `c2PTrackLoad::c2PTrackLoad` 0x247a60(this): as the one-player's, its animation at +0x410 and its 28 records at
/// +0x9ec, the summary from `LoadScreen_FillEventText`
pub fn track2_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    init(m, env, this);
    m.w32(this, TRACK2_VTABLE);
    call(m, env, ANIM_INIT, &[this + 0x410]);
    for i in 0..28 {
        let r = this + 0x9ec + 0x1c * i;
        m.w32(r, 0xffff_ffff);
        wf(m, r + 4, ONE);
        wf(m, r + 8, ONE);
        m.w32(r + 0xc, 0);
        m.w32(r + 0x10, 0);
        wf(m, r + 0x14, ONE);
        wf(m, r + 0x18, ONE);
    }
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    for i in (0..28).rev() {
        m.w32(this + 0x9ec + 0x1c * i, 0xffff_ffff);
    }
    fill_event_text(m, env, this);
    this
}

/// `c2PTrackLoad::~c2PTrackLoad` 0x247b78(this, flags)
pub fn track2_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this, TRACK2_VTABLE);
    call(m, env, BANK_UNLOAD, &[m.r32(SPEECH)]);
    call(m, env, ANIM_DELETE, &[this + 0x410, 2]);
    delete(m, env, this, flags);
}

/// `c2PTrackLoad::StartAssetLoad` 0x247bd0(this): as the one-player's, with two pictures (+0xcfc): each player's
/// rider
pub fn track2_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let list = this + 0xcfc;
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0x40c, 0);
    for i in 0..2u32 {
        let e = list + 0x34 * i;
        m.w32(e, 0);
        m.w8(e + 4, 0);
        m.w8(e + 0x29, 0);
        m.w32(e + 0x30, 0xffff_ffff);
        let id = call(m, env, PLAYER_ID, &[CAREER, i]);
        let p = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
        rider_pic_name(m, env, p, e + 4);
        put_shape_name(m, e, if i == 0 { SHAPE_R1 } else { SHAPE_R2 });
    }
    load_pics(m, env, this, list, 2);
}

/// `c2PTrackLoad::PollAssetLoad` 0x247d58(this): as [`poll_sprites`] but with its own table (the records at +0x9ec,
/// the flag +0x40c), then six records of the sprite set copied over others (0xc7 ← 0x11, 0xc6 ← 0x12, 0xc5 ←
/// 0x13, 0xc4 ← 0x19, 0xc3 ← 0x1a, 0xc2 ← 0x1b)
pub fn track2_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    if call(m, env, BANK_POLL, &[m.r32(SPEECH)]) == 0 {
        return 0;
    }
    if call(m, env, SPRITES_POLL, &[m.r32(this + 4)]) == 0 {
        return 0;
    }
    let set = m.r32(this + 4);
    for s in TRACK2_TABLE {
        match s {
            K(o, v) | Int(o, v) => m.w32(this + o, v),
            Sheet(o, w) => m.w32(this + o, m.r32(set + w)),
        }
    }
    for (to, from) in [(0xc7u32, 0x11u32), (0xc6, 0x12), (0xc5, 0x13), (0xc4, 0x19), (0xc3, 0x1a), (0xc2, 0x1b)] {
        call(m, env, SET_COPY_RECT, &[m.r32(this + 4), to, from]);
    }
    1
}

/// `LoadScreen_FillEventText` 0x2485f8(this): the two-player summary: the labels' ids 0xe81..0xe84 kept at 0x3463f8;
/// the mode's heading (+0x6a4) and line (+0x6e0) as the one-player's (nothing more for modes 8 and 9); the two
/// players' characters (+0x884, +0x8c0); the labels 0x120 (+0x71c), 0x123 (+0x848) and the four kept ones (+0x758..
/// 0x3c apart); the players' levels (+0x8fc, +0x938) and trick book pages (+0x974, +0x9b0)
pub fn fill_event_text(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let mode = event_mode(m);
    m.w32(TRACK2_LABELS + 0xc, 0xe84);
    m.w32(TRACK2_LABELS, 0xe81);
    m.w32(TRACK2_LABELS + 4, 0xe82);
    m.w32(TRACK2_LABELS + 8, 0xe83);
    let heading = match mode {
        1 | 6 => 0xe2b,
        2 | 3 | 7 => 0xe71,
        8 | 9 => return,
        _ => 0xe2f,
    };
    copy_loc(m, env, this + 0x6a4, heading);
    let line = match mode {
        2 | 4 => 0xe2c,
        3 | 5 => 0xe2d,
        6 => 0xe2e,
        7 => 0xe72,
        _ => 0xe2a,
    };
    copy_loc(m, env, this + 0x6e0, line);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let p0 = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 1]);
    let p1 = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    for (o, p) in [(0x884u32, p0), (0x8c0, p1)] {
        let name = m.r32(p.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
        call(m, env, WSTR_SET_ASCII, &[this + o, name]);
    }
    copy_loc(m, env, this + 0x71c, 0x120);
    copy_loc(m, env, this + 0x848, 0x123);
    for (k, o) in [0x758u32, 0x794, 0x7d0, 0x80c].into_iter().enumerate() {
        let id = m.r32(TRACK2_LABELS + 4 * k as u32);
        copy_loc(m, env, this + o, id);
    }
    for (o, p) in [(0x8fcu32, 0u32), (0x938, 1)] {
        let id = call(m, env, PLAYER_ID, &[CAREER, p]);
        let lv = call(m, env, LEVEL_NAME, &[CAREER, id]);
        call(m, env, WSTR_COPY, &[this + o, lv]);
    }
    let mut pages = [0u32; 2];
    for (p, slot) in pages.iter_mut().enumerate() {
        let id = call(m, env, PLAYER_ID, &[CAREER, p as u32]);
        let book = call(m, env, TRICK_BOOK, &[CAREER, id]);
        *slot = call(m, env, COMPLETE_PAGES, &[book]);
    }
    for (o, n) in [(0x974u32, pages[0]), (0x9b0, pages[1])] {
        if n == 6 {
            let f = loc(m, env, 0xc9e);
            call(m, env, WSTR_FORMAT, &[this + o, f]);
        } else {
            call(m, env, WSTR_FORMAT_ASCII, &[this + o, FMT_D, n.wrapping_add(1)]);
        }
    }
}

/// vf4 0x248a60(this): the loading loop played, then the bars started
pub fn track2_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    track1_vf4(m, env, this, scratch);
}

/// vf3 0x248a98(this): as the one-player's
pub fn track2_vf3(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) -> u32 {
    track1_vf3(m, env, this, scratch)
}

/// vf5 0x248ad0(this): the base's tick, and the loading loop stopped once the app's +8 reaches 8
pub fn track2_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    tick(m, env, this);
    if (m.r32(m.r32(APP) + 8) as i32) >= 8 {
        call(m, env, LOOP_STOP, &[m.r32(SPEECH), 1]);
    }
}

fn wu(m: &mut Mem, a: u32, v: u32) {
    m.w32(a, v);
}

/// The bar's virtual at its table's 0x138 (its size or place set from four floats at `v`)
fn bar_call(m: &mut Mem, env: &mut dyn TextBoxEnv, bar: u32, v: u32) {
    let vt = m.r32(bar + 0xc);
    let me = bar.wrapping_add(m.r16(vt + 0x138) as i16 as u32);
    let func = m.r32(vt + 0x13c);
    env.call(m, func, &[me, v], &[]);
}

/// `c2PTrackLoad::DrawBackground` 0x248b18(this): once the sprites are in, the bars and the ticker; then a progress
/// bar built in place on the stack (`scratch`: a `cFE3DThing` made a sprite and then a `cFE3DProgressBar`, 0x190
/// bytes, gold, 100 by 30) and, for each of the four attributes of the two players (`Career_GetPlayerAttributes`),
/// it is sized (its +0xc · 3.75 wide), given its value (+4, out of +0xc), placed (215 − the width for the first,
/// 425 for the second; rows from 237, its height + 10 apart) and drawn; the players' pictures in tip boxes at 23
/// and 360; the flakes; the bar deleted
pub fn track2_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0x40c) == 0 {
        return;
    }
    let sp = scratch;
    let recs = this + 0x9ec;
    draw_bars(m, env, this, recs, scratch + 0x400);
    draw_ticker(m, env, recs, scratch + 0x400);
    let one = ONE.0.to_bits();
    let half = 0x3f00_0000u32;
    let gold = [0x3f80_0000u32, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687];
    // the thing
    wu(m, sp, 0xffff_ffff);
    wu(m, sp + 0x40, 0);
    wu(m, sp + 0xc, VT_THING);
    wu(m, sp + 4, sp);
    wu(m, sp + 8, sp);
    for (o, v) in [(0x10u32, 0u32), (0x14, 1), (0x18, 0), (0x1c, 0), (0x20, 0), (0x24, 0), (0x28, 1), (0x2c, 0), (0x30, 0)] {
        wu(m, sp + o, v);
    }
    for o in [0x4cu32, 0x44, 0x48, 0x50, 0x5c, 0x54, 0x58] {
        wu(m, sp + o, 0);
    }
    for o in [0x60u32, 0x6c, 0x64, 0x68] {
        wu(m, sp + o, one);
    }
    wu(m, sp + 0x234, 3);
    for o in [0x190u32, 0x194, 0x198, 0x70, 0x74] {
        wu(m, sp + o, 0);
    }
    wu(m, sp + 0x19c, one);
    for k in 0..4 {
        let v = m.r32(sp + 0x190 + 4 * k);
        wu(m, sp + 0x80 + 4 * k, v);
    }
    for o in [0x190u32, 0x198, 0x194] {
        wu(m, sp + o, 0);
    }
    for o in [0x90u32, 0x94, 0x98] {
        wu(m, sp + o, 0);
    }
    for (k, v) in [(0xacu32, gold[0]), (0xb8, gold[3]), (0xb0, gold[1]), (0xb4, gold[2])] {
        wu(m, sp + k, v);
    }
    for o in [0xbcu32, 0xc8, 0xc0, 0xc4, 0xcc] {
        wu(m, sp + o, one);
    }
    for o in [0xd8u32, 0xd0, 0xd4] {
        wu(m, sp + o, half);
    }
    wu(m, sp + 0xdc, one);
    for o in [0xe8u32, 0xe0, 0xe4] {
        wu(m, sp + o, half);
    }
    // the sprite
    wu(m, sp + 0xc, VT_SPRITE);
    wu(m, sp + 0xa8, 1);
    for o in [0xa0u32, 0xa4, 0xec, 0xf0, 0x100, 0x104, 0x108] {
        wu(m, sp + o, 0);
    }
    wu(m, sp + 0x10c, one);
    call(m, env, WIDGET_COLOUR, &[sp]);
    // the bar
    for (o, v) in [(0x110u32, one), (0x11c, 0), (0x114, one), (0x118, one), (0x120, one), (0x12c, 0), (0x124, one), (0x128, one), (0x130, one)] {
        wu(m, sp + o, v);
    }
    let fifth = 0x3e4c_cccdu32;
    for o in [0x13cu32, 0x134, 0x138] {
        wu(m, sp + o, fifth);
    }
    wu(m, sp + 0x140, one);
    let far = 0xc61c_3c00u32;
    for (o, v) in [(0x14cu32, 0), (0x144, one), (0x148, 0), (0x15c, far), (0x174, 0xffff_ffff), (0x158, far), (0x16c, 0xffff_ffff), (0x170, 0xffff_ffff)] {
        wu(m, sp + o, v);
    }
    wu(m, sp + 0xc, VT_BAR);
    let hundred = F(100.0).0.to_bits();
    for (o, v) in [(0x150u32, hundred), (0x154, 0), (0x160, 0), (0x164, one), (0x168, 0), (0x178, one), (0x17c, one), (0x180, one), (0x184, one), (0x188, 0), (0x18c, 0)] {
        wu(m, sp + o, v);
    }
    for (o, v) in [(0x1a0u32, hundred), (0x1a4, F(30.0).0.to_bits()), (0x1a8, 0), (0x1ac, 0)] {
        wu(m, sp + o, v);
    }
    bar_call(m, env, sp, sp + 0x1a0);
    // its colours
    for (o, v) in [(0x110u32, [one; 4]), (0x120, [one, one, 0x3ef1_26e9, 0x3d1f_be77]), (0x140, [one, one, 0, 0])] {
        for (k, w) in v.into_iter().enumerate() {
            wu(m, sp + 0x190 + 4 * k as u32, w);
        }
        for k in 0..4 {
            let w = m.r32(sp + 0x190 + 4 * k);
            wu(m, sp + o + 4 * k, w);
        }
    }
    for (p, at) in [(0u32, 0x1b0u32), (1, 0x1f0)] {
        let id = call(m, env, PLAYER_ID, &[CAREER, p]);
        call(m, env, ATTRIBUTES, &[CAREER, id, sp + at]);
    }
    let x0 = m.r32(sp + 0x100);
    for (o, v) in [(0x190u32, x0), (0x194, F(10.0).0.to_bits()), (0x19c, 0), (0x198, 0)] {
        wu(m, sp + o, v);
    }
    bar_call(m, env, sp, sp + 0x190);
    let mut y = k(0x436d_0000);
    let scale = k(0x4070_0000);
    for row in 0..4u32 {
        let a = sp + 0x1b0 + 0x10 * row;
        let b = sp + 0x1f0 + 0x10 * row;
        // the first player's bar
        for (o, v) in [(0x16cu32, 0xc3u32), (0x174, 0xc4), (0x170, 0xc2), (0x168, 1)] {
            wu(m, sp + o, v);
        }
        let w = cvt_s(m.r32(a + 0xc)) * scale;
        let h = m.r32(sp + 0x104);
        wu(m, sp + 0x194, h);
        wu(m, sp + 0x198, 0);
        wu(m, sp + 0x19c, 0);
        wf(m, sp + 0x190, w);
        bar_call(m, env, sp, sp + 0x190);
        wf(m, sp + 0x150, cvt_s(m.r32(a + 0xc)));
        fcall(m, env, WIDGET_VALUE, &[sp], &[cvt_s(m.r32(a + 4))]);
        let total = cvt_s(m.r32(a + 4)) + cvt_s(m.r32(a + 8));
        wf(m, sp + 0x184, ONE);
        wf(m, sp + 0x194, y);
        wf(m, sp + 0x190, F(215.0) - w);
        wf(m, sp + 0x180, cvt_s(m.r32(a + 0xc)));
        wf(m, sp + 0x158, total);
        wu(m, sp + 0x198, 0);
        wf(m, sp + 0x19c, ONE);
        call(m, env, WIDGET_POS, &[sp, sp + 0x190]);
        call(m, env, BAR_DRAW, &[sp]);
        // the second's
        for (o, v) in [(0x174u32, 0xc7u32), (0x16c, 0xc6), (0x170, 0xc5), (0x168, 0)] {
            wu(m, sp + o, v);
        }
        let w = cvt_s(m.r32(b + 0xc)) * scale;
        let h = m.r32(sp + 0x104);
        wu(m, sp + 0x194, h);
        wu(m, sp + 0x198, 0);
        wu(m, sp + 0x19c, 0);
        wf(m, sp + 0x190, w);
        bar_call(m, env, sp, sp + 0x190);
        wf(m, sp + 0x150, cvt_s(m.r32(b + 0xc)));
        fcall(m, env, WIDGET_VALUE, &[sp], &[cvt_s(m.r32(b + 4))]);
        let total = cvt_s(m.r32(b + 4)) + cvt_s(m.r32(b + 8));
        wf(m, sp + 0x184, ONE);
        wf(m, sp + 0x194, y);
        wf(m, sp + 0x190, k(0x43d4_8000));
        wf(m, sp + 0x180, cvt_s(m.r32(b + 0xc)));
        wu(m, sp + 0x198, 0);
        wf(m, sp + 0x158, total);
        wf(m, sp + 0x19c, ONE);
        call(m, env, WIDGET_POS, &[sp, sp + 0x190]);
        call(m, env, BAR_DRAW, &[sp]);
        let step = rf(m, sp + 0x104) + F(10.0);
        y = y + step;
        wu(m, sp + 0x234, 2u32.wrapping_sub(row));
    }
    for i in 0..2u32 {
        let e = this + 0xcfc + 0x34 * i;
        if m.r32(e) != 1 {
            continue;
        }
        let x = if i == 0 { F(23.0) } else { F(360.0) };
        draw_tip_box(m, env, this, m.r32(e + 0x30), 0, 7, x, F(100.0), ONE, scratch + 0x400);
    }
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
    call(m, env, BAR_DELETE, &[sp, 2]);
}

fn cvt_s(v: u32) -> F {
    crate::animseq::cvt_s_w(v as i32)
}

/// 0x249250(this; x, y, w, h): a divider: two rects from x − w/2 and x, w/2 wide, black fading out to their
/// outer sides (the corners' colours on the stack)
pub fn draw_divider(m: &mut Mem, env: &mut dyn TextBoxEnv, x: F, y: F, w: F, h: F, scratch: u32) {
    rcall(m, env, 0x1f0, &[0xffff_ffff, 0], &[]);
    let half = w * F(0.5);
    let sp = scratch;
    let one = ONE.0.to_bits();
    let mut c = [0u32; 16];
    c[4] = one;
    c[12] = one;
    for (k, v) in c.into_iter().enumerate() {
        m.w32(sp + 4 * k as u32, v);
    }
    fcall(m, env, GRADIENT_RECT, &[DRAW_COLOUR, sp, sp + 0x10, sp + 0x20, sp + 0x30], &[x - half, y, half, h]);
    let mut c = [0u32; 16];
    c[0] = one;
    c[8] = one;
    for (k, v) in c.into_iter().enumerate() {
        m.w32(sp + 4 * k as u32, v);
    }
    fcall(m, env, GRADIENT_RECT, &[DRAW_COLOUR, sp, sp + 0x10, sp + 0x20, sp + 0x30], &[x, y, half, h]);
}

/// `c2PTrackLoad::DrawOverlay` 0x2493e0(this): on texture 6 (nothing more in modes 8 and 9): the heading in gold in
/// the title font (30, 30, 1.2; measured too), then in black the dividers (600 wide at 197, 227, 317, 347) and
/// between them the centred lines in the title font at 0.7 (0x71c, the four labels, 0x848), and the players' levels
/// and pages in the menu font at 140 and 490
pub fn track2_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let mode = event_mode(m);
    rcall(m, env, 0x1d0, &[6], &[]);
    if !((mode as i32) >= 10 || (mode as i32) < 8) {
        return;
    }
    let (title, menu) = (this + 0x48, this + 0xc);
    put4(m, scratch, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let big = k(0x3f99_999a);
    fcall(m, env, DRAW_STRING, &[title, this + 0x6a4], &[F(30.0), F(30.0), big]);
    fcall(m, env, MEASURE_WIDE, &[title, this + 0x6a4, 0], &[big]);
    put4(m, scratch, [ONE, ZERO, ZERO, ZERO]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let (cx, w, h) = (F(320.0), F(600.0), F(2.0));
    let s = k(0x3f33_3333);
    draw_divider(m, env, cx, k(0x4345_0000), w, h, scratch + 0x40);
    fcall(m, env, DRAW_CENTRED, &[title, this + 0x71c], &[cx, k(0x434d_0000), s]);
    draw_divider(m, env, cx, k(0x4363_0000), w, h, scratch + 0x40);
    for (o, y) in [(0x758u32, 0x436b_0000u32), (0x794, 0x437f_0000), (0x7d0, 0x4389_8000), (0x80c, 0x4393_8000)] {
        fcall(m, env, DRAW_CENTRED, &[title, this + o], &[cx, k(y), s]);
    }
    draw_divider(m, env, cx, k(0x439e_8000), w, h, scratch + 0x40);
    fcall(m, env, DRAW_CENTRED, &[title, this + 0x848], &[cx, k(0x43a2_8000), s]);
    draw_divider(m, env, cx, k(0x43ad_8000), w, h, scratch + 0x40);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 0xc, ZERO);
    wf(m, DRAW_COLOUR + 4, ZERO);
    wf(m, DRAW_COLOUR + 8, ZERO);
    for (o, x, y) in [(0x8fcu32, 140.0f32, 0x434d_0000u32), (0x974, 140.0, 0x43a2_8000), (0x938, 490.0, 0x434d_0000), (0x9b0, 490.0, 0x43a2_8000)] {
        fcall(m, env, DRAW_CENTRED, &[menu, this + o], &[F(x), k(y), ONE]);
    }
}

// --- G9w4: the front end's loads ---

pub const FE1_VTABLE: u32 = 0x3a51b8;
pub const FE2_VTABLE: u32 = 0x3a5158;
pub const FORMAT_RACE_TIME: u32 = 0x243b88;

/// The screens' common start: the base's, the table, an animation, 28 sprite records unset and full size, the
/// fade's colours red (and the records' first words −1 again)
fn screen_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, vtable: u32, anim: u32, recs: u32) {
    init(m, env, this);
    m.w32(this, vtable);
    call(m, env, ANIM_INIT, &[this + anim]);
    for i in 0..28 {
        let r = this + recs + 0x1c * i;
        m.w32(r, 0xffff_ffff);
        wf(m, r + 4, ONE);
        wf(m, r + 8, ONE);
        m.w32(r + 0xc, 0);
        m.w32(r + 0x10, 0);
        wf(m, r + 0x14, ONE);
        wf(m, r + 0x18, ONE);
    }
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    for i in (0..28).rev() {
        m.w32(this + recs + 0x1c * i, 0xffff_ffff);
    }
}

/// The screens' common end: the table, the sound bank unloaded, the animation deleted, the base's
fn screen_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32, vtable: u32, anim: u32) {
    m.w32(this, vtable);
    call(m, env, BANK_UNLOAD, &[m.r32(SPEECH)]);
    call(m, env, ANIM_DELETE, &[this + anim, 2]);
    delete(m, env, this, flags);
}

/// `c1PFELoad::c1PFELoad` 0x249758(this): the animation at +0x40c, the records at +0xc04, the race's summary
pub fn fe1_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    screen_new(m, env, this, FE1_VTABLE, 0x40c, 0xc04);
    fill_race_summary_1p(m, env, this);
    this
}

/// `c1PFELoad::~c1PFELoad` 0x249870(this, flags)
pub fn fe1_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    screen_delete(m, env, this, flags, FE1_VTABLE, 0x40c);
}

/// vf4 0x2498c8(this): the loading loop played (0), then the bars started
pub fn fe1_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    call(m, env, LOOP_PLAY, &[m.r32(SPEECH), 0]);
    start_bars(m, env, this, scratch);
}

/// `c1PFELoad::StartAssetLoad` 0x249900(this): the sound bank, the sprite set and its icons; no pictures (+0xf14 the
/// sprites not yet in)
pub fn fe1_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0xf14, 0);
}

/// `c1PFELoad::PollAssetLoad` 0x249948(this): [`poll_sprites`] with the records at +0xc04
pub fn fe1_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0xc04)
}

/// A float · 100 made whole (`cvt.w.s`), a race time's argument
fn hundredths(v: F) -> u32 {
    crate::sndlib::cvt_w((v * F(100.0)).0) as u32
}

/// `FE_FillRaceSummary1P` 0x24a188(this): the race's lines for the one-player front end load: the mode's heading
/// (+0x6a0: 0xe7d free ride, 0xe7e race, show-off and time challenge, 0xe7f modes 4 and 5, none else) and line
/// (+0x6dc), the course's name (the career's +0x2af8), ten labels (0x121, 0x120, 0x123, 0x125..0x12b), the player's
/// character and level, the trick book's pages, the stats block's +0x48, +0x1c, +0x10, +0x14 and three times
/// (+0x5c, +0x60, +0x64); then the animation's bars
pub fn fill_race_summary_1p(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let profile = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    let rider = call(m, env, FIND_RIDER, &[CAREER, 0]);
    let blk = rider.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3294;
    let mode = m.r32(CAREER + 0x2afc);
    let heading = match mode {
        1 => Some(0xe7d),
        2 | 3 | 7 => Some(0xe7e),
        4 | 5 => Some(0xe7f),
        _ => None,
    };
    if let Some(h) = heading {
        copy_loc(m, env, this + 0x6a0, h);
    }
    let line = match m.r32(CAREER + 0x2afc) {
        2 | 4 => 0xe2c,
        3 | 5 => 0xe2d,
        6 => 0xe2e,
        7 => 0xe72,
        _ => 0xe2a,
    };
    copy_loc(m, env, this + 0x6dc, line);
    let t = m.r32(m.r32(APP) + 0x50);
    let name = call(m, env, COURSE_NAME, &[CAREER, m.r32(CAREER + 0x2af8)]);
    let s = call(m, env, LOC, &[t, name]);
    call(m, env, WSTR_COPY, &[this + 0x718, s]);
    for (o, id) in [(0x754u32, 0x121u32), (0x7cc, 0x123), (0x790, 0x120), (0x808, 0x125), (0x844, 0x126), (0x880, 0x127), (0x8bc, 0x128), (0x8f8, 0x129), (0x934, 0x12a), (0x970, 0x12b)] {
        copy_loc(m, env, this + o, id);
    }
    let ch = m.r32(profile.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    call(m, env, WSTR_SET_ASCII, &[this + 0x9ac, ch]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let lv = call(m, env, LEVEL_NAME, &[CAREER, id]);
    call(m, env, WSTR_COPY, &[this + 0x9e8, lv]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let book = call(m, env, TRICK_BOOK, &[CAREER, id]);
    let pages = call(m, env, COMPLETE_PAGES, &[book]);
    if pages == 6 {
        let f = loc(m, env, 0xc9e);
        call(m, env, WSTR_FORMAT, &[this + 0xa24, f, pages]);
    } else {
        call(m, env, WSTR_FORMAT_ASCII, &[this + 0xa24, FMT_D, pages.wrapping_add(1)]);
    }
    for (o, w) in [(0xa60u32, 0x48u32), (0xa9c, 0x1c), (0xad8, 0x10), (0xb14, 0x14)] {
        call(m, env, WSTR_FORMAT_ASCII, &[this + o, FMT_D, m.r32(blk + w)]);
    }
    for (o, w) in [(0xb50u32, 0x5cu32), (0xb8c, 0x60), (0xbc8, 0x64)] {
        let t = hundredths(rf(m, blk + w));
        call(m, env, FORMAT_RACE_TIME, &[t, this + o]);
    }
    let anim = this + 0x40c;
    let count = |m: &mut Mem, env: &mut dyn TextBoxEnv, at: u32| call(m, env, WSTR_COUNT, &[at]);
    let bar = |m: &mut Mem, env: &mut dyn TextBoxEnv, a: [u32; 6]| {
        call(m, env, ANIM_ADD_BAR, &[anim, a[0], a[1], a[2], a[3], a[4], a[5]]);
    };
    for (line, at) in [(1u32, 0x6a0u32), (2, 0x6dc), (3, 0x718)] {
        let n = count(m, env, this + at);
        bar(m, env, [line, line, 1, 0, 0, n]);
    }
    bar(m, env, [4, 4, 0x46, 3, 0, 1000]);
    for (line, at) in [(5u32, 0x9acu32), (6, 0xa24), (7, 0x9e8), (8, 0xa60), (9, 0xa9c), (0xa, 0xad8), (0xb, 0xb14), (0xc, 0xb50), (0xd, 0xb8c)] {
        let n = count(m, env, this + at);
        bar(m, env, [line, line - 1, 1, 0, 0, n.wrapping_add(8)]);
    }
    let n = count(m, env, this + 0xbc8);
    bar(m, env, [0xe, 0xd, 1, 0, 0, n.wrapping_add(0x3c)]);
}

/// vf5 0x24a930(this): the base's tick, the animation, the loading loop stopped (0) once the app's +8 reaches 8
pub fn fe1_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    tick(m, env, this);
    call(m, env, ANIM_UPDATE, &[this + 0x40c]);
    if (m.r32(m.r32(APP) + 8) as i32) >= 8 {
        call(m, env, LOOP_STOP, &[m.r32(SPEECH), 0]);
    }
}

/// `c1PFELoad::DrawBackground` 0x24a988(this): once the sprites are in, the bars, the ticker, the course's sprite
/// (+0xdc4) in white at (28, 235) by 1.15, the flakes
pub fn fe1_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0xf14) == 0 {
        return;
    }
    let recs = this + 0xc04;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    rcall(m, env, 0x1d0, &[7], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let s = k(0x3f93_3333);
    fcall(m, env, SPRITE_RECT, &[this + 0xdc4, 0x40], &[F(28.0), k(0x436b_0000), s, s]);
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// `c1PFELoad::DrawOverlay` 0x24aa70(this): on texture 12 the heading typed in in gold (the title font at 1.2), the
/// ten labels right-aligned at 460 in black and their values typed in at 475 with the cursor on the animation's line
pub fn fe1_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let (title, menu) = (this + 0x48, this + 0xc);
    let (a, b, col) = (scratch, scratch + 0x10, scratch + 0x20);
    let anim = this + 0x40c;
    rcall(m, env, 0x1d0, &[0xc], &[]);
    call(m, env, TS_INIT, &[a]);
    call(m, env, TS_CONSTRUCT, &[b]);
    call(m, env, TS_SET_FONT, &[a, title]);
    m.w32(a + 4, 1);
    call(m, env, TS_SET_FONT, &[b, menu]);
    put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    m.w32(b + 4, 1);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
    let n = call(m, env, ANIM_VALUE, &[anim, 1]);
    fcall(m, env, DRAW_ALIGNED, &[a, 0, this + 0x6a0, n, 0, 1], &[k(0x3f99_999a), F(30.0), F(30.0)]);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 0xc, ZERO);
    wf(m, DRAW_COLOUR + 4, ZERO);
    wf(m, DRAW_COLOUR + 8, ZERO);
    let (lx, vx, sc) = (F(460.0), k(0x43ed_8000), k(0x3f94_7ae1));
    let ys = [0x42c4_0000u32, 0x42f8_0000, 0x4316_0000, 0x433e_0000, 0x4358_0000, 0x4372_0000, 0x4386_0000, 0x439a_0000, 0x43a7_0000, 0x43b4_0000];
    for (i, o) in [0x754u32, 0x7cc, 0x790, 0x808, 0x844, 0x880, 0x8bc, 0x8f8, 0x934, 0x970].into_iter().enumerate() {
        fcall(m, env, DRAW_RIGHT, &[menu, this + o], &[lx, k(ys[i]), sc]);
    }
    for (i, o) in [0x9acu32, 0xa24, 0x9e8, 0xa60, 0xa9c, 0xad8, 0xb14, 0xb50, 0xb8c, 0xbc8].into_iter().enumerate() {
        let line = 5 + i as u32;
        let n = call(m, env, ANIM_VALUE, &[anim, line]);
        let step = m.r32(anim);
        fcall(m, env, DRAW_CURSOR, &[b, 0, this + o, n, 0, (step == line - 1) as u32], &[sc, vx, k(ys[i])]);
    }
    call(m, env, TS_DESTRUCT_THUNK, &[b, 2]);
    call(m, env, TS_DESTRUCT, &[a, 2]);
}
pub const FIND_RIDER: u32 = 0x166598;
pub const SHADOWED_CENTRED: u32 = 0x19d770;

/// `c2PFELoad::c2PFELoad` 0x24b028(this): the animation at +0x40c, the records at +0xe5c, the two players' summary
pub fn fe2_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    screen_new(m, env, this, FE2_VTABLE, 0x40c, 0xe5c);
    fill_race_summary_2p(m, env, this);
    this
}

/// `c2PFELoad::~c2PFELoad` 0x24b140(this, flags)
pub fn fe2_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    screen_delete(m, env, this, flags, FE2_VTABLE, 0x40c);
}

/// `c2PFELoad::StartAssetLoad` 0x24b198(this): the sound bank, the sprite set and its icons, not yet in (+0x116c);
/// two pictures (+0x1170): each player's rider ("ldr1", "ldr2"); then their load started
pub fn fe2_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let list = this + 0x1170;
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0x116c, 0);
    for i in 0..2u32 {
        let e = list + 0x34 * i;
        m.w32(e, 0);
        m.w8(e + 4, 0);
        m.w8(e + 0x29, 0);
        m.w32(e + 0x30, 0xffff_ffff);
        let id = call(m, env, PLAYER_ID, &[CAREER, i]);
        let p = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
        rider_pic_name(m, env, p, e + 4);
        put_shape_name(m, e, if i == 0 { SHAPE_R1 } else { SHAPE_R2 });
    }
    load_pics(m, env, this, list, 2);
}

/// `c2PFELoad::PollAssetLoad` 0x24b320(this): [`poll_sprites`] with the records at +0xe5c
pub fn fe2_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0xe5c)
}

/// vf4 0x24bb60 (the one-player front end load's)
pub fn fe2_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    fe1_vf4(m, env, this, scratch);
}

/// vf3 0x24bb98: 1 once the fade is in, else the archive closed and 0
pub fn fe2_vf3(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) -> u32 {
    track1_vf3(m, env, this, scratch)
}

/// A player's stats lines from `at`: the block's +0x48, +0x1c, +0x10, +0x14 and its three times
fn stats_lines(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, blk: u32, at: u32) {
    for (k, w) in [0x48u32, 0x1c, 0x10, 0x14].into_iter().enumerate() {
        call(m, env, WSTR_FORMAT_ASCII, &[this + at + 0x3c * k as u32, FMT_D, m.r32(blk + w)]);
    }
    for (k, w) in [0x5cu32, 0x60, 0x64].into_iter().enumerate() {
        let t = hundredths(rf(m, blk + w));
        call(m, env, FORMAT_RACE_TIME, &[t, this + at + 0xf0 + 0x3c * k as u32]);
    }
}

/// A player's trick book's pages at `at`: "%d" of the next, or 0xc9e once all 6 are done
fn pages_line(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, player: u32, at: u32) {
    let id = call(m, env, PLAYER_ID, &[CAREER, player]);
    let book = call(m, env, TRICK_BOOK, &[CAREER, id]);
    let pages = call(m, env, COMPLETE_PAGES, &[book]);
    if pages == 6 {
        let f = loc(m, env, 0xc9e);
        call(m, env, WSTR_FORMAT, &[this + at, f, pages]);
    } else {
        call(m, env, WSTR_FORMAT_ASCII, &[this + at, FMT_D, pages.wrapping_add(1)]);
    }
}

/// `FE_FillRaceSummary2P` 0x24bbd0(this): the two players' lines: the heading (+0x6a0: 0xe7d free ride, 0xe7e race,
/// show-off and time challenge, none else), the line (+0x6dc; mode 7's is 0xe2c here), the course, the labels, then
/// each player's character, level, pages and stats (the first's from +0x9ac, the second's from +0xc04). No bars
pub fn fill_race_summary_2p(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let p0 = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 1]);
    let p1 = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    let r = call(m, env, FIND_RIDER, &[CAREER, 0]);
    let b0 = r.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3294;
    let r = call(m, env, FIND_RIDER, &[CAREER, 1]);
    let b1 = r.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3294;
    let mode = m.r32(CAREER + 0x2afc) as i32;
    let heading = match mode {
        1 => Some(0xe7d),
        2 | 3 | 7 => Some(0xe7e),
        _ => None,
    };
    if let Some(h) = heading {
        copy_loc(m, env, this + 0x6a0, h);
    }
    let line = match m.r32(CAREER + 0x2afc) {
        2 | 4 | 7 => 0xe2c,
        3 | 5 => 0xe2d,
        6 => 0xe2e,
        _ => 0xe2a,
    };
    copy_loc(m, env, this + 0x6dc, line);
    let t = m.r32(m.r32(APP) + 0x50);
    let name = call(m, env, COURSE_NAME, &[CAREER, m.r32(CAREER + 0x2af8)]);
    let s = call(m, env, LOC, &[t, name]);
    call(m, env, WSTR_COPY, &[this + 0x718, s]);
    for (o, id) in [(0x754u32, 0x121u32), (0x790, 0x120), (0x7cc, 0x123), (0x808, 0x125), (0x844, 0x126), (0x880, 0x127), (0x8bc, 0x128), (0x8f8, 0x129), (0x934, 0x12a), (0x970, 0x12b)] {
        copy_loc(m, env, this + o, id);
    }
    let ch = m.r32(p0.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    call(m, env, WSTR_SET_ASCII, &[this + 0x9ac, ch]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let lv = call(m, env, LEVEL_NAME, &[CAREER, id]);
    call(m, env, WSTR_COPY, &[this + 0x9e8, lv]);
    pages_line(m, env, this, 0, 0xa24);
    stats_lines(m, env, this, b0, 0xa60);
    let ch = m.r32(p1.wrapping_mul(0x38).wrapping_add(CHARACTERS) + 4);
    call(m, env, WSTR_SET_ASCII, &[this + 0xc04, ch]);
    let id = call(m, env, PLAYER_ID, &[CAREER, 1]);
    let lv = call(m, env, LEVEL_NAME, &[CAREER, id]);
    call(m, env, WSTR_COPY, &[this + 0xc40, lv]);
    pages_line(m, env, this, 1, 0xc7c);
    stats_lines(m, env, this, b1, 0xcb8);
}

/// vf5 0x24c278 (the one-player front end load's)
pub fn fe2_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    fe1_vf5(m, env, this);
}

/// `c2PFELoad::DrawBackground` 0x24c2d0(this): once the sprites are in, the bars, the ticker, in white the players'
/// pictures in tip boxes (at 23 and 360), the flakes
pub fn fe2_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0x116c) == 0 {
        return;
    }
    let recs = this + 0xe5c;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    for i in 0..2u32 {
        let e = this + 0x1170 + 0x34 * i;
        if m.r32(e) != 1 {
            continue;
        }
        let x = if i == 0 { F(23.0) } else { F(360.0) };
        draw_tip_box(m, env, this, m.r32(e + 0x30), 0, 7, x, F(100.0), ONE, scratch);
    }
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// `c2PFELoad::DrawOverlay` 0x24c428(this): on texture 12 the heading centred in gold (the title font, shadowed,
/// 1.2), then in black the six labels centred at 320, the first player's values right-aligned at 210 and the
/// second's at 430 (the menu font at 1.16), rows at 202, 228, 268, 294, 320, 346
pub fn fe2_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let menu = this + 0xc;
    rcall(m, env, 0x1d0, &[0xc], &[]);
    put4(m, scratch, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let cx = F(320.0);
    fcall(m, env, SHADOWED_CENTRED, &[this + 0x48, this + 0x6a0], &[k(0x3f99_999a), cx, F(30.0)]);
    wf(m, DRAW_COLOUR, ONE);
    wf(m, DRAW_COLOUR + 0xc, ZERO);
    wf(m, DRAW_COLOUR + 4, ZERO);
    wf(m, DRAW_COLOUR + 8, ZERO);
    let sc = k(0x3f94_7ae1);
    let ys = [0x434a_0000u32, 0x4364_0000, 0x4386_0000, 0x4393_0000, 0x43a0_0000, 0x43ad_0000];
    for (i, o) in [0x790u32, 0x7cc, 0x808, 0x844, 0x880, 0x8bc].into_iter().enumerate() {
        fcall(m, env, DRAW_CENTRED, &[menu, this + o], &[cx, k(ys[i]), sc]);
    }
    for (i, o) in [0x9e8u32, 0xa24, 0xa60, 0xa9c, 0xad8, 0xb14].into_iter().enumerate() {
        fcall(m, env, DRAW_RIGHT, &[menu, this + o], &[k(0x4352_0000), k(ys[i]), sc]);
    }
    for (i, o) in [0xc40u32, 0xc7c, 0xcb8, 0xcf4, 0xd30, 0xd6c].into_iter().enumerate() {
        fcall(m, env, DRAW_STRING, &[menu, this + o], &[k(0x43d7_0000), k(ys[i]), sc]);
    }
}

/// The two-player track load's stores (from `this`)
const TRACK2_TABLE: [Store; 197] = [
    Sheet(0x09ec, 0x15e8), Sheet(0x0a08, 0x15e8), Sheet(0x0a24, 0x15e8), Int(0x040c, 1), K(0x0a04, 0x3ee30000),
    K(0x09f0, 0x43320000), K(0x09f4, 0x42e40000), K(0x0a14, 0x3f418000), K(0x0a18, 0x3e850000),
    K(0x0a1c, 0x3f608000), K(0x0a0c, 0x431f0000), K(0x0a10, 0x427c0000), K(0x0a30, 0x3cb00000),
    K(0x0a3c, 0x3e0e0000), K(0x0a34, 0x3f5c8000), K(0x0a38, 0x3f7c8000), Sheet(0x0a40, 0x15e8),
    K(0x0a4c, 0x3f158000), K(0x0a54, 0x3e810000), K(0x0a44, 0x42820000), K(0x0a00, 0x3f318000),
    K(0x0a28, 0x42040000), K(0x0a2c, 0x41f80000), K(0x09f8, 0x3b000000), K(0x09fc, 0x3b000000),
    K(0x0a20, 0x3f7f8000), K(0x0a58, 0x3f7f8000), K(0x0a50, 0x3b000000), K(0x0a48, 0x42d60000),
    Sheet(0x0a5c, 0x15e8), Sheet(0x0a78, 0x15e8), Sheet(0x0a94, 0x15ec), Sheet(0x0ab0, 0x15ec),
    K(0x0a74, 0x3f2d8000), K(0x0a6c, 0x3ef50000), K(0x0a70, 0x3f0c8000), K(0x0a60, 0x41980000),
    K(0x0a64, 0x41a00000), K(0x0a90, 0x3f318000), K(0x0a88, 0x3e9b0000), K(0x0a8c, 0x3ec70000),
    K(0x0a7c, 0x41b80000), K(0x0aac, 0x3f3b8000), K(0x0aa4, 0x3cd00000), K(0x0aa8, 0x3e320000),
    K(0x0a9c, 0x41d80000), K(0x0abc, 0x3f1e8000), K(0x0ac0, 0x3e620000), K(0x0ac4, 0x3ead0000),
    K(0x0ab4, 0x41f80000), K(0x0a68, 0x3f1a8000), K(0x0a84, 0x3f198000), K(0x0a80, 0x41c80000),
    K(0x0aa0, 0x3f218000), K(0x0a98, 0x421c0000), K(0x0ab8, 0x41e00000), K(0x0ac8, 0x3f398000),
    Sheet(0x0acc, 0x15ec), Sheet(0x0ae8, 0x15ec), Sheet(0x0b04, 0x15ec), Sheet(0x0b20, 0x15ec),
    K(0x0ad8, 0x3f218000), K(0x0ae4, 0x3f3c8000), K(0x0adc, 0x3ec50000), K(0x0ae0, 0x3f0a8000),
    K(0x0ad0, 0x42240000), K(0x0ad4, 0x41e00000), K(0x0b00, 0x3f3a8000), K(0x0af8, 0x3f1a8000),
    K(0x0af0, 0x41c80000), K(0x0b10, 0x3f198000), K(0x0b1c, 0x3f458000), K(0x0b08, 0x42040000),
    K(0x0b0c, 0x42340000), K(0x0b2c, 0x3f408000), K(0x0b38, 0x3f668000), K(0x0b30, 0x3d380000),
    K(0x0b34, 0x3e120000), K(0x0b24, 0x41d00000), Sheet(0x0b3c, 0x15ec), K(0x0b48, 0x3f428000),
    K(0x0afc, 0x3f458000), K(0x0b18, 0x3f718000), K(0x0b28, 0x421c0000), K(0x0af4, 0x3f228000),
    K(0x0aec, 0x42300000), K(0x0b14, 0x3f518000), K(0x0b54, 0x3f718000), Sheet(0x0b58, 0x15ec),
    Sheet(0x0b74, 0x15ec), Sheet(0x0b90, 0x15ec), K(0x0b4c, 0x3e5a0000), K(0x0b50, 0x3eb90000),
    K(0x0b40, 0x421c0000), K(0x0b64, 0x3f438000), K(0x0b70, 0x3f748000), K(0x0b68, 0x3ed50000),
    K(0x0b6c, 0x3f118000), K(0x0b5c, 0x42200000), K(0x0b60, 0x42480000), K(0x0b80, 0x3f4b8000),
    K(0x0b8c, 0x3f7b8000), K(0x0b78, 0x423c0000), K(0x0b7c, 0x42440000), K(0x0b9c, 0x3f4d8000),
    K(0x0ba4, 0x3f728000), K(0x0b94, 0x42080000), Sheet(0x0bac, 0x15ec), K(0x0bb8, 0x3c200000),
    K(0x0bc4, 0x3f1b8000), K(0x0bbc, 0x3bc00000), K(0x0ba0, 0x3f518000), K(0x0b98, 0x42300000),
    K(0x0b44, 0x42400000), K(0x0b84, 0x3f188000), K(0x0b88, 0x3f468000), K(0x0ba8, 0x3f788000),
    K(0x0bc0, 0x3f804000), Sheet(0x0bc8, 0x15e8), Sheet(0x0be4, 0x15e8), Sheet(0x0c00, 0x15e8),
    K(0x0bb4, 0x431a0000), Sheet(0x0c1c, 0x15f0), K(0x0c20, 0x43800000), K(0x0bb0, 0x43800000),
    K(0x0c2c, 0x3b000000), K(0x0bd4, 0x3f188000), K(0x0be0, 0x3f228000), K(0x0bd8, 0x3f6f8000),
    K(0x0bdc, 0x3f778000), K(0x0bcc, 0x41100000), K(0x0bd0, 0x41300000), K(0x0bf0, 0x3f188000),
    K(0x0bfc, 0x3f228000), K(0x0bf4, 0x3f788000), K(0x0bf8, 0x3f788000), K(0x0be8, 0x3f800000),
    K(0x0bec, 0x41300000), K(0x0c0c, 0x3f188000), K(0x0c18, 0x3f228000), K(0x0c10, 0x3f798000),
    K(0x0c14, 0x3f7f8000), K(0x0c04, 0x40e00000), K(0x0c08, 0x41300000), K(0x0c28, 0x3b000000),
    K(0x0c34, 0x3f398000), K(0x0c30, 0x3f7f8000), K(0x0c24, 0x433a0000), Sheet(0x0c38, 0x15e8),
    Sheet(0x0c54, 0x15e8), Sheet(0x0c70, 0x15e8), Sheet(0x0c8c, 0x15e8), K(0x0c44, 0x3f088000),
    K(0x0c50, 0x3f378000), K(0x0c48, 0x3f138000), K(0x0c4c, 0x3f388000), K(0x0c60, 0x3f0e8000),
    K(0x0c6c, 0x3f338000), K(0x0c64, 0x3f398000), K(0x0c68, 0x3f688000), K(0x0c58, 0x42400000),
    K(0x0c5c, 0x42180000), K(0x0c80, 0x3f618000), K(0x0c98, 0x3ee50000), K(0x0ca4, 0x3f098000),
    K(0x0c9c, 0x3f468000), K(0x0ca0, 0x3f5d8000), K(0x0c94, 0x41c00000), K(0x0c3c, 0x42180000),
    K(0x0c40, 0x42400000), K(0x0c7c, 0x3ee50000), K(0x0c88, 0x3f098000), K(0x0c84, 0x3f788000),
    K(0x0c74, 0x41c00000), K(0x0c78, 0x41c00000), K(0x0c90, 0x41c00000), Sheet(0x0ca8, 0x15e8),
    K(0x0cb8, 0x3f778000), Sheet(0x0cc4, 0x15e8), K(0x0cbc, 0x3f6f8000), K(0x0cac, 0x41100000),
    K(0x0cd8, 0x3f788000), K(0x0cc8, 0x3f800000), Sheet(0x0ce0, 0x15e8), K(0x0cec, 0x3f188000),
    K(0x0cf8, 0x3f228000), K(0x0cf0, 0x3f7f8000), K(0x0cf4, 0x3f798000), K(0x0ce4, 0x40e00000),
    K(0x0ce8, 0x41300000), K(0x0cb4, 0x3f188000), K(0x0cc0, 0x3f228000), K(0x0cb0, 0x41300000),
    K(0x0cd0, 0x3f188000), K(0x0cdc, 0x3f228000), K(0x0cd4, 0x3f788000), K(0x0ccc, 0x41300000),
];

// --- G9w5: the load from quit, the trick tutorial's load and the rider ranking's ---

pub const QUIT_VTABLE: u32 = 0x3a50f8;
pub const TUTORIAL_VTABLE: u32 = 0x3a5098;
pub const RANKING_VTABLE: u32 = 0x3a5038;
pub const RAND: u32 = 0x23d4a8;
pub const LEVEL_STRING_ID: u32 = 0x164730;
/// The career's unlocks (12 bytes each: +4 the kind, +8 an index; the count before them) and the profiles' items
/// (0xf0 per profile, 0x14 each: +0 a name, +8 a kind)
pub const UNLOCKS_COUNT: u32 = 0x3353f8;
pub const UNLOCKS: u32 = 0x335400;
pub const PROFILE_ITEMS: u32 = 0x332148;
/// Four colour records the loading screens' static start fills
pub const STATIC_COLOURS: u32 = 0x3d2c88;
pub const SET_VEC4: u32 = 0x14f5f0;
pub use crate::resultpages::{SET_RECT, TB_APPEND, TB_DELETE, TB_DRAW, TB_INIT, TB_RESET, TB_SET_FONT, TB_SET_SCALE, TB_WRAP};

/// `cFELoadFromQuit::cFELoadFromQuit` 0x24c748(this, from_menu): the animation at +0x40c, a text box at +0x6dc, the
/// records at +0x1b50, the flag kept (+0x1e64), the tips set up
pub fn quit_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, from_menu: u32) -> u32 {
    init(m, env, this);
    m.w32(this, QUIT_VTABLE);
    call(m, env, ANIM_INIT, &[this + 0x40c]);
    call(m, env, TB_INIT, &[this + 0x6dc]);
    for i in 0..28 {
        let r = this + 0x1b50 + 0x1c * i;
        m.w32(r, 0xffff_ffff);
        wf(m, r + 4, ONE);
        wf(m, r + 8, ONE);
        m.w32(r + 0xc, 0);
        m.w32(r + 0x10, 0);
        wf(m, r + 0x14, ONE);
        wf(m, r + 0x18, ONE);
    }
    put4(m, this + 0x94, [ONE, ZERO, ZERO, ZERO]);
    put4(m, this + 0xa4, [ONE, ZERO, ZERO, ZERO]);
    m.w32(this + 0x1e64, from_menu);
    for i in (0..28).rev() {
        m.w32(this + 0x1b50 + 0x1c * i, 0xffff_ffff);
    }
    setup_tips(m, env, this);
    this
}

/// `cFELoadFromQuit::~cFELoadFromQuit` 0x24c878(this, flags): its text box deleted too
pub fn quit_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this, QUIT_VTABLE);
    call(m, env, BANK_UNLOAD, &[m.r32(SPEECH)]);
    call(m, env, TB_DELETE, &[this + 0x6dc, 2]);
    call(m, env, ANIM_DELETE, &[this + 0x40c, 2]);
    delete(m, env, this, flags);
}

/// vf9 0x24c8e0(this): the sound bank, the sprite set and its icons, not yet in (+0x1e60)
pub fn quit_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0x1e60, 0);
}

/// vf10 0x24c928(this): [`poll_sprites`] with the records at +0x1b50
pub fn quit_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0x1b50)
}

/// vf4 0x24d168 (the front end loads')
pub fn quit_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    fe1_vf4(m, env, this, scratch);
}

fn rand_tip(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    let t = call(m, env, RAND, &[]) % 19;
    m.w32(this + 0x1b4c, t);
    t
}

/// `LoadScreen_SetupTips` 0x24d1a0(this): the heading 0xe30; a tip (+0x1b4c) of the 19 at random, never 17: when
/// the screen comes from the menu (+0x1e64), or the player's level string is not 31 or 32, any; else one of 3, 4,
/// 6 and 15. Then the animation's bars: the heading's and the tip's title (0xcff + 2·tip) and text (0xd00 + 2·tip)
pub fn setup_tips(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    copy_loc(m, env, this + 0x6a0, 0xe30);
    if m.r32(this + 0x1e64) != 0 {
        while rand_tip(m, env, this) == 17 {}
    } else {
        let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
        let lv = call(m, env, LEVEL_STRING_ID, &[CAREER, id]);
        if lv.wrapping_sub(0x1f) < 2 {
            loop {
                let t = rand_tip(m, env, this);
                if t.wrapping_sub(3) < 2 || t == 6 || t == 15 {
                    break;
                }
            }
        } else {
            while rand_tip(m, env, this) == 17 {}
        }
    }
    let anim = this + 0x40c;
    let n = call(m, env, WSTR_COUNT, &[this + 0x6a0]);
    call(m, env, ANIM_ADD_BAR, &[anim, 1, 1, 2, 0, 0, n]);
    for (line, base) in [(2u32, 0xcffu32), (3, 0xd00)] {
        let tip = m.r32(this + 0x1b4c);
        let s = loc(m, env, (tip << 1).wrapping_add(base));
        let n = call(m, env, WSTR_COUNT, &[s]);
        call(m, env, ANIM_ADD_BAR, &[anim, line, line, 1, 0, 0, n]);
    }
}

/// vf5 0x24d390 (the front end loads')
pub fn quit_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    fe1_vf5(m, env, this);
}

/// vf7 0x24d3e8(this): once the sprites are in, the bars, the ticker, the course's sprite (+0x1d10), the flakes
pub fn quit_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0x1e60) == 0 {
        return;
    }
    let recs = this + 0x1b50;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    rcall(m, env, 0x1d0, &[7], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let s = k(0x3f93_3333);
    fcall(m, env, SPRITE_RECT, &[this + 0x1d10, 0x40], &[F(28.0), k(0x436b_0000), s, s]);
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// vf8 0x24d4d0(this): two text styles (the title font; the menu font, its +4 set unless the animation's +0xc is
/// 1); at step 4 the heading shadowed in gold, the tip's title in black right of the heading (or ending at 620 if
/// it fits), its text in the screen's text box (205, 280 wide, 320 high, at 1.07)
pub fn quit_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let (title, menu) = (this + 0x48, this + 0xc);
    let (a, b, col) = (scratch, scratch + 0x10, scratch + 0x20);
    rcall(m, env, 0x1d0, &[0xc], &[]);
    call(m, env, TS_INIT, &[a]);
    call(m, env, TS_CONSTRUCT, &[b]);
    call(m, env, TS_SET_FONT, &[a, title]);
    m.w32(a + 4, 1);
    call(m, env, TS_SET_FONT, &[b, menu]);
    if m.r32(this + 0x418) != 1 {
        m.w32(b + 4, 1);
    }
    if m.r32(this + 0xb4) == 4 {
        put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let s12 = k(0x3f99_999a);
        fcall(m, env, PRINTF_SHADOWED_A, &[title, this + 0x6a0], &[s12, F(30.0), F(27.0)]);
        let half = F(320.0);
        let tip = m.r32(this + 0x1b4c);
        let s = loc(m, env, (tip << 1).wrapping_add(0xcff));
        let w = fcall(m, env, MEASURE_WIDE, &[menu, s, 0], &[s12]);
        let room = F(620.0) - crate::animseq::cvt_s_w(w as i32);
        let x = if room < half { room } else { half };
        put4(m, col, [ONE, ZERO, ZERO, ZERO]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let tip = m.r32(this + 0x1b4c);
        let s = loc(m, env, (tip << 1).wrapping_add(0xcff));
        fcall(m, env, DRAW_STRING, &[menu, s], &[x, k(0x42fe_0000), s12]);
        let tb = this + 0x6dc;
        call(m, env, TB_SET_FONT, &[tb, menu]);
        call(m, env, TB_RESET, &[tb]);
        put4(m, col, [ONE, ZERO, ZERO, ZERO]);
        for i in 0..4 {
            m.w32(tb + 0x3c + 4 * i, m.r32(col + 4 * i));
        }
        m.w32(tb + 0x6c, 0);
        let s107 = k(0x3f88_f5c3);
        fcall(m, env, TB_SET_SCALE, &[tb], &[s107, s107]);
        fcall(m, env, SET_RECT, &[tb], &[half, k(0x434d_0000), k(0x438c_0000), half]);
        let tip = m.r32(this + 0x1b4c);
        let s = loc(m, env, (tip << 1).wrapping_add(0xd00));
        call(m, env, TB_APPEND, &[tb, s]);
        call(m, env, TB_WRAP, &[tb, 0]);
        m.w32(tb + 0x6c, 0);
        put4(m, col, [ONE, ZERO, ZERO, ZERO]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        call(m, env, TB_DRAW, &[tb, 0xffff_ffff, 0]);
    }
    call(m, env, TS_DESTRUCT_THUNK, &[b, 2]);
    call(m, env, TS_DESTRUCT, &[a, 2]);
}

/// `cTrickTutorialLoad::cTrickTutorialLoad` 0x24d7f8(this): the animation at +0x40c, the records at +0x6dc, the
/// title set
pub fn tutorial_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    screen_new(m, env, this, TUTORIAL_VTABLE, 0x40c, 0x6dc);
    tutorial_set_title(m, env, this);
    this
}

/// `cTrickTutorialLoad::~cTrickTutorialLoad` 0x24d910(this, flags)
pub fn tutorial_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    screen_delete(m, env, this, flags, TUTORIAL_VTABLE, 0x40c);
}

/// `cTrickTutorialLoad::StartAssetLoad` 0x24d968(this): the sound bank, the sprite set and its icons, not yet in
/// (+0x9ec)
pub fn tutorial_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0x9ec, 0);
}

/// `cTrickTutorialLoad::PollAssetLoad` 0x24d9b0(this): [`poll_sprites`] with the records at +0x6dc
pub fn tutorial_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0x6dc)
}

/// vf4 0x24e1f0 (the track loads')
pub fn tutorial_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    track1_vf4(m, env, this, scratch);
}

/// `TrickTutorialLoad_SetTitle` 0x24e228(this): the heading 0xe80
pub fn tutorial_set_title(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    copy_loc(m, env, this + 0x6a0, 0xe80);
}

/// vf5 0x24e268(this): the base's tick, the animation, the loading loop stopped (1) once the app's +8 reaches 8
pub fn tutorial_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    tick(m, env, this);
    call(m, env, ANIM_UPDATE, &[this + 0x40c]);
    if (m.r32(m.r32(APP) + 8) as i32) >= 8 {
        call(m, env, LOOP_STOP, &[m.r32(SPEECH), 1]);
    }
}

/// `cTrickTutorialLoad::DrawBackground` 0x24e2c0(this): once the sprites are in, the bars, the ticker, the pad
/// (+0x90c) in white at the centre by 1.1, the flakes
pub fn tutorial_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0x9ec) == 0 {
        return;
    }
    let recs = this + 0x6dc;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    rcall(m, env, 0x1d0, &[7], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let s = k(0x3f8c_cccd);
    fcall(m, env, SPRITE_RECT, &[this + 0x90c, 0x60], &[F(320.0), F(240.0), s, s]);
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// `cTrickTutorialLoad::DrawOverlay` 0x24e3a8(this): on texture 12, once the sprites are in: the heading centred
/// in gold (shadowed, 1.4), then in black the pad's labels for the career's controller (+0x168c: 0 the labels
/// 0xc7c..0xc80 and 0xc82, else 0xc84..0xc89, 0xc81 and 0xc83), the two pads (+0x928, +0x944) and their captions
pub fn tutorial_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    rcall(m, env, 0x1d0, &[0xc], &[]);
    if m.r32(this + 0x9ec) == 0 {
        return;
    }
    let menu = this + 0xc;
    put4(m, scratch, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let cx = F(320.0);
    fcall(m, env, SHADOWED_CENTRED, &[this + 0x48, this + 0x6a0], &[k(0x3fb3_3333), cx, F(25.0)]);
    put4(m, scratch, [ONE, ZERO, ZERO, ZERO]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let pad = m.r32(CAREER + 0x168c);
    let s = loc(m, env, if pad == 0 { 0xd28 } else { 0xd29 });
    fcall(m, env, DRAW_CENTRED, &[menu, s], &[cx, k(0x43bb_0000), ONE]);
    let (lx, lx2, small) = (F(160.0), F(130.0), k(0x3f59_999a));
    let right = |m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32, x: F, y: u32, sc: F| {
        let s = loc(m, env, id);
        fcall(m, env, DRAW_RIGHT, &[menu, s], &[x, k(y), sc]);
    };
    let text = |m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32, x: F, y: u32, sc: F| {
        let s = loc(m, env, id);
        fcall(m, env, DRAW_STRING, &[menu, s], &[x, k(y), sc]);
    };
    if m.r32(CAREER + 0x168c) == 0 {
        right(m, env, 0xc7c, lx, 0x4319_0000, ONE);
        right(m, env, 0xc7d, lx, 0x4327_0000, ONE);
        right(m, env, 0xc7e, lx, 0x4348_0000, ONE);
        right(m, env, 0xc7f, lx, 0x4356_0000, ONE);
        right(m, env, 0xc80, lx2, 0x4375_0000, small);
        right(m, env, 0xc82, lx2, 0x4393_8000, ONE);
        text(m, env, 0xde2, F(470.0), 0x431f_0000, ONE);
    } else {
        right(m, env, 0xc84, lx, 0x4319_0000, ONE);
        right(m, env, 0xc85, lx, 0x4327_0000, ONE);
        right(m, env, 0xc86, lx, 0x4348_0000, ONE);
        right(m, env, 0xc87, lx, 0x4356_0000, ONE);
        right(m, env, 0xc88, lx2, 0x4375_0000, small);
        right(m, env, 0xc81, lx2, 0x4381_8000, small);
        right(m, env, 0xc89, lx2, 0x4393_8000, ONE);
        right(m, env, 0xc83, lx2, 0x439a_8000, ONE);
        text(m, env, 0xc8e, F(470.0), 0x431f_0000, ONE);
    }
    let ps = k(0x3f38_51ec);
    fcall(m, env, SPRITE_RECT, &[this + 0x928, 0], &[F(140.0), F(240.0), ps, ps]);
    fcall(m, env, SPRITE_RECT, &[this + 0x944, 0], &[F(140.0), F(290.0), ps, ps]);
    text(m, env, 0xc8d, F(190.0), 0x42fa_0000, ONE);
    text(m, env, 0xc8d, F(360.0), 0x42fa_0000, ONE);
    right(m, env, 0xc8b, F(310.0), 0x43a0_0000, ONE);
    text(m, env, 0xc8c, F(330.0), 0x43a0_0000, ONE);
    text(m, env, 0xc90, F(470.0), 0x4364_0000, ONE);
    text(m, env, 0xc91, F(470.0), 0x437d_0000, small);
    text(m, env, 0xc8a, F(470.0), 0x4391_0000, ONE);
}

/// `cRiderRankingLoad::cRiderRankingLoad` 0x24ea20(this): the animation at +0x40c, the records at +0xa60, the
/// career's summary
pub fn ranking_new(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    screen_new(m, env, this, RANKING_VTABLE, 0x40c, 0xa60);
    fill_career_summary(m, env, this);
    this
}

/// `cRiderRankingLoad::~cRiderRankingLoad` 0x24eb38(this, flags)
pub fn ranking_delete(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    screen_delete(m, env, this, flags, RANKING_VTABLE, 0x40c);
}

/// `cRiderRankingLoad::StartAssetLoad` 0x24eb90(this): the sound bank, the sprite set and its icons, not yet in
/// (+0xd70); one picture (+0xd74): the player's rider ("ldr1"); its load started
pub fn ranking_start(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let e = this + 0xd74;
    call(m, env, BANK_LOAD, &[m.r32(SPEECH)]);
    call(m, env, SPRITES_INIT, &[m.r32(this + 4)]);
    call(m, env, LOAD_ICONS, &[m.r32(this + 4), 0x2cc, 1]);
    m.w32(this + 0xd70, 0);
    m.w32(e, 0);
    m.w8(e + 4, 0);
    m.w8(e + 0x29, 0);
    m.w32(e + 0x30, 0xffff_ffff);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let p = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    rider_pic_name(m, env, p, e + 4);
    put_shape_name(m, e, SHAPE_R1);
    load_pics(m, env, this, e, 1);
}

/// `cRiderRankingLoad::PollAssetLoad` 0x24eca8(this): [`poll_sprites`] with the records at +0xa60
pub fn ranking_poll(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) -> u32 {
    poll_sprites(m, env, this, 0xa60)
}

/// vf4 0x24f4e8 (the front end loads')
pub fn ranking_vf4(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    fe1_vf4(m, env, this, scratch);
}

/// `FE_FillCareerSummary` 0x24f520(this): the ranking's lines: 0xd2a, 0xd2b, 0xd2c, the player's level, 0xd2d, the
/// next unlock's item (the first unlock of kind 4: its index into the profile's items, 0 without one) and its kind's
/// line (0x81, 0xb9, 0x79 for kinds 0 to 2); then the animation's bars
pub fn fill_career_summary(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let profile = call(m, env, PLAYER_PROFILE, &[CAREER, id]);
    let mut item = 0u32;
    let n = m.r32(UNLOCKS_COUNT) as i32;
    let mut i = 0;
    while i < n {
        let e = UNLOCKS + 0xc * i as u32;
        if m.r32(e + 4) == 4 {
            item = m.r32(e + 8);
            break;
        }
        i += 1;
    }
    copy_loc(m, env, this + 0x6a0, 0xd2a);
    copy_loc(m, env, this + 0x6c8, 0xd2b);
    copy_loc(m, env, this + 0x8bc, 0xd2c);
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    let lv = call(m, env, LEVEL_NAME, &[CAREER, id]);
    call(m, env, WSTR_COPY, &[this + 0x902, lv]);
    copy_loc(m, env, this + 0x948, 0xd2d);
    let it = profile.wrapping_mul(0xf0).wrapping_add(item.wrapping_mul(0x14)).wrapping_add(PROFILE_ITEMS);
    call(m, env, WSTR_SET_ASCII, &[this + 0x98e, m.r32(it)]);
    copy_loc(m, env, this + 0x9d4, 0xd2e);
    match m.r32(it + 8) {
        0 => copy_loc(m, env, this + 0xa1a, 0x81),
        1 => copy_loc(m, env, this + 0xa1a, 0xb9),
        2 => copy_loc(m, env, this + 0xa1a, 0x79),
        _ => {}
    }
    let anim = this + 0x40c;
    for (line, at, k2, extra) in [(1u32, 0x6a0u32, 2u32, 0u32), (2, 0x6c8, 1, 0), (3, 0x902, 2, 4), (4, 0x98e, 2, 4), (5, 0xa1a, 2, 4)] {
        let n = call(m, env, WSTR_COUNT, &[this + at]);
        call(m, env, ANIM_ADD_BAR, &[anim, line, line, k2, 0, 0, n.wrapping_add(extra)]);
    }
}

/// vf5 0x24f8c0(this): the base's tick, the animation, the loading loop stopped (0) when the app's +8 is 9
pub fn ranking_vf5(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    tick(m, env, this);
    call(m, env, ANIM_UPDATE, &[this + 0x40c]);
    if m.r32(m.r32(APP) + 8) == 9 {
        call(m, env, LOOP_STOP, &[m.r32(SPEECH), 0]);
    }
}

/// `cRiderRankingLoad::DrawBackground` 0x24f918(this): once the sprites are in, the bars, the ticker, the badge
/// (the first record) in white by 0.9, the rider's picture in a tip box (23, 150) once it is in, the flakes
pub fn ranking_draw_background(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    if m.r32(this + 0xd70) == 0 {
        return;
    }
    let recs = this + 0xa60;
    draw_bars(m, env, this, recs, scratch);
    draw_ticker(m, env, recs, scratch);
    rcall(m, env, 0x1d0, &[8], &[]);
    put4(m, scratch, [ONE; 4]);
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, scratch]);
    let s = k(0x3f66_6666);
    fcall(m, env, SPRITE_RECT, &[recs, 0], &[F(10.0), ZERO, s, s]);
    if m.r32(this + 0xd74) == 1 {
        draw_tip_box(m, env, this, m.r32(this + 0xd74 + 0x30), 0, 7, F(23.0), k(0x4316_0000), ONE, scratch);
    }
    let n = m.r32(m.r32(APP) + 8);
    draw_flakes(m, env, recs, n as i32);
}

/// `cRiderRankingLoad::DrawOverlay` 0x24fa48(this): two text styles in the menu font; at step 4 on texture 10 the
/// headings 0xe73 and 0xe74 in gold, the ranking's heading typed in at 320, its text in a text box (on the stack,
/// typed in by the animation's line 2), and three labels right-aligned at 380 in black with their values typed in
/// at 410 in white (rows at 290, 323, 356)
pub fn ranking_draw_overlay(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let menu = this + 0xc;
    let (a, b, col, tb) = (scratch, scratch + 0x10, scratch + 0x20, scratch + 0x30);
    let anim = this + 0x40c;
    call(m, env, TS_INIT, &[a]);
    call(m, env, TS_CONSTRUCT, &[b]);
    call(m, env, TS_SET_FONT, &[a, menu]);
    m.w32(a + 4, 1);
    call(m, env, TS_SET_FONT, &[b, menu]);
    m.w32(b + 4, 1);
    if m.r32(this + 0xb4) == 4 {
        rcall(m, env, 0x1d0, &[0xa], &[]);
        put4(m, col, [ONE, k(0x3f73_f7cf), k(0x3f3a_9fbe), k(0x3dd9_1687)]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let s11 = k(0x3f8c_cccd);
        let s = loc(m, env, 0xe73);
        fcall(m, env, PRINTF_SHADOWED_A, &[this + 0x48, s], &[s11, F(15.0), F(25.0)]);
        let s = loc(m, env, 0xe74);
        fcall(m, env, PRINTF_SHADOWED_A, &[this + 0x48, s], &[s11, F(46.0), F(54.0)]);
        put4(m, col, [ONE, ZERO, ZERO, ZERO]);
        call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
        let s12 = k(0x3f99_999a);
        let cx = F(320.0);
        let n = call(m, env, ANIM_VALUE, &[anim, 1]);
        fcall(m, env, DRAW_ALIGNED, &[a, 0, this + 0x6a0, n, 0, 1], &[s12, cx, k(0x42dc_0000)]);
        call(m, env, TB_INIT, &[tb]);
        call(m, env, TB_SET_FONT, &[tb, menu]);
        call(m, env, TB_RESET, &[tb]);
        put4(m, col, [ONE, ZERO, ZERO, ZERO]);
        for i in 0..4 {
            m.w32(tb + 0x3c + 4 * i, m.r32(col + 4 * i));
        }
        m.w32(tb + 0x6c, 0);
        m.w32(tb + 0x24, 0);
        m.w32(tb + 0x20, 0);
        let s106 = k(0x3f87_ae14);
        fcall(m, env, TB_SET_SCALE, &[tb], &[s106, s106]);
        fcall(m, env, SET_RECT, &[tb], &[cx, k(0x4318_0000), k(0x4387_0000), F(120.0)]);
        call(m, env, TB_APPEND, &[tb, this + 0x6c8]);
        call(m, env, TB_WRAP, &[tb, 0]);
        m.w32(tb + 0x6c, 0);
        let n = call(m, env, ANIM_VALUE, &[anim, 2]);
        call(m, env, TB_DRAW, &[tb, n, 0]);
        let (lx, vx) = (F(380.0), k(0x43cd_0000));
        for (label, value, line, y) in [(0x8bcu32, 0x902u32, 3u32, 0x4391_0000u32), (0x948, 0x98e, 4, 0x43a1_8000), (0x9d4, 0xa1a, 5, 0x43b2_0000)] {
            put4(m, col, [ONE, ZERO, ZERO, ZERO]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
            fcall(m, env, DRAW_RIGHT, &[menu, this + label], &[lx, k(y), s12]);
            put4(m, col, [ONE; 4]);
            call(m, env, SET_COLOUR, &[DRAW_COLOUR, col]);
            let n = call(m, env, ANIM_VALUE, &[anim, line]);
            fcall(m, env, DRAW_ALIGNED, &[a, 0, this + value, n, 1, 1], &[s12, vx, k(y)]);
        }
        call(m, env, TB_DELETE, &[tb, 2]);
    }
    call(m, env, TS_DESTRUCT_THUNK, &[b, 2]);
    call(m, env, TS_DESTRUCT, &[a, 2]);
}

/// 0x24ff58 (the loading screens' static start): four colour records at 0x3d2c88 (white-gold, gold, red-gold, and a
/// sprite record: unset, white, full size)
pub fn static_init(m: &mut Mem) {
    let c = STATIC_COLOURS;
    let one = ONE.0.to_bits();
    for (o, v) in [
        (0x28u32, 0x3b03_126fu32), (0, one), (8, 0), (0x48, one), (0x20, one), (0x24, 0x3d9b_3d08), (0x10, one),
        (0x14, 0x3ef2_fec5), (0x18, 0), (4, 0x3f76_e979), (0x30, 0xffff_ffff), (0x34, one), (0x38, one), (0x3c, 0),
        (0x40, 0), (0x44, one),
    ] {
        m.w32(c + o, v);
    }
}

/// 0x250000(it, v): 0x14f5f0 on `it` with the four floats at `v` (`v` passed on too)
pub fn set_vec4(m: &mut Mem, env: &mut dyn TextBoxEnv, it: u32, v: u32) {
    let f = [rf(m, v), rf(m, v + 4), rf(m, v + 8), rf(m, v + 0xc)];
    fcall(m, env, SET_VEC4, &[it, v], &f);
}

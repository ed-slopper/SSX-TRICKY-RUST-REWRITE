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

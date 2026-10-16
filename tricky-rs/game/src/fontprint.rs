//! The fonts' shadowed prints (board row G8j): `Font_PrintfShadowed` 0x19d3f0 / `Font_PrintfShadowedA` 0x19d568 and
//! their centred and right-aligned forms, as the calls they make (through `hudsprites::SpritesEnv`), checked in the
//! function runner (`tests/fontprint.rs`). What they reach is the string's formatting (`vsprintf` 0x301ca0 /
//! `WStr_VFormat` 0x2c48e8), the string's measure (`Font_MeasureAscii` 0x19da30 / `Font_MeasureWide` 0x19dc78) and
//! its draw (`Font_DrawStringRaw` 0x19cf70 / `Font_DrawStringA` 0x19d0c0: the font, the string; x, y, the scale).
//!
//! Every print draws twice: a shadow 2 right and 2 down in black (the font colour's alpha kept), one layer below the
//! renderer's (never under 0), then the text in the font colour on the layer above that.
//!
//! The functions take `stack`, the game's stack pointer at the call: each builds its frame below it (the formatted
//! string at its bottom, the saved colour above).

use crate::hudsprites::SpritesEnv;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::replayui::FONT_COLOR;

const MEASURE_ASCII: u32 = 0x19da30;
const MEASURE_WIDE: u32 = 0x19dc78;

/// `Font_PrintfShadowed` 0x19d3f0 (ASCII) / `Font_PrintfShadowedA` 0x19d568 (wide): (the font, the format; the
/// scale, x, y)
#[allow(clippy::too_many_arguments)]
pub fn print_shadowed(m: &mut Mem, env: &mut dyn SpritesEnv, wide: bool, font: u32, fmt: u32, scale: f32, x: f32, y: f32, stack: u32) {
    let (size, saved, args) = if wide { (4320, 4096, 4272) } else { (2272, 2048, 2224) };
    let sp = stack.wrapping_sub(size);
    env.call(m, if wide { 0x2c48e8 } else { 0x301ca0 }, &[sp, fmt, sp + args], &[]);
    let l = env.renderer(m, 0x1dc, &[], &[]);
    for k in [0u32, 4, 8, 12] {
        let v = m.r32(FONT_COLOR + k);
        m.w32(sp + saved + k, v);
    }
    for k in [4u32, 8, 12] {
        m.w32(FONT_COLOR + k, 0);
    }
    let below = if (l.wrapping_sub(1) as i32) > -1 { l.wrapping_sub(1) } else { 0 };
    env.renderer(m, 0x1d4, &[below], &[]);
    let draw = if wide { 0x19d0c0 } else { 0x19cf70 };
    let (sx, sy) = (F(x) + F(2.0), F(y) + F(2.0));
    env.call(m, draw, &[font, sp], &[sx.0, sy.0, scale]);
    for k in [4u32, 12, 8] {
        let v = m.r32(sp + saved + k);
        m.w32(FONT_COLOR + k, v);
    }
    env.renderer(m, 0x1d4, &[below.wrapping_add(1)], &[]);
    env.call(m, draw, &[font, sp], &[x, y, scale]);
}

/// `Font_PrintfShadowedCentered` 0x19d6e0 (ASCII) / `Font_PrintShadowedCentered` 0x19d770 (wide): (the font, the
/// string; the scale, x, y) centred on x by its measured width (the size written to the frame, +8)
#[allow(clippy::too_many_arguments)]
pub fn print_centred(m: &mut Mem, env: &mut dyn SpritesEnv, wide: bool, font: u32, s: u32, scale: f32, x: f32, y: f32, stack: u32) {
    let sp = stack.wrapping_sub(96);
    env.call(m, if wide { MEASURE_WIDE } else { MEASURE_ASCII }, &[font, s, sp], &[scale]);
    let w = F(m.r32(sp + 8) as i32 as f32);
    let x = F(x) - w * F(0.5);
    print_shadowed(m, env, wide, font, s, scale, x.0, y, sp);
}

/// `Font_PrintShadowedRight` 0x19d868 (the font, the wide string; the scale, x, y): ending at x
#[allow(clippy::too_many_arguments)]
pub fn print_right(m: &mut Mem, env: &mut dyn SpritesEnv, font: u32, s: u32, scale: f32, x: f32, y: f32, stack: u32) {
    let w = F(env.call(m, MEASURE_WIDE, &[font, s, 0], &[scale]).0 as i32 as f32);
    let x = F(x) - w;
    print_shadowed(m, env, true, font, s, scale, x.0, y, stack);
}

/// `Font_PrintAsciiShadowedRight` 0x19d800 (the font, the ASCII string; the scale, x, y): made wide in the frame
/// (`WStr_SetAscii` 0x17ccb8), then `print_right`
#[allow(clippy::too_many_arguments)]
pub fn print_ascii_right(m: &mut Mem, env: &mut dyn SpritesEnv, font: u32, s: u32, scale: f32, x: f32, y: f32, stack: u32) {
    let sp = stack.wrapping_sub(464);
    env.call(m, 0x17ccb8, &[sp, s], &[]);
    print_right(m, env, font, sp, scale, x, y, sp);
}

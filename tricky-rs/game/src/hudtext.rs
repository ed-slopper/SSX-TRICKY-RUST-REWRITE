//! A player's HUD text (`HUD_DrawPlayerText` 0x1a4f70, board row G8h): every number and caption of one view's HUD as
//! the calls the game makes, from the HUD state of `hudstate.rs`, checked in the function runner
//! (`tests/hudtext.rs`). The calls go through `hudsprites::SpritesEnv`; the layout tables and the score entries are
//! as described in `hudsprites.rs`.
//!
//! By the HUD's flags (+0x104): 0x200 the lap (number and its ordinal), 0x1 the race time, 0x20000000 the gap to the
//! other player, 0x800 the speed, 0x200 without 0x1000000 "/" and the riders' count, 0x400 the kind-13 caption, 0x8
//! the multiplier, 0x100000 the kind-12 "X" bonus, 0x20000 the trick messages or the tricky prompt, 0x400000 the
//! kind-11 / 10 bonus and split, 0x100 the split markers' names, 0x1000000 the show-off's medal text; then the
//! race's pause prompt.

use crate::hudsprites::{bits, colour, fl, int, lay, put, text, white, SpritesEnv};
use crate::hudstate::wobble;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::GAPP;

/// `Font_PrintfShadowedA` 0x19d568 / `Font_PrintfShadowed` 0x19d3f0 / `Font_PrintShadowedCentered` 0x19d770 (a0 the
/// font, a1 the string; f12 the scale, f13 x, f14 y)
const PRINT_A: u32 = 0x19d568;
const PRINT: u32 = 0x19d3f0;
const PRINT_CENTRED: u32 = 0x19d770;
/// `Font_MeasureWide` 0x19dc78 / `Font_MeasureAscii` 0x19da30 (f12 the scale; the font, the string, the size out
/// or 0): the width
const MEASURE_WIDE: u32 = 0x19dc78;
const MEASURE_ASCII: u32 = 0x19da30;
/// `HUD_FormatTime` 0x1a6ba0 / `HUD_FormatTimeSigned` 0x1a6c90 (hundredths, the buffer)
const FORMAT_TIME: u32 = 0x1a6ba0;
const FORMAT_SIGNED: u32 = 0x1a6c90;
const SPRINTF: u32 = 0x2fca28;
/// `WStr_Format` 0x17ce38 / `WStr_Copy` 0x17cc80 / `WStr_FormatAscii` 0x17ce70
const WFORMAT: u32 = 0x17ce38;
const WCOPY: u32 = 0x17cc80;
const WFORMAT_ASCII: u32 = 0x17ce70;
/// `Boarder_GetRaceTime` 0x11ea28 (hundredths)
const RACE_TIME: u32 = 0x11ea28;
/// "%d" and the speed units' suffixes (mph, km/h)
pub const INT_FORMAT: u32 = 0x38d020;
pub const MPH: u32 = 0x38da70;
pub const KMH: u32 = 0x38da78;
/// The lap ordinals' strings (1st .. 4th), copied to the frame
pub const LAP_NAMES: [u32; 4] = [0xe44, 0xe54, 0xe58, 0xe45];
/// The time the gap counts from, the speed units (0 km/h), the career
pub const GAP_CLOCK: u32 = 0x32de98;
pub const UNITS: u32 = 0x32ca10;
pub const CAREER: u32 = 0x32b390;

fn race(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x730)
}
fn world(m: &Mem) -> u32 {
    m.r32(race(m) + 0x1c)
}
fn loc_font(m: &Mem, k: u32) -> u32 {
    m.r32(m.r32(GAPP) + k)
}
#[allow(clippy::too_many_arguments)]
fn print(m: &mut Mem, env: &mut dyn SpritesEnv, func: u32, font: u32, s: u32, sc: F, x: F, y: F) {
    env.call(m, func, &[font, s], &[sc.0, x.0, y.0]);
}
fn measure(m: &mut Mem, env: &mut dyn SpritesEnv, func: u32, sc: F, font: u32, s: u32, out: u32) -> F {
    int(env.call(m, func, &[font, s, out], &[sc.0]).0)
}
/// The ×0.5 alignment of a placed text (layout flags +8: 0x20 centre x, 0x40 centre y) by the measured size at `size`
fn align(m: &Mem, t: u32, size: u32, x: F, y: F) -> (F, F) {
    let (mut x, mut y) = (x, y);
    if m.r32(t + 8) & 0x20 != 0 {
        x = x - int(m.r32(size + 8)) * F(0.5);
    }
    if m.r32(t + 8) & 0x40 != 0 {
        y = y - int(m.r32(size + 12)) * F(0.5);
    }
    (x, y)
}
fn fade_edge(d: F, alpha: F) -> (F, F) {
    let off = d * F(150.0);
    let edge = if 0.0 < off.0 {
        (F(d.0.abs()) - F(0.25)) * F(4.0)
    } else {
        let e = F(d.0.abs()) - F(0.5);
        e + e
    };
    let a = if edge.0 < 0.0 {
        alpha
    } else if 1.0 < edge.0 {
        F(0.0)
    } else {
        (F(1.0) - edge) * alpha
    };
    (off, a)
}

/// `HUD_DrawPlayerText` 0x1a4f70 (the view's top, left, width and height; the HUD, the player). `sp` is the game's
/// stack frame there (0x210 bytes): its strings and measured sizes at +0 / +0x10 / +0x30 / +0x40 / +0x80 / +0xc0,
/// the colour vectors at +0 / +0x10
#[allow(clippy::too_many_arguments)]
pub fn draw_player_text(m: &mut Mem, env: &mut dyn SpritesEnv, top: f32, left: f32, width: f32, height: f32, hud: u32, i: u32, sp: u32) {
    let (top, left, width, height) = (F(top), F(left), F(width), F(height));
    let b = hud.wrapping_add(0x114 + 0x140 * i);
    let font = loc_font(m, 0x5c);
    let tall = int(m.r32(font + 0xc));
    let rider = m.r32(race(m).wrapping_add(i * 0x80) + 0xa4);
    let view = m.r32(crate::hudstate::VIEW_LAYOUT);
    let racing = |m: &Mem, r: u32| (m.r32(r + 0x418) as i32) < 0;
    let man = m.r32(race(m) + 0x38);
    let alpha = if env.call(m, 0x1a77a8, &[man], &[]).0 == 0 && racing(m, rider) { F(1.0) } else { bits(0x3eb33333) };
    env.renderer(m, 0x1d4, &[7], &[]);
    let flags = |m: &Mem| m.r32(hud + 0x104);
    if flags(m) & 0x200 != 0 {
        let (s1, v1) = (lay(m, 0x28), lay(m, 0x14));
        for (k, id) in LAP_NAMES.iter().enumerate() {
            m.w32(sp + 4 * k as u32, *id);
        }
        let laps = m.r32(rider + 0x110);
        let mut lap = laps.wrapping_add(1);
        if laps == 0 {
            colour(m, [alpha, bits(0x3f73f3f4), bits(0x3f3ababb), bits(0x3dd8d8d9)]);
        } else {
            colour(m, white(alpha));
        }
        let (x, y, sc, corner) = (left + fl(m, v1), top + fl(m, v1 + 4), fl(m, v1 + 12), m.r32(v1 + 8));
        env.call(m, 0x1a82e8, &[hud, lap, corner], &[x.0, y.0, sc.0, 0.0]);
        if (lap as i32) >= 5 {
            lap = 4;
        }
        let (x, y) = (left + fl(m, s1), top + fl(m, s1 + 4));
        let id = m.r32(sp.wrapping_add(lap.wrapping_sub(1).wrapping_mul(4)));
        let s = text(m, env, id);
        print(m, env, PRINT_A, font, s, fl(m, s1 + 12), x, y);
    }
    if flags(m) & 1 != 0 && m.r32(world(m) + 0x1c) != 3 {
        let s1 = lay(m, 0x64);
        if 0.0 < fl(m, s1 + 12).0 {
            let mut t = env.call(m, RACE_TIME, &[rider], &[]).0;
            let mut a = alpha;
            if !racing(m, rider) && view == 1 {
                let other = m.r32(race(m) + 0x124);
                if racing(m, other) {
                    let man = m.r32(race(m) + 0x38);
                    if env.call(m, 0x1a77a8, &[man], &[]).0 == 0 {
                        a = F(1.0);
                    }
                }
                let o = env.call(m, RACE_TIME, &[other], &[]).0;
                if (t as i32) < (o as i32) {
                    t = env.call(m, RACE_TIME, &[other], &[]).0;
                }
            }
            env.call(m, FORMAT_TIME, &[t, sp + 16], &[]);
            colour(m, white(a));
            if m.r32(s1 + 8) & 0x60 != 0 {
                measure(m, env, MEASURE_WIDE, fl(m, s1 + 12), font, sp + 16, sp);
                let (x, y) = align(m, s1, sp, left + fl(m, s1), top + fl(m, s1 + 4));
                print(m, env, PRINT_A, font, sp + 16, fl(m, s1 + 12), x, y);
            } else {
                let sc = fl(m, s1 + 12);
                let y = (top + fl(m, s1 + 4)) - tall * sc;
                print(m, env, PRINT_A, font, sp + 16, sc, left + fl(m, s1), y);
            }
        }
    }
    if flags(m) & 0x20000000 != 0 {
        let s3 = lay(m, 0x3fc);
        if 0.0 < fl(m, s3 + 12).0 && view < 2 {
            let mut gap = m.r32(GAP_CLOCK);
            let first = m.r32(race(m) + 0xa4);
            let second = m.r32(race(m) + 0x124);
            let timed = if second == 0 || racing(m, first) { first } else { second };
            if (env.call(m, RACE_TIME, &[timed], &[]).0 as i32) >= 0 {
                let t = env.call(m, RACE_TIME, &[timed], &[]).0;
                gap = gap.wrapping_sub(t);
            }
            let mut a = F(1.0);
            if !racing(m, first) && (second == 0 || !racing(m, second)) {
                a = alpha;
            }
            let mut sc = fl(m, s3 + 12);
            if !(view == 0 && !racing(m, rider)) {
                let secs = int(gap) / F(100.0);
                if 0.0 < secs.0 && secs.0 < 10.0 {
                    let frac = secs - int(secs.0 as i32 as u32);
                    let pulse = if frac.0 < 0.5 { frac + frac } else { (F(1.0) - frac) + (F(1.0) - frac) };
                    sc = sc + pulse * bits(0x3f333333);
                }
                env.call(m, FORMAT_SIGNED, &[gap, sp], &[]);
                if 0.0 < secs.0 {
                    colour(m, [a, F(0.0), bits(0x3f59999a), F(0.0)]);
                } else {
                    colour(m, [a, bits(0x3f666666), bits(0x3dcccccd), bits(0x3dcccccd)]);
                }
                if m.r32(s3 + 8) & 0x60 != 0 {
                    measure(m, env, MEASURE_WIDE, sc, font, sp, sp + 48);
                    let (x, y) = align(m, s3, sp + 48, left + fl(m, s3), top + fl(m, s3 + 4));
                    print(m, env, PRINT_A, font, sp, sc, x, y);
                } else {
                    let grow = sc - fl(m, s3 + 12);
                    let y = ((top + fl(m, s3 + 4)) - grow * F(10.0)) - sc;
                    let x = (left + fl(m, s3)) - grow * F(65.0);
                    print(m, env, PRINT_A, font, sp, sc, x, y);
                }
            }
        }
    }
    if flags(m) & 0x800 != 0 {
        let one = F(1.0);
        let sq = [0, 4, 8, 12].map(|k| fl(m, rider + 0x150 + k)).map(|x| x * x);
        let speed = (((sq[0] + sq[1]) + one * sq[2]) + one * sq[3]).sqrt();
        let (k, unit) = if m.r32(UNITS) != 0 { (bits(0x3cb73ff4), MPH) } else { (bits(0x3d1374bd), KMH) };
        let n = (speed * k).0 as i32 as u32;
        env.call(m, SPRINTF, &[sp, INT_FORMAT, n], &[]);
        env.call(m, SPRINTF, &[sp + 16, unit], &[]);
        let s0 = lay(m, 0x78);
        let wob = F(wobble(fl(m, b + 0x90).0, 1.5));
        let w = measure(m, env, MEASURE_ASCII, fl(m, s0 + 12) + wob, font, sp, 0);
        let sc = fl(m, s0 + 12) + wob;
        let x = left + fl(m, s0);
        let y = top + fl(m, s0 + 4);
        colour(m, white(alpha));
        let sc2 = fl(m, s0 + 16) + wob;
        print(m, env, PRINT, font, sp, sc, x - w, y - tall * sc);
        print(m, env, PRINT, font, sp + 16, sc2, x, y - tall * sc2);
    }
    if flags(m) & 0x1000200 == 0x200 {
        m.w8(sp, b'/');
        let t = lay(m, 0x3c);
        let n = m.r8(world(m) + 0x88).wrapping_add(b'0');
        m.w8(sp + 2, 0);
        m.w8(sp + 1, n);
        colour(m, white(alpha));
        print(m, env, PRINT, font, sp, fl(m, t + 12), left + fl(m, t), top + fl(m, t + 4));
    }
    let entry = |k: u32| rider + 0x597c + 20 * k;
    let ratio = |m: &Mem, e: u32| fl(m, e + 12) / fl(m, e + 8);
    if flags(m) & 0x400 != 0 {
        if let Some(e) = (0..12).map(entry).find(|&e| m.r32(e) == 13) {
            let s1 = lay(m, 0x2bc);
            let r = ratio(m, e);
            let sc = fl(m, s1 + 16) + fl(m, s1 + 12) * r;
            let fade = (F(1.0) - r) * alpha;
            let place = m.r32(e + 4);
            if place.wrapping_sub(1) == 1 {
                let s = text(m, env, 0x115);
                env.call(m, WCOPY, &[sp, s], &[]);
            } else {
                let s = text(m, env, 0x116);
                env.call(m, WFORMAT, &[sp, s, place.wrapping_sub(1)], &[]);
            }
            let w = measure(m, env, MEASURE_WIDE, sc, font, sp, 0);
            let (x, y) = (left + fl(m, s1), top + fl(m, s1 + 4));
            colour(m, [fade, F(1.0), bits(0x3f60e0e1), bits(0x3e0c8c8d)]);
            print(m, env, PRINT_A, font, sp, sc, x - w * F(0.5), y);
        }
    }
    if flags(m) & 8 != 0 {
        let s0 = lay(m, 0x50);
        let n = m.r32(rider + 0x5910);
        env.call(m, SPRINTF, &[sp, INT_FORMAT, n], &[]);
        let mut x = left + fl(m, s0);
        let mut mult = fl(m, b + 0x84);
        let sc = fl(m, s0 + 12) * mult;
        if m.r32(s0 + 8) & 8 != 0 {
            let w = measure(m, env, MEASURE_ASCII, sc, font, sp, 0);
            mult = fl(m, b + 0x84);
            x = x - w;
        }
        if mult.0 == 1.0 {
            colour(m, white(alpha));
        } else {
            colour(m, [alpha, F(0.0), F(1.0), F(0.0)]);
        }
        print(m, env, PRINT, font, sp, sc, x, top + fl(m, s0 + 4));
    }
    if flags(m) & 0x100000 != 0 {
        if let Some(e) = (0..12).map(entry).find(|&e| m.r32(e) == 12) {
            let s0 = lay(m, 0x26c);
            let n = m.r32(e + 4);
            match n {
                2 => colour(m, [alpha, F(1.0), bits(0x3f60e0e1), bits(0x3e0c8c8d)]),
                3 => colour(m, [alpha, F(1.0), bits(0x3ef0f0f1), bits(0x3d20a0a1)]),
                5 => colour(m, [alpha, bits(0x3f7dfdfe), F(0.0), F(0.0)]),
                _ => {}
            }
            m.w8(sp, b'X');
            m.w8(sp + 2, 0);
            m.w8(sp + 1, (m.r32(e + 4) as u8).wrapping_add(b'0'));
            let w = measure(m, env, MEASURE_ASCII, fl(m, s0 + 12), font, sp, 0);
            let x = (left + fl(m, s0)) - w * F(0.5);
            print(m, env, PRINT, font, sp, fl(m, s0 + 12), x, top + fl(m, s0 + 4));
        }
    }
    if flags(m) & 0x20000 != 0 {
        let msg = env.call(m, 0x155538, &[rider + 0x5820, sp + 256], &[]).0;
        let s1 = lay(m, 0x280);
        for (k, v) in [alpha, F(1.0), bits(0x3f5dddde), F(0.0)].into_iter().enumerate() {
            put(m, sp + 4 * k as u32, v);
        }
        if msg != 0 {
            let (x, y, w, sc) = (left + fl(m, s1), (top + height) - fl(m, s1 + 4), width * bits(0x3ecccccd), fl(m, s1 + 12));
            let corner = m.r32(s1 + 8);
            env.call(m, 0x1a6d38, &[msg, sp, corner], &[x.0, y.0, w.0, sc.0]);
        } else if m.r32(race(m) + 0x34) == 0 {
            let none = fl(m, sp + 12);
            let player = m.r8(m.r32(rider + 0x464) + 0x65) as i8 as i32 as u32;
            if none.0 < fl(m, rider + 0x24).0
                && (env.call(m, 0x164840, &[CAREER, player], &[]).0 as i32) >= 8
                && racing(m, rider)
            {
                let player = m.r8(m.r32(rider + 0x464) + 0x65) as i8 as i32 as u32;
                let w = world(m);
                let found = env.call(m, 0x1148e8, &[w, player], &[]).0;
                let mut f = fl(m, rider + 0x24) / F(5.0);
                if 1.0 < f.0 {
                    f = F(1.0);
                }
                if env.call(m, 0x11f430, &[rider], &[]).0 != 0 {
                    f = F(1.0);
                }
                let a = f * alpha;
                put(m, sp + 16, a);
                put(m, sp + 20, F(1.0));
                put(m, sp + 28, none);
                put(m, sp + 24, none);
                let (mut up, mut down) = (height - height * bits(0x3e23d70a), height - height * bits(0x3db851ec));
                if m.r32(race(m) + 0x298) == 2 {
                    up = height - height * bits(0x3e4ccccd);
                    down = height - height * bits(0x3e0f5c29);
                }
                let level = m.r32(rider + 0x28).wrapping_add(25);
                if m.r32(world(m) + 0x18) & 0x20 != 0 {
                    let y = top + up;
                    let c = [0, 4, 8, 12].map(|k| fl(m, sp + 16 + k));
                    colour(m, c);
                    let x = left + fl(m, s1);
                    let font2 = loc_font(m, 0x60);
                    let s = text(m, env, 0xdf2);
                    print(m, env, PRINT_CENTRED, font2, s, fl(m, s1 + 12), x, y);
                }
                let man = m.r32(race(m) + 0x38);
                let obj = env.call(m, 0x1a7c58, &[man], &[]).0.wrapping_add(0x7f8);
                let vt = m.r32(obj + 36);
                let this = obj.wrapping_add(m.r16(vt + 0x18) as i16 as i32 as u32);
                let (bar, look) = (m.r32(found + 0x870), m.r32(m.r32(rider + 0x464) + 0x38));
                env.call(m, m.r32(vt + 0x1c), &[this, bar, look, level], &[]);
                colour(m, white(a));
                let y = top + down;
                let x = left + width * F(0.5);
                let vt = m.r32(obj + 36);
                let this = obj.wrapping_add(m.r16(vt + 0x30) as i16 as i32 as u32);
                let bar = m.r32(found + 0x870);
                env.call(m, m.r32(vt + 0x34), &[this, bar], &[x.0, y.0]);
            }
        }
    }
    if flags(m) & 0x400000 != 0 {
        let s1 = lay(m, 0x104);
        for k in 0..12 {
            let e = entry(k);
            let kind = m.r32(e);
            if kind != 11 && kind != 10 {
                continue;
            }
            let r2 = ratio(m, e);
            let r2 = r2 + r2;
            if 1.0 < r2.0 {
                let fade = (F(1.0) - (r2 - F(1.0))) * alpha;
                let v = m.r32(e + 4);
                if kind == 11 {
                    colour(m, [fade, F(0.0), F(1.0), F(0.0)]);
                    let s = text(m, env, 0xe29);
                    env.call(m, WFORMAT, &[sp, s, v], &[]);
                } else if (v as i32) < 0 {
                    colour(m, [fade, F(1.0), F(0.0), F(0.0)]);
                    m.w16(sp, b'-' as u16);
                    env.call(m, FORMAT_TIME, &[v.wrapping_neg(), sp + 2], &[]);
                } else {
                    colour(m, [fade, F(0.0), F(1.0), F(0.0)]);
                    m.w16(sp, b'+' as u16);
                    env.call(m, FORMAT_TIME, &[v, sp + 2], &[]);
                }
                let x = left + fl(m, s1);
                let w = measure(m, env, MEASURE_WIDE, fl(m, s1 + 16), font, sp, 0);
                let sc = fl(m, s1 + 16);
                let y0 = fl(m, s1 + 4);
                let x = x - w * F(0.5);
                let y = (top + y0) - (tall * sc) * F(0.5);
                print(m, env, PRINT_A, font, sp, sc, x, y);
            }
            break;
        }
    }
    if flags(m) & 0x100 != 0 {
        let (s7, s4) = (lay(m, 0xa0), lay(m, 0x8c));
        colour(m, white(alpha));
        let mut k = 0u32;
        while (k as i32) < m.r32(hud + 0x2c) as i32 {
            let d = fl(m, b + 0xdc + 4 * k) / F(20000.0);
            if -1.0 < d.0 && d.0 < 1.0 {
                let (mut off, a) = fade_edge(d, alpha);
                let y0 = fl(m, s4 + 4) + fl(m, s7 + 4);
                let mut x = (left + fl(m, s4)) + fl(m, s7);
                let y0 = y0 - tall * F(0.5);
                colour(m, white(a));
                let kind = m.r32(hud + 0x94 + 4 * k) as i32;
                if (0..3).contains(&kind) {
                    let id = match kind {
                        1 => 0xe23,
                        0 => 0xe22,
                        _ => 0xe27,
                    };
                    let s = text(m, env, id);
                    let w = measure(m, env, MEASURE_WIDE, F(0.5), font, s, 0);
                    if 0.0 < off.0 && m.r32(hud + 0x94 + 4 * k) == 1 {
                        off = F(0.0);
                    }
                    if view == 0 {
                        x = x - F(20.0);
                    } else if view == 1 {
                        x = x - (w + F(3.0));
                    } else {
                        x = x + F(3.0);
                    }
                    // the markers' names sit on the track's offsets alone (no view top)
                    let y = y0 + off;
                    let s = text(m, env, id);
                    print(m, env, PRINT_A, font, s, F(0.5), x, y);
                }
            }
            k += 1;
        }
    }
    if flags(m) & 0x1000000 != 0 && m.r32(world(m) + 0x88) == 1 {
        let s1 = lay(m, 0x2e4);
        let s = text(m, env, 0xe28);
        env.call(m, WFORMAT, &[sp + 192, s], &[]);
        let medal = m.r32(b + 0x94) as i32;
        let wf = |m: &mut Mem, env: &mut dyn SpritesEnv, at: u32, id: u32| {
            let s = text(m, env, id);
            env.call(m, WFORMAT, &[at, s], &[]);
        };
        match medal {
            1 => {
                wf(m, env, sp + 128, 0xe32);
                wf(m, env, sp, 0xe26);
                let t = m.r32(hud + 0xfc);
                env.call(m, WFORMAT_ASCII, &[sp + 64, INT_FORMAT, t], &[]);
            }
            0 => {
                wf(m, env, sp, 0xe32);
                let t = m.r32(hud + 0x100);
                env.call(m, WFORMAT_ASCII, &[sp + 64, INT_FORMAT, t], &[]);
            }
            2 => {
                wf(m, env, sp + 128, 0xe3b);
                wf(m, env, sp, 0xe25);
                let t = m.r32(hud + 0xf8);
                env.call(m, WFORMAT_ASCII, &[sp + 64, INT_FORMAT, t], &[]);
            }
            3 => wf(m, env, sp + 128, 0xe35),
            _ => {}
        }
        colour(m, white(alpha));
        let flash = fl(m, b + 0x98);
        if flash.0 != 0.0 {
            let wob = F(wobble(flash.0, f32::from_bits(0x3f333333)));
            let sc = fl(m, s1 + 12) + wob;
            let y = top + fl(m, s1 + 4);
            let x = left + fl(m, s1);
            let sc2 = fl(m, s1 + 16) + wob;
            print(m, env, PRINT_A, font, sp + 128, sc, x, y - tall * sc);
            let (x, y) = (left + fl(m, s1), top + fl(m, s1 + 4));
            print(m, env, PRINT_A, font, sp + 192, sc2, x, y);
        } else if (m.r32(b + 0x94) as i32) < 3 {
            let s0 = lay(m, if m.r32(b + 0x94) == 0 { 0x2f8 } else { 0x30c });
            let sc = fl(m, s0 + 12);
            let y = (top + fl(m, s0 + 4)) - tall * sc;
            print(m, env, PRINT_A, font, sp, sc, left + fl(m, s0), y);
            let (x, y, sc) = (left + fl(m, s0), top + fl(m, s0 + 4), fl(m, s0 + 16));
            print(m, env, PRINT_A, font, sp + 64, sc, x, y);
        }
    }
    let pause = m.r32(race(m) + 0x40);
    if pause != 0 && m.r32(pause + 0x40) != 0 && m.r32(pause + 0x20) == 0 && (m.r32(pause + 0x3c) as i32) > 0 {
        let font2 = loc_font(m, 0x60);
        let s = text(m, env, 0x1b8);
        let w = measure(m, env, MEASURE_WIDE, F(1.0), font2, s, 0);
        let y = (top + height) - F(30.0);
        let x = (left + width * F(0.5)) - w * F(0.5);
        colour(m, [alpha, F(1.0), bits(0x3f46c6c7), F(0.0)]);
        let s = text(m, env, 0x1b8);
        print(m, env, PRINT_A, font2, s, F(1.0), x, y);
    }
}

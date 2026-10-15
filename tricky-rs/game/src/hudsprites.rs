//! A player's HUD sprites (`HUD_DrawPlayerSprites` 0x1a2c40, board row G8g): every sprite, number and caption of
//! one view's HUD as the draw calls the game makes, from the HUD state of `hudstate.rs`, checked in the function
//! runner (`tests/hudsprites.rs`).
//!
//! Everything is placed from the view layout tables (`HudOverlay_InitTables` fills 0x424 bytes per view from
//! 0x33ca40; a placed sprite reads +0 x, +4 y, +8 the corner, +0xc / +0x10 its scales) and is gated by the HUD's flags
//! (+0x104): 0x1000000 the speed wobble, 0x1000 the meter (its fourteen bands, the end cap), 0x2000 the tricky sprite,
//! 0x8000 the six combo letters, 0x4000 the combo stars, 0x80 the lap sprite, 0x100 the progress track (the riders'
//! dots, the split markers, the medal sprites), then (no overlay active) 0x4000000 / 0x8000000 / 0x10000000 the
//! score captions, 0x40000000 the kind-0x11 sprite, 0x10000 the score number and its floating trick scores, 0x400000
//! the kind-10 / 11 captions, 0x800000 the countdown.
//!
//! The rider's score entries (12 of 0x14 bytes at +0x597c): +0 the kind, +4 the points, +8 the time shown, +0xc the
//! time so far, +0x10 the points for the rank colour.

use crate::hudstate::{wobble, BANDS, VIEW_LAYOUT};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::GAPP;
use crate::replayui::FONT_COLOR;

/// What the HUD draws with
pub trait SpritesEnv {
    /// A function of the game with its integer (a0..) and float (f12..) arguments: its v0 and f0. `HUD_DrawSprite`
    /// 0x1c0068, `HUD_DrawNumber` 0x1a82e8, the overlay manager's `IsAnyVisible` 0x1a77a8 / `IsAnyActive` 0x1a77f8,
    /// `AIPathSet_GetMainPath` 0x197ef0, `Boarder_GetRaceProgress` 0x1170d0 / `_IsFinishedAndStopped` 0x11d998,
    /// `Score_PointsToIconTier` 0x156900 / `Score_GetRankForPoints` 0x19e7a0 (writes the rank's colour to a1),
    /// `Loc_GetString` 0x2c5160, `WStr_ToAscii` 0x17ccf8, `sprintf` 0x2fca28, `Font_PrintfShadowedCentered` 0x19d6e0,
    /// `Font_PrintShadowedCentered` 0x19d770, `SpriteSet_GetRect` 0x1f9e50, `Sprite_DrawTiled` 0x1fc728,
    /// `Sprite_DrawAnchored` 0x1fc178
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32);
    /// The renderer's virtual at `slot` with its integer (a1..) and float (f12..) arguments: +0x1d4 the layer,
    /// +0x1dc the layer now, +0x1e4 / +0x1fc / +0x1cc the blend states
    fn renderer(&mut self, m: &mut Mem, slot: u32, ints: &[u32], floats: &[f32]) -> u32;
}

/// The view layout tables (0x424 bytes per view)
pub const LAYOUTS: u32 = 0x33ca40;
/// The combo letters (six of 12 bytes: the layout entry, the sprite unlit, lit)
pub const COMBO_LETTERS: u32 = 0x338198;
/// The points under which the score's rank resets
pub const RANK_MIN: u32 = 0x338120;
/// The points' format ("%d")
pub const POINTS_FORMAT: u32 = 0x38da30;

const DRAW_SPRITE: u32 = 0x1c0068;
const DRAW_NUMBER: u32 = 0x1a82e8;
const LOC: u32 = 0x2c5160;
const PRINT: u32 = 0x19d770;
const PRINTF: u32 = 0x19d6e0;
const RECT: u32 = 0x1f9e50;
const RANK: u32 = 0x19e7a0;
/// The captions' gold (after the alpha)
const GOLD: [u32; 3] = [0x3f73f7cf, 0x3f3a9fbe, 0x3dd91687];

fn fl(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn put(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn int(v: u32) -> F {
    F(v as i32 as f32)
}
fn bits(b: u32) -> F {
    F(f32::from_bits(b))
}
/// The view's layout table (`DAT_003381e4` × 0x424 from 0x33ca40) plus `off`
fn lay(m: &Mem, off: u32) -> u32 {
    m.r32(VIEW_LAYOUT).wrapping_mul(0x424).wrapping_add(LAYOUTS).wrapping_add(off)
}
fn colour(m: &mut Mem, c: [F; 4]) {
    for (k, v) in c.into_iter().enumerate() {
        put(m, FONT_COLOR + 4 * k as u32, v);
    }
}
/// The colour built in the frame's vector (`sp`) and set by `Font_SetColorV` 0x14eee8
fn colour_v(m: &mut Mem, sp: u32, c: [F; 4]) {
    for (k, v) in c.into_iter().enumerate() {
        put(m, sp + 4 * k as u32, v);
    }
    let c = [fl(m, sp), fl(m, sp + 4), fl(m, sp + 8), fl(m, sp + 12)];
    colour(m, c);
}
fn white(a: F) -> [F; 4] {
    [a, F(1.0), F(1.0), F(1.0)]
}
#[allow(clippy::too_many_arguments)]
fn sprite(m: &mut Mem, env: &mut dyn SpritesEnv, x: F, y: F, sx: F, sy: F, hud: u32, id: u32, corner: u32) {
    env.call(m, DRAW_SPRITE, &[hud, id, corner], &[x.0, y.0, sx.0, sy.0]);
}
/// A sprite placed by a layout entry `t` from (x, y)
fn placed(m: &mut Mem, env: &mut dyn SpritesEnv, x: F, y: F, t: u32, hud: u32, id: u32) {
    let (px, py, sx, sy, corner) = (x + fl(m, t), y + fl(m, t + 4), fl(m, t + 12), fl(m, t + 16), m.r32(t + 8));
    sprite(m, env, px, py, sx, sy, hud, id, corner);
}
fn layer(m: &mut Mem, env: &mut dyn SpritesEnv, n: u32) {
    env.renderer(m, 0x1d4, &[n], &[]);
}
fn layer_now(m: &mut Mem, env: &mut dyn SpritesEnv) -> u32 {
    env.renderer(m, 0x1dc, &[], &[])
}
fn font(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x5c)
}
fn text(m: &mut Mem, env: &mut dyn SpritesEnv, id: u32) -> u32 {
    let loc = m.r32(m.r32(GAPP) + 0x50);
    env.call(m, LOC, &[loc, id], &[]).0
}
fn rect(m: &mut Mem, env: &mut dyn SpritesEnv, hud: u32, id: u32) -> u32 {
    let set = m.r32(hud);
    env.call(m, RECT, &[set, id], &[]).0
}

/// The score's rank frame round a number (wd wide at x, y, scale sc): the rank colour (read from the frame's
/// vector) times `fade`, the tiled middle (58) and the two ends (56 / 57)
#[allow(clippy::too_many_arguments)]
fn rank_frame(m: &mut Mem, env: &mut dyn SpritesEnv, hud: u32, sp: u32, fade: F, x: F, y: F, sc: F, wd: F, tiled: bool) {
    let c = fade * fl(m, sp);
    let tall = int(m.r32(font(m) + 0xc)) * sc + sc * F(14.0);
    let (c1, c2, c3) = (fl(m, sp + 4), fl(m, sp + 8), fl(m, sp + 12));
    colour(m, [c, c1, c2, c3]);
    let end = rect(m, env, hud, 56);
    let y = y - sc * F(7.0);
    let side = fl(m, end + 4) + tall * bits(0x3ebd70a4);
    let mid = rect(m, env, hud, 58);
    if tiled {
        env.call(m, 0x1fc728, &[mid, 32], &[x.0, y.0, wd.0, tall.0, wd.0, 1.0]);
    } else {
        env.call(m, 0x1fc178, &[mid, 32], &[x.0, y.0, wd.0, tall.0]);
    }
    let half = wd * F(0.5);
    let r = rect(m, env, hud, 56);
    env.call(m, 0x1fc178, &[r, 8], &[(x - half).0, y.0, side.0, tall.0]);
    let r = rect(m, env, hud, 57);
    env.call(m, 0x1fc178, &[r, 0], &[(x + half).0, y.0, side.0, tall.0]);
}

/// `HUD_DrawPlayerSprites` 0x1a2c40 (the view's top and left, its width; the HUD, the player). `sp` is the
/// game's stack frame there (0x180 bytes): the colour vector at +0, the caption strings at +0 / +0x10 / +0x20 / +0x30
// the captions' lift is the game's (scale − scale) × the font's height × 0.5, kept as it computes it
#[allow(clippy::too_many_arguments, clippy::eq_op)]
pub fn draw_player_sprites(m: &mut Mem, env: &mut dyn SpritesEnv, top: f32, left: f32, width: f32, hud: u32, i: u32, sp: u32) {
    let (top, left) = (F(top), F(left));
    let b = hud.wrapping_add(0x114 + 0x140 * i);
    let view = m.r32(VIEW_LAYOUT);
    let race = m.r32(m.r32(GAPP) + 0x730);
    let rider = m.r32(race.wrapping_add(i * 0x80) + 0xa4);
    let man = m.r32(race + 0x38);
    let racing = |m: &Mem| (m.r32(rider + 0x418) as i32) < 0;
    let alpha = if env.call(m, 0x1a77a8, &[man], &[]).0 == 0 && racing(m) { F(1.0) } else { F(0.5) };
    let flags = |m: &Mem| m.r32(hud + 0x104);
    env.renderer(m, 0x1e4, &[3], &[]);
    env.renderer(m, 0x1fc, &[1, 0], &[]);
    layer(m, env, 6);
    if flags(m) & 0x1000000 != 0 && m.r32(b + 0x94) != 0 {
        let id = m.r32(b + 0x94);
        colour(m, white(alpha));
        let t = lay(m, 0x2d0);
        let s = fl(m, t + 12) + F(wobble(fl(m, b + 0x98).0, f32::from_bits(0x3f333333)));
        let (x, y, corner) = (left + fl(m, t), top + fl(m, t + 4), m.r32(t + 8));
        sprite(m, env, x, y, s, s, hud, id.wrapping_add(38), corner);
    }
    if flags(m) & 0x1000 != 0 {
        let mut dim = alpha;
        let saved = layer_now(m, env);
        if fl(m, b + 12).0 == fl(m, BANDS).0 {
            dim = F(0.5);
        }
        let t = lay(m, 0x118);
        let ox = fl(m, t) + left;
        let oy = fl(m, t + 4) + top;
        layer(m, env, 8);
        let mut last = -1i32;
        for k in 0..14u32 {
            let sy = fl(m, b + 12 + 8 * k);
            let mut sx = fl(m, t + 12);
            let mut id = 21;
            colour(m, white(dim));
            if m.r32(b + 16 + 8 * k) != 0 {
                sx = sx * fl(m, b + 0x84);
                last = k as i32;
                id = m.r32(BANDS + 20 * k + 8);
            }
            let sx = sx * sy;
            let (x, y) = (ox + fl(m, BANDS + 20 * k + 0xc), oy + fl(m, BANDS + 20 * k + 0x10));
            sprite(m, env, x, y, sx, sy, hud, id, 96);
        }
        layer(m, env, saved);
        colour(m, white(dim));
        placed(m, env, ox, oy, lay(m, 0x12c), hud, 23);
        if flags(m) & 0x2000 != 0 {
            let v = fl(m, rider + 0x24);
            let t = lay(m, 0x1b8);
            m.w32(b + 0xa8, 1);
            let (x, y) = (ox + fl(m, t), oy + fl(m, t + 4));
            if 0.0 < v.0 {
                let left_ = F(20.0) - v;
                let f = if left_.0 < f32::from_bits(0x3f333333) { left_ / bits(0x3f333333) } else { v / bits(0x419a6666) };
                let f = f * F(40.0);
                let fall = F(1.0) - f / F(40.0);
                let old = fl(m, b + 0xb0);
                put(m, b + 0xac, f);
                let step = fall * bits(0x3e19999a);
                if racing(m) {
                    put(m, b + 0xb0, old - step);
                }
                let mut s = old;
                if s.0 <= -3.0 {
                    s = F(1.0) - F((s + F(3.0)).0.abs());
                    put(m, b + 0xb0, s);
                } else if s.0 <= -2.0 {
                    s = F((s + F(2.0)).0.abs());
                } else if s.0 < -1.0 {
                    s = -(s + F(2.0));
                }
                colour_v(m, sp, [F(1.0); 4]);
                let c = fl(m, FONT_COLOR) * dim;
                put(m, FONT_COLOR, c);
                let (yy, sx, sy, corner) = (y - fl(m, b + 0xac), fl(m, t + 12) * s, fl(m, t + 16), m.r32(t + 8));
                sprite(m, env, x, yy, sx, sy, hud, 37, corner);
            } else {
                put(m, b + 0xb0, F(1.0));
                put(m, b + 0xac, F(0.0));
                colour_v(m, sp, [bits(0x3f333333), F(1.0), F(1.0), F(1.0)]);
                let c = fl(m, FONT_COLOR) * dim;
                put(m, FONT_COLOR, c);
                let (yy, sx, sy, corner) = (y - fl(m, b + 0xac), fl(m, t + 12), fl(m, t + 16), m.r32(t + 8));
                sprite(m, env, x, yy, sx, sy, hud, 38, corner);
            }
        }
        if flags(m) & 0x8000 == 0 {
            m.w32(b + 0xc0, 0);
        } else {
            let lift = fl(m, b + 0xac);
            m.w32(b + 0xc0, 1);
            let (t, u) = (lay(m, 0x1cc), lay(m, 0x1b8));
            let y = ((oy + fl(m, t + 4)) + fl(m, u + 4)) - lift;
            let x = (ox + fl(m, t)) + fl(m, u);
            colour_v(m, sp, [F(1.0); 4]);
            let count = m.r32(rider + 0x58a8) as i32;
            let c = fl(m, FONT_COLOR) * alpha;
            put(m, FONT_COLOR, c);
            for k in 0..6u32 {
                let e = COMBO_LETTERS + 12 * k;
                let t = lay(m, m.r32(e).wrapping_mul(0x14));
                let id = if (k as i32) < count { m.r32(e + 8) } else { m.r32(e + 4) };
                placed(m, env, x, y, t, hud, id);
            }
        }
        if flags(m) & 0x4000 != 0 {
            let v = fl(m, b + 0x88);
            if 0.0 < v.0 && m.r32(b + 0xc0) == 0 && m.r32(b + 0xa8) == 0 {
                let n = ((v * F(14.0)).0 as i32) % 7;
                let t = lay(m, 0x140);
                let (y, x) = (oy + fl(m, t + 4), ox + fl(m, t));
                let saved = layer_now(m, env);
                layer(m, env, 8);
                let kind = m.r32(BANDS.wrapping_add((20 * last) as u32) + 8);
                let c = match kind {
                    0x12 => [0x3f800000, 0x3f76f6f7, 0x3e60e0e1, 0x3e109091],
                    0x13 => [0x3f800000, 0x3f73f3f4, 0x3f28a8a9, 0x3d808081],
                    _ => [0x3f800000, 0x3f7bfbfc, 0x3f7bfbfc, 0x3eacacad],
                };
                colour(m, c.map(bits));
                // the stars fall through from the most (5 / 6) down to one
                let stars = [(0x154, 13), (0x168, 14), (0x17c, 15), (0x190, 16), (0x1a4, 17)];
                let many = if n >= 5 { 5 } else { n.max(0) as usize };
                for &(off, id) in stars[..many].iter().rev() {
                    let t = lay(m, off);
                    placed(m, env, x, y, t, hud, id);
                }
                layer(m, env, saved);
            }
        }
    }
    if flags(m) & 0x80 != 0 {
        colour(m, white(alpha));
        if m.r32(b + 0xa4) != 0 {
            placed(m, env, left, top, lay(m, 0xf0), hud, 24);
        }
    }
    if flags(m) & 0x100 != 0 {
        let saved = layer_now(m, env);
        let path = env.call(m, 0x197ef0, &[0x337fb8], &[]).0;
        let length = fl(m, path + 0x38);
        let (s3, s1) = (lay(m, 0xa0), lay(m, 0x8c));
        layer(m, env, 10);
        colour(m, white(alpha));
        let x = (left + fl(m, s1)) + fl(m, s3);
        let y = (top + fl(m, s1 + 4)) + fl(m, s3 + 4);
        if view < 2 {
            let (sx, sy) = (fl(m, s3 + 12), fl(m, s1 + 16));
            sprite(m, env, x, y, sx, sy, hud, 3, 72);
        } else {
            let (sx, sy) = (fl(m, s1 + 12), fl(m, s1 + 16));
            sprite(m, env, x, y, sx, sy, hud, 2, 64);
        }
        let world = |m: &Mem| m.r32(m.r32(m.r32(GAPP) + 0x730) + 0x1c);
        let mut k = 0u32;
        while (k as i32) < m.r32(world(m) + 0x88) as i32 {
            // Race_GetRider 0x1814a8
            let other = m.r32(world(m).wrapping_add(4 * k) + 0xc4);
            if other != rider && 0.0 < length.0 {
                let mut d = fl(m, b + 0xc4 + 4 * k) / F(20000.0);
                if fl(m, other + 0x374).0 < 0.0 {
                    d = F(env.call(m, 0x1170d0, &[rider], &[]).1);
                }
                if -1.0 < d.0 && d.0 < 1.0 {
                    let edge = F(d.0.abs()) - F(0.5);
                    let edge = edge + edge;
                    let off = d * F(150.0);
                    let a = if edge.0 < 0.0 { alpha } else { (F(1.0) - edge) * alpha };
                    colour(m, white(a));
                    let (sx, sy, yy) = (fl(m, s1 + 12), fl(m, s1 + 16), y + off);
                    if (m.r8(m.r32(other + 0x464) + 0x65) as i8) < 0 {
                        layer(m, env, 8);
                        if view == 0 || view == 2 {
                            sprite(m, env, x, yy, sx, sy, hud, 5, 64);
                        } else {
                            sprite(m, env, x, yy, sx, sy, hud, 6, 72);
                        }
                    } else {
                        layer(m, env, 9);
                        if view == 2 {
                            sprite(m, env, x, yy, sx, sy, hud, 1, 64);
                        } else if view == 1 {
                            sprite(m, env, x, yy, sx, sy, hud, 4, 72);
                        }
                    }
                }
            }
            k += 1;
        }
        layer(m, env, 7);
        let mut k = 0u32;
        while (k as i32) < m.r32(hud + 0x2c) as i32 {
            let d = fl(m, b + 0xdc + 4 * k) / F(20000.0);
            if -1.0 < d.0 && d.0 < 1.0 {
                let mut off = d * F(150.0);
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
                colour(m, white(a));
                let id = match m.r32(hud + 0x94 + 4 * k) as i32 {
                    1 => {
                        if 0.0 < off.0 {
                            off = F(0.0);
                        }
                        7
                    }
                    0 | 2 => 8,
                    _ => 0,
                };
                let xx = (left + fl(m, s1)) + fl(m, s3);
                let yy = ((top + fl(m, s1 + 4)) + fl(m, s3 + 4)) + off;
                let (sx, sy) = (fl(m, s1 + 12), fl(m, s1 + 16));
                match view {
                    0 => {
                        sprite(m, env, xx, yy, sx, sy, hud, id, 8);
                        sprite(m, env, xx, yy, sx, sy, hud, id, 1);
                    }
                    1 => sprite(m, env, xx, yy, sx, sy, hud, id, 8),
                    2 => sprite(m, env, xx, yy, sx, sy, hud, id, 1),
                    _ => {}
                }
            }
            k += 1;
        }
        env.renderer(m, 0x1cc, &[1, 0], &[f32::from_bits(0x3da3d70a)]);
        layer(m, env, saved);
        colour(m, white(alpha));
        let t = lay(m, 0xb4);
        let s = fl(m, t + 12);
        if 0.0 < s.0 {
            let yb = top + fl(m, s1 + 4);
            let xb = left + fl(m, s1);
            let (x, y, sy, corner) = (xb + fl(m, t), yb + fl(m, t + 4), fl(m, t + 16), m.r32(t + 8));
            sprite(m, env, x, y, s, sy, hud, 9, corner);
            placed(m, env, xb, yb, lay(m, 0xc8), hud, 10);
            placed(m, env, xb, yb, lay(m, 0xdc), hud, 9);
        }
        env.renderer(m, 0x1cc, &[1, 2], &[f32::from_bits(0x3da3d70a)]);
    }
    if env.call(m, 0x1a77f8, &[m.r32(race + 0x38)], &[]).0 != 0 {
        return;
    }
    let entry = |k: u32| rider + 0x597c + 20 * k;
    let ratio = |m: &Mem, e: u32| fl(m, e + 12) / fl(m, e + 8);
    let gold = |m: &mut Mem, fade: F| colour(m, [fade, bits(GOLD[0]), bits(GOLD[1]), bits(GOLD[2])]);
    let mut names = false;
    if flags(m) & 0x4000000 != 0 {
        for k in 0..12 {
            let e = entry(k);
            if m.r32(e) != 15 {
                continue;
            }
            names = true;
            let half = F(width) * F(0.5);
            let scale = if view != 0 { bits(0x3f19999a) } else { bits(0x3f70a3d7) };
            let none = scale - scale;
            let r = ratio(m, e);
            let tall = int(m.r32(font(m) + 0xc));
            let fade = (F(1.0) - r) * alpha;
            let lift = (tall * none) * F(0.5);
            let tier = env.call(m, 0x156900, &[rider + 0x5820, m.r32(e + 4)], &[]).0 as i32;
            let id = match tier {
                2 => Some(0xeb3),
                3 => Some(0xeb4),
                4 => Some(0xeb5),
                t if t > 4 => Some(0xeb6),
                _ => None,
            };
            if let Some(id) = id {
                let s = text(m, env, id);
                env.call(m, 0x17ccf8, &[sp + 16, s], &[]);
            }
            let pts = m.r32(e + 4);
            env.call(m, 0x2fca28, &[sp + 48, POINTS_FORMAT, pts], &[]);
            let x = left + half;
            gold(m, fade);
            let f = font(m);
            env.call(m, PRINTF, &[f, sp + 16], &[scale.0, x.0, ((top + F(175.0)) - lift).0]);
            env.call(m, PRINTF, &[f, sp + 48], &[scale.0, (x - F(15.0)).0, ((top + F(225.0)) - lift).0]);
        }
    }
    let mut tens = false;
    if flags(m) & 0x8000000 != 0 {
        for k in 0..12 {
            let e = entry(k);
            if m.r32(e) != 16 {
                continue;
            }
            tens = true;
            let half = F(width) * F(0.5);
            let (r1, r2) = if names { (F(275.0), F(325.0)) } else { (F(175.0), F(225.0)) };
            let scale = if view != 0 { bits(0x3f19999a) } else { bits(0x3f70a3d7) };
            let x = left + half;
            let none = scale - scale;
            let r = ratio(m, e);
            let tall = int(m.r32(font(m) + 0xc));
            let fade = (F(1.0) - r) * alpha;
            let lift = (tall * none) * F(0.5);
            let s = text(m, env, 0xe94);
            env.call(m, 0x17ccf8, &[sp, s], &[]);
            let pts = m.r32(e + 4);
            env.call(m, 0x2fca28, &[sp + 32, POINTS_FORMAT, pts], &[]);
            gold(m, fade);
            let f = font(m);
            env.call(m, PRINTF, &[f, sp], &[scale.0, x.0, ((top + r1) - lift).0]);
            env.call(m, PRINTF, &[f, sp + 32], &[scale.0, (x - F(15.0)).0, ((top + r2) - lift).0]);
        }
    }
    if flags(m) & 0x10000000 != 0 && !(names && tens) {
        for k in 0..12 {
            let e = entry(k);
            if m.r32(e) != 4 {
                continue;
            }
            let half = F(width) * F(0.5);
            let down = if names || tens { F(300.0) } else { F(210.0) };
            let scale = if view != 0 { bits(0x3f333333) } else { F(1.0) };
            let x = left + half;
            let y = top + down;
            let r = ratio(m, e);
            gold(m, (F(1.0) - r) * alpha);
            let f = font(m);
            let s = text(m, env, 0xe93);
            env.call(m, PRINT, &[f, s], &[scale.0, x.0, y.0]);
        }
    }
    if flags(m) & 0x40000000 != 0 {
        let t = lay(m, 0x410);
        for k in 0..12 {
            if m.r32(entry(k)) != 17 {
                continue;
            }
            let (x, y, s) = (fl(m, t), fl(m, t + 4), fl(m, t + 12));
            let saved = layer_now(m, env);
            layer(m, env, 7);
            colour_v(m, sp, [F(1.0); 4]);
            put(m, FONT_COLOR, alpha);
            sprite(m, env, x, y, s, s, hud, 53, 96);
            layer(m, env, saved);
        }
    }
    if flags(m) & 0x10000 != 0 {
        let t = lay(m, 0x258);
        let pts = m.r32(b + 8);
        if (pts as i32) <= 0 {
            m.w32(b + 0xb4, u32::MAX);
        } else {
            let mut sc = fl(m, t + 12);
            if flags(m) & 0x2000000 != 0 {
                let y = top + fl(m, t + 4);
                let x = left + fl(m, t);
                let saved = layer_now(m, env);
                if int(pts).0 < fl(m, RANK_MIN).0 {
                    m.w32(b + 0xb4, 0);
                    m.w32(b + 0xb8, 0);
                }
                let rank = env.call(m, RANK, &[pts, sp], &[]).0;
                let frame = int(m.r32(m.r32(GAPP) + 0x10));
                let grow;
                if m.r32(b + 0xb4) == rank {
                    let since = frame - fl(m, b + 0xbc);
                    if since.0 <= 36.0 {
                        grow = if since.0 < 18.0 {
                            ((sc * bits(0x3f4ccccd)) * since) / F(18.0)
                        } else {
                            let f = (since - F(18.0)) / F(36.0);
                            (sc * bits(0x3f4ccccd)) * (F(1.0) - f)
                        };
                        put(m, b + 0xb8, grow);
                    } else {
                        m.w32(b + 0xb8, 0);
                        grow = fl(m, b + 0xb8);
                    }
                } else {
                    let g = fl(m, b + 0xb8);
                    if g.0 == 0.0 {
                        grow = F(0.0);
                        put(m, b + 0xb8, grow);
                        put(m, b + 0xbc, frame);
                    } else {
                        put(m, b + 0xbc, frame - (g / bits(0x3f4ccccd)) * F(18.0));
                        grow = g;
                    }
                    m.w32(b + 0xb4, rank);
                }
                colour(m, white(alpha));
                sc = sc + grow;
                layer(m, env, saved.wrapping_add(2));
                let corner = m.r32(t + 8);
                let wd = F(env.call(m, DRAW_NUMBER, &[hud, pts, corner], &[x.0, y.0, sc.0, 0.0]).1);
                layer(m, env, saved);
                rank_frame(m, env, hud, sp, alpha, x, y, sc, wd, true);
            } else {
                colour(m, white(alpha));
                let (x, y, corner) = (left + fl(m, t), top + fl(m, t + 4), m.r32(t + 8));
                env.call(m, DRAW_NUMBER, &[hud, pts, corner], &[x.0, y.0, sc.0, 0.0]);
            }
        }
        if racing(m) {
            for k in 0..12 {
                let e = entry(k);
                let kind = m.r32(e);
                if kind == 2 || kind == 3 {
                    let r = ratio(m, e);
                    let to = lay(m, 0x50);
                    let (x0, y0) = (fl(m, t), fl(m, t + 4));
                    let ease = r * r + r;
                    let x = (left + x0) + (fl(m, to) - x0) * ease;
                    let y = (top + y0) + (fl(m, to + 4) - y0) * ease;
                    let sc = (F(1.0) - r * F(0.5)) * ((fl(m, b + 0x84) * fl(m, t + 16)) * fl(m, t + 12));
                    let fade = (F(1.0) - r) * alpha;
                    if kind == 3 {
                        colour(m, [fade, F(1.0), F(0.5), F(0.5)]);
                    } else {
                        colour(m, white(fade));
                    }
                    let saved = layer_now(m, env);
                    if flags(m) & 0x2000000 != 0 {
                        layer(m, env, saved.wrapping_add(2));
                    }
                    let (pts, corner) = (m.r32(e + 4), m.r32(t + 8));
                    let wd = F(env.call(m, DRAW_NUMBER, &[hud, pts, corner], &[x.0, y.0, sc.0, 0.0]).1);
                    if flags(m) & 0x2000000 != 0 {
                        layer(m, env, saved);
                        let p = m.r32(e + 16);
                        env.call(m, RANK, &[p, sp], &[]);
                        rank_frame(m, env, hud, sp, fade, x, y, sc, wd, false);
                    }
                } else if kind == 1 {
                    let r = ratio(m, e);
                    colour(m, white((F(1.0) - r) * alpha));
                    let (x, y, sc, pts, corner) = (left + fl(m, t), top + fl(m, t + 4), fl(m, t + 12), m.r32(e + 4), m.r32(t + 8));
                    env.call(m, DRAW_NUMBER, &[hud, pts, corner], &[x.0, y.0, sc.0, r.0]);
                } else if kind == 14 {
                    let r = ratio(m, e);
                    let u = lay(m, 0x334);
                    let fade = (F(1.0) - r) * alpha;
                    let y = top + fl(m, u + 4);
                    let sc = fl(m, u + 12) + r * bits(0x3f400000);
                    let x = left + fl(m, u);
                    colour(m, white(fade));
                    let f = font(m);
                    let s = text(m, env, 0xe41);
                    env.call(m, PRINT, &[f, s], &[sc.0, x.0, y.0]);
                }
            }
        }
    }
    if flags(m) & 0x400000 != 0 {
        for k in 0..12 {
            let e = entry(k);
            let kind = m.r32(e);
            if kind != 11 && kind != 10 {
                continue;
            }
            let t = lay(m, 0x104);
            let r = ratio(m, e);
            let x = left + fl(m, t);
            let r2 = r + r;
            let y = top + fl(m, t + 4);
            if r2.0 < 1.0 {
                let fade = (F(1.0) - r2) * alpha;
                let green = if kind == 11 { F(0.0) } else { F(1.0) };
                colour(m, [fade, green, F(1.0), F(1.0)]);
                let f = font(m);
                let s = text(m, env, if kind == 11 { 0xe59 } else { 0xe40 });
                let sc = fl(m, t + 12);
                env.call(m, PRINT, &[f, s], &[sc.0, x.0, y.0]);
            }
        }
    }
    if flags(m) & 0x800000 != 0 && env.call(m, 0x11d998, &[rider], &[]).0 == 0 && racing(m) {
        let world = m.r32(m.r32(m.r32(GAPP) + 0x730) + 0x1c);
        let c = fl(m, world + 0x14);
        if c.0 < 11.0 && 1.0 < c.0 {
            let t = lay(m, 0x320);
            let n = c.0 as i32;
            let frac = c - int(n as u32);
            let green = bits(0x3dc0c0c1);
            colour(m, [frac * alpha, bits(0x3f66e6e7), green, green]);
            let (x, y, sc, corner) = (left + fl(m, t), top + fl(m, t + 4), fl(m, t + 12) + (F(1.0) - frac), m.r32(t + 8));
            env.call(m, DRAW_NUMBER, &[hud, n as u32, corner], &[x.0, y.0, sc.0, 0.0]);
        }
    }
}

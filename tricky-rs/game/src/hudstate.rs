//! The HUD overlay's state (`cHUDOverlay`, board row G8e), on the game's bytes (`pf::Mem`), checked in the function
//! runner (`tests/hudstate.rs`). The drawing itself is `HUD_DrawPlayerSprites` / `HUD_DrawPlayerText` (ported in
//! `hudsprites.rs`); here is what feeds them.
//!
//! The HUD overlay (0x618 bytes): +0x2c the split markers' count, +0x30 their course distances (25), +0x94 their kinds
//! (0 a checkpoint, 1 the start, 2 a second kind; -1 free), +0xf8 / +0xfc / +0x100 the time trial's three medal
//! times, +0x104 the HUD's flags (from +0x108 / +0x10c / +0x110 by the HUD option), +0x114 four players' blocks of
//! 0x140 bytes, +0x614 the race clock last frame (whole seconds).
//!
//! A player's block: +0 the fade-in (0..1), +4 the score shown (rolling up to the points), +8 the base points, +0xc
//! fourteen colour pairs (the meter's bands: +0xc the colour, +0x10 lit), +0x7c the meter shown, +0x80 the multiplier
//! shown, +0x84 the wobble scale, +0x88 the wobble's progress, +0x8c / +0x90 the speed flash (on, its time), +0x94 /
//! +0x98 the time trial's medal flash (the medals passed, its time), +0x9c the blink counter (0..53), +0xa0 the blink
//! on, +0xa4 the lap changed, +0xb4 -1, +0xc4 the riders' progress bars, +0xdc the split markers' distances ahead.

use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::GAPP;

/// The meter's band colours (fourteen of 0x14 bytes: +0 unlit, +4 lit)
pub const BANDS: u32 = 0x338008;
/// The race paths (`AIPathSet` at 0x337fb8: +8 the count, +0xc the paths of 0x3c bytes)
pub const PATHS: u32 = 0x337fb8;
/// The game mode (3 / 5 time trial and race with the countdown, 6 the tutorial), the lap mode (8 three laps)
pub const GAME_MODE: u32 = 0x32de8c;
pub const LAP_MODE: u32 = 0x32de88;
/// The HUD option (`Game_GetHudOption` 0x17f158) and the split-screen layout (`Game_GetViewportMode` 0x17f190)
pub const HUD_OPTION: u32 = 0x32ca40;
pub const VIEW_MODE: u32 = 0x32ca18;
/// The view the HUD draws in (`HUD_SetViewLayout`)
pub const VIEW_LAYOUT: u32 = 0x3381e4;

/// What the HUD calls out to
pub trait HudEnv {
    /// A function of the game with its arguments: its result (the overlay manager's getters 0x1a7be0 / 0x1a7c88 /
    /// 0x1a7c58, `OverlayMan_IsAnyActive` 0x1a77f8, `AIPathSet_GetMainPath` 0x197ef0, `Race_GetRider` 0x1814a8, the
    /// scores' `Score_CalcPoints` 0x155440 / `_CalcBasePoints` 0x1554c0, the sounds 0x233ff0 / 0x216930 / 0x21a210)
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// `Score_GetMultiplier` 0x155410 (a float)
    fn multiplier(&mut self, m: &mut Mem, score: u32) -> f32;
    /// `Boarder_GetRaceProgress` 0x1170d0 (a float)
    fn progress(&mut self, m: &mut Mem, rider: u32) -> f32;
    /// The renderer's virtual at `slot` (+0x5c / +0x64 the viewport's size, +0x1d4 the layer)
    fn renderer(&mut self, m: &mut Mem, slot: u32, args: &[u32]) -> u32;
    /// `HUD_DrawPlayerSprites` 0x1a2c40 / `HUD_DrawPlayerText` 0x1a4f70 (x, y, w, h: the view; the HUD, the player)
    #[allow(clippy::too_many_arguments)]
    fn draw_player(&mut self, m: &mut Mem, func: u32, x: f32, y: f32, w: f32, h: f32, hud: u32, i: u32);
}

fn fl(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn put(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn race(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x730)
}
fn riders(m: &Mem) -> i32 {
    m.r32(m.r32(race(m) + 0x1c) + 0x88) as i32
}
const AUDIO: u32 = 0x343050;

/// `HUD_HasSplitAt` 0x1a1e50 (a distance, a kind): a marker of the kind within 500
pub fn has_split_at(m: &Mem, hud: u32, d: f32, kind: u32) -> bool {
    (0..25).any(|i| m.r32(hud + 0x94 + 4 * i) == kind && (F(d) - fl(m, hud + 0x30 + 4 * i)).0.abs() < 500.0)
}

/// `HUD_BuildSplitList` 0x1a1968 (`cHUDOverlay::Reset` and vf16 jump to it): the markers cleared, the start (kind 1
/// at 0), then each race path's markers of kind 12 (as kind 0) and 13 (as kind 2) at the path's length less the
/// marker's distance (three laps of them in lap mode 8: laps 0..2 for 12, 1..2 for 13) unless one is within 500;
/// then each player's block set up, its markers' distances ahead from its rider's course length
pub fn reset(m: &mut Mem, env: &mut dyn HudEnv, hud: u32) {
    m.w32(hud + 0x2c, 0);
    for i in 0..25 {
        m.w32(hud + 0x30 + 4 * i, 0);
        m.w32(hud + 0x94 + 4 * i, u32::MAX);
    }
    m.w32(hud + 0x94, 1);
    m.w32(hud + 0x2c, 1);
    m.w32(hud + 0x30, 0);
    let mut n = 1u32;
    let mut p = 0i32;
    while p < m.r32(PATHS + 8) as i32 {
        let path = m.r32(PATHS + 0xc) + 0x3c * p as u32;
        let len = fl(m, path + 0x38);
        let mut mk = m.r32(path + 4);
        let mut j = 0;
        while j < m.r32(path) as i32 {
            let kind = m.r32(mk);
            if kind == 12 || kind == 13 {
                let out = if kind == 12 { 0 } else { 2 };
                if m.r32(LAP_MODE) == 8 {
                    let mp = env.call(m, 0x197ef0, &[PATHS]);
                    let lap = fl(m, mp + 0x38);
                    for k in (if kind == 12 { 0 } else { 1 })..3 {
                        let d = (len - fl(m, mk + 8)) + F(k as f32) * lap;
                        if !has_split_at(m, hud, d.0, out) {
                            put(m, hud + 0x30 + 4 * n, d);
                            m.w32(hud + 0x94 + 4 * n, out);
                            n += 1;
                        }
                    }
                } else {
                    let d = len - fl(m, mk + 8);
                    if !has_split_at(m, hud, d.0, out) {
                        m.w32(hud + 0x94 + 4 * n, out);
                        put(m, hud + 0x30 + 4 * n, d);
                        n += 1;
                    }
                }
            }
            mk += 16;
            j += 1;
        }
        p += 1;
    }
    m.w32(hud + 0x2c, n);
    for i in 0..4u32 {
        let rider = m.r32(race(m) + 0xa4 + 0x80 * i);
        let b = hud + 0x114 + 0x140 * i;
        if rider == 0 {
            continue;
        }
        m.w32(b + 0x7c, 0);
        m.w32(b + 0x80, 0);
        for k in 0..14 {
            m.w32(b + 0x10 + 8 * k, 0);
            m.w32(b + 0xc + 8 * k, m.r32(BANDS + 20 * k));
        }
        let one = 1.0f32.to_bits();
        for (o, v) in [(0x9c, 0), (0xb4, u32::MAX), (0x94, 0), (0xa0, 0), (0, one), (0x84, one), (0x8c, 0), (0x90, 0), (0x98, 0), (0x88, 0), (0xa4, 0), (0xa8, 0), (0xac, 0), (0xb0, one), (0xb8, 0), (0xbc, one), (0xc0, 0)] {
            m.w32(b + o, v);
        }
        let mut j = 0;
        while j < riders(m) {
            m.w32(b + 0xc4 + 4 * j as u32, 0);
            j += 1;
        }
        for k in 0..25 {
            let course = fl(m, rider + 0x380);
            let d = fl(m, hud + 0x30 + 4 * k) - course;
            put(m, b + 0xdc + 4 * k, d);
        }
    }
}

/// `HUD_WobbleOffset` 0x1a68f8 (the progress 0..1, the scale): a shake dying out over the first quarter
pub fn wobble(x: f32, scale: f32) -> f32 {
    if 0.25 < x {
        return 0.0;
    }
    let amp = (F(1.0) - F(x)) * F(0.5);
    let a = (F(x) * F(std::f32::consts::PI)) * F(4.0);
    if amp.0 < f32::from_bits(0x3d4ccccd) {
        return 0.0;
    }
    ((F(crate::math::sin(a.0)) * amp) * F(scale)).0
}

/// `HUD_ApplyHudOption` 0x1a1eb0: the HUD's flags (+0x104) for the HUD option: 0 +0x10c, 1 +0x108, else +0x110
pub fn apply_hud_option(m: &mut Mem, hud: u32) {
    let v = match m.r32(HUD_OPTION) {
        0 => m.r32(hud + 0x10c),
        1 => m.r32(hud + 0x108),
        _ => m.r32(hud + 0x110),
    };
    m.w32(hud + 0x104, v);
}

/// `HUD_SetViewLayout` 0x1a6860 (the view): one view 0; two views side by side (not the stacked layout) 1 / 2
pub fn set_view_layout(m: &mut Mem, view: u32) {
    match m.r32(race(m) + 0x298) {
        1 => m.w32(VIEW_LAYOUT, 0),
        2 if m.r32(VIEW_MODE) == 0 => {
            if view == 0 {
                m.w32(VIEW_LAYOUT, 1);
            } else if view == 1 {
                m.w32(VIEW_LAYOUT, 2);
            }
        }
        _ => {}
    }
}

/// A value eased toward a target by at most 0.005
fn ease(shown: F, target: F) -> F {
    let s = F(f32::from_bits(0x3ba3d70b));
    if shown.0 > (target + s).0 {
        shown - s
    } else if shown.0 < (target - s).0 {
        shown + s
    } else {
        target
    }
}

/// `cHUDOverlay::Update` 0x1a1f18: the HUD's flags; nothing more while an overlay is active. In time trials and
/// races the last ten seconds beep (the tenth spoken) for a player still racing; then for each player: the blink,
/// the score rolling up by 50s (a tick when its hundreds roll over, with the HUD's 0x10000 flag), the meter and the
/// multiplier eased, the meter's band colours (in the tutorial's fifth lesson the meter jumps and, from 20 up, all
/// bands lit), the wobble from a trick timer running, the speed flash over 3319.44, the progress bars and the
/// markers ahead eased by a 36th, the fade-in, the lap change, and the time trial's medal flashes
pub fn update(m: &mut Mem, env: &mut dyn HudEnv, hud: u32) {
    apply_hud_option(m, hud);
    let man = m.r32(race(m) + 0x38);
    if env.call(m, 0x1a77f8, &[man]) != 0 {
        return;
    }
    let mode = m.r32(GAME_MODE);
    if mode == 3 || mode == 5 {
        let last = f32::from_bits(m.r32(hud + 0x614)) as i32;
        let clock = f32::from_bits(m.r32(m.r32(race(m) + 0x1c) + 0x14)) as i32;
        if (clock as u32).wrapping_sub(1) < 10 && last.wrapping_sub(1) == clock {
            for i in 0..4u32 {
                let r = m.r32(race(m) + 0xa4 + 0x80 * i);
                if r != 0 && (m.r32(r + 0x418) as i32) < 0 {
                    if clock == 10 {
                        let a = m.r32(AUDIO);
                        env.call(m, 0x233ff0, &[a]);
                    }
                    let a = m.r32(AUDIO);
                    env.call(m, 0x216930, &[a]);
                    break;
                }
            }
        }
    }
    let clock = f32::from_bits(m.r32(m.r32(race(m) + 0x1c) + 0x14)) as i32;
    m.w32(hud + 0x614, (clock as f32).to_bits());
    let step = F(f32::from_bits(0x3c088889));
    let one = F(1.0);
    for i in 0..4u32 {
        let r = m.r32(race(m) + 0xa4 + 0x80 * i);
        if r == 0 {
            continue;
        }
        let b = hud + 0x114 + 0x140 * i;
        // the blink
        m.w32(b + 0xa0, 0);
        m.w32(b + 0xa4, 0);
        let c = (m.r32(b + 0x9c) as i32).wrapping_add(1) % 54;
        m.w32(b + 0x9c, c as u32);
        if m.r32(r + 0x428) == 19 || m.r32(r + 0x290) == 0 {
            if c >= 19 {
                m.w32(b + 0xa0, 1);
            }
        } else {
            m.w32(b + 0x9c, 0);
        }
        // the score
        let score = r + 0x5820;
        let pts = env.call(m, 0x155440, &[score]) as i32;
        let shown = m.r32(b + 4) as i32;
        if pts <= 0 || pts < shown {
            m.w32(b + 4, 0);
            m.w32(b + 8, 0);
        } else {
            let old = shown as f32;
            let v = if pts + 50 < shown {
                shown - 50
            } else if shown < pts - 50 {
                shown + 50
            } else {
                pts
            };
            m.w32(b + 4, v as u32);
            let base = env.call(m, 0x1554c0, &[score]);
            m.w32(b + 8, base);
            if m.r32(hud + 0x104) & 0x10000 != 0 && v % 100 < (old as i32) % 100 {
                let a = m.r32(AUDIO);
                env.call(m, 0x21a210, &[a]);
            }
        }
        // the meter and the multiplier
        let v = ease(fl(m, b + 0x7c), fl(m, r + 0x1c));
        put(m, b + 0x7c, v);
        let mul = env.multiplier(m, score);
        let v = ease(fl(m, b + 0x80), F(mul));
        put(m, b + 0x80, v);
        if m.r32(GAME_MODE) == 6 && m.r32(m.r32(race(m) + 0x34) + 0x50c) == 5 {
            m.w32(b + 0x7c, m.r32(r + 0x1c));
            if 20.0 <= fl(m, r + 0x24).0 {
                for k in 0..14 {
                    m.w32(b + 0x10 + 8 * k, 1);
                    m.w32(b + 0xc + 8 * k, m.r32(BANDS + 4 + 20 * k));
                }
            }
        }
        // the meter's bands
        let band = F(f32::from_bits(0x3d924925));
        let mut meter = fl(m, b + 0x7c);
        let mulv = fl(m, b + 0x80);
        for k in 0..14 {
            let t = if 0.0 < meter.0 {
                m.w32(b + 0x10 + 8 * k, 1);
                meter / band
            } else {
                m.w32(b + 0x10 + 8 * k, 0);
                (mulv + meter) / band
            };
            let t = if t.0 < 0.0 { F(0.0) } else if 1.0 < t.0 { one } else { t };
            let (unlit, lit) = (fl(m, BANDS + 20 * k), fl(m, BANDS + 4 + 20 * k));
            meter = meter - band;
            put(m, b + 0xc + 8 * k, t * lit + (one - t) * unlit);
        }
        // the wobble
        put(m, b + 0x84, one);
        m.w32(b + 0x88, 0);
        for k in 0..12 {
            let e = score + 348 + 20 * k;
            let kind = m.r32(e);
            if kind == 2 || kind == 3 {
                let w = wobble((fl(m, e + 12) / fl(m, e + 8)).0, f32::from_bits(0x3fb33333));
                put(m, b + 0x84, F(w) + one);
                put(m, b + 0x88, fl(m, e + 12) / fl(m, e + 8));
            }
        }
        // the speed flash
        let f = fl(m, b + 0x90);
        if 0.0 < f.0 {
            let n = f + step;
            put(m, b + 0x90, n);
            if 1.0 < n.0 {
                m.w32(b + 0x90, 0);
            }
        }
        let vel = [0, 4, 8, 12].map(|k| fl(m, r + 0x150 + k));
        let sq = vel.map(|x| x * x);
        let speed = (((sq[0] + sq[1]) + one * sq[2]) + one * sq[3]).sqrt();
        if f32::from_bits(0x454f771c) <= speed.0 {
            if m.r32(b + 0x8c) == 0 {
                put(m, b + 0x90, step);
                m.w32(b + 0x8c, 1);
            }
        } else {
            m.w32(b + 0x8c, 0);
        }
        // the progress bars and the markers ahead
        let p = env.progress(m, r);
        let mine = if (m.r32(r + 0x418) as i32) < 0 { F(p) } else { F(0.0) };
        let mut j = 0;
        while j < riders(m) {
            let o = env.call(m, 0x1814a8, &[race(m), j as u32]);
            let q = F(env.progress(m, o));
            let a = b + 0xc4 + 4 * j as u32;
            let bar = fl(m, a);
            put(m, a, bar - (bar - (q - mine)) / F(36.0));
            j += 1;
        }
        let mut k = 0;
        while k < m.r32(hud + 0x2c) as i32 {
            let a = b + 0xdc + 4 * k as u32;
            let s = fl(m, a);
            put(m, a, s - (s - (fl(m, hud + 0x30 + 4 * k as u32) - mine)) / F(36.0));
            k += 1;
        }
        // the fade-in, the lap
        if m.r32(r + 0x110) != 0 {
            m.w32(b, 0);
        } else {
            let a = fl(m, b);
            if a.0 < 1.0 {
                put(m, b, a + F(f32::from_bits(0x3bb60b61)));
            } else {
                put(m, b, one);
            }
        }
        if m.r32(r + 0x1b4) != m.r32(m.r32(r + 0x464) + 0x40) {
            m.w32(b + 0xa4, 1);
        }
        // the time trial's medal flashes
        let mode = m.r32(GAME_MODE);
        if mode == 3 || mode == 5 {
            let t = fl(m, b + 0x98);
            if 0.0 < t.0 {
                let n = t + step;
                put(m, b + 0x98, n);
                if 1.0 < n.0 {
                    m.w32(b + 0x98, 0);
                }
            } else {
                let x = m.r32(r + 0x5910) as i32;
                let st = m.r32(b + 0x94);
                let to = if !(x < m.r32(hud + 0xf8) as i32) && st == 2 {
                    Some(3)
                } else if !(x < m.r32(hud + 0xfc) as i32) && st == 1 {
                    Some(2)
                } else if !(x < m.r32(hud + 0x100) as i32) && st == 0 {
                    Some(1)
                } else {
                    None
                };
                if let Some(s) = to {
                    m.w32(b + 0x94, s);
                    put(m, b + 0x98, t + step);
                }
            }
        }
    }
}

/// `cHUDOverlay::Draw` 0x1a27d0 / `DrawText` 0x1a2a00: nothing while the replay's save page shows, results or the
/// pause menu are at their state 2, or a cut-scene other than 5, 7 or 8 runs; else each view scaled to 640 × 480
/// (the renderer's size as unsigned) and its player's sprites / text drawn (the text on layer 5)
pub fn draw(m: &mut Mem, env: &mut dyn HudEnv, hud: u32, text: bool) {
    let man = m.r32(race(m) + 0x38);
    let r = env.call(m, 0x1a7be0, &[man]);
    if m.r32(m.r32(r + 0x42c) + 0xc) != 0 {
        return;
    }
    let man = m.r32(race(m) + 0x38);
    let r = env.call(m, 0x1a7c88, &[man]);
    if m.r32(m.r32(r + 4) + 0x430) == 2 {
        return;
    }
    let man = m.r32(race(m) + 0x38);
    let r = env.call(m, 0x1a7c58, &[man]);
    if m.r32(r + 0x430) == 2 {
        return;
    }
    let scene = m.r32(race(m) + 0x34);
    if scene != 0 {
        let k = m.r32(scene + 0xc);
        if k != 5 && k != 7 && k != 8 {
            return;
        }
    }
    if text {
        env.renderer(m, 0x1d4, &[5]);
    }
    let u2f = |v: u32| -> F {
        if (v as i32) >= 0 {
            F(v as i32 as f32)
        } else {
            let h = F(((v >> 1) | (v & 1)) as i32 as f32);
            h + h
        }
    };
    let w = u2f(env.renderer(m, 0x5c, &[]));
    let sx = F(640.0) / w;
    let h = u2f(env.renderer(m, 0x64, &[]));
    let sy = F(480.0) / h;
    let func = if text { 0x1a4f70 } else { 0x1a2c40 };
    let mut i = 0u32;
    while (i as i32) < m.r32(race(m) + 0x298) as i32 {
        set_view_layout(m, i);
        let v = race(m) + 0x90 + 0x80 * i;
        let c = |k: u32| F(m.r32(v + k) as i32 as f32);
        let (x, y, ww, hh) = (c(4) * sy, c(0) * sx, c(8) * sx, c(12) * sy);
        env.draw_player(m, func, x.0, y.0, ww.0, hh.0, hud, i);
        i += 1;
    }
}

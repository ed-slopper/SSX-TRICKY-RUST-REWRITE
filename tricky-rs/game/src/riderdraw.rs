//! The rider's draw (board row G7g4): `BoarderRender_Draw` 0x1364d0 and the pieces of it that are its own,
//! `BoarderRender_PickLod` 0x136370, `BoarderRender_GetHandStreakPoints` 0x136c48,
//! `BoarderRender_CopyBoarderMatrix` 0x136e38 and `Career_IsPlayerOutfit6` 0x167038. On the bytes at their
//! addresses ([`Mem`]). The renderer's virtuals and the draw's other callees are calls out ([`RiderDrawEnv`]):
//! the skinning (`skin`), the shadow (`shadow`), the lights (`lights`), and the streaks, marker, icon and
//! effects (rows G7g5, G7e5), each checked on its own. Checked in the function runner (`tests/riderdraw.rs`).
//!
//! **The draw's object** (`br`): +0 the rider, +4 whether the shadow was made this frame, +8 the shadow, +0x20
//! and +0x200 the hand and board streaks (+0x1c4 their live count), +0x3e0 / +0x3e4 / +0x3e8 the effects, the
//! marker and the icon, +0x3f8 / +0x3fc the LODs, +0x400..+0x46f the lights (ambient, then three directions and
//! colours). **The rider** (as read here): +0x140 its place, +0x1a0 the light's way for its shadow, +0x1bc a
//! height, +0x418, +0x41c, +0x424 its kinds, +0x448 how visible it is (1 whole), +0x464 its part (+0x65 the
//! outfit, −1 none; +0x78 whether the board streak is off), +0x4494 its skeleton (+4 the matrices), +0x46b0
//! whether it is drawn, +0x46b8 its LODs, +0x4ad0 its box's centre, +0x4ae0 its bones, +0x5760 the board's
//! points (the first four of which are the board's matrix rows), +0x79c its skin.
//!
//! What the game builds on its stack, the port builds in `scratch` (STANDIN: the frame's place; 0x120 bytes at
//! the same offsets): +0 the streak points, +0x10 / +0x20 the box, +0x30 the shadow's centre, +0x40 the light
//! block, +0xc0 the shadow's up, +0xd0 / +0xe0 two colours.

use crate::animcurve::sin_cos;
use crate::animseq::{div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

type V = [F; 4];

pub const APP: u32 = 0x337c58;
/// 0x32b390: what `Career_IsPlayerOutfit6` is handed here
pub const CAREER: u32 = 0x32b390;
pub const GET_PLAYER_OUTFIT: u32 = 0x164dc0;
pub const STREAK_DRAW: u32 = 0x1f08d0;
pub const DRAW_EFFECTS: u32 = 0x135170;
pub const FOCUS_RIDER: u32 = 0x174008;
pub const MARKER_DRAW: u32 = 0x133c20;
pub const ICON_DRAW: u32 = 0x134830;
pub const COPY_BONES: u32 = 0x208400;
pub const LIGHTS_BUILD: u32 = 0x1ce140;
pub const SKIN_MATRICES: u32 = 0x207c30;
pub const SHADOW_UPDATE: u32 = 0x1c7e90;
pub const SET_BOARD_MATRIX: u32 = 0x207f18;
pub const SHADOW_DRAW: u32 = 0x1c8bc8;

/// The calls out. A float argument (f12) comes last in the words; a float answer is its bits.
pub trait RiderDrawEnv {
    /// A function with these words; its answer
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// A renderer virtual (the function found in the vtable) with these words; its answer
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rq(m: &Mem, a: u32) -> V {
    [0, 1, 2, 3].map(|k| rf(m, a.wrapping_add(4 * k)))
}
fn wq(m: &mut Mem, a: u32, v: V) {
    for k in 0..4 {
        wf(m, a + 4 * k, v[k as usize]);
    }
}
fn copy(m: &mut Mem, to: u32, from: u32, bytes: u32) {
    for k in 0..bytes / 4 {
        let w = m.r32(from.wrapping_add(4 * k));
        m.w32(to + 4 * k, w);
    }
}
fn vcall(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let f = m.r32(vt + off + 4);
    env.vcall(m, f, this, args)
}
fn bits(x: f32) -> u32 {
    x.to_bits()
}
const BIAS: f32 = 0.08;

/// `Career_IsPlayerOutfit6` 0x167038(career): whether `Career_GetPlayerOutfit` 0x164dc0 answers 6
pub fn is_player_outfit6(m: &mut Mem, env: &mut dyn RiderDrawEnv, career: u32) -> u32 {
    (env.call(m, GET_PLAYER_OUTFIT, &[career]) == 6) as u32
}

/// `BoarderRender_CopyBoarderMatrix` 0x136e38(br, out): the rider's +0x5760..+0x579f to `out`
pub fn copy_boarder_matrix(m: &mut Mem, br: u32, out: u32) {
    for k in 0..4 {
        let from = m.r32(br) + 0x5760 + 0x10 * k;
        copy(m, out + 0x10 * k, from, 0x10);
    }
}

/// `BoarderRender_GetHandStreakPoints` 0x136c48(br, out): for each hand, its bone's place moved 15.0157 on
/// along the arm (from the bone before; the arm's length over all four lanes, `vrsqrt`), w 1, then the elbow's
/// place: bones 8 from 7 with 6, then 12 from 11 with 10. A rider with no skeleton reads from 0.
pub fn get_hand_streak_points(m: &mut Mem, br: u32, out: u32) {
    let skel = m.r32(m.r32(br) + 0x4494);
    let mats = if skel == 0 { 0 } else { m.r32(skel + 4) };
    for (k, (hand, arm, elbow)) in [(0x230u32, 0x1f0u32, 0x1b0u32), (0x330, 0x2f0, 0x2b0)].into_iter().enumerate() {
        let a = rq(m, mats.wrapping_add(hand));
        let b = rq(m, mats.wrapping_add(arm));
        let d: V = [0, 1, 2, 3].map(|j| a[j] - b[j]);
        let sq = d.map(|x| x * x);
        let one = F(1.0);
        let len2 = ((sq[0] + sq[1]) + one * sq[2]) + one * sq[3];
        let q = div(one, sqrt(len2));
        let reach = F(f32::from_bits(0x4170_040e));
        let p: V = [0, 1, 2, 3].map(|j| a[j] + (d[j] * q) * reach);
        let at = out + 0x20 * k as u32;
        wq(m, at, [p[0], p[1], p[2], one]);
        let e = rq(m, mats.wrapping_add(elbow));
        wq(m, at + 0x10, e);
    }
}

/// `BoarderRender_PickLod` 0x136370(br): the rider's place (+0x140) through the renderer's view (+0xf8, its
/// rows by the place's lanes); its depth × tan(the field of view, +0xc8) × 0.05: below 26.02 LOD 0 for both,
/// below 45.18 1 for both, else 2 and 1; each then at most the rider's LODs (+0x46b8) less 2
pub fn pick_lod(m: &mut Mem, env: &mut dyn RiderDrawEnv, br: u32) {
    let r = m.r32(m.r32(APP) + 0x724);
    let view = vcall(m, env, r, 0xf8, &[]);
    let p = rq(m, m.r32(br) + 0x140);
    let rows = [0, 1, 2, 3].map(|i| rq(m, view.wrapping_add(0x10 * i)));
    let z = ((rows[0][2] * p[0] + rows[1][2] * p[1]) + rows[2][2] * p[2]) + rows[3][2] * p[3];
    let fov = F(f32::from_bits(vcall(m, env, r, 0xc8, &[])));
    let (s, c) = sin_cos(fov);
    let d = z * div(s, c) * F(f32::from_bits(0x3d4c_cccd));
    let (a, b) = if d < F(f32::from_bits(0x41d0_2902)) {
        (0, 0)
    } else if d < F(f32::from_bits(0x4234_bb19)) {
        (1, 1)
    } else {
        (2, 1)
    };
    m.w32(br + 0x3f8, a);
    m.w32(br + 0x3fc, b);
    let most = (m.r32(m.r32(br) + 0x46b8) as i32).wrapping_sub(2);
    if most < m.r32(br + 0x3f8) as i32 {
        m.w32(br + 0x3f8, most as u32);
    }
    if most < m.r32(br + 0x3fc) as i32 {
        m.w32(br + 0x3fc, most as u32);
    }
}

/// `BoarderRender_Draw` 0x1364d0(br): the rider, its streaks, effects, marker and icon, and its shadow.
/// - Nothing when the rider is not drawn (+0x46b0), the game's mode (app +0x730 → +0x1c → +0x1c) is 6, or the
///   rider is not visible at all (+0x448 = 0).
/// - The hand streaks when two or more are live, from [`get_hand_streak_points`]; the board's when two or more
///   are live and the part's +0x78 is 0, from [`copy_boarder_matrix`]; then the effects.
/// - The marker (a race of two or more players, its kind 2+, the rider's +0x41c 1, and the camera not on this
///   rider); the icon unless +0x41c is 1.
/// - Nothing more when the box test (the centre ± 70, or ± 100 when +0x418 ≥ 0; z −50 .. +150) says it is out.
/// - The LOD, the bones copied, the outfit-6 flag (+0x2b8), the lights (+0xb8 at the place with −35.02 after),
///   and the skin: once with blend 3, or, when the rider is fading (+0x448 below 0.95), twice: blend 1 then 3
///   with +0x1c8(1, 0, 0.08). +0xb8 again with 0.
/// - Once a frame (+4), the shadow: made about the mid-point of bones 15 and 18 with up (0, 0, −1), under the
///   renderer's state for it, and cast on the rider (`RiderSkin_SetBoardMatrix` with colours (1, k, k, k) and
///   (1, 0.3, 0.3, 0.3), k = 0.3 − 0.3 × h / −5, h the rider's height +0x1bc held to −5..0 when +0x424 is 2).
///   Every draw then draws the shadow, and the blend goes back to 3.
pub fn draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, br: u32, scratch: u32) {
    let rider = m.r32(br);
    if m.r32(rider + 0x46b0) == 0 {
        return;
    }
    let app = m.r32(APP);
    if m.r32(m.r32(m.r32(app + 0x730) + 0x1c) + 0x1c) == 6 {
        return;
    }
    if rf(m, rider + 0x448) == F(0.0) {
        return;
    }
    let r = m.r32(app + 0x724);
    if !((m.r32(br + 0x20 + 0x1c4) as i32) < 2) {
        get_hand_streak_points(m, br, scratch);
        env.call(m, STREAK_DRAW, &[br + 0x20, scratch]);
    }
    if !((m.r32(br + 0x200 + 0x1c4) as i32) < 2) && m.r32(m.r32(m.r32(br) + 0x464) + 0x78) == 0 {
        copy_boarder_matrix(m, br, scratch);
        env.call(m, STREAK_DRAW, &[br + 0x200, scratch]);
    }
    let effects = m.r32(br + 0x3e0);
    env.call(m, DRAW_EFFECTS, &[effects]);
    let game = m.r32(m.r32(APP) + 0x730);
    if !((m.r32(game + 0x298) as i32) < 2) && !((m.r32(m.r32(game + 0x1c) + 0x7c) as i32) < 2) && m.r32(m.r32(br) + 0x41c) == 1 {
        let k = m.r32(game + 0x290) << 7;
        let who = m.r32(game.wrapping_add(k) + 0xa0);
        if env.call(m, FOCUS_RIDER, &[who]) != m.r32(br) {
            let marker = m.r32(br + 0x3e4);
            env.call(m, MARKER_DRAW, &[marker]);
        }
    }
    if m.r32(m.r32(br) + 0x41c) != 1 {
        let icon = m.r32(br + 0x3e8);
        env.call(m, ICON_DRAW, &[icon]);
    }
    let rider = m.r32(br);
    let c = rq(m, rider + 0x4ad0);
    let reach = if (m.r32(rider + 0x418) as i32) < 0 { F(70.0) } else { F(100.0) };
    let one = F(1.0);
    wq(m, scratch + 0x10, [c[0] - reach, c[1] - reach, c[2] + F(-50.0), one]);
    wq(m, scratch + 0x20, [c[0] + reach, c[1] + reach, c[2] + F(150.0), one]);
    if vcall(m, env, r, 0x280, &[scratch + 0x10, scratch + 0x20]) == 1 {
        return;
    }
    pick_lod(m, env, br);
    let skin = m.r32(br) + 0x79c;
    env.call(m, COPY_BONES, &[skin]);
    let part = m.r32(m.r32(br) + 0x464);
    let flag = if m.r8(part + 0x65) as i8 != -1 { is_player_outfit6(m, env, CAREER) } else { 0 };
    let r = m.r32(m.r32(APP) + 0x724);
    vcall(m, env, r, 0x2b8, &[flag]);
    let lights = scratch + 0x40;
    env.call(m, LIGHTS_BUILD, &[lights, br + 0x400, br + 0x440, br + 0x410, br + 0x450, br + 0x420, br + 0x460, br + 0x430]);
    let place = m.r32(br) + 0x140;
    vcall(m, env, r, 0xb8, &[place, bits(f32::from_bits(0xc20c_1769))]);
    let skin_draw = |m: &mut Mem, env: &mut dyn RiderDrawEnv| {
        let skin = m.r32(br) + 0x79c;
        let lod = m.r32(br + 0x3f8);
        env.call(m, SKIN_MATRICES, &[skin, lod]);
    };
    if rf(m, m.r32(br) + 0x448) < F(f32::from_bits(0x3f73_3333)) {
        vcall(m, env, r, 0x218, &[lights]);
        vcall(m, env, r, 0x1e0, &[1]);
        vcall(m, env, r, 0x1e8, &[0, 0]);
        skin_draw(m, env);
        vcall(m, env, r, 0x1c8, &[1, 0, bits(BIAS)]);
        vcall(m, env, r, 0x1e0, &[3]);
        vcall(m, env, r, 0x1e8, &[2, 0]);
        skin_draw(m, env);
        vcall(m, env, r, 0x1e8, &[0, 0]);
        vcall(m, env, r, 0x1c8, &[0, 2, bits(BIAS)]);
    } else {
        vcall(m, env, r, 0x218, &[lights]);
        vcall(m, env, r, 0x1e0, &[3]);
        skin_draw(m, env);
    }
    let place = m.r32(br) + 0x140;
    vcall(m, env, r, 0xb8, &[place, bits(0.0)]);
    if m.r32(m.r32(APP) + 0x730) == 0 {
        vcall(m, env, r, 0x1e0, &[3]);
        return;
    }
    if m.r32(br + 4) == 0 {
        m.w32(br + 4, 1);
        let rider = m.r32(br);
        let skel = m.r32(rider + 0x4494);
        let mats = if skel == 0 { 0 } else { m.r32(skel + 4) };
        let (a, b) = (rq(m, mats.wrapping_add(0x3f0)), rq(m, mats.wrapping_add(0x4b0)));
        let mid: V = [0, 1, 2, 3].map(|j| (a[j] + b[j]) * F(0.5));
        for j in 0..3 {
            wf(m, scratch + 0x30 + 4 * j as u32, mid[j]);
        }
        wq(m, scratch + 0xd0, mid);
        let z = F(0.0);
        for (j, v) in [z, z, F(-1.0)].into_iter().enumerate() {
            wf(m, scratch + 0xc0 + 4 * j as u32, v);
        }
        let shadow = m.r32(br + 8);
        env.call(m, SHADOW_UPDATE, &[shadow, scratch + 0x30, rider + 0x1a0, scratch + 0xc0, rider + 0x4ae0, rider + 0x5760]);
        let state = vcall(m, env, r, 0x1d8, &[]);
        vcall(m, env, r, 0x1d0, &[1]);
        vcall(m, env, r, 0x1c8, &[2, 0, bits(BIAS)]);
        vcall(m, env, r, 0x1c0, &[1]);
        vcall(m, env, r, 0x1e0, &[0]);
        let tex = m.r32(m.r32(br + 8) + 0x170);
        vcall(m, env, r, 0x178, &[tex]);
        wq(m, scratch + 0xd0, [z; 4]);
        let tex = m.r32(m.r32(br + 8) + 0x170);
        vcall(m, env, r, 0x188, &[tex, scratch + 0xd0]);
        let rider = m.r32(br);
        let mut h = z;
        if m.r32(rider + 0x424) == 2 {
            let y = rf(m, rider + 0x1bc);
            if !(z <= y) {
                h = if y < F(-5.0) { F(-5.0) } else { y };
            }
        }
        let k = F(f32::from_bits(0x3e99_999a));
        let shade = k - div(h, F(-5.0)) * k;
        wq(m, scratch + 0xe0, [one, k, k, k]);
        wq(m, scratch + 0xd0, [one, shade, shade, shade]);
        let shadow = m.r32(br + 8);
        env.call(m, SET_BOARD_MATRIX, &[rider + 0x79c, shadow, scratch + 0xd0, scratch + 0xe0]);
        vcall(m, env, r, 0x180, &[]);
        vcall(m, env, r, 0x1c8, &[0, 2, bits(BIAS)]);
        vcall(m, env, r, 0x1c0, &[0]);
        vcall(m, env, r, 0x1d0, &[state]);
    }
    let shadow = m.r32(br + 8);
    env.call(m, SHADOW_DRAW, &[shadow]);
    vcall(m, env, r, 0x1e0, &[3]);
}

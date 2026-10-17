//! The rider's effect draws (board row G7e5): `BoarderRender_DrawEffects` 0x135170 and the draws it runs that
//! are not elsewhere: the board's puffs (`Fx_BoardPuff_Draw` 0x12f178), its trail (`Fx_BoardTrail_Draw`
//! 0x130b98) and the brake spray's sprites (`Fx_Spray_Draw` 0x12e918). On the bytes at their addresses
//! ([`Mem`]); the renderer's virtuals and the other draws are calls out (`riderdraw::RiderDrawEnv`). Checked in
//! the function runner (`tests/boardfx.rs`).
//!
//! What the game builds on its stack for the renderer, the port builds in `scratch` (STANDIN: the frame's place;
//! the same bytes): a sprite's place and size (0x20 bytes), the trail's vertices (0x1da0 bytes).

use crate::animseq::div;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::riderdraw::{RiderDrawEnv, APP};
use crate::sndlib::cvt_w;

pub const PUFF_DRAW: u32 = 0x12f178;
pub const TRAIL_DRAW: u32 = 0x130b98;
pub const SURFACE_SPRAY_DRAW: u32 = 0x132308;
pub const SPRAY_DRAW: u32 = 0x12e918;
pub const SPEED_DUST_DRAW: u32 = 0x12fb40;
pub const BOARD_TRACK_DRAW: u32 = 0x1331a8;
pub const SPRAY_PARTICLES_DRAW: u32 = 0x12d890;
pub const VEC4_SCALE: u32 = 0x102f50;
const BIAS: u32 = 0x3da3_d70a;

type V = [F; 4];

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
fn f(bits: u32) -> F {
    F(f32::from_bits(bits))
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}
fn texture(m: &Mem, k: u32) -> u32 {
    m.r32(m.r32(m.r32(APP) + 0x30).wrapping_add(k << 2))
}
fn vcall(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(r + 0x10);
    let this = r.wrapping_add(m.r16(vt + off) as i16 as i32 as u32);
    let func = m.r32(vt + off + 4);
    env.vcall(m, func, this, args)
}
fn byte(x: F) -> u32 {
    cvt_w((x * F(128.0)).0) as u32
}

/// `BoarderRender_DrawEffects` 0x135170(fx): the board's puffs (+0), its trail (+0x1c20), the ground spray
/// (+0x4b60), the brake spray (+0x99b0), the speed dust (+0xb600); then the board's track (+0x8020), unless the
/// race has two or more players and its mode's +0x88 is 3 or more
pub fn draw_effects(m: &mut Mem, env: &mut dyn RiderDrawEnv, fx: u32) {
    env.call(m, PUFF_DRAW, &[fx]);
    env.call(m, TRAIL_DRAW, &[fx + 0x1c20]);
    env.call(m, SURFACE_SPRAY_DRAW, &[fx + 0x4b60]);
    env.call(m, SPRAY_DRAW, &[fx + 0x99b0]);
    env.call(m, SPEED_DUST_DRAW, &[fx + 0xb600]);
    let game = m.r32(m.r32(APP) + 0x730);
    if !((m.r32(game + 0x298) as i32) < 2) && !((m.r32(m.r32(game + 0x1c) + 0x88) as i32) < 3) {
        return;
    }
    env.call(m, BOARD_TRACK_DRAW, &[fx + 0x8020]);
}

/// The draws' shared state: +0x1c8(0, 0, 0.08), +0x1c0(1), blend 5, +0x1e8(2, 0); and back: +0x1c8(0, 2, 0.08),
/// +0x1c0(0), +0x1e8(0, 0), blend 3
fn sprite_state(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, on: bool) {
    if on {
        vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
        vcall(m, env, r, 0x1c0, &[1]);
        vcall(m, env, r, 0x1e0, &[5]);
        vcall(m, env, r, 0x1e8, &[2, 0]);
    } else {
        vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
        vcall(m, env, r, 0x1c0, &[0]);
        vcall(m, env, r, 0x1e8, &[0, 0]);
        vcall(m, env, r, 0x1e0, &[3]);
    }
}

/// A sprite handed to the renderer's +0x240: its place `base` + `dir` × `k` (`Vec4_Scale`), its size (size,
/// size, and the scaled vector's z and w), and three words of the record
fn sprite(m: &mut Mem, env: &mut dyn RiderDrawEnv, r: u32, base: u32, dir: u32, k: F, size: F, words: [u32; 3], scratch: u32) {
    let d = rq(m, dir).map(|x| x * k);
    let b = rq(m, base);
    wq(m, scratch, [0, 1, 2, 3].map(|j| b[j] + d[j]));
    wq(m, scratch + 0x10, [size, size, d[2], d[3]]);
    vcall(m, env, r, 0x240, &[scratch, scratch + 0x10, words[0], words[1], words[2]]);
}

/// `Fx_BoardPuff_Draw` 0x12f178(puffs): each puff of the ring (64 of 0x70 bytes from +0x10; +0x1c10 how many,
/// +0x1c14 the oldest) not past its life (+0x5c against its age +0) a sprite: the texture 0x18 + its +0x40 in
/// the app's table; the size +0x64 × age / life, at least +0x60; the alpha (put at +0x48) +0x44 × age / +0x58
/// while younger than +0x58, else +0x44 × (life − age) / (life − +0x58); the place +0x20 + +0x30 × half the size;
/// the record's +4, +0xc and the alpha's place. The count is read again after each drawn puff.
pub fn puff_draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, puffs: u32, scratch: u32) {
    let r = renderer(m);
    sprite_state(m, env, r, true);
    let mut n = m.r32(puffs + 0x1c10) as i32;
    let mut i = 0i32;
    while i < n {
        let k = (m.r32(puffs + 0x1c14) as i32).wrapping_add(i).wrapping_rem(64);
        let p = puffs.wrapping_add(k.wrapping_mul(0x70).wrapping_add(0x10) as u32);
        i += 1;
        if rf(m, p + 0x5c) < rf(m, p) {
            continue;
        }
        let tex = texture(m, m.r32(p + 0x40).wrapping_add(0x18));
        vcall(m, env, r, 0x1f0, &[tex, 0]);
        let (life, age) = (rf(m, p + 0x5c), rf(m, p));
        let grown = div(rf(m, p + 0x64), life) * age;
        let least = rf(m, p + 0x60);
        let size = if grown < least { least } else { grown };
        let fade = rf(m, p + 0x58);
        let alpha = if age < fade { div(rf(m, p + 0x44) * age, fade) } else { div((life - age) * rf(m, p + 0x44), life - fade) };
        wf(m, p + 0x48, alpha);
        sprite(m, env, r, p + 0x20, p + 0x30, size * F(0.5), size, [p + 4, p + 0xc, p + 0x48], scratch);
        n = m.r32(puffs + 0x1c10) as i32;
    }
    sprite_state(m, env, r, false);
}

/// `Fx_Spray_Draw` 0x12e918(spray): its ribbons (`Fx_Spray_DrawParticles` 0x12d890 on +0x970), then each entry
/// of the brake spray's pool (30 of 0x50 bytes; +0x960 how many, +0x964 the oldest; `brakespray`) a sprite: the
/// texture 0x18 + its +0x44; by its age t (+0x40): below 0.2, the alpha (put at +0x20) t × +0x48 / 0.2 and the
/// size t × 120 / 0.2; else the size 120 + (t − 0.2) × 130 / 1.8 and the alpha k (0.4 k + 0.6) × +0x48 for
/// k = (2 − t) / 1.8; the place +0 + +0x10 × 0.9 × the size; the record's +0x30, +0x38 and the alpha's place.
pub fn spray_draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, spray: u32, scratch: u32) {
    env.call(m, SPRAY_PARTICLES_DRAW, &[spray + 0x970]);
    let r = renderer(m);
    sprite_state(m, env, r, true);
    let mut i = 0i32;
    while i < m.r32(spray + 0x960) as i32 {
        let k = (m.r32(spray + 0x964) as i32).wrapping_add(i).wrapping_rem(30);
        let e = spray.wrapping_add(k.wrapping_mul(0x50) as u32);
        let tex = texture(m, m.r32(e + 0x44).wrapping_add(0x18));
        vcall(m, env, r, 0x1f0, &[tex, 0]);
        let t = rf(m, e + 0x40);
        let fifth = f(0x3e4c_cccd);
        let (alpha, size) = if t < fifth {
            (div(t * rf(m, e + 0x48), fifth), div(t * F(120.0), fifth))
        } else {
            let k = (F(2.0) - t) * f(0x3f0e_38e4);
            let grow = div((t - fifth) * F(130.0), f(0x3fe6_6666));
            let a = k * (k * f(0x3ecc_cccd) + f(0x3f19_999a)) * rf(m, e + 0x48);
            (a, grow + F(120.0))
        };
        wf(m, e + 0x20, alpha);
        i += 1;
        sprite(m, env, r, e, e + 0x10, size * f(0x3f66_6666), size, [e + 0x30, e + 0x38, e + 0x20], scratch);
    }
    sprite_state(m, env, r, false);
}

/// A trail vertex at `at`: u, v, 1; the colour (four integers); the place
fn vertex(m: &mut Mem, at: u32, u: F, v: F, colour: [u32; 4], place: V) {
    wf(m, at, u);
    wf(m, at + 4, v);
    wf(m, at + 8, F(1.0));
    for (k, c) in colour.into_iter().enumerate() {
        m.w32(at + 0x10 + 4 * k as u32, c);
    }
    wq(m, at + 0x20, place);
}

/// `Fx_BoardTrail_Draw` 0x130b98(trail): nothing with no points (+0x50). Else with texture 3, +0x1f8(0, 0),
/// blend 5, +0x1c0(1), +0x1c8(0, 0, 0.08), +0x1e8(2, 0):
/// - **The cap**: a strip of four pairs, the board's point (+0x20; u the newest's +0x90 + 1, v 0.5, alpha 0) to
///   each of the newest entry's four places (u its +0x90, v 0.98 − 0.32 k), to the renderer's +0x220.
/// - **The ribbon**: for each of the three bands between the places k − 1 and k, each entry but the last (the
///   ring of 75 of 0xa0 bytes from +0x60, from the oldest +0x54) two vertices (v 0.98 − 0.32 (k − 1) and
///   0.98 − 0.32 k); an entry's break (+0x9c) draws what is there (3 or more) and starts again, as does the
///   band's end; each band starts anew.
///
/// The colour is the entry's +0x84, +0x88, +0x8c × 128 and alpha +0x80 × (0.9 − +0x94) / 0.9 × 128, all as
/// integers. Then +0x1c0(0), +0x1c8(0, 2, 0.08), +0x1e8(0, 0), blend 3.
pub fn trail_draw(m: &mut Mem, env: &mut dyn RiderDrawEnv, trail: u32, scratch: u32) {
    if m.r32(trail + 0x50) == 0 {
        return;
    }
    let r = renderer(m);
    let tex = texture(m, 3);
    vcall(m, env, r, 0x1f0, &[tex, 0]);
    vcall(m, env, r, 0x1f8, &[0, 0]);
    vcall(m, env, r, 0x1e0, &[5]);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let step = f(0x3ea3_d70a);
    let top = f(0x3f7a_e148);
    let colour = |m: &Mem, e: u32| {
        let a = rf(m, e + 0x80) * ((f(0x3f66_6666) - rf(m, e + 0x94)) * f(0x3f8e_38e4));
        [byte(rf(m, e + 0x84)), byte(rf(m, e + 0x88)), byte(rf(m, e + 0x8c)), byte(a)]
    };
    let entry = |k: i32| trail.wrapping_add(k.wrapping_mul(0xa0).wrapping_add(0x60) as u32);
    let e0 = entry(m.r32(trail + 0x54) as i32);
    let c = colour(m, e0);
    let centre = rq(m, trail + 0x20);
    let u = rf(m, e0 + 0x90);
    for k in 0..4u32 {
        let v = top - crate::animseq::cvt_s_w(k as i32) * step;
        vertex(m, scratch + 0x60 * k, u + F(1.0), F(0.5), [c[0], c[1], c[2], 0], centre);
        let place = rq(m, e0 + 0x10 * k);
        vertex(m, scratch + 0x60 * k + 0x30, u, v, c, place);
    }
    vcall(m, env, r, 0x220, &[8, scratch, 0]);
    let body = scratch + 0x180;
    for band in 1..4i32 {
        let mut count = 0u32;
        let mut i = 0i32;
        while i < (m.r32(trail + 0x50) as i32).wrapping_sub(1) {
            let k = (m.r32(trail + 0x54) as i32).wrapping_add(i).wrapping_rem(75);
            let e = entry(k);
            let v0 = top - crate::animseq::cvt_s_w(band - 1) * step;
            let v1 = top - crate::animseq::cvt_s_w(band) * step;
            let c = colour(m, e);
            let u = rf(m, e + 0x90);
            let a = rq(m, e.wrapping_add(((band - 1) as u32) << 4));
            let b = rq(m, e.wrapping_add((band as u32) << 4));
            vertex(m, body + 0x30 * count, u, v0, c, a);
            vertex(m, body + 0x30 * (count + 1), u, v1, c, b);
            count += 2;
            if m.r32(e + 0x9c) != 0 {
                if count >= 3 {
                    vcall(m, env, r, 0x220, &[count, body, 0]);
                }
                count = 0;
            }
            i += 1;
        }
        if count >= 3 {
            vcall(m, env, r, 0x220, &[count, body, 0]);
        }
    }
    vcall(m, env, r, 0x1c0, &[0]);
    vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
    vcall(m, env, r, 0x1e8, &[0, 0]);
    vcall(m, env, r, 0x1e0, &[3]);
}

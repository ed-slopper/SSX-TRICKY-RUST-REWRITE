//! The rider's effects' draws (board row G7e4): the ground spray (`Fx_SurfaceSpray_Draw` 0x132308), the speed
//! dust (`Fx_SpeedDust_Draw` 0x12fb40), the spray's ribbons (`Fx_Spray_DrawParticles` 0x12d890 with
//! `Fx_Spray_ComputeParticle` 0x12d6c0), and the turned sprite quads (`Sprite_DrawRotatedQuad` 0x19b8c8,
//! `Sprite_DrawRotatedTexQuad` 0x19bb48). Each sets the renderer's state, hands it what to draw, and puts the
//! state back. The effects' updates are `surfacespray`, `puff` and the spray's own. On the bytes at their
//! addresses ([`Mem`]); the renderer's virtuals are calls out ([`ParticleDrawEnv`]). Checked in the function
//! runner (`tests/fxdraw.rs`).
//!
//! The game builds the vertices, corners and boxes it hands the renderer on its stack. The port builds them in
//! `scratch` (STANDIN: the frame's place; the same bytes at the same offsets, bar the words the game leaves
//! unwritten): 0x20 bytes for the dust, 0xb30 for the spray, 0xc0 for a quad.
//!
//! **A vertex** (0x30 bytes, as the renderer's +0x220 takes them): +0 u, v, 1.0 (+0xc unwritten); +0x10 the
//! colour as four integers; +0x20 the place (x, y, z, w).

use crate::animseq::{cvt_s_w, div};
use crate::math;
use crate::particledraw::{vcall, ParticleDrawEnv, APP, BIAS};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::sndlib::cvt_w;

type V = [F; 4];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn rq(m: &Mem, a: u32) -> V {
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wq(m: &mut Mem, a: u32, v: V) {
    for k in 0..4 {
        wf(m, a + 4 * k, v[k as usize]);
    }
}
fn add(a: V, b: V) -> V {
    [0, 1, 2, 3].map(|k| a[k] + b[k])
}
/// `Vec4_Scale` 0x102f50 and `vmulx`: every lane times `s`
fn scale(a: V, s: F) -> V {
    a.map(|x| x * s)
}
fn renderer(m: &Mem) -> u32 {
    m.r32(m.r32(APP) + 0x724)
}
fn texture(m: &Mem, k: u32) -> u32 {
    let table = m.r32(m.r32(APP) + 0x30);
    m.r32(table.wrapping_add(k << 2))
}
fn f(bits: u32) -> F {
    F(f32::from_bits(bits))
}

/// A vertex at `at`: u, v, 1.0; the colour; the place
fn vertex(m: &mut Mem, at: u32, u: F, v: F, colour: [u32; 4], place: V) {
    wf(m, at, u);
    wf(m, at + 4, v);
    wf(m, at + 8, F(1.0));
    for (k, c) in colour.into_iter().enumerate() {
        m.w32(at + 0x10 + 4 * k as u32, c);
    }
    wq(m, at + 0x20, place);
}

/// The state the effects' draws put back: the depth bias (+0x1c8(0, 2, 0.08)), +0x1c0(0), +0x1e8(0, 0) and the
/// blend 3, in the order `first` says (the spray's draw does +0x1c0 first)
fn put_back(m: &mut Mem, env: &mut dyn ParticleDrawEnv, r: u32, c0_first: bool) {
    if c0_first {
        vcall(m, env, r, 0x1c0, &[0]);
        vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
    } else {
        vcall(m, env, r, 0x1c8, &[0, 2, BIAS]);
        vcall(m, env, r, 0x1c0, &[0]);
    }
    vcall(m, env, r, 0x1e8, &[0, 0]);
    vcall(m, env, r, 0x1e0, &[3]);
}

/// `Fx_SurfaceSpray_Draw` 0x132308(pool): each live emitter of the ring (40 of 0x150 bytes from +0x40, the
/// count at +0, the oldest at +4; live when its word 0, the block's count, is not 0) drawn by the renderer's
/// +0x258 (VU1 program 6) with its sprite (+0x140) and blend (+0x144). The count is read again after each draw.
pub fn surface_spray_draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, pool: u32) {
    let r = renderer(m);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let mut n = m.r32(pool) as i32;
    let mut i = 0i32;
    while i < n {
        let k = (m.r32(pool + 4) as i32).wrapping_add(i).wrapping_rem(40);
        let slot = pool.wrapping_add(k.wrapping_mul(0x150).wrapping_add(0x40) as u32);
        if m.r32(slot) != 0 {
            let sprite = m.r32(slot + 0x140);
            vcall(m, env, r, 0x1f0, &[sprite, 0]);
            let blend = m.r32(slot + 0x144);
            vcall(m, env, r, 0x1e0, &[blend]);
            vcall(m, env, r, 0x258, &[slot]);
            n = m.r32(pool) as i32;
        }
        i += 1;
    }
    put_back(m, env, r, false);
}

/// `Fx_SpeedDust_Draw` 0x12fb40(dust): each puff of the ring (64 of 0x34 bytes from +0x50; the count +0xd50, the
/// oldest +0xd54) not past its life (+0x28 against its age +0) drawn by the renderer's +0x240 as a sprite:
/// - its texture: the app's table at 0x18 + the puff's +0x1c;
/// - its size: +0x30 × age / life, at least +0x2c;
/// - its alpha (put at the dust's +0x40): +0x20 × age / +0x24 while younger than +0x24, else +0x20 × (life −
///   age) / (life − +0x24);
/// - its place: the dust's +0 + its +0x10 × (+0x14 − 200 age) + its +0x20 × +0x18 + its +0x30 × half the size.
///
/// +0x240 gets the place, (size, size, …), the puff's +4 and +0xc, and the alpha's place.
pub fn speed_dust_draw(m: &mut Mem, env: &mut dyn ParticleDrawEnv, dust: u32, scratch: u32) {
    let r = renderer(m);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1e0, &[5]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let mut n = m.r32(dust + 0xd50) as i32;
    let mut i = 0i32;
    while i < n {
        let k = (m.r32(dust + 0xd54) as i32).wrapping_add(i).wrapping_rem(64);
        let p = dust.wrapping_add(k.wrapping_mul(0x34).wrapping_add(0x50) as u32);
        i += 1;
        if rf(m, p + 0x28) < rf(m, p) {
            continue;
        }
        let tex = texture(m, m.r32(p + 0x1c).wrapping_add(0x18));
        vcall(m, env, r, 0x1f0, &[tex, 0]);
        let (life, age) = (rf(m, p + 0x28), rf(m, p));
        let grown = div(rf(m, p + 0x30), life) * age;
        let least = rf(m, p + 0x2c);
        let size = if grown < least { least } else { grown };
        let fade = rf(m, p + 0x24);
        let alpha = if age < fade { div(rf(m, p + 0x20) * age, fade) } else { div((life - age) * rf(m, p + 0x20), life - fade) };
        wf(m, dust + 0x40, alpha);
        let along = rf(m, p + 0x14) - rf(m, p) * F(200.0);
        let a = add(rq(m, dust), scale(rq(m, dust + 0x10), along));
        let b = add(a, scale(rq(m, dust + 0x20), rf(m, p + 0x18)));
        let place = add(b, scale(rq(m, dust + 0x30), size * F(0.5)));
        wq(m, scratch, place);
        wq(m, scratch + 0x10, [size, size, b[2], b[3]]);
        vcall(m, env, r, 0x240, &[scratch, scratch + 0x10, p + 4, p + 0xc, dust + 0x40]);
        n = m.r32(dust + 0xd50) as i32;
    }
    put_back(m, env, r, false);
}

/// `Fx_Spray_ComputeParticle` 0x12d6c0(out, spray, entry, k, t): the entry's k-th point at t. It is the
/// entry's place (+0x320) plus the point's offset (+16k), plus its velocity (+0x190 + 16k) × t × (1 − 1/p⁴),
/// plus a fall of (0, 0, −55.56) × t × (0.2 t + 0.8) × age (+0x3b8). p is the game's polynomial in 1.25 age
/// (1 + x/4 + x²/32 + …, so 1/p⁴ falls from 1 much as e^−x does).
pub fn spray_compute_particle(m: &Mem, e: u32, k: u32, t: F) -> V {
    let age = rf(m, e + 0x3b8);
    let x = age * F(1.25);
    let c = [0x3539_61ac, 0x36b6_3510, 0x3933_e553, 0x3b29_d3ff, 0x3d00_07f4, 0x3e7f_ffa8].map(f);
    let mut p = x * c[0];
    for &ck in &c[1..] {
        p = p + ck;
        p = x * p;
    }
    let p = p + F(1.0);
    let p = p * p;
    let p = p * p;
    let ease = F(1.0) - div(F(1.0), p);
    let off = k << 4;
    let vel = scale(rq(m, e + 0x190 + off), t);
    let fall = scale(scale([F(0.0), F(0.0), f(0xc25e_38e3), F(0.0)], t), t * f(0x3e4c_cccd) + f(0x3f4c_cccd));
    let base = add(rq(m, e + 0x320), rq(m, e.wrapping_add(off)));
    add(add(base, scale(vel, ease)), scale(fall, age))
}

/// `Fx_Spray_DrawParticles` 0x12d890(spray): nothing when +0x18 (the count) is 0. Else, with texture 3 of the
/// app's table, each live entry of the ring is drawn: 5 of 0x3c0 bytes from +0x20, the oldest at +0x1c, live
/// while its age (+0x3b8) is not past its life (+0x3bc), and kept by the box test (its place ± 1000).
/// - **Its colour**: +0x3a8..+0x3b0 moved 0.7504 of the way to their mean, plus 0.5286, held to 0..1.994, ×128
///   as integers.
/// - **Its ribbon**: the entry's points (+0x3b4 of them) at t = 0, then four strips (the renderer's +0x220).
///   Strip j joins the points at t = (j − 1)/4 and j/4: u = 3k/4 for point k, v the earlier t's 1 − t (0 for the
///   first) and 1 − t.
/// - **Alpha**: (life − age) × the point's width (+0x340 + 4k) × 128 / 0.995, as an integer; 0 on the first
///   strip's earlier edge.
pub fn spray_draw_particles(m: &mut Mem, env: &mut dyn ParticleDrawEnv, spray: u32, scratch: u32) {
    if m.r32(spray + 0x18) == 0 {
        return;
    }
    let r = renderer(m);
    vcall(m, env, r, 0x1c0, &[1]);
    vcall(m, env, r, 0x1c8, &[0, 0, BIAS]);
    vcall(m, env, r, 0x1e0, &[3]);
    vcall(m, env, r, 0x1e8, &[2, 0]);
    let tex = texture(m, 3);
    vcall(m, env, r, 0x1f0, &[tex, 0]);
    vcall(m, env, r, 0x1f8, &[0, 0]);
    let (pts, verts) = (scratch + 0x40, scratch + 0x1d0);
    let mut n = m.r32(spray + 0x18) as i32;
    let mut i = 0i32;
    while i < n {
        let k = (m.r32(spray + 0x1c) as i32).wrapping_add(i).wrapping_rem(5);
        let e = spray.wrapping_add(k.wrapping_mul(0x3c0).wrapping_add(0x20) as u32);
        i += 1;
        if rf(m, e + 0x3bc) < rf(m, e + 0x3b8) {
            continue;
        }
        let k1000 = [F(1000.0), F(1000.0), F(1000.0), F(0.0)];
        let place = rq(m, e + 0x320);
        wq(m, scratch, [0, 1, 2, 3].map(|j| place[j] - k1000[j]));
        wq(m, scratch + 0x10, add(place, k1000));
        if vcall(m, env, r, 0x280, &[scratch, scratch + 0x10]) == 1 {
            n = m.r32(spray + 0x18) as i32;
            continue;
        }
        let (c1, c2, c3) = (rf(m, e + 0x3a8), rf(m, e + 0x3ac), rf(m, e + 0x3b0));
        let mean = ((c1 + c2) + c3) * f(0x3eaa_aaab);
        let pull = f(0x3f40_1b3c);
        let (c1, c3, c2) = (c1 + (mean - c1) * pull, c3 + (mean - c3) * pull, c2 + (mean - c2) * pull);
        let channel = |c: F| {
            let v = c + f(0x3f07_5370);
            let v = if v < F(0.0) {
                F(0.0)
            } else if f(0x3fff_3eae) < v {
                f(0x3fff_3eae)
            } else {
                v
            };
            cvt_w((v * F(128.0)).0) as u32
        };
        let colour = [channel(c1), channel(c2), channel(c3)];
        let mut j = 0i32;
        while j < m.r32(e + 0x3b4) as i32 {
            let p = spray_compute_particle(m, e, j as u32, F(0.0));
            wq(m, pts + 16 * j as u32, p);
            j += 1;
        }
        let mut before = F(0.0);
        for strip in 1..5 {
            let t = cvt_s_w(strip) * F(0.25);
            let after = F(1.0) - t;
            let mut count = 0u32;
            let mut j = 0i32;
            while j < m.r32(e + 0x3b4) as i32 {
                let width = rf(m, e + 0x340 + 4 * j as u32) * F(128.0);
                let left = rf(m, e + 0x3bc) - rf(m, e + 0x3b8);
                let alpha = cvt_w((left * div(width, f(0x3f7e_b7a0))).0) as u32;
                let p = spray_compute_particle(m, e, j as u32, t);
                let u = cvt_s_w(j) * F(0.25) * F(3.0);
                let at = pts + 16 * j as u32;
                let old = rq(m, at);
                vertex(m, verts + 0x30 * count, u, before, [colour[0], colour[1], colour[2], if strip == 1 { 0 } else { alpha }], old);
                vertex(m, verts + 0x30 * (count + 1), u, after, [colour[0], colour[1], colour[2], alpha], p);
                wq(m, at, p);
                count += 2;
                j += 1;
            }
            before = after;
            vcall(m, env, r, 0x220, &[count, verts, 0]);
        }
        n = m.r32(spray + 0x18) as i32;
    }
    put_back(m, env, r, true);
}

/// `Sprite_DrawRotatedQuad` 0x19b8c8(colour, angle°, x, y, w, h) and `_DrawRotatedTexQuad` 0x19bb48 (… u0, v0,
/// u1, v1: the game's f18, f17, f19 and its stack word): the quad (x, y, w, h) turned by the angle about its
/// centre, as four vertices to the renderer's +0x220 (4, the vertices, 0). The colour: four floats ×128 as
/// integers (stored 1, 2, 3, 0), then the quad's z at +0x10. The corners go (x, y), (x + w, y), (x, y + h),
/// (x + w, y + h), with u, v from 0 to 1 or the rectangle given.
pub fn sprite_draw_rotated_quad(m: &mut Mem, env: &mut dyn ParticleDrawEnv, colour: u32, angle: F, x: F, y: F, w: F, h: F, uv: Option<[F; 4]>, scratch: u32) {
    let a = angle * f(0x3c8e_fa35);
    let (s, c) = (F(math::sin(a.0)), F(math::cos(a.0)));
    let (hw, hh) = (w * F(0.5), h * F(0.5));
    let ints = [1u32, 2, 3, 0].map(|k| cvt_w((rf(m, colour + 4 * k) * F(128.0)).0) as u32);
    let z = rf(m, colour + 0x10);
    let (cx, cy) = (x + hw, y + hh);
    let (dx0, dy0) = (x - cx, y - cy);
    let (ax, ay) = (cx + dx0 * c, cy + dx0 * s);
    let dx1 = dx0 + w;
    let (bx, by) = (cx + dx1 * c, cy + dx1 * s);
    let dy1 = dy0 + h;
    let corners = [
        (ax - dy0 * s, ay + dy0 * c),
        (bx - dy0 * s, by + dy0 * c),
        (ax - dy1 * s, ay + dy1 * c),
        (bx - dy1 * s, by + dy1 * c),
    ];
    let [u0, v0, u1, v1] = uv.unwrap_or([F(0.0), F(0.0), F(1.0), F(1.0)]);
    let uvs = [(u0, v0), (u1, v0), (u0, v1), (u1, v1)];
    for (k, ((px, py), (u, v))) in corners.into_iter().zip(uvs).enumerate() {
        vertex(m, scratch + 0x30 * k as u32, u, v, ints, [px, py, z, F(1.0)]);
    }
    let r = renderer(m);
    vcall(m, env, r, 0x220, &[4, scratch, 0]);
}

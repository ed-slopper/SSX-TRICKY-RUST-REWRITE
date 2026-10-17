//! The lights (board row G7d1): the world's lights gathered for a point (`World_GetLightsAt` 0x25a160 with
//! `Lights_InsertStrongest` 0x25a028 and `World_GetCellAt` 0x260d08), the light block the renderer takes
//! (`Lights_Build` 0x1ce140, `Lights_Clear` 0x1ce278), `Mat4_RotateBy` 0x1ce310 and the light manager `cLightMan`
//! 0x1ce0f0..0x1ce408. On the bytes at their addresses ([`Mem`]); the light manager's delete is a call out
//! ([`LightEnv`]). Checked in the function runner (`tests/lights.rs`).
//!
//! **A light** (the `.pbd`'s, listed by the `.ltg` INode it touches: +0x44 its list of pointers, +0x2e the count):
//! +0 the kind (0 directional, 1 spot, 2 point, 3 ambient), +4 flags (0x100 a local light, 0x200 not dimmed by the
//! local ones), +0x10 the colour (r, g, b), +0x1c the direction, +0x28 the position, +0x4c the spot's cone (a
//! cosine), +0x50 the falloff (1 1/d, 2 1/d², 3 1/d³, from 100 on), +0x54 the spot's power.
//!
//! **A colour quad** is (0, r, g, b); the three kept lights' colours and directions and an ambient quad go to
//! `Lights_Build`, which makes the renderer's block: the three directions as the columns of a matrix (+0), and the
//! colours as rows (r, g, b, 0-lane) with the ambient last, all × 128 (+0x40).

use crate::animseq::{cvt_s_w, div, sqrt};
use crate::pf::Mem;
use crate::ps2f::F;

/// The light manager (`cLightMan`): the one there is (0x33efb8) and its vtable
pub const LIGHT_MAN: u32 = 0x33efb8;
pub const LIGHT_MAN_VTABLE: u32 = 0x391e20;

/// The light manager's delete (`Mem_Delete` 0x114f80)
pub trait LightEnv {
    fn delete(&mut self, m: &mut Mem, p: u32);
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
/// A quad as `lqc2` / `lq` read it (the address rounded down to 16)
fn rq(m: &Mem, a: u32) -> [F; 4] {
    let a = a & !15;
    [0, 1, 2, 3].map(|k| rf(m, a + 4 * k))
}
fn wq(m: &mut Mem, a: u32, v: [F; 4]) {
    let a = a & !15;
    for k in 0..4 {
        wf(m, a + 4 * k, v[k as usize]);
    }
}
/// Sixteen bytes copied as `ldl`/`ldr` + `sdl`/`sdr` do (any alignment)
fn copy16(m: &mut Mem, to: u32, from: u32) {
    for k in 0..4 {
        let w = m.r32(from + 4 * k);
        m.w32(to + 4 * k, w);
    }
}
fn w16(m: &mut Mem, to: u32, v: [u32; 4]) {
    for k in 0..4 {
        m.w32(to + 4 * k, v[k as usize]);
    }
}

/// `cLightMan::cLightMan` 0x1ce0f0: the manager noted (0x33efb8), the vtable
pub fn light_man_construct(m: &mut Mem, this: u32) -> u32 {
    m.w32(LIGHT_MAN, this);
    m.w32(this, LIGHT_MAN_VTABLE);
    this
}

/// `cLightMan::~cLightMan` 0x1ce110: the vtable, no manager, deleted with flag 1
pub fn light_man_destruct(m: &mut Mem, env: &mut dyn LightEnv, this: u32, flags: u32) {
    m.w32(this, LIGHT_MAN_VTABLE);
    m.w32(LIGHT_MAN, 0);
    if flags & 1 != 0 {
        env.delete(m, this);
    }
}

/// 0x1ce400: the light manager. (Its virtuals 0x1ce3e0, 0x1ce3e8, 0x1ce3f0 and 0x1ce3f8 do nothing.)
pub fn light_man(m: &Mem) -> u32 {
    m.r32(LIGHT_MAN)
}

/// `Lights_Build` 0x1ce140(out, ambient, d0, c0, d1, c1, d2, c2): the renderer's light block. The words are moved
/// in the game's order (it reads each just before writing it); then the colour rows (+0x40..) × 128
#[allow(clippy::too_many_arguments)]
pub fn lights_build(m: &mut Mem, out: u32, ambient: u32, d0: u32, c0: u32, d1: u32, c1: u32, d2: u32, c2: u32) {
    let one = 1f32.to_bits();
    let mv = |m: &mut Mem, to: u32, from: u32| {
        let v = m.r32(from);
        m.w32(out + to, v);
    };
    mv(m, 0, d0);
    mv(m, 0x10, d0 + 4);
    mv(m, 0x20, d0 + 8);
    m.w32(out + 0x30, 0);
    mv(m, 4, d1);
    mv(m, 0x14, d1 + 4);
    mv(m, 0x24, d1 + 8);
    m.w32(out + 0x34, 0);
    mv(m, 8, d2);
    mv(m, 0x18, d2 + 4);
    let z = m.r32(d2 + 8);
    m.w32(out + 0x3c, one);
    m.w32(out + 0x28, z);
    for o in [0x38, 0xc, 0x1c, 0x2c] {
        m.w32(out + o, 0);
    }
    for (lane, at) in [(1u32, 0u32), (2, 4), (3, 8), (0, 0xc)] {
        for (row, c) in [c0, c1, c2, ambient].into_iter().enumerate() {
            mv(m, 0x40 + 0x10 * row as u32 + at, c + 4 * lane);
        }
    }
    let rows = out + 0x40;
    let v: Vec<[F; 4]> = (0..4).map(|k| rq(m, rows + 0x10 * k).map(|x| x * F(128.0))).collect();
    for (k, row) in v.into_iter().enumerate() {
        wq(m, rows + 0x10 * k as u32, row);
    }
}

/// `Lights_Clear` 0x1ce278(out): no lights, the ambient row (128, 128, 128, 128), the matrix's w 1
pub fn lights_clear(m: &mut Mem, out: u32) {
    let (k128, one) = (128f32.to_bits(), 1f32.to_bits());
    for o in (0..0x80).step_by(4) {
        m.w32(out + o, 0);
    }
    for o in [0x7c, 0x70, 0x74, 0x78] {
        m.w32(out + o, k128);
    }
    m.w32(out + 0x3c, one);
}

/// Four rows times a row vector on VU0 (vmulax / vmadday / vmaddaz / vmaddw): Σ rows[k] × v[k]
fn mat_vec(rows: &[[F; 4]; 4], v: [F; 4]) -> [F; 4] {
    [0, 1, 2, 3].map(|i| ((rows[0][i] * v[0] + rows[1][i] * v[1]) + rows[2][i] * v[2]) + rows[3][i] * v[3])
}

/// `Mat4_RotateBy` 0x1ce310(a, b): a = b × a (each of b's rows through a's), with its last row (0, 0, 0, 1)
pub fn mat4_rotate_by(m: &mut Mem, a: u32, b: u32) {
    let ra: [[F; 4]; 4] = [0, 1, 2, 3].map(|k| rq(m, a + 0x10 * k));
    let p: Vec<[F; 4]> = (0..4).map(|k| mat_vec(&ra, rq(m, b + 0x10 * k))).collect();
    wq(m, a + 0x30, [F(0.0), F(0.0), F(0.0), F(1.0)]);
    wq(m, a + 0x10, p[1]);
    wq(m, a + 0x20, p[2]);
    wq(m, a, p[0]);
}

/// `Lights_InsertStrongest` 0x25a028 with the three strengths held apart: the light (its direction quad, its colour
/// quad's words) kept when stronger (|(r + g) + b|) than the weakest of the three (ascending), the weaker ones
/// moved down; each kept light's colour at `colours` + 0x10 k, its direction at `dirs` + 0x10 k
fn insert(m: &mut Mem, st: &mut [F; 3], colours: u32, dirs: u32, dir: [F; 4], colour: [u32; 4]) {
    let c = |k: usize| F(f32::from_bits(colour[k]));
    let s = F(((c(1) + c(2)) + c(3)).0.abs());
    if !(st[0].0 < s.0) {
        return;
    }
    let mut slot = 0u32;
    if st[1].0 < s.0 {
        st[0] = st[1];
        copy16(m, colours, colours + 0x10);
        let d = rq(m, dirs + 0x10);
        wq(m, dirs, d);
        slot = 1;
        if st[2].0 < s.0 {
            st[1] = st[2];
            copy16(m, colours + 0x10, colours + 0x20);
            let d = rq(m, dirs + 0x20);
            wq(m, dirs + 0x10, d);
            slot = 2;
        }
    }
    st[slot as usize] = s;
    w16(m, colours + 0x10 * slot, colour);
    wq(m, dirs + 0x10 * slot, dir);
}

/// `Lights_InsertStrongest` 0x25a028(strengths, colours, dirs, dir, colour): see [`insert`]; the light's own
/// direction and colour are read first (they are not among the three)
pub fn insert_strongest(m: &mut Mem, strengths: u32, colours: u32, dirs: u32, dir: u32, colour: u32) {
    let mut st = [0, 1, 2].map(|k| rf(m, strengths + 4 * k));
    let d = rq(m, dir);
    let c = [0, 1, 2, 3].map(|k| m.r32(colour + 4 * k));
    insert(m, &mut st, colours, dirs, d, c);
    for k in 0..3 {
        wf(m, strengths + 4 * k, st[k as usize]);
    }
}

/// `World_GetCellAt` 0x260d08(world, tgroup, dx, dy): the TGroup's INode at (dx, dy) from its middle: each
/// coordinate n × (d + size / 2) / size (the EE's division), toward zero, clamped to 0..n−1
pub fn cell_at(m: &Mem, world: u32, tgroup: u32, dx: F, dy: F) -> u32 {
    let g = m.r32(world);
    let index = |d: F| {
        let size = rf(m, g + 0x28);
        let n = m.r32(g + 0x44) as i32;
        let v = div(cvt_s_w(n) * (d + size * F(0.5)), size).ee() as i32;
        if v < 0 { 0 } else { v.min(n - 1) }
    };
    let (x, y) = (index(dx), index(dy));
    let n = m.r32(g + 0x44);
    m.r32(tgroup + 0x38).wrapping_add((y as u32).wrapping_mul(n).wrapping_add(x as u32).wrapping_mul(0x4c))
}

/// |d| as VU0 makes it (all four lanes squared, x + y, + z, + w; `vsqrt`)
fn vu_len(d: [F; 4]) -> F {
    let s = d.map(|x| x * x);
    sqrt(((s[0] + s[1]) + F(1.0) * s[2]) + F(1.0) * s[3])
}
fn vu_dot(a: [F; 4], b: [F; 4]) -> F {
    let p = [0, 1, 2, 3].map(|k| a[k] * b[k]);
    ((p[0] + p[1]) + F(1.0) * p[2]) + F(1.0) * p[3]
}

/// The falloff with distance d: 1 below 100, else 100 / d, 10⁴ / d² or 10⁶ / d³ for kinds 1, 2, 3 (others 1)
fn falloff(kind: u32, d: F) -> F {
    let near = d.0 < 100.0;
    match kind {
        1 if !near => div(F(100.0), d),
        2 if !near => div(F(10000.0), d * d),
        3 if !near => div(F(1.0e6), (d * d) * d),
        _ => F(1.0),
    }
}

/// The spot's power: `base` to the whole power `e` (negative: of 1 / base), by squaring; 1 for e 0, 0 for a base
/// of 0
fn spot_power(base: F, e: i32) -> F {
    if e == 0 {
        return F(1.0);
    }
    let (mut b, mut e) = (base, e);
    if e < 0 {
        b = div(F(1.0), b);
        e = e.wrapping_neg();
    }
    if b.0 == 0.0 {
        return F(0.0);
    }
    let mut r = F(1.0);
    loop {
        if e & 1 != 0 {
            r = r * b;
        }
        e >>= 1;
        if e == 0 {
            return r;
        }
        b = b * b;
    }
}

/// The light at `l` seen from `pos` (a quad): the way to it (normalised; `vdiv`, `vmulq`) and its distance
fn toward(m: &Mem, l: u32, p: [F; 4]) -> ([F; 4], F) {
    let at = [rf(m, l + 0x28), rf(m, l + 0x2c), rf(m, l + 0x30), F(1.0)];
    let d = [0, 1, 2, 3].map(|k| at[k] - p[k]);
    let len = vu_len(d);
    let q = div(F(1.0), len);
    (d.map(|x| x * q), len)
}

/// `World_GetLightsAt` 0x25a160(world, pos, ambient, colours, dirs): the lights at a point.
///
/// Set first: the ambient (1, 0, 0, 0), three dark lights (colours 0, directions (1, 0, 0, 0), strengths 0). The
/// grid's TGroup there (the cell (p − corner + size) / size − 1, clamped; none: nothing more) and its INode
/// (`World_GetCellAt` from the unclamped cell's middle). Then over the INode's lights twice:
/// - **local lights** (flag 0x100), point (2) and spot (1), summed: each colour × its falloff (a spot only inside
///   its cone, × its power of the cosine). With any, the sum's r, g, b each become clamp(sum + 1, 0, 1).
/// - **the others**: ambient (3) added to the ambient's r, g, b; directional (0, its direction turned), point (2)
///   and spot (1, inside its cone) as colour quads (0, r, g, b) × falloff (× power) put through
///   `Lights_InsertStrongest`. With local lights, unless the light has flag 0x200, each is first dimmed by their
///   clamped sum (an ambient light the whole ambient, after it is added).
pub fn get_lights_at(m: &mut Mem, world: u32, pos: u32, ambient: u32, colours: u32, dirs: u32) {
    let (one, zero) = (F(1.0), F(0.0));
    w16(m, ambient, [one.0.to_bits(), 0, 0, 0]);
    let mut st = [zero; 3];
    for k in 0..3 {
        w16(m, colours + 0x10 * k, [0; 4]);
        wq(m, dirs + 0x10 * k, [one, zero, zero, zero]);
    }
    let g = m.r32(world);
    let (px, py) = (rf(m, pos), rf(m, pos + 4));
    let size = rf(m, g + 0x28);
    let cx = div((px - rf(m, g + 4)) + size, size).ee() as i32 - 1;
    let size = rf(m, g + 0x28);
    let cy = div((py - rf(m, g + 8)) + size, size).ee() as i32 - 1;
    let w = m.r32(g + 0x2c) as i32;
    let tx = if cx < 0 { 0 } else { cx.min(w - 1) };
    let h = m.r32(g + 0x30) as i32;
    let ty = if cy < 0 { 0 } else { cy.min(h - 1) };
    let tg = m.r32(m.r32(g + 0x4c).wrapping_add(((ty * m.r32(g + 0x2c) as i32 + tx) as u32).wrapping_mul(4)));
    if tg == 0 {
        return;
    }
    let size = rf(m, g + 0x28);
    let half = size * F(0.5);
    let my = (cvt_s_w(cy) * size + half) + rf(m, g + 8);
    let mx = (cvt_s_w(cx) * size + half) + rf(m, g + 4);
    let node = cell_at(m, world, tg, px - mx, py - my);
    let list = m.r32(node + 0x44);
    let count = |m: &Mem| m.r16(node + 0x2e) as i16 as i32;
    let p = rq(m, pos);
    let colour = |m: &Mem, l: u32| [rf(m, l + 0x10), rf(m, l + 0x14), rf(m, l + 0x18)];
    let dir_of = |m: &Mem, l: u32| [rf(m, l + 0x1c), rf(m, l + 0x20), rf(m, l + 0x24), F(0.0)];
    let cone = |m: &Mem, l: u32, to: [F; 4]| {
        let c = -vu_dot(to, dir_of(m, l));
        (rf(m, l + 0x4c).0 < c.0).then_some(c)
    };
    // the local lights
    let mut sum = [one, zero, zero, zero];
    let mut found = false;
    let mut i = 0;
    while i < count(m) {
        let l = m.r32(list.wrapping_add(4 * i as u32));
        i += 1;
        if m.r32(l + 4) & 0x100 == 0 {
            continue;
        }
        let k = match m.r32(l) {
            2 => {
                let d = vu_len([0, 1, 2, 3].map(|k| [rf(m, l + 0x28), rf(m, l + 0x2c), rf(m, l + 0x30), one][k] - p[k]));
                falloff(m.r32(l + 0x50), d)
            }
            1 => {
                let (to, d) = toward(m, l, p);
                let Some(c) = cone(m, l, to) else { continue };
                let att = falloff(m.r32(l + 0x50), d);
                att * spot_power(c, rf(m, l + 0x54).ee() as i32)
            }
            _ => continue,
        };
        found = true;
        let c = colour(m, l);
        for j in 0..3 {
            sum[j + 1] = sum[j + 1] + c[j] * k;
        }
    }
    if found {
        for j in 1..4 {
            let v = sum[j] + one;
            sum[j] = if v.0 < 0.0 { zero } else if 1.0 < v.0 { one } else { v };
        }
    }
    let dim = |m: &Mem, l: u32, q: [F; 4]| {
        if found && m.r32(l + 4) & 0x200 == 0 { [sum[0] * zero, q[1] * sum[1], q[2] * sum[2], q[3] * sum[3]] } else { q }
    };
    let bits = |q: [F; 4]| q.map(|x| x.0.to_bits());
    // the others
    let mut i = 0;
    while i < count(m) {
        let l = m.r32(list.wrapping_add(4 * i as u32));
        i += 1;
        let flags = m.r32(l + 4);
        if flags & 0x100 != 0 {
            continue;
        }
        match m.r32(l) {
            3 => {
                let c = colour(m, l);
                let mut a = [0, 1, 2, 3].map(|k| rf(m, ambient + 4 * k));
                for j in 0..3 {
                    a[j + 1] = a[j + 1] + c[j];
                }
                for j in 1..4 {
                    wf(m, ambient + 4 * j as u32, a[j]);
                }
                if found && flags & 0x200 == 0 {
                    let x = rf(m, ambient);
                    for j in 1..4 {
                        wf(m, ambient + 4 * j as u32, a[j] * sum[j]);
                    }
                    wf(m, ambient, x * sum[0]);
                }
            }
            0 => {
                let c = colour(m, l);
                let q = dim(m, l, [zero, c[0], c[1], c[2]]);
                let d = dir_of(m, l).map(|x| x * F(-1.0));
                insert(m, &mut st, colours, dirs, d, bits(q));
            }
            2 => {
                let (to, d) = toward(m, l, p);
                let att = falloff(m.r32(l + 0x50), d);
                let c = colour(m, l);
                let q = dim(m, l, [zero, c[0] * att, c[1] * att, c[2] * att]);
                insert(m, &mut st, colours, dirs, to, bits(q));
            }
            1 => {
                let (to, d) = toward(m, l, p);
                let Some(cs) = cone(m, l, to) else { continue };
                let att = falloff(m.r32(l + 0x50), d) * spot_power(cs, rf(m, l + 0x54).ee() as i32);
                let c = colour(m, l);
                let q = dim(m, l, [zero, att * c[0], att * c[1], att * c[2]]);
                insert(m, &mut st, colours, dirs, to, bits(q));
            }
            _ => {}
        }
    }
}

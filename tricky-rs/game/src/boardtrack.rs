//! The board tracks in the snow (board row G7f): `Fx_BoardTrack` 0x132648 and the light sum 0x1325b8 it shades
//! with. The object (one a rider) is words: +0 and +0x10 the strip's open end (the two edges), +0x2c / +0x30 /
//! +0x3c the shading at that end, then 100 points of 0x40 bytes from +0x40 (+0 edge A, +0x10 edge B, +0x20
//! shade right, +0x24 shade left, +0x28 fade, +0x2c right × fade, +0x30 left × fade, +0x34 the surface, +0x38 1
//! on the first point of a strip, +0x3c width × depth); at +0x1940 the head slot, +0x1944 how many points,
//! +0x1948 the fade-in, +0x194c the side the board is lit from (−1..1), +0x1950 whether a strip is open,
//! +0x1960 the last point laid, +0x1970 its direction, +0x1980 its width.

pub const WORDS: usize = 0x1984 / 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Trail {
    pub w: Vec<u32>,
}

impl Default for Trail {
    fn default() -> Self {
        Trail { w: vec![0; WORDS] }
    }
}

type V4 = [f32; 4];

impl Trail {
    fn f(&self, byte: usize) -> f32 {
        f32::from_bits(self.w[byte / 4])
    }
    fn set_f(&mut self, byte: usize, v: f32) {
        self.w[byte / 4] = v.to_bits();
    }
    fn v(&self, byte: usize) -> V4 {
        [0, 1, 2, 3].map(|k| self.f(byte + 4 * k))
    }
    fn set_v(&mut self, byte: usize, v: V4) {
        for k in 0..4 {
            self.set_f(byte + 4 * k, v[k]);
        }
    }
    fn head(&self) -> i32 {
        self.w[0x1940 / 4] as i32
    }
    fn count(&self) -> i32 {
        self.w[0x1944 / 4] as i32
    }
}

/// The rider's light set (+0x5ae0): the ambient triple at +0x404, then three lights, the weights at +0x414 (16
/// apart) and the directions at +0x440 (16 apart).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lights {
    pub ambient: [f32; 3],
    pub weights: [[f32; 3]; 3],
    pub dirs: [V4; 3],
}

/// What the strip reads of the rider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackIn {
    pub surface: u32,
    pub motion: u32,
    pub hit: u32,
    /// +0x5760, +0x5770, +0x5780, +0x5790
    pub m: [V4; 4],
    /// motion 5, 6: the vectors at +0x330 and +0x230 of the object the table at +0x4494 points to
    pub table: [V4; 2],
    pub pos: V4,
    pub up: V4,
    pub ground: V4,
    pub f1fc: f32,
    pub f214: f32,
    pub lights: Lights,
}

fn dot4(a: V4, b: V4) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| x * k)
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { -f32::MAX } else { f32::MAX };
    }
    a / b
}

/// 0x1325b8: the ambient plus each light's weights times the facing, for the lights the normal faces.
fn light(l: &Lights, n: V4) -> f32 {
    let mut sum = (l.ambient[0] + l.ambient[1]) + l.ambient[2];
    for k in 0..3 {
        let d = dot4(n, l.dirs[k]);
        if 0.0 < d {
            let w = (l.weights[k][0] + l.weights[k][1]) + l.weights[k][2];
            sum += d * w;
        }
    }
    sum
}

fn unit(v: V4) -> V4 {
    let s = dot4(v, v);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    scale(v, q)
}

/// `Fx_BoardTrack` 0x132648(trail, rider).
pub fn board_track(t: &mut Trail, r: &TrackIn) {
    let (mut on, depth) = match r.surface {
        1 | 2 | 8 | 0xf | 0x10 => (true, f32::from_bits(0x3ce9_f4f6)),
        3 | 4 => (true, f32::from_bits(0x3c88_7d2b)),
        5 => (true, f32::from_bits(0x3e4c_8aae)),
        _ => (false, 0.0),
    };
    if r.motion != 2 && r.motion != 5 && r.motion != 6 {
        on = false;
    }
    if r.hit.wrapping_sub(2) < 2 {
        on = false;
    }
    if !on {
        let head = t.head();
        if t.f(0x1948) != 0.0 {
            let count = t.count();
            let mut f3 = 0.0f32;
            let mut a1 = head - 1;
            let mut i = 0;
            if count > 0 {
                loop {
                    if a1 < 0 {
                        a1 += 100;
                    }
                    let p = 0x40 * a1 as usize;
                    let fade = t.f(p + 0x68) * f3;
                    let right = t.f(p + 0x60);
                    let left = t.f(p + 0x64);
                    f3 += f32::from_bits(0x3e99_999a);
                    t.set_f(p + 0x68, fade);
                    t.set_f(p + 0x70, left * fade);
                    t.set_f(p + 0x6c, right * fade);
                    if 1.0 <= f3 {
                        break;
                    }
                    i += 1;
                    if i >= count {
                        break;
                    }
                    a1 -= 1;
                }
            }
        }
        t.set_f(0x1948, 0.0);
        t.w[0x1950 / 4] = 0;
        let p = 0x40 * head as usize;
        let z = t.f(0x1948);
        for off in [0x68, 0x70, 0x6c, 0x64, 0x60] {
            t.set_f(p + off, z);
        }
        return;
    }
    // the four rows
    let (a, b, c, d) = if r.motion == 2 { (r.m[0], r.m[1], r.m[2], r.m[3]) } else { (r.table[0], r.table[1], r.m[0], r.m[3]) };
    let cross = {
        let (p, n) = (r.pos, r.up);
        [p[1] * n[2] - n[1] * p[2], p[2] * n[0] - n[2] * p[0], p[0] * n[1] - n[0] * p[1], 0.0]
    };
    let side = unit(cross);
    let ad = sub(a, d);
    let bc = sub(b, c);
    let f23 = dot4(ad, side);
    let f24 = dot4(bc, side);
    let lit = light(&r.lights, side);
    let neg = scale(side, -1.0);
    let lit_back = light(&r.lights, neg);
    let back_brighter = lit < lit_back;
    let fade = t.f(0x1948);
    if fade == 0.0 {
        t.set_f(0x194c, if back_brighter { 1.0 } else { -1.0 });
    } else {
        let step = if back_brighter { f32::from_bits(0x3d08_8889) } else { f32::from_bits(0xbd08_8889) };
        let mut s = t.f(0x194c) + step;
        if s < -1.0 {
            s = -1.0;
        } else if 1.0 < s {
            s = 1.0;
        }
        t.set_f(0x194c, s);
    }
    let sd = t.f(0x194c);
    let f4 = r.f214;
    let k = f32::from_bits(0x3f2a_aaab);
    let mut f21 = sd * k;
    let mut f22 = (-sd) * k;
    if 0.0 < f4 {
        f21 = (f4 * 0.5) * f21 + f21;
    } else {
        f22 *= 1.0 - f4 * 0.5;
    }
    let mut e0 = sub(r.ground, t.v(0x1960));
    let len = dot4(e0, e0).sqrt();
    if len != 0.0 {
        let q = ps2_div(1.0, len);
        e0 = scale(e0, q);
    }
    let mut width = f23.abs();
    if width < f24.abs() {
        width = f24.abs();
    }
    if r.f1fc == 0.0 {
        width *= 1.0 - f4.abs() * f32::from_bits(0x3f59_999a);
    }
    let speed = dot4(r.pos, r.pos).sqrt();
    let new_point = if f32::from_bits(0x42dc_48a5) < len || fade == 0.0 {
        true
    } else if !(f32::from_bits(0x430a_e38e) < speed) {
        false
    } else if f32::from_bits(0x4121_4776) < (width - t.f(0x1980)).abs() {
        true
    } else {
        dot4(e0, t.v(0x1970)) < f32::from_bits(0x3f7e_fd0f)
    };
    if new_point {
        let half = scale(side, width * 0.5);
        let lift = scale(r.up, 10.0);
        let p = add(r.ground, lift);
        let aa = add(p, half);
        let head = t.head();
        t.set_v(0x1970, e0);
        t.set_v(0x1960, r.ground);
        t.set_f(0x1980, width);
        let at = 0x40 * head as usize;
        t.set_v(at + 0x40, aa);
        t.set_v(at + 0x50, sub(p, half));
        t.set_f(at + 0x7c, width * depth);
        t.w[(at + 0x74) / 4] = r.surface;
        let f1 = t.f(0x1948);
        t.w[(at + 0x78) / 4] = (f1 == 0.0) as u32;
        let nf = f1 + f32::from_bits(0x3e99_999a);
        t.set_f(0x1948, nf);
        t.w[0x1940 / 4] = ((head + 1) % 100) as u32;
        if 1.0 < nf {
            t.set_f(0x1948, 1.0);
        }
        let f0 = t.f(0x1948);
        let nh = t.head();
        let mut cnt = t.count();
        let q = 0x40 * nh as usize;
        t.set_f(q + 0x70, f22 * f0);
        t.set_f(q + 0x6c, f21 * f0);
        t.set_f(q + 0x68, f0);
        t.set_f(q + 0x60, f21);
        t.set_f(q + 0x64, f22);
        if cnt < 100 {
            cnt += 1;
            t.w[0x1944 / 4] = cnt as u32;
        }
        let mut left = cnt - 0x5a;
        if left > 0 {
            let mut idx = nh - cnt + left;
            while left != 0 {
                if idx < 0 {
                    idx += 100;
                }
                let o = 0x40 * idx as usize;
                let f = t.f(o + 0x68) - f32::from_bits(0x3dcc_cccd);
                t.set_f(o + 0x68, f);
                if f < 0.0 {
                    t.w[(o + 0x68) / 4] = 0;
                }
                let fd = t.f(o + 0x68);
                let right = t.f(o + 0x60);
                let lft = t.f(o + 0x64);
                t.set_f(o + 0x6c, right * fd);
                t.set_f(o + 0x70, lft * fd);
                left -= 1;
                idx -= 1;
            }
        }
    }
    // the open end
    let ab = sub(a, b);
    let ac = sub(a, c);
    let mut pick = ab;
    if dot4(side, ab).abs() < dot4(side, ac).abs() {
        pick = ac;
    }
    let l = dot4(pick, pick).sqrt();
    let u = scale(pick, ps2_div(1.0, l));
    let dn = dot4(u, r.up);
    let bperp = sub(u, scale(r.up, dn));
    let mut dvec = unit(bperp);
    if dot4(dvec, side) < 0.0 {
        dvec = scale(dvec, -1.0);
    }
    let hv = scale(dvec, l * 0.5);
    let p = add(r.ground, scale(r.up, 10.0));
    let f0 = t.f(0x1948);
    let cnt = t.count();
    let aa = add(p, hv);
    t.set_f(0x3c, l * depth);
    t.set_f(0x2c, f21 * f0);
    let bb = sub(p, hv);
    t.set_v(0, aa);
    t.set_f(0x30, f22 * f0);
    t.set_v(0x10, bb);
    if cnt >= 2 {
        let head = t.head();
        let mut prev = head - 1;
        if !(-1 < prev) {
            prev = head + 99;
        }
        let o = 0x40 * prev as usize + 0x40;
        let pa = t.v(o);
        if dot4(sub(t.v(0), pa), r.pos) < 0.0 {
            t.set_v(0, pa);
        }
        let pb = t.v(o + 0x10);
        if dot4(sub(t.v(0x10), pb), r.pos) < 0.0 {
            t.set_v(0x10, pb);
        }
    }
    t.w[0x1950 / 4] = 1;
}

//! The game's own math routines (board row E14): sine, cosine, arcsine, arctangent and two helpers as the
//! original computes them (its polynomials, constants and order of operations), so exact ports of the code that
//! calls them give the same floats. `tests/math.rs` checks each bit for bit against the original in the function
//! runner.

/// The arcsine/arctangent polynomial's coefficients ($gp−0x7ff0..−0x7fe4, 0x3bb900–0x3bb90c).
const A1: f32 = f32::from_bits(0x3e2b_ba25);
const A2: f32 = f32::from_bits(0x3d8a_908b);
const A3: f32 = f32::from_bits(0x3d55_033a);
const A4: f32 = f32::from_bits(0x3d61_37ab);
const HALF_PI: f32 = 1.570_796_4;

/// The quadrant: x·2/π ± 0.5, truncated (cvt.w.s).
fn quadrant(x: f32) -> i32 {
    let q = if x < 0.0 { x * 0.636_619_75 - 0.5 } else { x * 0.636_619_75 + 0.5 };
    q as i32
}

fn sin_poly(r: f32, r2: f32) -> f32 {
    (((r2 * 2.755_731_9e-6 + -0.000_198_412_7) * r2 + 0.008_333_334) * r2 + -0.166_666_67) * r2 * r + r
}

fn cos_poly(r2: f32) -> f32 {
    (((r2 * 2.480_158_8e-5 + -0.001_388_888_9) * r2 + 0.041_666_668) * r2 + -0.5) * r2 + 1.0
}

fn asin_poly(t: f32) -> f32 {
    t * (A1 + t * (A2 + t * (A3 + t * A4)))
}

/// `Math_Sin` 0x250d60
pub fn sin(x: f32) -> f32 {
    let q = quadrant(x);
    let r = x - q as f32 * HALF_PI;
    let r2 = r * r;
    let v = if q & 1 == 0 { sin_poly(r, r2) } else { cos_poly(r2) };
    if q & 2 != 0 {
        -v
    } else {
        v
    }
}

/// `Math_Cos` 0x250e98
pub fn cos(x: f32) -> f32 {
    let q = quadrant(x);
    let r = x - q as f32 * HALF_PI;
    let r2 = r * r;
    let v = if q & 1 == 0 { cos_poly(r2) } else { sin_poly(r, r2) };
    if q.wrapping_add(1) & 2 != 0 {
        -v
    } else {
        v
    }
}

/// `Math_SinCos` 0x250c10: (sin, cos), the cosine as √(1 − sin²) of the reduced angle.
pub fn sin_cos(x: f32) -> (f32, f32) {
    let q = quadrant(x);
    let r = x - q as f32 * HALF_PI;
    let r2 = r * r;
    let s = ((((r2 * 2.755_731_9e-6 + -0.000_198_412_7) * r2 + 0.008_333_334) * r2 + -0.166_666_67) * r2 + 1.0) * r;
    let c = (1.0 - s * s).sqrt();
    match q & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `Math_Asin` 0x250fd0
pub fn asin(x: f32) -> f32 {
    if x < -0.707_106_77 {
        let t = 1.0 - x * x;
        return t.sqrt() * (asin_poly(t) + 1.0) + -HALF_PI;
    }
    let x2 = x * x;
    if x <= 0.707_106_77 {
        return asin_poly(x2) * x + x;
    }
    let t = 1.0 - x2;
    HALF_PI - t.sqrt() * (asin_poly(t) + 1.0)
}

/// `Math_Atan` 0x2510f8
pub fn atan(x: f32) -> f32 {
    if x < -1.0 {
        let t = 1.0 / (x * x + 1.0);
        return t.sqrt() * (asin_poly(t) + 1.0) + -HALF_PI;
    }
    let x2 = x * x;
    if 1.0 < x {
        let t = 1.0 / (x2 + 1.0);
        return HALF_PI - t.sqrt() * (asin_poly(t) + 1.0);
    }
    let t = x2 / (x2 + 1.0);
    let v = t.sqrt() * (asin_poly(t) + 1.0);
    if x < 0.0 {
        -v
    } else {
        v
    }
}

/// `Math_AngleDiff` 0x1741d0: a − b wrapped into [−π, π).
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let t = (a - b) * 0.159_154_94 + 0.5;
    let mut n = (t as i32) as f32;
    if t < n {
        n -= 1.0;
    }
    (a - b) - n * 6.283_185_5
}

/// `Math_Hermite` 0x1704b0: the cubic Hermite blend of p0, p1 with tangents m0, m1 at t.
pub fn hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    ((t3 + t3) - t2 * 3.0) * p0 + p0 + (t3 * -2.0 + t2 * 3.0) * p1 + ((t3 - (t2 + t2)) + t) * m0 + (t3 - t2) * m1
}

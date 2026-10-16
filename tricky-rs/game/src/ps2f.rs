//! The EE's and VU0's floats where they part from IEEE 754 (as `tools/r5900`'s `ps2float` models them, rounding to
//! nearest): no infinities, NaNs or denormals. A denormal is zero, a result too large is ±0x7F7FFFFF, division by
//! zero gives ±0x7F7FFFFF and the square root is of the absolute value. For ports whose inputs can reach those
//! cases (a degenerate patch's zero determinant, row E7c2).

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A float as the PS2 holds it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct F(pub f32);

/// A register value as the EE reads it: exponent 0 is zero, exponent 255 the largest float.
fn load(x: f32) -> f32 {
    let b = x.to_bits();
    match (b >> 23) & 0xff {
        0 => f32::from_bits(b & 0x8000_0000),
        0xff => f32::from_bits((b & 0x8000_0000) | f32::MAX.to_bits()),
        _ => x,
    }
}

/// An exact result (in f64) cut to the PS2's float: clamped, denormals to zero, rounded to nearest.
fn cut(r: f64) -> f32 {
    let a = r.abs();
    let mag = if a < f32::MIN_POSITIVE as f64 {
        0.0
    } else if a >= f32::MAX as f64 {
        f32::MAX
    } else {
        a as f32
    };
    if r.is_sign_negative() {
        -mag
    } else {
        mag
    }
}

impl Add for F {
    type Output = F;
    fn add(self, o: F) -> F {
        F(cut(load(self.0) as f64 + load(o.0) as f64))
    }
}

impl Sub for F {
    type Output = F;
    fn sub(self, o: F) -> F {
        self + F(-o.0)
    }
}

impl Mul for F {
    type Output = F;
    fn mul(self, o: F) -> F {
        F(cut(load(self.0) as f64 * load(o.0) as f64))
    }
}

impl Div for F {
    type Output = F;
    fn div(self, o: F) -> F {
        let (a, b) = (load(self.0), load(o.0));
        if b == 0.0 {
            let neg = a.is_sign_negative() != b.is_sign_negative();
            return F(if neg { -f32::MAX } else { f32::MAX });
        }
        F(cut(a as f64 / b as f64))
    }
}

impl Neg for F {
    type Output = F;
    fn neg(self) -> F {
        F(-self.0)
    }
}

impl F {
    /// The value as the EE reads it for a comparison (`c.lt.s` / `c.le.s` / `c.eq.s`): a denormal is zero, exponent
    /// 255 the largest float
    pub fn ee(self) -> f32 {
        load(self.0)
    }

    pub fn sqrt(self) -> F {
        F(cut((load(self.0).abs() as f64).sqrt()))
    }
}

//! Floating point as the EE's FPU and VU0 do it, which is not IEEE 754 (EE Core User's Manual, FPU chapter):
//! no infinities, NaNs or denormals. Denormal inputs and results are zero, results too large for a float are
//! clamped to ±0x7F7FFFFF, division by zero gives ±0x7F7FFFFF, and the square root of a negative number is the
//! root of its absolute value.
//!
//! How results are rounded is the open question (board rows F4c, F4f, F4g), so it is a setting, [`Rules`], one
//! rule per kind of operation:
//! - `Nearest`: IEEE round to nearest.
//! - `Chop`: toward zero, as the manual says the EE rounds.
//! - `EeAdder` (add and subtract only): the adder as the manual describes it: the smaller operand's mantissa is
//!   shifted right to line the exponents up and the bits shifted out are dropped (there is no guard bit), then the
//!   sum is normalised and cut toward zero.
//!
//! [`Rules::default`] is the best match to the game in PCSX2 so far (`tricky-rs/docs/checking.md`, F4f, F4g).
//! `TRICKY_FLOAT_RULES=add=ee,mul=chop` style strings parse with [`Rules::parse`].
//!
//! Exact results are formed in f64 and then cut to f32: products and quotients of two floats, and sums of
//! floats within 29 binary orders of each other, are exact or rounded once that way.

use std::cell::Cell;

const MAX: f32 = f32::MAX; // 0x7F7FFFFF

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Nearest,
    Chop,
    /// Add and subtract only.
    EeAdder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub add: Round,
    pub mul: Round,
    pub div: Round,
    pub cvt: Round,
}

impl Rules {
    /// Everything toward zero, as the manual describes the hardware.
    pub const MANUAL: Rules = Rules { add: Round::Chop, mul: Round::Chop, div: Round::Chop, cvt: Round::Chop };

    /// `add=ee,mul=chop,div=nearest,cvt=chop`; parts left out keep the default.
    pub fn parse(s: &str) -> Result<Rules, String> {
        let mut r = Rules::default();
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=').ok_or_else(|| format!("{part}: expected op=rule"))?;
            let v = match v {
                "n" | "nearest" => Round::Nearest,
                "c" | "chop" => Round::Chop,
                "ee" if k == "add" => Round::EeAdder,
                _ => return Err(format!("{part}: rule is nearest, chop or (add only) ee")),
            };
            match k {
                "add" => r.add = v,
                "mul" => r.mul = v,
                "div" => r.div = v,
                "cvt" => r.cvt = v,
                _ => return Err(format!("{part}: op is add, mul, div or cvt")),
            }
        }
        Ok(r)
    }
}

impl Default for Rules {
    /// The best match to the game in PCSX2 so far.
    fn default() -> Rules {
        Rules { add: Round::Nearest, mul: Round::Nearest, div: Round::Chop, cvt: Round::Chop }
    }
}

thread_local! {
    static RULES: Cell<Rules> = Cell::new(Rules::default());
}

/// Set by `Runner::call` from `Runner::float_rules`.
pub(crate) fn set_rules(r: Rules) {
    RULES.with(|c| c.set(r));
}

fn rules() -> Rules {
    RULES.with(|c| c.get())
}

/// A register or memory value as the EE sees it: an exponent of 0 is zero, an exponent of 255 is a large
/// ordinary number (here clamped to the largest float).
pub fn load(x: f32) -> f32 {
    let b = x.to_bits();
    match (b >> 23) & 0xff {
        0 => f32::from_bits(b & 0x8000_0000),
        0xff => f32::from_bits((b & 0x8000_0000) | MAX.to_bits()),
        _ => x,
    }
}

/// Cut an exact result to the EE's float: clamped, denormals to zero, rounded by `how`.
fn round(r: f64, how: Round) -> f32 {
    let neg = r.is_sign_negative();
    let a = r.abs();
    let mag = if a == 0.0 || a < f32::MIN_POSITIVE as f64 {
        0.0
    } else if a >= MAX as f64 {
        MAX
    } else {
        let mut f = a as f32; // nearest
        if how != Round::Nearest && f as f64 > a {
            f = f32::from_bits(f.to_bits() - 1); // one step toward zero
        }
        f
    };
    if neg { -mag } else { mag }
}

fn add_ee(a: f32, b: f32) -> f32 {
    if a == 0.0 {
        return b;
    }
    if b == 0.0 {
        return a;
    }
    let (ba, bb) = (a.to_bits(), b.to_bits());
    let (mut ea, mut eb) = (((ba >> 23) & 0xff) as i32, ((bb >> 23) & 0xff) as i32);
    let (mut ma, mut mb) = (((ba & 0x7f_ffff) | 0x80_0000) as i64, ((bb & 0x7f_ffff) | 0x80_0000) as i64);
    let (mut sa, mut sb) = (ba >> 31, bb >> 31);
    if (eb, mb) > (ea, ma) {
        std::mem::swap(&mut ea, &mut eb);
        std::mem::swap(&mut ma, &mut mb);
        std::mem::swap(&mut sa, &mut sb);
    }
    let shift = ea - eb;
    mb = if shift > 24 { 0 } else { mb >> shift };
    let mut m = if sa == sb { ma + mb } else { ma - mb };
    if m == 0 {
        return 0.0;
    }
    let mut e = ea;
    while m >= 0x100_0000 {
        m >>= 1;
        e += 1;
    }
    while m < 0x80_0000 {
        m <<= 1;
        e -= 1;
    }
    if e >= 255 {
        return if sa == 1 { -MAX } else { MAX };
    }
    if e <= 0 {
        return if sa == 1 { -0.0 } else { 0.0 };
    }
    f32::from_bits((sa << 31) | ((e as u32) << 23) | (m as u32 & 0x7f_ffff))
}

pub fn add(a: f32, b: f32) -> f32 {
    let (a, b) = (load(a), load(b));
    match rules().add {
        Round::EeAdder => add_ee(a, b),
        how => round(a as f64 + b as f64, how),
    }
}

pub fn sub(a: f32, b: f32) -> f32 {
    add(a, -b)
}

pub fn mul(a: f32, b: f32) -> f32 {
    round(load(a) as f64 * load(b) as f64, rules().mul)
}

pub fn div(a: f32, b: f32) -> f32 {
    let (a, b) = (load(a), load(b));
    if b == 0.0 {
        let neg = a.is_sign_negative() != b.is_sign_negative();
        return if neg { -MAX } else { MAX };
    }
    round(a as f64 / b as f64, rules().div)
}

pub fn sqrt(a: f32) -> f32 {
    round((load(a).abs() as f64).sqrt(), rules().div)
}

/// cvt.s.w
pub fn from_int(i: i32) -> f32 {
    round(i as f64, rules().cvt)
}

/// cvt.w.s: toward zero, clamped to the int range.
pub fn to_int(a: f32) -> i32 {
    let a = load(a) as f64;
    if a >= i32::MAX as f64 {
        i32::MAX
    } else if a <= i32::MIN as f64 {
        i32::MIN
    } else {
        a.trunc() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divide_rounds_toward_zero() {
        // 1/3 is 0x3EAAAAAB to nearest, 0x3EAAAAAA toward zero
        assert_eq!(div(1.0, 3.0).to_bits(), 0x3EAA_AAAA);
        assert_eq!(div(-1.0, 3.0).to_bits(), 0xBEAA_AAAA);
    }

    #[test]
    fn rules_switch_rounding() {
        let x = 1.1f32;
        set_rules(Rules::default());
        assert_eq!(mul(x, x).to_bits(), (x * x).to_bits(), "to nearest, as IEEE");
        set_rules(Rules::MANUAL);
        assert!((x as f64) * (x as f64) >= mul(x, x) as f64, "toward zero");
        // the EE adder drops the bits of the smaller operand: 1 + 2^-24 + 2^-24 stays 1
        set_rules(Rules::parse("add=ee").unwrap());
        assert_eq!(add(add(1.0, 2f32.powi(-24)), 2f32.powi(-24)), 1.0);
        assert_eq!(add(1.5, -0.25), 1.25);
        set_rules(Rules::default());
        assert!(Rules::parse("mul=ee").is_err());
    }

    #[test]
    fn clamps_and_flushes() {
        assert_eq!(mul(MAX, 2.0), MAX);
        assert_eq!(mul(-MAX, 2.0), -MAX);
        assert_eq!(div(1.0, 0.0), MAX);
        assert_eq!(div(-1.0, 0.0), -MAX);
        assert_eq!(mul(f32::MIN_POSITIVE, 0.5), 0.0);
        assert_eq!(load(f32::from_bits(1)), 0.0);
        assert_eq!(load(f32::INFINITY), MAX);
        assert_eq!(sqrt(-4.0), 2.0);
    }

    #[test]
    fn conversions() {
        assert_eq!(from_int(16_777_217).to_bits(), 16_777_216f32.to_bits()); // toward zero
        assert_eq!(to_int(-2.9), -2);
        assert_eq!(to_int(3.0e10), i32::MAX);
    }
}

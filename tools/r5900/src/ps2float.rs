//! Floating point as the EE's FPU and VU0 do it, which is not IEEE 754 (EE Core User's Manual, FPU chapter):
//! no infinities, NaNs or denormals. Denormal inputs and results are zero, results too large for a float are
//! clamped to ±0x7F7FFFFF, division by zero gives ±0x7F7FFFFF, and the square root of a negative number is the
//! root of its absolute value.
//!
//! How results are rounded is the open question (board rows F4c, F4f), so it is a setting, [`Rules`]:
//! - [`Rules::Measured`] (the default): add, subtract and multiply round to nearest, divide, square root and
//!   int-to-float round toward zero. This is what matched the game best in PCSX2 (2026-10-10, 24 calls of
//!   `Boarder_ForwardDrag` and `Boarder_SideFriction` captured in a race: 18 bit-exact, 6 one unit in the last
//!   place apart). Rounding add or multiply toward zero matched far worse (3 to 10 exact); divide and int-to-float
//!   made no difference on those calls, so they keep the manual's rule.
//! - [`Rules::Manual`]: everything toward zero, as the manual describes the hardware (3 of the 24 exact).
//!
//! The exact result is formed in f64 and then cut to f32: products and quotients of two floats, and sums of
//! floats within 29 binary orders of each other, are exact or rounded once that way.

use std::cell::Cell;

const MAX: f32 = f32::MAX; // 0x7F7FFFFF

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Rules {
    #[default]
    Measured,
    Manual,
}

thread_local! {
    static RULES: Cell<Rules> = const { Cell::new(Rules::Measured) };
}

/// Set by `Runner::call` from `Runner::float_rules`.
pub(crate) fn set_rules(r: Rules) {
    RULES.with(|c| c.set(r));
}

#[derive(Clone, Copy)]
enum Op {
    AddMul,
    Other,
}

fn to_nearest(op: Op) -> bool {
    matches!((RULES.with(|c| c.get()), op), (Rules::Measured, Op::AddMul))
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

/// Cut an exact result to the EE's float: clamped, denormals to zero, rounded as `op` is under the current rules.
fn round(r: f64, op: Op) -> f32 {
    let neg = r.is_sign_negative();
    let a = r.abs();
    let mag = if a == 0.0 || a < f32::MIN_POSITIVE as f64 {
        0.0
    } else if a >= MAX as f64 {
        MAX
    } else {
        let mut f = a as f32; // nearest
        if !to_nearest(op) && f as f64 > a {
            f = f32::from_bits(f.to_bits() - 1); // one step toward zero
        }
        f
    };
    if neg { -mag } else { mag }
}

pub fn add(a: f32, b: f32) -> f32 {
    round(load(a) as f64 + load(b) as f64, Op::AddMul)
}

pub fn sub(a: f32, b: f32) -> f32 {
    round(load(a) as f64 - load(b) as f64, Op::AddMul)
}

pub fn mul(a: f32, b: f32) -> f32 {
    round(load(a) as f64 * load(b) as f64, Op::AddMul)
}

pub fn div(a: f32, b: f32) -> f32 {
    let (a, b) = (load(a), load(b));
    if b == 0.0 {
        let neg = a.is_sign_negative() != b.is_sign_negative();
        return if neg { -MAX } else { MAX };
    }
    round(a as f64 / b as f64, Op::Other)
}

pub fn sqrt(a: f32) -> f32 {
    round((load(a).abs() as f64).sqrt(), Op::Other)
}

/// cvt.s.w
pub fn from_int(i: i32) -> f32 {
    round(i as f64, Op::Other)
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
    fn add_and_multiply_follow_the_rules() {
        // 1 + 2^-24 lies halfway between two floats; 1.1 * 1.1 is not exact either
        let x = 1.1f32;
        set_rules(Rules::Measured);
        assert_eq!(mul(x, x).to_bits(), (x * x).to_bits(), "to nearest, as IEEE");
        set_rules(Rules::Manual);
        let chopped = mul(x, x).to_bits();
        assert!(chopped == (x * x).to_bits() || chopped == (x * x).to_bits() - 1, "toward zero");
        assert!(((x as f64) * (x as f64)) >= f32::from_bits(chopped) as f64);
        set_rules(Rules::Measured);
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

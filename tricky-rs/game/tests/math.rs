//! The game's math routines against ours, bit for bit (board row E14): `Math_Sin`, `Math_Cos`, `Math_SinCos`,
//! `Math_Asin`, `Math_Atan`, `Math_AngleDiff` and `Math_Hermite` run in the function runner on the player's
//! SLUS_203.26 (TRICKY_ELF; skipped without it) and `tricky_game::math` on the same inputs.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::math;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn one(r: &mut Runner, f: u32, x: f32) -> f32 {
    r.cpu.set_f(12, x);
    r.call(f).unwrap();
    r.cpu.f(0)
}

fn check(name: &str, f: u32, ours: fn(f32) -> f32, xs: &[f32]) {
    let Some(mut r) = runner() else { return };
    for &x in xs {
        let g = one(&mut r, f, x);
        assert_eq!(g.to_bits(), ours(x).to_bits(), "{name}({x}): game {g} ours {}", ours(x));
    }
}

fn angles() -> Vec<f32> {
    let mut g = Rng(0x0005_1e00_0000_0001);
    let mut v: Vec<f32> = (0..20000).map(|_| g.f(-40.0, 40.0)).collect();
    v.extend((-64..=64).map(|k| k as f32 * std::f32::consts::FRAC_PI_4));
    v.extend([0.0, -0.0, 1e-7, -1e-7, 0.5, -0.5]);
    v
}

#[test]
fn sin_is_the_originals() {
    check("Math_Sin", 0x250d60, math::sin, &angles());
}

#[test]
fn cos_is_the_originals() {
    check("Math_Cos", 0x250e98, math::cos, &angles());
}

#[test]
fn sin_cos_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let out = r.mem.alloc(8);
    for x in angles() {
        r.cpu.set_f(12, x);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, (out + 4) as u64);
        r.call(0x250c10).unwrap();
        let (s, c) = math::sin_cos(x);
        assert_eq!(r.mem.read_u32(out), s.to_bits(), "Math_SinCos({x}) sin");
        assert_eq!(r.mem.read_u32(out + 4), c.to_bits(), "Math_SinCos({x}) cos");
    }
}

#[test]
fn asin_is_the_originals() {
    let mut g = Rng(0x0a51_0000_0000_0001);
    let mut v: Vec<f32> = (0..20000).map(|_| g.f(-1.0, 1.0)).collect();
    v.extend([-1.0, 1.0, 0.0, -0.0, 0.70710677, -0.70710677, 0.7071068, -0.7071068, 0.9999999, -0.9999999]);
    check("Math_Asin", 0x250fd0, math::asin, &v);
}

#[test]
fn atan_is_the_originals() {
    let mut g = Rng(0x0a7a_0000_0000_0001);
    let mut v: Vec<f32> = (0..20000).map(|_| g.f(-50.0, 50.0)).collect();
    v.extend((0..2000).map(|_| g.f(-2.0, 2.0)));
    v.extend([-1.0, 1.0, 0.0, -0.0, 1e6, -1e6, 1.0000001, -1.0000001]);
    check("Math_Atan", 0x2510f8, math::atan, &v);
}

#[test]
fn angle_diff_and_hermite_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x0d1f_0000_0000_0001);
    for _ in 0..20000 {
        let (a, b) = (g.f(-30.0, 30.0), g.f(-30.0, 30.0));
        r.cpu.set_f(12, a);
        r.cpu.set_f(13, b);
        r.call(0x1741d0).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), math::angle_diff(a, b).to_bits(), "Math_AngleDiff({a}, {b})");
        let p = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(0.0, 1.0)];
        for (k, x) in p.iter().enumerate() {
            r.cpu.set_f(12 + k, *x);
        }
        r.call(0x1704b0).unwrap();
        let ours = math::hermite(p[0], p[1], p[2], p[3], p[4]);
        assert_eq!(r.cpu.f(0).to_bits(), ours.to_bits(), "Math_Hermite({p:?})");
    }
}

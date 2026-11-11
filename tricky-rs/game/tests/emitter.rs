//! `tricky_game::emitter` against the particle emitter's setters in the function runner (TRICKY_ELF; skipped
//! without it): the emitter words compared after each call.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::emitter::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
    fn v(&mut self) -> V4 {
        [self.f(), self.f(), self.f(), self.f()]
    }
    fn k(&mut self) -> f32 {
        [0.5f32, 1.0, 5.0, 12.0, 0.3, 2.0][(self.next() % 6) as usize]
    }
}

fn put(r: &mut Runner, a: u32, v: V4) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn emitter_setters_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    let eaddr = r.mem.alloc(0x150);
    let va = r.mem.alloc(0x10);
    let vb = r.mem.alloc(0x10);
    let vc = r.mem.alloc(0x10);
    let vd = r.mem.alloc(0x10);
    let mut g = Rng(0x1d50_b0f7_aa31_5c9d);
    let mut e = vec![0u32; 0x54];
    for case in 0..20_000 {
        for w in e.iter_mut() {
            *w = g.f().to_bits();
        }
        e[0x3c / 4] = g.k().to_bits();
        for (i, w) in e.iter().enumerate() {
            r.mem.write_u32(eaddr + 4 * i as u32, *w);
        }
        let (a, b, c, d) = (g.v(), g.v(), g.v(), g.v());
        put(&mut r, va, a);
        put(&mut r, vb, b);
        put(&mut r, vc, c);
        put(&mut r, vd, d);
        let (f12, f13, f14) = (g.f(), g.f(), g.k());
        r.cpu.set_gpr(4, eaddr as u64);
        let which = case % 10;
        match which {
            0 => {
                let n = [1i32, 2, 5, 40][(g.next() % 4) as usize];
                let a2 = [1i32, 7, 3][(g.next() % 3) as usize];
                r.cpu.set_f(12, f12);
                r.cpu.set_f(13, f13);
                r.cpu.set_f(14, f14);
                r.cpu.set_gpr(5, n as u64);
                r.cpu.set_gpr(6, a2 as u64);
                r.cpu.set_gpr(7, va as u64);
                r.call(0x1d50b0).unwrap();
                init(&mut e, f12, f13, f14, n, a2, a);
            }
            1 => {
                r.cpu.set_f(12, f12);
                r.cpu.set_f(13, f13);
                r.call(0x1d52e8).unwrap();
                size_range(&mut e, f12, f13);
            }
            2 => {
                r.cpu.set_f(12, f12);
                r.cpu.set_f(13, f13);
                r.call(0x1d5300).unwrap();
                life_range(&mut e, f12, f13);
            }
            3 => {
                r.cpu.set_gpr(5, va as u64);
                r.cpu.set_gpr(6, vb as u64);
                r.call(0x1d5380).unwrap();
                spawn_line(&mut e, a, b);
            }
            4 => {
                r.cpu.set_gpr(5, va as u64);
                r.cpu.set_gpr(6, vb as u64);
                r.cpu.set_gpr(7, vc as u64);
                r.call(0x1d5418).unwrap();
                spawn_quad(&mut e, a, b, c);
            }
            5 => {
                r.call(0x1d5548).unwrap();
                clear_force(&mut e);
            }
            6 => {
                r.cpu.set_gpr(5, va as u64);
                r.cpu.set_gpr(6, vb as u64);
                r.cpu.set_gpr(7, vc as u64);
                r.cpu.set_gpr(8, vd as u64);
                r.call(0x1d5790).unwrap();
                velocity_basis(&mut e, a, b, c, d);
            }
            7 => {
                r.cpu.set_f(12, f12);
                r.cpu.set_gpr(5, va as u64);
                r.cpu.set_gpr(6, vb as u64);
                r.cpu.set_gpr(7, vc as u64);
                r.cpu.set_gpr(8, vd as u64);
                r.call(0x1d58e0).unwrap();
                colors(&mut e, f12, a, b, c, d);
            }
            8 => {
                r.cpu.set_f(12, f12);
                r.call(0x1d59e0).unwrap();
                set_time(&mut e, f12);
            }
            _ => {
                r.cpu.set_f(12, f12);
                r.call(0x1d59f0).unwrap();
                advance_time(&mut e, f12);
            }
        }
        for (i, w) in e.iter().enumerate() {
            assert_eq!(r.mem.read_u32(eaddr + 4 * i as u32), *w, "case {case} (fn {which}) word {i:#x}");
        }
    }
}

/// `Rand_VuNext` 0x1d5a08 on any word, and `ParticleSys_StepEmitter` 0x1d5a30 (with `_AdvanceTime` 0x1d59f0 and
/// `Rand_VuNext`) on emitters owing 0 to a few steps: the whole emitter compared.
#[test]
fn emitter_step_and_vu_random_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x1d59f0);
    r.allow(0x1d5a08);
    let w = r.mem.alloc(0x10);
    let mut g = Rng(0x1d5a_0830_e1b2_9101);
    for case in 0..50_000u32 {
        let x = match case % 4 {
            0 => g.next() ^ g.next() << 16,
            1 => g.f().to_bits(),
            2 => 0x3f80_0000 | (g.next() & 0x7f_ffff),
            _ => [0, 0xffff_ffff, 0x7f_ffff, 0x0040_0010, 0x3fff_ffff][(g.next() % 5) as usize],
        };
        r.mem.write_u32(w, x);
        r.cpu.set_gpr(4, w as u64);
        r.call(0x1d5a08).unwrap();
        assert_eq!(r.mem.read_u32(w), tricky_game::fxrand::vu_next(x), "Rand_VuNext {x:#010x}");
    }
    let eaddr = r.mem.alloc(0x150);
    let mut e = vec![0u32; 0x54];
    let mut steps = [0u32; 5];
    for case in 0..20_000 {
        for w in e.iter_mut() {
            *w = g.f().to_bits();
        }
        for o in tricky_game::fxrand::EMITTER_FACTORS {
            e[o / 4] = if g.next() % 2 == 0 { 0x3f80_0000 | (g.next() & 0x7f_ffff) } else { g.next() };
        }
        let interval = [0.5f32, 1.0, 1.0 / 30.0, 0.25, 2.0][(g.next() % 5) as usize];
        let count = (g.next() % 8) as i32 - 1;
        e[0] = count as u32;
        e[0xc / 4] = interval.to_bits();
        e[0x8 / 4] = (count as f32 * interval + (g.next() % 400) as f32 / 100.0 - 1.0).to_bits();
        e[0x3c / 4] = g.k().to_bits();
        let dt = [1.0f32 / 60.0, 0.0, 0.1, 1.0 / 30.0][(g.next() % 4) as usize];
        for (i, w) in e.iter().enumerate() {
            r.mem.write_u32(eaddr + 4 * i as u32, *w);
        }
        r.cpu.set_gpr(4, eaddr as u64);
        r.cpu.set_f(12, dt);
        r.call(0x1d5a30).unwrap();
        let before = e[0x28 / 4];
        step(&mut e, dt);
        let n = (0..64).take_while(|&k| (0..k).fold(before, |a, _| tricky_game::fxrand::vu_next(a)) != e[0x28 / 4]).count();
        steps[n.min(4)] += 1;
        for (i, w) in e.iter().enumerate() {
            assert_eq!(r.mem.read_u32(eaddr + 4 * i as u32), *w, "case {case}: word {:#x}", 4 * i);
        }
    }
    eprintln!("emitters stepped 0, 1, 2, 3, 4+ times: {steps:?}");
    assert!(steps.iter().all(|&n| n > 100), "every step count reached: {steps:?}");
}

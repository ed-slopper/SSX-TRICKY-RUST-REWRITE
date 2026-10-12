//! `Career_PrepareRace` 0x163980 (row E1b2j) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random career blocks, the rider-list builder stubbed (it sets the rider count), the real
//! `Rand_Fx` on both sides; the whole block and the random stream compared.

use std::cell::Cell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::career::{Career, PrepareEnv, CAREER_BASE, CAREER_SIZE};
use tricky_game::rand::Streams;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

struct Env {
    n: i32,
    rand: Streams,
}
impl PrepareEnv for Env {
    fn setup_riders_for_mode(&mut self, c: &mut Career) {
        c.w32(0x27dc, self.n as u32);
    }
    fn rand_fx(&mut self) -> u32 {
        self.rand.fx()
    }
}

fn byte(r: &mut Runner, a: u32) -> u8 {
    (r.mem.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

#[test]
fn prepare_race_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x163980u32, 0x1637f8, 0x23d4a8, 0x2401e0, 0x2ce04c, 0x2cc808, 0x2fca28, 0x2cd9d8] {
        r.allow(a);
    }
    let n_now: Rc<Cell<i32>> = Rc::default();
    let nn = n_now.clone();
    r.stub(0x165558, move |c, m| {
        let base = c.gpr(4) as u32;
        m.write_u32(base + 0x27dc, nn.get() as u32);
    });
    let mut g = Rng(0x0bad_cafe_1234_5678);
    let mut seen = [0u32; 3];
    for step in 0..20_000 {
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        for k in 0..4usize {
            let o = 0x44 * k;
            ours.bytes[0x30fc + o] = if g.next() % 4 == 0 { 0xff } else { (g.next() % 4) as u8 };
            ours.w32(0x3110 + o, CAREER_BASE + (0x3118 + o) as u32);
            ours.w32(0x3118 + o, g.next() % 12);
            ours.w32(0x3114 + o, CAREER_BASE + 0x100 + 0x1c4 * (g.next() % 12));
        }
        ours.w32(0x2af8, if g.next() % 3 == 0 { 10 } else { g.next() % 12 });
        let n = (g.next() % 10) as i32 - 2;
        n_now.set(n);
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        let mut rand = Streams::default();
        rand.seed_fx(g.next());
        for (k, w) in rand.fx.s.iter().enumerate() {
            r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
        }
        let mut env = Env { n, rand };
        r.cpu.set_gpr(4, CAREER_BASE as u64);
        r.call(0x163980).unwrap_or_else(|e| panic!("step {step}: {e}"));
        ours.prepare_race(&mut env);
        for k in 0..CAREER_SIZE as u32 {
            assert_eq!(byte(&mut r, CAREER_BASE + k), ours.bytes[k as usize], "step {step}: byte {k:#x} (rider count {n})");
        }
        for (k, w) in env.rand.fx.s.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x345e58 + 4 * k as u32), *w, "step {step}: stream word {k}");
        }
        seen[(n < 1) as usize + 2 * (n >= 6) as usize % 3] += 1;
    }
    assert!(seen.iter().all(|&c| c > 1000), "{seen:?}");
}

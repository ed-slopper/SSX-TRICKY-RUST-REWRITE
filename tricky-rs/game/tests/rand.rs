//! The game's random numbers against the original's, bit for bit (board row E1b1): `Rand_Seed`, `Rand_Next`,
//! `Rand_Fx`, `Rand_Game`, `Rand_GameRange` and `Rand_GameJitter` run in the function runner on the player's
//! SLUS_203.26 (TRICKY_ELF; skipped without it) and `tricky_game::rand` on the same states.

use r5900::Runner;
use tricky_game::rand::{Rand, Streams};

const RAND_NEXT: u32 = 0x2401e0;
const RAND_SEED: u32 = 0x2402c0;
const RAND_FX: u32 = 0x23d4a8;
const RAND_GAME: u32 = 0x23d4d8;
const RAND_GAME_RANGE: u32 = 0x23d4f8;
const RAND_GAME_JITTER: u32 = 0x23d560;
const FX_STATE: u32 = 0x345e58;
const GAME_STATE: u32 = 0x345e70;

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    for f in [RAND_NEXT, RAND_GAME, RAND_GAME_RANGE] {
        r.allow(f);
    }
    Some(r)
}

fn put(r: &mut Runner, at: u32, s: &Rand) {
    for (i, w) in s.s.iter().enumerate() {
        r.mem.write_u32(at + i as u32 * 4, *w);
    }
}

fn get(r: &mut Runner, at: u32) -> Rand {
    let mut s = Rand::default();
    for i in 0..6 {
        s.s[i] = r.mem.read_u32(at + i as u32 * 4);
    }
    s
}

/// States that walk every carry: random ones, and ones with the low words about to wrap.
fn states() -> Vec<Rand> {
    let mut x = 0x1234_5678_9abc_def1u64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 16) as u32
    };
    let mut v: Vec<Rand> = (0..400).map(|_| Rand { s: [next(), next(), next(), next(), next(), next()] }).collect();
    for k in 0..6 {
        // s5 at the wrap with the k words above it all ones: the counter's carry climbs k words
        let mut s = [next(), next(), next(), next(), next(), u32::MAX];
        for w in s.iter_mut().skip(5 - k).take(k) {
            *w = u32::MAX;
        }
        v.push(Rand { s });
        v.push(Rand { s: [u32::MAX; 6] });
        v.push(Rand { s: [0; 6] });
    }
    v
}

#[test]
fn seed_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let at = r.mem.alloc(24);
    for x in [0u32, 1, 0x0dd2_f1aa, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 12345, 0x9e37_79b9] {
        r.cpu.set_gpr(4, at as u64);
        r.cpu.set_gpr(5, x as i32 as i64 as u64);
        r.call(RAND_SEED).unwrap();
        assert_eq!(get(&mut r, at), Rand::seeded(x), "Rand_Seed({x:#x})");
    }
}

#[test]
fn next_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let at = r.mem.alloc(24);
    let mut draws = 0;
    for start in states() {
        put(&mut r, at, &start);
        let mut ours = start;
        for _ in 0..8 {
            r.cpu.set_gpr(4, at as u64);
            r.call(RAND_NEXT).unwrap();
            let v = ours.draw();
            assert_eq!(r.cpu.gpr(2) as u32, v, "Rand_Next from {start:x?}");
            assert_eq!(get(&mut r, at), ours, "Rand_Next state from {start:x?}");
            draws += 1;
        }
    }
    assert!(draws > 3000);
}

#[test]
fn streams_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut ours = Streams::default();
    ours.seed_fx(0x0bad_cafe);
    ours.seed_game(0x0001_0203);
    put(&mut r, FX_STATE, &ours.fx);
    put(&mut r, GAME_STATE, &ours.game);
    for i in 0..2000 {
        let (addr, v) = if i % 3 == 0 { (RAND_FX, ours.fx()) } else { (RAND_GAME, ours.game()) };
        r.call(addr).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32, v, "draw {i}");
    }
    assert_eq!(get(&mut r, FX_STATE), ours.fx);
    assert_eq!(get(&mut r, GAME_STATE), ours.game);
}

#[test]
fn range_and_jitter_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut ours = Streams::default();
    ours.seed_game(77);
    put(&mut r, GAME_STATE, &ours.game);
    let pairs = [(0.0f32, 1.0f32), (-1.0, 1.0), (250.0, 400.0), (-3.5, 12.25), (1e-3, 7e4), (5.0, -5.0)];
    for i in 0..3000 {
        let (a, b) = pairs[i % pairs.len()];
        r.cpu.set_f(12, a);
        r.cpu.set_f(13, b);
        let (addr, v) = if i % 2 == 0 { (RAND_GAME_RANGE, ours.game_range(a, b)) } else { (RAND_GAME_JITTER, ours.game_jitter(a, b)) };
        r.call(addr).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), v.to_bits(), "call {i} ({a}, {b}): game {} ours {v}", r.cpu.f(0));
    }
    assert_eq!(get(&mut r, GAME_STATE), ours.game);
}

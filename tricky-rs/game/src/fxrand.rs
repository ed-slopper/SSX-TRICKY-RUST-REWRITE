//! The effects' random draws that are not the world's (board row E1b2e): the particle emitters' nine random
//! factors (`ParticleSys_RandomizeFactors` 0x1d5180, stepped by `ParticleSys_StepEmitter` 0x1d5a30 on the vector
//! unit's random register through `Rand_VuNext` 0x1d5a08) and the snow layers (`SnowFall_InitLayers` 0x1c8ec8).
//! Notes: `docs/port-notes/world.md`, "Random numbers".
//!
//! The draw order is read from the decompiled code (proven). `vu_next` is the vector unit's R register step
//! (`VRINIT` then `VRNEXT`) as the EE manual and PCSX2 define it; it is not checked against the original here
//! (the function runner has no `VRNEXT`; E1b2g).

/// A `Rand_Fx` word as the float `1.0 + u`, `u` in [0, 1): `((r & 0x7fffff | 0x3f800000) − 1.0) + 1.0`. Exact
/// (the subtraction and the addition lose nothing in [1, 2)), so the bits are the word's mantissa under an
/// exponent of 0.
pub fn one_plus_unit(r: u32) -> u32 {
    r & 0x7f_ffff | 0x3f80_0000
}

/// `Rand_VuNext` 0x1d5a08 on one word: `VRINIT` (the low 23 bits under the exponent of 1.0) then `VRNEXT`
/// (shift left, feed back bit 22 xor bit 4, keep 23 bits, exponent of 1.0 again).
pub fn vu_next(state: u32) -> u32 {
    let r = state & 0x7f_ffff;
    let bit = ((r >> 22) ^ (r >> 4)) & 1;
    0x3f80_0000 | ((r << 1) & 0x7f_ffff) | bit
}

/// The emitter words `ParticleSys_RandomizeFactors` fills, as byte offsets in the emitter, in draw order.
pub const EMITTER_FACTORS: [usize; 9] = [0x28, 0x2c, 0x30, 0x34, 0x38, 0x40, 0x44, 0x48, 0x4c];

/// `ParticleSys_RandomizeFactors` 0x1d5180: nine draws, each a float in [1, 2), returned as bits in
/// [`EMITTER_FACTORS`] order.
pub fn emitter_factors(rand_fx: &mut dyn FnMut() -> u32) -> [u32; 9] {
    let mut out = [0u32; 9];
    for o in out.iter_mut() {
        *o = one_plus_unit(rand_fx());
    }
    out
}

/// `ParticleSys_StepEmitter` 0x1d5a30: for each whole emission step the emitter owes, every factor takes one
/// `Rand_VuNext`, in the order of [`EMITTER_FACTORS`]; `steps` is the number of steps the time loop runs.
pub fn step_factors(f: &mut [u32; 9], steps: u32) {
    for _ in 0..steps {
        for w in f.iter_mut() {
            *w = vu_next(*w);
        }
    }
}

/// One snow layer's four random words (floats in [1, 2) as bits), indexed by layer word 4, 5, 6, 7 (they are
/// drawn in the order 4, 6, 7, 5; `SnowFall_InitLayers`).
pub type SnowLayer = [u32; 4];

/// `SnowFall_InitLayers` 0x1c8ec8: seven layers, each four draws. The rest of a layer is fixed by the manager's
/// constants (`cSnowFallMan` 0x1c8db8: +0x4b0 = 0.1875, +0x4b4 = 0.5, +0x4b8 = 350.0, +0x4bc = 2.0, +0x4c0 = 3.0).
pub fn snow_layers(rand_fx: &mut dyn FnMut() -> u32) -> [SnowLayer; 7] {
    let mut out = [[0u32; 4]; 7];
    for l in out.iter_mut() {
        // drawn into words 4, 6, 7, 5 in that order
        let w4 = one_plus_unit(rand_fx());
        let w6 = one_plus_unit(rand_fx());
        let w7 = one_plus_unit(rand_fx());
        let w5 = one_plus_unit(rand_fx());
        *l = [w4, w5, w6, w7];
    }
    out
}

/// What `cGame::Init` 0x17e148 draws for the snow after `ParticleMan_Init`, in this order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSnow {
    /// `cGame+0x74`: snow falls in this run.
    pub on: bool,
    pub layers: [SnowLayer; 7],
    /// `cGame+0x78`, given to `SnowFall_SetDensity`: 0.05 + 0.15 u.
    pub density: f32,
    /// `FUN_001c91c0`'s two arguments (`+0x4bc`, `+0x4c0` of the snow manager): 5 + u and 7 − u.
    pub fall: (f32, f32),
}

/// The snow's draws in `cGame::Init`: one draw for whether it snows (only when the game mode is neither 2 nor 4,
/// the race has at most one rider and fewer than three entries: `unit < chance`, `chance` = the course record's
/// float at +0x54), then the 28 layer draws of the `cSnowFallMan` constructor, then the density, then the two
/// fall values. `mode` is `DAT_0032de8c`.
pub fn game_init_snow(
    rand_fx: &mut dyn FnMut() -> u32,
    mode: i32,
    riders: i32,
    entries: i32,
    snow_chance: f32,
) -> GameSnow {
    let on = if mode != 2 && mode != 4 && riders <= 1 && entries < 3 {
        crate::rand::Rand::unit(rand_fx()) + 0.0 < snow_chance
    } else {
        false
    };
    let layers = snow_layers(rand_fx);
    let density = crate::rand::Rand::unit(rand_fx()) * 0.15 + 0.05;
    let a = crate::rand::Rand::unit(rand_fx());
    let b = crate::rand::Rand::unit(rand_fx());
    GameSnow { on, layers, density, fall: (a + 0.0 + 5.0, 7.0 - (b + 0.0)) }
}

/// `Crowd_LoadTexturesRandomize` 0x1458e0: for every sub-item of every instance whose model flag bit 0 is set (in
/// instance order), one draw, `r & 15`, picks one of the sixteen crowd textures.
pub fn crowd_texture_pick(r: u32) -> usize {
    (r & 0xf) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factors_are_floats_in_one_two() {
        let mut n = 0u32;
        let mut r = move || {
            n = n.wrapping_add(0x1234_5679);
            n
        };
        for b in emitter_factors(&mut r) {
            let f = f32::from_bits(b);
            assert!((1.0..2.0).contains(&f));
        }
    }

    #[test]
    fn one_plus_unit_matches_the_float_expression() {
        for r in [0u32, 1, 0x7f_ffff, 0x1234_5678, 0xffff_ffff, 0x8000_0000] {
            let u = f32::from_bits(r & 0x7f_ffff | 0x3f80_0000) - 1.0;
            assert_eq!((u + 1.0).to_bits(), one_plus_unit(r));
        }
    }

    #[test]
    fn draw_order_is_the_codes() {
        let mut k = 0u32;
        let mut r = || {
            k += 1;
            k
        };
        let l = snow_layers(&mut r);
        // layer 0 drew 1, 2, 3, 4 into words 4, 6, 7, 5
        assert_eq!(l[0], [one_plus_unit(1), one_plus_unit(4), one_plus_unit(2), one_plus_unit(3)]);
        assert_eq!(l[6][0], one_plus_unit(25));
    }

    #[test]
    fn vu_register_stays_a_float_in_one_two_and_moves() {
        let mut s = 0x3f80_0001u32;
        for _ in 0..1000 {
            let n = vu_next(s);
            assert!((1.0..2.0).contains(&f32::from_bits(n)));
            assert_ne!(n, s);
            s = n;
        }
    }

    #[test]
    fn snow_draws_in_the_inits_order() {
        let mut k = 0u32;
        let mut count = 0;
        let mut r = || {
            k = k.wrapping_add(0x0100_0001);
            count += 1;
            k
        };
        let g = game_init_snow(&mut r, 0, 1, 1, 1.0);
        assert!(g.on); // every unit is below 1.0
        assert_eq!(count, 1 + 28 + 3);
        assert!((0.05..0.2).contains(&g.density));
        assert!((5.0..6.0).contains(&g.fall.0) && (6.0..=7.0).contains(&g.fall.1));
        // no chance draw in a race of three or in mode 4
        let mut n = 0;
        let mut r2 = || {
            n += 1;
            1
        };
        let g = game_init_snow(&mut r2, 4, 1, 1, 1.0);
        assert!(!g.on);
        assert_eq!(n, 28 + 3);
    }

    #[test]
    fn step_applies_each_factor_in_turn() {
        let mut f = [0x3f80_0001u32; 9];
        step_factors(&mut f, 3);
        let mut g = 0x3f80_0001u32;
        for _ in 0..3 {
            g = vu_next(g);
        }
        assert!(f.iter().all(|&x| x == g));
    }
}

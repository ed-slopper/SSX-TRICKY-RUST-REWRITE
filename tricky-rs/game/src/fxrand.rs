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

/// `Music_StartCourseSong` 0x214e60's segment pick: `(r & 0x7fff) * n / 0x7fff`, 0..=n (the top value only when the
/// low 15 bits are all set). The sound system's virtual rand (`cBXAudio::vf3` 0x21e6c0) is `Rand_Fx`.
pub fn segment_pick(r: u32, n: u32) -> u32 {
    (r & 0x7fff) * n / 0x7fff
}

/// `Audio_OnRaceGo` 0x215288 in game mode 6 (the lesson), in place of starting the race music: one `Rand_Fx` draw, an
/// unsigned remainder by 100 (`divu`), is the Pathfinder variable (`Music_SetPathVar`). Read at 0x2152dc.
pub fn lesson_path_var(rand_fx: u32) -> u32 {
    rand_fx % 100
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

/// `Speech_RollChance` 0x235d84: whether a speech line of `kind` plays. The level is the speech system's float
/// (+0x383c, truncated) held under its cap (+0x6e68); the chance table (0x3a3528, ten words a row, `kind` the row,
/// 0..=1024 for never..always) is read at that level, and `Rand_Fx & 0x3ff` must be below it. `table` is the
/// table from its start as words. (An index outside the table is an out-of-range read in the game: `None`.)
pub fn speech_roll(rand_fx: u32, level: f32, cap: i32, kind: usize, table: &[i32]) -> Option<bool> {
    let mut i = level as i32;
    if cap < i {
        i = cap;
    }
    let at = kind as i64 * 10 + i as i64;
    let v = *table.get(usize::try_from(at).ok()?)?;
    Some(((rand_fx & 0x3ff) as i32) < v)
}

/// `FUN_00224658` 0x224658: which of a sound bank's `count` variants is loaded (`FUN_002256c0` 0x2256c0 loads that
/// one): `Rand_Fx % count`, an unsigned remainder (`divu`). A count of 0 is a divide trap in the game.
pub fn bank_variant(rand_fx: u32, count: i32) -> i32 {
    (rand_fx % count as u32) as i32
}

/// The roster's rider id that means "no rider" in `FUN_00169378` (a fixed empty look, no draw).
pub const NO_RIDER: i32 = 0x1ad;

/// The tables `FUN_00169378` reads, each from its address: `a` 0x3cc028 (ten bytes a rider id: six signed bytes, a
/// halfword base at +6, a halfword variant count at +8), `b` 0x32f468 (twelve bytes a look: six halfwords), `c`
/// 0x37cd70 (500 entries of eight bytes: a halfword key, a halfword value, a word).
pub struct SceneTables<'a> {
    pub a: &'a [u8],
    pub b: &'a [u8],
    pub c: &'a [u8],
}

/// One of a rider's six look slots (the words at +0x340 + 0x10 k of the scene record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub d: i32,
}

/// A rider's scene record (100 bytes at +0x33c + 100 i): the word the roster gives (+0x33c) and six slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RiderScene {
    pub first: i32,
    pub slots: [Slot; 6],
}

fn h(t: &[u8], at: usize) -> i32 {
    i16::from_le_bytes([t[at], t[at + 1]]) as i32
}

/// `FUN_00169378` 0x169378 (`Career_FinalizeRoster`'s per-rider look picks, 15 riders): `ids` are the riders'
/// look ids (+0x14), `firsts` their first words (+0x50). A rider with [`NO_RIDER`] gets the empty look in every slot
/// (13, 0x300, 0x1bf, -1) and draws nothing; any other draws one `Rand_Fx % variants` (or takes `forced` when it is
/// not negative), notes it in `picks` (+0x8c) and fills the six slots from the tables. A slot whose key is not in
/// table `c` keeps its old `c` and `d`; one that is in it gets that entry's value and word. Panics on a variant count of 0 (the game traps).
pub fn rider_scene(
    ids: &[i32; 15],
    firsts: &[i32; 15],
    forced: i32,
    rand_fx: &mut dyn FnMut() -> u32,
    t: &SceneTables,
    scenes: &mut [RiderScene; 15],
    picks: &mut [i32; 15],
) {
    for i in 0..15 {
        let id = ids[i];
        scenes[i].first = firsts[i];
        if id == NO_RIDER {
            for s in scenes[i].slots.iter_mut() {
                *s = Slot { a: 0xd, b: 0x300, c: 0x1bf, d: -1 };
            }
            continue;
        }
        let row = id as usize * 10;
        let count = h(t.a, row + 8);
        let mut idx = (rand_fx() % count as u32) as i32; // divu
        let mut look = h(t.a, row + 6) + idx;
        if forced >= 0 {
            look = h(t.a, row + 6) + forced;
            idx = forced;
        }
        picks[i] = idx;
        for k in 0..6 {
            let key = h(t.b, k * 2 + look as usize * 12);
            let s = &mut scenes[i].slots[k];
            s.a = t.a[row + k] as i8 as i32;
            s.b = key;
            for e in 0..500 {
                if h(t.c, e * 8) as u16 as i32 == key && key >= 0 {
                    s.c = u16::from_le_bytes([t.c[e * 8 + 2], t.c[e * 8 + 3]]) as i32;
                    s.d = i32::from_le_bytes([t.c[e * 8 + 4], t.c[e * 8 + 5], t.c[e * 8 + 6], t.c[e * 8 + 7]]);
                    break;
                }
            }
        }
    }
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
    fn segment_pick_reaches_n_only_on_all_ones() {
        assert_eq!(segment_pick(0, 3), 0);
        assert_eq!(segment_pick(0x7ffe, 3), 2);
        assert_eq!(segment_pick(0x7fff, 3), 3);
        assert_eq!(segment_pick(0xffff_8000, 4), 0); // only the low 15 bits count
        assert_eq!(segment_pick(0x5fff, 4), 2);
        assert_eq!(segment_pick(0x6000, 4), 3);
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
    #[test]
    fn speech_roll_reads_the_row_and_level() {
        let mut table = vec![0i32; 30];
        table[10 + 3] = 512;
        assert_eq!(speech_roll(511, 3.9, 9, 1, &table), Some(true));
        assert_eq!(speech_roll(512, 3.9, 9, 1, &table), Some(false));
        assert_eq!(speech_roll(0x400 + 5, 7.0, 3, 1, &table), Some(true)); // held at the cap: 3
        assert_eq!(speech_roll(0, 0.0, 0, 5, &table), None);
        assert_eq!(speech_roll(0, -1.0, 9, 0, &table), None);
    }

    #[test]
    fn lesson_path_var_is_an_unsigned_remainder() {
        assert_eq!(lesson_path_var(0xffff_ffff), 95);
        assert_eq!(lesson_path_var(250), 50);
    }

    #[test]
    fn bank_variant_is_an_unsigned_remainder() {
        assert_eq!(bank_variant(10, 3), 1);
        assert_eq!(bank_variant(0xffff_fff9, 4), 1); // 4294967289 % 4
    }
}

/// The scene table a (0x3cc028; ten bytes per id, `SceneTables::a`) as `FUN_00168360` (run by `StaticInit_RaceScene`)
/// builds it from the game's data: `ids` the 901 signed shorts at 0x37ab50, `rows` the twenty six-byte rows at
/// 0x37b260 and `counts` the twenty shorts at 0x37b2d8 (how many of the ids each row serves). For each position
/// in `ids`, an id that differs from the one before gets its entry: the six bytes of the current row, the position
/// (short at +6) and a count of 1 (short at +8); an id repeating the one before adds one to that count.
pub fn scene_table_a(ids: &[u8], rows: &[u8], counts: &[u8]) -> Vec<u8> {
    let short = |b: &[u8], i: usize| i16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
    let n = ids.len() / 2;
    let top = (0..n).map(|i| short(ids, i).max(0) as usize).max().unwrap_or(0);
    let mut t = vec![0u8; 10 * (top + 1).max(0x800)];
    let (mut cnt, mut row, mut prev) = (0i64, 0usize, 0x1adi64);
    for i in 0..n {
        cnt += 1;
        let id = short(ids, i) as i64;
        if (short(counts, row) as i64) < cnt {
            cnt = 1;
            row += 1;
        }
        let at = (id * 10) as usize;
        if id == prev {
            let c = i16::from_le_bytes([t[at + 8], t[at + 9]]).wrapping_add(1);
            t[at + 8..at + 10].copy_from_slice(&c.to_le_bytes());
        } else {
            t[at..at + 6].copy_from_slice(&rows[6 * row..6 * row + 6]);
            t[at + 6..at + 8].copy_from_slice(&(i as i16).to_le_bytes());
            t[at + 8..at + 10].copy_from_slice(&1i16.to_le_bytes());
        }
        prev = id;
    }
    t
}

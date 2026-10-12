//! The game's random numbers (board row E1b1): one generator, six 32-bit words of state, and two streams of it,
//! `Rand_Fx` (effects, front end, scene picks, career shuffles) and `Rand_Game` (AI decisions, stumbles, path picks;
//! seeded at race init and saved in the replay snapshot). `tests/rand.rs` checks every function here bit for bit
//! against the original in the function runner. Notes: `docs/port-notes/world.md`, "Random numbers".

/// The generator's state: the six words `Rand_Next` works on, in memory order (s0 first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rand {
    pub s: [u32; 6],
}

impl Rand {
    /// `Rand_Seed` 0x2402c0: every word is the seed plus its own constant.
    pub const fn seeded(x: u32) -> Self {
        Self {
            s: [
                x.wrapping_sub(0x0dd2_f1aa),
                x.wrapping_sub(0x77ce_d917),
                x.wrapping_sub(0x39db_22d1),
                x.wrapping_add(0x0702_c49c),
                x.wrapping_sub(0x61ca_c083),
                x.wrapping_add(0x6fdf_3b64),
            ],
        }
    }

    /// `Rand_Seed` 0x2402c0 on an existing state.
    pub fn seed(&mut self, x: u32) {
        *self = Self::seeded(x);
    }

    /// `Rand_Next` 0x2401e0: s4 += s5, then s3, s2, s1, s0 each add the word below and its carry; s5 counts up,
    /// carrying into s4, s3, s2, s1 and s0 while each wraps. Returns s0.
    pub fn draw(&mut self) -> u32 {
        let s = &mut self.s;
        let s5 = s[5];
        let s4 = s5.wrapping_add(s[4]);
        // the first add's carry is tested against both words, as the original does
        let c = (s4 < s5 || s4 < s[4]) as u32;
        s[4] = s4;
        let old3 = s[3];
        let s3 = s4.wrapping_add(old3).wrapping_add(c);
        s[3] = s3;
        let old2 = s[2];
        let s2 = s3.wrapping_add(old2).wrapping_add((s3 < old3) as u32);
        s[2] = s2;
        let old1 = s[1];
        let s1 = s2.wrapping_add(old1).wrapping_add((s2 < old2) as u32);
        s[1] = s1;
        let s0 = s1.wrapping_add(s[0]).wrapping_add((s1 < old1) as u32);
        s[0] = s0;
        s[5] = s5.wrapping_add(1);
        if s[5] == 0 {
            s[4] = s[4].wrapping_add(1);
            if s[4] == 0 {
                s[3] = s[3].wrapping_add(1);
                if s[3] == 0 {
                    s[2] = s[2].wrapping_add(1);
                    if s[2] == 0 {
                        s[1] = s[1].wrapping_add(1);
                        if s[1] == 0 {
                            s[0] = s[0].wrapping_add(1);
                        }
                    }
                }
            }
        }
        s[0]
    }

    /// A draw as a float in [0, 1): the low 23 bits as the mantissa of a number in [1, 2), minus 1 (the way
    /// `Rand_GameRange` and the effects code turn a draw into a float).
    pub fn unit(r: u32) -> f32 {
        f32::from_bits(r & 0x007f_ffff | 0x3f80_0000) - 1.0
    }

    /// The 24 bytes `Rand_SaveSnapshot` 0x2401b0 writes to the replay (the state words, little-endian).
    pub fn snapshot(&self) -> [u8; 24] {
        let mut b = [0u8; 24];
        for (i, w) in self.s.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        b
    }
}

/// The game's two streams (state at 0x345e58 and 0x345e70 in the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Streams {
    pub fx: Rand,
    pub game: Rand,
}

impl Streams {
    /// `Rand_SeedFx` 0x23d498
    pub fn seed_fx(&mut self, x: u32) {
        self.fx.seed(x);
    }
    /// `Rand_SeedGame` 0x23d4c8
    pub fn seed_game(&mut self, x: u32) {
        self.game.seed(x);
    }
    /// `Rand_Fx` 0x23d4a8
    pub fn fx(&mut self) -> u32 {
        self.fx.draw()
    }
    /// `Rand_Game` 0x23d4d8
    pub fn game(&mut self) -> u32 {
        self.game.draw()
    }
    /// `Rand_GameRange` 0x23d4f8: lo + (hi − lo)·u, u in [0, 1) from one `Rand_Game` draw.
    pub fn game_range(&mut self, lo: f32, hi: f32) -> f32 {
        let u = Rand::unit(self.game());
        lo + (hi - lo) * u
    }
    /// `Rand_GameJitter` 0x23d560: centre + spread·`Rand_GameRange`(−1, 1).
    pub fn game_jitter(&mut self, centre: f32, spread: f32) -> f32 {
        let r = self.game_range(-1.0, 1.0);
        centre + spread * r
    }
    /// `Rand_SaveGameSnapshot` 0x23d5b0
    pub fn game_snapshot(&self) -> [u8; 24] {
        self.game.snapshot()
    }
}

/// The seed the port gives `Rand_SeedGame` at a race's start. STANDIN: the game seeds the game stream with `DAT_0032de90`,
/// which `cAI::vf15_1114d0` 0x1114d0 sets to a draw of `Rand_Fx` when the race is entered (0x111520), so it depends on
/// everything the front end drew before; the port has no such history.
pub const RACE_SEED: u32 = 0x1d2c_3b4a;

/// The seed `cGame::Init` 0x17e148 gives `Rand_SeedGame` (0x17e3f0) for the draws made while the course's objects are
/// built: `DAT_0032de94`. Nothing in the executable writes that word (its only reference is this read; the image has
/// zero there, in the zero-filled data after the event setup words 0x32de88..0x32de98), so it is 0 on a fresh start
/// (E1b2e). Unless a store through a base register not seen by the cross-references exists, which would show as a
/// different first draw in a recording of the first frames.
pub const GAME_INIT_SEED: u32 = 0;

/// The game's two streams are globals (state at 0x345e58 and 0x345e70): every consumer draws from the same one, so the
/// order of the draws across the riders and the systems is part of the result. Until a race is entered the game stream
/// is the one `RACE_SEED` gives.
static STREAMS: std::sync::Mutex<Streams> = std::sync::Mutex::new(Streams { fx: Rand::seeded(RACE_SEED), game: Rand::seeded(RACE_SEED) });

/// The global streams, locked for the caller's draws.
pub fn streams() -> std::sync::MutexGuard<'static, Streams> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Rand_Game` 0x23d4d8 on the global stream.
pub fn rand_game() -> u32 {
    streams().game()
}

/// `Rand_Fx` 0x23d4a8 on the global stream (effects, world animations, debris, crowds, front-end picks).
pub fn rand_fx() -> u32 {
    streams().fx()
}

/// `Rand_SeedGame` 0x23d4c8 on the global stream, as the race init does it (`cAI::vf14_111108` 0x111108, 0x1112dc).
pub fn seed_game(x: u32) {
    streams().seed_game(x);
}


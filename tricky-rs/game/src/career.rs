//! The career (board row E12): the game's profile, the block at 0x32b390 the memory card saves, kept here as the
//! same bytes at the same offsets (so our save file is the block), with the game's functions on it.
//! E12a: a new game (`Career_InitNewGame` 0x1617a0, `Profile_ResetNewGame` 0x161b40, `Career_DefaultPlayerName`
//! 0x163358, `Career_InitCharacterRecord` 0x166600, `HighScore_Reset` 0x1668f0, `FUN_00166c70`, the audio settings
//! `Profile_SyncAudioSettings` 0x161928 / `Profile_ApplyAudioSettings` 0x161a98). Checked in the function runner
//! (`tests/career.rs`).

use crate::rand::Streams;
use crate::rivals::ATTITUDE;
use crate::stats::{average_groups, BASE};

/// The bytes the career block reaches (the player slots' attributes end at +0x38e0)
pub const CAREER_SIZE: usize = 0x3900;

/// The four starting characters' indices (0x3352c0, 0x3352c4, 0x3352cc, 0x3352c8 in the exe: Mac, Moby, Elise and
/// Eddie), whose bits make the unlocked-character mask (+0xc).
pub const STARTERS: [u32; 4] = [3, 4, 0, 7];

/// Each rider's entry words +0x28 and +0x30 (`DAT_00331ea8 + rider · 0x38`), copied into the character's record.
pub const RIDER_WORDS: [(u32, u32); 12] = [(1, 0), (0, 1), (0, 2), (0, 3), (1, 4), (0, 5), (1, 6), (1, 7), (0, 7), (1, 7), (0, 7), (0, 7)];

/// The points each of a rider's twelve boards needs (the same for every rider: `DAT_00332154 + rider · 0xf0 +
/// board · 0x14`); a board needing none is unlocked at the start.
pub const BOARD_POINTS: [u32; 12] = [0, 0, 5, 20, 35, 60, 90, 120, 160, 200, 240, 999];

/// The default high-score entries `Profile_ResetNewGame` writes (offset of the entry, name, score): per course
/// (13, 0x150 apart from +0x16c4) three tables 0x70 apart of five 0x14-byte entries (+0 the name, +0xc the
/// character, +0x10 the score).
pub const DEFAULT_RECORDS: [(u32, &str, u32); 165] = [
    (0x16c4, "SOZ", 22500),
    (0x16d8, "PONCH", 24000),
    (0x16ec, "CALEB", 25500),
    (0x1700, "ALEC", 27000),
    (0x1714, "JASHA", 30000),
    (0x1734, "SOZ", 150000),
    (0x1748, "PONCH", 125000),
    (0x175c, "CALEB", 100000),
    (0x1770, "ALEC", 75000),
    (0x1784, "JASHA", 50000),
    (0x17a4, "SOZ", 24000),
    (0x17b8, "PONCH", 24000),
    (0x17cc, "CALEB", 24000),
    (0x17e0, "ALEC", 24000),
    (0x17f4, "JASHA", 24000),
    (0x1814, "VIKI", 17500),
    (0x1828, "CAT", 19000),
    (0x183c, "NAOMI", 21000),
    (0x1850, "AKIRA K", 23000),
    (0x1864, "KAHN", 25000),
    (0x1884, "VIKI", 200000),
    (0x1898, "CAT", 125000),
    (0x18ac, "NAOMI", 100000),
    (0x18c0, "AKIRA K", 75000),
    (0x18d4, "KAHN", 60000),
    (0x18f4, "VIKI", 17500),
    (0x1908, "CAT", 17500),
    (0x191c, "NAOMI", 17500),
    (0x1930, "AKIRA K", 17500),
    (0x1944, "KAHN", 17500),
    (0x1964, "HYCO", 32500),
    (0x1978, "CONOR", 34000),
    (0x198c, "CHUCKY", 35500),
    (0x19a0, "NATZ", 37000),
    (0x19b4, "LUCAS", 38500),
    (0x19d4, "HYCO", 250000),
    (0x19e8, "CONOR", 215000),
    (0x19fc, "CHUCKY", 185000),
    (0x1a10, "NATZ", 100000),
    (0x1a24, "LUCAS", 65000),
    (0x1a44, "HYCO", 34500),
    (0x1a58, "CONOR", 34500),
    (0x1a6c, "CHUCKY", 34500),
    (0x1a80, "NATZ", 34500),
    (0x1a94, "LUCAS", 34500),
    (0x1ab4, "EASE3", 20000),
    (0x1ac8, "FRESHG", 21000),
    (0x1adc, "EVB", 23500),
    (0x1af0, "MAKAYLA", 25000),
    (0x1b04, "TRACY", 27000),
    (0x1b24, "EASE3", 250000),
    (0x1b38, "FRESHG", 225000),
    (0x1b4c, "EVB", 165000),
    (0x1b60, "MAKAYLA", 155000),
    (0x1b74, "TRACY", 125000),
    (0x1b94, "EASE3", 22500),
    (0x1ba8, "FRESHG", 22500),
    (0x1bbc, "EVB", 22500),
    (0x1bd0, "MAKAYLA", 22500),
    (0x1be4, "TRACY", 22500),
    (0x1c04, "QUANTUM", 27000),
    (0x1c18, "WHARGOUL", 28000),
    (0x1c2c, "JOSH", 29500),
    (0x1c40, "PJ", 30500),
    (0x1c54, "MADELEINE", 32000),
    (0x1c74, "QUANTUM", 200000),
    (0x1c88, "WHARGOUL", 150000),
    (0x1c9c, "JOSH", 120000),
    (0x1cb0, "PJ", 75000),
    (0x1cc4, "MADELEINE", 50000),
    (0x1ce4, "QUANTUM", 27500),
    (0x1cf8, "WHARGOUL", 27500),
    (0x1d0c, "JOSH", 27500),
    (0x1d20, "PJ", 27500),
    (0x1d34, "MADELEINE", 27500),
    (0x1d54, "YOURPAL", 15500),
    (0x1d68, "JJ", 17000),
    (0x1d7c, "SMURRAY", 18000),
    (0x1d90, "ABBEY", 19500),
    (0x1da4, "TOAD", 21000),
    (0x1dc4, "YOURPAL", 175000),
    (0x1dd8, "JJ", 160000),
    (0x1dec, "SMURRAY", 100000),
    (0x1e00, "ABBEY", 80000),
    (0x1e14, "TOAD", 75000),
    (0x1e34, "YOURPAL", 16500),
    (0x1e48, "JJ", 16500),
    (0x1e5c, "SMURRAY", 16500),
    (0x1e70, "ABBEY", 16500),
    (0x1e84, "TOAD", 16500),
    (0x1ea4, "ANNIE", 7711),
    (0x1eb8, "THE PLUM", 7712),
    (0x1ecc, "TAZ", 7713),
    (0x1ee0, "TBJ", 7714),
    (0x1ef4, "SPINTACK", 7715),
    (0x1f14, "ANNIE", 400000),
    (0x1f28, "THE PLUM", 350000),
    (0x1f3c, "TAZ", 275000),
    (0x1f50, "TBJ", 240000),
    (0x1f64, "SPINTACK", 225000),
    (0x1f84, "ANNIE", 7711),
    (0x1f98, "THE PLUM", 7711),
    (0x1fac, "TAZ", 7711),
    (0x1fc0, "TBJ", 7711),
    (0x1fd4, "SPINTACK", 7711),
    (0x1ff4, "BOB", 7711),
    (0x2008, "AIR JER", 7712),
    (0x201c, "RIPPER", 7713),
    (0x2030, "RIDESGOOD", 7714),
    (0x2044, "YAKBAR", 7715),
    (0x2064, "BOB", 200000),
    (0x2078, "AIR JER", 160000),
    (0x208c, "RIPPER", 140000),
    (0x20a0, "RIDESGOOD", 120000),
    (0x20b4, "YAKBAR", 100000),
    (0x20d4, "BOB", 7711),
    (0x20e8, "AIR JER", 7711),
    (0x20fc, "RIPPER", 7711),
    (0x2110, "RIDESGOOD", 7711),
    (0x2124, "YAKBAR", 7711),
    (0x2144, "LIL LAR", 25000),
    (0x2158, "SPB", 26500),
    (0x216c, "JD", 28000),
    (0x2180, "LB", 30000),
    (0x2194, "TBBJ", 32000),
    (0x21b4, "LIL LAR", 150000),
    (0x21c8, "SPB", 100000),
    (0x21dc, "JD", 80000),
    (0x21f0, "LB", 65000),
    (0x2204, "TBBJ", 50000),
    (0x2224, "LIL LAR", 27000),
    (0x2238, "SPB", 27000),
    (0x224c, "JD", 27000),
    (0x2260, "LB", 27000),
    (0x2274, "TBBJ", 27000),
    (0x2294, "PRETTYBOY", 7711),
    (0x22a8, "JESTER", 7712),
    (0x22bc, "SILVERSRF", 7713),
    (0x22d0, "SICKBOY", 7714),
    (0x22e4, "NEOPOLITN", 7715),
    (0x2304, "PRETTYBOY", 200000),
    (0x2318, "JESTER", 190000),
    (0x232c, "SILVERSRF", 180000),
    (0x2340, "SICKBOY", 170000),
    (0x2354, "NEOPOLITN", 160000),
    (0x2374, "PRETTYBOY", 7711),
    (0x2388, "JESTER", 7711),
    (0x239c, "SILVERSRF", 7711),
    (0x23b0, "SICKBOY", 7711),
    (0x23c4, "NEOPOLITN", 7711),
    (0x2534, "BORANATOR", 29500),
    (0x2548, "SITO", 30500),
    (0x255c, "LVT SWEET", 31500),
    (0x2570, "BUCKNER", 33000),
    (0x2584, "CHAISE", 34500),
    (0x25a4, "BORANATOR", 300000),
    (0x25b8, "SITO", 285000),
    (0x25cc, "LVT SWEET", 225000),
    (0x25e0, "BUCKNER", 175000),
    (0x25f4, "CHAISE", 150000),
    (0x2614, "BORANATOR", 29500),
    (0x2628, "SITO", 29500),
    (0x263c, "LVT SWEET", 29500),
    (0x2650, "BUCKNER", 29500),
    (0x2664, "CHAISE", 29500),
];

/// The sound manager as the audio settings read and set it (`DAT_00343050`; none in a build without sound).
pub trait AudioSettings {
    /// `SndMgr_GetVolume`(channel 0..4), the speech level (+0x383c), `FUN_0021e688` (the output mode)
    fn volume(&self, channel: u32) -> f32;
    fn speech(&self) -> f32;
    fn mode(&self) -> u32;
    /// `SndMgr_SetVolume`, `Audio_SetSpeechLevel`, `FUN_00212270`
    fn set_volume(&mut self, channel: u32, v: f32);
    fn set_speech(&mut self, v: f32);
    fn set_mode(&mut self, m: u32);
}

/// The career block.
#[derive(Clone, PartialEq, Eq)]
pub struct Career {
    pub bytes: Vec<u8>,
}

impl Default for Career {
    fn default() -> Self {
        Career { bytes: vec![0; CAREER_SIZE] }
    }
}

impl std::fmt::Debug for Career {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Career({} bytes)", self.bytes.len())
    }
}

impl Career {
    pub fn r32(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.bytes[o..o + 4].try_into().unwrap())
    }
    pub fn w32(&mut self, o: usize, v: u32) {
        self.bytes[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn rf(&self, o: usize) -> f32 {
        f32::from_bits(self.r32(o))
    }
    pub fn wf(&mut self, o: usize, v: f32) {
        self.w32(o, v.to_bits());
    }
    pub fn w8(&mut self, o: usize, v: u8) {
        self.bytes[o] = v;
    }
    /// a C string copy (the name and its terminator)
    fn wstr(&mut self, o: usize, s: &str) {
        self.bytes[o..o + s.len()].copy_from_slice(s.as_bytes());
        self.bytes[o + s.len()] = 0;
    }

    /// `Career_InitNewGame` 0x1617a0: a new profile (`Profile_ResetNewGame`, then `Career_UpdateAttributes`, which
    /// with no player slots set does nothing), +0x325c 0, +0x3260 1. `language` is gApp+0x58.
    pub fn init_new_game(&mut self, language: u32, rand: &mut Streams, audio: Option<&mut dyn AudioSettings>) {
        self.reset_new_game(language, rand, audio);
        self.w32(0x325c, 0);
        self.w32(0x3260, 1);
    }

    /// `Profile_ResetNewGame` 0x161b40
    pub fn reset_new_game(&mut self, language: u32, rand: &mut Streams, mut audio: Option<&mut dyn AudioSettings>) {
        self.bytes[8..8 + 0x27d4].fill(0);
        // the options
        for (o, v) in [(0x1670, 2u32), (0x16ac, 1), (0x1678, 1), (0x167c, 1), (0x1680, 1), (0x16bc, 1), (0x16a4, 1), (0x16a8, 1)] {
            self.w32(o, v);
        }
        self.wf(0x16b4, 0.5);
        for (o, v) in [(0x16b8, 6u32), (0x16c0, 5), (0x1684, 0), (0x1688, 0), (0x169c, 0), (0x16a0, 0), (0x16b0, 0)] {
            self.w32(o, v);
        }
        for o in [0x1698, 0x1694, 0x1690, 0x168c] {
            self.w32(o, 0);
        }
        self.default_player_names(language);
        // the four player slots: none
        for k in 0..4 {
            let b = 0x44 * k;
            self.w8(0x30fc + b, 0xff);
            self.w32(0x3104 + b, 0xffff_ffff);
            self.w32(0x3108 + b, 0);
            self.w32(0x3110 + b, 0);
            self.w32(0x3114 + b, 0);
            self.w32(0x3100 + b, 0);
        }
        for r in 0..12 {
            self.init_character_record(r);
        }
        for t in 0..13 {
            let b = 0x16c4 + 0x150 * t;
            self.reset_high_score(b, 0, 1, 5, rand);
            self.reset_high_score(b + 0x70, 1, 0, 5, rand);
            self.reset_high_score(b + 0xe0, 0, 1, 1, rand);
        }
        for (o, name, score) in DEFAULT_RECORDS {
            self.wstr(o as usize, name);
            self.w32(o as usize + 0x10, score);
        }
        self.w32(8, 0x407);
        self.w32(0x3260, 2);
        self.w32(0x36fc, 0);
        let [a, b, c, d] = STARTERS;
        self.w32(0xc, 1 << (a & 31) | 1 << (b & 31) | 1 << (d & 31) | 1 << (c & 31));
        for k in 0..12 {
            let b = 0x1c4 * k;
            for j in 0..13 {
                self.w32(0x198 + b + 4 * j, 0);
                self.w32(0x1fc + b + 4 * j, 0xffff_ffff);
                self.w32(0x230 + b + 4 * j, 0xffff_ffff);
            }
            for (o, v) in [(0x164, 0xd), (0x16c, 0xd), (0x174, 0xd), (0x160, 0), (0x17c, 0xd), (0x168, 0), (0x180, 0), (0x184, 0xd), (0x170, 0), (0x188, 0), (0x18c, 0xd), (0x194, 0xd), (0x178, 0), (0x190, 0)] {
                self.w32(o + b, v);
            }
        }
        self.sync_audio(&mut audio);
        self.apply_audio(&mut audio);
        self.w32(0x2b14, 0);
        self.w8(0x3714, 0);
        self.w32(4, 0);
        for k in 0..13 {
            let b = 0x1c4 * k;
            self.w32(0x114 + b, k as u32);
            self.w32(0x118 + b, 0);
            self.w32(0x11c + b, 0);
        }
    }

    /// `Career_DefaultPlayerName` 0x163358: "PLAYER1".."PLAYER4" (languages 0, 1, 4), "SPIELER%d" (2), "JOUEUR%d"
    /// (3) at +0x1630, 10 bytes apart; another language leaves them.
    pub fn default_player_names(&mut self, language: u32) {
        let fmt = match language {
            0 | 1 | 4 => "PLAYER",
            2 => "SPIELER",
            3 => "JOUEUR",
            _ => return,
        };
        for i in 0..4 {
            self.wstr(0x1630 + 10 * i, &format!("{fmt}{}", i + 1));
        }
    }

    /// `Career_InitCharacterRecord` 0x166600(rider): its short record at +0x10 (the rider, its entry's words +0x28
    /// and +0x30), and its record at +0x100 + rider · 0x1c4: thirteen course bytes (3, 3, 0, 0; the first 3, 3, 1, 1),
    /// the boards needing no points unlocked (+0x58), +0x5c 1, the group averages (+0xc) and their levels
    /// (×0.4 + 0.5, +0x10), and how much each rider dislikes this one (+0x164 + 8j: 0 and the attitude).
    pub fn init_character_record(&mut self, r: usize) {
        let s = 0x10 + 0x14 * r;
        let (w28, w30) = RIDER_WORDS[r];
        for (k, v) in [r as u32, w28, w30, 0, 0].iter().enumerate() {
            self.w32(s + 4 * k, *v);
        }
        let p = 0x100 + 0x1c4 * r;
        for i in 0..13 {
            let q = p + 4 * i;
            self.w8(q + 0x26, 0);
            self.w8(q + 0x27, 0);
            self.w8(q + 0x25, 3);
            self.w8(q + 0x24, 3);
        }
        self.w8(p + 0x27, 1);
        self.w8(p + 0x26, 1);
        self.w32(p + 4, 0);
        self.w32(p + 8, 0);
        self.w32(p, 0);
        self.w32(p + 0x58, 0);
        self.w32(p + 0x5c, 0);
        let mut mask = 0;
        for (k, pts) in BOARD_POINTS.iter().enumerate() {
            if *pts == 0 {
                mask |= 1 << k;
            }
        }
        self.w32(p + 0x58, mask);
        self.w32(p + 0x5c, 1);
        let avg = average_groups(&BASE[r]);
        self.bytes[p + 0xc..p + 0x10].copy_from_slice(&avg);
        for g in 0..4 {
            self.w8(p + 0x10 + g, (self.bytes[p + 0xc + g] as f32 * 0.4 + 0.5) as i32 as u8);
        }
        for j in 0..12 {
            self.w32(p + 0x164 + 8 * j, 0);
            self.w32(p + 0x168 + 8 * j, ATTITUDE[j][r] as u32);
        }
        self.w32(0x3258, 1);
    }

    /// `HighScore_Reset` 0x1668f0(table, a, b, count): +0x68 a, +0x6c b, +0x64 count; all five entries emptied
    /// (name, score 0) with a random character (`Rand_Fx` % 12, unsigned).
    pub fn reset_high_score(&mut self, t: usize, a: u32, b: u32, count: u32, rand: &mut Streams) {
        self.w32(t + 0x68, a);
        self.w32(t + 0x6c, b);
        self.w32(t + 0x64, count);
        self.w8(t, 0);
        let mut e = t;
        loop {
            self.w32(e + 0x10, 0);
            self.w32(e + 0xc, rand.fx() % 12);
            e += 0x14;
            if t + 100 <= e {
                break;
            }
            self.w8(e, 0);
        }
    }

    /// `Profile_SyncAudioSettings` 0x161928: the sound manager's volumes, speech level and mode into +0x1658..+0x1674
    /// (each change marks the profile changed, +0x3258); no sound manager, as it was.
    pub fn sync_audio(&mut self, audio: &mut Option<&mut dyn AudioSettings>) -> u32 {
        let Some(a) = audio.as_mut() else { return self.r32(0x3258) };
        for (ch, o) in [(0, 0x1658), (1, 0x165c), (2, 0x1660)] {
            let v = a.volume(ch);
            if v != self.rf(o) {
                self.wf(o, v);
                self.w32(0x3258, 1);
            }
        }
        let s = a.speech();
        if s != self.rf(0x1664) {
            self.wf(0x1664, s);
            self.w32(0x3258, 1);
        }
        for (ch, o) in [(3, 0x1668), (4, 0x166c)] {
            let v = a.volume(ch);
            if v != self.rf(o) {
                self.wf(o, v);
                self.w32(0x3258, 1);
            }
        }
        let m = a.mode();
        if m != self.r32(0x1674) {
            self.w32(0x1674, m);
            self.w32(0x3258, 1);
        }
        self.r32(0x3258)
    }

    /// `Profile_ApplyAudioSettings` 0x161a98: the profile's volumes, speech level and mode to the sound manager,
    /// the profile unchanged (+0x3258 0); then `Career_UpdateAttributes` (nothing with no player slots: E12b).
    pub fn apply_audio(&mut self, audio: &mut Option<&mut dyn AudioSettings>) {
        if let Some(a) = audio.as_mut() {
            for (ch, o) in [(0, 0x1658), (1, 0x165c), (2, 0x1660), (3, 0x1668), (4, 0x166c)] {
                a.set_volume(ch, self.rf(o));
            }
            a.set_speech(self.rf(0x1664));
            a.set_mode(self.r32(0x1674));
        }
        self.w32(0x3258, 0);
    }
}

//! The career (board row E12): the game's profile, the block at 0x32b390 the memory card saves, kept here as the
//! same bytes at the same offsets (so our save file is the block), with the game's functions on it.
//! E12a: a new game (`Career_InitNewGame` 0x1617a0, `Profile_ResetNewGame` 0x161b40, `Career_DefaultPlayerName`
//! 0x163358, `Career_InitCharacterRecord` 0x166600, `HighScore_Reset` 0x1668f0, `FUN_00166c70`, the audio settings
//! `Profile_SyncAudioSettings` 0x161928 / `Profile_ApplyAudioSettings` 0x161a98). Checked in the function runner
//! (`tests/career.rs`).

use crate::rand::Streams;
use crate::rivals::ATTITUDE;
use crate::stats::{average_groups, BASE};

/// The bytes the career block reaches: the player slots' attributes end at +0x3910, then the new-record list
/// (0x32eca0, five 0x10 entries)
pub const CAREER_SIZE: usize = 0x3960;

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

/// The game address of the career block (0x32b390): its pointers into itself are this plus the offset.
pub const CAREER_BASE: u32 = 0x32b390;

/// `Crc_Compute16` 0x2ce04c: CRC-16/ARC (reflected 0xa001) from 0xeafb, four bytes a step, as the game's two
/// byte tables (0x34f0f8 low, 0x34f1f8 high) give it.
pub fn crc16(data: &[u8]) -> u32 {
    let table = |i: usize| {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xa001 } else { c >> 1 };
        }
        c
    };
    let lo = |i: u32| table(i as usize) & 0xff;
    let hi = |i: u32| table(i as usize) >> 8;
    let (mut a, mut b) = (0xfbu32, 0xeau32);
    let mut p = 0;
    let mut n = data.len() as i32;
    while 0 <= n - 4 {
        let d = |k: usize| data[p + k] as u32;
        a = d(1) ^ lo(d(0) ^ b) ^ a;
        let b1 = (d(2) ^ lo(a) ^ hi(d(0) ^ b)) & 0xff;
        let b2 = (d(3) ^ lo(b1) ^ hi(a)) & 0xff;
        a = hi(b2);
        b = (lo(b2) ^ hi(b1)) & 0xff;
        p += 4;
        n -= 4;
    }
    loop {
        n -= 1;
        if n < 0 {
            break;
        }
        let x = data[p] as u32 ^ b;
        b = lo(x) ^ a;
        p += 1;
        a = hi(x);
    }
    b | a << 8
}

/// The modes' masks `FUN_00163b38` reads (0x379fa8, by the event mode +0x2afc)
pub const MODE_MASKS: [u32; 10] = [0x400, 0x400, 0x4c0, 0x480, 0x4c0, 0x4c0, 0x1ff, 0x4c0, 0x400, 0];

/// What a player slot's attributes read out (`Career_GetAttributes` 0x165468: four groups of the base level, the
/// level now, the board's tenth and the cap plus it).
pub type Attributes = [[u32; 4]; 4];

impl Career {
    pub fn r8(&self, o: usize) -> u8 {
        self.bytes[o]
    }
    fn ri(&self, o: isize) -> u32 {
        self.r32(o as usize)
    }
    /// A pointer the block holds, as an offset in it
    pub fn ptr(&self, o: usize) -> usize {
        self.r32(o).wrapping_sub(CAREER_BASE) as usize
    }
    /// `Career_FindPlayerSlot` 0x166550: the slot (0..3) holding player `p`, or −1
    pub fn find_player_slot(&self, p: i8) -> i32 {
        (0..4).find(|&k| self.bytes[0x30fc + 0x44 * k] as i8 == p).map_or(-1, |k| k as i32)
    }

    /// The offset of slot `k` (−1 the one before the first, as the game reads it)
    fn so(k: i32) -> isize {
        0x44 * k as isize
    }

    /// `Career_GetPlayerId` 0x166580
    pub fn player_id(&self, slot: usize) -> i8 {
        self.bytes[0x30fc + 0x44 * slot] as i8
    }

    /// `Career_IsPlayerActive` 0x161900
    pub fn is_player_active(&self, slot: usize) -> bool {
        0 <= self.player_id(slot)
    }

    /// `Career_FindRiderOfPlayer` 0x166598: the race rider (+0x27dc count, 0x84 each, +0x69 the player) of the
    /// player in slot `slot`, or −1
    pub fn find_rider_of_player(&self, slot: usize) -> i32 {
        let p = self.player_id(slot);
        let n = self.r32(0x27dc) as i32;
        (0..n.max(0)).find(|&i| self.bytes[0x2845 + 0x84 * i as usize] as i8 == p).unwrap_or(-1)
    }

    /// `FUN_001664e8`: the rider player `p`'s slot last raced with (+0x312c, when +0x3104 says it was this player);
    /// 0xc for no player, −1 for another
    pub fn slot_last_rider(&self, p: i32) -> i32 {
        if p < 0 {
            return 0xc;
        }
        let k = Self::so(self.find_player_slot(p as i8));
        if self.ri(0x3104 + k) as i32 == p {
            self.ri(0x312c + k) as i32
        } else {
            -1
        }
    }

    /// `RiderSetup_Init` 0x16bba0 (the circuit's rider set-up at +0x320c): the slot, and the career, race and
    /// circuit tables' addresses (constants: 0x32b398, 0x32db6c, 0x32e5f4)
    pub fn rider_setup_init(&mut self, slot_addr: u32) {
        self.w32(0x320c + 0x10, slot_addr);
        self.w32(0x320c + 0x18, 0);
        self.w32(0x320c + 4, 0x32b398);
        self.w32(0x320c + 8, 0x32db6c);
        self.w32(0x320c + 0xc, 0x32e5f4);
        self.w32(0x320c + 0x14, 0);
    }

    /// `Career_AssignPlayerRider` 0x1635b8(slot, player, rider): slot −1 is the first free (the first if it is free;
    /// past the fourth when all are taken, as the game does); the slot's profile is the one it raced with when that
    /// was this rider, else the rider's short record; the player, its options word (+0x168c + 4·player), its
    /// character record, profile and name; flags cleared; `RiderSetup_Init`; `Career_UpdateAttributes`.
    pub fn assign_player_rider(&mut self, slot: i32, player: i32, rider: i32, cheats: &crate::cheats::Cheats) {
        let mut s = slot;
        if s < 0 {
            s = 0;
            if 0 <= self.bytes[0x30fc] as i8 {
                s = 1;
                while s < 4 && 0 <= self.bytes[0x30fc + 0x44 * s as usize] as i8 {
                    s += 1;
                }
            }
        }
        let k = 0x44 * s as usize;
        if self.slot_last_rider(player) == rider {
            for w in 0..5 {
                let v = self.r32(0x312c + k + 4 * w);
                self.w32(0x3118 + k + 4 * w, v);
            }
        } else {
            let r = 0x10 + 0x14 * rider as isize;
            for w in 0..5 {
                let v = self.ri(r + 4 * w as isize);
                self.w32(0x3118 + k + 4 * w, v);
            }
        }
        if 0 <= player {
            self.bytes[0x30fc + k] = player as u8;
        }
        let opt = self.ri(0x168c + 4 * player as isize);
        self.w32(0x3100 + k, opt);
        self.w32(0x3114 + k, CAREER_BASE.wrapping_add((rider * 0x1c4 + 0x100) as u32));
        self.w32(0x3110 + k, CAREER_BASE + (k + 0x3118) as u32);
        self.w32(0x3108 + k, CAREER_BASE + (10 * s as usize + 0x1630) as u32);
        // (cleared in the call's delay slot: before the set-up, which slot 4 overlaps)
        self.w32(0x310c + k, 0);
        self.rider_setup_init(CAREER_BASE + (k + 0x30fc) as u32);
        self.update_attributes(cheats);
    }

    /// `Career_ClearPlayerSlot` 0x1637d0
    pub fn clear_player_slot(&mut self, slot: usize) {
        let k = 0x44 * slot;
        self.bytes[0x30fc + k] = 0xff;
        self.w32(0x3114 + k, 0);
        self.w32(0x3110 + k, 0);
    }

    /// `Career_SetPlayerRiderSlot` 0x164950
    pub fn set_player_slot_id(&mut self, slot: usize, player: u8) {
        self.bytes[0x30fc + 0x44 * slot] = player;
    }

    /// `Career_UpdateAttributes` 0x166a58: for each slot in use, the points (record +0) and their level
    /// (×0.4 + 0.5) for spending, the four group levels (record +0x10..) as base and now, the caps from the rider's
    /// (×0.4 + 0.5); the max-attributes cheat puts every group at its cap and leaves no points; the points to spend
    /// are at most what the caps leave.
    pub fn update_attributes(&mut self, cheats: &crate::cheats::Cheats) {
        let max = cheats.max_attrib;
        for k in 0..4 {
            let s = 0x44 * k;
            if (self.bytes[0x30fc + s] as i8) < 0 {
                continue;
            }
            let rec = self.ptr(0x3114 + s);
            let pts = self.r32(rec);
            self.w32(0x3820 + 4 * k, pts);
            let lvl = (pts as i32 as f32 * 0.4 + 0.5) as i32 as u32;
            self.w32(0x3830 + 4 * k, lvl);
            self.w32(0x3840 + 4 * k, lvl);
            if max != 0 {
                self.w32(0x3830 + 4 * k, 0);
                self.w32(0x3840 + 4 * k, 0);
            }
            let rider = self.r32(self.ptr(0x3110 + s)) as usize;
            let caps = crate::stats::CAPS[rider.min(11)];
            let mut left = 0i32;
            for i in 0..4 {
                let b = self.bytes[rec + 0x10 + i] as u32;
                self.w32(0x38d0 + 0x10 * k + 4 * i, b);
                self.w32(0x3850 + 0x10 * k + 4 * i, b);
                let cap = (caps[i] as f32 * 0.4 + 0.5) as i32 as u32;
                self.w32(0x3890 + 0x10 * k + 4 * i, cap);
                if max != 0 {
                    self.w32(0x3850 + 0x10 * k + 4 * i, cap);
                    self.w32(0x38d0 + 0x10 * k + 4 * i, cap);
                }
                left = left.wrapping_add(self.r32(0x3890 + 0x10 * k + 4 * i).wrapping_sub(self.r32(0x3850 + 0x10 * k + 4 * i)) as i32);
            }
            if left < self.r32(0x3840 + 4 * k) as i32 {
                self.w32(0x3840 + 4 * k, left as u32);
                self.w32(0x3830 + 4 * k, left as u32);
            }
        }
    }

    /// The record of the slot of player `p` (+0x3114's pointer) and the slot's offset
    fn player_rec(&self, p: i8) -> (usize, isize) {
        let k = Self::so(self.find_player_slot(p));
        (self.ri(0x3114 + k).wrapping_sub(CAREER_BASE) as usize, k)
    }

    /// `Career_GetAttributePoints` 0x164368
    pub fn attribute_points(&self, p: i8) -> u32 {
        let k = self.find_player_slot(p) as isize;
        self.ri(0x3830 + 4 * k)
    }

    /// Spending or refunding changes the record: its points (+0) from what is left to spend, its group byte
    /// (+0xc + i) from the level against the cap times the rider's cap.
    fn rescale(&mut self, k: isize, i: usize) {
        let rec = self.ri(0x3114 + 0x44 * k).wrapping_sub(CAREER_BASE) as usize;
        let rider = self.r32(self.ri(0x3110 + 0x44 * k).wrapping_sub(CAREER_BASE) as usize) as usize;
        let pts = self.ri(0x3830 + 4 * k) as i32 as f32;
        let all = self.ri(0x3840 + 4 * k) as i32 as f32;
        let base = self.ri(0x3820 + 4 * k) as i32 as f32;
        self.w32(rec, ((pts / all) * base) as i32 as u32);
        let cur = self.ri(0x3850 + 0x10 * k + 4 * i as isize) as i32 as f32;
        let cap = self.ri(0x3890 + 0x10 * k + 4 * i as isize) as i32 as f32;
        let c = crate::stats::CAPS[rider.min(11)][i] as f32;
        self.bytes[rec + 0xc + i] = ((cur / cap) * c) as i32 as u8;
    }

    /// `Career_SpendAttributePoint` 0x164398(player, group): a point into a group below its cap
    pub fn spend_attribute_point(&mut self, p: i8, i: usize) -> bool {
        let k = self.find_player_slot(p) as isize;
        let pts = self.ri(0x3830 + 4 * k) as i32;
        if 0 < pts {
            let cur_at = (0x3850 + 0x10 * k + 4 * i as isize) as usize;
            let cap = self.ri(0x3890 + 0x10 * k + 4 * i as isize) as i32;
            if (self.r32(cur_at) as i32) < cap {
                self.w32((0x3830 + 4 * k) as usize, (pts - 1) as u32);
                let rec = self.ri(0x3114 + 0x44 * k).wrapping_sub(CAREER_BASE) as usize;
                self.w32(cur_at, self.r32(cur_at).wrapping_add(1));
                self.bytes[rec + 0x10 + i] = self.bytes[rec + 0x10 + i].wrapping_add(1);
                self.rescale(k, i);
                return true;
            }
        }
        false
    }

    /// `Career_RefundAttributePoint` 0x1644e8(player, group): a point back from a group above its base
    pub fn refund_attribute_point(&mut self, p: i8, i: usize) -> bool {
        let k = self.find_player_slot(p) as isize;
        let cur_at = (0x3850 + 0x10 * k + 4 * i as isize) as usize;
        let refund = (self.ri(0x38d0 + 0x10 * k + 4 * i as isize) as i32) < self.r32(cur_at) as i32;
        if refund {
            let pts_at = (0x3830 + 4 * k) as usize;
            self.w32(pts_at, self.r32(pts_at).wrapping_add(1));
            let rec = self.ri(0x3114 + 0x44 * k).wrapping_sub(CAREER_BASE) as usize;
            self.w32(cur_at, self.r32(cur_at).wrapping_sub(1));
            self.bytes[rec + 0x10 + i] = self.bytes[rec + 0x10 + i].wrapping_sub(1);
            self.rescale(k, i);
        }
        refund
    }

    /// `Career_GetAttributes` 0x165468(slot of player, board's bonus bytes): per group the base level, the level
    /// now (the cap with the cheat), the board's bonus / 10 and the cap plus it.
    pub fn attributes(&self, p: i8, bonus: [u8; 4], cheats: &crate::cheats::Cheats) -> Attributes {
        let k = self.find_player_slot(p) as isize;
        let rec = self.ri(0x30fc + 0x44 * k + 0x18).wrapping_sub(CAREER_BASE) as usize;
        let mut out = [[0u32; 4]; 4];
        for i in 0..4 {
            let cap = self.ri(0x3890 + 0x10 * k + 4 * i as isize);
            let tenth = (bonus[i] as f32 * 0.1) as i32 as u32;
            out[i] = [self.ri(0x38d0 + 0x10 * k + 4 * i as isize), self.bytes[rec + 0x10 + i] as u32, tenth, cap.wrapping_add(tenth)];
            if cheats.max_attrib != 0 {
                out[i][1] = cap;
            }
        }
        out
    }

    /// `Career_GetPlayerLevel` 0x164840: the level (0..9) from the record's +4 + +8 (points earned)
    pub fn player_level(&self, p: i8) -> u32 {
        let (rec, _) = self.player_rec(p);
        let v = self.r32(rec + 4).wrapping_add(self.r32(rec + 8)) as i32;
        match v {
            v if 0xf0 <= v => 0,
            v if 200 <= v => 1,
            v if 0xa0 <= v => 2,
            v if 0x78 <= v => 3,
            v if 0x5a <= v => 4,
            v if 0x3c <= v => 5,
            v if 0x23 <= v => 6,
            v if 0x14 <= v => 7,
            v if 5 <= v => 8,
            v if 0 <= v => 9,
            _ => 0,
        }
    }

    /// `Career_GetPlayerLevelStringId` 0x164730: the level's string (0x17..0x20; 0 below 0)
    pub fn player_level_string(&self, p: i8) -> u32 {
        match self.player_level(p) {
            0 => {
                let (rec, _) = self.player_rec(p);
                if (self.r32(rec + 4).wrapping_add(self.r32(rec + 8)) as i32) < 0 {
                    0
                } else {
                    0x17
                }
            }
            l => 0x17 + l,
        }
    }

    /// `Career_IsCharacterUnlocked` 0x163b10
    pub fn is_character_unlocked(&self, c: u32, cheats: &crate::cheats::Cheats) -> bool {
        cheats.unlock_everything != 0 || (self.r32(0xc) as i32 >> (c & 31)) & 1 != 0
    }

    /// `FUN_00163b38`: the mode's mask (`MODE_MASKS` by +0x2afc) has bit `b`
    pub fn mode_has(&self, b: u32) -> bool {
        let m = self.r32(0x2afc) as usize;
        let mask = *MODE_MASKS.get(m).unwrap_or(&0);
        (mask as i32 >> (b & 31)) & 1 != 0
    }

    /// `Career_IsUnlockBitSet` 0x163d30(player, bit): the player's record's boards (+0x58)
    pub fn is_board_unlocked(&self, p: i8, b: u32, cheats: &crate::cheats::Cheats) -> bool {
        if cheats.unlock_everything != 0 {
            return true;
        }
        let (rec, _) = self.player_rec(p);
        (self.r32(rec + 0x58) as i32 >> (b & 31)) & 1 != 0
    }

    /// `Career_IsUnlocked` 0x163d98(player, bit): the record's +0x5c (the outfits)
    pub fn is_outfit_unlocked(&self, p: i8, b: u32, cheats: &crate::cheats::Cheats) -> bool {
        if cheats.unlock_everything != 0 {
            return true;
        }
        let (rec, _) = self.player_rec(p);
        (self.r32(rec + 0x5c) as i32 >> (b & 31)) & 1 != 0
    }

    /// `Career_GetCourseMedal` 0x163e28(mode, course): the first slot's medal byte for the course (race mode 4:
    /// +0x25, show-off 5: +0x24), else 3 (none)
    pub fn course_medal(&self, mode: i32, course: i32) -> u8 {
        if 0 <= self.bytes[0x30fc] as i8 {
            let rec = self.ptr(0x3114) as isize;
            if mode == 4 {
                return self.bytes[(rec + 4 * course as isize + 0x25) as usize];
            }
            if mode == 5 {
                return self.bytes[(rec + 4 * course as isize + 0x24) as usize];
            }
        }
        3
    }

    /// `Career_ResetPlayerRecord` 0x163e70(player): the player's character record made new, its profile the short
    /// record, `Career_UpdateAttributes`
    pub fn reset_player_record(&mut self, p: i8, cheats: &crate::cheats::Cheats) {
        let k = Self::so(self.find_player_slot(p));
        let prof = self.ri(0x3110 + k).wrapping_sub(CAREER_BASE) as usize;
        let rider = self.r32(prof) as usize;
        self.init_character_record(rider);
        for w in 0..5 {
            let v = self.r32(0x10 + 0x14 * rider + 4 * w);
            self.w32((0x3118 + k) as usize + 4 * w, v);
        }
        self.update_attributes(cheats);
    }

    /// `TrickBook_IsTrickDone` 0x164258 on the player's book (record +0x14: +4 done, +8 forced)
    pub fn trick_done(&self, p: i8, t: u32) -> bool {
        let (rec, _) = self.player_rec(p);
        let book = rec + 0x14;
        if (self.r32(book + 8) as i32 >> (t & 31)) & 1 != 0 {
            return true;
        }
        (self.r32(book + 4) as i32 >> (t & 31)) & 1 != 0
    }

    /// `TrickBook_CountDoneInPage` 0x164178 (five a page, `TrickBook_PageFirstTrick` 5·page, 0x1e for page 6, 0
    /// past it)
    pub fn tricks_done_in_page(&self, p: i8, page: u32) -> u32 {
        let first = if page <= 6 { 5 * page } else { 0 };
        (0..5).filter(|i| self.trick_done(p, first + i)).count() as u32
    }

    /// `TrickBook_CountCompletePages` 0x164060
    pub fn complete_pages(&self, p: i8) -> u32 {
        let mut n = 0;
        while self.tricks_done_in_page(p, n) == 5 {
            n += 1;
        }
        n
    }

    /// `TrickBook_FirstUndoneTrick` 0x1641f8
    pub fn first_undone_trick(&self, p: i8) -> i32 {
        (0..0x1e).find(|&t| !self.trick_done(p, t)).map_or(-1, |t| t as i32)
    }

    /// `Career_GetPlayerFlags` +0x310c and `Career_CountPlayerFlags` 0x163f88
    pub fn count_player_flags(&self, p: i8) -> u32 {
        let k = Self::so(self.find_player_slot(p));
        self.ri(0x310c + k).count_ones()
    }

    /// `Career_ClearPlayerFlags` 0x165400(player): this race's flags taken off the record's (+0x1c) and cleared
    pub fn clear_player_flags(&mut self, p: i32) {
        if p < 0 {
            return;
        }
        let k = Self::so(self.find_player_slot(p as i8));
        if (self.bytes[(0x30fc + k) as usize] as i8) < 0 {
            return;
        }
        let flags = self.ri(0x310c + k);
        let rec = self.ri(0x3114 + k).wrapping_sub(CAREER_BASE) as usize;
        // (a player with no slot reads the bytes before the first: a pointer outside the block is not followed)
        if rec + 0x20 <= self.bytes.len() {
            self.w32(rec + 0x1c, self.r32(rec + 0x1c) & !flags);
        }
        self.w32((0x310c + k) as usize, 0);
    }

    /// `Career_ClearAllPlayerFlags` 0x161840: the rider set-up's +0x14 cleared, every slot's flags
    pub fn clear_all_player_flags(&mut self) {
        self.w32(0x320c + 0x14, 0);
        for k in 0..4 {
            let p = self.player_id(k) as i32;
            self.clear_player_flags(p);
        }
    }

    /// `Career_CountProfiles` 0x164288: players 0..3 with a slot and a profile
    pub fn count_profiles(&self) -> u32 {
        (0..4).filter(|&p| {
            let k = self.find_player_slot(p as i8);
            0 <= k && self.r32(0x3110 + 0x44 * k as usize) != 0
        }).count() as u32
    }

    /// `Career_GetPlayerName` 0x164918: the name pointer's offset
    pub fn player_name(&self, p: i8) -> String {
        let k = Self::so(self.find_player_slot(p));
        let at = self.ri(0x3108 + k).wrapping_sub(CAREER_BASE) as usize;
        let end = self.bytes[at..].iter().position(|c| *c == 0).unwrap_or(0) + at;
        String::from_utf8_lossy(&self.bytes[at..end]).into_owned()
    }

    /// `Career_SetPlayerName` 0x164968(player, name): a name of only spaces (or empty) is not taken; else copied
    /// over the slot's name (marking the profile changed where a byte differs), and into the race rider of the
    /// player (+0x27e4) if there is one.
    pub fn set_player_name(&mut self, p: i8, name: &[u8]) {
        let k = Self::so(self.find_player_slot(p));
        let dst = self.ri(0x3108 + k).wrapping_sub(CAREER_BASE) as usize;
        let mut q = 0;
        loop {
            let c = *name.get(q).unwrap_or(&0);
            if c == 0 {
                return;
            }
            if c != b' ' {
                break;
            }
            q += 1;
        }
        let mut d = dst;
        for &c in name.iter().take_while(|c| **c != 0) {
            if self.bytes[d] as i8 as i32 != c as i8 as i32 {
                self.w32(0x3258, 1);
            }
            self.bytes[d] = c;
            d += 1;
        }
        self.bytes[d] = 0;
        let n = self.r32(0x27dc) as i32;
        for i in 0..n.max(0) as usize {
            if self.bytes[0x2845 + 0x84 * i] as i8 == p {
                let len = self.bytes[dst..].iter().position(|c| *c == 0).unwrap_or(0) + 1;
                let src = self.bytes[dst..dst + len].to_vec();
                self.bytes[0x27e4 + 0x84 * i..0x27e4 + 0x84 * i + len].copy_from_slice(&src);
                break;
            }
        }
    }

    /// The profile words of player `p`'s slot (+0x3110's pointer: +0 rider, +4, +8, +0xc outfit, +0x10 board):
    /// `FUN_00164f40` / `FUN_00164ec0` / `Career_GetPlayerOutfit` 0x164dc0 / `Career_GetPlayerBoard` 0x164e40
    pub fn profile_word(&self, p: i8, w: usize) -> u32 {
        let k = Self::so(self.find_player_slot(p));
        self.r32(self.ri(0x3110 + k).wrapping_sub(CAREER_BASE) as usize + 4 * w)
    }

    /// `FUN_00164f78` / `FUN_00164ef8` / `Career_SetPlayerOutfit` 0x164df8 / `Career_SetPlayerBoard` 0x164e78
    pub fn set_profile_word(&mut self, p: i8, w: usize, v: u32) {
        let k = Self::so(self.find_player_slot(p));
        let at = self.ri(0x3110 + k).wrapping_sub(CAREER_BASE) as usize + 4 * w;
        self.w32(at, v);
    }

    /// `Career_MergePlayerRecords` 0x1637f8: for each character the lowest-numbered player racing it (by the slot's
    /// profile rider) has its profile copied back to the character's short record when the two differ (by
    /// `Crc_Compute16`), marking the career changed.
    pub fn merge_player_records(&mut self) {
        let mut best = [4i32; 12];
        let mut src = [0usize; 12];
        for k in 0..4 {
            let c = self.bytes[0x30fc + 0x44 * k] as i8 as i32;
            if 0 <= c {
                let rider = self.r32(self.ptr(0x3110 + 0x44 * k)) as usize;
                if c < best[rider] {
                    best[rider] = c;
                    src[rider] = 0x3118 + 0x44 * k;
                }
            }
        }
        for r in 0..12 {
            if best[r] != 4 {
                let dst = 0x10 + 0x14 * r;
                if crc16(&self.bytes[dst..dst + 0x14]) != crc16(&self.bytes[src[r]..src[r] + 0x14]) {
                    let v = self.bytes[src[r]..src[r] + 0x14].to_vec();
                    self.bytes[dst..dst + 0x14].copy_from_slice(&v);
                    self.w32(0x3258, 1);
                }
            }
        }
    }

    /// `Career_GetRecords` 0x163468(table, course): the high-score table's offset (tables 0..2, else none)
    pub fn records_offset(table: i32, course: i32) -> Option<usize> {
        let b = 0x150 * course as isize;
        match table {
            0 => Some((0x16c4 + b) as usize),
            1 => Some((0x1734 + b) as usize),
            2 => Some((0x17a4 + b) as usize),
            _ => None,
        }
    }

    /// `Career_GetEventMode` 0x1635b0 (a half)
    pub fn event_mode(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0x2b0c], self.bytes[0x2b0d]])
    }
}

/// What `Career_CheckNewRecords` asks of the race (`Race_GetRider`(gApp+0x730, i)+0x418: below 1 a computer
/// rider) and of the circuit (`Circuit_ProcessRaceResult` 0x16bcb0 on the rider set-up, row C12).
pub trait RecordsEnv {
    fn rider_is_human(&self, i: usize) -> bool;
    fn circuit_race_result(&mut self, career: &mut Career);
}

impl Career {
    /// `HighScore_Insert` 0x166968(table, value, entry): the first place the value beats (lower for a time table,
    /// +0x68 = 0; higher otherwise) among the table's +0x64 entries: those below move down one (the last drops),
    /// the entry (name, character) goes in with the value. The entry's offset, or none.
    pub fn insert_high_score(&mut self, t: usize, value: i32, name: &[u8], character: u32) -> Option<usize> {
        let n = self.r32(t + 0x64) as i32;
        let higher = self.r32(t + 0x68) != 0;
        for i in 0..n.max(0) {
            let e = t + 0x14 * i as usize;
            let cur = self.r32(e + 0x10) as i32;
            let beats = if higher { cur < value } else { value < cur };
            if beats {
                let mut j = n - 2;
                while i <= j {
                    let s = t + 0x14 * j as usize;
                    let v = self.bytes[s..s + 0x14].to_vec();
                    self.bytes[s + 0x14..s + 0x28].copy_from_slice(&v);
                    j -= 1;
                }
                // the template: the name (the game's buffer holds whatever its stack held after the terminator)
                self.bytes[e..e + 0xc].fill(0);
                let len = name.len().min(0xb);
                self.bytes[e..e + len].copy_from_slice(&name[..len]);
                self.w32(e + 0xc, character);
                self.w32(e + 0x10, value as u32);
                return Some(e);
            }
        }
        None
    }

    /// `Career_CheckNewRecords` 0x164fc0: in show-off modes 3 and 5 a player's race rider that is not human loses
    /// the race's flags; the circuit's result; `Career_UpdateAttributes`; every slot's race flags cleared (marking
    /// the profile changed if any were set); the new-record list (+0x3910, five 0x10 entries: player, table, table,
    /// entry) emptied; then by the mode each player's race result into the course's tables: race modes 2 and 4 the
    /// time (+0x327c by finishing place's rider) into table 0, show-off 3 and 5 the score (+0x3310 + 0xbc · rider)
    /// into table 1, mode 7 the first's time into table 2; each new record listed.
    pub fn check_new_records(&mut self, env: &mut impl RecordsEnv, cheats: &crate::cheats::Cheats) {
        let mode = self.r32(0x2afc);
        if mode == 3 || mode == 5 {
            let n = self.r32(0x27dc) as i32;
            for i in 0..n.max(0) as usize {
                let p = self.bytes[0x2845 + 0x84 * i] as i8;
                if 0 <= p && !env.rider_is_human(i) {
                    self.clear_player_flags(p as i32);
                }
            }
        }
        env.circuit_race_result(self);
        self.update_attributes(cheats);
        let course = self.r32(0x2af8) as i32;
        for k in 0..4 {
            if self.r32(0x310c + 0x44 * k) != 0 {
                self.w32(0x3258, 1);
            }
            self.w32(0x310c + 0x44 * k, 0);
        }
        for i in (0..5).rev() {
            self.w32(0x3910 + 0x10 * i, 0xffff_ffff);
        }
        let mut listed = 0usize;
        let table = (0x16c4 + 0x150 * course as isize) as usize;
        let mut list = |c: &mut Career, p: i8, kind: u32, t: usize, e: usize| {
            let at = 0x3910 + 0x10 * listed;
            c.w32(at, p as i32 as u32);
            c.w32(at + 4, kind);
            c.w32(at + 8, CAREER_BASE + t as u32);
            c.w32(at + 0xc, CAREER_BASE + e as u32);
            listed += 1;
        };
        let entry = |c: &Career, rider: usize| -> (i8, Vec<u8>, u32) {
            let p = c.bytes[0x2845 + 0x84 * rider] as i8;
            let k = Self::so(c.find_player_slot(p));
            let name_at = c.ri(0x3108 + 0x44 * c.find_player_slot(p) as isize).wrapping_sub(CAREER_BASE) as usize;
            let end = c.bytes[name_at..].iter().position(|b| *b == 0).unwrap_or(0) + name_at;
            let name = c.bytes[name_at..end].to_vec();
            let ch = c.r32(c.ri(0x3110 + k).wrapping_sub(CAREER_BASE) as usize);
            (p, name, ch)
        };
        match mode {
            2 | 4 | 3 | 5 => {
                let n = self.r32(0x27dc) as i32;
                for place in 0..n.max(0) as usize {
                    let rider = self.r32(0x3264 + 4 * place) as usize;
                    if (self.bytes[0x2845 + 0x84 * rider] as i8) < 0 {
                        continue;
                    }
                    let (p, name, ch) = entry(self, rider);
                    let (t, value, kind) = if mode == 2 || mode == 4 { (table, self.r32(0x327c + 4 * rider) as i32, 0) } else { (table + 0x70, self.r32(0x3310 + 0xbc * rider) as i32, 1) };
                    if let Some(e) = self.insert_high_score(t, value, &name, ch) {
                        list(self, p, kind, t, e);
                    }
                }
            }
            7 => {
                let rider = self.r32(0x3264) as usize;
                let (p, name, ch) = entry(self, rider);
                let t = table + 0xe0;
                let value = self.r32(0x327c + 4 * rider) as i32;
                if let Some(e) = self.insert_high_score(t, value, &name, ch) {
                    list(self, p, 2, t, e);
                }
            }
            _ => {}
        }
        if 0 < listed {
            self.w32(0x3258, 1);
        }
    }

    /// `Career_UpdateBestStats` 0x166cb0(character, a race's stats (0xbc bytes), tag): each best kept in the
    /// character's record (+0x160.. pairs of value and tag) where the race beat it: four counts (stats +0x48,
    /// +0x1c, +0x10, +0x14), three floats (+0x5c, +0x60, +0x64) and six counts (+0x84..).
    pub fn update_best_stats(&mut self, character: usize, stats: &[u8], tag: u32) {
        let base = 0x1c4 * character;
        let si = |o: usize| i32::from_le_bytes(stats[o..o + 4].try_into().unwrap());
        let sf = |o: usize| f32::from_le_bytes(stats[o..o + 4].try_into().unwrap());
        for (best, from) in [(0x160, 0x48), (0x168, 0x1c), (0x170, 0x10), (0x178, 0x14)] {
            if (self.r32(base + best) as i32) < si(from) {
                self.w32(base + best, si(from) as u32);
                self.w32(base + best + 4, tag);
            }
        }
        for (best, from) in [(0x180, 0x5c), (0x188, 0x60), (0x190, 0x64)] {
            if self.rf(base + best) < sf(from) {
                self.wf(base + best, sf(from));
                self.w32(base + best + 4, tag);
            }
        }
        for i in 0..6 {
            let best = base + 0x1cc + 8 * i;
            if (self.r32(best) as i32) < si(0x84 + 4 * i) {
                self.w32(best, si(0x84 + 4 * i) as u32);
                self.w32(best + 4, tag);
            }
        }
    }

    /// `Career_RecordBestTime` 0x166fa0(character, course, time): kept if lower, or if none yet (−1)
    pub fn record_best_time(&mut self, character: usize, course: usize, time: i32) {
        let at = 0x1fc + 4 * course + 0x1c4 * character;
        let cur = self.r32(at) as i32;
        if time < cur || cur == -1 {
            self.w32(at, time as u32);
        }
    }

    /// `Career_RecordBestScore` 0x166fe0(character, course, score): kept if higher
    pub fn record_best_score(&mut self, character: usize, course: usize, score: i32) {
        let at = 0x230 + 4 * course + 0x1c4 * character;
        if (self.r32(at) as i32) < score {
            self.w32(at, score as u32);
        }
    }

    /// `Career_CountCourseStart` 0x167010(character, course)
    pub fn count_course_start(&mut self, character: usize, course: usize) {
        let at = 0x198 + 4 * course + 0x1c4 * character;
        self.w32(at, self.r32(at).wrapping_add(1));
    }

    /// `FUN_00167038`: the player wears outfit 6
    pub fn wears_outfit_6(&self, p: i8) -> bool {
        self.profile_word(p, 3) == 6
    }

    /// `Career_SelectSoloRider` 0x167060(player): every slot cleared, the player in slot 0 on character 0; if no
    /// race rider is the player, the race's player riders renumbered: the first the player, the rest 0 (1 when the
    /// player is 0).
    pub fn select_solo_rider(&mut self, p: i32, cheats: &crate::cheats::Cheats) {
        for k in 0..4 {
            self.clear_player_slot(k);
        }
        self.assign_player_rider(0, p, 0, cheats);
        let n = self.r32(0x27dc) as i32;
        let found = (0..n.max(0) as usize).any(|i| self.bytes[0x2845 + 0x84 * i] as i8 as i32 == p);
        if !found {
            let ids = [p as u8, (p == 0) as u8];
            let mut next = 0usize;
            for i in 0..n.max(0) as usize {
                if self.bytes[0x2845 + 0x84 * i] as i8 != -1 {
                    // (the game reads its stack past the two: only the first two are known)
                    self.bytes[0x2845 + 0x84 * i] = *ids.get(next).unwrap_or(&0);
                    next += 1;
                }
            }
        }
    }
}

/// The memory card's part of the block: +8 for 0x27d4 bytes (the last two words the CRC and the version)
pub const SAVE_START: usize = 8;
pub const SAVE_LEN: usize = 0x27d4;
/// The version word a save carries (+0x27d8; the load does not check it)
pub const SAVE_VERSION: u32 = 0x1230_0003;

impl Career {
    /// `Career_SaveToCard` 0x1d2078: the sound options synced, the CRC word cleared, the version set, the CRC of the
    /// saved part (`Crc_Compute16`) stored, and the part handed to the card (`MemCardOp_StartWrite`, here returned).
    pub fn save_image(&mut self, mut audio: Option<&mut dyn AudioSettings>) -> Vec<u8> {
        self.sync_audio(&mut audio);
        self.w32(0x27d4, 0);
        self.w32(0x27d8, SAVE_VERSION);
        let crc = crc16(&self.bytes[SAVE_START..SAVE_START + SAVE_LEN]);
        self.w32(0x27d4, crc);
        self.bytes[SAVE_START..SAVE_START + SAVE_LEN].to_vec()
    }

    /// The load (the card overlay's state at 0x184ae8): the CRC word taken out of the read bytes (zeroed in them)
    /// and checked against theirs; a match copies them over the saved part (so the CRC word reads 0 after) and
    /// applies the sound options. False (the card's error 0x12) on a mismatch or a short image.
    pub fn load_image(&mut self, image: &[u8], mut audio: Option<&mut dyn AudioSettings>) -> bool {
        if image.len() < SAVE_LEN {
            return false;
        }
        let mut buf = image[..SAVE_LEN].to_vec();
        let saved = u32::from_le_bytes(buf[0x27cc..0x27d0].try_into().unwrap());
        buf[0x27cc..0x27d0].fill(0);
        if crc16(&buf) != saved {
            return false;
        }
        self.bytes[SAVE_START..SAVE_START + SAVE_LEN].copy_from_slice(&buf);
        self.apply_audio(&mut audio);
        true
    }
}

/// What `Career_PrepareRace` 0x163980 asks of the rest of the game (row E1b2j).
pub trait PrepareEnv {
    /// `Career_SetupRidersForMode` 0x165558: the rider list for the race mode (the builders are not ported yet)
    fn setup_riders_for_mode(&mut self, career: &mut Career);
    /// `Rand_Fx` 0x23d4a8
    fn rand_fx(&mut self) -> u32;
}

impl Career {
    /// `Career_PrepareRace` 0x163980: mode 10 (the career's +0x2af8) is race mode 6; the rider list built for the
    /// mode; the six race slots numbered 0..count and the rest -1; `FUN_001637f8` (`merge_player_records`); every
    /// player slot's id word (+0x3104), its attribute words copied from +0x3118 to +0x312c; then two draws kept as
    /// the race's seeds (+0x2b04, then +0x2b00).
    pub fn prepare_race(&mut self, env: &mut impl PrepareEnv) {
        if self.r32(0x2af8) == 10 {
            self.w32(0x2afc, 6);
        }
        env.setup_riders_for_mode(self);
        let n = self.r32(0x27dc) as i32;
        let mut i = 0usize;
        let mut fill = true;
        if n >= 1 {
            while (i as i32) < n {
                self.w32(0x27e0 + 0x84 * i, i as u32);
                i += 1;
            }
            fill = n < 6;
        }
        if fill {
            while i < 6 {
                self.w32(0x27e0 + 0x84 * i, u32::MAX);
                i += 1;
            }
        }
        self.merge_player_records();
        for k in 0..4 {
            let o = 0x44 * k;
            self.w32(0x3104 + o, self.bytes[0x30fc + o] as i8 as i32 as u32);
            let a = self.r32(0x3118 + o);
            let b = self.r32(0x311c + o);
            self.w32(0x312c + o, a);
            self.w32(0x3130 + o, b);
            let a = self.r32(0x3120 + o);
            let b = self.r32(0x3124 + o);
            self.w32(0x3134 + o, a);
            self.w32(0x3138 + o, b);
            self.w32(0x313c + o, self.r32(0x3128 + o));
        }
        let d = env.rand_fx();
        self.w32(0x2b04, d);
        let d = env.rand_fx();
        self.w32(0x2b00, d);
    }
}

/// The riders' relation table as `Rivals_InitPairs` reads it (`RelTable_GetAttitude` 0x167da0, `FUN_00167de0` the
/// cap, `FUN_00167dc0` the tolerance, `RelTable_GetRelation` 0x167e00): rows are the rider who feels.
pub trait RelSource {
    fn attitude(&self, me: i32, other: i32) -> u32;
    fn cap(&self, me: i32, other: i32) -> u32;
    fn tolerance(&self, me: i32, other: i32) -> u32;
    fn relation(&self, me: i32, other: i32) -> u32;
}

/// The table as `RelTable_Init` leaves it.
pub struct InitialRelations;
impl RelSource for InitialRelations {
    fn attitude(&self, me: i32, other: i32) -> u32 {
        crate::rivals::ATTITUDE[me as usize][other as usize] as u32
    }
    fn cap(&self, me: i32, other: i32) -> u32 {
        crate::rivals::CAP[me as usize][other as usize] as u32
    }
    fn tolerance(&self, me: i32, other: i32) -> u32 {
        crate::rivals::TOLERANCE[me as usize][other as usize] as u32
    }
    fn relation(&self, me: i32, other: i32) -> u32 {
        crate::rivals::relation(me as usize, other as usize) as u32
    }
}

impl Career {
    /// `Rivals_InitPairs` 0x167ea0(table, count, riders = +0x27e0, slots = +0x30fc): for every pair of the first
    /// `count` race riders (rows of 0x84 bytes, the rider at +0x38, the player byte at +0x65), the row's four bytes
    /// for the other at +0x20 + 4·other: the attitude (a human other: the player's own record, +0x168 + 8·rider of
    /// its profile), the cap, the tolerance and the relation.
    pub fn rivals_init_pairs(&mut self, count: i32, rel: &dyn RelSource) {
        for i in 0..count.max(0) as usize {
            for j in 0..count as usize {
                let (ri, rj) = (0x27e0 + 0x84 * i, 0x27e0 + 0x84 * j);
                let me = self.r32(ri + 0x38) as i32;
                let other = self.r32(rj + 0x38) as i32;
                let player = self.bytes[rj + 0x65] as i8;
                let att = if player < 0 {
                    rel.attitude(me, other) as u8
                } else {
                    let (rec, _) = self.player_rec(player);
                    self.bytes[(rec as isize + 8 * me as isize + 0x168) as usize]
                };
                let at = ri + 4 * j;
                self.bytes[at + 0x20] = att;
                self.bytes[at + 0x21] = rel.cap(me, other) as u8;
                self.bytes[at + 0x22] = rel.tolerance(me, other) as u8;
                self.bytes[at + 0x23] = rel.relation(me, other) as u8;
            }
        }
    }
}

/// The career as the app keeps it on disk (row E12d): the memory card's image (`Career::save_image`, CRC included).
pub struct CareerFile;

impl CareerFile {
    /// What a start does: the file's image loaded (`Career::load_image`: its CRC must match), else a new game
    /// (`Career::init_new_game`, `seed` for the game's random streams as the original seeds them at start). The flag
    /// says the file was used.
    pub fn open(file: Option<&[u8]>, language: u32, seed: u32, audio: Option<&mut dyn AudioSettings>) -> (Career, bool) {
        let mut c = Career::default();
        let mut audio = audio;
        if let Some(bytes) = file {
            if c.load_image(bytes, audio.as_deref_mut().map(|a| a as &mut dyn AudioSettings)) {
                return (c, true);
            }
            c = Career::default();
        }
        let mut rand = Streams::default();
        rand.seed_fx(seed);
        rand.seed_game(seed);
        c.init_new_game(language, &mut rand, audio);
        (c, false)
    }
}

/// The outfit table (0x3cd0e0, written by `Career_InitStaticTables`): by character and outfit four words: the
/// outfit's model, then the character three times (the readers below). Outfit 6 repeats outfit 5.
pub const OUTFITS: [[[u32; 4]; 7]; 12] = [
    [[0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0], [4, 0, 0, 0], [5, 0, 0, 0], [5, 0, 0, 0]],
    [[7, 1, 1, 1], [8, 1, 1, 1], [9, 1, 1, 1], [10, 1, 1, 1], [11, 1, 1, 1], [12, 1, 1, 1], [12, 1, 1, 1]],
    [[14, 2, 2, 2], [15, 2, 2, 2], [16, 2, 2, 2], [17, 2, 2, 2], [18, 2, 2, 2], [19, 2, 2, 2], [19, 2, 2, 2]],
    [[21, 3, 3, 3], [22, 3, 3, 3], [23, 3, 3, 3], [24, 3, 3, 3], [25, 3, 3, 3], [26, 3, 3, 3], [26, 3, 3, 3]],
    [[28, 4, 4, 4], [29, 4, 4, 4], [30, 4, 4, 4], [31, 4, 4, 4], [32, 4, 4, 4], [33, 4, 4, 4], [33, 4, 4, 4]],
    [[35, 5, 5, 5], [36, 5, 5, 5], [37, 5, 5, 5], [38, 5, 5, 5], [39, 5, 5, 5], [40, 5, 5, 5], [40, 5, 5, 5]],
    [[42, 6, 6, 6], [43, 6, 6, 6], [44, 6, 6, 6], [45, 6, 6, 6], [46, 6, 6, 6], [47, 6, 6, 6], [47, 6, 6, 6]],
    [[49, 7, 7, 7], [50, 7, 7, 7], [51, 7, 7, 7], [52, 7, 7, 7], [53, 7, 7, 7], [54, 7, 7, 7], [54, 7, 7, 7]],
    [[56, 8, 8, 8], [57, 8, 8, 8], [58, 8, 8, 8], [59, 8, 8, 8], [60, 8, 8, 8], [61, 8, 8, 8], [61, 8, 8, 8]],
    [[63, 9, 9, 9], [64, 9, 9, 9], [65, 9, 9, 9], [66, 9, 9, 9], [67, 9, 9, 9], [68, 9, 9, 9], [68, 9, 9, 9]],
    [[70, 10, 10, 10], [71, 10, 10, 10], [72, 10, 10, 10], [73, 10, 10, 10], [74, 10, 10, 10], [75, 10, 10, 10], [75, 10, 10, 10]],
    [[77, 11, 11, 11], [78, 11, 11, 11], [79, 11, 11, 11], [80, 11, 11, 11], [81, 11, 11, 11], [82, 11, 11, 11], [82, 11, 11, 11]],
];

/// The career's static tables: `DAT_00335258` (set until the first init), the outfit table and the board table
/// (0x3cd620, 12 × 12 words, character · 12 + board: their own index).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticTables {
    pub pending: u32,
    pub outfits: [[[u32; 4]; 7]; 12],
    pub boards: [[u32; 12]; 12],
}

impl Default for StaticTables {
    /// As the game loads: the tables zero, waiting for their init
    fn default() -> Self {
        StaticTables { pending: 1, outfits: [[[0; 4]; 7]; 12], boards: [[0; 12]; 12] }
    }
}

impl StaticTables {
    /// `Career_InitStaticTables` 0x169e38: once (while `pending`): the tables written and the character cheat's
    /// word cleared. (`Career_Construct` 0x161730 runs it, then again through `Circuit_Construct` 0x16bb70.)
    pub fn init(&mut self, cheats: &mut crate::cheats::Cheats) {
        if self.pending != 0 {
            self.pending = 0;
            self.outfits = OUTFITS;
            for (c, row) in self.boards.iter_mut().enumerate() {
                for (b, w) in row.iter_mut().enumerate() {
                    *w = (12 * c + b) as u32;
                }
            }
            cheats.character = [0];
        }
    }

    /// `Career_GetBoardWord` 0x16b1d0(character, board)
    /// (the game indexes one flat table: character · 12 + board)
    pub fn board_word(&self, character: usize, board: usize) -> u32 {
        let i = 12 * character + board;
        self.boards[i / 12][i % 12]
    }

    /// `Career_GetOutfitWord0` 0x16b1f8 / `Career_GetOutfitWord1` 0x16b248 / `Career_GetRiderAttrib` 0x16b220 /
    /// `Career_GetOutfitWord3` 0x16b270(character, outfit): word 0..3
    /// (the game indexes one flat table: character · 7 + outfit)
    pub fn outfit_word(&self, character: usize, outfit: usize, word: usize) -> u32 {
        let i = 7 * character + outfit;
        self.outfits[i / 7][i % 7][word]
    }
}

/// `Career_GetStatusName` 0x1634c0: the string id (`Loc_GetString` on gApp+0x50) of status 0..9, else none
pub fn status_name_id(status: i32) -> Option<u32> {
    (0..=9).contains(&status).then(|| 0x17 + status as u32)
}

/// What `Score_TrickBookCheck` 0x1579e0 reads and calls outside the career block (row C15).
pub trait BookEnv {
    /// The race object's lesson-mode gate: `(gApp+0x730)->+0x40` is null, or its `+0x20` is 0 or at least 10
    fn mode_allows(&self) -> bool;
    /// The scoring rider's player byte (`rider->+0x464->+0x65`); negative for none
    fn player(&self) -> i8;
    /// The tutorial lesson's trick to land (`(gApp+0x730)->+0x34`'s `+0x4e8`), if a lesson is running
    fn lesson_trick(&self) -> Option<u32>;
    /// `Trick_GetInfo(rider, t)` 0x154638's first two words (the trick's id and the flags the match compares)
    fn trick_info(&self, rider: u32, t: u32) -> [u32; 2];
    /// `FUN_0014c2a8`: the lesson's `+0x64` = 1 (the trick was landed)
    fn lesson_done(&mut self);
    /// `Scoring_PushMessage(1.2, score, 0x11, 0, 0)` 0x157330 then `Sfx_OnTrickBookDone`
    fn book_done_message(&mut self);
}

/// `FUN_00157c18`: a landed trick's id words against a book entry's (the flags that count are 0x07000f00)
pub fn trick_matches(id: [u32; 2], info: [u32; 2]) -> bool {
    id[0] == info[0] && ((id[1] ^ info[1]) & 0x0700_0f00) == 0
}

/// `FUN_001640a8` 0x1640a8: the page a book trick is on (five a page)
pub fn trick_page(t: u32) -> u32 {
    let t = t as i32;
    if t < 5 {
        0
    } else if t < 10 {
        1
    } else if t < 15 {
        2
    } else if t < 20 {
        3
    } else if t < 25 {
        4
    } else {
        5
    }
}

impl Career {
    /// `Score_TrickBookCheck` 0x1579e0 (row C15): a trick was landed (`id`). With a lesson running it only tells the
    /// lesson; else every book trick it matches that is neither done nor forced, on the first unfinished page,
    /// is forced (+8) and flagged for this race (+0x310c); a ticked trick gets the message and the sound.
    pub fn trick_book_check(&mut self, env: &mut dyn BookEnv, id: [u32; 2]) {
        if !env.mode_allows() {
            return;
        }
        let p = env.player();
        if p < 0 {
            return;
        }
        let (rec, k) = self.player_rec(p);
        let book = rec + 0x14;
        let rider = self.r32(book);
        if let Some(lt) = env.lesson_trick() {
            if trick_matches(id, env.trick_info(rider, lt)) {
                env.lesson_done();
            }
            return;
        }
        let mut any = false;
        for t in 0..0x1e {
            let bit = 1u32 << t;
            let rider = self.r32(book);
            if trick_matches(id, env.trick_info(rider, t)) && self.r32(book + 8) & bit == 0 && self.r32(book + 4) & bit == 0 && trick_page(t) == self.complete_pages(p) {
                any = true;
                let f = (0x310c + k) as usize;
                self.w32(f, self.r32(f) | bit);
                self.w32(book + 8, self.r32(book + 8) | bit);
            }
        }
        if any {
            env.book_done_message();
        }
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn the_file_round_trips_and_a_bad_one_starts_a_new_game() {
        let (mut a, used) = CareerFile::open(None, 0, 7, None);
        assert!(!used);
        a.w32(0x1670, 3);
        let img = a.save_image(None);
        assert_eq!(img.len(), SAVE_LEN);
        let (b, used) = CareerFile::open(Some(&img), 0, 99, None);
        assert!(used);
        assert_eq!(b.r32(0x1670), 3);
        assert_eq!(&b.bytes[SAVE_START..SAVE_START + SAVE_LEN - 8], &a.bytes[SAVE_START..SAVE_START + SAVE_LEN - 8]);
        // a flipped byte fails the CRC; a short file too
        let mut bad = img.clone();
        bad[0x100] ^= 1;
        assert!(!CareerFile::open(Some(&bad), 0, 7, None).1);
        assert!(!CareerFile::open(Some(&img[..100]), 0, 7, None).1);
    }
}

impl Career {
    /// The player's profile (+0x3110 of its slot: character, entry +0x28, entry +0x30, outfit, board)
    fn profile_at(&self, p: i8) -> usize {
        self.ptr((0x3110 + 0x44 * self.find_player_slot(p) as isize) as usize)
    }

    /// `Career_GetPlayerBoardWord` 0x164ac8: the board table's word of the player's character and board
    pub fn player_board_word(&self, t: &StaticTables, p: i8) -> u32 {
        let at = self.profile_at(p);
        t.board_word(self.r32(at) as usize, self.r32(at + 0x10) as usize)
    }

    /// `Career_GetPlayerBoardWordFor` 0x164b10(player, board)
    pub fn player_board_word_for(&self, t: &StaticTables, p: i8, board: usize) -> u32 {
        t.board_word(self.r32(self.profile_at(p)) as usize, board)
    }

    /// `Career_GetPlayerOutfitWord0` 0x164b60 / `Career_GetPlayerRiderAttrib` 0x164bf8 / `Career_GetPlayerOutfitWord1`
    /// 0x164c90 / `Career_GetPlayerOutfitWord3` 0x164d28 (`word` 0, 2, 1, 3): the outfit table's word of the player's
    /// character and outfit
    pub fn player_outfit_word(&self, t: &StaticTables, p: i8, word: usize) -> u32 {
        let at = self.profile_at(p);
        t.outfit_word(self.r32(at) as usize, self.r32(at + 0xc) as usize, word)
    }

    /// The `…For` forms (0x164ba8, 0x164c40, 0x164cd8, 0x164d70)(player, outfit)
    pub fn player_outfit_word_for(&self, t: &StaticTables, p: i8, outfit: usize, word: usize) -> u32 {
        t.outfit_word(self.r32(self.profile_at(p)) as usize, outfit, word)
    }

    /// `Career_GetPlayerRecordWord1` 0x164f40 / `Career_GetPlayerRecordWord2` 0x164ec0: the profile's word 1 or 2
    pub fn player_record_word(&self, p: i8, word: usize) -> u32 {
        self.r32(self.profile_at(p) + 4 * word)
    }

    /// `Career_SetPlayerRecordWord1` 0x164f78 / `Career_SetPlayerRecordWord2` 0x164ef8
    pub fn set_player_record_word(&mut self, p: i8, word: usize, v: u32) {
        let at = self.profile_at(p) + 4 * word;
        self.w32(at, v);
    }

    /// `Career_FinishRace` 0x163ab0: a circuit race or show-off advances its event (`Circuit_AdvanceEvent`, the
    /// next screen at +0x3260); otherwise the next screen is 2, the tutorial (6) going back to the circuit race (4);
    /// then `Career_UpdateAttributes`.
    pub fn finish_race(&mut self, circuit: &mut crate::circuit::Circuit, cheats: &crate::cheats::Cheats, setup_event_riders: &mut dyn FnMut(&mut Career, &mut crate::circuit::Circuit)) {
        let mode = self.r32(0x2afc);
        if mode.wrapping_sub(4) < 2 {
            let next = circuit.advance_event(self, setup_event_riders);
            self.w32(0x3260, next);
        } else {
            if mode == 6 {
                self.w32(0x2afc, 4);
            }
            self.w32(0x3260, 2);
        }
        self.update_attributes(cheats);
    }

    /// `Career_SetPendingVideo` 0x163f10: the video's name into +0x3714 (`strcpy`)
    pub fn set_pending_video(&mut self, name: &[u8]) {
        let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
        self.bytes[0x3714..0x3714 + end].copy_from_slice(&name[..end]);
        self.bytes[0x3714 + end] = 0;
    }
}

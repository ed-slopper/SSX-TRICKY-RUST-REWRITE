//! The roster builders' helpers (row E1b2k): `Sort_ShellPairs` 0x2cfd90 and what the race rider lists are built
//! from. Checked in the function runner (`tests/roster.rs`).

/// The shell sort's gaps (`DAT_003506c8`: 3k + 1).
pub const GAPS: [i32; 12] = [1, 4, 13, 40, 121, 364, 1093, 3280, 9841, 29524, 88573, 265720];

/// `Sort_ShellPairs` 0x2cfd90(count, keys, values): the keys sorted largest first (signed), the values moving with
/// them; a shell sort over the gap table, so equal keys keep an order only the algorithm decides.
pub fn sort_shell_pairs(keys: &mut [i32], vals: &mut [i32]) {
    let n = keys.len() as i32;
    let mut gi = 1usize;
    if GAPS[1] < n {
        gi = 2;
        while GAPS[gi] < n {
            gi += 1;
        }
    }
    loop {
        gi -= 1;
        let h = GAPS[gi];
        let mut i = h;
        while i < n {
            let key = keys[i as usize];
            let val = vals[i as usize];
            let mut j = i - h;
            loop {
                let cur = keys[j as usize];
                if key <= cur {
                    break;
                }
                keys[(j + h) as usize] = cur;
                vals[(j + h) as usize] = vals[j as usize];
                j -= h;
                if j < 0 {
                    break;
                }
            }
            keys[(j + h) as usize] = key;
            vals[(j + h) as usize] = val;
            i += 1;
        }
        if h < 2 {
            return;
        }
    }
}

/// `PreRaceSel` (0x32ecf0 in the career's rider set-up): the pre-race selection. Five co-rider words (+0), then 15
/// each of the look ids (+0x14, 0x1ad empty), their first words (+0x50, 0x39) and picks (+0x8c), and the chosen
/// rider (+0xc8). `PreRaceSel_Clear` 0x1686a8 resets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreRaceSel {
    pub co: [i32; 5],
    pub ids: [i32; 15],
    pub firsts: [i32; 15],
    pub picks: [i32; 15],
    pub rider: i32,
}

impl Default for PreRaceSel {
    fn default() -> Self {
        PreRaceSel { co: [0xc; 5], ids: [0x1ad; 15], firsts: [0x39; 15], picks: [0; 15], rider: 0xc }
    }
}

/// `PreRaceSel_MatchRow` 0x1687f0(sel, a, b, c, d, row): how well a 12-byte table row's four selector bytes fit
/// the wanted values: each byte equal to its wanted value (or the second wildcard) adds one, the first wildcard
/// (12, 13, 10, 3) matches without adding, anything else is -1.
pub fn match_row(a: u32, b: u32, c: u32, d: u32, row: &[u8]) -> i32 {
    let mut score = 0;
    for (k, (want, any, also)) in [(a, 0xc, 0xd), (b, 0xd, 0xe), (c, 10, 0xb)].into_iter().enumerate() {
        let u = row[k] as u32;
        if (u == any || u != want) && u != also {
            if u != any {
                return -1;
            }
        } else {
            score += 1;
        }
    }
    let u = row[3] as u32;
    if u == 3 || u != d {
        if u == 4 {
            score += 1;
        } else if u != 3 {
            return -1;
        }
    } else {
        score += 1;
    }
    score
}

impl PreRaceSel {
    /// `PreRaceSel_SetCoRiders` 0x1689e0: the row's five bytes at +4 (signed) into the co-rider words, but 12
    pub fn set_co_riders(&mut self, row: &[u8]) {
        for k in 0..5 {
            if row[4 + k] != 0xc {
                self.co[k] = row[4 + k] as i8 as i32;
            }
        }
    }

    /// `PreRaceSel_ChooseFromTable` 0x168a18(sel, slot, count, table, a, b, c, d): of the table's `count` 12-byte
    /// rows the best-matching ones (ties kept, at most 20), one picked by `Rand_Fx` (unsigned remainder), its
    /// co-riders set and its look id (bytes 9, 10) and first word (byte 11) put in `slot`. No match: no draw.
    pub fn choose_from_table(&mut self, slot: usize, count: usize, table: &[u8], want: [u32; 4], rand_fx: &mut dyn FnMut() -> u32) {
        let mut best = 0i32;
        let mut found: Vec<usize> = Vec::new();
        for i in 0..count {
            let m = match_row(want[0], want[1], want[2], want[3], &table[12 * i..12 * i + 12]);
            if m >= 0 && best <= m {
                if best < m {
                    found.clear();
                    best = m;
                }
                found.push(i);
            }
        }
        if !found.is_empty() {
            let d = rand_fx();
            let row = found[(d % found.len() as u32) as usize];
            let row = &table[12 * row..12 * row + 12];
            self.set_co_riders(row);
            self.ids[slot] = (u32::from_le_bytes([row[8], row[9], row[10], row[11]]) >> 8 & 0xffff) as i32;
            self.firsts[slot] = row[11] as i32;
        }
    }

    /// `PreRaceSel_Choose` 0x168720(sel, rider, b, c, d): cleared, the rider noted, then slots 0, 1 and 2 from the
    /// three tables (9, 0x85 and 0x9f rows at 0x37b390, 0x37b400, 0x37ba40) for (rider, b, c, d).
    pub fn choose(&mut self, rider: u32, b: u32, c: u32, d: u32, tables: [&[u8]; 3], rand_fx: &mut dyn FnMut() -> u32) {
        *self = PreRaceSel::default();
        self.rider = rider as i32;
        for (slot, count) in [(0usize, 9usize), (1, 0x85), (2, 0x9f)] {
            self.choose_from_table(slot, count, tables[slot], [rider, b, c, d], rand_fx);
        }
    }
}

/// `PreRaceSel_MatchRowWithRoster` 0x1688f0: [`match_row`], and then the row's five co-rider bytes (+4, signed;
/// 12 is none) against the roster's `list` of `count` rider ids: with `count` 0 every byte must be 12; else each
/// byte that is not 12 must be in the list (and adds one) or the row is -1. `count` below 0 reads on past the
/// list in the game: not modelled (the list is cut to its length).
pub fn match_row_with_roster(want: [u32; 4], list: &[i32], count: i32, row: &[u8]) -> i32 {
    let mut score = match_row(want[0], want[1], want[2], want[3], row);
    if score < 0 {
        return score;
    }
    if count == 0 {
        return if (0..5).all(|k| row[4 + k] as i8 == 0xc) { score } else { -1 };
    }
    for k in 0..5 {
        let v = row[4 + k] as i8 as i32;
        if v == 0xc {
            continue;
        }
        if !list.iter().take(count.max(0) as usize).any(|&x| x == v) {
            return -1;
        }
        score += 1;
    }
    score
}

impl PreRaceSel {
    /// `PreRaceSel_ResolveWithRoster` 0x1691f0(sel, slot, count, table, a, b, c, d, list, listcount): the slot
    /// emptied, then [`PreRaceSel::choose_from_table`]'s pick over rows matched with [`match_row_with_roster`];
    /// the co-riders are not touched.
    pub fn resolve_with_roster(&mut self, slot: usize, count: usize, table: &[u8], want: [u32; 4], list: &[i32], listcount: i32, rand_fx: &mut dyn FnMut() -> u32) {
        self.ids[slot] = 0x1ad;
        self.firsts[slot] = 0x39;
        let mut best = 0i32;
        let mut found: Vec<usize> = Vec::new();
        for i in 0..count {
            let m = match_row_with_roster(want, list, listcount, &table[12 * i..12 * i + 12]);
            if m >= 0 && best <= m {
                if best < m {
                    found.clear();
                    best = m;
                }
                found.push(i);
            }
        }
        if !found.is_empty() {
            let d = rand_fx();
            let row = found[(d % found.len() as u32) as usize];
            let row = &table[12 * row..12 * row + 12];
            self.ids[slot] = (u32::from_le_bytes([row[8], row[9], row[10], row[11]]) >> 8 & 0xffff) as i32;
            self.firsts[slot] = row[11] as i32;
        }
    }
}

/// The 12-byte-row tables `Career_FinalizeRoster` picks from: `rows[0..3]` the three at 0x37b390, 0x37b400 and
/// 0x37ba40 (9, 0x85, 0x9f rows), `rows[3]` 0x37c1b8 (0x3f), `rows[4]` 0x37c4b0 (0x34), `rows[5]` 0x37cd60 (1) and
/// `rows[6]` 0x37c720 (0x85); and the look tables `rider_scene` reads.
pub struct RosterTables<'a> {
    pub rows: [&'a [u8]; 7],
    pub scene: crate::fxrand::SceneTables<'a>,
}

impl crate::career::Career {
    /// `Career_FinalizeRoster` 0x168b88(sel, roster = +0x27dc): the selection made to fit the race's riders. The
    /// roster's `n` riders (rider +0x3c + 0x84 i, status +0x84 + 0x84 i, player byte +0x69 + 0x84 i): whether the
    /// selection's co-riders are all racing; the riders but the selected one listed; the first whose status is not
    /// 2, the selected rider's place, how many are the selected rider, how many are human. With no free rider and
    /// the selected rider no more than once the slots are re-picked against the roster (slots 3 and 4 with fewer
    /// than two humans, slots 5 to 14 by the rider count); else the slots are emptied and slots 0 to 2 picked with no
    /// rider asked for. +0x30f4 notes the chosen place, +0x30f8 is 0, and `rider_scene` fills the looks.
    /// (A roster of no riders is not modelled.)
    pub fn finalize_roster(&mut self, sel: &mut PreRaceSel, t: &RosterTables, rand_fx: &mut dyn FnMut() -> u32) {
        let b = 0x27dcusize;
        let n = self.r32(b) as i32;
        let u = self.r32(b + 0x330) & 0xffff;
        let (p6, p7) = (self.r32(b + 0x31c), self.r32(b + 0x320));
        let rider = |c: &Self, i: usize| c.r32(b + 0x3c + 0x84 * i) as i32;
        // the selection's co-riders all racing
        let mut all = true;
        for i in 0..5 {
            let co = sel.co[i];
            if co != 0xc {
                all = (0..n.max(0) as usize).any(|j| rider(self, j) == co);
            }
            if !all {
                break;
            }
        }
        let mut list = [0xci32; 5];
        let (mut first_free, mut at, mut same, mut humans, mut listed) = (-1i32, -1i32, 0, 0, 0usize);
        for i in 0..n.max(0) as usize {
            if self.r32(b + 0x84 + 0x84 * i) != 2 && first_free < 0 {
                first_free = i as i32;
            }
            let rid = rider(self, i);
            if rid != sel.rider || at >= 0 {
                if listed < 5 {
                    list[listed] = rid;
                }
                listed += 1;
            } else {
                at = i as i32;
            }
            if rid == sel.rider {
                same += 1;
            }
            if (self.bytes[b + 0x69 + 0x84 * i] as i8) >= 0 {
                humans += 1;
            }
        }
        if first_free >= 0 {
            at = first_free;
        }
        let want = |rider: u32| [rider, p6, p7, u];
        if first_free >= 0 || same >= 2 {
            sel.ids = [0x1ad; 15];
            sel.firsts = [0x39; 15];
            for (slot, count, rows) in [(0usize, 9usize, 0usize), (1, 0x85, 1), (2, 0x9f, 2)] {
                sel.resolve_with_roster(slot, count, t.rows[rows], want(0xc), &list, n - 1, rand_fx);
            }
        } else {
            if !all {
                for (slot, count, rows) in [(0usize, 9usize, 0usize), (1, 0x85, 1), (2, 0x9f, 2)] {
                    sel.resolve_with_roster(slot, count, t.rows[rows], want(sel.rider as u32), &list, n - 1, rand_fx);
                }
            }
            if humans < 2 {
                sel.resolve_with_roster(3, 0x3f, t.rows[3], want(sel.rider as u32), &list, n - 1, rand_fx);
                sel.resolve_with_roster(4, 0x34, t.rows[4], want(sel.rider as u32), &list, n - 1, rand_fx);
            }
            for (min, k, slot_a, slot_b) in [(6, 4usize, 9usize, 0xeusize), (5, 3, 8, 0xd), (4, 2, 7, 0xc), (3, 1, 6, 0xb), (2, 0, 5, 0xa)] {
                if n >= min {
                    sel.resolve_with_roster(slot_a, 1, t.rows[5], want(sel.rider as u32), &list[k..], 1, rand_fx);
                    sel.resolve_with_roster(slot_b, 0x85, t.rows[6], want(sel.rider as u32), &list[k..], 1, rand_fx);
                }
            }
        }
        self.w32(b + 0x918, at as u32);
        self.w32(b + 0x91c, 0);
        // the looks
        let mut ids = sel.ids;
        let _ = &mut ids;
        let mut scenes = [crate::fxrand::RiderScene::default(); 15];
        for i in 0..15 {
            let o = b + 0x33c + 100 * i;
            scenes[i].first = self.r32(o) as i32;
            for k in 0..6 {
                let s = o + 4 + 16 * k;
                scenes[i].slots[k] = crate::fxrand::Slot { a: self.r32(s) as i32, b: self.r32(s + 4) as i32, c: self.r32(s + 8) as i32, d: self.r32(s + 12) as i32 };
            }
        }
        crate::fxrand::rider_scene(&sel.ids, &sel.firsts, -1, rand_fx, &t.scene, &mut scenes, &mut sel.picks);
        for i in 0..15 {
            let o = b + 0x33c + 100 * i;
            self.w32(o, scenes[i].first as u32);
            for k in 0..6 {
                let s = o + 4 + 16 * k;
                let sl = scenes[i].slots[k];
                self.w32(s, sl.a as u32);
                self.w32(s + 4, sl.b as u32);
                self.w32(s + 8, sl.c as u32);
                self.w32(s + 12, sl.d as u32);
            }
        }
    }
}

impl crate::career::Career {
    /// `Career_SetupAIRider` 0x16ae68(rider entry, rival level, row): the race rider row (0x84 bytes at
    /// +0x27e0 + 0x84 row) made for a computer rider from its `gRider_Table` entry (`entry`, 0x38 bytes at
    /// 0x331ea8 + 0x38 rider; `name` the string its +4 points at, NUL included). Two draws, in this order: the
    /// outfit (`Rand_Fx % 5`) and the board (`% 12`, unsigned). `board_points` is the table at 0x332150 (rows of
    /// 0xf0, entries of 0x14). The stat bytes are [`crate::stats::ai_bytes`] from the entry's bytes +0xc (base)
    /// and +0x1c (caps).
    pub fn setup_ai_rider(&mut self, row: usize, entry: &[u8], name: &[u8], board_points: &[u8], rival: Option<i32>, tables: &crate::career::StaticTables, max_attrib: bool, rand_fx: &mut dyn FnMut() -> u32) {
        let o = 0x27e0 + 0x84 * row;
        let w = |e: &[u8], at: usize| u32::from_le_bytes([e[at], e[at + 1], e[at + 2], e[at + 3]]);
        self.bytes[o + 4..o + 4 + name.len()].copy_from_slice(name);
        let outfit = (rand_fx() % 5) as usize;
        let board = (rand_fx() % 12) as usize;
        let rider = w(entry, 0);
        self.w32(o + 0x38, rider);
        self.w32(o + 0x3c, rider);
        self.bytes[o + 0x1e] = entry[0x20];
        self.w32(o + 0x5c, w(entry, 0x2c));
        self.w32(o + 0x40, w(entry, 0x28));
        self.w32(o + 0x60, w(entry, 0x24));
        let bp = rider as usize * 0xf0 + board * 0x14;
        self.w32(o + 0x44, w(board_points, bp));
        self.bytes[o + 0x67] = 0xff;
        self.bytes[o + 0x64] = entry[0x34];
        self.bytes[o + 0x68] = 0xff;
        self.bytes[o + 0x65] = 0xff;
        self.bytes[o + 0x66] = 0xff;
        for k in [0x70, 0x74, 0x78, 0x7c] {
            self.w32(o + k, 0);
        }
        self.w32(o + 0x80, 2);
        self.w32(o + 0x6c, u32::MAX);
        let r = rider as usize;
        self.w32(o + 0x4c, tables.outfit_word(r, outfit, 0));
        self.w32(o + 0x50, tables.outfit_word(r, outfit, 2));
        self.w32(o + 0x54, tables.outfit_word(r, outfit, 1));
        self.w32(o + 0x58, tables.outfit_word(r, outfit, 3));
        self.w32(o + 0x48, tables.board_word(r, board));
        let base: [u8; 16] = entry[0xc..0x1c].try_into().unwrap();
        let caps: [u8; 4] = entry[0x1c..0x20].try_into().unwrap();
        let bytes = crate::stats::ai_bytes(&base, &caps, rival, max_attrib);
        self.bytes[o + 0xe..o + 0x1e].copy_from_slice(&bytes);
    }
}

/// What `Career_SetupPlayerRider` 0x16a9d8 reads outside the career block and the tables.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerRiderEnv {
    /// `DAT_003352a4`, `DAT_003352a8`: the two words kept in the row's +0x78 / +0x7c outside mode 6
    pub words: [u32; 2],
    /// `DAT_003352b8` (the character cheat), `gCheat_Mallora`, `gCheat_MaxAttrib`
    pub character_cheat: u32,
    pub mallora: bool,
    pub max_attrib: bool,
}

impl crate::career::Career {
    /// `Career_SetupPlayerRider` 0x16a9d8(slot = +0x30fc + 0x44 k, row): the race rider row made for the human
    /// player of slot `k` from the slot's profile (rider, outfit, board), the rider's `gRider_Table` entry
    /// (`entry`, `name` the player's name string the slot points at, NUL included) and the board's table row
    /// (`board_points` as in [`Career::setup_ai_rider`]; its bytes +8..12 are the board's stat bonuses). No draws.
    pub fn setup_player_rider(&mut self, k: usize, row: usize, entry: &[u8], name: &[u8], board_points: &[u8], tables: &crate::career::StaticTables, env: &PlayerRiderEnv) {
        let o = 0x27e0 + 0x84 * row;
        let mode = self.r32(0x2afc); // DAT_0032de8c, the career's race mode (6: the lesson)
        let sb = 0x30fc + 0x44 * k;
        let w = |e: &[u8], at: usize| u32::from_le_bytes([e[at], e[at + 1], e[at + 2], e[at + 3]]);
        let prof = self.ptr(sb + 0x14);
        let rec = self.ptr(sb + 0x18);
        let rider = self.r32(prof);
        let board = self.r32(prof + 0x10) as usize;
        let outfit = self.r32(prof + 0xc) as usize;
        let player = self.bytes[sb] as i8;
        self.bytes[o + 4..o + 4 + name.len()].copy_from_slice(name);
        self.w32(o + 0x38, rider);
        self.w32(o + 0x3c, rider);
        self.bytes[o + 0x1e] = entry[0x20];
        self.w32(o + 0x5c, w(entry, 0x2c));
        self.w32(o + 0x60, w(entry, 0x24));
        self.bytes[o + 0x64] = entry[0x34];
        self.w32(o + 0x40, self.r32(prof + 4));
        self.bytes[o + 0x65] = player as u8;
        self.w32(o + 0x6c, self.r32(sb + 4));
        self.bytes[o + 0x68] = self.find_player_slot(player) as u8;
        let bp = rider as usize * 0xf0 + board * 0x14;
        self.w32(o + 0x44, w(board_points, bp));
        self.bytes[o + 0x67] = 0xff;
        self.bytes[o + 0x66] = 0xff;
        if mode == 6 {
            self.w32(o + 0x78, 0);
            self.w32(o + 0x7c, 0);
        } else {
            self.w32(o + 0x78, env.words[0]);
            self.w32(o + 0x7c, env.words[1]);
        }
        self.w32(o + 0x70, 0);
        self.w32(o + 0x80, 2);
        let level = self.player_level(player);
        self.w32(o + 0x74, level);
        let r = rider as usize;
        self.w32(o + 0x4c, tables.outfit_word(r, outfit, 0));
        self.w32(o + 0x50, tables.outfit_word(r, outfit, 2));
        self.w32(o + 0x54, tables.outfit_word(r, outfit, 1));
        self.w32(o + 0x58, tables.outfit_word(r, outfit, 3));
        self.w32(o + 0x48, tables.board_word(r, board));
        if env.character_cheat != 0 {
            self.w32(o + 0x4c, 0x54);
            self.w32(o + 0x58, 0xc);
            self.w32(o + 0x48, 0x91);
            self.w32(o + 0x80, 0);
            self.w32(o + 0x50, 0xc);
            self.w32(o + 0x54, 0xc);
            if mode != 6 {
                self.w32(o + 0x3c, 1);
                self.w32(o + 0x60, 0x3f4c_cccd);
            }
        }
        let mut changed = self.r32(o + 0x80) != 2;
        if changed {
            self.bytes[o + 0x64] = 0xc;
        }
        if env.mallora && self.r32(o + 0x38) == 7 {
            self.w32(o + 0x4c, 0x55);
            changed = true;
            self.w32(o + 0x48, 0x90);
        }
        let base: [u8; 16] = entry[0xc..0x1c].try_into().unwrap();
        let caps: [u8; 4] = entry[0x1c..0x20].try_into().unwrap();
        let bonus: [u8; 4] = board_points[bp + 8..bp + 12].try_into().unwrap();
        let levels: [u8; 4] = self.bytes[rec + 0xc..rec + 0x10].try_into().unwrap();
        let use_caps = !((!env.max_attrib && !changed) || mode == 6);
        let bytes = crate::stats::player_bytes(&base, &caps, bonus, levels, use_caps);
        self.bytes[o + 0xe..o + 0x1e].copy_from_slice(&bytes);
    }
}

/// Everything `Career_BuildRaceRiders` and the other roster builders need outside the career block.
pub struct RosterCtx<'a> {
    /// `PreRaceSel` (0x32ecf0)
    pub sel: &'a mut PreRaceSel,
    /// `gRider_Table`: 12 entries of 0x38 bytes (0x331ea8), and the strings their +4 point at (NUL included)
    pub entries: &'a [u8],
    pub names: &'a [Vec<u8>],
    /// the board table at 0x332150
    pub board_points: &'a [u8],
    pub tables: &'a crate::career::StaticTables,
    pub sel_tables: &'a RosterTables<'a>,
    pub env: PlayerRiderEnv,
    pub rel: &'a dyn crate::career::RelSource,
}

impl crate::career::Career {
    fn rider_entry<'a>(ctx: &'a RosterCtx, rider: usize) -> &'a [u8] {
        &ctx.entries[rider * 0x38..rider * 0x38 + 0x38]
    }

    /// The player name the slot's +0xc points at, NUL included (the pointer is into the block).
    fn slot_name(&self, k: usize) -> Vec<u8> {
        let at = self.ptr(0x30fc + 0x44 * k + 0xc);
        self.bytes[at..].iter().copied().take_while(|&b| b != 0).chain(std::iter::once(0)).collect()
    }

    /// `Career_BuildRaceRiders` 0x165cf0: the race's `n` (+0x27dc) riders put in order and set up (`+0x2b0c` = 3).
    /// The human players' riders first (their slots in order, as many as `n`); if the first slot is still empty
    /// a random rider (`Rand_Fx % 12`); the pre-race selection made for it; its co-riders next, then (if short)
    /// random riders by one draw each of `% 10000` in the order of the highest; with two or more riders the
    /// order shuffled (a draw each, `% n`) and the selection's first two co-riders moved beside the first rider
    /// (race 0xb: a second place rider moved to the front half); then each rider is set up (`setup_ai_rider`,
    /// or `setup_player_rider` for a player whose outfit is made different from any other player's of the same
    /// rider for the call); the roster finalised and the pairs initialised.
    pub fn build_race_riders(&mut self, ctx: &mut RosterCtx, rand_fx: &mut dyn FnMut() -> u32) {
        self.w32(0x2b0c, 3);
        let n = self.r32(0x27dc) as i32;
        let mut a = [0xci32; 6]; // the riders
        let mut b = [-1i32; 6]; // their slots
        let mut rec = 0u32; // the last player's record (sp+0x70)
        let mut mask = 0u32;
        let mut count = 0i32;
        if count != n {
            let mut slot = 0usize;
            loop {
                if (self.bytes[0x30fc + 0x44 * slot] as i8) >= 0 {
                    let rider = self.r32(self.ptr(0x3110 + 0x44 * slot)) as i32;
                    b[count as usize] = slot as i32;
                    a[count as usize] = rider;
                    rec = self.r32(0x3114 + 0x44 * slot);
                    mask |= 1u32.wrapping_shl(rider as u32);
                    count += 1;
                }
                slot += 1;
                if slot >= 4 || count == n {
                    break;
                }
            }
        }
        if a[0] == 0xc {
            count += 1;
            a[0] = (rand_fx() % 12) as i32;
            mask |= 1u32.wrapping_shl(a[0] as u32);
        }
        let (p6, p7) = (self.r32(0x2af8), self.r32(0x2afc));
        ctx.sel.choose(a[0] as u32, p6, p7, 3, ctx.sel_tables.rows[..3].try_into().unwrap(), rand_fx);
        let n = self.r32(0x27dc) as i32;
        if count != n {
            for k in 0..5 {
                let v = ctx.sel.co[k];
                if v != 0xc && (mask >> (v as u32 & 31)) & 1 == 0 {
                    a[count as usize] = v;
                    mask |= 1u32.wrapping_shl(v as u32);
                    count += 1;
                }
                if count == n {
                    break;
                }
            }
        }
        if count < n {
            let mut c = [0i32; 12];
            for i in 0..12 {
                c[i] = if (mask >> i) & 1 != 0 { -1 } else { (rand_fx() % 10000) as i32 };
            }
            let n = self.r32(0x27dc) as i32;
            while count < n {
                let (mut best, mut bi) = (-1i32, 12usize);
                for (i, &v) in c.iter().enumerate() {
                    if best < v {
                        best = v;
                        bi = i;
                    }
                }
                a[count as usize] = bi as i32;
                if bi < 12 {
                    c[bi] = -1;
                } else {
                    rec = u32::MAX; // the game's write lands on its record pointer
                }
                count += 1;
            }
        }
        let first = a[0];
        if n >= 2 {
            let nu = n as usize;
            for i in 0..nu {
                let d = rand_fx();
                let n2 = self.r32(0x27dc);
                let j = (d % n2) as usize;
                a.swap(i, j);
                b.swap(i, j);
            }
            let mut t4 = 0usize;
            if a[0] != first {
                t4 = 1;
                while t4 < nu && a[t4] != first {
                    t4 += 1;
                }
            }
            if self.r32(0x2af8) == 0xb && (2..4).contains(&t4) {
                a.swap(1, t4);
                b.swap(1, t4);
                t4 = 1;
            }
            let find = |a: &[i32; 6], v: i32| (0..nu).find(|&i| a[i] == v).map(|i| i as i32).unwrap_or(-1);
            let c0 = ctx.sel.co[0];
            if c0 != 0xc {
                let mut i0 = find(&a, c0);
                if i0 >= 0 {
                    if t4 == 0 {
                        if i0 == 1 {
                            i0 = 0;
                        }
                        a.swap(0, 1);
                        b.swap(0, 1);
                        t4 = 1;
                    }
                    a.swap(i0 as usize, t4 - 1);
                    b.swap(i0 as usize, t4 - 1);
                }
            }
            let c1 = ctx.sel.co[1];
            if c1 != 0xc {
                let mut i1 = find(&a, c1);
                if i1 >= 0 {
                    if t4 == nu - 1 {
                        let a1 = nu - 2;
                        a.swap(t4, a1);
                        b.swap(t4, a1);
                        if i1 as usize == a1 {
                            i1 = t4 as i32;
                        }
                        t4 = a1;
                    }
                    a.swap(i1 as usize, t4 + 1);
                    b.swap(i1 as usize, t4 + 1);
                }
            }
        }
        let mut i = 0i32;
        while i < n.max(0) && (i as usize) < 6 {
            let iu = i as usize;
            let rider = a[iu];
            if b[iu] < 0 {
                let rival = if rec != 0 && rec != u32::MAX { Some(self.r32(rec.wrapping_sub(crate::career::CAREER_BASE) as usize + 0x164 + 8 * rider as usize) as i32) } else { None };
                let entry = Self::rider_entry(ctx, rider as usize);
                let name = &ctx.names[rider as usize];
                self.setup_ai_rider(iu, entry, name, ctx.board_points, rival, ctx.tables, ctx.env.max_attrib, rand_fx);
            } else {
                let s = b[iu] as usize;
                let prof = self.ptr(0x3110 + 0x44 * s);
                let outfit = self.r32(prof + 0xc);
                let mut t = 0;
                while t < 4 {
                    if t == s || (self.bytes[0x30fc + 0x44 * t] as i8) < 0 {
                        t += 1;
                        continue;
                    }
                    let other = self.ptr(0x3110 + 0x44 * t);
                    if self.r32(other) as i32 != rider {
                        t += 1;
                        continue;
                    }
                    let (oo, mine) = (self.r32(other + 0xc), self.r32(prof + 0xc));
                    if oo != mine {
                        t += 1;
                        continue;
                    }
                    self.w32(prof + 0xc, ((oo as i32 + 1) % 7) as u32);
                }
                let player = self.bytes[0x30fc + 0x44 * s] as i8;
                let word = self.r32((0x168c + 4 * player as isize) as usize);
                self.w32(0x3100 + 0x44 * s, word);
                let entry = Self::rider_entry(ctx, self.r32(self.ptr(0x3110 + 0x44 * s)) as usize);
                let name = self.slot_name(s);
                self.setup_player_rider(s, iu, entry, &name, ctx.board_points, ctx.tables, &ctx.env);
                self.w32(prof + 0xc, outfit);
            }
            i += 1;
        }
        self.finalize_roster(ctx.sel, ctx.sel_tables, rand_fx);
        let n = self.r32(0x27dc) as i32;
        self.rivals_init_pairs(n, ctx.rel);
    }
}

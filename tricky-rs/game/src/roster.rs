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

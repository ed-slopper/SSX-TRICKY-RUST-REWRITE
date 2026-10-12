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

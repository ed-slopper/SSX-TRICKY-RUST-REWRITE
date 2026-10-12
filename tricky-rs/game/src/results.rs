//! The race's results (board row G4j): `cAI::vf17_111820` 0x111820(race, place), which scores every rider, fills the
//! results tables (0x32e60c times, 0x32e624 stat blocks, the 0x32e5f4 order) and tells the career about the players'
//! stats. Checked in the function runner (`tests/results.rs`).

use crate::roster::sort_shell_pairs;

/// The 0xbc-byte stats block (rider +0x5894) copied to the results table.
pub const BLOCK: usize = 0xbc;

/// One pair record's fields the results read: rider +0x10 + j*0x20 (+0x40, +0x44, +0x45) and rider +0x5c + j*0x20.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairRec {
    pub b40: i8,
    pub b44: i8,
    pub b45: u8,
    pub w5c: u32,
}

#[derive(Clone, Debug)]
pub struct ResRider {
    /// +0x418: the finish time (negative while racing).
    pub finish: i32,
    /// +0x41c (1 for a human's rider).
    pub human: i32,
    /// The stats record's first word (+0x464 → +0): this rider's table slot.
    pub id: i32,
    /// The stats record's byte +0x65 (negative: a computer rider).
    pub kind: i8,
    /// +0x5910: show-off points.
    pub points: i32,
    /// +0x110: the place, set when `place >= 0`.
    pub rank: i32,
    pub block: [u8; BLOCK],
    /// Pair records j = -1 ..= count-1 (index j + 1).
    pub pairs: Vec<PairRec>,
}

impl ResRider {
    fn pair(&self, j: i32) -> PairRec {
        self.pairs[(j + 1) as usize]
    }
}

/// What the results call out to.
pub trait ResultsEnv {
    /// `Rivals_UpdateAttitude` 0x11f050(rider a, rider b).
    fn attitude(&mut self, a: usize, b: usize);
    /// `Career_FindRiderOfPlayer` 0x166598(player).
    fn find_rider_of_player(&mut self, p: i32) -> i32;
    /// `Career_CountProfiles` 0x164288.
    fn count_profiles(&mut self) -> i32;
    /// `Career_GetPlayerId` 0x166580(p) then `Career_GetPlayerProfile` 0x164330(id): the profile pointer.
    fn player_profile(&mut self, p: i32) -> u32;
    /// `Career_UpdateBestStats` 0x166cb0(profile, stats block, level).
    fn update_best_stats(&mut self, profile: u32, block: &[u8; BLOCK], level: i32);
    /// `FUN_00114f70` (mode 4's after-race career update).
    fn mode4_update(&mut self);
    /// `Boarder_EstimateFinishTime` 0x11f2a0(rider).
    fn estimate_finish(&mut self, rider: usize) -> i32;
    /// `Boarder_GetRaceTime` 0x11ea28(rider).
    fn race_time(&mut self, rider: usize) -> i32;
}

/// The results tables' state: `times` (0x32e60c), `blocks` (0x32e624, 0xbc each), `order` (0x32e5f4).
#[derive(Clone, Debug)]
pub struct Tables {
    pub times: [i32; 8],
    pub blocks: [[u8; BLOCK]; 8],
    pub order: [i32; 8],
}

/// The show-off point threshold for place `p` (0 = first) at `level` (word 0x32de88) in mode `mode`; None when that
/// level sets nothing.
pub fn points_threshold(mode: i32, level: i32, p: i32) -> Option<i32> {
    let pick = |a: i32, b: i32, c: i32| Some(match p { 0 => a, 1 => b, 2 => c, _ => 0 });
    if mode == 3 && level == 6 {
        return pick(800000, 500000, 250000);
    }
    match level {
        0 => pick(55000, 40000, 25000),
        1 => pick(95000, 65000, 35000),
        2 | 3 => pick(0x36ee8, 150000, 75000),
        4 => pick(0x43238, 0x2ab98, 0x1e848),
        5 => pick(0x2ab98, 0x1c138, 75000),
        8 => pick(350000, 0x36ee8, 100000),
        0xb => pick(500000, 300000, 150000),
        _ => None,
    }
}

fn put(block: &mut [u8; BLOCK], off: usize, v: u32) {
    block[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// `cAI::vf17_111820`. `table` is the three-doubleword table at 0x3644e0 (six words) of fixed times.
pub fn results(
    mode: i32,
    level: i32,
    place: i32,
    table: [i32; 6],
    riders: &mut [ResRider],
    t: &mut Tables,
    env: &mut dyn ResultsEnv,
) {
    let n = riders.len();
    let mut l = [-1i32; 2];
    let mut cc = 0usize;
    if mode == 4 || mode == 2 {
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    env.attitude(i, j);
                }
            }
        }
        for (i, r) in riders.iter().enumerate() {
            if r.human == 1 {
                if cc < 2 {
                    l[cc] = i as i32;
                }
                cc += 1;
            }
        }
        if cc == 2 {
            let f = env.find_rider_of_player(0);
            if f != l[0] {
                l.swap(0, 1);
            }
        }
    } else if mode == 5 || mode == 3 || mode == 7 {
        let mut i = 0;
        while i < env.count_profiles() {
            let f = env.find_rider_of_player(i);
            if i < 2 {
                l[i as usize] = f;
            }
            i += 1;
        }
    }
    let mut local_c8 = 0usize;
    let mut keys = [0i32; 8];
    let mut ord = [0i32; 8];
    for i in 0..n {
        let mut c4 = 50000;
        if place < 0 {
            if riders[i].finish < 0 && riders[i].kind >= 0 {
                riders[i].finish = 0;
            } else {
                c4 = env.estimate_finish(i);
            }
        } else if mode == 5 || mode == 3 {
            if let Some(v) = points_threshold(mode, level, place) {
                riders[i].points = v;
                // +0x5910 lies inside the copied stats block (offset 0x7c)
                put(&mut riders[i].block, 0x7c, v as u32);
            }
        } else if riders[i].kind < 0 {
            let mut idx = local_c8;
            if place as usize == local_c8 {
                idx += 1;
            }
            local_c8 = idx + 1;
            c4 = table[idx];
        } else if mode == 7 {
            c4 = env.race_time(i);
        } else {
            c4 = table[place as usize];
        }
        let id = riders[i].id as usize;
        t.times[id] = c4;
        t.blocks[id] = riders[i].block;
        let ii = i as i32;
        {
            let r = &riders[i];
            let b = &mut t.blocks[id];
            // columns at block +0x9c (0x32e6c0): two (kind, a, b, word) groups of 16 bytes… laid out as
            // [c0, c4, c8, cc, d0, d4, d8, dc]
            let one = |b: &mut [u8; BLOCK], g: usize, p: PairRec| {
                put(b, 0x9c + g * 4, p.b45 as u32);
                put(b, 0xa4 + g * 4, p.b44 as i32 as u32);
                put(b, 0xac + g * 4, p.b40 as i32 as u32);
                put(b, 0xb4 + g * 4, p.w5c);
            };
            let from_first = |b: &mut [u8; BLOCK]| {
                let p = r.pair(l[0]);
                one(b, 0, p);
                put(b, 0xa0, 3);
            };
            if cc == 1 {
                if ii == l[0] {
                    put(b, 0x9c, 3);
                    put(b, 0xa0, 3);
                } else {
                    from_first(b);
                }
            } else if ii == l[0] {
                put(b, 0x9c, 3);
                one(b, 1, r.pair(l[1]));
            } else if ii == l[1] {
                from_first(b);
            } else {
                one(b, 0, r.pair(l[0]));
                one(b, 1, r.pair(l[1]));
            }
        }
        if mode != 1 {
            if ii == l[0] {
                let prof = env.player_profile(0);
                env.update_best_stats(prof, &riders[i].block, level);
            }
            if env.count_profiles() == 2 && ii == l[1] {
                let prof = env.player_profile(1);
                env.update_best_stats(prof, &riders[i].block, level);
            }
        }
        ord[i] = ii;
        keys[i] = match mode {
            5 | 3 => riders[i].points,
            6 => ii,
            _ => c4.wrapping_neg(),
        };
    }
    if mode == 4 {
        env.mode4_update();
    }
    sort_shell_pairs(&mut keys[..n], &mut ord[..n]);
    for k in 0..n {
        let v = ord[riders[k].id as usize];
        t.order[k] = v;
        if place >= 0 {
            riders[v as usize].rank = k as i32;
        }
    }
}

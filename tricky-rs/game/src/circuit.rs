//! The World Circuit (board row C12): what a finished circuit race or show-off does to the career, on the rider
//! set-up at career +0x320c (`RiderSetup_Init`: +4 the saved part (+8), +8 the race (+0x27dc), +0xc the places
//! (+0x3264), +0x10 the player's slot, +0x14 the heat, +0x18 the heat's bonus, +0x1c.. twelve rider points) and the
//! circuit's globals (0x3353f0..: the two final-event flags, the unlock list the results screens show). The rider
//! set-up of each heat (`Circuit_SetupRiders`, `Circuit_SetupEventRiders`, `Rivals_InitPairsCircuit`) is row E1b2o's.
//! Checked in the function runner (`tests/circuit.rs`).

use crate::career::{Career, BOARD_POINTS};
use crate::courses::{CIRCUIT, MEDALS};
use crate::stats::CAPS;

/// The rider set-up's offset in the career block
pub const SETUP: usize = 0x320c;

/// A medal's points (0x37ee40: gold, silver, bronze, none)
pub const MEDAL_POINTS: [i32; 4] = [15, 10, 5, 0];

/// The order a gold medal unlocks the characters in (0x3352c0)
pub const CHARACTER_ORDER: [u32; 12] = [3, 4, 7, 0, 10, 5, 6, 1, 11, 8, 9, 2];

/// The rider level thresholds `Circuit_ProcessUnlocks` checks, in its order (a level reached when the total crosses
/// one: the last crossed wins)
pub const LEVELS: [i32; 10] = [0xf0, 200, 0xa0, 0x78, 0x5a, 0x3c, 0x23, 0x14, 5, 0];

/// The heat's bonus by place (+0x18: the float bits `Circuit_ScoreEventResult` stores for first, second, third)
pub const HEAT_BONUS: [u32; 3] = [0x3f553f3c, 0x3eaad18b, 0x3e2ad18b];

/// An entry of the unlock list (0x335400, 12 bytes): who (the player id, or the slot for the trick book and flag
/// entries), what (0 show-off course, 1 race course, 2 next event / final, 3 character, 4 board, 5 trick book page,
/// 6 level, 7 medal, 8 the whole book (the profile word), 9 flags), and which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlock {
    pub who: i32,
    pub kind: u32,
    pub value: i32,
}

/// The circuit's globals: `DAT_003353f0` (the show-off final unlocked, `Circuit_GotShowoffFinalUnlock` 0x16bc40),
/// `DAT_003353f4` (the race final, `Circuit_GotRaceFinalUnlock` 0x16bc30), the list
/// and its count `DAT_003353f8`, and the riders out of the circuit race (`DAT_00335664` + 0x10 · character).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Circuit {
    pub showoff_final: u32,
    pub race_final: u32,
    pub unlocks: Vec<Unlock>,
    pub out: [u32; 12],
}

/// `Circuit_FindEntry` 0x16ba18: the course's place in the race order (8 if none)
pub fn find_entry(course: u32) -> usize {
    CIRCUIT.iter().position(|c| *c == course).unwrap_or(8)
}

/// `Circuit_GetMedalThresholds` 0x16ba60: the course's place in the show-off order (8 if none)
pub fn threshold_entry(course: u32) -> usize {
    MEDALS.iter().position(|(c, _)| *c == course).unwrap_or(8)
}

/// `Circuit_ClampMedalRank` 0x16bb58
pub fn clamp_medal_rank(place: i32) -> i32 {
    if place < 3 {
        place
    } else {
        3
    }
}

impl Circuit {
    fn push(&mut self, who: i32, kind: u32, value: i32) {
        self.unlocks.push(Unlock { who, kind, value });
    }

    /// `Circuit_ProcessRaceResult` 0x16bcb0 (`Career_CheckNewRecords`' circuit call): in a circuit show-off (5)
    /// the medal of rider 0's score, in a circuit race (4) at its final heat the first player rider's place; a better
    /// medal than the course's adds the difference of their points to the event's, the total and every rider's
    /// points; the total is then held to what the caps leave (or set to it with every gold); then the unlocks; a
    /// non-empty list marks the career changed (+0x3258).
    pub fn process_race_result(&mut self, c: &mut Career) {
        self.race_final = 0;
        self.showoff_final = 0;
        self.unlocks.clear();
        let race = c.ptr(SETUP + 8);
        let mode = c.r32(race + 0x320);
        let (old_total, new_total, rank);
        if mode.wrapping_sub(4) > 1 || (mode == 4 && c.r32(SETUP + 0x14) != 2) {
            old_total = 0;
            new_total = 0;
            rank = 3;
        } else {
            let course = c.r32(race + 0x31c) as usize;
            let rec = c.ptr(c.ptr(SETUP + 0x10) + 0x18);
            let old;
            let mut r = 3;
            if mode == 5 {
                old = c.r8(rec + course * 4 + 0x24) as i32;
                let places = c.ptr(SETUP + 0xc);
                r = crate::courses::medal(c.r32(places + 0xac) as i32, course as u32) as i32;
            } else {
                old = c.r8(rec + course * 4 + 0x25) as i32;
                let places = c.ptr(SETUP + 0xc);
                let n = c.r32(race) as i32;
                for i in 0..n.max(0) as usize {
                    let rider = c.r32(places + 4 * i) as usize;
                    if 0 <= c.r8(race + rider * 0x84 + 0x69) as i8 {
                        r = clamp_medal_rank(i as i32);
                        break;
                    }
                }
            }
            rank = r;
            old_total = c.r32(rec + 4).wrapping_add(c.r32(rec + 8)) as i32;
            let delta = MEDAL_POINTS[rank as usize].wrapping_sub(MEDAL_POINTS[old as usize]);
            if rank < old {
                if mode == 5 {
                    c.w32(rec + 8, c.r32(rec + 8).wrapping_add(delta as u32));
                    c.bytes[rec + course * 4 + 0x24] = rank as u8;
                } else {
                    c.w32(rec + 4, c.r32(rec + 4).wrapping_add(delta as u32));
                    c.bytes[rec + course * 4 + 0x25] = rank as u8;
                }
                c.w32(rec, c.r32(rec).wrapping_add(delta as u32));
                add_points_to_all(c, delta);
                let ch = c.r32(c.ptr(c.ptr(SETUP + 0x10) + 0x14)) as usize;
                let caps = CAPS[ch];
                let left: i32 = (0..4).map(|k| caps[k] as i32 - c.r8(rec + 0xc + k) as i32).sum();
                if left < c.r32(rec) as i32 {
                    c.w32(rec, left as u32);
                } else {
                    let race_gold = CIRCUIT.iter().all(|k| c.r8(rec + *k as usize * 4 + 0x25) == 0);
                    if race_gold && MEDALS.iter().all(|(k, _)| c.r8(rec + *k as usize * 4 + 0x24) == 0) {
                        c.w32(rec, left as u32);
                    }
                }
            }
            new_total = c.r32(rec + 4).wrapping_add(c.r32(rec + 8)) as i32;
        }
        self.process_unlocks(c, old_total, new_total, rank);
        if !self.unlocks.is_empty() {
            c.w32(0x3258, 1);
        }
    }

    /// `Circuit_ProcessUnlocks` 0x16c420(old total, new total, medal)
    pub fn process_unlocks(&mut self, c: &mut Career, old: i32, new: i32, rank: i32) {
        let slot = c.ptr(SETUP + 0x10);
        if rank != 3 {
            self.push(c.r8(slot) as i8 as i32, 7, rank);
        }
        // the trick book: pages done, the whole book, the race's flags
        for k in 0..4usize {
            if !c.is_player_active(k) {
                continue;
            }
            let p = c.player_id(k);
            let rec = c.ptr(0x3114 + 0x44 * c.find_player_slot(p) as usize);
            let seen = c.r32(rec + 0x5c);
            let mut n = 0;
            for page in 0..5u32 {
                if c.tricks_done_in_page(p, page) == 5 {
                    n += 1;
                    if seen & (2 << page) == 0 {
                        self.push(k as i32, 5, page as i32 + 1);
                    }
                }
            }
            if c.tricks_done_in_page(p, 5) == 5 && n == 5 && c.r32(rec + 0x20) == 0 {
                let profile = c.r32(c.ptr(0x3110 + 0x44 * c.find_player_slot(p) as usize));
                self.push(k as i32, 8, profile as i32);
            }
            let flags = c.count_player_flags(p) as i32;
            if 0 < flags {
                self.push(k as i32, 9, flags);
            }
        }
        for k in 0..4usize {
            if !c.is_player_active(k) {
                continue;
            }
            let p = c.player_id(k);
            let rec = c.ptr(0x3114 + 0x44 * c.find_player_slot(p) as usize);
            let mut n = 0;
            for page in 0..5u32 {
                if c.tricks_done_in_page(p, page) == 5 {
                    n += 1;
                    c.w32(rec + 0x5c, c.r32(rec + 0x5c) | (2 << page));
                }
            }
            if c.tricks_done_in_page(p, 5) == 5 && n == 5 {
                let mine = c.ptr(slot + 0x18);
                c.w32(rec + 0x20, 1);
                c.w32(mine + 0x58, c.r32(mine + 0x58) | 0x800);
            }
        }
        if old == new {
            return;
        }
        let race = c.ptr(SETUP + 8);
        let saved = c.ptr(SETUP + 4);
        let course = c.r32(race + 0x31c);
        let who = c.r8(slot) as i8 as i32;
        let rec = c.ptr(slot + 0x18);
        // the next event
        let (idx, order, byte, kinds): (usize, [u32; 8], usize, (u32, u32)) = if c.r32(race + 0x320) == 4 {
            (find_entry(course), CIRCUIT, 0x26, (2, 1))
        } else {
            (threshold_entry(course), MEDALS.map(|(k, _)| k), 0x27, (2, 0))
        };
        if idx < 7 {
            let next = order[idx + 1];
            let bits = c.r32(saved);
            if (bits as i32 >> (next & 31)) & 1 == 0 {
                c.w32(saved, bits | 1 << (next & 31));
                self.push(who, kinds.0, next as i32);
            }
            let at = rec + next as usize * 4 + byte;
            if c.r8(at) == 0 {
                c.bytes[at] = 1;
                self.push(who, kinds.1, next as i32);
            }
        }
        // the finals: a medal on the last course
        let last = CIRCUIT[7] as usize;
        if c.r8(rec + last * 4 + 0x25) < 3 {
            let bits = c.r32(saved);
            if (bits as i32 >> 7) & 1 == 0 {
                c.w32(saved, bits | 0x80);
                self.push(who, 2, 7);
                self.race_final = 1;
            }
        }
        if c.r8(rec + last * 4 + 0x24) < 3 {
            let bits = c.r32(saved);
            if (bits as i32 >> 6) & 1 == 0 {
                c.w32(saved, bits | 0x40);
                self.push(who, 2, 6);
                self.showoff_final = 1;
            }
        }
        // a gold: the next character
        if rank == 0 {
            let bits = c.r32(saved + 4);
            for ch in CHARACTER_ORDER {
                if (bits as i32 >> (ch & 31)) & 1 == 0 {
                    c.w32(saved + 4, bits | 1 << (ch & 31));
                    self.push(who, 3, ch as i32);
                    break;
                }
            }
        }
        // boards by points (`DAT_00332154` + character · 0xf0: the same for every character)
        for b in 0..12u32 {
            if BOARD_POINTS[b as usize] as i32 <= new {
                let bits = c.r32(rec + 0x58);
                if (bits as i32 >> b) & 1 == 0 {
                    c.w32(rec + 0x58, bits | 1 << b);
                    self.push(who, 4, b as i32);
                }
            }
        }
        // the level crossed
        let mut level = -1;
        for (i, t) in LEVELS.iter().enumerate() {
            if *t - 1 < new && old < *t {
                level = i as i32;
            }
        }
        if 0 <= level {
            c.w32(0x370c, 1);
            self.push(who, 6, level);
            if level == 0 {
                c.w32(rec + 0x5c, c.r32(rec + 0x5c) | 0x40);
            }
        }
    }

    /// `Circuit_ScoreEventResult` 0x16d0b0: the player's character's place in the heat sets the bonus (+0x18; the
    /// event goes on while it is in the first three and the heat is not the last, 2); riders past third are out; at
    /// the last heat a player still in keeps the riders' points.
    pub fn score_event_result(&mut self, c: &mut Career) -> bool {
        let race = c.ptr(SETUP + 8);
        let places = c.ptr(SETUP + 0xc);
        let heat = c.r32(SETUP + 0x14);
        let mine = c.r32(c.ptr(c.ptr(SETUP + 0x10) + 0x14));
        let mut go_on = heat != 2;
        let n = c.r32(race) as i32;
        for i in 0..n.max(0) as usize {
            let ch = c.r32(race + 0x3c + 0x84 * c.r32(places + 4 * i) as usize);
            if ch == mine {
                match i {
                    0..=2 => c.w32(SETUP + 0x18, HEAT_BONUS[i]),
                    _ => go_on = false,
                }
            }
            if 2 < i {
                self.out[ch as usize] = 1;
            }
        }
        if heat == 2 && self.out[mine as usize] == 0 {
            save_rider_points(c);
        }
        go_on
    }

    /// `Circuit_AdvanceEvent` 0x16c3b0: a circuit race scores its heat and, while it goes on, sets up the next
    /// (`Circuit_SetupEventRiders`, E1b2o): 4; else the heat back to 0: 2
    pub fn advance_event(&mut self, c: &mut Career, setup_event_riders: &mut dyn FnMut(&mut Career, &mut Circuit)) -> u32 {
        if c.r32(c.ptr(SETUP + 8) + 0x320) == 4 {
            if self.score_event_result(c) {
                c.w32(SETUP + 0x14, c.r32(SETUP + 0x14).wrapping_add(1));
                setup_event_riders(c, self);
                return 4;
            }
        }
        c.w32(SETUP + 0x14, 0);
        2
    }

    /// `Circuit_IsFinalEvent` 0x16bbd0: a circuit race or show-off on its last course
    pub fn is_final_event(c: &Career) -> bool {
        let race = c.ptr(SETUP + 8);
        let course = c.r32(race + 0x31c);
        match c.r32(race + 0x320) {
            4 => find_entry(course) == 7,
            5 => threshold_entry(course) == 7,
            _ => false,
        }
    }
}

/// `Circuit_AddPointsToAll` 0x16d1f8: every rider's points in the player's record (+0x164, 8 apart)
pub fn add_points_to_all(c: &mut Career, points: i32) {
    let rec = c.ptr(c.ptr(SETUP + 0x10) + 0x18);
    for k in 0..12 {
        let at = rec + 0x164 + 8 * k;
        c.w32(at, c.r32(at).wrapping_add(points as u32));
    }
}

/// `Circuit_SaveRiderPoints` 0x16c328: the set-up's rider points (+0x1c..) that are set (not −1) into the record of
/// the slot's player (+0x168, 8 apart)
pub fn save_rider_points(c: &mut Career) {
    let slot = c.find_player_slot(c.r8(c.ptr(SETUP + 0x10)) as i8);
    let rec = c.ptr((0x3114 + 0x44 * slot as isize) as usize);
    for k in 0..12 {
        let v = c.r32(SETUP + 0x1c + 4 * k) as i32;
        if 0 <= v {
            c.w32(rec + 0x168 + 8 * k, v as u32);
        }
    }
}

/// `FUN_0016c228`(riders, the race's riders): the last player rider's view of each other rider (the race object's
/// per-rider table, `rival(i, player rider)`) into the set-up's rider points by character
pub fn take_rider_points(c: &mut Career, n: i32, riders: usize, rival: &dyn Fn(usize, i32) -> i8) {
    let mut last = -1i32;
    for i in 0..n.max(0) as usize {
        if 0 <= c.r8(riders + 0x84 * i + 0x65) as i8 {
            last = i as i32;
        }
    }
    for i in 0..n.max(0) as usize {
        if i as i32 != last {
            let ch = c.r32(riders + 0x38 + 0x84 * i) as usize;
            c.w32(SETUP + 0x1c + 4 * ch, rival(i, last) as i32 as u32);
        }
    }
}

/// `FUN_0016bca8`: the heat back to 0
pub fn reset_heat(c: &mut Career) {
    c.w32(SETUP + 0x14, 0);
}

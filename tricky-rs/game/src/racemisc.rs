//! Small race-object helpers (board row G4i): `Game_GetLeadingPlayer` 0x180760, `Game_MoveLeaderToSlot0` 0x180848,
//! `Race_FindPlayerRiders` 0x176720 and `Game_IsLeadOrFocusRider` 0x17f1f0. Checked in the function runner
//! (`tests/racemisc.rs`).

/// One rider slot of the game object's list: the rider's +0x5910 (show-off points) and +0x418 (finish time).
#[derive(Clone, Copy, Debug)]
pub struct LeadKey {
    pub points: i32,
    pub finish: i32,
}

/// `Game_GetLeadingPlayer` 0x180760: in the show-off modes (word 0x32de8c = 3 or 5) the slot with the most points
/// (a later slot wins only if strictly more, from 0.0), else the one with the smallest finish time as a float (a
/// later slot wins only if strictly smaller than the best so far, which starts at 0.0 — so only the first slot
/// or a negative time wins). -1 with no slots.
pub fn leading_player(mode: i32, slots: &[LeadKey]) -> i32 {
    let mut best = -1;
    let mut v = 0.0f32;
    if mode == 5 || mode == 3 {
        for (i, s) in slots.iter().enumerate() {
            if v < s.points as f32 || best == -1 {
                v = s.points as f32;
                best = i as i32;
            }
        }
    } else {
        for (i, s) in slots.iter().enumerate() {
            if (s.finish as f32) < v || best == -1 {
                v = s.finish as f32;
                best = i as i32;
            }
        }
    }
    best
}

/// The word offsets (from +0x90, in the slot) `Game_MoveLeaderToSlot0` swaps: everything up to +0x108 but +0xa8
/// and +0xac.
pub const SWAP_WORDS: [usize; 29] = [
    0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
];

/// `Game_MoveLeaderToSlot0` 0x180848: clears the leader index (+0x68); with two or more slots and a leader past slot
/// 0 it records the index and swaps the two slots' words. `slots[i]` is slot i's words from +0x90 (one per 4
/// bytes; slots are 0x80 apart, so 32 words). Returns the new +0x68.
pub fn move_leader_to_slot0(mode: i32, keys: &[LeadKey], slots: &mut [[u32; 32]]) -> i32 {
    if keys.len() > 1 {
        let k = leading_player(mode, keys);
        if k > 0 {
            let k = k as usize;
            for &w in SWAP_WORDS.iter() {
                let t = slots[0][w];
                slots[0][w] = slots[k][w];
                slots[k][w] = t;
            }
            return k as i32;
        }
    }
    0
}

/// `Race_FindPlayerRiders` 0x176720 → (player 0's index, player 1's index, the one before player 0, the one after).
/// `owner(i)` is rider i's +0x38. The "after" test runs on the last matching index (-1 counts as before the end).
pub fn find_player_riders(count: i32, owner: &dyn Fn(i32) -> i32) -> [i32; 4] {
    let mut out = [-1; 4];
    for i in 0..count {
        if owner(i) == 0 {
            out[0] = i;
        }
        if owner(i) == 1 {
            out[1] = i;
        }
    }
    let a = out[0];
    if a > 0 {
        out[2] = a - 1;
    }
    if a < count - 1 {
        out[3] = a + 1;
    }
    out
}

/// What `Game_IsLeadOrFocusRider` 0x17f1f0 reads.
pub struct LeadOrFocus<'a> {
    /// The camera controller pointer is zero (so the slots decide).
    pub no_camera: bool,
    /// The camera's +0x20.
    pub cam_word: i32,
    pub race_count: i32,
    /// Rider i (race order) is the camera's focus rider.
    pub is_focus: &'a dyn Fn(i32) -> bool,
    /// Rider i's +0x460 (race order).
    pub race_id: &'a dyn Fn(i32) -> i32,
    /// The game object's slots: (+0x110 rank, +0x460 id).
    pub slots: &'a [(i32, i32)],
}

pub fn is_lead_or_focus_rider(c: &LeadOrFocus, id: i32) -> bool {
    if !c.no_camera && c.cam_word != 0 && c.cam_word < 10 {
        for i in 0..c.race_count {
            if (c.is_focus)(i) && (c.race_id)(i) == id {
                return true;
            }
        }
        return false;
    }
    let mut best = 30000;
    let mut who = 0;
    for &(rank, rid) in c.slots {
        if rank < best {
            who = rid;
            best = rank;
        }
    }
    id == who
}

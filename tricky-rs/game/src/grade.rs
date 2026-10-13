//! The score's grades and small trick-list helpers (board row G4l): `Score_GetGradeForPoints` 0x155588,
//! `Score_PointsToIconTier` 0x156900, `TrickId_Matches` 0x157c18, `FUN_00155538`, `Score_ClearTrickEntries`
//! 0x157308, `Scoring_MsgCheckpointTime` 0x155c60, and the relation table's tolerance and cap
//! (`RelTable_GetTolerance` 0x167dc0, `RelTable_GetAttitudeCap` 0x167de0). Checked in the function runner
//! (`tests/aipad.rs`).

/// The points a grade needs by the rider's level (the character entry's +0x74, 0..9): grades 0..3, the next
/// thresholds down; 4 for any points from 0, 5 below 0 or for a level out of 0..9. (Level 2's grade 2 asks for
/// 36000, which its grade 1 already took: a level-2 rider goes from grade 1 to grade 3.)
pub const GRADES: [[i32; 4]; 10] = [
    [50000, 40000, 30000, 20000],
    [48000, 38000, 28000, 18000],
    [46000, 36000, 36000, 16000],
    [44000, 34000, 24000, 14000],
    [42000, 32000, 22000, 12000],
    [40000, 30000, 20000, 10000],
    [38000, 28000, 18000, 8000],
    [36000, 26000, 16000, 6000],
    [34000, 24000, 14000, 4000],
    [32000, 22000, 12000, 2000],
];

/// `Score_GetGradeForPoints` 0x155588: the grade of `points` (`Score_CalcPoints` of the score's +0x24 units and
/// +0x28 multiplier) for a rider of `level`
pub fn grade_for_points(points: i32, level: i32) -> u32 {
    if !(0..10).contains(&level) {
        return 5;
    }
    let t = GRADES[level as usize];
    for (g, need) in t.iter().enumerate() {
        if *need <= points {
            return g as u32;
        }
    }
    if 0 <= points {
        4
    } else {
        5
    }
}

/// `Score_PointsToIconTier` 0x156900: the trick's points as its icon (4000 → 2, 8000 → 3, 12000 → 4, 16000 → 5,
/// others 0)
pub fn icon_tier(points: i32) -> u32 {
    match points {
        4000 => 2,
        8000 => 3,
        12000 => 4,
        16000 => 5,
        _ => 0,
    }
}

/// `TrickId_Matches` 0x157c18: the same first word and the same rotation fields (0x7000f00) of the second
pub fn trick_id_matches(a: [u32; 2], b: [u32; 2]) -> bool {
    a[0] == b[0] && (a[1] ^ b[1]) & 0x0700_0f00 == 0
}

/// `FUN_00155538`: the first of the score's twelve entries (+0x15c, 0x14 apart) of kind 8 or 7, whether it was 8;
/// found or not (none past the twelfth: 0, the flag of the last read)
pub fn find_kind_8_or_7(kinds: &[i32; 12]) -> (bool, bool) {
    for k in kinds {
        if *k == 8 {
            return (true, true);
        }
        if *k == 7 {
            return (true, false);
        }
    }
    (false, false)
}

/// `Score_ClearTrickEntries` 0x157308(kind): every entry of that kind cleared to 0
pub fn clear_trick_entries(kinds: &mut [i32; 12], kind: i32) {
    for k in kinds.iter_mut() {
        if *k == kind {
            *k = 0;
        }
    }
}

/// `Scoring_MsgCheckpointTime` 0x155c60(seconds, late): the message pushed (`Scoring_PushMessage`): its duration (5
/// or 2.5), kind (10 ahead, 11 behind) and the time as an int (negated when ahead)
pub fn checkpoint_time_message(seconds: f32, late: bool) -> (f32, u32, i32) {
    if late {
        (2.5, 0xb, seconds as i32)
    } else {
        (5.0, 10, (-seconds) as i32)
    }
}

/// `RelTable_GetTolerance` 0x167dc0 / `RelTable_GetAttitudeCap` 0x167de0(a, b): the table's words +0 / +8 of the
/// pair (0xc0 a row, 0x10 a pair)
pub fn rel_word(table: &[u8], a: usize, b: usize, word: usize) -> u32 {
    let at = a * 0xc0 + b * 0x10 + 4 * word;
    u32::from_le_bytes(table[at..at + 4].try_into().unwrap())
}

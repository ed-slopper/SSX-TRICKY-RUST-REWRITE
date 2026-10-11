//! The rider's stat bytes as the game builds them (board row G20a): the 16 bytes at stats+0x0e..0x1d of a slot's
//! entry in the race table (0x32db70 + slot · 0x84, `rider+0x464`), which the ground tick, the jump and the meter read.
//!
//! Proven from `Career_SetupPlayerRider` 0x16a9d8, `Career_SetupAIRider` 0x16ae68 and `Career_AverageStatGroups`
//! 0x16b298: the entry starts as the rider's 16 base bytes (`DAT_00331eb4 + rider · 0x38`); four groups of those
//! bytes (the map at 0x335260: edging, speed, stability, tricks) are then mixed with the board and the rider's
//! level, and every byte b ends as `b · 255 / 100` in integers. The bytes of one group hold the same base, so the
//! game's grip (+0x13) is the edging group's second byte and its cubic drag term (+0x19) the speed group's second
//! byte, not two stats of their own. Bytes in no group (1, 2, 7, 9, 10, 13, 14: jump is 9) stay at the base.
//!
//! The tables below are numbers read out of the exe (rider order = `trickdata::RIDERS`).

/// Stat byte positions (stats+0x0e + index).
pub const SPEED: usize = 0; // +0x0e
pub const EDGING: usize = 3; // +0x11
pub const GRIP: usize = 5; // +0x13
pub const JUMP: usize = 9; // +0x17
pub const CUBIC: usize = 11; // +0x19

/// The four groups' byte positions (map at 0x335260: counts 2, 2, 2, 3): edging, speed, stability, tricks.
pub const GROUPS: [&[usize]; 4] = [&[3, 5], &[0, 11], &[4, 8], &[6, 12, 15]];

/// Each rider's 16 base bytes, `DAT_00331eb4 + rider · 0x38`.
pub const BASE: [[u8; 16]; 12] = [
    [40, 75, 80, 23, 20, 23, 28, 70, 20, 70, 75, 40, 28, 60, 48, 28],
    [28, 48, 95, 28, 13, 28, 43, 80, 13, 90, 94, 28, 43, 40, 50, 43],
    [25, 99, 80, 23, 50, 23, 13, 64, 50, 50, 55, 25, 13, 99, 95, 13],
    [28, 65, 90, 23, 15, 23, 45, 85, 15, 94, 88, 28, 45, 45, 56, 45],
    [30, 85, 80, 23, 23, 23, 35, 75, 23, 70, 80, 30, 35, 66, 84, 35],
    [33, 75, 80, 28, 20, 28, 30, 82, 20, 75, 75, 33, 30, 56, 82, 30],
    [30, 70, 85, 18, 23, 18, 40, 88, 23, 80, 65, 30, 40, 80, 72, 40],
    [30, 85, 75, 25, 25, 25, 30, 70, 25, 70, 70, 30, 30, 88, 64, 30],
    [38, 95, 75, 15, 30, 15, 28, 70, 30, 75, 70, 38, 28, 85, 10, 28],
    [33, 65, 80, 23, 18, 23, 38, 75, 18, 75, 80, 33, 38, 70, 10, 38],
    [43, 40, 75, 20, 33, 20, 15, 70, 33, 70, 65, 43, 15, 90, 10, 15],
    [40, 55, 80, 28, 13, 28, 30, 80, 13, 80, 80, 40, 30, 50, 10, 30],
];
pub const CAPS: [[u8; 4]; 12] = [
    [85, 95, 85, 85],
    [85, 85, 80, 99],
    [85, 85, 99, 80],
    [80, 85, 85, 99],
    [85, 85, 90, 90],
    [85, 90, 85, 90],
    [80, 88, 83, 99],
    [80, 95, 85, 90],
    [90, 88, 88, 85],
    [85, 90, 75, 99],
    [90, 99, 90, 70],
    [90, 95, 80, 85],
];

/// `Career_AverageStatGroups` 0x16b298: each group's base bytes averaged (integer division).
pub fn average_groups(base: &[u8; 16]) -> [u8; 4] {
    let mut avg = [0u8; 4];
    for (g, idx) in GROUPS.iter().enumerate() {
        let sum: i32 = idx.iter().map(|&i| base[i] as i32).sum();
        avg[g] = (sum / idx.len() as i32) as u8;
    }
    avg
}

/// The last loop of both set-ups: every byte b becomes `b · 255 / 100`.
fn scale(b: u8) -> u8 {
    ((b as i32 * 255) / 100) as u8
}

/// The player's stat bytes, the stat part of `Career_SetupPlayerRider` 0x16a9d8. `levels` are the career's four
/// group levels (career record +0xc..+0xf), `bonus` the board's, `use_caps` the max-attribute path (the cheat, or
/// the entry's +0x80 not 2) that takes the rider's cap byte for every grouped stat.
pub fn player_bytes(base: &[u8; 16], caps: &[u8; 4], bonus: [u8; 4], levels: [u8; 4], use_caps: bool) -> [u8; 16] {
    let avg = average_groups(base);
    let mut out = *base;
    for g in 0..4 {
        let a = avg[g] as i32;
        let frac = if avg[g] == 100 { 0.0f32 } else { (levels[g] as i32 - a) as f32 / (100 - a) as f32 };
        for &i in GROUPS[g] {
            out[i] = if use_caps {
                caps[g]
            } else {
                let b = base[i] as f32;
                (bonus[g] as f32 * 0.2 + (b + frac * (100.0 - b)) * 0.8) as i32 as u8
            };
        }
    }
    out.map(scale)
}

/// A computer rider's stat bytes, the stat part of `Career_SetupAIRider` 0x16ae68 (no board). `value` is the word
/// its caller passes (`*param_3`, the rider's rival level; `None` for a null pointer: the bare base bytes).
pub fn ai_bytes(base: &[u8; 16], caps: &[u8; 4], value: Option<i32>, use_caps: bool) -> [u8; 16] {
    let mut out = *base;
    if let Some(v) = value {
        let avg = average_groups(base);
        for g in 0..4 {
            let mut frac = if avg[g] == 100 { 0.0f32 } else { (v as f32 * 0.25) / (100 - avg[g] as i32) as f32 };
            if 1.0 < frac {
                frac = 1.0;
            }
            for &i in GROUPS[g] {
                out[i] = if use_caps { caps[g] } else { (base[i] as f32 + 0.0 + frac * (100.0 - base[i] as f32)) as i32 as u8 };
            }
        }
    }
    out.map(scale)
}

/// Each rider's body scale, stats+0x60 (board row G20b): the float at `gRider_Table` + rider · 0x38 + 0x24, which both
/// set-ups copy to the entry (`Career_SetupPlayerRider` 0x16a9d8 as `DAT_00331ecc`, `Career_SetupAIRider` 0x16ae68
/// from its row's `+0x24`; the computer riders are handed the table's own rows). The ground probe starts its ray
/// `scale · rider+0x244` to the side of the position and `Boarder_UpdatePose` 0x129628 scales its body offsets by it.
/// Rider order = `trickdata::RIDERS`; the bit patterns are the exe's.
pub const SCALE: [f32; 12] = [
    f32::from_bits(0x3f59999a), // 0.85
    f32::from_bits(0x3f333333), // 0.70
    f32::from_bits(0x3f800000), // 1.00
    f32::from_bits(0x3f400000), // 0.75
    f32::from_bits(0x3f5eb852), // 0.87
    f32::from_bits(0x3f59999a), // 0.85
    f32::from_bits(0x3f666666), // 0.90
    f32::from_bits(0x3f733333), // 0.95
    f32::from_bits(0x3f59999a), // 0.85
    f32::from_bits(0x3f666666), // 0.90
    f32::from_bits(0x3f7ae148), // 0.98
    f32::from_bits(0x3f59999a), // 0.85
];

/// The scale the all-max-attributes word (`DAT_003352b8` != 0) gives the player outside the trick tutorial (mode
/// `DAT_0032de8c` 6): `0x3f4ccccd`. The computer riders keep their row's.
pub const SCALE_ALL_MAX: f32 = f32::from_bits(0x3f4ccccd);

/// The player's stats+0x60, the scale part of `Career_SetupPlayerRider` 0x16a9d8.
pub fn player_scale(rider: usize, all_max: bool, mode: u32) -> f32 {
    if all_max && mode != 6 { SCALE_ALL_MAX } else { SCALE[rider] }
}

/// A computer rider's stats+0x60, the scale part of `Career_SetupAIRider` 0x16ae68.
pub fn ai_scale(rider: usize) -> f32 {
    SCALE[rider]
}

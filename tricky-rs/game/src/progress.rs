//! How far along the race a rider is, and what that says about time (board row G1f), each the game's and checked in
//! the function runner (`tests/progress.rs`): `Boarder_GetRaceProgress` 0x1170d0, `Boarder_GetTimeGap` 0x11f118,
//! `Boarder_EstimateFinishTime` 0x11f2a0 and `Boarder_IsOnRecordPace` 0x11f350.

/// What the progress functions read of a rider.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    /// +0x110: the place (0 first, 1 second, … negative before the start)
    pub rank: i32,
    /// +0x114: laps completed (0 in a point-to-point race); +0x374 how far round the lap, +0x37c where the lap line is
    pub laps: i32,
    pub along: f32,
    pub line: f32,
    /// +0x418: the finishing time once it has one (negative until then) and `Boarder_ElapsedTime` 0x11ea28 (the
    /// race clock, in ticks, that stops at the finish)
    pub finish: i32,
    pub time: i32,
}

/// `Boarder_GetRaceProgress`: +0x374 with no lap done; else (laps, one fewer once the rider is past the lap line
/// +0x37c: `along < line` counts it as the next lap's) × the lap length (`lap_len`: the race object's +0x38,
/// 1.0 when that is not positive) + +0x374.
pub fn race_progress(r: &Progress, lap_len: f32) -> f32 {
    if r.laps == 0 {
        return r.along;
    }
    let len = if lap_len <= 0.0 { 1.0 } else { lap_len };
    if r.along < r.line {
        r.laps as f32 * len + r.along
    } else {
        (r.laps - 1) as f32 * len + r.along
    }
}

/// `Boarder_GetTimeGap`(me): in ticks, how far `me` is from the rider it races for the lead: the first other rider
/// in race order with the place 1 when `me` is first (place 0), with the place 0 when `me` is behind (place above 0);
/// 0 if there is none or neither is still short of `total` (`Boarder_RaceLength` 0x11ea20 of me). The gap
/// is the time `me` is said to need to cover the other's distance at its own average pace, plus the difference of
/// their clocks.
pub fn time_gap(me: &Progress, others: &[Progress], total: f32, lap_len: f32) -> i32 {
    for o in others {
        let pick = if me.rank == 0 { o.rank == 1 } else { me.rank > 0 && o.rank == 0 };
        if !pick {
            continue;
        }
        let behind_o = total - race_progress(o, lap_len);
        let behind_me = total - race_progress(me, lap_len);
        let pace = if behind_me <= 0.0 {
            if behind_o <= 0.0 {
                return 0;
            }
            o.time as f32 / behind_o
        } else {
            me.time as f32 / behind_me
        };
        return ((pace * (behind_o - behind_me)) as i32).wrapping_add(me.time).wrapping_sub(o.time);
    }
    0
}

/// `Boarder_EstimateFinishTime`: the finish time once there is one (+0x418 not negative: the race clock); else the
/// clock scaled by how far round the whole race the rider is: clock × total / (total − progress), the clock itself
/// when the rider is at the end.
pub fn estimate_finish_time(r: &Progress, total: f32, lap_len: f32) -> i32 {
    if r.finish >= 0 {
        return r.time;
    }
    let left = total - race_progress(r, lap_len);
    if left <= 0.0 {
        return r.time;
    }
    ((r.time as f32 * total) / left) as i32
}

/// `Boarder_IsOnRecordPace`: whether the estimated finish is under the course's record (the best row's time, the
/// record list's entry `count − 1`, +0x10 of 0x14).
pub fn is_on_record_pace(estimate: i32, record_time: i32) -> bool {
    estimate < record_time
}

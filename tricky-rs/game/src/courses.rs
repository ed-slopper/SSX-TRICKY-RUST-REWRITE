//! The courses and game modes as the game defines them (board row C0), from its own tables, checked against the
//! ELF and the function runner (`tests/courses.rs`).
//!
//! - The course table 0x332f28 (13 records of 7 words): number, display name, menu code, the show-off time limit in
//!   hundredths, the file name (twice), a flag (5, or 0 for the front end). `Course_OpenModelsBig` 0x186268 builds
//!   the runtime list (gApp+100: count, then 0x84-byte records: `data/models/<file>.big` at +4, the file name at +0x44,
//!   the number at +0x84; the front end skipped); `Course_GetInfo` 0x186350 gives a record's +0x44, the file name the
//!   audio config is matched on ("gari", "megaple").
//! - The course being played is 0x32de88 (its number); the game mode 0x32de8c ([`Mode`]).
//! - The World Circuit's order and medal thresholds: 0x3352f0 and 0x335370.

/// One course of the game's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Course {
    pub number: u32,
    pub name: &'static str,
    pub code: &'static str,
    /// the show-off time limit, hundredths of a second (0: none)
    pub showoff_time: u32,
    pub file: &'static str,
}

/// The course table 0x332f28 (the front end, number 12, left out).
pub const COURSES: [Course; 12] = [
    Course { number: 0, name: "Garibaldi", code: "GARIBALDI", showoff_time: 12000, file: "gari" },
    Course { number: 1, name: "Snowdream", code: "SNOWDREAM", showoff_time: 9000, file: "snow" },
    Course { number: 2, name: "Elysium Alps", code: "ELYSIUM", showoff_time: 9000, file: "elysium" },
    Course { number: 3, name: "Mesablanca", code: "MESABLANCA", showoff_time: 9000, file: "mesa" },
    Course { number: 4, name: "Merqury City", code: "MERQCITY", showoff_time: 9000, file: "merquer" },
    Course { number: 5, name: "Aloha Ice Jam", code: "ICEJAM", showoff_time: 9000, file: "aloha" },
    Course { number: 6, name: "Pipedream", code: "PIPEDREAM", showoff_time: 9000, file: "pipe" },
    Course { number: 7, name: "Untracked", code: "UNTRACKED", showoff_time: 0, file: "untrack" },
    Course { number: 8, name: "Tokyo Megaplex", code: "MEGAPLEX", showoff_time: 9000, file: "megaple" },
    Course { number: 9, name: "Big Air Dome", code: "BIGAD", showoff_time: 9000, file: "bigair" },
    Course { number: 10, name: "Trick Tutorial", code: "TRICKT", showoff_time: 0, file: "trick" },
    Course { number: 11, name: "Alaska", code: "ALASKA", showoff_time: 13500, file: "alaska" },
];

/// The course whose file (or level folder's archive name) is `file`.
pub fn by_file(file: &str) -> Option<&'static Course> {
    COURSES.iter().find(|c| c.file.eq_ignore_ascii_case(file))
}

/// The game mode word 0x32de8c, as `cFE3DStateEventSel::CreateNextState` 0x29ad48 sets it from the event menu
/// (whose items are freeride, head-to-head, race, show-off and the trick tutorial) and the game type
/// (gApp+0x734 → +0xf0: 0 practice, 1 single event, 2 World Circuit; their descriptions are strings 0xca3, 0xca1,
/// 0xca2). The names of 0 and 1 are from what reads them; the others from the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Mode {
    /// 0: knock-down practice (the lower knock-down speed of `Boarder_RiderHit`)
    Knockdown = 0,
    /// 1: the event menu's second item (the number-of-players screen follows: head to head)
    HeadToHead = 1,
    /// 2: a race outside the World Circuit (a difficulty is chosen)
    Race = 2,
    /// 3: a show-off outside the World Circuit (timed, medals)
    Showoff = 3,
    /// 4: a World Circuit race (heats; the top three go on)
    CircuitRace = 4,
    /// 5: a World Circuit show-off (timed, medals)
    CircuitShowoff = 5,
    /// 6: the trick tutorial
    Tutorial = 6,
    /// 7: freeride
    Freeride = 7,
}

/// The World Circuit's courses in order (0x3352f0, the course numbers).
pub const CIRCUIT: [u32; 8] = [0, 1, 2, 3, 4, 8, 5, 11];

/// The show-off medal thresholds per course (0x335370: gold, silver, bronze points); a course not listed uses the
/// last (Alaska's).
pub const MEDALS: [(u32, [i32; 3]); 8] = [
    (0, [55000, 40000, 25000]),
    (1, [95000, 65000, 35000]),
    (2, [225000, 150000, 75000]),
    (3, [225000, 150000, 75000]),
    (4, [275000, 175000, 125000]),
    (8, [350000, 225000, 100000]),
    (5, [175000, 115000, 75000]),
    (11, [500000, 300000, 150000]),
];

/// `Circuit_GetMedal` 0x16baa8: the medal for `points` on course `course`: 0 gold, 1 silver, 2 bronze, 3 none.
/// Pipedream (6) has its own: 800000, 500000, 250000.
pub fn medal(points: i32, course: u32) -> u32 {
    if course == 6 {
        return if points < 800_000 {
            if points < 500_000 {
                if 249_999 < points { 2 } else { 3 }
            } else {
                1
            }
        } else {
            0
        };
    }
    let t = MEDALS.iter().find(|(c, _)| *c == course).map(|(_, t)| *t).unwrap_or(MEDALS[7].1);
    t.iter().position(|v| *v <= points).map(|i| i as u32).unwrap_or(3)
}

/// `cRaceHandler::Enter` 0x113740: the race clock's limit in seconds (race+0x14): the course's show-off time in the
/// show-off modes (3, 5), else 0 (none).
pub fn time_limit(course: u32, mode: u32) -> f32 {
    if mode == 3 || mode == 5 {
        COURSES.iter().find(|c| c.number == course).map(|c| c.showoff_time as i32 as f32 / 100.0).unwrap_or(0.0)
    } else {
        0.0
    }
}

/// The venue menu's order (`cFE3DStateVenueSel`, the course numbers at 0x3a8af0): the World Circuit's eight, then
/// Untracked and Pipedream. Its `UpdateLocks` 0x29baa0 greys each with [`course_unlocked`] for the current mode; the
/// help line for Pipedream and Untracked asks with mode 1 (unlocked: strings 0xde7, 0xde8; else the career's hint).
pub const VENUES: [u32; 10] = [0, 1, 2, 3, 4, 8, 5, 11, 7, 6];

/// The career state `Career_IsCourseUnlocked` reads (the career at 0x32b390).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlocks {
    /// +8: one bit per course unlocked for the single events
    pub bits: u32,
    /// +0x30fc (signed byte): below 0, no World Circuit career is open
    pub career_slot: i8,
    /// +0x3114 → +0x26 + course·4 / +0x27: the course is open in the circuit's race / show-off
    pub circuit: [[u8; 2]; 12],
}

/// The courses each mode may ever offer (`Career_IsCourseUnlocked`'s stack table, modes 0..9): every bit but 10 for
/// knock-down and head-to-head, no Pipedream, Untracked or tutorial for a race, no Untracked or tutorial for show-off,
/// the tutorial mode bits 9 up.
pub const MODE_COURSES: [u32; 10] = [0xfffffbff, 0xfffffbff, 0xfffffb3f, 0xfffffb7f, 0xfffffb3f, 0xfffffb3f, 0xfffffe00, 0xfffffb3f, 0xfffffbff, 0xffffffff];

/// `Career_IsCourseUnlocked` 0x163bb8: may `course` be played in `mode`? With the unlock-everything cheat
/// (0x3352a0), whatever the mode allows. Else for the circuit modes 4, 5 and the tutorial 6: the trick tutorial
/// (10) always, nothing without a career, else the career's circuit byte (race for 4, show-off for 5 and 6); for the
/// others the mode allows it and the career's bit is set. (`mode` 0..9, `course` 0..11 as the game asks.)
pub fn course_unlocked(u: &Unlocks, mode: u32, course: u32, cheat: bool) -> bool {
    let mask = MODE_COURSES[mode as usize];
    if cheat {
        return (mask as i32 >> (course & 31)) & 1 != 0;
    }
    if mode.wrapping_sub(4) < 3 {
        if course == 10 {
            true
        } else if u.career_slot < 0 {
            false
        } else {
            u.circuit[course as usize][if mode == 4 { 0 } else { 1 }] != 0
        }
    } else {
        u.bits & (1 << (course & 31)) & mask != 0
    }
}

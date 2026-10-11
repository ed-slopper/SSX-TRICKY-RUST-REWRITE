//! Riders' feelings about each other (`RelTable_Init` 0x167208) and the grudges ("vendettas") a
//! computer rider builds against a player who keeps hitting it (`Rider_OnKnockedDownBy`,
//! `AI_StartVendetta`, `AI_CoolVendetta`). Riders in the original's order: Eddie, Kaori, Luther,
//! Mac, Moby, Zoe, JP, Elise, Psymon, Seeiah, Brodi, Marisol.

pub const NAMES: [&str; 12] = ["Eddie", "Kaori", "Luther", "Mac", "Moby", "Zoe", "JP", "Elise", "Psymon", "Seeiah", "Brodi", "Marisol"];
pub fn index(name: &str) -> usize { NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or(0) }

/// (friend, foe) of each rider, as the row feels it.
const FRIEND_FOE: [(usize, usize); 12] = [(7, 4), (10, 8), (6, 3), (1, 4), (5, 6), (4, 6), (2, 10), (0, 11), (5, 1), (11, 2), (0, 2), (9, 7)];
/// 0 neutral, 1 friend, 2 foe (`RelTable_Init` 0x167208's table)
pub fn relation(me: usize, other: usize) -> u8 {
    let (f, e) = FRIEND_FOE[me.min(11)];
    if other == f { 1 } else if other == e { 2 } else { 0 }
}
/// how much `me` dislikes `other` at the start (0..100; 60 and over bears grudges, 81 and over chases)
pub const ATTITUDE: [[u8; 12]; 12] = [
    [50, 34, 62, 30, 82, 63, 62, 10, 63, 33, 35, 62], [23, 50, 30, 22, 27, 25, 37, 27, 82, 25, 10, 24],
    [66, 61, 50, 83, 67, 66, 18, 68, 40, 69, 65, 64], [28, 11, 70, 50, 81, 30, 70, 65, 65, 62, 23, 62],
    [65, 65, 70, 65, 60, 10, 83, 65, 68, 24, 64, 27], [63, 28, 70, 37, 10, 50, 83, 70, 63, 22, 33, 23],
    [65, 65, 15, 68, 67, 64, 60, 28, 66, 65, 82, 25], [10, 31, 70, 68, 69, 67, 65, 60, 68, 68, 27, 83],
    [70, 82, 70, 70, 70, 19, 70, 70, 60, 70, 67, 70], [31, 32, 82, 70, 22, 69, 65, 65, 64, 50, 26, 10],
    [10, 24, 83, 25, 62, 35, 50, 21, 65, 30, 50, 24], [63, 28, 68, 68, 25, 23, 63, 82, 62, 13, 26, 50],
];
/// how much knocking about `me` takes from `other` before holding a grudge
pub const TOLERANCE: [[u8; 12]; 12] = [
    [66, 80, 33, 85, 16, 66, 33, 100, 35, 70, 66, 66], [80, 100, 56, 80, 71, 66, 55, 62, 20, 66, 100, 70],
    [33, 100, 100, 16, 33, 59, 93, 66, 50, 57, 54, 50], [80, 100, 33, 100, 16, 71, 33, 50, 60, 79, 66, 60],
    [66, 66, 33, 44, 66, 99, 16, 50, 66, 87, 50, 83], [45, 66, 33, 50, 95, 66, 16, 33, 45, 81, 66, 75],
    [33, 60, 90, 48, 50, 59, 66, 66, 50, 66, 16, 70], [100, 60, 15, 33, 33, 33, 33, 66, 33, 33, 66, 15],
    [50, 36, 50, 50, 50, 66, 50, 50, 66, 50, 50, 50], [66, 77, 16, 50, 87, 59, 33, 28, 50, 66, 66, 100],
    [100, 90, 15, 70, 68, 88, 60, 50, 63, 80, 66, 80], [66, 66, 33, 66, 66, 95, 33, 16, 50, 99, 86, 66],
];

/// the most `me`'s attitude to `other` can reach (`RelTable_Init` 0x167208's word +8; `Rivals_InitPairs` 0x167ea0
/// copies it to the pair's byte +0x21 and `Rivals_StartGrudge` holds the attitude to it). Mostly 100; 79-80 toward a
/// friend (never a chasing grudge, 81), and Kaori and Brodi 59 toward most (never a grudge at all, 60).
pub const CAP: [[u8; 12]; 12] = [
    [100, 100, 100, 100, 100, 100, 100, 80, 100, 100, 100, 100], [59, 100, 79, 59, 59, 59, 79, 79, 100, 59, 59, 59],
    [100, 100, 100, 100, 100, 100, 79, 100, 100, 100, 100, 100], [100, 79, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100],
    [100, 100, 100, 100, 100, 79, 100, 100, 100, 100, 100, 100], [100, 100, 100, 100, 79, 100, 100, 100, 100, 100, 100, 100],
    [100, 100, 79, 100, 100, 100, 100, 100, 100, 100, 100, 100], [80, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100],
    [100, 100, 100, 100, 100, 79, 100, 100, 100, 100, 100, 100], [100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 79],
    [59, 59, 100, 79, 80, 79, 79, 80, 79, 59, 100, 59], [100, 100, 100, 100, 100, 100, 100, 100, 100, 79, 100, 100],
];

/// One computer rider's grudge against the player, kept with the game's own bookkeeping ([`RivalRider`], checked
/// bit for bit): the computer rider's record of the player (attitude, cap, tolerance, anger, the counts, rivalry)
/// and its grudge, and the player's record of it. Time is the game's tick (60 a second).
#[derive(Clone, Debug)]
pub struct Grudge {
    ai: RivalRider,
    player: RivalRider,
}

const AI: i32 = 1;
const PLAYER: i32 = 0;

impl Grudge {
    /// The pair as `Rivals_InitPairs` 0x167ea0 sets it for a computer rider `me` and the player: the table's
    /// attitude (also the start's), cap, tolerance and relation; no anger, no counts, no grudge.
    pub fn new(me: usize, player: usize) -> Self {
        Self::with_attitude(me, player, ATTITUDE[me.min(11)][player.min(11)] as i8)
    }

    fn with_attitude(me: usize, player: usize, attitude: i8) -> Self {
        let (m, p) = (me.min(11), player.min(11));
        let rec = PairBytes {
            attitude,
            cap: CAP[m][p] as i8,
            tolerance: TOLERANCE[m][p] as i8,
            start: attitude,
            relation: relation(m, p),
            ..PairBytes::default()
        };
        let back = PairBytes { attitude: ATTITUDE[p][m] as i8, cap: CAP[p][m] as i8, tolerance: TOLERANCE[p][m] as i8, start: ATTITUDE[p][m] as i8, relation: relation(p, m), ..PairBytes::default() };
        let mut ai = RivalRider { index: AI, human: false, recs: vec![PairBytes::default(); 2], ..RivalRider::default() };
        ai.recs[PLAYER as usize] = rec;
        let mut pl = RivalRider { index: PLAYER, human: true, recs: vec![PairBytes::default(); 2], ..RivalRider::default() };
        pl.recs[AI as usize] = back;
        Grudge { ai, player: pl }
    }

    /// Starting from how it felt about the player after the last heat: `Rivals_DecayAttitudes` 0x168028 wears it
    /// down between heats (a friend by 3, a neutral by 2, a foe by 1), never below 0 nor the table's.
    pub fn carried(me: usize, player: usize, last: Option<f32>) -> Self {
        let Some(a) = last else { return Self::new(me, player) };
        let (m, p) = (me.min(11), player.min(11));
        let mut att = a as i32 as i8;
        att = match relation(m, p) {
            1 => att.wrapping_sub(3),
            0 => att.wrapping_sub(2),
            2 => att.wrapping_sub(1),
            _ => att,
        };
        if att < 0 {
            att = 0;
        }
        let table = ATTITUDE[m][p] as i8;
        if att < table {
            att = table;
        }
        Self::with_attitude(me, player, att)
    }

    /// After the race (`Rivals_UpdateAttitude` 0x11f050): back to the start's attitude with no contact, else
    /// keeping (bumps + 2 shoves + 3 knock-downs) / (3 n) of the gain.
    pub fn settled(&self) -> f32 {
        let mut ai = self.ai.clone();
        ai.update_attitude(PLAYER);
        ai.recs[PLAYER as usize].attitude as f32
    }

    /// The player put this rider down (30), shoved it off balance (15) or bumped it off balance (9)
    /// (`Rivals_OnKnockdown`, `Rivals_OnHit`, `Rivals_OnBump`). `now` in seconds of race.
    pub fn hit(&mut self, amount: f32, now: f32) {
        let t = tick(now);
        let mut calls = Vec::new();
        if amount >= 30.0 {
            self.ai.on_knockdown(&mut self.player, t, &mut calls);
        } else if amount >= 15.0 {
            self.ai.on_hit(&mut self.player, t, &mut calls);
        } else {
            self.ai.on_bump(&mut self.player, t, &mut calls);
        }
    }

    /// It got its own back: it put the player down (`knocked`) or shoved it off balance (the player's
    /// `Rivals_OnKnockdown` / `Rivals_OnHit`: the grudge eased, the rivalry up).
    pub fn got_even(&mut self, knocked: bool) {
        let mut calls = Vec::new();
        if knocked {
            self.player.on_knockdown(&mut self.ai, 0, &mut calls);
        } else {
            self.player.on_hit(&mut self.ai, 0, &mut calls);
        }
    }

    /// `Rivals_UpdateGrudgeTimeout` 0x11efd8: a level-1 grudge cools 360.44952 ticks after it began.
    pub fn tick(&mut self, now: f32) {
        self.ai.update_grudge_timeout(tick(now));
    }

    /// The grudge's level (0..2), or −1 with none.
    pub fn level(&self) -> i8 {
        if self.ai.on != 0 { self.ai.level as i8 } else { -1 }
    }

    /// For checks (TRICKY_GRUDGE, TRICKY_POST): a level-`l` grudge against the player at once.
    pub fn force_level(&mut self, l: i32) {
        self.ai.on = 1;
        self.ai.target = PLAYER;
        self.ai.level = l;
        self.ai.cooled = 0;
    }

    /// The rivalry with the player (pair record +0x1c): grudges +1/+2/+3 by level, shoving the player +1,
    /// knocking it down +2; over 2 after the race, this rider has words with the player.
    pub fn score(&self) -> i32 {
        self.ai.recs[PLAYER as usize].rivalry
    }

    /// Chasing the player: `AIComputer_IsRivalTarget` 0x139928 exactly (a grudge against it at level 2, or level 1
    /// not yet cooled; in knockdown practice, game mode 0, every rider is a target, not ours).
    pub fn hostile(&self) -> bool {
        self.ai.on != 0 && (self.ai.level == 2 || (self.ai.level == 1 && self.ai.cooled == 0))
    }

    /// The grudge as the AI's helpers read it (rider+0x118..0x128; the player is rider 0).
    pub fn of(&self) -> crate::aicomp::GrudgeOf {
        crate::aicomp::GrudgeOf { on: self.ai.on, target: self.ai.target, level: self.ai.level, cooled: self.ai.cooled }
    }

    /// Will shove the player when alongside: level 1 or 2. STANDIN: the game decides inside its rival riding
    /// (`AIComputer_RideAtRival` 0x137a18 / `AIComputer_PickTargetRider` 0x1396c8, row G4), not ported yet.
    pub fn shoves(&self) -> bool {
        self.ai.on != 0 && self.ai.level >= 1
    }
}

/// Seconds of race as the game's ticks.
fn tick(now: f32) -> i32 {
    (now * 60.0).floor() as i32
}

// ---- the game's own grudge bookkeeping, exactly (board row G4a2, checked: `tests/rivals.rs`)

/// One rider's record of another (rider+0x40 + 0x20 × the other's index), the bytes the rivalry functions use:
/// +0x10 attitude, +0x11 its cap, +0x12 the tolerance, +0x13 the anger built up, +0x14 the attitude at the start,
/// +0x15 the relation (1 friend), +0x16 bumps, +0x18 shoves, +0x1a knock-downs (by the other), +0x1c the rivalry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PairBytes {
    pub attitude: i8,
    pub cap: i8,
    pub tolerance: i8,
    pub anger: i8,
    pub start: i8,
    pub relation: u8,
    pub bumps: u16,
    pub shoves: u16,
    pub knocks: u16,
    pub rivalry: i32,
}

/// A rider as the rivalry functions see it: its index (+0x460), whether a pad drives it (stats+0x65, the pad
/// port, ≥ 0), its grudge (+0x118 on, +0x11c since which tick, +0x120 against whom, +0x124 the level, +0x128 cooled)
/// and its records of the others.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RivalRider {
    pub index: i32,
    pub human: bool,
    pub on: i32,
    pub since: i32,
    pub target: i32,
    pub level: i32,
    pub cooled: i32,
    pub recs: Vec<PairBytes>,
}

/// What the rivalry functions call besides each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RivalCall {
    /// `Speech_OnGrudge`: a level-2 grudge begins
    Grudge,
}

impl RivalRider {
    fn rec(&mut self, of: i32) -> &mut PairBytes {
        &mut self.recs[of as usize]
    }

    /// `Rivals_OnBump` 0x11ea70: `by` bumped this rider: one more bump; a computer rider bumped by a human gets 9
    /// angrier (up to its tolerance) and at its tolerance starts a grudge.
    pub fn on_bump(&mut self, by: &mut RivalRider, now: i32, calls: &mut Vec<RivalCall>) {
        self.contact(by, 0, 9, now, calls);
    }

    /// `Rivals_OnHit` 0x11eaf8: shoved off balance by `by`: one more shove; a computer rider shoving a human eases
    /// its grudge if it is against that rider (`Rivals_EaseGrudge` 1) and adds 1 to its rivalry with it; a human
    /// shoving a computer rider makes it 15 angrier.
    pub fn on_hit(&mut self, by: &mut RivalRider, now: i32, calls: &mut Vec<RivalCall>) {
        self.contact(by, 1, 15, now, calls);
    }

    /// `Rivals_OnKnockdown` 0x11ebf8: put down by `by`: as a shove, with ease 2, rivalry 2 and anger 30.
    pub fn on_knockdown(&mut self, by: &mut RivalRider, now: i32, calls: &mut Vec<RivalCall>) {
        self.contact(by, 2, 30, now, calls);
    }

    fn contact(&mut self, by: &mut RivalRider, kind: i32, add: i8, now: i32, calls: &mut Vec<RivalCall>) {
        let r = self.rec(by.index);
        match kind {
            0 => r.bumps = r.bumps.wrapping_add(1),
            1 => r.shoves = r.shoves.wrapping_add(1),
            _ => r.knocks = r.knocks.wrapping_add(1),
        }
        if kind != 0 && !by.human {
            if self.human {
                if by.on != 0 && by.target == self.index {
                    by.ease_grudge(kind);
                }
                let r = by.rec(self.index);
                r.rivalry = r.rivalry.wrapping_add(kind);
            }
            return;
        }
        if by.human && !self.human {
            let r = self.rec(by.index);
            r.anger = r.anger.wrapping_add(add);
            if r.tolerance <= r.anger {
                r.anger = r.tolerance;
            }
            if r.anger == r.tolerance {
                self.start_grudge(by, now, calls);
            }
        }
    }

    /// `Rivals_StartGrudge` 0x11ecf8: a grudge against `at` (started, renewed, or moved from another rider, easing
    /// that one first), its level by the attitude (81 and up 2 (with `Speech_OnGrudge`, rivalry +3), 60 and up 1
    /// (+2), else 0 (+1); a friend always 0), the attitude 15 up (to its cap) and the anger spent.
    pub fn start_grudge(&mut self, at: &RivalRider, now: i32, calls: &mut Vec<RivalCall>) {
        if self.on == 0 {
            self.on = 1;
            self.cooled = 0;
            self.since = now;
            self.target = at.index;
        } else if self.target == at.index {
            self.cooled = 0;
            self.since = now;
        } else {
            self.ease_grudge(0);
            self.on = 1;
            self.cooled = 0;
            self.since = now;
            self.target = at.index;
        }
        let t = self.target;
        let att = self.rec(t).attitude as i32 as f32;
        if 80.989_26 <= att {
            self.level = 2;
            calls.push(RivalCall::Grudge);
            let r = self.rec(t);
            r.rivalry = r.rivalry.wrapping_add(3);
        } else if 60.0 <= att {
            self.level = 1;
            let r = self.rec(t);
            r.rivalry = r.rivalry.wrapping_add(2);
        } else {
            self.level = 0;
            let r = self.rec(t);
            r.rivalry = r.rivalry.wrapping_add(1);
        }
        if self.rec(t).relation == 1 {
            self.level = 0;
        }
        let r = self.rec(t);
        r.attitude = r.attitude.wrapping_add(15);
        if r.cap < r.attitude {
            r.attitude = r.cap;
        }
        r.anger = 0;
    }

    /// `Rivals_EaseGrudge` 0x11ef00: the grudge's anger eased: by 45 (1), by 15 and cooled (0), by half (at least
    /// 45) (2); never below 0.
    pub fn ease_grudge(&mut self, kind: i32) {
        if self.on == 0 {
            return;
        }
        if kind == 0 {
            self.cooled = 1;
        }
        let t = self.target;
        let r = self.rec(t);
        match kind {
            1 => r.anger = r.anger.wrapping_sub(45),
            0 => r.anger = r.anger.wrapping_sub(15),
            2 => {
                let a = r.anger;
                let half = (a as i32 + ((a as u8 >> 7) as i32)) >> 1;
                let d: i8 = if 45 < half { (half as i8).wrapping_neg() } else { -45 };
                r.anger = (a as u8).wrapping_add(d as u8) as i8;
            }
            _ => {}
        }
        if r.anger < 0 {
            r.anger = 0;
        }
    }

    /// `Rivals_UpdateGrudgeTimeout` 0x11efd8: a level-1 grudge not yet cooled cools 360.44952 ticks after it began.
    pub fn update_grudge_timeout(&mut self, now: i32) {
        if self.on != 0 && self.level == 1 && self.cooled == 0 && (self.since as f32 + 360.449_52) < now as f32 {
            self.ease_grudge(0);
        }
    }

    /// `Rivals_UpdateAttitude` 0x11f050, after the race: the attitude toward `of` goes back to the start's, keeping
    /// (bumps + 2·shoves + 3·knock-downs) / (3·(bumps + shoves + knock-downs)) of what it gained (all of it with no
    /// contact... none: the start's).
    pub fn update_attitude(&mut self, of: i32) {
        let r = self.rec(of);
        let s = r.shoves as f32;
        let start = r.start as i32 as f32;
        let n = (r.bumps as f32 + s) + r.knocks as f32;
        if n == 0.0 {
            r.attitude = start as i32 as i8;
            return;
        }
        let k = ((((r.bumps as f32 + s) + s) + r.knocks as f32 * 3.0) / (n * 3.0)) * (r.attitude as i32 as f32 - start);
        r.attitude = (start + k) as i32 as i8;
    }
}

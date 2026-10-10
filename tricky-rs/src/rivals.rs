//! Riders' feelings about each other (`RelTable_Init` 0x167208) and the grudges ("vendettas") a
//! computer rider builds against a player who keeps hitting it (`Rider_OnKnockedDownBy`,
//! `AI_StartVendetta`, `AI_CoolVendetta`). Riders in the original's order: Eddie, Kaori, Luther,
//! Mac, Moby, Zoe, JP, Elise, Psymon, Seeiah, Brodi, Marisol.

pub const NAMES: [&str; 12] = ["Eddie", "Kaori", "Luther", "Mac", "Moby", "Zoe", "JP", "Elise", "Psymon", "Seeiah", "Brodi", "Marisol"];
pub fn index(name: &str) -> usize { NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or(0) }

/// (friend, foe) of each rider, as the row feels it.
const FRIEND_FOE: [(usize, usize); 12] = [(7, 4), (10, 8), (6, 3), (1, 4), (5, 6), (4, 6), (2, 10), (0, 11), (5, 1), (11, 2), (0, 2), (9, 7)];
/// 0 neutral, 1 friend, 2 foe
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

/// One computer rider's grudge against the player.
#[derive(Clone, Debug)]
pub struct Grudge { pub attitude: f32, tolerance: f32, friend: bool, g: f32, pub level: i8, start: f32, pub cooled: bool,
    /// the rivalry score (pair record +0x1c): vendettas +1/+2/+3 by level, shoving the player +1,
    /// knocking the player down +2; over 2 after the race, this rider has words with the player
    pub score: u32,
    /// the attitude at the start of the race, and the player's bumps, shoves and knockdowns on it
    base: f32, bumps: u32, shoves: u32, knocks: u32, rel: u8 }
impl Grudge {
    pub fn new(me: usize, player: usize) -> Self {
        Self { attitude: ATTITUDE[me.min(11)][player.min(11)] as f32, tolerance: TOLERANCE[me.min(11)][player.min(11)] as f32,
               friend: relation(me, player) == 1, g: 0.0, level: -1, start: 0.0, cooled: false, score: 0,
               base: ATTITUDE[me.min(11)][player.min(11)] as f32, bumps: 0, shoves: 0, knocks: 0, rel: relation(me, player) }
    }
    /// Starting from how it felt about the player after the last heat (`Circuit_BuildHeatRelations`):
    /// that feeling, worn down between heats (a friend by 3, others by 2, a foe by 1), never below
    /// the table's.
    pub fn carried(me: usize, player: usize, last: Option<f32>) -> Self {
        let mut g = Self::new(me, player);
        if let Some(a) = last {
            let table = g.attitude;
            let decay = match g.rel { 1 => 3.0, 2 => 1.0, _ => 2.0 };
            g.attitude = (a - decay).max(table).max(0.0);
            g.base = g.attitude;
        }
        g
    }
    /// After the race (`Rider_SettleAttitudeAfterRace`): with no contact back to the start; else
    /// the gain is kept by (bumps + 2 shoves + 3 knockdowns) / (3 n).
    pub fn settled(&self) -> f32 {
        let n = self.bumps + self.shoves + self.knocks;
        if n == 0 { return self.base; }
        self.base + (self.bumps + 2 * self.shoves + 3 * self.knocks) as f32 / (3 * n) as f32 * (self.attitude - self.base)
    }
    /// The player put this rider down (30), shoved it off balance (15) or bumped it off balance (9).
    pub fn hit(&mut self, amount: f32, now: f32) {
        if amount >= 30.0 { self.knocks += 1 } else if amount >= 15.0 { self.shoves += 1 } else { self.bumps += 1 }
        self.g = (self.g + amount).min(self.tolerance);
        if self.g >= self.tolerance {
            let a = self.attitude;
            let lvl = if a >= 80.98926 { 2 } else if a >= 60.0 { 1 } else { 0 };
            self.score += lvl as u32 + 1;
            self.level = if self.friend { 0 } else { lvl };
            self.attitude = (a + 15.0).min(100.0);
            self.g = 0.0;
            self.start = now;
            self.cooled = false;
        }
    }
    /// It got its own back: a knockdown or shove-stumble on the player cools it down.
    pub fn got_even(&mut self, knocked: bool) {
        self.score += if knocked { 2 } else { 1 };
        if self.level < 0 { return; }
        self.g = (self.g - if knocked { 45f32.max(self.g / 2.0) } else { 45.0 }).max(0.0);
    }
    /// A level-1 grudge cools after about six seconds.
    pub fn tick(&mut self, now: f32) {
        if self.level == 1 && !self.cooled && now - self.start > 360.45 / 60.0 { self.cooled = true; self.g = (self.g - 15.0).max(0.0); }
    }
    /// Chasing the player (AIComputer_IsHostileTarget): a level-2 grudge, or a level-1 one not yet cooled.
    pub fn hostile(&self) -> bool { self.level == 2 || (self.level == 1 && !self.cooled) }
    /// Will shove the player when alongside (AIComputer_ShouldShove): level 1 or 2.
    pub fn shoves(&self) -> bool { self.level >= 1 }
}

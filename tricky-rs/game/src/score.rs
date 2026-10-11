//! The score object's arithmetic exactly as the original does it (board row G2a): the rider's score object
//! (boarder+0x5820) adds "units" for spins, flips, grabs and rails and turns them into points. Each function here
//! is one of the game's, with its constants and order of operations, checked bit for bit against the original in
//! the function runner (`tests/score.rs`). Notes: `docs/port-notes/scoring.md`.

/// The score object's fields these functions use, by their offset in the game's object.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    /// +0xc: nonzero clears +0xf0 every tick
    pub x0c: u32,
    /// +0x24 the trick's units, +0x28 its multiplier, +0x2c the grab's rate
    pub units: f32,
    pub mult: f32,
    pub grab_rate: f32,
    /// +0x30, +0x34: the spin and flip so far, degrees
    pub spin: i32,
    pub flip: i32,
    /// +0x38 ticks of grab, +0x3c ticks of rail, +0x40 and +0x44 two more tick counters (−1 when off)
    pub grab_ticks: i32,
    pub rail_ticks: i32,
    pub x40: i32,
    pub x44: i32,
    /// +0x48, +0x4c: the most spin and flip scored, degrees
    pub spin_scored: i32,
    pub flip_scored: i32,
    /// +0x60..+0x6c: the grabs done this jump (nonzero = done)
    pub grabs: [u32; 4],
    /// +0xf0
    pub xf0: u32,
    /// +0x15c: twelve timed entries of five words: [0] live, [2] lifetime, [3] age (seconds)
    pub timed: [(u32, f32, f32); 12],
    /// +0x10 switch (the chain began riding switch), +0x14, +0x18 (no repeat check), +0x1c (the chain had points
    /// when it began), +0x20 the chain's kind (0 none; set from `Score_StartTrickChain`'s second argument and
    /// `Score_LandTrick`'s rail flag; 4 mirrors the flip)
    pub x10: u32,
    pub x14: u32,
    pub x18: u32,
    pub x1c: u32,
    pub x20: u32,
    /// +0x50, +0x54 (the largest kept at +0xd4), +0x58, +0x5c: per-trick extras summed into +0xc8, +0x7c, +0x88
    pub x50: f32,
    pub x54: f32,
    pub x58: i32,
    pub x5c: i32,
    /// totals: +0x74 spin and +0x78 flip degrees, +0x7c, +0x80 rails done, +0x84 tricks scored, +0x88, +0x8c banks
    pub x74: i32,
    pub x78: i32,
    pub x7c: i32,
    pub x80: i32,
    pub x84: i32,
    pub x88: i32,
    pub x8c: i32,
    /// +0xc4 air time, +0xc8, +0xcc rail time, +0xd0 longest air, +0xd4, +0xd8 longest rail (seconds)
    pub xc4: f32,
    pub xc8: f32,
    pub xcc: f32,
    pub xd0: f32,
    pub xd4: f32,
    pub xd8: f32,
    /// +0xe4 the best trick's points and +0xe8 its id
    pub best_points: i32,
    pub best: [u32; 2],
    /// +0xf4 tricks in the chain (a bonus from two)
    pub chain: i32,
    /// +0xf8: tricks scored per rank (`Score_GetRankForPoints`)
    pub ranks: [i32; 6],
    /// +0x130: the last five trick ids, +0x158 where the next goes
    pub recent: [[u32; 2]; 5],
    pub recent_at: i32,
    /// the rider's +0x418 (through +0x24c): negative for a player
    pub rider_418: i32,
}

/// `Score_CalcPoints` 0x155440: round10(units · multiplier · 0.6786545 · 10000), never below 0.
pub fn calc_points(units: f32, mult: f32) -> i32 {
    let f = mult * units * 0.678_654_5;
    if f < 0.0 {
        return 0;
    }
    let i = (f * 10000.0 + 5.0) as i32;
    i - i % 10
}

/// `Score_CalcBasePoints` 0x1554c0: as [`calc_points`] without the multiplier.
pub fn calc_base_points(units: f32) -> i32 {
    let f = units * 0.678_654_5;
    if f < 0.0 {
        return 0;
    }
    let i = (f * 10000.0 + 5.0) as i32;
    i - i % 10
}

/// `Score_GetMultiplier` 0x155410: units · 0.6786545, never below 0 (the meter's share of a trick; the name is
/// the one in use, it returns no multiplier).
pub fn get_multiplier(units: f32) -> f32 {
    let f = units * 0.678_654_5;
    if f < 0.0 {
        0.0
    } else {
        f
    }
}

/// `Score_RoundSpinTo180` 0x157198: degrees to the nearest 180 (halves toward zero), C's `%` semantics.
pub fn round_spin_to_180(deg: i32) -> i32 {
    let m = deg.wrapping_rem(180);
    if m < -90 {
        deg.wrapping_sub(180).wrapping_sub(m)
    } else if m < 91 {
        deg.wrapping_sub(m)
    } else {
        deg.wrapping_add(180).wrapping_sub(m)
    }
}

impl Score {
    /// `Score_UpdateSpin` 0x155e00: each new half turn of spin scores 0.06995432.
    pub fn update_spin(&mut self, deg: i32) {
        self.spin = deg;
        let r = round_spin_to_180(deg.wrapping_abs());
        if self.spin_scored < r {
            self.spin_scored = r;
            self.units += f32::from_bits(0x3d8f_4436);
        }
    }

    /// `Score_UpdateFlip` 0x155e68: each new whole flip scores 0.2499898.
    pub fn update_flip(&mut self, deg: i32) {
        self.flip = deg;
        let r = round_spin_to_180(deg.wrapping_abs());
        if self.flip_scored < r && r % 360 == 0 {
            self.flip_scored = r;
            self.units += f32::from_bits(0x3e7f_fd53);
        }
    }

    /// `Score_StartGrab` 0x155ee8: a grab scores 0.04249751 × (grabs already done, up to three, + 1), starts the
    /// hold count and sets the hold rate by the grab's clip (`clip`); clips not in the game's table keep the rate.
    pub fn start_grab(&mut self, clip: u32) {
        let mut n = 0;
        if self.grabs[0] != 0 {
            n = 1;
            while n < 3 && self.grabs[n] != 0 {
                n += 1;
            }
        }
        self.grab_ticks = 0;
        self.units += (n + 1) as f32 * f32::from_bits(0x3d2e_11de);
        if let Some(rate) = grab_rate(clip) {
            self.grab_rate = rate;
        }
    }

    /// `Score_Tick` 0x156e80, once a tick: holding a grab scores rate·0.00075006403, grinding f(speed)·0.0025006225
    /// (f = v/(3200 − 3v) between 50 and 800 cm/s, 0 below, 1 above; `speed` the rider's |v|, cm/s); the timed
    /// entries age by a tick and die at their lifetime.
    pub fn tick(&mut self, speed: f32) {
        if self.x44 >= 0 {
            self.x44 = self.x44.wrapping_add(1);
        }
        if self.grab_ticks >= 0 {
            self.grab_ticks = self.grab_ticks.wrapping_add(1);
            self.units += self.grab_rate * f32::from_bits(0x3a44_9ff2);
        }
        if self.rail_ticks >= 0 {
            self.rail_ticks = self.rail_ticks.wrapping_add(1);
            let mut v = speed;
            let mut f = 1.0;
            if v < 800.0 {
                if v < 50.0 {
                    v = 0.0;
                }
                f = v / (3200.0 - v * 3.0);
            }
            self.units += f * f32::from_bits(0x3b23_e17c);
        }
        if self.x40 > 0 {
            self.x40 = self.x40.wrapping_add(1);
        }
        for e in self.timed.iter_mut() {
            if e.0 != 0 && 0.0 < e.1 {
                e.2 += 0.016_666_668;
                if e.1 <= e.2 {
                    e.0 = 0;
                }
            }
        }
        if self.x0c != 0 {
            self.xf0 = 0;
        }
    }
}

/// The hold rate `Score_StartGrab` 0x155ee8 gives each grab clip.
pub fn grab_rate(clip: u32) -> Option<f32> {
    Some(match clip {
        0x278 | 0x279 | 0x27a | 0x27d | 0x281 | 0x287 | 0x289 | 0x28a | 0x28b | 0x28d | 0x28f | 0x298 | 0x29c | 0x29d | 0x2a6 => 2.0,
        0x27b | 0x291 | 0x296 | 0x2a8 => 2.5,
        0x27c | 0x28e => 1.25,
        0x27e | 0x282 | 0x283 | 0x28c => 1.0,
        0x27f | 0x280 | 0x285 | 0x286 | 0x288 => 1.5,
        0x284 | 0x290 => 1.75,
        0x292 | 0x293 | 0x294 | 0x297 | 0x29b | 0x2a1 | 0x2a3 | 0x2a4 | 0x2a5 | 0x2a7 | 0x2a9 => 4.0,
        0x295 | 0x2ab => 5.0,
        0x299 | 0x29a | 0x29f | 0x2a0 | 0x2a2 => 3.0,
        0x29e | 0x2aa => 3.5,
        0x2ac..=0x2cf => 18.0,
        0x2d0..=0x2db => 20.0,
        _ => return None,
    })
}

/// The trick name table's index by spin (column: half turns, +10 the other way) and flip (row: whole turns, +3
/// backwards) at 0x320cf0; the second (0x320f40) for a chain begun with +0x14 set (`Score_BuildTrickId`'s fifth
/// argument).
const COMBO: [[u8; 7]; 21] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 6, 16, 0, 25, 35],
    [0, 0, 0, 17, 0, 0, 36],
    [0, 0, 7, 18, 0, 26, 37],
    [0, 0, 8, 19, 0, 27, 43],
    [0, 0, 9, 0, 0, 28, 0],
    [0, 0, 10, 0, 0, 29, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 20, 0, 0, 39],
    [0, 0, 11, 21, 0, 30, 40],
    [0, 0, 0, 22, 0, 0, 41],
    [0, 0, 12, 23, 0, 31, 42],
    [0, 0, 13, 24, 0, 32, 43],
    [0, 0, 14, 0, 0, 33, 0],
    [0, 0, 15, 0, 0, 34, 0],
];
const COMBO_CHAIN: [[u8; 7]; 21] = [
    [0, 2, 0, 0, 2, 0, 0],
    [0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 5, 6, 16, 0, 25, 35],
    [0, 0, 0, 17, 0, 0, 36],
    [0, 0, 7, 18, 0, 26, 37],
    [0, 0, 8, 19, 0, 27, 43],
    [0, 0, 9, 0, 0, 28, 0],
    [0, 0, 10, 0, 0, 29, 0],
    [0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 3, 0, 0, 0, 0, 0],
    [0, 0, 0, 20, 0, 0, 39],
    [0, 0, 11, 21, 0, 30, 40],
    [0, 0, 0, 22, 0, 0, 41],
    [0, 0, 12, 23, 0, 31, 42],
    [0, 0, 13, 24, 0, 32, 43],
    [0, 0, 14, 0, 0, 33, 0],
    [0, 0, 15, 0, 0, 34, 0],
];

/// The calls the trick functions make, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreCall {
    /// `Score_AnnounceTrick` 0x157088(id, a repeat)
    Announce([u32; 2], u32),
    /// `Score_TrickBookCheck` 0x1579e0(id)
    TrickBook([u32; 2]),
    /// `Scoring_PushMessage` 0x157330(seconds as bits, kind, value, extra)
    Message(u32, u32, i32, i32),
    /// `Score_StartTrickChain` for the focus rider: the audio object's +0x6e04 = 1, +0x6e00 = 0
    Focus,
}

/// `Score_GetRankForPoints` 0x19e7a0: the highest rank whose threshold the points reach (0, 1001, 2501, 4001, 7501,
/// 11501 at 0x338120 + 20·rank), 0 below them all.
pub fn rank_for_points(points: i32) -> i32 {
    const AT: [f32; 6] = [0.0, 1001.0, 2501.0, 4001.0, 7501.0, 11501.0];
    for r in (0..6).rev() {
        if AT[r] <= points as f32 {
            return r as i32;
        }
    }
    0
}

/// 0x1568b0: the chain bonus for n tricks in a chain: none for 0 or 1, then 4,000 a trick up to 16,000 (n is
/// taken unsigned: a negative count gets 16,000).
pub fn chain_bonus(n: i32) -> i32 {
    match n as u32 {
        0 | 1 => 0,
        2 => 4000,
        3 => 8000,
        4 => 12000,
        _ => 16000,
    }
}

/// `Score_AirTimePoints` 0x156968: 1,000 a second of air from four seconds (whole seconds past four, plus one).
pub fn air_time_points(seconds: f32) -> i32 {
    if 4.0 <= seconds {
        (((seconds - 4.0) + 1.0) as i32).wrapping_mul(1000)
    } else {
        0
    }
}

fn byte(w: u32, k: u32) -> u32 {
    (w >> (8 * k)) & 0xff
}

impl Score {
    /// `Scoring_ResetCombo` 0x157008: the trick in progress cleared (and the timed entries of kind 12).
    pub fn reset_combo(&mut self) {
        self.spin = 0;
        self.flip = 0;
        self.spin_scored = 0;
        self.flip_scored = 0;
        self.units = 0.0;
        self.x40 = 0;
        self.x20 = 0;
        self.x50 = 0.0;
        self.x54 = 0.0;
        self.x58 = 0;
        self.x5c = 0;
        self.x44 = -1;
        self.mult = 1.0;
        self.grab_ticks = -1;
        self.rail_ticks = -1;
        self.grabs[0] = 0;
        self.grabs[1] = 0;
        self.grabs[2] = 0;
        for e in self.timed.iter_mut() {
            if e.0 == 0xc {
                e.0 = 0;
            }
        }
    }

    /// `Score_CountRecentRepeats` 0x157268: how many of the last five tricks were this one (three-grab tricks and
    /// the off-the-table trick never count); the trick goes into the list.
    pub fn count_recent_repeats(&mut self, id: [u32; 2]) -> i32 {
        let mut n = 0;
        for r in self.recent {
            if r == id && byte(id[0], 2) != 0x65 && byte(id[0], 0) != 0x2c {
                n += 1;
            }
        }
        self.recent[self.recent_at as usize] = id;
        self.recent_at = (self.recent_at + 1) % 5;
        n
    }

    /// `Score_BuildTrickId` 0x157490: the landed trick's id (two words: the name table's index in byte 0, the grabs
    /// in bytes 1 and 2, spin and flip counts and directions, the chain's kind) and whether it is a trick at all
    /// (a spin, a flip or a grab). `rail`, `a`, `b` are its third to fifth arguments. Rounds the score's spin and
    /// flip to half turns first (the spin mirrored for a switch chain, the flip for kind 4).
    pub fn build_trick_id(&mut self, rail: u32, a: u32, b: u32) -> ([u32; 2], bool) {
        let mut id = [0u32; 2];
        if self.x10 != 0 {
            self.spin = self.spin.wrapping_neg();
        }
        if self.x20 == 4 {
            self.flip = self.flip.wrapping_neg();
        }
        self.spin = round_spin_to_180(self.spin);
        self.flip = round_spin_to_180(self.flip);
        let flip_raw = self.flip;
        let flips = (flip_raw / 360).wrapping_abs();
        let spins = (self.spin / 180).wrapping_abs();
        let mut combos = 0;
        if flips >= 1 && spins >= 3 {
            let (mut f, mut sp) = (flips, spins);
            loop {
                f -= 1;
                sp -= 3;
                combos += 1;
                if f < 1 || sp <= 2 {
                    break;
                }
            }
        }
        let col = if 0 < self.spin || 10 < spins { spins + 10 } else { spins };
        let row = if flip_raw < 0 || 3 < flips { flips + 3 } else { flips };
        let mut grabs = 0;
        if self.grabs[0] != 0 {
            grabs = 1;
            while grabs < 3 && self.grabs[grabs] != 0 {
                grabs += 1;
            }
        }
        if self.x20 == 0 {
            if self.x18 != 0 {
                id[1] = id[1] & 0xfff0_ffff | 0x40000;
            } else if self.x10 != 0 {
                id[1] = id[1] & 0xfff0_ffff | 0x10000;
            }
        } else {
            id[1] = id[1] & 0xfff0_ffff | if self.x10 == 0 { 0x20000 } else { 0x30000 };
        }
        let mut over = false;
        if col < 0x15 && row < 7 {
            let t = if self.x14 != 0 && b != 0 { &COMBO_CHAIN } else { &COMBO };
            id[0] = id[0] & !0xff | t[col as usize][row as usize] as u32;
        } else {
            over = true;
        }
        let mut digits = true;
        if byte(id[0], 0) == 0 {
            if 0 < spins {
                id[1] = id[1] & 0xc7ff_ffff | if self.spin < 1 { 0x0800_0000 } else { 0x1000_0000 };
            }
            if flips == 0 {
                id[0] = id[0] & 0xf0ff_ffff | ((spins as u32) & 0xf) << 24;
            }
            id[1] = id[1] & 0xf8ff_ffff | ((flips as u32) & 7) << 24;
            if 0 < flips {
                let kind = if combos < 1 || combos != flips {
                    if self.flip < 1 { 0x200 } else { 0x100 }
                } else if self.flip < 1 {
                    0x300
                } else {
                    0x400
                };
                id[1] = id[1] & 0xffff_f0ff | kind;
            } else {
                digits = false;
            }
        }
        if digits && 0 < flips && spins != flips * 3 && spins < 0xb {
            id[0] = id[0] & 0x0fff_ffff | (spins as u32) << 28;
        }
        let mut kind = None;
        if rail != 0 {
            kind = Some(0x30_0000);
        } else if byte(id[0], 0) == 0 && (combos == 0 || id[0] & 0xf000_0000 != 0) {
            if a != 0 && self.x10 == 0 {
                kind = Some(0x20_0000);
            } else if spins == 0 && flips == 0 && grabs > 0 {
                kind = Some(0x10_0000);
            }
        }
        if let Some(k) = kind {
            id[1] = id[1] & 0xff0f_ffff | k;
        }
        if grabs > 0 {
            id[0] = id[0] & !0xff00 | (self.grabs[0] & 0xff) << 8;
            if grabs > 1 {
                id[1] = id[1] & 0x3fff_ffff | 0x4000_0000;
                let g = if grabs == 2 { self.grabs[1] & 0xff } else { 0x65 };
                id[0] = id[0] & !0xff_0000 | g << 16;
            }
        }
        if over {
            id = [0x2c, 0];
        }
        (id, 0 < flips || 0 < spins || 0 < grabs)
    }

    /// `Score_LandTrick` 0x1569b0(rail, a, b): the trick landed. A player (rider+0x418 < 0) scores it: points over
    /// the repeats, the chain bonus from two tricks in a chain (not into a rail), air time past four seconds, the
    /// per-rank count and the best trick; then the combo is reset. Returns the meter's share over the repeats
    /// (0.0 for an AI rider). `rail` lands into a rail (its tick count kept).
    pub fn land_trick(&mut self, rail: u32, a: u32, b: u32) -> (f32, Vec<ScoreCall>) {
        let mut calls = Vec::new();
        let mut repeats = 0;
        let (id, any) = self.build_trick_id(rail, a, b);
        if any {
            self.chain = self.chain.wrapping_add(1);
            if self.x20 != 0 {
                self.units += f32::from_bits(0x3e4c_c6d7);
            }
            if rail != 0 {
                self.units += f32::from_bits(0x3e38_54cc);
            }
            if self.x18 == 0 {
                repeats = self.count_recent_repeats(id);
            }
            calls.push(ScoreCall::Announce(id, (0 < repeats) as u32));
            calls.push(ScoreCall::TrickBook(id));
        }
        if 0 <= self.rider_418 {
            self.reset_combo();
            self.mult = 1.0;
            return (0.0, calls);
        }
        let n = repeats + 1;
        let share = get_multiplier(self.units) / n as f32;
        self.x74 = self.x74.wrapping_add(self.spin.wrapping_abs());
        self.x78 = self.x78.wrapping_add(self.flip.wrapping_abs());
        self.xc8 += self.x50;
        self.x7c = self.x7c.wrapping_add(self.x58);
        self.x88 = self.x88.wrapping_add(self.x5c);
        self.xd4 = if self.x54 < self.xd4 { self.xd4 } else { self.x54 };
        let mut pts = calc_points(self.units, self.mult) / n;
        if id[1] & 0xf0_0000 != 0x30_0000 {
            if self.chain < 2 {
                self.chain = 0;
            } else {
                pts = pts.wrapping_add(chain_bonus(self.chain));
                calls.push(ScoreCall::Message(0x400c_cccd, 0xf, chain_bonus(self.chain), 0));
                self.chain = 0;
            }
        }
        let air = self.x44 as f32 * f32::from_bits(0x3c88_8889);
        if 0.0 < air {
            self.xc4 += air;
            if self.xd0 < air {
                self.xd0 = air;
            }
            let p = air_time_points(air);
            if 0 < p {
                pts = pts.wrapping_add(p);
                calls.push(ScoreCall::Message(0x400c_cccd, 0x10, p, 0));
            }
        }
        if 0 < pts {
            let base = calc_base_points(self.units);
            calls.push(ScoreCall::Message(0x4020_0000, if 0 < repeats { 3 } else { 2 }, pts, base));
            self.xf0 = self.xf0.wrapping_add(pts as u32);
            let r = rank_for_points(pts);
            if 0 <= r {
                self.ranks[r as usize] = self.ranks[r as usize].wrapping_add(1);
            }
            self.x84 = self.x84.wrapping_add(1);
        }
        if any && self.best_points < pts {
            self.best_points = pts;
            self.best = id;
        }
        if rail != 0 {
            let keep = self.rail_ticks;
            self.reset_combo();
            self.rail_ticks = keep.wrapping_add(1);
        } else {
            self.reset_combo();
        }
        self.x20 = rail;
        self.mult = 1.0;
        (share, calls)
    }

    /// `Score_StartTrickChain` 0x156618(switch, kind, a, b): a new chain begins. One in progress (+0x20) is first
    /// scored as a rail trick (its spin in half turns and its kind in the id, its time) and reset, the multiplier
    /// then 1.3 for a switch start. Returns the old chain's meter share (0.0 if none). `focus` is what
    /// `Score_IsFocusPlayer` answers.
    pub fn start_trick_chain(&mut self, switch: u32, kind: u32, a: u32, b: u32, focus: bool) -> (f32, Vec<ScoreCall>) {
        let mut calls = Vec::new();
        if focus {
            calls.push(ScoreCall::Focus);
        }
        let mut share = 0.0;
        if self.x20 != 0 {
            self.spin = round_spin_to_180(self.spin);
            let halves = (self.spin / 180).wrapping_abs();
            let t = self.rail_ticks as f32 * f32::from_bits(0x3c88_8889);
            self.x80 = self.x80.wrapping_add(1);
            self.xcc += t;
            if self.xd8 < t {
                self.xd8 = t;
            }
            if 1.0 < t {
                self.chain = self.chain.wrapping_add(1);
            }
            let id = [0, (kind & 0xf) << 12 | ((halves as u32) & 0xf) << 4];
            calls.push(ScoreCall::Announce(id, 0));
            calls.push(ScoreCall::TrickBook(id));
            share = get_multiplier(self.units);
            let pts = calc_points(self.units, self.mult);
            if 0 < pts {
                calls.push(ScoreCall::Message(0x4020_0000, 2, pts, calc_base_points(self.units)));
                self.xf0 = self.xf0.wrapping_add(pts as u32);
                if self.best_points < pts {
                    self.best_points = pts;
                    self.best = id;
                }
            }
            self.reset_combo();
            self.mult = if switch != 0 { f32::from_bits(0x3fa6_6666) } else { 1.0 };
        }
        if self.x44 < 0 {
            self.x44 = 0;
        }
        self.x10 = switch;
        self.x14 = a;
        self.x20 = kind;
        self.x18 = b;
        self.x1c = (b != 0 && 0 < calc_points(self.units, self.mult)) as u32;
        (share, calls)
    }

    /// `Score_BankPoints` 0x155aa8: the chain's points banked (a message of kind 1), the combo reset, the chain
    /// count cleared, one more bank. Returns −0.12 (0xbdf5b343).
    pub fn bank_points(&mut self) -> (f32, Vec<ScoreCall>) {
        let mut calls = Vec::new();
        let pts = calc_points(self.units, self.mult);
        if 0 < pts {
            calls.push(ScoreCall::Message(0x3fc0_0000, 1, pts, 0));
        }
        self.reset_combo();
        self.chain = 0;
        self.x8c = self.x8c.wrapping_add(1);
        (f32::from_bits(0xbdf5_b343), calls)
    }
}

/// The grab id `Scoring_OnTrickEvent` records for a grab clip (0x278..0x2db, the jump table at 0x370fc0; 0 for
/// any other clip) and whether it is an uber (clips 0x2ac.., grabs 0x35..0x64).
pub fn grab_of_clip(clip: u32) -> (u32, bool) {
    let k = clip.wrapping_sub(0x278);
    let id = match k {
        0..=3 => k + 1,
        4..=19 => k + 2,
        0x14 => 0x17,
        0x15 => 0x18,
        0x16 => 0x16,
        0x17 => 0x19,
        0x18 => 0x1a,
        0x19 => 5,
        0x1a..=0x1d => k + 1,
        0x1e..=0x2d => k + 2,
        0x2e => 0x31,
        0x2f => 0x32,
        0x30 => 0x30,
        0x31 => 0x33,
        0x32 => 0x34,
        0x33 => 0x1f,
        0x34..=0x63 => k + 1,
        _ => 0,
    };
    (id, (0x34..0x64).contains(&k))
}

impl Score {
    /// 0x1571e0: the grab goes into the jump's list (+0x60, three slots) unless it is the grab already last in it
    /// (a grab held again); a full list is left alone.
    pub fn add_grab(&mut self, id: u32) {
        if self.grabs[0] == 0 {
            self.grabs[0] = id;
            return;
        }
        let mut i = 0;
        loop {
            if self.grabs[i] == id && (i == 2 || self.grabs[i + 1] == 0) {
                return;
            }
            if 2 < i + 1 {
                return;
            }
            i += 1;
            if self.grabs[i] == 0 {
                self.grabs[i] = id;
                return;
            }
        }
    }

    /// `Scoring_OnTrickEvent` 0x156148(seconds, clip): a grab let go (nothing unless one is held, +0x38 ≥ 0). The
    /// grab goes into the list, one more uber (+0x5c; the rider's +0x58a8 that makes TRICKY from six) or grab
    /// (+0x58), its hold time summed (+0x50) and the longest kept (+0x54), the hold count off, and units for the
    /// clip's `seconds` × 0.04500384 × the grab's rate.
    pub fn end_grab(&mut self, seconds: f32, clip: u32) {
        if self.grab_ticks < 0 {
            return;
        }
        let (id, uber) = grab_of_clip(clip);
        self.add_grab(id);
        if uber {
            self.x5c = self.x5c.wrapping_add(1);
        } else {
            self.x58 = self.x58.wrapping_add(1);
        }
        let held = self.grab_ticks as f32 * f32::from_bits(0x3c88_8889);
        self.x50 += held;
        self.x54 = if held < self.x54 { self.x54 } else { held };
        self.grab_ticks = -1;
        self.units += (seconds * f32::from_bits(0x3d38_55f2)) * self.grab_rate;
    }
}

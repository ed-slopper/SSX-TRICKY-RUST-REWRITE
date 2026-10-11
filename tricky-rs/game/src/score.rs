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

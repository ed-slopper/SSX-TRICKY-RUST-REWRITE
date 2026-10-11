//! The pickups' effects as the game applies them (board row E9c): what the trigger script's pickup ops (6 points,
//! 14 multiplier, 15 meter, 16 time, 17 speed boost, 18 spin boost) call. Points, multiplier, meter and time count
//! only in show-off (modes 3 and 5, [`crate::courses::Mode`]). Each function is the game's, checked in the function
//! runner (`tests/pickup.rs`).

use crate::score::{Score, ScoreCall};

/// What the pickup functions call besides the score object, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupCall {
    /// `Scoring_PushMessage` 0x157330 (as the score functions report it)
    Score(ScoreCall),
    /// `Boarder_AddMeter` 0x11b018(amount, rider)
    AddMeter(f32),
    /// `Sfx_PickupMultiplier` 0x219d70 (multiplier, the audio system 0x343050)
    SfxMultiplier(f32),
    /// `Sfx_PickupTime` 0x219fc0 (seconds, the audio system)
    SfxTime(f32),
    /// the race clock's limit (gApp+0x730 → +0x1c → +0x14) += seconds
    ClockAdd(f32),
}

fn showoff(mode: u32) -> bool {
    mode == 3 || mode == 5
}

/// The game's `(int)` of a float (`cvt.w.s`: toward zero, out of range 0x7fffffff / 0x80000000).
fn trunc(v: f32) -> i32 {
    if v.is_nan() {
        i32::MAX
    } else {
        v as i32
    }
}

impl Score {
    /// `Score_AddPickupPoints` 0x155b10: the points to +0x94, a 2.5 s message of kind 6, the 500 / 2000 / 5000
    /// counters; returns the meter it earns (0.04).
    pub fn add_pickup_points(&mut self, points: i32) -> (f32, ScoreCall) {
        self.pickup_points = self.pickup_points.wrapping_add(points);
        let msg = ScoreCall::Message(0x4020_0000, 6, points, 0);
        for (k, v) in [500, 2000, 5000].iter().enumerate() {
            if points == *v {
                self.pickups[k] = self.pickups[k].wrapping_add(1);
            }
        }
        (f32::from_bits(0x3d23_e372), msg)
    }

    /// `Score_AddPickupTime` 0x155bb8: a 2.5 s message of kind 9 with the whole seconds, the 1 / 5 / 10 s counters.
    pub fn add_pickup_time(&mut self, seconds: f32) -> ScoreCall {
        let s = trunc(seconds);
        for (k, v) in [1, 5, 10].iter().enumerate() {
            if s == *v {
                self.pickups[3 + k] = self.pickups[3 + k].wrapping_add(1);
            }
        }
        ScoreCall::Message(0x4020_0000, 9, s, 0)
    }

    /// `Score_SetPickupMultiplier` 0x155cd0: the x2 / x3 / x5 counters; a higher multiplier than the trick's
    /// (+0x28) replaces it with a message of kind 12 (its time −1: until replaced).
    pub fn set_pickup_multiplier(&mut self, mult: f32) -> Option<ScoreCall> {
        let m = trunc(mult);
        for (k, v) in [2, 3, 5].iter().enumerate() {
            if m == *v {
                self.pickups[6 + k] = self.pickups[6 + k].wrapping_add(1);
            }
        }
        if self.mult < mult {
            self.mult = mult;
            Some(ScoreCall::Message(0xbf80_0000, 12, m, 0))
        } else {
            None
        }
    }
}

/// The rider's fields the pickups touch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PickupRider {
    /// +0x134 speed boost time left, +0x138 spin boost time left, +0x13c spin boost on
    pub speed_boost: f32,
    pub spin_boost: f32,
    pub spin_boost_on: u32,
    /// +0x41c: 1 for the rider whose pickups are heard
    pub x41c: i32,
    /// +0x424: the multiplier counts when 1 or 3
    pub x424: i32,
}

/// `Pickup_Points` 0x11e608 (op 6): in show-off, the points ([`Score::add_pickup_points`]) and the meter they earn.
pub fn points(mode: u32, score: &mut Score, points: i32) -> Vec<PickupCall> {
    if !showoff(mode) {
        return vec![];
    }
    let (meter, msg) = score.add_pickup_points(points);
    vec![PickupCall::Score(msg), PickupCall::AddMeter(meter)]
}

/// `Pickup_Multiplier` 0x11e660 (op 14): in show-off, for a rider whose +0x424 is 1 or 3, the multiplier
/// ([`Score::set_pickup_multiplier`]); the sound for the rider whose +0x41c is 1, in any mode.
pub fn multiplier(mode: u32, r: &PickupRider, score: &mut Score, mult: f32) -> Vec<PickupCall> {
    let mut calls = vec![];
    if showoff(mode) && (r.x424 == 1 || r.x424 == 3) {
        calls.extend(score.set_pickup_multiplier(mult).map(PickupCall::Score));
    }
    if r.x41c == 1 {
        calls.push(PickupCall::SfxMultiplier(mult));
    }
    calls
}

/// `Pickup_Meter` 0x11e958 (op 15): in show-off, `Boarder_AddMeter`.
pub fn meter(mode: u32, amount: f32) -> Vec<PickupCall> {
    if showoff(mode) {
        vec![PickupCall::AddMeter(amount)]
    } else {
        vec![]
    }
}

/// `Pickup_Time` 0x11e8a0 (op 16): in show-off, the message and counters ([`Score::add_pickup_time`]) and the
/// seconds onto the race clock's limit; the sound in any mode.
pub fn time(mode: u32, score: &mut Score, seconds: f32) -> Vec<PickupCall> {
    let mut calls = vec![];
    if showoff(mode) {
        calls.push(PickupCall::Score(score.add_pickup_time(seconds)));
        calls.push(PickupCall::ClockAdd(seconds));
    }
    calls.push(PickupCall::SfxTime(seconds));
    calls
}

/// `Boarder_AddSpeedBoostTime` 0x11e910 (op 17): the boost's time left becomes the larger.
pub fn speed_boost(r: &mut PickupRider, seconds: f32) {
    r.speed_boost = if r.speed_boost <= seconds { seconds } else { r.speed_boost };
}

/// `Boarder_AddSpinBoostTime` 0x11e930 (op 18): the same for the spin boost, and it turns on.
pub fn spin_boost(r: &mut PickupRider, seconds: f32) {
    r.spin_boost = if r.spin_boost <= seconds { seconds } else { r.spin_boost };
    r.spin_boost_on = 1;
}

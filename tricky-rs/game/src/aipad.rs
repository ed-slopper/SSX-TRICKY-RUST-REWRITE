//! The computer rider's pad word (board row G4l): `cComputer::GetControls` 0x1384b8 fills it each tick by the
//! rider's control state from the AI's control routines (`aictl`, `aicomp`), `AIComputer_ControlsRide` 0x1387e8 (the
//! ride's steer, crouch and brake fields, braking and tucking for an event-25 marker ahead),
//! `AIComputer_CrossedEvent1b` 0x139210, and `FUN_001371e0` (the steer toward a point across the line). In the
//! game's units and the vector unit's order; checked in the function runner (`tests/aipad.rs`).

use crate::ps2f::F;
use crate::rigidbody::{dot, ld, scale, sub};

type V4 = [f32; 4];

/// The stick's six-bit field, `(value × 31) as int & 0x3f`
fn stick(v: f32) -> u32 {
    ((F(v) * F(31.0)).0 as i32 as u32) & 0x3f
}

/// The rider's path query (its +0x57a4 object's virtual +0xc with two floats: the rider's +0x340 and its distance
/// +0x344 plus a look-ahead): the events found (their first words) and the flags it gives back
pub type Query<'a> = &'a mut dyn FnMut(f32, f32) -> (Vec<u32>, u32);

/// `AIComputer_ControlsRide` 0x1387e8(pad): the path's events 50 ahead of the rider (+0x340, +0x344): the first event 25 with the flag 0x2000000 set brakes (bit 1) and, slower than the computer's speed
/// (+0x868) less 138.89 by |v|, tucks (bit 3); then the steer toward the line point (`AIComputer_SteerTowards` on
/// +0x360) into bits 17..22, the computer's crouch (+0x888) into 5..10 and brake (+0x88c) into 11..16
pub fn controls_ride(pad: &mut u32, line: f32, dist: f32, vel: V4, speed: f32, crouch: f32, brake: f32, query: Query, steer: &mut dyn FnMut() -> f32) {
    let (events, flags) = query(line, (F(dist) + F(50.0)).0);
    for e in events {
        if e == 0x19 {
            if flags & 0x0200_0000 != 0 {
                let v = ld(vel);
                if dot(v, v).sqrt().0 < (F(speed) - F(138.88889)).0 {
                    *pad |= 8;
                }
                *pad |= 2;
            }
            break;
        }
    }
    let s = steer();
    *pad = (*pad & 0xff80_001f) | stick(s) << 0x11 | stick(crouch) << 5 | stick(brake) << 0xb;
}

/// `AIComputer_CrossedEvent1b` 0x139210: an event 0x1b at the rider's own distance (no look-ahead)
pub fn crossed_event_1b(line: f32, dist: f32, query: Query) -> bool {
    let (events, _) = query(line, (F(dist) + F(0.0)).0);
    events.contains(&0x1b)
}

/// `FUN_001371e0`(point): the point with its part along the rider's frame axis (+0x4aa0) taken off, normalised (VU
/// rsqrt), along the other axis (+0x4ab0); scaled by the skill scale (+0x86c) / 7.8941293 unless it is 1
pub fn across(point: V4, axis: V4, other: V4, skill: f32) -> f32 {
    let p = ld(point);
    let a = ld(axis);
    let d = dot(p, a);
    let w = sub(p, scale(a, d));
    let q = F(1.0) / dot(w, w).sqrt();
    let n = scale(w, q);
    let mut r = dot(n, ld(other));
    if skill != 1.0 {
        r = r * (F(skill) / F(7.8941293));
    }
    r.0
}

/// The control routines `cComputer::GetControls` dispatches to (each fills the pad word)
pub trait AiPadEnv {
    /// `AI_RubberBandSpeedScale` 0x13a0f0
    fn rubber_band(&mut self) -> f32;
    /// `AIComputer_ControlsDispatch`, `_ControlsCountdown`, `_ControlsTurnAround`, `_ControlsRide`, `_ControlsAir`,
    /// `_AirTrickInput`, `_ControlsIdle`
    fn dispatch(&mut self, pad: &mut u32);
    fn countdown(&mut self, pad: &mut u32);
    fn turn_around(&mut self, pad: &mut u32);
    fn ride(&mut self, pad: &mut u32);
    fn air(&mut self, pad: &mut u32);
    fn air_trick(&mut self, pad: &mut u32);
    fn idle(&mut self, pad: &mut u32);
    /// `AIComputer_CheckStuck`
    fn check_stuck(&mut self) -> u32;
    /// `AIComputer_UpdateSkillScale`
    fn update_skill_scale(&mut self);
}

/// `cComputer::GetControls` 0x1384b8: the pad word from zero; the rider's time scale (+0x12c) toward the rubber band's
/// (`Boarder_SetTimeScaleRateLimited`); by the control state (+0x428) its routine (5: the crouch field full (0x3e0); 9
/// and 0x11: the stick's x full (0x1f); 10 nothing; 0x14 bit 0 the stuck check); the skill scale last
pub fn get_controls(env: &mut dyn AiPadEnv, control: u32, time_scale: &mut f32) -> u32 {
    let mut pad = 0u32;
    let t = env.rubber_band();
    crate::control::set_time_scale_rate_limited(t, time_scale);
    match control {
        2 | 3 | 4 | 7 | 0xb | 0xc | 0x12 => env.dispatch(&mut pad),
        5 => pad = (pad & 0xffff_f81f) | 0x3e0,
        8 => env.countdown(&mut pad),
        9 | 0x11 => pad = (pad & 0xffff_ffc0) | 0x1f,
        0xd => env.turn_around(&mut pad),
        0xe => env.ride(&mut pad),
        0xf => env.air(&mut pad),
        0x10 => env.air_trick(&mut pad),
        0x13 => env.idle(&mut pad),
        0x14 => {
            let s = env.check_stuck();
            pad = (pad & !1) | (s & 1);
        }
        _ => {}
    }
    env.update_skill_scale();
    pad
}

//! The computer rider's pad word (board row G4l): `cComputer::GetControls` 0x1384b8 fills it each tick by the
//! rider's control state from the AI's control routines (`aictl`, `aicomp`), `AIComputer_ControlsRide` 0x1387e8 (the
//! ride's steer, crouch and brake fields, braking and tucking for an event-25 marker ahead),
//! `AIComputer_CrossedEvent1b` 0x139210, `FUN_001371e0` (`AIComputer_DirNoseComponent`: the steer toward a point
//! across the line) and `AIComputer_AirTrickInput` 0x138d60 (the air trick, F4b3k2f). In the
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

/// What `AIComputer_AirTrickInput` keeps in the computer object: +0x878 the trick (the pad word's low byte; 0x1a
/// when none is planned), the flags +0x87c, +0x880 and +0x884, and the spin and flip sticks +0x888 / +0x88c
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirTrick {
    pub trick: u32,
    pub x87c: u32,
    pub x880: u32,
    pub x884: u32,
    pub spin: f32,
    pub flip: f32,
}

/// What `AIComputer_AirTrickInput` reads of the rider: +0x57b8, the spin and flip angles +0x1d0 / +0x1d4 (radians),
/// the board +0x420 (2: 360° spins), the rates its handler (+0x5ae0) keeps at +0x76c (spin) and +0x770 (flip), its
/// stats' byte +0x14 (stats at +0x464) and +0x57fc
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirTrickRider {
    pub x57b8: f32,
    pub spin_angle: f32,
    pub flip_angle: f32,
    pub board: u32,
    pub spin_rate: f32,
    pub flip_rate: f32,
    pub stat: u8,
    pub x57fc: f32,
}

/// A stick field of two bits: 3 (−1) under −0.5, 1 over 0.5, else 0
fn two_bits(v: f32) -> u32 {
    if v < -0.5 {
        3
    } else if 0.5 < v {
        1
    } else {
        0
    }
}

/// `(int)(a × 57.295776)` made positive (`cvt.w.s`, then negated when negative), the angle in whole degrees
fn whole_degrees(a: f32) -> i32 {
    let d = (F(a) * F(f32::from_bits(0x4265_2ee0))).0 as i32;
    if d < 0 {
        d.wrapping_neg()
    } else {
        d
    }
}

/// `AIComputer_AirTrickInput` 0x138d60(pad), the computer's air trick (control state 0x10). With +0x87c set: a flip
/// stick is let go (0) when the flip angle is within 19° before or 5° past a whole turn (`% 360 − 5 ≥ 336` unsigned) and
/// +0x57b8 is under a whole flip at the rate (2π / |+0x770|); a spin stick when the spin angle is within 20° before or
/// 5° past a half turn (a whole turn on board 2) and +0x57b8 is under that turn at the rate (+0x76c). Then with no
/// trick planned (0x1a) and +0x884 set, a trick is drawn (`AIComputer_RandomTrick` 0x11d368) once +0x57fc and +0x57b8
/// are both over 0.8 / (stat × 0.8057834 / 255 + 0.78212035), clearing +0x884; with one, it is dropped (0x1a) when
/// +0x57b8 is under that (under 0.3 with +0x880 set). The pad word: the spin and flip sticks in bits 12–13 and 14–15,
/// the trick in the low byte, and with no trick and both sticks 0 the steer at the line (`AIComputer_SteerTowards`,
/// `steer`) and `AIComputer_DirNoseComponent` 0x1371e0 of +0x57e0 (`nose`, [`across`]) negated in bits 20–21 and
/// 22–23.
pub fn air_trick_input(pad: &mut u32, o: &mut AirTrick, r: &AirTrickRider, random_trick: &mut dyn FnMut() -> u32, steer: &mut dyn FnMut() -> f32, nose: &mut dyn FnMut() -> f32) {
    let left = r.x57b8;
    if o.x87c != 0 {
        if o.flip != 0.0 && (whole_degrees(r.flip_angle) % 360).wrapping_sub(5) as u32 >= 0x150 {
            let turn = F(f32::from_bits(0x40c9_0fdb)) / F(r.flip_rate.abs());
            if left < turn.0 {
                o.flip = 0.0;
            }
        }
        if o.spin != 0.0 {
            let m = if r.board == 2 { 360 } else { 180 };
            let rem = whole_degrees(r.spin_angle) % m;
            if m - 20 < rem || rem < 5 {
                let turn = F(m as f32) * F(f32::from_bits(0x3c8e_fa35)) / F(r.spin_rate.abs());
                if left < turn.0 {
                    o.spin = 0.0;
                }
            }
        }
    }
    let limit = || (F(f32::from_bits(0x3f4c_cccd)) / (F(r.stat as f32) * F(f32::from_bits(0x3f4e_47d2)) * F(f32::from_bits(0x3b80_8081)) + F(f32::from_bits(0x3f48_390a)))).0;
    if o.trick == 0x1a {
        if o.x884 != 0 {
            let t = limit();
            if t < r.x57fc && t < left {
                o.trick = random_trick();
                o.x884 = 0;
            }
        }
    } else {
        let drop = if o.x880 != 0 { left < f32::from_bits(0x3e99_999a) } else { left < limit() };
        if drop {
            o.trick = 0x1a;
        }
    }
    *pad = *pad & 0xffff_cfff | two_bits(o.spin) << 0xc;
    *pad = *pad & 0xffff_3fff | two_bits(o.flip) << 0xe;
    *pad = *pad & !0xff | (o.trick & 0xff);
    if o.trick == 0x1a && o.spin == 0.0 && o.flip == 0.0 {
        *pad = *pad & 0xffcf_ffff | two_bits(-steer()) << 0x14;
        *pad = *pad & 0xff3f_ffff | two_bits(-nose()) << 0x16;
    }
}

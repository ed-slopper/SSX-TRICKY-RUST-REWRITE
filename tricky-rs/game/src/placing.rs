//! The race's placings and its rubber band (board row G8m): `Race_UpdatePlacingsAndRubberBand` 0x115100 on the
//! game's bytes (`pf::Mem`), checked in the function runner (`tests/placing.rs`).
//!
//! Every 6th tick (`race+0x18`), with two riders or more (`race+0x88`, the list at `race+0xc4`) and a player
//! (`race+0x7c`):
//! - each rider gets a key by the game mode (0x32de8c): knock-down practice (0) its score's +0x48 less +0x4c (rider
//!   +0x58dc / +0x58e0), the show-offs (3 / 5) its points (+0x5910), the tutorial (6) its index, any other while racing
//!   −(race progress + place × 20) and once finished 0x7fffffff − its finishing time (+0x418);
//! - `Sort_ShellPairs` orders them, and each rider whose place (+0x110) changes gets `Sfx_OnPlacingChange` (the audio
//!   object; the rider, the one placed after it, the one before, the old place, the new) before the new place is set.
//!
//! Then the rubber band, by mode:
//! - knock-down practice: every rider's bounds +0x108 / +0x104 at ±520.33, and its target (+0x100) the rider placed
//!   just ahead (the leader's, the one just behind);
//! - races (2 and the World Circuit's 4): the riders split by +0x41c (0: the list given targets, else the targets'
//!   list), in placing order. The first `race+0x80` of the first list each get a rider of the second (the next one
//!   every n / players of them) and bounds from the main path's length, the course's distance and offset, the
//!   difficulty (0x32de9c as a halfword in mode 4, 0x32dea4 in mode 2) and four tables of six at 0x365060. As on the
//!   Xbox, neither walk is checked against how many each list got: past them the game reads whatever its stack frame
//!   holds, and so does this port (the lists live in the frame, `sp`; its float compares are the EE's, a stale
//!   index read as a float being a denormal, so zero);
//! - any other mode: every target −1.

use crate::pf::Mem;
use crate::ps2f::F;
use crate::roster::sort_shell_pairs;

/// What the placings call out to
pub trait PlacingEnv {
    /// `Boarder_GetRaceProgress` 0x1170d0
    fn progress(&mut self, m: &mut Mem, rider: u32) -> f32;
    /// `Sfx_OnPlacingChange` 0x219a48 (the audio object; the rider, the next, the previous, the old place, the new)
    fn placing_sound(&mut self, m: &mut Mem, args: [u32; 6]);
}

/// The game mode, the course, the difficulties, the audio object, the race's path set
pub const GAME_MODE: u32 = 0x32de8c;
pub const COURSE: u32 = 0x32de88;
pub const CIRCUIT_DIFFICULTY: u32 = 0x32de9c;
pub const RACE_DIFFICULTY: u32 = 0x32dea4;
pub const AUDIO: u32 = 0x343050;
pub const PATH_SET: u32 = 0x337fb8;
/// The rubber band's four tables of six floats (race / circuit ahead and behind)
pub const BAND_TABLES: u32 = 0x365060;

fn fl(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn put(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}

/// `Race_UpdatePlacingsAndRubberBand` 0x115100 (the race). `sp` is the game's stack frame there (0x190 bytes): the
/// order at +0, the keys at +0x20, the band tables copied to +0x40 / +0x60 / +0x80 / +0xa0, the lists at +0xc0 /
/// +0xe0
pub fn update(m: &mut Mem, env: &mut dyn PlacingEnv, race: u32, sp: u32) {
    if (m.r32(race + 0x18) as i32) % 6 != 0 {
        return;
    }
    let n = m.r32(race + 0x88) as i32;
    if n < 2 || m.r32(race + 0x7c) == 0 {
        return;
    }
    let mode = m.r32(GAME_MODE);
    let rider = |m: &Mem, k: u32| m.r32(race + 0xc4 + 4 * k);
    let mut keys = vec![0i32; n as usize];
    let mut order: Vec<i32> = (0..n).collect();
    for k in 0..n as u32 {
        let r = rider(m, k);
        keys[k as usize] = match mode {
            0 => m.r32(r + 0x58dc).wrapping_sub(m.r32(r + 0x58e0)) as i32,
            3 | 5 => m.r32(r + 0x5910) as i32,
            6 => k as i32,
            _ => {
                let fin = m.r32(r + 0x418) as i32;
                if fin < 0 {
                    let p = F(env.progress(m, r)) + F(m.r32(r + 0x110).wrapping_mul(20) as i32 as f32);
                    (p.0 as i32).wrapping_neg()
                } else {
                    0x7fff_ffff_i32.wrapping_sub(fin)
                }
            }
        };
    }
    sort_shell_pairs(&mut keys, &mut order);
    for (k, &o) in order.iter().enumerate() {
        m.w32(sp + 4 * k as u32, o as u32);
        m.w32(sp + 0x20 + 4 * k as u32, keys[k] as u32);
    }
    let n = m.r32(race + 0x88) as i32;
    for i in 0..n {
        let r = rider(m, order[i as usize] as u32);
        let old = m.r32(r + 0x110);
        if old as i32 != i {
            let prev = if i > 0 { rider(m, order[i as usize - 1] as u32) } else { 0 };
            let next = if i < m.r32(race + 0x88) as i32 - 1 { rider(m, order[i as usize + 1] as u32) } else { 0 };
            let a = m.r32(AUDIO);
            env.placing_sound(m, [a, r, next, prev, old, i as u32]);
        }
        let r = rider(m, order[i as usize] as u32);
        m.w32(r + 0x110, i as u32);
    }
    for t in 0..4u32 {
        for k in 0..6 {
            let v = m.r32(BAND_TABLES + 24 * t + 4 * k);
            m.w32(sp + 0x40 + 0x20 * t + 4 * k, v);
        }
    }
    let n = m.r32(race + 0x88) as i32;
    match mode {
        0 => {
            for k in 0..n as u32 {
                let r = rider(m, k);
                m.w32(r + 0x108, 0x4401_5603);
                m.w32(r + 0x104, 0xc401_5603);
            }
            let first = rider(m, m.r32(sp));
            m.w32(first + 0x100, m.r32(sp + 4));
            let mut i = 1;
            while i < m.r32(race + 0x88) as i32 {
                let r = rider(m, m.r32(sp + 4 * i as u32));
                let ahead = m.r32(sp + 4 * (i as u32 - 1));
                m.w32(r + 0x100, ahead);
                i += 1;
            }
        }
        2 | 4 => band(m, race, sp, mode, n),
        _ => {
            let mut k = 0;
            while k < m.r32(race + 0x88) as i32 {
                let r = rider(m, k as u32);
                m.w32(r + 0x100, u32::MAX);
                k += 1;
            }
        }
    }
}

/// The rubber band of a race (modes 2 and 4)
fn band(m: &mut Mem, race: u32, sp: u32, mode: u32, n: i32) {
    let rider = |m: &Mem, k: u32| m.r32(race + 0xc4 + 4 * k);
    let (given, targets) = (sp + 0xc0, sp + 0xe0);
    let (mut a, mut b) = (0u32, 0u32);
    for i in 0..n.max(0) as u32 {
        let k = m.r32(sp + 4 * i);
        if m.r32(rider(m, k) + 0x41c) == 0 {
            m.w32(given + 4 * a, k);
            a += 1;
        } else {
            m.w32(targets + 4 * b, k);
            b += 1;
        }
    }
    let players = m.r32(race + 0x7c) as i32;
    let step = n.wrapping_div(players);
    let main = m.r32(PATH_SET + 0x10);
    let mut len = fl(m, main + 0x38);
    if len.ee() <= 0.0 {
        len = F(1.0);
    }
    let (mut far, mut offset) = (F(0.0), F(0.0));
    match m.r32(COURSE) {
        0 => (far, offset) = (F(f32::from_bits(0x4808b96e)), F(f32::from_bits(0x44bf8baf))),
        1 => (far, offset) = (F(f32::from_bits(0x47f44e8c)), F(f32::from_bits(0x4462eb01))),
        2 => (far, offset) = (F(f32::from_bits(0x481b5ae0)), F(f32::from_bits(0x43d2970c))),
        3 => (far, offset) = (F(f32::from_bits(0x4806ce23)), F(f32::from_bits(0xc3fc1dbc))),
        4 => (far, offset) = (F(f32::from_bits(0x480ab338)), F(f32::from_bits(0x43fbbc71))),
        5 => (far, offset) = (F(f32::from_bits(0x47fffad0)), F(f32::from_bits(0x423ac82b))),
        8 => far = F(f32::from_bits(0x471c4b72)),
        11 => far = F(f32::from_bits(0x481c4922)),
        _ => {}
    }
    let (mut keep, mut scale) = (F(1.0), F(1.0));
    let zero = F(0.0);
    let back = -len;
    let mut j = 0u32;
    let mut i = 0i32;
    while i < m.r32(race + 0x80) as i32 {
        let ai = m.r32(given + 4 * i as u32);
        let to = m.r32(targets + 4 * j);
        let r = rider(m, ai);
        let t = rider(m, to);
        m.w32(r + 0x100, to);
        put(m, r + 0x108, len);
        put(m, r + 0x104, back);
        let lag = (len - fl(m, r + 0x378)) / len;
        let level = if mode == 4 { m.r16(CIRCUIT_DIFFICULTY) as u32 } else { m.r32(RACE_DIFFICULTY) };
        if mode == 4 {
            match level {
                0 => (keep, scale) = (F(1.0), F(f32::from_bits(0x3f19a14c))),
                1 => (keep, scale) = (F(0.5), F(f32::from_bits(0x3ecc_ccc0))),
                2 => (keep, scale) = (zero, F(f32::from_bits(0x3eb377dd))),
                _ => {}
            }
        } else {
            match level {
                0 => (keep, scale) = (F(1.0), F(f32::from_bits(0x3f19c960))),
                1 => (keep, scale) = (F(0.5), F(f32::from_bits(0x3ecd2213))),
                2 => (keep, scale) = (zero, F(f32::from_bits(0x3eb344c7))),
                _ => {}
            }
        }
        if (m.r32(r + 0x110) as i32) < m.r32(t + 0x110) as i32 {
            let table = if mode == 2 { sp + 0x60 } else { sp + 0xa0 };
            let v = fl(m, table + 4 * ai);
            let ahead = if v.ee() < 0.0 { len } else { v + offset * scale };
            let tl = fl(m, t + 0x378);
            let extra = if tl.ee() < far.ee() { tl * (F(1.0) - keep) } else { zero * lag };
            put(m, r + 0x108, ahead + extra);
        } else if fl(m, t + 0x378).ee() < far.ee() {
            put(m, r + 0x104, back);
        } else {
            let table = if mode == 2 { sp + 0x40 } else { sp + 0x80 };
            let v = fl(m, table + 4 * ai);
            let behind = if 0.0 < v.ee() { back } else { v + offset * scale };
            put(m, r + 0x104, behind - zero * lag);
        }
        i += 1;
        if step == 0 {
            // the game traps on the division by zero
            return;
        }
        if i % step == 0 {
            j += 1;
        }
    }
}

//! The sound system's scheduler (board row G10b2c2a): `cMonitor`'s 48 events (0x2c bytes each from +0x90 of the audio
//! system), scheduled with a delay and a callback, ticked every 10 ms (`cController::vf4` 0x21eb10) and fired when due
//! (0x21ea48), on the game's bytes (`pf::Mem`), checked in the function runner (`tests/sndevents.rs`). The callbacks
//! are calls through [`EventEnv`].
//!
//! An event: +0 in use, +4 its period (ms), +8 its countdown, +0xc the callback (a doubleword: +0xc the this-adjust,
//! +0xe a vtable index or −1 for a direct call, +0x10 the function, or for a vtable call its +0x10 the object's
//! vtable field), +0x18..+0x28 five words. The system: +0x80 the ticks, +0x88 how deep scheduling is, +0x8c paused.

use crate::pf::Mem;

/// What the scheduler calls out to
pub trait EventEnv {
    /// The callback `target` with (the adjusted object, the event): its answer (nonzero clears the event)
    fn call(&mut self, m: &mut Mem, target: u32, this: u32, event: u32) -> u32;
}

pub const EVENTS: u32 = 0x90;
pub const EVENT_SIZE: u32 = 0x2c;
pub const COUNT: u32 = 0x30;

fn ev(sys: u32, i: u32) -> u32 {
    sys + EVENTS + EVENT_SIZE * i
}

/// `SndEvent_Init` 0x222998: not in use, the period −1
pub fn event_init(m: &mut Mem, e: u32) {
    m.w32(e, 0);
    m.w32(e + 4, u32::MAX);
}

/// 0x222a78: free (its period −1)
pub fn event_free(m: &Mem, e: u32) -> bool {
    m.r32(e + 4) == u32::MAX
}

/// 0x222a88: cleared: not in use (0x222ad8), period and countdown −1 (0x222ab8)
pub fn event_clear(m: &mut Mem, e: u32) {
    m.w32(e, 0);
    set_times(m, e, u32::MAX);
}

/// 0x222ab8: the countdown and the period
pub fn set_times(m: &mut Mem, e: u32, ms: u32) {
    m.w32(e + 8, ms);
    m.w32(e + 4, ms);
}

/// `SndEvent_Set` 0x2229d0 (the event, the delay, the callback's doubleword, five words): cleared, the delay as its
/// period and countdown, the callback and words kept, in use (0x222ac8)
pub fn event_set(m: &mut Mem, e: u32, delay: u32, callback: [u32; 2], words: [u32; 5]) {
    event_clear(m, e);
    set_times(m, e, delay);
    m.w32(e + 0xc, callback[0]);
    m.w32(e + 0x10, callback[1]);
    for (k, w) in words.iter().enumerate() {
        m.w32(e + 0x18 + 4 * k as u32, *w);
    }
    m.w32(e, 1);
}

/// 0x222c50 (the system, n): the n-th free event (counting from 1), or −1 past the 48th
pub fn find_free(m: &Mem, sys: u32, n: u32) -> i32 {
    let mut left = n;
    for i in 0..COUNT {
        if event_free(m, ev(sys, i)) {
            left = left.wrapping_sub(1);
            if left == 0 {
                return i as i32;
            }
        }
    }
    -1
}

/// `SndSys_ScheduleEvent` 0x222cc0 (the system, the delay, the callback, five words): one deeper (+0x88), the free
/// event at that depth set; the event's number or −1
pub fn schedule(m: &mut Mem, sys: u32, delay: u32, callback: [u32; 2], words: [u32; 5]) -> i32 {
    m.w32(sys + 0x88, m.r32(sys + 0x88).wrapping_add(1));
    let n = m.r32(sys + 0x88);
    let i = find_free(m, sys, n);
    if 0 <= i {
        event_set(m, ev(sys, i as u32), delay, callback, words);
    }
    m.w32(sys + 0x88, m.r32(sys + 0x88).wrapping_sub(1));
    i
}

/// `SndSys_CancelEvents` 0x222df0 (the system, a callback): every event with the same vtable index and, for a direct
/// call (index −1) or a vtable field that differs in its low half, the same function word, cleared
pub fn cancel(m: &mut Mem, sys: u32, callback: [u32; 2]) {
    let index = (callback[0] >> 16) as u16;
    for i in 0..COUNT {
        let e = ev(sys, i);
        if index != m.r16(e + 0xe) {
            continue;
        }
        let hit = if index == 0xffff || (callback[1] as u16) != m.r16(e + 0x10) { callback[1] == m.r32(e + 0x10) } else { true };
        if hit {
            event_clear(m, e);
        }
    }
}

/// 0x222db0: all 48 events cleared
pub fn clear_all(m: &mut Mem, sys: u32) {
    for i in 0..COUNT {
        event_clear(m, ev(sys, i));
    }
}

/// `cMonitor::vf4` 0x222c30: unless paused (+0x8c), one tick more (+0x80)
pub fn monitor_tick(m: &mut Mem, sys: u32) {
    if m.r32(sys + 0x8c) == 0 {
        m.w32(sys + 0x80, m.r32(sys + 0x80).wrapping_add(1));
    }
}

/// 0x21ea48 (the event, the system): in use, its countdown down 10; under 5 its callback (direct, or the object's
/// vtable entry by index: the entry's function and its adjust added to the event's), a nonzero answer clearing the
/// event, else the countdown up by the period
pub fn fire(m: &mut Mem, env: &mut dyn EventEnv, e: u32, sys: u32) {
    if m.r32(e) == 0 {
        return;
    }
    let left = m.r32(e + 8).wrapping_sub(10);
    m.w32(e + 8, left);
    if 5 <= left as i32 {
        return;
    }
    let index = m.r16(e + 0xe) as i16 as i32;
    let adj = m.r16(e + 0xc) as i16 as i32;
    let (f, this) = if 0 < index {
        let vt = m.r32(sys.wrapping_add(m.r16(e + 0x10) as i16 as u32));
        let entry = vt.wrapping_add((index as u32) << 3).wrapping_sub(8);
        let f = m.r32(entry + 4);
        let a = m.r16(entry) as i16 as i32;
        (f, sys.wrapping_add(a.wrapping_add(adj) as u32))
    } else {
        (m.r32(e + 0x10), sys.wrapping_add(adj as u32))
    };
    if env.call(m, f, this, e) != 0 {
        event_clear(m, e);
        return;
    }
    m.w32(e + 8, m.r32(e + 8).wrapping_add(m.r32(e + 4)));
}

/// `cController::vf4` 0x21eb10 (the audio system, every 10 ms): unless paused, a tick and each event fired in turn
pub fn tick(m: &mut Mem, env: &mut dyn EventEnv, sys: u32) {
    if m.r32(sys + 0x8c) != 0 {
        return;
    }
    monitor_tick(m, sys);
    for i in 0..COUNT {
        fire(m, env, ev(sys, i), sys);
    }
}

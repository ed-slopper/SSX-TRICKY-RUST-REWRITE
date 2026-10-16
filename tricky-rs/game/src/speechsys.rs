//! The speech system's glue (board row G10c2): the speech system object (`cSpeechSys`, built on the stream system
//! at `gSpeechSys` 0x343700), its two slots (+0x3718: the announcer, the riders), each slot's banks, and the calls
//! that start a line against the stream playing, on the game's bytes (`pf::Mem`), checked in the function runner
//! (`tests/speechsys.rs`). Everything outside (the allocator, the stream objects, the speech library, the bank
//! loader `Speech_LoadEvents`) is a call through [`SysEnv`], answered by address.
//!
//! A slot (0x36c bytes, filled with 0xdeadc0de when allocated): +0 its stream, +4 the stream it follows (−1 none), +8
//! a byte, +0xc8 four pairs (a block, its flag), +0xe8 the count of open archives at +0x358, +0xec the count of banks
//! at +0xf0, +0x348 the follow flag, +0x34c / +0x350 / +0x354 the play call's last three words, +0x368 its float. A
//! bank's +0 is its id, +4 its name.

use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::GAPP;

/// What the speech system calls out to
pub trait SysEnv {
    /// The call to the function at `target` with its arguments (a0.., then t0..); its answer (v0)
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32;
    /// The same for a call answering a float (f0)
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32;
}

/// `gSpeechSys`, the speech system (and the stream system its slots play on)
pub const SYS: u32 = 0x343700;
/// How many banks are loaded
pub const BANK_COUNT: u32 = 0x343704;
/// The front end's lines (two words each)
pub const FE_LINES: u32 = 0x343710;
/// `cSpeechSys`'s vtable
pub const VTABLE: u32 = 0x3a0470;

pub const ALLOC: u32 = 0x2ccf70;
pub const FREE: u32 = 0x2ccfc0;
pub const MEMSET: u32 = 0x2cc808;
pub const DELETE: u32 = 0x114f80;
pub const SPRINTF: u32 = 0x2fca28;
pub const LOAD_EVENTS: u32 = 0x227d70;
pub const MUSIC_INIT: u32 = 0x224a98;
pub const SCHEDULE: u32 = 0x222cc0;
pub const STREAM_DELETE: u32 = 0x229268;
pub const STREAM_STATE: u32 = 0x229870;
pub const STREAM_TIME_LEFT: u32 = 0x229578;
pub const STREAM_PLAY_TIME: u32 = 0x2296c0;
pub const STREAM_PLAYING: u32 = 0x229628;
pub const STREAM_WAIT: u32 = 0x2295c0;
pub const STREAM_STOP: u32 = 0x229fa0;
pub const STREAM_OPEN: u32 = 0x229ae8;
pub const LIB_SERVICE: u32 = 0x2bc548;
pub const LIB_UNLOAD_BANK: u32 = 0x2bb2b0;
pub const LIB_SHUTDOWN: u32 = 0x2bb018;

fn cvt(v: f32) -> i32 {
    F(v).ee() as i32
}

fn stream(m: &Mem, sys: u32, slot: u32) -> u32 {
    m.r32(m.r32(sys + 0xa50).wrapping_add(slot.wrapping_mul(4)))
}

/// `StreamSys_GetSlotState` 0x229f60 (the stream system, a slot): its stream's state, 0 with none
pub fn slot_state(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32) -> u32 {
    let s = stream(m, sys, slot);
    if s == 0 { 0 } else { e.call(m, STREAM_STATE, &[s]) }
}

/// `StreamSys_GetSlotTimeLeftMs` 0x229fc8: the milliseconds left, 0.0 with no stream
pub fn time_left(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32) -> f32 {
    let s = stream(m, sys, slot);
    if s == 0 { 0.0 } else { e.call_f(m, STREAM_TIME_LEFT, &[s]) }
}

/// `StreamSys_GetPlayTime` 0x22a008: the milliseconds played, 0.0 with no stream
pub fn play_time(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32) -> f32 {
    let s = stream(m, sys, slot);
    if s == 0 { 0.0 } else { e.call_f(m, STREAM_PLAY_TIME, &[s]) }
}

/// `StreamSys_IsSlotPlaying` 0x22a178
pub fn is_playing(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32) -> u32 {
    let s = stream(m, sys, slot);
    if s == 0 { 0 } else { e.call(m, STREAM_PLAYING, &[s]) }
}

/// `StreamSys_HasReached` 0x22a048 (the stream system, a slot, milliseconds): the stream's answer, 0 with none
pub fn slot_wait(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32, ms: u32) -> u32 {
    let s = stream(m, sys, slot);
    if s == 0 { 0 } else { e.call(m, STREAM_WAIT, &[s, ms]) }
}

/// `StreamSys_DestroyStream` 0x229d40: the slot's stream deleted, its entry cleared
pub fn destroy_stream(m: &mut Mem, e: &mut dyn SysEnv, sys: u32, slot: u32) {
    let s = stream(m, sys, slot);
    if s != 0 {
        e.call(m, STREAM_DELETE, &[s, 3]);
    }
    let p = m.r32(sys + 0xa50).wrapping_add(slot.wrapping_mul(4));
    m.w32(p, 0);
}

/// `SpeechBank_Delete` 0x227c40 (a bank, the delete flags): its library bank unloaded, its block freed when it owns
/// one (+0x88 set, the block at +0x84), and itself with flag 1
pub fn bank_delete(m: &mut Mem, e: &mut dyn SysEnv, b: u32, flags: u32) {
    let id = m.r32(b);
    e.call(m, LIB_UNLOAD_BANK, &[id]);
    if m.r32(b + 0x88) != 0 {
        let p = m.r32(b + 0x84);
        e.call(m, FREE, &[p]);
    }
    if flags & 1 != 0 {
        e.call(m, DELETE, &[b]);
    }
}

/// `SpeechBank_HasId` 0x227ca8
pub fn bank_has_id(m: &Mem, b: u32, id: u32) -> bool {
    id == m.r32(b)
}

/// `SpeechBank_Init` 0x227cb8 (the slot, a1, its stream, a3, the stream's size, the stream it follows): the slot's
/// words cleared, its stream opened (`StreamSys_Open`, 0x20000 bytes) and its events loaded (`Speech_LoadEvents`
/// with a1 and a3)
#[allow(clippy::too_many_arguments)]
pub fn slot_init(m: &mut Mem, e: &mut dyn SysEnv, b: u32, a1: u32, stream: u32, a3: u32, size: u32, follows: u32) -> u32 {
    m.w32(b, stream);
    m.w32(b + 4, follows);
    for o in [0xe8, 0xec, 0x348, 0x34c, 0x350, 0x354, 0x368] {
        m.w32(b + o, 0);
    }
    m.w8(b + 8, 0);
    for k in 0..4 {
        m.w32(b + 0xc8 + 8 * k, 0);
        m.w32(b + 0xcc + 8 * k, 0);
    }
    let (sys, s) = (m.r32(SYS), m.r32(b));
    e.call(m, STREAM_OPEN, &[sys, s, size, 0, 0x20000]);
    e.call(m, LOAD_EVENTS, &[b, a1, a3]);
    b
}

/// `SpeechSys_FreeBanks` 0x228168 (a slot, the delete flags): its banks deleted (the loaded count less by them),
/// its archives and blocks freed, its stream destroyed, and itself with flag 1
pub fn slot_free(m: &mut Mem, e: &mut dyn SysEnv, b: u32, flags: u32) {
    let mut n = m.r32(b + 0xec) as i32;
    let mut i = 0;
    while i < n {
        let k = m.r32(b + 0xf0 + 4 * i as u32);
        if k != 0 {
            bank_delete(m, e, k, 3);
            n = m.r32(b + 0xec) as i32;
        }
        i += 1;
    }
    m.w32(BANK_COUNT, m.r32(BANK_COUNT).wrapping_sub(n as u32));
    let mut i = 0u32;
    while (i as i32) < m.r32(b + 0xe8) as i32 {
        let a = m.r32(b + 0x358 + 4 * i);
        if a != 0 {
            e.call(m, FREE, &[a]);
        }
        if m.r32(b + 0xcc + 8 * i) != 0 {
            let p = m.r32(b + 0xc8 + 8 * i);
            e.call(m, FREE, &[p]);
            m.w32(b + 0xcc + 8 * i, 0);
            m.w32(b + 0xc8 + 8 * i, 0);
        }
        i += 1;
    }
    let (sys, s) = (m.r32(SYS), m.r32(b));
    destroy_stream(m, e, sys, s);
    if flags & 1 != 0 {
        e.call(m, DELETE, &[b]);
    }
}

/// `SpeechSys_IsIdle` 0x2282e0 (a slot): its stream is free, or playing (state 1) with under 500 ms left
pub fn slot_idle(m: &mut Mem, e: &mut dyn SysEnv, b: u32) -> bool {
    let (sys, s) = (m.r32(SYS), m.r32(b));
    let st = slot_state(m, e, sys, s);
    if st == 0 {
        return true;
    }
    if st != 1 {
        return false;
    }
    let (sys, s) = (m.r32(SYS), m.r32(b));
    if is_playing(m, e, sys, s) == 0 {
        return false;
    }
    let (sys, s) = (m.r32(SYS), m.r32(b));
    cvt(time_left(m, e, sys, s)) < 500
}

/// `SpeechSys_ServiceIfIdle` 0x228370 (a slot, its number): the library serviced for it when the slot is idle
pub fn service_if_idle(m: &mut Mem, e: &mut dyn SysEnv, b: u32, slot: u32) {
    if slot_idle(m, e, b) {
        e.call(m, LIB_SERVICE, &[slot]);
    }
}

/// `Speech_PlayEvent` 0x228400 (a slot, the event: its bank id, its line; `sp` the frame, 0x1a0 bytes): the line of
/// the first bank with the id, named "<bank's name>" into the frame (the format at 0x3a0418), played by the stream
/// system's play call (vtable +0x7c: the slot's stream, the name, the line, +0x350, the start, +0x354, the float
/// +0x368). With the slot following another stream (+0x348 set, +4 not −1) the line starts at once (−1) and is
/// scheduled (`SndSys_ScheduleEvent`, the doubleword at 0x3a0420) to the other's time left less 30 ms, plus its play
/// time when that is under 300 ms. While the slot's own stream is busy that holds only when it is playing, and with
/// 300 ms or more left the line is played as usual (start +0x34c, not scheduled) unless the two streams' times left
/// are 40 ms or more apart, when the play time is added only if the other's time less 30 ms is under the slot's own.
/// The play call's answer, −1 when no bank has the id
pub fn play_event(m: &mut Mem, e: &mut dyn SysEnv, b: u32, ev: u32, sp: u32) -> i32 {
    let n = m.r32(b + 0xec) as i32;
    let mut i = 0;
    while i < n {
        let bank = m.r32(b + 0xf0 + 4 * i as u32);
        if !bank_has_id(m, bank, m.r32(ev)) {
            i += 1;
            continue;
        }
        e.call(m, SPRINTF, &[sp, 0x3a0418, bank.wrapping_add(4)]);
        let sys = m.r32(SYS);
        let pt = cvt(play_time(m, e, sys, m.r32(b + 4)));
        let sys = m.r32(SYS);
        let mut delay = cvt(time_left(m, e, sys, m.r32(b + 4)));
        let sys = m.r32(SYS);
        let own = cvt(time_left(m, e, sys, m.r32(b)));
        let gap = own.wrapping_sub(delay).wrapping_abs();
        let sys = m.r32(SYS);
        let busy = slot_state(m, e, sys, m.r32(b)) != 0;
        let follow = m.r32(b + 0x348) != 0 && m.r32(b + 4) != u32::MAX;
        let scheduled = if !follow {
            false
        } else if !busy {
            delay = delay.wrapping_sub(30);
            if delay <= 299 {
                delay = delay.wrapping_add(pt);
            }
            true
        } else {
            let sys = m.r32(SYS);
            if is_playing(m, e, sys, m.r32(b)) == 0 {
                false
            } else if delay < 0x12d {
                delay = delay.wrapping_sub(30).wrapping_add(pt);
                true
            } else if gap < 0x28 {
                false
            } else {
                let behind = (delay.wrapping_sub(30) < own) as i32;
                delay = behind.wrapping_mul(pt).wrapping_add(delay).wrapping_sub(30);
                true
            }
        };
        let start = if scheduled { u32::MAX } else { m.r32(b + 0x34c) };
        let sys = m.r32(SYS);
        let vt = m.r32(sys + 4);
        let f = m.r32(vt + 0x7c);
        let this = sys.wrapping_add(m.r16(vt + 0x78) as i16 as u32);
        let args = [this, m.r32(b), sp, m.r32(ev + 4), m.r32(b + 0x350), start, m.r32(b + 0x354), m.r32(b + 0x368)];
        let h = e.call(m, f, &args) as i32;
        if !scheduled || h < 0 {
            return h;
        }
        let sys = m.r32(SYS);
        let (lo, hi) = (m.r32(0x3a0420), m.r32(0x3a0424));
        e.call(m, SCHEDULE, &[sys, delay as u32, lo, hi, 0, h as u32, 0, 0, 0]);
        return h;
    }
    -1
}

/// `SpeechSys_StopStream` 0x2286e8 (a slot)
pub fn slot_stop(m: &mut Mem, e: &mut dyn SysEnv, b: u32) {
    let (sys, s) = (m.r32(SYS), m.r32(b));
    e.call(m, STREAM_STOP, &[sys, s]);
}

/// `SpeechSlot_IsPlaying` 0x228700 (a slot)
pub fn slot_playing(m: &mut Mem, e: &mut dyn SysEnv, b: u32) -> u32 {
    let (sys, s) = (m.r32(SYS), m.r32(b));
    is_playing(m, e, sys, s)
}

/// `SpeechSys_Init` 0x228728 (the speech system): the stream system built (`MusicSys_Init`), `gSpeechSys` set, the
/// vtable, +0x371c / +0x3720 0, +0x3724 −1, and the two slots' table (8 bytes, 0xdeadc0ed, then cleared) at +0x3718
pub fn init(m: &mut Mem, e: &mut dyn SysEnv, this: u32) -> u32 {
    e.call(m, MUSIC_INIT, &[this]);
    m.w32(SYS, this);
    m.w32(this + 0x371c, 0);
    m.w32(this + 0x3720, 0);
    m.w32(this + 4, VTABLE);
    m.w32(this + 0x3724, u32::MAX);
    let p = e.call(m, ALLOC, &[0x3a0428, 8, 0]);
    if p != 0 {
        e.call(m, MEMSET, &[p, 0xdead_c0ed, 8]);
    }
    m.w32(this + 0x3718, p);
    m.w32(p.wrapping_add(4), 0);
    m.w32(p, 0);
    this
}

/// 0x2bae08 (a rate, a width, a kind): the bytes of `rate × width` bits, a tenth of them for kind 1, two sevenths for
/// kind 2
pub fn buffer_bytes(rate: i32, width: i32, kind: i32) -> i32 {
    let bits = rate.wrapping_mul(width);
    let bytes = (if bits >= 0 { bits } else { bits.wrapping_add(7) }) >> 3;
    match kind {
        1 => bytes / 10,
        2 => (bytes << 1) / 7,
        _ => bytes,
    }
}

/// `Speech_InitLibrary` 0x228830 (the speech system): the library started (`SpeechLib_Init` with the callback
/// 0x227b00, the system's own answer from its vtable +0x24, and a tenth of a second at 22050 Hz, 16 bits), then 300
/// event slots allocated (+0x371c) and handed to the library
pub fn init_library(m: &mut Mem, e: &mut dyn SysEnv, this: u32) {
    let bytes = buffer_bytes(0x5622, 0x10, 1);
    let vt = m.r32(this + 4);
    let f = m.r32(vt + 0x24);
    let adj = this.wrapping_add(m.r16(vt + 0x20) as i16 as u32);
    let a = e.call(m, f, &[adj]);
    e.call(m, 0x188990, &[0x3a0438, a]);
    lib_init(m, e, 0x227b00, a, bytes as u32);
    let n = e.call(m, 0x2bb090, &[300]);
    let p = e.call(m, ALLOC, &[0x3a0450, n, 0]);
    m.w32(this + 0x371c, p);
    e.call(m, 0x2bb098, &[300, p]);
}

/// `SpeechSys_FreeAll` 0x2288d8: both slots freed and cleared, the library shut down, the event slots freed
pub fn free_all(m: &mut Mem, e: &mut dyn SysEnv, this: u32) {
    for i in 0..2u32 {
        let b = m.r32(m.r32(this + 0x3718) + 4 * i);
        if b != 0 {
            slot_free(m, e, b, 3);
        }
        let t = m.r32(this + 0x3718);
        m.w32(t + 4 * i, 0);
    }
    e.call(m, LIB_SHUTDOWN, &[]);
    let p = m.r32(this + 0x371c);
    e.call(m, FREE, &[p]);
    m.w32(this + 0x371c, 0);
}

/// `SpeechSys_UpdateAll` 0x228998: each slot serviced when idle
pub fn update_all(m: &mut Mem, e: &mut dyn SysEnv, this: u32) {
    for i in 0..2u32 {
        let b = m.r32(m.r32(this + 0x3718) + 4 * i);
        if b != 0 {
            service_if_idle(m, e, b, i);
        }
    }
}

/// `SpeechSys_AddBank` 0x228a00 (the speech system, the slot's number, then `SpeechBank_Init`'s stream, a3, size,
/// follows): a new slot (0x36c bytes, 0xdeadc0de) made and put in the table
#[allow(clippy::too_many_arguments)]
pub fn add_slot(m: &mut Mem, e: &mut dyn SysEnv, this: u32, slot: u32, stream: u32, a3: u32, size: u32, follows: u32) {
    let at = slot.wrapping_mul(4).wrapping_add(m.r32(this + 0x3718));
    let p = e.call(m, ALLOC, &[0x3a0460, 0x36c, 0]);
    if p != 0 {
        e.call(m, MEMSET, &[p, 0xdead_c0de, 0x36c]);
    }
    let v = slot_init(m, e, p, slot, stream, a3, size, follows);
    m.w32(at, v);
}

/// `Speech_LoadEventsForSlot` 0x228ac0 (the speech system, the slot's number, a2): `Speech_LoadEvents` on the slot
/// (with the number and a2) when it exists
pub fn load_events_for_slot(m: &mut Mem, e: &mut dyn SysEnv, this: u32, slot: u32, a2: u32) {
    let b = m.r32(slot.wrapping_mul(4).wrapping_add(m.r32(this + 0x3718)));
    if b != 0 {
        e.call(m, LOAD_EVENTS, &[b, slot, a2]);
    }
}

/// `SpeechSys_IsSlotPlaying` 0x228dd0 (the speech system, the slot's number)
pub fn is_slot_playing(m: &mut Mem, e: &mut dyn SysEnv, this: u32, slot: u32) -> u32 {
    let b = m.r32(slot.wrapping_mul(4).wrapping_add(m.r32(this + 0x3718)));
    if b == 0 { 0 } else { slot_playing(m, e, b) }
}

/// `Speech_GetFEModeMask` 0x22e550: the game mode (gApp+0x58) as the front end's speech bit
pub fn fe_mode_mask(m: &Mem) -> u32 {
    match m.r32(m.r32(GAPP) + 0x58) {
        2 => 2,
        3 => 8,
        4 => 4,
        _ => 1,
    }
}

/// `Speech_PlayFELine` 0x22e5a8 (the speech system, a line or −1; `sp` its frame): the speech stopped (0x228d10
/// with 0), then for a line its two words (0x343710 + 8·line) and the mode's bit posted (`SpeechEvt_Post` 0x22e4e0)
/// and the slots serviced
pub fn play_fe_line(m: &mut Mem, e: &mut dyn SysEnv, this: u32, line: u32, sp: u32) {
    e.call(m, 0x228d10, &[this, 0]);
    if line == u32::MAX {
        return;
    }
    let at = FE_LINES + 8 * line;
    m.w32(sp + 4, m.r32(at + 4));
    m.w32(sp, m.r32(at));
    let mask = fe_mode_mask(m);
    m.w32(sp + 8, mask);
    e.call(m, 0x22e4e0, &[sp, sp | 4, sp | 8]);
    update_all(m, e, this);
}

/// `Speech_FreeNarration` 0x230d78: the narration's object (+0x7dbc) deleted and its block (+0x7dc0) freed, +0x7dc4
/// cleared
pub fn free_narration(m: &mut Mem, e: &mut dyn SysEnv, this: u32) {
    let o = m.r32(this + 0x7dbc);
    if o != 0 {
        e.call(m, 0x209b40, &[o, 3]);
        m.w32(this + 0x7dbc, 0);
    }
    let p = m.r32(this + 0x7dc0);
    if p == 0 {
        m.w32(this + 0x7dc4, 0);
    } else {
        e.call(m, 0x14a040, &[p]);
        m.w32(this + 0x7dc0, 0);
        m.w32(this + 0x7dc4, 0);
    }
}

/// `SpeechSys_IsCharScriptReady` 0x235710: 1 unless a script is set (+0x6e64) and its slot (0x228e10 with 1) has a
/// stream, then `StreamSys_HasReached` at 500 ms
pub fn char_script_ready(m: &mut Mem, e: &mut dyn SysEnv, this: u32) -> u32 {
    if m.r32(this + 0x6e64) == 0 {
        return 1;
    }
    let slot = e.call(m, 0x228e10, &[this, 1]);
    if slot_state(m, e, this, slot) == 0 {
        return 1;
    }
    slot_wait(m, e, this, slot, 500)
}

/// `SpeechLib_Init` 0x2baf38 (the callback, a1, the buffer's bytes): the library's parts reset in its order, its
/// globals set (0x348de0 the callback, 0x3494f0 a1, 0x3494f4 the bytes, 0x348dd4 0x2bbcf8, 0x348dc8 0x1789a34),
/// eight words at 0x3494f8 cleared; 1
pub fn lib_init(m: &mut Mem, e: &mut dyn SysEnv, cb: u32, a1: u32, bytes: u32) -> u32 {
    e.call(m, 0x2baed0, &[]);
    e.call(m, 0x2baeb8, &[]);
    m.w32(0x348de0, cb);
    m.w32(0x3494f4, bytes);
    m.w32(0x348dd4, 0x2bbcf8);
    m.w32(0x3494f0, a1);
    e.call(m, 0x2be8c8, &[a1]);
    e.call(m, 0x2bdae0, &[]);
    e.call(m, 0x2be208, &[0]);
    for k in (0..8u32).rev() {
        m.w32(0x3494f8 + 4 * k, 0);
    }
    e.call(m, 0x2bb7a0, &[]);
    e.call(m, 0x2badc0, &[]);
    e.call(m, 0x2bb060, &[]);
    e.call(m, 0x2bb810, &[]);
    m.w32(0x348dc8, 0x1789a34);
    1
}

/// 0x2bebe8: the library's fatal stop (0x3495d3 = 'B', then 0x2f5450 with 0, which does not come back)
pub fn lib_fatal(m: &mut Mem, e: &mut dyn SysEnv) {
    m.w8(0x3495d3, 0x42);
    e.call(m, 0x2f5450, &[0]);
}

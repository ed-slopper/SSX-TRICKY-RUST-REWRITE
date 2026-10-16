//! The audio system's play of its queued sound (board row G10b2c2c2b): `SndSys_PlayPending` 0x20c948 and the delay's
//! echo callback 0x20c680, on the game's bytes (`pf::Mem`), checked in the function runner (`tests/musicsys.rs`). The
//! queue is the one [`crate::sndqueue`] describes (the index +0x20, the records +0x24, the tables +0x28..+0x7c,
//! +0xa40 the kind, +0xa44 the id); the sound library and the bank monitor are called through
//! [`crate::musicsong::SongEnv`].

use crate::musicsong::SongEnv;
use crate::pf::Mem;
use crate::ps2f::F;

/// `SND_Lock` 0x2d0908, `SND_PlayBankSound` 0x2d1718 (the bank's handle or −1, the program, the record),
/// `SND_Unlock` 0x2d0938
pub const SND_LOCK: u32 = 0x2d0908;
pub const SND_PLAY_BANK_SOUND: u32 = 0x2d1718;
pub const SND_UNLOCK: u32 = 0x2d0938;
/// `BankMonitor_AddInstance` 0x20bd68 (the system, the sound, the volume, 1), 0x20aba8 (its answer, the bank's
/// handle or −1, the program)
pub const ADD_INSTANCE: u32 = 0x20bd68;
pub const INSTANCE_SET: u32 = 0x20aba8;
pub const SCHEDULE: u32 = 0x222cc0;
pub const MEMSET: u32 = 0x2fb468;
/// The audio system (`gAudio`)
pub const AUDIO: u32 = 0x342c48;
/// The echo's scheduler event (a doubleword: the callback 0x20c680)
pub const EV_ECHO: u32 = 0x39e870;

fn tab(m: &Mem, s: u32, o: u32, i: u32) -> u32 {
    i.wrapping_mul(4).wrapping_add(m.r32(s + o))
}

/// A volume word held to the byte 0..127 (inlined everywhere)
fn byte(v: i32) -> u8 {
    if v < 0x7f {
        if v < 0 {
            0
        } else {
            v as u8
        }
    } else {
        0x7f
    }
}

/// The bank's handle for a kind (the bank's state 1, its +4; else −1)
fn handle(m: &Mem, s: u32, at4: u32) -> u32 {
    let bank = m.r32(at4.wrapping_add(m.r32(s + 0xa40)));
    let rec = bank.wrapping_mul(0x60).wrapping_add(m.r32(s + 0xa3c));
    if m.r32(rec) == 1 {
        m.r32(rec + 4)
    } else {
        u32::MAX
    }
}

/// Held to 0..127 as the echoes' volume is (above 127 → 127, negative → 0)
fn echo_clamp(v: i32) -> i32 {
    let v = if v < 0x80 { v } else { 0x7f };
    if -1 < v {
        v
    } else {
        0
    }
}

/// `SndSys_PlayPending` 0x20c948 (the system): the queue's top sound played. Its volume (+0x3c): 0 for a rider's
/// (+0x28 set), else times its slot's scale (+0x40, none: as it is) truncated and held to 0..127, into its record's
/// first byte. Then, with the sound library locked, `SND_PlayBankSound` (the kind's bank's handle, the id, the
/// record). A sound that played, or failed without +0x7c set, goes to the bank monitor (`BankMonitor_AddInstance`,
/// with the volume before the scaling, and 0x20aba8 on its answer), and with DelayCount left in its effect row
/// (+0x74: +0 DelayTime, +4 the count, +8 DelayFeedback %, +0xc DelayLevel %) its echoes are scheduled (the echo
/// event, DelayTime further on each, the slot, kind, id and the echo's volume: the volume × level % × feedback %,
/// then × feedback % each, held to 0..127) until the count runs out or the volume reaches 0. The index down one; the
/// sound's handle
pub fn play_pending(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> u32 {
    let t3c = m.r32(s + 0x3c);
    let i = m.r32(s + 0x20);
    let emitter = m.r32(s.wrapping_add(i.wrapping_mul(4)).wrapping_add(0x28));
    let vol0 = m.r32(i.wrapping_mul(4).wrapping_add(t3c)) as i32;
    let at = |m: &Mem| m.r32(s + 0x20).wrapping_mul(4).wrapping_add(t3c);
    let rec = |m: &Mem| m.r32(s + 0x20).wrapping_mul(0x18).wrapping_add(m.r32(s + 0x24));
    if emitter != 0 {
        let t24 = m.r32(s + 0x24);
        let a = at(m);
        m.w32(a, 0);
        let r = m.r32(s + 0x20).wrapping_mul(0x18).wrapping_add(t24);
        let b = byte(m.r32(at(m)) as i32);
        m.w8(r, b);
    } else {
        let slot = m.r32(tab(m, s, 0x40, m.r32(s + 0x20)));
        if slot != 0 {
            let scale = F(f32::from_bits(m.r32(slot)));
            let store = at(m);
            let scaled = |m: &Mem| F(m.r32(at(m)) as i32 as f32) * scale;
            let v = if scaled(m).ee() < 127.0 && scaled(m).ee() < 0.0 {
                0
            } else if scaled(m).ee() < 127.0 {
                scaled(m).ee() as i32 as u32
            } else {
                0x7f
            };
            m.w32(store, v);
            let r = rec(m);
            let b = byte(m.r32(at(m)) as i32);
            m.w8(r, b);
        }
    }
    let i4 = m.r32(s + 0x20).wrapping_mul(4);
    e.call(m, SND_LOCK, &[]);
    let h = handle(m, s, i4);
    let id = m.r32(i4.wrapping_add(m.r32(s + 0xa44)));
    let r = rec(m);
    let played = e.call(m, SND_PLAY_BANK_SOUND, &[h, id, r]);
    e.call(m, SND_UNLOCK, &[]);
    if (played as i32) < 0 && m.r32(tab(m, s, 0x7c, m.r32(s + 0x20))) != 0 {
        m.w32(s + 0x20, m.r32(s + 0x20).wrapping_sub(1));
        return played;
    }
    let inst = e.call(m, ADD_INSTANCE, &[s, played, vol0 as u32, 1]);
    if inst != 0 {
        let h = handle(m, s, i4);
        let id = m.r32(i4.wrapping_add(m.r32(s + 0xa44)));
        e.call(m, INSTANCE_SET, &[inst, h, id]);
    }
    let row = m.r32(s + 0x74).wrapping_add(m.r32(s + 0x20).wrapping_mul(0x10));
    let n = m.r32(row + 4) as i32;
    if 0 < n {
        let first = vol0.wrapping_mul(m.r32(row + 0xc) as i32) / 100;
        let fb = m.r32(row + 8) as i32;
        m.w32(row + 4, n.wrapping_sub(1) as u32);
        let mut v = echo_clamp(first.wrapping_mul(fb) / 100);
        let mut delay = m.r32(row);
        let d = [m.r32(EV_ECHO), m.r32(EV_ECHO + 4)];
        while v != 0 {
            let slot = m.r32(tab(m, s, 0x40, m.r32(s + 0x20)));
            let kind = m.r32(i4.wrapping_add(m.r32(s + 0xa40)));
            let id = m.r32(i4.wrapping_add(m.r32(s + 0xa44)));
            e.call(m, SCHEDULE, &[s, delay, d[0], d[1], slot, kind, id, v as u32, 0]);
            let n = m.r32(row + 4) as i32;
            if n <= 0 {
                break;
            }
            let fb = m.r32(row + 8) as i32;
            let step = m.r32(row);
            m.w32(row + 4, n.wrapping_sub(1) as u32);
            v = echo_clamp(v.wrapping_mul(fb) / 100);
            delay = delay.wrapping_add(step);
        }
    }
    m.w32(s + 0x20, m.r32(s + 0x20).wrapping_sub(1));
    played
}

/// The echo's callback 0x20c680 (this, the event; its words +0x18 the slot, +0x1c the kind, +0x20 the id, +0x24 the
/// volume): a sound queued on the audio system (`gAudio`) as `MusicSys_PlaySong` sets its stream up, with those, then
/// `SndSys_PlayPending`; 1 (the event cleared)
pub fn echo(m: &mut Mem, e: &mut dyn SongEnv, ev: u32) -> u32 {
    let s = m.r32(AUDIO);
    let (kind, id) = (m.r32(ev + 0x1c), m.r32(ev + 0x20));
    // (in the machine code's order of loads and stores)
    let t24 = m.r32(s + 0x24);
    m.w32(s + 0x20, m.r32(s + 0x20).wrapping_add(1));
    let ix = |m: &Mem| m.r32(s + 0x20);
    let rec = ix(m).wrapping_mul(0x18).wrapping_add(t24);
    let copy: Vec<u8> = (0..0x18).map(|k| m.r8(s + 8 + k)).collect();
    for (k, b) in copy.iter().enumerate() {
        m.w8(rec + k as u32, *b);
    }
    let i = ix(m);
    let (t38, t24) = (m.r32(s + 0x38), m.r32(s + 0x24));
    m.w32(s.wrapping_add(i.wrapping_mul(4)).wrapping_add(0x28), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t38), 0);
    let t3c = m.r32(s + 0x3c);
    let t40 = m.r32(s + 0x40);
    let vol = m.r8(i.wrapping_mul(0x18).wrapping_add(t24)) as i8 as i32;
    m.w32(i.wrapping_mul(4).wrapping_add(t3c), vol as u32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t40), 0);
    e.call(m, MEMSET, &[s.wrapping_add(i.wrapping_mul(8)).wrapping_add(0x44), 0, 8]);
    let i0 = ix(m);
    let (t68, g) = (m.r32(s + 0x68), m.r32(AUDIO));
    let t74 = m.r32(s + 0x74);
    m.w32(ix(m).wrapping_mul(4).wrapping_add(t68), 0);
    let t6c = m.r32(s + 0x6c);
    let i = ix(m);
    let t70 = m.r32(s + 0x70);
    let t64 = m.r32(s + 0x64);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(4), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t6c), 0);
    let t70_at = i.wrapping_mul(4).wrapping_add(t70);
    m.w32(i0.wrapping_mul(4).wrapping_add(t64), 0);
    let ta40 = m.r32(s + 0xa40);
    let t78 = m.r32(s + 0x78);
    m.w32(ix(m).wrapping_mul(0x10).wrapping_add(t74), 100);
    let t7c = m.r32(s + 0x7c);
    let i = ix(m);
    let ta44 = m.r32(s + 0xa44);
    let gt40 = m.r32(g + 0x40);
    m.w32(t70_at, 0);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(8), 0x5a);
    let slot = m.r32(ev + 0x18);
    m.w32(ix(m).wrapping_mul(0x10).wrapping_add(t74).wrapping_add(0xc), 0x32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t78), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(ta40), kind);
    m.w32(i.wrapping_mul(4).wrapping_add(t7c), 1);
    m.w32(ix(m).wrapping_mul(4).wrapping_add(ta44), id);
    m.w32(m.r32(g + 0x20).wrapping_mul(4).wrapping_add(gt40), slot);
    let gi = m.r32(g + 0x20);
    let (g3c, v, g24) = (m.r32(g + 0x3c), m.r32(ev + 0x24), m.r32(g + 0x24));
    m.w32(gi.wrapping_mul(4).wrapping_add(g3c), v);
    let r = m.r32(g + 0x20).wrapping_mul(0x18).wrapping_add(g24);
    let b = byte(m.r32(m.r32(g + 0x20).wrapping_mul(4).wrapping_add(g3c)) as i32);
    m.w8(r, b);
    let a = m.r32(AUDIO);
    play_pending(m, e, a);
    1
}

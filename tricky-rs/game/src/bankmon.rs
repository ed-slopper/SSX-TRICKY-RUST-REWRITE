//! The bank sounds playing and their monitor (board row E1b2g7d5e): the 64 sound instances (0xb0 each) the audio
//! system keeps of what it started on EA's sound library, and `cBankMonitor`, which finds, adds, pauses, stops and
//! counts them, on the game's bytes (`pf::Mem`), checked in the function runner (`tests/bankmon.rs`). What they call
//! out to (the sound library's handles and MIDI players, the streams, `SndVoice_IsDone` 0x20a8e0 and
//! `SndVoice_SetVolume` 0x20ae28, both ported in `sndlib`, the monitor's position virtual) goes through
//! [`SongEnv`].
//!
//! An instance: +0 its engine (0 free, 1 a bank sound, 2 a MIDI player, 3 a stream), +4 its handle, +8 playing, +0xc
//! the bank or stream and +0x10 the sound (or a name, 0x40 bytes), +0x54 stopping, +0x58 kept, +0x5c the distance
//! (float, −1 unknown), +0x60 a volume scale's address, +0x64 its volume, +0x68 the address of its position (or
//! +0x70, a copy), +0x80 / +0x84, +0x88 the fall-off distance, +0x8c the cull distance, +0x90 −1.0, +0x94 not
//! positional, +0x98 freed when done, +0x9c / +0xa0 its key, +0xa4 the pitch sent (0x1000 = 1.0), +0xa8 the volume
//! sent, +0xac the pan sent.
//!
//! The monitor is a `cMonitor` (the sound queue at +0x20: the record count, the 0x18-byte records at +0x24, and the
//! per-record arrays after; `sndqueue::Queue`) with +0x8d0 a search counter, +0x8d4 Doppler on, +0x8d8 positions on,
//! +0x8dc paused and +0x8e0 the instances (the count at −0x10).

use crate::banksys::strcpy;
use crate::musicsong::SongEnv;
use crate::pf::Mem;
use crate::ps2f::F;
use crate::sndlib::cvt_w;

pub const MONITOR_VTABLE: u32 = 0x39e7e0;
/// The instances' allocation name and the "no free instance" message
pub const NAME_INSTANCES: u32 = 0x39e790;
pub const MSG_FULL: u32 = 0x39e7a8;

pub const SND_LOCK: u32 = 0x2d0908;
pub const SND_UNLOCK: u32 = 0x2d0938;
/// `SndHandle_GetState` (the handle, a word's address): asked through `call_out` with the handle alone, it answers
/// [what it returns, the word]
pub const HANDLE_STATE: u32 = 0x2d2c10;
pub const HANDLE_FADE: u32 = 0x2d3048;
pub const HANDLE_VOLUME: u32 = 0x2d3108;
pub const HANDLE_PITCH: u32 = 0x2d31b8;
pub const HANDLE_STOP: u32 = 0x2d3260;
pub const HANDLE_PAN: u32 = 0x2d3480;
/// `SndHandle_SetPitchBend` (the handle, the bend)
pub const HANDLE_PITCH_BEND: u32 = 0x2d6a18;
pub const PLAY_BANK_SOUND: u32 = 0x2d1718;
pub const MIDI_STOP: u32 = 0x2d4368;
pub const MIDI_PLAY: u32 = 0x2d4440;
pub const MIDI_PITCH: u32 = 0x2d45a8;
pub const MIDI_PAN: u32 = 0x2d4660;
pub const MIDI_PAUSE: u32 = 0x2d4740;
pub const MIDI_RESUME: u32 = 0x2d4808;
pub const MIDI_VOLUME: u32 = 0x2d4888;
pub const MIDI_FADE: u32 = 0x2d4978;
pub const STREAM_PAN: u32 = 0x229348;
pub const STREAM_VOLUME: u32 = 0x229398;
pub const STREAM_PITCH: u32 = 0x2293a0;
pub const STREAM_STOP: u32 = 0x229868;
pub const VOICE_IS_DONE: u32 = 0x20a8e0;
pub const VOICE_SET_VOLUME: u32 = 0x20ae28;
pub const MEM_DELETE: u32 = 0x114f80;
pub const MEM_FREE: u32 = 0x14a040;
pub const EAMEM_ALLOC: u32 = 0x2ccf70;
pub const EAMEM_SET: u32 = 0x2cc808;
pub const MONITOR_CTOR: u32 = 0x222b10;
pub const MONITOR_DTOR: u32 = 0x222ba0;
pub const DEBUG_PRINTF: u32 = 0x188990;

const SIZE: u32 = 0xb0;
const COUNT: u32 = 64;
const MINUS_ONE: u32 = 0xbf80_0000;

fn f(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

fn locked(m: &mut Mem, e: &mut dyn SongEnv, target: u32, args: &[u32]) -> u32 {
    e.call(m, SND_LOCK, &[]);
    let r = e.call(m, target, args);
    e.call(m, SND_UNLOCK, &[]);
    r
}

/// `BankInstance_Init` 0x20a888
pub fn init(m: &mut Mem, s: u32) -> u32 {
    m.w32(s + 0xa4, 0x8000);
    m.w32(s + 0x90, MINUS_ONE);
    m.w32(s + 0xa8, u32::MAX);
    m.w32(s + 0xac, 0xffff_8ad0);
    m.w32(s, 0);
    m.w32(s + 0x54, 0);
    m.w32(s + 0x58, 0);
    s
}

/// `Mem_DeleteIfFlag7` 0x20a8c0 (the object, the delete flags)
pub fn delete_if(m: &mut Mem, e: &mut dyn SongEnv, p: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, MEM_DELETE, &[p]);
    }
}

/// `BankInstance_Reset` 0x20a9d8: stopped and freed
pub fn reset(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    voice_stop(m, e, s);
    m.w32(s + 0xa4, 0x8000);
    m.w32(s + 0xac, 0xffff);
    m.w32(s + 0xa8, u32::MAX);
    m.w32(s, 0);
    m.w32(s + 0x54, 0);
    m.w32(s + 0x58, 0);
}

/// `BankInstance_IsFree` 0x20aa20: free, or done and set to be freed when done (then freed)
pub fn is_free(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> bool {
    if m.r32(s) == 0 {
        return true;
    }
    if m.r32(s + 0x98) != 0 && e.call(m, VOICE_IS_DONE, &[s]) != 0 {
        reset(m, e, s);
        return true;
    }
    false
}

/// `SndVoice_DistanceVolume` 0x20aab8 (the volume, the distance, the fall-off distance): full volume within half a
/// metre, falling as the square to none at the fall-off distance
pub fn distance_volume(volume: u32, distance: F, far: F) -> u32 {
    let mut d = distance - F(0.5);
    if d.ee() < 0.0 {
        d = F(0.0);
    }
    if d.ee() < far.ee() && 0.0 < far.ee() {
        let x = (far - d) / far;
        cvt_w((x * x * F(volume as i32 as f32)).ee()) as u32
    } else {
        0
    }
}

/// `BankInstance_DistanceVolume` 0x20ab28 (the instance, the volume, the distance, the fall-off distance)
pub fn instance_distance_volume(volume: u32, distance: F, far: F) -> u32 {
    distance_volume(volume, distance, far)
}

/// `SndVoice_DopplerPitch` 0x20ab48 (the distance closed, the time): the pitch ratio of the speed, sound at 8000
/// (cm/s) and the speed clamped to ±894.08
pub fn doppler_pitch(closed: F, dt: F) -> F {
    let lim = F(f32::from_bits(0x445f_851e));
    let mut v = closed / dt;
    if v.ee() < (-lim).ee() {
        v = -lim;
    } else if lim.ee() < v.ee() {
        v = lim;
    }
    (v + F(8000.0)) / F(8000.0)
}

/// `BankInstance_SetHandle` 0x20aba8 and `BankInstance_SetHandleCopy1` 0x20abf8 (the instance, the bank, the sound)
pub fn set_handle(m: &mut Mem, s: u32, bank: u32, sound: u32) {
    m.w32(s + 0x10, sound);
    m.w32(s + 0xc, bank);
}

/// `BankInstance_SetStream` 0x20abb8 (the instance, the stream, its name, a word kept at +0x50)
pub fn set_stream(m: &mut Mem, s: u32, stream: u32, name: u32, word: u32) {
    m.w32(s + 0xc, stream);
    strcpy(m, s + 0x10, name);
    m.w32(s + 0x50, word);
}

/// The monitor's virtual +0x4c (adjusted by +0x48): the squared distance to the instance and the listener's view of
/// it (four words; the pan at +8 and +0xa), through `call_out` as [f0's bits, the words]
fn locate(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, s: u32) -> (F, Vec<u32>) {
    let vt = m.r32(mon + 4);
    let this = mon.wrapping_add(m.r16(vt + 0x48) as i16 as i32 as u32);
    let fun = m.r32(vt + 0x4c);
    let out = e.call_out(m, fun, &[this, s], 5);
    let w = |k: usize| out.get(k).copied().unwrap_or(0);
    (F(f32::from_bits(w(0))), (1..5).map(w).collect())
}

/// `BankInstance_ComputeDistance` 0x20ac08 (the instance, the monitor)
pub fn compute_distance(m: &mut Mem, e: &mut dyn SongEnv, s: u32, mon: u32) {
    let (d2, _) = locate(m, e, mon, s);
    m.w32(s + 0x5c, d2.sqrt().0.to_bits());
}

/// `BankInstance_SetPan` 0x20ac50 (the instance, the pan, the second pan word): sent when it moved more than 0x1ff
pub fn set_pan(m: &mut Mem, e: &mut dyn SongEnv, s: u32, pan: u32, b: u32) {
    if pan.wrapping_sub(m.r32(s + 0xac)).wrapping_add(0x1ff) < 0x3ff {
        return;
    }
    m.w32(s + 0xac, pan);
    match m.r32(s) as i32 {
        1 => {
            let h = m.r32(s + 4);
            locked(m, e, HANDLE_PAN, &[h, pan, b]);
        }
        2 => {
            let h = m.r32(s + 4);
            locked(m, e, MIDI_PAN, &[h, pan]);
        }
        3 => {
            let st = m.r32(s + 0xc);
            e.call(m, STREAM_PAN, &[st, pan, b]);
        }
        _ => {}
    }
}

/// `BankInstance_SetPitch` 0x20ad38 (the instance, the pitch): clamped to 0..0x3fff, sent when it moved more than
/// 0xff
pub fn set_pitch(m: &mut Mem, e: &mut dyn SongEnv, s: u32, pitch: u32) {
    let mut p = if (pitch as i32) < 0x4000 { pitch as i32 } else { 0x3fff };
    if p < 0 {
        p = 0;
    }
    let p = p as u32;
    if p.wrapping_sub(m.r32(s + 0xa4)).wrapping_add(0xff) < 0x1ff {
        return;
    }
    m.w32(s + 0xa4, p);
    match m.r32(s) as i32 {
        1 => {
            let h = m.r32(s + 4);
            locked(m, e, HANDLE_PITCH, &[h, p]);
        }
        2 => {
            let h = m.r32(s + 4);
            locked(m, e, MIDI_PITCH, &[h, p]);
        }
        3 => {
            let st = m.r32(s + 0xc);
            e.call(m, STREAM_PITCH, &[st, p]);
        }
        _ => {}
    }
}

/// `SndVoice_Stop` 0x20b0b8: faded out over 25 (once)
pub fn voice_stop(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    if m.r32(s + 0x54) != 0 {
        return;
    }
    m.w32(s + 8, 0);
    m.w32(s + 0x54, 1);
    let h = m.r32(s + 4);
    match m.r32(s) {
        1 => {
            locked(m, e, HANDLE_FADE, &[h, 0x19, u32::MAX]);
        }
        2 => {
            locked(m, e, MIDI_FADE, &[h, 0x19, u32::MAX]);
        }
        _ => {}
    }
}

/// `BankInstance_Start` 0x20b158 (the instance, the monitor's queue): a record pushed on the queue (a copy of its
/// header record, its arrays set: volume scale the instance's, envelope 100 / 90 / 50) and played from it, a bank
/// sound by `SND_PlayBankSound` 0x2d1718 or a MIDI player by 0x2d4440, the handle kept, the record popped
pub fn start(m: &mut Mem, e: &mut dyn SongEnv, s: u32, q: u32) {
    m.w32(s + 8, 1);
    let kind = m.r32(s) as i32;
    if kind != 1 && kind != 2 {
        return;
    }
    let n = m.r32(q + 0x20).wrapping_add(1);
    m.w32(q + 0x20, n);
    let rec = |m: &Mem| m.r32(q + 0x20).wrapping_mul(0x18).wrapping_add(m.r32(q + 0x24));
    let at = |m: &Mem, arr: u32, k: u32| m.r32(q + 0x20).wrapping_mul(k).wrapping_add(m.r32(q + arr));
    let r = rec(m);
    let header: Vec<u8> = (0..0x18).map(|k| m.r8(q + 8 + k)).collect();
    for (k, b) in header.iter().enumerate() {
        m.w8(r.wrapping_add(k as u32), *b);
    }
    let c = m.r32(q + 0x20);
    m.w32(q + 0x28 + c.wrapping_mul(4), 0);
    let a = at(m, 0x38, 4);
    m.w32(a, 0);
    let b0 = m.r8(rec(m)) as i8 as i32 as u32;
    let a = at(m, 0x3c, 4);
    m.w32(a, b0);
    let a = at(m, 0x40, 4);
    m.w32(a, 0);
    let p = q + 0x44 + m.r32(q + 0x20).wrapping_mul(8);
    for k in 0..8 {
        m.w8(p.wrapping_add(k), 0);
    }
    let a = at(m, 0x68, 4);
    m.w32(a, 0);
    let a = at(m, 0x6c, 4);
    m.w32(a, 0);
    let a = at(m, 0x74, 16);
    m.w32(a.wrapping_add(4), 0);
    let a = at(m, 0x70, 4);
    m.w32(a, 0);
    let a = at(m, 0x64, 4);
    m.w32(a, 0);
    let a = at(m, 0x74, 16);
    m.w32(a, 100);
    let a = at(m, 0x74, 16);
    m.w32(a.wrapping_add(8), 0x5a);
    let a = at(m, 0x74, 16);
    m.w32(a.wrapping_add(0xc), 0x32);
    let a = at(m, 0x78, 4);
    m.w32(a, 0);
    let a = at(m, 0x7c, 4);
    m.w32(a, 1);
    let scale = m.r32(s + 0x60);
    let a = at(m, 0x40, 4);
    m.w32(a, scale);
    let a = at(m, 0x3c, 4);
    m.w32(a, 0);
    let v = m.r32(at(m, 0x3c, 4)) as i32;
    let byte = if v < 0x7f {
        if v < 0 {
            0
        } else {
            m.r8(at(m, 0x3c, 4))
        }
    } else {
        0x7f
    };
    let r = rec(m);
    m.w8(r, byte);
    e.call(m, SND_LOCK, &[]);
    let r = rec(m);
    let (bank, sound) = (m.r32(s + 0xc), m.r32(s + 0x10));
    let h = if kind == 1 {
        e.call(m, PLAY_BANK_SOUND, &[bank, sound, r])
    } else {
        e.call(m, MIDI_PLAY, &[r, bank, sound, u32::MAX])
    };
    m.w32(s + 4, h);
    e.call(m, SND_UNLOCK, &[]);
    let n = m.r32(q + 0x20).wrapping_sub(1);
    m.w32(q + 0x20, n);
}

/// `BankInstance_CullByDistance` 0x20b688 (the instance, the monitor, the distance): a sound with a cull distance
/// stopped beyond it and started again within it; whether it plays
pub fn cull_by_distance(m: &mut Mem, e: &mut dyn SongEnv, s: u32, q: u32, d: F) -> bool {
    let c = f(m, s + 0x8c);
    if c.ee() == 0.0 {
        return true;
    }
    if m.r32(s + 8) != 0 {
        if c.ee() < d.ee() {
            voice_stop(m, e, s);
            return false;
        }
        true
    } else {
        if d.ee() < c.ee() {
            start(m, e, s, q);
            return true;
        }
        false
    }
}

/// `BankInstance_UpdatePositional` 0x20b700 (the instance, the monitor, the time step): a positional sound's
/// distance, cull, Doppler pitch, pan and fall-off volume; the volume (scaled) sent, or a stopping sound freed when
/// done
pub fn update_positional(m: &mut Mem, e: &mut dyn SongEnv, s: u32, mon: u32, dt: F) {
    let kind = m.r32(s);
    if kind == 0 {
        return;
    }
    let mut vol: i32 = 0;
    'volume: {
        if kind == 3 {
            e.call(m, SND_LOCK, &[]);
            let h = m.r32(s + 4);
            let out = e.call_out(m, HANDLE_STATE, &[h], 2);
            e.call(m, SND_UNLOCK, &[]);
            let (r, state) = (out.first().copied().unwrap_or(0) as i32, out.get(1).copied().unwrap_or(0));
            if r <= 0 && state != 2 {
                return;
            }
        }
        if m.r32(mon + 0x8d8) == 0 || m.r32(s + 0x68) == 0 {
            vol = m.r32(s + 0x64) as i32;
            break 'volume;
        }
        let (d2, view) = locate(m, e, mon, s);
        let d = d2.sqrt();
        let plays = m.r32(s + 0x94) != 0 || cull_by_distance(m, e, s, mon, d);
        if plays {
            let last = f(m, s + 0x5c);
            let pitch = if last.ee() == -1.0 || m.r32(mon + 0x8d4) == 0 {
                F(1.0)
            } else {
                doppler_pitch(last - d, dt)
            };
            let pan = view[2];
            set_pan(m, e, s, pan & 0xffff, (pan >> 16) as i16 as i32 as u32);
            set_pitch(m, e, s, cvt_w((pitch * F(4096.0)).ee()) as u32);
            vol = if m.r32(s + 0x94) != 0 {
                m.r32(s + 0x64) as i32
            } else {
                let far = f(m, s + 0x88);
                let far = if far.ee() == 0.0 { F(30.0) } else { far };
                instance_distance_volume(m.r32(s + 0x64), d, far) as i32
            };
            if vol >= 0x80 {
                vol = 0x7f;
            }
        }
        m.w32(s + 0x5c, d.0.to_bits());
        if vol < 0 {
            vol = 0;
        }
    }
    if m.r32(s + 0x54) != 0 {
        if e.call(m, VOICE_IS_DONE, &[s]) != 0 {
            m.w32(s + 8, 0);
            m.w32(s + 4, u32::MAX);
            m.w32(s + 0x54, 0);
        }
        return;
    }
    let scale = m.r32(s + 0x60);
    if scale != 0 {
        vol = cvt_w((F(vol as f32) * f(m, scale)).ee());
    }
    e.call(m, VOICE_SET_VOLUME, &[s, vol as u32]);
}

/// `SndVoice_Pause` 0x20b948: a playing sound's pitch and volume to 0, a MIDI player paused
pub fn pause(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    if is_free(m, e, s) || m.r32(s + 0x54) != 0 {
        return;
    }
    set_pitch(m, e, s, 0);
    e.call(m, VOICE_SET_VOLUME, &[s, 0]);
    if m.r32(s) == 2 {
        let h = m.r32(s + 4);
        e.call(m, MIDI_PAUSE, &[h]);
    }
}

/// `BankInstance_ResetPitch` 0x20b9b0: a sound's pitch back to 1.0, a MIDI player resumed
pub fn resume(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    if is_free(m, e, s) {
        return;
    }
    set_pitch(m, e, s, 0x1000);
    if m.r32(s) == 2 {
        let h = m.r32(s + 4);
        e.call(m, MIDI_RESUME, &[h]);
    }
}

/// `BankInstance_SetPitchBend` 0x21e7e0 (the instance, the bend: 64 none): the handle's voices bent, under the
/// sound library's lock (the rider's loops' updates, `sndqueue::update_air_loop` and the others)
pub fn set_pitch_bend(m: &mut Mem, e: &mut dyn SongEnv, s: u32, bend: u32) {
    let h = m.r32(s + 4);
    locked(m, e, HANDLE_PITCH_BEND, &[h, bend]);
}

/// `BankInstance_Stop` 0x20ba00 (the instance, the volume or −1 to stop, kept, the fade time in seconds): set or
/// faded to the volume; −1 stops (a stream at once)
pub fn stop(m: &mut Mem, e: &mut dyn SongEnv, s: u32, volume: u32, keep: u32, fade: F) {
    if keep != 0 {
        m.w32(s + 0x98, 1);
        m.w32(s + 0x58, 1);
    }
    if m.r32(s + 0x54) != 0 {
        return;
    }
    let mut v = volume as i32;
    if v == -1 {
        m.w32(s + 0x54, 1);
    } else {
        if v >= 0x80 {
            v = 0x7f;
        }
        if v < 0 {
            v = 0;
        }
    }
    let v = v as u32;
    let h = m.r32(s + 4);
    let time = || cvt_w((fade * F(100.0)).ee()) as u32;
    match m.r32(s) as i32 {
        1 => {
            e.call(m, SND_LOCK, &[]);
            if fade.ee() == 0.0 {
                e.call(m, HANDLE_VOLUME, &[h, v]);
            } else {
                e.call(m, HANDLE_FADE, &[h, time(), v]);
            }
            e.call(m, SND_UNLOCK, &[]);
        }
        2 => {
            e.call(m, SND_LOCK, &[]);
            if fade.ee() == 0.0 {
                e.call(m, MIDI_VOLUME, &[h, v]);
            } else {
                e.call(m, MIDI_FADE, &[h, time(), v]);
            }
            e.call(m, SND_UNLOCK, &[]);
        }
        3 => {
            let st = m.r32(s + 0xc);
            if m.r32(s + 0x54) != 0 {
                e.call(m, STREAM_STOP, &[st]);
            } else {
                e.call(m, STREAM_VOLUME, &[st, v]);
            }
        }
        _ => {}
    }
}

fn instance(m: &Mem, mon: u32, k: u32) -> u32 {
    m.r32(mon + 0x8e0).wrapping_add(k * SIZE)
}

/// `BankMonitor_Init` 0x20bb98 (the monitor, the `cMonitor` argument): 64 instances made
pub fn monitor_init(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, arg: u32) -> u32 {
    e.call(m, MONITOR_CTOR, &[mon, arg]);
    m.w32(mon + 0x8d0, 0);
    m.w32(mon + 4, MONITOR_VTABLE);
    m.w32(mon + 0x8d8, 1);
    m.w32(mon + 0x8d4, 1);
    let n = COUNT * SIZE + 0x10;
    let p = e.call(m, EAMEM_ALLOC, &[NAME_INSTANCES, n, 0]);
    if p != 0 {
        e.call(m, EAMEM_SET, &[p, 0xdead_c0ed, n]);
    }
    m.w32(p, COUNT);
    for k in 0..COUNT {
        init(m, p + 0x10 + k * SIZE);
    }
    m.w32(mon + 0x8e0, p + 0x10);
    mon
}

/// `cBankMonitor::~cBankMonitor` 0x20bc60 (the monitor, the delete flags)
pub fn monitor_destroy(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, flags: u32) {
    m.w32(mon + 4, MONITOR_VTABLE);
    let mut p = m.r32(mon + 0x8e0);
    if p != 0 {
        let mut s = m.r32(p.wrapping_sub(0x10)).wrapping_mul(SIZE).wrapping_add(p);
        while p != s {
            s = s.wrapping_sub(SIZE);
            delete_if(m, e, s, 0);
            p = m.r32(mon + 0x8e0);
        }
        e.call(m, MEM_FREE, &[p.wrapping_sub(0x10)]);
    }
    e.call(m, MONITOR_DTOR, &[mon, flags]);
}

/// `BankMonitor_FindFreeInstance` 0x20bce8 (the monitor, n): the n-th free instance, or −1
pub fn find_free(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, n: u32) -> i32 {
    let mut n = n;
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        if is_free(m, e, s) {
            n = n.wrapping_sub(1);
            if n == 0 {
                return k as i32;
            }
        }
    }
    -1
}

/// `BankMonitor_AddInstance` 0x20bd68 (the monitor, the handle, the volume, the engine): a free instance taken for a
/// sound just started from the queue's top record (its volume scale, position, key, fall-off, cull, flags); none
/// free stops the sound. The instance, or 0
pub fn add_instance(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, handle: u32, volume: u32, kind: u32) -> u32 {
    let mut s = 0;
    let n = m.r32(mon + 0x8d0).wrapping_add(1);
    m.w32(mon + 0x8d0, n);
    let k = find_free(m, e, mon, n);
    if k < 0 {
        e.call(m, DEBUG_PRINTF, &[MSG_FULL]);
        match kind {
            1 => {
                e.call(m, HANDLE_STOP, &[handle]);
            }
            2 => {
                e.call(m, MIDI_STOP, &[handle]);
            }
            _ => {}
        }
    } else {
        s = m.r32(mon + 0x8e0).wrapping_add((k as u32).wrapping_mul(SIZE));
        reset(m, e, s);
        let at = |m: &Mem, arr: u32| m.r32(mon + 0x20).wrapping_mul(4).wrapping_add(m.r32(mon + arr));
        m.w32(s + 4, handle);
        m.w32(s + 0x64, volume);
        m.w32(s, kind);
        let v = m.r32(at(m, 0x40));
        m.w32(s + 0x60, v);
        let v = m.r32(mon + 0x28 + m.r32(mon + 0x20).wrapping_mul(4));
        m.w32(s + 0x68, v);
        let key = mon + m.r32(mon + 0x20).wrapping_mul(8);
        let (k0, k1) = (m.r32(key + 0x44), m.r32(key + 0x48));
        m.w32(s + 0x9c, k0);
        m.w32(s + 0xa0, k1);
        let (a, b) = (m.r32(at(m, 0x68)), m.r32(at(m, 0x64)));
        m.w32(s + 0x84, a);
        m.w32(s + 0x80, b);
        if m.r32(at(m, 0x38)) != 0 {
            // (`lq` / `sq`: the 16 bytes at the position's address rounded down to 16)
            let from = m.r32(s + 0x68) & !0xf;
            let words: Vec<u32> = (0..4).map(|k| m.r32(from.wrapping_add(4 * k))).collect();
            m.w32(s + 0x68, s + 0x70);
            let to = (s + 0x70) & !0xf;
            for (k, w) in words.iter().enumerate() {
                m.w32(to + 4 * k as u32, *w);
            }
        }
        let v = m.r32(at(m, 0x6c));
        m.w32(s + 0x88, v);
        let flags = m.r32(at(m, 0x78));
        let cull = m.r32(at(m, 0x70));
        m.w32(s + 0x94, flags);
        let free = m.r32(at(m, 0x7c));
        m.w32(s + 0x8c, cull);
        if m.r32(s + 0x58) == 0 {
            m.w32(s + 0x98, free);
        }
        m.w32(s + 0x90, MINUS_ONE);
        if m.r32(mon + 0x28 + m.r32(mon + 0x20).wrapping_mul(4)) != 0 {
            compute_distance(m, e, s, mon);
        }
        m.w32(s + 8, !handle >> 31);
    }
    let n = m.r32(mon + 0x8d0).wrapping_sub(1);
    m.w32(mon + 0x8d0, n);
    s
}

/// `BankMonitor_PauseAll` 0x20bfe8
pub fn pause_all(m: &mut Mem, e: &mut dyn SongEnv, mon: u32) {
    m.w32(mon + 0x8dc, 1);
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        pause(m, e, s);
    }
}

/// `Audio_StopAllVoices` 0x20c040: every instance resumed (its pitch back), the monitor unpaused
pub fn resume_all(m: &mut Mem, e: &mut dyn SongEnv, mon: u32) {
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        resume(m, e, s);
    }
    m.w32(mon + 0x8dc, 0);
}

/// `SndSys_StopBankSoundSoft` 0x20c098 (the monitor, the handle, the engine, the fade time)
pub fn stop_bank_sound_soft(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, handle: u32, kind: u32, fade: F) {
    let s = find(m, e, mon, handle, kind);
    if s != 0 {
        stop(m, e, s, u32::MAX, 0, fade);
    }
}

/// `BankMonitor_ResetInstance` 0x20c0e0 (the monitor, the index)
pub fn reset_instance(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, k: u32) {
    let s = m.r32(mon + 0x8e0).wrapping_add(k.wrapping_mul(SIZE));
    reset(m, e, s);
}

/// `cBankMonitor::vf5` 0x20c0f8: every instance reset
pub fn reset_all(m: &mut Mem, e: &mut dyn SongEnv, mon: u32) {
    for k in 0..COUNT {
        reset_instance(m, e, mon, k);
    }
}

/// `cBankMonitor::vf6` 0x20c148 (the monitor, a key of eight bytes): every instance whose key matches reset (the
/// halfword at +0x9e against the key's bits 16..31, then the halfword at +0xa0 against bits 32..47 unless that was
/// −1, else the word at +0xa0 against bits 32..63)
pub fn reset_key(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, key: u64) {
    let k16 = (key >> 16) as i16;
    let mut base = m.r32(mon + 0x8e0);
    for k in 0..COUNT {
        let s = base.wrapping_add(k * SIZE);
        let hit = k16 == m.r16(s + 0x9e) as i16
            && ((k16 != -1 && (key >> 32) as i16 == m.r16(s + 0xa0) as i16) || (key >> 32) as u32 == m.r32(s + 0xa0));
        if hit {
            reset(m, e, s);
            base = m.r32(mon + 0x8e0);
        }
    }
}

/// `BankMonitor_Find` 0x20c200 (the monitor, the handle, the engine): the busy instance of that sound, or 0
pub fn find(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, handle: u32, kind: u32) -> u32 {
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        if !is_free(m, e, s) {
            let s = instance(m, mon, k);
            if m.r32(s) == kind && m.r32(s + 4) == handle {
                return s;
            }
        }
    }
    0
}

/// `SndSys_StopBankSound` 0x20c2a0 (the monitor, the handle, the engine, the fade time): at once with no fade, else
/// faded out and kept
pub fn stop_bank_sound(m: &mut Mem, e: &mut dyn SongEnv, mon: u32, handle: u32, kind: u32, fade: F) {
    let s = find(m, e, mon, handle, kind);
    if s == 0 {
        return;
    }
    if fade.ee() == 0.0 {
        reset(m, e, s);
    } else {
        stop(m, e, s, u32::MAX, 1, fade);
    }
}

/// `BankInstances_CountPlaying` 0x20c308
pub fn count_playing(m: &mut Mem, e: &mut dyn SongEnv, mon: u32) -> u32 {
    let mut n = 0;
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        if !is_free(m, e, s) && m.r32(instance(m, mon, k) + 8) != 0 {
            n += 1;
        }
    }
    n
}

/// `BankInstances_CountUsed` 0x20c380
pub fn count_used(m: &mut Mem, e: &mut dyn SongEnv, mon: u32) -> u32 {
    let mut n = 0;
    for k in 0..COUNT {
        let s = instance(m, mon, k);
        if !is_free(m, e, s) {
            n += 1;
        }
    }
    n
}

/// `BankMonitor_ResetTimers` 0x20c3e8: every instance's distance unknown (−1)
pub fn reset_distances(m: &mut Mem, mon: u32) {
    let base = m.r32(mon + 0x8e0);
    for k in (0..COUNT).rev() {
        m.w32(base.wrapping_add(k * SIZE + 0x5c), MINUS_ONE);
    }
}

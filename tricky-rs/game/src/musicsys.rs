//! The music system's song control (board row G10b2b): `cMusicSys` (the stream system at `gMusicSys` 0x343150) starting,
//! stopping and steering its one song (+0x36f0, a [`crate::musicsong`] song) on Pathfinder, its fades, its breaks and
//! air loops, the front end's music events and the song list, on the game's bytes (`pf::Mem`), checked in the function
//! runner (`tests/musicsys.rs`). The song functions are called as ported; Pathfinder, the stream system's scheduler, the
//! banks and the ini reader through [`SongEnv`].
//!
//! The music system: +4 its vtable, +8..+0x20 the stream settings a new stream starts from, +0x20 the stream index,
//! +0x24 the stream records (0x18 each), +0x28.. and the tables at +0x38..+0x7c (by index), +0xa3c the banks (0x60
//! each), +0xa54 the ini reader, +0x1260 the base path, +0x12e0 the song names (0x20 each), +0x36e0 the songs, +0x36e8
//! the song chosen, +0x36ec the enabled songs (bits), +0x36f0 the song, +0x36f4 / +0x36f8 / +0x36fc a fade (its step,
//! the volume now, the target), +0x3700 playing, +0x3704 the air loop's phrase, +0x3708 Tricky on, +0x370c its word,
//! +0x3710 a break asked, +0x3714 the front end's music event.

use crate::musicsong::{self as song, SongEnv};
use crate::pf::Mem;
use crate::ps2f::F;

pub use crate::musicsong::MUSIC_SYS;

/// The vtable `cMusicSys::~cMusicSys` puts back
pub const VTABLE: u32 = 0x3a00b0;
/// The scheduler's events (doublewords at these addresses): the break, the air loop, the third, the fade step
pub const EV_BREAK: u32 = 0x3a0090;
pub const EV_AIR_LOOP: u32 = 0x3a0098;
pub const EV_THIRD: u32 = 0x3a00a0;
pub const EV_FADE: u32 = 0x3a00a8;

pub const SCHEDULE: u32 = 0x222cc0;
pub const CANCEL: u32 = 0x222df0;
pub const MEMSET: u32 = 0x2fb468;
pub const BANK_LOAD: u32 = 0x20a520;
pub const PICK_VARIANT: u32 = 0x224658;
pub const RANDOM_SEGMENT: u32 = 0x2246a8;
pub const INI_CLOSE: u32 = 0x221788;
pub const STREAM_SYS_DTOR: u32 = 0x229948;
/// `SndSys_PlayPending` 0x20c948 (the system): the queued sounds played
pub const PLAY_PENDING: u32 = 0x20c948;
/// A word the air loop's beat reads when a Tricky phrase runs out (clear: Tricky cancelled; set: the loop starts over)
pub const AIR_LOOP_RESTART: u32 = 0x34308c;

fn ev(m: &Mem, at: u32) -> [u32; 2] {
    [m.r32(at), m.r32(at + 4)]
}

fn cancel(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, event: u32) {
    let d = ev(m, event);
    e.call(m, CANCEL, &[sys, d[0], d[1]]);
}

/// `SndSys_ScheduleEvent` 0x222cc0 (the system, a delay in ms, the event, then five words)
fn schedule(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, delay: u32, event: u32, w: [u32; 5]) {
    let d = ev(m, event);
    e.call(m, SCHEDULE, &[sys, delay, d[0], d[1], w[0], w[1], w[2], w[3], w[4]]);
}

fn cur(m: &Mem, sys: u32) -> u32 {
    m.r32(sys + 0x36f0)
}

/// `cMusicSys::~cMusicSys` 0x224c90 (the system, the delete flags): its vtable back, the song deleted, `gMusicSys`
/// cleared, the ini reader closed, then the stream system's destructor
pub fn destroy(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, flags: u32) {
    m.w32(sys + 4, VTABLE);
    let s = cur(m, sys);
    if s != 0 {
        song::delete(m, e, s, 3);
    }
    m.w32(MUSIC_SYS, 0);
    e.call(m, INI_CLOSE, &[sys + 0xa54, 2]);
    e.call(m, STREAM_SYS_DTOR, &[sys, flags]);
}

/// `MusicSys_PlaySong` 0x224e68 (the system, the song's name or 0, its bank, a volume scale's address; `sp` its frame,
/// 0x80 bytes): a new stream index (+0x20 up one) set up from the settings at +8 (its record, its tables cleared,
/// its volume the record's first byte, the defaults 100, 90, 50 and 1), `MusicSys_StartSong`, and the index back
pub fn play_song(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, name: u32, bank: u32, scale: u32, sp: u32) {
    // (in the machine code's order of loads and stores: the store at +0x28 + 4·index reaches the table pointers from
    // +0x38 on for index 4 and up, after they were read)
    m.w32(sys + 0x20, m.r32(sys + 0x20).wrapping_add(1));
    let ix = |m: &Mem| m.r32(sys + 0x20);
    let rec = ix(m).wrapping_mul(0x18).wrapping_add(m.r32(sys + 0x24));
    let copy: Vec<u8> = (0..0x18).map(|k| m.r8(sys + 8 + k)).collect();
    for (k, b) in copy.iter().enumerate() {
        m.w8(rec + k as u32, *b);
    }
    let (t38, t24) = (m.r32(sys + 0x38), m.r32(sys + 0x24));
    let i = ix(m);
    m.w32(sys.wrapping_add(i.wrapping_mul(4)).wrapping_add(0x28), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t38), 0);
    let t3c = m.r32(sys + 0x3c);
    let t40 = m.r32(sys + 0x40);
    let vol = m.r8(i.wrapping_mul(0x18).wrapping_add(t24)) as i8 as i32;
    m.w32(i.wrapping_mul(4).wrapping_add(t3c), vol as u32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t40), 0);
    e.call(m, MEMSET, &[sys.wrapping_add(i.wrapping_mul(8)).wrapping_add(0x44), 0, 8]);
    let i = ix(m);
    let (t68, t74, t6c) = (m.r32(sys + 0x68), m.r32(sys + 0x74), m.r32(sys + 0x6c));
    m.w32(i.wrapping_mul(4).wrapping_add(t68), 0);
    let t70 = m.r32(sys + 0x70);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t6c), 0);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(4), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t70), 0);
    let i = ix(m);
    let (t78, t64, t7c) = (m.r32(sys + 0x78), m.r32(sys + 0x64), m.r32(sys + 0x7c));
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74), 100);
    let i64_at = i.wrapping_mul(4).wrapping_add(t64);
    let t40 = m.r32(sys + 0x40);
    let i = ix(m);
    m.w32(i64_at, 0);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(8), 0x5a);
    let i = ix(m);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(0xc), 0x32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t78), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t7c), 1);
    m.w32(i.wrapping_mul(4).wrapping_add(t40), scale);
    start_song(m, e, sys, name, bank, sp - 0x40);
    m.w32(sys + 0x20, m.r32(sys + 0x20).wrapping_sub(1));
}

/// `MusicSys_StartSong` 0x225090 (the system, the song's name or 0 for the chosen one's, its bank; `sp` its frame,
/// 0x40 bytes): the song stopped; the stream's volume (its table at +0x3c) times its scale when it has one, held to
/// 0..127, and the record's byte from it; the front end's event cleared; the song created (`Music_CreatePathfinder`
/// with the base path, the name, the volume as it was before the scale, the scale, the bank), its bank loaded, playing
pub fn start_song(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, name: u32, bank: u32, sp: u32) {
    stop_song(m, e, sys);
    let i = m.r32(sys + 0x20);
    let tab = m.r32(sys + 0x3c);
    let scale = m.r32(m.r32(sys + 0x40).wrapping_add(i.wrapping_mul(4)));
    let vol = m.r8(i.wrapping_mul(4).wrapping_add(tab));
    if scale != 0 {
        let v = F(vol as i8 as i32 as f32) * F(f32::from_bits(m.r32(scale)));
        let w = if !(v.ee() < 127.0) {
            0x7f
        } else if v.ee() < 0.0 {
            0
        } else {
            (v.ee() as i32) as i8 as i32
        };
        let i = m.r32(sys + 0x20);
        m.w32(i.wrapping_mul(4).wrapping_add(tab), w as u32);
        let rec = m.r32(sys + 0x20).wrapping_mul(0x18).wrapping_add(m.r32(sys + 0x24));
        let at = m.r32(sys + 0x20).wrapping_mul(4).wrapping_add(tab);
        let w = m.r32(at) as i32;
        let b = if 0x7f <= w {
            0x7f
        } else if w < 0 {
            0
        } else {
            m.r8(at)
        };
        m.w8(rec, b);
    }
    m.w32(sys + 0x3714, 0);
    let name = if name != 0 { name } else { sys.wrapping_add(m.r32(sys + 0x36e8) << 5).wrapping_add(0x12e0) };
    let s = song::create(m, e, sys + 0x1260, name, sys, vol, scale, bank, sp - 0x80 - 0x1b0);
    m.w32(sys + 0x36f0, s);
    load_song_bank(m, e, sys, sp - 0x130);
    m.w32(sys + 0x3700, 1);
}

/// `MusicSys_StopCurrent` 0x225240: the song paused
pub fn stop_current(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    let s = cur(m, sys);
    if s != 0 {
        song::pause(m, e, s);
    }
}

/// `Audio_ResumeSong` 0x225260: the song resumed
pub fn resume_song(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    let s = cur(m, sys);
    if s != 0 {
        song::resume(m, e, s);
    }
}

/// `MusicSys_StopSong` 0x225280: its three scheduled events cancelled, the song's bank cancelled, resumed (so the
/// pitch is back), deleted; not playing, no song
pub fn stop_song(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    for ev in [EV_BREAK, EV_AIR_LOOP, EV_THIRD] {
        cancel(m, e, sys, ev);
    }
    let s = cur(m, sys);
    if s != 0 {
        let bank = m.r32(s + 8);
        e.call(m, song::BANK_CANCEL, &[sys, bank]);
    }
    resume_song(m, e, sys);
    let s = cur(m, sys);
    if s != 0 {
        song::delete(m, e, s, 3);
    }
    m.w32(sys + 0x3700, 0);
    m.w32(sys + 0x36f0, 0);
}

/// `Audio_MusicEvent` 0x225318 (the system, the event): unless it is the end (10) when not playing, the event to the
/// song; the end stops it playing
pub fn music_event(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, event: u32) {
    if event == 10 && m.r32(sys + 0x3700) == 0 {
        return;
    }
    let s = cur(m, sys);
    if s != 0 {
        song::music_event(m, e, s, event);
    }
    if event == 10 {
        m.w32(sys + 0x3700, 0);
    }
}

/// `MusicSys_SetSection` 0x225380: the same with the song's section (priority 1)
pub fn set_section(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, event: u32) {
    if event == 10 && m.r32(sys + 0x3700) == 0 {
        return;
    }
    let s = cur(m, sys);
    if s != 0 {
        song::set_section(m, e, s, event);
    }
    if event == 10 {
        m.w32(sys + 0x3700, 0);
    }
}

/// `Music_SetPathVar` 0x2253e8: Pathfinder's variable for the song
pub fn set_path_var(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, var: u32) {
    let s = cur(m, sys);
    if s != 0 {
        song::set_path_var(m, e, s, var);
    }
}

/// 0x225408: the song's track latency
pub fn set_latency(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, latency: u32) {
    let s = cur(m, sys);
    if s != 0 {
        song::set_latency(m, e, s, latency);
    }
}

/// `Audio_SetSongVolume` 0x225428: the song's volume byte
pub fn set_song_volume(m: &mut Mem, sys: u32, volume: u8) {
    let s = cur(m, sys);
    if s != 0 {
        m.w8(s + 0x10, volume);
    }
}

/// 0x225448 (seconds, the system): the song faded out
pub fn fade_out(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, seconds: f32) {
    let s = cur(m, sys);
    if s != 0 {
        song::fade_out(m, e, s, seconds);
    }
}

/// 0x225c30: the song faded out over 4 s
pub fn fade_out_4s(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    fade_out(m, e, sys, 4.0);
}

/// `MusicSys_FadeTo` 0x225468 (seconds, the system, the volume): toward a volume other than the song's own, the fade
/// set (+0x36fc the target, +0x36f8 the volume now, +0x36f4 a twenty-fifth of the way a step over the seconds) and
/// its step scheduled (40 ms, the fade event)
pub fn fade_to(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, seconds: f32, volume: u8) {
    let s = cur(m, sys);
    if s == 0 {
        return;
    }
    let target = volume as i8 as i32;
    let now = m.r8(s + 0x10) as i8 as i32;
    if target == now {
        return;
    }
    let q = target.wrapping_sub(now) / 0x19;
    m.w32(sys + 0x36fc, target as u32);
    m.w32(sys + 0x36f8, (now as f32).to_bits());
    m.w32(sys + 0x36f4, (F(q as f32) / F(seconds)).0.to_bits());
    schedule(m, e, sys, 0x28, EV_FADE, [0; 5]);
}

/// 0x2254f8, the fade's step: the volume now moved by the step; while it has not reached the target (rising or
/// falling) the song's volume byte is it, truncated (0, go on); else the target's (1, done). The song is not checked
pub fn fade_step(m: &mut Mem, sys: u32) -> u32 {
    let step = F(f32::from_bits(m.r32(sys + 0x36f4)));
    let now = F(f32::from_bits(m.r32(sys + 0x36f8))) + step;
    m.w32(sys + 0x36f8, now.0.to_bits());
    let target = F(m.r32(sys + 0x36fc) as i32 as f32);
    let going = (0.0 < step.ee() && now.ee() < target.ee()) || (step.ee() < 0.0 && target.ee() < now.ee());
    let s = cur(m, sys);
    if going {
        let v = F(f32::from_bits(m.r32(sys + 0x36f8))).ee() as i32;
        m.w8(s.wrapping_add(0x10), v as u8);
        0
    } else {
        let b = m.r8(sys + 0x36fc);
        m.w8(s.wrapping_add(0x10), b);
        1
    }
}

/// `MusicSys_CountEnabled` 0x225588: the enabled songs (+0x36ec's bits)
pub fn count_enabled(m: &Mem, sys: u32) -> u32 {
    m.r32(sys + 0x36ec).count_ones()
}

/// `MusicSys_SelectSong` 0x2255c0: from before the first, the `n`+1-th enabled song
pub fn select_song(m: &mut Mem, sys: u32, n: i32) {
    m.w32(sys + 0x36e8, u32::MAX);
    next_enabled(m, sys);
    let mut k = n;
    while 0 < k {
        k -= 1;
        next_enabled(m, sys);
    }
}

fn enabled(m: &Mem, sys: u32) -> bool {
    (m.r32(sys + 0x36ec) as i32 >> (m.r32(sys + 0x36e8) & 31)) & 1 != 0
}

/// `Music_PrevSong` 0x225618: back to the previous enabled song (round from the last of +0x36e0)
pub fn prev_song(m: &mut Mem, sys: u32) {
    loop {
        let v = m.r32(sys + 0x36e8).wrapping_sub(1);
        m.w32(sys + 0x36e8, v);
        if (v as i32) < 0 {
            m.w32(sys + 0x36e8, m.r32(sys + 0x36e0).wrapping_sub(1));
        }
        if enabled(m, sys) {
            break;
        }
    }
}

/// `MusicSys_NextEnabledSong` 0x225660: on to the next enabled song (round to the first)
pub fn next_enabled(m: &mut Mem, sys: u32) {
    loop {
        let v = m.r32(sys + 0x36e8).wrapping_add(1);
        m.w32(sys + 0x36e8, v);
        if m.r32(sys + 0x36e0) as i32 <= v as i32 {
            m.w32(sys + 0x36e8, 0);
        }
        if enabled(m, sys) {
            break;
        }
    }
}

/// `Audio_UpdateSongVolume` 0x2256a0: the song's stream volume
pub fn update_song_volume(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    let s = cur(m, sys);
    if s != 0 {
        song::update_volume(m, e, s);
    }
}

/// `MusicSys_LoadSongBank` 0x2256c0 (the system; `sp` its frame, 0x130 bytes): with LOOPDATA banks (+0xf8) and not
/// just the one already loading (+0xf8 1 with its bank's state 1), a variant picked (`SongBank_PickVariant`) and its
/// bank (base path + the name) loaded into the song's bank (`BankManager_Load`, 0x10)
pub fn load_song_bank(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, sp: u32) {
    let s = cur(m, sys);
    if s == 0 {
        return;
    }
    let n = m.r32(s + 0xf8);
    if n == 0 {
        return;
    }
    let bank = m.r32(s + 8);
    if n == 1 && m.r32(bank.wrapping_mul(0x60).wrapping_add(m.r32(sys + 0xa3c))) == 1 {
        return;
    }
    let k = e.call(m, PICK_VARIANT, &[s]);
    let name = cur(m, sys).wrapping_add((k << 5).wrapping_add(0x58));
    e.call(m, song::SPRINTF, &[sp, song::PATH_FORMAT, sys + 0x1260, name]);
    e.call(m, BANK_LOAD, &[sys, bank, sp, 0x10]);
}

/// `Music_ScheduleBreak` 0x225768 (the system, asked): with a song not already in a break (+0xfc), unless Tricky is on
/// and it was not asked (an ask marks +0x3710), the break: +0xfc set, its event scheduled at once
pub fn schedule_break(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, asked: u32) {
    let s = cur(m, sys);
    if s == 0 || m.r32(s + 0xfc) != 0 {
        return;
    }
    if asked == 0 {
        if m.r32(sys + 0x3708) != 0 {
            return;
        }
    } else {
        m.w32(sys + 0x3710, 1);
    }
    m.w32(s + 0xfc, 1);
    schedule(m, e, sys, 0, EV_BREAK, [0; 5]);
}

/// `Music_CancelBreak` 0x2257d8 (the system, forced): with a song, when forced or Tricky is off, the break and air loop
/// events cancelled and the break over (0x225948)
pub fn cancel_break(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, forced: u32) {
    if cur(m, sys) == 0 {
        return;
    }
    if forced == 0 && m.r32(sys + 0x3708) != 0 {
        return;
    }
    cancel(m, e, sys, EV_BREAK);
    cancel(m, e, sys, EV_AIR_LOOP);
    break_over(m, sys);
}

/// 0x225948: the song's break over (+0xfc cleared; the song is not checked)
pub fn break_over(m: &mut Mem, sys: u32) {
    let s = cur(m, sys);
    m.w32(s.wrapping_add(0xfc), 0);
}

/// 0x225850 (the system, the event): the scheduler's air-loop callback, the system's virtual at +0x74 (this adjusted
/// by +0x70) with the event passed on; on the game's audio object `cBXAudio::vf13` 0x20df08, [`air_loop_beat`]
pub fn virtual_74(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, event: u32) -> u32 {
    let vt = m.r32(sys + 4);
    let f = m.r32(vt + 0x74);
    let this = sys.wrapping_add(m.r16(vt + 0x70) as i16 as u32);
    e.call(m, f, &[this, event])
}

/// The random number generator (`Rand_Fx`'s object, its pointer at this address): a virtual at +0x24 of its vtable
/// (this adjusted by +0x20)
pub const RAND_OBJ: u32 = 0x3436f8;

fn rand(m: &mut Mem, e: &mut dyn SongEnv) -> u32 {
    let o = m.r32(RAND_OBJ);
    let vt = m.r32(o.wrapping_add(4));
    let f = m.r32(vt.wrapping_add(0x24));
    let this = o.wrapping_add(m.r16(vt.wrapping_add(0x20)) as i16 as u32);
    e.call(m, f, &[this])
}

/// 0x224658 (a song): which of its LOOPDATA banks (+0xf8) is loaded, a random number's unsigned remainder by
/// their count (by 0 a divide trap in the game)
pub fn pick_variant(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> u32 {
    let r = rand(m, e);
    r % m.r32(s.wrapping_add(0xf8))
}

/// `Song_RandomSegmentMs` 0x2246a8 (a song, `alt` the air loop's phrase set +0x3704, Tricky +0x3708): the air loop's
/// first program, a phrase times PhraseAlign (+0x118). With Tricky the bank's last phrase (PhrasesPerbank +0x10c − 1);
/// else a random one (unsigned remainders): of the bank's first PhrasesPerbank − 1, or with `alt` of the +0x110
/// phrases after the bank's. A remainder by 0 is a divide trap in the game
pub fn random_segment(m: &mut Mem, e: &mut dyn SongEnv, s: u32, alt: u32, tricky: u32) -> u32 {
    let align = |m: &Mem| m.r32(s.wrapping_add(0x118));
    if tricky != 0 {
        let p = m.r32(s.wrapping_add(0x10c));
        return p.wrapping_sub(1).wrapping_mul(align(m));
    }
    let r = rand(m, e);
    if alt == 0 {
        let n = m.r32(s.wrapping_add(0x10c)).wrapping_sub(1);
        (r % n).wrapping_mul(align(m))
    } else {
        let n = m.r32(s.wrapping_add(0x110));
        (r % n).wrapping_add(m.r32(s.wrapping_add(0x10c))).wrapping_mul(align(m))
    }
}

/// `Music_CancelTricky` 0x21c6c0: +0x6e7c cleared, then `Music_StopTricky`
pub fn cancel_tricky(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    m.w32(sys + 0x6e7c, 0);
    stop_tricky(m, e, sys);
}

/// 0x224a50 (a song, the system): the song's DelayCount, DelayFeedback and DelayTime (+0x11c, +0x120, +0x124) into
/// words 1, 2 and 0 of the current sound's effect row (+0x74, 16 bytes a sound: the echoes `SndSys_PlayPending`
/// schedules), in that order
pub fn song_delay_to_sound(m: &mut Mem, s: u32, sys: u32) {
    let t74 = m.r32(sys + 0x74);
    let row = |m: &Mem| m.r32(sys + 0x20).wrapping_mul(0x10).wrapping_add(t74);
    let at = row(m);
    m.w32(at + 4, m.r32(s.wrapping_add(0x11c)));
    let at = row(m);
    m.w32(at + 8, m.r32(s.wrapping_add(0x120)));
    let at = row(m);
    m.w32(at, m.r32(s.wrapping_add(0x124)));
}

/// A stream table's whole volume held to the byte 0..127 (inlined)
fn volume_byte(v: i32) -> u8 {
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

/// `cBXAudio::vf13` 0x20df08 (the system, the air-loop event; its words +0x18 the song, +0x1c the loop's first
/// program, +0x20 the beat, +0x24 a beat's ms): the air loop's beat. Past the phrase's beats (song +0x114) the loop
/// starts over at beat 0, except with Tricky on, no break asked (+0x3710) and [`AIR_LOOP_RESTART`] clear, when
/// Tricky is cancelled ([`cancel_tricky`]) and nothing plays. Otherwise a sound is queued (the next index, set up as
/// `MusicSys_PlaySong` sets up its stream; the kind the song's bank, the id the program plus the beat brought into the
/// phrase), its volume the system's Tricky volume (+0x6e84) with Tricky on, then scaled by the song's AsyncLevel
/// (+0x130) % (truncated, held to 0..127), the music volume's slot (+0x389c), the song's delay
/// ([`song_delay_to_sound`]); the pending sounds played; the beat up one and the event's period a beat's ms. 0
pub fn air_loop_beat(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, ev: u32) -> u32 {
    let saved = m.r32(ev + 0x24);
    let n0 = m.r32(ev + 0x20) as i32;
    let prog = m.r32(ev + 0x1c);
    let s = m.r32(ev + 0x18);
    let beats = m.r32(s.wrapping_add(0x114)) as i32;
    let level = m.r32(s.wrapping_add(0x130)) as i32;
    let bank = m.r32(s.wrapping_add(8));
    let mut beat = n0;
    if beats <= n0 {
        if m.r32(sys + 0x3708) == 0 {
            m.w32(ev + 0x20, 0);
        } else if m.r32(sys + 0x3710) != 0 || m.r32(AIR_LOOP_RESTART) != 0 {
            m.w32(ev + 0x20, 0);
            m.w32(sys + 0x3710, 0);
        } else {
            cancel_tricky(m, e, sys);
        }
        beat = m.r32(ev + 0x20) as i32;
        if beats <= beat {
            return 0;
        }
    }
    let t24 = m.r32(sys + 0x24);
    while beat < 0 {
        beat = beat.wrapping_add(beats);
    }
    let f20 = F(level as f32);
    let id = prog.wrapping_add(beat as u32);
    // (in the machine code's order of loads and stores, as `play_song`'s)
    m.w32(sys + 0x20, m.r32(sys + 0x20).wrapping_add(1));
    let ix = |m: &Mem| m.r32(sys + 0x20);
    let rec = ix(m).wrapping_mul(0x18).wrapping_add(t24);
    let copy: Vec<u8> = (0..0x18).map(|k| m.r8(sys + 8 + k)).collect();
    for (k, b) in copy.iter().enumerate() {
        m.w8(rec + k as u32, *b);
    }
    let i = ix(m);
    let (t38, t24) = (m.r32(sys + 0x38), m.r32(sys + 0x24));
    m.w32(sys.wrapping_add(i.wrapping_mul(4)).wrapping_add(0x28), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t38), 0);
    let t3c = m.r32(sys + 0x3c);
    let t40 = m.r32(sys + 0x40);
    let vol = m.r8(i.wrapping_mul(0x18).wrapping_add(t24)) as i8 as i32;
    m.w32(i.wrapping_mul(4).wrapping_add(t3c), vol as u32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t40), 0);
    e.call(m, MEMSET, &[sys.wrapping_add(i.wrapping_mul(8)).wrapping_add(0x44), 0, 8]);
    let i0 = ix(m);
    let (t68, t74, t6c) = (m.r32(sys + 0x68), m.r32(sys + 0x74), m.r32(sys + 0x6c));
    m.w32(i0.wrapping_mul(4).wrapping_add(t68), 0);
    let t70 = m.r32(sys + 0x70);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t6c), 0);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(4), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(t70), 0);
    let i = ix(m);
    let (ta40, t78, t7c) = (m.r32(sys + 0xa40), m.r32(sys + 0x78), m.r32(sys + 0x7c));
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74), 100);
    let t64 = m.r32(sys + 0x64);
    let i = ix(m);
    let ta44 = m.r32(sys + 0xa44);
    m.w32(i0.wrapping_mul(4).wrapping_add(t64), 0);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(8), 0x5a);
    let i = ix(m);
    m.w32(i.wrapping_mul(0x10).wrapping_add(t74).wrapping_add(0xc), 0x32);
    let i = ix(m);
    m.w32(i.wrapping_mul(4).wrapping_add(t78), 0);
    m.w32(i.wrapping_mul(4).wrapping_add(ta40), bank);
    m.w32(i.wrapping_mul(4).wrapping_add(t7c), 1);
    let i = ix(m);
    let tricky = m.r32(sys + 0x3708);
    m.w32(i.wrapping_mul(4).wrapping_add(ta44), id);
    if tricky != 0 {
        let i = ix(m);
        let (t3c, v, t24) = (m.r32(sys + 0x3c), m.r32(sys + 0x6e84), m.r32(sys + 0x24));
        m.w32(i.wrapping_mul(4).wrapping_add(t3c), v);
        let i = ix(m);
        let b = volume_byte(m.r32(i.wrapping_mul(4).wrapping_add(t3c)) as i32);
        m.w8(i.wrapping_mul(0x18).wrapping_add(t24), b);
    }
    // the volume scaled by AsyncLevel %: f = level / 100 · 1.0
    let scale = f20 / F(100.0) * F(1.0);
    let i = ix(m);
    let t3c = m.r32(sys + 0x3c);
    let t40 = m.r32(sys + 0x40);
    let store = i.wrapping_mul(4).wrapping_add(t3c);
    let scaled = |m: &Mem| F(m.r32(ix(m).wrapping_mul(4).wrapping_add(t3c)) as i32 as f32) * scale;
    let first = F(m.r32(store) as i32 as f32) * scale;
    m.w32(i.wrapping_mul(4).wrapping_add(t40), sys + 0x389c);
    let v = if first.ee() < 127.0 && scaled(m).ee() < 0.0 {
        0
    } else if scaled(m).ee() < 127.0 {
        scaled(m).ee() as i32 as u32
    } else {
        0x7f
    };
    m.w32(store, v);
    let t24 = m.r32(sys + 0x24);
    let rec = ix(m).wrapping_mul(0x18).wrapping_add(t24);
    let b = volume_byte(m.r32(ix(m).wrapping_mul(4).wrapping_add(t3c)) as i32);
    m.w8(rec, b);
    song_delay_to_sound(m, m.r32(sys + 0x36f0), sys);
    e.call(m, PLAY_PENDING, &[sys]);
    m.w32(ev + 0x20, m.r32(ev + 0x20).wrapping_add(1));
    m.w32(ev + 4, saved);
    0
}

/// `Music_ScheduleAirLoop` 0x225878 (the system, a song): with a song playing, an air loop scheduled at the next beat
/// (the song's ms to it, truncated): the song, the loop program (`Song_RandomSegmentMs` with the phrase +0x3704 and
/// Tricky +0x3708), the next beat's number and a beat's ms (truncated)
pub fn schedule_air_loop(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, s: u32) {
    if cur(m, sys) == 0 {
        return;
    }
    let beat = song::beat_ms(m, s);
    let next = song::ms_to_next_beat(m, e, s);
    let n = song::current_beat(m, e, s).wrapping_add(1);
    let (a, b) = (m.r32(sys + 0x3704), m.r32(sys + 0x3708));
    let prog = e.call(m, RANDOM_SEGMENT, &[s, a, b]);
    let c = cur(m, sys);
    schedule(m, e, sys, F(next).ee() as i32 as u32, EV_AIR_LOOP, [c, prog, n as u32, F(beat).ee() as i32 as u32, 0]);
}

/// `Music_TryScheduleAirLoop` 0x225958: by the song's bank's state: loaded (1) an air loop scheduled (1); 2 not yet
/// (0); else 1. The song is not checked
pub fn try_schedule_air_loop(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) -> u32 {
    let s = cur(m, sys);
    let st = m.r32(m.r32(s.wrapping_add(8)).wrapping_mul(0x60).wrapping_add(m.r32(sys + 0xa3c))) as i32;
    if st == 1 {
        schedule_air_loop(m, e, sys, s);
        return 1;
    }
    if st == 2 { 0 } else { 1 }
}

/// 0x2259d0: the song's bank not loaded (its state 1 with the handle's state 1 is loaded: 0): the break over, the
/// system's virtual at +0x24 (this adjusted by +0x20), and with its answer's low two bits clear the bank loaded again
/// (`sp` that function's frame); 1
pub fn reload_bank(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, sp: u32) -> u32 {
    let bank = m.r32(cur(m, sys).wrapping_add(8));
    let mut ok = false;
    e.call(m, song::SND_LOCK, &[]);
    if m.r32(bank.wrapping_mul(0x60).wrapping_add(m.r32(sys + 0xa3c))) == 1 {
        let h = e.call(m, song::BANK_HANDLE, &[sys, bank]);
        ok = e.call(m, song::SND_HANDLE_STATE, &[h]) == 1;
    }
    e.call(m, song::SND_UNLOCK, &[]);
    if ok {
        return 0;
    }
    let s = cur(m, sys);
    m.w32(s.wrapping_add(0xfc), 0);
    let vt = m.r32(sys + 4);
    let f = m.r32(vt + 0x24);
    let this = sys.wrapping_add(m.r16(vt + 0x20) as i16 as u32);
    if e.call(m, f, &[this]) & 3 == 0 {
        load_song_bank(m, e, sys, sp - 0x130);
    }
    1
}

/// `Music_SetFEEvent0`..`5` 0x225a90..0x225be8 (the event 0..5): the front end's music event, posted when it changes
pub fn set_fe_event(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, event: u32) {
    if m.r32(sys + 0x3714) != event {
        music_event(m, e, sys, event);
        m.w32(sys + 0x3714, event);
    }
}

/// `Music_StartTricky` 0x225c40 (the system, a word): when Tricky is off: the break cancelled (not forced), the word
/// kept (+0x370c), Tricky on, and a break asked
pub fn start_tricky(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, word: u32) {
    if m.r32(sys + 0x3708) != 0 {
        return;
    }
    cancel_break(m, e, sys, 0);
    m.w32(sys + 0x370c, word);
    m.w32(sys + 0x3708, 1);
    schedule_break(m, e, sys, 1);
}

/// `Music_StopTricky` 0x225ca8: when Tricky is on, the break cancelled (forced) and Tricky off
pub fn stop_tricky(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    if m.r32(sys + 0x3708) != 0 {
        cancel_break(m, e, sys, 1);
        m.w32(sys + 0x3708, 0);
    }
}

/// `Music_CountEnabledSongs` 0x225ce0: the enabled songs among the first +0x36e8 (the bits shifted arithmetically)
pub fn count_enabled_songs(m: &Mem, sys: u32) -> u32 {
    let (n, mask) = (m.r32(sys + 0x36e8) as i32, m.r32(sys + 0x36ec) as i32);
    let mut c = 0u32;
    let mut k = 0;
    while k < n {
        c = c.wrapping_add(((mask >> (k & 31)) & 1) as u32);
        k += 1;
    }
    c
}

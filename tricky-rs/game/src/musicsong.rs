//! The music system's song objects on Pathfinder (board row G10b2a): a song (0x7134 bytes) made by
//! `Music_CreatePathfinder` 0x223ca8 / `Music_LoadPathfinder` 0x223d60 from its `MUSIC.INF` section, its graph
//! registered with Pathfinder and its track loaded; then `Song_*` (delete, pause, resume, events, the path
//! variable, the volume, the fade) and the status and timing helpers 0x224768..0x2249c0, on the game's bytes
//! (`pf::Mem`), checked in the function runner (`tests/musicsong.rs`). Pathfinder itself is `crate::pf`; here its
//! functions, the ini reader, the allocator and EA's stream library are calls through [`SongEnv`].
//!
//! A song: +0 its Pathfinder track id (−1 none), +4 the stream buffer (0x37000 bytes), +8 its bank, +0xc a volume
//! scale (a float's address, or 0), +0x10 its volume (a signed byte), +0x14 fading out, +0x18 PATHDATA (the graph's
//! file), +0x38 the stream's file, +0x58 the names (0x20 each, +0xf8 their count), +0xfc, +0x100 the tempo (beats a
//! minute, 120), +0x104 / +0x108 beats a bar and bars (4, 2), +0x10c..+0x130 the section's other numbers, +0x12c its
//! own volume (percent), +0x134 the graph (0x7000 bytes).

use crate::pf::Mem;
use crate::ps2f::F;

/// What the song objects call out to
pub trait SongEnv {
    /// The call to `target` with its integer arguments (a0.., then t0..; a float's bits where it takes one); v0
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32;
    /// The same for a call answering a float (f0)
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32;
    /// A call handed a buffer to fill (its address left out of `args`): the words it leaves there
    fn call_out(&mut self, m: &mut Mem, target: u32, args: &[u32], words: usize) -> Vec<u32>;
}

/// `gMusicSys`, the volume scale every song is played at, the songs' streams (0x3c apart, by Pathfinder's slot)
pub const MUSIC_SYS: u32 = 0x343150;
pub const MASTER_VOLUME: u32 = 0x343058;
pub const STREAMS: u32 = 0x343158;

pub const ALLOC: u32 = 0x2ccf70;
pub const MEMSET: u32 = 0x2cc808;
pub const FREE: u32 = 0x14a040;
pub const DELETE: u32 = 0x114f80;
pub const SPRINTF: u32 = 0x2fca28;
/// `stricmp` (the token against a note's name, case ignored)
pub const STRICMP: u32 = 0x2fcc98;
pub const FMODF: u32 = 0x2f2908;
pub const INI_FIND_SECTION: u32 = 0x221af0;
pub const INI_READ_TOKEN: u32 = 0x221c90;
pub const INI_AT_END: u32 = 0x221e90;
pub const INI_READ_STRING: u32 = 0x221ef8;
pub const INI_READ_FLOAT: u32 = 0x221f70;
pub const INI_READ_INT: u32 = 0x221fd8;
pub const LOAD_FILE: u32 = 0x2c82f0;
pub const PF_REGISTER: u32 = 0x2c0b08;
pub const PF_LOAD_SONG: u32 = 0x2c0cb0;
pub const PF_SET_LATENCY: u32 = 0x2c1130;
pub const PF_SLOT_OF: u32 = 0x2c1168;
pub const PF_PLAYING: u32 = 0x2c1190;
pub const PF_SET_VARIABLE: u32 = 0x2c1288;
pub const PF_POST_EVENT: u32 = 0x2c15c8;
pub const PF_STOP_TRACKS: u32 = 0x2c1a38;
pub const BANK_CANCEL: u32 = 0x209088;
pub const BANK_HANDLE: u32 = 0x20a6b0;
pub const SND_LOCK: u32 = 0x2d0908;
pub const SND_UNLOCK: u32 = 0x2d0938;
pub const SND_HANDLE_STATE: u32 = 0x2d14d0;
pub const STREAM_STATUS: u32 = 0x2d2b08;
pub const HANDLE_STATE: u32 = 0x2d2c10;
pub const STREAM_SET_VOLUME: u32 = 0x2d2e10;
pub const STREAM_FADE: u32 = 0x2d2e50;
pub const STREAM_SET_PITCH: u32 = 0x2d2f10;

/// The ini keys (`MUSIC.INF`'s, at these addresses) the loader reads in this order: PATHDATA and MUSDATA, BPM (a
/// float), then the numbers, and LOOPDATA (one more bank each time it is found)
pub const KEY_PATHDATA: u32 = 0x39ff60;
pub const KEY_STREAM: u32 = 0x39ff70;
pub const KEY_TEMPO: u32 = 0x39ff78;
pub const INT_KEYS: [(u32, u32); 11] = [
    (0x39ff80, 0x104),
    (0x39ff90, 0x108),
    (0x39ffa0, 0x10c),
    (0x39ffb0, 0x114),
    (0x39ffc0, 0x118),
    (0x39ffd0, 0x11c),
    (0x39ffe0, 0x124),
    (0x39fff0, 0x120),
    (0x3a0000, 0x128),
    (0x3a0010, 0x12c),
    (0x3a0020, 0x130),
];
pub const KEY_NAME: u32 = 0x3a0030;
/// The note lengths a token may name ("Sixteenth", "Eighth", "Quarter", "Whole"), each setting +0x124 to that note's
/// milliseconds at the tempo
pub const NOTES: [(u32, u32); 4] = [(0x3a0040, 0x3d80_0000), (0x3a0050, 0x3e00_0000), (0x3a0058, 0x3e80_0000), (0x3a0060, 0x3f80_0000)];
/// "%s%s" (the base path and a file's name)
pub const PATH_FORMAT: u32 = 0x3a0068;

fn f(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

/// `Music_CreatePathfinder` 0x223ca8 (the base path, the section, the music system, the volume byte, the volume scale's
/// address, the bank; `sp` `Music_LoadPathfinder`'s frame, 0x1b0 bytes): a song allocated (0x7134 bytes,
/// 0xdeadc0de) and loaded
#[allow(clippy::too_many_arguments)]
pub fn create(m: &mut Mem, e: &mut dyn SongEnv, base: u32, section: u32, sys: u32, volume: u8, scale: u32, bank: u32, sp: u32) -> u32 {
    let s = e.call(m, ALLOC, &[0x39ff38, 0x7134, 0]);
    if s != 0 {
        e.call(m, MEMSET, &[s, 0xdead_c0de, 0x7134]);
    }
    load(m, e, s, base, section, sys, volume, scale, bank, sp)
}

/// `Music_LoadPathfinder` 0x223d60 (the song, the base path, the section, the music system, the volume byte, the
/// volume scale's address, the bank; `sp` its frame): the song's defaults, its stream buffer (0x37000 bytes,
/// 0xdeadc0ed), its `MUSIC.INF` section read key by key (the ini at the music system +0xa54, from its start) while
/// tokens last, a token naming a note length (case ignored) setting +0x124 to that note's milliseconds at the tempo; the graph
/// (base + PATHDATA) loaded into +0x134 and registered; the music system's volume for this song (its table at +0x3c,
/// entry +0x20) scaled by the song's own percent and held to 0..127, and the track record's byte set from it; then
/// the track loaded (`PathX_LoadSong`: base + the stream's file, latency 500, the record, 2, 0x96, the buffer of
/// 0x37000), its id at +0
#[allow(clippy::too_many_arguments)]
pub fn load(m: &mut Mem, e: &mut dyn SongEnv, s: u32, base: u32, section: u32, sys: u32, volume: u8, scale: u32, bank: u32, sp: u32) -> u32 {
    for o in [4, 0x14, 0xf8, 0xfc, 0x11c] {
        m.w32(s + o, 0);
    }
    m.w32(s + 0x114, 8);
    m.w32(s, u32::MAX);
    m.w32(s + 8, bank);
    m.w32(s + 0xc, scale);
    m.w8(s + 0x10, volume);
    m.w32(s + 0x100, 0x42f0_0000);
    for (o, v) in [(0x10c, 4), (0x110, 2), (0x118, 0x10), (0x120, 0x5a), (0x128, 0x32), (0x130, 100), (0x104, 4), (0x108, 2), (0x124, 100), (0x12c, 100)] {
        m.w32(s + o, v);
    }
    let buf = e.call(m, ALLOC, &[0x39ff48, 0x37000, 0]);
    if buf != 0 {
        e.call(m, MEMSET, &[buf, 0xdead_c0ed, 0x37000]);
    }
    m.w32(s + 4, buf);
    let ini = m.r32(MUSIC_SYS).wrapping_add(0xa54);
    m.w32(ini + 8, m.r32(ini + 4));
    if e.call(m, INI_FIND_SECTION, &[ini, section]) != 0 {
        loop {
            let ini = m.r32(MUSIC_SYS).wrapping_add(0xa54);
            if e.call(m, INI_AT_END, &[ini]) != 0 {
                break;
            }
            let tok = e.call(m, INI_READ_TOKEN, &[ini]);
            let i = m.r32(MUSIC_SYS).wrapping_add(0xa54);
            e.call(m, INI_READ_STRING, &[i, KEY_PATHDATA, s + 0x18]);
            let i = m.r32(MUSIC_SYS).wrapping_add(0xa54);
            e.call(m, INI_READ_STRING, &[i, KEY_STREAM, s + 0x38]);
            let i = m.r32(MUSIC_SYS).wrapping_add(0xa54);
            e.call(m, INI_READ_FLOAT, &[i, KEY_TEMPO, s + 0x100]);
            for (key, o) in INT_KEYS {
                let i = m.r32(MUSIC_SYS).wrapping_add(0xa54);
                e.call(m, INI_READ_INT, &[i, key, s + o]);
            }
            let i = m.r32(MUSIC_SYS).wrapping_add(0xa54);
            let at = s.wrapping_add((m.r32(s + 0xf8) << 5).wrapping_add(0x58));
            if e.call(m, INI_READ_STRING, &[i, KEY_NAME, at]) != 0 {
                m.w32(s + 0xf8, m.r32(s + 0xf8).wrapping_add(1));
            }
            for (name, frac) in NOTES {
                if e.call(m, STRICMP, &[name, tok]) == 0 {
                    let mut v = f(m, s + 0x100) / F(60.0);
                    if frac != 0x3f80_0000 {
                        v = v * F(f32::from_bits(frac));
                    }
                    let v = v * F(1000.0);
                    m.w32(s + 0x124, (v.ee() as i32) as u32);
                }
            }
        }
    }
    let path = sp + 0x10;
    e.call(m, SPRINTF, &[path, PATH_FORMAT, base, s + 0x18]);
    e.call(m, LOAD_FILE, &[path, s + 0x134, 0x7000]);
    e.call(m, PF_REGISTER, &[s + 0x134]);
    e.call(m, SPRINTF, &[path, PATH_FORMAT, base, s + 0x38]);
    // the music system's volume for the song: × its own percent, held to 0..127
    let pct = F(m.r32(s + 0x12c) as i32 as f32) / F(100.0);
    let at = m.r32(sys + 0x3c).wrapping_add(m.r32(sys + 0x20).wrapping_mul(4));
    let v = F(m.r32(at) as i32 as f32) * pct;
    let w = if !(v.ee() < 127.0) {
        0x7f
    } else if v.ee() < 0.0 {
        0
    } else {
        v.ee() as i32
    };
    m.w32(at, w as u32);
    let rec = m.r32(sys + 0x24).wrapping_add(m.r32(sys + 0x20).wrapping_mul(0x18));
    let w = m.r32(at) as i32;
    let b = if 0x7f <= w {
        0x7f
    } else if w < 0 {
        0
    } else {
        m.r8(at)
    };
    m.w8(rec, b);
    let buf = m.r32(s + 4);
    let id = e.call(m, PF_LOAD_SONG, &[0, 0, path, 500, rec, 2, 0x96, buf, 0x37000]);
    m.w32(s, id);
    s
}

/// `Song_Delete` 0x224368 (the song, the delete flags): its bank cancelled, its track stopped (an id of 0 or more),
/// its stream buffer freed, and itself with flag 1
pub fn delete(m: &mut Mem, e: &mut dyn SongEnv, s: u32, flags: u32) {
    let (sys, bank) = (m.r32(MUSIC_SYS), m.r32(s + 8));
    e.call(m, BANK_CANCEL, &[sys, bank]);
    let id = m.r32(s);
    if 0 <= id as i32 {
        e.call(m, PF_STOP_TRACKS, &[id]);
    }
    let buf = m.r32(s + 4);
    if buf != 0 {
        e.call(m, FREE, &[buf]);
    }
    if flags & 1 != 0 {
        e.call(m, DELETE, &[s]);
    }
}

/// 0x224620 (`PathX_GetSongState`'s caller): the song's stream (the word at 0x343158 + 0x3c · its Pathfinder slot)
pub fn stream(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> u32 {
    let id = m.r32(s);
    let slot = e.call(m, PF_SLOT_OF, &[id]);
    m.r32(STREAMS.wrapping_add(slot.wrapping_mul(0x3c)))
}

fn locked_stream_call(m: &mut Mem, e: &mut dyn SongEnv, s: u32, target: u32, arg: u32) {
    e.call(m, SND_LOCK, &[]);
    let st = stream(m, e, s);
    e.call(m, target, &[st, arg]);
    e.call(m, SND_UNLOCK, &[]);
}

/// `Song_Pause` 0x2243e8: the stream's pitch 0
pub fn pause(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    locked_stream_call(m, e, s, STREAM_SET_PITCH, 0);
}

/// `Song_Resume` 0x224420: the stream's pitch 0x1000
pub fn resume(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    locked_stream_call(m, e, s, STREAM_SET_PITCH, 0x1000);
}

/// `Song_MusicEvent` 0x224458 (the song, the event): the event posted to its track, priority 0
pub fn music_event(m: &mut Mem, e: &mut dyn SongEnv, s: u32, event: u32) {
    let id = m.r32(s);
    e.call(m, PF_POST_EVENT, &[id, event as i16 as i32 as u32, 0]);
}

/// `Song_SetSection` 0x224470: the same with priority 1
pub fn set_section(m: &mut Mem, e: &mut dyn SongEnv, s: u32, event: u32) {
    let id = m.r32(s);
    e.call(m, PF_POST_EVENT, &[id, event as i16 as i32 as u32, 1]);
}

/// `Song_SetPathVar` 0x224488 (the song, the variable): Pathfinder's variable for its track
pub fn set_path_var(m: &mut Mem, e: &mut dyn SongEnv, s: u32, var: u32) -> u32 {
    let id = m.r32(s);
    e.call(m, PF_SET_VARIABLE, &[id, var])
}

/// 0x224490 (the song, a latency): its track's latency (`FUN_002c1130`)
pub fn set_latency(m: &mut Mem, e: &mut dyn SongEnv, s: u32, latency: u32) -> u32 {
    let id = m.r32(s);
    e.call(m, PF_SET_LATENCY, &[id, latency])
}

/// `Song_UpdateVolume` 0x224498: unless fading out, the stream's volume: the volume byte × the song's percent (× the
/// scale when it has one) × the master volume (0x343058), held to 0..127
pub fn update_volume(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    if m.r32(s + 0x14) != 0 {
        return;
    }
    let pct = F(m.r32(s + 0x12c) as i32 as f32) / F(100.0);
    let vol = F(m.r8(s + 0x10) as i8 as i32 as f32);
    let scale = m.r32(s + 0xc);
    let v = if scale == 0 { vol * pct } else { vol * pct * f(m, scale) };
    let v = v * f(m, MASTER_VOLUME);
    e.call(m, SND_LOCK, &[]);
    let st = stream(m, e, s);
    let b = if !(v.ee() < 127.0) {
        0x7f
    } else if v.ee() < 0.0 {
        0
    } else {
        (v.ee() as i32) as i8 as i32 as u32
    };
    e.call(m, STREAM_SET_VOLUME, &[st, b]);
    e.call(m, SND_UNLOCK, &[]);
}

/// `Song_FadeOut` 0x2245b8 (seconds, the song): the stream faded out over the milliseconds, +0x14 set
pub fn fade_out(m: &mut Mem, e: &mut dyn SongEnv, s: u32, seconds: f32) {
    e.call(m, SND_LOCK, &[]);
    let st = stream(m, e, s);
    let ms = (F(seconds) * F(1000.0)).ee() as i32;
    e.call(m, STREAM_FADE, &[st, ms as u32, 0]);
    e.call(m, SND_UNLOCK, &[]);
    m.w32(s + 0x14, 1);
}

/// 0x224768: what the song's track is playing (`FUN_002c1190`'s first word)
pub fn playing(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> u32 {
    let id = m.r32(s);
    e.call_out(m, PF_PLAYING, &[id], 1)[0]
}

/// `Song_GetStreamStatus` 0x224790 (the song): the stream's status words (`SndStream_GetStatus`)
pub fn stream_status(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> Vec<u32> {
    let st = stream(m, e, s);
    e.call_out(m, STREAM_STATUS, &[st], 2)
}

/// `Song_GetStatus` 0x2247c0: the state of the stream's handle (its status word 1, `SndHandle_GetState`)
pub fn status(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> Vec<u32> {
    let h = stream_status(m, e, s)[1];
    e.call_out(m, HANDLE_STATE, &[h], 3)
}

/// `Song_GetPlayPosMs` 0x2247f8: the handle's position when it plays (state 2), else 0
pub fn play_pos_ms(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> f32 {
    let st = status(m, e, s);
    if st[0] == 2 { st[1] as i32 as f32 } else { 0.0 }
}

/// `Song_GetLengthMs` 0x224840: the handle's length when it plays, else the song's own (`Song_LengthMs`)
pub fn length_ms(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> f32 {
    let st = status(m, e, s);
    if st[0] == 2 { st[2] as i32 as f32 } else { bars_ms(m, s) }
}

/// `Song_BeatMs` 0x224898: (60 / the tempo) × 1000
pub fn beat_ms(m: &Mem, s: u32) -> f32 {
    (F(60.0) / f(m, s + 0x100) * F(1000.0)).0
}

/// `Song_LengthMs` 0x2248b8: a beat × beats a bar × bars
pub fn bars_ms(m: &Mem, s: u32) -> f32 {
    (F(beat_ms(m, s)) * F(m.r32(s + 0x104) as i32 as f32) * F(m.r32(s + 0x108) as i32 as f32)).0
}

/// `Song_MsToNextBeat` 0x224918: a beat less the position's remainder by a beat (`fmodf`)
pub fn ms_to_next_beat(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> f32 {
    let beat = beat_ms(m, s);
    let pos = play_pos_ms(m, e, s);
    let b = beat_ms(m, s);
    let r = e.call_f(m, FMODF, &[pos.to_bits(), b.to_bits()]);
    (F(beat) - F(r)).0
}

/// `Song_CurrentBeat` 0x224978: the position over a beat, truncated
pub fn current_beat(m: &mut Mem, e: &mut dyn SongEnv, s: u32) -> i32 {
    let pos = play_pos_ms(m, e, s);
    (F(pos) / F(beat_ms(m, s))).ee() as i32
}

/// 0x2249c0 (the song, the bank manager): ready (1) unless it waits on its bank (+0xfc clear) and the bank (the
/// manager's table at +0xa3c, 0x60 each) is loading (1) and its handle's state (0x2d14d0) is not 1
pub fn ready(m: &mut Mem, e: &mut dyn SongEnv, s: u32, banks: u32) -> u32 {
    if m.r32(s + 0xfc) != 0 {
        return 1;
    }
    let bank = m.r32(s + 8);
    let mut ok = false;
    e.call(m, SND_LOCK, &[]);
    if m.r32(bank.wrapping_mul(0x60).wrapping_add(m.r32(banks + 0xa3c))) == 1 {
        let h = e.call(m, BANK_HANDLE, &[banks, bank]);
        ok = e.call(m, SND_HANDLE_STATE, &[h]) == 1;
    }
    e.call(m, SND_UNLOCK, &[]);
    ok as u32
}

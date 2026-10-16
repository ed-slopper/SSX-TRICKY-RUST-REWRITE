//! The music system hosted (board row G10b2c2b): a `cMusicSys` in the engine's memory with the ported layers on it
//! (`musicsys`, `musicsong`), Pathfinder (`pf`) under them, the scheduler (`sndevents`) ticked every 10 ms, and the
//! port's stream driver (`pfplay::Driver`) beneath. `Audio_StartRaceMusic` 0x215220 starts a song as the game does
//! (`MusicSys_PlaySong` with the chosen song, its bank and the music volume's slot; the section 0; the path variable
//! 80).
//!
//! What the layers call out to is answered here ([`HostEnv`]): the allocator (a bump heap), `memset`, `sprintf`
//! ("%s%s"), `stricmp`, the ini reader (`tricky_data::ini::IniReader`, the game's own, over MUSIC.INF; its token
//! buffer kept in memory as the game's is, so the loader's note-length compare sees the latest token), the file
//! loader (the disc), Pathfinder's functions, the stream library (the driver: volume, fade, pitch, status) and the
//! scheduler. The air loop (row G10b2c2c1) runs on the game's code: the break's event, `Music_ScheduleAirLoop`, the
//! air-loop event's callback through the audio object's vtable (`cBXAudio::vf13` 0x20df08, `musicsys::air_loop_beat`)
//! queueing a sound of the LOOPDATA bank a beat, `Song_RandomSegmentMs` and the bank's variant on the game's random
//! number object (answered here from the host's own generator). STANDIN: the bank is not loaded here (its load is
//! marked done and its path kept, [`MusicHost::loop_bank`]) and `SndSys_PlayPending` does not play: the queued sound
//! (bank, program, volume as it makes it) is kept for the app ([`MusicHost::take_loops`]) without the delay's echoes
//! (row G10b2c2c2).

use crate::musicsong::{self as song, SongEnv};
use crate::musicsys as sys;
use crate::pf::{Mem, Pf};
use crate::pfplay::{Driver, GLOBALS_AT, AUTHOR_TEXT};
use crate::sndevents::{self as ev, EventEnv};
use tricky_data::ini::IniReader;

/// Where the host keeps things in the engine's memory
pub const SYS: u32 = 0x0108_0000;
pub const SYS_SIZE: u32 = 0x8000;
pub const TABLES: u32 = 0x010c_0000;
pub const HEAP: u32 = 0x0110_0000;
pub const HEAP_SIZE: u32 = 0x0010_0000;
const TOKEN: u32 = 0x010c_3000;
const SCRATCH: u32 = 0x010c_3100;
const CONSTANTS: u32 = 0x0039_ff00;
const STACK_TOP: u32 = 0x01ff_0000;

/// The strings and doublewords the layers read by address (`MUSIC.INF`'s keys, the note lengths, "%s%s", the
/// scheduler's callbacks)
const STRINGS: [(u32, &str); 22] = [
    (0x39ff38, "SongInstance"), (0x39ff48, "PathfinderStream"), (0x39ff60, "PATHDATA"), (0x39ff70, "MUSDATA"), (0x39ff78, "BPM"),
    (0x39ff80, "BeatsPerMeasure"), (0x39ff90, "MeasuresPerBar"), (0x39ffa0, "PhrasesPerbank"), (0x39ffb0, "BeatsPerPhrase"),
    (0x39ffc0, "PhraseAlign"), (0x39ffd0, "DelayCount"), (0x39ffe0, "DelayTime"), (0x39fff0, "DelayFeedback"), (0x3a0000, "DelayLevel"),
    (0x3a0010, "PathLevel"), (0x3a0020, "AsyncLevel"), (0x3a0030, "LOOPDATA"), (0x3a0040, "Sixteenth"), (0x3a0050, "Eighth"),
    (0x3a0058, "Quarter"), (0x3a0060, "Whole"), (0x3a0068, "%s%s"),
];
const EVENTS: [(u32, u32); 4] = [(sys::EV_BREAK, 0x225958), (sys::EV_AIR_LOOP, 0x225850), (sys::EV_THIRD, 0x2259d0), (sys::EV_FADE, 0x2254f8)];

fn cstr(m: &Mem, at: u32) -> Vec<u8> {
    (0..0x400).map(|k| m.r8(at + k)).take_while(|b| *b != 0).collect()
}

fn put_str(m: &mut Mem, at: u32, s: &[u8]) {
    for (k, b) in s.iter().enumerate() {
        m.w8(at + k as u32, *b);
    }
    m.w8(at + s.len() as u32, 0);
}

fn with_pf<R>(m: &mut Mem, f: impl FnOnce(&mut Pf) -> R) -> R {
    let mut pf = Pf { mem: std::mem::take(m), marker: None };
    let r = f(&mut pf);
    *m = pf.mem;
    r
}

/// The audio object's vtable (its +0x70 / +0x74 the air loop's beat), the random number object and its vtable, and
/// the function the host answers for its random number
const VTABLE: u32 = TABLES + 0x3800;
const RAND_AT: u32 = TABLES + 0x3900;
const HOST_RAND: u32 = 0x007f_0200;

/// A sound of the song's LOOPDATA bank the air loop queued: the bank (the queue's kind, +0xa40), the program (its id,
/// +0xa44) and the volume (0..127) `SndSys_PlayPending` plays it at
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopPlay {
    pub bank: u32,
    pub program: u32,
    pub volume: u8,
}

/// What the host keeps for the app
#[derive(Clone, Debug, Default)]
pub struct HostOut {
    pub loops: Vec<LoopPlay>,
    /// the LOOPDATA bank's file the song asked for (`BankManager_Load`)
    pub bank_path: Option<String>,
}

/// What the layers and the scheduler call out to, answered (see the module)
pub struct HostEnv<'a> {
    pub ini: &'a mut IniReader,
    pub files: &'a mut dyn FnMut(&str) -> Option<Vec<u8>>,
    pub heap: &'a mut u32,
    pub d: &'a mut Driver,
    pub rng: &'a mut u32,
    pub out: &'a mut HostOut,
}

impl HostEnv<'_> {
    fn token(&self, m: &mut Mem) {
        put_str(m, TOKEN, &self.ini.token().to_vec());
    }
}

impl SongEnv for HostEnv<'_> {
    fn call(&mut self, m: &mut Mem, target: u32, a: &[u32]) -> u32 {
        match target {
            song::ALLOC => {
                let at = (*self.heap + 15) & !15;
                if at + a[1] > HEAP + HEAP_SIZE {
                    return 0;
                }
                *self.heap = at + a[1];
                at
            }
            song::MEMSET | sys::MEMSET => {
                for k in 0..a[2] {
                    m.w8(a[0] + k, a[1] as u8);
                }
                a[0]
            }
            song::SPRINTF => {
                let mut s = cstr(m, a[2]);
                s.extend(cstr(m, a[3]));
                put_str(m, a[0], &s);
                s.len() as u32
            }
            song::STRICMP => !cstr(m, a[0]).eq_ignore_ascii_case(&cstr(m, a[1])) as u32,
            song::INI_FIND_SECTION => {
                // (the loader rewinds the reader by its +8 just before)
                self.ini.rewind();
                let found = self.ini.find_section(&cstr(m, a[1]));
                self.token(m);
                found as u32
            }
            song::INI_AT_END => self.ini.at_section_end() as u32,
            song::INI_READ_TOKEN => {
                self.ini.read_token();
                self.token(m);
                TOKEN
            }
            song::INI_READ_STRING => {
                let r = self.ini.read_string(&cstr(m, a[1]));
                self.token(m);
                match r {
                    Some(v) => {
                        put_str(m, a[2], &v);
                        1
                    }
                    None => 0,
                }
            }
            song::INI_READ_INT => {
                let r = self.ini.read_int(&cstr(m, a[1]));
                self.token(m);
                match r {
                    // (no number: the game stores whatever its stack held; nothing here)
                    Some(v) => {
                        if let Some(v) = v {
                            m.w32(a[2], v as u32);
                        }
                        1
                    }
                    None => 0,
                }
            }
            song::INI_READ_FLOAT => {
                let r = self.ini.read_float(&cstr(m, a[1]));
                self.token(m);
                match r {
                    Some(v) => {
                        if let Some(v) = v {
                            m.w32(a[2], v.to_bits());
                        }
                        1
                    }
                    None => 0,
                }
            }
            song::LOAD_FILE => {
                let path = String::from_utf8_lossy(&cstr(m, a[0])).to_string();
                match (self.files)(&path) {
                    Some(b) => {
                        let n = b.len().min(a[2] as usize);
                        for (k, x) in b[..n].iter().enumerate() {
                            m.w8(a[1] + k as u32, *x);
                        }
                        n as u32
                    }
                    None => u32::MAX,
                }
            }
            song::PF_REGISTER => with_pf(m, |pf| pf.register(a[0]) as u32),
            song::PF_LOAD_SONG => {
                let name = cstr(m, a[2]);
                let d = &mut *self.d;
                with_pf(m, |pf| pf.load_song(d, a[0], a[1], a[2], &name, a[3] as u16, a[4], a[5] as i32, a[6], a[7], a[8] as i32) as u32)
            }
            song::PF_SET_LATENCY => with_pf(m, |pf| pf.set_latency(a[0], a[1] as u16) as u32),
            song::PF_SLOT_OF => with_pf(m, |pf| pf.slot_of(a[0]) as u32),
            song::PF_SET_VARIABLE => with_pf(m, |pf| pf.set_variable(a[0], a[1] as u64) as u32),
            song::PF_POST_EVENT => {
                let d = &mut *self.d;
                with_pf(m, |pf| pf.post_event(d, a[0], a[1] as i16, a[2] as i8) as u32)
            }
            song::PF_STOP_TRACKS => {
                let d = &mut *self.d;
                with_pf(m, |pf| pf.stop_tracks(d, a[0]))
            }
            song::STREAM_SET_VOLUME => {
                self.d.volume = a[1] as u8;
                0
            }
            song::STREAM_FADE => {
                self.d.fade_out(a[1] as i32);
                0
            }
            song::STREAM_SET_PITCH => {
                self.d.paused = a[1] == 0;
                0
            }
            sys::SCHEDULE => ev::schedule(m, a[0], a[1], [a[2], a[3]], [a[4], a[5], a[6], a[7], a[8]]) as u32,
            sys::CANCEL => {
                ev::cancel(m, a[0], [a[1], a[2]]);
                0
            }
            0x20df08 => sys::air_loop_beat(m, self, a[0], a[1]),
            sys::RANDOM_SEGMENT => sys::random_segment(m, self, a[0], a[1], a[2]),
            sys::PICK_VARIANT => sys::pick_variant(m, self, a[0]),
            HOST_RAND => {
                let mut x = *self.rng;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *self.rng = x;
                x >> 1
            }
            // STANDIN (row G10b2c2c2): the bank (`BankManager_Load`: the system, the bank, its path) marked loaded
            // (state 1, the handle its number) and its path kept for the app
            sys::BANK_LOAD => {
                let at = a[1].wrapping_mul(0x60).wrapping_add(m.r32(a[0] + 0xa3c));
                m.w32(at, 1);
                m.w32(at + 4, a[1]);
                self.out.bank_path = Some(String::from_utf8_lossy(&cstr(m, a[2])).to_string());
                0
            }
            // STANDIN (row G10b2c2c2): `SndSys_PlayPending` (the system) takes the queue's top sound: its volume times
            // its slot's scale when it has one (0 for an emitter's), held to 0..127; kept for the app, not played
            // (`SND_PlayProgram`) and no echoes scheduled; the index down one
            sys::PLAY_PENDING => {
                let s = a[0];
                let i = m.r32(s + 0x20);
                let w = |m: &Mem, o: u32| m.r32(i.wrapping_mul(4).wrapping_add(m.r32(s + o)));
                let mut v = w(m, 0x3c) as i32;
                if m.r32(s.wrapping_add(0x28).wrapping_add(i.wrapping_mul(4))) != 0 {
                    v = 0;
                } else if w(m, 0x40) != 0 {
                    let f = v as f32 * f32::from_bits(m.r32(w(m, 0x40)));
                    v = if f < 0.0 { 0 } else if 127.0 <= f { 0x7f } else { f as i32 };
                }
                let volume = v.clamp(0, 0x7f) as u8;
                self.out.loops.push(LoopPlay { bank: w(m, 0xa40), program: w(m, 0xa44), volume });
                m.w32(s + 0x20, i.wrapping_sub(1));
                0
            }
            song::BANK_HANDLE | song::SND_HANDLE_STATE => 0,
            _ => 0,
        }
    }
    fn call_f(&mut self, m: &mut Mem, target: u32, a: &[u32]) -> f32 {
        if target == song::FMODF {
            return f32::from_bits(a[0]) % f32::from_bits(a[1]);
        }
        f32::from_bits(SongEnv::call(self, m, target, a))
    }
    fn call_out(&mut self, m: &mut Mem, target: u32, a: &[u32], words: usize) -> Vec<u32> {
        let v = match target {
            song::PF_PLAYING => {
                let d = &mut *self.d;
                with_pf(m, |pf| pf.playing(d, a[0], SCRATCH));
                (0..5).map(|k| m.r32(SCRATCH + 4 * k)).collect()
            }
            // the stream: (requests, the handle playing); a handle: (2 playing, its ms played, its length) or 3 done
            song::STREAM_STATUS => vec![self.d.requests() as u32, self.d.handle_playing() as u32],
            song::HANDLE_STATE => {
                let (state, at, len) = self.d.handle_state(a[0] as i32);
                vec![state, at as u32, len as u32]
            }
            _ => vec![0; 5],
        };
        v.into_iter().chain(std::iter::repeat(0)).take(words.max(1)).collect()
    }
}

impl EventEnv for HostEnv<'_> {
    /// The scheduler's callbacks: the music system's own
    fn call(&mut self, m: &mut Mem, target: u32, this: u32, _event: u32) -> u32 {
        match target {
            0x225958 => sys::try_schedule_air_loop(m, self, this),
            0x225850 => sys::virtual_74(m, self, this, _event),
            0x2259d0 => sys::reload_bank(m, self, this, STACK_TOP - 0x40),
            0x2254f8 => sys::fade_step(m, this),
            _ => 1,
        }
    }
}

/// The music system and what it runs on
pub struct MusicHost {
    pub mem: Mem,
    /// each song's PATHDATA (MUSIC.INF), by its index
    pub paths: Vec<Option<Vec<u8>>>,
    pub ini: IniReader,
    pub files: Box<dyn FnMut(&str) -> Option<Vec<u8>> + Send + Sync>,
    pub heap: u32,
    /// the 10 ms the scheduler has not yet ticked
    pub acc_ms: f32,
    /// the host's random numbers (for the game's `Rand_Fx` object)
    pub rng: u32,
    pub out: HostOut,
}

impl MusicHost {
    /// `MusicSys_Init`'s part the songs need: the base path, the song names and the enabled songs from MUSIC.INF;
    /// the stream index none yet, the stream settings (volume 0x7f), the music volume's slot 1.0, the tables; the
    /// engine's globals (no song selected), `gMusicSys`, the master volume 1.0, and the strings
    pub fn new(inf: &[u8], files: Box<dyn FnMut(&str) -> Option<Vec<u8>> + Send + Sync>) -> MusicHost {
        let mut mem = Mem::default();
        mem.regions.push((GLOBALS_AT, vec![0; 0xb00]));
        mem.regions.push((0x343050, vec![0; 0x700]));
        mem.regions.push((CONSTANTS, vec![0; 0x200]));
        mem.regions.push((SYS, vec![0; SYS_SIZE as usize]));
        mem.regions.push((TABLES, vec![0; 0x4000]));
        mem.regions.push((HEAP, vec![0; HEAP_SIZE as usize]));
        // the frames the layers keep their paths in (below the stack top the calls are given)
        mem.regions.push((STACK_TOP - 0x4000, vec![0; 0x4000]));
        mem.w32(crate::pf::AUTHOR, AUTHOR_TEXT);
        mem.w8(crate::pf::GLOBALS + 0xb, 0xff);
        for (at, s) in STRINGS {
            put_str(&mut mem, at, s.as_bytes());
        }
        for (at, f) in EVENTS {
            mem.w32(at, 0xffff_0000);
            mem.w32(at + 4, f);
        }
        let table = tricky_data::music::music_table(inf);
        put_str(&mut mem, SYS + 0x1260, &table.base);
        for (k, s) in table.songs.iter().enumerate().take(32) {
            put_str(&mut mem, SYS + 0x12e0 + 0x20 * k as u32, &s.name);
        }
        mem.w32(SYS + 0x36e0, table.songs.len() as u32);
        mem.w32(SYS + 0x36ec, table.mask);
        mem.w32(SYS + 0x20, u32::MAX);
        mem.w8(SYS + 8, 0x7f);
        mem.w32(SYS + 0x385c, 1.0f32.to_bits());
        // the air loop's sounds' volume slot (+0x389c) 1.0 too
        mem.w32(SYS + 0x389c, 1.0f32.to_bits());
        for (k, o) in [0x24u32, 0x38, 0x3c, 0x40, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x7c].iter().enumerate() {
            mem.w32(SYS + o, TABLES + 0x200 * k as u32);
        }
        mem.w32(SYS + 0xa3c, TABLES + 0x2000);
        mem.w32(SYS + 0xa40, TABLES + 0x1600);
        mem.w32(SYS + 0xa44, TABLES + 0x1800);
        // the scheduler's events free, as the monitor's constructor makes them (`SndEvent_Init`)
        for i in 0..ev::COUNT {
            ev::event_init(&mut mem, SYS + ev::EVENTS + ev::EVENT_SIZE * i);
        }
        // the audio object's vtable: +0x74 `cBXAudio::vf13`, its adjust (+0x70) 0
        mem.w32(SYS + 4, VTABLE);
        mem.w32(VTABLE + 0x74, 0x20df08);
        // the random number object
        mem.w32(sys::RAND_OBJ, RAND_AT);
        mem.w32(RAND_AT + 4, RAND_AT + 0x40);
        mem.w32(RAND_AT + 0x40 + 0x24, HOST_RAND);
        mem.w32(song::MUSIC_SYS, SYS);
        mem.w32(song::MASTER_VOLUME, 1.0f32.to_bits());
        let paths = table.songs.iter().map(|s| s.path_data.clone()).collect();
        MusicHost { mem, paths, ini: IniReader::new(inf), files, heap: HEAP, acc_ms: 0.0, rng: 0x2468_ace1, out: HostOut::default() }
    }

    fn env<'a>(&'a mut self, d: &'a mut Driver) -> (&'a mut Mem, HostEnv<'a>) {
        let MusicHost { mem, ini, files, heap, rng, out, .. } = self;
        let files: &mut dyn FnMut(&str) -> Option<Vec<u8>> = files.as_mut();
        (mem, HostEnv { ini, files, heap, d, rng, out })
    }

    /// The song of MUSIC.INF named `section` (its index), if there is one
    pub fn song_index(&self, section: &[u8]) -> Option<u32> {
        let n = self.mem.r32(SYS + 0x36e0);
        (0..n).find(|&k| cstr(&self.mem, SYS + 0x12e0 + 0x20 * k).eq_ignore_ascii_case(section))
    }

    /// The song whose PATHDATA is `file` (any case), if there is one
    pub fn song_index_by_data(&self, file: &[u8]) -> Option<u32> {
        self.paths.iter().position(|p| p.as_ref().is_some_and(|p| p.eq_ignore_ascii_case(file))).map(|k| k as u32)
    }

    /// `Audio_StartRaceMusic` 0x215220 with the song chosen (+0x36e8): `MusicSys_PlaySong` (no name: the chosen one;
    /// the bank the enabled songs' count + 8; the music volume's slot, `SndMgr_GetSlot` 0x211f38 with 1), the section
    /// 0 and the path variable 80
    pub fn start_race_music(&mut self, d: &mut Driver, index: u32) {
        self.mem.w32(SYS + 0x36e8, index);
        let (m, mut e) = self.env(d);
        let n = sys::count_enabled_songs(m, SYS);
        let slot = SYS + 0x3850 + 0xc;
        sys::play_song(m, &mut e, SYS, 0, n.wrapping_add(8), slot, STACK_TOP - 0x80);
        sys::set_section(m, &mut e, SYS, 0);
        sys::set_path_var(m, &mut e, SYS, 0x50);
    }

    /// The driver's segments by the offsets Pathfinder will ask for, from the loaded song's graph (`segments` by
    /// segment number: frames in the decoded audio)
    pub fn map_segments(&mut self, d: &mut Driver, segments: &std::collections::HashMap<i16, (usize, usize)>) {
        with_pf(&mut self.mem, |pf| {
            if 0 <= pf.select(0) {
                for n in 0..pf.node_count() {
                    let s = pf.node_segment(n);
                    if 0 < s {
                        if let Some(&f) = segments.get(&s) {
                            d.segments.insert(pf.segment_offset(n as i64), f);
                        }
                    }
                }
            }
            pf.mem.w32(crate::pf::GLOBALS, 0);
            pf.mem.w8(crate::pf::GLOBALS + 0xb, 0xff);
        });
    }

    /// The song, for the driver's segment map: its Pathfinder track id (+0), or none
    pub fn song(&self) -> Option<u32> {
        let s = self.mem.r32(SYS + 0x36f0);
        (s != 0).then_some(s)
    }

    /// `Music_SetPathVar`
    pub fn set_path_var(&mut self, d: &mut Driver, var: u8) {
        let (m, mut e) = self.env(d);
        sys::set_path_var(m, &mut e, SYS, var as u32);
    }

    /// `Audio_MusicEvent`
    pub fn music_event(&mut self, d: &mut Driver, event: u32) {
        let (m, mut e) = self.env(d);
        sys::music_event(m, &mut e, SYS, event);
    }

    /// `Music_ScheduleBreak` (asked: as the Tricky trigger asks it)
    pub fn schedule_break(&mut self, d: &mut Driver, asked: bool) {
        let (m, mut e) = self.env(d);
        sys::schedule_break(m, &mut e, SYS, asked as u32);
    }

    /// `Music_CancelBreak` (forced: even with Tricky on)
    pub fn cancel_break(&mut self, d: &mut Driver, forced: bool) {
        let (m, mut e) = self.env(d);
        sys::cancel_break(m, &mut e, SYS, forced as u32);
    }

    /// `Music_StartTricky`
    pub fn start_tricky(&mut self, d: &mut Driver, word: u32) {
        let (m, mut e) = self.env(d);
        sys::start_tricky(m, &mut e, SYS, word);
    }

    /// The air loop's sounds queued since the last call
    pub fn take_loops(&mut self) -> Vec<LoopPlay> {
        std::mem::take(&mut self.out.loops)
    }

    /// The song's LOOPDATA bank file, once asked for
    pub fn loop_bank(&self) -> Option<&str> {
        self.out.bank_path.as_deref()
    }

    /// `MusicSys_StopSong`
    pub fn stop(&mut self, d: &mut Driver) {
        let (m, mut e) = self.env(d);
        sys::stop_song(m, &mut e, SYS);
    }

    /// A frame of `dt` seconds: `Audio_UpdateSongVolume`, the scheduler's 10 ms ticks, and Pathfinder's update
    pub fn frame(&mut self, d: &mut Driver, dt: f32) {
        self.acc_ms += dt * 1000.0;
        let (m, mut e) = self.env(d);
        sys::update_song_volume(m, &mut e, SYS);
        let ticks = (self.acc_ms / 10.0) as u32;
        self.acc_ms -= ticks as f32 * 10.0;
        let (m, mut e) = self.env(d);
        for _ in 0..ticks {
            ev::tick(m, &mut e, SYS);
        }
        let d2 = &mut *e.d;
        with_pf(m, |pf| pf.update(d2));
    }
}

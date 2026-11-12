//! The speech system hosted (board row E1b2g7d3b2): a `cSpeechSys` in the engine's memory (`pf::Mem`) with the ported
//! layers on it: the glue (`speechsys`), the bank loader (`speechload`), the library (`speechlib`, `speechplay`). It
//! is set up as `Audio_LoadInGameConfig` 0x211098 does (`Speech_InitLibrary`; slot 1 the riders, stream 3, the
//! CHARSPCH banks and ANIM events; slot 0 the announcer, stream 0, following stream 2, the NARR banks; the MC events),
//! a posted event (`SpeechEvt_Post…`: the 0x50-byte record, `SpeechLib_PostEvent`) is kept by the library, and
//! `SpeechSys_UpdateAll` 0x228998 (from `Audio_UpdateSystems`) starts it on an idle slot: the library picks the line
//! and its callback 0x227b00 plays it through the audio object's vtable (+0x7c, `cBXAudio::vf14` 0x20e430).
//!
//! What the layers call out to is answered here by address ([`HostState::dispatch`]): every library function by its
//! port, the allocator (a bump heap), the C library (`memset`, `sprintf`, `stricmp`, `strlen`, `strcpy`, `strncpy`),
//! the ini reader (`tricky_data::ini::IniReader` over SPEECH.INF), the file loader (the disc) and the BIG directory
//! walk (`BIG_Lookup` 0x2cbdc0, on the archive in memory).
//!
//! STANDIN: the stream system (`cStreamSys`, `SndSys_OpenStream` 0x229ae8 and the `Stream_*` calls 0x228fb0..0x22a1a0)
//! is the host's: a stream is a state the app drives ([`SpeechHost::line_started`], [`SpeechHost::frame`]), and
//! `cBXAudio::vf14`'s play is kept as a [`LinePlay`] (the file, the offset) for the app to decode and play.
//! STANDIN: `Rand_Fx` (`cBXAudio::vf3` 0x21e6c0, the library's seed) is the host's own generator.

use crate::pf::Mem;
use crate::speechlib::{self as lib, Calls, Mem as LibMem};
use crate::speechload::{self as load, LoadEnv};
use crate::speechplay as play;
use crate::speechsys::{self as ss, SysEnv};
use tricky_data::ini::IniReader;

/// Where the host keeps things in the engine's memory
pub const SYS_AT: u32 = 0x0108_0000;
const SYS_SIZE: u32 = 0x8000;
const TABLES: u32 = 0x010c_0000;
const TOKEN: u32 = TABLES;
const INF_PATH: u32 = TABLES + 0x100;
const EVENT: u32 = TABLES + 0x200;
const STREAMS: u32 = TABLES + 0x400;
const STREAM_TABLE: u32 = TABLES + 0x600;
const VOLUME_SLOTS: u32 = TABLES + 0x700;
const HEAP: u32 = 0x0110_0000;
const HEAP_SIZE: u32 = 0x0040_0000;
const STACK_BASE: u32 = 0x01e0_0000;
const STACK_TOP: u32 = 0x0200_0000;

/// `cBXAudio`'s vtable, its two entries the speech system calls (+0x24 `vf3`, +0x7c `vf14`), as in the executable
const AUDIO_VTABLE: u32 = 0x39f200;
const RAND_FX: u32 = 0x21e6c0;
const PLAY_FILE: u32 = 0x20e430;
/// The library's play callback (`Speech_InitLibrary` hands it to `SpeechLib_Init`)
const PLAY_CALLBACK: u32 = 0x227b00;
const PRINTF: u32 = 0x188990;

/// The strings and the doubleword the layers read by address (from the executable)
const STRINGS: [(u32, &str); 16] = [
    (0x39ec00, "CHARSPCH"), (0x39ec10, "ANIM"), (0x39ec28, "NARR"), (0x39ec40, "MC"), (0x3a03b0, "%s.hdr"),
    (0x3a03b8, "SpeechBank"), (0x3a03c8, "BIGFILE"), (0x3a03d0, "BASEPATH"), (0x3a03e0, "%sheaders.big"),
    (0x3a03f0, "%seventdat\\events.evt"), (0x3a0408, "%s%s"), (0x3a0410, "BANK"), (0x3a0418, "%s.dat"),
    (0x3a0428, "mSpeechInstance"), (0x3a0438, "SPCH_Init: rNum=%d\n"), (0x3a0450, "bank mem"),
];

/// A line the speech system started: the stream it plays on (0 the announcer's, 3 the riders'), the file
/// (`"<bank's name>.dat"`, a path in the mounted archives) and the offset of the line in it (the library's row:
/// its threshold plus the entry's step)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinePlay {
    pub stream: u32,
    pub file: String,
    pub offset: u32,
    /// the play call's handle
    pub handle: u32,
}

/// A stream as the host keeps it (STANDIN: for the game's `cStream`)
#[derive(Clone, Copy, Debug, Default)]
struct Stream {
    open: bool,
    playing: bool,
    /// the line's length and how far it has played, in ms (the app's)
    length_ms: f32,
    played_ms: f32,
}

/// What the layers call out to (see the module)
pub struct HostState {
    ini: Option<IniReader>,
    inf: Vec<u8>,
    files: Box<dyn FnMut(&str) -> Option<Vec<u8>> + Send + Sync>,
    heap: u32,
    sp: u32,
    rng: u32,
    streams: [Stream; 8],
    plays: Vec<LinePlay>,
    handle: u32,
}

fn cstr(m: &Mem, at: u32) -> Vec<u8> {
    (0..0x400).map(|k| m.r8(at + k)).take_while(|b| *b != 0).collect()
}

fn put_str(m: &mut Mem, at: u32, s: &[u8]) {
    for (k, b) in s.iter().enumerate() {
        m.w8(at + k as u32, *b);
    }
    m.w8(at + s.len() as u32, 0);
}

/// `sprintf` 0x2fca28 for the formats the layers use (`%s`, `%d`)
fn sprintf(m: &mut Mem, out: u32, fmt: u32, args: &[u32]) -> u32 {
    let f = cstr(m, fmt);
    let mut s = Vec::new();
    let mut k = 0;
    let mut i = 0;
    while i < f.len() {
        if f[i] == b'%' && i + 1 < f.len() {
            match f[i + 1] {
                b's' => s.extend(cstr(m, args[k])),
                b'd' => s.extend((args[k] as i32).to_string().bytes()),
                c => panic!("speech host: sprintf %{}", c as char),
            }
            k += 1;
            i += 2;
        } else {
            s.push(f[i]);
            i += 1;
        }
    }
    put_str(m, out, &s);
    s.len() as u32
}

/// `BIG_Lookup` 0x2cbdc0 on a `BIGF` archive held in memory at `h` (the only kind the speech archives are): the entry
/// named `name` (`stricmp`) or, with none, the entry number `index`; its name's address and its offset
fn big_lookup(m: &Mem, h: u32, name: Option<&[u8]>, index: u32) -> Option<(u32, u32)> {
    let be = |at: u32| (0..4).fold(0u32, |v, k| v << 8 | m.r8(at + k) as u32);
    if cstr(m, h).get(..4) != Some(b"BIGF") {
        return None;
    }
    let end = h + be(h + 0xc);
    let mut p = h + 0x10;
    let mut i = 0;
    while p < end {
        let at = p + 8;
        let n = cstr(m, at);
        let hit = match name {
            None => i == index,
            Some(s) => n.eq_ignore_ascii_case(s),
        };
        if hit {
            return Some((at, be(p)));
        }
        i += 1;
        p = at + n.len() as u32 + 1;
    }
    None
}

impl HostState {
    fn alloc(&mut self, size: u32) -> u32 {
        let at = (self.heap + 15) & !15;
        if at + size > HEAP + HEAP_SIZE {
            return 0;
        }
        self.heap = at + size;
        at
    }

    /// A frame for a called function that takes one (its own `sp`), below the caller's and the space its inner
    /// frames may use
    fn frame<R>(&mut self, f: impl FnOnce(&mut Self, u32) -> R) -> R {
        let saved = self.sp;
        let sp = saved - 0x2000;
        assert!(STACK_BASE < sp - 0x2000, "speech host: stack overflow");
        self.sp = sp - 0x2000;
        let r = f(self, sp);
        self.sp = saved;
        r
    }

    fn ini(&mut self) -> &mut IniReader {
        self.ini.as_mut().expect("speech host: no ini open")
    }

    fn token(&mut self, m: &mut Mem) {
        let t = self.ini().token().to_vec();
        put_str(m, TOKEN, &t);
    }

    fn stream(&mut self, s: u32) -> &mut Stream {
        &mut self.streams[((s - STREAMS) / 0x20) as usize]
    }

    /// The call to the function at `t` with the arguments `a`, answered (see the module)
    pub fn dispatch(&mut self, m: &mut Mem, t: u32, a: &[u32]) -> u32 {
        let s32 = |k: usize| a[k] as i32;
        match t {
            // the C library and the allocator
            ss::ALLOC => self.alloc(a[1]),
            ss::FREE | ss::DELETE => 0,
            ss::MEMSET => {
                for k in 0..a[2] {
                    m.w8(a[0] + k, a[1] as u8);
                }
                a[0]
            }
            ss::SPRINTF => sprintf(m, a[0], a[1], &a[2..]),
            load::STRCMP => !cstr(m, a[0]).eq_ignore_ascii_case(&cstr(m, a[1])) as u32,
            load::STRLEN => cstr(m, a[0]).len() as u32,
            ss::STRCPY => {
                let s = cstr(m, a[1]);
                put_str(m, a[0], &s);
                a[0]
            }
            load::STRNCPY => {
                let s = cstr(m, a[1]);
                for k in 0..a[2] {
                    m.w8(a[0] + k, s.get(k as usize).copied().unwrap_or(0));
                }
                a[0]
            }
            PRINTF => 0,
            // the ini reader (SPEECH.INF; the game's path word at 0x3436fc)
            load::INI_OPEN => {
                self.ini = Some(IniReader::new(&self.inf));
                1
            }
            load::INI_CLOSE => {
                self.ini = None;
                0
            }
            load::FIND_SECTION => {
                let s = cstr(m, a[1]);
                let found = self.ini().find_section_matching(&s);
                self.token(m);
                found as u32
            }
            load::SECTION_START => {
                self.ini().skip_to_next_section();
                0
            }
            load::AT_END => self.ini().at_section_end() as u32,
            load::READ_TOKEN => {
                self.ini().read_token();
                self.token(m);
                TOKEN
            }
            load::READ_STRING => {
                let key = cstr(m, a[1]);
                let r = self.ini().read_string(&key);
                self.token(m);
                match r {
                    Some(v) => {
                        put_str(m, a[2], &v);
                        1
                    }
                    None => 0,
                }
            }
            // the files and the archives
            load::OPEN_BIG | ss::FILE_LOAD => {
                let path = String::from_utf8_lossy(&cstr(m, a[0])).into_owned();
                let Some(bytes) = (self.files)(&path) else { return 0 };
                let at = self.alloc(bytes.len() as u32);
                assert!(at != 0, "speech host: heap full");
                for (k, b) in bytes.iter().enumerate() {
                    m.w8(at + k as u32, *b);
                }
                at
            }
            ss::FILE_EXISTS => {
                let path = String::from_utf8_lossy(&cstr(m, a[0])).into_owned();
                (self.files)(&path).is_some() as u32
            }
            load::BIG_FIND => {
                if a[1] == 0 {
                    return 0;
                }
                let name = cstr(m, a[1]);
                match big_lookup(m, a[0], Some(&name), 0) {
                    Some((_, off)) if off != 0 => a[0] + off,
                    _ => 0,
                }
            }
            load::BIG_COUNT => {
                let mut n = 0;
                while big_lookup(m, a[0], None, n).is_some() {
                    n += 1;
                }
                n
            }
            load::BIG_LOOKUP => {
                assert!(a[1] == 0, "speech host: BIG_Lookup by name");
                let r = big_lookup(m, a[0], None, a[2]);
                if a[3] != 0 {
                    m.w32(a[3], r.map_or(0, |r| r.1));
                }
                r.map_or(0, |r| r.0)
            }
            // the glue
            load::REGISTER => lib::register_event_bank(m, a[0], s32(1) as i64 as u64),
            load::LOAD_BANK => {
                self.frame(|e, sp| ss::load_bank(m, e, a[0], a[1], a[2], sp));
                0
            }
            ss::LOAD_EVENTS => {
                self.frame(|e, sp| load::load_events(m, e, sp, a[0], a[1], a[2]));
                0
            }
            PLAY_CALLBACK => self.frame(|e, sp| ss::lib_play_callback(m, e, a[0], sp)) as u32,
            // the stream system (STANDIN, see the module)
            ss::MUSIC_INIT => {
                m.w32(a[0] + 0xa50, STREAM_TABLE);
                0
            }
            ss::STREAM_OPEN => {
                let s = STREAMS + 0x20 * a[1];
                *self.stream(s) = Stream { open: true, ..Stream::default() };
                m.w32(STREAM_TABLE + 4 * a[1], s);
                s
            }
            ss::STREAM_STATE => self.stream(a[0]).playing as u32,
            ss::STREAM_PLAYING => self.stream(a[0]).playing as u32,
            ss::STREAM_WAIT => {
                let st = *self.stream(a[0]);
                (st.playing && st.played_ms >= a[1] as f32) as u32
            }
            ss::STREAM_STOP => {
                let s = m.r32(STREAM_TABLE + 4 * a[1]);
                if s != 0 {
                    self.stream(s).playing = false;
                }
                0
            }
            RAND_FX => {
                self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                self.rng >> 1
            }
            PLAY_FILE => {
                let file = String::from_utf8_lossy(&cstr(m, a[2])).into_owned();
                let s = m.r32(STREAM_TABLE + 4 * a[1]);
                *self.stream(s) = Stream { open: true, playing: true, length_ms: f32::INFINITY, played_ms: 0.0 };
                self.handle += 1;
                self.plays.push(LinePlay { stream: a[1], file, offset: a[3], handle: self.handle });
                self.handle
            }
            // the library
            play::CLOCK => {
                assert!(m.r32(0x348e00) == 0, "speech host: a clock hook");
                0
            }
            play::FREE_VOICE => {
                lib::free_voice(m, a[0]);
                0
            }
            play::FIND_EVENT_BANK => match lib::find_event_bank(m, a[0]) {
                Some(slot) => {
                    m.w32(a[1], slot);
                    1
                }
                None => 0,
            },
            play::FIND_EVENT => lib::find_event(m, a[0]),
            play::KIND => match lib::event_bank_kind(m, a[0]) {
                Some(v) => {
                    m.w32(a[1], v);
                    1
                }
                None => 0,
            },
            play::EVENT_WORD => match lib::event_word(m, a[0]) {
                Some(v) => {
                    m.w32(a[1], v);
                    1
                }
                None => 0,
            },
            play::BANK_SIZE => lib::event_bank_size(m, a[0]) as u32,
            play::BANK_ID => lib::find_bank_by_id(m, a[0] as i16) as u32,
            play::BANK_SLOT => lib::find_bank_by_ids(m, a[0] as i16, a[1] as i16) as u32,
            play::BANK_HAS => lib::bank_has_sound(m, s32(0) as i64, s32(1) as i64 as u64) as u32,
            play::TEST_BIT => lib::test_bit(m, a[0], s32(1)),
            play::CONTAINS => play::list_contains(m, a[0], a[1]) as u32,
            play::CLEAR_PAIR => {
                play::clear_pair(m, a[0]);
                0
            }
            play::GROUP_LIMITS => lib::group_limits_ok(m, s32(0)) as u32,
            play::CLEAR_QUEUE => {
                lib::clear_queue(m);
                0
            }
            play::QUEUED_NOW => lib::has_queued(m, s32(0)) as u32,
            play::CALLBACK_CHECK => play::callback_check(m, self, a[0]),
            play::CLAIM => {
                play::claim_voice(m, self, a[0]);
                0
            }
            // `SpeechLib_ClearIdCounts` 0x2badc0: the eight (id, count) pairs at 0x3494b0 to (0xffff, 0)
            play::RELEASE_TAIL => {
                for k in 0..8 {
                    m.w16(0x3494b0 + 8 * k, 0xffff);
                    m.w32(0x3494b4 + 8 * k, 0);
                }
                0
            }
            play::FIND_VICTIM => self.frame(|e, sp| play::find_victim(m, e, sp, a[0], a[1])) as u32,
            play::PICK_VOICE => self.frame(|e, sp| play::pick_voice(m, e, sp, a[0])) as u32,
            play::START_ONE => self.frame(|e, sp| play::start_entry(m, e, sp, a[0])),
            play::QUEUE_RUN => self.frame(|e, sp| play::queue_run(m, e, sp, a[0])),
            play::START_VOICE => self.frame(|e, sp| play::start_voice(m, e, sp, a[0])),
            play::TRY_START => play::try_start(m, self, a[0]),
            play::POST_EVENT => lib::post_event(m, self, a[0]),
            // `SpeechLib_Init`'s calls
            0x2baed0 => {
                for o in [0, 4, 8] {
                    m.w32(0x348de0 + o, 0);
                }
                0
            }
            0x2baeb8 => {
                for o in [0, 4, 8] {
                    m.w32(0x348df8 + o, 0);
                }
                0
            }
            0x2be8c8 => {
                lib::init_history(m, s32(0));
                0
            }
            0x2be208 => {
                play::set_clock_base(m, a[0]);
                0
            }
            0x2bb7a0 => {
                lib::clear_event_banks(m);
                0
            }
            0x2bb060 => {
                lib::clear_bank_table(m);
                0
            }
            0x2bb810 => {
                lib::reset_voices(m);
                0
            }
            0x2bb090 => lib::shl2(s32(0)) as u32,
            0x2bb098 => {
                lib::set_bank_table(m, s32(0), a[1]);
                0
            }
            ss::LIB_REGISTER_BANK => lib::register_bank(m, a[0]) as u32,
            _ => panic!("speech host: a call to {t:#x} ({a:x?})"),
        }
    }

    /// The same for a call answering a float
    pub fn dispatch_f(&mut self, _: &mut Mem, t: u32, a: &[u32]) -> f32 {
        match t {
            ss::STREAM_TIME_LEFT => {
                let s = *self.stream(a[0]);
                if s.playing { (s.length_ms - s.played_ms).max(0.0) } else { 0.0 }
            }
            ss::STREAM_PLAY_TIME => self.stream(a[0]).played_ms,
            _ => panic!("speech host: a float call to {t:#x}"),
        }
    }
}

impl SysEnv for HostState {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        self.dispatch(m, target, args)
    }
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32 {
        self.dispatch_f(m, target, args)
    }
}

impl LoadEnv for HostState {
    fn call(&mut self, m: &mut dyn LibMem, target: u32, args: &[u32]) -> u32 {
        let m = m.pf().expect("speech host: the engine's memory");
        self.dispatch(m, target, args)
    }
}

impl Calls for HostState {
    /// `SpeechLib_GetTime` 0x2bebb0: the hook at 0x348e00, which the game only ever clears (`SpeechLib_ClearSpareHooks`
    /// 0x2baeb8): 0
    fn now(&mut self) -> i32 {
        0
    }
    fn release(&mut self, m: &mut dyn LibMem, slot: i32) {
        lib::free_voice(m, slot as u32);
    }
    fn stop_all(&mut self) {
        panic!("speech host: the library shut down");
    }
}

/// The speech system in its memory (see the module)
pub struct SpeechHost {
    pub mem: Mem,
    st: HostState,
}

impl SpeechHost {
    /// The speech system set up for a race (`Audio_LoadInGameConfig`'s speech part): `inf` SPEECH.INF, `files` the
    /// loader of the game's files (paths as the game opens them)
    pub fn new(inf: &[u8], files: Box<dyn FnMut(&str) -> Option<Vec<u8>> + Send + Sync>) -> SpeechHost {
        let mem = Mem {
            regions: vec![
                (0x0034_3000, vec![0; 0x1000]),
                (0x0034_8000, vec![0; 0x2000]),
                (0x0039_e000, vec![0; 0x3000]),
                (SYS_AT, vec![0; SYS_SIZE as usize]),
                (TABLES, vec![0; 0x1_0000]),
                (HEAP, vec![0; HEAP_SIZE as usize]),
                (STACK_BASE, vec![0; (STACK_TOP - STACK_BASE) as usize]),
            ],
        };
        let st = HostState {
            ini: None,
            inf: inf.to_vec(),
            files,
            heap: HEAP,
            sp: STACK_TOP - 0x100,
            rng: 0x1357_9bdf,
            streams: [Stream::default(); 8],
            plays: Vec::new(),
            handle: 0,
        };
        let mut h = SpeechHost { mem, st };
        let m = &mut h.mem;
        for (at, s) in STRINGS {
            put_str(m, at, s.as_bytes());
        }
        m.w32(0x3a0420, 0xffff_0000);
        m.w32(0x3a0424, 0x0022_a2d0);
        put_str(m, INF_PATH, b"data\\config\\speech.inf");
        m.w32(0x3436fc, INF_PATH);
        m.w32(AUDIO_VTABLE + 0x24, RAND_FX);
        m.w32(AUDIO_VTABLE + 0x7c, PLAY_FILE);
        // `cBXAudio::cBXAudio` 0x20db18: the crowd system's constructor runs `SpeechSys_Init`, then the audio
        // object's own vtable
        ss::init(m, &mut h.st, SYS_AT);
        m.w32(SYS_AT + 4, AUDIO_VTABLE);
        // `Audio_LoadInGameConfig` 0x211098's speech part (`SndMgr_GetSlot`'s volume slots: the host's words)
        ss::init_library(m, &mut h.st, SYS_AT);
        ss::add_slot(m, &mut h.st, SYS_AT, 1, 3, 0x39ec00, VOLUME_SLOTS + 0xc, u32::MAX);
        ss::load_events_for_slot(m, &mut h.st, SYS_AT, 1, 0x39ec10);
        ss::add_slot(m, &mut h.st, SYS_AT, 0, 0, 0x39ec28, VOLUME_SLOTS + 8, 2);
        ss::load_events_for_slot(m, &mut h.st, SYS_AT, 0, 0x39ec40);
        h
    }

    /// `SpeechEvt_Post…`: the event's record (0x50 bytes, the id then its words) posted (`SpeechLib_PostEvent`); its
    /// answer (1 kept)
    pub fn post(&mut self, words: &[u32; 20]) -> u32 {
        for (k, w) in words.iter().enumerate() {
            self.mem.w32(EVENT + 4 * k as u32, *w);
        }
        lib::post_event(&mut self.mem, &mut self.st, EVENT)
    }

    /// `SpeechSys_UpdateAll` 0x228998 (each frame, from `Audio_UpdateSystems`)
    pub fn update(&mut self) {
        ss::update_all(&mut self.mem, &mut self.st, SYS_AT);
    }

    /// The lines started since the last call
    pub fn take_plays(&mut self) -> Vec<LinePlay> {
        std::mem::take(&mut self.st.plays)
    }

    /// The app knows a started line's length (ms) once it has decoded it
    pub fn line_started(&mut self, stream: u32, length_ms: f32) {
        self.st.streams[stream as usize].length_ms = length_ms;
    }

    /// `dt` seconds of the streams playing: a line that reaches its length stops
    pub fn frame(&mut self, dt: f32) {
        for s in self.st.streams.iter_mut().filter(|s| s.open && s.playing) {
            s.played_ms += dt * 1000.0;
            if s.played_ms >= s.length_ms {
                s.playing = false;
            }
        }
    }

    /// The stream is playing a line
    pub fn is_playing(&self, stream: u32) -> bool {
        self.st.streams[stream as usize].playing
    }

    /// How many speech banks are loaded (`0x343704`)
    pub fn bank_count(&self) -> u32 {
        self.mem.r32(ss::BANK_COUNT)
    }
}

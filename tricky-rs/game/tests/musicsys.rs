//! `tricky_game::musicsys` (board row G10b2b) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random music systems (their stream tables, the song or none, its bank's state, a fade, Tricky, the
//! song list), every function run on both sides with the same answers from the calls out (the scheduler, Pathfinder,
//! the stream library, the banks), the calls logged in order (floats bit for bit) and the memory compared. The song
//! functions (`musicsong`) run as the original underneath.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::musicsong::{self as song, SongEnv};
use tricky_game::musicsys::*;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const STACK_TOP: u32 = 0x01ff_0000;
const ARENA: u32 = 0x0100_0000;
const SYS: u32 = ARENA;
const SONG: u32 = ARENA + 0x8000;
const TABLES: u32 = ARENA + 0x10000;
const SCALE: u32 = ARENA + 0x11000;
const V_74: u32 = 0x7f_0000;
const V_24: u32 = 0x7f_0100;
const FLOATS: [f32; 8] = [120.0, 90.0, 140.5, 60.0, 0.5, 1.0, 87.25, 133.0];

struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    fn draw(&mut self, target: u32) -> Rng {
        let n = match self.count.iter_mut().find(|c| c.0 == target) {
            Some(c) => {
                c.1 += 1;
                c.1
            }
            None => {
                self.count.push((target, 0));
                0
            }
        };
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..3 {
            g.next();
        }
        g
    }
    fn answer(&mut self, target: u32) -> u32 {
        let mut g = self.draw(target);
        match target {
            song::ALLOC => [0u32, ARENA + 0xe000][g.below(2) as usize],
            song::INI_FIND_SECTION => 0,
            song::PF_SLOT_OF => g.below(4),
            song::FMODF => FLOATS[g.below(FLOATS.len() as u32) as usize].to_bits(),
            song::SND_HANDLE_STATE => g.below(3),
            PICK_VARIANT => g.below(3),
            V_24 => g.below(8),
            _ => g.next(),
        }
    }
    fn words(&mut self, target: u32, first: u32) -> Vec<u32> {
        let mut g = self.draw(target ^ 0x3333);
        let mut w = vec![first];
        for _ in 1..5 {
            w.push(if target == song::HANDLE_STATE { [2u32, 1, 0][g.below(3) as usize] } else { g.next() % 100000 });
        }
        if target == song::HANDLE_STATE {
            w.swap(0, 1);
        }
        w
    }
}
type A = Rc<RefCell<Answers>>;

struct Ours(A);
impl SongEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        if target == song::STRICMP {
            let s = |a: u32| (0..).map(|k| m.r8(a + k)).take_while(|b| *b != 0).collect::<Vec<u8>>();
            return !s(args[0]).eq_ignore_ascii_case(&s(args[1])) as u32;
        }
        self.0.borrow_mut().log.push((target, args.to_vec()));
        self.0.borrow_mut().answer(target)
    }
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32 {
        f32::from_bits(self.call(m, target, args))
    }
    fn call_out(&mut self, _m: &mut Mem, target: u32, args: &[u32], words: usize) -> Vec<u32> {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        let first = self.0.borrow_mut().answer(target);
        let mut w = self.0.borrow_mut().words(target, first);
        w.truncate(words.max(1));
        w
    }
}

/// The leaves the runner stubs: (address, integer arguments, an out buffer in a1, a doubleword in a2 / a1)
const LEAVES: &[(u32, usize, bool)] = &[
    (song::ALLOC, 3, false), (song::MEMSET, 3, false), (song::FREE, 1, false), (song::DELETE, 1, false), (song::SPRINTF, 4, false),
    (song::INI_FIND_SECTION, 2, false), (song::LOAD_FILE, 3, false), (song::PF_REGISTER, 1, false), (song::PF_LOAD_SONG, 8, false),
    (song::PF_SET_LATENCY, 2, false), (song::PF_SLOT_OF, 1, false), (song::PF_SET_VARIABLE, 2, false), (song::PF_POST_EVENT, 3, false),
    (song::PF_STOP_TRACKS, 1, false), (song::BANK_CANCEL, 2, false), (song::BANK_HANDLE, 2, false), (song::SND_LOCK, 0, false),
    (song::SND_UNLOCK, 0, false), (song::SND_HANDLE_STATE, 1, false), (song::STREAM_STATUS, 1, true), (song::HANDLE_STATE, 1, true),
    (song::STREAM_SET_VOLUME, 2, false), (song::STREAM_FADE, 3, false), (song::STREAM_SET_PITCH, 2, false), (song::FMODF, 0, false),
    (SCHEDULE, 8, false), (CANCEL, 2, false), (MEMSET, 3, false), (BANK_LOAD, 4, false), (PICK_VARIANT, 1, false),
    (RANDOM_SEGMENT, 3, false), (INI_CLOSE, 2, false), (STREAM_SYS_DTOR, 2, false), (V_74, 1, false), (V_24, 1, false),
];

const REGIONS: [(u32, u32); 4] = [(ARENA, 0x1_2000), (0x343050, 0x200), (0x39ff00, 0x200), (0, 0x400)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m.regions.push((STACK_TOP - 0x800, vec![0; 0x800]));
    m
}

#[test]
fn music_system_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    for &(f, n, out) in LEAVES {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            match f {
                song::PF_LOAD_SONG => v.push(mem.read_u32(c.gpr(29) as u32)),
                song::FMODF => v = vec![c.f(12).to_bits(), c.f(13).to_bits()],
                SCHEDULE => {
                    let d = c.gpr(6);
                    v = vec![c.gpr(4) as u32, c.gpr(5) as u32, d as u32, (d >> 32) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32, c.gpr(10) as u32, c.gpr(11) as u32];
                }
                CANCEL => {
                    let d = c.gpr(5);
                    v = vec![c.gpr(4) as u32, d as u32, (d >> 32) as u32];
                }
                _ => {}
            }
            a2.borrow_mut().log.push((f, v));
            let ans = a2.borrow_mut().answer(f);
            if out {
                let mut w = a2.borrow_mut().words(f, ans);
                w.truncate(if f == song::STREAM_STATUS { 2 } else { 3 });
                let at = c.gpr(5) as u32;
                for (k, x) in w.iter().enumerate() {
                    mem.write_u32(at + 4 * k as u32, *x);
                }
            }
            c.set_gpr(2, ans as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(ans));
        });
    }
    for f in [
        song::STRICMP, 0x223ca8, 0x223d60, 0x224368, 0x2243e8, 0x224420, 0x224458, 0x224470, 0x224488, 0x224490, 0x224498,
        0x2245b8, 0x224620, 0x224790, 0x2247c0, 0x2247f8, 0x224898, 0x224918, 0x224978, 0x225090, 0x225260, 0x225280,
        0x225318, 0x225448, 0x225660, 0x2256c0, 0x225768, 0x2257d8, 0x225878, 0x225948,
    ] {
        r.allow(f);
    }
    let mut g = Rng(0x2250_90ee_8e8e_0001);
    for case in 0..40_000u32 {
        for k in 0..0x1_2000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.next() });
        }
        // the system: its stream tables, its song or none, the banks' states
        for (k, o) in [0x24u32, 0x38, 0x3c, 0x40, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x7c].iter().enumerate() {
            r.mem.write_u32(SYS + o, TABLES + 0x100 * k as u32);
        }
        r.mem.write_u32(SYS + 0x20, g.below(4));
        for k in 0..8 {
            r.mem.write_u32(TABLES + 0x200 + 4 * k, [0i32, 64, 127, 200, -5, 100, 126, 84][g.below(8) as usize] as u32);
            r.mem.write_u32(TABLES + 0x300 + 4 * k, if g.below(2) == 0 { 0 } else { SCALE });
        }
        r.mem.write_f32(SCALE, [0.5f32, 1.0, 2.0, 0.0][g.below(4) as usize]);
        r.mem.write_u32(SYS + 0x36f0, if g.below(4) == 0 { 0 } else { SONG });
        r.mem.write_u32(SONG + 8, g.below(4));
        r.mem.write_u32(SONG + 0xf8, g.below(3));
        r.mem.write_u32(SONG + 0xfc, g.below(2));
        r.mem.write_u32(SONG + 0x14, g.below(2));
        r.mem.write_u32(SONG + 0xc, if g.below(2) == 0 { 0 } else { SCALE });
        r.mem.write_u32(SONG + 0x12c, [100u32, 50, 150][g.below(3) as usize]);
        r.mem.write_f32(SONG + 0x100, FLOATS[g.below(FLOATS.len() as u32) as usize]);
        r.mem.write_u32(SYS + 0xa3c, TABLES + 0xc00);
        for k in 0..4 {
            r.mem.write_u32(TABLES + 0xc00 + 0x60 * k, g.below(4));
        }
        for o in [0x3700u32, 0x3708, 0x3710, 0x3714] {
            r.mem.write_u32(SYS + o, g.below(3));
        }
        r.mem.write_u32(SYS + 0x3714, g.below(6));
        // the fade
        r.mem.write_f32(SYS + 0x36f4, [0.5f32, -0.5, 0.0, 2.0, -1.25][g.below(5) as usize]);
        // (starting points the steps can land exactly on a target from)
        r.mem.write_f32(SYS + 0x36f8, [60.0f32, 30.0, 80.5, 0.0, -59.5, 29.5, 62.0, 127.5, 1.25, 31.25][g.below(10) as usize]);
        r.mem.write_u32(SYS + 0x36fc, [0i32, 30, 64, 127, -3][g.below(5) as usize] as u32);
        // the song list: some song enabled within the count
        let songs = 1 + g.below(8);
        r.mem.write_u32(SYS + 0x36e0, songs);
        r.mem.write_u32(SYS + 0x36ec, (g.next() & ((1 << songs) - 1)) | 1 << g.below(songs));
        r.mem.write_u32(SYS + 0x36e8, g.below(songs));
        // the vtable
        let vt = TABLES + 0xe00;
        r.mem.write_u32(SYS + 4, vt);
        r.mem.write_u32(vt + 0x74, V_74);
        r.mem.write_u32(vt + 0x24, V_24);
        r.mem.write_u16(vt + 0x70, [0u16, 8][g.below(2) as usize]);
        r.mem.write_u16(vt + 0x20, [0u16, 4][g.below(2) as usize]);
        r.mem.write_u32(song::MUSIC_SYS, SYS);
        r.mem.write_f32(song::MASTER_VOLUME, 1.0);
        let which = g.below(28);
        let x = [g.next(), g.next(), g.next()];
        let secs = [0.5f32, 1.0, 2.5, 4.0][g.below(4) as usize];
        let fe = g.below(6);
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let set = |r: &mut Runner, args: &[u32]| {
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(if k < 4 { 4 + k } else { 8 + k - 4 }, *v as i32 as i64 as u64);
            }
        };
        let name = if g.below(2) == 0 { 0 } else { TABLES + 0xf00 };
        let fe_addr = [0x225a90u32, 0x225ac8, 0x225b10, 0x225b58, 0x225be8, 0x225ba0][fe as usize];
        let (addr, args): (u32, Vec<u32>) = match which {
            0 => (0x224c90, vec![SYS, x[0] & 3]),
            1 => (0x224e68, vec![SYS, name, x[1], if x[2] & 1 == 0 { 0 } else { SCALE }]),
            2 => (0x225090, vec![SYS, name, x[1]]),
            3 => (0x225240, vec![SYS]),
            4 => (0x225260, vec![SYS]),
            5 => (0x225280, vec![SYS]),
            6 => (0x225318, vec![SYS, [0, 1, 10, 4][g.below(4) as usize]]),
            7 => (0x225380, vec![SYS, [0, 1, 10, 4][g.below(4) as usize]]),
            8 => (0x2253e8, vec![SYS, x[0]]),
            9 => (0x225408, vec![SYS, x[0]]),
            10 => (0x225428, vec![SYS, x[0] & 0xff]),
            11 => (0x225448, vec![SYS]),
            12 => (0x225468, vec![SYS, x[0] & 0xff]),
            13 => (0x2254f8, vec![SYS]),
            14 => (0x225588, vec![SYS]),
            15 => (0x2255c0, vec![SYS, g.below(4)]),
            16 => (0x225618, vec![SYS]),
            17 => (0x225660, vec![SYS]),
            18 => (0x2256a0, vec![SYS]),
            19 => (0x2256c0, vec![SYS]),
            20 => (0x225768, vec![SYS, g.below(2)]),
            21 => (0x2257d8, vec![SYS, g.below(2)]),
            22 => (0x225850, vec![SYS]),
            23 => (0x225958, vec![SYS]),
            24 => (0x2259d0, vec![SYS]),
            25 => (fe_addr, vec![SYS]),
            26 => (if g.below(2) == 0 { 0x225c40 } else { 0x225ca8 }, vec![SYS, x[0]]),
            _ => (if g.below(2) == 0 { 0x225ce0 } else { 0x225c30 }, vec![SYS]),
        };
        // a song for the calls that do not check it
        if matches!(which, 13 | 23 | 24) {
            r.mem.write_u32(SYS + 0x36f0, SONG);
            m.w32(SYS + 0x36f0, SONG);
        }
        set(&mut r, &args);
        if which == 10 || which == 12 {
            r.cpu.set_gpr(5, (x[0] & 0xff) as u8 as i8 as i64 as u64);
        }
        if which == 11 || which == 12 {
            r.cpu.set_f(12, secs);
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        let mine_ret: Option<u32> = match which {
            0 => { destroy(&mut m, e, SYS, x[0] & 3); None }
            1 => { play_song(&mut m, e, SYS, name, x[1], args[3], STACK_TOP - 0x80); None }
            2 => { start_song(&mut m, e, SYS, name, x[1], STACK_TOP - 0x40); None }
            3 => { stop_current(&mut m, e, SYS); None }
            4 => { resume_song(&mut m, e, SYS); None }
            5 => { stop_song(&mut m, e, SYS); None }
            6 => { music_event(&mut m, e, SYS, args[1]); None }
            7 => { set_section(&mut m, e, SYS, args[1]); None }
            8 => { set_path_var(&mut m, e, SYS, x[0]); None }
            9 => { set_latency(&mut m, e, SYS, x[0]); None }
            10 => { set_song_volume(&mut m, SYS, x[0] as u8); None }
            11 => { fade_out(&mut m, e, SYS, secs); None }
            12 => { fade_to(&mut m, e, SYS, secs, x[0] as u8); None }
            13 => Some(fade_step(&mut m, SYS)),
            14 => Some(count_enabled(&m, SYS)),
            15 => { select_song(&mut m, SYS, args[1] as i32); None }
            16 => { prev_song(&mut m, SYS); None }
            17 => { next_enabled(&mut m, SYS); None }
            18 => { update_song_volume(&mut m, e, SYS); None }
            19 => { load_song_bank(&mut m, e, SYS, STACK_TOP - 0x130); None }
            20 => { schedule_break(&mut m, e, SYS, args[1]); None }
            21 => { cancel_break(&mut m, e, SYS, args[1]); None }
            22 => Some(virtual_74(&mut m, e, SYS)),
            23 => Some(try_schedule_air_loop(&mut m, e, SYS)),
            24 => Some(reload_bank(&mut m, e, SYS, STACK_TOP - 0x40)),
            25 => { set_fe_event(&mut m, e, SYS, [0, 1, 2, 3, 4, 5][fe as usize]); None }
            26 => { if addr == 0x225c40 { start_tricky(&mut m, e, SYS, x[0]) } else { stop_tricky(&mut m, e, SYS) } None }
            _ => if addr == 0x225ce0 { Some(count_enabled_songs(&m, SYS)) } else { fade_out_4s(&mut m, e, SYS); None },
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate().take(REGIONS.len()) {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i]) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
}

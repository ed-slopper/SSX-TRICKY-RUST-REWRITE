//! `tricky_game::musicsong` (board row G10b2a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random songs, music systems and `MUSIC.INF` sections (the ini reader's answers, the tokens, the keys
//! found or not), every function run on both sides with the same answers from the calls out (Pathfinder, the
//! stream library, the allocator), the calls logged in order (floats bit for bit) and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::musicsong::*;
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
const SONG: u32 = ARENA;
const SYS: u32 = ARENA + 0x8000;
const TOKENS: u32 = ARENA + 0xc000;
const SCALE: u32 = ARENA + 0xd000;
const FLOATS: [f32; 8] = [120.0, 90.0, 140.5, 60.0, 0.5, 1.0, 87.25, 133.0];

/// The answers: the same on both sides for the same call (target, how many of it before)
struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
    tokens: u32,
}
impl Answers {
    fn draw(&mut self, target: u32) -> (Rng, u32) {
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
        (g, n)
    }
    fn answer(&mut self, target: u32) -> u32 {
        let tokens = self.tokens;
        let (mut g, n) = self.draw(target);
        match target {
            ALLOC => [0u32, ARENA + 0xe000, ARENA + 0xf000][g.below(3) as usize],
            INI_FIND_SECTION => (g.below(4) != 0) as u32,
            INI_AT_END => (n >= tokens) as u32,
            INI_READ_TOKEN => TOKENS + 0x20 * (n % 16),
            INI_READ_STRING | INI_READ_FLOAT | INI_READ_INT => g.below(2),
            PF_SLOT_OF => g.below(4),
            STREAM_STATUS | HANDLE_STATE | PF_PLAYING => g.next(),
            FMODF => FLOATS[g.below(FLOATS.len() as u32) as usize].to_bits(),
            SND_HANDLE_STATE => g.below(3),
            _ => g.next(),
        }
    }
    /// What a call writes to memory (the same on both sides): the token's text, a string, a float, an int
    fn effect(&mut self, target: u32, args: &[u32], ans: u32) -> Vec<(u32, Vec<u8>)> {
        match target {
            INI_READ_TOKEN => {
                let (mut g, _) = self.draw(target ^ 0x5555);
                let names: [&[u8]; 8] = [b"SIXTEENTH\0", b"Eighth\0", b"quarter\0", b"Whole\0", b"OTHER\0", b"\0", b"Sixteenths\0", b"Eight\0"];
                vec![(ans, names[g.below(8) as usize].to_vec())]
            }
            INI_READ_STRING if ans != 0 => vec![(args[2], format!("file{:x}\0", args[1] & 0xff).into_bytes())],
            INI_READ_FLOAT if ans != 0 => {
                let (mut g, _) = self.draw(target ^ 0x7777);
                vec![(args[2], FLOATS[g.below(FLOATS.len() as u32) as usize].to_bits().to_le_bytes().to_vec())]
            }
            INI_READ_INT if ans != 0 => {
                let (mut g, _) = self.draw(target ^ 0x9999);
                vec![(args[2], ([0i32, 1, 4, 100, 50, -3, 200][g.below(7) as usize]).to_le_bytes().to_vec())]
            }
            _ => vec![],
        }
    }
}
type A = Rc<RefCell<Answers>>;

struct Ours(A);
impl SongEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        if target == STRICMP {
            let s = |a: u32| (0..).map(|k| m.r8(a + k)).take_while(|b| *b != 0).collect::<Vec<u8>>();
            let (x, y) = (s(args[0]), s(args[1]));
            self.0.borrow_mut().log.push((target, args.to_vec()));
            return !x.eq_ignore_ascii_case(&y) as u32;
        }
        self.0.borrow_mut().log.push((target, args.to_vec()));
        let ans = self.0.borrow_mut().answer(target);
        for (at, bytes) in self.0.borrow_mut().effect(target, args, ans) {
            for (k, b) in bytes.iter().enumerate() {
                m.w8(at + k as u32, *b);
            }
        }
        ans
    }
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32 {
        f32::from_bits(self.call(m, target, args))
    }
    fn call_out(&mut self, _m: &mut Mem, target: u32, args: &[u32], words: usize) -> Vec<u32> {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        let first = self.0.borrow_mut().answer(target);
        let (mut g, _) = self.0.borrow_mut().draw(target ^ 0x3333);
        let mut v = vec![first];
        for _ in 1..5 {
            v.push(if target == HANDLE_STATE { [2u32, 1, 0][g.below(3) as usize] } else { g.next() % 100000 });
        }
        // the state word first for the handle's state (2: playing)
        if target == HANDLE_STATE {
            v.swap(0, 1);
        }
        v.truncate(words.max(1));
        v
    }
}

/// The leaves the runner stubs: (address, integer arguments, an out buffer in a1)
const LEAVES: &[(u32, usize, bool)] = &[
    (ALLOC, 3, false), (MEMSET, 3, false), (FREE, 1, false), (DELETE, 1, false), (SPRINTF, 4, false), (INI_FIND_SECTION, 2, false),
    (INI_READ_TOKEN, 1, false), (INI_AT_END, 1, false), (INI_READ_STRING, 3, false), (INI_READ_FLOAT, 3, false), (INI_READ_INT, 3, false),
    (LOAD_FILE, 3, false), (PF_REGISTER, 1, false), (PF_LOAD_SONG, 8, false), (PF_SET_LATENCY, 2, false), (PF_SLOT_OF, 1, false),
    (PF_PLAYING, 1, true), (PF_SET_VARIABLE, 2, false), (PF_POST_EVENT, 3, false), (PF_STOP_TRACKS, 1, false), (BANK_CANCEL, 2, false),
    (BANK_HANDLE, 2, false), (SND_LOCK, 0, false), (SND_UNLOCK, 0, false), (SND_HANDLE_STATE, 1, false), (STREAM_STATUS, 1, true),
    (HANDLE_STATE, 1, true), (STREAM_SET_VOLUME, 2, false), (STREAM_FADE, 3, false), (STREAM_SET_PITCH, 2, false), (FMODF, 0, false),
];

const REGIONS: [(u32, u32); 4] = [(ARENA, 0x1_0000), (0x343050, 0x200), (0x39ff00, 0x200), (0, 0x400)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m.regions.push((STACK_TOP - 0x400, vec![0; 0x400]));
    m
}

#[test]
fn song_objects_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![], tokens: 0 }));
    for &(f, n, out) in LEAVES {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == PF_LOAD_SONG {
                v.push(mem.read_u32(c.gpr(29) as u32));
            }
            if f == FMODF {
                v = vec![c.f(12).to_bits(), c.f(13).to_bits()];
            }
            a2.borrow_mut().log.push((f, v.clone()));
            let ans = a2.borrow_mut().answer(f);
            if out {
                // the words the port's env gives, into the buffer
                let (mut g, _) = a2.borrow_mut().draw(f ^ 0x3333);
                let mut w = vec![ans];
                for _ in 1..5 {
                    w.push(if f == HANDLE_STATE { [2u32, 1, 0][g.below(3) as usize] } else { g.next() % 100000 });
                }
                if f == HANDLE_STATE {
                    w.swap(0, 1);
                }
                let at = c.gpr(5) as u32;
                // as many words as each fills (its caller's buffer is no bigger)
                w.truncate(match f { STREAM_STATUS => 2, HANDLE_STATE => 3, _ => 5 });
                for (k, x) in w.iter().enumerate() {
                    mem.write_u32(at + 4 * k as u32, *x);
                }
            }
            for (at, bytes) in a2.borrow_mut().effect(f, &v, ans) {
                for (k, b) in bytes.iter().enumerate() {
                    mem.write_u8(at + k as u32, *b);
                }
            }
            c.set_gpr(2, ans as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(ans));
        });
    }
    // stricmp runs as the original; the song's own functions call each other
    for f in [STRICMP, 0x223d60, 0x224620, 0x224790, 0x2247c0, 0x2247f8, 0x224898, 0x2248b8, 0x224840] {
        r.allow(f);
    }
    // the stricmp log: the runner's calls to it are not stubbed, so the port's entries are dropped before comparing
    let mut g = Rng(0x2241_68ee_8e8e_0001);
    for case in 0..30_000u32 {
        for k in 0..0x1_0000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.next() });
        }
        // the song's floats real: the tempo, the volume scale, the master volume
        r.mem.write_f32(SONG + 0x100, FLOATS[g.below(FLOATS.len() as u32) as usize]);
        r.mem.write_f32(SCALE, [0.5f32, 1.0, 2.0, 0.0][g.below(4) as usize]);
        r.mem.write_u32(SONG + 0xc, if g.below(2) == 0 { 0 } else { SCALE });
        r.mem.write_u32(SONG + 0x12c, [100u32, 50, 0, 150, 300][g.below(5) as usize]);
        r.mem.write_u32(SONG + 0x14, g.below(2));
        r.mem.write_u32(SONG + 0x104, g.below(8));
        r.mem.write_u32(SONG + 0x108, g.below(8));
        r.mem.write_u32(0x343150, SYS);
        for k in 0..16u32 {
            r.mem.write_u32(STREAMS + 4 * k, g.next());
        }
        r.mem.write_f32(MASTER_VOLUME, [1.0f32, 0.5, 0.8][g.below(3) as usize]);
        r.mem.write_u32(SYS + 0x3c, SYS + 0x100);
        r.mem.write_u32(SYS + 0x20, g.below(4));
        r.mem.write_u32(SYS + 0x24, SYS + 0x200);
        for k in 0..4 {
            r.mem.write_u32(SYS + 0x100 + 4 * k, [0i32, 64, 127, 200, -5, 100, 126, 84][g.below(8) as usize] as u32);
        }
        r.mem.write_u32(SYS + 0xa3c, SYS + 0x400);
        r.mem.write_u32(SONG + 8, g.below(4));
        for k in 0..4 {
            r.mem.write_u32(SYS + 0x400 + 0x60 * k, g.below(2));
        }
        let which = g.below(19);
        let x = [g.next(), g.next(), g.next()];
        let secs = FLOATS[g.below(FLOATS.len() as u32) as usize];
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
            b.tokens = g.below(4);
        }
        let mut m = snap(&mut r);
        let set = |r: &mut Runner, args: &[u32]| {
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(if k < 4 { 4 + k } else { 8 + k - 4 }, *v as i32 as i64 as u64);
            }
        };
        let base = TOKENS + 0x300;
        let (addr, args): (u32, Vec<u32>) = match which {
            0 => (0x223ca8, vec![base, TOKENS + 0x400, SYS, x[0] & 0xff, x[1], x[2]]),
            1 => (0x223d60, vec![SONG, base, TOKENS + 0x400, SYS, x[0] & 0xff, x[1], x[2]]),
            2 => (0x224368, vec![SONG, x[0] & 3]),
            3 => (0x224620, vec![SONG]),
            4 => (0x2243e8, vec![SONG]),
            5 => (0x224420, vec![SONG]),
            6 => (0x224458, vec![SONG, x[0]]),
            7 => (0x224470, vec![SONG, x[0]]),
            8 => (0x224488, vec![SONG, x[0]]),
            9 => (0x224490, vec![SONG, x[0]]),
            10 => (0x224498, vec![SONG]),
            11 => (0x2245b8, vec![SONG]),
            12 => (0x224768, vec![SONG]),
            13 => (0x2247f8, vec![SONG]),
            14 => (0x224840, vec![SONG]),
            15 => (0x224898, vec![SONG]),
            16 => (0x2248b8, vec![SONG]),
            17 => (0x224918, vec![SONG]),
            _ => (if g.below(2) == 0 { 0x224978 } else { 0x2249c0 }, vec![SONG, SYS]),
        };
        set(&mut r, &args);
        if which == 11 {
            r.cpu.set_f(12, secs);
        }
        if which == 0 {
            // the signed byte as the caller passes it
            r.cpu.set_gpr(7, (x[0] & 0xff) as u8 as i8 as i64 as u64);
        }
        if which == 1 {
            r.cpu.set_gpr(8, (x[0] & 0xff) as u64);
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let (ret, ret_f) = (r.cpu.gpr(2) as u32, r.cpu.f(0).to_bits());
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        let mine_ret: Option<u32> = match which {
            0 => Some(create(&mut m, e, base, TOKENS + 0x400, SYS, x[0] as u8, x[1], x[2], STACK_TOP - 0x80 - 0x1b0)),
            1 => Some(load(&mut m, e, SONG, base, TOKENS + 0x400, SYS, x[0] as u8, x[1], x[2], STACK_TOP - 0x1b0)),
            2 => {
                delete(&mut m, e, SONG, x[0] & 3);
                None
            }
            3 => Some(stream(&mut m, e, SONG)),
            4 => {
                pause(&mut m, e, SONG);
                None
            }
            5 => {
                resume(&mut m, e, SONG);
                None
            }
            6 => {
                music_event(&mut m, e, SONG, x[0]);
                None
            }
            7 => {
                set_section(&mut m, e, SONG, x[0]);
                None
            }
            8 => Some(set_path_var(&mut m, e, SONG, x[0])),
            9 => Some(set_latency(&mut m, e, SONG, x[0])),
            10 => {
                update_volume(&mut m, e, SONG);
                None
            }
            11 => {
                fade_out(&mut m, e, SONG, secs);
                None
            }
            12 => Some(playing(&mut m, e, SONG)),
            13 => Some(play_pos_ms(&mut m, e, SONG).to_bits()),
            14 => Some(length_ms(&mut m, e, SONG).to_bits()),
            15 => Some(beat_ms(&m, SONG).to_bits()),
            16 => Some(bars_ms(&m, SONG).to_bits()),
            17 => Some(ms_to_next_beat(&mut m, e, SONG).to_bits()),
            _ => Some(if addr == 0x224978 { current_beat(&mut m, e, SONG) as u32 } else { ready(&mut m, e, SONG, SYS) }),
        };
        let mine: Vec<(u32, Vec<u32>)> = std::mem::take(&mut a.borrow_mut().log).into_iter().filter(|c| c.0 != STRICMP).collect();
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            let t = if (13..=17).contains(&which) { ret_f } else { ret };
            assert_eq!(t, v, "case {case}: {addr:#x} answer");
        }
        // (the last region, the frames, holds the runner's saved registers too)
        for (k, (at, b)) in after.regions.iter().enumerate().take(REGIONS.len()) {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i]) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
}

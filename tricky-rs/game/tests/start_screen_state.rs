//! `tricky_game::start_screen` (board row E1f2) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the single start screen's frame (vf5), `StartScreenSingle_SetState`, its message (vf7),
//! `Input_GetAnyNewButtons` and the static initialiser beside them. Every case builds a random screen (its state,
//! timers, card answers and block), app, input devices, strings and sound object in the arena and the game's data;
//! one function then runs on both sides. `SetState` and `Loc_GetUnquotedString` run for real on both sides; every
//! other callee is a stand-in answering from the case's seed: the memory card's virtuals (a table of stand-ins), the
//! strings (pointers into a table of random wide strings), the CRC (the kept one or another). The functions that
//! write a buffer (the names, formats and the brightness) write four words of the seed into it. The calls are
//! compared with their arguments (buffers on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_main as gm;
use tricky_game::pf::Mem;
use tricky_game::start_screen as ss;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
const CAREER: (u32, u32) = (0x32b000, 0x2000);
const SYS: (u32, u32) = (0x343000, 0x100);
const RECORD: (u32, u32) = (0x3cdd00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x900;
const S: u32 = ARENA + 0x1000;
/// the renderer's table
const VT: u32 = ARENA + 0x5000;
/// the card's table
const VT2: u32 = ARENA + 0x5200;
/// the sound object's table (its +4)
const VT3: u32 = ARENA + 0x5400;
const CARD: u32 = ARENA + 0x5500;
const SND: u32 = ARENA + 0x5600;
const INPUT: u32 = ARENA + 0x5700;
fn device(k: u32) -> u32 {
    ARENA + 0x5800 + 0x20 * k
}
const STRS: u32 = ARENA + 0x6000;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot2(off: u32) -> u32 {
    0x0182_0000 + off
}
fn slot3(off: u32) -> u32 {
    0x0183_0000 + off
}
const SLOTS: [u32; 1] = [0x120];
const SLOTS2: [u32; 24] = [
    0x18, 0x28, 0x30, 0x40, 0x48, 0x58, 0x68, 0x80, 0x88, 0x98, 0xb0, 0xb8, 0xc8, 0xd0, 0xd8, 0x140, 0x150, 0x160,
    0x170, 0x178, 0x190, 0x1a8, 0x1b8, 0x1e8,
];
const SLOTS3: [u32; 1] = [0x10];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 1] = [0x1842a8];
/// two CRCs: the one kept and another
const CRCS: [u32; 2] = [0x1234, 0xbeef];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
}
fn stack(a: u32) -> bool {
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x700).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

/// The stand-ins that write a buffer at their first argument
const WRITERS: [u32; 8] =
    [ss::GAME_CODE, ss::TAG_SET, ss::TAG_REP, ss::SPRINTF, ss::BRIGHTNESS, ss::WSTR_FORMAT, ss::WSTR_FORMAT_ASCII, ss::WSTR_COPY];

fn arity(f: u32, w: &[u32]) -> (usize, usize) {
    match f {
        ss::AUDIO_DELETE => (0, 0),
        ss::WSTR_FORMAT_ASCII => (if w[1] == ss::THREE_FORMAT { 5 } else { 4 }, 0),
        ss::SPRINTF | ss::DRAW_BOX => (4, 0),
        ss::MEM_COPY | gm::FILL | ss::WSTR_FORMAT => (3, 0),
        ss::INPUT_DEVICE | ss::GAME_CODE | ss::TAG_SET | ss::TAG_REP | ss::CRC16 | ss::BRIGHTNESS | ss::REND_COLOUR
        | ss::TB_FONT | ss::WSTR_COPY | ap::LOC_STRING => (2, 0),
        ss::TB_SCALE => (1, 2),
        f if f == slot(0x120) || f == slot3(0x10) => (2, 0),
        f if f == slot2(0x28) || f == slot2(0x30) || f == slot2(0x80) || f == slot2(0x98) || f == slot2(0x1b8) => (2, 0),
        f if f == slot2(0xb8) => (4, 0),
        _ => (1, 0),
    }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, w);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let writes = WRITERS.contains(&f);
    for (k, &a) in w[..ni].iter().enumerate() {
        if stack(a) && !(writes && k == 0) {
            let n = if f == ss::REND_COLOUR { 3 } else { 4 };
            log.extend((0..n).map(|j| token(io.r32(a + 4 * j))));
        }
    }
    if f == ss::SPRINTF {
        // (where the name's parts lie from the name)
        log.extend([w[2].wrapping_sub(w[0]), w[3].wrapping_sub(w[0])]);
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if writes {
        for j in 0..4 {
            io.w32(w[0] + 4 * j, g.next());
        }
    }
    let answer = match f {
        ss::INPUT_DEVICE => device(w[1] % 4),
        ss::CRC16 => g.pick(&CRCS),
        ap::LOC_STRING => STRS + 0x40 * g.below(32),
        ap::WSTR_LEN => g.below(20),
        f if f == slot3(0x10) => g.pick(&[0u32, 1, 1, 2]),
        f if f == slot2(0xc8) => g.pick(&[1u32, 1, 0, 2]),
        f if f == slot2(0xb0) => g.pick(&[1u32, 0]),
        f if f == slot2(0x68) => g.pick(&[0u32, 1, 2, 3, 4, 4, 5]),
        f if f == slot2(0x1b8) => g.pick(&[0u32, 0, 0, 1]),
        f if f == slot2(0x98) || f == slot2(0x88) => g.pick(&[0u32, 3, 8, 0x30]),
        f if f == slot2(0x58) => g.pick(&[0u32, 0, 1]),
        f if SLOTS2.iter().any(|&o| slot2(o) == f) => g.pick(&[0u32, 1]),
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let mut w0: Vec<u32> = args.to_vec();
        w0.resize(8, 0);
        let (ni, nf) = arity(f, &w0);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(8, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 5] = [0x1834a8, 0x1850e8, 0x1845e0, 0x185430, 0x1858c0];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    let buf = SCRATCH;
    Some(match f {
        0x1834a8 => ss::any_new_buttons(m, e),
        0x1850e8 => return { ss::set_state(m, e, a[0], a[1], buf); None },
        0x1845e0 => return { ss::single_frame(m, e, a[0], buf); None },
        0x185430 => return { ss::single_message(m, e, a[0], buf); None },
        0x1858c0 => return { ss::static_init(m); None },
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn start_screen_states_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    let mut stubs = vec![
        ss::INPUT_DEVICE, ss::GAME_CODE, ss::TAG_SET, ss::TAG_REP, ss::SPRINTF, ss::CRC16, ss::MEM_COPY,
        ss::AUDIO_DELETE, ss::AUDIO_CREATE, ss::AUDIO_SETTINGS, ss::BRIGHTNESS, ss::REND_COLOUR, ss::TB_RESET,
        ss::TB_FONT, ss::TB_SCALE, ss::WSTR_FORMAT, ss::WSTR_FORMAT_ASCII, ss::WSTR_COPY, ss::DRAW_BOX, gm::FILL,
        ap::LOC_STRING, ap::WSTR_LEN,
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS2.map(slot2));
    stubs.extend(SLOTS3.map(slot3));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x5e7a_0000_0000_0001);
    let per = 3000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (RECORD.0, vec![0; RECORD.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (SCRATCH, vec![0; 0x700]),
        ];
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..RECORD.1 / 4 {
            m.w32(RECORD.0 + 4 * k, g.next());
        }
        for k in 0..CAREER.1 / 4 {
            m.w32(CAREER.0 + 4 * k, g.next());
        }
        for k in 0..32 {
            if g.below(2) == 0 {
                m.w16(STRS + 0x40 * k, 0x22);
            }
            for j in 1..20 {
                if g.below(4) == 0 {
                    m.w16(STRS + 0x40 * k + 2 * j, 0x22);
                }
            }
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x24, INPUT);
        m.w32(APPOBJ + 0x724, R);
        m.w32(INPUT + 0x14, g.pick(&[0u32, 1, 2, 3]));
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for off in SLOTS2 {
            m.w16(VT2 + off, g.pick(&[0u16, 4]));
            m.w32(VT2 + off + 4, slot2(off));
        }
        for off in SLOTS3 {
            m.w16(VT3 + off, g.pick(&[0u16, 4]));
            m.w32(VT3 + off + 4, slot3(off));
        }
        m.w32(gm::SOUND, SND);
        m.w32(SND + 4, VT3);
        m.w32(S + 4, CARD);
        m.w32(CARD, VT2);
        // (the screen: its state, timers, flags, the card's answers kept, the blink and the block)
        m.w32(S + 0x3cc4, g.below(30));
        for at in [0x14b8u32, 0x14c4] {
            m.w32(S + at, g.pick(&[u32::MAX, 0, 0, 1, 2, 50]));
        }
        m.w32(S + 0x14c8, g.pick(&[0u32, 0, 1]));
        m.w32(S + 0x44, g.pick(&[0u32, 0, 0, 1]));
        for at in [0x3cb4u32, 0x3cb8] {
            m.w32(S + at, g.pick(&[0u32, 0, 1, 3]));
        }
        m.w32(S + 0x14c0, g.pick(&[0u32, 1, 1, 2, 3]));
        m.w32(S + 0x2b6c, g.below(2));
        m.w32(S + 0x3c9c, CRCS[0]);
        // (the blink, some landing on 1 or 0.2 exactly)
        let v = g.f(0.0, 1.2);
        let (b, st) = g.pick(&[(0.5f32, 0.5f32), (0.7, -0.5), (v, 1.0 / 60.0), (v, -1.0 / 60.0)]);
        m.w32(S + 0x3ca8, b.to_bits());
        m.w32(S + 0x3cac, st.to_bits());
        m.w32(S + 0x2b7c, g.f(-1.0, 2.5).to_bits());
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x1850e8 => [S, g.below(30), 0, 0],
            _ => [S, 0, 0, 0],
        };
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a);
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if let Some(v) = mine {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
}

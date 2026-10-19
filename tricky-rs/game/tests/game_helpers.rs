//! `tricky_game::game_helpers` (board row E1i) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the pause tests, the setters, the race restart and camera reset, the views, `Race_GetRider`, the
//! load module's destructor and the app's module switch, string region and archives. Every case builds a random game
//! object, race, app and career in the arena and the game's data (the course list read from the game); one function
//! then runs on both sides. The app's checked functions it calls run for real on both sides (`Loc_Load`, the course
//! table's fill); every other callee is a stand-in answering from the case's seed, and the display's colour read
//! writes its buffer. The calls are compared with their arguments (buffers on the stack read by value), the answers
//! (the brightness as a float) and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_helpers as gh;
use tricky_game::game_main as gm;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const SYS: (u32, u32) = (0x343000, 0x800);
/// the course list, the game's
const COURSES: (u32, u32) = (0x332f00, 0x200);
const QUIET: (u32, u32) = (0x3bb900, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x1000;
const G: u32 = ARENA + 0x1800;
const RACE: u32 = ARENA + 0x2000;
const VT: u32 = ARENA + 0x2400;
const MODULE: u32 = ARENA + 0x2800;
const REC: u32 = ARENA + 0x2900;
const HEAP: (u32, u32) = (0x0110_0000, 0x1000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
/// the race's virtuals (+0x80, +0x88) and the module's (+0x10)
const SLOTS: [u32; 3] = [0x10, 0x80, 0x88];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 3] = [0x17c0a8, 0x186268, 0x17c1d0];

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
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32, w: &[u32]) -> (usize, usize) {
    match f {
        // (the course table's names take three, the build string four)
        gh::SPRINTF => (if w[1] == gh::BUILD[0] { 4 } else { 3 }, 0),
        gh::SCRIPT_RESET | gh::BIGS_INIT => (0, 0),
        gh::BIG_OPEN => (4, 0),
        gh::COURSE_RECORD | gh::STRINGS_LOAD | gm::ALLOC => (3, 0),
        gh::SCRIPT_STOP | gh::BRIGHTNESS | gh::REND_COLOUR | ap::STRCPY => (2, 0),
        _ => (1, 0),
    }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
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
    for (k, &a) in w[..ni].iter().enumerate() {
        if stack(a) && !(f == gh::BRIGHTNESS && k == 0) {
            log.extend((0..3).map(|j| token(io.r32(a + 4 * j))));
        }
    }
    if f == gh::BIG_OPEN {
        // (the archives' quiet flag, which their opening reads)
        log.push(io.r32(gh::QUIET));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == gh::BRIGHTNESS {
        for j in 0..4 {
            io.w32(w[0] + 4 * j, g.next());
        }
    }
    let answer = match f {
        gm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        gh::COURSE_RECORD => REC,
        ap::FILE_SIZE => g.pick(&[0x40u32, 0x100, 0]),
        gh::PAUSE_OPEN | gh::LESSON_PAUSED | gh::SPEECH_BUSY | gh::FILE_EXISTS => g.pick(&[0u32, 0, 1, 2]),
        // (none in the stack's range)
        _ => r & !0x0100_0000,
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

const FUNCS: [u32; 15] = [
    0x17f078, 0x17f0f8, 0x17f168, 0x17f1a0, 0x17b970, 0x17b980, 0x180a10, 0x180b58, 0x181d50, 0x181d60, 0x1814a8,
    0x181e48, 0x17c6b8, 0x17c150, 0x17bdf8,
];

/// What a function answers: a word, a float (its bits), or nothing
enum Answer {
    Word(u32),
    Float(u32),
    None,
}

/// One function on our side
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], b: f32) -> Answer {
    let buf = SCRATCH;
    let none = Answer::None;
    match f {
        0x17f078 => Answer::Word(gh::is_paused(m, e, a[0])),
        0x17f0f8 => Answer::Word(gh::menu_or_overlay(m, e, a[0])),
        0x17f168 => {
            gh::set_hud_option(m, a[1]);
            none
        }
        0x17f1a0 => {
            gh::set_viewport_mode(m, e, a[0], a[1]);
            none
        }
        0x17b970 => Answer::Float(gh::get_brightness(m).0.to_bits()),
        0x17b980 => {
            gh::set_brightness(m, e, a[0], F(b), buf);
            none
        }
        0x180a10 => {
            gh::restart_race(m, e, a[0]);
            none
        }
        0x180b58 => {
            gh::reset_cameras(m, e, a[0]);
            none
        }
        0x181d50 => {
            gh::restore_views(m, e, a[0]);
            none
        }
        0x181d60 => {
            gh::use_single_view(m, e, a[0]);
            none
        }
        0x1814a8 => Answer::Word(gh::race_rider(m, a[0], a[1])),
        0x181e48 => {
            gh::load_module_dtor(m, e, a[0], a[1]);
            none
        }
        0x17c6b8 => {
            gh::app_set_mode(m, e, a[0], a[1]);
            none
        }
        0x17c150 => {
            gh::loc_load_for_region(m, e, a[0], a[1]);
            none
        }
        0x17bdf8 => {
            gh::open_big_files(m, e, a[0]);
            none
        }
        _ => unreachable!("{f:#x}"),
    }
}

#[test]
fn game_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        gh::PAUSE_OPEN, gh::LESSON_PAUSED, gh::SPEECH_BUSY, gh::LAYOUT_VIEWS, gh::RESTART_BEGIN, gh::CAMERA_RESET,
        gh::SCRIPT_STOP, gh::SCRIPT_RESET, gh::RIDERS_RESTART, gh::HELPER_RESTART, gh::REPLAY_RESTART,
        gh::SPEECH_RESET, gh::SOUND_RESTART, gh::PAD_LATCH, gh::COURSE_RECORD, gh::BRIGHTNESS, gh::REND_COLOUR,
        gh::BIGS_INIT, gh::FILE_EXISTS, gh::BIG_OPEN, gh::STRINGS_LOAD, gh::SPRINTF, gm::FREE, gm::ALLOC,
        ap::HEAP_FREE, ap::FILE_SIZE, ap::STRCPY,
    ];
    stubs.extend(SLOTS.map(slot));
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
    let mut courses = vec![0u8; COURSES.1 as usize];
    r.mem.read(COURSES.0, &mut courses).unwrap();
    let mut g = Rng(0x4e1f_0000_0000_0001);
    let per = 1000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (COURSES.0, courses.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (QUIET.0, vec![0; QUIET.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        let mut m = Mem { regions };
        // (random words, none in the stack's range)
        for (at, n) in [(ARENA, SIZE), (CAREER.0, CAREER.1), (QUIET.0, QUIET.1), (SYS.0, SYS.1)] {
            for k in 0..n / 4 {
                m.w32(at + 4 * k, g.next() & !0x0100_0000);
            }
        }
        let c = gm::CAREER;
        m.w32(c + 0x2afc, g.pick(&[0u32, 7, 7, 3]));
        m.w32(c + 0x16b0, g.below(3));
        m.w32(c + 0x1688, g.below(3));
        m.w32(c + 0x16b4, g.pick(&[0.5f32, 1.0, -0.0, 0.0]).to_bits());
        m.w32(0x32ca44, g.f(-1.0, 3.0).to_bits());
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(APPOBJ + 0x54, g.pick(&[0u32, 0x80, 0x400]));
        m.w32(APPOBJ + 0x50, if g.below(3) == 0 { 0 } else { HEAP.0 + 0x800 });
        m.w32(G + 0x1c, RACE);
        m.w32(G + 0x40, if g.below(3) == 0 { 0 } else { ARENA + 0x3000 });
        m.w32(G + 0x34, if g.below(3) == 0 { 0 } else { ARENA + 0x3100 });
        m.w32(G + 0x48, g.pick(&[0u32, 0, 1]));
        m.w32(G + 0x294, g.below(4));
        m.w32(RACE + 8, VT);
        m.w32(MODULE, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x17f168 | 0x17f1a0 => [G, g.below(3), 0, 0],
            0x1814a8 => [G, g.below(8), 0, 0],
            0x181e48 => [MODULE, g.below(4), 0, 0],
            0x17c6b8 => [APPOBJ, MODULE, 0, 0],
            0x17c150 => [APPOBJ, g.below(4), 0, 0],
            0x17b970 | 0x17b980 | 0x17bdf8 => [APPOBJ, 0, 0, 0],
            _ => [G, 0, 0, 0],
        };
        let v = g.f(-3.0, 3.0);
        let b = g.pick(&[0.5f32, 1.0, -0.0, v]);
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a, b);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, b);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        match mine {
            Answer::Word(v) => assert_eq!(r.cpu.gpr(2) as u32, v, "case {case} {func:#x}: the answer"),
            Answer::Float(v) => assert_eq!(r.cpu.f(0).to_bits(), v, "case {case} {func:#x}: the answer"),
            Answer::None => {}
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

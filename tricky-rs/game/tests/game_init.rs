//! `tricky_game::game_main::game_init` (board row E1h) against the original in the function runner (TRICKY_ELF;
//! skipped without it): `cGame::Init`, the race set up. Every case builds a random game object, app (its course
//! table, fonts and renderer), career (the course, mode, players and the settings it reads) and pads in the arena and
//! the game's data; the init then runs on both sides. The game's own functions it calls that are already checked run
//! for real on both sides (the course table's look-ups, the course archive's open and close, the effects loader, the
//! game's restart); every other callee is a stand-in answering from the case's seed: the allocator (a heap of junk the
//! fill stand-in really fills), the node constructor (which leaves the race a player and AI count), the courses'
//! constructor (which fills its block), `strcpy` (a short string), the effects' random draws and the objects' virtuals
//! (the race's table is the game's, its two virtuals stood in for). The calls are compared with their arguments
//! (buffers on the stack read by value) and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_main as gm;
use tricky_game::game_main::init as gi;
use tricky_game::pf::Mem;

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
}

const DATA: (u32, u32) = (0x337c00, 0x400);
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const SYS: (u32, u32) = (0x343000, 0x100);
/// the race's table, the game's
const RACE_VT: (u32, u32) = (0x364700, 0x200);
/// the race's two virtuals the init and the restart call
const RACE_FUNCS: [u32; 2] = [0x1108f0, 0x111108];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x1000;
const G: u32 = ARENA + 0x2000;
const VT: u32 = ARENA + 0x3000;
const DEV: u32 = ARENA + 0x3400;
const FONTS: [u32; 2] = [ARENA + 0x3500, ARENA + 0x3600];
const LOADER: u32 = ARENA + 0x3700;
const FOG: u32 = ARENA + 0x3800;
const RIDERS: u32 = ARENA + 0x3900;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
/// the renderer's virtual and the pads' (+0x30 of the table at their +0x38)
const SLOTS: [u32; 2] = [0x30, 0x138];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 7] = [0x1863f8, 0x186350, 0x186368, 0x17b848, 0x17c2e0, 0x17b8a8, 0x180c70];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
    fn r8(&mut self, a: u32) -> u8;
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write(a, &[v]).unwrap();
    }
}
fn stack(a: u32) -> bool {
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        gi::RAND_FX | gi::HEAP_MARK | gi::TRICK_TABLE | gi::EFFECTS => (0, 0),
        gi::RIDERS_CTOR => (4, 0),
        gi::POOL_INIT | gi::HUD_SIZE | gi::HELPER_CTOR | gi::FONT_LOAD | gm::ALLOC | gm::FILL | ap::BIG_OPEN => (3, 0),
        gm::RACE_RESTART | gm::REPLAY_RESET | gm::CAREER_RACE_RESET => (1, 0),
        gm::LESSON_RESTART => (5, 0),
        gi::DEVICE | gi::LISTS_INIT | gi::LISTS_LAYERS | gi::LESSON_INIT | gi::NODE_CTOR | gi::HUD_CTOR
        | gi::OVERLAYS_CTOR | gi::SNOW_CTOR | gi::COURSE_WORLD | gi::STRCPY | gi::MUSIC | gi::SPEECH_CTOR
        | gi::REPLAY_CAM_CTOR | gi::CONTROLLERS | gi::SHADOWS_CTOR | gi::HUD_OPTION_SET | gi::VIEW_MODE_SET => (2, 0),
        gi::SNOW_RATE | gi::BRIGHTNESS_SET => (1, 1),
        gi::SNOW_DRIFT => (1, 2),
        f if f == RACE_FUNCS[1] => (3, 0),
        f if f == slot(0x30) || f == slot(0x138) => (2, 0),
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
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (the app's step, which the load screen reads)
    log.push(io.r32(APPOBJ + 8));
    for (k, &a) in w[..ni].iter().enumerate() {
        if stack(a) && !(f == gi::STRCPY && k == 0) {
            log.extend((0..4).map(|j| token(io.r32(a + 4 * j))));
        }
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    match f {
        gm::FILL => {
            for j in 0..w[2] / 4 {
                io.w32(w[0] + 4 * j, w[1]);
            }
        }
        // (the race left a player count (+0x7c) and an AI count (+0x88))
        gi::NODE_CTOR if w[1] == 7 => {
            io.w32(w[0] + 0x7c, g.below(3));
            io.w32(w[0] + 0x88, g.below(5));
        }
        // (the courses: thirteen records of 0x94 bytes, their fog colour and attract chance)
        gi::COURSES_CTOR => {
            for k in 0..13 {
                let e = w[0] + 0x94 * k;
                for j in 0..3 {
                    io.w32(e + 0x48 + 4 * j, g.below(0x200));
                }
                io.w32(e + 0x54, g.pick(&[0.0f32, 0.25, 0.5, 1.0]).to_bits());
            }
        }
        gi::STRCPY => {
            for j in 0..15 {
                let b = io.r8(w[1] + j);
                io.w8(w[0] + j, b);
            }
            io.w8(w[0] + 15, 0);
        }
        _ => {}
    }
    let answer = match f {
        gm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        gi::DEVICE => DEV,
        gi::RIDERS_CTOR => RIDERS,
        ap::EFFECTS_CTOR => LOADER,
        ap::STRLEN => g.pick(&[0u32, 5]),
        // (some draws exactly 0, 0.25 and 0.5, the courses' chances)
        gi::RAND_FX => g.pick(&[r, r, 0, 0x0020_0000, 0x0040_0000]),
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
        let (ni, nf) = arity(f);
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

#[test]
fn game_init_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    r.allow(0x17e148);
    for f in REAL {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        gi::WORLD_RESET, gi::DEVICE, gi::SEED_GAME, gi::POOL_INIT, gi::LISTS_INIT, gi::LISTS_LAYERS, gi::LESSON_INIT,
        gi::NODE_CTOR, gi::LIST_CTOR, gi::RACE_LIST_CTOR, gi::RIDERS_CTOR, gi::WORLD_CTOR, gi::TRICKS_CTOR,
        gi::COURSES_CTOR, gi::REPLAY_INIT, gi::CAMERAS, gi::HUD_CTOR, gi::HUD_SIZE, gi::OVERLAYS_CTOR, gi::RAND_FX,
        gi::SNOW_CTOR, gi::SNOW_RATE, gi::SNOW_DRIFT, gi::PAUSE_INIT, gi::COURSE_WORLD, gi::HEAP_MARK, gi::MUSIC,
        gi::TRICK_TABLE, gi::SPEECH_CTOR, gi::HELPER_CTOR, gi::REPLAY_CAM_CTOR, gi::CONTROLLERS, gi::SHADOWS_CTOR,
        gi::EFFECTS, gi::HUD_OPTION_GET, gi::HUD_OPTION_SET, gi::VIEW_MODE_GET, gi::VIEW_MODE_SET,
        gi::BRIGHTNESS_GET, gi::BRIGHTNESS_SET, gi::RACE_MENU_CTOR, gi::SOUND_START, gi::FONT_LOAD, gi::STRCPY,
        gm::ALLOC, gm::FILL, gm::RACE_RESTART, gm::REPLAY_RESET, gm::CAREER_RACE_RESET, gm::LESSON_RESTART,
        ap::STRLEN, ap::BIG_OPEN, ap::BIG_CLOSE, ap::EFFECTS_CTOR,
    ];
    stubs.extend(RACE_FUNCS);
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
    let mut race_vt = vec![0u8; RACE_VT.1 as usize];
    r.mem.read(RACE_VT.0, &mut race_vt).unwrap();
    let mut g = Rng(0x1417_0000_0000_0001);
    for case in 0..3000u32 {
        let regions = vec![
            (DATA.0, data.clone()),
            (RACE_VT.0, race_vt.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        let mut m = Mem { regions };
        // (random words, none in the stack's range)
        for (at, n) in [(ARENA, SIZE), (CAREER.0, CAREER.1)] {
            for k in 0..n / 4 {
                m.w32(at + 4 * k, g.next() & !0x0100_0000);
            }
        }
        let c = gm::CAREER;
        m.w32(c + 0x27dc, g.below(7));
        m.w32(c + 0x2afc, g.pick(&[0u32, 1, 2, 4, 5, 6, 6]));
        m.w32(c + 0x2af8, g.pick(&[u32::MAX, 0, 3, 12, 13, 20]));
        m.w32(c + 0x1678, g.pick(&[0u32, 1, 2, 0x100]));
        m.w32(c + 0x167c, g.pick(&[0u32, 1, 2, 0x100]));
        // (the app: its course table, renderer, pads, fonts; the step it counts)
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x24, ARENA + 0x40);
        m.w32(APPOBJ + 0x5c, FONTS[0]);
        m.w32(APPOBJ + 0x60, FONTS[1]);
        m.w32(APPOBJ + 0x724, R);
        let n = g.below(14);
        m.w32(APPOBJ + 0x64, n);
        for k in 0..n {
            m.w32(APPOBJ + 0x64 + 0x84 + 0x84 * k, g.pick(&[k, k + 1, 3]));
        }
        m.w32(R + 0x10, VT);
        m.w32(R + 0x47c, FOG);
        m.w32(DEV + 0x38, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        m.w32(gm::SOUND, ARENA + 0x80);
        m.w32(RIDERS + 0x4c, g.below(2));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        gm::game_init(&mut m, &mut env, G, SCRATCH);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, G as i32 as i64 as u64);
        r.call(0x17e148).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(*log.borrow(), env.log, "case {case}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
}

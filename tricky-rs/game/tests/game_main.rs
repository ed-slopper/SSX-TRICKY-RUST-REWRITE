//! `tricky_game::game_main` (board row E1d) against the original in the function runner (TRICKY_ELF; skipped
//! without it): `cGame`'s constructor, destructor, enter, leave, restart, frame (`cGame::Update`, the results switch,
//! the object lists, the pause test), draw, and `Game_Main`. Every case builds a random game object (its race and
//! riders, lesson, overlays, replay and the rest), app, renderer, pause menu, career flags and lists in the arena and
//! the game's data; one function then runs on both sides. The game's own functions here and the application's
//! constructors run for real on both sides; every other callee is a stand-in answering from the case's seed: the
//! pause button (which writes a player), the overlay manager (the case's), the renderer's state, and the objects'
//! virtuals (a table of stand-ins). The calls are compared with their arguments (vectors on the stack read by
//! value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_main as gm;
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
/// the game's words: the application's table (its destructor)
const GAME: [(u32, u32); 1] = [(0x385b00, 0x40)];
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const SYS: (u32, u32) = (0x343000, 0x800);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x5000;
const APPOBJ: u32 = ARENA + 0x100;
const VT: u32 = ARENA + 0x1000;
/// the race's and the +0x2a0 object's own table (+8, +0x114)
const VT2: u32 = ARENA + 0x1200;
const R: u32 = ARENA + 0x1400;
const G: u32 = ARENA + 0x1800;
const RACE: u32 = ARENA + 0x1c00;
const LESSON_OBJ: u32 = ARENA + 0x1e00;
const VOBJ: u32 = ARENA + 0x1f00;
fn obj(k: u32) -> u32 {
    ARENA + 0x2000 + 0x100 * k
}
const HEAP: (u32, u32) = (0x0110_0000, 0x4000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot2(off: u32) -> u32 {
    0x0182_0000 + off
}
const SLOTS2: [u32; 3] = [8, 0x70, 0x78];
const SLOTS: [u32; 18] =
    [8, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80, 0x98, 0xb0, 0x108, 0x140, 0x148, 0x1c8, 0x1d0, 0x1e0, 0x1f0, 0x24, 0x34];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 9] = [0x17b748, 0x17c530, 0x1861f0, 0x186348, 0x17b938, 0x17c468, 0x181878, 0x1814c0, 0x1815a0];

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

fn arity(f: u32, app_dtor: u32) -> (usize, usize) {
    match f {
        gm::LESSON_RESTART => (5, 0),
        // (no arguments: the game leaves a stale a0)
        gm::TRICKS_FREE | gm::EFFECTS_FREE | gm::EXEC_CREATE => (0, 0),
        gm::OVERLAYS_PAGE | gm::ALLOC | gm::FILL | gm::APP_RUN => (3, 0),
        gm::LIST_UPDATE | gm::LIST_PASS | gm::INPUT_PAUSE | gm::RACE_PLAYER | gm::PAUSE_OPEN_FOR | gm::OVERLAYS_PLAYER
        | gm::PAUSE_PAGE | gm::RACE_MENU_STATE | gm::RACE_MENU_PAUSE | gm::APP_SET_MODE | gm::LESSON_DTOR
        | gm::HUD_DTOR | gm::COURSE_UNLOAD | gm::LIBC_INIT | gm::FONT_COLOUR => (2, 0),
        f if f == gm::LESSON_RESET[0] || f == gm::FRONT_END_MODULE.0 => (2, 0),
        f if f == tricky_game::app::FONT_CTOR => (2, 0),
        gm::SOUND_TICK => (2, 1),
        gm::FONT_SIZE => (1, 4),
        f if f == slot(0x98) => (1, 4),
        f if f == slot(0xb0) => (1, 6),
        f if f == slot(0x1c8) => (3, 1),
        f if f == slot(0x1f0) || f == slot(0x34) => (3, 0),
        f if f == slot(8) || f == slot(0x68) || f == slot(0x140) || f == slot(0x1d0) || f == slot(0x1e0) || f == slot(0x24) => (2, 0),
        f if f == app_dtor => (2, 0),
        f if f == slot2(0x78) => (3, 0),
        f if f == slot2(8) || f == slot2(0x70) => (2, 0),
        _ => (1, 0),
    }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
    app_dtor: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans.app_dtor.get());
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..4).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == gm::INPUT_PAUSE {
        io.w32(w[1], g.below(4));
    }
    let answer = match f {
        gm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        gm::OVERLAYS_MANAGER => VOBJ,
        gm::INPUT_PAUSE => g.pick(&[0xdu32, 0xd, 0, 3]),
        gm::IS_PAUSED | gm::PAUSE_OPEN | gm::OVERLAYS_BUSY | gm::RACE_PLAYER | gm::CAREER_TO_FRONT_END => g.pick(&[0u32, 0, 1]),
        gm::CAREER_TWO_PLAYERS => g.pick(&[0u32, 1, 2]),
        f if f == slot(0x70) || f == slot(0x78) => g.pick(&[1u32, 1, 0]),
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
        let (ni, nf) = arity(f, self.ans.app_dtor.get());
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(6, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 16] = [
    0x17e0f8, 0x1814c0, 0x1815a0, 0x181878, 0x181638, 0x180e00, 0x180c70, 0x180d08, 0x180df0, 0x1811a0, 0x1818c0,
    0x181eb0, 0x182060, 0x182068, 0x17ecf8, 0x1820b0,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    let buf = SCRATCH;
    Some(match f {
        0x17e0f8 => gm::game_ctor(m, a[0]),
        0x1814c0 => return { gm::update_object_lists(m, e, a[0]); None },
        0x1815a0 => gm::can_pause(m, e, a[0]),
        0x181878 => return { gm::update_paused(m, e, a[0]); None },
        0x181638 => return { gm::update(m, e, a[0], buf); None },
        0x180e00 => return { gm::update_or_show_results(m, e, a[0], buf); None },
        0x180c70 => return { gm::restart(m, e, a[0]); None },
        0x180d08 => return { gm::enter(m, e, a[0]); None },
        0x180df0 => gm::leave(m, a[0]),
        0x1811a0 => gm::draw(m, e, a[0], buf),
        0x1818c0 => return { gm::results_font(m, e); None },
        0x181eb0 => return { gm::module_dtor(m, e, a[0], a[1]); None },
        0x182060 | 0x182068 => return { gm::nothing(); None },
        0x17ecf8 => return { gm::game_dtor(m, e, a[0], a[1], buf); None },
        0x1820b0 => gm::game_main(m, e, a[0], a[1]),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn game_object_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let app_dtor = u32::from_le_bytes(game[0].1[0x1c..0x20].try_into().unwrap());
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0), app_dtor: Cell::new(app_dtor) });
    let mut stubs = vec![
        gm::IS_PAUSED, gm::UPDATE_CAMERAS, gm::LAYOUT_VIEWS, gm::LESSON_UPDATE, gm::DRAW_FRAME, gm::LIST_UPDATE,
        gm::LIST_PASS, gm::PAUSE_OPEN, gm::PAUSE_UPDATE, gm::PAUSE_OPEN_FOR, gm::PAUSE_PAGE, gm::OVERLAYS_BUSY,
        gm::OVERLAYS_PLAYER, gm::OVERLAYS_PAGE, gm::OVERLAYS_MANAGER, gm::RACE_MENU_STATE, gm::RACE_MENU_PAUSE,
        gm::INPUT_PAUSE, gm::RACE_PLAYER, gm::FADE_UPDATE, gm::FADE_DRAW, gm::SOUND_TICK, gm::SPEECH_TICK,
        gm::CAREER_TO_FRONT_END, gm::CAREER_TWO_PLAYERS, gm::APP_SET_MODE, gm::ALLOC, gm::FILL, gm::FRONT_END_MODULE.0,
        gm::RACE_RESTART, gm::REPLAY_RESET, gm::LESSON_RESTART, gm::CAREER_RACE_RESET, gm::SPEECH_RESET,
        gm::SOUND_START, gm::FONT_COLOUR, gm::FONT_SIZE, gm::FREE, gm::LESSON_DTOR, gm::FONT_RELEASE,
        gm::REPLAY_STOP, gm::RACE_FREE, gm::WORLD_DTOR, gm::RIDERS_DTOR, gm::TRICKS_FREE, gm::LISTS_DTOR,
        gm::POOL_RESET, gm::HUD_DTOR, gm::COURSE_UNLOAD, gm::EFFECTS_FREE, gm::LIBC_INIT, gm::EXEC_CREATE,
        gm::APP_RUN, tricky_game::app::BIG_INIT, tricky_game::app::FONT_CTOR, app_dtor,
    ];
    stubs.extend(gm::RESULTS_MODULES.map(|x| x.0));
    stubs.extend(gm::SOUND_STOP);
    stubs.extend(gm::LESSON_RESET);
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS2.map(slot2));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..18).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e62_0000_0000_0001);
    let per = 500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
        regions.extend(game.iter().cloned());
        regions.extend([
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ]);
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..CAREER.1 / 4 {
            m.w32(CAREER.0 + 4 * k, g.next());
        }
        for at in [0x32b394u32, 0x32de8c, 0x32e5ec - 0x2afc + 0x2afc] {
            m.w32(at, g.below(4));
        }
        for off in [0x2afcu32, 0x3704, 0x3708, 0x370c, 0x3710] {
            m.w32(gm::CAREER + off, g.pick(&[0u32, 1, 1, 6, 2]));
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(APPOBJ + 0x730, VOBJ);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        m.w32(VOBJ + 0x42c, VOBJ + 0x80);
        m.w32(VOBJ + 0x80 + 0xc, g.below(2));
        m.w32(gm::PAUSE_MENU, g.below(2));
        m.w32(gm::PAUSE_MENU + 0x38, g.next());
        for off in SLOTS2 {
            m.w16(VT2 + off, g.pick(&[0u16, 4]));
            m.w32(VT2 + off + 4, slot2(off));
        }
        // (the game: its race and riders, lesson or none, the objects it destroys, its flags)
        for at in [0x14u32, 0x18, 0x20, 0x34, 0x38, 0x44, 0x80] {
            m.w32(G + at, if g.below(4) == 0 { 0 } else { obj((at / 4) % 0x10) });
        }
        for k in 0..0x20 {
            m.w32(obj(k) + 8, VT);
            m.w32(obj(k) + 0x114, VT2);
        }
        m.w32(G + 0x2a0, if g.below(4) == 0 { 0 } else { obj(0x1f) });
        m.w32(G + 0x1c, RACE);
        m.w32(RACE + 8, VT2);
        m.w32(RACE + 0x1c, g.pick(&[0u32, 1, 2, 5, 5, 6]));
        let n = g.below(5);
        m.w32(RACE + 0x7c, n);
        for i in 0..4 {
            let rd = obj(0x10 + i);
            m.w32(RACE + 0x8c + 4 * i, rd);
            m.w32(rd + 0x870, rd + 0x40);
            m.w32(rd + 0x40, rd + 0x50);
            m.w32(rd + 0x54, g.pick(&[0u32, 1, 1]));
            m.w32(rd, rd + 0x60);
            m.w32(rd + 0x60 + 0x464, rd + 0x80);
        }
        m.w32(G + 0x40, if g.below(3) == 0 { 0 } else { LESSON_OBJ });
        m.w32(LESSON_OBJ + 0x20, g.pick(&[0u32, 1, 5, 9, 10]));
        m.w32(LESSON_OBJ + 0x4c, g.below(2));
        m.w32(G + 0x4c, g.below(2));
        m.w32(G + 0x54, g.below(2));
        m.w32(G + 0x58, g.pick(&[0u32, 0, 1]));
        m.w32(G + 0x50, g.below(100));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x181eb0 => [obj(5), g.below(4), 0, 0],
            0x17ecf8 => [G, g.below(4), 0, 0],
            0x1820b0 => [g.below(4), ARENA + 0x40, 0, 0],
            _ => [G, 0, 0, 0],
        };
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
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

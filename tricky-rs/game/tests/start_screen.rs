//! `tricky_game::start_screen` (board row E1f1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the start screen's enter, start, leave, draw, its destructors and `StartScreenSingle_Create`, and the
//! small functions beside them. Every case builds a random screen, app, renderer, memory card and pool in the arena
//! and the game's data; one function then runs on both sides. The game's own functions here (the module's destructor
//! and `Color_SetRGB`) run for real on both sides; every other callee is a stand-in answering from the case's seed:
//! the renderer's and the card's virtuals (a table of stand-ins), the allocator (a heap of junk) and the rest. The
//! calls are compared with their arguments (vectors on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_scene as sc;
use tricky_game::game_main as gm;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
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
const POOL: (u32, u32) = (0x345e00, 0x100);
const RECORD: (u32, u32) = (0x3cdd00, 0x80);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x6000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x900;
const S: u32 = ARENA + 0x1000;
/// the renderer's table
const VT: u32 = ARENA + 0x5000;
/// the card's and the screen's own table (+0x60, +0x40)
const VT2: u32 = ARENA + 0x5400;
const CARD: u32 = ARENA + 0x5800;
const SET: u32 = ARENA + 0x5900;
const RGB: u32 = ARENA + 0x5a00;
const HEAP: (u32, u32) = (0x0110_0000, 0x8000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot2(off: u32) -> u32 {
    0x0182_0000 + off
}
const SLOTS: [u32; 15] = [0x58, 0x60, 0x70, 0x78, 0x80, 0x98, 0xb0, 0x108, 0x138, 0x140, 0x148, 0x1c8, 0x1d0, 0x1e0, 0x1f0];
const SLOTS2: [u32; 2] = [0x40, 0x60];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 2] = [0x181eb0, 0x182040];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
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
        ss::CARD_NEW => (0, 0),
        ss::FONT_LOAD | ss::SPRITES_PICK | gm::ALLOC | gm::FILL => (3, 0),
        ss::FONT_CTOR | ss::FONT_DTOR | ss::REND_COLOUR | gm::APP_SET_MODE | ss::APP_SWITCH | ss::SPRITE_SET_RECT
        | ss::TEXT_BOX_DTOR | gm::FONT_COLOUR | sc::POOL_FREE => (2, 0),
        ss::REND_ALPHA => (1, 1),
        ss::TEXTURED_RECT => (1, 8),
        ss::SPRITE_DRAW => (2, 4),
        gm::FONT_SIZE => (1, 4),
        f if f == slot(0x98) => (1, 4),
        f if f == slot(0xb0) => (1, 6),
        f if f == slot(0x1c8) => (3, 1),
        f if f == slot(0x1f0) => (3, 0),
        f if f == slot(0x138) || f == slot(0x140) || f == slot(0x1d0) || f == slot(0x1e0) => (2, 0),
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
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..4).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        gm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        ss::CARD_NEW => CARD,
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

const FUNCS: [u32; 19] = [
    0x182d08, 0x182dd8, 0x182ed0, 0x1831d8, 0x184318, 0x1843a0, 0x1843a8, 0x1843e0, 0x1843f8, 0x184450, 0x1844a0,
    0x184508, 0x184528, 0x184538, 0x184578, 0x184580, 0x1845a0, 0x185410, 0x1859b8,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], fa: [f32; 4]) -> Option<u32> {
    let buf = SCRATCH;
    let fv = fa.map(F);
    Some(match f {
        0x182d08 => return { ss::enter(m, e, a[0]); None },
        0x182dd8 => return { ss::start(m, e, a[0], buf); None },
        0x182ed0 => ss::leave(m, e, a[0], buf),
        0x1831d8 => ss::draw(m, e, a[0], buf),
        0x184318 => ss::create(m, e),
        0x1843a0 | 0x184578 => return { ss::nothing(); None },
        0x1843a8 => return { ss::reset_record(m); None },
        0x1843e0 => return { ss::draw_scaled(m, e, a[0], a[1], fv[0], fv[1], fv[2], fv[3]); None },
        0x1843f8 => ss::running(m, a[0]),
        0x184450 | 0x1859b8 => return { ss::screen_dtor(m, e, a[0], a[1]); None },
        0x1844a0 => return { gm::module_dtor(m, e, a[0], a[1]); None },
        0x184508 => return { ss::free_if(m, e, a[0], a[1]); None },
        0x184528 => return { ss::set_rgb_from(m, a[0], a[1]); None },
        0x184538 => return { sc::free_block(m, e, a[0]); None },
        0x184580 => return { ss::set_rgb(m, a[0], fv[0], fv[1], fv[2]); None },
        0x1845a0 => return { ss::single_start(m, e, a[0], buf); None },
        0x185410 => ss::single_draw(m, e, a[0], buf),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn start_screen_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        ss::FONT_LOAD, ss::FONT_CTOR, ss::FONT_DTOR, ss::FONT_RELEASE, ss::SPRITES_RESET, ss::SPRITES_PICK,
        ss::CARD_NEW, ss::CARD_FREE, ss::REND_COLOUR, ss::REND_ALPHA, ss::START_MODULE_CTOR, ss::APP_SWITCH,
        ss::CAREER_START, ss::TEXTURED_RECT, ss::SPRITE_SET_RECT, ss::SPRITE_DRAW, ss::TEXT_BOX_CTOR,
        ss::TEXT_BOX_DTOR, gm::ALLOC, gm::FILL, gm::APP_SET_MODE, gm::FONT_COLOUR, gm::FONT_SIZE, gm::FREE,
        sc::POOL_FREE, sc::HEAP_FREE,
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS2.map(slot2));
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
    let mut g = Rng(0x51a7_0000_0000_0001);
    let per = 400u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (POOL.0, vec![0; POOL.1 as usize]),
            (RECORD.0, vec![0; RECORD.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..RECORD.1 / 4 {
            m.w32(RECORD.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for off in SLOTS2 {
            m.w16(VT2 + off, g.pick(&[0u16, 4]));
            m.w32(VT2 + off + 4, slot2(off));
        }
        m.w32(S, VT2);
        m.w32(S + 4, CARD);
        m.w32(CARD, VT2);
        m.w32(S + 0x44, g.below(2));
        m.w32(S + 0x3cb0, g.below(2));
        m.w32(sc::POOL, 0x0130_0000);
        m.w32(sc::POOL + 4, 0x1000);
        for k in 1..3 {
            let v = g.f(0.0, 4.0);
            m.w32(SET + 4 * k, g.pick(&[1.0f32, 0.5, 1.25, v]).to_bits());
        }
        for k in 0..3 {
            m.w32(RGB + 4 * k, g.f(0.0, 1.0).to_bits());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let block = g.pick(&[0x0130_0000u32, 0x0130_0010, 0x0130_0ff0, 0x0130_1000, 0x012f_fff0, 0x0100_0000]);
        let a = match func {
            0x1843e0 => [SET, g.below(2), 0, 0],
            0x184450 | 0x1844a0 | 0x184508 | 0x1859b8 => [S, g.below(4), 0, 0],
            0x184528 => [S + 0x100, RGB, 0, 0],
            0x184580 => [S + 0x100, 0, 0, 0],
            0x184538 => [block, 0, 0, 0],
            _ => [S, 0, 0, 0],
        };
        let fa = [g.f(-320.0, 640.0), g.f(-240.0, 480.0), g.f(0.0, 300.0), g.f(0.0, 300.0)];
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a, fa);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        for (k, v) in fa.iter().enumerate() {
            r.cpu.set_f(12 + k, *v);
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

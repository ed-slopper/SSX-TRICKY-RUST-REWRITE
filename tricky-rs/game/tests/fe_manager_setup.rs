//! `tricky_game::fe_manager` (board row G9b2d2a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the front end manager's two constructors, its destructor and vf4 (the start). Every case builds a
//! random manager (its twelve models, scenes, players, 3D scene and video manager), app, renderer and career in the
//! arena; one function then runs on both sides. The game's own functions it calls that are already checked run for
//! real on both sides (the player count, the effects loader's free and delete, the module's destructor); every other
//! callee is a stand-in answering from the case's seed: the objects' virtuals (two tables of stand-ins; the boot
//! films' is the game's, its +0x60 stood in for), the models' loaded test (which says yes in time) and the rest. The
//! calls are compared with their arguments, the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP};
use tricky_game::fe_manager as fm;
use tricky_game::fe_manager::setup as su;
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
const SYS: (u32, u32) = (0x347100, 0x200);
/// the boot films' table, the game's, and its +0x60
const FILM_VT: (u32, u32) = (0x3b5000, 0x100);
const FILM_START: u32 = 0x2ab910;
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x900;
const FE: u32 = ARENA + 0x1000;
const VT: u32 = ARENA + 0x2000;
/// the objects' tables: one for each place an object keeps one (+0, +4, +8, +0xc, +0x10), each with its own adjust
fn vt2(k: u32) -> u32 {
    ARENA + 0x3800 + 0x180 * k
}
fn obj(k: u32) -> u32 {
    ARENA + 0x2800 + 0x80 * k
}
const HEAP: (u32, u32) = (0x0110_0000, 0x1000);
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot2(off: u32) -> u32 {
    0x0182_0000 + off
}
/// the renderer's virtuals and the objects'
const SLOTS: [u32; 6] = [0x58, 0x60, 0x98, 0x120, 0x140, 0x148];
const SLOTS2: [u32; 5] = [8, 0x18, 0x60, 0x68, 0x120];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 4] = [0x263670, 0x17b938, 0x17c468, 0x181eb0];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        ALLOC | FILL => (3, 0),
        tricky_game::fe_charselect::SEEN => (4, 0),
        su::LESSON_INIT | su::COURSE_UNLOAD | su::PLAYER_DTOR | su::STACK_DTOR | su::DRAWN_DTOR | su::LIGHTS_DTOR2
        | su::MODEL_FINISH | su::FIRST_SCREEN | fm::STACK_PUSH | tricky_game::fe_trickbook::CLEAR_SLOT
        | tricky_game::fe_riderprofile::SET_FILM => (2, 0),
        f if f == slot(0x98) => (1, 4),
        // (the renderer's +0x140 is called with whatever a1 holds: not logged)
        f if f == slot(0x120) || f == slot2(8) => (2, 0),
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

fn standin(f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        su::MODEL_LOADED => g.pick(&[0u32, 1, 1, 2]),
        su::TWO_PLAYERS => g.pick(&[1u32, 2, 0x101]),
        _ => r & !0x0100_0000,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, _m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let (ni, nf) = arity(f);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(8, 0);
        let (a, c) = standin(f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 4] = [0x263280, 0x265ca0, 0x2633d0, 0x2637a0];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    Some(match f {
        0x263280 => fm::fe_ctor(m, e, a[0], a[1]),
        0x265ca0 => fm::fe_ctor_base(m, e, a[0]),
        0x2633d0 => return { fm::fe_dtor(m, e, a[0], a[1]); None },
        0x2637a0 => return { fm::fe_start(m, e, a[0]); None },
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn front_end_setup_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        su::LIST_CTOR, su::LESSON_INIT, su::TWO_PLAYERS, su::LIGHTS_DTOR, su::FADES_RESET, su::MODEL_UNLOAD,
        su::STACK_CLEAR, su::DRAWN_CLEAR, su::COURSE_UNLOAD, su::PLAYER_DTOR, su::FONT_RELEASE, su::LESSON_STOP,
        su::LESSON_RESET, su::LISTS_CLEAR, su::POOL_CLEAR, su::STACK_DTOR, su::DRAWN_DTOR, su::LIGHTS_DTOR2,
        su::MODEL_LOAD, su::MODEL_LOADED, su::MODEL_FINISH, su::FRAME_WAIT, su::FIRST_SCREEN, ALLOC, FILL,
        fm::STACK_PUSH, tricky_game::fe_trickbook::CLEAR_SLOT, tricky_game::fe_charselect::SEEN, tricky_game::fe_riderprofile::SET_FILM, tricky_game::fe_title::BASE_CTOR, tricky_game::game_main::FREE,
        FILM_START,
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS2.map(slot2));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, _mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut film_vt = vec![0u8; FILM_VT.1 as usize];
    r.mem.read(FILM_VT.0, &mut film_vt).unwrap();
    let mut g = Rng(0x9b2d_0000_0000_0001);
    let per = 2000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (FILM_VT.0, film_vt.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
        ];
        let mut m = Mem { regions };
        // (random words, none in the stack's range)
        for (at, n) in [(ARENA, SIZE), (CAREER.0, CAREER.1), (SYS.0, SYS.1)] {
            for k in 0..n / 4 {
                m.w32(at + 4 * k, g.next() & !0x0100_0000);
            }
        }
        m.w32(tricky_game::fe_characcept::CAREER + 0x3260, g.pick(&[0u32, 1, 1, 2]));
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(APPOBJ + 0x30, if g.below(2) == 0 { 0 } else { obj(30) });
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for k in 0..5 {
            for off in SLOTS2 {
                m.w16(vt2(k) + off, 4 * k as u16);
                m.w32(vt2(k) + off + 4, slot2(off));
            }
        }
        // (the manager's objects: each with its table where the game looks, some missing)
        for k in 0..32 {
            for (j, at) in [0u32, 4, 8, 0xc, 0x10].into_iter().enumerate() {
                m.w32(obj(k) + at, vt2(j as u32));
            }
        }
        for k in 0..12 {
            // (the start's models are always there; the destructor may find some gone)
            m.w32(FE + 0x20 + 4 * k, if g.below(4) == 0 && func == 0x2633d0 { 0 } else { obj(k) });
        }
        for (k, at) in [0xcu32, 0x10, 0x14, 0x18, 0xa4, 0x124].into_iter().enumerate() {
            let none = g.below(4) == 0 && at != 0xc && at != 0x10 && at != 0x14 && at != 0xa4;
            m.w32(FE + at, if none { 0 } else { obj(12 + k as u32) });
        }
        for (k, at) in [0x74u32, 0x78].into_iter().enumerate() {
            m.w32(FE + at, if g.below(4) == 0 && func == 0x2633d0 { 0 } else { obj(20 + k as u32) });
        }
        m.w8(FE + 4, g.below(3) as u8);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x263280 => [FE, g.below(2), 0, 0],
            0x2633d0 => [FE, g.below(4), 0, 0],
            _ => [FE, 0, 0, 0],
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

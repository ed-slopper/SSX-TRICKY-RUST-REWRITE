//! `tricky_game::app` (board row E1c) against the original in the function runner (TRICKY_ELF; skipped without
//! it): the application's constructor, the course table and its lookups, the course archives, the strings and the
//! effects loader. Every case builds a random application (its course table of up to 13 records, strings,
//! effects loader, renderer's fog record and game object) and app pointer in the arena; one function then runs on
//! both sides. The table's own functions run for real on both sides; every other callee is a stand-in answering
//! from the case's seed (`strcmp` matching a record's name about half the time, `strlen` sometimes 0, the
//! localised string pointing at a random wide string with or without its quotes). The calls are compared with
//! their arguments, the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
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
/// the game's words: the courses
const GAME: [(u32, u32); 1] = [(0x332f00, 0x200)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const APPOBJ: u32 = ARENA + 0x100;
const GAMEOBJ: u32 = ARENA + 0xa00;
const REND: u32 = ARENA + 0xb00;
const FOG: u32 = ARENA + 0xc00;
const WSTR: u32 = ARENA + 0xd00;
const HEAP: (u32, u32) = (0x0110_0000, 0x1000);
const SCRATCH: u32 = 0x01f8_0000;
/// the copies of the pool-or-heap free beside the table
const FREE_COPIES: [u32; 2] = [0x186170, 0x1861b0];
const POOL: (u32, u32) = (0x345e00, 0x100);

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn arity(f: u32) -> (usize, usize) {
    match f {
        ap::ALLOC | ap::FILL | ap::BIG_OPEN | ap::SPRINTF => (3, 0),
        ap::FONT_CTOR | ap::STRCMP | ap::STRCPY | ap::LOC_STRING | ap::COURSE_RESOLVE_PS2_DTOR => (2, 0),
        0x23d420 => (2, 0),
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

fn standin(f: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, _) = arity(f);
    let log: Vec<u32> = w[..ni].to_vec();
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        ap::ALLOC => {
            if r % 8 == 0 {
                0
            } else {
                let at = ans.heap.get();
                ans.heap.set(at + 0x100);
                at
            }
        }
        ap::STRCMP => r % 2,
        ap::STRLEN => g.pick(&[0u32, 3, 9]),
        ap::FILE_SIZE => g.pick(&[0u32, 0x40, 0x1000, 0x8000_0000]),
        ap::LOC_STRING => WSTR + 2 * g.below(4),
        ap::WSTR_LEN => g.below(6),
        ap::EFFECTS_CTOR => GAMEOBJ,
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, _m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let (ni, _) = arity(f);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let (a, c) = standin(f, &w, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 27] = [
    0x17c530,
    0x17b748, 0x17b848, 0x17b8a8, 0x17b938, 0x17bdf0, 0x17c0a8, 0x17c118, 0x17c1d0, 0x17c2e0, 0x17c360, 0x17c380,
    0x17c3a8, 0x17c468, 0x186168, 0x186170, 0x1861b0, 0x1861f0, 0x186218, 0x186268, 0x186348, 0x186350, 0x186368,
    0x186380, 0x1863f8, 0x186440, 0x1842a8,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    let t = a[0];
    Some(match f {
        0x17b748 => ap::app_ctor(m, e, a[0], a[1]),
        0x17c530 => ap::appman_ctor(m, a[0], a[1]),
        0x17b848 => return { ap::course_open_big(m, e, a[0], a[1]); None },
        0x17b8a8 => return { ap::create_effects_loader(m, e, a[0]); None },
        0x17b938 => return { ap::free_effects_loader(m, e, a[0]); None },
        0x17bdf0 | 0x17c1d0 => return { ap::nothing(); None },
        0x17c0a8 => return { ap::loc_load(m, e, a[0]); None },
        0x17c118 => return { ap::loc_unload(m, e, a[0]); None },
        0x17c2e0 => return { ap::course_close_big(m, e, a[0]); None },
        0x17c360 => ap::app_course_id(m, a[0], a[1]),
        0x17c380 => ap::app_find_course(m, a[0], a[1]),
        0x17c3a8 => ap::app_course_info(a[0], a[1]),
        0x17c468 => return { ap::effects_loader_delete(m, e, a[0], a[1]); None },
        0x186168 => return { ap::course_resolve_dtor(m, e, a[0], a[1]); None },
        0x186170 | 0x1861b0 => return { tricky_game::fe_scene::free_block(m, e, a[0]); None },
        0x1861f0 => ap::table_new(m, t),
        0x186218 => return { ap::table_delete(m, e, t, a[1]); None },
        0x186268 => ap::table_fill(m, e, t),
        0x186348 => return { ap::table_clear(m, t); None },
        0x186350 => ap::table_info(t, a[1]),
        0x186368 => ap::table_entry(t, a[1]),
        0x186380 => ap::table_find_by_name(m, e, t, a[1]),
        0x1863f8 => ap::table_find_by_id(m, t, a[1]),
        0x186440 => ap::table_id(m, t, a[1]),
        0x1842a8 => ap::loc_unquoted(m, e, a[1]),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn app_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let stubs = [
        ap::BIG_INIT, ap::BIG_OPEN, ap::BIG_CLOSE, ap::ALLOC, ap::FILL, ap::HEAP_FREE, ap::FONT_CTOR,
        ap::EFFECTS_CTOR, ap::FREE, ap::FILE_SIZE, ap::STRLEN, ap::STRCMP, ap::STRCPY, ap::SPRINTF, ap::LOC_STRING,
        ap::WSTR_LEN, ap::COURSE_RESOLVE_PS2_DTOR, 0x23d420,
    ];
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, _mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (v, call) = standin(f, &w, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e61_0000_0000_0001);
    let per = 400u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
        regions.extend(game.iter().cloned());
        regions.extend([
            (POOL.0, vec![0; POOL.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ]);
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(0x345e48, 0x0130_0000);
        m.w32(0x345e4c, 0x1000);
        // (the app: its course table of up to 13 records, strings, effects loader, renderer and game object)
        let t = APPOBJ + 0x64;
        m.w32(t, g.pick(&[0u32, 1, 5, 12, 13, 0xffff_ffff]));
        for k in 0..13 {
            m.w32(t + 0x84 + 0x84 * k, g.pick(&[0u32, 3, 12, 13, 14, 0xffff_ffff]));
        }
        m.w32(APPOBJ + 0x50, g.pick(&[0u32, 0x0120_0000]));
        m.w32(APPOBJ + 0x54, g.pick(&[0u32, 0x80, 0x2000]));
        m.w32(APPOBJ + 0x30, g.pick(&[0u32, 0x0120_0100]));
        m.w32(APPOBJ + 0x724, REND);
        m.w32(REND + 0x47c, FOG);
        for k in 0..4 {
            m.w16(WSTR + 2 * k, g.pick(&[0x22u16, 0x41, 0x22]));
        }
        for k in 4..16 {
            m.w16(WSTR + 2 * k, g.pick(&[0x22u16, 0x42]));
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let block = g.pick(&[0x0130_0000u32, 0x0130_0010, 0x0130_1000, 0x012f_fff0]);
        let k = g.pick(&[0u32, 1, 4, 11, 12, 13, 3, 14]);
        let a = match func {
            0x186170 | 0x1861b0 => [block, 0, 0, 0],
            0x1861f0 | 0x186218 | 0x186268 | 0x186348 | 0x186350 | 0x186368 | 0x186380 | 0x1863f8 | 0x186440 => {
                [t, if func == 0x186380 { ARENA + 0x40 } else { k }, 0, 0]
            }
            0x17c468 | 0x186168 => [ARENA + 0x40, g.below(4), 0, 0],
            0x1842a8 => [0, g.below(0x100), 0, 0],
            _ => [APPOBJ, k, 0, 0],
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
        let _ = FREE_COPIES;
    }
}

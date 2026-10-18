//! `tricky_game::fe_dvdbuild` (board row G9y8d) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the DVD screen's character list (0x296870) and jukebox (0x297178). Every case builds a random screen
//! (an existing character list or none, the title) and app in the arena; one builder then runs on both sides with a
//! random item. Every callee is a stand-in on both sides answering from the case's seed: the heap (handed out in
//! order), the constructors (which give their object a table of stand-ins and answer it), the localised strings
//! (a heading now and then), the list's walk, and the widget classes' own virtuals, read from the game's tables. The
//! calls are compared with their arguments (a vector on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use tricky_game::fe_dvdbuild as db;
use tricky_game::fe_widgets::{BUFFERED_TEXT_VTABLE, ICON_VTABLE, LIST_INIT, TEXT_ENTRY_VTABLE, UPDATE_COLOR};
use tricky_game::fe_world::MENU_LIST_VTABLE;
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
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
/// the game's words the builders read: the titles, songs, scales and gap; the text entry's, icon's, buffered text's
/// and menu list's tables
const GAME: [(u32, u32); 4] = [(0x348000, 0x200), (0x3ae600, 0x400), (0x3b6e00, 0x600), (0x3b7000, 0x1200)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const S: u32 = ARENA;
const APPOBJ: u32 = ARENA + 0x200;
const VT: u32 = ARENA + 0x300;
const OBJS: u32 = ARENA + 0x500;
const STRS: u32 = ARENA + 0x900;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 16] = [8, 0x10, 0x20, 0x28, 0x50, 0x70, 0x100, 0x108, 0x110, 0x118, 0x120, 0x138, 0x140, 0x148, 0x150, 0x158];
const SLOTS_F8: u32 = 0xf8;

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

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
    /// the widget classes' virtuals called through the game's tables (one argument each)
    vfs: RefCell<Vec<u32>>,
    /// the list's walk: how many widgets it has left
    walk: Cell<u32>,
    /// the virtuals that take only the object (the layouts)
    lone: RefCell<Vec<u32>>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        db::MENU_CTOR => (6, 0),
        db::COLOUR_RUN => (5, 0),
        db::TEXT_CTOR => (4, 0),
        ALLOC | FILL => (3, 0),
        db::LOC_STRING | db::LIST_NTH | db::STATE_ADD | WIDGET_PLACE => (2, 0),
        f if f == slot(0x140) || f == slot(0x10) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if ans.lone.borrow().contains(&f) => (1, 0),
        f if ans.vfs.borrow().contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector on the stack by value; the colour run's is its third)
    if ni >= 2 && stack(w[1]) {
        log.extend((0..4).map(|k| token(io.r32(w[1] + 4 * k))));
    }
    if f == db::COLOUR_RUN {
        log.extend((0..4).map(|k| io.r32(w[2] + 4 * k)));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        db::TEXT_CTOR | db::MENU_CTOR => {
            io.w32(w[0] + 0xc, VT);
            for k in 0..4 {
                io.w32(w[0] + 0x80 + 4 * k, g.f(-50.0, 300.0).to_bits());
            }
            w[0]
        }
        // (a string: a heading when it starts with the marker)
        db::LOC_STRING => STRS + 0x10 * (r % 4),
        db::LIST_NTH => {
            ans.walk.set(r % 4);
            OBJS + 0x40 * (r % 4)
        }
        db::LIST_END => {
            let left = ans.walk.get();
            if left == 0 {
                1
            } else {
                ans.walk.set(left - 1);
                0
            }
        }
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
        let (ni, nf) = arity(f, self.ans);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(4, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 2] = [0x296870, 0x297178];

#[test]
fn dvd_lists_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers {
        seed: Cell::new(0),
        n: Cell::new(0),
        heap: Cell::new(HEAP.0),
        vfs: RefCell::default(),
        walk: Cell::new(0),
        lone: RefCell::default(),
    });
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let word = |at: u32| {
        let (base, b) = game.iter().find(|(base, b)| (*base..*base + b.len() as u32).contains(&at)).unwrap();
        u32::from_le_bytes(b[(at - base) as usize..][..4].try_into().unwrap())
    };
    for (vt, off) in [
        (ICON_VTABLE, 0x138u32),
        (ICON_VTABLE, 0x108),
        (ICON_VTABLE, 0x110),
        (BUFFERED_TEXT_VTABLE, 0x148),
        (BUFFERED_TEXT_VTABLE, 0x110),
        (BUFFERED_TEXT_VTABLE, 0x70),
        (BUFFERED_TEXT_VTABLE, SLOTS_F8),
        (MENU_LIST_VTABLE, 0x148),
        (MENU_LIST_VTABLE, 0x140),
        (TEXT_ENTRY_VTABLE, 0x50),
        (TEXT_ENTRY_VTABLE, 0x140),
        (TEXT_ENTRY_VTABLE, 0x28),
    ] {
        let f = word(vt + off + 4);
        if !ans.vfs.borrow().contains(&f) {
            ans.vfs.borrow_mut().push(f);
        }
    }
    for (vt, off) in [(MENU_LIST_VTABLE, 0x140u32), (TEXT_ENTRY_VTABLE, 0x140)] {
        ans.lone.borrow_mut().push(word(vt + off + 4));
    }
    let mut stubs = vec![
        ALLOC, FILL, db::MENU_CTOR, db::TEXT_CTOR, db::LOC_STRING, db::COLOUR_RUN, db::LIST_NTH, db::LIST_END,
        db::STATE_ADD, WIDGET_PLACE, UPDATE_COLOR, LIST_INIT,
    ];
    stubs.extend(ans.vfs.borrow().iter().copied());
    stubs.extend(SLOTS.map(slot));
    stubs.push(slot(SLOTS_F8));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mark = u16::from_le_bytes(data[(db::HEADING_MARK - DATA.0) as usize..][..2].try_into().unwrap());
    let mut g = Rng(0x9ebb_0000_0000_0001);
    let per = 800u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
        regions.extend(game.iter().cloned());
        regions.extend([
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ]);
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..HEAP.1 / 4 {
            m.w32(HEAP.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        for off in SLOTS.iter().copied().chain([SLOTS_F8]) {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // the screen: an existing list or none, the title; the walk's widgets
        m.w32(S + 0x6c, g.pick(&[0, OBJS + 0x200]));
        m.w32(S + 0x64, OBJS + 0x240);
        for k in 0..10 {
            let o = OBJS + 0x40 * k;
            m.w32(o + 0xc, VT);
            m.w32(o + 8, OBJS + 0x40 * ((k + 1) % 4));
        }
        // the strings: a heading (the marker) or a line
        for k in 0..4 {
            m.w16(STRS + 0x10 * k, g.pick(&[mark, mark ^ 1]));
        }
        let item = g.pick(&[0u32, 1, 2, 5, 6, 7, 11, 12, 0x8000_0001]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        match func {
            0x296870 => db::character_list(mm, e, S, item, SCRATCH),
            0x297178 => db::jukebox(mm, e, S, SCRATCH),
            _ => unreachable!(),
        }
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, S as i32 as i64 as u64);
        r.cpu.set_gpr(5, item as i32 as i64 as u64);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
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

//! `tricky_game::fe_optbuild` (board rows G9y8e..g) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the options screen's builders. Every case builds a random options screen (its groups, page and
//! sub-menus) and app in the arena and sets the controller page's once flag either way; one builder then runs on
//! both sides. Every callee is a stand-in on both sides answering from the case's seed: the heap (handed out in
//! order), the constructors (which give their object a table of stand-ins and answer it), the sprite rectangles,
//! the finds, and the widget classes' own virtuals, read from the game's tables. The calls are compared with their
//! arguments (a vector on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use tricky_game::fe_optbuild as ob;
use tricky_game::fe_widgets::{GROUP_VTABLE, ICON_VTABLE, LIST_INIT, TEXT_ENTRY_VTABLE, UPDATE_COLOR};
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
/// the game's words the builders read and keep: the controller page's tables and once flag, its pictures' x; the
/// widget classes' tables
const GAME: [(u32, u32); 5] = [(0x348200, 0x300), (0x3d3500, 0x100), (0x3a9e00, 0x200), (0x3ae600, 0x400), (0x3b6e00, 0x600)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const S: u32 = ARENA;
const APPOBJ: u32 = ARENA + 0x200;
const VT: u32 = ARENA + 0x300;
const OBJS: u32 = ARENA + 0x500;
fn obj(k: u32) -> u32 {
    OBJS + 0x100 * k
}
const RECTS: u32 = ARENA + 0xe00;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 8] = [8, 0x28, 0x50, 0x108, 0x110, 0x140, 0x148, 0x150];

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
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        ob::MENU_CTOR => (6, 0),
        ob::TEXT_CTOR => (4, 0),
        ALLOC | FILL => (3, 0),
        ob::SPRITE_RECT | ob::GROUP_ADD | ob::FIND | WIDGET_PLACE => (2, 0),
        f if f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if ans.vfs.borrow().contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector on the stack by value)
    if ni >= 2 && stack(w[1]) {
        log.extend((0..4).map(|k| token(io.r32(w[1] + 4 * k))));
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
        ob::TEXT_CTOR | ob::MENU_CTOR => {
            io.w32(w[0] + 0xc, VT);
            for k in 0..4 {
                io.w32(w[0] + 0x80 + 4 * k, g.f(-50.0, 300.0).to_bits());
            }
            w[0]
        }
        ob::SPRITE_RECT => RECTS + 0x10 * (r % 4),
        ob::FIND => obj(r % 4),
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

const FUNCS: [u32; 1] = [0x2a36e8];

#[test]
fn options_builders_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0), vfs: RefCell::default() });
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
        (GROUP_VTABLE, 0x28),
        (TEXT_ENTRY_VTABLE, 0x110),
        (TEXT_ENTRY_VTABLE, 0x108),
        (TEXT_ENTRY_VTABLE, 0x50),
    ] {
        let f = word(vt + off + 4);
        if !ans.vfs.borrow().contains(&f) {
            ans.vfs.borrow_mut().push(f);
        }
    }
    let mut stubs = vec![
        ALLOC, FILL, ob::MENU_CTOR, ob::TEXT_CTOR, ob::SPRITE_RECT, ob::GROUP_ADD, ob::FIND, ob::PAGES_LAYOUT,
        WIDGET_PLACE, UPDATE_COLOR, LIST_INIT,
    ];
    stubs.extend(ans.vfs.borrow().iter().copied());
    stubs.extend(SLOTS.map(slot));
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
    let mut g = Rng(0x9ecc_0000_0000_0001);
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
        m.w32(ob::CONTROLLER_ONCE, g.below(2));
        m.w32(ob::CONTROLLER_X, g.f(0.0, 200.0).to_bits());
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // the screen: its group (+0x90), page (+0x8c), the third label group; the finds' items and sub-menus
        for (at, k) in [(0x90u32, 4u32), (0x8c, 5), (0xc8, 6)] {
            m.w32(S + at, obj(k));
        }
        for k in 0..8 {
            m.w32(obj(k) + 0xc, VT);
            m.w32(obj(k) + 0x114, obj(4 + k % 4));
        }
        for k in 0..16 {
            m.w32(RECTS + 4 * k, g.f(5.0, 120.0).to_bits());
        }
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
            0x2a36e8 => ob::controller_config(mm, e, S, SCRATCH),
            _ => unreachable!(),
        }
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, S as i32 as i64 as u64);
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

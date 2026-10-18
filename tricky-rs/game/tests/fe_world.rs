//! `tricky_game::fe_world` (board row G9y5) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random screen and app (its menu font's height) in the arena, a heap the
//! allocator stand-in hands out in order, random sprite rectangles and a widget with a random table for the find. One
//! of the row's three functions then runs on both sides with random arguments. Every callee is a stand-in on both
//! sides answering from the case's seed: the allocator, the sprite rectangles, the text and text box constructors
//! (which give their object a table of stand-ins), the group and world adds, the help bar, and the icon classes'
//! own virtuals, read from the game's tables. The calls are compared with their arguments (floats from f12; a vector
//! they are given read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use tricky_game::fe_charselect::{SET_STRING, SPRITE_RECT};
use tricky_game::fe_title::{ADD_WIDGET, HELP_BUTTONS, TEXT_CTOR};
use tricky_game::fe_trickbook::{TEXT_BOX, TEXT_BOX_SCALE};
use tricky_game::fe_widgets::{GROUP_ADD, ICON_VTABLE, LIST_INIT, UPDATE_COLOR};
use tricky_game::fe_world as fw;
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
/// the icon classes' and the menu list's tables (read, not written)
const TABLES: (u32, u32) = (0x3b6e00, 0x1400);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const S: u32 = ARENA;
const APPOBJ: u32 = ARENA + 0x200;
const FONT: u32 = ARENA + 0x280;
const BAR: u32 = ARENA + 0x300;
const W: u32 = ARENA + 0x400;
const TV: u32 = ARENA + 0x500;
const OBJ_VT: u32 = ARENA + 0x600;
const RECTS: u32 = ARENA + 0x800;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
const FIND: u32 = 0x0180_0010;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 4] = [0x50, 0x130, 0x138, 0x150];

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
    (0x01f0_0000..0x01ff_0000).contains(&a)
}
fn token(a: u32) -> u32 {
    if stack(a) { STACK_TOKEN } else { a }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
    /// the icon classes' own virtuals (taking one vector) and the menu list's +0x168 (taking only the list)
    vfs: RefCell<Vec<u32>>,
    list_vf: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        fw::MENU_CTOR => (6, 0),
        TEXT_CTOR | TEXT_BOX => (4, 0),
        ALLOC | FILL | HELP_BUTTONS => (3, 0),
        TEXT_BOX_SCALE | fw::TEXT_SET_SCALE => (1, 1),
        SPRITE_RECT | GROUP_ADD | ADD_WIDGET | SET_STRING | WIDGET_PLACE | FIND | fw::FIND_HASH_OP_DTOR => (2, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if f == ans.list_vf.get() => (1, 0),
        f if ans.vfs.borrow().contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector or operation by value; not the size call's buffer, which it only writes)
    if ni >= 2 && stack(w[1]) && f != slot(0x130) {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 300.0).to_bits());
        }
    }
    let answer = match f {
        ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        // (the constructors give their object the stand-ins' table, and answer it)
        TEXT_CTOR | TEXT_BOX => {
            io.w32(w[0] + 0xc, OBJ_VT);
            w[0]
        }
        SPRITE_RECT => RECTS + 0x10 * (r % 8),
        // (the world add may move the screen's bottom group: it is read again for the second add)
        ADD_WIDGET => {
            if r % 2 == 0 {
                io.w32(S + 0x44, BAR);
            }
            r
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

const FUNCS: [u32; 3] = [0x2720f8, 0x274000, 0x26fe50];

#[test]
fn world_group_matches_the_original() {
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
        vfs: RefCell::new(Vec::new()),
        list_vf: Cell::new(0),
    });
    // the game's tables the builders call through
    let mut tables = vec![0u8; TABLES.1 as usize];
    r.mem.read(TABLES.0, &mut tables).unwrap();
    let word = |at: u32| u32::from_le_bytes(tables[(at - TABLES.0) as usize..][..4].try_into().unwrap());
    for (vt, off) in [(ICON_VTABLE, 0x138u32), (ICON_VTABLE, 0x50), (fw::SCALED_ICON_VTABLE, 0x138), (fw::SCALED_ICON_VTABLE, 0x50)] {
        let f = word(vt + off + 4);
        if !ans.vfs.borrow().contains(&f) {
            ans.vfs.borrow_mut().push(f);
        }
    }
    ans.list_vf.set(word(fw::MENU_LIST_VTABLE + 0x16c));
    let mut stubs = vec![
        ALLOC, FILL, SPRITE_RECT, TEXT_CTOR, TEXT_BOX, TEXT_BOX_SCALE, fw::TEXT_SET_SCALE, GROUP_ADD, ADD_WIDGET,
        HELP_BUTTONS, SET_STRING, WIDGET_PLACE, UPDATE_COLOR, LIST_INIT, fw::MENU_CTOR, FIND, fw::FIND_HASH_OP_DTOR,
        ans.list_vf.get(),
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
    let mut g = Rng(0x9e55_0000_0000_0001);
    let per = 1500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (TABLES.0, tables.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (HEAP.0, vec![0; HEAP.1 as usize]),
                (SCRATCH, vec![0; 0x100]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..HEAP.1 / 4 {
            m.w32(HEAP.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x60, FONT);
        m.w32(FONT + 0xc, g.pick(&[12u32, 16, 21, 0x0100_0001, 0x0100_0003, 0x0300_0007, 0xffff_fff0, 0xfd00_0005]));
        for k in 0..32 {
            m.w32(RECTS + 4 * k, g.f(0.0, 120.0).to_bits());
        }
        for off in SLOTS {
            m.w16(OBJ_VT + off, g.pick(&[0u16, 4]));
            m.w32(OBJ_VT + off + 4, slot(off));
        }
        // the screen's help bar, and the find's widget and its table at +0x20
        m.w32(S + 0x48, BAR);
        m.w32(W + 0x20, TV);
        m.w16(TV + 0x10, g.pick(&[0u16, 0x20, 0xfff0]));
        m.w32(TV + 0x14, FIND);
        let any = g.next() | 0x8000_0000;
        let (a1, a2) = (any, g.pick(&[0u32, 0, any ^ 0x55]));
        for (at, b) in &m.regions {
            if *at != TABLES.0 {
                r.mem.write(*at, b).unwrap();
            }
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let ours = match func {
            0x2720f8 => { fw::build_world_group(mm, e, S, a1, a2, SCRATCH); none }
            0x274000 => { fw::create_list(mm, e, S, a1, SCRATCH); none }
            0x26fe50 => fw::find_by_hash(mm, e, W, a1, SCRATCH),
            _ => unreachable!(),
        };
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2720f8 => vec![S, a1, a2],
            0x274000 => vec![S, a1],
            _ => vec![W, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
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

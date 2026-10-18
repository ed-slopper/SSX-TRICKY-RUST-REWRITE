//! The rider bio's texts and the DVD screen's menus of board row G9y7 (batch 3) in `tricky_game::fe_misc` against
//! the original in the function runner (TRICKY_ELF; skipped without it). Every case builds a random screen (its
//! rider byte and bio groups) and app in the arena; one of the four builders then runs on both sides. Every callee is
//! a stand-in on both sides answering from the case's seed: the heap (handed out in order), the text constructors
//! (which give their object a table of stand-ins and answer it), the career's rider record and bio line, the placing,
//! the colour run, the state's add and the menu list's own virtuals, read from the game's table. The calls are
//! compared with their arguments (a vector on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP, WIDGET_PLACE};
use tricky_game::fe_misc as fm;
use tricky_game::fe_world::{MENU_CTOR, MENU_LIST_VTABLE};
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
/// the game's words the builders read: the bio's line ranges, the DVD menus' orders, strings, gap and scales, the
/// menu list's table
const GAME: [(u32, u32); 4] = [(0x3af900, 0x200), (0x348000, 0x200), (0x3b2900, 0x80), (0x3b7f00, 0x300)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const S: u32 = ARENA;
const APPOBJ: u32 = ARENA + 0x600;
const VT: u32 = ARENA + 0x700;
const REC: u32 = ARENA + 0x900;
const GROUPS: u32 = ARENA + 0xa00;
const HEAP: (u32, u32) = (0x0110_0000, 0x4000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 11] = [0x20, 0x50, 0x100, 0x108, 0x110, 0x118, 0x120, 0x138, 0x140, 0x150, 0x158];

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
    /// the menu list's own +0x140 and +0x148 (from the game's table)
    list_layout: Cell<u32>,
    list_add: Cell<u32>,
    /// the bio line answered: up to 11, or past it for the second text
    lines: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        MENU_CTOR => (6, 0),
        fm::COLOUR_RUN => (5, 0),
        fm::SCROLL_TEXT_CTOR | fm::TEXT_CTOR => (4, 0),
        fm::ALLOC | fm::FILL => (3, 0),
        fm::RIDER_RECORD | fm::RIDER_BIO_LINE | fm::LIST_ADD | fm::STATE_ADD | WIDGET_PLACE => (2, 0),
        f if f == slot(0x140) || f == ans.list_layout.get() => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 || f == ans.list_add.get() => (2, 0),
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
    if f == fm::COLOUR_RUN {
        log.extend((0..4).map(|k| io.r32(w[2] + 4 * k)));
    }
    // (the list's +0x190 as an item is added)
    if f == ans.list_add.get() {
        log.push(io.r32(w[0] + 0x190));
    }
    let r = ans.next();
    let answer = match f {
        fm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        fm::SCROLL_TEXT_CTOR | fm::TEXT_CTOR => {
            io.w32(w[0] + 0xc, VT);
            w[0]
        }
        fm::RIDER_RECORD => REC + 4 * (r % 4),
        fm::RIDER_BIO_LINE => r % ans.lines.get(),
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

const FUNCS: [u32; 4] = [0x284c70, 0x284f60, 0x297fa0, 0x298368];

#[test]
fn bio_texts_and_dvd_menus_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    // (the group add the texts reach runs as the original)
    r.allow(0x2762e0);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers {
        seed: Cell::new(0),
        n: Cell::new(0),
        heap: Cell::new(HEAP.0),
        list_layout: Cell::new(0),
        list_add: Cell::new(0),
        lines: Cell::new(12),
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
    ans.list_layout.set(word(MENU_LIST_VTABLE + 0x144));
    ans.list_add.set(word(MENU_LIST_VTABLE + 0x14c));
    let mut stubs = vec![
        fm::ALLOC, fm::FILL, fm::SCROLL_TEXT_CTOR, fm::TEXT_CTOR, fm::RIDER_RECORD, fm::RIDER_BIO_LINE, fm::LIST_ADD,
        fm::STATE_ADD, fm::COLOUR_RUN, MENU_CTOR, WIDGET_PLACE, ans.list_layout.get(), ans.list_add.get(),
    ];
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
    let mut g = Rng(0x9e99_0000_0000_0001);
    let per = 1000u32;
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
        m.w8(S + 0x2e, g.pick(&[0u8, 3, 0x7f, 0x80, 0xff]));
        for (k, at) in [0x500u32, 0x4f4].into_iter().enumerate() {
            m.w32(S + at, GROUPS + 0x100 * k as u32);
            m.w32(GROUPS + 0x100 * k as u32 + 0xc, VT);
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the first text reads the game's table only for lines 0..11; the second has none past 11)
        ans.lines.set(if func == 0x284f60 { 15 } else { 12 });
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
        let none = u32::MAX;
        let ours = match func {
            0x284c70 => { fm::rider_bio_text(mm, e, S, false, SCRATCH); none }
            0x284f60 => { fm::rider_bio_text(mm, e, S, true, SCRATCH); none }
            0x297fa0 => fm::dvd_character_menu(mm, e, S, SCRATCH),
            0x298368 => fm::dvd_demo_menu(mm, e, S, SCRATCH),
            _ => unreachable!(),
        };
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, S as i32 as i64 as u64);
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

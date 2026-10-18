//! `tricky_game::fe_mcard::build` (board row G9y4b, the memory card overlay's builder 0x28f3e0) against the original
//! in the function runner (TRICKY_ELF; skipped without it). Every case builds a random overlay (its operation, the
//! two-slot and quiet flags), its front end state's groups, a card answering a random number of slots and an app
//! with two fonts in the arena, then runs the builder on both sides. Every callee is a stand-in on both sides
//! answering from the case's seed: the heap (handed out in order), the constructors (which give their object a
//! table of stand-ins and answer it), the groups' adds, the placing, the format (which writes a few words) and the
//! widget classes' own virtuals the builder calls through the game's tables. The calls are compared with their
//! arguments (a vector or string on the stack read by value; floats from f12), the answer and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_mcard as mc;
use tricky_game::fe_misc::ICON_TEXT_VTABLE;
use tricky_game::fe_widgets::{BUFFERED_TEXT_VTABLE, ICON_VTABLE, UPDATE_COLOR};
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
/// the game's words the builder reads: the dialog's frames, its strings, the widget classes' tables
const GAME: [(u32, u32); 3] = [(0x3ae500, 0x40), (0x3b1000, 0x800), (0x3b6e00, 0x600)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const O: u32 = ARENA;
const ST: u32 = O + 0x42c;
const APPOBJ: u32 = ARENA + 0x3000;
const CARD: u32 = ARENA + 0x3100;
const CARDVT: u32 = ARENA + 0x3200;
const OBJ_VT: u32 = ARENA + 0x3400;
/// the state's root, title and bottom groups and the state's buttons
const GROUPS: u32 = ARENA + 0x3800;
fn group(k: u32) -> u32 {
    GROUPS + 0x100 * k
}
const HEAP: (u32, u32) = (0x0110_0000, 0x20000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const COUNT: u32 = 0x0182_01d8;
const SLOTS: [u32; 9] = [0x28, 0x50, 0x70, 0x128, 0x138, 0x140, 0x148, 0x158, 0x108];

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
    /// the widget classes' virtuals called through the game's tables (each handed one vector or string)
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
        mc::MENU_CTOR => (6, 0),
        mc::TEXT_CTOR | mc::TEXT_BOX => (4, 0),
        mc::ALLOC | mc::FILL | mc::FORMAT | mc::THING_CTOR => (3, 0),
        mc::CREATE_DIALOG => (2, 1),
        mc::THING_FLOAT => (1, 1),
        mc::WORLD_GROUP => (3, 0),
        mc::GROUP_ADD | mc::STATE_ADD | mc::PLACE | mc::SET_STRING | mc::STATE_BUTTON | mc::STATE_BUTTONS => (2, 0),
        f if f == slot(0x128) => (3, 0),
        f if f == slot(0x140) || f == COUNT => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if ans.vfs.borrow().contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector or string on the stack by value; not the bounds call's, which it only writes, nor the format's)
    if ni >= 2 && stack(w[1]) && f != slot(0x128) {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        mc::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        mc::TEXT_CTOR | mc::TEXT_BOX | mc::THING_CTOR | mc::MENU_CTOR => {
            // (the object made: the stand-ins' table, a random position)
            let o = w[0];
            io.w32(o + 0xc, OBJ_VT);
            for k in 0..4 {
                io.w32(o + 0x80 + 4 * k, g.f(-50.0, 300.0).to_bits());
            }
            o
        }
        mc::FORMAT => {
            for k in 0..4 {
                io.w32(w[0] + 4 * k, g.next() & 0x7f7f_7f7f);
            }
            r
        }
        f if f == slot(0x128) => {
            for k in 0..4 {
                io.w32(w[1] + 4 * k, g.f(0.0, 100.0).to_bits());
                io.w32(w[2] + 4 * k, g.f(100.0, 300.0).to_bits());
            }
            r
        }
        mc::STATE_BUTTON => group(3),
        COUNT => g.below(6),
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

const FUNC: u32 = 0x28f3e0;

#[test]
fn memory_card_overlay_builder_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    r.allow(FUNC);
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
        (BUFFERED_TEXT_VTABLE, 0x158u32),
        (BUFFERED_TEXT_VTABLE, 0x108),
        (BUFFERED_TEXT_VTABLE, 0x120),
        (ICON_VTABLE, 0x138),
        (ICON_TEXT_VTABLE, 0x138),
        (ICON_TEXT_VTABLE, 0x50),
    ] {
        let f = word(vt + off + 4);
        if !ans.vfs.borrow().contains(&f) {
            ans.vfs.borrow_mut().push(f);
        }
    }
    let mut stubs = vec![
        mc::ALLOC, mc::FILL, mc::CREATE_DIALOG, mc::WORLD_GROUP, mc::MENU_CTOR, mc::TEXT_CTOR, mc::TEXT_BOX,
        mc::SET_STRING, mc::GROUP_ADD, mc::THING_CTOR, mc::THING_FLOAT, mc::STATE_ADD, mc::PLACE, mc::SCREEN_LAYOUT,
        mc::MGR_QUIET, mc::FORMAT, mc::STATE_BUTTON, mc::STATE_BUTTONS, UPDATE_COLOR, tricky_game::fe_widgets::LIST_INIT,
        COUNT,
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
    let mut g = Rng(0x9e66_0000_0000_0001);
    for case in 0..1500u32 {
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
        m.w32(O, g.pick(&[0u32, 0, 1, 2]));
        m.w32(O + 4, g.pick(&[0u32, 1, 1, 2]));
        m.w32(O + 8, g.pick(&[0u32, 1, 1, 2]));
        m.w32(O + 0x224, CARD);
        m.w32(CARD, CARDVT);
        m.w16(CARDVT + 0x1d8, g.pick(&[0u16, 4]));
        m.w32(CARDVT + 0x1dc, COUNT);
        for off in SLOTS {
            m.w16(OBJ_VT + off, g.pick(&[0u16, 4]));
            m.w32(OBJ_VT + off + 4, slot(off));
        }
        for (k, at) in [(0u32, 0x38u32), (1, 0x40), (2, 0x44)] {
            m.w32(ST + at, group(k));
        }
        for k in 0..4 {
            m.w32(group(k) + 0xc, OBJ_VT);
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let ours = mc::build(&mut m, &mut env, O, SCRATCH);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, O as i32 as i64 as u64);
        r.call(FUNC).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case}: the answer");
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

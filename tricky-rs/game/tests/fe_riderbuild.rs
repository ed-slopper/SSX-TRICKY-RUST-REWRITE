//! `tricky_game::fe_riderbuild` (board rows G9y8a, G9y8b, G9y8c) against the original in the function runner (TRICKY_ELF;
//! skipped without it): the rider profile's frame (0x27e918), records panel (0x282d30), bio panel (0x27f600) and short
//! bio (0x285230). Every case builds a random screen
//! (its rider byte, group, buffers) and app in the arena and random career counts; one builder then runs on both
//! sides. Every callee is a stand-in on both sides answering from the case's seed: the heap (handed out in order),
//! the constructors (which give their object a table of stand-ins and answer it), the career's rider record, bio
//! line, values and medals, the copies and formats (which write a few words), and the widget classes' own virtuals,
//! read from the game's tables. The calls are compared with their arguments (a vector on the stack read by value),
//! the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP, WIDGET_PLACE};
use tricky_game::fe_riderbuild as rb;
use tricky_game::fe_widgets::{ICON_VTABLE, LIST_INIT, TEXT_ENTRY_VTABLE, UPDATE_COLOR};
use tricky_game::fe_world::{MENU_LIST_VTABLE, SCALED_ICON_VTABLE};
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
const CAREER: (u32, u32) = (0x32b000, 0x2000);
/// the game's words the builders read: the records' events and dash, the icon classes' and menu list's tables
const GAME: [(u32, u32); 5] =
    [(0x3af800, 0x300), (0x3b6e00, 0x600), (0x3b7f00, 0x300), (0x3ae600, 0x400), (0x332f00, 0x200)];
/// the short bio's wide strings
const STRINGS: (u32, u32) = (0x3d3000, 0x400);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const S: u32 = ARENA;
const APPOBJ: u32 = ARENA + 0x800;
const VT: u32 = ARENA + 0x900;
const REC: u32 = ARENA + 0xb00;
const RECTS: u32 = ARENA + 0xc00;
const GRP: u32 = ARENA + 0xd00;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 13] = [8, 0x20, 0x28, 0x50, 0x100, 0x108, 0x110, 0x118, 0x138, 0x140, 0x148, 0x150, 0x158];

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
    /// the widget classes' virtuals called through the game's tables (one vector or word each)
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
        rb::MENU_CTOR => (6, 0),
        rb::TEXT_CTOR => (4, 0),
        rb::FORMAT => (4, 0),
        ALLOC | FILL | rb::RECORD_VALUE | rb::MEDAL | rb::WSTR_FORMAT => (3, 0),
        rb::PLACE_CHILD => (2, 2),
        rb::RIDER_RECORD | rb::RIDER_BIO_LINE | rb::EVENT_NAME | rb::WSTR_COPY | rb::WRITE_NUMBER | rb::GROUP_ADD
        | rb::STATE_ADD | rb::SPRITE_RECT | WIDGET_PLACE => (2, 0),
        f if f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if ans.vfs.borrow().contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (mut ni, nf) = arity(f, ans);
    // (the format's fourth argument only where the format takes one: the records' leave a stray word there)
    if f == rb::FORMAT && w[1] != rb::FAVOURITE_FORMAT {
        ni = 3;
    }
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector on the stack by value)
    if ni >= 2 && stack(w[1]) {
        log.extend((0..4).map(|k| token(io.r32(w[1] + 4 * k))));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    // (the copies and formats write a few words at their target)
    if f == rb::WSTR_COPY || f == rb::FORMAT || f == rb::WSTR_FORMAT {
        for k in 0..2 {
            io.w32(w[0] + 4 * k, g.next() & 0x00ff_00ff);
        }
    }
    if f == rb::WRITE_NUMBER {
        for k in 0..2 {
            io.w32(w[1] + 4 * k, g.next() & 0x00ff_00ff);
        }
    }
    let answer = match f {
        ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        rb::TEXT_CTOR | rb::MENU_CTOR => {
            io.w32(w[0] + 0xc, VT);
            for k in 0..4 {
                io.w32(w[0] + 0x80 + 4 * k, g.f(-50.0, 300.0).to_bits());
            }
            // (a text's +0xa8 says whether it has a value)
            io.w32(w[0] + 0xa8, g.below(2));
            w[0]
        }
        rb::RIDER_RECORD => REC + 4 * (r % 4),
        rb::RIDER_BIO_LINE => r % 12,
        rb::MEDAL => g.pick(&[0u32, 1, 2, 3, u32::MAX]),
        rb::SPRITE_RECT => RECTS + 0x10 * (r % 4),
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

const FUNCS: [u32; 4] = [0x27e918, 0x282d30, 0x27f600, 0x285230];

#[test]
fn rider_profile_builders_match_the_original() {
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
        (ICON_VTABLE, 0x50),
        (ICON_VTABLE, 0x28),
        (SCALED_ICON_VTABLE, 0x138),
        (SCALED_ICON_VTABLE, 0x50),
        (MENU_LIST_VTABLE, 0xf8),
        (MENU_LIST_VTABLE, 0x148),
        (TEXT_ENTRY_VTABLE, 0x50),
    ] {
        let f = word(vt + off + 4);
        if !ans.vfs.borrow().contains(&f) {
            ans.vfs.borrow_mut().push(f);
        }
    }
    let mut stubs = vec![
        ALLOC, FILL, rb::MENU_CTOR, rb::TEXT_CTOR, rb::RIDER_RECORD, rb::RIDER_BIO_LINE, rb::EVENT_NAME,
        rb::RECORD_VALUE, rb::MEDAL, rb::PLACE_CHILD, rb::WSTR_COPY, rb::WRITE_NUMBER, rb::FORMAT, rb::GROUP_ADD,
        rb::STATE_ADD, rb::SPRITE_RECT, WIDGET_PLACE, UPDATE_COLOR, LIST_INIT, rb::WSTR_FORMAT,
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
    let mut g = Rng(0x9eaa_0000_0000_0001);
    let per = 600u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut career = vec![0u8; CAREER.1 as usize];
        for b in career.iter_mut() {
            *b = g.next() as u8;
        }
        let mut regions = vec![(DATA.0, data.clone()), (CAREER.0, career), (STRINGS.0, vec![0; STRINGS.1 as usize])];
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
        // (the counts small, none, or negative, so both branches are seen)
        for k in 0..0x200 {
            m.w32(rb::COUNTS_LEFT + 4 * k, g.pick(&[0u32, 3, 12, 0xffff_ffff]));
        }
        // (the riders' bests: counts and times either side of 0, their venues among the 13, the favourite's counts
        // sometimes all 0)
        for k in 0..12u32 {
            let b = rb::CAREER + 0x1c4 * k + 0x160;
            for off in [0u32, 8, 0x10, 0x18, 0x6c, 0x74, 0x7c, 0x84, 0x8c, 0x94] {
                m.w32(b + off, g.pick(&[0u32, 1, 7, 1234, 0xffff_fffe, 0x8000_0000]));
                m.w32(b + off + 4, g.below(13));
            }
            for off in [0x20u32, 0x28, 0x30] {
                m.w32(b + off, g.pick(&[-1.0f32, 0.0, 0.004, 12.34, 75.5, 1.0e6]).to_bits());
                m.w32(b + off + 4, g.below(13));
            }
            let none = g.below(4) == 0;
            for i in 0..13 {
                m.w32(b + 0x38 + 4 * i, if none { 0 } else { g.pick(&[0u32, 1, 5, 9, 0x8000_0001]) });
            }
        }
        m.w32(GAPP, APPOBJ);
        m.w8(S + 0x2e, g.pick(&[0u8, 3, 0x7f, 0x80, 0xff]));
        m.w32(S + 0x4f8, GRP);
        m.w32(S + 0x4f0, GRP);
        // (the short bio's group its own)
        m.w32(S + 0x4fc, GRP + 0x80);
        m.w32(GRP + 0x80 + 0xc, VT);
        m.w32(GRP + 0xc, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
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
        let none = u32::MAX;
        let ours = match func {
            0x27e918 => rb::build_profile_frame(mm, e, S, SCRATCH),
            0x282d30 => { rb::build_records(mm, e, S, SCRATCH); none }
            0x27f600 => { rb::build_bio(mm, e, S, SCRATCH); none }
            0x285230 => { rb::build_short_bio(mm, e, S, SCRATCH); none }
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

//! The options screen's small functions of board row G9y7 (batch 2) in `tricky_game::fe_misc` against the original
//! in the function runner (TRICKY_ELF; skipped without it): the three pages' reads and writes, the sound and game
//! applies, the refresh, the change handlers, the help line, the controller page's layout, the credits, the title's
//! arrows and the single-text page. Every case builds a random options screen and page (its three lists' items with
//! value widgets and chosen nodes, ids picked to reach every case), a sound player, a renderer, the career's
//! settings and a front end in the arena. One function then runs on both sides with random arguments. The pages'
//! refreshes and the bar's value set run as the original; every other callee is a stand-in on both sides answering
//! from the case's seed. The calls are compared with their arguments (floats from f12..f15; a vector on the stack
//! read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP, WIDGET_PLACE};
use tricky_game::fe_misc as fm;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widgets::{ICON_VTABLE, UPDATE_COLOR};
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
/// the help lines' string tables and the controller page's layouts
const TABLES: (u32, u32) = (0x348000, 0x600);
/// the icon's table, read for its size call
const ICON: (u32, u32) = (0x3b6e00, 0x400);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const P: u32 = ARENA;
const LISTS: u32 = ARENA + 0x200;
const VT: u32 = ARENA + 0x400;
const SND: u32 = ARENA + 0x600;
const SVT: u32 = ARENA + 0x680;
const REND: u32 = ARENA + 0x880;
const RVT: u32 = ARENA + 0x900;
const APPOBJ: u32 = ARENA + 0xa00;
const FE: u32 = ARENA + 0xb00;
const ITEMS: u32 = ARENA + 0x1000;
const NI: u32 = 8;
fn item(k: u32) -> u32 {
    ITEMS + 0x40 * k
}
const VWS: u32 = ARENA + 0x1400;
fn vw(k: u32) -> u32 {
    VWS + 0x200 * k
}
const NODES: u32 = ARENA + 0x2400;
fn node(k: u32) -> u32 {
    NODES + 0x100 * k
}
const OBJS: u32 = ARENA + 0x3400;
fn obj(k: u32) -> u32 {
    OBJS + 0x100 * k
}
const HEAP: (u32, u32) = (0x0110_0000, 0x2000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn sv(off: u32) -> u32 {
    0x0182_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0183_0000 + off
}
const SLOTS: [u32; 12] = [0x20, 0x28, 0x50, 0x70, 0x108, 0x110, 0x130, 0x138, 0x140, 0x148, 0x150, 0x160];
const SND_SLOTS: [u32; 17] = [
    0x50, 0x130, 0x138, 0x140, 0x148, 0x150, 0x158, 0x160, 0x168, 0x170, 0x178, 0x190, 0x198, 0x1a0, 0x1a8, 0x1b0, 0x1b8,
];
const SND_FLOATS: [u32; 5] = [0x158, 0x168, 0x178, 0x198, 0x1a8];
const REND_SLOTS: [u32; 4] = [0x58, 0x60, 0x98, 0x120];

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
    /// the icon class's size call and show / hide (read from the game's table)
    icon_size: Cell<u32>,
    icon_show: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        fm::ALLOC | fm::FILL => (3, 0),
        fm::SCROLL_TEXT_CTOR | fm::TEXT_CTOR => (4, 0),
        fm::FIND | fm::NTH | fm::SET_STRING | fm::SET_STRING_ID | fm::STATE_ADD | fm::PAD_RUMBLE_SET | WIDGET_PLACE => {
            (2, 0)
        }
        f if f == rv(0x98) => (1, 4),
        f if f == rv(0x120) => (2, 0),
        f if f & 0xffff_0000 == 0x0183_0000 => (1, 0),
        f if f & 0xffff_0000 == 0x0182_0000 => {
            if SND_FLOATS.contains(&(f & 0xffff)) {
                (1, 1)
            } else if [0x50, 0x138, 0x148, 0x1b8].contains(&(f & 0xffff)) {
                (2, 0)
            } else {
                (1, 0)
            }
        }
        f if f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if f == ans.icon_size.get() || f == ans.icon_show.get() => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector on the stack by value; not the size call's buffer, which it only writes)
    if ni >= 2 && stack(w[1]) && f != slot(0x130) {
        log.extend((0..4).map(|k| token(io.r32(w[1] + 4 * k))));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 200.0).to_bits());
        }
    }
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
        fm::FIND => item(r % NI),
        fm::NTH => obj(r % 4),
        f if f == rv(0x58) || f == rv(0x60) => g.pick(&[640u32, 448, 0x8000_0001, 0xffff_ffff, r]),
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

const FUNCS: [u32; 19] = [
    0x29e988, 0x29ea90, 0x29ebc0, 0x29ec70, 0x29ed50, 0x29ee30, 0x29f178, 0x29f508, 0x29fb40, 0x2a9350, 0x2a9720,
    0x2a9848, 0x2a99a0, 0x2a9c50, 0x2aa998, 0x2a9488, 0x2a8380, 0x2a8520, 0x2a7fd8,
];

#[test]
fn options_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    // the pages' refreshes and the bar's value set
    for f in [0x29ef50u32, 0x29f430, 0x29f5f0, 0x2b3950] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0), icon_size: Cell::new(0), icon_show: Cell::new(0) });
    let mut icon = vec![0u8; ICON.1 as usize];
    r.mem.read(ICON.0, &mut icon).unwrap();
    let at = (ICON_VTABLE + 0x13c - ICON.0) as usize;
    ans.icon_size.set(u32::from_le_bytes(icon[at..at + 4].try_into().unwrap()));
    let at = (ICON_VTABLE + 0x2c - ICON.0) as usize;
    ans.icon_show.set(u32::from_le_bytes(icon[at..at + 4].try_into().unwrap()));
    let mut stubs = vec![
        fm::ALLOC, fm::FILL, fm::SCROLL_TEXT_CTOR, fm::TEXT_CTOR, fm::FIND, fm::NTH, fm::NODE_INDEX, fm::SET_STRING,
        fm::SET_STRING_ID, fm::STATE_ADD, fm::PAD_RUMBLE_SET, fm::PAD_RUMBLE, fm::ARROW_LEFT, fm::ARROW_RIGHT,
        WIDGET_PLACE, UPDATE_COLOR, wg::BAR_SET, fm::BRIGHTNESS, ans.icon_size.get(), ans.icon_show.get(),
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SND_SLOTS.map(sv));
    stubs.extend(REND_SLOTS.map(rv));
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
    let mut tables = vec![0u8; TABLES.1 as usize];
    r.mem.read(TABLES.0, &mut tables).unwrap();
    let mut g = Rng(0x9e88_0000_0000_0001);
    let per = 800u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut career = vec![0u8; CAREER.1 as usize];
        for b in career.iter_mut() {
            *b = g.next() as u8;
        }
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (CAREER.0, career),
                (TABLES.0, tables.clone()),
                (ICON.0, icon.clone()),
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
        // (the career's settings words small, as the game keeps them)
        for k in 0..0x20 {
            m.w32(0x32b398 + 0x1650 + 4 * k, g.below(6));
        }
        // (the controller page's layouts given every mix of flags, not only the game's own)
        for &(n, t) in fm::CONTROLLER_LAYOUTS.iter() {
            for i in 0..n {
                m.w32(t + 16 * i + 8, g.pick(&[0u32, 4, 8, 0x20, 0x40, 0x0c, 0x24, 0x48, 0x60, 0x6c]));
            }
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(APPOBJ + 0x724, REND);
        m.w32(FE + 0xa4, SND);
        m.w32(SND, SVT);
        for off in SND_SLOTS {
            m.w16(SVT + off, g.pick(&[0u16, 8]));
            m.w32(SVT + off + 4, sv(off));
        }
        m.w32(REND + 0x10, RVT);
        for off in REND_SLOTS {
            m.w16(RVT + off, g.pick(&[0u16, 4]));
            m.w32(RVT + off + 4, rv(off));
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // the page: its three lists, arrows (+0xc0, +0xc4) and the screen's parts the others use
        for (k, at) in [0x80u32, 0x8c, 0x98].into_iter().enumerate() {
            m.w32(P + at, LISTS + 0x40 * k as u32);
            m.w32(LISTS + 0x40 * k as u32 + 0x140, node(g.below(NI)));
        }
        for at in [0xa4u32, 0xb0, 0xb4, 0x4c, 0xc0, 0xc4, 0xc8] {
            m.w32(P + at, obj(g.below(4)));
        }
        m.w32(P + 0x78, g.pick(&[u32::MAX, 0, 1, 2, 3, 4, 5]));
        for k in 0..4 {
            m.w32(obj(k) + 0xc, VT);
            for i in 0..4 {
                m.w32(obj(k) + 0x80 + 4 * i, g.f(-50.0, 300.0).to_bits());
            }
        }
        // the items: each a value widget with a chosen node (ids that reach every case) and a bar value
        for k in 0..NI {
            m.w32(item(k) + 0x114, vw(k));
            m.w32(vw(k) + 0xc, VT);
            m.w32(vw(k) + 0x140, node(g.below(NI)));
            m.w32(vw(k) + 0x160, g.f(0.0, 1.0).to_bits());
        }
        for k in 0..NI {
            m.w32(node(k), g.pick(&[0u32, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 0x12, 21]));
            m.w32(node(k) + 0xf0, node(g.below(NI)));
        }
        // the line chooser's item: an id it handles (0..7), or the end marker
        let line = item(g.below(NI));
        m.w32(line, g.pick(&[0u32, 1, 2, 3, 4, 5, 6, 7, 0xabcd_1234]));
        if func == 0x2a9350 {
            // (the chosen ids it reads must be 0..3, as the game reads no item for any other)
            for k in 0..NI {
                m.w32(node(k), g.below(4));
            }
            // (the last node now and then 6, the arrows' case; the game list's chosen node never it)
            m.w32(node(NI - 1), g.pick(&[6u32, 0, 1]));
            m.w32(LISTS + 0x40 + 0x140, node(g.below(NI - 1)));
        }
        if func == 0x2aa998 {
            m.w32(line, g.pick(&[0u32, 1, 2, 3, 4, 5, 0xabcd_1234]));
        }
        let changed = node(g.below(NI));
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
        let s = SCRATCH;
        let (ours, regs): (u32, Vec<u32>) = match func {
            0x29e988 => { fm::read_page_n(mm, e, P, 1); (none, vec![P]) }
            0x29ea90 => { fm::write_page_n(mm, e, P, 1); (none, vec![P]) }
            0x29ebc0 => { fm::read_page_n(mm, e, P, 2); (none, vec![P]) }
            0x29ec70 => { fm::write_page_n(mm, e, P, 2); (none, vec![P]) }
            0x29ed50 => { fm::read_page_n(mm, e, P, 3); (none, vec![P]) }
            0x29ee30 => { fm::write_page_n(mm, e, P, 3); (none, vec![P]) }
            0x29f178 => { fm::sound_apply(mm, e, P); (none, vec![P]) }
            0x29f508 => { fm::game_apply(mm, e, P); (none, vec![P]) }
            0x29fb40 => { fm::options_refresh(mm, e, P); (none, vec![P]) }
            0x2a9350 => { fm::game_line_arrows(mm, e, P, line); (none, vec![P, line]) }
            0x2a9720 => { fm::sound_on_change(mm, e, P, changed); (none, vec![P, 0, changed]) }
            0x2a9848 => { fm::game_on_change(mm, e, P, changed); (none, vec![P, 0, changed]) }
            0x2a99a0 => { fm::display_on_change(mm, e, changed); (none, vec![P, 0, changed]) }
            0x2a9c50 => { fm::nothing_2a9c50(); (none, vec![P]) }
            0x2aa998 => { fm::set_help_text(mm, e, P, line); (none, vec![P, line]) }
            0x2a9488 => { fm::controller_layout(mm, e, P, s); (none, vec![P]) }
            0x2a8380 => { fm::build_credits(mm, e, P, s); (none, vec![P]) }
            0x2a8520 => (fm::title_arrows(mm, e, P, s), vec![P]),
            0x2a7fd8 => (fm::build_single_text(mm, e, P, s), vec![P]),
            _ => unreachable!(),
        };
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
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

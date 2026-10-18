//! `tricky_game::fe_wentry` (board row G9x3a) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random entry, list box or name entry in the arena, with two children, an owner
//! and list nodes whose tables point at stand-ins (with random `this` shifts). The children's bounds and sizes are
//! written by their stand-ins from the case's seed. The name entry's 42 characters, its cell sizes and the front
//! end's player are random too. One of the row's functions then runs on both sides. The row's own helpers and the
//! widget's (`fe_widget`, `fe_wgroup`) run for real; every other callee is a stand-in on both sides. The calls are
//! compared with their float arguments, the colours, vectors and rectangles they are given (read by value), and the
//! memory after. The runner's stack is cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_venuesel::LOC_STRING;
use tricky_game::fe_wentry as we;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widget as fw;
use tricky_game::fe_widgets::UPDATE_COLOR;
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
const CELLS: (u32, u32) = (0x348400, 0x100);
const CHARS: (u32, u32) = (0x3b7f00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const W: u32 = ARENA;
const C1: u32 = ARENA + 0x400;
const C2: u32 = ARENA + 0x600;
const OWNER: u32 = ARENA + 0x800;
const VT: u32 = ARENA + 0x900;
const RVT: u32 = ARENA + 0xb00;
const APPOBJ: u32 = ARENA + 0xc00;
const RENDER: u32 = ARENA + 0xc80;
const FE: u32 = ARENA + 0xd00;
const SND: u32 = ARENA + 0xd80;
const SVT: u32 = ARENA + 0xda0;
const RECT: u32 = ARENA + 0xe00;
const VEC: u32 = ARENA + 0xe40;
const OUT: u32 = ARENA + 0xe80;
const OUT2: u32 = ARENA + 0xec0;
const FONT: u32 = ARENA + 0xf00;
const STRS: u32 = ARENA + 0xf40;
const NODES: u32 = ARENA + 0x1000;
const INPUT: u32 = ARENA + 0x1400;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// the objects' virtuals' stand-ins (by slot), the renderer's and the sound player's
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
const SND_CALL: u32 = 0x0182_0050;
const SLOTS: [u32; 25] = [
    8, 0x20, 0x28, 0x48, 0x50, 0x60, 0x68, 0x70, 0xb0, 0xc0, 0xd8, 0xe8, 0xf0, 0xf8, 0x100, 0x108, 0x110, 0x118,
    0x120, 0x128, 0x130, 0x138, 0x140, 0x150, 0x158,
];

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
fn readable(a: u32) -> bool {
    stack(a) || (ARENA..ARENA + SIZE - 0x40).contains(&a)
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

/// A callee's integer and float arguments
fn arity(f: u32) -> (usize, usize) {
    match f {
        we::SPRITE_DRAW => (2, 4),
        we::PRINT_CENTRED | we::PRINTF_CENTRED => (2, 3),
        we::GET_RECT | we::SET_WORLD_POS | LOC_STRING | fw::DRAWER_COLOUR | fw::THING_DTOR | SND_CALL => (2, 0),
        f if f == rv(0x1d0) => (2, 0),
        f if f == slot(0x128) => (3, 0),
        // (the virtuals that take only `this`)
        f if f == slot(0x48) || f == slot(0x60) || f == slot(0x68) || f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a colour, a vector, a rectangle or a string by value)
    let by_value = f == fw::DRAWER_COLOUR || f == we::SET_WORLD_POS || f == we::PRINTF_CENTRED
        || (f & 0xffff_0000 == 0x0180_0000 && ni == 2);
    if by_value && readable(w[1]) {
        log.extend(deref(io, w[1], 4));
    }
    // (the object's table as its children are destroyed; the button's gap as its label is given its text)
    if f == slot(8) {
        log.push(io.r32(W + 0xc));
    }
    if f == slot(0x150) {
        log.push(io.r32(W + 0x118));
    }
    let r = ans.next();
    // (a child's bounds and size, written where it is asked)
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x128) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(-100.0, 400.0).to_bits());
            io.w32(w[2] + 4 * k, g.f(-100.0, 400.0).to_bits());
        }
    }
    if f == slot(0x130) {
        for k in 0..4 {
            let v = g.f(0.0, 200.0);
            io.w32(w[1] + 4 * k, g.pick(&[0.0f32, 10.0, v]).to_bits());
        }
    }
    let answer = match f {
        f if f == rv(0x1d8) => r % 8,
        f if f == slot(0xe8) => r % 2,
        we::INPUT_LEFT | we::INPUT_RIGHT | we::INPUT_UP | we::INPUT_DOWN | we::INPUT_ACCEPT | we::INPUT_BACK => {
            (r % 3 == 0) as u32
        }
        we::NODE_IS_END => r % 2,
        we::LIST_FIRST | we::LIST_LAST => NODES + 0x40 * (r % 4),
        we::GET_RECT => RECT,
        LOC_STRING => STRS,
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
        fl.resize(4, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 34] = [
    0x2ac4b0, 0x2ae098, 0x2af0b8, 0x2af140, 0x2af1c8, 0x2af250, 0x2af2d8, 0x2af360, 0x2af3c8, 0x2af438, 0x2af4a0,
    0x2af510, 0x2af550, 0x2af5c0, 0x2af8c0, 0x2afb30, 0x2afe48, 0x2afec8, 0x2affd0, 0x2b0000, 0x2b0120, 0x2b0150,
    0x2b01f8, 0x2b0230, 0x2b0410, 0x2b0428, 0x2b0468, 0x2b04e0, 0x2b0528, 0x2b0850, 0x2b2990, 0x2b2998, 0x2b2a30,
    0x2b2cb0,
];
const MORE: [u32; 4] = [0x2b2d00, 0x2b30d0, 0x2b30d8, 0x2b3358];
const LAST: [u32; 1] = [0x2b35d0];
/// the widget's code they call, run as the original: its destructor, flags and placing; the node's and list's
/// destructors and vf1
const ALSO: [u32; 7] = [0x2adaa8, 0x2ada20, 0x2ada30, 0x2addb0, 0x2acf28, 0x2ace78, 0x2acf60];

fn kind(f: u32) -> u8 {
    match f {
        0x2affd0..=0x2b0228 | 0x2b2cb0 | 0x2b2d00 => b'l',
        0x2b0230..=0x2b0b90 | 0x2b2990..=0x2b2a74 => b'n',
        _ => b'e',
    }
}

#[test]
fn entries_and_name_entry_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let funcs: Vec<u32> = FUNCS.iter().chain(MORE.iter()).chain(LAST.iter()).copied().collect();
    for f in funcs.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    let mut stubs = vec![
        we::GET_RECT, we::NODE_INDEX, we::NODE_IS_END, we::LIST_FIRST, we::LIST_LAST, we::SET_WORLD_POS,
        we::SPRITE_DRAW, we::PRINT_CENTRED, we::PRINTF_CENTRED, we::INPUT_LEFT, we::INPUT_RIGHT, we::INPUT_UP,
        we::INPUT_DOWN, we::INPUT_ACCEPT, we::INPUT_BACK, LOC_STRING, fw::DRAWER_COLOUR, fw::THING_DTOR, UPDATE_COLOR,
        wg::LIST_DELETE_ALL, wg::LIST_INIT, wg::NODE_UNLINK, wg::FREE, rv(0x1d0), rv(0x1d8), SND_CALL,
    ];
    stubs.extend(SLOTS.map(slot));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let (data, cells, chars) = (read(&mut r, DATA), read(&mut r, CELLS), read(&mut r, CHARS));
    let mut g = Rng(0x9e1c_0000_0000_0001);
    let per = 1500u32;
    let mut drawn = 0usize;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (CELLS.0, cells.clone()),
                (CHARS.0, chars.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (its renderer, sprites, strings, font, front end and its sound player); the objects' table
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x60, FONT);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(RENDER + 0x10, RVT);
        for off in [0x1d0u32, 0x1d8] {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        m.w32(FE + 0xa4, SND);
        m.w32(SND, SVT);
        m.w16(SVT + 0x50, g.pick(&[0u16, 8]));
        m.w32(SVT + 0x54, SND_CALL);
        m.w32(FONT + 0xc, g.pick(&[8u32, 12, 16, 21]));
        for k in 0..8 {
            m.w32(RECT + 4 * k, g.f(0.0, 40.0).to_bits());
            m.w32(VEC + 4 * k, g.f(-50.0, 50.0).to_bits());
        }
        for o in [W, C1, C2, OWNER] {
            m.w32(o + 0xc, VT);
            for k in 0..4 {
                m.w32(o + 0x80 + 4 * k, g.f(-200.0, 500.0).to_bits());
            }
        }
        for k in 0..4 {
            m.w32(NODES + 0x40 * k + 0xc, VT);
            m.w32(NODES + 0x40 * k + 4, NODES + 0x40 * g.below(4));
            m.w32(NODES + 0x40 * k + 8, NODES + 0x40 * g.below(4));
        }
        for o in [0x10u32, 0x14, 0xa0] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, any]));
        }
        m.w32(W + 0x70, g.below(16));
        m.w32(W + 0xec, g.pick(&[0, OWNER, OWNER]));
        for k in 0..42 {
            let c = g.pick(&[0x41u8, 0x42, 0x5a, 0x30, 0x65, 0x64, 0x20, 0x2e, 0xc1]);
            m.w8(we::CHARS + k, c);
        }
        m.w32(we::CELL_W, g.f(10.0, 30.0).to_bits());
        m.w32(we::CELL_H, g.f(10.0, 30.0).to_bits());
        m.w32(we::END_SCALE, g.f(0.3, 0.9).to_bits());
        m.w8(FE + 5, g.pick(&[1u8, 2]));
        m.w32(INPUT + 0x40, g.pick(&[0u32, 1]));
        m.w32(INPUT, g.pick(&[0u32, 1, 1]));
        match kind(func) {
            b'l' => {
                m.w32(W + 0x110, g.pick(&[0, NODES, NODES + 0x40, NODES + 0xc0]));
            }
            b'n' => {
                let cols = g.pick(&[1u32, 2, 3, 5, 7, 10, 12, 41, 42]);
                m.w32(W + 0x110, g.below(42));
                m.w32(W + 0x114, cols);
                let sc = g.f(0.5, 2.0);
                m.w32(W + 0x118, g.pick(&[1.0f32, 0.75, sc]).to_bits());
                m.w32(W + 0x11c, g.pick(&[0, C1]));
                // (it tells its owner without looking)
                m.w32(W + 0xec, OWNER);
            }
            _ => {
                m.w32(W + 0x110, C1);
                m.w32(W + 0x114, g.pick(&[0, C2]));
                // (the button always has its label)
                if func == 0x2ae098 {
                    m.w32(W + 0x114, C2);
                }
                if matches!(func, 0x2af510 | 0x2af550 | 0x2b30d8 | 0x2b3358) {
                    m.w32(W + 0x110, g.pick(&[0, C1]));
                }
                m.w32(W + 0x118, g.f(0.0, 12.0).to_bits());
                m.w32(W + 0x11c, g.below(16));
                m.w32(W + 0x130, g.below(16));
                for o in [0x120u32, 0x124, 0x140, 0x144] {
                    let v = g.f(0.0, 300.0);
                    m.w32(W + o, g.pick(&[0.0f32, v, v]).to_bits());
                }
            }
        }
        let any = g.next();
        let a1 = g.pick(&[any, 0, 1, 0x41, 0x65]);
        let (a2, flags) = (g.next(), g.pick(&[0u32, 1, 2, 3]));
        let vec = VEC + g.pick(&[0u32, 0, 4]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let s = SCRATCH;
        let ours = match func {
            0x2ac4b0 => we::sprite_rect(mm, e, a1),
            0x2ae098 => { we::button_layout(mm, e, W, a1, a2, s); none }
            0x2af0b8 => { we::entry_set_colour(mm, e, W, 0x60, 0x50, VEC); none }
            0x2af140 => { we::entry_set_colour(mm, e, W, 0xac, 0x108, VEC); none }
            0x2af1c8 => { we::entry_set_colour(mm, e, W, 0xbc, 0x110, VEC); none }
            0x2af250 => { we::entry_set_colour(mm, e, W, 0xcc, 0x118, VEC); none }
            0x2af2d8 => { we::entry_set_colour(mm, e, W, 0xdc, 0x120, VEC); none }
            0x2af360 => { we::entry_set_word(mm, e, W, 0x14, 0x28, a1); none }
            0x2af438 => { we::entry_set_word(mm, e, W, 0x10, 0x20, a1); none }
            0x2af3c8 => { we::entry_set_flag(mm, e, W, 0xa0, 0xf0, a1); none }
            0x2af4a0 => { we::entry_set_flag(mm, e, W, 0xa8, 0x100, a1); none }
            0x2af510 => we::entry_on_input(mm, e, W, a1),
            0x2af550 => { we::entry_vf1(mm, e, W); none }
            0x2af5c0 => { we::entry_bounds(mm, e, W, OUT, OUT2, s); none }
            0x2af8c0 => { we::entry_size(mm, e, W, OUT, s); none }
            0x2afb30 => { we::entry_layout(mm, e, W, s); none }
            0x2afe48 => { we::entry_move(mm, e, W, vec); none }
            0x2afec8 => { we::entry_draw(mm, e, W); none }
            0x2affd0 => we::selected_index(mm, e, W),
            0x2b0000 => we::listbox_on_input(mm, e, W, a1),
            0x2b0120 => { we::listbox_vf1(mm, e, W); none }
            0x2b0150 => { we::listbox_draw(mm, e, W, s); none }
            0x2b01f8 => { we::listbox_size(mm, e, W, OUT); none }
            0x2b0230 => we::name_ctor(mm, e, W),
            0x2b0410 => we::name_get_char(mm, W),
            0x2b0428 => { we::name_set_char(mm, W, a1); none }
            0x2b0468 => { we::name_size(mm, W, OUT); none }
            0x2b04e0 => { we::name_vf1(mm, e, W); none }
            0x2b0528 => { we::name_draw(mm, e, W, s); none }
            0x2b0850 => we::name_on_input(mm, e, W, INPUT),
            0x2b2990 | 0x2b30d0 => we::entry_one(),
            0x2b2998 => { we::name_update(mm, e, W); none }
            0x2b2a30 => we::name_dtor(mm, e, W, flags),
            0x2b2cb0 => we::listbox_dtor(mm, e, W, flags),
            0x2b2d00 => we::listbox_ctor(mm, e, W, a1, a2),
            0x2b30d8 => { we::entry_update(mm, e, W); none }
            0x2b3358 => we::entry_dtor(mm, e, W, flags),
            0x2b35d0 => we::entry_ctor(mm, e, W),
            _ => unreachable!(),
        };
        drawn += env.log.iter().filter(|c| c.0 == we::PRINTF_CENTRED || c.0 == we::PRINT_CENTRED || c.0 == we::SPRITE_DRAW).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2ae098 | 0x2b2d00 => vec![W, a1, a2],
            0x2af0b8 | 0x2af140 | 0x2af1c8 | 0x2af250 | 0x2af2d8 => vec![W, VEC],
            0x2afe48 => vec![W, vec],
            0x2af5c0 => vec![W, OUT, OUT2],
            0x2af8c0 | 0x2b01f8 | 0x2b0468 => vec![W, OUT],
            0x2b0850 => vec![W, INPUT],
            0x2b2a30 | 0x2b2cb0 | 0x2b3358 => vec![W, flags],
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
    println!("{drawn} characters and keys drawn");
    assert!(drawn > 2000);
}

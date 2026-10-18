//! `tricky_game::fe_wlist` (board row G9x3b) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random menu list in the arena: its mode, wrap, orientation, alignment, gap, box,
//! window, chosen item, arrows and frame. Its items (widgets whose tables point at stand-ins with random `this`
//! shifts) are chained into its list at +0x118. One of the row's functions then runs on both sides. The list's walk,
//! the widget's placing and sliding in and the icon's set-up run for real. The menu's own code (G9b) and every other
//! callee are stand-ins on both sides answering from the case's seed, with the items' sizes and bounds written by
//! their stand-ins. The calls are compared with the vectors, colours and operations they are given (read by
//! value), and the memory after. The runner's stack is cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_widgets::{ICON_VTABLE, UPDATE_COLOR};
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_wlist as wl;
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
const ICONVT: (u32, u32) = (0x3b6f00, 0x200);
const COLOURS: (u32, u32) = (0x3d3600, 0x300);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const W: u32 = ARENA;
const ITEMS: u32 = ARENA + 0x400;
const FRAME: u32 = ARENA + 0x800;
const ICONS: u32 = ARENA + 0xa00;
const VT: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x1200;
const APPOBJ: u32 = ARENA + 0x1300;
const RENDER: u32 = ARENA + 0x1380;
const VEC: u32 = ARENA + 0x1400;
const OUT: u32 = ARENA + 0x1440;
const OUT2: u32 = ARENA + 0x1480;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
fn item(k: u32) -> u32 {
    ITEMS + 0x100 * k
}
const SLOTS: [u32; 15] = [8, 0x18, 0x20, 0x30, 0x50, 0x58, 0x60, 0x68, 0x70, 0xf0, 0xf8, 0x128, 0x130, 0x138, 0x160];

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
    stack(a) || (ARENA..ARENA + SIZE - 0x40).contains(&a) || (0x3d3600..0x3d3880).contains(&a)
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

fn arity(f: u32, icon_fns: &[u32]) -> usize {
    match f {
        wl::MENU_CTOR => 6,
        wg::TWEEN_RUN => 5,
        wg::TWEEN_START | wl::FILL | wl::ALLOC | wl::MENU_VF36 | wl::MENU_ALIGN | wl::MENU_ALIGN_LAST => 3,
        wl::MENU_NEXT | wl::MENU_PREV | wl::MENU_VF37 | wl::MENU_VF5 | wl::MENU_DTOR | wl::LIST_NTH
        | wl::LIST_PUSH_FRONT | wl::LIST_PUSH_BACK => 2,
        f if f == wg::OP_OFFSET.1 => 2,
        f if f == rv(0x1d0) => 2,
        f if f == slot(0x128) => 3,
        f if f == slot(0x60) || f == slot(0x68) => 1,
        f if f & 0xffff_0000 == 0x0180_0000 => 2,
        f if icon_fns.contains(&f) => 2,
        _ => 1,
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], ans: &Answers, icon_fns: &[u32]) -> (u32, Call) {
    let n = arity(f, icon_fns);
    let mut log: Vec<u32> = w[..n].iter().map(|&a| token(a)).collect();
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a vector, colour, rectangle or operation by value; an offset operation's vector too)
    let by_value = (f & 0xffff_0000 == 0x0180_0000 || icon_fns.contains(&f) || f == wg::TWEEN_START || f == wg::TWEEN_RUN)
        && n >= 2;
    if by_value && readable(w[1]) && f != slot(0x128) {
        let first = deref(io, w[1], 4);
        log.extend(&first);
        if first[0] == wg::OP_OFFSET.0 {
            log.extend(deref(io, w[1] + 0x10, 4));
        }
    }
    if f == wg::TWEEN_START && readable(w[2]) {
        log.extend(deref(io, w[2], 4));
    }
    if f == wg::OP_OFFSET.1 {
        log.extend(deref(io, w[0] + 0x10, 4));
    }
    let r = ans.next();
    // (an item's or the frame's bounds and size, written where it is asked)
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x128) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(-100.0, 400.0).to_bits());
            io.w32(w[2] + 4 * k, g.f(-100.0, 400.0).to_bits());
        }
    }
    // (the menu's steps and layout may choose another item)
    if (f == wl::MENU_VF41 || f == wl::MENU_VF42 || f == wl::MENU_VF39) && g.below(2) == 0 {
        io.w32(W + 0x140, item(g.below(4)));
    }
    if f == slot(0x130) {
        for k in 0..4 {
            let v = g.f(0.0, 200.0);
            io.w32(w[1] + 4 * k, g.pick(&[0.0f32, 10.0, v]).to_bits());
        }
    }
    let answer = match f {
        f if f == rv(0x1d8) => r % 8,
        f if f == slot(0x18) => [0u32, 0, 0, r][(r % 4) as usize],
        wl::MENU_NEXT | wl::MENU_PREV => item(r % 4),
        wl::LIST_FIRST | wl::LIST_LAST | wl::LIST_NTH => [0, item(0), item(1), item(2), item(3)][(r % 5) as usize],
        wl::LIST_COUNT => r % 8,
        wl::NODE_INDEX => r % 10,
        wl::ALLOC => ICONS + 0x200 * (r % 2),
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
    icon_fns: &'a [u32],
}
impl FeEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, f, &w, self.ans, self.icon_fns);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 19] = [
    0x2b51d8, 0x2b5668, 0x2b5958, 0x2b5a30, 0x2b5a78, 0x2b5c80, 0x2b5e40, 0x2b5e98, 0x2b5ef0, 0x2b60f0, 0x2b6210,
    0x2b64b8, 0x2b6560, 0x2b6568, 0x2b6680, 0x2b66d8, 0x2b6720, 0x2b6838, 0x2b68e0,
];
/// the widget's and list's code they call, run as the original: the list's walk, the widget's placing and sliding
/// in
const ALSO: [u32; 5] = [0x2ac220, 0x2addb0, 0x2ade40, 0x2adf18, 0x2adf30];

#[test]
fn menu_list_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in FUNCS.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    // (the vector scale runs as the original: the port does it in place)
    r.allow(0x102f50);
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let (data, iconvt, colours) = (read(&mut r, DATA), read(&mut r, ICONVT), read(&mut r, COLOURS));
    // the icon's own virtuals that the placing and sliding in reach through its table
    let icon_fns: Vec<u32> = (0..0x160u32)
        .step_by(8)
        .map(|off| u32::from_le_bytes(iconvt[(ICON_VTABLE + off + 4 - ICONVT.0) as usize..][..4].try_into().unwrap()))
        .filter(|&f| f != 0)
        .collect();
    let icon_fns: &'static [u32] = Box::leak(icon_fns.into_boxed_slice());
    let mut stubs = vec![
        wl::MENU_NEXT, wl::MENU_PREV, wl::MENU_ALIGN, wl::MENU_ALIGN_LAST, wl::MENU_ARROWS, wl::MENU_VF12,
        wl::MENU_VF11, wl::MENU_VF41, wl::MENU_VF42, wl::MENU_VF39, wl::MENU_VF36, wl::MENU_VF37, wl::MENU_VF1,
        wl::MENU_VF5, wl::MENU_DTOR, wl::MENU_CTOR, wl::LIST_COUNT, wl::LIST_FIRST, wl::LIST_LAST, wl::LIST_NTH,
        wl::LIST_PUSH_FRONT, wl::LIST_PUSH_BACK, wl::NODE_UNLINK, wl::NODE_INDEX, wl::ALLOC, wl::FILL, UPDATE_COLOR,
        wg::TWEEN_START, wg::TWEEN_RUN, wg::OP_OFFSET.1, rv(0x1d0), rv(0x1d8),
    ];
    stubs.extend(SLOTS.map(slot));
    for f in icon_fns {
        if !stubs.contains(f) {
            stubs.push(*f);
        }
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (v, call) = standin(mem, f, &w, &a, icon_fns);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x9e1d_0000_0000_0001);
    let per = 2500u32;
    let mut walked = 0usize;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (ICONVT.0, iconvt.clone()),
                (COLOURS.0, colours.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in [0x1d0u32, 0x1d8] {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for o in [W, FRAME, item(0), item(1), item(2), item(3)] {
            m.w32(o + 0xc, VT);
            for k in 0..4 {
                m.w32(o + 0x80 + 4 * k, g.f(-200.0, 500.0).to_bits());
            }
        }
        for k in 0..4 {
            m.w32(item(k) + 0x14, g.pick(&[0u32, 1, 1]));
        }
        // (the items chained into the list at +0x118: its +8 the first, each its next at +8, the last its own)
        let n = g.below(5);
        let first = item(g.below(4));
        m.w32(W + 0x118 + 8, first);
        let mut node = first;
        for _ in 0..n {
            let next = item(((node - ITEMS) / 0x100 + 1).min(3));
            m.w32(node + 8, next);
            node = next;
        }
        m.w32(node + 8, node);
        for o in [0x14u32, 0xa0] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, any]));
        }
        m.w32(W + 0x70, g.below(16));
        m.w32(W + 0x13c, g.below(2));
        let any_item = item(g.below(4));
        m.w32(W + 0x140, g.pick(&[0, any_item, any_item]));
        for o in [0x148u32, 0x14c] {
            let it = item(g.below(4));
            m.w32(W + o, g.pick(&[0, it]));
        }
        m.w32(W + 0x150, g.pick(&[0u32, 1, 2]));
        m.w32(W + 0x154, g.below(16));
        m.w32(W + 0x15c, g.f(0.0, 20.0).to_bits());
        for o in [0x170u32, 0x174, 0x178, 0x17c] {
            let v = g.f(0.0, 300.0);
            // (a denormal box is 0 to the EE)
            m.w32(W + o, g.pick(&[0.0f32, v, v, f32::from_bits(1)]).to_bits());
        }
        m.w32(W + 0x190, g.pick(&[0u32, 1, 2, 2, 3, 4, 4, 5]));
        m.w32(W + 0x194, g.pick(&[0, FRAME, FRAME]));
        m.w32(W + 0x198, item(g.below(4)));
        m.w32(W + 0x19c, item(g.below(4)));
        m.w32(W + 0x1a0, g.pick(&[u32::MAX, 0, 1, 2, 3, 5]));
        // (the frame's offset looks at the frame and the chosen item without checking, as does the scroll that
        // calls it)
        if matches!(func, 0x2b5958 | 0x2b5668 | 0x2b5e40 | 0x2b5e98 | 0x2b6568) {
            m.w32(W + 0x194, FRAME);
            m.w32(W + 0x140, any_item);
        }
        for k in 0..8 {
            m.w32(VEC + 4 * k, g.f(-50.0, 50.0).to_bits());
        }
        let any = g.next();
        let a1 = g.pick(&[any, 0, 1, 3]);
        let it = item(g.below(4));
        let dir = g.pick(&[1u32, u32::MAX, 0]);
        let flags = g.pick(&[0u32, 1, 2, 3]);
        let vec = VEC + g.pick(&[0u32, 0, 4]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans, icon_fns };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let s = SCRATCH;
        let ours = match func {
            0x2b51d8 => { wl::make_arrows(mm, e, W, s); none }
            0x2b5668 => { wl::scroll(mm, e, W, it, dir, s); none }
            0x2b5958 => wl::frame_offset(mm, e, OUT, W, s),
            0x2b5a30 => { wl::list_update(mm, e, W); none }
            0x2b5a78 => { wl::list_render(mm, e, W, a1); none }
            0x2b5c80 => { wl::list_render_text(mm, e, W); none }
            0x2b5e40 => { wl::list_step(mm, e, W, true, s); none }
            0x2b5e98 => { wl::list_step(mm, e, W, false, s); none }
            0x2b5ef0 => { wl::list_layout(mm, e, W, s); none }
            0x2b60f0 => { wl::list_bounds(mm, e, W, OUT, OUT2, s); none }
            0x2b6210 => { wl::list_size(mm, e, W, OUT, s); none }
            0x2b64b8 => { wl::static_init(mm); none }
            0x2b6560 => wl::list_one(),
            0x2b6568 => { wl::list_choose(mm, e, W, a1, s); none }
            0x2b6680 => { wl::list_vf1(mm, e, W); none }
            0x2b66d8 => { wl::list_vf5(mm, e, W, a1); none }
            0x2b6720 => { wl::list_move(mm, e, W, vec, s); none }
            0x2b6838 => wl::list_dtor(mm, e, W, flags),
            0x2b68e0 => wl::list_ctor(mm, e, W),
            _ => unreachable!(),
        };
        walked += env.log.iter().filter(|c| c.0 == slot(0x18)).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2b5668 => vec![W, a1, it, dir],
            0x2b5958 => vec![OUT, W],
            0x2b60f0 => vec![W, OUT, OUT2],
            0x2b6210 => vec![W, OUT],
            0x2b6720 => vec![W, vec],
            0x2b6838 => vec![W, flags],
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
    println!("{walked} items given an operation");
    assert!(walked > 500);
}

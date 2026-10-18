//! `tricky_game::fe_wgroup` (board row G9x1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds random widgets and groups in the arena (their flags, tween, colour and position,
//! their tables pointing at stand-ins with random `this` shifts) with a list of a few nodes (each its next at +8, the
//! last its own), then runs one of the row's functions on both sides. The list's walk and the row's own helpers run
//! for real; every other callee is a stand-in on both sides answering from the case's seed. The calls (with the
//! operations, vectors, rectangles and matrices they are given, read by value) and the memory are compared; the
//! runner's stack is cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_wgroup as wg;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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

const DATA: (u32, u32) = (0x328000, 0x18000);
const IDENT: (u32, u32) = (0x346400, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const W: u32 = ARENA;
const NODES: u32 = ARENA + 0x400;
const VT: u32 = ARENA + 0x800;
const APPOBJ: u32 = ARENA + 0xc00;
const RENDER: u32 = ARENA + 0xd00;
const RVT: u32 = ARENA + 0xe00;
const VEC: u32 = ARENA + 0x1000;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// the objects' virtuals' stand-ins (by slot) and the renderer's
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 12] = [0x18, 0x20, 0x38, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0x138, 0x140];
const MORE_SLOTS: [u32; 2] = [0xe8, 0xf8];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
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
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32) -> usize {
    match f {
        wg::TWEEN_RUN => 5,
        wg::TWEEN_START => 3,
        f if f == rv(0x1d0) => 2,
        f if f == rv(0x1d8) => 1,
        // (the virtuals that take only `this`)
        f if f == slot(0x38) || f == slot(0x40) || f == slot(0x48) || f == slot(0x60) || f == slot(0x68) || f == slot(0x140) => 1,
        f if f & 0xffff_0000 == 0x0180_0000 => 2,
        wg::MATRIX_MUL | wg::MATRIX_DTOR => 2,
        f if f == wg::OP_UPDATE_ACTIVITY.1 || f == wg::OP_RESET_ACTIVITY.1 || f == wg::OP_SKIP_ACTIVITY.1
            || f == wg::OP_ON_INPUT.1 || f == wg::OP_UPDATE.1 || f == wg::OP_RENDER_TEXT.1 || f == wg::OP_RENDER.1
            || f == wg::OP_FORMAT.1 || f == wg::OP_OFFSET.1 => 2,
        _ => 1,
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let n = arity(f);
    let mut log: Vec<u32> = w[..n].iter().map(|&a| token(a)).collect();
    // a pointer argument's words by value: an operation (its table, argument, vector), a rectangle, a vector, a
    // matrix
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    if f & 0xffff_0000 == 0x0180_0000 && n == 2 && (VEC..VEC + 0x100).contains(&w[1]) {
        log.extend(deref(io, w[1], 8));
    }
    // (an operation on the stack: its table and argument, and an offset's vector; a rectangle's four words)
    if f & 0xffff_0000 == 0x0180_0000 && n == 2 && stack(w[1]) {
        let first = deref(io, w[1], 4);
        log.extend(&first);
        if first[0] == wg::OP_OFFSET.0 {
            log.extend(deref(io, w[1] + 0x10, 4));
        }
    }
    if f == wg::MATRIX_MUL {
        log.extend(deref(io, w[0], 16));
    }
    // (the group's table as its children go)
    if f == wg::LIST_DELETE_ALL {
        log.push(io.r32(W + 0xc));
    }
    let r = ans.next();
    let answer = match f {
        f if f == slot(0x18) => [0u32, 0, 0, r][(r % 4) as usize],
        f if f == slot(0xe8) => r % 2,
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
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, f, &w, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 61] = [
    0x2aac40, 0x2aac90, 0x2aace0, 0x2aad30, 0x2aad90, 0x2aadf0, 0x2aaeb8, 0x2aaf90, 0x2aafc8, 0x2ab070, 0x2ab380,
    0x2ab410, 0x2ab490, 0x2ab510, 0x2ab590, 0x2ab610, 0x2ab698, 0x2ab718, 0x2ab798, 0x2abc70, 0x2abca0, 0x2abda8,
    0x2ac220, 0x2ac428, 0x2ac478, 0x2ac580, 0x2ac5d0, 0x2ac620, 0x2ac6b8, 0x2ac708, 0x2aca18, 0x2aca20, 0x2aca28,
    0x2aca30, 0x2aca38, 0x2aca40, 0x2aca48, 0x2aca50, 0x2aca58, 0x2aca60, 0x2aca68, 0x2aca70, 0x2aca78, 0x2aca80,
    0x2acaf8, 0x2acbb8, 0x2acbe0, 0x2acbe8, 0x2acc80, 0x2acca8, 0x2accf0, 0x2accf8, 0x2acd10, 0x2ace78, 0x2acf28,
    0x2acf60, 0x2addb0, 0x2ade40, 0x2adf18, 0x2adf30, 0x2b3950,
];

/// The copies of the operations above elsewhere in the game (the same bytes but for their branches, in other
/// translation units): (the copy, the function it copies)
const COPIES: [(u32, u32); 77] = [
    (0x268e40, 0x2aac40),
    (0x268e90, 0x2aac90),
    (0x268ee0, 0x2aace0),
    (0x268f30, 0x2aad30),
    (0x268f90, 0x2aad90),
    (0x268ff0, 0x2aadf0),
    (0x2690b8, 0x2aaeb8),
    (0x269190, 0x2aaf90),
    (0x2691c8, 0x2aafc8),
    (0x269270, 0x2ab070),
    (0x2693c0, 0x2ab698),
    (0x269448, 0x2ab718),
    (0x2694d0, 0x2ab798),
    (0x269758, 0x2ab380),
    (0x2697e8, 0x2ab410),
    (0x269868, 0x2ab490),
    (0x2698e8, 0x2ab510),
    (0x269968, 0x2ab590),
    (0x2699e8, 0x2ab610),
    (0x270008, 0x2ab698),
    (0x275e38, 0x2aac40),
    (0x275e88, 0x2aac90),
    (0x275ed8, 0x2aace0),
    (0x275f28, 0x2aad30),
    (0x275f88, 0x2aad90),
    (0x275fe8, 0x2aadf0),
    (0x2760b0, 0x2aaeb8),
    (0x276188, 0x2aaf90),
    (0x2761c0, 0x2aafc8),
    (0x276268, 0x2ab070),
    (0x276708, 0x2ab380),
    (0x276798, 0x2ab410),
    (0x276818, 0x2ab490),
    (0x276898, 0x2ab510),
    (0x276918, 0x2ab590),
    (0x276998, 0x2ab610),
    (0x276a20, 0x2ab698),
    (0x276aa0, 0x2ab718),
    (0x276b20, 0x2ab798),
    (0x28e640, 0x2aac40),
    (0x28e690, 0x2aac90),
    (0x28e6e0, 0x2aace0),
    (0x28e730, 0x2aad30),
    (0x28e790, 0x2aad90),
    (0x28e7f0, 0x2aadf0),
    (0x28e8b8, 0x2aaeb8),
    (0x28e990, 0x2aaf90),
    (0x28e9c8, 0x2aafc8),
    (0x28ea70, 0x2ab070),
    (0x28ed80, 0x2ab380),
    (0x28ee10, 0x2ab410),
    (0x28ee90, 0x2ab490),
    (0x28ef10, 0x2ab510),
    (0x28ef90, 0x2ab590),
    (0x28f010, 0x2ab610),
    (0x28f098, 0x2ab698),
    (0x28f118, 0x2ab718),
    (0x28f198, 0x2ab798),
    (0x29dd00, 0x2aac40),
    (0x29dd50, 0x2aac90),
    (0x29dda0, 0x2aace0),
    (0x29ddf0, 0x2aad30),
    (0x29de50, 0x2aad90),
    (0x29deb0, 0x2aadf0),
    (0x29df78, 0x2aaeb8),
    (0x29e050, 0x2aaf90),
    (0x29e088, 0x2aafc8),
    (0x29e130, 0x2ab070),
    (0x29e440, 0x2ab380),
    (0x29e4d0, 0x2ab410),
    (0x29e550, 0x2ab490),
    (0x29e5d0, 0x2ab510),
    (0x29e650, 0x2ab590),
    (0x29e6d0, 0x2ab610),
    (0x29e758, 0x2ab698),
    (0x29e7d8, 0x2ab718),
    (0x29e858, 0x2ab798),
];

#[test]
fn widget_bases_match_the_original() {
    let pairs: Vec<(u32, u32)> = FUNCS.iter().map(|&f| (f, f)).collect();
    run(&pairs, 800);
}

/// (board row G9y2) each copy run in the runner against the port of the function it copies
#[test]
fn copies_match_the_original() {
    run(&COPIES, 300);
}

/// Every case runs `pairs[k].0` in the runner against the port of `pairs[k].1`
fn run(pairs: &[(u32, u32)], per: u32) {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS.iter().chain(pairs.iter().map(|p| &p.0)) {
        r.allow(*f);
    }
    let mut stubs = vec![
        wg::LIST_DELETE_ALL, wg::LIST_INIT, wg::NODE_UNLINK, wg::WIDGET_UPDATE_ACTIVITY, wg::WIDGET_RESET_ACTIVITY,
        wg::TWEEN_START, wg::TWEEN_RUN, wg::BAR_SET, wg::FREE, wg::MATRIX_MUL, wg::MATRIX_DTOR, rv(0x1d0), rv(0x1d8),
    ];
    for k in [
        wg::OP_UPDATE_ACTIVITY, wg::OP_RESET_ACTIVITY, wg::OP_SKIP_ACTIVITY, wg::OP_ON_INPUT, wg::OP_UPDATE,
        wg::OP_RENDER_TEXT, wg::OP_RENDER, wg::OP_FORMAT, wg::OP_OFFSET,
    ] {
        stubs.push(k.1);
    }
    stubs.extend(SLOTS.map(slot));
    stubs.extend(MORE_SLOTS.map(slot));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (v, call) = standin(mem, f, &w, &a);
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
    let data = read(&mut r, DATA);
    let ident = read(&mut r, IDENT);
    let mut g = Rng(0x9e1a_0000_0000_0001);
    let mut walked = 0usize;
    for case in 0..pairs.len() as u32 * per {
        let (addr, func) = pairs[(case % pairs.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (IDENT.0, ident.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x800])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // (the identity is the game's to set up: random here, so all of it is seen copied)
        for k in 0..0x10 {
            m.w32(wg::IDENTITY + 4 * k, g.next());
        }
        // the app's renderer and its table; the objects' table; the widget's flags, tween and colour; its lists
        // (+0x110 and +0xa0, each a chain of nodes from +8, the last its own next); a vector
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in [0x1d0u32, 0x1d8] {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for off in SLOTS.iter().chain(MORE_SLOTS.iter()) {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(*off));
        }
        m.w32(W + 0xc, VT);
        for o in [0x10u32, 0x14, 0x2c, 0xa4] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, any]));
        }
        for k in 0..8 {
            let node = NODES + 0x40 * k;
            m.w32(node + 0xc, VT);
        }
        for list in [W + 0x110, W + 0xa0] {
            let n = g.below(5);
            let first = NODES + 0x40 * g.below(4);
            m.w32(list + 8, first);
            let mut node = first;
            for k in 0..n {
                let next = NODES + 0x40 * ((node - NODES) / 0x40 + 1 + k % 2).min(7);
                let next = if next == node { node } else { next };
                m.w32(node + 8, next);
                node = next;
            }
            m.w32(node + 8, node);
        }
        for k in 0..4 {
            m.w32(VEC + 4 * k, g.f(-50.0, 50.0).to_bits());
            m.w32(W + 0x80 + 4 * k, g.f(-300.0, 300.0).to_bits());
        }
        let (a1, a2) = (g.next(), g.below(40));
        let (x, y) = (g.f(-100.0, 600.0), g.f(-100.0, 500.0));
        let flags = g.pick(&[0u32, 1, 2, 3]);
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
            0x2aac40 => { wg::wgroup_update_activity(mm, e, W, s); none }
            0x2aac90 => { wg::wgroup_reset_activity(mm, e, W, s); none }
            0x2aace0 => { wg::wgroup_skip_activity(mm, e, W, s); none }
            0x2aad30 => wg::wgroup_on_input(mm, e, W, a1, s),
            0x2aad90 => { wg::wgroup_update(mm, e, W, s); none }
            0x2aadf0 => { wg::wgroup_render_text(mm, e, W, s); none }
            0x2aaeb8 => { wg::wgroup_render(mm, e, W, a1, s); none }
            0x2aaf90 => { wg::wgroup_format(mm, e, W, s); none }
            0x2aafc8 => { wg::wgroup_free_children(mm, e, W); none }
            0x2ab070 => { wg::wgroup_offset(mm, e, W, VEC, s); none }
            0x2ab380 => wg::op_on_input(mm, e, VEC, W),
            0x2ab410 => wg::op_plain(mm, e, 0x140, W),
            0x2ab490 => wg::op_plain(mm, e, 0x48, W),
            0x2ab510 => wg::op_plain(mm, e, 0x40, W),
            0x2ab590 => wg::op_plain(mm, e, 0x38, W),
            0x2ab610 => wg::op_offset(mm, e, VEC, W),
            0x2ab698 => wg::op_plain(mm, e, 0x68, W),
            0x2ab718 => wg::op_plain(mm, e, 0x60, W),
            0x2ab798 => wg::op_render(mm, e, VEC, W),
            0x2abc70 => { wg::group_vf1(mm, e, W); none }
            0x2abca0 => { wg::group_render(mm, e, W, a1, s); none }
            0x2abda8 => { wg::group_render_text(mm, e, W, s); none }
            0x2ac220 => wg::list_apply(mm, e, W + 0x110, a1),
            0x2ac428 => { wg::thing_vf6(mm, e, W); none }
            0x2ac478 => { wg::thing_vf7(mm, e, W); none }
            0x2ac580 => { wg::group_reset(mm, e, W, s); none }
            0x2ac5d0 => { wg::group_skip(mm, e, W, s); none }
            0x2ac620 => { wg::group_offset(mm, e, W, VEC, s); none }
            0x2ac6b8 => { wg::group_dtor(mm, e, W, flags); none }
            0x2ac708 => wg::group_ctor(mm, e, W, s),
            0x2aca18..=0x2aca78 => wg::thing_zero(),
            0x2aca80 => wg::thing_one(),
            0x2acaf8 => { wg::thing_vf13(mm, W, VEC, s); none }
            0x2acbb8 => { wg::thing_vf12(mm, e, W); none }
            0x2acbe0 | 0x2acbe8 | 0x2acca8 => { wg::thing_nothing(); none }
            0x2acc80 => { wg::thing_vf9(mm, W, VEC); none }
            0x2accf0 => { wg::thing_set(mm, W, 0x14, a1); none }
            0x2accf8 => { wg::thing_set(mm, W, 0x10, a1); none }
            0x2acd10 => { wg::thing_dtor(mm, e, W, flags); none }
            0x2ace78 => { wg::list_dtor(mm, e, W + 0xa0, flags); none }
            0x2acf28 => { wg::node_vf1(mm, e, W); none }
            0x2acf60 => { wg::node_dtor(mm, e, W, flags); none }
            0x2addb0 => { wg::widget_place(mm, e, W, a1, F(x), F(y), s); none }
            0x2ade40 => { wg::widget_slide_in(mm, e, W, a1, a2, F(x), F(y), s); none }
            0x2adf18 => { wg::widget_slide_in_dir(mm, e, W, 3, s); none }
            0x2adf30 => { wg::widget_slide_in_dir(mm, e, W, 4, s); none }
            0x2b3950 => { wg::widget_set_value(mm, e, W, F(x)); none }
            _ => unreachable!(),
        };
        walked += env.log.iter().filter(|c| c.0 == slot(0x18)).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2aad30 | 0x2aaeb8 | 0x2abca0 | 0x2accf0 | 0x2accf8 | 0x2addb0 => vec![W, a1],
            0x2ade40 => vec![W, a1, a2],
            0x2ab070 | 0x2ac620 | 0x2acaf8 | 0x2acc80 => vec![W, VEC],
            0x2ab380 | 0x2ab410 | 0x2ab490 | 0x2ab510 | 0x2ab590 | 0x2ab610 | 0x2ab698 | 0x2ab718 | 0x2ab798 => vec![VEC, W],
            0x2ac220 => vec![W + 0x110, a1],
            0x2ace78 => vec![W + 0xa0, flags],
            0x2ac6b8 | 0x2acd10 | 0x2acf60 => vec![W, flags],
            _ => vec![W],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, x);
        r.cpu.set_f(13, y);
        r.call(addr).unwrap_or_else(|e| panic!("case {case} {addr:#x} ({func:#x}): {e}"));
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
    println!("{walked} nodes given an operation");
    assert!(walked > 2000);
}

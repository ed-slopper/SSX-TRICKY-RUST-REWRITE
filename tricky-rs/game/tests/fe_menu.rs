//! `tricky_game::fe_menu` (board row G9y1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random menu in the arena: its sounds, way, alignment, wrap, gap, box, layout
//! flags, chosen item, arrows and owner. Its items (shown, enabled and chosen at random, linked both ways) and the
//! other objects have tables pointing at stand-ins (with random `this` shifts). One of the row's functions then runs
//! on both sides. The list's walk and the widget's code run for real; every other callee is a stand-in on both
//! sides answering from the case's seed. The items' sizes and bounds are written by their stand-ins, and the list's
//! end is found where the nodes leave the items (or at random). The calls are compared with their float arguments,
//! the vectors, colours and operations they are given (read by value), and the memory after. The runner's stack is
//! cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_menu as mn;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widget as fw;
use tricky_game::fe_widgets::UPDATE_COLOR;
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

const DATA: (u32, u32) = (0x337c00, 0x400);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const W: u32 = ARENA;
const ITEMS: u32 = ARENA + 0x400;
const A1: u32 = ARENA + 0x800;
const A2: u32 = ARENA + 0xa00;
const OWNER: u32 = ARENA + 0xc00;
const VT: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x1200;
const APPOBJ: u32 = ARENA + 0x1300;
const RENDER: u32 = ARENA + 0x1380;
const FE: u32 = ARENA + 0x1400;
const SND: u32 = ARENA + 0x1700;
const SVT: u32 = ARENA + 0x1780;
const VEC: u32 = ARENA + 0x1500;
const OUT: u32 = ARENA + 0x1540;
const OUT2: u32 = ARENA + 0x1580;
const OPB: u32 = ARENA + 0x1600;
const INPUT: u32 = ARENA + 0x1680;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
const SND_CALL: u32 = 0x0182_0050;
fn item(k: u32) -> u32 {
    ITEMS + 0x100 * k
}
fn is_item(a: u32) -> bool {
    (ITEMS..ITEMS + 0x400).contains(&a) && (a - ITEMS) % 0x100 == 0
}
const SLOTS: [u32; 26] = [
    8, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70, 0xb0, 0xb8, 0xc8, 0xd0, 0xd8, 0xe8,
    0xf0, 0x128, 0x130, 0x138, 0x140, 0x150, 0x158,
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
        wg::TWEEN_RUN => (5, 0),
        wg::TWEEN_START => (3, 0),
        mn::LIST_NTH | mn::LIST_PUSH_BACK | mn::LIST_FIND | fw::THING_DTOR | SND_CALL | wg::FREE => (2, 0),
        f if f == rv(0x1d0) => (2, 0),
        f if f == slot(0x128) || f == slot(0xb8) => (3, 0),
        f if f == slot(0x40) || f == slot(0x48) || f == slot(0x60) || f == slot(0x68) || f == slot(0x140)
            || f == slot(0x150) || f == slot(0x158) => (1, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, _) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a vector, colour or operation by value: an offset's vector, the format operation's whole state)
    // (a size's or bounds' buffer is only written: not logged)
    if f & 0xffff_0000 == 0x0180_0000 && ni >= 2 && f != slot(0x128) && f != slot(0x130) && readable(w[1]) {
        let first = deref(io, w[1], 4);
        log.extend(&first);
        if first[0] == wg::OP_OFFSET.0 {
            log.extend(deref(io, w[1] + 0x10, 4));
        }
        if first[0] == mn::FORMAT_OP {
            log.extend(deref(io, w[1] + 0x10, 12));
        }
    }
    // (the menu's table as its arrows and items go)
    if f == slot(8) || f == wg::LIST_DELETE_ALL {
        log.push(io.r32(W + 0xc));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x128) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(-100.0, 400.0).to_bits());
            io.w32(w[2] + 4 * k, g.f(-100.0, 400.0).to_bits());
        }
    }
    if f == slot(0x130) {
        for k in 0..4 {
            let v = g.f(0.0, 120.0);
            // (now and then below 1, so that what is more than 0 is seen)
            io.w32(w[1] + 4 * k, g.pick(&[0.0f32, 10.0, v, 0.25]).to_bits());
        }
    }
    let answer = match f {
        f if f == rv(0x1d8) => r % 8,
        f if f == slot(0x18) => [0u32, 0, 0, r][(r % 4) as usize],
        f if f == slot(0xe8) => r % 2,
        mn::INPUT_LEFT | mn::INPUT_RIGHT | mn::INPUT_UP | mn::INPUT_DOWN | mn::INPUT_ACCEPT | mn::INPUT_BACK
        | mn::INPUT_KEY63 | mn::INPUT_KEY61 => (r % 4 == 0) as u32,
        // (a list's end where the nodes leave the items, or now and then)
        mn::NODE_IS_END => (!is_item(w[0]) || r % 4 == 0) as u32,
        mn::LIST_FIRST | mn::LIST_LAST => item(r % 4),
        mn::LIST_NTH => [0, item(0), item(1), item(2), item(3)][(r % 5) as usize],
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

const FUNCS: [u32; 49] = [
    0x26d1c0, 0x26d3b8, 0x26d658, 0x26d660, 0x26d8c8, 0x26dd60, 0x26df58, 0x26e150, 0x26e538, 0x26e5c8, 0x26e658,
    0x26e7b8, 0x26e918, 0x26e9a8, 0x26e9f8, 0x26ea98, 0x26eb98, 0x26ec40, 0x26efd8, 0x26f2a0, 0x26f4c0, 0x26f4c8,
    0x26f4d0, 0x26f5c0, 0x26f688, 0x26f698, 0x26f7c0, 0x26f830, 0x26f8d8, 0x26f970, 0x26f990, 0x26f9d0, 0x26fa70,
    0x26faf8, 0x26fb78, 0x269560, 0x269388, 0x269390, 0x269398, 0x2693a0, 0x2693a8, 0x2693b0, 0x2693b8, 0x269440,
    0x2694c8, 0x269558, 0x270210, 0x269620, 0x2701a8,
];
/// the widget's and list's code they call, run as the original: the list's walk, the widget's vf5 and vf7 (and the
/// tween's start, the start from where it is and the vector scale they use), its flag, the node's vf1 and
/// destructor, the list's destructor, the widget's destructor
const ALSO: [u32; 11] = [0x2ac220, 0x2ad250, 0x2ad2c0, 0x2ac348, 0x2ac288, 0x102f50, 0x2ada30, 0x2acf28, 0x2acf60, 0x2ace78, 0x2adaa8];

#[test]
fn menu_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in FUNCS.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    let mut stubs = vec![
        mn::LIST_FIRST, mn::LIST_LAST, mn::LIST_NTH, mn::LIST_PUSH_BACK, mn::LIST_FIND, mn::NODE_IS_END,
        mn::INPUT_LEFT, mn::INPUT_RIGHT, mn::INPUT_UP, mn::INPUT_DOWN, mn::INPUT_ACCEPT, mn::INPUT_BACK,
        mn::INPUT_KEY63, mn::INPUT_KEY61, wg::LIST_INIT, wg::LIST_DELETE_ALL, wg::NODE_UNLINK, wg::FREE,
        UPDATE_COLOR, fw::THING_DTOR, rv(0x1d0), rv(0x1d8), SND_CALL,
    ];
    stubs.extend(SLOTS.map(slot));
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
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e1f_0000_0000_0001);
    let per = 1500u32;
    let mut moved = 0usize;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x800])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x734, g.pick(&[0, FE, FE]));
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
        for o in [W, A1, A2, OWNER, item(0), item(1), item(2), item(3)] {
            m.w32(o + 0xc, VT);
            for k in 0..4 {
                m.w32(o + 0x80 + 4 * k, g.f(-200.0, 500.0).to_bits());
            }
        }
        for k in 0..4 {
            let it = item(k);
            m.w32(it + 0x14, g.pick(&[0u32, 1, 1, 1]));
            m.w32(it + 0xa4, g.pick(&[0u32, 0, 0, 1]));
            m.w32(it + 0xa8, g.pick(&[0u32, 1, 1]));
            m.w32(it + 4, item(g.below(4)));
            m.w32(it + 8, item(g.below(4)));
        }
        // (one item can always be chosen: the game's search for one goes round for ever without)
        m.w32(item(0) + 0x14, 1);
        m.w32(item(0) + 0xa4, 0);
        for o in [0x10u32, 0x14, 0xa0, 0xa4, 0xa8] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, 1, any]));
        }
        m.w32(W + 0x20, g.pick(&[5u32, 1, 3]));
        m.w32(W + 0x70, g.below(16));
        m.w32(W + 0xec, g.pick(&[0, OWNER, OWNER]));
        m.w32(OWNER + 0x10, g.pick(&[2u32, 2, 0]));
        m.w32(OWNER + 0x1c, g.below(2));
        for k in 0..7 {
            // (sounds past 0x7f too: the id is a signed byte)
            m.w8(W + 0x110 + k, g.pick(&[1u8, 4, 5, 7, 0x80, 0xfe]));
        }
        m.w32(W + 0x13c, g.below(2));
        let it = item(g.below(4));
        m.w32(W + 0x140, g.pick(&[0, it, it]));
        m.w32(W + 0x144, g.below(2));
        m.w32(W + 0x148, g.pick(&[0, A1, A1]));
        m.w32(W + 0x14c, g.pick(&[0, A2, A2]));
        m.w32(W + 0x150, g.pick(&[0u32, 1, 2]));
        m.w32(W + 0x154, g.below(16));
        m.w32(W + 0x158, g.below(2));
        let gap = g.f(0.0, 20.0);
        m.w32(W + 0x15c, g.pick(&[gap, gap, 0.0]).to_bits());
        m.w32(W + 0x160, g.below(2));
        for o in [0x170u32, 0x174, 0x178, 0x17c] {
            let v = g.f(0.0, 300.0);
            m.w32(W + o, g.pick(&[0.0f32, v, v, f32::from_bits(1)]).to_bits());
        }
        m.w32(W + 0x180, g.pick(&[0u32, 0, 1]));
        // (the list at +0x118: its first node the items' chain, the end where it leaves them)
        m.w32(W + 0x118 + 8, item(g.below(4)));
        // (these read the chosen item, or tell the owner, without looking)
        if matches!(func, 0x26e150 | 0x26e658 | 0x26e7b8 | 0x26d8c8) {
            m.w32(W + 0x140, it);
        }
        if func == 0x26f4d0 {
            m.w32(W + 0xec, OWNER);
        }
        // the format operation's state
        m.w32(OPB, mn::FORMAT_OP);
        for k in 0..4 {
            m.w32(OPB + 0x10 + 4 * k, g.f(-100.0, 300.0).to_bits());
            let v = g.f(0.0, 80.0);
            m.w32(OPB + 0x30 + 4 * k, g.pick(&[0.0f32, v, v]).to_bits());
        }
        m.w32(OPB + 0x20, g.pick(&[0u32, 1]));
        m.w32(OPB + 0x24, g.below(16));
        m.w32(OPB + 0x28, g.f(0.0, 10.0).to_bits());
        // (the extent operation's corners; the hash looked for, the item's or not)
        if func == 0x269620 {
            m.w32(OPB + 4, OUT);
            m.w32(OPB + 8, OUT2);
            for k in 0..8 {
                m.w32(OUT + 4 * k, g.f(-100.0, 400.0).to_bits());
                m.w32(OUT2 + 4 * k, g.f(-100.0, 400.0).to_bits());
            }
        }
        if func == 0x2701a8 {
            for k in 0..4 {
                if g.below(2) == 0 {
                    let h = m.r32(OPB + 4);
                    m.w32(item(k) + 0x74, h);
                }
            }
        }
        for k in 0..8 {
            m.w32(VEC + 4 * k, g.f(-50.0, 50.0).to_bits());
        }
        m.w32(INPUT, g.pick(&[0u32, 1, 1]));
        let any = g.next() | 0x8000_0000;
        let a1 = g.pick(&[any, 0, 1, 2, VEC]);
        let (a2, a3) = (g.next() | 0x8000_0000, g.next() | 0x8000_0000);
        let (i1, i2) = (item(g.below(4)), item(g.below(4)));
        let flags = g.pick(&[0u32, 1, 2, 3]);
        let fs = [g.f(-100.0, 400.0), g.f(-100.0, 400.0)];
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
            0x26d1c0 => { mn::set_rumble_enabled(mm, W, a1); none }
            0x26d3b8 => mn::menu_ctor(mm, e, W, a1, a2, a3, flags, fs[0].to_bits()),
            0x26d658 => mn::menu_add(mm, e, W, a1),
            0x26d660 => { mn::layout(mm, e, W, s); none }
            0x26d8c8 => { mn::update_arrows(mm, e, W, s); none }
            0x26dd60 => { mn::align(mm, e, W, i1, i2, false, s); none }
            0x26df58 => { mn::align(mm, e, W, i1, i2, true, s); none }
            0x26e150 => mn::on_input(mm, e, W, INPUT),
            0x26e538 => mn::next_choosable(mm, e, W, i1, true),
            0x26e5c8 => mn::next_choosable(mm, e, W, i1, false),
            0x26e658 => { mn::step(mm, e, W, true, s); none }
            0x26e7b8 => { mn::step(mm, e, W, false, s); none }
            0x26e918 => { mn::menu_vf5(mm, e, W, s); none }
            0x26e9a8 => { mn::menu_vf7(mm, e, W, s); none }
            0x26e9f8 => { mn::menu_vf1(mm, e, W); none }
            0x26ea98 => { mn::menu_render_text(mm, e, W, s); none }
            0x26eb98 => { mn::menu_update(mm, e, W, s); none }
            0x26ec40 => { mn::menu_bounds(mm, e, W, OUT, OUT2, s); none }
            0x26efd8 => { mn::menu_size(mm, e, W, OUT, s); none }
            0x26f2a0 => mn::format_op_apply(mm, e, OPB, i1, s),
            f if mn::OPERATOR_DTORS.contains(&f) => { mn::operator_dtor(mm, e, OPB, flags); none }
            0x269620 => mn::get_extent_op(mm, e, OPB, i1, s),
            0x2701a8 => mn::find_hash_op(mm, OPB, i1),
            0x26f4c8 => mn::menu_one(),
            0x26f4d0 => { mn::menu_set_chosen(mm, e, W, a1); none }
            0x26f5c0 => { mn::menu_move(mm, e, W, vec, s); none }
            0x26f688 => { mn::set_box(mm, W, vec); none }
            0x26f698 => { mn::set_box_wh(mm, W, F(fs[0]), F(fs[1]), s); none }
            0x26f7c0 => { mn::arrows_text(mm, e, W); none }
            0x26f830 => { mn::arrows_render(mm, e, W, a1); none }
            0x26f8d8 => { mn::choose(mm, e, W, a1); none }
            0x26f970 => mn::nth(mm, e, W, a1),
            0x26f990 => mn::find(mm, e, W, a1),
            0x26f9d0 => mn::menu_dtor(mm, e, W, flags),
            0x26fa70 => mn::op_format(mm, e, i1),
            0x26faf8 => mn::op_set_active(mm, e, i1),
            _ => unreachable!(),
        };
        moved += env.log.iter().filter(|c| c.0 == slot(0x70)).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x26d3b8 => vec![W, a1, a2, a3, flags, fs[0].to_bits()],
            0x26dd60 | 0x26df58 => vec![W, i1, i2],
            0x26e150 => vec![W, INPUT],
            0x26e538 | 0x26e5c8 => vec![W, i1],
            0x26ec40 => vec![W, OUT, OUT2],
            0x26efd8 => vec![W, OUT],
            0x26f2a0 => vec![OPB, i1],
            f if mn::OPERATOR_DTORS.contains(&f) => vec![OPB, flags],
            0x269620 | 0x2701a8 => vec![OPB, i1],
            0x26f5c0 | 0x26f688 => vec![W, vec],
            0x26f9d0 => vec![W, flags],
            0x26fa70 | 0x26faf8 => vec![OPB, i1],
            _ => vec![W, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, fs[0]);
        r.cpu.set_f(13, fs[1]);
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
    println!("{moved} widgets moved");
    assert!(moved > 2000);
}

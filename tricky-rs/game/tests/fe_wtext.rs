//! `tricky_game::fe_wtext` (board row G9x2) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random text widget in the arena: its flags, colours, position and size; for the
//! texts, the font, scale, border, wrap mode, string and split; for the scrolling text, its line slots, ids, roll,
//! speed, gap and alignment. It then runs one of the row's functions on both sides. The row's own helpers and the
//! single-line text's run for real; every other callee is a stand-in on both sides answering from the case's seed.
//! The calls are compared with their float arguments (f12..f15), the colours and strings they are given (read by
//! value), and the memory after. The runner's stack is cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_venuesel::LOC_STRING;
use tricky_game::fe_widget as fw;
use tricky_game::fe_widgets::UPDATE_COLOR;
use tricky_game::fe_wtext as wt;
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
const STRS: u32 = ARENA + 0x400;
const FONT: u32 = ARENA + 0x800;
const VT: u32 = ARENA + 0x900;
const RVT: u32 = ARENA + 0xa00;
const APPOBJ: u32 = ARENA + 0xb00;
const RENDER: u32 = ARENA + 0xc00;
const VEC: u32 = ARENA + 0xd00;
const OUT: u32 = ARENA + 0xe00;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// the widget's virtuals' stand-ins (by slot) and the renderer's
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 2] = [0x48, 0x150];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
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
        wt::TB_SET_RECT => (1, 4),
        wt::TB_SET_LINE_HEIGHT => (1, 1),
        wt::TB_SET_SCALE => (1, 2),
        fw::DRAW_WIDE_BORDER | fw::DRAW_WIDE | fw::DRAW_NARROW_BORDER | fw::DRAW_NARROW => (2, 3),
        fw::WIDTH_WIDE | fw::WIDTH_NARROW => (3, 1),
        wt::TB_DRAW | wt::WSTR_COPY_N | wt::STRNCPY | wt::FILL => (3, 0),
        wt::TB_SET_FONT | wt::TB_APPEND | wt::TB_APPEND_NARROW | wt::TB_WRAP | wt::TB_DELETE | wt::WSTR_COPY
        | wt::STRCPY | LOC_STRING | fw::DRAWER_COLOUR | fw::THING_DTOR => (2, 0),
        f if f == rv(0x1d0) || f == slot(0x150) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a colour, and a string drawn or measured, by value)
    if f == fw::DRAWER_COLOUR {
        log.extend(deref(io, w[1], 4));
    }
    if matches!(f, fw::DRAW_WIDE_BORDER | fw::DRAW_WIDE | fw::DRAW_NARROW_BORDER | fw::DRAW_NARROW | fw::WIDTH_WIDE | fw::WIDTH_NARROW)
        && readable(w[1])
    {
        log.extend(deref(io, w[1], 8));
    }
    // (the copies with a limit fill their buffer, so where the end is put is seen)
    if f == wt::WSTR_COPY_N || f == wt::STRNCPY {
        for k in 0..0x40 {
            io.w8(w[0] + k, 0x43);
        }
    }
    let r = ans.next();
    let answer = match f {
        f if f == rv(0x1d8) => r % 8,
        LOC_STRING => STRS + 0x40 * (r % 6),
        fw::WIDTH_WIDE | fw::WIDTH_NARROW => r % 300,
        wt::TB_MAX_WIDTH | wt::TB_HEIGHT => r % 500,
        wt::INPUT_UP | wt::INPUT_DOWN => r % 2,
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

const FUNCS: [u32; 48] = [
    0x2b0b90, 0x2b0dd8, 0x2b0e78, 0x2b0ea0, 0x2b0f18, 0x2b0f68, 0x2b0fb8, 0x2b1008, 0x2b1070, 0x2b10c8, 0x2b1650,
    0x2b1690, 0x2b16a0, 0x2b1758, 0x2b1770, 0x2b19f8, 0x2b1a00, 0x2b1a68, 0x2b1b20, 0x2b1d60, 0x2b1da0, 0x2b1de8,
    0x2b1e58, 0x2b1ea0, 0x2b1ee8, 0x2b1fa8, 0x2b1ff0, 0x2b2048, 0x2b2080, 0x2b2420, 0x2b2510, 0x2b26c8, 0x2b2750,
    0x2b4828, 0x2b4830, 0x2b4858, 0x2b48b0, 0x2b4940, 0x2b4948, 0x2b4998, 0x2b4cb8, 0x2b4cc0, 0x2b4cc8, 0x2b4ce8,
    0x2b4d38, 0x2b4d90, 0x2b4df0, 0x2b4ed0,
];
/// the single-line text's and the widget's code they call, run as the original
const ALSO: [u32; 9] = [0x2b1138, 0x2b1328, 0x2b1340, 0x2b14b8, 0x2b14e0, 0x2b15d0, 0x2b15e8, 0x2b1638, 0x2adaa8];

fn scrolling(f: u32) -> bool {
    (0x2b1b20..0x2b2760).contains(&f)
}

#[test]
fn text_widgets_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in FUNCS.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    let mut stubs = vec![
        wt::TB_INIT, wt::TB_SET_FONT, wt::TB_SET_RECT, wt::TB_CLEAR, wt::TB_APPEND, wt::TB_APPEND_NARROW, wt::TB_DRAW,
        wt::TB_SET_LINE_HEIGHT, wt::TB_SET_SCALE, wt::TB_WRAP, wt::TB_MAX_WIDTH, wt::TB_HEIGHT, wt::TB_DELETE,
        wt::WSTR_COPY, wt::WSTR_COPY_N, wt::STRCPY, wt::STRNCPY, wt::INPUT_UP, wt::INPUT_DOWN, wt::FILL, LOC_STRING,
        fw::DRAWER_COLOUR, fw::DRAW_WIDE_BORDER, fw::DRAW_WIDE, fw::DRAW_NARROW_BORDER, fw::DRAW_NARROW,
        fw::WIDTH_WIDE, fw::WIDTH_NARROW, fw::THING_DTOR, UPDATE_COLOR, rv(0x1d0), rv(0x1d8),
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
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e1b_0000_0000_0001);
    let per = 1500u32;
    let mut lines = 0usize;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x800])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app's renderer and its table; the widget's table; the two characters the scrolling text looks for
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
        let (hide, mark) = (g.pick(&[0x7eu16, 0x2a]), g.pick(&[0x23u16, 0x2a]));
        m.w16(wt::HIDE_CHAR, hide);
        m.w16(wt::MARK_CHAR, mark);
        // six strings: a first character that may hide or mark the line, then letters, spaces and an end
        for k in 0..6 {
            let s = STRS + 0x40 * k;
            let first = g.pick(&[hide, mark, 0x41, 0x20, 0x4141, 0]);
            m.w16(s, first);
            let len = g.below(20);
            for i in 1..len {
                let c = g.pick(&[0x41u16, 0x20, 0x4120, 0x2041, 0x42]);
                m.w16(s + 2 * i, c);
            }
            m.w16(s + 2 * len.max(1), 0);
            m.w16(s + 0x3e, 0);
        }
        m.w32(FONT + 0xc, g.pick(&[0, 1, 8, 12, 16, 20, 30]));
        m.w32(W + 0xc, VT);
        for o in [0x14u32, 0x10] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, any]));
        }
        m.w32(W + 0x70, g.below(16));
        for o in (0x80u32..0x110).step_by(4) {
            m.w32(W + o, g.f(-300.0, 600.0).to_bits());
        }
        if scrolling(func) {
            let n = g.pick(&[0u32, 1, 2, 3, 5, 10, 10, 20, 29, 30, u32::MAX]);
            for i in 0..30 {
                let id = g.pick(&[u32::MAX, u32::MAX, 0, 1, 2, 3, 4, 5]);
                m.w32(W + 0x110 + 4 * i, id);
            }
            m.w32(W + 0x188, FONT);
            m.w32(W + 0x18c, g.pick(&[1.0f32, 0.5, 1.25, 2.0]).to_bits());
            m.w32(W + 0x190, g.pick(&[0u32, 1, 2, 3]));
            let ids = [g.below(12) as i32 - 2, g.below(12) as i32 - 2, g.below(12) as i32 - 2];
            for (k, v) in ids.iter().enumerate() {
                m.w32(W + 0x194 + 4 * k as u32, *v as u32);
            }
            m.w32(W + 0x1a0, g.pick(&[0u32, 1, 1, 2]));
            let lh = m.r32(FONT + 0xc) as f32 * 1.25 + 3.0;
            m.w32(W + 0x1a4, g.f(-1.5 * lh, 1.5 * lh).to_bits());
            let (sp, gap) = (g.f(-1.0, 8.0), g.f(0.0, 10.0));
            m.w32(W + 0x1a8, g.pick(&[0.3f32, 0.05, 1.0, 4.0, sp]).to_bits());
            m.w32(W + 0x1ac, g.pick(&[3.0f32, 0.0, gap]).to_bits());
            m.w32(W + 0x1b0, n);
            m.w32(W + 0x1b4, g.below(2));
            m.w32(W + 0x1b8, g.below(2));
            // (the add's scan onward stops here at the latest)
            m.w32(W + 0x1bc, u32::MAX);
        } else {
            m.w32(W + 0x110, FONT);
            m.w32(W + 0x114, g.below(2));
            let sc = g.f(0.1, 3.0);
            m.w32(W + 0x118, g.pick(&[1.0f32, 0.75, 1.5, sc]).to_bits());
            m.w32(W + 0x11c, g.below(2));
            m.w32(W + 0x120, g.pick(&[u32::MAX, 0, 1, 7]));
            m.w32(W + 0x124, g.pick(&[0, STRS, STRS + 0x40, STRS + 0x80, STRS + 0xc0, STRS + 0x100, STRS + 0x140]));
            for o in (0x130u32..0x140).step_by(4) {
                m.w32(W + o, g.f(0.0, 1.0).to_bits());
            }
            m.w32(W + 0x140, g.pick(&[u32::MAX, 0, 1, 2, 3, 5, 9]));
        }
        for k in 0..8 {
            m.w32(VEC + 4 * k, g.f(-50.0, 300.0).to_bits());
        }
        let vec = VEC + g.pick(&[0u32, 0, 4, 8]);
        let out = OUT + g.pick(&[0u32, 0, 4]);
        let any = g.next();
        let a1 = g.pick(&[any, 0, 1, 5, STRS + 0x40]);
        let (a2, a3) = (g.next(), g.next());
        let xf = g.f(-1.0, 10.0);
        let x = g.pick(&[xf, 0.05, 0.1, 0.35, 4.0]);
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
            0x2b0b90 => wt::text_box_init(mm, e, W, a1, a2, a3, s),
            0x2b0dd8 => { wt::wtext_draw(mm, e, W); none }
            0x2b0e78 => { wt::wtext_vf12(mm, e, W); none }
            0x2b0ea0 => { wt::wtext_size(mm, e, W, out); none }
            0x2b0f18 => { wt::set_wide_text(mm, e, W, a1); none }
            0x2b0f68 => { wt::set_string_id(mm, e, W, a1); none }
            0x2b0fb8 => { wt::set_text(mm, e, W, a1); none }
            0x2b1008 => { wt::setup_text_box(mm, e, W, a1); none }
            0x2b1070 => { wt::load_string_id(mm, e, W, a1); none }
            0x2b10c8 => { wt::layout(mm, e, W, a1); none }
            0x2b1650 => { wt::buffered_set_wide(mm, e, W, a1); none }
            0x2b1690 => { wt::buffered_set_id(mm, e, W, a1); none }
            0x2b16a0 => { wt::buffered_set_narrow(mm, e, W, a1); none }
            0x2b1758 => wt::derived_dtor(mm, e, W, wt::COLOURED_VTABLE, flags),
            0x2b1770 => { wt::coloured_draw(mm, e, W, s); none }
            0x2b19f8 => { wt::coloured_vf12(mm, e, W); none }
            0x2b1a00 => { wt::coloured_size(mm, e, W, out); none }
            0x2b1a68 => { wt::coloured_find_split(mm, W); none }
            0x2b1b20 => wt::scroll_ctor(mm, e, W, a1, a2, a3),
            0x2b1d60 => { wt::scroll_vf39(mm, W); none }
            0x2b1da0 => { wt::add_line(mm, W, a1); none }
            0x2b1de8 => { wt::shift_up(mm, W); none }
            0x2b1e58 => { wt::shift_down(mm, W); none }
            0x2b1ea0 => wt::count_lines(mm, W),
            0x2b1ee8 => { wt::advance(mm, W); none }
            0x2b1fa8 => { wt::clear_lines(mm, W); none }
            0x2b1ff0 => { wt::set_speed(mm, W, F(x)); none }
            0x2b2048 => { wt::scroll_vf38(mm, W, vec); none }
            0x2b2080 => { wt::scroll_draw(mm, e, W, s); none }
            0x2b2420 => { wt::scroll_update(mm, e, W); none }
            0x2b2510 => wt::scroll_on_input(mm, e, W, a1),
            0x2b26c8 => { wt::scroll_size(mm, W, out); none }
            0x2b2750 => wt::scroll_dtor(mm, e, W, flags),
            0x2b4828 | 0x2b4940 => wt::text_one(),
            0x2b4830 => { wt::coloured_vf39(mm, e, W); none }
            0x2b4858 => { wt::coloured_set_colour(mm, W, vec); none }
            0x2b48b0 => wt::derived_ctor(mm, e, W, wt::COLOURED_VTABLE),
            0x2b4948 => wt::derived_dtor(mm, e, W, wt::BUFFERED_VTABLE, flags),
            0x2b4998 => wt::derived_ctor(mm, e, W, wt::BUFFERED_VTABLE),
            0x2b4cb8 => wt::wtext_vf24(),
            0x2b4cc0 => { wt::set_scale(mm, e, W, F(x)); none }
            0x2b4cc8 => { wt::apply_scale(mm, e, W, F(x)); none }
            0x2b4ce8 => { wt::wtext_vf13(mm, e, W, vec); none }
            0x2b4d38 => { wt::wtext_vf38(mm, e, W, vec); none }
            0x2b4d90 => { wt::wtext_vf39(mm, e, W); none }
            0x2b4df0 => { wt::wtext_vf9(mm, W, vec); none }
            0x2b4ed0 => wt::wtext_dtor(mm, e, W, flags),
            _ => unreachable!(),
        };
        lines += env.log.iter().filter(|c| c.0 == fw::DRAW_WIDE || c.0 == fw::DRAW_WIDE_BORDER).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2b0b90 | 0x2b1b20 => vec![W, a1, a2, a3],
            0x2b0ea0 | 0x2b1a00 | 0x2b26c8 => vec![W, out],
            0x2b2048 | 0x2b4858 | 0x2b4ce8 | 0x2b4d38 | 0x2b4df0 => vec![W, vec],
            0x2b1758 | 0x2b2750 | 0x2b4948 | 0x2b4ed0 => vec![W, flags],
            _ => vec![W, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, x);
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
    println!("{lines} lines drawn");
    assert!(lines > 2000);
}

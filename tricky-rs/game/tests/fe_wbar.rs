//! `tricky_game::fe_wbar` (board row G9x4) against the original in the function runner (TRICKY_ELF; skipped without
//! it). Every case builds a random progress bar, icon, entry or button in the arena: the bar's colours, range,
//! limits, value, step, way of filling, sprites, tiles and arrows; the icon's sprite, flip and tiles; the button's
//! children. The children, arrows and owner have tables pointing at stand-ins (with random `this` shifts). One of the
//! row's functions then runs on both sides. The row's own helpers and the widget's (`fe_widget`, `fe_wgroup`,
//! `fe_wentry`) run for real; every other callee is a stand-in on both sides answering from the case's seed. The
//! calls are compared with their float arguments (f12..f17), the colours, vectors and rectangles they are given
//! (read by value), and the memory after. Floats come back from the steps in f0, and that is compared too. The
//! runner's stack is cleared before each case and its addresses log as one token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_wbar as wb;
use tricky_game::fe_wentry as we;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widget as fw;
use tricky_game::fe_widgets::{ICON_VTABLE, UPDATE_COLOR};
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
const ICONVT: (u32, u32) = (0x3b6f00, 0x200);
const COLOURS: (u32, u32) = (0x3d3600, 0x300);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const W: u32 = ARENA;
const A1: u32 = ARENA + 0x400;
const A2: u32 = ARENA + 0x600;
const OWNER: u32 = ARENA + 0x800;
const C1: u32 = ARENA + 0xa00;
const VT: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x1200;
const APPOBJ: u32 = ARENA + 0x1300;
const RENDER: u32 = ARENA + 0x1380;
const RECT: u32 = ARENA + 0x1400;
const VEC: u32 = ARENA + 0x1440;
const OUT: u32 = ARENA + 0x1480;
const INPUT: u32 = ARENA + 0x14c0;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 13] = [8, 0x20, 0x28, 0x38, 0x50, 0x60, 0x70, 0xc0, 0xf8, 0x130, 0x138, 0x140, 0x150];

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

/// A callee's integer and float arguments
fn arity(f: u32, icon_fns: &[u32]) -> (usize, usize) {
    match f {
        wb::DRAW_RECT => (2, 4),
        wb::DRAW_TILED => (2, 6),
        wb::DRAW_SHADED => (6, 6),
        wg::TWEEN_RUN => (5, 0),
        wg::TWEEN_START => (3, 0),
        wb::STATE_DTOR | wb::STRCPY | wb::LIST_PUSH_BACK | we::SET_WORLD_POS | we::GET_RECT | fw::DRAWER_COLOUR
        | fw::THING_DTOR => (2, 0),
        f if f == rv(0x1d0) => (2, 0),
        f if f == slot(0x38) || f == slot(0x60) || f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (2, 0),
        f if icon_fns.contains(&f) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers, icon_fns: &[u32]) -> (u32, Call) {
    let (ni, nf) = arity(f, icon_fns);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a colour, vector or rectangle by value; the shaded draw's four colours)
    let by_value = (f & 0xffff_0000 == 0x0180_0000 || icon_fns.contains(&f) || f == fw::DRAWER_COLOUR
        || f == we::SET_WORLD_POS || f == wg::TWEEN_START || f == wg::TWEEN_RUN)
        && ni >= 2;
    if by_value && readable(w[1]) {
        log.extend(deref(io, w[1], 4));
    }
    if f == wg::TWEEN_START && readable(w[2]) {
        log.extend(deref(io, w[2], 4));
    }
    if f == wb::DRAW_SHADED {
        for k in 2..6 {
            log.extend(deref(io, w[k], 4));
        }
    }
    // (the bar's table as its arrows are destroyed)
    if f == slot(8) {
        log.push(io.r32(W + 0xc));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 60.0).to_bits());
        }
    }
    let answer = match f {
        f if f == rv(0x1d8) => r % 8,
        we::GET_RECT => RECT,
        wb::INPUT_LEFT | wb::INPUT_RIGHT | wb::INPUT_UP | wb::INPUT_DOWN | wb::INPUT_ACCEPT => (r % 3 == 0) as u32,
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
        let (ni, nf) = arity(f, self.icon_fns);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(6, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans, self.icon_fns);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 41] = [
    0x2aab58, 0x2ab910, 0x2abb58, 0x2abb60, 0x2abbc0, 0x2abc68, 0x2ac698, 0x2aca88, 0x2acaa8, 0x2acb70, 0x2acc40,
    0x2ad148, 0x2ad1c8, 0x2adcb8, 0x2adce8, 0x2adf88, 0x2adfc8, 0x2ae230, 0x2ae3c0, 0x2ae760, 0x2ae7f0, 0x2ae880,
    0x2aef28, 0x2aff90, 0x2b2ec0, 0x2b32a8, 0x2b3300, 0x2b37d8, 0x2b37e0, 0x2b3820, 0x2b3860, 0x2b39b8, 0x2b3a20,
    0x2b3d10, 0x2b3f38, 0x2b3f60, 0x2b4190, 0x2b43e8, 0x2b4408, 0x2b4620, 0x2b4ab0,
];
/// the widget's code they call, run as the original: the sprite's rectangle, the placing and sliding in, the
/// widget's destructor
const ALSO: [u32; 4] = [0x2ac4b0, 0x2addb0, 0x2ade40, 0x2adaa8];

fn bar(f: u32) -> bool {
    (0x2ae3c0..=0x2aef28).contains(&f) || (0x2b37d8..=0x2b3d10).contains(&f)
}

#[test]
fn bars_and_pictures_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in FUNCS.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let (data, iconvt, colours) = (read(&mut r, DATA), read(&mut r, ICONVT), read(&mut r, COLOURS));
    // the icon's own virtuals that its constructor's size reaches through its table
    let icon_fns: Vec<u32> = (0..0x160u32)
        .step_by(8)
        .map(|off| u32::from_le_bytes(iconvt[(ICON_VTABLE + off + 4 - ICONVT.0) as usize..][..4].try_into().unwrap()))
        // (the row's own and the widget's code in that table run for real)
        .filter(|&f| f != 0 && !FUNCS.contains(&f) && !ALSO.contains(&f))
        .collect();
    let icon_fns: &'static [u32] = Box::leak(icon_fns.into_boxed_slice());
    let mut stubs = vec![
        wb::STATE_DTOR, wb::STRCPY, wb::LIST_PUSH_BACK, wb::DRAW_RECT, wb::DRAW_TILED, wb::DRAW_SHADED, wb::INPUT_LEFT,
        wb::INPUT_RIGHT, wb::INPUT_UP, wb::INPUT_DOWN, wb::INPUT_ACCEPT, we::LIST_FIRST, we::SET_WORLD_POS,
        we::GET_RECT, wg::LIST_INIT, wg::TWEEN_START, wg::TWEEN_RUN, fw::DRAWER_COLOUR, fw::THING_DTOR, UPDATE_COLOR,
        rv(0x1d0), rv(0x1d8),
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
            let fl: Vec<u32> = (12..18).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a, icon_fns);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x9e1e_0000_0000_0001);
    let per = 1500u32;
    let mut drawn = 0usize;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (ICONVT.0, iconvt.clone()),
                (COLOURS.0, colours.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0xa5; 0x800]),
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
        for o in [W, A1, A2, OWNER, C1, C1 + 0x200] {
            m.w32(o + 0xc, VT);
            for k in 0..4 {
                m.w32(o + 0x80 + 4 * k, g.f(-200.0, 500.0).to_bits());
                m.w32(o + 0x100 + 4 * k, g.f(0.0, 300.0).to_bits());
            }
        }
        for k in 0..16 {
            m.w32(RECT + 4 * k, g.f(0.0, 40.0).to_bits());
            m.w32(VEC + 4 * k, g.f(-50.0, 50.0).to_bits());
        }
        for o in [0x14u32, 0xa0, 0xa4] {
            let any = g.next();
            m.w32(W + o, g.pick(&[0u32, 1, any]));
        }
        m.w32(W + 0x70, g.below(16));
        m.w32(W + 0xec, g.pick(&[0, OWNER, OWNER]));
        m.w32(INPUT, g.pick(&[0u32, 1, 1]));
        if bar(func) {
            for o in (0x110u32..0x150).step_by(4) {
                m.w32(W + o, g.f(0.0, 1.0).to_bits());
            }
            let (top, bottom) = (g.f(10.0, 200.0), g.f(-50.0, 10.0));
            m.w32(W + 0x150, g.pick(&[top, top, bottom]).to_bits());
            m.w32(W + 0x154, bottom.to_bits());
            for o in [0x158u32, 0x15c] {
                let v = g.f(bottom - 20.0, top + 20.0);
                m.w32(W + o, g.pick(&[-9999.0f32, v, v]).to_bits());
            }
            let v = g.f(bottom - 20.0, top + 20.0);
            // (and now and then below the no-limit mark, so that it is seen to mean none)
            m.w32(W + 0x160, g.pick(&[v, v, top, bottom, -9998.0, -10000.0]).to_bits());
            m.w32(W + 0x164, g.f(0.5, 20.0).to_bits());
            m.w32(W + 0x168, g.pick(&[u32::MAX, 0, 1, 2, 3, 0, 1, 2, 3, 4]));
            m.w32(W + 0x16c, g.below(50));
            m.w32(W + 0x170, g.pick(&[u32::MAX, 0, 7, 9]));
            m.w32(W + 0x174, g.below(50));
            for o in [0x178u32, 0x17c, 0x180, 0x184] {
                m.w32(W + o, g.f(0.5, 4.0).to_bits());
            }
            m.w32(W + 0x188, g.pick(&[0, A1, A1]));
            m.w32(W + 0x18c, g.pick(&[0, A2, A2]));
        } else if func == 0x2ae230 {
            m.w32(W + 0x110, C1);
            m.w32(W + 0x114, C1 + 0x200);
        } else if func == 0x2adce8 || func == 0x2adfc8 {
            m.w32(W + 0x110, g.below(50));
            m.w32(W + 0x114, g.below(9));
            m.w32(W + 0x120, g.f(0.5, 4.0).to_bits());
            m.w32(W + 0x124, g.f(0.5, 4.0).to_bits());
        } else {
            m.w32(W + 0x110, g.pick(&[0, C1]));
            m.w32(W + 0x114, g.pick(&[0, C1 + 0x200]));
        }
        // (a random argument kept out of the stack's range, where the logging would read through it)
        let any = g.next() | 0x8000_0000;
        let a1 = g.pick(&[any, 0, 1, 3, VEC]);
        let (a2, flags) = (g.next() | 0x8000_0000, g.pick(&[0u32, 1, 2, 3]));
        let fs = [g.f(-100.0, 400.0), g.f(-100.0, 400.0), g.f(-100.0, 400.0), g.f(-1.0, 1.0)];
        let vec = VEC + g.pick(&[0u32, 0, 4]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        // (the stack and the port's scratch filled alike, so a word left unwritten is seen)
        r.mem.write(0x01ff_0000 - 0x4000, &[0xa5u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans, icon_fns };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let s = SCRATCH;
        let f4 = [F(fs[0]), F(fs[1]), F(fs[2]), F(fs[3])];
        let mut ours_f: Option<F> = None;
        let ours = match func {
            0x2aab58 => wb::state_dtor(mm, e, W, wb::OPTIONS_VTABLE, flags),
            0x2ab910 => { wb::video_vf11(); none }
            0x2abb58 => wb::state_set_name(mm, e, W, a1),
            0x2abb60 => wb::state_dtor(mm, e, W, wb::VIDEO_VTABLE, flags),
            0x2abbc0 => wb::video_vf13(),
            0x2abc68 => wb::add_widget(mm, e, W, a1),
            0x2ac698 => wb::first_widget(mm, e, W),
            0x2aca88 => { wb::set_90(mm, W, vec); none }
            0x2acaa8 => { wb::set_90_xyz(mm, W, f4[0], f4[1], f4[2], s); none }
            0x2acb70 => { wb::set_world_xyzw(mm, e, W, f4, s); none }
            0x2acc40 => { wb::set_colour_rgba(mm, e, W, f4, s); none }
            0x2ad148 => wb::op_skip(mm, e, A1),
            0x2ad1c8 => wb::op_move(mm, e, VEC, A1),
            0x2adcb8 => wb::vec_scale(mm, OUT, VEC, f4[0]),
            0x2adce8 => { wb::icon_draw(mm, e, W, false); none }
            0x2adf88 => { wb::slide_in_3(mm, e, W, s); none }
            0x2adfc8 => { wb::icon_draw(mm, e, W, true); none }
            0x2ae230 => { wb::button_layout_2(mm, e, W, a1, a2, s); none }
            0x2ae3c0 => { wb::update_fill(mm, e, W, s); none }
            0x2ae760 => { ours_f = Some(wb::step_up(mm, e, W, s)); none }
            0x2ae7f0 => { ours_f = Some(wb::step_down(mm, e, W, s)); none }
            0x2ae880 => { wb::bar_draw(mm, e, W, s); none }
            0x2aef28 => wb::bar_on_input(mm, e, W, INPUT, s),
            0x2aff90 => { wb::listbox_add(mm, e, W, a1); none }
            0x2b2ec0 => wb::listbox_ctor_bare(mm, e, W),
            0x2b32a8 => { wb::entry_set_child(mm, e, W, 0x114, a1); none }
            0x2b3300 => { wb::entry_set_child(mm, e, W, 0x110, a1); none }
            0x2b37d8 | 0x2b3f38 | 0x2b43e8 => wb::widget_one(),
            0x2b37e0 => { wb::bar_vf39(mm, e, W, s); none }
            0x2b3820 => { wb::set_size_one(mm, e, W, true, f4[0], s); none }
            0x2b3860 => { wb::set_size_one(mm, e, W, false, f4[0], s); none }
            0x2b39b8 => { wb::bar_vf1(mm, e, W); none }
            0x2b3a20 => wb::bar_dtor(mm, e, W, flags),
            0x2b3d10 => wb::bar_ctor(mm, e, W),
            0x2b3f60 => wb::tiled_dtor(mm, e, W, flags),
            0x2b4190 => wb::tiled_ctor(mm, e, W, s),
            0x2b4408 => wb::icon_dtor(mm, e, W, flags),
            0x2b4620 => wb::icon_ctor(mm, e, W),
            0x2b4ab0 => wb::text_ctor_bare(mm, e, W),
            _ => unreachable!(),
        };
        drawn += env.log.iter().filter(|c| c.0 == wb::DRAW_SHADED || c.0 == wb::DRAW_TILED || c.0 == wb::DRAW_RECT).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x2aab58 | 0x2abb60 | 0x2b3a20 | 0x2b3f60 | 0x2b4408 => vec![W, flags],
            0x2aca88 => vec![W, vec],
            0x2ad148 => vec![VEC, A1],
            0x2ad1c8 => vec![VEC, A1],
            0x2adcb8 => vec![OUT, VEC],
            0x2ae230 => vec![W, a1, a2],
            0x2aef28 => vec![W, INPUT],
            _ => vec![W, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        for (k, v) in fs.iter().enumerate() {
            r.cpu.set_f(12 + k, *v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        if let Some(v) = ours_f {
            assert_eq!(r.cpu.f(0).to_bits(), v.0.to_bits(), "case {case} {func:#x}: the float answer");
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
    println!("{drawn} sprites drawn");
    assert!(drawn > 2000);
}

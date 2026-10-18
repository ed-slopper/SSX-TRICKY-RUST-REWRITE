//! `tricky_game::fe_misc` (board row G9y6) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds random objects in the arena (cameras, animations, states, entries, an options
//! page with its lists and value widgets, a state with a title group) whose tables point at stand-ins, random sound
//! player answers, career settings and the item table, then runs one of the row's functions on both sides. The
//! widgets' code they reach runs for real; every other callee is a stand-in on both sides answering from the
//! case's seed (floats in f0 as well). The calls are compared with their arguments (floats from f12), the vectors
//! they are given (read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_misc as fm;
use tricky_game::fe_wentry as we;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widget::THING_DTOR;
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
const CAREER: (u32, u32) = (0x32b000, 0x2000);
const ITEMS: (u32, u32) = (0x347f00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const W: u32 = ARENA;
const W0: u32 = ARENA + 0x1000;
const NW: u32 = 8;
const VT: u32 = ARENA + 0x2800;
const APPOBJ: u32 = ARENA + 0x2c00;
const FE: u32 = ARENA + 0x2c80;
const SND: u32 = ARENA + 0x2d00;
const SVT: u32 = ARENA + 0x2e00;
const VEC: u32 = ARENA + 0x3000;
const OUT: u32 = ARENA + 0x3040;
const RECT: u32 = ARENA + 0x3080;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn sv(off: u32) -> u32 {
    0x0181_0000 + off
}
fn wdg(k: u32) -> u32 {
    W0 + 0x200 * k
}
const SLOTS: [u32; 10] = [8, 0x20, 0x28, 0x30, 0x48, 0x130, 0x138, 0x150, 0x160, 0xf8];
const SND_SLOTS: [u32; 8] = [0x130, 0x140, 0x150, 0x160, 0x170, 0x190, 0x1a0, 0x1b0];

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
    // (the runner's stack top and the port's scratch, not every word that falls in the stack's range)
    (0x01fe_0000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x7f0).contains(&a) || (ARENA..ARENA + SIZE - 0x40).contains(&a)
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    /// whether a find may answer none (only the group count looks)
    nulls: Cell<bool>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        wg::TWEEN_RUN => (5, 0),
        fm::THING_FLOAT => (1, 1),
        fm::ALLOC | fm::FILL | fm::THING_CTOR => (3, 0),
        fm::CAMERA_SHOT | fm::STATE_DTOR | fm::STATE_ADD | fm::STATE_RECT | fm::FIND | fm::NTH
        | we::SET_WORLD_POS | THING_DTOR => (2, 0),
        f if f == slot(0x48) => (1, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (2, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (1, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    if ni >= 2 && f != slot(0x130) && readable(w[1]) && f != fm::FIND && f != fm::NTH {
        log.extend(deref(io, w[1], 4));
    }
    if f == wg::TWEEN_RUN {
        log.extend(deref(io, w[2], 4));
    }
    // (the object's table as its parts are destroyed)
    if f == fm::SCRIPTED_PART_DTOR || f == THING_DTOR || f == fm::STATE_DTOR {
        log.push(io.r32(W + 0xc));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 200.0).to_bits());
        }
    }
    let answer = match f {
        fm::FIND if ans.nulls.get() && r % 4 == 0 => 0,
        fm::FIND | fm::NTH | fm::THING_CTOR => wdg((r >> 3) % NW),
        fm::ALLOC => [0, wdg(r % NW)][(r % 2) as usize],
        fm::STATE_RECT => RECT,
        f if f & 0xffff_0000 == 0x0181_0000 || f == fm::BRIGHTNESS => g.f(0.0, 1.0).to_bits(),
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

#[test]
fn front_end_odds_and_ends_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let mut funcs: Vec<u32> = vec![
        0x26c370, 0x26c450, 0x26c4f0, 0x26c500, 0x26c5d0, 0x26c680, 0x26c820, 0x26c9a0, 0x26cc40, 0x26cd98, 0x26cde8,
        0x26cdf8, 0x270218, 0x273f18, 0x2754d8, 0x275760, 0x2759f0, 0x275d50, 0x275dc8, 0x28e020, 0x293120, 0x2932a8,
        0x293320, 0x29ad00, 0x29ef50, 0x29f430, 0x29f5f0,
    ];
    funcs.extend(fm::STATE_DTORS.map(|p| p.0));
    funcs.extend(fm::SCREEN_NUMBERS.map(|p| p.0));
    funcs.extend(fm::NOTHINGS);
    funcs.extend(fm::ZEROS);
    funcs.extend(fm::ONES);
    assert_eq!(funcs.len(), 75);
    for f in &funcs {
        r.allow(*f);
    }
    // the widgets' code they reach: the node's vf1 and destructor, the list's, the entry's
    // destructor and vf1, the widget's destructor, the placing, the bar's value
    for f in [0x2acf28u32, 0x2acf60, 0x2ace78, 0x2b3358, 0x2af550, 0x2adaa8, 0x2addb0, 0x2b3950] {
        r.allow(f);
    }
    let mut stubs = vec![
        fm::CAMERA_SHOT, fm::SCRIPTED_PART_DTOR, fm::THING_CTOR, fm::THING_FLOAT, fm::STATE_DTOR, fm::STATE_ADD,
        fm::STATE_RECT, fm::STATE_274DF0, fm::FIND, fm::NTH, fm::ALLOC, fm::FILL, fm::BRIGHTNESS, wg::FREE,
        wg::TWEEN_RUN, wg::LIST_DELETE_ALL, wg::NODE_UNLINK, wg::BAR_SET, we::SET_WORLD_POS, THING_DTOR,
        UPDATE_COLOR,
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SND_SLOTS.map(sv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), nulls: Cell::new(false) });
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
    let mut g = Rng(0x9e22_0000_0000_0001);
    let per = 800u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut career = vec![0u8; CAREER.1 as usize];
        let mut items = vec![0u8; ITEMS.1 as usize];
        for b in career.iter_mut() {
            *b = g.next() as u8;
        }
        for b in items.iter_mut() {
            *b = g.below(3) as u8;
        }
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (CAREER.0, career),
                (ITEMS.0, items),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(FE + 0xa4, SND);
        m.w32(SND, SVT);
        for off in SND_SLOTS {
            m.w16(SVT + off, g.pick(&[0u16, 8]));
            m.w32(SVT + off + 4, sv(off));
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for o in [W].into_iter().chain((0..NW).map(wdg)) {
            m.w32(o + 0xc, VT);
            for k in 0..4 {
                m.w32(o + 0x80 + 4 * k, g.f(-100.0, 400.0).to_bits());
            }
        }
        for k in 0..NW {
            m.w32(wdg(k) + 0x114, wdg(g.below(NW)));
            m.w32(wdg(k) + 0x110, wdg(g.below(NW)));
            m.w32(wdg(k) + 0x118, wdg(g.below(NW)));
        }
        // the page's lists and the state's groups
        for at in [0x40u32, 0x48, 0x80, 0x8c, 0x98, 0x114] {
            m.w32(W + at, wdg(g.below(NW)));
        }
        m.w32(W + 0x40, g.pick(&[0, wdg(1), wdg(2)]));
        m.w32(W + 0x48, g.pick(&[0, wdg(3)]));
        m.w32(W + 0x438, VT);
        // (an entry's two children, or none)
        m.w32(W + 0x110, g.pick(&[0, wdg(4), wdg(5)]));
        m.w32(W + 0x114, g.pick(&[0, wdg(6), wdg(7)]));
        // (the child placed is there: the game doesn't look)
        if func == 0x28e020 {
            m.w32(W + 0x114, wdg(6));
        }
        for k in 0..8 {
            // (zero lanes too, for the sign of a zero product)
            let x = g.f(-50.0, 50.0);
            m.w32(VEC + 4 * k, g.pick(&[x, x, 0.0, -0.0]).to_bits());
            m.w32(RECT + 4 * k, g.f(0.0, 60.0).to_bits());
        }
        m.w32(W, g.pick(&[13u32, 27, 0, 1]));
        let any = g.next() | 0x8000_0000;
        let a1 = g.pick(&[any, 0, 1, 3]);
        let flags = g.pick(&[0u32, 1, 2, 3]);
        let fs = [g.f(-100.0, 400.0), g.f(-100.0, 400.0)];
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.nulls.set(func == 0x2754d8);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let s = SCRATCH;
        let ours = match func {
            0x26c370 => fm::camera_play_shot(mm, e, W, a1),
            0x26c450 => { fm::camera_combined_dtor(mm, e, W, flags); none }
            0x26c4f0 => fm::camera_scripted_vf28(mm, W),
            0x26c500 => { fm::camera_scripted_dtor(mm, e, W, flags); none }
            0x26c5d0 => { fm::camera_dtor(mm, e, W, flags); none }
            0x26c680 => { fm::anim_kind_dtor(mm, e, W, fm::ANIM_TRANSLATE_VTABLE, flags); none }
            0x26c820 => { fm::anim_kind_dtor(mm, e, W, fm::ANIM_ROTATE_VTABLE, flags); none }
            0x26c9a0 => { fm::anim_dtor(mm, e, W, fm::ANIM_VTABLE, flags); none }
            0x26cc40 => { fm::anim_dtor(mm, e, W, fm::GEOMETRY_VTABLE, flags); none }
            // (the vectors handed unaligned: the game reads and writes the quadwords they are in)
            0x26cd98 => fm::vec_scale(mm, OUT + 4, VEC + 8, F(fs[0])),
            0x26cde8 => fm::input_is(mm, W, 0xd),
            0x26cdf8 => fm::input_is(mm, W, 0x1b),
            0x270218 => { fm::free_on_flag(mm, e, W, flags); none }
            0x273f18 => { fm::create_dialog(mm, e, W, a1, F(fs[0]), s); none }
            0x2754d8 => fm::count_groups(mm, e, W),
            0x275760 => { fm::set_title(mm, e, W, a1, s); none }
            0x2759f0 => { fm::lists_dtor(mm, e, W, flags); none }
            0x275d50 => { fm::text_icon_dtor(mm, e, W, flags); none }
            0x275dc8 => { fm::text_entry_dtor(mm, e, W, flags); none }
            0x28e020 => { fm::place_child(mm, e, W, a1, F(fs[0]), F(fs[1]), s); none }
            0x293120 => { fm::overlay_state_update(mm, e, W); none }
            0x2932a8 => { fm::icon_text_dtor(mm, e, W, flags); none }
            0x293320 => { fm::icon_entry_dtor(mm, e, W, flags); none }
            0x29ad00 => fm::num_players_vf26(mm, e, W),
            0x29ef50 => { fm::sound_refresh(mm, e, W); none }
            0x29f430 => { fm::game_refresh(mm, e, W); none }
            0x29f5f0 => { fm::display_refresh(mm, e, W); none }
            f if fm::STATE_DTORS.iter().any(|p| p.0 == f) => {
                let t = fm::STATE_DTORS.iter().find(|p| p.0 == f).unwrap().1;
                fm::state_dtor(mm, e, W, t, flags)
            }
            f if fm::SCREEN_NUMBERS.iter().any(|p| p.0 == f) => fm::SCREEN_NUMBERS.iter().find(|p| p.0 == f).unwrap().1,
            f if fm::NOTHINGS.contains(&f) => { fm::nothing(); none }
            f if fm::ZEROS.contains(&f) => fm::zero(),
            f if fm::ONES.contains(&f) => fm::one(),
            _ => unreachable!(),
        };
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x26cd98 => vec![OUT + 4, VEC + 8],
            0x26c450 | 0x26c500 | 0x26c5d0 | 0x26c680 | 0x26c820 | 0x26c9a0 | 0x26cc40 | 0x270218 | 0x2759f0
            | 0x275d50 | 0x275dc8 | 0x2932a8 | 0x293320 => vec![W, flags],
            f if fm::STATE_DTORS.iter().any(|p| p.0 == f) => vec![W, flags],
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
}

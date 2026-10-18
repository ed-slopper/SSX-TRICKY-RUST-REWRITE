//! The front end's small helpers of board row G9y7 in `tricky_game::fe_misc` against the original in the function
//! runner (TRICKY_ELF; skipped without it): the venue cameras, the world's render, text and update passes, the
//! state's constructor and destructor, the sprite rectangle, the group add, the two dialog screens, the focus and
//! the help bar's layout. Every case builds a random camera, state, world (its table at +0x20), help bar with
//! widgets and an app in the arena; one function then runs on both sides with random arguments. The camera's shot,
//! the thing and list helpers' code they reach run as the original; every other callee is a stand-in on both sides
//! answering from the case's seed. The calls are compared with their arguments (floats from f12; an operation or
//! vector on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_misc as fm;
use tricky_game::fe_wgroup as wg;
use tricky_game::fe_widgets::LIST_INIT;
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
/// the render's matrix the render pass copies
const MATRIX: (u32, u32) = (0x346400, 0x80);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const C: u32 = ARENA;
const S: u32 = ARENA + 0x200;
const WORLD: u32 = ARENA + 0x300;
const TV: u32 = ARENA + 0x380;
const VT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x600;
const FE: u32 = ARENA + 0x700;
const RECTS: u32 = ARENA + 0x800;
const BAR: u32 = ARENA + 0x900;
const W0: u32 = ARENA + 0xa00;
const NW: u32 = 4;
fn wdg(k: u32) -> u32 {
    W0 + 0x200 * k
}
const HEAP: (u32, u32) = (0x0110_0000, 0x1000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
const APPLY: u32 = 0x0180_0010;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 3] = [0x130, 0x138, 0xf0];

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
    (0x01fe_0000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        fm::CREATE_DIALOG => (2, 1),
        fm::WORLD_GROUP | fm::ALLOC | fm::FILL => (3, 0),
        fm::CAMERA_SHOT | fm::SCRIPTED_PART_26AC30 | fm::LIST_ADD | fm::RENDER_OP_DTOR | fm::TEXT_OP_DTOR
        | fm::UPDATE_OP_DTOR | fm::SPRITE_SET_RECT | fm::CREATE_LIST | fm::CREATE_LIST2 | fm::STATE_BUTTONS
        | fm::NTH | APPLY => (2, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (an operation or vector on the stack by value; not the size call's buffer, which it only writes)
    if ni >= 2 && stack(w[1]) && f != slot(0x130) {
        log.extend((0..4).map(|k| token(io.r32(w[1] + 4 * k))));
        // (the render operation's copy of the matrix too)
        if f == APPLY && io.r32(w[1]) == fm::RENDER_OP_VTABLE {
            let at = io.r32(w[1] + 4);
            if stack(at) {
                log.extend((0..16).map(|k| io.r32(at + 4 * k)));
            }
        }
    }
    // (the state's table as its vf12 runs)
    if f == fm::STATE_VF12 {
        log.push(io.r32(w[0] + 0xc));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 300.0).to_bits());
        }
    }
    let answer = match f {
        fm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        fm::SPRITE_SET_RECT => RECTS + 0x10 * (r % 8),
        fm::NTH => wdg(r % NW),
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

const FUNCS: [u32; 15] = [
    0x26c090, 0x26c1a8, 0x26c2c0, 0x26c430, 0x26feb8, 0x26ff40, 0x26ff88, 0x2715c8, 0x2717a0, 0x2759c8, 0x2762e0,
    0x274208, 0x274278, 0x275468, 0x2755e8,
];

#[test]
fn front_end_small_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    // the code they reach: the camera's shot, the free on flag, the node's destructor and vf1
    for f in [0x26c370u32, 0x270218, 0x2acf60, 0x2acf28] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        fm::CAMERA_SHOT, fm::SCRIPTED_PART_26AC30, fm::LIST_ADD, fm::RENDER_OP_DTOR, fm::TEXT_OP_DTOR,
        fm::UPDATE_OP_DTOR, fm::STATE_VF12, fm::SPRITE_SET_RECT, fm::WORLD_GROUP, fm::CREATE_DIALOG, fm::CREATE_LIST,
        fm::CREATE_LIST2, fm::STATE_BUTTONS, fm::ALLOC, fm::FILL, LIST_INIT, fm::NTH, wg::FREE, wg::NODE_UNLINK, APPLY,
    ];
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
    let mut matrix = vec![0u8; MATRIX.1 as usize];
    r.mem.read(MATRIX.0, &mut matrix).unwrap();
    let mut g = Rng(0x9e77_0000_0000_0001);
    let per = 1000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut mx = matrix.clone();
        for b in mx.iter_mut() {
            *b = g.next() as u8;
        }
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (MATRIX.0, mx),
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
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x734, FE);
        // the camera's kept venue, any byte
        m.w8(C + 0x185, g.pick(&[0u8, 3, 9, 10, 0x7f, 0x80, 0xff]));
        // the world and its table
        m.w32(WORLD + 0x20, TV);
        m.w16(TV + 0x10, g.pick(&[0u16, 0x20, 0xfff0]));
        m.w32(TV + 0x14, APPLY);
        // the state: its focus, root group (+0x38), menu (+0x3c) and help bar (+0x44)
        m.w32(S + 0x30, g.pick(&[0, wdg(0), wdg(1)]));
        m.w32(S + 0x44, BAR);
        for o in [S, BAR].into_iter().chain((0..NW).map(wdg)) {
            m.w32(o + 0xc, VT);
        }
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the heights from a few values, which the help bar's height may equal)
        for k in 0..32 {
            let x = g.f(-10.0, 120.0);
            m.w32(RECTS + 4 * k, g.pick(&[x, 10.0, 25.0]).to_bits());
        }
        let any = g.next() | 0x8000_0000;
        let k = g.pick(&[0u32, 1, 4, 9, 10, 0xff, 0x80, 0x109, any]);
        let mut a1 = g.pick(&[any, 0, wdg(2), wdg(3), k]);
        // (the widget focused is one or none: the game calls on it)
        if func == 0x275468 {
            a1 = g.pick(&[0, wdg(2), wdg(3)]);
        }
        let a2 = g.next() | 0x8000_0000;
        let flags = g.pick(&[0u32, 1, 2, 3]);
        let (fa, fb) = (g.f(-50.0, 400.0), g.f(0.0, 200.0));
        let f12 = g.pick(&[fa, fb, 0.0, 10.0, 25.0]);
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
        let (ours, regs): (u32, Vec<u32>) = match func {
            0x26c090 => (fm::pan_to_venue(mm, e, C, k), vec![C, k]),
            0x26c1a8 => (fm::play_venue_shot(mm, e, C, k), vec![C, k]),
            0x26c2c0 => (fm::play_current_shot(mm, e, C), vec![C]),
            0x26c430 => (fm::camera_part_26c430(mm, e, C, a1), vec![C, a1]),
            0x26feb8 => { fm::render_all(mm, e, WORLD, SCRATCH); (none, vec![WORLD]) }
            0x26ff40 => { fm::text_render_all(mm, e, WORLD, SCRATCH); (none, vec![WORLD]) }
            0x26ff88 => { fm::update_all(mm, e, WORLD, SCRATCH); (none, vec![WORLD]) }
            0x2715c8 => (fm::state_ctor(mm, e, S), vec![S]),
            0x2717a0 => { fm::state_base_dtor(mm, e, S, flags); (none, vec![S, flags]) }
            0x2759c8 => (fm::sprite_rect(mm, e, k), vec![S, k]),
            0x2762e0 => (fm::group_add(mm, e, S, a1), vec![S, a1]),
            0x274208 => (fm::dialog_screen(mm, e, S, a1, a2, F(f12), false), vec![S, a1, a2]),
            0x274278 => (fm::dialog_screen(mm, e, S, a1, a2, F(f12), true), vec![S, a1, a2]),
            0x275468 => { fm::set_focused(mm, e, S, a1); (none, vec![S, a1]) }
            0x2755e8 => { fm::help_bar_layout(mm, e, S, F(f12), SCRATCH); (none, vec![S]) }
            _ => unreachable!(),
        };
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f12);
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

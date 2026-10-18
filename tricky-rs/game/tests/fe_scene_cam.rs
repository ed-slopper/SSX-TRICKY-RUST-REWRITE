//! `tricky_game::fe_scene`'s cameras (board row G9z5) against the original in the function runner (TRICKY_ELF;
//! skipped without it): `cFE3DCamera`'s constructor, world draw and view, `cFE3DCameraScripted`'s set-up, frame,
//! shot and view, `cFE3DCameraCombined`'s, the screens' camera shots and `FE3DCamera_SetMode`. Every case builds a
//! random camera (shown or not, its angles, position, lens, mode, rider and flag), camera animation frame, front end
//! and app in the arena; one function then runs on both sides. The cameras' own functions and the game's
//! `Math_SinCos`, `Math_Atan` and `Vec4_Scale` run for real on both sides; every other callee is a stand-in answering
//! from the case's seed: the shots played, the world's draw, the pool (handed out in order), the frame's maker (the
//! case's frame), and the objects' virtuals (a table of stand-ins). The calls are compared with their arguments (a
//! matrix or vector on the stack read by value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_scene as sc;
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
/// the game's words: the identity the view starts from, the shots
const GAME: [(u32, u32); 2] = [(0x346400, 0x100), (0x347200, 0x200)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const APPOBJ: u32 = ARENA + 0x800;
const FE: u32 = ARENA + 0xa00;
const VT: u32 = ARENA + 0x1000;
const R: u32 = ARENA + 0x1400;
const C: u32 = ARENA + 0x1800;
const INNER: u32 = ARENA + 0x1a00;
const FRAME: u32 = ARENA + 0x1c00;
const POS: u32 = ARENA + 0x1f00;
const HEAP: (u32, u32) = (0x0110_0000, 0x4000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 8] = [8, 0x48, 0x58, 0x68, 0xa8, 0xe8, 0xf0, 0x14];
const REAL: [u32; 3] = [0x250c10, 0x2510f8, 0x102f50];

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
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        sc::POOL_ALLOC => (6, 0),
        sc::WORLD_DRAW_BEGIN | sc::FILL | sc::CAM_FRAME_CREATE => (3, 0),
        sc::WORLD_DRAW => (2, 1),
        f if f == slot(0xa8) => (1, 3),
        f if f == slot(0x48) => (1, 0),
        _ => (2, 0),
    }
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

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a matrix (the view), the frame's five parameters or a vector on the stack by value)
    let words = if f == slot(0xf0) { 16 } else if f == sc::CAM_FRAME_CREATE || f == sc::CAM_SET_SHOT { 5 } else { 4 };
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..words).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let answer = match f {
        sc::POOL_ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + 0xd0);
            at
        }
        sc::CAM_FRAME_CREATE => FRAME,
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

const FUNCS: [u32; 21] = [
    0x26a4b8, 0x26a5d0, 0x26a658, 0x26ab60, 0x26ac30, 0x26ac50, 0x26ac98, 0x26b2b0, 0x26b2f0, 0x26b3f8, 0x26b450,
    0x26b480, 0x26b4b8, 0x26b4e8, 0x26b760, 0x26b8a8, 0x26b9f0, 0x26bb38, 0x26bcb8, 0x26be38, 0x26a4b8,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    let buf = SCRATCH;
    Some(match f {
        0x26a4b8 => sc::camera_ctor(m, a[0]),
        0x26a5d0 => return { sc::camera_draw_world(m, e, a[0], a[1]); None },
        0x26a658 => return { sc::camera_render(m, e, a[0], buf); None },
        0x26ab60 => sc::scripted_init(m, e, a[0], buf),
        0x26ac30 => return { sc::scripted_scale_frame(m, a[0]); None },
        0x26ac50 => return { sc::scripted_free(m, e, a[0]); None },
        0x26ac98 => return { sc::scripted_render(m, e, a[0], buf); None },
        0x26b2b0 => return { sc::scripted_update(m, e, a[0]); None },
        0x26b2f0 => return { sc::scripted_set_shot(m, e, a[0], a[1], buf); None },
        0x26b3f8 => sc::combined_ctor(m, e, a[0], buf),
        0x26b450 => return { sc::combined_inner(m, e, a[0], 0x68); None },
        0x26b4b8 => return { sc::combined_inner(m, e, a[0], 0xe8); None },
        0x26b480 => return { sc::combined_render(m, e, a[0], a[1]); None },
        0x26b4e8 => return { sc::pan_to_rider(m, e, a[0], a[1]); None },
        0x26b760 => return { sc::rider_shot(m, e, a[0], a[1], 0x27, 8); None },
        0x26b8a8 => return { sc::rider_shot(m, e, a[0], a[1], 0x33, 9); None },
        0x26b9f0 => return { sc::rider_shot(m, e, a[0], a[1], 0x3f, 0xa); None },
        0x26bb38 => return { sc::custom_shot(m, e, a[0], a[1], 0x4b, 0x57); None },
        0x26bcb8 => return { sc::custom_shot(m, e, a[0], a[1], 0x63, 0x6f); None },
        0x26be38 => sc::set_mode(m, e, a[0], a[1]),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn cameras_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let mut stubs = vec![
        sc::PLAY_SHOT, sc::WORLD_DRAW_BEGIN, sc::WORLD_DRAW, sc::POOL_ALLOC, sc::FILL, sc::CAM_FRAME_CREATE,
        sc::CAM_SET_SHOT,
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
    let mut g = Rng(0x9e60_0000_0000_0001);
    let per = 500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
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
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(APPOBJ + 0x58, g.pick(&[4u32, 0, 2]));
        m.w32(FE + 0xf0, g.below(6));
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the camera: shown or not, its position, angles, lens and planes, frame, shot, mode, rider and flag)
        m.w32(C + 0xc, VT);
        m.w32(C + 0x14, g.below(2));
        for k in 0..4 {
            m.w32(C + 0x80 + 4 * k, g.f(-500.0, 500.0).to_bits());
        }
        for k in 0..3 {
            m.w32(C + 0x90 + 4 * k, g.f(-7.0, 7.0).to_bits());
        }
        m.w32(C + 0xa0, g.f(0.3, 1.5).to_bits());
        m.w32(C + 0xa4, g.f(1.0, 50.0).to_bits());
        let far = g.f(1.0, 80000.0);
        m.w32(C + 0xa8, g.pick(&[far, 20000.0, 0.5]).to_bits());
        m.w32(C + 0xb0, if func == 0x26ac50 && g.below(3) == 0 { 0 } else { FRAME });
        m.w32(C + 0xb4, g.below(12));
        m.w32(C + 0x180, g.below(13));
        m.w8(C + 0x184, g.pick(&[0u8, 3, 11, 12, 0xff, 0x80]));
        m.w8(C + 0x185, g.next() as u8);
        // (the combined camera's inner one)
        if [0x26b450u32, 0x26b480, 0x26b4b8].contains(&func) {
            m.w32(C + 0xb0, INNER);
        }
        m.w32(INNER + 0xc, VT);
        // (the frame: its table, its shot's position, angles in degrees and lens)
        m.w32(FRAME + 0xb4, VT);
        m.w32(FRAME + 0x10, g.f(-5.0, 5.0).to_bits());
        for k in 0..3 {
            m.w32(FRAME + 0x68 + 4 * k, g.f(-500.0, 500.0).to_bits());
            m.w32(FRAME + 0x74 + 4 * k, g.f(-400.0, 400.0).to_bits());
        }
        let lens = g.f(1.0, 200.0);
        m.w32(FRAME + 0x94, g.pick(&[0.0f32, -1.0, 35.0, lens]).to_bits());
        for k in 0..4 {
            m.w32(POS + 4 * k, g.f(-500.0, 500.0).to_bits());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x26a5d0 => [C, POS, 0, 0],
            0x26b2f0 => [C, g.below(16), 0, 0],
            0x26b4e8 | 0x26b760 | 0x26b8a8 | 0x26b9f0 => [C, g.pick(&[0u32, 1, 4, 11, 12, 0xff, 0x105]), 0, 0],
            0x26be38 => [C, g.below(13), 0, 0],
            _ => [C, g.below(4), 0, 0],
        };
        // (the next rider after the kept one, so the pan's other branch is seen)
        if func == 0x26b4e8 && g.below(2) == 0 {
            let next = (m.r8(C + 0x184) as i8 as i32 + 1) % 12;
            let k = next as u32;
            let a = [C, k, 0, 0];
            run_case(&mut r, &mut m, &ans, &log, case, func, a);
            continue;
        }
        run_case(&mut r, &mut m, &ans, &log, case, func, a);
        let _ = &mut g;
    }
    // (the case's seed for the stand-ins comes from the arena's random words)
    fn run_case(
        r: &mut Runner,
        m: &mut Mem,
        ans: &Answers,
        log: &Rc<RefCell<Vec<Call>>>,
        case: u32,
        func: u32,
        a: [u32; 4],
    ) {
        ans.seed.set(((m.r32(ARENA + 0x40) as u64) << 32) | m.r32(ARENA + 0x44) as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans };
        let mine = ours(m, &mut env, func, a);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if let Some(v) = mine {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case} {func:#x}: the answer");
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

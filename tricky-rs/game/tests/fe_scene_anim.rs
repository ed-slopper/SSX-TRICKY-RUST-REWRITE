//! `tricky_game::fe_scene`'s geometry, films and animations (board row G9z4) against the original in the function
//! runner (TRICKY_ELF; skipped without it): `FE3DGeometry_BindInstance` / `_GetPos`, the geometry's and the film
//! geometry's draws and the film geometry's destructor, the animations' rate, step, rotation and translation, and
//! the copies of the free and `Vec4_Set`. Every case builds a random geometry or animation (its instance or none,
//! film, flags, angles, offsets and times), matrix, renderer and pool in the arena; one function then runs on both
//! sides. The film's stop and the game's `Math_SinCos` run for real on both sides; every other callee is a stand-in
//! answering from the case's seed (the world's instance: none or the case's), and the renderer's virtuals are a
//! table of stand-ins. The calls are compared with their arguments (vectors on the stack read by value), the answers
//! and the memory after.

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
const POOL: (u32, u32) = (0x345e00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const APPOBJ: u32 = ARENA + 0x800;
const FE: u32 = ARENA + 0xa00;
const VT: u32 = ARENA + 0x1000;
const R: u32 = ARENA + 0x1400;
const G: u32 = ARENA + 0x1800;
const INST: u32 = ARENA + 0x1a00;
const TARGET: u32 = ARENA + 0x1c00;
const MAT: u32 = ARENA + 0x1e00;
const VEC: u32 = ARENA + 0x1f00;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 4] = [0x1d0, 0x1d8, 0x1e0, 0x2a8];
const SIN_COS: u32 = 0x250c10;
const FREE_COPY: u32 = 0x2695c8;

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
        sc::LIGHTING_SETUP => (8, 0),
        f if f == slot(0x2a8) => (7, 0),
        sc::DRAW_INSTANCE => (4, 0),
        sc::FIND_INSTANCE | sc::GEOMETRY_DTOR | sc::VIDEO_TEXTURE | sc::VIDEO_RELEASE | sc::POOL_FREE => (2, 0),
        f if f == slot(0x1d0) || f == slot(0x1e0) => (2, 0),
        _ => (1, 0),
    }
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

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (vectors on the stack by value)
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..4).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let answer = match f {
        sc::FIND_INSTANCE => if r % 4 == 0 { 0 } else { INST },
        f if f == slot(0x1d8) => r % 8,
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
    0x26957c, 0x2695c8, 0x269608, 0x269a70, 0x269ad0, 0x269b08, 0x269c28, 0x269d10, 0x269ea0, 0x269ed0, 0x269ee8,
    0x26a370, 0x26a430, 0x269c70, 0x269cc8,
];

fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], fl: [u32; 4]) {
    let buf = SCRATCH;
    match f {
        0x26957c => sc::nothing(),
        0x2695c8 => sc::free_block(m, e, a[0]),
        0x269608 => sc::vec4_set(m, a[0], fl),
        0x269a70 => sc::bind_instance(m, e, a[0], a[1]),
        0x269ad0 => sc::geometry_pos(m, a[0], a[1]),
        0x269b08 => sc::geometry_render(m, e, a[0], buf),
        0x269c28 => sc::movie_geometry_dtor(m, e, a[0], a[1]),
        0x269d10 => sc::movie_geometry_render(m, e, a[0], a[1], buf),
        0x269ea0 => sc::anim_set_rate(m, a[0]),
        0x269ed0 => sc::anim_step(m, a[0]),
        0x269ee8 => sc::anim_rotate_apply(m, a[0], a[1]),
        0x26a370 => sc::anim_rotate_update(m, a[0]),
        0x26a430 => sc::anim_translate_apply(m, a[0], a[1]),
        0x269c70 => sc::open_course_video(m, e, a[0], a[1]),
        0x269cc8 => sc::movie_stop(m, e, a[0]),
        _ => unreachable!("{f:#x}"),
    }
}

#[test]
fn geometry_and_animations_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    r.allow(SIN_COS);
    let _ = FREE_COPY;
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    let mut stubs = vec![
        sc::POOL_FREE, sc::HEAP_FREE, sc::NAME_HASH, sc::FIND_INSTANCE, sc::MATRIX_MAKE, sc::DRAW_INSTANCE,
        sc::GEOMETRY_DTOR, sc::VIDEO_TEXTURE, sc::LIGHTING_SETUP, sc::VIDEO_OPEN, sc::VIDEO_RELEASE,
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
    let mut g = Rng(0x9e5f_0000_0000_0001);
    let per = 600u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (POOL.0, vec![0; POOL.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        m.w32(sc::POOL, 0x0130_0000);
        m.w32(sc::POOL + 4, 0x1000);
        let anim = [0x269ea0u32, 0x269ed0, 0x269ee8, 0x26a370, 0x26a430].contains(&func);
        if anim {
            // (the animation: driving another or not, its angles and ends, time, seconds and rate)
            m.w32(G + 0xac, if g.below(3) == 0 { TARGET } else { 0 });
            for k in 0..6 {
                m.w32(G + 0xb0 + 4 * k, g.f(-7.0, 7.0).to_bits());
            }
            m.w32(G + 0xa0, g.f(0.0, 1.0).to_bits());
            m.w32(G + 0xa4, g.f(0.1, 5.0).to_bits());
            m.w32(G + 0xa8, g.f(0.0, 0.05).to_bits());
        } else {
            // (the geometry: its instance or none, shown or not, its lift, its film or none)
            m.w32(G + 0xa0, if g.below(4) == 0 { 0 } else { INST });
            m.w32(G + 0x14, g.below(2));
            m.w32(G + 0x70, g.below(4));
            m.w32(G + 0xb0, g.pick(&[0u32, 3, 0xffff_ffff]));
        }
        for k in 0..16 {
            m.w32(MAT + 4 * k, g.f(-2.0, 2.0).to_bits());
            m.w32(INST + 4 * k, g.f(-100.0, 100.0).to_bits());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let block = g.pick(&[0x0130_0000u32, 0x0130_0010, 0x0130_1000, 0x012f_fff0]);
        let a = match func {
            0x2695c8 => [block, 0, 0, 0],
            0x269608 => [VEC, 0, 0, 0],
            0x269ad0 => [G, VEC, 0, 0],
            0x269ee8 | 0x26a430 => [G, MAT, 0, 0],
            _ => [G, g.next() & 0xff_ffff, 0, 0],
        };
        let fl = [g.next(), g.next(), g.next(), g.next()];
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        ours(&mut m, &mut env, func, a, fl);
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        for (k, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + k, f32::from_bits(*v));
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
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

//! `tricky_game::fe_scene`'s characters (board row G9z2) against the original in the function runner (TRICKY_ELF;
//! skipped without it): `cFE3DCharacter`'s set-up, update, loads, poses and lights. Every case builds a random
//! character (its rider, outfit and board, poses, load, changes, light and angles), rider model and front end in the
//! arena; one function then runs on both sides. The character's own functions and the game's sine and cosine run for
//! real on both sides; every other callee is a stand-in answering from the case's seed: the heap (handed out in
//! order), the model's constructor (the case's model), the loader (whose options write a word), the career, the
//! animation and the screen's mode, and the model's table (a table of stand-ins). The calls are compared with their
//! arguments (a request on the stack read by value), the answers and the memory after.

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
/// the game's words the characters read: the riders and their boards
const GAME: [(u32, u32); 1] = [(0x331e00, 0x1000)];
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const APPOBJ: u32 = ARENA + 0x800;
const FE: u32 = ARENA + 0x1000;
const VT: u32 = ARENA + 0x1800;
const C: u32 = ARENA + 0x2000;
const INFO: u32 = ARENA + 0x2400;
const LIGHTS: u32 = ARENA + 0x2800;
const MODEL: (u32, u32) = (0x0120_0000, 0x4800);
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SIN: u32 = 0x250d60;
const COS: u32 = 0x250e98;

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
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        sc::LOAD_START | sc::ANIM_PLAY => (4, 0),
        sc::ALLOC | sc::FILL | sc::MODEL_SET_RIDER | sc::LOAD_REQUEST | sc::KEEP_OUTFIT | sc::KEEP_BOARD => (3, 0),
        f if sc::RIDER_PARTS.contains(&f) => (3, 0),
        sc::SCENE_BASE_DTOR | sc::THING_DTOR | sc::MODEL_SET_PARTS | sc::MODEL_FREE_RIDER | sc::LOAD_RUN
        | sc::LOAD_DONE | sc::LOAD_FINISH | sc::LOAD_RELEASE | sc::ANIM_PLAYING => (2, 0),
        sc::ANIM_SPEED => (1, 1),
        f if f == slot(8) => (2, 0),
        _ => (1, 0),
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
    // (a request or options on the stack by value)
    if stack(w[0]) {
        log.extend((0..2).map(|k| token(io.r32(w[0] + 4 * k))));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    if f == sc::LOAD_OPTIONS || f == sc::LOAD_OPTIONS_ASYNC {
        io.w32(w[0], g.next());
    }
    let answer = match f {
        sc::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        sc::MODEL_CTOR => MODEL.0,
        sc::LOAD_DONE | sc::ANIM_PLAYING => r % 2,
        sc::SCREEN_MODE => g.pick(&[0u32, 1, 2, 3]),
        sc::LOAD_START => g.pick(&[0u32, 4, 0xffff_ffff]),
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

const FUNCS: [u32; 17] = [
    0x265da0, 0x265dc0, 0x265ed8, 0x266110, 0x266190, 0x2661e8, 0x2661f0, 0x266208, 0x266320, 0x266390, 0x2663e8,
    0x266df8, 0x2670a0, 0x2670e8, 0x2673f8, 0x267508, 0x267670,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], f12: u32) -> Option<u32> {
    let buf = SCRATCH;
    match f {
        0x265da0 => sc::scene_delete(m, e, a[0], a[1]),
        0x265dc0 => sc::scene_dtor(m, e, a[0], a[1]),
        0x265ed8 => return Some(sc::character_init(m, e, a[0], a[1], buf)),
        0x266110 => sc::character_dtor(m, e, a[0], a[1]),
        0x266190 => sc::save_selection(m, e, a[0], a[1]),
        0x2661e8 => sc::set_pose(m, a[0], a[1]),
        0x2661f0 => sc::stop_pose(m, e, a[0]),
        0x266208 => sc::fill_rider_info(m, e, a[0]),
        0x266320 => sc::load_rider_async(m, e, a[0], buf),
        0x266390 => sc::free_rider(m, e, a[0]),
        0x2663e8 => return Some(sc::has_model(m, a[0])),
        0x266df8 => sc::character_update(m, e, a[0], buf),
        0x2670a0 => sc::load_rider(m, e, a[0], buf),
        0x2670e8 => sc::update_lighting(m, a[0], buf),
        0x2673f8 => sc::fill_lights_a(m, a[1]),
        0x267508 => sc::fill_lights_b(m, a[1]),
        0x267670 => sc::lighting_lerp(m, a[1], a[2], a[3], f12),
        _ => unreachable!("{f:#x}"),
    }
    None
}

#[test]
fn characters_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    r.allow(SIN);
    r.allow(COS);
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
        sc::FREE, sc::SCENE_BASE_DTOR, sc::THING_DTOR, sc::ALLOC, sc::FILL, sc::MODEL_CTOR, sc::MODEL_SET_PARTS,
        sc::MODEL_SET_RIDER, sc::MODEL_FREE_RIDER, sc::LOAD_REQUEST, sc::LOAD_RUN, sc::LOAD_OPTIONS,
        sc::LOAD_OPTIONS_ASYNC, sc::LOAD_START, sc::LOAD_DONE, sc::LOAD_FINISH, sc::LOAD_RELEASE, sc::KEEP_OUTFIT,
        sc::KEEP_BOARD, sc::ANIM_PLAYING, sc::ANIM_PLAY, sc::ANIM_SPEED, sc::ANIM_STEP, sc::ANIM_STOP, sc::SCREEN_MODE,
        slot(8),
    ];
    stubs.extend(sc::RIDER_PARTS);
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
    let mut g = Rng(0x9e5d_0000_0000_0001);
    let per = 500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
        regions.extend(game.iter().cloned());
        regions.extend([
            (ARENA, vec![0; SIZE as usize]),
            (MODEL.0, vec![0; MODEL.1 as usize]),
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
        m.w32(APPOBJ + 0x734, FE);
        m.w16(VT + 8, g.pick(&[0u16, 4]));
        m.w32(VT + 0xc, slot(8));
        // (the character: its model or none, its record, rider, outfit and board, poses, load, changes, light)
        // (only the destructor sees a character without its model or record)
        let dtor = func == 0x266110;
        m.w32(C + 0xac, if dtor && g.below(3) == 0 { 0 } else { MODEL.0 });
        m.w32(C + 0xa8, if dtor && g.below(3) == 0 { 0 } else { INFO });
        m.w32(C + 0xb0, g.below(12));
        m.w8(C + 0x158, g.below(12) as u8);
        m.w8(C + 0x159, g.next() as u8);
        m.w8(C + 0xa0, g.pick(&[0u8, 1, 2, 3, 0xff]));
        m.w8(C + 0xa1, g.pick(&[0u8, 1, 2, 3, 0xff]));
        m.w32(C + 0xb4, g.pick(&[0u32, 7, 0xffff_ffff]));
        m.w32(C + 0x15c, g.pick(&[0u32, 0, 1, 2]));
        m.w32(C + 0x160, g.pick(&[0u32, 0, 1, 3]));
        m.w32(C + 0x150, g.pick(&[0.0f32, 1.0, 0.5, 0.95, 0.03, 1.5, -0.2, 0.0667]).to_bits());
        m.w32(C + 0x154, g.below(2));
        for k in 0..6 {
            m.w32(C + 0xd0 + 4 * k, g.f(-400.0, 400.0).to_bits());
        }
        for k in 0..MODEL.1 / 4 {
            m.w32(MODEL.0 + 4 * k, g.next());
        }
        m.w32(MODEL.0, VT);
        m.w32(MODEL.0 + 0x46b0, g.below(2));
        for k in 0..0x60u32 {
            m.w32(LIGHTS + 4 * k, g.f(-10.0, 400.0).to_bits());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = [C, g.pick(&[0u32, 1, 2, 3, 12]), LIGHTS + 0x60, LIGHTS + 0xc0];
        let a = if func == 0x2673f8 || func == 0x267508 || func == 0x267670 { [C, LIGHTS, a[2], a[3]] } else { a };
        let any = g.f(0.0, 1.0);
        let f12 = g.pick(&[0.0f32, 1.0, 0.25, 0.7, any]).to_bits();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a, f12);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f32::from_bits(f12));
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

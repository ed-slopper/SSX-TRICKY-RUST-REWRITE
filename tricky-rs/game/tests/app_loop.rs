//! `tricky_game::app`'s life and loop (board row E1e) against the original in the function runner (TRICKY_ELF;
//! skipped without it): `cApplication`'s start, next module, shutdown and destructors, `App_Run`, `App_MainLoop`
//! and the vsync mode (`App_StopVSyncMode` / `_StartVSyncMode` / `_VSyncTick` / `_RunVSyncModeTicks`), and
//! `cPS2Device` vf6 / vf7. Every case builds a random application (its modules, vsync stage and fraction, input,
//! execution manager, stage, career choices) in the arena; one function then runs on both sides. The application's
//! own functions and the game's constructor run for real on both sides; every other callee is a stand-in answering
//! from the case's seed. Three of them stand in for what happens around the loop: the display's wait sets the
//! quit flag now and then, the execution manager's wait moves the vsync stage on, and the input's pending frames
//! run out. The modules', execution manager's and application's virtuals are tables of stand-ins. The calls are
//! compared with their arguments, the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
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
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const WORLDS: (u32, u32) = (0x346400, 0x100);
const KEPT: (u32, u32) = (0x33ef00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const APP: u32 = ARENA + 0x100;
const VT: u32 = ARENA + 0x1000;
const VT_APP: u32 = ARENA + 0x1200;
const R: u32 = ARENA + 0x1400;
const EX: u32 = ARENA + 0x1500;
const MOD1: u32 = ARENA + 0x1600;
const MOD2: u32 = ARENA + 0x1700;
const KOBJ: u32 = ARENA + 0x1800;
const DEV: u32 = ARENA + 0x1900;
const OUT: u32 = ARENA + 0x1a00;
const HEAP: (u32, u32) = (0x0110_0000, 0x8000);
const SCRATCH: u32 = 0x01f8_0000;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot_app(off: u32) -> u32 {
    0x0182_0000 + off
}
const SLOTS: [u32; 7] = [8, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38];
const SLOTS_APP: [u32; 3] = [0x10, 0x18, 0x20];
const REAL: [u32; 7] = [0x17bdf0, 0x17c0a8, 0x17c118, 0x186218, 0x186348, 0x17e0f8, 0x17c488];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        ap::ALLOC | ap::FILL | ap::START_SCREEN | ap::VIDEO_MODULE_CTOR | ap::CAREER_COURSE => (3, 0),
        ap::FONT_DTOR | ap::BLOCK_FREE | ap::RECORDS_DTOR | ap::BIG_DTOR | ap::LOC_FOR_REGION | ap::FRONT_END_CTOR => (2, 0),
        ap::GS_INIT | ap::RENDERER_NEW | 0x22be18 => (0, 0),
        f if f == slot_app(0x10) => (3, 0),
        f if f == slot_app(0x18) || f == slot_app(0x20) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (2, 0),
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

fn standin(io: &mut dyn Io, f: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, _) = arity(f);
    let log: Vec<u32> = w[..ni].to_vec();
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    // (the world around the loop: the quit now and then, the vsync stage moved on)
    if f == ap::VSYNC_WAIT && g.below(6) == 0 {
        io.w32(APP + 0x20, g.pick(&[1u32, 2]));
    }
    if f == ap::VSYNC_WAIT && g.below(4) == 0 {
        io.w32(APP + 4, MOD2);
    }
    if f == slot(0x20) && w[0] & !0xf == EX && g.below(2) == 0 {
        io.w32(APP + 0xc, g.pick(&[0u32, 2]));
    }
    let answer = match f {
        ap::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        ap::RENDERER_NEW => R,
        ap::CAREER_COURSE => 0x32c000 + 4 * g.below(16),
        ap::INPUT_PENDING => g.pick(&[0u32, 1, 1]),
        ap::FILE_SIZE => g.pick(&[0x40u32, 0x100, 0]),
        f if f == slot_app(0x20) => g.pick(&[MOD1, MOD2]),
        f if f == slot(0x18) || f == slot(0x38) || f == slot(0x20) => g.pick(&[0u32, 0, 1]),
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
        let (ni, _) = arity(f);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let (a, c) = standin(m, f, &w, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 200_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 15] = [
    0x17b668, 0x17b6b0, 0x17ba50, 0x17bbd8, 0x17bf48, 0x17c418, 0x17c488, 0x17c908, 0x17c998, 0x17cb40, 0x17c9d8,
    0x17c6e8, 0x17c570, 0x17bbd8, 0x17c9d8,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    Some(match f {
        0x17b668 => return { ap::device_get(m, a[0], a[1]); None },
        0x17b6b0 => return { ap::device_set(m, a[0], a[1]); None },
        0x17ba50 => return { ap::app_start(m, e, a[0]); None },
        0x17bbd8 => ap::next_module(m, e, a[0]),
        0x17bf48 => return { ap::app_shutdown(m, e, a[0]); None },
        0x17c418 => return { ap::app_dtor(m, e, a[0], a[1]); None },
        0x17c488 => return { ap::appman_dtor(m, e, a[0], a[1]); None },
        0x17c908 => return { ap::stop_vsync_mode(m, e, a[0]); None },
        0x17c998 => return { ap::start_vsync_mode(m, e, a[0]); None },
        0x17cb40 => return { ap::run_vsync_mode_ticks(m, e, a[0]); None },
        0x17c9d8 => return { ap::vsync_tick(m, e, a[0]); None },
        0x17c6e8 => return { ap::main_loop(m, e, a[0]); None },
        0x17c570 => ap::app_run(m, e, a[0], a[1], a[2]),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn app_life_and_loop_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), heap: Cell::new(HEAP.0) });
    let mut stubs = vec![
        ap::ALLOC, ap::FILL, ap::HEAP_FREE, ap::FREE, ap::FILE_SIZE, ap::PRINT, ap::WORLD_NEW, ap::WORLD_INIT,
        ap::RENDERER_NEW, ap::FADES_CTOR, ap::GS_INIT, ap::VSYNC_WAIT, ap::LOC_FOR_REGION, ap::START_SCREEN,
        ap::OPEN_BIG_FILES, ap::FRONT_END_CTOR, ap::VIDEO_MODULE_CTOR, ap::CAREER_COURSE, ap::CAREER_PREPARE_RACE,
        ap::FONT_DTOR, ap::BLOCK_FREE, ap::RECORDS_DTOR, ap::BIG_DTOR, ap::INPUT_CTOR, ap::INPUT_DTOR,
        ap::INPUT_FRAME, ap::INPUT_PENDING, ap::INPUT_STEP,
    ];
    stubs.extend(ap::CAREER_STARTS);
    stubs.extend(ap::LESSON_RESET);
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS_APP.map(slot_app));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (v, call) = standin(mem, f, &w, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 200_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e63_0000_0000_0001);
    let per = 400u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![
            (DATA.0, data.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (WORLDS.0, vec![0; WORLDS.1 as usize]),
            (KEPT.0, vec![0; KEPT.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..CAREER.1 / 4 {
            m.w32(CAREER.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APP);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for off in SLOTS_APP {
            m.w16(VT_APP + off, g.pick(&[0u16, 4]));
            m.w32(VT_APP + off + 4, slot_app(off));
        }
        // (the application: its modules, vsync stage, rate and fraction, input, manager, renderer, stage)
        for o in [R, EX, MOD1, MOD2, KOBJ] {
            m.w32(o, VT);
        }
        m.w32(R + 0x10, VT);
        m.w32(APP, MOD1);
        // (the vsync mode's own functions always have a module pending, as the loop gives them)
        let needs = [0x17c998u32, 0x17cb40, 0x17c9d8].contains(&func);
        m.w32(APP + 4, if g.below(3) == 0 && !needs { 0 } else { MOD2 });
        m.w32(APP + 0xc, g.pick(&[0u32, 1, 2, 3, 3, 0xffff_ffff]));
        m.w32(APP + 0x14, g.pick(&[1u32, 2, 3]));
        m.w32(APP + 0x18, g.pick(&[0.5f32, 1.0, 1.25, 2.0, 3.5]).to_bits());
        m.w32(APP + 0x1c, g.f(0.0, 1.0).to_bits());
        m.w32(APP + 0x20, g.pick(&[0u32, 0, 0, 1]));
        m.w32(APP + 0x24, if g.below(4) == 0 && func == 0x17c9d8 { 0 } else { ARENA + 0x40 });
        m.w32(APP + 0x28, EX);
        m.w32(APP + 0x2c, VT_APP);
        for at in [0x44u32, 0x48, 0x4c, 0x5c, 0x60, 0x728] {
            m.w32(APP + at, if g.below(3) == 0 { 0 } else { g.next() });
        }
        m.w32(APP + 0x64, g.below(4));
        m.w32(APP + 0x724, R);
        m.w32(APP + 0x73c, g.pick(&[0u32, 1, 2, 2, 3, 4]));
        m.w32(0x32b390 + 0x3260, g.pick(&[0x20u32, 0x21, 0x1, 0x8, 0x40, 0, 0x4]));
        m.w32(0x32b390 + 0x3818, g.pick(&[7u32, 1, 2]));
        m.w32(ap::KEPT_OBJECT, if g.below(2) == 0 { 0 } else { KOBJ });
        m.w32(ap::PAUSE_DRAW, g.below(2));
        for k in 0..8 {
            m.w32(DEV + 0xc + 4 * k, g.next());
            m.w32(OUT + 4 * k, g.next());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x17b668 | 0x17b6b0 => [DEV, OUT, 0, 0],
            0x17c418 | 0x17c488 => [APP, g.below(4), 0, 0],
            0x17c570 => [APP, g.below(4), ARENA + 0x80, 0],
            _ => [APP, 0, 0, 0],
        };
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        ans.heap.set(HEAP.0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a);
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

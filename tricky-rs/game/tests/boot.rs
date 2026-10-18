//! `tricky_game::boot` (board row E1g) against the original in the function runner (TRICKY_ELF; skipped without
//! it): the boot films' modules, the PS2 execution manager (its start and stop, the frame wait and signal, the vsync
//! thread and handler), the stack canary and its report, the IOP reboot and `PS2ExecutionMan_Create`. Every case
//! builds a random module, player, manager, career, renderer and stack record in the arena and the game's data; one
//! function then runs on both sides. The module's destructor and the execution manager's run for real on both sides;
//! every other callee is a stand-in answering from the case's seed: the kernel's and the SDK's calls (the loops'
//! answers, a thread's status and the clock written into their buffers, `memset` done), the allocators (`malloc`
//! near a small heap whose end the stand-in picks) and the objects' virtuals. The vsync thread never returns: its
//! fifth wait stops it on both sides. The calls are compared with their arguments (buffers on the stack read by
//! value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use r5900::{Runner, RETURN_SENTINEL};
use tricky_game::boot as bt;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_main as gm;
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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const SYS: (u32, u32) = (0x343000, 0x100);
const MEMSYS: (u32, u32) = (0x349d00, 0x100);
const RECORD: (u32, u32) = (0x3cdd00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x10000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x900;
const O: u32 = ARENA + 0x1000;
const PLAYER: u32 = ARENA + 0x2000;
/// the renderer's table and the player's
const VT: u32 = ARENA + 0x3000;
const VT2: u32 = ARENA + 0x3200;
const EX: u32 = ARENA + 0x4000;
/// the canary's ground: the stack's words (a run of canaries in random words) for the report
const CAN: u32 = ARENA + 0x9000;
const HEAP: (u32, u32) = (0x0110_0000, 0x10000);
/// where `malloc` answers, and the heap after it
const LOW: (u32, u32) = (0x0120_0000, 0x2000);
/// the stack under the runner's stack pointer, for the canary laid under it
const STACK: (u32, u32) = (0x01fe_f000, 0x1000);
/// the canary's stack pointer inside it: the runner's top, less its frame
const CANARY_SP: u32 = 0x01ff_0000 - 0x10;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
fn slot2(off: u32) -> u32 {
    0x0182_0000 + off
}
const SLOTS: [u32; 2] = [0x70, 0x80];
const SLOTS2: [u32; 5] = [8, 0x10, 0x18, 0x20, 0x28];
/// the game's own functions here, run for real on both sides
const REAL: [u32; 1] = [0x181eb0];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write(a, &[v]).unwrap();
    }
}
fn stack(a: u32) -> bool {
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32, w: &[u32]) -> (usize, usize) {
    match f {
        bt::GET_THREAD_ID | bt::END_OF_HEAP | bt::IOP_SYNC | bt::SIF_INIT | bt::MC_INIT | bt::PLAYER_NEW => (0, 0),
        bt::PRINT => (
            match w[0] {
                x if x == bt::MESSAGES[3] => 7,
                x if x == bt::MESSAGES[4] => 3,
                x if bt::STACK_MESSAGES.contains(&x) => 2,
                _ => 1,
            },
            0,
        ),
        bt::ADD_INTC => (4, 0),
        bt::MEMSET | bt::LOAD_MODULE | bt::FILL_CANARY | bt::FILES_INIT | gm::ALLOC | gm::FILL => (3, 0),
        bt::REFER_THREAD | bt::START_THREAD | bt::REMOVE_INTC | bt::STRCPY | bt::MAIN_HEAP | bt::ASYNC_INIT
        | bt::LOAD_MODULE_DTOR | gm::APP_SET_MODE => (2, 0),
        f if f == gm::FRONT_END_MODULE.0 => (2, 0),
        gm::SOUND_TICK => (2, 1),
        f if f == slot2(8) || f == slot2(0x20) => (2, 0),
        f if f == slot2(0x10) => (3, 0),
        _ => (1, 0),
    }
}

/// How many words of a stack buffer a call is logged with
fn words(f: u32) -> u32 {
    match f {
        bt::CREATE_SEMA => 6,
        bt::CREATE_THREAD => 9,
        _ => 4,
    }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    heap: Cell<u32>,
    low: Cell<u32>,
    waits: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
    fn reset(&self) {
        self.n.set(0);
        self.heap.set(HEAP.0);
        self.low.set(0);
        self.waits.set(0);
    }
}

/// The waits the vsync thread gets before it is stopped
const WAITS: u32 = 5;

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, w);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (the buffers a call fills are not read before it)
    let filled = match f {
        bt::MEMSET | bt::READ_CLOCK => Some(0),
        bt::REFER_THREAD => Some(1),
        _ => None,
    };
    for (k, &a) in w[..ni].iter().enumerate() {
        if stack(a) && filled != Some(k) {
            log.extend((0..words(f)).map(|j| token(io.r32(a + 4 * j))));
        }
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    match f {
        bt::MEMSET => {
            for j in 0..w[2] {
                io.w8(w[0] + j, w[1] as u8);
            }
        }
        bt::REFER_THREAD => {
            for j in 0..0xc {
                io.w32(w[1] + 4 * j, g.next());
            }
        }
        bt::READ_CLOCK => {
            for j in 0..2 {
                io.w32(w[0] + 4 * j, g.next());
            }
        }
        bt::WAIT_SEMA => ans.waits.set(ans.waits.get() + 1),
        _ => {}
    }
    let answer = match f {
        gm::ALLOC => {
            let at = ans.heap.get();
            ans.heap.set(at + ((w[1] + 0xf) & !0xf));
            assert!(ans.heap.get() <= HEAP.0 + HEAP.1, "the heap ran out");
            at
        }
        bt::MALLOC => {
            let p = LOW.0 + g.pick(&[0u32, 4, 0x40, 0x7c]);
            ans.low.set(p);
            p
        }
        bt::END_OF_HEAP => {
            let base = (ans.low.get() + 0x3f & !0x3f) + 0x800;
            base.wrapping_add(g.pick(&[0u32, 0x40, 0x104, 0x400]))
        }
        bt::IOP_REBOOT | bt::IOP_SYNC => g.pick(&[0u32, 1, 1]),
        bt::LOAD_MODULE => g.pick(&[u32::MAX, 0, 0, 5]),
        bt::POLL_SEMA => g.pick(&[w[0], w[0], w[0] ^ 1]),
        gm::CAREER_TWO_PLAYERS => g.pick(&[0u32, 1, 1, 2]),
        f if f == slot(0x70) => g.pick(&[1u32, 1, 0]),
        f if f == slot2(0x28) => g.pick(&[0u32, 1]),
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
        let mut w0: Vec<u32> = args.to_vec();
        w0.resize(8, 0);
        let (ni, nf) = arity(f, &w0);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(8, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        if f == bt::WAIT_SEMA && self.ans.waits.get() >= WAITS {
            panic::resume_unwind(Box::new("the vsync thread stopped"));
        }
        a
    }
}

const FUNCS: [u32; 28] = [
    0x1819b0, 0x1819b8, 0x1819c0, 0x1819c8, 0x1819d0, 0x1819d8, 0x1819e0, 0x1864a0, 0x1864f8, 0x186558, 0x186580,
    0x1865d8, 0x186620, 0x186970, 0x186aa8, 0x186a00, 0x182418, 0x182c60, 0x182430, 0x182578, 0x1825d0, 0x1825e8,
    0x182618, 0x182660, 0x182680, 0x182728, 0x182858, 0x182ba8,
];

/// One function on our side; its answer where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4]) -> Option<u32> {
    let buf = SCRATCH;
    Some(match f {
        0x1819b0 | 0x1819b8 | 0x1819c0 | 0x1819d8 => return { bt::nothing(); None },
        0x1819c8 | 0x1819d0 => bt::one(),
        0x1819e0 => return { bt::video_load_dtor(m, e, a[0], a[1]); None },
        0x1864a0 => bt::video_ctor(m, e, a[0], a[1], a[2]),
        0x1864f8 => return { bt::video_dtor(m, e, a[0], a[1]); None },
        0x186558 => return { bt::video_enter(m, e, a[0]); None },
        0x186580 => bt::video_leave(m, e, a[0]),
        0x1865d8 => return { bt::video_start(m, e, a[0]); None },
        0x186620 => return { bt::video_frame(m, e, a[0]); None },
        0x186970 => bt::video_draw(m, e, a[0]),
        0x186aa8 => bt::video_playing(m, a[0]),
        0x186a00 => return { bt::video_static_init(m); None },
        0x182418 => return { bt::ps2_exec_dtor(m, e, a[0], a[1]); None },
        0x182c60 => return { bt::exec_dtor(m, e, a[0], a[1]); None },
        0x182430 => return { bt::ps2_exec_start(m, e, a[0], buf); None },
        0x182578 => return { bt::ps2_exec_stop(m, e, a[0]); None },
        0x1825d0 => bt::vsync_handler(m, e),
        0x1825e8 => {
            let hook = panic::take_hook();
            panic::set_hook(Box::new(|_| {}));
            let r = panic::catch_unwind(AssertUnwindSafe(|| bt::vsync_thread(m, e, a[0])));
            panic::set_hook(hook);
            assert!(r.is_err());
            return None;
        }
        0x182618 => return { bt::ps2_exec_wait(m, e, a[0]); None },
        0x182660 => return { bt::ps2_exec_signal(m, e, a[0]); None },
        0x182680 => return { bt::fill_canary(m, a[0], a[1], a[2], CANARY_SP); None },
        0x182728 => return { bt::print_stack_usage(m, e); None },
        0x182858 => return { bt::iop_reboot(m, e, buf); None },
        0x182ba8 => bt::ps2_exec_create(m, e, buf),
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn boot_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(REAL.iter()) {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers {
        seed: Cell::new(0),
        n: Cell::new(0),
        heap: Cell::new(HEAP.0),
        low: Cell::new(0),
        waits: Cell::new(0),
    });
    let mut stubs = vec![
        bt::GET_THREAD_ID, bt::REFER_THREAD, bt::CREATE_SEMA, bt::DELETE_SEMA, bt::WAIT_SEMA, bt::POLL_SEMA,
        bt::SIGNAL_SEMA, bt::I_SIGNAL_SEMA, bt::CREATE_THREAD, bt::START_THREAD, bt::TERMINATE_THREAD,
        bt::DELETE_THREAD, bt::DISABLE_INTC, bt::ENABLE_INTC, bt::ADD_INTC, bt::REMOVE_INTC, bt::END_OF_HEAP,
        bt::MEMSET, bt::STRCPY, bt::MALLOC, bt::LIBC_FREE, bt::PRINT, bt::SDK_INIT, bt::CD_INIT, bt::CD_MODE,
        bt::IOP_REBOOT, bt::IOP_SYNC, bt::SIF_INIT, bt::LOAD_MODULE, bt::MC_INIT, bt::PAD_INIT, bt::READ_CLOCK,
        bt::SEED_FX, bt::MAIN_THREAD, bt::MAIN_HEAP, bt::TIMER_INIT, bt::FILES_INIT, bt::ASYNC_INIT, bt::ROOT_PATH,
        bt::PLAYER_NEW, bt::VSYNC_TICK, bt::FILL_CANARY, bt::LOAD_MODULE_DTOR, gm::ALLOC, gm::FILL, gm::FREE,
        gm::SOUND_TICK, gm::APP_SET_MODE, gm::CAREER_TWO_PLAYERS, gm::FRONT_END_MODULE.0,
    ];
    stubs.extend(bt::RESULTS.map(|x| x.0));
    stubs.extend(SLOTS.map(slot));
    stubs.extend(SLOTS2.map(slot2));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
            if f == bt::WAIT_SEMA && a.waits.get() >= WAITS {
                c.set_gpr(31, RETURN_SENTINEL as i32 as i64 as u64);
            }
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0xb007_0000_0000_0001);
    let per = 1200u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![
            (DATA.0, data.clone()),
            (CAREER.0, vec![0; CAREER.1 as usize]),
            (SYS.0, vec![0; SYS.1 as usize]),
            (MEMSYS.0, vec![0; MEMSYS.1 as usize]),
            (RECORD.0, vec![0; RECORD.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (HEAP.0, vec![0xa5; HEAP.1 as usize]),
            (LOW.0, vec![0; LOW.1 as usize]),
            (SCRATCH, vec![0; 0x100]),
        ];
        if func == 0x182680 {
            regions.push((STACK.0, vec![0; STACK.1 as usize]));
        }
        let mut m = Mem { regions };
        for (at, n) in [(ARENA, SIZE), (CAREER.0, CAREER.1), (RECORD.0, RECORD.1), (MEMSYS.0, MEMSYS.1), (LOW.0, LOW.1)] {
            for k in 0..n / 4 {
                m.w32(at + 4 * k, g.next());
            }
        }
        for off in [0x2afcu32, 0x3704, 0x370c, 0x3710] {
            m.w32(gm::CAREER + off, g.pick(&[0u32, 1, 1, 6, 2]));
        }
        m.w32(gm::CAREER + 0x2af8, g.pick(&[0u32, 10, 10, 3]));
        m.w32(gm::CAREER + 0x3260, g.pick(&[0u32, 0, 1, 0x20, 0x3b, 0x3b, 0x40]));
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x28, EX);
        m.w32(APPOBJ + 0x724, R);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        for off in SLOTS2 {
            m.w16(VT2 + off, g.pick(&[0u16, 4]));
            m.w32(VT2 + off + 4, slot2(off));
        }
        m.w32(PLAYER, VT2);
        m.w32(O + 8, if g.below(4) == 0 && func != 0x1865d8 && func != 0x186620 && func != 0x186970 { 0 } else { PLAYER });
        m.w32(O + 0x110, g.pick(&[0u32, 0, 1, 2]));
        m.w32(EX + 0x10, g.below(2));
        m.w32(gm::SOUND, ARENA + 0x40);
        // (the canary's record and ground: a top, a bottom, an end, the stack's page, runs of canaries)
        let top = CAN + 0x400 + 4 * g.below(0x40);
        let bottom = CAN + 4 * g.below(0x80);
        let end = CAN + 0x800 + 4 * g.below(0x40);
        for k in 0..0x400 {
            if g.below(8) != 0 {
                m.w32(CAN + 4 * k, bt::CANARY);
            }
        }
        // (sometimes the canary all the way down to the bottom)
        if g.below(4) == 0 {
            for a in (bottom..top).step_by(4) {
                m.w32(a, bt::CANARY);
            }
        }
        m.w32(CAN + 0xffc, 0);
        m.w32(bt::CANARY_TOP, if g.below(8) == 0 { 0 } else { top });
        m.w32(bt::CANARY_TOP + 4, bottom);
        m.w32(bt::CANARY_TOP + 8, end);
        m.w32(bt::CANARY_TOP + 0xc, CAN + 0x1000 + 4 * g.below(0x100));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x1819e0 | 0x1864f8 | 0x182418 | 0x182c60 => [if func >= 0x186000 || func < 0x182000 { O } else { EX }, g.below(4), 0, 0],
            0x1864a0 => [O, ARENA + 0x80, g.next(), 0],
            0x182430 | 0x182578 | 0x1825e8 | 0x182618 | 0x182660 => [EX, 0, 0, 0],
            0x182680 => {
                // (the stack's end around 1 KiB under the stack pointer, so some is laid)
                let lim = CANARY_SP - 0x400;
                [CAN + 4 * g.below(0x100), CAN + 4 * g.below(0x100), lim - 4 * g.below(0x40) + g.pick(&[0u32, 0, 0x40]), 0]
            }
            _ => [O, 0, 0, 0],
        };
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.reset();
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a);
        ans.reset();
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

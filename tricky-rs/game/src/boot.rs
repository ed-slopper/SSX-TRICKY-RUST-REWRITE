//! The boot on the game's bytes (board row E1g): the boot films' modules (`cVideoModule`, `cVideoLoadModule`) and
//! the PS2 execution manager (`cPS2ExecutionMan`: the vsync thread, its semaphores and interrupt handler), the stack
//! canary and its report, the IOP reboot with the modules it loads, and `PS2ExecutionMan_Create`. What talks to the
//! kernel, the IOP or the SDK stays a call out through [`FeEnv`]. Checked in the function runner (`tests/boot.rs`).

use crate::fe3dstate::{FeEnv, GAPP};
use crate::game_main as gm;
use crate::pf::Mem;

/// The kernel's calls (thread id 0x3066d0, thread status 0x3066e0, semaphores: make 0x3067e0, delete 0x3067f0, wait
/// 0x306820, poll 0x306830, signal 0x306800 and from the handler 0x306810; threads: make 0x3065e0, start 0x306600,
/// end 0x306630, delete 0x3065f0; the interrupt controller: off 0x307208, on 0x307270, handler added 0x3064d0 and
/// removed 0x3064e0; the heap's end 0x3067c0) and the C library's (`memset` 0x2fb468, `strcpy` 0x2fd14c, `malloc`
/// 0x2fa818, `free` 0x2fa840)
pub const GET_THREAD_ID: u32 = 0x3066d0;
pub const REFER_THREAD: u32 = 0x3066e0;
pub const CREATE_SEMA: u32 = 0x3067e0;
pub const DELETE_SEMA: u32 = 0x3067f0;
pub const WAIT_SEMA: u32 = 0x306820;
pub const POLL_SEMA: u32 = 0x306830;
pub const SIGNAL_SEMA: u32 = 0x306800;
pub const I_SIGNAL_SEMA: u32 = 0x306810;
pub const CREATE_THREAD: u32 = 0x3065e0;
pub const START_THREAD: u32 = 0x306600;
pub const TERMINATE_THREAD: u32 = 0x306630;
pub const DELETE_THREAD: u32 = 0x3065f0;
pub const DISABLE_INTC: u32 = 0x307208;
pub const ENABLE_INTC: u32 = 0x307270;
pub const ADD_INTC: u32 = 0x3064d0;
pub const REMOVE_INTC: u32 = 0x3064e0;
pub const END_OF_HEAP: u32 = 0x3067c0;
pub const MEMSET: u32 = 0x2fb468;
pub const STRCPY: u32 = 0x2fd14c;
pub const MALLOC: u32 = 0x2fa818;
pub const LIBC_FREE: u32 = 0x2fa840;

/// The boot's other calls: the debug print (0x23d5d0); the SDK's (0x309088, the CD's init 0x2e9188 and 0x2e9660, the
/// IOP's reboot 0x30e2f0 and sync 0x30e2a8, 0x30a7b8, a module's load 0x30ddf0, the memory card's init 0x2f10c8, the
/// pad's 0x2e7078, the clock 0x2e9d78); the effects' seed (0x23d498); the main thread's set up (0x2ceec0), the main
/// heap (0x2cd798), the timer (0x2cf648), the file system (0x2ca170), the async files (0x2c5c60) and the root path
/// (0x2cb218); the video player's make (0x2b9fd8); the vsync tick (0x17c9d8); the stack canary (0x182680)
pub const PRINT: u32 = 0x23d5d0;
pub const SDK_INIT: u32 = 0x309088;
pub const CD_INIT: u32 = 0x2e9188;
pub const CD_MODE: u32 = 0x2e9660;
pub const IOP_REBOOT: u32 = 0x30e2f0;
pub const IOP_SYNC: u32 = 0x30e2a8;
pub const SIF_INIT: u32 = 0x30a7b8;
pub const LOAD_MODULE: u32 = 0x30ddf0;
pub const MC_INIT: u32 = 0x2f10c8;
pub const PAD_INIT: u32 = 0x2e7078;
pub const READ_CLOCK: u32 = 0x2e9d78;
pub const SEED_FX: u32 = 0x23d498;
pub const MAIN_THREAD: u32 = 0x2ceec0;
pub const MAIN_HEAP: u32 = 0x2cd798;
pub const TIMER_INIT: u32 = 0x2cf648;
pub const FILES_INIT: u32 = 0x2ca170;
pub const ASYNC_INIT: u32 = 0x2c5c60;
pub const ROOT_PATH: u32 = 0x2cb218;
pub const PLAYER_NEW: u32 = 0x2b9fd8;
pub const VSYNC_TICK: u32 = 0x17c9d8;
pub const FILL_CANARY: u32 = 0x182680;

pub const LOAD_MODULE_VTABLE: u32 = 0x386900;
pub const LOAD_MODULE_DTOR: u32 = 0x181e48;
pub const VIDEO_VTABLE: u32 = 0x388570;
pub const PS2_EXEC_VTABLE: u32 = 0x387480;
pub const EXEC_VTABLE: u32 = 0x387508;
pub const EXEC_NAME: u32 = 0x387470;
/// The two module names the films' end uses (0x388558, 0x388560)
pub const NAMES: [u32; 2] = [0x388558, 0x388560];
/// The vsync thread's start (0x1825e8), its gp (0x3c38f0), and the two functions given when the interrupt's
/// handler is added (0x182170 second, 0x1825d0 fourth: the one that signals the vsync semaphore)
pub const THREAD_FUNC: u32 = 0x1825e8;
pub const THREAD_GP: u32 = 0x3c38f0;
pub const VSYNC_HANDLER: u32 = 0x1825d0;
pub const INTC_ARG: u32 = 0x182170;
/// The canary word, and the heap's fill (one letter apart)
pub const CANARY: u32 = 0xdeaf_c0ed;
pub const HEAP_FILL: u32 = 0xdeaf_c0de;
/// The canary's record: the stack's top, its bottom, the heap's end, the stack pointer's page (0x3cdd14..)
pub const CANARY_TOP: u32 = 0x3cdd14;
/// The debug strings the boot prints
pub const MESSAGES: [u32; 6] = [0x387238, 0x387280, 0x387298, 0x387418, 0x387448, 0x387468];
/// The IOP reboot's image, and the six modules with the messages between them
pub const IOP_IMAGE: u32 = 0x387258;
pub const MODULES: [u32; 6] = [0x3872b0, 0x3872f0, 0x387330, 0x387370, 0x3873b0, 0x3873f0];
pub const MODULE_MESSAGES: [u32; 5] = [0x3872d8, 0x387318, 0x387358, 0x387398, 0x3873d8];
pub const STACK_MESSAGES: [u32; 2] = [0x3871f8, 0x387218];

/// A virtual call through a table at `at` of the object
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn rend(m: &mut Mem, e: &mut dyn FeEnv, off: u32) -> u32 {
    let r = m.r32(m.r32(GAPP) + 0x724);
    vcall_at(m, e, r, 0x10, off, &[])
}

/// `cVideoLoadModule` vf1, vf4, vf5 and vf6 (0x1819d8, 0x1819c0, 0x1819b8, 0x1819b0): nothing
pub fn nothing() {}

/// `cVideoLoadModule` vf2 and vf3 (0x1819d0, 0x1819c8) -> 1
pub fn one() -> u32 {
    1
}

/// `cVideoLoadModule::~cVideoLoadModule` 0x1819e0(o, flags): its table, then the load module's destructor
/// (0x181e48)
pub fn video_load_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, LOAD_MODULE_VTABLE);
    e.call(m, LOAD_MODULE_DTOR, &[o, flags]);
}

/// `cVideoModule::cVideoModule` 0x1864a0(o, name, then) -> o: no player (+8), its table, the film's name copied to
/// +0xc, +0x10c the word after, +0x110 and +4 cleared
pub fn video_ctor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, name: u32, then: u32) -> u32 {
    m.w32(o + 8, 0);
    m.w32(o, VIDEO_VTABLE);
    e.call(m, STRCPY, &[o + 0xc, name]);
    m.w32(o + 0x10c, then);
    m.w32(o + 0x110, 0);
    m.w32(o + 4, 0);
    o
}

/// `cVideoModule::~cVideoModule` 0x1864f8(o, flags): its table, the player (+8) destroyed when there is one, then
/// the module's destructor
pub fn video_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, VIDEO_VTABLE);
    let p = m.r32(o + 8);
    if p != 0 {
        vcall_at(m, e, p, 0, 8, &[3]);
    }
    gm::module_dtor(m, e, o, flags);
}

/// `cVideoModule` vf1 0x186558(o): the player made (+8)
pub fn video_enter(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let p = e.call(m, PLAYER_NEW, &[]);
    m.w32(o + 8, p);
}

/// `cVideoModule` vf2 0x186580(o) -> whether it may leave: with no player, or one that has finished (+0x28), +4
/// cleared and 1; else 0
pub fn video_leave(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    let p = m.r32(o + 8);
    if p == 0 || vcall_at(m, e, p, 0, 0x28, &[]) != 0 {
        m.w32(o + 4, 0);
        return 1;
    }
    0
}

/// `cVideoModule` vf4 0x1865d8(o): the player's +0x10 with the name (+0xc) and the word after (+0x10c), +4 set
pub fn video_start(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let p = m.r32(o + 8);
    let then = m.r32(o + 0x10c);
    vcall_at(m, e, p, 0, 0x10, &[o + 0xc, then]);
    m.w32(o + 4, 1);
}

/// One module made (`size` bytes from the pool named `name`, filled with 0xdeadc0de when there are) by `ctor`
/// (with `arg` when it takes one)
fn make(m: &mut Mem, e: &mut dyn FeEnv, name: u32, ctor: u32, size: u32, arg: Option<u32>) -> u32 {
    let a = e.call(m, gm::ALLOC, &[name, size, 0x10]);
    if a != 0 {
        e.call(m, gm::FILL, &[a, 0xdead_c0de, size]);
    }
    match arg {
        Some(x) => e.call(m, ctor, &[a, x]),
        None => e.call(m, ctor, &[a]),
    }
}

/// The results modules the films' end picks among: (constructor, size)
pub const RESULTS: [(u32, u32); 6] =
    [(0x24ea20, 0xda8), (0x249758, 0xf18), (0x24b028, 0x11d8), (0x247a60, 0xd64), (0x24d7f8, 0x9f0), (0x245840, 0xec4)];

/// `cVideoModule` vf5 0x186620(o), the frame: the sound's tick (0.02 s); while the film plays its +0x18. Once it is
/// over, the next module, once (+0x110): with the career's +0x3260 flags 0x3b, the front end (0) after a career back
/// (+0x3704 1) or in mode 6 or 1, else the results by +0x370c and the two players; without them, the front end (1)
/// when +0x3710, else the results by the two players and the mode (+0x2af8 10)
pub fn video_frame(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let so = m.r32(gm::SOUND);
    e.call(m, gm::SOUND_TICK, &[so, 0, 0x3ca3_d70a]);
    let p = m.r32(o + 8);
    if vcall_at(m, e, p, 0, 0x28, &[]) == 0 {
        let p = m.r32(o + 8);
        vcall_at(m, e, p, 0, 0x18, &[]);
        return;
    }
    if m.r32(o + 0x110) == 1 {
        return;
    }
    let c = gm::CAREER;
    let (fe_ctor, fe_size) = gm::FRONT_END_MODULE;
    let x = if m.r32(c + 0x3260) & 0x3b != 0 {
        let app = m.r32(GAPP);
        let mode = m.r32(c + 0x2afc);
        let x = if m.r32(c + 0x3704) == 1 || mode == 6 || mode == 1 {
            make(m, e, NAMES[0], fe_ctor, fe_size, Some(0))
        } else if m.r32(c + 0x370c) == 1 {
            make(m, e, NAMES[0], RESULTS[0].0, RESULTS[0].1, None)
        } else if e.call(m, gm::CAREER_TWO_PLAYERS, &[c]) == 1 {
            make(m, e, NAMES[0], RESULTS[1].0, RESULTS[1].1, None)
        } else {
            make(m, e, NAMES[0], RESULTS[2].0, RESULTS[2].1, None)
        };
        (app, x)
    } else if m.r32(c + 0x3710) == 1 {
        let app = m.r32(GAPP);
        (app, make(m, e, NAMES[1], fe_ctor, fe_size, Some(1)))
    } else if e.call(m, gm::CAREER_TWO_PLAYERS, &[c]) != 1 {
        let app = m.r32(GAPP);
        (app, make(m, e, NAMES[1], RESULTS[3].0, RESULTS[3].1, None))
    } else if m.r32(c + 0x2af8) == 10 {
        let app = m.r32(GAPP);
        (app, make(m, e, NAMES[1], RESULTS[4].0, RESULTS[4].1, None))
    } else {
        let app = m.r32(GAPP);
        (app, make(m, e, NAMES[1], RESULTS[5].0, RESULTS[5].1, None))
    };
    e.call(m, gm::APP_SET_MODE, &[x.0, x.1]);
    m.w32(o + 0x110, 1);
}

/// `cVideoModule` vf6 0x186970(o) -> whether it drew: nothing unless the renderer is ready (+0x70); the player's
/// +0x20 (-1), then the frame (+0x80)
pub fn video_draw(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    if rend(m, e, 0x70) == 0 {
        return 0;
    }
    let p = m.r32(o + 8);
    vcall_at(m, e, p, 0, 0x20, &[u32::MAX]);
    rend(m, e, 0x80);
    1
}

/// `cVideoModule` vf3 0x186aa8(o) -> +4 (whether it plays)
pub fn video_playing(m: &Mem, o: u32) -> u32 {
    m.r32(o + 4)
}

/// 0x186a00, a static initialiser: as 0x1858c0, at 0x3cdd98
pub fn video_static_init(m: &mut Mem) {
    crate::start_screen::colours_init(m, 0x3cdd98);
}

/// `cPS2ExecutionMan::~cPS2ExecutionMan` 0x182418(o, flags): its table, then the execution manager's
pub fn ps2_exec_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, PS2_EXEC_VTABLE);
    exec_dtor(m, e, o, flags);
}

/// `cExecutionMan::~cExecutionMan` 0x182c60(o, flags): its table put back, and freed when bit 0 is set
pub fn exec_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, EXEC_VTABLE);
    if flags & 1 != 0 {
        e.call(m, gm::FREE, &[o]);
    }
}

/// `cPS2ExecutionMan` vf1 0x182430(o; `buf` 0xa0 bytes), the start: this thread's id (+4) and priority (+8, from its
/// status); the frame semaphore (+0xc, its waiter +0x10 cleared) and the vsync one (+0x4024, at most 0x400); the vsync
/// thread (+0x4020: 0x1825e8 on the 0x4000-byte stack at +0x20, one priority above this thread's) started with the
/// manager; with interrupt 3 off, a handler added for it with 0x182170 and 0x1825d0 (its id kept at +0x4028)
pub fn ps2_exec_start(m: &mut Mem, e: &mut dyn FeEnv, o: u32, buf: u32) {
    let id = e.call(m, GET_THREAD_ID, &[]);
    m.w32(o + 4, id);
    e.call(m, MEMSET, &[buf, 0, 0x30]);
    let id = m.r32(o + 4);
    e.call(m, REFER_THREAD, &[id, buf]);
    let prio = m.r32(buf + 0x18);
    let s1 = buf + 0x30;
    m.w32(o + 8, prio);
    e.call(m, MEMSET, &[s1, 0, 0x18]);
    let sema = e.call(m, CREATE_SEMA, &[s1]);
    m.w32(o + 0x10, 0);
    m.w32(o + 0xc, sema);
    let s2 = buf + 0x50;
    e.call(m, MEMSET, &[s2, 0, 0x18]);
    m.w32(s2 + 4, 0x400);
    let sema = e.call(m, CREATE_SEMA, &[s2]);
    let t = buf + 0x70;
    m.w32(o + 0x4024, sema);
    e.call(m, MEMSET, &[t, 0, 0x30]);
    let prio = m.r32(o + 8).wrapping_sub(1);
    m.w32(t, 0x10);
    m.w32(t + 4, THREAD_FUNC);
    m.w32(t + 8, o + 0x20);
    m.w32(t + 0xc, 0x4000);
    m.w32(t + 0x18, prio);
    m.w32(t + 0x10, THREAD_GP);
    m.w32(t + 0x14, prio);
    m.w32(t + 0x1c, 0);
    m.w32(t + 0x20, 0);
    let th = e.call(m, CREATE_THREAD, &[t]);
    m.w32(o + 0x4020, th);
    e.call(m, START_THREAD, &[th, o]);
    e.call(m, DISABLE_INTC, &[3]);
    let h = e.call(m, ADD_INTC, &[3, INTC_ARG, 0, VSYNC_HANDLER]);
    m.w32(o + 0x4028, h);
    e.call(m, ENABLE_INTC, &[3]);
}

/// `cPS2ExecutionMan` vf2 0x182578(o), the stop: the handler removed with the interrupt off, the vsync thread ended
/// and deleted, both semaphores deleted
pub fn ps2_exec_stop(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    e.call(m, DISABLE_INTC, &[3]);
    let h = m.r32(o + 0x4028);
    e.call(m, REMOVE_INTC, &[3, h]);
    e.call(m, ENABLE_INTC, &[3]);
    let th = m.r32(o + 0x4020);
    e.call(m, TERMINATE_THREAD, &[th]);
    let th = m.r32(o + 0x4020);
    e.call(m, DELETE_THREAD, &[th]);
    let s = m.r32(o + 0x4024);
    e.call(m, DELETE_SEMA, &[s]);
    let s = m.r32(o + 0xc);
    e.call(m, DELETE_SEMA, &[s]);
}

/// 0x1825d0, the vsync interrupt's handler -> the kernel's answer: the vsync semaphore (the app's manager's +0x4024)
/// signalled
pub fn vsync_handler(m: &mut Mem, e: &mut dyn FeEnv) -> u32 {
    let ex = m.r32(m.r32(GAPP) + 0x28);
    let s = m.r32(ex + 0x4024);
    e.call(m, I_SIGNAL_SEMA, &[s])
}

/// 0x1825e8(o), the vsync thread: for ever, the vsync semaphore waited on, then the app's vsync tick
pub fn vsync_thread(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> ! {
    loop {
        let s = m.r32(o + 0x4024);
        e.call(m, WAIT_SEMA, &[s]);
        let app = m.r32(GAPP);
        e.call(m, VSYNC_TICK, &[app]);
    }
}

/// `cPS2ExecutionMan` vf3 0x182618(o), the frame wait: +0x10 set while waiting on the frame semaphore (+0xc), then
/// the semaphore drained (polled until it answers other than its id)
pub fn ps2_exec_wait(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    m.w32(o + 0x10, 1);
    let s = m.r32(o + 0xc);
    e.call(m, WAIT_SEMA, &[s]);
    m.w32(o + 0x10, 0);
    loop {
        let s = m.r32(o + 0xc);
        let v = e.call(m, POLL_SEMA, &[s]);
        if v != m.r32(o + 0xc) {
            break;
        }
    }
}

/// `cPS2ExecutionMan` vf4 0x182660(o): the frame semaphore signalled when something waits on it (+0x10)
pub fn ps2_exec_signal(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    if m.r32(o + 0x10) != 0 {
        let s = m.r32(o + 0xc);
        e.call(m, SIGNAL_SEMA, &[s]);
    }
}

/// `Stack_FillCanary` 0x182680(top, bottom, end; `sp` the stack pointer inside it): the record kept (the top, the
/// bottom, the end and the stack pointer's page's last byte); the words from the bottom up to the top, and from the
/// end up to 1 KiB under the stack pointer, set to the canary
pub fn fill_canary(m: &mut Mem, top: u32, bottom: u32, end: u32, sp: u32) {
    m.w32(CANARY_TOP, top);
    m.w32(CANARY_TOP + 0xc, sp | 0xffff);
    m.w32(CANARY_TOP + 4, bottom);
    m.w32(CANARY_TOP + 8, end);
    let mut a = bottom;
    while a < top {
        m.w32(a, CANARY);
        a = a.wrapping_add(4);
    }
    let lim = sp.wrapping_sub(0x400);
    let mut a = end;
    while a < lim {
        m.w32(a, CANARY);
        a = a.wrapping_add(4);
    }
}

/// `Mem_PrintStackUsage` 0x182728: nothing before the canary is laid; else the words still the canary counted down
/// from the top to the bottom and printed, then up from the end and printed as what is left of the stack
pub fn print_stack_usage(m: &mut Mem, e: &mut dyn FeEnv) {
    let top = m.r32(CANARY_TOP);
    if top == 0 {
        return;
    }
    let mut a = top.wrapping_sub(4);
    if a >= m.r32(CANARY_TOP + 4) && m.r32(a) == CANARY {
        loop {
            a = a.wrapping_sub(4);
            if a < m.r32(CANARY_TOP + 4) || m.r32(a) != CANARY {
                break;
            }
        }
    }
    let n = (a.wrapping_sub(m.r32(CANARY_TOP + 4)) as i32 >> 2) as u32;
    e.call(m, PRINT, &[STACK_MESSAGES[0], n]);
    let mut a = m.r32(CANARY_TOP + 8);
    if m.r32(a) == CANARY {
        loop {
            a = a.wrapping_add(4);
            if m.r32(a) != CANARY {
                break;
            }
        }
    }
    let n = (m.r32(CANARY_TOP + 0xc).wrapping_sub(a) as i32 >> 2) as u32;
    e.call(m, PRINT, &[STACK_MESSAGES[1], n]);
}

/// A binary-coded decimal byte's value
fn bcd(b: u32) -> u32 {
    (b >> 4) * 10 + (b & 0xf)
}

/// `Iop_RebootAndLoadModules` 0x182858(`buf` 8 bytes): the SDK and the CD up; the IOP rebooted (tried until it
/// takes) and synced; the SDK, the CD and the SIF up again; the six IOP modules loaded (each tried until it loads),
/// with the messages between; the memory card and the pad; the clock read, the effects' seed made from it and the date
/// printed; the main thread; the heap: from 2 KiB past the 64-byte line after a 1 KiB `malloc` (freed) to the heap's
/// end, filled with 0xdeafc0de and made the main heap, the canary laid; the timer (60), the memory system's +8 2, the
/// file system, the async files and the root path
pub fn iop_reboot(m: &mut Mem, e: &mut dyn FeEnv, buf: u32) {
    let sdk = |m: &mut Mem, e: &mut dyn FeEnv| {
        e.call(m, SDK_INIT, &[0]);
        e.call(m, CD_INIT, &[0]);
        e.call(m, CD_MODE, &[2]);
    };
    sdk(m, e);
    e.call(m, PRINT, &[MESSAGES[0]]);
    while e.call(m, IOP_REBOOT, &[IOP_IMAGE]) == 0 {}
    while e.call(m, IOP_SYNC, &[]) == 0 {}
    e.call(m, PRINT, &[MESSAGES[1]]);
    sdk(m, e);
    e.call(m, SIF_INIT, &[]);
    e.call(m, PRINT, &[MESSAGES[2]]);
    for (k, &md) in MODULES.iter().enumerate() {
        if k > 0 {
            e.call(m, PRINT, &[MODULE_MESSAGES[k - 1]]);
        }
        while (e.call(m, LOAD_MODULE, &[md, 0, 0]) as i32) < 0 {}
    }
    e.call(m, MC_INIT, &[]);
    e.call(m, PAD_INIT, &[0]);
    e.call(m, READ_CLOCK, &[buf]);
    let seed = m.r32(buf) ^ (m.r32(buf + 4) << 3);
    e.call(m, SEED_FX, &[seed]);
    let b = |m: &Mem, k: u32| bcd(m.r8(buf + k) as u32);
    let date = [b(m, 5), b(m, 6), b(m, 7), b(m, 3), b(m, 2), b(m, 1)];
    let mut args = vec![MESSAGES[3]];
    args.extend(date);
    e.call(m, PRINT, &args);
    e.call(m, MAIN_THREAD, &[0]);
    let p = e.call(m, MALLOC, &[0x400]);
    e.call(m, LIBC_FREE, &[p]);
    let base = (p.wrapping_add(0x3f) & !0x3f).wrapping_add(0x800);
    let size = e.call(m, END_OF_HEAP, &[]).wrapping_sub(base);
    let end = base.wrapping_add(size);
    e.call(m, PRINT, &[MESSAGES[4], base, size]);
    let mut a = base;
    while a < end {
        m.w32(a, HEAP_FILL);
        a = a.wrapping_add(4);
    }
    e.call(m, MAIN_HEAP, &[base, size]);
    e.call(m, FILL_CANARY, &[base, p, end]);
    e.call(m, TIMER_INIT, &[0x3c]);
    m.w32(0x349d48, 2);
    e.call(m, FILES_INIT, &[0x30, 0x1f4, 0x20]);
    e.call(m, ASYNC_INIT, &[0x10, 0]);
    e.call(m, ROOT_PATH, &[MESSAGES[5]]);
}

/// `PS2ExecutionMan_Create` 0x182ba8(`buf`) -> the manager: the IOP rebooted, 0x4030 bytes (filled with 0xdeadc0de
/// when there are) with its table
pub fn ps2_exec_create(m: &mut Mem, e: &mut dyn FeEnv, buf: u32) -> u32 {
    iop_reboot(m, e, buf);
    let a = e.call(m, gm::ALLOC, &[EXEC_NAME, 0x4030, 0]);
    if a != 0 {
        e.call(m, gm::FILL, &[a, 0xdead_c0de, 0x4030]);
    }
    m.w32(a, PS2_EXEC_VTABLE);
    a
}

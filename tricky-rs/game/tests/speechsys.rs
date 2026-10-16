//! `tricky_game::speechsys` (board row G10c2) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random speech systems, slots and banks; every function run on both sides with the same answers from
//! the calls out (streams' states, times, the play call's handle, allocations), the calls logged in order (floats
//! bit for bit) and the memory they touch compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;
use tricky_game::speechsys::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const STACK_TOP: u32 = 0x01ff_0000;
/// The fake play call and the system's own answer (vtable +0x7c, +0x24)
const V_PLAY: u32 = 0x7f_0000;
const V_OWN: u32 = 0x7f_0100;

const FLOATS: [f32; 15] = [0.0, 100.0, 299.5, 300.0, 330.0, 450.0, 499.9, 500.0, 1200.0, 30.0, 270.0, 340.0, -5.0, 29.0, 329.0];

/// The answers: the same on both sides for the same call (target, how many of it before)
struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    fn answer(&mut self, target: u32) -> u32 {
        let n = match self.count.iter_mut().find(|c| c.0 == target) {
            Some(c) => {
                c.1 += 1;
                c.1
            }
            None => {
                self.count.push((target, 0));
                0
            }
        };
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1) * 0x9e37_79b9_7f4a_7c15);
        for _ in 0..3 {
            g.next();
        }
        match target {
            STREAM_STATE => [0, 1, 1, 2][g.below(4) as usize],
            STREAM_PLAYING => g.below(2),
            STREAM_TIME_LEFT | STREAM_PLAY_TIME => FLOATS[g.below(FLOATS.len() as u32) as usize].to_bits(),
            V_PLAY => [0u32, 5, 0x1234, u32::MAX, 0xffff_fff0][g.below(5) as usize],
            ALLOC => [0u32, 0x10_8000, 0x10_9000][g.below(3) as usize],
            0x228e10 => g.below(3),
            _ => g.next(),
        }
    }
}
type A = Rc<RefCell<Answers>>;

struct Ours(A);
impl SysEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let mut v = args.to_vec();
        if target == 0x22e4e0 {
            v.extend(args.iter().map(|&a| m.r32(a)));
        }
        self.0.borrow_mut().log.push((target, v));
        self.0.borrow_mut().answer(target)
    }
    fn call_f(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> f32 {
        f32::from_bits(self.call(m, target, args))
    }
}

/// The leaves the runner stubs: (address, how many integer arguments, logs f12, logs a2 as a doubleword)
const LEAVES: &[(u32, usize)] = &[
    (ALLOC, 3), (FREE, 1), (MEMSET, 3), (DELETE, 1), (SPRINTF, 3), (LOAD_EVENTS, 3), (MUSIC_INIT, 1), (SCHEDULE, 8),
    (STREAM_DELETE, 2), (STREAM_STATE, 1), (STREAM_TIME_LEFT, 1), (STREAM_PLAY_TIME, 1), (STREAM_PLAYING, 1),
    (STREAM_WAIT, 2), (STREAM_STOP, 2), (STREAM_OPEN, 5), (LIB_SERVICE, 1), (LIB_UNLOAD_BANK, 1), (LIB_SHUTDOWN, 0),
    (0x188990, 2), (0x2bb090, 1), (0x2bb098, 2), (0x2baed0, 0), (0x2baeb8, 0), (0x2be8c8, 1), (0x2bdae0, 0),
    (0x2be208, 1), (0x2bb7a0, 0), (0x2badc0, 0), (0x2bb060, 0), (0x2bb810, 0), (0x228d10, 2), (0x22e4e0, 3),
    (0x209b40, 2), (0x14a040, 1), (0x228e10, 2), (0x2f5450, 1), (V_PLAY, 7), (V_OWN, 1),
];

const ARENA: u32 = 0x0100_0000;
/// The regions compared (and copied for the port): the arena, the globals, the library's globals, the strings
const REGIONS: [(u32, u32); 6] = [(0, 0x400), (ARENA, 0x1_0000), (0x343700, 0x100), (0x348dc0, 0x840), (0x3a0400, 0x80), (0x10_8000, 0x2000)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

#[test]
fn speech_system_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    for &(f, n) in LEAVES {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(if k < 4 { 4 + k } else { 8 + k - 4 }) as u32).collect();
            if f == SCHEDULE {
                // a2 is a doubleword: its two halves, then a3 and t0..t3
                let d = c.gpr(6);
                v = vec![c.gpr(4) as u32, c.gpr(5) as u32, d as u32, (d >> 32) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32, c.gpr(10) as u32, c.gpr(11) as u32];
            }
            if f == V_PLAY {
                v.push(c.f(12).to_bits());
            }
            if f == 0x22e4e0 {
                let w: Vec<u32> = v.iter().map(|&p| mem.read_u32(p)).collect();
                v.extend(w);
            }
            let ans = a2.borrow_mut().answer(f);
            a2.borrow_mut().log.push((f, v));
            c.set_gpr(2, ans as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(ans));
        });
    }
    for f in [0x227ca8u32, 0x229f60, 0x229fc8, 0x22a008, 0x22a178, 0x227c40, 0x229d40, 0x228168, 0x2282e0, 0x228370, 0x227cb8, 0x228700, 0x22a048, 0x2bae08, 0x2baf38, 0x22e550, 0x228998] {
        r.allow(f);
    }
    let app = ARENA + 0xf000;
    let mut g = Rng(0x2284_00ee_8e8e_0001);
    for case in 0..40_000u32 {
        // the arena: the system at ARENA (its stream table +0xa50 -> ARENA + 0x8000, slots' table +0x3718 -> +0x8100),
        // two slots at +0x9000 / +0xa000, banks at +0xb000.., the event at +0xe000
        let sys = ARENA;
        for k in 0..0x1_0000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.next() });
        }
        let streams = ARENA + 0x8000;
        let table = ARENA + 0x8100;
        let slots = [ARENA + 0x9000, ARENA + 0xa000];
        r.mem.write_u32(sys + 0xa50, streams);
        r.mem.write_u32(sys + 0x3718, table);
        let vt = ARENA + 0xc000;
        r.mem.write_u32(sys + 4, vt);
        r.mem.write_u32(vt + 0x7c, V_PLAY);
        r.mem.write_u32(vt + 0x24, V_OWN);
        r.mem.write_u16(vt + 0x78, [0u16, 8, 0xfffc][g.below(3) as usize]);
        r.mem.write_u16(vt + 0x20, [0u16, 4][g.below(2) as usize]);
        for k in 0..8u32 {
            r.mem.write_u32(streams + 4 * k, if g.below(4) == 0 { 0 } else { 0x0100_d000 + 0x10 * k });
        }
        for (k, &s) in slots.iter().enumerate() {
            r.mem.write_u32(table + 4 * k as u32, if g.below(5) == 0 { 0 } else { s });
            r.mem.write_u32(s, g.below(8));
            r.mem.write_u32(s + 4, [u32::MAX, 0, 1, 7][g.below(4) as usize]);
            r.mem.write_u32(s + 0x348, g.below(2));
            let nb = g.below(5);
            r.mem.write_u32(s + 0xec, nb);
            for i in 0..nb {
                let b = if g.below(5) == 0 { 0 } else { ARENA + 0xb000 + 0x100 * (4 * k as u32 + i) };
                r.mem.write_u32(s + 0xf0 + 4 * i, b);
                if b != 0 {
                    r.mem.write_u32(b, g.below(4));
                    r.mem.write_u32(b + 0x88, g.below(2));
                }
            }
            let na = g.below(5);
            r.mem.write_u32(s + 0xe8, na);
            for i in 0..4 {
                r.mem.write_u32(s + 0xcc + 8 * i, g.below(2));
            }
            r.mem.write_u32(s + 0x368, FLOATS[g.below(FLOATS.len() as u32) as usize].to_bits());
        }
        let ev = ARENA + 0xe000;
        r.mem.write_u32(ev, g.below(4));
        r.mem.write_u32(ev + 4, g.next());
        r.mem.write_u32(SYS, sys);
        r.mem.write_u32(BANK_COUNT, g.next());
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x58, g.below(6));
        r.mem.write_u32(sys + 0x6e64, g.below(2));
        for k in 0..12u32 {
            r.mem.write_u32(0x3494f0 + 4 * k, g.next());
        }
        let which = g.below(23);
        let slot = slots[g.below(2) as usize];
        let n = g.below(3);
        let x = [g.next(), g.next(), g.next(), g.next(), g.next()];
        let seed = g.next() as u64 | (g.next() as u64) << 32;
        {
            let mut b = a.borrow_mut();
            b.seed = seed;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let mut gb = vec![0u8; 4];
        r.mem.read(GAPP, &mut gb).unwrap();
        m.regions.push((GAPP, gb));
        // the frames (the port writes them; the runner's hold saved registers too, so not compared)
        m.regions.push((STACK_TOP - 0x200, vec![0; 0x200]));
        let set = |r: &mut Runner, args: &[u32]| {
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(if k < 4 { 4 + k } else { 8 + k - 4 }, *v as i32 as i64 as u64);
            }
        };
        let fe = [u32::MAX, 0, 1, 2][g.below(4) as usize];
        let (addr, args): (u32, Vec<u32>) = match which {
            0 => (0x229f60, vec![sys, n]),
            1 => (0x229fc8, vec![sys, n]),
            2 => (0x22a008, vec![sys, n]),
            3 => (0x22a178, vec![sys, n]),
            4 => (0x22a048, vec![sys, n, x[0]]),
            5 => (0x229d40, vec![sys, n]),
            6 => (0x227c40, vec![slot + 0x10, x[0] & 3]),
            7 => (0x227cb8, vec![slot, x[0], x[1], x[2], x[3], x[4]]),
            8 => (0x228168, vec![slot, x[0] & 3]),
            9 => (0x2282e0, vec![slot]),
            10 => (0x228370, vec![slot, n]),
            11 => (0x228400, vec![slot, ev]),
            12 => (0x2286e8, vec![slot]),
            13 => (0x228700, vec![slot]),
            14 => (0x228728, vec![sys]),
            15 => (0x228830, vec![sys]),
            16 => (0x2288d8, vec![sys]),
            17 => (0x228998, vec![sys]),
            18 => (0x228a00, vec![sys, n & 1, x[1], x[2], x[3], x[4]]),
            19 => (0x228ac0, vec![sys, n & 1, x[0]]),
            20 => (0x228dd0, vec![sys, n & 1]),
            21 => (0x235710, vec![sys]),
            _ => (0x22e5a8, vec![sys, fe]),
        };
        set(&mut r, &args);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let ret_f = r.cpu.f(0).to_bits();
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        {
            let mut b = a.borrow_mut();
            b.count.clear();
        }
        let e = &mut Ours(a.clone());
        let mine_ret: Option<u32> = match which {
            0 => Some(slot_state(&mut m, e, sys, n)),
            1 => Some(time_left(&mut m, e, sys, n).to_bits()),
            2 => Some(play_time(&mut m, e, sys, n).to_bits()),
            3 => Some(is_playing(&mut m, e, sys, n)),
            4 => Some(slot_wait(&mut m, e, sys, n, x[0])),
            5 => {
                destroy_stream(&mut m, e, sys, n);
                None
            }
            6 => {
                bank_delete(&mut m, e, slot + 0x10, x[0] & 3);
                None
            }
            7 => Some(slot_init(&mut m, e, slot, x[0], x[1], x[2], x[3], x[4])),
            8 => {
                slot_free(&mut m, e, slot, x[0] & 3);
                None
            }
            9 => Some(slot_idle(&mut m, e, slot) as u32),
            10 => {
                service_if_idle(&mut m, e, slot, n);
                None
            }
            11 => Some(play_event(&mut m, e, slot, ev, STACK_TOP - 0x1a0) as u32),
            12 => {
                slot_stop(&mut m, e, slot);
                None
            }
            13 => Some(slot_playing(&mut m, e, slot)),
            14 => Some(init(&mut m, e, sys)),
            15 => {
                init_library(&mut m, e, sys);
                None
            }
            16 => {
                free_all(&mut m, e, sys);
                None
            }
            17 => {
                update_all(&mut m, e, sys);
                None
            }
            18 => {
                add_slot(&mut m, e, sys, n & 1, x[1], x[2], x[3], x[4]);
                None
            }
            19 => {
                load_events_for_slot(&mut m, e, sys, n & 1, x[0]);
                None
            }
            20 => Some(is_slot_playing(&mut m, e, sys, n & 1)),
            21 => Some(char_script_ready(&mut m, e, sys)),
            _ => {
                play_fe_line(&mut m, e, sys, fe, STACK_TOP - 0x40);
                None
            }
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            let t = if which == 1 || which == 2 { ret_f } else { ret };
            assert_eq!(t, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i]) {
                panic!("case {case}: {addr:#x}: memory at {:#x}", at + i as u32);
            }
        }
    }
    // the bytes helper on every kind
    for (rate, w) in [(0x5622, 0x10), (44100, 16), (-3, 5), (7, -9), (0x7fff_ffff, 3)] {
        for kind in [0, 1, 2, 3] {
            r.cpu.set_gpr(4, rate as i64 as u64);
            r.cpu.set_gpr(5, w as i64 as u64);
            r.cpu.set_gpr(6, kind as u64);
            r.call(0x2bae08).unwrap();
            assert_eq!(r.cpu.gpr(2) as i32, buffer_bytes(rate, w, kind), "bytes {rate} {w} {kind}");
        }
    }
    // the front end's mode bit
    for mode in 0..7u32 {
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x58, mode);
        r.call(0x22e550).unwrap();
        let mut m = snap(&mut r);
        let mut gb = vec![0u8; 4];
        r.mem.read(GAPP, &mut gb).unwrap();
        m.regions.push((GAPP, gb));
        assert_eq!(r.cpu.gpr(2) as u32, fe_mode_mask(&m), "mode {mode}");
    }
    // the library's fatal stop
    a.borrow_mut().log.clear();
    // (0x2f5450 does not come back: with it stubbed the original runs on in a loop, so only its first call counts)
    assert!(r.call(0x2bebe8).is_err());
    let theirs = std::mem::take(&mut a.borrow_mut().log);
    let mut m = snap(&mut r);
    lib_fatal(&mut m, &mut Ours(a.clone()));
    assert_eq!(theirs[..1], std::mem::take(&mut a.borrow_mut().log)[..]);
    let mut b = [0u8; 1];
    r.mem.read(0x3495d3, &mut b).unwrap();
    assert_eq!(b[0], 0x42);
    // the empty one
    r.call(0x2bec68).unwrap();
}

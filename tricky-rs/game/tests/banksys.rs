//! `tricky_game::banksys` (board row E1b2g7d5c) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random loaders and bank managers (their four queues, a load busy or not, the slots' states, kinds,
//! rings and names), every function run on both sides with the same answers from the calls out (EA's memory, the file
//! readers, the sound library's banks, the owner's and the class's virtuals), the calls logged in order and the memory
//! compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::banksys::*;
use tricky_game::musicsong::SongEnv;
use tricky_game::pf::Mem;

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

const ARENA: u32 = 0x0100_0000;
const SYS: u32 = ARENA;
const SLOTS: u32 = ARENA + 0x1010;
const OBJ: u32 = ARENA + 0x2000;
const OVT: u32 = ARENA + 0x2100;
const VT: u32 = ARENA + 0x2200;
const QDATA: u32 = ARENA + 0x2400;
const FILE: u32 = ARENA + 0x3000;
const HEAP: u32 = ARENA + 0x3800;
const DATA: u32 = ARENA + 0x3c00;
/// The owner's virtuals (+0x24, +0x2c, +0x34, +0x3c) and the class's when not the bank manager's (+0x54, +0x5c, +0x64)
const V_24: u32 = 0x7f_0000;
const V_2C: u32 = 0x7f_0100;
const V_34: u32 = 0x7f_0200;
const V_3C: u32 = 0x7f_0300;
const V_54: u32 = 0x7f_0400;
const V_5C: u32 = 0x7f_0500;
const V_64: u32 = 0x7f_0600;
const NAMES: [&[u8]; 4] = [b"|data\\audio\\Tunnel1.bnk", b"|data\\audio\\RiverSmall.bnk", b"a.bnk", b""];

/// The leaves the runner stubs, with their integer argument counts
const LEAVES: &[(u32, usize)] = &[
    (EAMEM_SET, 3), (EAMEM_COPY, 3), (EAMEM_ALLOC, 3), (EAMEM_FREE, 1), (EAMEM_GET_SIZE, 1), (EAMEM_RESIZE, 2),
    (MEM_DELETE, 1), (MEM_FREE, 1), (MEMPOOL_ALLOC, 6), (TIMER_WAIT, 1), (FILE_LOAD_ALLOC, 2), (FILE_LOAD_INTO, 3),
    (ASYNC_LOAD, 2), (ASYNC_LOAD_INTO, 3), (ASYNC_IS_DONE, 1), (ASYNC_FINISH, 3), (ASYNC_CANCEL, 1), (BANK_REGISTER, 2),
    (BANK_UNREGISTER, 1), (BANK_RESIDENT_SIZE, 1), (BANK_RELOCATE, 2), (BANK_IS_PLAYING, 1), (SND_LOCK, 0),
    (SND_UNLOCK, 0), (MONITOR_INIT, 2), (MONITOR_DTOR, 2), (RING_DESTROY, 2), (V_24, 2), (V_2C, 2), (V_34, 2),
    (V_3C, 1), (V_54, 2), (V_5C, 4), (V_64, 7),
    // the pure-virtual stub in cAsyncSys's vtable, reached when its destructor finishes a load
    (0x2f54e0, 7),
];

/// The functions ported here, run as they are in the runner
const PORTED: &[u32] = &[
    0x208950, 0x208af8, 0x208b78, 0x208c48, 0x208cb0, 0x208ed8, 0x2086d8, 0x209aa0, 0x209a98, 0x209088, 0x209408,
    0x209c18, 0x209c48, 0x209ca8, 0x209cf8, 0x209f10, 0x209f70, 0x20a038, 0x20a0b0, 0x20a118, 0x20a170, 0x20a1b8,
    0x20a218, 0x20a268, 0x20a2f0, 0x20a3d0, 0x20a460, 0x20a508, 0x20a520, 0x20a5d0, 0x20a5f0, 0x20a608, 0x20a628,
    0x20a640, 0x20a658, 0x20a688, 0x20a6b0, 0x20a6e0, STRCPY, STRCMP,
];

struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    /// One call's answer: v0 and the words it writes ([data, size] for `AsyncFile_Finish`, [v0, handle] for
    /// `SndBank_Register`)
    fn reply(&mut self, target: u32) -> (u32, Vec<u32>) {
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
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..3 {
            g.next();
        }
        let v = match target {
            EAMEM_ALLOC | MEMPOOL_ALLOC | FILE_LOAD_ALLOC => [0, HEAP, HEAP + 0x100][g.below(3) as usize],
            FILE_LOAD_INTO => g.below(2),
            ASYNC_IS_DONE => [0u32, 1, 1, 2, u32::MAX][g.below(5) as usize],
            ASYNC_CANCEL | BANK_IS_PLAYING => g.below(3),
            BANK_REGISTER => [7u32, 8, 0, 3][g.below(4) as usize],
            V_24 => [-3i32, -1, 5, 0x100][g.below(4) as usize] as u32,
            V_5C => [0, HEAP + 0x200][g.below(2) as usize],
            _ => g.next(),
        };
        let words = match target {
            ASYNC_FINISH => vec![[0, DATA][g.below(2) as usize], [0i32, -1, 0x100, 0x2000][g.below(4) as usize] as u32],
            BANK_REGISTER => vec![v, g.next()],
            _ => vec![],
        };
        (v, words)
    }
}
type A = Rc<RefCell<Answers>>;

struct Ours(A);
impl SongEnv for Ours {
    fn call(&mut self, _m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        self.0.borrow_mut().reply(target).0
    }
    fn call_f(&mut self, _m: &mut Mem, _target: u32, _args: &[u32]) -> f32 {
        unreachable!()
    }
    fn call_out(&mut self, _m: &mut Mem, target: u32, args: &[u32], _words: usize) -> Vec<u32> {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        self.0.borrow_mut().reply(target).1
    }
}

const REGIONS: [(u32, u32); 4] = [(ARENA, 0x4000), (0x342c40, 0x10), (0, 0x400), (0x39e600, 0x200)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn peek(r: &mut Runner, q: u32) -> Option<u32> {
    let (w, rd) = (r.mem.read_u32(q + 8), r.mem.read_u32(q + 0xc));
    if w == rd {
        return None;
    }
    let data = r.mem.read_u32(q + 4);
    Some(r.mem.read_u32(data + rd * 4))
}

#[test]
fn bank_manager_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    for &(f, n) in LEAVES {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            let (ans, words) = a2.borrow_mut().reply(f);
            match f {
                ASYNC_FINISH if v[1] != 0 => {
                    mem.write_u32(v[1], words[0]);
                    mem.write_u32(v[2], words[1]);
                    v.truncate(1);
                }
                BANK_REGISTER => {
                    mem.write_u32(v[0], words[1]);
                    v.remove(0);
                }
                _ => {}
            }
            a2.borrow_mut().log.push((f, v));
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    for &f in PORTED {
        r.allow(f);
    }
    let mut g = Rng(0x20a5_20ba_4c00_0001);
    let mut seen = [0u32; 38];
    for case in 0..60_000u32 {
        for k in 0..0x4000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.next() });
        }
        for k in 0..0x400 / 4 {
            r.mem.write_u32(4 * k, 0);
        }
        // the owner and its virtuals
        r.mem.write_u32(SYS, OBJ);
        r.mem.write_u32(OBJ, OVT);
        for (o, f) in [(0x20u32, V_24), (0x28, V_2C), (0x30, V_34), (0x38, V_3C)] {
            r.mem.write_u16(OVT + o, [0u16, 8, 0xfff8][g.below(3) as usize]);
            r.mem.write_u32(OVT + o + 4, f);
        }
        // the class's virtuals: the bank manager's, or others with a this-adjust
        r.mem.write_u32(SYS + 4, VT);
        let own = g.below(4) != 0;
        for (o, mine, other) in [(0x50u32, VF_RELEASE, V_54), (0x58, VF_ALLOC, V_5C), (0x60, VF_CONFIGURE, V_64)] {
            r.mem.write_u16(VT + o, if own { 0 } else { [0u16, 8][g.below(2) as usize] });
            r.mem.write_u32(VT + o + 4, if own { mine } else { other });
        }
        // the four queues
        for (k, q) in [0x9f0u32, 0xa00, 0xa10, 0xa20].iter().enumerate() {
            let size = 1 + g.below(6);
            r.mem.write_u32(SYS + q, size);
            r.mem.write_u32(SYS + q + 4, QDATA + 0x40 * k as u32);
            r.mem.write_u32(SYS + q + 8, g.below(size));
            r.mem.write_u32(SYS + q + 0xc, g.below(size));
            for i in 0..8 {
                let v = match k {
                    0 => g.below(4),
                    1 => g.below(4),
                    2 => [0u32, 1, 2, 1, 0, 2, 3, u32::MAX][g.below(8) as usize],
                    _ => g.next(),
                };
                r.mem.write_u32(QDATA + 0x40 * k as u32 + 4 * i, v);
            }
        }
        r.mem.write_u32(SYS + 0x8e8, g.below(2));
        r.mem.write_u32(SYS + 0x8ec, g.below(4));
        for k in 0..4 {
            let s = NAMES[g.below(4) as usize];
            r.mem.write(SYS + 0x8f0 + 0x40 * k, s).unwrap();
            r.mem.write_u8(SYS + 0x8f0 + 0x40 * k + s.len() as u32, 0);
        }
        // the slots: states, kinds, names, a ring of some
        r.mem.write_u32(SYS + 0xa38, 4);
        r.mem.write_u32(SYS + 0xa3c, SLOTS);
        r.mem.write_u32(SLOTS - 0x10, 4);
        for k in 0..4 {
            let s = SLOTS + 0x60 * k;
            r.mem.write_u32(s, g.below(3));
            r.mem.write_u32(s + 8, [0u32, 1, 2, 0, 1, 2, 3][g.below(7) as usize]);
            r.mem.write_u32(s + 0x18, 0);
            r.mem.write_u32(s + 0x1c, 0);
            let n = NAMES[g.below(4) as usize];
            r.mem.write(s + 0x20, n).unwrap();
            r.mem.write_u8(s + 0x20 + n.len() as u32, 0);
        }
        if g.below(2) == 0 {
            let mut ring: Vec<u32> = (0..4).filter(|_| g.below(2) == 0).collect();
            if ring.len() < 2 {
                ring = vec![0, 2];
            }
            for (i, &k) in ring.iter().enumerate() {
                let (next, prev) = (ring[(i + 1) % ring.len()], ring[(i + ring.len() - 1) % ring.len()]);
                r.mem.write_u32(SLOTS + 0x60 * k + 0x18, SLOTS + 0x60 * next);
                r.mem.write_u32(SLOTS + 0x60 * k + 0x1c, SLOTS + 0x60 * prev);
            }
        }
        let file = NAMES[g.below(4) as usize];
        r.mem.write(FILE, file).unwrap();
        r.mem.write_u8(FILE + file.len() as u32, 0);
        r.mem.write_u32(ASYNC_SYS, SYS);
        r.mem.write_u32(BANK_MANAGER, SYS);
        let which = g.below(38);
        // the shut-down waits while a load it can't cancel is busy (for ever, here): only when it can
        if matches!(which, 1 | 3 | 25) && !matches!(peek(&mut r, SYS + 0xa10), Some(0..=2)) {
            r.mem.write_u32(SYS + 0x8e8, 0);
        }
        // the manager's destructor clears gBankManager before freeing its slots, and a static slot with sound
        // memory frees that through the owner found by it: it reads through null, so never a kind 2 there
        if which == 25 {
            for k in 0..4 {
                if r.mem.read_u32(SLOTS + 0x60 * k + 8) == 2 {
                    r.mem.write_u32(SLOTS + 0x60 * k + 8, 1);
                }
            }
        }
        seen[which as usize] += 1;
        let k = g.below(4);
        let s = SLOTS + 0x60 * k;
        let head = SLOTS + 0x60 * g.below(4);
        let w = [g.next(), g.next(), g.next(), g.next()];
        let size = [0u32, 0x100, 0x2000, g.next()][g.below(4) as usize];
        let spu = [0u32, 0x800, g.next()][g.below(3) as usize];
        let flags = [0u32, 0x4000_0000, 0x8000_0000, 0xc000_0010, g.next()][g.below(5) as usize];
        let args: Vec<u32> = match which {
            0 | 24 => vec![SYS, if which == 0 { w[0] } else { g.below(5) }, w[1]],
            1 | 25 => vec![SYS, g.below(4)],
            2 => vec![SYS, size, flags],
            3 | 4 | 7 | 8 | 10 => vec![SYS],
            5 => vec![SYS, k, FILE, [0u32, 1, 2, 3][g.below(4) as usize], w[0]],
            6 => vec![SYS, g.below(2)],
            9 | 27 | 30 | 32 | 33 | 36 | 37 => vec![SYS, k],
            11 | 13 | 15 | 17 | 19 | 21 => vec![s],
            12 => vec![s, g.below(4)],
            14 => vec![s, FILE, w[0]],
            16 => vec![s, size, spu, w[0]],
            18 => vec![s, head],
            20 => vec![s, FILE],
            22 => vec![s, size, w[0]],
            23 => vec![s, w[0], g.below(3), FILE],
            26 | 28 => vec![SYS, k, FILE, w[0]],
            29 => vec![SYS, k, size, spu, w[0]],
            31 => vec![SYS, k, g.below(4)],
            34 => vec![SYS, k, size, w[0]],
            _ => vec![SYS, k, w[0], g.below(3), FILE, w[1], w[2]],
        };
        let seed = g.next() as u64 | (g.next() as u64) << 32;
        *a.borrow_mut() = Answers { seed, count: vec![], log: vec![] };
        let mut m = snap(&mut r);
        let addr = [
            0x208950, 0x208af8, 0x208b78, 0x208c48, 0x208cb0, 0x208ed8, 0x2086d8, 0x209aa0, 0x209a98, 0x209088, 0x209408,
            0x209c18, 0x209c48, 0x209ca8, 0x209cf8, 0x209f10, 0x209f70, 0x20a038, 0x20a0b0, 0x20a118, 0x20a170,
            0x20a1b8, 0x20a218, 0x20a268, 0x20a2f0, 0x20a3d0, 0x20a460, 0x20a508, 0x20a520, 0x20a5d0, 0x20a5f0,
            0x20a608, 0x20a628, 0x20a640, 0x20a658, 0x20a688, 0x20a6b0, 0x20a6e0,
        ][which as usize];
        let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(reg(k), *v as i32 as i64 as u64);
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which}): {e}"));
        let v0 = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        *a.borrow_mut() = Answers { seed, count: vec![], log: vec![] };
        let mut e = Ours(a.clone());
        let x = |k: usize| args[k];
        let ours_v0: Option<u32> = match which {
            0 => Some(async_init(&mut m, &mut e, x(0), x(1))),
            1 => { async_destroy(&mut m, &mut e, x(0), x(1)); None }
            2 => { async_alloc_buffer(&mut m, &mut e, x(0), x(1), x(2)); None }
            3 => { async_shutdown(&mut m, &mut e, x(0)); None }
            4 => { async_start_next(&mut m, &mut e, x(0)); None }
            5 => { async_queue_load(&mut m, &mut e, x(0), x(1), x(2), x(3), x(4)); None }
            6 => { async_pop_queue(&mut m, &mut e, x(0), x(1)); None }
            7 => { async_pop_next(&mut m, &mut e, x(0)); None }
            8 => { async_cancel_all(&mut m, &mut e, x(0)); None }
            9 => { async_cancel_bank(&mut m, &mut e, x(0), x(1)); None }
            10 => { async_update(&mut m, &mut e, x(0)); None }
            11 => Some(slot_init(&mut m, x(0))),
            12 => { slot_destroy(&mut m, &mut e, x(0), x(1)); None }
            13 => { slot_unload_chain(&mut m, &mut e, x(0)); None }
            14 => { slot_load(&mut m, &mut e, x(0), x(1), x(2)); None }
            15 => { slot_unload(&mut m, &mut e, x(0)); None }
            16 => { slot_alloc_static(&mut m, &mut e, x(0), x(1), x(2), x(3)); None }
            17 => { slot_free(&mut m, &mut e, x(0)); None }
            18 => { slot_link(&mut m, x(0), x(1)); None }
            19 => { slot_unlink(&mut m, x(0)); None }
            20 => Some(slot_is_loaded(&m, x(0), x(1)) as u32),
            21 => { slot_release(&mut m, &mut e, x(0)); None }
            22 => Some(slot_alloc_dynamic(&mut m, &mut e, x(0), x(1), x(2))),
            23 => { slot_configure(&mut m, &mut e, x(0), x(1), x(2), x(3)); None }
            24 => Some(manager_init(&mut m, &mut e, x(0), x(1), x(2))),
            25 => { manager_destroy(&mut m, &mut e, x(0), x(1)); None }
            26 => { manager_load_now(&mut m, &mut e, x(0), x(1), x(2), x(3)); None }
            27 => { manager_unload(&mut m, &mut e, x(0), x(1)); None }
            28 => { manager_load(&mut m, &mut e, x(0), x(1), x(2), x(3)); None }
            29 => { manager_alloc_static(&mut m, &mut e, x(0), x(1), x(2), x(3), x(4)); None }
            30 => { manager_free(&mut m, &mut e, x(0), x(1)); None }
            31 => { manager_link(&mut m, x(0), x(1), x(2)); None }
            32 => { manager_unlink(&mut m, x(0), x(1)); None }
            33 => { manager_release(&mut m, &mut e, x(0), x(1)); None }
            34 => Some(manager_alloc(&mut m, &mut e, x(0), x(1), x(2), x(3))),
            35 => { manager_configure(&mut m, &mut e, x(0), x(1), x(2), x(3), x(4)); None }
            36 => Some(manager_handle(&m, x(0), x(1))),
            _ => Some(manager_is_ready(&mut m, &mut e, x(0), x(1)) as u32),
        };
        let ours = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(ours, theirs, "case {case} ({which} {addr:#x}): the calls");
        if let Some(v) = ours_v0 {
            assert_eq!(v, v0, "case {case} ({which} {addr:#x}): v0");
        }
        let after = snap(&mut r);
        for ((at, b), (_, ob)) in after.regions.iter().zip(m.regions.iter()) {
            if b != ob {
                let i = b.iter().zip(ob.iter()).position(|(x, y)| x != y).unwrap();
                panic!("case {case} ({which} {addr:#x}): memory differs at {:#x}: {:#x} (original) vs {:#x}", at + i as u32, b[i], ob[i]);
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 1000), "{seen:?}");
}

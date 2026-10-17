//! `tricky_game::texman` (board rows G7q1..) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is one of the manager's functions on fresh random memory: names with the world's and
//! the players' prefixes, near misses and others; every format (and ones that make no record), palette kinds,
//! sizes that are powers of two and not; heap flags with the pool bit and the clearing bit; allocations that
//! fail; courses with and without detail settings. The C library and heap functions are stand-ins on both sides
//! that do their work (compare, copy names, hand out addresses that differ by call, the natural log) and log what
//! they are handed. The calls, the answer and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::TextBoxEnv;
use tricky_game::texman as tx;

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

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const TM: u32 = ARENA;
const RECS: u32 = ARENA + 0x100;
const NAME: u32 = ARENA + 0x600;
const PIXELS: u32 = ARENA + 0x700;
const PALETTE: u32 = ARENA + 0x800;
const APPOBJ: u32 = ARENA + 0x900;
const GAME: u32 = ARENA + 0xc00;
const SET: u32 = ARENA + 0x1000;
/// Where the stand-in heaps hand out from (never read: the copies and fills are only logged)
const HEAP: u32 = 0x0300_0000;
/// The app (0x337c58) and the manager's strings (0x390930..)
const DATA: [(u32, u32); 2] = [(0x337c00, 0x100), (0x390900, 0x100)];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What the stand-ins need of a memory
trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8; 1];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
}

const STANDINS: [u32; 9] = [tx::STRNCMP, tx::STRNCPY, tx::POOL_ALLOC, tx::ALLOC, tx::MEM_SET, tx::MEM_COPY, tx::PRINTF, tx::LOGF, tx::FREE];

/// A stand-in's work and answer (and, for `logf`, a float), and its log. `fails` makes the heaps answer 0;
/// `taken` counts what they have handed out in this call.
fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], fails: bool, taken: &Cell<u32>) -> (u32, Call) {
    let mut log = Vec::new();
    let answer = match func {
        tx::STRNCMP => {
            log.extend([w[0], w[1], w[2]]);
            let mut d = 0i32;
            for k in 0..w[2] {
                let (a, b) = (io.r8(w[0] + k), io.r8(w[1] + k));
                if a != b {
                    d = a as i32 - b as i32;
                    break;
                }
                if a == 0 {
                    break;
                }
            }
            d as u32
        }
        tx::STRNCPY => {
            log.extend([w[0], w[1], w[2]]);
            let mut end = false;
            for k in 0..w[2] {
                let c = if end { 0 } else { io.r8(w[1] + k) };
                end |= c == 0;
                io.w8(w[0] + k, c);
            }
            w[0]
        }
        tx::POOL_ALLOC | tx::ALLOC => {
            log.extend(if func == tx::ALLOC { &w[..3] } else { &w[..6] });
            let k = taken.get();
            taken.set(k + 1);
            // an address that differs by call and is not always on a 128-byte boundary
            if fails { 0 } else { HEAP + 0x10000 * k + 0x10 * (w[1] % 7) }
        }
        tx::MEM_SET | tx::MEM_COPY => {
            log.extend([w[0], w[1], w[2]]);
            0
        }
        tx::PRINTF | tx::FREE => {
            log.push(w[0]);
            0
        }
        _ => {
            // logf
            log.push(f[0]);
            f32::from_bits(f[0]).ln().to_bits()
        }
    };
    log.push(answer);
    (answer, Call(func, log))
}

struct Ours {
    calls: Vec<Call>,
    fails: bool,
    taken: Cell<u32>,
}
impl TextBoxEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(4, 0);
        let (a, c) = standin(m, func, &w, &f, self.fails, &self.taken);
        self.calls.push(c);
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn texture_manager_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [0x1c0cd8u32, 0x1c0da8, 0x1c0e78, 0x1c0ef8, 0x1c17c0];
    for f in funcs {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let fails = Rc::new(Cell::new(false));
    let taken = Rc::new(Cell::new(0u32));
    for func in STANDINS {
        let (l, fl, tk) = (log.clone(), fails.clone(), taken.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..16).map(|k| c.fpr[k]).collect();
            let (a, call) = standin(mem, func, &w, &f, fl.get(), &tk);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
            c.fpr[0] = a;
        });
    }
    let data: Vec<Vec<u8>> = DATA
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            b
        })
        .collect();
    let mut g = Rng(0x7e70_0000_0000_0001);
    let mut seen = [0usize; 5];
    let (mut records, mut palettes, mut shared) = (0usize, 0usize, 0usize);
    for case in 0..5 * 2000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for (k, &(at, _)) in DATA.iter().enumerate() {
            m.regions.push((at, data[k].clone()));
        }
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the manager: its records, how many, the heap flags
        m.w32(TM + 0x24, RECS);
        m.w32(TM + 0x1c, g.below(4));
        m.w32(TM + 0x18, g.pick(&[0u32, 0x4000_0000, 0x8000_0001, 0xc000_0002, 0x1234_5678]));
        m.w32(TM + 0x2c, g.pick(&[0u32, 1]));
        m.w32(TM + 0x6c, g.pick(&[0u32, 1]));
        m.w32(TM + 0x10, g.pick(&[0u32, ARENA + 0x1800]));
        // the name: a prefix, a near miss or none
        let name: &[u8] = g.pick(&[&b"wrldsnow\0"[..], b"wrld\0", b"plr_mac1\0", b"pl\0", b"wrl_\0", b"rider01\0", b"\0", b"verylongname\0"]);
        for (k, &c) in name.iter().enumerate() {
            m.w8(NAME + k as u32, c);
        }
        // the course's detail settings (or none)
        m.w32(0x337c58, APPOBJ);
        m.w32(APPOBJ + 0x730, g.pick(&[0u32, GAME, GAME, GAME]));
        m.w32(GAME + 0x2c, g.pick(&[0u32, SET, SET, SET]));
        m.w32(GAME + 0x298, g.pick(&[0u32, 1, 2]));
        m.w32(SET + 0x68, g.pick(&[0u32, 1, 1]));
        wf(&mut m, SET + 0x6c, { let v = g.f(0.5, 5000.0); g.pick(&[1.0f32, 2.0, 100.0, v]) });
        wf(&mut m, SET + 0x74, { let v = g.f(0.5, 5000.0); g.pick(&[1.0f32, 8.0, v]) });
        m.w32(SET + 0x70, g.below(4));
        m.w32(SET + 0x78, g.below(4));
        // (and one so big its size in bytes is negative as a word)
        let (w, h) = if g.below(20) == 0 { (0x4000, 0x8000) } else { (g.pick(&[1u32, 16, 64, 512, 1024, 100, 3]), g.pick(&[1u32, 32, 256, 512, 2048, 48])) };
        let format = g.pick(&[0u32, 2, 4, 4, 1, 3, 0x13]);
        let (kind, kept) = (g.pick(&[1u32, 1, 0, 2]), g.pick(&[0u32, 1, 3]));
        fails.set(g.below(8) == 0);
        taken.set(0);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut o = Ours { calls: Vec::new(), fails: fails.get(), taken: Cell::new(0) };
        let which = (case % 5) as usize;
        seen[which] += 1;
        let func = funcs[which];
        let (regs, stack, ours): (Vec<u32>, Vec<u32>, u32) = match which {
            0 => {
                tx::init(&mut m, &mut o, TM);
                (vec![TM], vec![], 0)
            }
            1 => {
                tx::calc_lod_params(&mut m, &mut o, TM);
                (vec![TM], vec![], 0)
            }
            2 => {
                tx::shut_down(&mut m, &mut o, TM);
                (vec![TM], vec![], 0)
            }
            3 => {
                let n = tx::register(&mut m, &mut o, TM, PIXELS, w, h, format, NAME, PALETTE, kind, kept);
                (vec![TM, PIXELS, w, h, 0, format, NAME, PALETTE], vec![kind, kept], n)
            }
            _ => {
                let n = tx::lock(&mut m, &mut o, TM, w, h, format);
                (vec![TM, w, h, format], vec![], n)
            }
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        // the arguments past t3 on the caller's stack (the runner's stack top), a doubleword each
        let sp = 0x01ff_0000u32;
        for (k, v) in stack.into_iter().enumerate() {
            r.mem.write_u32(sp + 8 * k as u32, v);
            r.mem.write_u32(sp + 8 * k as u32 + 4, 0);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != o.calls {
            let i = theirs.iter().zip(&o.calls).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.calls.len()));
            let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(12)])).unwrap_or_default();
            panic!("case {case} {func:#x}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.calls.len(), show(theirs.get(i)), show(o.calls.get(i)));
        }
        let answer = r.cpu.gpr(2) as u32;
        assert!(which < 3 || answer == ours, "case {case} {func:#x}: answer theirs {answer:#x} ours {ours:#x}");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        records += (which >= 3 && o.calls.iter().any(|c| c.0 == tx::STRNCPY)) as usize;
        palettes += (which == 3 && o.calls.iter().filter(|c| c.0 == tx::MEM_COPY).count() > 1) as usize;
        shared += (which == 3 && m.r32(RECS + 0x5c * ours + 0x48) == 0 && o.calls.iter().any(|c| c.0 == tx::STRNCPY)) as usize;
    }
    println!("{records} records, {palettes} with palettes, {shared} shared");
    assert!(records > 1000 && palettes > 100 && shared > 50);
}

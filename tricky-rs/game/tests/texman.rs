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

const STANDINS: [u32; 12] = [tx::STRNCMP, tx::STRNCPY, tx::POOL_ALLOC, tx::ALLOC, tx::MEM_SET, tx::MEM_COPY, tx::PRINTF, tx::LOGF, tx::FREE, tx::TEX_SWIZZLE, tx::FLUSH_CACHE, tx::DMA_SEND];

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
        tx::TEX_SWIZZLE => {
            log.extend(&w[..5]);
            0
        }
        tx::DMA_SEND => {
            log.extend([w[0], w[1]]);
            0
        }
        tx::FLUSH_CACHE => {
            // (only a0 is set)
            log.push(w[0]);
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

/// The runner with the stand-ins, their log and their switches
fn runner() -> Option<(Runner, Rc<RefCell<Vec<Call>>>, Rc<Cell<bool>>, Rc<Cell<u32>>)> {
    let mut r = Runner::from_env()?;
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
    Some((r, log, fails, taken))
}

/// Runs both and compares the calls and the memory
fn compare(r: &mut Runner, log: &Rc<RefCell<Vec<Call>>>, m: &mut Mem, case: u32, func: u32, regs: &[u32], ours: impl FnOnce(&mut Mem, &mut Ours)) -> Vec<Call> {
    for (at, b) in &m.regions {
        r.mem.write(*at, b).unwrap();
    }
    log.borrow_mut().clear();
    let mut o = Ours { calls: Vec::new(), fails: false, taken: Cell::new(0) };
    ours(m, &mut o);
    for (k, &v) in regs.iter().enumerate() {
        r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
    }
    r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
    let theirs = log.borrow().clone();
    if theirs != o.calls {
        let i = theirs.iter().zip(&o.calls).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.calls.len()));
        let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(12)])).unwrap_or_default();
        panic!("case {case} {func:#x}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.calls.len(), show(theirs.get(i)), show(o.calls.get(i)));
    }
    for (at, b) in &m.regions {
        let mut th = vec![0u8; b.len()];
        r.mem.read(*at, &mut th).unwrap();
        if &th != b {
            let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
            let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
            panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
        }
    }
    theirs
}

const TABLE: u32 = ARENA + 0x100;
const SLOTS: u32 = ARENA + 0x600;
const CLUTS: u32 = ARENA + 0x700;
const PACKET: u32 = ARENA + 0x1400;
const PLACE: u32 = ARENA + 0x1600;
const COLOUR: u32 = ARENA + 0x1700;

/// A manager with four records in its table (sizes, formats, slots), its VRAM and CLUT slots
fn manager(g: &mut Rng) -> Mem {
    let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    m.w32(TM + 0x10, TABLE);
    m.w32(TM + 0x3c, g.pick(&[0u32, SLOTS, SLOTS]));
    m.w32(TM + 0x6c, g.pick(&[0u32, CLUTS, CLUTS]));
    m.w32(TM + 0x70, g.below(5));
    m.w32(PLACE, PACKET + 8 * g.below(4));
    for n in 0..4u32 {
        let r = TABLE + 0x5c * n;
        m.w32(r + 0x10, g.pick(&[1u32, 16, 64, 128, 512, 1000, 0x2000, 0x7fff_fff0, 0x8000_0010]));
        m.w32(r + 0x18, g.pick(&[1u32, 32, 256, 513, 2]));
        m.w32(r + 0x20, g.pick(&[0u32, 2, 0x13, 0x13, 1, 0x14]));
        m.w32(r + 0x24, g.pick(&[0u32, 1, 3, 0xffff_ffff, 0xffff_fffe]));
        m.w32(r + 0x30, g.pick(&[0u32, 2, 0xffff_ffff]));
        m.w32(r + 0x3c, g.below(2));
        m.w32(r + 0x2c, ARENA + 0x1800);
        m.w32(r + 0x38, ARENA + 0x1900);
    }
    for k in 0..4 {
        wf(&mut m, COLOUR + 4 * k, g.pick(&[0.0f32, 1.0, 0.5, -0.5, 2.0]));
    }
    m
}

#[test]
fn uploads_match_the_original() {
    let Some((mut r, log, _, taken)) = runner() else { return };
    for f in [0x1c1ab8u32, 0x1c1bd0, 0x1c1d58] {
        r.allow(f);
    }
    let mut g = Rng(0x7e70_1000_0000_0001);
    let (mut swizzled, mut palettes) = (0usize, 0usize);
    for case in 0..6000u32 {
        let mut m = manager(&mut g);
        taken.set(0);
        let n = g.below(4);
        if case % 3 == 0 {
            // (the draw-into packet reads its slot unchecked: one there)
            m.w32(TM + 0x3c, SLOTS);
            m.w32(TABLE + 0x5c * n + 0x24, g.below(4));
        }
        let theirs = match case % 3 {
            0 => compare(&mut r, &log, &mut m, case, 0x1c1ab8, &[TM, n, PLACE], |m, _| tx::draw_into_packet(m, TM, n, PLACE)),
            1 => compare(&mut r, &log, &mut m, case, 0x1c1bd0, &[TM, n, COLOUR, PLACE], |m, _| tx::fill_packet(m, TM, n, COLOUR, PLACE)),
            _ => compare(&mut r, &log, &mut m, case, 0x1c1d58, &[TM, PIXELS, PALETTE, n], |m, o| tx::upload_pixels(m, o, TM, PIXELS, PALETTE, n)),
        };
        swizzled += theirs.iter().filter(|c| c.0 == tx::TEX_SWIZZLE).count();
        palettes += (theirs.iter().filter(|c| c.0 == tx::MEM_COPY).count() > 30) as usize;
    }
    println!("{swizzled} swizzled, {palettes} palettes");
    assert!(swizzled > 100 && palettes > 100);
}

const BLOCKS: u32 = ARENA + 0x800;
const NODES: u32 = ARENA + 0x900;

/// The manager's VRAM blocks (three of 0x18 bytes, from +0x40 to +0x2c; their slots a list of nodes: +4 the
/// record or −1, +8 the slot, +0xc the next), its slots (+0x3c: four of 0x14 bytes and the one before), its CLUT
/// slots (+0x6c: three of 16 and the one before), the records' uses (+0x58) and slots, the course's settings
fn slotted(g: &mut Rng, m: &mut Mem) {
    m.w32(TM + 0x3c, SLOTS);
    m.w32(TM + 0x6c, CLUTS);
    m.w32(TM + 0x34, BLOCKS);
    m.w32(TM + 0x40, g.pick(&[0u32, 0, 1, 3]));
    m.w32(TM + 0x2c, g.pick(&[0u32, 2, 3, 3]));
    m.w32(TM + 0x68, g.pick(&[0u32, 1, 3, 3]));
    m.w32(TM + 0xc, g.pick(&[0u32, 2, 4, 4]));
    m.w32(TM + 0x70, g.pick(&[0u32, 1, 0xffff_ffff]));
    m.w32(TM + 0x74, g.pick(&[2u32, 3, 0xffff_ffff]));
    for b in 0..3u32 {
        let blk = BLOCKS + 0x18 * b;
        m.w32(blk + 4, g.pick(&[1u32, 2, 4, 8]));
        m.w32(blk + 8, g.pick(&[1u32, 2, 3]));
        m.w32(blk + 0x14, if g.below(5) == 0 { 0 } else { NODES + 0x40 * b });
        for k in 0..3u32 {
            let node = NODES + 0x40 * b + 0x10 * k;
            m.w32(node + 4, if g.below(6) == 0 { 0xffff_ffff } else { g.below(4) });
            m.w32(node + 8, g.below(4));
            m.w32(node + 0xc, if k < 2 && g.below(4) != 0 { node + 0x10 } else { 0 });
        }
    }
    for k in 0..5u32 {
        let s = SLOTS - 0x14 + 0x14 * k;
        m.w32(s, g.below(0x4000));
        m.w32(s + 4, g.below(4));
    }
    for k in 0..4u32 {
        let c = CLUTS - 0x10 + 0x10 * k;
        m.w32(c, g.below(0x4000));
        m.w32(c + 0xc, if g.below(6) == 0 { 0xffff_ffff } else { g.below(4) });
    }
    for n in 0..4u32 {
        let r = TABLE + 0x5c * n;
        wf(m, r + 0x58, g.pick(&[0.0f32, 1.0, 0.5, 3.0, 0.25]));
        m.w32(r + 0xc, g.pick(&[1u32, 2, 4, 8]));
        m.w32(r + 0x24, g.pick(&[0u32, 1, 2, 3, 0xffff_ffff, 0xffff_ffff]));
        m.w32(r + 0x30, g.pick(&[0u32, 1, 2, 0xffff_ffff]));
        m.w32(r + 0x40, g.below(2));
        m.w32(r + 0x44, g.below(3));
        m.w32(r + 0x48, g.pick(&[0u32, 1, 2, 3, 0xffff_ffff]));
        m.w32(r + 0x20, g.pick(&[0u32, 1, 2, 0x13, 0x13, 0x1b, 0x14, 0xffff_ffff]));
        // (and one of 2^31 bytes or more)
        let huge = g.below(10) == 0;
        m.w32(r + 0x10, if huge { 0x10000 } else { g.pick(&[8u32, 64, 100, 256, 512]) });
        m.w32(r + 0x18, if huge { 0x8000 } else { g.pick(&[8u32, 64, 32, 512]) });
        m.w32(r + 0x14, g.below(10));
        m.w32(r + 0x1c, g.below(10));
    }
    m.w32(0x337c58, APPOBJ);
    m.w32(APPOBJ + 0x730, g.pick(&[0u32, GAME, GAME]));
    m.w32(GAME + 0x2c, g.pick(&[0u32, SET, SET]));
    m.w32(GAME + 0x298, g.pick(&[1u32, 2]));
    m.w32(TM + 0x78, g.pick(&[1u32, 2]));
    m.w32(SET + 0x68, 1);
    let d = g.f(1.0, 3000.0);
    wf(m, SET + 0x6c, d);
    wf(m, SET + 0x74, d * 2.0);
    m.w32(SET + 0x70, g.below(3));
    m.w32(SET + 0x78, g.below(3));
}

#[test]
fn slots_and_binds_match_the_original() {
    let Some((mut r, log, fails, taken)) = runner() else { return };
    let funcs = [0x1c3348u32, 0x1c3480, 0x1c3560, 0x1c3650, 0x1c3760, 0x1c3818, 0x1c3ba8, 0x1c3d88, 0x1c3f68];
    for f in funcs.into_iter().chain([0x1c0da8]) {
        r.allow(f);
    }
    let data: Vec<Vec<u8>> = DATA
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            b
        })
        .collect();
    let mut g = Rng(0x7e70_2000_0000_0001);
    let mut seen = [0usize; 9];
    let (mut uploads, mut evictions, mut free_found) = (0usize, 0usize, 0usize);
    for case in 0..9 * 1500u32 {
        let mut m = manager(&mut g);
        for (k, &(at, _)) in DATA.iter().enumerate() {
            m.regions.push((at, data[k].clone()));
        }
        m.regions.push((tx::SPR, vec![0; 0xb0]));
        slotted(&mut g, &mut m);
        fails.set(false);
        taken.set(0);
        let which = (case % 9) as usize;
        seen[which] += 1;
        let func = funcs[which];
        let n = g.below(4);
        let (a, b) = (g.pick(&[1u32, 2, 4, 8]), g.pick(&[0u32, 1, 2, 3, 0xffff_ffff]));
        let flags = g.pick(&[0u32, 0x10, 0x4000_0000, 0x8000_0010]);
        let vram = g.below(0x4000);
        let mut ours = 0u32;
        let regs: Vec<u32> = match which {
            0 => vec![TM],
            1 => vec![TM, flags],
            2 => vec![TM],
            3 => vec![TM, a, b],
            4 => vec![TM, b],
            5 => vec![TM, n, g.pick(&[1u32, 2]), PLACE],
            6 => vec![TM, n, vram],
            7 => vec![TM, n, PLACE, vram],
            _ => vec![TM, n, PLACE, vram],
        };
        let regs2 = regs.clone();
        let theirs = compare(&mut r, &log, &mut m, case, func, &regs, |m, o| match which {
            0 => tx::update_lod(m, o, TM),
            1 => tx::init_load_nodes(m, o, TM, regs2[1]),
            2 => tx::free_all(m, o, TM),
            3 => ours = tx::find_vram_slot(m, TM, regs2[1], regs2[2]),
            4 => ours = tx::find_clut_slot(m, TM, regs2[1]),
            5 => ours = tx::bind(m, TM, regs2[1], regs2[2], PLACE),
            6 => tx::upload_texture(m, o, TM, regs2[1], regs2[2]),
            7 => tx::vram_packet(m, TM, regs2[1], PLACE, regs2[3]),
            _ => tx::clut_packet(m, TM, regs2[1], PLACE, regs2[3]),
        });
        let answer = r.cpu.gpr(2) as u32;
        assert!(!matches!(which, 3..=5) || answer == ours, "case {case} {func:#x}: answer theirs {answer:#x} ours {ours:#x}");
        uploads += (which == 5 && ours == 2) as usize;
        evictions += (matches!(which, 3 | 4) && (ours as i32) >= 0) as usize;
        free_found += theirs.len();
    }
    println!("{uploads} binds that uploaded, {evictions} slots found");
    assert!(uploads > 200 && evictions > 1000);
    let _ = free_found;
}

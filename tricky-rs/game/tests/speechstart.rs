//! The starter of `tricky_game::speechplay` (board row G10l): picking rows, the callbacks and starting events
//! (0x2bd430..0x2be468) against the game's code in the function runner (TRICKY_ELF; skipped without it). The
//! library functions that are not under test and have no ported body (kind, event word, bank size, the bit test, the
//! bank lookups, event find) and the four callbacks are stubs answering from one random stream on both sides (the
//! callbacks log the block they are given); the rest runs as the game's. The calls, the return, the headers, the
//! tables, the library's random state and the output frames are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::Mem;
use tricky_game::speechplay::*;
use tricky_game::speechload::LoadEnv;

#[derive(Clone)]
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

#[derive(Default)]
struct SM(HashMap<u32, u8>);
impl Mem for SM {
    fn r8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn r16(&self, a: u32) -> u16 {
        self.r8(a) as u16 | (self.r8(a + 1) as u16) << 8
    }
    fn r32(&self, a: u32) -> u32 {
        self.r16(a) as u32 | (self.r16(a + 2) as u32) << 16
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
    fn w16(&mut self, a: u32, v: u16) {
        self.w8(a, v as u8);
        self.w8(a + 1, (v >> 8) as u8);
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.w16(a, v as u16);
        self.w16(a + 2, (v >> 16) as u16);
    }
}

struct RM2<'a>(&'a mut r5900::Mem);
impl Mem for RM2<'_> {
    fn r8(&self, a: u32) -> u8 {
        let mut b = [0u8];
        // the runner reads through &mut; a clone of the pointer keeps this trait's &self
        unsafe { &mut *(self.0 as *const r5900::Mem as *mut r5900::Mem) }.read(a, &mut b).unwrap();
        b[0]
    }
    fn r16(&self, a: u32) -> u16 {
        self.r8(a) as u16 | (self.r8(a + 1) as u16) << 8
    }
    fn r32(&self, a: u32) -> u32 {
        self.r16(a) as u32 | (self.r16(a + 2) as u32) << 16
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.0.write_u8(a, v)
    }
    fn w16(&mut self, a: u32, v: u16) {
        self.0.write_u16(a, v)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.0.write_u32(a, v)
    }
}

const K: u32 = 4;
const HDR: u32 = 0x200;
const EVSZ: u32 = 0x800;
const SP0: u32 = 0x01ff_0000;
const TEST_BIT_A: u32 = 0x2bc9a0;
const CB0T: u32 = 0x90_0000;
const CB1T: u32 = 0x90_0010;
const CB2T: u32 = 0x90_0020;
const CB4T: u32 = 0x90_0040;

fn nargs(t: u32) -> usize {
    match t {
        BANK_ID | FIND_EVENT_T | BANK_SIZE | CB0T => 1,
        TEST_BIT_A | BANK_SLOT | BANK_HAS | KIND | EVENT_WORD | CB4T => 2,
        CB1T | CB2T => 4,
        _ => panic!("target {t:#x}"),
    }
}
const FIND_EVENT_T: u32 = 0x2bb6b0;

struct Oracle {
    g: Rng,
    ev: [u32; 2],
    log: Vec<(u32, Vec<u32>)>,
}
impl Oracle {
    fn id(&mut self) -> u32 {
        let r = self.g.below(K + 1);
        if r == K {
            0xffff_ffff
        } else {
            r
        }
    }
    fn call(&mut self, m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        let mut a = args.to_vec();
        if t == CB0T {
            for k in 0..7 {
                a.push(m.r32(args[0] + 4 * k));
            }
        }
        self.log.push((t, a));
        match t {
            TEST_BIT_A | BANK_HAS => self.g.below(2),
            BANK_ID | BANK_SLOT => self.id(),
            FIND_EVENT_T => [0, self.ev[0], self.ev[1]][self.g.below(3) as usize],
            KIND => {
                let v = self.g.next();
                m.w32(args[1], v);
                self.g.below(2)
            }
            EVENT_WORD => {
                let f = self.g.below(4) != 0;
                if f {
                    let v = self.g.below(8);
                    m.w32(args[1], v);
                }
                f as u32
            }
            BANK_SIZE => self.g.below(0x300),
            CB0T | CB2T => 0,
            CB1T => [0, 1, 0xffff_ffff, 5][self.g.below(4) as usize],
            CB4T => [0xffff_ffff, 0, 1, 2][self.g.below(4) as usize],
            _ => unreachable!(),
        }
    }
}
struct PortEnv(Rc<RefCell<Oracle>>);
impl LoadEnv for PortEnv {
    fn call(&mut self, m: &mut dyn Mem, target: u32, args: &[u32]) -> u32 {
        assert_eq!(args.len(), nargs(target), "argument count of {target:#x}");
        self.0.borrow_mut().call(m, target, args)
    }
}

struct W {
    r: Runner,
    h: [u32; 4],
    ev: [u32; 2],
    e: u32,
    ctx: u32,
    out: u32,
    bt: u32,
    b: u32,
    oracle: Rc<RefCell<Oracle>>,
}

const G1: (u32, u32) = (0x348930, 0x20);
const QR: (u32, u32) = (0x348950, 0x470);
const CBR: (u32, u32) = (0x348dc0, 0x40);
const VOICE: (u32, u32) = (0x348e48, 0x400);
const R2: (u32, u32) = (0x3494a0, 0x80);
const RH: (u32, u32) = (0x349528, 0xa0);
const KEYS: u32 = 0x348ebc;

thread_local! { static FRAMES: RefCell<Vec<(u32, u32)>> = RefCell::new(vec![]); }

fn world(stubs: &[u32], allow: &[u32]) -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let h = [r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR)];
    let ev = [r.mem.alloc(EVSZ), r.mem.alloc(EVSZ)];
    let (e, ctx, out, bt, b) = (r.mem.alloc(HDR), r.mem.alloc(0x200), r.mem.alloc(0x30), r.mem.alloc(0x40), r.mem.alloc(0x440));
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), ev, log: vec![] }));
    for &t in stubs {
        let l = oracle.clone();
        let n = nargs(t);
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            let v = l.borrow_mut().call(&mut RM2(m), t, &args[..n]);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    for &a in allow {
        r.allow(a);
    }
    Some(W { r, h, ev, e, ctx, out, bt, b, oracle })
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    let mut v = vec![G1, QR, CBR, VOICE, R2, RH, (w.e, HDR), (w.ctx, 0x200), (w.out, 0x30)];
    v.extend(w.h.iter().map(|&a| (a, HDR)));
    v.extend(w.ev.iter().map(|&a| (a, EVSZ)));
    FRAMES.with(|f| v.extend(f.borrow().iter().cloned()));
    v
}

fn put(o: &mut [u8], off: u32, v: &[u8]) {
    o[off as usize..off as usize + v.len()].copy_from_slice(v);
}

fn bank(g: &mut Rng) -> Vec<u8> {
    let mut o: Vec<u8> = (0..HDR).map(|_| g.next() as u8).collect();
    let n = g.below(5);
    let keys = g.below(4);
    let flag = g.below(2);
    let rows = g.below(11);
    o[3] = g.next() as u8;
    o[4] = (n | keys << 4 | flag << 7) as u8;
    o[5] = rows as u8;
    o[6] = [0u32, 1, 3, g.below(21), g.below(21)][g.below(5) as usize] as u8;
    o[7] = g.below(4) as u8;
    put(&mut o, 8, &(g.below(4000) as u16).to_le_bytes());
    put(&mut o, 0xa, &(g.below(5) as u16).to_le_bytes());
    for k in 0..rows {
        let at = (0xc + k * (n + 2)) as usize;
        let alias = g.below(4) == 0;
        o[at] = if alias { 0x80 | g.below(0x80) as u8 } else { g.below(0x80) as u8 };
        o[at + 1] = if alias { g.below(rows) as u8 } else { g.next() as u8 };
        for j in 0..n {
            o[at + 2 + j as usize] = if g.below(8) == 0 { g.next() as u8 } else { g.below(0x20) as u8 };
        }
    }
    o
}

fn event(g: &mut Rng, maxparts: u32) -> Vec<u8> {
    let mut o: Vec<u8> = (0..HDR).map(|_| g.next() as u8).collect();
    let parts = match g.below(10) {
        0 => 0,
        1..=6 => 1 + g.below(4),
        7 | 8 => 5 + g.below(8),
        _ => 13 + g.below(2),
    }
    .min(maxparts);
    o[1] = g.below(4) as u8;
    o[2] = g.below(4) as u8;
    o[3] = (parts << 2 | g.below(4)) as u8;
    let mut ptr = vec![];
    for i in 0..parts {
        let v = 8 * (1 + g.below(13));
        o[4 + i as usize] = v as u8;
        ptr.push(4 * v);
    }
    for p in ptr {
        put(&mut o, p, &(g.below(K + 2) as u16).to_le_bytes());
        o[p as usize + 2] = if g.below(3) == 0 { g.below(0x30) as u8 } else { g.below(0x14) as u8 };
        o[p as usize + 3] = g.below(4) as u8;
        o[p as usize + 4] = [0u8, 1, 2, 3, 4, 0x80][g.below(6) as usize];
        for j in 0..4 {
            o[p as usize + 8 + j] = [0u8, 0xff, 0xfe, g.below(8) as u8, g.below(0x30) as u8][g.below(5) as usize];
            o[p as usize + 0xc + j] = if g.below(3) == 0 { 0xff } else { g.below(10) as u8 };
            let w = if g.below(4) == 0 { g.next() } else { 0xffff_ffff };
            put(&mut o, p + 0x10 + 4 * j as u32, &w.to_le_bytes());
        }
    }
    o
}

/// An event header: rows (their headers further in the blob), records, the parameter word and the row masks.
fn evblob(g: &mut Rng) -> (Vec<u8>, u32) {
    let mut o: Vec<u8> = (0..EVSZ).map(|_| g.next() as u8).collect();
    let rows = 1 + g.below(3);
    let n7 = g.below(7);
    put(&mut o, 0, &(g.below(4) as u16).to_le_bytes());
    o[6] = rows as u8;
    o[7] = n7 as u8;
    o[8] = [0u8, 1, 2, 0xfe, 0x80][g.below(5) as usize];
    for k in 0..rows {
        put(&mut o, 0xc + 2 * k, &((0x80 * (k + 1)) as u16).to_le_bytes());
    }
    let rec0 = ((rows * 2 + 3) & 0x3fc) + 0xc;
    for r in 0..n7 {
        let at = (rec0 + 3 * r) as usize;
        o[at] = g.below(4) as u8;
        o[at + 1] = if g.below(5) == 0 { 0 } else { 1 + g.below(0x13) as u8 };
        o[at + 2] = g.below(6) as u8;
    }
    let w = rec0 + ((n7 * 3 + 3) & !3);
    let bits: u32 = (0..16).fold(0, |a, _| a << 2 | [0u32, 1, 1, 2, 3][g.below(5) as usize]) | (g.below(4) << 30);
    put(&mut o, w, &bits.to_le_bytes());
    for k in 0..rows {
        let v = if g.below(3) == 0 { g.below(0x10000) as u16 } else { 0 };
        put(&mut o, w + 4 + 2 * k, &v.to_le_bytes());
    }
    for k in 0..rows {
        let r = event(g, 4);
        put(&mut o, 0x200 * (k + 1), &r);
    }
    (o, rows)
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng, maxparts: u32) {
    sm.0.clear();
    let rnd = |n: u32, g: &mut Rng| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    for k in 0..K as usize {
        img.push((w.h[k], bank(g)));
    }
    img.push((w.e, event(g, maxparts)));
    let mut rows = [0u32; 2];
    for k in 0..2 {
        let (o, r) = evblob(g);
        rows[k] = r;
        img.push((w.ev[k], o));
    }
    let mut ctx = rnd(0x200, g);
    for i in 0..14u32 {
        put(&mut ctx, 8 * i, &(g.below(K) as u16).to_le_bytes());
        ctx[(8 * i + 4) as usize] = g.below(12) as u8;
        ctx[(8 * i + 5) as usize] = g.below(6) as u8;
        ctx[(8 * i + 6) as usize] = g.below(14) as u8;
    }
    ctx[0x60] = g.below(14) as u8;
    ctx[0x61] = [g.below(0x66), g.below(4), 0x63, 0x64][g.below(4) as usize] as u8;
    for k in 0..0x80 {
        ctx[0x62 + k] = g.below(12) as u8;
    }
    img.push((w.ctx, ctx));
    img.push((w.out, rnd(0x30, g)));
    let mut bt = rnd(0x40, g);
    for k in 0..K as usize {
        put(&mut bt, 4 * k as u32, &w.h[k].to_le_bytes());
    }
    img.push((w.bt, bt));
    let mut b = rnd(0x440, g);
    for k in 0..0x110usize {
        if g.below(5) != 0 {
            put(&mut b, 4 * k as u32, &0xffff_ffffu32.to_le_bytes());
        }
    }
    img.push((w.b, b));
    let mut g1 = vec![];
    for _ in 0..8 {
        g1.extend((if g.below(5) == 0 { 0 } else { g.below(6) }).to_le_bytes());
    }
    img.push((G1.0, g1));
    // the library's tables
    let mut r2 = rnd(R2.1, g);
    for k in 0..8u32 {
        put(&mut r2, 0x10 + 8 * k, &(g.below(3) as u16).to_le_bytes());
        put(&mut r2, 0x14 + 8 * k, &g.below(4).to_le_bytes());
        put(&mut r2, 0x58 + 4 * k, &g.below(4).to_le_bytes());
    }
    put(&mut r2, 0x54, &(if g.below(6) == 0 { 0 } else { 1 + g.below(300) }).to_le_bytes());
    img.push((R2.0, r2));
    let mut cbr = rnd(CBR.1, g);
    put(&mut cbr, 0xc, &w.bt.to_le_bytes());
    put(&mut cbr, 0x18, &g.below(2).to_le_bytes());
    for (off, t) in [(0x20u32, CB0T), (0x24, CB1T), (0x28, CB2T), (0x30, CB4T)] {
        put(&mut cbr, off, &(if g.below(6) == 0 { 0 } else { t }).to_le_bytes());
    }
    put(&mut cbr, 0, &g.below(3).to_le_bytes());
    img.push((CBR.0, cbr));
    let mut vo = rnd(VOICE.1, g);
    put(&mut vo, 0x60, &[0u32, 1, 1, 2][g.below(4) as usize].to_le_bytes());
    img.push((VOICE.0, vo));
    let mut rh = rnd(RH.1, g);
    put(&mut rh, 0x18, &g.below(32).to_le_bytes());
    img.push((RH.0, rh));
    // the queue: entries (bank id, halfword, row) and the records
    let mut q = rnd(QR.1, g);
    put(&mut q, 0, &g.below(8).to_le_bytes());
    put(&mut q, 4, &[g.below(4), g.below(4), 0, g.below(9)][g.below(4) as usize].to_le_bytes());
    put(&mut q, 8, &g.below(30).to_le_bytes());
    for k in 0..48u32 {
        let at = 0xc + 8 * k;
        put(&mut q, at, &(g.below(K) as u16).to_le_bytes());
        put(&mut q, at + 2, &[0xffffu16, 0, 1, 2][g.below(4) as usize].to_le_bytes());
        put(&mut q, at + 4, &(g.below(6) as u16).to_le_bytes());
    }
    for k in 0..8u32 {
        let at = 0x18c + 0x5c * k;
        let which = g.below(2) as usize;
        let rw = g.below(rows[which]);
        put(&mut q, at, &w.ev[which].to_le_bytes());
        put(&mut q, at + 4, &(w.ev[which] + 0x200 * (rw + 1)).to_le_bytes());
        q[(at + 8) as usize] = rw as u8;
        q[(at + 9) as usize] = 1;
    }
    img.push((QR.0, q));
    FRAMES.with(|f| {
        for &(a, n) in f.borrow().iter() {
            img.push((a, rnd(n, g)));
        }
    });
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn check(w: &mut W, sm: &mut SM, case: u32, seed: u64, addr: u32, set: &dyn Fn(&mut Runner), port: &dyn Fn(&mut SM, &mut PortEnv) -> u32, ret: bool) {
    {
        let mut o = w.oracle.borrow_mut();
        o.g = Rng(seed);
        o.log.clear();
    }
    set(&mut w.r);
    w.r.call(addr).unwrap();
    let theirs_calls = std::mem::take(&mut w.oracle.borrow_mut().log);
    let mut theirs = vec![];
    for (a, n) in regions(w) {
        let mut b = vec![0u8; n as usize];
        w.r.mem.read(a, &mut b).unwrap();
        theirs.extend(b);
    }
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), ev: w.ev, log: vec![] }));
    let mut env = PortEnv(mine.clone());
    let my_ret = port(sm, &mut env);
    let their_ret = w.r.cpu.gpr(2) as u32;
    let my_calls = std::mem::take(&mut mine.borrow_mut().log);
    for i in 0..my_calls.len().min(theirs_calls.len()) {
        assert_eq!(my_calls[i].0, theirs_calls[i].0, "case {case}: call {i}");
        assert_eq!(my_calls[i].1[..], theirs_calls[i].1[..my_calls[i].1.len()], "case {case}: args of call {i} ({:#x})", my_calls[i].0);
    }
    assert_eq!(my_calls.len(), theirs_calls.len(), "case {case}: number of calls");
    if ret {
        assert_eq!(my_ret, their_ret, "case {case}: return");
    }
    let mut mymem = vec![];
    for (a, n) in regions(w) {
        mymem.extend((a..a + n).map(|x| sm.r8(x)));
    }
    if mymem != theirs {
        let at = mymem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
        let mut left = at;
        let mut which = (0, 0);
        for (a, n) in regions(w) {
            if left < n as usize {
                which = (a, left);
                break;
            }
            left -= n as usize;
        }
        panic!("case {case}: memory differs at {:#x}+{:#x}: port {:#x} game {:#x}", which.0, which.1, mymem[at], theirs[at]);
    }
}

/// Mutation runs go faster with `FAST=1`: a quarter of the cases.
fn div() -> u32 {
    if std::env::var("FAST").is_ok() {
        4
    } else {
        1
    }
}
fn cases(n: u32) -> u32 {
    n / div()
}

fn frames(v: &[(u32, u32)]) {
    FRAMES.with(|f| *f.borrow_mut() = v.to_vec());
}

// every library function the code under test reaches, to run as the game's
const LIB: [u32; 46] = [
    0x2bea20, 0x2bea48, 0x2bc7a0, 0x2bca80, 0x2bc9d8, 0x2bb370, 0x2bccd0, 0x2bcdb0, 0x2bcf90, 0x2be650, 0x2bc850, 0x2bcb30,
    0x2bcd08, 0x2bcdd8, 0x2bd078, 0x2bd218, 0x2bd2e8, 0x2bd370, 0x2bd430, 0x2bd6a8, 0x2bd798, 0x2bd928, 0x2bdc98, 0x2bde40,
    0x2bddf0, 0x2be208, 0x2be218, 0x2be2b0, 0x2be310, 0x2be468, 0x2be908, 0x2be678, 0x2be880, 0x2be7d8, 0x2bc788, 0x2bc750,
    0x2bc768, 0x2bd198, 0x2bb2d8, 0x2bc938, 0x2bdc70, 0x2bdae0, 0x2bdad0, 0x2bdb68, 0x2bcfd8, 0x2bcf48,
];

/// A random event header (pointer) from the world: 0 or `which` 1 then the row header k of it.
fn evhdr(w: &W, g: &mut Rng) -> u32 {
    w.ev[g.below(2) as usize]
}

#[test]
fn small_functions_match() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(21), SM::default());
    frames(&[]);
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let (ev, e) = (evhdr(&w, &mut g), w.e);
        let i = g.below(10);
        check(&mut w, &mut sm, case, seed, 0x2be2b0, &|r| {
            r.cpu.set_gpr(4, ev as u64);
            r.cpu.set_gpr(5, i as u64);
        }, &|sm, _| get_record(sm, ev, i), true);
        let val = [g.below(0x14), g.below(6), g.below(4), g.below(0x30)][g.below(4) as usize];
        check(&mut w, &mut sm, case, seed, 0x2be218, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, val as u64);
        }, &|sm, _| has_value(sm, e, val), true);
        let (bits, v, k) = (g.next() | g.next() << 16, [0u32, 1, 0xffff_ffff, g.next()][g.below(4) as usize], g.below(0x14));
        check(&mut w, &mut sm, case, seed, 0x2bddf0, &|r| {
            r.cpu.set_gpr(4, bits as i32 as i64 as u64);
            r.cpu.set_gpr(5, v as i32 as i64 as u64);
            r.cpu.set_gpr(6, k as u64);
        }, &|_, _| param_test(bits, v, k), true);
        let clk = g.next();
        check(&mut w, &mut sm, case, seed, 0x2be208, &|r| r.cpu.set_gpr(4, clk as i32 as i64 as u64), &|sm, _| {
            set_clock_base(sm, clk);
            0
        }, false);
    }
}

#[test]
fn record_callback_matches() {
    let Some(mut w) = world(&[KIND, CB2T], &LIB) else { return };
    let (mut g, mut sm) = (Rng(22), SM::default());
    frames(&[(SP0 - 0xc0, 0x14)]);
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let (ctx, tbl) = (w.ctx, w.b);
        let which = g.below(2) as usize;
        let ev = w.ev[which];
        let rows = sm.r8(ev + 6) as u32;
        let i = g.below(rows);
        check(&mut w, &mut sm, case, seed, 0x2be310, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, ev as u64);
            r.cpu.set_gpr(6, i as u64);
            r.cpu.set_gpr(7, tbl as u64);
        }, &|sm, env| {
            record_callback(sm, env, SP0 - 0xc0, ctx, ev, i, tbl);
            0
        }, false);
    }
}

#[test]
fn param_masks_matches() {
    let Some(mut w) = world(&[KIND, CB1T], &LIB) else { return };
    let (mut g, mut sm) = (Rng(23), SM::default());
    frames(&[(SP0 - 0x120, 0xc), (SP0 - 0x120 + 0x10, 4)]);
    let mut nonzero = 0;
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let (ctx, out) = (w.ctx, w.out);
        let ev = evhdr(&w, &mut g);
        let arr = KEYS + 0x60 * g.below(8);
        check(&mut w, &mut sm, case, seed, 0x2be468, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, ev as u64);
            r.cpu.set_gpr(6, arr as u64);
            r.cpu.set_gpr(7, out as u64);
        }, &|sm, env| param_masks(sm, env, SP0 - 0x120, ctx, ev, arr, out), true);
        nonzero += (w.r.cpu.gpr(2) as u32 != 0) as u32;
    }
    assert!(nonzero > 1000 / div(), "{nonzero}");
}

#[test]
fn fire_params_matches() {
    let Some(mut w) = world(&[KIND, CB2T], &LIB) else { return };
    let (mut g, mut sm) = (Rng(24), SM::default());
    frames(&[(SP0 - 0xd0, 0x14)]);
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        let key = evhdr(&w, &mut g);
        let base = g.below(30);
        check(&mut w, &mut sm, case, seed, 0x2bd798, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, key as u64);
            r.cpu.set_gpr(6, e as u64);
            r.cpu.set_gpr(7, base as u64);
        }, &|sm, env| {
            fire_params(sm, env, SP0 - 0xd0, ctx, key, e, base);
            0
        }, false);
    }
}

#[test]
fn play_rows_matches() {
    let Some(mut w) = world(&[KIND, EVENT_WORD, CB0T], &LIB) else { return };
    let (mut g, mut sm) = (Rng(25), SM::default());
    frames(&[(SP0 - 0xe0, 0x38)]);
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let e = w.e;
        let key = KEYS + 0x60 * g.below(8);
        let base = g.below(30);
        check(&mut w, &mut sm, case, seed, 0x2bd928, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, key as u64);
            r.cpu.set_gpr(6, base as u64);
        }, &|sm, env| play_rows(sm, env, SP0 - 0xe0, e, key, base) as u32, true);
    }
}

#[test]
fn select_row_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(26), SM::default());
    frames(&[(SP0 - 0xf0, 0x10)]);
    for case in 0..cases(40_000) {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        let ent = ctx + 8 * g.below(12);
        let nparts = (sm.r8(e + 3) >> 2) as u32;
        if nparts == 0 {
            continue;
        }
        let part = e + 4 * sm.r8(e + 4 + g.below(nparts)) as u32;
        check(&mut w, &mut sm, case, seed, 0x2bd430, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, part as u64);
            r.cpu.set_gpr(6, ent as u64);
        }, &|sm, _| select_row(sm, SP0 - 0xf0, ctx, part, ent), true);
    }
}

#[test]
fn select_rows_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(27), SM::default());
    let mut yes = 0;
    for case in 0..cases(30_000) {
        let mode = [0u32, 1, 1, 2][g.below(4) as usize];
        // the callee of a callee spills its registers over the frame of the choice in mode 1
        if mode == 1 {
            frames(&[(SP0 - 0x80 - 0x50 - 0x70, 0x10)]);
        } else {
            frames(&[(SP0 - 0x80 - 0xf0, 0x10)]);
        }
        image(&mut w, &mut sm, &mut g, 3);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        if mode == 1 && sm.r8(e + 3) >> 2 == 0 {
            continue; // no parts: the combination search never ends, in the game too
        }
        let thr = [0u32, 1, 20, 50, 100, 150][g.below(6) as usize];
        check(&mut w, &mut sm, case, seed, 0x2bd6a8, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, e as u64);
            r.cpu.set_gpr(6, mode as u64);
            r.cpu.set_gpr(7, thr as u64);
        }, &|sm, env| select_rows(sm, env, SP0 - 0x80, ctx, e, mode, thr), true);
        yes += w.r.cpu.gpr(2) as u32;
    }
    assert!(yes > 1000 / div(), "{yes}");
}

#[test]
fn queue_run_matches() {
    let Some(mut w) = world(&[KIND, EVENT_WORD, CB0T, CB2T], &LIB) else { return };
    let (mut g, mut sm) = (Rng(28), SM::default());
    frames(&[(SP0 - 0xb0, 4)]);
    let mut played = 0;
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 6);
        let seed = g.next() as u64 | 1;
        let id = if g.below(4) != 0 { sm.r32(QR.0) } else { g.below(8) };
        check(&mut w, &mut sm, case, seed, 0x2bdc98, &|r| r.cpu.set_gpr(4, id as u64), &|sm, env| queue_run(sm, env, SP0 - 0xb0, id), true);
        played += (w.r.cpu.gpr(2) as u32 != 0) as u32;
    }
    assert!(played > 1000 / div(), "{played}");
}

const START_STUBS: [u32; 9] = [FIND_EVENT_T, EVENT_WORD, BANK_SIZE, KIND, CB1T, TEST_BIT_A, BANK_ID, BANK_SLOT, BANK_HAS];

#[test]
fn start_rec_matches() {
    let Some(mut w) = world(&START_STUBS, &LIB) else { return };
    let (mut g, mut sm) = (Rng(29), SM::default());
    frames(&[(SP0 - 0x210, 0x170)]);
    let mut started = 0;
    for case in 0..cases(40_000) {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let keyp = KEYS + 0x60 * g.below(8);
        check(&mut w, &mut sm, case, seed, 0x2bde40, &|r| r.cpu.set_gpr(4, keyp as u64), &|sm, env| start_rec(sm, env, SP0 - 0x210, keyp), true);
        started += (w.r.cpu.gpr(2) as u32 == 1) as u32;
    }
    assert!(started > 500 / div(), "{started}");
}

#[test]
fn start_entry_matches() {
    let mut stubs = START_STUBS.to_vec();
    stubs.push(CB4T);
    let Some(mut w) = world(&stubs, &LIB) else { return };
    let (mut g, mut sm) = (Rng(30), SM::default());
    frames(&[(SP0 - 0x60 - 0x210, 0x170)]);
    for case in 0..cases(30_000) {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let i = g.below(8);
        check(&mut w, &mut sm, case, seed, 0x2be140, &|r| r.cpu.set_gpr(4, i as u64), &|sm, env| start_entry(sm, env, SP0 - 0x60, i), true);
    }
}

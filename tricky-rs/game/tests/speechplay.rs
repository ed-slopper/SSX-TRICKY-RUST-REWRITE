//! `tricky_game::speechplay` (board row G10m) against the crowd box functions in the function runner (TRICKY_ELF;
//! skipped without it): every callee (the ini reader, strcmp, sprintf, the archive and bank loaders) is a stub that
//! answers from the same random stream on both sides and writes strings into the frame the way the real ones do; the
//! calls (address and arguments), the frame and the speech object are compared.

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

/// The runner's memory as a [`Mem`].
struct RM<'a>(&'a mut r5900::Mem);
impl Mem for RM<'_> {
    fn r8(&self, _: u32) -> u8 {
        unreachable!()
    }
    fn r16(&self, _: u32) -> u16 {
        unreachable!()
    }
    fn r32(&self, _: u32) -> u32 {
        unreachable!()
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



use std::cell::Cell;

const T1: u32 = 0x90_0000;
const TARGETS: [u32; 7] = [POST_EVENT, RELEASE_TAIL, BANK_ID, BANK_SLOT, BANK_HAS, TEST_BIT, T1];

struct Oracle {
    g: Rng,
    words: Rc<Cell<u32>>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Oracle {
    fn call(&mut self, m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(TARGETS.contains(&t), "target {t:#x}");
        let mut a = args.to_vec();
        if t == POST_EVENT {
            a = (0..=self.words.get()).map(|k| m.r32(args[0] + 4 * k)).collect();
            a.insert(0, args[0]);
        }
        self.log.push((t, a));
        match t {
            BANK_HAS => self.g.below(2),
            TEST_BIT => [0, 0x10, 0xff, 1][self.g.below(4) as usize],
            _ => self.g.next(),
        }
    }
}
struct PortEnv(Rc<RefCell<Oracle>>);
impl LoadEnv for PortEnv {
    fn call(&mut self, m: &mut dyn Mem, target: u32, args: &[u32]) -> u32 {
        self.0.borrow_mut().call(m, target, args)
    }
}

// arenas
const R1: (u32, u32) = (0x348900, 0xb80);
const R2: (u32, u32) = (0x3494a0, 0xa0);
struct W {
    r: Runner,
    hdr: [u32; 3],
    list: u32,
    out: u32,
    tbl: u32,
    banks: u32,
    words: Rc<Cell<u32>>,
    frame: bool,
    oracle: Rc<RefCell<Oracle>>,
}
const HDR: u32 = 0x400;

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(TEST_ENTRY);
    let hdr = [r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR)];
    let (list, out, tbl, banks) = (r.mem.alloc(0x60), r.mem.alloc(0x10), r.mem.alloc(0x400), r.mem.alloc(0x100));
    let words = Rc::new(Cell::new(0));
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), words: words.clone(), log: vec![] }));
    for t in TARGETS {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            let n = match t {
                POST_EVENT | BANK_ID => 1,
                RELEASE_TAIL => 0,
                BANK_SLOT | BANK_HAS | TEST_BIT => 2,
                _ => 1,
            };
            let v = l.borrow_mut().call(&mut RM2(m), t, &args[..n]);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, hdr, list, out, tbl, banks, words, frame: false, oracle })
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng) {
    sm.0.clear();
    let rnd = |n: u32, g: &mut Rng| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    let mut r1 = rnd(R1.1, g);
    let mut set = |o: &mut Vec<u8>, base: u32, off: u32, v: u32| {
        let a = (off - base) as usize;
        o[a..a + 4].copy_from_slice(&v.to_le_bytes());
    };
    set(&mut r1, R1.0, 0x348dcc, w.banks);
    set(&mut r1, R1.0, 0x348dec, if g.below(2) == 0 { 0 } else { T1 });
    for k in 0..8 {
        set(&mut r1, R1.0, 0x348e48 + 4 * k, g.below(4));
    }
    for i in 0..16 {
        let rec = 0x348e48 + 0x60 * i;
        r1[(rec + 0x6a - R1.0) as usize] = g.below(2) as u8;
        r1[(rec + 0x6b - R1.0) as usize] = g.below(8) as u8;
    }
    img.push((R1.0, r1));
    let mut r2 = rnd(R2.1, g);
    for k in 0..8 {
        let a = (0x3494b0 + 8 * k - R2.0) as usize;
        let id = g.below(3) as u16;
        r2[a..a + 2].copy_from_slice(&id.to_le_bytes());
        set(&mut r2, R2.0, 0x3494b0 + 8 * k + 4, g.below(5));
    }
    set(&mut r2, R2.0, 0x3494f4, [0, 7, 100, 0xffff_ffff][g.below(4) as usize]);
    img.push((R2.0, r2));
    for h in w.hdr {
        let mut o = rnd(HDR, g);
        o[5] = g.below(12) as u8;
        o[4] = (g.next() as u8 & 0xf0) | g.below(6) as u8;
        o[3] = g.below(4) as u8;
        o[2] = g.below(0x16) as u8;
        o[0..2].copy_from_slice(&(g.below(3) as u16).to_le_bytes());
        o[8] = g.below(6) as u8;
        o[10..12].copy_from_slice(&(g.below(6) as u16).to_le_bytes());
        o[6] = g.below(3) as u8;
        img.push((h, o));
    }
    let mut list = rnd(0x60, g);
    set(&mut list, w.list, w.list, g.below(6));
    set(&mut list, w.list, w.list + 4, w.list + 8);
    for k in 0..4 {
        let a = (8 + 2 * k) as usize;
        list[a..a + 2].copy_from_slice(&(g.below(4) as u16).to_le_bytes());
    }
    img.push((w.list, list));
    img.push((w.out, rnd(0x10, g)));
    let mut tbl = rnd(0x400, g);
    for k in 0..16u32 {
        let a = (4 * k) as usize;
        tbl[a..a + 4].copy_from_slice(&(if g.below(3) == 0 { 0 } else { g.next() }).to_le_bytes());
    }
    img.push((w.tbl, tbl));
    let mut banks = rnd(0x100, g);
    for k in 0..8u32 {
        set(&mut banks, w.banks, w.banks + 4 * k, w.hdr[g.below(3) as usize]);
    }
    img.push((w.banks, banks));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    let mut v = vec![];
    if w.frame {
        v.push((0x01fe_ff30, 0x20));
    }
    v.extend([R1, R2, (w.hdr[0], HDR), (w.hdr[1], HDR), (w.hdr[2], HDR), (w.list, 0x60), (w.out, 0x10), (w.tbl, 0x400), (w.banks, 0x100)]);
    v
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
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), words: w.words.clone(), log: vec![] }));
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
        panic!("case {case}: memory differs at +{at:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}



const SP0: u32 = 0x01ff_0000;

fn poke(w: &mut W, sm: &mut SM, a: u32, bytes: &[u8]) {
    w.r.mem.write(a, bytes).unwrap();
    for (k, x) in bytes.iter().enumerate() {
        sm.0.insert(a + k as u32, *x);
    }
}

#[test]
fn helpers_match() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(2), SM::default());
    for case in 0..60_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let (h0, h1, h2) = (w.hdr[0], w.hdr[1], w.hdr[2]);
        let (list, out, tbl) = (w.list, w.out, w.tbl);
        let (x, y) = (g.next(), g.next());
        let (idx, bank, small) = (if g.below(2) == 0 { g.below(12) as i32 - 2 } else { g.below(60) as i32 - 30 }, g.below(8), g.below(4));
        let n = g.below(7);
        let rg = [x, y, g.next(), g.next(), g.next(), g.next()];
        let which = g.below(17);
        let case = case * 100 + which;
        w.words.set(n);
        w.frame = which == 0;
        if which == 0 {
            poke(&mut w, &mut sm, 0x01fe_ff30, &[0u8; 0x20]);
        }
        if which == 7 || which == 10 || which == 11 {
            let n4 = [0xffu8, 0, 1, 2, 3, 4, 5, 6, 0x83][g.below(9) as usize];
            poke(&mut w, &mut sm, h1 + 4, &[n4]);
            if g.below(2) == 0 {
                let rowv: Vec<u8> = (0..8).map(|_| [0u8, 0xff, 0xfe, 1, 2, 3, 0x20][g.below(7) as usize]).collect();
                poke(&mut w, &mut sm, h1 + 8, &rowv[..4]);
                let masks: Vec<u8> = (0..16).flat_map(|_| if g.below(3) == 0 { vec![0, 0, 0, 0] } else if g.below(2) == 0 { vec![0xff; 4] } else { g.next().to_le_bytes().to_vec() }).collect();
                poke(&mut w, &mut sm, h1 + 0x10, &masks);
                let bits: Vec<u8> = (0..4).map(|_| [0u8, 1, 5, 0x1f, 0x20, 0x21][g.below(6) as usize]).collect();
                poke(&mut w, &mut sm, h0 + 0xc, &bits);
                let t: Vec<u8> = (0..16).flat_map(|_| if g.below(2) == 0 { vec![0xff; 4] } else { g.next().to_le_bytes().to_vec() }).collect();
                poke(&mut w, &mut sm, tbl, &t);
            }
            if which == 7 && g.below(3) == 0 {
                let n5 = [3u8, 4, 5, 5][g.below(4) as usize];
                poke(&mut w, &mut sm, h1 + 4, &[n5]);
                poke(&mut w, &mut sm, h1 + 8, &[0, 0xff, 0xfe, 0, 0xff]);
                poke(&mut w, &mut sm, h1 + 0x10, &[0xff; 0x14]);
                poke(&mut w, &mut sm, h0 + 0xc, &[1, 2, 3, 4, 5]);
            }
            if which != 7 && g.below(3) == 0 {
                poke(&mut w, &mut sm, h1 + 2, &[0xff, 0xff]);
            }
        }
        match which {
            0 => check(&mut w, &mut sm, case, seed, 0x2bbcf8, &|r| {
                r.cpu.set_gpr(4, x as u64);
                r.cpu.set_gpr(5, n as u64);
                for (k, v) in rg.iter().enumerate() {
                    r.cpu.set_gpr(6 + k, *v as u64);
                }
            }, &|sm, env| {
                post_event_args(sm, env, SP0 - 0xd0, x, n as i32, &rg);
                0
            }, false),
            1 => check(&mut w, &mut sm, case, seed, 0x2bbd68, &|r| r.cpu.set_gpr(4, h0 as u64), &|sm, _| {
                clear_pair(sm, h0);
                0
            }, false),
            2 => check(&mut w, &mut sm, case, seed, 0x2bbd78, &|r| {
                r.cpu.set_gpr(4, small as u64);
                r.cpu.set_gpr(5, list as u64);
            }, &|sm, _| list_contains(sm, small, list) as u32, true),
            3 => check(&mut w, &mut sm, case, seed, 0x2bc170, &|_| {}, &|sm, env| {
                release_all(sm, env);
                0
            }, false),
            4 => check(&mut w, &mut sm, case, seed, 0x2bc1e8, &|r| r.cpu.set_gpr(4, bank as u64), &|sm, _| group_count(sm, bank), true),
            5 => check(&mut w, &mut sm, case, seed, 0x2bc490, &|r| r.cpu.set_gpr(4, x as u64), &|sm, env| callback_check(sm, env, x), true),
            6 => check(&mut w, &mut sm, case, seed, 0x2bc720, &|r| {
                r.cpu.set_gpr(4, x as u64);
                r.cpu.set_gpr(5, bank as u64);
            }, &|sm, _| {
                set_slot(sm, x, bank);
                0
            }, false),
            7 => check(&mut w, &mut sm, case, seed, 0x2bc7a0, &|r| {
                r.cpu.set_gpr(5, h0 as u64);
                r.cpu.set_gpr(6, h1 as u64);
                r.cpu.set_gpr(7, tbl as u64);
            }, &|sm, _| masks_agree(sm, h0, h1, tbl) as u32, true),
            8 => check(&mut w, &mut sm, case, seed, 0x2bc850, &|r| {
                r.cpu.set_gpr(4, h0 as u64);
                r.cpu.set_gpr(5, tbl as u64);
                r.cpu.set_gpr(6, out as u64);
            }, &|sm, env| lookup_ids(sm, env, h0, tbl, out) as u32, true),
            9 => check(&mut w, &mut sm, case, seed, 0x2bc938, &|r| {
                r.cpu.set_gpr(4, h0 as u64);
                r.cpu.set_gpr(5, idx as i64 as u64);
            }, &|sm, _| {
                clear_row_bit(sm, h0, idx);
                0
            }, false),
            10 => check(&mut w, &mut sm, case, seed, 0x2bc9d8, &|r| {
                r.cpu.set_gpr(4, h1 as u64);
                r.cpu.set_gpr(5, h0 as u64);
                r.cpu.set_gpr(6, idx as i64 as u64);
            }, &|sm, env| test_entry(sm, env, h1, h0, idx), true),
            11 => check(&mut w, &mut sm, case, seed, 0x2bca80, &|r| {
                r.cpu.set_gpr(4, h1 as u64);
                r.cpu.set_gpr(5, h0 as u64);
                r.cpu.set_gpr(6, idx as i64 as u64);
            }, &|sm, env| row_allowed(sm, env, h1, h0, idx), true),
            12 => check(&mut w, &mut sm, case, seed, 0x2bccd0, &|r| r.cpu.set_gpr(4, x as i32 as i64 as u64), &|sm, _| percent(sm, x as i32) as u32, true),
            13 => check(&mut w, &mut sm, case, seed, 0x2bcdb0, &|r| r.cpu.set_gpr(4, x as u64), &|sm, _| weight(sm, x), true),
            14 => check(&mut w, &mut sm, case, seed, 0x2bcf48, &|r| {
                r.cpu.set_gpr(4, h0 as u64);
                r.cpu.set_gpr(5, bank as u64);
            }, &|sm, _| entry_ok(sm, h0, bank) as u32, true),
            15 => check(&mut w, &mut sm, case, seed, 0x2bcf90, &|r| {
                r.cpu.set_gpr(4, h0 as u64);
                r.cpu.set_gpr(5, small as u64);
            }, &|sm, _| mode_ok(sm, h0, small), true),
            _ => check(&mut w, &mut sm, case, seed, 0x2bcfd8, &|r| {
                r.cpu.set_gpr(4, bank as u64);
                r.cpu.set_gpr(5, small as u64);
            }, &|sm, _| find_key(sm, bank, small) as u32, true),
        }
            }
}

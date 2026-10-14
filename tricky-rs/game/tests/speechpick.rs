//! The voice picker of `tricky_game::speechplay` (board row G10m) against the crowd box functions in the function runner (TRICKY_ELF;
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

const T_ARGS: [(u32, usize); 18] = [
    (CLOCK, 0), (CONTAINS, 2), (FIND_EVENT_BANK, 2), (FREE_VOICE, 1), (CLEAR_PAIR, 1), (FIND_EVENT, 1), (FIND_VICTIM, 2),
    (PICK_VOICE, 1), (CLAIM, 1), (CALLBACK_CHECK, 1), (GROUP_LIMITS, 1), (CLEAR_QUEUE, 0), (START_ONE, 1), (QUEUED_NOW, 1),
    (QUEUE_RUN, 1), (START_VOICE, 1), (TRY_START, 1), (0, 0),
];
fn nargs(t: u32) -> usize {
    T_ARGS.iter().find(|x| x.0 == t).map(|x| x.1).expect("target")
}

struct Oracle {
    g: Rng,
    hdr: [u32; 3],
    xptr: u32,
    log: Vec<(u32, Vec<u32>)>,
}
impl Oracle {
    fn call(&mut self, m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        let mut a = args.to_vec();
        if t == FIND_EVENT_BANK || t == CALLBACK_CHECK {
            a.push(m.r32(args[0]));
        }
        self.log.push((t, a));
        match t {
            CLOCK => self.g.below(2000),
            CONTAINS | GROUP_LIMITS | QUEUED_NOW | START_VOICE | START_ONE => self.g.below(2),
            FIND_EVENT_BANK => {
                let f = self.g.below(2);
                if f != 0 {
                    m.w32(args[1], self.xptr);
                }
                f
            }
            FIND_VICTIM | PICK_VOICE => self.g.below(18).wrapping_sub(2),
            CALLBACK_CHECK => [1, 1, 0, 2][self.g.below(4) as usize],
            FIND_EVENT => {
                if self.g.below(4) == 0 {
                    0
                } else {
                    self.hdr[self.g.below(3) as usize]
                }
            }
            _ => self.g.next(),
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

const R1: (u32, u32) = (0x348900, 0xb80);
const R2: (u32, u32) = (0x3494a0, 0xa0);
const HDR: u32 = 0x400;
struct W {
    r: Runner,
    hdr: [u32; 3],
    xptr: u32,
    bank: u32,
    stack: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world(targets: &[u32]) -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let hdr = [r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR)];
    let (xptr, bank, stack) = (r.mem.alloc(0x10), r.mem.alloc(0x40), r.mem.alloc(0x10));
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), hdr, xptr, log: vec![] }));
    for &t in targets {
        let l = oracle.clone();
        let n = nargs(t);
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            let v = l.borrow_mut().call(&mut RM2(m), t, &args[..n]);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, hdr, xptr, bank, stack, oracle })
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng) {
    sm.0.clear();
    let rnd = |n: u32, g: &mut Rng| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    let mut r1 = rnd(R1.1, g);
    let set = |o: &mut Vec<u8>, base: u32, off: u32, v: u32| {
        let a = (off - base) as usize;
        o[a..a + 4].copy_from_slice(&v.to_le_bytes());
    };
    let set16 = |o: &mut Vec<u8>, base: u32, off: u32, v: u16| {
        let a = (off - base) as usize;
        o[a..a + 2].copy_from_slice(&v.to_le_bytes());
    };
    for k in 0..8 {
        set(&mut r1, R1.0, 0x348e48 + 4 * k, g.below(3));
        set(&mut r1, R1.0, 0x348e88 + 4 * k, g.below(18));
    }
    set(&mut r1, R1.0, 0x348900, g.below(2));
    for i in 0..16 {
        let rec = 0x348e48 + 0x60 * i;
        set(&mut r1, R1.0, rec + 0x64, g.below(2000));
        set16(&mut r1, R1.0, rec + 0x68, g.below(4) as u16);
        r1[(rec + 0x6a - R1.0) as usize] = g.below(3).min(1) as u8;
        r1[(rec + 0x6b - R1.0) as usize] = g.below(3) as u8;
        set16(&mut r1, R1.0, rec + 0x6c, [0u16, 0, 0, 1][g.below(4) as usize]);
        set16(&mut r1, R1.0, rec + 0x6e, [0xffffu16, 0xffff, 3, 7, 20, g.below(16) as u16][g.below(6) as usize]);
        set(&mut r1, R1.0, rec + 0x70, w.hdr[g.below(3) as usize]);
    }
    img.push((R1.0, r1));
    let mut r2 = rnd(R2.1, g);
    for k in 0..8 {
        set16(&mut r2, R2.0, 0x3494b0 + 8 * k, [0xffffu16, 0, 1, 2][g.below(4) as usize]);
        set(&mut r2, R2.0, 0x3494f8 + 4 * k, [1, 1, 0, 2][g.below(4) as usize]);
    }
    set(&mut r2, R2.0, 0x349518, g.below(200));
    img.push((R2.0, r2));
    for h in w.hdr {
        let mut o = rnd(HDR, g);
        set16(&mut o, h, h, g.below(3) as u16);
        set16(&mut o, h, h + 2, [0u16, 5, 300, 1500][g.below(4) as usize]);
        set16(&mut o, h, h + 4, g.below(5) as u16);
        o[6] = g.below(40) as u8;
        o[7] = (g.below(23) as i32 - 2) as u8;
        o[9] = [0x64u8, 0x64, 0x63, g.next() as u8][g.below(4) as usize];
        o[0xa] = g.next() as u8;
        img.push((h, o));
    }
    let mut x = rnd(0x10, g);
    set(&mut x, w.xptr, w.xptr, w.bank);
    img.push((w.xptr, x));
    let mut bank = rnd(0x40, g);
    set16(&mut bank, w.bank, w.bank + 0x16, g.below(6) as u16);
    img.push((w.bank, bank));
    img.push((w.stack, rnd(0x10, g)));
    img.push((SP0 - 0x40, rnd(8, g)));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

thread_local! { static FRAME: std::cell::Cell<bool> = std::cell::Cell::new(false); }

fn regions(w: &W) -> Vec<(u32, u32)> {
    let f = if FRAME.with(|f| f.get()) { 8 } else { 0 };
    vec![(SP0 - 0x40, f), R1, R2, (w.hdr[0], HDR), (w.hdr[1], HDR), (w.hdr[2], HDR), (w.xptr, 0x10), (w.bank, 0x40)]
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
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), hdr: w.hdr, xptr: w.xptr, log: vec![] }));
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

#[test]
fn find_victim_matches() {
    let Some(mut w) = world(&[CLOCK, CONTAINS, FIND_EVENT_BANK, FREE_VOICE]) else { return };
    let (mut g, mut sm) = (Rng(2), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let (filter, group) = (if g.below(2) == 0 { 0 } else { w.hdr[1] }, g.below(4));
        check(&mut w, &mut sm, case, seed, FIND_VICTIM, &|r| {
            r.cpu.set_gpr(4, filter as u64);
            r.cpu.set_gpr(5, group as u64);
        }, &|sm, env| find_victim(sm, env, SP0 - 0xe0, filter, group) as u32, true);
    }
}

#[test]
fn pick_voice_matches() {
    let Some(mut w) = world(&[CLEAR_PAIR, FIND_EVENT, FIND_VICTIM]) else { return };
    let (mut g, mut sm) = (Rng(3), SM::default());
    FRAME.with(|f| f.set(true));
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let group = g.below(8);
        check(&mut w, &mut sm, case, seed, PICK_VOICE, &|r| r.cpu.set_gpr(4, group as u64), &|sm, env| pick_voice(sm, env, SP0 - 0x40, group) as u32, true);
    }
}

#[test]
fn claim_and_run_match() {
    let Some(mut w) = world(&[FREE_VOICE, GROUP_LIMITS, CLEAR_QUEUE, START_ONE, QUEUE_RUN]) else { return };
    let (mut g, mut sm) = (Rng(4), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let (i, group) = (g.below(16), g.below(8));
        if g.below(2) == 0 {
            check(&mut w, &mut sm, case, seed, CLAIM, &|r| r.cpu.set_gpr(4, i as u64), &|sm, env| {
                claim_voice(sm, env, i);
                0
            }, false);
        } else {
            check(&mut w, &mut sm, case, seed, 0x2bc4c0, &|r| r.cpu.set_gpr(4, group as u64), &|sm, env| {
                run_group(sm, env, group);
                0
            }, false);
        }
    }
}

#[test]
fn start_functions_match() {
    let Some(mut w) = world(&[QUEUED_NOW, QUEUE_RUN, START_VOICE]) else { return };
    let (mut g, mut sm) = (Rng(5), SM::default());
    for case in 0..20_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let id = g.below(8);
        check(&mut w, &mut sm, case, seed, TRY_START, &|r| r.cpu.set_gpr(4, id as u64), &|sm, env| try_start(sm, env, id), true);
    }
    let Some(mut w) = world(&[TRY_START, START_VOICE]) else { return };
    for case in 0..2_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        check(&mut w, &mut sm, case, seed, 0x2bc5d8, &|_| {}, &|sm, env| try_start_zero(sm, env), true);
        check(&mut w, &mut sm, case, seed, 0x2bc700, &|_| {}, &|sm, env| start_voice_zero(sm, env), true);
    }
}

#[test]
fn start_voice_matches() {
    let Some(mut w) = world(&[PICK_VOICE, CALLBACK_CHECK, FREE_VOICE, CLAIM, CLEAR_QUEUE, START_ONE]) else { return };
    let (mut g, mut sm) = (Rng(6), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let id = g.below(4);
        check(&mut w, &mut sm, case, seed, START_VOICE, &|r| r.cpu.set_gpr(4, id as u64), &|sm, env| start_voice(sm, env, SP0 - 0x90, id), true);
    }
}

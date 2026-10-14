//! The event row builder of `tricky_game::speechplay` (board row G10l) against the game's code in the function
//! runner (TRICKY_ELF; skipped without it): the sound library's callees that are not under test (the bit test, the
//! bank lookups, the variant picker) are stubs answering from one random stream on both sides, the rest of the
//! library runs as the game's; the calls, the return, the headers, the event record and the output frames are compared.

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
const SP0: u32 = 0x01ff_0000;
const TEST_BIT_A: u32 = 0x2bc9a0;

fn nargs(t: u32) -> usize {
    match t {
        BANK_ID => 1,
        TEST_BIT_A | BANK_SLOT | BANK_HAS => 2,
        _ => panic!("target"),
    }
}

struct Oracle {
    g: Rng,
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
    fn call(&mut self, _m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        self.log.push((t, args.to_vec()));
        match t {
            TEST_BIT_A | BANK_HAS => self.g.below(2),
            BANK_ID | BANK_SLOT => self.id(),
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
    e: u32,
    ord: u32,
    ctx: u32,
    out: u32,
    outb: u32,
    bt: u32,
    b: u32,
    oracle: Rc<RefCell<Oracle>>,
}

const G1: (u32, u32) = (0x348930, 0x20);
const G2: (u32, u32) = (0x348dcc, 4);
const G3: (u32, u32) = (0x3494f4, 4);
const RH: (u32, u32) = (0x349528, 0xa0);

thread_local! { static FRAMES: RefCell<Vec<(u32, u32)>> = RefCell::new(vec![]); }

fn world(stubs: &[u32], allow: &[u32]) -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let h = [r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(HDR)];
    let (e, ord, ctx, out, outb, bt, b) = (r.mem.alloc(HDR), r.mem.alloc(HDR), r.mem.alloc(0x200), r.mem.alloc(0x30), r.mem.alloc(0x100), r.mem.alloc(0x40), r.mem.alloc(0x440));
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), log: vec![] }));
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
    Some(W { r, h, e, ord, ctx, out, outb, bt, b, oracle })
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    let mut v = vec![G1, G2, G3, RH, (w.e, HDR), (w.ord, HDR), (w.ctx, 0x200), (w.out, 0x30), (w.outb, 0x100)];
    v.extend(w.h.iter().map(|&a| (a, HDR)));
    FRAMES.with(|f| v.extend(f.borrow().iter().cloned()));
    v
}

fn put(o: &mut Vec<u8>, off: u32, v: &[u8]) {
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
        let v = 3 + g.below(0x6c);
        o[4 + i as usize] = v as u8;
        ptr.push(4 * v);
    }
    for p in ptr {
        // the part record: id, index, kind, then the mask data
        put(&mut o, p, &(g.below(K + 2) as u16).to_le_bytes());
        o[p as usize + 2] = if g.below(3) == 0 { g.below(0x30) as u8 } else { g.below(0x14) as u8 };
        o[p as usize + 3] = g.below(4) as u8;
        o[p as usize + 4] = [0u8, 1, 2, 3, 4, 5, 0x80][g.below(7) as usize];
        for j in 0..4 {
            o[p as usize + 8 + j] = [0u8, 0xff, 0xfe, g.below(8) as u8, g.next() as u8][g.below(5) as usize];
            let w = if g.below(4) == 0 { g.next() } else { 0xffff_ffff };
            put(&mut o, p + 0x10 + 4 * j as u32, &w.to_le_bytes());
        }
    }
    o
}

fn order(g: &mut Rng) -> Vec<u8> {
    let mut o: Vec<u8> = (0..HDR).map(|_| g.next() as u8).collect();
    let n = g.below(21);
    o[6] = n as u8;
    for k in 0..n {
        put(&mut o, 0xc + 2 * k, &(g.below(0x60) as u16).to_le_bytes());
    }
    o
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng, maxparts: u32) {
    sm.0.clear();
    let rnd = |n: u32, g: &mut Rng| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    for k in 0..K as usize {
        img.push((w.h[k], bank(g)));
    }
    img.push((w.e, event(g, maxparts)));
    img.push((w.ord, order(g)));
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
    img.push((w.outb, rnd(0x100, g)));
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
    img.push((G2.0, w.bt.to_le_bytes().to_vec()));
    img.push((G3.0, (if g.below(6) == 0 { 0 } else { 1 + g.below(300) }).to_le_bytes().to_vec()));
    let mut rh = rnd(RH.1, g);
    put(&mut rh, 0x18, &g.below(32).to_le_bytes());
    img.push((RH.0, rh));
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
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), log: vec![] }));
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

fn frames(v: &[(u32, u32)]) {
    FRAMES.with(|f| *f.borrow_mut() = v.to_vec());
}

// every library function the code under test reaches, to run as the game's
const LIB: [u32; 18] = [0x2bea20, 0x2bea48, 0x2bc7a0, 0x2bca80, 0x2bb370, 0x2bccd0, 0x2bcdb0, 0x2bcf90, 0x2be650, 0x2bc850, 0x2bc9d8, 0x2bcb30, 0x2bcd08, 0x2bd2e8, 0x2be908, 0x2be678, 0x2be880, 0x2be7d8];

#[test]
fn row_info_matches() {
    let Some(mut w) = world(&[], &[0x2bea20]) else { return };
    let (mut g, mut sm) = (Rng(11), SM::default());
    frames(&[]);
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let (hd, out) = (w.h[g.below(K) as usize], w.out);
        let rows = sm.r8(hd + 5) as u32;
        let k = g.below(rows + 3) as i32;
        check(&mut w, &mut sm, case, seed, 0x2bea48, &|r| {
            r.cpu.set_gpr(4, hd as u64);
            r.cpu.set_gpr(5, k as u64);
            r.cpu.set_gpr(6, out as u64);
        }, &|sm, _| row_info(sm, hd, k, out) as u32, true);
        if case % 7 == 0 {
            check(&mut w, &mut sm, case, seed, 0x2bea20, &|r| r.cpu.set_gpr(4, out as u64), &|sm, _| {
                clear_row(sm, out);
                0
            }, false);
        }
    }
}

#[test]
fn collect_rows_matches() {
    let Some(mut w) = world(&[TEST_BIT_A], &LIB) else { return };
    let (mut g, mut sm) = (Rng(12), SM::default());
    frames(&[(SP0 - 0xc0, 0x1c)]);
    for case in 0..40_000 {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let (ctx, e, bb) = (w.ctx, w.e, w.b);
        let ent = ctx + 8 * g.below(12);
        let nparts = (sm.r8(e + 3) >> 2) as u32;
        let part = e + 4 * sm.r8(e + 4 + g.below(nparts.max(1))) as u32;
        let part = if nparts == 0 { e + 0x40 } else { part };
        check(&mut w, &mut sm, case, seed, 0x2bcb30, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, ent as u64);
            r.cpu.set_gpr(6, part as u64);
            r.cpu.set_gpr(7, bb as u64);
        }, &|sm, env| collect_rows(sm, env, SP0 - 0xc0, ctx, ent, part, bb), true);
    }
}

#[test]
fn weight_sum_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(13), SM::default());
    frames(&[(SP0 - 0x70, 0x10)]);
    for case in 0..40_000 {
        image(&mut w, &mut sm, &mut g, 12);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        check(&mut w, &mut sm, case, seed, 0x2bcd08, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, ctx as u64);
        }, &|sm, _| weight_sum(sm, SP0 - 0x70, e, ctx) as u32, true);
    }
}

#[test]
fn order_rows_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(14), SM::default());
    frames(&[(SP0 - 0x220, 0x50)]);
    for case in 0..40_000 {
        image(&mut w, &mut sm, &mut g, 4);
        let seed = g.next() as u64 | 1;
        let (o, ob) = (w.ord, w.outb);
        check(&mut w, &mut sm, case, seed, 0x2bcdd8, &|r| {
            r.cpu.set_gpr(4, o as u64);
            r.cpu.set_gpr(5, ob as u64);
        }, &|sm, _| {
            order_rows(sm, SP0 - 0x220, o, ob);
            0
        }, false);
    }
}

#[test]
fn build_event_matches() {
    let Some(mut w) = world(&[TEST_BIT_A, BANK_ID, BANK_SLOT, BANK_HAS], &LIB) else { return };
    let (mut g, mut sm) = (Rng(15), SM::default());
    frames(&[(SP0 - 0xa0 - 0xc0, 0x1c)]);
    let mut built = 0;
    for case in 0..60_000 {
        image(&mut w, &mut sm, &mut g, 14);
        let seed = g.next() as u64 | 1;
        let (ctx, e, bb) = (w.ctx, w.e, w.b);
        let (a3, t0, t1) = (g.below(4), g.below(4), g.below(4));
        let before = w.oracle.borrow().log.len();
        let _ = before;
        check(&mut w, &mut sm, case, seed, 0x2bd078, &|r| {
            r.cpu.set_gpr(4, ctx as u64);
            r.cpu.set_gpr(5, e as u64);
            r.cpu.set_gpr(6, bb as u64);
            r.cpu.set_gpr(7, a3 as u64);
            r.cpu.set_gpr(8, t0 as u64);
            r.cpu.set_gpr(9, t1 as u64);
        }, &|sm, env| {
            let v = build_event(sm, env, SP0 - 0xa0, ctx, e, bb, a3, t0, t1);
            v
        }, true);
        built += (w.r.cpu.gpr(2) as u32 == 1) as u32;
    }
    assert!(built > 2000, "only {built} built");
}

#[test]
fn shuffle_rows_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(16), SM::default());
    frames(&[]);
    for case in 0..40_000 {
        image(&mut w, &mut sm, &mut g, 12);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        check(&mut w, &mut sm, case, seed, 0x2bd218, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, ctx as u64);
        }, &|sm, _| {
            shuffle_rows(sm, e, ctx);
            0
        }, false);
    }
}

#[test]
fn next_combo_matches() {
    let Some(mut w) = world(&[], &[]) else { return };
    let (mut g, mut sm) = (Rng(17), SM::default());
    frames(&[]);
    for case in 0..40_000 {
        image(&mut w, &mut sm, &mut g, 12);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        for i in 0..4 {
            let v = (sm.r8(ctx + 8 * i + 4) as u32 + sm.r8(ctx + 8 * i + 5) as u32).saturating_sub(g.below(3)) as u8;
            w.r.mem.write(ctx + 8 * i + 6, &[v]).unwrap();
            sm.w8(ctx + 8 * i + 6, v);
        }
        let n = (sm.r8(e + 3) >> 2) as u32;
        if n == 0 {
            continue;
        }
        check(&mut w, &mut sm, case, seed, 0x2bd2e8, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, ctx as u64);
        }, &|sm, _| next_combo(sm, e, ctx), true);
    }
}

#[test]
fn find_combo_matches() {
    let Some(mut w) = world(&[], &LIB) else { return };
    let (mut g, mut sm) = (Rng(18), SM::default());
    frames(&[(SP0 - 0x50 - 0x70, 0x10)]);
    let mut yes = 0;
    for case in 0..20_000 {
        image(&mut w, &mut sm, &mut g, 3);
        let seed = g.next() as u64 | 1;
        let (ctx, e) = (w.ctx, w.e);
        let n = (sm.r8(e + 3) >> 2) as u32;
        if n == 0 {
            continue;
        }
        for i in 0..n {
            let c = g.below(4) as u8;
            w.r.mem.write(ctx + 8 * i + 5, &[c]).unwrap();
            sm.w8(ctx + 8 * i + 5, c);
        }
        let thr = [0u32, 1, 20, 50, 100, 150][g.below(6) as usize];
        check(&mut w, &mut sm, case, seed, 0x2bd370, &|r| {
            r.cpu.set_gpr(4, e as u64);
            r.cpu.set_gpr(5, ctx as u64);
            r.cpu.set_gpr(6, thr as u64);
        }, &|sm, _| find_combo(sm, SP0 - 0x50, e, ctx, thr as i32), true);
        yes += w.r.cpu.gpr(2) as u32;
    }
    assert!(yes > 500, "{yes}");
}

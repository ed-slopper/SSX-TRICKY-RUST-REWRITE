//! `tricky_game::speechlib` (board row G10k) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Both sides get the same random memory image (the library's globals, banks, event banks, voices and
//! sounds) and must end with the same return value, the same memory and the same calls out.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::*;

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

#[derive(Default, Debug, PartialEq)]
struct Log {
    now: i32,
    released: Vec<i32>,
    stopped: u32,
}
struct TC(Rc<RefCell<Log>>);
impl Calls for TC {
    fn now(&mut self) -> i32 {
        self.0.borrow().now
    }
    fn release(&mut self, slot: i32) {
        self.0.borrow_mut().released.push(slot);
    }
    fn stop_all(&mut self) {
        self.0.borrow_mut().stopped += 1;
    }
}

const Q0: u32 = 0x348950;
const Q1: u32 = 0x348dc8;
const G0: u32 = 0x348dc8;
const G1: u32 = 0x3495d0;

struct World {
    r: Runner,
    table: u32,
    banks: [u32; 6],
    evbanks: [u32; 4],
    ev: u32,
    sounds: [u32; 4],
    scratch: u32,
    q1: u32,
    q4: u32,
    q6: u32,
    log: Rc<RefCell<Log>>,
}

fn world() -> Option<World> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log = Rc::new(RefCell::new(Log::default()));
    let l = log.clone();
    r.stub(0x2bebb0, move |c, _| c.set_gpr(2, l.borrow().now as i64 as u64));
    let l = log.clone();
    r.stub(0x2bc200, move |c, _| l.borrow_mut().released.push(c.gpr(4) as u32 as i32));
    let l = log.clone();
    r.stub(0x2baed0, move |_, _| l.borrow_mut().stopped += 1);
    for a in [0x2bb078, 0x2bb110, 0x2bb430, 0x2bb610, 0x2bb650, 0x2bb7a0, 0x2be678, 0x2be768, 0x2be7d8, 0x2be880] {
        r.allow(a);
    }
    let table = r.mem.alloc(0x40);
    let banks = [(); 6].map(|_| r.mem.alloc(0x100));
    let evbanks = [(); 4].map(|_| r.mem.alloc(0x100));
    let ev = r.mem.alloc(0x10);
    let sounds = [(); 4].map(|_| r.mem.alloc(0x20));
    let scratch = r.mem.alloc(0x10);
    let (q1, q4, q6) = (r.mem.alloc(0x200), r.mem.alloc(0x10), r.mem.alloc(0x50));
    Some(World { r, table, banks, evbanks, ev, sounds, scratch, q1, q4, q6, log })
}

type Image = Vec<(u32, Vec<u8>)>;

fn put(img: &mut Image, a: u32, v: &[u8]) {
    img.push((a, v.to_vec()));
}

fn history_at(f: u8, n: u8, b: u32) -> u32 {
    let (f, n) = (f as u32, n as u32);
    let bm = if f & 0x80 != 0 { (n + 7) / 8 + 1 } else { 0 };
    b + n * ((f & 0xf) + 2) + 0xc + ((f & 0x70) >> 3) + bm
}

fn build(g: &mut Rng, w: &World) -> Image {
    let mut img: Image = Vec::new();
    let mut noise = vec![0u8; (G1 - G0) as usize];
    for b in noise.iter_mut() {
        *b = g.next() as u8;
    }
    put(&mut img, G0, &noise);
    let u32s = |v: &[u32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    // magic, table
    put(&mut img, G0, &u32s(&[if g.below(3) == 0 { 0 } else { 0x178_9a34 }]));
    let count = g.below(7);
    let tptr = if g.below(8) == 0 { 0 } else { w.table };
    put(&mut img, 0x348dcc, &u32s(&[tptr, if g.below(10) == 0 { g.next() % 3 } else { count.min(6) }]));
    let mut entries = Vec::new();
    for _ in 0..10 {
        entries.push(if g.below(3) == 0 { 0 } else { w.banks[g.below(6) as usize] });
    }
    put(&mut img, w.table, &u32s(&entries));
    // banks
    for &b in &w.banks {
        let mut blk = vec![0u8; 0x100];
        for x in blk.iter_mut() {
            *x = if g.below(3) == 0 { 0xff } else { g.next() as u8 };
        }
        let f = g.next() as u8;
        let n = g.below(5) as u8;
        let len = g.below(5) as u8;
        blk[4] = f;
        blk[5] = n;
        blk[6] = len;
        let id = |g: &mut Rng| g.below(4) as u16;
        blk[0..2].copy_from_slice(&id(g).to_le_bytes());
        blk[2..4].copy_from_slice(&id(g).to_le_bytes());
        let cnt: u16 = [0xffff, 0, 3, g.next() as u16, 1][g.below(5) as usize];
        blk[10..12].copy_from_slice(&cnt.to_le_bytes());
        let p = (history_at(f, n, b) - b) as usize;
        let clean = g.below(2) == 0;
        if clean {
            blk[p] = 0;
            for k in 0..len as usize {
                blk[p + 1 + k] = 0xff;
            }
        } else {
            blk[p] = g.below(len as u32 + 2) as u8;
        }
        if f & 0x80 != 0 {
            let q = (b + n as u32 * ((f as u32 & 0xf) + 2) + 0xc + ((f as u32 & 0x70) >> 3) - b) as usize;
            blk[q] = g.below(6) as u8;
        }
        put(&mut img, b, &blk);
    }
    // event banks
    for &b in &w.evbanks {
        let mut blk = vec![0u8; 0x100];
        for x in blk.iter_mut() {
            *x = g.next() as u8;
        }
        let n = g.below(6);
        blk[0x10..0x12].copy_from_slice(&(n as u16).to_le_bytes());
        for k in 0..6 {
            let off = (0x10 + 4 * g.below(12)) as u16; // words
            blk[0x18 + 2 * k..0x1a + 2 * k].copy_from_slice(&off.to_le_bytes());
        }
        for k in 0..12 {
            let id: u16 = if g.below(4) == 0 { 0xfff0 + g.below(4) as u16 } else { g.below(5) as u16 };
            let at = (0x40 + 16 * k) as usize;
            if at + 2 <= 0x100 {
                blk[at..at + 2].copy_from_slice(&id.to_le_bytes());
            }
        }
        blk[8] = g.below(3) as u8;
        blk[9] = g.below(3) as u8;
        put(&mut img, b, &blk);
    }
    let mut slots = Vec::new();
    for _ in 0..8 {
        slots.push(if g.below(3) == 0 { 0 } else { w.evbanks[g.below(4) as usize] });
        slots.push(g.next());
    }
    put(&mut img, 0x348e08, &u32s(&slots));
    // the event record
    let id: u16 = if g.below(4) == 0 { 0xfff0 + g.below(4) as u16 } else { g.below(5) as u16 };
    let mut ev = vec![0u8; 0x10];
    ev[0..2].copy_from_slice(&id.to_le_bytes());
    ev[2] = g.below(3) as u8;
    ev[3] = g.below(3) as u8;
    put(&mut img, w.ev, &ev);
    // sounds
    for &s in &w.sounds {
        let mut blk = vec![0u8; 0x20];
        for x in blk.iter_mut() {
            *x = g.next() as u8;
        }
        let age: u16 = [0, 1, 5, 40, 1000][g.below(5) as usize];
        let size: u16 = [0, 2, 8, 30, 200][g.below(5) as usize];
        blk[2..4].copy_from_slice(&age.to_le_bytes());
        blk[4..6].copy_from_slice(&size.to_le_bytes());
        put(&mut img, s, &blk);
    }
    // voice groups, limits
    let mut grp = Vec::new();
    for _ in 0..8 {
        grp.push(g.below(4));
    }
    for _ in 0..8 {
        grp.push(if g.below(3) == 0 { g.next() } else { g.below(0x14) });
    }
    for _ in 0..8 {
        grp.push(if g.below(3) == 0 { g.next() } else { g.below(0x14) });
    }
    put(&mut img, 0x348e48, &u32s(&grp));
    // voices
    for i in 0..16u32 {
        let r = 0x348eac + 0x60 * i;
        let mut rec = vec![0u8; 0x10];
        for x in rec.iter_mut() {
            *x = g.next() as u8;
        }
        rec[0..4].copy_from_slice(&(g.below(2000)).to_le_bytes());
        rec[6] = if g.below(40) == 0 { 0 } else { 1 };
        rec[7] = if g.below(6) == 0 { 0xff } else { g.below(8) as u8 };
        let st: u16 = if g.below(3) == 0 { g.next() as u16 } else { 0 };
        rec[8..10].copy_from_slice(&st.to_le_bytes());
        rec[0xc..0x10].copy_from_slice(&w.sounds[g.below(4) as usize].to_le_bytes());
        put(&mut img, r, &rec);
    }
    // the event queue
    let mut qn = vec![0u8; (Q1 - Q0) as usize];
    for b in qn.iter_mut() {
        *b = g.next() as u8;
    }
    put(&mut img, Q0, &qn);
    put(&mut img, Q0, &u32s(&[[0xffff_ffff, 1, 2, 3][g.below(4) as usize], g.below(10), g.below(20)]));
    let mut q1b = vec![0u8; 0x200];
    for b in q1b.iter_mut() {
        *b = g.next() as u8;
    }
    put(&mut img, w.q1, &q1b);
    let mut q4b = vec![0u8; 0x10];
    for b in q4b.iter_mut() {
        *b = g.next() as u8;
    }
    q4b[3] = ((g.below(6) << 2) | g.below(4)) as u8;
    put(&mut img, w.q4, &q4b);
    let q6b: Vec<u8> = (0..0x50).map(|_| g.next() as u8).collect();
    put(&mut img, w.q6, &q6b);
    // the generator (some words about to carry) and the variant history
    let rw: Vec<u32> = (0..6).map(|_| [g.next(), 0xffff_ffff, 0xffff_fffe, 0, g.next()][g.below(5) as usize]).collect();
    put(&mut img, 0x349528, &u32s(&rw));
    let hp = g.below(32);
    put(&mut img, 0x349540, &u32s(&[hp]));
    let mut he = Vec::new();
    let wide = g.below(2) == 0;
    for k in 0..64 {
        let v = if wide {
            if k % 2 == 0 { g.below(40) } else if g.below(8) == 0 { g.below(3) } else { 0 }
        } else if g.below(6) == 0 {
            0xffffu32
        } else {
            g.below(5)
        };
        he.extend((v as u16).to_le_bytes());
    }
    put(&mut img, 0x349548, &he);
    put(&mut img, 0x3494f0, &u32s(&[if g.below(3) == 0 { g.next() } else { g.below(40) }]));
    img
}

fn apply(w: &mut World, sm: &mut SM, img: &Image) {
    sm.0.clear();
    for (a, v) in img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn dump(w: &mut World) -> Vec<u8> {
    let mut out = vec![0u8; (G1 - G0) as usize];
    w.r.mem.read(G0, &mut out).unwrap();
    let mut q = vec![0u8; (Q1 - Q0) as usize];
    w.r.mem.read(Q0, &mut q).unwrap();
    out.extend(q);
    for &(a, n) in w
        .banks
        .iter()
        .map(|b| (*b, 0x100usize))
        .chain(w.evbanks.iter().map(|b| (*b, 0x100)))
        .chain([(w.table, 0x40usize), (w.q1, 0x200)])
        .collect::<Vec<_>>()
        .iter()
    {
        let mut b = vec![0u8; n];
        w.r.mem.read(a, &mut b).unwrap();
        out.extend(b);
    }
    out
}

fn dump_sm(w: &World, sm: &SM) -> Vec<u8> {
    let mut out: Vec<u8> = (G0..G1).map(|a| sm.r8(a)).collect();
    out.extend((Q0..Q1).map(|a| sm.r8(a)));
    for &(a, n) in w
        .banks
        .iter()
        .map(|b| (*b, 0x100u32))
        .chain(w.evbanks.iter().map(|b| (*b, 0x100)))
        .chain([(w.table, 0x40u32), (w.q1, 0x200)])
        .collect::<Vec<_>>()
        .iter()
    {
        out.extend((a..a + n).map(|x| sm.r8(x)));
    }
    out
}

/// Run `n` cases: build an image, run the original through `orig` (which sets arguments, calls and returns v0) and
/// the port through `port` (returns the same value), compare the value, the memory and the calls out.
fn run(seed: u64, n: u32, mut args: impl FnMut(&mut Rng, &World) -> Vec<u64>, addr: u32, mut port: impl FnMut(&mut SM, &mut TC, &[u64]) -> u64, mask: u64) {
    let Some(mut w) = world() else { return };
    let mut g = Rng(seed);
    let mut sm = SM::default();
    for case in 0..n {
        let img = build(&mut g, &w);
        let a = args(&mut g, &w);
        let now = g.next() as i32;
        {
            let mut l = w.log.borrow_mut();
            *l = Log { now, ..Default::default() };
        }
        apply(&mut w, &mut sm, &img);
        for (k, v) in a.iter().enumerate() {
            w.r.cpu.set_gpr(4 + k, *v);
        }
        w.r.call(addr).unwrap();
        let want = w.r.cpu.gpr(2) & mask;
        let theirs = dump(&mut w);
        let their_log = std::mem::take(&mut *w.log.borrow_mut());
        let log = Rc::new(RefCell::new(Log { now, ..Default::default() }));
        let mut tc = TC(log.clone());
        let got = port(&mut sm, &mut tc, &a) & mask;
        assert_eq!(got, want, "case {case} args {a:x?}");
        let mine_mem = dump_sm(&w, &sm);
        if mine_mem != theirs {
            let at = mine_mem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
            panic!("{addr:#x} case {case} memory differs at +{at:#x}: port {:#x} game {:#x}, args {a:x?}", mine_mem[at], theirs[at]);
        }
        let mine = log.borrow();
        assert_eq!((&mine.released, mine.stopped), (&their_log.released, their_log.stopped), "case {case} calls");
    }
}

const ALL: u64 = u64::MAX;

#[test]
fn bank_table_functions() {
    // set_bank_table / free slot / find by id / by ids / has sound / clear slot / register
    run(1, 6000, |g, w| vec![g.below(8) as u64, if g.below(5) == 0 { 0 } else { w.table as u64 }], 0x2bb098, |m, _, a| { set_bank_table(m, a[0] as i32, a[1] as u32); 0 }, 0);
    run(2, 6000, |_, _| vec![], 0x2bb110, |m, _, _| find_free_bank_slot(m) as i64 as u64, 0xffff_ffff);
    run(3, 6000, |g, _| vec![g.below(5) as u64 | if g.below(8) == 0 { 0xffff_0000 } else { 0 }], 0x2bb180, |m, _, a| find_bank_by_id(m, a[0] as i16) as i64 as u64, 0xffff_ffff);
    run(4, 6000, |g, _| vec![g.below(5) as u64, g.below(5) as u64], 0x2bb1f0, |m, _, a| find_bank_by_ids(m, a[0] as i16, a[1] as i16) as i64 as u64, 0xffff_ffff);
    run(5, 6000, |g, _| vec![if g.below(6) == 0 { (-1i64) as u64 } else { g.below(8) as u64 }, g.below(6) as u64], 0x2bb270, |m, _, a| bank_has_sound(m, a[0] as i64, a[1]) as u64, ALL);
    run(6, 6000, |g, _| vec![g.below(8) as u64], 0x2bb2b0, |m, _, a| { clear_bank_slot(m, a[0] as i32); 0 }, 0);
    run(7, 12000, |g, w| vec![w.banks[g.below(6) as usize] as u64], 0x2bb4f0, |m, _, a| register_bank(m, a[0] as u32) as u64, ALL);
    run(8, 6000, |_, _| vec![], 0x2bb060, |m, _, _| { clear_bank_table(m); 0 }, 0);
}

#[test]
fn history_functions() {
    run(10, 8000, |g, w| vec![w.banks[g.below(6) as usize] as u64, g.below(256) as u64], 0x2bb2d8, |m, _, a| { history_push(m, a[0] as u32, a[1] as u8); 0 }, 0);
    run(11, 8000, |g, w| vec![w.banks[g.below(6) as usize] as u64, if g.below(3) == 0 { 0xff } else { g.below(256) as u64 }], 0x2bb370, |m, _, a| history_find(m, a[0] as u32, a[1]) as i64 as u64, 0xffff_ffff);
    run(12, 8000, |g, w| vec![w.banks[g.below(6) as usize] as u64], 0x2bb430, |m, _, a| { stir_bitmap(m, a[0] as u32); 0 }, 0);
}

#[test]
fn event_functions() {
    run(20, 8000, |g, w| vec![w.banks[g.below(4) as usize] as u64, g.below(5) as u64], 0x2bb610, |m, _, a| bank_event(m, a[0] as u32, a[1]) as u64, 0xffff_ffff);
    run(21, 8000, |_, w| vec![w.ev as u64], 0x2bb6b0, |m, _, a| find_event(m, a[0] as u32) as u64, 0xffff_ffff);
    run(22, 8000, |_, w| vec![w.ev as u64, w.scratch as u64], 0x2bb7d0, |m, _, a| event_bank_size(m, a[0] as u32) as u64, 0xffff);
    run(23, 4000, |_, _| vec![], 0x2bb7a0, |m, _, _| { clear_event_banks(m); 0 }, 0);
}

#[test]
fn event_out_functions() {
    for (addr, seed) in [(0x2bb700u32, 30u64), (0x2bb750, 31)] {
        let Some(mut w) = world() else { return };
        let mut g = Rng(seed);
        let mut sm = SM::default();
        for case in 0..8000 {
            let img = build(&mut g, &w);
            apply(&mut w, &mut sm, &img);
            w.r.mem.write_u32(w.scratch, 0xdead_beef);
            w.r.cpu.set_gpr(4, w.ev as u64);
            w.r.cpu.set_gpr(5, w.scratch as u64);
            w.r.call(addr).unwrap();
            let found = w.r.cpu.gpr(2) & 0xff != 0;
            let out = w.r.mem.read_u32(w.scratch);
            let mine = if addr == 0x2bb700 { event_word(&sm, w.ev) } else { event_bank_kind(&sm, w.ev) };
            assert_eq!(mine.is_some(), found, "case {case}");
            assert_eq!(mine.unwrap_or(0xdead_beef), out, "case {case}");
        }
    }
}

#[test]
fn group_and_reset() {
    run(40, 6000, |g, _| vec![g.below(12) as u64 | if g.below(10) == 0 { 0xffff_fff0 } else { 0 }], 0x2bba50, |m, _, a| group_limits_ok(m, a[0] as i32) as u64, 0xff);
    run(41, 3000, |_, _| vec![], 0x2bb810, |m, _, _| { reset_voices(m); 0 }, 0);
    run(42, 6000, |_, _| vec![], 0x2bb018, |m, c, _| { shutdown(m, c); 0 }, 0);
}

#[test]
fn alloc_voice_matches() {
    run(
        50,
        40_000,
        |g, _| vec![[0, 2, 8, 30, 200, 300][g.below(6) as usize] as u64, g.below(9) as u64, [0u64, 1, ALL][g.below(3) as usize]],
        0x2bb8d8,
        |m, c, a| alloc_voice(m, c, a[0], a[1], a[2] as i64) as i64 as u64,
        0xffff_ffff,
    );
}

#[test]
fn random_and_history() {
    run(60, 8000, |_, _| vec![], 0x2be678, |m, _, _| rand(m) as u64, 0xffff_ffff);
    run(61, 4000, |g, _| vec![g.next() as u64], 0x2be768, |m, _, a| { seed_rand(m, a[0] as i32); 0 }, 0);
    run(62, 4000, |g, _| vec![g.next() as u64], 0x2be8c8, |m, _, a| { init_history(m, a[0] as i32); 0 }, 0);
    run(63, 4000, |g, _| vec![g.below(5) as u64, g.below(5) as u64], 0x2be880, |m, _, a| { push_history(m, a[0] as u16, a[1] as u16); 0 }, 0);
    run(
        64,
        12000,
        |g, _| vec![g.below(40) as u64 | if g.below(6) == 0 { 0xffff_0000 } else { 0 }, [0u64, 3, 10, 32, 40, (-16i64) as u64][g.below(6) as usize], g.below(3) as u64],
        0x2be7d8,
        |m, _, a| find_in_history(m, a[0] as i16, a[1] as i32, a[2] as i16) as i64 as u64,
        0xffff_ffff,
    );
    run(65, 30000, |g, _| vec![[1u64, 2, 3, 5, 8, 20, 40, 40, 20][g.below(9) as usize], g.below(3) as u64], 0x2be908, |m, _, a| pick_variant(m, a[0] as i32, a[1] as u16) as u64, 0xffff_ffff);
    run(66, 4000, |g, w| vec![g.below(256) as u64, g.below(256) as u64, w.sounds[g.below(4) as usize] as u64], 0x2be650, |m, _, a| mask_match(m, a[0] as u8, a[1] as u8, a[2] as u32) as u64, 0xff);
}

#[test]
fn event_queue() {
    run(70, 3000, |_, _| vec![], 0x2bdae0, |m, _, _| { clear_queue(m); 0 }, 0);
    run(71, 3000, |g, _| vec![[0xffff_ffffu64 as i32 as i64 as u64, 1, 2, 3, 7][g.below(5) as usize]], 0x2bdc70, |m, _, a| has_queued(m, a[0] as i32) as u64, 0xff);
    run(72, 3000, |g, w| vec![w.q1 as u64 + 32, (g.below(200) as i64 - 20) as u64, 0], 0x2bc9a0, |m, _, a| test_bit(m, a[0] as u32, a[1] as i32) as u64, 0xffff_ffff);
    run(73, 3000, |_, w| vec![w.q1 as u64], 0x2bdad0, |m, _, a| { clear_flags(m, a[0] as u32); 0 }, 0);
    run(
        74,
        20000,
        |g, w| vec![w.q1 as u64, [0xffff_ffffu64 as i32 as i64 as u64, 1, 2, 3][g.below(4) as usize], g.next() as u64, w.q4 as u64, g.next() as u8 as u64, w.q6 as u64],
        0x2bdb68,
        |m, _, a| { queue_event(m, a[0] as u32, a[1] as i32, a[2] as u32, a[3] as u32, a[4] as u8, a[5] as u32); 0 },
        0,
    );
}

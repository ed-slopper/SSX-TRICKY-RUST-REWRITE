//! `tricky_game::pausemenu` (board row G9a) against the pause menu functions 0x1ac410..0x1ace00 in the function runner (TRICKY_ELF;
//! skipped without it): every callee (the ini reader, strcmp, sprintf, the archive and bank loaders) is a stub that
//! answers from the same random stream on both sides and writes strings into the frame the way the real ones do; the
//! calls (address and arguments), the frame and the speech object are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::Mem;
use tricky_game::pausemenu::*;
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


const FA: [u32; 4] = [0x90_0000, 0x90_0010, 0x90_0020, 0x90_0030];
const STATIC_TARGETS: [u32; 15] = [
    LOC_GET, GET_BUTTONS, SOUND_MOVE, SOUND_BACK, SOUND_SELECT, DIALOG_SET, GET_TRICKBOOK, COUNT_PAGES, PAGE_FIRST, SYNC_AUDIO, HIDE_ITEM,
    RESUME_ALL, PLAY_PAUSE, STOP_ALL, STOP_SONG,
];

struct Oracle {
    g: Rng,
    log: Vec<(u32, Vec<u32>)>,
}

impl Oracle {
    fn call(&mut self, _m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(STATIC_TARGETS.contains(&t) || FA.contains(&t), "target {t:#x}");
        self.log.push((t, args.to_vec()));
        const BTN: [u32; 16] = [0, 0, 0x120, 0x20, 0x100, 0x20001, 0x1, 0x40002, 0x2, 0x800, 0x800, 0x100008, 0x8, 0x80004, 0x4, 0x30000];
        match t {
            GET_BUTTONS => BTN[self.g.below(16) as usize],
            COUNT_PAGES => self.g.below(8),
            PAGE_FIRST => self.g.below(30),
            SYNC_AUDIO => self.g.below(2),
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

const P: u32 = 0xa00;
const TAB: u32 = 0x40;
const G: u32 = 0x800;

struct W {
    r: Runner,
    p: u32,
    tab: u32,
    g: u32,
    race: u32,
    q: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x1ac410);
    r.allow(0x1ac580);
    let p = r.mem.alloc(P);
    let tab = r.mem.alloc(TAB);
    let g = r.mem.alloc(G);
    let race = r.mem.alloc(0x80);
    let q = r.mem.alloc(0x20);
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), log: vec![] }));
    for t in STATIC_TARGETS.iter().chain(FA.iter()).copied() {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32];
            let v = l.borrow_mut().call(&mut RM2(m), t, &args);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, p, tab, g, race, q, oracle })
}

/// A random menu; `deep` allows the stack to be one page deeper.
fn image(w: &mut W, sm: &mut SM, g: &mut Rng, resume_only: bool) {
    sm.0.clear();
    let mut o: Vec<u8> = (0..P).map(|_| g.next() as u8).collect();
    let mut set = |off: u32, v: u32| o[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    let top: i32 = if resume_only { g.below(2) as i32 - 1 } else { g.below(3) as i32 - 1 };
    set(0x7e8, top as u32);
    for k in 0..2 {
        set(0x7e0 + k * 4, w.p + 0x438 + g.below(3) * 0x138);
    }
    set(0x42c, g.next());
    set(0x7f0, g.below(2));
    set(0x7f4, g.below(30));
    set(0x44c, g.below(6));
    set(0x450, 1 + g.below(6));
    for k in 0..6 {
        set(0x454 + k * 0x18, [0, 1, 4, 5, 6, 7, 2][g.below(7) as usize]);
    }
    set(0x100, w.tab);
    for j in 0..3u32 {
        let rec = 0x438 + j * 0x138;
        for (adj, idx, tbl) in [(0x128u32, 0x12au32, 0x12cu32), (0x130, 0x132, 0x134)] {
            let n: i32 = [0, -1, 1, 2][g.below(4) as usize];
            let a: u32 = [0, 0x10, 0xfff0, 0x20][g.below(4) as usize];
            let hi = g.next() as u16 as u32;
            set(rec + adj, a | hi << 16);
            let word = if n > 0 { 0x100 | (n as u32) << 16 } else { FA[g.below(4) as usize] };
            // the index halfword sits above the adjust halfword; the table offset or pointer is the next word
            set(rec + adj, a | (n as u16 as u32) << 16);
            set(rec + tbl, word);
        }
    }
    let mut tab: Vec<u8> = vec![];
    for _ in 0..8 {
        let adj = [0u32, 0x10, 0xffe0, 0x30][g.below(4) as usize] | (g.next() as u16 as u32) << 16;
        tab.extend(adj.to_le_bytes());
        tab.extend(FA[g.below(4) as usize].to_le_bytes());
    }
    let mut img: Vec<(u32, Vec<u8>)> = vec![(w.p, o), (w.tab, tab)];
    let mut gapp: Vec<u8> = (0..G).map(|_| g.next() as u8).collect();
    gapp[0x730..0x734].copy_from_slice(&w.race.to_le_bytes());
    img.push((w.g, gapp));
    let mut race: Vec<u8> = (0..0x80).map(|_| g.next() as u8).collect();
    race[0x38..0x3c].copy_from_slice(&w.q.to_le_bytes());
    race[0x54..0x58].copy_from_slice(&g.below(3).to_le_bytes());
    img.push((w.race, race));
    img.push((w.q, (0..0x20).map(|_| g.next() as u8).collect()));
    let mut st1: Vec<u8> = (0..16).map(|_| g.next() as u8).collect();
    st1[0..4].copy_from_slice(&w.g.to_le_bytes());
    img.push((0x337c58, st1));
    img.push((0x343050, g.next().to_le_bytes().to_vec()));
    img.push((0x38e328, (0..16).map(|_| g.next() as u8).collect()));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    vec![(w.p, P), (w.g, G), (w.race, 0x80), (w.q, 0x20)]
}

fn check(w: &mut W, sm: &mut SM, case: u32, seed: u64, addr: u32, a1: Option<u32>, port: &dyn Fn(&mut SM, &mut PortEnv)) {
    {
        let mut o = w.oracle.borrow_mut();
        o.g = Rng(seed);
        o.log.clear();
    }
    w.r.cpu.set_gpr(4, w.p as u64);
    if let Some(a) = a1 {
        w.r.cpu.set_gpr(5, a as u64);
    }
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
    port(sm, &mut env);
    let my_calls = std::mem::take(&mut mine.borrow_mut().log);
    for i in 0..my_calls.len().min(theirs_calls.len()) {
        assert_eq!(my_calls[i].0, theirs_calls[i].0, "case {case}: call {i}");
        assert_eq!(my_calls[i].1[..], theirs_calls[i].1[..my_calls[i].1.len()], "case {case}: args of call {i} ({:#x})", my_calls[i].0);
    }
    assert_eq!(my_calls.len(), theirs_calls.len(), "case {case}: number of calls");
    let mut mymem = vec![];
    for (a, n) in regions(w) {
        mymem.extend((a..a + n).map(|x| sm.r8(x)));
    }
    if mymem != theirs {
        let at = mymem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
        panic!("case {case}: memory differs at +{at:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}

#[test]
fn resume_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(3), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g, true);
        let (seed, page) = (g.next() as u64 | 1, g.below(2));
        let p = w.p;
        check(&mut w, &mut sm, case, seed, 0x1ac410, Some(page), &|sm, env| resume(sm, env, p, page));
    }
}

#[test]
fn pop_level_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(4), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g, false);
        // a top of -1 would read the stack below its start: the game never calls with an empty stack
        let t = sm.r32(w.p + 0x7e8) as i32;
        if t < 0 {
            continue;
        }
        let seed = g.next() as u64 | 1;
        let p = w.p;
        check(&mut w, &mut sm, case, seed, 0x1ac580, None, &|sm, env| pop_level(sm, env, p));
    }
}

#[test]
fn build_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(5), SM::default());
    for case in 0..5_000 {
        image(&mut w, &mut sm, &mut g, false);
        let seed = g.next() as u64 | 1;
        let p = w.p;
        check(&mut w, &mut sm, case, seed, 0x1ac678, None, &|sm, env| build_main_page(sm, env, p));
    }
}

#[test]
fn update_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(6), SM::default());
    for case in 0..60_000 {
        image(&mut w, &mut sm, &mut g, true);
        let t = sm.r32(w.p + 0x7e8) as i32;
        if t < 0 {
            continue;
        }
        let seed = g.next() as u64 | 1;
        let p = w.p;
        check(&mut w, &mut sm, case, seed, 0x1ace00, None, &|sm, env| update_main_page(sm, env, p));
    }
}

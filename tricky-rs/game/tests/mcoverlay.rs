//! `tricky_game::mcoverlay` (board row G9m) against the original in the function runner (TRICKY_ELF; skipped without
//! it): the overlay object in random states, the card system and the overlay's second vtable faked with stubs that
//! answer from the same random stream on both sides, and every virtual call (offset, arguments, `this`) and the
//! overlay's memory compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::mcoverlay::*;
use tricky_game::speechlib::Mem;

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

type Call = (bool, u32, Vec<u32>);

/// The oracle: the answer to a virtual call, drawn from the stream, and the poke it makes into the overlay.
struct Oracle {
    g: Rng,
    o: u32,
    log: Vec<Call>,
}

const FIELDS: [u32; 15] = [0x8, 0xc, 0x10, 0x14, 0x18, 0x24, 0x28, 0x30, 0x38, 0x44, 0x54, 0x15c, 0x160, 0x21c, 0x220];

impl Oracle {
    /// (answer, optional poke (address, value)).
    fn call(&mut self, owner: bool, off: u32, args: &[u32]) -> (i64, Option<(u32, u32)>) {
        self.log.push((owner, off, args.to_vec()));
        if owner && off == 0x1bc {
            return (0, Some((self.o + 0x54, args[0])));
        }
        let v: i64 = if owner && off == 0x1c {
            [0u64, 0x20, 0x800, 0x80, 0x880, 0x40, 0x1, 0x2, 0x20001, 0x40002, 0x820, 0x8a0][self.g.below(12) as usize] as i64
        } else if !owner && (off == 0x11c || off == 0xcc) {
            self.g.below(3) as i64
        } else if !owner && matches!(off, 0x8c | 0xa4 | 0x7c) {
            self.g.below(100) as i64 - 20
        } else {
            self.g.below(2) as i64
        };
        let poke = if self.g.below(8) == 0 {
            let f = FIELDS[self.g.below(15) as usize];
            let val = match f {
                0x54 => self.g.below(0x24),
                0x220 => self.g.below(2),
                0x8 | 0xc | 0x10 | 0x14 | 0x18 | 0x15c => self.g.below(2),
                _ => self.g.below(30),
            };
            Some((self.o + f, val))
        } else {
            None
        };
        (v, poke)
    }
}

struct PortEnv(Rc<RefCell<Oracle>>);
impl CardEnv for PortEnv {
    fn card(&mut self, m: &mut dyn Mem, off: u32, args: &[u32]) -> i64 {
        let (v, poke) = self.0.borrow_mut().call(false, off, args);
        if let Some((a, x)) = poke {
            m.w32(a, x);
        }
        v
    }
    fn owner(&mut self, m: &mut dyn Mem, off: u32, args: &[u32]) -> i64 {
        let (v, poke) = self.0.borrow_mut().call(true, off, args);
        if let Some((a, x)) = poke {
            m.w32(a, x);
        }
        v
    }
}

const CARD_OFFS: [u32; 29] = [
    0x1c, 0x2c, 0x4c, 0x5c, 0x64, 0x6c, 0x74, 0x7c, 0x84, 0x8c, 0xa4, 0xcc, 0xd4, 0xdc, 0x11c, 0x124, 0x12c, 0x144, 0x154, 0x164, 0x174, 0x17c, 0x194, 0x1ac, 0x1bc, 0x1c4, 0x1cc, 0x1e4, 0x1ec,
];
const OWNER_OFFS: [u32; 17] = [0x1c, 0x2c, 0x34, 0x3c, 0x54, 0x5c, 0x64, 0x6c, 0x15c, 0x164, 0x194, 0x19c, 0x1a4, 0x1ac, 0x1b4, 0x1bc, 0x1c4];

const S0: u32 = 0x33efb0;
const S1: u32 = 0x33f020;

struct W {
    r: Runner,
    o: u32,
    card: u32,
    cvt: u32,
    ovt: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [
        0x1cf040u32, 0x1cf8d0, 0x1cf918, 0x1cfaa8, 0x1cfbc8, 0x1cfcf8, 0x1cfee0, 0x1d0a38, 0x1d0150, 0x1d0240, 0x1d02e8, 0x1d0358, 0x1d0420, 0x1d04d8, 0x1d05f8, 0x1d06a8, 0x1d0740, 0x1d07d8,
        0x1d0870, 0x1d08b8, 0x1d0f20, 0x1d0ff0, 0x1d10b8, 0x1d1268, 0x1d12f0, 0x1d1498, 0x1d1720, 0x1d17c8, 0x1d1978, 0x1d1a10, 0x1d1ac0, 0x1d1bc0, 0x1d1c30, 0x1d3818, 0x1d1f60, 0x1ce880,
        0x1ce700, 0x1d2010, 0x2cc808,
    ] {
        r.allow(a);
    }
    let o = r.mem.alloc(0x600);
    let card = r.mem.alloc(0x20);
    let cvt = r.mem.alloc(0x400);
    let ovt = r.mem.alloc(0x400);
    r.mem.write_u32(card, cvt);
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), o, log: vec![] }));
    for (kind_owner, offs, vt, obj) in [(false, &CARD_OFFS[..], cvt, card), (true, &OWNER_OFFS[..], ovt, o)] {
        for (k, off) in offs.iter().enumerate() {
            let adj = 0x10 * (k as u32 % 3 + 1);
            r.mem.write_u32(vt + off - 4, adj); // the halfword (little end); the next halfword is part of the previous entry's slot
            let fa = 0x90_0000 + (kind_owner as u32) * 0x1000 + off;
            r.mem.write_u32(vt + off, fa);
            let l = oracle.clone();
            let off = *off;
            r.stub(fa, move |c, m| {
                assert_eq!(c.gpr(4) as u32, obj + adj, "this for {off:#x}");
                let args = [c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
                let mut lg = l.borrow_mut();
                let (v, poke) = lg.call(kind_owner, off, &args);
                // the logged argument list is the full three; the comparison takes the port's count
                if let Some(last) = lg.log.last_mut() {
                    last.2 = args.to_vec();
                }
                drop(lg);
                if let Some((a, x)) = poke {
                    m.write_u32(a, x);
                }
                c.set_gpr(2, v as u64);
            });
        }
    }
    r.mem.write_u32(o + 0x224, card);
    r.mem.write_u32(o + 0x428, ovt);
    Some(W { r, o, card, cvt, ovt, oracle })
}

fn build(g: &mut Rng, w: &W) -> Vec<(u32, Vec<u8>)> {
    let mut img = Vec::new();
    let mut o: Vec<u32> = (0..0x600 / 4).map(|_| g.next()).collect();
    let f = |g: &mut Rng, n: u32| g.below(n);
    let at = |off: u32| (off / 4) as usize;
    o[at(0x54)] = f(g, 0x25);
    o[at(4)] = [0, 0, 1, 2][g.below(4) as usize];
    for off in [0x8, 0xc, 0x10, 0x14, 0x18, 0x15c] {
        o[at(off)] = f(g, 2);
    }
    for off in [0x24, 0x28, 0x30] {
        o[at(off)] = if g.below(10) == 0 { 0 } else { f(g, 50) };
    }
    o[at(0x38)] = f(g, 4);
    o[at(0x44)] = f(g, 6).wrapping_sub(2);
    o[at(0x160)] = f(g, 8).wrapping_sub(1);
    o[at(0x21c)] = f(g, 5);
    o[at(0x220)] = f(g, 2);
    o[at(0x224)] = w.card;
    o[at(0x428)] = w.ovt;
    let fl = [0.0f32, 0.2, 0.19, 1.0, 1.01, 0.5, -0.0, 0.0166, -0.0166, -0.3, 0.9];
    o[at(0x4c)] = fl[g.below(11) as usize].to_bits();
    o[at(0x50)] = fl[g.below(11) as usize].to_bits();
    img.push((w.o, o.iter().flat_map(|x| x.to_le_bytes()).collect()));
    let st: Vec<u32> = (0..(S1 - S0) / 4).map(|_| g.below(2)).collect();
    img.push((S0, st.iter().flat_map(|x| x.to_le_bytes()).collect()));
    img
}

fn apply(w: &mut W, sm: &mut SM, img: &[(u32, Vec<u8>)]) {
    sm.0.clear();
    for (a, v) in img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn dump(w: &mut W) -> Vec<u8> {
    let mut a = vec![0u8; 0x600];
    w.r.mem.read(w.o, &mut a).unwrap();
    let mut b = vec![0u8; (S1 - S0) as usize];
    w.r.mem.read(S0, &mut b).unwrap();
    a.extend(b);
    a
}

fn dump_sm(w: &W, sm: &SM) -> Vec<u8> {
    let mut a: Vec<u8> = (w.o..w.o + 0x600).map(|x| sm.r8(x)).collect();
    a.extend((S0..S1).map(|x| sm.r8(x)));
    a
}

fn run(seed: u64, n: u32, addr: u32, port: impl Fn(&mut SM, &mut PortEnv, u32)) {
    let Some(mut w) = world() else { return };
    let mut g = Rng(seed);
    let mut sm = SM::default();
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..n {
        let img = build(&mut g, &w);
        let oseed = g.next() as u64 | 1;
        apply(&mut w, &mut sm, &img);
        let start_state = sm.r32(w.o + 0x54);
        {
            let mut ora = w.oracle.borrow_mut();
            ora.g = Rng(oseed);
            ora.log.clear();
        }
        w.r.cpu.set_gpr(4, w.o as u64);
        w.r.call(addr).unwrap();
        let their_calls = std::mem::take(&mut w.oracle.borrow_mut().log);
        let theirs = dump(&mut w);
        let mine_ora = Rc::new(RefCell::new(Oracle { g: Rng(oseed), o: w.o, log: vec![] }));
        let mut env = PortEnv(mine_ora.clone());
        port(&mut sm, &mut env, w.o);
        let my_calls = std::mem::take(&mut mine_ora.borrow_mut().log);
        let ctx = format!("case {case} state {start_state:#x}");
        let n = my_calls.len().min(their_calls.len());
        for i in 0..n {
            let (a, b) = (&my_calls[i], &their_calls[i]);
            assert_eq!((a.0, a.1), (b.0, b.1), "{ctx}: call {i}");
            assert_eq!(a.2[..], b.2[..a.2.len()], "{ctx}: args of call {i} ({:#x})", a.1);
        }
        assert_eq!(my_calls.len(), their_calls.len(), "{ctx}: number of calls");
        let mine = dump_sm(&w, &sm);
        if mine != theirs {
            let at = mine.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
            panic!("{ctx}: memory differs at +{at:#x}: port {:#x} game {:#x}", mine[at], theirs[at]);
        }
        seen.insert(start_state);
    }
    eprintln!("states seen: {}", seen.len());
}

#[test]
fn run_state_matches() {
    run(1, 60_000, 0x1cf040, |m, e, o| run_state(m, e, o));
}

#[test]
fn update_matches() {
    run(2, 20_000, 0x1ceee0, |m, e, o| update(m, e, o));
}

#[test]
fn small_ops_match() {
    run(3, 3000, 0x1d2140, |m, e, o| restart(m, e, o));
}

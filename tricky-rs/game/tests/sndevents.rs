//! `tricky_game::sndevents` (board row G10b2c2a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random audio systems with 48 events (in use or not, free or not, countdowns around the firing point,
//! direct and vtable callbacks with adjusts), every function run on both sides; the callbacks logged (the object and
//! the event they get) with the same answers, and the system's bytes compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::sndevents::*;

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
const VTABLE: u32 = ARENA + 0x1000;
/// The callbacks (fake addresses the runner stubs)
const CALLBACKS: [u32; 4] = [0x7f_0000, 0x7f_0100, 0x7f_0200, 0x7f_0300];

type L = Rc<RefCell<(Vec<(u32, u32, u32)>, Vec<u32>)>>;

struct Ours(L);
impl EventEnv for Ours {
    fn call(&mut self, _m: &mut Mem, target: u32, this: u32, event: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.0.push((target, this, event));
        if l.1.is_empty() { 0 } else { l.1.remove(0) }
    }
}

fn snap(r: &mut Runner) -> Mem {
    let mut b = vec![0u8; 0x2000];
    r.mem.read(ARENA, &mut b).unwrap();
    let mut m = Mem::default();
    m.regions.push((ARENA, b));
    m
}

#[test]
fn the_scheduler_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let l: L = Rc::new(RefCell::new((vec![], vec![])));
    for f in CALLBACKS {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let mut l = l2.borrow_mut();
            l.0.push((f, c.gpr(4) as u32, c.gpr(5) as u32));
            let a = if l.1.is_empty() { 0 } else { l.1.remove(0) };
            c.set_gpr(2, a as u64);
        });
    }
    for f in [0x222a78u32, 0x222a88, 0x222ab8, 0x222ac8, 0x222ad8, 0x222c50, 0x2229d0, 0x222c30, 0x21ea48] {
        r.allow(f);
    }
    let mut g = Rng(0x21ea_48ee_8e8e_0001);
    for case in 0..30_000u32 {
        for k in 0..0x2000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, g.next());
        }
        r.mem.write_u32(SYS + 0x8c, (g.below(4) == 0) as u32);
        r.mem.write_u32(SYS + 0x88, g.below(3));
        // the object's vtable field (at +4 here) and its entries: (adjust, function) doublewords
        r.mem.write_u32(SYS + 4, VTABLE);
        for k in 0..8u32 {
            r.mem.write_u16(VTABLE + 8 * k, [0u16, 8, 0xfff0][g.below(3) as usize]);
            r.mem.write_u32(VTABLE + 8 * k + 4, CALLBACKS[g.below(4) as usize]);
        }
        for i in 0..COUNT {
            let e = SYS + EVENTS + EVENT_SIZE * i;
            r.mem.write_u32(e, g.below(2));
            if g.below(3) == 0 {
                r.mem.write_u32(e + 4, u32::MAX);
            } else {
                r.mem.write_u32(e + 4, [10u32, 40, 100, 0][g.below(4) as usize]);
            }
            r.mem.write_u32(e + 8, [5u32, 14, 15, 16, 100, 0, u32::MAX][g.below(7) as usize]);
            r.mem.write_u16(e + 0xc, [0u16, 4, 0xfffc][g.below(3) as usize]);
            if g.below(2) == 0 {
                r.mem.write_u16(e + 0xe, 0xffff);
                r.mem.write_u32(e + 0x10, CALLBACKS[g.below(4) as usize]);
            } else {
                r.mem.write_u16(e + 0xe, 1 + g.below(8) as u16);
                r.mem.write_u16(e + 0x10, 4);
                r.mem.write_u16(e + 0x12, g.below(3) as u16);
            }
        }
        let answers: Vec<u32> = (0..64).map(|_| [0u32, 0, 1, 5][g.below(4) as usize]).collect();
        let which = g.below(9);
        let i = g.below(COUNT);
        let e = SYS + EVENTS + EVENT_SIZE * i;
        let delay = [10u32, 40, 0, 1000][g.below(4) as usize];
        let cb = [if g.below(2) == 0 { 0xffff_0000 } else { (1 + g.below(8)) << 16 | g.below(3) }, if g.below(2) == 0 { CALLBACKS[g.below(4) as usize] } else { 4 | g.below(3) << 16 }];
        let words = [g.next(), g.next(), g.next(), g.next(), g.next()];
        let n = g.below(4);
        let mut m = snap(&mut r);
        *l.borrow_mut() = (vec![], answers.clone());
        let set = |r: &mut Runner, args: &[u64]| {
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(if k < 4 { 4 + k } else { 8 + k - 4 }, *v);
            }
        };
        let dword = cb[0] as u64 | (cb[1] as u64) << 32;
        let (addr, args): (u32, Vec<u64>) = match which {
            0 => (0x222998, vec![e as u64]),
            1 => (0x222a78, vec![e as u64]),
            2 => (0x222a88, vec![e as u64]),
            3 => (0x2229d0, vec![e as u64, delay as u64, dword, words[0] as u64, words[1] as u64, words[2] as u64, words[3] as u64, words[4] as u64]),
            4 => (0x222c50, vec![SYS as u64, n as u64]),
            5 => (0x222cc0, vec![SYS as u64, delay as u64, dword, words[0] as u64, words[1] as u64, words[2] as u64, words[3] as u64, words[4] as u64]),
            6 => (0x222df0, vec![SYS as u64, dword]),
            7 => (if g.below(2) == 0 { 0x222db0 } else { 0x222c30 }, vec![SYS as u64]),
            _ => (if g.below(2) == 0 { 0x21ea48 } else { 0x21eb10 }, if addr_is_fire(&mut g) { vec![e as u64, SYS as u64] } else { vec![SYS as u64] }),
        };
        let addr = if which == 8 { if args.len() == 2 { 0x21ea48 } else { 0x21eb10 } } else { addr };
        set(&mut r, &args);
        r.call(addr).unwrap_or_else(|x| panic!("case {case}: {addr:#x}: {x}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut l.borrow_mut().0);
        let after = snap(&mut r);
        *l.borrow_mut() = (vec![], answers);
        let env = &mut Ours(l.clone());
        let mine_ret: Option<u32> = match addr {
            0x222998 => { event_init(&mut m, e); None }
            0x222a78 => Some(event_free(&m, e) as u32),
            0x222a88 => { event_clear(&mut m, e); None }
            0x2229d0 => { event_set(&mut m, e, delay, cb, words); None }
            0x222c50 => Some(find_free(&m, SYS, n) as u32),
            0x222cc0 => Some(schedule(&mut m, SYS, delay, cb, words) as u32),
            0x222df0 => { cancel(&mut m, SYS, cb); None }
            0x222db0 => { clear_all(&mut m, SYS); None }
            0x222c30 => { monitor_tick(&mut m, SYS); None }
            0x21ea48 => { fire(&mut m, env, e, SYS); None }
            _ => { tick(&mut m, env, SYS); None }
        };
        let mine = std::mem::take(&mut l.borrow_mut().0);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        if let Some(k) = (0..0x2000).find(|&k| after.regions[0].1[k] != m.regions[0].1[k]) {
            panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", ARENA + k as u32, after.regions[0].1[k], m.regions[0].1[k]);
        }
    }
}

fn addr_is_fire(g: &mut Rng) -> bool {
    g.below(2) == 0
}

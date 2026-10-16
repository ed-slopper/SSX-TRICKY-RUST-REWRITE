//! `tricky_game::placing` (board row G8m) against `Race_UpdatePlacingsAndRubberBand` 0x115100 in the function runner
//! (TRICKY_ELF; skipped without it): random races (ticks, rider and player counts, AI counts that agree with the
//! lists and ones that do not), modes, courses, difficulties, band tables, riders' keys, places, finish times and
//! distances; every call out logged on both sides and all memory compared, the stack frame's lists included.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::placing::*;

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
struct Log {
    calls: Vec<(u32, Vec<u32>)>,
    answers: Vec<u32>,
    next: usize,
}
type L = Rc<RefCell<Log>>;
const F: [f32; 12] = [0.0, 1000.0, 52000.5, 140005.0, 150000.0, -300.0, 460746.7, 99999.0, 0.5, 30000.25, 40011.445, 40011.45];

struct Ours(L);
impl PlacingEnv for Ours {
    fn progress(&mut self, _m: &mut Mem, rider: u32) -> f32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((0x1170d0, vec![rider]));
        let v = l.answers.get(l.next).copied().unwrap_or(0);
        l.next += 1;
        F[(v % F.len() as u32) as usize]
    }
    fn placing_sound(&mut self, _m: &mut Mem, args: [u32; 6]) {
        self.0.borrow_mut().calls.push((0x219a48, args.to_vec()));
    }
}

const SP: u32 = 0x01ff_0000 - 400;
const GLOBALS: [(u32, u32); 6] = [(0x32de88, 0x20), (0x343050, 4), (PATH_SET, 0x14), (BAND_TABLES, 0x60), (SP, 0xf0), (0, 0)];

#[test]
fn placings_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let rider_size = 0x5a00u32;
    let arena = 0x400 + rider_size * 8;
    let base = r.mem.alloc(arena);
    let (race, path, riders) = (base, base + 0x300, base + 0x400);
    let l: L = Rc::new(RefCell::new(Log::default()));
    let l2 = l.clone();
    r.stub(0x1170d0, move |c, _| {
        let mut l = l2.borrow_mut();
        l.calls.push((0x1170d0, vec![c.gpr(4) as u32]));
        let v = l.answers.get(l.next).copied().unwrap_or(0);
        l.next += 1;
        c.set_f(0, F[(v % F.len() as u32) as usize]);
    });
    let l3 = l.clone();
    r.stub(0x219a48, move |c, _| {
        let v = vec![c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32];
        l3.borrow_mut().calls.push((0x219a48, v));
    });
    r.allow(0x2cfd90);
    r.allow(0x197ef0);
    let mut g = Rng(0x1151_00ee_8e8e_0001);
    let regions = |b: u32| GLOBALS.iter().copied().filter(|x| x.1 != 0).chain([(b, arena)]).collect::<Vec<_>>();
    for case in 0..40_000u32 {
        for (at, n) in regions(base) {
            for k in 0..n / 4 {
                let v = if g.below(2) == 0 { F[g.below(F.len() as u32) as usize].to_bits() } else { g.below(9) };
                r.mem.write_u32(at + 4 * k, v);
            }
        }
        // the band tables as real floats, some negative (no denormals: the EE reads them as 0)
        for k in 0..24 {
            let v = [1500.0f32, -2200.5, 0.0, 3000.0, -1.0, 420.25][g.below(6) as usize];
            r.mem.write_u32(BAND_TABLES + 4 * k, v.to_bits());
        }
        let n = 2 + g.below(5);
        r.mem.write_u32(race + 0x18, [0, 6, 12, 3][g.below(4) as usize]);
        r.mem.write_u32(race + 0x88, if g.below(10) == 0 { 1 } else { n });
        let players = g.below(3) + if g.below(8) == 0 { 0 } else { 1 };
        r.mem.write_u32(race + 0x7c, players);
        // the stack frame's leftovers as rider indices, and all eight list slots real riders, so the unchecked walks
        // (past what the lists got) read riders as they would in a race
        for k in 0..0xf0 / 4 {
            r.mem.write_u32(SP + 4 * k, g.below(8));
        }
        let mut ai = 0;
        for k in 0..8 {
            r.mem.write_u32(race + 0xc4 + 4 * k, riders + rider_size * k);
        }
        for k in 0..8 {
            let rd = riders + rider_size * k;
            r.mem.write_u32(race + 0xc4 + 4 * k, rd);
            let human = k < players && k < n;
            r.mem.write_u32(rd + 0x41c, if human { [1, 2][g.below(2) as usize] } else { 0 });
            if !human && k < n {
                ai += 1;
            }
            r.mem.write_u32(rd + 0x110, g.below(n + 1));
            r.mem.write_u32(rd + 0x418, if g.below(3) == 0 { g.below(30000) } else { u32::MAX });
            r.mem.write_u32(rd + 0x378, F[g.below(F.len() as u32) as usize].to_bits());
            for o in [0x58dcu32, 0x58e0, 0x5910] {
                r.mem.write_u32(rd + o, g.below(5000));
            }
        }
        // the AI count as built, now and then not the list's
        r.mem.write_u32(race + 0x80, if g.below(6) == 0 { g.below(n + 1) } else { ai });
        r.mem.write_u32(GAME_MODE, [0, 1, 2, 3, 4, 5, 6, 7, 2, 4, 9][g.below(11) as usize]);
        r.mem.write_u32(COURSE, g.below(13));
        r.mem.write_u16(CIRCUIT_DIFFICULTY, g.below(4) as u16);
        r.mem.write_u32(RACE_DIFFICULTY, g.below(4));
        r.mem.write_u32(PATH_SET + 0x10, path);
        r.mem.write_u32(path + 0x38, F[g.below(F.len() as u32) as usize].to_bits());
        // a players' count that does not divide the riders would trap in the game; keep the division non-zero
        if players > n {
            r.mem.write_u32(race + 0x7c, n);
        }
        // the unchecked walk of the targets' list stays in its four slots (past them it reads the game's saved
        // registers, which a port cannot have)
        let p = r.mem.read_u32(race + 0x7c);
        let step = if p == 0 { 0 } else { r.mem.read_u32(race + 0x88) / p };
        let given = r.mem.read_u32(race + 0x80);
        if step > 0 && given > 0 && (given - 1) / step >= 4 {
            r.mem.write_u32(race + 0x80, ai);
        }
        let answers: Vec<u32> = (0..32).map(|_| g.next()).collect();
        let mut m = Mem::default();
        for (at, len) in regions(base) {
            let mut b = vec![0u8; len as usize];
            r.mem.read(at, &mut b).unwrap();
            m.regions.push((at, b));
        }
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.cpu.set_gpr(4, 0);
        r.cpu.set_gpr(5, race as u64);
        r.call(0x115100).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        update(&mut m, &mut Ours(l.clone()), race, SP);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "case {case}");
        for (k, (at, len)) in regions(base).into_iter().enumerate() {
            let mut b = vec![0u8; len as usize];
            r.mem.read(at, &mut b).unwrap();
            // the frame: only the order, keys, tables and lists (the rest is the game's saved registers)
            let upto = if at == SP { 0xf0 } else { len as usize };
            if let Some(i) = (0..upto).find(|&i| b[i] != m.regions[k].1[i]) {
                let i = i & !3;
                let a = at + i as u32;
                for q in 0..5u32 {
                    let rd = riders + rider_size * q;
                    let mut f = |o: u32| (r.mem.read_u32(rd + o), m.r32(rd + o));
                    eprintln!("rider {q}: 41c {:?} 110 {:?} 378 {:?} 100 {:?} 104 {:?} 108 {:?}", f(0x41c), f(0x110), f(0x378), f(0x100), f(0x104), f(0x108));
                }
                eprintln!("path len {:x} frame {:x?}", r.mem.read_u32(path + 0x38), (0..60u32).map(|k| r.mem.read_u32(SP + 4 * k)).collect::<Vec<_>>());
                panic!("case {case}: memory at {a:#x} (rider {} +{:#x}; mode {} course {} n {} players {} ai {}): {:x?} vs {:x?}", a.wrapping_sub(riders) / rider_size, a.wrapping_sub(riders) % rider_size, r.mem.read_u32(GAME_MODE), r.mem.read_u32(COURSE), r.mem.read_u32(race + 0x88), r.mem.read_u32(race + 0x7c), r.mem.read_u32(race + 0x80), &b[i..i + 4], &m.regions[k].1[i..i + 4]);
            }
        }
    }
}

//! `tricky_game::fontprint` (board row G8j) against the shadowed prints in the function runner (TRICKY_ELF; skipped
//! without it): random fonts, strings, placements, colours and layers, every call out logged on both sides (floats
//! bit for bit, with the font colour each draw sees) and the font colour compared after.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fontprint::*;
use tricky_game::hudsprites::SpritesEnv;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;

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
impl Log {
    fn take(&mut self) -> u32 {
        let v = self.answers.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        v
    }
}
type L = Rc<RefCell<Log>>;
const R_FN: u32 = 0x7e0000;
const FONT_COLOR: u32 = 0x337fe8;
const STACK: u32 = 0x01ff_0000;

/// The calls stubbed: integer and float arguments, and whether they see the font colour
const STUBS: [(u32, usize, usize, bool); 7] = [
    (0x301ca0, 2, 0, false),
    (0x2c48e8, 2, 0, false),
    (0x19cf70, 2, 3, true),
    (0x19d0c0, 2, 3, true),
    (0x19da30, 3, 1, false),
    (0x19dc78, 3, 1, false),
    (0x17ccb8, 2, 0, false),
];

fn answer(l: &mut Log, func: u32, ints: &[u32]) -> (u32, Vec<(u32, u32)>) {
    match func {
        0x19da30 | 0x19dc78 => {
            let w = l.take() % 300;
            let out = if ints[2] != 0 { (0..4).map(|k| (ints[2] + 4 * k, l.take() % 300)).collect() } else { vec![] };
            (w, out)
        }
        f if f == R_FN + 0x1dc => ([0, 1, 2, 7, u32::MAX][(l.take() % 5) as usize], vec![]),
        _ => (0, vec![]),
    }
}

struct Ours(L);
impl SpritesEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let mut l = self.0.borrow_mut();
        let n = STUBS.iter().find(|s| s.0 == func).unwrap();
        let mut v = ints[..n.1].to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
        if n.3 {
            v.extend((0..4).map(|k| m.r32(FONT_COLOR + 4 * k)));
        }
        l.calls.push((func, v));
        let (x, out) = answer(&mut l, func, ints);
        for (at, w) in out {
            m.w32(at, w);
        }
        (x, 0.0)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, ints: &[u32], _floats: &[f32]) -> u32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((R_FN + slot, ints.to_vec()));
        answer(&mut l, R_FN + slot, ints).0
    }
}

const F: [f32; 10] = [0.0, 1.0, 0.5, 2.25, 12.0, -3.5, 100.0, 0.75, 0.1, 0.3];

#[test]
fn font_prints_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(0x400);
    let (app, ren, vt) = (base, base + 0x100, base + 0x200);
    let l: L = Rc::new(RefCell::new(Log::default()));
    for (f, ni, nf, coloured) in STUBS {
        let l2 = l.clone();
        r.stub(f, move |c, mem| {
            let ints: Vec<u32> = (0..3).map(|k| c.gpr(4 + k) as u32).collect();
            let mut v = ints[..ni].to_vec();
            v.extend((0..nf).map(|k| c.f(12 + k).to_bits()));
            if coloured {
                v.extend((0..4).map(|k| mem.read_u32(FONT_COLOR + 4 * k)));
            }
            let mut l = l2.borrow_mut();
            l.calls.push((f, v));
            let (x, out) = answer(&mut l, f, &ints);
            for (at, w) in out {
                mem.write_u32(at, w);
            }
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for s in [0x1d4u32, 0x1dc] {
        let l2 = l.clone();
        r.stub(R_FN + s, move |c, _| {
            let v = if s == 0x1dc { vec![] } else { vec![c.gpr(5) as u32] };
            let mut l = l2.borrow_mut();
            l.calls.push((R_FN + s, v));
            let x = answer(&mut l, R_FN + s, &[]).0;
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for f in [0x19d3f0u32, 0x19d568, 0x19d6e0, 0x19d770, 0x19d868, 0x19d800] {
        r.allow(f);
    }
    let mut g = Rng(0x19d3_f0ee_8e8e_0001);
    for case in 0..40_000u32 {
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x724, ren);
        r.mem.write_u32(ren + 0x10, vt);
        for s in [0x1d4u32, 0x1dc] {
            r.mem.write_u32(vt + s, R_FN + s);
            r.mem.write_u16(vt + s - 4, 0);
        }
        for k in 0..4 {
            r.mem.write_u32(FONT_COLOR + 4 * k, F[g.below(F.len() as u32) as usize].to_bits());
        }
        let which = g.below(6);
        let (font, s) = (g.next(), g.next());
        let v: Vec<f32> = (0..3).map(|_| F[g.below(F.len() as u32) as usize]).collect();
        let answers: Vec<u32> = (0..32).map(|_| g.next()).collect();
        for (k, x) in v.iter().enumerate() {
            r.cpu.set_f(12 + k, *x);
        }
        r.cpu.set_gpr(4, font as u64);
        r.cpu.set_gpr(5, s as u64);
        let mut m = Mem::default();
        let mut b = vec![0u8; 16];
        r.mem.read(FONT_COLOR, &mut b).unwrap();
        m.regions.push((FONT_COLOR, b));
        m.regions.push((STACK - 0x2000, vec![0u8; 0x2000]));
        let addr = [0x19d3f0u32, 0x19d568, 0x19d6e0, 0x19d770, 0x19d868, 0x19d800][which as usize];
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which}): {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let e = &mut Ours(l.clone());
        let (sc, x, y) = (v[0], v[1], v[2]);
        match which {
            0 => print_shadowed(&mut m, e, false, font, s, sc, x, y, STACK),
            1 => print_shadowed(&mut m, e, true, font, s, sc, x, y, STACK),
            2 => print_centred(&mut m, e, false, font, s, sc, x, y, STACK),
            3 => print_centred(&mut m, e, true, font, s, sc, x, y, STACK),
            4 => print_right(&mut m, e, font, s, sc, x, y, STACK),
            _ => print_ascii_right(&mut m, e, font, s, sc, x, y, STACK),
        }
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "case {case} ({which})");
        let mut b = vec![0u8; 16];
        r.mem.read(FONT_COLOR, &mut b).unwrap();
        assert_eq!(b, m.regions[0].1, "case {case} ({which}): the font colour");
    }
}

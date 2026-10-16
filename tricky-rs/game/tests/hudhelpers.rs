//! The HUD's drawing helpers in `tricky_game::hudsprites` (board row G8i) against the original in the function
//! runner (TRICKY_ELF; skipped without it): `HUD_DrawSprite` / `HUD_GetSpriteRect` / `SpriteSet_GetRect`,
//! `HUD_DrawBar`, `HUD_DrawNumber`, `HUD_FormatTime` / `_FormatTimeSigned` and `HUD_ShowItem` / `_HideItem` on random
//! arguments, every call out logged on both sides (floats bit for bit) and all memory compared, the frame included.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::hudsprites::*;
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
const ARENA: u32 = 0x2000;
/// `HUD_DrawNumber`'s frame (the runner's stack top less 0x150), up to its spills at +0x60
const SP: u32 = 0x01ff_0000 - 0x150;

/// The functions stubbed: integer and float arguments
const STUBS: [(u32, usize, usize); 10] = [
    (0x14eed0, 2, 4),
    (0x2fca28, 3, 0),
    (0x19da30, 3, 1),
    (0x19d3f0, 2, 3),
    (0x2c5160, 2, 0),
    (0x17ce38, 2, 0),
    (0x17ccb8, 2, 0),
    (0x17ce70, 5, 0),
    (0x1a7db8, 2, 0),
    (0x1a7e70, 2, 0),
];

fn answer(l: &mut Log, func: u32, ints: &[u32]) -> (u32, Vec<(u32, u32)>) {
    match func {
        0x19da30 => {
            let w = l.take() % 40;
            let out = if ints[2] != 0 { (0..4).map(|k| (ints[2] + 4 * k, l.take() % 40)).collect() } else { vec![] };
            (w, out)
        }
        0x2c5160 => (0x7f0000 + ints[1], vec![]),
        f if f == R_FN + 0x1dc => (l.take() % 12, vec![]),
        _ => (0, vec![]),
    }
}

struct Ours(L);
impl SpritesEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let mut l = self.0.borrow_mut();
        let mut v = ints.to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
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

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

fn setup(r: &mut Runner, l: &L) {
    for f in [0x1c0068, 0x1c0020, 0x1f9e50] {
        r.allow(f);
    }
    for (f, ni, nf) in STUBS {
        let l2 = l.clone();
        r.stub(f, move |c, mem| {
            let ints: Vec<u32> = (0..ni).map(|k| if k < 4 { arg(c, 4 + k) } else { arg(c, 8 + k - 4) }).collect();
            let mut v = ints.clone();
            v.extend((0..nf).map(|k| c.f(12 + k).to_bits()));
            let mut l = l2.borrow_mut();
            l.calls.push((f, v));
            let (x, out) = answer(&mut l, f, &ints);
            for (at, w) in out {
                mem.write_u32(at, w);
            }
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for s in [0x1d4u32, 0x1dc, 0x1e4] {
        let l2 = l.clone();
        r.stub(R_FN + s, move |c, _| {
            let v = if s == 0x1dc { vec![] } else { vec![arg(c, 5)] };
            let mut l = l2.borrow_mut();
            l.calls.push((R_FN + s, v));
            let x = answer(&mut l, R_FN + s, &[]).0;
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
}

fn snapshot(r: &mut Runner, base: u32) -> Mem {
    let mut m = Mem::default();
    for (at, n) in [(GAPP, 4u32), (SP, 0x60), (base, ARENA)] {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

const F: [f32; 13] = [0.0, 1.0, 0.5, 2.25, 12.0, -3.5, 100.0, 0.75, 37.5, 0.1, 0.3, 1.3, 0.7];

#[test]
fn hud_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(ARENA);
    let (hud, app, ren, vt, set) = (base, base + 0x1000, base + 0x200, base + 0x300, base + 0x600);
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l);
    let mut g = Rng(0x1c00_68ee_8e8e_0001);
    let numbers = [0u32, 7, 42, 1234, 99999, 1_000_000_007, u32::MAX - 5, 10, 100, 9];
    let times = [0u32, 50000, u32::MAX, 359_999, 360_000, 12_345, 6_001, u32::MAX - 6_000, 1, 99, 0x8000_0000];
    for case in 0..60_000u32 {
        for k in 0..ARENA / 4 {
            r.mem.write_u32(base + 4 * k, if g.below(2) == 0 { F[g.below(F.len() as u32) as usize].to_bits() } else { g.next() });
        }
        for k in 0..0x60 / 4 {
            r.mem.write_u32(SP + 4 * k, g.next());
        }
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x724, ren);
        r.mem.write_u32(ren + 0x10, vt);
        for s in [0x1d4u32, 0x1dc, 0x1e4] {
            r.mem.write_u32(vt + s, R_FN + s);
            r.mem.write_u16(vt + s - 4, 0);
        }
        r.mem.write_u32(hud, set);
        // the sprite rects' sizes as real floats
        for k in 0..64 {
            for o in [4u32, 8] {
                r.mem.write_u32(set + 8 + 28 * k + o, F[1 + g.below(F.len() as u32 - 1) as usize].to_bits());
            }
        }
        let which = g.below(9);
        let fl = |g: &mut Rng| F[g.below(F.len() as u32) as usize];
        let (x, y, a, b) = (fl(&mut g), fl(&mut g), fl(&mut g), fl(&mut g));
        let id = [0u32, 5, 42, 63, u32::MAX, 0x8000_0000][g.below(6) as usize];
        let flags = g.next() & 0xff;
        let n = if which == 4 { numbers[g.below(10) as usize] } else { times[g.below(11) as usize] };
        let answers: Vec<u32> = (0..256).map(|_| g.next()).collect();
        for (k, v) in [x, y, a, b].iter().enumerate() {
            r.cpu.set_f(12 + k, *v);
        }
        let buf = base + 0x1f00;
        let (addr, a0, a1, a2) = match which {
            0 => (0x1c0068u32, hud, id, flags),
            1 => (0x1f9e50, set, id, 0),
            2 => (0x1c0020, hud, id, 0),
            3 => (0x1a9f80, hud, flags, 0),
            4 => (0x1a82e8, hud, n, flags),
            5 => (0x1a6ba0, n, buf, 0),
            6 => (0x1a6c90, n, buf, 0),
            7 => (0x1a7848, hud, g.below(8), flags),
            _ => (0x1a7860, hud, g.below(8), flags),
        };
        r.cpu.set_gpr(4, a0 as i32 as i64 as u64);
        r.cpu.set_gpr(5, a1 as i32 as i64 as u64);
        r.cpu.set_gpr(6, a2 as i32 as i64 as u64);
        let mut m = snapshot(&mut r, base);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which}): {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        let (v0, f0) = (r.cpu.gpr(2) as u32, r.cpu.f(0));
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let e = &mut Ours(l.clone());
        let (mine_v0, mine_f0) = match which {
            0 => {
                draw_sprite(&mut m, e, x, y, a, b, hud, id, flags);
                (None, None)
            }
            1 => (Some(sprite_rect(set, id as i32)), None),
            2 => (Some(sprite_rect(m.r32(hud), id as i32)), None),
            3 => {
                draw_bar(&mut m, e, x, y, a, b, hud, flags);
                (None, None)
            }
            4 => (None, Some(draw_number(&mut m, e, x, y, a, b, hud, n, flags, SP))),
            5 => {
                format_time(&mut m, e, n, buf);
                (None, None)
            }
            6 => {
                format_time_signed(&mut m, e, n, buf);
                (None, None)
            }
            _ => {
                show_item(&mut m, e, a0, a1, a2, which == 7);
                (None, None)
            }
        };
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "case {case} ({which})");
        if let Some(v) = mine_v0 {
            assert_eq!(v0, v, "case {case} ({which}): v0");
        }
        if let Some(f) = mine_f0 {
            assert_eq!(f0.to_bits(), f.to_bits(), "case {case} ({which}): f0");
        }
        let theirs = snapshot(&mut r, base);
        for ((at, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                panic!("case {case} ({which}): memory at {:#x}", at + i as u32);
            }
        }
    }
}

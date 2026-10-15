//! `tricky_game::hudsprites` (board row G8g) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random view layouts, HUD blocks, riders and score entries, every call out logged on both sides
//! (floats bit for bit) and all memory compared, the frame's colour vector and strings included.

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
const RIDER: u32 = 0x5b00;
const ARENA: u32 = 0x2000 + RIDER * 5;
/// The game's frame in `HUD_DrawPlayerSprites` (the runner's stack top less 0x180)
const SP: u32 = 0x01ff_0000 - 0x180;

struct At {
    b: u32,
}
impl At {
    fn hud(&self) -> u32 {
        self.b
    }
    fn app(&self) -> u32 {
        self.b + 0x800
    }
    fn race(&self) -> u32 {
        self.b + 0x1000
    }
    fn world(&self) -> u32 {
        self.b + 0x1300
    }
    fn renderer(&self) -> u32 {
        self.b + 0x1400
    }
    fn r_vt(&self) -> u32 {
        self.b + 0x1500
    }
    fn path(&self) -> u32 {
        self.b + 0x1700
    }
    fn font(&self) -> u32 {
        self.b + 0x1740
    }
    fn rects(&self) -> u32 {
        self.b + 0x1780
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x2000 + RIDER * i
    }
}

/// FONT_COLOR .. the view layout index (the band, combo and rank tables between), the view layouts, gApp, the frame
const GLOBALS: [(u32, u32); 4] = [(0x337fe8, 0x3381e8 - 0x337fe8), (LAYOUTS, 4 * 0x424), (GAPP, 4), (SP, 0x50)];
const SLOTS: [u32; 5] = [0x1cc, 0x1d4, 0x1dc, 0x1e4, 0x1fc];

fn regions(a: &At) -> Vec<(u32, u32)> {
    let mut v = GLOBALS.to_vec();
    v.push((a.b, ARENA));
    v
}

fn snapshot(r: &mut Runner, a: &At) -> Mem {
    let mut m = Mem::default();
    for (at, n) in regions(a) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn compare(r: &mut Runner, a: &At, m: &Mem, what: &str) {
    let theirs = snapshot(r, a);
    for ((at, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

fn fpick(g: &mut Rng, xs: &[f32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize].to_bits()
}
const F: [f32; 14] = [0.0, 0.002, -0.004, 0.25, 0.5, 0.9, 1.0, 1.01, 20.0, 19.0, 1000.0, -0.5, 3000.0, 2500.5];
const D: [f32; 12] = [0.0, -30000.0, -10000.0, -4000.0, 5000.0, 19999.0, 25000.0, 2500.5, -20000.0, 20000.0, 10000.0, 7500.0];

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    for k in 0..0x2000 / 4 {
        w(r, a.b + 4 * k, if g.below(3) == 0 { fpick(g, &F) } else { 0 });
    }
    for k in 0..4 * 0x424 / 4 {
        w(r, LAYOUTS + 4 * k, fpick(g, &F));
    }
    for k in 0..4 {
        w(r, 0x337fe8 + 4 * k, fpick(g, &F));
    }
    for k in 0..0x50 / 4 {
        w(r, SP + 4 * k, g.next());
    }
    // the meter's bands: +0 the unlit value, +8 the lit sprite (its kind for the stars' colour), +0xc / +0x10 x, y
    for k in 0..14 {
        let e = 0x338008 + 20 * k;
        w(r, e, fpick(g, &F));
        w(r, e + 8, [0x12, 0x13, 5, 0x20][g.below(4) as usize]);
        w(r, e + 12, fpick(g, &F));
        w(r, e + 16, fpick(g, &F));
    }
    // the kind the stars read when no band is lit (the band before the table)
    w(r, 0x337ffc, [0x12, 0x13, 5][g.below(3) as usize]);
    w(r, RANK_MIN, fpick(g, &[0.0, 100.0, 2999.0]));
    for k in 0..6 {
        let e = COMBO_LETTERS + 12 * k;
        w(r, e, g.below(48));
        w(r, e + 4, g.below(80));
        w(r, e + 8, g.below(80));
    }
    w(r, 0x3381e4, g.below(4));
    w(r, GAPP, a.app());
    w(r, a.app() + 0x10, if g.below(2) == 0 { [36, 136, 18, 118, 126, 1026][g.below(6) as usize] } else { g.below(1000) });
    w(r, a.app() + 0x50, 0x1234);
    w(r, a.app() + 0x5c, a.font());
    w(r, a.app() + 0x724, a.renderer());
    w(r, a.app() + 0x730, a.race());
    w(r, a.font() + 0xc, [12, 16, 20, u32::MAX - 2][g.below(4) as usize]);
    w(r, a.race() + 0x1c, a.world());
    w(r, a.race() + 0x38, 0x5678);
    for i in 0..4 {
        w(r, a.race() + 0xa4 + 0x80 * i, a.rider(i));
    }
    w(r, a.world() + 0x88, g.below(5));
    w(r, a.world() + 0x14, fpick(g, &[0.0, 1.0, 1.5, 9.5, 10.99, 11.0, 3.25, 60.0]));
    for k in 0..4 {
        w(r, a.world() + 0xc4 + 4 * k, a.rider(g.below(5)));
    }
    w(r, a.renderer() + 0x10, a.r_vt());
    for s in SLOTS {
        w(r, a.r_vt() + s, R_FN + s);
        r.mem.write_u16(a.r_vt() + s - 4, 0);
    }
    w(r, a.path() + 0x38, fpick(g, &[0.0, 10000.0, -1.0]));
    for k in 0..12 {
        w(r, a.rects() + 4 * k, fpick(g, &F));
    }
    let hud = a.hud();
    w(r, hud, 0x9abc);
    w(r, hud + 0x2c, g.below(26));
    for k in 0..25 {
        w(r, hud + 0x94 + 4 * k, [0, 1, 2, u32::MAX, 3][g.below(5) as usize]);
    }
    w(r, hud + 0x104, g.next() | g.next());
    for i in 0..4 {
        let b = hud + 0x114 + 0x140 * i;
        w(r, b + 8, [0, u32::MAX - 4, 100, 3000, 99999][g.below(5) as usize]);
        if g.below(3) == 0 {
            let v = r.mem.read_u32(0x338008);
            w(r, b + 12, v);
        }
        // a quarter of the meters with no band lit
        let unlit = g.below(4) == 0;
        for k in 0..14 {
            w(r, b + 16 + 8 * k, if unlit { 0 } else { g.below(2) });
        }
        w(r, b + 0x88, fpick(g, &[0.0, 0.1, 0.25, 0.5, 0.9, -1.0, 0.43]));
        w(r, b + 0x94, g.below(4));
        w(r, b + 0x98, fpick(g, &[0.0, 0.1, 0.2, 0.5]));
        for o in [0xa4u32, 0xa8, 0xc0] {
            w(r, b + o, g.below(2));
        }
        for o in [0xacu32, 0xb0] {
            w(r, b + o, fpick(g, &[-3.5, -3.0, -2.5, -2.0, -1.5, -1.0, 0.0, 0.5, 1.0]));
        }
        w(r, b + 0xb4, g.below(5));
        w(r, b + 0xb8, fpick(g, &[0.0, 0.3, 1.0]));
        w(r, b + 0xbc, fpick(g, &[0.0, 100.0, 990.0, 500.0]));
        for k in 0..(4 + 25) {
            w(r, b + 0xc4 + 4 * k, fpick(g, &D));
        }
    }
    for i in 0..5 {
        let rd = a.rider(i);
        w(r, rd + 0x24, fpick(g, &[0.0, 5.0, 19.5, 19.8, 20.0, -1.0, 10.0]));
        w(r, rd + 0x374, fpick(g, &[0.0, -1.0, 2.0]));
        w(r, rd + 0x418, if g.below(2) == 0 { u32::MAX } else { 0 });
        w(r, rd + 0x464, rd + 0x500);
        r.mem.write_u8(rd + 0x565, [0x80, 0x10, 0xff, 0][g.below(4) as usize]);
        w(r, rd + 0x58a8, g.below(8));
        for k in 0..12 {
            let e = rd + 0x597c + 20 * k;
            w(r, e, g.below(19));
            w(r, e + 4, g.below(5000));
            w(r, e + 8, fpick(g, &[1.0, 2.0, 0.5]));
            w(r, e + 12, fpick(g, &[0.0, 0.1, 0.2, 0.3, 0.6, 1.0]));
            w(r, e + 16, g.below(5000));
        }
    }
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

/// The functions stubbed: integer and float arguments
const STUBS: [(u32, usize, usize); 17] = [
    (0x1a77a8, 1, 0),
    (0x1a77f8, 1, 0),
    (0x11d998, 1, 0),
    (0x197ef0, 1, 0),
    (0x1170d0, 1, 0),
    (0x156900, 2, 0),
    (0x2c5160, 2, 0),
    (0x17ccf8, 2, 0),
    (0x2fca28, 3, 0),
    (0x19d6e0, 2, 3),
    (0x19d770, 2, 3),
    (0x1c0068, 3, 4),
    (0x1a82e8, 3, 4),
    (0x19e7a0, 2, 0),
    (0x1f9e50, 2, 0),
    (0x1fc728, 2, 6),
    (0x1fc178, 2, 4),
];

/// The draws, which read the font colour (logged with them)
const DRAWS: [u32; 6] = [0x1c0068, 0x1a82e8, 0x19d6e0, 0x19d770, 0x1fc728, 0x1fc178];

/// The answers (v0, f0) and the rank colour `Score_GetRankForPoints` writes, the same on both sides
fn answer(l: &mut Log, func: u32, args: &[u32], base: u32) -> (u32, f32, Option<[f32; 4]>) {
    let pick = |v: u32| F[(v % F.len() as u32) as usize];
    match func {
        0x1a77a8 | 0x1a77f8 | 0x11d998 => ((l.take() % 3 == 0) as u32, 0.0, None),
        0x197ef0 => (base + 0x1700, 0.0, None),
        0x1170d0 => (0, pick(l.take()), None),
        0x156900 => (l.take() % 7, 0.0, None),
        0x2c5160 => (0x7f0000 + args[1], 0.0, None),
        0x1a82e8 => (0, pick(l.take()), None),
        0x19e7a0 => {
            let c = [pick(l.take()), pick(l.take()), pick(l.take()), pick(l.take())];
            (l.take() % 5, 0.0, Some(c))
        }
        0x1f9e50 => (base + 0x1780 + 0x10 * (args[1].wrapping_sub(56) % 3), 0.0, None),
        R_FN_1DC => (l.take() % 12, 0.0, None),
        _ => (0, 0.0, None),
    }
}
const R_FN_1DC: u32 = R_FN + 0x1dc;

struct Ours(L, u32);
impl SpritesEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let mut l = self.0.borrow_mut();
        let mut v = ints.to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
        if DRAWS.contains(&func) {
            v.extend((0..4).map(|k| m.r32(0x337fe8 + 4 * k)));
        }
        l.calls.push((func, v));
        let (x, f, c) = answer(&mut l, func, ints, self.1);
        if let Some(c) = c {
            for (k, c) in c.iter().enumerate() {
                m.w32(ints[1] + 4 * k as u32, c.to_bits());
            }
        }
        (x, f)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, ints: &[u32], floats: &[f32]) -> u32 {
        let mut l = self.0.borrow_mut();
        let mut v = ints.to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
        l.calls.push((R_FN + slot, v));
        answer(&mut l, R_FN + slot, ints, self.1).0
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [0x1a68f8, 0x250d60, 0x14eee8, 0x14f5f0, 0x1814a8] {
        r.allow(f);
    }
    let base = a.b;
    for (f, ni, nf) in STUBS {
        let l2 = l.clone();
        r.stub(f, move |c, mem| {
            let ints: Vec<u32> = (0..ni).map(|k| arg(c, 4 + k)).collect();
            let mut v = ints.clone();
            v.extend((0..nf).map(|k| c.f(12 + k).to_bits()));
            if DRAWS.contains(&f) {
                v.extend((0..4).map(|k| mem.read_u32(0x337fe8 + 4 * k)));
            }
            let mut l = l2.borrow_mut();
            l.calls.push((f, v));
            let (x, fx, col) = answer(&mut l, f, &ints, base);
            if let Some(col) = col {
                for (k, cv) in col.iter().enumerate() {
                    mem.write_u32(ints[1] + 4 * k as u32, cv.to_bits());
                }
            }
            c.set_gpr(2, x as i32 as i64 as u64);
            c.set_f(0, fx);
        });
    }
    for s in SLOTS {
        let l2 = l.clone();
        r.stub(R_FN + s, move |c, _| {
            let v = match s {
                0x1dc => vec![],
                0x1d4 | 0x1e4 => vec![arg(c, 5)],
                0x1fc => vec![arg(c, 5), arg(c, 6)],
                _ => vec![arg(c, 5), arg(c, 6), c.f(12).to_bits()],
            };
            let mut l = l2.borrow_mut();
            l.calls.push((R_FN + s, v));
            let x = answer(&mut l, R_FN + s, &[], base).0;
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn player_sprites_match_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1a2c_40ee_8e8e_0001);
    let hud = a.hud();
    for case in 0..30_000u32 {
        build(&mut g, &mut r, &a);
        let i = g.below(4);
        let (top, left, width) = (
            f32::from_bits(fpick(&mut g, &[0.0, 240.0, 12.5])),
            f32::from_bits(fpick(&mut g, &[0.0, 320.0, 7.25])),
            f32::from_bits(fpick(&mut g, &[640.0, 320.0])),
        );
        let answers: Vec<u32> = (0..512).map(|_| g.next()).collect();
        r.cpu.set_f(12, top);
        r.cpu.set_f(13, left);
        r.cpu.set_f(14, width);
        r.cpu.set_gpr(4, hud as u64);
        r.cpu.set_gpr(5, i as u64);
        let mut m = snapshot(&mut r, &a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(0x1a2c40).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        draw_player_sprites(&mut m, &mut Ours(l.clone(), a.b), top, left, width, hud, i, SP);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        if theirs != mine {
            let i = (0..theirs.len().min(mine.len())).find(|&i| theirs[i] != mine[i]).unwrap_or(theirs.len().min(mine.len()));
            panic!("case {case}: call {i} of {}/{}: {:x?} vs {:x?}", theirs.len(), mine.len(), theirs.get(i), mine.get(i));
        }
        compare(&mut r, &a, &m, &format!("case {case}"));
    }
}

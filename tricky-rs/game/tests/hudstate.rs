//! `tricky_game::hudstate` (board row G8e) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random races, riders, race paths and HUD blocks, every call out logged on both sides (floats bit
//! for bit) and all memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::hudstate::*;
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
const RIDER: u32 = 0x5a00;
const ARENA: u32 = 0x4000 + RIDER * 5;

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
        self.b + 0x900
    }
    fn world(&self) -> u32 {
        self.b + 0xd00
    }
    fn scene(&self) -> u32 {
        self.b + 0xe00
    }
    fn renderer(&self) -> u32 {
        self.b + 0x1400
    }
    fn r_vt(&self) -> u32 {
        self.b + 0x1500
    }
    fn paths(&self) -> u32 {
        self.b + 0x1800
    }
    fn markers(&self, p: u32) -> u32 {
        self.b + 0x1a00 + 0x200 * p
    }
    fn pages(&self) -> u32 {
        self.b + 0x2400
    }
    fn lap(&self) -> u32 {
        self.b + 0x2800
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x4000 + RIDER * i
    }
}

const GLOBALS: [(u32, u32); 9] = [
    (0x32de88, 8),
    (0x32ca18, 4),
    (0x32ca40, 4),
    (0x3381e4, 4),
    (0x343050, 4),
    (0x337fb8, 0x10),
    (0x338008, 14 * 20),
    (GAPP, 4),
    (0, 0),
];

fn regions(a: &At) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = GLOBALS.iter().copied().filter(|r| r.1 != 0).collect();
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
            let d: Vec<String> = (0..x.len() / 4).filter(|&k| x[4 * k..4 * k + 4] != y[4 * k..4 * k + 4]).take(12).map(|k| format!("{:x}: {:x?} vs {:x?}", at + 4 * k as u32, &x[4 * k..4 * k + 4], &y[4 * k..4 * k + 4])).collect();
            eprintln!("{d:#?}");
            let i = i & !3;
            panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

fn fpick(g: &mut Rng, xs: &[f32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize].to_bits()
}
const F: [f32; 14] = [0.0, 0.002, -0.004, 0.25, 0.5, 0.9, 1.0, 1.01, 20.0, 19.0, 1000.0, -0.5, 3000.0, 2500.5];

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..0x4000 / 4 {
        // floats as the game holds them (no denormals: the EE reads them as 0); the integer fields are set below
        r.mem.write_u32(a.b + 4 * k, if g.below(3) == 0 { fpick(g, &F) } else { 0 });
    }
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    w(r, 0x32de8c, [3, 5, 6, 2, 3][g.below(5) as usize]);
    w(r, 0x32de88, [8, 0, 8][g.below(3) as usize]);
    w(r, 0x32ca18, g.below(2));
    w(r, 0x32ca40, g.below(3));
    w(r, 0x3381e4, g.below(3));
    w(r, GAPP, a.app());
    w(r, a.app() + 0x730, a.race());
    w(r, a.app() + 0x724, a.renderer());
    w(r, a.race() + 0x1c, a.world());
    // the tutorial (mode 6) reads the scene's +0x50c without a null check
    let tutorial = r.mem.read_u32(0x32de8c) == 6;
    w(r, a.race() + 0x34, if !tutorial && g.below(3) == 0 { 0 } else { a.scene() });
    w(r, a.scene() + 0xc, g.below(9));
    w(r, a.scene() + 0x50c, [5, 4][g.below(2) as usize]);
    w(r, a.race() + 0x298, g.below(3) + 1);
    for i in 0..3 {
        for k in 0..4 {
            w(r, a.race() + 0x90 + 0x80 * i + 4 * k, [0, 320, 240, 640, 480, 1][g.below(6) as usize]);
        }
    }
    w(r, a.world() + 0x88, g.below(5));
    w(r, a.world() + 0x14, fpick(g, &[0.0, 1.0, 9.5, 10.0, 10.99, 11.0, 3.2, 60.0]));
    for i in 0..4 {
        w(r, a.race() + 0xa4 + 0x80 * i, if g.below(4) == 0 { 0 } else { a.rider(i) });
    }
    w(r, a.renderer() + 0x10, a.r_vt());
    for s in [0x5cu32, 0x64, 0x1d4] {
        w(r, a.r_vt() + s, R_FN + s);
        r.mem.write_u16(a.r_vt() + s - 4, 0);
    }
    // the race paths: up to three, each with a few markers of kinds 12 / 13 / other
    w(r, 0x337fc0, g.below(4));
    w(r, 0x337fc4, a.paths());
    for p in 0..3 {
        let path = a.paths() + 0x3c * p;
        w(r, path, g.below(6));
        w(r, path + 4, a.markers(p));
        w(r, path + 0x38, fpick(g, &[10000.0, 25000.5, 3000.0]));
        for k in 0..6 {
            let mk = a.markers(p) + 16 * k;
            w(r, mk, [12, 13, 12, 7][g.below(4) as usize]);
            w(r, mk + 8, fpick(g, &[0.0, 100.0, 4000.0, 4300.0, 9000.0, 24800.0]));
        }
    }
    w(r, a.lap() + 0x38, fpick(g, &[10000.0, 600.0]));
    // the HUD's markers, medals and flags
    let hud = a.hud();
    w(r, hud + 0x2c, g.below(10));
    for k in 0..25 {
        w(r, hud + 0x30 + 4 * k, fpick(g, &[0.0, 4000.0, 6100.0, 9000.0]));
        w(r, hud + 0x94 + 4 * k, [0, 2, 1, u32::MAX][g.below(4) as usize]);
    }
    w(r, hud + 0xf8, 30);
    w(r, hud + 0xfc, 20);
    w(r, hud + 0x100, 10);
    for o in [0x104u32, 0x108, 0x10c, 0x110] {
        w(r, hud + o, if g.below(2) == 0 { 0x10000 } else { 0 });
    }
    w(r, hud + 0x614, fpick(g, &[0.0, 10.0, 11.0, 5.0, 2.0]));
    for i in 0..4 {
        let b = hud + 0x114 + 0x140 * i;
        w(r, b + 4, [0, 1050, 3199, 3260, 99][g.below(5) as usize]);
        w(r, b + 0x9c, g.below(55));
        w(r, b + 0x94, g.below(4));
        w(r, b + 0x8c, g.below(2));
    }
    for i in 0..5 {
        let rd = a.rider(i);
        for k in 0..0x600 / 4 {
            w(r, rd + 4 * k, if g.below(3) == 0 { fpick(g, &F) } else { 0 });
        }
        w(r, rd + 0x418, if g.below(2) == 0 { u32::MAX } else { 0 });
        for o in [0x110u32, 0x1b4, 0x290] {
            w(r, rd + o, g.below(3));
        }
        w(r, rd + 0x540, g.below(3));
        w(r, rd + 0x428, [19, 3][g.below(2) as usize]);
        w(r, rd + 0x464, rd + 0x500);
        for k in 0..4 {
            w(r, rd + 0x150 + 4 * k, fpick(g, &[0.0, 1000.0, -2000.0, 3000.0, 3319.0, 100.0]));
        }
        w(r, rd + 0x380, fpick(g, &[0.0, 5000.0, 9000.5]));
        w(r, rd + 0x5910, g.below(40));
        for k in 0..12 {
            let e = rd + 0x597c + 20 * k;
            w(r, e, g.below(5));
            w(r, e + 8, fpick(g, &[1.0, 2.0, 0.5]));
            w(r, e + 12, fpick(g, &[0.0, 0.1, 0.2, 0.3, 1.0]));
        }
    }
    // the overlay pages the draw checks: the replay page's backdrop +0xc, results +4 → +0x430, pause +0x430
    let pg = a.pages();
    w(r, pg + 0x42c, pg + 0x100);
    w(r, pg + 0x10c, (g.below(4) == 0) as u32);
    w(r, pg + 4, pg + 0x200);
    w(r, pg + 0x630, [2, 1, 0][g.below(3) as usize]);
    w(r, pg + 0x430, [2, 1, 0][g.below(3) as usize]);
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

struct Ours(L, u32);
impl HudEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((func, args.to_vec()));
        answer(&mut l, func, args, self.1)
    }
    fn multiplier(&mut self, _m: &mut Mem, score: u32) -> f32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((0x155410, vec![score]));
        f32::from_bits(F[(l.take() % F.len() as u32) as usize].to_bits())
    }
    fn progress(&mut self, _m: &mut Mem, rider: u32) -> f32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((0x1170d0, vec![rider]));
        F[(l.take() % F.len() as u32) as usize]
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, args: &[u32]) -> u32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((R_FN + slot, args.to_vec()));
        if slot == 0x1d4 { 0 } else { size(l.take()) }
    }
    fn draw_player(&mut self, _m: &mut Mem, func: u32, x: f32, y: f32, w: f32, h: f32, hud: u32, i: u32) {
        self.0.borrow_mut().calls.push((func, vec![x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits(), hud, i]));
    }
}

fn size(v: u32) -> u32 {
    [640, 512, 448, 0xffff_ff00, 1][(v % 5) as usize]
}

/// The answers of the direct calls, the same on both sides
fn answer(l: &mut Log, func: u32, args: &[u32], base: u32) -> u32 {
    match func {
        0x1a77f8 => (l.take() % 4 == 0) as u32,
        0x197ef0 => base + 0x2800,
        0x1814a8 => base + 0x4000 + RIDER * (args[1] % 5),
        0x155440 => [0, 1000, 3200, 3300, 40, 5000, 1050, 3260][(l.take() % 8) as usize],
        0x1554c0 => l.take() % 500,
        0x1a7be0 | 0x1a7c88 | 0x1a7c58 => base + 0x2400,
        _ => 0,
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [0x1a1958, 0x1a1960, 0x1a1968, 0x1a1e50, 0x1a68f8, 0x250d60, 0x11ea20, 0x1a1eb0, 0x17f158, 0x1a6860, 0x17f190, 0x1a88f8] {
        r.allow(f);
    }
    let base = a.b;
    for (f, n) in [(0x1a77f8u32, 1usize), (0x197ef0, 1), (0x1814a8, 2), (0x155440, 1), (0x1554c0, 1), (0x1a7be0, 1), (0x1a7c88, 1), (0x1a7c58, 1), (0x233ff0, 1), (0x216930, 1), (0x21a210, 1)] {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let v: Vec<u32> = (0..n).map(|k| arg(c, 4 + k)).collect();
            let mut l = l2.borrow_mut();
            l.calls.push((f, v.clone()));
            let x = answer(&mut l, f, &v, base);
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for f in [0x155410u32, 0x1170d0] {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let mut l = l2.borrow_mut();
            l.calls.push((f, vec![arg(c, 4)]));
            let x = F[(l.take() % F.len() as u32) as usize];
            c.set_f(0, x);
        });
    }
    for s in [0x5cu32, 0x64, 0x1d4] {
        let l2 = l.clone();
        r.stub(R_FN + s, move |c, _| {
            let mut l = l2.borrow_mut();
            let args = if s == 0x1d4 { vec![arg(c, 5)] } else { vec![] };
            l.calls.push((R_FN + s, args));
            let x = if s == 0x1d4 { 0 } else { size(l.take()) };
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for f in [0x1a2c40u32, 0x1a4f70] {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let v = vec![c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits(), c.f(15).to_bits(), arg(c, 4), arg(c, 5)];
            l2.borrow_mut().calls.push((f, v));
        });
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn hud_state_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1a1f_18ee_8e8e_0001);
    let hud = a.hud();
    for case in 0..40_000u32 {
        build(&mut g, &mut r, &a);
        let which = g.below(6);
        let answers: Vec<u32> = (0..128).map(|_| g.next()).collect();
        let (addr, extra) = match which {
            0 => (0x1a1958u32, 0),
            1 => (0x1a1960, 0),
            2 | 3 => (0x1a1f18, 0),
            4 => (0x1a27d0, 0),
            _ => (0x1a2a00, 0),
        };
        let _ = extra;
        r.cpu.set_gpr(4, hud as u64);
        let mut m = snapshot(&mut r, &a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which}): {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let mut e = Ours(l.clone(), a.b);
        match which {
            0 | 1 => reset(&mut m, &mut e, hud),
            2 | 3 => update(&mut m, &mut e, hud),
            4 => draw(&mut m, &mut e, hud, false),
            _ => draw(&mut m, &mut e, hud, true),
        }
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        if theirs != mine {
            let i = (0..theirs.len().min(mine.len())).find(|&i| theirs[i] != mine[i]).unwrap_or(theirs.len().min(mine.len()));
            panic!("case {case} ({which}): call {i}: {:x?} vs {:x?}", theirs.get(i), mine.get(i));
        }
        compare(&mut r, &a, &m, &format!("case {case} ({which})"));
    }
}

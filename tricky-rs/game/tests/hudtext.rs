//! `tricky_game::hudtext` (board row G8h) against the original in the function runner (TRICKY_ELF; skipped without
//! it): random view layouts, HUD blocks, riders, score entries and race states, every call out logged on both sides
//! (floats bit for bit, the font colour and the string's first bytes with every print and measure) and all memory
//! compared, the frame's strings and colour vectors included.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::hudsprites::{SpritesEnv, LAYOUTS};
use tricky_game::hudtext::*;
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
/// The tricky meter object's two virtuals
const V_FN: u32 = 0x7d0000;
const RIDER: u32 = 0x5b00;
const ARENA: u32 = 0x4400 + RIDER * 5;
/// The game's frame in `HUD_DrawPlayerText` (the runner's stack top less 0x210), up to its spills at +0x104
const SP: u32 = 0x01ff_0000 - 0x210;
const FONT_COLOR: u32 = 0x337fe8;

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
    fn font(&self) -> u32 {
        self.b + 0x1740
    }
    fn font2(&self) -> u32 {
        self.b + 0x1760
    }
    fn pause(&self) -> u32 {
        self.b + 0x1780
    }
    fn meter(&self) -> u32 {
        self.b + 0x1800
    }
    fn meter_vt(&self) -> u32 {
        self.b + 0x2100
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x4400 + RIDER * i
    }
}
/// The localised strings `Loc_GetString` hands out (2 bytes per id)
fn string(base: u32, id: u32) -> u32 {
    base + 0x2200 + 2 * (id & 0xfff)
}

const GLOBALS: [(u32, u32); 7] =
    [(FONT_COLOR, 0x10), (0x3381e4, 4), (LAYOUTS, 4 * 0x424), (GAPP, 4), (GAP_CLOCK, 4), (UNITS, 4), (SP, 0x100)];

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

fn build(g: &mut Rng, r: &mut Runner, a: &At, i: u32) {
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    for k in 0..0x4400 / 4 {
        w(r, a.b + 4 * k, if g.below(3) == 0 { fpick(g, &F) } else { 0 });
    }
    for k in 0..4 * 0x424 / 4 {
        // real floats (no denormals: the EE reads them as 0), their low bits doubling as the layout's flags
        let v = fpick(g, &F);
        let v = if v != 0 { v | g.below(0x80) } else { 0 };
        w(r, LAYOUTS + 4 * k, v);
    }
    for k in 0..4 {
        w(r, FONT_COLOR + 4 * k, fpick(g, &F));
    }
    for k in 0..0x100 / 4 {
        w(r, SP + 4 * k, g.next());
    }
    let view = g.below(4);
    w(r, 0x3381e4, view);
    w(r, GAP_CLOCK, [0, 150, 999, 1250, 3000, 60][g.below(6) as usize]);
    w(r, UNITS, g.below(2));
    w(r, GAPP, a.app());
    w(r, a.app() + 0x50, 0x1234);
    w(r, a.app() + 0x5c, a.font());
    w(r, a.app() + 0x60, a.font2());
    w(r, a.app() + 0x724, a.renderer());
    w(r, a.app() + 0x730, a.race());
    w(r, a.font() + 0xc, [12, 16, 20][g.below(3) as usize]);
    w(r, a.font2() + 0xc, [12, 16, 20][g.below(3) as usize]);
    w(r, a.race() + 0x1c, a.world());
    w(r, a.race() + 0x34, if g.below(3) == 0 { 0x77 } else { 0 });
    w(r, a.race() + 0x38, 0x5678);
    w(r, a.race() + 0x40, if g.below(2) == 0 { 0 } else { a.pause() });
    w(r, a.race() + 0x298, g.below(2) + 1);
    for k in 0..4 {
        w(r, a.race() + 0xa4 + 0x80 * k, a.rider(k));
    }
    w(r, a.world() + 0x1c, [3, 0, 1][g.below(3) as usize]);
    w(r, a.world() + 0x88, [1, 2, 4, 1][g.below(4) as usize]);
    w(r, a.world() + 0x18, g.next());
    w(r, a.pause() + 0x40, g.below(2));
    w(r, a.pause() + 0x20, g.below(2));
    w(r, a.pause() + 0x3c, [u32::MAX, 0, 3][g.below(3) as usize]);
    w(r, a.renderer() + 0x10, a.r_vt());
    w(r, a.r_vt() + 0x1d4, R_FN + 0x1d4);
    r.mem.write_u16(a.r_vt() + 0x1d0, 0);
    w(r, a.meter() + 0x7f8 + 36, a.meter_vt());
    r.mem.write_u16(a.meter_vt() + 0x18, 8);
    w(r, a.meter_vt() + 0x1c, V_FN + 0x1c);
    r.mem.write_u16(a.meter_vt() + 0x30, 0xfff0);
    w(r, a.meter_vt() + 0x34, V_FN + 0x34);
    let hud = a.hud();
    let mut flags = g.next() | g.next();
    // the second player's slot empty now and then (never read when it is the player, or by the race time in view 1)
    if i != 1 && g.below(4) == 0 {
        w(r, a.race() + 0x124, 0);
        flags &= !1;
    }
    w(r, hud + 0x104, flags);
    w(r, hud + 0x2c, g.below(26));
    for k in 0..25 {
        w(r, hud + 0x94 + 4 * k, [u32::MAX - 1, u32::MAX, 0, 1, 2, 3][g.below(6) as usize]);
    }
    for o in [0xf8u32, 0xfc, 0x100] {
        w(r, hud + o, g.below(9000));
    }
    for p in 0..4 {
        let b = hud + 0x114 + 0x140 * p;
        w(r, b + 0x84, fpick(g, &[1.0, 2.0, 0.5]));
        w(r, b + 0x90, fpick(g, &[0.0, 0.1, 0.2, 0.3]));
        w(r, b + 0x94, [u32::MAX, 0, 1, 2, 3, 4][g.below(6) as usize]);
        w(r, b + 0x98, fpick(g, &[0.0, 0.1, 0.3, -0.0, -0.1]));
        for k in 0..25 {
            w(r, b + 0xdc + 4 * k, fpick(g, &D));
        }
    }
    for k in 0..5 {
        let rd = a.rider(k);
        w(r, rd + 0x24, fpick(g, &[0.0, 2.0, 5.0, 7.0, -1.0]));
        w(r, rd + 0x28, g.below(4));
        w(r, rd + 0x110, [0, 1, 2, 5][g.below(4) as usize]);
        for o in 0..4 {
            w(r, rd + 0x150 + 4 * o, fpick(g, &[0.0, 1000.0, -2000.0, 30.0, 0.5, 123.4, 4567.8, -891.1]));
        }
        w(r, rd + 0x418, if g.below(2) == 0 { u32::MAX } else { 0 });
        w(r, rd + 0x464, rd + 0x500);
        w(r, rd + 0x538, g.next());
        r.mem.write_u8(rd + 0x565, [0x80, 0x10, 0xff, 0, 3][g.below(5) as usize]);
        w(r, rd + 0x870, g.next());
        w(r, rd + 0x5910, g.below(10));
        for e in 0..12 {
            let e = rd + 0x597c + 20 * e;
            w(r, e, g.below(15));
            w(r, e + 4, [u32::MAX - 300, 1, 2, 3, 5, 120, 4][g.below(7) as usize]);
            w(r, e + 8, fpick(g, &[1.0, 2.0, 0.5]));
            w(r, e + 12, fpick(g, &[0.0, 0.1, 0.3, 0.6, 0.9, 1.0]));
        }
    }
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

/// The functions stubbed: their integer and float arguments
const STUBS: [(u32, usize, usize); 22] = [
    (0x1a77a8, 1, 0),
    (0x11ea28, 1, 0),
    (0x1a6ba0, 2, 0),
    (0x1a6c90, 2, 0),
    (0x2fca28, 3, 0),
    (0x17ce38, 3, 0),
    (0x17cc80, 2, 0),
    (0x17ce70, 3, 0),
    (0x19dc78, 3, 1),
    (0x19da30, 3, 1),
    (0x19d568, 2, 3),
    (0x19d3f0, 2, 3),
    (0x19d770, 2, 3),
    (0x1a82e8, 3, 4),
    (0x155538, 2, 0),
    (0x1a6d38, 3, 4),
    (0x164840, 2, 0),
    (0x1148e8, 2, 0),
    (0x11f430, 1, 0),
    (0x1a7c58, 1, 0),
    (V_FN + 0x1c, 4, 0),
    (V_FN + 0x34, 2, 2),
];
/// Those that read the font colour, and those whose a1 is a string (its first bytes logged)
const COLOURED: [u32; 6] = [0x19d568, 0x19d3f0, 0x19d770, 0x1a82e8, 0x1a6d38, V_FN + 0x34];
const STRING_A1: [u32; 5] = [0x19d568, 0x19d3f0, 0x19d770, 0x19dc78, 0x19da30];

/// The arguments worth logging: sprintf's and `WStr_Format`'s third only with a format that takes it
fn trim(func: u32, mut ints: Vec<u32>, base: u32) -> Vec<u32> {
    let keep = match func {
        0x2fca28 => ints[1] == INT_FORMAT,
        0x17ce38 => ints[1] == string(base, 0x116) || ints[1] == string(base, 0xe29),
        _ => true,
    };
    if !keep {
        ints.truncate(2);
    }
    ints
}

/// The answers (v0, f0) and what a call writes (`Font_Measure*`'s size), the same on both sides
fn answer(l: &mut Log, func: u32, ints: &[u32], base: u32) -> (u32, f32, Vec<(u32, u32)>) {
    let pick = |v: u32| F[(v % F.len() as u32) as usize];
    match func {
        0x1a77a8 | 0x11f430 => (l.take() % 3 == 0) as u32,
        0x11ea28 => [u32::MAX - 40, 0, 999, 1500, 1500, 4321][(l.take() % 6) as usize],
        0x19dc78 | 0x19da30 => {
            let w = l.take() % 300;
            let out = if ints[2] != 0 { (0..4).map(|k| (ints[2] + 4 * k, l.take() % 300)).collect() } else { vec![] };
            return (w, 0.0, out);
        }
        0x1a82e8 => return (0, pick(l.take()), vec![]),
        0x155538 => return (if l.take() % 3 == 0 { base + 0x17c0 } else { 0 }, 0.0, vec![]),
        0x164840 => l.take() % 12,
        0x1148e8 => base + 0x4400 + RIDER * (l.take() % 5),
        0x1a7c58 => base + 0x1800,
        0x2c5160 => string(base, ints[1]),
        R_FN_1D4 => 0,
        _ => 0,
    }
    .into_answer()
}
const R_FN_1D4: u32 = R_FN + 0x1d4;
trait IntoAnswer {
    fn into_answer(self) -> (u32, f32, Vec<(u32, u32)>);
}
impl IntoAnswer for u32 {
    fn into_answer(self) -> (u32, f32, Vec<(u32, u32)>) {
        (self, 0.0, vec![])
    }
}
impl IntoAnswer for (u32, f32, Vec<(u32, u32)>) {
    fn into_answer(self) -> (u32, f32, Vec<(u32, u32)>) {
        self
    }
}

/// The colour and the string's first 8 bytes a call sees
fn extras(func: u32, a1: u32, r32: &dyn Fn(u32) -> u32) -> Vec<u32> {
    let mut v = vec![];
    if COLOURED.contains(&func) {
        v.extend((0..4).map(|k| r32(FONT_COLOR + 4 * k)));
    }
    if STRING_A1.contains(&func) {
        v.extend([r32(a1), r32(a1 + 4)]);
    }
    v
}

struct Ours(L, u32);
impl SpritesEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let mut l = self.0.borrow_mut();
        let mut v = trim(func, ints.to_vec(), self.1);
        v.extend(floats.iter().map(|f| f.to_bits()));
        let a1 = ints.get(1).copied().unwrap_or(0);
        v.extend(extras(func, a1, &|a| m.r32(a)));
        if func != 0x2c5160 {
            l.calls.push((func, v));
        }
        let (x, f, out) = answer(&mut l, func, ints, self.1);
        for (at, w) in out {
            m.w32(at, w);
        }
        (x, f)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, ints: &[u32], _floats: &[f32]) -> u32 {
        self.0.borrow_mut().calls.push((R_FN + slot, ints.to_vec()));
        0
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [0x1a68f8, 0x250d60, 0x14eee8, 0x14f5f0] {
        r.allow(f);
    }
    let base = a.b;
    let mut all = STUBS.to_vec();
    all.push((0x2c5160, 2, 0));
    for (f, ni, nf) in all {
        let l2 = l.clone();
        r.stub(f, move |c, mem| {
            let ints: Vec<u32> = (0..ni).map(|k| arg(c, 4 + k)).collect();
            let mut v = trim(f, ints.clone(), base);
            v.extend((0..nf).map(|k| c.f(12 + k).to_bits()));
            let a1 = ints.get(1).copied().unwrap_or(0);
            let strs: Vec<u32> = if STRING_A1.contains(&f) { vec![a1, a1 + 4] } else { vec![] };
            let snap: Vec<(u32, u32)> = (0..4).map(|k| FONT_COLOR + 4 * k).chain(strs).map(|at| (at, mem.read_u32(at))).collect();
            v.extend(extras(f, a1, &|at| snap.iter().find(|s| s.0 == at).map(|s| s.1).unwrap_or(0)));
            let mut l = l2.borrow_mut();
            if f != 0x2c5160 {
                l.calls.push((f, v));
            }
            let (x, fx, out) = answer(&mut l, f, &ints, base);
            for (at, w) in out {
                mem.write_u32(at, w);
            }
            c.set_gpr(2, x as i32 as i64 as u64);
            c.set_f(0, fx);
        });
    }
    let l2 = l.clone();
    r.stub(R_FN + 0x1d4, move |c, _| {
        l2.borrow_mut().calls.push((R_FN + 0x1d4, vec![arg(c, 5)]));
        c.set_gpr(2, 0);
    });
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn player_text_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1a4f_70ee_8e8e_0001);
    let hud = a.hud();
    for case in 0..30_000u32 {
        let i = g.below(4);
        build(&mut g, &mut r, &a, i);
        let pick = |g: &mut Rng, xs: &[f32]| f32::from_bits(fpick(g, xs));
        let (top, left, width, height) =
            (pick(&mut g, &[0.0, 240.0, 12.5]), pick(&mut g, &[0.0, 320.0, 7.25]), pick(&mut g, &[640.0, 320.0]), pick(&mut g, &[480.0, 240.0, 437.3, 223.7]));
        let answers: Vec<u32> = (0..512).map(|_| g.next()).collect();
        r.cpu.set_f(12, top);
        r.cpu.set_f(13, left);
        r.cpu.set_f(14, width);
        r.cpu.set_f(15, height);
        r.cpu.set_gpr(4, hud as u64);
        r.cpu.set_gpr(5, i as u64);
        let mut m = snapshot(&mut r, &a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(0x1a4f70).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        draw_player_text(&mut m, &mut Ours(l.clone(), a.b), top, left, width, height, hud, i, SP);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        if theirs != mine {
            let v = m.r32(0x3381e4);
            let lay = |o: u32| m.r32(LAYOUTS + v * 0x424 + o);
            eprintln!("i {i} flags {:x} view {v} world1c {} s1_12 {:x} s3_12 {:x} gap {}", m.r32(hud + 0x104), m.r32(a.world() + 0x1c), lay(0x64 + 12), lay(0x3fc + 12), m.r32(GAP_CLOCK));
            let i = (0..theirs.len().min(mine.len())).find(|&i| theirs[i] != mine[i]).unwrap_or(theirs.len().min(mine.len()));
            panic!("case {case}: call {i} of {}/{}: {:x?} vs {:x?}
{:x?}
{:x?}", theirs.len(), mine.len(), theirs.get(i), mine.get(i), &theirs[..i + 1], &mine[..(i + 1).min(mine.len())]);
        }
        compare(&mut r, &a, &m, &format!("case {case}"));
    }
}

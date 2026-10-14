//! `tricky_game::overlays` (board row G8b) against the original in the function runner (TRICKY_ELF; skipped
//! without it): a manager with nineteen random overlays on fake vtables, every virtual and direct call logged on
//! both sides (its integer and float arguments) and all memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::overlays::*;
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
    calls: Vec<(u32, u32, Vec<u32>)>,
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

/// The fake functions the vtables point at: an overlay's slot at `OV_FN + slot`, the renderer's at `R_FN + slot`
const OV_FN: u32 = 0x7d0000;
const R_FN: u32 = 0x7e0000;
/// The integer and float arguments each virtual takes
fn arity(func: u32) -> (usize, usize) {
    match func {
        f if f == OV_FN + 0xc || f == OV_FN + 0x8c => (1, 0),
        f if f == R_FN + 0x1c4 || f == R_FN + 0x1d4 || f == R_FN + 0x1e4 => (1, 0),
        f if f == R_FN + 0x1cc => (2, 1),
        f if f == R_FN + 0x9c => (0, 4),
        f if f == R_FN + 0xb4 => (0, 6),
        _ => (0, 0),
    }
}
const OV_SLOTS: [u32; 17] = [0xc, 0x14, 0x1c, 0x24, 0x2c, 0x34, 0x3c, 0x44, 0x4c, 0x5c, 0x64, 0x6c, 0x74, 0x7c, 0x8c, 0x94, 0x9c];
const R_SLOTS: [u32; 12] = [0x5c, 0x64, 0x8c, 0x94, 0x9c, 0xb4, 0x10c, 0x1c4, 0x1cc, 0x1d4, 0x1dc, 0x1e4];

/// The direct calls the port makes and the arguments each takes
const CALLS_N: [(u32, usize); 16] = [
    (0x17cf50, 1),
    (0x19e888, 1),
    (0x1af470, 2),
    (0x1b6b90, 2),
    (0x1b3f50, 2),
    (0x1eb5d0, 1),
    (0x1ab9f8, 2),
    (0x1ef610, 2),
    (0x1b38f0, 2),
    (0x1b58c0, 2),
    (0x1b8568, 2),
    (0x1bd190, 2),
    (0x1f4440, 1),
    (0x1f99f8, 3),
    (0x1f10c0, 1),
    (0x17cfe0, 2),
];

const ARENA: u32 = 0x20000;
struct At {
    b: u32,
}
impl At {
    fn man(&self) -> u32 {
        self.b
    }
    fn ov(&self, i: u32) -> u32 {
        self.b + 0x100 + 0x40 * i
    }
    fn ov_vt(&self, k: u32) -> u32 {
        self.b + 0x800 + 0x100 * k
    }
    fn r(&self) -> u32 {
        self.b + 0xc00
    }
    fn r_vt(&self) -> u32 {
        self.b + 0xd00
    }
    fn app(&self) -> u32 {
        self.b + 0xf00
    }
    fn race(&self) -> u32 {
        self.b + 0x1000
    }
    fn input(&self) -> u32 {
        self.b + 0x1400
    }
    fn pad(&self, i: u32) -> u32 {
        self.b + 0x1500 + 0x20 * i
    }
    /// the blocks `EAMem_Alloc` hands out, in turn
    fn block(&self, k: u32) -> u32 {
        self.b + 0x2000 + 0x1000 * k
    }
}

fn regions(a: &At) -> Vec<(u32, u32)> {
    vec![(0x32de8c, 4), (0x337fe8, 0x14), (PAD, 4), (GAPP, 4), (a.b, ARENA)]
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

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..0x2000 / 4 {
        r.mem.write_u32(a.b + 4 * k, if g.below(3) == 0 { g.next() } else { g.below(4) });
    }
    for k in 0..0x14 / 4 {
        r.mem.write_u32(0x337fe8 + 4 * k, g.next());
    }
    r.mem.write_u32(0x32de8c, [6, 2][g.below(2) as usize]);
    r.mem.write_u32(PAD, g.next());
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    w(r, GAPP, a.app());
    w(r, a.app() + 0x724, a.r());
    w(r, a.app() + 0x730, a.race());
    w(r, a.app() + 0x24, a.input());
    w(r, a.input() + 0x14, g.below(5));
    w(r, a.race() + 0x290, g.below(3));
    w(r, a.race() + 0x298, g.below(3) + 1);
    // two overlay vtables with different `this` adjustments, the renderer's
    for k in 0..2 {
        for s in OV_SLOTS {
            w(r, a.ov_vt(k) + s, OV_FN + s);
            r.mem.write_u16(a.ov_vt(k) + s - 4, if k == 0 { 0 } else { 8 });
        }
    }
    for s in R_SLOTS {
        w(r, a.r_vt() + s, R_FN + s);
        r.mem.write_u16(a.r_vt() + s - 4, 4);
    }
    w(r, a.r() + 0x10, a.r_vt());
    for i in 0..OVERLAYS {
        let o = a.ov(i);
        w(r, a.man() + 0x10 + 4 * i, o);
        w(r, o + 0x28, a.ov_vt(g.below(2)));
        w(r, o + 0xc, [0, 0, 1, 2, 3, 5, u32::MAX, u32::MAX][g.below(8) as usize]);
        w(r, o + 0x10, g.below(2));
        w(r, o + 0x14, g.below(2));
    }
    for i in 0..4 {
        w(r, a.pad(i) + 0xc, g.next() & 0xff);
        w(r, a.pad(i) + 0x10, g.next() & 0xff);
    }
}

/// The overlays' constructors (their results read through the object)
fn is_ctor(f: u32) -> bool {
    matches!(f, 0x19e888 | 0x1af470 | 0x1b6b90 | 0x1b3f50 | 0x1eb5d0 | 0x1ab9f8 | 0x1ef610 | 0x1b38f0 | 0x1b58c0 | 0x1bd190)
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

/// The words a constructor's result is read through, written the same on both sides: the object's +0 / +4 / +0x42c
fn construct(m: &mut dyn FnMut(u32, u32), p: u32) {
    m(p, p + 0x100);
    m(p + 4, p);
    m(p + 0x42c, p + 0x200);
}

struct Ours(L, u32);
impl OverlayManEnv for Ours {
    fn virtual_call(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32], floats: &[f32]) -> u32 {
        let mut l = self.0.borrow_mut();
        let mut v: Vec<u32> = args.to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
        l.calls.push((func, this, v));
        l.take()
    }
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.borrow_mut().calls.push((func, 0, args.to_vec()));
        if args.first().is_some_and(|p| *p >= self.1 + 0x2000 && *p < self.1 + ARENA) && is_ctor(func) {
            construct(&mut |at, v| m.w32(at, v), args[0]);
        }
        args.first().copied().unwrap_or(0)
    }
    fn alloc(&mut self, _m: &mut Mem, name: u32, size: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        let k = l.calls.iter().filter(|c| c.0 == 0x2ccf70).count() as u32;
        l.calls.push((0x2ccf70, name, vec![size]));
        self.1 + 0x2000 + 0x1000 * k
    }
    fn device(&mut self, pad: u32) -> u32 {
        self.0.borrow_mut().calls.push((0x17a830, pad, vec![]));
        self.1 + 0x1500 + 0x20 * (pad & 3)
    }
    fn is_player_pad(&mut self, pad: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.calls.push((0x110f28, pad, vec![]));
        l.take() & 1 != 0
    }
    fn device_pressed(&mut self, pad: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.calls.push((0x17a898, pad, vec![]));
        l.take() & 1 != 0
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [0x1a7e70, 0x1a7f40, 0x1a8110, 0x1a7d78, 0x1a8900, 0x2cc808] {
        r.allow(f);
    }
    for s in OV_SLOTS {
        let l2 = l.clone();
        let f = OV_FN + s;
        r.stub(f, move |c, _| {
            let (ni, _) = arity(f);
            let v: Vec<u32> = (0..ni).map(|k| arg(c, 5 + k)).collect();
            let mut l = l2.borrow_mut();
            l.calls.push((f, arg(c, 4), v));
            let x = l.take();
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    for s in R_SLOTS {
        let l2 = l.clone();
        let f = R_FN + s;
        r.stub(f, move |c, _| {
            let (ni, nf) = arity(f);
            let mut v: Vec<u32> = (0..ni).map(|k| arg(c, 5 + k)).collect();
            v.extend((0..nf).map(|k| c.f(12 + k).to_bits()));
            let mut l = l2.borrow_mut();
            l.calls.push((f, arg(c, 4), v));
            let x = l.take();
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
    let base = a.b;
    for (f, n) in CALLS_N {
        let l2 = l.clone();
        r.stub(f, move |c, m| {
            let v: Vec<u32> = (0..n).map(|k| arg(c, 4 + k)).collect();
            l2.borrow_mut().calls.push((f, 0, v.clone()));
            if v[0] >= base + 0x2000 && v[0] < base + ARENA && is_ctor(f) {
                construct(&mut |at, x| m.write_u32(at, x), v[0]);
            }
            c.set_gpr(2, v[0] as u64);
        });
    }
    let l2 = l.clone();
    r.stub(0x2ccf70, move |c, _| {
        assert_eq!(arg(c, 6), 0);
        let mut l = l2.borrow_mut();
        let k = l.calls.iter().filter(|c| c.0 == 0x2ccf70).count() as u32;
        l.calls.push((0x2ccf70, arg(c, 4), vec![arg(c, 5)]));
        c.set_gpr(2, (base + 0x2000 + 0x1000 * k) as u64);
    });
    let l2 = l.clone();
    r.stub(0x17a830, move |c, _| {
        l2.borrow_mut().calls.push((0x17a830, arg(c, 5), vec![]));
        c.set_gpr(2, (base + 0x1500 + 0x20 * (arg(c, 5) & 3)) as u64);
    });
    for f in [0x110f28u32, 0x17a898] {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let mut l = l2.borrow_mut();
            l.calls.push((f, arg(c, 5), vec![]));
            let x = l.take() & 1;
            c.set_gpr(2, x as i32 as i64 as u64);
        });
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[allow(clippy::too_many_arguments)]
fn check(r: &mut Runner, a: &At, l: &L, g: &mut Rng, cases: u32, addr: u32, what: &str, mut regs: impl FnMut(&mut Runner, &mut Rng) -> Vec<u32>, mut ours: impl FnMut(&mut Mem, &mut Ours, &[u32]) -> Option<u32>) {
    for case in 0..cases {
        build(g, r, a);
        let args = regs(r, g);
        let answers: Vec<u32> = (0..256).map(|_| if g.below(3) == 0 { g.next() } else { g.below(3) }).collect();
        let mut m = snapshot(r, a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(addr).unwrap_or_else(|e| panic!("{what} case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        let v0 = r.cpu.gpr(2) as u32;
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let ret = ours(&mut m, &mut Ours(l.clone(), a.b), &args);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        if theirs != mine {
            let i = (0..theirs.len().min(mine.len())).find(|&i| theirs[i] != mine[i]).unwrap_or(theirs.len().min(mine.len()));
            panic!("{what} case {case}: call {i}: {:x?} vs {:x?}", theirs.get(i), mine.get(i));
        }
        if let Some(v) = ret {
            assert_eq!(v0, v, "{what} case {case}: result");
        }
        compare(r, a, &m, &format!("{what} case {case}"));
    }
}

#[test]
fn overlay_manager_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1a6f_b8dd_6b8b_0001);
    let man = a.man();
    let set = move |r: &mut Runner, args: &[u32]| {
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        args.to_vec()
    };
    check(&mut r, &a, &l, &mut g, 300, 0x1a6fb8, "init", move |r, _| set(r, &[man]), move |m, e, _| {
        man_init(m, e, man);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1a76b8, "destroy", move |r, g| set(r, &[man, g.below(4)]), move |m, e, a| {
        man_destroy(m, e, man, a[1]);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1a7758, "hide all", move |r, _| set(r, &[man]), move |m, e, _| {
        hide_all(m, e, man);
        None
    });
    let ov0 = a.ov(0);
    check(&mut r, &a, &l, &mut g, 4_000, 0x1a7e70, "hide", move |r, g| set(r, &[ov0, g.below(5).wrapping_sub(1)]), move |m, e, a| {
        hide_for_player(m, e, ov0, a[1] as i32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1a77a8, "visible", move |r, _| set(r, &[man]), move |m, _, _| Some(any_visible(m, man) as u32));
    check(&mut r, &a, &l, &mut g, 2_000, 0x1a77f8, "active", move |r, _| set(r, &[man]), move |m, _, _| Some(any_active(m, man) as u32));
    check(&mut r, &a, &l, &mut g, 1_000, 0x1a7878, "set pad", move |r, g| set(r, &[ov0, g.below(4)]), move |m, e, a| {
        set_player_pad(m, e, ov0, a[1]);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1a7ad0, "update", move |r, _| set(r, &[man]), move |m, e, _| {
        man_update(m, e, man);
        None
    });
    for (addr, slot) in [(0x1a7b30u32, 0x5cu32), (0x1a7b88, 0x64)] {
        check(&mut r, &a, &l, &mut g, 500, addr, "call all", move |r, _| set(r, &[man]), move |m, e, _| {
            call_all(m, e, man, slot);
            None
        });
    }
    for (addr, at, slot) in CALLS {
        check(&mut r, &a, &l, &mut g, 300, addr, "call one", move |r, _| set(r, &[man]), move |m, e, _| {
            call_one(m, e, man, at, slot);
            None
        });
    }
    check(&mut r, &a, &l, &mut g, 300, 0x1a7c40, "replay control", move |r, _| set(r, &[man]), move |m, _, _| Some(replay_control(m, man)));
    check(&mut r, &a, &l, &mut g, 300, 0x1a7c48, "lesson", move |r, _| set(r, &[man]), move |m, _, _| Some(lesson(m, man)));
    check(&mut r, &a, &l, &mut g, 4_000, 0x1a78c0, "draw", move |r, _| set(r, &[man]), move |m, e, _| {
        man_draw(m, e, man);
        None
    });
    check(&mut r, &a, &l, &mut g, 4_000, 0x1a8688, "new buttons", move |r, g| set(r, &[ov0, g.below(6).wrapping_sub(2)]), move |m, e, a| {
        Some(new_buttons(m, e, ov0, a[1] as i32))
    });
    let blk = a.block(0);
    check(&mut r, &a, &l, &mut g, 500, 0x1a8900, "backdrop", move |r, _| set(r, &[blk]), move |m, e, _| {
        backdrop_init(m, e, blk);
        Some(blk)
    });
}

// ---- the backdrop's animation (G8c) ----

const FL: [f32; 12] = [0.0, 1.0, -1.0, 0.5, 0.034, -0.02, 10.0, -10.0, 640.0, -640.0, 499.0, 2.5];

fn backdrop(g: &mut Rng, r: &mut Runner, o: u32, fonts: u32) {
    for k in 0..0x24 {
        let v = FL[g.below(FL.len() as u32) as usize];
        r.mem.write_u32(o + 0x2c + 4 * k, v.to_bits());
    }
    for at in [0x60u32, 0x7c, 0x90] {
        r.mem.write_u32(o + at, if g.below(5) == 0 { 1 } else { g.below(128) });
    }
    r.mem.write_u32(o + 4, fonts);
    r.mem.write_u32(o + 8, fonts + 0x40);
}

#[test]
fn backdrop_animation_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let l2 = l.clone();
    r.stub(0x1f12d0, move |c, _| l2.borrow_mut().calls.push((0x1f12d0, 0, vec![c.gpr(4) as u32])));
    for f in [0x1a8c50, 0x1a8e10, 0x1a8f28] {
        r.allow(f);
    }
    let mut g = Rng(0x1a89_b0aa_8c8c_0002);
    let o = a.block(0);
    let fonts = a.block(1);
    for case in 0..60_000u32 {
        build(&mut g, &mut r, &a);
        backdrop(&mut g, &mut r, o, fonts);
        let which = g.below(7);
        let bits = g.below(128);
        let v = FL[g.below(FL.len() as u32) as usize];
        r.cpu.set_gpr(4, o as u64);
        r.cpu.set_gpr(5, bits as u64);
        r.cpu.set_f(12, v);
        let mut m = snapshot(&mut r, &a);
        l.borrow_mut().calls.clear();
        let addr = [0x1a89b0u32, 0x1a8ad8, 0x1a8b70, 0x1a8c50, 0x1a8e10, 0x1a8f28, 0x1a8bd8][which as usize];
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        let mut e = Ours(l.clone(), a.b);
        match which {
            0 => start_frame_anim(&mut m, o, bits, v),
            1 => start_child_slide(&mut m, o, bits, v),
            2 => start_edge_slide(&mut m, o, bits),
            3 => update_frame_anim(&mut m, o),
            4 => update_child_slide(&mut m, o),
            5 => update_edge_slide(&mut m, o),
            _ => backdrop_update(&mut m, &mut e, o),
        }
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "case {case} ({which}): calls");
        compare(&mut r, &a, &m, &format!("case {case} ({which})"));
    }
}

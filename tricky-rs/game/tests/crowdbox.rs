//! `tricky_game::pausemenu` (board row G10m) against the crowd box functions in the function runner (TRICKY_ELF;
//! skipped without it): every callee (the ini reader, strcmp, sprintf, the archive and bank loaders) is a stub that
//! answers from the same random stream on both sides and writes strings into the frame the way the real ones do; the
//! calls (address and arguments), the frame and the speech object are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::Mem;
use tricky_game::crowdbox::*;
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



const TARGETS: [u32; 4] = [OPEN_FILE, RAND, VOICE_STOP, BOX_UPDATE];

struct Oracle {
    g: Rng,
    log: Vec<(u32, Vec<u32>)>,
}
impl Oracle {
    fn call(&mut self, _m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(TARGETS.contains(&t), "target {t:#x}");
        self.log.push((t, args.to_vec()));
        match t {
            OPEN_FILE => {
                if self.g.below(4) == 0 {
                    0
                } else {
                    0x7000 + self.g.below(0x100)
                }
            }
            RAND => self.g.next(),
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

const B: u32 = 0x190;
struct W {
    r: Runner,
    b: u32,
    snd: u32,
    list: u32,
    sys: u32,
    boxes: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2201f8);
    r.allow(0x220720);
    let b = r.mem.alloc(B);
    let snd = r.mem.alloc(0xc * 16);
    let list = r.mem.alloc(0x20);
    let sys = r.mem.alloc(0x3800);
    let boxes = r.mem.alloc(B * 4);
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), log: vec![] }));
    for t in TARGETS {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let mut args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            if t == VOICE_STOP {
                args[3] = c.f(12).to_bits();
            }
            let n = if t == VOICE_STOP { 4 } else { 2 };
            let v = l.borrow_mut().call(&mut RM2(m), t, &args[..n.max(if t == BOX_UPDATE { 1 } else { 0 })]);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, b, snd, list, sys, boxes, oracle })
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng) {
    sm.0.clear();
    let mut o: Vec<u8> = (0..B).map(|_| g.next() as u8).collect();
    let mut set = |off: u32, v: u32| o[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    let levels = [0.0f32, 0.1, 0.25, 0.3, 1.0, 0.26];
    set(0x188, levels[g.below(6) as usize].to_bits());
    let mut used = 0;
    for s in 0..15u32 {
        let on = g.below(3) != 0;
        used += on as u32;
        set(0x20 + s * 0xc, on as u32);
        set(0x28 + s * 0xc, w.snd + 0xc * g.below(5));
    }
    set(0xd4, if g.below(3) == 0 { used } else { g.below(4) });
    set(0x174, [0xffff_ffffu32, 0, 5, 100][g.below(4) as usize]);
    set(0x180, g.below(3));
    set(0x170, g.below(10));
    set(0, g.next());
    let mut img: Vec<(u32, Vec<u8>)> = vec![(w.b, o)];
    let mut snd = vec![];
    for _ in 0..16 {
        snd.extend([0xffff_ffffu32, [0xffff_ffff, 3, 99][g.below(3) as usize], g.below(4)].iter().flat_map(|x| x.to_le_bytes()));
    }
    img.push((w.snd, snd));
    img.push((w.list, (0..0x20).map(|_| g.below(8) as u8).collect()));
    let mut sys: Vec<u8> = (0..0x3800).map(|_| g.next() as u8).collect();
    sys[0x3728..0x372c].copy_from_slice(&g.below(4).to_le_bytes());
    sys[0x372c..0x3730].copy_from_slice(&w.boxes.to_le_bytes());
    img.push((w.sys, sys));
    img.push((w.boxes, (0..B * 4).map(|_| g.next() as u8).collect()));
    img.push((0x343120, g.next().to_le_bytes().to_vec()));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
    // the list's count is a small number
    let n = g.below(7);
    w.r.mem.write_u32(w.list, n);
    sm.w32(w.list, n);
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    vec![(w.b, B), (w.snd, 0xc * 16), (w.list, 0x20), (w.sys, 0x3800), (w.boxes, B * 4)]
}

fn check(w: &mut W, sm: &mut SM, case: u32, seed: u64, addr: u32, set: &dyn Fn(&mut Runner), port: &dyn Fn(&mut SM, &mut PortEnv) -> u32, ret: bool) {
    {
        let mut o = w.oracle.borrow_mut();
        o.g = Rng(seed);
        o.log.clear();
    }
    set(&mut w.r);
    w.r.call(addr).unwrap();
    let theirs_calls = std::mem::take(&mut w.oracle.borrow_mut().log);
    let their_ret = w.r.cpu.gpr(2) as u32;
    let mut theirs = vec![];
    for (a, n) in regions(w) {
        let mut b = vec![0u8; n as usize];
        w.r.mem.read(a, &mut b).unwrap();
        theirs.extend(b);
    }
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), log: vec![] }));
    let mut env = PortEnv(mine.clone());
    let my_ret = port(sm, &mut env);
    let my_calls = std::mem::take(&mut mine.borrow_mut().log);
    for i in 0..my_calls.len().min(theirs_calls.len()) {
        assert_eq!(my_calls[i].0, theirs_calls[i].0, "case {case}: call {i}");
        assert_eq!(my_calls[i].1[..], theirs_calls[i].1[..my_calls[i].1.len()], "case {case}: args of call {i} ({:#x})", my_calls[i].0);
    }
    assert_eq!(my_calls.len(), theirs_calls.len(), "case {case}: number of calls");
    if ret {
        assert_eq!(my_ret, their_ret, "case {case}: return");
    }
    let mut mymem = vec![];
    for (a, n) in regions(w) {
        mymem.extend((a..a + n).map(|x| sm.r8(x)));
    }
    if mymem != theirs {
        let at = mymem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
        panic!("case {case}: memory differs at +{at:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}

fn fades(g: &mut Rng) -> f32 {
    [0.0f32, 0.5, 2.0, -0.0][g.below(4) as usize]
}

#[test]
fn list_functions_match() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(2), SM::default());
    for case in 0..20_000 {
        image(&mut w, &mut sm, &mut g);
        let (seed, l, name, mode) = (g.next() as u64 | 1, w.list, g.next(), g.next());
        match g.below(3) {
            0 => check(&mut w, &mut sm, case, seed, 0x21eca8, &|r| r.cpu.set_gpr(4, l as u64), &|sm, _| {
                list_construct(sm, l);
                l
            }, true),
            1 => check(&mut w, &mut sm, case, seed, 0x21ed30, &|r| {
                r.cpu.set_gpr(4, l as u64);
                r.cpu.set_gpr(5, name as i32 as i64 as u64);
                r.cpu.set_gpr(6, mode as i32 as i64 as u64);
            }, &|sm, env| list_add(sm, env, l, name, mode), true),
            _ => check(&mut w, &mut sm, case, seed, 0x21ed98, &|r| r.cpu.set_gpr(4, l as u64), &|sm, env| list_pick_random(sm, env, l), true),
        }
    }
}

#[test]
fn box_functions_match() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(3), SM::default());
    for case in 0..20_000 {
        image(&mut w, &mut sm, &mut g);
        let (seed, b) = (g.next() as u64 | 1, w.b);
        let (slot, fade) = (g.below(16) as i32 - 1, fades(&mut g));
        let which = g.below(6);
        let set = move |r: &mut Runner| {
            r.cpu.set_gpr(4, b as u64);
            r.cpu.set_gpr(5, slot as i64 as u64);
            r.cpu.set_f(12, fade);
        };
        match which {
            0 => check(&mut w, &mut sm, case, seed, 0x21ee70, &set, &|sm, _| {
                box_init(sm, b);
                b
            }, true),
            1 => check(&mut w, &mut sm, case, seed, 0x2201f8, &set, &|sm, env| {
                stop_slot(sm, env, b, slot, fade);
                0
            }, false),
            2 => check(&mut w, &mut sm, case, seed, 0x220338, &set, &|sm, env| {
                stop_all(sm, env, b, fade);
                0
            }, false),
            3 => check(&mut w, &mut sm, case, seed, 0x2206c8, &set, &|sm, _| {
                arm(sm, b);
                0
            }, false),
            4 => check(&mut w, &mut sm, case, seed, 0x2206e0, &set, &|sm, env| {
                stop(sm, env, b);
                0
            }, false),
            _ => check(&mut w, &mut sm, case, seed, 0x220720, &set, &|sm, env| {
                stop_cheer(sm, env, b);
                0
            }, false),
        }
    }
}

#[test]
fn update_boxes_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(4), SM::default());
    for case in 0..5_000 {
        image(&mut w, &mut sm, &mut g);
        let (seed, s) = (g.next() as u64 | 1, w.sys);
        check(&mut w, &mut sm, case, seed, 0x220968, &|r| r.cpu.set_gpr(4, s as u64), &|sm, env| {
            update_boxes(sm, env, s);
            0
        }, false);
    }
}

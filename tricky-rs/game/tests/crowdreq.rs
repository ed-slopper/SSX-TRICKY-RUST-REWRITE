//! `tricky_game::crowdbox`'s voice requests (board row G10m): `CrowdBox_Update` 0x21efb0, `_StartSound` 0x21fc98,
//! `_SetSlotValue` 0x21fc68, `_PlayCheer` 0x2203a8 and `_StartCheer` 0x220538 against the original in the function
//! runner (TRICKY_ELF; skipped without it). The callees that are not ported (the voice stop, the monitor find / set,
//! the pending-sound player, the request begin / play, the audio object's virtual call, memset) are stubs that answer
//! from the same random stream on both sides and change the audio object the way the real ones do (a begin adds an
//! entry, a play empties the queue); the list picker and `stop_cheer` run as real code. The calls (address and
//! arguments), the return value and the memory are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::crowdbox::*;
use tricky_game::speechlib::Mem;
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

struct RM2<'a>(&'a mut r5900::Mem);
impl Mem for RM2<'_> {
    fn r8(&self, a: u32) -> u8 {
        let mut b = [0u8];
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

const VCALL: u32 = 0x20d0d0;
const AUDIO_PTR: u32 = 0x343120;
const SP0: u32 = 0x01ff_0000;
const UPDATE_FRAME: u32 = 0x130;
/// (target, number of arguments)
const TARGETS: [(u32, usize); 9] = [
    (VOICE_STOP, 4),
    (RAND, 0),
    (MEMSET, 3),
    (MONITOR_FIND, 3),
    (MONITOR_SET, 2),
    (PLAY_PENDING, 1),
    (ENTRY_BEGIN, 3),
    (ENTRY_PLAY, 1),
    (VCALL, 4),
];

struct Oracle {
    g: Rng,
    log: Vec<(u32, Vec<u32>)>,
    mon: u32,
    audio: u32,
}
impl Oracle {
    fn call(&mut self, m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(TARGETS.iter().any(|x| x.0 == t), "target {t:#x}");
        self.log.push((t, args.to_vec()));
        match t {
            MEMSET => {
                for k in 0..args[2] {
                    m.w8(args[0] + k, args[1] as u8);
                }
                args[0]
            }
            MONITOR_FIND => {
                if self.g.below(3) == 0 {
                    0
                } else {
                    self.mon
                }
            }
            PLAY_PENDING => {
                m.w32(self.audio + 0x20, 0);
                self.g.below(9).wrapping_sub(1)
            }
            ENTRY_BEGIN => {
                let c = m.r32(self.audio + 0x20);
                m.w32(self.audio + 0x20, c + 1);
                self.g.next()
            }
            ENTRY_PLAY => self.g.below(24).wrapping_sub(5),
            VCALL => {
                if self.g.below(3) != 0 {
                    let (x, y) = (self.g.below(0x2000), self.g.below(0x2000));
                    m.w32(args[2], x);
                    m.w32(args[3], y);
                }
                self.g.below(5)
            }
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

struct W {
    r: Runner,
    b: u32,
    aud: u32,
    arrs: u32,
    src: u32,
    srcarrs: u32,
    pos: u32,
    mon: u32,
    vt: u32,
    oracle: Rc<RefCell<Oracle>>,
}

// the carve-up of the arrays region
const A38: u32 = 0x000;
const A3C: u32 = 0x020;
const A40: u32 = 0x040;
const A64: u32 = 0x060;
const A68: u32 = 0x080;
const A6C: u32 = 0x0a0;
const A70: u32 = 0x0c0;
const A78: u32 = 0x0e0;
const A7C: u32 = 0x100;
const KIND: u32 = 0x120;
const ID: u32 = 0x140;
const A74: u32 = 0x160;
const RECS: u32 = 0x1e0;
const ARRS: u32 = 0x2a0;

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x220720u32, 0x21ed98, 0x2201f8] {
        r.allow(a);
    }
    let b = r.mem.alloc(0x190);
    let aud = r.mem.alloc(0x4000);
    let arrs = r.mem.alloc(ARRS);
    let src = r.mem.alloc(0x100);
    let srcarrs = r.mem.alloc(0x80);
    let pos = r.mem.alloc(0x200);
    let mon = r.mem.alloc(0x100);
    let vt = r.mem.alloc(0x100);
    for a in [b, aud, arrs, src, srcarrs, pos, mon, vt] {
        assert_eq!(a % 16, 0);
    }
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), log: vec![], mon, audio: aud }));
    for (t, n) in TARGETS {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let mut args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            if t == VOICE_STOP {
                args[3] = c.f(12).to_bits();
            }
            let v = l.borrow_mut().call(&mut RM2(m), t, &args[..n]);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, b, aud, arrs, src, srcarrs, pos, mon, vt, oracle })
}

fn pf(g: &mut Rng) -> f32 {
    (g.below(2001) as f32 - 1000.0) * 0.37
}

/// Fill both memories (the runner's and the port's) with a random world; `slots` slots of the box in use.
fn image(w: &mut W, sm: &mut SM, g: &mut Rng, slots: u32) {
    sm.0.clear();
    let mut put: Vec<(u32, Vec<u8>)> = vec![];
    let rnd = |g: &mut Rng, n: u32| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    // the box
    let mut o = rnd(g, 0x190);
    let set = |o: &mut Vec<u8>, off: u32, v: u32| o[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    let levels = [0.0f32, 0.1, 0.25, 0.3, 1.0, 0.26];
    set(&mut o, 0x188, levels[g.below(6) as usize].to_bits());
    let mut order: Vec<u32> = (0..15).collect();
    for k in (1..15).rev() {
        order.swap(k, g.below(k as u32 + 1) as usize);
    }
    for s in 0..15u32 {
        set(&mut o, 0x20 + s * 0xc, 0);
        set(&mut o, 0x28 + s * 0xc, 0);
    }
    for k in 0..slots as usize {
        let s = order[k];
        let e = g.below(3);
        set(&mut o, 0x24 + s * 0xc, if g.below(8) == 0 { 1 + g.below(4) } else { 1 + g.below(3000) });
        set(&mut o, 0x28 + s * 0xc, w.b + 0xe0 + e * 0x30);
        let p = w.pos + 16 * g.below(32);
        set(&mut o, 0x20 + s * 0xc, p);
        // the in-use word is the position pointer (non-zero); the value word follows
    }
    set(&mut o, 0xd4, slots);
    set(&mut o, 0x174, [0xffff_ffffu32, 0, 5, 100][g.below(4) as usize]);
    set(&mut o, 0x180, g.below(3));
    set(&mut o, 0x170, [0u32, 1, 1, 2, 2, 3][g.below(6) as usize]);
    set(&mut o, 0xd8, g.below(6));
    set(&mut o, 0x178, g.below(300));
    for s in 0..3u32 {
        set(&mut o, 0xe0 + s * 0x30, g.below(4));
        set(&mut o, 0xe4 + s * 0x30, [0xffff_ffffu32, 0, 3, 9][g.below(4) as usize]);
        set(&mut o, 0xe8 + s * 0x30, g.below(3));
        set(&mut o, 0x100 + s * 0x30, s);
    }
    put.push((w.b, o));
    // the audio object
    let mut a = rnd(g, 0x4000);
    let aw = |a: &mut Vec<u8>, off: u32, v: u32| a[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    aw(&mut a, 4, w.vt);
    aw(&mut a, 0x20, g.below(2));
    for (off, base) in [(0x24, RECS), (0x38, A38), (0x3c, A3C), (0x40, A40), (0x64, A64), (0x68, A68), (0x6c, A6C), (0x70, A70), (0x74, A74), (0x78, A78), (0x7c, A7C), (0xa40, KIND), (0xa44, ID)] {
        aw(&mut a, off, w.arrs + base);
    }
    for k in 0..4u32 {
        let v = if g.below(5) == 0 { 0 } else { w.pos + 16 * g.below(32) };
        aw(&mut a, 0x28 + 4 * k, v);
    }
    for row in 0..6u32 {
        for l in [0u32, 0x18, 0x30] {
            aw(&mut a, 0x3730 + row * 0x54 + l, g.below(6));
        }
        aw(&mut a, 0x3730 + row * 0x54 + 0x48, pf(g).to_bits());
        aw(&mut a, 0x3730 + row * 0x54 + 0x4c, pf(g).to_bits());
    }
    put.push((w.aud, a));
    put.push((w.arrs, rnd(g, ARRS)));
    // the other queue
    let mut s = rnd(g, 0x100);
    s[0x20..0x24].copy_from_slice(&g.below(4).to_le_bytes());
    for k in 0..4u32 {
        let v = if g.below(6) == 0 { 0 } else { w.pos + 16 * g.below(32) };
        s[0x28 + 4 * k as usize..0x2c + 4 * k as usize].copy_from_slice(&v.to_le_bytes());
    }
    for (off, base) in [(0x3c, 0u32), (0x40, 0x20), (0x68, 0x40)] {
        s[off..off + 4].copy_from_slice(&(w.srcarrs + base).to_le_bytes());
    }
    put.push((w.src, s));
    put.push((w.srcarrs, rnd(g, 0x80)));
    // positions: floats, the fourth word anything
    let mut p = vec![];
    for k in 0..128 {
        p.extend(if k % 4 == 3 { g.next() } else { pf(g).to_bits() }.to_le_bytes());
    }
    put.push((w.pos, p));
    put.push((w.mon, rnd(g, 0x100)));
    let mut vt = rnd(g, 0x100);
    let off: i16 = [0i16, 0x10, -0x10, 0x20][g.below(4) as usize];
    vt[0x80..0x82].copy_from_slice(&off.to_le_bytes());
    vt[0x84..0x88].copy_from_slice(&VCALL.to_le_bytes());
    put.push((w.vt, vt));
    put.push((AUDIO_PTR, w.aud.to_le_bytes().to_vec()));
    put.push((0x39fbd8, rnd(g, 8)));
    put.push((SP0 - UPDATE_FRAME + 0x70, rnd(g, 8)));
    for (a, v) in &put {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    vec![
        (w.b, 0x190),
        (w.aud, 0x4000),
        (w.arrs, ARRS),
        (w.src, 0x100),
        (w.srcarrs, 0x80),
        (w.mon, 0x100),
        (SP0 - UPDATE_FRAME + 0x70, 8),
    ]
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
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), log: vec![], mon: w.mon, audio: w.aud }));
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
        let names = regions(w);
        let mut off = at as u32;
        let mut which = 0;
        for (i, (_, n)) in names.iter().enumerate() {
            if off < *n {
                which = i;
                break;
            }
            off -= *n;
        }
        panic!("case {case}: memory differs in region {which} at +{off:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}

#[test]
fn update_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(11), SM::default());
    let (mut one, mut multi, mut cheer) = (0, 0, 0);
    for case in 0..6_000 {
        let slots = [0u32, 1, 1, 2, 2, 3, 3, 4, 5, 6][g.below(10) as usize];
        image(&mut w, &mut sm, &mut g, slots);
        let (seed, b) = (g.next() as u64 | 1, w.b);
        let nv = sm.r32(b + 0x170);
        if slots > 0 {
            one += (nv == 1) as u32;
            multi += (nv != 1) as u32;
            cheer += (sm.r32(b + 0x180) != 0) as u32;
        }
        let sp = SP0 - UPDATE_FRAME;
        check(&mut w, &mut sm, case, seed, BOX_UPDATE, &|r| r.cpu.set_gpr(4, b as u64), &|sm, env| {
            update(sm, env, b, sp);
            0
        }, false);
    }
    println!("one {one} multi {multi} cheer {cheer}");
}

#[test]
fn start_sound_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(12), SM::default());
    for case in 0..8_000 {
        let slots = [0u32, 1, 2, 3, 8, 13, 14, 15][g.below(8) as usize].min(14);
        image(&mut w, &mut sm, &mut g, slots);
        let (seed, b) = (g.next() as u64 | 1, w.b);
        if g.below(8) == 0 {
            let v = 12 + g.below(6);
            w.r.mem.write_u32(b + 0xd4, v);
            sm.w32(b + 0xd4, v);
        }
        let src = if g.below(2) == 0 { w.aud } else { w.src };
        let id = g.below(4);
        // a new id needs a free entry
        let ids: Vec<u32> = (0..3).map(|k| sm.r32(b + 0xe0 + k * 0x30)).collect();
        if !ids.contains(&id) && (0..3).all(|k| sm.r32(b + 0xe8 + k * 0x30) != 0) {
            w.r.mem.write_u32(b + 0xe8, 0);
            sm.w32(b + 0xe8, 0);
        }
        let first_free = (0..15).find(|s| sm.r32(b + 0x20 + s * 0xc) == 0).is_some();
        assert!(first_free);
        check(&mut w, &mut sm, case, seed, 0x21fc98, &|r| {
            r.cpu.set_gpr(4, b as u64);
            r.cpu.set_gpr(5, src as u64);
            r.cpu.set_gpr(6, id as u64);
        }, &|sm, env| start_sound(sm, env, b, src, id) as u32, true);
    }
}

#[test]
fn small_ones_match() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(13), SM::default());
    for case in 0..12_000 {
        let slots = [0u32, 1, 2, 3, 5][g.below(5) as usize];
        image(&mut w, &mut sm, &mut g, slots);
        let (seed, b) = (g.next() as u64 | 1, w.b);
        let k = g.below(5) as i32;
        let slot = g.below(17) as i32 - 1;
        let v = g.next();
        match g.below(3) {
            0 => check(&mut w, &mut sm, case, seed, 0x21fc68, &|r| {
                r.cpu.set_gpr(4, b as u64);
                r.cpu.set_gpr(5, slot as i64 as u64);
                r.cpu.set_gpr(6, v as i32 as i64 as u64);
            }, &|sm, _| {
                set_slot_value(sm, b, slot, v);
                0
            }, false),
            1 => check(&mut w, &mut sm, case, seed, 0x2203a8, &|r| {
                r.cpu.set_gpr(4, b as u64);
                r.cpu.set_gpr(5, k as u64);
            }, &|sm, env| {
                play_cheer(sm, env, b, k);
                0
            }, false),
            _ => check(&mut w, &mut sm, case, seed, 0x220538, &|r| {
                r.cpu.set_gpr(4, b as u64);
                r.cpu.set_gpr(5, k as u64);
            }, &|sm, env| {
                start_cheer(sm, env, b, k);
                0
            }, false),
        }
    }
}

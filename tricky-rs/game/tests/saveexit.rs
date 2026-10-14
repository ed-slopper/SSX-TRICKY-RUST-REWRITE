//! `tricky_game::saveexit` (board row G10m) against the crowd box functions in the function runner (TRICKY_ELF;
//! skipped without it): every callee (the ini reader, strcmp, sprintf, the archive and bank loaders) is a stub that
//! answers from the same random stream on both sides and writes strings into the frame the way the real ones do; the
//! calls (address and arguments), the frame and the speech object are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::Mem;
use tricky_game::saveexit::*;
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



const T1: u32 = 0x90_0000;
const T2: u32 = 0x90_0010;
const TARGETS: [u32; 34] = [
    LOC_GET, GET_BUTTONS, SOUND_MOVE, SOUND_BACK, SOUND_SELECT, DIALOG_SET, SYNC_AUDIO, HUD_HIDE, HUD_SHOW, OVERLAY_TICK,
    OVERLAY_ASK, OP_UPDATE, OP_IS_DONE, OP_RESULT, OP_FINISH, OP_PREPARE, OP_BEGIN, OP_RESTART, STR_A, STR_B, STR_C, SPRINTF,
    CAREER_A, CAREER_B, START_RACE, CAREER_C, CAREER_D, CAREER_E, CAREER_F, CAREER_G, HUD_GET_A, HUD_GET_B, T1, T2,
];
const TARGETS2: [u32; 4] = [RACE_A, RACE_B, RACE_C, LIST_TICK];

struct Oracle {
    g: Rng,
    q: u32,
    log: Vec<(u32, Vec<u32>)>,
}
impl Oracle {
    fn call(&mut self, _m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(TARGETS.contains(&t) || TARGETS2.contains(&t), "target {t:#x}");
        self.log.push((t, args.to_vec()));
        match t {
            GET_BUTTONS => {
                let mut v = 0;
                for b in [0x2_0001u32, 0x4_0002, 0x800, 0x20, 0x1, 0x10] {
                    if self.g.below(3) == 0 {
                        v |= b;
                    }
                }
                if self.g.below(4) == 0 {
                    0
                } else {
                    v
                }
            }
            OVERLAY_ASK => self.g.below(5),
            OP_IS_DONE | OP_RESULT | SYNC_AUDIO => self.g.below(2),
            CAREER_G => [2, 2, 0, 1][self.g.below(4) as usize],
            CAREER_F => self.g.below(5).wrapping_sub(2),
            HUD_GET_A | HUD_GET_B => self.q,
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

// object layout in the test world
const P: u32 = 0x600;
struct W {
    r: Runner,
    p: u32,
    ov: u32,
    vt1: u32,
    vt2: u32,
    card: u32,
    gapp: u32,
    race: u32,
    base: u32,
    q: u32,
    list: u32,
    ob: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let (p, ov, vt1, vt2) = (r.mem.alloc(P), r.mem.alloc(0x340), r.mem.alloc(0x200), r.mem.alloc(0x90));
    let (card, gapp, race, base) = (r.mem.alloc(0x20), r.mem.alloc(0x740), r.mem.alloc(0x100), r.mem.alloc(0x500));
    let (q, list, ob) = (r.mem.alloc(0x600), r.mem.alloc(0x80), r.mem.alloc(0x20));
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), q, log: vec![] }));
    for t in TARGETS.iter().chain(TARGETS2.iter()).copied() {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32];
            let v = l.borrow_mut().call(&mut RM2(m), t, &args);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, p, ov, vt1, vt2, card, gapp, race, base, q, list, ob, oracle })
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng) {
    sm.0.clear();
    let mut rnd = |n: u32, g: &mut Rng| -> Vec<u8> { (0..n).map(|_| g.next() as u8).collect() };
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    let mut p = rnd(P, g);
    let mut set = |o: &mut Vec<u8>, off: u32, v: u32| o[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    set(&mut p, 0x42c, w.ov);
    set(&mut p, 0x428, w.vt1);
    set(&mut p, 0x224, w.card);
    set(&mut p, 0x430, g.below(3));
    set(&mut p, 0x438, g.below(6));
    set(&mut p, 0x558, g.below(7));
    set(&mut p, 0x460, g.below(7));
    set(&mut p, 0x464, g.below(2));
    set(&mut p, 0x454, g.below(2));
    set(&mut p, 0x55c, g.below(2));
    set(&mut p, 0, g.below(3));
    for i in 0..8 {
        set(&mut p, 0x4b0 + i * 4, g.below(2));
        set(&mut p, 0x440 + i * 4, g.below(4));
    }
    img.push((w.p, p));
    let mut ov = rnd(0x340, g);
    set(&mut ov, 0x328, g.below(2));
    img.push((w.ov, ov));
    let mut vt1 = rnd(0x200, g);
    vt1[0x1c0..0x1c2].copy_from_slice(&[0u16, 8, 0xfff0][g.below(3) as usize].to_le_bytes());
    set(&mut vt1, 0x1c4, T1);
    img.push((w.vt1, vt1));
    let mut vt2 = rnd(0x90, g);
    vt2[0x80..0x82].copy_from_slice(&[0u16, 4, 0xfffc][g.below(3) as usize].to_le_bytes());
    set(&mut vt2, 0x84, T2);
    img.push((w.vt2, vt2));
    let mut card = rnd(0x20, g);
    set(&mut card, 0, w.vt2);
    img.push((w.card, card));
    let mut gapp = rnd(0x740, g);
    set(&mut gapp, 0x730, w.race);
    img.push((w.gapp, gapp));
    let mut race = rnd(0x100, g);
    set(&mut race, 0x1c, w.base);
    img.push((w.race, race));
    let mut base = rnd(0x500, g);
    if g.below(2) == 0 {
        set(&mut base, 0x4f8, 0);
    }
    img.push((w.base, base));
    img.push((w.q, rnd(0x600, g)));
    let mut list = rnd(0x80, g);
    set(&mut list, 0x38, g.below(8));
    for i in 0..8 {
        let k = if g.below(2) == 0 { 0 } else { g.below(8) };
        set(&mut list, 0xc + i * 8, k);
    }
    img.push((w.list, list));
    let mut ob = rnd(0x20, g);
    set(&mut ob, 0, w.list);
    set(&mut ob, 4, w.p);
    set(&mut ob, 0x10, g.below(7));
    img.push((w.ob, ob));
    let mut c = rnd(0x3820, g);
    set(&mut c, 0x2afc, [0, 1, 4, 5, 4, 5, 6][g.below(7) as usize]);
    img.push((CAREER, c));
    img.push((GAPP, w.gapp.to_le_bytes().to_vec()));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn regions(w: &W) -> Vec<(u32, u32)> {
    vec![
        (w.p, P), (w.ov, 0x340), (w.card, 0x20), (w.gapp, 0x740), (w.race, 0x100), (w.base, 0x500), (w.q, 0x600), (w.list, 0x80),
        (w.ob, 0x20), (CAREER, 0x3820),
    ]
}

fn check(w: &mut W, sm: &mut SM, case: u32, seed: u64, addr: u32, set: &dyn Fn(&mut Runner), port: &dyn Fn(&mut SM, &mut PortEnv), ) {
    {
        let mut o = w.oracle.borrow_mut();
        o.g = Rng(seed);
        o.log.clear();
    }
    set(&mut w.r);
    w.r.call(addr).unwrap();
    let theirs_calls = std::mem::take(&mut w.oracle.borrow_mut().log);
    let mut theirs = vec![];
    for (a, n) in regions(w) {
        let mut b = vec![0u8; n as usize];
        w.r.mem.read(a, &mut b).unwrap();
        theirs.extend(b);
    }
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), q: w.q, log: vec![] }));
    let mut env = PortEnv(mine.clone());
    port(sm, &mut env);
    let my_calls = std::mem::take(&mut mine.borrow_mut().log);
    for i in 0..my_calls.len().min(theirs_calls.len()) {
        assert_eq!(my_calls[i].0, theirs_calls[i].0, "case {case}: call {i}");
        assert_eq!(my_calls[i].1[..], theirs_calls[i].1[..my_calls[i].1.len()], "case {case}: args of call {i} ({:#x})", my_calls[i].0);
    }
    assert_eq!(my_calls.len(), theirs_calls.len(), "case {case}: number of calls");
    let mut mymem = vec![];
    for (a, n) in regions(w) {
        mymem.extend((a..a + n).map(|x| sm.r8(x)));
    }
    if mymem != theirs {
        let at = mymem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
        panic!("case {case}: memory differs at +{at:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}


const SP0: u32 = 0x01ff_0000;

#[test]
fn saveexit_matches() {
    let Some(mut w) = world() else { return };
    let (mut g, mut sm) = (Rng(2), SM::default());
    for case in 0..30_000 {
        image(&mut w, &mut sm, &mut g);
        let (seed, p, ob) = (g.next() as u64 | 1, w.p, w.ob);
        let mode = g.below(3);
        let set = move |r: &mut Runner| {
            r.cpu.set_gpr(4, p as u64);
            r.cpu.set_gpr(5, mode as u64);
        };
        match [0, 1, 2, 3, 4, 4, 4, 4, 4][g.below(9) as usize] {
            0 => check(&mut w, &mut sm, case, seed, 0x1ce778, &set, &|sm, env| start_save(sm, env, SP0 - 0x60, p, mode)),
            1 => check(&mut w, &mut sm, case, seed, 0x1ce5b0, &set, &|sm, env| start_load(sm, env, SP0 - 0x60, p, mode)),
            2 => check(&mut w, &mut sm, case, seed, 0x1bbb30, &set, &|sm, env| save_before_exit_a(sm, env, SP0 - 0x70, p)),
            3 => check(&mut w, &mut sm, case, seed, 0x1bc8a0, &set, &|sm, env| save_before_exit_b(sm, env, SP0 - 0x70, p)),
            _ => check(&mut w, &mut sm, case, seed, 0x1b32c0, &|r| r.cpu.set_gpr(4, ob as u64), &|sm, env| {
                results_update_save_before_exit(sm, env, SP0 - 0x60, ob)
            }),
        }
    }
}

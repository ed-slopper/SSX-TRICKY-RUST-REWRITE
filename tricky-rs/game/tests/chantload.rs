//! `tricky_game::speechload` (board row G10j) against `Speech_LoadEvents` 0x227d70 in the function runner (TRICKY_ELF;
//! skipped without it): every callee (the ini reader, strcmp, sprintf, the archive and bank loaders) is a stub that
//! answers from the same random stream on both sides and writes strings into the frame the way the real ones do; the
//! calls (address and arguments), the frame and the speech object are compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tricky_game::speechlib::Mem;
use tricky_game::chantload::*;
use tricky_game::speechload::*;

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

const TARGETS: [(u32, usize); 24] = [
    (FIND_SECTION2, 2),
    (READ_INT, 3),
    (COUNT_ENABLED, 1),
    (NEXT_ENABLED, 1),
    (SELECT_SONG, 2),
    (INI_OPEN, 3),
    (FIND_SECTION, 2),
    (SECTION_START, 1),
    (READ_TOKEN, 1),
    (READ_STRING, 3),
    (AT_END, 1),
    (INI_CLOSE, 2),
    (STRCMP, 2),
    (SPRINTF, 3),
    (STRLEN, 1),
    (STRNCPY, 3),
    (SET_FILE, 3),
    (OPEN_BIG, 2),
    (BIG_FIND, 2),
    (BIG_COUNT, 1),
    (BIG_LOOKUP, 4),
    (FREE, 1),
    (REGISTER, 2),
    (LOAD_BANK, 3),
];

struct Oracle {
    g: Rng,
    sys: u32,
    log: Vec<(u32, Vec<u32>)>,
}

fn put_str(m: &mut dyn Mem, at: u32, s: &[u8]) {
    for (i, c) in s.iter().enumerate() {
        m.w8(at + i as u32, *c);
    }
    m.w8(at + s.len() as u32, 0);
}

impl Oracle {
    fn string(&mut self, max: u32) -> Vec<u8> {
        let n = self.g.below(max + 1);
        (0..n).map(|_| if self.g.below(6) == 0 { b'|' } else { b'a' + self.g.below(26) as u8 }).collect()
    }
    fn call(&mut self, m: &mut dyn Mem, t: u32, args: &[u32]) -> u32 {
        assert!(TARGETS.iter().any(|x| x.0 == t));
        self.log.push((t, args.to_vec()));
        match t {
            FIND_SECTION | FIND_SECTION2 => (self.g.below(8) != 0) as u32,
            READ_INT => {
                let v = self.g.below(100);
                m.w32(args[2], v);
                0
            }
            COUNT_ENABLED => [-1i32, 0, 1, 2, 3, 5][self.g.below(6) as usize] as u32,
            NEXT_ENABLED => {
                let v = self.g.below(40);
                m.w32(self.sys + 0x36e8, v);
                0
            }
            AT_END => (self.g.below(3) == 0) as u32,
            READ_TOKEN => self.g.next(),
            STRCMP => (self.g.below(9) != 0) as u32 + self.g.below(2),
            READ_STRING => {
                let s = self.string(12);
                put_str(m, args[2], &s);
                0
            }
            SPRINTF => {
                let s = self.string(30);
                put_str(m, args[0], &s);
                0
            }
            STRLEN => {
                let mut n = 0;
                while m.r8(args[0] + n) != 0 {
                    n += 1;
                }
                n
            }
            STRNCPY => {
                for i in 0..args[2] {
                    let c = m.r8(args[1] + i);
                    m.w8(args[0] + i, c);
                }
                args[0]
            }
            OPEN_BIG => {
                if self.g.below(3) == 0 {
                    0
                } else {
                    0x5000 + (self.g.below(0x100) << 4)
                }
            }
            BIG_FIND => {
                if self.g.below(4) == 0 {
                    0
                } else {
                    0x9000 + (self.g.below(0x100) << 4)
                }
            }
            BIG_COUNT => [-1i32, 0, 1, 2, 3][self.g.below(5) as usize] as u32,
            BIG_LOOKUP => {
                let n = m.r32(self.sys + 0xe8);
                let want = m.r32(self.sys + 0xc8 + n.wrapping_mul(8));
                let off = if self.g.below(3) == 0 { want.wrapping_sub(args[0]) } else { self.g.below(0x1000) };
                m.w32(args[3], off);
                0x7000 + self.g.below(0x100)
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

const OBJ: u32 = 0x4110;
const SYS: u32 = 0x3800;
const MENU: u32 = 0x420;

struct W {
    r: Runner,
    obj: u32,
    sys: u32,
    menu: u32,
    sp: u32,
    oracle: Rc<RefCell<Oracle>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let obj = r.mem.alloc(OBJ);
    let sys = r.mem.alloc(SYS);
    let menu = r.mem.alloc(MENU);
    let oracle = Rc::new(RefCell::new(Oracle { g: Rng(1), sys, log: vec![] }));
    for (t, _) in TARGETS {
        let l = oracle.clone();
        r.stub(t, move |c, m| {
            let args = [c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32];
            let v = l.borrow_mut().call(&mut RM2(m), t, &args);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    Some(W { r, obj, sys, menu, sp: 0x01ff_0000 - 0x900, oracle })
}

fn image(w: &mut W, sm: &mut SM, g: &mut Rng) {
    sm.0.clear();
    let mut img: Vec<(u32, Vec<u8>)> = vec![];
    img.push((w.obj, (0..OBJ).map(|_| g.next() as u8).collect()));
    img.push((w.sys, (0..SYS).map(|_| g.next() as u8).collect()));
    img.push((w.menu, (0..MENU).map(|_| g.next() as u8).collect()));
    let mut st: Vec<u8> = (0..16).map(|_| g.next() as u8).collect();
    st[8..12].copy_from_slice(&w.sys.to_le_bytes()); // 0x343050 is at +0x3050 of 0x340000: 0x343050
    img.push((0x343048, st));
    img.push((w.sp + 0x800, (0..0x60).map(|_| g.below(4) as u8).collect()));
    for (a, v) in &img {
        w.r.mem.write(*a, v).unwrap();
        for (k, x) in v.iter().enumerate() {
            sm.0.insert(a + k as u32, *x);
        }
    }
}

fn dump(w: &mut W) -> Vec<u8> {
    let mut out = vec![];
    for (a, n) in [(w.obj, OBJ), (w.sys, SYS), (w.menu, MENU), (w.sp + 0x800, 0x60)] {
        let mut b = vec![0u8; n as usize];
        w.r.mem.read(a, &mut b).unwrap();
        out.extend(b);
    }
    out
}

fn dump_sm(w: &W, sm: &SM) -> Vec<u8> {
    let mut out = vec![];
    for (a, n) in [(w.obj, OBJ), (w.sys, SYS), (w.menu, MENU), (w.sp + 0x800, 0x60)] {
        out.extend((a..a + n).map(|x| sm.r8(x)));
    }
    out
}

fn check(w: &mut W, sm: &mut SM, case: u32, seed: u64, addr: u32, set: &dyn Fn(&mut Runner), port: &dyn Fn(&mut SM, &mut PortEnv)) {
    {
        let mut o = w.oracle.borrow_mut();
        o.g = Rng(seed);
        o.log.clear();
    }
    w.r.cpu.set_gpr(29, 0x01ff_0000);
    set(&mut w.r);
    w.r.call(addr).unwrap();
    let theirs_calls = std::mem::take(&mut w.oracle.borrow_mut().log);
    let theirs = dump(w);
    let mine = Rc::new(RefCell::new(Oracle { g: Rng(seed), sys: w.sys, log: vec![] }));
    let mut env = PortEnv(mine.clone());
    port(sm, &mut env);
    let my_calls = std::mem::take(&mut mine.borrow_mut().log);
    for i in 0..my_calls.len().min(theirs_calls.len()) {
        assert_eq!(my_calls[i].0, theirs_calls[i].0, "case {case}: call {i}");
        assert_eq!(my_calls[i].1[..], theirs_calls[i].1[..my_calls[i].1.len()], "case {case}: args of call {i} ({:#x})", my_calls[i].0);
    }
    assert_eq!(my_calls.len(), theirs_calls.len(), "case {case}: number of calls");
    let mymem = dump_sm(w, sm);
    if mymem != theirs {
        let at = mymem.iter().zip(&theirs).position(|(x, y)| x != y).unwrap();
        panic!("case {case}: memory differs at +{at:#x}: port {:#x} game {:#x}", mymem[at], theirs[at]);
    }
}

#[test]
fn chant_banks_match() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(7);
    let mut sm = SM::default();
    for case in 0..12_000 {
        image(&mut w, &mut sm, &mut g);
        let file = g.next();
        let seed = g.next() as u64 | 1;
        let (obj, sp) = (w.obj, w.sp);
        check(
            &mut w,
            &mut sm,
            case,
            seed,
            0x22dce0,
            &|r| {
                r.cpu.set_gpr(4, obj as u64);
                r.cpu.set_gpr(5, file as i32 as i64 as u64);
            },
            &|sm, env| chant_port(sm, env, sp, obj, file),
        );
    }
}

fn chant_port(sm: &mut SM, env: &mut PortEnv, sp: u32, obj: u32, file: u32) {
    load_chant_banks(sm, env, sp, obj, file);
}

#[test]
fn music_list_matches() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(9);
    let mut sm = SM::default();
    for case in 0..20_000 {
        image(&mut w, &mut sm, &mut g);
        let seed = g.next() as u64 | 1;
        let menu = w.menu;
        check(
            &mut w,
            &mut sm,
            case,
            seed,
            0x187f58,
            &|r| r.cpu.set_gpr(4, menu as u64),
            &|sm, env| build_music_list(sm, env, menu),
        );
    }
}

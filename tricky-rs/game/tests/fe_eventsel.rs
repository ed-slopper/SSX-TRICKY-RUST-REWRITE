//! `tricky_game::fe_eventsel` (board row G9f) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random event select screens (the menu's item, the game type, the widgets, the players), every
//! `cFE3DStateEventSel` method run on both sides with the same answers from the calls out (the virtuals, the widget
//! constructor and screen helpers, the heap, the music, the front end's calls), the calls logged in order (colours by
//! value) and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{ALLOC, FILL, GAPP, INPUT_MASK, WIDGET_PLACE};
use tricky_game::fe_eventsel::*;
use tricky_game::fe_modeselect::{MUSIC_VF20, NUM_PLAYERS_SCREEN, PLAYERS_JOINED};
use tricky_game::fe_numplayers::{BUILD_TITLED, NEXT_FOR_PLAYERS, OVERLAY_MODE, SET_PLAYERS, SET_TEXT};
use tricky_game::fe_title::{BACKDROP, BASE_CTOR, TEXT_CTOR};
use tricky_game::pf::Mem;

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

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const FE: u32 = ARENA + 0x1000;
const SCREEN: u32 = ARENA + 0x2000;
const MENU: u32 = ARENA + 0x2400;
const ITEM: u32 = ARENA + 0x2600;
const GROUP: u32 = ARENA + 0x2800;
const PROMPT: u32 = ARENA + 0x2c00;
const SCENE: u32 = ARENA + 0x3000;
const P0: u32 = ARENA + 0x3400;
const P1: u32 = ARENA + 0x3800;
const NEW: u32 = ARENA + 0x4000;
/// the widgets and screens the calls make, 0x200 apart
const MADE: [u32; 6] = [ARENA + 0x5000, ARENA + 0x5200, ARENA + 0x5400, ARENA + 0x5600, ARENA + 0x5800, ARENA + 0x5a00];
const BUTTON: u32 = ARENA + 0x5c00;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const V_BASE: u32 = 0x7f_0000;

struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    fn answer(&mut self, target: u32) -> u32 {
        let n = match self.count.iter_mut().find(|c| c.0 == target) {
            Some(c) => {
                c.1 += 1;
                c.1
            }
            None => {
                self.count.push((target, 0));
                0
            }
        };
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..3 {
            g.next();
        }
        match target {
            // (the constructors answer the object; the heap one or none)
            TEXT_CTOR => MADE[n as usize % MADE.len()],
            ALLOC => [0, NEW, NEW][g.below(3) as usize],
            NEXT_FOR_PLAYERS | NUM_PLAYERS_SCREEN => [0, g.next()][g.below(2) as usize],
            PLAYERS_JOINED => [0, 1, 2, 3, u32::MAX][g.below(5) as usize],
            _ => g.below(2),
        }
    }
}
type A = Rc<RefCell<Answers>>;

fn arity(target: u32) -> usize {
    match target {
        ALLOC | FILL => 3,
        TEXT_CTOR | BUILD_TITLED => 4,
        INPUT_MASK | WIDGET_PLACE | SET_PLAYERS | NEXT_FOR_PLAYERS | OVERLAY_MODE | SET_TEXT | NUM_PLAYERS_SCREEN | HELP => 2,
        t if t == V_BASE + 0xb8 + 4 => 3,
        t if [0x10u32, 0x20, 0x28, 0x50, 0x78, 0x108, 0x110, 0x148, 0x150, 0x160].iter().any(|o| t == V_BASE + o + 4) => 2,
        _ => 1,
    }
}

/// The calls that take a vector by pointer: the colour (+0x50), the place (four words), the size getter (+0x130,
/// filled on both sides with words drawn from the case)
fn by_value(target: u32) -> usize {
    if target == V_BASE + 0x50 + 4 || target == V_BASE + 0x108 + 4 || target == V_BASE + 0x110 + 4 || target == WIDGET_PLACE {
        4
    } else {
        0
    }
}
fn getter_words(a: &A) -> [u32; 4] {
    let mut g = Rng(a.borrow().seed ^ 0x5151 ^ a.borrow().log.len() as u64);
    [0, 1, 2, 3].map(|_| (g.below(600) as f32 - 100.0).to_bits())
}

/// +0x20 and +0x28 take a flag on a widget or group, nothing on a screen
fn trim(target: u32, v: &mut Vec<u32>) {
    if (target == V_BASE + 0x28 + 4 || target == V_BASE + 0x20 + 4) && v[0] == SCREEN {
        v.truncate(1);
    }
}

fn log_call(a: &A, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    trim(target, &mut v);
    let n = by_value(target);
    if n != 0 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..n as u32).map(|k| read(at + 4 * k)));
    }
    if target == V_BASE + 0x130 + 4 {
        let at = v[1];
        for (k, x) in getter_words(a).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    a.borrow_mut().log.push((target, v));
    a.borrow_mut().answer(target)
}

struct Ours(A);
impl tricky_game::fe3dstate::FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let v: Vec<u32> = (0..arity(target)).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &mut |at| cell.borrow().r32(at), &mut |at, x| cell.borrow_mut().w32(at, x), target, v)
    }
}

const REGIONS: [(u32, u32); 5] = [(ARENA, 0x8000), (0x337c40, 0x40), (0x32de80, 0x10), (0x3b2ad0, 0x20), (0, 0x400)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

#[test]
fn event_select_screen_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    let mut leaves: Vec<u32> = vec![ALLOC, FILL, TEXT_CTOR, BASE_CTOR, BACKDROP, WIDGET_PLACE, INPUT_MASK, SET_PLAYERS, NEXT_FOR_PLAYERS, OVERLAY_MODE, SET_TEXT, BUILD_TITLED, NUM_PLAYERS_SCREEN, PLAYERS_JOINED, MUSIC_LOOP, MUSIC_VF18, MUSIC_VF20, HELP];
    leaves.extend(MUSIC_ITEMS);
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    // the base's methods the title calls run as the original
    for f in [0x274df0u32, 0x2758c0, 0x2759a0] {
        r.allow(f);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let mut v: Vec<u32> = (0..arity(f)).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == BUILD_TITLED {
                v = vec![c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32];
            }
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x7171_1e00_5c4e_0004);
    let mut counts = [0u32; 7];
    for case in 0..20_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x734, FE);
        r.mem.write_u32(APP + 0x5c, g.next());
        r.mem.write_u32(APP + 0x60, g.next());
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        for off in (0..0x170).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let mut objects = vec![SCREEN, MENU, GROUP, PROMPT, P0, P1, NEW, BUTTON];
        objects.extend(MADE);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        let s = SCREEN;
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, MENU);
        r.mem.write_u32(s + 0x4c, PROMPT);
        r.mem.write_u32(FE + 0x50, MADE[4]);
        r.mem.write_u32(MENU + 0x140, ITEM);
        r.mem.write_u32(ITEM, [0u32, 1, 2, 3, 4, 5, u32::MAX][g.below(7) as usize]);
        r.mem.write_u32(GAME_MODE, g.below(10));
        r.mem.write_u32(FE + 0xf0, [0u32, 1, 2, 3, u32::MAX][g.below(5) as usize]);
        r.mem.write_u32(ITEM + 0xc, VT);
        // the items vf22 is given are screens to the base vf22 (their +0x10, +0x20, +0xa8)
        for x in [MADE[3], ITEM] {
            r.mem.write_u32(x + 0xa8, g.below(2));
            r.mem.write_u32(x + 0x20, [5u32, 0][g.below(2) as usize]);
            r.mem.write_u32(x + 0x10, g.below(2));
        }
        let which = g.below(7);
        counts[which as usize] += 1;
        let from = [0, MADE[3]][g.below(2) as usize];
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let addr = [0x29ad48u32, 0x29b000, 0x29b2c8, 0x29b3a8, 0x29b3d8, 0x29b458, 0x29b550][which as usize];
        r.cpu.set_gpr(4, s as u64);
        r.cpu.set_gpr(5, if which == 6 { ITEM } else { from } as u64);
        r.cpu.set_gpr(6, ITEM as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        let mine_ret = match which {
            0 => Some(create_next_state(&mut m, e, s)),
            1 => {
                build(&mut m, e, s, BUF);
                None
            }
            2 => {
                activate(&mut m, e, s, from);
                None
            }
            3 => {
                vf18(&mut m, e);
                None
            }
            4 => {
                vf20(&mut m, e, s);
                None
            }
            5 => {
                vf22(&mut m, e, s, from, ITEM);
                None
            }
            _ => {
                vf26(&mut m, e, s, ITEM);
                None
            }
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x20).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}

/// 0x29b5d0 (`fe_eventsel::help`) on its own: the help line's text by the item, against the original
#[test]
fn the_help_line_follows_the_item() {
    let Some(mut r) = Runner::from_env() else { return };
    let log: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    let l2 = log.clone();
    r.stub(SET_TEXT, move |c, _| {
        l2.borrow_mut().push((c.gpr(4) as u32, c.gpr(5) as u32));
    });
    let mut g = Rng(0x29b5_d000_0000_0001);
    for case in 0..2000 {
        let item_word = [0u32, 1, 2, 3, 4, 5, u32::MAX][g.below(7) as usize];
        r.mem.write_u32(SCREEN + 0x4c, PROMPT + g.below(16) * 4);
        r.mem.write_u32(ITEM, item_word);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, SCREEN as u64);
        r.cpu.set_gpr(5, ITEM as u64);
        r.call(HELP).unwrap();
        let theirs = log.borrow().clone();
        let mut m = snap(&mut r);
        struct Text(Vec<(u32, u32)>);
        impl tricky_game::fe3dstate::FeEnv for Text {
            fn call(&mut self, _m: &mut Mem, _target: u32, args: &[u32]) -> u32 {
                self.0.push((args[0], args[1]));
                0
            }
        }
        let mut e = Text(Vec::new());
        help(&mut m, &mut e, SCREEN, ITEM);
        assert_eq!(theirs, e.0, "case {case}");
    }
}

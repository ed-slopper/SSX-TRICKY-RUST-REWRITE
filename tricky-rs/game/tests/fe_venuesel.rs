//! `tricky_game::fe_venuesel` (board row G9g) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random venue select screens (the menu's item, the game mode, the widgets, the overlay's venue), every
//! `cFE3DStateVenueSel` method run on both sides with the same answers from the calls out (the virtuals, the widget
//! constructor, the heap, the career's lock and text queries, the music and camera calls), the calls logged in order
//! (vectors by value, the wide-text scratch left out) and the memory compared. The lock update and the event title
//! are stubbed in the screen's methods and run on their own in a second runner.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{ALLOC, FILL, GAPP, INPUT_MASK, LIST_END, LIST_FIRST, LIST_NTH, TRANSITION_START, WIDGET_PLACE};
use tricky_game::fe_eventsel::MUSIC_VF18;
use tricky_game::fe_modeselect::{BUILD_TITLED, GAME_MODE};
use tricky_game::fe_numplayers::SET_TEXT;
use tricky_game::fe_title::{BACKDROP, TEXT_CTOR};
use tricky_game::fe_venuesel::*;
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
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const V_BASE: u32 = 0x7f_0000;
/// the screen before / the item going (its +0x70 answers the screen's kind)
const PREV: u32 = MADE[3];

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
            TEXT_CTOR | LIST_NTH => MADE[n as usize % MADE.len()],
            ALLOC => [0, NEW, NEW][g.below(3) as usize],
            // (an empty widget list for the base's layout)
            LIST_FIRST => 0,
            CIRCUIT_TITLE | COURSE_NAME | LOC_STRING | NAME_HASH => g.next(),
            t if t == V_BASE + 0x70 + 4 => [6, 0, 1][g.below(3) as usize],
            _ => g.below(2),
        }
    }
}
type A = Rc<RefCell<Answers>>;

fn arity(target: u32) -> usize {
    match target {
        TRANSITION_START => 6,
        ALLOC | FILL | COURSE_UNLOCKED | COURSE_VIDEO | WSTR_FORMAT | MARKER_FADE => 3,
        TEXT_CTOR | BUILD_TITLED => 4,
        INPUT_MASK | WIDGET_PLACE | LIST_NTH | SET_TEXT | FILL_TITLE | SHOW_MEDALS | SET_VENUE | CAMERA_NEXT
        | CAMERA_PAN | CAMERA_SHOT | COURSE_NAME | LOC_STRING | WSTR_COPY | SET_WIDE_TEXT => 2,
        t if [0x10u32, 0x20, 0x28, 0x70, 0x100, 0x130, 0x138, 0x148, 0x158, 0x160].iter().any(|o| t == V_BASE + o + 4) => 2,
        _ => 1,
    }
}

/// The calls that take a vector by pointer: the move (+0x70, three words), the place (+0x138 and 0x2acb20, four)
fn by_value(target: u32) -> usize {
    if target == V_BASE + 0x70 + 4 {
        3
    } else if target == V_BASE + 0x138 + 4 || target == WIDGET_PLACE {
        4
    } else {
        0
    }
}
fn getter_words(a: &A) -> [u32; 4] {
    let mut g = Rng(a.borrow().seed ^ 0x5151 ^ a.borrow().log.len() as u64);
    [0, 1, 2, 3].map(|_| (g.below(600) as f32 - 100.0).to_bits())
}

/// +0x20 and +0x28 take a flag on a widget or group, nothing on a screen; +0x70 is a query on the screen before;
/// the wide-text calls' scratch (the original's stack) is left out
fn trim(target: u32, v: &mut Vec<u32>) {
    if (target == V_BASE + 0x28 + 4 || target == V_BASE + 0x20 + 4) && v[0] == SCREEN {
        v.truncate(1);
    }
    if target == V_BASE + 0x70 + 4 && v[0] == PREV {
        v.truncate(1);
    }
    if target == WSTR_COPY || target == WSTR_FORMAT {
        v.remove(0);
    }
    if target == SET_WIDE_TEXT {
        v.truncate(1);
    }
}

fn log_call(a: &A, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    trim(target, &mut v);
    if target == TRANSITION_START {
        let at = v[4];
        v.splice(4..5, (0..3).map(|k| read(at + 4 * k)));
    }
    let n = by_value(target);
    if n != 0 && v.len() > 1 {
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

const REGIONS: [(u32, u32); 7] = [
    (ARENA, 0x8000),
    (0x337c40, 0x40),
    (0x32de80, 0x10),
    (0x3a8af0, 0x200),
    (0x332f28, 0x200),
    (0x3ae520, 0x10),
    (0, 0x400),
];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

/// The screen's methods (`direct` false), or the lock update and the event title on their own (`direct` true)
fn run(direct: bool) {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    let mut leaves: Vec<u32> = vec![
        ALLOC, FILL, TEXT_CTOR, BACKDROP, WIDGET_PLACE, INPUT_MASK, LIST_FIRST, LIST_END, LIST_NTH, BUILD_TITLED,
        SET_TEXT, SETUP_RACE, COURSE_UNLOCKED, MARKER_FADE, NAME_HASH, SHOW_MEDALS, COURSE_VIDEO, MUSIC_LOOP,
        SET_VENUE, CAMERA_NEXT, CAMERA_PAN, CAMERA_SHOT, CIRCUIT_TITLE, COURSE_NAME, LOC_STRING, WSTR_COPY,
        WSTR_FORMAT, SET_WIDE_TEXT, MUSIC_VF18, TRANSITION_START,
    ];
    if !direct {
        leaves.extend([UPDATE_LOCKS, FILL_TITLE]);
    }
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    // the base's methods the screen calls run as the original
    for f in [0x274df0u32, 0x274ec0, 0x2758c0, 0x2759a0, 0x274f18] {
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
            if f == MARKER_FADE {
                v = vec![c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32];
            }
            if f == TRANSITION_START {
                v = vec![c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32];
            }
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(if direct { 0x7171_1e00_5c4e_0006 } else { 0x7171_1e00_5c4e_0005 });
    let methods: &[u32] = if direct {
        &[0x29baa0, 0x29c010]
    } else {
        &[0x29ba50, 0x29bb90, 0x29bd88, 0x29be58, 0x29bee0, 0x29bf10, 0x29bf40, 0x29c238]
    };
    let mut counts = vec![0u32; methods.len()];
    // the order table as the game has it, put back each case (cases plant a byte past 0x7f past its ten courses)
    let mut order = vec![0u8; 0x200];
    r.mem.read(COURSE_ORDER, &mut order).unwrap();
    for case in 0..20_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x734, FE);
        r.mem.write_u32(APP + 0x5c, g.next());
        r.mem.write_u32(APP + 0x50, g.next());
        r.mem.write_u32(APP + 0x72c, g.next());
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        r.mem.write_u32(FE + 0x50, MADE[4]);
        r.mem.write_u32(FE + 0xf0, [0u32, 4, 2][g.below(3) as usize]);
        r.mem.write_u8(MADE[4] + 0x185, [0u8, 3, 9, 0xff][g.below(4) as usize]);
        for off in (0..0x170).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let mut objects = vec![SCREEN, MENU, GROUP, PROMPT, P0, P1, NEW, ITEM];
        objects.extend(MADE);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        let s = SCREEN;
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, MENU);
        r.mem.write_u32(s + 0x40, [0, PROMPT][g.below(2) as usize]);
        r.mem.write_u32(s + 0x4c, PROMPT);
        for k in 0..10 {
            r.mem.write_u32(s + 0x54 + 4 * k, MADE[k as usize % MADE.len()]);
        }
        r.mem.write_u32(MENU + 0x140, [ITEM, ITEM, ITEM, 0][g.below(4) as usize]);
        // the title (+0x3c) as the base's layout reads it: its width and x as small floats, its halves
        r.mem.write_f32(MENU + 0x170, g.below(600) as f32);
        r.mem.write_f32(MENU + 0x80, g.below(300) as f32);
        r.mem.write_u32(MENU + 0x148, MADE[1]);
        r.mem.write_u32(MENU + 0x14c, MADE[2]);
        // the items: a menu place (a few past the ten), the World Circuit flag; the base vf22's +0x10, +0x20, +0xa8
        for x in [PREV, ITEM] {
            r.mem.write_u32(x, [0u32, 1, 5, 6, 7, 8, 9, 0x80][g.below(8) as usize] + [0, 0x100][g.below(2) as usize]);
            r.mem.write_u32(x + 0xa8, [0, 0, 1][g.below(3) as usize]);
            r.mem.write_u32(x + 0x20, [5u32, 0][g.below(2) as usize]);
            r.mem.write_u32(x + 0x10, g.below(2));
        }
        // (one-on and nine-to-zero moves often)
        if g.below(3) == 0 {
            let to = r.mem.read_u32(ITEM) & 0xff;
            r.mem.write_u32(PREV, if to == 0 { 9 } else { to.wrapping_sub(1) & 0xff });
        }
        r.mem.write_u32(GAME_MODE, g.below(10));
        r.mem.write(COURSE_ORDER, &order).unwrap();
        let it = r.mem.read_u32(ITEM);
        if (10..0x200).contains(&it) && g.below(2) == 0 {
            r.mem.write_u8(COURSE_ORDER + it, 0xf5);
        }
        let which = g.below(methods.len() as u32) as usize;
        counts[which] += 1;
        let from = [0, PREV][g.below(2) as usize];
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let addr = methods[which];
        r.cpu.set_gpr(4, s as u64);
        let a1 = if addr == 0x29c238 || addr == 0x29c010 { ITEM } else { from };
        r.cpu.set_gpr(5, a1 as u64);
        r.cpu.set_gpr(6, ITEM as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        let mine_ret = match addr {
            0x29ba50 => Some(create_next_state(&mut m, e, s)),
            0x29bb90 => {
                build(&mut m, e, s);
                None
            }
            0x29bd88 => {
                activate(&mut m, e, s, from);
                None
            }
            0x29be58 => {
                deactivate(&mut m, e, s);
                None
            }
            0x29bee0 => {
                vf18(&mut m, e);
                None
            }
            0x29bf10 => {
                layout_widgets(&mut m, e, s, BUF);
                None
            }
            0x29bf40 => {
                vf22(&mut m, e, s, from, ITEM);
                None
            }
            0x29c238 => {
                vf26(&mut m, e, s, ITEM, BUF);
                None
            }
            0x29baa0 => {
                update_locks(&mut m, e, s);
                None
            }
            _ => {
                fill_event_title(&mut m, e, s, ITEM, BUF);
                None
            }
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x300).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}

#[test]
fn venue_select_screen_matches_the_original() {
    run(false);
}

#[test]
fn locks_and_event_title_match_the_original() {
    run(true);
}

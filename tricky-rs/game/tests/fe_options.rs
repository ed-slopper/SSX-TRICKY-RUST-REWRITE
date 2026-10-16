//! `tricky_game::fe_options` (board row G9o) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random options screens (the page open, the pages' widgets and controls, the chosen lines, the
//! display), every `cFE3DStateOptions` method run on both sides with the same answers from the calls out (the
//! virtuals, the widget constructor, the heap, the pages' helpers, the music), the calls logged in order (vectors by
//! value, the stack's scratch left out) and the memory compared. The builder's page groups get the game's group
//! vtable, so the group's functions it calls are stubbed by their addresses read from it. The display's apply is
//! stubbed in the screen's methods and runs on its own in a second runner.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{
    ALLOC, FILL, GAPP, INPUT_MASK, LIST_END, LIST_FIRST, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE,
};
use tricky_game::fe_charselect::{PRINT, STATS};
use tricky_game::fe_numplayers::BUILD_TITLED;
use tricky_game::fe_options::*;
use tricky_game::fe_title::{BACKDROP, BASE_CTOR, HELP_BUTTON, TEXT_CTOR};
use tricky_game::fe_widgets::*;
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
const PANEL: u32 = ARENA + 0x4400;
const RECT: u32 = ARENA + 0x4800;
/// the small font (gApp+0x60; its +0xc the height)
const FONT: u32 = ARENA + 0x4c00;
const MADE: [u32; 6] = [ARENA + 0x5000, ARENA + 0x5200, ARENA + 0x5400, ARENA + 0x5600, ARENA + 0x5800, ARENA + 0x5a00];
const PREV: u32 = MADE[3];
/// a second chosen word (the controls' lines)
const ITEM2: u32 = ARENA + 0x2680;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
/// the heap the builder's objects come from, 0x400 apart
const HEAP: u32 = ARENA + 0x8000;
const V_BASE: u32 = 0x7f_0000;

/// The game's functions behind the slots the builder calls on its in-place widgets: (address, arguments, words by
/// value); read from the ELF's vtables at start
type Real = Rc<Vec<(u32, usize, usize)>>;
const REAL_SLOTS: [(u32, u32, usize, usize); 2] = [(GROUP_VTABLE, 0x28, 2, 0), (GROUP_VTABLE, 0x20, 2, 0)];
/// The display (gApp+0x724; its vtable at +0x10)
const DISP: u32 = ARENA + 0x4c00 + 0x100;

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
            TEXT_CTOR | LIST_NTH | LIST_AT | HELP_BUTTON => MADE[n as usize % MADE.len()],
            // (not the pages the screen holds, so a page written for the wrong line shows)
            PAGE_WIDGET => MADE[(n as usize + 2) % MADE.len()],
            CARD_SCREEN => [0, MADE[2]][g.below(2) as usize],
            // (with words whose top bit is set and whose low bits round differently with the kept bit)
            t if t == V_BASE + 0x58 + 4 || t == V_BASE + 0x60 + 4 => {
                [g.next(), 0x8000_0081, 0x8000_0281, 640][g.below(4) as usize]
            }
            ALLOC => {
                if g.below(16) == 0 {
                    0
                } else {
                    HEAP + 0x400 * (n % 64)
                }
            }
            LIST_FIRST => 0,
            t if t == V_BASE + 0x70 + 4 => [0xdu32, 0, 1][g.below(3) as usize],
            _ => g.below(2),
        }
    }
}
type A = Rc<RefCell<Answers>>;

fn arity(real: &Real, target: u32) -> (usize, usize) {
    if let Some(r) = real.iter().find(|r| r.0 == target) {
        return (r.1, r.2);
    }
    let n = match target {
        PAGE_WIDGET => 6,
        ALLOC | FILL | PRINT | CARD_SCREEN | ITEM_MOVES0 | ITEM_MOVES1 | ITEM_MOVES2 | ITEM_MOVES3 => 3,
        TEXT_CTOR | BUILD_TITLED | STATS => 4,
        INPUT_MASK | WIDGET_PLACE | LIST_NTH | LIST_AT | GROUP_ADD | HELP_BUTTON | WIDGETS_RESET | LINE_CHOSEN
        | HELP | SET_VOLUME => 2,
        t if t == V_BASE + 0x98 + 4 => 5,
        t if t == V_BASE + 0x128 + 4 => 3,
        t if [8u32, 0x10, 0x20, 0x28, 0x50, 0x70, 0x100, 0x108, 0x110, 0x120, 0x130, 0x148, 0x150, 0x158, 0x160, 0x168,
            0x178, 0x198, 0x1a8]
            .iter()
            .any(|o| t == V_BASE + o + 4) =>
        {
            2
        }
        _ => 1,
    };
    let by = if target == V_BASE + 0x70 + 4 {
        3
    } else if [0x50u32, 0x108, 0x110, 0x138].iter().any(|o| target == V_BASE + o + 4) || target == WIDGET_PLACE {
        4
    } else {
        0
    };
    (n, by)
}
const ITEM_MOVES0: u32 = ITEM_MOVES[0];
const ITEM_MOVES1: u32 = ITEM_MOVES[1];
const ITEM_MOVES2: u32 = ITEM_MOVES[2];
const ITEM_MOVES3: u32 = ITEM_MOVES[3];

fn getter_words(a: &A) -> [u32; 4] {
    let mut g = Rng(a.borrow().seed ^ 0x5151 ^ a.borrow().log.len() as u64);
    [0, 1, 2, 3].map(|_| (g.below(600) as f32 - 100.0).to_bits())
}
fn stats_words(a: &A) -> [u32; 16] {
    let mut g = Rng(a.borrow().seed ^ 0x7777 ^ a.borrow().log.len() as u64);
    [0; 16].map(|_| g.below(21))
}

/// The game's pointer arguments that are its stack (or our scratch): only constants are compared
fn is_scratch(v: u32) -> bool {
    !(0x10_0000..0x40_0000).contains(&v)
}

fn trim(target: u32, v: &mut Vec<u32>) {
    // +0x20, +0x28 and +0x50 take nothing on a screen (the base's update asks +0x50 of it)
    if [0x20u32, 0x28, 0x50].iter().any(|o| target == V_BASE + o + 4) && v[0] == SCREEN {
        v.truncate(1);
    }
    // +0x70 is a query on the screen before, a move on the title
    if target == V_BASE + 0x70 + 4 && v[0] == PREV {
        v.truncate(1);
    }
    if target == PRINT {
        v.remove(0);
    }
    if target == V_BASE + 0x158 + 4 && is_scratch(v[1]) {
        v.truncate(1);
    }
}

fn log_call(a: &A, real: &Real, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    trim(target, &mut v);
    let (_, by) = arity(real, target);
    // (a small word is a value, as the scene's cue, not a vector)
    if by != 0 && v.len() > 1 && v[1] >= 0x600 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..by as u32).map(|k| read(at + 4 * k)));
    }
    if target == V_BASE + 0x128 + 4 {
        let (p, q) = (v[1], v[2]);
        for (k, x) in getter_words(a).iter().enumerate() {
            write(p + 4 * k as u32, *x);
            write(q + 4 * k as u32, x ^ 0x0010_0000);
        }
        v.truncate(1);
    }
    if target == V_BASE + 0x130 + 4 && v.len() > 1 {
        let at = v[1];
        for (k, x) in getter_words(a).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    if target == STATS {
        let at = v[3];
        for (k, x) in stats_words(a).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(3);
    }
    a.borrow_mut().log.push((target, v));
    a.borrow_mut().answer(target)
}

struct Ours(A, Real);
impl tricky_game::fe3dstate::FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let (n, _) = arity(&self.1, target);
        let v: Vec<u32> = (0..n).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &self.1, &mut |at| cell.borrow().r32(at), &mut |at, x| cell.borrow_mut().w32(at, x), target, v)
    }
}

const REGIONS: [(u32, u32); 8] = [
    (0x32ca00, 0x60),
    (ARENA, 0x8000),
    (HEAP, 0x10000),
    (0x337c40, 0x40),
    (0x3352c0, 0x40),
    (0x3a9e00, 0xd800),
    (0x3ae520, 0x40),
    (0, 0x600),
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

fn run(direct: bool) {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let real: Real = Rc::new(
        REAL_SLOTS
            .iter()
            .map(|&(vt, off, n, by)| {
                let f = r.mem.read_u32(vt + off + 4);
                (f, n, by)
            })
            .collect(),
    );
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    let mut leaves: Vec<u32> = vec![
        ALLOC, FILL, BASE_CTOR, TEXT_CTOR, BACKDROP, WIDGET_PLACE, INPUT_MASK, LIST_FIRST, LIST_END, LIST_NTH, LIST_AT,
        BUILD_TITLED, HELP_BUTTON, UPDATE_COLOR, LIST_INIT, GROUP_ADD, WIDGETS_RESET, TITLE_EXTRAS, PAGE_WIDGET,
        PAGES_LAYOUT, VALUES_READ, LINE_CHOSEN, HELP, SLIDER_ARROWS, LIST_ARROWS, SET_VOLUME, CARD_SCREEN, MUSIC_LOOP,
    ];
    for l in [&PAGE_BUILDERS[..], &ITEM_MOVES[..], &PAGE_OPEN[..], &PAGE_CLOSE[..], &PAGE_CUES[..]] {
        leaves.extend(l);
    }
    leaves.extend([PAGE_KEEP[0], PAGE_KEEP[1]]);
    if !direct {
        leaves.push(PAGE_KEEP[2]);
    }
    leaves.extend(real.iter().map(|r| r.0));
    for off in (0..0x200).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    leaves.sort_unstable();
    leaves.dedup();
    // the base's methods the screen calls run as the original
    for f in [0x274df0u32, 0x2758c0, 0x275888, 0x2717e8, 0x274f18] {
        r.allow(f);
    }
    for f in leaves {
        let a2 = a.clone();
        let real2 = real.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let (n, _) = arity(&real2, f);
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == BUILD_TITLED {
                v = vec![c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32];
            }
            if f == SET_VOLUME || [0x158u32, 0x168, 0x178, 0x198, 0x1a8].iter().any(|o| f == V_BASE + o + 4) {
                v = vec![c.gpr(4) as u32, c.f(12).to_bits()];
            }
            if f == V_BASE + 0x98 + 4 {
                v = vec![c.gpr(4) as u32, c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits(), c.f(15).to_bits()];
            }
            if f == PAGE_WIDGET {
                v = vec![c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32];
            }
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &real2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(if direct { 0x2a8a_0000_0000_0002 } else { 0x2a8a_0000_0000_0001 });
    let methods: &[u32] = if direct {
        &[0x29f720]
    } else {
        &[0x29e920, 0x2a9208, 0x2a8a30, 0x2a9258, 0x2a9c58, 0x2a9d38, 0x2aa098, 0x2aa1f8, 0x2aa5c0, 0x2aa820]
    };
    let mut counts = vec![0u32; methods.len()];
    let cases = 20_000u32;
    for case in 0..cases {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        for k in 0..0x10000 / 4 {
            r.mem.write_u32(HEAP + 4 * k, 0x5a5a_5a5a ^ k);
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x734, FE);
        r.mem.write_u32(APP + 0x5c, g.next());
        r.mem.write_u32(APP + 0x60, FONT);
        r.mem.write_u32(FONT + 0xc, g.below(30) + 8);
        r.mem.write_u32(APP + 0x72c, g.next());
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        r.mem.write_u32(FE + 0x50, 0);
        r.mem.write_u32(FE + 0xf0, [0u32, 2, 3][g.below(3) as usize]);
        r.mem.write_u8(FE + 4, [0u8, 1, 2][g.below(3) as usize]);
        for k in 0..4 {
            r.mem.write_u8(FE + 0x7c + k, [0u8, 5, 11, 12, 0xff][g.below(5) as usize]);
        }
        for k in 0..12 {
            r.mem.write_u32(FE + 0x20 + 4 * k, MADE[k as usize % MADE.len()]);
        }
        for off in (0..0x200).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let mut objects = vec![SCREEN, MENU, GROUP, PROMPT, P0, P1, ITEM, PANEL];
        objects.extend(MADE);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        for (k, x) in MADE.iter().enumerate() {
            r.mem.write_u32(x + 0xf0, MADE[(k + 2) % MADE.len()]);
            r.mem.write_u32(x + 0x14, g.below(2));
            r.mem.write_u8(x + 0x158, [0u8, 3, 0xff][g.below(3) as usize]);
            for o in [0x80u32, 0x84, 0x104] {
                r.mem.write_f32(x + o, g.below(400) as f32 - 50.0);
            }
        }
        r.mem.write_f32(RECT + 4, g.below(40) as f32);
        r.mem.write_f32(RECT + 8, g.below(40) as f32);
        let s = SCREEN;
        r.mem.write_u32(s + 0x10, 0);
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, MENU);
        r.mem.write_u32(s + 0x40, [0, PROMPT][g.below(2) as usize]);
        r.mem.write_u32(s + 0x44, MADE[0]);
        r.mem.write_u32(s + 0x4c, PROMPT);
        r.mem.write_u32(s + 0x38, PANEL);
        r.mem.write_u32(s + 0x40, PROMPT);
        r.mem.write_u32(s + 0x78, [u32::MAX, 0, 1, 2, 3, 4, 5][g.below(7) as usize]);
        for k in 0..5u32 {
            r.mem.write_u32(s + 0x7c + 12 * k, g.next());
            r.mem.write_u32(s + 0x80 + 12 * k, MADE[k as usize]);
            r.mem.write_u32(s + 0x84 + 12 * k, MADE[(k as usize + 1) % MADE.len()]);
        }
        r.mem.write_u32(s + 0xc0, MADE[5]);
        for o in [0xb4u32, 0xb8, 0xbc] {
            r.mem.write_u32(s + o, MADE[g.below(6) as usize]);
        }
        for (k, x) in MADE.iter().enumerate() {
            r.mem.write_u32(x + 0x140, [ITEM, ITEM2][k % 2]);
            // (an odd neighbour too, so a line's choice can be ITEM2's word: the display's screen modes 9..11)
            for o in [0xf0u32, 0x110, 0x114] {
                r.mem.write_u32(x + o, [0, MADE[(k + 3) % MADE.len()], MADE[(k + 2) % MADE.len()]][g.below(3) as usize]);
            }
            r.mem.write_u32(x + 0x160, g.next());
            r.mem.write_u32(x + 0x154, (g.below(200) as f32).to_bits());
        }
        for x in [ITEM, MENU] {
            for o in [0xf0u32, 0x110, 0x114] {
                r.mem.write_u32(x + o, [0, MADE[g.below(6) as usize]][g.below(2) as usize]);
            }
            // (an item answering +0xb8 has its list at +0x110; vf26 calls it unchecked)
            r.mem.write_u32(x + 0x110, MADE[g.below(6) as usize]);
            r.mem.write_u32(x + 0x160, g.next());
        }
        r.mem.write_u32(ITEM2, [0u32, 1, 5, 8, 9, 10, 11, 12, 15, 0x10, 0x14, 0x15][g.below(12) as usize]);
        r.mem.write_u8(FE + 6, [0u8, 0, 0, 1][g.below(4) as usize]);
        r.mem.write(0, &[0u8; 0x600]).unwrap();
        r.mem.write_u32(APP + 0x724, DISP);
        r.mem.write_u32(DISP + 0x10, VT);
        r.mem.write_u32(MENU + 0x140, ITEM);
        r.mem.write_f32(MENU + 0x170, g.below(600) as f32);
        r.mem.write_f32(MENU + 0x80, g.below(300) as f32);
        r.mem.write_u32(MENU + 0x144, g.below(2));
        r.mem.write_u32(MENU + 0x148, MADE[1]);
        r.mem.write_u32(MENU + 0x14c, MADE[2]);
        for x in [PREV, ITEM] {
            r.mem.write_u32(x, [0u32, 1, 2, 3, 4, 5, 6, 7, ACCEPT, ACCEPT][g.below(10) as usize]);
            r.mem.write_u32(x + 0xa8, g.below(2));
            r.mem.write_u32(x + 0x20, [5u32, 0][g.below(2) as usize]);
            r.mem.write_u32(x + 0x10, g.below(2));
            r.mem.write_u8(x + 0x2e, [0u8, 1, 0xff][g.below(3) as usize]);
        }
        let which = g.below(methods.len() as u32) as usize;
        let addr = methods[which];
        counts[which] += 1;
        // the locked text's place: only riders the order puts from 4 on (the game reads its stack for the others)
        let from = if addr == 0x2a9258 { PREV } else { [0, PREV, MENU][g.below(3) as usize] };
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        // (picking with no page open: the item is the menu's, a page's number)
        if addr == 0x2aa1f8 && r.mem.read_u32(s + 0x78) == u32::MAX {
            r.mem.write_u32(ITEM, g.below(5));
            r.mem.write_u32(MENU, g.below(5));
        }
        let mut m = snap(&mut r);
        r.cpu.set_gpr(4, s as u64);
        let a1 = if [0x2aa098u32, 0x2aa1f8, 0x2aa5c0].contains(&addr) { [ITEM, MENU][g.below(2) as usize] } else { from };
        r.cpu.set_gpr(5, a1 as u64);
        r.cpu.set_gpr(6, ITEM as u64);
        let a2 = r.cpu.gpr(6) as u32;
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone(), real.clone());
        let mine_ret = match addr {
            0x29e920 => Some(construct(&mut m, e, s)),
            0x2a9208 => {
                vf12(&mut m, e, s);
                None
            }
            0x2a8a30 => {
                build(&mut m, e, s, BUF);
                None
            }
            0x2a9258 => {
                activate(&mut m, e, s, from);
                None
            }
            0x2a9c58 => Some(create_next_state(&mut m, e, s)),
            0x2a9d38 => {
                vf22(&mut m, e, s, from, a2);
                None
            }
            0x2aa098 => {
                vf23(&mut m, e, s, a1);
                None
            }
            0x2aa1f8 => {
                vf26(&mut m, e, s, a1);
                None
            }
            0x2aa5c0 => {
                vf21(&mut m, e, s, a1);
                None
            }
            0x2aa820 => {
                layout_widgets(&mut m, e, s, BUF);
                None
            }
            _ => {
                apply_display(&mut m, e, s);
                None
            }
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        if theirs != mine {
            let k = (0..theirs.len().min(mine.len())).find(|&k| theirs[k] != mine[k]).unwrap_or(theirs.len().min(mine.len()));
            panic!(
                "case {case}: {addr:#x} calls differ at {k}: theirs {:x?}, mine {:x?} ({} and {} calls)",
                theirs.get(k),
                mine.get(k),
                theirs.len(),
                mine.len()
            );
        }
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x100).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}

#[test]
fn options_screen_matches_the_original() {
    run(false);
}

#[test]
fn display_apply_matches_the_original() {
    run(true);
}

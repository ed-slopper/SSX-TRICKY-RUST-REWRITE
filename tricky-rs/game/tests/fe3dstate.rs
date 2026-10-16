//! `tricky_game::fe3dstate` (board row G9b) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random screens (phases, flags, which widgets they have and whether those still animate, an overlay
//! or none, the players' input masks), every `cFE3DState` method run on both sides with the same answers from the
//! calls out (each object's virtuals, the pad queries, the fade, the heap, the screen stack), the calls logged in
//! order (the fade's colour by value) and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::*;
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
const NEXT: u32 = ARENA + 0x2400;
const TITLE: u32 = ARENA + 0x2800;
const HELP: u32 = ARENA + 0x2c00;
const BACKDROP: u32 = ARENA + 0x3000;
const GROUP: u32 = ARENA + 0x3400;
const MENU: u32 = ARENA + 0x3800;
const OVERLAY: u32 = ARENA + 0x3c00;
const SCENE: u32 = ARENA + 0x4000;
const P0: u32 = ARENA + 0x4400;
const P1: u32 = ARENA + 0x4800;
const VT: u32 = ARENA + 0x5000;
const BUF: u32 = ARENA + 0x6000;
/// Every virtual's function: a stub at V_BASE + the entry's offset
const V_BASE: u32 = 0x7f_0000;
const OBJECTS: [u32; 10] = [SCREEN, NEXT, TITLE, HELP, BACKDROP, GROUP, MENU, OVERLAY, P0, P1];

/// The calls that answer an object (dereferenced after)
fn answers_object(target: u32) -> bool {
    target == V_BASE + 0x5c || target == BACK_SCREEN || target == ALLOC
}

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
        if target == LIST_FIRST {
            [0, TITLE, HELP, BACKDROP][g.below(4) as usize]
        } else if target == LIST_NTH {
            // (the help bar has its three widgets)
            [TITLE, HELP, BACKDROP][g.below(3) as usize]
        } else if target == BACK_SCREEN {
            // (the constructor answers the object it made)
            NEXT
        } else if answers_object(target) {
            [0, NEXT, NEXT][g.below(3) as usize]
        } else {
            g.below(2)
        }
    }
}
type A = Rc<RefCell<Answers>>;

/// How many integer arguments each call is logged with; the fade logs its colour's three words in place of a1
fn arity(target: u32) -> usize {
    match target {
        TRANSITION_START => 6,
        ALLOC | FILL => 3,
        STACK_PUSH | INPUT_MASK | WIDGETS_RESET | LIST_NTH | WIDGET_PLACE => 2,
        LIST_FIRST | LIST_END => 1,
        STACK_TOP | STACK_POP | PAD_PAUSE | PAD_ACCEPT | PAD_BACK | OVERLAY_CLOSE | BACK_SCREEN => 1,
        PAD_ANY => 1,
        // the virtuals that take an argument (show / input on, the group's release, the menu's input)
        t if t == V_BASE + 0x20 + 4 || t == V_BASE + 0x28 + 4 || t == V_BASE + 8 + 4 || t == V_BASE + 0xe8 + 4 => 2,
        // a vector by pointer: logged by value (+0x70 three words, +0x138 and the place four; the getter none)
        t if t == V_BASE + 0x70 + 4 || t == V_BASE + 0x138 + 4 || t == V_BASE + 0x130 + 4 => 2,
        _ => 1,
    }
}

/// The calls that take a vector by pointer (the logged pointer replaced by its words), and the size getter (+0x130),
/// which fills it with words drawn from the case
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

/// The slots whose argument depends on the object: +0xe8 is the menu's input (the player) but the overlay's
/// "closing" query (nothing)
fn trim(target: u32, v: &mut Vec<u32>) {
    if target == V_BASE + 0xe8 + 4 && v[0] != MENU {
        v.truncate(1);
    }
    // +0x28 is the widget group's input switch (a flag) but a plain call on a screen
    if target == V_BASE + 0x28 + 4 && v[0] != GROUP {
        v.truncate(1);
    }
}

struct Ours(A);
impl FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let mut v: Vec<u32> = (0..arity(target)).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        trim(target, &mut v);
        if target == TRANSITION_START {
            let at = v[4];
            v.splice(4..5, (0..3).map(|k| m.r32(at + 4 * k)));
        }
        let n = by_value(target);
        if n != 0 {
            let at = v[1];
            v.truncate(1);
            v.extend((0..n as u32).map(|k| m.r32(at + 4 * k)));
        }
        if target == V_BASE + 0x130 + 4 {
            let w = getter_words(&self.0);
            for (k, x) in w.iter().enumerate() {
                m.w32(args[1] + 4 * k as u32, *x);
            }
            v.truncate(1);
        }
        self.0.borrow_mut().log.push((target, v));
        self.0.borrow_mut().answer(target)
    }
}

const REGIONS: [(u32, u32); 3] = [(ARENA, 0x7000), (0x337c40, 0x40), (0, 0x400)];

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
fn screen_base_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    let mut leaves: Vec<u32> = vec![TRANSITION_START, PAD_PAUSE, PAD_ACCEPT, PAD_BACK, PAD_ANY, OVERLAY_CLOSE, ALLOC, FILL, BACK_SCREEN, STACK_PUSH, STACK_POP, STACK_TOP, INPUT_MASK, WIDGETS_RESET, LIST_FIRST, LIST_END, LIST_NTH, WIDGET_PLACE];
    for off in (0..0x150).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let mut v: Vec<u32> = (0..arity(f)).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == TRANSITION_START {
                v = vec![c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits(), c.gpr(4) as u32];
                let at = c.gpr(5) as u32;
                v.extend((0..3).map(|k| mem.read_u32(at + 4 * k)));
                v.push(c.gpr(6) as u32);
            }
            trim(f, &mut v);
            let n = by_value(f);
            if n != 0 {
                let at = v[1];
                v.truncate(1);
                v.extend((0..n as u32).map(|k| mem.read_u32(at + 4 * k)));
            }
            if f == V_BASE + 0x130 + 4 {
                let w = getter_words(&a2);
                let at = c.gpr(5) as u32;
                for (k, x) in w.iter().enumerate() {
                    mem.write_u32(at + 4 * k as u32, *x);
                }
                v.truncate(1);
            }
            a2.borrow_mut().log.push((f, v));
            let ans = a2.borrow_mut().answer(f);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x7e3d_57a7_e000_0001);
    let mut counts = [0u32; 15];
    for case in 0..30_000u32 {
        for k in 0..0x7000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        // gApp and the front end, the renderer word
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x734, FE);
        r.mem.write_u32(APP + 0x72c, g.next());
        r.mem.write_u32(FE + 0x50, if g.below(2) == 0 { 0 } else { OVERLAY });
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        // the vtable every object points at (+0xc; the scene's at +0), the adjusts 0
        for off in (0..0x150).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        for o in OBJECTS {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        // the screen
        let s = SCREEN;
        r.mem.write_u32(s + 0x10, g.below(6));
        for o in [0x18u32, 0x1c, 0x28] {
            r.mem.write_u32(s + o, g.below(2));
        }
        r.mem.write_u32(s + 0x20, g.below(4));
        r.mem.write_u32(s + 0x24, g.below(4));
        r.mem.write_u32(s + 0x14, if g.below(2) == 0 { 0 } else { NEXT });
        r.mem.write_u32(s + 0x30, if g.below(3) == 0 { 0 } else { MENU });
        let group_may_miss = g.below(5) == 0;
        for (o, w) in [(0x38u32, TITLE), (0x40, HELP), (0x44, BACKDROP)] {
            r.mem.write_u32(s + o, if g.below(3) == 0 { 0 } else { w });
            r.mem.write_u32(w + 0x10, g.below(2));
        }
        for p in [P0, P1] {
            r.mem.write_u32(p, g.below(2));
            r.mem.write_u32(p + 0x3c, g.below(2));
        }
        // vf22's two screens
        for x in [SCREEN, NEXT] {
            r.mem.write_u32(x + 0xa8, g.below(2));
            r.mem.write_u32(x + 0x20, [5u32, 0, 1][g.below(3) as usize]);
        }
        let which = g.below(15);
        // (only vf12 checks for a widget group; the others call through it)
        r.mem.write_u32(s + 0x34, if group_may_miss && which == 12 { 0 } else { GROUP });
        counts[which as usize] += 1;
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        // the layout's title text and the widget list: the nodes' next pointers end the walk, the title's halves
        r.mem.write_u32(s + 0x3c, if g.below(3) == 0 { 0 } else { TITLE });
        r.mem.write_u32(TITLE + 8, [0, HELP][g.below(2) as usize]);
        r.mem.write_u32(HELP + 8, [0, BACKDROP][g.below(2) as usize]);
        r.mem.write_u32(BACKDROP + 8, 0);
        r.mem.write_u32(TITLE + 0x144, g.below(2));
        r.mem.write_u32(TITLE + 0x148, HELP);
        r.mem.write_u32(TITLE + 0x14c, BACKDROP);
        r.mem.write_f32(TITLE + 0x170, g.below(500) as f32);
        r.mem.write_f32(TITLE + 0x80, g.below(300) as f32 - 50.0);
        let mut m = snap(&mut r);
        let addr = [0x2747b0u32, 0x274878, 0x2749f0, 0x274b48, 0x274be0, 0x274c78, 0x274df0, 0x274ec0, 0x275888, 0x2758c0, 0x2759a0, 0x2764a8, 0x2717e8, 0x275148, 0x274f18][which as usize];
        let (a1, a2) = if g.below(4) == 0 { (0, NEXT) } else { (SCREEN, if g.below(3) == 0 { 0 } else { NEXT }) };
        r.cpu.set_gpr(4, SCREEN as u64);
        if which == 9 {
            r.cpu.set_gpr(5, a1 as u64);
            r.cpu.set_gpr(6, a2 as u64);
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        match which {
            0 => hide(&mut m, e, s),
            1 => transition_in(&mut m, e, s, BUF),
            2 => transition_out(&mut m, e, s, BUF),
            3 => finish(&mut m, e, s),
            4 => vf8(&mut m, e, s),
            5 => update_transition(&mut m, e, s),
            6 => activate(&mut m, e, s),
            7 => deactivate(&mut m, e, s),
            8 => vf21(&mut m, e, s),
            9 => vf22(&mut m, e, a1, a2),
            10 => vf26(&mut m, e, s),
            11 => vf6(&mut m, s),
            12 => vf12(&mut m, e, s),
            13 => update(&mut m, e, s, BUF),
            _ => layout_widgets(&mut m, e, s, BUF),
        }
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x30).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}

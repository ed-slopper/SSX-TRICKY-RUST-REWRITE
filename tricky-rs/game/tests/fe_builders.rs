//! `tricky_game::fe_builders` (board row G9b2b) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random screens (their menu with its arrows and items, title, prompt and help bar; the help bar's
//! button bits), the three builders run on both sides with the same answers from the calls out, the calls logged in
//! order (vectors by value; the stack's scratch, ours `BUF`, logged as one marker) and the memory compared. The help
//! bar's icons and buttons get the game's own vtables, so the functions behind the slots it calls are stubbed by their
//! addresses read from those vtables.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{ALLOC, FILL, GAPP, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use tricky_game::fe_builders::*;
use tricky_game::fe_charselect::SPRITE_RECT;
use tricky_game::fe_customboard::HELP_LAYOUT_SCALED;
use tricky_game::fe_title::TEXT_CTOR;
use tricky_game::fe_trickbook::LIST_CTOR;
use tricky_game::fe_username::{ICON_COLOUR, ICON_FRAMES};
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
const SCREEN: u32 = ARENA + 0x2000;
const MENU: u32 = ARENA + 0x2400;
const GROUP: u32 = ARENA + 0x2800;
const RECT: u32 = ARENA + 0x4800;
const MADE: [u32; 6] = [ARENA + 0x5000, ARENA + 0x5200, ARENA + 0x5400, ARENA + 0x5600, ARENA + 0x5800, ARENA + 0x5a00];
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const HEAP: u32 = ARENA + 0x8000;
const V_BASE: u32 = 0x7f_0000;
const SCRATCH: u32 = 0x5c5c_5c5c;

type Real = Rc<Vec<(u32, usize, usize)>>;
const REAL_SLOTS: [(u32, u32, usize, usize); 3] =
    [(ICON_VTABLE, 0x138, 2, 4), (HELP_BUTTON_VTABLE, 0x50, 2, 4), (ICON_VTABLE, 0x50, 2, 4)];

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
            TEXT_CTOR | LIST_CTOR => MADE[n as usize % MADE.len()],
            LIST_NTH => [0, MADE[g.below(6) as usize]][g.below(4).min(1) as usize],
            ALLOC => {
                if g.below(16) == 0 {
                    0
                } else {
                    HEAP + 0x400 * (n % 64)
                }
            }
            SPRITE_RECT => RECT,
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
        LIST_CTOR => 6,
        ICON_FRAMES => 5,
        TEXT_CTOR => 4,
        ALLOC | FILL | ICON_COLOUR | SET_BUTTON => 3,
        t if t == V_BASE + 0x128 + 4 => 3,
        WIDGET_PLACE | LIST_NTH | SPRITE_RECT | WIDGETS_RESET | HELP_LAYOUT_SCALED => 2,
        t if [8u32, 0x20, 0x28, 0x50, 0x70, 0x130, 0x138, 0x148, 0xf8].iter().any(|o| t == V_BASE + o + 4) => 2,
        _ => 1,
    };
    let by = if target == V_BASE + 0x70 + 4 {
        3
    } else if [0x50u32, 0x138].iter().any(|o| target == V_BASE + o + 4) || target == WIDGET_PLACE {
        4
    } else {
        0
    };
    (n, by)
}

fn getter_words(a: &A, salt: u64) -> [u32; 4] {
    let mut g = Rng(a.borrow().seed ^ 0x5151 ^ salt ^ a.borrow().log.len() as u64);
    [0, 1, 2, 3].map(|_| (g.below(600) as f32 - 100.0).to_bits())
}

/// The game's stack (the runner's grows down from 0x1ff0000) or our scratch
fn is_scratch(v: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&v) || (BUF..BUF + 0x300).contains(&v)
}

fn log_call(a: &A, real: &Real, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    // +0x20 takes nothing on a screen
    if target == V_BASE + 0x20 + 4 && v[0] == SCREEN {
        v.truncate(1);
    }
    let (_, by) = arity(real, target);
    if by != 0 && v.len() > 1 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..by as u32).map(|k| read(at + 4 * k)));
    }
    if target == ICON_COLOUR || target == ICON_FRAMES {
        let at = v[2];
        let mut w: Vec<u32> = v[..2].to_vec();
        w.extend((0..4).map(|k| read(at + 4 * k)));
        w.extend_from_slice(&v[3..]);
        v = w;
    }
    if target == V_BASE + 0x130 + 4 && v.len() > 1 {
        let at = v[1];
        for (k, x) in getter_words(a, 0).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    if target == V_BASE + 0x128 + 4 {
        let (p, q) = (v[1], v[2]);
        for (k, x) in getter_words(a, 0).iter().enumerate() {
            write(p + 4 * k as u32, *x);
        }
        for (k, x) in getter_words(a, 7).iter().enumerate() {
            write(q + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    for x in v.iter_mut() {
        if is_scratch(*x) {
            *x = SCRATCH;
        }
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

const REGIONS: [(u32, u32); 6] = [(ARENA, 0x8000), (HEAP, 0x10000), (0x337c40, 0x40), (0x3a9e00, 0xe400), (0x347f00, 0x100), (0, 0x600)];

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
fn shared_builders_match_the_original() {
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
        ALLOC, FILL, TEXT_CTOR, WIDGET_PLACE, LIST_NTH, UPDATE_COLOR, SPRITE_RECT, WIDGETS_RESET, ICON_COLOUR,
        ICON_FRAMES, LIST_CTOR, SET_BUTTON, HELP_LAYOUT_SCALED,
    ];
    leaves.extend(real.iter().map(|r| r.0));
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    leaves.sort_unstable();
    leaves.dedup();
    // the list's sentinel test runs as the original
    r.allow(0x2abf10);
    for f in leaves {
        let a2 = a.clone();
        let real2 = real.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let (n, _) = arity(&real2, f);
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == HELP_LAYOUT_SCALED {
                v = vec![c.gpr(4) as u32, c.f(12).to_bits()];
            }
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &real2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x2742_0000_0000_0001);
    let methods: &[u32] = &[0x274108, 0x2742e8, 0x2719c0];
    let mut counts = vec![0u32; methods.len()];
    for case in 0..20_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        for k in 0..0x10000 / 4 {
            r.mem.write_u32(HEAP + 4 * k, 0x5a5a_5a5a ^ k);
        }
        r.mem.write(0, &[0u8; 0x600]).unwrap();
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x60, g.next());
        for off in (0..0x170).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let mut objects = vec![SCREEN, MENU, GROUP];
        objects.extend(MADE);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
            for off in [0x80u32, 0x84, 0x88, 0x170, 0x174, 0x178] {
                r.mem.write_f32(o + off, g.below(300) as f32 - 50.0);
            }
        }
        // the menu's items: a chain of the made widgets ending at a node pointing at itself
        let n_items = g.below(5);
        for k in 0..6u32 {
            let x = MADE[k as usize];
            let next = if k + 1 < n_items { MADE[k as usize + 1] } else { MADE[5] };
            r.mem.write_u32(x + 8, next);
            r.mem.write_u32(x + 4, if k == 0 { MENU } else { MADE[k as usize - 1] });
            r.mem.write_u32(x + 0x20, [0u32, 0, 5][g.below(3) as usize]);
        }
        r.mem.write_u32(MADE[5] + 8, MADE[5]);
        r.mem.write_f32(RECT + 8, g.below(40) as f32);
        let s = SCREEN;
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, [0, MENU, MENU][g.below(3) as usize]);
        for o in [0x38u32, 0x40, 0x44] {
            r.mem.write_u32(s + o, [0, MADE[g.below(6) as usize]][g.below(3).min(1) as usize]);
        }
        r.mem.write_u32(MENU + 0x144, g.below(2));
        r.mem.write_u32(MENU + 0x148, [0, MADE[1]][g.below(3).min(1) as usize]);
        r.mem.write_u32(MENU + 0x14c, MADE[2]);
        r.mem.write_u32(MENU + 0x150, [0u32, 1, 1, 2][g.below(4) as usize]);
        let which = g.below(methods.len() as u32) as usize;
        let addr = methods[which];
        counts[which] += 1;
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let a1 = match addr {
            0x2719c0 => [0u32, 1, 0xa, 0x800e, 0x1ffff, g.next()][g.below(6) as usize],
            _ => [0, GROUP][g.below(2) as usize],
        };
        let a2 = [0, GROUP][g.below(2) as usize];
        r.cpu.set_gpr(4, s as u64);
        r.cpu.set_gpr(5, a1 as i32 as i64 as u64);
        r.cpu.set_gpr(6, a2 as i32 as i64 as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone(), real.clone());
        match addr {
            0x274108 => build_menu(&mut m, e, s, a1, BUF),
            0x2742e8 => layout(&mut m, e, s, BUF),
            _ => build_help_bar(&mut m, e, s, a1, a2, BUF),
        }
        let mine = std::mem::take(&mut a.borrow_mut().log);
        if theirs != mine {
            let k = (0..theirs.len().min(mine.len())).find(|&k| theirs[k] != mine[k]).unwrap_or(theirs.len().min(mine.len()));
            panic!(
                "case {case}: {addr:#x} calls differ at {k}: theirs {:x?}, mine {:x?} ({} and {} calls; before {:x?})",
                theirs.get(k),
                mine.get(k),
                theirs.len(),
                mine.len(),
                if k > 0 { theirs.get(k - 1) } else { None }
            );
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x300).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by builder: {counts:?}");
}

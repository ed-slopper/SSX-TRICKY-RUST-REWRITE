//! `tricky_game::debugmenu` (board row G9a4a) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000, as the ELF loads it) with a menu
//! manager, four menus and 64 items in the arena: the stack's depth, the menus' counts, selections and first
//! shown, the items' flags and their tables made random, the tables' virtuals pointing at stand-ins. Then one of
//! the row's functions runs on both sides. Every callee is a stand-in on both sides answering from the case's seed
//! (an item's size written through its hidden pointer); the draws log the box they are given. The runner's stack is
//! cleared before each case and a buffer on it logs as one token. The calls, the answers and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::debugmenu as dm;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::TextBoxEnv;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x328000, 0x18000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const MGR: u32 = ARENA;
const MENUS: [u32; 4] = [ARENA + 0x100, ARENA + 0x300, ARENA + 0x500, ARENA + 0x700];
const ITEMS: u32 = ARENA + 0x1000;
const ITEM_TABLES: u32 = ARENA + 0x2000;
const MENU_TABLES: u32 = ARENA + 0x2400;
const APPOBJ: u32 = ARENA + 0x3000;
const DEVICE: u32 = ARENA + 0x3100;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// The items' and the menus' virtuals' stand-ins
fn iv(off: u32) -> u32 {
    0x0180_0000 + off
}
fn mv(off: u32) -> u32 {
    0x0181_0000 + off
}
const IOFFS: [u32; 6] = [0x10, 0x18, 0x20, 0x28, 0x30, 0x38];
const MOFFS: [u32; 4] = [0x10, 0x28, 0x30, 0x38];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w32(&mut self, a: u32, v: u32);
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn w32(&mut self, a: u32, v: u32) {
        self.write(a, &v.to_le_bytes()).unwrap()
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn stack(a: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&a)
}
fn token(a: u32) -> u32 {
    if stack(a) { STACK_TOKEN } else { a }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

/// How many arguments a callee has (`this` counted for a virtual)
fn arity(func: u32) -> usize {
    match func {
        f if f == iv(0x20) || f == iv(0x28) || f == iv(0x30) || f == mv(0x30) => 2,
        f if f & 0xfffe_0000 == 0x0180_0000 => 1,
        dm::MEM_DELETE | dm::INPUT_DEVICE | dm::INPUT_PRESSED | dm::UP_ARROW | dm::DOWN_ARROW => 2,
        dm::BEGIN_DRAW | dm::END_DRAW => 1,
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let mut log: Vec<u32> = w[..arity(func)].iter().map(|&a| token(a)).collect();
    // the boxes handed to an item's draw and the arrows
    if func == iv(0x28) || func == dm::UP_ARROW || func == dm::DOWN_ARROW {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    let r = ans.next();
    let answer = match func {
        // (picked: 0 or not)
        f if f == iv(0x18) => [0u32, 1, r][(r % 3) as usize],
        f if f == iv(0x10) => [0u32, 1, 2, 3, 4, r][(r % 6) as usize],
        f if f == mv(0x10) => [0u32, 1, 2, r][(r % 4) as usize],
        // an item's size: its box through the hidden pointer (heights so the list sometimes passes 421)
        f if f == iv(0x20) => {
            // (and the bounds: 421 the list's, 451 the screen's; widths past 640 too)
            let edge = [0u32, 1, 300, 420, 421, 422, 450, 451][(r >> 8) as usize % 8];
            let h = [r % 20, 20 + r % 40, 60 + r % 100, edge][(r >> 20) as usize % 4];
            for (k, v) in [r >> 3, r >> 9, r % 800, h].into_iter().enumerate() {
                io.w32(w[0] + 4 * k as u32, v);
            }
            0
        }
        dm::INPUT_PRESSED => [8u32, 0xd, 0x62, 0x63, 0x64, r][(r % 6) as usize],
        dm::INPUT_DEVICE => DEVICE,
        _ => r,
    };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], _fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, func, &w, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

#[test]
fn debug_menu_framework_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x1889b8u32, 0x1889d8, 0x188a00, 0x188a80, 0x188ac8, 0x188ad0, 0x188ad8, 0x188ae8, 0x188b10, 0x188b20, 0x188b48,
        0x188b68, 0x188b88, 0x188ba8, 0x188bd0, 0x188bf8, 0x188c50, 0x188d20, 0x188df0, 0x188e78, 0x188e80, 0x188e88,
        0x189240, 0x1892e8, 0x189390, 0x1893a0, 0x189450, 0x189478, 0x1894a8, 0x189518, 0x189560, 0x189590, 0x1895b0,
        0x1895c8, 0x1895e0, 0x1895e8, 0x189718, 0x189720, 0x189760, 0x1897a0, 0x1897a8, 0x1897b0, 0x1897f0, 0x189898,
    ];
    for f in funcs {
        r.allow(f);
    }
    let mut stubs = vec![
        dm::MEM_DELETE, dm::INPUT_DEVICE, dm::INPUT_PRESSED, dm::BEGIN_DRAW, dm::END_DRAW, dm::UP_ARROW, dm::DOWN_ARROW,
    ];
    stubs.extend(IOFFS.map(iv));
    stubs.extend(MOFFS.map(mv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (v, call) = standin(mem, func, &w, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0xde6b_0000_0000_0001);
    let (mut drawn, mut scrolled, mut moved) = (0usize, 0usize, 0usize);
    let per = 500u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x800])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (its input +0x24, its mode +0x58)
        m.w32(dm::APP, APPOBJ);
        m.w32(APPOBJ + 0x58, g.pick(&[4u32, 4, 0, 1]));
        // the tables: four of each, the `this` adjustments 0 or 4
        for t in 0..4 {
            for off in IOFFS {
                m.w16(ITEM_TABLES + 0x40 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(ITEM_TABLES + 0x40 * t + off + 4, iv(off));
            }
            for off in MOFFS {
                m.w16(MENU_TABLES + 0x40 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(MENU_TABLES + 0x40 * t + off + 4, mv(off));
            }
        }
        // the items: flags, menu, table
        for i in 0..64 {
            let it = ITEMS + 0x20 * i;
            let any = g.next();
            m.w32(it + 8, g.pick(&[1u32, 1, 0, 9, 8, any]));
            m.w32(it + 4, g.pick(&MENUS[1..]));
            m.w32(it + 0xc, ITEM_TABLES + 0x40 * g.below(4));
        }
        // the menus: manager, count (1.., 64 at most), selection, first shown, items, table
        for menu in MENUS {
            let n = g.pick(&[1u32, 2, 3, 5, 12, 30, 63, 64]);
            // (only the first menu sometimes without its manager: the items' menus have theirs)
            let mgr = if menu == MENUS[0] { g.pick(&[MGR, MGR, 0]) } else { MGR };
            m.w32(menu + 4, mgr);
            m.w32(menu + 8, n);
            let (s1, s2) = (g.below(n), g.below(n));
            // (past the list sometimes, where the slot after it still holds an item)
            m.w32(menu + 0xc, g.pick(&[s1, s2, n.min(63)]));
            m.w32(menu + 0x10, g.below(n));
            for i in 0..64 {
                m.w32(menu + 0x14 + 4 * i, ITEMS + 0x20 * g.below(64));
            }
            m.w32(menu + 0x114, MENU_TABLES + 0x40 * g.below(4));
        }
        // the manager: its stack (1.. for the draw), the key, the pad
        let depth = if matches!(func, 0x1897b0 | 0x189898) { 1 + g.below(4) } else { g.below(5) };
        m.w32(MGR, depth);
        for i in 0..10 {
            m.w32(MGR + 4 + 4 * i, g.pick(&MENUS));
        }
        let any = g.next();
        m.w32(MGR + 0x30, g.pick(&[8u32, 0xd, 0x1b, 0x4800, 0x5000, any]));
        let any = g.next();
        m.w32(MGR + 0x34, g.pick(&[0u32, 1, any]));
        let any = g.next();
        m.w32(DEVICE + 0x2c, g.pick(&[0x1bu32, any]));
        m.w32(MGR + 0x2c, g.below(4));
        let this = match func {
            0x1889b8..=0x188b88 => ITEMS + 0x20 * g.below(64),
            0x188ba8..=0x189518 => MENUS[0],
            _ => MGR,
        };
        let n0 = m.r32(MENUS[0] + 8);
        let a1 = match func {
            // the item added (at most 63 in the menu then), where; the item removed; the selection
            0x189240 => {
                // (an empty menu sometimes)
                if n0 == 64 || g.below(4) == 0 {
                    m.w32(MENUS[0] + 8, if n0 == 64 { 63 } else { 0 });
                }
                ITEMS + 0x20 * g.below(64)
            }
            0x1892e8 => g.below(m.r32(MENUS[0] + 8)),
            0x1893a0 => {
                let (i, j) = (g.below(n0), g.below(n0));
                g.pick(&[0xffff_ffffu32, 0, 1, i, (0u32).wrapping_sub(j + 1)])
            }
            0x1894a8 => MGR,
            0x1897f0 => g.pick(&MENUS),
            0x1889d8 | 0x188bd0 | 0x189590 => g.below(4),
            _ => {
                let any = g.next();
                g.pick(&[0u32, 1, 8, 0x1b, any])
            }
        };
        let k = g.below(m.r32(MENUS[0] + 8) + 1);
        let at = g.pick(&[0xffff_ffffu32, 0, 1, k]);
        if func == 0x1897f0 && depth >= 9 {
            m.w32(MGR, 8);
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let none = u32::MAX;
        let e = &mut env;
        let mm = &mut m;
        let ours = match func {
            0x1889b8 => dm::item_new(mm, this, a1),
            0x1889d8 => {
                dm::item_delete(mm, e, this, a1);
                none
            }
            0x188a00 => dm::item_handle_input(mm, this),
            0x188a80 => dm::item_is_selectable(mm, this),
            0x188ac8 | 0x188ad0 | 0x188e80 => {
                dm::item_nothing();
                none
            }
            0x188ad8 => dm::item_is_shown(mm, this),
            0x188ae8 => {
                dm::item_set_shown(mm, this, a1);
                none
            }
            0x188b10 => dm::item_flag8(mm, this),
            0x188b20 => {
                dm::item_set_flag8(mm, this, a1);
                none
            }
            0x188b48 => dm::item_key(mm, this),
            0x188b68 => dm::item_is_pad_key(mm, e, this, a1),
            0x188b88 => dm::item_buttons(mm, e, this, a1),
            0x188ba8 => dm::menu_new(mm, this),
            0x188bd0 => {
                dm::menu_delete(mm, e, this, a1);
                none
            }
            0x188bf8 => {
                dm::menu_skip_unselectable(mm, e, this);
                none
            }
            0x188c50 => {
                dm::menu_select_next(mm, e, this);
                none
            }
            0x188d20 => {
                dm::menu_select_prev(mm, e, this);
                none
            }
            0x188df0 => dm::menu_handle_input(mm, e, this),
            0x188e78 => dm::menu_vf2(),
            0x188e88 => {
                dm::menu_draw(mm, e, this, SCRATCH);
                none
            }
            0x189240 => {
                dm::menu_add_item(mm, this, a1, at as i32);
                none
            }
            0x1892e8 => {
                dm::menu_remove_item(mm, e, this, a1 as i32);
                none
            }
            0x189390 => {
                dm::menu_clear(mm, this);
                none
            }
            0x1893a0 => {
                dm::menu_set_selection(mm, e, this, a1 as i32);
                none
            }
            0x189450 => dm::menu_selected_index(mm, e, this),
            0x189478 => dm::menu_current_item(mm, e, this),
            0x1894a8 => {
                dm::menu_enter(mm, e, this, a1);
                none
            }
            0x189518 => {
                dm::menu_leave(mm, e, this);
                none
            }
            0x189560 => dm::manager_new(mm, this),
            0x189590 => {
                dm::manager_delete(mm, e, this, a1);
                none
            }
            0x1895b0 => {
                dm::manager_reset(mm, this);
                none
            }
            0x1895c8 => dm::manager_is_active(mm, this),
            0x1895e0 => {
                dm::manager_set_active(mm, this, a1);
                none
            }
            0x1895e8 => {
                dm::manager_update(mm, e, this);
                none
            }
            0x189718 => dm::manager_key(mm, this),
            0x189720 => dm::manager_is_pad_key(mm, e, this, a1),
            0x189760 => dm::manager_buttons(mm, e, this, a1),
            0x1897a0 => {
                dm::manager_set_pad(mm, this, a1);
                none
            }
            0x1897a8 => dm::manager_pad(mm, this),
            0x1897b0 => {
                dm::manager_draw(mm, e, this);
                none
            }
            0x1897f0 => {
                dm::manager_push(mm, e, this, a1);
                none
            }
            0x189898 => {
                dm::manager_pop(mm, e, this);
                none
            }
            _ => unreachable!(),
        };
        if func == 0x188e88 {
            drawn += env.log.iter().filter(|c| c.0 == iv(0x28)).count();
            scrolled += env.log.iter().filter(|c| c.0 == dm::UP_ARROW).count();
        }
        if matches!(func, 0x188c50 | 0x188d20) {
            moved += env.log.iter().filter(|c| c.0 == iv(0x30)).count();
        }
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x189240 => vec![this, a1, at],
            _ => vec![this, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{drawn} items drawn, {scrolled} lists scrolled, {moved} selections moved");
    assert!(drawn > 1000 && scrolled > 30 && moved > 100);
}

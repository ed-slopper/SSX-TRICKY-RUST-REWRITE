//! `tricky_game::dmitems` (board row G9a4b) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000, as the ELF loads it) with an item of
//! the kind under test in the arena (its fields made random as that kind reads them: the value it edits, an enum's
//! list, a string's buffer, an expand item's menu of items), its outer menu and manager, and a pool of other items;
//! the items' and menus' tables point at stand-ins. Then one of the row's functions runs on both sides. Every callee
//! is a stand-in on both sides answering from the case's seed (a size written through its pointer, `strlen` done for
//! real); the draws log the box they are given. The runner's stack is cleared before each case and a buffer on it
//! logs as one token. The calls, the answers and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::debugmenu as dm;
use tricky_game::dmitems as di;
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
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x328000, 0x18000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x5000;
const THIS: u32 = ARENA;
const OUTER: u32 = ARENA + 0x1000;
/// a menu a sub item opens
const SUBMENU: u32 = ARENA + 0x1400;
const MGR: u32 = ARENA + 0x1200;
const ITEMS: u32 = ARENA + 0x2000;
const ITEM_TABLES: u32 = ARENA + 0x2800;
const MENU_TABLES: u32 = ARENA + 0x2a00;
const APPOBJ: u32 = ARENA + 0x2c00;
const DEVICE: u32 = ARENA + 0x2d00;
const VALUE: u32 = ARENA + 0x3000;
const LIST: u32 = ARENA + 0x3100;
const BUF: u32 = ARENA + 0x3200;
const COLOUR: u32 = ARENA + 0x3300;
const KEPT: u32 = ARENA + 0x3340;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn iv(off: u32) -> u32 {
    0x0180_0000 + off
}
fn mv(off: u32) -> u32 {
    0x0181_0000 + off
}
const IOFFS: [u32; 8] = [0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48];
const MOFFS: [u32; 6] = [0x10, 0x18, 0x20, 0x28, 0x30, 0x38];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w32(&mut self, a: u32, v: u32);
    fn r32(&mut self, a: u32) -> u32;
    fn r8(&mut self, a: u32) -> u8;
}
impl Io for Mem {
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
}
impl Io for r5900::Mem {
    fn w32(&mut self, a: u32, v: u32) {
        self.write(a, &v.to_le_bytes()).unwrap()
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8];
        self.read(a, &mut b).unwrap();
        b[0]
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

/// How many int and float arguments a callee has (`this` counted for a virtual)
fn arity(func: u32) -> (usize, usize) {
    match func {
        f if f == iv(0x40) || f == mv(0x18) => (3, 0),
        f if f == iv(0x20) || f == iv(0x28) || f == iv(0x30) || f == iv(0x48) || f == mv(0x20) || f == mv(0x30) => (2, 0),
        f if f & 0xfffe_0000 == 0x0180_0000 => (1, 0),
        di::MEASURE_LABEL | di::DRAW_LABEL => (5, 0),
        di::SPRINTF => (3, 0),
        di::STRLEN => (1, 0),
        di::TO_DOUBLE => (0, 1),
        dm::INPUT_DEVICE => (2, 0),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    // the boxes handed to the draws
    if func == iv(0x28) || func == di::DRAW_LABEL {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    let r = ans.next();
    let answer = match func {
        f if f == iv(0x18) => [0u32, 1, r][(r % 3) as usize],
        f if f == iv(0x10) => [0u32, 1, 2, 3, 4, r][(r % 6) as usize],
        // a size: its box through the pointer
        f if f == iv(0x20) || f == di::MEASURE_LABEL => {
            // (widths often equal: which of two the wider is)
            let wd = if (r >> 24) % 2 == 0 { 100 } else { r % 500 };
            for (k, v) in [r >> 3, r >> 9, wd, r % 90].into_iter().enumerate() {
                io.w32(w[0] + 4 * k as u32, v);
            }
            0
        }
        di::STRLEN => {
            let mut n = 0;
            while n < 0x40 && io.r8(w[0] + n) != 0 {
                n += 1;
            }
            n
        }
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
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(8, 0);
        let (a, c) = standin(m, func, &w, &f, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

const FUNCS: [u32; 79] = [
    0x189948, 0x1899c0, 0x189a90, 0x189b98, 0x189c38, 0x189c80, 0x189cc0, 0x189d38, 0x189d78, 0x18a000, 0x18a118,
    0x18a1d0, 0x18a228, 0x18a298, 0x18a2d0, 0x18a320, 0x18a368, 0x18a3d8, 0x18a410, 0x18a460, 0x18a498, 0x18a4e0,
    0x18a538, 0x18a548, 0x18a5d0, 0x18a628, 0x18a6b8, 0x18a700, 0x18a708, 0x18a740, 0x18a790, 0x18a7d8, 0x18a7e0,
    0x18a818, 0x18a820, 0x18a878, 0x18a900, 0x18a940, 0x18aab0, 0x18aae8, 0x18ab68, 0x18abf8, 0x18ada8, 0x18ade0,
    0x18ae80, 0x18af10, 0x18b0c0, 0x18b0f8, 0x18b198, 0x18b1d0, 0x18b240, 0x18b258, 0x18b660, 0x18b6b8, 0x18b810,
    0x18b840, 0x18b870, 0x18b8f8, 0x18b9a0, 0x18b9a8, 0x18b9b0, 0x18b9f8, 0x18ba68, 0x18bb58, 0x18bb88, 0x18bbb8,
    0x18bd78, 0x18beb0, 0x18bf28, 0x18bf50, 0x18bfa8, 0x18bfb0, 0x18c0a8, 0x18c118, 0x18c1c8, 0x18af10, 0x18abf8,
    0x18b258, 0x189d78,
];

#[test]
fn debug_menu_items_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    // (the framework's, which the items call)
    for f in [
        0x1889b8u32, 0x188a00, 0x188a80, 0x188ad8, 0x188b10, 0x188b48, 0x188b88, 0x188ba8, 0x188bf8, 0x188c50, 0x188d20,
        0x189240, 0x1892e8, 0x189390, 0x1893a0, 0x189450, 0x189478, 0x1894a8, 0x189518, 0x1895e0, 0x189718, 0x189760,
        0x1897f0, 0x189898, 0x188ac8, 0x188ad0,
    ] {
        r.allow(f);
    }
    let mut stubs = vec![di::MEASURE_LABEL, di::DRAW_LABEL, di::SPRINTF, di::STRLEN, di::TO_DOUBLE, dm::INPUT_DEVICE];
    stubs.extend(IOFFS.map(iv));
    stubs.extend(MOFFS.map(mv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, func, &w, &f, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0xd17e_0000_0000_0001);
    let (mut edits, mut steps, mut expanded) = (0usize, 0usize, 0usize);
    let per = 300u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x800])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(dm::APP, APPOBJ);
        let any = g.next();
        m.w32(DEVICE + 0x10, g.pick(&[0u32, 4, 8, 12, any]));
        for t in 0..4 {
            for off in IOFFS {
                m.w16(ITEM_TABLES + 0x60 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(ITEM_TABLES + 0x60 * t + off + 4, iv(off));
            }
            for off in MOFFS {
                m.w16(MENU_TABLES + 0x60 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(MENU_TABLES + 0x60 * t + off + 4, mv(off));
            }
        }
        // the pool of items: flags, menu (the outer one), table
        for i in 0..64 {
            let it = ITEMS + 0x20 * i;
            let any = g.next();
            m.w32(it + 8, g.pick(&[1u32, 1, 0, 9, any]));
            m.w32(it + 4, OUTER);
            m.w32(it + 0xc, ITEM_TABLES + 0x60 * g.below(4));
        }
        // the outer menu (its manager, items: the item under test sometimes picked) and the manager
        let n = g.pick(&[1u32, 2, 5, 12]);
        m.w32(OUTER + 4, MGR);
        m.w32(OUTER + 8, n);
        m.w32(OUTER + 0xc, g.below(n));
        for i in 0..64 {
            m.w32(OUTER + 0x14 + 4 * i, ITEMS + 0x20 * g.below(64));
        }
        if g.below(2) == 0 {
            let at = m.r32(OUTER + 0xc);
            m.w32(OUTER + 0x14 + 4 * at, THIS);
        }
        m.w32(OUTER + 0x114, MENU_TABLES + 0x60 * g.below(4));
        m.w32(SUBMENU + 0x114, MENU_TABLES + 0x60 * g.below(4));
        let depth = 1 + g.below(4);
        m.w32(MGR, depth);
        for i in 0..8 {
            m.w32(MGR + 4 + 4 * i, OUTER);
        }
        m.w32(MGR + 0x2c, g.below(4));
        let any = g.next();
        m.w32(MGR + 0x30, g.pick(&[0xdu32, 0xd, 0x4b00, 0x4d00, 0x4800, 0x5000, 0x61, 0x63, 0x41, 8, 0x1b, any]));
        // the item: its menu, flags and table; the kinds' fields
        m.w32(THIS + 4, OUTER);
        let any = g.next();
        m.w32(THIS + 8, g.pick(&[1u32, 1, 1, 9, 0, any]));
        m.w32(THIS + 0xc, ITEM_TABLES + 0x60 * g.below(4));
        let (title, word) = (g.next(), g.next());
        m.w32(THIS + 0x10, g.pick(&[title, title, 0]));
        let fam = func;
        let enum_n = g.pick(&[0u32, 1, 3, 8]);
        for i in 0..9 {
            m.w32(LIST + 8 * i, if i < enum_n { g.next() | 1 } else { 0 });
            m.w32(LIST + 8 * i + 4, g.below(10));
        }
        m.w32(VALUE, g.pick(&[0u32, 1, 3, 5, 9, 2]));
        match fam {
            // bool: the words, the bool
            0x1899c0 | 0x189a90 | 0x189b98 => {
                m.w32(THIS + 0x14, g.pick(&[word, word, 0]));
                m.w32(THIS + 0x18, g.pick(&[word ^ 1, word ^ 1, 0]));
                m.w32(THIS + 0x1c, VALUE);
            }
            // enum: count, list, value
            0x189d78 | 0x18a000 | 0x18a118 => {
                m.w32(THIS + 0x14, enum_n);
                m.w32(THIS + 0x18, LIST);
                m.w32(THIS + 0x1c, VALUE);
                if g.below(2) == 0 {
                    m.w32(VALUE, m.r32(LIST + 4));
                }
            }
            // sub: the menu it opens
            0x18a228 => m.w32(THIS + 0x14, SUBMENU),
            // command: a value shown, the argument
            0x18a548 | 0x18a5d0 | 0x18a628 => m.w32(THIS + 0x14, g.pick(&[word, 0])),
            0x18a7e0 => {}
            // int: the int, its range, held, the way, the fraction, a kept int
            0x18a900 | 0x18a940 | 0x18aae8 => {
                m.w32(THIS + 0x14, g.pick(&[VALUE, VALUE, 0]));
                let lo = g.below(20) as i32 - 10;
                m.w32(THIS + 0x18, lo as u32);
                m.w32(THIS + 0x1c, (lo + g.below(400) as i32) as u32);
                m.w32(VALUE, (lo + g.below(500) as i32 - 50) as u32);
                m.w32(THIS + 0x20, g.pick(&[0u32, 10, 100, 300, 1000]));
                m.w32(THIS + 0x24, g.pick(&[0u32, 1, 0xffff_ffff]));
                wf(&mut m, THIS + 0x28, g.f(0.0, 1.5));
                m.w32(THIS + 0x2c, g.pick(&[0u32, KEPT]));
                // (no int only for the range: the others read it)
                if func != 0x18a900 {
                    m.w32(THIS + 0x14, VALUE);
                }
            }
            // float, angle: the float, its range, scale, offset, held, the way, a kept float
            0x18abf8 | 0x18ada8 | 0x18ade0 | 0x18af10 | 0x18b0c0 | 0x18b0f8 => {
                m.w32(THIS + 0x14, VALUE);
                let lo = g.f(-10.0, 10.0);
                wf(&mut m, THIS + 0x18, lo);
                wf(&mut m, THIS + 0x1c, lo + g.f(0.0, 100.0));
                wf(&mut m, VALUE, lo + g.f(-5.0, 105.0));
                wf(&mut m, THIS + 0x20, g.f(0.5, 2.0));
                wf(&mut m, THIS + 0x24, g.f(-1.0, 1.0));
                let held = g.pick(&[0.0f32, 3.0, 40.0, 200.0, 1000.0]);
                wf(&mut m, THIS + 0x28, held);
                let way = g.pick(&[0.0f32, 1.0, -1.0]);
                wf(&mut m, THIS + 0x2c, way);
                m.w32(THIS + 0x30, g.pick(&[0u32, KEPT]));
            }
            // string: a name in the buffer, its size, the cursor
            0x18b258 => {
                let len = g.pick(&[0u32, 1, 4, 10, 30]);
                let chars = b"azAZ09_m";
                for i in 0..0x40 {
                    // (past the end junk: the stores of the end show)
                    m.w8(BUF + i, if i < len { chars[g.below(8) as usize] } else if i == len { 0 } else { 0x7e });
                }
                m.w32(THIS + 0x14, BUF);
                m.w32(THIS + 0x18, g.pick(&[len + 1, len + 2, 0x20, 0x40]));
                m.w32(THIS + 0x1c, g.below(len + 1));
            }
            // colour: the colour, the channel, held, the way
            0x18b6b8 => {
                m.w32(THIS + 0x10, COLOUR);
                m.w32(THIS + 0x14, g.below(3));
                for k in 0..4 {
                    wf(&mut m, COLOUR + 4 * k, g.pick(&[0.0f32, 1.0, 0.5, 0.9999, 0.0001]));
                }
                m.w32(THIS + 0x18, g.pick(&[0u32, 10, 100, 300]));
                m.w32(THIS + 0x1c, g.pick(&[0u32, 1, 0xffff_ffff]));
            }
            _ => {}
        }
        // an expand item: its header (in the pool), its menu (+0x14: manager, count, selection, items; the owner)
        if (0x18b810..=0x18bf28).contains(&func) || func == 0x18c0a8 {
            let header = ITEMS + 0x20 * g.below(64);
            m.w32(THIS + 0x10, g.pick(&[header, 0]));
            let en = g.pick(&[1u32, 2, 4, 8]);
            m.w32(THIS + 0x18, MGR);
            m.w32(THIS + 0x1c, en);
            m.w32(THIS + 0x20, g.below(en));
            for i in 0..16 {
                m.w32(THIS + 0x28 + 4 * i, ITEMS + 0x20 * g.below(64));
            }
            if g.below(2) == 0 {
                m.w32(THIS + 0x28 + 4 * g.below(en), header);
            }
            m.w32(THIS + 0x128, MENU_TABLES + 0x60 * g.below(4));
            m.w32(THIS + 0x12c, THIS);
            m.w32(THIS + 0x530, COLOUR);
            m.w32(THIS + 0x5ac, g.pick(&[0u32, KEPT]));
        }
        // the arguments
        let (a1, a2, a3, a4, a5) = (g.next(), g.next(), g.next(), g.next(), g.next());
        let item = ITEMS + 0x20 * g.below(64);
        let at = g.pick(&[0xffff_ffffu32, 0, 1, 3]);
        let (fmin, fmax) = (g.f(-10.0, 0.0), g.f(0.0, 10.0));
        let regs: Vec<u32> = match func {
            0x189948 => vec![THIS, a1, VALUE, a3, a4, a5],
            0x189cc0 => vec![THIS, a1, VALUE, a3, enum_n, LIST],
            0x189d38 => vec![THIS, LIST],
            0x18a1d0 => vec![THIS, OUTER, a2],
            0x18a4e0 | 0x18a538 => vec![THIS, a1, a2, a3],
            0x18a790 => vec![THIS, a1 % 50],
            0x18a878 => vec![THIS, a1, VALUE, (a3 % 20).wrapping_sub(10), a4 % 300, a5],
            0x18a900 => vec![THIS, (a1 % 20).wrapping_sub(10), a2 % 300],
            0x18ab68 | 0x18ae80 => vec![THIS, a1, VALUE, a3],
            0x18b1d0 => vec![THIS, a1, a2, BUF, a4 % 0x40],
            0x18b240 => vec![THIS, a1, BUF, a3 % 0x40],
            0x18b660 => vec![THIS, COLOUR, a2 % 3],
            0x18b810 => vec![THIS + 0x14, item, a2],
            0x18b840 => vec![THIS + 0x14, item],
            0x18b8f8 => vec![THIS, a1, g.pick(&[item, 0])],
            0x18b9a0 => {
                if m.r32(THIS + 0x1c) == 64 {
                    m.w32(THIS + 0x1c, 63);
                }
                vec![THIS, item, at]
            }
            0x18b9a8 => vec![THIS, g.below(m.r32(THIS + 0x1c))],
            0x18b9f8 => vec![THIS, item],
            0x18bb58 => vec![THIS, item, a2],
            0x18bb88 => vec![THIS, item],
            0x18beb0 => vec![THIS, g.pick(&[0u32, 1, 2])],
            0x18bf50 => vec![THIS, a1, COLOUR],
            0x18bfb0 => vec![THIS, a1, a2, COLOUR],
            0x18c1c8 => vec![THIS, a1, a2],
            0x189a90 | 0x18a000 | 0x18a298 | 0x18a3d8 | 0x18a5d0 | 0x18a708 | 0x18a7e0 | 0x18aab0 | 0x18ada8 | 0x18b0c0
            | 0x18bbb8 => vec![SCRATCH + 0x700, THIS],
            0x189b98 | 0x18a118 | 0x18a2d0 | 0x18a410 | 0x18a628 | 0x18a740 | 0x18aae8 | 0x18ade0 | 0x18b0f8 | 0x18bd78 => {
                vec![THIS, SCRATCH + 0x700]
            }
            _ => vec![THIS, a1],
        };
        // (a box for the measures and draws: on the scratch here, and the same bytes on the runner's side)
        let boxw: Vec<u32> = (0..4).map(|_| g.below(300)).collect();
        for (k, v) in boxw.iter().enumerate() {
            m.w32(SCRATCH + 0x700 + 4 * k as u32, *v);
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
        let sc = SCRATCH;
        let w = |i: usize| regs.get(i).copied().unwrap_or(0);
        let ours = match func {
            0x189948 => di::bool_new(mm, THIS, w(1), w(2), w(3), w(4), w(5)),
            0x1899c0 => di::bool_handle_input(mm, e, THIS),
            0x189a90 => di::bool_measure(mm, e, w(0), THIS, sc),
            0x189b98 => { di::bool_draw(mm, e, THIS, w(1), sc); none }
            0x189c38 => di::enum_new0(mm, THIS),
            0x189c80 => di::enum_new1(mm, THIS, w(1)),
            0x189cc0 => di::enum_new(mm, THIS, w(1), w(2), w(3), w(4), w(5)),
            0x189d38 => { di::enum_set_list(mm, THIS, w(1)); none }
            0x189d78 => di::enum_handle_input(mm, e, THIS),
            0x18a000 => di::enum_measure(mm, e, w(0), THIS, sc),
            0x18a118 => { di::enum_draw(mm, e, THIS, w(1), sc); none }
            0x18a1d0 => di::sub_new(mm, THIS, w(1), w(2)),
            0x18a228 => di::sub_handle_input(mm, e, THIS),
            0x18a298 | 0x18a3d8 | 0x18a708 => di::label_measure(mm, e, w(0), THIS),
            0x18a2d0 | 0x18a410 | 0x18a740 => { di::label_draw(mm, e, THIS, w(1), sc); none }
            0x18a320 => di::pop_new(mm, THIS, w(1)),
            0x18a368 => di::pop_handle_input(mm, e, THIS),
            0x18a460 => di::command_new0(mm, THIS),
            0x18a498 => di::command_new1(mm, THIS, w(1)),
            0x18a4e0 => di::command_new(mm, THIS, w(1), w(2), w(3)),
            0x18a538 => { di::command_set(mm, THIS, w(1), w(2)); none }
            0x18a548 => di::command_handle_input(mm, e, THIS),
            0x18a5d0 => di::command_measure(mm, e, w(0), THIS),
            0x18a628 => { di::command_draw(mm, e, THIS, w(1), sc); none }
            0x18a6b8 => di::null_new(mm, THIS, w(1)),
            0x18a700 | 0x18a7d8 | 0x18bfa8 => di::never_selectable(),
            0x18a790 => di::space_new(mm, THIS, w(1)),
            0x18a7e0 => di::space_measure(mm, w(0), THIS),
            0x18a818 => { di::space_draw(); none }
            0x18a820 => di::int_new1(mm, THIS, w(1)),
            0x18a878 => di::int_new(mm, THIS, w(1), w(2), w(3), w(4), w(5)),
            0x18a900 => { di::int_set_range(mm, THIS, w(1), w(2)); none }
            0x18a940 => di::int_handle_input(mm, e, THIS),
            0x18aab0 => di::int_measure(mm, e, w(0), THIS),
            0x18aae8 => { di::int_draw(mm, e, THIS, w(1), sc); none }
            0x18ab68 => di::float_new(mm, THIS, w(1), w(2), w(3), F(fmin), F(fmax)),
            0x18ae80 => di::angle_new(mm, THIS, w(1), w(2), w(3), F(fmin), F(fmax)),
            0x18abf8 | 0x18af10 => di::float_handle_input(mm, e, THIS),
            0x18ada8 | 0x18b0c0 => di::float_measure(mm, e, w(0), THIS),
            0x18ade0 => { di::float_draw(mm, e, THIS, w(1), sc); none }
            0x18b0f8 => { di::angle_draw(mm, e, THIS, w(1), sc); none }
            0x18b198 => di::string_new1(mm, THIS, w(1)),
            0x18b1d0 => di::string_new(mm, THIS, w(1), w(2), w(3), w(4)),
            0x18b240 => { di::string_set(mm, THIS, w(1), w(2), w(3)); none }
            0x18b258 => di::string_handle_input(mm, e, THIS),
            0x18b660 => di::colour_new(mm, THIS, w(1), w(2)),
            0x18b6b8 => di::colour_handle_input(mm, e, THIS),
            0x18b810 => di::expand_menu_command(mm, e, w(0), w(1), w(2)),
            0x18b840 => { di::expand_menu_changed(mm, e, w(0), w(1)); none }
            0x18b870 => di::expand_new0(mm, THIS),
            0x18b8f8 => di::expand_new(mm, THIS, w(1), w(2)),
            0x18b9a0 => { di::expand_add(mm, THIS, w(1), w(2) as i32); none }
            0x18b9a8 => { di::expand_remove(mm, e, THIS, w(1) as i32); none }
            0x18b9b0 => { di::expand_reset(mm, THIS); none }
            0x18b9f8 => { di::expand_replace(mm, e, THIS, w(1)); none }
            0x18ba68 => di::expand_handle_input(mm, e, THIS),
            0x18bb58 => di::expand_command(mm, e, THIS, w(1), w(2)),
            0x18bb88 => { di::expand_changed(mm, e, THIS, w(1)); none }
            0x18bbb8 => di::expand_measure(mm, e, w(0), THIS, sc),
            0x18bd78 => { di::expand_draw(mm, e, THIS, w(1), sc); none }
            0x18beb0 => { di::expand_picked(mm, e, THIS, w(1)); none }
            0x18bf28 => { di::expand_left(mm, e, THIS); none }
            0x18bf50 => di::rgb_title_new(mm, THIS, w(1), w(2)),
            0x18bfb0 => di::rgb_new(mm, THIS, w(1), w(2), w(3)),
            0x18c0a8 => di::rgb_handle_input(mm, e, THIS),
            0x18c118 => di::error_new0(mm, THIS),
            0x18c1c8 => di::error_new(mm, THIS, w(1), w(2)),
            _ => unreachable!(),
        };
        if matches!(func, 0x18b258 | 0x1899c0 | 0x189d78) {
            edits += env.log.iter().filter(|c| c.0 == mv(0x20)).count();
        }
        if matches!(func, 0x18a940 | 0x18abf8 | 0x18af10 | 0x18b6b8) {
            steps += env.log.iter().filter(|c| c.0 == mv(0x20)).count();
        }
        if matches!(func, 0x18bd78) {
            expanded += env.log.iter().filter(|c| c.0 == iv(0x28)).count();
        }
        ans.n.set(0);
        log.borrow_mut().clear();
        // the box: the runner's copy on its stack
        let boxat = 0x01ff_0000 - 0x3f00;
        let mut rregs = regs.clone();
        for v in rregs.iter_mut() {
            if *v == SCRATCH + 0x700 {
                *v = boxat;
            }
        }
        for (k, v) in boxw.iter().enumerate() {
            r.mem.write(boxat + 4 * k as u32, &v.to_le_bytes()).unwrap();
        }
        for (k, v) in rregs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, fmin);
        r.cpu.set_f(13, fmax);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            let theirs = r.cpu.gpr(2) as u32;
            assert_eq!(token(theirs), token(ours), "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        // the box after (the expand item's draw moves its y; the measures fill it)
        for k in 0..4 {
            assert_eq!(r.mem.read_u32(boxat + 4 * k), m.r32(SCRATCH + 0x700 + 4 * k), "case {case} {func:#x}: the box");
        }
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
    println!("{edits} edits, {steps} steps, {expanded} expanded items drawn");
    assert!(edits > 250 && steps > 300 && expanded > 100);
}

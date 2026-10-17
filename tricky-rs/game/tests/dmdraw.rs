//! `tricky_game::dmdraw` (board row G9a4c) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the debug menu's strings at
//! 0x389900.., as the ELF loads them) with the colours' tables made random, an item of the kind under test in the
//! arena (its strings, its value, its colour), its menu, manager and font, and the renderer's table pointing at
//! stand-ins. Then one of the row's functions runs on both sides. Every callee is a stand-in on both sides
//! answering from the case's seed; the font's stand-ins log the string they are given, the gradient its colours.
//! The runner's stack is cleared before each case and a buffer on it logs as one token. The calls, the answers and
//! the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::debugmenu as dm;
use tricky_game::dmdraw as dd;
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
const STRINGS: (u32, u32) = (0x389900, 0x800);
const COLOURS: (u32, u32) = (0x3cde30, 0x70);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const THIS: u32 = ARENA;
const OUTER: u32 = ARENA + 0x1000;
const MGR: u32 = ARENA + 0x1200;
const FONT: u32 = ARENA + 0x1300;
const ITEMS: u32 = ARENA + 0x1400;
const ITEM_TABLES: u32 = ARENA + 0x1c00;
const APPOBJ: u32 = ARENA + 0x3000;
const RENDER: u32 = ARENA + 0x1f00;
const RVT: u32 = ARENA + 0x2000;
const STRS: u32 = ARENA + 0x2400;
const BUF: u32 = ARENA + 0x2800;
const COLOUR: u32 = ARENA + 0x2900;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn iv(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
const IOFFS: [u32; 3] = [0x18, 0x30, 0x38];
const ROFFS: [u32; 5] = [0x1c8, 0x1d0, 0x1d8, 0x1e0, 0x1f0];

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
fn mapped(a: u32) -> bool {
    stack(a) || [DATA, STRINGS, COLOURS, (ARENA, SIZE)].iter().any(|&(at, s)| a >= at && a < at + s)
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
        f if f == iv(0x30) => (2, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (1, 0),
        f if f == rv(0x1c8) => (3, 1),
        f if f == rv(0x1d8) => (1, 0),
        f if f == rv(0x1f0) => (3, 0),
        f if f & 0xffff_0000 == 0x01ff_0000 => (2, 0),
        dd::MEASURE_ASCII => (3, 1),
        dd::PRINT => (2, 3),
        dd::LINE => (1, 4),
        dd::SET_RGB => (2, 3),
        dd::SET_VEC4 | dd::FILLED_RECT => (2, 4),
        dd::GRADIENT_RECT => (5, 4),
        dd::SPRINTF => (3, 0),
        dm::MEM_DELETE => (2, 0),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    // the strings the font is given (as far as their end), the gradient's colours
    if func == dd::MEASURE_ASCII || func == dd::PRINT {
        let mut k = 0;
        while k < 0x40 && mapped(w[1] + k) {
            let c = io.r8(w[1] + k);
            log.push(c as u32);
            if c == 0 {
                break;
            }
            k += 1;
        }
    }
    if func == dd::GRADIENT_RECT {
        for c in 1..5 {
            log.extend((0..4).map(|k| io.r32(w[c] + 4 * k)));
        }
    }
    let r = ans.next();
    let answer = match func {
        f if f == iv(0x18) => [0u32, 1, r][(r % 3) as usize],
        dd::MEASURE_ASCII => [r % 300, 100, 199, 200, 0][(r >> 20) as usize % 5],
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

const FUNCS: [u32; 65] = [
    0x18c288, 0x18c318, 0x18c380, 0x18c728, 0x18c9d0, 0x18cc78, 0x18ceb8, 0x18d0c8, 0x18d280, 0x18d360, 0x18d6a0,
    0x18d8d8, 0x18dce0, 0x18dd58, 0x18df38, 0x18df60, 0x18dfd0, 0x18e080, 0x18e140, 0x18e198, 0x18e228, 0x18e280,
    0x18e2d8, 0x18e360, 0x18e3e8, 0x18e450, 0x18e4a8, 0x18e510, 0x18e578, 0x18e5d8, 0x18e638, 0x18e6b0, 0x18e718,
    0x181a48, 0x18de50, 0x18de60, 0x18df70, 0x18e028, 0x18e0e8, 0x18e2e0, 0x18e2f0, 0x18e2f8, 0x18e308, 0x18e368,
    0x18e378, 0x18e380, 0x18e390, 0x18e3f0, 0x18e3f8, 0x18e4b0, 0x18e4b8, 0x18e518, 0x18e520, 0x18e580, 0x18e5e0,
    0x18e640, 0x18e648, 0x18e650, 0x18e6b8, 0x18e6c0, 0x18e720, 0x18e730, 0x18e740, 0x18e788, 0x18e790,
];

/// The word setters: their offset
fn setter(func: u32) -> Option<u32> {
    match func {
        0x18e0e8 | 0x18e308 | 0x18e390 | 0x18e3f8 | 0x18e4b0 | 0x18e520 | 0x18e5e0 => Some(0x14),
        0x18e2f0 | 0x18e378 | 0x18e3f0 | 0x18e4b8 | 0x18e518 | 0x18e580 | 0x18e640 | 0x18e6b8 => Some(0x10),
        0x18e648 | 0x18e6c0 => Some(0x1c),
        _ => None,
    }
}

#[test]
fn debug_menu_drawing_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    // (the framework's, which the drawing calls)
    for f in [0x1889d8u32, 0x188b10, 0x188bd0, 0x188bf8, 0x188c50, 0x189450, 0x189478, 0x189560, 0x1895b0, 0x189590] {
        r.allow(f);
    }
    let mut stubs = vec![
        dd::MEASURE_ASCII, dd::PRINT, dd::LINE, dd::SET_RGB, dd::SET_VEC4, dd::FILLED_RECT, dd::GRADIENT_RECT, dd::SPRINTF,
        dm::MEM_DELETE,
    ];
    stubs.extend(IOFFS.map(iv));
    stubs.extend(ROFFS.map(rv));
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
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let data = read(&mut r, DATA);
    let strings = read(&mut r, STRINGS);
    let mut g = Rng(0xd4a3_0000_0000_0001);
    let (mut lines, mut picked, mut deleted) = (0usize, 0usize, 0usize);
    let per = 300u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (STRINGS.0, strings.clone()),
                (COLOURS.0, vec![0; COLOURS.1 as usize]),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the colours' tables (floats around 0..1, a few outside)
        for k in 0..COLOURS.1 / 4 {
            wf(&mut m, COLOURS.0 + 4 * k, { let x = g.f(0.0, 1.0); g.pick(&[x, 0.0, 1.0, 0.8, -0.2]) });
        }
        // the app's renderer and its table
        m.w32(dm::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        // the items' tables, the pool, the menu (its items; the item under test sometimes picked), its manager's font
        for t in 0..4 {
            for off in IOFFS {
                m.w16(ITEM_TABLES + 0x40 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(ITEM_TABLES + 0x40 * t + off + 4, iv(off));
            }
        }
        for i in 0..32 {
            let it = ITEMS + 0x20 * i;
            let any = g.next();
            m.w32(it + 8, g.pick(&[1u32, 1, 9, any]));
            m.w32(it + 4, OUTER);
            m.w32(it + 0xc, ITEM_TABLES + 0x40 * g.below(4));
        }
        let n = g.pick(&[1u32, 2, 5]);
        m.w32(OUTER + 4, MGR);
        m.w32(OUTER + 8, n);
        m.w32(OUTER + 0xc, g.below(n));
        for i in 0..64 {
            m.w32(OUTER + 0x14 + 4 * i, ITEMS + 0x20 * g.below(32));
        }
        if g.below(2) == 0 {
            let at = m.r32(OUTER + 0xc);
            m.w32(OUTER + 0x14 + 4 * at, THIS);
        }
        m.w32(MGR + 0x38, FONT);
        m.w32(FONT + 0xc, g.pick(&[0u32, 10, 22, 30]));
        // strings
        for i in 0..32 {
            for k in 0..16 {
                m.w8(STRS + 16 * i + k, if k < 1 + g.below(14) { b'a' + g.below(26) as u8 } else { 0 });
            }
        }
        let str_ = |g: &mut Rng| STRS + 16 * g.below(32);
        // the item: menu, flags, table; its strings; a string's buffer and cursor; a colour and channel
        m.w32(THIS + 4, OUTER);
        let any = g.next();
        m.w32(THIS + 8, g.pick(&[1u32, 1, 9, any]));
        m.w32(THIS + 0xc, ITEM_TABLES + 0x40 * g.below(4));
        let s = str_(&mut g);
        m.w32(THIS + 0x10, s);
        let s = str_(&mut g);
        m.w32(THIS + 0x14, s);
        let len = g.pick(&[0u32, 1, 5, 12]);
        for i in 0..0x20 {
            m.w8(BUF + i, if i < len { b'a' + g.below(26) as u8 } else if i == len { 0 } else { 0x7e });
        }
        m.w32(THIS + 0x1c, g.below(len + 1));
        for k in 0..4 {
            wf(&mut m, COLOUR + 4 * k, { let x = g.f(0.0, 1.0); g.pick(&[x, 0.0, 1.0]) });
        }
        match func {
            0x18d360 => m.w32(THIS + 0x14, BUF),
            0x18d6a0 | 0x18d8d8 | 0x18dd58 => {
                m.w32(THIS + 0x10, COLOUR);
                m.w32(THIS + 0x14, g.below(3));
                if func == 0x18dd58 {
                    let s = str_(&mut g);
                    m.w32(THIS + 0x14, s);
                }
            }
            _ => {}
        }
        // the arguments: a box (four ints), strings or none, colours, a value
        let (t, l, v) = (str_(&mut g), str_(&mut g), str_(&mut g));
        let (t, l, v) = (g.pick(&[t, t, 0]), g.pick(&[l, l, 0]), g.pick(&[v, v, 0]));
        let (bc, tc) = (COLOURS.0 + 4 * g.below(16), COLOURS.0 + 4 * g.below(16));
        let flags = g.below(4);
        let word = g.next();
        let (f1, f2) = (g.f(-10.0, 10.0), g.f(-10.0, 10.0));
        let boxp = SCRATCH + 0x700;
        let boxw: Vec<u32> = (0..4).map(|_| { let (x, y) = (g.below(640), g.below(60)); g.pick(&[x, y, 0]) }).collect();
        for (k, v) in boxw.iter().enumerate() {
            m.w32(boxp + 4 * k as u32, *v);
        }
        let regs: Vec<u32> = match func {
            0x18c288 | 0x18c318 => vec![OUTER],
            0x18c380 => vec![boxp, COLOUR],
            0x18c728 | 0x18c9d0 => vec![OUTER, boxp],
            0x18cc78 => vec![boxp, THIS, t, l, v],
            0x18ceb8 => vec![THIS, boxp, t, l, v, bc, tc],
            0x18d0c8 => vec![THIS, boxp, t, l, v],
            0x18d280 | 0x18d6a0 | 0x18dce0 => vec![boxp, THIS],
            0x18d360 | 0x18d8d8 | 0x18dd58 => vec![THIS, boxp],
            0x18df38 => vec![dd::DRAW_COLOUR, boxp],
            0x18df60 | 0x181a48 | 0x18df70 => vec![dd::DRAW_COLOUR, COLOUR],
            0x18de50 | 0x18de60 => vec![],
            0x18e650 => vec![THIS, word, word.rotate_left(7)],
            f if setter(f).is_some() || f == 0x18e028 || f == 0x18e790 => vec![THIS, word],
            0x18e720 | 0x18e740 | 0x18e788 => vec![THIS],
            0x18e730 => vec![OUTER],
            _ => vec![THIS, flags],
        };
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
            0x18c288 => { dd::begin_draw(mm, e, w(0)); none }
            0x18c318 => { dd::end_draw(mm, e, w(0)); none }
            0x18c380 => { dd::draw_box(mm, e, w(0), w(1), sc); none }
            0x18c728 => { dd::draw_up_arrow(mm, e, w(0), w(1), sc); none }
            0x18c9d0 => { dd::draw_down_arrow(mm, e, w(0), w(1), sc); none }
            0x18cc78 => dd::measure_label(mm, e, w(0), w(1), w(2), w(3), w(4)),
            0x18ceb8 => { dd::draw_item_box(mm, e, w(0), w(1), w(2), w(3), w(4), w(5), w(6), sc); none }
            0x18d0c8 => { dd::draw_label(mm, e, w(0), w(1), w(2), w(3), w(4), sc); none }
            0x18d280 => dd::string_measure(mm, e, w(0), w(1)),
            0x18d360 => { dd::string_draw(mm, e, w(0), w(1), sc); none }
            0x18d6a0 => dd::colour_measure(mm, e, w(0), w(1)),
            0x18d8d8 => { dd::colour_draw(mm, e, w(0), w(1), sc); none }
            0x18dce0 => dd::rgb_title_measure(mm, e, w(0), w(1), sc),
            0x18dd58 => { dd::rgb_title_draw(mm, e, w(0), w(1), sc); none }
            0x18df38 => { dd::rect_i(mm, e, w(0), w(1)); none }
            0x18df60 => { dd::vec4_from_vec3(mm, e, w(0), w(1), F(f1)); none }
            0x181a48 | 0x18df70 => { dd::set_rgbv(mm, e, w(0), w(1)); none }
            0x18dfd0 => { dd::error_delete(mm, e, THIS, w(1)); none }
            0x18e080 => { dd::rgb_delete(mm, e, THIS, w(1)); none }
            0x18e198 => { dd::expand_delete(mm, e, THIS, w(1)); none }
            0x18e228 => { dm::menu_delete(mm, e, THIS, w(1)); none }
            0x18e140 | 0x18e280 | 0x18e2d8 | 0x18e360 | 0x18e3e8 | 0x18e450 | 0x18e4a8 | 0x18e510 | 0x18e578 | 0x18e5d8
            | 0x18e638 | 0x18e6b0 | 0x18e718 => { dd::plain_delete(mm, e, THIS, w(1)); none }
            0x18de50 => { dd::manager_at_exit(mm, e); none }
            0x18de60 => { dd::static_init(mm); none }
            0x18e028 => { dd::rgb_set_title(mm, THIS, w(1)); none }
            0x18e2e0 | 0x18e368 => { dd::float_set_scale(mm, THIS, F(f1), F(f2)); none }
            0x18e2f8 | 0x18e380 => { dd::float_set_range(mm, THIS, F(f1), F(f2)); none }
            0x18e650 => { dd::enum_set_count_list(mm, THIS, w(1), w(2)); none }
            0x18e720 => dd::item_font(mm, THIS),
            0x18e730 => dd::menu_font(mm, OUTER),
            0x18e740 => dd::get4(mm, THIS),
            0x18e788 => dd::get0(mm, THIS),
            0x18e790 => { dd::set0(mm, THIS, w(1)); none }
            f => { dd::set_word(mm, THIS, setter(f).unwrap(), w(1)); none }
        };
        lines += env.log.iter().filter(|c| c.0 == dd::LINE).count();
        if func == 0x18d0c8 {
            picked += env.log.iter().filter(|c| c.0 == iv(0x18)).count();
        }
        deleted += env.log.iter().filter(|c| c.0 == dm::MEM_DELETE).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        // the box: the runner's copy on its stack
        let boxat = 0x01ff_0000 - 0x3f00;
        for (k, v) in boxw.iter().enumerate() {
            r.mem.write(boxat + 4 * k as u32, &v.to_le_bytes()).unwrap();
        }
        for (k, v) in regs.iter().enumerate() {
            let v = if *v == boxp { boxat } else { *v };
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f1);
        r.cpu.set_f(13, f2);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(token(r.cpu.gpr(2) as u32), token(ours), "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for k in 0..4 {
            assert_eq!(r.mem.read_u32(boxat + 4 * k), m.r32(boxp + 4 * k), "case {case} {func:#x}: the box");
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
    println!("{lines} lines, {picked} labels' items checked, {deleted} frees");
    assert!(lines > 3000 && picked > 100 && deleted > 300);
}

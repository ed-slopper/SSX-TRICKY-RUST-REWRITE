//! `tricky_game::textbox` (board row G7n) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is a fresh box run through a random script of the text box's functions: reset,
//! text set wide, narrow or appended (words, runs of spaces, `\` breaks, words longer than the box, `~` lines),
//! the box's fields changed between steps (its size, alignment, indent, scale, line height, first line, shadow),
//! then the draws and the queries. The string, font and memory functions are stand-ins on both sides that do
//! their work (copy, append, count, measure by a made-up width per character) and log what they are handed; the
//! text style the draws take is on the game's stack, so its address is left out of their logs. After every
//! step the calls, the answer and the memory (bar the port's scratch) are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::{self as tb, TextBoxEnv};

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

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const TB: u32 = ARENA;
const FONT: u32 = ARENA + 0x1800;
const WIDE: u32 = ARENA + 0x2000;
const NARROW: u32 = ARENA + 0x3000;
const BUF: u32 = ARENA + 0x3800;
const SCRATCH: u32 = 0x0180_0000;
/// The break string (0x337e58) and the drawer (0x337fe8) live in the game's data
const DATA: u32 = 0x337e00;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What the stand-ins need of a memory
trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn r16(&mut self, a: u32) -> u16;
    fn w16(&mut self, a: u32, v: u16);
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn r16(&mut self, a: u32) -> u16 {
        Mem::r16(self, a)
    }
    fn w16(&mut self, a: u32, v: u16) {
        Mem::w16(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8; 1];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn r16(&mut self, a: u32) -> u16 {
        let mut b = [0u8; 2];
        self.read(a, &mut b).unwrap();
        u16::from_le_bytes(b)
    }
    fn w16(&mut self, a: u32, v: u16) {
        self.write_u16(a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
}

/// Each call out: how many words and floats it takes, and whether its first word is the stack's text style
fn shape(func: u32) -> (usize, usize, bool) {
    match func {
        tb::WSTR_COUNT | tb::STRLEN | tb::FREE => (1, 0, false),
        tb::WSTR_COPY | tb::WSTR_APPEND | tb::WSTR_SET_ASCII | tb::SET_COLOR => (2, 0, false),
        tb::ALLOC | tb::MEM_SET => (3, 0, false),
        tb::MEASURE => (3, 1, false),
        tb::DRAW_STRING => (2, 3, false),
        tb::STYLE_INIT | tb::STYLE_CONSTRUCT => (1, 0, true),
        tb::STYLE_SET_FONT | tb::STYLE_DESTRUCT | tb::STYLE_DESTRUCT_THUNK => (2, 0, true),
        tb::DRAW_ALIGNED | tb::DRAW_WITH_CURSOR => (6, 3, true),
        _ => panic!("no stand-in for {func:#x}"),
    }
}
const STANDINS: [u32; 18] = [
    tb::WSTR_COUNT, tb::WSTR_COPY, tb::WSTR_SET_ASCII, tb::WSTR_APPEND, tb::STRLEN, tb::ALLOC, tb::MEM_SET, tb::FREE,
    tb::SET_COLOR, tb::DRAW_STRING, tb::MEASURE, tb::STYLE_INIT, tb::STYLE_DESTRUCT, tb::DRAW_ALIGNED, tb::STYLE_SET_FONT,
    tb::STYLE_CONSTRUCT, tb::STYLE_DESTRUCT_THUNK, tb::DRAW_WITH_CURSOR,
];

/// A stand-in's work and answer, and its log
fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32]) -> (u32, Call) {
    let (nw, nf, style) = shape(func);
    let mut log: Vec<u32> = w[usize::from(style)..nw].to_vec();
    log.extend(&f[..nf]);
    let wlen = |io: &mut dyn Io, a: u32| {
        let mut n = 0u32;
        while n < 0x600 && io.r16(a + 2 * n) != 0 {
            n += 1;
        }
        n
    };
    let copy = |io: &mut dyn Io, d: u32, s: u32| {
        let mut k = 0u32;
        loop {
            let c = io.r16(s + 2 * k);
            io.w16(d + 2 * k, c);
            if c == 0 || k >= 0x600 {
                break;
            }
            k += 1;
        }
    };
    let answer = match func {
        tb::WSTR_COUNT => wlen(io, w[0]),
        tb::WSTR_COPY => {
            copy(io, w[0], w[1]);
            w[0]
        }
        tb::WSTR_APPEND => {
            let n = wlen(io, w[0]);
            copy(io, w[0] + 2 * n, w[1]);
            w[0]
        }
        tb::WSTR_SET_ASCII => {
            let mut k = 0u32;
            loop {
                let c = io.r8(w[1] + k);
                io.w16(w[0] + 2 * k, c as u16);
                if c == 0 {
                    break;
                }
                k += 1;
            }
            w[0]
        }
        tb::STRLEN => {
            let mut n = 0u32;
            while io.r8(w[0] + n) != 0 {
                n += 1;
            }
            n
        }
        tb::ALLOC => BUF,
        tb::SET_COLOR => {
            for k in 0..4 {
                let v = io.r32(w[1] + 4 * k);
                io.w32(w[0] + 4 * k, v);
            }
            0
        }
        tb::MEASURE => {
            // a made-up width: 3 to 10 a character, by its low bits
            let mut n = 0u32;
            let mut k = 0u32;
            while k < 0x600 {
                let c = io.r16(w[1] + 2 * k);
                if c == 0 {
                    break;
                }
                n += 3 + (c as u32 * 5) % 8;
                k += 1;
            }
            n
        }
        _ => 0,
    };
    log.push(answer);
    (answer, Call(func, log))
}

struct Ours(Vec<Call>);
impl TextBoxEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(4, 0);
        let (a, c) = standin(m, func, &w, &f);
        self.0.push(c);
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// A random text: words of letters, runs of spaces, `\` breaks, long words, `~` openings
fn text(g: &mut Rng) -> Vec<u16> {
    let mut t = Vec::new();
    // (and many short lines, so that two or three reach the 128 lines: `a\` again and again, or short words)
    match g.below(8) {
        0 => {
            for _ in 0..60 {
                t.extend([0x61, 0x5c]);
            }
            return t;
        }
        1 => {
            for _ in 0..40 {
                t.extend([0x61 + g.below(26) as u16, 0x62, 0x20]);
            }
            return t;
        }
        _ => {}
    }
    let words = g.pick(&[0u32, 1, 3, 8, 20, 40]);
    for k in 0..words {
        if k > 0 {
            for _ in 0..g.pick(&[1u32, 1, 1, 2, 3]) {
                t.push(0x20);
            }
        }
        match g.below(10) {
            0 => t.push(0x5c),
            1 => t.push(0x7e),
            _ => {}
        }
        let len = g.pick(&[1u32, 3, 5, 8, 30]);
        for _ in 0..len {
            t.push(0x61 + g.below(26) as u16);
        }
        if g.below(12) == 0 {
            t.push(0x5c);
        }
    }
    if g.below(6) == 0 {
        t.push(0x20);
    }
    // (at most eight appends of 121 stay in the box's 1024 characters)
    t.truncate(120);
    t
}

#[test]
fn text_box_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1957b0u32, 0x1957d8, 0x195a48, 0x195c88, 0x195de0, 0x195ee8, 0x195ff0, 0x196028, 0x196070, 0x196088, 0x196128, 0x1961e8, 0x1961f0, 0x196218, 0x1964a0, 0x1964d8, 0x196530] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in STANDINS {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..16).map(|k| c.fpr[k]).collect();
            let (a, call) = standin(mem, func, &w, &f);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x7e47_b000_0000_0001);
    let mut data = vec![0u8; 0x200];
    r.mem.read(DATA, &mut data).unwrap();
    let (mut wrapped, mut drawn, mut steps) = (0usize, 0usize, 0usize);
    for case in 0..3000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (DATA, data.clone()), (SCRATCH, vec![0; 0x10])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // (the font: Reset leaves it as it was)
        m.w32(TB, FONT);
        m.w32(FONT + 0xc, g.pick(&[0u32, 8, 12, 16]));
        for k in 0..4 {
            wf(&mut m, tb::DRAWER + 4 * k, g.f(0.0, 1.0));
        }
        let mut script = vec![0u32];
        for _ in 0..g.pick(&[2usize, 4, 7]) {
            script.push(g.below(16));
        }
        for (step, &op) in script.iter().enumerate() {
            // the fields changed between steps (but the lines: they stay the box's own)
            if step > 0 && g.below(2) == 0 {
                wf(&mut m, TB + 4, g.f(-50.0, 500.0));
                wf(&mut m, TB + 8, g.f(-50.0, 400.0));
                let w = g.f(10.0, 400.0);
                wf(&mut m, TB + 0xc, g.pick(&[w, w, 10.0, 40.0, 1000.0]));
                wf(&mut m, TB + 0x10, g.f(0.0, 300.0));
                wf(&mut m, TB + 0x14, { let v = g.f(5.0, 30.0); g.pick(&[9.0f32, 15.0, v]) });
                wf(&mut m, TB + 0x18, { let v = g.f(0.3, 2.0); g.pick(&[1.0f32, 0.5, v]) });
                m.w32(TB + 0x1c, g.pick(&[0u32, 0, 1, 3, 130]));
                m.w32(TB + 0x20, g.pick(&[0u32, 0, 1]));
                m.w32(TB + 0x24, g.pick(&[0u32, 0, 1]));
                wf(&mut m, TB + 0x28, { let v = g.f(-20.0, 40.0); g.pick(&[0.0f32, 10.0, v]) });
                wf(&mut m, TB + 0x34, g.f(-20.0, 20.0));
                wf(&mut m, TB + 0x38, g.f(0.0, 1.0));
                m.w32(TB + 0x6c, g.below(2));
            }
            let t = text(&mut g);
            // (an append that would run past the box's 1024 characters clears it instead)
            let mut held = 0u32;
            while held < 1024 && m.r16(TB + 0xa70 + 2 * held) != 0 {
                held += 1;
            }
            let op = if matches!(op, 4..=6) && held as usize + t.len() + 2 > 1000 { 2 } else { op };
            for (k, &c) in t.iter().chain([0u16].iter()).enumerate() {
                m.w16(WIDE + 2 * k as u32, c);
                m.w8(NARROW + k as u32, c as u8);
            }
            let (count, cursor, again) = (g.pick(&[-1i32, 0, 5, 40, 200]), g.below(2), g.below(2));
            let (fa, fb) = (g.f(0.3, 2.0), { let v = g.f(0.5, 1.5); g.pick(&[1.0f32, 0.8, v]) });
            let c4 = [g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0)];
            for (at, b) in &m.regions {
                if *at != SCRATCH {
                    r.mem.write(*at, b).unwrap();
                }
            }
            log.borrow_mut().clear();
            let mut o = Ours(Vec::new());
            let ours: u32;
            let (func, regs, fregs): (u32, Vec<(usize, u32)>, Vec<(usize, f32)>);
            match op {
                0 => {
                    ours = tb::init(&mut m, TB);
                    (func, regs, fregs) = (0x1957b0, vec![(4, TB)], vec![]);
                }
                1 => {
                    tb::reset(&mut m, TB);
                    ours = 0;
                    (func, regs, fregs) = (0x195ee8, vec![(4, TB)], vec![]);
                }
                2 => {
                    tb::clear_text(&mut m, TB);
                    ours = 0;
                    (func, regs, fregs) = (0x195ff0, vec![(4, TB)], vec![]);
                }
                3 => {
                    ours = tb::set_font(&mut m, &mut o, TB, FONT);
                    (func, regs, fregs) = (0x196028, vec![(4, TB), (5, FONT)], vec![]);
                }
                4 | 5 => {
                    ours = tb::append_text(&mut m, &mut o, TB, WIDE);
                    (func, regs, fregs) = (0x196088, vec![(4, TB), (5, WIDE)], vec![]);
                }
                6 => {
                    ours = tb::set_ascii(&mut m, &mut o, TB, NARROW);
                    (func, regs, fregs) = (0x196128, vec![(4, TB), (5, NARROW)], vec![]);
                }
                7 => {
                    ours = tb::wrap_lines(&mut m, &mut o, TB, again);
                    (func, regs, fregs) = (0x196218, vec![(4, TB), (5, again)], vec![]);
                }
                8 => {
                    tb::set_scale(&mut m, TB, F(fa), F(fb));
                    ours = 0;
                    (func, regs, fregs) = (0x1961f0, vec![(4, TB)], vec![(12, fa), (13, fb)]);
                }
                9 => {
                    tb::color_set4(&mut m, TB + 0x3c - 4, c4.map(F));
                    ours = 0;
                    (func, regs, fregs) = (0x196070, vec![(4, TB + 0x3c - 4)], vec![(12, c4[0]), (13, c4[1]), (14, c4[2]), (15, c4[3])]);
                }
                10 => {
                    tb::draw(&mut m, &mut o, TB, count, cursor, SCRATCH);
                    ours = 0;
                    (func, regs, fregs) = (0x1957d8, vec![(4, TB), (5, count as u32), (6, cursor)], vec![]);
                }
                11 => {
                    tb::draw_highlighted(&mut m, &mut o, TB);
                    ours = 0;
                    (func, regs, fregs) = (0x195a48, vec![(4, TB)], vec![]);
                }
                12 => {
                    tb::draw_centered(&mut m, &mut o, TB, count, cursor, SCRATCH);
                    ours = 0;
                    (func, regs, fregs) = (0x195c88, vec![(4, TB), (5, count as u32), (6, cursor)], vec![]);
                }
                13 => {
                    tb::draw_vcentered(&mut m, &mut o, TB, count, cursor, SCRATCH);
                    ours = 0;
                    (func, regs, fregs) = (0x195de0, vec![(4, TB), (5, count as u32), (6, cursor)], vec![]);
                }
                14 => {
                    ours = tb::height(&m, TB);
                    (func, regs, fregs) = (0x196530, vec![(4, TB)], vec![]);
                }
                _ => {
                    ours = tb::max_line_width(&m, TB);
                    (func, regs, fregs) = (0x1964d8, vec![(4, TB)], vec![]);
                }
            }
            for (k, v) in regs {
                r.cpu.set_gpr(k, v as i32 as i64 as u64);
            }
            for (k, v) in fregs {
                r.cpu.set_f(k, v);
            }
            r.call(func).unwrap_or_else(|e| panic!("case {case} step {step} op {op}: {e}"));
            let theirs = log.borrow().clone();
            if theirs != o.0 {
                let i = theirs.iter().zip(&o.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.0.len()));
                let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(12)])).unwrap_or_default();
                panic!("case {case} step {step} op {op}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.0.len(), show(theirs.get(i)), show(o.0.get(i)));
            }
            let answer = r.cpu.gpr(2) as u32;
            let returns = matches!(op, 0 | 3..=7 | 14 | 15);
            assert!(!returns || answer == ours, "case {case} step {step} op {op}: answer theirs {answer:#x} ours {ours:#x}");
            for (at, b) in &m.regions {
                if *at == SCRATCH {
                    continue;
                }
                let mut th = vec![0u8; b.len()];
                r.mem.read(*at, &mut th).unwrap();
                if &th != b {
                    let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                    let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                    panic!("case {case} step {step} op {op}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
                }
            }
            steps += 1;
            wrapped += (tb::line_count(&m, TB) > 1) as usize;
            drawn += o.0.iter().filter(|c| matches!(c.0, tb::DRAW_ALIGNED | tb::DRAW_WITH_CURSOR | tb::DRAW_STRING)).count();
        }
    }
    println!("{steps} steps, {wrapped} with two lines or more, {drawn} lines drawn");
    assert!(wrapped > 2500 && drawn > 2500);
}

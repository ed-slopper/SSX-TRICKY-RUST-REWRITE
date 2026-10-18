//! `tricky_game::start_screen` (board row E1f3) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the memory card messages (`MemCardMsg_DrawBox`, `_DrawOptions`, `_DrawButtonPrompt`, `_Draw` and
//! `Overlay_DrawDontRemoveCard`). Every case builds a random screen, app and renderer in the arena and the game's
//! data; one function then runs on both sides. The messages' own functions run for real on both sides; every other
//! callee is a stand-in answering from the case's seed: the strings, the text widths, the sprites, the text box's and
//! the font's calls and the renderer's virtual. The calls are compared with their arguments (vectors on the stack
//! read by value), the answers (the widths as floats) and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::app as ap;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::game_main as gm;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::start_screen as ss;

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
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x6000;
const APPOBJ: u32 = ARENA + 0x100;
const R: u32 = ARENA + 0x900;
const S: u32 = ARENA + 0x1000;
/// the renderer's table
const VT: u32 = ARENA + 0x5000;
const STRS: u32 = ARENA + 0x5400;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 1] = [0x1e0];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn stack(a: u32) -> bool {
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

fn arity(f: u32) -> (usize, usize) {
    match f {
        ss::TB_RECT => (1, 4),
        ss::TB_TEXT | ss::SPRITE_SET_RECT | gm::FONT_COLOUR | ap::LOC_STRING => (2, 0),
        ss::TB_LINE => (1, 1),
        ss::TB_DRAW => (3, 0),
        ss::TEXT_WIDTH => (3, 1),
        ss::FONT_PRINT => (2, 3),
        ss::SPRITE_AT => (2, 4),
        f if f == slot(0x1e0) => (2, 0),
        _ => (1, 0),
    }
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

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..3).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        ap::LOC_STRING => STRS + 0x40 * g.below(32),
        ss::TEXT_WIDTH => g.pick(&[0u32, 7, 64, 129, 300]),
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let (ni, nf) = arity(f);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(8, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 5] = [0x183538, 0x183660, 0x183788, 0x183ae8, 0x183c20];

/// What a function answers: a float (its bits), or nothing
enum Answer {
    Float(u32),
    None,
}

/// One function on our side
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], fa: [f32; 2]) -> Answer {
    let buf = SCRATCH;
    match f {
        0x183538 => {
            ss::draw_box(m, e, a[0], a[1], a[2], a[3], buf);
            Answer::None
        }
        0x183660 => {
            ss::draw_dont_remove_card(m, e, a[0]);
            Answer::None
        }
        0x183788 => Answer::Float(ss::draw_options(m, e, a[0], a[1], a[2]).0.to_bits()),
        0x183ae8 => Answer::Float(ss::button_prompt(m, e, a[0], a[1], a[2], a[3], F(fa[0]), F(fa[1]), buf).0.to_bits()),
        0x183c20 => {
            ss::draw_buttons(m, e, a[0], a[1], a[2], buf);
            Answer::None
        }
        _ => unreachable!("{f:#x}"),
    }
}

/// The flags the messages take: the seven buttons (1, 2, 4, 8, 0x10, 0x20, 0x40), any mix
fn flags(g: &mut Rng) -> u32 {
    match g.below(3) {
        0 => g.pick(&[1u32, 2, 4, 8, 0x10, 0x20, 0x40]),
        1 => g.next() & 0x7f,
        _ => g.next(),
    }
}

#[test]
fn memory_card_messages_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter() {
        r.allow(*f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    let mut stubs = vec![
        ss::TB_RECT, ss::TB_TEXT, ss::TB_LINE, ss::TB_LAYOUT, ss::TB_CLEAR, ss::TB_DRAW, ss::TEXT_WIDTH,
        ss::FONT_PRINT, ss::SPRITE_AT, ss::SPRITE_SET_RECT, gm::FONT_COLOUR, ap::LOC_STRING,
    ];
    stubs.extend(SLOTS.map(slot));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x3e55_0000_0000_0001);
    let per = 4000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let regions = vec![(DATA.0, data.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x100])];
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the screen: the font's line (+0x14, a word), the text box's line height (+0x5c), the blink)
        let v = g.next();
        m.w32(S + 0x14, g.pick(&[0u32, 12, 16, 20, 0x8000_0001, v]));
        m.w32(S + 0x5c, g.f(0.0, 30.0).to_bits());
        m.w32(S + 0x3ca8, g.f(0.0, 1.0).to_bits());
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = match func {
            0x183538 => {
                let f = flags(&mut g);
                [S, STRS + 0x40 * g.below(32), g.pick(&[0u32, 0, f]), g.below(2)]
            }
            0x183788 => [S, g.pick(&[0u32, 1, 2, 3, 4]), flags(&mut g), 0],
            0x183ae8 => {
                let b = if g.below(4) == 0 { g.below(0x50) } else { g.pick(&[1u32, 2, 4, 8, 0x10, 0x20, 0x40]) };
                [S, b, STRS + 0x40 * g.below(32), g.below(0x60)]
            }
            0x183c20 => [S, flags(&mut g), g.below(2), 0],
            _ => [S, 0, 0, 0],
        };
        let fa = [g.f(0.0, 640.0), g.f(0.0, 480.0)];
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a, fa);
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        for (k, v) in fa.iter().enumerate() {
            r.cpu.set_f(12 + k, *v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        match mine {
            Answer::Float(v) => assert_eq!(r.cpu.f(0).to_bits(), v, "case {case} {func:#x}: the answer"),
            Answer::None => {}
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
}

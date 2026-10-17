//! `tricky_game::resultpages` (board row G9v) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the speech's pointer, as the ELF
//! loads them) with the parts the pages read made random: the career's mode, its event's size and ids, the race's
//! riders (players or not, their characters), the finishing order and times; an overlay with its page and HUD in
//! the arena. Then one of the row's functions runs on both sides. Every callee is a stand-in on both sides,
//! answering from the case's seed and its count of calls (the fills done for real; the strings are never read). A
//! format's buffer on the game's stack and the port's scratch are logged as one token. The calls and the memory are
//! compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::resultpages as rp;
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

/// The game's data the pages touch, and the speech's pointer
const DATA: (u32, u32) = (0x328000, 0x18000);
const SPEECH_AT: (u32, u32) = (0x343000, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1000;
const THIS: u32 = ARENA;
const PAGE: u32 = ARENA + 0x200;
const PAGE0: u32 = ARENA + 0x300;
const BASE: u32 = ARENA + 0x340;
const HUD: u32 = ARENA + 0x400;
const SPRITES: u32 = ARENA + 0x900;
const RECT: u32 = ARENA + 0x980;
const APPOBJ: u32 = ARENA + 0xa00;
/// The port's scratch: inside the runner's stack's range, so both log as the token
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// Where the stand-in strings point (never read)
const STRINGS: u32 = 0x0170_0000;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
}
fn mapped(a: u32, n: u32) -> bool {
    [DATA, SPEECH_AT, (ARENA, SIZE)].iter().any(|&(at, s)| a >= at && a as u64 + n as u64 <= (at + s) as u64)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x0200_0000).contains(&a) { STACK_TOKEN } else { a }
}

/// The stand-ins' answers: from the case's seed and how many calls came before
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

/// How many of a callee's arguments are logged, and its answer
fn standin(io: &mut dyn Io, func: u32, w: &[u32], ans: &Answers) -> (u32, Call) {
    let n = match func {
        rp::LOC | rp::WSTR_COPY | rp::WSTR_SET_ASCII | rp::GET_RECT | rp::PLAYER_ID | rp::ATTR_POINTS => 2,
        rp::FORMAT_TIME => 2,
        rp::MEMSET | rp::MEDAL_RANK | rp::SPEECH_END => 3,
        rp::WSTR_FORMAT_ASCII => match w[1] {
            rp::FMT_D => 3,
            rp::FMT_DS | rp::FMT_F => 4,
            _ => 5,
        },
        rp::SPEECH_CLEAR => 2,
        _ => 1,
    };
    let log: Vec<u32> = w[..n].iter().map(|&a| token(a)).collect();
    let r = ans.next();
    let answer = match func {
        rp::LOC => STRINGS + 4 * (w[1] & 0xffff),
        rp::GET_RECT => RECT,
        rp::MEMSET => {
            if mapped(w[0], w[2]) {
                for k in 0..w[2] {
                    io.w8(w[0] + k, w[1] as u8);
                }
            }
            0
        }
        rp::EVENT_MODE => r % 4,
        rp::PLAYER_ID => r % 4,
        rp::ATTR_POINTS => [0u32, 0, 1, 7, 0xffff_ffff][(r % 5) as usize],
        rp::MEDAL_RANK => r % 5,
        rp::CLAMP_RANK => r % 4,
        rp::GOT_RACE_FINAL | rp::GOT_SHOWOFF_FINAL => r % 2,
        _ => 0,
    };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], _: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, func, &w, self.ans);
        self.log.push(c);
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn result_pages_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x1bf918u32, 0x1bf980, 0x1bf9d8, 0x1bfa08, 0x1bfa30, 0x1bfa50, 0x1bfa70, 0x1bfa90, 0x1bfab0, 0x1bfac8, 0x1bfae0,
        0x1bfaf8, 0x1bfb10, 0x1bfb20, 0x1aae10, 0x1aae88, 0x1af720, 0x1af940, 0x1af950, 0x1b1ba8,
    ];
    for f in funcs {
        r.allow(f);
    }
    // the compiler's float to double (`__extendsfdf2`): exact, so done here as the port does it
    r.stub(0x2f7aa8, |c, _| {
        let d = (c.f(12) as f64).to_bits();
        c.set_gpr(2, d);
    });
    let stubs = [
        rp::LOC, rp::MEMSET, rp::WSTR_COPY, rp::WSTR_SET_ASCII, rp::WSTR_FORMAT_ASCII, rp::GET_RECT, rp::BACKDROP_RESET,
        rp::GOT_RACE_FINAL, rp::GOT_SHOWOFF_FINAL, rp::EVENT_MODE, rp::PLAYER_ID, rp::ATTR_POINTS, rp::MEDAL_RANK,
        rp::CLAMP_RANK, rp::FORMAT_TIME, rp::SPEECH_END, rp::SPEECH_CLEAR,
    ];
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let mut w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            if func == rp::WSTR_FORMAT_ASCII && matches!(w[1], rp::FMT_F | rp::FMT_FS) {
                // the double in one register: its two halves, as the port passes them
                let d = c.gpr(6);
                w = vec![w[0], w[1], d as u32, (d >> 32) as u32, w[3], 0, 0, 0];
            }
            let (v, call) = standin(mem, func, &w, &a);
            l.borrow_mut().push(call);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let data = read(&mut r, DATA);
    let speech = read(&mut r, SPEECH_AT);
    let mut g = Rng(0x5ce1_5000_0000_0001);
    let (mut rows, mut verdicts) = (0usize, 0usize);
    let per = 300u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut m = Mem { regions: vec![(DATA.0, data.clone()), (SPEECH_AT.0, speech.clone()), (ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the overlay, its page, the HUD's sprites
        m.w32(THIS, PAGE);
        m.w32(THIS + 4, HUD);
        m.w32(PAGE, PAGE0);
        m.w32(PAGE0 + 4, BASE);
        // (the pages' builder empties the menu first; the career page called alone finds a few items)
        m.w32(PAGE + 0x34, g.below(3));
        m.w32(BASE + 0xc, g.below(300));
        m.w32(HUD + 0x42c, SPRITES);
        // the app (its string table) and the speech, set at run time
        m.w32(rp::APP, APPOBJ);
        m.w32(rp::SPEECH, g.next());
        // (and past 2^24, where whole numbers round: sums in another order come out differently)
        let x = { let v = g.f(-50.0, 400.0); let w = g.f(2e7, 2e8); g.pick(&[v, w]) };
        wf(&mut m, RECT + 8, x);
        // the career and the race
        let mode = g.pick(&[4u32, 4, 4, 8, 0, 10, 1]);
        m.w32(rp::CAREER + 0x2afc, mode);
        // (six riders at most: the order's six words run into the times)
        let n = { let v = g.below(7); g.pick(&[v, 6, 4]) };
        let any = g.below(7);
        m.w32(rp::CAREER + 0x27dc, g.pick(&[n, any]));
        m.w32(rp::RACE, n);
        for i in 0..8 {
            let e = rp::RACE + 0x84 * i;
            m.w8(e + 0x69, g.pick(&[0xffu8, 0xff, 0, 1, 0xfe]));
            m.w32(e + 0x3c, g.below(10));
        }
        for i in 0..6 {
            m.w32(rp::ORDER + 4 * i, g.below(6));
            m.w32(rp::TIMES + 4 * i, g.next());
        }
        m.w32(rp::CAREER + 0x3264, g.below(6));
        for o in [0x31c, 0x320] {
            m.w32(rp::CAREER + 0x27dc + o, g.next());
        }
        m.w32(THIS + 0x1c, g.pick(&[0u32, 1, 2, 3]));
        // what the ELF leaves 0 and the game fills at run time: the lines and the words before them, the modes' titles
        for at in (rp::LINES - 8..rp::MODE_TITLES + 0x20).step_by(4) {
            m.w32(at, g.next());
        }
        for at in (rp::COL_X..rp::COL_X + 0x48).step_by(4) {
            wf(&mut m, at, g.f(-100.0, 600.0));
        }
        let (row, col) = (g.below(8), g.below(16));
        let (a3, t0) = (g.next(), g.next());
        let any = g.f(-1000.0, 1000.0);
        let fv = g.pick(&[any, 0.0, 0.5, 1e6]);
        let (fx, fw) = (g.f(-100.0, 400.0), g.f(0.0, 40.0));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        let seed = ((g.next() as u64) << 32) | g.next() as u64;
        ans.seed.set(seed);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let none = u32::MAX;
        let ours = match func {
            0x1bf918 => {
                rp::set_cell_float_suffix(&mut m, &mut env, row, col, a3, F(fv));
                none
            }
            0x1bf980 => {
                rp::set_cell_float(&mut m, &mut env, row, col, F(fv));
                none
            }
            0x1bf9d8 => {
                rp::set_cell_int_suffix(&mut m, &mut env, row, col, a3, t0);
                none
            }
            0x1bfa08 => {
                rp::set_cell_int(&mut m, &mut env, row, col, a3);
                none
            }
            0x1bfa30 => {
                rp::set_cell_text(&mut m, &mut env, row, col, a3);
                none
            }
            0x1bfa50 => {
                rp::set_cell_ascii(&mut m, &mut env, row, col, a3);
                none
            }
            0x1bfa70 | 0x1bfa90 => rp::cell(row, col),
            0x1bfab0 => {
                rp::set_row_name(&mut m, &mut env, row, a3);
                none
            }
            0x1bfac8 => {
                rp::set_row_name_ascii(&mut m, &mut env, row, a3);
                none
            }
            0x1bfae0 | 0x1bfaf8 => rp::row_name(row),
            0x1bfb10 => {
                rp::set_title(&mut m, &mut env, a3);
                none
            }
            0x1bfb20 => {
                rp::set_title_ascii(&mut m, &mut env, a3);
                none
            }
            0x1aae10 => {
                rp::set_row_layout(&mut m, PAGE, F(fx), F(fw));
                none
            }
            0x1aae88 => {
                rp::clear_text(&mut m, &mut env);
                none
            }
            0x1af720 => {
                rp::build_pages(&mut m, &mut env, THIS, SCRATCH);
                none
            }
            0x1af940 => {
                rp::clear_speech(&mut m, &mut env);
                none
            }
            0x1af950 => {
                rp::career_page(&mut m, &mut env, THIS, SCRATCH);
                none
            }
            0x1b1ba8 => {
                rp::medal_verdict(&mut m, &mut env, THIS);
                none
            }
            _ => unreachable!(),
        };
        if matches!(func, 0x1af720 | 0x1af950) {
            rows += env.log.iter().filter(|c| c.0 == rp::FORMAT_TIME).count();
            verdicts += env.log.iter().filter(|c| c.0 == rp::MEDAL_RANK).count();
        }
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: [u32; 5] = match func {
            0x1bfb10 | 0x1bfb20 => [PAGE, a3, 0, 0, 0],
            0x1bfab0 | 0x1bfac8 | 0x1bfae0 | 0x1bfaf8 => [PAGE, row, a3, 0, 0],
            0x1aae10 | 0x1aae88 => [PAGE, 0, 0, 0, 0],
            0x1af720 | 0x1af950 | 0x1b1ba8 => [THIS, 0, 0, 0, 0],
            0x1af940 => [0; 5],
            _ => [PAGE, row, col, a3, t0],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        if func == 0x1aae10 {
            r.cpu.set_f(12, fx);
            r.cpu.set_f(13, fw);
        } else {
            r.cpu.set_f(12, fv);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x} mode {mode}: the calls");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x} mode {mode}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{rows} finishers' rows, {verdicts} verdicts");
    assert!(rows > 1000 && verdicts > 100);
}

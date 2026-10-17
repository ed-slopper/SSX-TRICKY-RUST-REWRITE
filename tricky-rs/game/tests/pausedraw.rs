//! `tricky_game::pausedraw` (board row G9a3) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the tables at 0x38e300.., as the
//! ELF loads them) with the overlay, its backdrop and fonts, the app, the race, the renderer and the music system
//! in the arena, their fields made random where the functions read them (both pages' items, the trick shown, the
//! players). Then one of the row's functions runs on both sides. Every callee is a stand-in on both sides
//! answering from the case's seed (the clear done for real); the draws log their float arguments and the drawer's
//! colour, the menu list its two arrays. The runner's stack is cleared before each case and a buffer on it logs as
//! one token. The calls, the answers and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pausedraw as pd;
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
const MUSIC_AT: (u32, u32) = (0x343000, 0x100);
const TABLES: (u32, u32) = (0x38e300, 0x1000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x6000;
const THIS: u32 = ARENA;
const HUD: u32 = ARENA + 0x1000;
const MENU_FONT: u32 = ARENA + 0x1200;
const TITLE_FONT: u32 = ARENA + 0x1300;
const APPOBJ: u32 = ARENA + 0x1400;
const RACE: u32 = ARENA + 0x1500;
const PLAYERS: u32 = ARENA + 0x1600;
const RENDER: u32 = ARENA + 0x1700;
const RVT: u32 = ARENA + 0x1800;
const RECT: u32 = ARENA + 0x1900;
const RIDER: u32 = ARENA + 0x1a00;
const SYS: u32 = ARENA + 0x2400;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// The renderer's virtuals' stand-ins
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
const ROFFS: [u32; 3] = [0x1d0, 0x1e0, 0x1f8];

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
fn mapped(a: u32, n: u32) -> bool {
    [DATA, MUSIC_AT, TABLES, (ARENA, SIZE)].iter().any(|&(at, s)| a >= at && a as u64 + n as u64 <= (at + s) as u64)
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
    if func & 0xffff_0000 == 0x01ff_0000 {
        return match func & 0xffff {
            0x1f8 => (3, 0),
            _ => (2, 0),
        };
    }
    match func {
        pd::LOC | pd::SET_COLOUR | pd::WSTR_COPY | pd::TRICK_BOOK | pd::PLAYER_PROFILE | pd::TRICK_DONE => (2, 0),
        pd::FIND_RIDER | pd::TRICK_GET_INFO | pd::FORMAT_TRICK_NAME | pd::GET_RECT | pd::MUSIC_SELECT => (2, 0),
        pd::SAVE_INIT => (2, 0),
        pd::MEMSET | pd::WSTR_FORMAT => (3, 0),
        pd::STRLEN | pd::COMPLETE_PAGES | pd::PAGE_FIRST | pd::TRICK_INFO_NEW | pd::MUSIC_COUNT | pd::MUSIC_NEXT => (1, 0),
        pd::BACKDROP_INIT | pd::BUILD_MAIN_PAGE => (1, 0),
        pd::DRAW_MENU_LIST => (10, 3),
        pd::LABEL_BOX => (3, 1),
        pd::DRAW_SPRITE => (3, 4),
        pd::PRINTF_SHADOWED | pd::PRINTF_SHADOWED_CENTRED | pd::PRINT_SHADOWED_CENTRED | pd::PRINTF_SHADOWED_A => (2, 3),
        pd::PRINT_SHADOWED_RIGHT | pd::PRINT_ASCII_SHADOWED_RIGHT => (2, 3),
        pd::MEASURE_WIDE | pd::MEASURE_ASCII => (3, 1),
        pd::TRICK_INFO_SET => (4, 0),
        pd::TRICK_INFO_DRAW => (2, 2),
        pd::SPRITE_TILED => (2, 6),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    if func == pd::SET_COLOUR {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    // the draws: the drawer's colour as it stands when they draw
    if matches!(
        func,
        pd::DRAW_MENU_LIST | pd::LABEL_BOX | pd::DRAW_SPRITE | pd::PRINTF_SHADOWED | pd::PRINTF_SHADOWED_CENTRED
            | pd::PRINT_SHADOWED_CENTRED | pd::PRINTF_SHADOWED_A | pd::PRINT_SHADOWED_RIGHT
            | pd::PRINT_ASCII_SHADOWED_RIGHT | pd::SPRITE_TILED | pd::TRICK_INFO_DRAW
    ) {
        log.extend((0..4).map(|k| io.r32(pd::DRAW_COLOUR + 4 * k)));
    }
    // the menu list's labels and "on" words (the count of them)
    if func == pd::DRAW_MENU_LIST {
        let n = w[4].min(16);
        log.extend((0..n).map(|k| io.r32(w[3] + 4 * k)));
        log.extend((0..n).map(|k| io.r32(w[9] + 4 * k)));
    }
    let r = ans.next();
    let answer = match func {
        pd::COMPLETE_PAGES => r % 8,
        pd::PLAYER_PROFILE => r % 12,
        // (the trick's name: past 49 characters it is smaller)
        pd::STRLEN => [r % 50, 49, 50, 50 + r % 30][(r >> 8) as usize % 4],
        pd::PAGE_FIRST => r % 30,
        pd::TRICK_DONE => r % 2,
        pd::GET_RECT => RECT + 0x10 * (r % 4),
        pd::FIND_RIDER => RIDER,
        pd::MEASURE_WIDE | pd::MEASURE_ASCII => r % 300,
        // (no songs, or −1, sometimes)
        pd::MUSIC_COUNT => [0u32, 0xffff_ffff, 1, 2, 5][(r % 5) as usize],
        pd::MEMSET => {
            if mapped(w[0], w[2]) {
                for k in 0..w[2] / 4 {
                    io.w32(w[0] + 4 * k, (w[1] & 0xff) * 0x0101_0101);
                }
            }
            0
        }
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
        w.resize(10, 0);
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

#[test]
fn pause_menu_draws_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [0x1ab9f8u32, 0x1ac7e0, 0x1ad198, 0x1ad628];
    for f in funcs {
        r.allow(f);
    }
    let mut stubs = vec![
        pd::LOC, pd::SET_COLOUR, pd::MEMSET, pd::DRAW_MENU_LIST, pd::LABEL_BOX, pd::DRAW_SPRITE, pd::PRINTF_SHADOWED,
        pd::PRINTF_SHADOWED_CENTRED, pd::PRINT_SHADOWED_CENTRED, pd::PRINTF_SHADOWED_A, pd::PRINT_SHADOWED_RIGHT,
        pd::PRINT_ASCII_SHADOWED_RIGHT, pd::MEASURE_WIDE, pd::MEASURE_ASCII, pd::WSTR_FORMAT, pd::WSTR_COPY, pd::STRLEN,
        pd::TRICK_BOOK, pd::COMPLETE_PAGES, pd::PLAYER_PROFILE, pd::PAGE_FIRST, pd::TRICK_DONE, pd::FIND_RIDER,
        pd::TRICK_GET_INFO, pd::FORMAT_TRICK_NAME, pd::TRICK_INFO_SET, pd::TRICK_INFO_DRAW, pd::TRICK_INFO_NEW,
        pd::GET_RECT, pd::SPRITE_TILED, pd::MUSIC_COUNT, pd::MUSIC_SELECT, pd::MUSIC_NEXT, pd::BACKDROP_INIT,
        pd::SAVE_INIT, pd::BUILD_MAIN_PAGE,
    ];
    stubs.extend(ROFFS.map(rv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let mut w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            // (the menu list's ninth and tenth arguments, on the stack)
            let sp = c.gpr(29) as u32;
            w.push(mem.read_u32(sp));
            w.push(mem.read_u32(sp + 8));
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
    let tables = read(&mut r, TABLES);
    let mut g = Rng(0x9a05_e000_0000_0001);
    let (mut lists, mut sliders, mut pages) = (0usize, 0usize, 0usize);
    let per = 3000u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (MUSIC_AT.0, vec![0; MUSIC_AT.1 as usize]),
                (TABLES.0, tables.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (its renderer +0x724, strings +0x50, race +0x730), the race (its players +0x298, +0x38 the
        // player's block, +0x1c), the renderer's table, the music system
        m.w32(pd::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x730, RACE);
        m.w32(RACE + 0x298, g.pick(&[0u32, 1, 2, 3, 0xffff_ffff]));
        m.w32(RACE + 0x38, PLAYERS);
        m.w32(PLAYERS + 0xc, g.below(2));
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        m.w32(pd::MUSIC, SYS);
        m.w32(SYS + 0x36e8, g.below(8));
        // the overlay: its backdrop and fonts (the menu font's height), the pages, the trick shown
        m.w32(THIS + 0x42c, HUD);
        m.w32(HUD + 4, MENU_FONT);
        m.w32(HUD + 8, TITLE_FONT);
        m.w32(MENU_FONT + 0xc, g.below(40));
        wf(&mut m, HUD + 0x2c, g.f(0.0, 480.0));
        wf(&mut m, HUD + 0x70, g.f(0.0, 1.0));
        for page in [THIS + 0x438, THIS + 0x570] {
            m.w32(page + 0x18, g.pick(&[0u32, 1, 4, 8, 12, 0xffff_ffff]));
            m.w32(page + 0x14, g.below(13));
            for i in 0..12 {
                let e = page + 0x1c + 0x18 * i;
                let any = g.next();
                m.w32(e, g.pick(&[8u32, 9, 9, 0xb, 0xc, 0xd, 0xe, 0x10, 0x13, 0x15, any]));
                // (small: a camera's value indexes the names)
                m.w32(e + 8, g.pick(&[0u32, 1, 2, 3, 4, 25, 49, 50, 0xffff_ffff]));
                let any = g.next();
                m.w32(e + 0xc, g.pick(&[0u32, 3, 50, any]));
                m.w32(e + 0x10, g.below(2));
            }
        }
        // the career's words the constructor clears and the camera names (zero as the ELF loads them)
        for o in [0x3704, 0x3708, 0x370c] {
            m.w32(pd::CAREER + o, g.next());
        }
        for k in 0..0x40 {
            m.w32(pd::CAMERA_NAMES - 0x80 + 4 * k, g.next());
        }
        m.w32(THIS + 0x7f0, g.below(2));
        m.w32(THIS + 0x7f4, g.below(30));
        for k in 0..4 {
            wf(&mut m, pd::DRAW_COLOUR + 4 * k, g.f(0.0, 1.0));
        }
        let any = g.next();
        let whole = g.pick(&[0u32, 1, any]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let none = u32::MAX;
        let e = &mut env;
        let ours = match func {
            0x1ab9f8 => pd::new(&mut m, e, THIS, whole, SCRATCH),
            0x1ac7e0 => {
                pd::draw_main_page(&mut m, e, THIS, SCRATCH);
                none
            }
            0x1ad198 => {
                pd::build_options_page(&mut m, e, THIS);
                none
            }
            0x1ad628 => {
                pd::draw_options_page(&mut m, e, THIS, SCRATCH);
                none
            }
            _ => unreachable!(),
        };
        lists += env.log.iter().filter(|c| c.0 == pd::DRAW_MENU_LIST && c.1[4] > 0).count();
        sliders += env.log.iter().filter(|c| c.0 == pd::SPRITE_TILED).count();
        pages += env.log.iter().filter(|c| c.0 == pd::TRICK_INFO_DRAW).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x1ab9f8 => vec![THIS, whole],
            _ => vec![THIS],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        // the constructor's table copy on the stack (when not whole): the backdrop's +0x28 points at it on both
        // sides, and its bytes are the same
        if func == 0x1ab9f8 && whole == 0 {
            let theirs = r.mem.read_u32(HUD + 0x28);
            assert!(stack(theirs), "case {case}: the table copy not on the stack ({theirs:#x})");
            assert_eq!(m.r32(HUD + 0x28), SCRATCH);
            let mut th = vec![0u8; 0x98];
            r.mem.read(theirs, &mut th).unwrap();
            let ours: Vec<u8> = (0..0x98).map(|k| m.r8(SCRATCH + k)).collect();
            assert_eq!(th, ours, "case {case}: the table copy");
            m.w32(HUD + 0x28, theirs);
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
    println!("{lists} menu lists, {sliders} sliders, {pages} trick pages");
    assert!(lists > 300 && sliders > 300 && pages > 300);
}

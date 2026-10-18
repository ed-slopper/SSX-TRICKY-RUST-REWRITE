//! `tricky_game::raceoverlays` (board row G8n1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the backdrop's table at
//! 0x38f880, as the ELF loads them) with an overlay and its backdrop in the arena (a dialog's lines and buttons, the
//! count's state made random), the app, its race and its clock, and the renderer's table pointing at stand-ins.
//! Then one of the row's functions runs on both sides. Every callee is a stand-in on both sides answering from the
//! case's seed (the clears done for real); the draws log their float arguments and the drawer's colour. The
//! runner's stack is cleared before each case and a buffer on it logs as one token. The calls, the answers and the
//! memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::raceoverlays as ro;
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
const TABLES: (u32, u32) = (0x38f800, 0x200);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const THIS: u32 = ARENA;
const BACKDROP: u32 = ARENA + 0x800;
const FONT: u32 = ARENA + 0xa00;
const APPOBJ: u32 = ARENA + 0x1000;
const RACE: u32 = ARENA + 0x1800;
const RACE1C: u32 = ARENA + 0x1900;
const RENDER: u32 = ARENA + 0x1a00;
const RVT: u32 = ARENA + 0x1b00;
const RECT: u32 = ARENA + 0x1e00;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
const ROFFS: [u32; 5] = [0x1d0, 0x1d8, 0x1e0, 0x1f0, 0x1f8];

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

/// How many int and float arguments a callee has (`this` counted for a virtual)
fn arity(func: u32) -> (usize, usize) {
    match func {
        f if f == rv(0x1d8) => (1, 0),
        f if f == rv(0x1f0) || f == rv(0x1f8) => (3, 0),
        f if f & 0xffff_0000 == 0x01ff_0000 => (2, 0),
        ro::LOC | ro::SET_COLOUR | ro::GET_RECT | ro::COURSE_NAME => (2, 0),
        ro::MEMSET | ro::OVERLAY_HIDE => (3, 0),
        ro::FILLED_RECT => (1, 4),
        ro::MEASURE_WIDE => (3, 1),
        ro::PRINTF_SHADOWED_A | ro::PRINT_SHADOWED_CENTRED => (2, 3),
        ro::DRAW_SPRITE => (3, 4),
        ro::BACKDROP_FRAME => (2, 1),
        ro::BACKDROP_INIT | ro::OVERLAY_HIDDEN | ro::RACE_PLAYERS => (1, 0),
        ro::COUNT_DIGITS => (3, 4),
        ro::SIN => (0, 1),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    if func == ro::SET_COLOUR {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    if matches!(
        func,
        ro::FILLED_RECT | ro::PRINTF_SHADOWED_A | ro::PRINT_SHADOWED_CENTRED | ro::DRAW_SPRITE | ro::BACKDROP_FRAME | ro::COUNT_DIGITS
    ) {
        log.extend((0..4).map(|k| io.r32(ro::DRAW_COLOUR + 4 * k)));
    }
    let r = ans.next();
    let answer = match func {
        ro::MEASURE_WIDE => r % 300,
        ro::GET_RECT => RECT,
        ro::OVERLAY_HIDDEN => [0u32, 0, 1, r][(r % 4) as usize],
        ro::RACE_PLAYERS => [1u32, 2, 1, 3][(r % 4) as usize],
        ro::MEMSET => {
            for k in 0..w[2] / 4 {
                io.w32(w[0] + 4 * k, (w[1] & 0xff) * 0x0101_0101);
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

const FUNCS: [u32; 12] = [
    0x1aa268, 0x1aacb0, 0x1aae10, 0x1aae88, 0x1aaec0, 0x1ab028, 0x1ae350, 0x1ae3b8, 0x1ae4b0, 0x1ae650, 0x1ae660, 0x1ae7a0,
];

#[test]
fn race_overlays_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    let mut stubs = vec![
        ro::LOC, ro::SET_COLOUR, ro::MEMSET, ro::FILLED_RECT, ro::MEASURE_WIDE, ro::PRINTF_SHADOWED_A,
        ro::PRINT_SHADOWED_CENTRED, ro::DRAW_SPRITE, ro::BACKDROP_FRAME, ro::BACKDROP_INIT, ro::GET_RECT, ro::COURSE_NAME,
        ro::OVERLAY_HIDDEN, ro::OVERLAY_HIDE, ro::RACE_PLAYERS, ro::COUNT_DIGITS, ro::SIN,
    ];
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
    let tables = read(&mut r, TABLES);
    let mut g = Rng(0x8a7e_0000_0000_0001);
    let (mut buttons, mut gos, mut digits) = (0usize, 0usize, 0usize);
    let per = 1500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (TABLES.0, tables.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (strings +0x50, a font +0x60, the renderer +0x724, the race +0x730), the race (its +0x1c with the
        // clock +0x14 and the state +0x1c, +0x38), the renderer's table, the career's course and mode
        m.w32(ro::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x730, RACE);
        m.w32(RACE + 0x1c, RACE1C);
        let c = g.f(0.0, 400.0);
        let clock = g.pick(&[c, 0.0, 59.0, 60.0, 61.0, 180.0, -1.0]);
        wf(&mut m, RACE1C + 0x14, clock);
        m.w32(RACE1C + 0x1c, g.pick(&[3u32, 3, 4, 2]));
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        m.w32(ro::CAREER + 0x2af8, g.below(12));
        m.w32(ro::CAREER + 0x2afc, g.pick(&[3u32, 3, 1, 2, 7]));
        // the overlay: its backdrop (fonts, alpha), a dialog's fields, the count
        m.w32(THIS, BACKDROP);
        m.w32(THIS + 4, FONT);
        m.w32(THIS + 8, FONT + 0x40);
        m.w32(BACKDROP + 4, FONT);
        m.w32(BACKDROP + 8, FONT + 0x40);
        wf(&mut m, BACKDROP + 0x70, g.f(0.0, 1.0));
        m.w32(FONT + 0xc, g.pick(&[0u32, 12, 22, 30]));
        for o in [0x328, 0x32c, 0x330, 0x334, 0x338] {
            let any = g.next();
            m.w32(THIS + o, g.pick(&[0u32, any, any]));
        }
        wf(&mut m, THIS + 0x33c, g.f(0.0, 500.0));
        wf(&mut m, THIS + 0x340, g.f(0.0, 300.0));
        wf(&mut m, THIS + 0x344, g.f(0.0, 200.0));
        let c = g.below(200);
        m.w32(THIS + 0x2c, g.pick(&[0u32, 1, 45, 90, 0xffff_ffff, c]));
        m.w32(THIS + 0x30, g.pick(&[0u32, 0, 1, 3]));
        let c = g.f(0.0, 1.0);
        wf(&mut m, THIS + 0x34, g.pick(&[c, 0.5, 0.25, 0.75, 1.0]));
        for k in 0..4 {
            wf(&mut m, ro::DRAW_COLOUR + 4 * k, g.f(0.0, 1.0));
        }
        let any = g.next();
        let whole = g.pick(&[0u32, 1, any]);
        let (f1, f2) = (g.f(0.0, 480.0), g.f(-5.0, 30.0));
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
            0x1aa268 => { ro::dialog_draw(mm, e, THIS); none }
            0x1aacb0 => ro::results_init(mm, e, THIS, whole, SCRATCH),
            0x1aae10 => { ro::title_rows(mm, THIS, F(f1), F(f2)); none }
            0x1aae88 => { ro::clear_buffers(mm, e); none }
            0x1aaec0 => { ro::draw_course_title(mm, e, THIS, F(f1), SCRATCH); none }
            0x1ab028 => { ro::draw_divider(mm, e, THIS, SCRATCH); none }
            0x1ae350 => { ro::countdown_reset(mm, THIS); none }
            0x1ae3b8 => { ro::countdown_update(mm, e, THIS); none }
            0x1ae4b0 => { ro::countdown_draw(mm, e, THIS); none }
            0x1ae650 => { ro::pop_async_update(mm, THIS); none }
            0x1ae660 => { ro::pop_async_draw(mm, e, THIS); none }
            0x1ae7a0 => { ro::pop_async_draw_text(); none }
            _ => unreachable!(),
        };
        buttons += env.log.iter().filter(|c| c.0 == ro::DRAW_SPRITE).count();
        gos += env.log.iter().filter(|c| c.0 == ro::PRINT_SHADOWED_CENTRED).count();
        digits += env.log.iter().filter(|c| c.0 == ro::COUNT_DIGITS).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x1aacb0 => vec![THIS, whole],
            _ => vec![THIS],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f1);
        r.cpu.set_f(13, f2);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        // the results overlay's table copy on the stack (when not whole): the backdrop's +0x28 points at it on
        // both sides, and its bytes are the same
        if func == 0x1aacb0 && whole == 0 {
            let theirs = r.mem.read_u32(BACKDROP + 0x28);
            assert!(stack(theirs), "case {case}: the table copy not on the stack ({theirs:#x})");
            assert_eq!(m.r32(BACKDROP + 0x28), SCRATCH);
            let mut th = vec![0u8; 0x98];
            r.mem.read(theirs, &mut th).unwrap();
            let ours: Vec<u8> = (0..0x98).map(|k| m.r8(SCRATCH + k)).collect();
            assert_eq!(th, ours, "case {case}: the table copy");
            m.w32(BACKDROP + 0x28, theirs);
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
    println!("{buttons} buttons, {gos} GOs, {digits} counts drawn");
    assert!(buttons > 1000 && gos > 200 && digits > 200);
}

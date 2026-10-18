//! `tricky_game::fe_mcard` (board row G9y4) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random memory card overlay in the arena: its operation, quiet flag, slots, the
//! manager's state, the card (its table at +0 pointing at stand-ins), the front end state and its root group, the
//! dialog, message and prompt (text widgets whose own code runs for real), and the career's load block and sound
//! settings. One of the row's functions then runs on both sides. The memory card manager, the front end state's
//! own code, the career's and every other callee are stand-ins on both sides answering from the case's seed. The
//! calls are compared with their arguments (the floats from f12), the vectors and texts they are given (read by
//! value), the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_mcard as mc;
use tricky_game::fe_venuesel::LOC_STRING;
use tricky_game::fe_wentry as we;
use tricky_game::fe_wtext as wt;
use tricky_game::pf::Mem;

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

const DATA: (u32, u32) = (0x337c00, 0x400);
const CAREER: (u32, u32) = (0x32b000, 0x4000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const O: u32 = ARENA;
const VT: u32 = ARENA + 0x3000;
const CARDVT: u32 = ARENA + 0x3200;
const CARD: u32 = ARENA + 0x3400;
const W0: u32 = ARENA + 0x3800;
const NW: u32 = 8;
/// the buttons' labels: objects of their own (a button's layout clears its label's +0x114)
const LABELS: u32 = ARENA + 0x6400;
fn label(k: u32) -> u32 {
    LABELS + 0x100 * k
}
/// the buttons' icons (sprites: placing one sets its +0x110 and +0x114), also the texts' fonts
fn icon(k: u32) -> u32 {
    LABELS + 0x400 + 0x100 * k
}
const APPOBJ: u32 = ARENA + 0x5000;
const RENDER: u32 = ARENA + 0x5080;
const RVT: u32 = ARENA + 0x5100;
const FE: u32 = ARENA + 0x5300;
const SND: u32 = ARENA + 0x5380;
const SVT: u32 = ARENA + 0x53c0;
const FONT: u32 = ARENA + 0x5500;
const STRS: u32 = ARENA + 0x5600;
const BLOCK: u32 = ARENA + 0x5800;
const SCRATCH: u32 = 0x01f8_0000;
/// the career's load block (its bytes at +0x920 in the arena) and its header 0x7fff8 on
// (the career's +0x36fc is the load block: one word)
const BASE: u32 = BLOCK;
const HEAD: (u32, u32) = ((BASE + 0x7fff8) & !0xff, 0x100);
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn cv(off: u32) -> u32 {
    0x0181_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0182_0000 + off
}
const SND_CALL: u32 = 0x0183_0138;
/// the front end state's own table (+0x438): its +0x28 and +0x40 take only the state (what else the registers hold
/// is left by earlier calls)
const STV: u32 = ARENA + 0x6c00;
fn sv(off: u32) -> u32 {
    0x0184_0000 + off
}
fn wdg(k: u32) -> u32 {
    W0 + 0x200 * k
}
const SLOTS: [u32; 12] = [0x28, 0x40, 0x50, 0x70, 0xf0, 0x100, 0x130, 0x140, 0x148, 0x150, 0x160, 0x168];
const CARD_SLOTS: [u32; 10] = [0x70, 0x160, 0x170, 0x178, 0x190, 0x1a0, 0x1b0, 0x1c0, 0x1c8, 0x1e0];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
}
fn stack(a: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&a)
}
fn token(a: u32) -> u32 {
    if stack(a) { STACK_TOKEN } else { a }
}
fn readable(a: u32) -> bool {
    stack(a) || (ARENA..ARENA + SIZE - 0x40).contains(&a)
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

fn arity(f: u32) -> (usize, usize) {
    match f {
        wt::TB_SET_RECT => (1, 4),
        wt::TB_SET_LINE_HEIGHT => (1, 1),
        mc::MGR_BEGIN | mc::MGR_SLOT_LABEL => (4, 0),
        mc::MGR_READ | mc::MGR_TEXT | mc::FILL | mc::COPY | mc::MEMCMP => (3, 0),
        mc::MGR_CTOR => (3, 0),
        f if f == cv(0x1e0) => (4, 0),
        f if f == cv(0x70) => (3, 0),
        f if f == cv(0x1c0) || f == cv(0x1c8) || f == cv(0x1b0) => (2, 0),
        f if (0x0181_0000..0x0181_1000).contains(&f) => (1, 0),
        f if f == slot(0x140) => (1, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (2, 0),
        mc::MGR_STEP | mc::MGR_SAVE | mc::MGR_LOAD | mc::MGR_SAVE_SET | mc::MGR_LOAD_SET | mc::MGR_DTOR
        | mc::STATE_DTOR | mc::STATE_BUTTONS | mc::STATE_BUTTON | mc::STATE_SHOW_GROUP | mc::SUM | mc::FIND | mc::NTH
        | mc::WSTR_COPY | mc::RENDER_GET | mc::RENDER_SET | mc::CAREER_KEEP | LOC_STRING | wt::TB_APPEND
        | wt::TB_SET_FONT | wt::TB_WRAP | we::SET_WORLD_POS | we::GET_RECT | SND_CALL => (2, 0),
        f if f == rv(0x120) => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    let deref = |io: &mut dyn Io, a: u32, k: u32| -> Vec<u32> { (0..k).map(|i| token(io.r32(a + 4 * i))).collect() };
    // (a vector, colour or text by value; not a buffer only written)
    let out_only = f == slot(0x130) || f == cv(0x70) || f == cv(0x1e0) || f == mc::MGR_TEXT || f == mc::MGR_SLOT_LABEL
        || f == mc::RENDER_GET || f == mc::FILL;
    if ni >= 2 && !out_only && readable(w[1]) {
        log.extend(deref(io, w[1], 4));
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let wide = |io: &mut dyn Io, at: u32, g: &mut Rng| {
        for k in 0..8 {
            let v = g.pick(&[0x0041_0042u32, 0x0020_0043, 0]);
            io.w32(at + 4 * k, v);
        }
        io.w32(at + 0x20, 0);
    };
    // (the colour set given its vector back scribbled: what is handed on next must be written again)
    if f == slot(0x50) && stack(w[1]) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.next());
        }
    }
    if f == slot(0x130) {
        for k in 0..4 {
            io.w32(w[1] + 4 * k, g.f(0.0, 200.0).to_bits());
        }
    }
    if f == cv(0x70) || f == cv(0x1e0) || f == mc::MGR_TEXT || f == mc::MGR_SLOT_LABEL {
        wide(io, if f == mc::MGR_TEXT { w[1] } else { w[2] }, &mut g);
    }
    if f == cv(0x1b0) {
        wide(io, w[1], &mut g);
    }
    if f == mc::RENDER_GET {
        for k in 0..4 {
            io.w32(w[0] + 4 * k, g.f(0.0, 1.0).to_bits());
        }
    }
    let answer = match f {
        mc::FIND | mc::NTH | mc::STATE_BUTTON => wdg(r % NW),
        LOC_STRING => STRS + 0x40 * (r % 3),
        mc::MEMCMP => [0u32, 0, 1][(r % 3) as usize],
        mc::SUM => [0x1234u32, r][(r % 2) as usize],
        mc::WSTR_COUNT => r % 8,
        wt::TB_MAX_WIDTH | wt::TB_HEIGHT => r % 300,
        we::GET_RECT => ARENA + 0x5700,
        f if (0x0181_0000..0x0181_1000).contains(&f) => r % 2,
        mc::INPUT_ENTER | mc::INPUT_ESCAPE | mc::INPUT_ACCEPT | mc::INPUT_BACK | mc::INPUT_UP | mc::INPUT_DOWN
        | mc::INPUT_KEY63 => (r % 4 == 0) as u32,
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
        fl.resize(4, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 37] = [
    0x28f260, 0x28f380, 0x290de0, 0x290fd0, 0x291178, 0x291198, 0x291240, 0x2912e8, 0x291450, 0x291470, 0x291498,
    0x2914a0, 0x2914a8, 0x2914d0, 0x291648, 0x291688, 0x2916c8, 0x2917b0, 0x291da8, 0x291e00, 0x2923b8, 0x292428,
    0x2924d0, 0x292638, 0x2927a8, 0x2927d8, 0x292810, 0x2928c0, 0x292918, 0x292940, 0x292968, 0x292c70, 0x292d40,
    0x292d80, 0x292e70, 0x2930a8, 0x293210,
];
/// the widgets' code they call, run as the original: the wrapped text's setters and scale, the button layout and
/// the sprite rectangle, the placing
const ALSO: [u32; 9] = [0x2b0f18, 0x2b0f68, 0x2b1008, 0x2b1070, 0x2b4cc0, 0x2ae098, 0x2ac4b0, 0x2addb0, 0x293218];

#[test]
fn memory_card_overlay_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS.iter().chain(ALSO.iter()) {
        r.allow(*f);
    }
    let mut stubs = vec![
        mc::MGR_CTOR, mc::MGR_DTOR, mc::MGR_START, mc::MGR_END, mc::MGR_SAVE_SET, mc::MGR_SAVE, mc::MGR_LOAD_SET,
        mc::MGR_LOAD, mc::MGR_BEGIN, mc::MGR_STEP, mc::MGR_DRAW, mc::MGR_READ, mc::MGR_FINISH, mc::MGR_TEXT,
        mc::MGR_SLOT_LABEL, mc::MGR_NEXT, mc::MGR_PREV, mc::MGR_SLOT_NEXT, mc::MGR_SLOT_PREV, mc::MGR_CLEAR,
        mc::MGR_SHOW, mc::STATE_SETUP, mc::STATE_DTOR, mc::STATE_BUTTONS, mc::STATE_BUTTON, mc::STATE_SHOW_GROUP,
        mc::CAREER_SAVE_BEGIN, mc::CAREER_SAVE_END, mc::OPTIONS_SAVE, mc::CAREER_LOAD, mc::CAREER_KEEP, mc::SUM,
        mc::COPY, mc::FILL, mc::FIND, mc::NTH, mc::WSTR_COUNT, mc::WSTR_COPY, mc::MEMCMP, mc::RENDER_GET,
        mc::RENDER_SET, mc::FE_MUSIC, mc::INPUT_ENTER, mc::INPUT_ESCAPE, mc::INPUT_ACCEPT, mc::INPUT_BACK,
        mc::INPUT_UP, mc::INPUT_DOWN, mc::INPUT_KEY63, LOC_STRING, wt::TB_CLEAR, wt::TB_SET_FONT, wt::TB_SET_RECT,
        wt::TB_APPEND, wt::TB_SET_LINE_HEIGHT, we::SET_WORLD_POS, we::GET_RECT, rv(0x120), SND_CALL,
    ];
    stubs.extend(SLOTS.map(slot));
    stubs.extend(CARD_SLOTS.map(cv));
    stubs.extend([sv(0x28), sv(0x40)]);
    stubs.extend([slot(0x28 + 0x100), slot(0xf8), slot(0x138)]);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let data = read(&mut r, DATA);
    let mut g = Rng(0x9e21_0000_0000_0001);
    let per = 1500u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut career = vec![0u8; CAREER.1 as usize];
        for b in career.iter_mut() {
            *b = g.next() as u8;
        }
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (CAREER.0, career),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x800]),
                (HEAD.0, vec![0; HEAD.1 as usize]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x734, FE);
        m.w32(APPOBJ + 0x60, FONT);
        // (heights past 2^24 too, where the float is rounded toward zero)
        m.w32(FONT + 0xc, g.pick(&[12u32, 16, 20, 0x0100_0003, 0x0300_0007, 0xfd00_0005]));
        m.w32(RENDER + 0x10, RVT);
        m.w16(RVT + 0x120, 0);
        m.w32(RVT + 0x124, rv(0x120));
        m.w32(FE + 0x74, ARENA + 0x5400);
        m.w32(FE + 0x78, ARENA + 0x5440);
        m.w8(FE + 5, g.pick(&[1u8, 2]));
        m.w32(FE + 0xa4, SND);
        m.w32(SND, SVT);
        m.w16(SVT + 0x138, 0);
        m.w32(SVT + 0x13c, SND_CALL);
        for off in SLOTS.iter().chain([0xf8u32, 0x138].iter()) {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(*off));
        }
        for off in CARD_SLOTS {
            m.w16(CARDVT + off, g.pick(&[0u16, 8]));
            m.w32(CARDVT + off + 4, cv(off));
        }
        m.w32(CARD, CARDVT);
        for k in 0..NW {
            let w = wdg(k);
            m.w32(w + 0xc, VT);
            m.w32(w + 0x14, g.below(2));
            // (a text's font and a button's icon: an icon, as the font is only handed on)
            m.w32(w + 0x110, icon(g.below(4)));
            m.w32(w + 0x114, label(g.below(4)));
            for i in 0..8 {
                m.w32(w + 0x80 + 4 * i, g.f(-100.0, 400.0).to_bits());
            }
            for i in 0..4 {
                m.w32(w + 0x100 + 4 * i, g.f(0.0, 300.0).to_bits());
            }
        }
        for o in (0..4).map(label).chain((0..4).map(icon)) {
            m.w32(o + 0xc, VT);
            for i in 0..4 {
                m.w32(o + 0x80 + 4 * i, g.f(-100.0, 400.0).to_bits());
            }
        }
        for k in 0..3 {
            let s = STRS + 0x40 * k;
            for i in 0..8 {
                m.w32(s + 4 * i, g.pick(&[0x0041_0042u32, 0x0020_0043]));
            }
            m.w32(s + 4 * g.below(8), 0);
            m.w32(s + 0x3c, 0);
        }
        // the overlay
        m.w32(O, g.pick(&[0u32, 1, 1, 2]));
        m.w32(O + 4, g.pick(&[0u32, 1, 2]));
        m.w32(O + 8, g.below(2));
        m.w32(O + 0x38, g.below(4));
        m.w32(O + 0x54, g.below(36));
        m.w32(O + 0x21c, g.below(4));
        m.w32(O + 0x224, CARD);
        m.w32(O + 0x428, VT);
        m.w32(O + 0x438, STV);
        for off in [0x28u32, 0x40] {
            m.w16(STV + off, g.pick(&[0u16, 4]));
            m.w32(STV + off + 4, sv(off));
        }
        m.w32(O + 0x464, wdg(0));
        m.w32(O + 0x488, g.below(2));
        // (state 1's career kept, options put back, or neither; the brightness either side of its clamp)
        let done = g.below(3);
        m.w32(O + 0x498, (done == 0) as u32);
        m.w32(O + 0x49c, (done == 1) as u32);
        // (the sound mode handed on: a small number, never an address)
        m.w32(mc::CAREER + 8 + 0x1668, g.below(4));
        m.w32(mc::CAREER + 0x16b4, g.pick(&[-0.75f32, -0.3, 0.2, 1.0, 1.4, 1.75, 3.0]).to_bits());
        m.w32(O + 0x4a0, BLOCK);
        m.w32(O + 0x2c78, wdg(1));
        m.w32(O + 0x2c7c, wdg(2));
        m.w32(O + 0x2c80, wdg(3));
        for k in 0..4 {
            m.w32(O + k * 0x58 + 0x174, g.below(2));
            m.w32(O + k * 0x58 + 0x168, g.below(2));
        }
        // (the card after the slots: slot 2's +0x174 is the card's word)
        m.w32(O + 0x224, CARD);
        // the career's load block: a few bytes and their sum from 0xdeadbeef, right or not
        let base = BASE;
        m.w32(mc::CAREER + 0x36fc, base);
        let n = g.pick(&[0u32, 3, 16]);
        let head = base.wrapping_add(0x7fff8);
        let mut sum = 0xdead_beefu32;
        for i in 0..n {
            sum = sum.wrapping_add(m.r8(base + 0x920 + i) as i8 as i32 as u32);
        }
        let hv = g.pick(&[sum, sum, sum ^ 1]);
        m.w32(head, n);
        m.w32(head + 4, hv);
        m.w32(O + 0x2c70, g.pick(&[0x1234u32, 7]));
        m.w32(BLOCK + 0x334, g.pick(&[0x1234u32, 7]));
        let any = g.next() | 0x8000_0000;
        let a1 = g.pick(&[any, 0, 1, 2, 3, 0x4001]);
        let (a2, a3) = (g.next() | 0x8000_0000, g.pick(&[0u32, 3, 0x10, 0x22, 0xff]));
        let flags = g.pick(&[0u32, 1, 2, 3]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        let none = u32::MAX;
        let s = SCRATCH;
        let ours = match func {
            0x28f260 => mc::ctor(mm, e, O, a1 & 3, flags & 1),
            0x28f380 => mc::dtor(mm, e, O, flags),
            0x290de0 => mc::pad_buttons(mm, e, O),
            0x290fd0 => mc::draw(mm, e, O),
            0x291178 => { mc::career_save_mark(mm, e, O, false); none }
            0x291450 => { mc::career_save_mark(mm, e, O, true); none }
            0x291198 => mc::begin(mm, e, O),
            0x291240 => mc::read_more(mm, e, O, a1),
            0x2912e8 => mc::finish(mm, e, O),
            0x291470 => mc::vf3(mm, e, O),
            0x291498 => mc::zero(),
            0x2914a0 | 0x293210 => { mc::nothing(); none }
            0x2914a8 => mc::vf5(mm, e, O),
            0x2914d0 => { mc::slot_line(mm, e, O, a1 & 3, s); none }
            0x291648 => { mc::step(mm, e, O, true); none }
            0x291688 => { mc::step(mm, e, O, false); none }
            0x2916c8 => { mc::vf49(mm, e, O); none }
            0x2917b0 => { mc::on_state(mm, e, O, a1, s); none }
            0x291da8 => { mc::show_loc_dialog(mm, e, O, a1 & 0x1ff, a2, a3, s); none }
            0x291e00 => { mc::show_dialog(mm, e, O, STRS, a2, a3, s); none }
            0x2923b8 => { mc::hide_dialog(mm, e, O); none }
            0x292428 => { mc::show_message_widgets(mm, e, O, a1); none }
            0x2924d0 => { mc::enable_buttons(mm, e, O, a1); none }
            0x292638 => { mc::show_prompt(mm, e, O, s); none }
            0x2927a8 => { mc::hide_prompt(mm, e, O); none }
            0x2927d8 => { mc::show_loc_message(mm, e, O, a1 & 0x1ff); none }
            0x292810 => { mc::show_message(mm, e, O, STRS + 0x40); none }
            0x2928c0 => { mc::hide_message(mm, e, O); none }
            0x292918 => { mc::slot_step(mm, e, O, true, s); none }
            0x292940 => { mc::slot_step(mm, e, O, false, s); none }
            0x292968 => { mc::fill_slot_list(mm, e, O, s); none }
            0x292c70 => { mc::vf1(mm, e, O); none }
            0x292d40 => { mc::vf55(mm, e, O); none }
            0x292d80 => { mc::show_empty_slots(mm, e, O); none }
            0x292e70 => { mc::layout_message(mm, e, O, s); none }
            0x2930a8 => { mc::hide_info_groups(mm, e, O); none }
            _ => unreachable!(),
        };
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x28f260 => vec![O, a1 & 3, flags & 1],
            0x28f380 => vec![O, flags],
            0x2914d0 => vec![O, a1 & 3],
            0x291da8 => vec![O, a1 & 0x1ff, a2, a3],
            0x2927d8 => vec![O, a1 & 0x1ff],
            0x291e00 => vec![O, STRS, a2, a3],
            0x292810 => vec![O, STRS + 0x40],
            _ => vec![O, a1],
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
}

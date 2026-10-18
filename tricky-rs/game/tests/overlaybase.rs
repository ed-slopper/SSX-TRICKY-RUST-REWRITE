//! `tricky_game::overlaybase` (board row G8n6) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the overlays' tables at
//! 0x38e400.., as the ELF loads them) with the overlay objects in the arena made random (the backdrop inside the
//! object or apart), then one of the row's functions runs on both sides. The callees outside the row are stand-ins
//! on both sides; the results overlay's init and the backdrop's and screen overlay's destructors run for real.
//! The runner's stack is cleared before each case; the tables a destructor or constructor copies to its frame are
//! compared byte for byte with the port's (each pointer to the frame then taken as the runner's), then the calls,
//! the answers and the memory.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::overlaybase as ob;
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
const TABLES: (u32, u32) = (0x38e400, 0x1900);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const THIS: u32 = ARENA;
const APART: u32 = ARENA + 0x1000;
const RES: u32 = ARENA + 0x1400;
const CARD: u32 = ARENA + 0x1800;
const CARD_B: u32 = ARENA + 0x2000;
const APPOBJ: u32 = ARENA + 0x2400;
const RENDER: u32 = ARENA + 0x2500;
const RVT: u32 = ARENA + 0x2600;
const PADS: u32 = ARENA + 0x2700;
const SCREEN_VT: u32 = ARENA + 0x2800;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
const ROFFS: [u32; 2] = [0x1e0, 0x1f0];
const SCREEN_VF: u32 = 0x0180_0078;

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

fn arity(func: u32) -> (usize, usize) {
    match func {
        f if f == rv(0x1f0) => (3, 0),
        f if f & 0xffff_0000 == 0x01ff_0000 => (2, 0),
        SCREEN_VF | ob::BACKDROP_INIT | ob::SCREEN_INIT | ob::DELETE | ob::SAVE_TO_CARD => (1, 0),
        ob::SAVE_INIT | ob::MC_DTOR | ob::ANIM_DTOR | ob::MEMBER_DTOR | ob::LOC | ob::WSTR_SET_ASCII | ob::SET_COLOUR => (2, 0),
        ob::FILLED_RECT => (1, 4),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    if func == ob::SET_COLOUR {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    if func == ob::FILLED_RECT {
        log.extend((0..4).map(|k| io.r32(ob::DRAW_COLOUR + 4 * k)));
    }
    (ans.next(), Call(func, log))
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

const FUNCS: [u32; 75] = [
    0x1be830, 0x1bf558, 0x1bf560, 0x1bf568, 0x1bf578, 0x1c0040, 0x1c0048, 0x1c0050, 0x1c0058, 0x1c0060, 0x1c01a0, 0x1c01a8, 0x1c01b0, 0x1c01b8, 0x1c01c0, 0x1c01c8, 0x1c01d0, 0x1c01d8, 0x1c01e0,
    0x1b3f50, 0x1b58c0, 0x1bd190, 0x1b8658, 0x1bda40, 0x1bdc10, 0x1bdd88, 0x1bdd90, 0x1bdd98, 0x1bdf68, 0x1be0e0,
    0x1be0e8, 0x1be2c8, 0x1be450, 0x1be468, 0x1be4f0, 0x1be508, 0x1be590, 0x1be5a8, 0x1be630, 0x1be638, 0x1be650,
    0x1be6d8, 0x1be6e0, 0x1be6e8, 0x1be700, 0x1be788, 0x1be7a0, 0x1be828, 0x1be838, 0x1be840, 0x1be848, 0x1be858,
    0x1beab8, 0x1becd8, 0x1bef08, 0x1bf130, 0x1bf348, 0x1bf570, 0x1bf580, 0x1bfc70, 0x1bfdc8, 0x1bfde8, 0x1bfdf8,
    0x1bfe00, 0x1bff68, 0x1bff78, 0x1c0018, 0x1c00d8, 0x1c00e8, 0x1c0108, 0x1c0128, 0x1c0138, 0x1c0148, 0x1c0180,
    0x1c0190,
];

#[test]
fn overlay_bases_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    // (the results overlay's init, its title rows' helpers, the screen overlay's destructor: real)
    for f in [0x1aacb0u32, 0x1c01e8] {
        r.allow(f);
    }
    let mut stubs = vec![
        ob::BACKDROP_INIT, ob::SCREEN_INIT, ob::DELETE, ob::SAVE_TO_CARD, ob::SAVE_INIT, ob::MC_DTOR, ob::ANIM_DTOR,
        ob::MEMBER_DTOR, ob::LOC, ob::WSTR_SET_ASCII, ob::SET_COLOUR, ob::FILLED_RECT, SCREEN_VF,
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
    let mut g = Rng(0x0b5e_0000_0000_0001);
    let mut frames = 0usize;
    let per = 600u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![(DATA.0, data.clone()), (TABLES.0, tables.clone()), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x2000])],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (strings, the renderer and its table), the pad state, the screen overlay's table
        m.w32(ob::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        m.w32(ob::PAD_STATE, PADS);
        // the overlay's backdrop: inside it (at the class's place) or apart, for both families; the race results'
        // two parts
        let card_base = match func {
            0x1bda40 | 0x1bdc10 => 0x478,
            0x1bdd98 | 0x1bdf68 => 0x570,
            0x1be0e8 | 0x1be2c8 => 0x44c,
            0x1be858 => 0x820,
            _ => 0x438,
        };
        let res_base = match func {
            0x1b3f50 | 0x1bef08 => 0x258,
            0x1b58c0 | 0x1beab8 => 0x268,
            0x1bf130 => 0x118,
            _ => 0x54,
        };
        m.w32(THIS, g.pick(&[THIS + res_base, APART]));
        m.w32(THIS + 0x42c, g.pick(&[THIS + card_base, APART]));
        if func == 0x1bf580 {
            m.w32(THIS, RES);
            m.w32(THIS + 4, CARD);
            m.w32(RES, g.pick(&[RES + 0x54, APART]));
            m.w32(CARD + 0x42c, CARD_B);
        }
        if func == 0x1bf570 {
            m.w32(THIS + 4, CARD);
        }
        if func == 0x1c0148 {
            m.w32(THIS + 0x28, SCREEN_VT);
            m.w16(SCREEN_VT + 0x78, g.pick(&[0u16, 8]));
            m.w32(SCREEN_VT + 0x7c, SCREEN_VF);
        }
        for o in [0xcu32, 0x10, 0x14, 0x430] {
            let any = g.next();
            m.w32(THIS + o, g.pick(&[0u32, 1, 2, any]));
        }
        let flags = g.pick(&[0u32, 0, 1, 2, 3]);
        let whole = g.pick(&[0u32, 0, 1]);
        let (a1, a2) = (g.next(), g.next());
        let rect = [g.f(0.0, 640.0), g.f(0.0, 480.0), g.f(0.0, 200.0), g.f(0.0, 200.0)];
        for k in 0..4 {
            wf(&mut m, ob::DRAW_COLOUR + 4 * k, g.f(0.0, 1.0));
        }
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
        let ours = match func {
            0x1b3f50 => ob::next_race_init(mm, e, THIS, whole, SCRATCH),
            0x1b58c0 => ob::unlocked_init(mm, e, THIS, whole, SCRATCH),
            0x1bd190 => ob::help_race_init(mm, e, THIS, whole, SCRATCH),
            0x1b8658 => {
                ob::draw_solid_rect(mm, e, THIS + 0x100, rect.map(F));
                none
            }
            0x1bda40 => { ob::card_dtor(mm, e, &ob::SELECT_EVENT, THIS, flags, SCRATCH); none }
            0x1bdd98 => { ob::card_dtor(mm, e, &ob::SELECT_VENUE, THIS, flags, SCRATCH); none }
            0x1be0e8 => { ob::card_dtor(mm, e, &ob::POINTS, THIS, flags, SCRATCH); none }
            0x1be858 => { ob::card_dtor(mm, e, &ob::PAUSE, THIS, flags, SCRATCH); none }
            0x1bfe00 => { ob::save_overlay_dtor(mm, e, THIS, flags, SCRATCH); none }
            0x1bdc10 => ob::card_ctor(mm, e, &ob::SELECT_EVENT, THIS, whole, SCRATCH),
            0x1bdf68 => ob::card_ctor(mm, e, &ob::SELECT_VENUE, THIS, whole, SCRATCH),
            0x1be2c8 => ob::card_ctor(mm, e, &ob::POINTS, THIS, whole, SCRATCH),
            0x1beab8 => { ob::results_dtor(mm, e, &ob::UNLOCKED, THIS, flags, SCRATCH); none }
            0x1becd8 => { ob::results_dtor(mm, e, &ob::HELP_RACE, THIS, flags, SCRATCH); none }
            0x1bef08 => { ob::results_dtor(mm, e, &ob::NEXT_RACE, THIS, flags, SCRATCH); none }
            0x1bf130 => { ob::results_dtor(mm, e, &ob::HISCORE_RECORD, THIS, flags, SCRATCH); none }
            0x1bf348 => { ob::results_dtor(mm, e, &ob::VIEW_HISCORES, THIS, flags, SCRATCH); none }
            0x1bfc70 => { ob::results_overlay_dtor(mm, e, THIS, flags, SCRATCH); none }
            0x1bf580 => { ob::race_results_dtor(mm, e, THIS, flags, SCRATCH); none }
            0x1bff78 => { ob::backdrop_dtor(mm, e, THIS, flags); none }
            0x1be450 => { ob::backdrop_kind_dtor(mm, e, ob::LESSON_CONTROLLER_TABLE, THIS, flags); none }
            0x1be4f0 => { ob::backdrop_kind_dtor(mm, e, ob::LESSON_TABLE, THIS, flags); none }
            0x1be468 => ob::backdrop_kind_ctor(mm, e, ob::LESSON_CONTROLLER_TABLE, THIS),
            0x1be508 => ob::backdrop_kind_ctor(mm, e, ob::LESSON_TABLE, THIS),
            0x1be590 => { ob::screen_kind_dtor(mm, e, ob::BLACK_FADE_TABLE, THIS, flags); none }
            0x1be638 => { ob::screen_kind_dtor(mm, e, ob::POP_ASYNC_TABLE, THIS, flags); none }
            0x1be6e8 => { ob::screen_kind_dtor(mm, e, ob::POP_FINISH_TABLE, THIS, flags); none }
            0x1be788 => { ob::screen_kind_dtor(mm, e, ob::COUNTDOWN_TABLE, THIS, flags); none }
            0x1be5a8 => ob::screen_kind_ctor(mm, e, ob::BLACK_FADE_TABLE, THIS),
            0x1be650 => ob::screen_kind_ctor(mm, e, ob::POP_ASYNC_TABLE, THIS),
            0x1be700 => ob::screen_kind_ctor(mm, e, ob::POP_FINISH_TABLE, THIS),
            0x1be7a0 => ob::screen_kind_ctor(mm, e, ob::COUNTDOWN_TABLE, THIS),
            0x1be630 => { ob::pop_async_reset(mm, THIS); none }
            0x1be6d8 => ob::answer_one(),
            0x1be6e0 | 0x1bdd90 | 0x1be0e0 | 0x1bf578 => ob::answer_this(THIS),
            0x1be838 | 0x1bf568 | 0x1c0040 | 0x1c01a0..=0x1c01e0 => ob::answer_zero(),
            0x1be828 | 0x1be830 | 0x1bf558 | 0x1bf560 | 0x1c0048..=0x1c0060 => { ob::nothing(); none }
            0x1be840 => { ob::save_to_card(mm, e, THIS); none }
            0x1bf570 => { ob::save_to_card_part(mm, e, THIS); none }
            0x1be848 => ob::pause_member(THIS),
            0x1bdd88 => { ob::set_venue_word(mm, THIS, a1); none }
            0x1bfdc8 => ob::new_buttons(mm),
            0x1bfde8 => ob::card_saving(mm, THIS),
            0x1bfdf8 => { ob::set_card_word(mm, THIS, a1); none }
            0x1bff68 => ob::dialog_up(mm, THIS),
            0x1c0018 => ob::screen_word18(mm, THIS),
            0x1c00d8 => ob::screen_shown(mm, THIS),
            0x1c00e8 => ob::screen_shown_and(mm, THIS, 0x14),
            0x1c0108 => ob::screen_shown_and(mm, THIS, 0x10),
            0x1c0128 => { ob::screen_set_pair(mm, THIS, a1, a2); none }
            0x1c0138 => { ob::set_pad_state(mm, a1); none }
            0x1c0148 => { ob::screen_vf_78(mm, e, THIS); none }
            0x1c0180 => { ob::screen_restore(mm, THIS); none }
            0x1c0190 => { ob::screen_keep(mm, THIS); none }
            _ => unreachable!(),
        };
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x1b8658 => vec![THIS, THIS + 0x100],
            0x1bdd88 | 0x1bfdf8 | 0x1c0138 => vec![THIS, a1],
            0x1c0128 => vec![THIS, a1, a2],
            0x1bda40 | 0x1bdd98 | 0x1be0e8 | 0x1be858 | 0x1bfe00 | 0x1beab8 | 0x1becd8 | 0x1bef08 | 0x1bf130 | 0x1bf348
            | 0x1bfc70 | 0x1bf580 | 0x1bff78 | 0x1be450 | 0x1be4f0 | 0x1be590 | 0x1be638 | 0x1be6e8 | 0x1be788 => vec![THIS, flags],
            0x1b3f50 | 0x1b58c0 | 0x1bd190 | 0x1bdc10 | 0x1bdf68 | 0x1be2c8 => vec![THIS, whole],
            _ => vec![THIS],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        for (k, v) in rect.into_iter().enumerate() {
            r.cpu.set_f(12 + k, v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        // the frame copies: each word that points into the port's frame has the runner's pointing into its stack,
        // the bytes there the same (0x1c8 for the race results' card table, else 0x98)
        let mut at = ARENA;
        while at < ARENA + SIZE {
            let ours = m.r32(at);
            let theirs = r.mem.read_u32(at);
            // (a random word the same on both sides is no pointer to a frame)
            if (SCRATCH..SCRATCH + 0x800).contains(&ours) && ours != theirs {
                assert!(stack(theirs), "case {case} {func:#x}: {at:#x} ours on the frame, theirs {theirs:#x}");
                let n = if ours == SCRATCH + 0x140 { 0x1c8 } else { 0x98 };
                let mut th = vec![0u8; n as usize];
                r.mem.read(theirs, &mut th).unwrap();
                let us: Vec<u8> = (0..n).map(|k| m.r8(ours + k)).collect();
                assert_eq!(th, us, "case {case} {func:#x}: the copy {at:#x} points at (ours {ours:#x} theirs {theirs:#x} flags {flags})");
                // a destructor's first copy, under its card table's
                if ours == SCRATCH && flags == 0 && matches!(func, 0x1bda40 | 0x1bdd98 | 0x1be0e8 | 0x1be858 | 0x1beab8 | 0x1becd8 | 0x1bef08 | 0x1bf130 | 0x1bf348) {
                    let mut th = vec![0u8; 0x98];
                    r.mem.read(theirs + 0xa0, &mut th).unwrap();
                    let us: Vec<u8> = (0..0x98).map(|k| m.r8(SCRATCH + 0xa0 + k)).collect();
                    assert_eq!(th, us, "case {case} {func:#x}: the first copy");
                }
                frames += 1;
                m.w32(at, theirs);
            }
            at += 4;
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
    println!("{frames} frame copies compared");
    assert!(frames > 2000);
}

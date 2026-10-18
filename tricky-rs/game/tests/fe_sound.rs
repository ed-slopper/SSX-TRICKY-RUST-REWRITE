//! `tricky_game::fe_sound` (board row G9y3) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a random sound object and sound system in the arena (their tables pointing at
//! stand-ins with random `this` shifts), random sound settings in the career, and a front end and renderer. One of
//! the row's functions then runs on both sides with random arguments in a1 and f12. The sound system's functions
//! and the front end's are stand-ins on both sides answering from the case's seed (the volumes as floats in f0). The
//! calls are compared with their arguments (the volumes' floats from f12), the answers (in v0 or f0) and the
//! memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_sound as sd;
use tricky_game::fe_wgroup as wg;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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
const SNDP: (u32, u32) = (0x343000, 0x100);
const CAREER: (u32, u32) = (0x32b000, 0x2000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x9000;
const GS: u32 = ARENA;
const S: u32 = ARENA + 0x8000;
const STV: u32 = ARENA + 0x8100;
const GSV: u32 = ARENA + 0x8300;
const APPOBJ: u32 = ARENA + 0x8400;
const RENDER: u32 = ARENA + 0x8480;
const RVT: u32 = ARENA + 0x8500;
const FE: u32 = ARENA + 0x8600;
const FEV: u32 = ARENA + 0x8700;
const SCRATCH: u32 = 0x01f8_0000;
/// the sound object's virtuals' stand-ins (by slot), the sound system's (its table at +4), the renderer's and the
/// front end's (its table at +0)
fn slot(off: u32) -> u32 {
    0x0180_0000 + off
}
fn ssv(off: u32) -> u32 {
    0x0181_0000 + off
}
fn rv(off: u32) -> u32 {
    0x0182_0000 + off
}
fn fev(off: u32) -> u32 {
    0x0183_0000 + off
}
const SLOTS: [u32; 3] = [0x10, 0x60, 0xc8];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

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

/// A callee's integer and float arguments
fn arity(f: u32) -> (usize, usize) {
    match f {
        sd::SND_SET_VOLUME => (2, 1),
        sd::SND_SET_MASTER => (1, 1),
        sd::SND_WAIT_LOAD => (0, 0),
        sd::SND_UPDATE | sd::SND_STOP_ALL | sd::SND_RELOAD | sd::SND_STOP_MUSIC | sd::SND_LOAD_BANK | sd::FE_UPDATE_ALL
        | 0x21c868 | 0x21c808 | 0x21c8a8 | 0x225a90 | 0x225ba0 | 0x225c30 | 0x21c930 | sd::SND_CLOSE => (1, 0),
        f if f == slot(0x60) || f == rv(0x70) => (1, 0),
        _ => (2, 0),
    }
}

fn standin(f: u32, w: &[u32], fl: &[u32], ans: &Answers, table: u32) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].to_vec();
    log.extend(&fl[..nf]);
    // (the object's table as the sound system is closed)
    if f == sd::SND_CLOSE {
        log.push(table);
    }
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        sd::SND_GET_VOLUME => g.f(0.0, 1.0).to_bits(),
        f if f == ssv(0x10) => [1u32, 1, 0, r][(r % 4) as usize],
        f if f == rv(0x70) => (r % 3 != 0) as u32,
        sd::FE_SOUND_EVENT => r % 2,
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
        let (a, c) = standin(f, &w, &fl, self.ans, m.r32(S));
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const VOLS: [(u32, u32, bool); 10] = [
    (0x270fd0, 0, false), (0x270ff8, 0, true), (0x271008, 1, false), (0x271030, 1, true), (0x271040, 2, false),
    (0x271068, 2, true), (0x271098, 3, false), (0x2710c0, 3, true), (0x2710d0, 4, false), (0x2710f8, 4, true),
];

#[test]
fn sound_object_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let mut funcs: Vec<u32> = vec![
        0x2702e0, 0x270328, 0x270378, 0x2703d0, 0x2704c8, 0x2704f0, 0x270610, 0x270770, 0x270790, 0x2707e0, 0x270860,
        0x2708b8, 0x270958, 0x2709b8, 0x2709f8, 0x270a70, 0x270b18, 0x270be8, 0x270c20, 0x270c30, 0x270fb0, 0x271078,
        0x271238,
    ];
    funcs.extend(sd::FORWARDS.map(|p| p.0));
    funcs.extend(VOLS.map(|v| v.0));
    funcs.extend(sd::BASE_NOTHING);
    funcs.extend(sd::BASE_ZERO);
    funcs.extend(sd::BASE_FZERO);
    funcs.extend(sd::BASE_ONE);
    assert_eq!(funcs.len(), 111);
    for f in &funcs {
        r.allow(*f);
    }
    let mut stubs: Vec<u32> = vec![
        sd::SND_OPEN, sd::SND_CLOSE, sd::SND_PLAY_EVENT, sd::FE_SOUND_EVENT, sd::SND_UPDATE, sd::SND_STOP_ALL,
        sd::SND_GET_VOLUME, sd::SND_SET_VOLUME, sd::SND_STOP_MUSIC, sd::SND_RELOAD, sd::SND_WAIT_LOAD,
        sd::SND_LOAD_BANK, sd::FE_UPDATE_ALL, sd::SND_MUSIC_2129F8, wg::FREE, 0x212428, 0x212300, 0x212348, 0x212388,
        0x212680, 0x2123c8, 0x2123f8, 0x21c868, 0x21c808, 0x21c8a8, 0x225a90, 0x225ba0, 0x225c30, 0x21c930, 0x21c960,
        0x21cfa0, slot(0x10), slot(0x60), slot(0xc8), ssv(0x10), rv(0x70), fev(0x38),
    ];
    for (_, f) in sd::FORWARDS {
        if !stubs.contains(&f) {
            stubs.push(f);
        }
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(f, &w, &fl, &a, mem.read_u32(S));
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
    let (data, sndp) = (read(&mut r, DATA), read(&mut r, SNDP));
    let mut g = Rng(0x9e20_0000_0000_0001);
    let per = 600u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut career = vec![0u8; CAREER.1 as usize];
        for b in career.iter_mut() {
            *b = g.next() as u8;
        }
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (SNDP.0, sndp.clone()),
                (CAREER.0, career),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x100]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(sd::SOUND_SYSTEM, GS);
        m.w32(GS + 4, GSV);
        m.w16(GSV + 0x10, g.pick(&[0u16, 8]));
        m.w32(GSV + 0x14, ssv(0x10));
        m.w32(S, STV);
        for off in SLOTS {
            m.w16(STV + off, g.pick(&[0u16, 4]));
            m.w32(STV + off + 4, slot(off));
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x734, g.pick(&[0, FE, FE]));
        m.w32(RENDER + 0x10, RVT);
        m.w16(RVT + 0x70, g.pick(&[0u16, 0x10]));
        m.w32(RVT + 0x74, rv(0x70));
        m.w32(FE, FEV);
        m.w16(FEV + 0x38, g.pick(&[0u16, 8]));
        m.w32(FEV + 0x3c, fev(0x38));
        // (vf38 waits on the front end: it is there)
        if func == 0x270c30 {
            m.w32(APPOBJ + 0x734, FE);
        }
        m.w32(sd::CAREER + 8 + 0x169c, g.pick(&[0u32, 1]));
        for k in 0..6 {
            m.w32(sd::CAREER + 8 + 0x1650 + 4 * k, g.f(0.0, 1.0).to_bits());
        }
        let any = g.next();
        let low = g.below(28);
        let a1 = g.pick(&[any, 0, 1, 5, 11, 12, 0xff, u32::MAX, 24, 25, low, low, low]);
        let flags = g.pick(&[0u32, 1, 2, 3]);
        let f12 = g.f(-1.0, 2.0);
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
        let fv = F(f12);
        let mut ours_f: Option<F> = None;
        let ours = match func {
            0x2702e0 => sd::ctor(mm, e, S, a1),
            0x270328 => { sd::dtor(mm, e, S, flags); none }
            0x271238 => { sd::base_dtor(mm, e, S, flags); none }
            0x270378 => { sd::vf1(mm, e, a1, fv); none }
            0x2703d0 => { sd::play_event(mm, e, S, a1, false); none }
            0x2704f0 => { sd::play_event(mm, e, S, a1, true); none }
            0x2704c8 => sd::vf3(mm, e, S),
            0x270610 => { sd::vf9(mm, e, a1, fv); none }
            0x270770 => { sd::vf10(mm, a1); none }
            0x270790 => { sd::vf12(mm, e, S, a1, false); none }
            0x270860 => { sd::vf12(mm, e, S, a1, true); none }
            0x2707e0 => { sd::vf13(mm, e, S, a1); none }
            0x2708b8 => { sd::vf15(mm, e, S, a1); none }
            0x270958 => { sd::vf18(mm, e, a1, fv, false); none }
            0x2709b8 => { sd::vf18(mm, e, a1, fv, true); none }
            0x2709f8 => { sd::vf24(mm, e, S); none }
            0x270a70 => { sd::vf28(mm, e, a1, fv); none }
            0x270b18 => { sd::vf34(mm, e); none }
            0x270be8 => sd::vf35(mm, e, a1, fv),
            0x270c20 => sd::read_word(mm, 0x3838),
            0x270fb0 => sd::read_word(mm, 0x3834),
            0x270c30 => { sd::vf38(mm, e, S, a1); none }
            0x271078 => { ours_f = Some(sd::vf47(mm)); none }
            f if sd::FORWARDS.iter().any(|p| p.0 == f) => {
                let to = sd::FORWARDS.iter().find(|p| p.0 == f).unwrap().1;
                // (those that call rather than jump answer nothing of their own)
                let v = sd::forward(mm, e, to, a1, fv);
                if matches!(f, 0x270ac0 | 0x270bf8 | 0x271108) { none } else { v }
            }
            f if VOLS.iter().any(|v| v.0 == f) => {
                let &(_, ch, set) = VOLS.iter().find(|v| v.0 == f).unwrap();
                if set {
                    sd::set_volume(mm, e, ch, fv);
                } else {
                    ours_f = Some(sd::get_volume(mm, e, ch));
                }
                none
            }
            f if sd::BASE_NOTHING.contains(&f) => { sd::base_nothing(); none }
            f if sd::BASE_ZERO.contains(&f) => sd::base_zero(),
            f if sd::BASE_FZERO.contains(&f) => { ours_f = Some(sd::base_fzero()); none }
            f if sd::BASE_ONE.contains(&f) => sd::base_one(),
            _ => unreachable!(),
        };
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x270328 | 0x271238 => vec![S, flags],
            _ => vec![S, a1],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f12);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        if let Some(v) = ours_f {
            assert_eq!(r.cpu.f(0).to_bits(), v.0.to_bits(), "case {case} {func:#x}: the float answer");
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

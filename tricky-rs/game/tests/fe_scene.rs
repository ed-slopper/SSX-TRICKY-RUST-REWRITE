//! `tricky_game::fe_scene` (board row G9z1) against the original in the function runner (TRICKY_ELF; skipped without
//! it): the screens' audio, voice-line and world helpers. Every case builds a random front end (its riders, camera,
//! choices, world, sound player, mode and course films) and app in the arena; one helper then runs on both sides with
//! random arguments. The helpers that call each other run for real on both sides; every other callee is a stand-in
//! answering from the case's seed: the scripts, the name hashes, the world's finds (none or one of four things, each
//! with or without its instance and controller), the camera, the medals and the film player, and the objects'
//! virtuals (a table of stand-ins). The calls are compared with their arguments, the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_scene as sc;
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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
/// the game's words the helpers read: the names' tables and the venues' music bytes; the career's player word
const GAME: [(u32, u32); 2] = [(0x347000, 0x300), (0x3a8a00, 0x400)];
const CAREER: (u32, u32) = (0x32de00, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x5000;
const APPOBJ: u32 = ARENA + 0x800;
const FE: u32 = ARENA + 0x1000;
const VT: u32 = ARENA + 0x1800;
const SND: u32 = ARENA + 0x1e00;
/// the riders (12), the films (10), the things (4), their instances and controllers: 0x100 apart from 0x2000
fn obj(k: u32) -> u32 {
    ARENA + 0x2000 + 0x100 * k
}
const SCRATCH: u32 = 0x01f8_0000;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
/// the stand-in table's slots: the sound player's, the riders' and films' +0x28, the controllers' +0xc0
const SLOTS: [u32; 15] = [0x10, 0x18, 0x20, 0x28, 0x60, 0x68, 0x70, 0x78, 0x80, 0x98, 0xa0, 0xa8, 0xb0, 0xb8, 0xc0];
/// the slots that take only the object
const LONE: [u32; 6] = [0x60, 0x98, 0xa0, 0xa8, 0xb0, 0xb8];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn arity(f: u32) -> (usize, usize) {
    match f {
        sc::VIDEO_OPEN => (6, 0),
        sc::MEDAL => (3, 0),
        sc::FIND_BY_HASH | sc::CAMERA_SET_MODE | sc::VIDEO_RELEASE => (2, 0),
        sc::RUN_SCRIPT | sc::NAME_HASH => (1, 0),
        f if f == slot(0xc0) => (2, 1),
        f if LONE.iter().any(|&o| slot(o) == f) => (1, 0),
        _ => (2, 0),
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

fn standin(f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f);
    let mut log: Vec<u32> = w[..ni].to_vec();
    log.extend(&fl[..nf]);
    let r = ans.next();
    let mut g = Rng(r as u64 | 1);
    let answer = match f {
        sc::FIND_BY_HASH => g.pick(&[0, obj(22), obj(23), obj(24), obj(25)]),
        sc::MEDAL => g.pick(&[0u32, 1, 2, 3, u32::MAX]),
        sc::VIDEO_OPEN => g.pick(&[0u32, 3, 0xffff_ffff]),
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, _m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let (ni, nf) = arity(f);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(4, 0);
        let (a, c) = standin(f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

/// The helpers that are not one line or one sound call
const OTHERS: [u32; 30] = [
    0x261b38, 0x261e98, 0x261ee0, 0x2621a0, 0x262260, 0x262608, 0x262338, 0x262388, 0x2624b8, 0x2623d8, 0x2623f0,
    0x262438, 0x2627f0, 0x262840, 0x262890, 0x262960, 0x262a68, 0x262ad0, 0x262b78, 0x262bd0, 0x262cd8, 0x262d80,
    0x263000, 0x263150, 0x263270, 0x2670e0, 0x269be8, 0x269c70, 0x269cc8, 0x262f48,
];

fn funcs() -> Vec<u32> {
    let mut v: Vec<u32> = sc::SAY_LINES.iter().map(|&(a, _)| a).collect();
    v.extend(sc::SOUND_CALLS.iter().map(|&(a, _)| a));
    v.extend(sc::SOUND_FLAGS.iter().map(|&(a, _, _)| a));
    v.extend(OTHERS.iter().filter(|a| !v.contains(a)).copied().collect::<Vec<_>>());
    v
}

/// One helper on our side
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], f12: u32) {
    if let Some(&(_, line)) = sc::SAY_LINES.iter().find(|&&(x, _)| x == f) {
        return sc::say_line(m, e, line);
    }
    if let Some(&(_, off)) = sc::SOUND_CALLS.iter().find(|&&(x, _)| x == f) {
        sc::snd(m, e, off, &[]);
        return;
    }
    if let Some(&(_, off, v)) = sc::SOUND_FLAGS.iter().find(|&&(x, _, _)| x == f) {
        sc::snd(m, e, off, &[v]);
        return;
    }
    match f {
        0x261b38 => sc::set_selected(m, e, a[0], a[1], a[2], a[3]),
        0x261e98 => sc::clear_venue(m, e, a[0]),
        0x261ee0 => sc::say_indexed_line(m, e, a[1]),
        0x2621a0 => sc::charsel_start_audio(m, e),
        0x262260 => sc::charmodify_start_audio(m, e),
        0x262608 => sc::options_start_audio(m, e),
        0x262338 => sc::line_then_music(m, e, 0x2b),
        0x262388 => sc::line_then_music(m, e, 7),
        0x2624b8 => sc::line_then_music(m, e, 0x2f),
        0x2623d8 => sc::modeselect_set_camera(m, e),
        0x2623f0 => sc::title_stop_then_line(m, e),
        0x262438 => sc::title_set_camera_and_music(m, e),
        0x2627f0 => sc::line_then_panel(m, e, a[0], 6, 0),
        0x262840 => sc::line_then_panel(m, e, a[0], 4, 1),
        0x262890 => sc::line_then_panel(m, e, a[0], 5, 2),
        0x262960 => sc::set_open_panel(m, e, a[0], a[1]),
        0x262a68 => sc::title_open(m, e),
        0x262ad0 => sc::charsel_open(m, e),
        0x262b78 => sc::camera_b(m, e),
        0x262bd0 => sc::panel_3(m, e, a[0]),
        0x262cd8 => sc::title_stop(m, e),
        0x262d80 => sc::show_medals(m, e, a[1]),
        0x263000 => sc::show_course_video(m, e, a[1], a[2]),
        0x263150 => sc::set_venue(m, e, a[0], a[1]),
        0x263270 => sc::reset_choice(m, a[0]),
        0x2670e0 => sc::character_set_selected(m, a[0], a[1]),
        0x269be8 => sc::thing_set_param(m, e, a[0], a[1], f12),
        0x269c70 => sc::open_course_video(m, e, a[0], a[1]),
        0x269cc8 => sc::movie_stop(m, e, a[0]),
        _ => unreachable!("{f:#x}"),
    }
}

/// The case's arguments for helper `f`
fn args(g: &mut Rng, f: u32) -> [u32; 4] {
    let byte = ARENA + 0x100 + 0x10 * g.below(4);
    match f {
        0x261b38 => [FE, g.pick(&[0u32, 1, 0xffff_ff01, 0xff]), g.next(), g.below(12)],
        0x261e98 | 0x2627f0 | 0x262840 | 0x262890 | 0x262bd0 | 0x263270 => [byte, g.next(), g.next(), g.next()],
        0x262960 => [byte, g.pick(&[0u32, 1, 2, 3, 0xff, 0x102, 0xffff_ff00]), g.next(), g.next()],
        0x263150 => [byte, g.pick(&[0u32, 3, 9, 0x105]), g.next(), g.next()],
        0x261ee0 => [g.next(), g.next(), g.next(), g.next()],
        0x262d80 => [g.next(), g.pick(&[0u32, 1, 0x100]), g.next(), g.next()],
        0x263000 => [g.next(), g.pick(&[0u32, 4, 9, 10, 0xff, 0x80, 0x103]), g.pick(&[0u32, 1]), g.next()],
        0x2670e0 => [obj(g.below(12)), g.next(), g.next(), g.next()],
        0x269be8 => [obj(22 + g.below(4)), g.next(), g.next(), g.next()],
        0x269c70 | 0x269cc8 => [obj(12 + g.below(10)), g.next(), g.next(), g.next()],
        _ => [g.next(), g.next(), g.next(), g.next()],
    }
}

#[test]
fn scene_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = funcs();
    for &f in &funcs {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let mut stubs = vec![
        sc::RUN_SCRIPT, sc::NAME_HASH, sc::FIND_BY_HASH, sc::CAMERA_SET_MODE, sc::MEDAL, sc::VIDEO_OPEN,
        sc::VIDEO_RELEASE,
    ];
    stubs.extend(SLOTS.map(slot));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, _mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e5c_0000_0000_0001);
    let per = 200u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone()), (CAREER.0, vec![0; CAREER.1 as usize])];
        regions.extend(game.iter().cloned());
        regions.extend([(ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x100])]);
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..CAREER.1 / 4 {
            m.w32(CAREER.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x734, FE);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the front end: its riders, the players' choices, its sound player, its mode, its films)
        for i in 0..12 {
            m.w32(FE + 0x20 + 4 * i, obj(i));
        }
        for k in 0..4 {
            m.w8(FE + 0x7c + k, g.pick(&[0u8, 3, 11, 0xff]));
            m.w8(ARENA + 0x100 + 0x10 * k, g.pick(&[0u8, 1, 2, 3, 0xff]));
            m.w8(ARENA + 0x101 + 0x10 * k, g.pick(&[0u8, 4, 9, 0xff, 0x80]));
        }
        // (the players' choices, and the byte a player of -1 reads)
        m.w8(FE + 0x7b, g.below(12) as u8);
        m.w8(FE + 0x7d, g.below(12) as u8);
        m.w8(FE + 0x7f, g.below(12) as u8);
        m.w32(FE + 0xa4, SND);
        m.w32(SND, VT);
        m.w32(FE + 0xf0, g.pick(&[0u32, 1, 2, 3]));
        for i in 0..10 {
            m.w32(FE + 0xfc + 4 * i, obj(12 + i));
        }
        for k in 0..22 {
            m.w32(obj(k) + 0xc, VT);
            m.w32(obj(k) + 0xb0, g.pick(&[0u32, 5, 0xffff_ffff]));
        }
        // (the things: each with or without its instance, and that with or without its controller)
        for k in 0..4 {
            let (t, inst, ctl) = (obj(22 + k), obj(26 + k), obj(30 + k));
            m.w32(t + 0xa0, if g.below(4) == 0 { 0 } else { inst });
            m.w32(inst + 0xe4, if g.below(4) == 0 { 0 } else { ctl });
            m.w32(ctl + 8, VT);
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let a = args(&mut g, func);
        let f12 = g.next();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        ours(&mut m, &mut env, func, a, f12);
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f32::from_bits(f12));
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
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

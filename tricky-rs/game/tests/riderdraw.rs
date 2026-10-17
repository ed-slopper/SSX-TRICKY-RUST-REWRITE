//! `tricky_game::riderdraw` (board row G7g4) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is a fresh random draw object, rider (drawn or not, visible, fading or whole, its
//! kinds, its streaks live or not, LODs, place and depth, skeleton, height) and game (its mode, players), under
//! a random renderer (its vtable's this-shifts). The draw's callees ported elsewhere (skin, shadow, lights,
//! streaks, effects, marker, icon) and the renderer's virtuals are logging stand-ins. They read what they are
//! handed (the streak points, the box, the shadow's centre and up, the colours) from the pointers, and answer
//! as the case says (the camera's rider, the box test, the field of view, the outfit). `Math_SinCos` and
//! `Vec4_Scale` run as the game's. The memory (bar the port's scratch: the game's stack) and the calls are
//! compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::riderdraw::{self as rd, RiderDrawEnv};

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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x1_0000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x800;
const GAME: u32 = ARENA + 0xa00;
const MODE: u32 = ARENA + 0xe00;
const VIEW: u32 = ARENA + 0x3800;
const BR: u32 = ARENA + 0x1000;
const PART: u32 = ARENA + 0x1600;
const SKEL: u32 = ARENA + 0x1700;
const MATS: u32 = ARENA + 0x1800;
const SHADOW: u32 = ARENA + 0x2000;
const RIDER: u32 = ARENA + 0x4000;
const SCRATCH: u32 = 0x0180_0000;
const VSLOTS: [u32; 18] = [0xb8, 0xc8, 0xf8, 0x178, 0x180, 0x188, 0x1c0, 0x1c8, 0x1d0, 0x1d8, 0x1e0, 0x1e8, 0x218, 0x280, 0x2b8, 0x1f0, 0x1f8, 0x200];
const FUNCS: [u32; 11] = [rd::GET_PLAYER_OUTFIT, rd::STREAK_DRAW, rd::DRAW_EFFECTS, rd::FOCUS_RIDER, rd::MARKER_DRAW, rd::ICON_DRAW, rd::COPY_BONES, rd::LIGHTS_BUILD, rd::SKIN_MATRICES, rd::SHADOW_UPDATE, rd::SET_BOARD_MATRIX];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// The answers a case gives: the outfit, the camera's rider, the box test, the field of view, the state
#[derive(Clone, Copy, Default)]
struct Answers {
    outfit: u32,
    focus: u32,
    boxed: u32,
    fov: u32,
}

/// What a call is logged with (`w` its eight argument words, `f12` its float) and its answer
fn logged(func: u32, w: &[u32], f12: u32, a: &Answers, read: &mut dyn FnMut(u32) -> u32) -> (Vec<u32>, u32) {
    let words = |read: &mut dyn FnMut(u32) -> u32, at: u32, n: u32| (0..n).map(|k| read(at + 4 * k)).collect::<Vec<u32>>();
    let mut v = vec![];
    let mut answer = 0;
    match func {
        rd::GET_PLAYER_OUTFIT => {
            v.push(w[0]);
            answer = a.outfit;
        }
        rd::STREAK_DRAW => {
            v.push(w[0]);
            v.extend(words(read, w[1], 16));
        }
        rd::FOCUS_RIDER => {
            v.push(w[0]);
            answer = a.focus;
        }
        rd::LIGHTS_BUILD => v.extend(&w[1..8]),
        rd::SHADOW_UPDATE => {
            v.push(w[0]);
            v.extend(words(read, w[1], 3));
            v.push(w[2]);
            v.extend(words(read, w[3], 3));
            v.extend([w[4], w[5]]);
        }
        rd::SET_BOARD_MATRIX => {
            v.extend([w[0], w[1]]);
            v.extend(words(read, w[2], 4));
            v.extend(words(read, w[3], 4));
        }
        f if f == rd::SKIN_MATRICES => v.extend([w[0], w[1]]),
        f if f >= vaddr(0) => {
            let off = f - vaddr(0);
            v.push(w[0]);
            match off {
                0xb8 => v.extend([w[1], f12]),
                0x1c8 => v.extend([w[1], w[2], f12]),
                0x280 => {
                    v.extend(words(read, w[1], 4));
                    v.extend(words(read, w[2], 4));
                    answer = a.boxed;
                }
                0x188 => {
                    v.push(w[1]);
                    v.extend(words(read, w[2], 4));
                }
                0xc8 => answer = a.fov,
                0xf8 => answer = VIEW,
                0x1d8 => answer = 0x5eed,
                0x218 | 0x180 => {}
                0x1e8 => v.extend([w[1], w[2]]),
                _ => v.push(w[1]),
            }
        }
        _ => v.push(w[0]),
    }
    (v, answer)
}

struct Ours(Vec<Call>, Answers);
impl RiderDrawEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut read = |a: u32| m.r32(a);
        let (l, a) = logged(func, &w, 0, &self.1, &mut read);
        self.0.push(Call(func, l));
        a
    }
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let off = func - vaddr(0);
        let mut w = vec![this];
        w.extend(args);
        let f12 = match off {
            0xb8 => args[1],
            0x1c8 => args[2],
            _ => 0,
        };
        w.resize(8, 0);
        let mut read = |a: u32| m.r32(a);
        let (l, a) = logged(func, &w, f12, &self.1, &mut read);
        self.0.push(Call(func, l));
        a
    }
}

fn runner(log: &Rc<RefCell<Vec<Call>>>, ans: &Rc<Cell<Answers>>) -> Option<Runner> {
    let mut r = Runner::from_env()?;
    for f in [0x1364d0u32, 0x136370, 0x136c48, 0x136e38, 0x167038, 0x250c10, 0x102f50] {
        r.allow(f);
    }
    for func in FUNCS.into_iter().chain([rd::SHADOW_DRAW]).chain(VSLOTS.map(vaddr)) {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let mut rdm = |at: u32| mem.read_u32(at);
            let (lg, answer) = logged(func, &w, c.fpr[12], &a.get(), &mut rdm);
            l.borrow_mut().push(Call(func, lg));
            c.set_gpr(2, answer as u64);
            c.set_f(0, f32::from_bits(answer));
        });
    }
    Some(r)
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// A fresh case's memory: random words, then the objects the draw follows
fn fresh(g: &mut Rng) -> Mem {
    let mut m = Mem { regions: vec![(0, vec![0; 0x600]), (ARENA, vec![0; SIZE as usize]), (rd::APP, vec![0; 4]), (SCRATCH, vec![0; 0x120])] };
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    for k in 0..0x120 / 4 {
        m.w32(SCRATCH + 4 * k, g.next());
    }
    m.w32(RENDER + 0x10, RVT);
    for off in VSLOTS {
        m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
        m.w32(RVT + off + 4, vaddr(off));
    }
    m.w32(rd::APP, APPOBJ);
    m.w32(APPOBJ + 0x724, RENDER);
    m.w32(APPOBJ + 0x730, if g.below(8) == 0 { 0 } else { GAME });
    m.w32(GAME + 0x1c, MODE);
    m.w32(MODE + 0x1c, [0u32, 1, 6, 3][g.below(4) as usize]);
    m.w32(MODE + 0x7c, g.below(4));
    m.w32(GAME + 0x298, g.below(4));
    m.w32(GAME + 0x290, g.below(2));
    // the view: camera-like rows
    for i in 0..4 {
        for j in 0..4 {
            wf(&mut m, VIEW + 0x10 * i + 4 * j, if i == j { 1.0 } else if i == 3 { g.f(-100.0, 100.0) } else { g.f(-0.3, 0.3) });
        }
    }
    // the draw object and the rider
    m.w32(BR, RIDER);
    m.w32(BR + 4, g.below(2));
    m.w32(BR + 8, SHADOW);
    m.w32(BR + 0x20 + 0x1c4, g.below(4));
    m.w32(BR + 0x200 + 0x1c4, g.below(4));
    m.w32(RIDER + 0x46b0, (g.below(6) != 0) as u32);
    wf(&mut m, RIDER + 0x448, [0.0f32, 1.0, 1.0, 0.5, 0.95, 0.9499][g.below(6) as usize]);
    m.w32(RIDER + 0x464, PART);
    m.w32(PART + 0x78, g.below(2));
    m.w8(PART + 0x65, [0xffu8, 0, 2, 0x80][g.below(4) as usize]);
    m.w32(RIDER + 0x41c, g.below(3));
    m.w32(RIDER + 0x418, [0u32, 1, u32::MAX][g.below(3) as usize]);
    m.w32(RIDER + 0x424, [2u32, 2, 1][g.below(3) as usize]);
    wf(&mut m, RIDER + 0x1bc, [0.0f32, -2.0, -7.0, 3.0, g.f(-6.0, 1.0)][g.below(5) as usize]);
    m.w32(RIDER + 0x46b8, [2u32, 3, 4, 5][g.below(4) as usize]);
    m.w32(RIDER + 0x4494, if g.below(8) == 0 { 0 } else { SKEL });
    m.w32(SKEL + 4, MATS);
    for k in 0..0x500 / 4 {
        wf(&mut m, MATS + 4 * k, g.f(-80.0, 80.0));
    }
    for k in 0..4 {
        wf(&mut m, RIDER + 0x4ad0 + 4 * k, g.f(-500.0, 500.0));
        wf(&mut m, RIDER + 0x140 + 4 * k, if k == 3 { 1.0 } else { g.f(-300.0, 300.0) });
        wf(&mut m, RIDER + 0x5760 + 4 * k, g.f(-2.0, 2.0));
    }
    m
}

/// One case: the port and the game, the calls and the memory (bar the scratch) compared
fn run(r: &mut Runner, log: &Rc<RefCell<Vec<Call>>>, m: &mut Mem, a: Answers, case: u32, port: &mut dyn FnMut(&mut Mem, &mut Ours), func: u32, regs: &[u32]) -> Vec<Call> {
    for (at, b) in &m.regions {
        if *at != SCRATCH {
            r.mem.write(*at, b).unwrap();
        }
    }
    log.borrow_mut().clear();
    let mut ours = Ours(Vec::new(), a);
    port(m, &mut ours);
    for (k, v) in regs.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *v as u64);
    }
    r.call(func).unwrap_or_else(|e| panic!("case {case}: {e}"));
    let theirs = log.borrow().clone();
    if theirs != ours.0 {
        let i = theirs.iter().zip(&ours.0).position(|(x, y)| x != y).unwrap_or(theirs.len().min(ours.0.len()));
        panic!("case {case}: call {i} of {}/{}: theirs {:x?} ours {:x?}", theirs.len(), ours.0.len(), theirs.get(i), ours.0.get(i));
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
            panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
        }
    }
    theirs
}

fn answers(g: &mut Rng) -> Answers {
    Answers {
        outfit: [6u32, 6, 0, 5, 7][g.below(5) as usize],
        focus: [RIDER, 0x0123_4560][g.below(2) as usize],
        boxed: [0u32, 0, 0, 1, 2][g.below(5) as usize],
        fov: [0.3f32, 0.6, 1.0, 1.4][g.below(4) as usize].to_bits(),
    }
}

#[test]
fn rider_draw_helpers_match_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Cell::new(Answers::default()));
    let Some(mut r) = runner(&log, &ans) else { return };
    let mut g = Rng(0x7d1d_0000_0000_0001);
    let mut lods = [0usize; 3];
    for case in 0..4000u32 {
        let mut m = fresh(&mut g);
        // (the rider's place from near to far, so that every LOD comes)
        wf(&mut m, RIDER + 0x148, [10.0f32, 200.0, 700.0, 2000.0, g.f(-100.0, 3000.0)][g.below(5) as usize]);
        let a = answers(&mut g);
        ans.set(a);
        match case % 4 {
            0 => {
                run(&mut r, &log, &mut m, a, case, &mut |m, e| rd::pick_lod(m, e, BR), 0x136370, &[BR]);
                lods[m.r32(BR + 0x3f8).min(2) as usize] += 1;
            }
            1 => {
                run(&mut r, &log, &mut m, a, case, &mut |m, _| rd::get_hand_streak_points(m, BR, ARENA + 0x3000), 0x136c48, &[BR, ARENA + 0x3000]);
            }
            2 => {
                run(&mut r, &log, &mut m, a, case, &mut |m, _| rd::copy_boarder_matrix(m, BR, ARENA + 0x3000), 0x136e38, &[BR, ARENA + 0x3000]);
            }
            _ => {
                let mut got = 0;
                run(&mut r, &log, &mut m, a, case, &mut |m, e| got = rd::is_player_outfit6(m, e, rd::CAREER), 0x167038, &[rd::CAREER]);
                assert_eq!(r.cpu.gpr(2) as u32, got, "case {case}: the answer");
            }
        }
    }
    println!("LODs {lods:?}");
    assert!(lods.iter().all(|&n| n > 50));
}

#[test]
fn rider_draw_matches_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Cell::new(Answers::default()));
    let Some(mut r) = runner(&log, &ans) else { return };
    let mut g = Rng(0x7d1d_1000_0000_0001);
    let (mut skinned, mut shadows, mut markers) = (0usize, 0usize, 0usize);
    for case in 0..6000u32 {
        let mut m = fresh(&mut g);
        let a = answers(&mut g);
        ans.set(a);
        let calls = run(&mut r, &log, &mut m, a, case, &mut |m, e| rd::draw(m, e, BR, SCRATCH), 0x1364d0, &[BR]);
        skinned += calls.iter().filter(|c| c.0 == rd::SKIN_MATRICES).count();
        shadows += calls.iter().filter(|c| c.0 == rd::SHADOW_UPDATE).count();
        markers += calls.iter().filter(|c| c.0 == rd::MARKER_DRAW).count();
    }
    println!("{skinned} skin draws, {shadows} shadows made, {markers} markers");
    assert!(skinned > 2000 && shadows > 500 && markers > 50);
}

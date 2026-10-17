//! `tricky_game::particledraw` (board row G7e1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). `ParticleSys_Draw`: every case a fresh random app, renderer (its vtable's this-shifts), system
//! (every kind from −2 to 12 and far beyond, its texture's place) and box-test answer; the renderer's virtuals are
//! logging stand-ins. The blend and the two particle packets: a fresh random renderer (the texture state equal to
//! what is set or not, the frame's matrices made or not), block and blend words; the renderer's +0x1e8, the
//! frame's two calls and the submit are logging stand-ins (the submit logs the texture state as it is then; the
//! second +0x1e8 sometimes leaves another program or word 0 in it, as a draw between could).
//! The memory and the calls compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::particledraw::{self as p, ParticleDrawEnv};
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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x1800;
const CAM: u32 = ARENA + 0x1c00;
const TEXSTATE: u32 = ARENA + 0x1c80;
const APPOBJ: u32 = ARENA + 0x1d00;
const TEXTABLE: u32 = ARENA + 0x2000;
const PS: u32 = ARENA + 0x2100;
const PKT: u32 = ARENA + 0x3000;
const SLOTS: [u32; 7] = [0x1c0, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x258, 0x280];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// The calls, the box test's answer, what the second +0x1e8 leaves in the texture state (its program, word 0)
struct Ours(Vec<Call>, u32, Option<(u8, u32)>);
impl CullEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) {
        let mut w = args.to_vec();
        if func == p::SUBMIT {
            w.extend([m.r32(TEXSTATE), m.r8(TEXSTATE + 0x10) as u32, m.r32(RENDER + 0x474)]);
        }
        self.0.push(Call(func, w));
    }
}
impl ParticleDrawEnv for Ours {
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.0.push(Call(func, std::iter::once(this).chain(args.iter().copied()).collect()));
        if let (true, Some((b, w))) = (func == vaddr(0x1e8) && args[0] == 0, self.2) {
            m.w8(TEXSTATE + 0x10, b);
            m.w32(TEXSTATE, w);
        }
        if func == vaddr(0x280) { self.1 } else { 0 }
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// The runner with logging stand-ins for the renderer's virtuals (the box test answering `answer`), the frame's
/// two calls and the submit
fn runner(log: &Rc<RefCell<Vec<Call>>>, answer: &Rc<Cell<u32>>, poke: &Rc<Cell<Option<(u8, u32)>>>) -> Option<Runner> {
    let mut r = Runner::from_env()?;
    for f in [0x1d8bc8, 0x1e93e8, 0x1e2a60, 0x1e2e60] {
        r.allow(f);
    }
    for (f, n) in [(0x1c4650u32, 1), (0x1c4c38, 2), (p::SUBMIT, 4)] {
        let l = log.clone();
        r.stub(f, move |c, mem| {
            let mut w: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            if f == p::SUBMIT {
                w.extend([mem.read_u32(TEXSTATE), mem.read_u32(TEXSTATE + 0x10) & 0xff, mem.read_u32(RENDER + 0x474)]);
            }
            l.borrow_mut().push(Call(f, w));
        });
    }
    for off in SLOTS {
        let (l, a, pk) = (log.clone(), answer.clone(), poke.clone());
        r.stub(vaddr(off), move |c, mem| {
            let n = match off {
                0x1c0 | 0x1e0 | 0x258 => 1,
                _ => 2,
            };
            let mut w: Vec<u32> = (0..=n).map(|k| c.gpr(4 + k) as u32).collect();
            if off == 0x1c8 {
                w.push(c.fpr[12]);
            }
            if let (true, Some((b, v))) = (off == 0x1e8 && w[1] == 0, pk.get()) {
                mem.write(TEXSTATE + 0x10, &[b]).unwrap();
                mem.write(TEXSTATE, &v.to_le_bytes()).unwrap();
            }
            l.borrow_mut().push(Call(vaddr(off), w));
            c.set_gpr(2, if off == 0x280 { a.get() as u64 } else { 0 });
        });
    }
    Some(r)
}

fn fresh(g: &mut Rng, extra: Vec<(u32, Vec<u8>)>) -> Mem {
    let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
    m.regions.extend(extra);
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    m.w32(RENDER + 0x10, RVT);
    for off in SLOTS {
        m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
        m.w32(RVT + off + 4, vaddr(off));
    }
    m
}

fn compare(r: &mut Runner, m: &Mem, case: u32, theirs: &[Call], ours: &[Call]) {
    if theirs != ours {
        let i = theirs.iter().zip(ours).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.len()));
        panic!("case {case}: call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.get(i));
    }
    for (at, b) in &m.regions {
        let mut th = vec![0u8; b.len()];
        r.mem.read(*at, &mut th).unwrap();
        if &th != b {
            let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
            let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
            panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
        }
    }
}

#[test]
fn particle_sys_draw_matches_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let answer = Rc::new(Cell::new(0));
    let poke = Rc::new(Cell::new(None));
    let Some(mut r) = runner(&log, &answer, &poke) else { return };
    let mut blends = vec![0u8; 44];
    r.mem.read(p::BLENDS, &mut blends).unwrap();
    let mut g = Rng(0x0e7e_0000_0000_0001);
    let mut drawn = 0usize;
    let mut kinds = [false; 11];
    for case in 0..5000u32 {
        let mut m = fresh(&mut g, vec![(p::APP, vec![0; 4]), (p::BLENDS, blends.clone())]);
        m.w32(p::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(APPOBJ + 0x30, TEXTABLE);
        let kind = [-2i32, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, i32::MAX, i32::MIN][g.below(17) as usize];
        m.w32(PS + 8, kind as u32);
        m.w32(PS + 4, g.below(0x40));
        answer.set([0, 1, 1, 2][g.below(4) as usize]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), answer.get(), None);
        p::particle_sys_draw(&mut m, &mut ours, PS);
        r.cpu.set_gpr(4, PS as u64);
        r.call(0x1d8bc8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        compare(&mut r, &m, case, &theirs, &ours.0);
        if answer.get() != 1 {
            drawn += 1;
            kinds[kind.clamp(0, 10) as usize] = true;
        }
    }
    println!("{drawn} drawn");
    assert!(drawn > 2000 && kinds.iter().all(|&k| k));
}

#[test]
fn particle_packets_and_blend_match_the_original() {
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let answer = Rc::new(Cell::new(0));
    let poke = Rc::new(Cell::new(None));
    let Some(mut r) = runner(&log, &answer, &poke) else { return };
    let mut g = Rng(0x0e7e_1000_0000_0001);
    let (mut made, mut changed) = (0usize, 0usize);
    for case in 0..9000u32 {
        let mut m = fresh(&mut g, vec![]);
        m.w32(RENDER + 0x470, TEXSTATE);
        m.w32(RENDER + 0x680, PKT + 0x10 * g.below(16));
        m.w32(RENDER + 0x17d0, [0u32, 1, 7][g.below(3) as usize]);
        m.w32(RENDER + 0x3f4, CAM);
        for base in [0x1730u32, 0x4d0, 0x570] {
            for k in 0..16 {
                wf(&mut m, RENDER + base + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else if k >= 12 { g.f(-500.0, 500.0) } else { g.f(-0.5, 0.5) });
            }
        }
        for k in 0..16 {
            wf(&mut m, CAM + 4 * k, if k % 5 == 0 { g.f(0.5, 1.5) } else { g.f(-0.5, 0.5) });
        }
        let which = g.below(3);
        // the texture state: the program (or blend) already the one set, or not; word 0 clear or not
        m.w8(TEXSTATE + 0x10, [0u8, 6, 7, 6, 7, 1][g.below(6) as usize]);
        m.w32(TEXSTATE, [0u32, 0, 0, 1, 0x100][g.below(5) as usize]);
        let (blend, half) = (g.next(), g.next());
        let (blend, half) = match g.below(4) {
            0 => (m.r8(TEXSTATE + 0xe) as u32, m.r16(TEXSTATE + 4) as u32),
            1 => (m.r8(TEXSTATE + 0xe) as u32, half & 0xffff),
            2 => (blend & 0xff, m.r16(TEXSTATE + 4) as u32),
            _ => [(blend & 0xff, half & 0xffff), (m.r8(TEXSTATE + 0xe) as u32 | 0x100, m.r16(TEXSTATE + 4) as u32), (m.r8(TEXSTATE + 0xe) as u32, m.r16(TEXSTATE + 4) as u32 | 0x1_0000)][g.below(3) as usize],
        };
        let block = PS + 0x10 * g.below(4);
        // what the renderer's +0x1e8(0, 0) leaves: the state as it was, or another program or word 0
        poke.set([None, None, Some((0, 0)), Some((0, 5)), Some((3, 0))][g.below(5) as usize]);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), 0, poke.get());
        let before = m.r32(RENDER + 0x474);
        let func = match which {
            0 => {
                p::set_blend(&mut m, RENDER, blend, half);
                for (k, v) in [RENDER, blend, half].into_iter().enumerate() {
                    r.cpu.set_gpr(4 + k, v as u64);
                }
                0x1e93e8
            }
            1 => {
                p::draw_program6(&mut m, &mut ours, RENDER, block);
                0x1e2a60
            }
            _ => {
                p::draw_program7(&mut m, &mut ours, RENDER, block);
                0x1e2e60
            }
        };
        if which != 0 {
            r.cpu.set_gpr(4, RENDER as u64);
            r.cpu.set_gpr(5, block as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        compare(&mut r, &m, case, &theirs, &ours.0);
        made += ours.0.iter().filter(|c| c.0 == 0x1c4650).count();
        changed += (which == 0 && m.r32(RENDER + 0x474) == 1 && before != 1) as usize;
    }
    println!("{made} frames made, {changed} blends changed");
    assert!(made > 1000 && changed > 500);
}

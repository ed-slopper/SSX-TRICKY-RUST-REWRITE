//! `tricky_game::boardfx` (board row G7e5) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random record (the puffs' ring, the brake spray's pool, the trail's ring:
//! how many, the oldest anywhere, ages at and about the draws' turning points) under a random renderer (its
//! vtable's this-shifts) and app. The renderer's virtuals and the other draws are logging stand-ins; +0x240 logs
//! the place and size it is handed, +0x220 the vertices. The memory (bar the port's scratch: the game's stack)
//! and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::boardfx as bf;
use tricky_game::pf::Mem;
use tricky_game::riderdraw::{RiderDrawEnv, APP};

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
const SIZE: u32 = 0x5000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x800;
const TEX: u32 = ARENA + 0xc00;
const GAME: u32 = ARENA + 0xd00;
const MODE: u32 = ARENA + 0xf00;
const REC: u32 = ARENA + 0x1000;
const SCRATCH: u32 = 0x0180_0000;
const SLOTS: [u32; 8] = [0x1c0, 0x1c8, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0x220, 0x240];
const CALLS: [u32; 7] = [bf::PUFF_DRAW, bf::TRAIL_DRAW, bf::SURFACE_SPRAY_DRAW, bf::SPRAY_DRAW, bf::SPEED_DUST_DRAW, bf::BOARD_TRACK_DRAW, bf::SPRAY_PARTICLES_DRAW];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn logged(off: u32, w: &[u32], f12: u32, read: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    let mut v = vec![w[0]];
    match off {
        0x1c8 => v.extend([w[1], w[2], f12]),
        0x1c0 | 0x1e0 => v.push(w[1]),
        0x220 => {
            v.extend([w[1], w[3]]);
            for k in 0..w[1].min(160) {
                for j in [0, 4, 8, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c] {
                    v.push(read(w[2] + 0x30 * k + j));
                }
            }
        }
        0x240 => {
            v.extend([w[3], w[4], w[5]]);
            v.extend((0..4).map(|k| read(w[1] + 4 * k)));
            v.extend((0..4).map(|k| read(w[2] + 4 * k)));
        }
        _ => v.extend([w[1], w[2]]),
    }
    v
}

struct Ours(Vec<Call>);
impl RiderDrawEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.push(Call(func, vec![args[0]]));
        0
    }
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let off = func - vaddr(0);
        let mut w = vec![this];
        w.extend(args);
        let f12 = if off == 0x1c8 { args[2] } else { 0 };
        w.resize(6, 0);
        let mut read = |a: u32| m.r32(a);
        let l = logged(off, &w, f12, &mut read);
        self.0.push(Call(func, l));
        0
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

fn runner() -> Option<(Runner, Rc<RefCell<Vec<Call>>>)> {
    let mut r = Runner::from_env()?;
    r.allow(bf::VEC4_SCALE);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, mem| {
            let w: Vec<u32> = (4..10).map(|k| c.gpr(k) as u32).collect();
            let mut read = |a: u32| mem.read_u32(a);
            let lg = logged(off, &w, c.fpr[12], &mut read);
            l.borrow_mut().push(Call(vaddr(off), lg));
        });
    }
    for func in CALLS {
        let l = log.clone();
        r.stub(func, move |c, _| l.borrow_mut().push(Call(func, vec![c.gpr(4) as u32])));
    }
    Some((r, log))
}

/// A fresh renderer, app and record (all random words)
fn fresh(g: &mut Rng) -> Mem {
    let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4]), (SCRATCH, vec![0; 0x2000])] };
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    m.w32(RENDER + 0x10, RVT);
    for off in SLOTS {
        m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
        m.w32(RVT + off + 4, vaddr(off));
    }
    m.w32(APP, APPOBJ);
    m.w32(APPOBJ + 0x724, RENDER);
    m.w32(APPOBJ + 0x30, TEX);
    m.w32(APPOBJ + 0x730, GAME);
    m
}

/// Runs both on `m` (the game's on the runner's copy) and compares the calls and the memory
fn compare(r: &mut Runner, log: &Rc<RefCell<Vec<Call>>>, m: &mut Mem, case: u32, func: u32, ours: impl FnOnce(&mut Mem, &mut Ours)) -> Vec<Call> {
    for (at, b) in &m.regions {
        if *at != SCRATCH {
            r.mem.write(*at, b).unwrap();
        }
    }
    log.borrow_mut().clear();
    let mut o = Ours(Vec::new());
    ours(m, &mut o);
    r.cpu.set_gpr(4, REC as u64);
    r.call(func).unwrap_or_else(|e| panic!("case {case}: {e}"));
    let theirs = log.borrow().clone();
    if theirs != o.0 {
        let i = theirs.iter().zip(&o.0).position(|(x, y)| x != y).unwrap_or(theirs.len().min(o.0.len()));
        let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(24)])).unwrap_or_default();
        panic!("case {case}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.0.len(), show(theirs.get(i)), show(o.0.get(i)));
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

#[test]
fn draw_effects_matches_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let mut g = Rng(0xb0fe_0000_0000_0001);
    let mut tracks = 0usize;
    for case in 0..2000u32 {
        let mut m = fresh(&mut g);
        m.w32(GAME + 0x298, [0u32, 1, 2, 3, 0xffff_ffff][g.below(5) as usize]);
        m.w32(GAME + 0x1c, MODE);
        m.w32(MODE + 0x88, [0u32, 2, 3, 4, 0x8000_0000][g.below(5) as usize]);
        let theirs = compare(&mut r, &log, &mut m, case, 0x135170, |m, o| bf::draw_effects(m, o, REC));
        tracks += theirs.iter().any(|c| c.0 == bf::BOARD_TRACK_DRAW) as usize;
    }
    println!("{tracks} with the track");
    assert!(tracks > 300 && tracks < 1800);
}

#[test]
fn puff_draw_matches_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let mut g = Rng(0xb0fe_1000_0000_0001);
    let (mut drawn, mut young, mut least) = (0usize, 0usize, 0usize);
    for case in 0..3000u32 {
        let mut m = fresh(&mut g);
        m.w32(REC + 0x1c10, [0u32, 1, 3, 8, 64, g.below(64)][g.below(6) as usize]);
        m.w32(REC + 0x1c14, g.below(64));
        for k in 0..64u32 {
            let p = REC + 0x10 + 0x70 * k;
            let life = [1.0f32, 2.0, 0.5, g.f(0.1, 3.0)][g.below(4) as usize];
            let fade = [0.0f32, life * 0.5, g.f(0.0, life * 0.9)][g.below(3) as usize];
            let age = [0.0f32, fade, life, life + 0.5, g.f(0.0, life)][g.below(5) as usize];
            wf(&mut m, p, age);
            wf(&mut m, p + 0x5c, life);
            wf(&mut m, p + 0x58, fade);
            wf(&mut m, p + 0x44, g.f(0.0, 1.0));
            wf(&mut m, p + 0x60, [0.0f32, 1.0, g.f(0.0, 40.0)][g.below(3) as usize]);
            wf(&mut m, p + 0x64, g.f(0.0, 80.0));
            m.w32(p + 0x40, g.below(8));
            for j in 0..8 {
                wf(&mut m, p + 0x20 + 4 * j, [g.f(-500.0, 500.0), 0.0, 1.0][g.below(3) as usize]);
            }
        }
        let theirs = compare(&mut r, &log, &mut m, case, bf::PUFF_DRAW, |m, o| bf::puff_draw(m, o, REC, SCRATCH));
        let sprites: Vec<&Call> = theirs.iter().filter(|c| c.0 == vaddr(0x240)).collect();
        drawn += sprites.len();
        for s in sprites {
            let p = s.1[3] - 0x48;
            let (age, fade) = (f32::from_bits(m.r32(p)), f32::from_bits(m.r32(p + 0x58)));
            young += (age < fade) as usize;
            least += (s.1[8] == m.r32(p + 0x60)) as usize;
        }
    }
    println!("{drawn} puffs, {young} young, {least} at the least size");
    assert!(drawn > 10000 && young > 1000 && least > 1000);
}

#[test]
fn spray_draw_matches_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let mut g = Rng(0xb0fe_2000_0000_0001);
    let (mut drawn, mut early) = (0usize, 0usize);
    for case in 0..3000u32 {
        let mut m = fresh(&mut g);
        m.w32(REC + 0x960, [0u32, 1, 5, 30, g.below(31)][g.below(5) as usize]);
        m.w32(REC + 0x964, g.below(30));
        for k in 0..30u32 {
            let e = REC + 0x50 * k;
            let t = [0.0f32, 0.2, 2.0, 1.0, g.f(0.0, 0.2), g.f(0.2, 2.0)][g.below(6) as usize];
            early += (t < 0.2) as usize;
            wf(&mut m, e + 0x40, t);
            wf(&mut m, e + 0x48, g.f(0.0, 1.0));
            m.w32(e + 0x44, g.below(8));
            for j in 0..8 {
                wf(&mut m, e + 4 * j, [g.f(-500.0, 500.0), 0.0, 1.0][g.below(3) as usize]);
            }
        }
        let theirs = compare(&mut r, &log, &mut m, case, bf::SPRAY_DRAW, |m, o| bf::spray_draw(m, o, REC, SCRATCH));
        drawn += theirs.iter().filter(|c| c.0 == vaddr(0x240)).count();
    }
    println!("{drawn} sprays");
    assert!(drawn > 10000 && early > 10000);
}

#[test]
fn trail_draw_matches_the_original() {
    let Some((mut r, log)) = runner() else { return };
    let mut g = Rng(0xb0fe_3000_0000_0001);
    let (mut strips, mut broken) = (0usize, 0usize);
    for case in 0..3000u32 {
        let mut m = fresh(&mut g);
        m.w32(REC + 0x50, [0u32, 1, 2, 3, 75, g.below(12), g.below(76)][g.below(7) as usize]);
        m.w32(REC + 0x54, g.below(75));
        for j in 0..4 {
            wf(&mut m, REC + 0x20 + 4 * j, g.f(-500.0, 500.0));
        }
        let breaks = [0u32, 2, 5][g.below(3) as usize];
        for k in 0..75u32 {
            let e = REC + 0x60 + 0xa0 * k;
            for j in 0..16 {
                wf(&mut m, e + 4 * j, [g.f(-500.0, 500.0), 0.0][g.below(2) as usize]);
            }
            for j in 0..4 {
                wf(&mut m, e + 0x80 + 4 * j, [0.0f32, 1.0, g.f(0.0, 2.0)][g.below(3) as usize]);
            }
            wf(&mut m, e + 0x90, g.f(0.0, 20.0));
            wf(&mut m, e + 0x94, [0.0f32, 0.9, g.f(0.0, 0.9)][g.below(3) as usize]);
            m.w32(e + 0x9c, (breaks != 0 && g.below(breaks) == 0) as u32 * [1u32, 0x100, 0x8000_0000][g.below(3) as usize]);
        }
        let theirs = compare(&mut r, &log, &mut m, case, bf::TRAIL_DRAW, |m, o| bf::trail_draw(m, o, REC, SCRATCH));
        let n = theirs.iter().filter(|c| c.0 == vaddr(0x220)).count();
        strips += n;
        broken += (n > 4) as usize;
    }
    println!("{strips} strips, {broken} broken trails");
    assert!(strips > 5000 && broken > 300);
}

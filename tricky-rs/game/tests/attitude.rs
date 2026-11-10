//! `tricky_game::attitude` (board row G7g5) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random icon (its points; both slots shown or not, moving, fixed, squashing or
//! settling, how far, their kinds), the camera's rider (the career player's or not) and view (the icon behind,
//! near, between and far), under a random renderer (its vtable's this-shifts). The calls out are logging
//! stand-ins answering as the case says. `Vec4_Scale` runs as the game's. The memory and the calls compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::attitude as at;
use tricky_game::pf::Mem;
use tricky_game::riderdraw::{RiderDrawEnv, APP, FOCUS_RIDER};

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
const RVT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x800;
const TEX: u32 = ARENA + 0xa00;
const GAME: u32 = ARENA + 0xc00;
const FOCUS: u32 = ARENA + 0x1000;
const ICON: u32 = ARENA + 0x2000;
const VIEW: u32 = ARENA + 0x3000;
const SLOTS: [u32; 7] = [0xf8, 0x1a0, 0x1a8, 0x1c8, 0x1e8, 0x1f0, 0x240];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn logged(func: u32, w: &[u32], f12: u32, player: u32) -> (Vec<u32>, u32) {
    match func {
        FOCUS_RIDER => (vec![w[0]], FOCUS),
        at::FIND_RIDER_OF_PLAYER => (vec![w[0], w[1]], player),
        f => {
            let off = f - vaddr(0);
            let v = match off {
                0xf8 | 0x1a0 | 0x1a8 => vec![w[0]],
                0x1c8 => vec![w[0], w[1], w[2], f12],
                0x240 => w[..6].to_vec(),
                _ => w[..3].to_vec(),
            };
            (v, if off == 0xf8 { VIEW } else { 0 })
        }
    }
}

struct Ours(Vec<Call>, u32);
impl RiderDrawEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (l, a) = logged(func, &w, 0, self.1);
        self.0.push(Call(func, l));
        a
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let mut w = vec![this];
        w.extend(args);
        let f12 = if func == vaddr(0x1c8) { args[2] } else { 0 };
        w.resize(8, 0);
        let (l, a) = logged(func, &w, f12, self.1);
        self.0.push(Call(func, l));
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn attitude_icon_draw_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x134830u32, 0x102f50] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let player = Rc::new(Cell::new(0u32));
    for func in [FOCUS_RIDER, at::FIND_RIDER_OF_PLAYER].into_iter().chain(SLOTS.map(vaddr)) {
        let (l, p) = (log.clone(), player.clone());
        r.stub(func, move |c, _| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (lg, a) = logged(func, &w, c.fpr[12], p.get());
            l.borrow_mut().push(Call(func, lg));
            c.set_gpr(2, a as u64);
        });
    }
    let mut g = Rng(0xa771_0000_0000_0001);
    let mut kinds = [0usize; 5];
    for case in 0..8000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4])] };
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
        m.w32(APPOBJ + 0x730, GAME);
        m.w32(APPOBJ + 0x30, TEX);
        m.w32(GAME + 0x290, g.below(2));
        m.w32(FOCUS + 0x460, g.below(2));
        player.set(g.below(2));
        // the view: rows near a camera's, the depth's row picking the icon's depth
        for i in 0..4 {
            for j in 0..4 {
                wf(&mut m, VIEW + 0x10 * i + 4 * j, if i == j { 1.0 } else { g.f(-0.2, 0.2) });
            }
        }
        // (sometimes the depth exactly the icon's z: the third column (0, 0, 1, 0), so the bounds are met exactly)
        if g.below(2) == 0 {
            for i in [0u32, 1, 3] {
                wf(&mut m, VIEW + 0x10 * i + 8, 0.0);
            }
        }
        for k in 0..16 {
            wf(&mut m, ICON + 4 * k, g.f(-200.0, 200.0));
        }
        for s in 0..2u32 {
            let slot = ICON + 0x80 * s;
            for k in 0..4 {
                wf(&mut m, slot + 0x60 + 4 * k, if k == 3 { 1.0 } else { g.f(-100.0, 100.0) });
            }
            wf(&mut m, slot + 0x68, [-50.0f32, 0.0, 300.0, 500.0, 1500.0, 2900.0, 3200.0, 3700.1, 4000.0, f32::from_bits(0x4567_4141), 3000.0, g.f(0.0, 3700.0)][g.below(12) as usize]);
            m.w32(slot + 0x98, (g.below(5) != 0) as u32);
            m.w32(slot + 0xac, g.below(2));
            m.w32(slot + 0xbc, g.below(2));
            m.w32(slot + 0xa0, g.below(2));
            m.w32(slot + 0xc0, g.below(4));
            wf(&mut m, slot + 0x50, [0.0f32, 0.5, 1.0, 1.7, 2.0, 2.9, g.f(0.0, 3.0)][g.below(7) as usize]);
            wf(&mut m, slot + 0xb0, [0.0f32, 0.05, 0.1, 0.5, 1.0, g.f(0.0, 1.0)][g.below(6) as usize]);
            wf(&mut m, slot + 0xa4, [0.0f32, 0.2, 0.33333334, 0.5, 0.6666667, 0.9, g.f(0.0, 1.0)][g.below(7) as usize]);
            wf(&mut m, slot + 0xb8, g.f(0.0, 1.0));
        }
        for (a, b) in &m.regions {
            r.mem.write(*a, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), player.get());
        at::draw(&mut m, &mut ours, ICON);
        r.cpu.set_gpr(4, ICON as u64);
        r.call(0x134830).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case}: the calls");
        for (a, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*a, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", a + i as u32, w(&th), w(b));
            }
        }
        if theirs.iter().any(|c| c.0 == vaddr(0x240)) {
            let slot = ICON + 0x80 * (m.r32(FOCUS + 0x460) != player.get()) as u32;
            let k = if m.r32(slot + 0xac) != 0 { if m.r32(slot + 0xbc) != 0 { 1 } else { 0 } } else if m.r32(slot + 0xa0) != 0 { 2 } else { 3 };
            kinds[k] += 1;
            kinds[4] += (f32::from_bits(m.r32(slot + 0x68)) > 2777.4) as usize;
        }
    }
    println!("drawn moving {}, moving fixed {}, squashing {}, settling {}, fading {}", kinds[0], kinds[1], kinds[2], kinds[3], kinds[4]);
    assert!(kinds.iter().all(|&n| n > 150));
}

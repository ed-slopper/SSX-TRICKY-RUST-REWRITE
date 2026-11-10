//! `tricky_game::shadow` (board row G7g1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random shadow, race size (1 rider or more), texture (none yet or one) and
//! renderer (its vtable's this-shifts); the renderer's virtuals are logging stand-ins (the draw's logs the place
//! it is handed, read from its pointer; the texture's answers a texture). The memory (bar the port's scratch:
//! the game's stack) and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::particledraw::{ParticleDrawEnv, APP};
use tricky_game::pf::Mem;
use tricky_game::shadow as s;

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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x800;
const APPOBJ: u32 = ARENA + 0xc00;
const SHADOW: u32 = ARENA + 0x1000;
const SCRATCH: u32 = 0x0180_0000;
const TEX_ANSWER: u32 = 0x1234;
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>);
impl CullEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
}
impl ParticleDrawEnv for Ours {
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let mut w = vec![this];
        if func == vaddr(0x2d8) {
            // the place (read from its pointer), +0x90, +0xd0, the texture, f12
            w.extend((0..4).map(|k| m.r32(args[0] + 4 * k)));
            w.extend(&args[1..]);
        } else {
            w.extend(args);
        }
        self.0.push(Call(func, w));
        TEX_ANSWER
    }
}

#[test]
fn shadow_set_up_and_draw_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1c8c38u32, 0x1c8c70, 0x1c8bc8] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let l = log.clone();
    r.stub(vaddr(0x170), move |c, _| {
        l.borrow_mut().push(Call(vaddr(0x170), (4..8).map(|k| c.gpr(k) as u32).collect()));
        c.set_gpr(2, TEX_ANSWER as u64);
    });
    let l = log.clone();
    r.stub(vaddr(0x2d8), move |c, mem| {
        let mut w = vec![c.gpr(4) as u32];
        let at = c.gpr(5) as u32;
        w.extend((0..4).map(|k| mem.read_u32(at + 4 * k)));
        w.extend([c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.fpr[12]]);
        l.borrow_mut().push(Call(vaddr(0x2d8), w));
    });
    let mut g = Rng(0x5ad0_0000_0000_0001);
    let (mut allocated, mut big) = (0usize, 0usize);
    for case in 0..6000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4]), (s::ENTRY_COUNT, vec![0; 4]), (SCRATCH, vec![0; 0x10])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(RENDER + 0x10, RVT);
        for off in [0x170u32, 0x2d8] {
            m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(s::ENTRY_COUNT, [0u32, 1, 1, 2, 4][g.below(5) as usize]);
        // which: init then the texture, the texture alone (none yet or one), or the draw
        let which = g.below(4);
        if which == 1 {
            m.w32(SHADOW + 0x170, [u32::MAX, 0x8000_0000, 0, 7][g.below(4) as usize]);
        }
        for (at, b) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, b).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        r.cpu.set_gpr(4, SHADOW as u64);
        match which {
            0 => {
                assert_eq!(s::init(&mut m, SHADOW), SHADOW);
                s::alloc_texture(&mut m, &mut ours, SHADOW);
                r.call(0x1c8c38).unwrap_or_else(|e| panic!("case {case}: {e}"));
                assert_eq!(r.cpu.gpr(2) as u32, SHADOW, "case {case}: init's answer");
                r.cpu.set_gpr(4, SHADOW as u64);
                r.call(0x1c8c70).unwrap_or_else(|e| panic!("case {case}: {e}"));
                big += (m.r32(SHADOW + 0x174) == 0x100) as usize;
            }
            1 | 2 => {
                s::alloc_texture(&mut m, &mut ours, SHADOW);
                r.call(0x1c8c70).unwrap_or_else(|e| panic!("case {case}: {e}"));
            }
            _ => {
                s::draw(&mut m, &mut ours, SHADOW, SCRATCH);
                r.call(0x1c8bc8).unwrap_or_else(|e| panic!("case {case}: {e}"));
            }
        }
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case}: the calls");
        allocated += theirs.iter().filter(|c| c.0 == vaddr(0x170)).count();
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                panic!("case {case}: memory at {:#x} differs", at + i as u32);
            }
        }
    }
    println!("{allocated} textures asked for, {big} large");
    assert!(allocated > 1500 && big > 300);
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}
impl Rng {
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[test]
fn shadow_update_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1c7e90u32, 0x137108, 0x137140] {
        r.allow(f);
    }
    let mut tables = vec![0u8; 0x28];
    r.mem.read(s::BONES, &mut tables).unwrap();
    const POS: u32 = ARENA + 0x200;
    const DIR: u32 = ARENA + 0x210;
    const UP: u32 = ARENA + 0x220;
    const BONES: u32 = ARENA + 0x1200;
    const POINTS: u32 = ARENA + 0x1700;
    let mut g = Rng(0x5ad1_0000_0000_0001);
    let mut parallel = 0usize;
    for case in 0..5000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (s::BONES, tables.clone())] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the shadow's words: the floats it keeps from before (side, third axis) real; its texture's size
        for k in 0..0x180 / 4 {
            wf(&mut m, SHADOW + 4 * k, g.f(-2.0, 2.0));
        }
        m.w32(SHADOW + 0x174, [0x80u32, 0x100][g.below(2) as usize]);
        m.w32(SHADOW + 0x178, [0x80u32, 0x100][g.below(2) as usize]);
        let mut c = [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-500.0, 500.0)];
        // (sometimes a lane −0: the eye is the place less up × 0, which then shows)
        if g.below(4) == 0 {
            c[g.below(3) as usize] = -0.0;
        }
        for k in 0..4 {
            wf(&mut m, POS + 4 * k, if k < 3 { c[k as usize] } else { 1.0 });
        }
        let up = match g.below(4) {
            0 => [0.0, 0.0, 1.0],
            1 => [0.0, 0.0, 0.0],
            _ => [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(0.2, 2.0)],
        };
        let dir = match g.below(6) {
            0 => up,
            1 => up.map(|x| -x),
            2 => [0.0, 0.0, 0.0],
            3 => [0.3, 0.2, -1.0],
            _ => [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-2.0, -0.1)],
        };
        for k in 0..3 {
            wf(&mut m, UP + 4 * k, up[k as usize]);
            wf(&mut m, DIR + 4 * k, dir[k as usize]);
        }
        // the rider's bones' places and the board's points about the rider
        for b in 0..20u32 {
            for k in 0..4 {
                wf(&mut m, BONES + 0x40 * b + 0x30 + 4 * k, if k < 3 { c[k as usize] + g.f(-60.0, 60.0) } else { 1.0 });
            }
        }
        for p in 0..4u32 {
            for k in 0..4 {
                wf(&mut m, POINTS + 0x10 * p + 4 * k, if k < 3 { c[k as usize] + g.f(-80.0, 80.0) } else { 1.0 });
            }
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        s::update(&mut m, SHADOW, POS, DIR, UP, BONES, POINTS);
        for (i, v) in [SHADOW, POS, DIR, UP, BONES, POINTS].into_iter().enumerate() {
            r.cpu.set_gpr(4 + i, v as u64);
        }
        r.call(0x1c7e90).unwrap_or_else(|e| panic!("case {case}: {e}"));
        parallel += (up != [0.0; 3] && (dir == up || dir == up.map(|x: f32| -x))) as usize;
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: memory at {:#x} (shadow +{:#x}): theirs {:#x} ours {:#x}", at + i as u32, (at + i as u32).wrapping_sub(SHADOW), w(&th), w(b));
            }
        }
    }
    println!("{parallel} parallel");
    assert!(parallel > 500);
}

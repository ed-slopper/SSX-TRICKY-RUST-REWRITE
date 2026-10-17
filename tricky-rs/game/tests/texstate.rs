//! `tricky_game::drawlist::write_tex_state` (board row G7h3) against the original in the function runner
//! (TRICKY_ELF; skipped without it): every case a fresh list (its matrix sets random, any floats; which is
//! current), a Bezier manager behind the renderer's +0x20 (a logging stand-in answering it), then
//! `PS2Draw_WriteTexState` with any program (past 10 too), how (−1..3) and word (0 or not). The memory and the
//! calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::drawlist::{self as dl, DrawListEnv};
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
const SIZE: u32 = 0x12000;
const D: u32 = ARENA;
const APP: u32 = ARENA + 0x10000;
const R: u32 = ARENA + 0x10800;
const RVT: u32 = ARENA + 0x10900;
const BZ: u32 = ARENA + 0x10a00;
const OUT: u32 = ARENA + 0x11000;
const SLOT: u32 = ARENA + 0x11f00;
const GLOBALS: (u32, u32) = (0x337c58, 0x10);
const GET_BZ: u32 = 0x01ff_0000;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>);
impl DrawListEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, _args: &[u32]) -> u32 {
        self.0.push(Call(func, vec![this]));
        BZ
    }
}

#[test]
fn tex_state_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1c5c88);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let l = log.clone();
    r.stub(GET_BZ, move |c, _| {
        l.borrow_mut().push(Call(GET_BZ, vec![c.gpr(4) as u32]));
        c.set_gpr(2, BZ as u64);
    });
    let mut g = Rng(0x0d71_3000_0000_0001);
    let mut seen = [0usize; 12];
    for case in 0..30_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (GLOBALS.0, vec![0; GLOBALS.1 as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the matrix sets' floats: real ones mostly (the z ÷ 16 of +0x168 and +0x178 among them), any bits else
        for set in 0..8u32 {
            for k in 0..0x60u32 {
                if g.below(4) != 0 {
                    let v = [g.f(-100.0, 100.0), 0.0, 1.0, 1.0e-37][g.below(4) as usize];
                    m.w32(D + 0x20 + 0x180 * set + 4 * k, v.to_bits());
                }
            }
        }
        m.w32(D + 0xfa5c, g.below(8));
        m.w32(GLOBALS.0, APP);
        m.w32(APP + 0x724, R);
        m.w32(R + 0x10, RVT);
        m.w16(RVT + 0x20, [0u16, 0x10][g.below(2) as usize]);
        m.w32(RVT + 0x24, GET_BZ);
        m.w32(SLOT, OUT + 0x10 * g.below(4));
        for (at, bytes) in &m.regions {
            r.mem.write(*at, bytes).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let prog = g.below(13);
        seen[prog.min(11) as usize] += 1;
        let how = g.below(5).wrapping_sub(1);
        let word = [0, g.next()][g.below(2) as usize];
        dl::write_tex_state(&mut m, &mut ours, D, prog, how, word, SLOT);
        for (k, v) in [D, prog, how, word, SLOT].into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(0x1c5c88).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(*log.borrow(), ours.0, "case {case} program {prog} how {how}: the calls");
        for (at, bytes) in &m.regions {
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} program {prog} how {how}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
    }
    println!("programs {seen:?}");
}

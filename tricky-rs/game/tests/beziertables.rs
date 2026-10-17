//! `tricky_game::bezier`'s tables (board row G7a3c) against the original in the function runner (TRICKY_ELF;
//! skipped without it): every case a fresh random manager (its old tables or none, its +0x28 any float), the
//! allocator answering 0 or a place; then `Render_BuildBezierTables`, `Render_FillBezierWeights` at a random place,
//! or `StaticInit_Render`. The allocator, the fill and the free are logging stand-ins. The memory (bar the port's
//! scratch: the game's stack), the calls compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bezier::{self as b, BezierEnv};
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
const SIZE: u32 = 0x2000;
const MAN: u32 = ARENA;
const SLOT: u32 = ARENA + 0x600;
const TABLES: u32 = ARENA + 0x800;
const SCRATCH: u32 = 0x0180_0000;
const PROGS: (u32, u32) = (0x3d0f10, 0x60);

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>, u32);
impl BezierEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.push(Call(func, args.to_vec()));
        if func == b::ALLOC {
            self.1
        } else {
            0
        }
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        panic!("no virtuals: {func:#x} {this:#x} {args:?}")
    }
}

#[test]
fn bezier_tables_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1db1e8u32, 0x1db530, 0x1db640] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let answer: Rc<Cell<u32>> = Rc::default();
    for (f, n) in [(b::ALLOC, 3), (b::MEM_SET, 3), (b::MEM_FREE, 1)] {
        let (l, a) = (log.clone(), answer.clone());
        r.stub(f, move |c, _| {
            l.borrow_mut().push(Call(f, (0..n).map(|k| c.gpr(4 + k) as u32).collect()));
            c.set_gpr(2, if f == b::ALLOC { a.get() as u64 } else { 0 });
        });
    }
    let mut g = Rng(0x0b3e_2c00_0000_0001);
    let mut kinds = [0usize; 3];
    for case in 0..6000u32 {
        let mut m = Mem { regions: vec![(0, vec![0; 0xe00]), (ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x10]), (PROGS.0, vec![0; PROGS.1 as usize])] };
        for (at, len) in [(ARENA, SIZE), (PROGS.0, PROGS.1)] {
            for k in 0..len / 4 {
                m.w32(at + 4 * k, g.next());
            }
        }
        let f = [g.f(0.0, 16.0), 8.0, g.f(-1.0e6, 1.0e6), 1.0e-38, f32::from_bits(g.next())][g.below(5) as usize];
        m.w32(MAN + 0x28, f.to_bits());
        m.w32(MAN + 0x530, [0, g.next()][g.below(2) as usize]);
        m.w32(SLOT, TABLES + 0x10 * g.below(16) + 4 * g.below(4));
        let alloc = [0, TABLES + 0x10 * g.below(8)][g.below(5).min(1) as usize];
        answer.set(alloc);
        for (at, bytes) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, bytes).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), alloc);
        let kind = g.below(3);
        kinds[kind as usize] += 1;
        let (func, args) = match kind {
            0 => {
                b::build_tables(&mut m, &mut ours, MAN, SCRATCH);
                (0x1db1e8, [MAN, 0])
            }
            1 => {
                b::fill_weights(&mut m, SLOT);
                (0x1db530, [MAN, SLOT])
            }
            _ => {
                b::static_init(&mut m);
                (0x1db640, [0, 0])
            }
        };
        r.cpu.set_gpr(4, args[0] as u64);
        r.cpu.set_gpr(5, args[1] as u64);
        r.call(func).unwrap_or_else(|e| panic!("case {case} kind {kind}: {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case} kind {kind}: the calls");
        for (at, bytes) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} kind {kind} (+0x28 {f:e}): memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
    }
    println!("kinds {kinds:?}");
}

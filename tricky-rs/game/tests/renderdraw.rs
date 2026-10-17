//! `tricky_game::renderdraw` (board row G7l) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random renderer (its texture state's program and first texture, the packet
//! place) and vertices, with the counts at and around the chunks' bounds. The matrix update and the submit are
//! logging stand-ins. The memory and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::renderdraw as rd;
use tricky_game::riderdraw::RiderDrawEnv;

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
const SIZE: u32 = 0x2_0000;
const R: u32 = ARENA;
const TS: u32 = ARENA + 0x2000;
const VERTS: u32 = ARENA + 0x4000;
const PKT: u32 = ARENA + 0x1_0000;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// A call's log; the submit's with the texture state as it is then (its word 0 and program, and the mark), which
/// the draw's end puts back
fn logged(func: u32, w: &[u32], read: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    match func {
        rd::SUBMIT => {
            let mut v = w[..4].to_vec();
            v.extend([read(TS), read(TS + 0x10) & 0xff, read(R + 0x474)]);
            v
        }
        _ => w[..1].to_vec(),
    }
}

struct Ours(Vec<Call>);
impl RiderDrawEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        self.0.push(Call(func, logged(func, &w, &mut |a| m.r32(a))));
        0
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, _this: u32, _args: &[u32]) -> u32 {
        panic!("no virtual expected: {func:#x}");
    }
}

#[test]
fn strips_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1de7c8);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in [rd::MATRIX_UPDATE, rd::SUBMIT] {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            l.borrow_mut().push(Call(func, logged(func, &w, &mut |a| mem.read_u32(a))));
        });
    }
    let mut g = Rng(0x5791_0000_0000_0001);
    let mut chunks = 0usize;
    for case in 0..2000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(R + 0x470, TS);
        m.w32(R + 0x474, 0);
        m.w8(TS + 0x10, [0u8, 5, 5, 3][g.below(4) as usize]);
        m.w32(TS, [0u32, 0, 9][g.below(3) as usize]);
        m.w16(TS + 8, [0u16, 3, 0xffff, 0x8000][g.below(4) as usize]);
        m.w32(R + 0x680, PKT + 0x10 * g.below(16));
        let n = [0u32, 1, 2, 3, 71, 72, 73, 74, 75, 144, 145, 146, 147, 216, 217, 218, 250][g.below(17) as usize];
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        rd::strips(&mut m, &mut ours, R, n, VERTS);
        r.cpu.set_gpr(4, R as u64);
        r.cpu.set_gpr(5, n as u64);
        r.cpu.set_gpr(6, VERTS as u64);
        r.call(0x1de7c8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case} (n {n}): the calls");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} (n {n}): memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        chunks += (n as usize).saturating_sub(2).div_ceil(71).max((n > 0) as usize);
    }
    println!("{chunks} chunks");
}

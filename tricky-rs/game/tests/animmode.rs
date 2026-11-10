//! `tricky_game::animmode` (board row G5b) against the original in the function runner (TRICKY_ELF; skipped without
//! it): every case a fresh random rider (its board class, riding switch, table row, crouch, brake, steer, wind-ups,
//! direction and lean, often exactly 0, ½ or 1) and sequence (its source trees and headers, the key table), then one
//! of the eight modes with a random dt; the game's own tables, the sequences' functions and `Math_Atan` run as the
//! original; delete and the tree maker are logging stand-ins. The memory and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animmode as md;
use tricky_game::animseq::{self as a, SeqEnv};
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
    /// A rider input: often exactly 0 (or a special value), else in the range
    fn input(&mut self, lo: f32, hi: f32, special: &[f32]) -> f32 {
        match self.below(4) {
            0 => 0.0,
            1 if !special.is_empty() => special[self.below(special.len() as u32) as usize],
            _ => self.f(lo, hi),
        }
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const CTL: u32 = ARENA;
const RIDER: u32 = ARENA + 0x100;
const SEQ: u32 = ARENA + 0x800;
const TREES: u32 = ARENA + 0x1000;
const NTREE: u32 = 24;
const HDRS: u32 = ARENA + 0x1400;
const NHDR: u32 = 12;
const KEYS: u32 = a::KEY_TABLE;
const NKEY: u32 = 0x584;
const TABLES: u32 = 0x379180;
const TABLES_LEN: u32 = 0x4a0;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
}

fn answer(f: u32, args: &[u32]) -> u32 {
    match f {
        a::SKEL_ANIM_CREATE => TREES + 0x20 * (args[1] % NTREE),
        _ => 0,
    }
}

struct Ours(World);
impl SeqEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        answer(func, args)
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

fn make_world(g: &mut Rng, m: &mut Mem, none: u32) {
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    for h in 0..NHDR {
        let at = HDRS + 0x30 * h;
        m.w8(at + 9, g.below(3) as u8);
        let len = 1 + g.below(40);
        m.w16(at + 0xa, len as u16);
        for k in 0..8 {
            let v = if g.below(3) == 0 { 0xffff } else { g.below(len + 2) };
            m.w16(at + 0xc + 2 * k, v as u16);
        }
    }
    for t in 0..NTREE {
        let at = TREES + 0x20 * t;
        m.w32(at + 8, HDRS + 0x30 * g.below(NHDR));
        for k in 0..2 {
            let later = NTREE - 1 - t;
            let child = if later == 0 || g.below(3) == 0 { 0 } else { TREES + 0x20 * (t + 1 + g.below(later)) };
            m.w32(at + 0xc + 4 * k, child);
        }
    }
    for k in 0..NKEY {
        let v = if g.below(3) == 0 { none } else { g.below(40) | g.below(3) << 16 };
        m.w32(KEYS + 4 * k, v);
    }
    m.w32(CTL + 0x24, RIDER);
    // the rider
    m.w32(RIDER + 0x1b4, if g.below(2) == 0 { 0 } else { 1 + g.below(3) });
    m.w32(RIDER + 0x1dc, g.below(5));
    m.w32(RIDER + 0x420, g.below(3));
    wf(m, RIDER + 0x1f0, g.f(0.0, 1.0));
    wf(m, RIDER + 0x1fc, g.input(-1.0, 1.0, &[1.0, -1.0, 0.5]));
    wf(m, RIDER + 0x208, g.input(0.0, 1.0, &[1.0, 0.5, 0.25, 0.75]));
    wf(m, RIDER + 0x214, g.input(-1.0, 1.0, &[1.0, -1.0]));
    for o in [0x220, 0x22c, 0x27c, 0x280] {
        wf(m, RIDER + o, g.input(-1.2, 1.2, &[1.0, -1.0, 0.70710677]));
    }
    wf(m, RIDER + 0x238, g.f(0.0, 1.0));
    wf(m, RIDER + 0x268, g.input(-1.0, 1.0, &[1.0, -1.0]));
    // the sequence
    let s = SEQ;
    m.w32(s + 0x34, a::BDR_VTABLE);
    wf(m, s + 0x10, [1.0, -1.0, 0.5, g.f(-2.0, 2.0)][g.below(4) as usize]);
    m.w32(s + 0x20, g.below(2));
    m.w32(s + 0x44, 1 + g.below(3));
    for i in 0..3 {
        let t = match g.below(3) {
            0 => g.below(45) as f32,
            1 => 0.0,
            _ => g.f(-3.0, 45.0),
        };
        wf(m, s + 0x48 + 4 * i, t);
        m.w32(s + 0x78 + 4 * i, TREES + 0x20 * g.below(NTREE));
        m.w32(s + 0x84 + 4 * i, if g.below(4) == 0 { none } else { g.below(40) | g.below(3) << 16 });
    }
    m.w32(s + 0x60, if g.below(2) == 0 { u32::MAX } else { g.below(45) });
}

#[test]
fn the_motion_modes_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x203a40u32, 0x203cf8, 0x203d38, 0x159a80, 0x159bb8, 0x159e80, 0x159ee8, 0x15a030, 0x2510f8, 0x160140, 0x1601d0,
        0x1605a0, 0x160680, 0x160af0, 0x160b20, 0x161118, 0x161300,
    ] {
        r.allow(f);
    }
    let world = Rc::new(RefCell::new(World::default()));
    for (f, n) in [(a::DELETE, 1usize), (a::SKEL_ANIM_CREATE, 2)] {
        let w = world.clone();
        r.stub(f, move |c, _| {
            let args: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            w.borrow_mut().log.push(Call(f, args.clone()));
            c.set_gpr(2, answer(f, &args) as i32 as i64 as u64);
        });
    }
    let mut consts = Vec::new();
    for (at, len) in [(0x379018u32, 0x38u32), (0x39e468, 0x40), (TABLES, TABLES_LEN)] {
        let mut b = vec![0u8; len as usize];
        r.mem.read(at, &mut b).unwrap();
        consts.push((at, b));
    }
    let none = u32::from_le_bytes(consts[1].1[0..4].try_into().unwrap());
    let mut g = Rng(0x1601_0000_0000_0001);
    let funcs = [0x160680u32, 0x1605a0, 0x161300, 0x161118, 0x160140, 0x1601d0, 0x160b20, 0x160af0];
    let mut counts = [0usize; 8];
    for case in 0..60_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (KEYS, vec![0; 4 * NKEY as usize])] };
        m.regions.extend(consts.iter().cloned());
        make_world(&mut g, &mut m, none);
        for (at, b) in &m.regions[..2] {
            r.mem.write(*at, b).unwrap();
        }
        let kind = g.below(8) as usize;
        let f = funcs[kind];
        let dt = [1.0, 2.0, 0.5, g.f(0.0, 3.0)][g.below(4) as usize];
        world.borrow_mut().log.clear();
        let mut ours = Ours(World::default());
        assert!(md::run(&mut m, &mut ours, f, CTL, SEQ, F(dt)));
        counts[kind] += 1;
        r.cpu.set_gpr(4, CTL as u64);
        r.cpu.set_gpr(5, SEQ as u64);
        r.cpu.set_f(12, dt);
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        assert_eq!(world.borrow().log, ours.0.log, "case {case}: {f:#x} calls");
        for (at, b) in &m.regions[..2] {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}");
}

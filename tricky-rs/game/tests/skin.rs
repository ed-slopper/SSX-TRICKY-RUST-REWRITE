//! `tricky_game::skin` (board row G7g3) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random rider skin (parts with 0 to 6 bones, their slots, bones and bind
//! matrices of real sizes, the LOD), board and skin parts (angles through every quarter, skeletons to copy
//! from), under a random renderer (its vtable's this-shifts). The renderer's virtuals are logging stand-ins that
//! read what they are handed (the parts' table, the board's matrix and vectors) from the pointers.
//! `Math_SinCos` runs as the game's; `memset` (ported) is a plain stand-in. The memory (bar the port's scratch: the game's stack) and the calls are
//! compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cull::CullEnv;
use tricky_game::particledraw::{ParticleDrawEnv, APP};
use tricky_game::pf::Mem;
use tricky_game::skin as sk;

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
const SIZE: u32 = 0x2_0000;
const RENDER: u32 = ARENA;
const RVT: u32 = ARENA + 0x400;
const APPOBJ: u32 = ARENA + 0x800;
const SKIN: u32 = ARENA + 0x1000;
const MODEL: u32 = ARENA + 0x5000;
const PARTS: u32 = ARENA + 0x5200;
const MESHES: u32 = ARENA + 0x5600;
const BONES: u32 = ARENA + 0x6000;
const BINDS: u32 = ARENA + 0x8000;
const BOARD: u32 = ARENA + 0xa000;
const VA: u32 = ARENA + 0xa200;
const VB: u32 = ARENA + 0xa210;
const SPARTS: u32 = ARENA + 0xb000;
const SKELS: u32 = ARENA + 0xd000;
const SCRATCH: u32 = 0x0180_0000;
const SLOTS: [u32; 4] = [0x2e0, 0x2f0, 0x2f8, 0x310];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What a renderer virtual is logged with: its words, and what +0x310 and +0x2f0 point at
fn logged(off: u32, w: &[u32], rd: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
    match off {
        0x310 => {
            let mut v = vec![w[0], w[1], w[3], w[4], w[5]];
            v.extend((0..32).map(|k| rd(w[2] + 4 * k)));
            v
        }
        0x2f0 => {
            let mut v = vec![w[0], w[1], w[2]];
            v.extend((0..16).map(|k| rd(w[3] + 4 * k)));
            v.extend((0..4).map(|k| rd(w[4] + 4 * k)));
            v.extend((0..4).map(|k| rd(w[5] + 4 * k)));
            v
        }
        0x2f8 => w[..5].to_vec(),
        _ => w[..3].to_vec(),
    }
}

struct Ours(Vec<Call>);
impl CullEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
}
impl ParticleDrawEnv for Ours {
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let mut w = vec![this];
        w.extend(args);
        w.resize(6, 0);
        let mut rd = |a: u32| m.r32(a);
        let l = logged(func - vaddr(0), &w, &mut rd);
        self.0.push(Call(func, l));
        0
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}
/// A matrix of real sizes: a turn-like 3 × 3 part, a place
fn matrix(g: &mut Rng, m: &mut Mem, at: u32) {
    for i in 0..4 {
        for j in 0..4 {
            let v = if i == 3 { if j == 3 { 1.0 } else { g.f(-200.0, 200.0) } } else if j == 3 { 0.0 } else { g.f(-1.0, 1.0) };
            wf(m, at + 0x10 * i + 4 * j, v);
        }
    }
}

#[test]
fn skinning_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x207c30u32, 0x207f18, 0x208400, 0x206818, 0x250c10] {
        r.allow(f);
    }
    // `memset` (ported; its MMI fill is not in the runner): the bytes set
    r.stub(0x2fb468, |c, mem| {
        let (at, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
        for k in 0..n {
            mem.write_u8(at + k, v);
        }
        c.set_gpr(2, at as u64);
    });
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, mem| {
            let w: Vec<u32> = (4..10).map(|k| c.gpr(k) as u32).collect();
            let mut rd = |a: u32| mem.read_u32(a);
            let lg = logged(off, &w, &mut rd);
            l.borrow_mut().push(Call(vaddr(off), lg));
        });
    }
    let mut g = Rng(0x5c1a_0000_0000_0001);
    let (mut skinned, mut copied, mut boards) = (0usize, 0usize, 0usize);
    for case in 0..2000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (APP, vec![0; 4]), (sk::SKINNED, vec![0; 0x40 * 64]), (SCRATCH, vec![0; 0x80])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // (the scratch as a stack would be: old words, so that the table's clearing shows)
        for k in 0..0x20 {
            m.w32(SCRATCH + 4 * k, g.next());
        }
        m.w32(RENDER + 0x10, RVT);
        for off in SLOTS {
            m.w16(RVT + off, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        // the model: up to 8 parts, each a mesh of 0..6 bones (fewer than 64 in all), a slot, bind matrices
        let parts = [0u32, 1, 3, 5, 8][g.below(5) as usize];
        m.w32(MODEL, parts);
        m.w32(MODEL + 4, PARTS);
        for p in 0..8u32 {
            let part = PARTS + 0x48 * p;
            let mesh = MESHES + 0x50 * p;
            m.w32(part + 0xc, mesh);
            m.w32(part + 8, BINDS + 0x180 * p);
            m.w16(mesh + 0x42, [0u16, 1, 2, 3, 6][g.below(5) as usize]);
            m.w8(mesh + 0x4a, g.below(32) as u8);
            for b in 0..6 {
                matrix(&mut g, &mut m, BINDS + 0x180 * p + 0x40 * b);
            }
        }
        for b in 0..64 {
            matrix(&mut g, &mut m, BONES + 0x40 * b);
        }
        m.w32(SKIN + 0x3d80, BONES);
        m.w32(SKIN + 0x3f18, MODEL);
        let lod = g.below(3);
        // the board, the two vectors (the same y or not)
        m.w32(SKIN + 0x3f20, g.below(3));
        let y = g.f(-5.0, 5.0);
        wf(&mut m, VA + 4, y);
        wf(&mut m, VB + 4, if g.below(2) == 0 { y } else { g.f(-5.0, 5.0) });
        // the skin parts: 15 places, some empty; each with 0..4 bones copying from 2 skeletons
        for k in 0..15u32 {
            let sp = SPARTS + 0x200 * (k % 8);
            m.w32(SKIN + 0x3d00 + 4 * k, if g.below(3) == 0 { 0 } else { sp });
        }
        for k in 0..8u32 {
            let sp = SPARTS + 0x200 * k;
            let mesh = MESHES + 0x50 * k;
            m.w32(sp, sp + 0x20);
            m.w32(sp + 4, ARENA + 0xe000 + 0x100 * k);
            m.w32(sp + 0xc, mesh);
            m.w32(sp + 0x18, ARENA + 0xf000 + 0x200 * k);
            for b in 0..6u32 {
                let e = ARENA + 0xf000 + 0x200 * k + 0x54 * b;
                m.w16(e + 0x10, g.below(2) as u16);
                m.w16(e + 0x12, g.below(8) as u16);
                for a in 0..3 {
                    wf(&mut m, sp + 0x20 + 12 * b + 4 * a, [0.0, 1.5707964, 3.1415927, -2.0, g.f(-7.0, 7.0)][g.below(5) as usize]);
                    wf(&mut m, sp + 0x20 + 0x108 + 12 * b + 4 * a, g.f(-50.0, 50.0));
                }
            }
        }
        // (the copies' meshes: at most 4 bones, as the parts' places hold)
        for k in 0..8u32 {
            let n = m.r16(MESHES + 0x50 * k + 0x42) as i16;
            if n > 4 {
                m.w16(MESHES + 0x50 * k + 0x42, 4);
            }
        }
        for s in 0..3u32 {
            m.w32(SKELS + 0x100 * s, SKELS + 0x100 * s);
            m.w32(SKELS + 0x100 * s + 4, ARENA + 0x1_0000 + 0x200 * s);
            for b in 0..8 {
                matrix(&mut g, &mut m, ARENA + 0x1_0000 + 0x200 * s + 0x40 * b);
            }
        }
        // (the skeleton list: the two words at +0x3cf8)
        for s in 0..2u32 {
            m.w32(SKIN + 0x3cf8 + 4 * s, SKELS + 0x100 * s);
        }
        for (at, b) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, b).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let which = case % 3;
        match which {
            0 => {
                sk::skin_matrices(&mut m, &mut ours, SKIN, lod, SCRATCH);
                r.cpu.set_gpr(4, SKIN as u64);
                r.cpu.set_gpr(5, lod as u64);
                r.call(0x207c30).unwrap_or_else(|e| panic!("case {case}: {e}"));
                skinned += 1;
            }
            1 => {
                sk::set_board_matrix(&mut m, &mut ours, SKIN, BOARD, VA, VB, SCRATCH);
                for (k, v) in [SKIN, BOARD, VA, VB].into_iter().enumerate() {
                    r.cpu.set_gpr(4 + k, v as u64);
                }
                r.call(0x207f18).unwrap_or_else(|e| panic!("case {case}: {e}"));
                boards += (m.r32(SKIN + 0x3f20) != 0) as usize;
            }
            _ => {
                sk::copy_bone_matrices(&mut m, SKIN);
                r.cpu.set_gpr(4, SKIN as u64);
                r.call(0x208400).unwrap_or_else(|e| panic!("case {case}: {e}"));
                copied += 1;
            }
        }
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} ({which}): memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{skinned} skinned, {boards} boards, {copied} copies");
    assert!(skinned > 600 && boards > 300 && copied > 600);
}

//! `tricky_game::bezier::draw` and `draw_views` (board row G7a3b) against the original in the function runner
//! (TRICKY_ELF; skipped without it): every case a fresh random manager (its four lists of linked records over
//! random patches: clip kinds, steps, textures; its views and their matrices; the views switch), a renderer (its
//! texture state, the packet buffer, a stand-in vtable); then vf15 or the views' draw alone. The renderer's
//! virtuals (the box test answering by its words), the frame's matrices and the submit are logging stand-ins. The
//! memory (bar the port's scratch: the game's stack), the calls compared.

use std::cell::RefCell;
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
const SIZE: u32 = 0x3000;
const MAN: u32 = ARENA;
const RENDER: u32 = ARENA + 0x800;
const RVT: u32 = ARENA + 0x1000;
const TEXSTATE: u32 = ARENA + 0x1400;
const RECS: u32 = ARENA + 0x1500;
const PATCHES: u32 = ARENA + 0x1900;
const PKT: u32 = 0x0140_0000;
const PKT_SIZE: u32 = 0x8000;
const SCRATCH: u32 = 0x0180_0000;
const PROGS: (u32, u32) = (0x3d0f10, 0x60);
const SLOTS: [u32; 5] = [0x1c8, 0x1f0, 0x1f8, 0x200, 0x288];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// The box test's answer: 0, 1 (outside) or 2 by its words
fn answer(words: &[u32]) -> u32 {
    let h = words.iter().fold(0x811c_9dc5u32, |h, &w| (h ^ w).wrapping_mul(0x0100_0193));
    [0, 1, 1, 2][(h >> 13) as usize & 3]
}

/// What a submit sees of the texture state: its word and program byte, and the renderer's +0x474
fn seen(m: &Mem) -> [u32; 3] {
    [m.r32(TEXSTATE), m.r8(TEXSTATE + 0x10) as u32, m.r32(RENDER + 0x474)]
}

struct Ours(Vec<Call>);
impl BezierEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        if func == b::SUBMIT {
            w.extend(seen(m));
        }
        self.0.push(Call(func, w));
        0
    }
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let mut w = vec![this];
        if func == vaddr(0x288) {
            w.extend((0..8).map(|k| m.r32(args[0 + k / 4] + 4 * (k as u32 % 4))));
            w.push(args[2]);
        } else {
            w.extend_from_slice(args);
        }
        let a = if func == vaddr(0x288) { answer(&w) } else { 0 };
        self.0.push(Call(func, w));
        a
    }
}

#[test]
fn bezier_draw_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x1d9f58);
    r.allow(0x1da5c8);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for (f, n) in [(b::MATRICES, 1), (b::SUBMIT, 4)] {
        let l = log.clone();
        r.stub(f, move |c, mem| {
            let mut w: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            if f == b::SUBMIT {
                w.extend([mem.read_u32(TEXSTATE), mem.read_u32(TEXSTATE + 0x10) & 0xff, mem.read_u32(RENDER + 0x474)]);
            }
            l.borrow_mut().push(Call(f, w));
        });
    }
    for off in SLOTS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, mem| {
            let mut w = vec![c.gpr(4) as u32];
            match off {
                0x288 => {
                    for k in 0..8u32 {
                        let at = c.gpr(5 + k as usize / 4) as u32 + 4 * (k % 4);
                        w.push(mem.read_u32(at));
                    }
                    w.push(c.gpr(7) as u32);
                }
                0x1c8 => w.extend([c.gpr(5) as u32, c.gpr(6) as u32, c.fpr[12]]),
                _ => w.extend([c.gpr(5) as u32, c.gpr(6) as u32]),
            }
            let a = if off == 0x288 { answer(&w) } else { 0 };
            l.borrow_mut().push(Call(vaddr(off), w));
            c.set_gpr(2, a as u64);
        });
    }
    let mut g = Rng(0x0b3e_2b00_0000_0001);
    let (mut sent, mut viewed, mut unlinked) = (0usize, 0usize, 0usize);
    for case in 0..20_000u32 {
        let mut m = Mem {
            regions: vec![(0, vec![0; 0x100]), (ARENA, vec![0; SIZE as usize]), (PKT, vec![0; PKT_SIZE as usize]), (SCRATCH, vec![0; 0x20]), (PROGS.0, vec![0; PROGS.1 as usize])],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the renderer: its vtable, the texture state, the packet place
        m.w32(RENDER + 0x10, RVT);
        for off in SLOTS {
            m.w16(RVT + off, [0u16, 0x10][g.below(2) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(RENDER + 0x470, TEXSTATE);
        m.w8(TEXSTATE + 0x10, [0u8, 1, 2, 3, 4, 9][g.below(6) as usize]);
        m.w32(TEXSTATE, [0u32, 0, 5][g.below(3) as usize]);
        m.w32(RENDER + 0x680, PKT + 0x10 * g.below(4));
        let whole = g.below(3) != 0;
        // the records: patches, clip kinds, steps, textures; four lists over them (some sharing)
        let nrec = 1 + g.below(16);
        let tex = |g: &mut Rng| [0u16, 1, 2, 0x7fff, 0x8000, 0xffff, 5][g.below(7) as usize];
        for k in 0..nrec {
            let rec = RECS + 0x18 * k;
            m.w32(rec + 4, [0u32, 3, 0, 3, 1, 2][g.below(6) as usize]);
            m.w16(rec + 0xc, tex(&mut g));
            m.w16(rec + 0xe, tex(&mut g));
            m.w32(rec + 0x10, PATCHES + 0x170 * g.below(4));
            m.w32(rec + 0x14, 0);
        }
        for p in 0..4u32 {
            for j in 0..3 {
                let v = [g.f(-300.0, 300.0), g.f(-2.0e7, 2.0e7), f32::from_bits(g.next())][g.below(8).min(2) as usize];
                m.w32(PATCHES + 0x170 * p + 4 * j, v.to_bits());
            }
        }
        let mut order: Vec<u32> = (0..nrec).collect();
        for k in (1..order.len()).rev() {
            order.swap(k, g.below(k as u32 + 1) as usize);
        }
        let mut at = 0usize;
        for (list, (count, head)) in [(0x54cu32, 0x550u32), (0x554, 0x558), (0x55c, 0x560), (0x564, 0x568)].into_iter().enumerate() {
            let n = if list == 3 { order.len() - at } else { (g.below(5) as usize).min(order.len() - at) };
            let mut prev = 0;
            for &k in order[at..at + n].iter().rev() {
                let rec = RECS + 0x18 * k;
                m.w32(rec + 0x14, prev);
                prev = rec;
            }
            at += n;
            m.w32(MAN + head, prev);
            m.w32(MAN + count, if g.below(6) == 0 { 0 } else { n as u32 });
            // as vf15 lays them out, for the views' draw alone (vf15's own to make)
            if !whole {
                let e = MAN + 0x56c + 0x10 * list as u32;
                m.w32(e, m.r32(MAN + count));
                m.w32(e + 4, prev);
                m.w32(e + 8, [3, 2, 1, 4][list]);
                m.w32(e + 0xc, b::PROGRAMS[list]);
            }
        }
        for k in 0..PROGS.1 / 4 {
            m.w32(PROGS.0 + 4 * k, g.next());
        }
        // the views: how many (to 8), their words; the switch
        m.w32(MAN + 0x2c, if g.below(16) == 0 { 0x8000_0000 | g.next() } else { g.below(9) });
        m.w32(MAN + 0x20, [0, 1, g.next()][g.below(3) as usize]);
        for v in 0..8 {
            m.w32(MAN + 0xa0 * v + 0xc4, [0u32, 1, 0xffff, 0x8000, u32::MAX, g.next()][g.below(6) as usize]);
        }
        for (at, bytes) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, bytes).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let counts = [0u32, 1, 2, 3].map(|k| m.r32(MAN + 0x56c + 0x10 * k));
        if whole {
            b::draw(&mut m, &mut ours, MAN, RENDER, SCRATCH);
        } else {
            b::draw_views(&mut m, &mut ours, MAN, RENDER, SCRATCH);
        }
        r.cpu.set_gpr(4, MAN as u64);
        r.cpu.set_gpr(5, RENDER as u64);
        r.call(if whole { 0x1d9f58 } else { 0x1da5c8 }).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.0 {
            let i = theirs.iter().zip(&ours.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.0.len()));
            panic!("case {case} whole {whole}: call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.0.get(i));
        }
        sent += ours.0.iter().filter(|c| c.0 == b::SUBMIT).count();
        viewed += ours.0.iter().filter(|c| c.0 == b::SUBMIT && c.1[3] == 1).count();
        for (at, bytes) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} whole {whole}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
        // the views' draw counts its lists down at +0x56c.. as it takes patches out
        unlinked += (0..4).filter(|k| m.r32(MAN + 0x56c + 0x10 * k) != counts[*k as usize]).count();
    }
    println!("{sent} packets, {viewed} view passes, {unlinked} lists shortened");
    assert!(sent > 20_000 && viewed > 5000 && unlinked > 1000);
}

//! `tricky_game::drawlist`'s flush (board row G7h2) against the original in the function runner (TRICKY_ELF;
//! skipped without it). Every case is a fresh list filled by the (checked) G7h1 port: random packets under a few
//! states, texture pages (negative words too), views and words, some following. Then one of:
//! - the flush;
//! - one batch's state changes against a random current state;
//! - the full state;
//! - a GS register write or close (every how) on a random builder;
//! - `Game_DrawLists`.
//!
//! The VU1 program write, the texture upload, the sort, the object lists' draw and the app's +0x730 question are
//! logging stand-ins. The memory (bar the port's scratch: the game's stack), the calls and the answers are compared.

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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x18000;
const D: u32 = ARENA;
const PAGES: u32 = ARENA + 0x10000;
const BATCHES: u32 = ARENA + 0x10400;
const PKT: u32 = ARENA + 0x11800;
const OUT: u32 = ARENA + 0x13000;
const SLOT: u32 = ARENA + 0x17f00;
const STATES: u32 = ARENA + 0x17e00;
const APPOBJ: u32 = ARENA + 0x16000;
const SCRATCH: u32 = 0x0180_0000;
const HEADER: (u32, u32) = (0x33d6c0, 0x30);
const GLOBALS: (u32, u32) = (0x337c58, 0x10);
const LISTS: (u32, u32) = (0x33d7d8, 0x40);
const ASK: u32 = 0x01ff_0000;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>, u32);
impl DrawListEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, _args: &[u32]) -> u32 {
        self.0.push(Call(func, vec![this]));
        self.1
    }
}

#[test]
fn draw_flush_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1c4fe0u32, 0x1c51f0, 0x1c5368, 0x1c7290, 0x1c0340, 0x1c03b8, 0x1c03e8, 0x1c04f8, 0x1c0560, 0x1c05d8, 0x1c06a0, 0x1c4220, 0x1c42a0] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ask: Rc<RefCell<u32>> = Rc::default();
    for (f, n) in [(dl::SORT, 3), (dl::UPLOAD, 4), (dl::WRITE_TEX_STATE, 5), (dl::DRAW_ALL, 2), (ASK, 1)] {
        let (l, a) = (log.clone(), ask.clone());
        r.stub(f, move |c, _| {
            l.borrow_mut().push(Call(f, (0..n).map(|k| c.gpr(4 + k) as u32).collect()));
            c.set_gpr(2, if f == ASK { *a.borrow() as u64 } else { 0 });
        });
    }
    let mut g = Rng(0x0d71_2000_0000_0001);
    let mut kinds = [0usize; 12];
    let (mut writes, mut calls) = (0usize, 0usize);
    for case in 0..4000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (HEADER.0, vec![0; HEADER.1 as usize]), (GLOBALS.0, vec![0; GLOBALS.1 as usize]), (LISTS.0, vec![0; LISTS.1 as usize]), (SCRATCH, vec![0; 0x30])] };
        for (at, len) in [(ARENA, SIZE), (LISTS.0, LISTS.1)] {
            for k in 0..len / 4 {
                m.w32(at + 4 * k, g.next());
            }
        }
        // the app: the renderer (its texture cache), the object asked and its vtable, +0x30's +0x70
        let app = APPOBJ;
        m.w32(GLOBALS.0, app);
        m.w32(app + 0x724, APPOBJ + 0x100);
        m.w32(app + 0x730, [0, APPOBJ + 0x80][g.below(4).min(1) as usize]);
        m.w32(APPOBJ + 0x88, APPOBJ + 0xc0);
        m.w16(APPOBJ + 0xc0 + 0x20, [0u16, 4][g.below(2) as usize]);
        m.w32(APPOBJ + 0xc0 + 0x24, ASK);
        m.w32(app + 0x30, APPOBJ + 0x140);
        *ask.borrow_mut() = g.below(2);
        // the list: empty, then filled by G7h1's port
        for k in 0..0x200 {
            m.w32(D + 0x3224 + 4 * k, 0);
        }
        for (at, v) in [(0u32, 0u32), (4, PAGES), (8, 0), (0xc, BATCHES), (0x3a24, 0), (0xfa4c, 0), (0xfa54, 0), (0xfa58, 0), (0xfa5c, 0), (0xfa60, 0)] {
            m.w32(D + at, v);
        }
        for k in 0..4u32 {
            m.w8(STATES + 0x20 * k + 0xe, g.below(4) as u8);
            m.w8(STATES + 0x20 * k + 0x10, [0u8, 1, 2, 3, 4, 6][g.below(6) as usize]);
            for at in [0x11u32, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0xc, 0xd] {
                m.w8(STATES + 0x20 * k + at, [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][g.below(12) as usize]);
            }
            // the alpha reference: the default's 12 too, so that the tests' bytes alone can differ
            m.w16(STATES + 0x20 * k + 6, [12u16, 0, 5][g.below(3) as usize]);
            if g.below(2) == 0 {
                let w = m.r32(STATES);
                m.w32(STATES + 0x20 * k, w);
            }
        }
        for k in 0..0x40u32 {
            // the views' rectangles
            m.w16(D + 0x3024 + 2 * k, g.below(640) as u16);
        }
        let n = g.below(24);
        let mut end = PKT;
        for _ in 0..n {
            let s = STATES + 0x20 * g.below(4);
            let pw = |g: &mut Rng| [0u32, 1, 5, u32::MAX][g.below(4) as usize];
            let (a, b) = (pw(&mut g), pw(&mut g));
            dl::set_state(&mut m, D, s);
            dl::set_texture_page(&mut m, D, a, b);
            if g.below(3) == 0 {
                let (x, y) = (g.below(3).wrapping_sub(1), g.below(4).wrapping_sub(1));
                if m.r32(D + 0xfa5c) != x || m.r32(D + 0xfa60) != y {
                    m.w32(D + 0xfa54, 0);
                }
                m.w32(D + 0xfa5c, x);
                m.w32(D + 0xfa60, y);
            }
            let start = end;
            end += 0x10 * (1 + g.below(3));
            dl::add_packet(&mut m, D, start, end, [0, 0, 1][g.below(3) as usize]);
            end += 0x10;
        }
        m.w32(D + 0xfa64, g.next());
        m.w32(D + 0xfa68, g.next());
        m.w32(SLOT, OUT + 0x10 * g.below(4));
        for (at, bytes) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, bytes).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), *ask.borrow());
        let kind = g.below(12) as usize;
        kinds[kind] += 1;
        let w: Vec<u32> = (0..5).map(|_| [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, u32::MAX, g.next()][g.below(14) as usize]).collect();
        let builder = OUT + 0x2000;
        if (3..11).contains(&kind) {
            // a builder: a count, the next place, the start
            m.w32(builder, [0, 1, 7][g.below(3) as usize]);
            m.w32(builder + 4, OUT + 0x2100 + 8 * g.below(8));
            m.w32(builder + 8, OUT + 0x2400);
            r.mem.write(builder, &m.regions[0].1[(builder - ARENA) as usize..(builder - ARENA + 12) as usize]).unwrap();
        }
        let mut want = None;
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                want = Some(dl::flush_buckets(&mut m, &mut ours, D, SLOT, SCRATCH));
                (0x1c4fe0, vec![D, SLOT])
            }
            1 if m.r32(D + 8) != 0 => {
                let bt = BATCHES + 0x40 * g.below(m.r32(D + 8));
                let (word, view) = (w[0], [u32::MAX, 0, 1, 2, 7, 0x8000_0000][g.below(6) as usize]);
                dl::write_state_changes(&mut m, &mut ours, D, bt, word, view, SLOT, SCRATCH);
                (0x1c5368, vec![D, bt, word, view, SLOT])
            }
            2 | 1 => {
                dl::write_full_state(&mut m, &mut ours, D, SLOT, SCRATCH);
                (0x1c51f0, vec![D, SLOT])
            }
            3 => {
                let (x, wd, y, h) = (g.below(640), g.below(640), g.below(480), w[0]);
                dl::gs_scissor(&mut m, builder, x, wd, y, h);
                (0x1c0340, vec![builder, x, wd, y, h])
            }
            4 => {
                dl::gs_texflush(&mut m, builder);
                (0x1c03b8, vec![builder])
            }
            5 => {
                dl::gs_alpha(&mut m, builder, w[0], w[1], w[2]);
                (0x1c03e8, vec![builder, w[0], w[1], w[2]])
            }
            6 => {
                dl::gs_clamp(&mut m, builder, w[0], w[1]);
                (0x1c04f8, vec![builder, w[0], w[1]])
            }
            7 => {
                let (base, format) = (g.next(), w[0].wrapping_add(0x30));
                dl::gs_zbuf(&mut m, builder, base, format, w[1], w[2]);
                (0x1c0560, vec![builder, base, format, w[1], w[2]])
            }
            8 => {
                dl::gs_test(&mut m, builder, w[0], w[1], w[2], w[3]);
                (0x1c05d8, vec![builder, w[0], w[1], w[2], w[3]])
            }
            9 | 10 => {
                let how = [0, 1, 2, 3, u32::MAX][g.below(5) as usize];
                dl::gs_close(&mut m, builder, SLOT, how);
                (0x1c06a0, vec![builder, SLOT, how])
            }
            _ => {
                dl::game_draw_lists(&mut m, &mut ours);
                (0x1c7290, vec![])
            }
        };
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} kind {kind}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.0 {
            let i = theirs.iter().zip(&ours.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.0.len()));
            panic!("case {case} kind {kind}: call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.0.get(i));
        }
        if let Some(v) = want {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: the answer");
        }
        calls += ours.0.len();
        for (at, bytes) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} kind {kind}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
        writes += ((m.r32(SLOT) - OUT) / 8) as usize;
    }
    println!("kinds {kinds:?}; {calls} calls, {writes} doublewords out");
    assert!(calls > 10_000 && writes > 200_000);
}

//! `tricky_game::drawlist` (board row G7j1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh list (its arrays and the rest random, its hash heads and counts empty, nothing
//! found) and a renderer, then a run of random steps on both: states set from a few (some alike but for one field)
//! or bound, texture pages (negative and past 512 too), the renderer's submits and flushes with their pending
//! flags, packets added (following or not), the two words, the sort, the state's inits, the compares and the key.
//! The sort is a logging stand-in. The memory, the calls and the answers compared after every step.

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
const SIZE: u32 = 0x14000;
const D: u32 = ARENA;
const PAGES: u32 = ARENA + 0x10000;
const BATCHES: u32 = ARENA + 0x10400;
const PKT: u32 = ARENA + 0x11800;
const R: u32 = ARENA + 0x13000;
const STATES: u32 = ARENA + 0x13100;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

struct Ours(Vec<Call>);
impl DrawListEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) {
        self.0.push(Call(func, args.to_vec()));
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        panic!("no virtuals: {func:#x} {this:#x} {args:?}")
    }
}

fn state(k: u32) -> u32 {
    STATES + 0x20 * k
}

#[test]
fn draw_lists_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1c4220u32, 0x1c4270, 0x1c42a0, 0x1c4340, 0x1c4468, 0x1c4638, 0x1c4d30, 0x1c4d68, 0x1c4ee8, 0x1c4f28, 0x1c6f98, 0x1c7068, 0x1c7088, 0x1c71b0, 0x1e8838, 0x1e88c8] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let l = log.clone();
    r.stub(dl::SORT, move |c, _| l.borrow_mut().push(Call(dl::SORT, (0..3).map(|k| c.gpr(4 + k) as u32).collect())));
    let mut g = Rng(0x0d71_1000_0000_0001);
    let mut ops = [0usize; 16];
    let (mut batches, mut pages, mut keyed) = (0u32, 0u32, 0u32);
    for case in 0..1500u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..0x200 {
            m.w32(D + 0x3224 + 4 * k, 0);
        }
        m.w32(D, 0);
        m.w32(D + 4, PAGES);
        m.w32(D + 8, 0);
        m.w32(D + 0xc, BATCHES);
        m.w32(D + 0x3a24, 0);
        for at in [0xfa4c, 0xfa54, 0xfa58] {
            m.w32(D + at, 0);
        }
        m.w32(D + 0xfa5c, g.below(4).wrapping_sub(1));
        m.w32(D + 0xfa60, g.below(4).wrapping_sub(1));
        m.w32(R + 0x480, D);
        // a few states, the later ones an earlier one but for one field
        m.w8(state(0) + 0xe, [0u8, 1, 2, 3, 7][g.below(5) as usize]);
        m.w8(state(0) + 0x10, g.below(13) as u8);
        let mut source = [0u32; 6];
        for k in 1..6 {
            source[k as usize] = g.below(k);
            let from = state(source[k as usize]);
            for j in 0..0x18 {
                let v = m.r8(from + j);
                m.w8(state(k) + j, v);
            }
            let j = g.below(0x18);
            m.w8(state(k) + j, [0u8, 1, 2, 3, 0xff, g.next() as u8][g.below(6) as usize]);
        }
        r.mem.write(ARENA, &m.regions[0].1).unwrap();
        let page_word = |g: &mut Rng| [0u32, 1, 2, u32::MAX, 0x200, 0x201, 0x8000_0005][g.below(7) as usize];
        let mut ours = Ours(Vec::new());
        let mut ends = 0u32;
        for step in 0..40 {
            let op = g.below(16) as usize;
            ops[op] += 1;
            log.borrow_mut().clear();
            ours.0.clear();
            let si = g.below(6);
            let s = state(si);
            let (a, b) = (page_word(&mut g), page_word(&mut g));
            let follows = [0, 0, 1, g.next()][g.below(4) as usize];
            let start = PKT + 0x10 * g.below(0x100);
            let end = PKT + 0x10 * (ends % 0x150);
            ends += 1 + g.below(3);
            let mut want: Option<u32> = None;
            let found_page = m.r32(D + 0xfa4c) != 0;
            let has_batch = m.r32(D + 0xfa58) != 0;
            let (func, args): (u32, Vec<u32>) = match op {
                0 => {
                    dl::set_state(&mut m, D, s);
                    (0x1c4d68, vec![D, s])
                }
                1 => {
                    dl::set_texture_page(&mut m, D, a, b);
                    (0x1c4d30, vec![D, a, b])
                }
                2 => {
                    dl::bind_texture(&mut m, D, s);
                    (0x1c4ee8, vec![D, s])
                }
                3 | 4 => {
                    // the renderer's pending state
                    for (at, v) in [(0x470, s), (0x474, [0, 1, 2, g.next()][g.below(4) as usize]), (0x478, [0, 1, 2, g.next()][g.below(4) as usize])] {
                        m.w32(R + at, v);
                        r.mem.write(R + at, &v.to_le_bytes()).unwrap();
                    }
                    if op == 3 {
                        dl::submit(&mut m, R, start, end, follows);
                        (0x1e8838, vec![R, start, end, follows])
                    } else {
                        dl::flush_state(&mut m, R);
                        (0x1e88c8, vec![R])
                    }
                }
                5 | 6 => {
                    dl::add_packet(&mut m, D, start, end, follows);
                    (0x1c4f28, vec![D, start, end, follows])
                }
                7 => {
                    dl::sort_buckets(&mut m, &mut ours, D);
                    (0x1c42a0, vec![D])
                }
                8 => {
                    let t = state(if g.below(2) == 0 { source[si as usize] } else { g.below(6) });
                    want = Some(dl::state_equals(&m, s, t) as u32);
                    (0x1c6f98, vec![s, t])
                }
                9 if has_batch => {
                    want = Some(dl::pack_sort_key(&m, D));
                    (0x1c7088, vec![D])
                }
                10 => {
                    if g.below(2) == 0 {
                        dl::state_init(&mut m, s);
                        (0x1c4220, vec![D, s])
                    } else {
                        dl::state_construct(&mut m, s);
                        (0x1c71b0, vec![s])
                    }
                }
                11 => {
                    dl::set_words(&mut m, D, a, b);
                    (0x1c4638, vec![D, a, b])
                }
                12 if g.below(8) == 0 => {
                    // the list emptied: no heads, nothing found, no sort entries (pages and batches go on)
                    dl::clear_buckets(&mut m, D);
                    for at in [0xfa4c, 0xfa54, 0xfa58, 0x3a24] {
                        m.w32(D + at, 0);
                        r.mem.write(D + at, &[0; 4]).unwrap();
                    }
                    (0x1c4270, vec![D])
                }
                13 if m.r32(D) != 0 => {
                    let p = PAGES + 0x10 * g.below(m.r32(D));
                    want = Some(dl::page_is(&m, p, a, b) as u32);
                    (0x1c7068, vec![p, a, b])
                }
                14 => {
                    dl::find_page(&mut m, D);
                    (0x1c4340, vec![D])
                }
                15 if found_page => {
                    dl::find_or_add_batch(&mut m, D, follows);
                    (0x1c4468, vec![D, follows])
                }
                _ => {
                    // the two words a batch is kept apart by
                    let (x, y) = (g.below(4).wrapping_sub(1), g.below(4).wrapping_sub(1));
                    for (at, v) in [(0xfa5c, x), (0xfa60, y)] {
                        m.w32(D + at, v);
                        r.mem.write(D + at, &v.to_le_bytes()).unwrap();
                    }
                    continue;
                }
            };
            for (i, v) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
            }
            r.call(func).unwrap_or_else(|e| panic!("case {case} step {step} op {op}: {e}"));
            assert_eq!(*log.borrow(), ours.0, "case {case} step {step} op {op}: the calls");
            if let Some(v) = want {
                assert_eq!(r.cpu.gpr(2) as u32, v, "case {case} step {step} op {op}: the answer");
            }
            let mut th = vec![0u8; SIZE as usize];
            r.mem.read(ARENA, &mut th).unwrap();
            let ours_mem = &m.regions[0].1;
            if &th != ours_mem {
                let i = th.iter().zip(ours_mem).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} step {step} op {op}: memory at {:#x}: theirs {:#x} ours {:#x}", ARENA + i as u32, w(&th), w(ours_mem));
            }
        }
        batches += m.r32(D + 8);
        pages += m.r32(D);
        keyed += m.r32(D + 0x3a24);
    }
    println!("ops {ops:?}; {pages} pages, {batches} batches, {keyed} keyed");
    assert!(pages > 3000 && batches > 5000 && keyed > 2000);
}

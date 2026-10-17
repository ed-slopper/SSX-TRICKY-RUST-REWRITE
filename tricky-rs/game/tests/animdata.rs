//! `tricky_game::animdata` (board row G5c1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random bank at its own address (0x3211a8; its memory bank's words, its files'
//! records with clip headers in an arena, random clip headers' markers), the key table, the clip names' hash table,
//! and random names; then one of the 19 functions with random arguments. The allocator, the free, the BIG file
//! object's three calls and delete are logging stand-ins; memset runs as the original. The bank, the arena, the
//! tables, the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animdata as d;
use tricky_game::animseq::{self as a, SeqEnv};
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
const SIZE: u32 = 0x4000;
const HDRS: u32 = ARENA;
const NHDR: u32 = 64;
const BLOCK: u32 = ARENA + 0x1000;
const STRS: u32 = ARENA + 0x3000;
const BANK_LEN: u32 = 0x30f4;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn answer(f: u32, args: &[u32]) -> u32 {
    match f {
        d::ALLOC => {
            if args[1] % 5 == 0 {
                0
            } else {
                BLOCK + (args[1] & 0x7c)
            }
        }
        _ => 0,
    }
}

struct Ours(Vec<Call>);
impl SeqEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.push(Call(func, args.to_vec()));
        answer(func, args)
    }
}

#[test]
fn the_bank_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x157d58u32, 0x157d68, 0x157da0, 0x157e10, 0x157e68, 0x157ea8, 0x158af0, 0x15ac48, 0x158798, 0x1587e8, 0x158818,
        0x158d68, 0x240e80, 0x240ed8, 0x159820, 0x158c38, 0x158cc8, 0x158d18, 0x159070, 0x15a8c0, 0x2cc808,
    ] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    for (f, n) in [(d::MEM_FREE, 1usize), (d::ALLOC, 3), (d::BIG_CTOR, 1), (d::BIG_DTOR, 2), (d::BIG_RESET, 1), (d::DELETE, 1)] {
        let l = log.clone();
        r.stub(f, move |c, _| {
            let args: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            l.borrow_mut().push(Call(f, args.clone()));
            c.set_gpr(2, answer(f, &args) as i32 as i64 as u64);
        });
    }
    // the clip names: the game's pointers and the strings they point at
    let mut names = vec![0u8; 4 * d::NCLIP as usize];
    r.mem.read(d::NAMES, &mut names).unwrap();
    let ptrs: Vec<u32> = names.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let (lo, hi) = (*ptrs.iter().min().unwrap(), *ptrs.iter().max().unwrap() + 0x40);
    let mut strs = vec![0u8; (hi - lo) as usize];
    r.mem.read(lo, &mut strs).unwrap();
    let mut nokey = vec![0u8; 4];
    r.mem.read(a::NO_KEY, &mut nokey).unwrap();
    let mut g = Rng(0x1587_0000_0000_0001);
    let mut counts = [0usize; 19];
    for case in 0..30_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (d::BANK, vec![0; BANK_LEN as usize]),
                (a::KEY_TABLE, vec![0; 4 * 0x584]),
                (d::HASHES, vec![0; 4 * d::NCLIP as usize]),
                (d::NAMES, names.clone()),
                (lo, strs.clone()),
                (a::NO_KEY, nokey.clone()),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..BANK_LEN / 4 {
            m.w32(d::BANK + 4 * k, g.next());
        }
        for k in 0..0x584 {
            m.w32(a::KEY_TABLE + 4 * k, g.next());
            m.w32(d::HASHES + 4 * k, g.next());
        }
        // the clip headers: a length (sometimes 0 or 1), the three lists (often unset)
        for h in 0..NHDR {
            let at = HDRS + 0x24 * h;
            let len = match g.below(6) {
                0 => g.below(2),
                _ => 1 + g.below(60),
            };
            m.w16(at + 0xa, len as u16);
            for k in 0..12 {
                m.w16(at + 0xc + 2 * k, if g.below(3) == 0 { 0xffff } else { g.below(70) } as u16);
            }
        }
        let b = d::BANK;
        // the memory bank's block somewhere (unaligned too)
        m.w32(b, if g.below(5) == 0 { 0 } else { BLOCK + g.below(0x200) });
        // the files: their headers in the arena
        for i in 0..d::NFILE {
            m.w32(b + d::FILES + 0x18 * i + 8, HDRS + 0x24 * g.below(NHDR / 2));
        }
        m.w32(b + 0x710, if g.below(2) == 0 { 0 } else { g.next() });
        // random names in the arena
        let name = STRS + 0x40 * g.below(8);
        let n = g.below(40);
        for k in 0..n {
            let ch = match g.below(4) {
                0 => 0x80 + g.below(0x7f),
                _ => 0x20 + g.below(0x5f),
            };
            m.w8(name + k, ch as u8);
        }
        m.w8(name + n, 0);
        for (at, bts) in &m.regions[..4] {
            r.mem.write(*at, bts).unwrap();
        }
        let file = b + d::FILES + 0x18 * g.below(d::NFILE);
        let (clip, k) = (g.below(32), g.below(4));
        let kind = g.below(19) as usize;
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        enum Ans {
            No,
            W(u32),
            Fl(u32),
        }
        let (f, args, mine) = match kind {
            0 => (0x157d58, vec![b], Ans::W(d::mem_bank_new(&mut m, b))),
            1 => {
                d::mem_bank_free(&mut m, &mut ours, b);
                (0x157d68, vec![b], Ans::No)
            }
            2 => {
                let size = g.below(0x180);
                d::mem_bank_init(&mut m, &mut ours, b, size);
                (0x157da0, vec![b, size], Ans::No)
            }
            3 => {
                let fl = g.below(4);
                d::mem_bank_destruct(&mut m, &mut ours, b, fl);
                (0x157e10, vec![b, fl], Ans::No)
            }
            4 => {
                let size = if g.below(6) == 0 { g.next() } else { g.below(0x400).wrapping_sub(0x80) };
                (0x157e68, vec![b, size], Ans::W(d::mem_bank_alloc(&mut m, b, size)))
            }
            5 => {
                d::mem_bank_align_start(&mut m, b);
                (0x157ea8, vec![b], Ans::No)
            }
            6 => {
                d::file_reset(&mut m, file);
                (0x158af0, vec![file], Ans::No)
            }
            7 => {
                let fl = g.below(4);
                d::file_destruct(&mut m, &mut ours, file, fl);
                (0x15ac48, vec![file, fl], Ans::No)
            }
            8 => (0x158798, vec![file, clip, k], Ans::Fl(d::marker_phase(&m, file, clip, k).0.to_bits())),
            9 => (0x1587e8, vec![file, clip, k], Ans::W(d::marker_frame(&m, file, clip, k) as u32)),
            10 => {
                let v = if g.below(3) == 0 { u32::MAX } else { g.below(0x20000) };
                d::set_marker(&mut m, file, clip, k, v);
                (0x158818, vec![file, clip, k, v], Ans::No)
            }
            11 => {
                let an = g.below(0x584);
                (0x158d68, vec![b, an], Ans::W(d::clip_id(&m, an)))
            }
            12 => (0x240ed8, vec![name], Ans::W(d::name_hash(&m, name))),
            13 => {
                d::hash_names(&mut m);
                (0x159820, vec![b], Ans::No)
            }
            14 => (0x158c38, vec![b], Ans::W(d::construct(&mut m, &mut ours, b))),
            15 => {
                d::clear(&mut m, &mut ours, b);
                (0x158cc8, vec![b], Ans::No)
            }
            16 => {
                let size = g.below(0x180);
                d::init(&mut m, &mut ours, b, size);
                (0x158d18, vec![b, size], Ans::No)
            }
            17 => {
                d::reset(&mut m, &mut ours, b);
                (0x159070, vec![b], Ans::No)
            }
            _ => {
                let fl = g.below(4);
                d::destruct(&mut m, &mut ours, b, fl);
                (0x15a8c0, vec![b, fl], Ans::No)
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        match mine {
            Ans::No => {}
            Ans::W(v) => assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {f:#x} answer"),
            Ans::Fl(v) => assert_eq!(r.cpu.f(0).to_bits(), v, "case {case}: {f:#x} answer"),
        }
        assert_eq!(*log.borrow(), ours.0, "case {case}: {f:#x} calls");
        for (at, bts) in &m.regions[..4] {
            let mut theirs = vec![0u8; bts.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != bts {
                let i = theirs.iter().zip(bts).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], bts[i]);
            }
        }
    }
    println!("{counts:?}");
}

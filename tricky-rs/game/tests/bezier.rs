//! `tricky_game::bezier` (board row G7a3a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random manager (its records somewhere, its lists, views and switches random), a
//! renderer holding it, a stand-in vtable (begin and the queue the real ones or logging stand-ins, the this-adjust
//! 0 or 0x10) or the game's own; then one of the functions with random words. The allocator (answering 0 or a
//! pointer), the fill, the free, the delete and the tables' build are logging stand-ins. The memory and the calls
//! compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bezier::{self as b, BezierEnv};
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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const MAN: u32 = ARENA;
const VT: u32 = ARENA + 0x800;
const STEPS: u32 = ARENA + 0x900;
const SHORTS: u32 = ARENA + 0x940;
const SRC: u32 = ARENA + 0xa00;
const RECS: u32 = ARENA + 0x1000;
const RENDER: u32 = ARENA + 0x2000;
const VT_GAME: (u32, u32) = (b::VTABLE, 0xa0);
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

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
        self.0.push(Call(func, std::iter::once(this).chain(args.iter().copied()).collect()));
        0
    }
}

#[test]
fn bezier_queue_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x1d9be0u32, 0x1d9c40, 0x1db908, 0x1d9c88, 0x1d9cf8, 0x1db150, 0x1db1c8, 0x1db4f8, 0x1d9d20, 0x1d9d50, 0x1d9eb8] {
        r.allow(f);
    }
    for f in [0x1db840u32, 0x1db848, 0x1db850, 0x1db858, 0x1db860, 0x1db868, 0x1db870, 0x1db878, 0x1db880, 0x1db888, 0x1db890, 0x1db898, 0x1db8a0, 0x1db8a8, 0x1db8b0] {
        r.allow(f);
    }
    for f in [0x1e9008u32, 0x1e9068, 0x1ea068] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let answer: Rc<Cell<u32>> = Rc::default();
    for (f, n) in [(b::ALLOC, 3), (b::MEM_SET, 3), (b::MEM_FREE, 1), (b::DELETE, 2), (b::BUILD_TABLES, 1), (vaddr(0x78), 2), (vaddr(0x88), 5)] {
        let (l, a) = (log.clone(), answer.clone());
        r.stub(f, move |c, _| {
            l.borrow_mut().push(Call(f, (0..n).map(|k| c.gpr(4 + k) as u32).collect()));
            c.set_gpr(2, if f == b::ALLOC { a.get() as u64 } else { 0 });
        });
    }
    let mut game_vt = vec![0u8; VT_GAME.1 as usize];
    r.mem.read(VT_GAME.0, &mut game_vt).unwrap();
    let mut g = Rng(0x0b3e_2000_0000_0001);
    let mut kinds = [0usize; 12];
    let mut lists = [0usize; 5];
    for case in 0..40_000u32 {
        let mut m = Mem { regions: vec![(0, vec![0; 0x400]), (ARENA, vec![0; SIZE as usize]), (VT_GAME.0, game_vt.clone())] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the manager: its records, how many, the views, the switches; the vtable a stand-in or the game's
        let game = g.below(4) == 0;
        let adj = [0u16, 0x10][g.below(2) as usize];
        m.w32(MAN, if game { b::VTABLE } else { VT });
        for off in [0x78u32, 0x88] {
            // the real ones on the manager itself; the stand-ins maybe at an adjusted this
            let real = if off == 0x78 { b::BEGIN } else { b::QUEUE };
            let f = if g.below(2) == 0 { real } else { vaddr(off) };
            m.w16(VT + off, if f == real { 0 } else { adj });
            m.w32(VT + off + 4, f);
        }
        let this = MAN;
        m.w32(MAN + 0x544, [RECS + 4 * g.below(4), 0][(g.below(8) == 0) as usize]);
        m.w32(MAN + 0x548, g.below(32));
        m.w32(MAN + 0x2c, g.below(8));
        m.w32(MAN + 0x530, [0, g.next()][g.below(2) as usize]);
        for at in [0x18u32, 0x1c, 0x20, 0x24] {
            m.w32(MAN + at, [0, 1, g.next()][g.below(3) as usize]);
        }
        m.w32(RENDER + 0x484, MAN);
        // a patch's four steps (one of the three tessellations all round, or not) and its shorts
        let pick = |g: &mut Rng| [4u8, 6, 8, 5, 0, g.next() as u8][g.below(6) as usize];
        let s0 = pick(&mut g);
        for k in 0..4 {
            m.w8(STEPS + k, if g.below(4) == 0 { pick(&mut g) } else { s0 });
        }
        m.w32(SHORTS + 4, [g.next() & 0x7fff_ffff, g.next() | 0x8000_0000, 0][g.below(3) as usize]);
        let alloc = [0, RECS + 0x10 * g.below(4)][g.below(3).min(1) as usize];
        answer.set(alloc);
        let kind = g.below(12);
        kinds[kind as usize] += 1;
        let words: Vec<u32> = (0..5).map(|_| g.next()).collect();
        let fl = f32::from_bits(g.next());
        let clip = [0, words[3]][g.below(2) as usize];
        for (at, bytes) in &m.regions {
            r.mem.write(*at, bytes).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new(), alloc);
        let mut v0 = None;
        let mut f0 = None;
        let set = |r: &mut Runner, ws: &[u32]| {
            for (i, w) in ws.iter().enumerate() {
                r.cpu.set_gpr(4 + i, *w as i32 as i64 as u64);
            }
        };
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                v0 = Some(b::construct(&mut m, &mut ours, MAN));
                (0x1d9be0, vec![MAN])
            }
            1 => {
                b::destruct(&mut m, &mut ours, MAN, words[0]);
                (0x1d9c40, vec![MAN, words[0]])
            }
            2 => {
                b::base_destruct(&mut m, &mut ours, MAN, words[0]);
                (0x1db908, vec![MAN, words[0]])
            }
            3 => {
                b::init(&mut m, &mut ours, MAN);
                (0x1d9c88, vec![MAN])
            }
            4 => {
                b::shutdown(&mut m, &mut ours, MAN);
                (0x1d9cf8, vec![MAN])
            }
            5 => {
                b::patch_alloc(&mut m, &mut ours, MAN);
                (0x1db150, vec![MAN])
            }
            6 => {
                b::begin(&mut m, this);
                (0x1d9d20, vec![this])
            }
            7 => {
                let n = m.r32(MAN + 0x548);
                let rec = m.r32(MAN + 0x544).wrapping_add(n.wrapping_mul(0x18));
                let before = if rec.wrapping_sub(ARENA) < SIZE - 4 { m.r32(rec) } else { 0 };
                b::queue(&mut m, MAN, words[0], STEPS, SHORTS, clip);
                let after = if rec.wrapping_sub(ARENA) < SIZE - 4 { m.r32(rec) } else { 9 };
                lists[if after == before { 4 } else { (after as usize).min(4) }] += 1;
                (0x1d9d50, vec![MAN, words[0], STEPS, SHORTS, clip])
            }
            8 => {
                let (q, a, c) = (SRC + 4 * g.below(4), SRC + 0x10 + 4 * g.below(4), SRC + 0x60 + 4 * g.below(16));
                // a manager off 16 too: the stores round down as the loads do
                let at = MAN + 4 * g.below(4);
                // off 16, the quad lands on the count, which is read again: that lane of the quad a count too
                for (w, v) in [(at + 0x2c, g.below(8)), ((q & !15) + ((at + 0x2c) & 15), g.below(8))] {
                    m.w32(w, v);
                    r.mem.write(w, &v.to_le_bytes()).unwrap();
                }
                b::add_view(&mut m, at, q, a, c, words[0], F(fl));
                r.cpu.set_f(12, fl);
                (0x1d9eb8, vec![at, q, a, c, words[0]])
            }
            9 => {
                b::renderer_patch_begin(&mut m, &mut ours, RENDER);
                (0x1e9068, vec![RENDER])
            }
            10 => {
                b::renderer_patch_draw(&mut m, &mut ours, RENDER, words[0], STEPS, SHORTS, clip);
                (0x1e9008, vec![RENDER, words[0], STEPS, SHORTS, clip])
            }
            _ => {
                // the accessors: each switch set or read, the distances set or read
                let k = g.below(15);
                let at = [0x20u32, 0x1c, 0x18, 0x20, 0x1c, 0x18, 0x24, 0x14, 0x10, 0x14, 0x10, 0x540, 0x53c, 0x538, 0x534][k as usize];
                match k {
                    0..=2 | 11..=14 => v0 = Some(b::switch(&m, MAN, at)),
                    3..=6 => b::set_switch(&mut m, MAN, at, words[0]),
                    7 => f0 = Some(b::far(&m, MAN).0.to_bits()),
                    8 => f0 = Some(b::near(&m, MAN).0.to_bits()),
                    9 => b::set_far(&mut m, MAN, F(fl)),
                    _ => b::set_near(&mut m, MAN, F(fl)),
                }
                r.cpu.set_f(12, fl);
                (0x1db840 + 8 * k, vec![MAN, words[0]])
            }
        };
        set(&mut r, &args);
        r.call(func).unwrap_or_else(|e| panic!("case {case} kind {kind}: {e}"));
        let mut theirs = log.borrow().clone();
        if kind != 9 {
            // begin's second word is only the forwarder's (elsewhere what the last call left)
            for c in theirs.iter_mut().filter(|c| c.0 == vaddr(0x78)) {
                c.1.truncate(1);
            }
        }
        assert_eq!(theirs, ours.0, "case {case} kind {kind}: the calls");
        if let Some(v) = v0 {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case} kind {kind}: the answer");
        }
        if let Some(f) = f0 {
            assert_eq!(r.cpu.fpr[0], f, "case {case}: the float");
        }
        for (at, bytes) in &m.regions {
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} kind {kind}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
    }
    println!("kinds {kinds:?}; queued into lists 4, 6, 8, mixed, none {lists:?}");
    assert!(lists.iter().all(|&n| n > 100));
}

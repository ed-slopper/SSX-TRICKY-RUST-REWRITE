//! `AnimCtl_GetClipDisplacement` 0x15fa40 (board row G5c3c) against the original in the function runner
//! (TRICKY_ELF; skipped without it): random controllers' layer clips, the clip resolved and its key given by logging
//! stand-ins, the bank's file records pointing at random clip headers, offsets and curves of every kind, and a random
//! rider scale; `AnimData_GetRootOffset` and the curves run as the original. The output, the memory and the calls
//! compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animctl as c;
use tricky_game::animdata as d;
use tricky_game::animseq::SeqEnv;
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
const SIZE: u32 = 0x4000;
const CTL: u32 = ARENA;
const RIDER: u32 = ARENA + 0x200;
const OBJ: u32 = ARENA + 0x800;
const OUT: u32 = ARENA + 0x900;
const HDRS: u32 = ARENA + 0x1000;
const OFFS: u32 = ARENA + 0x1400;
const DATA: u32 = ARENA + 0x1800;
const BANK_LEN: u32 = 0x30f4;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

fn answer(f: u32, args: &[u32]) -> u32 {
    match f {
        c::RESOLVE => mix(args[0] ^ args[1] << 9) % 0x584,
        c::GET_CLIP_ID => (mix(args[1]) % 8) << 16 | mix(args[1] ^ 0x55) % d::NFILE,
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

fn b24(v: f32) -> [u8; 3] {
    let b = v.to_bits();
    [(b >> 8) as u8, (b >> 16) as u8, (b >> 24) as u8]
}

#[test]
fn the_clip_displacement_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x15fa40u32, 0x1586c0, 0x157ed8, 0x158030] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    for (f, n) in [(c::RESOLVE, 2usize), (c::GET_CLIP_ID, 2)] {
        let l = log.clone();
        r.stub(f, move |cpu, _| {
            let args: Vec<u32> = (0..n).map(|k| cpu.gpr(4 + k) as u32).collect();
            l.borrow_mut().push(Call(f, args.clone()));
            cpu.set_gpr(2, answer(f, &args) as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x15fa_0000_0000_0001);
    for case in 0..20_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (d::BANK, vec![0; BANK_LEN as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..BANK_LEN / 4 {
            m.w32(d::BANK + 4 * k, g.next());
        }
        // every file record: the arena's headers, offsets and data
        for i in 0..d::NFILE {
            let f = d::BANK + d::FILES + 0x18 * i;
            m.w32(f + 8, HDRS + 0x24 * g.below(8));
            m.w32(f + 0xc, OFFS);
            m.w32(f + 0x10, DATA);
        }
        for h in 0..16 {
            m.w8(HDRS + 0x24 * h + 9, if g.below(2) == 0 { 0 } else { 1 });
            m.w32(HDRS + 0x24 * h + 4, g.below(20));
        }
        // 24 curves: constants, lines and tables
        let mut at = 0u32;
        for k in 0..24 {
            m.w32(OFFS + 4 * k, at);
            let kind = g.below(3);
            let len = 1 + g.below(8);
            let mut v = ((len << 4 | [0, 1, 4][kind as usize]) as u16).to_le_bytes().to_vec();
            match kind {
                0 => v.extend_from_slice(&b24(g.f(-50.0, 50.0))),
                1 => (0..2).for_each(|_| v.extend_from_slice(&b24(g.f(-50.0, 50.0)))),
                _ => (0..len).for_each(|_| v.extend_from_slice(&g.f(-50.0, 50.0).to_le_bytes())),
            }
            for (i, b) in v.iter().enumerate() {
                m.w8(DATA + at + i as u32, *b);
            }
            at += v.len() as u32 + g.below(3);
        }
        m.w32(CTL + 0x24, RIDER);
        m.w32(RIDER + 0x464, OBJ);
        m.w32(OBJ + 0x60, [1.0f32, 0.5, g.f(0.0, 3.0)][g.below(3) as usize].to_bits());
        for k in 0..3 {
            m.w32(CTL + 8 + 4 * k, g.below(0x300));
        }
        for (a, b) in &m.regions {
            r.mem.write(*a, b).unwrap();
        }
        let k = g.below(3);
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let ans = c::clip_displacement(&mut m, &mut ours, OUT, CTL, k);
        for (i, v) in [OUT, CTL, k].iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as u64);
        }
        r.call(0x15fa40).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32, ans, "case {case}: answer");
        assert_eq!(*log.borrow(), ours.0, "case {case}: calls");
        for (a, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*a, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", a + i as u32, theirs[i], b[i]);
            }
        }
    }
}

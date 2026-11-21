//! `tricky_game::spin::update_trick_clips` (`SpinState_UpdateTrickClips` 0x1272e8, board row F4b3d2) against the
//! original in the function runner (TRICKY_ELF; skipped without it). Every case a random rider: its trick words
//! (+0x1e4, +0x1e8, +0x1ec), +0x24, +0x158, its stats (tricks byte, board, rider), and its animation controller with
//! sequences of trick clips, the air clip and others on layers 0 and 1 (random marker flags +0x74 / +0x75), the
//! layer-0 clip often a grab clip of the rider's own table. The controller's functions run as the original; the
//! called-out ones (`AnimCtl_PlayClip`, the scoring, the sounds) are logging stand-ins, `AnimCtl_PlayClip` setting
//! the clip's layer's current clip (no sequence begun). The rider's and controller's bytes, the calls and the answer
//! compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animctl as c;
use tricky_game::animseq::{self as a, SeqEnv};
use tricky_game::pf::Mem;
use tricky_game::spin;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
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
    fn pick(&mut self, v: &[u32]) -> u32 {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x6000;
const RIDER: u32 = ARENA;
const CTL: u32 = RIDER + 0x46c0;
const NODES: u32 = ARENA + 0x4900;
const NNODE: u32 = 8;
const TREES: u32 = ARENA + 0x5000;
const STATS: u32 = ARENA + 0x5800;

const CALLS: [(u32, usize, bool); 7] = [
    (spin::PLAY_CLIP, 4, false),
    (spin::START_GRAB, 2, false),
    (spin::TRICK_EVENT, 2, true),
    (spin::SET_TRICK_CLIP_RATE, 2, false),
    (spin::SFX_GRAB_HELD, 2, false),
    (spin::SFX_TRICK_STOP, 2, false),
    (spin::SFX_TRICK_START, 2, false),
];


struct Ours(Vec<(u32, Vec<u32>)>);
impl SeqEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.push((func, args.to_vec()));
        // `AnimCtl_PlayClip`'s stand-in: the clip becomes its layer's current clip (its row's +0x10 the layer)
        if func == spin::PLAY_CLIP {
            let k = m.r32(c::row(args[1]) + 0x10);
            m.w32(args[0] + 8 + 4 * k, args[1]);
        }
        0
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn the_trick_clips_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x1272e8u32, 0x203a40, 0x203ae0, 0x203b00, 0x203b48, 0x203b60, 0x203b98, 0x203bd8, 0x203c20, 0x203cb0, 0x203cf8,
        0x203d38, 0x159a80, 0x159d88, 0x159e38, 0x159e80, 0x159ff8, 0x15a030, 0x15a330, 0x15a3a8, 0x15a600, 0x15a6c8, 0x15a700, 0x15a760,
        0x15eec0, 0x15ef98, 0x15f050, 0x15fdd0, 0x15fec8, 0x15ffb8, 0x160008,
    ] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::<(u32, Vec<u32>)>::new()));
    for (f, n, fl) in CALLS {
        let l = log.clone();
        r.stub(f, move |cpu, mem| {
            let mut args: Vec<u32> = [4usize, 5, 6, 7][..n].iter().map(|&k| cpu.gpr(k) as u32).collect();
            if fl {
                args.push(cpu.f(12).to_bits());
            }
            if f == spin::PLAY_CLIP {
                let k = mem.read_u32(c::row(args[1]) + 0x10);
                mem.write_u32(args[0] + 8 + 4 * k, args[1]);
            }
            l.borrow_mut().push((f, args));
            cpu.set_gpr(2, 0);
        });
    }
    let mut consts = Vec::new();
    for (at, len) in [(c::CLIP_TABLE, 0x1c * 0x300u32), (spin::TRICK_TABLE, 12 * 0x2d0), (c::AUDIO, 4), (0, 0x100)] {
        let mut b = vec![0u8; len as usize];
        r.mem.read(at, &mut b).unwrap();
        consts.push((at, b));
    }
    let table = |rider: u32, board: u32, row: u32, k: u32| {
        let o = (rider * 0x2d0 + board * 0xf0 + row * 0x10 + 4 * k) as usize;
        u32::from_le_bytes(consts[1].1[o..o + 4].try_into().unwrap())
    };
    let mut g = Rng(0x1272_e800_0000_0001);
    // the ways through: [cases, answered 1]
    let mut seen = std::collections::BTreeMap::<String, [usize; 2]>::new();
    for case in 0..30_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        m.regions.extend(consts.iter().cloned());
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        let (rider_no, board) = (g.below(12), g.below(3));
        m.w8(STATS + 0x14, g.next() as u8);
        m.w32(STATS + 0x44, board);
        m.w32(STATS + 0x5c, rider_no);
        m.w32(RIDER + 0x464, STATS);
        let row = g.below(15);
        let slot = table(rider_no, board, row, 0);
        let grab = table(rider_no, board, row, 1);
        let trick_clips: Vec<u32> = (0x278..0x2dc).collect();
        m.w32(RIDER + 0x1e4, [0x1a, 0x1a, slot, g.below(0x1b), g.next()][g.below(5) as usize]);
        m.w32(RIDER + 0x1e8, [0, 0, 1, 3, 4, 4, g.below(6)][g.below(7) as usize]);
        m.w32(RIDER + 0x1ec, [0, 0, 1 + g.below(30)][g.below(3) as usize]);
        wf(&mut m, RIDER + 0x158, [0.0, g.f(-3.0, 3.0), g.f(-3.0, 3.0)][g.below(3) as usize]);
        wf(&mut m, RIDER + 0x24, [0.0, g.f(-3.0, 3.0), g.f(-3.0, 3.0)][g.below(3) as usize]);
        for k in 0..3 {
            a::list_init(&mut m, c::layer(CTL, k));
        }
        // clip trees: +8 the header, whose +0xa is the clip's length
        for t in 0..4 {
            let at = TREES + 0x40 * t;
            m.w32(at + 8, at + 0x20);
            m.w16(at + 0x20 + 0xa, 1 + g.below(80) as u16);
        }
        let layer0 = [grab, grab, 0x223, g.pick(&trick_clips), g.below(0x300)][g.below(5) as usize];
        for n in 0..g.below(NNODE + 1) {
            let at = NODES + 0x90 * n;
            let clip = [layer0, grab, table(rider_no, board, row, 2), g.pick(&trick_clips), 0x223, g.below(0x300)][g.below(6) as usize];
            m.w32(at + 0x40, clip);
            m.w32(at + 0x78, TREES + 0x40 * g.below(4));
            wf(&mut m, at + 0x10, [1.0, g.f(0.2, 2.0)][g.below(2) as usize]);
            a::list_insert(&mut m, c::layer(CTL, g.below(2)), at);
        }
        m.w32(CTL + 8, if g.below(8) == 0 { c::NONE } else { layer0 });
        m.w32(CTL + 0xc, g.below(0x301));
        m.w32(CTL + 0x10, g.below(0x301));
        let held = g.below(2);
        let trick = [0x1a, slot, m.r32(RIDER + 0x1e4), g.below(0x1b)][g.below(4) as usize];
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let (w1e4, w1e8, ty) = (m.r32(RIDER + 0x1e4), m.r32(RIDER + 0x1e8), c::clip_type(&m, CTL, 0));
        let ans = spin::update_trick_clips(&mut m, &mut ours, RIDER, held, trick);
        r.cpu.set_gpr(4, RIDER as u64);
        r.cpu.set_gpr(5, held as u64);
        r.cpu.set_gpr(6, trick as i32 as i64 as u64);
        r.call(0x1272e8).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        let game = r.cpu.gpr(2) as u32;
        let mut after = vec![0u8; SIZE as usize];
        r.mem.read(ARENA, &mut after).unwrap();
        let ctx = format!("case {case}: +0x1e4 {w1e4:#x} +0x1e8 {w1e8} type {ty:#x} held {held} trick {trick:#x}");
        assert_eq!(ans, game, "{ctx}: the answer");
        assert_eq!(ours.0, *log.borrow(), "{ctx}: the calls");
        if let Some(o) = after.iter().zip(&m.regions[0].1).position(|(x, y)| x != y) {
            panic!("{ctx}: byte +{o:#x} game {:#x} ours {:#x}", after[o], m.regions[0].1[o]);
        }
        let way = format!(
            "{} stage {} type {:#x}: {:?}",
            if w1e4 == 0x1a { "none playing" } else { "playing" },
            w1e8.min(5),
            if [0xe, 0xf, 0x10].contains(&ty) { ty } else { 0 },
            ours.0.iter().map(|(f, _)| *f).collect::<Vec<_>>()
        );
        let e = seen.entry(way).or_default();
        e[0] += 1;
        e[1] += ans as usize;
    }
    for (way, [n, one]) in &seen {
        eprintln!("{n:6} ({one} answered 1) {way}");
    }
}

//! `tricky_game::ps2lensfx` (board row G7d2e) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random PS2 lens FX node (flare entries, the sun's block, a view block, a packet
//! under way, a depth read-back), renderer (screen size, depth buffer, frames) and hardware state (the DMA channels,
//! VIF1, the GS idle); then one of 10 functions. The allocator, the memory fill, the base node, the DMA sends and
//! the waits are logging stand-ins; the hardware registers are plain memory in both (a DMA start finishes at once).
//! The memory, the registers written, the calls and the answers compared.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::ps2lensfx::{self as p, Ps2LensEnv};

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
const SIZE: u32 = 0x8000;
const APP: u32 = ARENA;
const RENDER: u32 = ARENA + 0x100;
const FRAMES: u32 = ARENA + 0x2000;
const PKT: u32 = ARENA + 0x2400;
const NODE: u32 = ARENA + 0x4000;
const NEW: u32 = ARENA + 0x4000;
/// The hardware registers the read-back touches, and their idle values
const REGS: [(u32, bool, u64); 7] = [
    (0x1000_9000, false, 0),
    (0x1000_9010, false, 0),
    (0x1000_9020, false, 0),
    (0x1000_a000, false, 0),
    (0x1000_3c00, false, 0),
    (0x1200_1000, true, 0x4000),
    (0x1200_1040, true, 0),
];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

fn answer(func: u32, args: &[u32]) -> u32 {
    match func {
        p::POOL_ALLOC | p::ALLOC => {
            if args[2] & 0x100 != 0 {
                0
            } else {
                NEW
            }
        }
        p::BASE_CONSTRUCT => args[0],
        _ => 0,
    }
}

struct Ours {
    log: Vec<Call>,
    io: BTreeMap<u32, u64>,
}
impl Ps2LensEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.log.push(Call(func, args.to_vec()));
        answer(func, args)
    }
    fn io_read(&mut self, _m: &mut Mem, addr: u32, wide: bool) -> u64 {
        let v = *self.io.get(&addr).unwrap_or(&0);
        if wide { v } else { v as u32 as i32 as i64 as u64 }
    }
    fn io_write(&mut self, _m: &mut Mem, addr: u32, v: u64, wide: bool) {
        let mut v = if wide { v } else { v & 0xffff_ffff };
        // (a DMA channel's start bit clears at once, as the runner has it)
        if (0x1000_8000..0x1000_f000).contains(&addr) && addr & 0xfff == 0 {
            v &= !0x100;
        }
        self.io.insert(addr, v);
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn ps2_lens_fx_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x1ea408u32, 0x1ea4d0, 0x1ea508, 0x1ea520, 0x1ea990, 0x1eaa40, 0x1eaa68, 0x1eab88, 0x1eab90, 0x1eb0a8, 0x2f79a8, 0x2f7018] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for (f, n) in [(p::POOL_ALLOC, 6), (p::ALLOC, 3), (p::MEM_SET, 3), (p::BASE_CONSTRUCT, 2), (p::BASE_DESTRUCT, 2), (p::FLUSH_CACHE, 1), (p::DMA_SEND_GIF, 3), (p::SYNC, 2)] {
        let l = log.clone();
        r.stub(f, move |c, _| {
            let args: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            c.set_gpr(2, answer(f, &args) as u64);
            l.borrow_mut().push(Call(f, args));
        });
    }
    let mut g = Rng(0x0952_0000_0000_0001);
    let mut counts = [0usize; 10];
    let (mut fans, mut seen) = (0usize, 0usize);
    for case in 0..20_000u32 {
        // (address 0's first words too: a create that got nothing still constructs there)
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (p::GAPP, vec![0; 4]), (0, vec![0; 0x10])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(p::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        // the renderer: its packet buffer, screen, depth buffer and frames
        m.w32(RENDER + 0x694, PKT);
        m.w32(RENDER + 0x698, [640u32, 512, 641][g.below(3) as usize]);
        m.w32(RENDER + 0x69c, [448u32, 224, 449][g.below(3) as usize]);
        m.w32(RENDER + 0x6a4, [0x30u32, 0x31, 0x32, 0x2f, 0][g.below(5) as usize]);
        m.w32(RENDER + 0x1710, [g.below(0x200), 0x400_0000 | g.below(0x200), g.next()][g.below(3) as usize]);
        m.w32(RENDER + 0x650, g.below(3));
        m.w32(RENDER + 0x654, FRAMES);
        for f in 0..3 {
            m.w32(FRAMES + 0x94 * f + 0x80, [640u32, 512, 0xffff_ff00][g.below(3) as usize]);
            m.w32(FRAMES + 0x94 * f + 0x8c, g.below(0x40));
        }
        // the node's packet under way
        m.w32(NODE + 0x3350, PKT);
        m.w32(NODE + 0x3354, 2 * g.below(16));
        for k in 0..6 {
            wf(&mut m, NODE + 0x2fc4 + 4 * k, g.f(0.0, 1.0));
        }
        // a flare entry (also the sun's block): on, its part showing, alpha, colours, boxes, texture boxes, corners
        let e = NODE + 0x1c0;
        m.w32(e, g.below(3));
        wf(&mut m, e + 0x20, [g.f(0.0, 70000.0), -5.0, 3.0e9][g.below(3) as usize]);
        for k in [0x44u32, 0x48, 0x4c, 0x50, 0x54, 0x6c, 0x70, 0x74, 0x7c, 0x80, 0x84] {
            wf(&mut m, e + k, [g.f(0.0, 1.0), 1.0, g.f(-0.5, 2.5)][g.below(3) as usize]);
        }
        for b in [0x48u32, 0x58, 0x68, 0x78, 0x88, 0xb8] {
            for k in 0..4 {
                m.w32(e + b + 4 * k, if g.below(10) == 0 { g.next() } else { g.below(300) });
            }
        }
        for k in 0..8 {
            wf(&mut m, e + 0x98 + 4 * k, g.f(-50.0, 700.0));
        }
        // the sun's outline (in the same block: points from +0x8c, lengths and fades from +0x1f4)
        let s = e;
        m.w32(s + 0x88, [0u32, 1, 2, 5, 12][g.below(5) as usize]);
        wf(&mut m, s + 0x10, g.f(0.0, 640.0));
        wf(&mut m, s + 0x14, g.f(0.0, 448.0));
        for k in 0..12 {
            wf(&mut m, s + 0x8c + 8 * k, g.f(-10.0, 650.0));
            wf(&mut m, s + 0x90 + 8 * k, g.f(-10.0, 460.0));
            wf(&mut m, s + 0x1f4 + 8 * k, g.f(0.0, 1.0));
            wf(&mut m, s + 0x1f8 + 8 * k, g.f(0.0, 1.0));
        }
        // a view block
        // (now and then off a 16-byte boundary, and of no size at the corner)
        let view = NODE + 0x30 + 0x60 * g.below(2) + if g.below(4) == 0 { 4 } else { 0 };
        for k in 0..4 {
            m.w32(view + 4 * k, if g.below(4) == 0 { 0 } else { g.below(700) });
        }
        // the read-back: the box, the clipped box (the same, or part of it), the depths
        let bx = NODE + 0x2000;
        let b2 = NODE + 0x2010;
        let (x, y, w, h) = (g.below(600), g.below(400), 1 + g.below(8), 1 + g.below(8));
        for (k, v) in [x, y, w, h].into_iter().enumerate() {
            m.w32(bx + 4 * k as u32, v);
        }
        let same = g.below(2) == 0;
        let (x2, y2) = if same { (x, y) } else { (x + g.below(3), y.saturating_sub(g.below(3))) };
        let (w2, h2) = if same { (w, h) } else { (g.below(w + 1), 1 + g.below(h)) };
        for (k, v) in [x2, y2, w2, h2].into_iter().enumerate() {
            m.w32(b2 + 4 * k as u32, v);
        }
        let limit = g.below(0x100_0000);
        for k in 0..w * h {
            let d = if g.below(2) == 0 { limit.wrapping_add(g.below(3)).wrapping_sub(1) } else { g.next() } & 0xff_ffff;
            for j in 0..3 {
                m.w8(NODE + 0x3050 + 3 * k + j, (d >> (8 * j)) as u8);
            }
        }
        r.mem.write(ARENA, &m.regions[0].1).unwrap();
        r.mem.write(p::GAPP, &m.regions[1].1).unwrap();
        r.mem.write(0, &m.regions[2].1).unwrap();
        let mut ours = Ours { log: Vec::new(), io: BTreeMap::new() };
        for (a, wide, v) in REGS {
            if wide {
                r.mem.write(a, &v.to_le_bytes()).unwrap();
            } else {
                r.mem.write_u32(a, v as u32);
            }
            ours.io.insert(a, v);
        }
        log.borrow_mut().clear();
        let kind = g.below(10) as usize;
        let mut ans = None;
        let mut fans_ = None;
        let mut fl = Vec::new();
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let flags = [0u32, 0x4000_0000, 0x8000_0000, 0xc000_0000, 0x100, 0x4000_0100, 0x123_4567][g.below(7) as usize];
                let a0 = g.next();
                ans = Some(p::create(&mut m, &mut ours, a0, flags));
                (0x1ea408, vec![a0, flags])
            }
            1 => {
                let a1 = g.next();
                ans = Some(p::construct(&mut m, &mut ours, NODE, a1));
                (0x1ea4d0, vec![NODE, a1])
            }
            2 => {
                let a1 = g.below(4);
                p::destruct(&mut m, &mut ours, NODE, a1);
                (0x1ea508, vec![NODE, a1])
            }
            3 => {
                p::begin(&mut m, NODE);
                (0x1ea990, vec![NODE])
            }
            4 => {
                p::end(&mut m, NODE);
                (0x1eaa40, vec![NODE])
            }
            5 => {
                p::end_view(&mut m, NODE);
                (0x1eab88, vec![NODE])
            }
            6 => {
                p::set_view(&mut m, NODE, view);
                (0x1eaa68, vec![NODE, view])
            }
            7 => {
                p::draw_flare(&mut m, NODE, e);
                (0x1eab90, vec![NODE, e])
            }
            8 => {
                p::draw_sun(&mut m, NODE, s);
                if m.r32(s) != 0 && m.r32(s + 0x88) > 1 {
                    fans += 1;
                }
                (0x1eb0a8, vec![NODE, s])
            }
            _ => {
                let d = [limit as f32, g.f(0.0, 1.6e7), -1.0, 5.0e9, f32::from_bits(0x7fc0_0000)][g.below(5) as usize];
                fl.push(d);
                let v = p::visibility(&mut m, &mut ours, NODE, bx, b2, F(d));
                fans_ = Some(v.0.to_bits());
                if v.0 > 0.0 {
                    seen += 1;
                }
                (0x1ea520, vec![NODE, bx, b2])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        for (i, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + i, *v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        if let Some(v) = fans_ {
            assert_eq!(r.cpu.f(0).to_bits(), v, "case {case}: {func:#x} float answer");
        }
        assert_eq!(*log.borrow(), ours.log, "case {case}: {func:#x} calls");
        for (a, wide, _) in REGS {
            let theirs = if wide {
                let mut b = [0u8; 8];
                r.mem.read(a, &mut b).unwrap();
                u64::from_le_bytes(b)
            } else {
                r.mem.read_u32(a) as u64
            };
            assert_eq!(theirs, ours.io[&a], "case {case}: {func:#x} register {a:#x}");
        }
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&theirs), w(b));
            }
        }
    }
    println!("{counts:?}, {fans} suns' fans, {seen} flares partly seen");
    assert!(fans > 300 && seen > 300);
}

//! `tricky_game::puff` (board row G7f) against `Fx_BoardPuff_Update` 0x12eca8 and `Fx_SpeedDust_Update` 0x12f4c0 in
//! the function runner (TRICKY_ELF; skipped without it), the random words scripted, the whole particle block
//! compared word for word after every call.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::puff::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
    fn small(&mut self) -> f32 {
        (self.next() % 2000) as f32 / 1000.0 - 1.0
    }
    fn v(&mut self) -> [f32; 4] {
        [self.f(), self.f(), self.f(), self.f()]
    }
    fn col(&mut self) -> [f32; 4] {
        if self.next() % 4 == 0 {
            [self.f(), self.f(), self.f(), self.f()]
        } else {
            [0.0, (self.next() % 100) as f32 / 99.0, (self.next() % 100) as f32 / 99.0, (self.next() % 100) as f32 / 99.0]
        }
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

fn scripted(r: &mut Runner) -> Rc<RefCell<VecDeque<u32>>> {
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    rand
}

/// A ring block with a made-up count and start and every word a sane float.
fn fill(g: &mut Rng, w: &mut [u32], at: usize) {
    for x in w.iter_mut() {
        *x = (g.small().abs() * 2.0).to_bits();
    }
    w[at / 4] = [0, 1, 2, 10, 62, 63, 64][(g.next() % 7) as usize];
    w[at / 4 + 1] = g.next() % 64;
}

#[test]
fn board_puff_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let rand = scripted(&mut r);
    let block = r.mem.alloc(0x1d00);
    let rider = r.mem.alloc(0x5c00);
    let light = r.mem.alloc(0x500);
    r.mem.write_u32(rider + 0x5ae0, light);
    let mut g = Rng(0x9ff0_0012_eca8_0001);
    let mut p = Puffs::default();
    let (mut made, mut aged_out) = (0, 0);
    for case in 0..30_000 {
        if case % 2 == 0 {
            fill(&mut g, &mut p.w, 0x1c10);
            // ages near the limit so some drop
            let fresh = g.next() % 3 == 0;
            for k in 0..64 {
                let a = if fresh { 0.0 } else { [0.0f32, 1.99, 1.98, 1.5, 2.1][(g.next() % 5) as usize] };
                p.w[(0x10 + 0x70 * k) / 4] = a.to_bits();
            }
        }
        let at = g.v();
        let near = g.next() % 3 == 0;
        let first = if near { [at[0] + g.small() * 30.0, at[1], at[2] + g.small() * 30.0, at[3]] } else { g.v() };
        for k in 0..4 {
            p.w[k] = first[k].to_bits();
        }
        let ri = PuffIn {
            hit: [0u32, 1, 2, 3][(g.next() % 4) as usize],
            motion: [2u32, 2, 2, 5, 0][(g.next() % 5) as usize],
            surface: [0u32, 1, 3, 4, 5, 8, 9, 0xffff_fffe][(g.next() % 8) as usize],
            turn: [0.0f32, 0.1, 0.2, 0.21, -0.5, 1.0, g.small()][(g.next() % 7) as usize],
            at,
            axis: g.v(),
            colour: g.col(),
        };
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_u32(rider + 0x290, ri.surface);
        r.mem.write_f32(rider + 0x214, ri.turn);
        put(&mut r, rider + 0x4a90, ri.at);
        put(&mut r, rider + 0x1a0, ri.axis);
        put(&mut r, light + 0x470, ri.colour);
        for (i, w) in p.w.iter().enumerate() {
            r.mem.write_u32(block + 4 * i as u32, *w);
        }
        let words: VecDeque<u32> = (0..4).map(|_| g.next() | (g.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        let before = p.clone();
        r.cpu.set_gpr(4, block as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x12eca8).unwrap();
        let mut m = words.clone();
        puff_update(&mut p, &ri, &mut || m.pop_front().unwrap());
        assert_eq!(rand.borrow().len(), m.len(), "case {case} rand use");
        if m.len() < words.len() {
            made += 1;
        }
        if p.w[0x1c10 / 4] + 1 < before.w[0x1c10 / 4] + (m.len() < words.len()) as u32 {
            aged_out += 1;
        }
        for (i, w) in p.w.iter().enumerate() {
            assert_eq!(r.mem.read_u32(block + 4 * i as u32), *w, "case {case} byte {:#x} {ri:?}", 4 * i);
        }
    }
    assert!(made > 1500 && aged_out > 200, "{made} {aged_out}");
}

#[test]
fn speed_dust_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let rand = scripted(&mut r);
    r.allow(0x102f50);
    let block = r.mem.alloc(0xe00);
    let rider = r.mem.alloc(0x5c00);
    let light = r.mem.alloc(0x500);
    r.mem.write_u32(rider + 0x5ae0, light);
    let mut g = Rng(0x9ff0_0012_f4c0_0002);
    let mut d = Dust::default();
    let mut made = 0;
    for case in 0..30_000 {
        if case % 2 == 0 {
            fill(&mut g, &mut d.w, 0xd50);
            let fresh = g.next() % 3 == 0;
            for k in 0..64 {
                let a = if fresh { 0.0 } else { [0.0f32, 0.99, 0.98, 0.5, 1.1][(g.next() % 5) as usize] };
                d.w[(0x50 + 0x34 * k) / 4] = a.to_bits();
            }
        }
        let ri = DustIn {
            hit: [0u32, 1, 2, 3][(g.next() % 4) as usize],
            motion: [2u32, 2, 2, 5, 0][(g.next() % 5) as usize],
            surface: [0u32, 1, 3, 4, 4, 5, 0xffff_fffe][(g.next() % 7) as usize],
            ground: g.v(),
            other: g.v(),
            vel: if g.next() % 3 == 0 { g.v() } else { [g.f() * 20.0, g.f(), g.f() * 20.0, 0.0] },
            up: if g.next() % 3 == 0 { g.v() } else { [g.small() * 0.3, 1.0, g.small() * 0.3, 0.0] },
            colour: g.col(),
        };
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_u32(rider + 0x290, ri.surface);
        put(&mut r, rider + 0x2d0, ri.ground);
        put(&mut r, rider + 0x140, ri.other);
        put(&mut r, rider + 0x150, ri.vel);
        put(&mut r, rider + 0x2a0, ri.up);
        put(&mut r, light + 0x470, ri.colour);
        for (i, w) in d.w.iter().enumerate() {
            r.mem.write_u32(block + 4 * i as u32, *w);
        }
        let words: VecDeque<u32> = (0..64).map(|_| g.next() | (g.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        r.cpu.set_gpr(4, block as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x12f4c0).unwrap();
        let mut m = words.clone();
        dust_update(&mut d, &ri, &mut || m.pop_front().unwrap());
        assert_eq!(rand.borrow().len(), m.len(), "case {case} rand use");
        if m.len() < words.len() {
            made += 1;
        }
        for (i, w) in d.w.iter().enumerate() {
            assert_eq!(r.mem.read_u32(block + 4 * i as u32), *w, "case {case} byte {:#x} {ri:?}", 4 * i);
        }
    }
    assert!(made > 1500, "{made}");
}

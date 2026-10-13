//! `tricky_game::boardtrail` (board row G7f) against `Fx_BoardTrail_Update` 0x12fef0 (with Push 0x130960 and
//! SetPoint 0x1306b0 running as original code) in the function runner (TRICKY_ELF; skipped without it): the
//! whole trail object compared word for word after each call.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tricky_game::boardtrail::*;

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
    fn u(&mut self) -> [f32; 4] {
        [self.small(), self.small(), self.small(), 0.0]
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn board_trail_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    r.allow(0x130960);
    r.allow(0x1306b0);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let trail = r.mem.alloc(BYTES as u32);
    let rider = r.mem.alloc(0x5c00);
    let colour = r.mem.alloc(0x500);
    let app = r.mem.alloc(0x800);
    let mid = r.mem.alloc(0x40);
    let tab = r.mem.alloc(0x400);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, mid);
    r.mem.write_u32(mid + 0x24, tab);
    r.mem.write_u32(rider + 0x5ae0, colour);
    let mut g = Rng(0x12fe_f0a5_0b13_d777);
    let mut t = Trail::default();
    let (mut opened, mut closed, mut grew, mut aged_out) = (0, 0, 0, 0);
    for case in 0..30_000 {
        if case % 2 == 0 || t.w[0] > 1 {
            for w in t.w.iter_mut() {
                *w = (g.small() * 20.0).to_bits();
            }
            t.w[0] = (g.next() % 2 == 0) as u32;
            t.w[4 / 4] = g.next() % 200;
            let count = [0u32, 1, 2, 5, 40, 74, 75][(g.next() % 7) as usize];
            t.w[0x50 / 4] = count;
            let start = [0u32, 1, 30, 73, 74][(g.next() % 5) as usize];
            t.w[0x54 / 4] = start;
            t.w[0x30 / 4] = ([0.0f32, 0.29, -0.29, 0.1, g.small() * 0.3][(g.next() % 5) as usize]).to_bits();
            t.w[0x34 / 4] = ([0.0f32, 0.5, -0.5, g.small()][(g.next() % 4) as usize]).to_bits();
            t.w[0x38 / 4] = ([0.0f32, 0.01, 0.5, g.small().abs()][(g.next() % 4) as usize]).to_bits();
            let young = g.next() % 2 == 0;
            for i in 0..75usize {
                let b = 0x60 + 0xa0 * i;
                let a = if young { [0.0f32, 0.1, 0.5, 0.2][(g.next() % 4) as usize] } else { [0.0f32, 0.1, 0.5, 0.88, 0.8999, 0.9, 0.95][(g.next() % 7) as usize] };
                t.w[(b + 0x94) / 4] = a.to_bits();
            }
        }
        let ri = TrailIn {
            motion: [2u32, 2, 2, 5, 0][(g.next() % 5) as usize],
            hit: [0u32, 1, 2, 3, 4][(g.next() % 5) as usize],
            f1fc: [0.0f32, 0.25, -0.25, 1.0, g.small()][(g.next() % 5) as usize],
            f214: [0.0f32, 0.5, -0.5, 1.0, -1.0, 0.3, 0.31, g.small()][(g.next() % 8) as usize],
            flipped: (g.next() % 2) as u32,
            w4a60: if g.next() % 4 == 0 { g.v() } else { g.u() },
            up: if g.next() % 4 == 0 { g.v() } else { [g.small() * 0.3, 1.0, g.small() * 0.3, 0.0] },
            vel: if g.next() % 6 == 0 { [0.0; 4] } else { g.v() },
            ground: g.v(),
            colour: [g.small().abs(), g.small().abs() * 1.2, g.small().abs(), g.small().abs()],
            cfg: [g.small().abs() * 3.0, g.small() * 2.0, g.small() * 0.01],
        };
        let surface = g.next() % 4;
        r.mem.write_u32(rider + 0x290, surface);
        let c = tab + surface * 100;
        for k in 0..3 {
            r.mem.write_f32(c + 0x58 + 4 * k, ri.cfg[k as usize]);
        }
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        r.mem.write_f32(rider + 0x1fc, ri.f1fc);
        r.mem.write_f32(rider + 0x214, ri.f214);
        r.mem.write_u32(rider + 0x1b4, ri.flipped);
        put(&mut r, rider + 0x4a60, ri.w4a60);
        put(&mut r, rider + 0x2a0, ri.up);
        put(&mut r, rider + 0x150, ri.vel);
        put(&mut r, rider + 0x2d0, ri.ground);
        put(&mut r, colour + 0x470, ri.colour);
        for (i, w) in t.w.iter().enumerate() {
            r.mem.write_u32(trail + 4 * i as u32, *w);
        }
        let before = t.clone();
        let words: VecDeque<u32> = (0..4).map(|_| g.next() | (g.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        r.cpu.set_gpr(4, trail as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x12fef0).unwrap();
        let mut rd = words;
        update(&mut t, &ri, &mut || rd.pop_front().expect("rand"));
        if before.w[0] == 0 && t.w[0] == 1 {
            opened += 1;
        } else if before.w[0] == 1 && t.w[0] == 0 {
            closed += 1;
        }
        if t.w[4 / 4] != before.w[4 / 4] {
            grew += 1;
        }
        if t.w[0x50 / 4] < before.w[0x50 / 4] {
            aged_out += 1;
        }
        for (i, w) in t.w.iter().enumerate() {
            assert_eq!(r.mem.read_u32(trail + 4 * i as u32), *w, "case {case} word {i:#x} (rider {ri:?})");
        }
    }
    eprintln!("opened {opened} closed {closed} grew {grew} aged_out {aged_out}");
    assert!(opened > 200 && closed > 200 && grew > 1000 && aged_out > 200);
}

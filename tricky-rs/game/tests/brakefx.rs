//! `tricky_game::brakefx` (board row G7f) against `Fx_BrakeSpray_Update` 0x12e6e8 (with `Fx_BrakeFan_Update`
//! 0x12d108, `Fx_BrakeFan_Emit` 0x12c4d0 and `Fx_Spray_SpawnParticle` 0x12e330 run for real) in the function
//! runner (TRICKY_ELF; skipped without it), the random words scripted, the whole effect object compared word for
//! word.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::brakefx::*;
use tricky_game::brakespray::SprayEnv;
use tricky_game::splash::RING_WORDS;

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
        [0.0, (self.next() % 100) as f32 / 99.0, (self.next() % 100) as f32 / 99.0, (self.next() % 100) as f32 / 99.0]
    }
}

struct Env(VecDeque<u32>);
impl SprayEnv for Env {
    fn rand_fx(&mut self) -> u32 {
        self.0.pop_front().unwrap()
    }
    fn splash(&mut self, _: [f32; 4], _: [f32; 4], _: [f32; 4]) {
        unreachable!()
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

const SPLASH: u32 = 0x970;

fn load(r: &mut Runner, at: u32, b: &BrakeFx) {
    for (i, e) in b.pool.entries.iter().enumerate() {
        for (k, w) in e.iter().enumerate() {
            r.mem.write_u32(at + 0x50 * i as u32 + 4 * k as u32, *w);
        }
    }
    r.mem.write_u32(at + 0x960, b.pool.count as u32);
    r.mem.write_u32(at + 0x964, b.pool.head as u32);
    r.mem.write_f32(at + 0x968, b.pool.recent);
    r.mem.write_f32(at + 0x96c, b.pool.level);
    let s = &b.splash;
    for k in 0..4 {
        r.mem.write_f32(at + SPLASH + 4 * k, s.last[k as usize]);
    }
    r.mem.write_f32(at + SPLASH + 0x10, s.gathered);
    r.mem.write_f32(at + SPLASH + 0x14, s.timer);
    r.mem.write_u32(at + SPLASH + 0x18, s.count as u32);
    r.mem.write_u32(at + SPLASH + 0x1c, s.slot as u32);
    for (i, ring) in s.rings.iter().enumerate() {
        for (k, w) in ring.iter().enumerate() {
            r.mem.write_u32(at + SPLASH + 0x20 + 0x3c0 * i as u32 + 4 * k as u32, *w);
        }
    }
}

fn check(r: &mut Runner, at: u32, b: &BrakeFx, what: &str) {
    for (i, e) in b.pool.entries.iter().enumerate() {
        for (k, w) in e.iter().enumerate() {
            assert_eq!(r.mem.read_u32(at + 0x50 * i as u32 + 4 * k as u32), *w, "{what} spray {i} word {k}");
        }
    }
    assert_eq!(r.mem.read_u32(at + 0x960) as i32, b.pool.count, "{what} count");
    assert_eq!(r.mem.read_u32(at + 0x964) as i32, b.pool.head, "{what} head");
    assert_eq!(r.mem.read_u32(at + 0x968), b.pool.recent.to_bits(), "{what} recent");
    assert_eq!(r.mem.read_u32(at + 0x96c), b.pool.level.to_bits(), "{what} level");
    let s = &b.splash;
    for k in 0..4 {
        assert_eq!(r.mem.read_u32(at + SPLASH + 4 * k), s.last[k as usize].to_bits(), "{what} last {k}");
    }
    assert_eq!(r.mem.read_u32(at + SPLASH + 0x10), s.gathered.to_bits(), "{what} gathered");
    assert_eq!(r.mem.read_u32(at + SPLASH + 0x14), s.timer.to_bits(), "{what} timer");
    assert_eq!(r.mem.read_u32(at + SPLASH + 0x18) as i32, s.count, "{what} fan count");
    assert_eq!(r.mem.read_u32(at + SPLASH + 0x1c) as i32, s.slot, "{what} slot");
    for (i, ring) in s.rings.iter().enumerate() {
        for (k, w) in ring.iter().enumerate() {
            let got = r.mem.read_u32(at + SPLASH + 0x20 + 0x3c0 * i as u32 + 4 * k as u32);
            assert_eq!(got, *w, "{what} ring {i} word {k}");
        }
    }
}

#[test]
fn brake_spray_update_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    r.allow(0x250c10);
    r.allow(0x12c4d0);
    r.allow(0x12d108);
    r.allow(0x12e330);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let this = r.mem.alloc(0x1c60);
    let rider = r.mem.alloc(0x5c00);
    let light = r.mem.alloc(0x500);
    let (g0, h0, table) = (r.mem.alloc(0x800), r.mem.alloc(0x40), r.mem.alloc(0x64 * 40));
    r.mem.write_u32(0x337c58, g0);
    r.mem.write_u32(g0 + 0x730, h0);
    r.mem.write_u32(h0 + 0x24, table);
    r.mem.write_u32(rider + 0x5ae0, light);
    let mut g = Rng(0x12e6_e8b7_a5d0_0001);
    let mut b = BrakeFx::default();
    let (mut fans, mut sprays, mut gone) = (0, 0, 0);
    for case in 0..30_000 {
        if case % 2 == 0 {
            for e in b.pool.entries.iter_mut() {
                for w in e.iter_mut() {
                    *w = (g.small().abs() * 2.0).to_bits();
                }
            }
            for e in b.pool.entries.iter_mut() {
                e[16] = [0.0f32, 1.99, 1.98, 0.5, 2.1][(g.next() % 5) as usize].to_bits();
            }
            b.pool.count = [0, 1, 2, 10, 29, 30][(g.next() % 6) as usize];
            b.pool.head = (g.next() % 30) as i32;
            b.pool.recent = [0.0f32, 0.3, 0.55, 0.6, 1.0][(g.next() % 5) as usize];
            b.pool.level = [0.0f32, 0.05, 0.5, 0.95, 1.0, 1.5][(g.next() % 6) as usize];
            for ring in b.splash.rings.iter_mut() {
                for w in ring.iter_mut() {
                    *w = (g.small().abs() * 2.0).to_bits();
                }
                ring[238] = [0.0f32, 0.5, 0.99, 0.9949][(g.next() % 4) as usize].to_bits();
            }
            b.splash.count = (g.next() % 6) as i32;
            b.splash.slot = (g.next() % 5) as i32;
            b.splash.gathered = g.small().abs();
            b.splash.timer = [0.0f32, -0.1, 0.3, 1.0][(g.next() % 4) as usize];
            b.splash.last = g.v();
        }
        let surface = g.next() % 40;
        let cfg58 = [0.0f32, 0.01, 0.02, 0.5, 1.0][(g.next() % 5) as usize];
        let ri = FanIn {
            hit: [0u32, 1, 2, 3][(g.next() % 4) as usize],
            motion: [2u32, 2, 2, 5][(g.next() % 4) as usize],
            f1fc: [0.0f32, 0.5, 0.7, 0.71, -0.9, 1.0][(g.next() % 6) as usize],
            cfg58,
            vel: if g.next() % 4 == 0 { g.v() } else { [g.f() * 6.0, g.small(), g.f() * 6.0, 0.0] },
            up: if g.next() % 3 == 0 { g.v() } else { [g.small() * 0.3, 1.0, g.small() * 0.3, 0.0] },
            ground: g.v(),
            w4a60: g.v(),
            colour: g.col(),
        };
        r.mem.write_u32(rider + 0x290, surface);
        r.mem.write_f32(table + 0x64 * surface + 0x58, cfg58);
        r.mem.write_u32(rider + 0x2e0, ri.hit);
        r.mem.write_u32(rider + 0x424, ri.motion);
        r.mem.write_f32(rider + 0x1fc, ri.f1fc);
        put(&mut r, rider + 0x150, ri.vel);
        put(&mut r, rider + 0x2a0, ri.up);
        put(&mut r, rider + 0x2d0, ri.ground);
        put(&mut r, rider + 0x4a60, ri.w4a60);
        put(&mut r, light + 0x470, ri.colour);
        load(&mut r, this, &b);
        let words: VecDeque<u32> = (0..40).map(|_| g.next() | (g.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        let before = b.clone();
        r.cpu.set_gpr(4, this as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x12e6e8).unwrap();
        let mut env = Env(words.clone());
        brake_update(&mut b, &ri, &mut env);
        assert_eq!(rand.borrow().len(), env.0.len(), "case {case} rand use");
        let used = words.len() - env.0.len();
        if used >= 14 {
            fans += 1;
        }
        if used == 4 || used == 18 {
            sprays += 1;
        }
        if b.splash.count == 0 && before.splash.count > 0 || b.pool.count + 1 < before.pool.count {
            gone += 1;
        }
        check(&mut r, this, &b, &format!("case {case} {ri:?}"));
    }
    assert!(fans > 500 && sprays > 500 && gone > 500, "{fans} {sprays} {gone}");
}

#[test]
fn ring_words_are_the_size_the_object_has() {
    assert_eq!(RING_WORDS * 4, 0x3c0);
}

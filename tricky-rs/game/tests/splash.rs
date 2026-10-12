//! `tricky_game::splash` (board row G19b) against `Fx_Splash_EmitAtPoint` 0x12c7e0 and `Fx_Splash_EmitRing`
//! 0x12c190 in the function runner (TRICKY_ELF; skipped without it), the random words scripted, the whole splash
//! object compared word for word.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::splash::*;

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
            3 => (self.next() % 2000) as f32 / 1000.0 - 0.5,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn v(&mut self) -> [f32; 4] {
        [self.f(), self.f(), self.f(), self.f()]
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

fn load(r: &mut Runner, this: u32, s: &Splash) {
    for k in 0..4 {
        r.mem.write_f32(this + 4 * k, s.last[k as usize]);
    }
    r.mem.write_f32(this + 0x10, s.gathered);
    r.mem.write_u32(this + 0x18, s.count as u32);
    r.mem.write_u32(this + 0x1c, s.slot as u32);
    for (i, ring) in s.rings.iter().enumerate() {
        for (k, w) in ring.iter().enumerate() {
            r.mem.write_u32(this + 0x20 + 0x3c0 * i as u32 + 4 * k as u32, *w);
        }
    }
}

fn check(r: &mut Runner, this: u32, s: &Splash, what: &str) {
    for k in 0..4 {
        assert_eq!(r.mem.read_u32(this + 4 * k), s.last[k as usize].to_bits(), "{what} last {k}");
    }
    assert_eq!(r.mem.read_u32(this + 0x10), s.gathered.to_bits(), "{what} gathered");
    assert_eq!(r.mem.read_u32(this + 0x18) as i32, s.count, "{what} count");
    assert_eq!(r.mem.read_u32(this + 0x1c) as i32, s.slot, "{what} slot");
    for (i, ring) in s.rings.iter().enumerate() {
        for (k, w) in ring.iter().enumerate() {
            let got = r.mem.read_u32(this + 0x20 + 0x3c0 * i as u32 + 4 * k as u32);
            assert_eq!(got, *w, "{what} ring {i} word {k}");
        }
    }
}

#[test]
fn splash_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    r.allow(0x250c10);
    r.allow(0x12c190);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let this = r.mem.alloc(0x20 + 0x3c0 * 5 + 0x10);
    let rider = r.mem.alloc(0x5c00);
    let cols = r.mem.alloc(0x500);
    let (g, h, table) = (r.mem.alloc(0x800), r.mem.alloc(0x40), r.mem.alloc(0x64 * 8));
    r.mem.write_u32(0x337c58, g);
    r.mem.write_u32(g + 0x730, h);
    r.mem.write_u32(h + 0x24, table);
    let (pa, va, na) = (r.mem.alloc(0x10), r.mem.alloc(0x10), r.mem.alloc(0x10));
    let mut gen = Rng(0x5911_ab5f_0012_c7e0);
    let mut s = Splash::default();
    let (mut emitted, mut skipped) = (0, 0);
    for case in 0..20_000 {
        if case % 3 == 0 {
            s.count = (gen.next() % 6) as i32;
            s.slot = (gen.next() % 5) as i32;
            s.gathered = [0.0, 0.1, 0.29, 0.3, 0.31, 2.0][(gen.next() % 6) as usize];
        }
        if case % 4 == 0 {
            // a ring already there, so the ones the next call leaves alone are not blank
            for ring in s.rings.iter_mut() {
                for w in ring.iter_mut() {
                    *w = gen.next() | (gen.next() << 16);
                }
            }
        }
        let pos = gen.v();
        let near = gen.next() % 3 == 0;
        s.last = if near { pos } else { gen.v() };
        let mut n = gen.v();
        if gen.next() % 3 != 0 {
            n[2] = 0.3 + (gen.next() % 700) as f32 / 1000.0;
        }
        let v = gen.v();
        let colour = gen.v();
        let rpos = gen.v();
        let rpos = if gen.next() % 3 == 0 { [rpos[0] * 200.0, rpos[1] * 200.0, rpos[2] * 200.0, rpos[3]] } else { rpos };
        let mode = [0u32, 1, 2, 3][(gen.next() % 4) as usize];
        let sc = [0.0f32, 0.01, 0.02, 0.5, 1.0][(gen.next() % 5) as usize] * (gen.next() % 3) as f32;
        let ri = SplashRider { mode, pos: rpos, colour, scale: sc };
        let idx = gen.next() % 8;
        r.mem.write_u32(rider + 0x290, idx);
        r.mem.write_u32(rider + 0x2e0, mode);
        r.mem.write_u32(rider + 0x5ae0, cols);
        r.mem.write_f32(table + 0x64 * idx + 0x58, sc);
        put(&mut r, rider + 0x150, rpos);
        put(&mut r, cols + 0x470, colour);
        put(&mut r, pa, pos);
        put(&mut r, va, v);
        put(&mut r, na, n);
        load(&mut r, this, &s);
        let words: VecDeque<u32> = (0..27).map(|_| gen.next() | (gen.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        r.cpu.set_gpr(4, this as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.cpu.set_gpr(6, pa as u64);
        r.cpu.set_gpr(7, va as u64);
        r.cpu.set_gpr(8, na as u64);
        r.call(0x12c7e0).unwrap();
        let before = s.count;
        let mut m = words.clone();
        emit_at_point(&mut s, &ri, pos, v, n, &mut || m.pop_front().unwrap());
        assert_eq!(rand.borrow().len(), m.len(), "case {case} rand use");
        if m.len() < words.len() {
            emitted += 1;
        } else {
            skipped += 1;
        }
        let _ = before;
        check(&mut r, this, &s, &format!("case {case}"));
    }
    assert!(emitted > 1000 && skipped > 1000, "{emitted} {skipped}");
}

#[test]
fn ring_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    r.allow(0x250c10);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let ring = r.mem.alloc(0x3c0);
    let (na, ea, ua) = (r.mem.alloc(0x10), r.mem.alloc(0x10), r.mem.alloc(0x10));
    let mut gen = Rng(0x0ff1_c190_0000_0001);
    for case in 0..8_000 {
        let mut w = [0u32; RING_WORDS];
        for x in w.iter_mut() {
            *x = gen.next() | (gen.next() << 16);
        }
        w[237] = 1 + gen.next() % 25;
        for (k, x) in w.iter().enumerate() {
            r.mem.write_u32(ring + 4 * k as u32, *x);
        }
        let (n, e2, u1) = (gen.v(), gen.v(), gen.v());
        put(&mut r, na, n);
        put(&mut r, ea, e2);
        put(&mut r, ua, u1);
        let ks = [gen.f(), gen.f(), gen.f(), gen.f()];
        let words: VecDeque<u32> = (0..26).map(|_| gen.next() | (gen.next() << 16)).collect();
        *rand.borrow_mut() = words.clone();
        r.cpu.set_gpr(5, ring as u64);
        r.cpu.set_gpr(6, na as u64);
        r.cpu.set_gpr(7, ea as u64);
        r.cpu.set_gpr(8, ua as u64);
        for (i, k) in ks.iter().enumerate() {
            r.cpu.set_f(12 + i, *k);
        }
        r.call(0x12c190).unwrap();
        let mut m = words;
        emit_ring(&mut w, n, e2, u1, ks[0], ks[1], ks[2], ks[3], &mut || m.pop_front().unwrap());
        assert_eq!(rand.borrow().len(), m.len(), "case {case} rand use");
        for (k, x) in w.iter().enumerate() {
            assert_eq!(r.mem.read_u32(ring + 4 * k as u32), *x, "case {case} word {k}");
        }
    }
}

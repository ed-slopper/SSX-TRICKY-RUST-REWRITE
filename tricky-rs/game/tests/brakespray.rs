//! `tricky_game::brakespray` (board row G19) against `Fx_Spray_SpawnParticle` 0x12e330 and `Fx_BrakeSpray_Emit`
//! 0x12e008 in the function runner (TRICKY_ELF; skipped without it), the random words scripted and the splash
//! emitter 0x12c7e0 a logging stub.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::brakespray::*;

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

struct Env {
    rand: VecDeque<u32>,
    splashes: Vec<[[u32; 4]; 3]>,
}
impl SprayEnv for Env {
    fn rand_fx(&mut self) -> u32 {
        self.rand.pop_front().unwrap()
    }
    fn splash(&mut self, a: [f32; 4], b: [f32; 4], c: [f32; 4]) {
        self.splashes.push([a, b, c].map(|v| v.map(f32::to_bits)));
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn brake_spray_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, q.borrow_mut().pop_front().expect("rand") as i32 as i64 as u64));
    let splashes: Rc<RefCell<Vec<[[u32; 4]; 3]>>> = Rc::new(RefCell::new(vec![]));
    let sp = splashes.clone();
    r.stub(0x12c7e0, move |c, m| {
        let mut one = [[0u32; 4]; 3];
        for (i, reg) in [6u32, 7, 8].iter().enumerate() {
            for k in 0..4 {
                one[i][k] = m.read_u32(c.gpr(*reg as usize) as u32 + 4 * k as u32);
            }
        }
        sp.borrow_mut().push(one);
    });
    let this = r.mem.alloc(0x990);
    let rider = r.mem.alloc(0x5c00);
    let cols = r.mem.alloc(0x500);
    let (pa, fa, da) = (r.mem.alloc(0x10), r.mem.alloc(0x10), r.mem.alloc(0x10));
    let mut g = Rng(0x6b5a_1234_dead_beef);
    let mut pool = Pool::default();
    let mut spawned = 0;
    for case in 0..30_000 {
        // keep the model and the game's pool together; now and then jump the counters (the pool full, the head wrapped)
        if case % 7 == 0 {
            pool.count = (g.next() % 31) as i32;
            pool.head = (g.next() % 30) as i32;
        }
        if case % 5 == 0 {
            pool.recent = [0.0, 0.5, 0.69, 0.7, 0.71, 3.0][(g.next() % 6) as usize];
        }
        if case % 9 == 0 {
            pool.level = [0.0, 1.0, 1.5, 1.6, 2.0][(g.next() % 5) as usize];
        }
        for (i, e) in pool.entries.iter().enumerate() {
            for (k, w) in e.iter().enumerate() {
                r.mem.write_u32(this + 0x50 * i as u32 + 4 * k as u32, *w);
            }
        }
        r.mem.write_u32(this + 0x960, pool.count as u32);
        r.mem.write_u32(this + 0x964, pool.head as u32);
        r.mem.write_f32(this + 0x968, pool.recent);
        r.mem.write_f32(this + 0x96c, pool.level);
        let colour = if g.next() % 4 == 0 { [0.0, g.f(), g.f(), g.f()] } else { [0.0, (g.next() % 100) as f32 / 99.0, (g.next() % 100) as f32 / 99.0, (g.next() % 100) as f32 / 99.0] };
        let (pos, from, dir) = (g.v(), g.v(), g.v());
        let words: VecDeque<u32> = (0..4).map(|_| g.next() | (g.next() << 16)).collect();
        let mode = [0u32, 1, 2, 3][(g.next() % 4) as usize];
        put(&mut r, cols + 0x470, colour);
        r.mem.write_u32(rider + 0x2e0, mode);
        r.mem.write_u32(rider + 0x5ae0, cols);
        put(&mut r, pa, pos);
        put(&mut r, fa, from);
        put(&mut r, da, dir);
        *rand.borrow_mut() = words.clone();
        splashes.borrow_mut().clear();
        let mut env = Env { rand: words, splashes: vec![] };
        if case % 2 == 0 {
            // SpawnParticle direct
            let size = g.f();
            r.cpu.set_gpr(4, this as u64);
            r.cpu.set_f(12, size);
            r.cpu.set_gpr(5, (cols + 0x470) as u64);
            r.cpu.set_gpr(6, pa as u64);
            r.cpu.set_gpr(7, da as u64);
            r.cpu.set_gpr(8, fa as u64);
            r.call(0x12e330).unwrap();
            spawn(&mut pool, size, colour, pos, dir, from, &mut env);
        } else {
            r.cpu.set_gpr(4, this as u64);
            r.cpu.set_gpr(5, rider as u64);
            r.cpu.set_gpr(6, pa as u64);
            r.cpu.set_gpr(7, fa as u64);
            r.cpu.set_gpr(8, da as u64);
            r.call(0x12e008).unwrap();
            emit(&mut pool, mode, colour, pos, from, dir, &mut env);
            // the stub gets this+0x970 and the three vectors as a2, a3, t0
            assert_eq!(*splashes.borrow(), env.splashes, "case {case} splash");
        }
        assert!(rand.borrow().len() == env.rand.len(), "case {case} rand use");
        spawned += 1;
        for i in 0..30u32 {
            for k in 0..20u32 {
                let got = r.mem.read_u32(this + 0x50 * i + 4 * k);
                assert_eq!(got, pool.entries[i as usize][k as usize], "case {case} entry {i} word {k}");
            }
        }
        assert_eq!(r.mem.read_u32(this + 0x960) as i32, pool.count, "case {case} count");
        assert_eq!(r.mem.read_u32(this + 0x964) as i32, pool.head, "case {case} head");
        assert_eq!(r.mem.read_u32(this + 0x968), pool.recent.to_bits(), "case {case} recent");
        assert_eq!(r.mem.read_u32(this + 0x96c), pool.level.to_bits(), "case {case} level");
    }
    assert!(spawned > 0);
}

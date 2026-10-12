//! The rigid bodies against the original's (board row E8c): `RigidBody_SetPosition` 0x153808,
//! `RigidBody_UpdateVelocity` 0x153ad8, `RigidBody_Integrate` 0x153c50, `RigidBody_AddImpulse` 0x154338,
//! `RigidBody_ContactImpulse` 0x153ee8, `CollBody_Transform` 0x236498 and `CollBody_PointMassInertia` 0x236530 run in
//! the function runner on SLUS_203.26 (TRICKY_ELF; skipped without it) against `tricky_game::rigidbody`, bit for
//! bit.

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_game::collbody::CollBody;
use tricky_game::rigidbody::{point_mass_inertia, transform, RigidBody};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    fn v(&mut self, k: f32) -> [f32; 4] {
        let w = match self.next() % 4 {
            0 => self.f(-1.0, 1.0),
            1 => 1.0,
            _ => 0.0,
        };
        [self.f(-k, k), self.f(-k, k), self.f(-k, k), w]
    }
    fn body(&mut self) -> RigidBody {
        let m = self.f(0.5, 200.0);
        let s = self.f(0.0001, 0.05);
        let mut ii = [0.0; 9];
        for (k, x) in ii.iter_mut().enumerate() {
            *x = if k % 4 == 0 { s * self.f(0.5, 2.0) } else if self.next() % 2 == 0 { s * self.f(-0.3, 0.3) } else { 0.0 };
        }
        let mut q = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
        let n = q.iter().map(|x| x * x).sum::<f32>().sqrt();
        if self.next() % 3 != 0 {
            q = q.map(|x| x / n);
        }
        let mut b = RigidBody { inv_mass: 1.0 / m, inv_inertia: ii, pos: self.v(5000.0), q, p: self.v(3000.0 * m), l: self.v(500.0), v: self.v(3000.0), w: self.v(30.0) };
        if self.next() % 8 == 0 {
            b.inv_mass = 0.0;
        }
        b
    }
}

fn put4(m: &mut Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}

fn put_rb(m: &mut Mem, at: u32, b: &RigidBody) {
    m.write_f32(at + 8, b.inv_mass);
    for k in 0..9 {
        m.write_f32(at + 0xc + 4 * k as u32, b.inv_inertia[k]);
    }
    for (o, v) in [(0x30, b.pos), (0x40, b.q), (0x50, b.p), (0x60, b.l), (0x70, b.v), (0x80, b.w)] {
        put4(m, at + o, v);
    }
}
fn get_rb(m: &mut Mem, at: u32) -> RigidBody {
    let mut b = RigidBody { inv_mass: m.read_f32(at + 8), ..Default::default() };
    for k in 0..9 {
        b.inv_inertia[k] = m.read_f32(at + 0xc + 4 * k as u32);
    }
    b.pos = get4(m, at + 0x30);
    b.q = get4(m, at + 0x40);
    b.p = get4(m, at + 0x50);
    b.l = get4(m, at + 0x60);
    b.v = get4(m, at + 0x70);
    b.w = get4(m, at + 0x80);
    b
}

fn bits(b: &RigidBody) -> Vec<u32> {
    let mut v = vec![b.inv_mass.to_bits()];
    v.extend(b.inv_inertia.iter().map(|x| x.to_bits()));
    for a in [b.pos, b.q, b.p, b.l, b.v, b.w] {
        v.extend(a.iter().map(|x| x.to_bits()));
    }
    v
}

fn put_cb(m: &mut Mem, at: u32, b: &CollBody) {
    put4(m, at + 0x10, b.center);
    m.write_f32(at + 0x20, b.radius);
    m.write_u32(at + 0x24, b.x24 as u32);
    m.write_u32(at + 0x28, b.count as u32);
    for (k, (c, r)) in b.spheres.iter().enumerate() {
        put4(m, at + 0x30 + 0x20 * k as u32, *c);
        m.write_f32(at + 0x40 + 0x20 * k as u32, *r);
    }
}
fn get_cb(m: &mut Mem, at: u32) -> CollBody {
    let mut b = CollBody { center: get4(m, at + 0x10), radius: m.read_f32(at + 0x20), x24: m.read_u32(at + 0x24) as i32, count: m.read_u32(at + 0x28) as i32, spheres: [([0.0; 4], 0.0); 18] };
    for k in 0..18u32 {
        b.spheres[k as usize] = (get4(m, at + 0x30 + 0x20 * k), m.read_f32(at + 0x40 + 0x20 * k));
    }
    b
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn rigid_bodies_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x102f50, 0x2cc808, 0x153808, 0x153ad8, 0x153c50, 0x154338, 0x153ee8, 0x236498, 0x236530] {
        r.allow(a);
    }
    let rb = r.mem.alloc(0x100);
    let a1 = r.mem.alloc(0x40);
    let a2 = r.mem.alloc(0x40);
    let a3 = r.mem.alloc(0x40);
    let cb = r.mem.alloc(0x30 + 0x20 * 18);
    let mut g = Rng(0x6b0d_7e57_0000_0001);
    let mut n = [0usize; 7];
    for case in 0..20000 {
        let b0 = g.body();
        put_rb(&mut r.mem, rb, &b0);
        let what = case % 7;
        let mut b = b0;
        match what {
            0 => {
                let w = g.v(30.0);
                put4(&mut r.mem, a1, w);
                r.cpu.set_gpr(4, rb as u64);
                r.cpu.set_gpr(5, a1 as u64);
                r.call(0x153808).unwrap();
                b.set_spin(w);
            }
            1 => {
                if g.next() % 2 == 0 {
                    b.l = g.v(5000.0);
                    put_rb(&mut r.mem, rb, &b);
                }
                r.cpu.set_gpr(4, rb as u64);
                r.call(0x153ad8).unwrap();
                b.update_velocity();
            }
            2 => {
                let dt = [1.0f32 / 60.0, 1.0 / 30.0, 0.0, 0.5][g.next() as usize % 4];
                let (f, t) = (g.v(100000.0), g.v(1000.0));
                if g.next() % 6 == 0 {
                    b.q = [0.0; 4];
                    b.w = [0.0; 4];
                    put_rb(&mut r.mem, rb, &b);
                }
                put4(&mut r.mem, a1, f);
                put4(&mut r.mem, a2, t);
                r.cpu.set_f(12, dt);
                r.cpu.set_gpr(4, rb as u64);
                r.cpu.set_gpr(5, a1 as u64);
                r.cpu.set_gpr(6, a2 as u64);
                r.call(0x153c50).unwrap();
                b.integrate(dt, f, t);
            }
            3 => {
                let (rr, j) = (g.v(100.0), g.v(50000.0));
                put4(&mut r.mem, a1, rr);
                put4(&mut r.mem, a2, j);
                r.cpu.set_gpr(4, rb as u64);
                r.cpu.set_gpr(5, a1 as u64);
                r.cpu.set_gpr(6, a2 as u64);
                r.call(0x154338).unwrap();
                b.add_impulse(rr, j);
            }
            4 => {
                let e = [0.0f32, 0.15, 0.5, 0.8][g.next() as usize % 4];
                let mu = [0.0f32, 0.0, 0.15, 0.6][g.next() as usize % 4];
                let p = { let d = g.v(100.0); [b.pos[0] + d[0], b.pos[1] + d[1], b.pos[2] + d[2], g.v(1.0)[3]] };
                let nn = { let d = g.v(1.0); let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3); [d[0] / l, d[1] / l, d[2] / l, if g.next() % 4 == 0 { 1.0 } else { 0.0 }] };
                if g.next() % 8 == 0 {
                    // sliding straight into the surface: no sliding direction
                    b.w = [0.0; 4];
                    b.v = [nn[0] * -300.0, nn[1] * -300.0, nn[2] * -300.0, 0.0];
                    put_rb(&mut r.mem, rb, &b);
                }
                put4(&mut r.mem, a1, p);
                put4(&mut r.mem, a2, nn);
                r.cpu.set_f(12, e);
                r.cpu.set_f(13, mu);
                r.cpu.set_gpr(4, rb as u64);
                r.cpu.set_gpr(5, a1 as u64);
                r.cpu.set_gpr(6, a2 as u64);
                r.call(0x153ee8).unwrap();
                b.contact_impulse(e, mu, p, nn);
            }
            _ => {
                let mut c = CollBody::new();
                c.count = (g.next() % 19) as i32;
                c.center = g.v(500.0);
                c.radius = g.f(10.0, 200.0);
                for s in c.spheres.iter_mut() {
                    *s = (g.v(200.0), g.f(5.0, 40.0));
                }
                put_cb(&mut r.mem, cb, &c);
                if what == 5 {
                    let m = [g.v(1.0), g.v(1.0), g.v(1.0), g.v(3000.0)];
                    for (k, col) in m.iter().enumerate() {
                        put4(&mut r.mem, a1 + 0x10 * k as u32, *col);
                    }
                    r.cpu.set_gpr(4, cb as u64);
                    r.cpu.set_gpr(5, a1 as u64);
                    r.call(0x236498).unwrap();
                    transform(&mut c, &m);
                    let theirs = get_cb(&mut r.mem, cb);
                    assert_eq!(format!("{theirs:?}"), format!("{c:?}"), "case {case}: transform");
                    let all = |b: &CollBody| { let mut v = b.center.to_vec(); for s in b.spheres.iter() { v.extend(s.0); v.push(s.1); } v };
                    assert!(same(&all(&theirs), &all(&c)), "case {case}: transform bits");
                } else {
                    if c.count == 0 {
                        c.count = 1;
                        put_cb(&mut r.mem, cb, &c);
                    }
                    r.cpu.set_gpr(4, cb as u64);
                    r.cpu.set_gpr(5, a1 as u64);
                    r.cpu.set_gpr(6, a2 as u64);
                    r.cpu.set_gpr(7, a3 as u64);
                    r.call(0x236530).unwrap();
                    let (mass, com, ii) = point_mass_inertia(&c);
                    let mut theirs = vec![r.mem.read_f32(a1)];
                    theirs.extend(get4(&mut r.mem, a2));
                    theirs.extend((0..9).map(|k| r.mem.read_f32(a3 + 4 * k)));
                    let mut ours = vec![mass];
                    ours.extend(com);
                    ours.extend(ii);
                    assert!(same(&theirs, &ours), "case {case}: inertia\n theirs {theirs:?}\n ours   {ours:?}");
                }
                n[what] += 1;
                continue;
            }
        }
        let theirs = get_rb(&mut r.mem, rb);
        assert_eq!(bits(&theirs), bits(&b), "case {case} ({what}):\n before {b0:?}\n theirs {theirs:?}\n ours   {b:?}");
        n[what] += 1;
    }
    eprintln!("cases by function: {n:?}");
}

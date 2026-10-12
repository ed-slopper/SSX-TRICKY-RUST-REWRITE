//! `tricky_game::camera::collide_world` (board row E11c) against `Camera_CollideWorld` 0x16fe60 in the function
//! runner (TRICKY_ELF; skipped without it), the world line/ray/block calls and the push logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::camera::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            5 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn q(&mut self) -> [f32; 4] {
        [self.f(), self.f(), self.f(), self.f()]
    }
}

fn bits(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:08x}", x.to_bits())).collect::<Vec<_>>().join(",")
}

#[derive(Clone, Default)]
struct Script {
    ray: (f32, [f32; 4], [f32; 4]),
    blk: [(bool, Option<([f32; 4], [f32; 4])>); 2],
    push: Option<[f32; 3]>,
    nblk: usize,
    ctors: Vec<String>,
    log: Vec<String>,
}

struct Env(Rc<RefCell<Script>>);
impl WorldEnv for Env {
    fn line(&mut self, a: [f32; 4], b: [f32; 4], mode: i32) -> u32 {
        let mut s = self.0.borrow_mut();
        let t = s.ctors.len() as u32 + 1;
        s.ctors.push(format!("{} {} {mode} {}", bits(&a), bits(&b), bits(&[0.0])));
        let m = format!("ctor {t} {}", s.ctors[t as usize - 1]);
        s.log.push(m);
        t
    }
    fn ray_cast(&mut self, l: Line, res: &mut RayRes) -> f32 {
        let mut s = self.0.borrow_mut();
        let m = format!("ray {} {}", l.main, l.holes);
        s.log.push(m);
        res.hit = s.ray.1;
        res.normal = s.ray.2;
        s.ray.0
    }
    fn is_blocked(&mut self, l: Line, res: &mut RayRes) -> bool {
        let mut s = self.0.borrow_mut();
        let m = format!("blk {} {}", l.main, l.holes);
        s.log.push(m);
        let (b, m) = s.blk[s.nblk];
        s.nblk += 1;
        if let Some((h, n)) = m {
            res.hit = h;
            res.normal = n;
        }
        b
    }
    fn push(&mut self, k: f32, pos: &mut [f32; 3], t: [f32; 3]) {
        let mut s = self.0.borrow_mut();
        let m = format!("push {} {} {}", bits(&[k]), bits(pos), bits(&t));
        s.log.push(m);
        if let Some(p) = s.push {
            *pos = p;
        }
    }
}

#[test]
fn collide_world_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    let st = Rc::new(RefCell::new(Script::default()));
    let rd = |m: &mut r5900::Mem, a: u32, n: usize| -> Vec<f32> { (0..n).map(|k| m.read_f32(a + 4 * k as u32)).collect() };
    let s = st.clone();
    r.stub(0x25d7f0, move |c, m| {
        let a1 = rd(m, c.gpr(5) as u32, 4);
        let a2 = rd(m, c.gpr(6) as u32, 4);
        let mut s = s.borrow_mut();
        let t = s.ctors.len() as u32 + 1;
        let line = format!("{} {} {} {}", bits(&a1), bits(&a2), c.gpr(7) as i32, bits(&[c.f(12)]));
        s.ctors.push(line.clone());
        s.log.push(format!("ctor {t} {line}"));
        for k in 0..37u32 {
            m.write_u32(c.gpr(4) as u32 + 4 * k, t);
        }
    });
    let s = st.clone();
    r.stub(0x25aff8, move |c, m| {
        let l = c.gpr(5) as u32;
        let (w0, w8, w40) = (m.read_u32(l), m.read_u32(l + 8), m.read_u32(l + 0x40));
        assert_eq!(w8, w40);
        let mut s = s.borrow_mut();
        s.log.push(format!("ray {w0} {w8}"));
        let res = c.gpr(6) as u32;
        for k in 0..4u32 {
            m.write_f32(res + 0x10 + 4 * k, s.ray.1[k as usize]);
            m.write_f32(res + 0x20 + 4 * k, s.ray.2[k as usize]);
        }
        c.set_f(0, s.ray.0);
    });
    let s = st.clone();
    r.stub(0x25c690, move |c, m| {
        let l = c.gpr(5) as u32;
        let (w0, w8, w40) = (m.read_u32(l), m.read_u32(l + 8), m.read_u32(l + 0x40));
        assert_eq!(w8, w40);
        let mut s = s.borrow_mut();
        s.log.push(format!("blk {w0} {w8}"));
        let (b, mm) = s.blk[s.nblk];
        s.nblk += 1;
        let res = c.gpr(6) as u32;
        if let Some((h, n)) = mm {
            for k in 0..4u32 {
                m.write_f32(res + 0x10 + 4 * k, h[k as usize]);
                m.write_f32(res + 0x20 + 4 * k, n[k as usize]);
            }
        }
        c.set_gpr(2, b as u64);
    });
    let s = st.clone();
    r.stub(0x16fb08, move |c, m| {
        let pa = c.gpr(4) as u32;
        let pos = rd(m, pa, 3);
        let t = rd(m, c.gpr(5) as u32, 3);
        let mut s = s.borrow_mut();
        s.log.push(format!("push {} {} {}", bits(&[c.f(12)]), bits(&pos), bits(&t)));
        if let Some(p) = s.push {
            for k in 0..3 {
                m.write_f32(pa + 4 * k as u32, p[k]);
            }
        }
    });
    let posa = r.mem.alloc(16);
    let tga = r.mem.alloc(16);
    let mut g = Rng(0xfeed_beef_1234_5671);
    for case in 0..60_000 {
        let mut pos = [g.f(), g.f(), g.f()];
        let mut tgt = [g.f(), g.f(), g.f()];
        if g.next() % 8 == 0 {
            tgt = [0.0, if g.next() % 2 == 0 { -0.0 } else { 0.0 }, 0.0];
        }
        let radius = [0.0f32, 100.0, 499.0, 500.0, 500.5, 800.0, -3.0][(g.next() % 7) as usize];
        let mut sc = Script::default();
        sc.ray = ([-1.0f32, 0.0, 0.5, 3.0][(g.next() % 4) as usize], g.q(), g.q());
        if g.next() % 3 != 0 {
            sc.ray.2[2] = sc.ray.2[2].abs() + 0.1;
        }
        for k in 0..2 {
            sc.blk[k] = (g.next() % 3 != 0, if g.next() % 2 == 0 { Some((g.q(), g.q())) } else { None });
        }
        if g.next() % 2 == 0 {
            sc.push = Some([g.f(), g.f(), g.f()]);
        }
        *st.borrow_mut() = sc.clone();
        let before = r.mem.read_u32(0x335720);
        for k in 0..3 {
            r.mem.write_f32(posa + 4 * k, pos[k as usize]);
            r.mem.write_f32(tga + 4 * k, tgt[k as usize]);
        }
        r.cpu.set_gpr(4, posa as u64);
        r.cpu.set_gpr(5, tga as u64);
        r.cpu.set_f(12, radius);
        r.call(0x16fe60).unwrap();
        let ret = r.cpu.gpr(2) as u32;
        let want_pos = rd(&mut r.mem, posa, 3);
        let want_log = st.borrow().log.clone();
        let after = r.mem.read_u32(0x335720);

        let es = Rc::new(RefCell::new(sc));
        let mut counter = before;
        let got = collide_world(radius, &mut pos, tgt, &mut counter, &mut Env(es.clone()));
        let got_log = es.borrow().log.clone();
        assert_eq!(got_log, want_log, "case {case} log");
        assert_eq!(got, ret, "case {case} ret");
        assert_eq!(bits(&pos), bits(&want_pos), "case {case} pos");
        assert_eq!(counter, after, "case {case} counter");
    }
}

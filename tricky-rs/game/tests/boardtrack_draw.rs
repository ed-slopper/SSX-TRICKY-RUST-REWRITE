//! `tricky_game::boardtrack::board_track_draw` against `Fx_BoardTrack_Draw` 0x1331a8 (with the strip builder 0x1333c0
//! and the vector scale 0x102f50 as they are) in the function runner (TRICKY_ELF; skipped without it): the
//! renderer's virtual calls logged (their arguments and the vertices of each strip).

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::boardtrack::*;

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
    /// Shades that land near 0 and the ±1 steps after the scaling by 128 or -64.
    fn shade(&mut self) -> f32 {
        match self.next() % 5 {
            0 => 0.0,
            1 => -((self.next() % 100) as f32) / 1000.0,
            _ => (self.next() % 3000) as f32 / 1000.0 - 0.5,
        }
    }
}

const OFFS: [(u32, [bool; 3]); 10] = [
    (0x1c0, [true, false, false]),
    (0x1c8, [true, true, true]),
    (0x1d0, [true, false, false]),
    (0x1d8, [false, false, false]),
    (0x1e0, [true, false, false]),
    (0x1e8, [true, true, false]),
    (0x1f0, [true, true, false]),
    (0x1f8, [true, true, false]),
    (0xf8, [false, false, false]),
    (0x220, [false, false, false]),
];

struct W {
    log: Vec<Vec<u32>>,
    oracle: Vec<u32>,
    pos: usize,
    view: u32,
    origin: [f32; 4],
}
struct U(Rc<RefCell<W>>);
impl DrawEnv for U {
    fn vcall(&mut self, off: u32, a1: Option<u32>, a2: Option<u32>, f12: Option<f32>) -> u32 {
        let mut w = self.0.borrow_mut();
        let mut v = vec![off];
        v.extend(a1);
        v.extend(a2);
        v.extend(f12.map(|f| f.to_bits()));
        w.log.push(v);
        if off == 0x1d8 {
            let r = w.oracle[w.pos];
            w.pos += 1;
            return r;
        }
        0
    }
    fn view_word(&mut self) -> u32 {
        self.0.borrow().view
    }
    fn origin_quad(&mut self) -> [f32; 4] {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![0xf8]);
        w.origin
    }
    fn draw_strip(&mut self, verts: &[Vertex]) {
        let mut v = vec![0x220, verts.len() as u32];
        for x in verts {
            v.extend([x.u.to_bits(), x.v.to_bits(), x.one.to_bits(), x.rgb[0], x.rgb[1], x.rgb[2], x.w]);
            v.extend(x.pos.map(|f| f.to_bits()));
        }
        self.0.borrow_mut().log.push(v);
    }
}

#[test]
fn board_track_draw_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x1331a8u32, 0x1333c0, 0x102f50] {
        r.allow(a);
    }
    let trail = r.mem.alloc(0x1a00);
    let app = r.mem.alloc(0x800);
    let rend = r.mem.alloc(0x100);
    let vt = r.mem.alloc(0x300);
    let viewobj = r.mem.alloc(0x100);
    let originp = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x724, rend);
    r.mem.write_u32(app + 0x30, viewobj);
    r.mem.write_u32(rend + 0x10, vt);
    let w = Rc::new(RefCell::new(W { log: vec![], oracle: vec![], pos: 0, view: 0, origin: [0.0; 4] }));
    let mut adj = std::collections::HashMap::new();
    for (k, (off, args)) in OFFS.iter().enumerate() {
        let a = 0x10 * (k as u32 % 5);
        adj.insert(*off, a);
        r.mem.write_u32(vt + off, a); // the halfword (little end) and the next halfword zero
        let fa = 0x90_0000 + off;
        r.mem.write_u32(vt + off + 4, fa);
        let (l, args, off, a) = (w.clone(), *args, *off, a);
        r.stub(fa, move |c, m| {
            assert_eq!(c.gpr(4) as u32, rend + a, "this for {off:#x}");
            let mut w = l.borrow_mut();
            if off == 0x220 {
                assert_eq!(c.gpr(7), 0);
                let (n, p) = (c.gpr(5) as u32, c.gpr(6) as u32);
                let mut v = vec![0x220, n];
                for i in 0..n {
                    let b = p + 0x30 * i;
                    v.extend([m.read_u32(b), m.read_u32(b + 4), m.read_u32(b + 8), m.read_u32(b + 0x10), m.read_u32(b + 0x14), m.read_u32(b + 0x18), m.read_u32(b + 0x1c)]);
                    v.extend((0..4).map(|k| m.read_u32(b + 0x20 + 4 * k)));
                }
                w.log.push(v);
                return;
            }
            let mut v = vec![off];
            if args[0] {
                v.push(c.gpr(5) as u32);
            }
            if args[1] {
                v.push(c.gpr(6) as u32);
            }
            if args[2] {
                v.push(c.f(12).to_bits());
            }
            if off == 0xf8 {
                w.log.push(v);
                c.set_gpr(2, originp as u64);
                return;
            }
            w.log.push(v);
            if off == 0x1d8 {
                let x = w.oracle[w.pos];
                w.pos += 1;
                c.set_gpr(2, x as u64);
            }
        });
    }

    let mut g = Rng(0x1331_a8a1_c4d3_9b27);
    let (mut strips, mut draws, mut flips, mut idle) = (0u32, 0u32, 0u32, 0u32);
    for case in 0..20_000 {
        let mut words: Vec<u32> = (0..0x1984 / 4).map(|_| g.next()).collect();
        let count = match g.next() % 8 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 100,
            _ => 2 + g.next() % 40,
        };
        words[0x1944 / 4] = count;
        words[0x1940 / 4] = g.next() % 100;
        words[0x1950 / 4] = g.next() % 2;
        for off in [0x2c, 0x30, 0x3c] {
            words[off / 4] = g.shade().to_bits();
        }
        for k in 0..8 {
            words[k] = g.f().to_bits();
            words[0x10 / 4 + k % 4] = g.f().to_bits();
        }
        for e in 0..100usize {
            let b = (0x40 + 0x40 * e) / 4;
            for k in 0..8 {
                words[b + k] = g.f().to_bits();
            }
            words[b + 0x2c / 4] = g.shade().to_bits();
            words[b + 0x30 / 4] = g.shade().to_bits();
            words[b + 0x3c / 4] = g.shade().to_bits();
            words[b + 0x38 / 4] = [0u32, 0, 0, 0, 1][g.next() as usize % 5];
        }
        let t = Trail { w: words };
        for (i, wd) in t.w.iter().enumerate() {
            r.mem.write_u32(trail + 4 * i as u32, *wd);
        }
        let view = g.next();
        r.mem.write_u32(viewobj + 0x5c, view);
        let origin: [f32; 4] = [g.f() * 0.1, g.f() * 0.1, g.f() * 0.1, 0.1];
        for k in 0..4 {
            r.mem.write_f32(originp + 0x20 + 4 * k, origin[k as usize]);
        }
        {
            let mut wb = w.borrow_mut();
            wb.log.clear();
            wb.oracle = vec![g.next()];
            wb.pos = 0;
            wb.view = view;
            wb.origin = origin;
        }
        r.cpu.set_gpr(4, trail as u64);
        r.call(0x1331a8).unwrap();
        let want = std::mem::take(&mut w.borrow_mut().log);
        w.borrow_mut().pos = 0;
        let mut e = U(w.clone());
        board_track_draw(&t, &mut e);
        let got = std::mem::take(&mut w.borrow_mut().log);
        assert_eq!(want, got, "case {case} (count {count})");
        if want.is_empty() {
            idle += 1;
        }
        strips += want.iter().filter(|v| v[0] == 0xf8).count() as u32;
        draws += want.iter().filter(|v| v[0] == 0x220).count() as u32;
        let _ = &mut flips;
    }
    eprintln!("draw: {strips} strips, {draws} draws, {idle} idle");
}

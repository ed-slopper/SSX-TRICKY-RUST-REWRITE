//! `cWorldLine` and `World_RayCastMeshTris` (board row E7a) against the game in the function runner (TRICKY_ELF;
//! skipped without it): lines built by the game's constructor, put through instance frames and back, tested against
//! random triangles (one-sided and either side), boxes and meshes, every output compared bit for bit. The vector
//! unit's point-in-triangle program cannot run in the runner: `Tri_ContainsPointVu0` is answered by
//! `tri_contains_point` and its arguments are checked.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_game::worldline::{tri_contains_point, Hit, WorldLine};

type V4 = [f32; 4];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
    fn f(&mut self) -> f32 {
        let n = self.next();
        match n % 5 {
            0 => self.pick(&[0.0, 1.0, -1.0, 100.0, -250.0, 0.5]),
            _ => (n >> 8) as f32 / 65536.0 * 400.0 - 200.0 + if n % 7 == 0 { 0.0 } else { (n & 0xff) as f32 / 256.0 },
        }
    }
    fn v(&mut self, w: f32) -> V4 {
        [self.f(), self.f(), self.f(), w]
    }
}

fn wv(m: &mut Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

fn rv(m: &mut Mem, a: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k))
}

fn b(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn line_words(m: &mut Mem, at: u32) -> Vec<u32> {
    (0..0x94 / 4).map(|k| m.read_u32(at + 4 * k)).collect()
}

fn ours_words(l: &WorldLine, vtable: u32, rest: &[u32]) -> Vec<u32> {
    let mut w = rest.to_vec();
    w[0] = l.kind;
    w[1] = l.local;
    for (o, v) in [(4, l.hi), (8, l.lo), (12, l.mid), (20, l.start), (24, l.dir), (28, l.world_start), (32, l.world_dir)] {
        w[o..o + 4].copy_from_slice(&b(v));
    }
    w[16] = vtable;
    w[36] = l.target.to_bits();
    w
}

#[test]
fn world_line_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x25d9e0, 0x25db18, 0x25dc48, 0x25ebd8, 0x25d7f0, 0x25d948, 0x25d9c8, 0x25d3d8] {
        r.allow(a);
    }
    r.allow(0x102f50); // Vec4_Scale
    let tri_calls: Rc<RefCell<Vec<[[u32; 4]; 4]>>> = Rc::new(RefCell::new(vec![]));
    let tc = tri_calls.clone();
    r.stub(0x23b6b0, move |c, m| {
        let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
        let (a, bb, cc, p) = (g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
        tc.borrow_mut().push([b(a), b(bb), b(cc), b(p)]);
        c.set_gpr(2, tri_contains_point(a, bb, cc, p) as u64);
    });
    let line = r.mem.alloc(0xa0);
    let from = r.mem.alloc(16);
    let to = r.mem.alloc(16);
    let mat = r.mem.alloc(64);
    let pa = r.mem.alloc(16);
    let pb = r.mem.alloc(16);
    let pc = r.mem.alloc(16);
    let pn = r.mem.alloc(16);
    let out_p = r.mem.alloc(16);
    let out_n = r.mem.alloc(16);
    let out_t = r.mem.alloc(16);
    let mesh = r.mem.alloc(0x14);
    let tris = r.mem.alloc(12 * 40);
    let verts = r.mem.alloc(16 * 60);
    let planes = r.mem.alloc(16 * 40);
    let mut g = Rng(0x7071_e000_2525_0001);
    let (mut hits, mut boxes, mut meshes) = (0, 0, 0);
    for case in 0..20000 {
        // a line from the game's constructor
        let f = g.v(1.0);
        let t = if g.next() % 4 == 0 { [f[0], f[1], f[2] - g.pick(&[50.0f32, 300.0]), 1.0] } else {
            let w = g.pick(&[1.0, 1.0, 0.0]);
            g.v(w)
        };
        let target = g.pick(&[0.5f32, 0.0, 1.0, -1.0, 0.25]);
        let kind = g.pick(&[2u32, 0, 1, 7]);
        wv(&mut r.mem, from, f);
        wv(&mut r.mem, to, t);
        for o in (0..0xa0).step_by(4) {
            r.mem.write_u32(line + o, 0);
        }
        r.cpu.set_f(12, target);
        r.cpu.set_gpr(4, line as u64);
        r.cpu.set_gpr(5, from as u64);
        r.cpu.set_gpr(6, to as u64);
        r.cpu.set_gpr(7, kind as u64);
        r.call(0x25d7f0).unwrap();
        let mut l = WorldLine::new(target, f, t, kind);
        let vtable = r.mem.read_u32(line + 0x40);
        assert_eq!(vtable, 0x3a7aa8);
        let theirs = line_words(&mut r.mem, line);
        assert_eq!(theirs, ours_words(&l, vtable, &theirs), "case {case}: built");
        // sometimes in an instance's frame
        if g.next() % 3 == 0 {
            let (s, c) = g.pick(&[(0.0f32, 1.0f32), (0.6, 0.8), (1.0, 0.0), (-0.28, 0.96)]);
            let m = [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, g.pick(&[1.0f32, 2.0, 0.5]), 0.0], g.v(1.0)];
            for (i, row) in m.iter().enumerate() {
                wv(&mut r.mem, mat + 16 * i as u32, *row);
            }
            r.cpu.set_gpr(4, line as u64);
            r.cpu.set_gpr(5, mat as u64);
            r.call(0x25d948).unwrap();
            l.push(m);
            let theirs = line_words(&mut r.mem, line);
            assert_eq!(theirs, ours_words(&l, vtable, &theirs), "case {case}: pushed");
            if g.next() % 4 == 0 {
                r.cpu.set_gpr(4, line as u64);
                r.call(0x25d9c8).unwrap();
                l.pop();
                let theirs = line_words(&mut r.mem, line);
                assert_eq!(theirs, ours_words(&l, vtable, &theirs), "case {case}: popped");
            }
        }
        // a triangle, often across the line
        let aw = g.pick(&[1.0, 0.0, 3.5]);
        let a = g.v(aw);
        let (bb, cc) = if g.next() % 2 == 0 {
            let p = [l.start[0] + l.dir[0] * 0.4, l.start[1] + l.dir[1] * 0.4, l.start[2] + l.dir[2] * 0.4, 1.0];
            ([p[0] * 2.0 - a[0] + 20.0, p[1] * 2.0 - a[1], p[2] * 2.0 - a[2], a[3]], [p[0] * 2.0 - a[0] - 20.0, p[1] * 2.0 - a[1] + 30.0, p[2] * 2.0 - a[2] + 5.0, a[3]])
        } else {
            (g.v(a[3]), g.v(a[3]))
        };
        let e1 = [bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]];
        let e2 = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0], g.pick(&[0.0f32, -12.5])];
        let n = if g.next() % 2 == 0 { n } else { [n[0] * 0.001, n[1] * 0.001, n[2] * 0.001, n[3]] };
        for (at, v) in [(pa, a), (pb, bb), (pc, cc), (pn, n)] {
            wv(&mut r.mem, at, v);
        }
        for either in [false, true] {
            for at in [out_p, out_n, out_t] {
                wv(&mut r.mem, at, [-7.0; 4]);
            }
            tri_calls.borrow_mut().clear();
            r.cpu.set_gpr(4, line as u64);
            r.cpu.set_gpr(5, pa as u64);
            r.cpu.set_gpr(6, pb as u64);
            r.cpu.set_gpr(7, pc as u64);
            r.cpu.set_gpr(8, pn as u64);
            r.cpu.set_gpr(9, out_p as u64);
            r.cpu.set_gpr(10, out_n as u64);
            r.cpu.set_gpr(11, out_t as u64);
            r.call(if either { 0x25db18 } else { 0x25d9e0 }).unwrap();
            let got = r.cpu.gpr(2) as u32;
            let ours = if either { l.triangle_either_side(a, bb, cc, n) } else { l.triangle(a, bb, cc, n) };
            let what = format!("case {case}: triangle (either side {either})");
            assert_eq!(got != 0, ours.hit, "{what}");
            assert_eq!(r.mem.read_u32(out_t), ours.t.map_or((-7.0f32).to_bits(), f32::to_bits), "{what}: t");
            let (p, nn) = ours.at.unwrap_or(([-7.0; 4], [-7.0; 4]));
            assert_eq!((rv(&mut r.mem, out_p), rv(&mut r.mem, out_n)), (b(p), b(nn)), "{what}: point");
            if ours.at.is_some() {
                assert_eq!(tri_calls.borrow().as_slice(), &[[b(a), b(bb), b(cc), b(p)]], "{what}: the vector unit's call");
                hits += ours.hit as usize;
            }
        }
        // a box, often around part of the line
        let c0 = if g.next() % 2 == 0 { l.start } else { g.v(1.0) };
        let size = [g.pick(&[10.0f32, 80.0, 400.0]), g.pick(&[10.0f32, 80.0, 400.0]), g.pick(&[10.0f32, 80.0, 400.0])];
        let off = g.pick(&[0.0f32, 0.3, 0.9, 1.2]);
        let lo = [c0[0] + l.dir[0] * off - size[0], c0[1] + l.dir[1] * off - size[1], c0[2] + l.dir[2] * off - size[2], 1.0];
        let hi = [lo[0] + 2.0 * size[0], lo[1] + 2.0 * size[1], lo[2] + 2.0 * size[2], 1.0];
        wv(&mut r.mem, pa, lo);
        wv(&mut r.mem, pb, hi);
        for at in [out_p, out_n, out_t] {
            wv(&mut r.mem, at, [-7.0; 4]);
        }
        r.cpu.set_gpr(4, line as u64);
        r.cpu.set_gpr(5, pa as u64);
        r.cpu.set_gpr(6, pb as u64);
        r.cpu.set_gpr(7, out_p as u64);
        r.cpu.set_gpr(8, out_n as u64);
        r.cpu.set_gpr(9, out_t as u64);
        r.call(0x25dc48).unwrap();
        let got = r.cpu.gpr(2) as u32;
        let ours = l.boxed(lo, hi);
        let hb = |h: Option<Hit>| h.map(|h| (b(h.pos), b(h.normal), h.t.to_bits()));
        let theirs = if got != 0 { Some((rv(&mut r.mem, out_p), rv(&mut r.mem, out_n), r.mem.read_u32(out_t))) } else { None };
        assert_eq!(theirs, hb(ours), "case {case}: box");
        boxes += ours.is_some() as usize;
        // a mesh of triangles around the line
        let nt = 1 + g.next() % 40;
        let nv = 3 + g.next() % 57;
        let mut vs = vec![];
        for _ in 0..nv {
            let k = g.pick(&[0.0f32, 0.2, 0.5, 0.8, 1.0]);
            let p = [l.start[0] + l.dir[0] * k + g.pick(&[-40.0f32, 0.0, 25.0, 90.0]), l.start[1] + l.dir[1] * k + g.pick(&[-40.0f32, 0.0, 33.0, -90.0]), l.start[2] + l.dir[2] * k + g.pick(&[-15.0f32, 0.0, 5.0]), g.pick(&[1.0f32, 0.0])];
            vs.push(p);
        }
        let mut ts = vec![];
        let mut ps = vec![];
        for _ in 0..nt {
            let t3 = [g.next() % nv, g.next() % nv, g.next() % nv];
            let (a, bb, cc) = (vs[t3[0] as usize], vs[t3[1] as usize], vs[t3[2] as usize]);
            let e1 = [bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]];
            let e2 = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0], g.pick(&[0.0f32, 3.0])];
            ts.push(t3);
            ps.push(n);
        }
        for (i, t3) in ts.iter().enumerate() {
            for k in 0..3 {
                r.mem.write_u32(tris + 12 * i as u32 + 4 * k as u32, t3[k]);
            }
            wv(&mut r.mem, planes + 16 * i as u32, ps[i]);
        }
        for (i, v) in vs.iter().enumerate() {
            wv(&mut r.mem, verts + 16 * i as u32, *v);
        }
        for (k, w) in [nt, nv, tris, verts, planes].iter().enumerate() {
            r.mem.write_u32(mesh + 4 * k as u32, *w);
        }
        let either = g.next() % 2 == 0;
        for at in [out_p, out_n, out_t] {
            wv(&mut r.mem, at, [-7.0; 4]);
        }
        r.cpu.set_gpr(4, line as u64);
        r.cpu.set_gpr(5, mesh as u64);
        r.cpu.set_gpr(6, either as u64);
        r.cpu.set_gpr(7, out_p as u64);
        r.cpu.set_gpr(8, out_n as u64);
        r.cpu.set_gpr(9, out_t as u64);
        r.call(0x25d3d8).unwrap();
        let got = r.cpu.gpr(2) as u32;
        let ours = l.mesh(&ts, &vs, &ps, either);
        let theirs = if got != 0 { Some((rv(&mut r.mem, out_p), rv(&mut r.mem, out_n), r.mem.read_u32(out_t))) } else { None };
        assert_eq!(theirs, hb(ours), "case {case}: mesh of {nt} ({either})");
        meshes += ours.is_some() as usize;
    }
    eprintln!("{hits} triangle hits, {boxes} box hits, {meshes} mesh hits");
}

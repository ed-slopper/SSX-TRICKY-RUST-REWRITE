//! `cWorldBoarder` (board row E7a3) against the game in the function runner (TRICKY_ELF; skipped without it): shapes
//! made by the game's constructor from random collision bodies (no mask, all, some spheres), their target and
//! reject tests, put in a frame, and tested against triangles placed against their spheres (all three plane rules),
//! boxes and meshes; every output bit for bit. `Tri_ContainsPointVu0` is answered by its port.

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_game::collbody::CollBody;
use tricky_game::world::{CollMesh, Shape};
use tricky_game::worldboarder::WorldBoarder;
use tricky_game::worldline::{tri_contains_point, Hit};

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
    fn f(&mut self, k: f32) -> f32 {
        ((self.next() % 2001) as f32 - 1000.0) / 1000.0 * k
    }
}

fn wv(m: &mut Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

fn rw(m: &mut Mem, a: u32, n: u32) -> Vec<u32> {
    (0..n).map(|k| m.read_u32(a + 4 * k)).collect()
}

fn b4(v: V4) -> Vec<u32> {
    v.map(f32::to_bits).to_vec()
}

fn write_body(m: &mut Mem, at: u32, b: &CollBody) {
    for o in (0..0x280).step_by(4) {
        m.write_u32(at + o, 0);
    }
    wv(m, at + 0x10, b.center);
    m.write_f32(at + 0x20, b.radius);
    m.write_u32(at + 0x24, b.x24 as u32);
    m.write_u32(at + 0x28, b.count as u32);
    for (k, (c, r)) in b.spheres.iter().enumerate() {
        wv(m, at + 0x30 + 0x20 * k as u32, *c);
        m.write_f32(at + 0x40 + 0x20 * k as u32, *r);
    }
}

fn body_words(b: &CollBody) -> Vec<u32> {
    let mut w = vec![];
    w.extend(b4(b.center));
    w.push(b.radius.to_bits());
    w.push(b.x24 as u32);
    w.push(b.count as u32);
    for (c, r) in &b.spheres[..b.count.max(0) as usize] {
        w.extend(b4(*c));
        w.push(r.to_bits());
    }
    w
}

fn read_body_words(m: &mut Mem, at: u32, count: usize) -> Vec<u32> {
    let mut w = rw(m, at + 0x10, 7);
    for k in 0..count as u32 {
        w.extend(rw(m, at + 0x30 + 0x20 * k, 5));
    }
    w
}

#[test]
fn world_boarder_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x257810, 0x235fc8, 0x236830, 0x102f50, 0x2578c0, 0x257918, 0x257978, 0x257988, 0x257a50, 0x257ba8, 0x257c50, 0x236130, 0x236498, 0x236bb8, 0x237480, 0x237d50, 0x238640, 0x23b6e8, 0x25d3d8] {
        r.allow(a);
    }
    r.stub(0x23b6b0, |c, m| {
        let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
        let v = tri_contains_point(g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
        c.set_gpr(2, v as u64);
    });
    r.stub(0x2ce114, |c, m| {
        let mut b = vec![0u8; c.gpr(6) as usize];
        m.read(c.gpr(5) as u32, &mut b);
        m.write(c.gpr(4) as u32, &b).unwrap();
    });
    let body = r.mem.alloc(0x280);
    let shape = r.mem.alloc(0x300);
    let mat = r.mem.alloc(0x40);
    let tri = r.mem.alloc(0x40);
    let outs = r.mem.alloc(0x40);
    let mesh = r.mem.alloc(0x14);
    let tris = r.mem.alloc(12 * 24);
    let verts = r.mem.alloc(16 * 40);
    let planes = r.mem.alloc(16 * 24);
    let mut g = Rng(0xb0a7_de75_0000_0001);
    let mut hits = [0usize; 4];
    for case in 0..12000 {
        // a body: centre, bounding radius, spheres about it
        let center = [g.f(3000.0), g.f(3000.0), g.f(800.0), g.pick(&[1.0f32, 1.0, 0.0])];
        let mut b = CollBody::new();
        b.center = center;
        b.radius = g.pick(&[60.0f32, 120.0, 25.0]);
        b.count = g.pick(&[18, 18, 4, 0]);
        let some = (g.next() & 0x3ffff) as i32;
        b.x24 = g.pick(&[0, -1, 0x5, some, 0]);
        for s in b.spheres.iter_mut().take(b.count as usize) {
            *s = ([center[0] + g.f(70.0), center[1] + g.f(70.0), center[2] + g.f(90.0), center[3]], g.pick(&[10.0f32, 15.0, 20.0, 30.0]));
        }
        write_body(&mut r.mem, body, &b);
        for o in (0..0x300).step_by(4) {
            r.mem.write_u32(shape + o, 0);
        }
        let flag = g.pick(&[0u32, 1]);
        r.cpu.set_gpr(4, shape as u64);
        r.cpu.set_gpr(5, body as u64);
        r.cpu.set_gpr(6, flag as u64);
        r.call(0x257810).unwrap();
        let mut s = WorldBoarder::new(&b, flag);
        let m = &mut r.mem;
        assert_eq!(
            (m.read_u32(shape), rw(m, shape + 0x10, 4), rw(m, shape + 0x20, 4), rw(m, shape + 0x30, 4)),
            (s.kind, b4(s.hi), b4(s.lo), b4(s.mid)),
            "case {case}: built"
        );
        r.cpu.set_gpr(4, shape as u64);
        r.call(0x257c50).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), s.target().to_bits(), "case {case}: target");
        // the reject test
        let rn = [g.f(1.0), g.f(1.0), g.f(1.0), 0.0];
        if g.next() % 2 == 0 {
            let gn = [0.0f32, g.f(0.3), 1.0, 0.0];
            r.mem.write_u32(shape + 0x2d0, 1);
            wv(&mut r.mem, shape + 0x2e0, gn);
            s.reject = Some(gn);
        }
        wv(&mut r.mem, outs, rn);
        r.cpu.set_gpr(4, shape as u64);
        r.cpu.set_gpr(5, outs as u64);
        r.call(0x2578c0).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, s.reject(rn), "case {case}: reject");
        if g.next() % 3 == 0 {
            r.mem.write_u32(shape + 0x2f0, 1);
            s.front_only = true;
        }
        // sometimes in a frame
        if g.next() % 3 == 0 {
            let (sn, cs) = g.pick(&[(0.0f32, 1.0f32), (0.6, 0.8), (1.0, 0.0)]);
            let mm = [[cs, sn, 0.0, 0.0], [-sn, cs, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.f(500.0), g.f(500.0), g.f(100.0), 1.0]];
            for (i, row) in mm.iter().enumerate() {
                wv(&mut r.mem, mat + 16 * i as u32, *row);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, mat as u64);
            r.call(0x257918).unwrap();
            s.push(mm);
            assert_eq!(r.mem.read_u32(shape + 0x50), shape + 0x60, "case {case}: the copy in use");
            assert_eq!(read_body_words(&mut r.mem, shape + 0x60, b.count as usize), body_words(&s.body), "case {case}: put in the frame");
        }
        let cur = s.body;
        // a triangle against a sphere (or the bounding one): plane through a point at distance d along n
        let (sc, sr) = if cur.count > 0 && g.next() % 2 == 0 { cur.spheres[g.next() as usize % cur.count as usize] } else { (cur.center, cur.radius) };
        let n = {
            let v = [g.f(1.0), g.f(1.0), g.pick(&[1.0f32, -1.0, 0.3]), 0.0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            v.map(|x| x / l)
        };
        let d = sr * g.pick(&[0.5f32, -0.5, 0.95, 1.2, -1.2, 0.0]);
        let foot = [sc[0] - n[0] * d, sc[1] - n[1] * d, sc[2] - n[2] * d, sc[3]];
        let size = g.pick(&[200.0f32, 30.0, 8.0]);
        let off = [g.f(size), g.f(size), 0.0];
        // two directions in the plane
        let t1 = if n[2].abs() < 0.9 { [-n[1], n[0], 0.0] } else { [0.0, -n[2], n[1]] };
        let t2 = [n[1] * t1[2] - n[2] * t1[1], n[2] * t1[0] - n[0] * t1[2], n[0] * t1[1] - n[1] * t1[0]];
        let pt = |u: f32, v: f32| [foot[0] + t1[0] * (u + off[0]) + t2[0] * (v + off[1]), foot[1] + t1[1] * (u + off[0]) + t2[1] * (v + off[1]), foot[2] + t1[2] * (u + off[0]) + t2[2] * (v + off[1]), 1.0];
        let (a, bb, c) = (pt(-size, -size), pt(size, -size * 0.5), pt(0.0, size));
        let plane = [n[0], n[1], n[2], g.pick(&[0.0f32, -(n[0] * a[0] + n[1] * a[1] + n[2] * a[2])])];
        for (k, v) in [a, bb, c, plane].iter().enumerate() {
            wv(&mut r.mem, tri + 16 * k as u32, *v);
        }
        for either in [false, true] {
            for o in (0..0x40).step_by(4) {
                r.mem.write_u32(outs + o, 0x7777_7777);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, tri as u64);
            r.cpu.set_gpr(6, (tri + 16) as u64);
            r.cpu.set_gpr(7, (tri + 32) as u64);
            r.cpu.set_gpr(8, (tri + 48) as u64);
            r.cpu.set_gpr(9, outs as u64);
            r.cpu.set_gpr(10, (outs + 16) as u64);
            r.cpu.set_gpr(11, (outs + 32) as u64);
            r.call(if either { 0x257a50 } else { 0x257988 }).unwrap();
            let got = r.cpu.gpr(2) != 0;
            let ours = if either { s.triangle_either_side(a, bb, c, plane) } else { s.triangle(a, bb, c, plane) };
            let theirs = got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)));
            let hb = |h: Option<Hit>| h.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits()));
            assert_eq!(theirs, hb(ours), "case {case}: triangle (either side {either}, front {})", s.front_only);
            hits[either as usize] += ours.is_some() as usize;
        }
        // a box about a sphere
        let bc = if g.next() % 2 == 0 { cur.center } else { sc };
        let half = [g.pick(&[50.0f32, 200.0, 10.0]), g.pick(&[50.0f32, 200.0, 10.0]), g.pick(&[50.0f32, 200.0, 10.0])];
        let shift = [g.f(150.0), g.f(150.0), g.f(150.0)];
        let lo = [bc[0] + shift[0] - half[0], bc[1] + shift[1] - half[1], bc[2] + shift[2] - half[2], 1.0];
        let hi = [lo[0] + 2.0 * half[0], lo[1] + 2.0 * half[1], lo[2] + 2.0 * half[2], 1.0];
        wv(&mut r.mem, tri, lo);
        wv(&mut r.mem, tri + 16, hi);
        for o in (0..0x40).step_by(4) {
            r.mem.write_u32(outs + o, 0x7777_7777);
        }
        r.cpu.set_gpr(4, shape as u64);
        r.cpu.set_gpr(5, tri as u64);
        r.cpu.set_gpr(6, (tri + 16) as u64);
        r.cpu.set_gpr(7, outs as u64);
        r.cpu.set_gpr(8, (outs + 16) as u64);
        r.cpu.set_gpr(9, (outs + 32) as u64);
        r.call(0x257ba8).unwrap();
        let got = r.cpu.gpr(2) != 0;
        let ours = s.boxed(lo, hi);
        let theirs = got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)));
        assert_eq!(theirs, ours.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits())), "case {case}: box");
        hits[2] += ours.is_some() as usize;
        // a mesh of triangles about the body
        let nv = 3 + g.next() % 37;
        let nt = 1 + g.next() % 24;
        let vs: Vec<V4> = (0..nv).map(|_| [cur.center[0] + g.f(160.0), cur.center[1] + g.f(160.0), cur.center[2] + g.f(160.0), 1.0]).collect();
        let mut ts = vec![];
        let mut ps = vec![];
        for _ in 0..nt {
            let t3 = [g.next() % nv, g.next() % nv, g.next() % nv];
            let (a, b2, c2) = (vs[t3[0] as usize], vs[t3[1] as usize], vs[t3[2] as usize]);
            let e1 = [b2[0] - a[0], b2[1] - a[1], b2[2] - a[2]];
            let e2 = [c2[0] - a[0], c2[1] - a[1], c2[2] - a[2]];
            let nn = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let l = (nn[0] * nn[0] + nn[1] * nn[1] + nn[2] * nn[2]).sqrt().max(1e-6);
            ts.push(t3);
            ps.push([nn[0] / l, nn[1] / l, nn[2] / l, 0.0]);
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
        for o in (0..0x40).step_by(4) {
            r.mem.write_u32(outs + o, 0x7777_7777);
        }
        r.cpu.set_gpr(4, shape as u64);
        r.cpu.set_gpr(5, mesh as u64);
        r.cpu.set_gpr(6, either as u64);
        r.cpu.set_gpr(7, outs as u64);
        r.cpu.set_gpr(8, (outs + 16) as u64);
        r.cpu.set_gpr(9, (outs + 32) as u64);
        r.call(0x25d3d8).unwrap();
        let got = r.cpu.gpr(2) != 0;
        let cm = CollMesh { tris: ts, verts: vs, planes: ps };
        let ours = s.mesh(&cm, either);
        let theirs = got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)));
        assert_eq!(theirs, ours.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits())), "case {case}: mesh");
        hits[3] += ours.is_some() as usize;
        // back to the rider's body
        r.cpu.set_gpr(4, shape as u64);
        r.call(0x257978).unwrap();
        s.pop();
        assert_eq!((r.mem.read_u32(shape + 4), r.mem.read_u32(shape + 0x50)), (0, body), "case {case}: popped");
    }
    eprintln!("{hits:?} hits (triangles, either side, boxes, meshes)");
}

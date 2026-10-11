//! Sphere trees in the world (board row E7a4) against the game in the function runner (TRICKY_ELF, TRICKY_SSF_DIR;
//! skipped without them): every course's trees loaded by `Course_LoadSphereTrees`, packed ones decoded by
//! `SphTree_RleDecode` and compared with `rle_decode`, then trees turned and moved by `SphTree_SetTransform` and
//! random collision bodies dropped about them through `Collide_BoarderSphTree` and `cWorldBoarder` +0x38; every
//! output bit for bit.

use std::cell::Cell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::ssf::Ssf;
use tricky_game::collbody::CollBody;
use tricky_game::sphtree::{rle_decode, SphereTree};
use tricky_game::world::Shape;
use tricky_game::worldboarder::WorldBoarder;

type V4 = [f32; 4];

const COURSES: [&str; 11] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska"];

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

#[test]
fn sphere_trees_match_the_original() {
    let Ok(dir) = std::env::var("TRICKY_SSF_DIR") else { return };
    let mut g = Rng(0x5ea7_7ee5_0000_0001);
    let (mut packed, mut hits, mut casts) = (0usize, 0usize, 0usize);
    for c in COURSES {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.ssf")) else { continue };
        let objects = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap());
        let ssf = Ssf::parse(&bytes, objects).unwrap();
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        // the course's trees, loaded as the .ssf test loads them (row E3c1)
        let file = r.mem.alloc(bytes.len() as u32 + 16);
        r.mem.write(file, &bytes).unwrap();
        let course = r.mem.alloc(0x40);
        let model_data = r.mem.alloc(0x20);
        r.mem.write_u32(course + 4, model_data);
        r.mem.write_u32(model_data + 0xc, objects);
        let objs = r.mem.alloc(objects.max(1) * 0x100);
        let models = r.mem.alloc(objects.max(1) * 0x20);
        for i in 0..objects {
            r.mem.write_u32(objs + i * 0x100 + 0xc0, models + i * 0x20);
        }
        let segs = r.mem.alloc(ssf.spline_segments.len().max(1) as u32 * 0x40);
        let heap = Rc::new(Cell::new(0x0080_0000u32));
        r.stub(0x2cc808, |_, _| {});
        r.stub(0x254a28, move |c, _| c.set_gpr(2, (objs + c.gpr(5) as u32 * 0x100) as u64));
        r.stub(0x254b20, move |c, _| c.set_gpr(2, (segs + c.gpr(5) as u32 * 0x40) as u64));
        r.stub(0x2c7e38, |c, _| c.set_gpr(2, 1));
        r.stub(0x2c8a70, move |c, _| c.set_gpr(2, file as u64));
        r.stub(0x14a040, |_, _| {});
        let h = heap.clone();
        r.stub(0x2ccf70, move |c, _| {
            let at = h.get();
            h.set((at + c.gpr(5) as u32 + 15) & !15);
            c.set_gpr(2, at as u64);
        });
        r.stub(0x238af0, |_, _| {});
        r.allow(0x239ba8);
        r.stub(0x23b520, |_, _| {});
        r.allow(0x239340);
        r.stub(0x2ce114, |c, m| {
            let mut b = vec![0u8; c.gpr(6) as usize];
            m.read(c.gpr(5) as u32, &mut b);
            m.write(c.gpr(4) as u32, &b).unwrap();
        });
        r.call(0x23bab0).unwrap(); // the directions' table
        r.call(0x251238).unwrap(); // the identity matrices
        let path = r.mem.alloc(16);
        r.cpu.set_gpr(4, course as u64);
        r.cpu.set_gpr(5, path as u64);
        r.call(0x25f598).unwrap();
        // packed trees: the cache's decoding is the game's `SphTree_RleDecode`; the cache hands it back
        let decode_buf = 0x0120_0000u32 + 0x0010_0000;
        let bufs: Rc<std::cell::RefCell<std::collections::HashMap<u32, (u32, u32)>>> = Rc::new(std::cell::RefCell::new(std::collections::HashMap::new()));
        r.allow(0x239b20);
        r.stub(0x238f50, move |c, m| {
            let tree = c.gpr(5) as u32;
            let mut src = vec![0u8; 0x1_0000];
            let data = m.read_u32(tree + 0x24);
            m.read(data, &mut src);
            let out = rle_decode(&src);
            // each tree its own buffer (the game's cache keeps several)
            let mut map = bufs.borrow_mut();
            let next = map.values().map(|(a, n)| a + n).max().unwrap_or(decode_buf);
            let at = map.entry(tree).or_insert(((next + 15) & !15, out.len() as u32)).0;
            m.write(at, &out).unwrap();
            c.set_gpr(2, at as u64);
        });
        r.stub(0x2391c8, |_, _| {});
        for a in [0x2394a8, 0x23a728, 0x239d60, 0x102f50, 0x257810, 0x235fc8, 0x236830, 0x257af8] {
            r.allow(a);
        }
        let (gtab, tbase) = (r.mem.read_u32(course + 0x18), r.mem.read_u32(course + 0x1c));
        let _ = gtab;
        let mut k = 0u32;
        let mut trees = vec![];
        for gr in &ssf.sphere_trees {
            for t in gr {
                if t.flag != 0 {
                    // the game's decoder against ours
                    let (src, dst) = (0x0110_0000u32, 0x0130_0000u32);
                    r.mem.write(src, &t.data).unwrap();
                    r.cpu.set_gpr(4, dst as u64);
                    r.cpu.set_gpr(5, src as u64);
                    r.call(0x239b20).unwrap();
                    let n = r.cpu.gpr(2) as usize;
                    let mut got = vec![0u8; n];
                    r.mem.read(dst, &mut got);
                    assert_eq!(got, rle_decode(&t.data), "{c}: tree {k} decoded");
                    packed += 1;
                }
                trees.push((tbase + 0x150 * k, SphereTree::new(t)));
                k += 1;
            }
        }
        if trees.is_empty() {
            continue;
        }
        let body = r.mem.alloc(0x280);
        let shape = r.mem.alloc(0x300);
        let mat = r.mem.alloc(0x40);
        let outs = r.mem.alloc(0x40);
        for case in 0..2000 {
            let pick = g.next() as usize % trees.len();
            let (at, ref mut tree) = trees[pick];
            // sometimes turned and moved
            if g.next() % 3 == 0 {
                let (sn, cs) = g.pick(&[(0.0f32, 1.0f32), (0.6, 0.8), (-0.8, 0.6), (1.0, 0.0)]);
                let m = [[cs, sn, 0.0, 0.0], [-sn, cs, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.f(300.0), g.f(300.0), g.f(100.0), 1.0]];
                for (i, row) in m.iter().enumerate() {
                    wv(&mut r.mem, mat + 16 * i as u32, *row);
                }
                r.cpu.set_gpr(4, at as u64);
                r.cpu.set_gpr(5, mat as u64);
                r.call(0x239d60).unwrap();
                tree.set_transform(m);
                assert_eq!(rw(&mut r.mem, at + 0x10, 36), tree.dirs.iter().chain([&tree.centre]).flat_map(|v| b4(*v)).collect::<Vec<u32>>(), "{c} case {case}: transformed");
            }
            // a body about the tree's centre, out to its size
            let size = tree.levels.first().map_or(100.0, |l| l.0);
            let ctr = tree.centre;
            let mut b = CollBody::new();
            b.center = [ctr[0] + g.f(size * 1.2), ctr[1] + g.f(size * 1.2), ctr[2] + g.f(size * 1.2), 1.0];
            b.radius = g.pick(&[60.0f32, 120.0, 25.0]);
            b.count = g.pick(&[18, 0, 3]);
            b.x24 = g.pick(&[0, -1]);
            for s in b.spheres.iter_mut().take(b.count as usize) {
                *s = ([b.center[0] + g.f(70.0), b.center[1] + g.f(70.0), b.center[2] + g.f(90.0), 1.0], g.pick(&[10.0f32, 20.0, 30.0]));
            }
            for o in (0..0x280).step_by(4) {
                r.mem.write_u32(body + o, 0);
            }
            wv(&mut r.mem, body + 0x10, b.center);
            r.mem.write_f32(body + 0x20, b.radius);
            r.mem.write_u32(body + 0x24, b.x24 as u32);
            r.mem.write_u32(body + 0x28, b.count as u32);
            for (j, (cc, rr)) in b.spheres.iter().enumerate() {
                wv(&mut r.mem, body + 0x30 + 0x20 * j as u32, *cc);
                r.mem.write_f32(body + 0x40 + 0x20 * j as u32, *rr);
            }
            for o in (0..0x40).step_by(4) {
                r.mem.write_u32(outs + o, 0x7777_7777);
            }
            r.cpu.set_gpr(4, at as u64);
            r.cpu.set_gpr(5, body as u64);
            r.cpu.set_gpr(6, outs as u64);
            r.cpu.set_gpr(7, (outs + 16) as u64);
            r.call(0x2394a8).unwrap();
            let got = r.cpu.gpr(2) != 0;
            let ours = tree.body(&b);
            assert_eq!(got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4))), ours.map(|(p, q)| (b4(p), b4(q))), "{c} case {case}: the body");
            // and through the rider's shape
            for o in (0..0x300).step_by(4) {
                r.mem.write_u32(shape + o, 0);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, body as u64);
            r.cpu.set_gpr(6, 1);
            r.call(0x257810).unwrap();
            for o in (0..0x40).step_by(4) {
                r.mem.write_u32(outs + o, 0x7777_7777);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, at as u64);
            r.cpu.set_gpr(6, outs as u64);
            r.cpu.set_gpr(7, (outs + 16) as u64);
            r.cpu.set_gpr(8, (outs + 32) as u64);
            r.call(0x257af8).unwrap();
            let got = r.cpu.gpr(2) != 0;
            let s = WorldBoarder::new(&b, 1);
            let ours = s.sphere_tree(tree);
            assert_eq!(
                got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32))),
                ours.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits())),
                "{c} case {case}: through the shape"
            );
            hits += ours.is_some() as usize;
            casts += 1;
        }
    }
    eprintln!("{packed} packed trees decoded; {hits} hits in {casts} bodies");
}

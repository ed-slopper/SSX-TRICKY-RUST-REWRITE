//! `cWorldSphTree` (board row E7a5); its trees as in tests/sphtree.rs against the game in the function runner (TRICKY_ELF, TRICKY_SSF_DIR;
//! skipped without them): every course's trees loaded by `Course_LoadSphereTrees`, packed ones decoded by
//! `SphTree_RleDecode` and compared with `rle_decode`, then trees turned and moved by `SphTree_SetTransform` and
//! random collision bodies dropped about them through `Collide_BoarderSphTree` and `cWorldBoarder` +0x38; every
//! output bit for bit.

use std::cell::Cell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::ssf::Ssf;
use tricky_game::sphtree::{rle_decode, SphereTree};
use tricky_game::world::Shape;
use tricky_game::sphtree::WorldSphTree;
use tricky_game::worldline::tri_contains_point;

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
fn world_sphere_trees_match_the_original() {
    let Ok(dir) = std::env::var("TRICKY_SSF_DIR") else { return };
    let mut g = Rng(0x5ea7_7ee5_0000_0001);
    let mut hits = [0usize; 5];
    let mut packed = 0usize;
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
        r.stub(0x23b6b0, |c, m| {
            let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
            let v = tri_contains_point(g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
            c.set_gpr(2, v as u64);
        });
        for a in [0x261638, 0x239c50, 0x261700, 0x261760, 0x261770, 0x261818, 0x2618c0, 0x261968, 0x261a10, 0x2391e8, 0x239f88, 0x23a390, 0x23ada0, 0x23b398, 0x2cfd90, 0x2395c0, 0x23a490, 0x23a570, 0x23a210, 0x23a9a0, 0x23b6e8] {
            r.allow(a);
        }
        let shape = r.mem.alloc(0x300);
        let mat = r.mem.alloc(0x40);
        let tri = r.mem.alloc(0x40);
        let outs = r.mem.alloc(0x40);
        for case in 0..1500 {
            let pick = g.next() as usize % trees.len();
            let at = trees[pick].0;
            let tree0 = trees[pick].1.clone();
            for o in (0..0x300).step_by(4) {
                r.mem.write_u32(shape + o, 0);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, at as u64);
            r.call(0x261638).unwrap();
            let mut s = WorldSphTree::new(&tree0);
            assert_eq!(
                (r.mem.read_u32(shape), rw(&mut r.mem, shape + 0x10, 4), rw(&mut r.mem, shape + 0x20, 4), rw(&mut r.mem, shape + 0x30, 4)),
                (s.kind, b4(s.hi), b4(s.lo), b4(s.mid)),
                "{c} case {case}: built"
            );
            if g.next() % 3 == 0 {
                let (sn, cs) = g.pick(&[(0.0f32, 1.0f32), (0.6, 0.8), (1.0, 0.0)]);
                let m = [[cs, sn, 0.0, 0.0], [-sn, cs, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.f(200.0), g.f(200.0), g.f(80.0), 1.0]];
                for (i, row) in m.iter().enumerate() {
                    wv(&mut r.mem, mat + 16 * i as u32, *row);
                }
                r.cpu.set_gpr(4, shape as u64);
                r.cpu.set_gpr(5, mat as u64);
                r.call(0x261700).unwrap();
                s.push(m);
                assert_eq!(rw(&mut r.mem, shape + 0x60 + 0x10, 36), s.tree.dirs.iter().chain([&s.tree.centre]).flat_map(|v| b4(*v)).collect::<Vec<u32>>(), "{c} case {case}: put in the frame");
            }
            let t = s.tree.clone();
            let rad = t.levels.first().map_or(100.0, |l| l.0);
            // a triangle near the tree
            let n = {
                let v = [g.f(1.0), g.f(1.0), g.pick(&[1.0f32, -1.0, 0.4]), 0.0];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                v.map(|x| x / l)
            };
            let d = rad * g.pick(&[0.2f32, 0.6, -0.3, 0.9, 1.3]);
            let foot = [t.centre[0] - n[0] * d, t.centre[1] - n[1] * d, t.centre[2] - n[2] * d, 1.0];
            let t1 = if n[2].abs() < 0.9 { [-n[1], n[0], 0.0] } else { [0.0, -n[2], n[1]] };
            let t2 = [n[1] * t1[2] - n[2] * t1[1], n[2] * t1[0] - n[0] * t1[2], n[0] * t1[1] - n[1] * t1[0]];
            let size = rad * g.pick(&[3.0f32, 0.5, 0.15]);
            let off = [g.f(size), g.f(size)];
            let pt = |u: f32, v: f32| [foot[0] + t1[0] * (u + off[0]) + t2[0] * (v + off[1]), foot[1] + t1[1] * (u + off[0]) + t2[1] * (v + off[1]), foot[2] + t1[2] * (u + off[0]) + t2[2] * (v + off[1]), 1.0];
            let (a, bb, cc) = (pt(-size, -size), pt(size, -size * 0.5), pt(0.0, size));
            let plane = [n[0], n[1], n[2], 0.0];
            for (k, v) in [a, bb, cc, plane].iter().enumerate() {
                wv(&mut r.mem, tri + 16 * k as u32, *v);
            }
            for (either, f) in [(false, 0x261770u32), (true, 0x261818)] {
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
                r.call(f).unwrap();
                let got = r.cpu.gpr(2) != 0;
                let ours = if either { s.triangle_either_side(a, bb, cc, plane) } else { s.triangle(a, bb, cc, plane) };
                let theirs = got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)));
                assert_eq!(theirs, ours.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits())), "{c} case {case}: triangle (either side {either})");
                hits[either as usize] += ours.is_some() as usize;
            }
            // a box near the tree
            let half = [rad * g.pick(&[0.3f32, 1.0, 2.0]), rad * g.pick(&[0.3f32, 1.0, 2.0]), rad * g.pick(&[0.3f32, 1.0, 2.0])];
            let shift = [g.f(rad * 1.5), g.f(rad * 1.5), g.f(rad * 1.5)];
            let lo = [t.centre[0] + shift[0] - half[0], t.centre[1] + shift[1] - half[1], t.centre[2] + shift[2] - half[2], 1.0];
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
            r.call(0x261968).unwrap();
            let got = r.cpu.gpr(2) != 0;
            let ours = s.boxed(lo, hi);
            let theirs = got.then(|| (rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)));
            assert_eq!(theirs, ours.map(|h| (b4(h.pos), b4(h.normal), h.t.to_bits())), "{c} case {case}: box");
            hits[2] += ours.is_some() as usize;
            // another tree of the course, moved next to this one
            let pick2 = g.next() as usize % trees.len();
            let at2 = trees[pick2].0;
            let mut other = trees[pick2].1.clone();
            let mv = [t.centre[0] - other.centre[0] + g.f(rad), t.centre[1] - other.centre[1] + g.f(rad), t.centre[2] - other.centre[2] + g.f(rad), 0.0];
            let m2 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [mv[0], mv[1], mv[2], 1.0]];
            for (i, row) in m2.iter().enumerate() {
                wv(&mut r.mem, mat + 16 * i as u32, *row);
            }
            r.cpu.set_gpr(4, at2 as u64);
            r.cpu.set_gpr(5, mat as u64);
            r.call(0x239f88).unwrap();
            other.transform(m2);
            for o in (0..0x40).step_by(4) {
                r.mem.write_u32(outs + o, 0x7777_7777);
            }
            r.cpu.set_gpr(4, shape as u64);
            r.cpu.set_gpr(5, at2 as u64);
            r.cpu.set_gpr(6, outs as u64);
            r.cpu.set_gpr(7, (outs + 16) as u64);
            r.cpu.set_gpr(8, (outs + 32) as u64);
            r.call(0x2618c0).unwrap();
            let got = r.cpu.gpr(2) != 0;
            if pick2 == pick && s.local == 0 {
                s.tree = other.clone();
            }
            let ours = s.tree_hit(&other);
            if got != ours.is_some() {
                eprintln!("DBG A depth {} occ0 {:?} levels {:?} centre {:?} | B depth {} levels {:?} centre {:?} | ours {:?}", s.tree.depth, s.tree.masks.get(0), &s.tree.levels, s.tree.centre, other.depth, &other.levels[..1], other.centre, ours);
                let gb: Vec<f32> = rw(&mut r.mem, at2 + 0x90, 4).iter().map(|x| f32::from_bits(*x)).collect();
                eprintln!("DBG game B centre {gb:?} local {}", s.local);
            }
            assert_eq!(got, ours.is_some(), "{c} case {case}: tree against tree");
            if let Some(Some(h)) = ours {
                assert_eq!((rw(&mut r.mem, outs, 4), rw(&mut r.mem, outs + 16, 4), r.mem.read_u32(outs + 32)), (b4(h.pos), b4(h.normal), h.t.to_bits()), "{c} case {case}: tree against tree");
                hits[3] += 1;
            } else if ours.is_some() {
                hits[4] += 1;
            }
            // the moved tree stays moved, on both sides
            trees[pick2].1 = other;
        }
    }
    eprintln!("{packed} packed; hits {hits:?} (triangles, either side, boxes, trees, trees with nothing written)");
}

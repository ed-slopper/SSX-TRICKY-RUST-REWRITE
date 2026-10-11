//! The terrain's patch test (board row E7c1) against the game in the function runner (TRICKY_ELF, TRICKY_PBD_DIR;
//! skipped without them): the disc's patches tessellated by `terrain::tessellate` into a cache entry laid out by
//! the game's `CacheBuffer_ClearEntry` (the runner cannot run the vector unit's patch program), then lines of every
//! kind through them, with and without the caller's last-hit record, through `cWorldIntersectType` +0x48 0x25c7c8;
//! the answer, the hit, the last-hit record and every quad record the game filled compared bit for bit. A kind-2
//! hit's refinement (`cWorldLine` +0x50, row E7c2) is stubbed and its (u, v) compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::pbd::Pbd;
use tricky_game::terrain::{patch_test, LastHit, PatchGrid};
use tricky_game::worldline::{tri_contains_point, WorldLine};

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
}

const SENTINEL: u32 = 0x5015_02f9; // 1e10

fn wv(m: &mut Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

fn rw(m: &mut Mem, a: u32, n: u32) -> Vec<u32> {
    (0..n).map(|k| m.read_u32(a + 4 * k)).collect()
}

#[test]
fn patch_test_matches_the_original() {
    let Ok(dir) = std::env::var("TRICKY_PBD_DIR") else { return };
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x25c7c8, 0x25d9e0, 0x25d7f0, 0x252028, 0x102f50] {
        r.allow(a);
    }
    r.stub(0x23b6b0, |c, m| {
        let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
        let v = tri_contains_point(g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
        c.set_gpr(2, v as u64);
    });
    let refined: Rc<RefCell<Vec<(u32, u32, u32)>>> = Rc::new(RefCell::new(vec![]));
    let rf = refined.clone();
    r.stub(0x25df50, move |c, _| rf.borrow_mut().push((c.f(12).to_bits(), c.f(13).to_bits(), c.gpr(5) as u32)));
    let patch = r.mem.alloc(0x1c0);
    let entry = r.mem.alloc(0x1cd0);
    let line = r.mem.alloc(0xa0);
    let from = r.mem.alloc(16);
    let to = r.mem.alloc(16);
    let out = r.mem.alloc(0x30);
    let last = r.mem.alloc(0x20);
    let mut g = Rng(0x7e55_e11a_7e00_0001);
    let (mut hits, mut casts, mut refines) = (0usize, 0usize, 0usize);
    for c in ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska"] {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.pbd")) else { continue };
        let pbd = Pbd::parse(&bytes).unwrap();
        for _ in 0..60 {
            let pi = g.next() as usize % pbd.patches.len();
            let raw = &pbd.patches[pi];
            let words: Vec<u32> = raw.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            for (k, w) in words.iter().enumerate() {
                r.mem.write_u32(patch + 4 * k as u32, *w);
            }
            let coeffs: [V4; 16] = std::array::from_fn(|k| [0, 1, 2, 3].map(|l| f32::from_bits(words[0x14 + 4 * k + l])));
            let mut grid = PatchGrid::new(&coeffs);
            // the cache entry: cleared as the game clears it, the patch and its points
            for o in (0..0x1cd0).step_by(4) {
                r.mem.write_u32(entry + o, 0);
            }
            r.cpu.set_gpr(4, 0);
            r.cpu.set_gpr(5, entry as u64);
            r.call(0x252028).unwrap();
            r.mem.write_u32(entry, patch);
            for (k, p) in grid.points.iter().enumerate() {
                wv(&mut r.mem, entry + 0x10 + 16 * k as u32, *p);
            }
            let mut ours_last = LastHit::default();
            for _ in 0..25 {
                let kind = g.pick(&[0u32, 1, 2, 7, 1]);
                // a line through a point of the patch (or near it)
                let p = grid.points[g.next() as usize % 100];
                let up = g.pick(&[50.0f32, 300.0, 1500.0]);
                let side = [g.pick(&[0.0f32, 0.0, 40.0, -120.0]), g.pick(&[0.0f32, 0.0, 60.0, -25.0])];
                let fr = [p[0] - side[0], p[1] - side[1], p[2] + up, 1.0];
                let tt = [p[0] + side[0], p[1] + side[1], p[2] - up * g.pick(&[1.0f32, 0.2]), 1.0];
                let target = g.pick(&[0.5f32, 0.0, -1.0]);
                wv(&mut r.mem, from, fr);
                wv(&mut r.mem, to, tt);
                r.cpu.set_f(12, target);
                r.cpu.set_gpr(4, line as u64);
                r.cpu.set_gpr(5, from as u64);
                r.cpu.set_gpr(6, to as u64);
                r.cpu.set_gpr(7, kind as u64);
                r.call(0x25d7f0).unwrap();
                let q = WorldLine::new(target, fr, tt, kind);
                // the last-hit record: none, as the last call left it, or a made-up one (sometimes this patch)
                let with_last = g.next() % 4 != 0;
                if g.next() % 3 == 0 {
                    let n = if kind == 0 { 3 } else { 9 };
                    ours_last = LastHit {
                        patch: if g.next() % 2 == 0 { patch } else { 0x1234 },
                        row: (g.next() % n) as i16,
                        col: (g.next() % n) as i16,
                        tri_a: g.next() % 2,
                        kind: if g.next() % 3 == 0 { 9 } else { kind },
                        cell: 5,
                        node: 6,
                        index: 7,
                    };
                }
                let lw = |l: &LastHit| vec![l.patch, l.row as u16 as u32 | (l.col as u16 as u32) << 16, l.tri_a, l.kind, l.cell as u16 as u32 | (l.node as u16 as u32) << 16, l.index as u16 as u32];
                for (k, w) in lw(&ours_last).iter().enumerate() {
                    r.mem.write_u32(last + 4 * k as u32, *w);
                }
                refined.borrow_mut().clear();
                r.cpu.set_gpr(4, line as u64);
                r.cpu.set_gpr(5, patch as u64);
                r.cpu.set_gpr(6, entry as u64);
                r.cpu.set_gpr(7, out as u64);
                r.cpu.set_gpr(8, (out + 0x10) as u64);
                r.cpu.set_gpr(9, (out + 0x20) as u64);
                r.cpu.set_gpr(10, if with_last { last as u64 } else { 0 });
                r.call(0x25c7c8).unwrap();
                let got = r.cpu.gpr(2) as u32;
                let ours = patch_test(&q, &mut grid, patch, if with_last { Some(&mut ours_last) } else { None });
                let what = format!("{c} patch {pi} kind {kind}");
                assert_eq!(got != 0, ours.is_some(), "{what}: hit");
                if let Some(h) = ours {
                    let theirs = (rw(&mut r.mem, out, 4), rw(&mut r.mem, out + 0x10, 4), r.mem.read_u32(out + 0x20));
                    assert_eq!(theirs, (h.hit.pos.map(f32::to_bits).to_vec(), h.hit.normal.map(f32::to_bits).to_vec(), h.hit.t.to_bits()), "{what}: the hit");
                    let want: Vec<(u32, u32, u32)> = h.refine.map(|(u, v)| (u.to_bits(), v.to_bits(), patch)).into_iter().collect();
                    assert_eq!(*refined.borrow(), want, "{what}: refined at");
                    hits += 1;
                    refines += want.len();
                }
                if with_last {
                    let theirs = rw(&mut r.mem, last, 6);
                    let mut mine = lw(&ours_last);
                    // the halves the game resets as one word with the next (+0x14 then +0x16, which is not ours)
                    mine[5] = mine[5] & 0xffff | theirs[5] & 0xffff_0000;
                    assert_eq!(theirs, mine, "{what}: last hit");
                }
                // every quad record the game filled
                for (base, quads) in [(0x650u32, &grid.fine[..]), (0x1a90, &grid.coarse[..])] {
                    for (k, qd) in quads.iter().enumerate() {
                        let rec = rw(&mut r.mem, entry + base + 0x40 * k as u32, 16);
                        let opt4 = |o: Option<V4>, at: usize| match o {
                            Some(v) => assert_eq!(rec[at..at + 4].to_vec(), v.map(f32::to_bits).to_vec(), "{what}: quad {base:#x}+{k} at {at}"),
                            None => assert_eq!(rec[at], SENTINEL, "{what}: quad {base:#x}+{k} at {at} filled"),
                        };
                        opt4(qd.normal_a, 0);
                        opt4(qd.normal_b, 4);
                        match (qd.min, qd.max) {
                            (Some(lo), Some(hi)) => assert_eq!((rec[8..11].to_vec(), rec[12..15].to_vec()), (lo.map(f32::to_bits).to_vec(), hi.map(f32::to_bits).to_vec()), "{what}: quad {base:#x}+{k} box"),
                            _ => assert_eq!(rec[8], SENTINEL, "{what}: quad {base:#x}+{k} box filled"),
                        }
                    }
                }
                casts += 1;
            }
        }
    }
    eprintln!("{casts} patch tests, {hits} hits ({refines} to refine)");
}

/// The tessellation against the patches' polynomial in double precision (the s^a t^b coefficient at index
/// 15 − (4a + b), row E3a2): point i·10 + j is the surface at s = j/9, t = i/9.
#[test]
fn tessellation_follows_the_polynomial() {
    let Ok(dir) = std::env::var("TRICKY_PBD_DIR") else { return };
    let mut worst = 0f64;
    for c in ["gari", "snow", "alaska", "megaple"] {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.pbd")) else { continue };
        let pbd = Pbd::parse(&bytes).unwrap();
        for raw in pbd.patches.iter().step_by(7) {
            let w: Vec<u32> = raw.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            let coeffs: [V4; 16] = std::array::from_fn(|k| [0, 1, 2, 3].map(|l| f32::from_bits(w[0x14 + 4 * k + l])));
            let pts = tricky_game::terrain::tessellate(&coeffs);
            for i in 0..10 {
                for j in 0..10 {
                    let (s, t) = (j as f64 / 9.0, i as f64 / 9.0);
                    for x in 0..3 {
                        let mut v = 0f64;
                        let mut mag = 0f64;
                        for a in 0..4 {
                            for b in 0..4 {
                                let term = coeffs[15 - (4 * a + b)][x] as f64 * s.powi(a as i32) * t.powi(b as i32);
                                v += term;
                                mag += term.abs();
                            }
                        }
                        let err = (pts[i * 10 + j][x] as f64 - v).abs() / (mag + 1.0);
                        worst = worst.max(err);
                        assert!(err < 1e-5, "{c}: point {i},{j} lane {x}: {} vs {v}", pts[i * 10 + j][x]);
                    }
                }
            }
        }
    }
    eprintln!("worst relative difference {worst:e}");
}

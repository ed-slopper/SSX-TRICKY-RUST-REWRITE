//! A course's collision built in the function runner by the game's own loaders (`tests/world.rs` made it first):
//! the course object at 0x346488 from the course's `.pbd` (`ResolveDataHeader`), `.ltg` (`ResolveTGroupHeader`)
//! and `.ssf` (`Course_LoadSphereTrees`), so `World_RayCast` 0x25aff8 and the casts under it run as the original.
#![allow(dead_code)]

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use r5900::{Mem, Runner};

type V4 = [f32; 4];

/// The course object (`gWorld`).
pub const W: u32 = 0x346488;

/// Where the loaders left the patches and instances (+0x1c0 a patch, +0x100 an instance).
pub struct RunnerCourse {
    pub patch0: u32,
    pub inst0: u32,
}

/// The course object built from the files: the `.pbd` at 0x80_0000, the `.ltg` at 0x100_0000, the `.ssf` at
/// 0x110_0000 and what its loader allocates from 0x120_0000 (the game's own heap space, below the runner's test
/// heap). The objects' boxes (+0xcc, +0xd8) and flags (+0xe8) are as the file has them: the caller sets them.
pub fn load(r: &mut Runner, pb: &[u8], lb: &[u8], sb: &[u8]) -> RunnerCourse {
    // the course object: the .pbd resolved (row E3a)
    r.stub(0x255310, |_, _| {});
    r.stub(0x2f7aa8, |_, _| {});
    for a in [
        0x253400, 0x253540, 0x2535b0, 0x253630, 0x2537d8, 0x2539b0, 0x255c20, 0x253fb0, 0x254078, 0x254120, 0x2534c8, 0x253b60, 0x253bd0, 0x253c50, 0x253d30, 0x255cc0, 0x254160, 0x2541e8,
        0x2542a8, 0x254318, 0x254398, 0x254600, 0x254678, 0x2546a8, 0x254e58, 0x254e98, 0x2522e0, 0x252318, 0x2549f8, 0x254a28, 0x254a50, 0x254a80, 0x254af0, 0x254b20, 0x254b50, 0x254b78,
        0x252748, 0x2527e0, 0x252b88,
    ] {
        r.allow(a);
    }
    r.call(0x251238).unwrap(); // the identity matrices
    for o in (0..0x40).step_by(4) {
        r.mem.write_u32(W + o, 0);
    }
    let pf = 0x0080_0000u32;
    assert!(pb.len() < 0x80_0000);
    r.mem.write(pf, pb).unwrap();
    let blocks = r.mem.alloc(16000);
    let flips = r.mem.alloc(4000);
    r.mem.write_u32(W + 4, pf);
    r.mem.write_u32(W + 8, blocks);
    r.mem.write_u32(W + 0xc, flips);
    r.mem.write_u32(W + 0x10, 0x3a7d50);
    r.cpu.set_gpr(4, W as u64);
    r.cpu.set_gpr(5, pf as u64);
    r.call(0x255630).unwrap();
    // the grid (row E3e), its lists through the real lookups
    let lf = 0x0100_0000u32;
    assert!(lb.len() < 0x10_0000);
    r.mem.write(lf, lb).unwrap();
    let hdr = r.mem.alloc(8);
    r.mem.write(hdr, &lb[..4]).unwrap();
    r.cpu.set_gpr(4, W as u64);
    r.cpu.set_gpr(5, lf as u64);
    r.cpu.set_gpr(6, hdr as u64);
    r.call(0x252470).unwrap();
    assert_eq!(r.cpu.gpr(2), 1, "the grid");
    if r.mem.read_u32(W) == 0 {
        r.mem.write_u32(W, lf);
    }
    // the .ssf (row E3c1): objects' flags and physics, the collision tables
    let sf = 0x0110_0000u32;
    assert!(sb.len() < 0x10_0000);
    r.mem.write(sf, sb).unwrap();
    let heap = Rc::new(Cell::new(0x0120_0000u32));
    r.stub(0x2cc808, |_, _| {});
    r.stub(0x2c7e38, |c, _| c.set_gpr(2, 1));
    r.stub(0x2c8a70, move |c, _| c.set_gpr(2, sf as u64));
    r.stub(0x14a040, |_, _| {});
    let hp = heap.clone();
    r.stub(0x2ccf70, move |c, _| {
        let at = hp.get();
        hp.set((at + c.gpr(5) as u32 + 15) & !15);
        assert!(hp.get() < 0x0180_0000, "the course's heap");
        c.set_gpr(2, at as u64);
    });
    r.stub(0x238af0, |_, _| {});
    r.stub(0x239ba8, |_, _| {});
    r.stub(0x23b520, |_, _| {});
    r.allow(0x239340);
    r.stub(0x2ce114, |c, m| {
        let mut b = vec![0u8; c.gpr(6) as usize];
        m.read(c.gpr(5) as u32, &mut b);
        m.write(c.gpr(4) as u32, &b).unwrap();
    });
    let path = r.mem.alloc(16);
    r.cpu.set_gpr(4, W as u64);
    r.cpu.set_gpr(5, path as u64);
    r.call(0x25f598).unwrap();
    // the first patch and object, through the game's getters
    let get = |r: &mut Runner, a: u32, i: u32| {
        r.cpu.set_gpr(4, W as u64);
        r.cpu.set_gpr(5, i as u64);
        r.call(a).unwrap();
        r.cpu.gpr(2) as u32
    };
    RunnerCourse { patch0: get(r, 0x2549f8, 0), inst0: get(r, 0x254a28, 0) }
}

impl RunnerCourse {
    /// `World_RayCast` 0x25aff8 and `World_RayCastInstance` 0x25bf48 run as the original (with the grid walk,
    /// `cWorldIntersectType` +0x48 0x25c7c8 on the patches); `World_LookupCollCache` 0x25c750 answered with one
    /// cache entry laid out as `CacheBuffer_ClearEntry` 0x252028 leaves it and holding the patch's points as
    /// `terrain::tessellate` makes them (the runner does not run the VU0 patch program, `Vu0_RunPatchProgram`
    /// 0x2576a8), or with `chop` as PCSX2 runs it, its products and sums toward zero (`tests/terrain.rs`,
    /// `the_running_games_patch_cache`); `Tri_ContainsPointVu0` 0x23b6b0 by its port; the objects' controllers none.
    pub fn cast_as_original(&self, r: &mut Runner, patches: Vec<[V4; 16]>, chop: bool) {
        for a in [0x25aff8, 0x25bf48, 0x25d3d8, 0x25d948, 0x25d9c8, 0x25d9e0, 0x25db18, 0x25dc40, 0x25dc48, 0x25ebd8, 0x25d7a8, 0x102f50, 0x178078, 0x25c7c8] {
            r.allow(a);
        }
        r.stub(0x23b6b0, |c, m| {
            let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
            let v = tricky_game::worldline::tri_contains_point(g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
            c.set_gpr(2, v as u64);
        });
        let entry = r.mem.alloc(0x1cd0);
        let patch0 = self.patch0;
        let mut points: HashMap<usize, [V4; 100]> = HashMap::new();
        r.stub(0x25c750, move |c, m| {
            let p = c.gpr(5) as u32;
            let i = ((p - patch0) / 0x1c0) as usize;
            let pts = points.entry(i).or_insert_with(|| {
                if chop {
                    tricky_game::terrain::tessellate_by(&patches[i], chop_mul, |a, b| r5900::ps2float::add_by(a, b, r5900::ps2float::Round::Chop))
                } else {
                    tricky_game::terrain::tessellate(&patches[i])
                }
            });
            for o in (0..0x1cd0).step_by(4) {
                m.write_u32(entry + o, 0);
            }
            for (base, n) in [(0x650u32, 81u32), (0x1a90, 9)] {
                for k in 0..n {
                    for w in [0, 4, 8] {
                        m.write_u32(entry + base + 0x40 * k + 4 * w, 0x5015_02f9);
                    }
                }
            }
            m.write_u32(entry, p);
            for (k, v) in pts.iter().enumerate() {
                for (j, x) in v.iter().enumerate() {
                    m.write_f32(entry + 0x10 + 16 * k as u32 + 4 * j as u32, *x);
                }
            }
            m.write_u32(c.gpr(6) as u32, entry);
        });
    }

    /// An object's box (+0xcc, +0xd8) and flags (+0xe8).
    pub fn set_object(&self, m: &mut Mem, o: usize, min: [f32; 3], max: [f32; 3], flags: u32) {
        let a = self.inst0 + 0x100 * o as u32;
        for k in 0..3u32 {
            m.write_f32(a + 0xcc + 4 * k, min[k as usize]);
            m.write_f32(a + 0xd8 + 4 * k, max[k as usize]);
        }
        m.write_u32(a + 0xe8, flags);
    }
}

/// A product of two floats toward zero (exact in f64, then cut).
pub fn chop_mul(a: f32, b: f32) -> f32 {
    let x = a as f64 * b as f64;
    let f = x as f32;
    if (f as f64).abs() > x.abs() {
        f32::from_bits(f.to_bits() - 1)
    } else {
        f
    }
}

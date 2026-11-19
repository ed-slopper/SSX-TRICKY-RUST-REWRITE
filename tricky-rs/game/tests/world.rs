//! The world's casts (board row E7a2) against the game in the function runner on the disc's courses (TRICKY_ELF,
//! TRICKY_PBD_DIR, TRICKY_LTG_DIR, TRICKY_SSF_DIR; skipped without them): the course object at 0x346488 built by
//! the game's own loaders (`ResolveDataHeader`, `ResolveTGroupHeader`, `Course_LoadSphereTrees`), some objects given
//! animated controllers, then lines cast through `World_RayCast` and `World_QueryInstances` and compared with
//! `tricky_game::world` (result, hit record, last-patch halves). A patch's own test (the patch cache, row E7c) is
//! answered the same made-up way on both sides, and the order the patches are asked in is compared too.

mod runner_course;

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::pbd::Pbd;
use tricky_data::ssf::Ssf;
use tricky_game::controllers::restore_flags;
use tricky_game::world::{Cell, CollMesh, LastHit, Node, Object, Part, PartMesh, Patch, Shape, World, WorldEnv};
use tricky_game::worldline::{Hit, WorldLine};

type V4 = [f32; 4];

use runner_course::W;
const MODE: u32 = 0x32de8c;

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

fn h32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ x >> 16
}

/// The made-up patch test: about one patch in three is hit, at a t, point and normal that depend on it and the case.
fn fake_patch(salt: u32, i: usize) -> Option<Hit> {
    let k = h32(salt ^ (i as u32).wrapping_mul(0x9e37_79b9));
    if k % 3 != 0 {
        return None;
    }
    let t = (k >> 8 & 0xffff) as f32 / 65535.0;
    Some(Hit { pos: [i as f32, t * 100.0, (k & 0xff) as f32, 1.0], normal: [0.0, (k >> 4 & 1) as f32, 1.0, 0.0], t })
}

/// The made-up animation frames: a turn about z and a shift, by controller and part.
fn fake_frame(ctrl: u32, n: u32) -> [V4; 4] {
    let k = h32(ctrl ^ n.wrapping_mul(0x85eb_ca6b));
    let (s, c) = [(0.0f32, 1.0f32), (0.6, 0.8), (1.0, 0.0), (-0.28, 0.96)][(k % 4) as usize];
    [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [(k >> 4 & 31) as f32, (k >> 9 & 31) as f32 - 16.0, (k >> 14 & 7) as f32, 1.0]]
}

struct Env {
    salt: u32,
    asked: Vec<usize>,
    controllers: Vec<u32>,
}

impl WorldEnv for Env {
    fn patch(&mut self, _: &dyn Shape, i: usize, _: Option<&mut LastHit>) -> Option<Hit> {
        self.asked.push(i);
        fake_patch(self.salt, i)
    }
    fn frame(&mut self, o: usize, n: usize) -> [V4; 4] {
        fake_frame(self.controllers[o], n as u32)
    }
}

fn f3(m: &mut Mem, a: u32) -> [f32; 3] {
    [0, 1, 2].map(|k| m.read_f32(a + 4 * k))
}
fn f4(m: &mut Mem, a: u32) -> V4 {
    [0, 1, 2, 3].map(|k| m.read_f32(a + 4 * k))
}
fn h(m: &mut Mem, a: u32) -> i16 {
    let mut b = [0u8; 2];
    m.read(a, &mut b).unwrap();
    i16::from_le_bytes(b)
}

const COURSES: [&str; 11] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska"];

#[test]
fn world_casts_match_the_original() {
    let (Ok(pdir), Ok(ldir), Ok(sdir)) = (std::env::var("TRICKY_PBD_DIR"), std::env::var("TRICKY_LTG_DIR"), std::env::var("TRICKY_SSF_DIR")) else { return };
    let mut totals = [0usize; 4];
    for c in COURSES {
        let (Ok(pb), Ok(lb), Ok(sb)) = (std::fs::read(format!("{pdir}/{c}.pbd")), std::fs::read(format!("{ldir}/{c}.ltg")), std::fs::read(format!("{sdir}/{c}.ssf"))) else { continue };
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        let pbd = Pbd::parse(&pb).unwrap();
        let ssf = Ssf::parse(&sb, pbd.instances.len() as u32).unwrap();
        // the course object built by the game's loaders
        let runner_course::RunnerCourse { patch0, inst0, .. } = runner_course::load(&mut r, &pb, &lb, &sb);
        let grid = r.mem.read_u32(W);
        // some objects get an animated controller
        let mut g = Rng(0x30e1_d000_0000_0001 ^ c.len() as u64 * 0x1234_5678);
        let ctrls = r.mem.alloc(0x40 * pbd.instances.len() as u32 + 16);
        let mut controllers = vec![0u32; pbd.instances.len()];
        for i in 0..pbd.instances.len() as u32 {
            let o = inst0 + 0x100 * i;
            // the object's box (+0xcc, +0xd8): the file's replaced by the object's position ± a size, the same on
            // both sides, so the casts reach more objects
            let rad = g.pick(&[200.0f32, 800.0, 3000.0]);
            for k in 0..3u32 {
                let p = r.mem.read_f32(o + 0x30 + 4 * k);
                r.mem.write_f32(o + 0xcc + 4 * k, p - rad);
                r.mem.write_f32(o + 0xd8 + 4 * k, p + rad);
            }
            // the flags as a reset leaves them (the upper half's defaults copied down)
            let f = r.mem.read_u32(o + 0xe8);
            r.mem.write_u32(o + 0xe8, restore_flags(f, 0xffff_0000));
            if g.next() % 6 == 0 {
                let ct = ctrls + 0x40 * i;
                let fl = if g.next() % 3 == 0 { 0 } else { 0x100 };
                r.mem.write_u32(ct + 0x14, fl);
                r.mem.write_u32(o + 0xe4, ct);
                controllers[i as usize] = ct;
            }
        }
        r.stub(0x17d1c8, |c, _| {
            let ct = c.gpr(4);
            c.set_gpr(2, ct);
        });
        let frame_buf = r.mem.alloc(64);
        r.stub(0x1998d0, move |c, m| {
            let f = fake_frame(c.gpr(4) as u32, c.gpr(5) as u32);
            for (i, row) in f.iter().enumerate() {
                for (k, v) in row.iter().enumerate() {
                    m.write_f32(frame_buf + 16 * i as u32 + 4 * k as u32, *v);
                }
            }
            c.set_gpr(2, frame_buf as u64);
        });
        // the world, read back from what the game built
        let m = &mut r.mem;
        let patches: Vec<Patch> = (0..pbd.patches.len() as u32)
            .map(|i| {
                let p = patch0 + 0x1c0 * i;
                Patch { show_off: (m.read_u32(p + 0x1ac) as i32) < 0, surface: m.read_u32(p + 0x1a8) as i32, min: f3(m, p + 0x150), max: f3(m, p + 0x15c) }
            })
            .collect();
        let mut model_ptrs: Vec<u32> = vec![];
        let mut objects = vec![];
        for i in 0..pbd.instances.len() as u32 {
            let o = inst0 + 0x100 * i;
            let mp = m.read_u32(o + 0xc0);
            let model = model_ptrs.iter().position(|x| *x == mp).unwrap_or_else(|| {
                model_ptrs.push(mp);
                model_ptrs.len() - 1
            });
            let phys = m.read_u32(o + 0xec);
            let ct = m.read_u32(o + 0xe4);
            objects.push(Object {
                rows: [f4(m, o), f4(m, o + 16), f4(m, o + 32), f4(m, o + 48)],
                min: f3(m, o + 0xcc),
                max: f3(m, o + 0xd8),
                model,
                flags: m.read_u32(o + 0xe8),
                surface: m.read_u32(phys + 0xc) as i32,
                mode: h(m, phys + 0x10),
                group: h(m, phys + 0x12),
                controller: if ct == 0 { None } else { Some(m.read_u32(ct + 0x14)) },
            });
        }
        let models: Vec<Vec<Part>> = model_ptrs
            .iter()
            .map(|&mp| {
                let (n, parts) = (m.read_u32(mp + 4), m.read_u32(mp + 8));
                (0..n)
                    .map(|k| {
                        let p = parts + 0x18 * k;
                        let (mesh, mat) = (m.read_u32(p + 4), m.read_u32(p + 0x14));
                        Part {
                            parent: m.read_u32(p) as i32,
                            animated: m.read_u32(p + 0x10) != 0,
                            matrix: if mat == 0 { None } else { Some([f4(m, mat), f4(m, mat + 16), f4(m, mat + 32), f4(m, mat + 48)]) },
                            mesh: if mesh == 0 { None } else { Some(PartMesh { lo: f4(m, mesh + 4), hi: f4(m, mesh + 0x10), flags: m.read_u32(mesh + 0x1c) }) },
                        }
                    })
                    .collect()
            })
            .collect();
        let groups = m.read_u32(W + 0x20);
        let collision: Vec<Vec<CollMesh>> = (0..ssf.coll_groups.len() as u32)
            .map(|gi| {
                let (first, n) = (m.read_u32(groups + 8 * gi), m.read_u32(groups + 8 * gi + 4));
                (0..n)
                    .map(|k| {
                        let rec = first + 0x14 * k;
                        let (nt, nv, t, v, pl) = (m.read_u32(rec), m.read_u32(rec + 4), m.read_u32(rec + 8), m.read_u32(rec + 12), m.read_u32(rec + 16));
                        CollMesh {
                            tris: (0..nt).map(|i| [0, 1, 2].map(|j| m.read_u32(t + 12 * i + 4 * j))).collect(),
                            verts: (0..nv).map(|i| f4(m, v + 16 * i)).collect(),
                            planes: (0..nt).map(|i| f4(m, pl + 16 * i)).collect(),
                        }
                    })
                    .collect()
            })
            .collect();
        let (width, height, per) = (m.read_u32(grid + 0x2c) as i32, m.read_u32(grid + 0x30) as i32, m.read_u32(grid + 0x48) as i32);
        let table = m.read_u32(grid + 0x4c);
        let cells: Vec<Option<Cell>> = (0..(width * height) as u32)
            .map(|k| {
                let tg = m.read_u32(table + 4 * k);
                if tg == 0 {
                    return None;
                }
                let nodes_at = m.read_u32(tg + 0x38);
                let nodes = (0..per as u32)
                    .map(|n| {
                        let p = nodes_at + 0x4c * n;
                        let counts: Vec<i16> = (0..7).map(|j| h(m, p + 0x24 + 2 * j)).collect();
                        let (pl, ol) = (m.read_u32(p + 0x34), m.read_u32(p + 0x38));
                        let n_obj = counts[1].max(counts[2]).max(0) as u32;
                        Node {
                            min: f3(m, p),
                            max: f3(m, p + 12),
                            patches: (0..counts[0].max(0) as u32).map(|j| ((m.read_u32(pl + 4 * j) - patch0) / 0x1c0) as usize).collect(),
                            objects: (0..n_obj).map(|j| ((m.read_u32(ol + 4 * j) - inst0) / 0x100) as usize).collect(),
                            n_objects: counts[1],
                            n_objects_showoff: counts[2],
                            blank: m.read_u32(p + 0x24) == 0 && m.read_u32(p + 0x28) == 0 && counts[4] == 0 && m.read_u32(p + 0x48) == 0,
                        }
                    })
                    .collect();
                Some(Cell { min: f3(m, tg), max: f3(m, tg + 12), nodes })
            })
            .collect();
        let trees = ssf.sphere_trees.iter().map(|g| g.iter().map(tricky_game::sphtree::SphereTree::new).collect()).collect();
        let world = World { trees, origin: [m.read_f32(grid + 4), m.read_f32(grid + 8)], size: m.read_f32(grid + 0x28), width, height, nodes_per_cell: per, cells, patches, objects, models, collision };

        // `World::from_course` (row E7c3a) builds the same course from the files; the objects' boxes and
        // controllers are this test's own, and models are numbered by first use here, by id there
        let ltg = tricky_data::ltg::Ltg::parse(&lb).unwrap();
        let ours = World::from_course(&pb, &pbd, &lb, &ltg, &ssf);
        assert_eq!((ours.origin, ours.size, ours.width, ours.height, ours.nodes_per_cell), (world.origin, world.size, world.width, world.height, world.nodes_per_cell), "{c}: grid");
        assert!(ours.cells == world.cells, "{c}: cells");
        assert!(ours.patches == world.patches, "{c}: patches");
        assert!(ours.collision == world.collision, "{c}: collision");
        assert!(ours.trees == world.trees, "{c}: sphere trees");
        assert_eq!(ours.objects.len(), world.objects.len());
        for (i, (a, b)) in ours.objects.iter().zip(&world.objects).enumerate() {
            assert_eq!((a.rows, a.flags, a.surface, a.mode, a.group), (b.rows, b.flags, b.surface, b.mode, b.group), "{c}: object {i}");
            assert_eq!(ours.models[a.model], world.models[b.model], "{c}: object {i}'s model");
        }

        // the casts' stubs: the patch cache and a patch's own test
        let salt = Rc::new(std::cell::Cell::new(0u32));
        let asked: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(vec![]));
        r.stub(0x25c750, |c, m| m.write_u32(c.gpr(6) as u32, 0xcac4e));
        let (s2, a2) = (salt.clone(), asked.clone());
        r.stub(0x25c7c8, move |c, m| {
            let i = ((c.gpr(5) as u32 - patch0) / 0x1c0) as usize;
            a2.borrow_mut().push(i);
            match fake_patch(s2.get(), i) {
                Some(hh) => {
                    for (k, v) in hh.pos.iter().enumerate() {
                        m.write_f32(c.gpr(7) as u32 + 4 * k as u32, *v);
                    }
                    for (k, v) in hh.normal.iter().enumerate() {
                        m.write_f32(c.gpr(8) as u32 + 4 * k as u32, *v);
                    }
                    m.write_f32(c.gpr(9) as u32, hh.t);
                    c.set_gpr(2, 1);
                }
                None => c.set_gpr(2, 0),
            }
        });
        for a in [0x25aff8, 0x25b878, 0x25bf48, 0x25d3d8, 0x25d7f0, 0x25d948, 0x25d9c8, 0x25d9e0, 0x25db18, 0x25dc40, 0x25dc48, 0x25ebd8, 0x25d7a8, 0x102f50, 0x178078] {
            r.allow(a);
        }
        r.stub(0x23b6b0, |c, m| {
            let g = |m: &mut Mem, a: u64| [0, 1, 2, 3].map(|k| m.read_f32(a as u32 + 4 * k));
            let v = tricky_game::worldline::tri_contains_point(g(m, c.gpr(4)), g(m, c.gpr(5)), g(m, c.gpr(6)), g(m, c.gpr(7)));
            c.set_gpr(2, v as u64);
        });
        let line = r.mem.alloc(0xa0);
        let from = r.mem.alloc(16);
        let to = r.mem.alloc(16);
        let res = r.mem.alloc(0x40);
        let last = r.mem.alloc(0x20);
        let mut env = Env { salt: 0, asked: vec![], controllers: controllers.clone() };
        let targets: Vec<usize> = (0..world.objects.len()).filter(|&o| world.objects[o].flags & 0x20 != 0 && world.objects[o].mode >= 1 && world.objects[o].mode <= 3).collect();
        for case in 0..3000u32 {
            // a line through something: an object's box, a patch's box, or a random spot
            let pick = g.next() % 3;
            let (lo, hi) = if pick == 0 && !targets.is_empty() {
                let o = &world.objects[targets[g.next() as usize % targets.len()]];
                (o.min, o.max)
            } else {
                let p = &world.patches[g.next() as usize % world.patches.len()];
                (p.min, p.max)
            };
            let k = |g: &mut Rng| (g.next() % 1000) as f32 / 1000.0;
            let at = [lo[0] + (hi[0] - lo[0]) * k(&mut g), lo[1] + (hi[1] - lo[1]) * k(&mut g), lo[2] + (hi[2] - lo[2]) * k(&mut g)];
            let up = g.pick(&[150.0f32, 400.0, 2000.0]);
            let side = [g.pick(&[0.0f32, 0.0, 300.0, -700.0]), g.pick(&[0.0f32, 0.0, 500.0, -200.0])];
            let fr = [at[0] - side[0], at[1] - side[1], at[2] + up, 1.0];
            let tt = [at[0] + side[0], at[1] + side[1], at[2] - up, 1.0];
            let target = g.pick(&[0.5f32, 0.0, 1.0, -1.0, 0.25]);
            let show = g.next() % 4 == 0;
            r.mem.write_u32(MODE, if show { g.pick(&[3u32, 5]) } else { g.pick(&[0u32, 1, 2, 4]) });
            salt.set(case.wrapping_mul(0x632b_e5ab) ^ c.len() as u32);
            env.salt = salt.get();
            for (a, v) in [(from, fr), (to, tt)] {
                for (j, x) in v.iter().enumerate() {
                    r.mem.write_f32(a + 4 * j as u32, *x);
                }
            }
            r.cpu.set_f(12, target);
            r.cpu.set_gpr(4, line as u64);
            r.cpu.set_gpr(5, from as u64);
            r.cpu.set_gpr(6, to as u64);
            r.cpu.set_gpr(7, 2);
            r.call(0x25d7f0).unwrap();
            let query = g.next() % 3 == 0;
            for o in (0..0x40).step_by(4) {
                r.mem.write_u32(res + o, 0xdead_0000 + o);
            }
            for o in (0..0x20).step_by(4) {
                r.mem.write_u32(last + o, 0x7777_7777);
            }
            asked.borrow_mut().clear();
            env.asked.clear();
            let with_last = g.next() % 2 == 0;
            r.cpu.set_gpr(4, W as u64);
            r.cpu.set_gpr(5, line as u64);
            r.cpu.set_gpr(6, res as u64);
            r.cpu.set_gpr(7, if with_last { last as u64 } else { 0 });
            r.call(if query { 0x25b878 } else { 0x25aff8 }).unwrap_or_else(|e| panic!("{c} case {case}: {e:?}"));
            let got = r.cpu.f(0);
            let mut q = WorldLine::new(target, fr, tt, 2);
            let mut lp = LastHit { cell: 0x7777, node: 0x7777, index: 0x7777, ..Default::default() };
            let (t, hit) = if query { world.query_instances(&mut q, show, &mut env) } else { world.ray_cast(&mut q, show, if with_last { Some(&mut lp) } else { None }, &mut env) };
            let what = format!("{c} case {case} ({}, show-off {show})", if query { "query" } else { "ray" });
            assert_eq!(got.to_bits(), t.to_bits(), "{what}: t");
            assert_eq!(*asked.borrow(), env.asked, "{what}: patches asked");
            let m = &mut r.mem;
            let word = |m: &mut Mem, k: u32| m.read_u32(res + 4 * k);
            match hit {
                None => assert_eq!(word(m, 0), 0xdead_0000, "{what}: nothing written"),
                Some(hh) => {
                    let tree = match hh.tree {
                        None => 0,
                        Some((gi, n)) => {
                            let t = m.read_u32(W + 0x18);
                            m.read_u32(t + 8 * gi as u32) + 0x150 * n as u32
                        }
                    };
                    let obj = hh.object.map_or(0, |o| inst0 + 0x100 * o as u32);
                    let theirs: Vec<u32> = [0, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14].iter().map(|&k| word(m, k)).collect();
                    let mut ours = vec![hh.kind];
                    ours.extend(hh.pos.map(f32::to_bits));
                    ours.extend(hh.normal.map(f32::to_bits));
                    ours.extend([tree, hh.surface as u32, obj]);
                    assert_eq!(theirs, ours, "{what}: hit record");
                    totals[if hh.object.is_some() { 1 } else { 0 }] += 1;
                }
            }
            if with_last && !query {
                let theirs = [h(m, last + 0x10), h(m, last + 0x12), h(m, last + 0x14)];
                assert_eq!(theirs, [lp.cell, lp.node, lp.index], "{what}: last patch");
            }
            totals[2] += 1;
            totals[3] += env.asked.len();
        }
    }
    eprintln!("{} patch hits, {} object hits in {} casts ({} patch tests)", totals[0], totals[1], totals[2], totals[3]);
}

//! The world grid (board row E3e) against the game: every course's `.ltg` (TRICKY_LTG_DIR, extracted from the
//! player's disc with `disc_load "|data/models/<c>.ltg" <c>.ltg data/models/<c>.big`; skipped without it or
//! TRICKY_ELF) resolved by `cCourseResolve_Common::ResolveTGroupHeader` 0x252470 in the function runner (which walks
//! every TGroup and INode), the `.pbd` lookups stubbed to tag each index with its kind; then every pointer and list
//! it leaves compared with `tricky_data::ltg`, and `WorldGrid_CellAt` asked about every INode.

use r5900::Runner;
use tricky_data::ltg::Ltg;

const COURSES: [&str; 12] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska", "ssxfe"];

/// What the stubbed lookups return for index `i` of kind `k` (patch 1, instance 2, particle 3, light 4, spline 5).
fn tag(k: u32, i: u32) -> u32 {
    0x8000_0000 | k << 24 | (i & 0xff_ffff)
}

#[test]
fn ltg_files_resolve_as_the_game_resolves_them() {
    let Ok(dir) = std::env::var("TRICKY_LTG_DIR") else { return };
    for c in COURSES {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.ltg")) else { continue };
        let ltg = Ltg::parse(&bytes).unwrap_or_else(|| panic!("{c}: parse"));
        let Some(mut r) = Runner::from_env() else { return };
        for (a, k) in [(0x2549f8u32, 1u32), (0x254a28, 2), (0x254a50, 3), (0x254a80, 4), (0x254b50, 5)] {
            r.stub(a, move |cpu, _| {
                let i = cpu.gpr(5) as u32;
                cpu.set_gpr(2, tag(k, i) as u64);
            });
        }
        r.stub(0x255310, |_, _| {});
        r.stub(0x2f7aa8, |_, _| {});
        for a in [0x252748, 0x2527e0, 0x252b88] {
            r.allow(a);
        }
        let file = r.mem.alloc(bytes.len() as u32 + 16);
        r.mem.write(file, &bytes).unwrap();
        // the world object: its vtable at +0x10 (cCourseResolve's)
        let world = r.mem.alloc(0x40);
        r.mem.write_u32(world + 0x10, 0x388350);
        r.cpu.set_gpr(4, world as u64);
        r.cpu.set_gpr(5, file as u64);
        let hdr = r.mem.alloc(8);
        r.mem.write(hdr, &bytes[..4]).unwrap();
        r.cpu.set_gpr(6, hdr as u64);
        r.call(0x252470).unwrap_or_else(|e| panic!("{c}: {e:?}"));
        assert_eq!(r.cpu.gpr(2), 1, "{c}: resolved");
        let m = &mut r.mem;
        assert_eq!((m.read_u32(file + 0x4c), m.read_u32(file + 0x50)), (file + ltg.entry_offset, file + ltg.tgroup_offset), "{c}: header");
        assert_eq!(ltg.num, ltg.width * ltg.height, "{c}: grid");
        assert_eq!(ltg.tgroups.iter().filter(|t| t.is_some()).count() as i32, ltg.used, "{c}: used");
        let table = file + ltg.entry_offset;
        for (k, t) in ltg.tgroups.iter().enumerate() {
            let at = m.read_u32(table + 4 * k as u32);
            let Some((off, t)) = t else {
                assert_eq!(at, 0, "{c}: tgroup {k}");
                continue;
            };
            let base = file + off;
            assert_eq!(at, base, "{c}: tgroup {k}");
            assert_eq!(t.inodes_offset, 0x54, "{c}: tgroup {k} inodes");
            assert_eq!(m.read_u32(base + 0x38), base + 0x54, "{c}: tgroup {k} inodes");
            let lists = [(&t.patches, 1u32), (&t.instances, 2), (&t.splines, 5), (&t.lights_inside, 4), (&t.lights_crossing, 4), (&t.particles, 3)];
            for (j, (l, kind)) in lists.iter().enumerate() {
                let p = m.read_u32(base + 0x3c + 4 * j as u32);
                assert_eq!(p, if t.offsets[j] == 0 { 0 } else { base + t.offsets[j] }, "{c}: tgroup {k} list {j}");
                for (n, idx) in l.iter().enumerate() {
                    assert_eq!(m.read_u32(p + 4 * n as u32), tag(*kind, *idx), "{c}: tgroup {k} list {j} entry {n}");
                }
            }
            for (n, inode) in t.inodes.iter().enumerate() {
                let ip = base + 0x54 + 0x4c * n as u32;
                for (j, o) in inode.offsets.iter().enumerate() {
                    assert_eq!(m.read_u32(ip + 0x34 + 4 * j as u32), if *o == 0 { 0 } else { base + o }, "{c}: tgroup {k} inode {n} offset {j}");
                }
            }
        }
        // WorldGrid_CellAt(&world grid, x, y) for every INode, and a few outside
        let gp = r.mem.alloc(4);
        r.mem.write_u32(gp, file);
        let n = ltg.inode_wide;
        let mut asked = 0;
        for y in -1..=ltg.height * n {
            for x in -1..=ltg.width * n {
                if (x + y) % 3 != 0 && x >= 0 && y >= 0 && x < ltg.width * n && y < ltg.height * n {
                    continue;
                }
                r.cpu.set_gpr(4, gp as u64);
                r.cpu.set_gpr(5, x as i64 as u64);
                r.cpu.set_gpr(6, y as i64 as u64);
                r.call(0x260ba8).unwrap();
                let got = r.cpu.gpr(2) as u32;
                let want = ltg.inode_at(x, y).map(|i| {
                    let (tx, ty) = (x / n, y / n);
                    let (off, t) = ltg.tgroups[(ty * ltg.width + tx) as usize].as_ref().unwrap();
                    let k = t.inodes.iter().position(|j| std::ptr::eq(j, i)).unwrap() as u32;
                    file + off + 0x54 + 0x4c * k
                });
                assert_eq!(got, want.unwrap_or(0), "{c}: cell {x}, {y}");
                asked += 1;
            }
        }
        eprintln!("{c}: {}×{} TGroups ({} used), {} cells asked", ltg.width, ltg.height, ltg.used, asked);
    }
}

#[test]
fn adl_files_bind_as_the_game_binds_them() {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use tricky_data::adl::Adl;
    let Ok(dir) = std::env::var("TRICKY_LTG_DIR") else { return };
    for c in COURSES {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.adl")) else { continue };
        let adl = Adl::parse(&bytes).unwrap_or_else(|| panic!("{c}: parse"));
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        r.allow(0x22acc0);
        r.allow(0x137140);
        // the instances: every other hash found, each at its own place
        let insts = r.mem.alloc(0x100 * 4096);
        let found: Rc<RefCell<HashMap<u32, u32>>> = Rc::new(RefCell::new(HashMap::new()));
        let mut k = 0u32;
        for (h, _) in &adl.entries {
            if h % 2 == 0 && !found.borrow().contains_key(h) && k < 4096 {
                let at = insts + 0x100 * k;
                found.borrow_mut().insert(*h, at);
                k += 1;
            }
        }
        for (i, at) in found.borrow().values().enumerate() {
            let p = [i as f32 * 37.5 - 900.0, 1200.0 - i as f32 * 11.0, (i % 7) as f32 * 300.0];
            for (j, v) in p.iter().enumerate() {
                r.mem.write_f32(at + 0x30 + 4 * j as u32, *v);
            }
        }
        let fd = found.clone();
        r.stub(0x2550c8, move |cpu, _| {
            let h = cpu.gpr(5) as u32;
            cpu.set_gpr(2, fd.borrow().get(&h).copied().unwrap_or(0) as u64);
        });
        let file = r.mem.alloc(bytes.len() as u32 + 16);
        r.mem.write(file, &bytes).unwrap();
        let obj = r.mem.alloc(0x40);
        r.mem.write_u32(obj, file);
        r.mem.write_u32(obj + 4, bytes.len() as u32);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x22a5f0).unwrap();
        assert_eq!(r.cpu.gpr(2), 1, "{c}: bound");
        assert_eq!((r.mem.read_u32(obj + 8), r.mem.read_u32(obj + 12)), (adl.entries.len() as u32, file + 0xc), "{c}");
        let mut boxes = 0;
        for (i, (h, off)) in adl.entries.iter().enumerate() {
            let e = file + 0xc + 8 * i as u32;
            let inst = found.borrow().get(h).copied().unwrap_or(0);
            assert_eq!((r.mem.read_u32(e), r.mem.read_u32(e + 4)), (inst, file + off), "{c}: entry {i}");
            if inst == 0 {
                continue;
            }
            assert_eq!(r.mem.read_u32(inst + 0xf4) >= file, true, "{c}: entry {i} back link");
            let out = r.mem.alloc(0x20);
            r.cpu.set_gpr(4, 0);
            r.cpu.set_gpr(5, e as u64);
            r.cpu.set_gpr(6, out as u64);
            r.cpu.set_gpr(7, (out + 0x10) as u64);
            r.call(0x22aaa0).unwrap();
            let at = [0, 1, 2].map(|j| r.mem.read_f32(inst + 0x30 + 4 * j));
            let (lo, hi) = adl.zone(*off).unwrap().bounds(at);
            let got = ([0, 1, 2].map(|j| r.mem.read_u32(out + 4 * j)), [0, 1, 2].map(|j| r.mem.read_u32(out + 0x10 + 4 * j)));
            assert_eq!(got, (lo.map(f32::to_bits), hi.map(f32::to_bits)), "{c}: entry {i} bounds");
            boxes += 1;
        }
        eprintln!("{c}: {} links, {} zones, {} boxes compared", adl.entries.len(), adl.zones.len(), boxes);
    }
}

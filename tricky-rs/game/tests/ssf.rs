//! The `.ssf` reader (board row E3c1) against the game: every course's file (TRICKY_SSF_DIR, extracted from the
//! player's disc with `disc_load "|data/models/<c>.ssf" <c>.ssf data/models/<c>.big`; skipped without it or
//! TRICKY_ELF) loaded by `Course_LoadSphereTrees` 0x25f598 in the function runner, and what it leaves in the objects,
//! spline segments, collision tables and fixed-up scripts compared with `tricky_data::ssf`; then `World_FindScript`
//! and `Instance_GetEventScript` asked about every function and object.

use std::cell::Cell;
use std::rc::Rc;

use r5900::Runner;
use tricky_data::ssf::{Ssf, SsfError};

const COURSES: [&str; 12] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska", "ssxfe"];
const OBJ: u32 = 0x100;
const SEG: u32 = 0x40;

struct Loaded {
    r: Runner,
    course: u32,
    file: u32,
    objects: u32,
    models: u32,
    segs: u32,
}

fn load(bytes: &[u8], objects: u32, segments: u32) -> Option<Loaded> {
    let mut r = Runner::from_env()?;
    let file = r.mem.alloc(bytes.len() as u32 + 16);
    r.mem.write(file, bytes).unwrap();
    let course = r.mem.alloc(0x40);
    let model_data = r.mem.alloc(0x20);
    r.mem.write_u32(course + 4, model_data);
    r.mem.write_u32(model_data + 0xc, objects);
    let objs = r.mem.alloc(objects.max(1) * OBJ);
    let models = r.mem.alloc(objects.max(1) * 0x20);
    for i in 0..objects {
        r.mem.write_u32(objs + i * OBJ + 0xc0, models + i * 0x20);
    }
    let segs = r.mem.alloc(segments.max(1) * SEG);
    let heap = Rc::new(Cell::new(r.mem.alloc(0x40_0000)));
    r.stub(0x2cc808, |_, _| {});
    r.stub(0x254a28, move |c, _| {
        let i = c.gpr(5) as u32;
        c.set_gpr(2, (objs + i * OBJ) as u64);
    });
    r.stub(0x254b20, move |c, _| {
        let i = c.gpr(5) as u32;
        c.set_gpr(2, (segs + i * SEG) as u64);
    });
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
    r.stub(0x239ba8, |_, _| {});
    r.stub(0x23b520, |_, _| {});
    // SphTree_Load runs as the game's (row E3c2); EAMem_Copy is a plain copy
    r.allow(0x239340);
    r.stub(0x2ce114, |c, m| {
        let mut b = vec![0u8; c.gpr(6) as usize];
        m.read(c.gpr(5) as u32, &mut b);
        m.write(c.gpr(4) as u32, &b).unwrap();
    });
    r.cpu.set_gpr(4, course as u64);
    let path = r.mem.alloc(16);
    r.cpu.set_gpr(5, path as u64);
    r.call(0x25f598).unwrap();
    Some(Loaded { r, course, file, objects: objs, models, segs })
}

fn ops_at(m: &mut r5900::Mem, mut p: u32, count: u32) -> Vec<(u32, u32, Vec<u32>)> {
    let mut v = vec![];
    for _ in 0..count {
        let (op, size) = (m.read_u32(p), m.read_u32(p + 4));
        v.push((op, size, (0..(size / 4).saturating_sub(2)).map(|k| m.read_u32(p + 8 + 4 * k)).collect()));
        p += size;
    }
    v
}

fn cstr(m: &mut r5900::Mem, at: u32) -> String {
    let mut b = [0u8; 16];
    m.read(at, &mut b);
    String::from_utf8_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(16)]).to_string()
}

#[test]
fn ssf_files_load_as_the_game_loads_them() {
    let Ok(dir) = std::env::var("TRICKY_SSF_DIR") else { return };
    if std::env::var("TRICKY_ELF").is_err() {
        return;
    }
    for c in COURSES {
        let bytes = std::fs::read(format!("{dir}/{c}.ssf")).expect("ssf");
        let objects = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap());
        let ssf = Ssf::parse(&bytes, objects).unwrap_or_else(|e| panic!("{c}: {e:?}"));
        assert_eq!(Ssf::parse(&bytes, objects + 1), Err(SsfError::Objects(objects)));
        let Some(mut l) = load(&bytes, objects, ssf.spline_segments.len() as u32) else { return };
        let m = &mut l.r.mem;
        assert_eq!(m.read_u32(l.course + 0x14), l.file, "{c}: the file kept");
        let phys = m.read_u32(l.file + 0x38);
        assert_eq!(phys, l.file + u32::from_le_bytes(bytes[0x38..0x3c].try_into().unwrap()));
        for (i, p) in ssf.object_physics.iter().enumerate() {
            let o = l.objects + i as u32 * OBJ;
            let rec = ssf.physics[*p as usize];
            assert_eq!((m.read_u32(o + 0xe8), m.read_u32(o + 0xec)), (rec.flags(), phys + p * 0x18), "{c}: object {i}");
            let marked = m.read_u32(l.models + i as u32 * 0x20 + 0x10) & 0x10 != 0;
            assert_eq!(marked, (rec.flags() >> 16) & 0x2000 != 0, "{c}: object {i} model");
        }
        for (i, s) in ssf.spline_segments.iter().enumerate() {
            let at = l.segs + i as u32 * SEG;
            assert_eq!((m.read_u32(at + 0x18), m.read_u32(at + 0x24)), (Ssf::segment_x18(s[0]), s[1]), "{c}: segment {i}");
        }
        // effect headers and functions, fixed up to addresses
        let eff = m.read_u32(l.file + 0x28);
        for (i, e) in ssf.effects.iter().enumerate() {
            let (n, p) = (m.read_u32(eff + 8 * i as u32), m.read_u32(eff + 8 * i as u32 + 4));
            let want: Vec<_> = e.iter().map(|o| (o.op, o.size, o.args.clone())).collect();
            assert_eq!(ops_at(m, p, n), want, "{c}: effect {i}");
        }
        let fun = m.read_u32(l.file + 0x30);
        for (i, f) in ssf.functions.iter().enumerate() {
            let at = fun + 0x18 * i as u32;
            let (n, p) = (m.read_u32(at), m.read_u32(at + 4));
            let want: Vec<_> = f.ops.iter().map(|o| (o.op, o.size, o.args.clone())).collect();
            assert_eq!((cstr(m, at + 8), ops_at(m, p, n)), (f.name.clone(), want), "{c}: function {i}");
        }
        // the collision tables the loader builds: per group (meshes, count), per mesh (tris, verts, three pointers)
        let (groups, meshes) = (m.read_u32(l.course + 0x20), m.read_u32(l.course + 0x24));
        let mut k = 0;
        for (g, list) in ssf.collision.iter().enumerate() {
            if ssf.coll_groups.is_empty() {
                break;
            }
            assert_eq!((m.read_u32(groups + 8 * g as u32), m.read_u32(groups + 8 * g as u32 + 4)), (meshes + 0x14 * k, list.len() as u32), "{c}: collision group {g}");
            for (j, mesh) in list.iter().enumerate() {
                let rec = meshes + 0x14 * k;
                let (n, nv, tp, vp, pp) = (m.read_u32(rec), m.read_u32(rec + 4), m.read_u32(rec + 8), m.read_u32(rec + 12), m.read_u32(rec + 16));
                assert_eq!((n as usize, nv as usize), (mesh.tris.len(), mesh.verts.len()), "{c}: mesh {g}.{j}");
                for (t, tri) in mesh.tris.iter().enumerate() {
                    let a = tp + 12 * t as u32;
                    assert_eq!([m.read_u32(a), m.read_u32(a + 4), m.read_u32(a + 8)], *tri, "{c}: mesh {g}.{j} tri {t}");
                }
                for (q, v) in mesh.verts.iter().enumerate() {
                    let a = vp + 16 * q as u32;
                    assert_eq!([0, 1, 2, 3].map(|w| m.read_u32(a + 4 * w)), v.map(f32::to_bits), "{c}: mesh {g}.{j} vert {q}");
                }
                for (q, v) in mesh.planes.iter().enumerate() {
                    let a = pp + 16 * q as u32;
                    assert_eq!([0, 1, 2, 3].map(|w| m.read_u32(a + 4 * w)), v.map(f32::to_bits), "{c}: mesh {g}.{j} plane {q}");
                }
                k += 1;
            }
        }
        // the sphere trees: per group (first tree, count) at course+0x18, the trees (0x150 bytes) at course+0x1c, each
        // with its header copied to +0xd0 and pointers into the file
        let (gtab, tbase) = (m.read_u32(l.course + 0x18), m.read_u32(l.course + 0x1c));
        let mut k = 0u32;
        for (gi, (g, trees)) in ssf.sphere_groups.iter().zip(&ssf.sphere_trees).enumerate() {
            assert_eq!(trees.iter().map(|t| t.size()).sum::<usize>(), g.bytes as usize, "{c}: sphere group {gi} bytes");
            assert_eq!((m.read_u32(gtab + 8 * gi as u32), m.read_u32(gtab + 8 * gi as u32 + 4)), (tbase + 0x150 * k, g.count), "{c}: sphere group {gi}");
            let mut at = l.file + g.offset;
            for (ti, t) in trees.iter().enumerate() {
                let n = tbase + 0x150 * k;
                let h = n + 0xd0;
                let w = |m: &mut r5900::Mem, o: u32| m.read_u32(h + o);
                assert_eq!(m.read_u32(n + 0xa0), h, "{c}: tree {gi}.{ti}");
                assert_eq!([w(m, 0), w(m, 4), w(m, 8), w(m, 0xc), w(m, 0x10)], [k, 0, t.flag, t.depth, 1], "{c}: tree {gi}.{ti} header");
                let fl = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
                let rd = |m: &mut r5900::Mem, o: u32, n: u32| (0..n).map(|i| m.read_u32(h + o + 4 * i)).collect::<Vec<u32>>();
                assert_eq!(rd(m, 0x14, 3), fl(&t.centre), "{c}: tree {gi}.{ti} centre");
                assert_eq!(rd(m, 0x2c, 3), fl(&t.mass_centre), "{c}: tree {gi}.{ti} mass centre");
                assert_eq!(rd(m, 0x38, 9), fl(&t.inertia), "{c}: tree {gi}.{ti} inertia");
                assert_eq!(rd(m, 0x5c, 9), fl(&t.inv_inertia), "{c}: tree {gi}.{ti} inverse");
                let levels = w(m, 0x20);
                assert_eq!(levels, at + 0x70, "{c}: tree {gi}.{ti} levels");
                for (li, lv) in t.levels.iter().enumerate() {
                    let a = levels + 12 * li as u32;
                    assert_eq!((m.read_u32(a), m.read_u32(a + 4), m.read_u32(a + 8) as i32), (lv.0.to_bits(), lv.1.to_bits(), lv.2), "{c}: tree {gi}.{ti} level {li}");
                }
                let data = w(m, 0x24);
                assert_eq!(data, levels + 12 * t.levels.len() as u32, "{c}: tree {gi}.{ti} data");
                assert_eq!(w(m, 0x28), if t.flag == 0 { data } else { 0 }, "{c}: tree {gi}.{ti} masks");
                let mut db = vec![0u8; t.data.len()];
                m.read(data, &mut db);
                assert_eq!(db, t.data, "{c}: tree {gi}.{ti} data bytes");
                assert_eq!([0x90u32, 0x94, 0x98, 0x9c].map(|o| m.read_u32(n + o)), [t.centre[0].to_bits(), t.centre[1].to_bits(), t.centre[2].to_bits(), 0x3f80_0000], "{c}: tree {gi}.{ti}");
                at += t.size() as u32;
                k += 1;
            }
        }
        // the game's lookups on what it loaded
        let name = l.r.mem.alloc(32);
        let mut asks: Vec<String> = ssf.functions.iter().map(|f| f.name.to_ascii_uppercase()).collect();
        asks.extend(["RaceMode", "nosuch", "ShowoffMode", "FreerideMode"].map(String::from));
        for a in asks {
            let mut b = a.clone().into_bytes();
            b.push(0);
            l.r.mem.write(name, &b).unwrap();
            l.r.cpu.set_gpr(4, l.course as u64);
            l.r.cpu.set_gpr(5, name as u64);
            l.r.allow(0x2fcc98);
            l.r.call(0x260dd8).unwrap_or_else(|e| panic!("{c}: {e:?}"));
            assert_eq!(l.r.cpu.gpr(2) as i32, ssf.find_function(&a), "{c}: {a}");
        }
        for i in 0..ssf.object_physics.len() {
            for ev in 0..7 {
                l.r.cpu.set_gpr(4, l.course as u64);
                l.r.cpu.set_gpr(5, (l.objects + i as u32 * OBJ) as u64);
                l.r.cpu.set_gpr(6, ev as u64);
                l.r.call(0x25fe78).unwrap();
                assert_eq!(l.r.cpu.gpr(2) as i32, ssf.event_script(i, ev), "{c}: object {i} event {ev}");
            }
        }
        eprintln!("{c}: {} objects, {} effects, {} functions, {} collision groups", objects, ssf.effects.len(), ssf.functions.len(), ssf.collision.len());
    }
}

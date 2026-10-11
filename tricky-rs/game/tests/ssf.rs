//! The `.ssf` reader (board row E3c1) against the game: every course's file (TRICKY_SSF_DIR, extracted from the
//! player's disc with `disc_load "|data/models/<c>.ssf" <c>.ssf data/models/<c>.big`; skipped without it or
//! TRICKY_ELF) loaded by `Course_LoadSphereTrees` 0x25f598 in the function runner, and what it leaves in the objects,
//! spline segments, collision tables and fixed-up scripts compared with `tricky_data::ssf`; then `World_FindScript`
//! and `Instance_GetEventScript` asked about every function and object.

use std::cell::{Cell, RefCell};
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
    trees: Rc<RefCell<Vec<(u32, u32)>>>,
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
    let trees = Rc::new(RefCell::new(vec![]));
    let t = trees.clone();
    // SphTree_Load(tree, data, index) returns the bytes it read: a group's first tree is all this row checks
    r.stub(0x239340, move |c, _| {
        t.borrow_mut().push((c.gpr(5) as u32, c.gpr(6) as u32));
        c.set_gpr(2, 0);
    });
    r.cpu.set_gpr(4, course as u64);
    let path = r.mem.alloc(16);
    r.cpu.set_gpr(5, path as u64);
    r.call(0x25f598).unwrap();
    Some(Loaded { r, course, file, objects: objs, models, segs, trees })
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
        // each sphere-tree group's first tree is read where the group says
        let firsts: Vec<u32> = {
            let t = l.trees.borrow();
            let mut at = 0;
            ssf.sphere_groups.iter().filter_map(|g| { let f = t.get(at).map(|x| x.0); at += g.count as usize; f }).collect()
        };
        let want: Vec<u32> = ssf.sphere_groups.iter().filter(|g| g.count > 0).map(|g| l.file + g.offset).collect();
        assert_eq!(firsts, want, "{c}: sphere groups");
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

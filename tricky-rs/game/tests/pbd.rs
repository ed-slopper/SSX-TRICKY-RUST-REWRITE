//! The `.pbd` reader (board row E3a) against the game: every course's file (TRICKY_PBD_DIR, extracted from the
//! player's disc with `disc_load "|data/models/<c>.pbd" <c>.pbd data/models/<c>.big`; skipped without it or
//! TRICKY_ELF; the skies `<c>_sky.pbd` from the same archives) resolved by `cCourseResolve_PS2::ResolveDataHeader` 0x255630 in the function runner, every resolver
//! and getter the game's own; then every pointer it writes into the file and the world's two tables compared with
//! what `tricky_data::pbd` says they should be.

use r5900::Runner;
use tricky_data::pbd::{Animation, Pbd, INSTANCE, LIGHT, PATCH, SEGMENT, SPLINE};

const COURSES: [&str; 23] = [
    "gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska", "ssxfe", "gari_sky", "snow_sky", "elysium_sky", "mesa_sky", "merquer_sky", "aloha_sky",
    "pipe_sky", "untrack_sky", "megaple_sky", "trick_sky", "alaska_sky",
];
const IDENTITY: u32 = 0x346440;

fn check_anim(m: &mut r5900::Mem, f: u32, a: &Animation, what: &str) -> usize {
    let at = f + a.offset;
    let table = m.read_u32(at + 0x20);
    assert_eq!(table, at + a.head[8], "{what}: channel table");
    for (k, (c, w4)) in a.channels.iter().enumerate() {
        assert_eq!(m.read_u32(table + 4 * k as u32), f + c, "{what}: channel {k}");
        assert_eq!(m.read_u32(f + c + 4), f + c + w4, "{what}: channel {k} data");
    }
    a.channels.len()
}

#[test]
fn pbd_files_resolve_as_the_game_resolves_them() {
    let Ok(dir) = std::env::var("TRICKY_PBD_DIR") else { return };
    for c in COURSES {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.pbd")) else { continue };
        let p = Pbd::parse(&bytes).unwrap_or_else(|| panic!("{c}: parse"));
        let Some(mut r) = Runner::from_env() else { return };
        r.stub(0x255310, |_, _| {});
        r.stub(0x2f7aa8, |_, _| {});
        for a in [
            0x253400, 0x253540, 0x2535b0, 0x253630, 0x2537d8, 0x2539b0, 0x255c20, 0x253fb0, 0x254078, 0x254120, 0x2534c8, 0x253b60, 0x253bd0, 0x253c50, 0x253d30, 0x255cc0, 0x254160, 0x2541e8,
            0x2542a8, 0x254318, 0x254398, 0x254600, 0x254678, 0x2546a8, 0x254e58, 0x254e98, 0x2522e0, 0x252318, 0x2549f8, 0x254a28, 0x254a50, 0x254a80, 0x254af0, 0x254b20, 0x254b50, 0x254b78,
        ] {
            r.allow(a);
        }
        let f = r.mem.alloc(bytes.len() as u32 + 16);
        r.mem.write(f, &bytes).unwrap();
        let world = r.mem.alloc(0x40);
        let blocks = r.mem.alloc(16000);
        let flips = r.mem.alloc(4000);
        r.mem.write_u32(world + 4, f);
        r.mem.write_u32(world + 8, blocks);
        r.mem.write_u32(world + 0xc, flips);
        r.mem.write_u32(world + 0x10, 0x3a7d50);
        r.cpu.set_gpr(4, world as u64);
        r.cpu.set_gpr(5, f as u64);
        r.call(0x255630).unwrap_or_else(|e| panic!("{c}: {e:?}"));
        assert_eq!(r.cpu.gpr(2), 1, "{c}: resolved");
        let h = p.header;
        let (cnt, off) = (|a| h.count(a), |a| h.offset(a));
        let m = &mut r.mem;
        // the header's offsets become pointers (the VIF block's only with models)
        for a in (0x40..=0x84).step_by(4) {
            let o = off(a);
            // +0x64 is resolved twice by the game (before the flip table is built and again at the end)
            let want = if o == 0 || (a == 0x84 && off(0x6c) == 0) { o } else if a == 0x64 { f.wrapping_add(f).wrapping_add(o) } else { f + o };
            assert_eq!(m.read_u32(f + a as u32), want, "{c}: header +{a:#x}");
        }
        // the world's tables: material blocks and texture flips
        let mut at = off(0x54);
        for (k, b) in p.material_blocks.iter().enumerate() {
            assert_eq!(m.read_u32(blocks + 4 * k as u32), f + at, "{c}: block {k}");
            at += 4 * (b.len() as u32 + 1);
        }
        let mut at = off(0x64);
        for (k, b) in p.texture_flips.iter().enumerate() {
            assert_eq!(m.read_u32(flips + 4 * k as u32), f + at, "{c}: flip {k}");
            at += 4 * (b.len() as u32 + 1);
        }
        let block = |i: u32| if (i as usize) < p.material_blocks.len() { Some(blocks + 4 * i) } else { None };
        let model_ptr = |id: u32| f + off(0x6c) + p.model_pointers[id as usize];
        let get = |count: u32, base: u32, size: usize, i: u32| if (i as i32) < count as i32 { f + base + (size as u32).wrapping_mul(i) } else { 0 };
        // materials: +0x44 a texture flip
        for (k, mat) in p.materials.iter().enumerate() {
            let want = if mat[17] as i32 == -1 { 0 } else { m.read_u32(flips + 4 * mat[17]) };
            assert_eq!(m.read_u32(f + off(0x50) + 0x48 * k as u32 + 0x44), want, "{c}: material {k}");
        }
        // material block entries: materials
        let mut at = off(0x54);
        for (k, b) in p.material_blocks.iter().enumerate() {
            for (j, i) in b.iter().enumerate() {
                assert_eq!(m.read_u32(f + at + 4 + 4 * j as u32), get(cnt(0x14), off(0x50), 0x48, *i), "{c}: block {k} entry {j}");
            }
            at += 4 * (b.len() as u32 + 1);
        }
        // model pointers, models, their nodes, meshes, contexts, animations
        for (k, ptr) in p.model_pointers.iter().enumerate() {
            assert_eq!(m.read_u32(f + off(0x68) + 4 * k as u32), f + off(0x6c) + ptr, "{c}: model pointer {k}");
        }
        let (mut meshes, mut contexts, mut channels) = (0, 0, 0);
        for (k, md) in p.models.iter().enumerate() {
            let at = f + md.offset;
            assert_eq!(m.read_u32(at + 8), at + md.head[2], "{c}: model {k} hierarchy");
            let want = block(md.head[3]).map(|b| m.read_u32(b)).unwrap_or(0);
            assert_eq!(m.read_u32(at + 0xc), want, "{c}: model {k} block");
            for (j, n) in md.nodes.iter().enumerate() {
                let e = f + n.offset;
                for l in 1..4 {
                    let want = if n.words[l] == 0 { 0 } else { e + n.words[l] };
                    assert_eq!(m.read_u32(e + 4 * l as u32), want, "{c}: model {k} node {j} lod {l}");
                }
                let want = if n.words[4] == 0 { 0 } else { e + n.words[4] };
                assert_eq!(m.read_u32(e + 16), want, "{c}: model {k} node {j} animation");
                let want = if n.words[5] as i32 == -1 { IDENTITY } else { e + n.words[5] };
                assert_eq!(m.read_u32(e + 20), want, "{c}: model {k} node {j} matrix");
                if let Some(a) = &n.animation {
                    channels += check_anim(m, f, a, &format!("{c}: model {k} node {j}"));
                }
            }
            for ms in &md.meshes {
                let at = f + ms.offset;
                let t = m.read_u32(at + 0x28);
                assert_eq!(t, at + ms.head[10], "{c}: model {k} mesh {:#x}", ms.offset);
                for (j, cx) in ms.contexts.iter().enumerate() {
                    assert_eq!(m.read_u32(t + 4 * j as u32), f + cx.offset, "{c}: model {k} mesh {:#x} context {j}", ms.offset);
                    assert_eq!(m.read_u32(f + cx.offset + 0xc), cx.words[3].wrapping_add(f + off(0x84)), "{c}: context VIF");
                    contexts += 1;
                }
                meshes += 1;
            }
        }
        // particle models
        for (k, ptr) in p.particle_model_pointers.iter().enumerate() {
            assert_eq!(m.read_u32(f + off(0x70) + 4 * k as u32), f + off(0x74) + ptr, "{c}: particle model pointer {k}");
        }
        for (k, pm) in p.particle_models.iter().enumerate() {
            let at = f + pm.offset;
            assert_eq!(m.read_u32(at + 8), at + pm.head[2], "{c}: particle model {k}");
            let h0 = at + pm.head[2];
            for (j, (w, mesh, anim)) in pm.nodes.iter().enumerate() {
                let e = h0 + 16 * j as u32;
                assert_eq!(m.read_u32(e + 4), if w[1] == 0 { 0 } else { e + w[1] }, "{c}: particle model {k} node {j} mesh");
                assert_eq!(m.read_u32(e + 8), if w[2] == 0 { 0 } else { e + w[2] }, "{c}: particle model {k} node {j} animation");
                assert_eq!(m.read_u32(e + 12), if w[3] as i32 == -1 { IDENTITY } else { e + w[3] }, "{c}: particle model {k} node {j} matrix");
                if let Some(ms) = mesh {
                    assert_eq!(m.read_u32(f + ms.offset + 0x24), f + ms.offset + ms.words[9], "{c}: particle mesh");
                }
                if let Some(a) = anim {
                    channels += check_anim(m, f, a, &format!("{c}: particle model {k} node {j}"));
                }
            }
        }
        // instances
        for (k, ins) in p.instances.iter().enumerate() {
            let at = f + off(0x48) + (INSTANCE * k) as u32;
            let id = ins[0xc0 / 4];
            let want = if (id as i32) < 4000 && off(0x68) != 0 { model_ptr(id) } else { 0 };
            assert_eq!(m.read_u32(at + 0xc0), want, "{c}: instance {k} model");
            for o in [0xc4u32, 0xc8] {
                let i = ins[o as usize / 4];
                let want = if (i as i32) < 0 { 0 } else { get(cnt(0xc), off(0x48), INSTANCE, i) };
                assert_eq!(m.read_u32(at + o), want, "{c}: instance {k} +{o:#x}");
            }
            assert_eq!(m.read_u32(at + 0xf0), m.read_u32(blocks + 4 * ins[0xf0 / 4]), "{c}: instance {k} block");
            assert_eq!(m.read_u32(at + 0xf4), 0, "{c}: instance {k}");
        }
        // particle instances
        for (k, pi) in p.particle_instances.iter().enumerate() {
            let at = f + off(0x4c) + 0x70 * k as u32;
            let id = pi[0x40 / 4];
            let want = if (id as i32) < 4000 && off(0x70) != 0 { f + off(0x74) + p.particle_model_pointers[id as usize] } else { 0 };
            assert_eq!((m.read_u32(at + 0x40), m.read_u32(at + 0x64)), (want, 0), "{c}: particle instance {k}");
        }
        // splines (0x28) and spline segments (0x80)
        let seg = |i: u32| get(cnt(0x24), off(0x60), SEGMENT, i);
        let spl = |i: u32| get(cnt(0x20), off(0x5c), SPLINE, i);
        for (k, s) in p.splines.iter().enumerate() {
            assert_eq!(m.read_u32(f + off(0x5c) + (SPLINE * k) as u32 + 0x20), seg(s[8]), "{c}: spline {k}");
        }
        for (k, s) in p.segments.iter().enumerate() {
            let at = f + off(0x60) + (SEGMENT * k) as u32;
            for (o, w) in [(0x50u32, s[20]), (0x54, s[21])] {
                assert_eq!(m.read_u32(at + o), if w as i32 == -1 { 0 } else { seg(w) }, "{c}: segment {k} +{o:#x}");
            }
            assert_eq!(m.read_u32(at + 0x58), spl(s[22]), "{c}: segment {k} spline");
        }
        // cameras
        for (k, ptr) in p.camera_pointers.iter().enumerate() {
            assert_eq!(m.read_u32(f + off(0x78) + 4 * k as u32), f + off(0x7c) + ptr, "{c}: camera pointer {k}");
        }
        for (k, cam) in p.cameras.iter().enumerate() {
            let at = f + cam.offset;
            assert_eq!(m.read_u32(at + 0x54), if cam.words[0x15] == 0 { 0 } else { at + cam.words[0x15] }, "{c}: camera {k}");
            if let Some(a) = &cam.animation {
                channels += check_anim(m, f, a, &format!("{c}: camera {k}"));
            }
        }
        // the hash table
        if let Some(ht) = &p.hash {
            let at = f + off(0x80);
            for (l, list) in ht.lists.iter().enumerate() {
                let t = m.read_u32(at + 8 + 8 * l as u32);
                for (k, (hash, idx)) in list.iter().enumerate() {
                    let want = match l {
                        0 => get(cnt(8), off(0x44), PATCH, *idx),
                        1 => get(cnt(0xc), off(0x48), INSTANCE, *idx),
                        2 => if (*idx as i32) < 4000 && off(0x68) != 0 { model_ptr(*idx) } else { 0 },
                        3 => get(cnt(0x1c), off(0x58), LIGHT, *idx),
                        4 => spl(*idx),
                        _ => if off(0x78) != 0 { f + off(0x7c) + p.camera_pointers[*idx as usize] } else { 0 },
                    };
                    assert_eq!((m.read_u32(t + 8 * k as u32), m.read_u32(t + 8 * k as u32 + 4)), (*hash, want), "{c}: hash list {l} entry {k}");
                }
            }
        }
        eprintln!(
            "{c}: {} patches, {} instances, {} models ({} meshes, {} contexts), {} particle models, {} splines, {} segments, {} cameras, {} channels",
            p.patches.len(), p.instances.len(), p.models.len(), meshes, contexts, p.particle_models.len(), p.splines.len(), p.segments.len(), p.cameras.len(), channels
        );
    }
}

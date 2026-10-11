//! The `.pbd`'s patches (row E3a2) against the level folders' Patches.json (exported by SSX-Library from the same
//! files): with TRICKY_PBD_DIR (the disc's `.pbd` files) and the levels folder beside tricky-rs, every patch's
//! lightmap rectangle, texture coordinates, surface, show-off flag, texture and lightmap page must agree exactly, and
//! its control points to the export's rounding.

use tricky_data::pbd::{Pbd, PatchRec};

#[test]
fn patches_match_the_exported_json() {
    let Some(dir) = std::env::var_os("TRICKY_PBD_DIR") else { return };
    let levels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let mut checked = 0;
    for (folder, file) in [("gari", "gari"), ("snowdream", "snow"), ("elysium", "elysium"), ("mesablanca", "mesa"), ("merqury", "merquer"), ("aloha", "aloha"), ("pipedream", "pipe"), ("untracked", "untrack"), ("megaplex", "megaple"), ("alaska", "alaska")] {
        let (Ok(bytes), Ok(json)) = (std::fs::read(std::path::Path::new(&dir).join(format!("{file}.pbd"))), std::fs::read_to_string(levels.join(folder).join("Patches.json"))) else { continue };
        let pbd = Pbd::parse(&bytes).unwrap();
        let theirs: serde_json::Value = serde_json::from_str(&json).unwrap();
        let theirs = theirs["Patches"].as_array().unwrap();
        assert_eq!(pbd.patches.len(), theirs.len(), "{folder}: patches");
        // gari's file never sets the flags word (0 where the others have 0x29, and 0 at +0x1b4 for −1): the export's
        // show-off flag there comes from elsewhere (its show-off patches are the 60 of surface 18)
        let flags_set = pbd.patches.iter().any(|r| PatchRec::parse(r).unwrap().flags != 0);
        for (k, (raw, t)) in pbd.patches.iter().zip(theirs).enumerate() {
            let p = PatchRec::parse(raw).unwrap();
            let fs = |v: &serde_json::Value| v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect::<Vec<f32>>();
            assert_eq!(p.lightmap.to_vec(), fs(&t["LightMapPoint"]), "{folder}: patch {k} lightmap");
            for (c, uv) in t["UVPoints"].as_array().unwrap().iter().enumerate() {
                assert_eq!(p.uv[c][..2].to_vec(), fs(uv), "{folder}: patch {k} uv {c}");
            }
            assert_eq!(
                (p.surface as i64, if flags_set { p.showoff_only() } else { t["TrickOnlyPatch"].as_bool().unwrap() }, format!("{:04}.png", p.texture), p.lightmap_page as u64),
                (t["SurfaceType"].as_i64().unwrap(), t["TrickOnlyPatch"].as_bool().unwrap(), t["TexturePath"].as_str().unwrap().to_string(), t["LightmapID"].as_u64().unwrap()),
                "{folder}: patch {k}"
            );
            for (c, (a, b)) in p.bezier().iter().zip(t["Points"].as_array().unwrap()).enumerate() {
                let b = fs(b);
                for i in 0..3 {
                    assert!((a[i] - b[i]).abs() <= 0.05 + 2e-6 * b[i].abs(), "{folder}: patch {k} point {c}: {a:?} vs {b:?}");
                }
            }
            checked += 1;
        }
    }
    eprintln!("{checked} patches compared");
}


#[test]
fn instances_match_the_exported_json() {
    use tricky_data::pbd::InstanceRec;
    use tricky_data::ssf::Ssf;
    let (Some(dir), Some(sdir)) = (std::env::var_os("TRICKY_PBD_DIR"), std::env::var_os("TRICKY_SSF_DIR")) else { return };
    let levels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let mut checked = 0;
    for (folder, file) in [("gari", "gari"), ("snowdream", "snow"), ("elysium", "elysium"), ("mesablanca", "mesa"), ("merqury", "merquer"), ("aloha", "aloha"), ("pipedream", "pipe"), ("untracked", "untrack"), ("megaplex", "megaple"), ("alaska", "alaska")] {
        let (Ok(bytes), Ok(sbytes), Ok(json)) = (
            std::fs::read(std::path::Path::new(&dir).join(format!("{file}.pbd"))),
            std::fs::read(std::path::Path::new(&sdir).join(format!("{file}.ssf"))),
            std::fs::read_to_string(levels.join(folder).join("Instances.json")),
        ) else {
            continue;
        };
        let pbd = Pbd::parse(&bytes).unwrap();
        let ssf = Ssf::parse(&sbytes, pbd.instances.len() as u32).unwrap();
        let theirs: serde_json::Value = serde_json::from_str(&json).unwrap();
        let theirs = theirs["Instances"].as_array().unwrap();
        assert_eq!(pbd.instances.len(), theirs.len(), "{folder}: instances");
        for (k, (w, t)) in pbd.instances.iter().zip(theirs).enumerate() {
            let i = InstanceRec::from_words(w);
            let fs = |v: &serde_json::Value| v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect::<Vec<f32>>();
            let what = format!("{folder}: instance {k}");
            assert_eq!(i.matrix[3][..3].to_vec(), fs(&t["Location"]), "{what} location");
            for (n, key) in ["LightVector1", "LightVector2", "LightVector3"].iter().enumerate() {
                assert_eq!(i.light_dir(n).to_vec(), fs(&t[key])[..3].to_vec(), "{what} {key}");
            }
            for (n, key) in ["LightColour1", "LightColour2", "LightColour3"].iter().enumerate() {
                assert_eq!(i.light_colours[n].to_vec(), fs(&t[key]), "{what} {key}");
            }
            assert_eq!(i.ambient.to_vec(), fs(&t["AmbentLightColour"]), "{what} ambient");
            assert_eq!((i.model as i64, i.prev as i64, i.next as i64), (t["ModelID"].as_i64().unwrap(), t["PrevInstance"].as_i64().unwrap(), t["NextInstance"].as_i64().unwrap()), "{what}");
            // the matrix's rotation and scale against the export's quaternion and scale
            let m = glam::Mat4::from_cols_array_2d(&i.matrix);
            let (s, q, _) = m.to_scale_rotation_translation();
            let tq = fs(&t["Rotation"]);
            let tq = glam::Quat::from_xyzw(tq[0], tq[1], tq[2], tq[3]);
            assert!(q.dot(tq).abs() > 0.9999, "{what} rotation {q:?} vs {tq:?}");
            let ts = fs(&t["Scale"]);
            for a in 0..3 {
                assert!((s[a] - ts[a]).abs() < 1e-3 * ts[a].abs().max(1.0), "{what} scale {s:?} vs {ts:?}");
            }
            // the physics record
            let p = ssf.physics[ssf.object_physics[k] as usize];
            let d = p.defaults();
            assert_eq!(
                (d & 1 != 0, d & 0x20 != 0, d & 0x80 != 0, d & 0x1000 != 0, d & 0x2000 != 0),
                (t["Visable"].as_bool().unwrap(), t["PlayerCollision"].as_bool().unwrap(), t["PlayerBounce"].as_bool().unwrap(), t["Unknown241"].as_bool().unwrap(), t["UVScroll"].as_bool().unwrap()),
                "{what} flags"
            );
            let phys = if p.collision_mode() == 3 { p.group() as i64 } else { -1 };
            assert_eq!(
                (p.collision_mode() as i64, p.surface() as i64, p.slot() as i64, phys, p.bounce()),
                (t["CollsionMode"].as_i64().unwrap(), t["SurfaceType"].as_i64().unwrap(), t["EffectSlotIndex"].as_i64().unwrap(), t["PhysicsIndex"].as_i64().unwrap(), t["PlayerBounceAmmount"].as_f64().unwrap() as f32),
                "{what} physics"
            );
            let u0 = t["U0"].as_f64().unwrap() as f32;
            assert!((p.mass() - u0).abs() <= 1e-6 * u0.abs().max(1.0), "{what} mass");
            checked += 1;
        }
    }
    eprintln!("{checked} instances compared");
}


#[test]
fn materials_splines_lights_and_names_match_the_exported_json() {
    use tricky_data::map::Map;
    use tricky_data::pbd::{LightRec, MaterialRec, SegmentRec, SplineRec};
    let Some(dir) = std::env::var_os("TRICKY_PBD_DIR") else { return };
    let levels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let mut checked = [0usize; 5];
    for (folder, file) in [("gari", "gari"), ("snowdream", "snow"), ("elysium", "elysium"), ("mesablanca", "mesa"), ("merqury", "merquer"), ("aloha", "aloha"), ("pipedream", "pipe"), ("untracked", "untrack"), ("megaplex", "megaple"), ("alaska", "alaska")] {
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join(format!("{file}.pbd"))) else { continue };
        let pbd = Pbd::parse(&bytes).unwrap();
        let read = |n: &str| -> Option<serde_json::Value> { serde_json::from_str(&std::fs::read_to_string(levels.join(folder).join(n)).ok()?).ok() };
        let fs = |v: &serde_json::Value| v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect::<Vec<f32>>();
        let map = std::fs::read_to_string(std::path::Path::new(&dir).join(format!("{file}.map"))).ok().map(|t| Map::parse(&t));
        if let Some(j) = read("Materials.json") {
            let j = j["Materials"].as_array().unwrap();
            assert_eq!(pbd.materials.len(), j.len(), "{folder}: materials");
            for (k, (w, t)) in pbd.materials.iter().zip(j).enumerate() {
                let m = MaterialRec(*w);
                let tex = if m.texture() == 0xffff { String::new() } else { format!("{:04}.png", m.texture()) };
                assert_eq!(tex, t["TexturePath"].as_str().unwrap_or(""), "{folder}: material {k}");
                let book: Vec<String> = if m.flip() < 0 { vec![] } else { pbd.texture_flips[m.flip() as usize].iter().map(|x| format!("{:04}.png", x)).collect() };
                let theirs: Vec<String> = t["TextureFlipbook"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect()).unwrap_or_default();
                assert_eq!(book, theirs, "{folder}: material {k} flip-book");
                if let Some(map) = &map {
                    assert_eq!(map.name("MATERIALS", k), t["MaterialName"].as_str(), "{folder}: material {k} name");
                }
                checked[0] += 1;
            }
        }
        if let Some(j) = read("Splines.json") {
            let j = j["Splines"].as_array().unwrap();
            assert_eq!(pbd.splines.len(), j.len(), "{folder}: splines");
            for (k, (w, t)) in pbd.splines.iter().zip(j).enumerate() {
                let s = SplineRec(*w);
                let segs = t["Segments"].as_array().unwrap();
                assert_eq!(s.segments() as usize, segs.len(), "{folder}: spline {k} segments");
                for (n, sj) in segs.iter().enumerate() {
                    let g = SegmentRec(pbd.segments[(s.first() as usize) + n]);
                    assert_eq!(g.spline() as usize, k, "{folder}: spline {k} segment {n}");
                    // gari's export is of another build of the course (every segment differs; names and counts agree)
                    for (a, b) in g.bezier().iter().zip(sj["Points"].as_array().unwrap()).filter(|_| folder != "gari") {
                        let b = fs(b);
                        for i in 0..3 {
                            assert!((a[i] - b[i]).abs() <= 0.05 + 2e-6 * b[i].abs(), "{folder}: spline {k} segment {n}: {a:?} vs {b:?}");
                        }
                    }
                    checked[2] += 1;
                }
                if let Some(map) = &map {
                    assert_eq!(map.name("SPLINES", k), t["SplineName"].as_str(), "{folder}: spline {k} name");
                }
                checked[1] += 1;
            }
        }
        if let Some(j) = read("Lights.json") {
            let j = j["Lights"].as_array().unwrap();
            assert_eq!(pbd.lights.len(), j.len(), "{folder}: lights");
            for (k, (w, t)) in pbd.lights.iter().zip(j).enumerate() {
                let l = LightRec(*w);
                assert_eq!(
                    (l.kind() as i64, l.colour().to_vec(), l.direction().to_vec(), l.position().to_vec(), l.lowest().to_vec(), l.highest().to_vec()),
                    (t["Type"].as_i64().unwrap(), fs(&t["Colour"]), fs(&t["Direction"]), fs(&t["Position"]), fs(&t["LowestXYZ"]), fs(&t["HighestXYZ"])),
                    "{folder}: light {k}"
                );
                if let Some(map) = &map {
                    assert_eq!(map.name("LIGHTS", k), t["LightName"].as_str(), "{folder}: light {k} name");
                }
                checked[3] += 1;
            }
        }
        if let (Some(map), Some(pj), Some(ij)) = (&map, read("Patches.json"), read("Instances.json")) {
            for (k, t) in pj["Patches"].as_array().unwrap().iter().enumerate() {
                assert_eq!(map.name("PATCHES", k), t["PatchName"].as_str(), "{folder}: patch {k} name");
            }
            for (k, t) in ij["Instances"].as_array().unwrap().iter().enumerate() {
                assert_eq!(map.name("INTERNAL INSTANCES", k), t["InstanceName"].as_str(), "{folder}: instance {k} name");
                let e = &map.section("INTERNAL INSTANCES")[k];
                assert_eq!(e.hash as i64, t["Hash"].as_i64().unwrap(), "{folder}: instance {k} hash");
                checked[4] += 1;
            }
        }
    }
    eprintln!("materials {}, splines {} ({} segments), lights {}, names of {} instances compared", checked[0], checked[1], checked[2], checked[3], checked[4]);
}


#[test]
fn meshes_match_the_exported_obj_files() {
    use tricky_data::pbd::{context_strips, strip_triangles};
    let Some(dir) = std::env::var_os("TRICKY_PBD_DIR") else { return };
    let levels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let (mut objs, mut tris) = (0usize, 0usize);
    for (folder, file) in [("snowdream", "snow"), ("elysium", "elysium"), ("mesablanca", "mesa"), ("merqury", "merquer"), ("aloha", "aloha"), ("pipedream", "pipe"), ("untracked", "untrack"), ("megaplex", "megaple"), ("alaska", "alaska")] {
        let Ok(bytes) = std::fs::read(std::path::Path::new(&dir).join(format!("{file}.pbd"))) else { continue };
        let meshes = levels.join(folder).join("Meshes");
        if !meshes.exists() {
            continue;
        }
        let pbd = Pbd::parse(&bytes).unwrap();
        let vif = pbd.header.offset(0x84) as usize;
        let mut k = 0usize;
        for (mi, m) in pbd.models.iter().enumerate() {
            let scale = [6, 7, 8].map(|i| f32::from_bits(m.head[i]));
            for n in &m.nodes {
                if n.words[1] == 0 {
                    continue;
                }
                let mesh = m.meshes.iter().find(|x| x.offset == n.offset + n.words[1]).unwrap();
                for cx in &mesh.contexts {
                    let strips = context_strips(&bytes, vif + cx.words[3] as usize, scale).unwrap_or_else(|e| panic!("{folder}: model {mi}: {e}"));
                    let Ok(text) = std::fs::read_to_string(meshes.join(format!("{k}.obj"))) else { k += 1; continue };
                    let mut v = vec![];
                    let mut f = vec![];
                    let (mut vt, mut vn) = (vec![], vec![]);
                    for l in text.lines() {
                        let w: Vec<&str> = l.split_whitespace().collect();
                        match w.first() {
                            Some(&"v") => v.push([w[1], w[2], w[3]].map(|x| x.parse::<f32>().unwrap())),
                            Some(&"vt") => vt.push([w[1], w[2]].map(|x| x.parse::<f32>().unwrap())),
                            Some(&"vn") => vn.push([w[1], w[2], w[3]].map(|x| x.parse::<f32>().unwrap())),
                            Some(&"f") => f.push([w[1], w[2], w[3]].map(|x| x.split('/').map(|n| n.parse::<usize>().unwrap() - 1).collect::<Vec<_>>())),
                            _ => {}
                        }
                    }
                    let ours: Vec<[tricky_data::pbd::WorldVertex; 3]> = strips.iter().flat_map(|s| strip_triangles(s.len()).map(move |t| t.map(|i| s[i]))).collect();
                    assert_eq!(ours.len(), f.len(), "{folder}: {k}.obj (model {mi}) triangles");
                    for (ti, (a, b)) in ours.iter().zip(&f).enumerate() {
                        for c in 0..3 {
                            let (p, q) = (a[c], &b[c]);
                            let close = |x: f32, y: f32, e: f32| (x - y).abs() <= e + 1e-5 * y.abs();
                            let ok = (0..3).all(|x| close(p.pos[x], v[q[0]][x], 1e-3))
                                && close(p.uv[0], vt[q[1]][0], 1e-6)
                                && close(1.0 - p.uv[1], vt[q[1]][1], 1e-6)
                                && (0..3).all(|x| close(p.nrm[x], vn[q[2]][x], 1e-6));
                            assert!(ok, "{folder}: {k}.obj triangle {ti} corner {c}: {p:?} vs {:?} {:?} {:?}", v[q[0]], vt[q[1]], vn[q[2]]);
                        }
                    }
                    objs += 1;
                    tris += f.len();
                    k += 1;
                }
            }
        }
    }
    eprintln!("{objs} meshes, {tris} triangles compared");
}

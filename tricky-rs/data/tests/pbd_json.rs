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

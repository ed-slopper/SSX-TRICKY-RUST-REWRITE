//! `Level::from_disc` (row E3a4) against `Level::load` of the levels folders (SSX-Library's exports of the same
//! files): with TRICKY_DISC (the player's disc) and the levels folder beside tricky-rs, every course's patches,
//! instances, models, materials, splines, sky and meshes must agree (the exports round to 6 digits; gari's export is
//! of another build of the course, so its splines and meshes are not compared and its patches' show-off flag differs).

use std::path::Path;
use tricky_data::level::{Level, ObjMesh};

fn close(a: f32, b: f32, e: f32) -> bool {
    (a - b).abs() <= e + 2e-6 * b.abs()
}

fn same_mesh(a: &ObjMesh, b: &ObjMesh, what: &str) {
    assert_eq!(a.positions.len(), b.positions.len(), "{what}: corners");
    for k in 0..a.positions.len() {
        let ok = (0..3).all(|x| close(a.positions[k][x], b.positions[k][x], 1e-3) && close(a.normals[k][x], b.normals[k][x], 1e-5))
            && (0..2).all(|x| close(a.uvs[k][x], b.uvs[k][x], 1e-5));
        assert!(ok, "{what}: corner {k}: {:?} {:?} {:?} vs {:?} {:?} {:?}", a.positions[k], a.uvs[k], a.normals[k], b.positions[k], b.uvs[k], b.normals[k]);
    }
}

fn same_models(d: &tricky_data::level::ModelSet, f: &tricky_data::level::ModelSet, disc: &Level, folder: &Level, what: &str, names: bool, meshes: bool) -> usize {
    assert_eq!(d.models.len(), f.models.len(), "{what}: models");
    let mut n = 0;
    for (k, (a, b)) in d.models.iter().zip(&f.models).enumerate() {
        let w = format!("{what}: model {k}");
        if names {
            assert_eq!(a.model_name, b.model_name, "{w}");
        }
        assert_eq!(a.anim_time, b.anim_time, "{w}: time");
        assert_eq!(a.model_objects.len(), b.model_objects.len(), "{w}: objects");
        for (j, (x, y)) in a.model_objects.iter().zip(&b.model_objects).enumerate() {
            let w = format!("{w} object {j}");
            assert_eq!(x.parent_id, y.parent_id, "{w}: parent");
            let parts = |m: &tricky_data::level::ModelObject| m.mesh_data.iter().flatten().map(|p| (p.mesh_path.clone(), p.material_id)).collect::<Vec<_>>();
            assert_eq!(parts(x), parts(y), "{w}: meshes");
            if meshes {
                for p in x.mesh_data.iter().flatten() {
                    let path = d.dir.join("Meshes").join(&p.mesh_path);
                    assert!(disc.meshes.contains_key(&path), "{w}: {} not read", path.display());
                    same_mesh(&disc.mesh(&path), &folder.mesh(&f.dir.join("Meshes").join(&p.mesh_path)), &format!("{w}: {}", p.mesh_path));
                    n += 1;
                }
            }
            let v3 = |a: Option<[f32; 3]>, b: Option<[f32; 3]>, e: f32| match (a, b) {
                (Some(a), Some(b)) => (0..3).all(|i| close(a[i], b[i], e)),
                (None, None) => true,
                _ => false,
            };
            assert!(v3(x.position, y.position, 1e-3) && v3(x.scale, y.scale, 1e-4), "{w}: {:?} {:?} vs {:?} {:?}", x.position, x.scale, y.position, y.scale);
            match (x.rotation, y.rotation) {
                (Some(p), Some(q)) => assert!(glam::Quat::from_array(p).dot(glam::Quat::from_array(q)).abs() > 0.9999, "{w}: rotation {p:?} vs {q:?}"),
                (p, q) => assert_eq!(p.is_some(), q.is_some(), "{w}: rotation"),
            }
            match (&x.animation, &y.animation) {
                (Some(p), Some(q)) => {
                    assert_eq!((p.u1, p.u2, p.u3, p.u4, p.u5, p.u6, p.action), (q.u1, q.u2, q.u3, q.u4, q.u5, q.u6, q.action), "{w}: animation");
                    let segs = |o: &tricky_data::level::ObjAnim| o.entries.iter().map(|e| e.segs.iter().map(|s| [s.a, s.b, s.c, s.d, s.t0, s.t1]).collect::<Vec<_>>()).collect::<Vec<_>>();
                    assert_eq!(segs(p), segs(q), "{w}: animation");
                }
                (p, q) => assert_eq!(p.is_some(), q.is_some(), "{w}: animation"),
            }
        }
    }
    assert_eq!(d.materials.len(), f.materials.len(), "{what}: materials");
    for (k, (a, b)) in d.materials.iter().zip(&f.materials).enumerate() {
        if names {
            assert_eq!(a.material_name, b.material_name, "{what}: material {k}");
        }
        assert_eq!((&a.texture_path, a.texture_flipbook.clone().unwrap_or_default()), (&b.texture_path, b.texture_flipbook.clone().unwrap_or_default()), "{what}: material {k}");
    }
    n
}

#[test]
fn levels_from_the_disc_match_the_folders() {
    let Ok(iso) = std::env::var("TRICKY_DISC") else { return };
    let levels = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&iso).unwrap());
    let mut counts = [0usize; 6];
    for folder in ["gari", "snowdream", "elysium", "mesablanca", "merqury", "aloha", "pipedream", "untracked", "megaplex", "alaska", "trick"] {
        let dir = levels.join(folder);
        let Ok(f) = Level::load(&dir) else { continue };
        let d = Level::from_disc(&mut files, folder, Some(&dir)).unwrap();
        let same_build = folder != "gari";

        assert_eq!(d.patches.len(), f.patches.len(), "{folder}: patches");
        for (k, (a, b)) in d.patches.iter().zip(&f.patches).enumerate() {
            let w = format!("{folder}: patch {k}");
            assert_eq!((&a.patch_name, a.light_map_point, a.uv_points, a.surface_type, &a.texture_path, a.lightmap_id), (&b.patch_name, b.light_map_point, b.uv_points, b.surface_type, &b.texture_path, b.lightmap_id), "{w}");
            if same_build {
                assert_eq!(a.trick_only_patch, b.trick_only_patch, "{w}: show-off");
            }
            assert!(a.points.iter().zip(&b.points).all(|(p, q)| (0..3).all(|i| close(p[i], q[i], 0.05))), "{w}: points");
        }
        counts[0] += d.patches.len();

        assert_eq!(d.instances.len(), f.instances.len(), "{folder}: instances");
        for (k, (a, b)) in d.instances.iter().zip(&f.instances).enumerate() {
            let w = format!("{folder}: instance {k}");
            assert_eq!(
                (&a.instance_name, a.location, a.light_vector1, a.light_vector2, a.light_vector3, a.light_colour1, a.light_colour2, a.light_colour3, a.ambent_light_colour, a.model_id),
                (&b.instance_name, b.location, b.light_vector1, b.light_vector2, b.light_vector3, b.light_colour1, b.light_colour2, b.light_colour3, b.ambent_light_colour, b.model_id),
                "{w}"
            );
            assert_eq!(
                (a.visable, a.player_collision, a.player_bounce, a.collsion_mode, a.collsion_model_paths.clone().unwrap_or_default(), a.effect_slot_index, a.surface_type),
                (b.visable, b.player_collision, b.player_bounce, b.collsion_mode, b.collsion_model_paths.clone().unwrap_or_default(), b.effect_slot_index, b.surface_type),
                "{w}: physics"
            );
            assert!(close(a.u0, b.u0, 0.0), "{w}: mass");
            assert!(glam::Quat::from_array(a.rotation).dot(glam::Quat::from_array(b.rotation)).abs() > 0.9999, "{w}: rotation");
            assert!((0..3).all(|i| close(a.scale[i], b.scale[i], 1e-3)), "{w}: scale {:?} vs {:?}", a.scale, b.scale);
            for p in a.collsion_model_paths.iter().flatten() {
                let path = d.dir.join("Collision").join(p);
                assert!(d.meshes.contains_key(&path), "{w}: {p} not read");
                same_mesh(&d.mesh(&path), &f.mesh(&path), &format!("{w}: collision {p}"));
                counts[5] += 1;
            }
        }
        counts[1] += d.instances.len();
        assert_eq!((d.touch.len(), d.pieces.len(), d.switchable.len()), (f.touch.len(), f.pieces.len(), f.switchable.len()), "{folder}: scripts");

        counts[2] += same_models(&d.world, &f.world, &d, &f, folder, true, same_build);
        counts[3] += same_models(&d.sky, &f.sky, &d, &f, &format!("{folder} sky"), false, true);

        assert_eq!(d.splines.len(), f.splines.len(), "{folder}: splines");
        for (k, (a, b)) in d.splines.iter().zip(&f.splines).enumerate() {
            assert_eq!((&a.spline_name, a.segments.len()), (&b.spline_name, b.segments.len()), "{folder}: spline {k}");
            for (x, y) in a.segments.iter().zip(&b.segments).filter(|_| same_build) {
                assert!(x.points.iter().zip(&y.points).all(|(p, q)| (0..3).all(|i| close(p[i], q[i], 0.05))), "{folder}: spline {k}");
            }
        }
        counts[4] += d.splines.len();
    }
    eprintln!("{} patches, {} instances, {} world meshes, {} sky meshes, {} splines, {} collision meshes compared", counts[0], counts[1], counts[2], counts[3], counts[4], counts[5]);
}

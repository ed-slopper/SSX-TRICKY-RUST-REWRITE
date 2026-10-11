//! A course's `.aip` read from the disc (row E3d) against the level folder's AIP.json (exported by SSX-Library):
//! with TRICKY_AIP_DIR (the disc's files, unpacked) and the levels folder beside tricky-rs, every course's race lines
//! and AI paths must agree (points within a thousandth of a unit: the export multiplied direction by length itself).

use tricky_data::level::AiFile;

#[test]
fn disc_paths_match_the_exported_json() {
    let Some(dir) = std::env::var_os("TRICKY_AIP_DIR") else { return };
    let levels = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../levels");
    let mut checked = 0;
    for folder in ["gari", "snowdream", "elysium", "mesablanca", "merqury", "aloha", "pipedream", "untracked", "megaplex", "trick", "alaska"] {
        let Some(big) = tricky_data::aip::big_name(folder) else { continue };
        let (Ok(bytes), Ok(json)) = (std::fs::read(std::path::Path::new(&dir).join(format!("{big}.aip"))), std::fs::read_to_string(levels.join(folder).join("AIP.json"))) else { continue };
        let ours = AiFile::from_aip(&tricky_data::aip::read(&bytes).unwrap());
        let theirs: AiFile = serde_json::from_str(&json).unwrap();
        assert_eq!((ours.race_lines.len(), ours.ai_paths.len()), (theirs.race_lines.len(), theirs.ai_paths.len()), "{folder}: counts");
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() <= 1e-3 * (1.0 + b[k].abs()));
        for (k, (o, t)) in ours.ai_paths.iter().zip(&theirs.ai_paths).enumerate() {
            assert!(close(o.path_pos, t.path_pos), "{folder}: AI path {k} start");
            assert_eq!((o.u3, o.respawnable, o.path_points.len(), o.path_events.len()), (t.u3, t.respawnable, t.path_points.len(), t.path_events.len()), "{folder}: AI path {k}");
            for (j, (a, b)) in o.path_points.iter().zip(&t.path_points).enumerate() {
                assert!(close(*a, *b), "{folder}: AI path {k} point {j}: {a:?} vs {b:?}");
            }
            for (a, b) in o.path_events.iter().zip(&t.path_events) {
                assert_eq!((a.event_type, a.event_value, a.event_start, a.event_end), (b.event_type, b.event_value, b.event_start, b.event_end), "{folder}: AI path {k} event");
            }
        }
        for (k, (o, t)) in ours.race_lines.iter().zip(&theirs.race_lines).enumerate() {
            assert!(close(o.path_pos, t.path_pos), "{folder}: race line {k} start");
            assert!((o.distance_to_finish - t.distance_to_finish).abs() <= 1e-3 * (1.0 + t.distance_to_finish.abs()), "{folder}: race line {k} distance {} vs {}", o.distance_to_finish, t.distance_to_finish);
            assert_eq!(o.path_points.len(), t.path_points.len(), "{folder}: race line {k} points");
            for (a, b) in o.path_points.iter().zip(&t.path_points) {
                assert!(close(*a, *b), "{folder}: race line {k} point {a:?} vs {b:?}");
            }
            assert_eq!(o.path_events.len(), t.path_events.len(), "{folder}: race line {k} events");
        }
        checked += 1;
    }
    println!("{checked} courses agree");
}

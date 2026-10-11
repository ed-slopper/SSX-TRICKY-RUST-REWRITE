//! Summarises a course's `.aip` / `.sop` (row E3d): `cargo run -p tricky-data --example aip_dump -- <file>`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let aip = tricky_data::aip::read(&std::fs::read(&a[1]).expect("read")).expect("parse");
    println!("start positions {:?}", aip.start_positions);
    println!("{} AI paths, {} event paths (main {})", aip.ai_paths.len(), aip.event_paths.len(), aip.main_event_path);
    for (k, p) in aip.ai_paths.iter().take(2).chain(aip.event_paths.iter().take(1)).enumerate() {
        println!("path {k}: params {:?} start {:?} v1 {:?} v2 {:?} points {} first {:?} events {:?}", p.params, p.start, p.v1, p.v2, p.points.len(), p.points.first(), p.events.iter().take(3).collect::<Vec<_>>());
    }
}

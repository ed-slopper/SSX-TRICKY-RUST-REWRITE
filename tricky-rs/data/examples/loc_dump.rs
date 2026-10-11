//! Prints the strings of a `.loc` from the disc (row E5a): `cargo run -p tricky-data --example loc_dump -- <image> [path]`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount_game_archives().expect("archives");
    let path = a.get(2).map_or("data/lang/american.loc", |s| s.as_str());
    let loc = tricky_data::loc::LocFile::new(files.load(path).expect("read").expect("not found"));
    for i in 0..loc.count() as u32 {
        if let Some(s) = loc.get(i) { println!("{i:#06x} {s:?}"); }
    }
}

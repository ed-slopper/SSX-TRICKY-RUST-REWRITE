//! Loads one file the way the game's loader does (disc, then the mounted archives; unpacked):
//! `cargo run -p tricky-data --example disc_load -- <image or drive> <path> <out file>`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount_game_archives().expect("archives");
    let data = files.load(&a[2]).expect("read").expect("not found");
    std::fs::write(&a[3], &data).expect("write");
    eprintln!("{} bytes", data.len());
}

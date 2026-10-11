//! Lists every file of the game's mounted BIG archives (row E2a): `cargo run -p tricky-data --example big_list -- <image or drive>`.
//! Prints `archive name size checksum` (checksum: sum of the bytes as read, still packed).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount_game_archives().expect("archives");
    let list: Vec<(String, String)> = files.bigs.iter().flat_map(|b| tricky_data::big::entries(&b.header).into_iter().map(move |e| (b.path.clone(), e.name))).collect();
    for (big, name) in list {
        let f = files.open(&format!("{big}|{name}"), false).expect("open");
        let data = files.read(&f).expect("read");
        let sum: u64 = data.iter().map(|&b| b as u64).sum();
        println!("{big} {name} {} {sum}", data.len());
    }
}

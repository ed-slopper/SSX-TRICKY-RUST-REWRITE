//! Lists the game's file table of the player's disc (row E2): `cargo run -p tricky-data --example disc_list -- <image or drive> [path to dump] [out]`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut disc = tricky_data::disc::Disc::open(&a[1]).expect("mount");
    for f in disc.files() {
        println!("{:08x} {:7} {:10} {}", f.hash, f.lba, f.size, f.path);
    }
    eprintln!("{} files", disc.files().len());
    if a.len() > 3 {
        let data = disc.read_path(&a[2]).expect("read").expect("no such file");
        std::fs::write(&a[3], data).expect("write");
    }
}

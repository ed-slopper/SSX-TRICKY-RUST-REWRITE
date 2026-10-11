//! Writes every shape of an `.ssh` from the disc as raw RGBA (row E3b check):
//! `cargo run -p tricky-data --example ssh_dump -- <image> <path> <out dir>` writes `<name>.rgba` (u32 w, u32 h, pixels).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount_game_archives().expect("archives");
    let bytes = files.load(&a[2]).expect("read").expect("not found");
    let ssh = tricky_data::ssh::SshFile::open(&bytes).expect("ssh");
    for s in &ssh.shapes {
        let Some(img) = &s.image else { continue };
        let mut out = (img.width as u32).to_le_bytes().to_vec();
        out.extend((img.height as u32).to_le_bytes());
        out.extend(&img.rgba);
        std::fs::write(format!("{}/{}.rgba", a[3], s.name().replace('/', "_")), out).unwrap();
        println!("{} {} {}x{}", s.short, s.name(), img.width, img.height);
    }
}

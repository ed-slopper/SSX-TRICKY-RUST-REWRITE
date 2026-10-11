//! Lists the files of one BIG archive on disk (a level's, say): `cargo run -p tricky-data --example big_file_list -- <file.big>`.
//! Prints `name size unpacked-size`; with a name and an output path after the archive, writes that file unpacked.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let big = std::fs::read(&a[1]).expect("read");
    if let (Some(name), Some(out)) = (a.get(2), a.get(3)) {
        let data = tricky_data::big::file_data(&big, name).expect("no such file in the archive");
        std::fs::write(out, tricky_data::compress::load(data.to_vec()).unwrap_or_else(|| data.to_vec())).expect("write");
        return;
    }
    for e in tricky_data::big::entries(&big) {
        let data = tricky_data::big::file_data(&big, &e.name).unwrap_or_default();
        let un = tricky_data::compress::load(data.to_vec()).unwrap_or_default();
        println!("{} {} {}", e.name, data.len(), un.len());
    }
}

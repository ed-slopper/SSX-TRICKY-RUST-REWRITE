//! Lists the block types of every `.ssh` on the disc and in its archives (row E3b survey):
//! `cargo run -p tricky-data --example ssh_survey -- <image>`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount_game_archives().expect("archives");
    let mut paths: Vec<String> = files.disc.files().iter().filter(|f| f.path.ends_with(".SSH")).map(|f| f.path.clone()).collect();
    // the course and other archives on the disc, mounted here only for the survey
    let bigs: Vec<String> = files.disc.files().iter().filter(|f| f.path.ends_with(".BIG")).map(|f| f.path.clone()).collect();
    for b in &bigs { let _ = files.mount(b); }
    for b in &files.bigs {
        for e in tricky_data::big::entries(&b.header) {
            if e.name.to_ascii_lowercase().ends_with(".ssh") { paths.push(format!("{}|{}", b.path, e.name)); }
        }
    }
    let mut kinds = std::collections::BTreeMap::new();
    for p in &paths {
        let Some(f) = files.open(p, true) else { continue };
        let raw = files.read(&f).unwrap();
        let Some(d) = tricky_data::compress::load(raw) else { continue };
        if d.len() < 16 || &d[..4] != b"SHPS" { *kinds.entry("not SHPS".to_string()).or_insert(0) += 1; continue; }
        let n = u32::from_le_bytes(d[8..12].try_into().unwrap()) as usize;
        *kinds.entry(format!("version {}", String::from_utf8_lossy(&d[12..16]))).or_insert(0) += 1;
        for i in 0..n {
            let mut o = u32::from_le_bytes(d[16 + i * 8 + 4..16 + i * 8 + 8].try_into().unwrap()) as usize;
            while o + 4 <= d.len() {
                let w = u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                *kinds.entry(format!("type {:#04x}", w & 0xff)).or_insert(0) += 1;
                if w >> 8 == 0 { break; }
                o += (w >> 8) as usize;
            }
        }
    }
    println!("{} files", paths.len());
    for (k, v) in kinds { println!("{k}: {v}"); }
}

//! Prints one clip's curves at one frame (row E4b check): `cargo run -p tricky-data --example afl_dump -- <file.afl> <clip> <frame>`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let afl = tricky_data::afl::AflFile::open(&bytes).unwrap();
    let c = afl.clips.iter().find(|c| c.name() == Some(a[2].as_str())).expect("clip");
    let t: f32 = a[3].parse().unwrap();
    let (b, d) = (c.group(0).unwrap(), c.group(1).unwrap());
    let v: Vec<String> = (0..60).map(|k| afl.eval(b.first_curve, k, t)).chain((0..6).map(|k| afl.eval(d.first_curve, k, t))).map(|x| format!("{x:.4}")).collect();
    println!("{}", v.join(" "));
}

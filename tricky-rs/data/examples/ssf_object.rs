//! An object's scripts from a course's `.ssf` (`Instance_GetEventScript` 0x25fe78): its own event scripts and their
//! ops, every effect or function op that names it, and the objects whose event scripts run those effects:
//! `cargo run --example ssf_object -- <image or drive> <course> <object>` (`mesa 1444`, row F4h5l).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (course, object): (&str, usize) = (&a[2], a[3].parse().expect("object"));
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&a[1]).expect("mount"));
    files.mount(&format!("data/models/{course}.big")).expect("course archive");
    let mut get = |ext: &str| files.load(&format!("|data/models/{course}.{ext}")).expect("read").expect("not found");
    let pbd = tricky_data::pbd::Pbd::parse(&get("pbd")).expect("pbd");
    let ssf = tricky_data::ssf::Ssf::parse(&get("ssf"), pbd.instances.len() as u32).expect("ssf");
    let floats = |op: &tricky_data::ssf::Op| op.args.iter().map(|w| f32::from_bits(*w)).collect::<Vec<_>>();
    println!("object {object}: physics {:?}", ssf.object_physics.get(object).and_then(|p| ssf.physics.get(*p as usize)));
    for e in 0..7 {
        let s = ssf.event_script(object, e);
        if s < 0 {
            continue;
        }
        println!("event {e}: effect {s}");
        for op in &ssf.effects[s as usize] {
            println!("  {op:?}  floats {:?}", floats(op));
        }
    }
    for (k, e) in ssf.effects.iter().enumerate() {
        if !e.iter().any(|op| op.args.contains(&(object as u32))) {
            continue;
        }
        println!("effect {k} names it:");
        for op in e {
            println!("  {op:?}  floats {:?}", floats(op));
        }
        for o in 0..pbd.instances.len() {
            for ev in 0..7 {
                if ssf.event_script(o, ev) == k as i32 {
                    println!("  run by object {o} on event {ev}");
                }
            }
        }
    }
    for f in &ssf.functions {
        for op in f.ops.iter().filter(|op| op.args.contains(&(object as u32))) {
            println!("function {}: {op:?}", f.name);
        }
    }
}

//! The AI paths' geometry against the original's (board row G4f), on every course's own paths: each `.aip` in
//! TRICKY_AIP_DIR loaded by the game's loader in the function runner (TRICKY_ELF; skipped without either), then
//! `AIP_GetPathLength`, `AIP_GetPositionAtDistance`, `AIP_GetDirectionAtDistance`, `AIP_ProjectOntoPath`,
//! `AIP_ProjectOntoPathFrom`, `AIP_DistanceToLineBox` and `AIP_SortLinesByDistance` run on them at random places
//! against `tricky_game::aipath`, bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aipath::{sort_by_distance, Cache, Path};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [u32; 4] {
    [m.read_u32(at), m.read_u32(at + 4), m.read_u32(at + 8), m.read_u32(at + 12)]
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

/// The course's paths loaded by `AIP_LoadPathsFromFile` 0x197da0 in the runner; returns the path set's address.
fn load(r: &mut Runner, bytes: &[u8]) -> u32 {
    for f in [0x197da0u32, 0x198518, 0x1983d0, 0x196f28, 0x198738, 0x198720, 0x198750, 0x197130, 0x197170, 0x2cc808, 0x2ce114, 0x196700, 0x196718, 0x196990, 0x196998] {
        r.allow(f);
    }
    let buf = r.mem.alloc(bytes.len() as u32 + 16);
    for (k, b) in bytes.iter().enumerate() {
        r.mem.write_u8(buf + k as u32, *b);
    }
    r.stub(0x2c8a70, move |c, _| c.set_gpr(2, buf as u64));
    let heap = r.mem.alloc(0x40_0000);
    let next = std::rc::Rc::new(std::cell::Cell::new(heap));
    let n = next.clone();
    r.stub(0x2ccf70, move |c, _| {
        let size = (c.gpr(5) as u32 + 15) & !15;
        let at = n.get();
        n.set(at + size);
        c.set_gpr(2, at as u64);
    });
    let set = r.mem.alloc(0x200);
    r.cpu.set_gpr(4, set as u64);
    r.cpu.set_gpr(5, 0);
    r.call(0x197da0).expect("load");
    set
}

#[test]
fn path_geometry_matches_the_original() {
    let Some(dir) = std::env::var_os("TRICKY_AIP_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "aip")).collect();
    files.sort();
    let mut g = Rng(0x13a7_c2e9_04bd_5f61);
    for file in files {
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        r.allow(0x102f50); // Vec4_Scale
        let bytes = std::fs::read(&file).unwrap();
        let set = load(&mut r, &bytes);
        let paths: Vec<Path> = tricky_data::aip::read(&bytes).unwrap().ai_paths.iter().map(Path::from_aip).collect();
        let base = r.mem.read_u32(set + 4);
        let (pos, out, flag, lat, cache, list) = (r.mem.alloc(16), r.mem.alloc(16), r.mem.alloc(16), r.mem.alloc(16), r.mem.alloc(32), r.mem.alloc(4 * 8));
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        for case in 0..3000 {
            let k = g.next() as usize % paths.len();
            let p = &paths[k];
            let pa = base + 0x40 * k as u32;
            let len = p.length();
            r.cpu.set_gpr(4, pa as u64);
            r.call(0x197b50).unwrap();
            assert_eq!(r.cpu.f(0).to_bits(), len.to_bits(), "{name} path {k}: length");
            let d = match g.next() % 4 { 0 => g.f(-100.0, 0.0), 1 => g.f(len, len + 2000.0), _ => g.f(0.0, len) };
            r.cpu.set_f(12, d);
            r.cpu.set_gpr(4, out as u64);
            r.cpu.set_gpr(5, pa as u64);
            r.cpu.set_gpr(6, flag as u64);
            r.call(0x1979f8).unwrap();
            let (at, more) = p.position_at(d);
            assert_eq!((get4(&mut r.mem, out), r.mem.read_u32(flag) != 0), (bits(at), more), "{name} path {k}: position at {d}");
            r.cpu.set_f(12, d);
            r.cpu.set_gpr(4, out as u64);
            r.cpu.set_gpr(5, pa as u64);
            r.call(0x197738).unwrap();
            assert_eq!(get4(&mut r.mem, out), bits(p.direction_at(d)), "{name} path {k}: direction at {d}");
            // a place near the path (or anywhere on the course)
            let (q, _) = p.position_at(g.f(0.0, len));
            let spread = if g.next() % 3 == 0 { 20000.0 } else { 1500.0 };
            let at = [q[0] + g.f(-spread, spread), q[1] + g.f(-spread, spread), q[2] + g.f(-600.0, 600.0), 1.0];
            put4(&mut r.mem, pos, at);
            r.cpu.set_gpr(4, pa as u64);
            r.cpu.set_gpr(5, pos as u64);
            r.cpu.set_gpr(6, out as u64);
            r.cpu.set_gpr(7, lat as u64);
            r.call(0x197250).unwrap();
            let (along, near, side) = p.project(at);
            assert_eq!((r.cpu.f(0).to_bits(), get4(&mut r.mem, out), r.mem.read_u32(lat)), (along.to_bits(), bits(near), side.to_bits()), "{name} path {k}: project {at:?}");
            // from a remembered segment (or none)
            let mut c = if g.next() % 3 == 0 { Cache::default() } else {
                let i = g.next() as usize % p.points.len();
                let run: f32 = p.points[..i].iter().map(|x| x.1).fold(0.0, |a, b| a + b);
                Cache { start: p.position_at(run).0, along: run, index: i as i32 }
            };
            let near0 = [g.f(-9.0, 9.0); 4];
            put4(&mut r.mem, out, near0);
            put4(&mut r.mem, cache, c.start);
            r.mem.write_f32(cache + 0x10, c.along);
            r.mem.write_u32(cache + 0x14, c.index as u32);
            r.cpu.set_gpr(4, pa as u64);
            r.cpu.set_gpr(5, pos as u64);
            r.cpu.set_gpr(6, out as u64);
            r.cpu.set_gpr(7, lat as u64);
            r.cpu.set_gpr(8, cache as u64);
            r.call(0x197478).unwrap();
            let mut near = near0;
            let (along, side) = p.project_from(at, &mut near, &mut c);
            let theirs = (r.cpu.f(0).to_bits(), get4(&mut r.mem, out), r.mem.read_u32(lat), get4(&mut r.mem, cache), r.mem.read_u32(cache + 0x10), r.mem.read_u32(cache + 0x14) as i32);
            assert_eq!(theirs, (along.to_bits(), bits(near), side.to_bits(), bits(c.start), c.along.to_bits(), c.index), "{name} path {k} case {case}: project from");
            r.cpu.set_gpr(4, pa as u64);
            r.cpu.set_gpr(5, pos as u64);
            r.call(0x197830).unwrap();
            assert_eq!(r.cpu.f(0).to_bits(), p.box_distance(at).to_bits(), "{name} path {k}: box distance");
            let n = 1 + g.next() as usize % 8;
            let resp = g.next() % 2 == 0;
            r.cpu.set_gpr(4, set as u64);
            r.cpu.set_gpr(5, pos as u64);
            r.cpu.set_gpr(6, list as u64);
            r.cpu.set_gpr(7, n as u64);
            r.cpu.set_gpr(8, resp as u64);
            r.call(0x197f28).unwrap();
            let count = r.cpu.gpr(2) as usize;
            let theirs: Vec<usize> = (0..count).map(|i| ((r.mem.read_u32(list + 4 * i as u32) - base) / 0x40) as usize).collect();
            assert_eq!(theirs, sort_by_distance(&paths, at, n, resp), "{name} case {case}: sort ({n}, respawnable {resp}) at {at:?}");
        }
        println!("{name}: {} paths checked", paths.len());
    }
}

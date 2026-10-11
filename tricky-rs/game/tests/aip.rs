//! The courses' `.aip` / `.sop` files against the game's own loader (board row E3d): `AIP_LoadPathsFromFile`
//! 0x197da0 (with `AIP_LoadAIPaths`, `AIP_LoadEventPaths`, `AIPath_Load` and the path classes' parameter methods)
//! run in the function runner on the player's SLUS_203.26 (TRICKY_ELF) on every file in TRICKY_AIP_DIR (the disc's
//! files, unpacked; skipped without either), its file load answered with the file and its allocations from a
//! simple heap, against `tricky_data::aip::read`: every count, vector, parameter, point and event.

use r5900::Runner;
use tricky_data::aip;

fn f3(m: &mut r5900::Mem, at: u32) -> [f32; 3] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8)]
}

/// One loaded path (`cPath`: +0 events, +4 their address, +8 points, +0xc start, +0x18 the points' address, +0x1c
/// and +0x28 the box, +0x38 / +0x3c the class's parameters) against ours.
fn check_path(m: &mut r5900::Mem, at: u32, p: &aip::AipPath, ai: bool, what: &str) {
    let ev = m.read_u32(at) as usize;
    let pts = m.read_u32(at + 8) as usize;
    assert_eq!((pts, ev), (p.points.len(), p.events.len()), "{what}: counts");
    assert_eq!(f3(m, at + 0xc).map(f32::to_bits), p.start.map(f32::to_bits), "{what}: start");
    assert_eq!(f3(m, at + 0x1c).map(f32::to_bits), p.v1.map(f32::to_bits), "{what}: v1");
    assert_eq!(f3(m, at + 0x28).map(f32::to_bits), p.v2.map(f32::to_bits), "{what}: v2");
    if ai {
        assert_eq!((m.read_u32(at + 0x38), m.read_u32(at + 0x3c)), (p.u3(), p.respawnable()), "{what}: U3, respawnable");
    } else if let Some(d) = p.param(0) {
        assert_eq!(m.read_u32(at + 0x38), d, "{what}: distance parameter");
    }
    let pp = m.read_u32(at + 0x18);
    for (k, (o, d)) in p.points.iter().enumerate() {
        let a = pp + 16 * k as u32;
        assert_eq!((f3(m, a).map(f32::to_bits), m.read_u32(a + 12)), (o.map(f32::to_bits), d.to_bits()), "{what}: point {k}");
    }
    let ep = m.read_u32(at + 4);
    for (k, e) in p.events.iter().enumerate() {
        let a = ep + 16 * k as u32;
        let got = (m.read_u32(a), m.read_u32(a + 4), m.read_u32(a + 8), m.read_u32(a + 12));
        assert_eq!(got, (e.kind, e.value, e.start.to_bits(), e.end.to_bits()), "{what}: event {k}");
    }
}

#[test]
fn course_ai_files_load_as_the_game_loads_them() {
    let Some(dir) = std::env::var_os("TRICKY_AIP_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("TRICKY_AIP_DIR").filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "aip" || x == "sop")).collect();
    files.sort();
    assert!(!files.is_empty(), "no .aip / .sop files in TRICKY_AIP_DIR");
    for path in files {
        let Some(mut r) = Runner::from_env() else { return };
        let bytes = std::fs::read(&path).unwrap();
        let ours = aip::read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for f in [0x197da0u32, 0x198518, 0x1983d0, 0x196f28, 0x198738, 0x198720, 0x198750, 0x197130, 0x197170, 0x2cc808, 0x2ce114, 0x196700, 0x196718, 0x196990, 0x196998] {
            r.allow(f);
        }
        let buf = r.mem.alloc(bytes.len() as u32 + 16);
        for (k, b) in bytes.iter().enumerate() {
            r.mem.write_u8(buf + k as u32, *b);
        }
        r.stub(0x2c8a70, move |c, _| c.set_gpr(2, buf as u64)); // File_Load
        let heap = r.mem.alloc(0x40_0000);
        let next = std::rc::Rc::new(std::cell::Cell::new(heap));
        let n = next.clone();
        r.stub(0x2ccf70, move |c, _| {
            // EAMem_Alloc(name, size, flags): a simple heap, 16-byte aligned
            let size = (c.gpr(5) as u32 + 15) & !15;
            let at = n.get();
            n.set(at + size);
            c.set_gpr(2, at as u64);
        });
        let set = r.mem.alloc(0x200);
        r.cpu.set_gpr(4, set as u64);
        r.cpu.set_gpr(5, 0);
        r.call(0x197da0).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let m = &mut r.mem;
        assert_eq!(m.read_u32(set) as usize, ours.ai_paths.len(), "{name}: AI paths");
        assert_eq!(m.read_u32(set + 8) as usize, ours.event_paths.len(), "{name}: event paths");
        let (ai, evp) = (m.read_u32(set + 4), m.read_u32(set + 0xc));
        for (k, s) in ours.start_positions.iter().enumerate() {
            assert_eq!(m.read_u32(set + 0x14 + 4 * k as u32), ai + 0x40 * s, "{name}: start position {k}");
        }
        assert_eq!(m.read_u32(set + 0x10), evp + 0x3c * ours.main_event_path, "{name}: main event path");
        for (k, p) in ours.ai_paths.iter().enumerate() {
            check_path(m, ai + 0x40 * k as u32, p, true, &format!("{name}: AI path {k}"));
        }
        for (k, p) in ours.event_paths.iter().enumerate() {
            check_path(m, evp + 0x3c * k as u32, p, false, &format!("{name}: event path {k}"));
        }
        println!("{name}: {} AI paths, {} event paths, {} starts", ours.ai_paths.len(), ours.event_paths.len(), ours.start_positions.len());
    }
}

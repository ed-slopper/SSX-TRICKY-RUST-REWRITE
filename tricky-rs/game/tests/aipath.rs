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

#[test]
fn path_following_matches_the_original() {
    // `Boarder_UpdatePathAhead` 0x118090 (and `Boarder_ChoosePathAhead` 0x118830 under it) on riders placed about
    // every course's paths, loaded into the game's own path set (0x337fb8); `Rand_Game` and the rider's virtual +0x74
    // answered, the race tick and the rider's fields set
    use std::cell::RefCell;
    use std::rc::Rc;
    use tricky_game::aipath::{update_path_ahead, Chooser, Follow};
    let Some(dir) = std::env::var_os("TRICKY_AIP_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "aip")).collect();
    files.sort();
    let mut g = Rng(0x7c51_e0a9_3b26_d48f);
    let mut chose = [0u32; 2];
    for file in files {
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for f in [0x102f50u32, 0x2510f8, 0x2f79a8, 0x2f7018, 0x118090, 0x118830, 0x197b50, 0x1979f8, 0x197250, 0x197478, 0x197830, 0x197f28, 0x197f08, 0x197910, 0x197b90] {
            r.allow(f);
        }
        let bytes = std::fs::read(&file).unwrap();
        // the course's paths in the game's path set
        for f in [0x197da0u32, 0x198518, 0x1983d0, 0x196f28, 0x198738, 0x198720, 0x198750, 0x197130, 0x197170, 0x2cc808, 0x2ce114, 0x196700, 0x196718, 0x196990, 0x196998] {
            r.allow(f);
        }
        let buf = r.mem.alloc(bytes.len() as u32 + 16);
        for (k, b) in bytes.iter().enumerate() {
            r.mem.write_u8(buf + k as u32, *b);
        }
        r.stub(0x2c8a70, move |c, _| c.set_gpr(2, buf as u64));
        let heap = r.mem.alloc(0x40_0000);
        let next = Rc::new(std::cell::Cell::new(heap));
        let n = next.clone();
        r.stub(0x2ccf70, move |c, _| {
            let size = (c.gpr(5) as u32 + 15) & !15;
            let at = n.get();
            n.set(at + size);
            c.set_gpr(2, at as u64);
        });
        r.cpu.set_gpr(4, 0x337fb8);
        r.cpu.set_gpr(5, 0);
        r.call(0x197da0).expect("load");
        let paths: Vec<Path> = tricky_data::aip::read(&bytes).unwrap().ai_paths.iter().map(Path::from_aip).collect();
        let base = r.mem.read_u32(0x337fb8 + 4);
        let rolls: Rc<RefCell<Vec<i32>>> = Rc::default();
        let rl = rolls.clone();
        r.stub(0x23d4d8, move |c, _| { let v = rl.borrow_mut().remove(0); c.set_gpr(2, v as u64); });
        let keep = Rc::new(std::cell::Cell::new(0.5f32));
        let kp = keep.clone();
        r.stub(0x1000, move |c, _| c.set_f(0, kp.get()));
        let app = r.mem.alloc(0x800);
        let game = r.mem.alloc(0x100);
        let world = r.mem.alloc(0x100);
        r.mem.write_u32(0x337c58, app);
        r.mem.write_u32(app + 0x730, game);
        r.mem.write_u32(game + 0x1c, world);
        let rider = r.mem.alloc(0x5900);
        let vt = r.mem.alloc(0x100);
        r.mem.write_u32(rider, vt);
        r.mem.write_u16(vt + 0x70, 0);
        r.mem.write_u32(vt + 0x74, 0x1000);
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        for case in 0..1500 {
            let k = g.next() as usize % paths.len();
            let p = &paths[k];
            let len = p.length();
            let d0 = if g.next() % 4 == 0 { g.f((len - 250.0).max(0.0), len) } else { g.f(0.0, len) };
            let (q, _) = p.position_at(d0);
            let off = [300.0f32, 600.0, 900.0][g.next() as usize % 3];
            let pos = [q[0] + g.f(-off, off), q[1] + g.f(-off, off), q[2] + g.f(-200.0, 200.0), 1.0];
            let vel = [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-300.0, 300.0), 0.0];
            let mut f = Follow { path: k, last: g.f(0.0, len), along: d0, off: 0.0, near: [g.f(-9.0, 9.0); 4], ahead: [0.0; 4], heading: 0.0, cache: Cache::default() };
            if g.next() % 2 == 0 {
                let i = g.next() as usize % p.points.len();
                let run: f32 = p.points[..i].iter().map(|x| x.1).fold(0.0, |a, b| a + b);
                f.cache = Cache { start: p.position_at(run).0, along: run, index: i as i32 };
            }
            let c = Chooser { pos, vel, x41c: (g.next() % 4 == 0) as i32, class: g.next() % 3, place: (g.next() % 6) as i32, meter: g.f(-0.1, 1.0), keep: [g.f(0.0, 1.0), 1.0, 0.0][g.next() as usize % 3] };
            let racing = g.next() % 5 != 0;
            let tick = (g.next() % 300) as i32 * if g.next() % 2 == 0 { 60 } else { 1 };
            let roll_v: Vec<i32> = (0..4).map(|_| (g.next() % 1000) as i32).collect();
            *rolls.borrow_mut() = roll_v.clone();
            keep.set(c.keep);
            let m = &mut r.mem;
            m.write_u32(world + 0x18, tick as u32);
            put4(m, rider + 0x140, pos);
            put4(m, rider + 0x150, vel);
            m.write_u32(rider + 0x57a4, base + 0x40 * k as u32);
            m.write_f32(rider + 0x340, f.last);
            m.write_f32(rider + 0x344, f.along);
            put4(m, rider + 0x350, f.near);
            put4(m, rider + 0x390, f.cache.start);
            m.write_f32(rider + 0x3a0, f.cache.along);
            m.write_u32(rider + 0x3a4, f.cache.index as u32);
            m.write_u32(rider + 0x418, if racing { 0xffff_ffff } else { 500 });
            m.write_u32(rider + 0x41c, c.x41c as u32);
            m.write_u32(rider + 0x420, c.class);
            m.write_u32(rider + 0x110, c.place as u32);
            m.write_f32(rider + 0x1c, c.meter);
            r.cpu.set_gpr(4, rider as u64);
            r.call(0x118090).unwrap_or_else(|e| panic!("{name} case {case}: {e}"));
            let used = roll_v.len() - rolls.borrow().len();
            let path0 = f.path;
            let mut q = roll_v.clone();
            let mut rand = || q.remove(0);
            update_path_ahead(&paths, &mut f, racing, tick, &c, &mut rand);
            let m = &mut r.mem;
            let theirs = (
                ((m.read_u32(rider + 0x57a4) - base) / 0x40) as usize,
                m.read_u32(rider + 0x340), m.read_u32(rider + 0x344), m.read_u32(rider + 0x348),
                get4(m, rider + 0x350), get4(m, rider + 0x360), m.read_u32(rider + 0x370),
                get4(m, rider + 0x390), m.read_u32(rider + 0x3a0), m.read_u32(rider + 0x3a4) as i32,
            );
            let ours = (f.path, f.last.to_bits(), f.along.to_bits(), f.off.to_bits(), bits(f.near), bits(f.ahead), f.heading.to_bits(), bits(f.cache.start), f.cache.along.to_bits(), f.cache.index);
            assert_eq!(theirs, ours, "{name} case {case}: path {k}, racing {racing}, tick {tick}, {c:?}");
            assert_eq!(used, roll_v.len() - q.len(), "{name} case {case}: rolls");
            chose[(f.path != path0) as usize] += 1;
        }
        println!("{name}: path following checked");
    }
    assert!(chose[1] > 200, "kept / changed path: {chose:?}");
}

/// The course's paths loaded into the game's own path set (0x337fb8) in the runner.
fn load_set(r: &mut Runner, bytes: &[u8]) {
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
    r.cpu.set_gpr(4, 0x337fb8);
    r.cpu.set_gpr(5, 0);
    r.call(0x197da0).expect("load");
}

#[test]
fn reset_placing_matches_the_original() {
    // `AIP_CanResetAt` 0x196858, `ResetState_GetRespawnPos` 0x196758 and `AIPathSet_FindRaceLineAt` 0x1982d0 on every
    // course's AI paths and race lines; `Boarder_AdvanceOnPath` 0x118c08 and `Boarder_FindNearestLine` 0x119d30 (its
    // `ResetState_PlaceRider` recorded) on riders about them
    use std::cell::RefCell;
    use std::rc::Rc;
    use tricky_game::aipath::{advance_on_path, find_nearest_line, Follow, RaceLines, RacePlace};
    let Some(dir) = std::env::var_os("TRICKY_AIP_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "aip")).collect();
    files.sort();
    let mut g = Rng(0x5e2d_91c7_a04b_3f68);
    let (mut moved, mut blocked, mut changed, mut found) = (0u32, 0u32, 0u32, 0u32);
    for file in files {
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for f in [0x102f50u32, 0x2510f8, 0x2f79a8, 0x2f7018, 0x197b50, 0x1979f8, 0x197738, 0x197250, 0x197478, 0x197830, 0x197f28, 0x197f08, 0x197910, 0x197ef0, 0x196858, 0x196758, 0x1982d0, 0x118c08, 0x119d30] {
            r.allow(f);
        }
        let bytes = std::fs::read(&file).unwrap();
        load_set(&mut r, &bytes);
        let aip = tricky_data::aip::read(&bytes).unwrap();
        let paths: Vec<Path> = aip.ai_paths.iter().map(Path::from_aip).collect();
        let lines = RaceLines::from_aip(&aip);
        let (base, lbase) = (r.mem.read_u32(0x337fb8 + 4), r.mem.read_u32(0x337fb8 + 0xc));
        let placed: Rc<RefCell<Vec<(u64, u64, u32)>>> = Rc::default();
        let pl = placed.clone();
        r.stub(0x119220, move |c, _| pl.borrow_mut().push((c.gpr(4), c.gpr(5), c.f(12).to_bits())));
        let (out, dist) = (r.mem.alloc(16), r.mem.alloc(16));
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        // the events' checks, on both kinds of path, often right at an event's ends
        for case in 0..4000 {
            let line = g.next() % 2 == 0 && !lines.lines.is_empty();
            let (p, pa) = if line {
                let k = g.next() as usize % lines.lines.len();
                (&lines.lines[k], lbase + 0x3c * k as u32)
            } else {
                let k = g.next() as usize % paths.len();
                (&paths[k], base + 0x40 * k as u32)
            };
            let len = p.length();
            let d = if !p.events.is_empty() && g.next() % 2 == 0 {
                let e = p.events[g.next() as usize % p.events.len()];
                [e.2, e.3, g.f(e.2, e.3)][g.next() as usize % 3]
            } else {
                g.f(-100.0, len + 100.0)
            };
            r.cpu.set_f(12, d);
            r.cpu.set_gpr(4, pa as u64);
            r.call(0x196858).unwrap();
            assert_eq!(r.cpu.gpr(2) != 0, p.can_reset_at(d), "{name} case {case}: can reset at {d}");
            blocked += !p.can_reset_at(d) as u32;
            r.mem.write_f32(dist, d);
            r.cpu.set_gpr(4, out as u64);
            r.cpu.set_gpr(5, pa as u64);
            r.cpu.set_gpr(6, dist as u64);
            r.call(0x196758).unwrap();
            let (at, pos) = p.respawn_at(d);
            assert_eq!((r.mem.read_u32(dist), get4(&mut r.mem, out)), (at.to_bits(), bits(pos)), "{name} case {case}: respawn at {d}");
            moved += (at != d) as u32;
            let main = lines.to_finish.get(lines.main).copied().unwrap_or(0.0);
            let d = g.f(-500.0, main + 500.0);
            put4(&mut r.mem, out, [7.0; 4]);
            r.cpu.set_f(12, d);
            r.cpu.set_gpr(4, 0x337fb8);
            r.cpu.set_gpr(5, out as u64);
            r.call(0x1982d0).unwrap();
            let theirs = match r.cpu.gpr(2) as u32 {
                0 => None,
                a => Some((((a - lbase) / 0x3c) as usize, get4(&mut r.mem, out))),
            };
            assert_eq!(theirs, lines.line_at(d).map(|(k, v)| (k, bits(v))), "{name} case {case}: race line at {d}");
        }
        // riders put back along their paths, and onto the race line
        let rider = r.mem.alloc(0x5900);
        for case in 0..1500 {
            let k = g.next() as usize % paths.len();
            let p = &paths[k];
            let len = p.length();
            let d0 = if g.next() % 4 == 0 { g.f((len - 1500.0).max(0.0), len) } else { g.f(0.0, len) };
            let mut f = Follow { path: k, last: g.f(0.0, len), along: d0, off: g.f(0.0, 500.0), near: [g.f(-9.0, 9.0); 4], ahead: [g.f(-9.0, 9.0); 4], heading: g.f(-3.0, 3.0), cache: Cache::default() };
            if g.next() % 2 == 0 {
                let i = g.next() as usize % p.points.len();
                let run: f32 = p.points[..i].iter().map(|x| x.1).fold(0.0, |a, b| a + b);
                f.cache = Cache { start: p.position_at(run).0, along: run, index: i as i32 };
            }
            let mut rp = RacePlace { line: 0, to_finish: g.f(0.0, 9000.0), best: g.f(0.0, 90000.0), cache: Cache { start: [g.f(-9.0, 9.0); 4], along: g.f(0.0, 99.0), index: 3 } };
            let laps = (g.next() % 3) as i32 - 1;
            let m = &mut r.mem;
            m.write_u32(rider + 0x57a4, base + 0x40 * k as u32);
            m.write_u32(rider + 0x57a0, lbase);
            m.write_f32(rider + 0x340, f.last);
            m.write_f32(rider + 0x344, f.along);
            m.write_f32(rider + 0x348, f.off);
            put4(m, rider + 0x350, f.near);
            put4(m, rider + 0x360, f.ahead);
            m.write_f32(rider + 0x370, f.heading);
            put4(m, rider + 0x390, f.cache.start);
            m.write_f32(rider + 0x3a0, f.cache.along);
            m.write_u32(rider + 0x3a4, f.cache.index as u32);
            m.write_f32(rider + 0x374, rp.to_finish);
            m.write_f32(rider + 0x378, rp.best);
            put4(m, rider + 0x3b0, rp.cache.start);
            m.write_f32(rider + 0x3c0, rp.cache.along);
            m.write_u32(rider + 0x3c4, rp.cache.index as u32);
            m.write_u32(rider + 0x114, laps as u32);
            let nearest = g.next() % 2 == 0;
            let by = g.f(0.0, 4000.0);
            let path0 = f.path;
            r.cpu.set_gpr(4, rider as u64);
            if nearest {
                let d = g.f(-500.0, lines.to_finish[lines.main] + 500.0);
                if lines.line_at(d).is_none() {
                    continue;
                }
                r.cpu.set_f(12, d);
                r.cpu.set_f(13, 1.5);
                r.cpu.set_gpr(5, 0x5a5a);
                placed.borrow_mut().clear();
                r.call(0x119d30).unwrap_or_else(|e| panic!("{name} case {case}: {e}"));
                assert!(find_nearest_line(&paths, &lines, &mut f, &mut rp, d, laps));
                assert_eq!(placed.borrow().len(), 1, "{name} case {case}: placed");
                found += 1;
            } else {
                r.cpu.set_f(12, by);
                r.call(0x118c08).unwrap_or_else(|e| panic!("{name} case {case}: {e}"));
                advance_on_path(&paths, &mut f, by);
            }
            changed += (f.path != path0) as u32;
            let m = &mut r.mem;
            let theirs = (
                ((m.read_u32(rider + 0x57a4) - base) / 0x40) as usize,
                m.read_u32(rider + 0x340), m.read_u32(rider + 0x344), m.read_u32(rider + 0x348),
                get4(m, rider + 0x350), get4(m, rider + 0x360), m.read_u32(rider + 0x370),
                get4(m, rider + 0x390), m.read_u32(rider + 0x3a0), m.read_u32(rider + 0x3a4) as i32,
            );
            let ours = (f.path, f.last.to_bits(), f.along.to_bits(), f.off.to_bits(), bits(f.near), bits(f.ahead), f.heading.to_bits(), bits(f.cache.start), f.cache.along.to_bits(), f.cache.index);
            assert_eq!(theirs, ours, "{name} case {case}: nearest line {nearest}, path {k}, by {by}");
            let theirs = (((m.read_u32(rider + 0x57a0) - lbase) / 0x3c) as usize, m.read_u32(rider + 0x374), m.read_u32(rider + 0x378), get4(m, rider + 0x3b0), m.read_u32(rider + 0x3c0), m.read_u32(rider + 0x3c4) as i32);
            let ours = (rp.line, rp.to_finish.to_bits(), rp.best.to_bits(), bits(rp.cache.start), rp.cache.along.to_bits(), rp.cache.index);
            assert_eq!(theirs, ours, "{name} case {case}: race place, nearest line {nearest}, laps {laps}");
        }
        println!("{name}: reset placing checked");
    }
    println!("moved {moved}, blocked {blocked}, path changed {changed}, nearest line {found}");
    assert!(moved > 100 && blocked > 10 && changed > 100 && found > 1000, "moved {moved}, blocked {blocked}, changed {changed}, found {found}");
}

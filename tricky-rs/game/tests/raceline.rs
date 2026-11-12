//! Board row E1b2g7a: `Boarder_UpdateRaceLineProgress` 0x118298 and `Boarder_SelectRaceLine` 0x118610 (with
//! `AIP_FindNearestLines` 0x198110 under it) run in the function runner on the player's SLUS_203.26 (TRICKY_ELF)
//! against `tricky_game::raceline`, bit for bit, on every course's own race lines loaded by the game's own loader
//! (the `.aip` from the player's disc, TRICKY_DISC; skipped without either). The rider's `cBoarder::vf10` is a stub
//! that logs its event and changes rider+0x384 (the same in both), so its re-reads are checked too.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aipath::{Cache, RaceLines, RacePlace};
use tricky_game::raceline::{select_race_line, update_progress, Event, RaceRider};

const VF10: u32 = 0x4000;

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
    fn chance(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

fn courses() -> Vec<(&'static str, Vec<u8>)> {
    let Ok(disc) = std::env::var("TRICKY_DISC") else { return Vec::new() };
    let Ok(d) = tricky_data::disc::Disc::open(&disc) else { return Vec::new() };
    let mut files = tricky_data::big::Files::new(d);
    let mut out = Vec::new();
    for c in ["gari", "snowdream", "elysium", "mesablanca", "merqury", "megaplex", "aloha", "alaska", "pipedream", "untracked"] {
        let Some(name) = tricky_data::aip::big_name(c) else { continue };
        let Some(big) = files.load(&format!("data/models/{name}.big")).ok().flatten() else { continue };
        let Some(entry) = tricky_data::big::entries(&big).into_iter().find(|e| e.name.eq_ignore_ascii_case(&format!("Data/models/{name}.aip"))) else { continue };
        let data = tricky_data::big::file_data(&big, &entry.name).unwrap();
        out.push((c, tricky_data::compress::load(data.to_vec()).unwrap_or_else(|| data.to_vec())));
    }
    out
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
}

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

/// A rider somewhere about the course's lines.
fn random_rider(g: &mut Rng, lines: &RaceLines) -> RaceRider {
    let n = lines.lines.len();
    let line = g.next() as usize % n;
    let l = &lines.lines[line];
    let d = g.f(-300.0, l.length() + 300.0);
    let on = lines.lines[g.next() as usize % n].position_at(g.f(0.0, l.length())).0;
    let base = if g.chance(4) { on } else { l.position_at(d).0 };
    let spread = if g.chance(3) { 200.0 } else { 1500.0 };
    let pos = [base[0] + g.f(-spread, spread), base[1] + g.f(-spread, spread), base[2] + g.f(-300.0, 300.0), if g.chance(8) { g.f(-2.0, 2.0) } else { 1.0 }];
    let vel = if g.chance(20) {
        [0.0; 4]
    } else {
        [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-1000.0, 1000.0), if g.chance(8) { g.f(-5.0, 5.0) } else { 0.0 }]
    };
    let to_finish = lines.to_finish[line] - d + if g.chance(2) { 0.0 } else { g.f(-2500.0, 2500.0) };
    let best = if g.chance(3) { to_finish } else { to_finish + g.f(-lines.to_finish[lines.main], 2000.0) };
    let mut cache = Cache::default();
    if g.chance(2) {
        let mut near = [0.0; 4];
        let q = [pos[0] + g.f(-800.0, 800.0), pos[1] + g.f(-800.0, 800.0), pos[2], 1.0];
        l.project_from(q, &mut near, &mut cache);
    }
    let pick = |g: &mut Rng| match g.next() % 4 {
        0 => 0,
        1 => g.next(),
        2 => 1 << (g.next() % 32),
        _ => g.next() & 0x0180_4000,
    };
    RaceRider {
        pos,
        vel,
        laps_left: (g.next() % 4) as i32 - 1,
        place: RacePlace { line, to_finish, best, cache },
        events_prev: pick(g),
        events: pick(g),
        ai: if g.chance(3) { (g.next() % 4) as i32 } else { -1 },
    }
}

#[test]
fn race_line_progress_is_the_originals() {
    let courses = courses();
    if courses.is_empty() || std::env::var_os("TRICKY_ELF").is_none() {
        return;
    }
    let mut g = Rng(0x51e7_93a0_c4d2_8b61);
    // changed line, events passed on, vf10 calls, direct line choices
    let mut seen = [0u32; 4];
    for (course, bytes) in courses {
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for f in [0x118298u32, 0x118610, 0x198110, 0x197478, 0x197250, 0x1979f8, 0x197b50, 0x197830, 0x197ef0, 0x197910, 0x197b90, 0x1969b0, 0x1969d8, 0x102f50] {
            r.allow(f);
        }
        load_set(&mut r, &bytes);
        let aip = tricky_data::aip::read(&bytes).unwrap();
        let lines = RaceLines::from_aip(&aip);
        let lbase = r.mem.read_u32(0x337fb8 + 0xc);
        assert_eq!(r.mem.read_u32(0x337fb8 + 8) as usize, lines.lines.len(), "{course}: line count");
        for (k, t) in lines.to_finish.iter().enumerate() {
            assert_eq!(r.mem.read_u32(lbase + 0x3c * k as u32 + 0x38), t.to_bits(), "{course}: line {k} to finish");
        }

        let rider = r.mem.alloc(0x6000);
        let vt = r.mem.alloc(0x100);
        r.mem.write_u32(rider, vt);
        r.mem.write_u16(vt + 0x58, 0);
        r.mem.write_u32(vt + 0x5c, VF10);
        // the race's tick: gApp → +0x730 → +0x1c → +0x18
        let (app, a, b) = (r.mem.alloc(0x800), r.mem.alloc(0x40), r.mem.alloc(0x40));
        r.mem.write_u32(0x337c58, app);
        r.mem.write_u32(app + 0x730, a);
        r.mem.write_u32(a + 0x1c, b);

        let log: Rc<RefCell<Vec<(u32, Event)>>> = Rc::default();
        let flips: Rc<RefCell<Vec<u32>>> = Rc::default();
        {
            let (log, flips) = (log.clone(), flips.clone());
            r.stub(VF10, move |c, m| {
                let (a0, a1) = (c.gpr(4) as u32, c.gpr(5) as u32);
                log.borrow_mut().push((a0, [m.read_u32(a1), m.read_u32(a1 + 4)]));
                let f = flips.borrow_mut().pop().unwrap_or(0);
                let v = m.read_u32(rider + 0x384);
                m.write_u32(rider + 0x384, v ^ f);
            });
        }

        for case in 0..6000 {
            let before = random_rider(&mut g, &lines);
            let frame = if g.chance(3) { (g.next() % 1000 * 60) as i32 } else { (g.next() % 100_000) as i32 };
            let fl: Vec<u32> = (0..40).map(|_| if g.chance(3) { 1 << (g.next() % 32) } else { 0 }).collect();
            let direct = g.chance(4);
            let forced = g.chance(2);

            let m = &mut r.mem;
            put4(m, rider + 0x140, before.pos);
            put4(m, rider + 0x150, before.vel);
            m.write_u32(rider + 0x114, before.laps_left as u32);
            m.write_u32(rider + 0x57a0, lbase + 0x3c * before.place.line as u32);
            m.write_f32(rider + 0x374, before.place.to_finish);
            m.write_f32(rider + 0x378, before.place.best);
            put4(m, rider + 0x3b0, before.place.cache.start);
            m.write_f32(rider + 0x3c0, before.place.cache.along);
            m.write_u32(rider + 0x3c4, before.place.cache.index as u32);
            m.write_u32(rider + 0x384, before.events_prev);
            m.write_u32(rider + 0x388, before.events);
            m.write_u32(rider + 0x418, before.ai as u32);
            m.write_u32(b + 0x18, frame as u32);
            log.borrow_mut().clear();
            *flips.borrow_mut() = fl.clone();
            r.cpu.set_gpr(4, rider as u64);
            r.cpu.set_gpr(5, forced as u64);
            r.call(if direct { 0x118610 } else { 0x118298 }).unwrap_or_else(|e| panic!("{course} case {case}: {e}"));

            let mut ours = before;
            let mut our_log: Vec<(u32, Event)> = Vec::new();
            let mut our_flips = fl.clone();
            if direct {
                select_race_line(&lines, &mut ours, forced);
            } else {
                update_progress(&lines, &mut ours, frame, &mut |rr, ev| {
                    our_log.push((rider, ev));
                    rr.events_prev ^= our_flips.pop().unwrap_or(0);
                });
            }

            let m = &mut r.mem;
            let line_at = m.read_u32(rider + 0x57a0);
            assert_eq!((line_at - lbase) % 0x3c, 0, "{course} case {case}: line pointer");
            let theirs = (
                ((line_at - lbase) / 0x3c) as usize,
                m.read_u32(rider + 0x374),
                m.read_u32(rider + 0x378),
                bits(get4(m, rider + 0x3b0)),
                m.read_u32(rider + 0x3c0),
                m.read_u32(rider + 0x3c4) as i32,
                m.read_u32(rider + 0x384),
                m.read_u32(rider + 0x388),
            );
            let mine = (
                ours.place.line,
                ours.place.to_finish.to_bits(),
                ours.place.best.to_bits(),
                bits(ours.place.cache.start),
                ours.place.cache.along.to_bits(),
                ours.place.cache.index,
                ours.events_prev,
                ours.events,
            );
            assert_eq!(theirs, mine, "{course} case {case} (direct {direct}, forced {forced}, frame {frame}): {before:?}");
            assert_eq!(*log.borrow(), our_log, "{course} case {case}: vf10 calls; {before:?}");
            seen[0] += (ours.place.line != before.place.line) as u32;
            seen[1] += our_log.iter().any(|(_, e)| e[1] != 0 || (e[0] as usize) < 32 && before.events_prev >> e[0] & 1 == 0) as u32;
            seen[2] += !our_log.is_empty() as u32;
            seen[3] += direct as u32;
        }
    }
    eprintln!("changed line / events / vf10 calls / direct: {seen:?}");
    assert!(seen[0] > 500 && seen[1] > 100 && seen[2] > 1000, "{seen:?}");
}

/// Row E1b2g7: the app's race lines (`aiglue::race_lines_of`, from the level's form of the `.aip`) are the game's
/// (`RaceLines::from_aip`), and a rider started at the main line's start (`raceline::start`) and moved down it gets
/// its events' bits.
#[test]
fn app_race_lines_are_the_games() {
    for (c, bytes) in courses() {
        let aip = tricky_data::aip::read(&bytes).unwrap();
        let game = RaceLines::from_aip(&aip);
        let app = tricky_game::aiglue::race_lines_of(&tricky_data::level::AiFile::from_aip(&aip)).expect(c);
        assert_eq!(app, game, "{c}");
        let main = &game.lines[game.main];
        let mut none = |_: &mut RaceRider, _: Event| {};
        let at = |d: f32| main.position_at(d).0;
        let mut r = tricky_game::raceline::start(&game, 0, at(0.0), [0.0; 4], -1, 0, &mut none);
        let (mut seen, mut d) = (0, 0.0);
        while d < main.length() {
            r.pos = at(d);
            r.events_prev = r.events;
            r.events = 0;
            update_progress(&game, &mut r, 0, &mut none);
            seen |= r.events;
            d += 150.0;
        }
        let want = main.events.iter().fold(0u32, |m, e| m | 1 << (e.0 & 31));
        let all = game.lines.iter().flat_map(|l| &l.events).fold(0u32, |m, e| m | 1 << (e.0 & 31));
        println!("{c}: event bits down the main line {seen:#x}, its events' {want:#x}, all lines' {all:#x}");
    }
}

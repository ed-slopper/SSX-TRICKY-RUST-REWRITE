//! The World Circuit (board row C12) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `Circuit_ProcessRaceResult` 0x16bcb0 (with `Circuit_ProcessUnlocks`, `Circuit_GetMedal`,
//! `Circuit_AddPointsToAll`), `Circuit_ScoreEventResult`, `Circuit_AdvanceEvent`, `Circuit_SaveRiderPoints`,
//! `FUN_0016c228`, `Circuit_IsFinalEvent` and the lookups, on random careers: the whole block, the circuit's
//! globals and its unlock list compared after each call.

use std::cell::Cell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::career::{Career, StaticTables, CAREER_BASE, CAREER_SIZE};
use tricky_game::cheats::Cheats;
use tricky_game::circuit::{self, Circuit, Unlock, SETUP};
use tricky_game::rand::Streams;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const OUT: u32 = 0x335658;

/// The game's function on the case's state: a call to a career function not yet allowed is allowed and the case
/// loaded again before the retry (the first try may have written)
fn call(r: &mut Runner, f: u32, args: &[u64], state: &(Career, Circuit)) -> u64 {
    loop {
        load(r, &state.0, &state.1);
        for (k, a) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *a);
        }
        match r.call(f) {
            Ok(()) => return r.cpu.gpr(2),
            Err(r5900::Error::UnstubbedCall { target, .. }) if (0x161000..0x170000).contains(&target) => r.allow(target),
            Err(e) => panic!("{f:#x}: {e}"),
        }
    }
}

/// A random career with some players in slots and a finished race to score
fn random_career(g: &mut Rng) -> Career {
    let mut c = Career::default();
    let mut s = Streams::default();
    c.init_new_game(0, &mut s, None);
    let cheats = Cheats::default();
    let mut players: Vec<i32> = (0..4).collect();
    for i in (1..4).rev() {
        players.swap(i, g.below(i as u32 + 1) as usize);
    }
    let n = 1 + g.below(4) as usize;
    let mut slots = Vec::new();
    for k in 0..n {
        let k = if g.below(4) == 0 { g.below(4) as i32 } else { k as i32 };
        c.assign_player_rider(k, players[slots.len() % 4], g.below(12) as i32, &cheats);
        slots.push(k);
    }
    let active: Vec<usize> = (0..4).filter(|&k| c.is_player_active(k)).collect();
    let me = active[g.below(active.len() as u32) as usize];
    c.w32(SETUP + 0x10, CAREER_BASE + 0x30fc + 0x44 * me as u32);
    c.w32(SETUP + 0x14, g.below(4));
    c.w32(SETUP + 0x18, g.next());
    for k in 0..12 {
        c.w32(SETUP + 0x1c + 4 * k, (g.below(120) as i32 - 10) as u32);
    }
    // the race
    let count = 1 + g.below(6) as usize;
    c.w32(0x27dc, count as u32);
    let mode = [4u32, 5, 4, 5, 4, 5, 0, 1, 2, 3, 6, 7][g.below(12) as usize];
    c.w32(0x27dc + 0x320, mode);
    c.w32(0x27dc + 0x31c, g.below(12));
    let mut order: Vec<u32> = (0..count as u32).collect();
    for i in (1..count).rev() {
        order.swap(i, g.below(i as u32 + 1) as usize);
    }
    for (i, o) in order.iter().enumerate() {
        c.w32(0x3264 + 4 * i, *o);
    }
    c.w32(0x3264 + 0xac, g.below(700_000));
    let mine = c.r32(c.ptr(c.ptr(SETUP + 0x10) + 0x14));
    for i in 0..count {
        let r = 0x27dc + 0x84 * i;
        let pl = if g.below(3) == 0 { active[g.below(active.len() as u32) as usize] as u8 } else { 0xff };
        c.bytes[r + 0x69] = pl;
        c.w32(r + 0x3c, if g.below(3) == 0 { mine } else { g.below(12) });
    }
    // the players' records
    for &k in &active {
        let rec = c.ptr(0x3114 + 0x44 * k);
        c.w32(rec, g.below(300));
        c.w32(rec + 4, g.below(200));
        c.w32(rec + 8, g.below(200));
        for b in 0..4 {
            c.bytes[rec + 0xc + b] = (40 + g.below(60)) as u8;
        }
        let golds = g.below(4) == 0;
        for course in 0..12 {
            for o in [0x24, 0x25] {
                c.bytes[rec + course * 4 + o] = if golds && g.below(8) != 0 { 0 } else { g.below(4) as u8 };
            }
            for o in [0x26, 0x27] {
                c.bytes[rec + course * 4 + o] = g.below(2) as u8;
            }
        }
        c.w32(rec + 0x20, g.below(3) / 2);
        c.w32(rec + 0x58, g.next() & g.next());
        c.w32(rec + 0x5c, g.next() & g.next());
        for o in [0x18, 0x1c] {
            c.w32(rec + o, if g.below(2) == 0 { 0x3fff_ffff & !(1 << g.below(40)) } else { g.next() });
        }
        for j in 0..12 {
            c.w32(rec + 0x164 + 8 * j, g.below(100));
        }
        c.w32(0x310c + 0x44 * k, g.next() & g.next() & g.next());
    }
    c.w32(8, g.next() & g.next());
    c.w32(0xc, g.next() & g.next());
    c.w32(0x370c, g.below(2));
    c.w32(0x3258, g.below(2));
    c
}

fn load(r: &mut Runner, c: &Career, ci: &Circuit) {
    for (k, b) in c.bytes.iter().enumerate() {
        r.mem.write_u8(CAREER_BASE + k as u32, *b);
    }
    r.mem.write_u32(0x3353f0, ci.showoff_final);
    r.mem.write_u32(0x3353f4, ci.race_final);
    r.mem.write_u32(0x3353f8, ci.unlocks.len() as u32);
    for ch in 0..12 {
        r.mem.write_u32(OUT + 0x10 * ch + 0xc, ci.out[ch as usize]);
    }
}

fn compare(r: &mut Runner, c: &Career, ci: &Circuit, what: &str) {
    let mut diffs = Vec::new();
    for k in 0..CAREER_SIZE {
        let b = (r.mem.read_u32((CAREER_BASE + k as u32) & !3) >> (8 * ((CAREER_BASE as usize + k) & 3))) as u8;
        if b != c.bytes[k] && diffs.len() < 16 {
            diffs.push(format!("+{k:#x}: game {b:#x} ours {:#x}", c.bytes[k]));
        }
    }
    assert!(diffs.is_empty(), "{what}:\n{}", diffs.join("\n"));
    assert_eq!(r.mem.read_u32(0x3353f0), ci.showoff_final, "{what}: show-off final");
    assert_eq!(r.mem.read_u32(0x3353f4), ci.race_final, "{what}: race final");
    let n = r.mem.read_u32(0x3353f8) as usize;
    let theirs: Vec<Unlock> = (0..n as u32)
        .map(|i| Unlock { who: r.mem.read_u32(0x335400 + 12 * i) as i32, kind: r.mem.read_u32(0x335404 + 12 * i), value: r.mem.read_u32(0x335408 + 12 * i) as i32 })
        .collect();
    assert_eq!(theirs, ci.unlocks, "{what}: unlocks");
    for ch in 0..12u32 {
        assert_eq!(r.mem.read_u32(OUT + 0x10 * ch + 0xc), ci.out[ch as usize], "{what}: out {ch}");
    }
}

#[test]
fn circuit_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let events = Rc::new(Cell::new(0u32));
    let e = events.clone();
    r.stub(0x16d300, move |_, _| e.set(e.get() + 1));
    // the race object's per-rider table FUN_0016c228 reads (gApp+0x730 → +0x1c → +0xc4 + 4·rider → +0x50 + 0x20·j)
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x40);
    r.mem.write_u32(app + 0x730, game);
    let tbl = r.mem.alloc(0x100);
    r.mem.write_u32(game + 0x1c, tbl);
    let views = r.mem.alloc(6 * 0x100);
    for i in 0..6 {
        r.mem.write_u32(tbl + 0xc4 + 4 * i, views + 0x100 * i + 0x40);
    }
    let mut g = Rng(0xc12c_0000_0000_0001);
    let mut counts = [0u32; 12];
    // the static tables, the game's and ours
    let mut tables = StaticTables::default();
    let mut cheats = Cheats::default();
    tables.init(&mut cheats);
    r.mem.write_u32(0x335258, 1);
    r.mem.write_u32(0x3352b8, 0);
    r.mem.write_u32(0x3352ac, 0);
    r.call(0x169e38).unwrap();
    // strcpy (MMI loops): the C library's answer
    r.stub(0x2fd14c, |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(5) as u32);
        for k in 0.. {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            m.write_u8(d + k, b);
            if b == 0 {
                break;
            }
        }
        c.set_gpr(2, d as u64);
    });
    let name = r.mem.alloc(0x40);
    let mut kinds = [0u32; 10];
    for case in 0..20_000 {
        let mut c = random_career(&mut g);
        let mut ci = Circuit { showoff_final: g.below(2), race_final: g.below(2), unlocks: Vec::new(), out: [0; 12] };
        for o in ci.out.iter_mut() {
            *o = (g.below(4) == 0) as u32;
        }
        let state = (c.clone(), ci.clone());
        let setup = (CAREER_BASE + SETUP as u32) as u64;
        let op = g.below(12);
        counts[op as usize] += 1;
        let what = format!("case {case} op {op}");
        match op {
            0..=2 => {
                call(&mut r, 0x16bcb0, &[setup], &state);
                ci.process_race_result(&mut c);
                for u in &ci.unlocks {
                    kinds[u.kind as usize] += 1;
                }
            }
            3 => {
                let theirs = call(&mut r, 0x16d0b0, &[setup], &state) != 0;
                assert_eq!(theirs, ci.score_event_result(&mut c), "{what}: goes on");
            }
            4 => {
                let before = events.get();
                let theirs = call(&mut r, 0x16c3b0, &[setup], &state) as u32;
                let mut ours_events = 0;
                assert_eq!(theirs, ci.advance_event(&mut c, &mut |_, _| ours_events += 1), "{what}: result");
                assert_eq!(events.get() - before, ours_events, "{what}: next heat set up");
            }
            5 => {
                let mut v = [[0i8; 8]; 6];
                for (i, row) in v.iter_mut().enumerate() {
                    for (j, b) in row.iter_mut().enumerate() {
                        *b = g.next() as i8;
                        r.mem.write_u8(views + 0x100 * i as u32 + 0x40 + 0x50 + 0x20 * j as u32, *b as u8);
                    }
                    // the row the game reads with no player rider (j = −1)
                    r.mem.write_u8(views + 0x100 * i as u32 + 0x40 + 0x30, 0x55);
                }
                let n = c.r32(0x27dc) as i32;
                call(&mut r, 0x16c228, &[setup, n as u64, (CAREER_BASE + 0x27e0) as u64], &state);
                circuit::take_rider_points(&mut c, n, 0x27e0, &|i, j| if j < 0 { 0x55 } else { v[i][j as usize] });
            }
            6 => {
                call(&mut r, 0x16c328, &[setup], &state);
                circuit::save_rider_points(&mut c);
            }
            7 => {
                let p = (g.below(30) as i32 - 15) as u32;
                call(&mut r, 0x16d1f8, &[setup, p as u64], &state);
                circuit::add_points_to_all(&mut c, p as i32);
            }
            9 => {
                if g.below(4) == 0 {
                    c.w32(0x2afc, 6);
                }
                let state = (c.clone(), ci.clone());
                let before = events.get();
                call(&mut r, 0x163ab0, &[CAREER_BASE as u64], &state);
                let mut ours_events = 0;
                c.finish_race(&mut ci, &cheats, &mut |_, _| ours_events += 1);
                assert_eq!(events.get() - before, ours_events, "{what}: next heat set up");
            }
            10 => {
                // the player's words (profile words in range, boards and outfits past them too)
                let active: Vec<usize> = (0..4).filter(|&k| c.is_player_active(k)).collect();
                let k = active[g.below(active.len() as u32) as usize];
                let p = c.player_id(k);
                let prof = c.ptr(0x3110 + 0x44 * k);
                c.w32(prof, g.below(11));
                c.w32(prof + 0xc, g.below(7));
                c.w32(prof + 0x10, g.below(12));
                let state = (c.clone(), ci.clone());
                let pa = CAREER_BASE as u64;
                let pp = p as i64 as u64;
                assert_eq!(call(&mut r, 0x164ac8, &[pa, pp], &state) as u32, c.player_board_word(&tables, p), "{what}: board word");
                let b = g.below(20) as usize;
                assert_eq!(call(&mut r, 0x164b10, &[pa, pp, b as u64], &state) as u32, c.player_board_word_for(&tables, p, b), "{what}: board word for");
                for (f, ffor, w) in [(0x164b60u32, 0x164ba8u32, 0usize), (0x164bf8, 0x164c40, 2), (0x164c90, 0x164cd8, 1), (0x164d28, 0x164d70, 3)] {
                    assert_eq!(call(&mut r, f, &[pa, pp], &state) as u32, c.player_outfit_word(&tables, p, w), "{what}: outfit word {w}");
                    let o = g.below(12) as usize;
                    assert_eq!(call(&mut r, ffor, &[pa, pp, o as u64], &state) as u32, c.player_outfit_word_for(&tables, p, o, w), "{what}: outfit word {w} for {o}");
                }
                let mut state = state;
                for (get, set, w) in [(0x164f40u32, 0x164f78u32, 1usize), (0x164ec0, 0x164ef8, 2)] {
                    assert_eq!(call(&mut r, get, &[pa, pp], &state) as u32, c.player_record_word(p, w), "{what}: record word {w}");
                    let v = g.next();
                    call(&mut r, set, &[pa, pp, v as u64], &state);
                    c.set_player_record_word(p, w, v);
                    compare(&mut r, &c, &ci, &what);
                    state = (c.clone(), ci.clone());
                }
            }
            11 => {
                let len = g.below(30) as usize;
                let text: Vec<u8> = (0..len).map(|_| 1 + g.below(255) as u8).chain([0]).collect();
                for (i, b) in text.iter().enumerate() {
                    r.mem.write_u8(name + i as u32, *b);
                }
                call(&mut r, 0x163f10, &[CAREER_BASE as u64, name as u64], &state);
                c.set_pending_video(&text);
            }
            _ => {
                let theirs = call(&mut r, 0x16bbd0, &[setup], &state) != 0;
                assert_eq!(theirs, Circuit::is_final_event(&c), "{what}: final");
                let course = g.below(14);
                let at = r.mem.alloc(16);
                let e = call(&mut r, 0x16ba18, &[course as u64, at as u64], &state) as u32;
                assert_eq!(r.mem.read_u32(at) as usize, circuit::find_entry(course), "{what}: entry {course}");
                assert_eq!(e, 0x3352f0 + 0x10 * circuit::find_entry(course).min(7) as u32, "{what}: entry (the last when none)");
                let e = call(&mut r, 0x16ba60, &[course as u64, at as u64], &state) as u32;
                assert_eq!(r.mem.read_u32(at) as usize, circuit::threshold_entry(course), "{what}: thresholds {course}");
                assert_eq!(e, 0x335370 + 0x10 * circuit::threshold_entry(course).min(7) as u32, "{what}: thresholds (the last when none)");
                let p = g.below(10) as i32 - 3;
                assert_eq!(call(&mut r, 0x16bb58, &[p as i64 as u64], &state) as i32, circuit::clamp_medal_rank(p));
            }
        }
        compare(&mut r, &c, &ci, &what);
    }
    eprintln!("ops {counts:?}, unlocks by kind {kinds:?}");
    assert!(kinds.iter().all(|k| *k > 0), "every kind of unlock reached");
}

/// The course info lookups (`Course_Get*String`, the name, hint and description ids, the map markers) for every
/// course and past them
#[test]
fn course_info_is_the_original() {
    use tricky_game::courses::{self, Info};
    let Some(mut r) = Runner::from_env() else { return };
    r.stub(0x2c5160, |c, _| {
        let a = c.gpr(5);
        c.set_gpr(2, a)
    });
    let app = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, app);
    let mut ask = |f: u32, course: u64| {
        r.cpu.set_gpr(5, course);
        r.call(f).unwrap();
        (r.cpu.gpr(2) as u32, r.cpu.f(0).to_bits())
    };
    for course in (0..16u64).chain([1 << 40, u64::MAX, 0x8000_0000]) {
        for (f, info) in [(0x16b330u32, Info::Difficulty), (0x16b3f8, Info::Location), (0x16b4c0, Info::VerticalDrop), (0x16b588, Info::Length)] {
            assert_eq!(ask(f, course).0, courses::info_string_id(info, course), "{f:#x} course {course}");
        }
        for (f, t) in [(0x16b650u32, &courses::NAME_IDS), (0x16b860, &courses::UNLOCK_HINT_IDS), (0x16b8e0, &courses::DESCRIPTION_IDS)] {
            assert_eq!(ask(f, course).0, courses::by_course(t, course), "{f:#x} course {course}");
        }
        for (f, t) in [(0x16b6d8u32, &courses::MAP_MARKERS[0]), (0x16b7b0, &courses::MAP_MARKERS[1])] {
            assert_eq!(ask(f, course).1, courses::by_course(t, course), "{f:#x} course {course}");
        }
    }
}

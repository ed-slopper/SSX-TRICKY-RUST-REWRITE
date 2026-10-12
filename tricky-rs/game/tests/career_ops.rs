//! The career's player slots, attributes, unlocks, trick book, names and records (board row E12b) against the
//! game in the function runner (TRICKY_ELF; skipped without it): from a new game, 100,000 random operations
//! (`Career_AssignPlayerRider`, `Career_ClearPlayerSlot`, `Career_UpdateAttributes`, spending and refunding points,
//! `Career_ResetPlayerRecord`, `Career_MergePlayerRecords`, the flags, `Career_SetPlayerName`, the profile setters)
//! and queries, with the block's fields shaken between them, on the block at its own address (0x32b390) and on
//! `tricky_game::career`; the whole block and every answer compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::career::{Career, RecordsEnv, CAREER_BASE, CAREER_SIZE};
use tricky_game::cheats::Cheats;
use tricky_game::rand::Streams;
use tricky_game::trickdata::BOARDS;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

struct Rec {
    human: [bool; 6],
    circuit: usize,
}
impl RecordsEnv for Rec {
    fn rider_is_human(&self, i: usize) -> bool {
        self.human[i]
    }
    fn circuit_race_result(&mut self, _: &mut Career) {
        self.circuit += 1;
    }
}

/// Bytes the comparison skips: a high-score entry's name buffer past its terminator (the game copies its stack there)
fn masked(c: &Career, k: usize) -> bool {
    if !(0x16c4..0x16c4 + 13 * 0x150).contains(&k) {
        return false;
    }
    let rel = (k - 0x16c4) % 0x150;
    let table = rel / 0x70;
    let inner = rel % 0x70;
    if table > 2 || inner >= 0x64 {
        return false;
    }
    let e = k - inner % 0x14;
    let off = inner % 0x14;
    off < 0xc && c.bytes[e..e + off].contains(&0)
}

fn byte(r: &mut Runner, a: u32) -> u8 {
    (r.mem.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

fn call(r: &mut Runner, f: u32, args: &[u64]) -> u64 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *a);
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e}"));
    r.cpu.gpr(2)
}

#[test]
fn career_ops_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [
        0x1617a0u32, 0x161b40, 0x2cc808, 0x163358, 0x166600, 0x1668f0, 0x23d4a8, 0x2401e0, 0x165660, 0x16b298, 0x167da0, 0x167208, 0x166c70, 0x161928, 0x161a98, 0x166a58, 0x2fca28, 0x1635b8,
        0x1664e8, 0x166550, 0x16bba0, 0x1637d0, 0x164398, 0x1644e8, 0x1637f8, 0x2ce04c, 0x163e70, 0x165400, 0x161840, 0x16bca8, 0x166580, 0x164968, 0x164df8, 0x164e78, 0x164ef8, 0x164f78,
        0x164dc0, 0x164e40, 0x164ec0, 0x164f40, 0x163b10, 0x163b38, 0x163d30, 0x163d98, 0x163e28, 0x164368, 0x164840, 0x164730, 0x163f88, 0x164288, 0x163f18, 0x164258, 0x164178, 0x164060,
        0x1641f8, 0x164108, 0x164110, 0x164020, 0x165468, 0x166598, 0x164fc0, 0x166968, 0x164918, 0x166cb0, 0x166fa0, 0x166fe0, 0x167010, 0x167038, 0x167060,
    ] {
        r.allow(a);
    }
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
    let reltab = r.mem.alloc(0x900);
    r.stub(0x2ccf70, move |c, _| c.set_gpr(2, reltab as u64));
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x58, 0);
    r.mem.write_u32(0x343050, 0);
    r.mem.write_u32(0x3352b4, 0);
    let c = CAREER_BASE;
    // the game's new game, then ours from the same random stream
    let mut streams = Streams::default();
    streams.seed_fx(0x1234_5678);
    for (k, w) in streams.fx.s.iter().enumerate() {
        r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
    }
    let mut cheats = Cheats::default();
    for a in [0x3352a0u32, 0x3352ac] {
        r.mem.write_u32(a, 0);
    }
    loop {
        r.cpu.set_gpr(4, c as u64);
        match r.call(0x1617a0) {
            Ok(()) => break,
            Err(r5900::Error::UnstubbedCall { target, .. }) if target >= 0x2f0000 => r.allow(target),
            Err(e) => panic!("{e}"),
        }
    }
    let mut ours = Career::default();
    for k in 0..CAREER_SIZE as u32 {
        ours.bytes[k as usize] = byte(&mut r, c + k);
    }
    let name_buf = r.mem.alloc(0x40);
    let stats = r.mem.alloc(0xc0);
    let rider_structs = r.mem.alloc(0x600 * 6);
    let human: std::rc::Rc<std::cell::RefCell<[bool; 6]>> = std::rc::Rc::new(std::cell::RefCell::new([true; 6]));
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(app + 0x730, race);
    let h = human.clone();
    r.stub(0x1814a8, move |cpu, m| {
        let i = cpu.gpr(5) as u32;
        let at = rider_structs + 0x600 * i;
        m.write_u32(at + 0x418, if h.borrow()[i as usize] { 1 } else { 0 });
        cpu.set_gpr(2, at as u64);
    });
    let circuit = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let cc = circuit.clone();
    r.stub(0x16bcb0, move |_, _| cc.set(cc.get() + 1));
    let out = r.mem.alloc(0x80);
    let mut g = Rng(0xca2e_e2b1_0000_0001);
    let mut counts = std::collections::BTreeMap::<u32, usize>::new();
    let ops = 100000;
    for step in 0..ops {
        // shake a field (both sides)
        if g.next() % 3 == 0 {
            let k = (g.next() % 4) as usize * 0x44;
            let rec = (0x100 + 0x1c4 * (g.next() % 12)) as usize;
            let (at, v): (usize, u32) = match g.next() % 12 {
                0 => (0x310c + k, g.next() & 0xff),
                1 => (rec + 0x1c, g.next()),
                2 => (rec + 4, g.next() % 160),
                3 => (rec + 8, g.next() % 160),
                4 => (rec + 0x18, g.next()),
                5 => (rec, g.next() % 200),
                6 => (0x2afc, g.next() % 10),
                7 => (0x27dc, g.next() % 7),
                8 => (0x2844 + 0x84 * (g.next() % 6) as usize, g.pick(&[0xff00u32, 0x0000, 0x0100, 0x0200, 0x0300])),
                9 => (rec + 0x24 + 4 * (g.next() % 13) as usize, g.next() & 0x0303_0303),
                10 => (g.pick(&[0x2af8usize, 0x327c, 0x3280, 0x3284, 0x3310, 0x33cc, 0x3488]), g.next() % 40000),
                _ => (rec + 0x58 + 4 * (g.next() % 2) as usize, g.next()),
            };
            ours.w32(at, v);
            r.mem.write_u32(c + at as u32, v);
        }
        if g.next() % 50 == 0 {
            cheats.max_attrib = g.next() % 2;
            cheats.unlock_everything = g.next() % 2;
            r.mem.write_u32(0x3352ac, cheats.max_attrib);
            r.mem.write_u32(0x3352a0, cheats.unlock_everything);
        }
        let p = g.pick(&[0i32, 1, 2, 3, -1]);
        // most queries need the player to have a slot
        let has = ours.find_player_slot(p as i8) >= 0 && p >= 0;
        let op = g.next() % 36;
        let what = |s: &str| format!("step {step} op {op} {s}");
        match op {
            0..=3 => {
                let (slot, rider) = (g.pick(&[-1i32, 0, 1, 2, 3]), (g.next() % 12) as i32);
                call(&mut r, 0x1635b8, &[c as u64, slot as i64 as u64, p as i64 as u64, rider as u64]);
                ours.assign_player_rider(slot, p, rider, &cheats);
            }
            4 => {
                let slot = (g.next() % 4) as usize;
                call(&mut r, 0x1637d0, &[c as u64, slot as u64]);
                ours.clear_player_slot(slot);
            }
            5 => {
                call(&mut r, 0x166a58, &[c as u64]);
                ours.update_attributes(&cheats);
            }
            6 | 7 | 26..=29 if has => {
                let i = (g.next() % 4) as usize;
                let (f, v) = if op % 2 == 0 { (0x164398, ours.spend_attribute_point(p as i8, i)) } else { (0x1644e8, ours.refund_attribute_point(p as i8, i)) };
                let got = call(&mut r, f, &[c as u64, p as u64, i as u64]);
                assert_eq!(got != 0, v, "{}", what("spend / refund"));
                *counts.entry(100 + op).or_default() += v as usize;
            }
            8 => {
                call(&mut r, 0x1637f8, &[c as u64]);
                ours.merge_player_records();
            }
            9 if has => {
                call(&mut r, 0x163e70, &[c as u64, p as u64]);
                ours.reset_player_record(p as i8, &cheats);
            }
            10 if has => {
                call(&mut r, 0x165400, &[c as u64, p as i64 as u64]);
                ours.clear_player_flags(p);
            }
            11 => {
                call(&mut r, 0x161840, &[c as u64]);
                ours.clear_all_player_flags();
            }
            12 if has => {
                let n: Vec<u8> = match g.next() % 4 {
                    0 => b"   ".to_vec(),
                    1 => b"".to_vec(),
                    _ => (0..1 + g.next() % 8).map(|_| g.pick(&[b'A', b'Z', b' ', b'7', b'q'])).collect(),
                };
                for (k, b) in n.iter().chain([0u8].iter()).enumerate() {
                    r.mem.write_u8(name_buf + k as u32, *b);
                }
                call(&mut r, 0x164968, &[c as u64, p as u64, name_buf as u64]);
                ours.set_player_name(p as i8, &n);
            }
            13 if has => {
                let (w, f) = g.pick(&[(1usize, 0x164f78u32), (2, 0x164ef8), (3, 0x164df8), (4, 0x164e78)]);
                let v = g.next() % 8;
                call(&mut r, f, &[c as u64, p as u64, v as u64]);
                ours.set_profile_word(p as i8, w, v);
            }
            14 if has => {
                let (w, f) = g.pick(&[(1usize, 0x164f40u32), (2, 0x164ec0), (3, 0x164dc0), (4, 0x164e40)]);
                assert_eq!(call(&mut r, f, &[c as u64, p as u64]) as u32, ours.profile_word(p as i8, w), "{}", what("profile"));
            }
            15 => {
                let ch = g.next() % 32;
                assert_eq!(call(&mut r, 0x163b10, &[c as u64, ch as u64]) != 0, ours.is_character_unlocked(ch, &cheats), "{}", what("character"));
            }
            16 => {
                let b = g.next() % 12;
                assert_eq!(call(&mut r, 0x163b38, &[c as u64, b as u64]) != 0, ours.mode_has(b), "{}", what("mode"));
            }
            17 | 18 if has => {
                let b = g.next() % 14;
                let (f, v) = if op == 17 { (0x163d30, ours.is_board_unlocked(p as i8, b, &cheats)) } else { (0x163d98, ours.is_outfit_unlocked(p as i8, b, &cheats)) };
                assert_eq!(call(&mut r, f, &[c as u64, p as u64, b as u64]) != 0, v, "{}", what("unlock bit"));
            }
            19 => {
                let (mode, course) = (g.pick(&[4i32, 5, 0, 2]), (g.next() % 13) as i32);
                assert_eq!(call(&mut r, 0x163e28, &[c as u64, mode as u64, course as u64]) as u8, ours.course_medal(mode, course), "{}", what("medal"));
            }
            20 if has => {
                assert_eq!(call(&mut r, 0x164368, &[c as u64, p as u64]) as u32, ours.attribute_points(p as i8), "{}", what("points"));
                assert_eq!(call(&mut r, 0x164840, &[c as u64, p as u64]) as u32, ours.player_level(p as i8), "{}", what("level"));
                assert_eq!(call(&mut r, 0x164730, &[c as u64, p as u64]) as u32, ours.player_level_string(p as i8), "{}", what("level string"));
                assert_eq!(call(&mut r, 0x163f88, &[c as u64, p as u64]) as u32, ours.count_player_flags(p as i8), "{}", what("flags"));
            }
            21 => {
                assert_eq!(call(&mut r, 0x164288, &[c as u64]) as u32, ours.count_profiles(), "{}", what("profiles"));
            }
            22 if has => {
                let book = call(&mut r, 0x163f18, &[c as u64, p as u64]);
                let t = g.next() % 32;
                assert_eq!(call(&mut r, 0x164258, &[book, t as u64]) != 0, ours.trick_done(p as i8, t), "{}", what("trick"));
                let page = g.next() % 8;
                assert_eq!(call(&mut r, 0x164178, &[book, page as u64]) as u32, ours.tricks_done_in_page(p as i8, page), "{}", what("page"));
                assert_eq!(call(&mut r, 0x164060, &[book]) as u32, ours.complete_pages(p as i8), "{}", what("pages"));
                assert_eq!(call(&mut r, 0x1641f8, &[book]) as i32, ours.first_undone_trick(p as i8), "{}", what("undone"));
            }
            23 if has => {
                let (rider, board) = ((g.next() % 12) as usize, (g.next() % 12) as usize);
                let slot = ours.find_player_slot(p as i8) as u32;
                let entry = 0x332148 + 4 * (rider as u32 * 60 + board as u32 * 5);
                call(&mut r, 0x165468, &[c as u64, (c + 0x30fc + 0x44 * slot) as u64, entry as u64, out as u64]);
                let theirs: Vec<u32> = (0..16).map(|k| r.mem.read_u32(out + 4 * k)).collect();
                let a = ours.attributes(p as i8, BOARDS[rider][board].bonus, &cheats);
                assert_eq!(theirs, a.iter().flatten().copied().collect::<Vec<_>>(), "{}", what("attributes"));
            }
            24 => {
                let slot = (g.next() % 4) as usize;
                assert_eq!(call(&mut r, 0x166598, &[c as u64, slot as u64]) as i32, ours.find_rider_of_player(slot), "{}", what("rider of player"));
            }
            25 => {
                assert_eq!(call(&mut r, 0x1664e8, &[c as u64, p as i64 as u64]) as i32, ours.slot_last_rider(p), "{}", what("last rider"));
                assert_eq!(call(&mut r, 0x166550, &[c as u64, p as i64 as u64]) as i32, ours.find_player_slot(p as i8), "{}", what("slot"));
            }
            30 => {
                // a finished race: places a permutation of the riders, a mode
                let n = 1 + g.next() as usize % 6;
                let mut order: Vec<u32> = (0..n as u32).collect();
                for i in (1..n).rev() {
                    order.swap(i, g.next() as usize % (i + 1));
                }
                let mode = g.pick(&[2u32, 3, 4, 5, 7, 0]);
                let mut fields: Vec<(usize, u32)> = vec![(0x27dc, n as u32), (0x2afc, mode), (0x2af8, g.next() % 13)];
                for (i, o) in order.iter().enumerate() {
                    fields.push((0x3264 + 4 * i, *o));
                }
                for i in 0..n {
                    fields.push((0x327c + 4 * i, 15000 + g.next() % 30000));
                    fields.push((0x3310 + 0xbc * i, g.next() % 400000));
                    let pl = if (i as i32) < 4 && ours.find_player_slot(i as i8) >= 0 && g.next() % 3 != 0 { i as u32 } else { 0xff };
                    fields.push((0x2844 + 0x84 * i, pl << 8));
                }
                for (at, v) in fields {
                    ours.w32(at, v);
                    r.mem.write_u32(c + at as u32, v);
                }
                // mode 7 takes the winner whoever it is: only with a player winning (else a slot's unset pointers)
                if mode == 7 && ours.r8(0x2845 + 0x84 * order[0] as usize) == 0xff {
                    ours.w32(0x2afc, 2);
                    r.mem.write_u32(c + 0x2afc, 2);
                }
                let hm: [bool; 6] = std::array::from_fn(|_| g.next() % 4 != 0);
                *human.borrow_mut() = hm;
                let before = circuit.get();
                call(&mut r, 0x164fc0, &[c as u64]);
                let mut env = Rec { human: hm, circuit: 0 };
                ours.check_new_records(&mut env, &cheats);
                assert_eq!(circuit.get() - before, env.circuit, "{}", what("circuit"));
            }
            31 => {
                let ch = (g.next() % 12) as usize;
                let mut st = vec![0u8; 0xbc];
                for b in st.iter_mut() {
                    *b = g.next() as u8;
                }
                for o in [0x5cusize, 0x60, 0x64] {
                    let f = (g.next() % 1000) as f32 / 7.0;
                    st[o..o + 4].copy_from_slice(&f.to_le_bytes());
                }
                for o in [0x48usize, 0x1c, 0x10, 0x14, 0x84, 0x88, 0x8c, 0x90, 0x94, 0x98] {
                    let v = g.next() % 500;
                    st[o..o + 4].copy_from_slice(&v.to_le_bytes());
                }
                for (k, b) in st.iter().enumerate() {
                    r.mem.write_u8(stats + k as u32, *b);
                }
                let tag = g.next() % 9;
                call(&mut r, 0x166cb0, &[c as u64, ch as u64, stats as u64, tag as u64]);
                ours.update_best_stats(ch, &st, tag);
            }
            32 => {
                let (ch, course, v) = ((g.next() % 12) as usize, (g.next() % 13) as usize, (g.next() % 60000) as i32);
                match g.next() % 3 {
                    0 => {
                        call(&mut r, 0x166fa0, &[c as u64, ch as u64, course as u64, v as u64]);
                        ours.record_best_time(ch, course, v);
                    }
                    1 => {
                        call(&mut r, 0x166fe0, &[c as u64, ch as u64, course as u64, v as u64]);
                        ours.record_best_score(ch, course, v);
                    }
                    _ => {
                        call(&mut r, 0x167010, &[c as u64, ch as u64, course as u64]);
                        ours.count_course_start(ch, course);
                    }
                }
            }
            33 if has => {
                assert_eq!(call(&mut r, 0x167038, &[c as u64, p as u64]) != 0, ours.wears_outfit_6(p as i8), "{}", what("outfit 6"));
            }
            34 if (0..4).contains(&p) => {
                // at most two player riders in the race (the game reads its stack past two)
                let n = (ours.r32(0x27dc) as usize).min(6);
                let players = (0..n).filter(|i| ours.r8(0x2845 + 0x84 * i) != 0xff).count();
                if players > 2 {
                    continue;
                }
                call(&mut r, 0x167060, &[c as u64, p as u64]);
                ours.select_solo_rider(p, &cheats);
            }
            _ => continue,
        }
        *counts.entry(op).or_default() += 1;
        let mut diffs = vec![];
        for k in 0..CAREER_SIZE as u32 {
            let b = byte(&mut r, c + k);
            if b != ours.bytes[k as usize] && !masked(&ours, k as usize) && diffs.len() < 16 {
                diffs.push(format!("+{k:#x}: game {b:#04x} ours {:#04x}", ours.bytes[k as usize]));
            }
        }
        assert!(diffs.is_empty(), "{}:\n{}", what("block"), diffs.join("\n"));
    }
    eprintln!("ops run: {counts:?}");
}

//! The career's player slots, attributes, unlocks, trick book, names and records (board row E12b) against the
//! game in the function runner (TRICKY_ELF; skipped without it): from a new game, 100,000 random operations
//! (`Career_AssignPlayerRider`, `Career_ClearPlayerSlot`, `Career_UpdateAttributes`, spending and refunding points,
//! `Career_ResetPlayerRecord`, `Career_MergePlayerRecords`, the flags, `Career_SetPlayerName`, the profile setters)
//! and queries, with the block's fields shaken between them, on the block at its own address (0x32b390) and on
//! `tricky_game::career`; the whole block and every answer compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE, CAREER_SIZE};
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
        0x1641f8, 0x164108, 0x164110, 0x164020, 0x165468, 0x166598,
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
                10 => (0xc, g.next()),
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
        let op = g.next() % 30;
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
            _ => continue,
        }
        *counts.entry(op).or_default() += 1;
        let mut diffs = vec![];
        for k in 0..CAREER_SIZE as u32 {
            let b = byte(&mut r, c + k);
            if b != ours.bytes[k as usize] && diffs.len() < 16 {
                diffs.push(format!("+{k:#x}: game {b:#04x} ours {:#04x}", ours.bytes[k as usize]));
            }
        }
        assert!(diffs.is_empty(), "{}:\n{}", what("block"), diffs.join("\n"));
    }
    eprintln!("ops run: {counts:?}");
}

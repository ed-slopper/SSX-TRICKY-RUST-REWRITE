//! The roster builders' pieces against the original in the function runner (TRICKY_ELF; skipped without it).

use r5900::Runner;
use tricky_game::roster::sort_shell_pairs;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

#[test]
fn sort_shell_pairs_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x2cfd90);
    let keys = r.mem.alloc(0x1000);
    let vals = r.mem.alloc(0x1000);
    let mut g = Rng(0x5047_1234_beef_0001);
    for case in 0..20_000 {
        let n = match g.next() % 4 {
            0 => 12,
            1 => (g.next() % 6) as usize,
            _ => (g.next() % 300) as usize,
        };
        let range = [3u32, 20, 10_000, u32::MAX][g.next() as usize % 4];
        let mut k: Vec<i32> = (0..n).map(|_| (g.next() % range) as i32 - (range / 2) as i32).collect();
        let mut v: Vec<i32> = (0..n as i32).collect();
        for i in 0..n {
            r.mem.write_u32(keys + 4 * i as u32, k[i] as u32);
            r.mem.write_u32(vals + 4 * i as u32, v[i] as u32);
        }
        r.cpu.set_gpr(4, n as u64);
        r.cpu.set_gpr(5, keys as u64);
        r.cpu.set_gpr(6, vals as u64);
        r.call(0x2cfd90).unwrap_or_else(|e| panic!("case {case}: {e}"));
        sort_shell_pairs(&mut k, &mut v);
        for i in 0..n {
            assert_eq!(r.mem.read_u32(keys + 4 * i as u32) as i32, k[i], "case {case} n {n}: key {i}");
            assert_eq!(r.mem.read_u32(vals + 4 * i as u32) as i32, v[i], "case {case} n {n}: value {i}");
        }
    }
}

#[test]
fn pre_race_selection_matches_the_original() {
    use tricky_game::rand::Streams;
    use tricky_game::roster::{match_row, PreRaceSel};
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x168720u32, 0x1686a8, 0x1687f0, 0x168a18, 0x1689e0, 0x23d4a8, 0x2401e0, 0x2cd9d8] {
        r.allow(a);
    }
    let tab = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n * 3).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).collect() };
    let t0 = tab(&mut r, 0x37b390, 9);
    let t1 = tab(&mut r, 0x37b400, 0x85);
    let t2 = tab(&mut r, 0x37ba40, 0x9f);
    let sel = r.mem.alloc(0x100);
    let mut g = Rng(0x7ac3_0001_9e37_79b9);
    let (mut hits, mut none) = (0, 0);
    for case in 0..20_000 {
        // MatchRow on its own, on a random row
        let row: Vec<u8> = (0..12).map(|k| match k { 0 => [0u8, 3, 12, 13, 5][g.next() as usize % 5], 1 => [0u8, 2, 13, 14, 8][g.next() as usize % 5], 2 => [1u8, 5, 10, 11][g.next() as usize % 4], 3 => [0u8, 1, 3, 4][g.next() as usize % 4], _ => g.next() as u8 }).collect();
        let want = [g.next() % 14, g.next() % 15, g.next() % 12, g.next() % 5];
        let rowp = r.mem.alloc(16);
        for k in 0..3 {
            r.mem.write_u32(rowp + 4 * k, u32::from_le_bytes(row[4 * k as usize..4 * k as usize + 4].try_into().unwrap()));
        }
        for k in 0..4 {
            r.cpu.set_gpr(4 + k, if k == 0 { sel as u64 } else { want[k - 1] as u64 });
        }
        r.cpu.set_gpr(8, want[3] as u64);
        r.cpu.set_gpr(9, rowp as u64);
        r.call(0x1687f0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 as i32, match_row(want[0], want[1], want[2], want[3], &row), "case {case}: match {row:x?} {want:?}");
        // the whole selection
        let seed = g.next();
        let mut rand = Streams::default();
        rand.seed_fx(seed);
        for (k, w) in rand.fx.s.iter().enumerate() {
            r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
        }
        for k in 0..0x34 {
            r.mem.write_u32(sel + 4 * k, g.next());
        }
        let w = [g.next() % 14, g.next() % 15, g.next() % 12, g.next() % 5];
        r.cpu.set_gpr(4, sel as u64);
        for k in 0..4 {
            r.cpu.set_gpr(5 + k, w[k] as u64);
        }
        r.call(0x168720).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut ours = PreRaceSel::default();
        ours.choose(w[0], w[1], w[2], w[3], [&t0, &t1, &t2], &mut || rand.fx());
        let rd = |m: &mut r5900::Mem, o: u32| m.read_u32(sel + o) as i32;
        for k in 0..5 {
            assert_eq!(rd(&mut r.mem, 4 * k), ours.co[k as usize], "case {case}: co {k} want {w:?}");
        }
        for k in 0..15 {
            assert_eq!(rd(&mut r.mem, 0x14 + 4 * k), ours.ids[k as usize], "case {case}: id {k}");
            assert_eq!(rd(&mut r.mem, 0x50 + 4 * k), ours.firsts[k as usize], "case {case}: first {k}");
            assert_eq!(rd(&mut r.mem, 0x8c + 4 * k), ours.picks[k as usize], "case {case}: pick {k}");
        }
        assert_eq!(rd(&mut r.mem, 0xc8), ours.rider, "case {case}: rider");
        for (k, x) in rand.fx.s.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x345e58 + 4 * k as u32), *x, "case {case}: stream {k}");
        }
        if ours.ids[0] != 0x1ad || ours.ids[1] != 0x1ad || ours.ids[2] != 0x1ad { hits += 1 } else { none += 1 }
    }
    assert!(hits > 2000 && none > 200, "hits {hits} none {none}");
}

#[test]
fn rivals_init_pairs_matches_the_original() {
    use tricky_game::career::{Career, InitialRelations, CAREER_BASE, CAREER_SIZE};
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x167ea0u32, 0x167208, 0x167da0, 0x167de0, 0x167dc0, 0x167e00, 0x166550] {
        r.allow(a);
    }
    let t = r.mem.alloc(12 * 12 * 16);
    r.cpu.set_gpr(4, t as u64);
    r.call(0x167208).expect("RelTable_Init");
    let mut g = Rng(0x1357_9bdf_2468_ace0);
    let mut humans = 0;
    for case in 0..20_000 {
        let mut ours = tricky_game::career::Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        let mut players = Vec::new();
        for k in 0..4usize {
            let o = 0x44 * k;
            let p = if g.next() % 4 == 0 { 0xffu8 } else { k as u8 };
            ours.bytes[0x30fc + o] = p;
            if p != 0xff { players.push(p); }
            ours.w32(0x3114 + o, CAREER_BASE + 0x100 + 0x1c4 * (g.next() % 12));
        }
        let n = (g.next() % 8) as i32 - 1;
        for i in 0..6usize {
            let ri = 0x27e0 + 0x84 * i;
            ours.w32(ri + 0x38, g.next() % 12);
            ours.bytes[ri + 0x65] = if players.is_empty() || g.next() % 2 == 0 { 0xff } else { players[g.next() as usize % players.len()] };
        }
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        r.mem.write_u32(0x3352b4, t);
        r.cpu.set_gpr(4, t as u64);
        r.cpu.set_gpr(5, n.min(6) as i64 as u64);
        r.cpu.set_gpr(6, (CAREER_BASE + 0x27e0) as u64);
        r.cpu.set_gpr(7, (CAREER_BASE + 0x30fc) as u64);
        r.call(0x167ea0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.rivals_init_pairs(n.min(6), &InitialRelations);
        for k in 0..CAREER_SIZE as u32 {
            let got = (r.mem.read_u32((CAREER_BASE + k) & !3) >> (8 * ((CAREER_BASE + k) & 3))) as u8;
            assert_eq!(got, ours.bytes[k as usize], "case {case}: byte {k:#x} (count {n})");
        }
        humans += (0..6).filter(|&i| ours.bytes[0x27e0 + 0x84 * i + 0x65] as i8 >= 0).count();
    }
    let _ = Career::default();
    assert!(humans > 10_000);
}

#[test]
fn finalize_roster_matches_the_original() {
    use tricky_game::career::{Career, CAREER_BASE, CAREER_SIZE};
    use tricky_game::fxrand::SceneTables;
    use tricky_game::rand::Streams;
    use tricky_game::roster::{PreRaceSel, RosterTables};
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x168b88u32, 0x1691f0, 0x1688f0, 0x1687f0, 0x169378, 0x23d4a8, 0x2401e0] {
        r.allow(a);
    }
    let tab = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n * 3).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).collect() };
    let rows: Vec<Vec<u8>> = [(0x37b390u32, 9usize), (0x37b400, 0x85), (0x37ba40, 0x9f), (0x37c1b8, 0x3f), (0x37c4b0, 0x34), (0x37cd60, 1), (0x37c720, 0x85)].iter().map(|&(a, n)| tab(&mut r, a, n)).collect();
    let mut g = Rng(0x2468_ace0_1357_9bdf);
    // table a (0x3cc028) is built at run time: scripted rows for every id the tables hold
    let mut ta = vec![0u8; 0x800 * 10];
    for id in 0..0x800 {
        for k in 0..6 {
            ta[id * 10 + k] = g.next() as u8;
        }
        let base = (g.next() % 1500) as i16;
        let count = 1 + (g.next() % 6) as i16;
        ta[id * 10 + 6..id * 10 + 8].copy_from_slice(&base.to_le_bytes());
        ta[id * 10 + 8..id * 10 + 10].copy_from_slice(&count.to_le_bytes());
    }
    for (i, w) in ta.chunks(4).enumerate() {
        r.mem.write_u32(0x3cc028 + 4 * i as u32, u32::from_le_bytes(w.try_into().unwrap()));
    }
    let tb = tab(&mut r, 0x32f468, 0x2000);
    let tc = tab(&mut r, 0x37cd70, 500 * 8 / 12 + 1);
    let tables = RosterTables { rows: [&rows[0], &rows[1], &rows[2], &rows[3], &rows[4], &rows[5], &rows[6]], scene: SceneTables { a: &ta, b: &tb, c: &tc } };
    let sel = r.mem.alloc(0x100);
    let mut branches = [0u32; 4];
    for case in 0..20_000 {
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        let n = 1 + (g.next() % 6) as usize;
        ours.w32(0x27dc, n as u32);
        for i in 0..6usize {
            let o = 0x27dc + 0x84 * i;
            ours.w32(o + 0x3c, if g.next() % 5 == 0 { g.next() % 12 } else { i as u32 % 12 });
            ours.w32(o + 0x84, if g.next() % 3 == 0 { 2 } else { g.next() % 2 });
            ours.bytes[o + 0x69] = if g.next() % 2 == 0 { 0xff } else { (g.next() % 4) as u8 };
        }
        ours.w32(0x2af8, g.next() % 12);
        ours.w32(0x2afc, g.next() % 11);
        ours.w32(0x2b0c, g.next() % 6);
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        // the selection: co-riders from the roster or 12, ids and firsts and picks as they were
        let mut s = PreRaceSel::default();
        let roster_rider = |c: &Career, i: usize| c.r32(0x27dc + 0x84 * i + 0x3c) as i32;
        for k in 0..5 {
            s.co[k] = if g.next() % 3 == 0 { 0xc } else if g.next() % 4 == 0 { (g.next() % 12) as i32 } else { roster_rider(&ours, g.next() as usize % n) };
        }
        for k in 0..15 {
            s.ids[k] = g.next() as i32 % 0x400 & 0x3ff;
            s.firsts[k] = g.next() as i32 & 0x7f;
            s.picks[k] = g.next() as i32 & 7;
        }
        s.rider = if g.next() % 4 == 0 { 0xc } else if g.next() % 3 == 0 { (g.next() % 12) as i32 } else { roster_rider(&ours, g.next() as usize % n) };
        for k in 0..5 {
            r.mem.write_u32(sel + 4 * k as u32, s.co[k] as u32);
        }
        for k in 0..15 {
            r.mem.write_u32(sel + 0x14 + 4 * k as u32, s.ids[k] as u32);
            r.mem.write_u32(sel + 0x50 + 4 * k as u32, s.firsts[k] as u32);
            r.mem.write_u32(sel + 0x8c + 4 * k as u32, s.picks[k] as u32);
        }
        r.mem.write_u32(sel + 0xc8, s.rider as u32);
        let mut rand = Streams::default();
        rand.seed_fx(g.next());
        for (k, w) in rand.fx.s.iter().enumerate() {
            r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
        }
        r.cpu.set_gpr(4, sel as u64);
        r.cpu.set_gpr(5, (CAREER_BASE + 0x27dc) as u64);
        r.call(0x168b88).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.finalize_roster(&mut s, &tables, &mut || rand.fx());
        for k in 0..CAREER_SIZE as u32 {
            let got = (r.mem.read_u32((CAREER_BASE + k) & !3) >> (8 * ((CAREER_BASE + k) & 3))) as u8;
            assert_eq!(got, ours.bytes[k as usize], "case {case}: career byte {k:#x} (riders {n})");
        }
        let rd = |m: &mut r5900::Mem, o: u32| m.read_u32(sel + o) as i32;
        for k in 0..5 {
            assert_eq!(rd(&mut r.mem, 4 * k), s.co[k as usize], "case {case}: co {k}");
        }
        for k in 0..15 {
            assert_eq!(rd(&mut r.mem, 0x14 + 4 * k), s.ids[k as usize], "case {case}: id {k}");
            assert_eq!(rd(&mut r.mem, 0x50 + 4 * k), s.firsts[k as usize], "case {case}: first {k}");
            assert_eq!(rd(&mut r.mem, 0x8c + 4 * k), s.picks[k as usize], "case {case}: pick {k}");
        }
        for (k, x) in rand.fx.s.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x345e58 + 4 * k as u32), *x, "case {case}: stream {k}");
        }
        branches[(ours.r32(0x27dc + 0x918) as i32 >= 0) as usize] += 1;
    }
    assert!(branches.iter().take(2).all(|&c| c > 500), "{branches:?}");
}

#[test]
fn setup_ai_rider_matches_the_original() {
    use tricky_game::career::{Career, StaticTables, CAREER_BASE, CAREER_SIZE};
    use tricky_game::rand::Streams;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x16ae68u32, 0x16b1f8, 0x16b220, 0x16b248, 0x16b270, 0x16b1d0, 0x16b298, 0x23d4a8, 0x2401e0] {
        r.allow(a);
    }
    // strcpy and EAMem_Copy (MMI loops): the C library's answer
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
    r.stub(0x2ce114, |c, m| {
        let (d, s, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        for k in 0..n {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            m.write_u8(d + k, b);
        }
    });
    let rd = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n.div_ceil(4)).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).take(n).collect() };
    let entries = rd(&mut r, 0x331ea8, 12 * 0x38);
    let bpoints = rd(&mut r, 0x332150, 12 * 0xf0 + 0x20);
    let mut g = Rng(0x600d_f00d_0bad_cafe);
    let mut tables = StaticTables::default();
    for c in 0..12 {
        for o in 0..7 {
            for w in 0..4 {
                tables.outfits[c][o][w] = g.next();
                r.mem.write_u32(0x3cd0e0 + 0x70 * c as u32 + 0x10 * o as u32 + 4 * w as u32, tables.outfits[c][o][w]);
            }
        }
        for b in 0..12 {
            tables.boards[c][b] = g.next();
            r.mem.write_u32(0x3cd620 + 0x30 * c as u32 + 4 * b as u32, tables.boards[c][b]);
        }
    }
    let rival = r.mem.alloc(8);
    let mut maxa = [0u32; 2];
    for case in 0..20_000 {
        let rider = (g.next() % 12) as usize;
        let row = (g.next() % 6) as usize;
        let entry = entries[rider * 0x38..rider * 0x38 + 0x38].to_vec();
        let name_ptr = u32::from_le_bytes(entry[4..8].try_into().unwrap());
        let mut name = Vec::new();
        for i in 0.. {
            let b = rd(&mut r, name_ptr + i, 1)[0];
            name.push(b);
            if b == 0 { break; }
        }
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        let max_attrib = g.next() % 3 == 0;
        r.mem.write_u32(0x3352ac, max_attrib as u32);
        let level = if g.next() % 4 == 0 { None } else { Some((g.next() % 140) as i32 - 20) };
        if let Some(v) = level {
            r.mem.write_u32(rival, v as u32);
        }
        let mut rand = Streams::default();
        rand.seed_fx(g.next());
        for (k, w) in rand.fx.s.iter().enumerate() {
            r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
        }
        r.cpu.set_gpr(4, CAREER_BASE as u64);
        r.cpu.set_gpr(5, (0x331ea8 + 0x38 * rider) as u64);
        r.cpu.set_gpr(6, if level.is_some() { rival as u64 } else { 0 });
        r.cpu.set_gpr(7, (CAREER_BASE + 0x27e0 + 0x84 * row as u32) as u64);
        r.call(0x16ae68).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.setup_ai_rider(row, &entry, &name, &bpoints, level, &tables, max_attrib, &mut || rand.fx());
        for k in 0..CAREER_SIZE as u32 {
            let got = (r.mem.read_u32((CAREER_BASE + k) & !3) >> (8 * ((CAREER_BASE + k) & 3))) as u8;
            assert_eq!(got, ours.bytes[k as usize], "case {case}: byte {k:#x} (rider {rider} row {row} level {level:?} max {max_attrib})");
        }
        for (k, x) in rand.fx.s.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x345e58 + 4 * k as u32), *x, "case {case}: stream {k}");
        }
        maxa[max_attrib as usize] += 1;
    }
    assert!(maxa.iter().all(|&c| c > 3000));
}

#[test]
fn setup_player_rider_matches_the_original() {
    use tricky_game::career::{Career, StaticTables, CAREER_BASE, CAREER_SIZE};
    use tricky_game::roster::PlayerRiderEnv;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x16a9d8u32, 0x16b1f8, 0x16b220, 0x16b248, 0x16b270, 0x16b1d0, 0x16b298, 0x166550, 0x164968, 0x164840, 0x2cd9d8] {
        r.allow(a);
    }
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
    r.stub(0x2ce114, |c, m| {
        let (d, s, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        for k in 0..n {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            m.write_u8(d + k, b);
        }
    });
    let rd = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n.div_ceil(4)).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).take(n).collect() };
    let entries = rd(&mut r, 0x331ea8, 12 * 0x38);
    let bpoints = rd(&mut r, 0x332150, 12 * 0xf0 + 0x20);
    let mut g = Rng(0x0ddb_a11c_afe0_0001);
    let mut tables = StaticTables::default();
    for c in 0..12 {
        for o in 0..7 {
            for w in 0..4 {
                tables.outfits[c][o][w] = g.next();
                r.mem.write_u32(0x3cd0e0 + 0x70 * c as u32 + 0x10 * o as u32 + 4 * w as u32, tables.outfits[c][o][w]);
            }
        }
        for b in 0..12 {
            tables.boards[c][b] = g.next();
            r.mem.write_u32(0x3cd620 + 0x30 * c as u32 + 4 * b as u32, tables.boards[c][b]);
        }
    }
    let mut seen = [0u32; 4];
    for case in 0..20_000 {
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        for k in 0..4usize {
            let sb = 0x30fc + 0x44 * k;
            ours.bytes[sb] = k as u8;
            ours.w32(sb + 0x14, CAREER_BASE + 0x3400 + 0x20 * k as u32);
            ours.w32(sb + 0x18, CAREER_BASE + 0x100 + 0x1c4 * (g.next() % 12));
            ours.w32(sb + 0xc, CAREER_BASE + 0x3500 + 0x20 * k as u32);
            ours.bytes[0x3500 + 0x20 * k + 15] = 0;
            let p = 0x3400 + 0x20 * k;
            ours.w32(p, g.next() % 12);
            ours.w32(p + 0xc, g.next() % 7);
            ours.w32(p + 0x10, g.next() % 12);
        }
        let k = (g.next() % 4) as usize;
        let row = (g.next() % 6) as usize;
        let mode = if g.next() % 3 == 0 { 6 } else { g.next() % 6 };
        ours.w32(0x2afc, mode);
        let env = PlayerRiderEnv {
            words: [g.next(), g.next()],
            character_cheat: if g.next() % 4 == 0 { 1 + g.next() % 3 } else { 0 },
            mallora: g.next() % 3 == 0,
            max_attrib: g.next() % 3 == 0,
        };
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        r.mem.write_u32(0x3352a4, env.words[0]);
        r.mem.write_u32(0x3352a8, env.words[1]);
        r.mem.write_u32(0x3352b8, env.character_cheat);
        r.mem.write_u32(0x3352b0, env.mallora as u32);
        r.mem.write_u32(0x3352ac, env.max_attrib as u32);
        let rider = ours.r32(0x3400 + 0x20 * k) as usize;
        let entry = entries[rider * 0x38..rider * 0x38 + 0x38].to_vec();
        let name: Vec<u8> = ours.bytes[0x3500 + 0x20 * k..].iter().copied().take_while(|&b| b != 0).chain(std::iter::once(0)).collect();
        r.cpu.set_gpr(4, CAREER_BASE as u64);
        r.cpu.set_gpr(5, (CAREER_BASE + 0x30fc + 0x44 * k as u32) as u64);
        r.cpu.set_gpr(6, (CAREER_BASE + 0x27e0 + 0x84 * row as u32) as u64);
        r.call(0x16a9d8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.setup_player_rider(k, row, &entry, &name, &bpoints, &tables, &env);
        for b in 0..CAREER_SIZE as u32 {
            let got = (r.mem.read_u32((CAREER_BASE + b) & !3) >> (8 * ((CAREER_BASE + b) & 3))) as u8;
            assert_eq!(got, ours.bytes[b as usize], "case {case}: byte {b:#x} (rider {rider} slot {k} row {row} {env:?} mode {mode})");
        }
        seen[(mode == 6) as usize + 2 * (env.character_cheat != 0) as usize] += 1;
    }
    assert!(seen.iter().all(|&c| c > 1000), "{seen:?}");
}

#[test]
fn build_race_riders_matches_the_original() {
    run_builder(0x165cf0, 0, 12000, &|_, _| {}, &|c, x, f| c.build_race_riders(x, f));
}

#[test]
fn init_rider_list_matches_the_original() {
    run_builder(0x1664b0, 0, 6000, &human, &|c, x, f| c.init_rider_list(x, f));
}

#[test]
fn reset_riders_matches_the_original() {
    run_builder(0x166450, 0, 6000, &human, &|c, x, f| c.reset_riders(x, f));
}

#[test]
fn pick_six_random_riders_matches_the_original() {
    run_builder(0x165a40, 0, 6000, &|_, _| {}, &|c, x, f| c.pick_six_random_riders(x, f));
}

#[test]
fn pick_random_riders_matches_the_original() {
    run_builder(0x165b98, 0, 6000, &|c, g| c.w32(0x16b8, 1 + g.next() as u32 % 6), &|c, x, f| c.pick_random_riders(x, f));
}

#[test]
fn setup_tutorial_riders_matches_the_original() {
    run_builder(0x165778, 0, 6000, &human, &|c, x, f| c.setup_tutorial_riders(x, f));
}

/// these three need a human in slot 0, as the game always has (no human: a row never set up is read)
fn human(c: &mut tricky_game::career::Career, _: &mut Rng) {
    c.bytes[0x30fc] = 0;
}

#[test]
fn circuit_setup_riders_matches_the_original() {
    let set = |c: &mut tricky_game::career::Career, g: &mut Rng| {
        c.bytes[0x30fc] = 0;
        c.w32(0x2afc, 5);
        c.w32(0x320c + 4, tricky_game::career::CAREER_BASE);
        c.w32(0x320c + 8, tricky_game::career::CAREER_BASE + 0x27dc);
        c.w32(0x320c + 0x10, tricky_game::career::CAREER_BASE + 0x30fc);
        if g.next() % 2 == 0 {
            let l = [0, 1, 2, 3, 4, 8, 5, 11, 7, 9];
            c.w32(0x2af8, l[g.next() as usize % l.len()]);
        }
    };
    run_builder(0x16bc50, 0x320c, 3000, &set, &|c, x, f| c.circuit_setup_riders(0, x, &mut |_| unreachable!(), f));
}

fn run_builder(
    addr: u32,
    arg: u32,
    cases: usize,
    set_mode: &dyn Fn(&mut tricky_game::career::Career, &mut Rng),
    call: &dyn Fn(&mut tricky_game::career::Career, &mut tricky_game::roster::RosterCtx, &mut dyn FnMut() -> u32),
) {
    use tricky_game::career::{Career, InitialRelations, StaticTables, CAREER_BASE, CAREER_SIZE};
    use tricky_game::fxrand::SceneTables;
    use tricky_game::rand::Streams;
    use tricky_game::roster::{PlayerRiderEnv, PreRaceSel, RosterCtx, RosterTables};
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
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
    r.stub(0x2ce114, |c, m| {
        let (d, s, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        for k in 0..n {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            m.write_u8(d + k, b);
        }
    });
    let rd = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n.div_ceil(4)).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).take(n).collect() };
    let entries = rd(&mut r, 0x331ea8, 12 * 0x38);
    let bpoints = rd(&mut r, 0x332150, 12 * 0xf0 + 0x20);
    let mut names: Vec<Vec<u8>> = Vec::new();
    for i in 0..12 {
        let p = u32::from_le_bytes(entries[i * 0x38 + 4..i * 0x38 + 8].try_into().unwrap());
        let mut v = Vec::new();
        for k in 0.. {
            let b = rd(&mut r, p + k, 1)[0];
            v.push(b);
            if b == 0 { break; }
        }
        names.push(v);
    }
    let rt = r.mem.alloc(12 * 12 * 16);
    r.cpu.set_gpr(4, rt as u64);
    r.allow(0x167208);
    r.call(0x167208).expect("RelTable_Init");
    r.mem.write_u32(0x3352b4, rt);
    let tab = |r: &mut Runner, a: u32, n: usize| -> Vec<u8> { (0..n * 3).flat_map(|i| r.mem.read_u32(a + 4 * i as u32).to_le_bytes()).collect() };
    let rows: Vec<Vec<u8>> = [(0x37b390u32, 9usize), (0x37b400, 0x85), (0x37ba40, 0x9f), (0x37c1b8, 0x3f), (0x37c4b0, 0x34), (0x37cd60, 1), (0x37c720, 0x85)].iter().map(|&(a, n)| tab(&mut r, a, n)).collect();
    let mut g = Rng(0x5eed_0000_b01d_face);
    let mut ta = vec![0u8; 0x800 * 10];
    for id in 0..0x800 {
        for k in 0..6 {
            ta[id * 10 + k] = g.next() as u8;
        }
        let base = (g.next() % 1500) as i16;
        let count = 1 + (g.next() % 6) as i16;
        ta[id * 10 + 6..id * 10 + 8].copy_from_slice(&base.to_le_bytes());
        ta[id * 10 + 8..id * 10 + 10].copy_from_slice(&count.to_le_bytes());
    }
    for (i, w) in ta.chunks(4).enumerate() {
        r.mem.write_u32(0x3cc028 + 4 * i as u32, u32::from_le_bytes(w.try_into().unwrap()));
    }
    let tb = tab(&mut r, 0x32f468, 0x2000);
    let tc = tab(&mut r, 0x37cd70, 500 * 8 / 12 + 1);
    let sel_tables = RosterTables { rows: [&rows[0], &rows[1], &rows[2], &rows[3], &rows[4], &rows[5], &rows[6]], scene: SceneTables { a: &ta, b: &tb, c: &tc } };
    let mut tables = StaticTables::default();
    for c in 0..12 {
        for o in 0..7 {
            for w in 0..4 {
                tables.outfits[c][o][w] = g.next();
                r.mem.write_u32(0x3cd0e0 + 0x70 * c as u32 + 0x10 * o as u32 + 4 * w as u32, tables.outfits[c][o][w]);
            }
        }
        for b in 0..12 {
            tables.boards[c][b] = g.next();
            r.mem.write_u32(0x3cd620 + 0x30 * c as u32 + 4 * b as u32, tables.boards[c][b]);
        }
    }
    let mut cover = [0u32; 4];
    for case in 0..cases {
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        let mut humans = 0;
        for k in 0..4usize {
            let sb = 0x30fc + 0x44 * k;
            let p = 0x3400 + 0x20 * k;
            let present = g.next() % 2 == 0;
            ours.bytes[sb] = if present { humans += 1; k as u8 } else { 0xff };
            ours.w32(sb + 0x14, CAREER_BASE + p as u32);
            ours.w32(sb + 0x18, CAREER_BASE + 0x100 + 0x1c4 * (g.next() % 12));
            ours.w32(sb + 0xc, CAREER_BASE + 0x3500 + 0x20 * k as u32);
            ours.bytes[0x3500 + 0x20 * k + 15] = 0;
            ours.w32(p, g.next() % 12);
            ours.w32(p + 0xc, g.next() % 7);
            ours.w32(p + 0x10, g.next() % 12);
        }
        let n = 1 + g.next() % 6;
        ours.w32(0x27dc, n);
        ours.w32(0x2af8, if g.next() % 3 == 0 { 0xb } else { g.next() % 12 });
        ours.w32(0x2afc, if g.next() % 4 == 0 { 6 } else { g.next() % 11 });
        set_mode(&mut ours, &mut g);
        let env = PlayerRiderEnv { words: [g.next(), g.next()], character_cheat: if g.next() % 4 == 0 { 1 } else { 0 }, mallora: g.next() % 3 == 0, max_attrib: g.next() % 3 == 0 };
        let mut s = PreRaceSel::default();
        for k in 0..5 {
            s.co[k] = g.next() as i32 % 14;
        }
        for k in 0..15 {
            s.ids[k] = g.next() as i32 % 0x400 & 0x3ff;
            s.firsts[k] = g.next() as i32 & 0x7f;
            s.picks[k] = g.next() as i32 & 7;
        }
        s.rider = (g.next() % 13) as i32;
        let seed = g.next();
        let mut attempts = 0;
        loop {
            for k in 0..CAREER_SIZE as u32 / 4 {
                let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
                r.mem.write_u32(CAREER_BASE + 4 * k, v);
            }
            for (a, v) in [(0x3352a4u32, env.words[0]), (0x3352a8, env.words[1]), (0x3352b8, env.character_cheat), (0x3352b0, env.mallora as u32), (0x3352ac, env.max_attrib as u32)] {
                r.mem.write_u32(a, v);
            }
            for k in 0..5 {
                r.mem.write_u32(0x32ecf0 + 4 * k as u32, s.co[k] as u32);
            }
            for k in 0..15 {
                r.mem.write_u32(0x32ecf0 + 0x14 + 4 * k as u32, s.ids[k] as u32);
                r.mem.write_u32(0x32ecf0 + 0x50 + 4 * k as u32, s.firsts[k] as u32);
                r.mem.write_u32(0x32ecf0 + 0x8c + 4 * k as u32, s.picks[k] as u32);
            }
            r.mem.write_u32(0x32ecf0 + 0xc8, s.rider as u32);
            let mut rand = Streams::default();
            rand.seed_fx(seed);
            for (k, w) in rand.fx.s.iter().enumerate() {
                r.mem.write_u32(0x345e58 + 4 * k as u32, *w);
            }
            r.cpu.set_gpr(4, (CAREER_BASE + arg) as u64);
            match r.call(addr) {
                Ok(()) => break,
                Err(r5900::Error::UnstubbedCall { target, .. }) if target >= 0x100000 && attempts < 40 => {
                    attempts += 1;
                    r.allow(target);
                }
                Err(e) => panic!("case {case}: {e}"),
            }
        }
        let mut rand = Streams::default();
        rand.seed_fx(seed);
        let mut sel = s;
        let mut ctx = RosterCtx { sel: &mut sel, entries: &entries, names: &names, board_points: &bpoints, tables: &tables, sel_tables: &sel_tables, env, rel: &InitialRelations };
        call(&mut ours, &mut ctx, &mut || rand.fx());
        for k in 0..CAREER_SIZE as u32 {
            let got = (r.mem.read_u32((CAREER_BASE + k) & !3) >> (8 * ((CAREER_BASE + k) & 3))) as u8;
            assert_eq!(got, ours.bytes[k as usize], "case {case}: career byte {k:#x} (riders {n}, humans {humans}, race {:#x})", ours.r32(0x2af8));
        }
        let rdw = |m: &mut r5900::Mem, o: u32| m.read_u32(0x32ecf0 + o) as i32;
        for k in 0..5 {
            assert_eq!(rdw(&mut r.mem, 4 * k), sel.co[k as usize], "case {case}: co {k}");
        }
        for k in 0..15 {
            assert_eq!(rdw(&mut r.mem, 0x14 + 4 * k), sel.ids[k as usize], "case {case}: id {k}");
            assert_eq!(rdw(&mut r.mem, 0x50 + 4 * k), sel.firsts[k as usize], "case {case}: first {k}");
            assert_eq!(rdw(&mut r.mem, 0x8c + 4 * k), sel.picks[k as usize], "case {case}: pick {k}");
        }
        assert_eq!(rdw(&mut r.mem, 0xc8), sel.rider, "case {case}: rider");
        for (k, x) in rand.fx.s.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x345e58 + 4 * k as u32), *x, "case {case}: stream {k}");
        }
        cover[(humans > 0) as usize + 2 * (n >= 2) as usize] += 1;
    }
    assert!(cover[0] + cover[1] > 100, "{cover:?}");
}

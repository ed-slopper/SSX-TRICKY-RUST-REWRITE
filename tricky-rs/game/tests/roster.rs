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

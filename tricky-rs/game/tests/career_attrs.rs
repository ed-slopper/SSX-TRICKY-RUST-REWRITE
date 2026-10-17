//! The player's attribute getters (board row G3d) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random career blocks at their own address (0x32b390: the slots' players, profile and record pointers
//! into the block, the profiles' riders and boards in the board table), `Career_GetPlayerAttributes` 0x164638,
//! `_For` 0x1646b0, `Career_GetPlayerStats` 0x163fe8 and `Career_GetPlayerLevelName` 0x164808 called for random
//! players; the career's getters and `Career_GetAttributes` run as the original, the localiser stubbed.

use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE, CAREER_SIZE};
use tricky_game::cheats::Cheats;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

#[test]
fn attribute_getters_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x166550u32, 0x165468, 0x164730, 0x164840] {
        r.allow(f);
    }
    let app = r.mem.alloc(0x100);
    let loc = 0x0123_4560u32;
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x50, loc);
    r.stub(0x2c5160, |c, _| {
        let v = (c.gpr(4) as u32).wrapping_add((c.gpr(5) as u32).wrapping_mul(0x9e37));
        c.set_gpr(2, v as u64)
    });
    let out = r.mem.alloc(0x40);
    let mut g = Rng(0x1646_0000_0000_0001);
    let mut counts = [0usize; 4];
    for case in 0..20_000u32 {
        let mut c = Career::default();
        for k in 0..CAREER_SIZE / 4 {
            c.w32(4 * k, g.next());
        }
        for k in -1..=3isize {
            c.bytes[(0x30fc + 0x44 * k) as usize] = (g.below(8) as i32 - 1) as i8 as u8;
        }
        for k in -1..=3isize {
            let prof = 4 * g.below(0x2f00 / 4);
            let rec = 4 * g.below(0x2f00 / 4);
            c.w32((0x3110 + 0x44 * k) as usize, CAREER_BASE + prof);
            c.w32((0x3114 + 0x44 * k) as usize, CAREER_BASE + rec);
        }
        for k in -1..=3isize {
            // the record's points: small sums, so that every level comes up
            let rec = c.ptr((0x3114 + 0x44 * k) as usize);
            if g.below(2) == 0 {
                c.w32(rec + 4, g.below(3000) as i32 as u32);
                c.w32(rec + 8, (g.below(3000) as i32 - 500) as u32);
            }
        }
        // (the profiles last, so that their riders and boards stay in the table)
        for k in -1..=3isize {
            let prof = c.ptr((0x3110 + 0x44 * k) as usize);
            c.w32(prof, g.below(12));
            c.w32(prof + 0x10, g.below(12));
        }
        r.mem.write(CAREER_BASE, &c.bytes).unwrap();
        let cheats = Cheats { max_attrib: if g.below(5) == 0 { 1 + g.below(3) } else { 0 }, ..Cheats::default() };
        r.mem.write_u32(0x3352ac, cheats.max_attrib);
        let p = (g.below(9) as i32 - 2) as i8;
        let kind = g.below(4) as usize;
        counts[kind] += 1;
        r.cpu.set_gpr(4, CAREER_BASE as u64);
        r.cpu.set_gpr(5, p as i64 as u64);
        let read_out = |r: &mut Runner| {
            let mut a = [[0u32; 4]; 4];
            for (i, row) in a.iter_mut().enumerate() {
                for (j, w) in row.iter_mut().enumerate() {
                    *w = r.mem.read_u32(out + 16 * i as u32 + 4 * j as u32);
                }
            }
            a
        };
        match kind {
            0 => {
                r.cpu.set_gpr(6, out as u64);
                r.call(0x164638).unwrap_or_else(|e| panic!("case {case}: {e}"));
                assert_eq!(read_out(&mut r), c.player_attributes(p, &cheats), "case {case}: attributes of {p}");
            }
            1 => {
                let board = g.below(12);
                r.cpu.set_gpr(6, board as u64);
                r.cpu.set_gpr(7, out as u64);
                r.call(0x1646b0).unwrap_or_else(|e| panic!("case {case}: {e}"));
                assert_eq!(read_out(&mut r), c.player_attributes_for(p, board, &cheats), "case {case}: attributes of {p} on {board}");
            }
            2 => {
                r.call(0x163fe8).unwrap_or_else(|e| panic!("case {case}: {e}"));
                assert_eq!(r.cpu.gpr(2) as u32, c.player_record(p), "case {case}: record of {p}");
            }
            _ => {
                r.call(0x164808).unwrap_or_else(|e| panic!("case {case}: {e}"));
                let mine = c.player_level_name(p, &mut |id| loc.wrapping_add(id.wrapping_mul(0x9e37)));
                assert_eq!(r.cpu.gpr(2) as u32, mine, "case {case}: level name of {p}");
            }
        }
    }
    println!("{counts:?}");
}

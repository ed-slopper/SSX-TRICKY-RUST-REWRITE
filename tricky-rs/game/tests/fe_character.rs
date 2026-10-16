//! `tricky_game::fe_character` (board row G3c) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random career blocks at their own address (0x32b390: the player slots' bytes, the profile and record
//! pointers into the block, small profile words so that riders and outfits match often) and random characters, each
//! of the eight functions called with random arguments; the career's getters run as the original. The answer and the
//! character's choice bytes compared.

use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE, CAREER_SIZE};
use tricky_game::cheats::Cheats;
use tricky_game::fe_character::*;

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
    fn small(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u32) as i32
    }
}

const CHAR: u32 = 0x0100_0000;

fn byte(r: &mut Runner, a: u32) -> u8 {
    (r.mem.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

#[test]
fn character_choices_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x164318u32, 0x164330, 0x164dc0, 0x164e40, 0x163d30, 0x163d98, 0x166550, 0x267858, 0x267910, 0x2679a0, 0x267c20, 0x267c48] {
        r.allow(f);
    }
    let mut g = Rng(0x267c_0000_0000_0001);
    let mut counts = [0usize; 8];
    for case in 0..30_000u32 {
        // the career: random bytes, then the slots
        let mut c = Career::default();
        for k in 0..CAREER_SIZE / 4 {
            c.w32(4 * k, g.next());
        }
        for k in -2..=7isize {
            let at = (0x30fc + 0x44 * k) as usize;
            c.bytes[at] = g.small(-1, 6) as i8 as u8;
        }
        for k in -1..=3isize {
            let prof = 4 * g.below(0x2f00 / 4) as usize;
            let rec = 4 * g.below(0x2f00 / 4) as usize;
            c.w32((0x3110 + 0x44 * k) as usize, CAREER_BASE + prof as u32);
            c.w32((0x3114 + 0x44 * k) as usize, CAREER_BASE + rec as u32);
        }
        for k in -1..=3isize {
            let prof = c.r32((0x3110 + 0x44 * k) as usize).wrapping_sub(CAREER_BASE) as usize;
            c.w32(prof, g.below(4));
            c.w32(prof + 0xc, if g.below(8) == 0 { g.next() } else { g.small(-1, 3) as u32 });
            c.w32(prof + 0x10, if g.below(8) == 0 { g.next() } else { g.below(12) });
            if g.below(3) == 0 {
                // records with most bits set
                let rec = c.r32((0x3114 + 0x44 * k) as usize).wrapping_sub(CAREER_BASE) as usize;
                c.w32(rec + 0x58, g.next() | g.next());
                c.w32(rec + 0x5c, g.next() | g.next());
            }
        }
        r.mem.write(CAREER_BASE, &c.bytes).unwrap();
        let cheats = Cheats { unlock_everything: if g.below(6) == 0 { 1 + g.below(3) } else { 0 }, ..Cheats::default() };
        r.mem.write_u32(0x3352a0, cheats.unlock_everything);
        // the character
        let mut look = Look {
            rider: g.below(4),
            board: g.small(-1, 12) as i8,
            outfit: g.small(-1, 4) as i8,
            board_changed: g.below(2) * g.next(),
            outfit_changed: g.below(2) * g.next(),
        };
        r.mem.write_u32(CHAR + 0xb0, look.rider);
        r.mem.write_u32(CHAR + 0x158, look.board as u8 as u32 | (look.outfit as u8 as u32) << 8 | g.next() << 16);
        r.mem.write_u32(CHAR + 0x15c, look.board_changed);
        r.mem.write_u32(CHAR + 0x160, look.outfit_changed);
        // a byte argument: small or any, with junk above it
        let arg = |g: &mut Rng, lo: i32, hi: i32| -> (i8, u64) {
            let v = if g.below(6) == 0 { g.next() as i8 } else { g.small(lo, hi) as i8 };
            (v, (g.next() as u64) << 32 | (g.next() & 0xffff_ff00) as u64 | v as u8 as u64)
        };
        let kind = g.below(8) as usize;
        counts[kind] += 1;
        let (theirs, mine): (i64, i64);
        r.cpu.set_gpr(4, CHAR as u64);
        match kind {
            0 => {
                let (p, pr) = arg(&mut g, -1, 6);
                let (q, qr) = arg(&mut g, -1, 6);
                let (o, or) = arg(&mut g, -1, 4);
                r.cpu.set_gpr(5, pr);
                r.cpu.set_gpr(6, qr);
                r.cpu.set_gpr(7, or);
                r.call(0x267858).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = r.cpu.gpr(2) as i64;
                mine = outfit_blocked(&c, look.rider, p, q, o, &cheats) as i64;
            }
            1 => {
                let (_, pr) = arg(&mut g, -1, 6);
                let (q, qr) = arg(&mut g, -1, 6);
                let (o, or) = arg(&mut g, -1, 4);
                r.cpu.set_gpr(5, pr);
                r.cpu.set_gpr(6, qr);
                r.cpu.set_gpr(7, or);
                r.call(0x267910).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = r.cpu.gpr(2) as i64;
                mine = outfit_taken_by(&c, look.rider, q, o) as i64;
            }
            2 => {
                let n = g.small(-2, 6) as i8;
                let (o, or) = arg(&mut g, -1, 4);
                let (s, sr) = arg(&mut g, -9, 9);
                r.cpu.set_gpr(5, (g.next() as u64) << 32 | n as u8 as u64);
                r.cpu.set_gpr(6, or);
                r.cpu.set_gpr(7, sr);
                r.call(0x2679a0).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = r.cpu.gpr(2) as i64;
                mine = pick_free_outfit(&c, look.rider, n, o, s, &cheats) as i64;
            }
            3 => {
                let n = g.small(-2, 6) as i8;
                r.cpu.set_gpr(5, (g.next() as u64) << 32 | (g.next() & 0xffff_ff00) as u64 | n as u8 as u64);
                r.call(0x267b18).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = 0;
                look.restore_choice(&c, n, &cheats);
                mine = 0;
            }
            4 => {
                let (b, br) = arg(&mut g, -1, 12);
                r.cpu.set_gpr(5, br);
                r.call(0x267c20).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = 0;
                look.set_board(b);
                mine = 0;
            }
            5 => {
                let (o, or) = arg(&mut g, -1, 4);
                r.cpu.set_gpr(5, or);
                r.call(0x267c48).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = 0;
                look.set_outfit(o);
                mine = 0;
            }
            6 => {
                let n = g.small(-2, 6);
                let (b, br) = arg(&mut g, -1, 12);
                r.cpu.set_gpr(5, n as i64 as u64);
                r.cpu.set_gpr(6, br);
                r.call(0x267c70).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = r.cpu.gpr(2) as i64;
                mine = board_unlocked(&c, n, b, &cheats) as i64;
            }
            _ => {
                let n = g.small(-2, 6);
                let (o, or) = arg(&mut g, -1, 4);
                r.cpu.set_gpr(5, n as i64 as u64);
                r.cpu.set_gpr(6, or);
                r.call(0x267cc0).unwrap_or_else(|e| panic!("case {case}: {e}"));
                theirs = r.cpu.gpr(2) as i64;
                mine = outfit_available(&c, look.rider, n, o, &cheats) as i64;
            }
        }
        assert_eq!(theirs, mine, "case {case}: kind {kind}");
        let got = Look {
            rider: r.mem.read_u32(CHAR + 0xb0),
            board: byte(&mut r, CHAR + 0x158) as i8,
            outfit: byte(&mut r, CHAR + 0x159) as i8,
            board_changed: r.mem.read_u32(CHAR + 0x15c),
            outfit_changed: r.mem.read_u32(CHAR + 0x160),
        };
        assert_eq!(got, look, "case {case}: kind {kind}");
    }
    println!("{counts:?}");
}

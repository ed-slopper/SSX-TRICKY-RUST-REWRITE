//! `AnimIds_Init` 0x15aec0 (board row G5c4) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the clip table's lookup words and the index tables' whole ranges (the words between the runs too)
//! filled at random, the original and `animids::init` run over the same memory, every word compared.

use r5900::Runner;
use tricky_game::animids;
use tricky_game::pf::Mem;

const RANGES: [(u32, u32); 9] = [
    (0x3bf994, 0x3bf9d8),
    (0x3bfa78, 0x3bfa7c),
    (0x3bfd80, 0x3c07c0),
    (0x3c0a80, 0x3c0b90),
    (0x3c23a0, 0x3c240c),
    (0x3c2454, 0x3c2be0),
    (0x3c2ea4, 0x3c2eb0),
    (0x3c2ef8, 0x3c2f04),
    (0x3cbcc8, 0x3cbf98),
];
const TABLE: u32 = 0x325fa8;
const TABLE_LEN: u32 = 0x1c * 0x2ff;

#[test]
fn the_clip_ids_init_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x15aec0);
    let mut s = 0x15ae_c001u32;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for case in 0..20 {
        let mut m = Mem { regions: RANGES.iter().map(|&(a, b)| (a, vec![0u8; (b - a) as usize])).collect() };
        let mut table = vec![0u8; TABLE_LEN as usize];
        r.mem.read(TABLE, &mut table).unwrap();
        m.regions.push((TABLE, table));
        for (a, b) in RANGES {
            for x in (a..b).step_by(4) {
                m.w32(x, next());
            }
        }
        for c in 0..0x2ff {
            m.w32(TABLE + 0x1c * c + 4, next());
        }
        for (a, b) in &m.regions {
            r.mem.write(*a, b).unwrap();
        }
        r.call(0x15aec0).unwrap();
        animids::init(&mut m);
        for (a, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*a, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: memory at {:#x}: theirs {:#x} ours {:#x}", a + i as u32, theirs[i], b[i]);
            }
        }
    }
}

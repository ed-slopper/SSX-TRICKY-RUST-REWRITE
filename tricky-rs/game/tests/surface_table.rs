//! The surface table against the original's, bit for bit (board row E8f): `SurfaceTable_Init` 0x256188 run in
//! the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) fills a 20 × 25 word table,
//! which must equal `tricky_game::surface::SURFACE_TABLE` word for word. The second test checks that the
//! constants `Rider::step` reads (SURFACES, SURF_EXTRA, SURF_BAND) are taken from that table.

use r5900::Runner;
use tricky_game::rider::{SURFACES, SURF_BAND, SURF_EXTRA};
use tricky_game::surface::SURFACE_TABLE;

#[test]
fn surface_table_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    let table = r.mem.alloc(20 * 25 * 4);
    // poison it, so a word the original does not write cannot pass by being zero on both sides
    for i in 0..20 * 25 {
        r.mem.write_u32(table + i * 4, 0xdead_beef);
    }
    r.cpu.set_gpr(4, table as u64);
    r.call(0x256188).expect("SurfaceTable_Init runs to its return");
    let mut wrong = Vec::new();
    for (row, words) in SURFACE_TABLE.iter().enumerate() {
        for (col, ours) in words.iter().enumerate() {
            let theirs = r.mem.read_u32(table + ((row * 25 + col) * 4) as u32);
            if theirs != ours.to_bits() {
                wrong.push(format!("row {row} [{col}]: game {theirs:#010x} ({}), ours {:#010x} ({ours})",
                                   f32::from_bits(theirs), ours.to_bits()));
            }
        }
    }
    assert!(wrong.is_empty(), "{} of 500 words differ:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn rider_constants_come_from_the_table() {
    for (i, r) in SURFACE_TABLE.iter().enumerate() {
        assert_eq!(SURFACES[i].map(f32::to_bits), [r[0], r[1], r[2], r[3], r[4], r[5], r[11], r[12]].map(f32::to_bits), "SURFACES row {i}");
        assert_eq!(SURF_EXTRA[i].map(f32::to_bits), [r[6], r[9]].map(f32::to_bits), "SURF_EXTRA row {i}");
        assert_eq!(SURF_BAND[i], (r[7] * 0.01, r[8] * 0.01), "SURF_BAND row {i}");
    }
}

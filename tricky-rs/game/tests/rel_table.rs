//! The riders' relations table against the original's (board row G4a1): `RelTable_Init` 0x167208 run in the
//! function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) fills 12 × 12 entries of four
//! words (16 bytes; row = the rider who feels, column = the rider felt about, in `rivals::NAMES` order):
//! +0 tolerance, +4 attitude, +8 the attitude's cap (row G4a2), +0xc relation (0 neutral, 1 friend,
//! 2 foe). `tricky_game::rivals` must hold the same numbers.

use r5900::Runner;
use tricky_game::rivals::{relation, ATTITUDE, CAP, TOLERANCE};

#[test]
fn rel_table_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    let t = r.mem.alloc(12 * 12 * 16);
    for i in 0..12 * 12 * 4 {
        r.mem.write_u32(t + i * 4, 0xdead_beef);
    }
    r.cpu.set_gpr(4, t as u64);
    r.call(0x167208).expect("RelTable_Init runs to its return");
    let mut wrong = Vec::new();
    for me in 0..12 {
        for other in 0..12 {
            let at = t + ((me * 12 + other) * 16) as u32;
            let (tol, att, rel) = (r.mem.read_u32(at), r.mem.read_u32(at + 4), r.mem.read_u32(at + 0xc));
            if tol != TOLERANCE[me][other] as u32 { wrong.push(format!("tolerance [{me}][{other}]: game {tol}, ours {}", TOLERANCE[me][other])); }
            if att != ATTITUDE[me][other] as u32 { wrong.push(format!("attitude [{me}][{other}]: game {att}, ours {}", ATTITUDE[me][other])); }
            let cap = r.mem.read_u32(at + 8);
            if cap != CAP[me][other] as u32 { wrong.push(format!("cap [{me}][{other}]: game {cap}, ours {}", CAP[me][other])); }
            if rel != relation(me, other) as u32 { wrong.push(format!("relation [{me}][{other}]: game {rel}, ours {}", relation(me, other))); }
        }
    }
    assert!(wrong.is_empty(), "{} values differ:\n{}", wrong.len(), wrong.join("\n"));
}

//! The game's VU1 chains through VIF1 (TRICKY_ELF; skipped without it): each of the eight that
//! `PS2Draw_WriteTexState` calls walks to its end and fills micro memory with its programs (as an extract of the
//! ELF's MPG codes counts them: 743 instructions for the patches' 4-step chain), with no VIF code unknown.

use r5900::vu1::Vu1;
use r5900::Runner;

#[test]
fn the_games_vu1_chains_load() {
    let Some(mut r) = Runner::from_env() else { return };
    for (chain, nonzero) in [(0x30f960u32, 743usize), (0x3110f0, 0), (0x312af0, 0), (0x314620, 0), (0x319e30, 0), (0x31c260, 0), (0x31c790, 0), (0x318080, 0)] {
        let mut v = Vu1::default();
        let tags = v.run_chain(&mut r.mem, chain).unwrap_or_else(|e| panic!("{chain:#x}: {e}"));
        // the instructions placed: the pairs not all zero, up to the last one
        let pairs = v.micro.chunks(8).rposition(|p| p.iter().any(|b| *b != 0)).map_or(0, |k| k + 1);
        println!("{chain:#x}: {tags} tags, {pairs} instruction pairs, BASE {:#x} OFFSET {:#x}, CL/WL {}/{}", v.base, v.offset, v.cl, v.wl);
        assert!(pairs > 0, "{chain:#x} placed no program");
        if nonzero != 0 {
            assert_eq!(pairs, nonzero, "{chain:#x}");
        }
        assert!(v.calls.is_empty(), "{chain:#x}: the chains only load");
    }
}

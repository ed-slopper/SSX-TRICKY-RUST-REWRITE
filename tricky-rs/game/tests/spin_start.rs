//! Our spin start against the original's, bit for bit (board row G1a3): the game's own `SpinState_StartFromStick`
//! 0x102158 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with
//! `Math_Atan` run as the original and the other callees recorded, and `tricky_game::jump::spin_start_from_stick`
//! on the same pad words: every pad word whose direction bits matter, both stances, every tricks stat. The pad gives
//! eight directions; the game's atan puts the diagonals just inside 45°, so clips 0x22a and 0x226 and the 10° snap
//! are never reached from it (the port keeps them as written).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::jump::spin_start_from_stick;

#[test]
fn spin_start_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2510f8); // Math_Atan
    let log = Rc::new(RefCell::new(Vec::<(&str, u64)>::new()));
    for (addr, name) in [(0x15f050u32, "layers"), (0x15fcf8, "play"), (0x156568, "score"), (0x11b018, "meter"), (0x11f608, "state")] {
        let l = log.clone();
        r.stub(addr, move |cpu, _| {
            l.borrow_mut().push((name, cpu.gpr(5)));
            cpu.set_f(0, 0.0);
        });
    }
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let spin = r.mem.alloc(0x80);
    let pad = r.mem.alloc(4);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(spin + 0x70, rd);
    let mut n = 0;
    for switch in [false, true] {
        for tricks in 0u8..=255 {
            // the four direction fields (bits 12..19) in every combination, with and without 0x400
            for bits in 0u32..256 {
                for sel in [0u32, 0x400] {
                    let word = (bits << 12) | sel | 0x8001;
                    log.borrow_mut().clear();
                    r.mem.write_u32(pad, word);
                    r.mem.write_u8(stats + 0x1a, tricks);
                    r.mem.write_u32(rd + 0x1b4, switch as u32);
                    for o in [0x2cu32, 0x30, 0x58] {
                        r.mem.write_u32(spin + o, 0xdead_beef);
                    }
                    r.cpu.set_gpr(4, spin as u64);
                    r.cpu.set_gpr(5, pad as u64);
                    r.call(0x102158).unwrap();
                    let started = r.cpu.gpr(2) as u32;
                    let what = format!("pad {word:#x} tricks {tricks} switch {switch}");
                    match spin_start_from_stick(word, tricks, switch) {
                        None => {
                            assert_eq!(started, 0, "{what}: the game started a spin");
                            assert!(log.borrow().is_empty(), "{what}: no calls");
                        }
                        Some(s) => {
                            n += 1;
                            assert_eq!(started, 1, "{what}");
                            assert_eq!(r.mem.read_u32(spin + 0x2c), s.spin.to_bits(), "{what}: spin game {} ours {}", r.mem.read_f32(spin + 0x2c), s.spin);
                            assert_eq!(r.mem.read_u32(spin + 0x30), s.flip.to_bits(), "{what}: flip game {} ours {}", r.mem.read_f32(spin + 0x30), s.flip);
                            assert_eq!(r.mem.read_u32(spin + 0x58), 1, "{what}");
                            let l = log.borrow();
                            let clip = l.iter().find(|e| e.0 == "play").map(|e| e.1 as u32);
                            assert_eq!(clip, s.clip, "{what}: clip");
                            let states: Vec<u64> = l.iter().filter(|e| e.0 == "state").map(|e| e.1).collect();
                            assert_eq!(states, vec![1, 0x10], "{what}: states");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(n, 2 * 256 * 384, "spins started");
}

//! Our take-off spin rates against the original's, bit for bit (board row G1a2): the game's own
//! `Takeoff_SetSpinRates` 0x126e30 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped
//! without it), its callees recorded (`Takeoff_Setup`, `AnimCtl_GetClip` answering the clip we pick, the two clip
//! routines, `Boarder_SetState`), and `tricky_game::jump::takeoff_spin_rates` on the same rider.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::jump::{takeoff_spin_rates, TakeoffState};

#[test]
fn takeoff_spin_rates_are_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let clip = Rc::new(RefCell::new(0i64));
    let calls = Rc::new(RefCell::new(Vec::<(&str, u64)>::new()));
    let c = clip.clone();
    r.stub(0x15fdd0, move |cpu, _| cpu.set_gpr(2, *c.borrow() as u64));
    for (addr, name) in [(0x12bee0u32, "setup"), (0x127298, "takeoff clip"), (0x126fc0, "launch clip"), (0x11f608, "state")] {
        let k = calls.clone();
        r.stub(addr, move |cpu, _| k.borrow_mut().push((name, cpu.gpr(5))));
    }
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let motion = r.mem.alloc(0x800);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(rd + 0x5ae0, motion);
    let mut x = 0x0123_4567_89ab_cdefu64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 16) as u32
    };
    let clips = [0x200i64, 0x218, 0x25f, 0x260, 0x261, 0x262, 0x100, 0x217, 0x219];
    for case in 0..5000 {
        let f = |v: u32, lo: f32, hi: f32| lo + (hi - lo) * (v % 1_000_000) as f32 / 1_000_000.0;
        let s = TakeoffState {
            clip: clips[case % clips.len()],
            x208: f(next(), 0.0, 0.6),
            tricks_stat: next() as u8,
            wind_spin: f(next(), -1.0, 1.0),
            wind_flip: f(next(), -1.0, 1.0),
            class: next() % 3,
            fakie: next() % 2 == 0,
        };
        *clip.borrow_mut() = s.clip;
        calls.borrow_mut().clear();
        r.mem.write_u8(stats + 0x1a, s.tricks_stat);
        r.mem.write_f32(rd + 0x208, s.x208);
        r.mem.write_f32(rd + 0x220, s.wind_spin);
        r.mem.write_f32(rd + 0x22c, s.wind_flip);
        r.mem.write_u32(rd + 0x420, s.class);
        r.mem.write_u32(rd + 0x1b4, s.fakie as u32);
        for o in [0x1f0u32, 0x1f4, 0x1f8] {
            r.mem.write_u32(rd + o, 0xdead_beef);
        }
        r.mem.write_u32(motion + 0x76c, 0xdead_beef);
        r.mem.write_u32(motion + 0x770, 0xdead_beef);
        r.cpu.set_gpr(4, rd as u64);
        r.call(0x126e30).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let t = takeoff_spin_rates(&s);
        assert_eq!(r.mem.read_u32(motion + 0x76c), t.spin.to_bits(), "case {case} {s:?}: spin");
        assert_eq!(r.mem.read_u32(motion + 0x770), t.flip.to_bits(), "case {case} {s:?}: flip");
        match t.x1f0 {
            Some(v) => {
                assert_eq!(r.mem.read_u32(rd + 0x1f0), v.to_bits(), "case {case}: +0x1f0");
                assert_eq!(r.mem.read_u32(rd + 0x1f8), v.to_bits(), "case {case}: +0x1f8");
                assert_eq!(r.mem.read_u32(rd + 0x1f4), 0, "case {case}: +0x1f4");
            }
            None => assert_eq!(r.mem.read_u32(rd + 0x1f0), 0xdead_beef, "case {case}: +0x1f0 untouched"),
        }
        let want_clip = if t.launch_clip { "launch clip" } else { "takeoff clip" };
        let got: Vec<&str> = calls.borrow().iter().map(|c| c.0).collect();
        assert_eq!(got, vec!["setup", want_clip, "state"], "case {case}: calls");
        assert_eq!(calls.borrow()[2].1, 0x10, "case {case}: state 0x10");
        for o in (0x220u32..=0x234).step_by(4) {
            assert_eq!(r.mem.read_u32(rd + o), 0, "case {case}: rider+{o:#x} cleared");
        }
    }
}

//! Our landing state choice against the original's (board row E8a2): the game's own `Landing_ChooseState`
//! 0x109308 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), its callees
//! recorded, and `tricky_game::landing::choose_state` on the same rider: same clip, state, resets and turn, bit
//! for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::landing::{choose_state, ChooseState};

#[test]
fn landing_choice_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50); // Vec4_Scale
    let log = Rc::new(RefCell::new(Vec::<(&str, u64, u32)>::new()));
    let clip_type = Rc::new(RefCell::new(0i64));
    let ct = clip_type.clone();
    r.stub(0x15eec0, move |cpu, _| cpu.set_gpr(2, *ct.borrow() as u64));
    for (addr, name) in [(0x11f610u32, "motion"), (0x15fcf8, "play"), (0x11f608, "state"), (0x128e90, "rotate"), (0x15eef8, "turn layers")] {
        let l = log.clone();
        r.stub(addr, move |cpu, _| l.borrow_mut().push((name, cpu.gpr(5), cpu.f(12).to_bits())));
    }
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let motion = r.mem.alloc(0x40);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(motion + 0x20, rd);
    let mut x = 0x1a4d_c405_1a4d_c405u64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 16) as u32
    };
    let f = |v: u32, lo: f32, hi: f32| lo + (hi - lo) * (v % 1_000_000) as f32 / 1_000_000.0;
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..6000 {
        let s = ChooseState { clip_type: [0x1i64, 0xe, 0x10, 0x3][(next() % 4) as usize], class: next() % 3, switch: next() % 2, stance: next() % 2 };
        let (yaw, pitch, vn) = (f(next(), -3.2, 3.2), f(next(), -1.6, 1.6), f(next(), -4000.0, 200.0));
        *clip_type.borrow_mut() = s.clip_type;
        log.borrow_mut().clear();
        r.mem.write_u32(rd + 0x420, s.class);
        r.mem.write_u32(rd + 0x1b4, s.switch);
        r.mem.write_u32(stats + 0x40, s.stance);
        let fwd = [f(next(), -1.0, 1.0), f(next(), -1.0, 1.0), f(next(), -1.0, 1.0), 0.0];
        let side = [f(next(), -1.0, 1.0), f(next(), -1.0, 1.0), f(next(), -1.0, 1.0), 0.0];
        for k in 0..4 {
            r.mem.write_f32(rd + 0x320 + 4 * k, fwd[k as usize]);
            r.mem.write_f32(rd + 0x330 + 4 * k, side[k as usize]);
        }
        for o in (0x1fcu32..=0x21c).step_by(4).chain([0x268, 0x26c, 0x270, 0x46e0]) {
            r.mem.write_u32(rd + o, 0xdead_beef);
        }
        r.cpu.set_f(12, yaw);
        r.cpu.set_f(13, pitch);
        r.cpu.set_f(14, vn);
        r.cpu.set_gpr(4, motion as u64);
        r.call(0x109308).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let c = choose_state(&s, yaw, pitch, vn);
        seen.insert((c.clip, c.state, c.turned.is_some()));
        let l = log.borrow().clone();
        let names: Vec<&str> = l.iter().map(|e| e.0).collect();
        let want: Vec<&str> = if c.turned.is_some() { vec!["motion", "rotate", "turn layers", "play", "state"] } else { vec!["motion", "play", "state"] };
        assert_eq!(names, want, "case {case} {s:?} yaw {yaw} pitch {pitch} vn {vn}");
        assert_eq!(l[0].1, 2, "case {case}: motion 2");
        let play = l.iter().find(|e| e.0 == "play").unwrap();
        assert_eq!(play.1 as u32, c.clip, "case {case} {s:?} yaw {yaw} pitch {pitch} vn {vn}: clip");
        assert_eq!(l.last().unwrap().1 as u32, c.state, "case {case}: state");
        assert_eq!(r.mem.read_u32(rd + 0x208) == 0x3f80_0000, c.grab_reset, "case {case}: grab reset");
        match c.turned {
            Some((sw, ang)) => {
                assert_eq!(r.mem.read_u32(rd + 0x1b4), sw, "case {case}: switch");
                assert_eq!(r.mem.read_u32(rd + 0x46e0), ang.to_bits(), "case {case}: heading");
                assert_eq!(l[1].2, std::f32::consts::PI.to_bits(), "case {case}: turn π");
                for k in 0..4 {
                    assert_eq!(r.mem.read_u32(rd + 0x320 + 4 * k), (fwd[k as usize] * -1.0).to_bits(), "case {case}: forward");
                    assert_eq!(r.mem.read_u32(rd + 0x330 + 4 * k), (side[k as usize] * -1.0).to_bits(), "case {case}: side");
                }
            }
            None => assert_eq!(r.mem.read_u32(rd + 0x1b4), s.switch, "case {case}: switch kept"),
        }
    }
    assert!(seen.len() >= 8, "outcomes seen: {seen:?}");
}

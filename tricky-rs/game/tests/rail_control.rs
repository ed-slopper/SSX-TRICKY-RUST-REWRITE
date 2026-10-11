//! The rail's control state against the original's (board row G1b): `RailSlideControl_Update` 0x1073e0 with the
//! real `RailSlideControl_CommitQuarterTurn` 0x107730 and `RailSlideControl_PlayQueuedClip` 0x107628, run in the
//! function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) on random pad words and rail
//! states, every call and field against `tricky_game::rail_control`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::rail_control::{update, RailCall, RailControl, RailRider};

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
fn rail_control_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x107730);
    r.allow(0x107628);
    let log: Rc<RefCell<Vec<RailCall>>> = Rc::default();
    let rider = r.mem.alloc(0x6000);
    let (obj, pad, stats) = (r.mem.alloc(0x40), r.mem.alloc(16), r.mem.alloc(0x100));
    r.mem.write_u32(obj + 0x14, rider);
    r.mem.write_u32(rider + 0x464, stats);
    let at_end = Rc::new(Cell::new(false));
    let anim = rider + 0x46c0;
    let stubs: Vec<(u32, Box<dyn Fn(&mut r5900::Cpu) -> RailCall>)> = vec![
        (0x11d038, Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.gpr(6)), (rider, 0)); RailCall::Boost(c.gpr(5) as u32) })),
        (0x126ac8, Box::new(move |c| { assert_eq!(c.gpr(4) as u32, rider); RailCall::SteerTarget(c.f(12).to_bits()) })),
        (0x126c00, Box::new(move |c| { assert_eq!(c.gpr(4) as u32, rider); RailCall::CrouchTarget(c.f(12).to_bits()) })),
        (0x11f608, Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider, 0xe)); RailCall::Prewind })),
        (0x15ff40, {
            let ae = at_end.clone();
            Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (anim, 0)); c.set_gpr(2, ae.get() as u64); RailCall::ClipAtEnd })
        }),
        (0x15eef8, Box::new(move |c| { assert_eq!(c.gpr(4) as u32, anim); RailCall::TurnLayers(c.f(12).to_bits()) })),
        (0x155e00, Box::new(move |c| { assert_eq!(c.gpr(4) as u32, rider + 0x5820); RailCall::ScoreSpin(c.gpr(5) as i32) })),
        (0x106dc0, Box::new(move |c| { assert_eq!(c.gpr(4) as u32, rider); RailCall::OnClipQueued })),
        (0x15ef48, Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (anim, 0)); RailCall::Anim15ef48 })),
        (0x15fcf8, Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.gpr(6), c.gpr(7)), (anim, 0, 5)); RailCall::PlayClip(c.gpr(5) as u32) })),
        (0x118f10, Box::new(move |c| { assert_eq!((c.gpr(4) as u32, c.f(12).to_bits()), (rider, 0x42ca_ebe0)); RailCall::Reset })),
    ];
    for (a, f) in stubs {
        let l = log.clone();
        r.stub(a, move |c, _| {
            let call = f(c);
            l.borrow_mut().push(call);
        });
    }
    let mut g = Rng(0x7b3d_29a6_e01c_548f);
    let mut turns = 0;
    for case in 0..60_000 {
        // pad words: the bits that matter, the stick axes often zero
        let mut p = g.next() ^ (g.next() << 16);
        if g.next() % 2 == 0 {
            p &= !(0x3f << 17); // no turn axis
        }
        if g.next() % 3 == 0 {
            p &= !0xe;
        }
        let mut c = RailControl { queued: [0, 0, 1, 2, 3, 4, 5][g.next() as usize % 7], spin: (g.next() % 9) as i32 * 90 - 360, armed: g.next() % 2 };
        let mut rr = RailRider {
            kind: g.next() % 6,
            stance: g.next() % 2,
            x46d4: g.next() % 2,
            x46e0: 7.0,
            crouch: [0.0, 0.5][g.next() as usize % 2],
            class: g.next() % 3,
            stats40: g.next() % 2,
        };
        let end = g.next() % 2 == 0;
        at_end.set(end);
        let m = &mut r.mem;
        m.write_u32(pad, p);
        m.write_u32(obj + 8, c.queued);
        m.write_u32(obj + 0xc, c.spin as u32);
        m.write_u32(obj + 0x10, c.armed);
        m.write_u32(rider + 0x1dc, rr.kind);
        m.write_u32(rider + 0x1b4, rr.stance);
        m.write_u32(rider + 0x46d4, rr.x46d4);
        m.write_f32(rider + 0x46e0, rr.x46e0);
        m.write_f32(rider + 0x208, rr.crouch);
        m.write_u32(rider + 0x420, rr.class);
        m.write_u32(stats + 0x40, rr.stats40);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x1073e0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let input = (c, rr);
        let calls = update(&mut c, &mut rr, p, &mut || end);
        let m = &mut r.mem;
        let ctx = format!("case {case}: pad {p:#010x} {input:?} clip at end {end}");
        assert_eq!(*log.borrow(), calls, "{ctx}: calls");
        let theirs = (m.read_u32(obj + 8), m.read_u32(obj + 0xc) as i32, m.read_u32(obj + 0x10), m.read_u32(rider + 0x1dc), m.read_u32(rider + 0x1b4), m.read_u32(rider + 0x46d4), m.read_u32(rider + 0x46e0));
        let ours = (c.queued, c.spin, c.armed, rr.kind, rr.stance, rr.x46d4, rr.x46e0.to_bits());
        assert_eq!(theirs, ours, "{ctx}: fields");
        turns += calls.iter().filter(|c| matches!(c, RailCall::ScoreSpin(_))).count();
    }
    assert!(turns > 3000, "{turns}");
}

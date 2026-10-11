//! The board's frame against the original's (board row G16): `Boarder_UpdatePose` 0x129628 run in the function
//! runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), the frame (rider+0x4aa0..) taken when it
//! calls `Boarder_UpdateSkeleton` (every later call stubbed), against `tricky_game::pose::board_frame`, bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::pose::{board_frame, PoseIn};

type V4 = [f32; 4];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

fn put4(m: &mut r5900::Mem, at: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

#[test]
fn board_frame_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let real = [0x250c10u32, 0x102f50];
    for a in real {
        r.allow(a);
    }
    let rider = r.mem.alloc(0x6000);
    let stats = r.mem.alloc(0x100);
    r.mem.write_u32(rider + 0x464, stats);
    // every other call of the function returns 0; the skeleton call records the frame
    let frame: Rc<RefCell<Option<[u32; 16]>>> = Rc::default();
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..(0x1400 / 4) {
        let w = r.mem.read_u32(0x129628 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    for a in jals {
        if real.contains(&a) {
            continue;
        }
        if a == 0x12ae18 {
            let fr = frame.clone();
            r.stub(a, move |_c, m| {
                let mut w = [0u32; 16];
                for (i, x) in w.iter_mut().enumerate() {
                    *x = m.read_u32(rider + 0x4aa0 + 4 * i as u32);
                }
                *fr.borrow_mut() = Some(w);
            });
        } else {
            r.stub(a, |c, _| c.set_gpr(2, 0));
        }
    }
    let mut g = Rng(0x6c2e_08b5_f1d4_973a);
    let mut tilted = 0;
    for case in 0..40_000 {
        let mut o = Orient { q: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)], ..Default::default() };
        update_orientation(&mut o);
        let unit = |g: &mut Rng| {
            let v = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
            [v[0] / l, v[1] / l, v[2] / l, 0.0]
        };
        let p = PoseIn {
            quat: o.q,
            steer: [0.0, g.f(-1.0, 1.0), -0.0][g.next() as usize % 3],
            motion: [2u32, 3, 1][g.next() as usize % 3],
            class: g.next() % 3,
            pos: [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), [1.0, 0.0][g.next() as usize % 2]],
            up: unit(&mut g),
            height: g.f(0.0, 40.0),
            side: unit(&mut g),
            lean: g.f(-30.0, 30.0),
            lean_scale: g.f(0.0, 1.0),
            spin: [0.0, g.f(-12.0, 12.0)][g.next() as usize % 2],
            flip: [0.0, g.f(-12.0, 12.0)][g.next() as usize % 2],
            pivot_z: g.f(0.0, 120.0),
        };
        let bone = g.next() % 8;
        let m = &mut r.mem;
        put4(m, rider + 0x170, p.quat);
        m.write_f32(rider + 0x214, p.steer);
        m.write_u32(rider + 0x424, p.motion);
        m.write_u32(rider + 0x420, p.class);
        put4(m, rider + 0x140, p.pos);
        put4(m, rider + 0x2c0, p.up);
        m.write_f32(rider + 0x308, p.height);
        put4(m, rider + 0x330, p.side);
        m.write_f32(rider + 0x244, p.lean);
        m.write_f32(stats + 0x60, p.lean_scale);
        m.write_f32(rider + 0x1d0, p.spin);
        m.write_f32(rider + 0x1d4, p.flip);
        m.write_u32(rider + 0x44d8, bone);
        m.write_f32(rider + bone * 0x394 + 0x8d0, p.pivot_z);
        m.write_u32(stats + 0x78, 0);
        *frame.borrow_mut() = None;
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x129628).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = frame.borrow().expect("the skeleton call");
        let ours: Vec<u32> = board_frame(&p).iter().flat_map(|row| row.map(f32::to_bits)).collect();
        assert_eq!(theirs.to_vec(), ours, "case {case}: {p:?}");
        tilted += (p.steer != 0.0 && p.motion != 3) as u32;
    }
    assert!(tilted > 5000, "{tilted}");
}

#[test]
fn body_height_easing_matches_the_original() {
    // `cBoarder::vf7` 0x117198 run with every call stubbed: rider+0x308 against `pose::ease_body_height`
    use tricky_game::pose::ease_body_height;
    use tricky_game::surface::SURFACE_TABLE;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let real = [0x250c10u32, 0x102f50, 0x2510f8];
    for a in real {
        r.allow(a);
    }
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..(0x1000 / 4) {
        let w = r.mem.read_u32(0x117198 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    for a in jals {
        if !real.contains(&a) {
            r.stub(a, |c, _| c.set_gpr(2, 0));
        }
    }
    let rider = r.mem.alloc(0x6000);
    let (stats, app, game, table) = (r.mem.alloc(0x100), r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(2000));
    r.mem.write_u32(rider + 0x464, stats);
    // the rider's virtuals: every slot a stub (the this-adjust halves 0)
    let vt = r.mem.alloc(0x200);
    for k in 0..0x80u32 {
        r.mem.write_u32(vt + 4 * k, if k % 2 == 1 { 0x1000 } else { 0 });
    }
    r.mem.write_u32(rider, vt);
    r.stub(0x1000, |c, _| c.set_gpr(2, 0));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x24, table);
    for (s, row) in SURFACE_TABLE.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            r.mem.write_f32(table + 100 * s as u32 + 4 * k as u32, *v);
        }
    }
    let mut g = Rng(0x1d4c_83f0_5ea9_27b6);
    for case in 0..20_000 {
        let h = [g.f(-5.0, 5.0), 0.0, SURFACE_TABLE[3][10]][g.next() as usize % 3];
        let motion = [1u32, 2, 3, 5][g.next() as usize % 4];
        let surface = g.next() % 20;
        r.mem.write_f32(rider + 0x308, h);
        r.mem.write_u32(rider + 0x424, motion);
        r.mem.write_u32(rider + 0x290, surface);
        r.mem.write_u32(rider + 0x46b0, 1);
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x117198).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let (s2, m2) = (r.mem.read_u32(rider + 0x290), r.mem.read_u32(rider + 0x424));
        assert_eq!((s2, m2), (surface, motion), "case {case}: surface or motion changed");
        let ours = ease_body_height(h, motion, SURFACE_TABLE[surface as usize][10]);
        assert_eq!(r.mem.read_u32(rider + 0x308), ours.to_bits(), "case {case}: height {h}, motion {motion}, surface {surface}");
    }
}

//! The audio system's small calculations (`tricky_game::sfxcalc`, board row G10a) against the original in the
//! function runner (TRICKY_ELF; skipped without it).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::sfxcalc::*;

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

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

#[test]
fn surface_sound_type_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let sys = r.mem.alloc(0x7000);
    let rider = r.mem.alloc(0x500);
    for motion in [1u32, 2, 3, 5] {
        for rail in (-1i32..=0x20).map(|x| x as u32).chain([0x7fff_ffff, 0x8000_0000]) {
            for own in [rail, 0u32, 0xffff_ffff, 12] {
                r.mem.write_u32(rider + 0x424, motion);
                r.mem.write_u32(rider + 0x290, own);
                r.mem.write_u32(sys + 0x6e40, rail);
                r.cpu.set_gpr(4, sys as u64);
                r.cpu.set_gpr(5, rider as u64);
                r.call(0x20f908).unwrap();
                assert_eq!(r.cpu.gpr(2) as u32, surface_sound_type(motion, rail, own), "motion {motion} rail {rail:#x} own {own:#x}");
            }
        }
    }
}

#[test]
fn eval_curve_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x1d2c_3b4a_5968_7786);
    for case in 0..40_000 {
        let slot = g.next() % 8;
        let mut xs = [0i16; 4];
        let mut x = (g.next() % 2000) as i16 - 1000;
        for v in xs.iter_mut() {
            *v = x;
            x += [0, 1, (g.next() % 400) as i16][g.next() as usize % 3];
        }
        let ys: [i16; 4] = std::array::from_fn(|_| (g.next() % 6000) as i16 - 3000);
        let row = 0x343098 + slot * 16;
        for k in 0..4 {
            r.mem.write_u32(row + 2 * k as u32 - (2 * k as u32 & 3), 0); // keep word alignment simple below
        }
        let mut bytes = [0u8; 16];
        for k in 0..4 {
            bytes[2 * k..2 * k + 2].copy_from_slice(&xs[k].to_le_bytes());
            bytes[8 + 2 * k..10 + 2 * k].copy_from_slice(&ys[k].to_le_bytes());
        }
        for w in 0..4 {
            r.mem.write_u32(row + 4 * w as u32, u32::from_le_bytes(bytes[4 * w..4 * w + 4].try_into().unwrap()));
        }
        let probe = [xs[0] as i64 - 3, xs[1] as i64, xs[2] as i64, xs[3] as i64, xs[3] as i64 + 5, (g.next() % 2400) as i64 - 1200, xs[0] as i64, xs[1] as i64 - 1];
        let v = probe[g.next() as usize % probe.len()];
        r.cpu.set_gpr(4, slot as u64);
        r.cpu.set_gpr(5, v as u64);
        // a run of equal points would divide by zero only if x sat in an empty interval, which it cannot
        r.call(0x20d928).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let want = r.cpu.gpr(2) as i64;
        assert_eq!(eval_curve(xs, ys, v), want, "case {case}: {xs:?} {ys:?} x {v}");
    }
}

#[test]
fn impact_speed_and_listener_helpers_match_the_original() {
    let Some(mut r) = runner() else { return };
    let rider = r.mem.alloc(0x500);
    let mut g = Rng(0x0f0e_0d0c_0b0a_0908);
    for case in 0..20_000 {
        let n = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), 0.0];
        let v = [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), 0.0];
        put4(&mut r.mem, rider + 800, n);
        put4(&mut r.mem, rider + 0x150, v);
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x21a6a8).unwrap();
        assert_eq!(r.cpu.gpr(2) as i32, impact_speed(n, v), "impact {case}");
    }
    // IsNearerPlayer2 with the two positions from Audio_GetRiderPos
    let pos = Rc::new(RefCell::new([[0f32; 4]; 2]));
    let p = pos.clone();
    let slots = [r.mem.alloc(0x10), r.mem.alloc(0x10)];
    r.stub(0x21c010, move |c, _| c.set_gpr(2, slots[c.gpr(5) as usize] as u64));
    let at = r.mem.alloc(0x10);
    for case in 0..20_000 {
        let pp = [[g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0), 0.0], [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0), 0.0]];
        let a = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0), 0.0];
        *p.borrow_mut() = pp;
        put4(&mut r.mem, slots[0], pp[0]);
        put4(&mut r.mem, slots[1], if case % 7 == 0 { pp[0] } else { pp[1] });
        put4(&mut r.mem, at, a);
        r.cpu.set_gpr(4, 0x1000);
        r.cpu.set_gpr(5, at as u64);
        r.call(0x20fb68).unwrap();
        let p1 = pp[0];
        let p2 = if case % 7 == 0 { pp[0] } else { pp[1] };
        assert_eq!(r.cpu.gpr(2) as u32 & 1 != 0, is_nearer_player2(p1, p2, a), "nearer {case}");
    }
    // Audio_GetRiderListener
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let count = Rc::new(RefCell::new(2i64));
    let c2 = count.clone();
    r.stub(0x21c078, move |c, _| c.set_gpr(2, *c2.borrow() as u64));
    let near = Rc::new(RefCell::new(false));
    let nr = near.clone();
    r.stub(0x20fb68, move |c, _| c.set_gpr(2, *nr.borrow() as u64));
    for case in 0..2000 {
        let has_game = g.next() % 4 != 0;
        r.mem.write_u32(app + 0x730, if has_game { game } else { 0 });
        let f41c = [1u32, 0, 2][g.next() as usize % 3];
        let pl = g.next() % 3;
        let humans = 1 + (g.next() % 3) as i64;
        let nearer = g.next() % 2 == 0;
        r.mem.write_u32(rider + 0x41c, f41c);
        r.mem.write_u32(rider + 0x38, pl);
        *count.borrow_mut() = humans;
        *near.borrow_mut() = nearer;
        r.cpu.set_gpr(4, 0x1000);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x20fc60).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32, rider_listener(has_game, f41c, pl, humans, || nearer), "listener {case}");
    }
}

#[test]
fn near_ahead_volume_and_music_state_match_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x7766_5544_3322_1100);
    let list: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x10)).collect();
    let n = Rc::new(RefCell::new(0i64));
    let nn = n.clone();
    r.stub(0x21c078, move |c, _| c.set_gpr(2, *nn.borrow() as u64));
    let l2 = list.clone();
    r.stub(0x21c010, move |c, _| c.set_gpr(2, l2[c.gpr(5) as usize] as u64));
    let me = r.mem.alloc(0x10);
    for case in 0..20_000 {
        let count = (g.next() % 7) as usize;
        *n.borrow_mut() = count as i64;
        let radius = g.f(0.0, 12.0);
        let at = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0)];
        let ps: Vec<[f32; 3]> = (0..count)
            .map(|k| {
                let p = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-500.0, 500.0)];
                for (j, x) in p.iter().enumerate() {
                    r.mem.write_f32(list[k] + 4 * j as u32, *x);
                }
                p
            })
            .collect();
        for (j, x) in at.iter().enumerate() {
            r.mem.write_f32(me + 4 * j as u32, *x);
        }
        r.cpu.set_f(12, radius);
        r.cpu.set_gpr(4, 0x1000);
        r.cpu.set_gpr(5, me as u64);
        r.call(0x20fa80).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 & 1 != 0, any_rider_near_ahead(radius, at, &ps), "near ahead {case}");
    }
    // volume steps: the call goes to SndMgr_SetVolume 0x211db0 with the new value in f12
    let vol = Rc::new(RefCell::new(0u32));
    let v2 = vol.clone();
    r.stub(0x211db0, move |c, _| *v2.borrow_mut() = c.f(12).to_bits());
    let sys = r.mem.alloc(0x4000);
    for case in 0..2000 {
        let slot = g.next() % 4;
        let cur = g.f(0.0, 1.0);
        r.mem.write_f32(sys + 0x3848 + 0xc * slot, cur);
        for up in [true, false] {
            r.cpu.set_gpr(4, sys as u64);
            r.cpu.set_gpr(5, slot as u64);
            r.call(if up { 0x211f68 } else { 0x211f90 }).unwrap();
            assert_eq!(*vol.borrow(), volume_step(cur, up).to_bits(), "volume {case} {up}");
        }
    }
    for s in -3i32..8 {
        r.cpu.set_gpr(4, s as i64 as u64);
        r.call(0x20d9f0).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32, music_state_to_mode(s), "music state {s}");
    }
}

//! `SpinControl_Enter` 0x100598 with the real `SpinControl_CalcLandingAlign` 0x11db00 under it (board row G18a),
//! run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) on random attitudes and
//! landing slopes, against `tricky_game::spin::spin_enter` and `landing_align`, bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::spin::{landing_align, spin_enter, AlignRider, SpinEnterRider, SpinObj};

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
fn get4(m: &mut r5900::Mem, at: u32) -> [u32; 4] {
    [m.read_u32(at), m.read_u32(at + 4), m.read_u32(at + 8), m.read_u32(at + 12)]
}
fn bits(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[test]
fn spin_enter_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x11db00u32, 0x2508b8, 0x102e80, 0x10c120, 0x250fd0, 0x250d60, 0x250c10, 0x102f80, 0x102f50, 0x2510f8] {
        r.allow(a);
    }
    let clip = std::rc::Rc::new(std::cell::Cell::new(0u32));
    let cl = clip.clone();
    let rider = r.mem.alloc(0x6000);
    r.stub(0x15eec0, move |c, _| {
        assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider + 0x46c0, 0));
        c.set_gpr(2, cl.get() as u64);
    });
    let (st, stats) = (r.mem.alloc(0x100), r.mem.alloc(0x100));
    r.mem.write_u32(st + 0x70, rider);
    r.mem.write_u32(rider + 0x464, stats);
    let mut g = Rng(0x51a7_e8c0_3d29_b46f);
    let (mut dropped, mut swapped, mut aligned) = (0, 0, 0);
    for case in 0..40_000 {
        let mut o = Orient { q: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)], ..Default::default() };
        update_orientation(&mut o);
        let slope: V4 = match g.next() % 4 {
            0 => [0.0, 0.0, 1.0, 0.0],
            1 => [g.f(-0.3, 0.3), g.f(-0.3, 0.3), g.f(0.8, 1.0), 0.0],
            _ => [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), 0.0],
        };
        // a level slope with the board's forward along or against its own (the parallel case), at times
        let (slope, fwd) = if g.next() % 5 == 0 {
            let y = if g.next() % 2 == 0 { 1.0 } else { -1.0 };
            ([0.0, 0.0, 1.0, 0.0], [0.0, y, [0.0, g.f(-0.5, 0.5)][g.next() as usize % 2], 0.0])
        } else {
            (slope, o.fwd)
        };
        let a = AlignRider { slope_kind: [0u32, 3, 6, 10, 0, 0][g.next() as usize % 6], slope, fwd, quat: o.q, class: g.next() % 3 };
        let mut s = SpinObj {
            mode: 9,
            phase: g.f(-3.0, 3.0),
            owed: g.f(-3.0, 3.0),
            w_spin: [0.0, g.f(-12.0, 12.0), 1.3962634 / 1.6][g.next() as usize % 3],
            w_flip: [0.0, g.f(-12.0, 12.0)][g.next() as usize % 2],
            align: g.f(-3.0, 3.0),
            align_axis: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)],
            align_w: g.f(-1.0, 1.0),
            emax_spin: g.f(0.0, 5.0),
            emax_flip: g.f(0.0, 5.0),
            x5c: 1,
            x60: 1,
            x64: 1,
            ..Default::default()
        };
        let mut sr = SpinEnterRider {
            x138: [0.0, g.f(0.01, 2.0), -1.0][g.next() as usize % 3],
            switch: g.next() % 2,
            stance: g.next() % 2,
            kind: [0u32, 3][g.next() as usize % 2],
            up_z: [g.f(-1.0, 1.0), 0.0, -0.0][g.next() as usize % 3],
            wind_spin: g.f(-1.0, 1.0),
            wind_flip: g.f(-1.0, 1.0),
            x218: g.f(-1.0, 1.0),
            x21c: g.f(-1.0, 1.0),
            x268: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)],
        };
        let ct = [0xbu32, 3, 0][g.next() as usize % 3];
        clip.set(ct);
        let m = &mut r.mem;
        m.write_u32(st + 8, s.mode);
        m.write_f32(st + 0xc, s.phase);
        m.write_f32(st + 0x10, s.owed);
        for (k, v) in [s.spin_hi, s.spin_lo, s.spin_floor, s.flip_hi, s.flip_lo, s.flip_floor, s.w_spin, s.w_flip, s.align].iter().enumerate() {
            m.write_f32(st + 0x14 + 4 * k as u32, *v);
        }
        put4(m, st + 0x40, [s.align_axis[0], s.align_axis[1], s.align_axis[2], s.align_w]);
        m.write_f32(st + 0x50, s.emax_spin);
        m.write_f32(st + 0x54, s.emax_flip);
        m.write_u32(st + 0x5c, s.x5c);
        m.write_u32(st + 0x60, s.x60);
        m.write_u32(st + 0x64, s.x64);
        m.write_f32(rider + 0x138, sr.x138);
        put4(m, rider + 0x170, o.q);
        put4(m, rider + 0x180, a.fwd);
        m.write_f32(rider + 0x1a8, sr.up_z);
        m.write_u32(rider + 0x1b4, sr.switch);
        m.write_u32(stats + 0x40, sr.stance);
        m.write_u32(rider + 0x1dc, sr.kind);
        m.write_f32(rider + 0x218, sr.x218);
        m.write_f32(rider + 0x21c, sr.x21c);
        for k in 0..3 {
            m.write_f32(rider + 0x268 + 4 * k as u32, sr.x268[k]);
        }
        m.write_f32(rider + 0x274, sr.wind_spin);
        m.write_f32(rider + 0x278, sr.wind_flip);
        m.write_u32(rider + 0x420, a.class);
        put4(m, rider + 0x57e0, slope);
        m.write_u32(rider + 0x57f0, a.slope_kind);
        r.cpu.set_gpr(4, st as u64);
        r.call(0x100598).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let input = s;
        // the alignment alone, as `spin_enter` asks it
        let mut ax = [input.align_axis[0], input.align_axis[1], input.align_axis[2], input.align_w];
        let mut an = input.align;
        landing_align(&a, &mut ax, &mut an);
        spin_enter(&mut s, &mut sr, &a, ct);
        let m = &mut r.mem;
        let ctx = format!("case {case}: {a:?} {input:?} {sr:?} clip type {ct:#x}");
        let want: Vec<(u32, u32)> = vec![
            (0x8, s.mode), (0xc, s.phase.to_bits()), (0x10, s.owed.to_bits()),
            (0x14, s.spin_hi.to_bits()), (0x18, s.spin_lo.to_bits()), (0x1c, s.spin_floor.to_bits()),
            (0x20, s.flip_hi.to_bits()), (0x24, s.flip_lo.to_bits()), (0x28, s.flip_floor.to_bits()),
            (0x2c, s.w_spin.to_bits()), (0x30, s.w_flip.to_bits()), (0x34, s.align.to_bits()),
            (0x40, s.align_axis[0].to_bits()), (0x44, s.align_axis[1].to_bits()), (0x48, s.align_axis[2].to_bits()), (0x4c, s.align_w.to_bits()),
            (0x50, s.emax_spin.to_bits()), (0x54, s.emax_flip.to_bits()), (0x5c, s.x5c), (0x60, s.x60), (0x64, s.x64),
        ];
        let theirs: Vec<(u32, u32)> = want.iter().map(|(o, _)| (*o, m.read_u32(st + o))).collect();
        assert_eq!(theirs, want, "{ctx}: object");

        let rd = (m.read_u32(rider + 0x218), m.read_u32(rider + 0x21c), [0, 1, 2].map(|k| m.read_u32(rider + 0x268 + 4 * k)), m.read_u32(rider + 0x274), m.read_u32(rider + 0x278));
        let rw = (sr.x218.to_bits(), sr.x21c.to_bits(), sr.x268.map(f32::to_bits), sr.wind_spin.to_bits(), sr.wind_flip.to_bits());
        assert_eq!(rd, rw, "{ctx}: rider");
        dropped += (an != 0.0 && s.align == 0.0) as u32;
        aligned += (s.align != 0.0) as u32;
        swapped += (a.class != 2 && a.slope_kind != 6 && a.slope_kind != 10) as u32;
    }
    assert!(dropped > 1000 && aligned > 5000 && swapped > 5000, "dropped {dropped}, aligned {aligned}, two-way {swapped}");
}

#[test]
fn spin_exit_matches_the_original() {
    // `SpinControl_Exit` 0x1029d0 against `spin::spin_exit`: the attitude and its axes, the position, spin and flip
    use tricky_game::spin::{spin_exit, SpinRider};
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x102e80u32, 0x250c10, 0x129008, 0x2510f8, 0x102f80, 0x102f50] {
        r.allow(a);
    }
    let rider = r.mem.alloc(0x6000);
    let st = r.mem.alloc(0x100);
    r.mem.write_u32(st + 0x70, rider);
    let mut g = Rng(0x2c6f_90e1_b47a_d538);
    let mut moved = 0;
    for case in 0..40_000 {
        let mut o = Orient { q: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)], ..Default::default() };
        update_orientation(&mut o);
        let mut sr = SpinRider {
            quat: o.q,
            spin: [0.0, g.f(-20.0, 20.0)][g.next() as usize % 2],
            flip: [0.0, g.f(-20.0, 20.0)][g.next() as usize % 2],
            pos: [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), [1.0, 0.0][g.next() as usize % 2]],
            bone: [g.f(-50.0, 50.0), g.f(-50.0, 50.0), g.f(-20.0, 120.0)],
            ..Default::default()
        };
        let bone_index = g.next() % 8;
        let m = &mut r.mem;
        put4(m, rider + 0x170, o.q);
        for at in [0x180u32, 0x190, 0x1a0] {
            put4(m, rider + at, [9.0; 4]);
        }
        m.write_f32(rider + 0x1b0, 9.0);
        put4(m, rider + 0x140, sr.pos);
        m.write_f32(rider + 0x1d0, sr.spin);
        m.write_f32(rider + 0x1d4, sr.flip);
        m.write_u32(rider + 0x44d8, bone_index);
        let b = rider + bone_index * 0x394 + 0x8c8;
        m.write_f32(b, sr.bone[0]);
        m.write_f32(b + 4, sr.bone[1]);
        m.write_f32(b + 8, sr.bone[2]);
        r.cpu.set_gpr(4, st as u64);
        r.call(0x1029d0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let input = sr;
        let out = spin_exit(&mut sr);
        let m = &mut r.mem;
        let ctx = format!("case {case}: {input:?}");
        let theirs = (get4(m, rider + 0x170), get4(m, rider + 0x140), m.read_u32(rider + 0x1d0), m.read_u32(rider + 0x1d4));
        assert_eq!(theirs, (bits(sr.quat), bits(sr.pos), sr.spin.to_bits(), sr.flip.to_bits()), "{ctx}");
        if let Some(o) = out {
            let axes = (get4(m, rider + 0x180), get4(m, rider + 0x190), get4(m, rider + 0x1a0), m.read_u32(rider + 0x1b0));
            assert_eq!(axes, (bits(o.fwd), bits(o.side), bits(o.up), o.yaw.to_bits()), "{ctx}: axes");
            moved += 1;
        }
    }
    assert!(moved > 20_000, "{moved}");
}

//! The take-off flight prediction against the original's (board row E8a3): `AirPredict_AtTakeoff` 0x123b10 (with
//! `AirPredict_Reset` and the real `Air_IntegrateRK4`) run in the function runner on the player's SLUS_203.26
//! (TRICKY_ELF; skipped without it), the world ray answered from a script (each segment's ends recorded through the
//! `cWorldLine` constructor), against `tricky_game::air::predict_at_takeoff`, bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::air::{predict_at_takeoff, RayHit};

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
fn takeoff_prediction_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x123ab8);
    r.allow(0x12b340);
    // the script: for each segment, the fraction and the hit record
    let script: Rc<RefCell<Vec<(f32, RayHit)>>> = Rc::default();
    let segs: Rc<RefCell<Vec<([u32; 4], [u32; 4], u32, u64)>>> = Rc::default();
    let sg = segs.clone();
    r.stub(0x25d7f0, move |c, m| sg.borrow_mut().push((get4(m, c.gpr(5) as u32), get4(m, c.gpr(6) as u32), c.f(12).to_bits(), c.gpr(7))));
    let sc = script.clone();
    r.stub(0x25aff8, move |c, m| {
        assert_eq!(c.gpr(4), 0x346488);
        let (h, hit) = sc.borrow_mut().remove(0);
        let out = c.gpr(6) as u32;
        put4(m, out + 0x10, hit.point);
        put4(m, out + 0x20, hit.normal);
        m.write_u32(out + 0x34, hit.word);
        c.set_f(0, h);
    });
    let rider = r.mem.alloc(0x6000);
    let pr = rider + 0x57b0;
    r.mem.write_u32(pr + 0x60, rider);
    let mut g = Rng(0x40e1_7c95_2bd3_a86f);
    let (mut landed, mut rising) = (0, 0);
    for case in 0..20_000 {
        let pos = [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-2000.0, 2000.0), [1.0, 0.0][g.next() as usize % 2]];
        let vel = [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), [g.f(0.0, 1500.0), g.f(-800.0, 0.0), 0.0][g.next() as usize % 3], 0.0];
        let normal = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(0.0, 1.0), 0.0];
        let hit_at = if g.next() % 4 == 0 { 99 } else { (g.next() % 25) as usize };
        let answers: Vec<(f32, RayHit)> = (0..25)
            .map(|k| {
                let h = if k == hit_at { [0.0, 1.0, g.f(0.0, 1.0)][g.next() as usize % 3] } else { -1.0 };
                (h, RayHit { point: [g.f(-9.0, 9.0); 4], normal: [g.f(-1.0, 1.0); 4], word: g.next() })
            })
            .collect();
        *script.borrow_mut() = answers.clone();
        segs.borrow_mut().clear();
        let m = &mut r.mem;
        put4(m, rider + 0x140, pos);
        put4(m, rider + 0x150, vel);
        put4(m, rider + 0x2a0, normal);
        for w in 0..0x15 {
            m.write_u32(pr + 4 * w, 0x5555_0000 + w);
        }
        r.cpu.set_gpr(4, pr as u64);
        r.call(0x123b10).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut q = answers.clone();
        let mut ours_segs = Vec::new();
        let mut ray = |a: V4, b: V4| {
            ours_segs.push((bits(a), bits(b), 0u32, 0u64));
            q.remove(0)
        };
        let p = predict_at_takeoff(pos, vel, normal, &mut ray);
        let ctx = format!("case {case}: pos {pos:?} vel {vel:?} hit at {hit_at}");
        assert_eq!(*segs.borrow(), ours_segs, "{ctx}: segments");
        let m = &mut r.mem;
        let theirs = (
            m.read_u32(pr + 8), get4(m, pr + 0x10), get4(m, pr + 0x20), get4(m, pr + 0x30),
            m.read_u32(pr + 0x40), m.read_u32(pr + 0x44), m.read_u32(pr + 0x48), m.read_u32(pr + 0x4c), m.read_u32(pr + 0x50),
        );
        let ours = (
            p.t.to_bits(), bits(p.point), bits(p.vel), bits(p.normal),
            p.word, p.t_land.to_bits(), p.t_top.to_bits(), p.x4c.to_bits(), p.x50.to_bits(),
        );
        assert_eq!(theirs, ours, "{ctx}");
        landed += (p.word != 0xffff_ffff) as u32;
        rising += (0.0 < vel[2]) as u32;
    }
    assert!(landed > 10_000 && rising > 5000, "landed {landed}, rising {rising}");
}

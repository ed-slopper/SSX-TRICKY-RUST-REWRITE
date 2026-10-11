//! The rider's collision body against the original's (board row E8d2): `CollBody_InitRider` 0x236050,
//! `CollBody_UpdateFromBones` 0x2361b0, `CollBody_Translate` 0x236448 and `CollBody_OverlapSpheres` 0x236a00 run in
//! the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) against
//! `tricky_game::collbody`, bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::collbody::{overlap, CollBody};

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
    fn v(&mut self, k: f32) -> [f32; 4] {
        [self.f(-k, k), self.f(-k, k), self.f(-k, k), if self.next() % 4 == 0 { self.f(-1.0, 1.0) } else { 1.0 }]
    }
}

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}

fn put(m: &mut r5900::Mem, at: u32, b: &CollBody) {
    put4(m, at + 0x10, b.center);
    m.write_f32(at + 0x20, b.radius);
    m.write_u32(at + 0x24, b.x24 as u32);
    m.write_u32(at + 0x28, b.count as u32);
    for (k, (c, r)) in b.spheres.iter().enumerate() {
        put4(m, at + 0x30 + 0x20 * k as u32, *c);
        m.write_f32(at + 0x40 + 0x20 * k as u32, *r);
    }
}

fn get(m: &mut r5900::Mem, at: u32) -> CollBody {
    let mut b = CollBody { center: get4(m, at + 0x10), radius: m.read_f32(at + 0x20), x24: m.read_u32(at + 0x24) as i32, count: m.read_u32(at + 0x28) as i32, spheres: [([0.0; 4], 0.0); 18] };
    for k in 0..18u32 {
        b.spheres[k as usize] = (get4(m, at + 0x30 + 0x20 * k), m.read_f32(at + 0x40 + 0x20 * k));
    }
    b
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2cc808); // EAMem_Set
    r.allow(0x102f50); // Vec4_Scale
    Some(r)
}

#[test]
fn body_built_and_moved_as_the_original() {
    let Some(mut r) = runner() else { return };
    let body = r.mem.alloc(0x300);
    let skel = r.mem.alloc(0x40 * 20);
    let mut g = Rng(0x1c0d_9a77_3e52_b801);
    for case in 0..5_000 {
        let bound = g.f(0.5, 3.0);
        r.cpu.set_f(12, bound);
        r.cpu.set_gpr(4, body as u64);
        r.call(0x236050).unwrap_or_else(|e| panic!("case {case} init: {e}"));
        let mut ours = get(&mut r.mem, body);
        ours.init_rider(bound);
        assert_eq!(format!("{:?}", get(&mut r.mem, body)), format!("{ours:?}"), "case {case}: init");
        // a skeleton: bones around a rider, the board's two ends apart
        let base = g.v(5000.0);
        let bones: Vec<[f32; 4]> = (0..20).map(|_| { let o = g.v(120.0); [base[0] + o[0], base[1] + o[1], base[2] + o[2], o[3]] }).collect();
        for (k, b) in bones.iter().enumerate() {
            put4(&mut r.mem, skel + 0x30 + 0x40 * k as u32, *b);
        }
        r.mem.write_u32(body, skel);
        ours.x24 = g.next() as i32;
        r.mem.write_u32(body + 0x24, ours.x24 as u32);
        r.cpu.set_gpr(4, body as u64);
        r.call(0x2361b0).unwrap_or_else(|e| panic!("case {case} bones: {e}"));
        ours.update_from_bones(&bones);
        assert_eq!(format!("{:?}", get(&mut r.mem, body)), format!("{ours:?}"), "case {case}: from bones");
        let v = g.v(300.0);
        let vp = r.mem.alloc(0x10);
        put4(&mut r.mem, vp, v);
        r.cpu.set_gpr(4, body as u64);
        r.cpu.set_gpr(5, vp as u64);
        r.call(0x236448).unwrap_or_else(|e| panic!("case {case} translate: {e}"));
        ours.translate(v);
        assert_eq!(format!("{:?}", get(&mut r.mem, body)), format!("{ours:?}"), "case {case}: translate");
    }
}

#[test]
fn overlaps_as_the_original() {
    let Some(mut r) = runner() else { return };
    let (pa, pb, out1, out2) = (r.mem.alloc(0x300), r.mem.alloc(0x300), r.mem.alloc(0x10), r.mem.alloc(0x10));
    let mut g = Rng(0x55aa_0f1e_7c33_2b49);
    let mut hits = 0;
    for case in 0..20_000 {
        let body = |g: &mut Rng, at: [f32; 4]| {
            let mut b = CollBody::new();
            b.init_rider(g.f(0.8, 2.5));
            b.center = at;
            b.count = if g.next() % 8 == 0 { (g.next() % 19) as i32 } else { 18 };
            for s in b.spheres.iter_mut() {
                let o = g.v(90.0);
                s.0 = [at[0] + o[0], at[1] + o[1], at[2] + o[2], o[3]];
                if g.next() % 5 == 0 {
                    s.1 = g.f(0.0, 40.0);
                }
            }
            b
        };
        let ca = g.v(1000.0);
        let reach = if g.next() % 2 == 0 { 120.0 } else { 400.0 };
        let off = g.v(reach);
        let a = body(&mut g, ca);
        let b = body(&mut g, [ca[0] + off[0], ca[1] + off[1], ca[2] + off[2], 1.0]);
        put(&mut r.mem, pa, &a);
        put(&mut r.mem, pb, &b);
        put4(&mut r.mem, out1, [7.0; 4]);
        put4(&mut r.mem, out2, [7.0; 4]);
        r.cpu.set_gpr(4, pa as u64);
        r.cpu.set_gpr(5, pb as u64);
        r.cpu.set_gpr(6, out1 as u64);
        r.cpu.set_gpr(7, out2 as u64);
        r.call(0x236a00).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = if r.cpu.gpr(2) != 0 { Some((get4(&mut r.mem, out1), get4(&mut r.mem, out2))) } else { None };
        let ours = overlap(&a, &b);
        assert_eq!(format!("{theirs:?}"), format!("{ours:?}"), "case {case}");
        hits += ours.is_some() as u32;
    }
    assert!(hits > 1000 && hits < 19_000, "overlaps: {hits}");
}

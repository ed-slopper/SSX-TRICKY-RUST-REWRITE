//! `tricky_game::campath` against `CamPath_SegmentPoint` 0x170610, `CamPath_SegmentAngles` 0x1708e8 and
//! `Camera_UpdateCutFade` 0x174b38 (with `CamScript_UpdateCutFade` 0x174c58) in the function runner (TRICKY_ELF;
//! skipped without it).

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::campath::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
    fn ang(&mut self) -> f32 {
        (self.next() % 20_000) as f32 / 1000.0 - 10.0
    }
}

fn key(g: &mut Rng) -> Key {
    Key { pos: [g.f(), g.f(), g.f()], ang: [g.ang(), g.ang(), g.ang()], w: g.f() }
}

fn put_key(r: &mut Runner, a: u32, k: &Key) {
    for i in 0..3 {
        r.mem.write_f32(a + 4 * i, k.pos[i as usize]);
        r.mem.write_f32(a + 0xc + 4 * i, k.ang[i as usize]);
    }
    r.mem.write_f32(a + 0x18, k.w);
}

#[test]
fn camera_path_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x137140);
    r.allow(0x1741d0);
    r.allow(0x1704b0);
    r.allow(0x170610);
    r.allow(0x174b38);
    let out = r.mem.alloc(0x100);
    let path = r.mem.alloc(0x40);
    let owner = r.mem.alloc(0x200);
    let keys = [r.mem.alloc(0x20), r.mem.alloc(0x20), r.mem.alloc(0x20), r.mem.alloc(0x20)];
    let rr = r.mem.alloc(0x10);
    r.mem.write_u32(path + 0x24, owner);
    let mut g = Rng(0x1706_10a9_0c3d_5e11);
    let (mut firsts, mut lasts, mut mids) = (0, 0, 0);
    for case in 0..30_000 {
        let count = 2 + (g.next() % 8) as i32;
        let idx = (g.next() % (count as u32 + 1)) as i32;
        let at = PathAt { idx, count };
        let ks = [key(&mut g), key(&mut g), key(&mut g), key(&mut g)];
        for (a, k) in keys.iter().zip(ks.iter()) {
            put_key(&mut r, *a, k);
        }
        r.mem.write_u32(path + 4, idx as u32);
        r.mem.write_u32(owner + 0x12c, count as u32);
        let t = [0.0f32, 0.25, 0.5, 1.0, (g.next() % 1000) as f32 / 1000.0][(g.next() % 5) as usize];
        if idx == 0 {
            firsts += 1;
        } else if idx >= count - 2 {
            lasts += 1;
        } else {
            mids += 1;
        }
        // SegmentPoint
        for k in 0..3 {
            r.mem.write_u32(out + 4 * k, 0xdead_beef);
        }
        r.mem.write_u32(rr, 0x7777_7777);
        r.mem.write_u32(rr + 4, 0x7777_7777);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, path as u64);
        r.cpu.set_gpr(6, keys[0] as u64);
        r.cpu.set_gpr(7, keys[1] as u64);
        r.cpu.set_gpr(8, keys[2] as u64);
        r.cpu.set_gpr(9, keys[3] as u64);
        r.cpu.set_gpr(10, rr as u64);
        r.cpu.set_gpr(11, (rr + 4) as u64);
        r.cpu.set_f(12, t);
        r.call(0x170610).unwrap();
        let (p, ratios) = segment_point(at, t, &ks[0], &ks[1], &ks[2], &ks[3]);
        for k in 0..3 {
            assert_eq!(r.mem.read_u32(out + 4 * k), p[k as usize].to_bits(), "case {case} point {k}");
        }
        let want0 = ratios.r0.map(f32::to_bits).unwrap_or(0x7777_7777);
        let want1 = ratios.r1.map(f32::to_bits).unwrap_or(0x7777_7777);
        assert_eq!(r.mem.read_u32(rr), want0, "case {case} r0");
        assert_eq!(r.mem.read_u32(rr + 4), want1, "case {case} r1");
        // SegmentAngles
        for k in 0..0x16 {
            r.mem.write_u32(out + 4 * k, 0xdead_beef);
        }
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, path as u64);
        r.cpu.set_gpr(6, keys[0] as u64);
        r.cpu.set_gpr(7, keys[1] as u64);
        r.cpu.set_gpr(8, keys[2] as u64);
        r.cpu.set_gpr(9, keys[3] as u64);
        r.cpu.set_f(12, t);
        r.call(0x1708e8).unwrap();
        let a = segment_angles(at, t, &ks[0], &ks[1], &ks[2], &ks[3]);
        let want = [a.pos[0], a.pos[1], a.pos[2], a.ang[0], a.ang[1], a.ang[2], a.w, 100.0];
        for (k, w) in want.iter().enumerate() {
            assert_eq!(r.mem.read_u32(out + 4 * k as u32), w.to_bits(), "case {case} angles word {k} at {at:?} t {t} keys {ks:?}");
        }
        for k in [8u32, 12, 21] {
            assert_eq!(r.mem.read_u32(out + 4 * k), 0, "case {case} zero word {k}");
        }
    }
    eprintln!("first {firsts} last {lasts} mid {mids}");
    // the cut fade
    let this = r.mem.alloc(0x500);
    let other = r.mem.alloc(0x20);
    let view = r.mem.alloc(0x200);
    r.mem.write_u32(other + 4, view);
    for case in 0..20_000 {
        let mut f = CutFade {
            hold: [0, 1, 2, -1][(g.next() % 4) as usize],
            active: (g.next() % 2) as u32,
            t: [0.0f32, 0.1, 0.3, 0.59, 0.5999, 0.6, 0.7][(g.next() % 7) as usize],
        };
        r.mem.write_u32(0x3365f4, f.hold as u32);
        r.mem.write_u32(0x3365f8, f.active);
        r.mem.write_f32(0x3365fc, f.t);
        r.mem.write_u32(this + 0x44c, 0x5555_5555);
        r.cpu.set_gpr(4, this as u64);
        if case % 2 == 0 {
            r.call(0x174b38).unwrap();
        } else {
            let v = g.f();
            r.mem.write_f32(view + 0x168, v);
            r.mem.write_u32(this + 0x448, 0);
            r.cpu.set_gpr(5, other as u64);
            r.call(0x174c58).unwrap();
            assert_eq!(r.mem.read_u32(this + 0x448), v.to_bits(), "case {case} copied");
        }
        let level = update_cut_fade(&mut f);
        assert_eq!(r.mem.read_u32(this + 0x44c), level.to_bits(), "case {case} level");
        assert_eq!(r.mem.read_u32(0x3365f4), f.hold as u32, "case {case} hold");
        assert_eq!(r.mem.read_u32(0x3365f8), f.active, "case {case} active");
        assert_eq!(r.mem.read_u32(0x3365fc), f.t.to_bits(), "case {case} t");
    }
}

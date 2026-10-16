//! `teleport::teleport_target` and `camscript::update_cast_clip` (board row G20c3b3) against
//! `Boarder_TeleportToInstance` 0x11a098 and `RaceScene_UpdateCastClip` 0x16daa8 in the function runner (TRICKY_ELF;
//! skipped without it): random object matrices and rider slots, random cast structs; the placement call (heading,
//! place, velocity, flag) and the others logged on both sides, floats bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::camscript::{update_cast_clip, update_cast_clips, CastClipEnv, RiderEnv};
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;
use tricky_game::teleport::teleport_target;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self) -> f32 {
        [0.0, 1.0, -1.0, 0.5, -0.25, 0.70710677, -0.70710677, 0.8660254, 1234.5, -5678.25, 100.0, 3.0, -0.0][self.below(13) as usize]
    }
}

type L = Rc<RefCell<Vec<(u32, Vec<u32>)>>>;

struct Cast {
    log: L,
    rider: u32,
    progress: f32,
}
impl RiderEnv for Cast {
    fn rider(&mut self, i: u32) -> u32 {
        self.log.borrow_mut().push((0x1814a8, vec![i]));
        self.rider
    }
    fn anim_progress(&mut self, rider: u32) -> f32 {
        self.log.borrow_mut().push((0x15fe70, vec![rider.wrapping_add(0x46c0), 0]));
        self.progress
    }
}
impl CastClipEnv for Cast {
    fn place_at(&mut self, rider: u32, heading: f32, place: [u32; 4]) {
        let mut v = vec![rider, heading.to_bits(), 0];
        v.extend(place);
        v.extend([0, 0, 0, 0, 1]);
        self.log.borrow_mut().push((0x11cae8, v));
    }
    fn play_clip(&mut self, rider: u32, clip: u32) {
        self.log.borrow_mut().push((0x169be8, vec![0x32edc0, rider, clip]));
    }
}

#[test]
fn teleport_and_cast_clip_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(0x1000);
    let (rider, mat, app, race, cs, list) = (base, base + 0x500, base + 0x600, base + 0x700, base + 0x800, base + 0x900);
    let l: L = Rc::new(RefCell::new(vec![]));
    let l2 = l.clone();
    r.stub(0x11cae8, move |c, m| {
        let (p, v) = (c.gpr(5) as u32, c.gpr(6) as u32);
        let mut w = vec![c.gpr(4) as u32, c.f(12).to_bits(), c.f(13).to_bits()];
        w.extend((0..4).map(|k| m.read_u32(p + 4 * k)));
        w.extend((0..4).map(|k| m.read_u32(v + 4 * k)));
        w.push(c.gpr(7) as u32);
        l2.borrow_mut().push((0x11cae8, w));
    });
    let l2 = l.clone();
    r.stub(0x177db0, move |c, _| l2.borrow_mut().push((0x177db0, vec![c.gpr(4) as u32])));
    let l2 = l.clone();
    r.stub(0x169be8, move |c, _| l2.borrow_mut().push((0x169be8, vec![c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32])));
    let answers = Rc::new(RefCell::new((0u32, 0f32)));
    let (l2, a2) = (l.clone(), answers.clone());
    r.stub(0x1814a8, move |c, _| {
        l2.borrow_mut().push((0x1814a8, vec![c.gpr(5) as u32]));
        c.set_gpr(2, a2.borrow().0 as u64);
    });
    let (l2, a2) = (l.clone(), answers.clone());
    r.stub(0x15fe70, move |c, _| {
        l2.borrow_mut().push((0x15fe70, vec![c.gpr(4) as u32, c.gpr(5) as u32]));
        c.set_f(0, a2.borrow().1);
    });
    r.allow(0x2510f8);
    let mut g = Rng(0x11a0_98ee_8e8e_0001);
    for case in 0..60_000u32 {
        // the teleport
        let mut m = [[0f32; 4]; 4];
        for row in m.iter_mut() {
            for v in row.iter_mut() {
                *v = g.f();
            }
        }
        for (i, row) in m.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                r.mem.write_f32(mat + 16 * i as u32 + 4 * k as u32, *v);
            }
        }
        let slot = g.below(7);
        let seat = [u32::MAX, 0, 1, 3][g.below(4) as usize];
        r.mem.write_u32(rider + 0x460, slot);
        r.mem.write_u32(rider + 0x38, seat);
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x730, race);
        for k in 0..4 {
            r.mem.write_u32(race + 0xa0 + 0x80 * k, 0x5000 + k);
        }
        l.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, mat as u64);
        r.call(0x11a098).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        let (place, heading) = teleport_target(&m, slot);
        let mut want = vec![rider, heading.to_bits(), 0];
        want.extend(place.iter().map(|f| f.to_bits()));
        want.extend([0, 0, 0, 0, 0]);
        let mut mine = vec![(0x11cae8, want)];
        if (seat as i32) >= 0 {
            mine.push((0x177db0, vec![0x5000 + seat]));
        }
        assert_eq!(theirs, mine, "case {case}: teleport (slot {slot}, matrix {m:?})");

        // the cast clip
        let n = 1 + g.below(4);
        for k in 0..6 {
            r.mem.write_u32(list + 4 * k, if k < n { g.below(50) } else { [u32::MAX, 0x8000_0000][g.below(2) as usize] });
        }
        r.mem.write_u32(cs + 4, list);
        r.mem.write_u32(cs + 8, g.below(n));
        r.mem.write_u32(cs + 0xc, g.below(8));
        r.mem.write_u32(cs + 0x14, g.below(2));
        for k in 0..4 {
            r.mem.write_f32(cs + 0x20 + 4 * k, g.f());
        }
        r.mem.write_f32(cs + 0x30, g.f());
        *answers.borrow_mut() = ([0u32, 0x10_0000, 0x20_0000][g.below(3) as usize], [0.0f32, 0.5, 0.99999994, 1.0, 1.5, -1.0][g.below(6) as usize]);
        let mut pm = Mem::default();
        let mut b = vec![0u8; 0x200];
        r.mem.read(cs, &mut b).unwrap();
        pm.regions.push((cs, b));
        l.borrow_mut().clear();
        r.mem.write_u32(GAPP, app);
        r.cpu.set_gpr(4, cs as u64);
        r.call(0x16daa8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        let (rd, pr) = *answers.borrow();
        let mut e = Cast { log: l.clone(), rider: rd, progress: pr };
        update_cast_clip(&mut pm, &mut e, cs);
        let mine = std::mem::take(&mut *l.borrow_mut());
        assert_eq!(theirs, mine, "case {case}: cast clip");
        assert_eq!(r.mem.read_u32(cs + 8), pm.r32(cs + 8), "case {case}: clip index");
    }
    // the eight structs of a camera script
    r.allow(0x16daa8);
    let cam = r.mem.alloc(0x500);
    for case in 0..20_000u32 {
        r.mem.write_u32(cam + 0x210, g.below(3));
        for k in 0..8 {
            let s = cam + 0x220 + 0x40 * k;
            r.mem.write_u32(s + 4, list);
            r.mem.write_u32(s + 8, g.below(2));
            r.mem.write_u32(s + 0xc, k);
            r.mem.write_u32(s + 0x10, g.below(2));
            r.mem.write_u32(s + 0x14, g.below(2));
            r.mem.write_f32(s + 0x30, g.f());
        }
        for k in 0..6 {
            r.mem.write_u32(list + 4 * k, if k < 3 { g.below(50) } else { u32::MAX });
        }
        *answers.borrow_mut() = (0x10_0000, [0.5f32, 1.0][g.below(2) as usize]);
        let mut pm = Mem::default();
        for at in [cam, list] {
            let mut b = vec![0u8; 0x500];
            r.mem.read(at, &mut b).unwrap();
            pm.regions.push((at, b));
        }
        l.borrow_mut().clear();
        r.cpu.set_gpr(4, cam as u64);
        r.call(0x177020).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        let (rd, pr) = *answers.borrow();
        update_cast_clips(&mut pm, &mut Cast { log: l.clone(), rider: rd, progress: pr }, cam);
        assert_eq!(theirs, std::mem::take(&mut *l.borrow_mut()), "case {case}: cast clips");
    }
}

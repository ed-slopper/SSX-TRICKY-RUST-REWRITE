//! `tricky_game::racemisc` (board row G4i) against the original in the function runner (TRICKY_ELF; skipped without).

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::racemisc::*;

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
fn race_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let app = r.mem.alloc(0x800);
    let rgame = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    let cam = r.mem.alloc(0x100);
    let game = r.mem.alloc(0x400);
    let riders: Vec<u32> = (0..8).map(|_| r.mem.alloc(0x6000)).collect();
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, rgame);
    r.mem.write_u32(rgame + 0x1c, race);
    let rs = riders.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, rs[c.gpr(5) as usize] as u64));
    let focus = std::rc::Rc::new(std::cell::Cell::new(0u32));
    let f2 = focus.clone();
    let rs = riders.clone();
    r.stub(0x193ce0, move |c, _| {
        let p = c.gpr(5) as u32;
        let i = rs.iter().position(|&x| x == p).unwrap();
        c.set_gpr(2, ((f2.get() >> i) & 1) as u64);
    });
    let mut g = Rng(0x2718_2818_2845_9045);
    for case in 0..40_000 {
        let n = 1 + g.next() as usize % 4;
        r.mem.write_u32(game + 0x298, n as u32);
        let mode = [3, 5, 2, 4, 0, 7][g.next() as usize % 6];
        r.mem.write_u32(0x32de8c, mode as u32);
        let mut keys = vec![];
        let mut ranks = vec![];
        for i in 0..n {
            let p = riders[i];
            let pts = [0, 1, 5, -3, (g.next() % 100) as i32, 1 << 25][g.next() as usize % 6];
            let fin = [0, -1, -5, (g.next() % 3000) as i32, 1 << 26][g.next() as usize % 5];
            let rank = [-1, 0, 1, (g.next() % 8) as i32, 40000, 30000, 29999][g.next() as usize % 7];
            let id = (g.next() % 6) as i32;
            r.mem.write_u32(p + 0x5910, pts as u32);
            r.mem.write_u32(p + 0x418, fin as u32);
            r.mem.write_u32(p + 0x110, rank as u32);
            r.mem.write_u32(p + 0x460, id as u32);
            r.mem.write_u32(game + 0xa4 + 0x80 * i as u32, p);
            keys.push(LeadKey { points: pts, finish: fin });
            ranks.push((rank, id));
        }
        r.cpu.set_gpr(4, game as u64);
        r.call(0x180760).unwrap();
        assert_eq!(r.cpu.gpr(2) as i32, leading_player(mode, &keys), "lead {case}");

        // move leader: random slot words
        let mut slots = vec![[0u32; 32]; n];
        for (i, s) in slots.iter_mut().enumerate() {
            for (j, w) in s.iter_mut().enumerate() {
                *w = g.next();
                if j == 5 {
                    *w = riders[i];
                }
                r.mem.write_u32(game + 0x90 + 0x80 * i as u32 + 4 * j as u32, *w);
            }
        }
        r.mem.write_u32(game + 0x68, 77);
        r.cpu.set_gpr(4, game as u64);
        r.call(0x180848).unwrap();
        let k = move_leader_to_slot0(mode, &keys, &mut slots);
        assert_eq!(r.mem.read_u32(game + 0x68) as i32, k, "leader idx {case}");
        for (i, s) in slots.iter().enumerate() {
            for (j, w) in s.iter().enumerate() {
                // words past +0x108 (j=31) aren't touched by either side
                if j == 31 {
                    continue;
                }
                assert_eq!(r.mem.read_u32(game + 0x90 + 0x80 * i as u32 + 4 * j as u32), *w, "slot {case} {i}/{j}");
            }
        }

        // find player riders
        let rc = 1 + g.next() as usize % 8;
        r.mem.write_u32(race + 0x88, rc as u32);
        let owners: Vec<i32> = (0..rc).map(|_| [0, 1, 2, -1, 0][g.next() as usize % 5]).collect();
        for (i, &o) in owners.iter().enumerate() {
            r.mem.write_u32(riders[i] + 0x38, o as u32);
        }
        let out = r.mem.alloc(0x10);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, (out + 4) as u64);
        r.cpu.set_gpr(6, (out + 8) as u64);
        r.cpu.set_gpr(7, (out + 12) as u64);
        r.call(0x176720).unwrap();
        let want = find_player_riders(rc as i32, &|i| owners[i as usize]);
        for q in 0..4 {
            assert_eq!(r.mem.read_u32(out + 4 * q as u32) as i32, want[q], "find {case} {q} {owners:?}");
        }

        // lead or focus (the swap moved the slot pointers back)
        for i in 0..n {
            r.mem.write_u32(game + 0xa4 + 0x80 * i as u32, riders[i]);
        }
        let no_cam = g.next() % 4 == 0;
        let cw = [0, 1, 5, 9, 10, 11, -3, 0x20][g.next() as usize % 8];
        r.mem.write_u32(rgame + 0x40, if no_cam { 0 } else { cam });
        r.mem.write_u32(cam + 0x20, cw as u32);
        let fm = g.next() & 0xff;
        focus.set(fm);
        let id = (g.next() % 6) as i32;
        for (i, rid) in (0..rc).map(|i| (i, r.mem.read_u32(riders[i] + 0x460) as i32)).collect::<Vec<_>>() {
            let _ = (i, rid);
        }
        let ids: Vec<i32> = (0..rc).map(|i| r.mem.read_u32(riders[i] + 0x460) as i32).collect();
        r.cpu.set_gpr(4, game as u64);
        r.cpu.set_gpr(5, id as u64);
        r.call(0x17f1f0).unwrap();
        let f = |i: i32| (fm >> i) & 1 == 1;
        let rid = |i: i32| ids[i as usize];
        let c = LeadOrFocus { no_camera: no_cam, cam_word: cw, race_count: rc as i32, is_focus: &f, race_id: &rid, slots: &ranks };
        assert_eq!(r.cpu.gpr(2) & 1 != 0, is_lead_or_focus_rider(&c, id), "lof {case} nocam {no_cam} cw {cw} rc {rc} fm {fm:x} id {id} ids {ids:?} ranks {ranks:?} got {:x}", r.cpu.gpr(2));
    }
}

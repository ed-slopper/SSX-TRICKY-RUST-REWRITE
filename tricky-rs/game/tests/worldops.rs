//! The world ops (board row E9d) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `TriggerOp_SpawnEffect` for every sub-type, controller and rider count; `Boarder_TeleportToInstance` on random
//! object matrices for every rider slot; `RailMan_SetRail` over long random sequences, comparing the manager's 128
//! slots and the splines' live bits.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::worldops::{collide_room, spawn_effect, teleport, Effect, RailMan};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

#[test]
fn spawn_effect_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let made: Rc<RefCell<Vec<Effect>>> = Rc::new(RefCell::new(vec![]));
    r.allow(0x148900);
    let pool = r.mem.alloc(0x400);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    r.stub(0x2cc808, |_, _| {});
    for (a, e) in [(0x147cf8u32, Effect::Emitter), (0x1fa108, Effect::SplineModel), (0x148560, Effect::CollideEmitter)] {
        let m = made.clone();
        r.stub(a, move |_, _| m.borrow_mut().push(e));
    }
    let thread = r.mem.alloc(0x100);
    let object = r.mem.alloc(0x100);
    let ctrl = r.mem.alloc(0x40);
    let rider = r.mem.alloc(0x500);
    let op = r.mem.alloc(0x20);
    let mut g = Rng(0x5eed_e77e_c700_0001);
    for case in 0..5000 {
        let has_obj = g.next() % 4 != 0;
        let ctype = g.pick(&[None, Some(5u32), Some(2), Some(7)]);
        r.mem.write_u32(thread + 0xe4, if has_obj { object } else { 0 });
        r.mem.write_u32(object + 0xe4, if ctype.is_some() { ctrl } else { 0 });
        r.mem.write_u32(ctrl + 0x14, ctype.unwrap_or(0));
        r.mem.write_u32(thread + 0xe8, rider);
        let human = g.next() % 2 == 1;
        let slot = g.next() % 6;
        let count = g.pick(&[0i32, 1, 2, 3, 5, -1]);
        r.mem.write_u32(rider + 0x41c, human as u32);
        r.mem.write_u32(rider + 0x460, slot);
        r.mem.write_u32(0x31d498 + 4 * slot, count as u32);
        let p2 = g.pick(&[0i32, 1, 2, 3, -1]);
        let p3 = g.pick(&[-1i32, 0, 7]);
        r.mem.write_u32(op + 8, p2 as u32);
        r.mem.write_u32(op + 12, p3 as u32);
        made.borrow_mut().clear();
        r.cpu.set_gpr(4, thread as u64);
        r.cpu.set_gpr(5, op as u64);
        r.call(0x13d130).unwrap();
        let ctl = if has_obj { ctype } else { None };
        let want = spawn_effect(ctl, p2, p3, collide_room(human, count));
        assert_eq!(made.borrow().first().copied(), want, "case {case}: object {has_obj}, controller {ctype:?}, p2 {p2}, p3 {p3}");
    }
}

#[test]
fn teleport_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2510f8);
    let placed: Rc<RefCell<Option<(u32, [u32; 4], [u32; 4])>>> = Rc::new(RefCell::new(None));
    let cut: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(vec![]));
    let p = placed.clone();
    r.stub(0x11cae8, move |c, m| {
        assert_eq!(c.gpr(7), 0);
        let v = |m: &mut r5900::Mem, a: u32| [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k));
        *p.borrow_mut() = Some((c.f(12).to_bits(), v(m, c.gpr(5) as u32), v(m, c.gpr(6) as u32)));
    });
    let k = cut.clone();
    r.stub(0x177db0, move |c, _| k.borrow_mut().push(c.gpr(4) as u32));
    let gapp = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x400);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, race);
    let rider = r.mem.alloc(0x500);
    let mat = r.mem.alloc(0x40);
    let mut g = Rng(0x7e1e_9077_0bad_c0de);
    for case in 0..20000 {
        let f = |g: &mut Rng| g.pick(&[0.0f32, 1.0, -1.0, 0.6, -0.8, 0.25, 1500.0, -3000.0]);
        let mut rows = [[0.0f32; 4]; 4];
        for row in rows.iter_mut() {
            for v in row.iter_mut() {
                *v = f(&mut g);
            }
        }
        for (i, row) in rows.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                r.mem.write_f32(mat + 16 * i as u32 + 4 * k as u32, *v);
            }
        }
        let slot = g.next() % 7;
        let cam = g.pick(&[-1i32, 0, 1]);
        r.mem.write_u32(rider + 0x460, slot);
        r.mem.write_u32(rider + 0x38, cam as u32);
        if cam >= 0 {
            r.mem.write_u32(race + 0xa0 + 0x80 * cam as u32, 0x9000 + cam as u32);
        }
        *placed.borrow_mut() = None;
        cut.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, mat as u64);
        r.call(0x11a098).unwrap();
        let (yaw, pos) = teleport(rows, slot);
        assert_eq!(*placed.borrow(), Some((yaw.to_bits(), pos.map(f32::to_bits), [0; 4])), "case {case} slot {slot} {rows:?}");
        let want: Vec<u32> = if cam >= 0 { vec![0x9000 + cam as u32] } else { vec![] };
        assert_eq!(*cut.borrow(), want, "case {case}");
    }
}

#[test]
fn rail_switches_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x148ed8, 0x2609b0] {
        r.allow(a);
    }
    let segs = r.mem.alloc(0x40 * 300);
    r.stub(0x254b20, move |c, _| {
        assert_eq!(c.gpr(4) as u32, 0x346488);
        let i = c.gpr(5) as u16 as i16 as i32;
        c.set_gpr(2, (segs + 0x40 * (i + 20) as u32) as u64);
    });
    let man = r.mem.alloc(0x228);
    for i in 0..128 {
        r.mem.write_u32(man + 0x28 + 4 * i, 0xffff);
    }
    r.mem.write_u32(0x31d4b0, man);
    let mut mine = RailMan::default();
    let mut flags: HashMap<i16, u32> = HashMap::new();
    let mut g = Rng(0x0a11_5e75_0000_7777);
    for step in 0..20000 {
        // many splines, so the 128 slots fill
        let spline = (g.next() % 160) as i16 - 1;
        let state = g.pick(&[0i16, 1, 2]);
        let seg = segs + 0x40 * (spline as i32 + 20) as u32;
        let before = r.mem.read_u32(seg + 0x18);
        r.cpu.set_gpr(4, spline as i64 as u64);
        r.cpu.set_gpr(5, state as i64 as u64);
        r.call(0x149030).unwrap();
        let slot = mine.set(spline, state);
        let mut want = before;
        if let Some(i) = slot {
            want = mine.apply(i, before);
            flags.insert(spline, want);
        }
        assert_eq!(r.mem.read_u32(seg + 0x18), want, "step {step} spline {spline}");
        for i in 0..128u32 {
            let w = r.mem.read_u32(man + 0x28 + 4 * i);
            assert_eq!(((w & 0xffff) as u16 as i16, (w >> 16) as u16 as i16), mine.0[i as usize], "step {step} slot {i}");
        }
    }
}

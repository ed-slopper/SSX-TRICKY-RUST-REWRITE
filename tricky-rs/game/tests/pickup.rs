//! The pickups (board row E9c) against the game: `Pickup_Points`, `Pickup_Multiplier`, `Pickup_Meter`,
//! `Pickup_Time`, `Boarder_AddSpeedBoostTime`, `Boarder_AddSpinBoostTime` and the score functions under them run in
//! the function runner (TRICKY_ELF; skipped without it) for random riders, modes and amounts, against
//! `tricky_game::pickup`.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::pickup::{self, PickupCall, PickupRider};
use tricky_game::score::{Score, ScoreCall};

const SCORE: u32 = 0x5820;

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

fn put(m: &mut r5900::Mem, rider: u32, r: &PickupRider, s: &Score) {
    m.write_f32(rider + 0x134, r.speed_boost);
    m.write_f32(rider + 0x138, r.spin_boost);
    m.write_u32(rider + 0x13c, r.spin_boost_on);
    m.write_u32(rider + 0x41c, r.x41c as u32);
    m.write_u32(rider + 0x424, r.x424 as u32);
    m.write_f32(rider + SCORE + 0x28, s.mult);
    m.write_u32(rider + SCORE + 0x94, s.pickup_points as u32);
    for (k, v) in s.pickups.iter().enumerate() {
        m.write_u32(rider + SCORE + 0x98 + 4 * k as u32, *v as u32);
    }
}

fn get(m: &mut r5900::Mem, rider: u32) -> (PickupRider, f32, i32, [i32; 9]) {
    let r = PickupRider {
        speed_boost: m.read_f32(rider + 0x134),
        spin_boost: m.read_f32(rider + 0x138),
        spin_boost_on: m.read_u32(rider + 0x13c),
        x41c: m.read_u32(rider + 0x41c) as i32,
        x424: m.read_u32(rider + 0x424) as i32,
    };
    let mut p = [0; 9];
    for (k, v) in p.iter_mut().enumerate() {
        *v = m.read_u32(rider + SCORE + 0x98 + 4 * k as u32) as i32;
    }
    (r, m.read_f32(rider + SCORE + 0x28), m.read_u32(rider + SCORE + 0x94) as i32, p)
}

#[test]
fn pickups_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<PickupCall>>> = Rc::new(RefCell::new(vec![]));
    let l = log.clone();
    r.stub(0x157330, move |c, _| {
        l.borrow_mut().push(PickupCall::Score(ScoreCall::Message(c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as i32, c.gpr(7) as i32)));
    });
    let l = log.clone();
    r.stub(0x11b018, move |c, _| l.borrow_mut().push(PickupCall::AddMeter(c.f(12))));
    let l = log.clone();
    r.stub(0x219d70, move |c, _| l.borrow_mut().push(PickupCall::SfxMultiplier(c.f(12))));
    let l = log.clone();
    r.stub(0x219fc0, move |c, _| l.borrow_mut().push(PickupCall::SfxTime(c.f(12))));
    for a in [0x155b10, 0x155bb8, 0x155cd0] {
        r.allow(a);
    }
    let rider = r.mem.alloc(0x6000);
    let gapp = r.mem.alloc(0x800);
    let handler = r.mem.alloc(0x40);
    let clock = r.mem.alloc(0x40);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, handler);
    r.mem.write_u32(handler + 0x1c, clock);
    let mut g = Rng(0x2545_f491_4f6c_dd1d);
    for i in 0..30000 {
        let mode = g.next() % 8;
        let mut pr = PickupRider {
            speed_boost: g.pick(&[0.0, 1.0, 2.5, 5.0, 10.0, -1.0]),
            spin_boost: g.pick(&[0.0, 1.0, 2.5, 5.0, 10.0, -1.0]),
            spin_boost_on: g.next() % 2,
            x41c: g.pick(&[0, 1, 2, -1]),
            x424: g.pick(&[0, 1, 2, 3, 4]),
        };
        let mut s = Score { mult: g.pick(&[1.0, 1.5, 2.0, 3.0, 5.0, 6.0]), pickup_points: (g.next() % 100000) as i32, ..Score::default() };
        for v in s.pickups.iter_mut() {
            *v = (g.next() % 20) as i32;
        }
        put(&mut r.mem, rider, &pr, &s);
        let limit = g.pick(&[0.0f32, 90.0, 120.0, 135.0, 47.25]);
        r.mem.write_f32(clock + 0x14, limit);
        r.mem.write_u32(0x32de8c, mode);
        log.borrow_mut().clear();
        let op = i % 6;
        let (want, clock_want) = match op {
            0 => {
                let pts = g.pick(&[500, 2000, 5000, 100, 1000, 0, -500, 250]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_gpr(5, pts as i64 as u64);
                r.call(0x11e608).unwrap();
                (pickup::points(mode, &mut s, pts), limit)
            }
            1 => {
                let m = g.pick(&[2.0f32, 3.0, 5.0, 4.0, 1.0, 2.5, 6.0, 1.5]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_f(12, m);
                r.call(0x11e660).unwrap();
                (pickup::multiplier(mode, &pr, &mut s, m), limit)
            }
            2 => {
                let a = g.pick(&[0.1f32, 0.25, 0.5, 1.0, -0.2]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_f(12, a);
                r.call(0x11e958).unwrap();
                (pickup::meter(mode, a), limit)
            }
            3 => {
                let t = g.pick(&[1.0f32, 5.0, 10.0, 2.0, 7.5, 0.0, 10.9]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_f(12, t);
                r.call(0x11e8a0).unwrap();
                let calls = pickup::time(mode, &mut s, t);
                let c = calls.iter().fold(limit, |c, k| if let PickupCall::ClockAdd(v) = k { c + v } else { c });
                (calls.into_iter().filter(|k| !matches!(k, PickupCall::ClockAdd(_))).collect(), c)
            }
            4 => {
                let t = g.pick(&[0.0f32, 3.0, 5.0, 10.0, -2.0]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_f(12, t);
                r.call(0x11e910).unwrap();
                pickup::speed_boost(&mut pr, t);
                (vec![], limit)
            }
            _ => {
                let t = g.pick(&[0.0f32, 3.0, 5.0, 10.0, -2.0]);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_f(12, t);
                r.call(0x11e930).unwrap();
                pickup::spin_boost(&mut pr, t);
                (vec![], limit)
            }
        };
        assert_eq!(*log.borrow(), want, "op {op}, mode {mode}, {pr:?}");
        assert_eq!(get(&mut r.mem, rider), (pr, s.mult, s.pickup_points, s.pickups), "op {op}, mode {mode}");
        assert_eq!(r.mem.read_f32(clock + 0x14).to_bits(), clock_want.to_bits(), "op {op}, mode {mode}");
    }
}

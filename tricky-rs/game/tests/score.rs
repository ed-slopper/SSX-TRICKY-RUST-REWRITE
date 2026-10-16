//! The score object's arithmetic against the original's, bit for bit (board row G2a): `Score_CalcPoints`,
//! `Score_CalcBasePoints`, `Score_GetMultiplier`, `Score_RoundSpinTo180`, `Score_UpdateSpin`, `Score_UpdateFlip`,
//! `Score_StartGrab` and `Score_Tick` run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped
//! without it) and `tricky_game::score` on the same score objects.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::score::{self, Score};

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
    fn i(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x157198); // Score_RoundSpinTo180, called by UpdateSpin/UpdateFlip
    Some(r)
}

fn put(r: &mut Runner, at: u32, s: &Score) {
    let m = &mut r.mem;
    m.write_u32(at + 0x0c, s.x0c);
    m.write_f32(at + 0x24, s.units);
    m.write_f32(at + 0x28, s.mult);
    m.write_f32(at + 0x2c, s.grab_rate);
    for (o, v) in [(0x30, s.spin), (0x34, s.flip), (0x38, s.grab_ticks), (0x3c, s.rail_ticks), (0x40, s.x40), (0x44, s.x44), (0x48, s.spin_scored), (0x4c, s.flip_scored)] {
        m.write_u32(at + o, v as u32);
    }
    for (k, g) in s.grabs.iter().enumerate() {
        m.write_u32(at + 0x60 + 4 * k as u32, *g);
    }
    m.write_u32(at + 0x6c, s.trick_id[0]);
    m.write_u32(at + 0x70, s.trick_id[1]);
    m.write_u32(at + 0xf0, s.xf0);
    for (k, e) in s.timed.iter().enumerate() {
        let b = at + 0x15c + 20 * k as u32;
        m.write_u32(b, e.kind);
        m.write_u32(b + 4, e.value as u32);
        m.write_f32(b + 8, e.life);
        m.write_f32(b + 12, e.age);
        m.write_u32(b + 16, e.extra as u32);
    }
}

fn get(r: &mut Runner, at: u32) -> Score {
    let m = &mut r.mem;
    let mut s = Score { x0c: m.read_u32(at + 0x0c), units: m.read_f32(at + 0x24), mult: m.read_f32(at + 0x28), grab_rate: m.read_f32(at + 0x2c), ..Score::default() };
    let i = |m: &mut r5900::Mem, o: u32| m.read_u32(at + o) as i32;
    s.spin = i(m, 0x30);
    s.flip = i(m, 0x34);
    s.grab_ticks = i(m, 0x38);
    s.rail_ticks = i(m, 0x3c);
    s.x40 = i(m, 0x40);
    s.x44 = i(m, 0x44);
    s.spin_scored = i(m, 0x48);
    s.flip_scored = i(m, 0x4c);
    for k in 0..3 {
        s.grabs[k] = m.read_u32(at + 0x60 + 4 * k as u32);
    }
    s.trick_id = [m.read_u32(at + 0x6c), m.read_u32(at + 0x70)];
    s.xf0 = m.read_u32(at + 0xf0);
    for k in 0..12 {
        let b = at + 0x15c + 20 * k as u32;
        s.timed[k] = tricky_game::score::Timed { kind: m.read_u32(b), value: m.read_u32(b + 4) as i32, life: m.read_f32(b + 8), age: m.read_f32(b + 12), extra: m.read_u32(b + 16) as i32 };
    }
    s
}

fn random_score(g: &mut Rng) -> Score {
    let mut s = Score {
        x0c: g.next() % 2,
        units: g.f(-0.5, 3.0),
        mult: [1.0, 1.3, 2.0, 3.0, 5.0][(g.next() % 5) as usize],
        grab_rate: g.f(0.0, 20.0),
        spin: g.i(-1500, 1500),
        flip: g.i(-1500, 1500),
        grab_ticks: g.i(-1, 300),
        rail_ticks: g.i(-1, 300),
        x40: g.i(-1, 50),
        x44: g.i(-1, 50),
        spin_scored: g.i(0, 1080),
        flip_scored: g.i(0, 1080),
        xf0: g.next(),
        ..Score::default()
    };
    for k in 0..3 {
        s.grabs[k] = if g.next() % 2 == 0 { 0 } else { 1 };
    }
    for e in s.timed.iter_mut() {
        *e = tricky_game::score::Timed { kind: g.next() % 2, value: g.next() as i32, life: g.f(-0.1, 2.0), age: g.f(0.0, 2.0), extra: g.next() as i32 };
    }
    s
}

#[test]
fn points_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let at = r.mem.alloc(0x300);
    let mut g = Rng(0x5c0e_0000_0000_0001);
    for case in 0..5000 {
        let s = random_score(&mut g);
        put(&mut r, at, &s);
        r.cpu.set_gpr(4, at as u64);
        r.call(0x155440).unwrap();
        assert_eq!(r.cpu.gpr(2) as i32, score::calc_points(s.units, s.mult), "case {case}: Score_CalcPoints {s:?}");
        r.cpu.set_gpr(4, at as u64);
        r.call(0x1554c0).unwrap();
        assert_eq!(r.cpu.gpr(2) as i32, score::calc_base_points(s.units), "case {case}: Score_CalcBasePoints");
        r.cpu.set_gpr(4, at as u64);
        r.call(0x155410).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), score::get_multiplier(s.units).to_bits(), "case {case}: Score_GetMultiplier");
    }
}

#[test]
fn spin_rounding_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x0018_0018_0018_0018);
    let mut v: Vec<i32> = (-1000..=1000).collect();
    v.extend((0..3000).map(|_| g.next() as i32));
    v.extend([i32::MIN, i32::MAX, i32::MIN + 1]);
    for d in v {
        r.cpu.set_gpr(5, d as i64 as u64);
        r.call(0x157198).unwrap();
        assert_eq!(r.cpu.gpr(2) as i32, score::round_spin_to_180(d), "Score_RoundSpinTo180({d})");
    }
}

#[test]
fn trick_units_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let at = r.mem.alloc(0x300);
    let rider = r.mem.alloc(0x200);
    let mut g = Rng(0x0bad_5eed_0bad_5eed);
    for case in 0..8000 {
        let mut ours = random_score(&mut g);
        put(&mut r, at, &ours);
        r.cpu.set_gpr(4, at as u64);
        let which = case % 4;
        match which {
            0 => {
                let d = g.i(-1500, 1500);
                r.cpu.set_gpr(5, d as i64 as u64);
                r.call(0x155e00).unwrap();
                ours.update_spin(d);
            }
            1 => {
                let d = g.i(-1500, 1500);
                r.cpu.set_gpr(5, d as i64 as u64);
                r.call(0x155e68).unwrap();
                ours.update_flip(d);
            }
            2 => {
                let clip = 0x270 + g.next() % 0x70;
                r.cpu.set_gpr(5, clip as u64);
                r.call(0x155ee8).unwrap();
                ours.start_grab(clip);
            }
            _ => {
                let v = [g.f(-900.0, 900.0), g.f(-900.0, 900.0), g.f(-300.0, 300.0), 0.0];
                for (k, x) in v.iter().enumerate() {
                    r.mem.write_f32(rider + 0x150 + 4 * k as u32, *x);
                }
                r.mem.write_u32(at + 0x24c, rider);
                r.call(0x156e80).unwrap();
                // |v| as the vector unit forms it: sqrt(((x² + y²) + z²) + w²)
                let speed = (((v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]) + v[3] * v[3]).sqrt();
                ours.tick(speed);
            }
        }
        let theirs = get(&mut r, at);
        assert_eq!(theirs.units.to_bits(), ours.units.to_bits(), "case {case} (fn {which}): units game {} ours {}", theirs.units, ours.units);
        assert_eq!(theirs, ours, "case {case} (fn {which})");
    }
}

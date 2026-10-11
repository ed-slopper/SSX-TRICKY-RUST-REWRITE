//! The computer rider's target and steering helpers against the original's (board row G4b): `AIComputer_SteerTowards`
//! 0x1372f8, `AIComputer_SetThrottle` 0x139f60, `AIComputer_SetTuck` 0x13a028, `AIComputer_IsRivalTarget` 0x139928
//! and `AIComputer_PickTargetRider` 0x1396c8 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF;
//! skipped without it) against `tricky_game::aicomp`, bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aicomp::{is_rival_target, pick_target_rider, set_throttle, set_tuck, steer_towards, Frame, GrudgeOf};

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
    fn unit(&mut self) -> [f32; 4] {
        let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        [v[0] / l, v[1] / l, v[2] / l, 0.0]
    }
}

const GAPP: u32 = 0x337c58;
const MODE: u32 = 0x32de8c;

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

struct World {
    r: Runner,
    comp: u32,
    rider: u32,
    world: u32,
    out: u32,
}

fn world() -> Option<World> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x2510f8u32, 0x250e98, 0x102f50, 0x139928] {
        r.allow(f); // Math_Atan, Math_Cos, Vec4_Scale, AIComputer_IsRivalTarget
    }
    let comp = r.mem.alloc(0x900);
    let rider = r.mem.alloc(0x5000);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let world = r.mem.alloc(0x100);
    let out = r.mem.alloc(0x10);
    r.mem.write_u32(comp, rider);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, world);
    Some(World { r, comp, rider, world, out })
}

#[test]
fn steering_and_throttle_match_the_original() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x2b77_1d3e_c905_6a41);
    let mut steered = 0;
    for case in 0..30_000 {
        let skill = if g.next() % 3 == 0 { 1.0 } else { g.f(0.5, 2.5) };
        let pos = [g.f(-5000.0, 5000.0), g.f(-5000.0, 5000.0), g.f(-500.0, 500.0), 1.0];
        let f = Frame { pos, fwd: g.unit(), side: g.unit(), up: if g.next() % 2 == 0 { [0.0, 0.0, 1.0, 0.0] } else { g.unit() } };
        let target = [pos[0] + g.f(-3000.0, 3000.0), pos[1] + g.f(-3000.0, 3000.0), pos[2] + g.f(-300.0, 300.0), 1.0];
        let vel = [g.f(-2000.0, 2000.0), g.f(-2000.0, 2000.0), g.f(-300.0, 300.0), 0.0];
        let m = &mut w.r.mem;
        m.write_f32(w.comp + 0x86c, skill);
        put4(m, w.rider + 0x140, f.pos);
        put4(m, w.rider + 0x4aa0, f.fwd);
        put4(m, w.rider + 0x4ab0, f.side);
        put4(m, w.rider + 0x4ac0, f.up);
        put4(m, w.rider + 0x150, vel);
        put4(m, w.out, target);
        w.r.cpu.set_gpr(4, w.comp as u64);
        w.r.cpu.set_gpr(5, w.out as u64);
        w.r.call(0x1372f8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let ours = steer_towards(skill, &f, target);
        assert_eq!(w.r.cpu.f(0).to_bits(), ours.to_bits(), "case {case}: steer (skill {skill}) {} vs {ours}", w.r.cpu.f(0));
        steered += (ours != 0.0) as u32;
        for (addr, tuck) in [(0x139f60u32, false), (0x13a028, true)] {
            let word = g.next();
            w.r.mem.write_u32(w.out, word);
            w.r.cpu.set_gpr(4, w.comp as u64);
            w.r.cpu.set_gpr(5, w.out as u64);
            w.r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
            let ours = if tuck { set_tuck(skill, vel, word) } else { set_throttle(skill, vel, word) };
            assert_eq!(w.r.mem.read_u32(w.out), ours, "case {case}: {addr:#x} skill {skill} vel {vel:?}");
        }
    }
    assert!(steered > 10_000, "steered: {steered}");
}

#[test]
fn target_choice_matches_the_original() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x7f30_a52c_9e18_0d47);
    let mut picked = [0u32; 3];
    for case in 0..30_000 {
        let n = 2 + (g.next() % 5) as i32; // a race has at most six riders: six pair records end at +0x11f, the grudge's +0x118
        let me = (g.next() % n as u32) as i32;
        let practice = g.next() % 5 == 0;
        let gr = GrudgeOf { on: (g.next() % 2) as i32, target: (g.next() % n as u32) as i32, level: (g.next() % 4) as i32 - 1, cooled: (g.next() % 2) as i32 };
        let heading = g.f(-7.0, 7.0);
        let x348 = [g.f(0.0, 500.0), g.f(500.0, 700.0), 540.0, 600.0][g.next() as usize % 4];
        let tick = (g.next() % 200) as i32 * if g.next() % 2 == 0 { 12 } else { 1 };
        let mut target = if g.next() % 2 == 0 { -1 } else { (g.next() % n as u32) as i32 };
        let recs: Vec<[f32; 2]> = (0..6).map(|_| [[g.f(0.0, 700.0), g.f(700.0, 1200.0), 700.0, 1050.0][g.next() as usize % 4], g.f(-3.5, 3.5)]).collect();
        let m = &mut w.r.mem;
        m.write_u32(MODE, if practice { 0 } else { 1 });
        m.write_u32(w.world + 0x18, tick as u32);
        m.write_u32(w.world + 0x88, n as u32);
        m.write_u32(w.comp + 0x890, target as u32);
        m.write_f32(w.rider + 0x1b0, heading);
        m.write_f32(w.rider + 0x348, x348);
        m.write_u32(w.rider + 0x460, me as u32);
        for (o, v) in [(0x118, gr.on), (0x120, gr.target), (0x124, gr.level), (0x128, gr.cooled)] {
            m.write_u32(w.rider + o, v as u32);
        }
        for (k, rc) in recs.iter().enumerate() {
            m.write_f32(w.rider + 0x40 + 0x20 * k as u32, rc[0]);
            m.write_f32(w.rider + 0x44 + 0x20 * k as u32, rc[1]);
        }
        let flag0 = g.next() % 2;
        m.write_u32(w.out, flag0);
        w.r.cpu.set_gpr(4, w.comp as u64);
        w.r.cpu.set_gpr(5, w.out as u64);
        w.r.call(0x1396c8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let t0 = target;
        let mut rival = flag0 != 0;
        let has = pick_target_rider(&mut target, &mut rival, &recs, heading, x348, me, n, tick, |i| is_rival_target(practice, &gr, i));
        let ctx = format!("case {case}: n {n} me {me} tick {tick} x348 {x348} grudge {gr:?} target0 {t0} flag0 {flag0} recs {:?} -> {target} practice {practice}", &recs[..n as usize]);
        assert_eq!(w.r.cpu.gpr(2) != 0, has, "{ctx}: has a target");
        assert_eq!(w.r.mem.read_u32(w.comp + 0x890) as i32, target, "{ctx}: target");
        assert_eq!(w.r.mem.read_u32(w.out) != 0, rival, "{ctx}: rival");
        picked[(has as usize) + (has && rival) as usize] += 1;
    }
    assert!(picked.iter().all(|n| *n > 200), "none / target / rival target: {picked:?}");
}

//! The computer rider's target and steering helpers against the original's (board row G4b): `AIComputer_SteerTowards`
//! 0x1372f8, `AIComputer_SetThrottle` 0x139f60, `AIComputer_SetTuck` 0x13a028, `AIComputer_IsRivalTarget` 0x139928
//! and `AIComputer_PickTargetRider` 0x1396c8 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF;
//! skipped without it) against `tricky_game::aicomp`, bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aicomp::{is_rival_target, pick_target_rider, set_throttle, set_tuck, steer_towards, Frame, GrudgeOf};

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

#[test]
fn shove_choice_matches_the_original() {
    use tricky_game::aicomp::find_nearby_rider;
    let Some(mut w) = world() else { return };
    let shove = std::rc::Rc::new(std::cell::RefCell::new(false));
    let s = shove.clone();
    w.r.stub(0x15eec0, move |c, _| c.set_gpr(2, if *s.borrow() { 0x15 } else { 2 }));
    let mut g = Rng(0x41d9_0c66_b3e2_5a18);
    let mut found = [0u32; 3];
    for case in 0..40_000 {
        let n = 2 + (g.next() % 5) as i32;
        let me = (g.next() % n as u32) as i32;
        let gr = GrudgeOf { on: (g.next() % 3 != 0) as i32, target: (g.next() % n as u32) as i32, level: (g.next() % 4) as i32 - 1, cooled: (g.next() % 2) as i32 };
        let heading = g.f(-7.0, 7.0);
        let tick = (g.next() % 100) as i32 * if g.next() % 2 == 0 { 6 } else { 1 };
        let shoving = g.next() % 5 == 0;
        let recs: Vec<[f32; 2]> = (0..6).map(|_| [[g.f(0.0, 200.0), g.f(200.0, 400.0), 200.0][g.next() as usize % 3], heading + g.f(-4.0, 4.0)]).collect();
        *shove.borrow_mut() = shoving;
        let m = &mut w.r.mem;
        m.write_u32(w.world + 0x18, tick as u32);
        m.write_u32(w.world + 0x88, n as u32);
        m.write_f32(w.rider + 0x1b0, heading);
        m.write_u32(w.rider + 0x460, me as u32);
        for (o, v) in [(0x118, gr.on), (0x120, gr.target), (0x124, gr.level), (0x128, gr.cooled)] {
            m.write_u32(w.rider + o, v as u32);
        }
        for (k, rc) in recs.iter().enumerate() {
            m.write_f32(w.rider + 0x40 + 0x20 * k as u32, rc[0]);
            m.write_f32(w.rider + 0x44 + 0x20 * k as u32, rc[1]);
        }
        m.write_u32(w.out, 7);
        w.r.cpu.set_gpr(4, w.comp as u64);
        w.r.cpu.set_gpr(5, w.out as u64);
        w.r.call(0x1394e0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = if w.r.cpu.gpr(2) != 0 { Some(w.r.mem.read_u32(w.out) != 0) } else { None };
        let ours = find_nearby_rider(shoving, tick, &recs, heading, me, n, &gr);
        assert_eq!(theirs, ours, "case {case}: n {n} me {me} tick {tick} grudge {gr:?} recs {recs:?} heading {heading}");
        found[match ours { None => 0, Some(false) => 1, Some(true) => 2 }] += 1;
    }
    assert!(found.iter().all(|n| *n > 200), "none / right / left: {found:?}");
}

#[test]
fn riding_modes_match_the_original() {
    // the four riding modes run as the dispatcher calls them, with the course queries (`AIComputer_FindJumpMarker`,
    // `AIComputer_FindSpeedHint`) answered from a script both sides read in the same order, the target rider's
    // position and velocity, and `AIComputer_PlanJumpTrick` recorded
    use std::cell::RefCell;
    use std::rc::Rc;
    use tricky_game::aicomp::{Ai, Mode, ModeCall, World as AiWorld};
    let Some(mut w) = world() else { return };
    for f in [0x1372f8u32, 0x139f60, 0x13a028, 0x1396c8, 0x250c10, 0x1375c8] {
        w.r.allow(f);
    }
    #[derive(Clone, Copy, Debug)]
    enum Ans { Jump(Option<([i32; 3], Option<f32>)>), Hint(Option<(i32, Option<f32>)>) }
    let script: Rc<RefCell<Vec<Ans>>> = Rc::default();
    let s = script.clone();
    w.r.stub(0x1392a8, move |c, m| {
        let a = s.borrow_mut().remove(0);
        let Ans::Jump(a) = a else { panic!("expected a jump query, got {a:?}") };
        match a {
            Some((v, sp)) => {
                if let Some(sp) = sp { m.write_f32(c.gpr(5) as u32, sp); }
                for (k, r) in [6usize, 7, 8].iter().enumerate() { m.write_u32(c.gpr(*r) as u32, v[k] as u32); }
                c.set_gpr(2, 1);
            }
            None => c.set_gpr(2, 0),
        }
    });
    let s = script.clone();
    w.r.stub(0x1393e0, move |c, m| {
        let a = s.borrow_mut().remove(0);
        let Ans::Hint(a) = a else { panic!("expected a hint query, got {a:?}") };
        match a {
            Some((flag, sp)) => {
                if let Some(sp) = sp { m.write_f32(c.gpr(5) as u32, sp); }
                m.write_u32(c.gpr(6) as u32, flag as u32);
                c.set_gpr(2, 1);
            }
            None => c.set_gpr(2, 0),
        }
    });
    let plans: Rc<RefCell<Vec<ModeCall>>> = Rc::default();
    let p = plans.clone();
    w.r.stub(0x139d58, move |c, _| p.borrow_mut().push(ModeCall::PlanJumpTrick(c.gpr(5) as i32, c.gpr(6) as i32)));
    let others: Vec<u32> = (0..6).map(|_| w.r.mem.alloc(0x200)).collect();
    let os = others.clone();
    w.r.stub(0x1814a8, move |c, _| c.set_gpr(2, os[c.gpr(5) as usize] as u64));
    let table = [0x31d318u32, 0x31d320, 0x31d328, 0x31d330];
    let mut g = Rng(0x0c3a_5fe1_8d27_b694);
    let mut seen = [[0u32; 4]; 4];
    for case in 0..40_000 {
        let n = 2 + (g.next() % 5) as i32;
        let me = (g.next() % n as u32) as i32;
        let mode = [Mode::ToJump, Mode::Line, Mode::NearRider, Mode::AtRival][g.next() as usize % 4];
        let target = if mode == Mode::Line || mode == Mode::ToJump { -1 } else { (me + 1 + (g.next() % (n as u32 - 1)) as i32) % n };
        let pos = [g.f(-5000.0, 5000.0), g.f(-5000.0, 5000.0), g.f(-500.0, 500.0), 1.0];
        let speed_scale = if g.next() % 2 == 0 { 1.0 } else { g.f(0.5, 2.5) };
        let mut ai = Ai {
            skill: speed_scale,
            speed: g.f(300.0, 3000.0),
            target,
            mode,
            frame: Frame { pos, fwd: g.unit(), side: g.unit(), up: if g.next() % 2 == 0 { [0.0, 0.0, 1.0, 0.0] } else { g.unit() } },
            vel: [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-300.0, 300.0), 0.0],
            line: [pos[0] + g.f(-3000.0, 3000.0), pos[1] + g.f(-3000.0, 3000.0), pos[2] + g.f(-300.0, 300.0), 1.0],
            travel: g.f(-7.0, 7.0),
            heading: g.f(-7.0, 7.0),
            x348: [g.f(0.0, 150.0), g.f(150.0, 700.0), 153.10213, 540.0][g.next() as usize % 4],
            meter: [1.0, g.f(0.0, 1.5), 0.9][g.next() as usize % 3],
            me,
            recs: (0..6).map(|_| [[g.f(0.0, 700.0), g.f(0.0, 1200.0), 0.0, 200.0][g.next() as usize % 4], g.f(-3.5, 3.5)]).collect(),
            grudge: GrudgeOf { on: (g.next() % 2) as i32, target: (g.next() % n as u32) as i32, level: (g.next() % 3) as i32, cooled: (g.next() % 2) as i32 },
        };
        let tick = (g.next() % 100) as i32 * if g.next() % 2 == 0 { 12 } else { 1 };
        let practice = g.next() % 5 == 0;
        let ov: Vec<(V4, V4)> = (0..6).map(|_| ([g.f(-5000.0, 5000.0), g.f(-5000.0, 5000.0), g.f(-500.0, 500.0), 1.0], [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-300.0, 300.0), 0.0])).collect();
        // the course's answers: enough for any path through the modes
        let mut answers = Vec::new();
        for k in 0..4 {
            let sp = if g.next() % 3 == 0 { Some(g.f(300.0, 3000.0)) } else { None };
            if k % 2 == 0 {
                answers.push(Ans::Jump(if g.next() % 3 == 0 { Some(([(g.next() % 2) as i32, (g.next() % 30) as i32, (g.next() % 30) as i32], sp)) } else { None }));
            } else {
                answers.push(Ans::Hint(if g.next() % 2 == 0 { Some(((g.next() % 2) as i32, sp)) } else { None }));
            }
        }
        // in the order the modes ask: line: hint, jump; to-jump: jump (then the line's hint, jump); others: jump
        let order: Vec<Ans> = match mode {
            Mode::Line => vec![answers[1], answers[0]],
            Mode::ToJump => vec![answers[0], answers[1], answers[2]],
            _ => vec![answers[0]],
        };
        *script.borrow_mut() = order.clone();
        plans.borrow_mut().clear();
        let m = &mut w.r.mem;
        m.write_f32(w.comp + 0x86c, ai.skill);
        m.write_f32(w.comp + 0x868, ai.speed);
        m.write_u32(w.comp + 0x890, ai.target as u32);
        m.write_u16(w.comp + 0x89c, mode as u16);
        m.write_u32(w.comp + 0x8a0, 0xdead);
        m.write_u32(w.comp + 0x8a4, 0xbeef);
        put4(m, w.rider + 0x140, ai.frame.pos);
        put4(m, w.rider + 0x4aa0, ai.frame.fwd);
        put4(m, w.rider + 0x4ab0, ai.frame.side);
        put4(m, w.rider + 0x4ac0, ai.frame.up);
        put4(m, w.rider + 0x150, ai.vel);
        put4(m, w.rider + 0x360, ai.line);
        m.write_f32(w.rider + 0x370, ai.travel);
        m.write_f32(w.rider + 0x1b0, ai.heading);
        m.write_f32(w.rider + 0x348, ai.x348);
        m.write_f32(w.rider + 0x1c, ai.meter);
        m.write_u32(w.rider + 0x460, me as u32);
        for (o, v) in [(0x118, ai.grudge.on), (0x120, ai.grudge.target), (0x124, ai.grudge.level), (0x128, ai.grudge.cooled)] {
            m.write_u32(w.rider + o, v as u32);
        }
        for (k, rc) in ai.recs.iter().enumerate() {
            m.write_f32(w.rider + 0x40 + 0x20 * k as u32, rc[0]);
            m.write_f32(w.rider + 0x44 + 0x20 * k as u32, rc[1]);
        }
        for (k, (p, v)) in ov.iter().enumerate() {
            put4(m, others[k] + 0x140, *p);
            put4(m, others[k] + 0x150, *v);
        }
        m.write_u32(MODE, if practice { 0 } else { 1 });
        m.write_u32(w.world + 0x18, tick as u32);
        m.write_u32(w.world + 0x88, n as u32);
        let word0 = g.next();
        m.write_u32(w.out, word0);
        let addr = [0x137858u32, 0x1375c8, 0x137dc0, 0x137a18][mode as usize];
        w.r.cpu.set_gpr(4, w.comp as u64);
        w.r.cpu.set_gpr(5, w.out as u64);
        w.r.call(addr).unwrap_or_else(|e| panic!("case {case} ({mode:?}): {e}"));
        let left = script.borrow().len();
        // ours, from the same answers
        let mut q = order;
        let qa = std::cell::RefCell::new(&mut q);
        let mut jump = |sp: &mut f32| {
            let a = qa.borrow_mut().remove(0);
            let Ans::Jump(a) = a else { panic!("ours: expected a jump query") };
            a.map(|(v, s)| { if let Some(s) = s { *sp = s; } v })
        };
        let mut calls = Vec::new();
        let word;
        {
            // both queries read the one queue, in the order asked
            let hint_queue = &qa;
            let mut hint = |sp: &mut f32| {
                let a = hint_queue.borrow_mut().remove(0);
                let Ans::Hint(a) = a else { panic!("ours: expected a hint query") };
                a.map(|(f, s)| { if let Some(s) = s { *sp = s; } f })
            };
            let other = |i: i32| ov[i as usize];
            let mut aw = AiWorld { jump_marker: &mut jump, speed_hint: &mut hint, other: &other, tick, n, practice };
            word = ai.ride(&mut aw, word0, &mut calls);
        }
        let ctx = format!("case {case} ({mode:?}) answers {:?}", answers);
        assert_eq!(w.r.mem.read_u32(w.out), word, "{ctx}: pad word {:#010x} vs {word:#010x}", w.r.mem.read_u32(w.out));
        assert_eq!(qa.borrow().len(), left, "{ctx}: queries asked");
        assert_eq!(*plans.borrow(), calls, "{ctx}: calls");
        assert_eq!(w.r.mem.read_u32(w.comp + 0x868), ai.speed.to_bits(), "{ctx}: speed");
        assert_eq!(w.r.mem.read_u32(w.comp + 0x890) as i32, ai.target, "{ctx}: target");
        let m2 = (w.r.mem.read_u32(w.comp + 0x89c) & 0xffff) as u16;
        assert_eq!(m2, ai.mode as u16, "{ctx}: mode");
        if ai.mode != mode {
            assert_eq!(w.r.mem.read_u32(w.comp + 0x8a0), w.r.mem.read_u32(table[ai.mode as usize]), "{ctx}: the mode's method");
        }
        seen[mode as usize][ai.mode as usize] += 1;
    }
    println!("mode changes {seen:?}");
}

#[test]
fn course_queries_match_the_original() {
    // `AIComputer_FindJumpMarker` 0x1392a8 and `AIComputer_FindSpeedHint` 0x1393e0 on a `cAIPath` in memory (its
    // vtable the game's, 0x38abd8, so its query `cPath::vf0` 0x197910 runs), and the query alone
    use tricky_game::aicomp::{events_between, find_jump_marker, find_speed_hint, PathEvent};
    let Some(mut w) = world() else { return };
    w.r.allow(0x197910);
    let path = w.r.mem.alloc(0x40);
    let evs = w.r.mem.alloc(16 * 40);
    let outs = w.r.mem.alloc(0x20);
    w.r.mem.write_u32(path + 0x34, 0x38abd8);
    w.r.mem.write_u32(w.rider + 0x57a4, path);
    let mut g = Rng(0x5e21_c0a4_7d93_1f86);
    let mut seen = [0u32; 4];
    for case in 0..30_000 {
        let n = (g.next() % 12) as usize;
        let mut events = Vec::new();
        for _ in 0..n {
            let s = g.f(0.0, 3000.0);
            events.push(PathEvent { kind: [0x19, 0x1a, 0x1f, 0, 0x1b][g.next() as usize % 5], value: g.next() % 400, start: s, end: s + [0.0, g.f(0.0, 800.0)][g.next() as usize % 2] });
        }
        for (k, e) in events.iter().enumerate() {
            let a = evs + 16 * k as u32;
            w.r.mem.write_u32(a, e.kind);
            w.r.mem.write_u32(a + 4, e.value);
            w.r.mem.write_f32(a + 8, e.start);
            w.r.mem.write_f32(a + 12, e.end);
        }
        w.r.mem.write_u32(path, n as u32);
        w.r.mem.write_u32(path + 4, evs);
        let at = g.f(0.0, 3000.0);
        let from = if g.next() % 4 == 0 { at + g.f(0.0, 400.0) } else { at - g.f(0.0, 100.0) };
        w.r.mem.write_f32(w.rider + 0x340, from);
        w.r.mem.write_f32(w.rider + 0x344, at);
        let sp0 = g.f(100.0, 3000.0);
        for which in 0..2 {
            w.r.mem.write_f32(w.out, sp0);
            for k in 0..3 { w.r.mem.write_u32(outs + 4 * k, 0x77); }
            w.r.cpu.set_gpr(4, w.comp as u64);
            w.r.cpu.set_gpr(5, w.out as u64);
            w.r.cpu.set_gpr(6, outs as u64);
            w.r.cpu.set_gpr(7, (outs + 4) as u64);
            w.r.cpu.set_gpr(8, (outs + 8) as u64);
            let addr = if which == 0 { 0x1392a8 } else { 0x1393e0 };
            w.r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
            let found = w.r.cpu.gpr(2) != 0;
            let sp = w.r.mem.read_f32(w.out);
            let mut ours_sp = sp0;
            if which == 0 {
                let ours = find_jump_marker(&events, from, at, &mut ours_sp);
                let theirs = found.then(|| [0, 1, 2].map(|k| w.r.mem.read_u32(outs + 4 * k) as i32));
                assert_eq!(theirs, ours, "case {case}: jump marker in {events:?} from {from} at {at}");
            } else {
                let ours = find_speed_hint(&events, from, at, &mut ours_sp);
                let theirs = found.then(|| w.r.mem.read_u32(outs) as i32);
                assert_eq!(theirs, ours, "case {case}: speed hint in {events:?} from {from} at {at}");
            }
            assert_eq!(sp.to_bits(), ours_sp.to_bits(), "case {case}: speed");
            seen[which * 2 + found as usize] += 1;
        }
        // the query on its own, with its type mask
        let mask0 = g.next() & 0xffff;
        w.r.mem.write_u32(outs + 0x10, mask0);
        let to = at + g.f(-100.0, 800.0);
        w.r.cpu.set_f(12, from);
        w.r.cpu.set_f(13, to);
        w.r.cpu.set_gpr(4, path as u64);
        w.r.cpu.set_gpr(5, outs as u64);
        w.r.cpu.set_gpr(6, (outs + 0x10) as u64);
        w.r.cpu.set_gpr(7, 0);
        w.r.call(0x197910).unwrap_or_else(|e| panic!("case {case}: query {e}"));
        let count = w.r.cpu.gpr(2) as usize;
        let list = w.r.mem.read_u32(outs);
        let theirs: Vec<usize> = (0..count).map(|k| ((w.r.mem.read_u32(list + 4 * k as u32) - evs) / 16) as usize).collect();
        let mut mask = mask0;
        let ours = events_between(&events, from, to, &mut mask);
        assert_eq!((theirs, w.r.mem.read_u32(outs + 0x10)), (ours, mask), "case {case}: query {from}..{to} over {events:?}");
    }
    assert!(seen.iter().all(|n| *n > 1000), "jump none/found, hint none/found: {seen:?}");
}

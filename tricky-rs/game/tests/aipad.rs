//! The computer rider's pad word and the score's grades (board row G4l) against the game in the function runner
//! (TRICKY_ELF; skipped without it): `cComputer::GetControls`, `AIComputer_ControlsRide`,
//! `AIComputer_CrossedEvent1b`, `FUN_001371e0`, `Score_GetGradeForPoints`, `Score_PointsToIconTier`,
//! `TrickId_Matches`, `FUN_00155538`, `Score_ClearTrickEntries`, `Scoring_MsgCheckpointTime`, the relation table's
//! tolerance and cap: results, fields and calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::aipad::{self, AiPadEnv};
use tricky_game::{grade, score};

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
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.below(100_000) as f32 / 100_000.0)
    }
    fn pick(&mut self, v: &[f32]) -> f32 {
        v[self.below(v.len() as u32) as usize]
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

const QUERY: u32 = 0x01f0_0000;

#[derive(Clone, Default)]
struct Path {
    events: Vec<u32>,
    flags: u32,
    asked: Vec<(u32, u32)>,
}

#[test]
fn ride_and_line_helpers_are_the_original() {
    let Some(mut r) = runner() else { return };
    let path = Rc::new(RefCell::new(Path::default()));
    let steer = Rc::new(RefCell::new(0f32));
    let rider = r.mem.alloc(0x5000);
    let comp = r.mem.alloc(0x900);
    let pobj = r.mem.alloc(0x40);
    let pvt = r.mem.alloc(0x40);
    let evs = r.mem.alloc(0x100);
    let evlist = r.mem.alloc(0x40);
    r.mem.write_u32(comp, rider);
    r.mem.write_u32(rider + 0x57a4, pobj);
    r.mem.write_u32(pobj + 0x34, pvt);
    r.mem.write_u32(pvt + 0xc, QUERY);
    let p = path.clone();
    r.stub(QUERY, move |c, m| {
        let mut p = p.borrow_mut();
        p.asked.push((c.f(12).to_bits(), c.f(13).to_bits()));
        assert_eq!(c.gpr(4) as u32, pobj, "the path object");
        assert_eq!(c.gpr(7), 0);
        let (list_out, flags_out) = (c.gpr(5) as u32, c.gpr(6) as u32);
        for (k, e) in p.events.iter().enumerate() {
            m.write_u32(evs + 4 * k as u32, *e);
            m.write_u32(evlist + 4 * k as u32, evs + 4 * k as u32);
        }
        m.write_u32(list_out, evlist);
        m.write_u32(flags_out, p.flags);
        c.set_gpr(2, p.events.len() as u64);
    });
    let s = steer.clone();
    r.stub(0x1372f8, move |c, _| c.set_f(0, *s.borrow()));
    r.allow(0x102f50);
    let pad_at = r.mem.alloc(8);
    let point = r.mem.alloc(16);
    let mut g = Rng(0xa1_9ad_0000_0001);
    for case in 0..40_000 {
        let line = g.f(0.0, 5.0);
        let dist = g.f(0.0, 90000.0);
        let vel = [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-400.0, 400.0), g.pick(&[0.0, 1.0])];
        let speed = g.f(0.0, 3000.0);
        let (crouch, brake, st) = (g.f(-1.2, 1.2), g.f(-1.2, 1.2), g.f(-1.2, 1.2));
        r.mem.write_f32(rider + 0x340, line);
        r.mem.write_f32(rider + 0x344, dist);
        for k in 0..4 {
            r.mem.write_f32(rider + 0x150 + 4 * k, vel[k as usize]);
        }
        r.mem.write_f32(comp + 0x868, speed);
        r.mem.write_f32(comp + 0x888, crouch);
        r.mem.write_f32(comp + 0x88c, brake);
        *steer.borrow_mut() = st;
        let events: Vec<u32> = (0..g.below(4)).map(|_| [0x19u32, 0x1b, 3, 0x19][g.below(4) as usize]).collect();
        let flags = if g.below(2) == 0 { 0x0200_0000 } else { g.next() & !0x0200_0000 };
        *path.borrow_mut() = Path { events: events.clone(), flags, asked: vec![] };
        let pad0 = g.next();
        r.mem.write_u32(pad_at, pad0);
        let mut asked = vec![];
        let mut q = |l: f32, d: f32| {
            asked.push((l.to_bits(), d.to_bits()));
            (events.clone(), flags)
        };
        match g.below(3) {
            0 => {
                r.cpu.set_gpr(4, comp as u64);
                r.cpu.set_gpr(5, pad_at as u64);
                r.call(0x1387e8).unwrap();
                let mut pad = pad0;
                aipad::controls_ride(&mut pad, line, dist, vel, speed, crouch, brake, &mut q, &mut || st);
                assert_eq!(r.mem.read_u32(pad_at), pad, "case {case}: ride's pad");
            }
            1 => {
                r.cpu.set_gpr(4, comp as u64);
                r.call(0x139210).unwrap();
                assert_eq!(r.cpu.gpr(2) != 0, aipad::crossed_event_1b(line, dist, &mut q), "case {case}: event 0x1b");
            }
            _ => {
                let axis = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), 0.0];
                let other = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), 0.0];
                let pt = [g.f(-500.0, 500.0), g.f(-500.0, 500.0), g.f(-50.0, 50.0), g.pick(&[0.0, 1.0])];
                let skill = g.pick(&[1.0, 0.5, 7.8941293, 9.0]);
                for k in 0..4 {
                    r.mem.write_f32(rider + 0x4aa0 + 4 * k, axis[k as usize]);
                    r.mem.write_f32(rider + 0x4ab0 + 4 * k, other[k as usize]);
                    r.mem.write_f32(point + 4 * k, pt[k as usize]);
                }
                r.mem.write_f32(comp + 0x86c, skill);
                r.cpu.set_gpr(4, comp as u64);
                r.cpu.set_gpr(5, point as u64);
                r.call(0x1371e0).unwrap();
                assert_eq!(r.cpu.f(0).to_bits(), aipad::across(pt, axis, other, skill).to_bits(), "case {case}: across");
                continue;
            }
        }
        assert_eq!(path.borrow().asked, asked, "case {case}: the path queries");
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Band,
    Run(&'static str),
    Stuck,
    Skill,
}

#[derive(Clone, Default)]
struct Ai {
    band: f32,
    stuck: u32,
    marks: [u32; 7],
    log: Vec<Ev>,
}

impl AiPadEnv for Ai {
    fn rubber_band(&mut self) -> f32 {
        self.log.push(Ev::Band);
        self.band
    }
    fn dispatch(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("dispatch"));
        *pad |= self.marks[0];
    }
    fn countdown(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("countdown"));
        *pad |= self.marks[1];
    }
    fn turn_around(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("turn"));
        *pad |= self.marks[2];
    }
    fn ride(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("ride"));
        *pad |= self.marks[3];
    }
    fn air(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("air"));
        *pad |= self.marks[4];
    }
    fn air_trick(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("trick"));
        *pad |= self.marks[5];
    }
    fn idle(&mut self, pad: &mut u32) {
        self.log.push(Ev::Run("idle"));
        *pad |= self.marks[6];
    }
    fn check_stuck(&mut self) -> u32 {
        self.log.push(Ev::Stuck);
        self.stuck
    }
    fn update_skill_scale(&mut self) {
        self.log.push(Ev::Skill);
    }
}

#[test]
fn get_controls_is_the_original() {
    let Some(mut r) = runner() else { return };
    let w = Rc::new(RefCell::new(Ai::default()));
    let x = w.clone();
    r.stub(0x13a0f0, move |c, _| {
        x.borrow_mut().log.push(Ev::Band);
        let v = x.borrow().band;
        c.set_f(0, v)
    });
    for (addr, name, k) in [(0x1389c8u32, "dispatch", 0usize), (0x138628, "countdown", 1), (0x138ac0, "turn", 2), (0x1387e8, "ride", 3), (0x138bb8, "air", 4), (0x138d60, "trick", 5), (0x138b60, "idle", 6)] {
        let x = w.clone();
        r.stub(addr, move |c, m| {
            let pad_at = c.gpr(5) as u32;
            let mark = x.borrow().marks[k];
            let v = m.read_u32(pad_at);
            m.write_u32(pad_at, v | mark);
            x.borrow_mut().log.push(Ev::Run(name));
        });
    }
    let x = w.clone();
    r.stub(0x139ab8, move |c, _| {
        x.borrow_mut().log.push(Ev::Stuck);
        let v = x.borrow().stuck;
        c.set_gpr(2, v as u64)
    });
    let x = w.clone();
    r.stub(0x13a220, move |_, _| x.borrow_mut().log.push(Ev::Skill));
    // EAMem_Set
    r.stub(0x2cc808, |c, m| {
        let (d, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
        for k in 0..n {
            m.write_u8(d + k, v);
        }
    });
    r.allow(0x11cff0);
    r.allow(0x1389c0);
    let rider = r.mem.alloc(0x500);
    let comp = r.mem.alloc(0x900);
    r.mem.write_u32(comp, rider);
    let pad_at = r.mem.alloc(8);
    let mut g = Rng(0x6e7c_0000_0000_0001);
    for case in 0..30_000 {
        let control = g.below(0x18);
        let ts = g.pick(&[1.0, 0.5, 1.2, 0.99]);
        let mut ours = Ai { band: g.pick(&[1.0, 0.5, 1.2, 0.995]), stuck: g.next(), marks: [0; 7].map(|_: u32| g.next() & 0x00ff_ff00), log: vec![] };
        *w.borrow_mut() = ours.clone();
        r.mem.write_u32(rider + 0x428, control);
        r.mem.write_f32(rider + 0x12c, ts);
        r.mem.write_u32(pad_at, g.next());
        r.cpu.set_gpr(4, comp as u64);
        r.cpu.set_gpr(5, pad_at as u64);
        r.call(0x1384b8).unwrap();
        let mut t = ts;
        let pad = aipad::get_controls(&mut ours, control, &mut t);
        assert_eq!(r.mem.read_u32(pad_at), pad, "case {case} control {control:#x}: pad");
        assert_eq!(r.mem.read_u32(rider + 0x12c), t.to_bits(), "case {case}: time scale");
        assert_eq!(w.borrow().log, ours.log, "case {case} control {control:#x}: calls");
    }
}

#[test]
fn grades_and_trick_helpers_are_the_original() {
    let Some(mut r) = runner() else { return };
    let pushed = Rc::new(RefCell::new(None));
    let p = pushed.clone();
    r.stub(0x157330, move |c, _| *p.borrow_mut() = Some((c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as i32, c.gpr(7) as u32)));
    r.allow(0x155440);
    let obj = r.mem.alloc(0x400);
    let rider = r.mem.alloc(0x500);
    let entry = r.mem.alloc(0x80);
    r.mem.write_u32(obj + 0x24c, rider);
    r.mem.write_u32(rider + 0x464, entry);
    let a = r.mem.alloc(8);
    let b = r.mem.alloc(8);
    let out = r.mem.alloc(8);
    let table = r.mem.alloc(12 * 0xc0);
    let mut tbytes = vec![0u8; 12 * 0xc0];
    let mut g = Rng(0x96ad_e000_0000_0001);
    for (k, v) in tbytes.chunks_mut(4).enumerate() {
        let w = g.next();
        v.copy_from_slice(&w.to_le_bytes());
        r.mem.write_u32(table + 4 * k as u32, w);
    }
    let mut seen = [0u32; 6];
    for case in 0..60_000 {
        // the grade of Score_CalcPoints(units, multiplier) at a level
        let level = g.below(12) as i32 - 1;
        let (units, mult) = (g.f(0.0, 9.0), g.pick(&[1.0, 2.0, 3.0, 0.5, 1.5]));
        r.mem.write_u32(entry + 0x74, level as u32);
        r.mem.write_f32(obj + 0x24, units);
        r.mem.write_f32(obj + 0x28, mult);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x155588).unwrap();
        let gr = grade::grade_for_points(score::calc_points(units, mult), level);
        assert_eq!(r.cpu.gpr(2) as u32, gr, "case {case}: grade of {units} × {mult} at level {level}");
        seen[gr as usize] += 1;
        // the icon tier
        let pts = [4000, 8000, 12000, 16000, 0, 4001, 20000][g.below(7) as usize];
        r.cpu.set_gpr(5, pts as u64);
        r.call(0x156900).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32, grade::icon_tier(pts), "case {case}: icon");
        // trick ids
        let x = [g.below(3), g.next()];
        let y = [if g.below(2) == 0 { x[0] } else { g.below(3) }, if g.below(2) == 0 { x[1] ^ (g.next() & !0x0700_0f00) } else { g.next() }];
        r.mem.write_u32(a, x[0]);
        r.mem.write_u32(a + 4, x[1]);
        r.mem.write_u32(b, y[0]);
        r.mem.write_u32(b + 4, y[1]);
        r.cpu.set_gpr(5, a as u64);
        r.cpu.set_gpr(6, b as u64);
        r.call(0x157c18).unwrap();
        assert_eq!(r.cpu.gpr(2) != 0, grade::trick_id_matches(x, y), "case {case}: trick id");
        // the score's entries
        let mut kinds = [0i32; 12];
        for k in kinds.iter_mut() {
            *k = [0, 1, 7, 8, 3][g.below(5) as usize] * (g.below(3) == 0) as i32;
        }
        for (k, v) in kinds.iter().enumerate() {
            r.mem.write_u32(obj + 0x15c + 0x14 * k as u32, *v as u32);
        }
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, out as u64);
        r.call(0x155538).unwrap();
        let (found, eight) = grade::find_kind_8_or_7(&kinds);
        assert_eq!((r.cpu.gpr(2) as u32, r.mem.read_u32(out) != 0), (if found { obj + 0x250 } else { 0 }, eight), "case {case}: kind 8 or 7");
        let kind = [0, 7, 8, 3][g.below(4) as usize];
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, kind as u64);
        r.call(0x157308).unwrap();
        grade::clear_trick_entries(&mut kinds, kind);
        for (k, v) in kinds.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 0x15c + 0x14 * k as u32) as i32, *v, "case {case}: cleared entry {k}");
        }
        // the checkpoint time message
        let (secs, late) = (g.f(-30.0, 30.0), g.below(2) == 0);
        r.cpu.set_f(12, secs);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, late as u64);
        r.call(0x155c60).unwrap();
        let (d, k, t) = grade::checkpoint_time_message(secs, late);
        assert_eq!(*pushed.borrow(), Some((d.to_bits(), k, t, 0)), "case {case}: checkpoint message");
        // the relation table's words
        let (x, y) = (g.below(12) as usize, g.below(12) as usize);
        for (f, w) in [(0x167dc0u32, 0usize), (0x167de0, 2)] {
            r.cpu.set_gpr(4, table as u64);
            r.cpu.set_gpr(5, x as u64);
            r.cpu.set_gpr(6, y as u64);
            r.call(f).unwrap();
            assert_eq!(r.cpu.gpr(2) as u32, grade::rel_word(&tbytes, x, y, w), "case {case}: rel word {w}");
        }
    }
    eprintln!("grades {seen:?}");
    assert!(seen.iter().all(|n| 0 < *n), "every grade reached");
}

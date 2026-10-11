//! The scenery controllers (board row E9j) against the game in the function runner (TRICKY_ELF; skipped without
//! it): `cCrowdBoxNode` built and updated over random race frames, riders and box states; `cParticleNode` with random
//! lifetimes, messages and updates; the jumbotron video's name for every course; and the crowd tables read from the
//! ELF.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::scenery::{jumbotron_video, Crowd, CrowdEnv, Particle, ParticleCall, ParticleEnv, CROWD_REST, CROWD_SETS};
use tricky_game::texanim::AnimEnv;

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

#[derive(Debug, PartialEq, Clone, Copy)]
enum Ev {
    Ctrl(CtrlCall),
    P(ParticleCall),
    Box(u32),
}

struct Env {
    flags: u32,
    events: HashMap<u32, i32>,
    fx: VecDeque<u32>,
    frame: (i32, i32),
    riders: Vec<(i32, [f32; 4], u32)>,
    obj: [f32; 4],
    states: HashMap<u32, i32>,
    log: Vec<Ev>,
}

impl CtrlEnv for Env {
    fn flags(&mut self, _: u32) -> &mut u32 {
        &mut self.flags
    }
    fn event_script(&self, _: u32, event: u32) -> i32 {
        self.events.get(&event).copied().unwrap_or(-1)
    }
    fn rider_in_box(&self, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn call(&mut self, c: CtrlCall) {
        self.log.push(Ev::Ctrl(c));
    }
}
impl AnimEnv for Env {
    fn rand_fx(&mut self) -> u32 {
        self.fx.pop_front().unwrap()
    }
    fn contact_script(&mut self) {}
}
impl CrowdEnv for Env {
    fn race_frame(&self) -> (i32, i32) {
        self.frame
    }
    fn riders(&self) -> Vec<(i32, [f32; 4], u32)> {
        self.riders.clone()
    }
    fn object_pos(&self) -> [f32; 4] {
        self.obj
    }
    fn box_state(&mut self, x38: u32) -> i32 {
        self.log.push(Ev::Box(x38));
        self.states[&x38]
    }
}
impl ParticleEnv for Env {
    fn particles(&mut self, c: ParticleCall) {
        self.log.push(Ev::P(c));
    }
}

fn runner() -> Option<(Runner, Rc<RefCell<Vec<Ev>>>, Rc<RefCell<HashMap<u32, i32>>>, Rc<RefCell<VecDeque<u32>>>)> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    let fx: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x13a980, 0x145c00, 0x145e08, 0x1d8ac8, 0x1d8b20] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    r.stub(0x2cc808, |c, m| {
        let v = vec![c.gpr(5) as u8; c.gpr(6) as usize];
        m.write(c.gpr(4) as u32, &v).unwrap();
    });
    let q = fx.clone();
    r.stub(0x23d4a8, move |c, _| {
        let v = q.borrow_mut().pop_front().expect("fx");
        c.set_gpr(2, v as u64);
    });
    let e = events.clone();
    r.stub(0x25fe78, move |c, _| {
        let v = e.borrow().get(&(c.gpr(6) as u32)).copied().unwrap_or(-1);
        c.set_gpr(2, v as i64 as u64);
    });
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(Ev::Ctrl(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32))));
    for a in [0x149610, 0x1480b0] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(Ev::Ctrl(CtrlCall::Delete)));
    }
    Some((r, log, events, fx))
}

#[test]
fn crowd_tables_are_the_games() {
    let Some(mut r) = Runner::from_env() else { return };
    for (s, set) in CROWD_SETS.iter().enumerate() {
        for (k, v) in set.iter().enumerate() {
            assert_eq!(r.mem.read_u32(0x31d378 + 0x40 * s as u32 + 4 * k as u32) as i32, *v);
        }
    }
    for (k, v) in CROWD_REST.iter().enumerate() {
        assert_eq!(r.mem.read_u32(0x31d478 + 4 * k as u32), *v);
    }
}

#[test]
fn crowd_box_matches_the_original() {
    let Some((mut r, log, _events, fx)) = runner() else { return };
    let gapp = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x40);
    let racers = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, race);
    r.mem.write_u32(race + 0x1c, racers);
    let riders: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x500)).collect();
    let states: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    let (l, st) = (log.clone(), states.clone());
    r.stub(0x2213f8, move |c, _| {
        l.borrow_mut().push(Ev::Box(c.gpr(5) as u32));
        c.set_gpr(2, st.borrow()[&(c.gpr(5) as u32)] as i64 as u64);
    });
    let node = r.mem.alloc(0x120);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let mut g = Rng(0xc20d_b0c5_1357_9bdf);
    for case in 0..3000 {
        let (rows, cols) = (1 + g.next() % 4, 1 + g.next() % 4);
        let a = [17u32, 0, rows, cols];
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let fxw: Vec<u32> = (0..4000).map(|_| g.next() << 8 ^ g.next()).collect();
        *fx.borrow_mut() = fxw.iter().copied().collect();
        let mut env = Env { flags, events: HashMap::new(), fx: fxw.iter().copied().collect(), frame: (0, 0), riders: vec![], obj: [0.0; 4], states: HashMap::new(), log: vec![] };
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.call(0x145b28).unwrap();
        let mut c = Crowd::new(object, &a, &mut env);
        let read = |m: &mut r5900::Mem, c: &Crowd| {
            let ph: Vec<i32> = (0..16).map(|i| m.read_u32(node + 0x84 + 4 * i) as i32).collect();
            let se: Vec<u32> = (0..16).map(|i| m.read_u32(node + 0xc4 + 4 * i)).collect();
            let fr: Vec<i32> = (0..c.frame.len() as u32).map(|i| ((m.read_u32(node + 0x44 + 4 * i) - 0x31d4b8) / 0x48) as i32).collect();
            (m.read_u32(node + 0x3c) as i32, ph, se, fr, m.read_u32(object + 0xe8))
        };
        let mine = |c: &Crowd, f: u32| (c.excited, c.phase.to_vec(), c.set.to_vec(), c.frame.clone(), f);
        assert_eq!(read(&mut r.mem, &c), mine(&c, env.flags), "case {case} built");
        for step in 0..60 {
            let frame = g.pick(&[0i32, 5, 10, 3, 15, 7]);
            let mode = g.pick(&[0i32, 1, 0, 2]);
            r.mem.write_u32(racers + 0x18, frame as u32);
            r.mem.write_u32(racers + 0x1c, mode as u32);
            let n = g.next() % 4;
            r.mem.write_u32(racers + 0x88, n);
            env.frame = (frame, mode);
            env.riders.clear();
            let opos = [g.pick(&[0.0f32, 1000.0]), g.pick(&[0.0f32, -500.0]), 0.0, 1.0];
            for k in 0..4 {
                r.mem.write_f32(object + 0x30 + 4 * k as u32, opos[k]);
            }
            env.obj = opos;
            states.borrow_mut().clear();
            env.states.clear();
            for i in 0..n {
                let rp = riders[i as usize];
                let human = g.pick(&[1i32, 0, 1]);
                let pos = [g.pick(&[0.0f32, 15000.0, 25000.0, 900.0]), g.pick(&[0.0f32, 14000.0]), g.pick(&[0.0f32, 300.0]), 1.0];
                let x38 = 0x500 + i;
                r.mem.write_u32(racers + 0xc4 + 4 * i, rp);
                r.mem.write_u32(rp + 0x41c, human as u32);
                r.mem.write_u32(rp + 0x38, x38);
                for k in 0..4 {
                    r.mem.write_f32(rp + 0x140 + 4 * k as u32, pos[k]);
                }
                let s = g.pick(&[0i32, 1, 2, 3, 5, -1, 9]);
                states.borrow_mut().insert(x38, s);
                env.states.insert(x38, s);
                env.riders.push((human, pos, x38));
            }
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            r.call(0x145c88).unwrap();
            c.update(&mut env);
            assert_eq!(*log.borrow(), env.log, "case {case} step {step}");
            assert_eq!(read(&mut r.mem, &c), mine(&c, env.flags), "case {case} step {step}");
        }
        assert_eq!(fx.borrow().len(), env.fx.len(), "case {case} random words");
    }
}

#[test]
fn particle_node_matches_the_original() {
    let Some((mut r, log, events, _fx)) = runner() else { return };
    let life0: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.0));
    let lf = life0.clone();
    r.stub(0x1d8488, |_, _| {});
    r.stub(0x1d8490, move |c, m| m.write_f32(c.gpr(4) as u32, *lf.borrow()));
    let l = log.clone();
    r.stub(0x1d59e0, move |c, _| l.borrow_mut().push(Ev::P(ParticleCall::SetTime(c.f(12).to_bits()))));
    let l = log.clone();
    r.stub(0x1d5a30, move |c, _| {
        assert_eq!(c.f(12).to_bits(), 0x3c88_8889);
        l.borrow_mut().push(Ev::P(ParticleCall::StepEmitter));
    });
    let l = log.clone();
    r.stub(0x1d59f0, move |c, _| {
        assert_eq!(c.f(12).to_bits(), 0x3c88_8889);
        l.borrow_mut().push(Ev::P(ParticleCall::AdvanceTime));
    });
    let node = r.mem.alloc(0x200);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let mut g = Rng(0x9a27_1c1e_0000_1111);
    for case in 0..5000 {
        let life = g.pick(&[-1.0f32, 0.0, 0.5, 1.0 / 60.0, 2.0, -0.5]);
        *life0.borrow_mut() = life;
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let mut env = Env { flags, events: HashMap::new(), fx: VecDeque::new(), frame: (0, 0), riders: vec![], obj: [0.0; 4], states: HashMap::new(), log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, 2);
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.call(0x147f88).unwrap();
        let mut p = Particle::new(object, life, &mut env);
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        assert_eq!(r.mem.read_u32(node + 0x30), p.life.to_bits(), "case {case} built");
        for step in 0..60 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            let gone = if g.next() % 5 == 0 {
                let v = g.pick(&[0.0f32, 0.25, 3.0]);
                let k = g.pick(&[1u32, 7, 8, 2]);
                r.cpu.set_f(12, v);
                r.cpu.set_gpr(5, k as u64);
                r.call(0x148188).unwrap();
                p.on_message(v, k, &mut env);
                false
            } else {
                r.call(0x148100).unwrap();
                p.update(&mut env)
            };
            assert_eq!(*log.borrow(), env.log, "case {case} step {step}");
            assert_eq!((r.mem.read_u32(node + 0x30), r.mem.read_u32(object + 0xe8)), (p.life.to_bits(), env.flags), "case {case} step {step}");
            if gone {
                break;
            }
        }
    }
}

#[test]
fn jumbotron_videos_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let name: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let n = name.clone();
    r.stub(0x1fff30, move |c, m| {
        assert_eq!((c.gpr(6), c.gpr(7), c.gpr(8), c.gpr(9)), (0x80, 0x40, 1, 0xf));
        let mut b = vec![0u8; 40];
        m.read(c.gpr(5) as u32, &mut b);
        *n.borrow_mut() = String::from_utf8_lossy(&b[..b.iter().position(|x| *x == 0).unwrap()]).to_string();
        c.set_gpr(2, 0x1234);
    });
    // strcat (it uses MMI words the runner does not run)
    r.stub(0x2fcd4c, |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let mut n = 0;
        let mut b = [0u8];
        loop {
            m.read(d + n, &mut b);
            if b[0] == 0 {
                break;
            }
            n += 1;
        }
        let mut k = 0;
        loop {
            m.read(s + k, &mut b);
            m.write(d + n + k, &b).unwrap();
            if b[0] == 0 {
                break;
            }
            k += 1;
        }
        c.set_gpr(2, d as u64);
    });
    let gapp = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x80);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, race);
    let node = r.mem.alloc(0x40);
    for course in -2..16i32 {
        r.mem.write_u32(0x32de88, course as u32);
        r.cpu.set_gpr(4, node as u64);
        r.call(0x148b28).unwrap();
        assert_eq!(*name.borrow(), jumbotron_video(course), "course {course}");
        assert_eq!(r.mem.read_u32(node + 0x2c), 0x1234);
    }
}

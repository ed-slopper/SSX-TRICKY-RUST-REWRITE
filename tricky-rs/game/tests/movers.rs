//! The movers (board row E9h) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `cRailNode` built by its constructor, then random Update / Kill / Finish / OnMessage, comparing the object's
//! and the spline segment's flags, the node and the calls out; and the controller layer of `cRollerNode`: a node
//! with random momenta, inertia, energy, knock and time awake run through `cRollerNode::Update` up to its rigid-body
//! integration, comparing the forces it hands over, the sleep and the fields with `tricky_game::controllers::Roller`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controllers::{CtrlCall, CtrlEnv, RailEnv, RailNode, Roller};

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

struct Env {
    flags: u32,
    seg: u32,
    events: HashMap<u32, i32>,
    log: Vec<CtrlCall>,
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
        self.log.push(c);
    }
}
impl RailEnv for Env {
    fn segment_flags(&mut self, _: i32) -> &mut u32 {
        &mut self.seg
    }
}

fn setup(r: &mut Runner, log: &Rc<RefCell<Vec<CtrlCall>>>, events: &Rc<RefCell<HashMap<u32, i32>>>) {
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x2609b0, 0x1403c0, 0x140400, 0x140218, 0x102f50] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    let l = log.clone();
    r.stub(0x13a8e0, move |_, _| l.borrow_mut().push(CtrlCall::Delete));
    let e = events.clone();
    r.stub(0x25fe78, move |c, _| {
        let v = e.borrow().get(&(c.gpr(6) as u32)).copied().unwrap_or(-1);
        c.set_gpr(2, v as i64 as u64);
    });
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32)));
}

#[test]
fn rail_node_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let log: Rc<RefCell<Vec<CtrlCall>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    setup(&mut r, &log, &events);
    let seg = r.mem.alloc(0x40);
    r.stub(0x254b20, move |c, _| {
        assert_eq!(c.gpr(4) as u32, 0x346488);
        c.set_gpr(2, seg as u64);
    });
    let node = r.mem.alloc(0x40);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let mut g = Rng(0x7a11_5e60_1357_2468);
    for case in 0..4000 {
        let args = [9u32, g.pick(&[0u32, 3, 0xffff, 0x1_0005]), g.pick(&[0u32, 1, 7]), g.pick(&[0.0f32, 1.0 / 60.0, 0.05, 2.0, -0.1]).to_bits()];
        for (k, a) in args.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *a);
        }
        let flags = g.next();
        let segf = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        r.mem.write_u32(seg + 0x18, segf);
        let mut env = Env { flags, seg: segf, events: HashMap::new(), log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, g.pick(&[0, 4]));
        }
        *events.borrow_mut() = env.events.clone();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.call(0x1400d8).unwrap();
        let mut n = RailNode::new(object, &args, &mut env);
        let read = |m: &mut r5900::Mem| (m.read_u32(node + 0x34) as i32, m.read_u32(node + 0x38) as i32, m.read_u32(node + 0x3c));
        assert_eq!(read(&mut r.mem), (n.segment, n.ticks, n.on), "case {case}");
        assert_eq!((r.mem.read_u32(object + 0xe8), r.mem.read_u32(seg + 0x18)), (env.flags, env.seg), "case {case} constructed");
        for step in 0..40 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            let what = g.pick(&[0u32, 0, 0, 0, 1, 2, 3, 3]);
            match what {
                0 => {
                    r.call(0x1402b0).unwrap();
                    n.update(&mut env);
                }
                1 => {
                    r.call(0x1402f8).unwrap();
                    n.kill(&mut env);
                }
                2 => {
                    let f = g.next() % 2;
                    r.cpu.set_gpr(5, f as u64);
                    r.call(0x140338).unwrap();
                    n.finish(f, &mut env);
                }
                _ => {
                    let k = g.pick(&[0u32, 1, 7, 8, 2]);
                    r.cpu.set_gpr(5, k as u64);
                    r.call(0x140398).unwrap();
                    n.on_message(k, &mut env);
                }
            }
            assert_eq!(*log.borrow(), env.log, "case {case} step {step} ({what})");
            assert_eq!((r.mem.read_u32(object + 0xe8), r.mem.read_u32(seg + 0x18)), (env.flags, env.seg), "case {case} step {step} ({what})");
            assert_eq!(read(&mut r.mem), (n.segment, n.ticks, n.on), "case {case} step {step}");
            if env.log.contains(&CtrlCall::Delete) {
                break;
            }
        }
    }
}

#[test]
fn roller_control_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let log: Rc<RefCell<Vec<CtrlCall>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    setup(&mut r, &log, &events);
    for a in [0x13e860, 0x149e28, 0x149e18, 0x149e10] {
        r.allow(a);
    }
    let forces: Rc<RefCell<Vec<([u32; 4], [u32; 4])>>> = Rc::new(RefCell::new(vec![]));
    let f = forces.clone();
    r.stub(0x153c50, move |c, m| {
        assert_eq!(c.f(12).to_bits(), 0x3c88_8889);
        let v = |m: &mut r5900::Mem, at: u32| [0, 1, 2, 3].map(|k| m.read_u32(at + 4 * k));
        let pair = (v(m, c.gpr(5) as u32), v(m, c.gpr(6) as u32));
        f.borrow_mut().push(pair);
    });
    for a in [0x13e140, 0x153ad8, 0x13e6a0, 0x250428, 0x239d60, 0x14fe68, 0x13eda8] {
        r.stub(a, |_, _| {});
    }
    r.stub(0x239c50, |c, _| c.set_f(0, 50.0));
    let node = r.mem.alloc(0x400);
    let object = r.mem.alloc(0x100);
    r.mem.write_u32(node + 8, 0x36d790);
    r.mem.write_u32(node + 0x28, object);
    let mut g = Rng(0x4011_e400_0ddb_a11e);
    let mut pushed = 0;
    for case in 0..20000 {
        let fl = |g: &mut Rng| g.pick(&[0.0f32, 1.0, -2.5, 120.0, 900.0, -3000.0, 0.01]);
        let v4 = |g: &mut Rng| [fl(g), fl(g), fl(g), g.pick(&[0.0f32, 0.0, 1.0])];
        let mut ro = Roller {
            object,
            inv_mass: g.pick(&[0.0f32, 0.01, 0.002, 1.0, -0.5]),
            inv_inertia: [0; 9].map(|_| g.pick(&[0.0f32, 1e-4, -2e-4, 0.5])),
            momentum: v4(&mut g),
            ang_momentum: v4(&mut g),
            vel: v4(&mut g),
            ang_vel: v4(&mut g),
            energy: g.pick(&[0.0f32, 9999.0, 10000.0, 50000.0, 1e7]),
            awake: g.pick(&[0.0f32, 1.0, 9.98, 9.99, 10.0]),
            knock: g.pick(&[0.0f32, 0.1, 0.5, -0.2, 3.0]),
            asleep: (g.next() % 6 == 0) as u32,
        };
        let w4 = |m: &mut r5900::Mem, at: u32, v: [f32; 4]| {
            for k in 0..4 {
                m.write_f32(at + 4 * k as u32, v[k]);
            }
        };
        r.mem.write_f32(node + 0x38, ro.inv_mass);
        for (k, v) in ro.inv_inertia.iter().enumerate() {
            r.mem.write_f32(node + 0x3c + 4 * k as u32, *v);
        }
        w4(&mut r.mem, node + 0x80, ro.momentum);
        w4(&mut r.mem, node + 0x90, ro.ang_momentum);
        w4(&mut r.mem, node + 0xa0, ro.vel);
        w4(&mut r.mem, node + 0xb0, ro.ang_vel);
        r.mem.write_f32(node + 0xc8, ro.energy);
        r.mem.write_f32(node + 0xe4, ro.awake);
        r.mem.write_f32(node + 0xe8, ro.knock);
        r.mem.write_u32(node + 0x140, ro.asleep);
        let mut env = Env { flags: 0, seg: 0, events: HashMap::new(), log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, 6);
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        forces.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.call(0x13e8a8).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        let out = ro.update(&mut env);
        let want: Vec<([u32; 4], [u32; 4])> = out.iter().map(|(a, b)| (a.map(f32::to_bits), b.map(f32::to_bits))).collect();
        assert_eq!(*forces.borrow(), want, "case {case} {ro:?}");
        assert_eq!(*log.borrow(), env.log, "case {case}");
        let rv = |m: &mut r5900::Mem, at: u32| [0, 1, 2, 3].map(|k| m.read_u32(at + 4 * k));
        let got = (
            rv(&mut r.mem, node + 0x80),
            rv(&mut r.mem, node + 0x90),
            rv(&mut r.mem, node + 0xa0),
            rv(&mut r.mem, node + 0xb0),
            [r.mem.read_u32(node + 0xc8), r.mem.read_u32(node + 0xe4), r.mem.read_u32(node + 0xe8), r.mem.read_u32(node + 0x140)],
        );
        let b = |v: [f32; 4]| v.map(f32::to_bits);
        assert_eq!(got, (b(ro.momentum), b(ro.ang_momentum), b(ro.vel), b(ro.ang_vel), [ro.energy.to_bits(), ro.awake.to_bits(), ro.knock.to_bits(), ro.asleep]), "case {case}");
        pushed += want.len();
    }
    eprintln!("{pushed} integrations compared");
}

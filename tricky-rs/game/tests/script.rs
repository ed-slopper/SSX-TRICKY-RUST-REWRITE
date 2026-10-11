//! The script threads (board row E9b) against the game: random scripts of every op run frame by frame through
//! `TriggerScript_Run` 0x13be78 in the function runner (TRICKY_ELF; skipped without it), every call out of it
//! stubbed and logged, against `tricky_game::script` (the calls in order, and the thread's +0x38, +0x3c, +0x40 after
//! each frame).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_data::ssf::Op;
use tricky_game::script::{run, Context, ScriptCall, ScriptEnv, ScriptRef, Thread};

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
    ops: Vec<Op>,
    rand: VecDeque<f32>,
    vel: [f32; 4],
    x41c: i32,
    controllers: HashMap<u32, u32>,
    instances: HashMap<u32, u32>,
    camera: u32,
    log: Vec<ScriptCall>,
}

impl ScriptEnv for Env {
    fn ops(&self, _: ScriptRef) -> &[Op] {
        &self.ops
    }
    fn rand01(&mut self) -> f32 {
        self.rand.pop_front().unwrap()
    }
    fn rider_velocity(&self, _: u32) -> [f32; 4] {
        self.vel
    }
    fn rider_x41c(&self, _: u32) -> i32 {
        self.x41c
    }
    fn controller(&self, object: u32) -> u32 {
        self.controllers.get(&object).copied().unwrap_or(0)
    }
    fn instance(&self, index: u32) -> u32 {
        self.instances.get(&index).copied().unwrap_or(0)
    }
    fn camera(&self, _: u32) -> u32 {
        self.camera
    }
    fn call(&mut self, c: ScriptCall) {
        self.log.push(c);
    }
}

// fake method addresses for the vtables
const END: u32 = 0x00f0_0000;
const SWITCH: u32 = 0x00f0_0010;
const XBC: u32 = 0x00f0_0020;
const MESSAGE: u32 = 0x00f0_0030;

fn ctx(m: &mut r5900::Mem, at: u32) -> Option<Context> {
    if at == 0 {
        return None;
    }
    let mut c = [0u32; 16];
    for (k, v) in c.iter_mut().enumerate() {
        *v = m.read_u32(at + 4 * k as u32);
    }
    Some(c)
}

#[test]
fn script_threads_run_as_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<ScriptCall>>> = Rc::new(RefCell::new(vec![]));
    let rand: Rc<RefCell<VecDeque<f32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let insts: Rc<RefCell<HashMap<u32, u32>>> = Rc::new(RefCell::new(HashMap::new()));
    let camera = Rc::new(RefCell::new(0u32));
    let ops_base = Rc::new(RefCell::new(0u32));
    let pool = r.mem.alloc(0x100);

    macro_rules! logs {
        ($addr:expr, |$c:ident, $m:ident, $b:ident| $e:expr) => {{
            let l = log.clone();
            #[allow(unused_variables)]
            let $b = ops_base.clone();
            #[allow(unused_variables)]
            r.stub($addr, move |$c, $m| {
                let v = $e;
                l.borrow_mut().push(v);
            });
        }};
    }
    logs!(0x13c5d0, |c, m, b| ScriptCall::SpawnController(c.gpr(5) as u32 - *b.borrow()));
    logs!(0x13d0b8, |c, m, b| ScriptCall::SpawnCamera(c.gpr(5) as u32 - *b.borrow()));
    logs!(0x13d130, |c, m, b| ScriptCall::SpawnEffect(c.gpr(5) as u32 - *b.borrow()));
    logs!(MESSAGE, |c, m, b| ScriptCall::ControllerMessage { controller: c.gpr(4) as u32, p2: c.gpr(5) as u32, p3: c.f(12).to_bits() });
    logs!(SWITCH, |c, m, b| ScriptCall::ControllerSwitch { controller: c.gpr(4) as u32, on: c.gpr(5) as u32 });
    logs!(XBC, |c, m, b| ScriptCall::ControllerXbc(c.gpr(4) as u32));
    logs!(END, |c, m, b| ScriptCall::End);
    logs!(0x11e608, |c, m, b| ScriptCall::PickupPoints { rider: c.gpr(4) as u32, points: c.gpr(5) as u32 });
    logs!(0x13b8c0, |c, m, b| ScriptCall::StartEffect {
        effect: c.gpr(6) as u32,
        object: c.gpr(7) as u32,
        rider: c.gpr(8) as u32,
        ctx_a: ctx(m, c.gpr(9) as u32),
        ctx_b: ctx(m, c.gpr(10) as u32)
    });
    logs!(0x13bc78, |c, m, b| ScriptCall::StartFunction { function: c.gpr(6) as u32, rider: c.gpr(7) as u32, ctx_a: ctx(m, c.gpr(8) as u32), ctx_b: ctx(m, c.gpr(9) as u32) });
    logs!(0x2165c8, |c, m, b| ScriptCall::ScriptSound { object: c.gpr(5) as u32, sound: c.gpr(6) as u32 });
    logs!(0x118f10, |c, m, b| {
        assert_eq!(m.read_u32(c.gpr(4) as u32 + 0x304), 1);
        assert_eq!(c.f(12).to_bits(), 0x42ca_ebe0);
        ScriptCall::ResetRider(c.gpr(4) as u32)
    });
    logs!(0x11e660, |c, m, b| ScriptCall::PickupMultiplier { rider: c.gpr(4) as u32, value: c.f(12).to_bits() });
    logs!(0x11e958, |c, m, b| ScriptCall::PickupMeter { rider: c.gpr(4) as u32, value: c.f(12).to_bits() });
    logs!(0x11e8a0, |c, m, b| ScriptCall::PickupTime { rider: c.gpr(4) as u32, value: c.f(12).to_bits() });
    logs!(0x11e910, |c, m, b| ScriptCall::SpeedBoost { rider: c.gpr(4) as u32, value: c.f(12).to_bits() });
    logs!(0x2340b8, |c, m, b| ScriptCall::SpeechSpeedBoost(c.gpr(5) as u32));
    logs!(0x11e930, |c, m, b| ScriptCall::SpinBoost { rider: c.gpr(4) as u32, value: c.f(12).to_bits() });
    logs!(0x234308, |c, m, b| ScriptCall::SpeechSpinBoost(c.gpr(5) as u32));
    logs!(0x177758, |c, m, b| ScriptCall::CamShake { camera: c.gpr(4) as u32, p3: c.f(12).to_bits(), p2: c.f(13).to_bits() });
    logs!(0x11a098, |c, m, b| ScriptCall::Teleport { rider: c.gpr(4) as u32, object: c.gpr(5) as u32 });
    logs!(0x149030, |c, m, b| ScriptCall::SetRail(c.gpr(4) as u32, c.gpr(5) as u32));
    r.allow(0x13bfd0);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    r.stub(0x2cc808, |_, _| {});
    let q = rand.clone();
    r.stub(0x23d4f8, move |c, _| {
        assert_eq!((c.f(12), c.f(13)), (0.0, 1.0));
        let v = q.borrow_mut().pop_front().expect("rand");
        c.set_f(0, v);
    });
    let i = insts.clone();
    r.stub(0x254a28, move |c, _| {
        assert_eq!(c.gpr(4) as u32, 0x346488);
        let v = i.borrow().get(&(c.gpr(5) as u32)).copied().unwrap_or(0);
        c.set_gpr(2, v as u64);
    });
    let cam = camera.clone();
    r.stub(0x11ca88, move |c, _| c.set_gpr(2, *cam.borrow() as u64));

    // the thread, its vtable, an object with a controller, a rider
    let node = r.mem.alloc(0xf0);
    let vt = r.mem.alloc(0x20);
    r.mem.write_u32(node + 8, vt);
    r.mem.write_u32(vt + 0xc, END);
    let object = r.mem.alloc(0x100);
    let ctrl = r.mem.alloc(0x40);
    let cvt = r.mem.alloc(0x100);
    r.mem.write_u32(ctrl + 8, cvt);
    r.mem.write_u32(cvt + 0xb4, SWITCH);
    r.mem.write_u32(cvt + 0xbc, XBC);
    r.mem.write_u32(cvt + 0xc4, MESSAGE);
    let rider = r.mem.alloc(0x500);
    let header = r.mem.alloc(8);
    let script = r.mem.alloc(0x1000);
    *ops_base.borrow_mut() = script;
    let inst_a = r.mem.alloc(0x100);

    let mut g = Rng(0x0dd1_5ea5_e123_4567);
    let mut ran = [0usize; 3];
    for case in 0..6000 {
        // a script
        let n = 1 + g.next() as usize % 7;
        let mut ops = vec![];
        for _ in 0..n {
            let op = g.pick(&[0, 1, 2, 3, 3, 4, 4, 4, 5, 5, 5, 5, 6, 7, 7, 8, 9, 10, 10, 11, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 21, 22, 23, 24, 25, 26, 26, 27, 30]);
            let mut args: Vec<u32> = (0..3 + g.next() as usize % 3).map(|_| g.next()).collect();
            match op {
                4 => args[0] = g.pick(&[0.0f32, 1.0 / 60.0, 0.02, 0.05, 0.1, -1.0, 0.033_333_335]).to_bits(),
                5 => {
                    args[0] = g.pick(&[0u32, 0, 1, 1, 2, 3, 4, u32::MAX]);
                    args[1] = g.pick(&[0u32, 1, 2]);
                    args[2] = g.pick(&[0.0f32, 0.5, 100.0, 1500.0, 30.0, 2.0, 0.25]).to_bits();
                }
                7 | 24 => args[0] = g.pick(&[0u32, 1, 2]),
                10 | 11 => args[0] = g.pick(&[0u32, 1, 2]),
                21 | 26 => args[0] = g.pick(&[0u32, 3, u32::MAX, 7]),
                _ => {}
            }
            ops.push(Op { op, size: 8 + 4 * args.len() as u32, args });
        }
        let has_obj = g.next() % 4 != 0;
        let has_ctrl = g.next() % 3 != 0;
        let has_rider = g.next() % 5 != 0;
        // ops that read through a missing object or rider without a check are not generated
        if (!has_obj && ops.iter().any(|o| o.op == 9 || (o.op == 5 && o.args[0] == 3))) || (!has_rider && ops.iter().any(|o| o.op == 5 && o.args[0] == 2)) {
            continue;
        }
        let obj = if has_obj { object } else { 0 };
        let rid = if has_rider { rider } else { 0 };
        let mut env = Env {
            ops: ops.clone(),
            rand: VecDeque::new(),
            vel: [g.pick(&[0.0f32, 300.0, -800.0, 1200.0]), g.pick(&[0.0f32, 50.0, -20.0]), g.pick(&[0.0f32, 900.0, 1600.0]), g.pick(&[0.0f32, 0.0, 1.0])],
            x41c: g.pick(&[0, 1, 2]),
            controllers: HashMap::new(),
            instances: HashMap::new(),
            camera: g.pick(&[0u32, 0x55_0000]),
            log: vec![],
        };
        if has_obj && has_ctrl {
            env.controllers.insert(object, ctrl);
        }
        env.instances.insert(1, inst_a);
        if g.next() % 2 == 0 {
            env.instances.insert(2, object);
        }
        let rands: Vec<f32> = (0..64).map(|_| g.pick(&[0.0f32, 0.1, 0.5, 0.9, 0.25, 0.75])).collect();
        env.rand = rands.iter().copied().collect();
        *rand.borrow_mut() = rands.iter().copied().collect();
        *insts.borrow_mut() = env.instances.clone();
        *camera.borrow_mut() = env.camera;
        // the game's memory
        r.mem.write_u32(object + 0xe4, if has_ctrl { ctrl } else { 0 });
        for (k, v) in env.vel.iter().enumerate() {
            r.mem.write_f32(rider + 0x150 + 4 * k as u32, *v);
        }
        r.mem.write_u32(rider + 0x41c, env.x41c as u32);
        r.mem.write_u32(rider + 0x304, 0);
        let mut p = script;
        for o in &ops {
            r.mem.write_u32(p, o.op);
            r.mem.write_u32(p + 4, o.size);
            for (k, a) in o.args.iter().enumerate() {
                r.mem.write_u32(p + 8 + 4 * k as u32, *a);
            }
            p += o.size;
        }
        r.mem.write_u32(header, ops.len() as u32);
        r.mem.write_u32(header + 4, script);
        let ca: Option<Context> = (g.next() % 2 == 0).then(|| [0; 16].map(|_| g.next()));
        let cb: Option<Context> = (g.next() % 2 == 0).then(|| [0; 16].map(|_| g.next()));
        for (o, c, flag) in [(0x50, ca, 0xd0), (0x90, cb, 0xd4)] {
            let c2 = c.unwrap_or([0; 16]);
            for (k, v) in c2.iter().enumerate() {
                r.mem.write_u32(node + o + 4 * k as u32, *v);
            }
            r.mem.write_u32(node + flag, c.is_some() as u32);
        }
        r.mem.write_u32(node + 0x38, 0);
        r.mem.write_u32(node + 0x3c, 0);
        r.mem.write_u32(node + 0x40, 0);
        r.mem.write_u32(node + 0xe0, header);
        r.mem.write_u32(node + 0xe4, obj);
        r.mem.write_u32(node + 0xe8, rid);
        let mut t = Thread::new(ScriptRef::Effect(0), obj, rid, ca, cb);
        for frame in 0..200 {
            log.borrow_mut().clear();
            r.cpu.set_gpr(4, node as u64);
            r.call(0x13be78).unwrap_or_else(|e| panic!("case {case} frame {frame}: {e:?} {ops:?}"));
            env.log.clear();
            let ended = run(&mut t, &mut env);
            assert_eq!(*log.borrow(), env.log, "case {case} frame {frame}: {ops:?}");
            assert_eq!((r.mem.read_u32(node + 0x38) as i32, r.mem.read_u32(node + 0x3c), r.mem.read_u32(node + 0x40)), (t.index, t.offset, t.wait.to_bits()), "case {case} frame {frame}: {ops:?}");
            if ended {
                ran[(frame > 0) as usize + (frame > 3) as usize] += 1;
                break;
            }
        }
    }
    eprintln!("threads ended on the first frame {}, within 4 {}, later {}", ran[0], ran[1], ran[2]);
}

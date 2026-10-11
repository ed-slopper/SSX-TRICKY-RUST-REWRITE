//! The logic controllers (board row E9e) against the game: `cDebounceNode`, `cCounterNode`, `cTimerNode` and
//! `cTrickTriggerNode` built by their own constructors in the function runner (TRICKY_ELF; skipped without it), then
//! random sequences of Update, Kill, Finish and OnMessage run on both sides; the object's flags, the node's fields
//! and the calls out (new threads, the destructor) compared after each. Also the dead and restore nodes'
//! constructors and `cRestoreNode::Kill`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controllers::{apply, spawn, Apply, Controller, CtrlCall, CtrlEnv, Kind, Spawn};

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
    events: HashMap<u32, i32>,
    inside: Option<(i32, i32)>,
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
        self.inside
    }
    fn call(&mut self, c: CtrlCall) {
        self.log.push(c);
    }
}

/// Update, Kill, Finish, OnMessage of each class
const METHODS: [(u32, [u32; 4]); 4] = [
    (2, [0x13fca8, 0x13fcf0, 0x13fd30, 0x13aa48]),
    (6, [0x13b178, 0x13b218, 0x13b278, 0x13b2f0]),
    (8, [0x13fea0, 0x13ffa8, 0x13ffe8, 0x140048]),
    (0x15, [0x147ad8, 0x13a928, 0x13a980, 0x13aa48]),
];

fn read_kind(m: &mut r5900::Mem, node: u32, k: &Kind) -> Kind {
    let w = |m: &mut r5900::Mem, o: u32| m.read_u32(node + o);
    match k {
        Kind::Debounce { .. } => Kind::Debounce { ticks: w(m, 0x2c) as i32 },
        Kind::Counter { .. } => {
            let v = w(m, 0x34);
            Kind::Counter { count: v as u16 as i16, bits: (v >> 16) as u16, life: w(m, 0x38) as i32 }
        }
        Kind::Timer { .. } => Kind::Timer { finish_in: w(m, 0x34) as i32, life: w(m, 0x38) as i32 },
        Kind::TrickTrigger { .. } => Kind::TrickTrigger { x34: w(m, 0x34) as i32, need: w(m, 0x38) as i32, start: w(m, 0x3c) as i32, x40: w(m, 0x40), ticks: w(m, 0x48) as i32, rider: w(m, 0x4c) },
        k => *k,
    }
}

#[test]
fn controllers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<CtrlCall>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x13a980, 0x13fd30, 0x13b278, 0x13ffe8] {
        r.allow(a);
    }
    // the node base: its type at +0x14
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    for a in [0x150670, 0x150818, 0x14fa70] {
        r.stub(a, |_, _| {});
    }
    for a in [0x149688, 0x13b5f8, 0x149d98, 0x147a80] {
        let l = log.clone();
        r.stub(a, move |c, _| {
            assert_eq!(c.gpr(5), 3);
            l.borrow_mut().push(CtrlCall::Delete);
        });
    }
    let e = events.clone();
    r.stub(0x25fe78, move |c, _| {
        assert_eq!(c.gpr(4) as u32, 0x346488);
        let v = e.borrow().get(&(c.gpr(6) as u32)).copied().unwrap_or(-1);
        c.set_gpr(2, v as i64 as u64);
    });
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x13b8c0, move |c, _| {
        assert_eq!((c.gpr(8), c.gpr(9), c.gpr(10)), (0, 0, 0));
        l.borrow_mut().push(CtrlCall::Start(c.gpr(6) as i32, c.gpr(7) as u32));
    });
    let pool = r.mem.alloc(0x100);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    r.stub(0x2cc808, |_, _| {});
    // Obj_HasId(pair, box): 0 when the box is the pair's first object
    r.stub(0x110838, |c, m| {
        let first = m.read_u32(c.gpr(4) as u32);
        c.set_gpr(2, (first != c.gpr(5) as u32) as u64);
    });

    let node = r.mem.alloc(0x100);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let rider = r.mem.alloc(0x6000);
    let pairs = r.mem.alloc(0x40);
    let owners = r.mem.alloc(0x2000);
    let other_rider = r.mem.alloc(0x6000);

    let mut g = Rng(0x1357_9bdf_2468_ace0);
    let mut deleted = 0;
    for case in 0..8000 {
        let (ty, addrs) = METHODS[case % 4];
        let args: [u32; 4] = [
            ty,
            match ty {
                6 => g.pick(&[0u32, 1, 2, 3, 0xffff, 0x10002]),
                0x15 => g.pick(&[0u32, 1, 0xfffe]),
                _ => g.pick(&[0.0f32, 1.0 / 60.0, 0.05, 0.1, -0.1, 2.0]).to_bits(),
            },
            match ty {
                0x15 => g.pick(&[0u32, 500, 1000, 5000, 0xffff_fff0]),
                _ => g.pick(&[0.0f32, 1.0 / 60.0, 0.05, 0.1, -0.05, 1.0]).to_bits(),
            },
            g.pick(&[0.0f32, 0.05, 0.1, 1.0]).to_bits(),
        ];
        for (k, a) in args.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *a);
        }
        let x460 = g.next();
        let points0 = g.pick(&[0i32, 100, 5000, -20]);
        r.mem.write_u32(rider + 0x460, x460);
        r.mem.write_u32(rider + 0x5910, points0 as u32);
        let flags = g.next() & 0xffff_ffff;
        r.mem.write_u32(object + 0xe8, flags);
        for o in [0x2c, 0x34, 0x38, 0x3c, 0x40, 0x48, 0x4c, 0x50] {
            r.mem.write_u32(node + o, 0);
        }
        // construct
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.cpu.set_gpr(8, rider as u64);
        let ctor = match ty {
            2 => 0x13fbf0,
            6 => 0x13b0b8,
            8 => 0x13fdd0,
            _ => 0x1478f0,
        };
        r.call(ctor).unwrap();
        let mut c = Controller::new(object, &args, Some((rider, x460, points0))).unwrap();
        assert_eq!(r.mem.read_u32(node + 0x14), ty);
        assert_eq!(read_kind(&mut r.mem, node, &c.kind), c.kind, "case {case}: constructed from {args:x?}");
        let mut env = Env { flags, events: HashMap::new(), inside: None, log: vec![] };
        for ev in [3u32, 4] {
            if g.next() % 2 == 0 {
                env.events.insert(ev, g.pick(&[0, 5, 17]));
            }
        }
        *events.borrow_mut() = env.events.clone();
        for step in 0..60 {
            // the trick trigger's box: who overlaps it this frame
            if ty == 0x15 {
                let n = g.next() % 3;
                let mut inside = None;
                let mut head = 0;
                for k in (0..n).rev() {
                    // a pair record: the two objects, then each one's next pair
                    let p = pairs + 16 * k;
                    // the other object; its +0x84 is its owner, a rider's owner being type 0x3ef with the rider
                    // 0x82c before it (here the other object's own first word)
                    let other = owners + 0x900 * k;
                    let owner = other + 0x82c;
                    let (first, second) = if g.next() % 2 == 0 { (node + 0x50, other) } else { (other, node + 0x50) };
                    r.mem.write_u32(p, first);
                    r.mem.write_u32(p + 4, second);
                    r.mem.write_u32(p + 8, head);
                    r.mem.write_u32(p + 12, head);
                    head = p;
                    let kind = g.next() % 3;
                    r.mem.write_u32(other + 0x84, owner);
                    r.mem.write_u32(owner + 0x14, if kind == 2 { 0x3e9 } else { 0x3ef });
                    r.mem.write_u32(other, if kind == 0 { rider } else { other_rider });
                    if kind == 0 && inside.is_none() {
                        let x424 = g.pick(&[0i32, 1, 3]);
                        let pts = points0.wrapping_add(g.pick(&[0i32, 499, 500, 1000, 6000]));
                        r.mem.write_u32(rider + 0x424, x424 as u32);
                        r.mem.write_u32(rider + 0x5910, pts as u32);
                        inside = Some((x424, pts));
                    }
                }
                // the box's list head
                r.mem.write_u32(node + 0x50, head);
                env.inside = inside;
                if inside.is_none() {
                    r.mem.write_u32(rider + 0x424, 0);
                }
            }
            log.borrow_mut().clear();
            env.log.clear();
            let what = g.pick(&[0u32, 0, 0, 0, 0, 1, 2, 2, 3, 3, 3]);
            match what {
                0 => {
                    r.cpu.set_gpr(4, node as u64);
                    r.call(addrs[0]).unwrap();
                    c.update(&mut env);
                }
                1 => {
                    r.cpu.set_gpr(4, node as u64);
                    r.call(addrs[1]).unwrap();
                    c.kill(&mut env);
                }
                2 => {
                    let f = g.pick(&[0u32, 1]);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, f as u64);
                    r.call(addrs[2]).unwrap();
                    c.finish(f, &mut env);
                }
                _ => {
                    let v = g.pick(&[1.0f32, 2.0, 3.0, 4.0, 5.0, 10.0, 11.0, 0.5, 0.05]);
                    let k = g.pick(&[1u32, 2, 3, 7, 8, 4]);
                    r.cpu.set_f(12, v);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, k as u64);
                    r.call(addrs[3]).unwrap();
                    c.on_message(v, k, &mut env);
                }
            }
            assert_eq!(*log.borrow(), env.log, "case {case} step {step} ({what}) {c:?}");
            assert_eq!(r.mem.read_u32(object + 0xe8), env.flags, "case {case} step {step} ({what}) flags");
            assert_eq!(read_kind(&mut r.mem, node, &c.kind), c.kind, "case {case} step {step} ({what})");
            if env.log.contains(&CtrlCall::Delete) {
                deleted += 1;
                break;
            }
        }
    }
    eprintln!("{deleted} of 8000 controllers ended");

    // the dead and restore nodes
    for case in 0..2000 {
        let flags = g.next();
        let stop = g.next() % 2;
        r.mem.write_u32(object + 0xe8, flags);
        let mut env = Env { flags, events: HashMap::new(), inside: None, log: vec![] };
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, object as u64);
        if case % 2 == 0 {
            r.cpu.set_gpr(6, stop as u64);
            r.call(0x13af38).unwrap();
            Controller::dead(object, stop != 0, &mut env);
            assert_eq!(r.mem.read_u32(node + 0x14), 5);
        } else {
            r.call(0x13aff8).unwrap();
            let mut c = Controller::restore(object, &mut env);
            assert_eq!(r.mem.read_u32(node + 0x14), 0x3ee);
            assert_eq!(r.mem.read_u32(object + 0xe8), env.flags, "restore {flags:x}");
            let l = log.clone();
            l.borrow_mut().clear();
            r.stub(0x13b060, move |_, _| l.borrow_mut().push(CtrlCall::Delete));
            r.cpu.set_gpr(4, node as u64);
            r.call(0x13a928).unwrap();
            c.kill(&mut env);
            assert_eq!(*log.borrow(), env.log);
        }
        assert_eq!(r.mem.read_u32(object + 0xe8), env.flags, "case {case} flags {flags:x}");
    }
}

#[test]
fn spawn_and_apply_rules() {
    // the decision tables read from `TriggerScript_Op0_SpawnController` and `TriggerScript_ApplyControllerOp`
    assert_eq!(spawn(None, 2, false), vec![Spawn::Create(2)]);
    assert_eq!(spawn(None, 0x15, false), vec![]);
    assert_eq!(spawn(None, 0x15, true), vec![Spawn::Create(0x15)]);
    assert_eq!(spawn(None, 1, true), vec![]);
    assert_eq!(spawn(Some((5, false)), 2, true), vec![]);
    assert_eq!(spawn(Some((2, false)), 2, true), vec![]);
    assert_eq!(spawn(Some((7, true)), 7, true), vec![Spawn::Restart]);
    assert_eq!(spawn(Some((6, false)), 8, true), vec![Spawn::DeleteOld, Spawn::Create(8)]);
    assert_eq!(spawn(Some((6, false)), 0x14, true), vec![Spawn::MeshAnimReplacing, Spawn::DeleteOld]);
    assert_eq!(apply(None, 0), vec![Apply::RestoreFlags]);
    assert_eq!(apply(Some(5), 0), vec![]);
    assert_eq!(apply(Some(8), 2), vec![Apply::DeleteOld, Apply::Dead { stop: false }]);
    assert_eq!(apply(Some(0x3ee), 4), vec![]);
    assert_eq!(apply(Some(0x3ee), 3), vec![Apply::DeleteOld, Apply::Dead { stop: true }]);
    assert_eq!(apply(Some(6), 1), vec![Apply::FinishOld]);
}

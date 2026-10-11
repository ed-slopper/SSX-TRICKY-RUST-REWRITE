//! The falling pieces (board row E9g2) against the game: `cMeshAnimNode` built by its own constructor in the
//! function runner (TRICKY_ELF; skipped without it) on a made-up model of up to 24 parts and a random object matrix,
//! with the random words scripted; the pieces' starting poses (from the game's `Mat4_ToPosQuat`) handed to
//! `tricky_game::debris`, then frames of Update and the odd Kill on both sides, comparing every piece.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::breakables::{BreakCall, BreakEnv, Contact, Neighbour};
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::debris::{Debris, DebrisEnv, Piece};

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
    fx: VecDeque<u32>,
    log: Vec<BreakCall>,
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
        self.log.push(BreakCall::Ctrl(c));
    }
}
impl BreakEnv for Env {
    fn breaks(&mut self, c: BreakCall) {
        self.log.push(c);
    }
    fn neighbour(&self, _: u32) -> Neighbour {
        Neighbour::None
    }
}
impl DebrisEnv for Env {
    fn rand_fx(&mut self) -> u32 {
        self.fx.pop_front().unwrap()
    }
}

fn v4(m: &mut r5900::Mem, a: u32) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| m.read_f32(a + 4 * k))
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}
fn piece_bits(p: &Piece) -> ([u32; 4], [u32; 4], u32, [u32; 4], [u32; 4]) {
    (bits(p.rot), bits(p.axis), p.spin.to_bits(), bits(p.pos), bits(p.vel))
}

#[test]
fn debris_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<BreakCall>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    let fx: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    for a in [0x13a840, 0x13a9f8, 0x13a980, 0x102f50, 0x2504f0, 0x146f40, 0x250c10, 0x102e80] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    r.stub(0x2cc808, |_, _| {});
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
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(BreakCall::Ctrl(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32))));
    let l = log.clone();
    r.stub(0x146ef0, move |_, _| l.borrow_mut().push(BreakCall::Ctrl(CtrlCall::Delete)));
    let l = log.clone();
    r.stub(0x13abf0, move |_, _| l.borrow_mut().push(BreakCall::ReplacedWithDead));

    let node = r.mem.alloc(0x700);
    let object = r.mem.alloc(0x100);
    let model = r.mem.alloc(0x20);
    let parts = r.mem.alloc(0x18 * 24);
    let mats = r.mem.alloc(0x40 * 24);
    let op = r.mem.alloc(0x40);
    let ctx = r.mem.alloc(0x40);
    let mut g = Rng(0xdeb2_1500_1234_8765);
    let mut steps = 0;
    for case in 0..1500 {
        // the object's matrix: a rotation about z and a position
        let (s, c) = g.pick(&[(0.0f32, 1.0f32), (1.0, 0.0), (0.6, 0.8), (-0.8, 0.6)]);
        let rows = [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.pick(&[0.0f32, 500.0]), g.pick(&[0.0f32, -1200.0]), g.pick(&[0.0f32, 300.0]), 1.0]];
        for (i, row) in rows.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                r.mem.write_f32(object + 16 * i as u32 + 4 * k as u32, *v);
            }
        }
        // the model: n parts, each its own matrix (a translation), no parents
        let n = 1 + g.next() % 24;
        r.mem.write_u32(object + 0xc0, model);
        r.mem.write_u32(model + 4, n);
        r.mem.write_u32(model + 8, parts);
        for p in 0..n {
            let at = parts + 0x18 * p;
            let m = mats + 0x40 * p;
            r.mem.write_u32(at, 0xffff_ffff);
            r.mem.write_u32(at + 0x14, m);
            let t = [g.pick(&[0.0f32, 30.0, -45.0]), g.pick(&[0.0f32, 12.0]), g.pick(&[0.0f32, 80.0]), 1.0];
            let id = [[1.0f32, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], t];
            for (i, row) in id.iter().enumerate() {
                for (k, v) in row.iter().enumerate() {
                    r.mem.write_f32(m + 16 * i as u32 + 4 * k as u32, *v);
                }
            }
        }
        let f = |g: &mut Rng, v: &[f32]| g.pick(v).to_bits();
        let mut a = [0u32; 11];
        a[0] = 0x14;
        a[1] = g.pick(&[0u32, 1]);
        a[2] = f(&mut g, &[0.0, 1.0, 0.5, 2.0]);
        a[3] = f(&mut g, &[0.0, 0.1, 0.5, 2.0]);
        let dirless = g.next() % 2 == 0;
        for k in 4..7 {
            a[k] = if dirless { 0 } else { f(&mut g, &[0.0, 1.0, -0.5, 3.0]) };
        }
        for k in 7..10 {
            a[k] = f(&mut g, &[0.0, 100.0, 450.0, 30.0]);
        }
        a[10] = f(&mut g, &[1.0, 0.5, 2.0, 0.0]);
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        let with = g.next() % 3 != 0;
        let c0 = Contact { a: [g.pick(&[0.0f32, 1.0, -0.6]), g.pick(&[0.0f32, 0.8]), g.pick(&[0.0f32, 0.3]), 0.0], b: [0.0; 4], speed: g.pick(&[0.0f32, 900.0, -1500.0]) };
        for k in 0..4 {
            r.mem.write_f32(ctx + 0x10 + 4 * k as u32, c0.a[k]);
        }
        r.mem.write_f32(ctx + 0x30, c0.speed);
        for o in (0x2c..0x700).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let words: Vec<u32> = (0..400).map(|_| g.next() << 8 ^ g.next()).collect();
        *fx.borrow_mut() = words.iter().copied().collect();
        let mut env = Env { flags, events: HashMap::new(), fx: words.iter().copied().collect(), log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, 6);
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.cpu.set_gpr(8, if with { ctx as u64 } else { 0 });
        r.cpu.set_gpr(9, 0);
        r.call(0x1466b8).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        let starts: Vec<([f32; 4], [f32; 4])> = (0..n).map(|i| (v4(&mut r.mem, node + 0x3c0 + 16 * i), v4(&mut r.mem, node + 0x38 + 16 * i))).collect();
        let mut d = Debris::new(object, &a, rows, with.then_some(&c0), &starts, &mut env);
        let read = |m: &mut r5900::Mem, i: u32| {
            let p = Piece { rot: v4(m, node + 0x38 + 16 * i), axis: v4(m, node + 0x1c0 + 16 * i), spin: m.read_f32(node + 0x340 + 4 * i), pos: v4(m, node + 0x3c0 + 16 * i), vel: v4(m, node + 0x540 + 16 * i) };
            piece_bits(&p)
        };
        let check = |r: &mut Runner, d: &Debris, env: &Env, what: &str| {
            let m = &mut r.mem;
            for (i, p) in d.pieces.iter().enumerate() {
                assert_eq!(read(m, i as u32), piece_bits(p), "case {case} {what} piece {i}");
            }
            assert_eq!(
                [m.read_u32(node + 0x34), m.read_u32(node + 0x6c4), m.read_u32(node + 0x6c8), m.read_u32(node + 0x6cc), m.read_u32(object + 0xe8)],
                [d.life as u32, d.stays, d.gravity.to_bits(), d.flying, env.flags],
                "case {case} {what}"
            );
            assert_eq!([bits(v4(m, node + 0x3a0)), bits(v4(m, node + 0x3b0))], [bits(d.spread), bits(d.base)], "case {case} {what}");
        };
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        check(&mut r, &d, &env, "built");
        for step in 0..40 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            let gone = if g.next() % 20 == 0 {
                r.call(0x1474a0).unwrap();
                d.kill(&mut env);
                true
            } else {
                r.call(0x147230).unwrap();
                d.update(&mut env)
            };
            assert_eq!(*log.borrow(), env.log, "case {case} step {step}");
            check(&mut r, &d, &env, &format!("step {step}"));
            steps += 1;
            if gone {
                break;
            }
        }
        assert_eq!(fx.borrow().len(), env.fx.len(), "case {case}: random words used");
    }
    eprintln!("{steps} steps compared");
}

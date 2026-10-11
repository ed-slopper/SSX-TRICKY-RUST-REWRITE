//! The breakables (board row E9g) against the game: `cCrackedNode` and `cFenceNode` built by their own
//! constructors in the function runner (TRICKY_ELF; skipped without it) from random contacts, then random
//! sequences of Update, hits, Kill, Finish and messages on both sides, comparing the nodes, the object's flags and
//! the calls out (new threads, deletion, the texture flip, the fence's shape and its neighbours).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::breakables::{BreakCall, BreakEnv, Contact, Cracked, Fence, Neighbour};
use tricky_game::controllers::{CtrlCall, CtrlEnv};

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
    sides: [Neighbour; 2],
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
    fn neighbour(&self, side: u32) -> Neighbour {
        self.sides[side as usize]
    }
}

fn contact(g: &mut Rng) -> Contact {
    let f = |g: &mut Rng| g.pick(&[0.0f32, 1.0, -1.0, 0.6, -0.8, 0.25]);
    Contact { a: [f(g), f(g), f(g), 0.0], b: [f(g), f(g), f(g), 0.0], speed: g.pick(&[0.0f32, 500.0, 1500.0, -900.0, 3000.0, 120.0]) }
}

fn write_contact(m: &mut r5900::Mem, at: u32, c: &Contact) {
    for k in 0..4 {
        m.write_f32(at + 0x10 + 4 * k as u32, c.a[k]);
        m.write_f32(at + 0x20 + 4 * k as u32, c.b[k]);
    }
    m.write_f32(at + 0x30, c.speed);
}

struct World {
    r: Runner,
    log: Rc<RefCell<Vec<BreakCall>>>,
    events: Rc<RefCell<HashMap<u32, i32>>>,
}

fn world(stub_swing: bool) -> Option<World> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<BreakCall>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x13a980, 0x145808, 0x1444d0, 0x143928] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    r.stub(0x142ac8, |_, _| {});
    r.stub(0x23d3b0, |c, _| c.set_gpr(2, 0x1f0_0000));
    r.stub(0x2cc808, |_, _| {});
    let e = events.clone();
    r.stub(0x25fe78, move |c, _| {
        let v = e.borrow().get(&(c.gpr(6) as u32)).copied().unwrap_or(-1);
        c.set_gpr(2, v as i64 as u64);
    });
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(BreakCall::Ctrl(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32))));
    let l = log.clone();
    r.stub(0x13b8c0, move |c, _| {
        assert_eq!((c.gpr(8), c.gpr(9), c.gpr(10)), (0, 0, 0));
        l.borrow_mut().push(BreakCall::Ctrl(CtrlCall::Start(c.gpr(6) as i32, c.gpr(7) as u32)));
    });
    for a in [0x1496f8, 0x143800] {
        let l = log.clone();
        r.stub(a, move |c, _| {
            assert_eq!(c.gpr(5), 3);
            l.borrow_mut().push(BreakCall::Ctrl(CtrlCall::Delete));
        });
    }
    let l = log.clone();
    r.stub(0x142af0, move |c, _| {
        assert_eq!(c.gpr(6) as i64, -1);
        l.borrow_mut().push(BreakCall::TexFlipInit);
    });
    let l = log.clone();
    r.stub(0x13abf0, move |_, _| l.borrow_mut().push(BreakCall::ReplacedWithDead));
    let l = log.clone();
    r.stub(0x143cd0, move |_, _| l.borrow_mut().push(BreakCall::FenceShape));
    let l = log.clone();
    r.stub(0x143740, move |c, _| l.borrow_mut().push(BreakCall::NewFence { side: c.gpr(8) as u32, swing: c.f(12).to_bits() }));
    if stub_swing {
        let l = log.clone();
        r.stub(0x143b08, move |c, _| l.borrow_mut().push(BreakCall::SwingNeighbour { side: c.gpr(5) as u32, swing: c.f(12).to_bits() }));
    } else {
        r.allow(0x143b08);
    }
    Some(World { r, log, events })
}

#[test]
fn cracked_matches_the_original() {
    let Some(World { mut r, log, events }) = world(true) else { return };
    let node = r.mem.alloc(0x100);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let ctx = r.mem.alloc(0x40);
    let mut g = Rng(0xc0ac_ed00_91a5_5e5e);
    for case in 0..6000 {
        let args = [14u32, g.pick(&[0.0f32, 1.0 / 60.0, 0.1, 3.0, -1.0]).to_bits(), g.pick(&[0.0f32, 10.0, 40.0, 100.0, -5.0]).to_bits()];
        for (k, a) in args.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *a);
        }
        let c0 = contact(&mut g);
        let with = g.next() % 4 != 0;
        write_contact(&mut r.mem, ctx, &c0);
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let mut env = Env { flags, events: HashMap::new(), sides: [Neighbour::None; 2], log: vec![] };
        for ev in [3u32, 4] {
            if g.next() % 2 == 0 {
                env.events.insert(ev, g.pick(&[0, 2, 9]));
            }
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.cpu.set_gpr(8, if with { ctx as u64 } else { 0 });
        r.call(0x145458).unwrap();
        let mut c = Cracked::new(object, &args, with.then_some(&c0), &mut env);
        let read = |m: &mut r5900::Mem| (m.read_u32(node + 0x34) as i32, m.read_u32(node + 0x38) as i32, m.read_u32(node + 0x3c), m.read_u32(object + 0xe8));
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        assert_eq!(read(&mut r.mem), (c.ticks, c.cooldown, c.health.to_bits(), env.flags), "case {case} built");
        for step in 0..40 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            let what = g.pick(&[0u32, 0, 0, 0, 1, 1, 2, 3, 4]);
            let gone = match what {
                0 => {
                    r.call(0x1455a0).unwrap();
                    c.update(&mut env)
                }
                1 => {
                    let h = contact(&mut g);
                    write_contact(&mut r.mem, ctx, &h);
                    r.cpu.set_gpr(6, ctx as u64);
                    r.call(0x1457e8).unwrap();
                    c.hit(Some(&h));
                    false
                }
                2 => {
                    r.call(0x145740).unwrap();
                    c.kill(&mut env)
                }
                3 => {
                    let f = g.next() % 2;
                    r.cpu.set_gpr(5, f as u64);
                    r.call(0x1456c0).unwrap();
                    c.finish(f, &mut env)
                }
                _ => {
                    let v = g.pick(&[0.0f32, 0.5, 2.0, -1.0, 30.0]);
                    let k = g.pick(&[1u32, 2, 7, 8]);
                    r.cpu.set_f(12, v);
                    r.cpu.set_gpr(5, k as u64);
                    r.call(0x1457a8).unwrap();
                    c.on_message(v, k);
                    false
                }
            };
            assert_eq!(*log.borrow(), env.log, "case {case} step {step} ({what})");
            assert_eq!(read(&mut r.mem), (c.ticks, c.cooldown, c.health.to_bits(), env.flags), "case {case} step {step} ({what})");
            if gone {
                break;
            }
        }
    }
}

#[test]
fn fence_matches_the_original() {
    let Some(World { mut r, log, events }) = world(true) else { return };
    let node = r.mem.alloc(0x200);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let ctx = r.mem.alloc(0x40);
    let left = r.mem.alloc(0x100);
    let right = r.mem.alloc(0x100);
    let fence_ctrl = r.mem.alloc(0x40);
    let other_ctrl = r.mem.alloc(0x40);
    r.mem.write_u32(fence_ctrl + 0x14, 0xc);
    r.mem.write_u32(other_ctrl + 0x14, 7);
    let shape: Rc<RefCell<(u32, u32, u32)>> = Rc::new(RefCell::new((0, 0, 0)));
    let s = shape.clone();
    r.stub(0x143ff8, move |c, m| {
        let n = c.gpr(4) as u32;
        let (has, l, rt) = *s.borrow();
        m.write_u32(n + 0x54, has);
        m.write_u32(n + 0x1b0, l);
        m.write_u32(n + 0x1b4, rt);
    });
    let Some(World { r: mut r2, .. }) = world(false) else { return };
    let node2 = r2.mem.alloc(0x200);
    let mut g = Rng(0xfe0c_e5e7_0a1b_2c3d);
    let mut swung = 0;
    for case in 0..6000 {
        // the object's matrix, its neighbours (side 1 at +0x1b0, side 0 at +0x1b4) and their controllers
        let row0 = [g.pick(&[1.0f32, 0.0, 0.6, -2.0]), g.pick(&[0.0f32, 0.8, 1.0]), g.pick(&[0.0f32, 0.5]), g.pick(&[0.0f32, 1.0])];
        for k in 0..4 {
            r.mem.write_f32(object + 4 * k as u32, row0[k]);
        }
        let has_shape = g.next() % 4 != 0;
        let mut sides = [Neighbour::None; 2];
        let mut ptr = [0u32; 2];
        for (side, obj) in [(1usize, left), (0usize, right)] {
            let kind = g.next() % 4;
            if kind == 0 {
                continue;
            }
            ptr[side] = obj;
            let (ctrl, n) = match kind {
                1 => (0, Neighbour::Free),
                2 => (fence_ctrl, Neighbour::Fence),
                _ => (other_ctrl, Neighbour::Other),
            };
            r.mem.write_u32(obj + 0xe4, ctrl);
            if has_shape {
                sides[side] = n;
            }
        }
        *shape.borrow_mut() = (has_shape as u32, ptr[1], ptr[0]);
        let c0 = contact(&mut g);
        write_contact(&mut r.mem, ctx, &c0);
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        for o in (0x2c..0x60).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        let mut env = Env { flags, events: HashMap::new(), sides, log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, 1);
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.cpu.set_gpr(8, ctx as u64);
        r.call(0x143550).unwrap();
        let mut f = Fence::new(object, &c0, has_shape.then_some(row0), &mut env);
        let read = |m: &mut r5900::Mem| [0x34u32, 0x38, 0x3c, 0x40, 0x44].map(|o| m.read_u32(node + o));
        let mine = |f: &Fence| [f.angle.to_bits(), f.size.to_bits(), f.speed.to_bits(), f.pass1 as u32, f.pass0 as u32];
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        assert_eq!(read(&mut r.mem), mine(&f), "case {case} built");
        for step in 0..60 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, node as u64);
            let what = g.pick(&[0u32, 0, 0, 0, 0, 0, 1, 2]);
            let gone = match what {
                0 => {
                    r.call(0x143b68).unwrap();
                    f.update(&mut env)
                }
                1 => {
                    let h = contact(&mut g);
                    write_contact(&mut r.mem, ctx, &h);
                    r.cpu.set_gpr(6, ctx as u64);
                    r.call(0x144428).unwrap();
                    f.hit(&h);
                    false
                }
                _ => {
                    // a neighbour's swing arriving: the real Fence_Swing, in a runner where it is not stubbed
                    let by = g.pick(&[0.3f32, -0.3, 0.5, -0.8]);
                    let side = g.next() % 2;
                    for o in [0x34u32, 0x38, 0x3c, 0x40, 0x44] {
                        let v = r.mem.read_u32(node + o);
                        r2.mem.write_u32(node2 + o, v);
                    }
                    r2.cpu.set_f(12, by);
                    r2.cpu.set_gpr(4, node2 as u64);
                    r2.cpu.set_gpr(5, side as u64);
                    r2.call(0x143b08).unwrap();
                    for o in [0x34u32, 0x38, 0x3c, 0x40, 0x44] {
                        let v = r2.mem.read_u32(node2 + o);
                        r.mem.write_u32(node + o, v);
                    }
                    f.swing(by, side);
                    false
                }
            };
            assert_eq!(*log.borrow(), env.log, "case {case} step {step} ({what})");
            assert_eq!(read(&mut r.mem), mine(&f), "case {case} step {step} ({what})");
            assert_eq!(r.mem.read_u32(object + 0xe8), env.flags, "case {case} step {step} flags");
            swung += env.log.iter().filter(|c| matches!(c, BreakCall::NewFence { .. } | BreakCall::SwingNeighbour { .. })).count();
            if gone {
                break;
            }
        }
    }
    eprintln!("{swung} swings passed on");
}

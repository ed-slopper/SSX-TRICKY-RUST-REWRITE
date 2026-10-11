//! The object animation controllers (board row E9i2) against the game: `cFlagNode`, `cAnimObjectNode`,
//! `cAnimDeltaNode`, `cAnimComboNode` and `cAnimTexFlipNode` built by their own constructors in the function runner
//! (TRICKY_ELF; skipped without it) on made-up models, with both random streams scripted, then random sequences of
//! Update, contacts, messages and Kill on both sides, comparing the fields, the object's flags and the calls out.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::objanim::{AnimCombo, AnimDelta, AnimObject, AnimTexFlip, Flag, ObjAnimCall, ObjAnimEnv};
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
    Anim(ObjAnimCall),
    Contact,
}

struct Env {
    flags: u32,
    events: HashMap<u32, i32>,
    fx: VecDeque<u32>,
    game: VecDeque<u32>,
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
    fn contact_script(&mut self) {
        self.log.push(Ev::Contact);
    }
}
impl ObjAnimEnv for Env {
    fn rand_game(&mut self) -> u32 {
        self.game.pop_front().unwrap()
    }
    fn anim(&mut self, c: ObjAnimCall) {
        if c != ObjAnimCall::ComboCopy {
            self.log.push(Ev::Anim(c));
        }
    }
}

enum Node {
    F(Flag),
    O(AnimObject),
    D(AnimDelta),
    C(AnimCombo),
    T(AnimTexFlip),
}

fn read_obj(m: &mut r5900::Mem, at: u32) -> [u32; 10] {
    let mut h = [0u8; 2];
    m.read(at + 8, &mut h);
    [u16::from_le_bytes(h) as u32, m.read_u32(at + 0xc), m.read_u32(at + 0x10), m.read_u32(at + 0x14), m.read_u32(at + 0x18), m.read_u32(at + 0x1c), m.read_u32(at + 0x20), m.read_u32(at + 0x24), m.read_u32(at + 0x28), m.read_u32(at + 0x2c)]
}
fn obj_bits(a: &AnimObject) -> [u32; 10] {
    [a.looping as u16 as u32, a.playing, a.done, a.delay as u32, a.rearm as u32, a.cooldown as u32, a.rate.to_bits(), a.time.to_bits(), a.start.to_bits(), a.end.to_bits()]
}

#[test]
fn object_animations_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    let fx: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let game: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x13a980, 0x142af0, 0x142c78, 0x23d4f8, 0x23d560, 0x199030, 0x198810, 0x199058, 0x199850, 0x144938, 0x13b7c8] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    for a in [0x142ac8, 0x1cb150, 0x1cb960, 0x1cb0d8, 0xf0_0100, 0xf0_0110] {
        r.stub(a, |_, _| {});
    }
    r.stub(0x2cc808, |c, m| {
        let v = vec![c.gpr(5) as u8; c.gpr(6) as usize];
        m.write(c.gpr(4) as u32, &v).unwrap();
    });
    let pool = r.mem.alloc(0x4000);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    let q = fx.clone();
    r.stub(0x23d4a8, move |c, _| {
        let v = q.borrow_mut().pop_front().expect("fx");
        c.set_gpr(2, v as u64);
    });
    let q = game.clone();
    r.stub(0x23d4d8, move |c, _| {
        let v = q.borrow_mut().pop_front().expect("game");
        c.set_gpr(2, v as u64);
    });
    let e = events.clone();
    r.stub(0x25fe78, move |c, _| {
        let v = e.borrow().get(&(c.gpr(6) as u32)).copied().unwrap_or(-1);
        c.set_gpr(2, v as i64 as u64);
    });
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(Ev::Ctrl(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32))));
    let l = log.clone();
    r.stub(0x13b8c0, move |c, _| l.borrow_mut().push(Ev::Ctrl(CtrlCall::Start(c.gpr(6) as i32, c.gpr(7) as u32))));
    for a in [0x199258u32, 0x19a478] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(Ev::Anim(ObjAnimCall::ApplyFrames)));
    }
    let l = log.clone();
    r.stub(0x144ec8, move |_, _| l.borrow_mut().push(Ev::Anim(ObjAnimCall::FlagStrip)));
    for a in [0x198da8, 0x19ad78, 0x19a1a8, 0x19ac48, 0x1448c8] {
        let l = log.clone();
        r.stub(a, move |c, _| {
            assert_eq!(c.gpr(5), 3);
            l.borrow_mut().push(Ev::Ctrl(CtrlCall::Delete));
        });
    }
    // the renderer the flag asks for a strip buffer and the model's outline
    let gapp = r.mem.alloc(0x800);
    let rend = r.mem.alloc(0x20);
    let rvt = r.mem.alloc(0x300);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x724, rend);
    r.mem.write_u32(rend + 0x10, rvt);
    r.mem.write_u32(rvt + 0x22c, 0xf0_0100);
    r.mem.write_u32(rvt + 0x2b4, 0xf0_0110);

    let node = r.mem.alloc(0x400);
    let op = r.mem.alloc(0x60);
    let ctx = r.mem.alloc(0x40);
    let object = r.mem.alloc(0x100);
    let model = r.mem.alloc(0x40);
    let parts = r.mem.alloc(0x18 * 4);
    let flipparts = r.mem.alloc(0x10);
    let fpart = r.mem.alloc(0x50);
    let table = r.mem.alloc(0x40);
    let mut g = Rng(0x0b1e_c7a1_1e11_5eed);
    let mut steps = 0;
    for case in 0..6000 {
        let kind = case % 5; // flag, object, delta, combo, texflip
        r.mem.write_u32(object + 0xc0, model);
        let np = 1 + g.next() % 4;
        r.mem.write_u32(model + 4, np);
        r.mem.write_u32(model + 8, parts);
        for p in 0..np {
            r.mem.write_u32(parts + 0x18 * p + 0x10, g.next() % 2);
            r.mem.write_u32(parts + 0x18 * p + 4, 0);
        }
        let model_end = g.pick(&[30.0f32, 60.0, 90.0, 12.0]);
        r.mem.write_f32(model + 0x14, model_end);
        let dims = [g.pick(&[0.5f32, 2.0, 8.0]), 3.0, 4.0];
        for k in 0..3 {
            r.mem.write_f32(model + 0x18 + 4 * k as u32, dims[k]);
        }
        let count = g.pick(&[1i32, 2, 3, 6]);
        r.mem.write_u32(object + 0xf0, flipparts);
        r.mem.write_u32(flipparts, 1);
        r.mem.write_u32(flipparts + 4, fpart);
        r.mem.write_u32(fpart + 0x44, table);
        r.mem.write_u32(table, count as u32);
        let mut a = [0u32; 13];
        a[0] = [13u32, 0x100, 0x101, 0x102, 0x103][kind];
        let fl = |g: &mut Rng, v: &[f32]| g.pick(v).to_bits();
        if kind == 0 {
            a[1] = g.next() % 2;
            a[2] = fl(&mut g, &[0.0, 1.0, 2.5, -60.0]);
            a[3] = fl(&mut g, &[0.0, 10.0, 30.0, -1.0]);
            a[4] = fl(&mut g, &[0.0, 0.05, 1.0, -1.0]);
        } else {
            a[1] = g.pick(&[0u32, 1, 2, 0x1_0002]);
            a[2] = fl(&mut g, &[0.0, 5.0, -3.0, 20.0]);
            a[3] = fl(&mut g, &[-1.0, 30.0, 45.0, 10.0]);
            a[4] = fl(&mut g, &[30.0, 15.0, 60.0, 0.0, 300.0]);
            a[5] = fl(&mut g, &[0.0, 0.0, 5.0]);
            a[6] = fl(&mut g, &[0.0, 0.05, 1.0, -1.0]);
            a[7] = g.next() % 2;
            a[8] = g.pick(&[0u32, 4, 1]);
            a[9] = if kind == 4 { g.pick(&[0u32, 1, 0xffff_ffff, 0xffff_fffe]) } else { fl(&mut g, &[-1.0, 5.0, 25.0]) };
            a[10] = fl(&mut g, &[0.0, 12.0, 60.0, 90.0, -1.0]);
            a[11] = fl(&mut g, &[0.0, 30.0, 60.0]);
            a[12] = g.pick(&[0u32, 1, 0x8000_0001]);
        }
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        for o in (0x8..0x400).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let fxw: Vec<u32> = (0..300).map(|_| g.next() << 8 ^ g.next()).collect();
        let gw: Vec<u32> = (0..300).map(|_| g.next() << 8 ^ g.next()).collect();
        *fx.borrow_mut() = fxw.iter().copied().collect();
        *game.borrow_mut() = gw.iter().copied().collect();
        let mut env = Env { flags, events: HashMap::new(), fx: fxw.iter().copied().collect(), game: gw.iter().copied().collect(), log: vec![] };
        for ev in [1u32, 3] {
            if g.next() % 2 == 0 {
                env.events.insert(ev, g.pick(&[0, 3]));
            }
        }
        *events.borrow_mut() = env.events.clone();
        let armed = g.next() % 2 == 0;
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        let mut n = if kind == 0 {
            r.cpu.set_gpr(6, object as u64);
            r.cpu.set_gpr(7, op as u64);
            r.call(0x144778).unwrap();
            Node::F(Flag::new(object, &a, dims, &mut env))
        } else if kind == 1 {
            r.cpu.set_gpr(6, 0x100);
            r.cpu.set_gpr(7, object as u64);
            r.cpu.set_gpr(8, op as u64);
            r.cpu.set_gpr(9, if armed { ctx as u64 } else { 0 });
            r.cpu.set_gpr(10, 1);
            r.call(0x198810).unwrap();
            Node::O(AnimObject::new(object, &a, model_end, armed, true, &mut env))
        } else {
            r.cpu.set_gpr(6, object as u64);
            r.cpu.set_gpr(7, op as u64);
            r.cpu.set_gpr(8, if armed { ctx as u64 } else { 0 });
            let addr = [0, 0, 0x199d10, 0x199f20, 0x19a5a8][kind];
            r.call(addr).unwrap_or_else(|e| panic!("case {case} ctor {addr:#x}: {e:?}"));
            match kind {
                2 => Node::D(AnimDelta::new(object, &a, model_end, armed, &mut env)),
                3 => Node::C(AnimCombo::new(object, &a, model_end, armed, &mut env)),
                _ => Node::T(AnimTexFlip::new(object, &a, model_end, armed, count, &mut env)),
            }
        };
        let check = |r: &mut Runner, n: &Node, env: &Env, what: &str| {
            let m = &mut r.mem;
            match n {
                Node::F(f) => {
                    let got = [0x34u32, 0x38, 0x3c, 0x40, 0x44, 0x190, 0x194, 0x198].map(|o| m.read_u32(node + o));
                    let want = [f.life as u32, f.mirror, f.rate.to_bits(), f.size.to_bits(), f.phase.to_bits(), f.dims[0].to_bits(), f.dims[1].to_bits(), f.dims[2].to_bits()];
                    assert_eq!(got, want, "case {case} {what}");
                }
                Node::O(o) => assert_eq!(read_obj(m, node), obj_bits(o), "case {case} {what}"),
                Node::D(d) => {
                    assert_eq!(read_obj(m, node), obj_bits(&d.anim), "case {case} {what}");
                    assert_eq!(m.read_u32(node + 0x68), d.left.to_bits(), "case {case} {what}");
                }
                Node::C(c) => {
                    assert_eq!(read_obj(m, node + 0x1c), obj_bits(&c.anim), "case {case} {what}");
                    let mut b = [0u8; 4];
                    m.read(node + 0x18, &mut b);
                    assert_eq!(
                        ([0x8u32, 0xc, 0x10, 0x14].map(|o| m.read_u32(node + o)), b),
                        ([c.rate2.to_bits(), c.time2.to_bits(), c.start2.to_bits(), c.end2.to_bits()], [c.second, c.stopped, c.stop_at_end, c.sign]),
                        "case {case} {what}"
                    );
                }
                Node::T(t) => {
                    assert_eq!(read_obj(m, node + 0x18), obj_bits(&t.anim), "case {case} {what}");
                    assert_eq!([8u32, 0xc, 0x10, 0x14, 0x84].map(|o| m.read_u32(node + o)), [t.acc.to_bits(), t.rate.to_bits(), t.frame as u32, t.random, t.tex.frame as u32], "case {case} {what}");
                }
            }
            assert_eq!(m.read_u32(object + 0xe8), env.flags, "case {case} {what} flags");
        };
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        check(&mut r, &n, &env, "built");
        for step in 0..80 {
            log.borrow_mut().clear();
            env.log.clear();
            let what = g.pick(&[0u32, 0, 0, 0, 0, 0, 1, 2, 2, 3]);
            let gone;
            // the object part's address, the type's update / message addresses
            let (base, upd) = match &n {
                Node::F(_) => (node, 0x144e48),
                Node::O(_) => (node, 0x199058),
                Node::D(_) => (node, 0x199db8),
                Node::C(_) => (node + 0x1c, 0x19a218),
                Node::T(_) => (node + 0x18, 0x19a778),
            };
            match what {
                0 => {
                    r.cpu.set_gpr(4, node as u64);
                    r.call(upd).unwrap();
                    gone = match &mut n {
                        Node::F(f) => f.update(&mut env),
                        Node::O(o) => o.update(&mut env),
                        Node::D(d) => d.update(&mut env),
                        Node::C(c) => c.update(&mut env),
                        Node::T(t) => t.update(&mut env),
                    };
                }
                1 => {
                    let o = match &mut n {
                        Node::F(_) => continue,
                        Node::O(o) => o,
                        Node::D(d) => &mut d.anim,
                        Node::C(c) => &mut c.anim,
                        Node::T(t) => &mut t.anim,
                    };
                    r.cpu.set_gpr(4, base as u64);
                    r.cpu.set_gpr(5, 0x77);
                    r.cpu.set_gpr(6, 0);
                    r.cpu.set_gpr(7, 0);
                    r.call(0x198eb8).unwrap();
                    o.on_contact(&mut env);
                    gone = false;
                }
                2 => {
                    let v = g.pick(&[0.0f32, 0.5, 2.0, 10.0, 30.0, 45.0, -1.0]);
                    let k = g.pick(&[1u32, 2, 3, 4, 7, 8]);
                    r.cpu.set_f(12, v);
                    r.cpu.set_gpr(5, k as u64);
                    match &mut n {
                        Node::F(_) => continue,
                        Node::O(o) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x199850).unwrap();
                            o.on_message(v, k, &mut env);
                        }
                        Node::D(d) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x199eb0).unwrap();
                            d.on_message(v, k, &mut env);
                        }
                        Node::C(c) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x19a2d0).unwrap();
                            c.on_message(v, k, &mut env);
                        }
                        Node::T(t) => {
                            r.cpu.set_gpr(4, base as u64);
                            r.call(0x199850).unwrap();
                            t.anim.on_message(v, k, &mut env);
                        }
                    }
                    gone = false;
                }
                _ => {
                    match &mut n {
                        Node::F(_) => continue,
                        Node::O(o) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x199200).unwrap();
                            o.kill(&mut env);
                        }
                        Node::D(d) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x199200).unwrap();
                            d.anim.kill(&mut env);
                        }
                        Node::C(c) => {
                            r.cpu.set_gpr(4, node as u64);
                            r.call(0x19a390).unwrap();
                            c.kill(&mut env);
                        }
                        Node::T(t) => {
                            r.cpu.set_gpr(4, base as u64);
                            r.call(0x199200).unwrap();
                            t.anim.kill(&mut env);
                        }
                    }
                    gone = env.log.contains(&Ev::Ctrl(CtrlCall::Delete));
                }
            }
            assert_eq!(*log.borrow(), env.log, "case {case} kind {kind} step {step} ({what})");
            check(&mut r, &n, &env, &format!("kind {kind} step {step} ({what})"));
            steps += 1;
            if gone {
                break;
            }
        }
        assert_eq!((fx.borrow().len(), game.borrow().len()), (env.fx.len(), env.game.len()), "case {case}: random words used");
    }
    eprintln!("{steps} steps compared");
}

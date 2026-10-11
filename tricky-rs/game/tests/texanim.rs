//! The texture animation controllers (board row E9i) against the game: `cUVScrollNode`, `cTexFlipNode` and
//! `cUVScrollTexFlipNode` built by their own constructors in the function runner (TRICKY_ELF; skipped without it) on
//! made-up models (parts with vertex counts and flip-book frame tables), with the random words scripted; then random
//! sequences of Update, contacts and messages on both sides, comparing every field, the object's flags, the
//! scrolled-model registry (0x31d938) and the calls out.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::texanim::{AnimEnv, Flip, TexFlipNode, UvRegistry, UvScroll, UvScrollFlip};

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
    Contact,
}

struct Env {
    flags: u32,
    events: HashMap<u32, i32>,
    rand: VecDeque<u32>,
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
        self.rand.pop_front().unwrap()
    }
    fn contact_script(&mut self) {
        self.log.push(Ev::Contact);
    }
}

fn read_flip(m: &mut r5900::Mem, at: [u32; 8]) -> [u32; 8] {
    at.map(|a| m.read_u32(a))
}

fn flip_bits(f: &Flip) -> [u32; 8] {
    [f.mode as u32, f.backwards, f.frame as u32, f.acc.to_bits(), f.rate.to_bits(), f.base_rate.to_bits(), f.random, f.tex.frame as u32]
}

fn read_scroll(m: &mut r5900::Mem, n: u32) -> [u32; 14] {
    [0x34u32, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x4c, 0x54, 0x58, 0x5c, 0x60, 0x64, 0x68, 0x28].map(|o| m.read_u32(n + o))
}

fn scroll_bits(s: &UvScroll) -> [u32; 14] {
    [
        s.mode as u32,
        s.life as u32,
        s.u.to_bits(),
        s.v.to_bits(),
        s.clock.to_bits(),
        s.moving,
        s.cooldown as u32,
        s.du.to_bits(),
        s.dv.to_bits(),
        s.run.to_bits(),
        s.pause.to_bits(),
        s.verts as u32,
        s.slot as u32,
        s.object,
    ]
}

#[test]
fn texture_animations_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
    let rand: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    for a in [0x13a840, 0x13a9f8, 0x13aa48, 0x13a980, 0x142af0, 0x142c78, 0x1421a8, 0x142220, 0x141ff0, 0x1422e0] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    r.stub(0x142ac8, |_, _| {});
    // EAMem_Set(p, byte, n)
    r.stub(0x2cc808, |c, m| {
        let v = vec![c.gpr(5) as u8; c.gpr(6) as usize];
        m.write(c.gpr(4) as u32, &v).unwrap();
    });
    let q = rand.clone();
    r.stub(0x23d4a8, move |c, _| {
        let v = q.borrow_mut().pop_front().expect("rand");
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
    r.stub(0x13aaf8, move |_, _| l.borrow_mut().push(Ev::Contact));
    for a in [0x142128, 0x149938, 0x149868] {
        let l = log.clone();
        r.stub(a, move |c, _| {
            assert_eq!(c.gpr(5), 3);
            l.borrow_mut().push(Ev::Ctrl(CtrlCall::Delete));
        });
    }

    let node = r.mem.alloc(0x400);
    let op = r.mem.alloc(0x60);
    let objects: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x100)).collect();
    let models: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x40)).collect();
    let parts = r.mem.alloc(0x18 * 4);
    let verts = r.mem.alloc(0x40 * 4);
    let flipparts = r.mem.alloc(0x40);
    let tables = r.mem.alloc(0x100);
    let fparts: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x50)).collect();
    let mut reg = UvRegistry::default();
    r.mem.write(0x31d938, &[0u8; 128 * 12]).unwrap();
    let mut g = Rng(0x7e1f_11b0_0c5c_7011);
    let mut flips = 0;
    for case in 0..6000 {
        let kind = case % 3; // 0 scroll, 1 flip, 2 both
        let oi = g.next() as usize % 3;
        let object = objects[oi];
        let model = models[oi];
        // the model: parts with vertex counts (scroll) and frame tables (flip)
        r.mem.write_u32(object + 0xc0, model);
        let np = 1 + g.next() % 4;
        r.mem.write_u32(model + 4, np);
        r.mem.write_u32(model + 8, parts);
        let mut vsum = 0i32;
        for p in 0..np {
            let has = g.next() % 4 != 0;
            let n = g.pick(&[3i32, 12, 40, 0]);
            r.mem.write_u32(verts + 0x40 * p + 0x20, n as u32);
            r.mem.write_u32(parts + 0x18 * p + 4, if has { verts + 0x40 * p } else { 0 });
            if has {
                vsum += n;
            }
        }
        let count = g.pick(&[1i32, 2, 3, 4, 8, 12]);
        let nf = 1 + g.next() % 3;
        r.mem.write_u32(object + 0xf0, flipparts);
        r.mem.write_u32(flipparts, nf);
        for p in 0..nf {
            let fp = fparts[p as usize];
            r.mem.write_u32(flipparts + 4 + 4 * p, fp);
            r.mem.write_u32(fp + 0x40, 0x99);
            r.mem.write_u32(fp + 0x44, if p == 0 { tables } else { 0 });
        }
        r.mem.write_u32(tables, count as u32);
        for k in 0..count as u32 {
            r.mem.write_u32(tables + 4 + 4 * k, 100 + k);
        }
        // the op
        let mut a = [0u32; 11];
        a[0] = [10, 11, 19][kind];
        let small = |g: &mut Rng| g.pick(&[0.0f32, 0.01, -0.02, 0.25, -1.0, 0.5]).to_bits();
        let secs = |g: &mut Rng| g.pick(&[0.0f32, 1.0 / 60.0, 0.05, 0.25, 1.0, -0.5]).to_bits();
        if kind == 1 {
            a[1] = g.pick(&[0u32, 1, 3, 0xffff_ffff, 0xffff_fffe, 20]);
            a[2] = g.next() % 2;
            a[3] = g.pick(&[0.0f32, 10.0, 30.0, 60.0, 120.0]).to_bits();
            a[4] = secs(&mut g);
            a[5] = g.pick(&[0u32, 1, 2, 3]);
        } else {
            a[1] = g.pick(&[0u32, 1, 2, 3]);
            a[2] = small(&mut g);
            a[3] = small(&mut g);
            a[4] = secs(&mut g);
            a[5] = secs(&mut g);
            a[6] = secs(&mut g);
            a[7] = g.pick(&[0u32, 1, 3, 0xffff_ffff, 0xffff_fffe]);
            a[8] = g.pick(&[0.0f32, 10.0, 30.0, 120.0]).to_bits();
            a[9] = g.pick(&[0u32, 1, 2]);
            a[10] = g.next() % 2;
        }
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        for o in (0x2c..0x400).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        let flags = g.next();
        r.mem.write_u32(object + 0xe8, flags);
        let rands: Vec<u32> = (0..200).map(|_| g.next() << 8 ^ g.next()).collect();
        *rand.borrow_mut() = rands.iter().copied().collect();
        let mut env = Env { flags, events: HashMap::new(), rand: rands.iter().copied().collect(), log: vec![] };
        if g.next() % 2 == 0 {
            env.events.insert(3, 4);
        }
        *events.borrow_mut() = env.events.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        enum Node {
            S(UvScroll),
            F(TexFlipNode),
            B(UvScrollFlip),
        }
        let mut n = match kind {
            0 => {
                r.cpu.set_gpr(6, 10);
                r.cpu.set_gpr(7, object as u64);
                r.cpu.set_gpr(8, op as u64);
                r.call(0x141ff0).unwrap();
                Node::S(UvScroll::new(object, &a, vsum, model, &mut reg, &mut env))
            }
            1 => {
                r.cpu.set_gpr(6, object as u64);
                r.cpu.set_gpr(7, op as u64);
                r.call(0x142e90).unwrap();
                Node::F(TexFlipNode::new(object, &a, count, &mut env))
            }
            _ => {
                r.cpu.set_gpr(6, object as u64);
                r.cpu.set_gpr(7, op as u64);
                r.call(0x1460f0).unwrap();
                Node::B(UvScrollFlip::new(object, &a, vsum, model, count, &mut reg, &mut env))
            }
        };
        let check = |r: &mut Runner, n: &Node, env: &Env, what: &str| {
            let m = &mut r.mem;
            match n {
                Node::S(s) => assert_eq!(read_scroll(m, node), scroll_bits(s), "case {case} {what}"),
                Node::F(f) => {
                    let got = read_flip(m, [0x34, 0x4c, 0x48, 0x3c, 0x40, 0x44, 0x54, 0x5c].map(|o| node + o));
                    assert_eq!(got, flip_bits(&f.flip), "case {case} {what}");
                    assert_eq!([m.read_u32(node + 0x38), m.read_u32(node + 0x50), m.read_u32(node + 0x2d0), m.read_u32(node + 0x64)], [f.life as u32, f.cooldown as u32, f.shown, f.flip.tex.count as u32], "case {case} {what}");
                }
                Node::B(b) => {
                    let got = read_flip(m, [8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x94].map(|o| node + o));
                    assert_eq!(got, flip_bits(&b.flip), "case {case} {what}");
                    assert_eq!(read_scroll(m, node + 0x24), scroll_bits(&b.scroll), "case {case} {what}");
                }
            }
            assert_eq!(m.read_u32(object + 0xe8), env.flags, "case {case} {what} flags");
            for (i, e) in reg.0.iter().enumerate() {
                let at = 0x31d938 + 12 * i as u32;
                assert_eq!([m.read_u32(at), m.read_u32(at + 4)], [e[0], e[1]], "case {case} {what} registry {i}");
            }
        };
        assert_eq!(*log.borrow(), env.log, "case {case} built");
        check(&mut r, &n, &env, "built");
        for step in 0..50 {
            log.borrow_mut().clear();
            env.log.clear();
            r.cpu.set_gpr(4, match &n {
                Node::B(_) => node as u64,
                _ => node as u64,
            });
            let what = g.pick(&[0u32, 0, 0, 0, 0, 0, 1, 2]);
            let mut gone = false;
            match (&mut n, what) {
                (Node::S(s), 0) => {
                    r.call(0x1422e0).unwrap();
                    s.update(&mut env);
                }
                (Node::F(f), 0) => {
                    r.call(0x1430b8).unwrap();
                    f.update(&mut env);
                }
                (Node::B(b), 0) => {
                    r.call(0x1462d0).unwrap();
                    b.update(&mut env);
                }
                (Node::S(s), 1) => {
                    r.call(0x142690).unwrap();
                    s.on_contact(&mut env);
                }
                (Node::F(f), 1) => {
                    r.call(0x143378).unwrap();
                    f.on_contact(&mut env);
                }
                (Node::S(s), 2) => {
                    let v = g.pick(&[0.0f32, 0.5, -0.5, 2.0, 5.0, 61.0, -3.0, 8.5]);
                    let k = g.pick(&[1u32, 2, 3, 4, 5, 6, 7, 8]);
                    r.cpu.set_f(12, v);
                    r.cpu.set_gpr(5, k as u64);
                    r.call(0x142948).unwrap();
                    s.on_message(v, k, &mut env);
                }
                (Node::F(f), 2) => {
                    let v = g.pick(&[0.0f32, 0.5, 2.0, 3.0, 9.0, 0.25, 1.0]);
                    let k = g.pick(&[1u32, 2, 3, 4, 7, 8]);
                    r.cpu.set_f(12, v);
                    r.cpu.set_gpr(5, k as u64);
                    r.call(0x1433d8).unwrap();
                    f.on_message(v, k, &mut env);
                }
                _ => continue,
            }
            assert_eq!(*log.borrow(), env.log, "case {case} step {step} ({what})");
            check(&mut r, &n, &env, &format!("step {step} ({what})"));
            if env.log.contains(&Ev::Ctrl(CtrlCall::Delete)) {
                gone = true;
            }
            flips += 1;
            if gone {
                break;
            }
        }
        assert_eq!(rand.borrow().len(), env.rand.len(), "case {case}: random words used");
    }
    eprintln!("{flips} steps compared");
}

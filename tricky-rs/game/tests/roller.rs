//! The knockable objects' bodies (board row E8c2) against the game in the function runner (TRICKY_ELF,
//! TRICKY_SSF_DIR; skipped without them): every course's sphere trees loaded as tests/sphtree_shape.rs loads them,
//! roller nodes made over them with random bodies, then `RollerNode_UpdateTransform`, `FUN_0013e6a0`,
//! `Mat4_FromPosQuat`, `RollerNode_Collide` (the world's answers scripted: `World_RayCast` and
//! `World_QueryInstances` stubbed, the shapes they are handed compared), the whole `cRollerNode::Update`,
//! `Roller_OnHit`, the constructor's pushes (`FUN_0013dd40`, `FUN_0013dbe8`) and `Roller_CheckCollision` on two
//! rollers near each other, against `tricky_game::roller`; every field, call and argument bit for bit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::ssf::Ssf;
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::rigidbody::RigidBody;
use tricky_game::orient::mat4_to_pos_quat;
use tricky_game::roller::{mat4_from_pos_quat, pair, RollerBody, RollerEnv};
use tricky_game::sphtree::{rle_decode, SphereTree, WorldSphTree};

type V4 = [f32; 4];

const COURSES: [&str; 11] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska"];

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
    fn f(&mut self, k: f32) -> f32 {
        ((self.next() % 2001) as f32 - 1000.0) / 1000.0 * k
    }
    fn v(&mut self, k: f32) -> V4 {
        [self.f(k), self.f(k), self.f(k), self.pick(&[0.0f32, 0.0, 1.0])]
    }
    fn unit(&mut self) -> V4 {
        let v = [self.f(1.0), self.f(1.0), self.f(1.0) + 0.001];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l, self.pick(&[0.0f32, 0.0, 1.0])]
    }
}

fn w4(m: &mut Mem, a: u32, v: V4) {
    for k in 0..4 {
        m.write_f32(a + 4 * k as u32, v[k]);
    }
}
fn r4(m: &mut Mem, a: u32) -> V4 {
    [0, 1, 2, 3].map(|k| m.read_f32(a + 4 * k))
}
fn b4(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Cast(u32, [u32; 4], [u32; 4], [u32; 4]),
    Box([u32; 4], [u32; 4]),
    Script(i32, u32),
    Sfx(u32, u32),
    Ctrl(CtrlCall),
}

#[derive(Default)]
struct Env {
    ray: VecDeque<(f32, V4, V4)>,
    query: VecDeque<(f32, V4, V4)>,
    events: HashMap<u32, i32>,
    log: Vec<Ev>,
    flags: u32,
}

fn shape_ev(kind: u32, q: &WorldSphTree) -> Ev {
    Ev::Cast(kind, b4(q.hi), b4(q.lo), b4(q.mid))
}

impl RollerEnv for Env {
    fn ray_cast(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        self.log.push(shape_ev(0, q));
        self.ray.pop_front().unwrap()
    }
    fn query_instances(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        self.log.push(shape_ev(1, q));
        self.query.pop_front().unwrap()
    }
    fn move_box(&mut self, lo: V4, hi: V4) {
        self.log.push(Ev::Box(b4(lo), b4(hi)));
    }
    fn event_script(&self, object: u32, event: u32) -> i32 {
        if event == 1 {
            self.events.get(&object).copied().unwrap_or(-1)
        } else {
            self.events.get(&(object + 0x1000 * event)).copied().unwrap_or(-1)
        }
    }
    fn start_script(&mut self, s: i32, o: u32) {
        self.log.push(Ev::Script(s, o));
    }
    fn sfx(&mut self, r: u32, o: u32) {
        self.log.push(Ev::Sfx(r, o));
    }
}

impl CtrlEnv for Env {
    fn flags(&mut self, _: u32) -> &mut u32 {
        &mut self.flags
    }
    fn event_script(&self, object: u32, event: u32) -> i32 {
        RollerEnv::event_script(self, object, event)
    }
    fn rider_in_box(&self, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn call(&mut self, c: CtrlCall) {
        self.log.push(Ev::Ctrl(c));
    }
}

/// A roller node's fields from +0x30 to +0x2bc (the tree's directions and centre; the rest of it as loaded).
fn put_node(m: &mut Mem, at: u32, b: &RollerBody) {
    let rb = at + 0x30;
    m.write_f32(rb + 8, b.rb.inv_mass);
    for k in 0..9 {
        m.write_f32(rb + 0xc + 4 * k as u32, b.rb.inv_inertia[k]);
        m.write_f32(at + 0x100 + 4 * k as u32, b.body_inertia[k]);
    }
    for (o, v) in [(0x30, b.rb.pos), (0x40, b.rb.q), (0x50, b.rb.p), (0x60, b.rb.l), (0x70, b.rb.v), (0x80, b.rb.w)] {
        w4(m, rb + o, v);
    }
    for (o, v) in [(0xc8, b.energy), (0xcc, b.mass), (0xd0, b.xd0), (0xd4, b.xd4), (0xd8, b.e), (0xdc, b.mu), (0xe0, b.bounce_speed), (0xe4, b.awake), (0xe8, b.knock)] {
        m.write_f32(at + o, v);
    }
    w4(m, at + 0xf0, b.xf0);
    w4(m, at + 0x130, b.com);
    m.write_u32(at + 0x140, b.asleep);
    for k in 0..8 {
        w4(m, at + 0x160 + 16 * k as u32, b.tree.dirs[k]);
    }
    w4(m, at + 0x1e0, b.tree.centre);
    w4(m, at + 0x2a0, b.lo);
    w4(m, at + 0x2b0, b.hi);
}

fn get_node(m: &mut Mem, at: u32, like: &RollerBody) -> RollerBody {
    let rb = at + 0x30;
    let mut b = like.clone();
    b.rb = RigidBody { inv_mass: m.read_f32(rb + 8), ..Default::default() };
    for k in 0..9 {
        b.rb.inv_inertia[k] = m.read_f32(rb + 0xc + 4 * k as u32);
        b.body_inertia[k] = m.read_f32(at + 0x100 + 4 * k as u32);
    }
    b.rb.pos = r4(m, rb + 0x30);
    b.rb.q = r4(m, rb + 0x40);
    b.rb.p = r4(m, rb + 0x50);
    b.rb.l = r4(m, rb + 0x60);
    b.rb.v = r4(m, rb + 0x70);
    b.rb.w = r4(m, rb + 0x80);
    let f = |m: &mut Mem, o: u32| m.read_f32(at + o);
    b.energy = f(m, 0xc8);
    b.mass = f(m, 0xcc);
    b.xd0 = f(m, 0xd0);
    b.xd4 = f(m, 0xd4);
    b.e = f(m, 0xd8);
    b.mu = f(m, 0xdc);
    b.bounce_speed = f(m, 0xe0);
    b.awake = f(m, 0xe4);
    b.knock = f(m, 0xe8);
    b.xf0 = r4(m, at + 0xf0);
    b.com = r4(m, at + 0x130);
    b.asleep = m.read_u32(at + 0x140);
    for k in 0..8 {
        b.tree.dirs[k] = r4(m, at + 0x160 + 16 * k as u32);
    }
    b.tree.centre = r4(m, at + 0x1e0);
    b.lo = r4(m, at + 0x2a0);
    b.hi = r4(m, at + 0x2b0);
    b
}

/// Every float of a body as bits, for comparing (the tree's directions and centre with it).
fn bits(b: &RollerBody) -> Vec<u32> {
    let mut v = vec![b.rb.inv_mass.to_bits()];
    v.extend(b.rb.inv_inertia.map(f32::to_bits));
    for a in [b.rb.pos, b.rb.q, b.rb.p, b.rb.l, b.rb.v, b.rb.w, b.xf0, b.com, b.tree.centre, b.lo, b.hi] {
        v.extend(b4(a));
    }
    v.extend([b.energy, b.mass, b.xd0, b.xd4, b.e, b.mu, b.bounce_speed, b.awake, b.knock].map(f32::to_bits));
    v.extend(b.body_inertia.map(f32::to_bits));
    v.push(b.asleep);
    for d in b.tree.dirs {
        v.extend(b4(d));
    }
    v
}

fn body(g: &mut Rng, tree: &SphereTree, t: &tricky_data::ssf::SphTree) -> RollerBody {
    let mass = g.pick(&[1.0f32, 5.0, 25.0, 120.0, 400.0]);
    let q = {
        let v = [g.f(1.0), g.f(1.0), g.f(1.0), g.f(1.0) + 0.001];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
        v.map(|x| x / l)
    };
    let mut b = RollerBody {
        rb: RigidBody { inv_mass: 1.0 / mass, pos: [g.f(20000.0), g.f(20000.0), g.f(5000.0), g.pick(&[0.0f32, 1.0])], q, p: g.v(300.0 * mass), l: g.v(2000.0), v: g.v(300.0), w: g.v(5.0), ..Default::default() },
        energy: g.f(50000.0).abs(),
        mass,
        xd0: g.f(3.0),
        xd4: 0.0,
        e: g.pick(&[0.0f32, 0.3, 0.6]),
        mu: g.pick(&[0.5f32, 0.0, 0.2]),
        bounce_speed: g.pick(&[f32::from_bits(0x438a_e38e), 50.0]),
        awake: g.pick(&[0.0f32, 1.0, 5.0, 5.1, 9.99]),
        knock: g.pick(&[0.0f32, 0.05, 0.3]),
        xf0: [0.0; 4],
        body_inertia: t.inv_inertia.map(|x| x * 0.075),
        com: [t.mass_centre[0], t.mass_centre[1], t.mass_centre[2], 0.0],
        asleep: (g.next() % 4 == 0) as u32,
        tree: tree.clone(),
        lo: [0.0; 4],
        hi: [0.0; 4],
    };
    b.update_transform();
    b.place_tree();
    b
}

#[test]
fn rollers_match_the_original() {
    let Ok(dir) = std::env::var("TRICKY_SSF_DIR") else { return };
    let mut g = Rng(0x7011_e75a_0000_0001);
    let mut n = [0usize; 10];
    let mut moved = [0usize; 10];
    let mut calls = 0usize;
    for c in COURSES {
        let Ok(bytes) = std::fs::read(format!("{dir}/{c}.ssf")) else { continue };
        let objects = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap());
        let ssf = Ssf::parse(&bytes, objects).unwrap();
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        // the course's trees, loaded as tests/sphtree_shape.rs loads them
        let file = r.mem.alloc(bytes.len() as u32 + 16);
        r.mem.write(file, &bytes).unwrap();
        let course = r.mem.alloc(0x40);
        let model_data = r.mem.alloc(0x20);
        r.mem.write_u32(course + 4, model_data);
        r.mem.write_u32(model_data + 0xc, objects);
        let objs = r.mem.alloc(objects.max(1) * 0x100);
        let models = r.mem.alloc(objects.max(1) * 0x20);
        for i in 0..objects {
            r.mem.write_u32(objs + i * 0x100 + 0xc0, models + i * 0x20);
        }
        let segs = r.mem.alloc(ssf.spline_segments.len().max(1) as u32 * 0x40);
        let heap = Rc::new(Cell::new(0x0080_0000u32));
        r.stub(0x2cc808, |_, _| {});
        r.stub(0x254a28, move |c, _| c.set_gpr(2, (objs + c.gpr(5) as u32 * 0x100) as u64));
        r.stub(0x254b20, move |c, _| c.set_gpr(2, (segs + c.gpr(5) as u32 * 0x40) as u64));
        r.stub(0x2c7e38, |c, _| c.set_gpr(2, 1));
        r.stub(0x2c8a70, move |c, _| c.set_gpr(2, file as u64));
        r.stub(0x14a040, |_, _| {});
        let h = heap.clone();
        r.stub(0x2ccf70, move |c, _| {
            let at = h.get();
            h.set((at + c.gpr(5) as u32 + 15) & !15);
            c.set_gpr(2, at as u64);
        });
        r.stub(0x238af0, |_, _| {});
        r.allow(0x239ba8);
        r.stub(0x23b520, |_, _| {});
        r.allow(0x239340);
        r.stub(0x2ce114, |c, m| {
            let mut b = vec![0u8; c.gpr(6) as usize];
            m.read(c.gpr(5) as u32, &mut b);
            m.write(c.gpr(4) as u32, &b).unwrap();
        });
        r.call(0x23bab0).unwrap();
        r.call(0x251238).unwrap();
        let path = r.mem.alloc(16);
        r.cpu.set_gpr(4, course as u64);
        r.cpu.set_gpr(5, path as u64);
        r.call(0x25f598).unwrap();
        let decode_buf = 0x0130_0000u32;
        let bufs: Rc<RefCell<HashMap<u32, (u32, u32)>>> = Rc::new(RefCell::new(HashMap::new()));
        r.allow(0x239b20);
        r.stub(0x238f50, move |c, m| {
            let tree = c.gpr(5) as u32;
            let mut src = vec![0u8; 0x1_0000];
            let data = m.read_u32(tree + 0x24);
            m.read(data, &mut src);
            let out = rle_decode(&src);
            let mut map = bufs.borrow_mut();
            let next = map.values().map(|(a, n)| a + n).max().unwrap_or(decode_buf);
            let at = map.entry(tree).or_insert(((next + 15) & !15, out.len() as u32)).0;
            m.write(at, &out).unwrap();
            c.set_gpr(2, at as u64);
        });
        r.stub(0x2391c8, |_, _| {});
        let tbase = r.mem.read_u32(course + 0x1c);
        let mut trees = vec![];
        let mut k = 0u32;
        for gr in &ssf.sphere_trees {
            for t in gr {
                trees.push((tbase + 0x150 * k, SphereTree::new(t), t.clone()));
                k += 1;
            }
        }
        if trees.is_empty() {
            continue;
        }
        // the functions under test and what they call
        for a in [
            0x13e140, 0x13e6a0, 0x102f80, 0x1108d0, 0x250428, 0x13eda8, 0x13ed30, 0x110830, 0x239d18, 0x261638, 0x239c50, 0x2391e8, 0x239d60, 0x102f50, 0x153808, 0x153ad8, 0x153c50,
            0x153ee8, 0x154338, 0x13e8a8, 0x13f448, 0x13dd40, 0x13dbe8, 0x13f5c8, 0x23a490, 0x23a570, 0x23a728, 0x13e860, 0x149e28, 0x149e18, 0x149e10, 0x13a9f8,
        ] {
            r.allow(a);
        }
        r.stub(0x261a48, |_, _| {});
        let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
        let ray: Rc<RefCell<VecDeque<(f32, V4, V4)>>> = Rc::new(RefCell::new(VecDeque::new()));
        let query: Rc<RefCell<VecDeque<(f32, V4, V4)>>> = Rc::new(RefCell::new(VecDeque::new()));
        let events: Rc<RefCell<HashMap<u32, i32>>> = Rc::new(RefCell::new(HashMap::new()));
        for (kind, addr, q) in [(0u32, 0x25aff8u32, ray.clone()), (1, 0x25b878, query.clone())] {
            let l = log.clone();
            r.stub(addr, move |c, m| {
                assert_eq!(c.gpr(4) as u32, 0x346488);
                let s = c.gpr(5) as u32;
                l.borrow_mut().push(Ev::Cast(kind, b4(r4(m, s + 0x10)), b4(r4(m, s + 0x20)), b4(r4(m, s + 0x30))));
                let (t, p, n) = q.borrow_mut().pop_front().unwrap();
                let out = c.gpr(6) as u32;
                w4(m, out + 0x10, p);
                w4(m, out + 0x20, n);
                c.set_f(0, t);
            });
        }
        let l = log.clone();
        r.stub(0x14fe68, move |c, m| {
            let b = c.gpr(5) as u32;
            l.borrow_mut().push(Ev::Box(b4(r4(m, b - 0x20)), b4(r4(m, b - 0x10))));
        });
        let app = r.mem.alloc(0x800);
        let race = r.mem.alloc(0x100);
        r.mem.write_u32(0x337c58, app);
        r.mem.write_u32(app + 0x730, race);
        let e = events.clone();
        r.stub(0x25fe78, move |c, _| {
            let (o, ev) = (c.gpr(5) as u32, c.gpr(6) as u32);
            let key = if ev == 1 { o } else { o + 0x1000 * ev };
            c.set_gpr(2, e.borrow().get(&key).copied().unwrap_or(-1) as i64 as u64);
        });
        let l = log.clone();
        r.stub(0x13d538, move |c, _| l.borrow_mut().push(Ev::Ctrl(CtrlCall::Start(c.gpr(4) as i32, c.gpr(5) as u32))));
        let pool = r.mem.alloc(0x100);
        r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
        let l = log.clone();
        r.stub(0x13b8c0, move |c, _| {
            assert_eq!(c.gpr(5), 3);
            l.borrow_mut().push(Ev::Script(c.gpr(6) as i32, c.gpr(7) as u32));
        });
        let l = log.clone();
        r.stub(0x216c50, move |c, _| l.borrow_mut().push(Ev::Sfx(c.gpr(5) as u32, c.gpr(6) as u32)));
        let nodes = [r.mem.alloc(0x400), r.mem.alloc(0x400)];
        let objects_at = [r.mem.alloc(0x100), r.mem.alloc(0x100)];
        let scratch = r.mem.alloc(0x100);
        for (nd, ob) in nodes.iter().zip(objects_at) {
            r.mem.write_u32(nd + 8, 0x36d790);
            r.mem.write_u32(nd + 0x28, ob);
        }
        // the constructor's callees
        for a in [0x13d6c0, 0x1537d0, 0x110898, 0x110860, 0x239ba8, 0x150670, 0x2504f0, 0x260ca0, 0x123098, 0x1507e8] {
            r.allow(a);
        }
        r.stub(0x13a840, |c, m| {
            let (node, object) = (c.gpr(4) as u32, c.gpr(7) as u32);
            m.write_u32(object + 0xe4, node);
            m.write_u32(node + 0x28, object);
        });
        r.stub(0x14fa70, |_, _| {});
        let tree_table = r.mem.alloc(0x10);
        r.mem.write_u32(0x346488 + 0x18, tree_table);
        let phys = r.mem.alloc(0x40);
        let ob_phys = r.mem.alloc(0x40);
        let kick = r.mem.alloc(0x40);
        let holder = r.mem.alloc(0x40);
        for case in 0..400 {
            let which = g.next() % 10;
            let pick = g.next() as usize % trees.len();
            let (taddr, ref tree0, ref traw) = trees[pick];
            let mut b = body(&mut g, tree0, traw);
            let node = nodes[0];
            // the node's tree copied from the course's (`FUN_002391e8`), then our state over it
            r.cpu.set_gpr(4, (node + 0x150) as u64);
            r.cpu.set_gpr(5, taddr as u64);
            r.call(0x2391e8).unwrap();
            put_node(&mut r.mem, node, &b);
            log.borrow_mut().clear();
            let mut env = Env::default();
            let object = objects_at[0];
            for (o, k) in [(object, 1u32), (object, 3), (objects_at[1], 1)] {
                let s = g.pick(&[-1i32, -1, 7, 30]);
                let key = if k == 1 { o } else { o + 0x1000 * k };
                env.events.insert(key, s);
            }
            *events.borrow_mut() = env.events.clone();
            for _ in 0..2 {
                let rad = tree0.levels[0].0;
                let hit = |g: &mut Rng| (g.pick(&[0.0f32, -1.0, 0.5, 1.5, 3.0, 20.0]), { let d = g.v(rad); [b.rb.pos[0] + d[0], b.rb.pos[1] + d[1], b.rb.pos[2] + d[2], g.pick(&[1.0f32, 0.0])] }, g.unit());
                let (h1, h2) = (hit(&mut g), hit(&mut g));
                env.ray.push_back(h1);
                env.query.push_back(h2);
            }
            *ray.borrow_mut() = env.ray.clone();
            *query.borrow_mut() = env.query.clone();
            let mut other: Option<(RollerBody, u32)> = None;
            let before = bits(&b);
            match which {
                0 => {
                    // a new orientation (any length) and body inertia
                    b.rb.q = [g.f(1.5), g.f(1.5), g.f(1.5), g.f(1.5)];
                    b.body_inertia = [0; 9].map(|_| g.f(0.01));
                    put_node(&mut r.mem, node, &b);
                    r.cpu.set_gpr(4, node as u64);
                    r.call(0x13e140).unwrap();
                    b.update_transform();
                }
                1 => {
                    r.cpu.set_gpr(4, scratch as u64);
                    r.cpu.set_gpr(5, node as u64);
                    r.call(0x13e6a0).unwrap();
                    let (p, q) = b.frame();
                    assert_eq!((b4(r4(&mut r.mem, scratch)), b4(r4(&mut r.mem, scratch + 0x10))), (b4(p), b4(q)), "{c} case {case}: frame");
                    r.cpu.set_gpr(4, (scratch + 0x40) as u64);
                    r.cpu.set_gpr(5, scratch as u64);
                    r.call(0x250428).unwrap();
                    let m = mat4_from_pos_quat(p, q);
                    for k in 0..4u32 {
                        assert_eq!(b4(r4(&mut r.mem, scratch + 0x40 + 16 * k)), b4(m[k as usize]), "{c} case {case}: matrix column {k}");
                    }
                }
                2 => {
                    r.cpu.set_gpr(4, node as u64);
                    r.call(0x13eda8).unwrap();
                    b.collide(&mut env);
                }
                3 => {
                    r.cpu.set_gpr(4, node as u64);
                    r.call(0x13e8a8).unwrap();
                    b.update(object, &mut env);
                }
                4 => {
                    let rider = g.pick(&[0u32, 0x00a0_0000]);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, rider as u64);
                    r.call(0x13f448).unwrap();
                    b.on_hit(object, if rider == 0 { None } else { Some(rider) }, &mut env);
                }
                5 => {
                    let (d, ax, sp) = (g.unit(), g.v(3.0), g.pick(&[0.0f32, 300.0, 1500.0]));
                    w4(&mut r.mem, scratch + 0x10, d);
                    w4(&mut r.mem, scratch + 0x20, ax);
                    r.mem.write_f32(scratch + 0x30, sp);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, scratch as u64);
                    r.call(0x13dd40).unwrap();
                    b.kick_contact(d, ax, sp);
                }
                6 => {
                    let v = [g.f(800.0), g.f(800.0), g.f(800.0), 0.0];
                    w4(&mut r.mem, scratch, v);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, scratch as u64);
                    r.call(0x13dbe8).unwrap();
                    b.kick_velocity(v);
                }
                9 => {
                    // `cRollerNode::cRollerNode` on a zeroed node over an object with a random place
                    for o in (0..0x400).step_by(4) {
                        r.mem.write_u32(node + o, 0);
                    }
                    let ob = objects_at[0];
                    let q = { let v = [g.f(1.0), g.f(1.0), g.f(1.0), g.f(1.0) + 0.01]; let l = (v.iter().map(|x| x * x).sum::<f32>()).sqrt(); v.map(|x| x / l) };
                    let mm = mat4_from_pos_quat([g.f(20000.0), g.f(20000.0), g.f(4000.0), 1.0], q);
                    for k in 0..4u32 {
                        w4(&mut r.mem, ob + 16 * k, mm[k as usize]);
                    }
                    let fl = g.next();
                    r.mem.write_u32(ob + 0xe8, fl);
                    r.mem.write_u32(ob + 0xec, ob_phys);
                    r.mem.write(ob_phys + 0x12, &0i16.to_le_bytes()).unwrap();
                    r.mem.write_u32(tree_table, taddr);
                    r.mem.write_u32(holder + 0x30, taddr);
                    let mass = g.pick(&[1.0f32, 12.0, 80.0, 300.0]);
                    let xd0 = g.f(2.0);
                    let vel = if g.next() % 2 == 0 { [0.0; 4] } else { [g.f(600.0), g.f(600.0), g.f(600.0), 0.0] };
                    r.mem.write_f32(phys + 0xc, mass);
                    r.mem.write_f32(phys + 0x10, xd0);
                    for k in 0..3u32 {
                        r.mem.write_f32(phys + 0x18 + 4 * k, vel[k as usize]);
                    }
                    let (d, ax, sp) = (g.unit(), g.v(3.0), g.pick(&[0.0f32, 300.0, 1500.0]));
                    w4(&mut r.mem, kick + 0x10, d);
                    w4(&mut r.mem, kick + 0x20, ax);
                    r.mem.write_f32(kick + 0x30, sp);
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, 0);
                    r.cpu.set_gpr(6, ob as u64);
                    r.cpu.set_gpr(7, 0);
                    r.cpu.set_gpr(8, kick as u64);
                    r.cpu.set_gpr(9, holder as u64);
                    r.cpu.set_gpr(10, phys as u64);
                    r.call(0x13d6c0).unwrap();
                    assert_eq!(r.mem.read_u32(ob + 0xe8), fl & 0xffff_ffdd | 0x45, "{c} case {case}: the object's flags");
                    let (pos, q) = mat4_to_pos_quat(mm);
                    b = RollerBody { rb: RigidBody::default(), energy: 0.0, mass: 0.0, xd0: 0.0, xd4: 0.0, e: 0.0, mu: 0.0, bounce_speed: 0.0, awake: 0.0, knock: 0.0, xf0: [0.0; 4], body_inertia: [0.0; 9], com: [0.0; 4], asleep: 0, tree: tree0.clone(), lo: [0.0; 4], hi: [0.0; 4] };
                    let mc = traw.mass_centre;
                    b.init(mass, xd0, [mc[0], mc[1], mc[2], 0.0], traw.inv_inertia, pos, q);
                    if vel == [0.0; 4] {
                        b.kick_contact(d, ax, sp);
                    } else {
                        b.kick_velocity(vel);
                    }
                }
                _ => {
                    // two rollers near each other
                    let pick2 = g.next() as usize % trees.len();
                    let (taddr2, ref tree2, ref traw2) = trees[pick2];
                    let mut b2 = body(&mut g, tree2, traw2);
                    let reach = tree0.levels[0].0 + tree2.levels[0].0;
                    let d = g.v(reach * 0.8);
                    b2.rb.pos = [b.rb.pos[0] + d[0], b.rb.pos[1] + d[1], b.rb.pos[2] + d[2], b2.rb.pos[3]];
                    b2.update_transform();
                    b2.place_tree();
                    r.cpu.set_gpr(4, (nodes[1] + 0x150) as u64);
                    r.cpu.set_gpr(5, taddr2 as u64);
                    r.call(0x2391e8).unwrap();
                    put_node(&mut r.mem, nodes[1], &b2);
                    if b.tree.tree(&b2.tree) == Some(None) {
                        // the game reads its stack: not compared
                        continue;
                    }
                    r.cpu.set_gpr(4, node as u64);
                    r.cpu.set_gpr(5, nodes[1] as u64);
                    r.call(0x13f5c8).unwrap();
                    pair(&mut b, &mut b2, objects_at[1], &mut env);
                    other = Some((b2, nodes[1]));
                }
            }
            let theirs = get_node(&mut r.mem, node, &b);
            assert_eq!(bits(&theirs), bits(&b), "{c} case {case} ({which}):\n theirs {theirs:?}\n ours   {b:?}");
            if let Some((b2, at)) = other {
                let t2 = get_node(&mut r.mem, at, &b2);
                assert_eq!(bits(&t2), bits(&b2), "{c} case {case}: the other roller");
            }
            assert_eq!(*log.borrow(), env.log, "{c} case {case} ({which}): calls");
            n[which as usize] += 1;
            moved[which as usize] += (bits(&b) != before) as usize;
            calls += env.log.len();
        }
    }
    eprintln!("cases by kind: {n:?}, changed: {moved:?}, {calls} calls");
}

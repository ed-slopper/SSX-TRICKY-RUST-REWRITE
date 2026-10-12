//! The wipe-out body (board row G6) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `Ragdoll_ComputeMassProps`, `Ragdoll_InitFromBoarder`, `Ragdoll_Step` (with `Ragdoll_Contacts`),
//! `Ragdoll_HitRoller` and `Ragdoll_GroundProbe` on random riders (bodies spread about the rider as the skeleton
//! puts them), the world's answers scripted (`World_RayCast`, `World_QueryInstances`: the shapes handed over
//! compared), every callee logged; the rider's and the body's fields, the record and the calls bit for bit
//! against `tricky_game::ragdoll`.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_game::collbody::CollBody;
use tricky_game::ragdoll::{ground_probe, CastOut, RagEnv, RagRider, Ragdoll};
use tricky_game::rigidbody::RigidBody;
use tricky_game::roller::{RollerBody, RollerEnv};
use tricky_game::sphtree::{SphereTree, WorldSphTree};
use tricky_game::walls::Contact;
use tricky_game::wipeout_motion::{get_up_update, play_impact_clip, WipeEnv, WipeRider, Wipeout};
use tricky_game::worldboarder::WorldBoarder;

type V4 = [f32; 4];

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
    fn quat(&mut self) -> V4 {
        let v = [self.f(1.0), self.f(1.0), self.f(1.0), self.f(1.0) + 0.001];
        let l = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
        v.map(|x| x / l)
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
    Shape(u32, [u32; 4], [u32; 4], [u32; 4]),
    Line([u32; 4], [u32; 4]),
    Impact([u32; 4]),
    RiderContact([u32; 4], [u32; 4], [u32; 4]),
    Player(u32, Vec<u32>),
    Sfx(u32, u32),
    Orient,
    Touch,
    Script(i32, u32),
    Play(u32),
    BlendOut,
    Disp,
    Probe,
    Rotate(u32, [u32; 4]),
    Collide(i32),
    Wall,
    Translate([u32; 4]),
    Predict(bool),
    PredictReset,
    Motion(u32),
    State(u32),
    Mute,
    Impact2,
    Slide,
    Stop,
    Unmute,
    Leave,
}

#[derive(Default)]
struct Env {
    casts: VecDeque<(f32, CastOut)>,
    queries: VecDeque<(f32, CastOut)>,
    lines: VecDeque<(f32, CastOut)>,
    flags: HashMap<u32, u32>,
    mask: i32,
    ev1: i32,
    touch: Option<(V4, V4)>,
    log: Vec<Ev>,
    // the wipe-out motion's
    clip: u32,
    clip_type: u32,
    disp: V4,
    probes: VecDeque<bool>,
    g: f32,
    wmask: i32,
    predict: (f32, V4, V4, u32),
}

fn contact_words(c: &Contact) -> Vec<u32> {
    let mut v = vec![];
    for a in [c.pos, c.dir, c.normal] {
        v.extend(b4(a));
    }
    v.push(c.speed_in.to_bits());
    v
}

impl RollerEnv for Env {
    fn ray_cast(&mut self, _: &WorldSphTree) -> (f32, V4, V4) {
        unreachable!()
    }
    fn query_instances(&mut self, _: &WorldSphTree) -> (f32, V4, V4) {
        unreachable!()
    }
    fn move_box(&mut self, _: V4, _: V4) {
        unreachable!()
    }
    fn event_script(&self, _: u32, _: u32) -> i32 {
        self.ev1
    }
    fn start_script(&mut self, s: i32, o: u32) {
        self.log.push(Ev::Script(s, o));
    }
    fn sfx(&mut self, r: u32, o: u32) {
        self.log.push(Ev::Sfx(r, o));
    }
}

impl RagEnv for Env {
    fn mask(&self) -> i32 {
        self.mask
    }
    fn cast_body(&mut self, q: &WorldBoarder) -> (f32, CastOut) {
        self.log.push(Ev::Shape(q.kind, b4(q.hi), b4(q.lo), b4(q.mid)));
        self.casts.pop_front().unwrap()
    }
    fn query_body(&mut self, q: &WorldBoarder) -> (f32, CastOut) {
        self.log.push(Ev::Shape(q.kind + 0x10, b4(q.hi), b4(q.lo), b4(q.mid)));
        self.queries.pop_front().unwrap()
    }
    fn cast_line(&mut self, from: V4, to: V4) -> (f32, CastOut) {
        self.log.push(Ev::Line(b4(from), b4(to)));
        self.lines.pop_front().unwrap()
    }
    fn accumulate_impact(&mut self, n: V4) {
        self.log.push(Ev::Impact(b4(n)));
    }
    fn rider_contact(&mut self, p: V4, v: V4, n: V4) {
        self.log.push(Ev::RiderContact(b4(p), b4(v), b4(n)));
    }
    fn object_flags(&self, o: u32) -> u32 {
        self.flags.get(&o).copied().unwrap_or(0)
    }
    fn player_contact(&mut self, o: u32, c: &Contact, out: &CastOut) {
        let mut v = contact_words(c);
        v.extend(b4(out.pos));
        v.extend(b4(out.normal));
        self.log.push(Ev::Player(o, v));
    }
    fn update_orientation(&mut self, _: &mut RagRider) {
        self.log.push(Ev::Orient);
    }
    fn roller_touch(&mut self, _: &RollerBody, _: &CollBody) -> Option<(V4, V4)> {
        self.log.push(Ev::Touch);
        self.touch
    }
}

// the rider's fields
const POS: u32 = 0x140;
const VEL: u32 = 0x150;
const Q: u32 = 0x170;
const UP: u32 = 0x1a0;
const HEIGHT: u32 = 0x1bc;
const SURF: u32 = 0x290;
const GROUND: u32 = 0x2a0;
const FALL: u32 = 0x2b0;
const GROUND_AT: u32 = 0x2d0;
const KIND: u32 = 0x2e0;
const CRASH_N: u32 = 0x2f0;
const SIDE: u32 = 0x320;
const COURSE: u32 = 0x350;
const BODY: u32 = 0x470;
const HIP: u32 = 0x8c8;

fn put_body(m: &mut Mem, at: u32, b: &CollBody) {
    w4(m, at + 0x10, b.center);
    m.write_f32(at + 0x20, b.radius);
    m.write_u32(at + 0x24, b.x24 as u32);
    m.write_u32(at + 0x28, b.count as u32);
    for (k, (c, r)) in b.spheres.iter().enumerate() {
        w4(m, at + 0x30 + 0x20 * k as u32, *c);
        m.write_f32(at + 0x40 + 0x20 * k as u32, *r);
    }
}
fn get_body(m: &mut Mem, at: u32) -> CollBody {
    let mut b = CollBody { center: r4(m, at + 0x10), radius: m.read_f32(at + 0x20), x24: m.read_u32(at + 0x24) as i32, count: m.read_u32(at + 0x28) as i32, spheres: [([0.0; 4], 0.0); 18] };
    for k in 0..18u32 {
        b.spheres[k as usize] = (r4(m, at + 0x30 + 0x20 * k), m.read_f32(at + 0x40 + 0x20 * k));
    }
    b
}

fn put_rider(m: &mut Mem, at: u32, r: &RagRider) {
    for (o, v) in [(POS, r.pos), (VEL, r.vel), (Q, r.q), (UP, r.up), (GROUND, r.ground), (FALL, r.fall), (GROUND_AT, r.ground_at), (CRASH_N, r.crash_n), (SIDE, r.side), (COURSE, r.course), (HIP, r.hip)] {
        w4(m, at + o, v);
    }
    m.write_f32(at + HEIGHT, r.height);
    m.write_u32(at + SURF, r.surface);
    m.write_u32(at + KIND, r.ground_kind);
    put_body(m, at + BODY, &r.body);
}
fn get_rider(m: &mut Mem, at: u32, like: &RagRider) -> RagRider {
    let mut r = like.clone();
    r.pos = r4(m, at + POS);
    r.vel = r4(m, at + VEL);
    r.q = r4(m, at + Q);
    r.up = r4(m, at + UP);
    r.ground = r4(m, at + GROUND);
    r.fall = r4(m, at + FALL);
    r.ground_at = r4(m, at + GROUND_AT);
    r.crash_n = r4(m, at + CRASH_N);
    r.side = r4(m, at + SIDE);
    r.course = r4(m, at + COURSE);
    r.height = m.read_f32(at + HEIGHT);
    r.surface = m.read_u32(at + SURF);
    r.ground_kind = m.read_u32(at + KIND);
    r.body = get_body(m, at + BODY);
    r
}

fn put_rb(m: &mut Mem, rb: u32, b: &RigidBody) {
    m.write_f32(rb + 8, b.inv_mass);
    for k in 0..9 {
        m.write_f32(rb + 0xc + 4 * k as u32, b.inv_inertia[k]);
    }
    for (o, v) in [(0x30, b.pos), (0x40, b.q), (0x50, b.p), (0x60, b.l), (0x70, b.v), (0x80, b.w)] {
        w4(m, rb + o, v);
    }
}
fn get_rb(m: &mut Mem, rb: u32) -> RigidBody {
    let mut b = RigidBody { inv_mass: m.read_f32(rb + 8), ..Default::default() };
    for k in 0..9 {
        b.inv_inertia[k] = m.read_f32(rb + 0xc + 4 * k as u32);
    }
    b.pos = r4(m, rb + 0x30);
    b.q = r4(m, rb + 0x40);
    b.p = r4(m, rb + 0x50);
    b.l = r4(m, rb + 0x60);
    b.v = r4(m, rb + 0x70);
    b.w = r4(m, rb + 0x80);
    b
}

fn rider_bits(r: &RagRider) -> Vec<u32> {
    let mut v = vec![];
    for a in [r.pos, r.vel, r.q, r.up, r.ground, r.fall, r.ground_at, r.crash_n, r.side, r.course, r.body.center] {
        v.extend(b4(a));
    }
    v.extend([r.height.to_bits(), r.surface, r.ground_kind, r.body.x24 as u32]);
    for s in r.body.spheres.iter().take(r.body.count as usize) {
        v.extend(b4(s.0));
    }
    v
}
fn rb_bits(b: &RigidBody) -> Vec<u32> {
    let mut v = vec![b.inv_mass.to_bits()];
    v.extend(b.inv_inertia.map(f32::to_bits));
    for a in [b.pos, b.q, b.p, b.l, b.v, b.w] {
        v.extend(b4(a));
    }
    v
}

fn put_out(m: &mut Mem, at: u32, o: &CastOut) {
    for k in 0..16 {
        m.write_u32(at + 4 * k, 0x5eed_0000 + k);
    }
    m.write_u32(at, o.kind);
    w4(m, at + 0x10, o.pos);
    w4(m, at + 0x20, o.normal);
    for k in 0..3 {
        m.write_u32(at + 0x30 + 4 * k as u32, o.x30[k]);
    }
}
fn out_words(o: &CastOut) -> Vec<u32> {
    let mut v = vec![o.kind];
    v.extend(b4(o.pos));
    v.extend(b4(o.normal));
    v.extend(o.x30);
    v
}
fn get_out(m: &mut Mem, at: u32) -> CastOut {
    CastOut { kind: m.read_u32(at), pos: r4(m, at + 0x10), normal: r4(m, at + 0x20), x30: [m.read_u32(at + 0x30), m.read_u32(at + 0x34), m.read_u32(at + 0x38)] }
}

/// The runner with the functions the wipe-out body needs, its world stubbed (scripted from the shared queues) and
/// every callee logged, a rider, its motion, a record, two objects and a roller in the rider's pairs.
struct H {
    r: Runner,
    log: Rc<RefCell<Vec<Ev>>>,
    casts: Rc<RefCell<VecDeque<(f32, CastOut)>>>,
    queries: Rc<RefCell<VecDeque<(f32, CastOut)>>>,
    lines: Rc<RefCell<VecDeque<(f32, CastOut)>>>,
    touch: Rc<RefCell<Option<(V4, V4)>>>,
    ev1: Rc<RefCell<i32>>,
    rider: u32,
    vt: u32,
    motion: u32,
    rec: u32,
    scratch: u32,
    objs: [u32; 2],
    node: u32,
    node_obj: u32,
    entry: u32,
}

fn harness() -> Option<H> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [
        0x10d6f8, 0x10dad8, 0x10dca8, 0x10e190, 0x10ed68, 0x10fd58, 0x236530, 0x2cc808, 0x153808, 0x153ad8, 0x153c50, 0x153ee8, 0x154338, 0x102f50, 0x102f80, 0x110830, 0x236448, 0x110838, 0x110898,
        0x110860, 0x128a18, 0x10ebc8, 0x257810, 0x235fc8, 0x236830, 0x13f448, 0x149e28, 0x149e10, 0x13b7a8,
    ] {
        r.allow(a);
    }
    r.stub(0x257ca0, |_, _| {});
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let casts: Rc<RefCell<VecDeque<(f32, CastOut)>>> = Rc::new(RefCell::new(VecDeque::new()));
    let queries: Rc<RefCell<VecDeque<(f32, CastOut)>>> = Rc::new(RefCell::new(VecDeque::new()));
    let lines: Rc<RefCell<VecDeque<(f32, CastOut)>>> = Rc::new(RefCell::new(VecDeque::new()));
    // the line's constructor: logs its ends, marks the shape
    let l = log.clone();
    r.stub(0x25d7f0, move |c, m| {
        assert_eq!(c.f(12), 0.5);
        assert_eq!(c.gpr(7), 1);
        let s = c.gpr(4) as u32;
        l.borrow_mut().push(Ev::Line(b4(r4(m, c.gpr(5) as u32)), b4(r4(m, c.gpr(6) as u32))));
        m.write_u32(s, 0x11ae);
    });
    let (l, cq, lq) = (log.clone(), casts.clone(), lines.clone());
    r.stub(0x25aff8, move |c, m| {
        assert_eq!(c.gpr(4) as u32, 0x346488);
        let s = c.gpr(5) as u32;
        let (t, o) = if m.read_u32(s) == 0x11ae {
            lq.borrow_mut().pop_front().unwrap()
        } else {
            l.borrow_mut().push(Ev::Shape(m.read_u32(s), b4(r4(m, s + 0x10)), b4(r4(m, s + 0x20)), b4(r4(m, s + 0x30))));
            cq.borrow_mut().pop_front().unwrap()
        };
        put_out(m, c.gpr(6) as u32, &o);
        c.set_f(0, t);
    });
    let (l, qq) = (log.clone(), queries.clone());
    r.stub(0x25b878, move |c, m| {
        let s = c.gpr(5) as u32;
        l.borrow_mut().push(Ev::Shape(m.read_u32(s) + 0x10, b4(r4(m, s + 0x10)), b4(r4(m, s + 0x20)), b4(r4(m, s + 0x30))));
        let (t, o) = qq.borrow_mut().pop_front().unwrap();
        put_out(m, c.gpr(6) as u32, &o);
        c.set_f(0, t);
    });
    let l = log.clone();
    r.stub(0x125c48, move |c, m| l.borrow_mut().push(Ev::Impact(b4(r4(m, c.gpr(5) as u32)))));
    let rider_fn = r.mem.alloc(0x10);
    let l = log.clone();
    r.stub(rider_fn, move |c, m| l.borrow_mut().push(Ev::RiderContact(b4(r4(m, c.gpr(5) as u32)), b4(r4(m, c.gpr(6) as u32)), b4(r4(m, c.gpr(7) as u32)))));
    let l = log.clone();
    r.stub(0x13bd40, move |c, m| {
        let cw = c.gpr(6) as u32;
        let mut v: Vec<u32> = (0..13).map(|k| m.read_u32(cw + 4 * k)).collect();
        let o = get_out(m, c.gpr(7) as u32);
        v.extend(b4(o.pos));
        v.extend(b4(o.normal));
        l.borrow_mut().push(Ev::Player(c.gpr(4) as u32, v));
    });
    let l = log.clone();
    r.stub(0x216c50, move |c, _| l.borrow_mut().push(Ev::Sfx(c.gpr(5) as u32, c.gpr(6) as u32)));
    let l = log.clone();
    r.stub(0x129008, move |_, _| l.borrow_mut().push(Ev::Orient));
    let touch: Rc<RefCell<Option<(V4, V4)>>> = Rc::new(RefCell::new(None));
    let (l, t) = (log.clone(), touch.clone());
    r.stub(0x2394a8, move |c, m| {
        l.borrow_mut().push(Ev::Touch);
        let h = *t.borrow();
        if let Some((p, q)) = h {
            w4(m, c.gpr(6) as u32, p);
            w4(m, c.gpr(7) as u32, q);
        }
        c.set_gpr(2, h.is_some() as u64);
    });
    let ev1: Rc<RefCell<i32>> = Rc::new(RefCell::new(-1));
    let e = ev1.clone();
    r.stub(0x25fe78, move |c, _| c.set_gpr(2, *e.borrow() as i64 as u64));
    let pool = r.mem.alloc(0x100);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    let l = log.clone();
    r.stub(0x13b8c0, move |c, _| l.borrow_mut().push(Ev::Script(c.gpr(6) as i32, c.gpr(7) as u32)));

    let rider = r.mem.alloc(0x5900);
    let vt = r.mem.alloc(0x100);
    r.mem.write_u32(rider, vt);
    r.mem.write_u32(vt + 0x60, 0);
    r.mem.write_u32(vt + 0x64, rider_fn);
    r.mem.write_u32(rider + 0x44d8, 0);
    let motion = r.mem.alloc(0x200);
    r.mem.write_u32(motion + 0x60, rider);
    let rec = r.mem.alloc(0x40);
    let scratch = r.mem.alloc(0x40);
    let objs = [r.mem.alloc(0x100), r.mem.alloc(0x100)];
    let node = r.mem.alloc(0x400);
    let node_obj = r.mem.alloc(0x100);
    let entry = r.mem.alloc(0x10);
    r.mem.write_u32(node + 8, 0x36d790);
    r.mem.write_u32(node + 0x14, 0);
    r.mem.write_u32(node + 0x28, node_obj);
    r.mem.write_u32(node + 0x344, node);
    r.mem.write_u32(entry, node + 0x2c0);
    r.mem.write_u32(entry + 4, rider + 0x700);
    r.mem.write_u32(entry + 8, 0);
    r.mem.write_u32(entry + 0xc, 0);
    Some(H { r, log, casts, queries, lines, touch, ev1, rider, vt, motion, rec, scratch, objs, node, node_obj, entry })
}

#[test]
fn ragdoll_matches_the_original() {
    let Some(H { mut r, log, casts, queries, lines, touch, ev1, rider, motion, rec, scratch, objs, node, node_obj, entry, .. }) = harness() else { return };
    let mut g = Rng(0x7a9d_0115_0000_0001);
    let mut n = [0usize; 5];
    let mut moved = [0usize; 5];
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for case in 0..6000 {
        // the rider: its body's spheres spread about it as a skeleton puts them
        let pos = [g.f(30000.0), g.f(30000.0), g.f(5000.0), 1.0];
        let mut body = CollBody::new();
        body.init_rider(g.pick(&[1.1f32, 1.3]));
        let spread = g.pick(&[60.0f32, 25.0, 0.0]);
        for (k, s) in body.spheres.iter_mut().enumerate() {
            s.0 = [pos[0] + g.f(spread), pos[1] + g.f(spread), pos[2] + 20.0 + 6.0 * k as f32 + g.f(spread), g.pick(&[1.0f32, 1.0, 0.0])];
        }
        body.center = [pos[0], pos[1], pos[2] + 80.0, 1.0];
        body.x24 = g.pick(&[-1i32, 0x273c3]);
        let mut rr = RagRider {
            id: rider,
            pos,
            vel: g.v(2500.0),
            q: g.quat(),
            up: g.unit(),
            height: g.f(50.0),
            surface: g.next(),
            ground: if g.next() % 2 == 0 { [0.0f32, 0.0, 1.0, 0.0] } else { g.unit() },
            fall: g.unit(),
            ground_at: g.v(100.0),
            ground_kind: g.pick(&[3u32, 10, 1]),
            crash_n: g.unit(),
            side: g.unit(),
            course: [pos[0] + g.f(3000.0), pos[1] + g.f(3000.0), pos[2] + g.f(500.0), g.pick(&[1.0f32, 0.0])],
            body,
            hip: [g.f(30.0), g.f(30.0), g.f(100.0), 0.0],
        };
        let mut rd = Ragdoll { t_air: g.pick(&[0.0f32, 0.1, 0.149, 0.15, 2.0]), crash_at: [pos[0] + g.f(80.0), pos[1] + g.f(80.0), pos[2] + g.f(80.0), 1.0], crash_n: g.unit(), rb: RigidBody::default() };
        // a body already set going
        rd.init(&mut rr.clone());
        rd.rb.p = g.v(3000.0 / rd.rb.inv_mass.max(1e-6) * 0.001);
        rd.rb.l = g.v(3.0e5);
        rd.rb.v = g.v(2500.0);
        rd.rb.w = g.v(10.0);
        rd.rb.pos = [rd.rb.pos[0] + g.f(5.0), rd.rb.pos[1] + g.f(5.0), rd.rb.pos[2] + g.f(5.0), rd.rb.pos[3]];
        rd.rb.q = g.quat();
        put_rider(&mut r.mem, rider, &rr);
        r.mem.write_f32(motion + 8, rd.t_air);
        w4(&mut r.mem, motion + 0x20, rd.crash_at);
        w4(&mut r.mem, motion + 0x40, rd.crash_n);
        put_rb(&mut r.mem, motion + 0x70, &rd.rb);
        let mut rc = CastOut { kind: 0x77, pos: g.v(1.0), normal: g.v(1.0), x30: [1, 2, 3] };
        put_out(&mut r.mem, rec, &rc);
        // the world's answers
        let mut env = Env { mask: g.pick(&[0x273c3i32, 5]), ev1: g.pick(&[-1i32, 9]), ..Default::default() };
        r.mem.write_u32(0x31d314, env.mask as u32);
        *ev1.borrow_mut() = env.ev1;
        for o in objs {
            let f = g.pick(&[0x80u32, 0, 0x81]);
            r.mem.write_u32(o + 0xe8, f);
            env.flags.insert(o, f);
        }
        let hit = |g: &mut Rng, at: V4| -> (f32, CastOut) {
            let t = g.pick(&[0.0f32, -1.0, 0.5, 3.0, 12.0]);
            let n = g.unit();
            (t, CastOut { kind: g.pick(&[1u32, 6, 10]), pos: [at[0] + g.f(60.0), at[1] + g.f(60.0), at[2] + g.f(60.0), 1.0], normal: n, x30: [g.next(), g.next(), g.pick(&objs)] })
        };
        for _ in 0..2 {
            let h = hit(&mut g, rd.rb.pos);
            env.casts.push_back(h);
        }
        let h = hit(&mut g, rd.rb.pos);
        env.queries.push_back(h);
        let h = hit(&mut g, rr.pos);
        let h = if g.next() % 3 == 0 { (-1.0, h.1) } else { h };
        env.lines.push_back(h);
        *casts.borrow_mut() = env.casts.clone();
        *queries.borrow_mut() = env.queries.clone();
        *lines.borrow_mut() = env.lines.clone();
        // a roller in the pairs, sometimes
        let with_roller = g.next() % 3 == 0;
        let tree = SphereTree { dirs: [[0.0; 4]; 8], centre: [0.0; 4], home: [0.0; 4], depth: 0, levels: vec![], masks: vec![] };
        let mut roller = RollerBody {
            rb: RigidBody { inv_mass: g.pick(&[0.01f32, 0.2]), pos: [pos[0] + g.f(100.0), pos[1] + g.f(100.0), pos[2] + g.f(100.0), 1.0], q: g.quat(), p: g.v(100.0), l: g.v(1000.0), v: g.v(300.0), w: g.v(2.0), ..Default::default() },
            energy: 5.0,
            mass: 1.0,
            xd0: 0.0,
            xd4: 0.0,
            e: 0.0,
            mu: 0.5,
            bounce_speed: 0.0,
            awake: 3.0,
            knock: 0.25,
            xf0: [0.0; 4],
            body_inertia: [0.0; 9],
            com: [0.0; 4],
            asleep: (g.next() % 2) as u32,
            tree,
            lo: [0.0; 4],
            hi: [0.0; 4],
        };
        roller.rb.inv_inertia = [0; 9].map(|_| g.pick(&[0.0f32, 1e-4, -3e-5, 2e-4]));
        let tch = if g.next() % 4 == 0 { None } else { Some((g.v(8.0), [pos[0] + g.f(60.0), pos[1] + g.f(60.0), pos[2] + g.f(90.0), 1.0])) };
        *touch.borrow_mut() = tch;
        env.touch = tch;
        put_rb(&mut r.mem, node + 0x30, &roller.rb);
        r.mem.write_f32(node + 0xc8, roller.energy);
        r.mem.write_f32(node + 0xe4, roller.awake);
        r.mem.write_f32(node + 0xe8, roller.knock);
        r.mem.write_u32(node + 0x140, roller.asleep);
        r.mem.write_u32(rider + 0x700, if with_roller { entry } else { 0 });
        log.borrow_mut().clear();
        let which = g.next() % 5;
        let mut rollers: Vec<(u32, RollerBody)> = if with_roller { vec![(node_obj, roller.clone())] } else { vec![] };
        let before = (rider_bits(&rr), rb_bits(&rd.rb));
        match which {
            0 => {
                let out = scratch;
                r.cpu.set_gpr(4, out as u64);
                r.cpu.set_gpr(5, motion as u64);
                r.call(0x10d6f8).unwrap();
                let com = rd.mass_props(&rr);
                assert_eq!(b4(r4(&mut r.mem, out)), b4(com), "case {case}: the centre of mass");
            }
            1 => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10dad8).unwrap();
                rd.init(&mut rr);
            }
            2 => {
                let dt = g.pick(&[1.0f32 / 60.0, 1.0 / 30.0]);
                r.cpu.set_f(12, dt);
                r.cpu.set_gpr(4, motion as u64);
                r.cpu.set_gpr(5, rec as u64);
                r.call(0x10dca8).unwrap();
                rd.step(dt, &mut rr, &mut rc, &mut rollers, &mut env);
            }
            3 => {
                r.cpu.set_gpr(4, motion as u64);
                r.cpu.set_gpr(5, rec as u64);
                r.call(0x10e190).unwrap();
                rd.contacts(&mut rr, &mut rc, &mut rollers, &mut env);
            }
            _ => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10fd58).unwrap();
                let got = ground_probe(&mut rr, &mut env);
                assert_eq!(r.cpu.gpr(2) != 0, got, "case {case}: the probe's answer");
            }
        }
        let theirs = get_rider(&mut r.mem, rider, &rr);
        assert_eq!(rider_bits(&theirs), rider_bits(&rr), "case {case} ({which}): the rider\n theirs {theirs:?}\n ours   {rr:?}");
        let trb = get_rb(&mut r.mem, motion + 0x70);
        assert_eq!(rb_bits(&trb), rb_bits(&rd.rb), "case {case} ({which}): the body\n theirs {trb:?}\n ours   {:?}", rd.rb);
        assert_eq!(b4(r4(&mut r.mem, motion + 0x40)), b4(rd.crash_n), "case {case} ({which}): the crash normal");
        assert_eq!(out_words(&get_out(&mut r.mem, rec)), out_words(&rc), "case {case} ({which}): the record");
        if let Some((_, ro)) = rollers.first() {
            assert_eq!(rb_bits(&get_rb(&mut r.mem, node + 0x30)), rb_bits(&ro.rb), "case {case} ({which}): the roller");
            assert_eq!(r.mem.read_u32(node + 0x140), ro.asleep, "case {case} ({which}): the roller asleep");
        }
        let theirs_log = log.borrow().clone();
        assert_eq!(theirs_log, env.log, "case {case} ({which}): calls");
        for e in &env.log {
            *seen.entry(format!("{e:?}").split('(').next().unwrap().to_string()).or_default() += 1;
        }
        if rc.kind != 0x77 {
            *seen.entry("record".into()).or_default() += 1;
        }
        n[which as usize] += 1;
        moved[which as usize] += ((rider_bits(&rr), rb_bits(&rd.rb)) != before) as usize;
    }
    eprintln!("cases {n:?}, changed {moved:?}, calls {seen:?}");
}

impl WipeEnv for Env {
    fn clip(&self) -> u32 {
        self.clip
    }
    fn clip_type(&self) -> u32 {
        self.clip_type
    }
    fn play_clip(&mut self, c: u32) {
        self.log.push(Ev::Play(c));
    }
    fn blend_out_all(&mut self) {
        self.log.push(Ev::BlendOut);
    }
    fn displacement(&mut self) -> V4 {
        self.log.push(Ev::Disp);
        self.disp
    }
    fn probe_ground(&mut self, _: &mut RagRider, _: &mut WipeRider) -> bool {
        self.log.push(Ev::Probe);
        self.probes.pop_front().unwrap()
    }
    fn rotate_about_axis(&mut self, _: &mut RagRider, _: &mut WipeRider, angle: f32, axis: V4) {
        self.log.push(Ev::Rotate(angle.to_bits(), b4(axis)));
    }
    fn surface_g(&self, _: u32) -> f32 {
        self.g
    }
    fn collide_world(&mut self, _: &mut RagRider, _: &mut WipeRider) {
        self.log.push(Ev::Collide(self.wmask));
        self.log.push(Ev::Wall);
    }
    fn translate(&mut self, _: &mut RagRider, d: V4) {
        self.log.push(Ev::Translate(b4(d)));
    }
    fn predict(&mut self, _: &RagRider, w: &mut WipeRider, takeoff: bool) {
        self.log.push(Ev::Predict(takeoff));
        (w.t_land, w.land_v, w.land_n, w.land_surface) = self.predict;
    }
    fn predict_reset(&mut self) {
        self.log.push(Ev::PredictReset);
    }
    fn set_motion(&mut self, m: u32) {
        self.log.push(Ev::Motion(m));
    }
    fn set_state(&mut self, st: u32) {
        self.log.push(Ev::State(st));
    }
    fn sfx_enter(&mut self) {
        self.log.push(Ev::Mute);
    }
    fn sfx_impact(&mut self) {
        self.log.push(Ev::Impact2);
    }
    fn sfx_slide(&mut self) {
        self.log.push(Ev::Slide);
    }
    fn sfx_stop(&mut self) {
        self.log.push(Ev::Stop);
    }
    fn sfx_leave(&mut self) {
        self.log.push(Ev::Unmute);
    }
    fn rider_leave(&mut self) {
        self.log.push(Ev::Leave);
    }
}

// the wipe-out motion's rider fields
const PAIRS: [u32; 6] = [0x1f4, 0x200, 0x20c, 0x218, 0x248, 0x26c];

fn put_wipe(m: &mut Mem, at: u32, w: &WipeRider) {
    m.write_f32(at + 0x12c, w.time_scale);
    w4(m, at + 0x180, w.fwd);
    w4(m, at + 0x190, w.side);
    m.write_u32(at + 0x1b4, w.x1b4);
    m.write_u32(at + 0x1dc, w.x1dc);
    m.write_u32(at + 0x1f0, w.x1f0);
    for (k, o) in PAIRS.iter().enumerate() {
        m.write_f32(at + o, w.pairs[k].0);
        m.write_u32(at + o + 4, w.pairs[k].1);
    }
    m.write_f32(at + 0x294, w.x294);
    m.write_f32(at + 0x2e4, w.stuck);
    m.write_u32(at + 0x310, w.x310);
    w4(m, at + 0x360, w.course8);
    m.write_u32(at + 0x418, w.player as u32);
    m.write_u32(at + 0x420, w.x420);
    m.write_u32(at + 0x46d4, w.x46d4);
    m.write_f32(at + 0x46e0, w.x46e0);
    m.write_f32(at + 0x57b8, w.t_land);
    w4(m, at + 0x57d0, w.land_v);
    w4(m, at + 0x57e0, w.land_n);
    m.write_u32(at + 0x57f0, w.land_surface);
}

fn wipe_bits(m: &mut Mem, at: u32) -> Vec<u32> {
    let mut v = vec![m.read_u32(at + 0x12c)];
    for o in [0x180u32, 0x190, 0x360, 0x57d0, 0x57e0] {
        v.extend(b4(r4(m, at + o)));
    }
    for o in [0x1b4u32, 0x1dc, 0x1f0, 0x294, 0x2e4, 0x310, 0x418, 0x420, 0x46d4, 0x46e0, 0x57b8, 0x57f0] {
        v.push(m.read_u32(at + o));
    }
    for o in PAIRS {
        v.push(m.read_u32(at + o));
        v.push(m.read_u32(at + o + 4));
    }
    v
}

fn wipe_ours(w: &WipeRider) -> Vec<u32> {
    let mut v = vec![w.time_scale.to_bits()];
    for a in [w.fwd, w.side, w.course8, w.land_v, w.land_n] {
        v.extend(b4(a));
    }
    v.extend([w.x1b4, w.x1dc, w.x1f0, w.x294.to_bits(), w.stuck.to_bits(), w.x310, w.player as u32, w.x420, w.x46d4, w.x46e0.to_bits(), w.t_land.to_bits(), w.land_surface]);
    for p in w.pairs {
        v.push(p.0.to_bits());
        v.push(p.1);
    }
    v
}

#[test]
fn wipeout_motion_matches_the_original() {
    let Some(H { mut r, log, casts, queries, lines, touch, ev1, rider, vt, motion, objs, .. }) = harness() else { return };
    for a in [0x10c198, 0x10c2f0, 0x10d680, 0x10f178, 0x10f9a0, 0x1102c8, 0x250fd0, 0x2508b8, 0x250a48, 0x250c10, 0x250d60, 0x250e98, 0x102e80, 0x102f80, 0x1108d0, 0x10c120] {
        r.allow(a);
    }
    let clip: Rc<RefCell<(u32, u32, V4)>> = Rc::new(RefCell::new((0, 0, [0.0; 4])));
    let probes: Rc<RefCell<VecDeque<bool>>> = Rc::new(RefCell::new(VecDeque::new()));
    let predict: Rc<RefCell<(f32, V4, V4, u32)>> = Rc::new(RefCell::new((0.0, [0.0; 4], [0.0; 4], 0)));
    let c = clip.clone();
    r.stub(0x15fdd0, move |cpu, _| cpu.set_gpr(2, c.borrow().0 as u64));
    let c = clip.clone();
    r.stub(0x15eec0, move |cpu, _| cpu.set_gpr(2, c.borrow().1 as u64));
    let l = log.clone();
    r.stub(0x15fcf8, move |cpu, _| {
        assert_eq!((cpu.gpr(6), cpu.gpr(7)), (0, 5));
        l.borrow_mut().push(Ev::Play(cpu.gpr(5) as u32))
    });
    let l = log.clone();
    r.stub(0x15f050, move |_, _| l.borrow_mut().push(Ev::BlendOut));
    let (l, c) = (log.clone(), clip.clone());
    r.stub(0x15fa40, move |cpu, m| {
        l.borrow_mut().push(Ev::Disp);
        w4(m, cpu.gpr(4) as u32, c.borrow().2);
    });
    let (l, p) = (log.clone(), probes.clone());
    r.stub(0x128ae8, move |cpu, _| {
        l.borrow_mut().push(Ev::Probe);
        cpu.set_gpr(2, p.borrow_mut().pop_front().unwrap() as u64);
    });
    let l = log.clone();
    r.stub(0x128e90, move |cpu, m| l.borrow_mut().push(Ev::Rotate(cpu.f(12).to_bits(), b4(r4(m, cpu.gpr(5) as u32)))));
    let l = log.clone();
    r.stub(0x125088, move |cpu, m| l.borrow_mut().push(Ev::Collide(m.read_u32(cpu.gpr(4) as u32 + 0x494) as i32)));
    let l = log.clone();
    r.stub(0x126250, move |_, _| l.borrow_mut().push(Ev::Wall));
    let l = log.clone();
    r.stub(0x1267a0, move |cpu, m| l.borrow_mut().push(Ev::Translate(b4(r4(m, cpu.gpr(5) as u32)))));
    for (a, takeoff) in [(0x123b10u32, true), (0x123ec8, false)] {
        let (l, p) = (log.clone(), predict.clone());
        r.stub(a, move |cpu, m| {
            l.borrow_mut().push(Ev::Predict(takeoff));
            let at = cpu.gpr(4) as u32 - 0x57b0;
            let (t, v, n, s) = *p.borrow();
            m.write_f32(at + 0x57b8, t);
            w4(m, at + 0x57d0, v);
            w4(m, at + 0x57e0, n);
            m.write_u32(at + 0x57f0, s);
        });
    }
    let l = log.clone();
    r.stub(0x123f00, move |_, _| l.borrow_mut().push(Ev::PredictReset));
    let l = log.clone();
    r.stub(0x11f610, move |cpu, _| l.borrow_mut().push(Ev::Motion(cpu.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x11f608, move |cpu, _| l.borrow_mut().push(Ev::State(cpu.gpr(5) as u32)));
    for (a, e) in [(0x218318u32, Ev::Mute), (0x218328, Ev::Impact2), (0x218700, Ev::Slide), (0x218b20, Ev::Stop), (0x218b68, Ev::Unmute)] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(e.clone()));
    }
    let leave_fn = r.mem.alloc(0x10);
    let l = log.clone();
    r.stub(leave_fn, move |_, _| l.borrow_mut().push(Ev::Leave));
    r.mem.write_u32(vt + 0x98, 0);
    r.mem.write_u32(vt + 0x9c, leave_fn);
    let x464 = r.mem.alloc(0x50);
    r.mem.write_u32(rider + 0x464, x464);
    let app = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    let table = r.mem.alloc(100 * 16);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, race);
    r.mem.write_u32(race + 0x24, table);
    let getup = r.mem.alloc(0x20);
    r.mem.write_u32(getup + 0xc, rider);
    let mut g = Rng(0x0b1d_e0d1_0000_0001);
    let mut n = [0usize; 6];
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for case in 0..40000 {
        let pos = [g.f(30000.0), g.f(30000.0), g.f(5000.0), 1.0];
        let mut body = CollBody::new();
        body.init_rider(1.2);
        for (k, s) in body.spheres.iter_mut().enumerate() {
            s.0 = [pos[0] + g.f(40.0), pos[1] + g.f(40.0), pos[2] + 20.0 + 6.0 * k as f32 + g.f(40.0), 1.0];
        }
        body.center = [pos[0], pos[1], pos[2] + 80.0, 1.0];
        let ground = if g.next() % 2 == 0 { [0.0f32, 0.0, 1.0, 0.0] } else { g.unit() };
        let mut rr = RagRider {
            id: rider,
            pos,
            vel: g.v(2000.0),
            q: g.quat(),
            up: g.unit(),
            height: g.f(30.0),
            surface: g.pick(&[1u32, 6, 10, 2]),
            ground,
            fall: g.unit(),
            ground_at: g.v(100.0),
            ground_kind: g.pick(&[3u32, 0, 1]),
            crash_n: g.unit(),
            side: g.unit(),
            course: [pos[0] + g.f(3000.0), pos[1] + g.f(3000.0), pos[2] + g.f(500.0), 1.0],
            body,
            hip: [g.f(30.0), g.f(30.0), g.f(100.0), 0.0],
        };
        let mut w = WipeRider {
            time_scale: g.pick(&[1.0f32, 0.5, 2.0]),
            fwd: g.unit(),
            side: g.unit(),
            x1b4: g.next() % 2,
            x1dc: 7,
            x1f0: 9,
            pairs: [(1.5, 3); 6],
            x294: g.pick(&[0.0f32, 20.0, -15.0]),
            stuck: 0.0,
            x310: 4,
            course8: [pos[0] + g.f(3000.0), pos[1] + g.f(3000.0), pos[2] + g.f(300.0), 1.0],
            player: g.pick(&[-1i32, 1]),
            x420: g.pick(&[2u32, 0]),
            x464: g.next() % 2 == 0,
            x46d4: 0,
            x46e0: 0.25,
            t_land: g.pick(&[0.05f32, 0.5]),
            land_v: g.v(2000.0),
            land_n: g.unit(),
            land_surface: g.pick(&[1u32, 6]),
        };
        let mut wo = Wipeout { rag: Ragdoll::default(), t: g.pick(&[0.0f32, 0.6, 3.0, 7.5]), t_ground: g.pick(&[0.0f32, 0.05, 0.2, 0.3, 3.5]), stick: [g.pick(&[0.0f32, 0.7, -0.2]), g.pick(&[0.0f32, -0.9, 0.3])], x30: [0.0; 4], speed: 0.0 };
        wo.rag.t_air = g.pick(&[0.0f32, 0.05, 0.12, 0.3]);
        wo.rag.crash_at = [pos[0] + g.f(80.0), pos[1] + g.f(80.0), pos[2] + g.f(80.0), 1.0];
        wo.rag.crash_n = g.unit();
        wo.rag.init(&mut rr.clone());
        wo.rag.rb.l = g.v(2.0e5);
        wo.rag.rb.w = g.v(8.0);
        if g.next() % 3 == 0 {
            // upright on the snow, facing the course, hardly turning: the ways straight out of the wipe-out
            let up = [g.f(0.2), g.f(0.2), 1.0];
            let l = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
            let up = [up[0] / l, up[1] / l, up[2] / l, 0.0];
            rr.up = up;
            rr.ground = if g.next() % 2 == 0 { up } else { [0.0, 0.0, 1.0, 0.0] };
            rr.surface = g.pick(&[1u32, 2]);
            w.fwd = [1.0, g.f(0.1), 0.0, 0.0];
            w.course8 = [pos[0] + g.pick(&[2000.0f32, -2000.0]), pos[1] + g.f(200.0), pos[2], 1.0];
            w.land_n = up;
            w.land_surface = 1;
            wo.stick = [0.0, 0.0];
            wo.rag.rb.l = g.v(100.0);
            wo.rag.rb.w = g.v(1.0);
        }
        put_rider(&mut r.mem, rider, &rr);
        put_wipe(&mut r.mem, rider, &w);
        r.mem.write_u32(x464 + 0x40, w.x464 as u32);
        r.mem.write_f32(motion + 8, wo.rag.t_air);
        r.mem.write_f32(motion + 0xc, wo.t);
        r.mem.write_f32(motion + 0x10, wo.t_ground);
        r.mem.write_f32(motion + 0x14, wo.stick[0]);
        r.mem.write_f32(motion + 0x18, wo.stick[1]);
        w4(&mut r.mem, motion + 0x20, wo.rag.crash_at);
        w4(&mut r.mem, motion + 0x30, wo.x30);
        w4(&mut r.mem, motion + 0x40, wo.rag.crash_n);
        r.mem.write_f32(motion + 0x50, wo.speed);
        put_rb(&mut r.mem, motion + 0x70, &wo.rag.rb);
        // the scripted world and animation
        let mut env = Env { mask: 0x273c3, ev1: -1, ..Default::default() };
        r.mem.write_u32(0x31d314, env.mask as u32);
        env.wmask = g.pick(&[0x1234i32, 77]);
        r.mem.write_u32(0x31d310, env.wmask as u32);
        *ev1.borrow_mut() = -1;
        for o in objs {
            let f = g.pick(&[0x80u32, 0]);
            r.mem.write_u32(o + 0xe8, f);
            env.flags.insert(o, f);
        }
        let hit = |g: &mut Rng, at: V4| -> (f32, CastOut) {
            let t = g.pick(&[0.0f32, -1.0, 0.5, 3.0]);
            (t, CastOut { kind: g.pick(&[0u32, 2, 1]), pos: [at[0] + g.f(60.0), at[1] + g.f(60.0), at[2] + g.f(60.0), 1.0], normal: g.unit(), x30: [g.next(), g.pick(&[1u32, 6, 3]), g.pick(&objs)] })
        };
        for _ in 0..2 {
            let h = hit(&mut g, wo.rag.rb.pos);
            env.casts.push_back(h);
        }
        let h = hit(&mut g, wo.rag.rb.pos);
        env.queries.push_back(h);
        let h = hit(&mut g, rr.pos);
        let h = if g.next() % 4 == 0 { (-1.0, h.1) } else { h };
        env.lines.push_back(h);
        env.clip = g.pick(&[0x2dc, 0x2dd, 0x2de, 0x2df, 0x2e0, 0x2e8]);
        env.clip_type = g.pick(&[0x11u32, 0x12]);
        env.disp = g.v(40.0);
        env.probes = (0..3).map(|_| g.next() % 2 == 0).collect();
        env.g = g.pick(&[1900.0f32, 980.0]);
        env.predict = (g.pick(&[0.05f32, 0.4]), g.v(2000.0), if g.next() % 2 == 0 { w.land_n } else { g.unit() }, g.pick(&[1u32, 6]));
        for k in 0..16u32 {
            r.mem.write_f32(table + 100 * k, env.g);
        }
        *casts.borrow_mut() = env.casts.clone();
        *queries.borrow_mut() = env.queries.clone();
        *lines.borrow_mut() = env.lines.clone();
        *touch.borrow_mut() = None;
        *clip.borrow_mut() = (env.clip, env.clip_type, env.disp);
        *probes.borrow_mut() = env.probes.clone();
        *predict.borrow_mut() = env.predict;
        r.mem.write_u32(rider + 0x700, 0);
        log.borrow_mut().clear();
        let which = g.next() % 6;
        let mut rollers: Vec<(u32, RollerBody)> = vec![];
        match which {
            0 => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10c198).unwrap();
                wo.enter(&mut rr, &mut w, &mut env);
            }
            1 | 2 => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10c2f0).unwrap();
                wo.update(&mut rr, &mut w, &mut rollers, &mut env);
            }
            3 => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10f178).unwrap();
                wo.start_get_up(&mut rr, &mut w, &mut env);
            }
            4 => {
                r.cpu.set_gpr(4, motion as u64);
                r.call(0x10f9a0).unwrap();
                play_impact_clip(&rr, &w, &mut env);
            }
            _ => {
                if g.next() % 2 == 0 {
                    r.cpu.set_gpr(4, getup as u64);
                    r.call(0x1102c8).unwrap();
                    get_up_update(&mut rr, &mut w, &mut env);
                } else {
                    let next = g.pick(&[6u32, 1]);
                    r.cpu.set_gpr(4, motion as u64);
                    r.cpu.set_gpr(5, next as u64);
                    r.call(0x10d680).unwrap();
                    wo.exit(next, &mut env);
                }
            }
        }
        let theirs = get_rider(&mut r.mem, rider, &rr);
        assert_eq!(rider_bits(&theirs), rider_bits(&rr), "case {case} ({which}): the rider\n theirs {theirs:?}\n ours   {rr:?}");
        assert_eq!(wipe_bits(&mut r.mem, rider), wipe_ours(&w), "case {case} ({which}): the rider's motion fields");
        let trb = get_rb(&mut r.mem, motion + 0x70);
        assert_eq!(rb_bits(&trb), rb_bits(&wo.rag.rb), "case {case} ({which}): the body\n theirs {trb:?}\n ours   {:?}", wo.rag.rb);
        let mf: Vec<u32> = [8u32, 0xc, 0x10, 0x14, 0x18, 0x50].iter().map(|o| r.mem.read_u32(motion + o)).collect();
        assert_eq!(mf, [wo.rag.t_air, wo.t, wo.t_ground, wo.stick[0], wo.stick[1], wo.speed].map(f32::to_bits).to_vec(), "case {case} ({which}): the motion's timers");
        for (o, v) in [(0x20u32, wo.rag.crash_at), (0x30, wo.x30), (0x40, wo.rag.crash_n)] {
            assert_eq!(b4(r4(&mut r.mem, motion + o)), b4(v), "case {case} ({which}): motion +{o:#x}");
        }
        let theirs_log = log.borrow().clone();
        assert_eq!(theirs_log, env.log, "case {case} ({which}): calls");
        for e in &env.log {
            let k = match e { Ev::Motion(_) | Ev::State(_) | Ev::Play(0x224 | 0x250 | 0x251) => format!("{e:?}"), _ => format!("{e:?}").split('(').next().unwrap().to_string() };
            *seen.entry(k).or_default() += 1;
        }
        n[which as usize] += 1;
    }
    eprintln!("cases {n:?}, calls {seen:?}");
}

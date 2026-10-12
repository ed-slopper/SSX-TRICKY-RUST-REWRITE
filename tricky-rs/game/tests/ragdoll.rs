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
const FWD: u32 = 0x1a0;
const HEIGHT: u32 = 0x1bc;
const X290: u32 = 0x290;
const GROUND: u32 = 0x2a0;
const FALL: u32 = 0x2b0;
const GROUND_AT: u32 = 0x2d0;
const SURFACE: u32 = 0x2e0;
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
    for (o, v) in [(POS, r.pos), (VEL, r.vel), (Q, r.q), (FWD, r.fwd), (GROUND, r.ground), (FALL, r.fall), (GROUND_AT, r.ground_at), (CRASH_N, r.crash_n), (SIDE, r.side), (COURSE, r.course), (HIP, r.hip)] {
        w4(m, at + o, v);
    }
    m.write_f32(at + HEIGHT, r.height);
    m.write_u32(at + X290, r.x290);
    m.write_u32(at + SURFACE, r.surface);
    put_body(m, at + BODY, &r.body);
}
fn get_rider(m: &mut Mem, at: u32, like: &RagRider) -> RagRider {
    let mut r = like.clone();
    r.pos = r4(m, at + POS);
    r.vel = r4(m, at + VEL);
    r.q = r4(m, at + Q);
    r.fwd = r4(m, at + FWD);
    r.ground = r4(m, at + GROUND);
    r.fall = r4(m, at + FALL);
    r.ground_at = r4(m, at + GROUND_AT);
    r.crash_n = r4(m, at + CRASH_N);
    r.side = r4(m, at + SIDE);
    r.course = r4(m, at + COURSE);
    r.height = m.read_f32(at + HEIGHT);
    r.x290 = m.read_u32(at + X290);
    r.surface = m.read_u32(at + SURFACE);
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
    for a in [r.pos, r.vel, r.q, r.fwd, r.ground, r.fall, r.ground_at, r.crash_n, r.side, r.course, r.body.center] {
        v.extend(b4(a));
    }
    v.extend([r.height.to_bits(), r.x290, r.surface, r.body.x24 as u32]);
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

#[test]
fn ragdoll_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
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

    let rider = r.mem.alloc(0x5000);
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
            fwd: g.unit(),
            height: g.f(50.0),
            x290: g.next(),
            ground: if g.next() % 2 == 0 { [0.0f32, 0.0, 1.0, 0.0] } else { g.unit() },
            fall: g.unit(),
            ground_at: g.v(100.0),
            surface: g.pick(&[3u32, 10, 1]),
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


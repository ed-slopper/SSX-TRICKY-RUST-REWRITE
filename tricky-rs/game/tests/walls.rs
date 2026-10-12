//! Walls and objects (board row E8b) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `Boarder_WallCollide`, `Boarder_WallImpactCheck`, `Boarder_InstanceCollisions` (half the time with a roller in the rider's contact list, its touch scripted:
//! `roller::rider_kick_touch`, row E8c2) and
//! `Boarder_InstanceBounce` on random riders, hits and objects, every callee stubbed and logged; the calls in
//! order with their arguments, the answer and the rider's fields compared bit for bit with `tricky_game::walls`.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_game::rigidbody::RigidBody;
use tricky_game::roller::{rider_kick_touch, RollerBody, RollerEnv};
use tricky_game::sphtree::{SphereTree, WorldSphTree};
use tricky_game::walls::{impact_check, instance_bounce, instance_collisions, wall_collide, CastRecord, Contact, Touched, WallEnv, WallRider};

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
    fn unit(&mut self) -> V4 {
        let v = [self.pick(&[0.0f32, 0.6, -0.8, 1.0, 0.28]), self.pick(&[0.0f32, 0.8, -0.6, -1.0, 0.96]), self.pick(&[0.0f32, 1.0, 0.3, -0.2, 0.7]), 0.0];
        let s = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if s == 0.0 {
            [0.0, 0.0, 1.0, 0.0]
        } else {
            v.map(|x| x / s)
        }
    }
    fn vec(&mut self, k: f32) -> V4 {
        [self.pick(&[0.0f32, 300.0, -1200.0, 2500.0, 45.0]) * k, self.pick(&[0.0f32, 800.0, -400.0, 90.0]) * k, self.pick(&[0.0f32, -500.0, 150.0]) * k, 0.0]
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Cast([u32; 4]),
    Query,
    Accumulate([u32; 4]),
    Translate([u32; 4]),
    Turn([u32; 4]),
    Wipeout([u32; 13]),
    Crash,
    Meter(u32),
    Stumble([u32; 4], u32),
    Speed,
    Knock(u32),
    Contact(u32, [u32; 13], [u32; 12]),
    SfxObject(u32),
    SfxWall,
    Touch,
    Script(i32, u32),
}

fn b4(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn r4(m: &mut Mem, a: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k))
}

fn rn<const N: usize>(m: &mut Mem, a: u32) -> [u32; N] {
    std::array::from_fn(|k| m.read_u32(a + 4 * k as u32))
}

fn contact_words(c: &Contact) -> [u32; 13] {
    let mut w = [0u32; 13];
    w[0..4].copy_from_slice(&b4(c.pos));
    w[4..8].copy_from_slice(&b4(c.dir));
    w[8..12].copy_from_slice(&b4(c.normal));
    w[12] = c.speed_in.to_bits();
    w
}

fn record_words(r: &CastRecord) -> [u32; 12] {
    let mut w = [0u32; 12];
    w[4..8].copy_from_slice(&b4(r.pos));
    w[8..12].copy_from_slice(&b4(r.normal));
    w
}

/// The scripted world and callees, shared by the runner's stubs and our env.
#[derive(Clone, Default)]
struct Script {
    cast: (f32, CastRecord),
    query: (f32, CastRecord, Touched),
    crash: f32,
    speed: f32,
    touch: Option<(V4, V4)>,
    ev1: i32,
    log: Vec<Call>,
}

struct Env(Script);

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
        self.0.ev1
    }
    fn start_script(&mut self, s: i32, o: u32) {
        self.0.log.push(Call::Script(s, o));
    }
    fn sfx(&mut self, _: u32, o: u32) {
        self.0.log.push(Call::SfxObject(o));
    }
}

/// A roller node (`cRollerNode`) in the runner's memory: its rigid body and what `Roller_OnHit` reads and writes.
fn put_roller(m: &mut Mem, at: u32, b: &RollerBody) {
    let rb = at + 0x30;
    m.write_f32(rb + 8, b.rb.inv_mass);
    for k in 0..9 {
        m.write_f32(rb + 0xc + 4 * k as u32, b.rb.inv_inertia[k]);
    }
    for (o, v) in [(0x30, b.rb.pos), (0x40, b.rb.q), (0x50, b.rb.p), (0x60, b.rb.l), (0x70, b.rb.v), (0x80, b.rb.w)] {
        for k in 0..4 {
            m.write_f32(rb + o + 4 * k, v[k as usize]);
        }
    }
    m.write_f32(at + 0xc8, b.energy);
    m.write_f32(at + 0xe4, b.awake);
    m.write_f32(at + 0xe8, b.knock);
    m.write_u32(at + 0x140, b.asleep);
}

fn roller_words(m: &mut Mem, at: u32) -> Vec<u32> {
    let mut v: Vec<u32> = (0..34).map(|k| m.read_u32(at + 0x38 + 4 * k)).collect();
    v.extend([0xc8u32, 0xe4, 0xe8, 0x140].map(|o| m.read_u32(at + o)));
    v
}

fn roller_ours(b: &RollerBody) -> Vec<u32> {
    let mut v = vec![b.rb.inv_mass.to_bits()];
    v.extend(b.rb.inv_inertia.map(f32::to_bits));
    for a in [b.rb.pos, b.rb.q, b.rb.p, b.rb.l, b.rb.v, b.rb.w] {
        v.extend(b4(a));
    }
    v.extend([b.energy.to_bits(), b.awake.to_bits(), b.knock.to_bits(), b.asleep]);
    v
}

impl WallEnv for Env {
    fn cast_walls(&mut self, r: &WallRider) -> (f32, CastRecord) {
        self.0.log.push(Call::Cast(b4(r.ground)));
        self.0.cast
    }
    fn query_objects(&mut self, _: &WallRider) -> (f32, CastRecord, Touched) {
        self.0.log.push(Call::Query);
        self.0.query
    }
    fn accumulate_impact(&mut self, _: &mut WallRider, n: V4) {
        self.0.log.push(Call::Accumulate(b4(n)));
    }
    fn translate(&mut self, _: &mut WallRider, by: V4) {
        self.0.log.push(Call::Translate(b4(by)));
    }
    fn turn_from_wall(&mut self, _: &mut WallRider, n: V4) {
        self.0.log.push(Call::Turn(b4(n)));
    }
    fn wipeout(&mut self, _: &mut WallRider, c: &Contact) {
        self.0.log.push(Call::Wipeout(contact_words(c)));
    }
    fn score_crash(&mut self, _: &mut WallRider) -> f32 {
        self.0.log.push(Call::Crash);
        self.0.crash
    }
    fn add_meter(&mut self, _: &mut WallRider, a: f32) {
        self.0.log.push(Call::Meter(a.to_bits()));
    }
    fn stumble(&mut self, _: &mut WallRider, d: V4, f: u32) {
        self.0.log.push(Call::Stumble(b4(d), f));
    }
    fn speed_stat(&mut self, _: &WallRider) -> f32 {
        self.0.log.push(Call::Speed);
        self.0.speed
    }
    fn knock(&mut self, _: &mut WallRider, a: f32) {
        self.0.log.push(Call::Knock(a.to_bits()));
    }
    fn contact(&mut self, o: u32, _: &mut WallRider, c: &Contact, rec: &CastRecord) {
        self.0.log.push(Call::Contact(o, contact_words(c), record_words(rec)));
    }
    fn sfx_object(&mut self, _: &WallRider, o: u32) {
        self.0.log.push(Call::SfxObject(o));
    }
    fn sfx_wall(&mut self, _: &WallRider) {
        self.0.log.push(Call::SfxWall);
    }
}

const POS: u32 = 0x140;
const VEL: u32 = 0x150;
const GROUND: u32 = 0x2a0;
const KNOCKS: u32 = 0x300;
const MODE: u32 = 0x424;
const STATE: u32 = 0x428;
const UP: u32 = 0x4ac0;

#[test]
fn walls_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x126250, 0x126540, 0x125088, 0x125a00, 0x102f50] {
        r.allow(a);
    }
    let script: Rc<RefCell<Script>> = Rc::new(RefCell::new(Script::default()));
    let rider = r.mem.alloc(0x5900);
    let vtable = r.mem.alloc(0x100);
    let knock_fn = r.mem.alloc(0x10);
    let object = r.mem.alloc(0x100);
    let phys = r.mem.alloc(0x20);
    let ctrl = r.mem.alloc(0x40);
    let cvt = r.mem.alloc(0x80);
    let fixed_fn = r.mem.alloc(0x10);
    let contact_buf = r.mem.alloc(0x60);
    let roller_obj = r.mem.alloc(0x100);
    r.mem.write_u32(rider, vtable);
    r.mem.write_u32(vtable + 0x68, 0);
    r.mem.write_u32(vtable + 0x6c, knock_fn);
    r.mem.write_u32(object + 0xec, phys);
    r.mem.write_u32(ctrl + 8, cvt);
    r.mem.write_u32(cvt + 0x50, 0);
    r.mem.write_u32(cvt + 0x54, fixed_fn);
    let s = script.clone();
    let log = move |c: Call| s.borrow_mut().log.push(c);
    let lg = Rc::new(log);
    let fixed = Rc::new(RefCell::new(true));
    // the shape's constructor and destructor
    r.stub(0x257810, |_, _| {});
    r.stub(0x257ca0, |_, _| {});
    let s = script.clone();
    let l = lg.clone();
    r.stub(0x25aff8, move |c, m| {
        let shape = c.gpr(5) as u32;
        l(Call::Cast(r4(m, shape + 0x2e0)));
        let (t, rec) = s.borrow().cast;
        let out = c.gpr(6) as u32;
        for k in 0..16 {
            m.write_u32(out + 4 * k, 0x5eed_0000 + k);
        }
        for k in 0..4 {
            m.write_f32(out + 0x10 + 4 * k, rec.pos[k as usize]);
            m.write_f32(out + 0x20 + 4 * k, rec.normal[k as usize]);
        }
        m.write_u32(out + 0x38, rec.object);
        c.set_f(0, t);
    });
    let s = script.clone();
    let l = lg.clone();
    r.stub(0x25b878, move |c, m| {
        l(Call::Query);
        let (t, rec, o) = s.borrow().query;
        let out = c.gpr(6) as u32;
        for k in 0..16 {
            m.write_u32(out + 4 * k, 0x5eed_0000 + k);
        }
        for k in 0..4 {
            m.write_f32(out + 0x10 + 4 * k, rec.pos[k as usize]);
            m.write_f32(out + 0x20 + 4 * k, rec.normal[k as usize]);
        }
        m.write_u32(out + 0x38, if o.id == 0 { 0 } else { object });
        c.set_f(0, t);
    });
    for (a, f) in [(0x125c48u32, 0u8), (0x1267a0, 1), (0x1257a0, 2)] {
        let l = lg.clone();
        r.stub(a, move |c, m| {
            let v = r4(m, c.gpr(5) as u32);
            l(match f {
                0 => Call::Accumulate(v),
                1 => Call::Translate(v),
                _ => Call::Turn(v),
            });
        });
    }
    let l = lg.clone();
    r.stub(0x11d830, move |c, m| l(Call::Wipeout(rn(m, c.gpr(5) as u32))));
    let (l, s) = (lg.clone(), script.clone());
    r.stub(0x156de0, move |c, _| {
        l(Call::Crash);
        c.set_f(0, s.borrow().crash);
    });
    let l = lg.clone();
    r.stub(0x11b018, move |c, _| l(Call::Meter(c.f(12).to_bits())));
    let l = lg.clone();
    r.stub(0x124dd0, move |c, m| l(Call::Stumble(r4(m, c.gpr(5) as u32), c.gpr(6) as u32)));
    let (l, s) = (lg.clone(), script.clone());
    r.stub(0x11d7b0, move |c, _| {
        l(Call::Speed);
        c.set_f(0, s.borrow().speed);
    });
    let l = lg.clone();
    r.stub(knock_fn, move |c, _| l(Call::Knock(c.f(12).to_bits())));
    let l = lg.clone();
    r.stub(0x13bd40, move |c, m| {
        let o = c.gpr(4) as u32;
        l(Call::Contact(if o == 0 { 0 } else { 1 }, rn(m, c.gpr(6) as u32), rn(m, c.gpr(7) as u32)))
    });
    let l = lg.clone();
    r.stub(0x216c50, move |c, _| {
        let o = c.gpr(6) as u32;
        l(Call::SfxObject(if o == object { 1 } else if o == roller_obj { 2 } else { o }))
    });
    let l = lg.clone();
    r.stub(0x2162f0, move |_, _| l(Call::SfxWall));
    let fx = fixed.clone();
    r.stub(fixed_fn, move |c, _| c.set_gpr(2, *fx.borrow() as u64));
    // a roller in the rider's contact list (row E8c2): the touch scripted, `Roller_OnHit` the game's
    let roller = r.mem.alloc(0x400);
    let entry = r.mem.alloc(0x10);
    r.mem.write_u32(roller + 8, 0x36d790);
    r.mem.write_u32(roller + 0x14, 0);
    r.mem.write_u32(roller + 0x28, roller_obj);
    r.mem.write_u32(roller + 0x344, roller);
    r.mem.write_u32(entry, roller + 0x2c0);
    r.mem.write_u32(entry + 4, rider + 0x700);
    r.mem.write_u32(entry + 8, 0);
    r.mem.write_u32(entry + 0xc, 0);
    for a in [0x13b7a8, 0x110838, 0x13f448, 0x149e28, 0x149e10, 0x154338] {
        r.allow(a);
    }
    let (l, s) = (lg.clone(), script.clone());
    r.stub(0x2394a8, move |c, m| {
        l(Call::Touch);
        let t = s.borrow().touch;
        if let Some((p, q)) = t {
            for k in 0..4 {
                m.write_f32(c.gpr(6) as u32 + 4 * k, p[k as usize]);
                m.write_f32(c.gpr(7) as u32 + 4 * k, q[k as usize]);
            }
        }
        c.set_gpr(2, t.is_some() as u64);
    });
    let s = script.clone();
    r.stub(0x25fe78, move |c, _| {
        assert_eq!(c.gpr(6), 1);
        c.set_gpr(2, s.borrow().ev1 as i64 as u64);
    });
    let pool = r.mem.alloc(0x100);
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, pool as u64));
    r.stub(0x2cc808, |_, _| {});
    let l = lg.clone();
    r.stub(0x13b8c0, move |c, _| l(Call::Script(c.gpr(6) as i32, if c.gpr(7) as u32 == roller_obj { 2 } else { 0 })));

    let mut g = Rng(0x0a11_5000_0000_0001);
    let mut counts = [0usize; 4];
    let mut rolled = 0usize;
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for case in 0..30000 {
        // the rider
        let vk = g.pick(&[1.0f32, 0.1, 3.0]);
        let mut wr = WallRider {
            pos: [g.vec(1.0)[0], g.vec(1.0)[1], g.vec(1.0)[2], 1.0],
            vel: g.vec(vk),
            ground: g.unit(),
            knocks: g.pick(&[0.0f32, 3.5]),
            mode: g.pick(&[0u32, 1, 2, 5, 3]),
            state: g.pick(&[0u32, 0x16, 4]),
            up: g.unit(),
        };
        for (o, v) in [(POS, wr.pos), (VEL, wr.vel), (GROUND, wr.ground), (UP, wr.up)] {
            for k in 0..4 {
                r.mem.write_f32(rider + o + 4 * k, v[k as usize]);
            }
        }
        r.mem.write_f32(rider + KNOCKS, wr.knocks);
        r.mem.write_u32(rider + MODE, wr.mode);
        r.mem.write_u32(rider + STATE, wr.state);
        r.mem.write_u32(rider + 0x700, 0);
        // the hits
        let rec = CastRecord { pos: g.vec(1.0), normal: g.unit(), object: g.pick(&[0u32, 0, 77]) };
        let t = g.pick(&[0.0f32, 0.3, 1.0, -1.0, 0.05, 2.5, 25.0]);
        let o = Touched {
            id: 1,
            flags: g.pick(&[0x80u32, 0x80, 0, 0x20]),
            mass: g.pick(&[0.0f32, 1e30, 50.0, 400.0]),
            bounce: g.pick(&[0.5f32, 0.0, 1.0]),
            controller: g.pick(&[None, Some(true), Some(false)]),
        };
        r.mem.write_u32(object + 0xe8, o.flags);
        r.mem.write_f32(phys, o.mass);
        r.mem.write_f32(phys + 4, o.bounce);
        r.mem.write_u32(object + 0xe4, if o.controller.is_some() { ctrl } else { 0 });
        *fixed.borrow_mut() = o.controller.unwrap_or(true);
        let sc = Script { cast: (t, rec), query: (t, rec, o), crash: g.pick(&[-1.0f32, -2.5]), speed: g.pick(&[150.0f32, 900.0]), touch: None, ev1: g.pick(&[-1i32, 12]), log: vec![] };
        *script.borrow_mut() = sc.clone();
        let mut env = Env(sc);
        let which = g.next() % 4;
        r.cpu.set_gpr(4, rider as u64);
        let got = match which {
            0 => {
                r.call(0x126250).unwrap();
                let ours = wall_collide(&mut wr, &mut env);
                assert_eq!(r.cpu.gpr(2) != 0, ours, "case {case}: wall answer");
                0
            }
            1 => {
                // half the time a roller in the contact list
                let mut ro = None;
                if g.next() % 2 == 0 {
                    let q = { let v = g.unit(); let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + 0.25).sqrt(); [0.5 / l, v[0] / l, v[1] / l, v[2] / l] };
                    let mut rb = RigidBody { inv_mass: g.pick(&[0.01f32, 0.2, 1.0]), pos: g.vec(1.0), q, p: g.vec(vk), l: g.vec(1.0), v: g.vec(vk), w: g.vec(0.01), ..Default::default() };
                    rb.inv_inertia = [0; 9].map(|_| g.pick(&[0.0f32, 1e-4, -3e-5, 2e-4]));
                    let tree = SphereTree { dirs: [[0.0; 4]; 8], centre: [0.0; 4], home: [0.0; 4], depth: 0, levels: vec![], masks: vec![] };
                    let b = RollerBody { rb, energy: 5.0, mass: 1.0, xd0: 0.0, xd4: 0.0, e: 0.0, mu: 0.5, bounce_speed: 0.0, awake: 3.0, knock: 0.25, xf0: [0.0; 4], body_inertia: [0.0; 9], com: [0.0; 4], asleep: (g.next() % 2) as u32, tree, lo: [0.0; 4], hi: [0.0; 4] };
                    put_roller(&mut r.mem, roller, &b);
                    let touch = if g.next() % 5 == 0 { None } else { let d = g.vec(1.0); Some((g.vec(0.05), [wr.pos[0] + d[0] * 0.3, wr.pos[1] + d[1] * 0.3, wr.pos[2] + d[2] * 0.3, g.pick(&[1.0f32, 0.0])])) };
                    script.borrow_mut().touch = touch;
                    env.0.touch = touch;
                    r.mem.write_u32(rider + 0x700, entry);
                    ro = Some(b);
                }
                r.call(0x125088).unwrap();
                instance_collisions(&mut wr, &mut env);
                if let Some(mut b) = ro {
                    let touch = env.0.touch;
                    env.0.log.push(Call::Touch);
                    rider_kick_touch(&mut b, 2, &mut wr, rider, touch, &mut env);
                    assert_eq!(roller_words(&mut r.mem, roller), roller_ours(&b), "case {case}: the roller");
                    rolled += 1;
                }
                1
            }
            2 => {
                let speed = g.pick(&[100.0f32, 900.0, 2500.0, 4000.0, 55.555553]);
                let c = Contact { pos: g.vec(1.0), dir: g.unit(), normal: g.unit(), speed_in: g.pick(&[3.0f32, 600.0]) };
                for (k, w) in contact_words(&c).iter().enumerate() {
                    r.mem.write_u32(contact_buf + 4 * k as u32, *w);
                }
                let n = g.unit();
                let nb = contact_buf + 0x40;
                for k in 0..4 {
                    r.mem.write_f32(nb + 4 * k, n[k as usize]);
                }
                r.cpu.set_f(12, speed);
                r.cpu.set_gpr(5, contact_buf as u64);
                r.cpu.set_gpr(6, nb as u64);
                r.call(0x126540).unwrap();
                impact_check(&mut wr, speed, &c, n, &mut env);
                2
            }
            _ => {
                let c = Contact { pos: g.vec(1.0), dir: g.unit(), normal: rec.normal, speed_in: 5.0 };
                for (k, w) in contact_words(&c).iter().enumerate() {
                    r.mem.write_u32(contact_buf + 4 * k as u32, *w);
                }
                let recb = contact_buf + 0x40;
                // the record: +0x20 its normal
                let rb = r.mem.alloc(0x40);
                for k in 0..4 {
                    r.mem.write_f32(rb + 0x10 + 4 * k, rec.pos[k as usize]);
                    r.mem.write_f32(rb + 0x20 + 4 * k, rec.normal[k as usize]);
                }
                let _ = recb;
                r.cpu.set_f(12, t);
                r.cpu.set_gpr(5, object as u64);
                r.cpu.set_gpr(6, contact_buf as u64);
                r.cpu.set_gpr(7, rb as u64);
                r.call(0x125a00).unwrap();
                instance_bounce(&mut wr, t, &o, &c, &rec, &mut env);
                3
            }
        };
        counts[got] += 1;
        for c in &script.borrow().log {
            *seen.entry(format!("{c:?}").split('(').next().unwrap().to_string()).or_default() += 1;
        }
        let theirs = script.borrow().log.clone();
        // the object's id in our log is the scripted one; the runner's stubs name it 1
        let ours: Vec<Call> = env
            .0
            .log
            .iter()
            .map(|c| match c {
                Call::Contact(o, cw, rw) => Call::Contact(if *o == 0 { 0 } else { 1 }, *cw, *rw),
                Call::SfxObject(o) => Call::SfxObject(if *o == 77 || *o == 2 { *o } else if *o == 0 { 0 } else { 1 }),
                c => c.clone(),
            })
            .collect();
        let norm = |v: &[Call]| -> Vec<Call> {
            v.iter()
                .map(|c| match c {
                    // the contact record's words past the point, direction, normal and speed are the game's stack
                    Call::Contact(o, cw, rw) => {
                        let mut r2 = *rw;
                        r2[..4].copy_from_slice(&[0; 4]);
                        Call::Contact(*o, *cw, r2)
                    }
                    c => c.clone(),
                })
                .collect()
        };
        assert_eq!(norm(&theirs), norm(&ours), "case {case} ({which}): calls");
        let m = &mut r.mem;
        assert_eq!(
            [r4(m, rider + POS), r4(m, rider + VEL)],
            [b4(wr.pos), b4(wr.vel)],
            "case {case} ({which}): position, velocity"
        );
        assert_eq!(m.read_u32(rider + KNOCKS), wr.knocks.to_bits(), "case {case} ({which}): knocks");
    }
    eprintln!("{counts:?} (walls, objects, impact checks, bounces), {rolled} with a roller; calls {seen:?}");
}

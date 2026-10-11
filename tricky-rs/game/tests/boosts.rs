//! The boost controllers (board row E9f) against the game: `cBoostNode`, `cRandomBoostNode`, `cTubeEndBoostNode`
//! and `cZBoostNode` built by their own constructors in the function runner (TRICKY_ELF; skipped without it), with
//! six riders overlapping their box or not, the mesh test (`WorldNode_RayCast`) and the random numbers scripted; then
//! frames of Update and the odd re-arm message on both sides, comparing the riders' positions and velocities, the
//! node's fields and the calls out.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boosts::{Boost, BoostCall, BoostEnv, BoostRider, Extra};

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
    Call(BoostCall),
    Rand(u32, u32),
}

struct Env {
    riders: Vec<BoostRider>,
    touching: Vec<(u32, bool)>,
    rand: VecDeque<f32>,
    choice: [u32; 6],
    log: Vec<Ev>,
}

impl BoostEnv for Env {
    fn touching(&self) -> Vec<(u32, bool)> {
        self.touching.clone()
    }
    fn riders(&self) -> u32 {
        self.riders.len() as u32
    }
    fn rider(&mut self, slot: u32) -> &mut BoostRider {
        &mut self.riders[slot as usize]
    }
    fn rand(&mut self, lo: f32, hi: f32) -> f32 {
        self.log.push(Ev::Rand(lo.to_bits(), hi.to_bits()));
        self.rand.pop_front().unwrap()
    }
    fn tube_choice(&self, slot: u32) -> u32 {
        self.choice[slot as usize]
    }
    fn call(&mut self, c: BoostCall) {
        self.log.push(Ev::Call(c));
    }
}

fn v4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| m.read_f32(at + 4 * k))
}

fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn read_boost(m: &mut r5900::Mem, node: u32, like: &Boost) -> Boost {
    let bytes = |m: &mut r5900::Mem, at: u32| {
        let mut b = [0u8; 6];
        m.read(at, &mut b);
        b
    };
    let dirs = |m: &mut r5900::Mem, at: u32| [0, 1, 2, 3, 4, 5].map(|i| v4(m, at + 16 * i));
    let extra = match &like.extra {
        Extra::Plain => Extra::Plain,
        Extra::Random { .. } => Extra::Random { corner: v4(m, node + 0x110), dirs: dirs(m, node + 0x120), pushed: bytes(m, node + 0x180) },
        Extra::TubeEnd { .. } => Extra::TubeEnd {
            points: [v4(m, node + 0x110), v4(m, node + 0x120), v4(m, node + 0x130)],
            targets: [m.read_f32(node + 0x1a0), m.read_f32(node + 0x1a4), m.read_f32(node + 0x1a8)],
            dirs: dirs(m, node + 0x140),
            pushed: bytes(m, node + 0x1ac),
        },
        Extra::Z { .. } => Extra::Z { height: m.read_f32(node + 0x5c), snap: m.read_f32(node + 0x60), inside: bytes(m, node + 0x64) },
    };
    let z = matches!(like.extra, Extra::Z { .. });
    Boost {
        dir: v4(m, node + 0x40),
        target: m.read_f32(node + 0x50),
        rate: m.read_f32(node + 0x54),
        rearm: if z { 0 } else { m.read_u32(node + 0x58) as i32 },
        left: if z { 0 } else { m.read_u32(node + 0x5c) as i32 },
        mode: if z { 0 } else { m.read_u32(node + 0x60) as i32 },
        extra,
    }
}

/// Compares two boosts bit for bit (NaN-safe).
fn same(a: &Boost, b: &Boost) -> bool {
    format!("{:?}", BoostBits(a)) == format!("{:?}", BoostBits(b))
}
struct BoostBits<'a>(&'a Boost);
impl std::fmt::Debug for BoostBits<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let b = self.0;
        write!(f, "{:x?} {:x} {:x} {} {} {} ", bits(b.dir), b.target.to_bits(), b.rate.to_bits(), b.rearm, b.left, b.mode)?;
        match &b.extra {
            Extra::Plain => write!(f, "plain"),
            Extra::Random { corner, dirs, pushed } => write!(f, "{:x?} {:x?} {:?}", bits(*corner), dirs.map(bits), pushed),
            Extra::TubeEnd { points, targets, dirs, pushed } => write!(f, "{:x?} {:x?} {:x?} {:?}", points.map(bits), targets.map(f32::to_bits), dirs.map(bits), pushed),
            Extra::Z { height, snap, inside } => write!(f, "{:x} {:x} {:?}", height.to_bits(), snap.to_bits(), inside),
        }
    }
}

#[test]
fn boosts_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let rand: Rc<RefCell<VecDeque<f32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let hits: Rc<RefCell<Vec<(u32, bool)>>> = Rc::new(RefCell::new(vec![]));
    for a in [0x13a840, 0x13aa48, 0x140838, 0x149cd0, 0x149cc8, 0x141360, 0x1415a0, 0x1415b0, 0x1418a8, 0x1419f8, 0x141a08, 0x140498] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    for a in [0x150670, 0x150818, 0x14fa70, 0x257ca0] {
        r.stub(a, |_, _| {});
    }
    // EAMem_Set(p, byte, n)
    r.stub(0x2cc808, |c, m| {
        let v = vec![c.gpr(5) as u8; c.gpr(6) as usize];
        m.write(c.gpr(4) as u32, &v).unwrap();
    });
    // cWorldBoarder(this, rider + 0x470, 1): keep the rider for the mesh test
    r.stub(0x257810, |c, m| m.write_u32(c.gpr(4) as u32, c.gpr(5) as u32 - 0x470));
    let h = hits.clone();
    r.stub(0x13aad0, move |c, m| {
        let rider = m.read_u32(c.gpr(5) as u32);
        let idx = m.read_u32(rider + 0x460);
        let hit = h.borrow().iter().find(|(i, _)| *i == idx).map(|x| x.1).unwrap_or(false);
        c.set_f(0, if hit { 3.5 } else { 0.0 });
    });
    let l = log.clone();
    r.stub(0x13a980, move |c, _| {
        assert_eq!(c.gpr(5), 1);
        l.borrow_mut().push(Ev::Call(BoostCall::Finish));
    });
    let l = log.clone();
    r.stub(0x11cae8, move |c, m| {
        // Boarder_PlaceAt(rider, &position, &velocity, 0) with rider+0x1b0 in f12, 0 in f13
        let rider = c.gpr(4) as u32;
        assert_eq!((c.gpr(7), c.f(12).to_bits(), c.f(13)), (0, m.read_u32(rider + 0x1b0), 0.0));
        assert_eq!((v4(m, c.gpr(5) as u32), v4(m, c.gpr(6) as u32)).0.map(f32::to_bits), v4(m, rider + 0x140).map(f32::to_bits));
        l.borrow_mut().push(Ev::Call(BoostCall::PlaceAt(m.read_u32(rider + 0x460))));
    });
    let (l, q) = (log.clone(), rand.clone());
    r.stub(0x23d4f8, move |c, _| {
        l.borrow_mut().push(Ev::Rand(c.f(12).to_bits(), c.f(13).to_bits()));
        let v = q.borrow_mut().pop_front().unwrap();
        c.set_f(0, v);
    });
    r.stub(0x110838, |c, m| {
        let first = m.read_u32(c.gpr(4) as u32);
        c.set_gpr(2, (first != c.gpr(5) as u32) as u64);
    });

    let node = r.mem.alloc(0x200);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x80);
    let gapp = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x40);
    let racers = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, race);
    r.mem.write_u32(race + 0x1c, racers);
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x600)).collect();
    let pairs = r.mem.alloc(0x100);
    let others = r.mem.alloc(0x900 * 8);

    let mut g = Rng(0xb005_7ed0_0c0f_fee1);
    let mut pushes = 0;
    for case in 0..4000 {
        let ty = [7u32, 0x10, 0x18, 0x12][case % 4];
        let mut a = [0u32; 20];
        a[0] = ty;
        a[1] = if ty == 0x12 { g.pick(&[0.5f32, 1.0, 3.0]).to_bits() } else { g.pick(&[0u32, 1, 2]) };
        a[2] = g.pick(&[0.0f32, 1.0 / 60.0, 0.05, 0.5, 10.0, 30.0]).to_bits();
        for k in 3..20 {
            a[k] = g.pick(&[0.0f32, 1.0, -1.0, 0.5, 30.0, 12.0, -250.0, 400.0, 0.7071, 2.5]).to_bits();
        }
        if ty != 0x12 {
            // rate and target as the courses have them (huge ones overflow, which VU0 clamps and IEEE does not)
            a[3] = g.pick(&[0.0f32, 0.5, 1.0, 2.5, 12.0, 30.0]).to_bits();
            a[4] = g.pick(&[0.0f32, 0.7, 1.0, 12.0, 30.0, -1.0]).to_bits();
        }
        if ty == 0x12 {
            a[6] = g.pick(&[100.0f32, 500.0, -50.0]).to_bits();
            a[7] = g.pick(&[10.0f32, 200.0, 1000.0]).to_bits();
        }
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        for o in (0x2c..0x200).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        let n = 1 + g.next() % 6;
        r.mem.write_u32(racers + 0x88, n);
        let mut env = Env { riders: vec![], touching: vec![], rand: VecDeque::new(), choice: [0; 6], log: vec![] };
        for i in 0..n {
            let rp = riders[i as usize];
            r.mem.write_u32(racers + 0xc4 + 4 * i, rp);
            let br = BoostRider {
                index: i,
                pos: [g.pick(&[0.0f32, 100.0]), g.pick(&[0.0f32, -40.0]), g.pick(&[0.0f32, 90.0, 99.5, 450.0, 600.0, -100.0]), 1.0],
                vel: [g.pick(&[0.0f32, 1200.0, -300.0]), g.pick(&[0.0f32, 500.0]), g.pick(&[0.0f32, -800.0, 40.0]), 0.0],
                x424: g.pick(&[0, 5, 6, 1]),
            };
            r.mem.write_u32(rp + 0x460, i);
            r.mem.write_u32(rp + 0x424, br.x424 as u32);
            r.mem.write_u32(rp + 0x1b0, 0x7000 + i);
            for k in 0..4 {
                r.mem.write_f32(rp + 0x140 + 4 * k as u32, br.pos[k]);
                r.mem.write_f32(rp + 0x150 + 4 * k as u32, br.vel[k]);
            }
            env.riders.push(br);
            env.choice[i as usize] = g.next() % 3;
            r.mem.write_u32(0x31d338 + 4 * i, env.choice[i as usize]);
        }
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        let ctor = match ty {
            7 => {
                r.cpu.set_gpr(6, 7);
                r.cpu.set_gpr(7, object as u64);
                r.cpu.set_gpr(8, op as u64);
                0x140498
            }
            0x10 => 0x141208,
            0x18 => 0x141688,
            _ => 0x141af0,
        };
        if ty != 7 {
            r.cpu.set_gpr(6, object as u64);
            r.cpu.set_gpr(7, op as u64);
        }
        r.call(ctor).unwrap_or_else(|e| panic!("case {case} ctor: {e:?}"));
        let mut b = Boost::new(&a).unwrap();
        assert!(same(&read_boost(&mut r.mem, node, &b), &b), "case {case} constructed: {:?} vs {:?}", BoostBits(&read_boost(&mut r.mem, node, &b)), BoostBits(&b));
        let update = if ty == 0x12 { 0x141cd8 } else { 0x140688 };
        for frame in 0..12 {
            // who overlaps the box, and who the mesh hits
            let k = g.next() % 4;
            let mut head = 0;
            env.touching.clear();
            let mut touch = vec![];
            for j in 0..k {
                let p = pairs + 16 * j;
                let other = others + 0x900 * j;
                let owner = other + 0x82c;
                let (first, second) = if g.next() % 2 == 0 { (node + 0x70, other) } else { (other, node + 0x70) };
                r.mem.write_u32(p, first);
                r.mem.write_u32(p + 4, second);
                r.mem.write_u32(p + 8, head);
                r.mem.write_u32(p + 12, head);
                head = p;
                let slot = g.next() % n;
                let is_rider = g.next() % 4 != 0;
                r.mem.write_u32(other + 0x84, owner);
                r.mem.write_u32(owner + 0x14, if is_rider { 0x3ef } else { 0x3e9 });
                r.mem.write_u32(other, riders[slot as usize]);
                if is_rider {
                    touch.push(slot);
                }
            }
            r.mem.write_u32(node + 0x70, head);
            // the walk goes from the head: the last pair written first
            let mut hitmap = vec![];
            for s in 0..n {
                hitmap.push((s, g.next() % 3 != 0));
            }
            *hits.borrow_mut() = hitmap.clone();
            for s in touch.iter().rev() {
                env.touching.push((*s, hitmap[*s as usize].1));
            }
            let rands: Vec<f32> = (0..20).map(|_| g.pick(&[0.0f32, 0.3, -0.7, 12.0, -3.0])).collect();
            env.rand = rands.iter().copied().collect();
            *rand.borrow_mut() = rands.iter().copied().collect();
            log.borrow_mut().clear();
            env.log.clear();
            if g.next() % 8 == 0 {
                r.cpu.set_gpr(4, node as u64);
                r.cpu.set_gpr(5, 1);
                if ty != 0x12 {
                    r.call(0x1408d0).unwrap();
                    b.on_message(1);
                }
            } else {
                r.cpu.set_gpr(4, node as u64);
                r.call(update).unwrap_or_else(|e| panic!("case {case} frame {frame}: {e:?}"));
                b.update(&mut env);
            }
            assert_eq!(*log.borrow(), env.log, "case {case} frame {frame} type {ty:#x}");
            for (i, br) in env.riders.iter().enumerate() {
                let rp = riders[i];
                assert_eq!((bits(v4(&mut r.mem, rp + 0x140)), bits(v4(&mut r.mem, rp + 0x150))), (bits(br.pos), bits(br.vel)), "case {case} frame {frame} type {ty:#x} rider {i}");
            }
            let got = read_boost(&mut r.mem, node, &b);
            assert!(same(&got, &b), "case {case} frame {frame} type {ty:#x}:\n{:?}\n{:?}", BoostBits(&got), BoostBits(&b));
            if env.log.iter().any(|e| matches!(e, Ev::Call(BoostCall::Finish))) {
                break;
            }
            pushes += env.touching.iter().filter(|t| t.1).count();
        }
    }
    eprintln!("{pushes} rider pushes compared");
}

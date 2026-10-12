//! `Boarder_RiderCollisions` 0x123fc8 against `tricky_game::ridercol::rider_collisions` (board row E8d4): the
//! original run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) over up to six
//! riders in memory, with every callee answered from one script and logged; the log, the riders' positions and
//! their records must come out equal, bit for bit.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::ridercol::{rider_collisions, ColEnv, ColRec, ColRider};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    fn unit(&mut self) -> [f32; 4] {
        let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        [v[0] / l, v[1] / l, v[2] / l, 0.0]
    }
    fn v(&mut self, k: f32) -> [f32; 4] {
        [self.f(-k, k), self.f(-k, k), self.f(-k, k), 0.0]
    }
}

const GAPP: u32 = 0x337c58;
const MODE: u32 = 0x32de8c;
const SND: u32 = 0x343050;

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

/// The script both sides answer from.
#[derive(Clone, Default)]
struct Script {
    overlap: Vec<Vec<Option<[f32; 4]>>>,
    fs: Vec<bool>,
    stat: Vec<f32>,
    hits: VecDeque<bool>,
    meter: Vec<f32>,
    clip: u32,
    flag1: bool,
    flag2: bool,
    log: Vec<String>,
}

struct Env<'a>(&'a mut Script);
impl ColEnv for Env<'_> {
    fn overlap(&mut self, a: usize, b: usize) -> Option<[f32; 4]> {
        let o = self.0.overlap[a][b];
        self.0.log.push(format!("overlap {a} {b} {:?}", o.map(bits)));
        o
    }
    fn finished_stopped(&mut self, r: usize) -> bool {
        self.0.fs[r]
    }
    fn translate(&mut self, r: usize, v: [f32; 4]) {
        self.0.log.push(format!("translate {r} {:?}", bits(v)));
    }
    fn speed_stat(&mut self, r: usize) -> f32 {
        self.0.stat[r]
    }
    fn rider_hit(&mut self, speed: f32, to: usize, from: usize, owner: usize, of: usize, dir: [f32; 4]) -> bool {
        let h = self.0.hits.pop_front().unwrap();
        self.0.log.push(format!("hit {:08x} {to} {from} rec {owner}/{of} {:?} -> {h}", speed.to_bits(), bits(dir)));
        h
    }
    fn score_knockdown(&mut self, r: usize) {
        self.0.log.push(format!("knockdown {r}"));
    }
    fn score_crash(&mut self, r: usize) {
        self.0.log.push(format!("crash {r}"));
    }
    fn crash_meter(&mut self, r: usize) {
        let m = self.0.meter[r];
        self.0.log.push(format!("meter {r} {:08x}", m.to_bits()));
    }
    fn rand_game(&mut self) {
        self.0.log.push("rand".into());
    }
    fn collision_sound(&mut self, hitter: usize, hit: usize) {
        self.0.log.push(format!("sound {hitter} {hit}"));
    }
    fn clip_type1(&mut self, _r: usize) -> u32 {
        self.0.clip
    }
    fn clip_flag(&mut self, _r: usize, layer: u32) -> bool {
        if layer == 1 { self.0.flag1 } else { self.0.flag2 }
    }
}

fn locate(base: &[u32], a: u32) -> usize {
    base.iter().position(|&x| a >= x && a < x + 0x5900).unwrap()
}

#[test]
fn rider_collisions_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x102f50u32, 0x2510f8] {
        r.allow(f);
    }
    let sc: Rc<RefCell<Script>> = Rc::default();
    let base: Rc<Vec<u32>> = Rc::new((0..6).map(|_| r.mem.alloc(0x5900)).collect());
    let nodes: Vec<(u32, u32, u32)> = (0..6).map(|_| (r.mem.alloc(0x10), r.mem.alloc(0x100), r.mem.alloc(0x900))).collect();
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    let stats = r.mem.alloc(0x100);
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x236a00, move |c, m| {
        let (a, o) = (locate(&b, c.gpr(4) as u32 - 0x470), locate(&b, c.gpr(5) as u32 - 0x470));
        let v = s.borrow().overlap[a][o];
        s.borrow_mut().log.push(format!("overlap {a} {o} {:?}", v.map(bits)));
        if let Some(v) = v {
            put4(m, c.gpr(6) as u32, v);
        }
        c.set_gpr(2, v.is_some() as u64);
    });
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x11d998, move |c, _| c.set_gpr(2, s.borrow().fs[locate(&b, c.gpr(4) as u32)] as u64));
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x1267a0, move |c, m| {
        let v = get4(m, c.gpr(5) as u32);
        s.borrow_mut().log.push(format!("translate {} {:?}", locate(&b, c.gpr(4) as u32), bits(v)));
    });
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x11d7b0, move |c, _| c.set_f(0, s.borrow().stat[locate(&b, c.gpr(4) as u32)]));
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x124920, move |c, m| {
        let h = s.borrow_mut().hits.pop_front().unwrap();
        let (to, from) = (locate(&b, c.gpr(4) as u32), locate(&b, c.gpr(5) as u32));
        let rec = c.gpr(6) as u32;
        let owner = locate(&b, rec);
        let of = ((rec - b[owner] - 0x40) / 0x20) as usize;
        let dir = get4(m, c.gpr(7) as u32);
        s.borrow_mut().log.push(format!("hit {:08x} {to} {from} rec {owner}/{of} {:?} -> {h}", c.f(12).to_bits(), bits(dir)));
        c.set_gpr(2, h as u64);
    });
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x11e9a0, move |c, _| s.borrow_mut().log.push(format!("knockdown {}", locate(&b, c.gpr(4) as u32))));
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x11e988, move |c, _| s.borrow_mut().log.push(format!("crash {}", locate(&b, c.gpr(4) as u32))));
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x156de0, move |c, _| {
        let k = locate(&b, c.gpr(4) as u32 - 0x5820);
        c.set_f(0, s.borrow().meter[k]);
    });
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x11b018, move |c, _| s.borrow_mut().log.push(format!("meter {} {:08x}", locate(&b, c.gpr(4) as u32), c.f(12).to_bits())));
    let s = sc.clone();
    r.stub(0x23d4d8, move |_, _| s.borrow_mut().log.push("rand".into()));
    let (s, b) = (sc.clone(), base.clone());
    r.stub(0x216960, move |c, _| s.borrow_mut().log.push(format!("sound {} {}", locate(&b, c.gpr(5) as u32), locate(&b, c.gpr(6) as u32))));
    r.stub(0x233cb8, |_, _| {});
    let s = sc.clone();
    r.stub(0x15eec0, move |c, _| c.set_gpr(2, s.borrow().clip as u64));
    let s = sc.clone();
    r.stub(0x15ffb8, move |c, _| {
        let b = s.borrow();
        c.set_gpr(2, (if c.gpr(5) == 1 { b.flag1 } else { b.flag2 }) as u64)
    });
    let b = base.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, b[c.gpr(5) as usize] as u64));
    r.stub(0x110838, |c, _| c.set_gpr(2, 0));

    let mut g = Rng(0x7a3c_91e5_0b42_d8f6);
    let mut seen = [0u32; 8];
    for case in 0..30_000 {
        let n = 2 + g.next() as usize % 5;
        let me = g.next() as usize % n;
        let tick = 20 + (g.next() % 900) as i32;
        let practice = g.next() % 3 == 0;
        let mut riders: Vec<ColRider> = (0..n)
            .map(|_| ColRider {
                pos: g.v(300.0),
                vel: g.v(1500.0),
                motion: [1, 2, 3, 4, 5, 6][g.next() as usize % 6],
                heading: g.f(-7.0, 7.0),
                fwd: g.unit(),
                side: g.unit(),
                up: g.unit(),
                at: g.v(160.0),
                stability: g.next() as u8,
                rec: (0..n)
                    .map(|_| {
                        let edge = [tick - 4, tick - 3, tick - 2, tick, (g.next() % tick as u32) as i32][g.next() as usize % 5];
                        let edge2 = [tick - 4, tick - 3, tick - 2, tick, (g.next() % tick as u32) as i32][g.next() as usize % 5];
                        ColRec { bump: edge, shove: edge2, hits: g.next() as u16 }
                    })
                    .collect(),
            })
            .collect();
        let others: Vec<usize> = (0..n).filter(|&k| k != me).collect();
        let list: Vec<usize> = others.iter().copied().filter(|_| g.next() % 3 != 0).collect();
        let mut script = Script {
            overlap: (0..n).map(|_| (0..n).map(|_| if g.next() % 3 == 0 { None } else { Some(g.v(40.0)) }).collect()).collect(),
            fs: (0..n).map(|_| g.next() % 4 == 0).collect(),
            stat: (0..n).map(|_| g.f(0.3, 3.0)).collect(),
            hits: (0..40).map(|_| g.next() % 2 == 0).collect(),
            meter: (0..n).map(|_| g.f(-1.0, 1.0)).collect(),
            clip: if g.next() % 4 == 0 { 0x14 } else { 0x15 },
            flag1: g.next() % 5 != 0,
            flag2: g.next() % 4 == 0,
            log: Vec::new(),
        };
        if g.next() % 2 == 0 {
            // make the shove's reach likely: the other's spot close to mine
            for k in 0..n {
                riders[k].at = [g.f(-40.0, 40.0), g.f(-40.0, 40.0), g.f(-30.0, 30.0), 0.0];
            }
        }
        // memory
        r.mem.write_u32(race + 0x18, tick as u32);
        r.mem.write_u32(race + 0x88, n as u32);
        r.mem.write_u32(MODE, if practice { 0 } else { 1 + g.next() % 5 });
        r.mem.write_u32(SND, 0x1234);
        for (k, rd) in riders.iter().enumerate() {
            let b = base[k];
            r.mem.write_u32(b + 0x460, k as u32);
            r.mem.write_u32(b + 0x464, stats);
            put4(&mut r.mem, b + 0x140, rd.pos);
            put4(&mut r.mem, b + 0x150, rd.vel);
            r.mem.write_u32(b + 0x424, rd.motion);
            r.mem.write_f32(b + 0x1e0, rd.heading);
            put4(&mut r.mem, b + 0x4aa0, rd.fwd);
            put4(&mut r.mem, b + 0x4ab0, rd.side);
            put4(&mut r.mem, b + 0x4ac0, rd.up);
            put4(&mut r.mem, b + 0x4ad0, rd.at);
            for (j, rc) in rd.rec.iter().enumerate() {
                let a = b + 0x40 + 0x20 * j as u32;
                r.mem.write_u32(a + 8, rc.bump as u32);
                r.mem.write_u32(a + 0xc, rc.shove as u32);
                r.mem.write_u32(a + 0x18, (rc.hits as u32) << 16 | 0x55);
            }
        }
        let st = (riders[me].stability) as u32;
        r.mem.write_u32(stats + 0x12 - (0x12 & 3), st << (8 * (0x12 & 3)));
        // the neighbour list
        let mut first = 0;
        for (k, &o) in list.iter().enumerate().rev() {
            let (node, obj, blk) = nodes[k];
            let rec = blk + 0x82c;
            r.mem.write_u32(blk, base[o]);
            r.mem.write_u32(rec + 0x14, 0x3ef);
            r.mem.write_u32(obj + 0x84, rec);
            r.mem.write_u32(node + 4, obj);
            r.mem.write_u32(node + 8, first);
            first = node;
        }
        r.mem.write_u32(base[me] + 0x700, first);
        *sc.borrow_mut() = script.clone();
        r.cpu.set_gpr(4, base[me] as u64);
        r.call(0x123fc8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let got_log = std::mem::take(&mut sc.borrow_mut().log);
        rider_collisions(me, &list, &mut riders, tick, practice, &mut Env(&mut script));
        assert_eq!(got_log, script.log, "case {case}: log (me {me} list {list:?} tick {tick})");
        for (k, rd) in riders.iter().enumerate() {
            let b = base[k];
            assert_eq!(bits(get4(&mut r.mem, b + 0x140)), bits(rd.pos), "case {case}: rider {k} pos");
            for (j, rc) in rd.rec.iter().enumerate() {
                let a = b + 0x40 + 0x20 * j as u32;
                assert_eq!(r.mem.read_u32(a + 8) as i32, rc.bump, "case {case}: rec {k}/{j} bump");
                assert_eq!(r.mem.read_u32(a + 0xc) as i32, rc.shove, "case {case}: rec {k}/{j} shove");
                assert_eq!(r.mem.read_u32(a + 0x18) >> 16, rc.hits as u32, "case {case}: rec {k}/{j} hits");
            }
        }
        for l in &got_log {
            let k = l.split(' ').next().unwrap();
            let i = ["overlap", "translate", "hit", "knockdown", "meter", "rand", "sound", "crash"].iter().position(|x| *x == k).unwrap();
            seen[i] += 1;
        }
    }
    assert!(seen.iter().all(|&n| n > 200), "{seen:?}");
}

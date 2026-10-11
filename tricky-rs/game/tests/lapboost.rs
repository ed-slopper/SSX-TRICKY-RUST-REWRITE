//! The lap boost (board row E9f2) against the game: `cLapBoostNode` built by its own constructor in the function
//! runner (TRICKY_ELF; skipped without it), then frames of Update with up to six riders overlapping its box or not,
//! on rails or slowed or riding, low or high over the gate, comparing the riders, their rail objects, the side
//! choices (0x31d338) and the node's counts with `tricky_game::boosts::LapBoost`.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boosts::{LapBoost, LapEnv, LapObject, LapRider};

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
    riders: Vec<LapRider>,
    touching: Vec<u32>,
    choice: [u32; 6],
}

impl LapEnv for Env {
    fn touching(&self) -> Vec<u32> {
        self.touching.clone()
    }
    fn riders(&self) -> u32 {
        self.riders.len() as u32
    }
    fn rider(&mut self, slot: u32) -> &mut LapRider {
        &mut self.riders[slot as usize]
    }
    fn set_tube_choice(&mut self, slot: u32, side: u32) {
        self.choice[slot as usize] = side;
    }
}

fn v4(m: &mut r5900::Mem, at: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| m.read_u32(at + 4 * k))
}
fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for k in 0..4 {
        m.write_f32(at + 4 * k as u32, v[k]);
    }
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[test]
fn lap_boost_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x13a840, 0x1814a8] {
        r.allow(a);
    }
    r.stub(0x17d0e0, |c, m| m.write_u32(c.gpr(4) as u32 + 0x14, c.gpr(6) as u32));
    for a in [0x150670, 0x150818, 0x14fa70] {
        r.stub(a, |_, _| {});
    }
    r.stub(0x110838, |c, m| {
        let first = m.read_u32(c.gpr(4) as u32);
        c.set_gpr(2, (first != c.gpr(5) as u32) as u64);
    });

    let node = r.mem.alloc(0x100);
    let object = r.mem.alloc(0x100);
    let op = r.mem.alloc(0x40);
    let gapp = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x40);
    let racers = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, gapp);
    r.mem.write_u32(gapp + 0x730, race);
    r.mem.write_u32(race + 0x1c, racers);
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x6000)).collect();
    let rails: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x600)).collect();
    let pairs = r.mem.alloc(0x100);
    let others = r.mem.alloc(0x900 * 8);

    let mut g = Rng(0x1a9b_0057_2468_0ace);
    let mut compared = 0;
    for case in 0..3000 {
        let a: Vec<u32> = vec![
            15,
            g.pick(&[0.5f32, 1.0, 2.5, 12.0]).to_bits(),
            g.pick(&[0.0f32, 12.0, 30.0]).to_bits(),
            g.pick(&[0.0f32, 1.0, 0.7071, -1.0]).to_bits(),
            g.pick(&[0.0f32, 1.0, -0.7071]).to_bits(),
            g.pick(&[0.0f32, 0.25]).to_bits(),
        ];
        for (k, w) in a.iter().enumerate() {
            r.mem.write_u32(op + 8 + 4 * k as u32, *w);
        }
        for o in (0x2c..0x100).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        // the gate: a box and a matrix
        let lo = [g.pick(&[-500.0f32, 0.0]), g.pick(&[-300.0f32, 100.0]), g.pick(&[0.0f32, 200.0])];
        let hi = [lo[0] + g.pick(&[400.0f32, 1600.0]), lo[1] + g.pick(&[600.0f32, 2400.0]), lo[2] + 300.0];
        let rows = [[g.pick(&[1.0f32, 0.0, -0.6]), g.pick(&[0.0f32, 0.8, -1.0]), g.pick(&[0.0f32, 0.3]), 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [g.pick(&[0.0f32, 50.0]), 10.0, 0.0, 1.0]];
        for (k, row) in rows.iter().enumerate() {
            put4(&mut r.mem, object + 16 * k as u32, *row);
        }
        for k in 0..3 {
            r.mem.write_f32(object + 0xcc + 4 * k as u32, lo[k]);
            r.mem.write_f32(object + 0xd8 + 4 * k as u32, hi[k]);
        }
        let obj = LapObject { min: lo, max: hi, rows };
        let n = 1 + g.next() % 6;
        r.mem.write_u32(racers + 0x88, n);
        let mut env = Env { riders: vec![], touching: vec![], choice: [9; 6] };
        for i in 0..n {
            let rp = riders[i as usize];
            r.mem.write_u32(racers + 0xc4 + 4 * i, rp);
            let lr = LapRider {
                index: i,
                lap: g.pick(&[0, 1, 2, 3]),
                x424: g.pick(&[0, 5, 6, 1]),
                pos: [g.pick(&[lo[0] - 200.0, lo[0] + 300.0, hi[0] + 900.0]), g.pick(&[lo[1], lo[1] + 400.0, hi[1] + 1200.0]), g.pick(&[lo[2] + 50.0, lo[2] + 1500.0]), 1.0],
                vel: [g.pick(&[0.0f32, 1200.0, -300.0]), g.pick(&[0.0f32, 500.0]), g.pick(&[0.0f32, -800.0]), 0.0],
                rail_len: g.pick(&[100.0f32, 2500.0, 0.5]),
                rail_rate: [0.0; 4],
                rail_vel: [0.0; 4],
            };
            r.mem.write_u32(rp + 0x460, i);
            r.mem.write_u32(rp + 0x114, lr.lap as u32);
            r.mem.write_u32(rp + 0x424, lr.x424 as u32);
            r.mem.write_u32(rp + 0x5ae0, rails[i as usize]);
            r.mem.write_f32(rails[i as usize] + 0x568, lr.rail_len);
            put4(&mut r.mem, rails[i as usize] + 0x5b0, [0.0; 4]);
            put4(&mut r.mem, rails[i as usize] + 0x5d0, [0.0; 4]);
            put4(&mut r.mem, rp + 0x140, lr.pos);
            put4(&mut r.mem, rp + 0x150, lr.vel);
            r.mem.write_u32(0x31d338 + 4 * i, 9);
            env.riders.push(lr);
        }
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, 3);
        r.cpu.set_gpr(6, object as u64);
        r.cpu.set_gpr(7, op as u64);
        r.call(0x140948).unwrap();
        let laps: Vec<i32> = env.riders.iter().map(|x| x.lap).collect();
        let mut b = LapBoost::new(&a, &laps);
        let read = |m: &mut r5900::Mem| -> ([u32; 4], u32, u32, [i16; 6], [i16; 6]) {
            let mut h = [[0i16; 6]; 2];
            for (j, at) in [0x58u32, 0x64].iter().enumerate() {
                for i in 0..6 {
                    let mut x = [0u8; 2];
                    m.read(node + at + 2 * i as u32, &mut x);
                    h[j][i] = i16::from_le_bytes(x);
                }
            }
            (v4(m, node + 0x40), m.read_u32(node + 0x50), m.read_u32(node + 0x54), h[0], h[1])
        };
        let mine = |b: &LapBoost, n: u32| {
            let mut l = b.laps;
            let mut ins = b.inside;
            for i in n as usize..6 {
                l[i] = 0;
                ins[i] = 0;
            }
            (bits(b.dir), b.target.to_bits(), b.rate.to_bits(), l, ins)
        };
        assert_eq!(read(&mut r.mem), mine(&b, n), "case {case} constructed");
        for frame in 0..10 {
            // a rider may move about, change lap or motion between frames
            for i in 0..n as usize {
                if g.next() % 4 == 0 {
                    let rp = riders[i];
                    let lr = &mut env.riders[i];
                    lr.lap = g.pick(&[0, 1, 2, 3]);
                    lr.x424 = g.pick(&[0, 5, 6]);
                    lr.pos[2] = g.pick(&[lo[2] + 50.0, lo[2] + 1500.0]);
                    r.mem.write_u32(rp + 0x114, lr.lap as u32);
                    r.mem.write_u32(rp + 0x424, lr.x424 as u32);
                    put4(&mut r.mem, rp + 0x140, lr.pos);
                }
            }
            let k = g.next() % 5;
            let mut head = 0;
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
                let is_rider = g.next() % 5 != 0;
                r.mem.write_u32(other + 0x84, owner);
                r.mem.write_u32(owner + 0x14, if is_rider { 0x3ef } else { 0x3e9 });
                r.mem.write_u32(other, riders[slot as usize]);
                if is_rider {
                    touch.push(slot);
                }
            }
            r.mem.write_u32(node + 0x70, head);
            env.touching = touch.into_iter().rev().collect();
            r.cpu.set_gpr(4, node as u64);
            r.call(0x140b90).unwrap_or_else(|e| panic!("case {case} frame {frame}: {e:?}"));
            b.update(&obj, &mut env);
            for i in 0..n as usize {
                let rp = riders[i];
                let lr = env.riders[i];
                assert_eq!((v4(&mut r.mem, rp + 0x140), v4(&mut r.mem, rp + 0x150)), (bits(lr.pos), bits(lr.vel)), "case {case} frame {frame} rider {i}");
                assert_eq!((v4(&mut r.mem, rails[i] + 0x5b0), v4(&mut r.mem, rails[i] + 0x5d0)), (bits(lr.rail_rate), bits(lr.rail_vel)), "case {case} frame {frame} rider {i} rail");
                assert_eq!(r.mem.read_u32(0x31d338 + 4 * i as u32), env.choice[i], "case {case} frame {frame} rider {i} side");
            }
            assert_eq!(read(&mut r.mem), mine(&b, n), "case {case} frame {frame}");
            compared += env.touching.len();
        }
    }
    eprintln!("{compared} rider visits compared");
}

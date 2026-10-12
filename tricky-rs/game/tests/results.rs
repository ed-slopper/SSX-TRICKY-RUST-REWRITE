//! `tricky_game::results` (board row G4j) against `cAI::vf17_111820` in the function runner (TRICKY_ELF; skipped
//! without it), the career and boarder calls logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tricky_game::results::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
}

struct Env {
    log: Rc<RefCell<Vec<String>>>,
    cnt: i32,
    find: [i32; 2],
    est: Vec<i32>,
    rt: Vec<i32>,
}
impl ResultsEnv for Env {
    fn attitude(&mut self, a: usize, b: usize) {
        self.log.borrow_mut().push(format!("att {a} {b}"));
    }
    fn find_rider_of_player(&mut self, p: i32) -> i32 {
        self.log.borrow_mut().push(format!("frp {p}"));
        self.find[p as usize]
    }
    fn count_profiles(&mut self) -> i32 {
        self.log.borrow_mut().push("cnt".into());
        self.cnt
    }
    fn player_profile(&mut self, p: i32) -> u32 {
        self.log.borrow_mut().push(format!("prof {p}"));
        0x5000 + p as u32
    }
    fn update_best_stats(&mut self, profile: u32, block: &[u8; BLOCK], level: i32) {
        self.log.borrow_mut().push(format!("ubs {profile:x} {level} {:x}", fnv(block)));
    }
    fn mode4_update(&mut self) {
        self.log.borrow_mut().push("m4".into());
    }
    fn estimate_finish(&mut self, rider: usize) -> i32 {
        self.log.borrow_mut().push(format!("est {rider}"));
        self.est[rider]
    }
    fn race_time(&mut self, rider: usize) -> i32 {
        self.log.borrow_mut().push(format!("rt {rider}"));
        self.rt[rider]
    }
}

const TIMES: u32 = 0x32e60c;
const BLOCKS: u32 = 0x32e624;
const ORDER: u32 = 0x32e5f4;

#[test]
fn results_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let rg = r.mem.alloc(0x400);
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x6000)).collect();
    let stats: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x100)).collect();
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let cnt = Rc::new(Cell::new(0i32));
    let find = Rc::new(Cell::new([0i32; 2]));
    let est = Rc::new(RefCell::new(vec![0i32; 6]));
    let rt = Rc::new(RefCell::new(vec![0i32; 6]));
    let idx = {
        let rs = riders.clone();
        move |p: u32| rs.iter().position(|&x| x == p).unwrap()
    };
    let (l, i2) = (log.clone(), idx.clone());
    r.stub(0x11f050, move |c, _| l.borrow_mut().push(format!("att {} {}", i2(c.gpr(4) as u32), i2(c.gpr(5) as u32))));
    let (l, f) = (log.clone(), find.clone());
    r.stub(0x166598, move |c, _| {
        let p = c.gpr(5) as i32;
        l.borrow_mut().push(format!("frp {p}"));
        c.set_gpr(2, f.get()[p as usize] as i64 as u64);
    });
    let (l, n) = (log.clone(), cnt.clone());
    r.stub(0x164288, move |c, _| {
        l.borrow_mut().push("cnt".into());
        c.set_gpr(2, n.get() as i64 as u64);
    });
    r.stub(0x166580, move |c, _| c.set_gpr(2, 100 + c.gpr(5)));
    let l = log.clone();
    r.stub(0x164330, move |c, _| {
        let id = c.gpr(5) as u32 - 100;
        l.borrow_mut().push(format!("prof {id}"));
        c.set_gpr(2, 0x5000 + id as u64);
    });
    let l = log.clone();
    r.stub(0x166cb0, move |c, m| {
        let mut b = [0u8; BLOCK];
        m.read(c.gpr(6) as u32, &mut b).unwrap();
        l.borrow_mut().push(format!("ubs {:x} {} {:x}", c.gpr(5), c.gpr(7) as i32, fnv(&b)));
    });
    let l = log.clone();
    r.stub(0x114f70, move |_, _| l.borrow_mut().push("m4".into()));
    let (l, e, i2) = (log.clone(), est.clone(), idx.clone());
    r.stub(0x11f2a0, move |c, _| {
        let i = i2(c.gpr(4) as u32);
        l.borrow_mut().push(format!("est {i}"));
        c.set_gpr(2, e.borrow()[i] as i64 as u64);
    });
    let (l, e, i2) = (log.clone(), rt.clone(), idx.clone());
    r.stub(0x11ea28, move |c, _| {
        let i = i2(c.gpr(4) as u32);
        l.borrow_mut().push(format!("rt {i}"));
        c.set_gpr(2, e.borrow()[i] as i64 as u64);
    });
    r.allow(0x2cfd90);
    let mut table = [0i32; 6];
    for (k, t) in table.iter_mut().enumerate() {
        *t = r.mem.read_u32(0x3644e0 + 4 * k as u32) as i32;
    }
    let mut g = Rng(0x1618_0339_8874_9894);
    for case in 0..30_000 {
        let n = 1 + g.next() as usize % 6;
        let mode = (g.next() % 8) as i32;
        let level = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12][g.next() as usize % 13];
        let place = [-1, 0, 1, 2, 3, 5][g.next() as usize % 6];
        r.mem.write_u32(0x32de8c, mode as u32);
        r.mem.write_u32(0x32de88, level as u32);
        r.mem.write_u32(rg + 0x88, n as u32);
        cnt.set((g.next() % 3) as i32);
        find.set([(g.next() % 8) as i32 - 1, (g.next() % 8) as i32 - 1]);
        *est.borrow_mut() = (0..6).map(|_| (g.next() % 100000) as i32).collect();
        *rt.borrow_mut() = (0..6).map(|_| (g.next() % 100000) as i32).collect();
        // the tables start random
        let mut junk = vec![0u8; 0x5e0 + 0x30];
        for b in junk.iter_mut() {
            *b = g.next() as u8;
        }
        r.mem.write(ORDER, &junk).unwrap();
        let humans = g.next() % 4;
        let mut rs = vec![];
        for i in 0..n {
            let b = riders[i];
            r.mem.write_u32(rg + 0xc4 + 4 * i as u32, b);
            let mut data = vec![0u8; 0x150];
            for x in data.iter_mut() {
                *x = g.next() as u8;
            }
            r.mem.write(b + 0x30, &data).unwrap();
            let finish = [-1, 0, 500, (g.next() % 90000) as i32, -7][g.next() as usize % 5];
            let human = if (i as u32) < humans && g.next() % 5 != 0 { 1 } else { (g.next() % 3) as i32 };
            let id = (g.next() % n as u32) as i32;
            let mut kind = [-1i8, 0, 3, -5, 90][g.next() as usize % 5];
            if i >= 3 && kind < 0 {
                kind = 4;
            }
            let points = (g.next() % 900000) as i32;
            r.mem.write_u32(b + 0x418, finish as u32);
            r.mem.write_u32(b + 0x41c, human as u32);
            r.mem.write_u32(b + 0x464, stats[i]);
            r.mem.write_u32(stats[i], id as u32);
            r.mem.write_u8(stats[i] + 0x65, kind as u8);
            r.mem.write_u32(b + 0x5910, points as u32);
            let mut block = [0u8; BLOCK];
            for x in block.iter_mut() {
                *x = g.next() as u8;
            }
            block[0x7c..0x80].copy_from_slice(&points.to_le_bytes());
            r.mem.write(b + 0x5894, &block).unwrap();
            // pair records j = -1 ..= n-1 (read before the call)
            let mut pairs = vec![];
            for j in -1..8 {
                let rec = b.wrapping_add(0x10).wrapping_add((j * 0x20) as u32);
                let mut one = [0u8; 1];
                r.mem.read(rec + 0x40, &mut one).unwrap();
                let b40 = one[0] as i8;
                r.mem.read(rec + 0x44, &mut one).unwrap();
                let b44 = one[0] as i8;
                r.mem.read(rec + 0x45, &mut one).unwrap();
                let b45 = one[0];
                let w5c = r.mem.read_u32(b.wrapping_add(0x5c).wrapping_add((j * 0x20) as u32));
                pairs.push(PairRec { b40, b44, b45, w5c });
            }
            let rank0 = r.mem.read_u32(b + 0x110) as i32;
            rs.push(ResRider { finish, human, id, kind, points, rank: rank0, block, pairs });
        }
        // the tables as they start
        let mut t = Tables { times: [0; 8], blocks: [[0; BLOCK]; 8], order: [0; 8] };
        for k in 0..6 {
            t.times[k] = r.mem.read_u32(TIMES + 4 * k as u32) as i32;
            t.order[k] = r.mem.read_u32(ORDER + 4 * k as u32) as i32;
            r.mem.read(BLOCKS + (k * BLOCK) as u32, &mut t.blocks[k]).unwrap();
        }
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rg as u64);
        r.cpu.set_gpr(5, place as i64 as u64);
        r.call(0x111820).unwrap_or_else(|e| panic!("call {case}: {e}"));
        let want_log = log.borrow().clone();
        log.borrow_mut().clear();
        let mut env = Env { log: log.clone(), cnt: cnt.get(), find: find.get(), est: est.borrow().clone(), rt: rt.borrow().clone() };
        results(mode, level, place, table, &mut rs, &mut t, &mut env);
        let got_log = log.borrow().clone();
        let ctx = format!("case {case}: mode {mode} level {level} place {place} n {n} humans {:?} ids {:?}", rs.iter().map(|x| x.human).collect::<Vec<_>>(), rs.iter().map(|x| x.id).collect::<Vec<_>>());
        assert_eq!(got_log, want_log, "calls {ctx}");
        for k in 0..6 {
            assert_eq!(r.mem.read_u32(TIMES + 4 * k as u32) as i32, t.times[k], "times {k} {ctx}");
            assert_eq!(r.mem.read_u32(ORDER + 4 * k as u32) as i32, t.order[k], "order {k} {ctx}");
            let mut b = [0u8; BLOCK];
            r.mem.read(BLOCKS + (k * BLOCK) as u32, &mut b).unwrap();
            assert_eq!(b[..], t.blocks[k][..], "block {k} {ctx}");
        }
        for (i, rd) in rs.iter().enumerate() {
            assert_eq!(r.mem.read_u32(riders[i] + 0x418) as i32, rd.finish, "finish {i} {ctx}");
            assert_eq!(r.mem.read_u32(riders[i] + 0x5910) as i32, rd.points, "points {i} {ctx}");
            if place >= 0 {
                assert_eq!(r.mem.read_u32(riders[i] + 0x110) as i32, rd.rank, "rank {i} {ctx}");
            }
        }
    }
}

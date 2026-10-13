//! `tricky_game::sndqueue` (board row G10a2) against the original in the function runner (TRICKY_ELF; skipped
//! without it).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::sndqueue::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

const N: usize = 8;

#[derive(Default)]
struct Or {
    vals: Vec<i64>,
    log: Vec<(&'static str, u64, u64)>,
}
impl Or {
    fn take(&mut self, name: &'static str, a: u64, b: u64) -> i64 {
        self.log.push((name, a, b));
        self.vals.remove(0)
    }
}

struct TEnv(Rc<RefCell<Or>>);
impl QueueEnv for TEnv {
    fn get_slot(&mut self, which: u32) -> u32 {
        self.0.borrow_mut().take("slot", which as u64, 0) as u32
    }
    fn rider_listener(&mut self) -> u32 {
        self.0.borrow_mut().take("rl", 0, 0) as u32
    }
    fn play_pending(&mut self) {
        self.0.borrow_mut().log.push(("pp", 0, 0));
    }
}
impl AirEnv for TEnv {
    fn replay_is_playing(&mut self) -> bool {
        self.0.borrow_mut().take("replay", 0, 0) != 0
    }
    fn listener_rider(&mut self, who: u32) -> i64 {
        self.0.borrow_mut().take("lr", who as u64, 0)
    }
    fn crowd_off(&mut self, who: u32) {
        self.0.borrow_mut().log.push(("off", who as u64, 0));
    }
    fn crowd_level(&mut self, level: u32, who: u32) {
        self.0.borrow_mut().log.push(("lvl", level as u64, who as u64));
    }
    fn find_rider_near(&mut self, radius: f32, emitter: u32) -> i64 {
        self.0.borrow_mut().take("near", radius.to_bits() as u64, emitter as u64)
    }
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64 {
        self.0.borrow_mut().take("curve", slot as u64, x as u32 as u64)
    }
    fn surface_sound_type(&mut self) -> u32 {
        self.0.borrow_mut().take("surf", 0, 0) as u32
    }
}

struct Layout {
    sys: u32,
    arr: [u32; 16],
}
const PTRS: [u32; 15] = [0x24, 0x38, 0x3c, 0x40, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x7c, 0xa40, 0xa44, 0, 0];

fn read(r: &mut Runner, l: &Layout) -> Queue {
    let mut q = Queue::with_capacity(N);
    let s = l.sys;
    r.mem.read(s + 8, &mut q.header).unwrap();
    q.count = r.mem.read_u32(s + 0x20) as i32;
    let w = |r: &mut Runner, p: u32, n: usize| -> Vec<u32> { (0..n).map(|k| r.mem.read_u32(p + 4 * k as u32)).collect() };
    for i in 0..N {
        r.mem.read(l.arr[0] + 24 * i as u32, &mut q.record[i]).unwrap();
    }
    for i in 0..7 {
        q.emitter[i] = r.mem.read_u32(s + 0x28 + 4 * i as u32);
    }
    q.a38 = w(r, l.arr[1], N);
    q.a3c = w(r, l.arr[2], N);
    q.a40 = w(r, l.arr[3], N);
    for i in 0..4 {
        q.a44[i] = [r.mem.read_u32(s + 0x44 + 8 * i as u32), r.mem.read_u32(s + 0x48 + 8 * i as u32)];
    }
    q.a64 = w(r, l.arr[4], N);
    q.a68 = w(r, l.arr[5], N);
    q.a6c = w(r, l.arr[6], N);
    q.a70 = w(r, l.arr[7], N);
    for i in 0..N {
        let v = w(r, l.arr[8] + 16 * i as u32, 4);
        q.a74[i] = [v[0], v[1], v[2], v[3]];
    }
    q.a78 = w(r, l.arr[9], N);
    q.a7c = w(r, l.arr[10], N);
    q.kind = w(r, l.arr[11], N);
    q.id = w(r, l.arr[12], N);
    q
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn update_air_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let or: Rc<RefCell<Or>> = Rc::new(RefCell::new(Or::default()));
    let rider = r.mem.alloc(0x900);
    let sys = r.mem.alloc(0xb00);
    let mut l = Layout { sys, arr: [0; 16] };
    for k in 0..13 {
        l.arr[k] = r.mem.alloc(24 * N as u32);
    }
    let stubs: [(u32, &'static str); 8] = [(0x21bfd0, "replay"), (0x20ed20, "lr"), (0x20f9b0, "near"), (0x20d928, "curve"), (0x20f908, "surf"), (0x211f38, "slot"), (0x20fc60, "rl"), (0, "")];
    for (a, name) in stubs {
        if a == 0 {
            continue;
        }
        let o = or.clone();
        r.stub(a, move |c, _| {
            let (x, y) = match name {
                "near" => (c.f(12).to_bits() as u64, c.gpr(5) as u32 as u64),
                "curve" => (c.gpr(4) as u32 as u64, c.gpr(5) as u32 as u64),
                "lr" => (c.gpr(5) as u32 as u64, 0),
                "slot" => (c.gpr(5) as u32 as u64, 0),
                _ => (0, 0),
            };
            let v = o.borrow_mut().take(name, x, y);
            c.set_gpr(2, v as u64);
        });
    }
    let o = or.clone();
    r.stub(0x2210c8, move |c, _| o.borrow_mut().log.push(("off", c.gpr(5) as u32 as u64, 0)));
    let o = or.clone();
    r.stub(0x221068, move |c, _| o.borrow_mut().log.push(("lvl", c.gpr(5) as u32 as u64, c.gpr(6) as u32 as u64)));
    let o = or.clone();
    r.stub(0x20c948, move |_, _| o.borrow_mut().log.push(("pp", 0, 0)));
    r.allow(0x2fb468);
    let mut g = Rng(0x600d_5eed_1234_5678);
    for case in 0..30_000 {
        // random memory
        for k in 0..0xb00 / 4 {
            r.mem.write_u32(sys + 4 * k as u32, g.next());
        }
        for (k, p) in PTRS.iter().enumerate().take(13) {
            r.mem.write_u32(sys + p, l.arr[k]);
        }
        for k in 0..13 {
            for j in 0..(24 * N as u32 / 4) {
                r.mem.write_u32(l.arr[k] + 4 * j, g.next());
            }
        }
        r.mem.write_u32(sys + 0x20, g.next() % 3);
        let flag = [0u32, 1, 1, 2, 5][g.next() as usize % 5];
        let player = g.next() % 4;
        let air = [0.0f32, 12.5, -3.0, 1e9, 400.0, 1e-3][g.next() as usize % 6];
        r.mem.write_u32(rider + 0x41c, flag);
        r.mem.write_u32(rider + 0x38, player);
        r.mem.write_f32(rider + 0x798, air);
        let level = [0.0f32, -1.0, 0.1, 0.25, 0.3, 0.65, 0.649, 0.9, 5.0, f32::from_bits(0x3f26_6666), f32::from_bits(0x3f26_6665)][g.next() as usize % 11];
        let rid = rider as i64;
        let near_ok = (g.next() % 2) as i64;
        let vals: Vec<i64> = vec![
            (g.next() % 2) as i64,
            if g.next() % 2 == 0 { rid } else { rid + 4 },
            [-1i64, 0, 3][(near_ok as usize + g.next() as usize) % 3],
            [-5i64, 0, 50, 126, 127, 128, 500, -1 << 40, 1 << 40, -1][g.next() as usize % 10],
            (g.next() % 10) as i64,
            (g.next() % 100) as i64,
            (g.next() % 100) as i64,
        ];
        let before = read(&mut r, &l);
        or.borrow_mut().vals = vals.clone();
        or.borrow_mut().log.clear();
        r.cpu.set_f(12, level);
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x217d50).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let orig_log = std::mem::take(&mut or.borrow_mut().log);
        let after = read(&mut r, &l);

        let mut q = before.clone();
        or.borrow_mut().vals = vals;
        or.borrow_mut().log.clear();
        let ar = AirRider { flag_41c: flag, player, air, emitter: rider + 0x140 };
        update_air(&mut q, level, rid, &ar, &mut TEnv(or.clone()));
        let my_log = std::mem::take(&mut or.borrow_mut().log);
        assert_eq!(orig_log, my_log, "case {case} calls flag {flag} level {level}");
        assert_eq!(after, q, "case {case} state flag {flag}");
    }
}

struct EnterEnv {
    o: Rc<RefCell<Or>>,
    impact: i32,
}
impl QueueEnv for EnterEnv {
    fn get_slot(&mut self, which: u32) -> u32 {
        self.o.borrow_mut().take("slot", which as u64, 0) as u32
    }
    fn rider_listener(&mut self) -> u32 {
        self.o.borrow_mut().take("rl", 0, 0) as u32
    }
    fn play_pending(&mut self) {
        self.o.borrow_mut().log.push(("pp", 0, 0));
    }
}
impl AirEnterEnv for EnterEnv {
    fn big_landing(&mut self) {
        self.o.borrow_mut().log.push(("big", 0, 0));
    }
    fn on_air_enter_prelude(&mut self) {
        self.o.borrow_mut().log.push(("pre", 0, 0));
    }
    fn replay_is_playing(&mut self) -> bool {
        self.o.borrow_mut().take("replay", 0, 0) != 0
    }
    fn listener_rider(&mut self, who: u32) -> i64 {
        self.o.borrow_mut().take("lr", who as u64, 0)
    }
    fn crowd_stop(&mut self, who: u32) {
        self.o.borrow_mut().log.push(("stop", who as u64, 0));
    }
    fn crowd_clear(&mut self) {
        self.o.borrow_mut().log.push(("clear", 0, 0));
    }
    fn impact_speed(&mut self) -> i32 {
        self.impact
    }
    fn surface_sound_type(&mut self) -> u32 {
        self.o.borrow_mut().take("surf", 0, 0) as u32
    }
}

#[test]
fn on_air_enter_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let or: Rc<RefCell<Or>> = Rc::new(RefCell::new(Or::default()));
    let rider = r.mem.alloc(0x5900);
    let sys = r.mem.alloc(0xb00);
    let mut l = Layout { sys, arr: [0; 16] };
    for k in 0..13 {
        l.arr[k] = r.mem.alloc(24 * N as u32);
    }
    let impact = Rc::new(RefCell::new(0i32));
    for (a, name) in [(0x21bfd0u32, "replay"), (0x20ed20, "lr"), (0x20f908, "surf"), (0x211f38, "slot"), (0x20fc60, "rl")] {
        let o = or.clone();
        r.stub(a, move |c, _| {
            let (x, y) = match name {
                "lr" | "slot" => (c.gpr(5) as u32 as u64, 0),
                _ => (0, 0),
            };
            let v = o.borrow_mut().take(name, x, y);
            c.set_gpr(2, v as u64);
        });
    }
    let i2 = impact.clone();
    r.stub(0x21a6a8, move |c, _| c.set_gpr(2, *i2.borrow() as i64 as u64));
    for (a, name) in [(0x233950u32, "big"), (0x235170, "pre"), (0x2210a8, "stop"), (0x225768, "clear"), (0x20c948, "pp")] {
        let o = or.clone();
        r.stub(a, move |c, _| {
            let x = if name == "stop" { c.gpr(5) as u32 as u64 } else { 0 };
            o.borrow_mut().log.push((name, x, 0));
        });
    }
    r.allow(0x2fb468);
    let mut g = Rng(0x3a5c_0de1_77aa_90b3);
    for case in 0..30_000 {
        for k in 0..0xb00 / 4 {
            r.mem.write_u32(sys + 4 * k as u32, g.next());
        }
        for (k, p) in PTRS.iter().enumerate().take(13) {
            r.mem.write_u32(sys + p, l.arr[k]);
        }
        for k in 0..13 {
            for j in 0..(24 * N as u32 / 4) {
                r.mem.write_u32(l.arr[k] + 4 * j, g.next());
            }
        }
        r.mem.write_u32(sys + 0x20, g.next() % 3);
        let flag = [0u32, 1, 1, 2][g.next() as usize % 4];
        let player = g.next() % 4;
        let height = [0.0f32, 1.5, 1.6, 2.0, 2.1, 5.0, 5.1, 1.4, 9.0, -1.0][g.next() as usize % 10];
        r.mem.write_u32(rider + 0x41c, flag);
        r.mem.write_u32(rider + 0x38, player);
        r.mem.write_f32(rider + 0x57f4, height);
        let imp: i32 = match g.next() % 7 {
            0 => 0,
            1 => (g.next() % 20_000) as i32 - 5_000,
            2 => (g.next() % 1000) as i32,
            3 => 504 + (g.next() % 12) as i32,
            4 => (g.next() as i32) / 3,
            5 => 1000 * 64 / 127 + (g.next() % 20) as i32 - 10,
            _ => 1000 + (g.next() % 40) as i32 - 20,
        };
        *impact.borrow_mut() = imp;
        let rid = rider as i64;
        let vals: Vec<i64> = vec![
            (g.next() % 2) as i64,
            if g.next() % 4 != 0 { rid } else { rid + 4 },
            (g.next() % 10) as i64,
            (g.next() % 100) as i64,
            (g.next() % 100) as i64,
        ];
        let before = read(&mut r, &l);
        or.borrow_mut().vals = vals.clone();
        or.borrow_mut().log.clear();
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, rider as u64);
        r.call(0x2174f8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let orig_log = std::mem::take(&mut or.borrow_mut().log);
        let after = read(&mut r, &l);

        let mut q = before.clone();
        or.borrow_mut().vals = vals;
        or.borrow_mut().log.clear();
        let ar = AirRider { flag_41c: flag, player, air: 0.0, emitter: rider + 0x140 };
        on_air_enter(&mut q, height, rid, &ar, &mut EnterEnv { o: or.clone(), impact: imp });
        let my_log = std::mem::take(&mut or.borrow_mut().log);
        assert_eq!(orig_log, my_log, "case {case} calls flag {flag} height {height} impact {imp}");
        assert_eq!(after, q, "case {case} state impact {imp}");
    }
}

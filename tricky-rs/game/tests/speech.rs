//! The speech triggers (board row G10c1) against the game in the function runner (TRICKY_ELF; skipped without it):
//! every `SpeechEvt_Post*` against `speech::Event::new`, and the race handlers (`Speech_On*` with the game's own
//! `Speech_RollChance`) on random riders and speech states, with the speech library, the replay, the listener, the
//! near test, the relation table and `Rand_Fx` scripted the same on both sides: every call in order compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE};
use tricky_game::speech::{self, voices, Announcer, Event, SpeechEnv, SpeechRider, Trigger};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// Every post (address, event id, its arguments: p through a pointer, v by value)
#[rustfmt::skip]
const POSTS: &[(u32, u32, &str)] = &[
    (0x22e4e0, 0x0, "ppp"), (0x22e6a8, 0x100200f, "pppppvvp"), (0x22e778, 0x1002010, "v"), (0x22e7c0, 0x1002012, "ppp"),
    (0x22e838, 0x1002013, "pppp"), (0x22e8c0, 0x100201a, ""), (0x22e8f8, 0x100201c, "pppp"), (0x22e980, 0x100201e, ""),
    (0x22e9b8, 0x100201f, "ppp"), (0x22ea30, 0x1002020, "ppp"), (0x22eaa8, 0x100202a, ""), (0x22eae0, 0x100202b, "p"),
    (0x22eb30, 0x100202c, ""), (0x22eb68, 0x100202d, "p"), (0x22ebb8, 0x100202e, "p"), (0x22ec08, 0x100202f, ""),
    (0x22ec40, 0x1002030, "p"), (0x22ec90, 0x1002031, "p"), (0x22ece0, 0x1002033, "p"), (0x22ed30, 0x1002034, "pp"),
    (0x22ed90, 0x1002035, ""), (0x22edc8, 0x1002036, ""), (0x22ee00, 0x1002037, ""), (0x22ee38, 0x1002046, "p"),
    (0x22ee88, 0x1002047, "p"), (0x22eed8, 0x1002048, "p"), (0x22ef28, 0x1002049, "ppp"), (0x22efa0, 0x100204a, "ppp"),
    (0x22f018, 0x100204b, "p"), (0x22f068, 0x100204c, ""), (0x22f0a0, 0x100204d, ""), (0x22f0d8, 0x100204e, "p"),
    (0x22f128, 0x100204f, ""), (0x22f160, 0x1002050, "p"), (0x22f1b0, 0x1002051, "p"), (0x22f200, 0x1002052, ""),
    (0x22f238, 0x1002053, ""), (0x22f270, 0x1002054, "p"), (0x22f2c0, 0x1002055, ""), (0x22f2f8, 0x1002056, ""),
    (0x22f330, 0x300600f, "pp"), (0x22f390, 0x3006019, "pp"), (0x22f3f0, 0x300601a, "pp"), (0x22f450, 0x300601b, "pp"),
    (0x22f4b0, 0x300601c, "pp"), (0x22f510, 0x3006020, "pp"), (0x22f570, 0x3006021, "pp"), (0x22f5d0, 0x3006022, "p"),
    (0x22f620, 0x3006023, "pp"), (0x22f680, 0x3006024, "pp"), (0x22f6e0, 0x3006025, "p"), (0x22f730, 0x3006026, "p"),
    (0x22f780, 0x3006027, "p"), (0x22f7d0, 0x3006028, "p"), (0x22f820, 0x300602b, "p"), (0x22f870, 0x3006030, "pp"),
    (0x22f8d0, 0x3006031, "pp"), (0x22f930, 0x3006032, "pp"), (0x22f990, 0x3006033, "p"), (0x22f9e0, 0x3006034, "p"),
    (0x22fa30, 0x3006036, "pp"), (0x22fa90, 0x3006038, "p"), (0x22fae0, 0x8000, "p"), (0x22fb38, 0x8001, "pp"),
    (0x22fb90, 0x8002, "pp"), (0x22fbe8, 0x8003, "ppp"), (0x22fc50, 0x8004, "p"), (0x22fca8, 0x8005, "pp"),
    (0x22fd00, 0x500a000, "v"), (0x22fd48, 0x4008000, "pp"), (0x22fda8, 0x4008002, "vp"), (0x22fe08, 0x4008003, "vp"),
    (0x22fe68, 0x4008004, "vp"), (0x22fec8, 0x4008005, "vp"), (0x22ff28, 0x4008006, "pppp"), (0x22ffb0, 0x4008007, "ppppp"),
    (0x230050, 0x4008008, "ppp"), (0x2300c8, 0x4008009, "ppppp"), (0x230168, 0x400800a, "pp"), (0x2301c8, 0x400800b, "ppp"),
    (0x230240, 0x400800c, "pppp"), (0x2302c8, 0x400800d, "pp"), (0x230328, 0x400800e, "pp"), (0x230388, 0x400800f, "ppp"),
    (0x230400, 0x4008010, "pp"), (0x230460, 0x4008011, "p"), (0x2304b0, 0x4008012, "p"), (0x230500, 0x4008013, "pppppp"),
    (0x2305b0, 0x4008014, "pp"), (0x230610, 0x4008015, "p"), (0x230660, 0x4008016, "vp"), (0x2306c0, 0x4008017, "pp"),
];

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Post(Vec<u32>),
    /// gain bits, slot, at (0 none), callback, voice
    Trigger(u32, u32, u32, bool, u32),
    Interrupt(u32),
    Rand(u32),
    Hold(u32, u32),
}

/// The scripted game both sides ask
#[derive(Clone)]
struct World {
    rand: u64,
    replay: bool,
    listener: u32,
    mode: u32,
    can_play: bool,
    near: Vec<(u32, i32)>,
    rel: [[i32; 12]; 12],
    course: u32,
    app_mode: u32,
    rider0: u32,
    /// what the library hands `Speech_QueueBigScore` on a trigger with it
    callbacks: Vec<u32>,
    log: Vec<Ev>,
}

impl World {
    fn new(seed: u64) -> World {
        World { rand: seed, replay: false, listener: 0, mode: 0, can_play: true, near: vec![], rel: [[0; 12]; 12], course: 0, app_mode: 0, rider0: 0, callbacks: vec![], log: vec![] }
    }
    fn draw(&mut self) -> u32 {
        self.rand ^= self.rand << 13;
        self.rand ^= self.rand >> 7;
        self.rand ^= self.rand << 17;
        let r = (self.rand >> 20) as u32;
        self.log.push(Ev::Rand(r));
        r
    }
    fn near_of(&self, id: u32) -> i32 {
        self.near.iter().find(|(i, _)| *i == id).map_or(-1, |(_, n)| *n)
    }
}

impl SpeechEnv for World {
    fn rand_fx(&mut self) -> u32 {
        self.draw()
    }
    fn replay_playing(&mut self) -> bool {
        self.replay
    }
    fn listener(&mut self) -> u32 {
        self.listener
    }
    fn mode(&mut self) -> u32 {
        self.mode
    }
    fn can_play(&mut self, _slot: u32) -> bool {
        self.can_play
    }
    fn find_near(&mut self, rider: u32) -> i32 {
        self.near_of(rider)
    }
    fn interrupt_if_playing(&mut self, voice: u32) {
        self.log.push(Ev::Interrupt(voice));
    }
    fn relation(&mut self, a: u32, b: u32) -> i32 {
        self.rel[a as usize][b as usize]
    }
    fn post(&mut self, e: Event) {
        self.log.push(Ev::Post(e.words.to_vec()));
    }
    fn trigger(&mut self, t: Trigger) -> Vec<u32> {
        self.log.push(Ev::Trigger(t.gain.to_bits(), t.slot, t.at.map_or(0, |r| r + 0x140), t.callback, t.voice));
        if t.callback {
            self.callbacks.clone()
        } else {
            vec![]
        }
    }
    fn course(&mut self) -> u32 {
        self.course
    }
    fn app_mode(&mut self) -> u32 {
        self.app_mode
    }
    fn slot_hold(&mut self, slot: u32, v: u32) {
        self.log.push(Ev::Hold(slot, v));
    }
    fn race_rider(&mut self, _i: u32) -> u32 {
        self.rider0
    }
}

/// The big-score ring in the game's memory (sys+0x7dd4 → size, buffer, write, read)
const RING: u32 = 0x7dd4;

/// `Speech_QueueBigScore` as the library's callback does it in the game (the test's stand-in for the library;
/// the function itself is checked on its own)
fn game_queue(m: &mut r5900::Mem, sys: u32, v: u32) {
    let ring = m.read_u32(sys + RING);
    let (n, buf, w, rd) = (m.read_u32(ring) as i32, m.read_u32(ring + 4), m.read_u32(ring + 8) as i32, m.read_u32(ring + 12) as i32);
    if (w + 1) % n != rd {
        m.write_u32(buf + 4 * w as u32, v);
        m.write_u32(ring + 8, if n <= w + 1 { 0 } else { (w + 1) as u32 });
    }
    m.write_u32(sys + 0x7dd8, 1);
}

fn setup(r: &mut Runner, w: &Rc<RefCell<World>>) {
    let x = w.clone();
    r.stub(0x2bba80, move |c, m| {
        let at = c.gpr(4) as u32;
        let words = (0..20).map(|k| m.read_u32(at + 4 * k)).collect();
        x.borrow_mut().log.push(Ev::Post(words));
    });
    let x = w.clone();
    r.stub(0x23d4a8, move |c, _| {
        let v = x.borrow_mut().draw();
        c.set_gpr(2, v as u64);
    });
    let x = w.clone();
    r.stub(0x21bfd0, move |c, _| c.set_gpr(2, x.borrow().replay as u64));
    let x = w.clone();
    r.stub(0x20ed20, move |c, _| c.set_gpr(2, x.borrow().listener as u64));
    let x = w.clone();
    r.stub(0x228958, move |c, _| c.set_gpr(2, x.borrow().can_play as u64));
    let x = w.clone();
    r.stub(0x20f9b0, move |c, _| {
        let n = x.borrow().near_of((c.gpr(5) as u32).wrapping_sub(0x140));
        c.set_gpr(2, n as i64 as u64);
    });
    let x = w.clone();
    r.stub(0x234bb8, move |c, _| x.borrow_mut().log.push(Ev::Interrupt(c.gpr(5) as u32 & 0xff)));
    let x = w.clone();
    r.stub(0x167e00, move |c, _| {
        let v = x.borrow().rel[c.gpr(5) as usize][c.gpr(6) as usize];
        c.set_gpr(2, v as i64 as u64);
    });
    let x = w.clone();
    r.stub(0x228bd8, move |c, m| {
        let callback = c.gpr(7) as u32 == 0x235530;
        let ev = Ev::Trigger(c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as u32, callback, c.gpr(8) as u32 & 0xff);
        x.borrow_mut().log.push(ev);
        if callback {
            let sys = c.gpr(4) as u32;
            let vals = x.borrow().callbacks.clone();
            for v in vals {
                game_queue(m, sys, v);
                x.borrow_mut().log.push(Ev::Hold(1, 0));
            }
        }
    });
    let x = w.clone();
    r.stub(0x228bb0, move |c, _| x.borrow_mut().log.push(Ev::Hold(c.gpr(5) as u32, c.gpr(6) as u32)));
    let x = w.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, x.borrow().rider0 as u64));
    // memset
    r.stub(0x2fb468, |c, m| {
        let (d, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
        for k in 0..n {
            m.write_u8(d + k, v);
        }
        c.set_gpr(2, d as u64);
    });
}

fn call(r: &mut Runner, f: u32, args: &[u64]) -> u64 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *a);
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e}"));
    r.cpu.gpr(2)
}

#[test]
fn posts_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let w = Rc::new(RefCell::new(World::new(1)));
    setup(&mut r, &w);
    let cells = r.mem.alloc(0x40);
    let mut g = Rng(0x5bee_c400_0000_0001);
    for &(f, id, kinds) in POSTS {
        for _ in 0..4 {
            let vals: Vec<u32> = kinds.chars().map(|_| g.next()).collect();
            let mut regs = Vec::new();
            for (k, (kind, v)) in kinds.chars().zip(&vals).enumerate() {
                if kind == 'p' {
                    r.mem.write_u32(cells + 4 * k as u32, *v);
                    regs.push((cells + 4 * k as u32) as u64);
                } else {
                    regs.push(*v as u64);
                }
            }
            w.borrow_mut().log.clear();
            call(&mut r, f, &regs);
            let log = w.borrow().log.clone();
            assert_eq!(log, vec![Ev::Post(Event::new(id, &vals).words.to_vec())], "{f:#x}");
        }
    }
}

fn random_rider(g: &mut Rng) -> SpeechRider {
    let mut att = [0i8; 8];
    for a in att.iter_mut() {
        *a = g.next() as i8;
    }
    SpeechRider {
        id: 0,
        controller: if g.below(4) == 0 { g.below(3) as i32 } else { -1 },
        kind: g.below(3) as i32,
        index: g.below(6) as i32,
        rival_on: g.below(2) as i32,
        rival: g.below(4) as i32,
        slot: g.below(6) as i32,
        attitudes: att,
        speed: [0.0f32, 1341.0, 1341.12, 1341.13, 2000.0][g.below(5) as usize],
        air: [0.0f32, 2.9, 3.0, 5.0][g.below(4) as usize],
        tricky: g.below(9) as i32,
        character: g.below(12),
        voice: g.below(14) as u8,
        level: g.below(8) as i32,
    }
}

fn write_rider(r: &mut Runner, at: u32, entry: u32, s: &SpeechRider) {
    r.mem.write_u32(at + 0x418, s.controller as u32);
    r.mem.write_u32(at + 0x41c, s.kind as u32);
    r.mem.write_u32(at + 0x38, s.index as u32);
    r.mem.write_u32(at + 0x118, s.rival_on as u32);
    r.mem.write_u32(at + 0x120, s.rival as u32);
    r.mem.write_u32(at + 0x460, s.slot as u32);
    for (k, a) in s.attitudes.iter().enumerate() {
        r.mem.write_u8(at + 0x50 + 0x20 * k as u32, *a as u8);
    }
    r.mem.write_f32(at + 0x158, s.speed);
    r.mem.write_f32(at + 0x57f4, s.air);
    r.mem.write_u32(at + 0x58a8, s.tricky as u32);
    r.mem.write_u32(at + 0x464, entry);
    r.mem.write_u32(entry + 0x38, s.character);
    r.mem.write_u8(entry + 100, s.voice);
    r.mem.write_u32(entry + 0x74, s.level as u32);
}

#[test]
fn handlers_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let w = Rc::new(RefCell::new(World::new(7)));
    setup(&mut r, &w);
    // the handlers' own callees run as the game's
    for a in [0x235d58u32, 0x233b70, 0x234698, 0x234730, 0x234c38, 0x235090, 0x233138, 0x1635b0, 0x2309a8, 0x235ba8, 0x235530, 0x164288, 0x164318, 0x166550] {
        r.allow(a);
    }
    for &(f, _, _) in POSTS {
        r.allow(f);
    }
    let sys = r.mem.alloc(0x8000);
    r.mem.write_u32(0x343050, sys);
    let ring = r.mem.alloc(16);
    let ring_buf = r.mem.alloc(4 * 8);
    r.mem.write_u32(sys + RING, ring);
    r.mem.write_u32(ring + 4, ring_buf);
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let riders: Vec<u32> = (0..2).map(|_| r.mem.alloc(0x5900)).collect();
    let entries: Vec<u32> = (0..2).map(|_| r.mem.alloc(0x80)).collect();
    let mut g = Rng(0x5bee_c400_0000_0002);
    let mut seen = [0u32; 37];
    let mut posted = [0u32; 37];
    for case in 0..60_000 {
        // the speech system
        let n = 1 + g.below(8) as usize;
        let mut ann = Announcer {
            level: g.below(120) as f32 / 10.0,
            cap: g.below(10) as i32,
            resets_on: g.below(2),
            checkpoints: [0; 8],
            ring: (0..n).map(|_| g.next()).collect(),
            ring_write: g.below(n as u32) as i32,
            ring_read: g.below(n as u32) as i32,
            big_queued: g.below(2),
        };
        r.mem.write_u32(ring, n as u32);
        for (k, v) in ann.ring.iter().enumerate() {
            r.mem.write_u32(ring_buf + 4 * k as u32, *v);
        }
        r.mem.write_u32(ring + 8, ann.ring_write as u32);
        r.mem.write_u32(ring + 12, ann.ring_read as u32);
        r.mem.write_u32(sys + 0x7dd8, ann.big_queued);
        for c in ann.checkpoints.iter_mut() {
            *c = g.below(6) as i32;
        }
        r.mem.write_f32(sys + 0x383c, ann.level);
        r.mem.write_u32(sys + 0x6e68, ann.cap as u32);
        r.mem.write_u32(sys + 0x6e50, ann.resets_on);
        for (k, c) in ann.checkpoints.iter().enumerate() {
            r.mem.write_u32(sys + 0x6e6c + 4 * k as u32, *c as u32);
        }
        // the riders
        let mut rs = Vec::new();
        for k in 0..2 {
            let mut s = random_rider(&mut g);
            s.id = riders[k];
            write_rider(&mut r, riders[k], entries[k], &s);
            rs.push(s);
        }
        // the world
        {
            let mut x = w.borrow_mut();
            x.rand = g.next() as u64 | 1;
            x.replay = g.below(6) == 0;
            x.listener = if g.below(5) == 0 { 0x1234 } else { riders[g.below(2) as usize] };
            x.mode = g.below(8);
            x.can_play = g.below(5) != 0;
            x.near = riders.iter().map(|id| (*id, if g.below(3) == 0 { -1 } else { g.below(3) as i32 })).collect();
            for a in 0..12 {
                for b in 0..12 {
                    x.rel[a][b] = g.below(5) as i32 - 1;
                }
            }
            x.course = g.below(13);
            x.app_mode = g.below(6);
            x.rider0 = riders[g.below(2) as usize];
            x.callbacks = (0..[0, 0, 1, 2][g.below(4) as usize]).map(|_| g.next()).collect();
            x.log.clear();
        }
        r.mem.write_u32(0x32de8c, w.borrow().mode);
        r.mem.write_u32(0x32de88, w.borrow().course);
        r.mem.write_u32(app + 0x58, w.borrow().app_mode);
        // the career the start and next-race lines read
        let mut car = Career::default();
        car.bytes[0x2b0c] = g.below(4) as u8;
        car.w32(0x27dc, 1 + g.below(6));
        for i in 0..6 {
            car.bytes[0x2845 + 0x84 * i] = if g.below(3) == 0 { g.below(4) as u8 } else { 0xff };
            for j in 0..8 {
                car.bytes[0x27dc + 0x20 + 0x84 * i + 4 * j] = [0u8, 20, 21, 80, 81, 0x90][g.below(6) as usize];
            }
        }
        for k in 0..4 {
            car.bytes[0x30fc + 0x44 * k] = if g.below(2) == 0 { g.below(4) as u8 } else { 0xff };
            car.w32(0x3110 + 0x44 * k, g.below(2));
        }
        for k in [0x2b0cu32, 0x27dc] {
            for b in 0..4 {
                r.mem.write_u8(CAREER_BASE + k + b, car.bytes[(k + b) as usize]);
            }
        }
        for i in 0..6u32 {
            r.mem.write_u8(CAREER_BASE + 0x2845 + 0x84 * i, car.bytes[(0x2845 + 0x84 * i) as usize]);
            for j in 0..8 {
                let at = 0x27dc + 0x20 + 0x84 * i + 4 * j;
                r.mem.write_u8(CAREER_BASE + at, car.bytes[at as usize]);
            }
        }
        for k in 0..4u32 {
            r.mem.write_u8(CAREER_BASE + 0x30fc + 0x44 * k, car.bytes[(0x30fc + 0x44 * k) as usize]);
            r.mem.write_u32(CAREER_BASE + 0x3110 + 0x44 * k, car.r32((0x3110 + 0x44 * k) as usize));
        }
        let mut ours = w.borrow().clone();
        let op = g.below(37) as usize;
        seen[op] += 1;
        let (a, b) = (rs[0], rs[1]);
        let (pa, pb) = (riders[0] as u64, riders[1] as u64);
        let s = sys as u64;
        let kind = g.below(3) as u64;
        let mut ret = None;
        match op {
            0 => {
                call(&mut r, 0x233d38, &[s, pa]);
                ann.on_wipeout(&mut ours, &a);
            }
            1 => {
                call(&mut r, 0x233e78, &[s, pa]);
                ann.on_boost(&mut ours, &a);
            }
            2 => {
                call(&mut r, 0x233e10, &[s, pa]);
                ann.on_boost_full(&mut ours, &a);
            }
            3 => {
                call(&mut r, 0x2333e0, &[s, pa]);
                ann.on_trick_landed(&mut ours, &a);
            }
            4 => {
                call(&mut r, 0x233448, &[s, pa]);
                ann.on_air_time(&mut ours, &a);
            }
            5 => {
                call(&mut r, 0x233ff0, &[s, pa]);
                ann.on_ten_seconds_left(&mut ours, &a);
            }
            6 => {
                call(&mut r, 0x234058, &[s]);
                ann.on_race_line(&mut ours);
            }
            7 => {
                call(&mut r, 0x233cb8, &[s, pa]);
                ann.on_rider_collision(&mut ours, &a);
            }
            8 => {
                call(&mut r, 0x2336b8, &[s, pa, pb]);
                ann.on_grudge(&mut ours, &a, &b);
            }
            9 => {
                call(&mut r, 0x233740, &[s, pa]);
                ann.on_listener_line(&mut ours, &a);
            }
            10 => {
                call(&mut r, 0x2337c8, &[s, pa]);
                ann.on_tricky(&mut ours, &a);
            }
            11 => {
                call(&mut r, 0x233858, &[s, pa, pb]);
                ann.on_rider_hit(&mut ours, &a, &b);
            }
            12 => {
                call(&mut r, 0x233950, &[s, pa, kind]);
                ann.on_air_enter(&mut ours, &a, kind as i64);
            }
            13 => {
                call(&mut r, 0x233a18, &[s, pa]);
                ann.on_rider_reset(&mut ours, &a);
            }
            14 => {
                call(&mut r, 0x233ee0, &[s, pa]);
                ann.on_checkpoint(&mut ours, &a);
            }
            15 => {
                call(&mut r, 0x234558, &[s, pa, kind]);
                ann.on_local_line(&mut ours, &a, kind as i64);
            }
            16 => {
                call(&mut r, 0x234610, &[s, pa]);
                ann.on_local_line2(&mut ours, &a);
            }
            17 => {
                let theirs = call(&mut r, 0x234c38, &[s, pa, pb]) != 0;
                ret = Some((theirs, speech::is_rival_or_grudge(&mut ours, &a, &b)));
            }
            18 => {
                call(&mut r, 0x235090, &[s, pa]);
                voices::on_crash_nearby(&mut ours, &a);
            }
            19 => {
                call(&mut r, 0x235398, &[s, pa, kind]);
                voices::on_stumble(&mut ours, &a, kind as i64);
            }
            20 => {
                let (which, crash) = (g.below(2) as u64, g.below(2) as u64);
                call(&mut r, 0x234d00, &[s, pa, pb, which, crash]);
                voices::rider_hit_line(&mut ours, &a, &b, which as i64, crash as i64);
            }
            21 => {
                call(&mut r, 0x234f68, &[s, pa, pb]);
                voices::on_ai_taunt(&mut ours, &a, &b);
            }
            22 => {
                call(&mut r, 0x235170, &[s, pa, kind]);
                voices::on_air_nearby(&mut ours, &a, kind as i64);
            }
            23 => {
                let grade = [0.0f32, 0.64, 0.65, 0.9][g.below(4) as usize];
                r.cpu.set_f(12, grade);
                call(&mut r, 0x235298, &[s, pa]);
                voices::on_graded_line(&mut ours, grade, &a);
            }
            24 => {
                call(&mut r, 0x235468, &[s, pa]);
                voices::on_rider_line(&mut ours, &a);
            }
            27 => {
                let v = [0u64, 3, 11, 12, 13, 1 << 33, u64::MAX][g.below(7) as usize] ^ g.below(2) as u64;
                let t1 = call(&mut r, 0x2307f0, &[v]) as u32;
                let t2 = call(&mut r, 0x230880, &[v]) as u32;
                ret = Some((t1 == speech::course_mask(v) && t2 == speech::char_mask_of(v), true));
            }
            28 => {
                let t = call(&mut r, 0x230910, &[]) as u32;
                ret = Some((t == speech::current_course_mask(ours.course), true));
            }
            29 => {
                let t = call(&mut r, 0x2309a8, &[s]) as u32;
                ret = Some((t == speech::mode_mask(ours.app_mode), true));
            }
            30 => {
                let out = r.mem.alloc(4);
                let (e, c, m) = (g.below(4) as i32 - 1, g.below(8) as i32, g.below(9) as i64);
                call(&mut r, 0x233138, &[out as u64, e as i64 as u64, c as u64, m as u64]);
                ret = Some((r.mem.read_u32(out) == speech::race_go_line(e, c, m), true));
            }
            31 => {
                call(&mut r, 0x2334b0, &[s]);
                ann.on_race_go(&mut ours, &car);
            }
            32 => {
                let none = g.below(5) == 0;
                let msg = [4i64, 0xb, 0xc, 0xd, 0xe, 0xf, 0x10, 0][g.below(8) as usize];
                let k = g.below(8) as i64 - 1;
                call(&mut r, 0x233220, &[s, if none { 0 } else { pa }, msg as u64, k as u64]);
                ann.on_score_message(&mut ours, if none { None } else { Some(&a) }, msg, k);
            }
            33 => {
                let none = g.below(2) == 0;
                let score = [999i64, 1000, 5000, 123456][g.below(4) as usize];
                let k = g.below(2) as i64;
                call(&mut r, 0x235ba8, &[s, if none { 0 } else { pa }, score as u64, k as u64]);
                ann.on_big_score(&mut ours, if none { None } else { Some(a.id) }, score, k);
            }
            34 => {
                let score = [999i64, 1000, 5000][g.below(3) as usize];
                call(&mut r, 0x2356f0, &[s, score as u64]);
                ann.check_big_score(&mut ours, score);
            }
            35 => {
                let v = g.next();
                call(&mut r, 0x235530, &[v as u64]);
                ann.queue_big_score(&mut ours, v);
            }
            36 => {
                call(&mut r, 0x231ff8, &[s]);
                ann.on_next_race(&mut ours, &car);
            }
            25 => {
                let theirs = call(&mut r, 0x233b70, &[pa]) as u32;
                ret = Some((theirs == speech::wipeout_grade(a.level), true));
            }
            _ => {
                let t1 = call(&mut r, 0x234698, &[pa]) as u32;
                let t2 = call(&mut r, 0x234730, &[pa]) as u32;
                ret = Some((t1 == speech::character_bit(a.voice) && t2 == t1, true));
            }
        }
        if let Some((t, o)) = ret {
            assert_eq!(t, o, "case {case} op {op}");
        }
        let theirs = w.borrow().log.clone();
        posted[op] += theirs.iter().filter(|e| matches!(e, Ev::Post(_))).count() as u32;
        assert_eq!(theirs, ours.log, "case {case} op {op}: calls\nriders {a:?}\n{b:?}");
        for k in 0..8 {
            assert_eq!(r.mem.read_u32(sys + 0x6e6c + 4 * k) as i32, ann.checkpoints[k as usize], "case {case} op {op}: checkpoint {k}");
        }
        let ring_now: Vec<u32> = (0..ann.ring.len() as u32).map(|k| r.mem.read_u32(ring_buf + 4 * k)).collect();
        assert_eq!(ring_now, ann.ring, "case {case} op {op}: ring");
        assert_eq!(r.mem.read_u32(ring + 8) as i32, ann.ring_write, "case {case} op {op}: ring write");
        assert_eq!(r.mem.read_u32(sys + 0x7dd8), ann.big_queued, "case {case} op {op}: queued");
    }
    eprintln!("ops {seen:?}\nposts by op {posted:?}");
    for op in (0..25).chain([31, 32, 33, 34, 36]) {
        assert!(op == 17 || posted[op] > 0, "op {op} never posted");
    }
}

//! The world nodes' load constructors (board row G12k) against the game in the function runner (TRICKY_ELF; skipped
//! without it): a random node of each class saved by our (checked) save into a stream, then loaded into a fresh node
//! by the game's load constructor and ours; the world set-up they call is stand-ins that log (and answer), every call
//! and all the memory compared. Also `cControllerNode` 0x13a840 and `cDeadNode` 0x13af38 on their own.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::snapshot::{self as sn, LoadEnv};

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

/// (the save, the load constructor)
const PAIRS: [(u32, u32); 26] = [
    (0x13b0b0, 0x13b078),
    (0x13b520, 0x13b128),
    (0x13fd90, 0x13fc58),
    (0x140080, 0x13fe50),
    (0x140440, 0x1401a0),
    (0x1408f0, 0x140590),
    (0x1411b0, 0x140a98),
    (0x141630, 0x1412e8),
    (0x141a88, 0x141800),
    (0x141f98, 0x141be0),
    (0x142a60, 0x1420b8),
    (0x1434f8, 0x143040),
    (0x144710, 0x143870),
    (0x1453f0, 0x144810),
    (0x145888, 0x145528),
    (0x146098, 0x145ba8),
    (0x146660, 0x146258),
    (0x147898, 0x146e10),
    (0x147ca0, 0x1479d0),
    (0x147f30, 0x147d78),
    (0x1481d0, 0x148028),
    (0x1489f0, 0x148680),
    (0x148dc0, 0x148ad0),
    (0x149158, 0x148e38),
    (0x13d3d8, 0x13d2f8),
    (0x1fb1a8, 0x1fa480),
];

const STAND_INS: [u32; 18] = [
    0x1403c0, 0x140400, 0x150670, 0x150818, 0x14fa70, 0x1421a8, 0x142220, 0x142ac8, 0x142af0, 0x143ff8, 0x144938, 0x145c00, 0x1814a8,
    0x1d8488, 0x1d8490, 0x148748, 0x148b28, 0x148ed8,
];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
    pool: u32,
}

struct Ours(World);
impl LoadEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        answer(func, args)
    }
    fn pool_alloc(&mut self, _m: &mut Mem, size: u32, name: u32) -> u32 {
        self.0.log.push(Call(0x23d3b0, vec![size, name]));
        self.0.pool
    }
}

/// What the stand-ins answer: a rider for `Race_GetRider`, a length's bits for `Spline_GetLength`
fn answer(func: u32, args: &[u32]) -> u32 {
    match func {
        0x1814a8 => 0x0100_0000 + args[1],
        0x251968 => (12.5f32 + args[0] as f32 / 1e9).to_bits(),
        _ => 0,
    }
}

fn args_of(c: &r5900::Cpu, func: u32) -> Vec<u32> {
    let n = match func {
        0x150818 | 0x142af0 | 0x1d8490 => 3,
        0x14fa70 | 0x1814a8 | 0x148ed8 => 2,
        _ => 1,
    };
    (0..n).map(|k| c.gpr(4 + k) as u32).collect()
}

#[test]
fn world_node_loads_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for (_, l) in PAIRS {
        r.allow(l);
    }
    for a in [
        0x13a888, 0x13a840, 0x13af38, 0x17d178, 0x17d090, 0x17cf98, 0x17cf50, 0x17d4e8, 0x17d0e0, 0x17d048, 0x192ca0, 0x192e40, 0x192df0,
        0x192ec0, 0x192ef8, 0x254a28, 0x1d8d80, 0x251a28, 0x2cc808, 0x2ce114,
    ] {
        r.allow(a);
    }
    let world = Rc::new(RefCell::new(World::default()));
    for f in STAND_INS {
        let w = world.clone();
        r.stub(f, move |c, _| {
            let args = args_of(c, f);
            let v = answer(f, &args);
            w.borrow_mut().log.push(Call(f, args));
            c.set_gpr(2, v as u64);
        });
    }
    let w = world.clone();
    r.stub(0x251968, move |c, _| {
        let args = vec![c.gpr(4) as u32];
        let v = answer(0x251968, &args);
        w.borrow_mut().log.push(Call(0x251968, args));
        c.set_f(0, f32::from_bits(v));
    });
    let w = world.clone();
    r.stub(0x23d3b0, move |c, _| {
        let mut w = w.borrow_mut();
        w.log.push(Call(0x23d3b0, vec![c.gpr(5) as u32, c.gpr(7) as u32]));
        let p = w.pool;
        c.set_gpr(2, p as u64);
    });
    let src = r.mem.alloc(0x800);
    let dst = r.mem.alloc(0x800);
    let inst = r.mem.alloc(0x100 * 8);
    let riders = r.mem.alloc(0xa00);
    let extra = r.mem.alloc(0x800);
    let pool = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let race = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x80);
    let course = r.mem.alloc(0x80);
    let lists = r.mem.alloc(14 * 0x34 + 0x10);
    let stream = r.mem.alloc(0x10);
    let sbuf = r.mem.alloc(0x1000);
    world.borrow_mut().pool = pool;
    let regions = [
        (0u32, 0x200u32),
        (0x337c58, 0x18),
        (0x34648c, 4),
        (0x34649c, 8),
        (0x31d338, 0x18),
        (0x31d498, 0x400),
        (src, 0x800),
        (dst, 0x800),
        (inst, 0x800),
        (riders, 0xa00),
        (extra, 0x800),
        (pool, 0x100),
        (app, 0x800),
        (game, 0x100),
        (race, 0x100),
        (ctl, 0x80),
        (course, 0x80),
        (lists, 14 * 0x34 + 0x10),
        (stream, 0x10),
        (sbuf, 0x1000),
    ];
    let snapshot = |r: &mut Runner| {
        let mut m = Mem::default();
        for (a, n) in regions {
            let mut b = vec![0u8; n as usize];
            r.mem.read(a, &mut b).unwrap();
            m.regions.push((a, b));
        }
        m
    };
    let mut g = Rng(0x6c6f_6164_736e_0b01);
    let mut seen = vec![0u32; PAIRS.len() + 2];
    for trial in 0..12000 {
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if ![0u32, 0x31d498, 0x31d338].contains(a) {
                v.iter_mut().for_each(|b| *b = g.next() as u8);
            }
        }
        m.w32(0x337c58, app);
        m.w32(app + 0x730, game);
        m.w32(game + 0x40, ctl);
        m.w32(game + 0x1c, race);
        m.w32(ctl + 0x58, 0);
        m.w32(ctl + 0xc, 0x1000);
        m.w32(0x34648c, course);
        m.w32(course + 0x48, inst);
        m.w32(course + 0xc, 8);
        // the world's lists, empty
        m.w32(0x337c60, 14);
        m.w32(0x337c64, lists + 0x10);
        for l in 0..14 {
            let at = lists + 0x10 + 0x34 * l;
            m.w32(at, (g.below(3) == 0) as u32);
            m.w32(at + 4, at + 0x10);
            m.w32(at + 0x14, at + 4);
            m.w32(at + 0x1c, at + 0x28);
            m.w32(at + 0x2c, at + 0x1c);
        }
        for i in 0..2 {
            m.w32(riders + 0x500 * i + 0x460, [0u32, 1, 2, 3][g.below(4) as usize]);
        }
        let k = g.below(PAIRS.len() as u32 + 2) as usize;
        let l = g.below(14);
        if k < PAIRS.len() {
            let (save, load) = PAIRS[k];
            // a random node of the class, its pointers aimed at real places (as the save test does)
            let dead_at = match save {
                0x146660 => Some(0x24u32),
                0x147f30 | 0x1489f0 | 0x149158 | 0x13d3d8 | 0x1fb1a8 => None,
                _ => Some(0),
            };
            if let Some(d) = dead_at {
                let i = if g.below(5) == 0 { 0 } else { inst + 0x100 * g.below(8) };
                m.w32(src + d + 0x28, i);
                if i != 0 {
                    m.w32(i + 0xe4, if g.below(2) == 0 { src + d } else { g.next() });
                }
            }
            m.w32(src + 0xe8, if g.below(3) == 0 { 0 } else { riders + 0x500 * g.below(2) });
            for off in [0x18u32, 0x60, 0x78, 0xe4] {
                if g.below(2) == 0 {
                    m.w32(src + off, if g.below(4) == 0 { 0 } else { inst + 0x100 * g.below(8) });
                }
            }
            if save == 0x1fb1a8 {
                m.w32(src + 0x7c, extra + 0x600);
                m.w32(src + 0x30, g.below(5));
                m.w32(src + 0x78, if g.below(4) == 0 { 0 } else { inst + 0x100 * g.below(8) });
            }
            if save == 0x1489f0 {
                // its kind (+0x50) indexes the counters at 0x31d498
                m.w32(src + 0x50, g.below(64));
            }
            if save == 0x149158 {
                for i in 0..0x80 {
                    if g.below(3) == 0 {
                        m.w16(src + 0x28 + 4 * i, 0xffff);
                    }
                }
            }
            m.w32(stream, sbuf);
            m.w32(stream + 4, sbuf);
            m.w32(stream + 8, sbuf);
            m.w32(stream + 0xc, 0);
            assert!(sn::save_by_address(&mut m, save, src, stream));
            m.w32(stream + 8, sbuf);
            for (a, v) in &m.regions {
                r.mem.write(*a, v).unwrap();
            }
            let what = format!("trial {trial} load {load:#x}");
            let mut ours = Ours(World { pool, ..Default::default() });
            assert!(sn::load_by_address(&mut m, &mut ours, load, dst, l, stream), "{what}: no port");
            *world.borrow_mut() = World { pool, ..Default::default() };
            for (k, a) in [dst, l, stream].iter().enumerate() {
                r.cpu.set_gpr(4 + k, *a as u64);
            }
            r.call(load).unwrap_or_else(|e| panic!("{what}: {e:?}"));
            assert_eq!(world.borrow().log, ours.0.log, "{what}: calls");
            compare(&mut r, &snapshot, &m, &what);
        } else {
            for (a, v) in &m.regions {
                r.mem.write(*a, v).unwrap();
            }
            let i = inst + 0x100 * g.below(8);
            let flag = g.below(2);
            let what = format!("trial {trial} constructor {}", k - PAIRS.len());
            let (f, args) = if k == PAIRS.len() {
                sn::controller_new(&mut m, dst, l, 5, i);
                (0x13a840, vec![dst, l, 5, i])
            } else {
                sn::dead_new(&mut m, dst, i, flag);
                (0x13af38, vec![dst, i, flag])
            };
            for (k, a) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + k, *a as u64);
            }
            r.call(f).unwrap_or_else(|e| panic!("{what}: {e:?}"));
            compare(&mut r, &snapshot, &m, &what);
        }
        seen[k] += 1;
    }
    eprintln!("loads {seen:?}");
}

fn compare(r: &mut Runner, snapshot: &dyn Fn(&mut Runner) -> Mem, m: &Mem, what: &str) {
    let theirs = snapshot(r);
    for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

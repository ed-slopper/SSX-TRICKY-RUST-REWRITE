//! The riders' and AI's snapshots (board row G12l) against the game in the function runner (TRICKY_ELF; skipped
//! without it): a random player, computer, boarder sequence, AI or AI world saved on both sides (stream and object
//! compared), then the game's stream loaded into a fresh one on both sides; the animation sequences on the layers'
//! rings are fakes whose save / load virtuals log, the world calls stand-ins; every call and all the memory compared.
//! The boarder's save writes a stack word the game never sets: it is taken from the game's stream before comparing.

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

const SEQ_SAVE: u32 = 0x01f3_0000;
const SEQ_LOAD: u32 = 0x01f3_0010;
const STAND_INS: [u32; 11] = [0x203b98, 0x198688, 0x1986d8, 0x203c20, 0x15a700, 0x1551c0, 0x260cb8, 0x159a80, 0x158dc0, 0x1125c8, 0x1163a0];
const MORE: [u32; 2] = [0x115fe8, 0x260060];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
    seq_at: u32,
    seq_vt: u32,
}

fn n_args(f: u32) -> usize {
    match f {
        0x198688 | 0x1986d8 | 0x1551c0 | 0x158dc0 | 0x1125c8 | 0x260060 => 2,
        0x260cb8 => 4,
        SEQ_SAVE | SEQ_LOAD => 2,
        _ => 1,
    }
}

/// The stand-ins' answers (and `BdrSeq_Create`'s new sequence, its vtable set)
fn answer(w: &mut World, m: &mut dyn FnMut(u32, u32), f: u32, a: &[u32]) -> u32 {
    match f {
        0x203b98 => a[0] % 7,
        0x198688 => a[1] % 300,
        0x1986d8 => (a[1] % 500).wrapping_neg(),
        0x260cb8 => a[1] ^ a[2] ^ a[3] ^ 0x5a5a,
        0x158dc0 => a[1] ^ 0x1234,
        0x15a700 => {
            let q = w.seq_at;
            w.seq_at += 0x40;
            m(q + 0x34, w.seq_vt);
            q
        }
        _ => 0,
    }
}

struct Ours(World);
impl LoadEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        answer(&mut self.0, &mut |a, v| m.w32(a, v), func, args)
    }
    fn pool_alloc(&mut self, _m: &mut Mem, _size: u32, _name: u32) -> u32 {
        unreachable!()
    }
}

#[test]
fn rider_and_ai_snapshots_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x1533f8, 0x153450, 0x13a460, 0x13a4b8, 0x117d20, 0x117ec0, 0x15a3d8, 0x15a4b0, 0x203aa0, 0x203ac8, 0x112790, 0x1127e0, 0x1164c0,
        0x1164d8, 0x160058, 0x1600d0, 0x15a798, 0x15a818, 0x123f08, 0x123f30, 0x157980, 0x1579a8, 0x154480, 0x1544a8, 0x17d260, 0x17d2a0,
        0x192c10, 0x192ca0, 0x2ce114, 0x2cc808,
    ]
    .into_iter()
    .chain(sn::RIDER_PARTS.iter().flat_map(|_| []))
    {
        r.allow(a);
    }
    for f in [
        0x109668, 0x109690, 0x10aed8, 0x10af00, 0x10c060, 0x10c088, 0x1101c0, 0x110218, 0x110770, 0x110798, 0x1043b8, 0x1043e0, 0x103e78,
        0x103ea0, 0x106a00, 0x106a28, 0x1061e8, 0x106210, 0x105030, 0x105058, 0x100558, 0x100580, 0x105ad0, 0x105af8, 0x108050, 0x108078,
        0x102dc0, 0x102de8, 0x106cd8, 0x106d00,
    ] {
        r.allow(f);
    }
    let world = Rc::new(RefCell::new(World::default()));
    for f in STAND_INS.iter().chain(MORE.iter()).copied().chain([SEQ_SAVE, SEQ_LOAD]) {
        let w = world.clone();
        r.stub(f, move |c, m| {
            let args: Vec<u32> = (0..n_args(f)).map(|k| c.gpr(4 + k) as u32).collect();
            let mut w = w.borrow_mut();
            w.log.push(Call(f, args.clone()));
            let v = answer(&mut w, &mut |a, v| m.write_u32(a, v), f, &args);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let src = r.mem.alloc(0x900);
    let dst = r.mem.alloc(0x900);
    let data_s = r.mem.alloc(0x5c00);
    let data_d = r.mem.alloc(0x5c00);
    let seqs = r.mem.alloc(0x800);
    let vts = r.mem.alloc(0x40);
    let trees = r.mem.alloc(0x40);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x80);
    let cellw = r.mem.alloc(0x60);
    let cells = r.mem.alloc(0x400);
    let stream = r.mem.alloc(0x10);
    let sbuf = r.mem.alloc(0x8000);
    let regions = [
        (0x337c58u32, 0x18u32),
        (0x337fb8, 0x10),
        (0x346488, 0x20),
        (0x31d318, 0x200),
        (src, 0x900),
        (dst, 0x900),
        (data_s, 0x5c00),
        (data_d, 0x5c00),
        (seqs, 0x800),
        (vts, 0x40),
        (trees, 0x40),
        (app, 0x800),
        (game, 0x100),
        (ctl, 0x80),
        (cellw, 0x60),
        (cells, 0x400),
        (stream, 0x10),
        (sbuf, 0x8000),
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
    let compare = |r: &mut Runner, m: &Mem, what: &str| {
        let theirs = snapshot(r);
        for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                let i = i & !3;
                panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
            }
        }
    };
    let mut g = Rng(0x7269_6465_7273_0c01);
    let mut seen = [0u32; 5];
    for trial in 0..1500 {
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if *a != 0x31d318 {
                v.iter_mut().for_each(|b| *b = g.next() as u8);
            }
        }
        m.w32(0x337c58, app);
        m.w32(app + 0x730, game);
        m.w32(game + 0x40, ctl);
        m.w32(ctl + 0x58, 0);
        m.w32(ctl + 0xc, 0x8000);
        m.w32(0x346488, cellw);
        m.w32(cellw + 0x4c, cells);
        m.w32(vts + 0x20, 0);
        m.w32(vts + 0x24, SEQ_SAVE);
        m.w32(vts + 0x28, 0);
        m.w32(vts + 0x2c, SEQ_LOAD);
        // the riders: their boarder data, the animation layers' rings (empty or a sequence or two)
        for (rider, data) in [(src, data_s), (dst, data_d)] {
            m.w32(rider, data);
            for i in 0..3 {
                let layer = data + 0x46c0 + 0x28 + 0x70 * i;
                let n = g.below(3);
                let mut prev = layer;
                for k in 0..n {
                    let q = seqs + 0x100 * i + 0x40 * k + if rider == dst { 0x400 } else { 0 };
                    m.w32(q + 0x34, vts);
                    m.w32(prev + if prev == layer { 0x64 } else { 0x2c }, q);
                    prev = q;
                }
                m.w32(prev + if prev == layer { 0x64 } else { 0x2c }, layer);
                m.w32(layer + 0x30, prev);
            }
            m.w16(rider + 0x89c, g.below(64) as u16);
        }
        // the sequences' trees, the AI world's cell numbers
        for (o, n) in [(src, g.below(4)), (dst, g.below(4))] {
            m.w32(o + 0x44, n);
            for k in 0..4 {
                m.w32(o + 0x78 + 4 * k, trees + 0x10 * k);
            }
        }
        for k in 0..0x6c {
            let v = match g.below(4) {
                0 => u32::MAX,
                1 => 0xffff,
                _ => g.below(0x100),
            };
            m.w32(src + 0x36c + 4 * k, v);
        }
        m.w32(stream, sbuf);
        m.w32(stream + 4, sbuf);
        m.w32(stream + 8, sbuf);
        m.w32(stream + 0xc, 0);
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        let kind = g.below(5) as usize;
        let seq_at = seqs + 0x700;
        let (save, load): (u32, u32) = [(0x1533f8, 0x153450), (0x13a460, 0x13a4b8), (0x15a3d8, 0x15a4b0), (0x112790, 0x1127e0), (0x1164c0, 0x1164d8)][kind];
        let what = format!("trial {trial} kind {kind}");
        // the save
        let mut ours = Ours(World { seq_at, seq_vt: vts, ..Default::default() });
        match kind {
            0 => sn::player_save(&mut m, &mut ours, src, stream),
            1 => sn::computer_save(&mut m, &mut ours, src, stream),
            2 => sn::boarder_sequence_save(&mut m, src, stream),
            3 => sn::ai_save(&mut m, src, stream),
            _ => sn::ai_world_save(&mut m, src, stream),
        }
        *world.borrow_mut() = World { seq_at, seq_vt: vts, ..Default::default() };
        r.cpu.set_gpr(4, src as u64);
        r.cpu.set_gpr(5, stream as u64);
        r.call(save).unwrap_or_else(|e| panic!("{what} save: {e:?}"));
        assert_eq!(world.borrow().log, ours.0.log, "{what} save: calls");
        if kind < 2 {
            // the word the game never sets: as the game's stack held it
            let at = sbuf + 0x450 + 4;
            let w = r.mem.read_u32(at);
            m.w32(at, w);
        }
        compare(&mut r, &m, &format!("{what} save"));
        // the load, into the other one
        m.w32(stream + 8, sbuf);
        r.mem.write_u32(stream + 8, sbuf);
        let mut ours = Ours(World { seq_at, seq_vt: vts, ..Default::default() });
        match kind {
            0 => sn::player_load(&mut m, &mut ours, dst, stream),
            1 => sn::computer_load(&mut m, &mut ours, dst, stream),
            2 => sn::boarder_sequence_load(&mut m, &mut ours, dst, stream),
            3 => sn::ai_load(&mut m, &mut ours, dst, stream),
            _ => sn::ai_world_load(&mut m, &mut ours, dst, stream),
        }
        *world.borrow_mut() = World { seq_at, seq_vt: vts, ..Default::default() };
        r.cpu.set_gpr(4, dst as u64);
        r.cpu.set_gpr(5, stream as u64);
        r.call(load).unwrap_or_else(|e| panic!("{what} load: {e:?}"));
        assert_eq!(world.borrow().log, ours.0.log, "{what} load: calls");
        compare(&mut r, &m, &format!("{what} load"));
        seen[kind] += 1;
    }
    eprintln!("kinds {seen:?}");
}

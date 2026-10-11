//! The ground probe against the original's (board row E8g): `Boarder_ProbeGround` 0x128ae8 and
//! `Boarder_GetFallLine` 0x128a18 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped
//! without it), `Vec4_Scale` and the fall line run as the original, the line's constructor recorded and the world's
//! ray cast answered, and `tricky_game::probe` on the same rider: every field, the line and the result bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::probe::{self, Hit, Rider};

type V4 = [f32; 4];

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
    fn unit(&mut self, up: f32) -> V4 {
        let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(up, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l, 0.0]
    }
    fn v(&mut self, k: f32) -> V4 {
        [self.f(-k, k), self.f(-k, k), self.f(-k, k), 0.0]
    }
}

type Line = (u32, V4, V4, u64);

struct W {
    r: Runner,
    rd: u32,
    stats: u32,
    hit: Rc<RefCell<Option<Hit>>>,
    line: Rc<RefCell<Vec<Line>>>,
}

fn rv(m: &mut r5900::Mem, a: u32) -> V4 {
    [m.read_f32(a), m.read_f32(a + 4), m.read_f32(a + 8), m.read_f32(a + 12)]
}
fn wv(m: &mut r5900::Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x102f50);
    r.allow(0x128a18);
    let hit = Rc::new(RefCell::new(None::<Hit>));
    let line = Rc::new(RefCell::new(Vec::new()));
    let l = line.clone();
    r.stub(0x25d7f0, move |cpu, mem| {
        let (from, to) = (rv(mem, cpu.gpr(5) as u32), rv(mem, cpu.gpr(6) as u32));
        l.borrow_mut().push((cpu.f(12).to_bits(), from, to, cpu.gpr(7)));
    });
    let h = hit.clone();
    r.stub(0x25aff8, move |cpu, mem| match *h.borrow() {
        None => cpu.set_f(0, -1.0),
        Some(h) => {
            let out = cpu.gpr(6) as u32;
            mem.write_u32(out, h.kind);
            wv(mem, out + 0x10, h.point);
            wv(mem, out + 0x20, h.normal);
            mem.write_u32(out + 0x34, h.surface);
            cpu.set_f(0, 0.4);
        }
    });
    let stats = r.mem.alloc(0x100);
    let rd = r.mem.alloc(0x6000);
    r.mem.write_u32(rd + 0x464, stats);
    Some(W { r, rd, stats, hit, line })
}

impl W {
    fn put(&mut self, p: &Rider) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        for (o, v) in [(0x140, p.pos), (0x180, p.fwd), (0x2a0, p.normal), (0x2d0, p.ground), (0x320, p.plane_fwd), (0x330, p.side), (0x2b0, p.fall)] {
            wv(m, rd + o, v);
        }
        m.write_f32(rd + 0x244, p.x244);
        m.write_f32(self.stats + 0x60, p.stat60);
        m.write_f32(rd + 0x1bc, p.height);
        m.write_u32(rd + 0x290, p.surface);
        m.write_u32(rd + 0x29c, p.surface_ticks);
        m.write_u32(rd + 0x2e0, p.kind);
    }
    fn get(&mut self, like: &Rider) -> Rider {
        let (m, rd) = (&mut self.r.mem, self.rd);
        Rider {
            pos: rv(m, rd + 0x140),
            fwd: rv(m, rd + 0x180),
            normal: rv(m, rd + 0x2a0),
            ground: rv(m, rd + 0x2d0),
            plane_fwd: rv(m, rd + 0x320),
            side: rv(m, rd + 0x330),
            fall: rv(m, rd + 0x2b0),
            height: m.read_f32(rd + 0x1bc),
            surface: m.read_u32(rd + 0x290),
            surface_ticks: m.read_u32(rd + 0x29c),
            kind: m.read_u32(rd + 0x2e0),
            ..*like
        }
    }
}

fn bits(p: &Rider) -> Vec<u32> {
    [p.pos, p.fwd, p.normal, p.ground, p.plane_fwd, p.side, p.fall]
        .iter()
        .flatten()
        .chain([p.height].iter())
        .map(|x| x.to_bits())
        .chain([p.surface, p.surface_ticks, p.kind])
        .collect()
}

#[test]
fn probe_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x9a0b_e000_0000_0001);
    let (mut hits, mut flat) = (0, 0);
    for case in 0..20000 {
        let normal = g.unit(0.3);
        let p0 = Rider {
            pos: g.v(50000.0),
            fwd: g.unit(-1.0),
            normal,
            ground: g.v(100.0),
            plane_fwd: g.v(1.0),
            side: g.unit(-1.0),
            fall: g.v(1.0),
            x244: g.f(-1.0, 1.0),
            stat60: g.f(0.0, 20.0),
            height: g.f(-5.0, 5.0),
            surface: g.next() % 4,
            surface_ticks: [0, 5, 0x8000_0001][(g.next() % 3) as usize],
            kind: g.next() % 4,
        };
        let hit = match g.next() % 5 {
            0 => None,
            k => Some(Hit {
                kind: g.next() % 3,
                point: g.v(50000.0),
                normal: match k {
                    1 => [0.0, 0.0, 1.0, 0.0],
                    // near vertical: the fall line's 0.0001 cut
                    2 => [g.f(-0.0003, 0.0003), g.f(-0.0003, 0.0003), 1.0, 0.0],
                    _ => g.unit(0.2),
                },
                surface: g.next() % 4,
            }),
        };
        if hit.is_some_and(|h| h.normal[2] == 1.0) {
            flat += 1;
        }
        *w.hit.borrow_mut() = hit;
        w.line.borrow_mut().clear();
        w.put(&p0);
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x128ae8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let ret = w.r.cpu.gpr(2);
        let theirs = w.get(&p0);
        let mut ours = p0;
        let on = probe::probe(&mut ours, hit);
        hits += on as u32;
        assert_eq!(bits(&theirs), bits(&ours), "case {case} hit {hit:?}\n rider {p0:?}\n game {theirs:?}\n ours {ours:?}");
        assert_eq!(ret, on as u64, "case {case}: result");
        let (from, to) = probe::line(&p0);
        let lines = w.line.borrow().clone();
        assert_eq!(lines.len(), 1, "case {case}: one line");
        let (k, f, t, kind) = lines[0];
        assert_eq!((k, f.map(f32::to_bits), t.map(f32::to_bits), kind), (0.5f32.to_bits(), from.map(f32::to_bits), to.map(f32::to_bits), 2), "case {case}: line");
    }
    assert!(hits > 10000 && flat > 2000, "hits {hits} flat {flat}");
}

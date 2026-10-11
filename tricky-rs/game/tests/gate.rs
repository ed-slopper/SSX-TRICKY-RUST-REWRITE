//! The start gate against the original's, bit for bit (board row G1c2): `GateAnticipateControl_Enter`, `_Update`,
//! `_Exit` and `GateLaunchControl_Enter`, `_Update` run in the function runner on the player's SLUS_203.26
//! (TRICKY_ELF; skipped without it), the clip marker and the gate timer answered and the motion, clip and state calls
//! recorded, and `tricky_game::gate` on the same rider.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::gate::{self, Rider, Rock};

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
}

struct W {
    r: Runner,
    rd: u32,
    stats: u32,
    st: u32,
    holder: u32,
    marker: Rc<RefCell<f32>>,
    timer: Rc<RefCell<f32>>,
    log: Rc<RefCell<Vec<(&'static str, u64)>>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let marker = Rc::new(RefCell::new(0.0f32));
    let timer = Rc::new(RefCell::new(0.0f32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let m = marker.clone();
    r.stub(0x15fde0, move |cpu, _| cpu.set_f(0, *m.borrow()));
    let t = timer.clone();
    r.stub(0x11e9d8, move |cpu, _| cpu.set_f(0, *t.borrow()));
    for (addr, name) in [(0x11f610u32, "motion"), (0x15fcf8, "clip"), (0x11f608, "state")] {
        let l = log.clone();
        r.stub(addr, move |cpu, _| l.borrow_mut().push((name, cpu.gpr(5))));
    }
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let st = r.mem.alloc(0x20);
    let holder = r.mem.alloc(8);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(st + 0x14, rd);
    r.mem.write_u32(holder, rd);
    Some(W { r, rd, stats, st, holder, marker, timer, log })
}

impl W {
    fn put(&mut self, p: &Rider, rock: &Rock) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        for (o, v) in [(0x238, p.lean), (0x23c, p.lean_rate), (0x240, p.lean_target), (0x1c0, p.timer), (0x1c8, p.launch)] {
            m.write_f32(rd + o, v);
        }
        for k in 0..4u32 {
            m.write_f32(rd + 0x150 + 4 * k, p.vel[k as usize]);
            m.write_f32(rd + 0x180 + 4 * k, p.board[k as usize]);
        }
        m.write_u32(rd + 0x424, p.motion);
        m.write_u8(self.stats + 0x15, p.start_stat);
        m.write_f32(self.st + 8, rock.still);
        m.write_f32(self.st + 0xc, rock.low);
        m.write_f32(self.st + 0x10, rock.high);
    }
    fn get(&mut self, like: &Rider) -> (Rider, Rock) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        let g = |m: &mut r5900::Mem, o: u32| m.read_f32(rd + o);
        let p = Rider {
            lean: g(m, 0x238),
            lean_rate: g(m, 0x23c),
            lean_target: g(m, 0x240),
            timer: g(m, 0x1c0),
            launch: g(m, 0x1c8),
            vel: [g(m, 0x150), g(m, 0x154), g(m, 0x158), g(m, 0x15c)],
            ..*like
        };
        let rock = Rock { still: m.read_f32(self.st + 8), low: m.read_f32(self.st + 0xc), high: m.read_f32(self.st + 0x10) };
        (p, rock)
    }
}

fn bits(p: &Rider, rock: &Rock) -> Vec<u32> {
    [p.lean, p.lean_rate, p.lean_target, p.timer, p.launch, rock.still, rock.low, rock.high].iter().chain(p.vel.iter()).map(|x| x.to_bits()).collect()
}

fn random(g: &mut Rng) -> (Rider, Rock) {
    let lean = match g.next() % 4 { 0 => 0.0, 1 => 1.0, 2 => 0.5, _ => g.f(0.0, 1.0) };
    let still = match g.next() % 3 { 0 => 0.0, 1 => 0.016666668, _ => g.f(0.0, 1.0) };
    (
        Rider {
            lean,
            lean_rate: g.f(0.0, 4.0),
            lean_target: if g.next().is_multiple_of(2) { g.f(0.0, 1.0) } else { 1.0 },
            timer: g.f(0.0, 5.0),
            launch: g.f(0.0, 2000.0),
            vel: if g.next().is_multiple_of(2) { [0.0; 4] } else { [g.f(-100.0, 100.0), g.f(-100.0, 100.0), 0.0, 0.0] },
            board: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-0.3, 0.3), 0.0],
            motion: [1, 2, 4][(g.next() % 3) as usize],
            start_stat: g.next() as u8,
        },
        Rock { still, low: g.f(0.0, 0.5), high: g.f(0.5, 1.0) },
    )
}

#[test]
fn anticipate_is_the_originals() {
    let Some(mut w) = world() else { return };
    let pad = w.r.mem.alloc(4);
    let mut g = Rng(0x6a7e_0000_0000_0001);
    for case in 0..20000 {
        let (p0, rock0) = random(&mut g);
        let (marker, timer) = (g.f(0.2, 1.0), g.f(0.0, 4.0));
        let word = g.next();
        *w.marker.borrow_mut() = marker;
        *w.timer.borrow_mut() = timer;
        w.put(&p0, &rock0);
        w.r.mem.write_u32(pad, word);
        w.r.cpu.set_gpr(4, w.st as u64);
        w.r.cpu.set_gpr(5, pad as u64);
        w.r.call(0x105f58).unwrap();
        let (mut p, mut rock) = (p0, rock0);
        gate::anticipate_update(word, marker, timer, &mut rock, &mut p);
        let (tp, trock) = w.get(&p0);
        assert_eq!(bits(&tp, &trock), bits(&p, &rock), "case {case}: Update pad {word:#x}\n game {tp:?} {trock:?}\n ours {p:?} {rock:?}");

        w.put(&p0, &rock0);
        w.r.cpu.set_gpr(4, w.st as u64);
        w.r.call(0x106140).unwrap();
        let mut p = p0;
        gate::anticipate_exit(&rock0, &mut p);
        let (tp, trock) = w.get(&p0);
        assert_eq!(bits(&tp, &trock), bits(&p, &rock0), "case {case}: Exit {p0:?} {rock0:?}");

        w.put(&p0, &rock0);
        w.r.cpu.set_gpr(4, w.st as u64);
        w.r.call(0x105f48).unwrap();
        let mut rock = rock0;
        gate::anticipate_enter(&mut rock);
        let (tp, trock) = w.get(&p0);
        assert_eq!(bits(&tp, &trock), bits(&p0, &rock), "case {case}: Enter");
    }
}

#[test]
fn launch_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x1a0c_0000_0000_0001);
    let mut pushed = 0;
    for case in 0..20000 {
        let (p0, rock0) = random(&mut g);
        let marker = match g.next() % 4 { 0 => 1.0, 1 => p0.lean, _ => g.f(0.0, 1.0) };
        let timer = g.f(0.0, 4.0);
        *w.marker.borrow_mut() = marker;
        *w.timer.borrow_mut() = timer;
        w.put(&p0, &rock0);
        w.r.cpu.set_gpr(4, w.holder as u64);
        w.r.call(0x106228).unwrap();
        let mut p = p0;
        gate::launch_enter(&mut p);
        let (tp, trock) = w.get(&p0);
        assert_eq!(bits(&tp, &trock), bits(&p, &rock0), "case {case}: Enter");

        w.put(&p0, &rock0);
        w.log.borrow_mut().clear();
        w.r.cpu.set_gpr(4, w.holder as u64);
        w.r.call(0x1062b0).unwrap();
        let mut p = p0;
        let c = gate::launch_update(marker, timer, &mut p);
        let (tp, trock) = w.get(&p0);
        assert_eq!(bits(&tp, &trock), bits(&p, &rock0), "case {case}: Update {p0:?} marker {marker}");
        let mut want = vec![];
        if c.pushed {
            want.push(("motion", 2));
            pushed += 1;
        }
        if c.ride {
            if p0.motion != 2 {
                want.push(("motion", 2));
            }
            want.push(("clip", 0x1fd));
            want.push(("state", 7));
        }
        assert_eq!(*w.log.borrow(), want, "case {case}: calls");
    }
    assert!(pushed > 1000, "pushed {pushed}");
}

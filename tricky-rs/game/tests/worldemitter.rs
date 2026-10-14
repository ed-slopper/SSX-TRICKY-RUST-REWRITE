//! `tricky_game::worldemitter` (board row G10a2) against the original in the function runner (TRICKY_ELF; skipped
//! without it).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::worldemitter::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

#[derive(Debug, PartialEq, Clone)]
enum Ev {
    A(u32, u32, u32),
    B(u32, u32, u32),
    Up(u32),
}

struct E {
    g: u32,
    log: Vec<Ev>,
}
impl Env for E {
    fn global(&mut self) -> u32 {
        self.g
    }
    fn stop_a(&mut self, global: u32, handle: u32) {
        self.log.push(Ev::A(global, handle, 0x3e80_0000));
    }
    fn stop_b(&mut self, global: u32, handle: u32) {
        self.log.push(Ev::B(global, handle, 0x3e80_0000));
    }
    fn update(&mut self, index: usize) {
        self.log.push(Ev::Up(index as u32));
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn table(g: &mut Rng) -> Vec<u32> {
    (0..COUNT * WORDS)
        .map(|k| match k % WORDS {
            0 => [0, 1, 1, 2][g.next() as usize % 4],
            1 | 2 | 0xf => g.next() % 3,
            0xd | 0xe => g.next() % 2,
            _ => g.next() % 5 + if g.next() % 4 == 0 { 0xffff_fffb } else { 0 },
        })
        .collect()
}

fn put(r: &mut Runner, at: u32, t: &[u32]) {
    for (k, w) in t.iter().enumerate() {
        r.mem.write_u32(at + 4 * k as u32, *w);
    }
}

fn get(r: &mut Runner, at: u32) -> Vec<u32> {
    (0..COUNT * WORDS).map(|k| r.mem.read_u32(at + 4 * k as u32)).collect()
}

#[test]
fn emitter_functions_match_the_original() {
    let Some(mut r) = runner() else { return };
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(Vec::new()));
    let (l1, l2, l3) = (log.clone(), log.clone(), log.clone());
    r.stub(0x20c2a0, move |c, _| {
        assert_eq!(c.gpr(6) as u32, 1);
        l1.borrow_mut().push(Ev::A(c.gpr(4) as u32, c.gpr(5) as u32, c.f(12).to_bits()));
    });
    r.stub(0x220f98, move |c, _| l2.borrow_mut().push(Ev::B(c.gpr(4) as u32, c.gpr(5) as u32, c.f(12).to_bits())));
    let (tab, mgr) = (r.mem.alloc(COUNT as u32 * 0x50), r.mem.alloc(0x10));
    r.stub(0x22bf60, move |c, _| l3.borrow_mut().push(Ev::Up(((c.gpr(4) as u32) - tab) / 0x50)));
    r.mem.write_u32(mgr, tab);
    r.mem.write_u32(0x343050, 0);
    let mut g = Rng(0x5eed_0f_3c_9a71_b2d4);
    for case in 0..20_000 {
        let t = table(&mut g);
        let glob = g.next();
        r.mem.write_u32(0x343050, glob);
        let (a, b, c) = (g.next() % 3, g.next() % 3, g.next() % 3);
        // Matches, Find, FindFree.
        put(&mut r, tab, &t);
        let i = (g.next() as usize) % COUNT;
        r.cpu.set_gpr(4, (tab + (i * 0x50) as u32) as u64);
        r.cpu.set_gpr(5, a as u64);
        r.cpu.set_gpr(6, b as u64);
        r.cpu.set_gpr(7, c as u64);
        r.call(0x22bf30).unwrap();
        assert_eq!(r.cpu.gpr(2) & 0xff, matches(&t[i * WORDS..], a, b, c) as u64, "case {case} matches");
        r.cpu.set_gpr(4, mgr as u64);
        r.cpu.set_gpr(5, a as u64);
        r.cpu.set_gpr(6, b as u64);
        r.cpu.set_gpr(7, c as u64);
        r.allow(0x22bf30);
        r.call(0x22d1b8).unwrap();
        let want = find(&t, a, b, c).map_or(0, |i| tab + (i * 0x50) as u32);
        assert_eq!(r.cpu.gpr(2) as u32, want, "case {case} find");
        r.cpu.set_gpr(4, mgr as u64);
        r.call(0x22d188).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32, find_free(&t).map_or(0, |i| tab + (i * 0x50) as u32), "case {case} find_free");
        // Clear.
        r.cpu.set_gpr(4, (tab + (i * 0x50) as u32) as u64);
        r.call(0x22d080).unwrap();
        let mut w = t.clone();
        clear(&mut w[i * WORDS..]);
        assert_eq!(get(&mut r, tab), w, "case {case} clear");
        // Stop.
        put(&mut r, tab, &t);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, (tab + (i * 0x50) as u32) as u64);
        r.call(0x22c5a0).unwrap();
        let mut env = E { g: glob, log: Vec::new() };
        let mut w = t.clone();
        stop(&mut w[i * WORDS..], &mut env);
        assert_eq!(get(&mut r, tab), w, "case {case} stop");
        assert_eq!(*log.borrow(), env.log, "case {case} stop log");
        // StopAll, UpdateAll.
        for (which, addr) in [(0, 0x22d408u32), (1, 0x22d370)] {
            put(&mut r, tab, &t);
            log.borrow_mut().clear();
            r.cpu.set_gpr(4, mgr as u64);
            r.call(addr).unwrap();
            let mut env = E { g: glob, log: Vec::new() };
            let mut w = t.clone();
            if which == 0 {
                stop_all(&mut w, &mut env);
            } else {
                update_all(&mut w, &mut env);
            }
            assert_eq!(get(&mut r, tab), w, "case {case} {which}");
            assert_eq!(*log.borrow(), env.log, "case {case} {which} log");
        }
    }
}

#[test]
fn falloff_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x1357_9bdf_2468_ace0);
    let kinds = [-1.0f32, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 2.9, 5.99, -0.5, 100.0, 1e20, -1e20, f32::NAN];
    for case in 0..60_000 {
        let k = kinds[g.next() as usize % kinds.len()];
        let x = match g.next() % 8 {
            0 => 0.7,
            1 => f32::from_bits(0x3f33_3333),
            2 => 0.0,
            3 => 1.0,
            4 => 1.5,
            _ => (g.next() % 40_000) as f32 / 10_000.0 - 1.0,
        };
        let f3 = (g.next() % 1000) as f32 / 7.0;
        r.cpu.set_f(12, k);
        r.cpu.set_f(13, x);
        r.cpu.set_f(3, f3);
        r.call(0x22bc40).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), falloff(k, x, f3).to_bits(), "case {case} kind {k} x {x} f3 {f3}");
    }
}

// ---- GatherForListener ----

use std::collections::BTreeMap;

struct Shadow(BTreeMap<u32, u32>);
impl Mem for Shadow {
    fn u32(&self, a: u32) -> u32 {
        *self.0.get(&a).unwrap_or_else(|| panic!("read of unwritten {a:#x} (keys near: {:?})", self.0.range(a.saturating_sub(0x40)..a + 0x40).map(|(k, _)| *k).collect::<Vec<_>>()))
    }
}

type Call = (u32, u32, u32, u32, u64, u32, u32, [u32; 3]);

struct GEnv(Rc<RefCell<Vec<Call>>>);
impl GatherEnv for GEnv {
    fn add_sound(&mut self, volume: f32, global: u32, instance: u32, index: u32, arg3: u64, record_word_1: u32, faded: u32, centre: [f32; 3]) {
        self.0.borrow_mut().push((volume.to_bits(), global, instance, index, arg3, record_word_1, faded, centre.map(f32::to_bits)));
    }
}

struct Wr<'a> {
    r: &'a mut Runner,
    s: BTreeMap<u32, u32>,
}
impl Wr<'_> {
    fn u(&mut self, a: u32, v: u32) {
        self.r.mem.write_u32(a, v);
        self.s.insert(a, v);
    }
    fn f(&mut self, a: u32, v: f32) {
        self.u(a, v.to_bits());
    }
}

fn rf(g: &mut Rng, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * (g.next() % 1_000_000) as f32 / 1_000_000.0
}

#[test]
fn gather_for_listener_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let log: Rc<RefCell<Vec<Call>>> = Rc::new(RefCell::new(Vec::new()));
    let base = r.mem.alloc(0x2000);
    let (sys, cells, nodes, groups, insts, blocks, lis) = (base, base + 0x100, base + 0x200, base + 0x300, base + 0x400, base + 0x800, base + 0x1800);
    let l2 = log.clone();
    r.stub(0x22d260, move |c, m| {
        let t2 = c.gpr(10) as u32;
        let centre = [m.read_u32(t2), m.read_u32(t2 + 4), m.read_u32(t2 + 8)];
        l2.borrow_mut().push((c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7), c.gpr(8) as u32, c.gpr(9) as u32, centre));
    });
    for a in [0x22bc40u32, 0x250fd0, 0x137140] {
        r.allow(a);
    }
    let mut g = Rng(0x6a09_e667_f3bc_c908);
    let mut hits = [0u32; 4];
    for case in 0..30_000 {
        let mut w = Wr { r: &mut r, s: BTreeMap::new() };
        let (gw, gh) = (1 + g.next() % 3, 1 + g.next() % 3);
        let cs = rf(&mut g, 20.0, 200.0);
        let (x0, y0) = (rf(&mut g, -100.0, 100.0), rf(&mut g, -100.0, 100.0));
        w.u(sys + 0x10, gw);
        w.u(sys + 0x14, gh);
        w.u(sys + 0x18, cells);
        w.f(sys + 0x1c, x0);
        w.f(sys + 0x20, y0);
        w.f(sys + 0x24, cs);
        w.u(0x34370c, g.next());
        let (cx, cy) = ((g.next() % (gw + 2)) as i32 - 1, (g.next() % (gh + 2)) as i32 - 1);
        let lx = x0 + (cx as f32 + rf(&mut g, 0.0, 0.999)) * cs;
        let ly = y0 + (cy as f32 + rf(&mut g, 0.0, 0.999)) * cs;
        let lz = rf(&mut g, -50.0, 50.0);
        w.f(lis, lx);
        w.f(lis + 4, ly);
        w.f(lis + 8, lz);
        let (mut nn, mut ni, mut nb) = (0u32, 0u32, 0u32);
        for k in 0..gw * gh {
            let head = if g.next() % 3 == 0 { 0 } else { 1 };
            let mut prev_next: Option<u32> = None;
            let mut first = 0;
            if head == 1 {
                for _ in 0..1 + g.next() % 3 {
                    if nn >= 8 {
                        break;
                    }
                    let node = nodes + nn * 8;
                    let grp = groups + nn * 8;
                    let inst = insts + ni * 0x40;
                    let blk = blocks + nb * 0x200;
                    nn += 1;
                    ni += 1;
                    nb += 1;
                    w.u(node, grp);
                    w.u(node + 4, 0);
                    if let Some(p) = prev_next {
                        w.u(p, node);
                    } else {
                        first = node;
                    }
                    prev_next = Some(node + 4);
                    w.u(grp, inst);
                    w.u(grp + 4, blk);
                    w.f(inst + 0x30, lx + rf(&mut g, -30.0, 30.0));
                    w.f(inst + 0x34, ly + rf(&mut g, -30.0, 30.0));
                    w.f(inst + 0x38, lz + rf(&mut g, -30.0, 30.0));
                    let count = g.next() % 5;
                    w.u(blk, g.next());
                    w.u(blk + 4, count);
                    let mut at = blk + 8;
                    for ri in 0..count {
                        // an unknown kind has no size in the original, so only the last record may be one
                        let kind = [0u32, 0, 1, 1, 2, 2, 3, 3, if ri + 1 == count { 4 } else { 3 }][g.next() as usize % 9];
                        w.u(at, kind);
                        w.u(at + 4, g.next());
                        for q in 0..3 {
                            w.f(at + 8 + 4 * q, rf(&mut g, -20.0, 20.0));
                        }
                        let fk = |g: &mut Rng| (g.next() % 6) as f32;
                        let size = match kind {
                            0 => {
                                w.f(at + 0x14, rf(&mut g, 1.0, 60.0));
                                let v = fk(&mut g);
                                w.f(at + 0x18, v);
                                0x1c
                            }
                            1 => {
                                for q in 0..3 {
                                    let v = if g.next() % 6 == 0 { 0.0 } else { rf(&mut g, 1.0, 40.0) };
                                    w.f(at + 0x14 + 4 * q, v);
                                }
                                let (ax, ay) = (rf(&mut g, -1.0, 1.0) + 1.2, rf(&mut g, -1.0, 1.0));
                                let az = rf(&mut g, -1.0, 1.0);
                                w.f(at + 0x20, ax);
                                w.f(at + 0x24, ay);
                                w.f(at + 0x28, az);
                                let v = fk(&mut g);
                                w.f(at + 0x2c, v);
                                0x30
                            }
                            2 => {
                                w.f(at + 0x14, rf(&mut g, 1.0, 60.0));
                                let k = rf(&mut g, 0.3, 0.9);
                                for q in 0..3 {
                                    let v = rf(&mut g, -1.0, 1.0) * k;
                                    w.f(at + 0x18 + 4 * q, v);
                                }
                                let v = fk(&mut g);
                                w.f(at + 0x24, v);
                                let v = rf(&mut g, -0.9, 0.9);
                                w.f(at + 0x28, v);
                                let v = fk(&mut g);
                                w.f(at + 0x2c, v);
                                0x30
                            }
                            3 => {
                                w.f(at + 0x14, rf(&mut g, 1.0, 60.0));
                                0x18
                            }
                            _ => 0,
                        };
                        at += size;
                    }
                }
            }
            w.u(cells + k * 0x14 + 0x10, first);
        }
        let arg3 = ((g.next() as u64) << 32) | g.next() as u64;
        let shadow = Shadow(w.s.clone());
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, lis as u64);
        r.cpu.set_gpr(6, arg3);
        r.call(0x22b370).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let orig = std::mem::take(&mut *log.borrow_mut());
        gather_for_listener(&shadow, sys, lis, arg3, &mut GEnv(log.clone()));
        let mine = std::mem::take(&mut *log.borrow_mut());
        for c in &mine {
            hits[((c.1 == 0) as usize) * 0] += 0;
            let _ = c;
        }
        assert_eq!(orig, mine, "case {case}");
        hits[0] += (!mine.is_empty()) as u32;
        hits[1] += mine.len() as u32;
    }
    assert!(hits[0] > 3_000, "{hits:?}");
}

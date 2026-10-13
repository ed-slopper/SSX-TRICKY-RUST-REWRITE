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

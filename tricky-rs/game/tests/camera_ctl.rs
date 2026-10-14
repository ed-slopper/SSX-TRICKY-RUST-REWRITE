//! `tricky_game::camera_ctl` (board row E11g) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random camera objects, named cameras and riders, every call out logged on both sides and all memory
//! compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::camera_ctl::*;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;

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

type L = Rc<RefCell<Vec<(&'static str, u32, u32)>>>;

/// The game around the camera: the race camera's globals (one region from 0x3368a0), and an arena holding a second
/// camera, the named cameras' blocks, the riders and their characters, the race and the output words
const GLOBALS: (u32, u32) = (0x3368a0, 0xf00);
const NAMES: [&str; 8] = ["chase reverse", "chase board", "chase eyes", "chase near", "chase far", "chase over", "replay start", "replay cam"];
const BLOCK: u32 = 0x170;
const RIDER: u32 = 0x5800;
const NRIDERS: u32 = 5;

struct At {
    b: u32,
}
impl At {
    fn cam2(&self) -> u32 {
        self.b
    }
    fn block(&self, i: u32) -> u32 {
        self.b + 0x1000 + BLOCK * i
    }
    fn app(&self) -> u32 {
        self.b + 0x2000
    }
    fn race(&self) -> u32 {
        self.b + 0x2800
    }
    fn world(&self) -> u32 {
        self.b + 0x2c00
    }
    fn chars(&self) -> u32 {
        self.b + 0x2d00
    }
    fn out(&self) -> u32 {
        self.b + 0x2f00
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x3000 + RIDER * i
    }
    fn len() -> u32 {
        0x3000 + RIDER * NRIDERS
    }
}

fn block_of(name: &str) -> u32 {
    NAMES.iter().position(|n| *n == name).map(|i| i as u32).unwrap_or(9) % 8
}

fn regions(a: &At) -> Vec<(u32, u32)> {
    vec![GLOBALS, (GAPP, 4), (a.b, At::len())]
}

fn snapshot(r: &mut Runner, a: &At) -> Mem {
    let mut m = Mem::default();
    for (at, n) in regions(a) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn compare(r: &mut Runner, a: &At, m: &Mem, what: &str) {
    let theirs = snapshot(r, a);
    for ((at, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

fn fpick(g: &mut Rng, xs: &[f32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize].to_bits()
}

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..GLOBALS.1 / 4 {
        r.mem.write_u32(GLOBALS.0 + 4 * k, g.next());
    }
    // the arena: small words mostly, so indexes and modes land in range
    for k in 0..At::len() / 4 {
        let v = if g.below(4) == 0 { g.next() } else { g.below(12) };
        r.mem.write_u32(a.b + 4 * k, v);
    }
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    w(r, GAPP, a.app());
    w(r, a.app() + 0x730, a.race());
    w(r, a.race() + 0x1c, a.world());
    w(r, a.world() + 0x88, g.below(NRIDERS + 1));
    for cam in [MAIN, a.cam2()] {
        w(r, cam, g.below(6));
        w(r, cam + 0x428, [2, 3, 4, 5, 6, 7, 3][g.below(7) as usize]);
        w(r, cam + 0x14, g.below(8));
        w(r, cam + 0x34, (g.below(4) == 0) as u32);
        w(r, cam + 0xc, g.below(5));
        w(r, cam + 0x1e4, (g.below(3) == 0) as u32);
        for k in 0..26 {
            let s = cam + 0x4e8 + 0x20 * k;
            w(r, s, if g.below(4) == 0 { 0 } else { a.rider(g.below(NRIDERS)) });
        }
    }
    w(r, PRESET, g.below(11));
    w(r, CHASE_RIDER, g.below(NRIDERS + 1));
    w(r, REPLAY_MODE, (g.below(3) == 0) as u32);
    w(r, SECOND, if g.below(2) == 0 { MAIN } else { a.cam2() });
    for i in 0..8 {
        let b = a.block(i);
        w(r, b + 0xcc, fpick(g, &[150.0, 200.0, 500.0, 1200.0, 1300.0, 600.5]));
        w(r, b + 0xdc, fpick(g, &[0.0, 1.5, -3.0, 6.2]));
        w(r, b + 0x100, fpick(g, &[0.0, 0.99, 1.0, 1.01, -1.2, -0.5]));
        w(r, b + 0x120 + 0x20, fpick(g, &[0.2, 0.5, 1.0, 1.9, 2.5, 0.26]));
    }
    for i in 0..NRIDERS {
        let rd = a.rider(i);
        w(r, rd + 0x464, a.chars() + 0x40 * i);
        w(r, a.chars() + 0x40 * i + 0x38, if g.below(4) == 0 { g.below(30) } else { g.below(12) });
        w(r, rd + 0x460, g.below(NRIDERS + 1));
    }
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

fn cstr(m: &mut r5900::Mem, mut at: u32) -> String {
    let mut s = String::new();
    loop {
        let mut b = [0u8];
        m.read(at, &mut b).unwrap();
        if b[0] == 0 {
            return s;
        }
        s.push(b[0] as char);
        at += 1;
    }
}

struct Ours(L, u32);
impl Ours {
    fn rider_at(&self, i: u32) -> u32 {
        if i == 7 { 0 } else { self.1 + 0x3000 + RIDER * (i % NRIDERS) }
    }
}
impl CamCtlEnv for Ours {
    fn find_camera(&mut self, name: &str) -> u32 {
        self.0.borrow_mut().push(("find", block_of(name), 0));
        self.1 + 0x1000 + BLOCK * block_of(name)
    }
    fn cut(&mut self, _m: &mut Mem, cam: u32) {
        self.0.borrow_mut().push(("cut", cam, 0));
    }
    fn script_stop(&mut self, _m: &mut Mem, cam: u32) {
        self.0.borrow_mut().push(("stop", cam, 1));
    }
    fn rider(&mut self, i: u32) -> u32 {
        self.0.borrow_mut().push(("rider", i, 0));
        self.rider_at(i)
    }
    fn track_rider(&mut self, _m: &mut Mem, slot: u32, cut: u32) {
        self.0.borrow_mut().push(("track", slot, cut));
    }
    fn cut_to(&mut self, _m: &mut Mem, cam: u32, name: &str) {
        self.0.borrow_mut().push(("cutto", cam, block_of(name)));
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [0x16dd68, 0x16dda8, 0x16ddb8, 0x16ded0, 0x16dcc0, 0x1741d0, 0x1776d0, 0x173eb8, 0x173a98, 0x1731f8, 0x173c50, 0x1786d0, 0x178968, 0x178a28] {
        r.allow(f);
    }
    let base = a.b;
    let l2 = l.clone();
    r.stub(0x16e518, move |c, m| {
        let n = cstr(m, arg(c, 5));
        let i = block_of(&n);
        l2.borrow_mut().push(("find", i, 0));
        c.set_gpr(2, (base + 0x1000 + BLOCK * i) as u64);
    });
    let l2 = l.clone();
    r.stub(0x177db0, move |c, _| l2.borrow_mut().push(("cut", arg(c, 4), 0)));
    let l2 = l.clone();
    r.stub(0x175eb8, move |c, _| l2.borrow_mut().push(("stop", arg(c, 4), arg(c, 5))));
    let l2 = l.clone();
    r.stub(0x1814a8, move |c, _| {
        let i = arg(c, 5);
        l2.borrow_mut().push(("rider", i, 0));
        let v = if i == 7 { 0 } else { base + 0x3000 + RIDER * (i % NRIDERS) };
        c.set_gpr(2, v as u64);
    });
    let l2 = l.clone();
    r.stub(0x173270, move |c, _| l2.borrow_mut().push(("track", arg(c, 4), arg(c, 5))));
    let l2 = l.clone();
    r.stub(0x174c98, move |c, m| {
        let n = cstr(m, arg(c, 5));
        l2.borrow_mut().push(("cutto", arg(c, 4), block_of(&n)));
    });
}

/// One function on both sides from the same random game: `regs` sets the runner's arguments, `ours` runs the port
/// and gives what it returns (compared with v0 when `Some`)
fn check(r: &mut Runner, a: &At, l: &L, g: &mut Rng, cases: u32, addr: u32, what: &str, mut regs: impl FnMut(&mut Runner, &mut Rng) -> Vec<u64>, mut ours: impl FnMut(&mut Mem, &mut Ours, &[u64]) -> Option<u32>) {
    for case in 0..cases {
        build(g, r, a);
        let args = regs(r, g);
        let mut m = snapshot(r, a);
        l.borrow_mut().clear();
        r.call(addr).unwrap_or_else(|e| panic!("{what} case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        let v0 = r.cpu.gpr(2) as u32;
        let ret = ours(&mut m, &mut Ours(l.clone(), a.b), &args);
        let mine = std::mem::take(&mut *l.borrow_mut());
        assert_eq!(theirs, mine, "{what} case {case}: calls");
        if let Some(v) = ret {
            assert_eq!(v0, v, "{what} case {case}: result");
        }
        compare(r, a, &m, &format!("{what} case {case}"));
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn camera_control_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(At::len()) };
    let l: L = Rc::new(RefCell::new(Vec::new()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1786_d0aa_1234_0001);
    let cam2 = a.cam2();
    let pick_cam = move |g: &mut Rng| if g.below(2) == 0 { MAIN } else { cam2 };
    check(&mut r, &a, &l, &mut g, 3_000, 0x1786d0, "select preset", |r, g| {
        let p = g.below(12).wrapping_sub(1);
        r.cpu.set_gpr(4, p as i32 as i64 as u64);
        vec![p as u64]
    }, |m, e, a| {
        select_preset(m, e, a[0] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x178680, "next preset", |_, _| vec![], |m, e, _| {
        next_preset(m, e);
        None
    });
    // GetChaseParams writes its five outputs: compared through the arena
    let out = a.out();
    check(&mut r, &a, &l, &mut g, 2_000, 0x178968, "chase params out", move |r, _| {
        for (k, reg) in [4usize, 5, 6, 7, 8].iter().enumerate() {
            r.cpu.set_gpr(*reg, (out + 4 * k as u32) as u64);
        }
        vec![]
    }, move |m, e, _| {
        let c = chase_params(m, e);
        m.w32(out, c.heading.to_bits());
        m.w32(out + 4, c.pitch.to_bits());
        m.w32(out + 8, c.distance.to_bits());
        m.w32(out + 12, c.height.to_bits());
        m.w32(out + 16, c.rider);
        None
    });
    let floats = [0.0f32, 0.5, 1.0, 1.5, -1.5, 150.0, 250.0, 1250.0, 0.2, 2.0, 0.3, 3.0];
    check(&mut r, &a, &l, &mut g, 6_000, 0x178a28, "set chase", |r, g| {
        let f: Vec<f32> = (0..4).map(|_| floats[g.below(floats.len() as u32) as usize]).collect();
        for (k, v) in f.iter().enumerate() {
            r.cpu.set_f(12 + k, *v);
        }
        let i = g.below(NRIDERS + 2);
        let cut = g.below(2);
        r.cpu.set_gpr(4, i as u64);
        r.cpu.set_gpr(5, cut as u64);
        vec![f[0].to_bits() as u64, f[1].to_bits() as u64, f[2].to_bits() as u64, f[3].to_bits() as u64, i as u64, cut as u64]
    }, |m, e, a| {
        let f = |k: usize| f32::from_bits(a[k] as u32);
        set_chase_params(m, e, Chase { heading: f(0), pitch: f(1), distance: f(2), height: f(3), rider: a[4] as u32 }, a[5] != 0);
        None
    });
    check(&mut r, &a, &l, &mut g, 6_000, 0x178be8, "adjust chase", |r, g| {
        let s = [1.0f32, 0.5, 2.0, 30.0][g.below(4) as usize];
        let w = g.below(11);
        r.cpu.set_f(12, s);
        r.cpu.set_gpr(4, w as u64);
        vec![s.to_bits() as u64, w as u64]
    }, |m, e, a| {
        adjust_chase(m, e, f32::from_bits(a[0] as u32), a[1] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1776d0, "set mode", move |r, g| {
        let (c, v) = (pick_cam(g), g.below(8));
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, v as u64);
        vec![c as u64, v as u64]
    }, |m, _, a| {
        set_mode(m, a[0] as u32, a[1] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 3_000, 0x1776f0, "look back", move |r, g| {
        let (c, v) = (pick_cam(g), g.below(3));
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, v as u64);
        vec![c as u64, v as u64]
    }, |m, _, a| {
        set_look_back(m, a[0] as u32, a[1] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 1_000, 0x173e08, "mode index", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, _, a| Some(mode_index(m, a[0] as u32)));
    check(&mut r, &a, &l, &mut g, 2_000, 0x173e70, "cycle mode", move |r, g| {
        let (c, b) = (pick_cam(g), g.below(2));
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, b as u64);
        vec![c as u64, b as u64]
    }, |m, e, a| {
        cycle_mode(m, e, a[0] as u32, a[1] != 0);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x173eb0, "set mode index", move |r, g| {
        let (c, i) = (pick_cam(g), g.below(6));
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, i as u64);
        vec![c as u64, i as u64]
    }, |m, e, a| {
        set_mode_index(m, e, a[0] as u32, a[1] as u32);
        None
    });
    let base = a.b;
    check(&mut r, &a, &l, &mut g, 5_000, 0x173c50, "focus rider", move |r, g| {
        let c = pick_cam(g);
        let rd = if g.below(5) == 0 { 0 } else { base + 0x3000 + RIDER * g.below(NRIDERS) };
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, rd as u64);
        vec![c as u64, rd as u64]
    }, |m, e, a| {
        set_focus_rider(m, e, a[0] as u32, a[1] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x173f60, "slot index", move |r, g| {
        let (c, s) = (pick_cam(g), g.below(26));
        r.cpu.set_gpr(4, c as u64);
        r.cpu.set_gpr(5, s as u64);
        vec![c as u64, s as u64]
    }, |m, e, a| Some(rider_slot_index(m, e, a[0] as u32, a[1] as u32)));
    check(&mut r, &a, &l, &mut g, 500, 0x174008, "get focus", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, _, a| Some(focus_rider(m, a[0] as u32)));
    check(&mut r, &a, &l, &mut g, 500, 0x173c18, "focus point", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, _, a| Some(focus_point(m, a[0] as u32)));
    check(&mut r, &a, &l, &mut g, 2_000, 0x193ce0, "is focus", move |r, g| {
        let rd = if g.below(5) == 0 { 0 } else { base + 0x3000 + RIDER * g.below(NRIDERS) };
        r.cpu.set_gpr(5, rd as u64);
        vec![rd as u64]
    }, |m, e, a| Some(is_focus_rider(m, e, a[0] as u32) as u32));
    check(&mut r, &a, &l, &mut g, 1_000, 0x174c70, "chase if idle", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        // "chase far" (0x382e48) or "replay cam" (0x382e78)
        let n = [0x382e48u32, 0x382e78][g.below(2) as usize];
        r.cpu.set_gpr(5, n as u64);
        vec![c as u64, n as u64]
    }, |m, e, a| {
        chase_if_idle(m, e, a[0] as u32, if a[1] == 0x382e48 { "chase far" } else { "replay cam" });
        None
    });
    check(&mut r, &a, &l, &mut g, 1_000, 0x173a98, "reset", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, e, a| {
        reset(m, e, a[0] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 1_000, 0x173a88, "control reset", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, e, a| {
        control_reset(m, e, a[0] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1731f8, "reset track", move |r, g| {
        let s = pick_cam(g) + 0x4e8 + 0x20 * g.below(26);
        r.cpu.set_gpr(4, s as u64);
        vec![s as u64]
    }, |m, _, a| {
        reset_rider_track(m, a[0] as u32);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x174010, "update tracks", move |r, g| {
        let c = pick_cam(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c as u64]
    }, |m, e, a| {
        update_rider_tracks(m, e, a[0] as u32);
        None
    });
}

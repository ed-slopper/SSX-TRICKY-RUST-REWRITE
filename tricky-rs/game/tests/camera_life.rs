//! `tricky_game::camera_ctl`'s frame and the game's cameras (board row E11h) against the original in the function
//! runner (TRICKY_ELF; skipped without it): random cameras, trigger regions, riders and races, every call out
//! logged on both sides and all memory compared.

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

#[derive(Default)]
struct Log {
    calls: Vec<(&'static str, u32, u32)>,
    answers: Vec<u32>,
    next: usize,
}
impl Log {
    fn take(&mut self) -> u32 {
        let v = self.answers.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        v
    }
}
type L = Rc<RefCell<Log>>;

const GLOBALS: (u32, u32) = (0x3368a0, 0xf00);
const LOW: (u32, u32) = (0x3365f0, 0x14);
const SAVED_MODES: (u32, u32) = (0x32ca38, 8);
const NAMES: [&str; 9] = ["chase reverse", "chase board", "chase eyes", "chase near", "chase far", "chase over", "replay start", "replay cam", "manual target"];
const BLOCK: u32 = 0x170;
const RIDER: u32 = 0x5800;
const NRIDERS: u32 = 5;
const NREGIONS: u32 = 8;
const REGION: u32 = 0x180;
const RENDERER: u32 = 0x7e0000;

struct At {
    b: u32,
}
impl At {
    fn cam2(&self) -> u32 {
        self.b
    }
    fn app(&self) -> u32 {
        self.b + 0x2000
    }
    fn mgr(&self) -> u32 {
        self.b + 0x2100
    }
    fn path(&self) -> u32 {
        self.b + 0x2180
    }
    fn vt(&self) -> u32 {
        self.b + 0x21c0
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
    fn entry(&self, i: u32) -> u32 {
        self.b + 0x2f80 + 4 * i
    }
    fn region(&self, i: u32) -> u32 {
        self.b + 0x3000 + REGION * i
    }
    fn alloc(&self, i: u32) -> u32 {
        self.b + 0x3c00 + 0x1000 * i
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x7000 + RIDER * i
    }
    fn len() -> u32 {
        0x7000 + RIDER * NRIDERS
    }
}

fn block_of(name: &str) -> u32 {
    NAMES.iter().position(|n| *n == name).map(|i| i as u32).unwrap_or(0)
}

fn regions(a: &At) -> Vec<(u32, u32)> {
    vec![SAVED_MODES, LOW, GLOBALS, (GAPP, 4), (a.b, At::len())]
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

fn put_str(r: &mut Runner, at: u32, s: &str) {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    r.mem.write(at, &b).unwrap();
}

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..GLOBALS.1 / 4 {
        r.mem.write_u32(GLOBALS.0 + 4 * k, g.next());
    }
    for k in 0..LOW.1 / 4 {
        r.mem.write_u32(LOW.0 + 4 * k, g.next());
    }
    r.mem.write_u32(SAVED_MODES.0, g.below(5));
    r.mem.write_u32(SAVED_MODES.0 + 4, g.below(5));
    for k in 0..At::len() / 4 {
        let v = if g.below(4) == 0 { g.next() } else { g.below(12) };
        r.mem.write_u32(a.b + 4 * k, v);
    }
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    w(r, GAPP, a.app());
    w(r, a.app() + 0x730, a.race());
    w(r, a.race() + 0x1c, a.world());
    w(r, a.race() + 0x40, if g.below(4) == 0 { 0 } else { a.mgr() });
    w(r, a.mgr() + 0x20, [0, 2, 3, 3, 5, 12][g.below(6) as usize]);
    w(r, a.mgr() + 0x30, g.below(3));
    w(r, a.mgr() + 0x34, g.below(3));
    let n = g.below(NRIDERS + 1);
    w(r, a.world() + 0x88, n);
    w(r, a.world() + 0x7c, [0, 1, 1, 2, 3, 4, 5][g.below(7) as usize].min(n.max(1)));
    for i in 0..8 {
        w(r, a.world() + 0x8c + 4 * i, a.entry(i));
        w(r, a.entry(i), a.rider(g.below(NRIDERS)));
        w(r, a.world() + 0xc4 + 4 * i, a.rider(g.below(NRIDERS)));
    }
    for cam in [MAIN, a.cam2()] {
        w(r, cam, g.below(6));
        w(r, cam + 4, g.below(2));
        w(r, cam + 0x10, fpick(g, &[0.0, 1.5, 100.25]));
        w(r, cam + 0x20, fpick(g, &[0.0, 0.002, 0.5, 0.97, 0.99, -0.1]));
        w(r, cam + 0x428, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0][g.below(12) as usize]);
        w(r, cam + 0x34, (g.below(4) == 0) as u32);
        w(r, cam + 0xc, g.below(5));
        w(r, cam + 0x1e4, (g.below(4) == 0) as u32);
        w(r, cam + 0xea8, (g.below(4) != 0) as u32);
        w(r, cam + 0xeb0, (g.below(5) == 0) as u32);
        w(r, cam + 0xeb4, (g.below(3) == 0) as u32);
        for k in 0..26 {
            let s = cam + 0x4e8 + 0x20 * k;
            w(r, s, if k > 0 && g.below(4) == 0 { 0 } else { a.rider(g.below(NRIDERS)) });
        }
        put_str(r, cam + 0x50, ["intro", "jump", "x"][g.below(3) as usize]);
    }
    w(r, PRESET, g.below(11));
    w(r, CHASE_RIDER, g.below(NRIDERS + 1));
    w(r, REPLAY_MODE, g.below(3));
    w(r, CAM_COUNT, g.below(3));
    w(r, MANUAL_FROM, if g.below(2) == 0 { MAIN } else { a.cam2() });
    for i in 0..NRIDERS {
        let rd = a.rider(i);
        w(r, rd + 0x464, a.chars() + 0x70 * i);
        r.mem.write_u8(a.chars() + 0x70 * i + 0x65, g.below(8) as u8);
        w(r, a.chars() + 0x70 * i + 0x38, g.below(12));
        w(r, rd + 0x424, [0, 3, 5, 6, 6, 2][g.below(6) as usize]);
        w(r, rd + 0x428, if g.below(6) == 0 { 0x12 } else { g.below(4) });
        w(r, rd + 0x418, [0xffff_ffff, 1, 0][g.below(3) as usize]);
        w(r, rd + 0x114, g.below(2));
    }
    for i in 0..NREGIONS {
        let rg = a.region(i);
        w(r, rg + 0x154, g.below(26));
        for o in [0x158u32, 0x15c, 0x160, 0x164, 0x168, 0x16c] {
            w(r, rg + o, (g.below(3) == 0) as u32);
        }
        r.mem.write_u16(rg + 0x170, (g.below(3) == 0) as u16);
        r.mem.write_u16(rg + 0x172, (g.below(3) == 0) as u16);
        put_str(r, rg + 0x120, "cut");
        put_str(r, rg + 0x12f, "pan");
        put_str(r, rg + 0x13e, ["intro", "jump", "y"][g.below(3) as usize]);
    }
    // the cameras' vtables (+0xeb8) and the control's (+8) for the destructors
    w(r, a.vt() + 0xc, RENDERER + 0xc);
    r.mem.write_u16(a.vt() + 8, 0);
    w(r, a.race() + 0x84, a.alloc(2));
    w(r, a.alloc(2) + 8, a.vt());
    w(r, MAIN + 0xeb8, a.vt());
    w(r, a.cam2() + 0xeb8, a.vt());
    w(r, a.race() + 0x298, g.below(3));
    w(r, a.race() + 0x294, g.below(3));
    w(r, a.race() + 0x68, g.below(3));
    for i in 0..3 {
        w(r, a.race() + 0xa0 + 0x80 * i, [MAIN, a.cam2()][g.below(2) as usize]);
        w(r, a.race() + 0xa4 + 0x80 * i, a.rider(g.below(NRIDERS)));
    }
}

/// The player slot of a character: always 0..3, so the first camera finds a player (with none the game keeps an
/// old register's value as the rider)
fn slot_of(ch: i8) -> i32 {
    (ch as i32).rem_euclid(4)
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

/// The regions each file lists: file k the regions with index ≡ k (mod 3)
fn first_region(base: u32, file: u32) -> u32 {
    let k = REGION_FILES.iter().position(|f| *f == file).unwrap() as u32;
    base + 0x3000 + REGION * k
}
fn next_region(base: u32, region: u32) -> u32 {
    let i = (region - base - 0x3000) / REGION + 3;
    if i < NREGIONS { base + 0x3000 + REGION * i } else { 0 }
}

struct Ours(L, u32);
impl Ours {
    fn push(&self, n: &'static str, a: u32, b: u32) {
        self.0.borrow_mut().calls.push((n, a, b));
    }
    fn take(&self, n: &'static str, a: u32, b: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.calls.push((n, a, b));
        l.take()
    }
}
impl CamCtlEnv for Ours {
    fn find_camera(&mut self, name: &str) -> u32 {
        self.push("find", block_of(name), 0);
        self.1 + 0x1000 + BLOCK * block_of(name)
    }
    fn cut(&mut self, _m: &mut Mem, cam: u32) {
        self.push("cut", cam, 0);
    }
    fn script_stop(&mut self, _m: &mut Mem, cam: u32) {
        self.push("stop", cam, 1);
    }
    fn rider(&mut self, i: u32) -> u32 {
        self.push("rider", i, 0);
        self.1 + 0x7000 + RIDER * (i % NRIDERS)
    }
    fn track_rider(&mut self, _m: &mut Mem, slot: u32, cut: u32) {
        self.push("track", slot, cut);
    }
    fn cut_to(&mut self, _m: &mut Mem, cam: u32, name: &str) {
        self.push("cutto", cam, block_of(name));
    }
}
impl CamFrameEnv for Ours {
    fn first_region(&mut self, file: u32) -> u32 {
        self.push("first", file, 0);
        first_region(self.1, file)
    }
    fn next_region(&mut self, file: u32, region: u32) -> u32 {
        self.push("next", file, region);
        next_region(self.1, region)
    }
    fn check_trigger(&mut self, _m: &mut Mem, region: u32, slots: u32, prev: u32, kind: u32) -> bool {
        self.push("trigger slots", slots, prev);
        self.take("trigger", region, kind) & 1 != 0
    }
    fn rider_ready(&mut self, rider: u32) -> bool {
        self.take("ready", rider, 0) & 1 != 0
    }
    fn script_running(&mut self, _m: &mut Mem, cam: u32, region: u32) -> bool {
        self.take("running", cam, region) & 1 != 0
    }
    fn script_start(&mut self, _m: &mut Mem, cam: u32, name: u32) {
        self.push("start", cam, name);
    }
    fn script_update(&mut self, _m: &mut Mem, cam: u32) {
        self.push("script", cam, 0);
    }
    fn update_path(&mut self, _m: &mut Mem, cam: u32) {
        self.push("path", cam, 0);
    }
    fn cast_clips(&mut self, _m: &mut Mem, cam: u32) {
        self.push("clips", cam, 0);
    }
    fn cut_fade(&mut self, _m: &mut Mem, cam: u32) {
        self.push("fade", cam, 0);
    }
}
impl CamLifeEnv for Ours {
    fn alloc(&mut self, m: &mut Mem, name: u32, size: u32) -> u32 {
        self.push("alloc", name, size);
        let p = self.1 + 0x3c00 + if size == 12 { 0x2000 } else { 0 } + (self.take("alloc at", 0, 0) % 2) * 0x1000;
        for k in 0..size {
            m.w8(p + k, 0);
        }
        p
    }
    fn cam_control_new(&mut self, _m: &mut Mem, p: u32) -> u32 {
        self.push("control", p, 9);
        p
    }
    fn player_slot(&mut self, character: i8) -> i32 {
        self.push("slot", character as i32 as u32, 0);
        slot_of(character)
    }
    fn layout_viewports(&mut self, _m: &mut Mem, race: u32) {
        self.push("layout", race, 0);
    }
    fn destroy(&mut self, _m: &mut Mem, obj: u32, _vt_at: u32) {
        self.push("destroy", obj, 3);
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    for f in [
        0x16dd68, 0x16dda8, 0x16ddb8, 0x16ded0, 0x16dcc0, 0x171fb0, 0x1703a0, 0x1730c0, 0x173c18, 0x174c70, 0x174010, 0x1731f8, 0x2fd008,
        0x173820, 0x173a88, 0x173a98, 0x173eb0, 0x173eb8, 0x1782a8, 0x173c50, 0x2cc808, 0x180518, 0x17f328, 0x181470,
    ] {
        r.allow(f);
    }
    let base = a.b;
    let st = |r: &mut Runner, addr: u32, f: Box<dyn Fn(&mut r5900::Cpu, &mut r5900::Mem, &mut Log)>| {
        let l2 = l.clone();
        r.stub(addr, move |c, m| f(c, m, &mut l2.borrow_mut()));
    };
    st(r, 0x16e518, Box::new(move |c, m, l| {
        let n = cstr(m, arg(c, 5));
        let i = block_of(&n);
        l.calls.push(("find", i, 0));
        c.set_gpr(2, (base + 0x1000 + BLOCK * i) as u64);
    }));
    st(r, 0x177db0, Box::new(|c, _, l| l.calls.push(("cut", arg(c, 4), 0))));
    st(r, 0x175eb8, Box::new(|c, _, l| l.calls.push(("stop", arg(c, 4), arg(c, 5)))));
    st(r, 0x1814a8, Box::new(move |c, _, l| {
        let i = arg(c, 5);
        l.calls.push(("rider", i, 0));
        c.set_gpr(2, (base + 0x7000 + RIDER * (i % NRIDERS)) as u64);
    }));
    st(r, 0x173270, Box::new(|c, _, l| l.calls.push(("track", arg(c, 4), arg(c, 5)))));
    st(r, 0x174c98, Box::new(|c, m, l| {
        let n = cstr(m, arg(c, 5));
        l.calls.push(("cutto", arg(c, 4), block_of(&n)));
    }));
    st(r, 0x16e408, Box::new(move |c, _, l| {
        assert_eq!(arg(c, 5), 4);
        l.calls.push(("first", arg(c, 4), 0));
        c.set_gpr(2, first_region(base, arg(c, 4)) as u64);
    }));
    st(r, 0x16e468, Box::new(move |c, _, l| {
        assert_eq!(arg(c, 6), 4);
        l.calls.push(("next", arg(c, 4), arg(c, 5)));
        c.set_gpr(2, next_region(base, arg(c, 5)) as u64);
    }));
    st(r, 0x171e28, Box::new(|c, m, l| {
        let region = m.read_u32(arg(c, 4) + 4);
        l.calls.push(("trigger slots", arg(c, 5), arg(c, 6)));
        l.calls.push(("trigger", region, arg(c, 7)));
        let v = l.take() & 1;
        c.set_gpr(2, v as u64);
    }));
    st(r, 0x11f488, Box::new(|c, _, l| {
        l.calls.push(("ready", arg(c, 4), 0));
        let v = l.take() & 1;
        c.set_gpr(2, v as u64);
    }));
    st(r, 0x175e50, Box::new(|c, _, l| {
        l.calls.push(("running", arg(c, 4), arg(c, 5)));
        let v = l.take() & 1;
        c.set_gpr(2, v as u64);
    }));
    st(r, 0x175de0, Box::new(|c, _, l| {
        assert_eq!(arg(c, 6), 1);
        l.calls.push(("start", arg(c, 4), arg(c, 5)));
    }));
    st(r, 0x175f50, Box::new(|c, _, l| l.calls.push(("script", arg(c, 4), 0))));
    st(r, 0x175cb0, Box::new(|c, _, l| l.calls.push(("path", arg(c, 4), 0))));
    st(r, 0x177020, Box::new(|c, _, l| l.calls.push(("clips", arg(c, 4), 0))));
    st(r, 0x174b38, Box::new(|c, _, l| l.calls.push(("fade", arg(c, 4), 0))));
    st(r, 0x2ccf70, Box::new(move |c, _, l| {
        assert_eq!(arg(c, 6), 0);
        l.calls.push(("alloc", arg(c, 4), arg(c, 5)));
        let p = base + 0x3c00 + if arg(c, 5) == 12 { 0x2000 } else { 0 } + (l.take() % 2) * 0x1000;
        l.calls.push(("alloc at", 0, 0));
        c.set_gpr(2, p as u64);
    }));
    st(r, 0x1781c0, Box::new(|c, _, l| {
        l.calls.push(("control", arg(c, 4), arg(c, 5)));
        c.set_gpr(2, c.gpr(4));
    }));
    st(r, 0x166550, Box::new(|c, _, l| {
        assert_eq!(arg(c, 4), 0x32b390);
        l.calls.push(("slot", arg(c, 5) as i8 as i32 as u32, 0));
        c.set_gpr(2, slot_of(arg(c, 5) as i8) as i64 as u64);
    }));
    st(r, 0x180350, Box::new(|c, _, l| l.calls.push(("layout", arg(c, 4), 0))));
    st(r, RENDERER + 0xc, Box::new(|c, _, l| l.calls.push(("destroy", arg(c, 4), arg(c, 5)))));
    st(r, 0x197ef0, Box::new(move |c, _, l| {
        assert_eq!(arg(c, 4), 0x337fb8);
        l.calls.push(("main path", 0, 0));
        c.set_gpr(2, (base + 0x2180) as u64);
    }));
    st(r, 0x17d950, Box::new(|c, _, l| l.calls.push(("nodes", arg(c, 4), arg(c, 5)))));
    st(r, 0x17f390, Box::new(|c, _, l| l.calls.push(("cameras", arg(c, 4), 0))));
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

/// One function on both sides: `regs` sets the arguments, `ours` runs the port (its result compared with v0)
#[allow(clippy::too_many_arguments)]
fn check(r: &mut Runner, a: &At, l: &L, g: &mut Rng, cases: u32, addr: u32, what: &str, mut regs: impl FnMut(&mut Runner, &mut Rng) -> Vec<u32>, mut ours: impl FnMut(&mut Mem, &mut Ours, &[u32]) -> Option<u32>) {
    for case in 0..cases {
        build(g, r, a);
        let args = regs(r, g);
        let answers: Vec<u32> = (0..64).map(|_| g.next()).collect();
        let mut m = snapshot(r, a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.call(addr).unwrap_or_else(|e| panic!("{what} case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        let v0 = r.cpu.gpr(2) as u32;
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let ret = ours(&mut m, &mut Ours(l.clone(), a.b), &args);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "{what} case {case}: calls");
        if let Some(v) = ret {
            assert_eq!(v0, v, "{what} case {case}: result");
        }
        compare(r, a, &m, &format!("{what} case {case}"));
    }
}

#[test]
fn camera_frame_and_life_match_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(At::len()) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1770_78bb_e11a_0001);
    let cam2 = a.cam2();
    let race = a.race();
    let pick = move |g: &mut Rng| if g.below(2) == 0 { MAIN } else { cam2 };
    check(&mut r, &a, &l, &mut g, 20_000, 0x177078, "frame", move |r, g| {
        let c = pick(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c]
    }, |m, e, a| {
        frame(m, e, a[0]);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1782a8, "register", move |r, g| {
        let c = pick(g);
        r.cpu.set_gpr(5, c as u64);
        vec![c]
    }, |m, _, a| {
        register_camera(m, a[0]);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x181470, "get camera", |r, g| {
        let i = g.below(4);
        r.cpu.set_gpr(4, i as u64);
        vec![i]
    }, |m, _, a| Some(game_camera(m, a[0])));
    check(&mut r, &a, &l, &mut g, 2_000, 0x17f328, "cut all", move |r, _| {
        r.cpu.set_gpr(4, race as u64);
        vec![]
    }, move |m, e, _| {
        cut_all(m, e, race);
        None
    });
    check(&mut r, &a, &l, &mut g, 4_000, 0x180030, "create", move |r, _| {
        r.cpu.set_gpr(4, race as u64);
        vec![]
    }, move |m, e, _| {
        create_cameras(m, e, race);
        None
    });
    check(&mut r, &a, &l, &mut g, 2_000, 0x1802b8, "delete", move |r, g| {
        if g.below(3) == 0 {
            r.mem.write_u32(race + 0xa0 + 0x80 * g.below(2), 0);
        }
        r.cpu.set_gpr(4, race as u64);
        vec![]
    }, move |m, e, _| {
        delete_cameras(m, e, race);
        None
    });
    check(&mut r, &a, &l, &mut g, 4_000, 0x180518, "assign", move |r, _| {
        r.cpu.set_gpr(4, race as u64);
        vec![]
    }, move |m, e, _| {
        assign_camera_riders(m, e, race);
        None
    });
    let blk = a.alloc(0);
    check(&mut r, &a, &l, &mut g, 1_000, 0x173820, "init", move |r, _| {
        r.cpu.set_gpr(4, blk as u64);
        vec![]
    }, move |m, e, _| Some(bx_camera_init(m, e, blk)));
    let path = a.path();
    check(&mut r, &a, &l, &mut g, 500, 0x1739f0, "main path", move |r, g| {
        let c = pick(g);
        r.cpu.set_gpr(4, c as u64);
        vec![c]
    }, move |m, e, a| {
        e.push("main path", 0, 0);
        focus_from_main_path(m, a[0], path);
        None
    });
    check(&mut r, &a, &l, &mut g, 500, 0x181878, "step", move |r, _| {
        r.cpu.set_gpr(4, race as u64);
        vec![]
    }, move |m, e, _| {
        e.push("nodes", 0x337c60, 9);
        e.push("cameras", race, 0);
        cameras_step_count(m, race);
        None
    });
}

//! Board row G1h: `cBoarder::vf7` 0x117198 run whole in the function runner on the player's SLUS_203.26 (TRICKY_ELF;
//! skipped without it) against `tricky_game::boarder_tick::boarder_tick`, bit for bit: every field it reads and
//! writes, and every call it makes, in order, with its arguments. Each call is a stub that changes a few random
//! fields of the rider (the same in both), so the order of the tick's reads around its calls is checked too.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boarder_tick::{boarder_tick, TickEnv, TickGame, TickRider, EASES};

const APP: u32 = 0x337c58;
const WORLD: u32 = 0x346488;
const HANDLER: u32 = 0x00e0_0000;
const CONTROLS: u32 = 0x4000;
/// the vtable's this-adjust for the controls (the short at +0x50)
const ADJUST: u32 = 0x40;

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
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.next() as usize % xs.len()]
    }
    fn chance(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

macro_rules! pick {
    ($g:expr, [$($x:expr),* $(,)?]) => {{
        let xs = [$($x),*];
        $g.pick(&xs)
    }};
}

enum Slot<'a> {
    F(&'a mut f32),
    U(&'a mut u32),
}

/// Every field of `TickRider` with its offset in the boarder.
fn slots(r: &mut TickRider) -> Vec<(u32, Slot<'_>)> {
    use Slot::{F, U};
    let mut v: Vec<(u32, Slot)> = Vec::new();
    v.push((0x46b0, U(&mut r.active)));
    v.push((0x18, F(&mut r.weather)));
    v.push((0x1c, F(&mut r.meter)));
    v.push((0x12c, F(&mut r.time_scale)));
    v.push((0x130, F(&mut r.boost_level)));
    v.push((0x134, F(&mut r.boost_time)));
    v.push((0x138, F(&mut r.spin_boost_time)));
    v.push((0x148, F(&mut r.pos_z)));
    for (k, x) in r.vel.iter_mut().enumerate() {
        v.push((0x150 + 4 * k as u32, F(x)));
    }
    v.push((0x164, F(&mut r.travelled)));
    v.push((0x168, F(&mut r.pitch)));
    for (k, x) in r.fwd.iter_mut().enumerate() {
        v.push((0x180 + 4 * k as u32, F(x)));
    }
    for (k, x) in r.right.iter_mut().enumerate() {
        v.push((0x190 + 4 * k as u32, F(x)));
    }
    v.push((0x1c4, F(&mut r.speed_limit)));
    for (e, at) in r.eases.iter_mut().zip(EASES) {
        for (k, x) in e.iter_mut().enumerate() {
            v.push((at + 4 * k as u32, F(x)));
        }
    }
    v.push((0x274, F(&mut r.x274)));
    v.push((0x278, F(&mut r.x278)));
    v.push((0x27c, F(&mut r.x27c)));
    v.push((0x280, F(&mut r.x280)));
    v.push((0x290, U(&mut r.surface)));
    v.push((0x294, F(&mut r.x294)));
    v.push((0x298, F(&mut r.x298)));
    v.push((0x2e4, F(&mut r.impact)));
    for (k, x) in r.crash_normal.iter_mut().enumerate() {
        v.push((0x2f0 + 4 * k as u32, F(x)));
    }
    v.push((0x300, F(&mut r.knocks)));
    v.push((0x304, U(&mut r.reset_flag)));
    v.push((0x308, F(&mut r.height)));
    v.push((0x30c, F(&mut r.on_snow)));
    v.push((0x384, U(&mut r.events_prev)));
    v.push((0x388, U(&mut r.events)));
    v.push((0x420, U(&mut r.class)));
    v.push((0x424, U(&mut r.motion)));
    v.push((0x428, U(&mut r.state)));
    for (k, x) in r.board_up.iter_mut().enumerate() {
        v.push((0x4a80 + 4 * k as u32, F(x)));
    }
    v
}

fn words(r: &TickRider) -> Vec<(u32, u32)> {
    let mut c = *r;
    slots(&mut c)
        .into_iter()
        .map(|(o, s)| (o, match s { Slot::F(x) => x.to_bits(), Slot::U(x) => *x }))
        .collect()
}

fn set(r: &mut TickRider, off: u32, w: u32) {
    for (o, s) in slots(r) {
        if o == off {
            match s {
                Slot::F(x) => *x = f32::from_bits(w),
                Slot::U(x) => *x = w,
            }
            return;
        }
    }
    panic!("no field at {off:#x}");
}

/// A field change a call makes: offset and word
type Patch = Vec<(u32, u32)>;

/// A random value for the field at `off`, as a call might leave it.
fn value(g: &mut Rng, off: u32) -> u32 {
    match off {
        0x424 => pick!(g, [1u32, 2, 3, 4, 5, 6]),
        0x428 => pick!(g, [0x16u32, 3, 0xe, 0x10, 2]),
        0x420 => g.next() % 3,
        0x388 | 0x384 => pick!(g, [0u32, 0x4000, 0x4001, g.next()]),
        0x46b0 | 0x304 | 0x290 => g.next() % 2,
        0x150..=0x15c => g.f(-3000.0, 3000.0).to_bits(),
        0x2e4 => g.f(0.0, 5.0).to_bits(),
        0x300 => g.f(0.0, 13.0).to_bits(),
        0x148 => g.f(-12000.0, 2000.0).to_bits(),
        _ => g.f(-2.0, 2.0).to_bits(),
    }
}

/// The fields a call may change in the test: the ones the tick reads after its calls.
const PATCHABLE: [u32; 20] = [
    0x424, 0x428, 0x420, 0x388, 0x384, 0x150, 0x154, 0x158, 0x15c, 0x2e4, 0x300, 0x148, 0x134, 0x130, 0x1c, 0x1c4, 0x18,
    0x2f0, 0x304, 0x46b0,
];

fn random_patch(g: &mut Rng) -> Patch {
    let n = pick!(g, [0usize, 0, 1, 2, 3]);
    (0..n).map(|_| { let o = g.pick(&PATCHABLE); (o, value(g, o)) }).collect()
}

#[derive(Clone, Debug, PartialEq)]
struct Call {
    addr: u32,
    a0: u32,
    extra: Vec<u32>,
}

fn random_rider(g: &mut Rng) -> TickRider {
    let mut r = TickRider::default();
    let v4 = |g: &mut Rng, s: f32| [g.f(-s, s), g.f(-s, s), g.f(-s, s), pick!(g, [0.0, 1.0, g.f(-s, s)])];
    r.active = if g.chance(12) { 0 } else { 1 };
    r.weather = g.f(0.0, 1.0);
    r.meter = pick!(g, [0.0, 1.0, f32::from_bits(0x3f2a_7e78), f32::from_bits(0x3eaa_d18b), g.f(-0.2, 1.0)]);
    r.time_scale = pick!(g, [1.0, g.f(0.0, 2.0)]);
    r.boost_level = pick!(g, [0.0, 0.0, g.f(-1.0, 3.0)]);
    r.boost_time = pick!(g, [0.0, 0.0, f32::from_bits(0x3c88_8889), g.f(-0.1, 0.1), g.f(0.0, 3.0)]);
    r.spin_boost_time = pick!(g, [0.0, f32::from_bits(0x3c88_8889), g.f(-0.1, 0.1), g.f(0.0, 3.0)]);
    r.pos_z = g.f(-12000.0, 3000.0);
    let speed = pick!(g, [100.0, 2800.0, 3100.0, 3400.0, 5000.0]);
    r.vel = v4(g, speed);
    r.travelled = g.f(-1e5, 1e5);
    r.pitch = g.f(-3.5, 3.5);
    r.fwd = v4(g, 1.0);
    r.right = v4(g, 1.0);
    r.board_up = v4(g, 1.0);
    match g.next() % 8 {
        0 => r.board_up = [0.0; 4],
        1 => {
            r.board_up = [0.0, 0.0, 1.0, 0.0];
            r.fwd = [1.0, 0.0, 0.0, 0.0];
            r.right = [0.0, 0.0, pick!(g, [1.0, -1.0, 0.0]), 0.0];
        }
        _ => {}
    }
    r.speed_limit = pick!(g, [2788.8394, 3347.2222, g.f(2700.0, 3500.0)]);
    for e in r.eases.iter_mut() {
        let now = g.f(-2.0, 2.0);
        let rate = pick!(g, [0.0, g.f(0.0, 120.0)]);
        let target = match g.next() % 4 {
            0 => now,
            1 => now + rate * f32::from_bits(0x3c88_8889) * pick!(g, [1.0, -1.0, 0.5, -0.5]),
            _ => g.f(-2.0, 2.0),
        };
        *e = [now, rate, target];
    }
    r.x274 = g.f(-1.0, 1.0);
    r.x278 = g.f(-1.0, 1.0);
    r.x27c = pick!(g, [r.x274, g.f(-1.0, 1.0)]);
    r.x280 = pick!(g, [r.x278, g.f(-1.0, 1.0)]);
    r.surface = g.next() % 20;
    r.x294 = g.f(-10.0, 10.0);
    r.x298 = g.f(-10.0, 10.0);
    r.impact = g.f(0.0, 5.5);
    r.crash_normal = v4(g, 1.0);
    r.knocks = g.f(0.0, 13.0);
    r.reset_flag = g.next() % 2;
    r.height = g.f(-5.0, 15.0);
    r.on_snow = g.f(0.0, 1.0);
    r.events_prev = g.next();
    r.events = pick!(g, [0, 0x4000, g.next()]);
    r.class = g.next() % 3;
    r.motion = pick!(g, [1, 2, 3, 4, 5, 6]);
    r.state = pick!(g, [0x16, 3, 0xe, 0x10, 2]);
    r.on_snow = g.f(0.0, 1.0);
    r
}

struct Ours<'a> {
    paused: bool,
    pad: [u32; 4],
    patches: &'a std::collections::HashMap<u32, Patch>,
    rider: u32,
    log: Vec<Call>,
}

impl Ours<'_> {
    fn call(&mut self, r: &mut TickRider, addr: u32, a0: u32, extra: Vec<u32>) {
        self.log.push(Call { addr, a0, extra });
        if let Some(p) = self.patches.get(&addr) {
            for (o, w) in p {
                set(r, *o, *w);
            }
        }
    }
}

impl TickEnv for Ours<'_> {
    fn gather_world_emitters(&mut self, r: &mut TickRider) {
        self.call(r, 0x1166f0, self.rider, vec![]);
    }
    fn is_paused(&mut self) -> bool {
        self.log.push(Call { addr: 0x17f078, a0: 0, extra: vec![] });
        self.paused
    }
    fn update_motion(&mut self, r: &mut TickRider) {
        self.call(r, 0x11c770, HANDLER, vec![]);
    }
    fn add_speed_sample(&mut self, r: &mut TickRider, speed: f32) {
        self.call(r, 0x155dc8, self.rider + 0x5820, vec![speed.to_bits()]);
    }
    fn update_race_line(&mut self, r: &mut TickRider) {
        self.call(r, 0x118298, self.rider, vec![]);
    }
    fn update_path_ahead(&mut self, r: &mut TickRider) {
        self.call(r, 0x118090, self.rider, vec![]);
    }
    fn get_controls(&mut self, r: &mut TickRider) -> [u32; 4] {
        self.call(r, CONTROLS, self.rider + ADJUST, vec![]);
        self.pad
    }
    fn update_control(&mut self, r: &mut TickRider, pad: &[u32; 4]) {
        self.call(r, 0x11c9a0, HANDLER, pad.to_vec());
    }
    fn request_reset(&mut self, r: &mut TickRider, height: f32) {
        self.call(r, 0x118f10, self.rider, vec![height.to_bits()]);
    }
    fn score_tick(&mut self, r: &mut TickRider) {
        self.call(r, 0x156e80, self.rider + 0x5820, vec![]);
    }
    fn pick_idle_clip(&mut self, r: &mut TickRider) {
        self.call(r, 0x11d4e8, self.rider, vec![]);
    }
    fn rider_collisions(&mut self, r: &mut TickRider) {
        self.call(r, 0x123fc8, self.rider, vec![]);
    }
    fn update_grudge_timeout(&mut self, r: &mut TickRider) {
        self.call(r, 0x11efd8, self.rider, vec![]);
    }
    fn update_lean_targets(&mut self, r: &mut TickRider) {
        self.call(r, 0x129500, self.rider, vec![]);
    }
    fn render_update(&mut self, r: &mut TickRider) {
        self.call(r, 0x1356f8, HANDLER, vec![]);
    }
}

#[test]
fn boarder_tick_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let math_atan = 0x2510f8u32;
    r.allow(math_atan);
    let rider = r.mem.alloc(0x6000);
    let (app, game, sub, table, world, vt) =
        (r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(0x40), r.mem.alloc(20 * 100), r.mem.alloc(0x40), r.mem.alloc(0x100));
    r.mem.write_u32(APP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x24, table);
    r.mem.write_u32(game + 0x1c, sub);
    r.mem.write_u32(WORLD, world);
    r.mem.write_u32(rider, vt);
    r.mem.write_u16(vt + 0x50, ADJUST as u16);
    r.mem.write_u32(vt + 0x54, CONTROLS);
    r.mem.write_u32(rider + 0x5ae0, HANDLER);

    // the state each case shares with the stubs: the call log, each call's field changes, pause and pad word
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let patches: Rc<RefCell<std::collections::HashMap<u32, Patch>>> = Rc::default();
    let paused = Rc::new(RefCell::new(false));
    let pad = Rc::new(RefCell::new([0u32; 4]));
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..((0x117d1c - 0x117198) / 4) {
        let w = r.mem.read_u32(0x117198 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    jals.remove(&math_atan);
    jals.insert(CONTROLS);
    for a in jals {
        let (log, patches, paused, pad) = (log.clone(), patches.clone(), paused.clone(), pad.clone());
        r.stub(a, move |c, m| {
            let a0 = c.gpr(4) as u32;
            let mut extra = Vec::new();
            match a {
                0x17f078 => {
                    log.borrow_mut().push(Call { addr: a, a0: 0, extra });
                    c.set_gpr(2, *paused.borrow() as u64);
                    return;
                }
                0x155dc8 | 0x118f10 => extra.push(c.f(12).to_bits()),
                CONTROLS => {
                    let a1 = c.gpr(5) as u32;
                    for (k, w) in pad.borrow().iter().enumerate() {
                        m.write_u32(a1 + 4 * k as u32, *w);
                    }
                }
                0x11c9a0 => {
                    let a1 = c.gpr(5) as u32;
                    extra = (0..4).map(|k| m.read_u32(a1 + 4 * k)).collect();
                }
                _ => {}
            }
            log.borrow_mut().push(Call { addr: a, a0, extra });
            if let Some(p) = patches.borrow().get(&a) {
                for (o, w) in p {
                    m.write_u32(rider + o, *w);
                }
            }
            c.set_gpr(2, 0);
        });
    }

    let mut g = Rng(0x0011_7198_a1b2_c3d4);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..60_000u32 {
        let before = random_rider(&mut g);
        let tg = TickGame {
            mode: pick!(g, [0, 1, 2, 3, 4, 5, 6]),
            density: g.f(0.0, 1.0),
            world_floor: g.f(-2000.0, 2000.0),
            row_1c: g.f(-10.0, 10.0),
            row_20: g.f(-10.0, 10.0),
            row_28: g.f(-5.0, 15.0),
        };
        let is_paused = g.chance(10);
        let pad_word = [g.next(), g.next(), g.next(), g.next()];
        let mut ps = std::collections::HashMap::new();
        for a in [0x1166f0u32, 0x11c770, 0x155dc8, 0x118298, 0x118090, CONTROLS, 0x11c9a0, 0x118f10, 0x156e80, 0x11d4e8, 0x123fc8, 0x11efd8, 0x129500, 0x1356f8] {
            // what the calls after the tick's last read change is not read again: only the earlier ones matter
            if g.chance(3) {
                ps.insert(a, random_patch(&mut g));
            }
        }

        // the game's
        for (o, w) in words(&before) {
            r.mem.write_u32(rider + o, w);
        }
        let row = table + 100 * before.surface;
        r.mem.write_f32(row + 0x1c, tg.row_1c);
        r.mem.write_f32(row + 0x20, tg.row_20);
        r.mem.write_f32(row + 0x28, tg.row_28);
        r.mem.write_u32(sub + 0x1c, tg.mode as u32);
        r.mem.write_f32(game + 0x78, tg.density);
        r.mem.write_f32(world + 0xc, tg.world_floor);
        *patches.borrow_mut() = ps.clone();
        *paused.borrow_mut() = is_paused;
        *pad.borrow_mut() = pad_word;
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x117198).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs: Vec<(u32, u32)> = words(&before).iter().map(|(o, _)| (*o, r.mem.read_u32(rider + o))).collect();

        // ours
        let mut me = before;
        let mut env = Ours { paused: is_paused, pad: pad_word, patches: &ps, rider, log: Vec::new() };
        boarder_tick(&mut env, &tg, &mut me);
        let ours = words(&me);

        let diff: Vec<String> = theirs
            .iter()
            .zip(&ours)
            .filter(|(a, b)| a.1 != b.1)
            .map(|(a, b)| format!("+{:#x}: game {:#x} ({}), ours {:#x} ({})", a.0, a.1, f32::from_bits(a.1), b.1, f32::from_bits(b.1)))
            .collect();
        assert!(diff.is_empty(), "case {case}: {before:?} {tg:?} patches {ps:x?}\n{}", diff.join("\n"));
        assert_eq!(*log.borrow(), env.log, "case {case}: the calls; {before:?} {tg:?} patches {ps:x?}");

        let names: &[(&str, u32)] = &[("reset", 0x118f10), ("race line", 0x118298), ("collisions", 0x123fc8), ("idle clip", 0x11d4e8), ("render", 0x1356f8)];
        for (n, a) in names {
            if env.log.iter().any(|c| c.addr == *a) {
                *seen.entry(n).or_default() += 1;
            }
        }
        let capped = before.motion != 1 && env.log.iter().any(|c| c.addr == 0x155dc8 && c.extra[0] == me.speed_limit.to_bits());
        *seen.entry("capped").or_default() += capped as u32;
    }
    for (n, k) in &seen {
        assert!(*k > 1000, "{n}: only {k} cases");
    }
    eprintln!("{seen:?}");
}

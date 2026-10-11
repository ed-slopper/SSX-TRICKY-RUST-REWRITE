//! The board's attitude at a take-off, kept through the air (board row G20c3c): the real `GroundMotion_Update`
//! 0x10a0d8 runs in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) on the same
//! game-side rider the port's ground tick is handed (`Rider::ground_input`), with the ground missing under it so the
//! tick ends in `Boarder_TakeOff` 0x128470. The quaternion rider+0x170, the axes +0x180 forward, +0x190 side, +0x1a0 up
//! and the heading +0x1b0 it leaves are the original's attitude at the take-off. Then the real `AirMotion_Update`
//! 0x108378 runs on that rider for a number of air ticks with nothing hit (no wall bump): it leaves the attitude as it
//! was (the spin and flip are the separate angles +0x1d0 and +0x1d4). The port must hand the landing's first ground tick
//! (`Rider::seed_ground`) exactly that attitude, bit for bit, for a jump without spin or flip: `Rider::take_off` keeps
//! the ground tick's attitude in `Rider::att`, the port's air steps leave it alone.
//!
//! A spin or flip in the air is not kept (`SpinState_Update`'s landing alignment is the original's turn, rows G20c3c2 and
//! G1a5b3b), and the wall bump of `AirMotion_Update` (the port's air walls are not the game's, row G20c3c3) turns it.

use std::cell::Cell;
use std::rc::Rc;

use glam::Vec3;
use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::collide::CollisionWorld;
use tricky_game::ground_motion::{Rider as Gm, Stats};
use tricky_game::probe;
use tricky_game::rails::Rails;
use tricky_game::rider::ground_tick::GroundCache;
use tricky_game::rider::{wrap, Input, Rider};
use tricky_game::surface::SURFACE_TABLE;

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
    fn unit4(&mut self) -> V4 {
        loop {
            let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
            if l > 0.1 {
                return [v[0] / l, v[1] / l, v[2] / l, v[3] / l];
            }
        }
    }
    fn chance(&mut self, one_in: u32) -> bool {
        self.next() % one_in == 0
    }
}

fn rv(m: &mut r5900::Mem, a: u32) -> V4 {
    [m.read_f32(a), m.read_f32(a + 4), m.read_f32(a + 8), m.read_f32(a + 12)]
}
fn wv(m: &mut r5900::Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

/// The attitude fields in the layout the rider keeps them (+0x170 quaternion, +0x180 forward, +0x190 side, +0x1a0 up,
/// +0x1b0 heading), as words.
fn attitude_words(q: V4, fwd: V4, side: V4, up: V4, yaw: f32) -> Vec<u32> {
    let mut v: Vec<u32> = [q, fwd, side, up].iter().flatten().map(|x| x.to_bits()).collect();
    v.push(yaw.to_bits());
    v
}

fn seed_words(g: &Gm) -> Vec<u32> {
    attitude_words(g.quat, g.p.fwd, g.board_side, g.board_up, g.yaw)
}

/// The original's `GroundMotion_Update`: one tick of a game-side rider with no ground under it and no wall.
struct Ground {
    r: Runner,
    rd: u32,
    stats: u32,
    motion: u32,
    /// calls of `Boarder_SetState`, `Boarder_TakeOff` since the last tick
    states: Rc<Cell<u32>>,
    take_offs: Rc<Cell<u32>>,
}

fn ground_world() -> Option<Ground> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // run as the original (tests/ground_motion.rs): the forces, the maths, the probe, the orientation routines
    for a in [
        0x102f50, 0x250c10, 0x250fd0, 0x250e98, 0x109878, 0x109950, 0x109cb8, 0x109ef8, 0x15fdd0, 0x128ae8, 0x128a18, 0x128e90, 0x129008,
        0x102f80, 0x2510f8, 0x11a798, 0x136e70, 0x135258,
    ] {
        r.allow(a);
    }
    let stats = r.mem.alloc(0x100);
    let rd = r.mem.alloc(0x6000);
    let motion = r.mem.alloc(0x40);
    let vtable = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    let app_730 = r.mem.alloc(0x40);
    let table = r.mem.alloc(20 * 100);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(motion + 0xc, rd);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, app_730);
    r.mem.write_u32(app_730 + 0x24, table);
    for (i, row) in SURFACE_TABLE.iter().enumerate() {
        for (j, w) in row.iter().enumerate() {
            r.mem.write_f32(table + (i * 100 + j * 4) as u32, *w);
        }
    }
    r.mem.write_u32(rd, vtable);
    r.mem.write_u16(vtable + 0x60, 0);
    r.mem.write_u32(vtable + 0x64, 0x11a798);
    let handler = r.mem.alloc(0x400);
    let render = r.mem.alloc(0x9a00);
    r.mem.write_u32(rd + 0x5ae0, handler);
    r.mem.write_u32(handler, rd);
    r.mem.write_u32(handler + 0x3e0, render);
    r.call(0x10af18).expect("StaticInit_GroundMotion");
    let states = Rc::new(Cell::new(0));
    let take_offs = Rc::new(Cell::new(0));
    let c = states.clone();
    r.stub(0x11f608, move |_, _| c.set(c.get() + 1)); // Boarder_SetState
    r.stub(0x126250, |cpu, _| cpu.set_gpr(2, 0)); // Boarder_WallCollide: no wall
    let c = take_offs.clone();
    r.stub(0x128470, move |_, _| c.set(c.get() + 1)); // Boarder_TakeOff (checked in tests/orient.rs)
    r.stub(0x12e008, |_, _| {}); // Fx_BrakeSpray_Emit
    r.stub(0x11d830, |_, _| {}); // Boarder_Wipeout
    r.stub(0x156de0, |cpu, _| cpu.set_f(0, 0.0)); // Score_Crash
    r.stub(0x11b018, |_, _| {}); // Boarder_AddMeter
    r.stub(0x125cb8, |_, _| {}); // Rail_TryCapture
    r.stub(0x125088, |_, _| {}); // Boarder_InstanceCollisions
    r.stub(0x25d7f0, |_, _| {}); // cWorldLine
    r.stub(0x25aff8, |cpu, _| cpu.set_f(0, -1.0)); // World_RayCast: nothing under the board
    Some(Ground { r, rd, stats, motion, states, take_offs })
}

impl Ground {
    fn put(&mut self, p: &Gm) {
        let (m, rd, st) = (&mut self.r.mem, self.rd, self.stats);
        for (o, v) in [
            (0x140, p.p.pos),
            (0x150, p.vel),
            (0x170, p.quat),
            (0x180, p.p.fwd),
            (0x190, p.board_side),
            (0x1a0, p.board_up),
            (0x2a0, p.p.normal),
            (0x2b0, p.p.fall),
            (0x2c0, p.up),
            (0x2d0, p.p.ground),
            (0x320, p.p.plane_fwd),
            (0x330, p.p.side),
        ] {
            wv(m, rd + o, v);
        }
        for (o, v) in [
            (0x12c, p.timescale),
            (0x130, p.boost_level),
            (0x134, p.speed_timer),
            (0x160, p.drag),
            (0x1b0, p.yaw),
            (0x1bc, p.p.height),
            (0x1c0, p.bob),
            (0x1cc, p.wobble),
            (0x1d8, p.lean),
            (0x1fc, p.brake),
            (0x208, p.crouch),
            (0x214, p.steer),
            (0x244, p.p.x244),
            (0x294, p.d1),
            (0x298, p.d2),
            (0x370, p.course_yaw),
        ] {
            m.write_f32(rd + o, v);
        }
        for (o, v) in [
            (0x1b4, p.switch),
            (0x290, p.p.surface),
            (0x29c, p.p.surface_ticks),
            (0x2e0, p.p.kind),
            (0x420, p.class),
            (0x428, p.state),
            (0x494, p.mask),
        ] {
            m.write_u32(rd + o, v);
        }
        m.write_u32(rd + 0x46c0 + 8, if p.skating { 0x221 } else { 0x200 });
        for (o, v) in [(0x0e, p.stats.speed), (0x11, p.stats.edging), (0x13, p.stats.grip), (0x19, p.stats.cubic)] {
            m.write_u8(st + o, v as u8);
        }
        m.write_u32(st + 0x40, p.stats.stance);
        m.write_f32(st + 0x60, p.p.stat60);
        m.write_f32(self.motion + 8, p.phase);
    }

    /// One tick of the original on `p`: the attitude it leaves, and the rider's position and velocity.
    fn tick(&mut self, p: &Gm) -> Result<(Vec<u32>, V4, V4), String> {
        self.states.set(0);
        self.take_offs.set(0);
        self.put(p);
        self.r.cpu.set_gpr(4, self.motion as u64);
        self.r.call(0x10a0d8).map_err(|e| format!("the original: {e}"))?;
        if self.states.get() != 0 {
            return Err("state".into());
        }
        assert_eq!(self.take_offs.get(), 1, "no ground under the board: the tick ends in Boarder_TakeOff");
        let (m, rd) = (&mut self.r.mem, self.rd);
        let att = attitude_words(rv(m, rd + 0x170), rv(m, rd + 0x180), rv(m, rd + 0x190), rv(m, rd + 0x1a0), m.read_f32(rd + 0x1b0));
        Ok((att, rv(m, rd + 0x140), rv(m, rd + 0x150)))
    }
}

/// The original's `AirMotion_Update`, with nothing hit by any ray: a free flight.
struct Air {
    r: Runner,
    rider: u32,
}

fn air_world() -> Option<Air> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x12b340u32, 0x102e80, 0x250c10, 0x250fd0, 0x129008, 0x2510f8, 0x102f50, 0x102f80] {
        r.allow(a);
    }
    let rider = r.mem.alloc(0x6000);
    let vt = r.mem.alloc(0x200);
    r.mem.write_u32(rider, vt);
    for a in [0x123ec8u32, 0x25d7f0, 0x257810, 0x125cb8, 0x125088, 0x257ca0] {
        r.stub(a, |_, _| {});
    }
    r.stub(0x25aff8, |cpu, _| cpu.set_f(0, -1.0)); // the down ray and the wall ray: nothing hit
    Some(Air { r, rider })
}

impl Air {
    /// Puts the attitude and a flight state in, runs `ticks` ticks of `AirMotion_Update`, returns the attitude after each.
    fn fly(&mut self, att: &Vec<u32>, pos: V4, vel: V4, ticks: usize) -> Vec<Vec<u32>> {
        let (m, rd) = (&mut self.r.mem, self.rider);
        let f = |w: &[u32]| -> V4 { [f32::from_bits(w[0]), f32::from_bits(w[1]), f32::from_bits(w[2]), f32::from_bits(w[3])] };
        wv(m, rd + 0x170, f(&att[0..4]));
        wv(m, rd + 0x180, f(&att[4..8]));
        wv(m, rd + 0x190, f(&att[8..12]));
        wv(m, rd + 0x1a0, f(&att[12..16]));
        m.write_u32(rd + 0x1b0, att[16]);
        wv(m, rd + 0x140, pos);
        wv(m, rd + 0x150, vel);
        m.write_f32(rd + 0x12c, 1.0);
        m.write_f32(rd + 0x1d0, 0.0); // no spin, no flip
        m.write_f32(rd + 0x1d4, 0.0);
        m.write_u32(rd + 0x57f0, u32::MAX); // the flight prediction names no surface below
        for (k, row) in [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]].iter().enumerate() {
            wv(m, rd + 0x4aa0 + 0x10 * k as u32, *row);
        }
        m.write_u32(rd + 0x44d8, 0);
        for k in 0..3 {
            m.write_f32(rd + 0x8c8 + 4 * k, 0.0);
        }
        m.write_u32(rd + 0x494, 0x77);
        let mut out = Vec::new();
        for _ in 0..ticks {
            self.r.mem.write_u32(rd + 0x480 + 0x20, rd);
            self.r.cpu.set_gpr(4, (rd + 0x480) as u64);
            self.r.call(0x108378).expect("AirMotion_Update runs to its return");
            let m = &mut self.r.mem;
            out.push(attitude_words(rv(m, rd + 0x170), rv(m, rd + 0x180), rv(m, rd + 0x190), rv(m, rd + 0x1a0), m.read_f32(rd + 0x1b0)));
        }
        out
    }
}

/// A game-side rider that is about to leave the snow: random, on a plain riding state, high above the world.
fn gen(g: &mut Rng) -> Gm {
    let normal = g.unit(-0.2);
    let pos = [g.f(-50000.0, 50000.0), g.f(-50000.0, 50000.0), g.f(3000.0, 9000.0), 1.0];
    let speed = if g.chance(10) { g.f(0.5, 8.0) } else { g.f(0.5, 3400.0) };
    let v = g.unit(-1.0);
    let vel = [v[0] * speed, v[1] * speed, v[2] * speed, 0.0];
    let d1 = g.f(0.3, 5.0);
    let d2 = d1 + g.f(0.5, 35.0);
    Gm {
        p: probe::Rider {
            pos,
            fwd: g.unit(-1.0),
            normal,
            ground: [g.f(-100.0, 100.0), g.f(-100.0, 100.0), g.f(-100.0, 100.0), 0.0],
            plane_fwd: g.unit(-1.0),
            side: g.unit(-1.0),
            fall: g.unit(-1.0),
            x244: g.f(-0.1, 0.1),
            stat60: g.f(0.0, 20.0),
            height: g.f(-45.0, 10.0),
            surface: g.next() % 20,
            surface_ticks: [0, 5, 0x8000_0001][(g.next() % 3) as usize],
            kind: 0,
        },
        vel,
        up: g.unit(0.3),
        board_up: g.unit(-1.0),
        quat: g.unit4(),
        board_side: g.unit(-1.0),
        timescale: 1.0,
        boost_level: 0.0,
        speed_timer: 0.0,
        drag: g.f(-5.0, 5.0),
        yaw: g.f(-3.0, 3.0),
        course_yaw: g.f(-3.0, 3.0),
        switch: g.next() % 2,
        bob: g.f(-25.0, 25.0),
        wobble: g.f(-5.0, 5.0),
        lean: g.f(-1.0, 1.0),
        brake: if g.chance(2) { 0.0 } else { g.f(0.0, 1.0) },
        crouch: if g.chance(5) { 0.0 } else { g.f(0.0, 1.0) },
        steer: if g.chance(8) { 0.0 } else { g.f(-1.0, 1.0) },
        d1,
        d2,
        class: g.next() % 3,
        state: 3,
        mask: 0,
        stats: Stats { speed: (g.next() % 256) as f32, edging: (g.next() % 256) as f32, grip: (g.next() % 256) as f32, cubic: (g.next() % 256) as f32, stance: g.next() % 2 },
        skating: false,
        phase: g.f(0.0, 6.2831855),
    }
}

/// A port rider on the snow whose last ground tick left the game-side rider `x`.
fn riding(x: Gm) -> Rider {
    let mut r = Rider::new(Vec3::ZERO, 0.0);
    r.grounded = true;
    r.pos = Vec3::new(x.p.pos[0], x.p.pos[2], -x.p.pos[1]) * 0.01;
    r.vel = Vec3::new(x.vel[0], x.vel[2], -x.vel[1]) * 0.01;
    r.yaw = wrap(x.yaw - std::f32::consts::FRAC_PI_2);
    r.gm = Some(GroundCache::new(x, r.pos, r.vel, r.yaw));
    r.att = None;
    r
}

const DT: f32 = 1.0 / 60.0;

/// The take-off tick's attitude and the landing's seed are the original's, for jumps without spin or flip.
#[test]
fn the_take_off_attitude_is_kept_through_the_air() {
    let (Some(mut ground), Some(mut air)) = (ground_world(), air_world()) else { return };
    let world = CollisionWorld::default();
    let rails = Rails::default();
    let mut g = Rng(0x6_20c3_c000_0001);
    let (mut cases, mut skipped, mut turned) = (0u32, 0u32, 0u32);
    while cases < 1500 {
        let x = gen(&mut g);
        let mut r = riding(x);
        // the game-side rider the port's ground tick is handed, and the original's tick on it
        let g_in = r.clone().ground_input(DT, 0.0);
        let Ok((game, pos, vel)) = ground.tick(&g_in) else {
            skipped += 1;
            continue;
        };
        // the port's tick: off the snow, `Boarder_TakeOff` ran, the port's take-off keeps the attitude
        assert!(r.ground_tick(&world, DT, 0.0), "case {cases}: the tick left the snow");
        r.take_off();
        let kept = r.att.as_ref().map(|(o, _)| attitude_words(o.q, o.fwd, o.side, o.up, o.yaw));
        assert_eq!(kept.as_ref(), Some(&game), "case {cases}: the attitude the take-off tick left");
        if game[..4] != seed_words(&g_in)[..4] {
            turned += 1;
        }
        // the original's air ticks leave it alone; so do the port's: the landing's first tick starts from it
        let ticks = 1 + (g.next() % 24) as usize;
        for a in air.fly(&game, pos, vel, ticks) {
            assert_eq!(a, game, "case {cases}: AirMotion_Update leaves the attitude (no wall)");
        }
        for _ in 0..ticks {
            r.step(&world, &rails, Input::default(), DT);
            assert!(!r.grounded && r.att.is_some(), "case {cases}: in the air, the attitude stays");
        }
        assert_eq!(seed_words(&r.seed_ground()), game, "case {cases}: the attitude the landing's ground tick starts from");
        cases += 1;
    }
    eprintln!("take-off attitude: {cases} cases, {skipped} skipped (a state change), the tick turned the board in {turned}");
    assert!(turned > 1000, "the tick's own turns are in what is kept ({turned})");
}

/// The comparison can tell: the rebuilt attitude (what the port made of the yaw and the normal before this row) is not the
/// original's, and a spin in the air still drops the kept one (its landing is `SpinState_Update`'s, rows G20c3c2, G1a5b3b).
#[test]
fn the_rebuild_was_not_the_originals_and_a_spin_is_not_kept() {
    let (Some(mut ground), Some(_)) = (ground_world(), air_world()) else { return };
    let world = CollisionWorld::default();
    let rails = Rails::default();
    let mut g = Rng(0x6_20c3_c000_0002);
    let (mut differ, mut cases) = (0u32, 0u32);
    while cases < 200 {
        let x = gen(&mut g);
        let mut r = riding(x);
        let g_in = r.clone().ground_input(DT, 0.0);
        let Ok((game, ..)) = ground.tick(&g_in) else { continue };
        assert!(r.ground_tick(&world, DT, 0.0));
        r.take_off();
        // the rebuild: no kept attitude
        let mut rebuilt = r.clone();
        rebuilt.att = None;
        if seed_words(&rebuilt.seed_ground()) != game {
            differ += 1;
        }
        // a spin in the air: not kept
        let mut spun = r.clone();
        spun.spin_ax = tricky_game::rider::Axis::start(6.0);
        for _ in 0..10 {
            spun.step(&world, &rails, Input { steer: 1.0, ..Input::default() }, DT);
        }
        assert!(spun.spin != 0.0 && spun.att.is_none(), "case {cases}: a spin drops the kept attitude");
        cases += 1;
    }
    assert!(differ > 150, "the rebuilt attitude is not the original's ({differ} of {cases})");
}

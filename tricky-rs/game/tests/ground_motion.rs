//! Our ground motion against the original's, bit for bit (board row E8h): the game's own `GroundMotion_Update`
//! 0x10a0d8 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), with its
//! callees run as the original wherever they are small and already checked (the four forces 0x109878, 0x109950,
//! 0x109cb8, 0x109ef8, `Math_SinCos`, `Math_Asin`, `Math_Cos`, `Vec4_Scale`, `Boarder_ProbeGround` 0x128ae8 and
//! `Boarder_GetFallLine` 0x128a18, with the world's ray cast answered by the test as in `tests/probe.rs`; since
//! E8j also `Boarder_RotateAboutAxis` 0x128e90 with `Boarder_UpdateOrientation` 0x129008, `Vec3_Scale` and
//! `Math_Atan`, and the rider's crash virtual, the real chain from the vtable: `cBoarder::vf11` 0x11a798 →
//! 0x136e70 → 0x135258, down to `Fx_BrakeSpray_Emit` 0x12e008, which is recorded) and the other callees recorded
//! (state change, wall collision, take-off, the crash calls, rail capture, instance collisions), and
//! `tricky_game::ground_motion::update` on the same rider: every field the function writes and every recorded
//! call, with its arguments, in order, over many random ticks.
//!
//! `planted_bugs_are_caught` shows the comparison can tell: a rider that differs from the game's by one bit in one
//! field must be caught.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::ground_motion::{update, CrashRecord, Env, Rider, Stats, WALL_MASK};
use tricky_game::air::{predict_at_takeoff, Predict, RayHit};
use tricky_game::jump::{jump_impulse, JumpState};
use tricky_game::orient::{take_off, TakeOffEnv, TakeOffRider};
use tricky_game::probe::{self, Hit};
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
    fn chance(&mut self, one_in: u32) -> bool {
        self.next() % one_in == 0
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
}

/// A call the function makes to something that is not run here, with what it passes (bits, so NaN-proof).
#[derive(Clone, Debug, PartialEq)]
enum Ev {
    State(u32),
    Wall(u32),
    Line(u32, [u32; 4], [u32; 4], u64),
    TakeOff,
    Virtual([u32; 4], [u32; 4], [u32; 4]),
    Wipeout([u32; 4], [u32; 4], [u32; 4], u32, [u32; 4]),
    Score(u32, u64),
    Meter(u32),
    Rail,
    Instances,
}

fn b4(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn rv(m: &mut r5900::Mem, a: u32) -> V4 {
    [m.read_f32(a), m.read_f32(a + 4), m.read_f32(a + 8), m.read_f32(a + 12)]
}
fn wv(m: &mut r5900::Mem, a: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

/// The port's callees: record each call as the stubs do and answer as the test says.
struct E {
    log: Vec<Ev>,
    hit: Option<Hit>,
    wall: bool,
    meter: f32,
}
impl Env for E {
    fn set_state(&mut self, _: &mut Rider, state: u32) {
        self.log.push(Ev::State(state));
    }
    fn wall_collide(&mut self, r: &mut Rider) -> bool {
        self.log.push(Ev::Wall(r.mask));
        self.wall
    }
    fn ray_cast(&mut self, _: &mut Rider, from: V4, to: V4) -> Option<Hit> {
        self.log.push(Ev::Line(0.5f32.to_bits(), b4(from), b4(to), 2));
        self.hit
    }
    fn take_off(&mut self, _: &mut Rider) {
        self.log.push(Ev::TakeOff);
    }
    fn crash_virtual(&mut self, _: &mut Rider, pos: V4, vel: V4, normal: V4) {
        self.log.push(Ev::Virtual(b4(pos), b4(vel), b4(normal)));
    }
    fn wipeout(&mut self, _: &mut Rider, c: &CrashRecord) {
        self.log.push(Ev::Wipeout(b4(c.pos), b4(c.dir), b4(c.normal), c.along.to_bits(), b4(c.dir)));
    }
    fn score_crash(&mut self, _: &mut Rider) -> f32 {
        self.log.push(Ev::Score(0x5820, 0));
        self.meter
    }
    fn add_meter(&mut self, _: &mut Rider, amount: f32) {
        self.log.push(Ev::Meter(amount.to_bits()));
    }
    fn rail_try_capture(&mut self, _: &mut Rider) {
        self.log.push(Ev::Rail);
    }
    fn instance_collisions(&mut self, _: &mut Rider) {
        self.log.push(Ev::Instances);
    }
}

struct W {
    r: Runner,
    rd: u32,
    stats: u32,
    motion: u32,
    log: Rc<RefCell<Vec<Ev>>>,
    hit: Rc<RefCell<Option<Hit>>>,
    wall: Rc<Cell<bool>>,
    meter: Rc<Cell<f32>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // what runs as the original (E8j: Boarder_RotateAboutAxis 0x128e90, Boarder_UpdateOrientation 0x129008,
    // Vec3_Scale 0x102f80, Math_Atan 0x2510f8, and the crash virtual cBoarder::vf11 0x11a798 down to the stub)
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
    // the surface table where the game keeps it: gApp 0x337c58 → +0x730 → +0x24
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, app_730);
    r.mem.write_u32(app_730 + 0x24, table);
    for (i, row) in SURFACE_TABLE.iter().enumerate() {
        for (j, w) in row.iter().enumerate() {
            r.mem.write_f32(table + (i * 100 + j * 4) as u32, *w);
        }
    }
    // the rider's vtable (cBoarder's, 0x367640): the virtual at +0x64, `cBoarder::vf11` 0x11a798, called with `this`
    // moved by the short at +0x60, which is 0 in the game's vtable
    r.mem.write_u32(rd, vtable);
    r.mem.write_u16(vtable + 0x60, 0);
    r.mem.write_u32(vtable + 0x64, 0x11a798);
    // what vf11 reaches: the object at rider+0x5ae0 (its word 0 is the rider, +0x3e0 the render object whose +0x99b0
    // is the brake spray's state)
    let handler = r.mem.alloc(0x400);
    let render = r.mem.alloc(0x9a00);
    r.mem.write_u32(rd + 0x5ae0, handler);
    r.mem.write_u32(handler, rd);
    r.mem.write_u32(handler + 0x3e0, render);
    // the game's start-up code for the ground motion sets the load factor at 0x31d308 (30 / 30)
    r.call(0x10af18).expect("StaticInit_GroundMotion");

    let log = Rc::new(RefCell::new(Vec::new()));
    let hit = Rc::new(RefCell::new(None::<Hit>));
    let wall = Rc::new(Cell::new(false));
    let meter = Rc::new(Cell::new(0.0f32));
    let l = log.clone();
    // Boarder_SetState 0x11f608
    r.stub(0x11f608, move |cpu, _| l.borrow_mut().push(Ev::State(cpu.gpr(5) as u32)));
    let (l, w) = (log.clone(), wall.clone());
    // Boarder_WallCollide 0x126250
    r.stub(0x126250, move |cpu, mem| {
        l.borrow_mut().push(Ev::Wall(mem.read_u32(rd + 0x494)));
        cpu.set_gpr(2, w.get() as u64);
    });
    let l = log.clone();
    // Boarder_TakeOff 0x128470 (ported and checked in tests/orient.rs; its callees are the port's to provide)
    r.stub(0x128470, move |_, _| l.borrow_mut().push(Ev::TakeOff));
    let l = log.clone();
    // Fx_BrakeSpray_Emit 0x12e008, where the crash virtual ends: (the brake spray's state, the rider, position,
    // velocity, normal); the rider's own position, velocity and normal are what the virtual passes on
    r.stub(0x12e008, move |cpu, mem| {
        assert_eq!(cpu.gpr(4) as u32, render + 0x99b0, "the brake spray's state");
        assert_eq!(cpu.gpr(5) as u32, rd, "the rider");
        let (pos, vel, normal) = (rv(mem, cpu.gpr(6) as u32), rv(mem, cpu.gpr(7) as u32), rv(mem, cpu.gpr(8) as u32));
        l.borrow_mut().push(Ev::Virtual(b4(pos), b4(vel), b4(normal)));
    });
    let l = log.clone();
    // Boarder_Wipeout 0x11d830
    r.stub(0x11d830, move |cpu, mem| {
        let p = cpu.gpr(5) as u32;
        let ev = Ev::Wipeout(b4(rv(mem, p)), b4(rv(mem, p + 0x10)), b4(rv(mem, p + 0x20)), mem.read_u32(p + 0x30), b4(rv(mem, p + 0x40)));
        l.borrow_mut().push(ev);
    });
    let (l, m) = (log.clone(), meter.clone());
    // Score_Crash 0x156de0, returning a float; Boarder_AddMeter 0x11b018 gets it
    r.stub(0x156de0, move |cpu, _| {
        l.borrow_mut().push(Ev::Score((cpu.gpr(4) as u32).wrapping_sub(rd), cpu.gpr(5)));
        cpu.set_f(0, m.get());
    });
    let l = log.clone();
    r.stub(0x11b018, move |cpu, _| l.borrow_mut().push(Ev::Meter(cpu.f(12).to_bits())));
    let l = log.clone();
    // Rail_TryCapture 0x125cb8 (G1b)
    r.stub(0x125cb8, move |_, _| l.borrow_mut().push(Ev::Rail));
    let l = log.clone();
    // Boarder_InstanceCollisions 0x125088 (E8b)
    r.stub(0x125088, move |_, _| l.borrow_mut().push(Ev::Instances));
    // the world under the probe, as in tests/probe.rs
    let l = log.clone();
    r.stub(0x25d7f0, move |cpu, mem| {
        let (from, to) = (rv(mem, cpu.gpr(5) as u32), rv(mem, cpu.gpr(6) as u32));
        l.borrow_mut().push(Ev::Line(cpu.f(12).to_bits(), b4(from), b4(to), cpu.gpr(7)));
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
    Some(W { r, rd, stats, motion, log, hit, wall, meter })
}

impl W {
    fn put(&mut self, p: &Rider) {
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

    /// The rider as the original left it (what it never writes is taken from `like`).
    fn get(&mut self, like: &Rider) -> Rider {
        let (m, rd) = (&mut self.r.mem, self.rd);
        let mut r = *like;
        r.p.pos = rv(m, rd + 0x140);
        r.vel = rv(m, rd + 0x150);
        r.quat = rv(m, rd + 0x170);
        r.p.fwd = rv(m, rd + 0x180);
        r.board_side = rv(m, rd + 0x190);
        r.board_up = rv(m, rd + 0x1a0);
        r.yaw = m.read_f32(rd + 0x1b0);
        r.p.normal = rv(m, rd + 0x2a0);
        r.p.fall = rv(m, rd + 0x2b0);
        r.up = rv(m, rd + 0x2c0);
        r.p.ground = rv(m, rd + 0x2d0);
        r.p.plane_fwd = rv(m, rd + 0x320);
        r.p.side = rv(m, rd + 0x330);
        r.drag = m.read_f32(rd + 0x160);
        r.p.height = m.read_f32(rd + 0x1bc);
        r.bob = m.read_f32(rd + 0x1c0);
        r.wobble = m.read_f32(rd + 0x1cc);
        r.lean = m.read_f32(rd + 0x1d8);
        r.p.surface = m.read_u32(rd + 0x290);
        r.p.surface_ticks = m.read_u32(rd + 0x29c);
        r.p.kind = m.read_u32(rd + 0x2e0);
        r.state = m.read_u32(rd + 0x428);
        r.mask = m.read_u32(rd + 0x494);
        r.phase = m.read_f32(self.motion + 8);
        r
    }
}

/// Every field of the rider the function can touch, as bits.
fn bits(r: &Rider) -> Vec<u32> {
    [r.p.pos, r.vel, r.p.fwd, r.board_up, r.p.normal, r.p.fall, r.up, r.p.ground, r.p.plane_fwd, r.p.side, r.quat, r.board_side]
        .iter()
        .flatten()
        .chain([r.drag, r.p.height, r.bob, r.wobble, r.lean, r.phase, r.yaw].iter())
        .map(|x| x.to_bits())
        .chain([r.p.surface, r.p.surface_ticks, r.p.kind, r.state, r.mask])
        .collect()
}

struct Case {
    rider: Rider,
    hit: Option<Hit>,
    wall: bool,
    meter: f32,
}

fn sub4(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}
fn mul4(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

fn gen(g: &mut Rng) -> Case {
    let normal = g.unit(-0.2);
    let pos = [g.f(-50000.0, 50000.0), g.f(-50000.0, 50000.0), g.f(-20000.0, 20000.0), 1.0];
    let speed = if g.chance(10) { g.f(0.5, 8.0) } else { g.f(0.5, 3400.0) };
    let vel = mul4(g.unit(-1.0), speed);
    let d1 = g.f(0.3, 5.0);
    let d2 = d1 + g.f(0.5, 35.0);
    let height = if g.chance(3) { g.f(-3.0, 3.0) } else { g.f(-45.0, 10.0) };
    let steer = match g.next() % 8 {
        0 => 0.0,
        1 => [1.0, -1.0][(g.next() % 2) as usize],
        _ => g.f(-1.0, 1.0),
    };
    let mut rider = Rider {
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
            height,
            surface: g.next() % 20,
            surface_ticks: [0, 5, 0x8000_0001][(g.next() % 3) as usize],
            kind: g.next() % 4,
        },
        vel: [vel[0], vel[1], vel[2], 0.0],
        up: g.unit(0.3),
        board_up: g.unit(-1.0),
        quat: g.unit4(),
        board_side: g.unit(-1.0),
        timescale: if g.chance(3) { g.f(0.2, 3.0) } else { 1.0 },
        boost_level: [0.0, 0.0, 0.25, 0.6013, 1.0][(g.next() % 5) as usize],
        speed_timer: if g.chance(4) { g.f(0.0, 3.0) } else { 0.0 },
        drag: g.f(-5.0, 5.0),
        yaw: g.f(-12.0, 12.0),
        course_yaw: g.f(-12.0, 12.0),
        switch: g.next() % 2,
        bob: g.f(-25.0, 25.0),
        wobble: g.f(-5.0, 5.0),
        lean: g.f(-1.0, 1.0),
        brake: if g.chance(2) { 0.0 } else { g.f(0.0, 1.0) },
        crouch: if g.chance(5) { 0.0 } else { g.f(0.0, 1.0) },
        steer,
        d1,
        d2,
        class: g.next() % 3,
        state: [3, 5, 0xc, 0xe, 0xe, 4, 1, 0xb, 0x10, 0xd][(g.next() % 10) as usize],
        mask: 0,
        stats: Stats {
            speed: (g.next() % 256) as f32,
            edging: (g.next() % 256) as f32,
            grip: (g.next() % 256) as f32,
            cubic: (g.next() % 256) as f32,
            stance: g.next() % 2,
        },
        skating: g.chance(3),
        phase: g.f(0.0, 6.2831855),
    };
    // exact ties, which random floats never reach: sunk to the spring band's depth exactly, to the 10 cm clamp
    // exactly, and riding backwards at exactly the limit of −111.11111 cm/s (the compares are `<`, not `<=`)
    match g.next() % 40 {
        0 => rider.p.height = -rider.d2,
        1 => {
            rider.d2 = 10.0;
            rider.p.height = -20.0;
        }
        2 => {
            rider.p.plane_fwd = [1.0, 0.0, 0.0, 0.0];
            rider.vel[0] = f32::from_bits(0xc2de_38e3);
            rider.brake = 0.0;
            rider.state = 3;
        }
        _ => {}
    }
    // what the ray cast finds: mostly ground near the board (so both sides of each row's air height are met)
    let hit = match g.next() % 6 {
        0 => None,
        n => {
            let normal = match n {
                1 => [0.0, 0.0, 1.0, 0.0],
                2 => [g.f(-0.0003, 0.0003), g.f(-0.0003, 0.0003), 1.0, 0.0],
                _ => g.unit(0.2),
            };
            let h = if g.chance(8) { g.f(-500.0, 500.0) } else { g.f(-6.0, 4.0) };
            Some(Hit { kind: g.next() % 3, point: sub4(pos, mul4(normal, h)), normal, surface: g.next() % 20 })
        }
    };
    Case { rider, hit, wall: g.chance(4), meter: g.f(-5.0, 5.0) }
}

/// What a tick did, for the coverage counts.
#[derive(Default, Debug)]
struct Seen {
    state: u32,
    wall: u32,
    take_off: u32,
    crash: u32,
    rail: u32,
    rotations: u32,
    pushed_out: u32,
    clamped_out: u32,
    ceiling: u32,
    skipped_yaw: u32,
    powder: u32,
}

/// One tick in the original and in ours (the latter on `ours`, which may have been tampered with): `Err` says
/// which field or call differs.
fn compare(w: &mut W, case: &Case, ours: &Rider) -> Result<Seen, String> {
    *w.hit.borrow_mut() = case.hit;
    w.wall.set(case.wall);
    w.meter.set(case.meter);
    w.log.borrow_mut().clear();
    w.put(&case.rider);
    w.r.cpu.set_gpr(4, w.motion as u64);
    w.r.call(0x10a0d8).map_err(|e| format!("the original: {e}"))?;
    let theirs = w.get(&case.rider);
    let their_log = w.log.borrow().clone();

    let mut mine = *ours;
    let mut env = E { log: Vec::new(), hit: case.hit, wall: case.wall, meter: case.meter };
    update(&mut mine, &SURFACE_TABLE, &mut env);

    if their_log != env.log {
        return Err(format!("calls differ\n game {their_log:?}\n ours {:?}", env.log));
    }
    let (tb, ob) = (bits(&theirs), bits(&mine));
    if tb != ob {
        let names = [
            "pos", "vel", "fwd", "board_up", "normal", "fall", "up", "ground", "plane_fwd", "side", "quat", "board_side", "drag", "height", "bob", "wobble",
            "lean", "phase", "yaw", "surface", "surface_ticks", "kind", "state", "mask",
        ];
        let at = (0..tb.len()).find(|&i| tb[i] != ob[i]).unwrap();
        let name = if at < 48 { format!("{}[{}]", names[at / 4], at % 4) } else { names[at - 48 + 12].to_string() };
        return Err(format!("{name}: game {:#010x} ours {:#010x}\n game {theirs:?}\n ours {mine:?}", tb[at], ob[at]));
    }
    let r = &case.rider;
    let has = |f: fn(&Ev) -> bool| their_log.iter().any(f);
    Ok(Seen {
        state: has(|e| matches!(e, Ev::State(_))) as u32,
        wall: case.wall as u32,
        take_off: has(|e| matches!(e, Ev::TakeOff)) as u32,
        crash: has(|e| matches!(e, Ev::Virtual(..))) as u32,
        rail: has(|e| matches!(e, Ev::Rail)) as u32,
        rotations: (theirs.quat.map(f32::to_bits) != case.rider.quat.map(f32::to_bits)) as u32,
        pushed_out: (r.d2 + r.p.height < 0.0) as u32,
        clamped_out: (r.d2 + r.p.height < -10.0) as u32,
        ceiling: (r.p.normal[2] < 0.0) as u32,
        skipped_yaw: (r.state == 0xe && r.steer == 0.0) as u32,
        powder: (r.p.surface == 3 || r.p.surface == 4) as u32,
    })
}

#[test]
fn ground_motion_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0xe8_0000_0000_0001);
    let mut t = Seen::default();
    for n in 0..20000 {
        let case = gen(&mut g);
        let s = compare(&mut w, &case, &case.rider).unwrap_or_else(|e| panic!("tick {n}: {e}\n rider {:?}\n hit {:?} wall {}", case.rider, case.hit, case.wall));
        t.state += s.state;
        t.wall += s.wall;
        t.take_off += s.take_off;
        t.crash += s.crash;
        t.rail += s.rail;
        t.rotations += s.rotations;
        t.pushed_out += s.pushed_out;
        t.clamped_out += s.clamped_out;
        t.ceiling += s.ceiling;
        t.skipped_yaw += s.skipped_yaw;
        t.powder += s.powder;
    }
    eprintln!("GroundMotion_Update: 20000 ticks, all bit-identical; {t:?}");
    assert!(t.state > 300, "{t:?}");
    assert!(t.wall > 3000, "{t:?}");
    assert!(t.take_off > 3000, "{t:?}");
    assert!(t.crash > 300, "{t:?}");
    assert!(t.rail > 3000, "{t:?}");
    assert!(t.rotations > 15000, "{t:?}");
    assert!(t.pushed_out > 3000 && t.clamped_out > 1500, "{t:?}");
    assert!(t.ceiling > 500 && t.skipped_yaw > 300 && t.powder > 1500, "{t:?}");
    assert_eq!(WALL_MASK, 3);
}

/// A rider that is one bit off the game's in one field must not pass.
#[test]
fn planted_bugs_are_caught() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x0b09_0000_0000_0001);
    let bugs: [(&str, fn(&mut Rider)); 10] = [
        ("crouch", |r| r.crouch = f32::from_bits(r.crouch.to_bits() + 1)),
        ("steer", |r| r.steer = f32::from_bits(r.steer.to_bits() + 1)),
        ("timescale", |r| r.timescale = f32::from_bits(r.timescale.to_bits() + 1)),
        ("d2", |r| r.d2 = f32::from_bits(r.d2.to_bits() + 1)),
        ("vel x", |r| r.vel[0] = f32::from_bits(r.vel[0].to_bits() + 1)),
        ("phase", |r| r.phase = f32::from_bits(r.phase.to_bits() + 1)),
        ("bob", |r| r.bob = f32::from_bits(r.bob.to_bits() + 1)),
        ("height", |r| r.p.height = f32::from_bits(r.p.height.to_bits() + 1)),
        ("quat w", |r| r.quat[0] = f32::from_bits(r.quat[0].to_bits() + 1)),
        ("quat z", |r| r.quat[3] = f32::from_bits(r.quat[3].to_bits() + 1)),
    ];
    let cases: Vec<Case> = (0..400).map(|_| gen(&mut g)).collect();
    for (name, bug) in bugs {
        let caught = cases
            .iter()
            .filter(|c| {
                let mut tampered = c.rider;
                bug(&mut tampered);
                compare(&mut w, c, &tampered).is_err()
            })
            .count();
        eprintln!("a one-bit error in {name}: caught in {caught} of 400 ticks");
        assert!(caught >= 20, "a one-bit error in {name} was caught in only {caught} of 400 ticks");
    }
}

mod recording;
use recording::{f32_at, load, u32_le, vec4};

/// The rider as `GroundMotion_Update` reads it, from a recorded rider block (0x600 bytes from the rider), its stats
/// block, the ground motion object (its phase +8) and the animation's block at rider+0x46c0 (the clip +8).
fn rider_of(a: &[u8], st: &[u8], motion: &[u8], anim: &[u8]) -> Rider {
    Rider {
        p: probe::Rider {
            pos: vec4(a, 0x140),
            fwd: vec4(a, 0x180),
            normal: vec4(a, 0x2a0),
            ground: vec4(a, 0x2d0),
            plane_fwd: vec4(a, 0x320),
            side: vec4(a, 0x330),
            fall: vec4(a, 0x2b0),
            x244: f32_at(a, 0x244),
            stat60: f32_at(st, 0x60),
            height: f32_at(a, 0x1bc),
            surface: u32_le(a, 0x290),
            surface_ticks: u32_le(a, 0x29c),
            kind: u32_le(a, 0x2e0),
        },
        vel: vec4(a, 0x150),
        up: vec4(a, 0x2c0),
        board_up: vec4(a, 0x1a0),
        quat: vec4(a, 0x170),
        board_side: vec4(a, 0x190),
        timescale: f32_at(a, 0x12c),
        boost_level: f32_at(a, 0x130),
        speed_timer: f32_at(a, 0x134),
        drag: f32_at(a, 0x160),
        yaw: f32_at(a, 0x1b0),
        course_yaw: f32_at(a, 0x370),
        switch: u32_le(a, 0x1b4),
        bob: f32_at(a, 0x1c0),
        wobble: f32_at(a, 0x1cc),
        lean: f32_at(a, 0x1d8),
        brake: f32_at(a, 0x1fc),
        crouch: f32_at(a, 0x208),
        steer: f32_at(a, 0x214),
        d1: f32_at(a, 0x294),
        d2: f32_at(a, 0x298),
        class: u32_le(a, 0x420),
        state: u32_le(a, 0x428),
        mask: u32_le(a, 0x494),
        stats: Stats { speed: st[0x0e] as f32, edging: st[0x11] as f32, grip: st[0x13] as f32, cubic: st[0x19] as f32, stance: u32_le(st, 0x40) },
        skating: u32_le(anim, 8) == 0x221,
        phase: f32_at(motion, 8),
    }
}

/// The ground tick over a recorded race (board row F4b3b). For every pair of samples one tick apart in which a rider
/// is on the snow (motion +0x424 = 2) at both: `GroundMotion_Update` 0x10a0d8 on the rider as recorded at the first,
/// against the second, field by field. Run three ways: our `ground_motion::update`, and the game's own code in the
/// function runner (the world of `ground_motion_is_the_originals`) under IEEE adds and under PCSX2's COP1 adder (F4g).
///
/// The world's ray cast under `Boarder_ProbeGround` is answered with what the probe left in the second sample (ground
/// point +0x2d0, normal +0x2a0, surface +0x290, kind +0x2e0; no hit when the kind is 3), so the course is not needed.
/// `cBoarder::vf7` 0x117198 shapes the inputs (brake, crouch, steer, the probe's side offset) and the spring band
/// before the motion, so those are taken from the second sample. After the motion the tick still turns the rider's up
/// (+0x2c0, not compared), caps the speed (`speed_limit_of`, `speed_limit`: applied to each way), pushes riders apart
/// (`Boarder_RiderCollisions` 0x123fc8) and may change the control state.
/// Not in the recording, so not given: the walls (`Boarder_WallCollide` 0x126250 answers no wall) and the instances
/// (`Boarder_InstanceCollisions` 0x125088 does nothing). The ground motion object is the state manager
/// *(rider+0x5ae0) + 0x4b0, rider − 0x400 for most riders (window `-400:10`; the test checks its +0xc is the rider),
/// the clip window `46c0:10`. Needs TRICKY_RECORDING and TRICKY_STATS (lines `rider stats-address hex`).
#[test]
fn ground_ticks_against_the_running_game() {
    let (Ok(paths), Ok(stats)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS")) else { return };
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let path = paths.split(';').next().unwrap();
    let (n, ticks) = load(path);
    let (Some(km), Some(ka)) = (ticks[0].extra.iter().position(|(o, _)| *o == -0x400), ticks[0].extra.iter().position(|(o, _)| *o == 0x46c0)) else {
        eprintln!("{path}: no -400 and 46c0 windows: skipping");
        return;
    };
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners = Vec::new();
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut w) = world() else { return };
        w.r.float_rules = Rules::parse(rules).unwrap();
        runners.push((rules, w));
    }
    let names = [
        "pos", "vel", "fwd", "board_up", "normal", "fall", "up", "ground", "plane_fwd", "side", "quat", "board_side", "drag", "height", "bob", "wobble",
        "lean", "phase", "yaw", "surface", "surface_ticks", "kind", "state", "mask",
    ];
    // per field: the field's bits (vectors as four words)
    let fields = |r: &Rider| -> Vec<Vec<u32>> {
        let b = bits(r);
        (0..names.len()).map(|f| if f < 12 { b[4 * f..4 * f + 4].to_vec() } else { vec![b[48 + f - 12]] }).collect()
    };
    // the fields the motion writes (+0x2c0, the rider's up, it only reads: something after it in the tick turns it)
    let written: Vec<usize> = (0..names.len()).filter(|&f| names[f] != "up").collect();
    let one_step = |a: &[u8], b: &[u8]| {
        let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
        let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
        e > 1.0 && d > 0.7 * e && d < 1.4 * e
    };
    let mut count = vec![vec![0usize; names.len()]; 1 + runners.len()];
    let (mut skew, mut capped) = (0, 0);
    let mut whole = vec![0usize; 1 + runners.len()];
    let (mut pairs, mut other) = (0, 0);
    for w in ticks.windows(2) {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            if u32_le(a, 0x424) != 2 || u32_le(b, 0x424) != 2 {
                continue;
            }
            let (ma, mb) = (&w[0].extra[km].1[i], &w[1].extra[km].1[i]);
            let (na, nb) = (&w[0].extra[ka].1[i], &w[1].extra[ka].1[i]);
            if u32_le(ma, 0xc) != addr[i] {
                other += 1;
                continue;
            }
            if !one_step(a, b) {
                skew += 1;
                continue;
            }
            pairs += 1;
            let st = &stats[i];
            let mut rider = rider_of(a, st, ma, na);
            // what `cBoarder::vf7` 0x117198 moves before the motion, so the motion read it as the second sample holds
            // it: the input records (value +0, rate +4, target +8; each value moved toward its target by rate / 60:
            // brake +0x1fc, crouch +0x208, steer +0x214, the probe's side offset +0x244) and the spring band +0x294,
            // +0x298
            let after = rider_of(b, st, mb, nb);
            (rider.brake, rider.crouch, rider.steer, rider.p.x244, rider.d1, rider.d2) = (after.brake, after.crouch, after.steer, after.p.x244, after.d1, after.d2);
            let want = fields(&rider_of(b, st, mb, nb));
            let kind = u32_le(b, 0x2e0);
            let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
            let mut got = Vec::new();
            let mut mine = rider;
            update(&mut mine, &SURFACE_TABLE, &mut E { log: Vec::new(), hit, wall: false, meter: 0.0 });
            mine.vel = speed_limit(mine.vel, speed_limit_of(a, b, |x, y| x - y));
            got.push(fields(&mine));
            for (_, g) in runners.iter_mut() {
                *g.hit.borrow_mut() = hit;
                g.wall.set(false);
                g.put(&rider);
                g.r.cpu.set_gpr(4, g.motion as u64);
                g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                let mut r = g.get(&rider);
                // (the runner's float rules are still set from its call)
                r.vel = speed_limit(r.vel, speed_limit_of(a, b, r5900::ps2float::fsub));
                got.push(fields(&r));
            }
            // how many met the speed limit (`speed_limit_of`, applied above to each way's velocity)
            let v = vec4(b, 0x150);
            capped += ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() >= f32_at(b, 0x1c4) - 0.01) as usize;
            for (k, g) in got.iter().enumerate() {
                let mut all = true;
                for &f in &written {
                    let ok = g[f] == want[f];
                    count[k][f] += ok as usize;
                    all &= ok;
                }
                whole[k] += all as usize;
            }
        }
    }
    eprintln!("{path}: ground ticks (motion 2 at both samples): {pairs} compared ({capped} at the speed limit); left out {other} whose motion object's +0xc is not the rider, {skew} not one update apart");
    for (k, way) in ["ours (ground_motion::update)".to_string()].into_iter().chain(runners.iter().map(|(r, _)| format!("the game's in the runner, {r}"))).enumerate() {
        let line: Vec<String> = written.iter().map(|&f| format!("{} {}", names[f], count[k][f])).collect();
        eprintln!("  {way}: every field {}; {}", whole[k], line.join(", "));
    }
}

/// What the take-off's callees saw: `Boarder_SetMotion` 0x11f610 (the motion, and the rider's position, velocity and
/// normal at that call, which `AirMotion_Enter` 0x108310 hands to `AirPredict_AtTakeoff`), `Takeoff_SetSpinRates`
/// 0x126e30 and `Boarder_SetState` 0x11f608.
#[derive(Clone, Debug, Default, PartialEq)]
struct Off {
    motion: Vec<(u32, [V4; 3])>,
    spin_rates: u32,
    state: Vec<u32>,
}

/// Our take-off's callees on our rider (`orient::take_off`): the jump is `jump::jump_impulse`.
struct OurTakeOff<'a> {
    r: &'a mut Rider,
    motion: u32,
    jump_stat: u8,
    off: Off,
}
impl TakeOffEnv for OurTakeOff<'_> {
    fn set_motion(&mut self, _: &mut TakeOffRider, motion: u32) {
        self.motion = motion;
        self.off.motion.push((motion, [self.r.p.pos, self.r.vel, self.r.p.normal]));
    }
    fn jump_impulse(&mut self, t: &mut TakeOffRider, min: f32) {
        let s = JumpState {
            crouch: t.crouch,
            x208: t.x208,
            vel: self.r.vel,
            jump_stat: self.jump_stat,
            motion: self.motion,
            up: self.r.board_up,
            normal: self.r.p.normal,
            forward: self.r.p.plane_fwd,
        };
        self.r.vel = jump_impulse(min, &s).0;
    }
    fn takeoff_spin_rates(&mut self, _: &mut TakeOffRider) {
        self.off.spin_rates += 1;
    }
    fn set_state(&mut self, _: &mut TakeOffRider, state: u32) {
        self.off.state.push(state);
    }
}

/// The speed limit as `cBoarder::vf7` 0x117198 sets it after the motion when the rider is not in the air
/// (0x1179a4..0x117a84): the target is 3347.2222 cm/s while the speed timer +0x134 runs, else while boosting (+0x130
/// > 0) by the meter +0x1c (over 0.6659923 3347.2222, over 0.33362994 3072.1418, over 0 2932.43), else 2788.8394;
/// the limit +0x1c4 is the larger of that and the last tick's less 3.472222 (COP1 `sub.s`: `sub`). `a` is the rider
/// at the start of the tick, `b` after the shaping (the timer is moved before the motion).
fn speed_limit_of(a: &[u8], b: &[u8], sub: fn(f32, f32) -> f32) -> f32 {
    let target = if f32_at(b, 0x134) > 0.0 {
        f32::from_bits(0x4551338e)
    } else if f32_at(a, 0x130) > 0.0 {
        let m = f32_at(a, 0x1c);
        match () {
            _ if f32::from_bits(0x3f2a7e78) < m => f32::from_bits(0x4551338e),
            _ if f32::from_bits(0x3eaad18b) < m => f32::from_bits(0x45400245),
            _ if 0.0 < m => f32::from_bits(0x453746e1),
            _ => f32::from_bits(0x452e4d6e),
        }
    } else {
        f32::from_bits(0x452e4d6e)
    };
    let decayed = sub(f32_at(a, 0x1c4), f32::from_bits(0x405e38e3));
    if target < decayed { decayed } else { target }
}

/// Over the limit the velocity is scaled down to it (0x117968..0x117ab0: the length on VU0, `div.s`, `vmulx`).
fn speed_limit(vel: V4, limit: f32) -> V4 {
    use r5900::ps2float as pf;
    let sq = vel.map(|x| pf::mul(x, x));
    let speed = pf::sqrt(pf::add(pf::add(pf::add(sq[0], sq[1]), pf::mul(1.0, sq[2])), pf::mul(1.0, sq[3])));
    if limit < speed {
        let k = pf::div(limit, speed);
        vel.map(|x| pf::mul(x, k))
    } else {
        vel
    }
}

/// The game's prediction from (pos, vel, normal) with the game's landing (record +0x10 point, +0x30 normal, +0x40
/// word) on the first of our segments within 0.05 cm of its point, as `flight_predictions_against_the_running_game`
/// in `tests/replay.rs` answers it (no landing: every ray misses).
fn predict_like(pos: V4, vel: V4, normal: V4, rec: &[u8]) -> Predict {
    let (gpoint, gnormal, gword) = (vec4(rec, 0x10), vec4(rec, 0x30), u32_le(rec, 0x40));
    let mut found = false;
    let mut ray = |a: V4, b: V4| {
        if gword == 0xffff_ffff || found {
            return (-1.0, RayHit::default());
        }
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let q = [gpoint[0] - a[0], gpoint[1] - a[1], gpoint[2] - a[2]];
        let h = (q[0] * d[0] + q[1] * d[1] + q[2] * d[2]) / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
        let off = [q[0] - h * d[0], q[1] - h * d[1], q[2] - h * d[2]];
        if (0.0..=1.0).contains(&h) && (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt() < 0.05 {
            found = true;
            (h, RayHit { point: gpoint, normal: gnormal, word: gword })
        } else {
            (-1.0, RayHit::default())
        }
    };
    predict_at_takeoff(pos, vel, normal, &mut ray)
}

/// The take-off tick over a recorded race (board row F4b3c): every pair of samples one tick apart in which a rider
/// goes from the snow to the air (motion +0x424 2 → 1). The tick runs the ground motion as in
/// `ground_ticks_against_the_running_game` (same inputs, same answered ray cast), and then one of two ways up:
/// - the motion leaves the snow itself (`GroundMotion_Update` ends with `Boarder_TakeOff` 0x128470: air motion,
///   then the jump with no smallest jump, then state 0xd, or from the jump control the spin rates);
/// - the jump control (`JumpControl_Update` 0x1046b0, state 0xa) once the crouch +0x208 is unwound to 0, after
///   the motion and the speed limit: `Jump_ApplyImpulse` 0x1284e0 with 630.8786 cm/s at least, then the air
///   motion and the spin rates.
/// Run three ways: ours (`ground_motion::update`, `orient::take_off`, `jump::jump_impulse`; the jump control is not
/// ported, so for it the test makes the same calls), and the game's code in the function runner under IEEE adds and
/// under `fadd=g1`. Compared with the second sample: position and velocity, the ground motion's other fields, the
/// calls, and the flight prediction (rider+0x57b0, made inside `Boarder_SetMotion`): its inputs where it found no
/// landing (it keeps them), the time to the top (+0x48) from the inputs each way saw.
/// Needs TRICKY_RECORDING (windows `-400:10 46c0:10 57b0:64`) and TRICKY_STATS.
#[test]
fn takeoff_ticks_against_the_running_game() {
    let (Ok(paths), Ok(stats)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS")) else { return };
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let path = paths.split(';').next().unwrap();
    let (n, ticks) = load(path);
    let win = |o: i64| ticks[0].extra.iter().position(|(x, _)| *x == o);
    let (Some(km), Some(ka), Some(kp)) = (win(-0x400), win(0x46c0), win(0x57b0)) else {
        eprintln!("{path}: no -400, 46c0 and 57b0 windows: skipping");
        return;
    };
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners = Vec::new();
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut w) = world() else { return };
        w.r.float_rules = Rules::parse(rules).unwrap();
        let off = Rc::new(RefCell::new(Off::default()));
        let (o, rd) = (off.clone(), w.rd);
        w.r.stub(0x11f610, move |cpu, mem| {
            let m = cpu.gpr(5) as u32;
            o.borrow_mut().motion.push((m, [rv(mem, rd + 0x140), rv(mem, rd + 0x150), rv(mem, rd + 0x2a0)]));
            mem.write_u32(rd + 0x424, m);
        });
        let o = off.clone();
        w.r.stub(0x126e30, move |_, _| o.borrow_mut().spin_rates += 1);
        let o = off.clone();
        w.r.stub(0x11f608, move |cpu, _| o.borrow_mut().state.push(cpu.gpr(5) as u32));
        w.r.stub(0x156618, |c, _| c.set_f(0, 0.0)); // Score_StartTrickChain
        w.r.stub(0x126948, |_, _| {}); // Boarder_SetSteerTarget (the jump control while still crouched)
        w.r.stub(0x118f10, |_, _| {}); // Boarder_RequestReset
        for f in [0x1284e0, 0x250d60] {
            w.r.allow(f); // Jump_ApplyImpulse, Math_Sin
        }
        let ctl = w.r.mem.alloc(0x10);
        w.r.mem.write_u32(ctl, w.rd);
        runners.push((rules, w, off, ctl));
    }
    let one_step = |a: &[u8], b: &[u8]| {
        let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
        let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
        e > 1.0 && d > 0.7 * e && d < 1.4 * e
    };
    let names = ["pos", "vel", "fwd", "board_up", "plane_fwd", "side", "quat", "drag", "height", "bob", "lean", "yaw", "the way up", "prediction inputs", "time to the top"];
    let ways = 1 + runners.len();
    let mut count = vec![vec![0usize; names.len()]; ways];
    let mut of = vec![0usize; names.len()];
    let mut whole = vec![0usize; ways];
    let (mut pairs, mut other, mut skew, mut by_motion, mut by_jump, mut neither, mut no_pred, mut torn) = (0, 0, 0, 0, 0, 0, 0, 0);
    for w in ticks.windows(2) {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            if u32_le(a, 0x424) != 2 || u32_le(b, 0x424) != 1 {
                continue;
            }
            let (ma, mb) = (&w[0].extra[km].1[i], &w[1].extra[km].1[i]);
            let (na, nb) = (&w[0].extra[ka].1[i], &w[1].extra[ka].1[i]);
            let (pa, pb) = (&w[0].extra[kp].1[i], &w[1].extra[kp].1[i]);
            if u32_le(ma, 0xc) != addr[i] {
                other += 1;
                continue;
            }
            if !one_step(a, b) {
                skew += 1;
                continue;
            }
            pairs += 1;
            let st = &stats[i];
            let mut rider = rider_of(a, st, ma, na);
            let after = rider_of(b, st, mb, nb);
            (rider.brake, rider.crouch, rider.steer, rider.p.x244, rider.d1, rider.d2) = (after.brake, after.crouch, after.steer, after.p.x244, after.d1, after.d2);
            let kind = u32_le(b, 0x2e0);
            let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
            let crouch0 = f32_at(a, 0x1c8);
            // the jump control fires on this tick: state 0xa and the crouch unwound to 0 before the motion
            let jumps = rider.state == 0xa && rider.crouch == 0.0;

            // ours
            let mut mine = rider;
            let mut e = E { log: Vec::new(), hit, wall: false, meter: 0.0 };
            update(&mut mine, &SURFACE_TABLE, &mut e);
            let up_by_motion = e.log.contains(&Ev::TakeOff);
            let mut t = TakeOffRider { state: mine.state, crouch: crouch0, x208: mine.crouch };
            let mut env = OurTakeOff { r: &mut mine, motion: 2, jump_stat: st[0x17], off: Off::default() };
            if up_by_motion {
                take_off(&mut t, &mut env);
            } else if jumps {
                env.r.vel = speed_limit(env.r.vel, speed_limit_of(a, b, |x, y| x - y));
                env.jump_impulse(&mut t, f32::from_bits(0x441db83b));
                env.set_motion(&mut t, 1);
                env.takeoff_spin_rates(&mut t);
            }
            let ours_off = env.off;
            let mut got = vec![(mine, ours_off)];

            // the game's code
            for (_, g, off, ctl) in runners.iter_mut() {
                *g.hit.borrow_mut() = hit;
                g.wall.set(false);
                g.log.borrow_mut().clear();
                *off.borrow_mut() = Off::default();
                g.put(&rider);
                g.r.mem.write_u32(g.rd + 0x424, 2);
                g.r.mem.write_f32(g.rd + 0x1c8, crouch0);
                g.r.mem.write_u32(g.rd + 0x1dc, u32_le(a, 0x1dc));
                g.r.mem.write_u8(g.stats + 0x17, st[0x17]);
                g.r.cpu.set_gpr(4, g.motion as u64);
                g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                if g.log.borrow().contains(&Ev::TakeOff) {
                    g.r.cpu.set_gpr(4, g.rd as u64);
                    g.r.call(0x128470).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                } else if jumps {
                    // (the runner's float rules are still set from its call)
                    let v = speed_limit(rv(&mut g.r.mem, g.rd + 0x150), speed_limit_of(a, b, r5900::ps2float::fsub));
                    wv(&mut g.r.mem, g.rd + 0x150, v);
                    let pad = *ctl + 8;
                    g.r.mem.write_u32(pad, 0);
                    g.r.cpu.set_gpr(4, *ctl as u64);
                    g.r.cpu.set_gpr(5, pad as u64);
                    g.r.call(0x1046b0).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                }
                let r = g.get(&rider);
                got.push((r, off.borrow().clone()));
            }
            if up_by_motion {
                by_motion += 1;
            } else if jumps {
                by_jump += 1;
            } else {
                neither += 1;
            }

            // the prediction made this tick
            // (a record read while the game was inside the prediction, as in F4h2: `AirPredict_Reset` 0x123ab8 done,
            // velocity 0, or a landing with no landing time yet)
            let changed = f32_at(pb, 0x4c) == 0.0 && pb[8..0x54] != pa[8..0x54];
            let inside = vec4(pb, 0x20) == [0.0; 4] || (u32_le(pb, 0x40) != 0xffff_ffff && f32_at(pb, 0x44) == 0.0);
            let made = changed && !inside;
            no_pred += !changed as usize;
            torn += (changed && inside) as usize;
            let want = [vec4(b, 0x140), vec4(b, 0x150), vec4(b, 0x180), vec4(b, 0x1a0), vec4(b, 0x320), vec4(b, 0x330), vec4(b, 0x170)].map(b4);
            let want_f = [0x160, 0x1bc, 0x1c0, 0x1d8, 0x1b0].map(|o| f32_at(b, o).to_bits());
            for (k, (r, off)) in got.iter().enumerate() {
                let mut ok = [r.p.pos, r.vel, r.p.fwd, r.board_up, r.p.plane_fwd, r.p.side, r.quat].map(b4).iter().zip(&want).map(|(x, y)| x == y).collect::<Vec<_>>();
                ok.extend([r.drag, r.p.height, r.bob, r.lean, r.yaw].iter().zip(&want_f).map(|(x, y)| x.to_bits() == *y));
                ok.push(off.motion.len() == 1 && off.motion[0].0 == 1);
                let mut used = vec![true; names.len()];
                used[13] = false;
                used[14] = false;
                if made && off.motion.len() == 1 {
                    let [p, v, nn] = off.motion[0].1;
                    let no_landing = u32_le(pb, 0x40) == 0xffff_ffff;
                    used[13] = no_landing;
                    ok.push(no_landing && [p, v, nn].map(b4) == [vec4(pb, 0x10), vec4(pb, 0x20), vec4(pb, 0x30)].map(b4));
                    used[14] = true;
                    ok.push(predict_like(p, v, nn, pb).t_top.to_bits() == u32_le(pb, 0x48));
                } else {
                    ok.extend([false, false]);
                }
                let mut all = true;
                for f in 0..names.len() {
                    if used[f] {
                        count[k][f] += ok[f] as usize;
                        if k == 0 {
                            of[f] += 1;
                        }
                        all &= ok[f];
                    }
                }
                whole[k] += all as usize;
            }
        }
    }
    eprintln!("{path}: take-off ticks (motion 2 → 1): {pairs} compared ({by_motion} left the snow in the motion, {by_jump} by the jump control, {neither} neither; {no_pred} with no new prediction read, {torn} read while it was being made); left out {other} whose motion object's +0xc is not the rider, {skew} not one update apart");
    for (k, way) in ["ours".to_string()].into_iter().chain(runners.iter().map(|(r, ..)| format!("the game's in the runner, {r}"))).enumerate() {
        let line: Vec<String> = (0..names.len()).map(|f| format!("{} {} of {}", names[f], count[k][f], of[f])).collect();
        eprintln!("  {way}: all {}; {}", whole[k], line.join(", "));
    }
}

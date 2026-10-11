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

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
use tricky_game::collbody::{overlap, CollBody};
use tricky_game::hit::{rider_hit, HitRider};
use tricky_game::ridercol::{rider_collisions, ColEnv, ColRec, ColRider};
use tricky_game::jump::{jump_impulse, JumpState};
use tricky_game::orient::{take_off, TakeOffEnv, TakeOffRider};
use tricky_game::probe::{self, Hit};
use tricky_game::rider::ground_tick::{lean_targets, lean_up};
use tricky_game::surface::SURFACE_TABLE;
use tricky_game::animctl;
use tricky_game::control::{self, Bump, ControlEnv, LandNormal};
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::spin::{spin_enter, spin_update, AlignRider, Anim, SpinCall, SpinEnterRider, SpinObj, SpinRider};
use tricky_game::jump::{takeoff_spin_rates, TakeoffState};

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
    /// a boarder helper the control calls (its address)
    Call(u32),
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
    /// the boarder handler (rider+0x5ae0: word 0 the rider, its control states' objects from +0x610), the pad word
    /// `Boarder_UpdateControl` reads, and two animation sequences with their clip trees and headers (`Ctl`)
    handler: u32,
    pad: u32,
    seqs: [(u32, u32, u32); 2],
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
    let handler = r.mem.alloc(0x900);
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
    // the control after the motion (F4b3i): `Boarder_UpdateControl` 0x11c9a0 and the updates replayed (bump 0x1041b0,
    // fakie change 0x104030, hard landing 0x104800, normal landing 0x104e68), the animation queries they make
    // (`AnimCtl_GetClipType` 0x15eec0, `AnimCtl_GetLayerPhase` 0x15fe70, `AnimLayer_FindClip` 0x15a6c8,
    // `AnimClip_GetPhase` 0x159e80) on the layer `Ctl::put` lays out; what they ask of the boarder is logged
    for a in [0x11c9a0, 0x1041b0, 0x104030, 0x104800, 0x104e68, 0x15eec0, 0x15fe70, 0x15a6c8, 0x159e80] {
        r.allow(a);
    }
    for a in [0x126948, 0x126c00, 0x126b38, 0x126c78, 0x127918, 0x15fcf8, 0x118f10] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(Ev::Call(a)));
    }
    let pad = r.mem.alloc(16);
    let mut seqs = [(0, 0, 0); 2];
    for s in &mut seqs {
        *s = (r.mem.alloc(0x80), r.mem.alloc(0x10), r.mem.alloc(0x10));
    }
    Some(W { r, rd, stats, motion, log, hit, wall, meter, handler, pad, seqs })
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
    /// `Boarder_UpdateControl` 0x11c9a0 in the runner on the rider `get` left (its velocity `p.vel`): the animation
    /// controller laid out at +0x46c0, the handler's control objects copied (the game rider's address, `addr`, made
    /// the runner's), the pad word; then the velocity, board and the state `Boarder_SetState` was asked for
    fn control(&mut self, p: &mut Rider, c: &Ctl, addr: u32) {
        let (rd, h) = (self.rd, self.handler);
        let m = &mut self.r.mem;
        wv(m, rd + 0x150, p.vel);
        m.write_u32(rd + 0x418, c.controller as u32);
        for (a, v) in lay(c, rd + 0x46c0, self.seqs[0]) {
            m.write_u32(a, v);
        }
        for o in (0..c.handler.len()).step_by(4) {
            let v = u32_le(&c.handler, o);
            m.write_u32(h + 0x600 + o as u32, if v == addr { rd } else { v });
        }
        m.write_u32(self.pad, c.pad);
        self.log.borrow_mut().clear();
        self.r.cpu.set_gpr(4, h as u64);
        self.r.cpu.set_gpr(5, self.pad as u64);
        self.r.call(0x11c9a0).unwrap_or_else(|e| panic!("Boarder_UpdateControl: {e:?}"));
        let m = &mut self.r.mem;
        (p.vel, p.quat, p.p.fwd, p.board_side, p.board_up, p.yaw) = (rv(m, rd + 0x150), rv(m, rd + 0x170), rv(m, rd + 0x180), rv(m, rd + 0x190), rv(m, rd + 0x1a0), m.read_f32(rd + 0x1b0));
        for e in self.log.borrow().iter() {
            if let Ev::State(s) = e {
                if *s != p.state {
                    p.state = *s;
                }
            }
        }
    }

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
/// What `Boarder_UpdateControl` 0x11c9a0 reads of a rider (F4b3i): layer 0's and 1's clips (+0x46c0 +8, +0xc) and, of
/// layer 0's sequences (windows `4718>0`, `4718>30>0`, their lengths `4718>78>8>a`, `4718>30>78>8>a`), the first
/// playing layer 0's clip (what `AnimLayer_FindClip` finds; none: the phase is 1); the handler's control objects
/// (window `5ae0>600`, +0x600..+0x830), the pad word and the controller (+0x418)
#[derive(Clone)]
struct Ctl {
    clips: [u32; 2],
    seq: Option<(Vec<u8>, u16)>,
    handler: Vec<u8>,
    pad: u32,
    controller: i32,
}

/// The controls replayed: bump (2), change fakie (4), fakie cruise (5, not on its turn clip 0x202, which plays clips
/// and flips the stance), hard landing (0xb), normal landing (0xc)
const CONTROLS: [u32; 5] = [2, 4, 5, 0xb, 0xc];

/// (the animation read from sample `t`, the handler from `th`)
fn ctl_of(t: &recording::Tick, th: &recording::Tick, i: usize, ka: usize, addr: u32, pad: u32) -> Option<Ctl> {
    let w = |name: &str| t.chased.iter().find(|(s, _)| s == name).map(|(_, v)| &v[i]);
    let h = th.chased.iter().find(|(s, _)| s == "5ae0>600").map(|(_, v)| &v[i])?;
    let (s1, s2, l1, l2) = (w("4718>0")?, w("4718>30>0")?, w("4718>78>8>a")?, w("4718>30>78>8>a")?);
    let c = &t.extra[ka].1[i];
    let clips = [u32_le(c, 8), u32_le(c, 0xc)];
    // the list ends at the layer's sentinel, rider + 0x4720
    let seq = if u32_le(s1, 0x40) == clips[0] {
        Some((s1.clone(), u16::from_le_bytes([l1[0], l1[1]])))
    } else if u32_le(s1, 0x30) != addr + 0x4720 && u32_le(s2, 0x40) == clips[0] {
        Some((s2.clone(), u16::from_le_bytes([l2[0], l2[1]])))
    } else {
        None
    };
    Some(Ctl { clips, seq, handler: h.clone(), pad, controller: u32_le(&t.riders[i], 0x418) as i32 })
}

/// The animation controller (at `ctl`) laid out for `animctl`'s queries: the three layers' clips (layer 2's none) and
/// lists (+0x28 + 0x70·k: head +0x30, sentinel +0x38), layer 0's holding the sequence (at `seq`: +0x30 next, +0x78
/// the tree, whose +8 is the header, whose u16 +0xa is the length) when there is one; as word writes, in order
fn lay(c: &Ctl, ctl: u32, seq: (u32, u32, u32)) -> Vec<(u32, u32)> {
    let mut w = vec![(ctl + 8, c.clips[0]), (ctl + 0xc, c.clips[1]), (ctl + 0x10, animctl::NONE)];
    for k in 0..3 {
        let l = ctl + 0x28 + 0x70 * k;
        w.push((l + 0x30, l + 0x38));
    }
    if let Some((bytes, len)) = &c.seq {
        w.extend((0..0x80).step_by(4).map(|o| (seq.0 + o as u32, u32_le(bytes, o))));
        let l = ctl + 0x28;
        w.extend([(l + 0x30, seq.0), (seq.0 + 0x30, l + 0x38), (seq.0 + 0x78, seq.1), (seq.1 + 8, seq.2), (seq.2 + 8, (*len as u32) << 16)]);
    }
    w
}

/// Our side of the control: `control::update_control` on an animation controller in a `pf::Mem` (the clip table
/// copied from the ELF), the board turned by `orient::rotate_about_axis` about its up (+0x1a0, as
/// `BumpControl_Update` passes it); the state set is kept (as `Boarder_SetState` 0x11f608: only a different one)
struct CtlEnv {
    m: tricky_game::pf::Mem,
    o: Orient,
    pad: u32,
    bump: Bump,
    land: LandNormal,
    state: Option<u32>,
}

const CTL: u32 = 0x0200_0000;

impl ControlEnv for CtlEnv {
    fn clip(&mut self, layer: u32) -> i32 {
        animctl::clip(&self.m, CTL, layer) as i32
    }
    fn clip_type(&mut self, layer: u32) -> i32 {
        animctl::clip_type(&self.m, CTL, layer) as i32
    }
    fn clip_at_end(&mut self, layer: u32) -> bool {
        animctl::at_end(&self.m, CTL, layer)
    }
    fn layer_phase(&mut self, layer: u32) -> f32 {
        animctl::layer_phase(&self.m, CTL, layer).0
    }
    fn play_clip(&mut self, _: &mut control::Rider, _: i32) {}
    fn set_state(&mut self, r: &mut control::Rider, s: u32) {
        if s != r.control {
            r.control = s;
            self.state = Some(s);
        }
    }
    fn set_motion(&mut self, _: &mut control::Rider, _: u32) {}
    fn set_steer_target(&mut self, _: &mut control::Rider, _: f32) {}
    fn set_crouch_target(&mut self, _: &mut control::Rider, _: f32) {}
    fn set_brake_crouch_input(&mut self, _: &mut control::Rider, _: f32, _: f32) {}
    fn set_brake_target(&mut self, _: &mut control::Rider, _: f32) {}
    fn update_boost(&mut self, _: &mut control::Rider, _: u32, _: u32) {}
    fn update_lean_anim(&mut self, _: &mut control::Rider, _: f32, _: f32) {}
    fn rotate_about_axis(&mut self, r: &mut control::Rider, angle: f32) {
        let axis = self.o.up;
        tricky_game::orient::rotate_about_axis(&mut self.o, angle, axis);
        r.board = self.o.fwd;
    }
    fn request_reset(&mut self, _: &mut control::Rider) {}
    fn jump_impulse(&mut self, _: &mut control::Rider, _: f32) {
        unreachable!("the jump control is replayed by the take-off test")
    }
    fn takeoff_spin_rates(&mut self, _: &mut control::Rider) {
        unreachable!("the jump control is replayed by the take-off test")
    }
    fn play_celebrate(&mut self, _: &mut control::Rider) {}
    fn play_disappointed(&mut self, _: &mut control::Rider) {}
    fn play_wind_up(&mut self, _: &mut control::Rider) {}
    fn play_takeoff(&mut self, _: &mut control::Rider) {}
    fn ai_taunt_check(&mut self, _: &mut control::Rider) {}
    fn random_flourish(&mut self, _: &mut control::Rider) {}
    fn stop(&mut self, _: &mut control::Rider) {}
    fn race_riders(&mut self) -> i32 {
        6
    }
    fn race_state(&mut self) -> i32 {
        3
    }
    fn cam_start(&mut self, _: i32, _: &str) {}
    fn run_control(&mut self, r: &mut control::Rider, c: u32) {
        let pad = self.pad;
        match c {
            2 => {
                let mut b = self.bump;
                b.update(self, pad, r);
                self.bump = b;
            }
            4 => control::change_fakie(self, pad, r),
            0xb => control::land_hard(self, pad, r),
            0xc => {
                let mut l = self.land;
                l.update(self, pad, r);
                self.land = l;
            }
            _ => panic!("control {c:x} not replayed"),
        }
    }
}

/// `Boarder_UpdateControl` ours on the rider after its motion: the velocity, board, state it leaves
fn control_ours(p: &mut Rider, c: &Ctl, table: &[u8]) {
    let mut m = tricky_game::pf::Mem { regions: vec![(animctl::CLIP_TABLE, table.to_vec()), (CTL, vec![0; 0x300])] };
    for (a, v) in lay(c, CTL, (CTL + 0x200, CTL + 0x280, CTL + 0x290)) {
        m.w32(a, v);
    }
    let o = Orient { q: p.quat, fwd: p.p.fwd, side: p.board_side, up: p.board_up, yaw: p.yaw };
    let (bump, land) = (Bump { turn: f32_at(&c.handler, 0x18), done: f32_at(&c.handler, 0x1c) }, LandNormal { crouch: f32_at(&c.handler, 0xf8) });
    let mut env = CtlEnv { m, o, pad: c.pad, bump, land, state: None };
    let mut r = control::Rider { vel: p.vel, board: p.p.fwd, brake: p.brake, crouch: p.crouch, steer: p.steer, surface: p.p.surface, controller: c.controller, stance: p.switch, control: p.state, ..Default::default() };
    control::update_control(&mut env, c.pad, &mut r);
    (p.vel, p.quat, p.p.fwd, p.board_side, p.board_up, p.yaw) = (r.vel, env.o.q, env.o.fwd, env.o.side, env.o.up, env.o.yaw);
    if let Some(s) = env.state {
        p.state = s;
    }
}

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

/// A rider's collision body as recorded (rider+0x470: +0x10 the bounding centre, +0x20 its radius, +0x24, +0x28 the
/// sphere count, +0x30 the spheres, 0x20 each).
fn body_of(r: &[u8]) -> CollBody {
    let mut b = CollBody::new();
    b.center = vec4(r, 0x480);
    b.radius = f32_at(r, 0x490);
    b.x24 = u32_le(r, 0x494) as i32;
    b.count = (u32_le(r, 0x498) as i32).clamp(0, 18);
    for k in 0..18 {
        b.spheres[k] = (vec4(r, 0x4a0 + 0x20 * k), f32_at(r, 0x4b0 + 0x20 * k));
    }
    b
}

/// What `Boarder_RiderCollisions` 0x123fc8 (`ridercol::rider_collisions`, checked in `tests/ridercol.rs`) asks while
/// one frame is replayed (board rows F4b3g, F4b3h). The bodies start as the first sample holds them (set by
/// `Boarder_UpdateBroadphase` 0x126748 after every rider's tick; the ground motion does not move a body, the air
/// motion does, by its step: taken as the rider's move between the samples) and are moved by the pushes
/// (`Boarder_Translate` 0x1267a0). The hits are `hit::rider_hit` on the hit rider as it is at that moment (`hit`:
/// after its tick when it has ticked, else as the first sample holds it), with the pair's record from the second
/// sample (+0x40 + 0x20·j: [0] the distance, [1] the bearing; read only for a hop or a wipe-out, which leave the
/// ground). Finished riders (`Boarder_IsFinishedAndStopped`) are not looked at: none in these recordings stop
/// before the end; nor the shoves (the layer-1 clip type is not in the recording: `clip_type1` answers 0).
#[derive(Clone)]
struct Frame {
    body: Vec<CollBody>,
    hit: Vec<HitRider>,
    rec: Vec<Vec<[f32; 2]>>,
    /// `Boarder_GetSpeedStat` 0x11d7b0 of each rider at that moment
    stat: Vec<f32>,
    /// per rider: pushes and hits it got this frame
    pushes: Vec<usize>,
    hits: Vec<usize>,
}

impl Frame {
    fn new(a: &[Vec<u8>], b: &[Vec<u8>], stats: &[Vec<u8>]) -> Frame {
        let n = a.len();
        Frame {
            body: a.iter().map(|r| body_of(r)).collect(),
            hit: a.iter().map(|r| hit_rider(r)).collect(),
            rec: b.iter().map(|r| (0..n).map(|j| [f32_at(r, 0x40 + 0x20 * j), f32_at(r, 0x44 + 0x20 * j)]).collect()).collect(),
            stat: (0..n).map(|i| speed_stat(&stats[i], f32_at(&a[i], 0x130))).collect(),
            pushes: vec![0; n],
            hits: vec![0; n],
        }
    }
}

impl ColEnv for Frame {
    fn overlap(&mut self, a: usize, b: usize) -> Option<V4> {
        overlap(&self.body[a], &self.body[b]).map(|(v, _)| v)
    }
    fn finished_stopped(&mut self, _: usize) -> bool {
        false
    }
    fn translate(&mut self, r: usize, v: V4) {
        self.body[r].translate(v);
        self.pushes[r] += 1;
    }
    fn speed_stat(&mut self, r: usize) -> f32 {
        self.stat[r]
    }
    fn rider_hit(&mut self, speed: f32, to: usize, _: usize, owner: usize, of: usize, dir: V4, vel: &mut V4) -> bool {
        let mut h = self.hit[to];
        h.vel = *vel;
        let knocked = rider_hit(speed, dir, self.rec[owner][of], false, false, &mut h, &mut Vec::new());
        *vel = h.vel;
        self.hit[to] = h;
        self.hits[to] += 1;
        knocked
    }
    fn score_knockdown(&mut self, _: usize) {}
    fn score_crash(&mut self, _: usize) {}
    fn crash_meter(&mut self, _: usize) {}
    fn rand_game(&mut self) {}
    fn collision_sound(&mut self, _: usize, _: usize) {}
    fn clip_type1(&mut self, _: usize) -> u32 {
        0
    }
    fn clip_flag(&mut self, _: usize, _: u32) -> bool {
        false
    }
}

/// What `Boarder_RiderHit` reads of a recorded rider.
fn hit_rider(r: &[u8]) -> HitRider {
    HitRider { heading: f32_at(r, 0x1b0), pos: vec4(r, 0x140), vel: vec4(r, 0x150), normal: vec4(r, 0x2a0), motion: u32_le(r, 0x424), crouch: f32_at(r, 0x1c8) }
}

/// `Boarder_GetSpeedStat` 0x11d7b0: stats+0x1e × (stats+0x12 × 2.145779 × 0.003921569 + 0.79599), then × (1 + the
/// boost level +0x130 × 7.999995).
fn speed_stat(st: &[u8], boost: f32) -> f32 {
    let m = st[0x1e] as f32 * (st[0x12] as f32 * 2.145_779 * 0.003_921_569 + 0.795_99);
    boost * 7.999_995 * m + m
}

/// The driver's view of a recorded rider: position, velocity, motion and its records of the others (+0x40 + 0x20·j:
/// +8 the last bump's tick, +0xc the last shove's, +0x1a the hits).
fn col_rider(r: &[u8], n: usize) -> ColRider {
    ColRider {
        pos: vec4(r, 0x140),
        vel: vec4(r, 0x150),
        motion: u32_le(r, 0x424),
        rec: (0..n).map(|j| ColRec { bump: u32_le(r, 0x48 + 0x20 * j) as i32, shove: u32_le(r, 0x4c + 0x20 * j) as i32, hits: u16::from_le_bytes([r[0x5a + 0x20 * j], r[0x5b + 0x20 * j]]) }).collect(),
        ..Default::default()
    }
}

/// One frame's rider collisions (`Boarder_RiderCollisions` 0x123fc8 at 0x117cd4, after the motion, the speed limit
/// and the control) around one replayed rider, `me` (board row F4h4). The others are taken as
/// `ground_ticks_against_the_running_game` takes the riders it does not replay (the second sample's velocity and
/// motion, an air rider's body moved by its step). `before` runs the riders that tick before `me` in `order` (their
/// pushes and hits move `me` before its motion: its start position and velocity are returned); `after` puts `me` as
/// its motion left it and runs the rest, returning its position and velocity at the frame's end.
#[derive(Clone)]
struct FrameRun {
    f: Frame,
    riders: Vec<ColRider>,
    me: usize,
    at: usize,
    tick: i32,
}

impl FrameRun {
    fn before(w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], order: &[usize], tick: i32, me: usize) -> (FrameRun, V4, V4) {
        let n = w0.len();
        let mut run = FrameRun { f: Frame::new(w0, w1, stats), riders: w0.iter().map(|x| col_rider(x, n)).collect(), me, at: 0, tick };
        while order[run.at] != me {
            run.step(w0, w1, stats, order[run.at], None);
            run.at += 1;
        }
        let (pos, vel) = (run.riders[me].pos, run.riders[me].vel);
        (run, pos, vel)
    }

    fn after(mut self, w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], order: &[usize], r: &Rider, motion: u32) -> (V4, V4, usize, usize) {
        for k in self.at..order.len() {
            let o = order[k];
            self.step(w0, w1, stats, o, (o == self.me).then_some((r, motion)));
        }
        (self.riders[self.me].pos, self.riders[self.me].vel, self.f.pushes[self.me], self.f.hits[self.me])
    }

    fn step(&mut self, w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], o: usize, mine: Option<(&Rider, u32)>) {
        let n = w0.len();
        let (a, b) = (&w0[o], &w1[o]);
        let (f, riders) = (&mut self.f, &mut self.riders);
        if let Some((r, motion)) = mine {
            riders[o].pos = r.p.pos;
            riders[o].vel = r.vel;
            riders[o].motion = motion;
            f.hit[o] = HitRider { heading: r.yaw, pos: r.p.pos, vel: r.vel, normal: r.p.normal, motion, crouch: f32_at(b, 0x1c8) };
        } else {
            if u32_le(a, 0x424) == 1 && u32_le(b, 0x424) == 1 {
                let (p, q) = (vec4(a, 0x140), vec4(b, 0x140));
                f.body[o].translate([q[0] - p[0], q[1] - p[1], q[2] - p[2], q[3] - p[3]]);
            }
            riders[o].vel = vec4(b, 0x150);
            riders[o].motion = u32_le(b, 0x424);
            f.hit[o] = hit_rider(b);
        }
        f.stat[o] = speed_stat(&stats[o], f32_at(b, 0x130));
        let list: Vec<usize> = (0..n).filter(|&x| x != o).collect();
        rider_collisions(o, &list, riders, self.tick, false, f);
    }
}

fn w0_len(ticks: &[recording::Tick]) -> usize {
    ticks.first().map_or(0, |t| t.riders[0].len())
}

/// The ground tick over a recorded race (board row F4b3b). For every pair of samples one tick apart in which a rider
/// is on the snow (motion +0x424 = 2) at both: `GroundMotion_Update` 0x10a0d8 on the rider as recorded at the first,
/// against the second, field by field. Run three ways: our `ground_motion::update`, and the game's own code in the
/// function runner (the world of `ground_motion_is_the_originals`) under IEEE adds and under PCSX2's COP1 adder (F4g).
///
/// The world's ray cast under `Boarder_ProbeGround` is answered with what the probe left in the second sample (ground
/// point +0x2d0, normal +0x2a0, surface +0x290, kind +0x2e0; no hit when the kind is 3), so the course is not needed.
/// `cBoarder::vf7` 0x117198 shapes the inputs (brake, crouch, steer, the probe's side offset) and the spring band
/// before the motion, so those are taken from the second sample. After the motion the tick caps the speed
/// (`speed_limit_of`, `speed_limit`: applied to each way), may change the control state, pushes riders apart and hits
/// them (`Boarder_RiderCollisions` 0x123fc8), and at its end `Boarder_UpdateLeanTargets` 0x129500 turns the rider's up
/// +0x2c0 toward the normal and aims the lean record (+0x24c): run each way too (ours `lean_up`, `lean_targets`), and
/// both compared (board row F4b3e). Samples read inside a tick are left out (`late`). With the bodies recorded
/// (window 0x700), the whole frame is replayed each way: the riders in tick order, each one's motion from where the
/// earlier riders' pushes and hits left it, its collisions with `ridercol::rider_collisions` and `hit::rider_hit`
/// (`Frame`; board rows F4b3g, F4b3h); with the riders' tick counts (window `5900:4`), frames not sampled at a frame's
/// end are left out. TRICKY_DBG lists the bump records and the hit ticks' velocities the replay misses.
/// Not in the recording, so not given: the walls (`Boarder_WallCollide` 0x126250 answers no wall) and the instances
/// (`Boarder_InstanceCollisions` 0x125088 does nothing). The ground motion object is the state manager
/// *(rider+0x5ae0) + 0x4b0, rider − 0x400 for most riders (window `-400:10`; the test checks its +0xc is the rider),
/// rider − 0x3d0 for the one whose manager is at rider − 0x880 (window `-3d0:10`, when recorded), the clip window
/// `46c0:10`. Needs TRICKY_RECORDING and TRICKY_STATS (lines `rider stats-address hex`).
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
    // the rider whose state manager is at rider − 0x880: its ground motion object is 0x30 further on (window `-3d0:10`)
    let km6 = ticks[0].extra.iter().position(|(o, _)| *o == -0x3d0);
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
        "lean", "phase", "yaw", "surface", "surface_ticks", "kind", "state", "mask", "lean_target",
    ];
    // per field: the field's bits (vectors as four words)
    // (the last, the lean record's target +0x24c, is not the motion's: `Boarder_UpdateLeanTargets` sets it)
    let fields = |r: &Rider, target: f32| -> Vec<Vec<u32>> {
        let b = bits(r);
        (0..names.len() - 1).map(|f| if f < 12 { b[4 * f..4 * f + 4].to_vec() } else { vec![b[48 + f - 12]] }).chain([vec![target.to_bits()]]).collect()
    };
    let written: Vec<usize> = (0..names.len()).collect();
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
    // A sample read while the game was inside the rider's tick, after the motion and before `Boarder_UpdateLeanTargets`
    // at its end: the rider's up +0x2c0 is still the last tick's (inferred from the samples: PINE reads while the EE
    // runs; the up only stands still once it equals the normal)
    let late = |p: &[u8], x: &[u8]| vec4(x, 0x2c0) == vec4(p, 0x2c0) && vec4(x, 0x2c0) != vec4(x, 0x2a0);
    let mut torn = 0;
    let kc = ticks[0].extra.iter().position(|(o, _)| *o == 0x5900);
    let mut mid = 0;
    // the riders' bodies are in the recording when its window reaches +0x6e0: then each frame's pushes are replayed
    // (`frame_pushes`), the riders ticking in `order`: their `cBoarderHandler` objects sit in object list 8 (updated
    // by `Game_UpdateObjectLists` 0x1814c0 through `BxList_UpdateAll` 0x17d950) from slot 5 down to slot 0, as read
    // over PINE; TRICKY_ORDER (slots, comma separated) overrides it, TRICKY_NOPUSH turns the pushes off
    let pushes = w0_len(&ticks) >= 0x6e0 && std::env::var("TRICKY_NOPUSH").is_err();
    let order: Vec<usize> = std::env::var("TRICKY_ORDER").map(|o| o.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or((0..n).rev().collect());
    let mut pushed = 0;
    let mut pushed_whole = vec![0usize; 1 + runners.len()];
    // pushed ticks under fadd=g1 by whether the rider was also hit (the game's bump record +0x40 + 0x20·j +8 holds
    // the second sample's tick): [hit][position right], and of the hit ones how many have the velocity right
    let mut pushed_pos = [[0usize; 2]; 2];
    let mut hit_vel = 0;
    // the replayed frame's bumps against the game's records (pairs whose record either side changed this frame)
    let (mut bumps, mut bumps_right) = (0, 0);
    struct Prep {
        rider: Rider,
        want: Vec<Vec<u32>>,
        hit: Option<Hit>,
        ctl: Option<Ctl>,
    }
    // the clip table the control's `AnimCtl_GetClipType` reads (ELF data)
    let table: Vec<u8> = (0..0x300 * 0x1c).step_by(4).flat_map(|o| runners[0].1.r.mem.read_u32(animctl::CLIP_TABLE + o).to_le_bytes()).collect();
    let anim_b = std::env::var("TRICKY_ANIM_B").is_ok();
    // the clips are taken from the second sample (TRICKY_CLIP_A: the first), the sequences from the first
    // (TRICKY_ANIM_B: the second): what matches best (rec15: the fakie change's clip 0x200 already 0x1ff, the bump's
    // phase the first sample's). Inferred: the animation moves on before the rider's tick, and the sequence windows,
    // read in later PINE batches than the rider's, are read after it
    let clip_b = std::env::var("TRICKY_CLIP_A").is_err();
    // controls replayed by state, and those left out (fakie cruise on its turn clip; no windows)
    let mut ctl_count = std::collections::BTreeMap::<u32, usize>::new();
    let (mut ctl_turn, mut ctl_none, mut ctl_right) = (0, 0, 0);
    let mut ctl_same = vec![0usize; runners.len()];
    // and of those, the quaternion equal to the game's: ours, the runner's
    let (mut ctl_ours_game, mut ctl_runner_game) = (vec![0usize; runners.len()], vec![0usize; runners.len()]);
    for (wi, w) in ticks.windows(2).enumerate() {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        // with the riders' tick counts recorded (window `5900:4`), a frame whose samples are not both of a frame's
        // end (every rider ticked once between them, all at the same count) is left out
        if let Some(kc) = kc {
            let c = |t: &recording::Tick, r: usize| f32_at(&t.extra[kc].1[r], 0);
            if (0..n).any(|r| c(&w[1], r) != c(&w[0], r) + 1.0 || c(&w[0], r) - w[0].tick as f32 != c(&w[0], 0) - w[0].tick as f32) {
                mid += 1;
                continue;
            }
        }
        // the riders whose ground tick is replayed, and what each needs
        let mut prep: Vec<Option<Prep>> = (0..n).map(|_| None).collect();
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            if u32_le(a, 0x424) != 2 || u32_le(b, 0x424) != 2 {
                continue;
            }
            let k = match km6 {
                Some(k6) if u32_le(&w[0].extra[km].1[i], 0xc) != addr[i] => k6,
                _ => km,
            };
            let (ma, mb) = (&w[0].extra[k].1[i], &w[1].extra[k].1[i]);
            let (na, nb) = (&w[0].extra[ka].1[i], &w[1].extra[ka].1[i]);
            if u32_le(ma, 0xc) != addr[i] {
                other += 1;
                continue;
            }
            if !one_step(a, b) {
                skew += 1;
                continue;
            }
            let before = (wi > 0 && ticks[wi - 1].tick + 1 == w[0].tick).then(|| &ticks[wi - 1].riders[i]);
            if late(a, b) || before.is_some_and(|p| late(p, a)) {
                torn += 1;
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
            let want = fields(&after, f32_at(b, 0x24c));
            let kind = u32_le(b, 0x2e0);
            let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
            // how many met the speed limit (`speed_limit_of`, applied below to each way's velocity)
            let v = vec4(b, 0x150);
            capped += ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() >= f32_at(b, 0x1c4) - 0.01) as usize;
            // the control (F4b3i) on the states replayed. STANDIN: the pad word is the computer's (cComputer::GetControls
            // 0x1384b8 into the stack of `cBoarder::vf7`, not recordable): only the brake bit (4), set when the rider
            // ends the tick prewinding (0xe), as the landings and the bump pick 0xe on it
            let ctl = if CONTROLS.contains(&rider.state) && std::env::var("TRICKY_NOCTL").is_err() {
                let pad = if u32_le(b, 0x428) == 0xe { 4 } else { 0 };
                match ctl_of(if anim_b { &w[1] } else { &w[0] }, &w[0], i, ka, addr[i], pad) {
                    Some(c) if rider.state == 5 && c.clips[0] == 0x202 => {
                        ctl_turn += 1;
                        None
                    }
                    Some(mut c) => {
                        if clip_b {
                            let x = &w[1].extra[ka].1[i];
                            c.clips = [u32_le(x, 8), u32_le(x, 0xc)];
                        }
                        *ctl_count.entry(rider.state).or_default() += 1;
                        Some(c)
                    }
                    None => {
                        ctl_none += 1;
                        None
                    }
                }
            } else {
                None
            };
            prep[i] = Some(Prep { rider, want, hit, ctl });
        }
        if prep.iter().all(Option::is_none) {
            continue;
        }
        // The frame replayed each way: the riders tick in `order`; a replayed rider's motion starts from where the
        // pushes and hits of the riders that ticked before it left it (+0x140, +0x150), then the speed limit, then
        // its collisions (`Boarder_RiderCollisions` 0x123fc8 at 0x117cd4, while the race's state is 3 or more and not
        // 5: always so in these recordings), then `Boarder_UpdateLeanTargets` 0x129500 (0x117ce4) on the velocity as
        // it is then; the riders that tick after it may still push and hit it. The others' ticks are taken from the
        // second sample (velocity, motion; an air rider's body moved by its step).
        let tick = w[1].tick as i32;
        let mut got: Vec<Vec<Vec<Vec<u32>>>> = vec![Vec::new(); n];
        let mut frame_pushes = vec![0usize; n];
        let mut frame_hits = vec![0usize; n];
        for k in 0..1 + runners.len() {
            let mut f = Frame::new(&w[0].riders, &w[1].riders, &stats);
            let mut riders: Vec<ColRider> = w[0].riders.iter().map(|r| col_rider(r, n)).collect();
            let mut done: Vec<Option<(Rider, f32)>> = vec![None; n];
            for &me in &order {
                let (a, b) = (&w[0].riders[me], &w[1].riders[me]);
                if let Some(p) = &prep[me] {
                    let mut rider = p.rider;
                    rider.p.pos = riders[me].pos;
                    rider.vel = riders[me].vel;
                    let r = if k == 0 {
                        let mut mine = rider;
                        update(&mut mine, &SURFACE_TABLE, &mut E { log: Vec::new(), hit: p.hit, wall: false, meter: 0.0 });
                        mine.vel = speed_limit(mine.vel, speed_limit_of(a, b, |x, y| x - y));
                        if let Some(c) = &p.ctl {
                            control_ours(&mut mine, c, &table);
                        }
                        mine
                    } else {
                        let g = &mut runners[k - 1].1;
                        *g.hit.borrow_mut() = p.hit;
                        g.wall.set(false);
                        g.put(&rider);
                        g.r.cpu.set_gpr(4, g.motion as u64);
                        g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("tick {} rider {me}: {e:?}", w[0].tick));
                        let mut r = g.get(&rider);
                        // (the runner's float rules are still set from its call)
                        r.vel = speed_limit(r.vel, speed_limit_of(a, b, r5900::ps2float::fsub));
                        if let Some(c) = &p.ctl {
                            // ours from the same rider, against the game's under each way's rules
                            let mut mine = r;
                            control_ours(&mut mine, c, &table);
                            g.control(&mut r, c, addr[me]);
                            ctl_same[k - 1] += (bits(&mine) == bits(&r)) as usize;
                            ctl_ours_game[k - 1] += (fields(&mine, 0.0)[10] == p.want[10]) as usize;
                            ctl_runner_game[k - 1] += (fields(&r, 0.0)[10] == p.want[10]) as usize;
                            if bits(&mine) != bits(&r) && std::env::var("TRICKY_DBG3").is_ok() {
                                let (x, y) = (bits(&mine), bits(&r));
                                let d: Vec<usize> = (0..x.len()).filter(|&j| x[j] != y[j]).collect();
                                eprintln!("DBG3 way {k} state {:x} clip {:x} words {d:?} ulps {:?}", p.rider.state, c.clips[0], d.iter().map(|&j| x[j] as i64 - y[j] as i64).collect::<Vec<_>>());
                            }
                        }
                        r
                    };
                    riders[me].pos = r.p.pos;
                    riders[me].vel = r.vel;
                    riders[me].motion = 2;
                    f.hit[me] = HitRider { heading: r.yaw, pos: r.p.pos, vel: r.vel, normal: r.p.normal, motion: 2, crouch: f32_at(b, 0x1c8) };
                    done[me] = Some((r, 0.0));
                } else {
                    if u32_le(a, 0x424) == 1 && u32_le(b, 0x424) == 1 {
                        let (p, q) = (vec4(a, 0x140), vec4(b, 0x140));
                        f.body[me].translate([q[0] - p[0], q[1] - p[1], q[2] - p[2], q[3] - p[3]]);
                    }
                    riders[me].vel = vec4(b, 0x150);
                    riders[me].motion = u32_le(b, 0x424);
                    f.hit[me] = hit_rider(b);
                }
                f.stat[me] = speed_stat(&stats[me], f32_at(b, 0x130));
                if pushes {
                    let list: Vec<usize> = (0..n).filter(|&o| o != me).collect();
                    rider_collisions(me, &list, &mut riders, tick, false, &mut f);
                }
                // the end of the tick: `Boarder_UpdateLeanTargets` turns the up toward the normal and aims the lean
                if let Some((r, target)) = &mut done[me] {
                    r.vel = riders[me].vel;
                    if k == 0 {
                        r.up = lean_up(r.up, r.p.normal);
                        *target = lean_targets(2, r.steer, r.vel).1;
                    } else {
                        let g = &mut runners[k - 1].1;
                        wv(&mut g.r.mem, g.rd + 0x150, r.vel);
                        g.r.mem.write_u32(g.rd + 0x424, 2);
                        g.r.cpu.set_gpr(4, g.rd as u64);
                        g.r.call(0x129500).unwrap_or_else(|e| panic!("tick {} rider {me}: {e:?}", w[0].tick));
                        r.up = rv(&mut g.r.mem, g.rd + 0x2c0);
                        *target = g.r.mem.read_f32(g.rd + 0x24c);
                    }
                }
            }
            for i in 0..n {
                if let Some((mut r, target)) = done[i] {
                    r.vel = riders[i].vel;
                    r.p.pos = riders[i].pos;
                    got[i].push(fields(&r, target));
                }
            }
            if k == runners.len() {
                frame_pushes = f.pushes.clone();
                frame_hits = f.hits.clone();
                for i in 0..n {
                    for j in (0..n).filter(|&j| j != i) {
                        let (was, game) = (u32_le(&w[0].riders[i], 0x48 + 0x20 * j) as i32, u32_le(&w[1].riders[i], 0x48 + 0x20 * j) as i32);
                        if riders[i].rec[j].bump != was || game != was {
                            bumps += 1;
                            bumps_right += (riders[i].rec[j].bump == game) as usize;
                            if riders[i].rec[j].bump != game && std::env::var("TRICKY_DBG").is_ok() {
                                eprintln!("DBG bump tick {tick} {i}->{j}: was {was} game {game} ours {} motions {} {} -> {} {}", riders[i].rec[j].bump, u32_le(&w[0].riders[i], 0x424), u32_le(&w[0].riders[j], 0x424), u32_le(&w[1].riders[i], 0x424), u32_le(&w[1].riders[j], 0x424));
                            }
                        }
                    }
                }
            }
        }
        for i in 0..n {
            let Some(p) = &prep[i] else { continue };
            let b = &w[1].riders[i];
            let was_pushed = frame_pushes[i] > 0;
            pushed += was_pushed as usize;
            for (k, g) in got[i].iter().enumerate() {
                let mut all = true;
                for &f in &written {
                    let ok = g[f] == p.want[f];
                    count[k][f] += ok as usize;
                    all &= ok;
                }
                whole[k] += all as usize;
                ctl_right += (k == runners.len() && all && p.ctl.is_some()) as usize;
                pushed_whole[k] += (all && was_pushed) as usize;
                if k == runners.len() && !all && std::env::var("TRICKY_DBG2").is_ok() {
                    let bad: Vec<&str> = written.iter().filter(|&&f| g[f] != p.want[f]).map(|&f| names[f]).collect();
                    let a = &w[0].riders[i];
                    eprintln!("DBG2 tick {} rider {i}: state {:x}->{:x} ours {:x} clip {:x}->{:x} bad {bad:?}", w[1].tick, u32_le(a, 0x428), u32_le(b, 0x428), g[22][0], u32_le(&w[0].extra[ka].1[i], 8), u32_le(&w[1].extra[ka].1[i], 8));
                }
                if k == runners.len() && was_pushed {
                    let hit = (0..n).any(|j| u32_le(b, 0x48 + 0x20 * j) == w[1].tick);
                    pushed_pos[hit as usize][(g[0] == p.want[0]) as usize] += 1;
                    hit_vel += (hit && g[1] == p.want[1]) as usize;
                    if hit && g[1] != p.want[1] && std::env::var("TRICKY_DBG").is_ok() {
                        let ulps: Vec<i64> = (0..3).map(|c| g[1][c] as i32 as i64 - p.want[1][c] as i32 as i64).collect();
                        eprintln!("DBG hitvel tick {} rider {i}: ulps {ulps:?} pos ok {} pushes {} hits(ours) {:?}", w[1].tick, g[0] == p.want[0], frame_pushes[i], frame_hits[i]);
                    }
                }
            }
        }
    }
    eprintln!("{path}: ground ticks (motion 2 at both samples): {pairs} compared ({capped} at the speed limit); left out {other} whose motion object's +0xc is not the rider, {skew} not one update apart, {torn} read inside a tick (it or the sample before)");
    eprintln!("  controls replayed by state: {ctl_count:x?}, every field right under fadd=g1: {ctl_right}; ours from the runner's rider the same as the runner's (per runner way): {ctl_same:?}, quaternion the game's: ours {ctl_ours_game:?}, the runner's {ctl_runner_game:?}; left out: {ctl_turn} fakie cruise on its turn clip, {ctl_none} without the windows");
    if kc.is_some() {
        eprintln!("  frames left out because a sample was read before every rider had ticked: {mid}");
    }
    if pushes {
        eprintln!("  pushed apart by another rider in the frame (order {order:?}): {pushed}; every field right: {pushed_whole:?}");
        eprintln!("  of those, under fadd=g1: not hit {} (position right {}), hit too {} (position right {}, velocity right {hit_vel})", pushed_pos[0][0] + pushed_pos[0][1], pushed_pos[0][1], pushed_pos[1][0] + pushed_pos[1][1], pushed_pos[1][1]);
        eprintln!("  bump records (+0x40 + 0x20·j +8) changed this frame by the game or the replay: {bumps}, the replay's equal to the game's: {bumps_right}");
    }
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
    /// the jump went off a steep lip
    steep: bool,
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
        let (v, steep) = jump_impulse(min, &s);
        self.r.vel = v;
        self.steep |= steep;
    }
    fn takeoff_spin_rates(&mut self, _: &mut TakeOffRider) {
        self.off.spin_rates += 1;
    }
    fn set_state(&mut self, _: &mut TakeOffRider, state: u32) {
        self.off.state.push(state);
    }
}

/// `JumpControl_Update` (`control::jump`) on the ground replay's rider: its calls are [`OurTakeOff`]'s, the rider's
/// crouch (+0x208) and surface its own; the steer target and the reset check only counted (not compared here).
struct OurJump<'a, 'b> {
    t: TakeOffRider,
    env: &'a mut OurTakeOff<'b>,
    steer: usize,
}
impl ControlEnv for OurJump<'_, '_> {
    fn jump_impulse(&mut self, _: &mut control::Rider, min: f32) {
        self.env.jump_impulse(&mut self.t, min);
    }
    fn set_motion(&mut self, _: &mut control::Rider, m: u32) {
        self.env.set_motion(&mut self.t, m);
    }
    fn takeoff_spin_rates(&mut self, _: &mut control::Rider) {
        self.env.takeoff_spin_rates(&mut self.t);
    }
    fn set_steer_target(&mut self, _: &mut control::Rider, _: f32) {
        self.steer += 1;
    }
    fn request_reset(&mut self, _: &mut control::Rider) {}
    fn clip(&mut self, _: u32) -> i32 { unreachable!() }
    fn clip_type(&mut self, _: u32) -> i32 { unreachable!() }
    fn clip_at_end(&mut self, _: u32) -> bool { unreachable!() }
    fn layer_phase(&mut self, _: u32) -> f32 { unreachable!() }
    fn play_clip(&mut self, _: &mut control::Rider, _: i32) { unreachable!() }
    fn set_state(&mut self, _: &mut control::Rider, _: u32) { unreachable!() }
    fn set_crouch_target(&mut self, _: &mut control::Rider, _: f32) { unreachable!() }
    fn set_brake_crouch_input(&mut self, _: &mut control::Rider, _: f32, _: f32) { unreachable!() }
    fn set_brake_target(&mut self, _: &mut control::Rider, _: f32) { unreachable!() }
    fn update_boost(&mut self, _: &mut control::Rider, _: u32, _: u32) { unreachable!() }
    fn update_lean_anim(&mut self, _: &mut control::Rider, _: f32, _: f32) { unreachable!() }
    fn rotate_about_axis(&mut self, _: &mut control::Rider, _: f32) { unreachable!() }
    fn play_celebrate(&mut self, _: &mut control::Rider) { unreachable!() }
    fn play_disappointed(&mut self, _: &mut control::Rider) { unreachable!() }
    fn play_wind_up(&mut self, _: &mut control::Rider) { unreachable!() }
    fn play_takeoff(&mut self, _: &mut control::Rider) { unreachable!() }
    fn ai_taunt_check(&mut self, _: &mut control::Rider) { unreachable!() }
    fn random_flourish(&mut self, _: &mut control::Rider) { unreachable!() }
    fn stop(&mut self, _: &mut control::Rider) { unreachable!() }
    fn race_riders(&mut self) -> i32 { unreachable!() }
    fn race_state(&mut self) -> i32 { unreachable!() }
    fn cam_start(&mut self, _: i32, _: &str) { unreachable!() }
    fn run_control(&mut self, _: &mut control::Rider, _: u32) { unreachable!() }
}

/// The spin state a take-off from the jump control enters, from the recorded tick: `Takeoff_SetSpinRates` 0x126e30's
/// rates into the object (+0x2c, +0x30; +0x58 0), what `SpinControl_Enter` 0x100598 reads of the rider, and the clip
/// type it asks (0xb, mode 0, when the end sample's object is in mode 0). The prediction `Boarder_SetMotion` made this
/// tick is the end sample's (`pb`): the landing slope and its kind (+0x57e0, +0x57f0), the time to touch down (+0x57b8).
struct SpinIn {
    rates: (f32, f32),
    enter: SpinEnterRider,
    clip_type: u32,
    slope_kind: u32,
    slope: V4,
    class: u32,
    rider: SpinRider,
}

fn spin_in(a: &[u8], b: &[u8], st: &[u8], na: &[u8], pb: &[u8], mode_after: Option<u32>, x208: f32) -> SpinIn {
    // STANDIN: Takeoff_Setup 0x12bee0 (called first) is not replayed; the rates from the sample before the tick
    let t = takeoff_spin_rates(&TakeoffState {
        clip: u32_le(na, 8) as i32 as i64,
        x208,
        tricks_stat: st[0x1a],
        wind_spin: f32_at(a, 0x220),
        wind_flip: f32_at(a, 0x22c),
        class: u32_le(a, 0x420),
        fakie: u32_le(a, 0x1b4) != 0,
    });
    SpinIn {
        rates: (t.spin, t.flip),
        enter: SpinEnterRider {
            x138: f32_at(a, 0x138),
            switch: u32_le(a, 0x1b4),
            stance: u32_le(st, 0x40),
            kind: u32_le(a, 0x1dc),
            up_z: 0.0,
            wind_spin: f32_at(a, 0x274),
            wind_flip: f32_at(a, 0x278),
            x218: 0.0,
            x21c: 0.0,
            x268: [0.0; 3],
        },
        clip_type: if mode_after == Some(0) { 0xb } else { 0 },
        slope_kind: u32_le(pb, 0x40),
        slope: vec4(pb, 0x30),
        class: u32_le(a, 0x420),
        rider: SpinRider {
            // the controller virtual (`GetControls` 0x151250 / 0x1384b8) moves the time scale between the motion and
            // the control (`Boarder_SetTimeScaleRateLimited` 0x11cff0): the control sees the second sample's (F4b3m)
            timescale: f32_at(b, 0x12c),
            land: f32_at(pb, 8),
            spin: f32_at(a, 0x1d0),
            flip: f32_at(a, 0x1d4),
            tricks: st[0x1a],
            stat10: st[0x10],
            mirror: u32_le(st, 0x40),
            switch: u32_le(a, 0x1b4),
            wind_spin: f32_at(a, 0x274),
            wind_flip: f32_at(a, 0x278),
            x1f0: f32_at(a, 0x1f0),
            x1f4: f32_at(a, 0x1f4),
            x1f8: f32_at(a, 0x1f8),
            class: u32_le(a, 0x420),
            hp_face: vec4(pb, 0x20),
            hp_axis: vec4(pb, 0x30),
            x5800: f32_at(pb, 0x50),
            ..Default::default()
        },
    }
}

/// Ours: `SpinControl_Enter` (`spin::spin_enter`) as `Takeoff_SetSpinRates` enters state 0x10, then the control's
/// update this tick, `SpinState_Update` (`spin::spin_update`, pad 0, the animation's answers 0), on the rider as the
/// take-off left it; its attitude (`Boarder_UpdateOrientation`) when the update turned it.
fn spin_on_takeoff_tick(r: &mut Rider, s: &SpinIn) -> SpinObj {
    let mut st = SpinObj { w_spin: s.rates.0, w_flip: s.rates.1, ..Default::default() };
    let mut e = s.enter;
    e.up_z = r.board_up[2];
    let al = AlignRider { slope_kind: s.slope_kind, slope: s.slope, fwd: r.p.fwd, quat: r.quat, class: s.class };
    spin_enter(&mut st, &mut e, &al, s.clip_type);
    let mut sr = SpinRider { quat: r.quat, vel: r.vel, pos: r.p.pos, ..s.rider };
    sr.wind_spin = e.wind_spin;
    sr.wind_flip = e.wind_flip;
    let calls = spin_update(0, &mut st, &mut sr, &Anim::default());
    if calls.contains(&SpinCall::Orientation) {
        let mut o = Orient { q: sr.quat, fwd: r.p.fwd, side: r.board_side, up: r.board_up, yaw: r.yaw };
        update_orientation(&mut o);
        (r.quat, r.p.fwd, r.board_side, r.board_up, r.yaw) = (o.q, o.fwd, o.side, o.up, o.yaw);
    }
    r.p.pos = sr.pos;
    st
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
/// landing (it keeps them), the time to the top (+0x48) from the inputs each way saw, and for a landing whether the
/// game's landing point lies on the flight from those inputs (`predict_like`).
/// With the bodies recorded (window 0x700) the frame's rider collisions are replayed around the rider (`FrameRun`,
/// board row F4h4): the riders that tick before it push and hit it before its motion, the rest after its take-off.
/// TRICKY_DBG lists the ticks off under `fadd=g1`.
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
    let km6 = win(-0x3d0);
    let ks = win(-0x170);
    let clip_type = Rc::new(Cell::new(0u32));
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
        // the spin state's enter and first update (F4b3f): `SpinControl_CalcLandingAlign` and its maths run, the
        // clip type is the test's, the update's other calls answer 0
        for f in [0x11db00, 0x2508b8, 0x102e80, 0x10c120, 0x250fd0, 0x250d60, 0x250c10, 0x102f80, 0x102f50, 0x2510f8, 0x129008] {
            w.r.allow(f);
        }
        for f in [0x155e00, 0x155e68, 0x102800, 0x127918, 0x15fcf8, 0x15eef8, 0x127d08] {
            w.r.stub(f, |_, _| {});
        }
        for f in [0x1272e8, 0x102158] {
            w.r.stub(f, |c, _| c.set_gpr(2, 0));
        }
        let ct = clip_type.clone();
        w.r.stub(0x15eec0, move |c, _| c.set_gpr(2, ct.get() as u64));
        runners.push((rules, w, off, ctl));
    }
    let one_step = |a: &[u8], b: &[u8]| {
        let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
        let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
        e > 1.0 && d > 0.7 * e && d < 1.4 * e
    };
    let names = ["pos", "vel", "fwd", "board_up", "plane_fwd", "side", "quat", "drag", "height", "bob", "lean", "yaw", "the way up", "prediction inputs", "time to the top", "landing on the flight"];
    let ways = 1 + runners.len();
    let mut count = vec![vec![0usize; names.len()]; ways];
    let mut of = vec![0usize; names.len()];
    let mut whole = vec![0usize; ways];
    let (mut steep, mut steep_whole) = (0, vec![0usize; ways]);
    // the frame's rider collisions, as in `ground_ticks_against_the_running_game` (bodies recorded; TRICKY_NOPUSH off)
    let pushes = w0_len(&ticks) >= 0x6e0 && std::env::var("TRICKY_NOPUSH").is_err();
    let order: Vec<usize> = std::env::var("TRICKY_ORDER").map(|o| o.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or((0..n).rev().collect());
    let (mut pushed, mut pushed_whole) = (0, vec![0usize; ways]);
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
            let k = match km6 {
                Some(k6) if u32_le(&w[0].extra[km].1[i], 0xc) != addr[i] => k6,
                _ => km,
            };
            let (ma, mb) = (&w[0].extra[k].1[i], &w[1].extra[k].1[i]);
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
            // the riders that tick before it in the frame push and hit it before its motion (F4h4)
            let run = pushes.then(|| {
                let (run, pos, vel) = FrameRun::before(&w[0].riders, &w[1].riders, &stats, &order, w[1].tick as i32, i);
                (rider.p.pos, rider.vel) = (pos, vel);
                run
            });
            // the jump control fires on this tick: state 0xa and the crouch unwound to 0 before the motion
            let jumps = rider.state == 0xa && rider.crouch == 0.0;

            // ours
            let mut mine = rider;
            let mut e = E { log: Vec::new(), hit, wall: false, meter: 0.0 };
            update(&mut mine, &SURFACE_TABLE, &mut e);
            let up_by_motion = e.log.contains(&Ev::TakeOff);
            let mut t = TakeOffRider { state: mine.state, crouch: crouch0, x208: mine.crouch };
            // the spin state entered by a take-off from the jump control, and (motion take-offs) its update in the
            // control this tick (F4b3f)
            let spun = up_by_motion && rider.state == 0xa && std::env::var("TRICKY_NOSPIN").is_err();
            let sin = spin_in(a, b, st, na, pb, ks.map(|k| u32_le(&w[1].extra[k].1[i], 8)), mine.crouch);
            let mut env = OurTakeOff { r: &mut mine, motion: 2, jump_stat: st[0x17], off: Off::default(), steep: false };
            if up_by_motion {
                take_off(&mut t, &mut env);
            } else if jumps {
                env.r.vel = speed_limit(env.r.vel, speed_limit_of(a, b, |x, y| x - y));
                let mut cr = control::Rider { crouch: env.r.crouch, surface: env.r.p.surface, ..Default::default() };
                let mut j = OurJump { t, env: &mut env, steer: 0 };
                control::jump(&mut j, 0, &mut cr);
                assert_eq!(j.steer, 0, "the jump control fires only with the crouch let go");
            }
            let ours_off = env.off;
            let lip = env.steep;
            let our_spin = spun.then(|| spin_on_takeoff_tick(&mut mine, &sin));
            steep += lip as usize;
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
                    if spun {
                        let (m, rd) = (&mut g.r.mem, g.rd);
                        let so = g.handler + 0x740;
                        for k in 0..0x70 / 4 {
                            m.write_u32(so + 4 * k, 0);
                        }
                        m.write_u32(so + 0x70, rd);
                        m.write_f32(so + 0x2c, sin.rates.0);
                        m.write_f32(so + 0x30, sin.rates.1);
                        m.write(rd + 0x57b0, pb).unwrap();
                        m.write(g.stats, st).unwrap();
                        for o in [0x138, 0x1d0, 0x1d4, 0x1f0, 0x1f4, 0x1f8, 0x274, 0x278, 0x420] {
                            m.write_u32(rd + o, u32_le(a, o as usize));
                        }
                        m.write_f32(rd + 0x12c, sin.rider.timescale);
                        clip_type.set(sin.clip_type);
                        g.r.cpu.set_gpr(4, so as u64);
                        g.r.call(0x100598).unwrap_or_else(|e| panic!("tick {} rider {i}: enter {e:?}", w[0].tick));
                        let pad = *ctl + 8;
                        g.r.mem.write_u32(pad, 0);
                        g.r.cpu.set_gpr(4, so as u64);
                        g.r.cpu.set_gpr(5, pad as u64);
                        g.r.call(0x100908).unwrap_or_else(|e| panic!("tick {} rider {i}: spin {e:?}", w[0].tick));
                    }
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
            // the frame's rider collisions after the take-off (F4h4)
            let mut was_pushed = false;
            if let Some(run) = &run {
                for (r, _) in got.iter_mut() {
                    let (pos, vel, np, _) = run.clone().after(&w[0].riders, &w[1].riders, &stats, &order, r, u32_le(b, 0x424));
                    (r.p.pos, r.vel) = (pos, vel);
                    was_pushed |= np > 0;
                }
            }
            pushed += was_pushed as usize;
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
                let r = &*r;
                let mut ok = [r.p.pos, r.vel, r.p.fwd, r.board_up, r.p.plane_fwd, r.p.side, r.quat].map(b4).iter().zip(&want).map(|(x, y)| x == y).collect::<Vec<_>>();
                ok.extend([r.drag, r.p.height, r.bob, r.lean, r.yaw].iter().zip(&want_f).map(|(x, y)| x.to_bits() == *y));
                ok.push(off.motion.len() == 1 && off.motion[0].0 == 1);
                let mut used = vec![true; names.len()];
                used[13] = false;
                used[14] = false;
                used[15] = false;
                if made && off.motion.len() == 1 {
                    let [p, v, nn] = off.motion[0].1;
                    let no_landing = u32_le(pb, 0x40) == 0xffff_ffff;
                    used[13] = no_landing;
                    ok.push(no_landing && [p, v, nn].map(b4) == [vec4(pb, 0x10), vec4(pb, 0x20), vec4(pb, 0x30)].map(b4));
                    used[14] = true;
                    let pr = predict_like(p, v, nn, pb);
                    ok.push(pr.t_top.to_bits() == u32_le(pb, 0x48));
                    // a landing: the game's landing point on the flight from what this way's `Boarder_SetMotion` saw
                    used[15] = !no_landing;
                    ok.push(!no_landing && pr.word == u32_le(pb, 0x40) && pr.point == vec4(pb, 0x10));
                } else {
                    ok.extend([false, false, false]);
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
                steep_whole[k] += (all && lip) as usize;
                pushed_whole[k] += (all && was_pushed) as usize;
                if !all && k == 2 && std::env::var("TRICKY_DBG").is_ok() {
                    let near = (0..n).filter(|&j| j != i).map(|j| {
                        let (p, q) = (vec4(b, 0x140), vec4(&w[1].riders[j], 0x140));
                        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
                    }).fold(f32::MAX, f32::min);
                    let bad: Vec<&str> = (0..names.len()).filter(|&f| used[f] && !ok[f]).map(|f| names[f]).collect();
                    let d = |x: V4, y: V4| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
                    eprintln!("  tick {} rider {i}: {} state {:x}, nearest rider {near:.0} cm, steep {lip}, pushed {was_pushed}: {bad:?} pos off {:?} vel off {:?} (kind {:x} surface {:x})", w[1].tick,
                        if up_by_motion { "motion" } else if jumps { "jump control" } else { "neither" }, rider.state,
                        d(r.p.pos, vec4(b, 0x140)), d(r.vel, vec4(b, 0x150)), u32_le(b, 0x2e0), u32_le(b, 0x290));
                    if std::env::var("TRICKY_DBG").as_deref() == Ok("2") {
                        eprintln!("    fwd {:?} want {:?} (before {:?})\n    up {:?} want {:?} (before {:?})\n    yaw {} want {} (before {}) crouch {} {} -> {} steer {} {} spin {:?}", r.p.fwd, vec4(b, 0x180), vec4(a, 0x180), r.board_up, vec4(b, 0x1a0), vec4(a, 0x1a0), r.yaw, f32_at(b, 0x1b0), f32_at(a, 0x1b0), f32_at(a, 0x1c8), f32_at(a, 0x208), f32_at(b, 0x208), f32_at(a, 0x214), f32_at(b, 0x214), (0..8).map(|k| f32_at(b, 0x5d0 + 4 * k)).collect::<Vec<_>>());
                        if let Some(o) = our_spin {
                            let z = vec![0u8; 0x74];
                            let g = ks.map_or(&z, |k| &w[1].extra[k].1[i]);
                            eprintln!("    spin ours mode {} rates {} {} align {} axis {:?} | game mode {} rates {} {} align {} axis {:?}\n    land {} timescale {} slope kind {} slope {:?} class {} clip type {:x}",
                                o.mode, o.w_spin, o.w_flip, o.align, o.align_axis, u32_le(g, 8), f32_at(g, 0x2c), f32_at(g, 0x30), f32_at(g, 0x34), vec4(g, 0x40),
                                sin.rider.land, sin.rider.timescale, sin.slope_kind, sin.slope, sin.class, sin.clip_type);
                        }
                    }
                }
            }
        }
    }
    eprintln!("{path}: pushed apart in the frame after the take-off (replayed: {pushes}): {pushed}, whole for ours, the runner's: {pushed_whole:?}");
    eprintln!("{path}: {steep} take-offs off a steep lip (`Jump_ApplyImpulse`'s second branch), whole for ours, the runner's: {steep_whole:?}");
    eprintln!("{path}: take-off ticks (motion 2 → 1): {pairs} compared ({by_motion} left the snow in the motion, {by_jump} by the jump control, {neither} neither; {no_pred} with no new prediction read, {torn} read while it was being made); left out {other} whose motion object's +0xc is not the rider, {skew} not one update apart");
    for (k, way) in ["ours".to_string()].into_iter().chain(runners.iter().map(|(r, ..)| format!("the game's in the runner, {r}"))).enumerate() {
        let line: Vec<String> = (0..names.len()).map(|f| format!("{} {} of {}", names[f], count[k][f], of[f])).collect();
        eprintln!("  {way}: all {}; {}", whole[k], line.join(", "));
    }
}

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
use std::collections::HashMap;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::ground_motion::{update, CrashRecord, Env, Rider, Stats, WALL_MASK};
use tricky_game::air::{predict_at_takeoff, Predict, RayHit};
use tricky_game::collbody::{overlap, CollBody};
use tricky_game::hit::{rider_hit, HitCall, HitRider};
use tricky_game::ridercol::{rider_collisions, ColEnv, ColRec, ColRider};
use tricky_game::jump::{jump_impulse, JumpState};
use tricky_game::orient::{take_off, TakeOffEnv, TakeOffRider};
use tricky_game::probe::{self, Hit};
use tricky_game::rider::ground_tick::{lean_targets, lean_up};
use tricky_game::surface::SURFACE_TABLE;
use tricky_game::animctl;
use tricky_game::control::{self, Bump, ControlEnv, LandNormal};
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::pose::{board_frame, frame_where, PoseIn};
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
    /// also change the state as `Boarder_SetState` 0x11f608 does (`ground_tick::set_state`: the exit and enter
    /// routines; F4h5h), not only log it
    states: bool,
    /// the course's walls (F4h5j): `Boarder_WallCollide` 0x126250 (`walls::wall_collide`) answered from them, else
    /// `wall`
    walls: Option<Walls>,
}

/// The course's collision (TRICKY_DISC, TRICKY_COURSE: the course's file name, `mesa` for `data/models/mesa.big`)
/// and the rider's body as the first sample holds it (F4h5j).
#[derive(Clone)]
struct Walls {
    course: std::sync::Arc<tricky_game::course_ground::CourseGround>,
    body: CollBody,
    /// the frame's up (+0x4ac0, [`frame_of`]), which `Boarder_WallImpactCheck` 0x126540 measures the knock against
    up: V4,
    /// the knocked objects among the rider's pairs (object index, body), as recorded (`pine.py record ... rollers`,
    /// [`rollers_of`]; F4b3k1): what the rollers' loop of `Boarder_InstanceCollisions` 0x125088 kicks
    rollers: Vec<(u32, tricky_game::roller::RollerBody)>,
    /// the rider's speed stat (`Boarder_GetSpeedStat` 0x11d7b0), which a kick's impulse divides by
    stat: f32,
}

/// The course the recording was made on, from the player's disc: TRICKY_DISC (the image) and TRICKY_COURSE (its
/// models file's name) or the recording's `course N`; none without them (the walls then answer nothing).
fn course(path: &str) -> Option<std::sync::Arc<tricky_game::course_ground::CourseGround>> {
    let (pbd, ltg, ssf) = course_files(path)?;
    Some(std::sync::Arc::new(tricky_game::course_ground::CourseGround::new(&pbd, &ltg, &ssf)?))
}

/// The course's `.pbd`, `.ltg` and `.ssf` from the player's disc (as [`course`]).
fn course_files(path: &str) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let name = match std::env::var("TRICKY_COURSE") {
        Ok(name) => name,
        Err(_) => course_name(recording::course(path)?)?.to_string(),
    };
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount(&format!("data/models/{name}.big")).ok()?;
    let mut get = |ext: &str| files.load(&format!("|data/models/{name}.{ext}")).ok().flatten();
    Some((get("pbd")?, get("ltg")?, get("ssf")?))
}

/// The models file of course id `id` (0x32de88; the recorder writes it in the header, `course N`). Inferred: the
/// models files' names in the order of `Camera_GetCourseCmlName` 0x178330's courses (0 garibald, 1 snowdrea, 2
/// elysium, 3 mesablan: mesa proven by rec19's walls, 4 merqury, 5 aloha, 6 pipedrea, 7 untracke, 8 tokyo, 10 trick,
/// 11 alaska); 9 (pipedrea again for the cameras) left out.
fn course_name(id: u32) -> Option<&'static str> {
    ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "", "trick", "alaska"].get(id as usize).copied().filter(|n| !n.is_empty())
}

/// The course's objects carry the flags the game had over the race (`pine.py record ... flags`, F4b3k4): the static
/// touch objects are then cast by default (`E::instance_collisions`)
static FLAGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The walls for a rider whose first sample is `a` (recordings with the bodies only).
fn walls_of(course: &Option<std::sync::Arc<tricky_game::course_ground::CourseGround>>, a: &[u8], st: &[u8]) -> Option<Walls> {
    let course = course.clone().filter(|_| 0x700 <= a.len())?;
    Some(Walls { course, body: body_of(a), up: frame_of(a, st, None)[2], rollers: Vec::new(), stat: 0.0 })
}

/// Rider `me`'s knocked objects at sample `t` (window `rollers`, F4b3k3) as `roller::RollerBody`s, with the
/// course's instance base `base` (header `instances`): the body's fields from node + 0x30 on (`RecRoller.body[k]` =
/// node + 0x30 + k; the layout as `tests/roller.rs` reads it), the sphere tree the object's group's
/// (`World_GetInstanceSphTree` 0x260ca0: the physics record's group) whose radius makes the recorded box (+0x2a0 /
/// +0x2b0: its centre ∓ the radius; a tree hit by a contact may be another of the group's), placed from the body
/// ([`tricky_game::roller::RollerBody::place_tree`]). TRICKY_ROLLDBG lists them with the gap between the recorded box
/// and ours.
fn rollers_of(t: &recording::Tick, me: usize, course: &tricky_game::course_ground::CourseGround, base: u32) -> Vec<(u32, tricky_game::roller::RollerBody)> {
    use tricky_game::rigidbody::RigidBody;
    use tricky_game::roller::RollerBody;
    let Some(list) = recording::rollers(t, me) else { return Vec::new() };
    let mut out = Vec::new();
    for r in list {
        let o = r.instance.wrapping_sub(base) / 0x100;
        let b = &r.body;
        let f = |k: usize| f32_at(b, k);
        let v = |k: usize| vec4(b, k);
        let radius = (r.max[0] - r.min[0]) * 0.5;
        let Some(tree) = course.with_world(|w| {
            let g = w.objects.get(o as usize)?.group;
            let trees = w.trees.get(usize::try_from(g).ok()?)?;
            trees.iter().min_by(|x, y| {
                let d = |t: &tricky_game::sphtree::SphereTree| (t.levels.first().map_or(0.0, |l| l.0) - radius).abs();
                d(x).total_cmp(&d(y))
            }).cloned()
        }) else { continue };
        let mut body = RollerBody {
            rb: RigidBody { inv_mass: f(8), inv_inertia: std::array::from_fn(|k| f(0xc + 4 * k)), pos: v(0x30), q: v(0x40), p: v(0x50), l: v(0x60), v: v(0x70), w: v(0x80) },
            energy: f(0x98),
            mass: f(0x9c),
            xd0: f(0xa0),
            xd4: f(0xa4),
            e: f(0xa8),
            mu: f(0xac),
            bounce_speed: f(0xb0),
            awake: f(0xb4),
            knock: f(0xb8),
            xf0: v(0xc0),
            body_inertia: std::array::from_fn(|k| f(0xd0 + 4 * k)),
            com: v(0x100),
            asleep: u32_le(b, 0x110),
            tree,
            lo: r.min,
            hi: r.max,
        };
        body.place_tree();
        if std::env::var("TRICKY_ROLLDBG").is_ok() {
            eprintln!("ROLLDBG tick {} rider {me} object {o} node {:#x} asleep {} pos {:?} box {:?} {:?} ours {:?} {:?}", t.tick, r.node, body.asleep, body.rb.pos, r.min, r.max, body.lo, body.hi);
        }
        (body.lo, body.hi) = (r.min, r.max);
        out.push((o, body));
    }
    out
}

/// `World_RayCast` 0x25aff8 with `cWorldBoarder`(rider+0x470, 1) and the ground normal set, on the course: t and the
/// record. TRICKY_WALL_SKIP: instances left out (comma-separated; mesa 1444: hit by ours, not by the game, row
/// F4h5l); TRICKY_WALLDBG lists the hits.
fn wall_cast(course: &tricky_game::course_ground::CourseGround, body: &CollBody, ground: V4) -> (f32, tricky_game::walls::CastRecord) {
    let mut q = tricky_game::worldboarder::WorldBoarder::new(body, 1);
    q.reject = Some(ground);
    let skip = wall_skip();
    let (t, h) = course.ray_cast(&mut q, false, None, &|i| skip.contains(&i));
    if 0.0 < t && std::env::var("TRICKY_WALLDBG").is_ok() {
        eprintln!("WALLDBG t {t} at {:?} n {:?} object {:?} body centre {:?} r {}", h.map(|h| h.pos), h.map(|h| h.normal), h.and_then(|h| h.object), body.center, body.radius);
    }
    (t, h.map_or(Default::default(), |h| tricky_game::walls::CastRecord { pos: h.pos, normal: h.normal, object: h.object.map_or(0, |o| o as u32 + 1) }))
}

/// `World_QueryInstances` 0x25b878 with `cWorldBoarder`(the body, 1), front faces only (`Boarder_InstanceCollisions`
/// sets the shape's +0x2f0), on the course: t, the record (object = index + 1) and the object, as
/// `ground_tick::touch_query` (F4b3k1). TRICKY_INSTDBG lists the hits.
fn instance_query(course: &tricky_game::course_ground::CourseGround, body: &CollBody) -> (f32, tricky_game::walls::CastRecord, tricky_game::walls::Touched) {
    let mut q = tricky_game::worldboarder::WorldBoarder::new(body, 1);
    q.front_only = true;
    let skip = wall_skip();
    let (t, h) = course.query_instances(&mut q, false, &|i| skip.contains(&i));
    let Some(h) = h else { return (t, Default::default(), Default::default()) };
    let o = h.object.unwrap_or(0);
    let phys = course.ssf.object_physics.get(o).and_then(|p| course.ssf.physics.get(*p as usize));
    let touched = tricky_game::walls::Touched { id: o as u32, flags: course.object_flags(o), mass: phys.map_or(0.0, |p| p.mass()), bounce: phys.map_or(0.0, |p| p.bounce()), controller: None };
    if std::env::var("TRICKY_INSTDBG").is_ok() {
        eprintln!("INSTDBG t {t} object {o} flags {:#x} mass {} bounce {} at {:?} n {:?}", touched.flags, touched.mass, touched.bounce, h.pos, h.normal);
    }
    (t, tricky_game::walls::CastRecord { pos: h.pos, normal: h.normal, object: o as u32 + 1 }, touched)
}

/// TRICKY_WALL_SKIP: the instances the walls leave out.
fn wall_skip() -> Vec<u32> {
    std::env::var("TRICKY_WALL_SKIP").unwrap_or_default().split(',').filter_map(|x| x.parse().ok()).collect()
}

/// What `Boarder_WallCollide` 0x126250 calls (`walls::WallEnv`), on the ground motion's rider. The body is the
/// one `cPostAI::vf1` posed at the end of the last frame (the first sample's): the motion moves only the position
/// (proven by the cast: rec19 2294 and 2295 r4, the depths the recorded pushes over 1.1).
struct WallTest<'a> {
    g: &'a mut Rider,
    course: &'a tricky_game::course_ground::CourseGround,
    body: CollBody,
    /// the rider's speed stat (a roller's kick, F4b3k1)
    stat: f32,
}
impl tricky_game::walls::WallEnv for WallTest<'_> {
    /// `World_RayCast` 0x25aff8 with `cWorldBoarder`(rider+0x470, 1), its ground normal set
    fn cast_walls(&mut self, r: &tricky_game::walls::WallRider) -> (f32, tricky_game::walls::CastRecord) {
        wall_cast(self.course, &self.body, r.ground)
    }
    /// `World_QueryInstances` 0x25b878 with `cWorldBoarder`(rider+0x470, 1), front faces only (F4b3k1): t, the
    /// record and the object (its flags, its physics record's mass and bounce). STANDIN: no controller, as
    /// `ground_tick::touch_query` (row E7b2)
    fn query_objects(&mut self, _: &tricky_game::walls::WallRider) -> (f32, tricky_game::walls::CastRecord, tricky_game::walls::Touched) {
        instance_query(self.course, &self.body)
    }
    /// +0x2e4 and +0x2f0, which the test does not compare
    fn accumulate_impact(&mut self, _: &mut tricky_game::walls::WallRider, _: V4) {}
    /// `Boarder_Translate` 0x1267a0: the body moves with the push
    fn translate(&mut self, _: &mut tricky_game::walls::WallRider, by: V4) {
        self.body.translate(by);
    }
    /// `Boarder_TurnFromWall` 0x1257a0 (`orient::turn_from_wall`)
    fn turn_from_wall(&mut self, _: &mut tricky_game::walls::WallRider, n: V4) {
        let g = &mut *self.g;
        let mut o = Orient { q: g.quat, fwd: g.p.fwd, side: g.board_side, up: g.board_up, yaw: g.yaw };
        tricky_game::orient::turn_from_wall(&mut o, n);
        (g.quat, g.p.fwd, g.board_side, g.board_up, g.yaw) = (o.q, o.fwd, o.side, o.up, o.yaw);
    }
    /// not replayed (a wall wipe-out ends the ground motion's frame as a crash; none in the recordings so far)
    fn wipeout(&mut self, _: &mut tricky_game::walls::WallRider, _: &tricky_game::walls::Contact) {}
    fn score_crash(&mut self, _: &mut tricky_game::walls::WallRider) -> f32 {
        0.0
    }
    fn add_meter(&mut self, _: &mut tricky_game::walls::WallRider, _: f32) {}
    /// `Boarder_Stumble` 0x124dd0: from the cruise (3) or the prewind (0xe), `Boarder_SetState`(2) (its clip, speech
    /// and the bump control's random +0x618 are not compared)
    fn stumble(&mut self, _: &mut tricky_game::walls::WallRider, _: V4, _: u32) {
        if matches!(self.g.state, 3 | 0xe) {
            tricky_game::rider::ground_tick::set_state(self.g, 2);
        }
    }
    /// `Boarder_GetSpeedStat` 0x11d7b0, for a roller's kick (F4b3k1)
    fn speed_stat(&mut self, _: &tricky_game::walls::WallRider) -> f32 {
        self.stat
    }
    fn knock(&mut self, _: &mut tricky_game::walls::WallRider, _: f32) {}
    fn contact(&mut self, _: u32, _: &mut tricky_game::walls::WallRider, _: &tricky_game::walls::Contact, _: &tricky_game::walls::CastRecord) {}
    fn sfx_object(&mut self, _: &tricky_game::walls::WallRider, _: u32) {}
    fn sfx_wall(&mut self, _: &tricky_game::walls::WallRider) {}
}
impl Env for E {
    fn set_state(&mut self, r: &mut Rider, state: u32) {
        self.log.push(Ev::State(state));
        if self.states {
            tricky_game::rider::ground_tick::set_state(r, state);
        }
    }
    fn wall_collide(&mut self, r: &mut Rider) -> bool {
        self.log.push(Ev::Wall(r.mask));
        if let Some(w) = &self.walls {
            let course = w.course.clone();
            let mut body = w.body;
            body.x24 = r.mask as i32;
            let mut wr = tricky_game::walls::WallRider { pos: r.p.pos, vel: r.vel, ground: r.p.normal, knocks: 0.0, mode: 2, state: r.state, up: w.up };
            let mut t = WallTest { g: &mut *r, course: &course, body, stat: w.stat };
            let hit = tricky_game::walls::wall_collide(&mut wr, &mut t);
            // the body as `Boarder_Translate` left it, for the touch objects after (F4b3k1)
            let body = t.body;
            (r.p.pos, r.vel) = (wr.pos, wr.vel);
            if let Some(w) = &mut self.walls {
                w.body = body;
            }
            return hit;
        }
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
    /// `Boarder_InstanceCollisions` 0x125088 (`walls::instance_collisions`, then its rollers' loop): the course's
    /// static touch objects when the recording kept the objects' flags ([`FLAGGED`], F4b3k4; TRICKY_INSTANCES=1:
    /// always, 0: never; without the flags they make it worse: rec22, rec23, the objects the game had made rollers or
    /// `cDeadNode`s are bounced off), then (F4b3k1) a kick for each knocked object among the rider's
    /// pairs (`roller::rider_kick`, the rollers recorded, [`rollers_of`]; TRICKY_NOROLL=1: not). Their contact
    /// scripts and sounds are not run; the keyframed objects among the pairs (types 0x100..0x103) are not either.
    fn instance_collisions(&mut self, r: &mut Rider) {
        self.log.push(Ev::Instances);
        let Some(w) = &mut self.walls else { return };
        let course = w.course.clone();
        let mut body = w.body;
        body.x24 = r.mask as i32;
        let mut rollers = std::mem::take(&mut w.rollers);
        let mut wr = tricky_game::walls::WallRider { pos: r.p.pos, vel: r.vel, ground: r.p.normal, knocks: 0.0, mode: 2, state: r.state, up: w.up };
        let mut t = WallTest { g: &mut *r, course: &course, body, stat: w.stat };
        let statics = match std::env::var("TRICKY_INSTANCES").as_deref() {
            Ok("0") => false,
            Ok(_) => true,
            Err(_) => FLAGGED.load(std::sync::atomic::Ordering::Relaxed),
        };
        if statics {
            tricky_game::walls::instance_collisions(&mut wr, &mut t);
        }
        if std::env::var("TRICKY_NOROLL").is_err() {
            for (o, rb) in &mut rollers {
                let b = t.body;
                tricky_game::roller::rider_kick(rb, *o, &mut wr, 0, &b, &mut t);
            }
        }
        let body = t.body;
        (r.p.pos, r.vel) = (wr.pos, wr.vel);
        w.body = body;
        w.rollers = rollers;
    }
}

/// What a roller kicked by the rider asks of the world (`roller::RollerEnv`): only `Roller_OnHit` 0x13f448's script
/// and sound, which the ground test does not run (the rollers are read again from the next sample).
impl tricky_game::roller::RollerEnv for WallTest<'_> {
    fn ray_cast(&mut self, _: &tricky_game::sphtree::WorldSphTree) -> (f32, V4, V4) {
        unreachable!("not called by a kick")
    }
    fn query_instances(&mut self, _: &tricky_game::sphtree::WorldSphTree) -> (f32, V4, V4) {
        unreachable!("not called by a kick")
    }
    fn move_box(&mut self, _: V4, _: V4) {}
    fn event_script(&self, _: u32, _: u32) -> i32 {
        -1
    }
    fn start_script(&mut self, _: i32, _: u32) {}
    fn sfx(&mut self, _: u32, _: u32) {}
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
    /// the air motion object (+0x20 the rider) and, while `landing` is set, World_RayCast's answer placed 99.9 cm
    /// along the normal from the start of the last line made (`land_runner`)
    air: u32,
    landing: Rc<Cell<bool>>,
    /// the course `World_RayCast`'s stub answers `Boarder_WallCollide`'s cast from (F4h5k; [`W::put_walls`])
    course: Rc<RefCell<Option<std::sync::Arc<tricky_game::course_ground::CourseGround>>>>,
    /// the course built in the runner ([`W::load_course`]): `Boarder_WallCollide`'s cast then runs as the
    /// original, `World_RayCast` 0x25aff8 and the `cWorldBoarder` 0x257810 the wall makes
    real_cast: Rc<Cell<bool>>,
    /// `Boarder_SetState` 0x11f608 runs as the original ([`W::real_states`], or the take-off test's own set-up)
    states_real: bool,
    /// the course built in the runner ([`W::load_course`]), for its keyframed objects' poses ([`Keyframed`])
    rc: Option<runner_course::RunnerCourse>,
    /// its instances' base (0 without it), where `World_QueryInstances`' stub points the object it hit (F4b3k4)
    inst0: Rc<Cell<u32>>,
}

/// `Boarder_WallCollide` 0x126250 stubbed: logged, answering `wall`.
fn stub_wall(r: &mut Runner, log: &Rc<RefCell<Vec<Ev>>>, wall: &Rc<Cell<bool>>, rd: u32) {
    let (l, w) = (log.clone(), wall.clone());
    r.stub(0x126250, move |cpu, mem| {
        l.borrow_mut().push(Ev::Wall(mem.read_u32(rd + 0x494)));
        cpu.set_gpr(2, w.get() as u64);
    });
}

impl W {
    /// The course's walls for the next call (F4h5k): with `walls`, `Boarder_WallCollide` 0x126250 runs as the
    /// original, its `World_RayCast` 0x25aff8 answered by the course's cast of the body written to rider+0x470 (the
    /// first sample's), the frame's up at +0x4ac0; without, the stub answering `wall`.
    fn put_walls(&mut self, walls: Option<&Walls>) {
        let (m, rd) = (&mut self.r.mem, self.rd);
        if let Some(w) = walls {
            let b = &w.body;
            wv(m, rd + 0x480, b.center);
            m.write_f32(rd + 0x490, b.radius);
            m.write_u32(rd + 0x494, b.x24 as u32);
            m.write_u32(rd + 0x498, b.count as u32);
            for (k, (c, r)) in b.spheres.iter().enumerate() {
                wv(m, rd + 0x4a0 + 0x20 * k as u32, *c);
                m.write_f32(rd + 0x4b0 + 0x20 * k as u32, *r);
            }
            wv(m, rd + 0x4ac0, w.up);
            self.r.unstub(0x126250);
            // `Boarder_InstanceCollisions` 0x125088 as the original when the objects' flags are the game's and the
            // runner has the course's instances (F4b3k4); else the stub
            if FLAGGED.load(std::sync::atomic::Ordering::Relaxed) && self.inst0.get() != 0 {
                self.r.unstub(0x125088);
            } else {
                let l = self.log.clone();
                self.r.stub(0x125088, move |_, _| l.borrow_mut().push(Ev::Instances));
            }
        } else {
            stub_wall(&mut self.r, &self.log, &self.wall, rd);
        }
        *self.course.borrow_mut() = walls.map(|w| w.course.clone());
    }

    /// The course built in the runner by the game's loaders (`runner_course`, F4h5m), with the objects' boxes and
    /// flags ours has (TRICKY_WALL_SKIP's left out, as [`wall_cast`]): `Boarder_WallCollide` 0x126250's cast then
    /// runs as the original (`World_RayCast` 0x25aff8, the `cWorldBoarder` 0x257810 and its tests), the patches'
    /// points as `terrain::tessellate` makes them, under `fadd=g1` (the PCSX2 way) toward zero as PCSX2 does.
    fn load_course(&mut self, files: &(Vec<u8>, Vec<u8>, Vec<u8>), course: &tricky_game::course_ground::CourseGround) {
        let (pb, lb, sb) = files;
        let rc = runner_course::load(&mut self.r, pb, lb, sb);
        let pbd = tricky_data::pbd::Pbd::parse(pb).unwrap();
        let coeffs = pbd.patches.iter().map(|r| tricky_data::pbd::PatchRec::parse(r).map_or([[0.0; 4]; 16], |p| p.coeffs)).collect();
        // the PCSX2 way (`fadd=g1`) tessellates as PCSX2 runs the VU0 patch program, toward zero
        let chop = self.r.float_rules.fadd == r5900::ps2float::Round::Guard;
        rc.cast_as_original(&mut self.r, coeffs, chop);
        let skip = wall_skip();
        course.with_world(|w| {
            for (i, o) in w.objects.iter().enumerate() {
                let flags = if skip.contains(&(i as u32)) { o.flags & !0x20 } else { o.flags };
                rc.set_object(&mut self.r.mem, i, o.min, o.max, flags);
            }
        });
        // the shape's constructor and the tests the casts make of it (as tests/worldboarder.rs)
        for a in [0x257810, 0x235fc8, 0x236830, 0x2578c0, 0x257918, 0x257978, 0x257988, 0x257a50, 0x257ba8, 0x257c50, 0x257af8, 0x236130, 0x236498, 0x236bb8, 0x237480, 0x237d50, 0x238640, 0x23b6e8] {
            self.r.allow(a);
        }
        self.real_cast.set(true);
        self.inst0.set(rc.inst0);
        self.rc = Some(rc);
    }

    /// An object's flags (+0xe8) in the runner's course as the game had them (F4b3k4), TRICKY_WALL_SKIP's solid bit
    /// off as [`W::load_course`] does; nothing without the runner's course.
    fn set_object_flags(&mut self, course: &tricky_game::course_ground::CourseGround, o: usize, flags: u32) {
        let Some(rc) = &self.rc else { return };
        let Some((min, max)) = course.with_world(|w| w.objects.get(o).map(|x| (x.min, x.max))) else { return };
        let flags = if wall_skip().contains(&(o as u32)) { flags & !0x20 } else { flags };
        rc.set_object(&mut self.r.mem, o, min, max, flags);
    }

    /// `Boarder_SetState` 0x11f608 as the original for the next call (F4h5m): `Boarder_ExitControlState` 0x11c840 and
    /// `Boarder_EnterControlState` 0x11c8e8 with the routines the ground motion's changes reach (the revert, state 4,
    /// from 3, 5 or 0xc; the stumble, state 2, from 3 or 0xe: `CruiseControl_Exit` 0x1033c0, `FakieCruiseControl_Exit`
    /// 0x104688, `LandNormalControl_Exit` 0x104e38, `PrewindControl_Exit` 0x1055a8, `ChangeFakieControl_Exit` 0x104178,
    /// `ChangeFakieControl_Enter` 0x103eb8, `BumpControl_Enter` 0x104190), their objects pointed at the rider;
    /// `Boarder_UpdateBoost` 0x11d038 and `AnimCtl_TurnAllLayers` 0x15eef8 logged. Without, the stub that
    /// only logs the state asked for.
    fn real_states(&mut self, on: bool) {
        self.states_real = on;
        if !on {
            let l = self.log.clone();
            self.r.stub(0x11f608, move |cpu, _| l.borrow_mut().push(Ev::State(cpu.gpr(5) as u32)));
            return;
        }
        self.r.unstub(0x11f608);
        for f in [0x11f608, 0x11c840, 0x11c8e8, 0x1033c0, 0x104688, 0x104e38, 0x1055a8, 0x104178, 0x103eb8, 0x104190] {
            self.r.allow(f);
        }
        for f in [0x11d038, 0x15eef8] {
            let l = self.log.clone();
            self.r.stub(f, move |_, _| l.borrow_mut().push(Ev::Call(f)));
        }
        // the objects' rider words: bump +0x610 (+0x10), cruise +0x630 (+0x14), change fakie +0x650 (+0), fakie cruise
        // +0x660 (+0), normal landing +0x6f0 (+0xc), prewind +0x710 (+0xc)
        for o in [0x620, 0x644, 0x650, 0x660, 0x6fc, 0x71c] {
            self.r.mem.write_u32(self.handler + o, self.rd);
        }
    }
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
    // Boarder_WallCollide 0x126250 (as the original with the course's walls, F4h5k: `W::put_walls`; its callees
    // `Boarder_AccumulateImpact` 0x125c48,
    // `Boarder_Translate` 0x1267a0, `CollBody_Translate` 0x236448, `Boarder_TurnFromWall` 0x1257a0,
    // `Boarder_WallImpactCheck` 0x126540, `Boarder_Stumble` 0x124dd0; the sounds, `Rand_Game` (0) and the speech stubbed;
    // `cWorldBoarder`'s constructor and vf10 below)
    stub_wall(&mut r, &log, &wall, rd);
    for a in [0x126250, 0x125c48, 0x1267a0, 0x236448, 0x1257a0, 0x126540, 0x124dd0] {
        r.allow(a);
    }
    // (and `Broadphase_MoveBox` 0x14fe68, the broadphase box `Boarder_Translate` moves)
    for a in [0x216c50, 0x2162f0, 0x235398, 0x14fe68] {
        r.stub(a, |_, _| {});
    }
    r.stub(0x23d4d8, |cpu, _| cpu.set_gpr(2, 0));
    let course = Rc::new(RefCell::new(None::<std::sync::Arc<tricky_game::course_ground::CourseGround>>));
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
    // Boarder_InstanceCollisions 0x125088 (E8b): logged; as the original with the objects' flags (`W::put_walls`,
    // F4b3k4): its `World_QueryInstances` 0x25b878 answered by the course's ([`instance_query`]: t, the point +0x10,
    // the normal +0x20, the object +0x38 as the runner's course's instance), `Instance_OnPlayerContact` 0x13bd40 (the
    // event scripts) stubbed, `Boarder_InstanceBounce` 0x125a00 the original
    r.stub(0x125088, move |_, _| l.borrow_mut().push(Ev::Instances));
    r.allow(0x125088);
    r.allow(0x125a00);
    r.stub(0x13bd40, |_, _| {});
    let inst0 = Rc::new(Cell::new(0u32));
    let (c, i0) = (course.clone(), inst0.clone());
    r.stub(0x25b878, move |cpu, mem| {
        let course = c.borrow().clone().expect("World_QueryInstances ran without the course");
        let mut b = CollBody::new();
        b.center = rv(mem, rd + 0x480);
        b.radius = mem.read_f32(rd + 0x490);
        b.x24 = mem.read_u32(rd + 0x494) as i32;
        b.count = (mem.read_u32(rd + 0x498) as i32).clamp(0, 18);
        for k in 0..18u32 {
            b.spheres[k as usize] = (rv(mem, rd + 0x4a0 + 0x20 * k), mem.read_f32(rd + 0x4b0 + 0x20 * k));
        }
        let (t, h, o) = instance_query(&course, &b);
        let out = cpu.gpr(6) as u32;
        wv(mem, out + 0x10, h.pos);
        wv(mem, out + 0x20, h.normal);
        mem.write_u32(out + 0x38, if h.object == 0 { 0 } else { i0.get() + 0x100 * o.id });
        cpu.set_f(0, t);
    });
    // the world under the probe, as in tests/probe.rs
    let l = log.clone();
    let (landing, line_from) = (Rc::new(Cell::new(false)), Rc::new(Cell::new([0.0f32; 4])));
    let lf = line_from.clone();
    r.stub(0x25d7f0, move |cpu, mem| {
        let (from, to) = (rv(mem, cpu.gpr(5) as u32), rv(mem, cpu.gpr(6) as u32));
        lf.set(from);
        l.borrow_mut().push(Ev::Line(cpu.f(12).to_bits(), b4(from), b4(to), cpu.gpr(7)));
    });
    let h = hit.clone();
    let (ld, lf) = (landing.clone(), line_from.clone());
    let (l, c) = (log.clone(), course.clone());
    let real_cast = Rc::new(Cell::new(false));
    let rc = real_cast.clone();
    r.hook(0x25aff8, move |cpu, mem| {
        // `Boarder_WallCollide`'s cast (the shape a `cWorldBoarder`, vtable 0x3a77d8 at +0x40): the original's on the
        // course built in the runner (F4h5m), else ours, of the body at rider+0x470 with the ground normal the shape
        // holds (+0x2e0)
        if mem.read_u32(cpu.gpr(5) as u32 + 0x40) == 0x3a77d8 && rc.get() {
            l.borrow_mut().push(Ev::Wall(mem.read_u32(rd + 0x494)));
            return false;
        }
        match *h.borrow() {
        _ if mem.read_u32(cpu.gpr(5) as u32 + 0x40) == 0x3a77d8 => {
            l.borrow_mut().push(Ev::Wall(mem.read_u32(rd + 0x494)));
            let course = c.borrow().clone().expect("Boarder_WallCollide ran without the course");
            let mut b = CollBody::new();
            b.center = rv(mem, rd + 0x480);
            b.radius = mem.read_f32(rd + 0x490);
            b.x24 = mem.read_u32(rd + 0x494) as i32;
            b.count = (mem.read_u32(rd + 0x498) as i32).clamp(0, 18);
            for k in 0..18u32 {
                b.spheres[k as usize] = (rv(mem, rd + 0x4a0 + 0x20 * k), mem.read_f32(rd + 0x4b0 + 0x20 * k));
            }
            let (t, h) = wall_cast(&course, &b, rv(mem, cpu.gpr(5) as u32 + 0x2e0));
            let out = cpu.gpr(6) as u32;
            wv(mem, out + 0x10, h.pos);
            wv(mem, out + 0x20, h.normal);
            mem.write_u32(out + 0x38, h.object);
            cpu.set_f(0, t);
        }
        None => cpu.set_f(0, -1.0),
        Some(h) => {
            let out = cpu.gpr(6) as u32;
            mem.write_u32(out, h.kind);
            let f = lf.get();
            wv(mem, out + 0x10, if ld.get() { [f[0] + h.normal[0] * 99.9, f[1] + h.normal[1] * 99.9, f[2] + h.normal[2] * 99.9, f[3]] } else { h.point });
            wv(mem, out + 0x20, h.normal);
            mem.write_u32(out + 0x34, h.surface);
            cpu.set_f(0, 0.4);
        }
        }
        true
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
    // F4b3k, on the pad word the recorder hooked: the cruise (`CruiseControl_Update` 0x102fc8) and the prewind
    // (`PrewindState_Update` 0x1050e8 with `PrewindState_SetWindTarget` 0x105a60 and `Math_Atan` 0x2510f8); logged:
    // `Boarder_UpdateBoost` 0x11d038, `Boarder_PlayCelebrate` 0x128150, `Boarder_PlayDisappointed` 0x127e80,
    // `Ride_AITauntCheck` 0x103660, `CruiseControl_LookBack` 0x1033d0, `CruiseControl_RandomFlourish` 0x103b88,
    // `PrewindState_UpdateWindClip` 0x1055f0 and the rider's virtual +0x7c (put at 0x1040)
    for a in [0x102fc8, 0x1050e8, 0x105a60, 0x2510f8] {
        r.allow(a);
    }
    for a in [0x11d038, 0x128150, 0x127e80, 0x103660, 0x1033d0, 0x103b88, 0x1055f0, 0x1040] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(Ev::Call(a)));
    }
    // the wiped-out riders' slides (F4b3w): `Boarder_UpdateMotion` 0x11c770 (case 5, `WipeoutMotion_Update` inlined:
    // the handler's +0x550 is the rider) and `Ragdoll_GroundProbe` 0x10fd58 on the world above; the body made again
    // (`Ragdoll_InitFromBoarder` 0x10dad8) and the get-up (`Wipeout_StartGetUp` 0x10f178) logged
    for a in [0x11c770, 0x10fd58] {
        r.allow(a);
    }
    // the get-up (F4b3x2): `GetUpMotion_Update` 0x1102c8 and its turn toward the course (`Vec4_Set` 0x10c120,
    // `Quat_RotateVecToVec` 0x2508b8, `Quat_Slerp` 0x250a48)
    for a in [0x1102c8, 0x10c120, 0x2508b8, 0x250a48, 0x102e80] {
        r.allow(a);
    }
    for a in [0x10dad8, 0x10f178] {
        let l = log.clone();
        r.stub(a, move |_, _| l.borrow_mut().push(Ev::Call(a)));
    }
    // the landing (F4b3x3): `AirMotion_Update` 0x108378 with `Landing_CheckAngles` 0x12ba78 and the clip queries it
    // makes (`AnimCtl_GetClipType` 0x15eec0, `AnimCtl_TestClipFlag` 0x15ffb8); logged and answered as nothing: the
    // flight prediction (`AirPredict_Tick` 0x123ec8, `AirPredict_ClearLandTime` 0x123f00), the fall scale
    // (`AirMotion_SetFallScale` 0x1049a0), `AnimCtl_BlendOutAll` 0x15f050, `Landing_ChooseState` 0x109308 (its
    // clips and states), `Score_LandTrick` 0x1569b0 (0 points), `Boarder_StartCelebrate` 0x11f3b8,
    // `Boarder_IsTricky` 0x11f430 (no), `Speech_OnAirTime` 0x233448, `Boarder_SpawnLandingPuffs` 0x136e90,
    // `Sfx_UpdateAir` 0x217d50 and the rider's virtuals +0x6c, +0x84, +0x9c (put at 0x1010, 0x1020, 0x1030)
    for a in [0x108378, 0x12ba78, 0x15ffb8] {
        r.allow(a);
    }
    for a in [0x123ec8, 0x123f00, 0x1049a0, 0x15f050, 0x109308, 0x1569b0, 0x11f3b8, 0x11f430, 0x233448, 0x136e90, 0x217d50, 0x1010, 0x1020, 0x1030] {
        let l = log.clone();
        r.stub(a, move |cpu, _| {
            l.borrow_mut().push(Ev::Call(a));
            cpu.set_gpr(2, 0);
            cpu.set_f(0, 0.0);
        });
    }
    // a rider that does not land (rec16 8288 r2): the air's wall sweep, `cWorldBoarder::cWorldBoarder` 0x257810 (the
    // body at rider+0x470) and `cWorldBoarder::vf10` 0x257ca0, logged; STANDIN: the sweep's `World_RayCast` answered
    // as a miss (the hit is cleared: the world around the body is not recorded)
    // (from `Boarder_WallCollide` 0x126250..0x126540, F4h5k: only the vtable set at +0x40, which `World_RayCast`'s
    // stub tells the wall's cast by)
    // (and from `Boarder_InstanceCollisions` 0x125088..0x1257a0 when it runs as the original, F4b3k4)
    let in_wall = |cpu: &r5900::Cpu| (0x126250..0x126540).contains(&(cpu.gpr(31) as u32)) || (0x125088..0x1257a0).contains(&(cpu.gpr(31) as u32));
    let (l, h) = (log.clone(), hit.clone());
    let rc = real_cast.clone();
    r.hook(0x257810, move |cpu, mem| {
        if in_wall(cpu) && rc.get() {
            return false;
        }
        if in_wall(cpu) {
            mem.write_u32(cpu.gpr(4) as u32 + 0x40, 0x3a77d8);
            return true;
        }
        l.borrow_mut().push(Ev::Call(0x257810));
        *h.borrow_mut() = None;
        true
    });
    let l = log.clone();
    r.stub(0x257ca0, move |cpu, _| {
        if !in_wall(cpu) {
            l.borrow_mut().push(Ev::Call(0x257ca0));
        }
    });
    for (o, f) in [(0x68, 0x1010), (0x80, 0x1020), (0x98, 0x1030), (0x78, 0x1040)] {
        r.mem.write_u16(vtable + o, 0);
        r.mem.write_u32(vtable + o + 4, f);
    }
    let air = r.mem.alloc(0x40);
    r.mem.write_u32(air + 0x20, rd);
    let pad = r.mem.alloc(16);
    let mut seqs = [(0, 0, 0); 2];
    for s in &mut seqs {
        *s = (r.mem.alloc(0x80), r.mem.alloc(0x10), r.mem.alloc(0x10));
    }
    Some(W { r, rd, stats, motion, log, hit, wall, meter, handler, pad, seqs, air, landing, course, real_cast, states_real: false, rc: None, inst0 })
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
        m.write_u32(rd + 0x418, c.finish as u32);
        // what the cruise and the prewind read besides (F4b3k): the crouch kept for the jump +0x1c8, the take-off kind
        // +0x1dc, the input records' rates and targets, the wind-ups +0x220..+0x234, as the tick left them
        for o in [0x1c8, 0x1dc, 0x200, 0x204, 0x20c, 0x210, 0x218, 0x21c, 0x220, 0x224, 0x228, 0x22c, 0x230, 0x234] {
            m.write_u32(rd + o, u32_le(&c.rider, o as usize));
        }
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
    let mut env = E { log: Vec::new(), hit: case.hit, wall: case.wall, meter: case.meter, states: false, walls: None };
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

mod course_clock;
mod recording;
mod runner_course;
use recording::{f32_at, load, pair_list, player_pad, rider_addrs, u32_le, vec4};

/// The rider as `GroundMotion_Update` reads it, from a recorded rider block (0x600 bytes from the rider), its stats
/// block, the ground motion object (its phase +8) and the animation's block at rider+0x46c0 (the clip +8).
/// What `Boarder_UpdateControl` 0x11c9a0 reads of a rider (F4b3i): layer 0's and 1's clips (+0x46c0 +8, +0xc) and, of
/// layer 0's sequences (windows `4718>0`, `4718>30>0`, their lengths `4718>78>8>a`, `4718>30>78>8>a`), the first
/// playing layer 0's clip (what `AnimLayer_FindClip` finds; none: the phase is 1); the handler's control objects
/// (window `5ae0>600`, +0x600..+0x830), the pad word and the finish time (+0x418)
#[derive(Clone)]
struct Ctl {
    clips: [u32; 2],
    seq: Option<(Vec<u8>, u16)>,
    handler: Vec<u8>,
    pad: u32,
    finish: i32,
    /// the rider as the second sample holds it (F4b3k: the prewind's wind-ups +0x220..+0x238, its kind +0x1dc, the
    /// crouch kept +0x1c8, the class +0x420: the control leaves the values as `cBoarder::vf7` moved them before the
    /// motion)
    rider: Vec<u8>,
}

/// The controls replayed: bump (2), change fakie (4), fakie cruise (5, not on its turn clip 0x202, which plays clips
/// and flips the stance), hard landing (0xb), normal landing (0xc)
const CONTROLS: [u32; 5] = [2, 4, 5, 0xb, 0xc];

/// The controls replayed only with the tick's own pad word (F4b3k, [`hook_pad`]): cruise (3, `CruiseControl_Update`
/// 0x102fc8) and prewind (0xe, `PrewindState_Update` 0x1050e8), which go by its buttons
const PADDED: [u32; 2] = [3, 0xe];

/// The pad word the rider's controller made for the tick that ends at sample `t` (F4b3k): `cPlayer::GetControls`
/// 0x151250 or `cComputer::GetControls` 0x1384b8 build it on `cBoarder::vf7` 0x117198's stack, where PINE cannot see
/// it; `pine.py record ... hookpad` puts a hook on the call that hands it to `Boarder_UpdateControl` (0x117b78) that
/// keeps it with the rider's speed-sample count (+0x5900) of that tick (window `hookpad`). None without the window, or
/// when its count is not the sample's (window `5900`): the rider did not tick in this frame.
fn hook_pad(t: &recording::Tick, i: usize) -> Option<u32> {
    let x = &t.chased.iter().find(|(s, _)| s == "hookpad")?.1[i];
    let count = t.extra.iter().find(|(o, _)| *o == 0x5900).map(|(_, v)| f32_at(&v[i], 0));
    (count.is_none() || count == Some(f32_at(x, 4))).then(|| u32_le(x, 0))
}

/// (the animation read from sample `t`, the handler from `th`)
fn ctl_of(t: &recording::Tick, th: &recording::Tick, i: usize, ka: usize, addr: u32, pad: u32, after: &[u8]) -> Option<Ctl> {
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
    Some(Ctl { clips, seq, handler: h.clone(), pad, finish: u32_le(&t.riders[i], 0x418) as i32, rider: after.to_vec() })
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
    cruise: control::Cruise,
    prewind: tricky_game::boarder::Prewind,
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
            3 => {
                let mut c = self.cruise;
                c.update(self, pad, r);
                self.cruise = c;
            }
            4 => control::change_fakie(self, pad, r),
            // F4b3k: `PrewindState_Update` ours (`boarder::prewind_update`), its state asked for as `Boarder_SetState`
            0xe => {
                for call in tricky_game::boarder::prewind_update(pad, false, &mut self.prewind) {
                    if let tricky_game::boarder::PrewindCall::State(s) = call {
                        self.set_state(r, s);
                    }
                }
            }
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
    // the cruise's look-back timer and brake held (handler +0x638, +0x640), the prewind's started (+0x718) and the
    // rider's fields it reads (F4b3k)
    let cruise = control::Cruise { look: f32_at(&c.handler, 0x38), braking: u32_le(&c.handler, 0x40) };
    let x = &c.rider;
    let v3 = |o: usize| [f32_at(x, o), f32_at(x, o + 4), f32_at(x, o + 8)];
    let prewind = tricky_game::boarder::Prewind {
        f: tricky_game::boarder::Fields { x1fc: p.brake, crouch: p.crouch, crouch_rate: f32_at(x, 0x20c), crouch_target: f32_at(x, 0x210), boost_level: p.boost_level, ..Default::default() },
        started: u32_le(&c.handler, 0x118),
        kind: u32_le(x, 0x1dc),
        x418: c.finish,
        spin: v3(0x220),
        flip: v3(0x22c),
        x1c8: f32_at(x, 0x1c8),
        vel: p.vel,
        board: p.p.fwd,
        steer: p.steer,
        steer_rate: f32_at(x, 0x218),
        steer_target: f32_at(x, 0x21c),
        surface: p.p.surface,
        switch: p.switch,
        class: p.class,
        ..Default::default()
    };
    let mut env = CtlEnv { m, o, pad: c.pad, bump, land, cruise, prewind, state: None };
    let mut r = control::Rider { vel: p.vel, board: p.p.fwd, brake: p.brake, crouch: p.crouch, steer: p.steer, surface: p.p.surface, finish: c.finish, stance: p.switch, control: p.state, class: p.class, b718: u32_le(&c.handler, 0x118), ..Default::default() };
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
    // (recordings without the bodies, window under 0x700: an empty body; their frames are not replayed)
    if r.len() < 0x700 {
        return b;
    }
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
/// `Boarder_UpdateBroadphase` 0x126748 from the bones, in `cPostAI::vf1` 0x114730 after every rider's tick; neither
/// motion moves a body by its step: `AirMotion_Update` 0x108378 calls `Boarder_Translate` only to push a rider out
/// of the world, F4b3j; TRICKY_AIRSTEP=1 moves an air rider's body by its step as before) and are moved by the
/// pushes (`Boarder_Translate` 0x1267a0). The hits are `hit::rider_hit` on the hit rider as it is at that moment (`hit`:
/// after its tick when it has ticked, else as the first sample holds it), with the pair's record from the second
/// sample (+0x40 + 0x20·j: [0] the distance, [1] the bearing; read only for a hop or a wipe-out, which leave the
/// ground). Finished riders (`Boarder_IsFinishedAndStopped`) are not looked at: none in these recordings stop
/// before the end; nor the shoves (the layer-1 clip type is not in the recording: `clip_type1` answers 0).
#[derive(Clone)]
struct Frame {
    body: Vec<CollBody>,
    /// per rider: its body was read stale and placed by its guess ([`Twins`])
    stale: Vec<bool>,
    hit: Vec<HitRider>,
    rec: Vec<Vec<[f32; 2]>>,
    /// `Boarder_GetSpeedStat` 0x11d7b0 of each rider at that moment
    stat: Vec<f32>,
    /// COP1's `add.s` for the way (F4b3q): IEEE, or PCSX2's adder under `fadd=g1` ([`adder`])
    add: Add,
    /// the first sample's tick
    tick: u32,
    /// per rider: its state as the frame goes (the first sample's, 2 once a hit stumbles it) and whether a hit
    /// stumbled it out of the cruise or prewind state (F4b3r: `Boarder_Stumble` 0x124dd0 → `Boarder_SetState` 2 →
    /// `CruiseControl_Exit` 0x1033c0 / `PrewindControl_Exit` 0x1055a8 → `Boarder_UpdateBoost`(rider, 0, 0): the
    /// boost level +0x130 is 0 from then on)
    state: Vec<u32>,
    stumbled: Vec<bool>,
    /// per rider: `Boarder_GetSpeedStat` with the boost level 0
    stat0: Vec<f32>,
    /// per rider: the type of its layer-1 clip (`AnimCtl_GetClipType`(rider+0x46c0, 1): the second sample's clip,
    /// window `46c0`) and whether it is between the clip's flags 1 and 2 (F4b3s, the shove)
    clip1: Vec<u32>,
    shoving: Vec<bool>,
    /// per rider: pushes and hits it got this frame
    pushes: Vec<usize>,
    hits: Vec<usize>,
}

/// The bodies read before `cPostAI::vf1` 0x114730 re-posed them, and a guess at each (board rows F4h5a, F4b3o).
/// `cPostAI::vf1` poses riders 0 to 5 in turn after all have ticked (5 to 0), so a PINE read that lands inside it finds
/// the last ones still on the previous tick's pose, moved only by the pushes of their own frame (`Boarder_Translate`
/// 0x1267a0): its centre's offset from the rider (rider+0x480 − +0x140) is then the last fresh sample's less the step.
/// Such a sample is stale when its body is the previous sample's byte for byte while the rider moved (F4h5a), or when
/// its offset is within 3 cm of the last fresh offset less the step and more than twice that plus 2 cm from the last
/// fresh offset itself, the step being over 2 cm (F4b3o: rec17 11131 r2, pushed in its frame; 11817 r4 and r5, two
/// stale samples in a row), or when it is the previous sample's body moved whole by its pushes ([`pushed_only`], F4b3y:
/// pushes too big for the 3 cm, and wiped-out riders, whose offset swings). TRICKY_LAG=off: the byte for byte ones
/// only; TRICKY_RIGID=off: not the moved whole ones.
///
/// The guess, in order: the same tick recorded fresh on another pass of a save state (the game replays the same ticks
/// bit for bit, rec16; F4h5e); the next sample's body when it is stale too (it is this tick's pose, moved by the next
/// frame's pushes); else each sphere at the rider's position plus the mean of its offsets in the last fresh sample and
/// the next one (median 0.19 cm off on fresh ground samples of rec17, against 1.2 cm for the previous body moved by
/// the step), or the one of them there is. TRICKY_TWINS=off: no other pass.
// STANDIN: a guess at the body PINE missed, for the body `Boarder_UpdateBroadphase` 0x126748 posed from the bones
// (only for recordings made before F4h5d: the recorder now reads such samples again, header `posed 1`)
#[derive(Default)]
struct Twins {
    /// the stale samples' guesses by (tick, rider, +0x140..+0x160 and +0x480..+0x490 bytes)
    guess: std::collections::HashMap<(u32, usize, Vec<u8>), CollBody>,
}

/// The body `r` is the one before it, `p`, moved whole (every sphere and the centre by one vector, within 0.05 cm)
/// by something other than the rider's step (more than 0.5 cm from it): the last pose moved only by the pushes of
/// its frame (`Boarder_Translate` 0x1267a0 adds one vector to each sphere), read before `cPostAI::vf1` re-posed it
/// (F4b3y: rec14 9097 riders 1 and 2, sliding wiped out, pushed 9 cm apart). A pose made from the bones is never
/// such a copy, except moved by the step with the rider.
fn pushed_only(p: &[u8], r: &[u8]) -> bool {
    let count = (u32_le(r, 0x498) as usize).min(18);
    if count == 0 || u32_le(p, 0x498) != u32_le(r, 0x498) {
        return false;
    }
    let d = |o: usize| sub4(vec4(r, o), vec4(p, o));
    let shift = d(0x480);
    let step = d(0x140);
    let same = (0..count).all(|s| {
        let x = d(0x4a0 + 0x20 * s);
        (0..3).all(|c| (x[c] - shift[c]).abs() < 0.05)
    });
    let off = (0..3).map(|c| (shift[c] - step[c]) * (shift[c] - step[c])).sum::<f32>().sqrt();
    same && off > 0.5
}

fn stale_key(tick: u32, i: usize, r: &[u8]) -> (u32, usize, Vec<u8>) {
    (tick, i, [&r[0x140..0x160], &r[0x480..0x490]].concat())
}

impl Twins {
    /// [`Twins::of`], or none for a recording whose recorder read stale bodies again (header `posed 1`, F4h5d: its
    /// few left, 6 of 7,008 rider samples in a 20 s test, are kept as read). TRICKY_STALE=guess: guess anyway.
    fn of_recording(path: &str, ticks: &[recording::Tick]) -> Twins {
        if recording::posed(path) && !std::env::var("TRICKY_STALE").is_ok_and(|v| v == "guess") {
            return Twins::default();
        }
        Twins::of(ticks)
    }

    fn of(ticks: &[recording::Tick]) -> Twins {
        use std::collections::HashMap;
        let lag = !std::env::var("TRICKY_LAG").is_ok_and(|v| v == "off");
        let rigid = lag && !std::env::var("TRICKY_RIGID").is_ok_and(|v| v == "off");
        let n = ticks.first().map_or(0, |t| t.riders.len());
        let near = |k: usize| k > 0 && ticks[k - 1].tick + 1 == ticks[k].tick;
        let sub3 = |x: V4, y: V4| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
        let len = |x: [f32; 3]| (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        // per sample and rider: stale, and the last fresh sample before it in an unbroken run
        let mut stale = vec![vec![false; n]; ticks.len()];
        let mut fresh_before = vec![vec![None; n]; ticks.len()];
        for i in 0..n {
            let mut last: Option<usize> = None;
            for k in 0..ticks.len() {
                let r = &ticks[k].riders[i];
                if r.len() < 0x700 || !near(k) {
                    last = (r.len() >= 0x700).then_some(k);
                    continue;
                }
                fresh_before[k][i] = last;
                let p = &ticks[k - 1].riders[i];
                let moved = r[0x140..0x14c] != p[0x140..0x14c];
                let off = sub3(vec4(r, 0x480), vec4(r, 0x140));
                let step = sub3(vec4(r, 0x140), vec4(p, 0x140));
                let lagging = lag && last.is_some_and(|l| {
                    let lr = &ticks[l].riders[i];
                    let of = sub3(vec4(lr, 0x480), vec4(lr, 0x140));
                    let ef = len(sub3([off[0], off[1], off[2], 0.0], [of[0], of[1], of[2], 0.0]));
                    let es = len([off[0] + step[0] - of[0], off[1] + step[1] - of[1], off[2] + step[2] - of[2]]);
                    len(step) > 2.0 && es < 3.0 && ef > 2.0 * es + 2.0
                });
                stale[k][i] = moved && (r[0x480..0x700] == p[0x480..0x700] || lagging || (rigid && pushed_only(p, r)));
                if !stale[k][i] {
                    last = Some(k);
                }
            }
        }
        // the fresh samples by (tick, rider, the rider's +0x140..+0x160 bytes)
        let mut twin = HashMap::new();
        if !std::env::var("TRICKY_TWINS").is_ok_and(|v| v == "off") {
            for (k, t) in ticks.iter().enumerate() {
                for (i, r) in t.riders.iter().enumerate() {
                    if r.len() >= 0x700 && near(k) && ticks[k - 1].riders[i][0x480..0x700] != r[0x480..0x700] && !stale[k][i] {
                        twin.entry((t.tick, i, r[0x140..0x160].to_vec())).or_insert_with(|| r.clone());
                    }
                }
            }
        }
        let mut guess = HashMap::new();
        for (k, t) in ticks.iter().enumerate() {
            for i in 0..n {
                if !stale[k][i] {
                    continue;
                }
                let a = &t.riders[i];
                let next = (k + 1 < ticks.len() && near(k + 1)).then_some(k + 1);
                let body = if let Some(tw) = twin.get(&(t.tick, i, a[0x140..0x160].to_vec())) {
                    body_of(tw)
                } else if let Some(m) = next.filter(|&m| stale[m][i]) {
                    body_of(&ticks[m].riders[i])
                } else {
                    let from: Vec<&Vec<u8>> = [fresh_before[k][i], next].into_iter().flatten().map(|m| &ticks[m].riders[i]).collect();
                    if from.is_empty() {
                        continue;
                    }
                    let pa = vec4(a, 0x140);
                    let place = |o: usize| -> V4 {
                        let mut v = [0.0f32, 0.0, 0.0, f32_at(a, o + 0xc)];
                        for r in &from {
                            let (c, q) = (vec4(r, o), vec4(r, 0x140));
                            for x in 0..3 {
                                v[x] += c[x] - q[x];
                            }
                        }
                        for x in 0..3 {
                            v[x] = v[x] / from.len() as f32 + pa[x];
                        }
                        v
                    };
                    let mut b = body_of(a);
                    b.center = place(0x480);
                    for s in 0..18 {
                        b.spheres[s].0 = place(0x4a0 + 0x20 * s);
                    }
                    b
                };
                guess.entry(stale_key(t.tick, i, a)).or_insert(body);
            }
        }
        Twins { guess }
    }

    /// the guess at `r`'s body when it was read stale
    fn guess(&self, tick: u32, i: usize, r: &[u8]) -> Option<&CollBody> {
        (r.len() >= 0x700).then(|| self.guess.get(&stale_key(tick, i, r))).flatten()
    }
}

impl Frame {
    /// `p`: the sample before `a`, when there is one (for the stale bodies, [`Twins`]; TRICKY_STALE=keep leaves the bodies as read);
    /// `tick`: `a`'s tick
    #[allow(clippy::too_many_arguments)]
    fn new(p: Option<&[Vec<u8>]>, a: &[Vec<u8>], b: &[Vec<u8>], stats: &[Vec<u8>], twins: &Twins, tick: u32, add: Add, clips: Option<(&[Vec<u8>], &[u8])>, flags: &[Option<bool>]) -> Frame {
        let n = a.len();
        let shoves = std::env::var("TRICKY_SHOVE").map_or(true, |v| v != "off");
        // between the clip's flags 1 and 2 (`flags`, [`shoving_of`]) when recorded (rec21 on), else
        // STANDIN: the clip's flags 1 and 2 (bits of the layer-1 clip record's byte +0x74, not recorded), for the game's
        // answer: the rider shoved someone this tick (its record of them +0x4c is the tick and its hit count +0x5a
        // went up: `Boarder_RiderCollisions` 0x123fc8 adds one to the shover's count only)
        let shoving = (0..n)
            .map(|i| {
                if let Some(Some(f)) = flags.get(i) {
                    return *f;
                }
                (0..n).any(|j| {
                    j != i && u32_le(&b[i], 0x4c + 0x20 * j) == tick + 1 && u16::from_le_bytes([b[i][0x5a + 0x20 * j], b[i][0x5b + 0x20 * j]]) != u16::from_le_bytes([a[i][0x5a + 0x20 * j], a[i][0x5b + 0x20 * j]])
                })
            })
            .collect();
        let clip1 = (0..n)
            .map(|i| match clips.filter(|_| shoves) {
                Some((c, table)) => match u32_le(&c[i], 0xc) {
                    animctl::NONE => 8,
                    c => u32_le(table, c as usize * 0x1c),
                },
                None => 0,
            })
            .collect();
        let keep = std::env::var("TRICKY_STALE").is_ok_and(|v| v == "keep");
        let mut stale = vec![false; n];
        Frame {
            body: (0..n)
                .map(|i| {
                    let mut body = body_of(&a[i]);
                    if let Some(g) = twins.guess(tick, i, &a[i]).filter(|_| p.is_some() && !keep) {
                        body = g.clone();
                        stale[i] = true;
                    }
                    body
                })
                .collect(),
            stale,
            hit: a.iter().map(|r| hit_rider(r)).collect(),
            rec: b.iter().map(|r| (0..n).map(|j| [f32_at(r, 0x40 + 0x20 * j), f32_at(r, 0x44 + 0x20 * j)]).collect()).collect(),
            stat: (0..n).map(|i| speed_stat(&stats[i], f32_at(&a[i], 0x130), add)).collect(),
            add,
            tick,
            state: a.iter().map(|r| u32_le(r, 0x428)).collect(),
            stumbled: vec![false; n],
            stat0: (0..n).map(|i| speed_stat(&stats[i], 0.0, add)).collect(),
            clip1,
            shoving,
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
        if std::env::var("TRICKY_DBGP").is_ok_and(|t| t.parse() == Ok(self.tick)) {
            eprintln!("DBGP tick {} push rider {r} by {v:?}", self.tick);
        }
    }
    fn speed_stat(&mut self, r: usize) -> f32 {
        self.stat[r]
    }
    /// 0x124200..0x1242c4: the dots on VU0 ((x + y) + z) + w, each rounded to nearest), then `sub.s`, `add.s` (the
    /// way's adder), `div.s` (nearest, F4b3l) and `mul.s`
    fn pair_speeds(&mut self, n: V4, va: V4, vb: V4, ma: f32, mb: f32) -> (f32, f32) {
        let dot = |a: V4| ((a[0] * n[0] + a[1] * n[1]) + a[2] * n[2]) + a[3] * n[3];
        let k = (self.add)(dot(vb), -dot(va)) / (self.add)(ma, mb);
        (mb * k, -ma * k)
    }
    fn rider_hit(&mut self, speed: f32, to: usize, _: usize, owner: usize, of: usize, dir: V4, vel: &mut V4) -> bool {
        let mut h = self.hit[to];
        h.vel = *vel;
        let pre = h.vel;
        let mut calls = Vec::new();
        let knocked = rider_hit(speed, dir, self.rec[owner][of], false, false, &mut h, &mut calls);
        if calls.iter().any(|c| matches!(c, HitCall::Stumble { .. })) && matches!(self.state[to], 3 | 0xe) && std::env::var("TRICKY_BOOST").map_or(true, |v| v != "end") {
            self.state[to] = 2;
            self.stumbled[to] = true;
            self.stat[to] = self.stat0[to];
        }
        if std::env::var("TRICKY_DBG").is_ok_and(|v| v == "3") {
            eprintln!("DBG3 tick {} hit to {to} owner {owner} of {of} speed {speed} dir {dir:?} pre {pre:?} post {:?} normal {:?} motion {} stats {:?}", self.tick + 1, h.vel, h.normal, h.motion, self.stat);
        }
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
    fn clip_type1(&mut self, r: usize) -> u32 {
        self.clip1[r]
    }
    /// flag 1 (between the shove's markers) and flag 2 (past it): see `shoving`
    fn clip_flag(&mut self, r: usize, flag: u32) -> bool {
        flag == 1 && self.shoving[r]
    }
}

/// What `Boarder_RiderHit` reads of a recorded rider.
fn hit_rider(r: &[u8]) -> HitRider {
    HitRider { heading: f32_at(r, 0x1b0), pos: vec4(r, 0x140), vel: vec4(r, 0x150), normal: vec4(r, 0x2a0), motion: u32_le(r, 0x424), crouch: f32_at(r, 0x1c8) }
}

/// `Boarder_GetSpeedStat` 0x11d7b0: stats+0x1e × (stats+0x12 × 2.145779 × 0.003921569 + 0.79599), then × (1 + the
/// boost level +0x130 × 7.999995).
fn speed_stat(st: &[u8], boost: f32, add: Add) -> f32 {
    let m = st[0x1e] as f32 * add(st[0x12] as f32 * 2.145_779 * 0.003_921_569, 0.795_99);
    add(boost * 7.999_995 * m, m)
}

/// The boost level (rider+0x130) a rider has when its collisions run (board row F4b3r): its second sample's, unless
/// that is 0 while its first was boosting and it went from the cruise (3) or prewind (0xe) state to the bump state (2)
/// in the tick: a stumble in its own collisions. (A state its control leaves zeroes the boost before the collisions.)
/// `CruiseControl_Update` 0x102fc8 (likewise `PrewindState_Update`, `RailSlideControl_Update`) sets the level each
/// tick through `Boarder_UpdateBoost` 0x11d038 (1.0, 0x3f19f24e or 0x3e8003b9 by the meter +0x1c, 0 when the boost
/// button is up), before `Boarder_RiderCollisions`; a hit that stumbles the rider (state 2) then runs
/// `CruiseControl_Exit` 0x1033c0 → `Boarder_UpdateBoost`(rider, 0, 0), which zeroes it, so the sample holds 0 but the
/// hit was taken at the boosted speed stat. TRICKY_BOOST=end: the second sample's always, and no stumble tracking
/// ([`Frame`]'s `stumbled`).
/// STANDIN: the first sample's level, for the button (computer riders' pad word is not recorded, F4b3k) held as the
/// tick before and the meter in the same band.
fn boost_at_collisions(a: &[u8], b: &[u8]) -> f32 {
    let (sa, sb) = (u32_le(a, 0x428), u32_le(b, 0x428));
    let (ba, bb) = (f32_at(a, 0x130), f32_at(b, 0x130));
    let left = matches!(sa, 3 | 0xe) && sb == 2;
    if bb == 0.0 && ba > 0.0 && left && std::env::var("TRICKY_BOOST").map_or(true, |v| v != "end") {
        ba
    } else {
        bb
    }
}

/// A COP1 `add.s`.
type Add = fn(f32, f32) -> f32;

/// The way's `add.s` (F4b3q): `fadd=g1` in its rules → PCSX2's adder ([`pcsx2_add`]), else IEEE; TRICKY_COLADD=n:
/// IEEE always.
fn adder(rules: &str) -> Add {
    if rules.contains("fadd=g1") && std::env::var("TRICKY_COLADD").map_or(true, |v| v != "n") {
        pcsx2_add
    } else {
        |a, b| a + b
    }
}

/// `add.s` as PCSX2 runs it in normal play (`ps2float`'s `Guard`, F4h2, F4g): the smaller operand cut toward zero
/// to one bit below the larger's last mantissa bit (one guard bit, no sticky bit), then the sum rounded to nearest,
/// ties to even. A test model only, not the port's float rule (F4c).
fn pcsx2_add(a: f32, b: f32) -> f32 {
    let (big, small) = if b.abs() > a.abs() { (b, a) } else { (a, b) };
    if small == 0.0 || big == 0.0 {
        return a + b;
    }
    let e = ((big.to_bits() >> 23) & 0xff) as i32 - 127;
    let guard = 2f64.powi(e - 24);
    let cut = (small as f64 / guard).trunc() * guard;
    (big as f64 + cut) as f32
}

/// The driver's view of a recorded rider: position, velocity, motion and its records of the others (+0x40 + 0x20·j:
/// +8 the last bump's tick, +0xc the last shove's, +0x1a the hits).
fn col_rider(r: &[u8], n: usize, st: &[u8], pose: Option<Pose>) -> ColRider {
    let [fwd, side, up, at] = frame_of(r, st, pose);
    ColRider {
        pos: vec4(r, 0x140),
        vel: vec4(r, 0x150),
        motion: u32_le(r, 0x424),
        heading: f32_at(r, 0x1e0),
        fwd,
        side,
        up,
        at,
        stability: st[0x12],
        rec: (0..n).map(|j| ColRec { bump: u32_le(r, 0x48 + 0x20 * j) as i32, shove: u32_le(r, 0x4c + 0x20 * j) as i32, hits: u16::from_le_bytes([r[0x5a + 0x20 * j], r[0x5b + 0x20 * j]]) }).collect(),
        ..Default::default()
    }
}

/// What the rest of `Boarder_UpdatePose` 0x129628 reads to place the frame (F4b3t, [`frame_where`]): the pivot bone
/// (`8c0` window: +0x8c8, when +0x44d8 is 0) and whether the layer-0 clip (`46c0` window +8) has the skin's
/// children follow the frame, from the sample the pose was made at.
#[derive(Clone, Copy, Debug)]
struct Pose {
    pivot: [f32; 3],
    skin_child: bool,
}

/// Whether rider `i`'s layer-1 clip is between its flags 1 and 2 (`AnimCtl_TestClipFlag` 0x15ffb8: the byte +0x74,
/// bits 0 and 1, of the clip's record in the layer's list, `AnimLayer_FindClip` 0x15a6c8: rider+0x4788 the first,
/// +0x30 the next, +0x40 the clip id), from the windows `4788>0`, `4788>30>0`, `4788>30>30>0` and the layer-1 clip
/// (`46c0` +0xc) of the pair's first sample (TRICKY_FLAGS=b: the second; F4b3t1: rec23 and rec24, Snow, 58 of 58 shove
/// records right with the first, 58 of 74 with the second, which shoves a tick early; rec21, rec22 the same either
/// way). None without them, or when the
/// clip is not among the three records.
fn shoving_of(w: &[recording::Tick], i: usize, ka: usize) -> Option<bool> {
    let t = &w[(std::env::var("TRICKY_FLAGS").as_deref() == Ok("b")) as usize];
    let clip = u32_le(&t.extra[ka].1[i], 0xc);
    ["4788>0", "4788>30>0", "4788>30>30>0"].iter().find_map(|name| {
        let r = &t.chased.iter().find(|(s, _)| s == name)?.1[i];
        (r.len() >= 0x75 && u32_le(r, 0x40) == clip).then(|| r[0x74] & 3 == 1)
    })
}

fn pose_windows(ticks: &[recording::Tick]) -> Option<(Option<usize>, Option<usize>, usize)> {
    let win = |o: i64| ticks[0].extra.iter().position(|(x, _)| *x == o);
    Some((win(0x8c0), win(0x44d8), win(0x46c0)?)).filter(|_| std::env::var("TRICKY_POSE").map_or(true, |v| v != "off"))
}

/// The [`Pose`] of rider `i` at sample `t`: `k` the `8c0`, `44d8` and `46c0` windows, `table` the clip table. None
/// without them (the recordings before rec20) or with another pivot bone; but TRICKY_PIVOT=mean takes rec21's mean
/// pivot on the snow (motion 2, 53,020 samples: x −3.95 (−36.7..37.8), y −7.70 (−53.5..14.7), z 70.29 (6.2..130.8))
/// when the recording has no pivot window (a test of what the old recordings' shoves miss, not a stand-in kept)
fn pose_of(t: &recording::Tick, i: usize, k: Option<(Option<usize>, Option<usize>, usize)>, table: &[u8]) -> Option<Pose> {
    let (kp, ki, ka) = k?;
    let clip = u32_le(&t.extra[ka].1[i], 8);
    let skin_child = (u32_le(&t.riders[i], 0x428) == 1 && clip != 0x1fc) || clip == 0x291 || clip == 0x2ab || (clip != animctl::NONE && u32_le(table, clip as usize * 0x1c) == 0x10);
    let pivot = match (kp, ki) {
        (Some(kp), Some(ki)) => {
            if u32_le(&t.extra[ki].1[i], 0) != 0 {
                return None;
            }
            let pb = &t.extra[kp].1[i];
            [f32_at(pb, 8), f32_at(pb, 0xc), f32_at(pb, 0x10)]
        }
        _ if std::env::var("TRICKY_PIVOT").is_ok_and(|v| v == "mean") => [-3.95, -7.70, 70.29],
        _ => return None,
    };
    Some(Pose { pivot, skin_child })
}

/// The rider's frame (+0x4aa0..+0x4ae0) as `Boarder_UpdatePose` 0x129628 built it in `cPostAI::vf1` 0x114730 after the
/// last tick (the sample's attitude, steer, position, height and lean: [`board_frame`]; then its position moved to
/// the pivot bone, [`frame_where`], when the recording has the [`Pose`]); the shove measures its reach in it (F4b3s,
/// F4b3t). rec20: the position right on 70,104 of 71,292 samples, the rest a few ulps off (the rows' own ulps).
// STANDIN: without the pose windows (the recordings before rec20) the position is left at the board's (the pivot
// bone is the hips, ~67 cm up the board and ~25 along it), and the pivot's height 0 in the spin's turn: it only counts
// with a spin or a flip (+0x1d0, +0x1d4), which riders on the snow don't have
fn frame_of(r: &[u8], st: &[u8], pose: Option<Pose>) -> [V4; 4] {
    let mut f = board_frame(&PoseIn {
        quat: vec4(r, 0x170),
        steer: f32_at(r, 0x214),
        motion: u32_le(r, 0x424),
        class: u32_le(r, 0x420),
        pos: vec4(r, 0x140),
        up: vec4(r, 0x2c0),
        height: f32_at(r, 0x308),
        side: vec4(r, 0x330),
        lean: f32_at(r, 0x244),
        lean_scale: f32_at(st, 0x60),
        spin: f32_at(r, 0x1d0),
        flip: f32_at(r, 0x1d4),
        pivot_z: pose.map_or(0.0, |p| p.pivot[2]),
    });
    if let Some(p) = pose {
        f[3] = frame_where(f, p.pivot, p.skin_child, u32_le(r, 0x424), vec4(r, 0x2c0), f32_at(r, 0x1c0), f32_at(st, 0x60));
    }
    f
}

/// One frame's rider collisions (`Boarder_RiderCollisions` 0x123fc8 at 0x117cd4, after the motion, the speed limit
/// and the control) around one replayed rider, `me` (board rows F4h4, F4h5a). The other riders on the snow at both
/// samples (`others`, [`other_of`]) have their ground tick replayed at their turn by `tick` (each way its own: the
/// motion, the speed limit and the control, from where the pushes so far left them), as
/// `ground_ticks_against_the_running_game` replays them; the rest are taken as that test takes the riders it does not
/// replay (the second sample's velocity and motion; bodies as the first sample holds them). Each rider walks its pair
/// list as recorded (`L700`, the second sample's; without it every other rider). `before` runs the riders that tick
/// before `me` in `order` (their pushes and hits move `me` before its motion: its start position and velocity are
/// returned); `after` puts `me` as its motion left it and runs the rest, returning its position and velocity at the
/// frame's end.
#[derive(Clone)]
struct FrameRun {
    f: Frame,
    riders: Vec<ColRider>,
    others: Vec<Option<Other>>,
    lists: Vec<Vec<usize>>,
    me: usize,
    at: usize,
    tick: i32,
}

/// A rider's ground tick replayed by [`FrameRun`]: from its first sample with the inputs the second holds (as
/// `ground_ticks_against_the_running_game` builds it), the ground answer, and the control if its state is one the
/// replay runs.
#[derive(Clone)]
struct Other {
    rider: Rider,
    hit: Option<Hit>,
    ctl: Option<Ctl>,
    /// the course's walls (F4h5m), as for the rider the test compares
    walls: Option<Walls>,
}

/// `Boarder_SetState` 0x11f608 in the ground motion as the game's, ours and the runner's (F4h5m; TRICKY_STATES=stub:
/// only logged).
fn states_on() -> bool {
    std::env::var("TRICKY_STATES").map_or(true, |v| v != "stub")
}

/// A revert in the motion (`Boarder_SetState`(4) while riding backward) enters `ChangeFakieControl_Enter` 0x103eb8,
/// which negates the steer and +0x244 the second sample holds: before it the motion had them the other way round
/// (F4h5h). Found by a dry run of ours.
fn unrevert(r: &mut Rider, hit: Option<Hit>, walls: &Option<Walls>) {
    if !states_on() {
        return;
    }
    let mut dry = *r;
    let mut e = E { log: Vec::new(), hit, wall: false, meter: 0.0, states: false, walls: walls.clone() };
    update(&mut dry, &SURFACE_TABLE, &mut e);
    if e.log.contains(&Ev::State(4)) {
        (r.steer, r.p.x244) = (-r.steer, -r.p.x244);
    }
}

/// What a replayed ground tick does to a rider (`Other`) starting at a position and velocity: each way's own.
type Ticker<'a> = dyn FnMut(usize, &Other, V4, V4, f32) -> Rider + 'a;

/// Whether the rider moved about one tick's step between the samples (else the samples are not one update apart).
fn one_step(a: &[u8], b: &[u8]) -> bool {
    let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
    let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
    let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
    e > 1.0 && d > 0.7 * e && d < 1.4 * e
}

/// Windows a ground tick's replay reads: the motion object (`-400`, or `-3d0` for the rider whose state manager
/// sits 0x30 further on) and the animation controller (`46c0`).
#[derive(Clone, Copy)]
struct Wins {
    km: usize,
    km6: Option<usize>,
    ka: usize,
}

/// Rider `i`'s ground tick between two samples, when it is on the snow at both, its motion object is its own and
/// the samples are one update apart (F4h5a).
fn other_of(t0: &recording::Tick, t1: &recording::Tick, i: usize, st: &[u8], addr: u32, x: Wins, course: &Option<std::sync::Arc<tricky_game::course_ground::CourseGround>>) -> Option<Other> {
    let (a, b) = (&t0.riders[i], &t1.riders[i]);
    // (F4b3x) a rider wiped out by a hit in the frame's collisions ends it in motion 5: its ground tick ran first.
    // `WipeoutMotion_Enter` 0x10c198 keeps the surface at +0x310 (+0x290 becomes 10) and blends the clips out
    // (the control's clips taken from the first sample then)
    let wiped = u32_le(b, 0x424) == 5;
    if u32_le(a, 0x424) != 2 || !(u32_le(b, 0x424) == 2 || wiped) || !one_step(a, b) {
        return None;
    }
    let k = match x.km6 {
        Some(k6) if u32_le(&t0.extra[x.km].1[i], 0xc) != addr => k6,
        _ => x.km,
    };
    let (ma, mb) = (&t0.extra[k].1[i], &t1.extra[k].1[i]);
    if u32_le(ma, 0xc) != addr {
        return None;
    }
    let (na, nb) = (&t0.extra[x.ka].1[i], &t1.extra[x.ka].1[i]);
    let mut rider = rider_of(a, st, ma, na);
    let after = rider_of(b, st, mb, nb);
    (rider.brake, rider.crouch, rider.steer, rider.p.x244, rider.d1, rider.d2) = (after.brake, after.crouch, after.steer, after.p.x244, after.d1, after.d2);
    let kind = u32_le(b, 0x2e0);
    let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, if wiped { 0x310 } else { 0x290 }) });
    // the control as the ground test runs it (clips from the second sample, sequences from the first; the pad word
    // the hook kept, F4b3k; STANDIN without it: the pad word's brake bit only, from the state the tick ends in); not
    // the fakie cruise on its turn clip
    let hooked = hook_pad(t1, i);
    let ctl = (CONTROLS.contains(&rider.state) || hooked.is_some() && PADDED.contains(&rider.state))
        .then(|| ctl_of(t0, t0, i, x.ka, addr, hooked.unwrap_or(if u32_le(b, 0x428) == 0xe { 4 } else { 0 }), b)).flatten()
        .filter(|c| !(rider.state == 5 && c.clips[0] == 0x202))
        .map(|mut c| {
            let n = if wiped { na } else { nb };
            c.clips = [u32_le(n, 8), u32_le(n, 0xc)];
            c
        });
    Some(Other { rider, hit, ctl, walls: walls_of(course, a, st) })
}

/// A replayed ground tick, ours: `ground_motion::update`, the speed limit, the control (as the ground test).
fn tick_ours(x: &Other, pos: V4, vel: V4, boost: f32, a: &[u8], b: &[u8], table: &[u8]) -> Rider {
    let mut r = x.rider;
    (r.p.pos, r.vel, r.boost_level) = (pos, vel, boost);
    unrevert(&mut r, x.hit, &x.walls);
    update(&mut r, &SURFACE_TABLE, &mut E { log: Vec::new(), hit: x.hit, wall: false, meter: 0.0, states: states_on(), walls: x.walls.clone() });
    r.vel = speed_limit(r.vel, speed_limit_at(a, b, boost, |x, y| x - y));
    if let Some(c) = &x.ctl {
        control_ours(&mut r, c, table);
    }
    r
}

/// A replayed ground tick, the game's in the runner: `GroundMotion_Update` 0x10a0d8, the speed limit under its
/// rules, `Boarder_UpdateControl` 0x11c9a0.
fn tick_runner(g: &mut W, x: &Other, pos: V4, vel: V4, boost: f32, a: &[u8], b: &[u8], addr: u32) -> Rider {
    let mut rider = x.rider;
    (rider.p.pos, rider.vel, rider.boost_level) = (pos, vel, boost);
    unrevert(&mut rider, x.hit, &x.walls);
    *g.hit.borrow_mut() = x.hit;
    g.wall.set(false);
    g.put(&rider);
    g.put_walls(x.walls.as_ref());
    let was = g.states_real;
    if !was {
        g.real_states(states_on());
    }
    g.r.cpu.set_gpr(4, g.motion as u64);
    g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("another rider's ground tick: {e:?}"));
    if !was {
        g.real_states(false);
    }
    let mut r = g.get(&rider);
    r.vel = speed_limit(r.vel, speed_limit_at(a, b, boost, r5900::ps2float::fsub));
    if let Some(c) = &x.ctl {
        g.control(&mut r, c, addr);
    }
    r
}

/// A rider that leaves the snow in the frame (motion 2 → 1), replayed by the ground test so the pushes and hits it
/// gives the riders that tick after it use the velocity it has then (board row F4h5g): its ground tick as
/// `takeoff_ticks_against_the_running_game` runs it, from the first sample with the second's inputs.
#[derive(Clone)]
struct Lift {
    rider: Rider,
    /// the course's walls (F4h5m)
    walls: Option<Walls>,
    hit: Option<Hit>,
    /// +0x1c8 at the first sample (the crouch the jump measures)
    crouch0: f32,
    /// stats+0x17
    jump_stat: u8,
    /// +0x1dc at the first sample
    x1dc: u32,
}

impl Lift {
    /// The jump control fires on this tick: state 0xa and the crouch unwound to 0 before the motion.
    fn jumps(&self) -> bool {
        self.rider.state == 0xa && self.rider.crouch == 0.0
    }

    /// Ours: `ground_motion::update`; on a take-off in it `Boarder_TakeOff` 0x128470 (`orient::take_off`), else
    /// when the jump control fires the speed limit and `JumpControl_Update` 0x1046b0 (`control::jump`). The spin
    /// state's first update after a take-off from the jump state (F4b3f) turns only the orientation: not run.
    fn ours(&self, pos: V4, vel: V4, a: &[u8], b: &[u8]) -> Rider {
        let mut mine = self.rider;
        (mine.p.pos, mine.vel) = (pos, vel);
        unrevert(&mut mine, self.hit, &self.walls);
        let mut e = E { log: Vec::new(), hit: self.hit, wall: false, meter: 0.0, states: states_on(), walls: self.walls.clone() };
        update(&mut mine, &SURFACE_TABLE, &mut e);
        let mut t = TakeOffRider { state: mine.state, crouch: self.crouch0, x208: mine.crouch };
        let mut env = OurTakeOff { r: &mut mine, motion: 2, jump_stat: self.jump_stat, off: Off::default(), steep: false };
        if e.log.contains(&Ev::TakeOff) {
            take_off(&mut t, &mut env);
        } else if self.jumps() {
            env.r.vel = speed_limit(env.r.vel, speed_limit_of(a, b, |x, y| x - y));
            let mut cr = control::Rider { crouch: env.r.crouch, surface: env.r.p.surface, ..Default::default() };
            control::jump(&mut OurJump { t, env: &mut env, steer: 0 }, 0, &mut cr);
        }
        mine
    }

    /// The game's in the runner: `GroundMotion_Update` 0x10a0d8, then `Boarder_TakeOff` 0x128470 or the speed limit
    /// and `JumpControl_Update` 0x1046b0 (the runner as [`lift_world`] sets it up).
    fn runner(&self, g: &mut W, pos: V4, vel: V4, a: &[u8], b: &[u8]) -> Rider {
        let mut rider = self.rider;
        (rider.p.pos, rider.vel) = (pos, vel);
        unrevert(&mut rider, self.hit, &self.walls);
        *g.hit.borrow_mut() = self.hit;
        g.wall.set(false);
        g.log.borrow_mut().clear();
        g.put(&rider);
        g.r.mem.write_u32(g.rd + 0x424, 2);
        g.r.mem.write_f32(g.rd + 0x1c8, self.crouch0);
        g.r.mem.write_u32(g.rd + 0x1dc, self.x1dc);
        g.r.mem.write_u8(g.stats + 0x17, self.jump_stat);
        g.r.cpu.set_gpr(4, g.motion as u64);
        g.put_walls(self.walls.as_ref());
        let was = g.states_real;
        if !was {
            g.real_states(states_on());
        }
        g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("a take-off's ground tick: {e:?}"));
        if !was {
            g.real_states(false);
        }
        if g.log.borrow().contains(&Ev::TakeOff) {
            g.r.cpu.set_gpr(4, g.rd as u64);
            g.r.call(0x128470).unwrap_or_else(|e| panic!("a take-off: {e:?}"));
        } else if self.jumps() {
            let v = speed_limit(rv(&mut g.r.mem, g.rd + 0x150), speed_limit_of(a, b, r5900::ps2float::fsub));
            wv(&mut g.r.mem, g.rd + 0x150, v);
            // the handler's word 0 is the rider, as the jump control's object's
            g.r.mem.write_u32(g.pad, 0);
            g.r.cpu.set_gpr(4, g.handler as u64);
            g.r.cpu.set_gpr(5, g.pad as u64);
            g.r.call(0x1046b0).unwrap_or_else(|e| panic!("the jump control: {e:?}"));
        }
        g.get(&rider)
    }
}

/// What a [`Lift`] needs of the runner beyond `world`: `Boarder_SetMotion` 0x11f610 sets +0x424,
/// `Takeoff_SetSpinRates` 0x126e30 and `Score_StartTrickChain` 0x156618 answer nothing, `Jump_ApplyImpulse`
/// 0x1284e0, `JumpControl_Update` 0x1046b0 and `Math_Sin` 0x250d60 run.
fn lift_world(w: &mut W) {
    let rd = w.rd;
    w.r.stub(0x11f610, move |cpu, mem| mem.write_u32(rd + 0x424, cpu.gpr(5) as u32));
    w.r.stub(0x126e30, |_, _| {});
    w.r.stub(0x156618, |c, _| c.set_f(0, 0.0));
    for f in [0x1284e0, 0x1046b0, 0x250d60] {
        w.r.allow(f);
    }
}

impl FrameRun {
    #[allow(clippy::too_many_arguments)]
    fn before(wp: Option<&[Vec<u8>]>, w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], twins: &Twins, add: Add, clips: Option<(&[Vec<u8>], &[u8])>, poses: &[Option<Pose>], flags: &[Option<bool>], order: &[usize], tick: i32, me: usize, others: Vec<Option<Other>>, lists: Vec<Vec<usize>>, t: &mut Ticker) -> (FrameRun, V4, V4) {
        let n = w0.len();
        let mut run = FrameRun { f: Frame::new(wp, w0, w1, stats, twins, tick as u32 - 1, add, clips, flags), riders: w0.iter().enumerate().map(|(i, x)| col_rider(x, n, &stats[i], poses[i])).collect(), others, lists, me, at: 0, tick };
        while order[run.at] != me {
            run.step(w0, w1, stats, order[run.at], None, t);
            run.at += 1;
        }
        let (pos, vel) = (run.riders[me].pos, run.riders[me].vel);
        (run, pos, vel)
    }

    #[allow(clippy::too_many_arguments)]
    fn after(mut self, w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], order: &[usize], r: &Rider, motion: u32, t: &mut Ticker) -> (V4, V4, usize, usize) {
        for k in self.at..order.len() {
            let o = order[k];
            self.step(w0, w1, stats, o, (o == self.me).then_some((r, motion)), t);
        }
        (self.riders[self.me].pos, self.riders[self.me].vel, self.f.pushes[self.me], self.f.hits[self.me])
    }

    fn step(&mut self, w0: &[Vec<u8>], w1: &[Vec<u8>], stats: &[Vec<u8>], o: usize, mine: Option<(&Rider, u32)>, t: &mut Ticker) {
        let (a, b) = (&w0[o], &w1[o]);
        let (f, riders) = (&mut self.f, &mut self.riders);
        let replayed = match (mine, &self.others[o]) {
            (Some((r, motion)), _) => Some((*r, motion)),
            (None, Some(x)) => Some((t(o, x, riders[o].pos, riders[o].vel, if f.stumbled[o] { 0.0 } else { f32_at(a, 0x130) }), 2)),
            (None, None) => None,
        };
        if let Some((r, motion)) = replayed {
            riders[o].pos = r.p.pos;
            riders[o].vel = r.vel;
            riders[o].motion = motion;
            f.hit[o] = HitRider { heading: r.yaw, pos: r.p.pos, vel: r.vel, normal: r.p.normal, motion, crouch: f32_at(b, 0x1c8) };
        } else {
            if u32_le(a, 0x424) == 1 && u32_le(b, 0x424) == 1 && std::env::var("TRICKY_AIRSTEP").is_ok() {
                let (p, q) = (vec4(a, 0x140), vec4(b, 0x140));
                f.body[o].translate([q[0] - p[0], q[1] - p[1], q[2] - p[2], q[3] - p[3]]);
            }
            riders[o].vel = vec4(b, 0x150);
            riders[o].motion = u32_le(b, 0x424);
            f.hit[o] = hit_rider(b);
        }
        f.stat[o] = if f.stumbled[o] { f.stat0[o] } else { speed_stat(&stats[o], boost_at_collisions(a, b), f.add) };
        rider_collisions(o, &self.lists[o], riders, self.tick, false, f);
    }
}

/// The course's keyframed objects over the recording (F4h5l, `course_clock`): on each frame the objects whose
/// `cAnimObjectNode` the cells and scripts made and its clip's time, by our clock (IEEE) and by PCSX2's adder (the
/// running game's, `Round::Guard`); and the parts' curves (`meshanim`). TRICKY_KEYFRAMED=rest: none posed (their rest
/// pose, as before F4h5l); =list: the samples where the live ones change, listed.
struct Keyframed {
    ieee: course_clock::Clocks,
    pcsx2: course_clock::Clocks,
    /// each object's animated parts' curves (the model's hierarchy in order)
    anims: HashMap<usize, Vec<tricky_data::anim::ObjAnim>>,
    /// ours: the objects posed now and their frames' channels (`AnimFrame_SetKey`'s, then each sample's)
    ours: RefCell<HashMap<usize, Vec<[f32; 16]>>>,
}

impl Keyframed {
    fn of(course: &tricky_game::course_ground::CourseGround, files: &(Vec<u8>, Vec<u8>, Vec<u8>), ticks: &[recording::Tick], n: usize) -> Option<Keyframed> {
        if std::env::var("TRICKY_KEYFRAMED").is_ok_and(|v| v == "rest") {
            return None;
        }
        let pbd = tricky_data::pbd::Pbd::parse(&files.0)?;
        let ssf = tricky_data::ssf::Ssf::parse(&files.2, pbd.instances.len() as u32).ok()?;
        let samples: Vec<(u32, Vec<V4>)> = ticks.iter().map(|t| (t.tick, (0..n).map(|i| vec4(&t.riders[i], 0x140)).collect())).collect();
        let (ieee, pcsx2) = course.with_world(|w| {
            (course_clock::clocks(&pbd, &ssf, w, &samples, |a, b| a + b), course_clock::clocks(&pbd, &ssf, w, &samples, |a, b| r5900::ps2float::add_by(a, b, r5900::ps2float::Round::Guard)))
        });
        let mut anims = HashMap::new();
        for o in ieee.values().chain(pcsx2.values()).flatten().map(|x| x.0) {
            anims.entry(o).or_insert_with(|| {
                let model = &pbd.models[pbd.instances[o][0x30] as usize];
                model.nodes.iter().filter_map(|n| n.animation.as_ref()).map(|a| tricky_data::level::node_animation(&files.0, a).unwrap()).collect()
            });
        }
        if std::env::var("TRICKY_KEYFRAMED").is_ok_and(|v| v == "list") {
            let mut last: Vec<usize> = vec![];
            for (k, l) in &pcsx2 {
                let now: Vec<usize> = l.iter().map(|x| x.0).collect();
                if now != last {
                    eprintln!("KEYFRAMED sample {k} (tick {}): {l:?}", ticks[*k].tick);
                    last = now;
                }
            }
        }
        Some(Keyframed { ieee, pcsx2, anims, ours: RefCell::new(HashMap::new()) })
    }

    /// Ours for the motion of the frame ending at sample `frame`: each live object's parts at its time (`CourseGround::pose`, the frames by
    /// `meshanim::mesh_frame`), the ones gone back at rest.
    fn pose_ours(&self, course: &tricky_game::course_ground::CourseGround, frame: usize) {
        let live = self.ieee.get(&frame).cloned().unwrap_or_default();
        let mut ours = self.ours.borrow_mut();
        let gone: Vec<usize> = ours.keys().filter(|o| !live.iter().any(|x| x.0 == **o)).copied().collect();
        for o in gone {
            ours.remove(&o);
            course.unpose(o as u32);
        }
        for (o, t) in live {
            let anims = &self.anims[&o];
            let ch = ours.entry(o).or_insert_with(|| {
                anims.iter().map(|a| {
                    let mut c = tricky_game::meshanim::key_channels(a);
                    let _ = tricky_game::meshanim::mesh_frame(a, t, &mut c);
                    c
                }).collect()
            });
            let frames = anims.iter().zip(ch.iter_mut()).map(|(a, c)| tricky_game::meshanim::mesh_frame(a, t, c)).collect();
            course.pose(o as u32, frames);
        }
    }

    /// The runner's for the motion of the frame ending at sample `frame` (`RunnerCourse::pose`: the game's frame code under the runner's
    /// rules), by PCSX2's clock under `fadd=g1`, else ours.
    fn pose_runner(&self, w: &mut W, frame: usize) {
        let Some(rc) = &w.rc else { return };
        let clocks = if w.r.float_rules.fadd == r5900::ps2float::Round::Guard { &self.pcsx2 } else { &self.ieee };
        let live = clocks.get(&frame).cloned().unwrap_or_default();
        let posed: Vec<usize> = rc.nodes.borrow().keys().copied().collect();
        for o in posed {
            if !live.iter().any(|x| x.0 == o) {
                rc.pose(&mut w.r, o, None);
            }
        }
        for (o, t) in live {
            rc.pose(&mut w.r, o, Some(t));
        }
    }
}

fn w0_len(ticks: &[recording::Tick]) -> usize {
    ticks.first().map_or(0, |t| t.riders[0].len())
}

/// `Boarder_WallCollide` 0x126250's cast, bit for bit (F4h5m): for the bodies a recording holds (every `TRICKY_WALL_STEP`th
/// tick, 10 unless set; riders on the snow, motion 2), ours ([`wall_cast`]: `CourseGround::ray_cast` with
/// `WorldBoarder`) against the original's in the runner on the course its loaders built (`runner_course`):
/// `cWorldBoarder::cWorldBoarder` 0x257810 on the body (mask `WALL_MASK`, as the ground motion sets it) with the
/// ground normal to reject near (+0x2d0 = 1, +0x2e0), then `World_RayCast` 0x25aff8; t, the point, the normal and the
/// object compared. Needs TRICKY_RECORDING (with the course's id) and TRICKY_DISC.
#[test]
fn wall_casts_against_the_original() {
    let Ok(paths) = std::env::var("TRICKY_RECORDING") else { return };
    let path = paths.split(';').next().unwrap();
    let (Some(course), Some(files)) = (course(path), course_files(path)) else { return };
    let (n, ticks) = load(path);
    let step: usize = std::env::var("TRICKY_WALL_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    let Some(mut w) = world() else { return };
    w.load_course(&files, &course);
    let keyframed = Keyframed::of(&course, &files, &ticks, n);
    let shape = w.r.mem.alloc(0x400);
    let out = w.r.mem.alloc(0x40);
    let (mut casts, mut hits, mut same) = (0, 0, 0);
    let mut off: Vec<String> = Vec::new();
    for (ti, t) in ticks.iter().enumerate().step_by(step) {
        // the body at the end of this frame is cast in the next (the next sample's, when it is the next frame): the
        // keyframed objects as they are there (F4h5l)
        if let Some(k) = &keyframed {
            let next = ticks.get(ti + 1).filter(|x| x.tick == t.tick + 1).map_or(usize::MAX, |_| ti + 1);
            k.pose_ours(&course, next);
            k.pose_runner(&mut w, next);
        }
        for i in 0..n {
            let a = &t.riders[i];
            if a.len() < 0x700 || u32_le(a, 0x424) != 2 {
                continue;
            }
            let mut body = body_of(a);
            body.x24 = tricky_game::ground_motion::WALL_MASK as i32;
            let ground = vec4(a, 0x2a0);
            let (t_ours, rec) = wall_cast(&course, &body, ground);
            let (m, rd) = (&mut w.r.mem, w.rd);
            wv(m, rd + 0x480, body.center);
            m.write_f32(rd + 0x490, body.radius);
            m.write_u32(rd + 0x494, body.x24 as u32);
            m.write_u32(rd + 0x498, body.count as u32);
            for (k, (c, r)) in body.spheres.iter().enumerate() {
                wv(m, rd + 0x4a0 + 0x20 * k as u32, *c);
                m.write_f32(rd + 0x4b0 + 0x20 * k as u32, *r);
            }
            w.r.cpu.set_gpr(4, shape as u64);
            w.r.cpu.set_gpr(5, (rd + 0x470) as u64);
            w.r.cpu.set_gpr(6, 1);
            w.r.call(0x257810).unwrap();
            w.r.mem.write_u32(shape + 0x2d0, 1);
            wv(&mut w.r.mem, shape + 0x2e0, ground);
            for o in (0..0x40).step_by(4) {
                w.r.mem.write_u32(out + o, 0);
            }
            w.r.cpu.set_gpr(4, runner_course::W as u64);
            w.r.cpu.set_gpr(5, shape as u64);
            w.r.cpu.set_gpr(6, out as u64);
            w.r.cpu.set_gpr(7, 0);
            w.r.call(0x25aff8).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", t.tick));
            let t_game = w.r.cpu.f(0);
            casts += 1;
            hits += (0.0 < t_ours || 0.0 < t_game) as usize;
            let m = &mut w.r.mem;
            let obj = m.read_u32(out + 0x38);
            let game = (t_game.to_bits(), rv(m, out + 0x10).map(f32::to_bits), rv(m, out + 0x20).map(f32::to_bits), obj);
            let ours = (t_ours.to_bits(), rec.pos.map(f32::to_bits), rec.normal.map(f32::to_bits), rec.object);
            let right = if 0.0 < t_game || 0.0 < t_ours { (game.0, game.1, game.2) == (ours.0, ours.1, ours.2) && (obj == 0) == (rec.object == 0) } else { true };
            same += right as usize;
            if !right && off.len() < 20 {
                off.push(format!("tick {} rider {i}: ours t {t_ours} n {:?} object {}, the game's t {t_game} n {:?} object {obj:#x}", t.tick, rec.normal, rec.object, rv(m, out + 0x20)));
            }
        }
    }
    for o in &off {
        eprintln!("  {o}");
    }
    eprintln!("{path}: wall casts {casts} ({hits} hit either way), the same bit for bit {same}");
    assert!(casts > 0);
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
/// Not in the recording, so not given: the walls (`Boarder_WallCollide` 0x126250 answers no wall; ours casts at the
/// course with TRICKY_DISC and TRICKY_COURSE, F4h5j) and the instances
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
    let course = course(path);
    // the knocked objects among each rider's pairs, when recorded (F4b3k3)
    let roll_base = recording::instance_base(path);
    let files = course_files(path);
    let twins = Twins::of_recording(path, &ticks);
    let addrs = rider_addrs(path);
    let (Some(km), Some(ka)) = (ticks[0].extra.iter().position(|(o, _)| *o == -0x400), ticks[0].extra.iter().position(|(o, _)| *o == 0x46c0)) else {
        eprintln!("{path}: no -400 and 46c0 windows: skipping");
        return;
    };
    // the rider whose state manager is at rider − 0x880: its ground motion object is 0x30 further on (window `-3d0:10`)
    let km6 = ticks[0].extra.iter().position(|(o, _)| *o == -0x3d0);
    let kpose = pose_windows(&ticks);
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners = Vec::new();
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut w) = world() else { return };
        w.r.float_rules = Rules::parse(rules).unwrap();
        // the walls' cast as the original's on the course built in the runner (F4h5m; TRICKY_WALLCAST=ours: ours)
        if let (Some(f), Some(c), true) = (&files, &course, std::env::var("TRICKY_WALLCAST").map_or(true, |v| v != "ours")) {
            w.load_course(f, c);
        }
        lift_world(&mut w);
        runners.push((rules, w));
    }
    let keyframed = match (&course, &files) {
        (Some(c), Some(f)) => Keyframed::of(c, f, &ticks, n),
        _ => None,
    };
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
    let lifts = std::env::var("TRICKY_NOLIFT").is_err();
    let mut lifted = 0;
    let mut wiped_in = 0;
    // the riders in the air the whole frame, their flight step replayed: [frames, velocity right, position right] by way
    let mut flights = vec![[0usize; 3]; 1 + runners.len()];
    let mut slides = vec![[0usize; 3]; 1 + runners.len()];
    // tumbling the whole frame (F4b3x): [frames, velocity right]
    let mut tumbles = [0usize; 2];
    let mut getups = vec![[0usize; 3]; 1 + runners.len()];
    let mut landings = vec![[0usize; 3]; 1 + runners.len()];
    let kt = ticks[0].chased.iter().position(|(s, _)| s == "5ae0>4f0").filter(|_| std::env::var("TRICKY_TUMBLE").map_or(true, |v| v != "off"));
    let mut missed_all = [0usize; 3];
    // pushed ticks under fadd=g1 by whether the rider was also hit (the game's bump record +0x40 + 0x20·j +8 holds
    // the second sample's tick): [hit][position right], and of the hit ones how many have the velocity right
    let mut pushed_pos = [[0usize; 2]; 2];
    let mut hit_vel = 0;
    // the replayed frame's bumps against the game's records (pairs whose record either side changed this frame)
    let (mut bumps, mut bumps_right) = (0, 0);
    let (mut shoves, mut shoves_right) = (0, 0);
    // the clip flags recorded (F4b3t): [rider-ticks known, the game shoved, of those between the flags, between the
    // flags on a shove clip (type 0x15) with no shove]
    let mut flag_count = [0usize; 4];
    // The wipe-out motion's flight predictions (F4h5b): `Boarder_UpdateMotion` 0x11c770's motion 5 re-makes the
    // prediction (`AirPredict_AtTakeoff` 0x123b10 at rider+0x57b0, window `57b0:64`) from the rider as
    // `Ragdoll_Step` left it, and the rider's collisions after it (its own, 0x117cd4, and those of the riders that tick
    // after it) still push it: [made, from the end of the tick, from the end less the pushes after its motion]. A
    // prediction made has +0x4c 0 and +8 = +0x44 (`AirPredict_ClearLandTime` on a touch zeroes only +0x44)
    let kp = ticks[0].extra.iter().position(|(o, _)| *o == 0x57b0);
    let mut wiped = [0usize; 3];
    let mut stale_bodies = 0;
    // tumbling riders in a replayed frame whose body was not recorded (no `5ae0>4f0` window: rec14, rec16)
    let mut bodiless = 0;
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
    let (mut ctl_turn, mut ctl_none, mut ctl_right, mut player_ctl, mut hook_ctl) = (0, 0, 0, 0, 0);
    let mut ctl_same = vec![0usize; runners.len()];
    // and of those, the quaternion equal to the game's: ours, the runner's
    let (mut ctl_ours_game, mut ctl_runner_game) = (vec![0usize; runners.len()], vec![0usize; runners.len()]);
    // the course objects' flags as the game had them over the race (F4b3k4, `pine.py record ... flags`): the first
    // sample's (TRICKY_OBJFLAGS=b: the second's, which holds the frame's own changes, the same on rec24; off: as
    // loaded)
    let flag_way = std::env::var("TRICKY_OBJFLAGS").unwrap_or_default();
    FLAGGED.store(ticks.first().is_some_and(|t| !t.flags.is_empty()) && flag_way != "off", std::sync::atomic::Ordering::Relaxed);
    for (wi, w) in ticks.windows(2).enumerate() {
        if let (Some(c), false) = (&course, flag_way == "off") {
            for t in if flag_way == "b" { &w[..] } else { &w[..1] } {
                for &(o, fl) in &t.flags {
                    c.set_flags(o as usize, fl);
                    for (_, g) in runners.iter_mut() {
                        g.set_object_flags(c, o as usize, fl);
                    }
                }
            }
        }
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        // the keyframed objects where the game had them for this frame's casts (F4h5l)
        if let (Some(k), Some(c)) = (&keyframed, &course) {
            k.pose_ours(c, wi + 1);
            for (_, g) in runners.iter_mut() {
                k.pose_runner(g, wi + 1);
            }
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
        // the riders that leave the snow in the frame, replayed for their pushes and hits (F4h5g; TRICKY_NOLIFT: taken
        // from the second sample), not compared (the take-off test does)
        let mut lift: Vec<Option<Lift>> = vec![None; n];
        // (F4b3x) the riders a hit in this frame's collisions wipes out (motion 2 to 5): their ground tick replayed
        // (TRICKY_NOWIPE: as the others)
        let wipe: Vec<Option<Other>> = (0..n)
            .map(|i| (u32_le(&w[1].riders[i], 0x424) == 5 && std::env::var("TRICKY_NOWIPE").is_err()).then(|| other_of(&w[0], &w[1], i, &stats[i], addr[i], Wins { km, km6, ka }, &course)).flatten())
            .collect();
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            if u32_le(a, 0x424) == 2 && u32_le(b, 0x424) == 1 && lifts {
                let k = match km6 {
                    Some(k6) if u32_le(&w[0].extra[km].1[i], 0xc) != addr[i] => k6,
                    _ => km,
                };
                let (ma, mb) = (&w[0].extra[k].1[i], &w[1].extra[k].1[i]);
                if u32_le(ma, 0xc) == addr[i] && one_step(a, b) {
                    let (na, nb) = (&w[0].extra[ka].1[i], &w[1].extra[ka].1[i]);
                    let mut rider = rider_of(a, &stats[i], ma, na);
                    let after = rider_of(b, &stats[i], mb, nb);
                    (rider.brake, rider.crouch, rider.steer, rider.p.x244, rider.d1, rider.d2) = (after.brake, after.crouch, after.steer, after.p.x244, after.d1, after.d2);
                    let kind = u32_le(b, 0x2e0);
                    let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
                    lift[i] = Some(Lift { rider, walls: walls_of(&course, a, &stats[i]), hit, crouch0: f32_at(a, 0x1c8), jump_stat: stats[i][0x17], x1dc: u32_le(a, 0x1dc) });
                }
            }
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
            // F4b3k: the tick's own pad word, where the recorder hooked it (TRICKY_NOHOOK: not used)
            let hooked = hook_pad(&w[1], i).filter(|_| std::env::var("TRICKY_NOHOOK").is_err());
            let ctl = if (CONTROLS.contains(&rider.state) || hooked.is_some() && PADDED.contains(&rider.state)) && std::env::var("TRICKY_NOCTL").is_err() {
                // TRICKY_PAD=b: the player's from the recorded pad (F4b3d, `player_pad`: the pad read at the second
                // sample; =a the first's). Not by default: rec16 6182 r5's bump goes to prewind (the pad's bit 4, X
                // held) while every recorded pad of 6170..6190 has X up (rec16: 678 of the player's 1,535 prewind
                // samples show no X; rec15 5 of 1,199)
                let read = if !std::env::var("TRICKY_PAD").is_ok_and(|v| v == "a" || v == "b") {
                    None
                } else if std::env::var("TRICKY_PAD").is_ok_and(|v| v == "a") { wi.checked_sub(1).map(|k| (ticks[k].pad.as_deref(), w[0].pad.as_deref())) } else { Some((w[0].pad.as_deref(), w[1].pad.as_deref())) };
                let mine = read.and_then(|(p, now)| player_pad(&stats[i], p, now?, rider.state));
                if mine.is_some() {
                    player_ctl += 1;
                }
                if hooked.is_some() {
                    hook_ctl += 1;
                }
                let pad = hooked.or(mine).unwrap_or(if u32_le(b, 0x428) == 0xe { 4 } else { 0 });
                match ctl_of(if anim_b { &w[1] } else { &w[0] }, &w[0], i, ka, addr[i], pad, b) {
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
        // second sample (velocity, motion; its body stays as the first sample holds it).
        let tick = w[1].tick as i32;
        let mut got: Vec<Vec<Vec<Vec<u32>>>> = vec![Vec::new(); n];
        let mut frame_pushes = vec![0usize; n];
        let mut frame_hits = vec![0usize; n];
        let mut frame_stale = vec![false; n];
        // The riders not replayed tick with the second sample's velocity, which already holds the hits they took
        // later in the frame (F4b3s: rec14 6925, rider 5 wiped out, hitting rider 1 with them). Trial passes used to
        // take our hits off it again (TRICKY_PREHIT, F4b3s..F4b3x3); since every kind of rider whose velocity they
        // fixed is now replayed (F4b3x..F4b3x3) they are gone (F4b3x4): what is left are the tumbling riders of the
        // recordings without their body (`5ae0>4f0`: rec14, rec16), counted below.
        // STANDIN: the second sample's velocity, for the velocity the game's motion left them (not recorded)
        // a rider whose own motion is replayed (in the air, motion 4, sliding) but ends the frame more than 1 cm/s
        // from the second sample in some axis (a wall, a landing, a get-up: not replayed) is missed: the frame runs
        // again with it as the others
        let mut missed = vec![false; n];
        let states = states_on();
        let air = std::env::var("TRICKY_NOAIR").is_err();
        // (TRICKY_AIRPOS=off: the flight step's position not taken; its body stays as the first sample holds it either way)
        let air_pos = std::env::var("TRICKY_AIRPOS").map_or(true, |v| v != "off");
        let mut step = 0;
        let flags: Vec<Option<bool>> = (0..n).map(|o| shoving_of(w, o, ka)).collect();
        for (i, fl) in flags.iter().enumerate() {
            let Some(fl) = *fl else { continue };
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            let shoved = (0..n).any(|j| j != i && u32_le(b, 0x4c + 0x20 * j) == w[0].tick + 1 && b[0x5a + 0x20 * j..][..2] != a[0x5a + 0x20 * j..][..2]);
            let c1 = u32_le(&w[1].extra[ka].1[i], 0xc);
            let shove_clip = c1 != animctl::NONE && u32_le(&table, c1 as usize * 0x1c) == 0x15;
            flag_count[0] += 1;
            flag_count[1] += shoved as usize;
            flag_count[2] += (shoved && fl) as usize;
            flag_count[3] += (!shoved && fl && shove_clip) as usize;
        }
        while step < 1 + runners.len() {
            let k = step;
            let mut f = Frame::new((wi > 0 && ticks[wi - 1].tick + 1 == w[0].tick).then(|| &ticks[wi - 1].riders[..]), &w[0].riders, &w[1].riders, &stats, &twins, w[0].tick, if k == 0 { adder("") } else { adder(runners[k - 1].0) }, Some((&w[1].extra[ka].1[..], &table[..])), &flags);
            stale_bodies += if k > 0 { 0 } else { f.stale.iter().filter(|x| **x).count() };
            if k == 0 {
                frame_stale = f.stale.clone();
            }
            let mut riders: Vec<ColRider> = w[0].riders.iter().enumerate().map(|(i, r)| col_rider(r, n, &stats[i], pose_of(&w[0], i, kpose, &table))).collect();
            let mut done: Vec<Option<(Rider, f32)>> = vec![None; n];
            // where the riders not replayed were before their collisions (position, velocity)
            let mut snap: Vec<Option<(V4, V4)>> = vec![None; n];
            let mut flown = vec![false; n];
            let mut slid = vec![false; n];
            let mut tumbled = vec![false; n];
            let mut bare = vec![false; n];
            let mut rose = vec![false; n];
            let mut landed = vec![false; n];
            for &me in &order {
                let (a, b) = (&w[0].riders[me], &w[1].riders[me]);
                // a rider an earlier one stumbled out of the cruise in this frame ticks with the boost off (F4b3r)
                let boost = if f.stumbled[me] { 0.0 } else { f32_at(a, 0x130) };
                if let Some(p) = &prep[me] {
                    let mut rider = p.rider;
                    rider.p.pos = riders[me].pos;
                    rider.vel = riders[me].vel;
                    rider.boost_level = boost;
                    unrevert(&mut rider, p.hit, &walls_of(&course, a, &stats[me]));
                    let r = if k == 0 {
                        let mut mine = rider;
                        // the rollers as the riders meet them: the controllers (list 3, `cRollerNode`) step before the
                        // riders (list 8), so the second sample's (TRICKY_ROLLERS=a: the first's) (F4b3k1)
                        let mut walls = walls_of(&course, a, &stats[me]);
                        if let (Some(wl), Some(base)) = (&mut walls, roll_base) {
                            let at = if std::env::var("TRICKY_ROLLERS").as_deref() == Ok("a") { 0 } else { 1 };
                            wl.rollers = rollers_of(&w[at], me, &wl.course, base);
                            wl.stat = speed_stat(&stats[me], boost, f.add);
                        }
                        update(&mut mine, &SURFACE_TABLE, &mut E { log: Vec::new(), hit: p.hit, wall: false, meter: 0.0, states, walls });
                        mine.vel = speed_limit(mine.vel, speed_limit_at(a, b, boost, |x, y| x - y));
                        if let Some(c) = &p.ctl {
                            control_ours(&mut mine, c, &table);
                        }
                        mine
                    } else {
                        let g = &mut runners[k - 1].1;
                        *g.hit.borrow_mut() = p.hit;
                        g.wall.set(false);
                        g.put(&rider);
                        g.put_walls(walls_of(&course, a, &stats[me]).as_ref());
                        g.real_states(states);
                        g.r.cpu.set_gpr(4, g.motion as u64);
                        g.r.call(0x10a0d8).unwrap_or_else(|e| panic!("tick {} rider {me}: {e:?}", w[0].tick));
                        g.real_states(false);
                        let mut r = g.get(&rider);
                        // (the runner's float rules are still set from its call)
                        r.vel = speed_limit(r.vel, speed_limit_at(a, b, boost, r5900::ps2float::fsub));
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
                    if !f.stumbled[me] {
                        f.state[me] = r.state;
                    }
                    done[me] = Some((r, 0.0));
                } else if let Some(x) = wipe[me].as_ref().filter(|_| pushes) {
                    let (pos, vel) = (riders[me].pos, riders[me].vel);
                    let r = if k == 0 { tick_ours(x, pos, vel, boost, a, b, &table) } else { tick_runner(&mut runners[k - 1].1, x, pos, vel, boost, a, b, addr[me]) };
                    riders[me].pos = r.p.pos;
                    riders[me].vel = r.vel;
                    riders[me].motion = 2;
                    f.hit[me] = HitRider { heading: r.yaw, pos: r.p.pos, vel: r.vel, normal: r.p.normal, motion: 2, crouch: f32_at(b, 0x1c8) };
                    wiped_in += (k == 0) as usize;
                } else if let Some(l) = lift[me].clone().filter(|_| pushes) {
                    let (pos, vel) = (riders[me].pos, riders[me].vel);
                    let r = if k == 0 { l.ours(pos, vel, a, b) } else { l.runner(&mut runners[k - 1].1, pos, vel, a, b) };
                    riders[me].pos = r.p.pos;
                    riders[me].vel = r.vel;
                    riders[me].motion = 1;
                    f.hit[me] = HitRider { heading: r.yaw, pos: r.p.pos, vel: r.vel, normal: r.p.normal, motion: 1, crouch: f32_at(b, 0x1c8) };
                    lifted += (k == 0) as usize;
                } else {
                    if u32_le(a, 0x424) == 1 && u32_le(b, 0x424) == 1 && std::env::var("TRICKY_AIRSTEP").is_ok() {
                        let (p, q) = (vec4(a, 0x140), vec4(b, 0x140));
                        f.body[me].translate([q[0] - p[0], q[1] - p[1], q[2] - p[2], q[3] - p[3]]);
                    }
                    let start = (riders[me].pos, riders[me].vel);
                    riders[me].vel = vec4(b, 0x150);
                    // a rider in the air the whole frame (F4b3v): its flight step, `Air_IntegrateRK4` 0x12b340 as
                    // `AirMotion_Update` 0x108378 calls it first (dt = +0x12c / 60), from where the riders before it
                    // left it; no speed limit in the air (`cBoarder::vf7` 0x117998). STANDIN: the rest of the air
                    // tick (the landing and wall rays, its own wall pushes) is taken as missed (TRICKY_NOAIR: the
                    // second sample less our hits, as for the others)
                    let ma = (u32_le(a, 0x424), u32_le(b, 0x424));
                    let clip = (u32_le(&w[0].extra[ka].1[me], 8), u32_le(&w[1].extra[ka].1[me], 8));
                    if ma == (5, 5) && clip.0 == clip.1 && clip.0.wrapping_sub(0x2dc) < 2 && air && !missed[me] {
                        // sliding the whole frame (F4b3w), then the speed limit (0x1179a4)
                        let (pos, vel) = if k == 0 {
                            slide_ours(start.0, start.1, a, b, walls_of(&course, a, &stats[me]).as_ref())
                        } else {
                            let kind = u32_le(b, 0x2e0);
                            let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
                            slide_runner(&mut runners[k - 1].1, start.0, start.1, a, b, clip.0, hit, walls_of(&course, a, &stats[me]).as_ref())
                        };
                        riders[me].vel = speed_limit(vel, speed_limit_at(a, b, boost, if k == 0 { |x, y| x - y } else { r5900::ps2float::fsub }));
                        riders[me].pos = pos;
                        slid[me] = true;
                    }
                    // tumbling the whole frame (F4b3x): `WipeoutMotion_Update` 0x10c2f0 off the slide clips leaves the
                    // rider the body's velocity (`Ragdoll_Step` 0x10dca8: rider +0x150 = body +0x70 +0x70), which the
                    // hits after it don't touch (`Boarder_RiderHit` 0x124920 adds to rider +0x150 only), and its
                    // position (`Boarder_Translate` 0x1267a0 moves the collision body, not +0x140); the clip it reads is the
                    // first sample's (a slide clip the tumble plays this tick is for the next). STANDIN: the
                    // body's step answered from the second sample's motion state (handler +0x4f0, `5ae0>4f0`), as
                    // its contacts with the course are not replayed (F4h5c)
                    // getting up the whole frame (F4b3x2), then the speed limit (0x1179a4: motion 6 is not 1)
                    if ma == (6, 6) && air && !missed[me] {
                        let kind = u32_le(b, 0x2e0);
                        let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
                        let stat60 = f32_at(&stats[me], 0x60);
                        let (pos, vel) = if k == 0 {
                            get_up_ours(start.0, start.1, a, b, stat60, hit)
                        } else {
                            get_up_runner(&mut runners[k - 1].1, start.0, start.1, a, b, stat60, hit)
                        };
                        riders[me].vel = speed_limit(vel, speed_limit_at(a, b, boost, if k == 0 { |x, y| x - y } else { r5900::ps2float::fsub }));
                        riders[me].pos = pos;
                        rose[me] = true;
                    }
                    // landing in the frame (F4b3x2), then the speed limit (0x1179a4: motion 2 by then); the runner's
                    // `AirMotion_Update` (F4b3x3; one that does not land is left where it started)
                    if ma == (1, 2) && air && !missed[me] && std::env::var("TRICKY_NOLAND").is_err() {
                        let board = if std::env::var("TRICKY_LANDBOARD").is_ok_and(|v| v == "a") { a } else { b };
                        let (pos, vel) = if k == 0 {
                            land_ours(start.0, start.1, a, b, board, &stats[me]).unwrap_or(start)
                        } else {
                            // the first sample's clips, and layer 0's sequence when recorded (a grab's flag 4,
                            // `AnimCtl_TestClipFlag`)
                            let c = ctl_of(&w[0], &w[0], me, ka, addr[me], 0, b).unwrap_or_else(|| {
                                let c = &w[0].extra[ka].1[me];
                                Ctl { clips: [u32_le(c, 8), u32_le(c, 0xc)], seq: None, handler: Vec::new(), pad: 0, finish: 0, rider: b.to_vec() }
                            });
                            land_runner(&mut runners[k - 1].1, start.0, start.1, a, b, board, &stats[me], &c).unwrap_or(start)
                        };
                        riders[me].vel = speed_limit(vel, speed_limit_at(a, b, boost, if k == 0 { |x, y| x - y } else { r5900::ps2float::fsub }));
                        if air_pos {
                            riders[me].pos = pos;
                        }
                        landed[me] = true;
                    }
                    if ma == (5, 5) && clip.0.wrapping_sub(0x2dc) >= 2 {
                        if let Some(m) = kt.map(|kt| &w[1].chased[kt].1[me]).filter(|m| m.len() >= 0x100) {
                            riders[me].vel = vec4(m, 0xe0);
                            riders[me].pos = vec4(b, 0x140);
                            tumbled[me] = true;
                        } else {
                            bare[me] = true;
                        }
                    }
                    if ma == (1, 1) && air && !missed[me] {
                        let dt = f32_at(a, 0x12c) * f32::from_bits(0x3c88_8889);
                        let (mut pos, mut vel) = start;
                        if k == 0 {
                            tricky_game::air::integrate_rk4(dt, &mut pos, &mut vel, false);
                        } else {
                            let g = &mut runners[k - 1].1;
                            wv(&mut g.r.mem, g.rd + 0x140, pos);
                            wv(&mut g.r.mem, g.rd + 0x150, vel);
                            g.r.cpu.set_gpr(4, g.rd as u64);
                            g.r.cpu.set_gpr(5, (g.rd + 0x140) as u64);
                            g.r.cpu.set_gpr(6, (g.rd + 0x150) as u64);
                            g.r.cpu.set_gpr(7, 0);
                            g.r.cpu.set_f(12, dt);
                            g.r.call(0x12b340).unwrap_or_else(|e| panic!("tick {} rider {me}: the flight step: {e:?}", w[0].tick));
                            (pos, vel) = (rv(&mut g.r.mem, g.rd + 0x140), rv(&mut g.r.mem, g.rd + 0x150));
                        }
                        // (a wall ray that hit turns the velocity far from the flight step's: missed, below)
                        riders[me].vel = vel;
                        if air_pos {
                            riders[me].pos = pos;
                        }
                        flown[me] = true;
                    }
                    // motion 4 the whole frame: `Boarder_UpdateMotion` 0x11c770 does nothing for it, then the speed
                    // limit (0x1179a4) as on the snow (F4b3v: rec14 4862, rider 3)
                    // (F4b3x: and a rider in motion 4 a hit in the collisions wipes out, rec14 4859 rider 3)
                    if (ma == (4, 4) || (ma == (4, 5) && std::env::var("TRICKY_NOWIPE").is_err())) && air && !missed[me] {
                        riders[me].vel = speed_limit(start.1, speed_limit_at(a, b, boost, if k == 0 { |x, y| x - y } else { r5900::ps2float::fsub }));
                        flown[me] = true;
                    }
                    riders[me].motion = u32_le(b, 0x424);
                    f.hit[me] = HitRider { vel: riders[me].vel, ..hit_rider(b) };
                    snap[me] = Some((riders[me].pos, riders[me].vel));
                }
                f.stat[me] = if f.stumbled[me] { f.stat0[me] } else { speed_stat(&stats[me], boost_at_collisions(a, b), f.add) };
                if pushes {
                    // the pair list as recorded (`L700`), from the frame's start (`Boarder_UpdateBroadphase` 0x126748
                    // in the last frame's `cPostAI::vf1`) or (TRICKY_LIST=b) its end; else every other rider in slot
                    // order
                    let how = std::env::var("TRICKY_LIST").unwrap_or_default();
                    let list = pair_list(&w[(how == "b") as usize], &addrs, me)
                        .filter(|_| how != "none")
                        .unwrap_or_else(|| (0..n).filter(|&o| o != me).collect());
                    // TRICKY_DBGT=<tick>: the gap between the bodies of each pair the rider looks at (ours), as the frame
                    // has them and as either sample has them (F4b3p)
                    if k == 0 && std::env::var("TRICKY_DBGT").is_ok_and(|v| v.parse() == Ok(tick)) {
                        let gap = |x: &CollBody, y: &CollBody| {
                            let mut m = f32::MAX;
                            for i in 0..x.count.max(0) as usize {
                                for j in 0..y.count.max(0) as usize {
                                    let d = sub4(y.spheres[j].0, x.spheres[i].0);
                                    m = m.min((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - x.spheres[i].1 - y.spheres[j].1);
                                }
                            }
                            m
                        };
                        for &o in &list {
                            eprintln!("DBGT tick {tick} {me}->{o}: gap now {} stale {} {}; first sample {}, second {}", gap(&f.body[me], &f.body[o]), f.stale[me], f.stale[o], gap(&body_of(&w[0].riders[me]), &body_of(&w[0].riders[o])), gap(&body_of(&w[1].riders[me]), &body_of(&w[1].riders[o])));
                        }
                    }
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
            if k == 0 {
                let mut again = false;
                for i in (0..n).filter(|&i| flown[i] || slid[i] || rose[i] || landed[i]) {
                    if sub4(riders[i].vel, vec4(&w[1].riders[i], 0x150)).iter().any(|x| x.abs() >= 1.0) {
                        missed[i] = true;
                        missed_all[match u32_le(&w[1].riders[i], 0x424) { 1 => 0, 4 => 1, _ => 2 }] += 1;
                        again = true;
                        if std::env::var("TRICKY_DBG").is_ok() {
                            eprintln!("DBG missed tick {tick} rider {i}: motion {}", u32_le(&w[1].riders[i], 0x424));
                        }
                    }
                }
                if again {
                    continue;
                }
            }
            step += 1;
            for i in 0..n {
                if let Some((mut r, target)) = done[i] {
                    r.vel = riders[i].vel;
                    r.p.pos = riders[i].pos;
                    // `Boarder_Stumble` 0x124dd0's `Boarder_SetState`(rider, 2) (F4b3r)
                    r.state = f.state[i];
                    got[i].push(fields(&r, target));
                }
            }
            if let (0, true, Some(kp)) = (k, pushes, kp) {
                for i in 0..n {
                    let (a, b, ra, rb) = (&w[0].riders[i], &w[1].riders[i], &w[0].extra[kp].1[i], &w[1].extra[kp].1[i]);
                    let Some((p0, v0)) = snap[i] else { continue };
                    if u32_le(a, 0x424) != 5 || u32_le(b, 0x424) != 5 || f32_at(rb, 0x4c) != 0.0 || ra == rb || u32_le(rb, 8) != u32_le(rb, 0x44) {
                        continue;
                    }
                    let right = |pr: &Predict| {
                        pr.t_top.to_bits() == u32_le(rb, 0x48)
                            && (u32_le(rb, 0x40) == 0xffff_ffff || (pr.word == u32_le(rb, 0x40) && pr.point == vec4(rb, 0x10)))
                    };
                    let (pos, vel, normal) = (vec4(b, 0x140), vec4(b, 0x150), vec4(b, 0x2a0));
                    let (dp, dv) = (sub4(riders[i].pos, p0), sub4(riders[i].vel, v0));
                    let end = right(&predict_like(pos, vel, normal, rb));
                    let before = right(&predict_like(sub4(pos, dp), sub4(vel, dv), normal, rb));
                    wiped[0] += 1;
                    wiped[1] += end as usize;
                    wiped[2] += before as usize;
                    if std::env::var("TRICKY_DBG").is_ok() {
                        eprintln!("DBG wiped prediction tick {tick} rider {i}: from the end {end}, less the pushes {dp:?} and hits {dv:?}: {before}");
                    }
                }
            }
            if k == 0 {
                bodiless += bare.iter().filter(|x| **x).count();
                for i in (0..n).filter(|&i| tumbled[i]) {
                    tumbles[0] += 1;
                    tumbles[1] += (riders[i].vel == vec4(&w[1].riders[i], 0x150)) as usize;
                }
            }
            for i in (0..n).filter(|&i| slid[i]) {
                let b = &w[1].riders[i];
                slides[k][0] += 1;
                slides[k][1] += (riders[i].vel == vec4(b, 0x150)) as usize;
                slides[k][2] += (riders[i].pos == vec4(b, 0x140)) as usize;
                if k == runners.len() && (riders[i].vel != vec4(b, 0x150) || riders[i].pos != vec4(b, 0x140)) && std::env::var("TRICKY_DBG").is_ok() {
                    let (o, g) = (riders[i].vel, vec4(b, 0x150));
                    eprintln!("DBG slide tick {tick} rider {i}: hits {} pushes {} velocity ulps {:?}, position off {:?}", f.hits[i], f.pushes[i], (0..3).map(|c| o[c].to_bits() as i64 - g[c].to_bits() as i64).collect::<Vec<_>>(), sub4(riders[i].pos, vec4(b, 0x140)));
                }
            }
            for i in (0..n).filter(|&i| landed[i]) {
                let b = &w[1].riders[i];
                landings[k][0] += 1;
                landings[k][1] += (riders[i].vel == vec4(b, 0x150)) as usize;
                landings[k][2] += (riders[i].pos == vec4(b, 0x140)) as usize;
                if riders[i].vel != vec4(b, 0x150) && std::env::var("TRICKY_DBG").is_ok() {
                    let (o, g) = (riders[i].vel, vec4(b, 0x150));
                    eprintln!("DBG landing way {k} tick {tick} rider {i}: hits {} ulps {:?}, position off {:?}", f.hits[i], (0..3).map(|c| o[c].to_bits() as i64 - g[c].to_bits() as i64).collect::<Vec<_>>(), sub4(riders[i].pos, vec4(b, 0x140)));
                }
            }
            for i in (0..n).filter(|&i| rose[i]) {
                let b = &w[1].riders[i];
                getups[k][0] += 1;
                getups[k][1] += (riders[i].vel == vec4(b, 0x150)) as usize;
                getups[k][2] += (riders[i].pos == vec4(b, 0x140)) as usize;
                if k == runners.len() && (riders[i].vel != vec4(b, 0x150) || riders[i].pos != vec4(b, 0x140)) && std::env::var("TRICKY_DBG").is_ok() {
                    let (o, g) = (riders[i].vel, vec4(b, 0x150));
                    eprintln!("DBG get-up tick {tick} rider {i}: hits {} ulps {:?}, position off {:?}", f.hits[i], (0..3).map(|c| o[c].to_bits() as i64 - g[c].to_bits() as i64).collect::<Vec<_>>(), sub4(riders[i].pos, vec4(b, 0x140)));
                }
            }
            for i in (0..n).filter(|&i| flown[i]) {
                let b = &w[1].riders[i];
                flights[k][0] += 1;
                flights[k][1] += (riders[i].vel == vec4(b, 0x150)) as usize;
                flights[k][2] += (riders[i].pos == vec4(b, 0x140)) as usize;
                if k == runners.len() && riders[i].vel != vec4(b, 0x150) && std::env::var("TRICKY_DBG").is_ok() {
                    let (o, g) = (riders[i].vel, vec4(b, 0x150));
                    eprintln!("DBG flight tick {tick} rider {i}: hits {} ulps {:?}", f.hits[i], (0..3).map(|c| o[c].to_bits() as i64 - g[c].to_bits() as i64).collect::<Vec<_>>());
                    eprintln!("    a {:?} ts {} ours {o:?} game {g:?}", vec4(&w[0].riders[i], 0x150), f32_at(&w[0].riders[i], 0x12c));
                }
            }
            if k == runners.len() {
                frame_pushes = f.pushes.clone();
                frame_hits = f.hits.clone();
                for i in 0..n {
                    for j in (0..n).filter(|&j| j != i) {
                        let (was, game) = (u32_le(&w[0].riders[i], 0x48 + 0x20 * j) as i32, u32_le(&w[1].riders[i], 0x48 + 0x20 * j) as i32);
                        let (swas, sgame) = (u32_le(&w[0].riders[i], 0x4c + 0x20 * j) as i32, u32_le(&w[1].riders[i], 0x4c + 0x20 * j) as i32);
                        if riders[i].rec[j].shove != swas || sgame != swas {
                            shoves += 1;
                            shoves_right += (riders[i].rec[j].shove == sgame) as usize;
                            if riders[i].rec[j].shove != sgame && std::env::var("TRICKY_DBG").is_ok() {
                                // the shove's reach from the first sample's frame (`frame_of`)
                                eprintln!("DBG shove tick {tick} {i}->{j}: was {swas} game {sgame} ours {} clip types {} {}", riders[i].rec[j].shove, f.clip1[i], f.clip1[j]);
                                let (m, o) = (col_rider(&w[0].riders[i], n, &stats[i], pose_of(&w[0], i, kpose, &table)), col_rider(&w[0].riders[j], n, &stats[j], pose_of(&w[0], j, kpose, &table)));
                                let d = [o.at[0] - m.at[0], o.at[1] - m.at[1], o.at[2] - m.at[2], o.at[3] - m.at[3]];
                                let dt = |x: V4, y: V4| x[0] * y[0] + x[1] * y[1] + x[2] * y[2] + x[3] * y[3];
                                let h = dt(d, m.up);
                                let lat = [d[0] - m.up[0] * h, d[1] - m.up[1] * h, d[2] - m.up[2] * h];
                                eprintln!("    h {h} lat {} bearing {} heading {} shoving {} motions {} {} stat {} {}", (lat[0] * lat[0] + lat[1] * lat[1] + lat[2] * lat[2]).sqrt(), -(dt(d, m.fwd)).atan2(dt(d, m.side)), m.heading, f.shoving[i], m.motion, o.motion, f.stat[i], f.stat[j]);
                            }
                        }
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
                if (k == runners.len() || k == 0 && std::env::var("TRICKY_DBG2").is_ok_and(|v| v == "ours")) && !all && std::env::var("TRICKY_DBG2").is_ok() {
                    let bad: Vec<&str> = written.iter().filter(|&&f| g[f] != p.want[f]).map(|&f| names[f]).collect();
                    let a = &w[0].riders[i];
                    eprintln!("DBG2 tick {} rider {i}: state {:x}->{:x} ours {:x} clip {:x}->{:x} bad {bad:?}", w[1].tick, u32_le(a, 0x428), u32_le(b, 0x428), g[22][0], u32_le(&w[0].extra[ka].1[i], 8), u32_le(&w[1].extra[ka].1[i], 8));
                    let v = |x: &[u32]| (0..3).map(|c| f32::from_bits(x[c])).collect::<Vec<_>>();
                    eprintln!("    pos ours {:?} game {:?}; vel ours {:?} game {:?}", v(&g[0]), v(&p.want[0]), v(&g[1]), v(&p.want[1]));
                    let d = |x: &[u8], y: &[u8]| { let (a, c) = (vec4(x, 0x140), vec4(y, 0x140)); ((a[0] - c[0]).powi(2) + (a[1] - c[1]).powi(2) + (a[2] - c[2]).powi(2)).sqrt() };
                    eprintln!("    others' distance at the samples: {:?}; vel before {:?}", (0..n).filter(|&j| j != i).map(|j| (j, d(a, &w[0].riders[j]) as i32, d(b, &w[1].riders[j]) as i32)).collect::<Vec<_>>(), vec4(a, 0x150));
                }
                if k == runners.len() && was_pushed {
                    let hit = (0..n).any(|j| u32_le(b, 0x48 + 0x20 * j) == w[1].tick);
                    pushed_pos[hit as usize][(g[0] == p.want[0]) as usize] += 1;
                    hit_vel += (hit && g[1] == p.want[1]) as usize;
                    if hit && g[1] != p.want[1] && std::env::var("TRICKY_DBG").is_ok() {
                        let ulps: Vec<i64> = (0..3).map(|c| g[1][c] as i32 as i64 - p.want[1][c] as i32 as i64).collect();
                        eprintln!("DBG hitvel tick {} rider {i}: ulps {ulps:?} pos ok {} pushes {} hits(ours) {:?} stale bodies {:?}", w[1].tick, g[0] == p.want[0], frame_pushes[i], frame_hits[i], (0..n).filter(|&x| frame_stale[x]).collect::<Vec<_>>());
                        if std::env::var("TRICKY_DBG").is_ok_and(|v| v == "2") {
                            let v = |x: &[u32]| (0..3).map(|c| f32::from_bits(x[c])).collect::<Vec<_>>();
                            eprintln!("    ours {:?} game {:?} first {:?}", v(&g[1]), v(&p.want[1]), vec4(&w[0].riders[i], 0x150));
                        }
                    }
                }
            }
        }
    }
    eprintln!("{path}: ground ticks (motion 2 at both samples): {pairs} compared ({capped} at the speed limit); left out {other} whose motion object's +0xc is not the rider, {skew} not one update apart, {torn} read inside a tick (it or the sample before)");
    eprintln!("  controls replayed by state: {ctl_count:x?}, every field right under fadd=g1: {ctl_right}; ours from the runner's rider the same as the runner's (per runner way): {ctl_same:?}, quaternion the game's: ours {ctl_ours_game:?}, the runner's {ctl_runner_game:?}; left out: {ctl_turn} fakie cruise on its turn clip, {ctl_none} without the windows; the player's pad word from the recorded pad on {player_ctl} (F4b3d, TRICKY_PAD=b); the pad word the hook kept on {hook_ctl} (F4b3k)");
    if kc.is_some() {
        eprintln!("  frames left out because a sample was read before every rider had ticked: {mid}");
    }
    if pushes {
        eprintln!("  pushed apart by another rider in the frame (order {order:?}): {pushed}; every field right: {pushed_whole:?}");
        eprintln!("  of those, under fadd=g1: not hit {} (position right {}), hit too {} (position right {}, velocity right {hit_vel})", pushed_pos[0][0] + pushed_pos[0][1], pushed_pos[0][1], pushed_pos[1][0] + pushed_pos[1][1], pushed_pos[1][1]);
        eprintln!("  bump records (+0x40 + 0x20·j +8) changed this frame by the game or the replay: {bumps}, the replay's equal to the game's: {bumps_right}");
        eprintln!("  shove records (+0x40 + 0x20·j +0xc) changed this frame by the game or the replay: {shoves}, the replay's equal to the game's: {shoves_right}; the layer-1 clip's flags recorded on {} rider ticks: the game shoved on {}, between the flags on {} of them; between them on a shove clip with no shove {}", flag_count[0], flag_count[1], flag_count[2], flag_count[3]);
        eprintln!("  the wipe-out motion's flight predictions (motion 5 at both samples, window 57b0): {}, right from the end of the tick {}, from it less the pushes and hits after the motion {}", wiped[0], wiped[1], wiped[2]);
        eprintln!("  bodies read before `cPostAI::vf1` re-posed them, placed by a guess (TRICKY_STALE=keep: as read): {stale_bodies}");
        eprintln!("  riders in the air the whole frame, their flight step replayed (TRICKY_NOAIR: as recorded) [frames, velocity right, position right] by way: {flights:?}");
        eprintln!("  wiped-out riders tumbling the whole frame, the velocity their motion left them taken from its recorded body (5ae0>4f0; TRICKY_TUMBLE=off: as the others) [frames, velocity right]: {tumbles:?}; with no body recorded (the second sample's velocity): {bodiless}");
        eprintln!("  wiped-out riders sliding the whole frame, their slide replayed [frames, velocity right, position right] by way: {slides:?}");
        eprintln!("  riders landing in the frame, their landing's velocity replayed [frames, velocity right, position right] by way: {landings:?}");
        eprintln!("  riders getting up the whole frame, their get-up replayed [frames, velocity right, position right] by way: {getups:?}");
        eprintln!("  riders whose own motion was replayed but ended the frame 1 cm/s or more from the game's, taken as the others (in the air, motion 4, sliding): {missed_all:?}");
        eprintln!("  riders leaving the snow in a replayed frame, their take-off replayed for their pushes and hits (TRICKY_NOLIFT: as recorded): {lifted}");
        eprintln!("  riders wiped out by a hit in a replayed frame, their ground tick replayed (TRICKY_NOWIPE: as the others): {wiped_in}");
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
    speed_limit_at(a, b, f32_at(a, 0x130), sub)
}

/// [`speed_limit_of`] with the boost level as the rider has it after its motion (F4b3r: 0 when a hit earlier in the
/// frame stumbled it).
fn speed_limit_at(a: &[u8], b: &[u8], boost: f32, sub: fn(f32, f32) -> f32) -> f32 {
    let target = if f32_at(b, 0x134) > 0.0 {
        f32::from_bits(0x4551338e)
    } else if boost > 0.0 {
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

/// A rider's eased input at `at` (the brake +0x1fc, the crouch +0x208, the steer +0x214) as `cBoarder::vf7` 0x117198
/// eases it before the motion (the brake at 0x117288, the crouch at 0x1172d8, the steer at 0x11732c): toward its
/// target (`at` + 8) by its rate (`at` + 4) / 60, the target once within that step, under `add` (PCSX2's adder,
/// [`pcsx2_add`], gives the recorded values).
fn eased(a: &[u8], at: usize, add: Add) -> f32 {
    let (now, step, target) = (f32_at(a, at), f32_at(a, at + 4) * f32::from_bits(0x3c88_8889), f32_at(a, at + 8));
    if add(target, step) < now {
        add(now, -step)
    } else if now < add(target, -step) {
        add(now, step)
    } else {
        target
    }
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

/// A wiped-out rider sliding the whole frame (motion 5 on clip 0x2dc / 0x2dd at both samples, F4b3w): the slide's
/// step from (pos, vel), on the first sample's ground (+0x2a0), surface (+0x290) and height (+0x1bc) and the second's
/// ride height (+0x294, moved by `cBoarder::vf7` before the motion). Ours: `wipeout_motion::slide_step` and
/// `hold_step`; the runner's: `Boarder_UpdateMotion` 0x11c770 on the handler, the world under the probe the second
/// sample's ground. Its body is not recorded: the probe's hip is 0 (its answer moves neither position nor velocity).
/// With the course (F4b3p), then the slide's `Boarder_WallCollide` 0x126250 (rider+0x494 the word at 0x31d310, 0xfc3;
/// mode 5: no impact check), the body the first sample's, the ground the probe's (the second sample's +0x2a0), as
/// `walls::wall_collide` for ours. `Boarder_InstanceCollisions` 0x125088 before it is still answered nothing.
fn slide_ours(pos: V4, vel: V4, a: &[u8], b: &[u8], walls: Option<&Walls>) -> (V4, V4) {
    let (mut pos, mut vel, mut h) = (pos, vel, f32_at(a, 0x1bc));
    let n = vec4(a, 0x2a0);
    tricky_game::wipeout_motion::slide_step(&mut pos, &mut vel, n, SURFACE_TABLE[u32_le(a, 0x290) as usize][0]);
    tricky_game::wipeout_motion::hold_step(&mut pos, &mut vel, &mut h, n, f32_at(b, 0x294));
    if let Some(w) = walls {
        let mut body = w.body;
        body.x24 = 0xfc3;
        let mut wr = tricky_game::walls::WallRider { pos, vel, ground: vec4(b, 0x2a0), knocks: 0.0, mode: 5, state: u32_le(a, 0x428), up: w.up };
        tricky_game::walls::wall_collide(&mut wr, &mut SlideWalls { course: &w.course, body });
        (pos, vel) = (wr.pos, wr.vel);
    }
    (pos, vel)
}

/// What the slide's `Boarder_WallCollide` 0x126250 calls (mode 5: no turn, no impact check past the knocks).
struct SlideWalls<'a> {
    course: &'a tricky_game::course_ground::CourseGround,
    body: CollBody,
}
impl tricky_game::walls::WallEnv for SlideWalls<'_> {
    fn cast_walls(&mut self, r: &tricky_game::walls::WallRider) -> (f32, tricky_game::walls::CastRecord) {
        wall_cast(self.course, &self.body, r.ground)
    }
    fn query_objects(&mut self, _: &tricky_game::walls::WallRider) -> (f32, tricky_game::walls::CastRecord, tricky_game::walls::Touched) {
        unreachable!("not called by Boarder_WallCollide")
    }
    fn accumulate_impact(&mut self, _: &mut tricky_game::walls::WallRider, _: V4) {}
    fn translate(&mut self, _: &mut tricky_game::walls::WallRider, by: V4) {
        self.body.translate(by);
    }
    fn turn_from_wall(&mut self, _: &mut tricky_game::walls::WallRider, _: V4) {
        unreachable!("mode 2 only")
    }
    fn wipeout(&mut self, _: &mut tricky_game::walls::WallRider, _: &tricky_game::walls::Contact) {
        unreachable!("not in mode 5")
    }
    fn score_crash(&mut self, _: &mut tricky_game::walls::WallRider) -> f32 {
        unreachable!("not in mode 5")
    }
    fn add_meter(&mut self, _: &mut tricky_game::walls::WallRider, _: f32) {}
    fn stumble(&mut self, _: &mut tricky_game::walls::WallRider, _: V4, _: u32) {
        unreachable!("not in mode 5")
    }
    fn speed_stat(&mut self, _: &tricky_game::walls::WallRider) -> f32 {
        unreachable!("not called by Boarder_WallCollide")
    }
    fn knock(&mut self, _: &mut tricky_game::walls::WallRider, _: f32) {}
    fn contact(&mut self, _: u32, _: &mut tricky_game::walls::WallRider, _: &tricky_game::walls::Contact, _: &tricky_game::walls::CastRecord) {}
    fn sfx_object(&mut self, _: &tricky_game::walls::WallRider, _: u32) {}
    fn sfx_wall(&mut self, _: &tricky_game::walls::WallRider) {}
}

fn slide_runner(g: &mut W, pos: V4, vel: V4, a: &[u8], b: &[u8], clip: u32, hit: Option<Hit>, walls: Option<&Walls>) -> (V4, V4) {
    let (rd, h) = (g.rd, g.handler);
    let m = &mut g.r.mem;
    for (o, v) in [(0x140, pos), (0x150, vel), (0x170, vec4(a, 0x170)), (0x180, vec4(a, 0x180)), (0x190, vec4(a, 0x190)), (0x1a0, vec4(a, 0x1a0)), (0x2a0, vec4(a, 0x2a0))] {
        wv(m, rd + o, v);
    }
    for (o, v) in [(0x12c, f32_at(a, 0x12c)), (0x1bc, f32_at(a, 0x1bc)), (0x294, f32_at(b, 0x294))] {
        m.write_f32(rd + o, v);
    }
    m.write_u32(rd + 0x290, u32_le(a, 0x290));
    m.write_u32(rd + 0x418, u32_le(a, 0x418));
    m.write_u32(rd + 0x424, 5);
    m.write_u32(rd + 0x46c0 + 8, clip);
    m.write_u32(h + 0x550, rd);
    // the motion's seconds (+0x4fc) and seconds on the ground (+0x500), not recorded
    for o in [0x4f8, 0x4fc, 0x500] {
        m.write_u32(h + o, 0);
    }
    *g.hit.borrow_mut() = hit;
    g.wall.set(false);
    g.put_walls(walls);
    g.r.cpu.set_gpr(4, h as u64);
    g.r.call(0x11c770).unwrap_or_else(|e| panic!("the slide: {e:?}"));
    (rv(&mut g.r.mem, rd + 0x140), rv(&mut g.r.mem, rd + 0x150))
}

/// A rider getting up the whole frame (motion 6 at both samples, F4b3x2): `GetUpMotion_Update` 0x1102c8's step from
/// (pos, vel): the slide on the first sample's ground (+0x2a0) and surface (+0x290), `Boarder_ProbeGround` 0x128ae8
/// on the second sample's ground (`hit`), the hold to it with the second's ride height (+0x294). Ours:
/// `wipeout_motion::slide_step`, `probe::probe`, `hold_step` (the order of `wipeout_motion::get_up_update`).
fn get_up_ours(pos: V4, vel: V4, a: &[u8], b: &[u8], stat60: f32, hit: Option<Hit>) -> (V4, V4) {
    let (mut pos, mut vel) = (pos, vel);
    tricky_game::wipeout_motion::slide_step(&mut pos, &mut vel, vec4(a, 0x2a0), SURFACE_TABLE[u32_le(a, 0x290) as usize][0]);
    let mut p = probe::Rider {
        pos,
        fwd: vec4(a, 0x180),
        normal: vec4(a, 0x2a0),
        ground: vec4(a, 0x2d0),
        plane_fwd: vec4(a, 0x320),
        side: vec4(a, 0x330),
        fall: vec4(a, 0x2b0),
        x244: f32_at(a, 0x244),
        stat60,
        height: f32_at(a, 0x1bc),
        surface: u32_le(a, 0x290),
        surface_ticks: u32_le(a, 0x29c),
        kind: u32_le(a, 0x2e0),
    };
    probe::probe(&mut p, hit);
    tricky_game::wipeout_motion::hold_step(&mut pos, &mut vel, &mut p.height, p.normal, f32_at(b, 0x294));
    (pos, vel)
}

/// Our landing (F4b3x3): `tricky_game::air_motion::update_by` (`AirMotion_Update` 0x108378) from (pos, vel) with
/// the first sample's time scale (+0x12c), orientation (+0x170, +0x1d0, +0x1d4), surface, side and switch, the
/// board (+0x180, +0x1a0) of `board`, both rays answered with the second sample's ground (normal +0x2a0, surface
/// +0x290), and the COP1 adds by PCSX2's adder (`fadd=g1`, F4g: the slow-downs' and `Landing_CheckAngles`'s,
/// `check_angles_by`). `None`: it did not land (no `Landing_ChooseState`), as `land_runner`. STANDIN, as
/// `land_runner`: the down ray's hit 99.9 cm along the normal from its start and the probe's height 0.1 cm (a
/// landing not clamped to the ground: the clamp's pivot needs the body's bone, not recorded), the bone and the
/// board's frame 0; the clip type taken as no grab (a grab not at its safe point crashes, motion 5, not 2).
fn land_ours(pos: V4, vel: V4, a: &[u8], b: &[u8], board: &[u8], stats: &[u8]) -> Option<(V4, V4)> {
    use tricky_game::air_motion::{update_by, AirEnv, AirRider, Ray};
    use tricky_game::landing::{check_angles_by, LandingState};
    fn g1(x: f32, y: f32) -> f32 {
        r5900::ps2float::add_by(x, y, r5900::ps2float::Round::Guard)
    }
    struct LandEnv {
        normal: V4,
        surface: u32,
        board: V4,
        up: V4,
        stability: u8,
        landed: bool,
    }
    impl AirEnv for LandEnv {
        fn predict_tick(&mut self, _: &mut AirRider) {}
        fn down_ray(&mut self, _: &mut AirRider, from: V4, _: V4) -> Ray {
            let n = self.normal;
            Ray { frac: 0.4, point: [from[0] + n[0] * 99.9, from[1] + n[1] * 99.9, from[2] + n[2] * 99.9, from[3]], normal: n, word: self.surface }
        }
        fn probe_ground(&mut self, r: &mut AirRider) -> bool {
            r.normal = self.normal;
            r.surface = self.surface;
            r.height = 0.1;
            true
        }
        fn predict_landed(&mut self, _: &mut AirRider) {}
        fn set_fall_scale(&mut self, _: &mut AirRider, _: f32) {}
        fn virtual_6c(&mut self, _: &mut AirRider, _: f32) {}
        fn clip_type_layer1(&mut self, _: &mut AirRider) -> u32 {
            0
        }
        fn blend_out_all(&mut self, _: &mut AirRider) {}
        fn check_angles(&mut self, r: &mut AirRider) -> Option<(f32, f32)> {
            let s = LandingState { clip_type: 0, clip_flag4: false, surface: r.surface, board: self.board, vel: r.vel, up: self.up, stability: self.stability };
            check_angles_by(&s, r.normal, g1).map(|l| (l.pitch, l.roll))
        }
        fn crash_virtual(&mut self, _: &mut AirRider, _: V4, _: V4, _: V4) {}
        fn wipeout(&mut self, _: &mut AirRider, _: &CrashRecord) {}
        fn score_crash(&mut self, _: &mut AirRider) -> f32 {
            0.0
        }
        fn add_meter(&mut self, _: &mut AirRider, _: f32) {}
        fn choose_state(&mut self, _: &mut AirRider, _: f32, _: f32, _: f32) {
            self.landed = true;
        }
        fn land_trick(&mut self, _: &mut AirRider, _: bool, _: bool) -> f32 {
            0.0
        }
        fn virtual_84(&mut self, _: &mut AirRider, _: f32) {}
        fn start_celebrate(&mut self, _: &mut AirRider, _: f32) {}
        fn is_tricky(&mut self, _: &mut AirRider) -> bool {
            false
        }
        fn speech_air_time(&mut self, _: &mut AirRider) {}
        fn spawn_landing_puffs(&mut self, _: &mut AirRider, _: f32) {}
        fn sfx_update_air(&mut self, _: &mut AirRider, _: f32) {}
        fn virtual_9c(&mut self, _: &mut AirRider) {}
        fn rail_try_capture(&mut self, _: &mut AirRider) {}
        fn instance_collisions(&mut self, _: &mut AirRider) {}
        fn wall_ray(&mut self, _: &mut AirRider) -> Ray {
            Ray { frac: -1.0, ..Default::default() }
        }
        fn translate(&mut self, _: &mut AirRider, _: V4) {}
        fn wall_done(&mut self, _: &mut AirRider) {}
    }
    let mut r = AirRider {
        timescale: f32_at(a, 0x12c),
        pos,
        vel,
        orient: Orient { q: vec4(a, 0x170), fwd: vec4(board, 0x180), side: vec4(board, 0x190), up: vec4(board, 0x1a0), yaw: f32_at(a, 0x1b0) },
        spin: f32_at(a, 0x1d0),
        flip: f32_at(a, 0x1d4),
        slope_kind: -1,
        surface: u32_le(a, 0x290),
        surface_ticks: u32_le(a, 0x29c) as i32,
        normal: vec4(a, 0x2a0),
        height: f32_at(a, 0x1bc),
        switch: u32_le(a, 0x1b4),
        stance: u32_le(stats, 0x40),
        mask: u32_le(a, 0x494),
        ..Default::default()
    };
    let mut env = LandEnv { normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290), board: vec4(board, 0x180), up: vec4(board, 0x1a0), stability: stats[0x16], landed: false };
    update_by(&mut r, &SURFACE_TABLE, &mut env, g1);
    env.landed.then_some((r.pos, r.vel))
}

/// The runner's landing (F4b3x3): `AirMotion_Update` 0x108378 on the air motion object from (pos, vel), with the
/// first sample's time scale (+0x12c), orientation (+0x170, +0x1d0, +0x1d4), side (+0x330), surface and clips
/// (`c`, laid out as `lay` does), the board (+0x180..+0x1a0) of `board`, and both rays (down, then `Boarder_ProbeGround`
/// 0x128ae8) answered with the second sample's ground: normal (+0x2a0), kind (+0x2e0), surface (+0x290). `None`: it
/// did not land (no `Landing_ChooseState`). STANDIN: each hit point is put 99.9 cm along the normal from its line's
/// start (the probe's line starts 100 cm below the board), so the probe's height is about 0.1 cm, a landing not
/// clamped to the ground (the clamp's pivot needs the body's bone, not recorded): the position is the flight
/// step's, as ours; the bone is 0, the predicted landing's surface (+0x57f0) none.
fn land_runner(g: &mut W, pos: V4, vel: V4, a: &[u8], b: &[u8], board: &[u8], stats: &[u8], c: &Ctl) -> Option<(V4, V4)> {
    let rd = g.rd;
    let m = &mut g.r.mem;
    m.write_f32(g.stats + 0x60, f32_at(stats, 0x60));
    m.write_u8(g.stats + 0x16, stats[0x16]);
    m.write_u32(g.stats + 0x74, 0);
    wv(m, rd + 0x140, pos);
    wv(m, rd + 0x150, vel);
    for o in [0x170, 0x2a0, 0x330] {
        wv(m, rd + o, vec4(a, o as usize));
    }
    for o in [0x180, 0x190, 0x1a0] {
        wv(m, rd + o, vec4(board, o as usize));
    }
    for o in [0x12c, 0x1bc, 0x1d0, 0x1d4, 0x244] {
        m.write_f32(rd + o, f32_at(a, o as usize));
    }
    for o in [0x1b4, 0x290, 0x29c] {
        m.write_u32(rd + o, u32_le(a, o as usize));
    }
    m.write_u32(rd + 0x44d8, 0);
    for o in (0x8c0..0x8d0).chain(0x4aa0..0x4ae0).step_by(4) {
        m.write_u32(rd + o, 0);
    }
    m.write_u32(rd + 0x57f0, u32::MAX);
    for (at, v) in lay(c, rd + 0x46c0, g.seqs[0]) {
        g.r.mem.write_u32(at, v);
    }
    *g.hit.borrow_mut() = Some(Hit { kind: u32_le(b, 0x2e0), point: [0.0; 4], normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
    g.wall.set(false);
    g.landing.set(true);
    let from = g.log.borrow().len();
    g.r.cpu.set_gpr(4, g.air as u64);
    let ran = g.r.call(0x108378);
    g.landing.set(false);
    let landed = g.log.borrow()[from..].iter().any(|e| matches!(e, Ev::Call(0x109308)));
    if std::env::var("TRICKY_DBG").is_ok_and(|v| v == "land") {
        eprintln!("LAND {ran:?} {landed} clips {:x?} seq {:?} {:?}", c.clips, c.seq.as_ref().map(|s| s.1), &g.log.borrow()[from..]);
    }
    (ran.is_ok() && landed).then(|| (rv(&mut g.r.mem, rd + 0x140), rv(&mut g.r.mem, rd + 0x150)))
}

/// The runner's get-up: `Boarder_UpdateMotion` 0x11c770 on the handler, motion 6 (`GetUpMotion_Update` on handler
/// +0x5f0, whose +0xc is the rider), the world under the probe `hit`
fn get_up_runner(g: &mut W, pos: V4, vel: V4, a: &[u8], b: &[u8], stat60: f32, hit: Option<Hit>) -> (V4, V4) {
    let (rd, h) = (g.rd, g.handler);
    let m = &mut g.r.mem;
    m.write_f32(g.stats + 0x60, stat60);
    for o in [0x170, 0x180, 0x190, 0x1a0, 0x2a0, 0x2b0, 0x2c0, 0x2d0, 0x320, 0x330, 0x360] {
        wv(m, rd + o, vec4(a, o as usize));
    }
    wv(m, rd + 0x140, pos);
    wv(m, rd + 0x150, vel);
    for o in [0x12c, 0x1bc, 0x244] {
        m.write_f32(rd + o, f32_at(a, o as usize));
    }
    m.write_f32(rd + 0x294, f32_at(b, 0x294));
    for o in [0x290, 0x29c, 0x2e0, 0x418] {
        m.write_u32(rd + o, u32_le(a, o as usize));
    }
    m.write_u32(rd + 0x424, 6);
    m.write_u32(h + 0x5f0 + 0xc, rd);
    *g.hit.borrow_mut() = hit;
    g.wall.set(false);
    g.r.cpu.set_gpr(4, h as u64);
    g.r.call(0x11c770).unwrap_or_else(|e| panic!("the get-up: {e:?}"));
    (rv(&mut g.r.mem, rd + 0x140), rv(&mut g.r.mem, rd + 0x150))
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
    let twins = Twins::of_recording(path, &ticks);
    let course = course(path);
    let files = course_files(path);
    let win = |o: i64| ticks[0].extra.iter().position(|(x, _)| *x == o);
    let (Some(km), Some(ka), Some(kp)) = (win(-0x400), win(0x46c0), win(0x57b0)) else {
        eprintln!("{path}: no -400, 46c0 and 57b0 windows: skipping");
        return;
    };
    let km6 = win(-0x3d0);
    let kpose = pose_windows(&ticks);
    let ks = win(-0x170);
    let clip_type = Rc::new(Cell::new(0u32));
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners = Vec::new();
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut w) = world() else { return };
        w.r.float_rules = Rules::parse(rules).unwrap();
        // the walls' cast as the original's on the course built in the runner (F4h5m; TRICKY_WALLCAST=ours: ours)
        if let (Some(f), Some(c), true) = (&files, &course, std::env::var("TRICKY_WALLCAST").map_or(true, |v| v != "ours")) {
            w.load_course(f, c);
        }
        let off = Rc::new(RefCell::new(Off::default()));
        let (o, rd) = (off.clone(), w.rd);
        w.r.stub(0x11f610, move |cpu, mem| {
            let m = cpu.gpr(5) as u32;
            o.borrow_mut().motion.push((m, [rv(mem, rd + 0x140), rv(mem, rd + 0x150), rv(mem, rd + 0x2a0)]));
            mem.write_u32(rd + 0x424, m);
        });
        let o = off.clone();
        w.r.stub(0x126e30, move |_, _| o.borrow_mut().spin_rates += 1);
        // `Boarder_SetState` 0x11f608 runs (F4h5h): the revert in the motion (`ChangeFakieControl_Enter` 0x103eb8 turns
        // the board half round) and the take-off's air state (`NaturalAirControl_Enter` 0x1000b8); the exits are not
        // run (they change only what the motion has used by then: brake, crouch, boost), each logs the state entered
        w.real_states(true);
        for f in [0x11f608, 0x11c7d8, 0x11c8e8, 0x103eb8, 0x1000b8] {
            w.r.allow(f);
        }
        let o = off.clone();
        w.r.stub(0x11c840, move |cpu, _| o.borrow_mut().state.push(cpu.gpr(5) as u32));
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
    let keyframed = match (&course, &files) {
        (Some(c), Some(f)) => Keyframed::of(c, f, &ticks, n),
        _ => None,
    };
    let names = ["pos", "vel", "fwd", "board_up", "plane_fwd", "side", "quat", "drag", "height", "bob", "lean", "yaw", "the way up", "prediction inputs", "time to the top", "landing on the flight"];
    // the riders around the take-off on the snow have their ground tick replayed (F4h5a; TRICKY_NOREPLAY: taken from
    // the second sample, as before), with the control's clip table (ELF data)
    let replay_others = std::env::var("TRICKY_NOREPLAY").is_err();
    let wins = Wins { km, km6, ka };
    let table: Vec<u8> = (0..0x300 * 0x1c).step_by(4).flat_map(|o| runners[0].1.r.mem.read_u32(animctl::CLIP_TABLE + o).to_le_bytes()).collect();
    let addrs = rider_addrs(path);
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
    for (wi, w) in ticks.windows(2).enumerate() {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        // the keyframed objects where the game had them for this frame's casts (F4h5l)
        if let (Some(k), Some(c)) = (&keyframed, &course) {
            k.pose_ours(c, wi + 1);
            for (_, g, _, _) in runners.iter_mut() {
                k.pose_runner(g, wi + 1);
            }
        }
        let wp = (wi > 0 && ticks[wi - 1].tick + 1 == w[0].tick).then(|| &ticks[wi - 1].riders[..]);
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
            // from the bump state (2) the take-off's `Boarder_SetState`(0xd) runs `BumpControl_Exit` 0x104388 after the
            // motion, which zeroes the brake and the crouch (+0x1fc..+0x210): the motion's are the first sample's eased
            // as `cBoarder::vf7` eases them (F4h5c)
            if rider.state == 2 {
                (rider.brake, rider.crouch) = (eased(a, 0x1fc, pcsx2_add), eased(a, 0x208, pcsx2_add));
            }
            let kind = u32_le(b, 0x2e0);
            let hit = (kind != 3).then(|| Hit { kind, point: vec4(b, 0x2d0), normal: vec4(b, 0x2a0), surface: u32_le(b, 0x290) });
            let crouch0 = f32_at(a, 0x1c8);
            // the riders that tick before it in the frame push and hit it before its motion (F4h4), each way with
            // the riders around it replayed its own way (F4h5a)
            let others: Vec<Option<Other>> = (0..n).map(|o| (o != i && replay_others).then(|| other_of(&w[0], &w[1], o, &stats[o], addr[o], wins, &course)).flatten()).collect();
            let lists: Vec<Vec<usize>> = (0..n).map(|o| pair_list(&w[(std::env::var("TRICKY_LIST").as_deref() == Ok("b")) as usize], &addrs, o).unwrap_or_else(|| (0..n).filter(|&x| x != o).collect())).collect();
            let (w0r, w1r, tick) = (&w[0].riders, &w[1].riders, w[1].tick as i32);
            let poses: Vec<Option<Pose>> = (0..n).map(|o| pose_of(&w[0], o, kpose, &table)).collect();
            let flags: Vec<Option<bool>> = (0..n).map(|o| shoving_of(w, o, ka)).collect();
            let mut runs: Vec<Option<FrameRun>> = vec![None; ways];
            let mut starts = vec![(rider.p.pos, rider.vel); ways];
            if pushes {
                for k in 0..ways {
                    let (run, pos, vel) = if k == 0 {
                        FrameRun::before(wp, w0r, w1r, &stats, &twins, adder(""), Some((&w[1].extra[ka].1[..], &table[..])), &poses, &flags, &order, tick, i, others.clone(), lists.clone(), &mut |o, x, p, v, bo| tick_ours(x, p, v, bo, &w0r[o], &w1r[o], &table))
                    } else {
                        let add = adder(runners[k - 1].0);
                        let g = &mut runners[k - 1].1;
                        FrameRun::before(wp, w0r, w1r, &stats, &twins, add, Some((&w[1].extra[ka].1[..], &table[..])), &poses, &flags, &order, tick, i, others.clone(), lists.clone(), &mut |o, x, p, v, bo| tick_runner(g, x, p, v, bo, &w0r[o], &w1r[o], addr[o]))
                    };
                    starts[k] = (pos, vel);
                    runs[k] = Some(run);
                }
            }
            // the jump control fires on this tick: state 0xa and the crouch unwound to 0 before the motion
            let jumps = rider.state == 0xa && rider.crouch == 0.0;
            // a revert in the motion (`Boarder_SetState`(4) while riding backward) enters `ChangeFakieControl_Enter`,
            // which negates the steer and +0x244 the second sample holds: before it the motion had them the other way
            // round (F4h5h; found by a run of ours)
            let mut dry = rider;
            (dry.p.pos, dry.vel) = starts[0];
            let mut e = E { log: Vec::new(), hit, wall: false, meter: 0.0, states: false, walls: walls_of(&course, a, st) };
            update(&mut dry, &SURFACE_TABLE, &mut e);
            if e.log.contains(&Ev::State(4)) {
                (rider.steer, rider.p.x244) = (-rider.steer, -rider.p.x244);
            }

            // ours
            let mut mine = rider;
            (mine.p.pos, mine.vel) = starts[0];
            let mut e = E { log: Vec::new(), hit, wall: false, meter: 0.0, states: true, walls: walls_of(&course, a, st) };
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
            for (k, (_, g, off, ctl)) in runners.iter_mut().enumerate() {
                *g.hit.borrow_mut() = hit;
                g.wall.set(false);
                g.log.borrow_mut().clear();
                *off.borrow_mut() = Off::default();
                let mut rider = rider;
                (rider.p.pos, rider.vel) = starts[k + 1];
                g.put(&rider);
                // the control states' objects `Boarder_SetState` reaches: the change fakie's (+0x650, word 0) and the
                // natural air's (+0x700, word 3) point at the rider
                g.r.mem.write_u32(g.handler + 0x650, g.rd);
                g.r.mem.write_u32(g.handler + 0x70c, g.rd);
                g.r.mem.write_u32(g.rd + 0x424, 2);
                g.r.mem.write_f32(g.rd + 0x1c8, crouch0);
                g.r.mem.write_u32(g.rd + 0x1dc, u32_le(a, 0x1dc));
                g.r.mem.write_u8(g.stats + 0x17, st[0x17]);
                g.put_walls(walls_of(&course, a, st).as_ref());
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
            for (k, (r, _)) in got.iter_mut().enumerate() {
                if let Some(run) = runs[k].take() {
                    let m = u32_le(b, 0x424);
                    let (pos, vel, np, _) = if k == 0 {
                        run.after(w0r, w1r, &stats, &order, r, m, &mut |o, x, p, v, bo| tick_ours(x, p, v, bo, &w0r[o], &w1r[o], &table))
                    } else {
                        let g = &mut runners[k - 1].1;
                        run.after(w0r, w1r, &stats, &order, r, m, &mut |o, x, p, v, bo| tick_runner(g, x, p, v, bo, &w0r[o], &w1r[o], addr[o]))
                    };
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

/// F4b3k2: `cComputer::GetControls` 0x1384b8 (`aipad::get_controls`) against the pad word the hook kept
/// (`hook_pad`), for the control states whose routine reads nothing of the computer (5, 9, 0x11, 10 and the states
/// the switch has no case for: 0) and the stuck check's (0x14, and 0x13 `AIComputer_ControlsIdle` 0x138b60: only bit 0
/// may be set). The state is the first sample's +0x428 (the control runs before the
/// motion, `cBoarder::vf7` 0x117198); a rider that did not tick once in the frame is left out (its +0x5900 count
/// not the first's + 1), and so is one whose state changed in the frame (rec22: 9 → 0x10 with an air trick's pad
/// word, so the state moved before the control). The player (stats +0x6c not −1) is left out. Prints, by state:
/// ticks, pad words equal.
#[test]
fn computer_pad_words_against_the_running_game() {
    use tricky_game::aipad::{get_controls, AiPadEnv};
    struct Const;
    impl AiPadEnv for Const {
        fn rubber_band(&mut self) -> f32 {
            1.0
        }
        fn dispatch(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn countdown(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn turn_around(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn ride(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn air(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn air_trick(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn idle(&mut self, _: &mut u32) {
            unreachable!()
        }
        fn check_stuck(&mut self) -> u32 {
            0
        }
        fn update_skill_scale(&mut self) {}
    }
    let (Ok(paths), Ok(stats)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS")) else { return };
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let path = paths.split(';').next().unwrap();
    let (n, ticks) = load(path);
    let Some(kc) = ticks[0].extra.iter().position(|(o, _)| *o == 0x5900) else { return };
    let ks = ticks[0].extra.iter().position(|(o, _)| *o == 0x57b0);
    // the computer object's +0x830.. (window `5ae0>830`: +0x868 the side, +0x86c the skill) and the race's clock
    // (`clock`), for the countdown (F4b3k2)
    let kh = ticks[0].chased.iter().position(|(s, _)| s == "5ae0>830");
    let clock_a = std::env::var("TRICKY_CLOCK").as_deref() == Ok("a");
    // the frame the AI steers by (window `4aa0`: forward, side, up), for the turn-around's steer (F4b3k2);
    // TRICKY_TURN=<p><f><t>: the samples (a or b) of the position, the frame and the line point +0x360
    let kf = ticks[0].extra.iter().position(|(o, _)| *o == 0x4aa0);
    let turn = std::env::var("TRICKY_TURN").unwrap_or("bab".into()).into_bytes();
    let pick = |k: usize| (turn.get(k) == Some(&b'b')) as usize;
    let mut by: std::collections::BTreeMap<u32, [usize; 2]> = Default::default();
    // the dispatch states (F4b3k2d): the course's AI paths from the player's disc, the clip table from TRICKY_ELF, the
    // layer clips (`46c0`); by the riding mode at the first sample (object +0x89c)
    let paths = ai_paths(path);
    let clip_types: Option<Vec<u8>> = std::env::var("TRICKY_ELF").ok().and_then(|e| std::fs::read(e).ok()).map(|b| b[(animctl::CLIP_TABLE - 0xff000) as usize..][..0x300 * 0x1c].to_vec());
    let ka = ticks[0].extra.iter().position(|(o, _)| *o == 0x46c0);
    let mut by_mode: std::collections::BTreeMap<u32, [usize; 2]> = Default::default();
    let mut changed = 0;
    for w in ticks.windows(2) {
        for i in (0..n).filter(|&i| u32_le(&stats[i], 0x6c) == u32::MAX) {
            let Some(pad) = hook_pad(&w[1], i) else { continue };
            if f32_at(&w[1].extra[kc].1[i], 0) != f32_at(&w[0].extra[kc].1[i], 0) + 1.0 {
                continue;
            }
            let state = u32_le(&w[0].riders[i], 0x428);
            if state != u32_le(&w[1].riders[i], 0x428) {
                changed += 1;
                continue;
            }
            let e = by.entry(state).or_default();
            e[0] += 1;
            let slow = ks.map(|k| f32_at(&w[1].extra[k].1[i], 8));
            // the countdown (`aictl::controls_countdown`) with the object and the clock recorded: the clock the second
            // sample's (TRICKY_CLOCK=a: the first's), the leans +0x238 / +0x240 and the side +0x868 the first's
            let countdown = (state == 8).then(|| {
                let h = &w[0].chased[kh?].1[i];
                let time = if clock_a { w[0].clock } else { w[1].clock }?;
                let (a, mut side) = (&w[0].riders[i], f32_at(h, 0x38));
                let mut word = 0;
                tricky_game::aictl::controls_countdown(&mut word, f32_at(h, 0x3c), time, f32_at(a, 0x238), f32_at(a, 0x240), &mut side);
                Some(word)
            }).flatten();
            let dispatch = matches!(state, 2 | 3 | 4 | 7 | 0xb | 0xc | 0x12).then(|| dispatch_word(w, i, n, kh?, kf?, ka?, paths.as_deref()?, clip_types.as_deref()?, &mut by_mode)).flatten();
            let routine = matches!(state, 0xe | 0xf | 0x10) || state == 8 && countdown.is_none() || matches!(state, 2 | 3 | 4 | 7 | 0xb | 0xc | 0x12) && dispatch.is_none();
            let ok = match state {
                _ if routine => false,
                // the stuck check alone (0x14), or after the idle's clearing of the zero word (0x13, `aictl::controls_idle`)
                0x13 | 0x14 => pad & !1 == 0,
                // the turn-around (`aictl::controls_turn_around`): the stick's y alone, and only under 0.5 (rider+0x57b8,
                // the second sample's: it moves before the control, rec22 2401 r3, 2602 r1, 10463 r0 cross 0.5 in the
                // frame); its steer (`AIComputer_SteerTowards`) is not recorded
                0xd => match (slow, kh, kf) {
                    // with the object and the frame recorded, the whole word (`aicomp::steer_towards` toward the line
                    // point +0x360, the skill +0x86c)
                    (Some(v), Some(kh), Some(kf)) => {
                        let fr = &w[pick(1)].extra[kf].1[i];
                        let f = tricky_game::aicomp::Frame { pos: vec4(&w[pick(0)].riders[i], 0x140), fwd: vec4(fr, 0), side: vec4(fr, 0x10), up: vec4(fr, 0x20) };
                        let skill = f32_at(&w[0].chased[kh].1[i], 0x3c);
                        let target = vec4(&w[pick(2)].riders[i], 0x360);
                        let mut word = 0;
                        tricky_game::aictl::controls_turn_around(&mut word, v, || tricky_game::aicomp::steer_towards(skill, &f, target));
                        word == pad
                    }
                    (Some(v), _, _) if v < 0.5 => pad & !0x3f00 == 0,
                    (Some(_), _, _) => pad == 0,
                    (None, _, _) => false,
                },
                8 => countdown == Some(pad),
                2 | 3 | 4 | 7 | 0xb | 0xc | 0x12 => {
                    let d = dispatch.unwrap();
                    if d.0 != pad && std::env::var("TRICKY_DBGD").is_ok() {
                        eprintln!("DBGD tick {} rider {i} state {state:#x} mode {} -> {}: pad {pad:#x} ours {:#x} (xor {:#x})", w[1].tick, d.1, d.2, d.0, d.0 ^ pad);
                    }
                    let m = by_mode.entry(d.1).or_default();
                    m[1] += (d.0 == pad) as usize;
                    d.0 == pad
                }
                _ => get_controls(&mut Const, state, &mut 1.0) == pad,
            };
            e[1] += ok as usize;
            if !ok && !routine && std::env::var("TRICKY_DBG").is_ok() {
                eprintln!("DBG pad tick {} rider {i} state {state:#x} -> {:#x}: pad {pad:#x} slow {slow:?} -> {:?}", w[1].tick, u32_le(&w[1].riders[i], 0x428), ks.map(|k| f32_at(&w[1].extra[k].1[i], 8)));
            }
        }
    }
    eprintln!("{path}: the computer's pad words by control state [ticks, equal (the states without a routine of their own only)], the state the same at both samples ({changed} ticks where it changed left out): {:?}", by.iter().map(|(s, v)| (format!("{s:#x}"), *v)).collect::<Vec<_>>());
    eprintln!("{path}: the dispatch states by riding mode (object +0x89c at the first sample) [ticks, equal]: {by_mode:?}");
}

/// The AI paths of the recording's course (`course N`) from the player's disc (TRICKY_DISC), as the game keeps them.
fn ai_paths(path: &str) -> Option<Vec<tricky_game::aipath::Path>> {
    let folder = ["gari", "snowdream", "elysium", "mesablanca", "merqury", "aloha", "pipedream", "untracked", "megaplex", "", "trick", "alaska"].get(recording::course(path)? as usize)?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&std::env::var("TRICKY_DISC").ok()?).ok()?);
    let aip = tricky_data::aip::from_disc(&mut files, folder, "aip")?;
    Some(aip.ai_paths.iter().map(tricky_game::aipath::Path::from_aip).collect())
}

/// The path rider `r` follows (+0x57a4, not recorded): the one whose point at the rider's distance along (+0x344) is
/// nearest the rider's nearest point (+0x350), and how far (cm)
fn path_of(paths: &[tricky_game::aipath::Path], r: &[u8]) -> (usize, f32) {
    let (at, near) = (f32_at(r, 0x344), vec4(r, 0x350));
    paths.iter().enumerate().map(|(k, p)| {
        let q = p.position_at(at).0;
        (k, ((q[0] - near[0]).powi(2) + (q[1] - near[1]).powi(2) + (q[2] - near[2]).powi(2)).sqrt())
    }).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap()
}

/// `AIComputer_ControlsDispatch` 0x1389c8 for computer rider `i` over the frame `w` (F4b3k2d): the stuck check into
/// bit 0 (`aictl::check_stuck`), the shove (`aicomp::find_nearby_rider`, bits 23–24), then the riding mode
/// (`aicomp::Ai::ride`, by object +0x89c). The object (window `5ae0>830` from +0x830: +0x868 the speed, +0x86c the
/// skill, +0x890 the target, +0x894 / +0x898 the stuck counters, +0x89c the mode) from the first sample; the rider's
/// fields from TRICKY_DISP=<rider><frame> (a or b: the samples of the rider's 0x700 bytes and of the frame `4aa0`;
/// default `ba`, as the turn-around, F4b3k2c). The race mode word 0x32de8c (not recorded) TRICKY_GMODE (default 1).
/// The word, the mode it began in, the mode it ended in.
#[allow(clippy::too_many_arguments)]
fn dispatch_word(w: &[recording::Tick], i: usize, n: usize, kh: usize, kf: usize, ka: usize, paths: &[tricky_game::aipath::Path], clip_types: &[u8], by_mode: &mut std::collections::BTreeMap<u32, [usize; 2]>) -> Option<(u32, u32, u32)> {
    use tricky_game::aicomp::{self, Ai, GrudgeOf, Mode, World};
    let sel = std::env::var("TRICKY_DISP").unwrap_or("ba".into()).into_bytes();
    let pick = |k: usize| (sel.get(k) == Some(&b'b')) as usize;
    let gmode: u32 = std::env::var("TRICKY_GMODE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let h = &w[0].chased[kh].1[i];
    let o = |off: usize| off - 0x830;
    let r = &w[pick(0)].riders[i];
    let fr = &w[pick(1)].extra[kf].1[i];
    let tick = w[1].tick as i32;
    let me = u32_le(r, 0x460) as i32;
    let rs = std::env::var("TRICKY_RECS").unwrap_or_default();
    let rfs = |j: usize| match rs.as_str() { "a" => 0, "o" => (j > i) as usize, "p" => (j < i) as usize, _ => 1 };
    let recs: Vec<[f32; 2]> = (0..n).map(|j| [f32_at(&w[rfs(j)].riders[i], 0x40 + 0x20 * j), f32_at(&w[rfs(j)].riders[i], 0x44 + 0x20 * j)]).collect();
    let grudge = GrudgeOf { on: u32_le(r, 0x118) as i32, target: u32_le(r, 0x120) as i32, level: u32_le(r, 0x124) as i32, cooled: u32_le(r, 0x128) as i32 };
    let skill = f32_at(h, o(0x86c));
    let heading = f32_at(r, 0x1b0);
    // the stuck check
    let ahead = u32_le(r, 0x100) as i32;
    let ahead_rec = (0 <= ahead).then(|| recs[ahead as usize]);
    let mut s = tricky_game::aictl::StuckIn {
        heading: f32_at(r, 0x370),
        path: heading,
        state: u32_le(r, 0x424),
        state2: u32_le(r, 0x428),
        speed: f32_at(r, 0x348),
        f57fc: f32_at(&w[pick(0)].extra[w[0].extra.iter().position(|(x, _)| *x == 0x57b0)?].1[i], 0x4c),
        mode: gmode,
        skill,
        w890: u32_le(h, o(0x890)) as i32,
        c894: u32_le(h, o(0x894)),
        c898: u32_le(h, o(0x898)),
    };
    let stuck = tricky_game::aictl::check_stuck(&mut s, &mut || tricky_game::aictl::below_gap_min(ahead_rec, heading, f32_at(r, 0x104)), &mut || tricky_game::aictl::above_gap_max(ahead_rec, heading, f32_at(r, 0x108)));
    let mut word = stuck as u32;
    // the shove: not while the layer-1 clip is a shove (type 0x15, `AnimCtl_GetClipType`)
    let c1 = u32_le(&w[0].extra[ka].1[i], 0xc);
    let shoving = c1 != animctl::NONE && u32_le(clip_types, c1 as usize * 0x1c) == 0x15;
    word = aicomp::shove_bits(word, aicomp::find_nearby_rider(shoving, tick, &recs, heading, me, n as i32, &grudge));
    // the riding mode
    let mode0 = u32_le(h, o(0x89c)) & 0xffff;
    by_mode.entry(mode0).or_default()[0] += 1;
    let mode = [Mode::ToJump, Mode::Line, Mode::NearRider, Mode::AtRival][mode0 as usize];
    let (pk, _) = path_of(paths, r);
    let events: Vec<aicomp::PathEvent> = paths[pk].events.iter().map(|&(kind, value, start, end)| aicomp::PathEvent { kind, value, start, end }).collect();
    let (from, at) = (f32_at(r, 0x340), f32_at(r, 0x344));
    let mut jump = |sp: &mut f32| aicomp::find_jump_marker(&events, from, at, sp);
    let mut hint = |sp: &mut f32| aicomp::find_speed_hint(&events, from, at, sp);
    // the other riders as rider i sees them (TRICKY_OTHERS=a or b: one sample for all; default: the second for those
    // that tick before it in the frame, the riders running 5 .. 0, F4b3z)
    let os = std::env::var("TRICKY_OTHERS").unwrap_or_default();
    let ofs = |j: usize| match os.as_str() { "a" => 0, "b" => 1, _ => (j > i) as usize };
    let others: Vec<([f32; 4], [f32; 4])> = (0..n).map(|j| (vec4(&w[ofs(j)].riders[j], 0x140), vec4(&w[ofs(j)].riders[j], 0x150))).collect();
    let other = |j: i32| others[j as usize];
    let mut world = World { jump_marker: &mut jump, speed_hint: &mut hint, other: &other, tick, n: n as i32, practice: gmode == 0 };
    let mut ai = Ai {
        skill,
        speed: f32_at(h, o(0x868)),
        target: u32_le(h, o(0x890)) as i32,
        mode,
        frame: aicomp::Frame { pos: vec4(r, 0x140), fwd: vec4(fr, 0), side: vec4(fr, 0x10), up: vec4(fr, 0x20) },
        vel: vec4(r, 0x150),
        line: vec4(r, 0x360),
        travel: f32_at(r, 0x370),
        heading,
        x348: f32_at(r, 0x348),
        meter: f32_at(r, 0x1c),
        me,
        recs,
        grudge,
    };
    if std::env::var("TRICKY_DBGN").is_ok_and(|v| v == w[1].tick.to_string()) {
        let t = ai.target.max(0) as usize;
        let sp = |v: [f32; 4]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        eprintln!("DBGN tick {} rider {i}: target {} rec {:?} travel {} speed {} vs other a {} b {} (used {}), steer {}", w[1].tick, ai.target, ai.recs.get(t), ai.travel, sp(ai.vel), sp(vec4(&w[0].riders[t], 0x150)), sp(vec4(&w[1].riders[t], 0x150)), sp(others[t].1), aicomp::steer_towards(skill, &ai.frame, ai.line));
    }
    let mut calls = Vec::new();
    let word = ai.ride(&mut world, word, &mut calls);
    Some((word, mode0, ai.mode as u32))
}

/// The `rollers` window's layout (F4b3k3, `pine.py record ... rollers`): a count byte, then each roller's node,
/// instance, 0x150 bytes from +0x30 and the box.
#[test]
fn rollers_window_parses() {
    let mut b = vec![2u8];
    for k in 0..2u32 {
        b.extend((0x1000 + k).to_le_bytes());
        b.extend((0x2000 + 0x100 * k).to_le_bytes());
        b.extend((0..0x150).map(|x| x as u8));
        for v in [1.0f32, 2.0, 3.0, 0.0, 4.0, 5.0, 6.0, 0.0] {
            b.extend(v.to_le_bytes());
        }
    }
    let r = recording::parse_rollers(&b);
    assert_eq!(r.len(), 2);
    assert_eq!((r[1].node, r[1].instance, r[1].body[0x9c], r[1].min, r[1].max), (0x1001, 0x2100, 0x9c, [1.0, 2.0, 3.0, 0.0], [4.0, 5.0, 6.0, 0.0]));
    assert!(recording::parse_rollers(&[0]).is_empty());
}

/// `pine.py record ... flags clock` lines (F4b3k4, F4b3k2): the ` F ` tail read as (index, flags), `-` as none; the
/// ` C ` the race clock's bits.
#[test]
fn flags_tail_parses() {
    let path = std::env::temp_dir().join(format!("tricky_flags_{}.txt", std::process::id()));
    std::fs::write(&path, "race 1 riders 10 bytes 4 pad 0 flags 3 course 1\ntick 5 00000000 C c3950000 F 0:10003,2:a100a3\ntick 6 00000000 F -\n").unwrap();
    let (n, ticks) = load(path.to_str().unwrap());
    std::fs::remove_file(&path).ok();
    assert_eq!(n, 1);
    assert_eq!(ticks[0].flags, vec![(0, 0x10003), (2, 0xa100a3)]);
    assert!(ticks[1].flags.is_empty());
    assert_eq!((ticks[0].clock, ticks[1].clock), (Some(-298.0), None));
}

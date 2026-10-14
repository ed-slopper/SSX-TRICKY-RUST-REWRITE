//! The app's wipe-out on the game's (board row G6c): `WipeoutMotion_Update` 0x10c2f0 (motion 5, state 0x13) and
//! `GetUpMotion_Update` 0x1102c8 (motion 6, state 0x14) as `crate::wipeout_motion` ports them, on the wipe-out body
//! `crate::ragdoll` (`Ragdoll_Step` 0x10dca8, `Ragdoll_Contacts` 0x10e190, `Ragdoll_GroundProbe` 0x10fd58), each
//! 60 Hz tick in the game's units (cm, z up), its casts on the course's collision (`CollisionWorld::course`, rows
//! E7c3 and E7c3e).
//!
//! What the motion asks of the rest of the game is answered here ([`Glue`]); the stand-ins among it are marked.
use super::*;
use super::ground_tick::{bind_body_at, kick_rollers, quat_of, rider_id, roller_cast, to_game, to_port, touch_query, wall_cast};
use crate::air::{self, Predict, RayHit};
use crate::collbody::CollBody;
use crate::orient::{self, Orient};
use crate::probe::{cross, dot, scale};
use crate::ragdoll::{CastOut, RagEnv, RagRider};
use crate::roller::RollerEnv;
use crate::sphtree::WorldSphTree;
use crate::terrain::LastHit;
use crate::walls::{CastRecord, Contact, Touched, WallEnv, WallRider};
use crate::wipeout_motion::{self as wm, WipeEnv, WipeRider, Wipeout};
use crate::worldboarder::WorldBoarder;
use crate::worldline::WorldLine;
use glam::{Mat3, Quat};

type V4 = [f32; 4];

/// The word at 0x31d314 (0x273c3, the data's): the body's spheres `Ragdoll_Contacts` casts.
const RAG_MASK: i32 = 0x273c3;
/// The word at 0x31d310 (0xfc3, the data's): rider+0x494 while the wipe-out's walls run.
const WALL_MASK: i32 = 0xfc3;
/// One game tick (s)
const TICK: f32 = 1.0 / 60.0;

/// The wipe-out while it runs: the motion (+0x4f0 of the rider's motion object), the rider's fields it works on
/// and what the glue keeps for it.
#[derive(Clone, Debug)]
pub struct Wipe {
    /// the body's orientation for drawing (Bevy: local −Z the board's nose, +Y its up, +X its right)
    pub q: Quat,
    /// seconds down
    pub t: f32,
    /// the clip playing (the original's id) and seconds into it
    pub clip: u16,
    pub clip_t: f32,
    motion: Wipeout,
    rag: RagRider,
    wr: WipeRider,
    s: State,
    /// seconds not yet run as game ticks
    acc: f32,
}

/// What the glue keeps between calls: the motion and control state the motion set, the clip (STANDIN: our clip
/// clock, for the animation controller rider+0x46c0, row G5), the board's forward and side and the heading as
/// `Boarder_UpdateOrientation` last wrote them (+0x180, +0x190, +0x1b0), the probe's +0x330 and +0x29c, the flight
/// prediction (rider+0x57b0).
#[derive(Clone, Debug, Default)]
struct State {
    motion: u32,
    state: u32,
    clip: u16,
    clip_t: f32,
    fwd: V4,
    side: V4,
    yaw: f32,
    probe_side: V4,
    surface_ticks: u32,
    predict: Predict,
    /// +0x2f0: the last impact's normal (`Boarder_AccumulateImpact`)
    impact_n: V4,
    /// what this tick's impacts add to +0x2e4 (folded into the motion's copy after the tick)
    impact: f32,
    /// the rider's speed this tick (cm/s), for the knock sounds
    speed: f32,
    /// the sphere tree the last object query hit (the record's +0x30) and the object a contact was last for
    tree: Option<(usize, usize)>,
    contact_obj: u32,
    /// scripts the rollers the body hit started (`cScriptNode`(3, script, object))
    starts: Vec<(i32, u32)>,
    /// `Boarder_Wipeout` 0x11d830 again from the walls during the get-up: its contact
    rewipe: Option<Contact>,
    /// what the rider itself does after the tick
    calls: Vec<Call>,
}

/// What the walls ask of the rider during the wipe-out, done after the tick on the port's rider.
#[derive(Clone, Copy, Debug)]
enum Call {
    /// `Boarder_Wipeout` 0x11d830's scoring side, then `Score_Crash` 0x156de0 and `Boarder_AddMeter` 0x11b018 with
    /// its answer (the walls' impact check)
    Crash,
    /// `Boarder_AddMeter` 0x11b018
    Meter(f32),
    /// the rider's controller's virtual +0x64 (`cBoarder::EmitBrakeSpray` 0x11a798: point, velocity, normal)
    Spray(V4, V4, V4),
}

/// The rider as the scripts, the rollers and the walls see it: its handle (`rider_id`), human or not, and
/// `Boarder_GetSpeedStat` 0x11d7b0.
#[derive(Clone, Copy, Debug, Default)]
struct Who {
    id: u32,
    human: bool,
    speed_stat: f32,
}

/// `Boarder_AccumulateImpact` 0x125c48: +0x2e4 += 1 − max(n · +0x2f0, 0), +0x2f0 = n.
fn accumulate(s: &mut State, n: V4) {
    let d = dot(n, s.impact_n);
    s.impact += if 0.0 < d { 1.0 - d } else { 1.0 };
    s.impact_n = n;
}

/// What the motion calls, answered on the course and the port's rider.
struct Glue<'a> {
    world: &'a CollisionWorld,
    last: &'a mut LastHit,
    s: &'a mut State,
    /// stats+0x60 and rider+0x244 (the probe's start to the side)
    k: f32,
    lean: f32,
    who: Who,
}

/// The cast's record as the motion reads it: +0x34 the surface, +0x38 the object (index + 1, 0 none).
fn out_of(h: Option<crate::world::CastHit>) -> CastOut {
    h.map_or(CastOut { kind: 3, ..Default::default() }, |h| CastOut {
        kind: h.kind,
        pos: h.pos,
        normal: h.normal,
        x30: [0, h.surface as u32, h.object.map_or(0, |o| o as u32 + 1)],
    })
}

impl Glue<'_> {
    /// `World_RayCast` 0x25aff8 on the course with a `cWorldLine`(target, from, to, kind).
    fn line(&mut self, target: f32, from: V4, to: V4, kind: u32, last: bool) -> (f32, CastOut) {
        let world = self.world;
        let Some(course) = &world.course else { return mesh_line(world, from, to) };
        let mut q = WorldLine::new(target, from, to, kind);
        let last = if last { Some(&mut *self.last) } else { None };
        let (t, h) = course.ray_cast(&mut q, world.showoff, last, &|i| world.course_skips(i));
        (t, out_of(h.filter(|_| t >= 0.0)))
    }
}

/// STANDIN: the drawn mesh's ground across the middle of the line, for a level without the course's collision
/// (a levels folder; row E3a4c).
fn mesh_line(world: &CollisionWorld, from: V4, to: V4) -> (f32, CastOut) {
    let (a, b) = (to_port(from, 0.01), to_port(to, 0.01));
    let mid = (a + b) * 0.5;
    let half = (a.y - b.y).abs() * 0.5 + 0.5;
    match world.ground(mid, half, half) {
        Some(h) => {
            let at = Vec3::new(mid.x, h.y, mid.z);
            let t = ((at - a).length() / (b - a).length().max(1e-6)).clamp(0.0, 1.0);
            (t, CastOut { kind: 0, pos: to_game(at, 100.0, 1.0), normal: to_game(h.normal, 1.0, 0.0), x30: [0, h.surface as u32, 0] })
        }
        None => (-1.0, CastOut { kind: 3, ..Default::default() }),
    }
}

/// STANDIN: the deepest of the body's spheres (those in its +0x24 mask) in the drawn mesh's walls and ground, for a
/// level without the course's collision (row E3a4c).
fn mesh_body(world: &CollisionWorld, body: &CollBody) -> (f32, CastOut) {
    let mut best: Option<(f32, Vec3, Vec3, u8)> = None;
    for (i, &(c, r)) in body.spheres.iter().enumerate().take(body.count.max(0) as usize) {
        if body.x24 & (1 << i) == 0 {
            continue;
        }
        let (c, r) = (to_port(c, 0.01), r * 0.01);
        if let Some(h) = world.ground(c, r + 0.5, r + 0.5) {
            let d = (c.y - h.y) * h.normal.y;
            if d < r && best.is_none_or(|b| r - d > b.0) {
                best = Some((r - d, h.normal, c - h.normal * d, h.surface));
            }
        }
        let (c2, ns) = world.push_out(c, r);
        let depth = (c2 - c).length();
        if !ns.is_empty() && depth > 1e-5 && best.is_none_or(|b| depth > b.0) {
            let n = (c2 - c) / depth;
            best = Some((depth, n, c - n * (r - depth), 10));
        }
    }
    best.map_or((-1.0, CastOut { kind: 3, ..Default::default() }), |(d, n, at, surface)| {
        (d * 100.0, CastOut { kind: 0, pos: to_game(at, 100.0, 1.0), normal: to_game(n, 1.0, 0.0), x30: [0, surface as u32, 0] })
    })
}

/// What the rollers the body hits ask of the world (`Ragdoll_HitRoller` 0x10ed68 waking them, `roller::on_hit`).
impl RollerEnv for Glue<'_> {
    fn ray_cast(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        match &self.world.course {
            Some(c) => roller_cast(self.world, c, q, false),
            None => (-1.0, [0.0; 4], [0.0; 4]),
        }
    }
    fn query_instances(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        match &self.world.course {
            Some(c) => roller_cast(self.world, c, q, true),
            None => (-1.0, [0.0; 4], [0.0; 4]),
        }
    }
    /// STANDIN: nothing, for the race's broadphase (`Broadphase_MoveBox` 0x14fe68).
    fn move_box(&mut self, _: V4, _: V4) {}
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.world.course.as_ref().map_or(-1, |c| c.ssf.event_script(object as usize, event as usize))
    }
    fn start_script(&mut self, script: i32, object: u32) {
        self.s.starts.push((script, object));
    }
    /// `Sfx_InstanceCollision` 0x216c50: the object (a roller's index, or the contact's object, index + 1) and the
    /// rider's speed, for the app's sound.
    fn sfx(&mut self, _: u32, object: u32) {
        let o = if object != 0 && object == self.s.contact_obj { object - 1 } else { object };
        self.s.contact_obj = 0;
        if let Some(c) = &self.world.course {
            c.knock.lock().unwrap_or_else(|e| e.into_inner()).knocks.push((o, self.s.speed));
        }
    }
}

impl RagEnv for Glue<'_> {
    fn mask(&self) -> i32 {
        RAG_MASK
    }
    /// `World_RayCast` 0x25aff8 with the body's `cWorldBoarder`(+0x470, 0).
    fn cast_body(&mut self, q: &WorldBoarder) -> (f32, CastOut) {
        let world = self.world;
        let Some(course) = &world.course else { return mesh_body(world, &q.rider) };
        let mut q = q.clone();
        let (t, h) = course.ray_cast(&mut q, world.showoff, None, &|i| world.course_skips(i));
        (t, out_of(h.filter(|_| t > 0.0)))
    }
    /// `World_QueryInstances` 0x25bdc4 with it (nothing without the course's collision).
    fn query_body(&mut self, q: &WorldBoarder) -> (f32, CastOut) {
        let world = self.world;
        let Some(course) = &world.course else { return (-1.0, CastOut { kind: 3, ..Default::default() }) };
        let mut q = q.clone();
        let (t, h) = course.query_instances(&mut q, world.showoff, &|i| world.course_skips(i));
        self.s.tree = h.as_ref().and_then(|h| h.tree);
        (t, out_of(h.filter(|_| t > 0.0)))
    }
    /// `World_RayCast` 0x25aff8 with a `cWorldLine`(0.5, from, to, 1).
    fn cast_line(&mut self, from: V4, to: V4) -> (f32, CastOut) {
        self.line(0.5, from, to, 1, false)
    }
    /// `Boarder_AccumulateImpact` 0x125c48.
    fn accumulate_impact(&mut self, n: V4) {
        accumulate(self.s, n);
    }
    /// The rider's controller's virtual +0x64 (a human's: `cPlayer::vf11` 0x1532c0, `cBoarder::EmitBrakeSpray`
    /// 0x11a798, then `Rumble_Impact` 0x150a80): the spray kept in `crash_spray`, as the ground tick's. STANDIN: no
    /// rumble (the port drives no pad motors).
    fn rider_contact(&mut self, point: V4, vel: V4, n: V4) {
        self.s.calls.push(Call::Spray(point, vel, n));
    }
    fn object_flags(&self, object: u32) -> u32 {
        match (&self.world.course, object) {
            (Some(c), 1..) => c.object_flags(object as usize - 1),
            _ => 0,
        }
    }
    /// `Instance_OnPlayerContact` 0x13bd40 (`knockables::Knockables::contact`): the object (index + 1), the rider at
    /// the contact's velocity, the record with the tree the query hit. Without the course's collision: nothing.
    fn player_contact(&mut self, object: u32, c: &Contact, out: &CastOut) {
        self.s.contact_obj = object;
        let (Some(course), 1..) = (&self.world.course, object) else { return };
        let rider = crate::knockables::Rider { id: self.who.id, human: self.who.human, vel: scale(c.dir, c.speed_in) };
        let record = crate::knockables::record_context(out.pos, out.normal, self.s.tree);
        course.knock.lock().unwrap_or_else(|e| e.into_inner()).contact(course, object - 1, rider, c, record);
    }
    /// `Boarder_UpdateOrientation` 0x129008 (`orient::update_orientation`).
    fn update_orientation(&mut self, r: &mut RagRider) {
        let mut o = Orient { q: r.q, fwd: self.s.fwd, side: self.s.side, up: r.up, yaw: self.s.yaw };
        orient::update_orientation(&mut o);
        (r.q, r.up, self.s.fwd, self.s.side, self.s.yaw) = (o.q, o.up, o.fwd, o.side, o.yaw);
    }
}

impl WipeEnv for Glue<'_> {
    fn clip(&self) -> u32 {
        self.s.clip as u32
    }
    /// The clip's type: 0x11 the curled ones (category 17), else 0x12.
    fn clip_type(&self) -> u32 {
        if category(self.s.clip) == 17 { 0x11 } else { 0x12 }
    }
    fn play_clip(&mut self, clip: u32) {
        self.s.clip = clip as u16;
        self.s.clip_t = 0.0;
    }
    fn blend_out_all(&mut self) {}
    /// STANDIN: no displacement, for the clip's (`AnimCtl_GetClipDisplacement`, row G5).
    fn displacement(&mut self) -> V4 {
        [0.0; 4]
    }
    /// `Boarder_ProbeGround` 0x128ae8 (`probe::probe`) down `probe::line`, a `cWorldLine`(0.5, from, to, 2) through
    /// the rider's last-hit record (rider+0x430).
    fn probe_ground(&mut self, r: &mut RagRider, w: &mut WipeRider) -> bool {
        let mut p = crate::probe::Rider {
            pos: r.pos,
            fwd: w.fwd,
            normal: r.ground,
            ground: r.ground_at,
            plane_fwd: r.side,
            side: self.s.probe_side,
            fall: r.fall,
            x244: self.lean,
            stat60: self.k,
            height: r.height,
            surface: r.surface,
            surface_ticks: self.s.surface_ticks,
            kind: r.ground_kind,
        };
        let (from, to) = crate::probe::line(&p);
        let (t, out) = self.line(0.5, from, to, 2, true);
        let hit = (t >= 0.0).then_some(crate::probe::Hit { kind: out.kind, point: out.pos, normal: out.normal, surface: out.x30[1] });
        let on = crate::probe::probe(&mut p, hit);
        (r.ground, r.ground_at, r.side, r.fall, r.height, r.surface, r.ground_kind) = (p.normal, p.ground, p.plane_fwd, p.fall, p.height, p.surface, p.kind);
        (self.s.probe_side, self.s.surface_ticks) = (p.side, p.surface_ticks);
        on
    }
    /// `Boarder_RotateAboutAxis` 0x128e90 (`orient::rotate_about_axis`).
    fn rotate_about_axis(&mut self, r: &mut RagRider, w: &mut WipeRider, angle: f32, axis: V4) {
        let mut o = Orient { q: r.q, fwd: w.fwd, side: w.side, up: r.up, yaw: self.s.yaw };
        orient::rotate_about_axis(&mut o, angle, axis);
        (r.q, r.up, w.fwd, w.side, self.s.yaw) = (o.q, o.up, o.fwd, o.side, o.yaw);
        (self.s.fwd, self.s.side) = (o.fwd, o.side);
    }
    /// The surface table's gravity (+0x00).
    fn surface_g(&self, surface: u32) -> f32 {
        crate::surface::surf(surface as usize, 0)
    }
    /// With the mask 0xfc3 in the body: `Boarder_InstanceCollisions` 0x125088 (the touch objects,
    /// `walls::instance_collisions`, then the rollers, as the ground tick's) and `Boarder_WallCollide` 0x126250
    /// (`walls::wall_collide`). Without the course's collision only the walls (on the drawn mesh).
    fn collide_world(&mut self, r: &mut RagRider, w: &mut WipeRider) {
        let _ = w;
        let mut body = r.body;
        body.x24 = WALL_MASK;
        let mut wr = WallRider { pos: r.pos, vel: r.vel, ground: r.ground, knocks: 0.0, mode: self.s.motion, state: self.s.state, up: r.up };
        let mut walls = Walls { world: self.world, body, s: &mut *self.s, who: self.who };
        if let Some(course) = self.world.course.clone() {
            crate::walls::instance_collisions(&mut wr, &mut walls);
            kick_rollers(&course, &mut wr, self.who.id, &mut walls, |w| w.body);
        }
        crate::walls::wall_collide(&mut wr, &mut walls);
        (r.pos, r.vel) = (wr.pos, wr.vel);
        r.body = walls.body;
        r.body.x24 = -1;
    }
    /// `Boarder_Translate` 0x1267a0: the collision body moved (the broadphase box the port does not keep).
    fn translate(&mut self, r: &mut RagRider, d: V4) {
        r.body.translate(d);
    }
    /// `AirPredict_AtTakeoff` 0x123b10 (`air::predict_at_takeoff`, its rays `cWorldLine`(0, …, 0) on the course) or
    /// `AirPredict_Tick` 0x123ec8; the motion reads the prediction's +8, +0x20, +0x30 and +0x40.
    fn predict(&mut self, r: &RagRider, w: &mut WipeRider, takeoff: bool) {
        if takeoff {
            let mut ray = |a: V4, b: V4| {
                let (t, o) = self.line(0.0, a, b, 0, false);
                (t, RayHit { point: o.pos, normal: o.normal, word: o.x30[1] })
            };
            let p = air::predict_at_takeoff(r.pos, r.vel, r.ground, &mut ray);
            self.s.predict = p;
        } else {
            self.s.predict.tick();
        }
        let p = &self.s.predict;
        (w.t_land, w.land_v, w.land_n, w.land_surface) = (p.t, p.vel, p.normal, p.word);
    }
    /// `AirPredict_ClearLandTime` 0x123f00
    fn predict_reset(&mut self) {
        self.s.predict.t_land = 0.0;
    }
    fn set_motion(&mut self, m: u32) {
        self.s.motion = m;
    }
    fn set_state(&mut self, s: u32) {
        self.s.state = s;
    }
    /// STANDIN: the wipe-out's sounds are the port's own (`boardsound`), for `Sfx_*` (rows G10).
    fn sfx_enter(&mut self) {}
    fn sfx_impact(&mut self) {}
    fn sfx_slide(&mut self) {}
    fn sfx_stop(&mut self) {}
    fn sfx_leave(&mut self) {}
    fn rider_leave(&mut self) {}
    fn sync(&mut self, w: &mut WipeRider) {
        (w.fwd, w.side) = (self.s.fwd, self.s.side);
    }
}

/// What `Boarder_InstanceCollisions` and `Boarder_WallCollide` call during the wipe-out and the get-up.
struct Walls<'a> {
    world: &'a CollisionWorld,
    body: CollBody,
    s: &'a mut State,
    who: Who,
}

impl WallEnv for Walls<'_> {
    fn cast_walls(&mut self, r: &WallRider) -> (f32, CastRecord) {
        match &self.world.course {
            Some(course) => wall_cast(self.world, course, &self.body, r.ground),
            None => {
                let (t, o) = mesh_body(self.world, &self.body);
                (t, CastRecord { pos: o.pos, normal: o.normal, object: 0 })
            }
        }
    }
    /// `touch_query` (only with the course's collision).
    fn query_objects(&mut self, _: &WallRider) -> (f32, CastRecord, Touched) {
        let Some(course) = &self.world.course else { return (0.0, Default::default(), Default::default()) };
        let (t, rec, touched, tree) = touch_query(self.world, course, &self.body);
        self.s.tree = tree;
        (t, rec, touched)
    }
    /// `Boarder_AccumulateImpact` 0x125c48.
    fn accumulate_impact(&mut self, _: &mut WallRider, n: V4) {
        accumulate(self.s, n);
    }
    fn translate(&mut self, _: &mut WallRider, by: V4) {
        self.body.translate(by);
    }
    /// Only in motion 2.
    fn turn_from_wall(&mut self, _: &mut WallRider, _: V4) {}
    /// `Boarder_Wipeout` 0x11d830 again (only the get-up gets here: motion 5's impact check stops first): the motion
    /// starts over from this contact, after the tick.
    fn wipeout(&mut self, _: &mut WallRider, c: &Contact) {
        self.s.rewipe = Some(*c);
    }
    /// `Score_Crash` 0x156de0(rider+0x5820, 0), done on the port's rider after the tick with the meter it returns;
    /// so the `Boarder_AddMeter` after it adds nothing here.
    fn score_crash(&mut self, _: &mut WallRider) -> f32 {
        self.s.calls.push(Call::Crash);
        0.0
    }
    fn add_meter(&mut self, _: &mut WallRider, amount: f32) {
        if amount != 0.0 {
            self.s.calls.push(Call::Meter(amount));
        }
    }
    /// `Boarder_Stumble` 0x124dd0: not in state 2 and without the flag, −0.02 to the meter (0xbca3f553); the
    /// stumble itself only in states 3 and 0xe, never during the wipe-out (0x13) or the get-up (0x14).
    fn stumble(&mut self, _: &mut WallRider, _: V4, flag: u32) {
        if self.s.state != 2 && flag == 0 {
            self.s.calls.push(Call::Meter(f32::from_bits(0xbca3_f553)));
        }
    }
    fn speed_stat(&mut self, _: &WallRider) -> f32 {
        self.who.speed_stat
    }
    /// The rider's controller's virtual +0x6c (a human's: `cPlayer::vf12` 0x153350, `Rumble_Add`). STANDIN: nothing
    /// (no pad motors), as the ground tick's.
    fn knock(&mut self, _: &mut WallRider, _: f32) {}
    /// `Instance_OnPlayerContact` 0x13bd40 (`knockables::Knockables::contact`).
    fn contact(&mut self, object: u32, r: &mut WallRider, c: &Contact, rec: &CastRecord) {
        let Some(course) = &self.world.course else { return };
        let rider = crate::knockables::Rider { id: self.who.id, human: self.who.human, vel: r.vel };
        let record = crate::knockables::record_context(rec.pos, rec.normal, self.s.tree);
        course.knock.lock().unwrap_or_else(|e| e.into_inner()).contact(course, object, rider, c, record);
    }
    /// `Sfx_InstanceCollision` 0x216c50: the object and the rider's speed, for the app's sound.
    fn sfx_object(&mut self, r: &WallRider, object: u32) {
        if let Some(c) = &self.world.course {
            c.knock.lock().unwrap_or_else(|e| e.into_inner()).knocks.push((object, dot(r.vel, r.vel).sqrt()));
        }
    }
    /// STANDIN: nothing, for `Sfx_OnWallHit` 0x2162f0 (the port's wall sound is its own, rows G10).
    fn sfx_wall(&mut self, _: &WallRider) {}
}

/// What the rollers the body kicks ask of the world (`roller::rider_kick`).
impl RollerEnv for Walls<'_> {
    fn ray_cast(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        match &self.world.course {
            Some(c) => roller_cast(self.world, c, q, false),
            None => (-1.0, [0.0; 4], [0.0; 4]),
        }
    }
    fn query_instances(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        match &self.world.course {
            Some(c) => roller_cast(self.world, c, q, true),
            None => (-1.0, [0.0; 4], [0.0; 4]),
        }
    }
    /// STANDIN: nothing, for the race's broadphase (`Broadphase_MoveBox` 0x14fe68).
    fn move_box(&mut self, _: V4, _: V4) {}
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.world.course.as_ref().map_or(-1, |c| c.ssf.event_script(object as usize, event as usize))
    }
    fn start_script(&mut self, script: i32, object: u32) {
        self.s.starts.push((script, object));
    }
    /// `Sfx_InstanceCollision` 0x216c50 (the body's kick on a roller).
    fn sfx(&mut self, _: u32, object: u32) {
        if let Some(c) = &self.world.course {
            c.knock.lock().unwrap_or_else(|e| e.into_inner()).knocks.push((object, self.s.speed));
        }
    }
}

/// How a tick of the motion ended.
enum Out {
    Down,
    /// out of it through the air (xA_FALLRECOVER, motion 1 state 0xd) or on the snow (motion 2 state 3; the get-up's
    /// end), with the clip that brings the body back to its base pose (0 for none)
    Air(u16),
    Ride(u16),
    /// the time-out's reset request (+0x2e4 = 1e6)
    Reset,
}

impl Wipe {
    /// One game tick of motion 5 or 6.
    fn tick(&mut self, world: &CollisionWorld, last: &mut LastHit, k: f32, lean: f32, kind: u8, who: Who) -> Out {
        let was = self.s.motion;
        self.s.speed = dot(self.rag.vel, self.rag.vel).sqrt();
        // the rollers among the rider's broadphase pairs, for `Ragdoll_Contacts`. STANDIN: those whose boxes meet
        // the body's, for the race's broadphase (rider+0x700)
        let course = world.course.clone();
        let mut all = course.as_ref().map_or(Vec::new(), |c| std::mem::take(&mut c.knock.lock().unwrap_or_else(|e| e.into_inner()).rollers));
        let (c, rad) = (self.rag.body.center, self.rag.body.radius);
        let (lo, hi) = ([c[0] - rad, c[1] - rad, c[2] - rad, 0.0], [c[0] + rad, c[1] + rad, c[2] + rad, 0.0]);
        let near: Vec<usize> = (0..all.len()).filter(|&i| crate::knockables::overlap(all[i].body.lo, all[i].body.hi, lo, hi)).collect();
        let mut pairs: Vec<(u32, crate::roller::RollerBody)> = near.iter().map(|&i| (all[i].object, all[i].body.clone())).collect();
        let mut glue = Glue { world, last, s: &mut self.s, k, lean, who };
        if was == 5 {
            self.motion.update(&mut self.rag, &mut self.wr, &mut pairs, &mut glue);
        } else {
            wm::get_up_update(&mut self.rag, &mut self.wr, &mut glue);
        }
        let now = glue.s.motion;
        if now != was && was == 5 {
            self.motion.exit(now, &mut glue);
        }
        // the get-up's walls called `Boarder_Wipeout` 0x11d830: the contact into the body (+0x510, +0x530), state
        // 0x13, `Boarder_SetMotion`(5) (`WipeoutMotion_Enter` 0x10c198)
        if let Some(c) = glue.s.rewipe.take() {
            (self.motion.rag.crash_at, self.motion.rag.crash_n) = (c.pos, c.normal);
            (glue.s.motion, glue.s.state) = (5, 0x13);
            self.motion.enter(&mut self.rag, &mut self.wr, &mut glue);
        }
        if let Some(c) = &course {
            for (&i, (_, b)) in near.iter().zip(pairs) {
                all[i].body = b;
            }
            let starts = std::mem::take(&mut self.s.starts);
            let mut kn = c.knock.lock().unwrap_or_else(|e| e.into_inner());
            let mut new = std::mem::take(&mut kn.rollers);
            new.append(&mut all);
            kn.rollers = new;
            kn.start_scripts(c, &starts);
        }
        self.wr.stuck += std::mem::take(&mut self.s.impact);
        // STANDIN: the collision body re-posed from the bind pose where the rider now is, for
        // `CollBody_UpdateFromBones` 0x2361b0 on the animated skeleton (row G5)
        self.rag.body = bind_body_at(self.rag.pos, self.wr.fwd, self.rag.up, k);
        self.s.clip_t += TICK;
        // cBoarder's update (vf7 0x117198) after the motion: +0x2e4 decays (× 0.95603746); above 4.492007,
        // `Boarder_RequestReset` 0x118f10 and 0 (the motion's time-out sets 1e6)
        self.wr.stuck *= 0.956_037_46;
        if 4.492_007 < self.wr.stuck {
            self.wr.stuck = 0.0;
            return Out::Reset;
        }
        match now {
            1 => Out::Air(self.s.clip),
            2 => Out::Ride(self.s.clip),
            // STANDIN: the get-up ends with its clip, 2n − 1 ticks for an n-frame clip, for the animation's end
            // action (row G5)
            6 if self.s.clip_t >= (2.0 * clip_frames(self.s.clip, kind) - 1.0) / 60.0 => Out::Ride(0),
            _ => Out::Down,
        }
    }

    /// The body's orientation for drawing, from the board's side, forward and up (Bevy: +X right, +Y up, −Z nose).
    fn draw_q(&self) -> Quat {
        let (x, y, z) = (to_port(self.wr.side, 1.0), to_port(self.rag.up, 1.0), to_port(self.wr.fwd, 1.0));
        Quat::from_mat3(&Mat3::from_cols(x, y, -z)).normalize()
    }
}

impl Rider {
    /// `Boarder_Wipeout` 0x11d830's motion change: the crash's point and normal into the body (+0x20, +0x40 of the
    /// body, from the contact's +0 and +0x20), then `WipeoutMotion_Enter` 0x10c198 on the rider as it is.
    fn wipe_enter(&mut self, world: &CollisionWorld) -> Wipe {
        let k = self.stats.scale;
        let o = match self.att {
            Some((a, yaw)) if yaw == self.yaw => a,
            _ => {
                let up = to_game(self.board_up(), 1.0, 0.0);
                let h = to_game(heading(self.yaw), 1.0, 0.0);
                let fwd = super::ground_tick::unit(crate::probe::sub(h, scale(up, dot(h, up))));
                let mut o = Orient { q: quat_of(cross(fwd, up), fwd, up), ..Orient::default() };
                orient::update_orientation(&mut o);
                o
            }
        };
        let pos = to_game(self.pos, 100.0, 1.0);
        let ground = if self.grounded { to_game(self.normal, 1.0, 0.0) } else { [0.0, 0.0, 1.0, 0.0] };
        let (course, course8) = self.course_pts.map_or((pos, pos), |(a, b)| (to_game(a, 100.0, 1.0), to_game(b, 100.0, 1.0)));
        let mut rag = RagRider {
            id: 0,
            pos,
            vel: to_game(self.vel, 100.0, 0.0),
            q: o.q,
            up: o.up,
            height: 0.0,
            surface: self.surface as u32,
            ground,
            fall: crate::probe::fall_line(ground, o.up),
            ground_at: pos,
            ground_kind: if self.grounded { 0 } else { 3 },
            crash_n: [0.0; 4],
            side: o.fwd,
            course,
            body: bind_body_at(pos, o.fwd, o.up, k),
            // STANDIN: the hip bone (rider+0x44d8's) at the bind pose's root, 91.7 cm up (row G5)
            hip: [0.0, 0.0, 91.7 * k, 0.0],
        };
        let mut wr = WipeRider {
            time_scale: 1.0,
            fwd: o.fwd,
            side: o.side,
            x294: self.band.0 * 100.0,
            course8,
            player: if self.ai { 0 } else { -1 },
            x420: self.stats.kind as u32,
            x464: self.data.goofy,
            // STANDIN: +0x2e4 (and +0x2f0) from 0 at each wipe-out, for the riding ticks' own (the riding walls'
            // `Boarder_AccumulateImpact` and the decay outside the wipe-out are not ported, row G6e)
            stuck: 0.0,
            ..Default::default()
        };
        let mut motion = Wipeout::default();
        let (at, n) = self.crash_at.take().unwrap_or((self.pos, self.normal));
        (motion.rag.crash_at, motion.rag.crash_n) = (to_game(at, 100.0, 1.0), to_game(n, 1.0, 0.0));
        let mut s = State { motion: 5, state: 0x13, clip: CLIP_CRUNCH, fwd: o.fwd, side: o.side, yaw: o.yaw, probe_side: o.side, ..Default::default() };
        let who = self.who();
        let mut glue = Glue { world, last: &mut self.last_hit, s: &mut s, k, lean: self.lean[0], who };
        motion.enter(&mut rag, &mut wr, &mut glue);
        self.switch = false;
        self.clear_air();
        self.air_time = 0.0;
        let mut w = Wipe { q: Quat::IDENTITY, t: 0.0, clip: s.clip, clip_t: 0.0, motion, rag, wr, s, acc: 0.0 };
        w.q = w.draw_q();
        w
    }

    /// Motion 5 / 6 (`WipeoutMotion_Update` 0x10c2f0, `GetUpMotion_Update` 0x1102c8) a game tick at a time.
    pub(super) fn wipe_step(&mut self, world: &CollisionWorld, dt: f32) {
        let mut w = match self.wipe.take() {
            Some(w) => w,
            None => self.wipe_enter(world),
        };
        self.down_t += dt;
        w.t += dt;
        // STANDIN: clips that end chain on (end actions 5 and 6: impacts → crunch cycle, crunch-to-stretch →
        // falling), for the animation controller (row G5)
        if w.s.motion == 5 && w.s.clip_t >= (clip_frames(w.s.clip, self.stats.kind) - 1.0) / 30.0 {
            match w.s.clip {
                0x2e1..=0x2ed => (w.s.clip, w.s.clip_t) = (CLIP_CRUNCH, 0.0),
                CLIP_CRUNCH2STRETCH => (w.s.clip, w.s.clip_t) = (CLIP_FALLING, 0.0),
                _ => {}
            }
        }
        // states 0x13 / 0x14 (`GetUpState_Update` 0x108198): out of bounds (surface 0) is a reset at once
        if self.surface == 0 {
            let at = self.safe;
            self.respawn(at);
            return;
        }
        if let Some((near, ahead)) = self.course_pts {
            (w.rag.course, w.wr.course8) = (to_game(near, 100.0, 1.0), to_game(ahead, 100.0, 1.0));
        }
        let dbg = std::env::var("TRICKY_WIPEDBG").is_ok();
        w.acc += dt;
        let mut out = Out::Down;
        while w.acc >= TICK * 0.999 && matches!(out, Out::Down) {
            w.acc -= TICK;
            let was = w.s.clip;
            let who = self.who();
            out = w.tick(world, &mut self.last_hit, self.stats.scale, self.lean[0], self.stats.kind, who);
            for c in std::mem::take(&mut w.s.calls) {
                self.wipe_call(c);
            }
            if dbg && (w.s.clip != was || !matches!(out, Out::Down)) {
                println!("    wipe t {:.2} motion {} clip {} air {:.2} ground {:.2} speed {:.1} kind {} surf {} pos {:.1?}", w.t, w.s.motion, clip_name(w.s.clip), w.motion.rag.t_air, w.motion.t_ground,
                    to_port(w.rag.vel, 0.01).length(), w.rag.ground_kind, w.rag.surface, to_port(w.rag.pos, 0.01));
            }
        }
        self.pos = to_port(w.rag.pos, 0.01);
        self.vel = to_port(w.rag.vel, 0.01);
        self.normal = to_port(w.rag.ground, 1.0);
        self.surface = w.rag.surface.min(19) as u8;
        self.grounded = w.rag.ground_kind != 3;
        let f = to_port(w.wr.fwd, 1.0);
        if Vec3::new(f.x, 0.0, f.z).length() > 0.2 {
            self.yaw = f32::atan2(-f.x, -f.z);
        }
        (w.q, w.clip, w.clip_t) = (w.draw_q(), w.s.clip, w.s.clip_t);
        self.getup = if w.s.motion == 6 { w.s.clip_t } else { -1.0 };
        match out {
            Out::Down => {}
            Out::Air(clip) => return self.end_wipe(clip, false),
            Out::Ride(clip) => return self.end_wipe(clip, true),
            Out::Reset => {
                let at = self.safe;
                self.respawn(at);
                return;
            }
        }
        self.wipe = Some(w);
        // out of the world, or into one of the course's out-of-bounds boxes
        if self.pos.y < world.min_y - 30.0 || world.in_reset(self.pos + Vec3::Y * 0.8, BODY_RADIUS) {
            let at = self.safe;
            self.respawn(at);
        }
    }

    fn who(&self) -> Who {
        Who { id: rider_id(self), human: !self.ai, speed_stat: self.speed_stat() }
    }

    /// What the wipe-out's walls asked of the rider.
    fn wipe_call(&mut self, c: Call) {
        match c {
            Call::Crash => {
                // `Boarder_Wipeout` 0x11d830: with a multiplier, a running uber window and not "tricky", the meter
                // cut to two thirds (0.66599226) and the window closed; STANDIN: no `Sfx_OnWipeout` (rows G10)
                if 0.0 < crate::score::get_multiplier(self.sc.units) && 0.0 < self.uber_timer && !self.tricky() {
                    self.boost = self.boost.min(0.665_992_26);
                    self.uber_timer = 0.0;
                }
                let (m, calls) = self.sc.crash(false);
                self.show(&calls);
                self.add_meter(m);
            }
            Call::Meter(m) => self.add_meter(m),
            Call::Spray(p, v, n) => self.crash_spray = Some((to_port(p, 0.01), to_port(v, 0.01), to_port(n, 1.0))),
        }
    }

    /// Back to riding (on the snow) or to an ordinary jump, with the clip that brings the body back to its base
    /// pose (0 for none).
    fn end_wipe(&mut self, clip: u16, on_snow: bool) {
        self.crashed = 0.0;
        self.getup = -1.0;
        self.wipe = None;
        self.grounded = on_snow;
        self.spin = 0.0;
        self.flip = 0.0;
        self.clear_air();
        self.air_time = 0.0;
        self.charge = 0.0;
        if on_snow {
            let n = self.normal;
            self.vel -= n * self.vel.dot(n).min(0.0);
        } else {
            self.normal = Vec3::Y;
        }
        self.cap = SPEED_CAP.max(self.vel.length());
        self.recover = if clip != 0 { Some((clip, 0.0)) } else { None };
    }
}

// the original's clip ids for the wipe-out (game clip id → name via `AnimClip_ResolveAnmIndex`)
pub const CLIP_BUTTSLIDE: u16 = 0x2dc;
pub const CLIP_FACESLIDE: u16 = 0x2dd;
pub const CLIP_FALLING: u16 = 0x2de;
pub const CLIP_CRUNCH: u16 = 0x2df;
pub const CLIP_CRUNCH2STRETCH: u16 = 0x2e0;
pub const CLIP_FALLRECOVER: u16 = 0x224;
const IMPACTS: [&str; 6] = ["BUTT", "FACE", "FEET", "HEAD", "LEFT", "RIGHT"];

/// The name of a wipe-out clip ("bx" names are looked up in the board type's own set first).
pub fn clip_name(id: u16) -> String {
    match id {
        CLIP_BUTTSLIDE => "cmCC_BUTTSLIDE".into(),
        CLIP_FACESLIDE => "cmCC_FACESLIDE".into(),
        CLIP_FALLING => "cmF_FALLINGCYCLE".into(),
        CLIP_CRUNCH => "cmCC_CRUNCHCYCLE".into(),
        CLIP_CRUNCH2STRETCH => "cmCT_CRUNCH2STRETCH".into(),
        0x2e1..=0x2e6 => format!("cmCI_{}", IMPACTS[(id - 0x2e1) as usize]),
        0x2e8..=0x2ed => format!("cmI_{}", IMPACTS[(id - 0x2e8) as usize]),
        0x24a => "bxCT_ROLLBWD2BASE".into(),
        0x24b => "bxGU_FROMBUTTFWD".into(),
        0x24c => "bxGU_FROMBUTTRIGHT".into(),
        0x24d => "bxGU_FROMFACEBWD".into(),
        0x24e => "bxGU_FROMFACEFWD".into(),
        0x24f => "bxGU_FROMFACERIGHT".into(),
        0x250 => "bxCT_CRUNCH2BASE".into(),
        0x251 => "bxCT_STRETCH2BASE".into(),
        0x252 => "bxGU_FROMBUTTRIGHT2FAKIE".into(),
        0x253 => "bxGU_FROMFACERIGHT2FAKIE".into(),
        CLIP_FALLRECOVER => "bxA_FALLRECOVER".into(),
        _ => String::new(),
    }
}
/// Frames in a clip (30 a second) for a board type (0 bx, 1 fr, 2 ex), from the exported sets.
pub fn clip_frames(id: u16, kind: u8) -> f32 {
    let k = kind.min(2) as usize;
    let f: [f32; 3] = match id {
        CLIP_BUTTSLIDE => [22.0; 3], CLIP_FACESLIDE => [47.0; 3], CLIP_FALLING => [20.0; 3], CLIP_CRUNCH => [23.0; 3],
        CLIP_CRUNCH2STRETCH => [14.0; 3],
        0x2e1 | 0x2e3 => [19.0; 3], 0x2e2 => [16.0; 3], 0x2e4..=0x2e6 => [21.0; 3],
        0x2ea => [34.0; 3], 0x2e8..=0x2ed => [30.0; 3],
        0x24a => [51.0, 57.0, 56.0], 0x24b => [35.0, 35.0, 30.0], 0x24c => [35.0, 35.0, 33.0],
        0x24d => [41.0, 31.0, 33.0], 0x24e => [30.0, 35.0, 33.0], 0x24f => [35.0, 35.0, 33.0],
        0x250 | 0x251 => [21.0, 21.0, 10.0], 0x252 | 0x253 => [33.0; 3],
        CLIP_FALLRECOVER => [18.0; 3],
        _ => [30.0; 3],
    };
    f[k]
}
/// Clip category (descriptor table 0x325fa8): 17 the curled-up ("crunch") clips, 18 the
/// stretched-out ones.
fn category(id: u16) -> u8 { if matches!(id, CLIP_CRUNCH | CLIP_CRUNCH2STRETCH | 0x2e1..=0x2e6) { 17 } else { 18 } }

/// The nearest point of a polyline course to `pos` (searched near `idx`) and the point 8 m
/// further along it (the original's 0x350 / 0x360, `fn 0x119d30`).
pub fn course_points(line: &[Vec3], idx: usize, pos: Vec3) -> Option<(Vec3, Vec3)> {
    if line.len() < 2 { return None; }
    let lo = idx.saturating_sub(10).min(line.len() - 2);
    let hi = (idx + 10).min(line.len() - 2);
    let mut best = (f32::MAX, lo, 0.0f32, line[lo]);
    for i in lo..=hi {
        let (a, b) = (line[i], line[i + 1]);
        let ab = b - a;
        let t = if ab.length_squared() > 1e-8 { ((pos - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + ab * t;
        let d = (q - pos).length_squared();
        if d < best.0 { best = (d, i, t, q); }
    }
    let (_, mut i, t, near) = best;
    let mut left = 8.0f32;
    let mut at = near;
    let mut seg_rest = (line[i + 1] - near).length();
    let _ = t;
    loop {
        if seg_rest >= left { at += (line[i + 1] - at).normalize_or_zero() * left; break; }
        left -= seg_rest;
        at = line[i + 1];
        i += 1;
        if i + 1 >= line.len() { break; }
        seg_rest = (line[i + 1] - at).length();
    }
    Some((near, at))
}

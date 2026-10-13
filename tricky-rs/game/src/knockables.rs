//! The course's knockable objects as the game runs them (board row E8c3): a rider's contact with a touch object
//! (`Boarder_InstanceCollisions` 0x125088 → `Instance_OnPlayerContact` 0x13bd40) starts the object's event-1 script
//! (`script`), whose op 0 sub-type 0 (`TriggerScript_Op0_SpawnController` 0x13c5d0) makes the object a `cRollerNode`
//! (`roller`): it leaves the casts (flags `& ~0x22 | 0x45`) and moves as a rigid body until it sleeps. The rider then
//! kicks it through the rollers' loop at the end of `Boarder_InstanceCollisions` (`roller::rider_kick`). In the
//! game's units (cm, z up); objects are the course's instance indices.

use crate::course_ground::CourseGround;
use crate::controllers::{CtrlCall, CtrlEnv};
use crate::roller::{RollerBody, RollerEnv};
use crate::script::{self, Context, ScriptCall, ScriptEnv, ScriptRef, Thread};
use crate::sphtree::WorldSphTree;
use crate::walls::Contact;
use tricky_data::ssf::Op;

type V4 = [f32; 4];

/// `cRollerNode` on an object.
#[derive(Clone, Debug)]
pub struct Roller {
    pub object: u32,
    pub body: RollerBody,
}

/// The moving knockables and the script threads that are waiting.
#[derive(Default)]
pub struct Knockables {
    /// newest first (`BxList_Add` 0x17d4e8 links a new node at the head of the world's list)
    pub rollers: Vec<Roller>,
    threads: Vec<(Thread, Rider)>,
    /// `Sfx_InstanceCollision` 0x216c50 calls since the app last took them: the object and the rider's speed (cm/s)
    pub knocks: Vec<(u32, f32)>,
    /// the 60 Hz frames' clock (the port's ticks may be shorter)
    clock: f32,
}

/// A thread's rider as the ops see it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rider {
    /// the rider's handle (0 none)
    pub id: u32,
    /// +0x41c nonzero: the human
    pub human: bool,
    /// +0x150. STANDIN: as it was when the thread started, for the rider's own (row E9b2)
    pub vel: V4,
}

/// A contact (`Boarder_InstanceCollisions`' record, the thread's first context +0x50): +0 the point, +0x10 the
/// direction of travel, +0x20 the normal, +0x30 the speed into it.
pub fn contact_context(c: &Contact) -> Context {
    let mut k = [0u32; 16];
    for i in 0..4 {
        k[i] = c.pos[i].to_bits();
        k[4 + i] = c.dir[i].to_bits();
        k[8 + i] = c.normal[i].to_bits();
    }
    k[12] = c.speed_in.to_bits();
    k
}

/// The cast's record (the thread's second context +0x90): +0 the point, +0x10 the normal, +0x30 the sphere tree hit.
/// STANDIN: the tree (the cast's (group, index)) as the group and its number from 1 in words 12 and 13, for its
/// address (0, 0 for none).
pub fn record_context(pos: V4, normal: V4, tree: Option<(usize, usize)>) -> Context {
    let mut k = [0u32; 16];
    for i in 0..4 {
        k[i] = pos[i].to_bits();
        k[4 + i] = normal[i].to_bits();
    }
    let (g, n) = tree.map_or((0, 0), |(g, n)| (g, n + 1));
    (k[12], k[13]) = (g as u32, n as u32);
    k
}

fn word4(k: &Context, at: usize) -> V4 {
    [0, 1, 2, 3].map(|i| f32::from_bits(k[at + i]))
}

/// What the rollers ask of the world, on the course.
struct RollEnv<'a> {
    course: &'a CourseGround,
    show_off: bool,
    skip: &'a dyn Fn(u32) -> bool,
    /// STANDIN: the object's flags word for the controller layer (a roller's update never reads or writes it)
    scratch: u32,
    starts: Vec<(i32, u32)>,
}

fn hit(t: f32, h: Option<crate::world::CastHit>) -> (f32, V4, V4) {
    h.map_or((t, [0.0; 4], [0.0; 4]), |h| (t, h.pos, h.normal))
}

impl RollerEnv for RollEnv<'_> {
    fn ray_cast(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        let (t, h) = self.course.ray_cast(&mut q.clone(), self.show_off, None, self.skip);
        hit(t, h)
    }
    fn query_instances(&mut self, q: &WorldSphTree) -> (f32, V4, V4) {
        let (t, h) = self.course.query_instances(&mut q.clone(), self.show_off, self.skip);
        hit(t, h)
    }
    /// STANDIN: nothing, for the race's broadphase (`Broadphase_MoveBox` 0x14fe68); the port pairs by the boxes.
    fn move_box(&mut self, _: V4, _: V4) {}
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.course.ssf.event_script(object as usize, event as usize)
    }
    fn start_script(&mut self, script: i32, object: u32) {
        self.starts.push((script, object));
    }
    /// Only a rider's hit calls it, and the rollers' own update never does.
    fn sfx(&mut self, _: u32, _: u32) {}
}

impl CtrlEnv for RollEnv<'_> {
    fn flags(&mut self, _: u32) -> &mut u32 {
        &mut self.scratch
    }
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.course.ssf.event_script(object as usize, event as usize)
    }
    fn rider_in_box(&self, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn call(&mut self, c: CtrlCall) {
        if let CtrlCall::Start(s, o) = c {
            self.starts.push((s, o));
        }
    }
}

/// What the script threads ask of the world.
struct ThreadEnv<'a> {
    course: &'a CourseGround,
    rollers: &'a mut Vec<Roller>,
    waiting: Vec<(Thread, Rider)>,
    rider: Rider,
    /// the thread running: its script, object (index + 1) and contexts
    cur: Option<(ScriptRef, u32, Option<Context>, Option<Context>)>,
}

impl ThreadEnv<'_> {
    /// `cScriptNode` 0x13b8c0 / 0x13bc78: a thread runs at once; one that waits stays in the world's list.
    fn start(&mut self, mut t: Thread, rider: Rider) {
        let (saved, saved_rider) = (self.cur, self.rider);
        self.cur = Some((t.script, t.object, t.ctx_a, t.ctx_b));
        self.rider = rider;
        if !script::run(&mut t, self) {
            self.waiting.push((t, rider));
        }
        (self.cur, self.rider) = (saved, saved_rider);
    }

    /// `TriggerScript_Op0_SpawnController` 0x13c5d0 for sub-type 0 on the thread's object: an asleep roller there is
    /// woken (`cRollerNode` vf 0x64 0x149e28 asleep, vf 0x74 0x149e10 awake); an awake one is left; else a new
    /// `cRollerNode` (`cRollerNode::cRollerNode` 0x13d6c0): the sphere tree hit (the second context's +0x30) when
    /// the op gives no velocity, else the object's group's (`World_GetInstanceSphTree` 0x260ca0), its mass and
    /// +0xd0 from the op (+0xc, +0x10), placed where the object is, the object's flags `& ~0x22 | 0x45`, pushed
    /// from the contact or with the op's velocity (+0x18..+0x20).
    /// STANDIN: the other sub-types are left to the port's own scripts (`logic.rs`, row E9b2).
    fn spawn(&mut self, op: &Op) {
        let a = |k: usize| op.args.get(k).copied().unwrap_or(0);
        let Some((_, object, ctx_a, ctx_b)) = self.cur else { return };
        if a(0) != 0 || object == 0 {
            return;
        }
        let o = object - 1;
        if let Some(r) = self.rollers.iter_mut().find(|r| r.object == o) {
            if r.body.asleep != 0 {
                r.body.asleep = 0;
            }
            return;
        }
        let f = |k: usize| f32::from_bits(a(k));
        let (mass, xd0, vel) = (f(1), f(2), [f(4), f(5), f(6), 0.0]);
        let Some((rows, group)) = self.course.with_world(|w| w.objects.get(o as usize).map(|x| (x.rows, x.group as usize))) else { return };
        let (g, n) = match ctx_b {
            Some(k) if vel == [0.0; 4] => (k[12] as usize, k[13] as usize),
            _ => (group, 1),
        };
        let Some(raw) = self.course.ssf.sphere_trees.get(g).and_then(|t| t.get(n.wrapping_sub(1))) else { return };
        let Some(tree) = self.course.with_world(|w| w.trees.get(g).and_then(|t| t.get(n - 1)).cloned()) else { return };
        let (pos, q) = crate::orient::mat4_to_pos_quat(rows);
        let mut body = RollerBody {
            rb: Default::default(),
            energy: 0.0,
            mass: 0.0,
            xd0: 0.0,
            xd4: 0.0,
            e: 0.0,
            mu: 0.0,
            bounce_speed: 0.0,
            awake: 0.0,
            knock: 0.0,
            xf0: [0.0; 4],
            body_inertia: [0.0; 9],
            com: [0.0; 4],
            asleep: 0,
            tree,
            lo: [0.0; 4],
            hi: [0.0; 4],
        };
        let mc = raw.mass_centre;
        body.init(mass, xd0, [mc[0], mc[1], mc[2], 0.0], raw.inv_inertia, pos, q);
        let fl = self.course.object_flags(o as usize);
        self.course.set_flags(o as usize, fl & 0xffff_ffdd | 0x45);
        if vel == [0.0; 4] {
            let k = ctx_a.unwrap_or([0; 16]);
            body.kick_contact(word4(&k, 4), word4(&k, 8), f32::from_bits(k[12]));
        } else {
            body.kick_velocity(vel);
        }
        self.rollers.insert(0, Roller { object: o, body });
    }
}

impl ScriptEnv for ThreadEnv<'_> {
    fn ops(&self, s: ScriptRef) -> &[Op] {
        let ssf = &self.course.ssf;
        match s {
            ScriptRef::Effect(e) => ssf.effects.get(e as usize).map_or(&[], |v| &v[..]),
            ScriptRef::Function(f) => ssf.functions.get(f as usize).map_or(&[], |v| &v.ops[..]),
        }
    }
    fn rand01(&mut self) -> f32 {
        crate::rand::streams().game_range(0.0, 1.0)
    }
    fn rider_velocity(&self, _: u32) -> [f32; 4] {
        self.rider.vel
    }
    fn rider_x41c(&self, _: u32) -> i32 {
        self.rider.human as i32
    }
    /// A roller's node, as a handle (0 none). STANDIN: the course's other controllers are not kept (row E7b2).
    fn controller(&self, object: u32) -> u32 {
        match object.checked_sub(1) {
            Some(o) if self.rollers.iter().any(|r| r.object == o) => 0x1000_0000 | o,
            _ => 0,
        }
    }
    fn instance(&self, index: u32) -> u32 {
        if (index as usize) < self.course.ssf.object_physics.len() { index + 1 } else { 0 }
    }
    fn camera(&self, _: u32) -> u32 {
        0
    }
    /// STANDIN: only the rollers' spawn and new threads, for the rest of the ops' calls (row E9b2: the port's own
    /// `logic.rs` does pickups, boosts, sounds and the rest).
    fn call(&mut self, c: ScriptCall) {
        match c {
            ScriptCall::SpawnController(at) => {
                let Some((s, ..)) = self.cur else { return };
                let mut off = 0;
                let op = self.ops(s).iter().find(|op| {
                    let here = off == at;
                    off += op.size;
                    here
                });
                if let Some(op) = op.cloned() {
                    self.spawn(&op);
                }
            }
            ScriptCall::StartEffect { effect, object, rider, ctx_a, ctx_b } => {
                let r = Rider { id: rider, ..self.rider };
                self.start(Thread::new(ScriptRef::Effect(effect), object, rider, ctx_a, ctx_b), r);
            }
            ScriptCall::StartFunction { function, rider, ctx_a, ctx_b } => {
                let r = Rider { id: rider, ..self.rider };
                self.start(Thread::new(ScriptRef::Function(function), 0, rider, ctx_a, ctx_b), r);
            }
            _ => {}
        }
    }
}

impl Knockables {
    /// `Instance_OnPlayerContact` 0x13bd40(object, rider, contact, record): with no controller, or one that says yes
    /// to vf 0x64 (a roller: asleep), the object's event-1 script runs as a new thread (`cScriptNode`(3, …) with the
    /// rider and the two records); else the controller's vf 0x94 (nothing, for a roller).
    pub fn contact(&mut self, course: &CourseGround, object: u32, rider: Rider, c: &Contact, record: Context) {
        if self.rollers.iter().any(|r| r.object == object && r.body.asleep == 0) {
            return;
        }
        let s = course.ssf.event_script(object as usize, 1);
        if s == -1 {
            return;
        }
        let t = Thread::new(ScriptRef::Effect(s as u32), object + 1, rider.id, Some(contact_context(c)), Some(record));
        self.run_new(course, vec![(t, rider)]);
    }

    /// Scripts a roller started (`Roller_OnHit`, `Finish`): `cScriptNode`(3, script, object), no rider.
    pub fn start_scripts(&mut self, course: &CourseGround, starts: &[(i32, u32)]) {
        let t = starts.iter().filter(|(s, _)| *s >= 0).map(|&(s, o)| (Thread::new(ScriptRef::Effect(s as u32), o + 1, 0, None, None), Rider::default()));
        self.run_new(course, t.collect());
    }

    fn run_new(&mut self, course: &CourseGround, list: Vec<(Thread, Rider)>) {
        let mut env = ThreadEnv { course, rollers: &mut self.rollers, waiting: Vec::new(), rider: Rider::default(), cur: None };
        for (t, r) in list {
            env.start(t, r);
        }
        let w = env.waiting;
        self.threads.extend(w);
    }

    /// The world's frames for `dt` seconds of the port's (each 1/60 s): every roller's `cRollerNode::Update`
    /// 0x13e8a8, newest first, then the rollers knocking each other (`Roller_CheckCollision` 0x13f5c8), then the
    /// waiting threads (`TriggerScript_Run` 0x13be78). `skip` as [`CourseGround::ray_cast`]'s.
    /// STANDIN: the pairs are every two rollers whose boxes meet, for the race's broadphase (its pass and order are
    /// not found yet); the rollers and threads go in two passes, for the world's one list.
    pub fn tick(&mut self, course: &CourseGround, show_off: bool, skip: &dyn Fn(u32) -> bool, dt: f32) {
        self.clock += dt;
        while self.clock >= crate::roller::DT * 0.999 {
            self.clock -= crate::roller::DT;
            let mut env = RollEnv { course, show_off, skip, scratch: 0, starts: Vec::new() };
            for r in self.rollers.iter_mut() {
                r.body.update(r.object, &mut env);
            }
            for i in 0..self.rollers.len() {
                for j in i + 1..self.rollers.len() {
                    let (a, b) = self.rollers.split_at_mut(j);
                    let (a, b) = (&mut a[i], &mut b[0]);
                    if overlap(a.body.lo, a.body.hi, b.body.lo, b.body.hi) {
                        crate::roller::pair(&mut a.body, &mut b.body, b.object, &mut env);
                    }
                }
            }
            let starts = std::mem::take(&mut env.starts);
            self.start_scripts(course, &starts);
            let threads = std::mem::take(&mut self.threads);
            let mut env = ThreadEnv { course, rollers: &mut self.rollers, waiting: Vec::new(), rider: Rider::default(), cur: None };
            for (mut t, r) in threads {
                env.cur = Some((t.script, t.object, t.ctx_a, t.ctx_b));
                env.rider = r;
                if !script::run(&mut t, &mut env) {
                    env.waiting.push((t, r));
                }
            }
            let w = env.waiting;
            self.threads.extend(w);
        }
    }

    /// Where a moving object is (game units): its position and orientation (w, x, y, z), `RollerNode_PlaceFromBody`.
    pub fn frame(&self, object: u32) -> Option<(V4, V4)> {
        self.rollers.iter().find(|r| r.object == object).map(|r| r.body.frame())
    }
}

/// Two boxes (min, max) meet.
pub fn overlap(alo: V4, ahi: V4, blo: V4, bhi: V4) -> bool {
    (0..3).all(|k| alo[k] <= bhi[k] && blo[k] <= ahi[k])
}

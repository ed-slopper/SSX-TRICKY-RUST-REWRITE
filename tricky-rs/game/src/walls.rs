//! Walls and objects (board row E8b): `Boarder_WallCollide` 0x126250 (the rider's collision spheres against the
//! world's solid surfaces, pushed out and bounced), `Boarder_WallImpactCheck` 0x126540 (whether the knock is a
//! wipe-out or a stumble), `Boarder_InstanceCollisions` 0x125088 (the touch objects: contact scripts and, for
//! bouncy ones, `Boarder_InstanceBounce` 0x125a00). The casts (`World_RayCast` / `World_QueryInstances` with a
//! `cWorldBoarder` shape, row E7a3) and the callees are the caller's ([`WallEnv`]); the rollers' loop at the end of
//! `Boarder_InstanceCollisions` is `roller::rider_kick` (row E8c2). Checked in the function runner (`tests/walls.rs`).

type V4 = [f32; 4];

/// The rider's fields these read and write: +0x140 position, +0x150 velocity, +0x2a0 the ground normal, +0x300
/// (grows by wall knocks in mode 1), +0x424 the motion mode, +0x428 the control state, +0x4ac0 the body's up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WallRider {
    pub pos: V4,
    pub vel: V4,
    pub ground: V4,
    pub knocks: f32,
    pub mode: u32,
    pub state: u32,
    pub up: V4,
}

/// What a cast with the rider's shape found (the result record): +0x10 the point, +0x20 the normal, +0x38 the
/// object (0 for none).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CastRecord {
    pub pos: V4,
    pub normal: V4,
    pub object: u32,
}

/// The record handed on (to the impact check's wipe-out, contact scripts and bounces): +0 the hit point, +0x10 the
/// velocity's direction, +0x20 the normal, +0x30 the speed into it; the contact record also has the direction at
/// +0x40.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub pos: V4,
    pub dir: V4,
    pub normal: V4,
    pub speed_in: f32,
}

/// What a touched object brings: its flags (+0xe8; 0x80 riders bounce off), its physics record's mass and bounce
/// (+0xec: +0, +4), and whether it has a controller (+0xe4) and that controller's +0x54 answer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Touched {
    pub id: u32,
    pub flags: u32,
    pub mass: f32,
    pub bounce: f32,
    pub controller: Option<bool>,
}

/// The calls these make, in the order they make them.
pub trait WallEnv {
    /// `World_RayCast` with `cWorldBoarder`(rider+0x470, 1) given the ground normal: t and the record
    fn cast_walls(&mut self, r: &WallRider) -> (f32, CastRecord);
    /// `World_QueryInstances` with `cWorldBoarder`(rider+0x470, 1): t, the record and the object
    fn query_objects(&mut self, r: &WallRider) -> (f32, CastRecord, Touched);
    /// `Boarder_AccumulateImpact` 0x125c48(rider, n)
    fn accumulate_impact(&mut self, r: &mut WallRider, n: V4);
    /// `Boarder_Translate` 0x1267a0(rider, by)
    fn translate(&mut self, r: &mut WallRider, by: V4);
    /// `Boarder_TurnFromWall` 0x1257a0(rider, n)
    fn turn_from_wall(&mut self, r: &mut WallRider, n: V4);
    /// `Boarder_Wipeout` 0x11d830(rider, the caller's record)
    fn wipeout(&mut self, r: &mut WallRider, record: &Contact);
    /// `Score_Crash` 0x156de0(rider+0x5820, 0), its answer then to `Boarder_AddMeter` 0x11b018
    fn score_crash(&mut self, r: &mut WallRider) -> f32;
    fn add_meter(&mut self, r: &mut WallRider, amount: f32);
    /// `Boarder_Stumble` 0x124dd0(rider, direction, flag)
    fn stumble(&mut self, r: &mut WallRider, dir: V4, flag: u32);
    /// `Boarder_GetSpeedStat` 0x11d7b0
    fn speed_stat(&mut self, r: &WallRider) -> f32;
    /// the rider's virtual +0x6c (a knock's strength, 0.0012 · the speed in)
    fn knock(&mut self, r: &mut WallRider, amount: f32);
    /// `Instance_OnPlayerContact` 0x13bd40(object, rider, contact, record)
    fn contact(&mut self, object: u32, r: &mut WallRider, c: &Contact, rec: &CastRecord);
    /// `Sfx_InstanceCollision` 0x216c50 (with the object) and `Sfx_OnWallHit` 0x2162f0
    fn sfx_object(&mut self, r: &WallRider, object: u32);
    fn sfx_wall(&mut self, r: &WallRider);
}

/// vmul, (x + y) + 1·z + 1·w.
fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn scale(v: V4, k: f32) -> V4 {
    v.map(|x| x * k)
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// vrsqrt (1/√0 is the largest float) and vmulq.
fn unit(v: V4) -> V4 {
    let s = dot(v, v);
    scale(v, if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() })
}

/// n with its part along `g` taken out, made unit length (when n faces away from the ground).
fn off_ground(n: V4, g: V4, d: f32) -> V4 {
    unit(sub(n, scale(g, d)))
}

/// `Boarder_WallImpactCheck` 0x126540(speed, rider, record, n): with d = up · n, in mode 1 a knock below
/// 833.3333 cm/s adds (2.5 − d)·max(n.z, 0.2) to +0x300; then (not in mode 5) a speed over 833.3333·d + 1944.4443
/// is a wipe-out (and `Score_Crash` to `Boarder_AddMeter`), else one over 833.3333·d (not when d > 0 in mode 1) a
/// stumble along n less its part along up.
pub fn impact_check(r: &mut WallRider, speed: f32, record: &Contact, n: V4, env: &mut impl WallEnv) {
    let d = dot(r.up, n);
    let lim = d * 833.3333;
    if r.mode == 1 && dot(r.vel, r.vel).sqrt() < 833.3333 {
        let k = if n[2] < 0.2 { 0.2 } else { n[2] };
        r.knocks += (2.5 - d) * k;
    }
    if r.mode == 5 {
        return;
    }
    if lim + 1944.4443 < speed {
        env.wipeout(r, record);
        let s = env.score_crash(r);
        env.add_meter(r, s);
    } else if !(0.0 < d && r.mode == 1) && lim < speed {
        let dir = unit(sub(n, scale(r.up, dot(n, r.up))));
        env.stumble(r, dir, 0);
    }
}

/// `Boarder_WallCollide` 0x126250: the rider's shape cast against the world; a hit beyond 0 pushes the rider out
/// along the normal (turned off the ground when it faces into it) by 1.1·t, and when moving into it adds
/// (v_in + max(v_in/2, 55.555553))·n to the velocity, turns from the wall in mode 2, and checks the impact at
/// max(v_in/2, 55.555553). True when it bounced.
pub fn wall_collide(r: &mut WallRider, env: &mut impl WallEnv) -> bool {
    let (t, mut rec) = env.cast_walls(r);
    if !(0.0 < t) {
        return false;
    }
    let d = dot(r.ground, rec.normal);
    if d < 0.0 {
        rec.normal = off_ground(rec.normal, r.ground, d);
    }
    let n = rec.normal;
    env.accumulate_impact(r, n);
    let push = scale(scale(n, 1.1), t);
    env.translate(r, push);
    r.pos = add(r.pos, push);
    let vin = dot(scale(n, -1.0), r.vel);
    if vin < 0.0 {
        return false;
    }
    let s = if vin * 0.5 < 55.555553 { 55.555553 } else { vin * 0.5 };
    r.vel = add(r.vel, scale(n, vin + s));
    if r.mode == 2 {
        env.turn_from_wall(r, n);
    }
    let record = Contact { pos: rec.pos, dir: unit(r.vel), normal: n, speed_in: vin };
    impact_check(r, s, &record, n, env);
    if rec.object != 0 {
        env.sfx_object(r, rec.object);
    }
    env.sfx_wall(r);
    true
}

/// `Boarder_InstanceBounce` 0x125a00(t, rider, object, contact, record) (not in control state 0x16): a massless
/// object only makes the rider stumble; else, with v_in the speed into the normal, a fixed object (no controller,
/// or one whose +0x54 says so) pushes the rider out by n·(1.1·t) and bounces it at max(b·v_in + v_in,
/// v_in + 55.555553) (turning from it in mode 2); a moving one shares the knock by the masses (k = 1 / (speed stat /
/// mass + 1), a = v_in·k, s = √max(a² − (1 − b)·k, 0) − a). Then the knock virtual, v += s·n, the impact check.
pub fn instance_bounce(r: &mut WallRider, t: f32, o: &Touched, c: &Contact, rec: &CastRecord, env: &mut impl WallEnv) {
    if r.state == 0x16 {
        return;
    }
    let (mass, bounce) = (o.mass, o.bounce);
    let n = rec.normal;
    if mass == 0.0 {
        env.stumble(r, n, 1);
        return;
    }
    env.accumulate_impact(r, n);
    let vin = dot(scale(n, -1.0), r.vel);
    let s = if o.controller.is_none_or(|fixed| fixed) {
        let push = scale(n, t * 1.1);
        r.pos = add(r.pos, push);
        env.translate(r, push);
        let mut s = bounce * vin + vin;
        if s < vin + 55.555553 {
            s = vin + 55.555553;
        }
        if r.mode == 2 {
            env.turn_from_wall(r, n);
        }
        s
    } else {
        let k = 1.0 / (env.speed_stat(r) / mass + 1.0);
        let a = vin * k;
        let mut b = a * a - (1.0 - bounce) * k;
        if b < 0.0 {
            b = 0.0;
        }
        b.sqrt() - a
    };
    env.knock(r, vin * 0.0012);
    r.vel = add(r.vel, scale(n, s));
    impact_check(r, s, c, n, env);
}

/// `Boarder_InstanceCollisions` 0x125088 up to its rollers' loop (`roller::rider_kick`, row E8c2): the rider's shape against the touch
/// objects; on a hit (t ≥ 0) the normal is turned off the ground in mode 2, the contact record made (point,
/// direction of travel, normal, speed into it), the object's contact scripts run, a bouncy one (flag 0x80)
/// bounces the rider, and its collision sound plays.
pub fn instance_collisions(r: &mut WallRider, env: &mut impl WallEnv) {
    let (t, mut rec, o) = env.query_objects(r);
    if !(0.0 <= t) {
        return;
    }
    if r.mode == 2 {
        let d = dot(rec.normal, r.ground);
        rec.normal = off_ground(rec.normal, r.ground, d);
    }
    let vin = dot(scale(rec.normal, -1.0), r.vel);
    let c = Contact { pos: rec.pos, dir: unit(r.vel), normal: rec.normal, speed_in: vin };
    env.contact(o.id, r, &c, &rec);
    if o.flags & 0x80 != 0 {
        instance_bounce(r, t, &o, &c, &rec, env);
    }
    env.sfx_object(r, o.id);
}

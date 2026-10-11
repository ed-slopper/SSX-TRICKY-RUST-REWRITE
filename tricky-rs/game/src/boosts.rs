//! The level scripts' boost controllers (board row E9f) as the game runs them: `cBoostNode` (type 7),
//! `cRandomBoostNode` (16), `cTubeEndBoostNode` (24) and `cZBoostNode` (18). (`cLapBoostNode`, 15, is row E9f2.)
//! Checked against the game's own objects in the function runner (`tests/boosts.rs`).
//!
//! A boost node keeps a box over its object (+0x70, in the broadphase); each frame it walks the objects overlapping
//! the box, and for each rider whose collision shape the object's mesh actually hits (`WorldNode_RayCast`) pushes
//! it: with `d` = target − velocity·direction (all four lanes), when `d` > 0 the velocity gains
//! direction · (d · rate · 1/60). The plain boost's direction is fixed; the random boost aims each rider at a random
//! point of a box once (until the rider leaves while the boost is idle); the tube-end boost aims at one of three
//! points with one of three targets, picked per rider by the lap boost (0x31d338); the Z boost works on height.

/// A rider as the boosts see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoostRider {
    /// +0x460: the rider's slot in the race
    pub index: u32,
    /// +0x140, +0x150
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    /// +0x424: the rider's motion (5: on a rail)
    pub x424: i32,
}

/// What a boost calls outside itself, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoostCall {
    /// the controller's Finish(1) (`cControllerNode::Finish` 0x13a980)
    Finish,
    /// `Boarder_PlaceAt` 0x11cae8(rider+0x1b0, 0, rider, its position, its velocity, 0) for the rider in slot n
    PlaceAt(u32),
}

pub trait BoostEnv {
    /// the riders overlapping the boost's box, in the broadphase's order: their slot, and whether the object's mesh
    /// hits them (`WorldNode_RayCast` > 0)
    fn touching(&self) -> Vec<(u32, bool)>;
    /// the race's rider count (gApp+0x730 → +0x1c → +0x88)
    fn riders(&self) -> u32;
    fn rider(&mut self, slot: u32) -> &mut BoostRider;
    /// `Rand_GameRange` 0x23d4f8(lo, hi)
    fn rand(&mut self, lo: f32, hi: f32) -> f32;
    /// the lap boost's choice for a rider (0x31d338 + 4·slot)
    fn tube_choice(&self, slot: u32) -> u32;
    fn call(&mut self, c: BoostCall);
}

/// "No direction yet" (1e10).
pub const UNSET: u32 = 0x5015_02f9;

/// What the kinds keep beyond the plain boost's fields.
#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    Plain,
    /// +0x110 the other corner of the box the random points are picked in; +0x120 per rider its direction;
    /// +0x180 per rider pushed this frame
    Random { corner: [f32; 4], dirs: [[f32; 4]; 6], pushed: [u8; 6] },
    /// +0x110 three directions (unnormalised), +0x1a0 three targets (×100), +0x140 per rider its direction,
    /// +0x1ac per rider pushed this frame
    TubeEnd { points: [[f32; 4]; 3], targets: [f32; 3], dirs: [[f32; 4]; 6], pushed: [u8; 6] },
    /// +0x5c the height to lift riders to, +0x60 how far below it still snaps; +0x64 per rider inside
    Z { height: f32, snap: f32, inside: [u8; 6] },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Boost {
    /// +0x40 the direction (w 0), +0x50 the target speed (×100), +0x54 the rate
    pub dir: [f32; 4],
    pub target: f32,
    pub rate: f32,
    /// +0x58 the re-arm frames, +0x5c frames left, +0x60 the mode (1: always on) — not used by the Z boost
    pub rearm: i32,
    pub left: i32,
    pub mode: i32,
    pub extra: Extra,
}

fn frames(v: f32) -> i32 {
    let p = v * 60.0;
    if p.is_nan() {
        i32::MAX
    } else {
        p as i32
    }
}

/// a·b over four lanes as VU0 sums it (x + y, then + z, then + w).
fn dot4(a: [f32; 4], b: [f32; 4]) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    let acc = m[0] + m[1];
    let acc = acc + 1.0 * m[2];
    acc + 1.0 * m[3]
}

/// v / |v| (`vrsqrt` then `vmulq`, all four lanes); VU0's 1/√0 is the largest float, so a zero vector stays 0.
fn normalise(v: [f32; 4]) -> [f32; 4] {
    let s = dot4(v, v);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    v.map(|x| x * q)
}

/// The push (`cBoostNode` vf27 0x140838 and its copies): `d` = target − v·dir; when positive v += dir·(d·rate/60).
fn push(vel: &mut [f32; 4], dir: [f32; 4], target: f32, rate: f32) {
    let d = target - dot4(*vel, dir);
    if 0.0 < d {
        let k = d * rate * f32::from_bits(0x3c88_8889);
        for i in 0..4 {
            vel[i] += dir[i] * k;
        }
    }
}

impl Boost {
    /// The constructors from op 0's words (`a[0]` the type; op+0xc p3 …).
    pub fn new(a: &[u32]) -> Option<Boost> {
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let f = |k: usize| f32::from_bits(w(k));
        let v3 = |k: usize| [f(k), f(k + 1), f(k + 2), 0.0];
        match w(0) {
            // `cBoostNode` 0x140498 (and the random and tube-end boosts built on it)
            7 | 0x10 | 0x18 => {
                let mode = w(1) as u16 as i16 as i32;
                let rearm = frames(f(2));
                let extra = match w(0) {
                    7 => Extra::Plain,
                    0x10 => Extra::Random { corner: v3(8), dirs: [[f32::from_bits(UNSET), f32::from_bits(UNSET), f32::from_bits(UNSET), 0.0]; 6], pushed: [0; 6] },
                    _ => Extra::TubeEnd {
                        points: [v3(8), v3(11), v3(14)],
                        targets: [f(17) * 100.0, f(18) * 100.0, f(19) * 100.0],
                        dirs: [[f32::from_bits(UNSET), f32::from_bits(UNSET), f32::from_bits(UNSET), 0.0]; 6],
                        pushed: [0; 6],
                    },
                };
                Some(Boost { dir: v3(5), target: f(4) * 100.0, rate: f(3), rearm, left: if mode == 1 { rearm } else { 0 }, mode, extra })
            }
            // `cZBoostNode` 0x141af0
            0x12 => Some(Boost { dir: v3(3), target: f(2) * 100.0, rate: f(1), rearm: 0, left: 0, mode: 0, extra: Extra::Z { height: f(6), snap: f(7), inside: [0; 6] } }),
            _ => None,
        }
    }

    /// OnMessage (`cTubeEndBoostNode::OnMessage` 0x1408d0 for the plain, random and tube-end boosts): kind 1 re-arms
    /// (+0x5c = +0x58); other kinds go to the base (flag 0x800, `controllers`). Returns whether it was taken here.
    pub fn on_message(&mut self, kind: u32) -> bool {
        if matches!(self.extra, Extra::Z { .. }) {
            return false;
        }
        if kind == 1 {
            self.left = self.rearm;
            return true;
        }
        false
    }

    /// The per-rider push (vtable +0xe4).
    fn boost_rider(&mut self, slot: u32, env: &mut impl BoostEnv) {
        let (dir, target, rate) = (self.dir, self.target, self.rate);
        match &mut self.extra {
            Extra::Plain => {
                let r = env.rider(slot);
                push(&mut r.vel, dir, target, rate);
            }
            // `cRandomBoostNode` vf27 0x141360
            Extra::Random { corner, dirs, pushed } => {
                let i = env.rider(slot).index as usize;
                if dirs[i][0] == f32::from_bits(UNSET) {
                    let span = [0, 1, 2, 3].map(|k| dir[k] - corner[k]);
                    let mid = [0, 1, 2, 3].map(|k| (dir[k] + corner[k]) * 0.5);
                    let h = [span[0].abs() * 0.5, span[1].abs() * 0.5, span[2].abs() * 0.5];
                    let mut d = mid;
                    for k in 0..3 {
                        if f32::from_bits(0x3ba3_d70a) < h[k] {
                            d[k] += env.rand(-h[k], h[k]);
                        }
                    }
                    dirs[i] = normalise(d);
                }
                let d = dirs[i];
                let r = env.rider(slot);
                push(&mut r.vel, d, target, rate);
                pushed[r.index as usize] = 1;
            }
            // `cTubeEndBoostNode::BoostRider` 0x1418a8
            Extra::TubeEnd { points, targets, dirs, pushed } => {
                let i = env.rider(slot).index as usize;
                let pick = env.tube_choice(i as u32) as usize;
                if dirs[i][0] == f32::from_bits(UNSET) {
                    dirs[i] = normalise(points[pick]);
                }
                let d = dirs[i];
                let t = targets[pick];
                let r = env.rider(slot);
                let i2 = r.index as usize;
                // the target is read with the choice of the slot read first
                push(&mut r.vel, d, t, rate);
                pushed[i2] = 1;
            }
            Extra::Z { .. } => {}
        }
    }

    /// Update: `cBoostNode`'s 0x140688 (plain, random, tube end) or `cZBoostNode::Update` 0x141cd8.
    pub fn update(&mut self, env: &mut impl BoostEnv) {
        if let Extra::Z { .. } = self.extra {
            return self.update_z(env);
        }
        let mut active = self.mode == 0;
        if 0 < self.left {
            self.left -= 1;
            if 0 < self.left {
                active = true;
            }
        }
        // vtable +0xec: forget who was pushed
        match &mut self.extra {
            Extra::Random { pushed, .. } | Extra::TubeEnd { pushed, .. } => *pushed = [0; 6],
            _ => {}
        }
        if self.mode == 1 || self.left <= 0 {
            for (slot, hit) in env.touching() {
                if hit {
                    active = true;
                    self.boost_rider(slot, env);
                }
            }
        }
        // vtable +0xf4: while idle, riders not pushed this frame get a new direction next time
        let n = env.riders() as usize;
        let left = self.left;
        match &mut self.extra {
            Extra::Random { dirs, pushed, .. } | Extra::TubeEnd { dirs, pushed, .. } => {
                if left < 1 {
                    for i in 0..n.min(6) {
                        if pushed[i] == 0 {
                            dirs[i][0] = f32::from_bits(UNSET);
                        }
                    }
                }
            }
            _ => {}
        }
        if !active {
            env.call(BoostCall::Finish);
        }
    }

    fn update_z(&mut self, env: &mut impl BoostEnv) {
        let Extra::Z { height, snap, mut inside } = self.extra else { return };
        let mut active = false;
        for (slot, hit) in env.touching() {
            if hit {
                let i = env.rider(slot).index as usize;
                inside[i] = 1;
            }
        }
        for i in 0..env.riders().min(6) {
            if inside[i as usize] == 0 {
                continue;
            }
            let r = *env.rider(i);
            if r.pos[2] < height {
                if height - r.pos[2] < snap {
                    env.rider(i).pos[2] = height;
                } else {
                    if r.x424 == 5 {
                        env.call(BoostCall::PlaceAt(i));
                    }
                    let (dir, target, rate) = (self.dir, self.target, self.rate);
                    let r = env.rider(i);
                    push(&mut r.vel, dir, target, rate);
                    r.vel[1] = 0.0;
                    r.vel[0] = 0.0;
                }
                active = true;
            } else {
                inside[i as usize] = 0;
            }
        }
        self.extra = Extra::Z { height, snap, inside };
        if !active {
            env.call(BoostCall::Finish);
        }
    }
}

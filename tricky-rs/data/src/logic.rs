//! The level scripts (SSFLogic.json; `TriggerScript_ExecOp`, `TriggerScript_Op0_SpawnController`):
//! what the course does by itself. Read statically here: which objects each event mode hides
//! (RaceMode / ShowoffMode / FreerideMode -> HideRace, HideShowOff, HideStartGate), which rails it
//! switches off, which objects reset a rider who touches them (op 0xd), which teleport (op 0x18),
//! and which push riders along (Boost controllers, sub-type 7).
//!
//! Script rules (from the decompile): EffectSlots point at EffectHeaders: PersistantEffectSlot runs
//! when the object's area wakes, CollisionEffectSlot on rider contact, Slot4 when a controller
//! stops, EffectTriggerSlot when it reaches its goal. Op 7 runs EffectHeaders[EffectIndex] on
//! Instances[InstanceIndex]; op 0x15 (21) runs Functions[FunctionRunIndex]; op 0 sub-type 5 with
//! DeadNodeMode 2/3 hides an object and takes away its collision for good, 4 until it is restored;
//! op 0x19 (25) turns spline SplineIndex's rail on (Effect != 0) or off.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A boost controller (sub-type 7, `cBoostNode`): while a rider touches the object, speed along
/// `dir` (game space, unit) is pulled up towards `target` cm/s at `gain` per second.
/// An object riding a spline (`cSplinePathNode`, op 2 sub-type 1): `count` copies spaced evenly,
/// moving at `speed` m/s; mode 0 stop at the end, 1 loop, 2 back and forth, 3 pace the leader;
/// orientation 0 yaw and pitch, 1 yaw only, 2/3 fixed; `yaw` an offset in radians.
#[derive(Clone, Copy, Debug)]
pub struct MoverDef { pub spline: usize, pub mode: i32, pub orient: i32, pub count: usize, pub speed: f32, pub yaw: f32, /** the cable drawn along it (ski lifts) */ pub cable: Option<[f32; 3]> }

/// A breakable's debris (`cMeshAnimNode`, sub-type 20), in game units: gravity scale (×980 cm/s²),
/// life (s), base velocity (cm/s; all zero: the rider's), its multiplier, and the random spread
/// (± x, ± y, + z cm/s).
#[derive(Clone, Copy, Debug)]
pub struct DebrisDef { pub gravity: f32, pub life: f32, pub base: [f32; 3], pub mult: f32, pub spread: [f32; 3] }

/// A keyframed object (`cAnimObjectNode`, sub-types 256 / 258): mode 0 once, 1 loop, 2 back and
/// forth; start and end frame (end < 0: the model's length); rate in frames a second (30 = normal),
/// random between `rate` and `rate2` when that is set; a random start; played in reverse.
#[derive(Clone, Copy, Debug)]
pub struct AnimDef { pub mode: i32, pub start: f32, pub end: f32, pub rate: f32, pub rate2: f32, pub random_start: bool, pub reverse: bool }

/// A particle emitter (`Emitter_InitFromRecord`, op 2 sub-type 0), in game units (cm, s), its
/// vectors in the owning instance's frame: N particles born evenly over `dur` s (continuous when
/// negative: recycled, `dur` = the longest life); drag `k` (1/s) towards terminal velocity accel/k;
/// size and life as mean ± spread/2; colour from c0 to c1 over the longest life plus random c2, c3
/// (×128); trails of `trail` copies `trail_dt` s apart; texture and blend (0 additive, 1 alpha, 4 darken).
#[derive(Clone, Copy, Debug)]
pub struct EmitterDef {
    pub n: u32, pub trail: u32, pub dur: f32, pub k: f32, pub size: f32, pub size_sp: f32, pub life: f32, pub life_sp: f32, pub trail_dt: f32,
    pub origin: [f32; 3], pub area_a: [f32; 3], pub area_b: [f32; 3], pub vel: [f32; 3], pub vs: [[f32; 3]; 3], pub accel: [f32; 3],
    pub c: [[f32; 4]; 4], pub texture: u32, pub blend: u32,
}
impl EmitterDef {
    fn from(r: &Value) -> Self {
        let u = |i: usize| fb(r, &format!("U{i}"));
        let v3 = |i: usize| [u(i), u(i + 1), u(i + 2)];
        let col = |a: usize| [u(a + 1), u(a + 2), u(a + 3), u(a)];
        Self { n: u(0).max(0.0) as u32, trail: u(1).max(1.0) as u32, dur: u(2), k: u(3), size: u(4), life: u(5), size_sp: u(6), life_sp: u(7), trail_dt: u(8),
            origin: v3(9), area_a: v3(12), area_b: v3(15), vel: v3(18), vs: [v3(21), v3(24), v3(27)], accel: v3(30),
            c: [col(33), col(37), col(41), col(45)], texture: u(49).max(0.0) as u32, blend: u(50).max(0.0) as u32 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoostDef { pub dir: [f32; 3], pub target: f32, pub gain: f32 }

/// A texture flipbook (`cTexFlipNode`, sub-type 11, ctor 0x142e90): start frame (< 0 random, -2
/// a random frame every step), backwards, frames a second, lifetime s (0 for ever), mode (0 cycle,
/// 1 blink).
#[derive(Clone, Copy, Debug)]
pub struct FlipDef { pub start: i32, pub back: bool, pub speed: f32, pub life: f32, pub mode: i32 }
/// A texture scroll (`cUVScrollNode`, sub-type 10, ctor 0x141ff0): mode (0 linear, 1 eased, 2; 1 and
/// 2 turn round after each run), du, dv a tick, run and pause s, lifetime s (0 for ever).
#[derive(Clone, Copy, Debug)]
pub struct ScrollDef { pub mode: i32, pub du: f32, pub dv: f32, pub run: f32, pub pause: f32, pub life: f32 }
/// A waving flag (`cFlagNode`, sub-type 13, 0x144938): the fixed edge mirrored, waves a second
/// (-1 random), amplitude cm (-1 random), lifetime s.
#[derive(Clone, Copy, Debug)]
pub struct FlagDef { pub mirror: bool, pub rate: f32, pub amp: f32, pub life: f32 }
/// Cracked glass (`cCrackedNode`, sub-type 14, ctor 0x145458): lifetime s (< 0 for ever) and
/// health (each impact takes |impact| × 0.036 off; at 0 the EffectTriggerSlot runs).
#[derive(Clone, Copy, Debug)]
pub struct CrackDef { pub life: f32, pub health: f32 }
/// A crowd box (`cCrowdBoxNode`, sub-type 17, ctor 0x145b28): its cells in rows × columns (one
/// mesh each, at most 16).
#[derive(Clone, Copy, Debug)]
pub struct CrowdDef { pub rows: i32, pub cols: i32 }
/// A counter (`cCounterNode`, sub-type 6, ctor 0x13b0b8): hits still needed (each SetParam takes
/// one off), lifetime s (< 0 for ever); at 0 the EffectTriggerSlot runs.
#[derive(Clone, Copy, Debug)]
pub struct CounterDef { pub count: i32, pub life: f32 }

/// The script ops the world animations need, run at play time (`worldanim`).
#[derive(Clone, Copy, Debug)]
pub enum WOp {
    Flip(FlipDef), Scroll(ScrollDef), Flag(FlagDef), Delta(AnimDef), Crack(CrackDef), Crowd(CrowdDef), Counter(CounterDef),
    /// a keyframed clip played on the object (`cAnimObjectNode`, sub-types 256 / 258)
    // parsed for the object-animation port, not read yet
    Anim(#[allow(dead_code)] AnimDef),
    /// Debounce (sub-type 2): the object counts as busy for this long (0: for good)
    Debounce(f32),
    /// a dead node (sub-type 5) that takes the object away
    Dead,
    /// op 3 / op 9: SetParam(which, value) on the object's controller
    Param(i32, f32),
    Wait(f32),
    /// op 7: run header `.1` on instance `.0`; op 21: run function
    Run(usize, usize), Call(usize),
}
/// The level scripts as `WOp`s, with where they start: on wake-up (instance, header), on touch,
/// and when a controller on the object reaches its goal (EffectTriggerSlot: a counter counted
/// down, cracked glass broken).
#[derive(Default)]
pub struct WorldScript { pub headers: Vec<Vec<WOp>>, pub functions: Vec<(String, Vec<WOp>)>, pub wake: Vec<(usize, usize)>, pub touch: HashMap<usize, usize>, pub trigger: HashMap<usize, usize> }
impl WorldScript {
    pub fn function(&self, name: &str) -> Option<usize> { self.functions.iter().position(|(n, _)| n.eq_ignore_ascii_case(name)) }
}

/// What the scripts do, worked out once when the level loads.
#[derive(Default)]
pub struct LevelLogic {
    /// per event (0 race, 1 show-off, 2 free ride): objects hidden, and rails switched on/off
    pub hide: [HashSet<usize>; 3],
    pub rails: [HashMap<usize, bool>; 3],
    /// touching these puts the rider back on the course
    pub resets: HashSet<usize>,
    /// touching these sends the rider to another object
    pub teleports: HashMap<usize, usize>,
    pub boosts: HashMap<usize, BoostDef>,
    pub movers: HashMap<usize, MoverDef>,
    pub debris: HashMap<usize, DebrisDef>,
    pub anims: HashMap<usize, AnimDef>,
    /// keyframed clips a touch script plays on an object (megaplex's iris doors, flippers), and
    /// which object's touch plays which (trigger, target)
    pub anim_on: HashMap<usize, AnimDef>, pub anim_triggers: Vec<(usize, usize)>,
    /// touching these gives a pickup: multiplier (op 0xe), speed boost seconds (0x11), spin boost seconds (0x12)
    pub pickups: HashMap<usize, (Option<f32>, Option<f32>, Option<f32>)>,
    /// objects whose touch script hides themselves (glass, logos); a multiplier gem stays
    pub self_hide: HashSet<usize>,
    /// emitters running from the start (instance, emitter), and those set off by touching an
    /// object (that object -> the instance each emitter belongs to, and the emitter)
    pub emit_wake: Vec<(usize, EmitterDef)>,
    pub emit_touch: HashMap<usize, Vec<(usize, EmitterDef)>>,
    /// world animations: objects some script flips the textures of, scrolls, waves, or plays a
    /// keyframed clip on by the bit (AnimDelta, sub-type 257); and the scripts to run them
    pub flips: HashSet<usize>, pub scrolls: HashSet<usize>, pub flags: HashMap<usize, FlagDef>, pub anim_delta: HashMap<usize, AnimDef>,
    /// glass that cracks before it breaks, and crowd boxes
    pub cracks: HashMap<usize, CrackDef>, pub crowds: HashMap<usize, CrowdDef>,
    pub world: WorldScript,
}

struct Script { headers: Vec<Vec<Value>>, slots: Vec<[i32; 5]>, functions: Vec<(String, Vec<Value>)> }

#[derive(Default)]
struct Found { hide: HashSet<usize>, rails: HashMap<usize, bool>, reset: bool, teleport: Option<usize>, boost: Option<BoostDef>, movers: Vec<(usize, MoverDef)>, debris: Vec<(usize, DebrisDef)>, anims: Vec<(usize, AnimDef)>, mult: Option<f32>, speed: Option<f32>, spin: Option<f32>, emitters: Vec<(usize, EmitterDef)> }

fn f(v: &Value, k: &str) -> f64 { v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) }
/// The exporter writes some float fields as the integer of their bits (1128792064 = 200.0).
fn fb(v: &Value, k: &str) -> f32 {
    match v.get(k) {
        Some(x) if x.is_i64() || x.is_u64() => { let i = x.as_i64().unwrap_or(0); if i.unsigned_abs() > 0x0080_0000 { f32::from_bits(i as u32) } else { i as f32 } }
        Some(x) => x.as_f64().unwrap_or(0.0) as f32,
        None => 0.0,
    }
}

impl Script {
    fn load(dir: &Path) -> Option<Self> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("SSFLogic.json")).ok()?).ok()?;
        let list = |k: &str| v.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let effects = |h: &Value| h.get("Effects").and_then(|e| e.as_array()).cloned().unwrap_or_default();
        let headers = list("EffectHeaders").iter().map(effects).collect();
        let slots = list("EffectSlots").iter().map(|s| {
            let g = |k: &str| s.get(k).and_then(|x| x.as_i64()).unwrap_or(-1) as i32;
            [g("PersistantEffectSlot"), g("CollisionEffectSlot"), g("Slot3"), g("Slot4"), g("EffectTriggerSlot")]
        }).collect();
        let functions = list("Functions").iter().map(|fun| (fun.get("FunctionName").and_then(|n| n.as_str()).unwrap_or("").to_string(), effects(fun))).collect();
        Some(Self { headers, slots, functions })
    }
    /// The ops of one script the world animations care about (others are dropped).
    fn wops(ops: &[Value], n_inst: usize) -> Vec<WOp> {
        let anim = |d: &Value| AnimDef { mode: fb(d, "U0") as i32, start: fb(d, "U1"), end: fb(d, "U2"), rate: fb(d, "U3"), rate2: fb(d, "U4"), random_start: fb(d, "U6") != 0.0, reverse: fb(d, "U7") as i32 == 4 };
        ops.iter().filter_map(|op| Some(match op.get("MainType").and_then(|m| m.as_i64())? {
            0 => {
                let t0 = op.get("type0")?;
                match t0.get("SubType").and_then(|s| s.as_i64())? {
                    2 => WOp::Debounce(f(t0, "Debounce") as f32),
                    6 => { let d = t0.get("Counter")?; WOp::Counter(CounterDef { count: f(d, "Count") as i32, life: fb(d, "U1") }) }
                    5 => match t0.get("DeadNodeMode").and_then(|m| m.as_i64()).unwrap_or(0) { 2..=4 => WOp::Dead, _ => return None },
                    10 => { let d = t0.get("UVScroll")?; WOp::Scroll(ScrollDef { mode: fb(d, "U0") as i32, du: fb(d, "U1"), dv: fb(d, "U2"), run: fb(d, "U3"), pause: fb(d, "U4"), life: fb(d, "U5") }) }
                    11 => { let d = t0.get("TextureFlip")?; WOp::Flip(FlipDef { start: fb(d, "U0") as i32, back: fb(d, "Direction") != 0.0, speed: fb(d, "Speed"), life: fb(d, "Length"), mode: fb(d, "U4") as i32 }) }
                    14 => { let d = t0.get("type0Sub14")?; WOp::Crack(CrackDef { life: fb(d, "U0"), health: fb(d, "U1") }) }
                    17 => { let d = t0.get("CrowdEffect")?; WOp::Crowd(CrowdDef { rows: fb(d, "U1") as i32, cols: fb(d, "U2") as i32 }) }
                    13 => { let d = t0.get("type0Sub13")?; WOp::Flag(FlagDef { mirror: fb(d, "U0") as i32 == 1, rate: fb(d, "U1"), amp: fb(d, "U2"), life: fb(d, "U3") }) }
                    257 => WOp::Delta(anim(t0.get("type0Sub257")?)),
                    256 => WOp::Anim(anim(t0.get("type0Sub256")?)),
                    258 => WOp::Anim(anim(t0.get("type0Sub258")?)),
                    _ => return None,
                }
            }
            n @ (3 | 9) => { let p = op.get(if n == 3 { "type3" } else { "type9" })?; WOp::Param(fb(p, "U0") as i32, fb(p, "U1")) }
            4 => WOp::Wait(f(op, "WaitTime") as f32),
            7 => { let r = op.get("Instance")?; let i = usize::try_from(f(r, "InstanceIndex") as i64).ok().filter(|i| *i < n_inst)?; WOp::Run(i, usize::try_from(f(r, "EffectIndex") as i64).ok()?) }
            21 | 26 => WOp::Call(f(op, "FunctionRunIndex") as usize),
            _ => return None,
        })).collect()
    }
    fn header(&self, h: i64) -> &[Value] { usize::try_from(h).ok().and_then(|h| self.headers.get(h)).map_or(&[], |v| v.as_slice()) }
    /// Follow a script through its calls (waits and conditions are taken as passed).
    fn walk(&self, ops: &[Value], inst: Option<usize>, n_inst: usize, out: &mut Found, depth: u32) {
        if depth > 12 { return; }
        for op in ops {
            match op.get("MainType").and_then(|m| m.as_i64()).unwrap_or(-1) {
                0 => {
                    let Some(t0) = op.get("type0") else { continue };
                    match t0.get("SubType").and_then(|s| s.as_i64()).unwrap_or(-1) {
                        5 => if let (Some(i), m) = (inst, t0.get("DeadNodeMode").and_then(|m| m.as_i64()).unwrap_or(0)) { if m == 2 || m == 3 { out.hide.insert(i); } },
                        n @ (256 | 258) => if let (Some(i), Some(d)) = (inst, t0.get(if n == 256 { "type0Sub256" } else { "type0Sub258" })) {
                            out.anims.push((i, AnimDef { mode: fb(d, "U0") as i32, start: fb(d, "U1"), end: fb(d, "U2"), rate: fb(d, "U3"), rate2: fb(d, "U4"),
                                random_start: fb(d, "U6") != 0.0, reverse: fb(d, "U7") as i32 == 4 }));
                        },
                        20 => if let (Some(i), Some(d)) = (inst, t0.get("type0Sub20")) {
                            out.debris.push((i, DebrisDef { gravity: fb(d, "U1"), life: fb(d, "U2"), base: [fb(d, "U3"), fb(d, "U4"), fb(d, "U5")],
                                spread: [fb(d, "U6"), fb(d, "U7"), fb(d, "U8")], mult: fb(d, "U9") }));
                        },
                        7 => if let Some(b) = t0.get("Boost") {
                            let d: Vec<f32> = b.get("BoostDir").and_then(|d| d.as_array()).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
                            if d.len() == 3 { out.boost = Some(BoostDef { dir: [d[0], d[1], d[2]], target: f(b, "BoostAmount") as f32 * 100.0, gain: f(b, "U2") as f32 }); }
                        },
                        _ => {}
                    }
                }
                2 => if let (Some(i), Some(t2)) = (inst, op.get("type2")) {
                    if let (Some(0), Some(r)) = (t2.get("SubType").and_then(|s| s.as_i64()), t2.get("type2Sub0")) { out.emitters.push((i, EmitterDef::from(r))); }
                    if let (Some(1), Some(a)) = (t2.get("SubType").and_then(|s| s.as_i64()), t2.get("SplineAnimation")) {
                        out.movers.push((i, MoverDef { spline: f(a, "SplineIndex") as usize, mode: f(a, "U1") as i32, orient: f(a, "U2") as i32,
                            count: (f(a, "InstanceCount") as usize).max(1), speed: f(a, "AnimationSpeed") as f32, yaw: f(a, "U5") as f32,
                            cable: (f(a, "U6") != 0.0).then(|| [f(a, "R") as f32, f(a, "G") as f32, f(a, "B") as f32]) }));
                    }
                },
                7 => if let Some(r) = op.get("Instance") {
                    let i = f(r, "InstanceIndex") as i64;
                    if i >= 0 && (i as usize) < n_inst { self.walk(self.header(f(r, "EffectIndex") as i64), Some(i as usize), n_inst, out, depth + 1); }
                },
                13 => out.reset = true,
                14 => out.mult = Some(f(op, "MultiplierScore") as f32),
                17 => out.speed = Some(f(op, "type17") as f32),
                18 => out.spin = Some(f(op, "type18") as f32),
                21 | 26 => if let Some((_, ops)) = self.functions.get(f(op, "FunctionRunIndex") as usize) { self.walk(ops, None, n_inst, out, depth + 1); },
                24 => { let t = f(op, "TeleportInstanceIndex") as i64; if t >= 0 && (t as usize) < n_inst { out.teleport = Some(t as usize); } }
                25 => if let Some(s) = op.get("Spline") { out.rails.insert(f(s, "SplineIndex") as usize, f(s, "Effect") != 0.0); },
                _ => {}
            }
        }
    }
    fn function(&self, name: &str, n_inst: usize, out: &mut Found) {
        if let Some((_, ops)) = self.functions.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) { self.walk(ops, None, n_inst, out, 0); }
    }
}

impl LevelLogic {
    /// `slot_of`: each instance's EffectSlotIndex (-1 none).
    pub fn load(dir: &Path, slot_of: &[i32]) -> Self {
        let mut l = LevelLogic::default();
        let Some(s) = Script::load(dir) else { return l };
        let n = slot_of.len();
        // the event's start: RaceMode / ShowoffMode / FreerideMode, then the countdown (or not)
        for (k, names) in [["RaceMode", "StartCountDown"], ["ShowoffMode", "StartCountDown"], ["FreerideMode", "NoCountDown"]].iter().enumerate() {
            let mut out = Found::default();
            for name in names { s.function(name, n, &mut out); }
            l.hide[k] = out.hide;
            l.rails[k] = out.rails;
        }
        for (i, slot) in slot_of.iter().enumerate() {
            let Some(sl) = usize::try_from(*slot).ok().and_then(|x| s.slots.get(x)) else { continue };
            // contact: reset, teleport; a boost can be set up when the area wakes or on contact
            let mut touch = Found::default();
            s.walk(s.header(sl[1] as i64), Some(i), n, &mut touch, 0);
            let mut wake = Found::default();
            s.walk(s.header(sl[0] as i64), Some(i), n, &mut wake, 0);
            if touch.reset { l.resets.insert(i); }
            if touch.hide.contains(&i) { l.self_hide.insert(i); }
            if touch.mult.is_some() || touch.speed.is_some() || touch.spin.is_some() { l.pickups.insert(i, (touch.mult, touch.speed, touch.spin)); }
            if let Some(t) = touch.teleport { l.teleports.insert(i, t); }
            if let Some(b) = touch.boost.or(wake.boost) { l.boosts.insert(i, b); }
            for (k, m) in wake.movers { l.movers.insert(k, m); }
            for (k, d) in touch.debris.into_iter().chain(wake.debris) { l.debris.insert(k, d); }
            for (k, a) in wake.anims { l.anims.insert(k, a); }
            for (k, a) in touch.anims { l.anim_on.entry(k).or_insert(a); l.anim_triggers.push((i, k)); }
            l.emit_wake.extend(wake.emitters);
            if !touch.emitters.is_empty() { l.emit_touch.insert(i, touch.emitters); }
        }
        l.world_scripts(&s, slot_of);
        // set going by the event's start-up scripts too
        for name in ["RaceMode", "ShowoffMode", "FreerideMode", "StartCountDown", "NoCountDown"] {
            let mut out = Found::default();
            s.function(name, n, &mut out);
            for (k, m) in out.movers { l.movers.insert(k, m); }
        }
        l
    }
    /// The world-animation scripts, and which objects they animate (followed from every slot
    /// and every function).
    fn world_scripts(&mut self, s: &Script, slot_of: &[i32]) {
        let n = slot_of.len();
        let mut w = WorldScript { headers: s.headers.iter().map(|h| Script::wops(h, n)).collect(),
            functions: s.functions.iter().map(|(name, ops)| (name.clone(), Script::wops(ops, n))).collect(), ..Default::default() };
        let (mut wake, mut touch, mut trigger) = (Vec::new(), HashMap::new(), HashMap::new());
        // does a script (through its calls) start an animation?
        fn animates(w: &WorldScript, ops: &[WOp], depth: u32) -> bool {
            depth < 12 && ops.iter().any(|op| match op {
                WOp::Flip(_) | WOp::Scroll(_) | WOp::Flag(_) | WOp::Delta(_) | WOp::Crack(_) | WOp::Crowd(_) | WOp::Counter(_) | WOp::Param(..) | WOp::Anim(_) => true,
                WOp::Run(_, h) => w.headers.get(*h).is_some_and(|o| animates(w, o, depth + 1)),
                WOp::Call(f) => w.functions.get(*f).is_some_and(|o| animates(w, &o.1, depth + 1)),
                _ => false,
            })
        }
        let mut roots: Vec<(Option<usize>, &[WOp])> = Vec::new();
        for (i, slot) in slot_of.iter().enumerate() {
            let Some(sl) = usize::try_from(*slot).ok().and_then(|x| s.slots.get(x)) else { continue };
            for (k, h) in [(0, sl[0]), (1, sl[1]), (4, sl[4])] {
                let Some(ops) = usize::try_from(h).ok().and_then(|h| w.headers.get(h)) else { continue };
                // a trigger script counts whatever it does (glass: hide itself, break its neighbours)
                if k != 4 && !animates(&w, ops, 0) { continue; }
                match k { 0 => wake.push((i, h as usize)), 1 => { touch.insert(i, h as usize); } _ => { trigger.insert(i, h as usize); } }
                roots.push((Some(i), ops.as_slice()));
            }
        }
        for (_, ops) in &w.functions { roots.push((None, ops.as_slice())); }
        // every object an animation op lands on
        let mut seen: HashSet<(Option<usize>, usize)> = HashSet::new();
        let mut stack: Vec<(Option<usize>, &[WOp])> = roots;
        while let Some((inst, ops)) = stack.pop() {
            for op in ops {
                match (*op, inst) {
                    (WOp::Flip(_), Some(i)) => { self.flips.insert(i); }
                    // a scroll that never moves by itself still counts: SetParam 5 / 6 sets its offset
                    // (merqury's strike lights show their lit half)
                    (WOp::Scroll(_), Some(i)) => { self.scrolls.insert(i); }
                    (WOp::Crack(d), Some(i)) => { self.cracks.insert(i, d); }
                    (WOp::Crowd(d), Some(i)) => { self.crowds.insert(i, d); }
                    (WOp::Flag(d), Some(i)) => { self.flags.insert(i, d); }
                    (WOp::Delta(d), Some(i)) => { self.anim_delta.insert(i, d); }
                    (WOp::Run(i, h), _) if seen.insert((Some(i), h)) => { if let Some(o) = w.headers.get(h) { stack.push((Some(i), o)); } }
                    (WOp::Call(fi), _) if seen.insert((None, usize::MAX - fi)) => { if let Some(o) = w.functions.get(fi) { stack.push((None, &o.1)); } }
                    _ => {}
                }
            }
        }
        // a touch script that breaks cracked glass (the glass floors' undersides) counts too
        fn reaches(w: &WorldScript, ops: &[WOp], cracks: &HashMap<usize, CrackDef>, depth: u32) -> bool {
            depth < 12 && ops.iter().any(|op| match op {
                WOp::Run(i, h) => cracks.contains_key(i) || w.headers.get(*h).is_some_and(|o| reaches(w, o, cracks, depth + 1)),
                WOp::Call(f) => w.functions.get(*f).is_some_and(|o| reaches(w, &o.1, cracks, depth + 1)),
                _ => false,
            })
        }
        for (i, slot) in slot_of.iter().enumerate() {
            let Some(sl) = usize::try_from(*slot).ok().and_then(|x| s.slots.get(x)) else { continue };
            let Some(ops) = usize::try_from(sl[1]).ok().and_then(|h| w.headers.get(h)) else { continue };
            if !touch.contains_key(&i) && reaches(&w, ops, &self.cracks, 0) { touch.insert(i, sl[1] as usize); }
        }
        w.wake = wake;
        w.touch = touch;
        w.trigger = trigger;
        self.world = w;
    }
    /// Objects some event hides (drawn and collided one by one so they can be switched).
    pub fn switchable(&self) -> HashSet<usize> { self.hide.iter().flatten().copied().collect() }
}

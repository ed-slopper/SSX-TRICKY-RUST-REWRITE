//! The course's keyframed objects over a recorded race (row F4h5l): when each gets its `cAnimObjectNode` and where
//! its clip is on every frame, so the casts pose its parts as the game's did.
//!
//! Proven (code and PINE on mesa, 2026-11-18): a frame runs the object lists in the order 0, 1, 2, 3, 5, 6, 7, 8, 4,
//! 10, 11 (`Game_UpdateObjectLists` 0x1814c0, the table at 0x337c70), the AI world (`cAIWorld::vf1` 0x115e28, list
//! 2) before the controllers (list 3) and the riders (list 8). So the cells turn on and off from where the riders were
//! at the end of the last frame; a node made then (`TriggerScript_Op0_SpawnController` 0x13c5d0 from the persistent
//! script, `World_CellActivate_RunPersistant` 0x25fed0) waits in the pending list until the frame's end
//! (`BxList_MergePending`), so its clip is at its start for that frame and steps once a frame after (before the
//! riders: their casts see the frame's step); a node killed (`World_CellDeactivate` 0x260108) is gone for that
//! frame's casts. Mesa's 1444 read every tick over PINE: made on the frame after the riders' positions turned its
//! cell on, killed the frame after they turned it off, its clock (`cAnimObjectNode::Update` 0x199058) PCSX2's COP1
//! adder bit for bit (2041 of 2041).
//!
//! The scripts are run only as far as the clocks need: op 0 of type 0x100 (`cAnimObjectNode`, made with
//! `objanim::AnimObject::new`), other controller types as present only, op 7 (an effect on another object) at once,
//! the other ops not. The nodes alive at the recording's first sample have no known start and are left out.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use tricky_data::pbd::Pbd;
use tricky_data::ssf::Ssf;
use tricky_game::cells::{AiCells, CellEnv, RiderCell};
use tricky_game::controllers::{CtrlCall, CtrlEnv};
use tricky_game::objanim::{AnimObject, ObjAnimCall, ObjAnimEnv};
use tricky_game::texanim::AnimEnv;
use tricky_game::world::World;

/// Each sample's keyframed objects (by the sample's index): the object and its clip's time as the riders' casts in
/// the sample's frame saw it.
pub type Clocks = BTreeMap<usize, Vec<(usize, f32)>>;

struct Env<'a> {
    pbd: &'a Pbd,
    ssf: &'a Ssf,
    flags: Vec<u32>,
    /// each object's controller type (+0x14), if any
    ctrl: Vec<Option<u32>>,
    nodes: HashMap<usize, AnimObject>,
    /// made this frame (pending until its end), or alive at the start (no known clock)
    fresh: Vec<usize>,
    unknown: Vec<usize>,
}

impl CtrlEnv for Env<'_> {
    fn flags(&mut self, object: u32) -> &mut u32 {
        &mut self.flags[object as usize]
    }
    fn event_script(&self, object: u32, event: u32) -> i32 {
        self.ssf.event_script(object as usize, event as usize)
    }
    fn rider_in_box(&self, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn call(&mut self, _: CtrlCall) {}
}
impl AnimEnv for Env<'_> {
    fn rand_fx(&mut self) -> u32 {
        0
    }
    fn contact_script(&mut self) {}
}
impl ObjAnimEnv for Env<'_> {
    /// a random start (op word 7) or speed jitter (word 5) would need the game's random stream: none on the clocks
    /// checked so far
    fn rand_game(&mut self) -> u32 {
        0
    }
    fn anim(&mut self, _: ObjAnimCall) {}
}

impl Env<'_> {
    /// `TriggerScript_Spawn`(effect, object), its ops at once.
    fn run(&mut self, effect: i32, o: usize) {
        let Some(ops) = self.ssf.effects.get(effect as usize).cloned() else { return };
        for op in ops {
            match op.op {
                0 if self.ctrl[o].is_none() => {
                    let kind = op.args.first().copied().unwrap_or(0);
                    self.ctrl[o] = Some(kind);
                    if kind == 0x100 {
                        let model = self.pbd.instances[o][0x30] as usize;
                        let model_end = f32::from_bits(self.pbd.models.get(model).map_or(0, |m| m.head[5]));
                        let a = AnimObject::new(o as u32, &op.args, model_end, false, true, self);
                        self.nodes.insert(o, a);
                        self.fresh.push(o);
                    }
                }
                7 => {
                    let (obj, eff) = (op.args.first().copied().unwrap_or(0), op.args.get(1).copied().unwrap_or(0));
                    if (obj as usize) < self.ctrl.len() {
                        self.run(eff as i32, obj as usize);
                    }
                }
                _ => {}
            }
        }
    }
}

impl CellEnv for Env<'_> {
    fn flags(&mut self, o: usize) -> &mut u32 {
        &mut self.flags[o]
    }
    fn controller(&self, o: usize) -> Option<u32> {
        self.ctrl[o]
    }
    fn event_script(&self, o: usize, e: u32) -> i32 {
        self.ssf.event_script(o, e as usize)
    }
    fn spawn(&mut self, s: i32, o: usize) {
        self.run(s, o);
    }
    /// `cControllerNode`'s +0x64 answers 0 (0x13b7d0)
    fn ctrl_wants(&mut self, _: usize) -> bool {
        false
    }
    fn ctrl_7c(&mut self, _: usize) -> bool {
        false
    }
    fn ctrl_74(&mut self, _: usize) {}
    fn ctrl_kill(&mut self, o: usize) {
        self.ctrl[o] = None;
        self.nodes.remove(&o);
        self.unknown.retain(|x| *x != o);
    }
}

/// The clocks over a recording: `samples` each sample's frame number and the riders' positions at its end
/// (+0x140), in the recording's order; `add` the clock's adder (IEEE for ours, `Round::Guard` for PCSX2's). For each
/// sample, the keyframed objects whose node's start is known and their times as that sample's frame (the one ending
/// at it) had them. Frames missing between two samples take the earlier one's positions; a sample whose frame is not
/// after the last one's (a new race) starts over, with nothing known.
pub fn clocks(pbd: &Pbd, ssf: &Ssf, world: &World, samples: &[(u32, Vec<[f32; 4]>)], add: fn(f32, f32) -> f32) -> Clocks {
    let n_obj = pbd.instances.len();
    let fresh_env = || Env { pbd, ssf, flags: world.objects.iter().map(|o| o.flags).collect(), ctrl: vec![None; n_obj], nodes: HashMap::new(), fresh: vec![], unknown: vec![] };
    let mut env = fresh_env();
    let mut cells = AiCells::default();
    let mut out = Clocks::new();
    let mut prev: Vec<i32> = vec![];
    for k in 0..samples.len() {
        let (tick, _) = &samples[k];
        if k == 0 || *tick <= samples[k - 1].0 {
            // the first sample of a race: the cells the riders hold now, their nodes of unknown age
            env = fresh_env();
            cells = AiCells::default();
            let pos = &samples[k].1;
            prev = vec![-1; pos.len()];
            let mut rc: Vec<RiderCell> = pos.iter().map(|p| RiderCell { pos: *p, cell: 0, prev: -1 }).collect();
            cells.tick(world, &mut rc, 4, [0.0; 4], false, &mut env);
            prev = rc.iter().map(|r| r.cell).collect();
            env.unknown = env.nodes.keys().copied().collect();
            out.insert(k, vec![]);
            continue;
        }
        let pos = &samples[k - 1].1;
        for _frame in samples[k - 1].0 + 1..=*tick {
            // the cells from where the riders were at the end of the last frame, then the nodes made before step
            let mut rc: Vec<RiderCell> = pos.iter().zip(&prev).map(|(p, c)| RiderCell { pos: *p, cell: 0, prev: *c }).collect();
            env.fresh.clear();
            cells.tick(world, &mut rc, 4, [0.0; 4], false, &mut env);
            for (c, r) in prev.iter_mut().zip(&rc) {
                *c = r.cell;
            }
            let fresh = env.fresh.clone();
            let mut stepped: Vec<(usize, AnimObject)> = env.nodes.iter().map(|(o, a)| (*o, *a)).collect();
            stepped.sort_by_key(|x| x.0);
            let mut done = vec![];
            for (o, a) in stepped.iter_mut() {
                if !fresh.contains(o) && a.update_by(&mut env, add) {
                    done.push(*o);
                }
            }
            for (o, a) in stepped {
                env.nodes.insert(o, a);
            }
            for o in done {
                env.nodes.remove(&o);
                env.ctrl[o] = None;
            }
        }
        let mut live: Vec<(usize, f32)> = env.nodes.iter().filter(|(o, _)| !env.unknown.contains(o)).map(|(o, a)| (*o, a.time)).collect();
        live.sort_by_key(|x| x.0);
        out.insert(k, live);
    }
    out
}

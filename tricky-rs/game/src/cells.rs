//! Live world cells (board row E7b): the AI world keeps the grid cells (TGroups) around the riders live, and
//! turning a cell on or off runs its objects' scripts: `cAIWorld::vf1` 0x115e28 (each tick), `Boarder_UpdateGridCell` 0x11d438 (rider+0x16c), `AIWorld_MarkCellsAround` 0x1160f8, `AIWorld_MarkCellActive` 0x116420, `FUN_001162c0`,
//! `AIWorld_UpdateActiveCells` 0x1162f8, `AIWorld_ReleaseActiveCells` 0x1163a0, `World_CellActivate_RunPersistant`
//! 0x25fed0 and `World_CellDeactivate` 0x260108. Checked in the function runner (`tests/cells.rs`).

use crate::world::World;

/// One of the AI world's 108 cell slots (+0xc, 8 bytes each, and +0x36c its grid index): the cell (a TGroup, by
/// its grid index), how many ticks it has gone unmarked (+4: set to 1 when marked, less 1 each tick), and whether
/// it was marked for the first time (+6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Slot {
    pub cell: Option<usize>,
    pub seen: i16,
    pub new: i16,
    pub index: i32,
}

/// The AI world's cells (cAIWorld +0xc .. +0x51c).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiCells {
    pub slots: [Slot; 108],
}

impl Default for AiCells {
    fn default() -> Self {
        AiCells { slots: [Slot { cell: None, seen: 0, new: 0, index: 0 }; 108] }
    }
}

/// An object as turning cells on and off sees it: +0xe4 its controller (with its type, +0x14), +0xe8 its flags.
pub trait CellEnv {
    /// the object's flags (+0xe8)
    fn flags(&mut self, object: usize) -> &mut u32;
    /// its controller's type (+0x14), or none
    fn controller(&self, object: usize) -> Option<u32>;
    /// `Instance_GetEventScript`(object, event): −1 for none
    fn event_script(&self, object: usize, event: u32) -> i32;
    /// `TriggerScript_Spawn`(script, object)
    fn spawn(&mut self, script: i32, object: usize);
    /// the controller's virtual +0x64 (whether it wants waking), +0x7c and +0x74 (wake), +0xbc (Kill)
    fn ctrl_wants(&mut self, object: usize) -> bool;
    fn ctrl_7c(&mut self, object: usize) -> bool;
    fn ctrl_74(&mut self, object: usize);
    fn ctrl_kill(&mut self, object: usize);
}

/// The objects of a cell the activation passes walk: each INode with show-off objects (+0x28 not 0), its first
/// +0x26 objects (+0x28 in show-off modes 3 and 5).
fn cell_objects(world: &World, cell: usize, show_off: bool) -> Vec<usize> {
    let mut out = vec![];
    let Some(Some(c)) = world.cells.get(cell) else { return out };
    for k in 0..world.nodes_per_cell.max(0) as usize {
        let Some(node) = c.nodes.get(k) else { break };
        if node.n_objects_showoff == 0 {
            continue;
        }
        let n = if show_off { node.n_objects_showoff } else { node.n_objects };
        out.extend(node.objects.iter().take(n.max(0) as usize));
    }
    out
}

/// `World_CellActivate_RunPersistant` 0x25fed0: each object gets flag 0x100; without a controller its event-0
/// script starts; with one, the controller's +0x64 and then +0x7c say whether its +0x74 runs.
pub fn activate(world: &World, cell: usize, show_off: bool, env: &mut impl CellEnv) {
    for o in cell_objects(world, cell, show_off) {
        *env.flags(o) |= 0x100;
        if env.controller(o).is_none() {
            let s = env.event_script(o, 0);
            if s != -1 {
                env.spawn(s, o);
            }
        } else if env.ctrl_wants(o) && env.ctrl_7c(o) {
            env.ctrl_74(o);
        }
    }
}

/// `World_CellDeactivate` 0x260108: each object loses flag 0x100; unless its controller is of type 5, its
/// event-2 script starts, or (none) its controller is killed.
pub fn deactivate(world: &World, cell: usize, show_off: bool, env: &mut impl CellEnv) {
    for o in cell_objects(world, cell, show_off) {
        *env.flags(o) &= !0x100;
        let ctrl = env.controller(o);
        if ctrl == Some(5) {
            continue;
        }
        let s = env.event_script(o, 2);
        if s != -1 {
            env.spawn(s, o);
        } else if ctrl.is_some() {
            env.ctrl_kill(o);
        }
    }
}

impl AiCells {
    /// `FUN_001162c0`: every held cell one tick older.
    pub fn age(&mut self) {
        for s in self.slots.iter_mut() {
            if s.cell.is_some() {
                s.seen = s.seen.wrapping_sub(1);
            }
        }
    }

    /// `AIWorld_MarkCellActive` 0x116420(grid index): the cell there (none at index 0xffff or an empty one)
    /// marked seen; a cell not held yet takes the first free slot, marked new.
    pub fn mark(&mut self, world: &World, index: i32) {
        if index == 0xffff {
            return;
        }
        let Some(Some(_)) = world.cells.get(index as usize) else { return };
        let cell = index as usize;
        let mut free = None;
        for (k, s) in self.slots.iter_mut().enumerate() {
            if s.cell == Some(cell) {
                s.seen = 1;
                return;
            }
            if free.is_none() && s.cell.is_none() {
                free = Some(k);
            }
        }
        let Some(k) = free else { return };
        self.slots[k] = Slot { cell: Some(cell), seen: 1, new: 1, index };
    }

    /// `AIWorld_MarkCellsAround` 0x1160f8: the cells from two below to the one at the point (x outer, y inner), as
    /// the casts take them.
    pub fn mark_around(&mut self, world: &World, p: [f32; 4]) {
        let cx = (((p[0] - world.origin[0]) + world.size) / world.size) as i32;
        let cy = (((p[1] - world.origin[1]) + world.size) / world.size) as i32;
        let x_hi = cx.max(0).min(world.width - 1);
        let x_lo = cx.wrapping_sub(2).max(0);
        let y_hi = cy.max(0).min(world.height - 1);
        let y_lo = cy.wrapping_sub(2).max(0);
        let mut x = x_lo;
        while x <= x_hi {
            let mut y = y_lo;
            while y <= y_hi {
                let gx = if x < 0 { 0 } else { x.min(world.width - 1) };
                let gy = if y < 0 { 0 } else { y.min(world.height - 1) };
                self.mark(world, gy * world.width + gx);
                y += 1;
            }
            x += 1;
        }
    }

    /// `AIWorld_UpdateActiveCells` 0x1162f8: new cells turned on; cells unmarked for a tick (seen below 1)
    /// turned off and let go.
    pub fn update(&mut self, world: &World, show_off: bool, env: &mut impl CellEnv) {
        for k in 0..self.slots.len() {
            let Some(cell) = self.slots[k].cell else { continue };
            if self.slots[k].new != 0 {
                activate(world, cell, show_off, env);
                self.slots[k].new = 0;
            } else if self.slots[k].seen < 1 {
                deactivate(world, cell, show_off, env);
                self.slots[k].cell = None;
                self.slots[k].index = -1;
            }
        }
    }

    /// `AIWorld_ReleaseActiveCells` 0x1163a0: every held cell turned off and let go.
    pub fn release(&mut self, world: &World, show_off: bool, env: &mut impl CellEnv) {
        for k in 0..self.slots.len() {
            if let Some(cell) = self.slots[k].cell {
                deactivate(world, cell, show_off, env);
                self.slots[k].cell = None;
                self.slots[k].index = -1;
            }
        }
    }
}

/// `Boarder_UpdateGridCell`: the rider's grid cell (rider+0x16c) from its position, (p − corner + size)/size − 1 on
/// each axis, clamped.
pub fn grid_cell(world: &World, p: [f32; 4]) -> i32 {
    let x = (((p[0] - world.origin[0]) + world.size) / world.size) as i32 - 1;
    let y = (((p[1] - world.origin[1]) + world.size) / world.size) as i32 - 1;
    let x = if x < 0 { 0 } else { x.min(world.width - 1) };
    let y = if y < 0 { 0 } else { y.min(world.height - 1) };
    y * world.width + x
}

/// A rider as the tick reads it: +0x140 its position, +0x16c its grid cell (written), +0x42c the cell it had.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RiderCell {
    pub pos: [f32; 4],
    pub cell: i32,
    pub prev: i32,
}

impl AiCells {
    /// `cAIWorld::vf1` 0x115e28 (after the patch cache ages, `FUN_00251d70`): each rider grid cell; then, unless
    /// every rider is still in the cell it had (checked from the first; when the race object +0x1c is below 2 and
    /// the first rider has not moved on, the camera point counts too), the cells age, those around each rider (and
    /// the camera point) are marked, and the update runs.
    pub fn tick(&mut self, world: &World, riders: &mut [RiderCell], race_1c: i32, camera: [f32; 4], show_off: bool, env: &mut impl CellEnv) {
        for r in riders.iter_mut() {
            r.cell = grid_cell(world, r.pos);
        }
        if riders.is_empty() {
            return;
        }
        let mut cam = false;
        if riders[0].prev == riders[0].cell {
            let mut j = 0;
            loop {
                if race_1c < 2 {
                    cam = true;
                    break;
                }
                j += 1;
                if j >= riders.len() {
                    return;
                }
                if riders[j].prev != riders[j].cell {
                    break;
                }
            }
        }
        self.age();
        for r in riders.iter() {
            self.mark_around(world, r.pos);
        }
        if cam {
            self.mark_around(world, [camera[0], camera[1], camera[2], 1.0]);
        }
        self.update(world, show_off, env);
    }
}

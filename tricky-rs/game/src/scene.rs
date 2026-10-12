//! The race scenes' set-up (board row G11b): the scene object (`RaceScene`, built by `RaceScene_Construct`
//! 0x169640) takes the pre-race selection's scenes (the selection object 0x32ecf0 from +0x33c: fifteen scenes of 100
//! bytes, each its script then six cast slots of 0x10: two words, the rider id, the candidate) and picks each slot's
//! rider; a scene, when chosen, orders its cast by when each one's clip first marks. The scene index the selection
//! reads (0x3cc028, `PreRaceSel_BuildSceneIndex` 0x168360) is row E1b2o's.
//! Checked in the function runner (`tests/scene.rs`).

/// The scripts by number (0x32f380), the selection's first word of a scene
pub const SCRIPTS: [&str; 58] = [
    "RaceLoaded", "FreeLoaded", "ShowLoaded", "STG_VEN_1", "STG_VEN_2", "STG_VEN_3", "STG_VEN_4", "UNTR_VEN",
    "PIPE_VEN", "STG_COM_1", "STG_COM_2", "STG_COM_3", "STG_COM_4", "GAT_6COM_1", "GAT_6COM_2", "GAT_6COM_3",
    "GAT_6COM_4", "GAT_SO", "GAT_FR_1", "GAT_FR_2", "GAT_FR_3", "GAT_FO_1", "GAT_FO_2", "GAT_FO_3", "CRD_1",
    "CRD_2", "CRD_3", "FL_WIN_1", "FL_WIN_2", "FL_WIN_3", "FL_LOS_1", "FL_LOS_2", "FL_LOS_3", "FL_USR_VS_EDD",
    "FL_USR_VS_KAO", "FL_USR_VS_LUT", "FL_USR_VS_MAC", "FL_USR_VS_MOB", "FL_USR_VS_ZOE", "FL_USR_VS_JP",
    "FL_USR_VS_ELI", "FL_USR_VS_PSY", "FL_USR_VS_SEE", "FL_USR_VS_BRO", "FL_USR_VS_MAR", "FL_EDD_VS_USR",
    "FL_KAO_VS_USR", "FL_LUT_VS_USR", "FL_MAC_VS_USR", "FL_MOB_VS_USR", "FL_ZOE_VS_USR", "FL_JP_VS_USR",
    "FL_ELI_VS_USR", "FL_PSY_VS_USR", "FL_SEE_VS_USR", "FL_BRO_VS_USR", "FL_MAR_VS_USR", "Null",
];

/// The rider id that ends the riders list (and marks a slot with no rider)
pub const NO_RIDER: i32 = 0x1bf;

/// The scenes in a selection and the slots in a scene
pub const SCENES: usize = 15;
pub const SLOTS: usize = 6;

/// The selection's scenes as words (25 a scene from +0x33c): the script, then per slot two words, the rider id and
/// the candidate
pub type Selection = [[i32; 25]; SCENES];

/// A cast slot as the scene object keeps it: the selection's two words and the rider picked (−1 none)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub a: i32,
    pub b: i32,
    pub pick: i32,
}

/// `RaceScene` (0x458 + 0x168 bytes): +0 the scene chosen, +4 the scenes' slots (0x48 each), +0x43c the selection
/// (here whether one was taken), +0x440 the chosen scene's cast (−1 ends it), +0x458 the riders of all scenes (90,
/// `NO_RIDER` ends it)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceScene {
    pub current: i32,
    pub slots: [[Slot; SLOTS]; SCENES],
    pub has_selection: bool,
    pub cast: [i32; SLOTS],
    pub riders: [i32; 90],
}

impl Default for RaceScene {
    fn default() -> Self {
        RaceScene { current: 0, slots: [[Slot::default(); SLOTS]; SCENES], has_selection: false, cast: [0; SLOTS], riders: [0; 90] }
    }
}

/// What `RaceScene_Select` asks the game
pub trait SceneEnv {
    /// `Race_GetRider`(gApp+0x730, 0): its id
    fn race_rider0(&mut self) -> u32;
    /// `AnimCtl_FindFirstMarkerFrame` 0x15fb08(the rider's animation +0x46c0, clip): the frame, below 0 none
    fn first_marker_frame(&mut self, rider: u32, clip: i32) -> i32;
}

impl RaceScene {
    /// `RaceScene_Reset` 0x169688: no selection, the scene 0x10
    pub fn reset(&mut self) {
        self.has_selection = false;
        self.current = 0x10;
    }

    /// `RaceScene_SetupFromPicks` 0x169698(selection): every scene's slots copied, each slot with a candidate (≥ 0)
    /// picking among its one candidate with a `Rand_Fx` draw (`% 1`), else −1; then the riders of all scenes once
    /// each in order (`NO_RIDER` skipped), ended by `NO_RIDER` when under 90
    pub fn setup_from_picks(&mut self, sel: &Selection, rand_fx: &mut dyn FnMut() -> u32) {
        for (i, scene) in sel.iter().enumerate() {
            for j in 0..SLOTS {
                let w = &scene[1 + 4 * j..5 + 4 * j];
                let pick = if w[3] < 0 {
                    -1
                } else {
                    // (the game counts the candidates up to one)
                    let r = rand_fx() as i32;
                    w[3 + (r % 1) as usize]
                };
                self.slots[i][j] = Slot { a: w[0], b: w[1], pick };
            }
        }
        let mut n = 0usize;
        for scene in sel {
            for j in 0..SLOTS {
                let id = scene[3 + 4 * j];
                if id != NO_RIDER && !self.riders[..n].contains(&id) {
                    self.riders[n] = id;
                    n += 1;
                }
            }
        }
        if n < 90 {
            self.riders[n] = NO_RIDER;
        }
        self.has_selection = true;
    }

    /// `FUN_00169948`: the riders list's length (0 when it starts with `NO_RIDER`; 90 when it never ends)
    pub fn rider_count(&self) -> usize {
        if self.riders[0] == NO_RIDER {
            return 0;
        }
        (1..90).find(|&k| self.riders[k] == NO_RIDER).unwrap_or(90)
    }

    /// `RaceScene_Select` 0x1699a0(scene): the scene chosen; its picked slots whose clip (the slot's second word)
    /// has a first marker on race rider 0, ordered by that frame (ties the earlier slot), become the cast
    pub fn select(&mut self, env: &mut dyn SceneEnv, scene: i32) {
        self.current = scene;
        let mut found: Vec<(i32, i32)> = Vec::new();
        for j in 0..SLOTS {
            let s = self.slots[scene as usize][j];
            if 0 <= s.pick {
                let rider = env.race_rider0();
                let f = env.first_marker_frame(rider, s.b);
                if 0 <= f {
                    found.push((s.pick, f));
                }
            }
        }
        let n = found.len();
        let mut taken = vec![false; n];
        for k in 0..n {
            let mut best: Option<usize> = None;
            for i in 0..n {
                if !taken[i] && best.map_or(true, |b| found[i].1 < found[b].1) {
                    best = Some(i);
                }
            }
            let b = best.unwrap();
            taken[b] = true;
            self.cast[k] = found[b].0;
        }
        if n < 6 {
            self.cast[n] = -1;
        }
    }

    /// `RaceScene_GetCast` 0x169b60: the cast's length (to the first −1, at most 6)
    pub fn cast_count(&self) -> usize {
        if self.cast[0] == -1 {
            return 0;
        }
        (1..6).find(|&k| self.cast[k] == -1).unwrap_or(6)
    }

    /// `RaceScene_GetScriptName` 0x169bb8: the chosen scene's script (its first word in the selection)
    pub fn script_name(&self, sel: &Selection) -> Option<&'static str> {
        SCRIPTS.get(sel[self.current as usize][0] as usize).copied()
    }
}

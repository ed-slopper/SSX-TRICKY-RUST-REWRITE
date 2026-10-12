//! The two riders' idle gestures that draw from `Rand_Game` (board row E1b2d): `Ride_AITauntCheck` 0x103660 (a
//! computer rider pointing at a rival it has just passed) and `CruiseControl_RandomFlourish` 0x103b88 (a flourish
//! while cruising in a long turn). Both pick a clip and set the layer mask (rider+0x46dc) around it, which they put
//! back to -1 after. Checked in the function runner (`tests/taunt.rs`).

type V4 = [f32; 4];

/// What they do to the rider, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureCall {
    /// rider+0x46dc is this mask of animation layers (the bit `layer + k` for each of the gesture's offsets)
    Mask(u32),
    /// `Speech_OnAITaunt`(rider, `Race_GetRider`(rival index))
    Taunt(usize),
    /// `AnimCtl_PlayClip`(rider+0x46c0, clip, 0, 5)
    Clip(u32),
    /// rider+0x46dc = -1
    MaskOff,
}

/// The mask: the bit `(layer + k) & 31` for each offset `k`, or nothing for a rider with no layer (-1).
fn layer_mask(layer: Option<i32>, offsets: &[u32]) -> u32 {
    match layer {
        None => 0,
        Some(l) => offsets.iter().fold(0, |m, &k| m | 1u32 << ((l as u32).wrapping_add(k) & 31)),
    }
}

/// What `Ride_AITauntCheck` reads of the rider and its race.
#[derive(Clone, Debug, Default)]
pub struct TauntRider {
    /// the rider's layer-1 clip is a ride clip (type 8)
    pub riding: bool,
    /// rider+0x460: its own place in the race
    pub me: usize,
    /// rider+0x118 and +0x120 (the rival it passed) and +0x124 (how long ago, 0..2)
    pub passed: bool,
    pub rival: usize,
    pub stage: i32,
    /// the pair records, rider+0x40 + 0x20 × index: [0] the distance, [1] the angle
    pub pairs: Vec<[f32; 2]>,
    /// rider+0x1b0 (heading) and +0x1b4 (switch stance)
    pub heading: f32,
    pub switch: bool,
    /// the rider's animation layer: the word at rider+0xb50 + 0x394 × (rider+0x44d8), if that is not negative
    pub layer: Option<i32>,
    /// how many riders race (the game's +0x88)
    pub count: usize,
}

/// `Ride_AITauntCheck` 0x103660(object): `timer` (the object's +0xc) grows 1/60 a tick while the rider rides and is
/// 0 otherwise; at 1.0253273 s each tick has a `Rand_Game() % 100 < 91` chance (unsigned) to look for a rival: the
/// one it passed within three ticks (stage 0..=2) that is within 1000.13 and more than 60° off its heading
/// (the angle wrapped to ±π) gets a taunt, with a clip by the angle: over 135° 0x211, else 0x213 or 0x212 by the side
/// (against the stance).
pub fn ai_taunt(timer: &mut f32, r: &TauntRider, rand_game: &mut dyn FnMut() -> u32, calls: &mut Vec<GestureCall>) {
    if !r.riding {
        *timer = 0.0;
        return;
    }
    *timer += 0.016_666_668;
    if !(1.025_327_3 <= *timer) || (rand_game() % 100) >= 0x5b {
        return;
    }
    let mut i = 0usize;
    while i < r.count {
        if i != r.me {
            let ok = r.passed && r.rival == i && r.stage < 3 && r.stage >= 0;
            if ok {
                let p = r.pairs[i];
                let d = p[1] - r.heading;
                let t = d * 0.159_154_94 + 0.5;
                let mut turns = t as i32 as f32;
                if t < turns {
                    turns -= 1.0;
                }
                let d = d - turns * 6.283_185_5;
                if p[0] < 1000.132_14 && 1.047_197_6 < d.abs() {
                    calls.push(GestureCall::Mask(layer_mask(r.layer, &[3, 6, 10, 12, 4, 9, 11, 5, 7, 8, 1, 2])));
                    calls.push(GestureCall::Taunt(i));
                    calls.push(GestureCall::Clip(if 2.356_194_5 < d.abs() {
                        0x211
                    } else if (d < 0.0) == r.switch {
                        0x213
                    } else {
                        0x212
                    }));
                    calls.push(GestureCall::MaskOff);
                    return;
                }
            }
        }
        i += 1;
    }
}

/// What `CruiseControl_RandomFlourish` reads of the rider.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlourishRider {
    /// the layer-1 clip is a ride clip (type 8)
    pub riding: bool,
    /// rider+0x214 and +0x218: the lean and its change
    pub lean: f32,
    pub lean_rate: f32,
    /// rider+0x208
    pub crouch: f32,
    pub layer: Option<i32>,
}

/// `CruiseControl_RandomFlourish` 0x103b88(object): riding and leaning past 0.5502836 with the lean growing
/// (`lean × rate > 0`), one tick in 96 (`Rand_Game() % 0x60 == 0`, unsigned) the rider flourishes: crouching low
/// (under 0.2) clip 0x20a, or 0x20b when the lean is past 0.8000276; crouching high (over 0.9) 0x209; between,
/// no clip. The layer mask is set and put back either way.
pub fn flourish(r: &FlourishRider, rand_game: &mut dyn FnMut() -> u32, calls: &mut Vec<GestureCall>) {
    if !r.riding {
        return;
    }
    let a = r.lean.abs();
    if a <= 0.550_283_6 || r.lean * r.lean_rate <= 0.0 || rand_game() % 0x60 != 0 {
        return;
    }
    calls.push(GestureCall::Mask(layer_mask(r.layer, &[9, 8, 12, 6, 10, 11, 5, 7])));
    if r.crouch < 0.2 {
        calls.push(GestureCall::Clip(if a <= 0.800_027_6 { 0x20a } else { 0x20b }));
    } else if r.crouch > 0.9 {
        calls.push(GestureCall::Clip(0x209));
    }
    calls.push(GestureCall::MaskOff);
}

#[allow(dead_code)]
type _Unused = V4;
